"""The unit driver's own arithmetic, proved without a compiler.
`python3 -m unittest nix/scripts/test_rust_unit.py` — and the
`rust-build-scripts-pass-their-tests` check runs the same file in a build.

The flags a unit passes are cargo's for that unit (`build_base_args`), a
build script's `cargo::` lines are read the way cargo reads them, and a read
of a name nobody declared stops the build."""

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("rust_unit", os.path.join(HERE, "rust-unit.py"))
ru = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ru)

RELEASE = {"opt_level": "3", "lto": "thin", "codegen_units": 1, "debuginfo": 0, "debug_assertions": False,
           "overflow_checks": False, "panic": "unwind", "rpath": False, "strip": "debuginfo", "root": "release"}
HOST = dict(RELEASE, opt_level="0", codegen_units=None)
CRATE = {"name": "ronoctua-x", "dir": "crates/ronoctua-x", "features": ["a", "b"], "lints": ["--warn=missing_docs"],
         "pkgEnv": {}, "hasLib": True, "links": None, "version": "0.1.0"}


def unit(kind="lib", mode="build", profile="release", lto="bitcode", features=("a",), target="ronoctua-x"):
    return {"kind": [kind], "crate_types": [kind if kind != "custom-build" else "bin"], "target": target,
            "src": "src/lib.rs", "edition": "2024", "mode": mode, "profile": profile, "lto": lto,
            "features": list(features), "deps": []}


class Flags(unittest.TestCase):
    def setUp(self):
        ru.PROFILES = {"release": RELEASE, "host": HOST}
        ru.UNIT = {"rustc": "1.97.1", "crate": CRATE}
        ru.TOP = "/build"

    def args(self, u, harness=True):
        a, meta = ru.rustc_args(u, "key", CRATE, "/out", "dep-info,link", harness=harness)
        return a, meta

    def test_a_release_library_is_cargo_s(self):
        a, meta = self.args(unit())
        line = " ".join(a)
        self.assertIn("--crate-name ronoctua_x --edition=2024 crates/ronoctua-x/src/lib.rs", line)
        self.assertIn("--crate-type lib", line)
        self.assertIn("-C opt-level=3 -C linker-plugin-lto -C codegen-units=1 --warn=missing_docs", line)
        self.assertIn('--cfg feature="a"', line)
        self.assertIn('--check-cfg cfg(feature, values("a", "b"))', line)
        self.assertIn(f"-C metadata={meta} -C extra-filename=-{meta}", line)
        self.assertIn("-C strip=debuginfo", line)
        self.assertNotIn("debug-assertions", line)
        self.assertNotIn("--remap-path-prefix", line)

    def test_a_host_unit_says_what_opt_level_0_would_not(self):
        a, _ = self.args(unit(kind="custom-build", profile="host", lto="object"))
        line = " ".join(a)
        self.assertNotIn("opt-level", line)
        self.assertIn("-C embed-bitcode=no", line)
        self.assertIn("-C debug-assertions=off", line)
        self.assertIn("--crate-type bin", line)

    def test_a_harness_is_a_test_and_not_a_crate_type(self):
        a, _ = self.args(unit(mode="test", lto="object"))
        self.assertIn("--test", a)
        self.assertNotIn("--crate-type", a)
        b, _ = self.args(unit(mode="test", lto="object"), harness=False)
        self.assertNotIn("--test", b)
        self.assertEqual(b[b.index("--crate-type") + 1], "bin")

    def test_overflow_checks_follow_cargo_s_rule(self):
        ru.PROFILES["checked"] = dict(RELEASE, overflow_checks=True)
        a, _ = self.args(unit(profile="checked"))
        self.assertIn("overflow-checks=on", " ".join(a))

    def test_the_metadata_is_the_key_s(self):
        _, one = ru.rustc_args(unit(), "k1", CRATE, "/o", "link")
        _, two = ru.rustc_args(unit(), "k2", CRATE, "/o", "link")
        _, again = ru.rustc_args(unit(), "k1", CRATE, "/o", "link")
        self.assertNotEqual(one, two)
        self.assertEqual(one, again)

    def test_outside_the_sandbox_paths_are_remapped(self):
        ru.TOP = "/tmp/nix-build-x"
        a, _ = self.args(unit())
        self.assertIn("--remap-path-prefix=/tmp/nix-build-x=/build", a)

    def test_lto_names(self):
        self.assertEqual(ru.lto_args("run:thin"), ["-C", "lto=thin"])
        self.assertEqual(ru.lto_args("run:"), ["-C", "lto"])
        self.assertEqual(ru.lto_args("both"), [])
        self.assertEqual(ru.lto_args("off"), ["-C", "lto=off", "-C", "embed-bitcode=no"])


