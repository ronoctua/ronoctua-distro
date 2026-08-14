# The terminal wired to the shell — section 7.10.
#
# "A named command opens a terminal in an `xdg_toplevel` of its own and a VM
# test reads on screen what was typed into it."
#
# WHAT ONLY THE VM PROVES
# -----------------------
# The whole chain, and each of its links is a place where the text disappears
# silently:
#
#   xdotool -> niri -> wl_keyboard -> key_bytes -> Pty::escrever
#      -> process -> Pty::pump -> VT thread -> Grade
#      -> Comando::Grade -> Trecho -> wgpu -> pixel -> OCR
#
# A unit test covers the ends (`key_bytes`, `lista_da_grade`) and cannot
# cover the middle: calloop's `ping`, the keyboard focus, the conversion of
# size into cells. If the Enter's `\r` became `\n`, everything would compile,
# every unit test would pass, and the line would never be accepted.
#
# WHY `cat`, AND NOT A SHELL
# --------------------------
# The `cat` gives back exactly what it receives. A shell would print a prompt,
# and the OCR would start depending on someone else's prompt format — a test
# that breaks when nixpkgs's `PS1` changes is not testing our terminal.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  # Two common words and with NO digit and no hyphen: the OCR confuses `1`/`l`,
  # `0`/`O` and eats hyphens in a small monospaced font. What is asserted is that
  # the text crossed over, and not that tesseract is good.
  needle = "terminal funcionando";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-terminal";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
      pkgs.xdotool
      # `setsid --ctty`: it is what gives the child a CONTROLLING terminal. See the
      # top of `crates/ronoctua-term/src/pty.rs`.
      pkgs.util-linux
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

    with subtest("o terminal sobe com o motor VT numa thread propria"):
        machine.succeed(
            "systemd-run --unit=term ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            # AN ABSOLUTE PATH. A `systemd-run` comes up with a minimal PATH, and
            # `setsid` would complain "failed to execute cat" — which shows up on the
            # terminal's screen, and not in the shell's journal.
            "${ronoctua-shell}/bin/ronoctua-shell terminal ${pkgs.coreutils}/bin/cat"
        )
        try:
            machine.wait_until_succeeds(
                "journalctl -u term --no-pager | grep -q 'terminal aberto'",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u term --no-pager")[1])
            raise
        journal = machine.succeed("journalctl -u term --no-pager")
        # The engine's name comes from INSIDE the thread (`Terminal::nome`). If the
        # thread had not come up, there would be no name here at all.
        #
        # WITH QUOTES: `tracing` quotes a `Debug` value, and `engine=alacritty`
        # without them does not match. It is the trap already recorded in the CLAUDE.md.
        assert 'engine="alacritty"' in journal, journal

    with subtest("o COMPOSITOR ve uma janela, nao uma regiao nossa"):
        # Invariant 4: a terminal drawn inside our surface would not show up in
        # niri's IPC.
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'janelas: 1'",
            timeout=30,
        )

    with subtest("o terminal desenhou"):
        machine.wait_until_succeeds(
            "journalctl -u term --no-pager | grep -q 'terminal desenhado no xdg_toplevel'",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u term --no-pager")
        for bad in ["desenhar o frame falhou", "preparar texto falhou", "panicked"]:
            assert bad not in journal, f"o terminal reportou: {bad}"

    with subtest("o que se digita chega ao programa e volta para a tela"):
        # With no window manager nobody calls `XSetInputFocus`: the injected key
        # would be lost with no error, no log, nowhere.
        focus_niri()

        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 '${needle}'"
        )
        # The Enter is the test of the `\r`. With `\n` the line would not be
        # accepted by the driver and the `cat` would never repeat it.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")

        # The redraw comes from the pty's `ping`, not from the key. Waiting for the
        # LOG is the right signal — sleeping a fixed time would be a race.
        machine.wait_until_succeeds(
            "journalctl -u term --no-pager | "
            "grep -c 'terminal desenhado no xdg_toplevel' | awk '$1 > 1'",
            timeout=60,
        )

        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/term.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/term.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        # `funcionando` on its own: it is the longer and less ambiguous word of the
        # pair, and requiring both would turn an OCR error into a terminal error.
        assert "funcionando" in read_back.lower(), (
            f"o que foi digitado nao chegou a tela; OCR leu {read_back!r}"
        )
  '';
}
