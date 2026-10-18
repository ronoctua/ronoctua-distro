#!/usr/bin/env python3
"""A third-party layer, named by the plan's keys — and refused when it is not
the plan's.

    rust-index.py --third-party FILE --raw DIR --out DIR

Run in a derivation of its own (`nix/rust/layers.nix`), beside the layer's
cargo build: it reads what `rust-layer.py` kept (the libraries, the
build-script directories, cargo's JSON messages and `-v` log), finds each
third-party unit the plan names among cargo's messages, and writes
`index.json` keyed exactly like the plan. Every unit of ours reads that file,
so the derivation's inputs are only the plan's THIRD-PARTY half (the file
`--third-party` names, cut from `nix/rust/plan.json` at evaluation), the
layer and this script: a new test, a new path dependency between two of our
crates or a feature of ours changes none of them, and no unit is rebuilt for
it. That is also why this is a file of its own and not a subcommand of
`rust-plan.py`: an edit to the planner must not rebuild every unit.

# Spike S1, on every build

The layer and the plan must hold the same third-party units — the same
package, version, target, mode, opt-level and features —, and each library
must carry what cargo would have compiled it with for LTO (`-C
linker-plugin-lto`, `-C embed-bitcode=no`, or both), read from the `-v` log:
the layer is built from a workspace of its own, not ours, so its roots are
not our roots, and a library compiled without the bitcode a thin-LTO program
needs would fail at the link, minutes later. Any difference FAILS here,
naming each unit, before a single unit of ours is handed a crate built
another way.

# A build script's `links` metadata

What a build script of the layer printed for its dependents (`DEP_<links>_*`)
can name its own `OUT_DIR`, in the layer's sandbox. `rust-layer.py` kept those
paths under `native/`; here they are written relative to the index
(`@LAYER@/native/…`), and the unit that hands them to a build script of ours
puts its own layer in place, so the variable names a directory that exists —
as it does under cargo. A path of the layer's sandbox that was not kept is
recorded as unreachable, and a build script that would receive it is refused.
"""

import argparse
import json
import os
import re
import shlex
import sys

FORMAT = 1
# What `rust-unit.py` replaces with the layer's path.
LAYER_TOKEN = "@LAYER@"
RUNNING = re.compile(r"^\s*Running `(.*)`\s*$")


def die(message):
    print(f"rust-index: {message}", file=sys.stderr)
    sys.exit(1)


def load(path):
    with open(path) as f:
        return json.load(f)


def read(path):
    with open(path) as f:
        return f.read()


def lto_of(codegen):
    """What cargo's `-C` options for LTO say, in the plan's words (`lto_map`
    in `rust-plan.py`): `run:<name>`, `off`, `bitcode` (`-C
    linker-plugin-lto`), `object` (`-C embed-bitcode=no`) or `both` (rustc's
    default, nothing said)."""
    lto = None
    for option in codegen:
        if option == "lto":
            lto = "run:"
        elif option.startswith("lto="):
            value = option[4:]
            lto = "off" if value == "off" else f"run:{value}"
    if lto is not None:
        return lto
    if "linker-plugin-lto" in codegen:
        return "bitcode"
    if "embed-bitcode=no" in codegen:
        return "object"
    return "both"


def rustc_lines(logs):
    """`(crate-name, extra-filename)` → what cargo passed that unit: the `-L`
    search paths, in order, and its LTO — read from `cargo -v`, the only place
    the per-unit command line is printed. Every log of the layer together: a
    unit the second command found fresh was printed by the first."""
    out = {}
    for log in logs:
        with open(log, errors="replace") as f:
            lines = f.read().splitlines()
        for line in lines:
            m = RUNNING.match(line)
            if not m:
                continue
            try:
                argv = shlex.split(m.group(1))
            except ValueError:
                continue
            if not argv or not os.path.basename(argv[0]).startswith("rustc"):
                continue
            name = extra = None
            search = []
            codegen = []
            i = 0
            while i < len(argv):
                a = argv[i]
                nxt = argv[i + 1] if i + 1 < len(argv) else ""
                if a == "--crate-name":
                    name = nxt
                    i += 2
                    continue
                if a == "-C":
                    if nxt.startswith("extra-filename="):
                        extra = nxt.split("=", 1)[1]
                    codegen.append(nxt)
                    i += 2
                    continue
                if a.startswith("-C") and len(a) > 2:
                    codegen.append(a[2:])
                    if a[2:].startswith("extra-filename="):
                        extra = a[2:].split("=", 1)[1]
                    i += 1
                    continue
                if a == "-L" and not nxt.startswith("dependency="):
                    search.append(nxt)
                    i += 2
                    continue
                if a.startswith("-L") and len(a) > 2 and not a[2:].startswith("dependency="):
                    search.append(a[2:])
                i += 1
            if name is not None and extra is not None:
                out[(name, extra)] = {"search": search, "lto": lto_of(codegen)}
    return out


