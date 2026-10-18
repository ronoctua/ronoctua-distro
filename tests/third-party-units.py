#!/usr/bin/env python3
"""Third-party units against cargo, unit for unit (`third-party-units-are-cargo-s`).

    third-party-units.py --units FILE --messages FILE --log FILE --target DIR

Run at the end of a cargo build (`cargo build -v --message-format json`) of a
workspace made of some third-party units' plan entries (`tests/third-party-units.nix`).
`--units` names those units: each key's plan entry and the output of the unit
that built it (`nix-support/rustc.json`, the compiler commands it ran, and
`nix-support/unit.json`, what its build script said). For each unit, cargo's
command for the same compilation (found by the plan's own entry among cargo's
messages, and by its file name in the `-v` log) must be the unit's, flag for
flag, once what is the build's own is set aside: the paths of the sources and
of the build directories, the `-C metadata` hashes, the diagnostics format.
Each build script's compilation, what each one said (cfgs, libraries, search
paths, `rustc-env`) and its `links` metadata must be cargo's too. Any
difference fails, naming the unit and the words that differ.
"""

import argparse
import json
import os
import re
import shlex
import sys

RUNNING = re.compile(r"^\s*Running `(.*)`\s*$")
# A source path: a registry package's directory, under cargo's vendored copy or
# a unit's build directory.
PACKAGE_DIR = re.compile(r"/([A-Za-z0-9_.+-]+?-\d+\.\d+\.\d+[A-Za-z0-9.+-]*)/(.*)$")


def norm_path(p):
    """A path as both sides can be compared: a build script's output directory
    as `<OUT:package>`, a package's source as `<SRC:package-version>`."""
    m = re.search(r"/build/([A-Za-z0-9_-]+)-[0-9a-f]{16}/out(/.*)?$", p)
    if m:
        return f"<OUT:{m.group(1)}>{m.group(2) or ''}"
    m = re.match(r"^/build/out-dir/([A-Za-z0-9_-]+)(/.*)?$", p)
    if m:
        return f"<OUT:{m.group(1)}>{m.group(2) or ''}"
    m = re.match(r"^/nix/store/[a-z0-9]{32}-rust-(?:loop-)?dep-([A-Za-z0-9_-]+?)-\d[^/]*/out-dir(/.*)?$", p)
    if m:
        return f"<OUT:{m.group(1)}>{m.group(2) or ''}"
    m = re.match(r"^/nix/store/[a-z0-9]{32}-vendor-cargo-deps/[0-9a-f]{64}/([^/]+)(/.*)?$", p)
    if m:
        return f"<SRC:{m.group(1)}>{m.group(2) or ''}"
    m = re.match(r"^/nix/store/[a-z0-9]{32}-cargo-package-(.+?-\d+\.\d+\.\d+[^/]*)(/.*)?$", p)
    if m:
        return f"<SRC:{m.group(1)}>{m.group(2) or ''}"
    m = re.match(r"^/build/(.+?-\d+\.\d+\.\d+[^/]*)(/.*)?$", p)
    if m:
        return f"<SRC:{m.group(1)}>{m.group(2) or ''}"
    return p


def norm_word(word):
    """A word's path normalised, with what is glued before it (`-I`, `@`)
    kept: the driver moves a path wherever it begins (`path_spans`)."""
    m = re.match(r"([^/]*)(/.*)$", word)
    return m.group(1) + norm_path(m.group(2)) if m else word


def norm_value(value):
    """Every path inside a value (`a:b`, `x=y`, `a b`, `-I/x`, `"/x"`) normalised."""
    return "".join(norm_word(part) for part in re.split(r"([\s:;,=\"'])", value))


def norm_command(argv):
    """A compiler command without what is the build's own: the diagnostics
    format, the `-C metadata` hashes, the output directory, the libraries'
    search path and their file paths, and the source's directory. `--extern`
    is a set to rustc: sorted (cargo orders it by package, the units by
    extern name)."""
    out, externs = [], []
    i = 1
    while i < len(argv):
        a = argv[i]
        nxt = argv[i + 1] if i + 1 < len(argv) else None
        if a.startswith(("--error-format=", "--json=", "--color=", "--remap-path-prefix")):
            i += 1
        elif a == "-C" and nxt and nxt.split("=")[0] in ("metadata", "extra-filename"):
            i += 2
        elif a == "--out-dir" or (a == "-L" and nxt and nxt.startswith("dependency=")):
            i += 2
        elif a == "--extern" and nxt:
            if not externs:
                out.append("<EXTERNS>")
            externs.append(nxt.split("=", 1)[0])
            i += 2
        elif a == "-L" and nxt:
            kind, _, path = nxt.partition("=")
            out += ["-L", f"{kind}={norm_path(path)}"]
            i += 2
        elif a.endswith(".rs") and not a.startswith("-"):
            m = PACKAGE_DIR.search(a)
            out.append(f"<SRC>/{m.group(2)}" if m else a)
            i += 1
        else:
            out.append(a)
            i += 1
    if externs:
        at = out.index("<EXTERNS>")
        out[at : at + 1] = [w for e in sorted(externs) for w in ("--extern", e)]
    return out


