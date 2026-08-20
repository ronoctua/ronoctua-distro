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
# # Why the footer starts from outside, and not through `spawn-at-startup`
#
# niri sends the output of the processes it spawns to `/dev/null`. With
# `spawn-at-startup` the footer's log simply does not exist — and the log IS the
# measurement. Same reason the VM tests use `systemd-run` with an explicit
# environment.
{
  lib,
  writeShellApplication,
  niri,
  ronoctua-shell,
  python3,
  graphicsHost,
}:

writeShellApplication {
  name = "ronoctua-bench-frames";

  runtimeInputs = [
    niri
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
    niri_log="$dir/niri.log"

    # A MINIMAL config: no `spawn-at-startup` (see the header) and no leader
    # bind, because the measurement fires the leader through the control socket.
    cat > "$dir/niri.kdl" <<'KDL'
    prefer-no-csd
    input {
        keyboard {
            xkb { layout "br"; }
        }
        focus-follows-mouse
    }
    layout {
        gaps 0
        border { off; }
        focus-ring { off; }
        shadow { off; }
    }
    output "winit" {
        mode "1600x900"
        scale 1
    }
    hotkey-overlay { skip-at-startup; }
    KDL

    echo "== compositor aninhado =="
    niri --config "$dir/niri.kdl" > "$niri_log" 2>&1 &
    niri_proc=$!
    footer_proc=""
    cleanup() {
      [ -n "$footer_proc" ] && kill "$footer_proc" 2>/dev/null || true
      kill "$niri_proc" 2>/dev/null || true
      wait "$niri_proc" 2>/dev/null || true
    }
    trap 'cleanup; rm -rf "$dir"' EXIT

    # THE DISPLAY COMES FROM THE IPC SOCKET'S NAME, and not from niri's log.
    #
    # niri names it `niri.<display>.<pid>.sock`, so the pid we have just
    # spawned identifies the socket unambiguously — two niris on the same
    # machine do not get confused, and it does not depend on the directory
    # being quiet.
    #
    # READING THE LOG DOES NOT WORK. The `RUST_LOG` this script exports for the
    # FOOTER also reaches niri, which uses the same `EnvFilter`: with
    # `ronoctua_shell=debug` the compositor comes up perfectly and stays MUTE,
    # the grep never matches, and the message claims "niri did not start" —
    # which is false.
    display=""
    for _ in $(seq 60); do
      # `find` and not `ls`: shellcheck rejects `ls` on a glob (SC2012), and
      # here it happens to be right — `-quit` stops at the first one and does
      # not depend on sort order.
      s=$(find "''${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" -maxdepth 1 \
            -name "niri.*.$niri_proc.sock" -print -quit 2>/dev/null || true)
      if [ -n "$s" ]; then
        sock="$s"
        # `niri.<display>.<pid>.sock` — the display is the middle field.
        display=$(basename "$s" | cut -d. -f2)
        break
      fi
      # If the process died, waiting the full 30 s only delays the error.
      kill -0 "$niri_proc" 2>/dev/null || break
      sleep 0.5
    done
    if [ -z "$display" ]; then
      echo "ERRO: o compositor aninhado nao subiu." >&2
      tail -20 "$niri_log" >&2
      exit 1
    fi
    echo "display=$display"
    sleep 2

    start_footer() {
      # NO_COLOR: `tracing` interleaves ANSI escapes BETWEEN the fields, and
      # `marco=...` stops matching any grep.
      env WAYLAND_DISPLAY="$display" NIRI_SOCKET="$sock" NO_COLOR=1 \
          RUST_LOG="ronoctua_shell=info,ronoctua_render=info" \
          ${ronoctua-shell}/bin/ronoctua-shell > "$1" 2>&1 &
      footer_proc=$!
      for _ in $(seq 90); do
        if grep -q 'alvo da 5.2' "$1" 2>/dev/null; then return 0; fi
        kill -0 "$footer_proc" 2>/dev/null || return 1
        sleep 0.5
      done
      return 1
    }

    echo
    echo "== primeiro frame: $startups subidas =="
    for i in $(seq "$startups"); do
      if start_footer "$dir/subida-$i.log"; then
        echo "  subida $i ok"
      else
        echo "  subida $i FALHOU" >&2
        tail -5 "$dir/subida-$i.log" >&2
      fi
      # The last one stays up for the which-key phase.
      if [ "$i" != "$startups" ]; then
        kill "$footer_proc" 2>/dev/null || true
        wait "$footer_proc" 2>/dev/null || true
        sleep 1
      fi
    done

    echo
    echo "== which-key: $leaders aberturas =="
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
    # already in CLAUDE.md, and it has already cost one measurement round.
    PATTERN = re.compile(r'marco="([a-z-]+)" ms=([0-9.]+)')
    TARGETS = {"primeiro-frame": 200.0, "which-key": 16.7}

    d = {}
    for f in sorted(pathlib.Path(sys.argv[1]).glob("subida-*.log")):
        for item_name, ms in PATTERN.findall(f.read_text(errors="replace")):
            d.setdefault(item_name, []).append(float(ms))

    if not d:
        print("ERRO: nenhum marco no log", file=sys.stderr)
        raise SystemExit(1)

    print("== alvos da secao 5.2 ==")
    failed = False
    for item_name, target_path in TARGETS.items():
        v = sorted(d.get(item_name, []))
        if not v:
            print(f"  {item_name}: nao medido", file=sys.stderr)
            failed = True
            continue
        p99 = v[min(len(v) - 1, int(len(v) * 0.99))]
        ok = p99 <= target_path
        failed = failed or not ok
        print(
            f"  {'OK ' if ok else 'NAO'} {item_name:15s} n={len(v):3d}"
            f"  p50={st.median(v):8.1f}  p99={p99:8.1f}"
            f"  max={v[-1]:8.1f} ms   (alvo {target_path} ms)"
        )
    print()
    print("Medido ate `Queue::present`, que no Wayland faz attach+commit: e o")
    print("instante em que o frame sai das NOSSAS maos. Falta o compositor")
    print("compor e o painel varrer, o que nao depende de nos.")
    raise SystemExit(1 if failed else 0)
    PY
  '';

  meta = {
    description = "Mede os alvos de frame da secao 5.2 em hardware real";
    platforms = lib.platforms.linux;
  };
}