def match_tuple(name, version, kinds, target, mode, opt, features):
    return (name, version, "+".join(kinds), target, mode, str(opt), tuple(sorted(features)))


def third_party(msg):
    """A message about a crate that is not ours (`build-finished` and its kin
    are about no crate at all, and the layer's own workspace crates are
    `path+`)."""
    pkg = msg.get("package_id")
    return pkg is not None and not pkg.startswith("path+")


def messages_of(path):
    with open(path, errors="replace") as f:
        for raw in f:
            raw = raw.strip()
            if raw.startswith("{"):
                yield json.loads(raw)


def split_search(value):
    kind, sep, path = value.partition("=")
    if sep and "/" not in kind:
        return kind, path
    return "native", value


def inside(path, root):
    real = os.path.realpath(path)
    return real == root or real.startswith(root + os.sep)


def under(root):
    """A path under `root` inside a value: at its start or after a
    separator, and followed by `/` or the end of the path."""
    return re.compile(r"(?:^|(?<=[\s:;,=]))" + re.escape(root) + r"(/[^\s:;,]*)?(?=$|[\s:;,])")


def relocated(metadata, target_dir, build_top, raw):
    """A build script's `links` metadata with the layer's target directory
    written as `@LAYER@/native` — where the units find what `rust-layer.py`
    kept —, and the keys whose value names some other path of the layer's
    sandbox, which no unit can see."""
    out, unreachable = {}, []
    in_target = under(target_dir)
    in_sandbox = under(build_top) if build_top else None
    for key, value in metadata.items():
        rest = in_target.sub("", value)
        if in_sandbox is not None and in_sandbox.search(rest):
            unreachable.append(key)
        missing = [
            m.group(0)
            for m in in_target.finditer(value)
            if not os.path.exists(os.path.join(raw, "native", os.path.normpath((m.group(1) or "/").lstrip("/") or ".")))
        ]
        if missing:
            unreachable.append(key)
        out[key] = in_target.sub(lambda m: f"{LAYER_TOKEN}/native{m.group(1) or ''}", value)
    return out, sorted(set(unreachable))


