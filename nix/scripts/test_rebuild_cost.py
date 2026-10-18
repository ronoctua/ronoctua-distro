"""`rebuild-cost`'s arithmetic, on indexes small enough to read.
`python3 -m unittest nix/scripts/test_rebuild_cost.py` — and the
`rust-build-scripts-pass-their-tests` check runs the same file in a build.

What moved is every entry whose derivation changed (and every new one), the
time is what the old builds measured (an unmeasured entry is said, never
free), and the critical path is the longest chain of moved entries."""

import importlib.util
import os
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("rebuild_cost", os.path.join(HERE, "rebuild-cost.py"))
rc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rc)


def entry(drv, kind="libs", flavour="release", crate="c", name=None, deps=(), metrics=None):
    return {"drv": drv, "kind": kind, "flavour": flavour, "crate": crate, "name": name or crate,
            "deps": list(deps), "metrics": metrics, "out": drv + "-out"}


class Moves(unittest.TestCase):
    def test_a_changed_or_new_entry_moved_and_a_removed_one_is_gone(self):
        before = {"a": entry("1"), "b": entry("2"), "old": entry("9")}
        after = {"a": entry("1"), "b": entry("3"), "new": entry("4")}
        self.assertEqual(rc.moved(before, after), ["b", "new"])
        self.assertEqual(rc.gone(before, after), ["old"])

    def test_nothing_moved_is_nothing(self):
        same = {"a": entry("1")}
        r = rc.report(same, same)
        self.assertEqual(r["moved"], [])
        self.assertEqual(r["seconds"], 0)
        self.assertEqual(r["criticalPath"], {"seconds": 0, "chain": []})


class Time(unittest.TestCase):
    def measured(self, d, name, seconds):
        path = os.path.join(d, name)
        os.makedirs(path)
        with open(os.path.join(path, "seconds"), "w") as f:
            f.write(f"{seconds}\n")
        return path

    def test_the_old_builds_measurements_are_summed_and_the_unmeasured_said(self):
        with tempfile.TemporaryDirectory() as d:
            before = {
                "release/libs/x": entry("1", metrics=self.measured(d, "x", 2.5)),
                "release/bins/p": entry("2", kind="bins", name="p", deps=["release/libs/x"],
                                        metrics=self.measured(d, "p", 40)),
                "test/testRuns/x": entry("3", kind="testRuns", flavour="test", crate="x",
                                         metrics=os.path.join(d, "collected")),
            }
            after = {k: dict(v, drv=v["drv"] + "'") for k, v in before.items()}
            r = rc.report(before, after)
            self.assertEqual(r["seconds"], 42.5)
            self.assertEqual(r["measured"], 2)
            self.assertEqual(r["unmeasured"], ["test/testRuns/x"])
            self.assertEqual(r["relinked"], ["p"])
            self.assertEqual(r["rerun"], ["x"])
            self.assertEqual(r["byFlavour"], {"release": {"libs": 1, "bins": 1}, "test": {"testRuns": 1}})

    def test_the_edited_builds_time_stands_in_and_is_said(self):
        with tempfile.TemporaryDirectory() as d:
            before = {"release/libs/x": entry("1", metrics=os.path.join(d, "never"))}
            after = {"release/libs/x": entry("2", metrics=self.measured(d, "x", 7))}
            r = rc.report(before, after)
            self.assertEqual(r["seconds"], 7)
            self.assertEqual(r["measuredAfter"], ["release/libs/x"])
            self.assertIn("by the edited build's own time", rc.text(r, "probe"))

    def test_an_unmeasured_chain_is_not_given_a_time(self):
        before = {"a": entry("1"), "b": entry("2", deps=["a"])}
        after = {"a": entry("1'"), "b": entry("2'", deps=["a"])}
        self.assertIn("critical path: not measured", rc.text(rc.report(before, after), "probe"))

    def test_the_critical_path_is_the_longest_chain_of_moved_entries(self):
        after = {
            "a": entry("1", deps=[]),
            "b": entry("2", deps=["a"]),
            "c": entry("3", deps=["b", "a"]),
            "d": entry("4", deps=["a"]),
            "still": entry("5"),
        }
        seconds = {"a": 1.0, "b": 10.0, "c": 2.0, "d": 30.0}
        self.assertEqual(rc.critical_path(["a", "b", "c", "d"], after, seconds), (31.0, ["a", "d"]))
        # A dependency that did not move is not waited on.
        self.assertEqual(rc.critical_path(["b", "c"], after, seconds), (12.0, ["b", "c"]))

    def test_an_unmeasured_link_weighs_nothing(self):
        after = {"a": entry("1"), "b": entry("2", deps=["a"])}
        self.assertEqual(rc.critical_path(["a", "b"], after, {"a": None, "b": 3.0}), (3.0, ["a", "b"]))

    def test_a_cycle_is_refused(self):
        after = {"a": entry("1", deps=["b"]), "b": entry("2", deps=["a"])}
        with self.assertRaises(SystemExit):
            rc.critical_path(["a", "b"], after, {})


class Words(unittest.TestCase):
    def test_a_layer_that_moves_is_said(self):
        before = {"layer/release": entry("1", kind="layer", crate=None, name="release")}
        after = {"layer/release": entry("2", kind="layer", crate=None, name="release")}
        r = rc.report(before, after)
        self.assertEqual(r["layers"], ["layer/release"])
        self.assertIn("a third-party LAYER moves", rc.text(r, "probe"))

    def test_a_nix_string_cannot_end_early(self):
        self.assertEqual(rc.nix_string('a"b${c}\\'), '"a\\"b\\${c}\\\\"')


if __name__ == "__main__":
    unittest.main()
