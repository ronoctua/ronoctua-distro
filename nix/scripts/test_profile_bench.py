#!/usr/bin/env python3
"""The tests of `profile-bench.py`: the reading of `.#bench`, the rule, the
cost of an edit, the checks that a variant reached its build, and a whole
report over a directory a run could have left."""

import importlib.util
import json
import os
import subprocess
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("profile_bench", os.path.join(HERE, "profile-bench.py"))
pb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pb)

FRAMES_OK = """\
== section 5.2's targets ==
  OK  first-frame     n=  6  p50=    61.2  p99=    69.0  max=    69.0 ms   (target 200.0 ms)
  OK  which-key       n= 15  p50=     4.3  p99=     6.7  max=     6.7 ms   (target 16.7 ms)
"""


def bench_log(layout_p50="0.0035", which_key_p99="6.7", index_p50="1.888", frames=True, verdict="OK"):
    text = f"""\
ronoctua-bench — section 5.2

== the frame's CPU work ==
layout  200 columns: p50={layout_p50} ms  p99=0.0035 ms
layout   80 columns: p50=0.0043 ms  p99=0.0048 ms

worst p99: 0.0048 ms   ceiling: 1.6 ms (10% of the 16 ms frame)
OK: the layout fits with room to spare

== text shaping ==
  upstream + Monospace                     436.4 ms
  upstream + the exact name                120.4 ms
  ours     + Monospace                       0.3 ms
  ours     + the exact name  <- production       0.1 ms

production: 0.1 ms   ceiling 300 ms

== component instantiation ==
with     memory_init_cow: p50=0.007 ms  p99=0.010 ms  (n=200)
without  memory_init_cow: p50=0.017 ms  p99=0.021 ms

== the super index: a million names, keystroke by keystroke ==
indexed 1000000 names (100000 with a body) in 4.2 s; the folder holds 170 MiB
names (/f)            : 7159 queries  p50={index_p50} ms  p95=2.603 ms  p99=3.175 ms  max=41.483 ms  (one letter: p99=3.927 ms over 400)  hits=130629
"""
    if frames:
        text += f"""\
== section 5.2's targets ==
  OK  first-frame     n=  6  p50=    61.2  p99=    69.0  max=    69.0 ms   (target 200.0 ms)
  {verdict}  which-key       n= 15  p50=     4.3  p99=  {which_key_p99:>6}  max=     6.7 ms   (target 16.7 ms)
"""
    return text