def option(argv, flag):
    for i, a in enumerate(argv):
        if a == flag and i + 1 < len(argv):
            return argv[i + 1]
    return None


def codegen(argv, name):
    for i, a in enumerate(argv):
        if a == "-C" and i + 1 < len(argv) and argv[i + 1].startswith(name + "="):
            return argv[i + 1].split("=", 1)[1]
    return None


def source_of(argv):
    return next((a for a in argv[1:] if a.endswith(".rs") and not a.startswith("-")), None)


def package_dir_of(argv):
    m = PACKAGE_DIR.search(source_of(argv) or "")
    return m.group(1) if m else None


def cargo_commands(log):
    """Every rustc command of cargo's `-v` log."""
    out = []
    with open(log, errors="replace") as f:
        for line in f:
            m = RUNNING.match(line)
            if not m:
                continue
            try:
                argv = shlex.split(m.group(1))
            except ValueError:
                continue
            if argv and os.path.basename(argv[0]).startswith("rustc"):
                out.append(argv)
    return out


def script_metadata(output):
    """The `links` metadata a build script printed (`cargo:KEY=VALUE`,
    `cargo::metadata=KEY=VALUE`) — as `rust-unit.py` reads it."""
    known = {
        "rustc-link-lib", "rustc-link-search", "rustc-flags", "rustc-cfg", "rustc-check-cfg",
        "rustc-env", "rustc-cdylib-link-arg", "rustc-link-arg", "rustc-link-arg-bin",
        "rustc-link-arg-bins", "rustc-link-arg-tests", "rustc-link-arg-benches",
        "rustc-link-arg-examples", "warning", "error", "rerun-if-changed", "rerun-if-env-changed",
    }
    found = {}
    for line in output.splitlines():
        if line.startswith("cargo::metadata="):
            k, _, v = line[len("cargo::metadata="):].partition("=")
            found[k] = v
        elif line.startswith("cargo:") and not line.startswith("cargo::"):
            k, _, v = line[len("cargo:"):].partition("=")
            if k not in known:
                found[k] = v
    return found


