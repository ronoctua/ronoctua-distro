# The clipboard history, proved ON SCREEN and on the clipboard itself.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# That the capture happens. The queue is unit-tested (a password does not go
# in, a pinned entry survives the ceiling), but none of that says that the
# daemon SEES what was copied: that depends on `wl_data_device`, on a real
# compositor, and on the read not hanging the event loop.
#
# And it proves the whole route: copying two things from OUTSIDE, opening the
# view, stepping to the oldest one, and re-copying it — with `wl-paste`
# answering. A check that only looked at the queue would not tell "it kept" from
# "it shows and gives back".
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
  first = "ronoctua-primeiro-copiado";
  second = "ronoctua-segundo-copiado";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-clipboard";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.wl-clipboard
      pkgs.imagemagick
      pkgs.tesseract
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    def copy_text(body):
        """Copia de FORA, como qualquer programa faria."""
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"${pkgs.wl-clipboard}/bin/wl-copy {body}"
        )
        machine.sleep(2)


    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("o rodape sobe"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'socket de controle aberto'",
            timeout=90,
        )
        focus_niri()

    with subtest("o daemon VE o que foi copiado de fora"):
        # The capture is native (`wl_data_device`), and not a `wl-paste --watch`:
        # no process per copy, and the MIME types arrive along with it — which is
        # what the secret filter needs.
        copy_text("${first}")
        copy_text("${second}")
        try:
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager "
                "| grep -c 'entrada nova no historico de clipboard' "
                "| grep -qE '^[2-9]'",
                timeout=30,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -40")[1])
            raise

    with subtest("`LEADER c` abre a vista com as duas entradas"):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )
        press_key("c")
        journal = machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'clipboard aberto'", timeout=30
        )
        print(journal.strip())
        assert "inputs=2" in journal, (
            f"o historico devia ter as duas entradas; log:\n{journal}"
        )

    with subtest("o texto copiado esta NA TELA"):
        # The queue keeping is not the same thing as the region showing. The OCR
        # closes the chain: capture -> queue -> expansion -> span -> pixel.
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/clip.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/clip.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        assert "copiado" in read_back.lower(), (
            f"o conteudo copiado nao esta na tela; OCR leu {read_back!r}"
        )

    with subtest("andar ate a mais antiga e recopia-la devolve ELA"):
        # The route that matters: choosing in the history and putting it back on
        # the clipboard. Re-copying the TOP would prove nothing — it is already
        # there.
        press_key("Down")
        press_key("Return")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'recopiado do historico'",
            timeout=30,
        )
        pasted = machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.wl-clipboard}/bin/wl-paste --no-newline"
        ).strip()
        print(f"area de transferencia: {pasted!r}")
        assert pasted == "${first}", (
            f"esperava a entrada mais antiga (${first}), veio {pasted!r}"
        )

    with subtest("imagem copiada entra no historico e VIRA PREVIA"):
        # What only the VM proves: that the binary offer is read (and not
        # discarded as "it is not UTF-8"), that the image is decoded, and that it
        # reaches the texture pipeline. The decoding itself has a unit test; what
        # is missing is the path — capture -> queue -> preview.
        machine.succeed("systemctl restart footer")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'socket de controle aberto'",
            timeout=60,
        )
        focus_niri()

        # A PNG of a known size, so the log can be checked.
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick -size 64x32 xc:red /tmp/vermelho.png"
        )
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.wl-clipboard}/bin/wl-copy --type image/png < /tmp/vermelho.png"
        )
        machine.sleep(2)

        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )
        press_key("c")

        try:
            journal = machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep 'previa decodificada'", timeout=30
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -30")[1])
            raise
        print(journal.strip())
        # THE REAL DIMENSIONS: a decoder that gave back 0x0 or the wrong size
        # would pass a test that only looked at "it decoded".
        assert "width=64" in journal and "height=32" in journal, (
            f"a previa saiu com o tamanho errado; log:\n{journal}"
        )
  '';
}