class Reading(unittest.TestCase):
    def test_every_metric_is_read(self):
        got = pb.parse_log(bench_log())
        self.assertEqual(got["layout-200col-p50-ms"], "0.0035")
        self.assertEqual(got["layout-80col-p99-ms"], "0.0048")
        self.assertEqual(got["shaping-upstream-Monospace-ms"], "436.4")
        self.assertEqual(got["shaping-ours-the-exact-name-ms"], "0.1")
        self.assertEqual(got["instance-without-cow-p99-ms"], "0.021")
        self.assertEqual(got["index-build-s"], "4.2")
        self.assertEqual(got["index-size-mib"], "170")
        self.assertEqual(got["index-names-f-p95-ms"], "2.603")
        self.assertEqual(got["index-names-f-max-ms"], "41.483")
        # The first keystroke in fast access: a p99 of its own.
        self.assertEqual(got["index-names-f-one-letter-p99-ms"], "3.927")
        self.assertEqual(pb.judged("index-names-f-one-letter-p99-ms"), "p99")
        self.assertEqual(got["first-frame-p99-ms"], "69.0")
        self.assertEqual(got["which-key-p50-ms"], "4.3")
        self.assertEqual(len(got), 25)

    def test_every_index_line_is_read(self):
        lines = (
            "names (/f)            : 7159 queries  p50=1.891 ms  p95=2.651 ms  p99=3.285 ms  max=41.434 ms  "
            "(one letter: p99=3.678 ms over 400)  hits=130629\n"
            "names + contents      : 6254 queries  p50=4.163 ms  p95=7.599 ms  p99=10.564 ms  max=14.919 ms  "
            "(one letter: p99=12.808 ms over 400)  hits=216414\n"
            "names typed, default  : 7159 queries  p50=3.422 ms  p95=5.517 ms  p99=7.536 ms  max=41.659 ms  "
            "(one letter: p99=9.422 ms over 400)  hits=239345\n"
        )
        got = pb.parse_log(lines)
        self.assertEqual(got["index-names-contents-one-letter-p99-ms"], "12.808")
        self.assertEqual(got["index-names-typed-default-one-letter-p99-ms"], "9.422")
        self.assertEqual(got["index-names-typed-default-p99-ms"], "7.536")
        self.assertEqual(len(got), 15)
        self.assertEqual(pb.unread(lines), [])

    def test_a_quantile_no_pattern_reads_is_said(self):
        self.assertEqual(pb.unread(bench_log()), [])
        extra = bench_log() + "names by date        : 900 queries  p50=1.0 ms  p95=2.0 ms  p99=3.0 ms  max=4.0 ms\n"
        self.assertEqual(len(pb.unread(extra)), 1)
        # A known line with one quantile more than the pattern reads.
        more = "with     memory_init_cow: p50=0.007 ms  p99=0.010 ms  (cold: p99=0.020 ms)\n"
        self.assertEqual(pb.unread(more), [more.strip()])

    def test_a_failed_frame_target_is_still_read(self):
        got = pb.parse_log(bench_log(which_key_p99="17.0", verdict="NO"))
        self.assertEqual(got["which-key-p99-ms"], "17.0")

    def test_a_clean_round_is_ok(self):
        self.assertEqual(pb.classify(bench_log(), 0), "ok")

    def test_a_frame_past_its_target_is_a_budget(self):
        log = bench_log(which_key_p99="17.0", verdict="NO")
        self.assertEqual(pb.classify(log, 1), "budget")
        self.assertEqual(len(pb.budget_failures(log)), 1)

    def test_a_ceiling_of_the_cpu_half_is_a_budget(self):
        log = "layout  200 columns: p50=2.0 ms  p99=2.0 ms\n\nERROR: layout on its own goes past 10% of the frame.\n"
        self.assertEqual(pb.classify(log, 1), "budget")
        log = "Error: a p99 of 6.000 ms goes past section 5.2's ceiling of 5 ms.\n"
        self.assertEqual(pb.classify(log, 1), "budget")
        self.assertEqual(pb.classify("ERROR: the index missed the gate; fast-access must not be built on it.\n", 1), "budget")

    def test_a_round_the_harness_could_not_measure_is_broken(self):
        no_session = bench_log(frames=False) + "No graphical session: the frame targets were not measured.\n"
        self.assertEqual(pb.classify(no_session, 0), "broken")
        compositor = bench_log(frames=False) + "ERROR: the nested compositor did not come up.\n"
        self.assertEqual(pb.classify(compositor, 1), "broken")

    def test_a_program_of_the_variant_that_fails_is_a_failure(self):
        cpu = bench_log(frames=False).split("== the super index")[0]
        # The index bench refusing a query (not missing its gate)…
        self.assertEqual(pb.classify(cpu + "ERROR: the query refused: Busy\n", 1), "failed")
        # …a panic, a shell that did not start or logged nothing…
        self.assertEqual(pb.classify(cpu + "thread 'main' panicked at src/main.rs:3:5:\n", 101), "failed")
        self.assertEqual(pb.classify(bench_log() + "  startup 2 FAILED\n", 0), "failed")
        self.assertEqual(pb.classify(bench_log(frames=False) + "ERROR: no milestone in the log at all\n", 1), "failed")
        # …and a leader that opened fewer times than asked.
        short = bench_log() + "  NO  which-key: 3 openings measured, 15 asked for\n"
        self.assertEqual(pb.classify(short, 1), "failed")
        self.assertEqual(pb.failure_of(short, 1), "NO  which-key: 3 openings measured, 15 asked for")
        # A round that stopped with nothing saying why is no harness's.
        self.assertEqual(pb.classify(bench_log(frames=False), 1), "failed")
        self.assertEqual(pb.failure_of(bench_log(frames=False), 1), "exit 1")
        self.assertEqual(pb.classify(bench_log(frames=False), 0), "failed")
        # And a budget line is a budget, never also a failure.
        self.assertEqual(pb.program_failures("ERROR: the index missed the gate; fast-access must not be built on it.\n"), [])

    def test_the_round_exit_is_read_back(self):
        self.assertEqual(pb.exit_of("== round 1 x\n...\n== exit 0\n"), 0)
        self.assertEqual(pb.exit_of("== round 1 x\n...\n== exit 124\n"), 124)
        self.assertEqual(pb.exit_of("cut short"), 1)


