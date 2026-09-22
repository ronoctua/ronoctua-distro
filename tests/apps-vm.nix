# The app finder, proved ON SCREEN and on the clock.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# Three things, and the last one is the reason this file exists:
#
#   1. that the sweep happens AT STARTUP, and not on pressing the key. By unit
#      test one can prove that it works; that it has already happened when the
#      list opens depends on the real event loop;
#   2. that the application's name reaches the PIXEL — catalogue -> expansion
#      -> span -> screen. The whole chain, closed by OCR;
#   3. that the footer GOES ON RESPONDING after opening an application.
#
# The third is a defect that existed before this feature and that the feature
# would make central: `systemd-run --scope` is SYNCHRONOUS — from the manual,
# "execution in this case is synchronous, and will return only when the
# command finishes". With the `calloop` on a single thread, waiting for it
# froze keyboard, painting and the compositor events for the application's whole
# lifetime.
#
# The test application sleeps TWO MINUTES on purpose. With the defect, the
# footer goes mute for those two minutes and this check blows the `timeout`;
# without it, the next key is served in milliseconds. There is no way for the
# check to pass by accident.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  # A NAME WITH NO ACCENT AND NO OCR AMBIGUITY. Tesseract confuses `l`/`I` and
  # eats accents in a small font; what is proved here is that the name reaches
  # the pixel, and not the quality of the OCR.
  item_name = "Dozing";
  # TWO MINUTES. See the header: it is what separates "it did not freeze" from
  # "the test waited long enough".
  sleepSecs = 120;
  # THE PATTERN WITH A BRACKET, and not a raw `sleep ${toString sleepSecs}`.
  #
  # The `pgrep -f` matches the command line of EVERY process — including that of
  # the shell running the `pgrep` itself, whose line contains the pattern.
  # Without the bracket, `pgrep -f 'sleep 120'` finds itself and gives back
  # success even with the application dead: the assertion "it is still alive"
  # was EMPTY, and an empty check is worse than none. The `[s]` matches `s` and
  # does not match `[s]`.
  sleepPattern = "[s]leep ${toString sleepSecs}";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-apps";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
      pkgs.coreutils
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    def leader():
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )


    def leaders_in_log():
        return int(
            machine.succeed(
                "journalctl -u footer --no-pager | grep -c 'leader active' || true"
            ).strip()
            or 0
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the test `.desktop` exists BEFORE the footer comes up"):
        # The sweep is at startup: a file written afterwards would not go into
        # this round, and the test would start measuring the re-sweep instead of
        # the main path.
        machine.succeed("mkdir -p /tmp/apps/applications")
        machine.succeed(
            "cat > /tmp/apps/applications/dozing.desktop <<'EOF'\n"
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=${item_name}\n"
            "Comment=Stands still for ${toString sleepSecs} seconds\n"
            "Exec=${pkgs.coreutils}/bin/sleep ${toString sleepSecs}\n"
            "Categories=Utility;\n"
            "[Desktop Action other]\n"
            "Name=ShouldNotShowUp\n"
            "Exec=/bin/false\n"
            "EOF"
        )
        # And one that must NOT show up: `NoDisplay` is what takes 38% of the list out.
        machine.succeed(
            "cat > /tmp/apps/applications/escondido.desktop <<'EOF'\n"
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=ShouldNotShowUp\n"
            "Exec=/bin/false\n"
            "NoDisplay=true\n"
            "EOF"
        )

    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("the footer comes up and SCANS at start-up"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=XDG_DATA_DIRS=/tmp/apps "
            "--setenv=XDG_DATA_HOME=/tmp/empty "
            f"--setenv=SCROLLSOCK={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'control socket opened' > /dev/null",
            timeout=90,
        )
        # THE SWEEP IS THE STARTUP ONE, and the log carries the count and the
        # time. If it only happened on opening, this line would not exist yet.
        journal = machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'application catalogue scanned'",
            timeout=60,
        )
        print(journal.strip())
        assert "apps=1" in journal, (
            "the scan should find ONE application — the `NoDisplay` one and the actions\n"
            f"of the desktop entry cannot get in. Log:\n{journal}"
        )
        focus_compositor()

    with subtest("`LEADER a` opens with the list ALREADY READY"):
        leader()
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'leader active' > /dev/null", timeout=30
        )
        press_key("a")
        journal = machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'finder opened'", timeout=30
        )
        print(journal.strip())
        assert "apps=1" in journal, (
            f"the finder opened with an empty catalogue; log:\n{journal}"
        )

    with subtest("the application's name is ON SCREEN"):
        # It closes the chain: catalogue -> expansion -> span -> pixel. A test
        # that only looked at the log would not tell "it found" from "it draws".
        w, h, x, y = compositor_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")

        # THE FOOTER'S BAND, and not the whole output.
        #
        # The finder is drawn in the footer, which is anchored at the BOTTOM —
        # so a crop of the whole screen is mostly empty desk, and
        # `tesseract --psm 6`, which expects one uniform block of text, reads
        # nothing at all from it. It used to work because the window filled
        # differently; what makes it not depend on that again is counting from
        # the bottom edge, which is where the footer always is.
        #
        # THE HEIGHT THE FOOTER SAYS IT RESOLVED, which is two fixed lines
        # plus whatever the open expansion took. A guessed band is mostly desk,
        # and an image that is mostly desk reads as an empty string.
        band = min(h, footer_band())
        top = max(0, h - band)
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop {w}x{band}+0+{top} +repage "
            "-colorspace gray -negate -resize 200% /tmp/apps.png"
        )
        # DIAGNOSIS BEFORE THE OCR, the shape `modeline-vm` uses: `cores=1`
        # says "uniform band, nothing drawn" and separates a wrong crop from an
        # absent drawing without costing a second VM round.
        print("the footer's band: " + machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/apps.png "
            "-format 'cores=%k media=%[fx:mean]' info:"
        ))
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/apps.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        assert "dozing" in read_back.lower(), (
            f"the application's name is not on screen; OCR read {read_back!r}"
        )
        assert "shouldnotshowup" not in read_back.lower().replace(" ", ""), (
            f"the desktop entry's action or the `NoDisplay` leaked into the list; OCR: {read_back!r}"
        )

    with subtest("typing FILTERS, and Enter opens the application"):
        # `doz` matches by subsequence inside the name, which is the search the
        # finder offers.
        for c in "doz":
            press_key(c)
        prior = leaders_in_log()
        press_key("Return")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'opening the application' > /dev/null", timeout=30
        )
        machine.wait_until_succeeds(
            "pgrep -f '${sleepPattern}' >/dev/null", timeout=30
        )

    with subtest("AND THE FOOTER GOES ON ANSWERING — the finding"):
        # HERE IS THE CHECK. With the synchronous `.status()` in the event loop,
        # the footer would go mute for the `sleep`'s ${toString sleepSecs} seconds, and
        # this block would blow the 20 s timeout without ever seeing the second leader.
        #
        # The application stays ALIVE while this runs: it is that simultaneity
        # that the defect made impossible.
        leader()
        machine.wait_until_succeeds(
            f"test $(journalctl -u footer --no-pager | grep -c 'leader active') -gt {prior}",
            timeout=20,
        )
        machine.succeed("pgrep -f '${sleepPattern}' >/dev/null")
        print("the footer answered with the application still open")

    with subtest("what we start, we stop"):
        machine.succeed("pkill -f '${sleepPattern}' || true")
  '';
}
