# HOW MANY COMPILATIONS FIT, AND THE UNITS A MACHINE KEEPS — nix-design §4
# and §2 "Kept on the machine" (lane N4), with each unit by its own class
# (lane S1): the arithmetic of `nix/build-sizing.nix`, the scheduler of the
# units (`sized_units`), the batteries' phases, what `ronoctua-keep-units`
# roots, `.#rust-peaks` and the classes it writes, and the plan a machine's
# apply builds by.
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
  # never built.
  fake = name: pkgs.writeText "fake-unit-${name}" name;
  # THE PLANTED GRAPH, shaped as `rust.unitIndex` is, each unit with its
  # class: two heavy units, the second built from a unit that is built from
  # the first; a medium and lights below them; a light and a medium on
  # their own; a test run and its aggregate.
  planted = {
    "release/deps/a" = {
      kind = "deps";
      deps = [ ];
      class = "light";
    };
    "release/libs/b" = {
      kind = "libs";
      deps = [ "release/deps/a" ];
      class = "light";
    };
    "release/libs/m" = {
      kind = "libs";
      deps = [ "release/deps/a" ];
      class = "medium";
    };
    "release/libs/h1" = {
      kind = "libs";
      deps = [
        "release/libs/b"
        "release/libs/m"
      ];
      class = "heavy";
    };
    "release/libs/c" = {
      kind = "libs";
      deps = [ "release/libs/h1" ];
      class = "light";
    };
    "release/bins/h2" = {
      kind = "bins";
      deps = [
        "release/libs/c"
        "release/libs/b"
      ];
      class = "heavy";
    };
    "release/libs/d" = {
      kind = "libs";
      deps = [ ];
      class = "light";
    };
    "release/libs/e" = {
      kind = "libs";
      deps = [ ];
      class = "medium";
    };
    "test/testRuns/x" = {
      kind = "testRuns";
      deps = [ "release/libs/b" ];
      class = "light";
    };
    "aggregate/rust-tests" = {
      kind = "aggregate";
      deps = [ "test/testRuns/x" ];
    };
  };
  index = lib.mapAttrs (id: u: {
    inherit (u) kind deps;
    drv = plain (fake (baseNameOf id)).drvPath;
  }) planted;
  plantedClasses = lib.mapAttrs (_: u: u.class) (lib.filterAttrs (_: u: u ? class) planted);
  plantedPlan = sizing.unitPlan {
    units = index;
    classes = plantedClasses;
  };
  # A battery's one check, as the fake `nix` evaluates it.
  oneCheck = "/nix/store/00000000000000000000000000000000-one-check.drv";
  # What a sized build is given, as the fake `nix` evaluates it.
  target = "/nix/store/" + lib.fixedWidthString 32 "t" "" + "-target.drv";
  plantedFacts = pkgs.writeText "planted-units.json" (
    builtins.toJSON {
      drv = lib.mapAttrs (_: e: e.drv) index;
      inherit (plantedPlan) lines lateFrom;
      classes = plantedClasses;
      deps = lib.mapAttrs (_: u: u.deps) planted;
      inherit target oneCheck;
    }
  );
  # WHO REFERS TO WHOM, for the fake store's `--referrers-closure`: every
  # unit to what it is built from, the battery's check to the aggregate,
  # and what a sized build is given to every unit.
  plantedReferrers = pkgs.writeText "planted-referrers" (
    lib.concatStrings (
      lib.concatLists (
        lib.mapAttrsToList (id: u: map (d: "${index.${d}.drv} ${index.${id}.drv}\n") u.deps) planted
      )
      ++ [ "${index."aggregate/rust-tests".drv} ${oneCheck}\n" ]
      ++ map (e: "${e.drv} ${target}\n") (lib.attrValues index)
    )
  );
  battery = pkgs.callPackage ../nix/gate.nix {
    inherit system sizing;
    units = index;
    classes = plantedClasses;
    name = "ronoctua-fake-battery";
    what = "targets";
    targets = [ "one-check" ];
  };

  # A `nix` THAT WRITES DOWN EVERY BUILD AND HOW LONG IT RAN: `start <drv>
  # <cores> <max-jobs> <µs> <pid>` and `end <drv> <µs>` (the realtime clock,
  # digits only: no locale moves it), a unit taking what `$FAKE_TIMES` says
  # (or 0.05 s) and failing when `$FAKE_FAILS` names it; killed, it writes
  # `killed <drv> held|gone` — whether the folder of its out-link was still
  # there. A dry run of what `$FAKE_TARGETS` names answers `$FAKE_PLAN`; any
  # other answers what it was asked and what `$FAKE_NEEDS` lists (what the
  # units are built from that is no unit), less what `$FAKE_HELD` lists (the
  # store has it). The targets of `path-info` are `$FAKE_TARGETS`, and a
  # battery's two evaluations are answered as one check of no VM. A dry run
  # takes `$FAKE_DRY_SLEEP` seconds and an evaluation `$FAKE_EVAL_SLEEP`
  # when set, writing `dry <pid>` or `eval <pid>` when it starts, and
  # `killed-dry <pid>` or `killed-eval <pid>` when a TERM ends it.
  timedNix = pkgs.writeShellScriptBin "nix" ''
    echo "$*" >> "$FAKE_LOG"
    slow() {
      local what=$1 seconds=$2 sl
      echo "$what $$" >> "$FAKE_TIMED"
      trap 'echo "killed-$what $$" >> "$FAKE_TIMED"; kill "$sl" 2>/dev/null; exit 143' TERM
      sleep "$seconds" &
      sl=$!
      wait "$sl"
      trap - TERM
    }
    case "$1" in
      eval)
        if [ -n "''${FAKE_EVAL_SLEEP:-}" ]; then slow eval "$FAKE_EVAL_SLEEP"; fi
        case "$*" in
          *memorySize*) echo '[{"n":"one-check","mib":0,"cores":0,"nodes":0}]' ;;
          *) echo '[{"n":"one-check","drv":"${oneCheck}","driver":null,"out":null}]' ;;
        esac
        exit 0 ;;
      path-info) cat "$FAKE_TARGETS"; exit 0 ;;
      build) ;;
      *) exit 0 ;;
    esac
    drvs=() cores="" jobs="" prev="" out="" dry=0
    for a in "$@"; do
      if [ "$a" = --dry-run ]; then dry=1; fi
      if [ "$prev" = --cores ]; then cores=$a; fi
      if [ "$prev" = --max-jobs ]; then jobs=$a; fi
      if [ "$prev" = --out-link ]; then out=$a; fi
      case "$a" in *"^*") drvs+=("''${a%^*}") ;; esac
      prev=$a
    done
    if [ "$dry" = 1 ]; then
      if [ -n "''${FAKE_DRY_SLEEP:-}" ]; then slow dry "$FAKE_DRY_SLEEP"; fi
      if [ -e "''${FAKE_DRY_FAILS:-/nonexistent}" ]; then echo "error: cannot open the store" >&2; exit 1; fi
      for d in "''${drvs[@]}"; do
        if grep -qxF -- "$d" "''${FAKE_TARGETS:-/dev/null}" 2>/dev/null; then cat "$FAKE_PLAN" >&2; exit 0; fi
      done
      { printf '%s\n' "''${drvs[@]}"; cat "''${FAKE_NEEDS:-/dev/null}" 2>/dev/null || true; } \
        | awk -v held="''${FAKE_HELD:-/dev/null}" '
            BEGIN { while ((getline h < held) > 0) skip[h] = 1 }
            $1 != "" && !($1 in skip) && !($1 in seen) { seen[$1] = 1; keep[++n] = $1 }
            END {
              if (n == 1) print "this derivation will be built:"
              else if (n > 1) print "these " n " derivations will be built:"
              for (i = 1; i <= n; i++) print "  " keep[i]
            }' >&2
      exit 0
    fi
    for d in "''${drvs[@]}"; do
      echo "start $d $cores $jobs ''${EPOCHREALTIME//[!0123456789]/} $$" >> "$FAKE_TIMED"
    done
    t=0.05
    if [ "''${#drvs[@]}" -eq 1 ] && [ -e "''${FAKE_TIMES:-/nonexistent}" ]; then
      t=$(awk -v d="''${drvs[0]}" '$1 == d {print $2}' "$FAKE_TIMES")
      t=''${t:-0.05}
    fi
    killed() {
      local d where=gone
      if [ -n "$out" ] && [ -d "''${out%/*}" ]; then where=held; fi
      for d in "''${drvs[@]}"; do echo "killed $d $where" >> "$FAKE_TIMED"; done
      kill "$sl" 2>/dev/null || true
      exit 143
    }
    trap killed TERM
    sleep "$t" &
    sl=$!
    wait "$sl"
    for d in "''${drvs[@]}"; do
      echo "end $d ''${EPOCHREALTIME//[!0123456789]/}" >> "$FAKE_TIMED"
      if grep -qxF -- "$d" "''${FAKE_FAILS:-/dev/null}"; then exit 1; fi
    done
    exit 0
  '';

  # A `nix-store` THAT ANSWERS `--query --referrers-closure` from
  # `$FAKE_REFERRERS` (`<path> <a path that refers to it>`, a line each):
  # the paths asked and every one that refers to them, at any depth; and
  # `--query --references`: what the paths asked refer to, directly.
  fakeStore = pkgs.writeScriptBin "nix-store" ''
    #!${pkgs.python3}/bin/python3
    import os, sys
    a = sys.argv[1:]
    with open(os.environ.get("FAKE_LOG", "/dev/null"), "a") as f:
        f.write("nix-store " + " ".join(a) + "\n")
    if a[:2] not in (["--query", "--referrers-closure"], ["--query", "--references"]):
        sys.exit(f"fake nix-store: {a[:2]}")
    refs, down = {}, {}
    for line in open(os.environ.get("FAKE_REFERRERS", "/dev/null")).read().splitlines():
        p, r = line.split()
        refs.setdefault(p, set()).add(r)
        down.setdefault(r, set()).add(p)
    if a[1] == "--references":
        print("\n".join(sorted({p for x in a[2:] for p in down.get(x, ())})))
        sys.exit(0)
    seen, todo = set(a[2:]), list(a[2:])
    while todo:
        for r in refs.get(todo.pop(), ()):
            if r not in seen:
                seen.add(r)
                todo.append(r)
    print("\n".join(sorted(seen)))
  '';

  # WHAT A TIMED LOG SAYS, read the same way by every check below: each
  # build as an interval, and what ran together at every instant.
  timedPy = pkgs.writeText "timed.py" ''
    import collections

    def timed(path):
        """{drv: [(start, end, cores, jobs)]} from a timed log, in seconds."""
        runs = collections.defaultdict(list)
        open_ = {}
        for line in open(path).read().splitlines():
            w = line.split()
            if w[0] == "start":
                open_.setdefault(w[1], []).append((int(w[4]) / 1e6, w[2], w[3]))
            elif w[0] == "end":
                s, cores, jobs = open_[w[1]].pop(0)
                runs[w[1]].append((s, int(w[2]) / 1e6, cores, jobs))
        return runs

    def started(path):
        """[(drv, pid)] of every build started, in the order they started."""
        return [(w[1], int(w[5])) for w in (l.split() for l in open(path).read().splitlines()) if w[0] == "start"]

    def together(runs):
        """The most that ran at once: the largest set of builds alive at one
        instant (every interval's start is such an instant)."""
        flat = [(d, s, e) for d, rs in runs.items() for (s, e, _, _) in rs]
        best = []
        for _, s, _ in flat:
            alive = [d for d, s2, e2 in flat if s2 <= s < e2]
            if len(alive) > len(best):
                best = alive
        return flat, best

    def overlapping(flat, d):
        """What ran at any instant while `d` did."""
        mine = [(s, e) for x, s, e in flat if x == d]
        return sorted({x for x, s2, e2 in flat if x != d for (s, e) in mine if s2 < e and s < e2})

    def heaviest_moment(flat, weight):
        """The largest sum of `weight(drv)` alive at one instant."""
        most = 0
        for _, s, _ in flat:
            most = max(most, sum(weight(d) for d, s2, e2 in flat if s2 <= s < e2))
        return most

    def cost_kb(cost, budget_kb, cpus, client_kb):
        """What a plan's cost word plans a unit at, and its cores — the rule
        written out again: a slot and its client at one core, or alone with
        the cores the budget leaves."""
        if cost.startswith("alone:"):
            base, per = (int(x) * 1024 for x in cost[6:].split("+"))
            cores = 1 if budget_kb <= base else min(cpus, 1 + (budget_kb - base) // per)
            return base + (cores - 1) * per, cores
        return int(cost) * 1024 + client_kb, 1
  '';

  # THE MACHINE THAT IS BUILT: the reference system, its phases and what the
  # flake hands its apply.
  noctua = self.nixosConfigurations.noctua.config;
  releaseIndex = lib.filterAttrs (_: e: e.flavour == "release") rust.unitIndex;
  releasePlanned = lib.filterAttrs (_: e: lib.elem e.kind sizing.plannedKinds) releaseIndex;
  machineFacts = pkgs.writeText "machine-phases-facts.json" (
    builtins.toJSON {
      phases = noctua.ronoctua.machine.phases;
      system = plain noctua.system.build.toplevel.drvPath;
      # Every release unit the plan must hold, with the class it is built
      # by and how that class was found.
      release = lib.mapAttrs (_: e: { inherit (e) kind drv deps; }) releasePlanned;
      found = sizing.classesOf { units = lib.attrNames releasePlanned; };
      shell = plain rust.release.bins.ronoctua-shell.drvPath;
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
      units = noctua.ronoctua.machine.environment.RONOCTUA_BUILD_UNITS;
    }
  );

  # THE PLANTED MEASUREMENTS of `.#rust-peaks` — each a peak in MiB and the
  # cores it was taken at —, and the classes it is handed: the class each
  # planned unit is built by is `classesOf`'s, the plans' own resolution.
  peaks =
    let
      n = sizing.numbers;
      light = (lib.elemAt n.classes 0).name;
      medium = (lib.elemAt n.classes 1).name;
      sl = (lib.elemAt n.classes 0).slotMiB;
      sm = (lib.elemAt n.classes 1).slotMiB;
      unit = kind: flavour: mib: cores: {
        inherit
          kind
          flavour
          mib
          cores
          ;
      };
      units = {
        "release/libs/small" = unit "libs" "release" 100 1;
        "release/libs/grew" = unit "libs" "release" (sl + 10) 1;
        "release/deps/mid" = unit "deps" "release" (sl + 50) 1;
        "release/libs/new" = unit "libs" "release" (sm + 10) 1;
        # Heavy at one core and past every slot: heavy still, and past what
        # a heavy unit is planned at with one core.
        "release/bins/shell" = unit "bins" "release" (n.shellMiB + 100) 1;
        "release/bins/other" = unit "bins" "release" null 1;
        # Heavy, alone at twelve cores, under the medium slot: kept heavy.
        "release/libs/footer" = unit "libs" "release" (sm / 2) 12;
        # Heavy, measured at one core under the third class's slot: it leaves.
        "release/libs/demoted" = unit "libs" "release" (sm + 10) 1;
        # Just past the light slot with the headroom, inside it without:
        # judged by the slot, it fits; `--write` moves it.
        "test/clippy/edge" = unit "clippy" "test" (sl * 100 / (100 + n.headroomPercent) + 1) 1;
        # At the light slot itself: inside it.
        "test/clippy/full" = unit "clippy" "test" sl 1;
        # Light, built at twelve cores (a `nix build` not sized), past the
        # light slot: neither judged nor moved.
        "test/clippy/wide" = unit "clippy" "test" (sl * 2) 12;
        "test/testBins/never" = unit "testBins" "test" null 1;
        # Renamed by a lock change: one never measured since, and one
        # measured past the class it carried.
        "release/deps/renamed@1.0.0/lib:renamed/build/o3/bbbbbbbbbbbb" = unit "deps" "release" null 1;
        "release/deps/moved@1.0.0/lib:moved/build/o3/dddddddddddd" = unit "deps" "release" (sl + 100) 1;
        "aggregate/rust-tests" = unit "aggregate" null null 1;
      };
      classes = {
        "release/libs/small" = light;
        "release/libs/grew" = light;
        "release/deps/mid" = medium;
        "release/bins/shell" = "heavy";
        "release/bins/other" = "heavy";
        "release/libs/footer" = "heavy";
        "release/libs/demoted" = "heavy";
        "test/clippy/edge" = light;
        "test/clippy/full" = light;
        "test/clippy/wide" = light;
        "test/testBins/never" = medium;
        "release/libs/gone" = light;
        "release/deps/renamed@1.0.0/lib:renamed/build/o3/aaaaaaaaaaaa" = light;
        "release/deps/moved@1.0.0/lib:moved/build/o3/cccccccccccc" = light;
      };
      found = sizing.classesOf {
        units = lib.attrNames (lib.filterAttrs (_: u: lib.elem u.kind sizing.plannedKinds) units);
        inherit classes;
      };
    in
    pkgs.writeText "planted-peaks.json" (
      builtins.toJSON {
        inherit units classes;
        resolved = lib.mapAttrs (_: f: f.class) found;
        how = lib.mapAttrs (_: f: f.how) found;
      }
    );
in
{
  # THE ARITHMETIC, on machines planted around the program.
  #
  # `ronoctua-build-sizing` measures `/proc/meminfo`, the processors and the
  # ceiling of where the builds run; `--meminfo`, `--cpus` and `--ceiling`
  # stand in for each. What it answers is checked against the rule written
  # out again here — the general slots are what the budget holds of the
  # general peak, never past the processors; what is no unit at most four
  # with the slots split among them — on a small machine, this one, a large
  # one, and the large one under a ceiling. It holds no class of unit: the
  # plans carry those (`sized-units-keep-to-the-budget`). And what it cannot
  # read is refused with a sentence, never sized by a default: a meminfo
  # without the two lines, a count of processors that is not one, a ceiling
  # that is not bytes. The cgroup's ceiling (root's builds run in its own)
  # is the smallest limit from the cgroup up, asked of planted cgroup
  # folders.
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
            client = N["clientMiB"] * 1024
            budget = available - floor_kb - reserve
            if ceiling_b is not None:
                budget = min(budget, max(0, ceiling_b // 1024 - reserve))
            budget = max(0, budget)
            slots = 1 if budget < peak else budget // peak
            slots = min(slots, cpus)
            jobs = min(slots, N["jobsCeiling"])
            cores = max(1, slots // jobs)
            return {"budget_kb": budget, "slots": slots, "jobs": jobs, "cores": cores,
                    "cpus": cpus, "client_kb": client}

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
            s, j, c = (int(got[k]) for k in ("slots", "jobs", "cores"))
            budget = int(got["budget_kb"])
            if j * c > s or j > N["jobsCeiling"] or s > cpus or s < 1:
                wrong.append(f"{name}: {j} jobs of {c} cores in {s} slots and {cpus} processors")
            if s > 1 and s * N["peakMiB"] * 1024 > budget:
                wrong.append(f"{name}: {s} compilers of {N['peakMiB']} MiB do not fit {budget // 1024} MiB")
            if ceiling is not None and budget > ceiling // 1024:
                wrong.append(f"{name}: the budget {budget} kB is past the ceiling")
            classy = sorted(k for k in got if k.startswith("class_") or k == "heavy_cores")
            if classy:
                wrong.append(f"{name}: the program says {classy}: a class table of the running system's, which a newer plan would contradict")
        # THE CLASSES ARE LIGHTEST FIRST, and none is heavier than a heavy unit.
        sizes = [c["slotMiB"] for c in N["classes"]]
        if sizes != sorted(sizes) or len(set(sizes)) != len(sizes) or sizes[-1] >= N["shellMiB"]:
            wrong.append(f"the classes' slots {sizes} are not lightest first and under a heavy unit's")
        # THE SAME MACHINE, SMALLER UNDER ITS CEILING, AND NEVER LARGER.
        _, free, _ = ask("--meminfo", meminfo("c1", 64 * GIB, 60 * GIB), "--cpus", "16", "--ceiling", "infinity")
        _, held, _ = ask("--meminfo", meminfo("c2", 64 * GIB, 60 * GIB), "--cpus", "16", "--ceiling", str(8 * GIB * 1024))
        if not int(held["slots"]) < int(free["slots"]):
            wrong.append(f"a ceiling of 8 GiB did not shrink the slots ({free['slots']} -> {held['slots']})")
        if not int(held["budget_kb"]) < int(free["budget_kb"]):
            wrong.append("a ceiling did not shrink the budget")

        # WHAT IT CANNOT READ IS REFUSED.
        bad = tmp / "no-available"
        bad.write_text("MemTotal: 1000 kB\n")
        refusals = [
            (("--meminfo", str(bad), "--cpus", "4", "--ceiling", "infinity"), "MemAvailable"),
            (("--meminfo", meminfo("ok", GIB, GIB), "--cpus", "0", "--ceiling", "infinity"), "processors"),
            (("--meminfo", meminfo("ok2", GIB, GIB), "--cpus", "4", "--ceiling", "lots"), "ceiling"),
            (("--meminfo", meminfo("ok3", GIB, GIB), "--cpus", "08", "--ceiling", "infinity"), "processors"),
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
        floor(len(N["classes"]), 3, "classes of unit")
        if wrong:
            print("ERROR: the build sizing does not fit the machine it measures:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(machines)} machines sized by the rule and no class table said, a ceiling shrinks them, four unreadable answers refused, and the cgroup's ceiling is its smallest limit")
        END
      '';

  # THE UNITS ARE STARTED BY THE COST THEIR LINE SAYS, AND THE MEMORY NEVER
  # PASSES THE BUDGET.
  #
  # `sized_units` itself, sourced, over a planted plan of forty units in
  # layers — lights, mediums, a class no distro has (`vast`, as a newer
  # plan would say) and two heavy ones whose `alone` numbers are planted
  # too, each built from units below it, the last layer started last — with
  # the budget and the processors set (two mediums or four lights fit, a
  # vast beside a light and never beside a medium, never more than four at
  # once), and a `nix` that takes each unit's time and writes down when
  # each ran. Every unit is built once, in its own `nix build --max-jobs 1`,
  # never before what it is built from ended; at no instant do the costs
  # its plan gave what runs (each slot with its client) pass the budget, nor
  # the builds the processors; a heavy unit runs alone, with the cores its
  # own line's numbers leave, and every other with one; the last layer
  # starts after every other unit ended; the slots are used — four lights
  # ran together, and a medium beside lights — and the classes it planned
  # by are said, with how many carried a class and how many had none. A
  # unit the dry run does not list is never built, and what is built from
  # it does not wait for it.
  #
  # What the units are built from that is no unit (a C library, planted) is
  # built FIRST, alone, sized as what is no unit, and never inside a unit's
  # build: what the caller's dry run listed beside the units and is below
  # one of them — found in the store's references, level by level, asked
  # only of what will be built, with no second dry run; what is built FROM
  # the units (a program, also listed) is not. With the units alone (no dry
  # run answered the caller) their own dry run says it. When it fails,
  # every unit that needs it (the store's own references) is not built and
  # said, the others are, it is written down as not built too, and without
  # `--keep-going` nothing is. And with a REAL `nix`, in a store of its own:
  # of two such derivations one fails and the other takes a while, each
  # built from by a unit — the slow one is still built (`--keep-going`
  # reaches the build of what is no unit), its unit too, and only the
  # failing one's unit is left out, both written down.
  #
  # A unit that fails is built ONCE: with `--keep-going` everything built
  # from it is not built — eight test runs of one failing unit among them,
  # said in one line — and every unit that does not need it is, and what
  # was not built is written down for the caller; without it nothing starts
  # after it. A signal stops the builds before the caller cleans up
  # (`sized-build-builds-what-nix-would`). And a plan it cannot read — a
  # line that is no unit, a class's name that is none, how a class was found
  # in no name, a cost that is no cost, a path that is no derivation, a
  # number that is not a line's or is no line of it — is refused before any
  # build; how a class was found in a word no distro says today is built.
  sized-units-keep-to-the-budget =
    mkCheck "sized-units-keep-to-the-budget"
      [
        pkgs.python3
        pkgs.bash
        timedNix
        fakeStore
      ]
      ''
        python3 - <<'END'
        import json, os, pathlib, re, subprocess, sys
        exec(open("${pyPrelude}").read())
        exec(open("${timedPy}").read())
        REAL_NIX = "${pkgs.nix}/bin"

        # PARSED: the plan this writes, the numbers the sizing was built with
        # (JSON), the timed log the fake `nix` wrote, line by line, and the
        # scheduler's own lines, in order; with the real `nix`, its own
        # answers about its own store.
        # FILE-WIDE: the planted references are copied whole to add two lines
        # to them, and the scheduler's output is searched whole for each
        # sentence: the question is what it says, wherever it says it.
        FUNCTIONS = "${sizing.functions}"
        N = json.loads('${numbersJson}')
        tmp = pathlib.Path(os.environ["TMPDIR"])
        LIGHT, MEDIUM = N["classes"][0]["name"], N["classes"][1]["name"]
        CLIENT = N["clientMiB"] * 1024
        # THE PLAN'S OWN NUMBERS, in MiB: the distro's two classes, one no
        # distro has, and a heavy unit's planted `alone`.
        SLOT = {LIGHT: N["classes"][0]["slotMiB"], MEDIUM: N["classes"][1]["slotMiB"], "vast": 3000}
        COST = {c: str(s) for c, s in SLOT.items()} | {"heavy": "alone:2000+1000"}
        BUDGET = 2 * (SLOT[MEDIUM] * 1024 + CLIENT) + 64 * 1024
        CPUS, JOBS, CORES = 4, 2, 2
        WEIGHT = {c: cost_kb(COST[c], BUDGET, CPUS, CLIENT) for c in COST}
        HEAVY = WEIGHT["heavy"][1]
        wrong = []

        # FORTY UNITS IN LAYERS: each built from up to three of the layer
        # below; every fourth a medium, two vast, two heavy in the middle;
        # the last layer started last (nothing is built from it).
        units = []
        for layer in range(8):
            for k in range(5):
                i = len(units)
                below = [j for j in range(len(units)) if units[j]["layer"] == layer - 1]
                deps = [below[(k + s) % len(below)] for s in range(min(3, len(below)))] if below else []
                cls = ("heavy" if (layer, k) in ((3, 2), (5, 0)) else "vast" if i in (9, 22)
                       else MEDIUM if i % 4 == 1 else LIGHT)
                how = "carried" if i % 7 == 3 else "default" if i % 11 == 5 else "named"
                units.append({"id": f"u/{i}", "layer": layer, "deps": sorted(set(deps)), "class": cls, "how": how,
                              "drv": f"/nix/store/{i:032d}-unit-{i}.drv", "time": 0.05 + (i % 3) * 0.04})
        LATE = 35
        def line(u):
            return f"{u['id']} {u['class']} {u['how']} {COST[u['class']]} {u['drv']} {' '.join(map(str, u['deps']))}\n"
        def write_plan(path, units):
            path.write_text("".join(line(u) for u in units))
        plan = tmp / "plan"
        write_plan(plan, units)
        ZIG = "/nix/store/" + "z" * 32 + "-libghostty-vt.drv"
        (tmp / "times").write_text("".join(f"{u['drv']} {u['time']}\n" for u in units) + f"{ZIG} 0.3\n")
        by_drv = {u["drv"]: u for u in units}
        # Every unit referred to by what is built from it, for the fake store.
        (tmp / "referrers").write_text("".join(f"{units[d]['drv']} {u['drv']}\n" for u in units for d in u["deps"]))

        def run(name, will, fails=(), keep_going=False, plan_file=plan, needs=(), referrers=None, late=LATE, units_only=False):
            where = tmp / name
            where.mkdir()
            (where / "will").write_text("".join(d + "\n" for d in will))
            (where / "fails").write_text("".join(d + "\n" for d in fails))
            (where / "needs").write_text("".join(d + "\n" for d in needs))
            script = f"""
        set -euo pipefail
        . "{FUNCTIONS}"
        sizing_measure /proc/meminfo {CPUS} infinity > /dev/null
        budget_kb={BUDGET}; cpus={CPUS}; jobs={JOBS}; cores={CORES}
        sizing_keep_going={1 if keep_going else 0}
        sizing_late_from={late}
        sizing_will_units_only={1 if units_only else 0}
        sized_flags=()
        sized_units "{plan_file}" "{where}/will" "{where}/held"
        """
            env = dict(os.environ, FAKE_LOG=str(where / "log"), FAKE_TIMED=str(where / "timed"),
                       FAKE_TIMES=str(tmp / "times"), FAKE_FAILS=str(where / "fails"),
                       FAKE_NEEDS=str(where / "needs"), FAKE_REFERRERS=str(referrers or tmp / "referrers"))
            (where / "timed").write_text("")
            ran = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True)
            nb = where / "held/not-built"
            return ran, timed(where / "timed"), set(nb.read_text().split()) if nb.exists() else None

        def weight(d):
            return WEIGHT[by_drv[d]["class"]][0] if d in by_drv else 0

        def launches(said):
            return [re.match(r"^\[[\d.]+ s\] -> (\S+) \(([\w-]+), (?:(\d+) running|alone with (\d+) cores)", l) for l in said]

        # 1. EVERYTHING, AND THE BOUND.
        ran, runs, not_built = run("all", [u["drv"] for u in units])
        if ran.returncode != 0:
            wrong.append(f"all: exit {ran.returncode}: {ran.stdout[-600:]} {ran.stderr[-600:]}")
        if sorted(runs) != sorted(by_drv) or any(len(r) != 1 for r in runs.values()):
            wrong.append(f"all: built {sum(map(len, runs.values()))} times over {len(runs)} units, not each of {len(units)} once")
        flat, most = together(runs)
        early_end = max((rs[0][1] for d, rs in runs.items() if int(by_drv[d]["id"][2:]) < LATE), default=0)
        for d, rs in runs.items():
            u = by_drv[d]
            s, e, cores, jobs = rs[0]
            for j in u["deps"]:
                dep = units[j]["drv"]
                if dep in runs and runs[dep][0][1] > s:
                    wrong.append(f"all: {u['id']} started before {units[j]['id']}, which it is built from, ended")
            want_cores = str(HEAVY) if u["class"] == "heavy" else "1"
            if cores != want_cores or jobs != "1":
                wrong.append(f"all: {u['id']} ({u['class']}) built with --max-jobs {jobs} --cores {cores}, not {want_cores}")
            if u["class"] == "heavy" and overlapping(flat, d):
                wrong.append(f"all: the heavy {u['id']} ran beside {overlapping(flat, d)[:3]}")
            if u["class"] == "vast" and any(by_drv[o]["class"] == MEDIUM for o in overlapping(flat, d)):
                wrong.append(f"all: the vast {u['id']} ran beside a medium: its line's 3000 MiB was not what it was planned at")
            if int(u["id"][2:]) >= LATE and s < early_end:
                wrong.append(f"all: {u['id']}, of the lines started last, started before every other unit ended")
        heaviest = heaviest_moment(flat, weight)
        if heaviest > BUDGET:
            wrong.append(f"all: {heaviest // 1024} MiB of slots ran at once, past the budget of {BUDGET // 1024}")
        if len(most) > CPUS:
            wrong.append(f"all: {len(most)} builds ran at once on {CPUS} processors")
        # THE SLOTS ARE USED, read from what the scheduler says it started,
        # in its order (deterministic; the fake's clock is not): its first
        # round, before any build ended, starts a medium beside lights, and
        # it holds more at once than one slot of the medium size would fit.
        said = ran.stdout.splitlines()
        launched = launches(said)
        first_end = next((i for i, l in enumerate(said) if l.startswith("<- ")), len(said))
        first_round = [m.group(2) for m in launched[:first_end] if m]
        most_running = max((int(m.group(3)) for m in launched if m and m.group(3)), default=0)
        if first_round.count(MEDIUM) < 1 or first_round.count(LIGHT) < 2 or most_running <= BUDGET // (SLOT[MEDIUM] * 1024 + CLIENT):
            wrong.append(f"all: the slots were not used: the first round started {first_round}, at most {most_running} at once")
        alone = [m.group(4) for m in launched if m and m.group(4)]
        if alone != [str(HEAVY)] * 2:
            wrong.append(f"all: the heavy units were said alone with {alone} cores, not {HEAVY} each")
        n_carried = sum(1 for u in units if u["how"] == "carried")
        n_default = sum(1 for u in units if u["how"] == "default")
        for needle in (f"-- {len(units)} units to build (of {len(units)} in the plan)",
                       f": vast, 3000 MiB each",
                       f": heavy, alone with {HEAVY} cores (2000 MiB and 1000 MiB a core more)",
                       f"-- {n_carried} of them carry the class of the unit a lock change renamed",
                       f"-- {n_default} of them the class data does not name",
                       "nix run .#rust-peaks -- --write", "units built"):
            if needle not in ran.stdout:
                wrong.append(f"all: it did not say {needle!r}: {ran.stdout[:900]!r}")
        if not_built != set():
            wrong.append(f"all: it wrote down {sorted(not_built)[:3]} as not built")

        # 2. WHAT THE STORE HOLDS IS NOT BUILT, AND NOTHING WAITS FOR IT.
        held = {units[0]["drv"], units[6]["drv"]}
        ran, runs, _ = run("held", [u["drv"] for u in units if u["drv"] not in held])
        if ran.returncode != 0 or held & set(runs) or len(runs) != len(units) - 2:
            wrong.append(f"held: exit {ran.returncode}, built {len(runs)}, the held ones among them: {sorted(held & set(runs))}")

        # 3. A UNIT FAILS: built once, and nothing built from it.
        bad = units[7]
        def needs(i, j):
            return j in units[i]["deps"] or any(needs(k, j) for k in units[i]["deps"])
        needing = {u["drv"] for i, u in enumerate(units) if needs(i, 7)}
        free_of_it = {u["drv"] for i, u in enumerate(units) if i != 7 and not needs(i, 7)}
        ran, runs, not_built = run("keep-going", [u["drv"] for u in units], fails=[bad["drv"]], keep_going=True)
        if ran.returncode != 1 or set(runs) != free_of_it | {bad["drv"]} or len(runs[bad["drv"]]) != 1:
            wrong.append(f"keep-going: exit {ran.returncode}; built {len(runs)}, of the {len(free_of_it)} that do not need the failed one and itself (once)")
        if f"-- {len(needing)} unit(s) built from {bad['id']} are not built, because it failed" not in ran.stdout:
            wrong.append(f"keep-going: it did not say the {len(needing)} units built from {bad['id']} are not built: {ran.stdout[-800:]!r}")
        if not_built != needing | {bad["drv"]}:
            wrong.append(f"keep-going: it wrote down {len(not_built or ())} as not built, not the failed one and the {len(needing)} built from it")
        ran, runs, _ = run("stop", [u["drv"] for u in units], fails=[bad["drv"]])
        # Nothing starts once the scheduler has seen the failure: no `->`
        # after its `FAILED` line (its own order; two builds ending at once
        # may be seen either way round).
        said = ran.stdout.splitlines()
        seen = next((i for i, l in enumerate(said) if l.startswith("<- " + bad["id"] + ": FAILED")), None)
        late = [l for l in said[seen + 1:] if " -> " in l] if seen is not None else ["(no FAILED line)"]
        if ran.returncode != 1 or late or len(runs) >= len(units):
            wrong.append(f"stop: exit {ran.returncode}; {late[:3]} started after the failure was seen")

        # 4. A FAILING DEPENDENCY OF EIGHT TEST RUNS IS BUILT ONCE: the runs
        # are never handed a build, and the other runs still run, last.
        small = [{"id": "f", "class": LIGHT, "how": "named", "deps": [], "drv": "/nix/store/" + "f" * 32 + "-f.drv"},
                 {"id": "g", "class": LIGHT, "how": "named", "deps": [], "drv": "/nix/store/" + "g" * 32 + "-g.drv"}]
        small += [{"id": f"r{k}", "class": LIGHT, "how": "named", "deps": [0], "drv": f"/nix/store/{k:031d}r-run-{k}.drv"} for k in range(8)]
        small += [{"id": f"s{k}", "class": LIGHT, "how": "named", "deps": [1], "drv": f"/nix/store/{k:031d}s-run-{k}.drv"} for k in range(2)]
        p = tmp / "plan-runs"
        write_plan(p, small)
        ran, runs, not_built = run("runs", [u["drv"] for u in small], fails=[small[0]["drv"]], keep_going=True, plan_file=p, late=2)
        rs = {u["id"]: runs.get(u["drv"], []) for u in small}
        if (ran.returncode != 1 or len(rs["f"]) != 1 or any(rs[f"r{k}"] for k in range(8))
                or any(len(rs[f"s{k}"]) != 1 for k in range(2))):
            wrong.append(f"runs: exit {ran.returncode}; f built {len(rs['f'])} time(s), its runs {sum(len(rs[f'r{k}']) for k in range(8))}, the others' {[len(rs[f's{k}']) for k in range(2)]}")
        if "-- 8 unit(s) built from f are not built, because it failed: r0, r1, r2, r3, r4 and 3 more" not in ran.stdout:
            wrong.append(f"runs: it did not say the eight runs of f are not built, and why: {ran.stdout[-600:]!r}")
        if not_built != {u["drv"] for u in small[:1] + small[2:10]}:
            wrong.append(f"runs: it wrote down {sorted(not_built or ())} as not built")

        # 5. WHAT THE UNITS ARE BUILT FROM THAT IS NO UNIT, FIRST: what the
        # caller's dry run listed beside them and is below a unit (the
        # store's references), never asked of Nix again; what is built from
        # the units (a program, listed too) is no such thing.
        every = [u["drv"] for u in units]
        TOP = "/nix/store/" + "p" * 32 + "-program.drv"
        refs = tmp / "referrers-zig"
        refs.write_text((tmp / "referrers").read_text() + f"{ZIG} {units[2]['drv']}\n{ZIG} {units[9]['drv']}\n"
                        + "".join(f"{u['drv']} {TOP}\n" for u in units[-5:]))
        listed = every + [ZIG, TOP]
        ran, runs, _ = run("needs", listed, referrers=refs)
        zig = runs.get(ZIG, [])
        first_unit = min((rs[0][0] for d, rs in runs.items() if d in by_drv), default=0)
        if ran.returncode != 0 or len(zig) != 1 or zig[0][2:] != (str(CORES), str(JOBS)) or zig[0][1] > first_unit:
            wrong.append(f"needs: exit {ran.returncode}; the library built {zig}, not once, first, with --max-jobs {JOBS} --cores {CORES}")
        if "-- first, what they are built from that is no unit: 1 derivation(s), sized as what is no unit" not in ran.stdout:
            wrong.append(f"needs: it did not say what it built first: {ran.stdout[:600]!r}")
        if TOP in runs:
            wrong.append("needs: what is built FROM the units was built first, as what they are built from")
        calls = (tmp / "needs" / "log").read_text().splitlines()
        dry = [c for c in calls if c.startswith("build ") and "--dry-run" in c.split()]
        asked = [w for c in calls if c.startswith("nix-store --query --references ") for w in c.split()[3:]]
        if dry:
            wrong.append(f"needs: Nix's dry run was asked again, of what the caller's had answered: {dry[0][:200]}")
        if not asked or set(asked) - set(listed) or ZIG not in asked:
            wrong.append(f"needs: the store was asked of {sorted(set(asked) - set(listed))[:3] or asked[:3]}: only of what will be built, the library among it")
        # The units alone, as when no dry run answered the caller: their
        # own dry run says what they are built from.
        ran, runs, _ = run("needs-units-only", every, needs=[ZIG], units_only=True)
        zig = runs.get(ZIG, [])
        calls = (tmp / "needs-units-only" / "log").read_text().splitlines()
        dry = [c for c in calls if c.startswith("build ") and "--dry-run" in c.split()]
        if ran.returncode != 0 or len(zig) != 1 or len(dry) != 1:
            wrong.append(f"needs-units-only: exit {ran.returncode}; the library built {len(zig)} time(s) after {len(dry)} dry run(s), not once after the units' own")
        # It fails: what needs it (units 2 and 9, and what is built from
        # them) is not built; the rest is, and the answer is no. The build
        # of it was told to keep going, and it is written down as not built.
        blocked = {u["drv"] for i, u in enumerate(units) if i in (2, 9) or needs(i, 2) or needs(i, 9)}
        ran, runs, not_built = run("needs-fails", listed, fails=[ZIG], keep_going=True, referrers=refs)
        if ran.returncode != 1 or set(runs) != (set(every) - blocked) | {ZIG} or len(runs[ZIG]) != 1:
            wrong.append(f"needs-fails: exit {ran.returncode}; built {len(runs)}, not the {len(every) - len(blocked)} that do not need the library and it, once")
        if f"-- {len(blocked)} unit(s) are not built, because what they are built from did not build" not in ran.stdout:
            wrong.append(f"needs-fails: it did not say which units need what did not build: {ran.stdout[-600:]!r}")
        if not_built != blocked | {ZIG}:
            wrong.append(f"needs-fails: it wrote down {len(not_built or ())} as not built, not the {len(blocked)} that need the library and the library")
        calls = (tmp / "needs-fails" / "log").read_text().splitlines()
        if not any(ZIG + "^*" in c.split() and "--keep-going" in c.split() and "--dry-run" not in c.split() for c in calls):
            wrong.append("needs-fails: what the units are built from was built without --keep-going: one failing stops the others")
        ran, runs, not_built = run("needs-stop", listed, fails=[ZIG], referrers=refs)
        if ran.returncode != 1 or set(runs) != {ZIG} or not_built != set(every) | {ZIG}:
            wrong.append(f"needs-stop: exit {ran.returncode}; built {sorted(runs)[:3]}: without --keep-going no unit is built after it")

        # 6. WHAT IT CANNOT READ IS REFUSED, AND NOTHING IS BUILT.
        d0 = units[0]["drv"]
        for name, text in (("no-unit", f"u/0 {LIGHT} named {d0}\n"),
                           ("no-class", f"u/0 l!ght named 768 {d0}\n"),
                           ("no-how", f"u/0 {LIGHT} me@sured 768 {d0}\n"),
                           ("no-cost", f"u/0 {LIGHT} named lots {d0}\n"),
                           ("no-alone", f"u/0 heavy named alone:4864 {d0}\n"),
                           ("octal", f"u/0 {LIGHT} named 0768 {d0}\n"),
                           ("no-number", f"u/0 {LIGHT} named 768 {d0} first\n"),
                           ("past-the-plan", f"u/0 {LIGHT} named 768 {d0} 1\n"),
                           ("leading-zero", f"u/0 {LIGHT} named 768 {d0}\nu/1 {LIGHT} named 768 {units[1]['drv']} 00\n"),
                           ("no-store", f"u/0 {LIGHT} named 768 /tmp/x.drv\n"),
                           ("old-grammar", f"u/0 {LIGHT} {d0}\n")):
            p = tmp / f"plan-{name}"
            p.write_text(text)
            ran, runs, _ = run(name, [d0], plan_file=p)
            if ran.returncode != 2 or runs or "build sizing:" not in ran.stderr:
                wrong.append(f"{name}: exit {ran.returncode}, {len(runs)} built, {ran.stderr.strip()!r}: not a refusal")
        # How a class was found, in a word a newer distro says: built, and
        # no refusal (which would revert every update of a machine).
        p = tmp / "plan-new-how"
        p.write_text(f"u/0 {LIGHT} measured 768 {d0}\n")
        ran, runs, _ = run("new-how", [d0], plan_file=p)
        if ran.returncode != 0 or len(runs.get(d0, [])) != 1:
            wrong.append(f"new-how: a plan saying how a class was found in a new word: exit {ran.returncode}, {ran.stderr.strip()!r}")

        # 7. A REAL NIX, IN A STORE OF ITS OWN (`local?root=`, its builds in
        # its own sandbox): of what two units are built from that is no
        # unit, one fails at once and the other takes a while. Nix stops
        # every other build at the first failure unless it keeps going: with
        # --keep-going the slow one is still built, and its unit, and only
        # the failing one's unit is left out — both written down.
        real = tmp / "real"
        (real / "root").mkdir(parents=True)
        (real / "two.nix").write_text(
            "let\n"
            "  mk = name: script: deps: derivation {\n"
            "    inherit name deps;\n"
            "    system = \"${system}\";\n"
            "    builder = \"/bin/sh\";\n"
            "    args = [ \"-c\" script ];\n"
            "  };\n"
            "  p1 = mk \"p1\" \"exit 1\" [ ];\n"
            "  p2 = mk \"p2\" \"i=0; while [ $i -lt 400000 ]; do i=$((i + 1)); done; echo ok > $out\" [ ];\n"
            "in\n"
            "{\n"
            "  inherit p1 p2;\n"
            "  u1 = mk \"u1\" \"echo ok > $out\" [ p1 ];\n"
            "  u2 = mk \"u2\" \"echo ok > $out\" [ p2 ];\n"
            "}\n")
        renv = dict(os.environ, HOME=str(real), NIX_REMOTE=f"local?root={real / 'root'}",
                    NIX_CONFIG="experimental-features = nix-command\nsubstituters =\nbuild-users-group =\nsandbox = true\n")
        inst = subprocess.run([REAL_NIX + "/nix-instantiate", str(real / "two.nix"), "-A", "u1", "-A", "u2", "-A", "p1", "-A", "p2"],
                              env=renv, capture_output=True, text=True)
        drvs = inst.stdout.split()
        if len(drvs) != 4:
            print(f"ERROR: the real nix did not instantiate the four derivations: {inst.stderr[-800:]}", file=sys.stderr)
            sys.exit(1)
        U1, U2, P1, P2 = drvs
        (real / "plan").write_text(f"u/1 {LIGHT} named 768 {U1}\nu/2 {LIGHT} named 768 {U2}\n")
        script = f"""
        set -euo pipefail
        . "{FUNCTIONS}"
        sizing_measure /proc/meminfo 4 infinity > /dev/null
        budget_kb={BUDGET}; cpus=4; jobs=2; cores=1
        sizing_keep_going=1
        sizing_nix={REAL_NIX}/nix sizing_nix_store={REAL_NIX}/nix-store
        sized_flags=()
        "$sizing_nix" build --dry-run "{U1}^*" "{U2}^*" > /dev/null 2> "{real}/dry"
        sizing_will_build < "{real}/dry" > "{real}/will"
        sized_units "{real}/plan" "{real}/will" "{real}/held"
        """
        ran = subprocess.run(["bash", "-c", script], env=renv, capture_output=True, text=True)
        listed = set((real / "will").read_text().split()) if (real / "will").exists() else set()
        nb = real / "held/not-built"
        not_built = set(nb.read_text().split()) if nb.exists() else None
        if listed != {U1, U2, P1, P2}:
            wrong.append(f"real: the real dry run listed {sorted(listed)}, not the two units and what they are built from: {ran.stderr[-600:]!r}")
        if ran.returncode != 1 or "<- u/2: built in" not in ran.stdout or "<- u/1: built in" in ran.stdout:
            wrong.append(f"real: exit {ran.returncode}; the unit of the slow one was not built, or the failing one's was: "
                         f"{ran.stdout[-800:]!r} {ran.stderr[-800:]!r}")
        if "-- 1 unit(s) are not built, because what they are built from did not build: u/1" not in ran.stdout:
            wrong.append(f"real: it did not say the failing one's unit is not built, and why: {ran.stdout[-600:]!r}")
        if not_built != {U1, P1}:
            wrong.append(f"real: it wrote down {sorted(not_built or ())} as not built, not the failing one and its unit")

        floor(len(units), 40, "planted units")
        floor(sum(1 for u in units if u["class"] == "heavy"), 2, "heavy units")
        floor(sum(1 for u in units if u["class"] == "vast"), 2, "units of a class no distro has")
        if "vast" in [c["name"] for c in N["classes"]]:
            wrong.append("the planted class `vast` is a class of the distro's: it no longer stands for a newer plan's")
        if wrong:
            print("ERROR: the units are not started by their line's cost within the budget:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(units)} units, each once after what it is built from, by their lines' costs; at most {heaviest // 1024} of {BUDGET // 1024} MiB at once, "
              f"{most_running} builds together, the heavy ones alone, the last lines last; what is no unit first, found without a second dry run, "
              "and with a real nix one failing does not stop the others; a failed unit once, "
              "what needs it said and not built; held ones skipped, unreadable plans refused and a new word built")
        END
      '';

  # NO LOCALE MOVES THE SCHEDULER'S CLOCK.
  #
  # Bash writes `$EPOCHREALTIME` with the locale's decimal mark — a comma in
  # pt_BR, the owner's and the ISO's —, and the scheduler once took only a
  # dot out of it: `built in 0.-3 s`, then `value too great for base` (a
  # field with a leading zero read as octal), exit 1 with its builds still
  # running. Here `sized_units` runs under `LC_ALL=pt_BR.UTF-8` (glibc's own
  # locale, built for this), whose `$EPOCHREALTIME` is first proved to have
  # a comma: it answers 0, every unit's time is what its build took, the
  # times it says go forward, and no arithmetic error is said.
  sized-units-keep-time-in-any-locale =
    let
      locales = pkgs.glibcLocales.override {
        allLocales = false;
        locales = [ "pt_BR.UTF-8/UTF-8" ];
      };
    in
    mkCheck "sized-units-keep-time-in-any-locale"
      [
        pkgs.python3
        pkgs.bash
        timedNix
      ]
      ''
        export LOCALE_ARCHIVE=${locales}/lib/locale/locale-archive
        python3 - <<'END'
        import os, pathlib, re, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the scheduler's own lines, each time it says read as a
        # number, against the time each planted build takes.
        FUNCTIONS = "${sizing.functions}"
        tmp = pathlib.Path(os.environ["TMPDIR"])
        env = dict(os.environ, LC_ALL="pt_BR.UTF-8", FAKE_LOG=str(tmp / "log"), FAKE_TIMED=str(tmp / "timed"),
                   FAKE_TIMES=str(tmp / "times"))
        clock = subprocess.run(["bash", "-c", 'printf %s "$EPOCHREALTIME"'], env=env, capture_output=True, text=True).stdout
        must("," in clock and "." not in clock and clock, "a comma in $EPOCHREALTIME under pt_BR.UTF-8 (the locale took)")

        times = {}
        lines = []
        for i in range(6):
            d = f"/nix/store/{i:032d}-unit-{i}.drv"
            times[f"u/{i}"] = 0.3 if i < 4 else 1.2
            deps = "" if i < 2 else f" {i - 2}"
            lines.append(f"u/{i} light named 768 {d}{deps}\n")
        (tmp / "plan").write_text("".join(lines))
        (tmp / "will").write_text("".join(l.split()[4] + "\n" for l in lines))
        (tmp / "times").write_text("".join(f"{l.split()[4]} {times[l.split()[0]]}\n" for l in lines))
        (tmp / "timed").write_text("")
        script = f"""
        set -euo pipefail
        . "{FUNCTIONS}"
        sizing_measure /proc/meminfo 4 infinity > /dev/null
        budget_kb=$(( 8 * 1024 * 1024 )); cpus=4; jobs=1; cores=1
        sized_flags=()
        sized_units "{tmp}/plan" "{tmp}/will" "{tmp}/held"
        """
        ran = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True)
        wrong = []
        if ran.returncode != 0:
            wrong.append(f"it answered {ran.returncode}: {ran.stdout[-500:]!r} {ran.stderr[-500:]!r}")
        for word in ("value too great", "syntax error", "invalid", "arithmetic"):
            if word in ran.stderr:
                wrong.append(f"it said {word!r}: {ran.stderr[-300:]!r}")
        said = ran.stdout.splitlines()
        built = {m.group(1): float(m.group(2)) for m in (re.match(r"^<- (\S+): built in (\d+\.\d) s$", l) for l in said) if m}
        starts = [float(m.group(1)) for m in (re.match(r"^\[(\d+\.\d) s\] -> ", l) for l in said) if m]
        if sorted(built) != sorted(times):
            wrong.append(f"it said {sorted(built)} built, not {sorted(times)}: {ran.stdout[-500:]!r}")
        for uid, took in built.items():
            if not times[uid] - 0.15 <= took <= times[uid] + 3:
                wrong.append(f"{uid} took {times[uid]} s and it said {took} s")
        if len(starts) != len(times) or starts != sorted(starts) or starts[-1] < 0.3 - 0.15:
            wrong.append(f"the starts it said do not go forward from the first: {starts}")
        total = next((int(m.group(1)) for m in (re.match(r"^-- 6 of 6 units built in (\d+) s", l) for l in said) if m), None)
        if total is None or not 1 <= total <= 10:
            wrong.append(f"the whole build was said to take {total} s, not about 1.5")
        floor(len(built), 6, "units timed")
        if wrong:
            print("ERROR: the scheduler's clock moves with the locale:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: under pt_BR.UTF-8 ($EPOCHREALTIME {clock}) six units timed as they took, the starts in order, the whole in {total} s")
        END
      '';

  # THE BATTERIES BUILD EACH UNIT BY ITS CLASS, AND A FAILED UNIT ONCE.
  #
  # `nix/gate.nix` over the planted units (with their planted classes), a
  # `nix` that writes down what it was asked and when each build ran — whose
  # store holds one unit — and a `nix-store` that answers from the planted
  # references. The battery asks the dry run of every unit once before any
  # build, and Nix nothing again; builds every unit Nix would and not the
  # held one, each its own `nix build --max-jobs 1` with one core — the
  # heavy ones alone, with the cores their lines leave —, never before what
  # it is built from; the test run after every compilation, in the same
  # scheduler; the aggregate after the test run, sized as what is no unit;
  # and the checks last. The plan it builds by is the planted classes'
  # (`unitPlan`): the longest chain above first, the test runs after every
  # other line, every unit after what it is built from.
  #
  # And with a unit the test run and both heavy units are built from
  # failing: the battery builds it ONCE — no later phase hands it, or
  # anything that needs it, a build —; what is built from it is not built,
  # said in one line; the aggregate over the run and the check over the
  # aggregate are not built and fail by name; the battery answers no. And a
  # TERM while phase 0 evaluates ends the battery at once (143), the
  # evaluation stopped with it, instead of waiting for it to end.
  batteries-build-each-unit-by-its-class =
    mkCheck "batteries-build-each-unit-by-its-class"
      [
        pkgs.python3
        timedNix
        fakeStore
      ]
      ''
        python3 - <<'END'
        import json, os, pathlib, re, signal, subprocess, sys, time
        exec(open("${pyPrelude}").read())
        exec(open("${timedPy}").read())

        # PARSED: what the battery said (its sizing lines), the `nix` command
        # lines it ran word by word, the timed log, and the planted facts.
        # FILE-WIDE: the battery's output is searched whole for its sizing
        # lines: the question is the numbers it said.
        f = json.load(open("${plantedFacts}"))
        D = f["drv"]
        name = {v: k for k, v in D.items()}
        tmp = pathlib.Path(os.environ["TMPDIR"])
        wrong = []

        def battery(where_name, fails=()):
            where = tmp / where_name
            where.mkdir()
            (where / "held").write_text(D["release/libs/d"] + "\n")
            (where / "fails").write_text("".join(x + "\n" for x in fails))
            (where / "timed").write_text("")
            env = dict(os.environ, FAKE_LOG=str(where / "calls"), FAKE_TIMED=str(where / "timed"),
                       FAKE_HELD=str(where / "held"), FAKE_FAILS=str(where / "fails"),
                       FAKE_REFERRERS="${plantedReferrers}", HOME=str(where))
            ran = subprocess.run(["${battery}/bin/ronoctua-fake-battery"], env=env, capture_output=True, text=True,
                                 cwd=where)
            calls = [l.split() for l in (where / "calls").read_text().splitlines()]
            return ran, calls, timed(where / "timed"), (where / "timed").read_text().splitlines()

        ran, calls, runs, _ = battery("whole")
        said = ran.stdout + ran.stderr
        if ran.returncode != 0:
            wrong.append(f"whole: the battery failed ({ran.returncode}): {said[-1500:]}")
        m = re.search(r"what is no unit: --max-jobs (\d+) --cores (\d+) \(\d+ MiB each\)", said)
        must(m and m.group(0), "the battery's sizing line")
        j, c = m.groups()
        hc = {mm.group(1): mm.group(2) for mm in re.finditer(r"-> (\S+) \(heavy, alone with (\d+) cores", said)}
        builds = [w for w in calls if w[0] == "build"]
        dry = [i for i, w in enumerate(builds) if "--dry-run" in w]
        units = {i for i, w in enumerate(builds) if "--dry-run" not in w and any(x[:-2] in name and not name[x[:-2]].startswith("aggregate") for x in w if x.endswith("^*"))}
        if len(dry) != 1 or max(dry) > min(units, default=0):
            wrong.append(f"whole: the dry run was not asked once, before any build: {dry}")
        else:
            asked = {x[:-2] for x in builds[dry[0]] if x.endswith("^*")}
            if asked != {D[i] for i in D if not i.startswith("aggregate")}:
                wrong.append(f"whole: the dry run asked of {sorted(name.get(x, x) for x in asked)}, not every unit")
        built = {name.get(x, "one-check" if x == f["oneCheck"] else x) for x in runs}
        want = {i for i in D if i != "release/libs/d"} | {"one-check"}
        if built != want:
            wrong.append(f"whole: built {sorted(built)}, not {sorted(want)}: a unit the dry run did not list is never built")
        flat, _ = together(runs)
        for x, rs in runs.items():
            i = name.get(x)
            if i is None or i.startswith("aggregate"):
                continue
            s, e, cores, jobs = rs[0]
            heavy = f["classes"].get(i) == "heavy"
            if (cores, jobs) != ((hc.get(i, "?") if heavy else "1"), "1") or len(rs) != 1:
                wrong.append(f"whole: {i} built {len(rs)} time(s), --max-jobs {jobs} --cores {cores}")
            for dep in f["deps"][i]:
                if D[dep] in runs and runs[D[dep]][0][1] > s:
                    wrong.append(f"whole: {i} started before {dep}, which it is built from, ended")
            if heavy and overlapping(flat, x):
                wrong.append(f"whole: the heavy {i} ran beside {[name.get(o, o) for o in overlapping(flat, x)]}")
        compiled_end = max(rs[0][1] for x, rs in runs.items() if name.get(x, "").startswith("release/"))
        run_x = runs.get(D["test/testRuns/x"], [(0, 0, 0, 0)])[0]
        agg = runs.get(D["aggregate/rust-tests"], [(0, 0, 0, 0)])[0]
        check = next((rs[0] for x, rs in runs.items() if x == f["oneCheck"]), (0, 0, 0, 0))
        if run_x[0] < compiled_end:
            wrong.append("whole: the test run started before every compilation ended")
        if agg[0] < run_x[1] or (agg[2], agg[3]) != (c, j):
            wrong.append(f"whole: the aggregate was not built after the run, sized as what is no unit: {agg}")
        if check[0] < agg[1]:
            wrong.append("whole: the checks were built before the Rust build ended")
        # THE PLAN: every unit after what it is built from, the longest
        # chain above first, the test runs after every other line, each with
        # its planted class.
        lines = [l.split() for l in f["lines"]]
        at = {w[0]: n for n, w in enumerate(lines)}
        for n, w in enumerate(lines):
            if w[1] != f["classes"][w[0]] or w[4] != D[w[0]]:
                wrong.append(f"the plan says {w[:5]}, not the planted unit")
            if sorted(int(x) for x in w[5:]) != sorted(at[dep] for dep in f["deps"][w[0]] if dep in at):
                wrong.append(f"the plan's {w[0]} is built from lines {w[5:]}, not what it is built from")
            if any(int(x) >= n for x in w[5:]):
                wrong.append(f"the plan puts {w[0]} before a unit it is built from")
            if (n >= f["lateFrom"]) != w[0].startswith("test/testRuns/"):
                wrong.append(f"the plan's line {n}, {w[0]}, is on the wrong side of the test runs (from {f['lateFrom']})")

        # A UNIT FAILS: built once in the whole battery, and nothing that
        # needs it handed a build.
        ran, calls, runs, timed_lines = battery("fails", [D["release/libs/b"]])
        said = ran.stdout + ran.stderr
        starts = [l.split()[1] for l in timed_lines if l.startswith("start ")]
        never = ["release/libs/h1", "release/libs/c", "release/bins/h2", "test/testRuns/x", "aggregate/rust-tests"]
        if ran.returncode == 0:
            wrong.append("fails: a battery with a failed unit answered yes")
        if starts.count(D["release/libs/b"]) != 1:
            wrong.append(f"fails: the failed unit was built {starts.count(D['release/libs/b'])} times in the battery, not once")
        handed = [i for i in never if D[i] in starts or any(D[i] + "^*" in w for w in calls if w[0] == "build" and "--dry-run" not in w)]
        if handed or f["oneCheck"] in starts:
            wrong.append(f"fails: {handed + (['one-check'] if f['oneCheck'] in starts else [])} were handed a build, needing the failed unit")
        if {name.get(x) for x in starts} - {None} != {"release/deps/a", "release/libs/b", "release/libs/m", "release/libs/e"}:
            wrong.append(f"fails: built {sorted(name.get(x, x) for x in starts)}, not what does not need the failed unit")
        for needle in ("-- 4 unit(s) built from release/libs/b are not built, because it failed",
                       f"<- {D['aggregate/rust-tests']}: not built: it needs a unit phase 1 did not build",
                       "<- one-check: not built: it needs a unit phase 1 did not build"):
            if needle not in said:
                wrong.append(f"fails: it did not say {needle!r}: {said[-1200:]!r}")

        # A TERM DURING AN EVALUATION OF PHASE 0: the battery ends at once,
        # and the evaluation with it — a trap waits for a command in the
        # foreground to end, so every one runs in the background.
        where = tmp / "signalled"
        where.mkdir()
        (where / "timed").write_text("")
        env = dict(os.environ, FAKE_LOG=str(where / "calls"), FAKE_TIMED=str(where / "timed"), FAKE_EVAL_SLEEP="30",
                   FAKE_REFERRERS="${plantedReferrers}", HOME=str(where))
        proc = subprocess.Popen(["${battery}/bin/ronoctua-fake-battery"], env=env, cwd=where,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        deadline = time.time() + 20
        while time.time() < deadline and not any(l.startswith("eval ") for l in (where / "timed").read_text().splitlines()):
            time.sleep(0.1)
        sent = time.time()
        os.kill(proc.pid, signal.SIGTERM)
        try:
            proc.communicate(timeout=20)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.communicate()
        took = time.time() - sent
        marks = [l.split() for l in (where / "timed").read_text().splitlines()]
        evals = [int(w[1]) for w in marks if w[0] == "eval"]
        killed = [int(w[1]) for w in marks if w[0] == "killed-eval"]
        alive = [pid for pid in evals if pathlib.Path(f"/proc/{pid}").exists()]
        if proc.returncode != 143 or took > 10 or not evals or sorted(killed) != sorted(evals) or alive:
            wrong.append(f"signalled: a TERM during an evaluation answered {proc.returncode} after {took:.1f} s; "
                         f"evaluations {evals}, killed {killed}, alive {alive}: not at once, with the evaluation stopped")

        floor(len(lines), 8, "lines of the planted plan")
        floor(len(runs), 4, "builds the failing battery ran")
        if wrong:
            print("ERROR: the batteries do not build each unit by its class, a failed one once:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: one dry run, each unit its own build by its class (heavy alone with {sorted(set(hc.values()))} cores), the test run after the compilations, then the aggregate and the checks; a failed unit built once, what needs it said and failed by name; a TERM during an evaluation acted on at once")
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

  # `.#rust-peaks` HOLDS EACH UNIT TO ITS CLASS, WITH THE HEADROOM, AND
  # WRITES THE CLASSES — BY A MEASUREMENT AT THE CORES THE CLASS BUILDS WITH.
  #
  # Over planted measurements (each a peak and its cores) and planted
  # classes, each unit judged by the slot of the class it is built by
  # (`classesOf`, the plans' resolution): a light unit past the light slot
  # fails it and is the one named; one inside the slot but past it with the
  # headroom does not — the headroom is the margin between the measurement
  # that wrote a class and the one that judges it — and is said, as is one
  # at the slot itself; a medium unit past the light slot and under its own
  # does not; a unit the data does not name, past the medium slot, fails it
  # (it is medium until measured); a unit a lock change renamed is judged by
  # the class it carried, and fails it when measured past it; a heavy unit
  # past every slot does not, and one past what a heavy unit is planned at
  # with its cores is said; a unit never built is not counted; a light unit
  # measured at twelve cores past its slot is neither judged nor moved, and
  # is said by name with no command (its next sized build measures it again;
  # a rooted measurement is not deleted), and it fails nothing: with the one
  # light unit past its slot back under it, the report passes while that one
  # is still at twelve cores. What was measured into another class is said: a heavy
  # unit measured at ONE core under the third class's slot leaves heavy, one
  # measured alone at its twelve cores under a lighter slot is said and
  # stays heavy. `--write` puts every unit measured at one core in the first
  # class whose slot holds its peak and the headroom more, a heavy one
  # included; keeps the class of a unit not measured, or measured at other
  # cores; writes the carried class of a renamed unit not measured since;
  # drops a unit the index no longer has — and what it writes is what
  # `nix/build-sizing.nix` reads. With the light unit back under its slot,
  # it passes. Over data written for other slots, a unit `--write` did not
  # measure here at one core keeps the first class whose slot holds the one
  # it was written for — a large one of a larger old slot goes heavy, one of
  # an old class of a smaller slot large —, one no heavier than the class of
  # a unit no measurement names is dropped (it is built as that), and it
  # says how many of each. And a measurement of another format (the largest
  # process's RSS with no cores, or the PSS of the tree) is refused with a
  # sentence naming it, and `--write` writes nothing.
  rust-peaks-names-what-is-past-the-slot =
    mkCheck "rust-peaks-names-what-is-past-the-slot" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import json, os, pathlib, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the report, as JSON and as the lines it prints, and the
        # file it writes, as JSON.
        # FILE-WIDE: the report's text and its errors are searched whole for
        # each sentence: the question is what it says, wherever it says it.
        N = json.loads('${numbersJson}')
        P = json.load(open("${peaks}"))
        tmp = pathlib.Path(os.environ["TMPDIR"])
        LIGHT, MEDIUM, LARGE = (N["classes"][i]["name"] for i in range(3))
        SLOTS = {c["name"]: c["slotMiB"] for c in N["classes"]}
        H = N["headroomPercent"]
        RENAMED = "release/deps/renamed@1.0.0/lib:renamed/build/o3/bbbbbbbbbbbb"
        MOVED = "release/deps/moved@1.0.0/lib:moved/build/o3/dddddddddddd"
        def measured(name, mib, cores, seconds=1.5, old=False, fmt=3):
            d = tmp / name.replace("/", "_")
            d.mkdir(exist_ok=True)
            (d / "peak-kib").write_text(f"{mib * 1024}\n")
            (d / "seconds").write_text(f"{seconds}\n")
            if not old:
                (d / "format").write_text(f"{fmt}\n")
                (d / "largest-kib").write_text(f"{mib * 1024 // 2}\n")
                (d / "cores").write_text(f"{cores}\n")
            return str(d)
        units = {}
        for uid, u in P["units"].items():
            metrics = None if u["kind"] == "aggregate" else str(tmp / "never-built") if u["mib"] is None else measured(uid, u["mib"], u["cores"])
            units[uid] = {"kind": u["kind"], "flavour": u["flavour"], "metrics": metrics}
        classes = {"format": 2, "headroomPercent": H, "slots": SLOTS, "units": P["classes"]}
        facts = {"units": units, "classes": classes, "resolved": P["resolved"], "how": P["how"],
                 "order": [c["name"] for c in N["classes"]],
                 "slots": SLOTS, "headroomPercent": H, "unclassed": N["unclassed"],
                 "kinds": ["deps", "libs", "scripts", "bins", "testBins", "testRuns", "clippy", "fmt"],
                 "shellKiB": N["shellMiB"] * 1024, "besideKiB": N["besideMiB"] * 1024}
        f = tmp / "facts.json"
        f.write_text(json.dumps(facts))
        SCRIPT = "${../nix/scripts/rust-peaks.py}"
        def ask(*args):
            return subprocess.run(["python3", SCRIPT, *args, str(f)], capture_output=True, text=True)
        wrong = []
        if (P["how"][RENAMED], P["resolved"][RENAMED], P["how"][MOVED]) != ("carried", LIGHT, "carried"):
            wrong.append(f"the renamed units were not carried by classesOf: {P['how'][RENAMED]} {P['resolved'][RENAMED]} {P['how'][MOVED]}")
        if (SLOTS[MEDIUM] + 10) * (100 + H) > SLOTS[LARGE] * 100:
            wrong.append(f"the planted demoted unit ({SLOTS[MEDIUM] + 10} MiB) does not fit the third class's slot {SLOTS[LARGE]} with the headroom")
        ran = ask("--json")
        r = json.loads(ran.stdout)
        if ran.returncode != 1:
            wrong.append(f"a light unit past its slot answered {ran.returncode}, not 1")
        want_past = ["release/libs/new", MOVED, "release/libs/grew"]
        if r["past-the-slot"] != want_past:
            wrong.append(f"past the slot: {r['past-the-slot']}, not {want_past} (the unnamed one past medium, the carried one, the light one past light)")
        if r["within-the-headroom"] != ["test/clippy/edge", "test/clippy/full"]:
            wrong.append(f"inside the slot, past it with the headroom: {r['within-the-headroom']}, not the edge and the full one")
        if ("release/libs/grew" not in ran.stderr or "test/clippy/edge" in ran.stderr or "test/clippy/full" in ran.stderr
                or "release/deps/mid" in ran.stderr or "test/clippy/wide" in ran.stderr):
            wrong.append(f"the error names {ran.stderr!r}")
        if r["heavy-past-the-plan"] != ["release/bins/shell"] or len(r["measured"]) != 11 or r["judged"] != 7:
            wrong.append(f"heavy past the plan {r['heavy-past-the-plan']}, measured {sorted(r['measured'])}, judged {r['judged']}")
        moves = {u: (m["from"], m["to"]) for u, m in r["would-move"].items()}
        want_moves = {"release/libs/grew": (LIGHT, MEDIUM), "release/libs/new": (None, LARGE),
                      "test/clippy/edge": (LIGHT, MEDIUM), "test/clippy/full": (LIGHT, MEDIUM), MOVED: (None, MEDIUM),
                      "release/libs/demoted": ("heavy", LARGE)}
        if moves != want_moves:
            wrong.append(f"would move {moves}, not {want_moves}")
        gone = sorted(["release/libs/gone", "release/deps/renamed@1.0.0/lib:renamed/build/o3/aaaaaaaaaaaa",
                       "release/deps/moved@1.0.0/lib:moved/build/o3/cccccccccccc"])
        if r["unclassed"] != ["release/libs/new"] or r["carried"] != [MOVED, RENAMED] or r["gone"] != gone:
            wrong.append(f"unclassed {r['unclassed']}, carried {r['carried']}, gone {r['gone']}")
        if r["could-leave-heavy"] != ["release/libs/footer"]:
            wrong.append(f"a heavy unit measured at twelve cores under the medium slot is not said: {r['could-leave-heavy']}")
        if r["other-cores"] != ["test/clippy/wide"] or r["other-format"]:
            wrong.append(f"measured at other cores {r['other-cores']}, of another format {r['other-format']}")
        said = r["other-cores-said"] or ""
        if not said.startswith("1 unit(s) of a class with a slot were measured at other cores") or "test/clippy/wide" not in said:
            wrong.append(f"the units at other cores are not said by name and how many: {said!r}")
        if "nix store delete" in said or "nix-store" in said:
            wrong.append(f"the note gives a command, which a kept root refuses: {said!r}")
        text = ask().stdout
        for needle in ("measured: 11 of 14 units", "7 of them at the 1 core", "release/bins/shell:", "--write` moves them",
                       "release/libs/footer is heavy and measured under a lighter class's slot alone at 12 cores",
                       "note: 1 unit(s) of a class with a slot were measured at other cores", "test/clippy/wide",
                       "2 unit(s) are inside the slot of their class but past it with the headroom",
                       "until its next sized build (`.#sized-build`, a battery, a machine's apply)",
                       "release/libs/demoted: heavy -> " + LARGE,
                       "built as " + N["unclassed"], "2 unit(s) a lock change renamed carry the class",
                       "no unit any more"):
            if needle not in text:
                wrong.append(f"the report does not say {needle!r}:\n{text}")

        # --write: the classes as measured.
        out = tmp / "unit-classes.json"
        ran = ask("--write", str(out))
        written = json.loads(out.read_text()) if out.exists() else {}
        want = {"release/libs/small": LIGHT, "release/libs/grew": MEDIUM, "release/deps/mid": MEDIUM,
                "release/libs/new": LARGE, "release/bins/shell": "heavy", "release/bins/other": "heavy",
                "release/libs/footer": "heavy", "release/libs/demoted": LARGE, "test/clippy/edge": MEDIUM,
                "test/clippy/full": MEDIUM, "test/clippy/wide": LIGHT, "test/testBins/never": MEDIUM, RENAMED: LIGHT,
                MOVED: MEDIUM}
        if ran.returncode != 0 or written.get("units") != want:
            wrong.append(f"--write wrote {written.get('units')} ({ran.returncode} {ran.stderr!r}), not {want}")
        if (written.get("format"), written.get("slots"), written.get("headroomPercent")) != (2, SLOTS, H):
            wrong.append(f"--write wrote the numbers {written.get('format')} {written.get('slots')} {written.get('headroomPercent')}")
        lines = out.read_text().splitlines() if out.exists() else []
        if sum(1 for l in lines if l.startswith('"release/') or l.startswith('"test/')) != len(want):
            wrong.append("--write did not write one unit a line")
        # And what it wrote is what the build reads, with no complaint: every
        # unit now named (its resolution the trivial one, no stem left to
        # carry from).
        after = dict(facts, classes=written,
                     resolved={u: written["units"].get(u, N["unclassed"]) for u in P["resolved"]},
                     how={u: "named" if u in written["units"] else "default" for u in P["how"]})
        after["units"] = dict(units)
        after["units"]["release/libs/grew"] = dict(units["release/libs/grew"], metrics=measured("grew2", 100, 1))
        f.write_text(json.dumps(after))
        ran = ask("--json")
        if ran.returncode != 0:
            wrong.append(f"after --write, and the unit back under its slot, it still failed: {ran.stderr!r}")
        elif json.loads(ran.stdout)["other-cores"] != ["test/clippy/wide"]:
            wrong.append("after --write, the unit at twelve cores is not the one that passes measured at other cores")

        # WRITTEN FOR OTHER NUMBERS: only what this store measured at one
        # core is classed again; a unit it did not keeps a slot no smaller
        # than the one it was written for — the large one of an old large
        # slot past the new one goes heavy (it peaks past the new slot, at
        # one core, measured only at twelve), the one of an old class whose
        # slot the new large holds goes large —; one whose slot is no larger
        # than the medium one is dropped, and built as medium.
        BIG, BIGGER = "release/libs/big", "release/libs/bigger"
        stale_units = dict(units)
        stale_units[BIG] = {"kind": "libs", "flavour": "release", "metrics": measured("big", SLOTS[LARGE] - 200, 12)}
        stale_units[BIGGER] = {"kind": "libs", "flavour": "release", "metrics": str(tmp / "never-built")}
        stale = dict(facts, units=stale_units,
                     classes=dict(classes, units=dict(P["classes"], **{BIG: LARGE, BIGGER: "huge"}),
                                  slots=dict(SLOTS, **{LIGHT: SLOTS[LIGHT] - 68, LARGE: SLOTS[LARGE] + 84,
                                                       "huge": SLOTS[LARGE] - 100})),
                     resolved=dict(P["resolved"], **{BIG: LARGE, BIGGER: N["unclassed"]}),
                     how=dict(P["how"], **{BIG: "named", BIGGER: "named"}))
        f.write_text(json.dumps(stale))
        ran = ask("--write", str(out))
        written = json.loads(out.read_text()) if out.exists() else {}
        kept = written.get("units", {})
        if ran.returncode != 0 or "test/testBins/never" in kept or RENAMED in kept or "test/clippy/wide" in kept:
            wrong.append(f"over stale data, --write kept a class no heavier than medium it did not measure at one core: {sorted(kept)}")
        if kept.get("release/bins/other") != "heavy" or kept.get("release/libs/footer") != "heavy":
            wrong.append("over stale data, --write dropped a heavy unit it did not measure at one core")
        if (kept.get(BIG), kept.get(BIGGER)) != ("heavy", LARGE):
            wrong.append(f"over stale data, the large unit of a larger old slot went {kept.get(BIG)} (not heavy), "
                         f"the one of a smaller old class {kept.get(BIGGER)} (not {LARGE})")
        if ("of the units this store did not measure at one core, 4 keep the first class whose slot holds the one they "
                "were written for (the heavy ones stay heavy), and 2 are dropped from it and built as " + N["unclassed"]) not in ran.stdout:
            wrong.append(f"over stale data, --write did not say what it kept and dropped: {ran.stdout!r}")

        # ANOTHER FORMAT: the largest process's RSS and no cores (format 1),
        # or the PSS of the tree (format 2), refused by name; and nothing
        # is written.
        old = dict(facts)
        old["units"] = dict(units)
        old["units"]["release/libs/small"] = dict(units["release/libs/small"], metrics=measured("small-old", 100, 1, old=True))
        old["units"]["release/deps/mid"] = dict(units["release/deps/mid"], metrics=measured("mid-pss", 100, 1, fmt=2))
        f.write_text(json.dumps(old))
        ran = ask()
        if (ran.returncode != 1 or "release/libs/small (format 1)" not in ran.stderr
                or "release/deps/mid (format 2)" not in ran.stderr or "another format than 3" not in ran.stderr):
            wrong.append(f"measurements of formats 1 and 2 were not refused by name: {ran.returncode} {ran.stderr!r}")
        before = out.read_text()
        ran = ask("--write", str(out))
        if ran.returncode != 1 or out.read_text() != before or "nothing is written" not in ran.stderr:
            wrong.append(f"--write over a measurement of format 1 wrote, or did not say it would not: {ran.returncode} {ran.stderr!r}")

        floor(len(units), 15, "planted units")
        if wrong:
            print("ERROR: .#rust-peaks does not hold each unit to its class:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the units past their class's slot at one core named and failing, those inside it within the headroom said, a carried class judged, "
              "a measure at other cores neither judged nor moved nor failing and said by name, a heavy unit leaving by its one-core peak, the moves said, --write writes the "
              "classes as measured, keeps over stale data a slot no smaller than the one written, and a measurement of another format is refused")
        END
      '';

  # THE COMMITTED CLASSES HOLD EVERY UNIT AS IT MEASURES NOW.
  #
  # The check `nix/rust/unit-classes.json` is held to, as `plan.json` is to
  # the planner: over every unit's own measurement (`metrics`, an input of
  # this check, so every unit is built before it — at least two thousand),
  # `.#rust-peaks` finds no unit that is not heavy whose whole build's peak
  # at one core is past the slot of the class it is built by (named, or
  # carried from the unit a lock change renamed) — the slot it is started by
  # beside others in every build that schedules it —, and no measurement of
  # another format. A unit measured into another class, or the data naming
  # none, is said and passes: built by a class it fits, nothing is
  # oversubscribed. A unit of a class with a slot measured at other cores
  # than the one its class builds it with — by a `nix build` that was not
  # sized (`nix flake check`, `.#vm`, `.#iso`, `.#png`) — is judged by no
  # measurement, and is said by name and how many, never failed and with no
  # command: its next sized build measures it again, and the store refuses
  # to delete a measurement a root holds, which a battery's
  # `ronoctua-keep-sources` makes of the release graph's units and the
  # loop's third-party ones. The floor counts every unit measured, at any
  # cores.
  unit-classes-fit-the-measures =
    let
      planned = lib.filterAttrs (_: e: lib.elem e.kind sizing.plannedKinds) rust.unitIndex;
      raw = lib.importJSON sizing.classesFile;
      found = sizing.classesOf {
        units = lib.attrNames planned;
        classes = raw.units;
      };
      facts = pkgs.writeText "unit-measures.json" (
        builtins.toJSON {
          units = lib.mapAttrs (_: e: {
            inherit (e) kind flavour;
            # WITH ITS CONTEXT: the measurement is read in the sandbox.
            metrics = builtins.appendContext e.metrics { ${e.drv}.outputs = [ "metrics" ]; };
          }) (lib.filterAttrs (_: e: e.metrics != null) planned);
          classes = raw;
          resolved = lib.mapAttrs (_: f: f.class) found;
          how = lib.mapAttrs (_: f: f.how) found;
          order = map (c: c.name) sizing.numbers.classes;
          slots = lib.listToAttrs (map (c: lib.nameValuePair c.name c.slotMiB) sizing.numbers.classes);
          inherit (sizing.numbers) headroomPercent unclassed;
          kinds = sizing.plannedKinds;
          shellKiB = sizing.numbers.shellMiB * 1024;
          besideKiB = sizing.numbers.besideMiB * 1024;
        }
      );
    in
    mkCheck "unit-classes-fit-the-measures" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, subprocess, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the report of `.#rust-peaks` over every unit's measurement,
      # as JSON.
      ran = subprocess.run(["python3", "${../nix/scripts/rust-peaks.py}", "--json", "${facts}"],
                           capture_output=True, text=True)
      r = json.loads(ran.stdout)
      # A CHECK THAT STOPPED REACHING THE UNITS: their measurements are its
      # inputs, whatever cores they were taken at.
      floor(len(r["measured"]), 2000, "units measured")
      if ran.returncode != 0:
          print("ERROR: a unit outgrew the slot of its class in nix/rust/unit-classes.json, "
                "or a measurement is of another format:", file=sys.stderr)
          print(ran.stderr, file=sys.stderr)
          print("       Run, after a build of the units: nix run .#rust-peaks -- --write", file=sys.stderr)
          sys.exit(1)
      if r["other-cores-said"]:
          print("NOTE: " + r["other-cores-said"])
      print(f"OK: {len(r['measured'])} units measured, {r['judged']} at one core and none of those past the slot of "
            f"its class; {len(r['within-the-headroom'])} inside it within the headroom, {len(r['would-move'])} measured "
            f"into another class, {len(r['unclassed'])} with none, {len(r['carried'])} carried, "
            f"{len(r['other-cores'])} at other cores (said by .#rust-peaks)")
      END
    '';

  # THE CLASS DATA NAMES UNITS OF THE INDEX, AND A RENAMED UNIT CARRIES ITS
  # CLASS.
  #
  # Evaluation only (~1 s to build): every id `nix/rust/unit-classes.json`
  # names is a unit of `rust.unitIndex` — a lock change renames the
  # third-party units (their ids end in a digest of what they are built
  # with), and data still naming the old ids would leave the new ones
  # planned by no measurement, in silence; `nix run .#rust-peaks -- --write`
  # drops the old ids and writes the carried class of each new one. And
  # `classesOf` carries a class only across that digest: over planted ids,
  # a unit renamed from two named ones of different classes takes the
  # heavier, one whose crate's VERSION changed takes none, one of our own
  # crates (no digest) none, and a named one its own.
  unit-classes-name-units-of-the-index =
    let
      named = lib.attrNames (lib.importJSON sizing.classesFile).units;
      gone = lib.filter (id: !(rust.unitIndex ? ${id})) named;
      dep = v: digest: "release/deps/x@${v}/lib:x/build/o3/${digest}";
      found = sizing.classesOf {
        units = [
          (dep "1.0.0" "222222222222")
          (dep "1.0.1" "555555555555")
          "release/libs/ours"
          (dep "1.0.0" "111111111111")
        ];
        classes = {
          ${dep "1.0.0" "111111111111"} = "light";
          ${dep "1.0.0" "333333333333"} = "medium";
          "release/libs/ours-before" = "light";
        };
      };
      planted = lib.mapAttrs (_: f: "${f.class} ${f.how}") found;
      want = {
        ${dep "1.0.0" "222222222222"} = "medium carried";
        ${dep "1.0.1" "555555555555"} = "${sizing.numbers.unclassed} default";
        "release/libs/ours" = "${sizing.numbers.unclassed} default";
        ${dep "1.0.0" "111111111111"} = "light named";
      };
      facts = pkgs.writeText "unit-classes-index.json" (
        builtins.toJSON {
          inherit gone planted want;
          named = lib.length named;
        }
      );
    in
    mkCheck "unit-classes-name-units-of-the-index" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, sys
      exec(open("${pyPrelude}").read())

      # PARSED: what the evaluation found, as JSON.
      f = json.load(open("${facts}"))
      floor(f["named"], 2000, "units the class data names")
      wrong = []
      if f["gone"]:
          wrong.append(f"nix/rust/unit-classes.json names {len(f['gone'])} id(s) that are no unit of the index, e.g. "
                       f"{f['gone'][:3]}: run nix run .#rust-peaks -- --write (no build needed: it drops them, and the "
                       "units a lock change renamed carry their class)")
      if f["planted"] != f["want"]:
          wrong.append(f"classesOf found {f['planted']}, not {f['want']}")
      if wrong:
          print("ERROR: the class data does not name the units of the index:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: the {f['named']} ids of the class data are units of the index, and a renamed unit carries the heavier class across its digest only")
      END
    '';

  # A MACHINE'S APPLY BUILDS BY THE DISTRO'S PLAN, AND KEEPS ITS UNITS.
  #
  # The reference system's `ronoctua.machine.phases`, as `nix eval` gives it
  # to `ronoctua-machine apply`: the plan of every release unit, each line
  # its id, the class it is built by, how that was found, its cost (the
  # class's slot, or a heavy unit's `alone`), its `.drv` and the lines it is
  # built from — and no unit of the loop or the test graph; the system's own
  # derivation; and the units to keep — every output of the release set
  # (`metrics` beside `out`, or the batteries' `^*` builds a kept unit again
  # once a collection took its `metrics`), the shell's program among them,
  # no unit of the loop or the test graph. Then the EXTRACTED
  # `ronoctua-machine` (the one the units run as root) over that very JSON,
  # with the machine's own sizing and units programs and a `nix` that writes
  # down what it was asked, when each build ran, and says, for a dry run of
  # the system, the plan planted. With the whole release graph planned it
  # builds every unit once, its own build, never before what it is built
  # from, the heavy ones alone with the cores their lines leave, the costs
  # of what runs within the budget; then the system, sized as what is no
  # unit; with a store that holds the shell's bundle (the seed's, the
  # person's cache's) it builds the system and not one unit, though every
  # unit is in the system's `.drv` closure. Both hand the release set to
  # what keeps it — once the build worked.
  #
  # AND ACROSS THE VERSION BOUNDARY: a plan of a newer distro — classes no
  # distro has today, at slots none plans — is built by its lines' numbers
  # by the running system's programs: six units of a 300 MiB class, three of
  # which fit the planted budget at once and four do not, ran three at once,
  # though their lines say how their class was found in a word no distro
  # says today; a unit `alone:500+100` got the seven cores those numbers
  # leave (the running system's own heavy numbers would give one); and what
  # the units are built from that is no unit — in the system's own dry
  # run, below the giant in the store's references — was built first,
  # alone, sized as what is no unit, with no second dry run.
  machine-builds-by-the-distros-plan =
    mkCheck "machine-builds-by-the-distros-plan" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import json, os, pathlib, re, subprocess, sys
        exec(open("${pyPrelude}").read())
        exec(open("${timedPy}").read())

        # PARSED: the evaluated phases and facts, as JSON; the program's
        # format, its one `pub const` line; and what the fakes were asked.
        # FILE-WIDE: the constant is the crate's, in a file read whole.
        facts = json.load(open("${machineFacts}"))
        N = json.loads('${numbersJson}')
        p = facts["phases"]
        crate = uncommented(pathlib.Path("${../crates/ronoctua-machine/src/build.rs}").read_text())
        fmt = must(next((l for l in crate.splitlines() if l.startswith("pub const PHASES_FORMAT: u32 = ")), None),
                   "PHASES_FORMAT in ronoctua-machine's build.rs")
        SLOT = {c["name"]: c["slotMiB"] for c in N["classes"]}
        def cost_of(cls):
            return f"alone:{N['shellMiB']}+{N['besideMiB']}" if cls == "heavy" else str(SLOT[cls])
        wrong = []
        if str(p["format"]) != fmt.rsplit("= ", 1)[1].rstrip(";"):
            wrong.append(f"the module says format {p['format']}, the program reads {fmt}")
        lines = [l.split() for l in p["units"]]
        at = {w[0]: n for n, w in enumerate(lines)}
        rel = facts["release"]
        floor(len(lines), 300, "release units in the plan")
        if set(at) != set(rel):
            wrong.append(f"the plan holds {len(at)} units, the release graph {len(rel)}: "
                         f"{sorted(set(at) ^ set(rel))[:3]}")
        for n, w in enumerate(lines):
            uid = w[0]
            if uid not in rel:
                continue
            fo = facts["found"][uid]
            if w[1:5] != [fo["class"], fo["how"], cost_of(fo["class"]), rel[uid]["drv"]]:
                wrong.append(f"the plan says {w[:5]}, not the unit, the class it is built by, how, its cost and its .drv")
            if sorted(int(x) for x in w[5:]) != sorted(at[d] for d in rel[uid]["deps"] if d in at):
                wrong.append(f"the plan's {uid} is built from other lines than what it is built from")
        heavy_ids = {w[0] for w in lines if w[3].startswith("alone:")}
        if "release/bins/ronoctua-shell" not in heavy_ids:
            wrong.append(f"the shell's program is not built alone in the plan: {sorted(heavy_ids)}")
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
        # release graph whole, and the plan of a machine whose store holds
        # the shell's bundle — freshly installed from the seed, or fed by the
        # person's cache — where Nix builds the system and nothing below it.
        MACHINE = "${rust.machine}/bin/ronoctua-machine"
        PY = "${pkgs.python3}/bin/python3"
        def plan_of(drvs):
            return f"these {len(drvs)} derivations will be built:\n" + "".join(f"  {d}\n" for d in drvs)
        def apply(name, phases, plan, needs=(), times=None, units_args=(), refs=None):
            root = pathlib.Path(os.environ["TMPDIR"]) / name
            store = root / "nix/store"
            store.mkdir(parents=True)
            def program(path, text):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("#!" + PY + "\n" + text)
                path.chmod(0o755)
            (root / "phases.json").write_text(json.dumps(phases))
            (root / "plan").write_text(plan)
            (root / "needs").write_text("".join(d + "\n" for d in needs))
            (root / "times").write_text(json.dumps(times or {}))
            (root / "references.json").write_text(json.dumps(refs or {}))
            # THE STORE'S REFERENCES, as the units' program asks them (the
            # `nix-store` beside the machine's `nix`): what each derivation
            # asked is built from, directly, from `refs`.
            program(root / "bin/nix-store", """
        import json, os, pathlib, sys
        root = pathlib.Path(os.environ["FAKE_ROOT"])
        a = sys.argv[1:]
        with open(root / "calls", "a") as f:
            f.write("nix-store " + " ".join(a) + "\\n")
        refs = json.loads((root / "references.json").read_text())
        if a[:2] == ["--query", "--references"]:
            print("\\n".join(sorted({r for x in a[2:] for r in refs.get(x, [])})))
        elif a[:2] == ["--query", "--referrers-closure"]:
            print("\\n".join(a[2:]))
        else:
            sys.exit(f"fake nix-store: {a}")
        """)
            program(root / "bin/nix", """
        import json, os, pathlib, sys, time
        root = pathlib.Path(os.environ["FAKE_ROOT"])
        a = sys.argv[1:]
        with open(root / "calls", "a") as f:
            f.write("nix " + " ".join(a) + "\\n")
        phases = json.loads((root / "phases.json").read_text())
        if a[0] == "eval":
            print((root / "phases.json").read_text())
        elif a[0] == "build" and "--dry-run" in a:
            asked = [x[:-2] for x in a if x.endswith("^*")]
            if phases["system"] in asked:
                sys.stderr.write((root / "plan").read_text())
            else:
                said = list(dict.fromkeys(asked + (root / "needs").read_text().split()))
                sys.stderr.write(f"these {len(said)} derivations will be built:\\n" + "".join(f"  {d}\\n" for d in said))
        elif a[0] == "build":
            ds = [x[:-2] for x in a if x.endswith("^*")]
            cores, jobs = a[a.index("--cores") + 1], a[a.index("--max-jobs") + 1]
            with open(root / "timed", "a") as f:
                for d in ds:
                    f.write(f"start {d} {cores} {jobs} {int(time.time() * 1e6)} {os.getpid()}\\n")
            times = json.loads((root / "times").read_text())
            time.sleep(times.get(ds[0], 0.01) if len(ds) == 1 else 0.01)
            with open(root / "timed", "a") as f:
                for d in ds:
                    f.write(f"end {d} {int(time.time() * 1e6)}\\n")
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
            # THE MACHINE'S OWN PROGRAMS, and what they said to the apply
            # written down: memory moves between two runs, and the apply is
            # held to the numbers it was given.
            program(root / "bin/sizing", f"""
        import pathlib, subprocess, sys
        said = subprocess.run(["{facts['sizing']}"], capture_output=True, text=True)
        pathlib.Path("{root}/sized").write_text(said.stdout)
        sys.stdout.write(said.stdout)
        sys.exit(said.returncode)
        """)
            program(root / "bin/build-units", f"""
        import pathlib, subprocess, sys
        with open("{root}/calls", "a") as f:
            f.write("build-units " + " ".join(sys.argv[1:]) + "\\n")
        said = subprocess.run(["{facts['units']}", *{list(units_args)!r}, *sys.argv[1:]], capture_output=True, text=True)
        pathlib.Path("{root}/units-said").write_text(said.stdout + said.stderr)
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
            (root / "timed").write_text("")
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
                "RONOCTUA_BUILD_UNITS": str(root / "bin/build-units"),
                "RONOCTUA_KEEP_UNITS": str(root / "bin/keep-units"),
            }
            ran = subprocess.run([MACHINE, "apply"], env=env, capture_output=True, text=True)
            sized = dict(l.split("=", 1) for l in (root / "sized").read_text().splitlines()) if (root / "sized").exists() else {}
            floor(len(sized), 10, "numbers the machine's sizing said")
            calls = (root / "calls").read_text().splitlines() if (root / "calls").exists() else []
            asked = [c.split() for c in calls if c.startswith("nix build") and "--dry-run" in c.split() and phases["system"] + "^*" in c.split()]
            kept_file = root / "kept"
            if [w[-1] for w in asked] != [phases["system"] + "^*"]:
                wrong.append(f"{name}: Nix was not asked once what the system's build builds: {asked}")
            if not kept_file.exists() or kept_file.read_text().split() != phases["kept"]:
                wrong.append(f"{name}: what keeps the units was not handed its units: {ran.stderr[-1500:]}")
            said = (root / "units-said").read_text() if (root / "units-said").exists() else ""
            return ran, sized, timed(root / "timed"), calls, said

        # The release graph changed whole: every unit its own build, then
        # the system.
        drv_of = {w[4]: w for w in lines}
        everything = [w[4] for w in lines] + [p["system"]]
        ran, sized, runs, calls, said = apply("changed", p, plan_of(everything))
        # The units' program measures for itself: its own numbers, as it
        # said them, are the ones its builds are held to.
        m = re.search(r"the budget: (\d+) MiB", said)
        must(m and m.group(0), "the budget the units' program said")
        budget = int(m.group(1)) * 1024
        m = re.search(r"; (\d+) processors", said)
        cpus = int(must(m and m.group(1), "the processors the units' program said"))
        flat, most = together(runs)
        if ran.returncode != 0:
            wrong.append(f"changed: the apply failed: {ran.stderr[-1500:]}")
        missing = [drv_of[d][0] for d in drv_of if d not in runs]
        twice = [drv_of[d][0] for d, rs in runs.items() if d in drv_of and len(rs) != 1]
        if missing or twice:
            wrong.append(f"changed: {len(missing)} unit(s) not built, e.g. {missing[:3]}; {len(twice)} built more than once")
        client = N["clientMiB"] * 1024
        def weight(d):
            w = drv_of.get(d)
            return 0 if w is None else cost_kb(w[3], budget, cpus, client)[0]
        for d, rs in runs.items():
            w = drv_of.get(d)
            if w is None:
                continue
            s, e, cores, jobs = rs[0]
            if (cores, jobs) != (str(cost_kb(w[3], budget, cpus, client)[1]), "1"):
                wrong.append(f"changed: {w[0]} ({w[3]}) built with --max-jobs {jobs} --cores {cores}")
                break
            late = [lines[int(x)][0] for x in w[5:] if runs.get(lines[int(x)][4], [(0, 0)])[0][1] > s]
            if late:
                wrong.append(f"changed: {w[0]} started before {late[:2]}, which it is built from, ended")
                break
            if w[3].startswith("alone:") and overlapping(flat, d):
                wrong.append(f"changed: the heavy {w[0]} ran beside {len(overlapping(flat, d))} other build(s)")
        heaviest = heaviest_moment(flat, weight)
        if heaviest > budget:
            wrong.append(f"changed: {heaviest // 1024} MiB of slots ran at once, past the {budget // 1024} MiB the program said")
        system_run = runs.get(p["system"], [(0, 0, None, None)])[0]
        if (system_run[2], system_run[3]) != (sized.get("cores"), sized.get("jobs")) or system_run[0] < max(rs[0][1] for d, rs in runs.items() if d in drv_of):
            wrong.append(f"changed: the system was not built last, sized as what is no unit: {system_run}")

        # The shell's bundle in the store, as the seed leaves it: every unit
        # is in the system's `.drv` closure, and none is built.
        ran, sized, runs, calls, said = apply("fresh", p, f"this derivation will be built:\n  {p['system']}\n")
        if sorted(runs) != [p["system"]] or any(c.startswith("build-units") for c in calls):
            wrong.append(f"fresh: a store holding the shell built {len(runs)} derivation(s), not the system alone: {ran.stderr[-1500:]}")

        # A NEWER DISTRO'S PLAN: classes no distro has, at their own slots,
        # and what the units are built from that is no unit. The units'
        # program measures a planted machine: 8 GiB, a budget of 1100 MiB,
        # twelve processors.
        total_kb, budget_mib = 8 * 1024 * 1024, 1100
        avail_kb = budget_mib * 1024 + total_kb * N["floorPercent"] // 100 + N["reserveMiB"] * 1024
        mem = pathlib.Path(os.environ["TMPDIR"]) / "meminfo-newer"
        mem.write_text(f"MemTotal: {total_kb} kB\nMemAvailable: {avail_kb} kB\n")
        feather = [f"/nix/store/{k:032d}-feather-{k}.drv" for k in range(6)]
        giant = "/nix/store/" + "g" * 32 + "-giant.drv"
        lib_ = "/nix/store/" + "l" * 32 + "-libsomething.drv"
        # How a class was found is said in a word no distro says today
        # (`measured`): built, and no refusal. What the giant is built from
        # that is no unit is in the system's own dry run, as Nix lists it.
        newer = dict(p, units=[f"release/libs/f{k} feather measured 300 {d}" for k, d in enumerate(feather)]
                     + [f"release/bins/giant giant named alone:500+100 {giant} 0"], kept=[])
        times = {d: 0.6 for d in feather} | {giant: 0.2, lib_: 0.3}
        ran, sized, runs, calls, said = apply("newer", newer, plan_of(feather + [giant, lib_, newer["system"]]),
                                              times=times, refs={giant: [lib_, feather[0]]},
                                              units_args=("--meminfo", str(mem), "--cpus", "12", "--ceiling", "infinity"))
        again = [c for c in calls if c.startswith("nix build") and "--dry-run" in c.split() and newer["system"] + "^*" not in c.split()]
        if again:
            wrong.append(f"newer: Nix was asked again what the units are built from, after the system's dry run said it: {again[0][:200]}")
        flat, _ = together(runs)
        alive = max((len([d for d, s2, e2 in flat if d in feather and s2 <= s < e2]) for _, s, _ in flat), default=0)
        g = runs.get(giant, [(0, 0, None, None)])[0]
        libr = runs.get(lib_, [(0, 0, None, None)])[0]
        m = re.search(r"what is no unit: --max-jobs (\d+) --cores (\d+)", said)
        if ran.returncode != 0 or f"the budget: {budget_mib} MiB" not in said:
            wrong.append(f"newer: the apply failed or the planted budget was not the program's: {ran.stderr[-800:]} {said[:500]}")
        if alive != 3:
            wrong.append(f"newer: {alive} units of the 300 MiB class ran at once, not the 3 their lines fit in {budget_mib} MiB")
        beside = " beside others" if overlapping(flat, giant) else ""
        if g[2] != "7" or overlapping(flat, giant):
            wrong.append(f"newer: the unit alone:500+100 got --cores {g[2]}{beside}, not the 7 its line leaves")
        if not m or (libr[2], libr[3]) != (m.group(2), m.group(1)) or libr[1] > min((rs[0][0] for d, rs in runs.items() if d in feather + [giant]), default=0):
            wrong.append(f"newer: what the units are built from that is no unit was not built first, sized as what is no unit: {libr}")
        if wrong:
            print("ERROR: a machine's apply does not build by the distro's plan:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(lines)} release units in the plan ({len(heavy_ids)} alone), each its own build within "
              f"{budget // 1024} MiB (at most {heaviest // 1024} at once), the system last; "
              f"the system alone when the store holds the shell; {len(kept)} paths kept; a newer plan built by its own numbers")
        END
      '';

  # `.#sized-build` BUILDS WHAT NIX WOULD BUILD, EACH UNIT BY ITS CLASS.
  #
  # `ronoctua-sized-build` over the planted plan (its classes planted), with
  # a `nix` that writes down every build and when it ran, and answers a dry
  # run of what it was given with the plan planted, and a `nix-store` that
  # answers from the planted references. With the whole graph planned:
  # every unit its own build, by its class, the heavy ones alone and after
  # what they are built from, then what it was given, sized as what is no
  # unit. With a store that holds the units' consumer: only what it was
  # given, though every unit is in its `.drv` closure. With the first heavy
  # unit held: neither it nor what only it is built from, the rest as
  # before. A unit that fails stops it before what it was given, and
  # `--keep-going` builds every unit that does not need it, never what it
  # was given when that needs it (said), and still answers no. A dry run
  # that does not answer, or answers in words it does not read, builds
  # nothing. The test run starts only after every compilation ended, a slow
  # one beside it included (the plan's `lateFrom`). And a TERM sent to it
  # alone stops the builds it started, and waits for them, before its
  # clean-up removes the folder that holds their out-links: it answers 143,
  # every build it started was killed with that folder still there, none
  # outlives it, and the folder is gone; sent during its dry run, it answers
  # 143 at once, the dry run stopped, instead of waiting for it to end.
  sized-build-builds-what-nix-would =
    let
      sized = sizing.sizedBuild plantedPlan;
    in
    mkCheck "sized-build-builds-what-nix-would" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, os, pathlib, re, signal, subprocess, sys, time
      exec(open("${pyPrelude}").read())
      exec(open("${timedPy}").read())

      # PARSED: the planted units' paths, as JSON; what the program said of
      # its sizing, its lines; and every call the fake `nix` got, timed.
      # FILE-WIDE: the fake's log is read whole, and the program's output is
      # searched whole for its sizing line: the question is every build it
      # was handed, at the numbers it said.
      SIZED = "${sized}/bin/ronoctua-sized-build"
      f = json.load(open("${plantedFacts}"))
      d = f["drv"]
      name = {v: k for k, v in d.items()}
      target = f["target"]
      tmp = pathlib.Path(os.environ["TMPDIR"])
      wrong = []

      def setup(where_name, plan, fails=(), dry_fails=False, times=None):
          where = tmp / where_name
          where.mkdir()
          (where / "targets").write_text(target + "\n")
          (where / "plan").write_text(plan)
          (where / "fails").write_text("".join(x + "\n" for x in fails))
          (where / "timed").write_text("")
          (where / "times").write_text("".join(f"{x} {t}\n" for x, t in (times or {}).items()))
          if dry_fails:
              (where / "dry-fails").write_text("")
          env = dict(os.environ, PATH="${timedNix}/bin:${fakeStore}/bin:" + os.environ["PATH"], FAKE_LOG=str(where / "log"),
                     FAKE_TARGETS=str(where / "targets"), FAKE_PLAN=str(where / "plan"),
                     FAKE_DRY_FAILS=str(where / "dry-fails"), FAKE_TIMED=str(where / "timed"),
                     FAKE_FAILS=str(where / "fails"), FAKE_TIMES=str(where / "times"),
                     FAKE_REFERRERS="${plantedReferrers}")
          return where, env

      def run(where_name, plan, fails=(), dry_fails=False, *flags):
          where, env = setup(where_name, plan, fails, dry_fails)
          ran = subprocess.run([SIZED, "--no-link", *flags, ".#anything"], env=env, capture_output=True, text=True)
          calls = (where / "log").read_text().splitlines() if (where / "log").exists() else []
          asked = [c.split() for c in calls if c.startswith("build ") and "--dry-run" in c.split()]
          return ran, asked, timed(where / "timed")

      def plan_of(*drvs):
          return f"these {len(drvs)} derivations will be built:\n" + "".join(f"  {x}\n" for x in drvs)

      def judge(what, ran, runs, expected):
          m = re.search(r"what is no unit: --max-jobs (\d+) --cores (\d+)", ran.stdout)
          if not m:
              wrong.append(f"{what}: the program did not say its sizing: {ran.stdout[-500:]!r}")
              return
          j, c = m.groups()
          hc = {mm.group(1): mm.group(2) for mm in re.finditer(r"-> (\S+) \(heavy, alone with (\d+) cores", ran.stdout)}
          built = {name.get(x, "target" if x == target else x) for x in runs}
          if built != set(expected):
              wrong.append(f"{what}: built {sorted(built)}, not {sorted(expected)} ({ran.stdout[-400:]!r} {ran.stderr[-400:]!r})")
          flat, _ = together(runs)
          for x, rs in runs.items():
              s, e, cores, jobs = rs[0]
              if x == target:
                  if (cores, jobs) != (c, j) or s < max((r[0][1] for y, r in runs.items() if y != target), default=0):
                      wrong.append(f"{what}: what it was given was not built last, sized as what is no unit: {rs[0]}")
                  continue
              i = name[x]
              heavy = f["classes"][i] == "heavy"
              if (cores, jobs) != ((hc.get(i, "?") if heavy else "1"), "1"):
                  wrong.append(f"{what}: {i} built with --max-jobs {jobs} --cores {cores}")
              for dep in f["deps"][i]:
                  if d[dep] in runs and runs[d[dep]][0][1] > s:
                      wrong.append(f"{what}: {i} started before {dep} ended")
              if heavy and overlapping(flat, x):
                  wrong.append(f"{what}: the heavy {i} ran beside {[name.get(o, o) for o in overlapping(flat, x)]}")

      units = [i for i in d if not i.startswith("aggregate")]
      for what, plan, expected in [
          ("whole", plan_of(*[d[i] for i in units], target), units + ["target"]),
          ("fresh", f"this derivation will be built:\n  {target}\n", ["target"]),
          ("first-held", plan_of(*[d[i] for i in ("release/libs/c", "release/bins/h2", "release/libs/e")], target),
           ["release/libs/c", "release/bins/h2", "release/libs/e", "target"]),
      ]:
          ran, asked, runs = run(what, plan)
          if ran.returncode != 0:
              wrong.append(f"{what}: exit {ran.returncode}: {ran.stderr[-800:]!r}")
          judge(what, ran, runs, expected)
          if [w[-1] for w in asked if target + "^*" in w] != [target + "^*"]:
              wrong.append(f"{what}: Nix was not asked once what the build builds: {asked}")
      # A UNIT FAILS: without --keep-going nothing after it, and not what it
      # was given; with it, every unit that does not need it — and not what
      # it was given, which needs it, said.
      whole = plan_of(*[d[i] for i in units], target)
      ran, _, runs = run("fails", whole, [d["release/libs/m"]])
      if ran.returncode == 0 or target in runs:
          wrong.append(f"fails: exit {ran.returncode}, what it was given built: {target in runs}")
      ran, _, runs = run("fails-keep-going", whole, [d["release/libs/m"]], False, "--keep-going")
      free = {"release/deps/a", "release/libs/b", "release/libs/d", "release/libs/e", "test/testRuns/x", "release/libs/m"}
      if ran.returncode == 0 or {name.get(x, x) for x in runs} != free:
          wrong.append(f"fails-keep-going: exit {ran.returncode}, built {sorted(name.get(x, x) for x in runs)}, not {sorted(free)}")
      if f"not built: {target} needs a unit that was not built above" not in ran.stdout:
          wrong.append(f"fails-keep-going: it did not say what it was given is not built: {ran.stdout[-500:]!r}")
      floor(len(d), 10, "planted units")
      for what, plan, dry_fails in [("unread", "these 3 derivations will be built:\n  /nix/store/x.drv\n", False),
                                    ("no-answer", "", True)]:
          ran, _, runs = run(what, plan, (), dry_fails)
          if ran.returncode == 0 or runs:
              wrong.append(f"{what}: exit {ran.returncode} after {len(runs)} build(s); a plan it cannot read builds nothing")

      # THE TEST RUNS AFTER EVERY COMPILATION, as in the batteries: the
      # planted test run is built from a unit that ends at once, beside a
      # compilation that takes a second and a half; it starts after that
      # one ended.
      where, env = setup("late", whole, times={d["release/libs/e"]: 1.5})
      ran = subprocess.run([SIZED, "--no-link", ".#anything"], env=env, capture_output=True, text=True)
      runs = timed(where / "timed")
      x = runs.get(d["test/testRuns/x"], [(0, 0, 0, 0)])[0]
      compiled = [rs[0][1] for y, rs in runs.items() if name.get(y, "").startswith("release/")]
      if ran.returncode != 0 or not compiled or x[0] < max(compiled) or len(runs.get(d["release/libs/e"], [])) != 1:
          wrong.append(f"late: exit {ran.returncode}; the test run started at {x[0]:.2f} s, the compilations ended at "
                       f"{max(compiled, default=0):.2f} s: a test that measures time ran beside a compiler")

      # A TERM DURING ITS DRY RUN: it ends at once, and the dry run with it
      # (a trap waits for a command in the foreground to end).
      where, env = setup("signalled-dry", whole)
      env["FAKE_DRY_SLEEP"] = "30"
      proc = subprocess.Popen([SIZED, "--no-link", ".#anything"], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
      deadline = time.time() + 20
      while time.time() < deadline and not any(l.startswith("dry ") for l in (where / "timed").read_text().splitlines()):
          time.sleep(0.1)
      sent = time.time()
      os.kill(proc.pid, signal.SIGTERM)
      try:
          proc.communicate(timeout=20)
      except subprocess.TimeoutExpired:
          proc.kill()
          proc.communicate()
      took = time.time() - sent
      marks = [l.split() for l in (where / "timed").read_text().splitlines()]
      drys = [int(w[1]) for w in marks if w[0] == "dry"]
      killed = [int(w[1]) for w in marks if w[0] == "killed-dry"]
      alive = [pid for pid in drys if pathlib.Path(f"/proc/{pid}").exists()]
      if proc.returncode != 143 or took > 10 or not drys or sorted(killed) != sorted(drys) or alive:
          wrong.append(f"signalled-dry: a TERM during the dry run answered {proc.returncode} after {took:.1f} s; "
                       f"dry runs {drys}, killed {killed}, alive {alive}: not at once, with the dry run stopped")

      # A TERM TO IT ALONE: its builds stopped, and waited for, before its
      # clean-up.
      slow = {d[i]: 30 for i in units}
      where, env = setup("signalled", whole, times=slow)
      proc = subprocess.Popen([SIZED, "--no-link", ".#anything"], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
      deadline = time.time() + 20
      while time.time() < deadline and len(started(where / "timed")) < 2:
          time.sleep(0.1)
      first = started(where / "timed")
      os.kill(proc.pid, signal.SIGTERM)
      try:
          out, err = proc.communicate(timeout=20)
      except subprocess.TimeoutExpired:
          proc.kill()
          out, err = proc.communicate()
          wrong.append("signalled: it did not end within 20 s of a TERM")
      seen = started(where / "timed")
      killed = [l.split() for l in (where / "timed").read_text().splitlines() if l.startswith("killed ")]
      log = (where / "log").read_text()
      held = {pathlib.Path(w[w.index("--out-link") + 1]).parent.parent for w in (c.split() for c in log.splitlines()) if "--out-link" in w and w[0] == "build"}
      alive = [pid for _, pid in seen if pathlib.Path(f"/proc/{pid}").exists()]
      if proc.returncode != 143:
          wrong.append(f"signalled: it answered {proc.returncode}, not 143: {err[-400:]!r}")
      if len(first) < 2 or sorted(w[1] for w in killed) != sorted(x for x, _ in seen) or any(w[2] != "held" for w in killed):
          wrong.append(f"signalled: of {len(seen)} builds started, killed {[w[1:] for w in killed]}: each must be killed while its folder is held")
      if alive:
          wrong.append(f"signalled: {len(alive)} build(s) outlived it: {alive}")
      if not held or any(h.exists() for h in held):
          wrong.append(f"signalled: the folder that held the out-links ({held}) was not cleaned up after")
      if "stopping the" not in err:
          wrong.append(f"signalled: it did not say it stopped its builds: {err[-300:]!r}")
      if wrong:
          print("ERROR: .#sized-build does not build what Nix would, each unit by its class:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print("OK: the whole graph unit by unit, the test run after the compilations, the fresh store only what was given, a held heavy unit not again; failures, an unread plan and a TERM, during the dry run and during the builds, handled")
      END
    '';

  # THE BATTERIES REACH EVERY UNIT, and the short loop is sized too.
  #
  # The gate as the flake builds it: every unit of the index that compiles
  # or runs is a line of its plan — the test runs its last lines, from the
  # number it starts them at — with the class it is built by and how that
  # class was found (`classesOf`), its cost, after every line it is built
  # from, built from exactly what it is built from; the aggregates apart.
  # And `.#quick` builds through `.#sized-build`, never a bare `nix build`.
  batteries-reach-every-unit =
    let
      planned = lib.filterAttrs (_: u: lib.elem u.kind sizing.plannedKinds) rust.unitIndex;
      facts = pkgs.writeText "battery-units.json" (
        builtins.toJSON {
          units = lib.mapAttrs (_: u: { inherit (u) kind drv deps; }) rust.unitIndex;
          found = sizing.classesOf { units = lib.attrNames planned; };
          kinds = sizing.plannedKinds;
          # With its context: the script is read in the sandbox.
          quick = self.apps.${system}.quick.program;
        }
      );
    in
    mkCheck "batteries-reach-every-unit" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, pathlib, re, shlex, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the gate's plan, as the shell reads it (`plan_units=<path>`,
      # `runs_from=<n>`, one line each) and the file it names, word by word;
      # its aggregates array; the units as JSON.
      # FILE-WIDE: the quick loop's script is asked whole whether any line of
      # it runs `nix build`, its comments off.
      facts = json.load(open("${facts}"))
      N = json.loads('${numbersJson}')
      SLOT = {c["name"]: c["slotMiB"] for c in N["classes"]}
      def cost_of(cls):
          return f"alone:{N['shellMiB']}+{N['besideMiB']}" if cls == "heavy" else str(SLOT[cls])
      script = pathlib.Path("${gateApp}/bin/ronoctua-gate").read_text()
      def value(name):
          m = re.search(r"^\s*" + name + r"=(\S+)$", script, re.M)
          return must(m and m.group(1), "the gate's " + name)
      plan = [l.split() for l in pathlib.Path(value("plan_units")).read_text().splitlines()]
      runs_from = int(value("runs_from"))
      m = re.search(r"^\s*aggregates=\((.*)\)$", script, re.M)
      aggregates = [w[:-2] for w in shlex.split(must(m and m.group(1), "the gate's aggregates"))]
      units = facts["units"]
      floor(len(units), 1000, "units of the index")
      floor(runs_from, 1000, "compilations in the gate's plan")
      wrong = []
      planned = {i for i, u in units.items() if u["kind"] in facts["kinds"]}
      in_plan = [w[0] for w in plan]
      if set(in_plan) != planned or len(in_plan) != len(set(in_plan)):
          wrong.append(f"{len(planned - set(in_plan))} unit(s) in no line, {len(in_plan) - len(set(in_plan))} in two")
      if {w[0] for w in plan[runs_from:]} != {i for i in planned if units[i]["kind"] == "testRuns"}:
          wrong.append(f"the lines from {runs_from} on are not the test runs, and only those")
      if set(aggregates) != {u["drv"] for u in units.values() if u["kind"] == "aggregate"}:
          wrong.append("the aggregates are not built apart")
      at = {w[0]: n for n, w in enumerate(plan)}
      for n, w in enumerate(plan):
          u = units[w[0]]
          fo = facts["found"][w[0]]
          if w[1:5] != [fo["class"], fo["how"], cost_of(fo["class"]), u["drv"]]:
              wrong.append(f"the plan says {w[:5]}, not the unit, the class it is built by, how, its cost and its .drv")
              break
          if sorted(int(x) for x in w[5:]) != sorted(at[d] for d in u["deps"] if d in at):
              wrong.append(f"the plan's {w[0]} is built from other lines than what it is built from")
              break
          if any(int(x) >= n for x in w[5:]):
              wrong.append(f"the plan puts {w[0]} before a unit it is built from")
              break
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
      heavy = sum(1 for w in plan if w[3].startswith("alone:"))
      print(f"OK: {runs_from} compilations ({heavy} alone) and {len(plan) - runs_from} test runs, last, planned by class, {len(aggregates)} aggregates apart, and .#quick sized")
      END
    '';
}
