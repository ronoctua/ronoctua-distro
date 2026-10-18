# Section 5.2's FRAME targets, measured on real hardware — section 9.3.
#
# "The end-to-end frame numbers are measured on real hardware by
# `nix run .#bench`." This file is what makes that sentence true; before,
# the command only printed a warning saying it did not measure them.
#
# # Why this cannot be measured in a check
#
# Section 9.3 forbids measuring performance under lavapipe, and a `runNixOSTest`
# has no GPU. So the frame targets stay OUT of `nix flake check` on
# purpose: what CI gates are the GPU-independent metrics
# (`layout-fast`, `shaping-fast`, `instance-fast`), and what runs here
# needs a real graphics session.
#
# # Why the footer starts from outside, and not from the config
#
# Because the measurement is the footer's LOG, and it has to land in a file
# this script names, once per start-up, with the previous one still readable.
# A child of the compositor writes to whatever the compositor's stdout is, all
# of them into the same stream, and it cannot be restarted without restarting
# the compositor — and restarting the footer is what "six start-ups" means.
# Same reason the VM tests use `systemd-run` with an explicit environment.
{
  lib,
  writeShellApplication,
  scroll,
  ronoctua-shell,
  python3,
  graphicsHost,
}:

writeShellApplication {
  name = "ronoctua-bench-frames";

  runtimeInputs = [
    scroll
    python3
  ];

  text = ''
    startups="''${1:-6}"
    leaders="''${2:-15}"

    ${graphicsHost.prelude}

    # AND THE GBM BACKENDS, as `nix/dev.nix` sets them and for its reason: the
    # nested compositor allocates through GBM, whose loader reads neither
    # `LIBGL_DRIVERS_PATH` nor anything but `/run/opengl-driver/lib/gbm`
    # unless told — and that folder exists on NixOS alone. Without it, on any
    # other host, the compositor never came up and no frame was measured.
    # Derived from what the prelude decided; empty on NixOS and NVIDIA.
    if [ -n "''${LIBGL_DRIVERS_PATH:-}" ]; then
      export GBM_BACKENDS_PATH="''${LIBGL_DRIVERS_PATH%/dri}/gbm"
    fi

    # HEADLESS: NO WINDOW IN THE PERSON'S SESSION.
    #
    # Run on the host's Wayland, the nested compositor was a window there, and
    # three things about it belonged to the host. Its SIZE: the host tiles it,
    # so the `output * mode` below was ignored and the footer was measured at
    # whatever tile the host handed out (on a fractional host, blown up on the
    # screen as well). Its FRAMES: that backend draws when the host calls back,
    # and a host owes no callback to a window it does not show. Its FOCUS: a
    # new window takes it, and what the person typed landed in the measured
    # session. Part of every number was the host's layout, and every round
    # flashed on the person's screen.
    #
    # The headless backend draws on the GPU through a render node, honours the
    # mode to the pixel and opens nothing. `WLR_RENDERER=gles2` because, asked
    # for nothing, wlroots falls back to pixman on a backend with no DRM device
    # of its own when no GPU renderer comes up: the CPU, which section 9.3
    # forbids measuring under any name. Named, it fails instead. The host's
    # `WAYLAND_DISPLAY` and `DISPLAY` are taken from the compositor, so no
    # backend can reach the host's session even if this choice were ignored.
    export WLR_BACKENDS=headless
    export WLR_HEADLESS_OUTPUTS=1
    export WLR_RENDERER=gles2

    # A SECOND INSTANCE, as `nix/dev.nix` names its own (`ronoctua_xdg::INSTANCE`):
    # every root becomes `ronoctua-bench` and every socket `ronoctua-bench-…`.
    # Unnamed, the measured footer read the installed session's preferences —
    # the numbers moved with the person's theme and text size — and talked to
    # its queue and brain. The `leader` and `command` calls below inherit it,
    # which is how they find this footer's control socket.
    export RONOCTUA_INSTANCE=bench

    # `writeShellApplication` turns on `errexit` and `pipefail`. A `grep` that
    # has not yet found what it is looking for returns 1, and inside `var=$(...)` that
    # TAKES THE SCRIPT DOWN — silently, before the error message that exists
    # for precisely that case. That is why the waiting greps carry `|| true`.
    dir=$(mktemp -d -t ronoctua-bench-XXXXXX)
    trap 'rm -rf "$dir"' EXIT
    compositor_log="$dir/compositor.log"
    display_file="$dir/display"

    # THE ROOTS ARE THE ROUND'S. The instance above names them, and these put
    # them in this folder: every round measures the footer as it first starts
    # (no theme, text size or consent of the person's, nothing left by the
    # round before), and nothing is written into the home. The cache stays the
    # person's, so the fonts are as warm as in their session.
    export XDG_CONFIG_HOME="$dir/roots/config"
    export XDG_STATE_HOME="$dir/roots/state"
    export XDG_DATA_HOME="$dir/roots/data"
    mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_DATA_HOME"

    # AND EVERY PROCESS OF THE ROUND CARRIES THE ROUND'S NAME. A client starts
    # the daemon it finds missing beside itself (`ronoctua_brain::client`, and
    # the queue's and the studio's alike), and that daemon outlives the footer
    # on purpose: killing the footer left a brain, a queue and a studio running
    # for good, the brain acting on the machine. They are found by this mark at
    # the end, never by name (AGENTS.md: `pkill 'ronoctua-'` is the installed
    # session too).
    export RONOCTUA_BENCH_ROUND="$dir"

    # THE IPC SOCKET IS NAMED BEFORE THE COMPOSITOR STARTS, which is what
    # `SCROLLSOCK` is for: set to a path that does not exist yet, it is the path
    # the compositor binds instead of one it invents
    # (`sway/ipc-server.c`). So the socket stops being something to go looking
    # for — no glob, no sort order, no second compositor to be confused with.
    export SCROLLSOCK="$dir/ipc.sock"

    # A MINIMAL config: nothing spawned (see the header) and no leader bind,
    # because the measurement fires the leader through the control socket.
    #
    # The one `exec` is how the script learns the DISPLAY: the compositor picks
    # its Wayland socket with `wl_display_add_socket_auto`, so the name is not
    # knowable from outside and not passable in — the compositor itself is the
    # only honest source. Reading the log does not work either: the `RUST_LOG`
    # this script exports for the FOOTER also reaches the compositor, and a
    # filter naming our crates leaves it perfectly silent.
    cat > "$dir/config" <<CONF
    scrollnag_command -
    animations enabled no
    input type:keyboard {
        xkb_layout "br"
    }
    focus_follows_mouse yes
    default_border none
    titlebar_padding 1
    gaps inner 0
    gaps outer 0
    output * mode 1600x900
    output * scale 1
    exec sh -c 'printf %s "\$WAYLAND_DISPLAY" > "$display_file"'
    CONF

    echo "== nested compositor =="
    env -u WAYLAND_DISPLAY -u DISPLAY scroll -c "$dir/config" > "$compositor_log" 2>&1 &
    compositor_proc=$!
    footer_proc=""
    cleanup() {
      [ -n "$footer_proc" ] && kill "$footer_proc" 2>/dev/null || true
      kill "$compositor_proc" 2>/dev/null || true
      wait "$compositor_proc" 2>/dev/null || true
      # Everything else that carries the round's mark: the daemons above. The
      # scan skips itself and this script, which carry it too.
      python3 - "$dir" "$$" <<'PY'
    import os, signal, sys
    mark = ("RONOCTUA_BENCH_ROUND=" + sys.argv[1]).encode()
    skip = {os.getpid(), int(sys.argv[2])}
    for pid in filter(str.isdigit, os.listdir("/proc")):
        if int(pid) in skip:
            continue
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = f.read().split(b"\0")
        except OSError:
            continue
        if mark in env:
            try:
                os.kill(int(pid), signal.SIGTERM)
            except OSError:
                pass
    PY
      # Their sockets stay on the disk, as a server killed by a signal leaves
      # its socket; the instance is the bench's alone.
      if [ -n "''${XDG_RUNTIME_DIR:-}" ]; then
        rm -f "$XDG_RUNTIME_DIR"/ronoctua-bench-*.sock
      fi
    }
    trap 'cleanup; rm -rf "$dir"' EXIT

    display=""
    for _ in $(seq 60); do
      if [ -s "$display_file" ]; then
        display=$(cat "$display_file")
        break
      fi
      # If the process died, waiting the full 30 s only delays the error.
      kill -0 "$compositor_proc" 2>/dev/null || break
      sleep 0.5
    done
    if [ -z "$display" ]; then
      echo "ERROR: the nested compositor did not come up." >&2
      tail -20 "$compositor_log" >&2
      exit 1
    fi
    echo "display=$display"
    sleep 2

    start_footer() {
      # NO_COLOR: `tracing` interleaves ANSI escapes BETWEEN the fields, and
      # `milestone=...` stops matching any grep.
      env WAYLAND_DISPLAY="$display" NO_COLOR=1 \
          RUST_LOG="ronoctua_shell=info,ronoctua_render=info" \
          ${ronoctua-shell}/bin/ronoctua-shell > "$1" 2>&1 &
      footer_proc=$!
      for _ in $(seq 90); do
        if grep -q "5.2's target" "$1" 2>/dev/null; then return 0; fi
        kill -0 "$footer_proc" 2>/dev/null || return 1
        sleep 0.5
      done
      return 1
    }

    echo
    echo "== first frame: $startups start-ups =="
    for i in $(seq "$startups"); do
      if start_footer "$dir/startup-$i.log"; then
        echo "  startup $i ok"
      else
        echo "  startup $i FAILED" >&2
        tail -5 "$dir/startup-$i.log" >&2
      fi
      # The last one stays up for the which-key phase.
      if [ "$i" != "$startups" ]; then
        kill "$footer_proc" 2>/dev/null || true
        wait "$footer_proc" 2>/dev/null || true
        sleep 1
      fi
    done

    echo
    echo "== which-key: $leaders openings =="
    for _ in $(seq "$leaders"); do
      env WAYLAND_DISPLAY="$display" ${ronoctua-shell}/bin/ronoctua-shell leader \
        >/dev/null 2>&1 || true
      sleep 0.5
      env WAYLAND_DISPLAY="$display" ${ronoctua-shell}/bin/ronoctua-shell command notif-dismiss \
        >/dev/null 2>&1 || true
      # THE NEXT LEADER MUST OPEN. The leader pressed again with its list open
      # walks to the next set (`crate::hidden::leader_again`) and records no
      # opening, so the one just opened is CLOSED, the way its `Esc` closes
      # it — and every round measures one opening, which the count below
      # holds the log to.
      env WAYLAND_DISPLAY="$display" ${ronoctua-shell}/bin/ronoctua-shell command leader-close \
        >/dev/null 2>&1 || true
      sleep 0.4
    done
    sleep 1

    echo
    python3 - "$dir" "$leaders" <<'PY'
    import pathlib, re, statistics as st, sys

    # QUOTES around the value: `tracing` adds them on a `Debug` field. It is
    # already in AGENTS.md, and it has already cost one measurement round.
    # The field is the renderer's `milestone` (`ronoctua_render::gpu`): it
    # was `marco` once, and the rename left this pattern matching nothing.
    PATTERN = re.compile(r'milestone="([a-z-]+)" ms=([0-9.]+)')
    TARGETS = {"first-frame": 200.0, "which-key": 16.7}

    d = {}
    for f in sorted(pathlib.Path(sys.argv[1]).glob("startup-*.log")):
        for item_name, ms in PATTERN.findall(f.read_text(errors="replace")):
            d.setdefault(item_name, []).append(float(ms))

    if not d:
        print("ERROR: no milestone in the log at all", file=sys.stderr)
        raise SystemExit(1)

    print("== section 5.2's targets ==")
    failed = False
    # EVERY OPENING ASKED FOR IS ONE MEASURED. A p99 over whatever samples
    # the log holds passed on ONE, when every leader after the first switched
    # sets instead of opening: the count is part of the answer.
    asked = int(sys.argv[2])
    got = len(d.get("which-key", []))
    if got != asked:
        print(f"  NO  which-key: {got} openings measured, {asked} asked for")
        failed = True
    for item_name, target_path in TARGETS.items():
        v = sorted(d.get(item_name, []))
        if not v:
            print(f"  {item_name}: not measured", file=sys.stderr)
            failed = True
            continue
        p99 = v[min(len(v) - 1, int(len(v) * 0.99))]
        ok = p99 <= target_path
        failed = failed or not ok
        print(
            f"  {'OK ' if ok else 'NO '} {item_name:15s} n={len(v):3d}"
            f"  p50={st.median(v):8.1f}  p99={p99:8.1f}"
            f"  max={v[-1]:8.1f} ms   (target {target_path} ms)"
        )
    print()
    print("Measured up to `Queue::present`, which on Wayland does attach+commit: it is")
    print("the instant the frame leaves OUR hands. Still to come are the compositor")
    print("composing and the panel scanning out, which do not depend on us.")
    raise SystemExit(1 if failed else 0)
    PY
  '';

  meta = {
    description = "Measures section 5.2's frame targets on real hardware";
    platforms = lib.platforms.linux;
  };
}
