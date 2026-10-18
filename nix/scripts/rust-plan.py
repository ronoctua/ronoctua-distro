#!/usr/bin/env python3
"""Cargo plans, Nix builds: the workspace's unit graph, normalised.

    rust-plan.py plan --metadata FILE --manifest FILE --graph NAME=FILE...
                      --features F --rustc FILE --inputs HASH > plan.json

`plan` reads what `cargo … --unit-graph` printed for each flavour (release, loop,
test and the check graph clippy walks) and writes `nix/rust/plan.json`: every
unit under a key, relative paths only, sorted, one unit per line so that a diff
of the file says which unit changed. The key is written here and nowhere else:
`rust-index.py` names the layers' units by the plan's own entries (package,
version, target, mode, opt-level, features), never by a key of its own.
Clippy's targets come from the check graph, and a plain target the tests
unify with more features is linted a second time with the product's
(`product_twins`).

# The key

`<name>@<version>/<kind>:<target>/<mode>/o<opt-level>/<hash>`, where the hash
covers the features, the profile, the target's shape and the KEYS of every
dependency. So a key is a Merkle name: two units with one key are the same
build, in any graph, and a dependency built differently gives every unit above
it a different key. That is what lets the loop layer hold the loop graph and
the test graph side by side (the same crate, compiled with the test graph's
extra features, is a second key), and what lets clippy's check graph name the
test graph's libraries (a `check` unit is keyed as the `build` it would be).

# LTO

Cargo decides per unit whether an rlib carries objects, bitcode or both, from
the graph's roots (`cargo/src/cargo/core/compiler/lto.rs`). It is computed here
the same way, once, so a unit's derivation passes the flag cargo would have
passed without knowing the graph.
"""

import argparse
import hashlib
import json
import os
import re
import sys

sys.setrecursionlimit(100000)

FORMAT = 1
# Modes whose units are compiled as test harnesses: cargo treats them as a
# binary for LTO.
TEST_MODES = {"test", "bench", "doctest"}
CAN_LTO = {"bin", "staticlib", "cdylib"}
DYNAMIC = {"dylib", "cdylib", "proc-macro"}


def die(message):
    print(f"rust-plan: {message}", file=sys.stderr)
    sys.exit(1)


def load(path):
    with open(path) as f:
        return json.load(f)


def read(path):
    with open(path) as f:
        return f.read()


def digest(value, width=12):
    data = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(data).hexdigest()[:width]


def strip_of(profile):
    """`{"resolved": {"Named": "debuginfo"}}` → `"debuginfo"`."""
    strip = profile.get("strip")
    while isinstance(strip, dict):
        if not strip:
            return None
        strip = next(iter(strip.values()))
    return strip


# Which built-in profile each named one descends from (`inherits`): a build
# script's `PROFILE` is `release` or `debug` by that root, not by the name.
PROFILE_ROOTS = {"dev": "dev", "release": "release", "test": "dev", "bench": "release"}


def profile_roots(manifest):
    declared = manifest.get("profile", {})

    def root(name, seen=()):
        if name in ("dev", "release"):
            return name
        if name in seen:
            die(f"the profile {name} inherits itself")
        parent = declared.get(name, {}).get("inherits") or PROFILE_ROOTS.get(name)
        if parent is None:
            die(f"the profile {name} inherits nothing cargo knows")
        return root(parent, seen + (name,))

    return {name: root(name) for name in set(declared) | set(PROFILE_ROOTS)}


ROOTS = {}


def profile_of(profile):
    """A unit's profile without its name: two profiles that compile the same
    way, from the same root, are one profile, whatever `[profile.*]` they came
    from."""
    out = {k: v for k, v in profile.items() if k != "name"}
    out["strip"] = strip_of(profile)
    out["root"] = ROOTS.get(profile.get("name"), "dev")
    return out


def crate_types(unit):
    if unit["mode"] in TEST_MODES:
        return ["bin"]
    return unit["target"]["crate_types"]