class Rule(unittest.TestCase):
    def test_what_is_judged_and_how(self):
        self.assertEqual(pb.judged("layout-200col-p99-ms"), "p99")
        self.assertEqual(pb.judged("layout-200col-p50-ms"), "median")
        self.assertEqual(pb.judged("shaping-ours-Monospace-ms"), "median")
        self.assertEqual(pb.judged("index-build-s"), "median")
        self.assertIsNone(pb.judged("index-names-f-p95-ms"))
        self.assertIsNone(pb.judged("index-names-f-max-ms"))
        self.assertIsNone(pb.judged("index-size-mib"))

    def test_quantum_is_the_last_printed_digit(self):
        self.assertAlmostEqual(pb.quantum("0.0035"), 0.0001)
        self.assertAlmostEqual(pb.quantum("4.2"), 0.1)
        self.assertEqual(pb.quantum("170"), 1.0)

    def rows(self, ref, var, metric="index-names-f-p50-ms"):
        reference = [{metric: v} for v in ref]
        variant = [{metric: v} for v in var]
        return pb.judge(reference, variant)[0]

    def test_a_median_within_three_percent_passes(self):
        row = self.rows(["1.000"] * 5, ["1.030"] * 5)
        self.assertTrue(row["passes"])
        self.assertEqual(row["note"], "")

    def test_a_median_past_three_percent_fails(self):
        row = self.rows(["1.000"] * 5, ["1.040"] * 5)
        self.assertFalse(row["passes"])
        self.assertEqual(row["note"], "past the tolerance")

    def test_a_p99_gets_five_percent(self):
        row = self.rows(["1.000"] * 5, ["1.049"] * 5, metric="index-names-f-p99-ms")
        self.assertTrue(row["passes"])
        row = self.rows(["1.000"] * 5, ["1.060"] * 5, metric="index-names-f-p99-ms")
        self.assertFalse(row["passes"])

    def test_one_printed_digit_is_no_measured_difference(self):
        # 0.0034 / 0.0033 is 1.0303: past 3%, by less than the printed digit.
        row = self.rows(["0.0033"] * 5, ["0.0034"] * 5, metric="layout-200col-p50-ms")
        self.assertTrue(row["passes"])
        self.assertEqual(row["note"], "within the printed resolution")
        row = self.rows(["0.0033"] * 5, ["0.0035"] * 5, metric="layout-200col-p50-ms")
        self.assertFalse(row["passes"])

    def test_the_median_of_the_rounds_is_compared(self):
        # One slow round of five does not fail a variant; three do.
        row = self.rows(["1.000"] * 5, ["1.000", "1.000", "1.000", "1.000", "2.000"])
        self.assertTrue(row["passes"])
        row = self.rows(["1.000"] * 5, ["1.000", "1.000", "2.000", "2.000", "2.000"])
        self.assertFalse(row["passes"])

    def test_a_judged_metric_the_variant_lacks_fails(self):
        rows = pb.judge([{"index-build-s": "4.1"}] * 3, [{}] * 3)
        self.assertFalse(rows[0]["passes"])
        self.assertEqual(rows[0]["note"], "not measured")

    def test_a_metric_of_the_references_own_program_is_not_judged(self):
        reference = [{"instance-with-cow-p99-ms": "0.009", "which-key-p99-ms": "6.6"}] * 5
        variant = [{"instance-with-cow-p99-ms": "0.011", "which-key-p99-ms": "9.0"}] * 5
        rows = {r["metric"]: r for r in pb.judge(reference, variant, {"ronoctua-bench-instance"})}
        self.assertTrue(rows["instance-with-cow-p99-ms"]["passes"])
        self.assertEqual(rows["instance-with-cow-p99-ms"]["note"], "the reference's own program: not judged")
        # The program the variant did change is still judged.
        self.assertFalse(rows["which-key-p99-ms"]["passes"])
        # And without the shared program, the same numbers fail.
        rows = {r["metric"]: r for r in pb.judge(reference, variant)}
        self.assertFalse(rows["instance-with-cow-p99-ms"]["passes"])

    def test_which_program_measures_what(self):
        self.assertEqual(pb.program_of("layout-200col-p50-ms"), "ronoctua-bench-layout")
        self.assertEqual(pb.program_of("shaping-ours-Monospace-ms"), "ronoctua-bench-shaping")
        self.assertEqual(pb.program_of("instance-with-cow-p99-ms"), "ronoctua-bench-instance")
        self.assertEqual(pb.program_of("index-names-f-p50-ms"), "ronoctua-bench-index")
        self.assertEqual(pb.program_of("first-frame-p99-ms"), "ronoctua-shell")
        self.assertEqual(pb.program_of("which-key-p50-ms"), "ronoctua-shell")

    def test_shared_programs_are_the_same_unit(self):
        a = [
            {"name": "ronoctua-shell-0.1.0", "kind": "bin", "metrics": "/s/a-shell"},
            {"name": "ronoctua-bench-index-0.1.0", "kind": "bin", "metrics": "/s/index"},
            {"name": "ronoctua-tags-0.1.0", "kind": "bin", "metrics": "/s/tags"},
            {"name": "rust-lib-ronoctua-index-0.1.0", "kind": "lib", "metrics": "/s/lib"},
        ]
        e = [dict(u, metrics="/s/e-shell") if u["name"].startswith("ronoctua-shell-") else u for u in a]
        # The shell differs; the index bench is one unit; the tags program is
        # shared but measures nothing.
        self.assertEqual(pb.same_programs(a, e), {"ronoctua-bench-index"})

    def test_unjudged_metrics_never_fail(self):
        row = self.rows(["40.0"] * 5, ["80.0"] * 5, metric="index-names-f-max-ms")
        self.assertTrue(row["passes"])


