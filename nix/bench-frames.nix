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

    # `writeShellApplication` turns on `errexit` and `pipefail`. A `grep` that
    # has not yet found what it is looking for returns 1, and inside `var=$(...)` that
    # TAKES THE SCRIPT DOWN — silently, before the error message that exists
    # for precisely that case. That is why the waiting greps carry `|| true`.
    dir=$(mktemp -d -t ronoctua-bench-XXXXXX)
    trap 'rm -rf "$dir"' EXIT
    compositor_log="$dir/compositor.log"
    display_file="$dir/display"

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
    scroll -c "$dir/config" > "$compositor_log" 2>&1 &
    compositor_proc=$!
    footer_proc=""
    cleanup() {
      [ -n "$footer_proc" ] && kill "$footer_proc" 2>/dev/null || true
      kill "$compositor_proc" 2>/dev/null || true
      wait "$compositor_proc" 2>/dev/null || true
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
      # `marco=...` stops matching any grep.
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
      sleep 0.4
    done
    sleep 1

    echo
    python3 - "$dir" <<'PY'
    import pathlib, re, statistics as st, sys

    # QUOTES around the value: `tracing` adds them on a `Debug` field. It is
    # already in AGENTS.md, and it has already cost one measurement round.
    PATTERN = re.compile(r'marco="([a-z-]+)" ms=([0-9.]+)')
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