def for_host(unit):
    kinds = unit["target"]["kind"]
    return "custom-build" in kinds or "proc-macro" in kinds


def profile_lto(unit):
    value = unit["profile"]["lto"]
    if value == "false":
        return ("bool", False)
    if value == "true":
        return ("bool", True)
    if value == "off":
        return ("off", None)
    return ("named", value)


def needs_object(types):
    return any(t in CAN_LTO or t in DYNAMIC for t in types)


def when_needs_object(types):
    return "object" if all(t == "dylib" for t in types) else "both"


def lto_map(units, roots):
    """cargo's `lto::generate`, unit for unit. Values: `run:<name>` (the unit
    runs LTO: `-C lto=<name>`, or `-C lto` for an empty name), `off`,
    `object` (`-C embed-bitcode=no`), `bitcode` (`-C linker-plugin-lto`) and
    `both` (rustc's default)."""
    result = {}

    def calculate(index, parent):
        unit = units[index]
        types = crate_types(unit)
        all_lto = all(t in CAN_LTO for t in types)
        kind, name = profile_lto(unit)
        if for_host(unit):
            lto = "object"
        elif all_lto:
            if kind == "named":
                lto = f"run:{name}"
            elif kind == "off":
                lto = "off"
            elif kind == "bool" and name:
                lto = "run:"
            else:
                lto = "object"
        else:
            obj = needs_object(types)
            if parent.startswith("run:") and not obj:
                lto = "bitcode"
            elif (parent.startswith("run:") or parent == "bitcode") and obj:
                lto = when_needs_object(types)
            elif parent == "off":
                lto = "off"
            else:
                lto = parent
        if index in result:
            old = result[index]
            if lto == old:
                return
            if lto.startswith("run:"):
                merged = lto
            elif old.startswith("run:"):
                merged = old
            elif "off" in (lto, old):
                merged = "off"
            else:
                # `both` with anything, and `object` with `bitcode`.
                merged = "both"
            if merged == old:
                return
            result[index] = merged
        else:
            result[index] = merged = lto
        for dep in unit["dependencies"]:
            calculate(dep["index"], merged)

    for root in roots:
        unit = units[root]
        kind, name = profile_lto(unit)
        if kind == "bool" and not name:
            first = "object"
        elif kind == "off":
            first = "off"
        else:
            types = unit["target"]["crate_types"]
            if for_host(unit):
                first = "object"
            elif needs_object(types):
                first = when_needs_object(types)
            else:
                first = "bitcode"
        calculate(root, first)
    return result


