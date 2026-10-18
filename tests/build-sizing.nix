# HOW MANY COMPILATIONS FIT, AND THE UNITS A MACHINE KEEPS — nix-design §4
# and §2 "Kept on the machine" (lane N4): the arithmetic of
# `nix/build-sizing.nix`, the batteries' phases, what `ronoctua-keep-units`
# roots, `.#rust-peaks`, and the plan a machine's apply builds by.
#
# Joined into `tests/checks.nix` with one line, beside `tests/build-profile.nix`.
{
  pkgs,
  lib,
  self,
  system,
  rust,
  mkCheck,
  pyPrelude,
  # The gate as the flake builds it, whose phases are read back.
  gateApp,
}:

let
  plain = builtins.unsafeDiscardStringContext;
  sizing = pkgs.callPackage ../nix/build-sizing.nix { };
  numbersJson = builtins.toJSON sizing.numbers;

  # A UNIT OF NOTHING: a derivation whose path is real and whose output is
  # never built, with the `passthru.unitDeps` the plan of the heavy units
  # reads.
  fake =
    name: deps:
    (pkgs.writeText "fake-unit-${name}" name)
    // {
      passthru = {
        unitDeps = deps;
        kind = "lib";
      };
    };
  # Two heavy units, the second built from a unit that reaches the first;
  # named so the alphabet would put them the wrong way round.
  a = fake "a" [ ];
  b = fake "b" [ a ];
  h1 = fake "h1" [
    a
    b
  ];
  c = fake "c" [ h1 ];
  h2 = fake "h2" [
    c
    b
  ];
  d = fake "d" [ ];
  run = fake "run" [ ];
  aggregate = fake "aggregate" [ ];
  heavy = {
    zeta = h1;
    alpha = h2;
  };
  entry = kind: u: {
    inherit kind;
    drv = plain u.drvPath;
  };
  index = {
    "release/libs/a" = entry "libs" a;
    "release/libs/b" = entry "libs" b;
    "release/libs/c" = entry "libs" c;
    "release/libs/d" = entry "libs" d;
    "release/bins/h1" = entry "bins" h1;
    "test/testBins/h2" = entry "testBins" h2;
    "test/testRuns/x" = entry "testRuns" run;
    "aggregate/rust-tests" = entry "aggregate" aggregate;
  };
  # Two heavy units built from each other.
  x = fake "x" [ y ];
  y = fake "y" [ x ];
  cycleRefused =
    !(builtins.tryEval (
      builtins.deepSeq (map (s: s.name) (sizing.heavyPlan { inherit x y; }).steps) true
    )).success;
  battery = pkgs.callPackage ../nix/gate.nix {
    inherit system sizing heavy;
    units = index;
    name = "ronoctua-fake-battery";
    what = "targets";
    targets = [ "one-check" ];
  };
  batteryFacts = pkgs.writeText "battery-facts.json" (
    builtins.toJSON {
      drv = lib.mapAttrs (_: u: plain u.drvPath) {
        inherit
          a
          b
          c
          d
          h1
          h2
          run
          aggregate
          ;
      };
      inherit cycleRefused;
    }
  );

  # THE MACHINE THAT IS BUILT: the reference system, its phases and what the
  # flake hands its apply.
  noctua = self.nixosConfigurations.noctua.config;
  releaseHeavy = lib.filterAttrs (_: u: u.passthru.flavour == "release") rust.heavyUnits;
  machineFacts = pkgs.writeText "machine-phases-facts.json" (
    builtins.toJSON {
      phases = noctua.ronoctua.machine.phases;
      system = plain noctua.system.build.toplevel.drvPath;
      heavy = lib.mapAttrs (_: u: plain u.drvPath) releaseHeavy;
      # What each heavy unit is built from, read off its units.
      below = lib.mapAttrs (_: u: map (x: plain x.drvPath) u.passthru.unitDeps) releaseHeavy;
      kept = rust.release.keep.paths;
      shellOut = plain rust.release.bins.ronoctua-shell.outPath;
      shellMetrics = plain rust.release.bins.ronoctua-shell.metrics.outPath;
      # What must never be kept: the loop's and the test graph's units.
      neverKept = map (u: plain u.outPath) (
        lib.attrValues rust.loop.libs
        ++ lib.attrValues rust.loop.bins
        ++ lib.attrValues rust.loop.deps
        ++ lib.attrValues rust.test.libs
      );
      sizing = noctua.ronoctua.machine.environment.RONOCTUA_BUILD_SIZING;
    }
  );