class Script(unittest.TestCase):
    def test_both_syntaxes_are_read(self):
        out = "\n".join([
            "cargo:rerun-if-env-changed=RONOCTUA_A",
            "cargo::rerun-if-env-changed=RONOCTUA_B",
            "cargo::rustc-link-search=native=/nix/store/x/lib",
            "cargo:rustc-link-search=/nix/store/y/lib",
            "cargo::rustc-link-lib=dylib=ghostty-vt",
            "cargo::rustc-cfg=ghostty_vt",
            "cargo::rustc-check-cfg=cfg(ghostty_vt)",
            "cargo::rustc-env=WHERE=here",
            "cargo::rustc-link-arg=-Wl,-rpath,/nix/store/z/lib",
            "cargo::rustc-link-arg-bin=prog=-lfoo",
            "cargo:include=/inc",
            "cargo::metadata=root=/r",
            "not an instruction",
        ])
        said = ru.parse_script_output(out, CRATE)
        self.assertEqual(said["reruns"], ["RONOCTUA_A", "RONOCTUA_B"])
        self.assertEqual(said["search"], [{"kind": "native", "path": "/nix/store/x/lib"}, {"kind": "native", "path": "/nix/store/y/lib"}])
        self.assertEqual(said["libs"], ["dylib=ghostty-vt"])
        self.assertEqual(said["cfgs"], ["ghostty_vt"])
        self.assertEqual(said["checkCfgs"], ["cfg(ghostty_vt)"])
        self.assertEqual(said["env"], {"WHERE": "here"})
        self.assertEqual(said["linkArgs"], [["all", "-Wl,-rpath,/nix/store/z/lib"], ["bin=prog", "-lfoo"]])
        self.assertEqual(said["metadata"], {"include": "/inc", "root": "/r"})

    def test_an_unknown_new_instruction_stops_the_build(self):
        with self.assertRaises(SystemExit):
            ru.parse_script_output("cargo::rustc-link-everything=1", CRATE)

    def test_a_reported_error_stops_the_build(self):
        with self.assertRaises(SystemExit):
            ru.parse_script_output("cargo::error=no table", CRATE)

    def test_link_args_reach_the_targets_cargo_gives_them_to(self):
        lib, prog, other = unit(), unit(kind="bin", target="prog"), unit(kind="bin", target="other")
        test = unit(kind="test", mode="test")
        self.assertTrue(ru.link_arg_applies("all", lib))
        self.assertTrue(ru.link_arg_applies("bins", prog))
        self.assertFalse(ru.link_arg_applies("bins", lib))
        self.assertTrue(ru.link_arg_applies("bin=prog", prog))
        self.assertFalse(ru.link_arg_applies("bin=prog", other))
        self.assertTrue(ru.link_arg_applies("tests", test))
        self.assertFalse(ru.link_arg_applies("tests", prog))


class Reads(unittest.TestCase):
    def setUp(self):
        ru.UNIT = {"crate": CRATE, "declared": ["RONOCTUA_YES"]}

    def test_a_declared_read_passes_and_others_are_not_ours(self):
        ru.verify_reads({"RONOCTUA_YES", "CARGO_PKG_VERSION", "OUT_DIR"}, "x")

    def test_an_undeclared_read_stops_the_build(self):
        with self.assertRaises(SystemExit):
            ru.verify_reads({"RONOCTUA_YES", "RONOCTUA_NO"}, "x")

    def test_dep_info_names_every_read_value_or_not(self):
        with tempfile.NamedTemporaryFile("w", suffix=".d", delete=False) as f:
            f.write("/o/x.rlib: src/lib.rs\n\nsrc/lib.rs:\n\n# env-dep:RONOCTUA_SET=/nix/store/a\n# env-dep:RONOCTUA_UNSET\n")
        self.assertEqual(ru.read_env_deps(f.name), {"RONOCTUA_SET", "RONOCTUA_UNSET"})
        os.unlink(f.name)


class Search(unittest.TestCase):
    def test_cargo_s_artifacts_come_first_and_once(self):
        ru.LAYER = "/layer"
        deps = ru.Deps.__new__(ru.Deps)
        deps.native = [
            {"kind": "native", "path": "/nix/store/sys/lib"},
            {"kind": "native", "layer": "native/a"},
            {"kind": "native", "path": "/nix/store/sys/lib"},
        ]
        own = [{"kind": "native", "path": "/build/out-dir/x", "artifact": True}]
        self.assertEqual(
            deps.native_args(own),
            ["-L", "native=/build/out-dir/x", "-L", "native=/layer/native/a", "-L", "native=/nix/store/sys/lib"],
        )

    def test_a_search_kind_is_kept(self):
        self.assertEqual(ru.search_entry("framework=/f"), {"kind": "framework", "path": "/f"})
        self.assertEqual(ru.search_entry("/plain"), {"kind": "native", "path": "/plain"})


