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
#   xdotool -> niri -> wl_keyboard -> key_bytes -> Pty::write_bytes
#      -> process -> Pty::pump -> VT thread -> Grid
#      -> grid_list -> DisplayList -> wgpu -> pixel -> OCR
#
# A unit test covers the ends (`key_bytes`, `grid_list`) and cannot
# cover the middle: calloop's `ping`, the keyboard focus, the conversion of
# size into cells. If the Enter's `\r` became `\n`, everything would compile,
# every unit test would pass, and the line would never be accepted.
#
# WHY `cat`, AND NOT A SHELL
# --------------------------
# The `cat` gives back exactly what it receives. A shell would print a prompt,
# and the OCR would start depending on someone else's prompt format — a test
# that breaks when nixpkgs's `PS1` changes is not testing our terminal.
#
# AND THE MONITOR, AT THE END
# ---------------------------
# The last subtest is the other half of `Ctrl+Alt+Esc`: it runs a real `btop`
# under a niri loaded with the GENERATED `window-rule`s and asks the compositor
# — not the screen — whether the window floats. A rule copied into this file
# would prove that a rule written here works, which nobody doubts; what is in
# doubt is whether what the shell EMITS matches the `app_id` the window SETS.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  # THE GENERATED WINDOW RULES, on top of the minimal test config.
  #
  # Not a rule written here: this test exists to prove that what the shell
  # EMITS makes a real niri float a real window. A copy of the rule would prove
  # that a rule written in this file works, which nobody doubts.
  windowRules = pkgs.callPackage ../nix/niri-window-rules.nix { inherit ronoctua-shell; };
  configFloating = pkgs.runCommand "niri-test-floating.kdl" { } ''
    cat ${xvfb.configNiri} ${windowRules} > $out
  '';

  # Two common words and with NO digit and no hyphen: the OCR confuses `1`/`l`,
  # `0`/`O` and eats hyphens in a small monospaced font. What is asserted is that
  # the text crossed over, and not that tesseract is good.
  needle = "terminal working";
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
    start_niri("niri", "${niri}", config="${configFloating}")
    sock = socket_of("niri")

    with subtest("the terminal comes up with the VT engine on a thread of its own"):
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
                "journalctl -u term --no-pager | grep -q 'terminal opened'",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u term --no-pager")[1])
            raise
        journal = machine.succeed("journalctl -u term --no-pager")
        # The engine's name comes from INSIDE the thread (`Terminal::name`). If the
        # thread had not come up, there would be no name here at all.
        #
        # WITH QUOTES: `tracing` quotes a `Debug` value, and `engine=alacritty`
        # without them does not match. It is the trap already recorded in the AGENTS.md.
        assert 'engine="alacritty"' in journal, journal

    with subtest("the COMPOSITOR sees a window, not a region of ours"):
        # Invariant 4: a terminal drawn inside our surface would not show up in
        # niri's IPC.
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 1'",
            timeout=30,
        )

    with subtest("the terminal drew"):
        machine.wait_until_succeeds(
            "journalctl -u term --no-pager | grep -q 'terminal drawn on the xdg_toplevel'",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u term --no-pager")
        for bad in ["drawing the frame failed", "preparing the text failed", "panicked"]:
            assert bad not in journal, f"the terminal reported: {bad}"

    with subtest("what is typed reaches the program and comes back to the screen"):
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
            "grep -c 'terminal drawn on the xdg_toplevel' | awk '$1 > 1'",
            timeout=60,
        )

        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/term.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/term.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        # `working` on its own: it is the longer and less ambiguous word of the
        # pair, and requiring both would turn an OCR error into a terminal error.
        assert "working" in read_back.lower(), (
            f"what was typed did not reach the screen; OCR read {read_back!r}"
        )

    with subtest("the monitor opens FLOATING, and the ordinary terminal does not"):
        import json

        # `LANG`: btop REFUSES to start without a UTF-8 locale — it prints
        # `No UTF-8 locale detected!` in red and exits. On the machine the
        # session has it (`i18n.defaultLocale` reaches the user manager through
        # `pam_env`); a `systemd-run` in a test does not, so the test says it
        # out loud instead of pretending the environment is empty.
        machine.succeed(
            "systemd-run --unit=monitor ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=C.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            "${ronoctua-shell}/bin/ronoctua-shell monitor ${pkgs.btop}/bin/btop"
        )

        def windows():
            out = machine.succeed(
                f"NIRI_SOCKET={sock} ${niri}/bin/niri msg --json windows"
            )
            return {w.get("app_id"): w for w in json.loads(out)}

        # THE COMPOSITOR HAS TO SEE IT — with another `app_id` than the terminal's,
        # which is the whole mechanism: one rule, one window.
        machine.wait_until_succeeds(
            f"NIRI_SOCKET={sock} ${niri}/bin/niri msg --json windows "
            "| grep -q br.ronoctua.monitor",
            timeout=60,
        )
        open_now = windows()
        print(json.dumps(open_now, indent=1))
        assert open_now["br.ronoctua.monitor"]["is_floating"], (
            "the generated rule did not make the monitor float; it opened side by side — "
            "which is exactly what a working shortcut looks like"
        )
        # AND THE ORDINARY TERMINAL DOES NOT FLOAT. Without this the test would
        # pass just as well with a rule matching every window, which is the
        # mistake `LEADER w o` would pay for.
        assert not open_now["br.ronoctua.terminal"]["is_floating"], (
            "the rule caught the ordinary terminal too"
        )

    with subtest("the rescue opens FLOATING, with an identity of its own"):
        # THE UNIT IS `ronoctua-rescue` AND NOT `rescue`. systemd ships its own
        # `rescue.service` — the single-user target — so `systemd-run
        # --unit=rescue` is refused with "already loaded or has a fragment
        # file". The name used to be Portuguese, which by accident could not
        # collide; renaming it to English (invariant 18) walked straight into
        # the collision, and nothing said so until this test ran.
        # THE FIRST DOOR of `Ctrl+Alt+Del`. The second — the VT — is in
        # `tests/rescue-vm.nix`, where there is no compositor at all; this one
        # belongs here because it is the SAME mechanism as the monitor's, and
        # what would break it is the same thing: a rule that matched one app_id
        # and not the other.
        #
        # A third identity and not the monitor's: the rule asks for a bigger
        # window, because what one reads during a failure is a list with a log
        # under it.
        machine.succeed(
            "systemd-run --unit=ronoctua-rescue ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=C.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell rescue "
            "${ronoctua-shell}/bin/ronoctua-rescue"
        )
        try:
            machine.wait_until_succeeds(
                f"NIRI_SOCKET={sock} ${niri}/bin/niri msg --json windows "
                "| grep -q br.ronoctua.rescue",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u ronoctua-rescue --no-pager")[1])
            raise
        with_rescue = windows()
        assert with_rescue["br.ronoctua.rescue"]["is_floating"], (
            "the generated rule did not make the rescue float; it opened side by side"
        )
        # AND IT IS STILL THERE. A console that draws once and dies is
        # indistinguishable from one that works, in a screenshot.
        machine.sleep(5)
        assert "br.ronoctua.rescue" in windows(), (
            "the rescue's window went away: the program inside it died.\n"
            + machine.execute("journalctl -u ronoctua-rescue --no-pager")[1]
        )

        # AND THE PROGRAM STAYS UP. When the child exits the pty closes and the
        # window goes with it (see the `ping` at the end of the pty's thread), so
        # a window still there is a btop still running — which is what proves
        # the terminal gave it what a TUI needs. Without `TERM`, ncurses dies at
        # once; without a UTF-8 locale, btop does.
        machine.sleep(8)
        still = windows()
        assert "br.ronoctua.monitor" in still, (
            "the monitor's window went away: the program inside it died.\n"
            + machine.execute("journalctl -u monitor --no-pager")[1]
        )
        journal = machine.succeed("journalctl -u monitor --no-pager")
        for bad in ["panicked", "drawing the frame failed"]:
            assert bad not in journal, f"the monitor reported: {bad}"
  '';
}