class Graph:
    """One `--unit-graph` output, with every unit keyed."""

    def __init__(self, name, data, packages, root, as_build=False):
        if data.get("version") != 1:
            die(f"the {name} unit graph is format {data.get('version')}, this reads format 1")
        self.name = name
        self.units = data["units"]
        self.roots = data["roots"]
        self.packages = packages
        self.root = root
        self.as_build = as_build
        self.keys = {}
        for index in range(len(self.units)):
            self.key(index)
        seen = {}
        for index, key in self.keys.items():
            if key in seen and not as_build:
                die(f"two units of the {name} graph share the key {key}")
            seen[key] = index
        self.by_key = seen

    def package(self, unit):
        pkg = self.packages.get(unit["pkg_id"])
        if pkg is None:
            die(f"`cargo metadata` does not know the package {unit['pkg_id']}")
        return pkg

    def mode(self, unit):
        mode = unit["mode"]
        if self.as_build and mode == "check":
            return "build"
        return mode

    def key(self, index):
        if index in self.keys:
            return self.keys[index]
        unit = self.units[index]
        pkg = self.package(unit)
        target = unit["target"]
        mode = self.mode(unit)
        deps = sorted(
            (d["extern_crate_name"], self.key(d["index"]), bool(d.get("noprelude")), bool(d.get("public")))
            for d in unit["dependencies"]
        )
        body = [
            sorted(unit["features"]),
            profile_of(unit["profile"]),
            target["kind"],
            target["crate_types"],
            target["edition"],
            bool(target.get("test")),
            mode,
            deps,
        ]
        kind = "+".join(target["kind"])
        key = f"{pkg['name']}@{pkg['version']}/{kind}:{target['name']}/{mode}/o{unit['profile']['opt_level']}/{digest(body)}"
        self.keys[index] = key
        return key

    def relative_src(self, unit):
        pkg = self.package(unit)
        src = unit["target"]["src_path"]
        rel = os.path.relpath(src, pkg["dir_abs"])
        if rel.startswith(".."):
            die(f"{unit['target']['name']}: the target root {src} is outside its package")
        return rel

    def entry(self, index, lto):
        unit = self.units[index]
        pkg = self.package(unit)
        target = unit["target"]
        return {
            "pkg": pkg["name"],
            "version": pkg["version"],
            "workspace": pkg["workspace"],
            "kind": target["kind"],
            "crate_types": target["crate_types"],
            "target": target["name"],
            "src": self.relative_src(unit),
            "edition": target["edition"],
            "test": bool(target.get("test")),
            "mode": self.mode(unit),
            "opt": unit["profile"]["opt_level"],
            "profile": digest(profile_of(unit["profile"]), 8),
            "lto": lto,
            "features": sorted(unit["features"]),
            "deps": [
                {"extern": e, "key": k, **({"noprelude": True} if n else {}), **({"public": True} if p else {})}
                for e, k, n, p in sorted(
                    (d["extern_crate_name"], self.key(d["index"]), bool(d.get("noprelude")), bool(d.get("public")))
                    for d in unit["dependencies"]
                )
            ],
        }


def packages_of(metadata):
    root = metadata["workspace_root"]
    members = set(metadata["workspace_members"])
    out = {}
    for pkg in metadata["packages"]:
        dir_abs = os.path.dirname(pkg["manifest_path"])
        workspace = pkg["id"] in members
        out[pkg["id"]] = {
            "name": pkg["name"],
            "version": pkg["version"],
            "workspace": workspace,
            "dir_abs": dir_abs,
            "dir": os.path.relpath(dir_abs, root) if workspace else None,
            "source": pkg.get("source"),
            "links": pkg.get("links"),
            "features": sorted(pkg.get("features", {}).keys()),
        }
    return out


