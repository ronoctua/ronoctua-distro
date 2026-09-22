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
    # The footer's ENVIRONMENT is inherited (WAYLAND_DISPLAY, RUST_LOG, SCROLLSOCK).
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
        # SAID OUT LOUD, so the watcher below knows this death was asked for.
        #
        # Hot reload kills the footer on every rebuild, and a watcher that
        # could not tell that from a crash would raise the rescue line on
        # every save — which is the shape of alarm nobody reads after the
        # second time.
        echo "$previous" > "$pidfile.onpurpose"
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

    # ---------------------------------------------------------------
    # AND SOMEBODY NOTICES IF IT DIES ON ITS OWN
    # ---------------------------------------------------------------
    # In production this is `OnFailure=ronoctua-lifeline.service` on the unit,
    # and it costs nothing at all. Here there is no unit: `dev.nix` waits on
    # the COMPOSITOR, so a footer that aborts leaves the screen bare until somebody saves
    # a file — which is exactly where the owner met a crash and reported that
    # the distro "acaba deixando o usuário sem interface".
    #
    # A POLL, AND IT IS THE ONLY ONE IN THIS TREE. `wait` would be exact and
    # cannot be used: this script has to return so the loop can go on, and a
    # subshell may not wait for a process its PARENT started. One `kill -0` a
    # second against something that is almost always alive is the cheapest
    # correct thing, and it lives only in the nested loop.
    #
    # THE LINE IS RAISED AFTER THE FOOTER IS BACK, in that order, because the
    # production side does the same: `Restart=on-failure` brings it back and
    # the line says it fell. A line over a screen that is still bare would be
    # the rescue arriving before the rescue.
    (
      while kill -0 "$new_pid" 2>/dev/null; do sleep 1; done

      # ---------------------------------------------------------------
      # THREE REASONS NOT TO RAISE IT AGAIN, and any one of them is enough
      # ---------------------------------------------------------------
      # Because this loop re-executes the script, which starts another footer
      # AND another copy of this subshell — so it has to be able to stop, and
      # for a while it could not. The owner ended a session with Ctrl+C and his
      # terminal filled with `footer <pid> standing` and `there is no reachable
      # Wayland compositor`, one turn every 2.01 s, for an hour. Measured:
      # ~2000 turns and ~8000 processes.
      #
      # It is orphaned and SIGINT-proof BY CONSTRUCTION, and neither is fixable
      # from here. Bash sets SIGINT and SIGQUIT to SIG_IGN for an asynchronous
      # command when job control is off (`SigIgn: 0000000000000006` in
      # /proc/<pid>/status, measured), SIG_IGN survives `execve` so every
      # generation is born immune too, and a non-interactive bash cannot install
      # a trap for a signal ignored on entry. So the stopping is not a signal:
      # it is three questions, asked before doing it again.

      # ONE. THE DEATH WAS ASKED FOR. The restart block above writes this file,
      # and so does `dev.nix`'s `shutdown` — which for a year it did not, and
      # that is the whole of why the loop had no end.
      if [ -s "$pidfile.onpurpose" ] && [ "$(cat "$pidfile.onpurpose")" = "$new_pid" ]; then
        rm -f "$pidfile.watcher"
        exit 0
      fi

      # TWO. THE COMPOSITOR THAT FOOTER WAS BORN UNDER IS STILL THERE — and the
      # question is its PID, never whether its socket file is on disk. A
      # Wayland server killed by a signal does not unlink its own socket:
      # measured, `/run/user/1000/wayland-2` was still there an hour after the
      # compositor that made it was gone, which is also why the lifeline answers
      # "could not find wayland compositor" (it connects and takes
      # ECONNREFUSED) instead of hanging. A `[ -S … ]` test here would pass with
      # the whole defect in place.
      #
      # THE PID IS HANDED OVER and no longer derived. It used to be read out of
      # the IPC socket's NAME, because the previous compositor put it there;
      # this one takes `SCROLLSOCK` as a path, and `dev.nix` gives it a
      # `mktemp -u` name with nothing in it to read. So `dev.nix` exports the
      # pid it already holds (`$!`), and this costs no process either.
      if [ -n "''${RONOCTUA_COMPOSITOR_PID:-}" ]; then
        case "$RONOCTUA_COMPOSITOR_PID" in
          *[!0-9]* | "") ;;
          *)
            kill -0 "$RONOCTUA_COMPOSITOR_PID" 2>/dev/null ||
              { rm -f "$pidfile.watcher"; exit 0; }
            ;;
        esac
      fi

      # THREE. THE SESSION THAT ASKED FOR A FOOTER IS STILL RUNNING. Reparented
      # or not, this stays answerable — which is exactly what being reparented
      # takes away from every other way of asking.
      if [ -n "''${RONOCTUA_DEV_PID:-}" ] && ! kill -0 "$RONOCTUA_DEV_PID" 2>/dev/null; then
        rm -f "$pidfile.watcher"
        exit 0
      fi

      # AND A CEILING ON TOP OF THE THREE, because a footer that dies AS it
      # starts answers yes to all of them and would spin at the speed of the
      # poll for as long as the session lasts. Five in a minute is a crash loop
      # and not a crash: the line is raised once, and this stops. `EPOCHSECONDS`
      # and not `date`, so the guard against runaway processes is not itself two
      # processes a turn.
      tries=$(( ''${RONOCTUA_FOOTER_TRIES:-0} + 1 ))
      since=''${RONOCTUA_FOOTER_SINCE:-$EPOCHSECONDS}
      if [ "$(( EPOCHSECONDS - since ))" -gt 60 ]; then
        tries=1
        since=$EPOCHSECONDS
      fi
      if [ "$tries" -gt 5 ]; then
        echo "  the footer died $tries times in a minute; not starting it again" >&2
        rm -f "$pidfile.watcher"
        "$binary" lifeline ronoctua-footer.service || true
        exit 0
      fi
      export RONOCTUA_FOOTER_TRIES="$tries"
      export RONOCTUA_FOOTER_SINCE="$since"

      # THE SAME SECOND THE UNIT WAITS. `RestartSec=1` over there; a tight loop
      # here would be a crash loop spinning at the speed of the poll.
      sleep 1
      "$0" "$pidfile" "$binary" "$@" || true
      "$binary" lifeline ronoctua-footer.service || true
    ) &
    # AND THE WATCHER IS REACHABLE NOW, which it was not.
    #
    # `dev.nix` held the footer's PID in a file and the watcher's in nothing at
    # all, so the one thing that could resurrect a footer was the one thing the
    # session could not stop. It goes beside the footer's pidfile, by the
    # argument at the top of this file: whoever starts and whoever stops are
    # different processes, and state between two processes lives in the
    # filesystem or does not exist.
    echo $! > "$pidfile.watcher"
  '';
}
