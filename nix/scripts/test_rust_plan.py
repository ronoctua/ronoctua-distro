"""The planner, proved on graphs small enough to read.
`python3 -m unittest nix/scripts/test_rust_plan.py` — and the
`rust-build-scripts-pass-their-tests` check runs the same file in a build.

What is asserted is what the units LEAN ON: the LTO each unit gets is
cargo's (`lto.rs`), a key changes exactly when a unit's build does, and
clippy's check graph names the test graph's libraries. (The layer's index:
`test_rust_index.py`.)"""

import importlib.util
import io
import json
import os
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("rust_plan", os.path.join(HERE, "rust-plan.py"))
rp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rp)

RELEASE = {"name": "release", "opt_level": "3", "lto": "thin", "codegen_units": 1, "debuginfo": 0,
           "debug_assertions": False, "overflow_checks": False, "panic": "unwind", "rpath": False,
           "incremental": False, "split_debuginfo": None, "codegen_backend": None,
           "strip": {"resolved": {"Named": "debuginfo"}}}
HOST = dict(RELEASE, opt_level="0", codegen_units=None)
LOOP = dict(RELEASE, name="loop", opt_level="2", lto="false", codegen_units=16)


def unit(pkg, kind, name, mode="build", profile=RELEASE, features=(), deps=(), types=None, test=False):
    return {
        "pkg_id": pkg,
        "target": {"kind": [kind], "crate_types": types or ([kind] if kind != "custom-build" else ["bin"]),
                   "name": name, "src_path": f"/w/{name}/src/lib.rs", "edition": "2024", "test": test},
        "profile": dict(profile),
        "mode": mode,
        "features": list(features),
        "dependencies": [{"index": i, "extern_crate_name": e} for i, e in deps],
    }


PACKAGES = {
    "path+file:///w/app#0.1.0": {"name": "app", "version": "0.1.0", "workspace": True, "dir_abs": "/w/app", "dir": "app"},
    "path+file:///w/lib#0.1.0": {"name": "lib", "version": "0.1.0", "workspace": True, "dir_abs": "/w/lib", "dir": "lib"},
    "registry#dep@1.0.0": {"name": "dep", "version": "1.0.0", "workspace": False, "dir_abs": "/v/dep", "dir": None},
    "registry#host@1.0.0": {"name": "host", "version": "1.0.0", "workspace": False, "dir_abs": "/v/host", "dir": None},
}
for p in PACKAGES.values():
    p.setdefault("links", None)
    p.setdefault("features", [])


def release_graph(dep_features=()):
    # 0 dep (third party), 1 lib, 2 the lib's build script, 3 its run, 4 a
    # host-only library the script uses, 5 the program.
    units = [
        unit("registry#dep@1.0.0", "lib", "dep", features=dep_features),
        unit("path+file:///w/lib#0.1.0", "lib", "lib", deps=[(0, "dep"), (3, "build_script_build")]),
        unit("path+file:///w/lib#0.1.0", "custom-build", "build-script-build", profile=HOST, deps=[(4, "host")]),
        unit("path+file:///w/lib#0.1.0", "custom-build", "build-script-build", mode="run-custom-build", deps=[(2, "build_script_build")]),
        unit("registry#host@1.0.0", "lib", "host", profile=HOST),
        unit("path+file:///w/app#0.1.0", "bin", "app", deps=[(1, "lib"), (0, "dep")]),
    ]
    for u in units[:5]:
        u["target"]["src_path"] = u["target"]["src_path"].replace("/w/app", "/w/lib")
    units[5]["target"]["src_path"] = "/w/app/src/main.rs"
    units[0]["target"]["src_path"] = "/v/dep/src/lib.rs"
    units[4]["target"]["src_path"] = "/v/host/src/lib.rs"
    units[2]["target"]["src_path"] = "/w/lib/build.rs"
    units[3]["target"]["src_path"] = "/w/lib/build.rs"
    return {"version": 1, "units": units, "roots": [1, 5]}


class Lto(unittest.TestCase):
    def test_release_is_cargo_s(self):
        g = release_graph()
        lto = rp.lto_map(g["units"], g["roots"])
        # The program runs thin LTO; every rlib below it carries bitcode only;
        # what runs on the host carries objects only.
        self.assertEqual(lto[5], "run:thin")
        self.assertEqual(lto[1], "bitcode")
        self.assertEqual(lto[0], "bitcode")
        self.assertEqual(lto[2], "object")
        self.assertEqual(lto[4], "object")

    def test_loop_carries_objects(self):
        g = release_graph()
        for u in g["units"]:
            u["profile"] = dict(LOOP)
        lto = rp.lto_map(g["units"], g["roots"])
        self.assertTrue(all(v == "object" for v in lto.values()), lto)

    def test_one_unit_needed_both_ways_carries_both(self):
        g = release_graph()
        # The host-only library becomes a dependency of the program too: the
        # same unit wanted as objects (by the script) and as bitcode.
        g["units"][5]["dependencies"].append({"index": 4, "extern_crate_name": "host"})
        lto = rp.lto_map(g["units"], g["roots"])
        self.assertEqual(lto[4], "both")