def clippy_targets(check, test, test_keys):
    """What `cargo clippy --all-targets` lints, crate by crate: each root of
    the check graph (a target checked plainly or as a test) and each
    workspace build script, with its dependencies named by TEST-graph keys.

    A check unit is keyed as the build it would be, so its dependencies'
    keys ARE the test graph's; one that is not there is a graph clippy and the
    tests disagree on, refused."""
    per_crate = {}

    def add(index, as_test):
        unit = check.units[index]
        pkg = check.package(unit)
        target = unit["target"]
        deps = []
        for d in unit["dependencies"]:
            key = check.key(d["index"])
            dep = check.units[d["index"]]
            if dep["mode"] == "run-custom-build":
                deps.append({"extern": d["extern_crate_name"], "key": key, "run": True})
                if key not in test_keys:
                    die(f"clippy: the build script run {key} is not in the test graph")
                continue
            if "bin" in dep["target"]["kind"]:
                continue
            if key not in test_keys:
                die(f"clippy: {target['name']} needs {key}, which the test graph does not build")
            deps.append({"extern": d["extern_crate_name"], "key": key})
        deps.sort(key=lambda d: (d["extern"], d["key"]))
        per_crate.setdefault(pkg["name"], []).append(
            {
                "target": target["name"],
                "kind": target["kind"],
                "crate_types": target["crate_types"],
                "src": check.relative_src(unit),
                "edition": target["edition"],
                "test": as_test,
                "features": sorted(unit["features"]),
                "deps": deps,
            }
        )

    groups = {}
    for root in check.roots:
        unit = check.units[root]
        if not check.package(unit)["workspace"]:
            continue
        groups.setdefault((unit["pkg_id"], tuple(unit["target"]["kind"]), unit["target"]["name"]), []).append(root)
    for (pkg_id, kind, name), roots in sorted(groups.items()):
        if "test" in kind or "bench" in kind:
            for r in roots:
                add(r, True)
            continue
        if len(roots) == 1:
            # `--all-targets` checks a lib or bin both plainly and as a
            # test; a target with `test = false` only plainly.
            add(roots[0], False)
            continue
        if len(roots) != 2:
            die(f"clippy: {name} is checked {len(roots)} times")
        # THE ONE WITH THE DEV-DEPENDENCIES IS THE TEST: its dependency set is
        # the test graph's harness for the same target. Without any
        # dev-dependency the two are the same unit, and either is either.
        harness = [
            i
            for i, u in enumerate(test.units)
            if u["pkg_id"] == pkg_id and u["mode"] == "test" and u["target"]["name"] == name
        ]
        want = set()
        if harness:
            want = {test.key(d["index"]) for d in test.units[harness[0]]["dependencies"]}
        a, b = roots
        deps_a = {check.key(d["index"]) for d in check.units[a]["dependencies"]}
        deps_b = {check.key(d["index"]) for d in check.units[b]["dependencies"]}
        if deps_b == want and deps_a != want:
            a, b = b, a
        add(a, True)
        add(b, False)

    # AND EVERY WORKSPACE BUILD SCRIPT, which cargo compiles through the
    # workspace wrapper too — so clippy lints `build.rs`.
    for index, unit in enumerate(check.units):
        pkg = check.package(unit)
        if pkg["workspace"] and "custom-build" in unit["target"]["kind"] and unit["mode"] == "build":
            entry_deps = []
            for d in unit["dependencies"]:
                key = check.key(d["index"])
                if key not in test_keys:
                    die(f"clippy: {pkg['name']}'s build script needs {key}, which the test graph does not build")
                entry_deps.append({"extern": d["extern_crate_name"], "key": key})
            per_crate.setdefault(pkg["name"], []).append(
                {
                    "target": unit["target"]["name"],
                    "kind": unit["target"]["kind"],
                    "crate_types": unit["target"]["crate_types"],
                    "src": check.relative_src(unit),
                    "edition": unit["target"]["edition"],
                    "test": False,
                    "features": sorted(unit["features"]),
                    "deps": sorted(entry_deps, key=lambda d: (d["extern"], d["key"])),
                }
            )
    for name in per_crate:
        per_crate[name].sort(key=lambda t: ("+".join(t["kind"]), t["target"], t["test"]))
    return per_crate


def product_twins(clippy, product):
    """A plain target the tests unify DIFFERENTLY, linted once more with the
    features a product compiles it with.

    `cargo check --all-targets` unifies the dev-dependencies' features into
    every unit of a crate: a library that another crate's tests name with
    `test-support` (`ronoctua-consent`, the shell's kit, door and inspection
    panels) is checked with it on, plainly and as a test, and what it compiles
    only WITHOUT it — the clipboard's real `wl-copy`, the guard's product
    branch, an import only the product uses — is never linted. While the shell
    was one crate its program was checked plainly, and that was the product's
    side; split, nothing was.

    So each such target gets a twin: the product's features (`product`, the
    loop graph's own unit of that target), against the same test-graph
    libraries (features only add, so the product's code compiles against
    them), with the dependencies the product does not have dropped — an
    `--extern` it has no use for is not a product's build. A product
    dependency the check does not name is refused: the twin would be linted
    against a graph nobody builds."""
    units = {}
    for unit in product["units"].values():
        if unit["workspace"] and unit["mode"] == "build" and ("lib" in unit["kind"] or "bin" in unit["kind"]):
            units[(unit["pkg"], tuple(unit["kind"]), unit["target"])] = unit
    for crate, targets in clippy.items():
        twins = []
        for t in targets:
            if t["test"] or "custom-build" in t["kind"]:
                continue
            unit = units.get((crate, tuple(t["kind"]), t["target"]))
            if unit is None or unit["features"] == t["features"]:
                continue
            externs = {d["extern"] for d in unit["deps"] if not units_of_kind(product, d["key"], ("bin", "custom-build"))}
            have = {d["extern"] for d in t["deps"] if not d.get("run")}
            missing = sorted(externs - have)
            if missing:
                die(f"clippy: {crate}'s product {t['target']} needs {', '.join(missing)}, which its check does not name")
            twin = dict(t)
            twin["features"] = unit["features"]
            twin["product"] = True
            twin["deps"] = [d for d in t["deps"] if d.get("run") or d["extern"] in externs]
            twins.append(twin)
        if twins:
            targets.extend(twins)
            targets.sort(key=lambda t: ("+".join(t["kind"]), t["target"], t["test"], bool(t.get("product"))))
    return clippy


