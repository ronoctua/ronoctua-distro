# HOW MANY COMPILATIONS FIT IN THIS MACHINE AT ONCE — one arithmetic, read by
# `.#gate`, `.#most`, `.#quick`, `.#sized-build` and `ronoctua-machine apply`.
#
# # The defect this replaces
#
# The arithmetic lived in `nix/gate.nix`, and only the batteries had it. A
# machine applying its own flake (`ronoctua-machine apply`: the update, the
# rescue's rebuild, the software centre's apply) ran `nix build` with
# `max-jobs = auto` and `cores = 0` — every core, times every core. With the
# Rust build in one derivation per compilation unit (`nix/rust/units.nix`,
# some 300 units in a system's build) that is a dozen `rustc`, each with a
# dozen threads, on a laptop: the shape that has `earlyoom` or the kernel kill
# a compiler with no message. So the arithmetic moved here, and every place
# that builds the unit graph asks it.
#
# # Why `rustc` dies with no message
#
# It is not the build and it is not memory pressure inside the sandbox. It is
# `earlyoom` on the HOST, which sends SIGTERM when available memory falls below
# its threshold and has `rustc` in its `--prefer` list — so `rustc` is chosen
# first, by configuration — or `systemd-oomd`, or the kernel's OOM killer. Nix
# reports the signal, the compiler never gets to print anything, and the
# failure looks like a compiler bug.
#
# `--max-jobs` alone does not save it: that caps DERIVATIONS in parallel, and a
# derivation of ours hands its compiler `$NIX_BUILD_CORES` jobserver tokens.
# What has to be capped is the product.
#
# # The arithmetic, from measurements and not from taste
#
# The floor is `earlyoom`'s trigger — 10% of total memory in the configuration
# measured here, which is also its default — plus a reserve, because the
# browser and the editor go on allocating while the build runs. What is left
# of what is AVAILABLE, never past the ceiling of the cgroup the builds run in,
# is the budget; the budget divided by the peak of one compiler (`peakMiB`) is
# how many compilations fit at once — the slots.
#
# The slots are spent three ways:
#
#   the Rust units      `--max-jobs <slots> --cores 1`: a unit is one `rustc`,
#                       mostly one thread (one codegen unit or a few), and
#                       the many small ones are what a build of the unit graph
#                       is — so as many at once as fit;
#   the heavy units     one at a time, `--max-jobs 1 --cores <heavy>`: their
#                       own compiler peaks past a slot (`rust.heavyUnits`);
#   everything else     `--max-jobs <jobs> --cores <cores>`, jobs at most four
#                       and `jobs × cores ≤ slots`: a derivation of someone
#                       else's (a NixOS system, a VM test, a C library) is
#                       usually one build using every core it is given, and
#                       past four the wall-clock gain is small and the memory
#                       cost is not.
#
# # The heavy ones are paid once, not per core
#
# A heavy unit has one compiler no division describes: the shell's program
# under `release` is one thin-LTO link of the whole footer, and its test
# harness and clippy are the same crate compiled whole. It does not split into
# slots, so it is paid once, and every OTHER core pays what was measured beside
# it: `shellMiB` at one core, `besideMiB` for each core more. The cores a heavy
# unit gets are what the budget leaves room for, and nothing else builds beside
# it.
#
# # Where the builds run decides the ceiling
#
# A person's `nix build` hands its builds to `nix-daemon`, whose cgroup has the
# ceiling: on this distro `ronoctua.memory.buildCeiling` (`MemoryHigh`, half the
# RAM by default), past which the kernel throttles every compiler and
# `systemd-oomd` kills the build. ROOT's `nix` opens the store itself and
# builds in its own process — `ronoctua-machine apply`, under the update's
# unit — so its ceiling is that unit's cgroup and its parents'. Sized by the
# daemon's when the builds run elsewhere, the budget would be someone else's.
{
  lib,
  writeText,
  writeShellApplication,
  coreutils,
  gawk,
  gnugrep,
  gnused,
}:

