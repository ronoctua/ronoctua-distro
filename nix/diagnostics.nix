# A snapshot of what is standing, for when the shortcuts freeze.
#
# # Why this exists
#
# Because the shortcuts froze **four times** and on all four there was no
# evidence at all. The footer's log went to a terminal that had already been
# closed, niri's had the four lines from startup, and what was left was a
# description of the symptom — which does not tell the three possible causes apart:
#
#   1. the key does not reach the footer (the compositor stopped dispatching);
#   2. the key arrives with a stuck modifier and matches no binding;
#   3. the footer does not have keyboard focus (exclusivity was not granted).
#
# All three give the SAME screen: "I press it and nothing happens". And they
# are fixed in opposite places.
#
# What this script does is tell them apart, and it is the same routine that
# solved the fourth occurrence by hand:
#
#   - `wl_display.sync` on the nested compositor. It is the simplest question a
#     Wayland client can ask; whoever does not answer it is not dispatching
#     anything, and then no key would arrive at all. **This is the one that
#     closes the case**: the socket queues corroborate, but the sync proves it.
#   - the socket queues: the `Recv-Q` of niri's IPC says how many connections
#     went without an `accept`, and the footer's `Send-Q` says how many bytes it
#     wrote that the compositor never read.
#   - the layer surface's keyboard interactivity, which answers (3).
#   - the CPU consumed over a time window: it separates "frozen" from "spinning".
#
# All of this is READING. Nothing here sends a command, kills a process or
# changes focus: a diagnostic that disturbs what it is measuring destroys the
# evidence it came for.
{
  writeShellApplication,
  procps,
  iproute2,
  python3,
  niri,
}:

writeShellApplication {
  name = "ronoctua-diagnostics";

  runtimeInputs = [
    procps
    iproute2
    python3
    niri
  ];

  text = ''
    out_text="''${1:-$(mktemp -t ronoctua-diagnostics-XXXXXX.txt)}"
    exec > >(tee "$out_text") 2>&1

    echo "# diagnostics do ronoctua-distro — $(date -Is)"
    echo

    runtime="''${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

    # ---------------------------------------------------------------
    # What is standing
    # ---------------------------------------------------------------
    echo "## processos"
    # `pgrep` to find and `ps` to describe: shellcheck rejects grep over
    # `ps` output (SC2009), and here it happens to be right — the grep
    # would match its own command line.
    #
    # `|| true`: with no match at all `pgrep` exits 1, and under `set -e` that
    # would take diagnostics down in precisely the "nothing is running" case.
    target_pids=$( { pgrep -d, -f "ronoctua|niri --config" || true; } )
    if [ -n "$target_pids" ]; then
      ps -o pid,etime,rss,args -p "$target_pids" || true
    else
      echo "  nada de ronoctua ou niri rodando"
    fi
    echo

    # THE NESTED NIRI, through the IPC socket: `niri.<display>.<pid>.sock`.
    niri_pid=""
    niri_sock=""
    for s in "$runtime"/niri.*.sock; do
      [ -e "$s" ] || continue
      p=$(basename "$s" | rev | cut -d. -f2 | rev)
      if kill -0 "$p" 2>/dev/null; then
        niri_pid="$p"
        niri_sock="$s"
        display=$(basename "$s" | cut -d. -f2)
      fi
    done
    if [ -z "$niri_pid" ]; then
      echo "nenhum niri aninhado de pe — nada mais a medir."
      echo
      echo "o instantaneo esta em $out_text"
      exit 0
    fi
    echo "niri aninhado: pid $niri_pid, display $display"
    echo

    # ---------------------------------------------------------------
    # 1. Is the compositor still dispatching?
    # ---------------------------------------------------------------
    echo "## o compositor responde?"
    WAYLAND_SOCK="$runtime/$display" python3 - <<'PY'
    import os, socket, struct, sys

    path = os.environ["WAYLAND_SOCK"]
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(3)
    try:
        s.connect(path)
    except OSError as e:
        print(f"  wl_display.sync: NAO CONECTA ({e}) — o compositor morreu")
        sys.exit(0)
    # wl_display.sync(new_id=2): object 1, opcode 0, 12 bytes.
    s.sendall(struct.pack("=III", 1, (12 << 16) | 0, 2))
    try:
        s.recv(64)
        print("  wl_display.sync: RESPONDEU — o laco de eventos esta vivo")
    except socket.timeout:
        print("  wl_display.sync: TIMEOUT em 3 s.")
        print("    ISTO SOZINHO NAO QUER DIZER TRAVADO. O niri aninhado e")
        print("    cliente do compositor do host: com a janela dele oculta, o")
        print("    host para de alimenta-lo e ele dorme — sem servir os")
        print("    proprios clientes nem o IPC. Traga a janela para a")
        print("    workspace visivel e rode de novo antes de concluir.")
    PY
    echo

    # ---------------------------------------------------------------
    # 2. The socket queues corroborate
    # ---------------------------------------------------------------
    echo "## filas de socket"
    echo "  (Recv-Q do LISTEN = conexoes sem accept; Send-Q = bytes que o"
    echo "   outro lado nunca leu)"
    ss -x -a -p 2>/dev/null | grep -E "niri\.|ronoctua-" | grep -v "Recv-Q" || true
    echo

    # ---------------------------------------------------------------
    # 3. Frozen or spinning?
    # ---------------------------------------------------------------
    echo "## CPU em 3 s (100 ticks = 1 s)"
    for p in $niri_pid $(pgrep -f "bin/ronoctua-shell$" || true); do
      a=$(awk '{print $14+$15}' "/proc/$p/stat" 2>/dev/null || echo 0)
      echo "$p $a"
    done > /tmp/ronoctua-diag-cpu.$$
    sleep 3
    while read -r p a; do
      b=$(awk '{print $14+$15}' "/proc/$p/stat" 2>/dev/null || echo "$a")
      echo "  pid $p: $((b - a)) ticks   ($(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null | cut -c1-50))"
    done < /tmp/ronoctua-diag-cpu.$$
    rm -f /tmp/ronoctua-diag-cpu.$$
    echo

    # ---------------------------------------------------------------
    # 4. Does the footer have the keyboard?
    # ---------------------------------------------------------------
    echo "## layer surface do rodape"
    if NIRI_SOCKET="$niri_sock" timeout 5 niri msg layers 2>/dev/null | grep -A3 "ronoctua-footer"; then
      :
    else
      echo "  o IPC do niri nao respondeu em 5 s (coerente com o sync acima)"
    fi
    echo

    # ---------------------------------------------------------------
    # 5. The logs
    # ---------------------------------------------------------------
    # ---------------------------------------------------------------
    # 5. The modifiers the compositor announces
    # ---------------------------------------------------------------
    # THIS IS THE LINE THAT CLOSES THE CASE, and it took an hour to work out by hand.
    #
    # A modifier stuck in the nested compositor explains on its own
    # everything one sees: `Alt+Space` stops firing (niri matches a bind by
    # EXACT mask, and the mask has one bit too many), and the click stops
    # reaching the buffers because in the nested backend niri's `Mod` IS Alt —
    # `Mod+drag` means move window, so every click becomes a drag.
    #
    # And it is the value RIGHT NOW, not the one from when the footer had the
    # keyboard: that is why it comes out with the time. The footer only receives
    # modifier announcements while it has focus, and in the broken state it cannot
    # even open the leader.
    echo "## modificadores anunciados pelo compositor"
    last_line=""
    for f in /tmp/ronoctua-footer-*.log; do
      [ -e "$f" ] || continue
      line=$(grep "modificadores anunciados" "$f" | tail -1 || true)
      [ -n "$line" ] && last_line="$line"
    done
    if [ -z "$last_line" ]; then
      echo "  nenhum anuncio no log — a leader nunca chegou a abrir nesta sessao."
      echo "  (sem \$RONOCTUA_LOG o rodape nao guarda log: rode pelo 'nix run .#dev')"
    else
      echo "  $last_line"
      # `alt=true` on its own is NORMAL: in the nested loop the leader is `Alt+Space`,
      # and Alt is held down by construction when the sequence begins.
      if printf '%s' "$last_line" | grep -q "logo.*true"; then
        echo
        echo "  >>> SUPER PRESO. No aninhado nada segura Super legitimamente:"
        echo "      ele ficou latched quando o compositor do HOST capturou um"
        echo "      atalho 'Super+…' e a soltura nunca chegou aqui."
        echo "      Efeito: 'Alt+Espaco' nao casa mais (a mascara tem um bit a"
        echo "      mais) e o teclado inteiro para."
      fi
    fi
    echo

    echo "## fim do log do rodape"
    for f in /tmp/ronoctua-footer-*.log; do
      [ -e "$f" ] || continue
      echo "--- $f"
      tail -40 "$f"
    done
    echo
    echo "## fim do log do niri"
    for f in /tmp/ronoctua-niri-*.log; do
      [ -e "$f" ] || continue
      echo "--- $f"
      tail -20 "$f"
    done

    echo
    echo "o instantaneo esta em $out_text"
  '';
}