in
{
  # THE ARITHMETIC, on machines planted around the program.
  #
  # `ronoctua-build-sizing` measures `/proc/meminfo`, the processors and the
  # ceiling of where the builds run; `--meminfo`, `--cpus` and `--ceiling`
  # stand in for each. What it answers is checked against the rule written
  # out again here — the slots are what the budget holds of one compiler's
  # peak, never past the processors; the Rust units take every slot one core
  # each; everything else at most four with the slots split among them; a
  # heavy unit its own compiler and what each core more adds, inside the
  # budget — on a small machine, this one, a large one, and the large one
  # under a ceiling. And what it cannot read is refused with a sentence,
  # never sized by a default: a meminfo without the two lines, a count of
  # processors that is not one, a ceiling that is not bytes. The cgroup's
  # ceiling (root's builds run in its own) is the smallest limit from the
  # cgroup up, asked of planted cgroup folders.
  build-sizing-fits-the-machine =
    mkCheck "build-sizing-fits-the-machine"
      [
        pkgs.python3
        pkgs.bash
      ]
      ''
        python3 - <<'END'
        import json, os, pathlib, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the program's answers, `name=value` lines, and the numbers
        # it was built with, as JSON; nothing is searched.
        PROGRAM = "${sizing.program}/bin/ronoctua-build-sizing"
        FUNCTIONS = "${sizing.functions}"
        N = json.loads('${numbersJson}')
        tmp = pathlib.Path(os.environ["TMPDIR"])
        wrong = []

        def meminfo(name, total_kb, available_kb):
            p = tmp / name
            p.write_text(f"MemTotal:       {total_kb} kB\nMemFree:         1 kB\nMemAvailable:   {available_kb} kB\n")
            return str(p)

        def ask(*args):
            ran = subprocess.run([PROGRAM, *args], capture_output=True, text=True)
            return ran.returncode, dict(l.split("=", 1) for l in ran.stdout.splitlines() if "=" in l), ran.stderr

        def expected(total, available, cpus, ceiling_b):
            # THE RULE, WRITTEN OUT AGAIN — a second spelling to hold the
            # first to.
            floor_kb = total * N["floorPercent"] // 100
            reserve = N["reserveMiB"] * 1024
            peak = N["peakMiB"] * 1024
            shell = N["shellMiB"] * 1024
            beside = N["besideMiB"] * 1024
            budget = available - floor_kb - reserve
            if ceiling_b is not None:
                budget = min(budget, max(0, ceiling_b // 1024 - reserve))
            budget = max(0, budget)
            slots = 1 if budget < peak else budget // peak
            slots = min(slots, cpus)
            jobs = min(slots, N["jobsCeiling"])
            cores = max(1, slots // jobs)
            heavy = 1 if budget <= shell else 1 + (budget - shell) // beside
            return {"budget_kb": budget, "slots": slots, "rust_jobs": slots, "rust_cores": 1,
                    "jobs": jobs, "cores": cores, "heavy_cores": min(heavy, cpus), "cpus": cpus}

        GIB = 1024 * 1024
        machines = [
            ("small", 4 * GIB, 2 * GIB, 4, None),
            ("this", 14967384, 7280824, 12, None),
            ("large", 64 * GIB, 60 * GIB, 16, None),
            ("large-under-a-ceiling", 64 * GIB, 60 * GIB, 16, 8 * GIB * 1024),
        ]
        for name, total, available, cpus, ceiling in machines:
            code, got, err = ask("--meminfo", meminfo(name, total, available), "--cpus", str(cpus),
                                 "--ceiling", "infinity" if ceiling is None else str(ceiling))
            if code != 0:
                wrong.append(f"{name}: the program failed ({code}): {err.strip()}")
                continue
            want = expected(total, available, cpus, ceiling)
            for k, v in want.items():
                if int(got.get(k, "-1")) != v:
                    wrong.append(f"{name}: {k} is {got.get(k)}, the rule says {v}")
            s, j, c, h = (int(got[k]) for k in ("slots", "jobs", "cores", "heavy_cores"))
            budget = int(got["budget_kb"])
            if j * c > s or j > N["jobsCeiling"] or s > cpus or s < 1:
                wrong.append(f"{name}: {j} jobs of {c} cores in {s} slots and {cpus} processors")
            if s > 1 and s * N["peakMiB"] * 1024 > budget:
                wrong.append(f"{name}: {s} compilers of {N['peakMiB']} MiB do not fit {budget // 1024} MiB")
            if h > 1 and (N["shellMiB"] + (h - 1) * N["besideMiB"]) * 1024 > budget:
                wrong.append(f"{name}: a heavy unit with {h} cores does not fit {budget // 1024} MiB")
            if ceiling is not None and budget > ceiling // 1024:
                wrong.append(f"{name}: the budget {budget} kB is past the ceiling")
        # THE SAME MACHINE, SMALLER UNDER ITS CEILING, AND NEVER LARGER.
        _, free, _ = ask("--meminfo", meminfo("c1", 64 * GIB, 60 * GIB), "--cpus", "16", "--ceiling", "infinity")
        _, held, _ = ask("--meminfo", meminfo("c2", 64 * GIB, 60 * GIB), "--cpus", "16", "--ceiling", str(8 * GIB * 1024))
        if not int(held["slots"]) < int(free["slots"]):
            wrong.append(f"a ceiling of 8 GiB did not shrink the slots ({free['slots']} -> {held['slots']})")
        if int(held["heavy_cores"]) >= int(free["heavy_cores"]):
            wrong.append("a ceiling did not shrink the heavy cores")

        # WHAT IT CANNOT READ IS REFUSED.
        bad = tmp / "no-available"
        bad.write_text("MemTotal: 1000 kB\n")
        refusals = [
            (("--meminfo", str(bad), "--cpus", "4", "--ceiling", "infinity"), "MemAvailable"),
            (("--meminfo", meminfo("ok", GIB, GIB), "--cpus", "0", "--ceiling", "infinity"), "processors"),
            (("--meminfo", meminfo("ok2", GIB, GIB), "--cpus", "4", "--ceiling", "lots"), "ceiling"),
        ]
        for args, word in refusals:
            code, got, err = ask(*args)
            if code == 0 or got or word not in err:
                wrong.append(f"{args} answered {code} {got} {err.strip()!r}: not a refusal naming {word}")
        code, _, _ = ask("--bogus")
        if code != 2:
            wrong.append(f"an unknown argument answered {code}, not 2")

        # THE CGROUP'S CEILING: the smallest limit from the cgroup up.
        fs = tmp / "cgroup"
        (fs / "a/b").mkdir(parents=True)
        (fs / "a/memory.high").write_text("8589934592\n")
        (fs / "a/memory.max").write_text("max\n")
        (fs / "a/b/memory.high").write_text("max\n")
        (fs / "a/b/memory.max").write_text("4294967296\n")
        (fs / "c").mkdir()
        (fs / "c/memory.high").write_text("max\n")
        def cgroup(own):
            ran = subprocess.run(["bash", "-c", f'. "{FUNCTIONS}"; sizing_cgroup_ceiling "{fs}" "{own}"'],
                                 capture_output=True, text=True)
            return ran.stdout.strip()
        for own, want in (("/a/b", "4294967296"), ("/a", "8589934592"), ("/c", ""), ("/", "")):
            if cgroup(own) != want:
                wrong.append(f"the ceiling of {own} is {cgroup(own)!r}, not {want!r}")

        floor(len(machines), 4, "machines sized")
        if wrong:
            print("ERROR: the build sizing does not fit the machine it measures:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(machines)} machines sized by the rule, a ceiling shrinks the budget, three unreadable answers refused, and the cgroup's ceiling is its smallest limit")
        END
      '';

  # THE BATTERIES BUILD THE HEAVY UNITS ALONE, after what they are built from.
  #
  # `nix/gate.nix` over planted units: two heavy ones (the second built from
  # a unit that reaches the first, and named so the alphabet would put it
  # first), the units below them, one more, a test run and its aggregate.
  # Run with a `nix` that writes down what it was asked, the battery must
  # build, in this order: what the heavy units are built from that reaches
  # none of them, as Rust units (`--max-jobs <slots> --cores 1`); the first
  # heavy unit alone (`--max-jobs 1 --cores <heavy>`); the unit between,
  # then the second heavy unit alone; every other unit as Rust units; the
  # test runs and the aggregate after every compilation, sized as everything
  # else; and only then the checks. Every build other than a heavy one keeps
  # jobs × cores within the slots the battery said. And two heavy units built
  # from each other are refused at evaluation.
  batteries-build-the-heavy-alone =
    mkCheck "batteries-build-the-heavy-alone"
      [
        pkgs.python3
        pkgs.git
      ]
      ''
        mkdir -p bin
        cat > bin/nix <<'EOF'
        #!/bin/sh
        printf '%s\n' "$*" >> "$LOG"
        case "$1" in
          eval)
            case "$*" in
              *memorySize*) echo '[{"n":"one-check","mib":0,"cores":0,"nodes":0}]' ;;
              *) echo '[{"n":"one-check","drv":"/nix/store/00000000000000000000000000000000-one-check.drv","driver":null,"out":null}]' ;;
            esac ;;
        esac
        exit 0
        EOF
        chmod +x bin/nix
        export LOG=$PWD/calls PATH=$PWD/bin:$PATH HOME=$PWD
        ${battery}/bin/ronoctua-fake-battery > said.txt 2>&1 || { cat said.txt >&2; exit 1; }
        python3 - <<'END'
        import json, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: what the battery said (its sizing line) and the `nix`
        # command lines it ran, word by word.
        facts = json.load(open("${batteryFacts}"))
        D = facts["drv"]
        said = open("said.txt").read()
        m = re.search(r"(\d+) compilations fit at once -> the Rust units: --max-jobs (\d+) --cores (\d+);"
                      r"  heavy: --max-jobs 1 --cores (\d+);  the rest: --max-jobs (\d+) --cores (\d+)", said)
        must(m and m.group(0), "the battery's sizing line")
        slots, uj, uc, hc, j, c = (int(g) for g in m.groups())
        builds = []
        for line in open("calls").read().splitlines():
            w = line.split()
            if w[0] != "build":
                continue
            builds.append({
                "link": w[w.index("--out-link") + 1].rsplit("/", 1)[-1],
                "jobs": int(w[w.index("--max-jobs") + 1]),
                "cores": int(w[w.index("--cores") + 1]),
                "drvs": {x[:-2] for x in w if x.endswith("^*")},
            })
        floor(len(builds), 6, "builds the battery ran")
        name = {v: k for k, v in D.items()}
        steps = [(b["link"], sorted(name.get(x, x.split("-", 1)[-1]) for x in b["drvs"]), b["jobs"], b["cores"]) for b in builds]
        want = [
            ("first", ["a", "b"], uj, uc),
            ("heavy-0", ["h1"], 1, hc),
            ("heavy-1-below", ["c"], uj, uc),
            ("heavy-1", ["h2"], 1, hc),
            ("units", ["a", "b", "c", "d"], uj, uc),
            ("runs", ["aggregate", "run"], j, c),
            ("rest", ["one-check.drv"], j, c),
        ]
        wrong = []
        if steps != want:
            wrong.append("the battery built\n    " + "\n    ".join(map(str, steps)) + "\n  and not\n    " + "\n    ".join(map(str, want)))
        if (uj, uc) != (slots, 1):
            wrong.append(f"the Rust units are {uj} jobs of {uc} cores, not the {slots} slots one core each")
        for b in builds:
            if b["jobs"] != 1 and b["jobs"] * b["cores"] > slots:
                wrong.append(f"{b['link']}: {b['jobs']} jobs of {b['cores']} cores in {slots} slots")
        if not facts["cycleRefused"]:
            wrong.append("two heavy units built from each other were not refused at evaluation")
        if wrong:
            print("ERROR: the batteries do not build the heavy units alone:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: below, the heavy units alone in their order, the rest as Rust units, the test runs last ({slots} slots, heavy with {hc} cores)")
        END
      '';

  # WHAT `ronoctua-keep-units` ROOTS IS WHAT THE STORE HOLDS, AND ONLY THE
  # CURRENT SET.
  #
  # Over a `nix-store` that answers from a planted list of valid paths and
  # refuses — as a build would be — to root a path it does not hold: of
  # three candidates two valid, it roots those two and asks for nothing
  # else; asked again with the same, it roots nothing twice; when the third
  # becomes valid it makes a new generation of three and only then removes
  # the old; a path collected between the question and the root leaves the
  # last generation standing; another set is its own; a generation nobody
  # kept in the days given is released, one kept is not; and a set name or
  # a path that is not one is refused.
  kept-units-are-the-ones-the-store-holds =
    let
      fakeStore = pkgs.writeShellScriptBin "nix-store" ''
        echo "$*" >> "$FAKE_LOG"
        case "$1" in
          --check-validity)
            shift
            [ "$1" = --print-invalid ] && shift
            for p in "$@"; do grep -qxF -- "$p" "$FAKE_VALID" || echo "$p"; done
            ;;
          --realise)
            shift
            root=""
            while [ $# -gt 0 ]; do
              case "$1" in
                --add-root) root=$2; shift 2 ;;
                --indirect) shift ;;
                *) break ;;
              esac
            done
            n=0
            for p in "$@"; do
              if ! grep -qxF -- "$p" "$FAKE_REALISABLE"; then
                echo "WOULD BUILD $p" >> "$FAKE_LOG"
                exit 1
              fi
              n=$((n + 1))
              link=$root
              [ "$n" -gt 1 ] && link="$root-$n"
              ln -s "$p" "$link"
            done
            ;;
          --query)
            shift
            [ "$1" = --size ] && shift
            for p in "$@"; do echo 1048576; done
            ;;
        esac
      '';
      keep = pkgs.callPackage ../nix/keep-units.nix { nix = fakeStore; };
    in
    mkCheck "kept-units-are-the-ones-the-store-holds" [ pkgs.python3 ] ''
      python3 - <<'END'
      import os, pathlib, subprocess, sys, time
      exec(open("${pyPrelude}").read())

      # PARSED: the roots the program left, link by link, and what the fake
      # store was asked.
      # FILE-WIDE: the fake store's log is read whole: the question is every
      # call it got.
      KEEP ="${keep}/bin/ronoctua-keep-units"
      tmp = pathlib.Path(os.environ["TMPDIR"])
      roots = tmp / "roots"
      valid, realisable, log = tmp / "valid", tmp / "realisable", tmp / "log"
      P = [f"/nix/store/{c * 32}-unit-{c}" for c in "abc"]
      cands = tmp / "candidates"
      cands.write_text("\n".join(P) + "\n")
      env = dict(os.environ, FAKE_VALID=str(valid), FAKE_REALISABLE=str(realisable), FAKE_LOG=str(log))
      wrong = []

      def keep(*args, set_="release"):
          return subprocess.run([KEEP, "--roots", str(roots), "--set", set_, *args, str(cands)],
                                env=env, capture_output=True, text=True)

      def generations(set_="release"):
          return sorted(p for p in roots.glob(set_ + "@*") if p.is_dir())

      def held(gen):
          return sorted(os.readlink(l) for l in gen.iterdir())

      def asked(word):
          return [l for l in log.read_text().splitlines() if l.startswith(word)] if log.exists() else []

      # 1. TWO OF THREE IN THE STORE: those two, and nothing built.
      valid.write_text(P[0] + "\n" + P[2] + "\n")
      realisable.write_text(valid.read_text())
      ran = keep()
      gens = generations()
      if ran.returncode != 0 or len(gens) != 1 or held(gens[0]) != [P[0], P[2]]:
          wrong.append(f"two valid of three: {ran.returncode} {ran.stderr.strip()!r} {[held(g) for g in gens]}")
      if any("WOULD BUILD" in l for l in log.read_text().splitlines()):
          wrong.append("it asked for a path the store does not hold")
      if "kept 2 of 3 paths" not in ran.stdout:
          wrong.append(f"it did not say what it kept: {ran.stdout!r}")
      # 2. THE SAME AGAIN: nothing rooted twice.
      before = len(asked("--realise"))
      keep()
      if len(asked("--realise")) != before or generations() != gens:
          wrong.append("the same set was rooted again")
      # 3. THE THIRD ARRIVES: a new generation of three, the old one gone.
      time.sleep(1)
      valid.write_text("\n".join(P) + "\n")
      realisable.write_text(valid.read_text())
      keep()
      now = generations()
      if len(now) != 1 or now == gens or held(now[0]) != P:
          wrong.append(f"a new set did not replace the old: {[str(g) for g in now]}")
      # 4. COLLECTED BETWEEN THE QUESTION AND THE ROOT: the last one stays.
      realisable.write_text(P[0] + "\n")
      valid.write_text(P[0] + "\n" + P[1] + "\n")
      ran = keep()
      if ran.returncode == 0 or generations() != now or [held(g) for g in generations()] != [P]:
          wrong.append(f"a failed root removed the last set: {ran.returncode} {[held(g) for g in generations()]}")
      # 5. ANOTHER SET IS ITS OWN, AND A STALE ONE IS RELEASED.
      realisable.write_text("\n".join(P) + "\n")
      valid.write_text(P[1] + "\n")
      old = roots / "units-gone@20200101T000000-1"
      old.mkdir()
      os.utime(old, (time.time() - 9 * 86400,) * 2)
      ran = keep("--release-days", "7", set_="loop")
      if [held(g) for g in generations("loop")] != [[P[1]]]:
          wrong.append(f"another set: {[held(g) for g in generations('loop')]}")
      if [held(g) for g in generations()] != [P]:
          wrong.append(f"rooting another set touched this one: {[held(g) for g in generations()]}")
      if old.exists() or "released units-gone@" not in ran.stderr:
          wrong.append(f"a set nobody kept in 9 days was not released: {ran.stderr!r}")
      if not generations():
          wrong.append("a set kept today was released")
      # 6. REFUSALS.
      for args, set_ in ((("--release-days", "soon"), "release"), ((), "../escape"), ((), "")):
          if keep(*args, set_=set_).returncode != 2:
              wrong.append(f"{args} {set_!r} was not refused")
      cands.write_text("/etc/passwd\n")
      if keep().returncode != 2:
          wrong.append("a path outside the store was not refused")

      floor(len(P), 3, "candidates")
      if wrong:
          print("ERROR: ronoctua-keep-units does not keep what the store holds, and only the current set:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print("OK: the valid units rooted and no other, a set replaced whole and never emptied, a stale one released")
      END
    '';

  # `.#rust-peaks` NAMES WHAT PEAKED PAST THE SLOT.
  #
  # Over planted measurements: a light unit past the slot fails it and is
  # the one named; a heavy unit past it does not; a heavy unit under the
  # slot and one past what a heavy unit is planned at are said; a heavy
  # unit never built is "not measured"; a unit with no measurement, or a
  # measurement that is not there, is not counted. With the light unit back
  # under the slot it passes.
  rust-peaks-names-what-is-past-the-slot =
    mkCheck "rust-peaks-names-what-is-past-the-slot" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import json, os, pathlib, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the report, as JSON and as the lines it prints.
        tmp = pathlib.Path(os.environ["TMPDIR"])
        def measured(name, mib, seconds=1.5):
            d = tmp / name
            d.mkdir(exist_ok=True)
            (d / "peak-kib").write_text(f"{mib * 1024}\n")
            (d / "seconds").write_text(f"{seconds}\n")
            return str(d)
        units = {
            "release/libs/small": {"kind": "libs", "flavour": "release", "metrics": measured("small", 500)},
            "release/deps/big": {"kind": "deps", "flavour": "release", "metrics": measured("big", 2000)},
            "release/bins/shell": {"kind": "bins", "flavour": "release", "metrics": measured("shell", 7000)},
            "test/clippy/shell": {"kind": "clippy", "flavour": "test", "metrics": measured("clippy", 1000)},
            "test/testBins/shell": {"kind": "testBins", "flavour": "test", "metrics": None},
            "fmt/x": {"kind": "fmt", "flavour": None, "metrics": None},
            "loop/libs/gone": {"kind": "libs", "flavour": "loop", "metrics": str(tmp / "never-built")},
        }
        facts = {
            "units": units,
            "heavy": {"shell-release": "release/bins/shell", "shell-clippy": "test/clippy/shell",
                      "shell-test-harness": "test/testBins/shell"},
            "peakKiB": 1792 * 1024, "shellKiB": 5632 * 1024, "besideKiB": 1024 * 1024,
        }
        f = tmp / "facts.json"
        f.write_text(json.dumps(facts))
        SCRIPT = "${../nix/scripts/rust-peaks.py}"
        def ask(*args):
            return subprocess.run(["python3", SCRIPT, *args, str(f)], capture_output=True, text=True)
        wrong = []
        ran = ask("--json")
        r = json.loads(ran.stdout)
        if ran.returncode != 1:
            wrong.append(f"a light unit past the slot answered {ran.returncode}, not 1")
        if r["past-the-slot"] != ["release/deps/big"] or "release/deps/big" not in ran.stderr:
            wrong.append(f"past the slot: {r['past-the-slot']} / {ran.stderr!r}")
        if r["heavy-under-the-slot"] != ["shell-clippy"] or r["heavy-past-the-plan"] != ["shell-release"]:
            wrong.append(f"the heavy units: {r['heavy-under-the-slot']} {r['heavy-past-the-plan']}")
        if r["heavy-unmeasured"] != ["shell-test-harness"] or len(r["measured"]) != 4:
            wrong.append(f"measured {sorted(r['measured'])}, unmeasured heavy {r['heavy-unmeasured']}")
        text = ask().stdout
        for needle in ("measured: 4 of 7 units", "shell-test-harness: test/testBins/shell: not measured",
                       "heavy: shell-release", "note: shell-clippy is heavy and measured under the slot"):
            if needle not in text:
                wrong.append(f"the report does not say {needle!r}:\n{text}")
        units["release/deps/big"]["metrics"] = measured("big", 1000)
        f.write_text(json.dumps(facts))
        if ask().returncode != 0:
            wrong.append("with every light unit under the slot it still failed")
        floor(len(units), 7, "planted units")
        if wrong:
            print("ERROR: .#rust-peaks does not say what peaked past the slot:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the light unit past the slot is named and fails it; the heavy ones are said, measured or not")
        END
      '';

  # A MACHINE'S APPLY BUILDS BY THE DISTRO'S PLAN, AND KEEPS ITS UNITS.
  #
  # The reference system's `ronoctua.machine.phases`, as `nix eval` gives it
  # to `ronoctua-machine apply`: the release graph's heavy units and nothing
  # else (the shell's program and the footer's library), each after every
  # unit it is built from; the system's own derivation; and the units to
  # keep — every output of the release set (`metrics` beside `out`, or the
  # batteries' `^*` builds a kept unit again once a collection took its
  # `metrics`), the shell's program among them, no unit of the loop or the
  # test graph. Then the EXTRACTED `ronoctua-machine` (the one the units run
  # as root) over that very JSON, with the machine's own sizing program and
  # a `nix` that writes down what it was asked and says, for a dry run of
  # the system, the plan planted. With the shell's whole graph planned it
  # builds what the heavy units are built from as Rust units, then each heavy
  # unit alone with the heavy cores the sizing said, in its order and after
  # the units between, the system sized as everything else; with a store
  # that holds the shell's bundle (the seed's, the person's cache's) it
  # builds the system and not one unit, though every unit is in the system's
  # `.drv` closure. Both hand the release set to what keeps it — once the
  # build worked.
  machine-builds-by-the-distros-plan =
    mkCheck "machine-builds-by-the-distros-plan" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import json, os, pathlib, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the evaluated phases and facts, as JSON; the program's
        # format, its one `pub const` line; and what the fakes were asked.
        # FILE-WIDE: the constant is the crate's, in a file read whole.
        facts = json.load(open("${machineFacts}"))
        p = facts["phases"]
        crate = uncommented(pathlib.Path("${../crates/ronoctua-machine/src/build.rs}").read_text())
        fmt = must(next((l for l in crate.splitlines() if l.startswith("pub const PHASES_FORMAT: u32 = ")), None),
                   "PHASES_FORMAT in ronoctua-machine's build.rs")
        wrong = []
        if str(p["format"]) != fmt.rsplit("= ", 1)[1].rstrip(";"):
            wrong.append(f"the module says format {p['format']}, the program reads {fmt}")
        names = [s["name"] for s in p["heavy"]]
        if sorted(names) != sorted(facts["heavy"]) or [s["drv"] for s in p["heavy"]] != [facts["heavy"][n] for n in names]:
            wrong.append(f"the heavy steps are {names}, the release graph's heavy units {sorted(facts['heavy'])}")
        heavy = set(facts["heavy"].values())
        first = set(p["first"])
        floor(len(first), 100, "units the shell's program is built from")
        floor(len(p["heavy"]), 2, "heavy units of the release graph")
        for n, below in facts["below"].items():
            missing = set(below) - first - heavy - {d for s in p["heavy"] for d in s["between"]}
            if missing:
                wrong.append(f"{n} is built from {len(missing)} unit(s) no phase names, e.g. {sorted(missing)[:3]}")
        if first & heavy:
            wrong.append("a heavy unit is among what is built first")
        if p["system"] != facts["system"]:
            wrong.append("the system's derivation is not the system's")
        kept = p["kept"]
        floor(len(kept), 300, "units kept")
        if kept != facts["kept"] or facts["shellOut"] not in kept:
            wrong.append("the units to keep are not the release set with the shell's program")
        if facts["shellMetrics"] not in kept:
            wrong.append("the units to keep hold a unit's out and not its metrics: a collection takes "
                         "them, and a battery's ^* compiles the kept unit again")
        stray = set(kept) & set(facts["neverKept"])
        if stray:
            wrong.append(f"{len(stray)} unit(s) of the loop or the test graph would be kept, e.g. {sorted(stray)[:2]}")

        # THE PROGRAM, OVER THAT JSON, in two worlds: Nix's plan (its dry
        # run of the system, what the fake `nix` says it) building the
        # shell's whole graph, and the plan of a machine whose store holds
        # the shell's bundle — freshly installed from the seed, or fed by the
        # person's cache — where Nix builds the system and nothing below it.
        MACHINE = "${rust.machine}/bin/ronoctua-machine"
        PY = "${pkgs.python3}/bin/python3"
        shell_drv = facts["heavy"]["shell-release"]
        def plan_of(drvs):
            return f"these {len(drvs)} derivations will be built:\n" + "".join(f"  {d}\n" for d in drvs)
        def apply(name, plan):
            root = pathlib.Path(os.environ["TMPDIR"]) / name
            store = root / "nix/store"
            store.mkdir(parents=True)
            def program(path, text):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("#!" + PY + "\n" + text)
                path.chmod(0o755)
            (root / "phases.json").write_text(json.dumps(p))
            (root / "plan").write_text(plan)
            program(root / "bin/nix", """
        import os, pathlib, sys
        root = pathlib.Path(os.environ["FAKE_ROOT"])
        a = sys.argv[1:]
        with open(root / "calls", "a") as f:
            f.write("nix " + " ".join(a) + "\\n")
        if a[0] == "eval":
            print((root / "phases.json").read_text())
        elif a[0] == "build" and "--dry-run" in a:
            sys.stderr.write((root / "plan").read_text())
        elif a[0] == "build":
            out = a[a.index("--out-link") + 1]
            os.makedirs(os.path.dirname(out), exist_ok=True)
            if os.path.lexists(out):
                os.remove(out)
            os.symlink((root / "build-result").read_text(), out)
        """)
            program(root / "bin/nix-env", """
        import os, pathlib, sys
        a = sys.argv[1:]
        root = pathlib.Path(os.environ["FAKE_ROOT"])
        p = pathlib.Path(a[a.index("--profile") + 1])
        os.symlink(str(root) + a[a.index("--set") + 1], p.parent / "system-1-link")
        os.symlink("system-1-link", p)
        """)
            # THE MACHINE'S OWN SIZING, and what it said to the apply written
            # down: memory moves between two runs, and the apply is held to
            # the numbers it was given.
            program(root / "bin/sizing", f"""
        import pathlib, subprocess, sys
        said = subprocess.run(["{facts['sizing']}"], capture_output=True, text=True)
        pathlib.Path("{root}/sized").write_text(said.stdout)
        sys.stdout.write(said.stdout)
        sys.exit(said.returncode)
        """)
            program(root / "bin/keep-units", """
        import os, pathlib, shutil, sys
        root = pathlib.Path(os.environ["FAKE_ROOT"])
        with open(root / "calls", "a") as f:
            f.write("keep-units " + " ".join(sys.argv[1:]) + "\\n")
        shutil.copy(sys.argv[-1], root / "kept")
        """)
            top = store / ("b" * 32 + "-nixos-system-b")
            (top / "bin").mkdir(parents=True)
            (top / "kernel").write_text("k")
            (top / "initrd").write_text("i")
            (top / "kernel-params").write_text("quiet")
            program(top / "bin/switch-to-configuration", "import sys\n")
            (root / "build-result").write_text(str(top))
            (root / "nix/var/nix/profiles").mkdir(parents=True)
            (root / "run").mkdir()
            (root / "machine").mkdir()
            (root / "machine/flake.nix").write_text("{ }\n")
            (root / "machine/flake.lock").write_text("{}\n")
            (root / "proc/sys/kernel/random").mkdir(parents=True)
            (root / "proc/sys/kernel/random/boot_id").write_text("boot-1\n")
            env = {
                "PATH": "/no-path",
                "FAKE_ROOT": str(root),
                "RONOCTUA_MACHINE_SYSROOT": str(root),
                "RONOCTUA_MACHINE_DIR": str(root / "machine"),
                "RONOCTUA_MACHINE_STATE": str(root / "state"),
                "RONOCTUA_MACHINE_RUN": str(root / "run/ronoctua-update"),
                "RONOCTUA_MACHINE_ATTR": "noctua",
                "RONOCTUA_MACHINE_MENU_LIMIT": "5",
                "RONOCTUA_NIX": str(root / "bin/nix"),
                "RONOCTUA_BOOTCTL": "/no-bootctl",
                "RONOCTUA_BUILD_SIZING": str(root / "bin/sizing"),
                "RONOCTUA_KEEP_UNITS": str(root / "bin/keep-units"),
            }
            ran = subprocess.run([MACHINE, "apply"], env=env, capture_output=True, text=True)
            sized = dict(l.split("=", 1) for l in (root / "sized").read_text().splitlines()) if (root / "sized").exists() else {}
            floor(len(sized), 10, "numbers the machine's sizing said")
            calls = (root / "calls").read_text().splitlines() if (root / "calls").exists() else []
            asked = [c.split() for c in calls if c.startswith("nix build") and "--dry-run" in c.split()]
            builds = [c.split() for c in calls if c.startswith("nix build") and "--dry-run" not in c.split()]
            kept_file = root / "kept"
            if [w[-1] for w in asked] != [p["system"] + "^*"]:
                wrong.append(f"{name}: Nix was not asked once what the system's build builds: {asked}")
            if not kept_file.exists() or kept_file.read_text().split() != kept:
                wrong.append(f"{name}: what keeps the units was not handed the release set")
            return ran, sized, builds
        def sizes(w):
            return w[w.index("--max-jobs") + 1], w[w.index("--cores") + 1]
        def drvs_of(w):
            return {x[:-2] for x in w if x.endswith("^*")}

        # The shell changed: everything below it as Rust units, each heavy
        # unit alone with the heavy cores (after the units between, as Rust
        # units), the system last as everything else.
        steps = p["heavy"]
        between = [d for s in steps for d in s["between"]]
        ran, sized, builds = apply("changed", plan_of(sorted(first) + between + [s["drv"] for s in steps] + [p["system"]]))
        shell = next((w for w in builds if shell_drv + "^*" in w), None)
        system_built = next((w for w in builds if w[-1] == p["system"] + "^*"), None)
        expected = 2 + sum(2 if s["between"] else 1 for s in steps)
        if len(builds) != expected:
            wrong.append(f"changed: the apply ran {len(builds)} builds, not {expected}: {[w[:12] for w in builds]} / {ran.stderr[-2000:]}")
        else:
            if sizes(builds[0]) != (sized["rust_jobs"], sized["rust_cores"]) or drvs_of(builds[0]) != first:
                wrong.append(f"changed: the first build is not every unit the shell is built from, as Rust units: {builds[0][:12]}")
            at = 1
            for s in steps:
                if s["between"]:
                    w = builds[at]
                    if sizes(w) != (sized["rust_jobs"], sized["rust_cores"]) or drvs_of(w) != set(s["between"]):
                        wrong.append(f"changed: what {s['name']} is built from after a heavy unit was not built before it, as Rust units: {w[:12]}")
                    at += 1
                w = builds[at]
                if sizes(w) != ("1", sized["heavy_cores"]) or drvs_of(w) != {s["drv"]}:
                    wrong.append(f"changed: {s['name']} was not built alone, in its order, with {sized['heavy_cores']} cores: {w[:12]}")
                at += 1
            if shell is None or sizes(shell) != ("1", sized["heavy_cores"]) or drvs_of(shell) != {shell_drv}:
                wrong.append(f"changed: the shell's program was not built alone with {sized['heavy_cores']} cores: {shell}")
            if system_built is None or sizes(system_built) != (sized["jobs"], sized["cores"]) or builds[-1] != system_built:
                wrong.append(f"changed: the system was not built last, sized as the rest: {system_built}")

        # The shell's bundle in the store, as the seed leaves it: every unit
        # is in the system's `.drv` closure, and none is built.
        ran, sized, builds = apply("fresh", f"this derivation will be built:\n  {p['system']}\n")
        if [drvs_of(w) for w in builds] != [{p["system"]}] or sizes(builds[0]) != (sized["jobs"], sized["cores"]):
            wrong.append(
                f"fresh: a store holding the shell built {sum(len(drvs_of(w)) for w in builds)} derivation(s) in "
                f"{len(builds)} build(s), not the system alone: {[w[:12] for w in builds]} / {ran.stderr[-2000:]}"
            )
        if wrong:
            print("ERROR: a machine's apply does not build by the distro's plan:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(first)} units first, {len(steps)} heavy ones alone, the system last when Nix plans them; "
              f"the system alone when the store holds the shell; {len(kept)} paths kept")
        END
      '';

  # `.#sized-build` BUILDS WHAT NIX WOULD BUILD, SIZED.
  #
  # `ronoctua-sized-build` over the two planted heavy units above (the second
  # built from a unit that reaches the first), with a `nix` that writes down
  # every build it is handed and answers a dry run with the plan planted.
  # With the whole graph planned: what reaches no heavy unit as Rust units,
  # the first heavy unit alone, the unit between, the second alone, then what
  # it was given. With a store that holds the heavy units' consumer: only
  # what it was given, though every unit is in its `.drv` closure. With the
  # first heavy unit held and the unit between changed: that unit, the
  # second heavy one alone, what it was given. And a dry run that does not
  # answer, or answers in words it does not read, builds nothing.
  sized-build-builds-what-nix-would =
    let
      sized = sizing.sizedBuild heavy;
      fakeNix = pkgs.writeShellScriptBin "nix" ''
        echo "$*" >> "$FAKE_LOG"
        case "$1" in
          path-info) cat "$FAKE_TARGETS" ;;
          build)
            for a in "$@"; do
              if [ "$a" = --dry-run ]; then
                if [ -e "$FAKE_DRY_FAILS" ]; then echo "error: cannot open the store" >&2; exit 1; fi
                cat "$FAKE_PLAN" >&2
                exit 0
              fi
            done
            ;;
        esac
      '';
    in
    mkCheck "sized-build-builds-what-nix-would" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, os, pathlib, re, subprocess, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the planted units' paths, as JSON; what the program said of
      # its sizing, its one line; and every call the fake `nix` got.
      # FILE-WIDE: the fake's log is read whole, and the program's output is
      # searched whole for its sizing line: the question is every build it
      # was handed, at the numbers it said.
      SIZED = "${sized}/bin/ronoctua-sized-build"
      d = json.load(open("${batteryFacts}"))["drv"]
      target = "/nix/store/" + "t" * 32 + "-target.drv"
      tmp = pathlib.Path(os.environ["TMPDIR"])
      wrong = []

      def run(name, plan, fails=False):
          where = tmp / name
          where.mkdir()
          (where / "targets").write_text(target + "\n")
          (where / "plan").write_text(plan)
          if fails:
              (where / "dry-fails").write_text("")
          env = dict(os.environ, PATH="${fakeNix}/bin:" + os.environ["PATH"], FAKE_LOG=str(where / "log"),
                     FAKE_TARGETS=str(where / "targets"), FAKE_PLAN=str(where / "plan"),
                     FAKE_DRY_FAILS=str(where / "dry-fails"))
          ran = subprocess.run([SIZED, "--no-link", ".#anything"], env=env, capture_output=True, text=True)
          calls = (where / "log").read_text().splitlines() if (where / "log").exists() else []
          asked = [c.split() for c in calls if c.startswith("build ") and "--dry-run" in c.split()]
          builds = [c.split() for c in calls if c.startswith("build ") and "--dry-run" not in c.split()]
          return ran, asked, builds

      def plan_of(*drvs):
          return f"these {len(drvs)} derivations will be built:\n" + "".join(f"  {x}\n" for x in drvs)

      def shape(ran, builds):
          """Each build as (its derivations, rust or heavy), read against the
          numbers the program said."""
          m = re.search(r"the Rust units: --max-jobs (\d+) --cores (\d+);  heavy: --max-jobs 1 --cores (\d+);", ran.stdout)
          if not m:
              wrong.append(f"the program did not say its sizing: {ran.stdout[-500:]!r}")
              return []
          sizes = {(m.group(1), m.group(2)): "rust", ("1", m.group(3)): "heavy"}
          out = []
          for w in builds:
              got = (w[w.index("--max-jobs") + 1], w[w.index("--cores") + 1])
              out.append((sorted(x[:-2] for x in w if x.endswith("^*")), sizes.get(got, f"sized {got}")))
          return out

      for name, plan, expected in [
          ("whole", plan_of(d["a"], d["b"], d["h1"], d["c"], d["h2"], target),
           [([d["a"], d["b"]], "rust"), ([d["h1"]], "heavy"), ([d["c"]], "rust"), ([d["h2"]], "heavy"), ([target], "rust")]),
          ("fresh", f"this derivation will be built:\n  {target}\n", [([target], "rust")]),
          ("first-held", plan_of(d["c"], d["h2"], target),
           [([d["c"]], "rust"), ([d["h2"]], "heavy"), ([target], "rust")]),
      ]:
          ran, asked, builds = run(name, plan)
          expected = [(sorted(x), s) for x, s in expected]
          if ran.returncode != 0 or shape(ran, builds) != expected:
              wrong.append(f"{name}: built {shape(ran, builds)}, not {expected} (exit {ran.returncode}: {ran.stderr[-800:]!r})")
          if [w[-1] for w in asked] != [target + "^*"]:
              wrong.append(f"{name}: Nix was not asked once what the build builds: {asked}")
      floor(len(d), 8, "planted units")
      for name, plan, fails in [("unread", "these 3 derivations will be built:\n  /nix/store/x.drv\n", False),
                                ("no-answer", "", True)]:
          ran, _, builds = run(name, plan, fails)
          if ran.returncode == 0 or builds:
              wrong.append(f"{name}: exit {ran.returncode} after {len(builds)} build(s); a plan it cannot read builds nothing")
      if wrong:
          print("ERROR: .#sized-build does not build what Nix would, sized:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print("OK: the whole graph in its phases, the fresh store only what was given, a held heavy unit not again; an unread plan builds nothing")
      END
    '';

  # THE BATTERIES REACH EVERY UNIT, and the short loop is sized too.
  #
  # The gate as the flake builds it: its heavy units are `rust.heavyUnits`,
  # and every derivation of the unit index is built in phase 1 — the
  # compilations before the test runs and the aggregates, which go last. And
  # `.#quick` builds through `.#sized-build`, never a bare `nix build`.
  batteries-reach-every-unit =
    let
      facts = pkgs.writeText "battery-units.json" (
        builtins.toJSON {
          heavy = lib.mapAttrs (_: u: plain u.drvPath) rust.heavyUnits;
          units = lib.mapAttrs (_: u: { inherit (u) kind drv; }) rust.unitIndex;
          # With its context: the script is read in the sandbox.
          quick = self.apps.${system}.quick.program;
        }
      );
    in
    mkCheck "batteries-reach-every-unit" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, pathlib, re, shlex, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the gate's arrays, as the shell reads them (`name=(…)`, one
      # line each); the units as JSON.
      # FILE-WIDE: the quick loop's script is asked whole whether any line of
      # it runs `nix build`, its comments off.
      facts = json.load(open("${facts}"))
      script = pathlib.Path("${gateApp}/bin/ronoctua-gate").read_text()
      def array(name):
          m = re.search(r"^" + name + r"=\((.*)\)$", script, re.M)
          return [w[:-2] for w in shlex.split(must(m and m.group(1), "the gate's " + name))]
      heavy, first, rest, runs = array("heavy"), array("first"), array("units_rest"), array("runs")
      wrong = []
      if sorted(heavy) != sorted(facts["heavy"].values()):
          wrong.append(f"the gate's heavy units are not rust.heavyUnits ({len(heavy)} against {len(facts['heavy'])})")
      units = facts["units"]
      floor(len(units), 1000, "units of the index")
      built = set(heavy) | set(first) | set(rest) | set(runs)
      missing = sorted(i for i, u in units.items() if u["drv"] not in built)
      if missing:
          wrong.append(f"{len(missing)} unit(s) no phase builds, e.g. {missing[:3]}")
      late = {u["drv"] for u in units.values() if u["kind"] in ("testRuns", "aggregate")}
      if set(runs) != late or late & (set(first) | set(rest)):
          wrong.append("the test runs and the aggregates are not what goes last, and only there")
      quick = hashless(pathlib.Path(facts["quick"]).read_text())
      if "/bin/ronoctua-sized-build " not in quick:
          wrong.append("`.#quick` does not build through `.#sized-build`")
      if any(l.strip().startswith("nix build") for l in quick.splitlines()):
          wrong.append("`.#quick` runs a bare `nix build`")
      if wrong:
          print("ERROR: the batteries do not reach the unit graph as sized:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(heavy)} heavy units alone, {len(units)} units in phase 1 ({len(runs)} runs last), and .#quick sized")
      END
    '';
}
