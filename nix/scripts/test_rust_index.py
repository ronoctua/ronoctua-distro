"""The layer's index, proved on a layer small enough to read.
`python3 -m unittest nix/scripts/test_rust_index.py` — and the
`rust-build-scripts-pass-their-tests` check runs the same file in a build.

An index names the layer's units by the plan's keys, and refuses a layer whose
units are not the plan's: a unit missing or extra, or a library compiled for
LTO otherwise than the plan says. A build script's `links` metadata that names
the layer's target directory is written relative to the layer, and one that
names a path of the layer's sandbox no unit can see is marked so."""

import importlib.util
import json
import os
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("rust_index", os.path.join(HERE, "rust-index.py"))
ri = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ri)

KEY = "dep@1.0.0/lib:dep/build/o3/aaa"
RUN = "dep@1.0.0/custom-build:build-script-build/run-custom-build/o3/ccc"


def write(path, text):
    with open(path, "w") as f:
        f.write(text)


class Index(unittest.TestCase):
    def make(self, messages, extra_plan_unit=False, lto="bitcode", flags="-C linker-plugin-lto", metadata=None):
        d = tempfile.mkdtemp()
        raw = os.path.join(d, "raw")
        os.makedirs(os.path.join(raw, "cargo"))
        os.makedirs(os.path.join(raw, "deps"))
        os.makedirs(os.path.join(raw, "native", "release", "build", "dep-1", "out", "include"))
        write(os.path.join(raw, "cargo", "target-dir"), "/build/t\n")
        write(os.path.join(raw, "cargo", "build-top"), "/build\n")
        write(os.path.join(raw, "cargo", "runs"), "release\n")
        with open(os.path.join(raw, "cargo", "release.json"), "w") as f:
            for m in messages:
                f.write(json.dumps(m) + "\n")
        write(
            os.path.join(raw, "cargo", "release.log"),
            f"     Running `rustc --crate-name dep --edition=2021 src/lib.rs -C opt-level=3 {flags} "
            "-C extra-filename=-abc "
            "-L native=/build/t/release/build/dep-1/out -L native=/nix/store/x-sys/lib "
            "-L dependency=/build/t/release/deps`\n"
        )
        reported = []
        if metadata is not None:
            reported.append({"graph": "release", "package_id": "registry#dep@1.0.0", "out_dir": "/build/t/x",
                             "metadata": metadata})
        write(os.path.join(raw, "cargo", "scripts.json"), json.dumps(reported))
        units = {
            KEY: {"workspace": False, "mode": "build", "pkg": "dep", "version": "1.0.0", "kind": ["lib"],
                  "target": "dep", "opt": "3", "features": [], "lto": lto},
        }
        if metadata is not None:
            units[RUN] = {"workspace": False, "mode": "run-custom-build", "pkg": "dep", "version": "1.0.0",
                          "kind": ["custom-build"], "target": "build-script-build", "opt": "3", "features": [],
                          "lto": "object"}
        if extra_plan_unit:
            units["host@1.0.0/lib:host/build/o0/bbb"] = {"workspace": False, "mode": "build", "pkg": "host",
                                                         "version": "1.0.0", "kind": ["lib"], "target": "host",
                                                         "opt": "0", "features": [], "lto": "object"}
        plan = os.path.join(d, "plan.json")
        write(plan, json.dumps({"graphs": {"release": {"units": units}}, "links": {"dep@1.0.0": "dep"}}))
        return type("A", (), {"third_party": plan, "raw": raw, "out": os.path.join(d, "out")})

    ARTIFACT = {"reason": "compiler-artifact", "package_id": "registry#dep@1.0.0",
                "target": {"kind": ["lib"], "name": "dep"}, "profile": {"opt_level": "3", "test": False},
                "features": [], "filenames": ["/build/t/release/deps/libdep-abc.rlib", "/build/t/release/deps/libdep-abc.rmeta"]}
    EXECUTED = {"reason": "build-script-executed", "package_id": "registry#dep@1.0.0", "linked_paths": [],
                "out_dir": "/build/t/release/build/dep-1/out"}

    def index_of(self, args):
        ri.cmd_index(args)
        return ri.load(os.path.join(args.out, "index.json"))

    def test_the_layer_is_named_by_the_plan(self):
        args = self.make([self.ARTIFACT, {"reason": "build-finished", "success": True}])
        entry = self.index_of(args)["units"][KEY]
        self.assertEqual(entry["files"], ["deps/libdep-abc.rlib"])
        self.assertEqual(entry["native"], [
            {"kind": "native", "layer": "native/release/build/dep-1/out"},
            {"kind": "native", "path": "/nix/store/x-sys/lib"},
        ])

    def test_a_unit_cargo_did_not_build_is_refused(self):
        args = self.make([self.ARTIFACT], extra_plan_unit=True)
        with self.assertRaises(SystemExit):
            ri.cmd_index(args)

    def test_a_unit_the_plan_does_not_have_is_refused(self):
        other = dict(self.ARTIFACT, features=["more"])
        args = self.make([self.ARTIFACT, other])
        with self.assertRaises(SystemExit):
            ri.cmd_index(args)

    def test_a_library_compiled_for_another_lto_is_refused(self):
        # The plan wants bitcode for a thin-LTO program; cargo compiled
        # objects only.
        args = self.make([self.ARTIFACT], flags="-C embed-bitcode=no")
        with self.assertRaises(SystemExit):
            ri.cmd_index(args)
        args = self.make([self.ARTIFACT], lto="object", flags="-C embed-bitcode=no")
        self.assertIn(KEY, self.index_of(args)["units"])

    def test_a_library_with_no_command_line_is_refused(self):
        args = self.make([dict(self.ARTIFACT, filenames=["/build/t/release/deps/libdep-fff.rlib"])])
        with self.assertRaises(SystemExit):
            ri.cmd_index(args)

    def test_links_metadata_is_written_relative_to_the_layer(self):
        metadata = {"include": "/build/t/release/build/dep-1/out/include", "root": "/build/t/release/build/dep-1/out",
                    "conf": "A,B", "store": "/nix/store/x-sys/build/include"}
        args = self.make([self.ARTIFACT, self.EXECUTED], metadata=metadata)
        run = self.index_of(args)["runs"][RUN]
        self.assertEqual(run["links"], "dep")
        self.assertEqual(run["metadata"], {
            "include": "@LAYER@/native/release/build/dep-1/out/include",
            "root": "@LAYER@/native/release/build/dep-1/out",
            "conf": "A,B",
            "store": "/nix/store/x-sys/build/include",
        })
        self.assertEqual(run["unreachable"], [])

    def test_links_metadata_no_unit_can_see_is_marked(self):
        metadata = {"src": "/build/source/include", "gone": "/build/t/release/build/dep-1/never"}
        args = self.make([self.ARTIFACT, self.EXECUTED], metadata=metadata)
        run = self.index_of(args)["runs"][RUN]
        self.assertEqual(run["unreachable"], ["gone", "src"])


class Lto(unittest.TestCase):
    def test_cargo_s_flags_in_the_plan_s_words(self):
        self.assertEqual(ri.lto_of(["opt-level=3", "linker-plugin-lto"]), "bitcode")
        self.assertEqual(ri.lto_of(["embed-bitcode=no"]), "object")
        self.assertEqual(ri.lto_of(["opt-level=3"]), "both")
        self.assertEqual(ri.lto_of(["lto=thin"]), "run:thin")
        self.assertEqual(ri.lto_of(["lto"]), "run:")
        self.assertEqual(ri.lto_of(["lto=off", "embed-bitcode=no"]), "off")


if __name__ == "__main__":
    unittest.main()
