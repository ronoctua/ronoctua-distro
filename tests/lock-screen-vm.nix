# VM test: the lock screen (`ronoctua-shell lock`, `crates/ronoctua-shell-lock`),
# over the compositor's own lock (`ext-session-lock-v1`).
#
# WHAT ONLY A COMPOSITOR PROVES. The screen's rules — a wrong password said and
# refused, a right one or a finger opening, the finger offered only where a
# reader is — are unit-tested in the crate. What no unit test reaches is the
# LOCK: that the command, through the footer, takes the compositor's lock; that
# while it is held a window no longer gets a key (a key-logging client, `wev`,
# hears nothing); that the account's password, typed on the screen, goes
# through PAM (`ronoctua-unlock`, the module the machine imports) and opens
# the session again — after a wrong one was refused with its sentence on the
# screen — and the window hears keys again; and that the automatic lock the
# person turns on (`lock.json`) is armed by the compositor's idle
# notifications and locks by itself after the minute chosen.
#
# THE ACCOUNT IS ROOT, as every process of the compositor's tests is: the lock
# checks the password of the account that runs it, and root's is set here. The
# finger cannot be had in a VM (no reader), and the screen then says nothing
# of one — which the unit test of the screen holds.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-lock-screen";

  nodes.machine = {
    # THE TWO PAM SERVICES the lock asks, as the distro declares them: the
    # password's, and the finger's where fprintd is on (it is not here).
    imports = [ ../nix/modules/unlock.nix ];

    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.wev
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
    from datetime import timedelta

    ${xvfb.prelude}

    RIGHT = "right-horse"
    WRONG = "wrong-horse"
    RUN = "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "


    def journal(unit):
        return machine.succeed(f"journalctl -u {unit} --no-pager")


    def keys_heard():
        """How many keys the window `wev` was given — its own log, one
        `sym:` line per key event."""
        return journal("wev").count("sym: ")


    def typed(text):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 80 '{text}'")


    def pressed(key):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {key}")


    def lock_running():
        # THE LOCK IS THE FOOTER'S CHILD here (no user unit in this VM): its
        # own program by its path (`current_exe`), the wrapped binary — so it
        # is counted by its word, not by the wrapper's name.
        return machine.execute("pgrep -f '[s]hell-wrapped lock$'")[0] == 0


    def bottom_of_the_screen():
        """What OCR reads off the lowest quarter of the compositor's window,
        where the lock's two lines and its sentence are drawn."""
        w, h, x, y = compositor_geometry()
        band = h // 4
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop {w}x{band}+0+{h - band} +repage "
            "-colorspace gray -negate -resize 200% /tmp/lock.png"
        )
        return machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/lock.png stdout --psm 6 2>/dev/null"
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")
    # THE ACCOUNT'S PASSWORD, the one the lock asks.
    machine.succeed(f"echo 'root:{RIGHT}' | chpasswd")
    machine.succeed("test -e /etc/pam.d/ronoctua-unlock")
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("a window hears the keys before the lock"):
        # LINE-BUFFERED: `wev` writes with stdio, and into the journal's pipe
        # stdio holds a whole block back.
        machine.succeed(
            "systemd-run --unit=wev ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.coreutils}/bin/stdbuf -oL ${pkgs.wev}/bin/wev -f wl_keyboard:key"
        )
        await_windows(sock, 1)
        focus_compositor()
        typed("ab")
        machine.wait_until_succeeds(
            "journalctl -u wev --no-pager | grep -q \"utf8: 'b'\"", timeout=timedelta(minutes=1)
        )

    with subtest("the footer comes up"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 --setenv=HOME=/root --setenv=USER=root "
            "--setenv=LANG=en_GB.UTF-8 "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "--setenv=RUST_LOG=info "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'control socket opened'", timeout=timedelta(seconds=90)
        )

    with subtest("the command locks the session, through the footer"):
        machine.succeed(RUN + "${ronoctua-shell}/bin/ronoctua-shell command lock-screen")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'the session is locked'", timeout=timedelta(minutes=1)
        )
        assert lock_running(), "the lock is not running:\n" + journal("footer")[-3000:]
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'the password is asked on this screen'",
            timeout=timedelta(seconds=30),
        )

    with subtest("while it is locked, the window hears nothing"):
        before = keys_heard()
        focus_compositor()
        typed("zzz")
        machine.sleep(duration=timedelta(seconds=3))
        assert keys_heard() == before, (
            "a key reached the window under the lock:\n" + journal("wev")[-2000:]
        )
        # AND `ESC` lets what was typed go, and never the lock.
        pressed("Escape")
        machine.sleep(duration=timedelta(seconds=1))
        assert lock_running(), "Esc opened the lock"

    with subtest("a wrong password is refused, and the screen says so"):
        typed(WRONG)
        pressed("Return")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'the password was refused' "
            "| grep -q wrong",
            timeout=timedelta(minutes=1),
        )
        read_back = ""
        for _ in range(15):
            read_back = bottom_of_the_screen()
            print(f"OCR: {read_back!r}")
            if "wrong password" in read_back.lower():
                break
            machine.sleep(duration=timedelta(seconds=1))
        assert "wrong password" in read_back.lower(), (
            f"the refusal is not on the screen; OCR read {read_back!r}"
        )
        assert lock_running(), "a wrong password opened the lock"
        assert keys_heard() == before, "a key reached the window under the lock"
        # THE PASSWORD NEVER REACHES THE LOG.
        assert WRONG not in journal("footer"), "the typed password is in the log"

    with subtest("the account's password opens the session, and the window hears again"):
        typed(RIGHT)
        pressed("Return")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'the password opened the session'",
            timeout=timedelta(minutes=1),
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'the session is open again'", timeout=timedelta(seconds=30)
        )
        machine.wait_until_fails("pgrep -f '[s]hell-wrapped lock$'", timeout=timedelta(seconds=30))
        assert RIGHT not in journal("footer"), "the typed password is in the log"
        # NOT ABANDONED: a lock that went with the session still locked would
        # keep every key from the window.
        focus_compositor()
        typed("q")
        machine.wait_until_succeeds(
            "journalctl -u wev --no-pager | grep -q \"utf8: 'q'\"", timeout=timedelta(seconds=30)
        )

    with subtest("by itself, after the minute chosen, with nobody at the machine"):
        machine.succeed(
            "install -d /root/.config/ronoctua"
            " && printf '%s' '{\"automatic\":true,\"minutes\":1}' > /root/.config/ronoctua/lock.json"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager "
            "| grep -q 'the screen locks by itself after a time without use'",
            timeout=timedelta(seconds=30),
        )
        # NOTHING TOUCHED for the minute: the compositor counts it.
        machine.wait_until_succeeds(
            "test $(journalctl -u footer --no-pager | grep -c 'the session is locked') -ge 2",
            timeout=timedelta(minutes=3),
        )
        assert "idle for the time chosen" in journal("footer"), journal("footer")[-3000:]
        focus_compositor()
        typed(RIGHT)
        pressed("Return")
        machine.wait_until_fails("pgrep -f '[s]hell-wrapped lock$'", timeout=timedelta(minutes=1))
  '';
}