let
  # THE NUMBERS, each with where it was measured.
  numbers = {
    # ONE COMPILER'S PEAK, the slot. `earlyoom`'s own log on this project:
    # 1577, 958 and 857 MiB of `VmRSS` for the three `rustc` it killed
    # (2026-09); the worst of those with margin. The units measured since
    # (`metrics/peak-kib`, 2026-10-12/15) stay under it but for the heavy
    # ones: the heaviest light unit is cranelift-codegen's release library,
    # 1562 MiB. `.#rust-peaks` says when that stops being true.
    peakMiB = 1792;
    # THE HEAVIEST HEAVY UNIT AT ONE CORE, and what each core more adds beside
    # it — `metrics/peak-kib` (the build's largest child's `ru_maxrss`, so a
    # link's linker counts) of each heavy unit built alone at 1, 3 and 6
    # cores (2026-10-15, this tree, a 12-processor machine):
    #
    #   the shell's test harness       4350, 4717, 5175 MiB   165 MiB a core
    #   the shell's release program    3847, 4359, 4985 MiB   228 MiB a core
    #                                  (thin LTO, sixteen codegen units)
    #
    # the worst at one core and the steepest slope, with margin. Crane's
    # workspace, one codegen unit, measured 5120 MiB for the shell at four
    # cores and 846 MiB a core beside it (2026-09-27): the 5632 and 1024 this
    # replaces. With 10.6 GiB free on that machine they gave a heavy unit
    # three cores; these give it eleven, and the measured slope puts it at
    # about 6 GiB there, inside the 8 GiB budget.
    shellMiB = 4864;
    besideMiB = 320;
    # Kept free on top of `earlyoom`'s floor.
    reserveMiB = 1024;
    # `earlyoom -m 10,5`: SIGTERM at 10% of total.
    floorPercent = 10;
    # Past this the batteries' general builds gain little (see above).
    jobsCeiling = 4;
  };

  # THE ARITHMETIC, as shell functions: sourced by the batteries and
  # `.#sized-build`, and run by `ronoctua-build-sizing`, so there is one copy.
  #
  # `sizing_measure [meminfo] [cpus] [ceiling-bytes|infinity]` sets every
  # variable below; with no arguments it reads this machine. It refuses —
  # returns 1, with a sentence — a meminfo it cannot read and a CPU count
  # that is not one, never a plausible default.
  functions = writeText "build-sizing.sh" ''
    # GENERATED from nix/build-sizing.nix. Sourced, never run.

    sizing_refuse() {
      echo "build sizing: $*" >&2
      return 1
    }

    # `sizing_cgroup_ceiling <cgroup fs> <cgroup>`: the smallest `memory.high`
    # or `memory.max` from that cgroup up to the root, in bytes, or nothing
    # (`max` everywhere) — the effective limit of what runs in it.
    sizing_cgroup_ceiling() {
      local fs=$1 own=$2 path limit best="" level
      path=$fs''${own%/}
      while :; do
        for level in memory.high memory.max; do
          limit=$(cat "$path/$level" 2>/dev/null || true)
          if [[ "$limit" =~ ^[0-9]+$ ]] && { [ -z "$best" ] || [ "$limit" -lt "$best" ]; }; then
            best=$limit
          fi
        done
        [ "$path" != "$fs" ] || break
        path=''${path%/*}
        [ "''${#path}" -ge "''${#fs}" ] || break
      done
      echo "$best"
    }

    # THE CEILING OF WHERE THE BUILDS RUN: sets `ceiling_b` (bytes, or
    # nothing) and `sizing_ceiling_from` (what it is).
    sizing_read_ceiling() {
      local own
      if [ "$(id -u)" = 0 ] && [ -z "''${NIX_REMOTE:-}" ]; then
        # Root's own store: its builds are children of this process, in
        # this cgroup and under its parents' limits.
        sizing_ceiling_from="this process's cgroup"
        own=$(sed -n 's/^0::\(\/.*\)$/\1/p' /proc/self/cgroup 2>/dev/null || true)
        ceiling_b=""
        if [ -n "$own" ]; then
          ceiling_b=$(sizing_cgroup_ceiling /sys/fs/cgroup "$own")
        fi
      else
        # Anybody else hands the builds to the daemon. `infinity`, an empty
        # answer or no systemd (another distro, a container) is no ceiling.
        sizing_ceiling_from="nix-daemon's MemoryHigh"
        ceiling_b=$(systemctl show -p MemoryHigh --value nix-daemon.service 2>/dev/null || true)
        [[ "$ceiling_b" =~ ^[0-9]+$ ]] || ceiling_b=""
      fi
    }

    sizing_measure() {
      local meminfo=''${1:-/proc/meminfo} given_cpus=''${2:-} given_ceiling=''${3:-}
      total_kb=$(awk '/^MemTotal:/ {print $2}' "$meminfo" 2>/dev/null || true)
      available_kb=$(awk '/^MemAvailable:/ {print $2}' "$meminfo" 2>/dev/null || true)
      if ! [[ "$total_kb" =~ ^[0-9]+$ && "$available_kb" =~ ^[0-9]+$ ]]; then
        sizing_refuse "$meminfo says no MemTotal and MemAvailable in kB: nothing to size by"
        return 1
      fi
      if [ -n "$given_cpus" ]; then
        cpus=$given_cpus
      else
        cpus=$(nproc)
      fi
      if ! [[ "$cpus" =~ ^[1-9][0-9]*$ ]]; then
        sizing_refuse "$cpus is not a count of processors"
        return 1
      fi
      floor_kb=$(( total_kb * ${toString numbers.floorPercent} / 100 ))
      reserve_kb=$(( ${toString numbers.reserveMiB} * 1024 ))
      peak_kb=$(( ${toString numbers.peakMiB} * 1024 ))
      shell_kb=$(( ${toString numbers.shellMiB} * 1024 ))
      beside_kb=$(( ${toString numbers.besideMiB} * 1024 ))

      # NEVER PAST THE CEILING, read once: every budget below follows it.
      sizing_ceiling_from=""
      case "$given_ceiling" in
        "") sizing_read_ceiling ;;
        infinity) ceiling_b=""; sizing_ceiling_from="given" ;;
        *)
          if ! [[ "$given_ceiling" =~ ^[0-9]+$ ]]; then
            sizing_refuse "$given_ceiling is not a ceiling in bytes, nor infinity"
            return 1
          fi
          ceiling_b=$given_ceiling
          sizing_ceiling_from="given"
          ;;
      esac
      ceiling_kb=""
      if [ -n "$ceiling_b" ]; then
        ceiling_kb=$(( ceiling_b / 1024 - reserve_kb ))
        if [ "$ceiling_kb" -lt 0 ]; then
          ceiling_kb=0
        fi
      fi

      budget_kb=$(sizing_under_ceiling $(( available_kb - floor_kb - reserve_kb )))
      if [ "$budget_kb" -lt "$peak_kb" ]; then
        slots=1
      else
        slots=$(( budget_kb / peak_kb ))
      fi
      [ "$slots" -le "$cpus" ] || slots=$cpus

      # THE RUST UNITS: as many at once as fit, one core each.
      rust_jobs=$slots
      rust_cores=1
      # EVERYTHING ELSE: at most four at once, the slots split among them.
      jobs=$slots
      [ "$jobs" -le ${toString numbers.jobsCeiling} ] || jobs=${toString numbers.jobsCeiling}
      cores=$(( slots / jobs ))
      [ "$cores" -ge 1 ] || cores=1
      # THE HEAVY ONES: the compiler once, and what each other core adds.
      if [ "$budget_kb" -le "$shell_kb" ]; then
        heavy_cores=1
      else
        heavy_cores=$(( 1 + (budget_kb - shell_kb) / beside_kb ))
      fi
      [ "$heavy_cores" -le "$cpus" ] || heavy_cores=$cpus
      return 0
    }

    # `sizing_under_ceiling <kB>`: the smaller of what is free and the ceiling
    # (`sizing_measure` read it). A negative budget is no budget.
    sizing_under_ceiling() {
      local kb=$1
      if [ -n "''${ceiling_kb:-}" ] && [ "$ceiling_kb" -lt "$kb" ]; then
        kb=$ceiling_kb
      fi
      if [ "$kb" -lt 0 ]; then
        kb=0
      fi
      echo "$kb"
    }

    # WHAT WAS DECIDED, said. MiB and not GiB: the division truncates, and a
    # floor of 1466 MiB shown as "1 GiB" is a number that argues with the
    # arithmetic beside it.
    sizing_say() {
      printf 'memory: %s MiB in total, %s MiB available; %s processors\n' \
        "$(( total_kb / 1024 ))" "$(( available_kb / 1024 ))" "$cpus"
      if [ -n "$ceiling_kb" ]; then
        printf 'ceiling (%s): %s MiB, and every budget follows it\n' \
          "$sizing_ceiling_from" "$(( (ceiling_kb + reserve_kb) / 1024 ))"
      fi
      printf 'earlyoom floor: %s MiB;  reserve: %s MiB;  peak per rustc: %s MiB\n' \
        "$(( floor_kb / 1024 ))" "$(( reserve_kb / 1024 ))" "$(( peak_kb / 1024 ))"
      printf '%s compilations fit at once -> the Rust units: --max-jobs %s --cores %s;  heavy: --max-jobs 1 --cores %s;  the rest: --max-jobs %s --cores %s\n' \
        "$slots" "$rust_jobs" "$rust_cores" "$heavy_cores" "$jobs" "$cores"
    }

    # `sizing_will_build`: what Nix's dry run (its standard error, on standard
    # input) says will be built — the `.drv`s under "will be built:", one a
    # line. That is Nix's own plan: a unit whose consumer the store already
    # holds is not in it, though it is in the `.drv` closure. Returns 1 for
    # words it does not read (a count that is not the paths under it, a path
    # there that is not a `.drv`, a store path under no heading it knows),
    # never a shorter list than Nix said. The same rules as
    # `ronoctua_machine::dry::to_build`.
    sizing_will_build() {
      awk '
        function close_heading() {
          if (built && listed != said) bad = 1
          built = 0
        }
        /^[[:space:]]+\/nix\/store\// {
          if (built) {
            if (NF != 1 || $1 !~ /\.drv$/) bad = 1
            listed++
            print $1
          } else if (!other) {
            bad = 1
          }
          next
        }
        {
          close_heading()
          other = 0
          line = $0
          sub(/[[:space:]]+$/, "", line)
          if (line ~ /will be built:$/) {
            built = 1
            listed = 0
            if ($1 == "this") said = 1
            else if ($1 == "these" && $2 ~ /^[0-9]+$/) said = $2 + 0
            else said = -1
          } else if (line ~ /will be fetched/ || line ~ /know how to build/) {
            other = 1
          }
        }
        END { close_heading(); exit bad }
      '
    }

    # `sized_heavy <link> <name> <heavy .drv> [<below .drv>…]`, with
    # `sized_flags` (an array) handed to every `nix build`: what the heavy
    # unit is built from, as Rust units, then the heavy unit alone. Each
    # result is an out-link at `<link>-below…` / `<link>`, held as long as
    # the caller keeps it. The status of the first build that failed.
    sized_heavy() {
      local link=$1 name=$2 heavy=$3
      shift 3
      if [ "$#" -gt 0 ]; then
        echo "-- $name: first the $# derivations it is built from"
        nix build --out-link "$link-below" "''${sized_flags[@]}" \
          --max-jobs "$rust_jobs" --cores "$rust_cores" "$@" || return 1
      fi
      echo "-- $name, alone"
      nix build --out-link "$link" "''${sized_flags[@]}" \
        --max-jobs 1 --cores "$heavy_cores" "$heavy" || return 1
    }
  '';

  # THE PROGRAM: the same arithmetic, printed as `name=value` lines for a
  # program to read — `ronoctua-machine apply` sizes its builds with it — and,
  # with `--explain`, said on standard error. `--meminfo`, `--cpus` and
  # `--ceiling` measure another machine (a check plants one); without them it
  # measures this one.
  program = writeShellApplication {
    name = "ronoctua-build-sizing";
    runtimeInputs = [
      coreutils
      gawk
      gnugrep
      gnused
    ];
    text = ''
      # shellcheck source=/dev/null
      . ${functions}
      meminfo="" given_cpus="" given_ceiling="" explain=0
      needs() {
        if [ "$1" -lt 2 ]; then
          echo "ronoctua-build-sizing: $2 needs a value" >&2
          exit 2
        fi
      }
      while [ $# -gt 0 ]; do
        case "$1" in
          --meminfo) needs $# "$1"; meminfo=$2; shift ;;
          --cpus) needs $# "$1"; given_cpus=$2; shift ;;
          --ceiling) needs $# "$1"; given_ceiling=$2; shift ;;
          --explain) explain=1 ;;
          -h | --help)
            echo "usage: ronoctua-build-sizing [--explain] [--meminfo FILE] [--cpus N] [--ceiling BYTES|infinity]"
            echo "prints how many compilations fit at once here, as name=value lines"
            exit 0
            ;;
          *) echo "ronoctua-build-sizing: unknown argument $1" >&2; exit 2 ;;
        esac
        shift
      done
      sizing_measure "$meminfo" "$given_cpus" "$given_ceiling" || exit 1
      if [ "$explain" = 1 ]; then
        sizing_say >&2
      fi
      for name in total_kb available_kb cpus floor_kb reserve_kb peak_kb shell_kb beside_kb \
                  ceiling_kb budget_kb slots rust_jobs rust_cores jobs cores heavy_cores; do
        printf '%s=%s\n' "$name" "''${!name}"
      done
    '';
  };

  # THE HEAVY UNITS IN THE ORDER THEY ARE BUILT, and what each is built from
  # first — computed at evaluation from the units' `passthru.unitDeps`
  # (`nix/rust/units.nix`), never from a guess.
  #
  # `heavy` is `{ <name> = <unit>; }`. The answer is
  #
  #   { first = [ <unit> … ];                     every unit below a heavy one
  #                                               that reaches none: built
  #                                               first, as Rust units
  #     steps = [ { name; unit; between; } … ];   the heavy ones, a heavy one
  #   }                                           below another first; each
  #                                               after `between`, its units
  #                                               that reach an earlier heavy
  #                                               one (built by then)
  #
  # A heavy unit inside another's closure goes first, so no step ever builds
  # a heavy unit as a dependency at the wrong size; two that reach each other
  # are refused.
  heavyPlan =
    heavy:
    let
      plain = builtins.unsafeDiscardStringContext;
      keyOf = d: plain d.drvPath;
      depsOf = d: d.passthru.unitDeps or [ ];
      heavyKeys = lib.listToAttrs (lib.mapAttrsToList (name: d: lib.nameValuePair (keyOf d) name) heavy);
      entry = d: {
        key = keyOf d;
        unit = d;
      };
      closureOf =
        d:
        builtins.genericClosure {
          startSet = map entry (depsOf d);
          operator = e: map entry (depsOf e.unit);
        };
      tops = lib.mapAttrsToList (
        name: d:
        let
          closure = closureOf d;
        in
        {
          inherit name closure;
          key = keyOf d;
          unit = d;
          below = lib.filter (e: heavyKeys ? ${e.key}) closure;
        }
      ) heavy;
      # Every unit below any heavy one, and whether it reaches a heavy one
      # (memoised: an attribute set refers to itself lazily).
      every = lib.listToAttrs (
        map (e: lib.nameValuePair e.key e.unit) (lib.concatMap (t: t.closure) tops)
      );
      reaches = lib.mapAttrs (
        _: d: lib.any (x: heavyKeys ? ${keyOf x} || reaches.${keyOf x}) (depsOf d)
      ) every;
      sorted = lib.toposort (a: b: lib.any (e: e.key == a.key) b.below) tops;
      order =
        sorted.result or (throw "nix/build-sizing.nix: the heavy units ${
          lib.concatMapStringsSep ", " (t: t.name) (sorted.cycle or [ ])
        } are built from each other");
      light = e: !(heavyKeys ? ${e.key});
    in
    {
      first = lib.attrValues (
        lib.listToAttrs (
          map (e: lib.nameValuePair e.key e.unit) (
            lib.filter (e: light e && !reaches.${e.key}) (lib.concatMap (t: t.closure) order)
          )
        )
      );
      steps = map (t: {
        inherit (t) name unit;
        between = map (e: e.unit) (lib.filter (e: light e && reaches.${e.key}) t.closure);
      }) order;
    };

  # `.#sized-build`: `nix build`, sized. What it is given is evaluated once
  # (`nix path-info --derivation`); of the heavy units and what each is built
  # from, only what Nix's own plan for that build lists (a dry run,
  # `sizing_will_build`) is built — the heavy ones first and alone, each after
  # what it is built from; then everything, as Rust units. Never the `.drv`
  # closure: it names every unit below a program the store already holds,
  # and building those would compile what the build itself never needs.
  # What the plan was made from is `heavy` (`rust.heavyUnits` of the flake
  # this program comes from).
  sizedBuild =
    heavy:
    let
      plan = heavyPlan heavy;
      plain = builtins.unsafeDiscardStringContext;
      drv = d: plain d.drvPath;
      quoted = lib.concatMapStringsSep " " (s: "\"${s}\"");
    in
    writeShellApplication {
      name = "ronoctua-sized-build";
      # `sizing_measure` sets the numbers, where shellcheck cannot follow.
      excludeShellChecks = [ "SC2154" ];
      runtimeInputs = [
        coreutils
        gawk
        gnugrep
        gnused
      ];
      text = ''
        # shellcheck source=/dev/null
        . ${functions}
        usage() {
          echo "usage: nix run .#sized-build -- [-L] [--keep-going] [--option NAME VALUE] [--no-link | --out-link PATH] INSTALLABLE…"
          echo "builds the installables sized to this machine: the heavy units they reach one at a"
          echo "time, after what each is built from, and everything else as many at once as fit"
        }
        # The options it knows: the out-link's, which only the last build
        # takes, and Nix's settings and logging, which every call takes. Any
        # other is refused, never guessed at (it might take a value).
        sized_flags=() link=() wanted=()
        while [ $# -gt 0 ]; do
          case "$1" in
            -L | --print-build-logs | --keep-going | --fallback | --show-trace | --impure | \
              --offline | --refresh | -v | --verbose) sized_flags+=("$1") ;;
            --option)
              [ $# -ge 3 ] || { echo "ronoctua-sized-build: --option needs a name and a value" >&2; exit 2; }
              sized_flags+=("$1" "$2" "$3")
              shift 2
              ;;
            --no-link) link=(--no-link) ;;
            --out-link)
              [ $# -ge 2 ] || { echo "ronoctua-sized-build: --out-link needs a path" >&2; exit 2; }
              link=(--out-link "$2")
              shift
              ;;
            -h | --help) usage; exit 0 ;;
            --) shift; wanted+=("$@"); break ;;
            -*) echo "ronoctua-sized-build: $1 is not an option this takes (see --help)" >&2; exit 2 ;;
            *) wanted+=("$1") ;;
          esac
          shift
        done
        [ "''${#wanted[@]}" -gt 0 ] || { usage >&2; exit 2; }

        sizing_measure || exit 1
        sizing_say
        echo ""

        # WHAT IT IS GIVEN, evaluated once: every later build names `.drv`s,
        # which needs no evaluator.
        evaluated=$(nix path-info --derivation "''${sized_flags[@]}" "''${wanted[@]}") || exit 1
        mapfile -t drvs <<< "$evaluated"
        [ -n "$evaluated" ] || { echo "ronoctua-sized-build: nothing to build" >&2; exit 1; }
        targets=()
        for d in "''${drvs[@]}"; do targets+=("$d^*"); done
        # WHAT THEIR BUILD WILL BUILD, in Nix's own plan — asked, and never
        # assumed: an answer lost or unread here would build the heavy units
        # beside everything else.
        if ! planned=$(nix build --dry-run "''${sized_flags[@]}" "''${targets[@]}" 2>&1 >/dev/null); then
          printf '%s\n' "$planned" >&2
          echo "ronoctua-sized-build: the dry run did not say what the build builds" >&2
          exit 1
        fi
        if ! will_build=$(printf '%s\n' "$planned" | sizing_will_build); then
          printf '%s\n' "$planned" >&2
          echo "ronoctua-sized-build: the dry run said what it builds in words this does not read" >&2
          exit 1
        fi
        declare -A will=()
        while IFS= read -r p; do
          if [ -n "$p" ]; then will[$p]=1; fi
        done <<< "$will_build"

        # A FAILED PHASE stops it, as a failed `nix build` stops; with
        # `--keep-going` the rest is still built, and the answer is still no.
        keep_going=0
        for f in "''${sized_flags[@]}"; do
          if [ "$f" = --keep-going ]; then keep_going=1; fi
        done
        failed=0
        phase_failed() {
          if [ "$keep_going" = 0 ]; then exit 1; fi
          failed=1
        }

        held=$(mktemp -d)
        trap 'rm -rf "$held"' EXIT

        first=(${quoted (map drv plan.first)})
        heavy=(${quoted (map (s: drv s.unit) plan.steps)})
        heavy_name=(${quoted (map (s: s.name) plan.steps)})
        between=(${quoted (map (s: lib.concatMapStringsSep " " drv s.between) plan.steps)})

        # The units below the heavy ones Nix would build, as Rust units.
        below=()
        for d in "''${first[@]}"; do
          if [ -n "''${will[$d]:-}" ]; then below+=("$d^*"); fi
        done
        if [ "''${#below[@]}" -gt 0 ]; then
          echo "== what the heavy units are built from: ''${#below[@]} units =="
          nix build --out-link "$held/first" "''${sized_flags[@]}" \
            --max-jobs "$rust_jobs" --cores "$rust_cores" "''${below[@]}" || phase_failed
        fi
        for i in "''${!heavy[@]}"; do
          [ -n "''${will[''${heavy[i]}]:-}" ] || continue
          read -r -a more <<< "''${between[i]}"
          others=()
          for d in "''${more[@]}"; do
            if [ -n "''${will[$d]:-}" ]; then others+=("$d^*"); fi
          done
          sized_heavy "$held/heavy-$i" "''${heavy_name[i]}" "''${heavy[i]}^*" "''${others[@]}" || phase_failed
        done

        echo "== what it was given =="
        nix build "''${link[@]}" "''${sized_flags[@]}" \
          --max-jobs "$rust_jobs" --cores "$rust_cores" "''${targets[@]}" || exit 1
        exit "$failed"
      '';
    };
in
{
  inherit
    numbers
    functions
    program
    heavyPlan
    sizedBuild
    ;
}
