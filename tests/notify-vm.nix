# Notifications, history and tray — acceptance criterion of milestone 6.
#
# "Notifications queued ONE AT A TIME; a critical one does not disappear on its
# own; the history opens BELOW the minibuffer, with NO second surface."
#
# The last clause is the one only a VM test proves. "No second surface" is not
# visible in the log nor in the code — it is a property of what the compositor
# has. We ask it: the IPC's `Layers` lists ALL the layer-shell surfaces, and
# ours has to go on being a single one with the history open.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  # THE HEIGHT COMES FROM THE TOKENS, not from a nailed-down number. It used to
  # be `35`, from when the font was 14 px and the line height 1.25 — both
  # changed, and the test started measuring against a value that no longer exists.
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = builtins.ceil (typography.size.base * typography.lineHeight * 2);

in
pkgs.testers.runNixOSTest {
  name = "ronoctua-notify";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
      pkgs.libnotify
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    FOOTER = ${toString footerPx}


    def footer_height():
        """Altura real da superficie na tela, em px."""
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        raw = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop 1x{h}+2+0 +repage -depth 8 txt:-"
        )
        colors = [l.split("#")[1][:6] for l in raw.splitlines()[1:] if "#" in l]
        background = colors[0] if colors else ""
        height = 0
        for color in reversed(colors):
            if color == background:
                break
            height += 1
        return height

    def wait_for_height(expected, tentativas=20):
        """Wait for the surface to reach the given height, and report the value.

        Two lessons in one helper: `assert x == y` with no message hides the
        value that actually came, and measuring right after the paint log runs
        ahead of the COMPOSITOR — the footer has drawn, niri has not composed yet.
        """
        seen_now = None
        for _ in range(tentativas):
            seen_now = footer_height()
            if seen_now == expected:
                return seen_now
            machine.sleep(1)
        raise AssertionError(
            f"o rodape tem {seen_now} px, esperava {expected} "
            f"(depois de {tentativas}s)"
        )



    def layers():
        out_text = machine.succeed(f"${ronoctua-shell}/bin/niri-contract {sock}")
        for line in out_text.splitlines():
            if line.startswith("camadas:"):
                return line
        raise AssertionError(f"niri-contract nao listou camadas:\n{out_text}")


    def notificar(resumo, urgencia="normal", corpo=""):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            f"${pkgs.libnotify}/bin/notify-send -u {urgencia} "
            f"-a testes '{resumo}' '{corpo}'"
        )
        machine.sleep(1)


    def control(command):
        # `comando <id>`, and not one word per feature.
        #
        # The socket spoke a vocabulary of its own (`historico`, `dispensar`)
        # parallel to that of the command ids. Now it speaks COMMAND, and this
        # test exercises the same path `Ctrl+Shift+L` uses.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"${ronoctua-shell}/bin/ronoctua-shell command {command}"
        )
        machine.sleep(1)


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("o rodape sobe com o daemon de notificacoes no barramento"):
        # A SESSION bus of its own: the daemon claims
        # `org.freedesktop.Notifications`, and if there is another one on the
        # machine the name is refused. In this VM there is not, but the explicit
        # address keeps the test independent of what NixOS decides to turn on by default.
        machine.succeed(
            "systemd-run --unit=bus "
            "${pkgs.dbus}/bin/dbus-daemon --session --nofork "
            "--address=unix:path=/run/user/0/bus"
        )
        machine.wait_until_succeeds("test -S /run/user/0/bus", timeout=30)

        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_notify=debug "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'daemon de notificacoes no barramento'",
            timeout=60,
        )
        # THE PAINTING, and not only the daemon: measuring the height before the
        # first `configure` gives zero pixels, with no error at all.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'layout do rodape resolvido'",
            timeout=90,
        )
        wait_for_height(FOOTER)

    with subtest("tres notificacoes, UMA na tela"):
        for i in (1, 2, 3):
            notificar(f"mensagem {i}")
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert journal.count("notificacao recebida") == 3, journal

        # The queue counts the ones that are WAITING: 2, not 3. The one on the
        # screen is not pending.
        #
        # The grep is in the QUEUE's log, not in the segment's: the segment's text
        # starts with a PUA glyph, and `body= 2` does not match
        # `body=<glyph> 2`.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'pending=2'", timeout=30
        )

    with subtest("a critica nao some sozinha"):
        notificar("bateria em 3%", urgencia="critical")
        machine.sleep(9)  # much more than the time of a normal one
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert "urgency=Critical" in journal, journal
        assert "critical=true" in journal, journal
        # Still expanded: the critical one forced the history to open (section
        # 4.3) and did not expire.
        assert footer_height() > FOOTER, "a critica sumiu sozinha"

    with subtest("dispensar a critica e o unico jeito de fecha-la"):
        control("notif-dismiss")
        control("notif-history")  # closes the history the critical one opened
        wait_for_height(FOOTER)

    with subtest("o historico abre ABAIXO do minibuffer, na MESMA superficie"):
        control("notif-history")
        expanded = footer_height()
        assert expanded > FOOTER, f"o historico nao expandiu: {expanded} px"

        # The clause only the VM proves. One surface, not two.
        c = layers()
        print(c)
        assert c.count("ronoctua-footer") == 1, (
            f"o historico criou uma SEGUNDA superficie (invariante 14):\n{c}"
        )

        # And the content is there, on the screen, below the footer's two lines.
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop {w}x{expanded - FOOTER}+0+{h - expanded + FOOTER} +repage "
            "-colorspace gray -negate -resize 400% /tmp/hist.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/hist.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR do historico: {read_back!r}")
        assert "mensagem" in read_back.lower() or "bateria" in read_back.lower(), (
            f"o historico nao esta na tela; OCR leu {read_back!r}"
        )

    with subtest("fechar o historico devolve as duas linhas"):
        control("notif-history")
        wait_for_height(FOOTER)
  '';
}
