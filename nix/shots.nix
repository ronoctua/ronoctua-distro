# Screenshots of the notification band and the manager, on a real compositor.
#
# WHY A VM, AND NOT A SCRIPT ON THE DEVELOPER'S MACHINE
# -----------------------------------------------------
# Because a screenshot is only worth something if it shows what the SYSTEM
# does, and the system is a layer-shell surface under niri. The path here is
# the one `tests/notify-vm.nix` already uses — Xvfb, niri on winit, `xwd` for
# the pixels — so what comes out is what a check would have measured.
#
# It is a package and not an app: `nix build .#shots` leaves the PNGs in the
# store, and there is nothing to install, nothing to clean up and nothing that
# depends on the machine that ran it.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ../tests/niri-xvfb.nix { inherit pkgs lib; };
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = linePx * 2;
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-shots";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.imagemagick
      pkgs.libnotify
      pkgs.xdotool
      pkgs.xwininfo
      # THE SCREENSHOT'S EDITOR. Without it the `edit` action is not offered —
      # `usable_actions` drops what the machine cannot carry out — and the scene
      # would show three actions where a full install shows four.
      pkgs.swappy
      # `gdbus`, to send the compositor's own `Notify` by hand.
      pkgs.glib
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
    LINE = ${toString linePx}


    def notificar(resumo, corpo="", level="normal", app="testes"):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            f"${pkgs.libnotify}/bin/notify-send -u {level} "
            f"-a {app} '{resumo}' '{corpo}'"
        )
        machine.sleep(1)


    def control(command):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"${ronoctua-shell}/bin/ronoctua-shell command {command}"
        )
        machine.sleep(1)


    def shot(name, lines=None):
        """Crops the footer's region and saves it as a PNG.

        `lines` is how many lines from the bottom to keep; `None` takes the
        whole niri window, which is what the manager's screenshots want.
        """
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        if lines is None:
            crop = f"{w}x{h}+{x}+{y}"
        else:
            tall = LINE * lines
            crop = f"{w}x{tall}+{x}+{y + h - tall}"
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {crop} +repage "
            f"/tmp/{name}.png"
        )
        machine.copy_from_vm(f"/tmp/{name}.png", "")
        print(f"tirou {name}")


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    machine.succeed(
        "systemd-run --unit=bus "
        "${pkgs.dbus}/bin/dbus-daemon --session --nofork "
        "--address=unix:path=/run/user/0/bus"
    )
    machine.wait_until_succeeds("test -S /run/user/0/bus", timeout=30)
    machine.succeed(
        "systemd-run --unit=footer ${xvfb.env} "
        "--setenv=WAYLAND_DISPLAY=wayland-1 "
        "--setenv=HOME=/root "
        f"--setenv=NIRI_SOCKET={sock} "
        "--setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
        "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_notify=debug "
        "--setenv=RONOCTUA_SWAPPY=${pkgs.swappy}/bin/swappy "
        "${ronoctua-shell}/bin/ronoctua-shell"
    )
    machine.wait_until_succeeds(
        "journalctl -u footer --no-pager | grep -q 'layout do rodape resolvido'",
        timeout=90,
    )

    with subtest("o rodape em repouso, com o sino"):
        shot("1-rodape-em-repouso", lines=2)

    with subtest("a faixa discreta, de uma linha"):
        notificar("copiado para a area de transferencia")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'band=1'", timeout=30
        )
        shot("2-faixa-discreta", lines=3)

    with subtest("a fila, com o contador"):
        for i in (1, 2, 3, 4):
            notificar(f"mensagem {i}")
        shot("3-fila-com-contador", lines=3)

    with subtest("a faixa chamativa, de tres linhas"):
        # A critical one is level 90, so the distro's `important-gets-room`
        # catches it: full, loud, and long enough to photograph.
        notificar(
            "bateria em 3%",
            corpo="a maquina desliga em poucos minutos; ligue na tomada",
            level="critical",
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'loud=true'", timeout=30
        )
        shot("4-faixa-chamativa", lines=4)

    with subtest("a captura de tela: glifo proprio da fonte, e as acoes"):
        # THE WHOLE SCREENSHOT PATH, as the compositor drives it.
        #
        # niri sends `app_name = "niri"`, the English summary, and the file's
        # path in the ICON field as a `file://` URL — which is where the actions
        # get the image from. Here the URL is real, so the actions survive
        # `usable_actions`.
        #
        # And the glyph is `md-monitor_screenshot`, named in
        # `assets/notifications.nix` as a NerdFonts glyph rather than a design
        # system token: nothing in the design system says "screen capture".
        machine.succeed("mkdir -p /root/Imagens")
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick -size 320x200 canvas:gray30 "
            "'/root/Imagens/Captura 2026-08-22.png'"
        )
        for _ in range(8):
            control("notif-dismiss")
        # THE RAW `Notify`, exactly as the compositor sends it.
        #
        # `notify-send` is the wrong tool here and the log said so: libnotify
        # loads the file and sends the standard `image-path` hint, leaving
        # `app_icon` empty. niri does neither — it calls the method itself with
        # the `file://` URL IN the icon field (`utils/mod.rs`), and that is the
        # path the `screenshot` entry reads.
        #
        # Reproducing it by hand is the only way this scene shows what actually
        # happens on the machine rather than what libnotify would have done.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "${pkgs.glib}/bin/gdbus call --session "
            "--dest org.freedesktop.Notifications "
            "--object-path /org/freedesktop/Notifications "
            "--method org.freedesktop.Notifications.Notify "
            # EVERY ARGUMENT TYPED, and the last one especially: `-1` on its
            # own is read by GOption as a FLAG, and `gdbus` answers with its
            # usage instead of sending anything. `int32 -1` is one GVariant
            # literal that does not begin with a dash.
            "'niri' 'uint32 0' 'file:///root/Imagens/Captura%202026-08-22.png' "
            "'Screenshot captured' 'You can paste the image from the clipboard.' "
            "'@as []' \"{'urgency': <byte 1>}\" 'int32 -1'"
        )
        machine.sleep(1)
        decided = machine.succeed(
            "journalctl -u footer --no-pager | grep 'notificacao decidida' | tail -1"
        ).strip()
        print(decided)
        assert 'catalogue="screenshot"' in decided, decided
        shot("5-captura-com-acoes", lines=5)

    with subtest("o gerenciador, em duas colunas"):
        control("notif-history")
        shot("6-gerenciador-duas-colunas")

    with subtest("o catalogo da distro, na terceira coluna"):
        # X INPUT FOCUS FIRST. With no window manager nobody calls
        # `XSetInputFocus`, so the focus stays on `PointerRoot` and every key
        # `xdotool` injects is lost — no error, no log, nowhere.
        focus_niri()
        # TAB twice: what happened, what was suppressed, what is configured.
        for _ in (1, 2):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Tab")
            machine.sleep(1)
        shot("7-catalogo-da-distro")
        # And one entry's sheet, which is where "use the distro's default or
        # customise" is actually done.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(1)
        shot("8-folha-de-ajuste")
        control("notif-history")
  '';
}