class Cost(unittest.TestCase):
    UNITS = [
        {"name": "lib-a", "kind": "lib", "crate": "a", "deps": []},
        {"name": "lib-b", "kind": "lib", "crate": "b", "deps": ["lib-a"]},
        {"name": "lib-shell", "kind": "lib", "crate": "shell", "deps": ["lib-b"]},
        {"name": "lib-other", "kind": "lib", "crate": "other", "deps": []},
        {"name": "shell-bin", "kind": "bin", "crate": "shell", "deps": ["lib-shell", "lib-b", "lib-a"]},
        {"name": "a-bin", "kind": "bin", "crate": "a", "deps": ["lib-a"]},
        {"name": "other-bin", "kind": "bin", "crate": "other", "deps": ["lib-other"]},
    ]
    SECONDS = {"lib-a": 1, "lib-b": 2, "lib-shell": 50, "lib-other": 7, "shell-bin": 100, "a-bin": 300, "other-bin": 9}

    def test_an_edit_rebuilds_what_is_above_it(self):
        cost = pb.edit_cost(self.UNITS, self.SECONDS, "a")
        self.assertEqual(cost["units"], ["a-bin", "lib-a", "lib-b", "lib-shell", "shell-bin"])
        self.assertEqual(cost["programs"], ["a-bin", "shell-bin"])
        self.assertEqual(cost["sum"], 453)
        # a → a-bin (301) is longer than a → b → shell → shell-bin (153).
        self.assertEqual(cost["critical"], 301)

    def test_an_edit_at_the_top_rebuilds_the_top(self):
        cost = pb.edit_cost(self.UNITS, self.SECONDS, "shell")
        self.assertEqual(cost["units"], ["lib-shell", "shell-bin"])
        self.assertEqual(cost["critical"], 150)

    def test_a_crate_of_programs_only_rebuilds_its_programs(self):
        # The shell: `src/main.rs` and a second program, no library.
        units = self.UNITS + [
            {"name": "tool-bin", "kind": "bin", "crate": "tool", "deps": ["lib-b", "lib-a"]},
            {"name": "tool-png", "kind": "bin", "crate": "tool", "deps": ["lib-a"]},
        ]
        seconds = dict(self.SECONDS, **{"tool-bin": 360, "tool-png": 80})
        cost = pb.edit_cost(units, seconds, "tool")
        self.assertEqual(cost["units"], ["tool-bin", "tool-png"])
        self.assertEqual(cost["critical"], 360)
        self.assertEqual(cost["sum"], 440)
        # And an edit below it reaches it through its dependencies.
        self.assertIn("tool-bin", pb.edit_cost(units, seconds, "b")["units"])

    def test_a_crate_with_no_unit_is_refused(self):
        with self.assertRaises(ValueError):
            pb.edit_cost(self.UNITS, self.SECONDS, "nowhere")

    def test_the_cold_build(self):
        cold = pb.cold_build(self.UNITS, self.SECONDS, 400.0)
        self.assertEqual(cold["units-sum"], 469)
        self.assertEqual(cold["work"], 869)
        self.assertEqual(cold["units-critical"], 301)

    def test_a_cold_build_without_the_layer_is_refused(self):
        with self.assertRaises(ValueError):
            pb.cold_build(self.UNITS, self.SECONDS, None)

    def test_cargo_elapsed(self):
        self.assertEqual(pb.cargo_elapsed("x\n    Finished `release` profile [optimized] target(s) in 6m 20s\n"), 380)
        self.assertEqual(pb.cargo_elapsed("    Finished `release` profile [optimized] target(s) in 45.30s\n"), 45.3)
        self.assertEqual(pb.cargo_elapsed("    Finished `release` profile [optimized] target(s) in 75m 03s\n"), 4503)
        self.assertIsNone(pb.cargo_elapsed("error: could not compile\n"))
        # The last one wins: a layer that runs two cargo commands.
        two = "Finished `loop` profile in 1m 00s\nFinished `test` profile in 2m 00s\n"
        self.assertEqual(pb.cargo_elapsed(two), 120)


class Reached(unittest.TestCase):
    def plan(self, cgu, lto, lto_of_units, shell_cgu=None):
        units = {f"u{i}": {"pkg": "x", "profile": "p", "lto": x} for i, x in enumerate(lto_of_units)}
        units["h"] = {"pkg": "x", "profile": "host", "lto": "object"}
        units["shell"] = {"pkg": "ronoctua-shell", "profile": "s", "lto": lto_of_units[0]}
        return {
            "profiles": {
                "p": {"codegen_units": cgu, "lto": lto},
                "s": {"codegen_units": shell_cgu or cgu, "lto": lto},
                "host": {"codegen_units": None, "lto": lto},
            },
            "graphs": {"release": {"units": units}},
        }

    def test_a_plan_that_is_its_variant(self):
        self.assertEqual(pb.check_plan(self.plan(16, "false", ["object", "object"]), "false", 16), [])
        self.assertEqual(pb.check_plan(self.plan(1, "thin", ["bitcode", "run:thin"]), "thin", 1), [])

    def test_a_plan_that_is_another_variant(self):
        self.assertTrue(pb.check_plan(self.plan(1, "false", ["object"]), "false", 16))
        self.assertTrue(pb.check_plan(self.plan(16, "false", ["bitcode", "object"]), "false", 16))
        self.assertTrue(pb.check_plan(self.plan(16, "thin", ["bitcode", "object"]), "thin", 16))

    def test_a_per_package_override_is_held_to_its_package(self):
        override = {"ronoctua-shell": {"codegen-units": 16}}
        plan = self.plan(1, "thin", ["bitcode", "run:thin"], shell_cgu=16)
        self.assertEqual(pb.check_plan(plan, "thin", 1, override), [])
        # Without the override the shell's sixteen units are another variant…
        self.assertTrue(pb.check_plan(plan, "thin", 1))
        # …and an override the plan did not take is refused.
        self.assertTrue(pb.check_plan(self.plan(1, "thin", ["bitcode", "run:thin"]), "thin", 1, override))
        # So is one naming a crate the graph does not build.
        self.assertTrue(pb.check_plan(plan, "thin", 1, override | {"nowhere": {"codegen-units": 4}}))

    def test_a_layer_log_that_is_its_variant(self):
        log = "rustc --crate-name a -C opt-level=3 -C embed-bitcode=no -C codegen-units=16\n"
        self.assertEqual(pb.check_layer_log(log, "false", 16), [])
        log = "rustc --crate-name a -C opt-level=3 -C linker-plugin-lto -C codegen-units=1\n"
        self.assertEqual(pb.check_layer_log(log, "thin", 1), [])

    def test_a_layer_log_that_is_another_variant(self):
        log = "rustc --crate-name a -C opt-level=3 -C linker-plugin-lto -C codegen-units=1\n"
        self.assertEqual(len(pb.check_layer_log(log, "false", 16)), 2)
        mixed = log + "rustc --crate-name b -C codegen-units=16\n"
        self.assertTrue(pb.check_layer_log(mixed, "thin", 1))