class Links(unittest.TestCase):
    RUNS = {
        "aws@1/custom-build:b/run-custom-build/o3/x": {
            "links": "aws_lc_rs_1_sys",
            "metadata": {"include": "@LAYER@/native/release/build/aws-1/out/include", "conf": "NO_ASYNC"},
            "unreachable": [],
        },
        "bad@1/custom-build:b/run-custom-build/o3/y": {
            "links": "bad", "metadata": {"src": "/build/source/include"}, "unreachable": ["src"],
        },
    }

    def test_a_links_path_names_this_unit_s_layer(self):
        env = ru.links_env(CRATE, ["aws@1/custom-build:b/run-custom-build/o3/x"], self.RUNS, "/nix/store/l-index")
        self.assertEqual(env, {
            "DEP_AWS_LC_RS_1_SYS_INCLUDE": "/nix/store/l-index/native/release/build/aws-1/out/include",
            "DEP_AWS_LC_RS_1_SYS_CONF": "NO_ASYNC",
        })

    def test_an_unreachable_value_is_refused_not_handed_over(self):
        with self.assertRaises(SystemExit):
            ru.links_env(CRATE, ["bad@1/custom-build:b/run-custom-build/o3/y"], self.RUNS, "/nix/store/l-index")

    def test_a_run_the_layer_could_not_attribute_is_refused(self):
        with self.assertRaises(SystemExit):
            ru.links_env(CRATE, ["other@1/custom-build:b/run-custom-build/o3/z"], self.RUNS, "/nix/store/l-index")


class Keeps(unittest.TestCase):
    """A unit keeps its product and nothing else: a `deps/` of compiled
    crates is a layer's alone."""

    def plant(self, root, files):
        for f in files:
            path = os.path.join(root, f)
            if f.endswith("/"):
                os.makedirs(path, exist_ok=True)
            else:
                os.makedirs(os.path.dirname(path), exist_ok=True)
                open(path, "w").close()

    def test_each_kind_keeps_its_product(self):
        products = {
            "lib": ["lib/libronoctua_x-0123.rlib", "lib/libronoctua_x-0123.rmeta", "out-dir/gen.rs",
                    "nix-support/unit.json"],
            "dep": ["lib/libserde_derive-0123.so", "out-dir/gen.rs", "nix-support/unit.json",
                    "nix-support/build-script.json", "nix-support/rustc.json"],
            "script": ["out-dir/gen.rs", "nix-support/build-script.json"],
            "bin": ["bin/ronoctua-x"],
            "test-bins": ["bin/ronoctua_x-0123", "libexec/ronoctua-x", "nix-support/tests.json"],
            "test-run": ["log", "verdict"],
            "test-one": ["log"],
            "clippy": ["verdict"],
            "fmt": ["verdict"],
        }
        for mode, files in products.items():
            with tempfile.TemporaryDirectory() as d:
                out, metrics = os.path.join(d, "out"), os.path.join(d, "metrics")
                self.plant(out, files)
                self.plant(metrics, ["format", "seconds", "peak-kib", "largest-kib", "cores"])
                self.assertEqual(ru.kept_beyond(mode, out, metrics), [], mode)

    def test_a_deps_directory_is_refused_in_every_kind(self):
        for mode in ru.KEEPS:
            with tempfile.TemporaryDirectory() as d:
                out = os.path.join(d, "out")
                self.plant(out, ["deps/libserde-1.rlib"])
                said = ru.kept_beyond(mode, out, None)
                self.assertEqual(len(said), 1, mode)
                self.assertIn("deps/ beside its product", said[0])

    def test_what_is_not_the_crate_in_lib_is_refused(self):
        for mode in ("lib", "dep"):
            with tempfile.TemporaryDirectory() as d:
                out = os.path.join(d, "out")
                self.plant(out, ["lib/libronoctua_x-0123.rlib", "lib/ronoctua_x-0123.d", "lib/incremental/"])
                said = ru.kept_beyond(mode, out, None)
                self.assertEqual(sorted(said), ["lib/incremental, which is not the crate compiled",
                                                "lib/ronoctua_x-0123.d, which is not the crate compiled"], mode)

    def test_a_measurement_is_its_numbers_and_nothing_else(self):
        with tempfile.TemporaryDirectory() as d:
            out, metrics = os.path.join(d, "out"), os.path.join(d, "metrics")
            self.plant(out, ["verdict"])
            self.plant(metrics, ["format", "seconds", "peak-kib", "rustc.log"])
            self.assertEqual(ru.kept_beyond("fmt", out, metrics),
                             ["metrics/rustc.log, which is no measurement (cores, format, largest-kib, peak-kib, seconds)"])

    def test_the_build_stops_naming_what_it_kept(self):
        with tempfile.TemporaryDirectory() as d:
            ru.OUT, ru.METRICS = os.path.join(d, "out"), None
            self.plant(ru.OUT, ["verdict", "target/"])
            with self.assertRaises(SystemExit):
                ru.keep_only_the_product("clippy")

    def test_an_unknown_kind_keeps_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(len(ru.kept_beyond("doc", d, None)), 1)