def said(libs, search, cfgs, env):
    return json.dumps(
        {"libs": libs, "search": search, "cfgs": cfgs, "env": sorted([k, norm_value(v)] for k, v in env)},
        sort_keys=True,
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--units", required=True)
    ap.add_argument("--messages", required=True)
    ap.add_argument("--log", required=True)
    ap.add_argument("--target", required=True)
    a = ap.parse_args()
    with open(a.units) as f:
        units = json.load(f)
    commands = cargo_commands(a.log)
    by_file = {}
    script_compiles = {}
    for argv in commands:
        name, extra = option(argv, "--crate-name"), codegen(argv, "extra-filename")
        if name and name.startswith("build_script_"):
            script_compiles.setdefault(package_dir_of(argv), []).append(norm_command(argv))
        elif name and extra:
            # `-C extra-filename=-<hash>`: the file is `lib<name>-<hash>.rlib`.
            by_file[(name, extra.lstrip("-"))] = argv

    wanted = {}
    for key, u in units.items():
        e = u["entry"]
        t = (e["pkg"], e["version"], "+".join(e["kind"]), e["target"], e["mode"], str(e["opt"]), tuple(sorted(e["features"])))
        wanted[t] = key
    problems = []
    cargo_of = {}
    runs, metadata = {}, {}
    with open(a.messages, errors="replace") as f:
        for raw in f:
            if not raw.startswith("{"):
                continue
            msg = json.loads(raw)
            pkg_id = msg.get("package_id") or ""
            if not pkg_id or pkg_id.startswith("path+"):
                continue
            name, version = pkg_id.rsplit("#", 1)[1].rsplit("@", 1)
            if msg.get("reason") == "build-script-executed":
                search = []
                for value in msg["linked_paths"]:
                    kind, sep, path = value.partition("=")
                    if not sep or "/" in kind:
                        kind, path = "native", value
                    search.append(f"{kind}={norm_path(path)}")
                runs.setdefault(f"{name}@{version}", []).append(said(msg["linked_libs"], search, msg["cfgs"], msg["env"]))
                output = os.path.join(os.path.dirname(msg["out_dir"]), "output")
                with open(output, errors="replace") as o:
                    m = script_metadata(o.read())
                metadata.setdefault(f"{name}@{version}", []).append(json.dumps({k: norm_value(v) for k, v in m.items()}, sort_keys=True))
                continue
            if msg.get("reason") != "compiler-artifact":
                continue
            target = msg["target"]
            if "custom-build" in target["kind"]:
                # A build script's compilation: compared below, by package.
                continue
            mode = "test" if msg["profile"].get("test") else "build"
            t = (name, version, "+".join(target["kind"]), target["name"], mode, str(msg["profile"]["opt_level"]), tuple(sorted(msg["features"])))
            key = wanted.get(t)
            if key is None:
                problems.append(f"cargo built {name}@{version} {target['name']} with {sorted(msg['features'])}, which no unit is")
                continue
            for path in msg["filenames"]:
                m = re.match(r"^lib(.+?)-([0-9a-f]+)\.(rlib|so)$", os.path.basename(path))
                if m and (m.group(1), m.group(2)) in by_file:
                    cargo_of[key] = by_file[(m.group(1), m.group(2))]

    same = {"libraries": 0, "build scripts": 0, "script results": 0, "links metadata": 0}
    for key, u in sorted(units.items()):
        e = u["entry"]
        with open(os.path.join(u["out"], "nix-support", "rustc.json")) as f:
            mine = json.load(f)
        with open(os.path.join(u["out"], "nix-support", "unit.json")) as f:
            info = json.load(f)
        crate = e["target"].replace("-", "_")
        lib_commands = [c for c in mine if option(c, "--crate-name") == crate]
        if key not in cargo_of:
            problems.append(f"{key}: cargo did not build it")
        elif not lib_commands:
            problems.append(f"{key}: the unit ran no compiler command for it")
        else:
            theirs, ours = norm_command(cargo_of[key]), norm_command(lib_commands[-1])
            if theirs == ours:
                same["libraries"] += 1
            else:
                problems.append(
                    f"{key}: cargo's command and the unit's differ — only cargo's: "
                    f"{[w for w in theirs if w not in ours]}, only the unit's: {[w for w in ours if w not in theirs]}"
                    + ("" if set(theirs) != set(ours) else " (the order)")
                )
        script = info.get("script")
        pv, pdir = f"{e['pkg']}@{e['version']}", f"{e['pkg']}-{e['version']}"
        compiles = [c for c in mine if (option(c, "--crate-name") or "").startswith("build_script_")]
        if bool(compiles) != bool(script):
            problems.append(f"{key}: a build script compiled without a result, or a result without one")
            continue
        if not script:
            continue
        if norm_command(compiles[-1]) in script_compiles.get(pdir, []):
            same["build scripts"] += 1
        else:
            problems.append(f"{key}: its build script is compiled otherwise than cargo compiles {pdir}'s: {norm_command(compiles[-1])}")
        result = said(script["libs"], [f"{s['kind']}={norm_path(s['path'])}" for s in script["search"]], script["cfgs"], list(script["env"].items()))
        if result in runs.get(pv, []):
            same["script results"] += 1
        else:
            problems.append(f"{key}: its build script said {result}, cargo's {runs.get(pv, [])}")
        links = json.dumps({k: norm_value(v) for k, v in script["metadata"].items()}, sort_keys=True)
        if links in metadata.get(pv, []):
            same["links metadata"] += 1
        else:
            problems.append(f"{key}: its `links` metadata is {links}, cargo's {metadata.get(pv, [])}")
    if problems:
        print("ERROR: the third-party units and cargo disagree:", file=sys.stderr)
        for p in problems:
            print(f"  {p}", file=sys.stderr)
        sys.exit(1)
    if same["libraries"] < len(units) or same["build scripts"] == 0:
        print(f"ERROR: compared {same}, of {len(units)} units: this check compared nothing", file=sys.stderr)
        sys.exit(1)
    print(f"OK: {len(units)} third-party units are cargo's — " + ", ".join(f"{n} {k}" for k, n in same.items()))


if __name__ == "__main__":
    main()
