#!/usr/bin/env python3
"""Keeps what a third-party layer's units link, out of a cargo build.

    rust-layer.py --target DIR --out DIR --run GRAPH=MESSAGES,VERBOSE-LOG...

Run at the end of a layer's cargo build (`nix/rust/layers.nix`): copies the
third-party libraries cargo reported (`deps/`), the build-script directories
their search paths point into (`native/`), and cargo's own record of the build
(the JSON messages and the `-v` log of each command, the target directory's
path, and the `links` metadata each build script printed, with the paths it
names) into `cargo/`, for `rust-index.py` to name by the plan's keys.

It is its own file, small and with no plan in it, because the layer it runs in
is the most expensive derivation of the build: an edit to the indexer must not
compile five hundred crates again.
"""

import argparse
import json
import os
import re
import shutil


def messages_of(path):
    for raw in open(path, errors="replace"):
        raw = raw.strip()
        if raw.startswith("{"):
            yield json.loads(raw)


def third_party(msg):
    """A message about a crate that is not ours (`build-finished` and its kin
    are about no crate at all)."""
    pkg = msg.get("package_id")
    return pkg is not None and not pkg.startswith("path+")


def metadata_of(output):
    """The `links` metadata a build script printed (`cargo:KEY=VALUE` in the
    old syntax, `cargo::metadata=KEY=VALUE` in the new): what cargo hands its
    dependents' build scripts as `DEP_<LINKS>_<KEY>`."""
    known = {
        "rustc-link-lib", "rustc-link-search", "rustc-flags", "rustc-cfg", "rustc-check-cfg",
        "rustc-env", "rustc-cdylib-link-arg", "rustc-link-arg", "rustc-link-arg-bin",
        "rustc-link-arg-bins", "rustc-link-arg-tests", "rustc-link-arg-benches",
        "rustc-link-arg-examples", "warning", "error", "rerun-if-changed", "rerun-if-env-changed",
    }
    found = {}
    for line in output.splitlines():
        if line.startswith("cargo::metadata="):
            key, _, value = line[len("cargo::metadata="):].partition("=")
            found[key] = value
        elif line.startswith("cargo:") and not line.startswith("cargo::"):
            key, _, value = line[len("cargo:"):].partition("=")
            if key not in known:
                found[key] = value
    return found


def split_search(value):
    kind, sep, path = value.partition("=")
    if sep and "/" not in kind:
        return kind, path
    return "native", value


def inside(path, root):
    real = os.path.realpath(path)
    return real == root or real.startswith(root + os.sep)


def paths_in(value, root):
    """The paths under `root` a metadata value names: the whole value, or one
    of several separated by `:`, `;`, `,` or blanks."""
    pattern = re.compile(r"(?:^|(?<=[\s:;,=]))" + re.escape(root) + r"(?:/[^\s:;,]*)?(?=$|[\s:;,])")
    return [m.group(0) for m in pattern.finditer(value)]


def keep(args):
    """In the cargo derivation: keep what a unit links — the third-party
    libraries and the build-script directories their search paths point into
    — and cargo's own record of the build, for `index` to read."""
    target_dir = os.path.realpath(args.target)
    out = args.out
    os.makedirs(os.path.join(out, "deps"), exist_ok=True)
    os.makedirs(os.path.join(out, "cargo"), exist_ok=True)
    runs = []
    scripts = []
    kept_dirs = set()
    for spec in args.run:
        graph, files = spec.split("=", 1)
        messages, log = files.split(",", 1)
        runs.append(graph)
        shutil.copy2(messages, os.path.join(out, "cargo", f"{graph}.json"))
        shutil.copy2(log, os.path.join(out, "cargo", f"{graph}.log"))
        for msg in messages_of(messages):
            if not third_party(msg):
                continue
            if msg.get("reason") == "compiler-artifact":
                names = msg["filenames"]
                for f in names:
                    base = os.path.basename(f)
                    if base.endswith(".rmeta") and any(x.endswith(".rlib") for x in names):
                        continue
                    if base.endswith((".rlib", ".so", ".rmeta")):
                        dest = os.path.join(out, "deps", base)
                        if not os.path.exists(dest):
                            shutil.copy2(f, dest)
            elif msg.get("reason") == "build-script-executed":
                output = os.path.join(os.path.dirname(msg["out_dir"]), "output")
                metadata = metadata_of(open(output, errors="replace").read()) if os.path.exists(output) else {}
                scripts.append(
                    {
                        "graph": graph,
                        "package_id": msg["package_id"],
                        "out_dir": msg["out_dir"],
                        "metadata": metadata,
                    }
                )
                for value in msg.get("linked_paths", []):
                    _, path = split_search(value)
                    if inside(path, target_dir):
                        rel = os.path.relpath(os.path.realpath(path), target_dir)
                        if rel not in kept_dirs and os.path.isdir(path):
                            shutil.copytree(path, os.path.join(out, "native", rel), symlinks=True, dirs_exist_ok=True)
                            kept_dirs.add(rel)
                # AND WHAT THE `links` METADATA NAMES in the target directory
                # (aws-lc's `include`, `libdir`, `root`): cargo hands it to a
                # dependent's build script as `DEP_<links>_*`, and the
                # dependent must find it where the index says it is.
                for value in metadata.values():
                    for path in paths_in(value, target_dir):
                        rel = os.path.relpath(os.path.realpath(path), target_dir)
                        dest = os.path.join(out, "native", rel)
                        if os.path.lexists(dest) or not os.path.exists(path):
                            continue
                        if os.path.isdir(path):
                            shutil.copytree(path, dest, symlinks=True)
                        else:
                            os.makedirs(os.path.dirname(dest), exist_ok=True)
                            shutil.copy2(path, dest)
                        kept_dirs.add(rel)
    write = lambda name, value: open(os.path.join(out, "cargo", name), "w").write(value)
    write("target-dir", target_dir + "\n")
    # The sandbox's root: a metadata path under it and outside the target
    # directory is one no unit can see, and the index says so.
    write("build-top", os.path.realpath(os.environ.get("NIX_BUILD_TOP", os.path.dirname(target_dir))) + "\n")
    write("runs", "\n".join(runs) + "\n")
    write("scripts.json", json.dumps(scripts, sort_keys=True, indent=0) + "\n")
    print(f"rust-layer: kept {len(os.listdir(os.path.join(out, 'deps')))} third-party files and {len(kept_dirs)} build-script directories")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target", required=True, help="cargo's target directory")
    parser.add_argument("--out", required=True)
    parser.add_argument("--run", action="append", default=[], help="GRAPH=MESSAGES,VERBOSE-LOG")
    keep(parser.parse_args())


if __name__ == "__main__":
    main()