class Measures(unittest.TestCase):
    """A unit measures its whole build — the builder and every process below
    it — by what the budget pays for (anonymous memory, shmem, swap and page
    tables), sampled, and says the cores it was built with."""

    def proc(self, d, pid, parent, name="rustc", held=None, rss=None, old_kernel=False, statm=None, started=100,
             exe=None, layout=None):
        """A process of a planted `/proc`: `held` its RssAnon (with 1 kB of
        each other field of `HELD`), `rss` its VmRSS; `old_kernel` a status
        with no split of its memory; `statm` a process with no status;
        `started` when it started (field 22 of its stat); `exe` where its
        `exe` link points; `layout` where `execve` laid it out (the fields
        `LAYOUT` of its stat, which a planted stat has none of otherwise)."""
        base = os.path.join(d, str(pid))
        os.makedirs(base, exist_ok=True)
        fields = f"S {parent} 1 1 0 -1 4194304 0 0 0 0 1 2 0 0 20 0 1 0 {started} 1000 10".split()
        if layout is not None:
            fields += ["0"] * (max(ru.LAYOUT) - 2 - len(fields))
            for n, value in zip(ru.LAYOUT, layout):
                fields[n - 3] = str(value)
        with open(os.path.join(base, "stat"), "w") as f:
            f.write(f"{pid} ({name}) " + " ".join(fields) + "\n")
        link = os.path.join(base, "exe")
        if os.path.lexists(link):
            os.remove(link)
        if exe is not None:
            os.symlink(exe, link)
        if held is not None or rss is not None:
            lines = [f"Name:\t{name}", f"VmHWM:\t{(rss or 0) + 999} kB", f"VmRSS:\t{rss or 0} kB"]
            if not old_kernel:
                lines += [f"RssAnon:\t{held or 0} kB", f"RssFile:\t{(rss or 0) - (held or 0)} kB",
                          "RssShmem:\t1 kB", "VmSwap:\t1 kB", "VmPTE:\t1 kB"]
            with open(os.path.join(base, "status"), "w") as f:
                f.write("\n".join(lines) + "\n")
        if statm is not None:
            with open(os.path.join(base, "statm"), "w") as f:
                f.write(f"9999 {statm} 10 1 0 100 0\n")

    def test_the_parent_is_read_past_a_name_with_parentheses_and_spaces(self):
        self.assertEqual(ru.parent_of("4242 (rustc (x) y) S 17 4242 1 0 -1"), 17)
        stat = "4242 (rustc (x) y) S 17 4242 1 0 -1 4194304 0 0 0 0 1 2 0 0 20 0 1 0 98765 1000 10"
        self.assertEqual(ru.started_of(stat), 98765)
        self.assertIsNone(ru.started_of("4242 (rustc) S 17 4242 1 0 -1"))

    def test_the_tree_is_the_root_and_everything_below_it(self):
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 10, 1, "python3")
            self.proc(d, 11, 10)
            self.proc(d, 12, 10, "cc")
            self.proc(d, 13, 12, "ld (gold) x")
            self.proc(d, 20, 1, "other")
            self.proc(d, 21, 20)
            os.makedirs(os.path.join(d, "30"))  # gone before its stat was read
            os.makedirs(os.path.join(d, "self"))
            self.assertEqual(sorted(ru.build_tree(10, d)), [10, 11, 12, 13])
            self.assertEqual(ru.build_tree(99, d), [99])

    def test_a_process_is_what_it_holds_and_never_its_file_pages(self):
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 10, 1, held=1000, rss=4000)
            # RssAnon, and the shmem, swap and page tables beside it; never
            # the 3000 kB of file pages.
            self.assertEqual(ru.memory_kib(10, d), (1003, "held"))
            self.assertEqual(ru.memory_kib(99, d), (0, "gone"))
            # A zombie's status says no memory: it holds nothing.
            with open(os.path.join(d, "10", "status"), "w") as f:
                f.write("Name:\trustc\nState:\tZ (zombie)\n")
            self.assertEqual(ru.memory_kib(10, d), (0, "gone"))

    def test_what_is_readable_is_read_when_the_split_is_not(self):
        with tempfile.TemporaryDirectory() as d:
            # A kernel before 4.5: no RssAnon, the whole resident set.
            self.proc(d, 10, 1, rss=4000, old_kernel=True)
            self.assertEqual(ru.memory_kib(10, d), (4000, "rss"))
            # No status at all: statm's resident pages.
            self.proc(d, 11, 10, statm=25)
            self.assertEqual(ru.memory_kib(11, d), (25 * os.sysconf("SC_PAGE_SIZE") // 1024, "rss"))
            # Nothing readable but the stat: nothing.
            self.proc(d, 12, 10)
            self.assertEqual(ru.memory_kib(12, d), (0, "gone"))

    def test_a_kernel_that_does_not_split_its_memory_still_measures(self):
        # The first reading of an old kernel's `/proc` is its RSS, never a
        # reading of nothing that stops the build.
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 10, 1, "bash", rss=5000, old_kernel=True)
            self.proc(d, 11, 10, "python3", statm=100)
            s = ru.Sampler(10, proc=d, period=60)
            s.start()
            s.stop()
            self.assertEqual(s.most_kib, 5000 + 100 * os.sysconf("SC_PAGE_SIZE") // 1024)
            self.assertEqual(s.by_rss, 2)

    def test_a_reading_sums_the_tree_and_the_most_is_kept(self):
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 9, 1, "bash", held=5000, rss=8000)
            self.proc(d, 10, 9, "python3", held=20000, rss=30000)
            self.proc(d, 11, 10, "rustc", held=400000, rss=700000)
            self.proc(d, 12, 11, "rust-lld", held=300000, rss=1300000)
            self.proc(d, 20, 1, "outside", held=900000, rss=900000)
            s = ru.Sampler(9, proc=d)
            self.assertEqual(s.sample(), 725000 + 4 * 3)
            shutil.rmtree(os.path.join(d, "12"))
            self.assertEqual(s.sample(), 425000 + 3 * 3)
            self.assertEqual((s.most_kib, s.most_processes, s.samples), (725012, 4, 2))
            self.assertEqual(s.largest_kib, 400003)

    def test_each_live_process_counts_at_the_most_it_held(self):
        # The compiler freed what it held before it ran the linker, and its
        # allocator had not yet given it back — or had: which, depends on
        # the moment. Each process counts at its own most while it lives,
        # so both builds measure the same.
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 9, 1, "bash", held=5000, rss=8000)
            self.proc(d, 11, 9, "rustc", held=400000, rss=700000)
            s = ru.Sampler(9, proc=d)
            s.sample()
            self.proc(d, 11, 9, "rustc", held=100000, rss=400000)
            self.proc(d, 12, 11, "rust-lld", held=300000, rss=1300000)
            self.assertEqual(s.sample(), 5003 + 400003 + 300003)
            self.assertEqual(s.most_kib, 705009)
            # Ended, a process holds nothing; and a pid given again is
            # another process, which starts from what it holds.
            shutil.rmtree(os.path.join(d, "12"))
            self.proc(d, 11, 9, "rustc", held=1000, rss=9000, started=500)
            self.assertEqual(s.sample(), 5003 + 1003)
            self.assertEqual(sorted(s.held), [(9, 100), (11, 500)])

    def test_a_fork_copy_is_not_counted_and_its_exec_starts_from_what_it_holds(self):
        # `rustc` starts the linker with a plain fork: until the `execve`,
        # the child's status says rustc's memory, copied on write. Read
        # then and kept as the child's most while it lives (it becomes
        # `gcc`, waiting for the linker), rustc counted twice.
        rustc = (93824992235520, 93824992235865, 140737488346224, 1, 2, 3, 140737488347199, 2, 2, 140737488351152)
        gcc = (93824992100000, 93824992100500, 140737488346000, 4, 5, 6, 140737488347000, 7, 7, 140737488351152)
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 9, 1, "bash", held=5000, rss=8000, exe="/bin/bash", layout=(9,) * 10)
            self.proc(d, 11, 9, "rustc", held=400000, rss=700000, exe="/bin/rustc", layout=rustc)
            self.proc(d, 12, 11, "rustc", held=400000, rss=700000, exe="/bin/rustc", layout=rustc, started=200)
            s = ru.Sampler(9, proc=d)
            self.assertEqual(s.sample(), 5003 + 400003)
            self.proc(d, 12, 11, "gcc", held=1021, rss=3000, exe="/bin/gcc", layout=gcc, started=200)
            self.assertEqual(s.sample(), 5003 + 400003 + 1024)
            self.assertEqual((s.most_kib, s.most_processes, s.copies), (5003 + 400003 + 1024, 3, 1))
            # A process that exec'd holds what its new program holds: the
            # most of the one before is not carried across (the shell of a
            # wrapper that held 300 MiB, then the compiler it became).
            self.proc(d, 13, 9, "sh", held=300000, rss=301000, exe="/bin/sh", layout=(3,) * 10, started=300)
            self.assertEqual(s.sample(), 5003 + 400003 + 1024 + 300003)
            self.proc(d, 13, 9, "cc1", held=1000, rss=2000, exe="/bin/cc1", layout=(4,) * 10, started=300)
            self.assertEqual(s.sample(), 5003 + 400003 + 1024 + 1003)
            # The same program again, laid out the same (no randomization),
            # is the same image: its most is kept.
            self.proc(d, 13, 9, "cc1", held=500, rss=2000, exe="/bin/cc1", layout=(4,) * 10, started=300)
            self.assertEqual(s.sample(), 5003 + 400003 + 1024 + 1003)
            # And a child laid out elsewhere runs another program, even if
            # it is the same executable: counted.
            self.proc(d, 14, 11, "rustc", held=2000, rss=4000, exe="/bin/rustc", layout=gcc, started=400)
            self.assertEqual(s.sample(), 5003 + 400003 + 1024 + 1003 + 2003)

    def test_with_no_layout_or_executable_to_read_a_copy_is_known_by_its_name(self):
        # A kernel whose stat says no layout (before 3.5, or a process the
        # reader may not see into) and no readable `exe`: the name alone.
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 9, 1, "bash", held=5000, rss=8000)
            self.proc(d, 11, 9, "rustc", held=400000, rss=700000)
            self.proc(d, 12, 11, "rustc", held=400000, rss=700000, started=200)
            s = ru.Sampler(9, proc=d)
            self.assertEqual(s.sample(), 5003 + 400003)
            self.proc(d, 12, 11, "gcc", held=1021, rss=3000, started=200)
            self.assertEqual(s.sample(), 5003 + 400003 + 1024)

    def test_a_real_child_waiting_to_exec_is_not_its_parent_twice(self):
        # A real process holding 128 MiB forks a child that waits a second
        # before its `execve` (a stand-in for a busy machine, where the
        # instant between the two is long), then runs `sleep` for a
        # second. Counted at its parent's size while it waits, the child
        # would be 128 MiB more.
        sleep = shutil.which("sleep")
        if sleep is None:
            self.skipTest("no `sleep` to exec")
        work = ("import os, sys, time\n"
                "b = bytearray(128 << 20)\n"
                "for i in range(0, len(b), 4096): b[i] = 1\n"
                "sys.stdout.write('ready\\n'); sys.stdout.flush()\n"
                "pid = os.fork()\n"
                "if pid == 0:\n"
                "    time.sleep(1.0)\n"
                f"    os.execv({sleep!r}, ['sleep', '1'])\n"
                "os.waitpid(pid, 0)\n")
        big = subprocess.Popen([sys.executable, "-c", work], stdout=subprocess.PIPE, text=True)
        self.assertEqual(big.stdout.readline().strip(), "ready")
        s = ru.Sampler(os.getpid(), period=0.02, share=0.5)
        s.start()
        big.wait()
        s.stop()
        big.stdout.close()
        self.assertGreaterEqual(s.most_kib, 128 * 1024)
        self.assertGreater(s.copies, 5)
        # This test's own process, the one holding 128 MiB and `sleep`: far
        # less than twice the 128.
        self.assertLess(s.most_kib, 128 * 1024 + 96 * 1024)

    def test_reading_stays_a_share_of_a_core(self):
        self.assertEqual(ru.next_wait(0.0001), ru.SAMPLE_SECONDS)
        self.assertAlmostEqual(ru.next_wait(0.030), 0.030 * 19)
        took, wait = 0.030, ru.next_wait(0.030)
        self.assertLessEqual(took / (took + wait), ru.SAMPLE_SHARE + 1e-9)
        with tempfile.TemporaryDirectory() as d:
            self.proc(d, 10, 1, "python3", held=20000, rss=30000)
            s = ru.Sampler(10, proc=d, period=0.2)
            s.sample()
            self.assertEqual(s.wait, 0.2)

    def test_a_first_reading_of_nothing_stops_the_build(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(SystemExit):
                ru.Sampler(10, proc=d).start()

    def test_the_tree_is_read_from_the_builder(self):
        self.assertEqual(ru.builder_pid(), os.getppid())

    def test_the_build_runs_with_no_transparent_huge_page(self):
        # Asked of a child, as every process of the build is: the setting is
        # inherited, and kept across `execve`.
        with open("/proc/self/status") as f:
            if not any(line.startswith("THP_enabled:") for line in f):
                self.skipTest("this kernel's status says nothing of transparent huge pages")
        child = [sys.executable, "-c", "import sys; sys.path.insert(0, sys.argv[1]); import importlib.util as u; "
                 "s = u.spec_from_file_location('ru', sys.argv[2]); m = u.module_from_spec(s); s.loader.exec_module(m); "
                 "m.no_huge_pages(); import subprocess; print(m.HUGE_PAGES); "
                 "print(subprocess.run(['cat', '/proc/self/status'], capture_output=True, text=True).stdout)",
                 os.path.dirname(os.path.abspath(ru.__file__)), os.path.abspath(ru.__file__)]
        said = subprocess.run(child, capture_output=True, text=True, check=True).stdout
        self.assertEqual(said.splitlines()[0], "off")
        self.assertIn("THP_enabled:\t0", said)

    def test_the_thread_reads_a_real_child_while_it_runs(self):
        # A real child holding 64 MiB of its own for a while, read by the
        # real `/proc` (outside a build's sandbox it lists every process of
        # the machine, so the share is wide: the test asks for readings, not
        # their cost).
        s = ru.Sampler(os.getpid(), period=0.05, share=0.5)
        s.start()
        child = subprocess.run(
            [sys.executable, "-c", "import time; b = b'x' * (64 << 20); time.sleep(1.0)"], check=True)
        s.stop()
        self.assertEqual(child.returncode, 0)
        self.assertGreaterEqual(s.most_kib, 64 * 1024)
        self.assertGreaterEqual(s.largest_kib, 64 * 1024)
        self.assertGreaterEqual(s.most_processes, 2)
        self.assertGreater(s.samples, 5)
        self.assertEqual(s.by_rss, 0)

    def test_the_peak_is_the_tree_and_says_its_cores(self):
        m = ru.measurement(720000, 400000, 12.345, 3)
        self.assertEqual(m, {"format": f"{ru.METRICS_FORMAT}\n", "peak-kib": "720000\n", "largest-kib": "400000\n",
                             "seconds": "12.35\n", "cores": "3\n"})
        self.assertEqual(set(m), ru.MEASURED)

    def test_the_metrics_are_written_and_are_what_a_unit_may_keep(self):
        with tempfile.TemporaryDirectory() as d:
            saved = ru.METRICS, ru.SAMPLER, ru.CORES
            try:
                ru.METRICS, ru.CORES = os.path.join(d, "metrics"), 4
                ru.SAMPLER = ru.Sampler(os.getpid())
                ru.SAMPLER.most_kib, ru.SAMPLER.largest_kib = 1 << 40, 1 << 30
                ru.write_metrics()
                read = {}
                for n in os.listdir(ru.METRICS):
                    with open(os.path.join(ru.METRICS, n)) as f:
                        read[n] = f.read()
            finally:
                ru.METRICS, ru.SAMPLER, ru.CORES = saved
            self.assertEqual(set(read), ru.MEASURED)
            self.assertEqual((read["format"], read["cores"], read["peak-kib"], read["largest-kib"]),
                             ("3\n", "4\n", f"{1 << 40}\n", f"{1 << 30}\n"))
            os.makedirs(os.path.join(d, "out"))
            open(os.path.join(d, "out", "verdict"), "w").close()
            self.assertEqual(ru.kept_beyond("fmt", os.path.join(d, "out"), os.path.join(d, "metrics")), [])


class ThirdParty(unittest.TestCase):
    """A crate that is not ours, compiled the way cargo compiles a registry
    package (nix-design §1.4, phase 2)."""

    def setUp(self):
        ru.PROFILES = {"release": RELEASE, "host": HOST}
        ru.TOP = "/build"

    def test_a_dependency_s_warnings_are_capped_and_it_is_no_primary_package(self):
        crate = dict(CRATE, capLints=True, primary=False, lints=[], dir=".", srcRoot="/build/foo-1.0.0")
        ru.UNIT = {"rustc": "1.97.1", "crate": crate, "cargo": "cargo"}
        ru.ROOT = "/build/foo-1.0.0"
        deps = ru.Deps.__new__(ru.Deps)
        deps.search, deps.externs, deps.native = ["-L", "dependency=/f"], ["--extern", "a=/a.rlib"], []
        seen = {}
        with tempfile.TemporaryDirectory() as out, \
                mock.patch.object(ru, "run", lambda argv, env, **kw: seen.update(argv=argv)), \
                mock.patch.object(ru, "read_env_deps", lambda path: set()):
            ru.compile_unit(unit(), "k", crate, out, "dep-info,metadata,link", {}, deps, None)
        argv = seen["argv"]
        self.assertIn("/build/foo-1.0.0/src/lib.rs", argv)
        self.assertEqual(argv[argv.index("--extern") + 2:argv.index("--extern") + 4], ["--cap-lints", "allow"])
        self.assertNotIn("CARGO_PRIMARY_PACKAGE", ru.unit_env(crate, unit()))

    def test_a_proc_macro_is_linked_like_cargo_links_one(self):
        u = unit(kind="proc-macro", profile="host", lto="object")
        a, _ = ru.rustc_args(u, "k", CRATE, "/out", ru.lib_emit(u))
        self.assertEqual(a[a.index("--emit=dep-info,link") + 1:][:2], ["-C", "prefer-dynamic"])
        self.assertEqual(ru.lib_emit(unit()), "dep-info,metadata,link")

    def test_versions_split_as_cargo_splits_them(self):
        self.assertEqual(ru.version_parts("1.2.3"), ("1", "2", "3", ""))
        self.assertEqual(ru.version_parts("0.9.0-rc.1+build.5"), ("0", "9", "0", "rc.1"))

    def test_an_optional_dependency_is_a_feature_unless_named_with_dep(self):
        manifest = {
            "features": {"default": ["std"], "std": [], "fast": ["dep:simd"]},
            "dependencies": {"serde": {"version": "1", "optional": True}, "simd": {"optional": True}, "libc": "0.2"},
            "target": {"cfg(unix)": {"dependencies": {"nix": {"optional": True}}}},
            "build-dependencies": {"cc": {"optional": True}},
        }
        self.assertEqual(ru.manifest_features(manifest), ["cc", "default", "fast", "nix", "serde", "std"])

    def test_lints_are_cargo_s_order_and_check_cfg_follows(self):
        manifest = {"lints": {
            "rust": {"unused_qualifications": "allow", "unexpected_cfgs": {"level": "warn", "check-cfg": ["cfg(a)"]}},
            "clippy": {"ptr_as_ptr": "warn", "identity_op": "allow", "all": {"level": "deny", "priority": -1}},
            "cargo": {"anything": "warn"},
        }}
        self.assertEqual(ru.manifest_lints(manifest), [
            "--deny=clippy::all",
            "--allow=unused_qualifications", "--warn=unexpected_cfgs",
            "--warn=clippy::ptr_as_ptr", "--allow=clippy::identity_op",
            "--check-cfg", "cfg(a)",
        ])

    def test_a_dependent_reads_paths_where_the_unit_keeps_them(self):
        script = {
            "search": [{"kind": "native", "path": "/build/out-dir/sys/lib", "artifact": True},
                       {"kind": "native", "path": "/nix/store/x-sqlite/lib"},
                       {"kind": "native", "path": "/build/elsewhere"}],
            "metadata": {"include": "/build/out-dir/sys/include", "src": "/build/sys-1.0.0/c",
                         "lost": "/build/tmp/x", "conf": "NO_ASYNC"},
        }
        moved = ru.relocated(script, [("/build/out-dir/sys", "/nix/store/o-unit/out-dir"),
                                      ("/build/sys-1.0.0", "/nix/store/s-src")], "/build")
        self.assertEqual([e["path"] for e in moved["search"]], ["/nix/store/o-unit/out-dir/lib", "/nix/store/x-sqlite/lib"])
        self.assertEqual(moved["metadata"]["include"], "/nix/store/o-unit/out-dir/include")
        self.assertEqual(moved["metadata"]["src"], "/nix/store/s-src/c")
        self.assertEqual(moved["unreachable"], ["lost"])

    def test_a_path_that_only_starts_like_the_build_directory_is_not_moved(self):
        script = {"search": [], "metadata": {"a": "/build/out-dir/sys2/x", "b": "/nix/store/h-foo/build/out-dir/sys"}}
        moved = ru.relocated(script, [("/build/out-dir/sys", "/o")], "/build")
        self.assertEqual(moved["metadata"], {"a": "/build/out-dir/sys2/x", "b": "/nix/store/h-foo/build/out-dir/sys"})
        self.assertEqual(moved["unreachable"], ["a"])

    def test_a_path_after_a_flag_or_in_quotes_is_moved_or_refused(self):
        script = {"search": [{"kind": "native", "path": "'/build/out-dir/sys/lib'"}], "metadata": {
            "cflags": "-I/build/out-dir/sys/include -DX=1",
            "quoted": '"/build/out-dir/sys/include"',
            "list": "-I/usr/include;-isystem/build/sys-1.0.0/c,@/build/out-dir/sys/link.rsp",
            "url": "file:///build/sys-1.0.0/c",
            "inside": "-I/nix/store/h-foo/build/out-dir/sys/include",
            "lost": "-I/build/tmp/x",
            "lost-quoted": "'/build/tmp/y'",
        }}
        moved = ru.relocated(script, [("/build/out-dir/sys", "/nix/store/o-unit/out-dir"),
                                      ("/build/sys-1.0.0", "/nix/store/s-src")], "/build")
        self.assertEqual([e["path"] for e in moved["search"]], ["'/nix/store/o-unit/out-dir/lib'"])
        self.assertEqual(moved["metadata"], {
            "cflags": "-I/nix/store/o-unit/out-dir/include -DX=1",
            "quoted": '"/nix/store/o-unit/out-dir/include"',
            "list": "-I/usr/include;-isystem/nix/store/s-src/c,@/nix/store/o-unit/out-dir/link.rsp",
            "url": "file:///nix/store/s-src/c",
            "inside": "-I/nix/store/h-foo/build/out-dir/sys/include",
            "lost": "-I/build/tmp/x",
            "lost-quoted": "'/build/tmp/y'",
        })
        self.assertEqual(moved["unreachable"], ["lost", "lost-quoted"])
        runs = {"sys@1/r": {"links": "sys", "metadata": moved["metadata"], "unreachable": moved["unreachable"]}}
        with self.assertRaises(SystemExit):
            ru.links_env(CRATE, ["sys@1/r"], runs, None)

    def test_one_place_is_moved_once_by_its_longest_root(self):
        moves = [("/build/a", "/n/x"), ("/build/a/b", "/n/y"), ("/n/x", "/never")]
        self.assertEqual(ru.moved_paths("/build/a/b/c:/build/a/c", moves), "/n/y/c:/n/x/c")

    def test_the_links_metadata_comes_from_the_unit_that_ran_the_script(self):
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, "nix-support"))
            with open(os.path.join(d, "nix-support", "unit.json"), "w") as f:
                f.write('{"script": {"metadata": {"include": "/nix/store/o/out-dir/include"}, "unreachable": []}}')
            ru.UNIT = {"runs": {"aws@1/r": {"path": d, "links": "aws_lc_1"}}}
            known, layer = ru.links_known(["aws@1/r"])
            self.assertIsNone(layer)
            self.assertEqual(ru.links_env(CRATE, ["aws@1/r"], known, layer), {"DEP_AWS_LC_1_INCLUDE": "/nix/store/o/out-dir/include"})
            ru.UNIT["crate"] = CRATE
            with self.assertRaises(SystemExit):
                ru.links_known(["other@1/r"])

    def test_a_registry_manifest_gives_cargo_s_package_environment(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, "Cargo.toml"), "w") as f:
                f.write('[package]\nname = "foo"\nversion = "1.2.3-beta.1"\nauthors = ["A", "B"]\n'
                        'license = "MIT"\n[features]\nx = []\n')
            open(os.path.join(d, "README.md"), "w").close()
            ru.ROOT = d
            crate = ru.dep_crate({"name": "foo", "version": "1.2.3-beta.1", "links": "foo"})
            self.assertEqual(crate["pkgEnv"]["CARGO_PKG_AUTHORS"], "A:B")
            self.assertEqual(crate["pkgEnv"]["CARGO_PKG_VERSION_PRE"], "beta.1")
            self.assertEqual(crate["pkgEnv"]["CARGO_PKG_README"], "README.md")
            self.assertEqual(crate["features"], ["x"])
            self.assertEqual((crate["links"], crate["srcRoot"], crate["capLints"]), ("foo", d, True))
            with self.assertRaises(SystemExit):
                ru.dep_crate({"name": "bar", "version": "1.2.3-beta.1"})


if __name__ == "__main__":
    unittest.main()