class Decision(unittest.TestCase):
    def test_the_cheapest_acceptable_edit_wins(self):
        summary = {
            "A": {"acceptable": True, "edit": 600.0, "cold": 2000.0},
            "B": {"acceptable": False, "edit": 50.0, "cold": 900.0},
            "C": {"acceptable": True, "edit": 100.0, "cold": 1500.0},
            "D": {"acceptable": True, "edit": 300.0, "cold": 1400.0},
        }
        winner, ranking = pb.decide(summary)
        self.assertEqual(winner, "C")
        self.assertEqual(ranking, ["C", "D", "A"])

    def test_close_edits_are_decided_by_the_cold_build(self):
        summary = {
            "A": {"acceptable": True, "edit": 600.0, "cold": 2000.0},
            "C": {"acceptable": True, "edit": 100.0, "cold": 1500.0},
            "D": {"acceptable": True, "edit": 104.0, "cold": 1400.0},
        }
        self.assertEqual(pb.decide(summary)[0], "D")

    def test_nothing_acceptable_is_no_winner(self):
        self.assertEqual(pb.decide({"A": {"acceptable": False, "edit": 1.0, "cold": 1.0}}), (None, []))


class Builds(unittest.TestCase):
    """How a variant is built: as every machine builds (`nix/build-sizing.nix`),
    or at the two numbers given."""

    PATHS = {
        "layer": "/nix/store/l-layer.drv",
        "sized": "/nix/store/s-sized.drv",
        "sizedOut": "/nix/store/s-sized",
        "cost": "/nix/store/c-cost.drv",
        "runner": "/nix/store/r-runner.drv",
        "bundle": "/nix/store/b-bundle.drv",
    }
    PRODUCTS = ["/nix/store/c-cost.drv^*", "/nix/store/r-runner.drv^*", "/nix/store/b-bundle.drv^*"]

    def test_a_variant_is_built_as_every_machine_builds(self):
        builds = pb.variant_builds(self.PATHS, None, {"slots": 4, "jobs": 4, "cores": 1})
        self.assertEqual([name for name, _ in builds], ["layer", "sized", "units"])
        built = dict(builds)
        # The layer first and alone: one cargo, with every slot inside it.
        self.assertEqual(
            built["layer"],
            ["nix", "build", "--no-link", "--max-jobs", "1", "--option", "cores", "4", "/nix/store/l-layer.drv^*"],
        )
        self.assertEqual(built["sized"][3:8], ["--max-jobs", "4", "--option", "cores", "1"])
        # Every unit through the variant's own sized build, with no number of
        # its own: at the general ones the shell's link ran beside three
        # release units, at one core.
        self.assertEqual(built["units"], ["/nix/store/s-sized/bin/ronoctua-sized-build", "--no-link"] + self.PRODUCTS)

    def test_the_numbers_given_are_every_builds(self):
        builds = pb.variant_builds(self.PATHS, (2, 3), None)
        self.assertEqual([name for name, _ in builds], ["layer", "units"])
        for _, command in builds:
            self.assertEqual(command[:8], ["nix", "build", "--no-link", "--max-jobs", "2", "--option", "cores", "3"])
        self.assertEqual(dict(builds)["units"][8:], self.PRODUCTS)

    def sizing(self, root, says, status=0):
        program = os.path.join(root, "sizing")
        body = f"#!/bin/sh\nprintf '%s' '{says}'\nexit {status}"
        subprocess.run(["/bin/sh", "-c", 'printf "%s\n" "$1" > "$2" && chmod +x "$2"', "sh", body, program],
                       check=True)
        return program

    def test_the_sizing_is_read_and_never_guessed(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            said = "slots=5\nrust_jobs=5\njobs=4\ncores=1\nheavy_cores=9\n"
            self.assertEqual(pb.sizing_of(self.sizing(root, said)), {"slots": 5, "jobs": 4, "cores": 1})
        for says, status in [("jobs=4\ncores=1\n", 0), ("slots=0\njobs=4\ncores=1\n", 0),
                             ("slots=many\njobs=4\ncores=1\n", 0), ("slots=5\njobs=4\ncores=1\n", 1)]:
            with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
                with self.assertRaises(SystemExit, msg=says):
                    pb.sizing_of(self.sizing(root, says, status))

    def test_what_the_sized_build_said_is_read_back(self):
        log = "memory: 14000 MiB in total\n4 compilations fit at once -> the Rust units: --max-jobs 4 --cores 1\n== x =="
        self.assertEqual(pb.sized_line(log), "4 compilations fit at once -> the Rust units: --max-jobs 4 --cores 1")
        self.assertIsNone(pb.sized_line("nothing of the kind"))


class Rounds(unittest.TestCase):
    """The rounds over fake runners: written by a `/bin/sh` child, never by
    this process (a file it holds open for writing when another test forks
    is `ETXTBSY` at random)."""

    def runner(self, root, letter, body):
        where = os.path.join(root, f"runner-{letter}", "bin")
        os.makedirs(where)
        program = os.path.join(where, f"ronoctua-bench-profile-{letter.lower()}")
        subprocess.run(["/bin/sh", "-c", 'printf "%s\\n" "$1" > "$2" && chmod +x "$2"', "sh", body, program],
                       check=True)
        return os.path.dirname(where)

    def test_the_variants_take_turns(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            order = os.path.join(root, "order")
            built = {}
            for letter in ("A", "C", "D"):
                body = f'#!/bin/sh\necho {letter} >> "{order}"\necho "args: $*"\necho "tmp: $TMPDIR"'
                built[letter] = {"runner": self.runner(root, letter, body)}
                os.makedirs(os.path.join(root, letter))
            args = type("Args", (), {"rounds": 3})()
            pb.run_rounds(args, built, root, ["5000"])
            with open(order) as f:
                self.assertEqual(f.read().split(), ["A", "C", "D", "C", "D", "A", "D", "A", "C"])
            with open(os.path.join(root, "C", "run-2.log")) as f:
                text = f.read()
            self.assertIn("args: 5000", text)
            self.assertEqual(pb.exit_of(text), 0)
            # Every round's TMPDIR is a folder of the run — on disk, never
            # the RAM tmpfs it would inherit.
            self.assertIn(f"tmp: {os.path.join(root, 't')}\n", text)
            self.assertTrue(os.path.isdir(os.path.join(root, "t")))

    def test_a_tmpdir_the_compositors_socket_does_not_fit_under_is_refused(self):
        self.assertEqual(pb.SOCKET_PATH_MAX, 107)
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            # The longest `--out` whose `t/` still holds the socket's path,
            # and one byte more — against a limit moved to where this
            # folder is, so that the test does not depend on how long the
            # path of the folder it runs in is.
            fits = os.path.join(root, "xxxx")
            saved = pb.SOCKET_PATH_MAX
            pb.SOCKET_PATH_MAX = len(os.path.join(fits, "t", "ronoctua-bench-XXXXXX", "ipc.sock"))
            try:
                self.assertEqual(pb.rounds_tmpdir(fits), os.path.join(fits, "t"))
                too_long = fits + "x"
                with self.assertRaises(SystemExit):
                    pb.rounds_tmpdir(too_long)
                self.assertFalse(os.path.exists(too_long))
            finally:
                pb.SOCKET_PATH_MAX = saved

    def test_a_round_that_hangs_is_killed_with_what_it_started(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            pidfile = os.path.join(root, "grandchild")
            body = f'#!/bin/sh\nsleep 300 &\necho $! > "{pidfile}"\nwait'
            built = {"A": {"runner": self.runner(root, "A", body)}}
            os.makedirs(os.path.join(root, "A"))
            saved = pb.RUN_TIMEOUT_S
            pb.RUN_TIMEOUT_S = 1
            try:
                pb.run_rounds(type("Args", (), {"rounds": 1})(), built, root, [])
            finally:
                pb.RUN_TIMEOUT_S = saved
            with open(os.path.join(root, "A", "run-1.log")) as f:
                text = f.read()
            self.assertEqual(pb.exit_of(text), 124)
            self.assertEqual(pb.classify(text, 124), "failed")
            self.assertEqual(pb.failure_of(text, 124), "== killed after 1 s")
            with open(pidfile) as f:
                grandchild = int(f.read())
            # Killed: gone, or a zombie nobody has reaped yet (in a build
            # sandbox, the builder is the namespace's init).
            for _ in range(50):
                try:
                    with open(f"/proc/{grandchild}/stat") as f:
                        if f.read().rsplit(")", 1)[1].split()[0] == "Z":
                            break
                except FileNotFoundError:
                    break
                time.sleep(0.1)
            else:
                self.fail("the round's own child survived the kill")


class Report(unittest.TestCase):
    """A directory a run could have left: two variants, five rounds each,
    their cost files, metrics and layer logs."""

    def unit_dir(self, root, name, seconds):
        d = os.path.join(root, "store", name)
        os.makedirs(d)
        with open(os.path.join(d, "seconds"), "w") as f:
            f.write(f"{seconds}\n")
        with open(os.path.join(d, "peak-kib"), "w") as f:
            f.write("1024\n")
        return d

    def bundle(self, root, letter, shell_bytes):
        where = os.path.join(root, "store", f"bundle-{letter}", "bin")
        os.makedirs(where)
        for name, size in ((".ronoctua-shell-wrapped", shell_bytes), ("ronoctua-shell", 20), ("ronoctua-bench-layout", 500)):
            with open(os.path.join(where, name), "wb") as f:
                f.write(b"x" * size)
        return os.path.dirname(where)

    def variant(self, root, letter, lto, cgu, shell_lib, shell_bin, layout, shared=None, shell_bytes=1000,
                layer_log=None, sizing=None):
        shared = shared or {}
        units = [
            ("ronoctua-bench-layout-0.1.0", "bin", "ronoctua-render", ["rust-lib-ronoctua-xdg"], 1),
            ("rust-lib-ronoctua-xdg", "lib", "ronoctua-xdg", [], 3),
            ("rust-lib-ronoctua-zip", "lib", "ronoctua-zip", [], 2),
            ("rust-lib-ronoctua-stick", "lib", "ronoctua-stick", [], 2),
            ("rust-lib-ronoctua-http", "lib", "ronoctua-http", ["rust-lib-ronoctua-xdg"], 4),
            ("rust-lib-ronoctua-brain", "lib", "ronoctua-brain", ["rust-lib-ronoctua-xdg"], 5),
            (
                "rust-lib-ronoctua-footer",
                "lib",
                "ronoctua-footer",
                ["rust-lib-ronoctua-http", "rust-lib-ronoctua-zip", "rust-lib-ronoctua-stick", "rust-lib-ronoctua-brain"],
                shell_lib,
            ),
            (
                "ronoctua-shell-0.1.0",
                "bin",
                "ronoctua-shell",
                ["rust-lib-ronoctua-footer", "rust-lib-ronoctua-http", "rust-lib-ronoctua-xdg"],
                shell_bin,
            ),
        ]
        rows = []
        for name, kind, crate, deps, seconds in units:
            program = name.rsplit("-", 1)[0]
            owner = shared.get(program, letter)
            where = os.path.join(root, "store", f"{owner}-{name}")
            rows.append(
                {
                    "name": name,
                    "kind": kind,
                    "crate": crate,
                    "deps": deps,
                    "metrics": where if os.path.isdir(where) else self.unit_dir(root, f"{owner}-{name}", seconds),
                }
            )
        layer = os.path.join(root, "store", f"layer-{letter}")
        os.makedirs(os.path.join(layer, "cargo"))
        with open(os.path.join(layer, "cargo", "release.log"), "w") as f:
            f.write(
                layer_log
                if layer_log is not None
                else f"rustc -C codegen-units={cgu}\n    Finished `release` profile [optimized] target(s) in 7m 00s\n"
            )
        cost = os.path.join(root, "store", f"cost-{letter}.json")
        with open(cost, "w") as f:
            json.dump({"variant": letter, "lto": lto, "codegenUnits": cgu, "layer": layer, "units": rows}, f)
        where = os.path.join(root, "out", letter)
        os.makedirs(where)
        with open(os.path.join(where, "build.json"), "w") as f:
            json.dump(
                {
                    "cost": cost,
                    "runner": "/nowhere",
                    "bundle": self.bundle(root, letter, shell_bytes),
                    "wall": {"layer": 1, "units": 2},
                    # As a sized run writes it, or as a run before the sized
                    # builds did (two numbers).
                    **(sizing or {"sizing": {"layer": {"max-jobs": 1, "cores": 4},
                                             "units": "4 compilations fit at once -> the Rust units: ..."}}),
                },
                f,
            )
        for r in range(1, 6):
            with open(os.path.join(where, f"run-{r}.log"), "w") as f:
                f.write(f"== round {r} t\n" + bench_log(layout_p50=layout) + "== exit 0\n")

    def test_a_whole_report(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "C", "false", 16, shell_lib=120, shell_bin=10, layout="0.0034")
            self.variant(root, "D", "thin", 16, shell_lib=60, shell_bin=300, layout="0.0040")
            out = os.path.join(root, "out")
            result = pb.report(out, ["A", "C", "D"])
            self.assertEqual(result["winner"], "C")
            self.assertEqual(result["ranking"], ["C", "A"])
            self.assertFalse(result["summary"]["D"]["acceptable"])
            self.assertIn("layout-200col-p50-ms", result["summary"]["D"]["reasons"][0])
            # The shell is programs only: its edit is its program.
            self.assertEqual(result["summary"]["C"]["edit"], 10)
            self.assertEqual(result["summary"]["A"]["edit"], 500)
            self.assertEqual(result["costs"]["C"]["cold"]["layer"], 420)
            self.assertEqual(result["costs"]["A"]["edits"]["ronoctua-xdg"]["critical"], 3 + 5 + 60 + 500)
            with open(os.path.join(out, "report.md")) as f:
                md = f.read()
            self.assertIn("Winner: **C**", md)
            self.assertIn("0.0034 ~", md)
            # Sizes are reported: the wrapped shell is the binary behind
            # its wrapper.
            self.assertEqual(result["costs"]["A"]["sizes"]["programs"]["ronoctua-shell"], 1000)
            self.assertEqual(result["costs"]["A"]["sizes"]["total"], 1520)
            # How each variant was built is reported with it.
            self.assertEqual(result["costs"]["A"]["sizing"]["layer"], {"max-jobs": 1, "cores": 4})
            with open(os.path.join(out, "report.json")) as f:
                self.assertEqual(json.load(f)["winner"], "C")
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033",
                         sizing={"max-jobs": 2, "cores": 2})
            result = pb.report(os.path.join(root, "out"), ["A"])
            self.assertEqual(result["costs"]["A"]["sizing"], {"max-jobs": 2, "cores": 2})

    def test_noise_in_a_shared_program_does_not_refuse_a_variant(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            # The layout bench is A's own program in E (one unit), and its
            # rounds came out slower: noise, not E.
            self.variant(root, "E", "thin", 1, shell_lib=60, shell_bin=380, layout="0.0040",
                         shared={"ronoctua-bench-layout": "A"})
            result = pb.report(os.path.join(root, "out"), ["A", "E"])
            self.assertEqual(result["winner"], "E")
            self.assertEqual(result["summary"]["E"]["same-programs"], ["ronoctua-bench-layout"])
        # Unshared, the same rounds refuse it.
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "E", "thin", 1, shell_lib=60, shell_bin=380, layout="0.0040")
            self.assertEqual(pb.report(os.path.join(root, "out"), ["A", "E"])["winner"], "A")

    def test_the_size_of_every_program_is_reported(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "E", "thin", 1, shell_lib=60, shell_bin=380, layout="0.0033", shell_bytes=1103)
            out = os.path.join(root, "out")
            result = pb.report(out, ["A", "E"])
            # Reported, not judged: the bigger shell still wins.
            self.assertEqual(result["winner"], "E")
            self.assertEqual(result["costs"]["E"]["sizes"]["programs"]["ronoctua-shell"], 1103)
            with open(os.path.join(out, "report.md")) as f:
                md = f.read()
            self.assertIn("| ronoctua-shell (MB) | 0.0 | 0.0 (+10.3%) |", md)
            # The bench whose size did not move has no row of its own.
            self.assertNotIn("| ronoctua-bench-layout (MB)", md)
            # A run that did not record its bundle cannot say its sizes.
            path = os.path.join(out, "E", "build.json")
            with open(path) as f:
                built = json.load(f)
            del built["bundle"]
            with open(path, "w") as f:
                json.dump(built, f)
            with self.assertRaises(SystemExit):
                pb.report(out, ["A", "E"])

    def test_a_layer_of_unknown_time_is_refused(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "C", "false", 16, shell_lib=120, shell_bin=10, layout="0.0033",
                         layer_log="rustc -C codegen-units=16\n")
            with self.assertRaises(SystemExit):
                pb.report(os.path.join(root, "out"), ["A", "C"])

    def test_a_number_the_script_does_not_read_refuses_the_report(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "C", "false", 16, shell_lib=120, shell_bin=10, layout="0.0033")
            with open(os.path.join(root, "out", "C", "run-3.log"), "a") as f:
                f.write("names by date        : 900 queries  p50=1.0 ms  p95=2.0 ms  p99=3.0 ms  max=4.0 ms\n")
            with self.assertRaises(SystemExit):
                pb.report(os.path.join(root, "out"), ["A", "C"])

    def test_a_round_whose_program_failed_refuses_the_variant(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "C", "false", 16, shell_lib=120, shell_bin=10, layout="0.0033")
            # Two of five rounds of C crash in the index bench: three valid
            # rounds remain, and C is still refused.
            for r in (2, 4):
                with open(os.path.join(root, "out", "C", f"run-{r}.log"), "w") as f:
                    f.write(f"== round {r} t\n" + bench_log(frames=False).split("== the super index")[0])
                    f.write("ERROR: the query refused: Busy\n== exit 1\n")
            result = pb.report(os.path.join(root, "out"), ["A", "C"])
            self.assertEqual(result["winner"], "A")
            self.assertFalse(result["summary"]["C"]["acceptable"])
            self.assertEqual(result["summary"]["C"]["failed"], ["run-2.log", "run-4.log"])
            self.assertIn("its programs failed in run-2.log (ERROR: the query refused: Busy)", result["summary"]["C"]["reasons"])
        # The harness failing in the same two rounds leaves them out, and C
        # is judged on the three it measured.
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0033")
            self.variant(root, "C", "false", 16, shell_lib=120, shell_bin=10, layout="0.0033")
            for r in (2, 4):
                with open(os.path.join(root, "out", "C", f"run-{r}.log"), "w") as f:
                    f.write(f"== round {r} t\n" + bench_log(frames=False))
                    f.write("ERROR: the nested compositor did not come up.\n== exit 1\n")
            result = pb.report(os.path.join(root, "out"), ["A", "C"])
            self.assertEqual(result["winner"], "C")
            self.assertEqual(result["summary"]["C"]["broken"], ["run-2.log", "run-4.log"])
            self.assertEqual(result["summary"]["C"]["valid-rounds"], 3)

    def test_a_reference_with_too_few_rounds_decides_nothing(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as root:
            self.variant(root, "A", "thin", 1, shell_lib=60, shell_bin=500, layout="0.0035")
            self.variant(root, "C", "false", 16, shell_lib=120, shell_bin=10, layout="0.0035")
            for r in range(1, 4):
                path = os.path.join(root, "out", "A", f"run-{r}.log")
                with open(path, "w") as f:
                    f.write("== round x\nERROR: the nested compositor did not come up.\n== exit 1\n")
            result = pb.report(os.path.join(root, "out"), ["A", "C"])
            self.assertIsNone(result["winner"])
            self.assertEqual(result["summary"]["A"]["broken"], ["run-1.log", "run-2.log", "run-3.log"])


if __name__ == "__main__":
    unittest.main()