def units_of_kind(graph, key, kinds):
    """Whether the unit `key` of a plan graph is one of these target kinds —
    a build script's run and a program are not `--extern`s of a check."""
    unit = graph["units"].get(key)
    return unit is not None and (any(k in unit["kind"] for k in kinds) or unit["mode"] == "run-custom-build")


def write_plan(plan, out):
    """One unit per line: a diff of the plan names the units that changed."""
    lines = ["{"]
    head = {k: v for k, v in plan.items() if k not in ("graphs", "clippy", "packages", "profiles")}
    for k in sorted(head):
        lines.append(f"{json.dumps(k)}:{json.dumps(head[k], sort_keys=True, separators=(',', ':'))},")
    lines.append('"packages":{')
    names = sorted(plan["packages"])
    for i, name in enumerate(names):
        comma = "," if i + 1 < len(names) else ""
        lines.append(f"{json.dumps(name)}:{json.dumps(plan['packages'][name], sort_keys=True, separators=(',', ':'))}{comma}")
    lines.append("},")
    lines.append('"profiles":{')
    names = sorted(plan["profiles"])
    for i, name in enumerate(names):
        comma = "," if i + 1 < len(names) else ""
        lines.append(f"{json.dumps(name)}:{json.dumps(plan['profiles'][name], sort_keys=True, separators=(',', ':'))}{comma}")
    lines.append("},")
    lines.append('"clippy":{')
    names = sorted(plan["clippy"])
    for i, name in enumerate(names):
        comma = "," if i + 1 < len(names) else ""
        lines.append(f"{json.dumps(name)}:{json.dumps(plan['clippy'][name], sort_keys=True, separators=(',', ':'))}{comma}")
    lines.append("},")
    lines.append('"graphs":{')
    graphs = sorted(plan["graphs"])
    for gi, graph in enumerate(graphs):
        g = plan["graphs"][graph]
        lines.append(f'{json.dumps(graph)}:{{"roots":{json.dumps(g["roots"], separators=(",", ":"))},"units":{{')
        keys = sorted(g["units"])
        for i, key in enumerate(keys):
            comma = "," if i + 1 < len(keys) else ""
            lines.append(f"{json.dumps(key)}:{json.dumps(g['units'][key], sort_keys=True, separators=(',', ':'))}{comma}")
        lines.append("}}" + ("," if gi + 1 < len(graphs) else ""))
    lines.append("}")
    lines.append("}")
    out.write("\n".join(lines) + "\n")