class Keys(unittest.TestCase):
    def graph(self, data, as_build=False):
        return rp.Graph("release", data, PACKAGES, "/w", as_build=as_build)

    def test_a_dependency_built_differently_renames_everything_above(self):
        a = self.graph(release_graph())
        b = self.graph(release_graph(dep_features=["extra"]))
        self.assertNotEqual(a.key(0), b.key(0))
        self.assertNotEqual(a.key(1), b.key(1), "the library above the dependency kept its name")
        self.assertNotEqual(a.key(5), b.key(5), "the program above the dependency kept its name")
        # The build script does not depend on it, and keeps its key.
        self.assertEqual(a.key(2), b.key(2))

    def test_the_same_build_has_the_same_key_in_any_graph(self):
        a = self.graph(release_graph())
        again = self.graph(json.loads(json.dumps(release_graph())))
        self.assertEqual([a.key(i) for i in range(6)], [again.key(i) for i in range(6)])

    def test_a_check_is_keyed_as_the_build_it_would_be(self):
        build = self.graph(release_graph())
        g = release_graph()
        for i in (0, 1):
            g["units"][i]["mode"] = "check"
        check = self.graph(g, as_build=True)
        self.assertEqual(build.key(1), check.key(1))

    def test_the_key_says_what_the_unit_is(self):
        key = self.graph(release_graph()).key(1)
        self.assertTrue(key.startswith("lib@0.1.0/lib:lib/build/o3/"), key)

    def test_two_units_with_one_key_are_refused(self):
        g = release_graph()
        g["units"].append(json.loads(json.dumps(g["units"][0])))
        with self.assertRaises(SystemExit):
            self.graph(g)


class Clippy(unittest.TestCase):
    def test_the_test_check_is_the_one_with_the_dev_dependencies(self):
        # The test graph: the library, and its harness with a dev-dependency.
        test = {"version": 1, "roots": [2], "units": [
            unit("registry#dep@1.0.0", "lib", "dep", profile=LOOP),
            unit("registry#host@1.0.0", "lib", "host", profile=LOOP),
            unit("path+file:///w/lib#0.1.0", "lib", "lib", mode="test", profile=LOOP, deps=[(0, "dep"), (1, "host")]),
            unit("path+file:///w/lib#0.1.0", "lib", "lib", profile=LOOP, deps=[(0, "dep")]),
        ]}
        check = {"version": 1, "roots": [2, 3], "units": [
            unit("registry#dep@1.0.0", "lib", "dep", mode="check", profile=LOOP),
            unit("registry#host@1.0.0", "lib", "host", mode="check", profile=LOOP),
            unit("path+file:///w/lib#0.1.0", "lib", "lib", mode="check", profile=LOOP, deps=[(0, "dep")]),
            unit("path+file:///w/lib#0.1.0", "lib", "lib", mode="check", profile=LOOP, deps=[(0, "dep"), (1, "host")]),
        ]}
        tg = rp.Graph("test", test, PACKAGES, "/w")
        cg = rp.Graph("check", check, PACKAGES, "/w", as_build=True)
        keys = set(tg.keys.values())
        out = rp.clippy_targets(cg, tg, keys)
        targets = out["lib"]
        as_test = [t for t in targets if t["test"]]
        plain = [t for t in targets if not t["test"]]
        self.assertEqual(len(as_test), 1)
        self.assertEqual(len(plain), 1)
        self.assertEqual({d["extern"] for d in as_test[0]["deps"]}, {"dep", "host"})
        self.assertEqual({d["extern"] for d in plain[0]["deps"]}, {"dep"})
        # Every dependency is a key the test graph builds.
        for t in targets:
            for d in t["deps"]:
                self.assertIn(d["key"], keys)

    def test_a_dependency_the_tests_do_not_build_is_refused(self):
        test = {"version": 1, "roots": [0], "units": [unit("path+file:///w/lib#0.1.0", "lib", "lib", mode="test", profile=LOOP)]}
        check = {"version": 1, "roots": [1], "units": [
            unit("registry#dep@1.0.0", "lib", "dep", mode="check", profile=LOOP),
            unit("path+file:///w/lib#0.1.0", "lib", "lib", mode="check", profile=LOOP, deps=[(0, "dep")]),
        ]}
        tg = rp.Graph("test", test, PACKAGES, "/w")
        cg = rp.Graph("check", check, PACKAGES, "/w", as_build=True)
        with self.assertRaises(SystemExit):
            rp.clippy_targets(cg, tg, set(tg.keys.values()))


