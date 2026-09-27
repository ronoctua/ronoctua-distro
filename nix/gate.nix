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
# # Why `rustc` dies with no message
#
# It is not the build and it is not memory pressure inside the sandbox. It is
# `earlyoom` on the HOST, which sends SIGTERM when available memory falls below
# its threshold and has `rustc` in its `--prefer` list — so `rustc` is chosen
# first, by configuration. Nix reports the signal, the compiler never gets to
# print anything, and the failure looks like a compiler bug.
#
# `--max-jobs` alone does not save it: that caps DERIVATIONS in parallel, and one
# derivation of ours starts one `rustc` per core. What has to be capped is the
# product.
#
# # The arithmetic, from measurements and not from taste
#
# The peak comes from `earlyoom`'s own log on this project: 1577 MiB, 958 MiB and
# 857 MiB of `VmRSS` for the three `rustc` processes it killed. `peak_kb` is the
# worst of those with margin.
#
# The floor is `earlyoom`'s trigger — 10% of total memory in the configuration
# seen here, which is also its default — plus a reserve, because the browser and
# the editor go on allocating while the gate runs.
#
# What is left over, divided by the peak, is how many compilers may run at once.
#
# # The heavy ones are paid once, not per core
#
# The four derivations that compile the whole workspace have one compiler that
# no division describes: `ronoctua-shell`'s own `rustc`. Under `release` it is
# one codegen unit with thin LTO, the whole crate in one LLVM module, and it
# measured 5120 MiB on its own (2026-09-27, `--cores 4`, `VmRSS` sampled every
# second in the daemon's cgroup). It does not split across cores, so it is paid
# once, and every OTHER core pays what was measured beside it: the whole build
# peaked at 7658 MiB, which is the shell plus 846 MiB a core. `shell_kb` and
# `beside_kb` are those two numbers with margin, and the cores a heavy build
# gets are what they leave room for. (Under `loop` the same workspace peaked at
# 2460 MiB, its shell at 2293: the `release` numbers cover both.)
#
# # A VM is not a compiler
#
# The first version sized everything by that one number, and the VM tests
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
#   1. the heavy ones, one at a time;
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
  coreutils,
  gawk,
  gnugrep,
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
  # The derivations that compile the whole workspace, by name. They go one at
  # a time, before anything else.
  heavy ? { },
  # The downloader that resumes, or `null`. See `nix/prefetch.nix`.
  prefetch ? null,
  # Roots the flake's downloads first, or `null`. See `nix/kept-sources.nix`.
  keepSources ? null,
}:

let
  quoted = lib.concatMapStringsSep " " (n: "\"${n}\"");
  heavyDrvs = lib.concatMapStringsSep " " (
    d: "\"${builtins.unsafeDiscardStringContext d.drvPath}^*\""
  ) (lib.attrValues heavy);

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
          blame_earlyoom
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
  runtimeInputs = [
    coreutils
    gawk
    gnugrep
    jq
  ];
  text = ''
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
    # Named by the flake rather than discovered, because "heavy" is not a
    # property Nix exposes: it is the ones that compile the whole workspace.
    # The fourth, the `loop` workspace, was missing for months: three checks
    # reach it through `.#dev`, and it compiled in the last phase, four at a
    # time, beside whatever else was building.
    heavy=(${heavyDrvs})
    heavy_name=(${quoted (lib.attrNames heavy)})

    names=(${quoted targets})
    alone=(${quoted alone})

    total_kb=$(awk '/^MemTotal:/ {print $2}' /proc/meminfo)
    available_kb=$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo)
    cpus=$(nproc)

    # `earlyoom -m 10,5` sends SIGTERM at 10% of total. That is both the
    # configuration measured on this host and earlyoom's own default, so it is
    # the right floor even where earlyoom is absent — there the number is simply
    # unused headroom.
    floor_kb=$(( total_kb / 10 ))
    # AND A RESERVE ON TOP, because the floor is where the killing STARTS. The
    # gate runs for hours; a browser tab opened halfway through must not be what
    # decides whether it finishes.
    reserve_kb=$(( 1024 * 1024 ))
    peak_kb=$(( 1792 * 1024 ))

    budget_kb=$(( available_kb - floor_kb - reserve_kb ))
    if [ "$budget_kb" -lt "$peak_kb" ]; then
      slots=1
    else
      slots=$(( budget_kb / peak_kb ))
    fi
    [ "$slots" -le "$cpus" ] || slots=$cpus

    # For the batteries the slots are split between derivations and the cores
    # inside each. Four is the ceiling because past that the wall-clock gain is
    # small and the memory cost is not.
    jobs=$slots
    [ "$jobs" -le 4 ] || jobs=4
    cores=$(( slots / jobs ))
    [ "$cores" -ge 1 ] || cores=1

    # THE HEAVY ONES: the shell's compiler once, and what each other core
    # was measured to add beside it. See the head of this file.
    shell_kb=$(( 5632 * 1024 ))
    beside_kb=$(( 1024 * 1024 ))
    if [ "$budget_kb" -le "$shell_kb" ]; then
      heavy_cores=1
    else
      heavy_cores=$(( 1 + (budget_kb - shell_kb) / beside_kb ))
    fi
    [ "$heavy_cores" -le "$cpus" ] || heavy_cores=$cpus

    # MiB and not GiB: the division truncates, and a floor of 1466 MiB shown as
    # "1 GiB" is a number that argues with the arithmetic beside it.
    printf 'memory: %s MiB in total, %s MiB available\n' \
      "$(( total_kb / 1024 ))" "$(( available_kb / 1024 ))"
    printf 'earlyoom floor: %s MiB;  reserve: %s MiB;  peak per rustc: %s MiB\n' \
      "$(( floor_kb / 1024 ))" "$(( reserve_kb / 1024 ))" "$(( peak_kb / 1024 ))"
    printf '%s compilations fit at once -> heavy: --cores %s;  the rest: --max-jobs %s --cores %s\n\n' \
      "$slots" "$heavy_cores" "$jobs" "$cores"

    started=$(date '+%Y-%m-%d %H:%M:%S')
    failures=0
    failed=()
    fail() {
      failures=$(( failures + 1 ))
      failed+=("$1")
    }

    # WHAT TELLS THE TRUTH ABOUT A `signal: 15`, and the reason this function
    # exists at all: without it the message names the compiler, and the compiler
    # is not what went wrong. It names every process earlyoom took, because a
    # VM's QEMU and an evaluator die the same silent way.
    blame_earlyoom() {
      local killed
      killed=$(journalctl -u earlyoom --since "$started" --no-pager -o cat 2>/dev/null \
        | awk -F'"' '/^sending SIG/ {print $2}' | sort | uniq -c || true)
      if [ -n "$killed" ]; then
        echo ""
        echo "earlyoom killed, during this round:"
        echo "$killed"
        echo "It is NOT a defect of what was being built: it is the HOST's memory."
        echo "Close what you can and run again — this command fits itself to"
        echo "whatever is free at the moment it starts."
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
      blame_earlyoom
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
        blame_earlyoom
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
    echo "== 1. the ''${#heavy[@]} heavy ones, one at a time =="
    for i in "''${!heavy[@]}"; do
      echo "-- ''${heavy_name[i]}"
      if ! nix build --no-link --max-jobs 1 --cores "$heavy_cores" "''${heavy[i]}"; then
        blame_earlyoom
        exit 1
      fi
    done
    echo ""

    # ── 2 ──────────────────────────────────────────────────────────────────
    echo "== 2. what the ''${#vms[@]} VM tests boot, without booting them =="
    if [ "''${#vms[@]}" -gt 0 ]; then
      boots=()
      for n in "''${vms[@]}"; do
        boots+=("''${driver[$n]}^*")
      done
      # `--keep-going` and no exit: a system that does not build fails its
      # test in phase 3, by name, and every other test still runs.
      if ! nix build --no-link --keep-going -L \
           --max-jobs "$jobs" --cores "$cores" "''${boots[@]}"; then
        blame_earlyoom
      fi
    fi
    echo ""

    # ── 3 ──────────────────────────────────────────────────────────────────
    # THE VMS, PACKED BY WHAT THEY DECLARE.
    #
    # Measured again here, because phases 1 and 2 left the page cache full and
    # it is `MemAvailable`, which counts that cache as free, that says what fits.
    available_kb=$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo)
    vm_budget_mib=$(( (available_kb - floor_kb - reserve_kb) / 1024 ))
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
      nix build --no-link -L --max-jobs 1 "''${drv[$n]}^*" &
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
      blame_earlyoom
    fi
    echo ""

    # ── 4 ──────────────────────────────────────────────────────────────────
    rest=()
    for n in "''${names[@]}"; do
      if [ "''${mib[$n]}" -eq 0 ]; then rest+=("''${drv[$n]}^*"); fi
    done
    echo "== 4. the other ''${#rest[@]} checks =="
    if [ "''${#rest[@]}" -gt 0 ]; then
      if ! nix build --no-link --keep-going -L \
           --max-jobs "$jobs" --cores "$cores" "''${rest[@]}" "$@"; then
        blame_earlyoom
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