def cmd_plan(args):
    import tomllib

    with open(args.manifest, "rb") as f:
        ROOTS.update(profile_roots(tomllib.load(f)))
    metadata = load(args.metadata)
    packages = packages_of(metadata)
    graphs = {}
    for spec in args.graph:
        name, path = spec.split("=", 1)
        graphs[name] = load(path)
    for need in ("release", "loop", "test", "check"):
        if need not in graphs:
            die(f"no {need} graph given")

    profiles = {}
    plan_graphs = {}
    built = {}
    for name in ("release", "loop", "test"):
        g = Graph(name, graphs[name], packages, metadata["workspace_root"])
        built[name] = g
        lto = lto_map(g.units, g.roots)
        units = {}
        for index, key in g.keys.items():
            unit = g.units[index]
            # NO DOCTEST UNITS: none of the workspace's doctests runs (they
            # are `text`, `json` or `ignore` blocks), and `no-runnable-doctests`
            # holds that.
            if unit["mode"] == "doctest":
                continue
            entry = g.entry(index, lto.get(index, "both"))
            profiles[entry["profile"]] = profile_of(unit["profile"])
            units[key] = entry
        roots = sorted({g.key(r) for r in g.roots if g.units[r]["mode"] != "doctest"})
        # A doctest unit can be the only dependent of nothing else; drop the
        # references a removed doctest left in no unit (doctests depend, but
        # nothing depends on them).
        plan_graphs[name] = {"roots": roots, "units": units}

    check = Graph("check", graphs["check"], packages, metadata["workspace_root"], as_build=True)
    test_keys = set(plan_graphs["test"]["units"])
    clippy = clippy_targets(check, built["test"], test_keys)

    # THE CHECK GRAPH'S FEATURES ARE THE TEST GRAPH'S, crate for crate: clippy
    # reads the libraries the tests built, and a crate unified differently
    # in the two would be linted against code it is not compiled with.
    test_features = {}
    for unit in plan_graphs["test"]["units"].values():
        if unit["workspace"] and unit["mode"] == "build" and "lib" in unit["kind"]:
            test_features[unit["pkg"]] = unit["features"]
    for crate, targets in clippy.items():
        for t in targets:
            if "custom-build" in t["kind"]:
                continue
            if crate in test_features and t["features"] != test_features[crate]:
                die(f"clippy: {crate} is checked with {t['features']} and tested with {test_features[crate]}")
    # AND THE PRODUCT'S SIDE OF WHAT THE TESTS UNIFY DIFFERENTLY: added after
    # the check above, which they are the one exception to (`product_twins`).
    clippy = product_twins(clippy, plan_graphs["loop"])

    pkgs = {}
    for pkg in packages.values():
        if pkg["workspace"]:
            pkgs[pkg["name"]] = {
                "version": pkg["version"],
                "dir": pkg["dir"],
                "links": pkg["links"],
                "features": pkg["features"],
            }
    rustc = read(args.rustc)
    release = re.search(r"^release: (\S+)", rustc, re.M)
    host = re.search(r"^host: (\S+)", rustc, re.M)
    # THE `links` OF EVERY PACKAGE THAT HAS ONE: a build script below ours
    # hands its metadata up as `DEP_<links>_<KEY>`, and the unit that runs
    # ours names the variables by it.
    links = {f"{p['name']}@{p['version']}": p["links"] for p in packages.values() if p["links"]}
    plan = {
        "format": FORMAT,
        "inputs": args.inputs,
        "links": links,
        "rustc": release.group(1) if release else rustc.strip().splitlines()[0],
        "host": host.group(1) if host else None,
        "features": sorted(f for f in args.features.split(",") if f),
        "packages": pkgs,
        "profiles": profiles,
        "graphs": plan_graphs,
        "clippy": clippy,
    }
    write_plan(plan, sys.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan", help="normalise cargo's unit graphs into the plan")
    p.add_argument("--metadata", required=True)
    p.add_argument("--manifest", required=True, help="the workspace's root Cargo.toml, for its profiles")
    p.add_argument("--graph", action="append", default=[], help="NAME=FILE")
    p.add_argument("--features", default="")
    p.add_argument("--rustc", required=True, help="the output of `rustc -vV`")
    p.add_argument("--inputs", required=True, help="the hash of what the plan was made from")
    args = parser.parse_args()
    {"plan": cmd_plan}[args.cmd](args)


if __name__ == "__main__":
    main()
