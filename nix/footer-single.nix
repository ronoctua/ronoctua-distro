# Starts the footer by killing the previous one — one per session, always.
#
# # Why this is a package, and not a function inside `dev.nix`
#
# Because it was a function inside `dev.nix`, and it **did not work**. Hot
# reload's `rebuild` runs inside the watcher's process, which is another
# process; the function got there through `export -f`, but the `rodapepid` it
# read is a shell variable, and a shell variable does not cross a process
# boundary. Under the watcher's `set -u` that is not an `if` evaluating to
# false: it is `unbound variable`, which **takes the whole shell down**, and the
# loop's `|| true` does not save it. The result, in order: the old footer did
# not die, the new one did not start, and the watcher died — hot reload ceased
# to exist after the first edit, in silence, with a "compiling…" on screen to
# say everything was fine.
#
# Separated out, the mechanism becomes a function from (pidfile, binary) to
# "there is exactly one alive, and it is the new one" — which a check proves in
# seconds, with no compositor and no GPU. It is the same reason, and the same
# mistake, as `reload.nix`.
#
# # Why a file, and not a variable
#
# Because whoever starts and whoever restarts are different processes by
# construction: `dev.nix` starts, the watcher restarts. State shared between two
# processes lives in the filesystem or does not exist.
{
  writeShellApplication,
  procps,
}:

writeShellApplication {
  name = "ronoctua-footer-single";

  # `procps` for `pkill`/`kill -0`: coreutils does not have them.
  runtimeInputs = [ procps ];

  text = ''
    # Usage: ronoctua-footer-single <pidfile> <binary> [args...]
    #
    # The footer's ENVIRONMENT is inherited (WAYLAND_DISPLAY, RUST_LOG, NIRI_SOCKET).
    # Passing it by argument would force the caller to build a complete `env`
    # at both call sites, and the two would diverge.
    if [ $# -lt 2 ]; then
      echo "ERROR: usage: ronoctua-footer-single <pidfile> <binary> [args...]" >&2
      exit 2
    fi
    pidfile="$1"
    binary="$2"
    shift 2

    [ -x "$binary" ] || { echo "ERROR: '$binary' is not executable." >&2; exit 2; }

    # ---------------------------------------------------------------
    # Take the previous one down, and WAIT for it to die
    # ---------------------------------------------------------------
    # Waiting is not fussiness: the footer only lets go of the layer surface and the
    # control socket on the way out. Starting the new one before that gives two
    # footers on screen and the keyboard contention `leader::ceder_o_lugar`
    # describes — the shortcut list opens in the new one and the keys go to the old one.
    if [ -s "$pidfile" ]; then
      previous=$(cat "$pidfile")
      if kill -0 "$previous" 2>/dev/null; then
        kill "$previous" 2>/dev/null || true
        # 5 s of TERM before the KILL. The footer leaves the event loop in
        # milliseconds; 5 s is the time of a frozen one, not of a busy one.
        for _ in $(seq 50); do
          kill -0 "$previous" 2>/dev/null || break
          sleep 0.1
        done
        if kill -0 "$previous" 2>/dev/null; then
          echo "  the previous footer ($previous) ignored the TERM; killing it" >&2
          kill -9 "$previous" 2>/dev/null || true
          # No `wait`: it is not a child of THIS process. `kill -0` is what
          # answers, and it is what decides.
          for _ in $(seq 50); do
            kill -0 "$previous" 2>/dev/null || break
            sleep 0.1
          done
        fi
      fi
    fi

    # ---------------------------------------------------------------
    # Start the new one
    # ---------------------------------------------------------------
    # In the background and WITHOUT `setsid`. With `setsid`, `$!` would be the
    # wrapper's, and the wrapper exits right after — and the pidfile would hold a
    # dead PID, which is precisely the defect this script exists to avoid.
    # Being reparented to init when we exit is normal behaviour and is enough:
    # what shuts the footer down is `dev.nix`'s `trap`, through the pidfile.
    #
    # THE LOG GOES TO A FILE when `RONOCTUA_LOG` says where.
    #
    # # Why this exists
    #
    # Because the log went to the terminal and nowhere else. When the shortcuts
    # freeze — four times so far — what was known was what was on screen, and the
    # screen does not say which modifier got stuck nor whether the key arrived. A
    # file is what survives closing the terminal, and it is what gets sent along
    # with "it happened again".
    #
    # APPEND, and not truncate: hot reload restarts the footer, and each restart
    # wiping the previous one would lose exactly the part that matters. And no
    # `tee`: a `tee` in the middle would make `$!` be its own, and the pidfile would
    # hold the wrong PID — the defect this script exists to avoid. What puts it on
    # screen is `dev.nix`'s `tail -f`.
    if [ -n "''${RONOCTUA_LOG:-}" ]; then
      "$binary" "$@" >> "$RONOCTUA_LOG" 2>&1 &
    else
      "$binary" "$@" &
    fi
    new_pid=$!
    echo "$new_pid" > "$pidfile"
    echo "  footer $new_pid standing"
  '';
}
