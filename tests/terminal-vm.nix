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
#   xdotool -> the compositor -> wl_keyboard -> key_bytes -> Pty::write_bytes
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
# under a compositor loaded with the GENERATED `window-rule`s and asks the compositor
# — not the screen — whether the window floats. A rule copied into this file
# would prove that a rule written here works, which nobody doubts; what is in
# doubt is whether what the shell EMITS matches the `app_id` the window SETS.
#
# AND THE CALCULATOR, LAST
# ------------------------
# The same generated rules open the calculator's window FLOATING and compact
# (`crates/ronoctua-shell-door/src/floating.rs`), and its header's control — `C-f`,
# typed into it — asks the compositor, through the contract
# (`back_in_the_layout`, `float_centred`), to tile it and to float it again.
# Only a real compositor says whether the window it found by its own pid and
# `app_id` moved.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };

  # THE GENERATED WINDOW RULES, on top of the minimal test config.
  #
  # Not a rule written here: this test exists to prove that what the shell
  # EMITS makes a real compositor float a real window. A copy of the rule would prove
  # that a rule written in this file works, which nobody doubts.
  windowRules = pkgs.callPackage ../nix/compositor-window-rules.nix { inherit ronoctua-shell; };
  configFloating = pkgs.runCommand "scroll-test-floating.conf" { } ''
    cat ${xvfb.configCompositor} ${windowRules} > $out
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
      scroll
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
    start_compositor("compositor", "${scroll}", config="${configFloating}")
    sock = socket_of("compositor")

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
                "journalctl -u term --no-pager | grep 'terminal opened' > /dev/null",
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
        # the compositor's IPC.
        await_windows(sock, 1, timeout=30)

    with subtest("the terminal drew"):
        machine.wait_until_succeeds(
            "journalctl -u term --no-pager | grep 'terminal drawn on the xdg_toplevel' > /dev/null",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u term --no-pager")
        for bad in ["drawing the frame failed", "preparing the text failed", "panicked"]:
            assert bad not in journal, f"the terminal reported: {bad}"

    with subtest("what is typed reaches the program and comes back to the screen"):
        import time

        # With no window manager nobody calls `XSetInputFocus`: the injected key
        # would be lost with no error, no log, nowhere.
        focus_compositor()

        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 '${needle}'"
        )
        # The Enter is the test of the `\r`. With `\n` the line would not be
        # accepted by the driver and the `cat` would never repeat it.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")

        def read_the_screen():
            # THE WINDOW'S OWN RECTANGLE, asked of the compositor.
            #
            # Cropping the whole OUTPUT was the old way of not assuming where a
            # window goes (invariant 4 forbids assuming), and it stops working the
            # moment a layout puts the window in a column of its own: the rest of
            # the crop is empty desk, and `tesseract --psm 6` — which expects one
            # uniform block of text — reads nothing from it. `window_box` reads the
            # rectangle instead, which is asking rather than assuming.
            w, h, x, y = window_box(sock, "br.ronoctua.terminal")
            machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
                "-colorspace gray -negate -resize 200% /tmp/term.png"
            )
            return machine.succeed(
                "${pkgs.tesseract}/bin/tesseract /tmp/term.png stdout --psm 6 2>/dev/null"
            ).strip()

        # THE SCREEN ITSELF, asked until it says so — for a minute or more.
        #
        # A count of the terminal's draws was the signal, and it says nothing of
        # the text: a loaded machine draws the window twice before a key is
        # typed (at its first size, and again at the size the compositor
        # configures), so "more than one draw" held at once and the screen was
        # read before the echo came; and a draw that carries the echo is not yet
        # a frame the compositor showed. What is asserted is what `xwd` reads,
        # so that is what is waited for.
        #
        # `working` on its own: it is the longer and less ambiguous word of the
        # pair, and requiring both would turn an OCR error into a terminal error.
        read_back = ""
        for _ in range(15):
            read_back = read_the_screen()
            print(f"OCR: {read_back!r}")
            if "working" in read_back.lower():
                break
            time.sleep(1)
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

        # THE COMPOSITOR HAS TO SEE IT — with another `app_id` than the terminal's,
        # which is the whole mechanism: one rule, one window.
        machine.wait_until_succeeds(
            f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree "
            "| grep br.ronoctua.monitor > /dev/null",
            timeout=60,
        )
        open_now = windows_by_app_id(sock)
        print(json.dumps(open_now, indent=1))
        assert open_now["br.ronoctua.monitor"]["floating"], (
            "the generated rule did not make the monitor float; it opened side by side — "
            "which is exactly what a working shortcut looks like"
        )
        # AND THE ORDINARY TERMINAL DOES NOT FLOAT. Without this the test would
        # pass just as well with a rule matching every window, which is the
        # mistake `LEADER RET /` would pay for.
        assert not open_now["br.ronoctua.terminal"]["floating"], (
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
                f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree "
                "| grep br.ronoctua.rescue > /dev/null",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u ronoctua-rescue --no-pager")[1])
            raise
        with_rescue = windows_by_app_id(sock)
        assert with_rescue["br.ronoctua.rescue"]["floating"], (
            "the generated rule did not make the rescue float; it opened side by side"
        )
        # AND IT IS STILL THERE. A console that draws once and dies is
        # indistinguishable from one that works, in a screenshot.
        machine.sleep(5)
        assert "br.ronoctua.rescue" in windows_by_app_id(sock), (
            "the rescue's window went away: the program inside it died.\n"
            + machine.execute("journalctl -u ronoctua-rescue --no-pager")[1]
        )

        # AND THE PROGRAM STAYS UP. When the child exits the pty closes and the
        # window goes with it (see the `ping` at the end of the pty's thread), so
        # a window still there is a btop still running — which is what proves
        # the terminal gave it what a TUI needs. Without `TERM`, ncurses dies at
        # once; without a UTF-8 locale, btop does.
        machine.sleep(8)
        still = windows_by_app_id(sock)
        assert "br.ronoctua.monitor" in still, (
            "the monitor's window went away: the program inside it died.\n"
            + machine.execute("journalctl -u monitor --no-pager")[1]
        )
        journal = machine.succeed("journalctl -u monitor --no-pager")
        for bad in ["panicked", "drawing the frame failed"]:
            assert bad not in journal, f"the monitor reported: {bad}"

    with subtest("the calculator opens FLOATING, and its header's control tiles it"):
        import time

        # THE SOCKET IN ITS ENVIRONMENT: the window asks the compositor to
        # move it through the contract, and the contract finds the compositor
        # by this variable, as the session gives it to every program.
        machine.succeed(
            "systemd-run --unit=calc ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell calc"
        )
        try:
            machine.wait_until_succeeds(
                f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree "
                "| grep br.ronoctua.calc > /dev/null",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u calc --no-pager")[1])
            raise
        assert windows_by_app_id(sock)["br.ronoctua.calc"]["floating"], (
            "the generated rule did not make the calculator float; it opened in the layout"
        )
        # WHERE AND HOW BIG THE RULE OPENED IT: what the control must give it
        # back. Read once it stops moving — a floating window is placed by a
        # transaction.
        def settled_rect():
            last = None
            for _ in range(30):
                now = windows_by_app_id(sock)["br.ronoctua.calc"]["rect"]
                if now == last:
                    return now
                last = now
                time.sleep(1)
            return last

        opened = settled_rect()
        print(f"the rule opened the calculator at {opened}")

        def calc_floats(wanted, what):
            # THE COMPOSITOR'S ANSWER, asked until it says so — a move is a
            # layout's transaction, not an instant.
            for _ in range(30):
                if windows_by_app_id(sock)["br.ronoctua.calc"]["floating"] == wanted:
                    return
                time.sleep(1)
            raise AssertionError(
                f"{what}\n" + machine.execute("journalctl -u calc --no-pager")[1]
            )

        # THE KEYBOARD ON THE CALCULATOR, and its control typed: `C-f`.
        machine.succeed(
            f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[app_id=br.ronoctua.calc] focus'"
        )
        focus_compositor()
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+f")
        calc_floats(False, "the header's control did not put the calculator back in the layout")

        # AND AGAIN: it floats, at its row's size, in the middle of the room.
        machine.succeed(
            f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[app_id=br.ronoctua.calc] focus'"
        )
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+f")
        calc_floats(True, "the header's control did not float the calculator again")
        # AT THE SIZE AND PLACE THE RULE OPENED IT AT: the control and the
        # rule read one row of `floating.rs` and speak one unit. A pixel or
        # two of rounding is the compositor's, not a second size.
        again = settled_rect()
        print(f"the control floated the calculator again at {again}")
        for k in ("x", "y", "width", "height"):
            assert abs(again[k] - opened[k]) <= 2, (
                f"floated again at {again}, but the rule opened it at {opened} — "
                "the rule and the control are two sizes for one window"
            )
        journal = machine.succeed("journalctl -u calc --no-pager")
        for bad in ["panicked", "refused the calculator's place"]:
            assert bad not in journal, f"the calculator reported: {bad}"
  '';
}