def cmd_index(args):
    """Names every unit of the layer by the plan's key, and refuses a layer
    whose units are not the plan's (spike S1)."""
    plan = load(args.third_party)
    raw = args.raw
    out = args.out
    target_dir = read(os.path.join(raw, "cargo", "target-dir")).strip()
    top_file = os.path.join(raw, "cargo", "build-top")
    build_top = read(top_file).strip() if os.path.exists(top_file) else ""
    runs = [r for r in read(os.path.join(raw, "cargo", "runs")).split() if r]
    lines = rustc_lines([os.path.join(raw, "cargo", f"{r}.log") for r in runs])
    index = {}
    problems = []

    def place(value):
        kind, path = split_search(value)
        if inside(path, target_dir):
            rel = os.path.relpath(os.path.realpath(path), target_dir)
            if not os.path.isdir(os.path.join(raw, "native", rel)):
                problems.append(f"a search path {path} that the layer did not keep")
            return {"layer": os.path.join("native", rel), "kind": kind}
        return {"path": path, "kind": kind}

    for graph in runs:
        if graph not in plan["graphs"]:
            die(f"the layer ran the {graph} graph, and the plan's third-party half has none")
        units = plan["graphs"][graph]["units"]
        wanted = {}
        runs_wanted = {}
        for key, unit in units.items():
            if unit["workspace"]:
                continue
            if unit["mode"] == "run-custom-build":
                pkg = (unit["pkg"], unit["version"])
                runs_wanted[pkg] = runs_wanted.get(pkg, 0) + 1
                continue
            t = match_tuple(unit["pkg"], unit["version"], unit["kind"], unit["target"], unit["mode"], unit["opt"], unit["features"])
            if t in wanted:
                die(f"{graph}: {key} and {wanted[t]} look the same to cargo's messages")
            wanted[t] = key
        found = set()
        runs_found = {}
        for msg in messages_of(os.path.join(raw, "cargo", f"{graph}.json")):
            if not third_party(msg):
                continue
            name, version = msg["package_id"].rsplit("#", 1)[1].rsplit("@", 1)
            reason = msg.get("reason")
            if reason == "build-script-executed":
                runs_found[(name, version)] = runs_found.get((name, version), 0) + 1
                continue
            if reason != "compiler-artifact":
                continue
            target = msg["target"]
            mode = "test" if msg["profile"].get("test") else "build"
            t = match_tuple(name, version, target["kind"], target["name"], mode, msg["profile"]["opt_level"], msg["features"])
            key = wanted.get(t)
            if key is None:
                problems.append(f"{graph}: cargo built {name}@{version} {target['name']} {t[4:]}, which the plan does not have")
                continue
            found.add(key)
            files, search = [], []
            names = msg["filenames"]
            command = None
            for f in names:
                base = os.path.basename(f)
                if base.endswith(".rmeta") and any(x.endswith(".rlib") for x in names):
                    continue
                if not base.endswith((".rlib", ".so", ".rmeta")):
                    continue
                files.append(os.path.join("deps", base))
                stem = re.match(r"^lib(.+?)(-[0-9a-f]+)\.(rlib|so|rmeta)$", base)
                if stem and (stem.group(1), stem.group(2)) in lines:
                    command = lines[(stem.group(1), stem.group(2))]
                    search = command["search"]
            if files:
                # A LIBRARY A UNIT LINKS: its command line must be in the log,
                # and its LTO the plan's.
                if command is None:
                    problems.append(f"{graph}: cargo's log has no command line for {key}")
                elif command["lto"] != units[key]["lto"]:
                    problems.append(
                        f"{graph}: cargo compiled {key} for LTO as `{command['lto']}`, and the plan says `{units[key]['lto']}`"
                    )
            entry = {"files": sorted(set(files)), "native": [place(v) for v in search]}
            if key in index and index[key] != entry:
                problems.append(f"{graph}: {key} came out twice, differently")
            index[key] = entry
        for key in sorted(set(wanted.values()) - found):
            problems.append(f"{graph}: the plan has {key}, which cargo did not build")
        for pkg in sorted(set(runs_wanted) | set(runs_found)):
            # A FRESH BUILD SCRIPT IS REPORTED AGAIN by a layer's second
            # command, so cargo may only report AT LEAST the plan's runs.
            if runs_found.get(pkg, 0) < runs_wanted.get(pkg, 0):
                problems.append(
                    f"{graph}: the plan runs {pkg[0]}@{pkg[1]}'s build script {runs_wanted.get(pkg, 0)} times, cargo reported {runs_found.get(pkg, 0)}"
                )
    # EACH BUILD SCRIPT RUN'S `links` METADATA, where cargo's report names the
    # run without doubt: one run of the package in the graph, or several that
    # printed the same. Another is left out, and a unit that needs it says so.
    runs_index = {}
    scripts_file = os.path.join(raw, "cargo", "scripts.json")
    reported = load(scripts_file) if os.path.exists(scripts_file) else []
    for graph in runs:
        units = plan["graphs"][graph]["units"]
        by_pkg = {}
        for key, unit in units.items():
            if not unit["workspace"] and unit["mode"] == "run-custom-build":
                by_pkg.setdefault((unit["pkg"], unit["version"]), []).append(key)
        for (name, version), keys in by_pkg.items():
            links = plan.get("links", {}).get(f"{name}@{version}")
            if links is None:
                continue
            said = [
                r["metadata"]
                for r in reported
                if r["graph"] == graph and r["package_id"].rsplit("#", 1)[1] == f"{name}@{version}"
            ]
            distinct = {json.dumps(m, sort_keys=True) for m in said}
            if len(distinct) == 1:
                metadata, unreachable = relocated(said[0], target_dir, build_top, raw)
                for key in keys:
                    runs_index[key] = {"links": links, "metadata": metadata, "unreachable": unreachable}
    if problems:
        die("the layer and the plan disagree (run `nix run .#rust-plan`):\n  " + "\n  ".join(problems))
    os.makedirs(out, exist_ok=True)
    for d in ("deps", "native"):
        if os.path.isdir(os.path.join(raw, d)):
            os.symlink(os.path.join(raw, d), os.path.join(out, d))
    with open(os.path.join(out, "index.json"), "w") as f:
        json.dump({"format": FORMAT, "units": index, "runs": runs_index}, f, sort_keys=True, indent=0)
        f.write("\n")
    print(f"rust-index: indexed {len(index)} third-party units of {', '.join(runs)}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--third-party", required=True, help="the plan's third-party half (`nix/rust/layers.nix`)")
    parser.add_argument("--raw", required=True, help="what `rust-layer.py` kept")
    parser.add_argument("--out", required=True)
    cmd_index(parser.parse_args())


if __name__ == "__main__":
    main()
