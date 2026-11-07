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
#   xdotool -> the compositor -> wl_keyboard -> the window's Key frame
#      -> the holder's encoder -> PTY -> process -> PTY -> the holder's engine
#      -> Bytes frame -> the window's mirror -> render state -> cells -> wgpu
#      -> pixel -> OCR
#
# A unit test covers the ends (the key's frame, a frame made cells) and
# cannot cover the middle: calloop's channel and ping, the keyboard focus, the
# conversion of size into cells, a grid drawn on a real surface. If the
# Enter's `\r` became `\n`, everything would compile, every unit test would
# pass, and the line would never be accepted.
#
# EACH TERMINAL IS HELD IN A SCOPE OF ITS OWN, as in a session: root's user
# manager is up (`start_user_manager`), so the window's `systemd-run --user
# --scope` starts a holder (`ronoctua-term-<id>.scope`) and the window
# attaches to it — and closing the window, or killing it, leaves the
# terminal running. Without a manager the window would fall back to the
# holder's core in its own process (`terminal_link.rs`), and the first
# subtest says which one it got.
#
# AND WHAT A HOLDER MUST SURVIVE, after the floating windows are gone: a
# window closed, a window killed, the other terminal's holder killed — each
# read from outside the window (the holder's scope, its state file, a
# heartbeat its program writes) and the screen read only for what only the
# screen says (the earlier output a reopened window shows).
#
# THE PROGRAM IN THOSE TERMINALS IS DRIVEN FROM OUTSIDE (`held` below): it
# runs the lines a test writes into its FIFO, writes what it ran, and beats
# once a second — so "its program runs on" is a file that grows, not a
# screen somebody reads.
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

  # A PROGRAM A TEST DRIVES FROM OUTSIDE, named by the terminal it runs in
  # (`RONOCTUA_TERMINAL`, which only a held terminal exports). Under
  # `/tmp/held/`: `started` (one id per program, in the order they began),
  # `<id>.path` (its PATH, as it was given), `<id>.in` (a FIFO: each line
  # written there is run), `<id>.ran` (each line it ran, after running it)
  # and `<id>.beat` (a line a second, while it runs). The FIFO is held open
  # read-write, so a writer never waits for a reader and `read -t` beats on
  # with nothing to read. Every program by its store path: the PATH it gets
  # is the one under test.
  #
  # A READ THAT TIMES OUT KEEPS WHAT IT HAD READ: when the beat's second ends
  # just after a line's first byte came, bash leaves that byte in `line` and
  # answers a timeout, and the rest arrives on the next read — `echo kept
  # before` ran as `cho kept before`. The part is kept and joined to the rest.
  held = pkgs.writeShellScript "held-program" ''
    set -u
    name=''${RONOCTUA_TERMINAL:-here}
    dir=/tmp/held
    ${pkgs.coreutils}/bin/mkdir -p "$dir"
    printf '%s' "$PATH" > "$dir/$name.path"
    : > "$dir/$name.ran"
    ${pkgs.coreutils}/bin/rm -f "$dir/$name.in"
    ${pkgs.coreutils}/bin/mkfifo "$dir/$name.in"
    exec 3<>"$dir/$name.in"
    echo "$name" >> "$dir/started"
    part=""
    while :; do
      ${pkgs.coreutils}/bin/date +%s%N >> "$dir/$name.beat"
      if IFS= read -r -t 1 line <&3; then
        line="$part$line"
        part=""
        eval "$line"
        printf '%s\n' "$line" >> "$dir/$name.ran"
      else
        part="$part$line"
      fi
    done
  '';
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
    from datetime import timedelta

    ${xvfb.prelude}


    # WHERE A HELD TERMINAL IS SEEN FROM OUTSIDE: its holder's state file
    # (`ronoctua-terminals`' registry), the held program's files, and the
    # user manager its scope is a unit of.
    REGISTRY = "/run/user/0/ronoctua-terms"
    HELD = "/tmp/held"
    USER = "XDG_RUNTIME_DIR=/run/user/0 systemctl --user"
    # THE SESSION'S PATH, as the windows below are given it: a folder no
    # manager's PATH has first, so a program that got any other PATH — the
    # user manager's, the system's — reads as different.
    SESSION_PATH = "/tmp/session-path/bin:/run/current-system/sw/bin"


    def open_terminal(unit, words):
        """`ronoctua-shell terminal <words>` in a unit of its own, as a session would start it."""
        machine.succeed(
            f"systemd-run --unit={unit} "
            "${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            f"--setenv=PATH={SESSION_PATH} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell terminal " + words
        )


    def main_pid(unit):
        return int(machine.succeed(f"systemctl show -p MainPID --value {unit}").strip())


    def opened_id(unit):
        """The id of the terminal a window's journal says it opened — once it says so: the
        holder writes its viewer before the window hears the attach answered and logs it."""
        import re

        machine.wait_until_succeeds(
            f"journalctl -u {unit} --no-pager | grep 'terminal opened' > /dev/null",
            timeout=timedelta(minutes=1),
        )
        said = machine.succeed(f"journalctl -u {unit} --no-pager")
        found = re.findall(r"terminal opened .*?\bid=([0-9a-f]{16})", said)
        assert found, f"{unit} opened no terminal:\n{said[-3000:]}"
        return found[-1]


    def started(count):
        """The ids of the held programs, in the order they began — once there are `count`."""
        machine.wait_until_succeeds(
            f"test $(wc -l < {HELD}/started) -ge {count}", timeout=timedelta(minutes=1)
        )
        return machine.succeed(f"cat {HELD}/started").split()


    def state(terminal):
        import json

        return json.loads(machine.succeed(f"cat {REGISTRY}/{terminal}.json"))


    def wait_viewer(terminal, pid):
        """Until the holder's state says which window shows it: `pid`, or none for `None`.
        Its whole line as serde writes it, so a pid is never read as the start of another."""
        said = "null" if pid is None else str(pid)
        machine.wait_until_succeeds(
            f"grep -qF '\"viewer\":{said},' {REGISTRY}/{terminal}.json",
            timeout=timedelta(minutes=1),
        )


    def scope_of(terminal):
        return f"ronoctua-term-{terminal}.scope"


    def held_alive(terminal):
        return machine.execute(f"{USER} is-active {scope_of(terminal)}")[0] == 0


    def beats_go_on(terminal):
        """Its program still runs: two heartbeats more than now."""
        beats = f"{HELD}/{terminal}.beat"
        now = int(machine.succeed(f"wc -l < {beats}").strip())
        machine.wait_until_succeeds(
            f"test $(wc -l < {beats}) -ge {now + 2}", timeout=timedelta(seconds=30)
        )


    def tell(terminal, line):
        """`line`, run by the program in `terminal` — and waited for: it writes what it ran."""
        import shlex

        ran = f"{HELD}/{terminal}.ran"
        before = int(machine.succeed(f"wc -l < {ran}").strip())
        # A TIMEOUT, because a FIFO nobody reads any more takes no writer:
        # the open would wait for ever on a program that died.
        machine.succeed(
            "timeout 10 bash -c "
            + shlex.quote(f"printf '%s\\n' {shlex.quote(line)} > {HELD}/{terminal}.in")
        )
        machine.wait_until_succeeds(
            f"test $(wc -l < {ran}) -gt {before}", timeout=timedelta(seconds=30)
        )


    def terminal_windows():
        """Every terminal window in the compositor's tree: its pid and its rectangle."""
        import json

        found = []

        def walk(node):
            if node.get("pid") and node.get("app_id") == "br.ronoctua.terminal":
                found.append({"pid": node["pid"], "rect": node["rect"]})
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                walk(kid)

        walk(json.loads(machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree")))
        return found


    def wait_window(pid):
        """Until the compositor lists a terminal window of `pid`: a holder may take its
        window's attach before the window is mapped."""
        for _ in range(60):
            if pid in [w["pid"] for w in terminal_windows()]:
                return
            machine.sleep(duration=timedelta(seconds=1))
        raise AssertionError(f"the compositor never listed a terminal window of {pid}")


    def close_window(pid):
        """The window of `pid` closed as a person closes one: the compositor asks it to."""
        machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[pid={pid}] kill'")


    def holder_pid(terminal):
        """The holder's own process, among its scope's: the one that runs the holder's program."""
        group = machine.succeed(f"{USER} show -p ControlGroup --value {scope_of(terminal)}").strip()
        assert group, f"{scope_of(terminal)} has no control group"
        for pid in machine.succeed(f"cat /sys/fs/cgroup{group}/cgroup.procs").split():
            if machine.execute(f"readlink /proc/{pid}/exe")[1].strip().endswith("/ronoctua-term-holder"):
                return int(pid)
        raise AssertionError(f"no holder among the processes of {scope_of(terminal)}")


    def window_says(pid):
        """What the terminal window of `pid` shows, by OCR — its own rectangle, asked of the
        compositor, for the reason `read_the_screen` below gives."""
        boxes = [w["rect"] for w in terminal_windows() if w["pid"] == pid]
        assert boxes, f"there is no terminal window of {pid}"
        r = boxes[0]
        _, _, x0, y0 = compositor_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd "
            f"-crop {r['width']}x{r['height']}+{x0 + r['x']}+{y0 + r['y']} +repage "
            "-colorspace gray -negate -resize 200% /tmp/held.png"
        )
        return machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/held.png stdout --psm 6 2>/dev/null"
        ).strip()


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_user_manager()
    start_xvfb()
    start_compositor("compositor", "${scroll}", config="${configFloating}")
    sock = socket_of("compositor")

    with subtest("the terminal comes up on the product's engine"):
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
                timeout=timedelta(minutes=1),
            )
        except Exception:
            print(machine.execute("journalctl -u term --no-pager")[1])
            raise
        journal = machine.succeed("journalctl -u term --no-pager")
        # The engine's name is said once the terminal is REACHED — its holder
        # started and attached to. If it had not come up, there would be no
        # name here at all.
        #
        # WITH QUOTES: `tracing` quotes a `Debug` value, and `engine=ghostty`
        # without them does not match. It is the trap already recorded in the AGENTS.md.
        assert 'engine="ghostty"' in journal, journal
        # AND HELD IN A SCOPE OF ITS OWN, not the window's process: the user
        # manager gave it one, which every subtest after the floating ones
        # stands on. The fallback says why it fell back, in this journal.
        assert "in_process=false" in journal, journal

    with subtest("the COMPOSITOR sees a window, not a region of ours"):
        # Invariant 4: a terminal drawn inside our surface would not show up in
        # the compositor's IPC.
        await_windows(sock, 1, timeout=30)

    with subtest("the terminal drew"):
        machine.wait_until_succeeds(
            "journalctl -u term --no-pager | grep 'terminal drawn on the xdg_toplevel' > /dev/null",
            timeout=timedelta(minutes=1),
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
            timeout=timedelta(minutes=1),
        )
        open_now = windows_by_app_id(sock)
        print(json.dumps(open_now, indent=1))
        assert open_now["br.ronoctua.monitor"]["floating"], (
            "the generated rule did not make the monitor float; it opened side by side — "
            "which is exactly what a working shortcut looks like"
        )
        # AND THE ORDINARY TERMINAL DOES NOT FLOAT. Without this the test would
        # pass just as well with a rule matching every window, which is the
        # mistake `LEADER RET RET` would pay for.
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
                timeout=timedelta(minutes=1),
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
        machine.sleep(duration=timedelta(seconds=5))
        assert "br.ronoctua.rescue" in windows_by_app_id(sock), (
            "the rescue's window went away: the program inside it died.\n"
            + machine.execute("journalctl -u ronoctua-rescue --no-pager")[1]
        )

        # AND THE PROGRAM STAYS UP. When the child exits the holder says so
        # (`Exited`) and the window goes with it, so
        # a window still there is a btop still running — which is what proves
        # the terminal gave it what a TUI needs. Without `TERM`, ncurses dies at
        # once; without a UTF-8 locale, btop does.
        machine.sleep(duration=timedelta(seconds=8))
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
                timeout=timedelta(minutes=1),
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

        # THE KEYBOARD ON THE CALCULATOR, and its control run as `LEADER Alt`
        # runs it (`calc-float`; `C-f` typed there runs nothing).
        machine.succeed(
            f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[app_id=br.ronoctua.calc] focus'"
        )
        calc = windows_by_app_id(sock)["br.ronoctua.calc"]["pid"]
        shortcut(calc, "calc-float")
        calc_floats(False, "the header's control did not put the calculator back in the layout")

        # AND AGAIN: it floats, at its row's size, in the middle of the room.
        machine.succeed(
            f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[app_id=br.ronoctua.calc] focus'"
        )
        shortcut(calc, "calc-float")
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

    with subtest("the window closes when its last program exits"):
        # PROVES the end of a held terminal: the `cat` above reads the end of
        # its input (`C-d`, typed into its window), ends, and its holder says
        # so; the window lets its last terminal go and quits, and the holder
        # goes with it — its scope and its state file. A window left up over
        # a program that ended, or a holder that outlived the window it told,
        # fails here and nowhere above.
        cat = opened_id("term")
        window = main_pid("term")
        assert held_alive(cat), f"the cat's holder, {scope_of(cat)}, is not running"
        machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[pid={window}] focus'")
        focus_compositor()
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+d")
        try:
            machine.wait_until_fails("systemctl is-active term", timeout=timedelta(minutes=1))
        except Exception:
            print(machine.execute("journalctl -u term --no-pager")[1])
            raise
        for _ in range(30):
            if window not in [w["pid"] for w in terminal_windows()]:
                break
            machine.sleep(duration=timedelta(seconds=1))
        else:
            raise AssertionError("the window's process ended and the compositor still lists its window")
        machine.wait_until_fails(f"{USER} is-active {scope_of(cat)}", timeout=timedelta(seconds=30))
        machine.wait_until_fails(f"test -e {REGISTRY}/{cat}.json", timeout=timedelta(seconds=30))

        # THE ROOM, for the held terminals below: the floating windows go, so
        # each window there is alone on the screen it is read off.
        machine.succeed("systemctl stop monitor ronoctua-rescue calc")
        await_windows(sock, 0, timeout=60)
        machine.succeed(f"mkdir -p /tmp/session-path/bin {HELD}")

    with subtest("closing a window keeps its holder, and the program in it runs on"):
        # PROVES what a held terminal is: the window closed as a person closes
        # one, and its terminal is still there — its scope active in the user
        # manager, its state file saying no window shows it, its program
        # still beating. A window that ended its program (the in-process
        # fallback, a holder taken down with the window) stops the beat.
        open_terminal("alpha", "-- ${held}")
        alpha = started(1)[0]
        window = main_pid("alpha")
        wait_viewer(alpha, window)
        wait_window(window)
        # SAID WHILE THE WINDOW SHOWED IT, for the reopening below.
        tell(alpha, "echo kept before")
        close_window(window)
        try:
            machine.wait_until_fails("systemctl is-active alpha", timeout=timedelta(minutes=1))
        except Exception:
            print(machine.execute("journalctl -u alpha --no-pager")[1])
            raise
        wait_viewer(alpha, None)
        assert held_alive(alpha), f"{scope_of(alpha)} went with its window"
        beats_go_on(alpha)

    with subtest("a held program's PATH is the session's, and not the user manager's"):
        # PROVES the holder carries the window's environment: a scope runs in
        # the environment of whoever asks for it, and a holder that was a
        # SERVICE of the user manager would hand its program the manager's
        # PATH instead — every program installed in the person's profile
        # missing, in a terminal that otherwise works. The second assertion
        # is what makes the first answerable: the two PATHs differ here.
        got = machine.succeed(f"cat {HELD}/{alpha}.path")
        assert got == SESSION_PATH, f"the program got PATH={got!r}, and the session's is {SESSION_PATH!r}"
        manager = [
            line.split("=", 1)[1]
            for line in machine.succeed(f"{USER} show-environment").splitlines()
            if line.startswith("PATH=")
        ]
        assert manager != [SESSION_PATH], (
            f"the user manager's PATH is the session's ({manager}), so this cannot tell them apart"
        )

    with subtest("reopened with --show, a terminal shows what it said before, and while away"):
        # PROVES the holder kept the screen and went on feeding it with no
        # window: a line said while a window showed it, a line said while
        # none did, and a new window — `terminal --show <id>`, attached to the
        # running holder — draws both. Read off the screen, because that is
        # where a snapshot that lost its history, or a holder that stopped
        # reading its program while away, shows.
        tell(alpha, "echo heard meanwhile")
        open_terminal("again", f"--show {alpha}")
        again = main_pid("again")
        wait_viewer(alpha, again)
        wait_window(again)
        assert opened_id("again") == alpha, "--show opened another terminal than the one it named"
        read_back = ""
        for _ in range(30):
            read_back = window_says(again).lower()
            print(f"OCR: {read_back!r}")
            if "before" in read_back and "meanwhile" in read_back:
                break
            machine.sleep(duration=timedelta(seconds=1))
        assert "before" in read_back and "meanwhile" in read_back, (
            f"the reopened window does not show what the terminal said; OCR read {read_back!r}"
        )

    with subtest("killing a window leaves its holders running, and away"):
        # PROVES a window that dies without a word — `kill -9`, no close, no
        # detach — takes none of its terminals with it: each holder hears its
        # window's socket end, says no window shows it, and its program goes
        # on. Two terminals in ONE window (`--show a,b`), so a holder that
        # only noticed its own window's goodbye fails on both.
        open_terminal("beta", "-- ${held}")
        beta = started(2)[1]
        wait_viewer(beta, main_pid("beta"))
        wait_window(main_pid("beta"))
        close_window(main_pid("beta"))
        close_window(again)
        wait_viewer(alpha, None)
        wait_viewer(beta, None)
        await_windows(sock, 0, timeout=60)
        open_terminal("both", f"--show {alpha},{beta}")
        both = main_pid("both")
        wait_viewer(alpha, both)
        wait_viewer(beta, both)
        # ONE WINDOW holds the two.
        await_windows(sock, 1, timeout=60)
        assert [w["pid"] for w in terminal_windows()] == [both], terminal_windows()
        machine.succeed(f"kill -9 {both}")
        wait_viewer(alpha, None)
        wait_viewer(beta, None)
        for terminal in (alpha, beta):
            assert held_alive(terminal), f"{scope_of(terminal)} died with the window that was killed"
            beats_go_on(terminal)

    with subtest("killing one holder leaves the other terminal running, in its window"):
        # PROVES a holder is one terminal's and no more: `kill -9` of the
        # first's holder, and the second — in the same window — is still
        # held, still shown by that window, still beating; the window lets
        # the dead one go and stays. A holder that was the whole group's, or
        # a window that closed when one of its terminals vanished, fails it.
        open_terminal("pair", f"--show {alpha},{beta}")
        pair = main_pid("pair")
        wait_viewer(alpha, pair)
        wait_viewer(beta, pair)
        wait_window(pair)
        holder = holder_pid(alpha)
        machine.succeed(f"kill -9 {holder}")
        machine.wait_until_fails(f"test -d /proc/{holder}", timeout=timedelta(seconds=30))
        # `> /dev/null` AND NOT `-q`: a grep that quits at its first match
        # leaves journalctl writing into a closed pipe, and under `pipefail`
        # the line fails because it was found.
        machine.wait_until_succeeds(
            "journalctl -u pair --no-pager | grep \"a terminal's holder went away\" > /dev/null",
            timeout=timedelta(minutes=1),
        )
        assert held_alive(beta), f"{scope_of(beta)} died with the other terminal's holder"
        assert state(beta)["viewer"] == pair, state(beta)
        assert pair in [w["pid"] for w in terminal_windows()], (
            "the window closed when one of its two terminals went away"
        )
        beats_go_on(beta)
  '';
}
