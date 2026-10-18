# `nix flake check`, sized to the RAM of whatever machine is running it.
#
# # The defect this replaces
#
# `AGENTS.md` used to carry an instruction: build `rust-tests`, `rust-clippy` and
# the package one at a time BEFORE the gate, because otherwise `rustc` dies with
# `signal: 15` and no diagnostic. That instruction is the kind this project says
# should not exist — a written step that could be automatic — and it failed in
# the only way written steps fail: it was there, it was correct, and the gate was
# still run without it.
#
# # The arithmetic is not here
#
# Why `rustc` dies with no message (`earlyoom`, `systemd-oomd`, the kernel),
# how many compilations fit at once, how many cores a heavy unit gets and
# where the ceiling is are `nix/build-sizing.nix`'s, sourced below: the same
# answer a machine's own apply builds by (`ronoctua-machine apply`) and
# `.#sized-build` gives. What stays here is what only a battery has: the
# phases, the VM tests and the evaluators.
#
# # A VM is not a compiler
#
# The first version sized everything by the compiler's peak, and the VM tests
# broke it. A VM test holds what its nodes declare (`virtualisation.memorySize`,
# 1 to 4 GiB a test) for as long as its script runs, and `--max-jobs 4` put
# four of them side by side: 8 to 16 GiB on a machine with 14. It passed when
# small ones happened to coincide and was killed when the large ones lined up —
# one round finished in 29 minutes and the next died halfway through the VMs.
#
# So the VM tests are a phase of their own, sized by what they declare. The
# declared number is the bound, not the typical: measured (2026-09-27, QEMU's
# `VmRSS` at its peak), `footer-vm` declares 2048 MiB and touched 582, the
# 3072 MiB `camera-vm` touched 965, and `player-remote-vm`, two nodes and
# 4096 MiB, touched 1064. It is still the number to plan by: it is the one the
# guest is allowed to reach, and a measured one ages with every test that
# changes. On top of it, `vm_node_mib` for QEMU itself and `vm_test_mib` for the
# driver and the builder around it (measured: 100 to 115 MiB).
#
# # And evaluating is not free either
#
# A VM test is a NixOS system, and evaluating one holds ~260 MiB until the
# evaluator exits. `nix build` of the whole battery evaluated all of them in one
# process — 8.2 GiB of `VmRSS`, measured — and that process stayed alive for
# the WHOLE build, beside every compiler and every VM, counted by nothing.
#
# So nothing is evaluated while something builds. Phase 0 asks for the `.drv`
# of every target in batches whose evaluators fit the budget, one after
# another, and every later phase builds those paths, which needs no evaluator
# at all. The one exception is the gate's own last step, `nix flake check`,
# which evaluates everything at once by design — by then there is nothing left
# for it to build.
#
# # The order
#
#   0. the `.drv` of every target, evaluated in batches;
#   1. the Rust build, unit by unit (`nix/rust/units.nix`):
#      a. everything the heavy units are built from, as Rust units
#         (`--max-jobs <slots> --cores 1`);
#      b. the heavy units, one at a time, each with the cores the budget
#         leaves beside its own compiler (`rust.heavyUnits`);
#      c. every other unit, as Rust units again;
#      d. the test runs and the aggregates, sized as everything else is —
#         after every compilation, so a test that measures time does not
#         measure a compiler beside it;
#   2. what the VMs BOOT — each test's driver, which is its systems — sized as
#      compilations, because that is what they are;
#   3. the VMs, as many at once as their declared memory and their cores fit;
#   4. everything else, sized as compilations;
#   5. the gate only: `nix flake check`, over a store that has it all.
#
# # Why `most` and `gate` are one function
#
# They size memory identically, so they are the same function with different
# arguments. Written twice, the middle one would kill `rustc` through
# `earlyoom` exactly as the gate did before this file existed — the same
# defect reappearing in a new command, with its fix sitting right next to it.
{
  writeShellApplication,
  callPackage,
  coreutils,
  gawk,
  gnugrep,
  gnused,
  jq,
  lib,
  system,
  # WHAT THE BATTERY IS CALLED, and what it does once the sizing is done.
  #
  # `gate` ends in the whole `nix flake check`; `most` stops at its targets.
  # Everything before that point is shared.
  name ? "ronoctua-gate",
  what ? "gate",
  # The checks this battery builds, by name.
  targets ? [ ],
  # Targets that each evaluate whole systems of their own (the milestones, the
  # ISO): phase 0 gives each one an evaluator to itself.
  alone ? [ ],
  # What is left out, so the battery can say what it did not verify.
  left ? [ ],
  # THE SIZING (`nix/build-sizing.nix`): its shell functions and its plan of
  # the heavy units.
  sizing,
  # The units whose own compiler no division describes, by name
  # (`rust.heavyUnits`). They go one at a time, each after what it is built
  # from (`sizing.heavyPlan`).
  heavy ? { },
  # EVERY UNIT OF THE RUST BUILD, as data (`rust.unitIndex`: `kind` and
  # `drv` by id), or `null`: phase 1 builds them all, the test runs last.
  units ? null,
  # The downloader that resumes, or `null`. See `nix/prefetch.nix`.
  prefetch ? null,
  # Roots the flake's downloads first, or `null`. See `nix/kept-sources.nix`.
  keepSources ? null,
}:

let
  # NOTHING STARTS OVER A BUILD INSIDE THE TREE (`nix/tree-guard.nix`).
  treeGuard = callPackage ./tree-guard.nix { };
  quoted = lib.concatMapStringsSep " " (n: "\"${n}\"");
  plain = builtins.unsafeDiscardStringContext;
  # EVERY OUTPUT of a unit (`^*`): its `metrics` too, which `.#rust-peaks`
  # and `.#rebuild-cost` read. A derivation with one output gone is built
  # again whole, so the kept units are rooted with both (`keptUnits`,
  # `nix/rust.nix`): rooted by `out` alone, a collection took their
  # `metrics` and this phase compiled every kept unit again.
  drvOf = d: "${plain d.drvPath}^*";
  # THE HEAVY UNITS IN THE ORDER THEY ARE BUILT, and what they are built
  # from — read off the units' own `passthru.unitDeps`, at evaluation.
  plan = sizing.heavyPlan heavy;
  heavyDrvs = quoted (map (s: drvOf s.unit) plan.steps);
  heavyNames = quoted (map (s: s.name) plan.steps);
  heavyBetween = quoted (map (s: lib.concatMapStringsSep " " drvOf s.between) plan.steps);
  firstDrvs = quoted (map drvOf plan.first);
  # EVERY OTHER UNIT, and the test runs (with the aggregates over them) apart.
  heavyPaths = map (s: plain s.unit.drvPath) plan.steps;
  entries = if units == null then [ ] else lib.attrValues units;
  isRun = e: e.kind == "testRuns" || e.kind == "aggregate";
  restDrvs = quoted (
    map (e: "${e.drv}^*") (lib.filter (e: !isRun e && !(lib.elem e.drv heavyPaths)) entries)
  );
  runDrvs = quoted (map (e: "${e.drv}^*") (lib.filter isRun entries));

  # WHAT PHASE 0 ASKS THE FLAKE, as functions of the names. The text goes
  # through the shell inside double quotes, so it has none of its own and no
  # `${`: `getAttr` and not `cs.${n}`, `null` and not a string.
  #
  # The facts are cheap — a node's `memorySize` is one option, not a system —
  # and the `.drv` paths are what costs.
  factsOf = ''
    names: cs:
    map (
      n:
      let
        t = builtins.getAttr n cs;
        ns = if t ? nodes then builtins.attrValues t.nodes else [ ];
        sum = f: builtins.foldl' (s: x: s + f x) 0 ns;
      in
      {
        inherit n;
        mib = sum (x: x.virtualisation.memorySize);
        cores = sum (x: x.virtualisation.cores);
        nodes = builtins.length ns;
      }
    ) names
  '';
  drvsOf = ''
    names: cs:
    map (
      n:
      let
        t = builtins.getAttr n cs;
        vm = t ? nodes;
      in
      {
        inherit n;
        drv = t.drvPath;
        driver = if vm then t.driver.drvPath else null;
        out = if vm then t.driver.outPath else null;
      }
    ) names
  '';

  leftLines = lib.concatMapStringsSep "\n    " (t: "echo \"       ${t}\"") left;

  # THE TAIL, ASSEMBLED OUT HERE AND NOT INSIDE `text`.
  #
  # One doubled-quote fence inside another CLOSES the outer one: it ends at
  # any `''`, and the trap is written down in AGENTS.md. With the tail in a
  # binding, the two strings are siblings instead of nested.
  tail =
    if what == "gate" then
      ''
        echo "== 5. the gate =="
        if [ "$failures" -gt 0 ]; then
          # `nix flake check` would build what failed again, sized for
          # nothing — the VMs among them four at a time. The gate is red
          # already; the names are above.
          echo "not run: $failures target(s) failed above: ''${failed[*]}"
          exit 1
        fi
        if ! nix flake check --keep-going -L --max-jobs "$jobs" --cores "$cores" "$@"; then
          blame_memory
          exit 1
        fi
        echo ""
        echo "GATE OK"
      ''
    else
      ''
        if [ "$failures" -gt 0 ]; then
          echo "$failures target(s) failed: ''${failed[*]}"
          exit 1
        fi
        echo "OK — and this does NOT close invariant 7."
        echo ""
        echo "     ${toString (builtins.length left)} were left out: the ones that build an"
        echo "     INSTALLATION — a NixOS system, a disk image or the ISO —, and the"
        echo "     milestones (mN), which are a question for publishing time:"
        echo ""
        ${leftLines}
        echo ""
        echo "     What runs them is  nix run .#gate  (invariant 7)."
      '';
