# A native buffer in a window of its own — acceptance criterion of milestone 7.
#
# "The first native buffer end to end with an `xdg_toplevel` of its own."
#
# WHAT ONLY THE VM PROVES
# -----------------------
# That the compositor sees a WINDOW, and not a region drawn by us. The
# difference matters: invariant 4 says the shell never reimplements window
# layout, and drawing buffers inside a surface of ours would be exactly that.
# We ask niri — the IPC's `Windows` lists the windows, and the buffer's has to
# be there, with our `app_id`.
#
# And the content has to be ON SCREEN: the component produces a `DisplayList`
# with a text key, the host translates it and draws. The OCR closes the whole
# chain — component, protocol, i18n, tokens, wgpu.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
  bufferExample,
  bufferGlyphs,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-buffer";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
      # `wl-paste` to READ the clipboard. The `wl-copy` that writes it arrives
      # through the store path inside the binary.
      pkgs.wl-clipboard
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("o buffer sobe e o componente renderiza"):
        machine.succeed(
            "systemd-run --unit=buffer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # pt-BR: the key the component sends has to come out TRANSLATED.
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            "${ronoctua-shell}/bin/ronoctua-shell buffer "
            "${bufferExample}/lib/ronoctua_buffer_example.wasm"
        )
        try:
            machine.wait_until_succeeds(
                "journalctl -u buffer --no-pager | grep -q 'componente renderizou'",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u buffer --no-pager")[1])
            raise

        journal = machine.succeed("journalctl -u buffer --no-pager")
        # THREE commands: title, rule and the key counter — the example only
        # emits the third when there are more than two lines of context, and the
        # host asks for 24. If the `render` gave back empty, everything below
        # would pass with a blank window.
        assert "commands=3" in journal, journal

    with subtest("o COMPOSITOR ve uma janela, nao uma regiao nossa"):
        # The proof of invariant 4. A buffer drawn inside our surface would not
        # show up here.
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'janelas: 1'",
            timeout=30,
        )
        out_text = machine.succeed(f"${ronoctua-shell}/bin/niri-contract {sock}")
        print(out_text)
        assert "focada: Some" in out_text, out_text

    with subtest("o buffer desenhou na janela"):
        machine.wait_until_succeeds(
            "journalctl -u buffer --no-pager | grep -q 'buffer desenhado no xdg_toplevel'",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u buffer --no-pager")
        for bad in ["desenhar o frame falhou", "preparar texto falhou", "panicked"]:
            assert bad not in journal, f"o buffer reportou: {bad}"

    with subtest("a chave do componente saiu TRADUZIDA na tela"):
        # The whole chain: component -> DisplayList -> i18n -> tokens -> wgpu ->
        # pixel. The component sends `cmd-mx-open`; in pt-BR that is
        # "Executar comando…", and that is what the OCR has to read.
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        # THE WHOLE WINDOW, and not a crop at the top: whoever decides where the
        # window goes is niri (invariant 4), and it does not put it at (0,0) —
        # its column has a width of its own and the window is centered in it.
        # Cropping by an assumed position is assuming layout, which is precisely
        # what the invariant forbids the shell to do.
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/buffer.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/buffer.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        assert "executar" in read_back.lower(), (
            f"a chave nao saiu traduzida na tela; OCR leu {read_back!r}"
        )

    with subtest("COPIAR copia de verdade, na area de transferencia do sistema"):
        # THE DEFECT THIS CATCHES, and that nobody saw for months.
        #
        # `Efeito::Copiar` reached the host and became a `tracing::info!`. The
        # glyph browser's nine copy formats NEVER put anything anywhere, and the
        # feature looked finished — the worst failure mode this project knows.
        #
        # The criterion is the CLIPBOARD, and not the call: a test that asserted
        # "the wl-copy was invoked" would repeat the same defect one level up.
        machine.succeed("systemctl stop buffer")
        machine.succeed(
            "systemd-run --unit=glyphs ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell buffer "
            "${bufferGlyphs}/lib/ronoctua_buffer_glyphs.wasm"
        )
        machine.wait_until_succeeds(
            "journalctl -u glyphs --no-pager | grep -q 'buffer desenhado no xdg_toplevel'",
            timeout=90,
        )

        # With no WM on Xvfb nobody has input focus, and the key would be lost.
        focus_niri()
        machine.sleep(1)
        # The FIRST Enter opens the sheet; the SECOND copies the selected option,
        # which is the first of the list — the glyph's NAME.
        for _ in range(2):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
            machine.sleep(2)

        try:
            machine.wait_until_succeeds(
                "journalctl -u glyphs --no-pager | grep -q 'copiado'", timeout=30
            )
        except Exception:
            print(machine.execute("journalctl -u glyphs --no-pager | tail -40")[1])
            raise

        pasted = machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.wl-clipboard}/bin/wl-paste --no-newline"
        ).strip()
        print(f"area de transferencia: {pasted!r}")
        # A Nerd Fonts glyph NAME: `<set>-<thing>`. Which one is not nailed down —
        # the order of the list with no search depends on the database, and nailing
        # the name down would make the test fail on a font update for the wrong reason.
        assert "-" in pasted and pasted.isascii() and len(pasted) > 3, (
            f"o que foi colado nao parece nome de glifo: {pasted!r}"
        )

    with subtest("Ctrl+V le a area de transferencia e entrega ao buffer"):
        # The other side of the same thing. `Ctrl+V` fell into the host's Ctrl
        # guard and did NOTHING — and there was no way of telling that from
        # "there was nothing copied", which was exactly the owner's doubt.
        #
        # What is asserted here is the HOST's end: it really read the clipboard
        # and delivered the text to the component. That the text goes into the
        # search is proved by `browser-filters`, which drives the component
        # directly — together, the two close the path.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Escape")
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+v")
        machine.sleep(2)

        journal = machine.succeed("journalctl -u glyphs --no-pager")
        target_path = f'input=Paste("{pasted}")'
        assert target_path in journal, (
            f"`Ctrl+V` nao entregou o texto ao buffer; esperava {target_path!r} no log:\n"
            + journal[-2000:]
        )
        print(f"Ctrl+V entregou: {pasted!r}")
  '';
}
