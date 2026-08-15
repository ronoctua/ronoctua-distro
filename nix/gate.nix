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
{
  writeShellApplication,
  coreutils,
  gawk,
  gnugrep,
  system,
}:

writeShellApplication {
  name = "ronoctua-gate";
  runtimeInputs = [
    coreutils
    gawk
    gnugrep
  ];
  text = ''
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
    printf 'memoria: %s MiB no total, %s MiB disponivel\n' \
      "$(( total_kb / 1024 ))" "$(( available_kb / 1024 ))"
    printf 'piso do earlyoom: %s MiB;  reserva: %s MiB;  pico por rustc: %s MiB\n' \
      "$(( floor_kb / 1024 ))" "$(( reserve_kb / 1024 ))" "$(( peak_kb / 1024 ))"
    printf 'cabem %s compilacoes ao mesmo tempo -> pesadas: --cores %s;  gate: --max-jobs %s --cores %s\n\n' \
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
        echo "O earlyoom matou o rustc $killed vez(es) durante esta rodada."
        echo "NAO e defeito de compilacao: e a memoria do HOST, e o rustc esta"
        echo "no --prefer dele. Feche o que puder e rode de novo — este comando"
        echo "ja se ajusta ao que estiver livre no momento em que comeca."
      fi
    }

    echo "== as tres pesadas, uma de cada vez =="
    for alvo in "''${heavy[@]}"; do
      echo "-- $alvo"
      if ! nix build --no-link --max-jobs 1 --cores "$slots" "$alvo"; then
        blame_earlyoom
        exit 1
      fi
    done

    echo ""
    echo "== o gate =="
    if ! nix flake check --keep-going -L --max-jobs "$jobs" --cores "$cores" "$@"; then
      blame_earlyoom
      exit 1
    fi
    echo ""
    echo "GATE OK"
  '';
  meta = {
    description = "nix flake check dimensionado pela RAM da maquina";
    platforms = [ system ];
  };
}
