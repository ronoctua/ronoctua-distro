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
  # THE HEIGHT OF N LINES, rounded the way the shell rounds it.
  #
  # ONE ceiling over the whole product, and not a sum of ceilings: the line is
  # 26,1 px, so two lines are `ceil(52,2) = 53` and three are `ceil(78,3) = 79`
  # — while `53 + ceil(26,1)` is 80. The test asked for 80, the footer drew 79,
  # and the failure looked like a missing line.
  heightOf = lines: builtins.ceil (typography.size.base * typography.lineHeight * lines);
  footerPx = heightOf 2;

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
      # The history is JSON on disk, and the last subtest counts it.
      pkgs.jq
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
    # THE FOOTER WITH A BAND of one and of two lines. The band is one line for a
    # brief notification and up to three for a full one, and the height is what
    # the test can see from outside — see `heightOf` for why these are not sums.
    WITH_ONE = ${toString (heightOf 3)}
    WITH_TWO = ${toString (heightOf 4)}


    def footer_height():
        """The surface's real height on screen, in px."""
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        raw = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
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

    def wait_for_height(expected, attempts=20):
        """Wait for the surface to reach the given height, and report the value.

        Two lessons in one helper: `assert x == y` with no message hides the
        value that actually came, and measuring right after the paint log runs
        ahead of the COMPOSITOR — the footer has drawn, niri has not composed yet.
        """
        seen_now = None
        for _ in range(attempts):
            seen_now = footer_height()
            if seen_now == expected:
                return seen_now
            machine.sleep(1)
        raise AssertionError(
            f"the footer is {seen_now} px, expected {expected} "
            f"(after {attempts}s)"
        )



    def layers():
        out_text = machine.succeed(f"${ronoctua-shell}/bin/niri-contract {sock}")
        for line in out_text.splitlines():
            if line.startswith("layers:"):
                return line
        raise AssertionError(f"niri-contract did not list layers:\n{out_text}")


    def notify(summary, urgency="normal", body=""):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            f"${pkgs.libnotify}/bin/notify-send -u {urgency} "
            f"-a testes '{summary}' '{body}'"
        )
        machine.sleep(1)


    def control(command):
        # `command <id>`, and not one word per feature.
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

    with subtest("the footer comes up with the notification daemon on the bus"):
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
            # THE HISTORY IS WRITTEN UNDER `$XDG_STATE_HOME`, and a systemd
            # scope inherits neither that nor `HOME` — so without this the
            # store would silently write nowhere and the last subtest would be
            # measuring an absence.
            "--setenv=HOME=/root "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_notify=debug "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'the notification daemon is on the bus'",
            timeout=60,
        )
        # THE PAINTING, and not only the daemon: measuring the height before the
        # first `configure` gives zero pixels, with no error at all.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'footer layout resolved'",
            timeout=90,
        )
        wait_for_height(FOOTER)

    with subtest("three notifications, ONE on screen"):
        for i in (1, 2, 3):
            notify(f"message {i}")
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert journal.count("notification received") == 3, journal

        # The queue counts the ones that are WAITING: 2, not 3. The one on the
        # band is not pending.
        #
        # The grep is in the QUEUE's log, not in the segment's: the segment's
        # text starts with a PUA glyph, and `body= 2` does not match
        # `body=<glyph>2`.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'pending=2'", timeout=30
        )

    with subtest("the band appears ABOVE the modeline, on the SAME surface"):
        # THE BAND LASTS SECONDS, so it is raised HERE and measured at once.
        # Measuring after another subtest is measuring an empty band: three
        # normal notifications are eighteen seconds of screen, and the checks in
        # between spend more than that.
        #
        # The queue is drained first, so what is on the band is what this
        # subtest put there.
        for _ in range(6):
            control("notif-dismiss")
        wait_for_height(FOOTER)

        notify("one discreet line")
        # A brief notification is ONE line above the pair. The log says which,
        # and the height is what proves it reached the screen.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'band=1'", timeout=30
        )
        wait_for_height(WITH_ONE, attempts=5)

        # THE CLAUSE ONLY THE VM PROVES. The owner asked for a line above the
        # modeline and not a floating toast; what makes that legal is that it is
        # the footer's own surface growing upwards. One surface, still.
        c = layers()
        print(c)
        assert c.count("ronoctua-footer") == 1, (
            f"the band created a SECOND surface (invariant 14):\n{c}"
        )

    with subtest("the critical one goes away on its own, and stays unread"):
        # THE RULE THIS REPLACED said a critical one never expires. The band
        # pushes the content up while it is there, so one that never leaves is a
        # permanent tax on every window — the owner's decision, and this is
        # where it is proved rather than asserted.
        notify("battery at 3%", urgency="critical", body="change sockets")
        # Loud, and TWO lines: the summary and the body. `Show::Full` is up to
        # three, and the third is the row of actions — this one has none, and a
        # band with an empty line is a band lying about its size.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'band=2 loud=true'", timeout=30
        )
        wait_for_height(WITH_TWO, attempts=5)
        # `long` is 20 s. Well past it, the band is gone and the pair is back.
        wait_for_height(FOOTER, attempts=40)
        # AND IT IS STILL UNREAD. Leaving the screen and being seen are not the
        # same event, which is the whole point of the count surviving the band.
        #
        # The LAST line the queue logged is the one that counts: the ones before
        # it are the states it passed through.
        last = machine.succeed(
            "journalctl -u footer --no-pager | grep 'notification queue' | tail -1"
        ).strip()
        print(last)
        assert "unread=" in last, last
        assert "unread=0 " not in last, "the critical one left the screen AND the counter"

    with subtest("a tag from the bus lands in the catalogue"):
        # THE OTHER DOOR into the catalogue: an app of ours that is not the
        # shell names itself with a hint, and the entry recognises it. Without
        # this it would have to be recognised by its summary — an English
        # string, in a system that translates.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "${pkgs.libnotify}/bin/notify-send -a testes "
            "--hint=string:tags:memory-killed 'something closed' 'for lack of memory'"
        )
        machine.sleep(2)
        decided = machine.succeed(
            "journalctl -u footer --no-pager | grep 'notification decided' | tail -1"
        ).strip()
        print(decided)
        # THE QUOTES ARE PART OF IT: `tracing` writes a string field as
        # `catalogue="memory-killed"`, and matching without them passes on
        # nothing and fails on everything.
        assert 'catalogue="memory-killed"' in decided, decided
        # And the entry's verdict reached it: three lines, and long.
        assert 'life="long"' in decided, decided
        assert 'show="full"' in decided, decided
        for _ in range(4):
            control("notif-dismiss")

    with subtest("the file's path arrives by BOTH conventions"):
        # The compositor puts a screenshot's `file://` URL in the icon field;
        # libnotify puts it in the standard `image-path` hint and leaves the
        # icon empty. Reading only the first made every action that needs a file
        # vanish for every libnotify client — a band two lines tall where three
        # were expected, and nothing failing.
        for _ in range(4):
            control("notif-dismiss")
        machine.succeed("touch /root/an-image.png")
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "${pkgs.libnotify}/bin/notify-send -a testes "
            "--hint=string:image-path:file:///root/an-image.png "
            "'with an image' 'through the hint'"
        )
        machine.sleep(1)
        got = machine.succeed(
            "journalctl -u footer --no-pager | grep 'notification received' | tail -1"
        ).strip()
        print(got)
        assert 'icon="file:///root/an-image.png"' in got, got

    with subtest("the manager opens BELOW the minibuffer, on the SAME surface"):
        control("notif-history")
        expanded = footer_height()
        assert expanded > FOOTER, f"the manager did not expand: {expanded} px"

        c = layers()
        print(c)
        assert c.count("ronoctua-footer") == 1, (
            f"the manager created a SECOND surface (invariant 14):\n{c}"
        )

        # And the content is there, on the screen, below the footer's two lines.
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop {w}x{expanded - FOOTER}+0+{h - expanded + FOOTER} +repage "
            "-colorspace gray -negate -resize 400% /tmp/hist.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/hist.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR of the manager: {read_back!r}")
        assert "message" in read_back.lower() or "battery" in read_back.lower(), (
            f"the manager is not on screen; OCR read {read_back!r}"
        )

    with subtest("closing the manager gives the two lines back"):
        control("notif-history")
        wait_for_height(FOOTER)

    with subtest("the history survives the footer restarting"):
        # FIFTEEN DAYS ON DISK, and this is the half a VM can prove: the file is
        # written where the footer says it is, and a fresh process reads it.
        machine.wait_until_succeeds(
            "test -s /root/.local/state/ronoctua/notifications.json", timeout=30
        )
        before = machine.succeed(
            "${pkgs.jq}/bin/jq 'length' /root/.local/state/ronoctua/notifications.json"
        ).strip()
        assert int(before) >= 4, f"the history holds {before} records"
        machine.succeed("systemctl restart footer")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'notification history read'",
            timeout=60,
        )
        after = machine.succeed(
            "journalctl -u footer --no-pager | grep 'notification history read' | tail -1"
        )
        print(after)
        assert f"read={before}" in after, after
  '';
}
