# `nix flake check`, sized to the RAM of whatever machine is running it.
#
# # The defect this replaces
#
# `CLAUDE.md` used to carry an instruction: build `rust-tests`, `rust-clippy` and
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
# 857 MiB of `VmRSS` for the three `rustc` processes it killed. `PEAK_MIB` is the
# worst of those with margin.
#
# The floor is `earlyoom`'s trigger — 10% of total memory in the configuration
# seen here, which is also its default — plus a reserve, because the browser and
# the editor go on allocating while the gate runs.
#
# What is left over, divided by the peak, is how many compilers may run at once.
#
# # Two batteries, one piece of arithmetic
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
  lib,
  system,
  # WHAT THE BATTERY IS CALLED, and what it does once the sizing is done.
  #
  # `gate` runs the whole `nix flake check`; `most` builds a list of targets.
  # Everything before that point is shared.
  name ? "ronoctua-gate",
  what ? "gate",
  # The targets, when `what` is `targets`.
  targets ? [ ],
  # What is left out, so the battery can say what it did not verify.
  left ? [ ],
  # The downloader that resumes, or `null`. See `nix/prefetch.nix`.
  prefetch ? null,
}:

let
  # THE TAIL, ASSEMBLED OUT HERE AND NOT INSIDE `text`.
  #
  # One doubled-quote fence inside another CLOSES the outer one: it ends at
  # any `''`, and the trap is written down in CLAUDE.md. With the tail in a
  # binding, the two strings are siblings instead of nested.
  targetLines = lib.concatMapStringsSep "\n      " (t: "\".#checks.${system}.${t}\"") targets;
  leftLines = lib.concatMapStringsSep "\n    " (t: "echo \"       ${t}\"") left;

  tail =
    if what == "gate" then
      ''
        echo "== o gate =="
        if ! nix flake check --keep-going -L --max-jobs "$jobs" --cores "$cores" "$@"; then
          blame_earlyoom
          exit 1
        fi
        echo ""
        echo "GATE OK"
      ''
    else
      ''
        echo "== ${toString (builtins.length targets)} checks, without building an installation =="
        targets=(
          ${targetLines}
        )
        if ! nix build --no-link --keep-going -L \
             --max-jobs "$jobs" --cores "$cores" "''${targets[@]}" "$@"; then
          blame_earlyoom
          exit 1
        fi
        echo ""
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
  ];
  text = ''
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
    # THE THREE HEAVY ONES, and they go one at a time.
    #
    # Named here rather than discovered, because "heavy" is not a property Nix
    # exposes: it is the three that compile the whole workspace. If a fourth
    # appears, this list is where it goes.
    heavy=(
      ".#checks.${system}.rust-tests"
      ".#checks.${system}.rust-clippy"
      ".#ronoctua-shell"
    )

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

    # For the gate itself the slots are split between derivations and the cores
    # inside each. Four is the ceiling because past that the wall-clock gain is
    # small and the memory cost is not.
    jobs=$slots
    [ "$jobs" -le 4 ] || jobs=4
    cores=$(( slots / jobs ))
    [ "$cores" -ge 1 ] || cores=1

    # MiB and not GiB: the division truncates, and a floor of 1466 MiB shown as
    # "1 GiB" is a number that argues with the arithmetic beside it.
    printf 'memory: %s MiB in total, %s MiB available\n' \
      "$(( total_kb / 1024 ))" "$(( available_kb / 1024 ))"
    printf 'earlyoom floor: %s MiB;  reserve: %s MiB;  peak per rustc: %s MiB\n' \
      "$(( floor_kb / 1024 ))" "$(( reserve_kb / 1024 ))" "$(( peak_kb / 1024 ))"
    printf '%s compilations fit at once -> heavy: --cores %s;  gate: --max-jobs %s --cores %s\n\n' \
      "$slots" "$slots" "$jobs" "$cores"

    started=$(date '+%Y-%m-%d %H:%M:%S')

    # WHAT TELLS THE TRUTH ABOUT A `signal: 15`, and the reason this function
    # exists at all: without it the message names the compiler, and the compiler
    # is not what went wrong.
    blame_earlyoom() {
      local killed
      killed=$(journalctl -u earlyoom --since "$started" --no-pager 2>/dev/null \
        | grep -c 'sending SIG.* "rustc"' || true)
      if [ "''${killed:-0}" -gt 0 ]; then
        echo ""
        echo "earlyoom killed rustc $killed time(s) during this round."
        echo "It is NOT a compilation defect: it is the HOST's memory, and rustc is"
        echo "in its --prefer. Close what you can and run again — this command"
        echo "already fits itself to whatever is free at the moment it starts."
      fi
    }

    echo "== the three heavy ones, one at a time =="
    for target in "''${heavy[@]}"; do
      echo "-- $target"
      if ! nix build --no-link --max-jobs 1 --cores "$slots" "$target"; then
        blame_earlyoom
        exit 1
      fi
    done

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