class ProductTwins(unittest.TestCase):
    """A library the tests unify with `test-support` is linted as the product
    has it too, against the same test-graph libraries."""

    def clippy(self, features):
        return {"lib": [
            {"target": "lib", "kind": ["lib"], "crate_types": ["lib"], "src": "src/lib.rs", "edition": "2024",
             "test": False, "features": list(features),
             "deps": [{"extern": "dep", "key": "dep-test"}, {"extern": "helper", "key": "helper-test"},
                      {"extern": "build_script_build", "key": "run-test", "run": True}]},
            {"target": "lib", "kind": ["lib"], "crate_types": ["lib"], "src": "src/lib.rs", "edition": "2024",
             "test": True, "features": list(features), "deps": [{"extern": "dep", "key": "dep-test"}]},
        ]}

    def product(self, features=(), externs=("dep",)):
        units = {
            "lib-loop": {"pkg": "lib", "workspace": True, "mode": "build", "kind": ["lib"], "target": "lib",
                         "features": list(features),
                         "deps": [{"extern": e, "key": f"{e}-loop"} for e in externs]
                         + [{"extern": "build_script_build", "key": "run-loop"}]},
            "run-loop": {"pkg": "lib", "workspace": True, "mode": "run-custom-build", "kind": ["custom-build"],
                         "target": "build-script-build", "features": list(features), "deps": []},
        }
        for e in externs:
            units[f"{e}-loop"] = {"pkg": e, "workspace": False, "mode": "build", "kind": ["lib"], "target": e,
                                  "features": [], "deps": []}
        return {"roots": ["lib-loop"], "units": units}

    def test_the_product_side_is_linted_too(self):
        out = rp.product_twins(self.clippy(["test-support"]), self.product())
        twins = [t for t in out["lib"] if t.get("product")]
        self.assertEqual(len(twins), 1)
        twin = twins[0]
        self.assertFalse(twin["test"])
        self.assertEqual(twin["features"], [])
        # The same test-graph libraries, less what only the tests' features
        # bring; the build script's run stays.
        self.assertEqual([(d["extern"], d["key"]) for d in twin["deps"]],
                         [("dep", "dep-test"), ("build_script_build", "run-test")])
        # The plain check with the tests' features is still there.
        plain = [t for t in out["lib"] if not t["test"] and not t.get("product")]
        self.assertEqual(plain[0]["features"], ["test-support"])

    def test_a_crate_unified_alike_has_no_twin(self):
        out = rp.product_twins(self.clippy(["default"]), self.product(features=["default"]))
        self.assertFalse(any(t.get("product") for t in out["lib"]))
        self.assertEqual(len(out["lib"]), 2)

    def test_a_product_dependency_the_check_does_not_name_is_refused(self):
        with self.assertRaises(SystemExit):
            rp.product_twins(self.clippy(["test-support"]), self.product(externs=("dep", "elsewhere")))


class Plan(unittest.TestCase):
    def test_the_file_is_json_with_one_unit_a_line(self):
        g = rp.Graph("release", release_graph(), PACKAGES, "/w")
        lto = rp.lto_map(g.units, g.roots)
        units = {k: g.entry(i, lto.get(i, "both")) for i, k in g.keys.items()}
        plan = {"format": 1, "packages": {}, "profiles": {}, "clippy": {}, "graphs": {"release": {"roots": [], "units": units}}}
        out = io.StringIO()
        rp.write_plan(plan, out)
        text = out.getvalue()
        self.assertEqual(json.loads(text)["graphs"]["release"]["units"], json.loads(json.dumps(units)))
        unit_lines = [l for l in text.splitlines() if l.startswith('"') and "@" in l.split(":", 1)[0]]
        self.assertEqual(len(unit_lines), len(units))

    def test_relative_paths_only(self):
        g = rp.Graph("release", release_graph(), PACKAGES, "/w")
        self.assertEqual(g.entry(5, "run:thin")["src"], "src/main.rs")
        self.assertEqual(g.entry(0, "bitcode")["src"], "src/lib.rs")

    def test_profiles_keep_their_root(self):
        rp.ROOTS.update(rp.profile_roots({"profile": {"loop": {"inherits": "release"}}}))
        self.assertEqual(rp.profile_of(LOOP)["root"], "release")
        self.assertNotIn("name", rp.profile_of(LOOP))


if __name__ == "__main__":
    unittest.main()