in
writeShellApplication {
  inherit name;
  # The numbers come from the sizing's functions, which shellcheck cannot
  # follow into (`sizing_measure` sets them).
  excludeShellChecks = [ "SC2154" ];
  runtimeInputs = [
    coreutils
    gawk
    gnugrep
    gnused
    jq
  ];
  text = ''
    ${treeGuard}/bin/ronoctua-tree-holds-no-build
    ${lib.optionalString (keepSources != null) keepSources}
    ${lib.optionalString (prefetch != null) ''
      # THE SOURCES NO CACHE HAS COME FIRST.
      #
      # One source in the `full` profile is 1.37 GB, unfree, and behind a single
      # URL that resets the connection partway through — and `fetchurl` retries
      # WITHOUT resuming, so it never finishes. Left to the check, the gate dies
      # after minutes of work with `cannot download ... from any mirror`, which
      # reads like the file is gone. Measured and written down in
      # `nix/prefetch.nix`.
      #
      # It is a no-op once the store has them, which is every run after the
      # first.
      ${prefetch}
      echo ""
    ''}
    # THE HEAVY ONES, and they go one at a time.
    #
    # Named by the flake rather than discovered (`rust.heavyUnits`), because
    # "heavy" is not a property Nix exposes: it is the units whose own
    # compiler was MEASURED past the slot (`metrics/peak-kib`; `.#rust-peaks`
    # says when the list is wrong). The `loop` workspace was missing for
    # months: three checks reach it through `.#dev`, and it compiled in the
    # last phase, four at a time, beside whatever else was building.
    heavy=(${heavyDrvs})
    heavy_name=(${heavyNames})
    heavy_between=(${heavyBetween})
    first=(${firstDrvs})
    units_rest=(${restDrvs})
    runs=(${runDrvs})

    names=(${quoted targets})
    alone=(${quoted alone})

    # HOW MANY FIT (`nix/build-sizing.nix`): every number below comes from
    # it, measured once, here, before anything builds.
    # shellcheck source=/dev/null
    . ${sizing.functions}
    sizing_measure || exit 1
    sizing_say
    echo ""
    # What every heavy build is also handed (`sized_heavy`): nothing here.
    # shellcheck disable=SC2034
    sized_flags=()

    started=$(date '+%Y-%m-%d %H:%M:%S')
    failures=0
    failed=()
    fail() {
      failures=$(( failures + 1 ))
      failed+=("$1")
    }

    # WHAT TELLS THE TRUTH ABOUT A `signal: 15` — or a `signal: 9` — and the
    # reason this function exists at all: without it the message names the
    # compiler, and the compiler is not what went wrong. Three killers act on
    # memory, and each says it in its own place, so each is asked:
    #
    # * `earlyoom` on a host that has it (SIGTERM, by name, in its own log);
    # * `systemd-oomd`, which on this distro watches `system.slice` — where the
    #   daemon's builds run — and kills a whole cgroup when its pressure stays
    #   high (`nix/modules/memory.nix`); it logs the cgroup it killed;
    # * the KERNEL's OOM killer, the last one, in the kernel's log
    #   (`Out of memory: Killed process <pid> (<name>)`).
    #
    # It names every process or cgroup they took, because a VM's QEMU and an
    # evaluator die the same silent way. A journal it may not read (no
    # `systemd-journal` group) is no answer, not an error.
    blame_memory() {
      local killed
      killed=$(
        {
          journalctl -u earlyoom --since "$started" --no-pager -o cat 2>/dev/null \
            | awk -F'"' '/^sending SIG/ {print "earlyoom: " $2}' || true
          journalctl -u systemd-oomd --since "$started" --no-pager -o cat 2>/dev/null \
            | awk '/^Killed / {print "systemd-oomd: " $2}' || true
          journalctl -k --since "$started" --no-pager -o cat 2>/dev/null \
            | sed -n 's/^.*[Oo]ut of memory: Killed process [0-9]* (\([^)]*\)).*$/kernel oom-kill: \1/p' || true
        } | sort | uniq -c
      )
      if [ -n "$killed" ]; then
        echo ""
        echo "Killed for memory during this round (earlyoom, systemd-oomd, the kernel):"
        echo "$killed"
        echo "It is NOT a defect of what was being built: it is the HOST's memory."
        echo "Close what you can and run again — this command fits itself to"
        echo "whatever is free at the moment it starts. On a machine that only"
        echo "builds, ronoctua.memory.buildCeiling raises the daemon's ceiling"
        echo "(docs/DEVELOPMENT.md)."
      fi
    }

    nix_list() {
      local s="[" x
      for x in "$@"; do s+=" \"$x\""; done
      printf '%s ]' "$s"
    }
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT

    # ── 0 ──────────────────────────────────────────────────────────────────
    echo "== 0. what there is to build, evaluated before anything builds =="
    # The facts first, all in one evaluator: which targets are VM tests, and
    # what their nodes declare.
    if ! nix eval --json ".#checks.${system}" \
         --apply "cs: (${factsOf}) $(nix_list "''${names[@]}") cs" \
         | jq -r '.[] | [.n, .mib, .cores, .nodes] | @tsv' > "$work/facts"; then
      blame_memory
      exit 1
    fi
    declare -A mib=() vcores=() nodes=() drv=() driver=() driver_out=()
    while IFS=$'\t' read -r n m c k; do
      mib[$n]=$m
      vcores[$n]=$c
      nodes[$n]=$k
    done < "$work/facts"

    # THE BATCHES. A VM test costs its evaluator ~260 MiB, measured; with a
    # base of a gigabyte for the flake and nixpkgs, as many go in one batch as
    # the budget holds. Everything else is a few MiB and goes in one batch,
    # except the targets that evaluate whole systems of their own, which get
    # one each.
    vm_eval_kb=$(( 320 * 1024 ))
    base_eval_kb=$(( 1024 * 1024 ))
    per_batch=$(( (budget_kb - base_eval_kb) / vm_eval_kb ))
    [ "$per_batch" -ge 1 ] || per_batch=1
    vms=()
    while read -r _ n; do
      vms+=("$n")
    done < <(for n in "''${names[@]}"; do
               if [ "''${mib[$n]}" -gt 0 ]; then echo "''${mib[$n]} $n"; fi
             done | sort -k1,1nr -k2,2)
    plain=()
    for n in "''${names[@]}"; do
      if [ "''${mib[$n]}" -gt 0 ]; then continue; fi
      if printf '%s\n' "''${alone[@]}" | grep -qxF -- "$n"; then continue; fi
      plain+=("$n")
    done
    batches=()
    for (( i = 0; i < ''${#vms[@]}; i += per_batch )); do
      batches+=("''${vms[*]:i:per_batch}")
    done
    if [ "''${#plain[@]}" -gt 0 ]; then
      batches+=("''${plain[*]}")
    fi
    for n in "''${alone[@]}"; do
      # Only the ones this battery builds, and not a VM test, which is in a
      # batch of VMs already.
      if [ -n "''${mib[$n]:-}" ] && [ "''${mib[$n]}" -eq 0 ]; then
        batches+=("$n")
      fi
    done
    for b in "''${batches[@]}"; do
      read -r -a batch <<< "$b"
      echo "-- ''${#batch[@]} target(s)"
      if ! nix eval --json ".#checks.${system}" \
           --apply "cs: (${drvsOf}) $(nix_list "''${batch[@]}") cs" \
           | jq -r '.[] | [.n, .drv, .driver // "-", .out // "-"] | @tsv' >> "$work/drvs"; then
        blame_memory
        exit 1
      fi
    done
    while IFS=$'\t' read -r n d r o; do
      drv[$n]=$d
      driver[$n]=$r
      driver_out[$n]=$o
    done < "$work/drvs"
    echo ""

    # ── 1 ──────────────────────────────────────────────────────────────────
    #
    # HELD WHILE THE BATTERY RUNS, and not a minute longer. The store now
    # collects itself when the disk runs short (Nix's floor, see
    # `nix/modules/store.nix`), and a collection during a later phase would
    # take what this one paid for — and the phases after it, whose systems
    # carry the shell and whose last step builds every check, would compile
    # the units again.
    # So each result is an out-link in `$work`, which the `EXIT` trap removes:
    # a root for exactly as long as the battery lasts.
    echo "== 1. the Rust build: ''${#first[@]} units below the ''${#heavy[@]} heavy ones, the heavy ones alone, ''${#units_rest[@]} more units, ''${#runs[@]} test runs =="
    if [ "''${#first[@]}" -gt 0 ]; then
      echo "-- a. what the heavy units are built from"
      if ! nix build --out-link "$work/first" --max-jobs "$rust_jobs" --cores "$rust_cores" "''${first[@]}"; then
        blame_memory
        exit 1
      fi
    fi
    echo "-- b. the heavy units, one at a time"
    for i in "''${!heavy[@]}"; do
      read -r -a between <<< "''${heavy_between[i]}"
      if ! sized_heavy "$work/heavy-$i" "''${heavy_name[i]}" "''${heavy[i]}" "''${between[@]}"; then
        blame_memory
        exit 1
      fi
    done
    if [ "''${#units_rest[@]}" -gt 0 ]; then
      echo "-- c. every other unit"
      # `--keep-going` and no exit: a unit that does not compile fails the
      # checks that need it, by name, and every other check still runs.
      if ! nix build --out-link "$work/units" --keep-going \
           --max-jobs "$rust_jobs" --cores "$rust_cores" "''${units_rest[@]}"; then
        blame_memory
        fail "phase 1c (the units that failed are named above)"
      fi
    fi
    if [ "''${#runs[@]}" -gt 0 ]; then
      echo "-- d. the test runs"
      if ! nix build --out-link "$work/runs" --keep-going \
           --max-jobs "$jobs" --cores "$cores" "''${runs[@]}"; then
        blame_memory
        fail "phase 1d (the test runs that failed are named above)"
      fi
    fi
    ${lib.optionalString (keepSources != null) ''
      # AND THE UNITS, which now exist: the current ones keep a root of their
      # own past the battery (`nix/kept-sources.nix`).
      ${keepSources}
    ''}
    echo ""

    # ── 2 ──────────────────────────────────────────────────────────────────
    echo "== 2. what the ''${#vms[@]} VM tests boot, without booting them =="
    if [ "''${#vms[@]}" -gt 0 ]; then
      boots=()
      for n in "''${vms[@]}"; do
        boots+=("''${driver[$n]}^*")
      done
      # `--keep-going` and no exit: a system that does not build fails its
      # test in phase 3, by name, and every other test still runs. Held in
      # `$work` for phase 1's reason.
      if ! nix build --out-link "$work/boot" --keep-going -L \
           --max-jobs "$jobs" --cores "$cores" "''${boots[@]}"; then
        blame_memory
      fi
    fi
    echo ""

    # ── 3 ──────────────────────────────────────────────────────────────────
    # THE VMS, PACKED BY WHAT THEY DECLARE.
    #
    # Measured again here, because phases 1 and 2 left the page cache full and
    # it is `MemAvailable`, which counts that cache as free, that says what fits.
    available_kb=$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo)
    vm_budget_mib=$(( $(sizing_under_ceiling $(( available_kb - floor_kb - reserve_kb ))) / 1024 ))
    # QEMU itself, per node, and the driver and the builder around a test.
    vm_node_mib=256
    vm_test_mib=128
    printf '== 3. the VM tests: %s MiB for them, %s cores ==\n' "$vm_budget_mib" "$cpus"

    declare -A running=()
    used_mib=0
    used_cores=0
    # Every lookup goes through a plain variable first: a key like
    # `footer-vm` inside `$(( ))` would be read as a subtraction.
    cost_of() {
      local m=''${mib[$1]} k=''${nodes[$1]}
      echo $(( m + k * vm_node_mib + vm_test_mib ))
    }
    launch() {
      local n=$1 cost c=''${vcores[$1]}
      cost=$(cost_of "$n")
      used_mib=$(( used_mib + cost ))
      used_cores=$(( used_cores + c ))
      printf -- '-> %s (%s MiB declared, %s of %s MiB in use)\n' \
        "$n" "''${mib[$n]}" "$used_mib" "$vm_budget_mib"
      # AN OUT-LINK IN `$work`, for phase 1's reason: a collection between
      # here and phase 5 would take a test that passed, and `nix flake check`
      # would boot its VM again.
      nix build --out-link "$work/vm-$n" -L --max-jobs 1 "''${drv[$n]}^*" &
      running[$!]=$n
    }
    reap_one() {
      local done_pid=0 rc=0 n cost c
      if wait -n -p done_pid "''${!running[@]}"; then rc=0; else rc=$?; fi
      n=''${running[$done_pid]}
      unset "running[$done_pid]"
      cost=$(cost_of "$n")
      c=''${vcores[$n]}
      used_mib=$(( used_mib - cost ))
      used_cores=$(( used_cores - c ))
      if [ "$rc" -eq 0 ]; then
        echo "<- $n: ok"
      else
        echo "<- $n: FAILED"
        fail "$n"
      fi
    }

    pending=()
    for n in "''${vms[@]}"; do
      # A test whose systems did not build in phase 2 is not started: its
      # `nix build` would compile them here, beside the VMs.
      if [ -e "''${driver_out[$n]}" ]; then
        pending+=("$n")
      else
        echo "<- $n: FAILED (what it boots did not build, above)"
        fail "$n"
      fi
    done
    while [ "''${#pending[@]}" -gt 0 ] || [ "''${#running[@]}" -gt 0 ]; do
      waiting=()
      for n in "''${pending[@]}"; do
        cost=$(cost_of "$n")
        c=''${vcores[$n]}
        # Nothing running means it goes, whatever it costs: a test larger than
        # the whole budget runs alone rather than never.
        if [ "''${#running[@]}" -eq 0 ] \
          || { [ $(( used_mib + cost )) -le "$vm_budget_mib" ] \
               && [ $(( used_cores + c )) -le "$cpus" ]; }; then
          launch "$n"
        else
          waiting+=("$n")
        fi
      done
      pending=("''${waiting[@]}")
      if [ "''${#running[@]}" -gt 0 ]; then
        reap_one
      fi
    done
    if [ "$failures" -gt 0 ]; then
      blame_memory
    fi
    echo ""

    # ── 4 ──────────────────────────────────────────────────────────────────
    rest=()
    for n in "''${names[@]}"; do
      if [ "''${mib[$n]}" -eq 0 ]; then rest+=("''${drv[$n]}^*"); fi
    done
    echo "== 4. the other ''${#rest[@]} checks =="
    if [ "''${#rest[@]}" -gt 0 ]; then
      # HELD in `$work` too, for the same reason as phase 3's.
      if ! nix build --out-link "$work/rest" --keep-going -L \
           --max-jobs "$jobs" --cores "$cores" "''${rest[@]}" "$@"; then
        blame_memory
        fail "phase 4 (the failed builds are named above)"
      fi
    fi
    echo ""

    ${tail}
  '';

  meta = {
    description =
      if what == "gate" then
        "nix flake check, sized by the machine's RAM"
      else
        "the checks that do not build an installation, sized by the RAM";
    platforms = [ system ];
  };
}
