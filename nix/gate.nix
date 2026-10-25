# What `nix flake check` asks, sized to the RAM of whatever machine is running it.
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
# at all.
#
# # Nor is `nix flake check` one evaluator any more
#
# The gate used to END in `nix flake check`, which evaluates the whole flake in
# one process by design, and the flake outgrew the machine: on 14 GiB it was
# killed by `earlyoom` twice (2026-10-21) at 9.4 GiB, a quarter of the way
# through the 720 checks — every one of them built and passed already. So the
# last phase asks what `nix flake check` asks, in an evaluator per group of
# outputs (measured that day: the apps 2.2 GiB, the packages 1.3, each system
# 0.9 to 1.3, the rest 0.5): the lock needs no change; every app is an app
# whose program is in the store; every package, shell and the formatter is a
# derivation; every module and template has its shape; every system's
# `toplevel` evaluates. The checks are the targets, which phases 0 to 4 asked
# and built. An output it does not know how to ask about is a failure that
# names it, never one it leaves out (invariant 7, as the owner amended it).
#
# # The order
#
#   0. the `.drv` of every target, evaluated in batches;
#   1. the Rust build, unit by unit (`nix/rust/units.nix`), each unit its
#      own build by its class (`sized_units`, `nix/build-sizing.nix`): as
#      many at once as their slots fit, the heavy ones alone, of what Nix's
#      dry run says it will build:
#      a. every compilation and every test run, ONE scheduler over one plan
#         — the test runs last, started only once every compilation has
#         ended, so a test that measures time does not measure a compiler
#         beside it (the reason is time, not memory: a run is planned at
#         its own slot like any unit);
#      b. the aggregates over them, sized as everything else is;
#   2. what the VMs BOOT — each test's driver, which is its systems — sized as
#      compilations, because that is what they are;
#   3. the VMs, as many at once as their declared memory and their cores fit;
#   4. everything else, sized as compilations;
#   5. the gate only: what `nix flake check` asks of the rest of the flake,
#      an evaluator per group.
#
# # A unit that failed is built once
#
# The test runs were a scheduler of their own, after the compilations, and
# knew nothing of what the first one had not built: each run was its own
# `nix build`, so a hub dependency that failed was built again by each of
# its forty runs, one after another, and after a failure that was the
# machine's (a compiler killed for memory) the chain below a run was
# compiled inside the run's light slot. One scheduler over both never
# builds a failed unit again, nor anything built from it, and says which
# and why; and every later phase leaves out, by name, what needs a unit
# phase 1 did not build (`sizing_needing`, the store's own references),
# which its own `nix build` would build again, sized for nothing.
#
# # Why `most` and `gate` are one function
#
# They size memory identically, so they are the same function with different
# arguments. Written twice, the middle one would kill `rustc` through
# `earlyoom` exactly as the gate did before this file existed — the same
# defect reappearing in a new command, with its fix sitting right next to it.
{
  writeShellApplication,
  writeText,
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
  # `gate` ends in what `nix flake check` asks of the rest of the flake; `most`
  # stops at its targets.
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
  # THE SIZING (`nix/build-sizing.nix`): its shell functions and the plan
  # of the units (`unitPlan`, each unit with its measured class).
  sizing,
  # EVERY UNIT OF THE RUST BUILD, as data (`rust.unitIndex`: `kind`, `drv`
  # and `deps` by id), or `null`: phase 1 builds them all, the test runs
  # last.
  units ? null,
  # The class of each unit, by id, or `null`: the measured ones
  # (`nix/rust/unit-classes.json`). A check plants its own.
  classes ? null,
  # The downloader that resumes, or `null`. See `nix/prefetch.nix`.
  prefetch ? null,
  # Roots the flake's downloads first, or `null`. See `nix/kept-sources.nix`.
  keepSources ? null,
}:

let
  # NOTHING STARTS OVER A BUILD INSIDE THE TREE (`nix/tree-guard.nix`).
  treeGuard = callPackage ./tree-guard.nix { };
  quoted = lib.concatMapStringsSep " " (n: "\"${n}\"");
  # THE PLAN OF THE UNITS (`unitPlan`): every compilation and every test
  # run, the runs last (`lateFrom`). Each unit is built with every output
  # (`^*`): its `metrics` too, which `.#rust-peaks` and `.#rebuild-cost`
  # read. A derivation with one output gone is built again whole, so the
  # kept units are rooted with both (`keptUnits`, `nix/rust.nix`): rooted by
  # `out` alone, a collection took their `metrics` and this phase compiled
  # every kept unit again.
  index = if units == null then { } else units;
  unitPlan = sizing.unitPlan (
    {
      units = index;
      late = [ "testRuns" ];
    }
    // lib.optionalAttrs (classes != null) { inherit classes; }
  );
  aggregateDrvs = quoted (
    map (e: "${e.drv}^*") (lib.filter (e: e.kind == "aggregate") (lib.attrValues index))
  );

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

  # WHAT PHASE 5 ASKS, the questions `nix flake check` asks of the outputs
  # that are not checks: each a function of an output that answers with the
  # names failing it, `[ ]` when every one holds. Files and not text in the
  # script, because they hold strings and the script hands them to `--apply`
  # inside double quotes.
  asks = {
    apps = writeText "gate-asks-apps.nix" ''
      # An app is `{ type = "app"; program = <a path in the store>; }`, its
      # program carrying the derivation it comes from.
      as:
      let
        store = builtins.storeDir + "/";
        isApp =
          a:
          (a.type or null) == "app"
          && builtins.isString (a.program or null)
          && builtins.hasContext a.program
          && builtins.substring 0 (builtins.stringLength store) a.program == store;
      in
      builtins.filter (n: !(isApp (builtins.getAttr n as))) (builtins.attrNames as)
    '';
    derivations = writeText "gate-asks-derivations.nix" ''
      # Every value a derivation whose `.drv` evaluates.
      ds:
      builtins.filter (
        n:
        let
          d = builtins.getAttr n ds;
        in
        !((d.type or null) == "derivation" && builtins.isString d.drvPath)
      ) (builtins.attrNames ds)
    '';
    derivation = writeText "gate-asks-derivation.nix" ''
      # One derivation whose `.drv` evaluates.
      d: if (d.type or null) == "derivation" && builtins.isString d.drvPath then [ ] else [ "it" ]
    '';
    set = writeText "gate-asks-set.nix" ''
      # A set, and nothing asked of what is in it: `nix flake check` does not
      # go into `legacyPackages`.
      s: if builtins.isAttrs s then [ ] else [ "it" ]
    '';
    modules = writeText "gate-asks-modules.nix" ''
      # Every module a function, a set or a path.
      ms:
      builtins.filter (
        n:
        let
          m = builtins.getAttr n ms;
        in
        !(builtins.isFunction m || builtins.isAttrs m || builtins.isPath m)
      ) (builtins.attrNames ms)
    '';
    templates = writeText "gate-asks-templates.nix" ''
      # Every template a path that exists, and a description.
      ts:
      builtins.filter (
        n:
        let
          t = builtins.getAttr n ts;
        in
        !(t ? path && builtins.pathExists t.path && builtins.isString (t.description or null))
      ) (builtins.attrNames ts)
    '';
    nixos = writeText "gate-asks-nixos.nix" ''
      # A system whose `toplevel` evaluates to a derivation.
      c: if builtins.isString c.config.system.build.toplevel.drvPath then [ ] else [ "its toplevel" ]
    '';
  };

  # THE TAIL, ASSEMBLED OUT HERE AND NOT INSIDE `text`.
  #
  # One doubled-quote fence inside another CLOSES the outer one: it ends at
  # any `''`, and the trap is written down in AGENTS.md. With the tail in a
  # binding, the two strings are siblings instead of nested.
  tail =
    if what == "gate" then
      ''
        echo "== 5. the gate: what nix flake check asks of the rest of the flake, an evaluator per group =="
        if [ "$failures" -gt 0 ]; then
          # The gate is red already; the names are above.
          echo "not run: $failures target(s) failed above: ''${failed[*]}"
          exit 1
        fi
        # ONE QUESTION, in an evaluator of its own, in the background and
        # waited for: the names of an output that fail the function in a
        # file (`asks`), and what failing it means.
        ask() {
          local label=$1 installable=$2 question=$3 says=$4
          nix eval --json "$installable" --apply "$(< "$question")" > "$work/answer.json" &
          if ! wait "$!"; then
            blame_memory
            echo "<- $label: did not evaluate (above)"
            fail "$label"
            return 0
          fi
          if [ "$(jq length < "$work/answer.json")" = 0 ]; then
            echo "<- $label: ok"
          else
            echo "<- $label: $(jq -r 'join(", ")' < "$work/answer.json") $says"
            fail "$label"
          fi
        }
        # THE LOCK, as `nix flake check` reads it: complete, and current.
        nix flake metadata --no-update-lock-file --json > "$work/metadata.json" &
        if ! wait "$!"; then
          echo "<- the lock: it needs a change, or the flake does not resolve (above)"
          fail "the lock"
        else
          echo "<- the lock: ok"
          # EVERY OUTPUT BY NAME, asked of the flake the lock resolved: one
          # this phase does not know how to ask about is a failure that names
          # it, never one left out.
          url=$(jq -r .url < "$work/metadata.json")
          nix eval --impure --json --expr "builtins.attrNames (builtins.getFlake \"$url\").outputs" > "$work/outputs.json" &
          if ! wait "$!"; then
            blame_memory
            echo "<- the outputs: they were not listed (above)"
            fail "the outputs"
          else
            mapfile -t kinds < <(jq -r '.[]' < "$work/outputs.json")
            for kind in "''${kinds[@]}"; do
              case "$kind" in
                checks) echo "<- checks: the targets, asked and built by phases 0 to 4" ;;
                apps) ask apps ".#apps.${system}" ${asks.apps} "not an app whose program is in the store" ;;
                packages | devShells) ask "$kind" ".#$kind.${system}" ${asks.derivations} "not a derivation" ;;
                formatter) ask formatter ".#formatter.${system}" ${asks.derivation} "not a derivation" ;;
                legacyPackages) ask legacyPackages ".#legacyPackages.${system}" ${asks.set} "not a set" ;;
                nixosModules) ask nixosModules ".#nixosModules" ${asks.modules} "not a module" ;;
                templates) ask templates ".#templates" ${asks.templates} "without a path that exists or a description" ;;
                nixosConfigurations)
                  nix eval --json ".#nixosConfigurations" --apply builtins.attrNames > "$work/systems.json" &
                  if ! wait "$!"; then
                    blame_memory
                    echo "<- nixosConfigurations: they were not listed (above)"
                    fail nixosConfigurations
                  else
                    mapfile -t systems < <(jq -r '.[]' < "$work/systems.json")
                    for c in "''${systems[@]}"; do
                      ask "nixosConfigurations.$c" ".#nixosConfigurations.$c" ${asks.nixos} "does not evaluate to a derivation"
                    done
                  fi
                  ;;
                lib) echo "<- lib: no output nix flake check knows, which it only warns about" ;;
                *)
                  echo "<- $kind: an output phase 5 does not know how to ask about — nix/gate.nix has to learn what nix flake check asks of it"
                  fail "$kind"
                  ;;
              esac
            done
          fi
        fi
        if [ "$failures" -gt 0 ]; then
          echo ""
          echo "$failures failed: ''${failed[*]}"
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
    # THE UNITS, each by its class — measured, not discovered
    # (`nix/rust/unit-classes.json`; `.#rust-peaks` says when a unit has
    # outgrown its own): the compilations, the test runs, the aggregates.
    plan_units=${unitPlan.text}
    runs_from=${toString unitPlan.lateFrom}
    aggregates=(${aggregateDrvs})

    names=(${quoted targets})
    alone=(${quoted alone})

    # HOW MANY FIT (`nix/build-sizing.nix`): every number below comes from
    # it, measured once, here, before anything builds.
    # shellcheck source=/dev/null
    . ${sizing.functions}
    sizing_measure || exit 1
    sizing_say
    echo ""
    # A SIGNAL STOPS WHAT RUNS before `$work` goes (`sizing_on_signal`):
    # every evaluation, dry run and build from here on runs in the
    # background and is waited for, since a trap waits for a command in the
    # foreground to end — a TERM during an evaluation of every VM test would
    # wait minutes for it.
    sizing_stop_on_signals
    # What every unit's build is also handed (`sized_units`): nothing here;
    # a unit that fails stops nothing that does not need it; and the test
    # runs, the plan's last lines, start after every compilation.
    # shellcheck disable=SC2034
    sized_flags=() sizing_keep_going=1 sizing_late_from=$runs_from

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

    # WHAT NEEDS A UNIT PHASE 1 DID NOT BUILD (`$work/units/not-built`), of
    # the derivations given, one a line: never handed a build of a later
    # phase, which would build that unit again. A store that does not say
    # leaves out everything given, and is said.
    # (In this shell, its answer to a file: a command substitution is a
    # subshell, which a signal's trap does not reach.)
    blocked=""
    blocked_of() {
      if [ ! -s "$work/units/not-built" ]; then return 0; fi
      printf '%s\n' "$@" > "$work/candidates"
      if ! sizing_needing "$work/units/not-built" "$work/candidates"; then
        echo "the store did not say what needs the units phase 1 did not build: none of it is built" >&2
        printf '%s\n' "$@"
      fi
    }
    is_blocked() {
      [ -n "$blocked" ] && grep -qxF -- "$1" <<< "$blocked"
    }

    nix_list() {
      local s="[" x
      for x in "$@"; do s+=" \"$x\""; done
      printf '%s ]' "$s"
    }
    work=$(mktemp -d)
    # AND THE OLD ISOs LEAVE (`nix/iso-sweep.nix`) once the out-links that
    # held this run's builds are gone: the newest of each name stays.
    trap 'rm -rf "$work"; ${callPackage ./iso-sweep.nix { }}/bin/ronoctua-iso-sweep sweep || true' EXIT

    # ── 0 ──────────────────────────────────────────────────────────────────
    echo "== 0. what there is to build, evaluated before anything builds =="
    # The facts first, all in one evaluator: which targets are VM tests, and
    # what their nodes declare.
    nix eval --json ".#checks.${system}" \
      --apply "cs: (${factsOf}) $(nix_list "''${names[@]}") cs" > "$work/facts.json" &
    if ! wait "$!" || ! jq -r '.[] | [.n, .mib, .cores, .nodes] | @tsv' < "$work/facts.json" > "$work/facts"; then
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
      nix eval --json ".#checks.${system}" \
        --apply "cs: (${drvsOf}) $(nix_list "''${batch[@]}") cs" > "$work/batch.json" &
      if ! wait "$!" || ! jq -r '.[] | [.n, .drv, .driver // "-", .out // "-"] | @tsv' < "$work/batch.json" >> "$work/drvs"; then
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
    mapfile -t every_unit < <(awk '{print $5 "^*"}' "$plan_units")
    echo "== 1. the Rust build: $runs_from compilations, $(( $(wc -l < "$plan_units") - runs_from )) test runs, ''${#aggregates[@]} aggregates =="
    if [ "''${#every_unit[@]}" -gt 0 ]; then
      # WHAT NIX WILL BUILD of them, asked once: a unit the store holds is
      # never handed a build of its own. A dry run that does not say it in
      # words this reads builds no unit: the phase fails, and says it.
      nix build --dry-run "''${every_unit[@]}" > /dev/null 2> "$work/dry" &
      if wait "$!" && sizing_will_build < "$work/dry" > "$work/will"; then
        echo "-- a. the compilations, and then the test runs"
        if ! sized_units "$plan_units" "$work/will" "$work/units"; then
          blame_memory
          fail "phase 1a (the units that failed or were not built are named above)"
        fi
      else
        tail -n 5 "$work/dry"
        echo "the dry run of the units did not say what it builds: no unit is built"
        mkdir -p "$work/units"
        awk '{print $5}' "$plan_units" > "$work/units/not-built"
        fail "phase 1a (the dry run of the units)"
      fi
    fi
    # WHAT THE LATER PHASES MAY NOT BUILD, asked once of the store.
    later=("''${aggregates[@]%^*}")
    for n in "''${names[@]}"; do
      if [ "''${driver[$n]:-}" != "" ] && [ "''${driver[$n]}" != - ]; then later+=("''${driver[$n]}"); fi
      if [ "''${mib[$n]}" -eq 0 ]; then later+=("''${drv[$n]}"); fi
    done
    if [ "''${#later[@]}" -gt 0 ]; then
      blocked_of "''${later[@]}" > "$work/blocked"
      blocked=$(< "$work/blocked")
    fi
    if [ "''${#aggregates[@]}" -gt 0 ]; then
      echo "-- b. the aggregates"
      ready_aggregates=()
      for a in "''${aggregates[@]}"; do
        if is_blocked "''${a%^*}"; then
          echo "<- ''${a%^*}: not built: it needs a unit phase 1 did not build"
          fail "''${a%^*}"
        else
          ready_aggregates+=("$a")
        fi
      done
      if [ "''${#ready_aggregates[@]}" -gt 0 ]; then
        nix build --out-link "$work/aggregates" --keep-going \
          --max-jobs "$jobs" --cores "$cores" "''${ready_aggregates[@]}" &
        if ! wait "$!"; then
          blame_memory
          fail "phase 1b (the aggregates that failed are named above)"
        fi
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
        # What needs a unit phase 1 did not build is not booted: its test
        # fails in phase 3, by name.
        if is_blocked "''${driver[$n]}"; then
          echo "<- $n: what it boots needs a unit phase 1 did not build: not built"
        else
          boots+=("''${driver[$n]}^*")
        fi
      done
      # `--keep-going` and no exit: a system that does not build fails its
      # test in phase 3, by name, and every other test still runs. Held in
      # `$work` for phase 1's reason.
      if [ "''${#boots[@]}" -gt 0 ]; then
        nix build --out-link "$work/boot" --keep-going -L \
          --max-jobs "$jobs" --cores "$cores" "''${boots[@]}" &
        if ! wait "$!"; then
          blame_memory
        fi
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
      # AN OUT-LINK IN `$work`, for phase 1's reason: what passed stays in
      # the store for as long as the battery lasts.
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
      if [ "''${mib[$n]}" -eq 0 ]; then
        if is_blocked "''${drv[$n]}"; then
          echo "<- $n: not built: it needs a unit phase 1 did not build"
          fail "$n"
        else
          rest+=("''${drv[$n]}^*")
        fi
      fi
    done
    echo "== 4. the other ''${#rest[@]} checks =="
    if [ "''${#rest[@]}" -gt 0 ]; then
      # HELD in `$work` too, for the same reason as phase 3's.
      nix build --out-link "$work/rest" --keep-going -L \
        --max-jobs "$jobs" --cores "$cores" "''${rest[@]}" "$@" &
      if ! wait "$!"; then
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
        "what nix flake check asks, sized by the machine's RAM"
      else
        "the checks that do not build an installation, sized by the RAM";
    platforms = [ system ];
  };
}
