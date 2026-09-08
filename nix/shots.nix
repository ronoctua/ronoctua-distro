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
  # The solved palettes and the theme list: the driver reads the default
  # theme's values when it measures a screenshot.
  themed,
  themes,
  ronoctua-shell,
  ronoctua-brain,
  fonts,
}:

let
  xvfb = import ../tests/niri-xvfb.nix { inherit pkgs lib; };
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = linePx * 2;

  # THE TOKENS' VALUES, AS THE GENERATOR SOLVED THEM.
  #
  # Read by the test driver rather than typed into it, and that is what makes
  # the measurements below worth taking: they tie the photograph to the
  # palette. A colour hard-coded in Rust, or a role-to-token map that lost an
  # arm, fails in this build instead of on the owner's screen — and changing a
  # value moves the assertion with it rather than breaking it (invariant 9,
  # reaching a screenshot).
  #
  # The DEFAULT theme's flat map, a store path the driver opens when it runs:
  # nothing here is read at evaluation (no import-from-derivation). The driver's
  # `token_hex` turns `0xRRGGBBAA` into the `RRGGBB` ImageMagick wants; the
  # alpha is always opaque where it is used.
  palette = "${themed}/${builtins.head themes}.json";
  # AND THE SECOND THEME'S, for the chooser's shots: a preview has to show a
  # colour the default does not have, and the assertion reads it from the
  # generator rather than from a number typed here.
  # With one theme declared the chooser's shots have nothing to prove and the
  # driver skips them (`THEMES > 1`); the path still has to exist.
  second = if builtins.length themes > 1 then builtins.elemAt themes 1 else builtins.head themes;
  secondPalette = "${themed}/${second}.json";
  # How many answers the desk option has, counted from the default language's
  # texts: one `theme-backdrop-<id>` key per answer, which the chooser's own
  # test demands of every answer. It sizes the chooser's shots without a
  # second copy of the number.
  deskAnswers = lib.count (line: lib.hasPrefix "theme-backdrop-" line) (
    lib.splitString "\n" (
      builtins.readFile (../assets/i18n + "/${builtins.head (import ../assets/i18n/languages.nix)}.ftl")
    )
  );

  # THE FILE THE PREVIEW IS PHOTOGRAPHED SHOWING.
  #
  # A `.nix` because it is what this repository is made of, because the type is
  # one we invent ourselves in `mime-glyphs.nix` — so the shot proves that path
  # too — and because a dozen lines of it reach five of the twelve syntax
  # tokens: a comment, a string, a number, a keyword and a property.
  #
  # It is a store file rather than a heredoc in the test script for the reason
  # written at the copy below.
  sampleCode = pkgs.writeText "sample.nix" (
    builtins.concatStringsSep "\n" [
      # THE FIRST THREE LINES CARRY EVERY COLOUR THIS SHOT MEASURES, and that
      # is not decoration: the panel shows as many lines as its band has, and
      # the band gives one up to the sideways bar. A fixture that put the
      # string on line four measured the layout as it was before that bar
      # existed — and the build failed the day it appeared, which is the check
      # doing its job.
      "# a comment, which is the one colour that recedes"
      "let name = \"ronoctua\"; size = 42; in"
      # AND A LINE WIDER THAN ANY PANEL, because the bar along the bottom is
      # the half the owner found missing and a shot without one cannot show it.
      "{ description = \"a line long enough that no panel in this system can hold it, which is what the bar along the foot of the preview is for\";"
      "  enable = true;"
      "  packages = with pkgs; [ git ripgrep fd ];"
      "  imports = [ ./base.nix ./desktop.nix ];"
      "  settings = { theme = \"tokyo-night\"; lines = 12; };"
      "}"
      ""
    ]
  );
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-shots";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      ronoctua-brain
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
    import json as _json

    # The default theme's palette, `key -> 0xRRGGBBAA` (see `palette` above),
    # and the second theme's, for the chooser.
    TOKENS = _json.load(open("${palette}"))
    SECOND = _json.load(open("${secondPalette}"))
    THEMES = ${toString (builtins.length themes)}
    DESK_ANSWERS = ${toString deskAnswers}


    def token_hex(key, table=None):
        """`RRGGBB` of a token, for ImageMagick. The alpha is dropped."""
        return (table or TOKENS)[key][2:8]

    ${xvfb.prelude}

    FOOTER = ${toString footerPx}
    LINE = ${toString linePx}
    STRIP = ${toString (import ../assets/tokens/space.nix).files.stripLines}
    AIR = ${toString (import ../assets/tokens/space.nix).files.pictureAir}


    def notify(summary, body="", level="normal", app="tests"):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            f"${pkgs.libnotify}/bin/notify-send -u {level} "
            f"-a {app} '{summary}' '{body}'"
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
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        if lines is None:
            crop = f"{w}x{h}+{x}+{y}"
        else:
            tall = LINE * lines
            crop = f"{w}x{tall}+{x}+{y + h - tall}"
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {crop} +repage "
            f"/tmp/{name}.png"
        )
        machine.copy_from_vm(f"/tmp/{name}.png", "")
        print(f"took {name}")


    def crop(box, into="/tmp/measured.png"):
        """Cuts `box` out of the last screenshot, for measuring.

        THE DESTINATION IS A PARAMETER, and it is not decoration: two
        measurements sharing one file is one measurement quietly overwriting
        the other's evidence. It happened here — the sideways bar's crop landed
        on the panel's, and the comparison below then held two pictures of
        different sizes, which `compare` refuses and `|| true` swallowed. The
        assertion passed on an error message.
        """
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {box} +repage {into}"
        )


    def has_colour(hex_value, at_least=1, of="/tmp/panel-before.png"):
        """How many pixels of `of` are that colour.

        A LOOSE MATCH on purpose: the surface is drawn through wgpu and the
        text through a shaper, so a glyph's interior is the token's colour and
        its edges are blends of it. Twelve per cent is wide enough to catch the
        body of a letter and narrow enough that green does not answer for
        purple — the two furthest apart in this palette are `syntax.string`
        and `syntax.keyword`, and they differ by far more than that.
        """
        n = int(
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick {of} -fuzz 12% "
                f"-fill '#000000' +opaque '{hex_value}' -fill '#FFFFFF' "
                f"-opaque '{hex_value}' -format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )
        assert n >= at_least, (
            f"{hex_value} covers {n} pixel(s) of the panel, and {at_least} were owed"
        )
        return n


    def lacks_colour(hex_value, of):
        """The other half of `has_colour`: not one pixel of `of` is that colour,
        at the same twelve per cent. What proves a theme LEFT the screen."""
        n = int(
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick {of} -fuzz 12% "
                f"-fill '#000000' +opaque '{hex_value}' -fill '#FFFFFF' "
                f"-opaque '{hex_value}' -format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )
        assert n == 0, f"{hex_value} still covers {n} pixel(s) of {of}"


    def rows_with_ink(box):
        """How many rows of pixels in `box` have anything drawn on them."""
        crop(box, "/tmp/ink.png")
        out = machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/ink.png -resize 1x! "
            "-threshold 6% -format '%[fx:int(mean*h+0.5)]' info:"
        )
        return int(out.strip())


    machine.start()
    machine.wait_for_unit("multi-user.target")
    # THE ONE TEST THAT NEEDS THE BRAIN TO SPEAK: it photographs its line.
    start_xvfb(brain=True)
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
        # The theme's floor and the generator's fragments, as the module
        # exports them — the chooser's shots copy a fragment and assert it.
        "--setenv=RONOCTUA_THEME=${builtins.head themes} "
        "--setenv=RONOCTUA_THEMES=${themed} "
        "${ronoctua-shell}/bin/ronoctua-shell"
    )
    machine.wait_until_succeeds(
        "journalctl -u footer --no-pager | grep -q 'footer layout resolved'",
        timeout=90,
    )

    with subtest("the footer at rest — two lines, and no bell"):
        shot("1-footer-at-rest", lines=2)

    with subtest("the discreet band, one line tall"):
        notify("copied to the clipboard")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'band=1'", timeout=30
        )
        shot("2-discreet-band", lines=3)

    with subtest("the queue, with the counter"):
        for i in (1, 2, 3, 4):
            notify(f"message {i}")
        shot("3-queue-with-counter", lines=3)

    with subtest("the loud band, three lines tall"):
        # A critical one is level 90, so the distro's `important-gets-room`
        # catches it: full, loud, and long enough to photograph.
        notify(
            "battery at 3%",
            body="the machine shuts down in a few minutes; plug it in",
            level="critical",
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'loud=true'", timeout=30
        )
        shot("4-loud-band", lines=4)

    with subtest("the screenshot: the font's own glyph, and the actions"):
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
        machine.succeed("mkdir -p /root/Pictures")
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick -size 320x200 canvas:gray30 "
            "'/root/Pictures/Screenshot 2026-08-22.png'"
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
            "'niri' 'uint32 0' 'file:///root/Pictures/Screenshot%202026-08-22.png' "
            "'Screenshot captured' 'You can paste the image from the clipboard.' "
            "'@as []' \"{'urgency': <byte 1>}\" 'int32 -1'"
        )
        machine.sleep(1)
        decided = machine.succeed(
            "journalctl -u footer --no-pager | grep 'notification decided' | tail -1"
        ).strip()
        print(decided)
        assert 'catalogue="screenshot"' in decided, decided
        shot("5-screenshot-with-actions", lines=5)

    with subtest("the manager, in two columns"):
        control("notif-open")
        shot("6-manager-two-columns")

    with subtest("the distro's catalogue, in the third column"):
        # X INPUT FOCUS FIRST. With no window manager nobody calls
        # `XSetInputFocus`, so the focus stays on `PointerRoot` and every key
        # `xdotool` injects is lost — no error, no log, nowhere.
        focus_niri()
        # TAB twice: what happened, what was suppressed, what is configured.
        for _ in (1, 2):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Tab")
            machine.sleep(1)
        shot("7-distro-catalogue")
        # And one entry's sheet, which is where "use the distro's default or
        # customise" is actually done.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(1)
        shot("8-adjustment-sheet")
        control("notif-open")

    with subtest("the file manager, with the preview and the facts"):
        # A PICTURE MADE IN THE VM, and nothing brought in. The owner's own
        # test files live outside this repository and stay there; what a shot
        # has to prove is that a picture reaches the panel, and a picture made
        # here proves it without carrying somebody's photographs into a store
        # path that never forgets.
        #
        # A PNG on purpose: the raster path is the `image` crate, in process,
        # so this shot needs none of the five external programs and works on a
        # machine that has no media group at all.
        machine.succeed("mkdir -p /root/shots")
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick -size 480x320 "
            # TWO LIGHT COLOURS on purpose: a gradient that fades into the
            # background makes the picture's own edge unmeasurable, and the
            # air below it then reads as forty pixels that are not there.
            "gradient:'#7AA2F7'-'#E0AF68' /root/shots/test-picture.png"
        )
        focus_niri()
        control("files-open")
        # THE LETTER IS QUERY in this view, which is what makes this
        # deterministic: typing narrows to the one file rather than depending on
        # where the cursor happened to land.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'shot'"
        )
        machine.sleep(1)
        # AND INTO THE DIRECTORY. Filtering alone leaves the cursor on
        # `shots/` — which is a folder, and a folder has no picture inside
        # it, so the panel correctly drew the type's glyph. The file is one
        # level down.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(1)
        # AND NOTHING IS PRESSED FOR THE PANELS. They used to need `C-v` and
        # `C-i`; the owner asked for them to be on from the start, so pressing
        # the two keys here is what would TURN THEM OFF — which is how this
        # shot came back with no picture in it at all.
        machine.sleep(1)
        # THE POINTER ON A ROW, because the band only exists under it — and the
        # band's left edge is the thing the owner keeps photographing. A shot
        # with the pointer parked outside cannot show it at all.
        w_g, h_g, x_g, y_g = niri_geometry()
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x_g + 300} {y_g + h_g - 3 * LINE}"
        )
        machine.sleep(1)
        shot("10-manager-with-preview")

        # AND THE POINTER OVER THE FACTS, which is the defect the owner
        # reported in words: hovering a fact lit the row of the LISTING beside
        # it, because every section's fields ride one `Row`. What has to be on
        # screen here is a band inside the facts' own column and NOTHING lit in
        # the listing.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x_g + w_g - 120} {y_g + h_g - 4 * LINE}"
        )
        machine.sleep(1)
        shot("14-hover-on-the-facts")

        # AND A LISTING THAT IS ACTUALLY CUT, which no shot here had.
        #
        # Every manager shot above opens on a folder with one file, so no
        # section scrolls and no bar is drawn — which is why the owner's
        # photograph of TWO BARS in one column could not be reproduced from
        # this file. The footer used to draw its own generic bar over the
        # manager's per-section one: two rectangles in the same cell, moving
        # against different totals. Forty files make the listing scroll, and
        # the picture then has exactly one bar in it or the defect is back.
        machine.succeed("mkdir -p /root/many && touch /root/many/f-{01..40}.txt")
        for _ in range(8):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Escape")
        machine.sleep(1)
        control("files-open")
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'many'"
        )
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(2)
        shot("16-footer-listing-cut")
        # `files-open` OPENS — it does not toggle, so asking twice reopened it.
        # And what the manager leaves up is a PAGE, which the footer draws BELOW
        # the band and below the modeline: cropping the last lines with it on
        # screen photographs the listing. That is exactly how the brain's line
        # came out as a file listing and no one noticed — green is not proof,
        # and neither is a screenshot nobody opened.
        for _ in range(8):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Escape")
        # TWO DIGITS IS THE ASSERTION, and not a number of my own: the footer
        # sums a fractional line height and comes to 53 where `LINE` — which
        # ceils, because a crop cannot take half a pixel — would say 54. What
        # is being asserted is that the PAGE is gone, and chrome alone is tens
        # of pixels tall where a page is hundreds.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | tail -6"
            " | grep -qE 'height=[0-9]{1,2} '",
            timeout=30,
        )

    with subtest("the manager at the WINDOW door, which the footer does not show"):
        # THE DOOR NOBODY PHOTOGRAPHED. Every shot here is of the footer, and
        # the manager has two doors — so the window's own spacing went three
        # rounds unseen while the owner was looking straight at it. A door with
        # no picture of it is a door whose defects are reported from his screen
        # instead of caught here.
        control("files-window")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | tail -40"
            " | grep -q 'br.ronoctua.files'",
            timeout=60,
        )
        machine.sleep(3)
        # AND INTO THE PICTURE'S FOLDER, so this door is photographed with a
        # picture in it: the window opens at HOME, where every entry is a
        # directory and the preview correctly says there is none.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'shots'"
        )
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(2)
        # SAME REASON as the footer's: the band needs the pointer.
        w_w, h_w, x_w, y_w = niri_geometry()
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x_w + 300} {y_w + 4 * LINE}"
        )
        machine.sleep(1)
        shot("13-manager-in-a-window")

        # AND THE POINTER ON THE FACTS, in THIS door — the one the owner
        # photographed. Down in the strip the facts have a `top` of their own,
        # and a band that forgot it lit the listing's row of the same number.
        #
        # THE WINDOW'S OWN HALF, and not the output's: niri tiles this window
        # into the left half of the screen, so `w_w` — which is the whole
        # output — put the pointer on the desktop and nothing was hovered.
        # The facts sit to the right of the strip's rule; the third of them is
        # three lines below the strip's top.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x_w + 450} {y_w + h_w - 3 * LINE - 6}"
        )
        machine.sleep(1)
        shot("15-hover-on-the-windows-facts")

        # AND THE FACTS SCROLLED TO THE END, which is the photograph the owner
        # sent twice.
        #
        # No shot here ever reached the last row of a panel, so the gap under
        # the thumb could not be seen from this file: the track was
        # `lines × line_px` and a surface is not a whole number of lines, so at
        # the last offset the thumb stopped short by the residue and the
        # background showed through below it. The wheel takes it to the end;
        # what has to be true in the picture is that the thumb's bottom and the
        # window's bottom are the same pixel.
        for _ in range(12):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool click 5")
        machine.sleep(2)
        shot("17-facts-scrolled-to-the-end")

        # AND THE LISTING SCROLLED TO ITS END, in the SAME door and with the
        # strip below it — the other half of the same arithmetic.
        #
        # Giving the facts the surface's bottom was right: theirs is the last
        # band in its column. Giving it to EVERY band was not, and the owner
        # watched the listing's thumb run past the boundary and down into the
        # facts' box. What has to be true in this picture is the opposite of
        # the one above: the thumb's bottom is the strip's rule, not the
        # window's bottom edge.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key BackSpace")
        machine.sleep(1)
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'many'"
        )
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(2)
        # THE POINTER OVER THE LISTING and not over a panel: the wheel moves
        # the section it is on, which is the whole point of the four sections.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x_w + 200} {y_w + 6 * LINE}"
        )
        machine.sleep(1)
        for _ in range(60):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool click 5")
        machine.sleep(2)
        shot("18-window-listing-scrolled-to-the-end")

        # AND A FILE OF LETTERS, WITH ITS OWN CONTENT IN THE PANEL.
        #
        # THE DEFECT THIS IS THE PICTURE OF: the owner selected a markdown
        # file and the preview said "there is no picture inside this kind of
        # file". The panel is not an image panel — it answers what is IN a
        # file — and every one of the eighteen shots above photographed it
        # with a picture or with nothing, so the sentence was on his screen and
        # in none of ours.
        #
        # A `.nix` on purpose: it is the file this repository is made of, the
        # type is one we invent ourselves in `mime-glyphs.nix`, and its
        # highlight uses four of the twelve tokens in one screenful.
        machine.succeed("mkdir -p /root/shots/code")
        # FROM THE STORE, and not from a heredoc written here.
        #
        # A heredoc would be a shell string inside a Python string inside this
        # Nix fence, and AGENTS.md names that trap in as many words: inside the
        # fence a Nix interpolation is live even in a comment, a backtick is
        # command substitution, and the fence itself closes on two apostrophes
        # wherever they appear. Writing this very comment closed it once. A
        # file the store already holds has none of those levels.
        machine.succeed("cp ${sampleCode} /root/shots/code/sample.nix")
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key BackSpace")
        machine.sleep(1)
        control("files-window")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | tail -40"
            " | grep -q 'br.ronoctua.files'",
            timeout=60,
        )
        machine.sleep(3)
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'shots'"
        )
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(1)
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'code'"
        )
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        machine.sleep(2)
        shot("19-preview-of-a-file-of-text")

        # AND IT IS MEASURED, because a picture nobody reads is not proof.
        #
        # The panels sit in the right-hand half of the window from
        # `space.files.panelsFrom` on, and the preview is the left of the two.
        w_t, h_t, x_t, y_t = niri_geometry()
        # SAFELY INSIDE THE PREVIEW, and narrower than the panel it sits in.
        #
        # A quarter of the width was the first box, and the two panels are not
        # exactly a quarter each: it reached past the rule and into the facts,
        # so a measurement of "the preview" was reading its neighbour's text.
        # The panels begin at the halfway mark; this stops well short of the
        # rule between them, which costs a few columns of reach and buys an
        # answer about the right panel.
        panel = f"{w_t // 4 - 70}x{h_t - 4 * LINE}+{w_t // 2 + 10}+{2 * LINE}"

        # ONE: THERE IS CONTENT, and not a sentence. A refused panel draws one
        # dim line; planting `Refusal::NoPicture` back brings this to 1.
        drawn = rows_with_ink(panel)
        assert drawn >= 3, (
            f"the preview drew ink on {drawn} row(s) of pixels — a file of "
            "letters came out as a sentence again"
        )

        # TWO: THE HIGHLIGHT IS REAL, AND IT IS THE TOKENS'.
        #
        # This is the sharpest measurement available here, because it ties the
        # photograph to `assets/tokens/colors.nix`: a colour written into Rust
        # by hand, or a role-to-token map that lost an arm, fails HERE and not
        # on the owner's screen. The two asked for are the two furthest apart
        # in the palette — a string's green and a keyword's purple.
        crop(panel, "/tmp/panel-before.png")
        has_colour("#" + token_hex("syntax.string"), at_least=20)
        has_colour("#" + token_hex("syntax.keyword"), at_least=20)

        # AND IT SCROLLS. `C-,` pages the preview and the bar follows.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+comma")
        machine.sleep(2)
        shot("20-preview-of-text-paged")

        # AND THE SIDEWAYS BAR IS THERE, which is the half the owner found
        # missing: the keys moved the line and there was nothing to drag.
        #
        # WHERE IT IS, IS MEASURED AND NOT ASSUMED. The first version computed
        # the foot from the WINDOW's height and cropped the modeline instead —
        # which has ink of its own, so the assertion passed while pointing at
        # the wrong thing entirely. The panel's last row of ink is a fact on
        # the screen; ask for it.
        def ink_bottom(box, at_y):
            """The absolute y where the ink inside `box` ends."""
            crop(box, "/tmp/col.png")
            got = machine.succeed(
                "${pkgs.imagemagick}/bin/magick /tmp/col.png -threshold 20% -trim "
                "-format '%[fx:page.y+h]' info: 2>/dev/null || echo 0"
            ).strip()
            return at_y + int(got)

        bottom = ink_bottom(panel, 2 * LINE)
        foot = f"{w_t // 4 - 70}x{LINE}+{w_t // 2 + 10}+{bottom - LINE}"
        assert rows_with_ink(foot) >= 1, (
            "there is nothing along the foot of the preview — a line one can "
            "only reach with the keyboard is half a view"
        )

        # AND THE SIDEWAYS BAR MOVES, which is the whole of what the owner
        # asked for: a thumb that is only there to look at is not a control.
        #
        # Measured as its own left edge before and after the key. `C-]` moves
        # the line, and the thumb has to follow it — where a bar is drawn is
        # where it is grabbed, and a thumb that stood still while the text
        # moved would be lying about where one is.
        # DID THE THUMB LEAVE THE LEFT EDGE? — asked of a four-pixel probe
        # there, counting the pixels drawn in the bar's own token.
        #
        # NOT `-trim`, and the reason is the case that matters most: trim
        # answers from the CORNER pixel, and at column nought the corner IS the
        # thumb — so it reported nothing exactly when the thumb was where it
        # should be. The first version read that "nothing" as a position of
        # minus one, and then `after > before` passed because 3 is more than
        # -1 rather than because anything moved.
        #
        # This count cannot do that. With no bar at all it is nought at both
        # ends and the assertion fails; with a bar that does not move it is the
        # same at both ends and the assertion fails.
        def thumb_over_the_left(box):
            probe = f"4x{LINE}+{box.split('+')[1]}+{box.split('+')[2]}"
            return int(
                machine.succeed(
                    f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {probe} +repage "
                    f"-fuzz 6% -fill white -opaque '#{token_hex('rule.strong')}' -fill black "
                    "+opaque white -format '%[fx:int(mean*w*h+0.5)]' info:"
                ).strip()
            )

        before_left = thumb_over_the_left(foot)
        assert before_left > 0, (
            "the foot of the preview has no bar on it at all — a line one can "
            "only reach with the keyboard is half a view"
        )
        for _ in range(6):
            machine.succeed(
                "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+bracketright"
            )
        machine.sleep(2)
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        after_left = thumb_over_the_left(foot)
        assert after_left < before_left, (
            f"the sideways thumb covered {before_left} pixels of the left edge "
            f"and still covers {after_left} — the line moved and the bar did "
            "not follow it"
        )
        shot("21-preview-scrolled-sideways")

        # AND IT IS MEASURED TOO: the panel's own pixels changed. Restoring
        # `total: 1` leaves the key with nothing to move and this fails.
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop "
            f"{panel} +repage /tmp/after.png"
        )
        # BY SIGNATURE, and not by `compare`.
        #
        # `compare`'s EXIT CODE is its answer — nought when the pictures match,
        # one when they differ — so every attempt to keep the command from
        # "failing" also threw away the result. The first version wrapped it in
        # `|| true` and read an error message as "they differ"; the second
        # measured two crops of different sizes and got away with it.
        #
        # A signature is one number per picture. Two of them are equal or they
        # are not, and a magick that fell over gives two empty strings — which
        # compare EQUAL, so the assertion below fails rather than passes.
        def signature(path):
            return machine.succeed(
                f"${pkgs.imagemagick}/bin/magick {path} -format '%#' info:"
            ).strip()

        assert signature("/tmp/panel-before.png") != signature("/tmp/after.png"), (
            "the preview drew the same pixels after the page key — it did not "
            "scroll"
        )

    with subtest("the miniplayer: it moves, it holds, and it draws over nothing"):
        # A GIF MADE HERE, and not a video, and that is not a compromise.
        #
        # The GIF path is the `image` crate, decoded in process — so this
        # subtest needs none of the five external programs and runs on a machine
        # with no media group at all. What it proves is the whole player: a
        # frame reaching the panel, the box being sized from the FRAME rather
        # than from a still, the transport taking a line of its own, and the
        # hold being real.
        #
        # What it cannot prove is the TIMELINE, which needs a duration, which
        # needs a video, which needs `ffmpeg-full` inside this VM. That is a
        # closure decision and not a test decision, so it is not taken here.
        #
        # FOUR COLOURS THIS INTERFACE DOES NOT CONTAIN, and that is measured
        # rather than assumed. The first version used four palette colours and
        # the blue one matched the interface elsewhere within the fuzz: the
        # picture's own box came back as 172x644 — most of the panel — while
        # the orange, which is not near anything, came back as 130x97 for
        # exactly 12610 pixels. A measurement that catches its neighbours is
        # not a measurement.
        FILM = ["#FF00FF", "#00FFFF", "#FFFF00", "#FF6000"]
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick -delay 20 -loop 0 -size 240x180 "
            + " ".join(f"xc:'{c}'" for c in FILM)
            + " /root/shots/moving.gif"
        )
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key BackSpace")
        machine.sleep(1)
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 40 'moving'"
        )
        machine.sleep(3)
        shot("22-miniplayer")

        w_m, h_m, x_m, y_m = niri_geometry()
        # THE SAME BOX THE TEXT PREVIEW USES, and for the reason written there:
        # a quarter of the width reaches past the rule into the facts.
        pane = f"{w_m // 4 - 70}x{h_m - 4 * LINE}+{w_m // 2 + 10}+{2 * LINE}"

        def panel_now(name):
            """Crops the preview out of a fresh capture."""
            machine.succeed(
                "${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd"
            )
            machine.succeed(
                "${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop "
                f"{pane} +repage /tmp/{name}.png"
            )
            return f"/tmp/{name}.png"

        def film_pixels(path, colour):
            """How many pixels of one of the film's colours the panel holds."""
            return int(
                machine.succeed(
                    f"${pkgs.imagemagick}/bin/magick {path} -fuzz 4% "
                    f"-fill white -opaque '{colour}' -fill black +opaque white "
                    "-format '%[fx:int(mean*w*h+0.5)]' info:"
                ).strip()
            )

        def frame_showing(path):
            """Which of the film's frames is on screen, or None."""
            counts = [film_pixels(path, c) for c in FILM]
            best = max(range(len(FILM)), key=lambda i: counts[i])
            # A THRESHOLD, because "none of them" has to be answerable: the
            # picture is thousands of pixels, and a stray match is tens.
            return (best, counts) if counts[best] > 500 else (None, counts)

        # ZERO: THE PLAYER OPENED, AND AT THE SIZE THE PANEL ASKED FOR.
        #
        # This one needs no pixels, and it is the sharpest of the four: under
        # the defect the owner photographed, this line said `512x32`.
        opened = machine.succeed(
            "journalctl --no-pager | grep -o 'motion stream opened.*' | tail -1 || true"
        ).strip()
        assert opened, (
            "the player never opened a stream at all — the file under the "
            "cursor is not being read as one that moves"
        )
        assert "height=32" not in opened, (
            f"the frame's height is being guessed again: {opened}"
        )

        # ONE: DIFFERENT FRAMES OF THE FILM REACH THE SCREEN.
        #
        # Asked as "which frame is up", sampled six times, and NOT as "did two
        # signatures differ" — which is what this was first and which is flaky
        # by construction: two samples can land on the same frame, and the
        # answer then reads exactly like a film that is frozen. It did, once,
        # with the player provably running.
        #
        # Naming the frame also tells apart the two things a frozen signature
        # cannot: a film that is stuck, and frames that never arrive at all.
        seen = []
        for k in range(6):
            which, counts = frame_showing(panel_now(f"moving-{k}"))
            assert which is not None, (
                f"not one of the film's four colours is in the panel: {counts} "
                "— the frames are not reaching the screen at all"
            )
            seen.append(which)
            machine.sleep(1)
        assert len(set(seen)) >= 2, (
            f"the panel showed frame {seen} over five seconds — the film is not "
            "running"
        )

        # TWO: THE HOLD IS REAL, and this is the sharper half. A stopped film is
        # DETERMINISTIC — the same frame every time — and it proves the
        # transport rather than the decoder.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+semicolon"
        )
        machine.sleep(2)
        held = []
        for k in range(3):
            which, _ = frame_showing(panel_now(f"held-{k}"))
            held.append(which)
            machine.sleep(1)
        assert len(set(held)) == 1 and held[0] is not None, (
            f"the panel showed frames {held} after the hold key — pausing did "
            "not stop the player"
        )
        shot("23-miniplayer-held")

        # THREE: THE SHAPE IS RIGHT, and this one assertion catches what the
        # owner photographed.
        #
        # Measured as the BOX OF THE FILM'S OWN COLOUR, which is the picture and
        # nothing else. A plain threshold was the first attempt and it answered
        # 250x658 — the whole panel, text included — so it passed whatever the
        # picture looked like.
        #
        # The GIF is 240x180, four to three. Under the old defect the player
        # declared its frames 512 by 32 and the doors stretched that inside a
        # box measured from the STILL, so the picture came out a band of roughly
        # sixteen to one.
        box = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/held-0.png -fuzz 4% "
            f"-fill white -opaque '{FILM[held[0]]}' -fill black +opaque white "
            "-trim -format '%[fx:w] %[fx:h]' info:"
        ).split()
        ink_w, ink_h = int(box[0]), int(box[1])
        assert ink_w > 0 and ink_h > 0, "the picture has no extent at all"
        assert ink_w < ink_h * 2, (
            f"the picture is {ink_w} by {ink_h} — a four-to-three film came out "
            "as a band, which is the frame's height being guessed again"
        )

        # FOUR: THE CONTROLS ARRIVE WITH THE POINTER, AND NOT BEFORE.
        #
        # The owner asked for exactly this — *"controles que aparecem no hover
        # do mouse"* — and it is the only claim here that has to be written as a
        # negative as well as a positive. Without the first half, a row that was
        # always drawn would pass the second.
        #
        # The transport is the panel's LAST line, which is where the picture
        # stops: `View::transport_rows` takes it off the box, so this row is
        # never ink drawn over the film.
        # THE BOTTOM OF THE PANEL, which is where the controls go — counted in
        # the colour a GLYPH is drawn in and not by a threshold. Hovering used
        # to light the section's own band, and a band is ink: a threshold would
        # have risen for the hover rather than for the control, and passed
        # either way. (The band over a picture is gone now — see
        # `View::band_belongs` — but the measurement stays honest by colour.)
        foot = f"{w_m // 4 - 70}x{2 * LINE}+{w_m // 2 + 10}+{2 * LINE + (h_m - 4 * LINE) - 2 * LINE}"

        def glyph_pixels(box):
            return int(
                machine.succeed(
                    f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {box} "
                    f"+repage -fuzz 12% -fill white -opaque '#{token_hex('text.primary')}' "
                    "-fill black +opaque white "
                    "-format '%[fx:int(mean*w*h+0.5)]' info:"
                ).strip()
            )

        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        cold = glyph_pixels(foot)
        assert cold == 0, (
            f"the transport row has {cold} pixels of glyph with the pointer "
            "elsewhere — the controls are drawn whether or not anybody is "
            "reaching for them"
        )

        # AND ON THE PICTURE, found by ITS OWN COLOUR rather than computed.
        #
        # The panel's midpoint is not the film: here the preview sits in the
        # bottom STRIP, so the middle of the column is the LISTING. That is
        # where this pointer went the first time, and the controls correctly did
        # not arrive — the assertion was right and the aim was wrong.
        spot = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/held-0.png -fuzz 4% "
            f"-fill white -opaque '{FILM[held[0]]}' -fill black +opaque white "
            "-trim -format '%[fx:page.x] %[fx:page.y] %[fx:w] %[fx:h]' info:"
        ).split()
        px, py, pw, ph = (int(v) for v in spot)
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x_m + (w_m // 2 + 10) + px + pw // 2} "
            f"{y_m + (2 * LINE) + py + ph // 2}"
        )
        machine.sleep(2)
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        warm = glyph_pixels(foot)
        shot("24-miniplayer-controls")
        assert warm > 0, (
            "the pointer is on the film and the transport row has no glyph in "
            "it — the controls do not arrive"
        )

        # FIVE: THE CONTROLS ARE ON THE FILM, AND COST IT NO HEIGHT.
        #
        # The owner asked for the picture to keep *"toda a altura possível"*, so
        # the transport is a band painted over the film's last line rather than
        # a line taken off it. Measured as the picture's own box, with the
        # controls up and with them away: the same number, or the band is still
        # eating a line.
        def film_box(path, colour):
            got = machine.succeed(
                f"${pkgs.imagemagick}/bin/magick {path} -fuzz 4% "
                f"-fill white -opaque '{colour}' -fill black +opaque white "
                "-trim -format '%[fx:w] %[fx:h]' info:"
            ).split()
            return int(got[0]), int(got[1])

        machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop "
            f"{pane} +repage /tmp/with-controls.png"
        )
        held_w, held_h = film_box("/tmp/held-0.png", FILM[held[0]])

        # MEASURED AGAINST THE PANEL, and NOT against the same picture with the
        # controls up.
        #
        # That was the first shape of this and it is self-contradicting: the
        # band is half-transparent, so it DARKENS the film it covers, and the
        # film's own colour stops matching there. The picture then measures
        # shorter exactly BECAUSE the band works — and the assertion two blocks
        # down checks that the film still shows through it. One of the two had
        # to be wrong, and it was this one.
        #
        # What says the controls cost the film nothing is the film filling the
        # panel's own height: four to three in a panel twice as wide as it is
        # tall is bounded by the height, so it reaches the box's edge. A line
        # reserved for the controls would leave it a whole line short.
        panel_tall = STRIP * LINE - 2 * AIR
        assert held_h > panel_tall - LINE, (
            f"the film is {held_w} by {held_h} in a panel {panel_tall} tall — "
            "more than a line short, so the transport is still costing it room"
        )

        # AND NO HOVER BAND ON THE FILM.
        #
        # The owner reported it in those words — *"o hover do mouse está
        # destacando o background da linha no preview de vídeos e imagens, o
        # que é totalmente desnecessário"*. A band says which ROW of a list the
        # pointer is on, and a picture is not a list: it lights an arbitrary
        # stripe across the film and covers part of what one came to look at.
        #
        # MEASURED IN BOTH STATES, and that is not caution — it is the only way
        # this can be measured at all.
        #
        # `surface.selection` and `rule.default` are THE SAME VALUE, so a count
        # of that colour inside the panel includes every divider and rule that
        # is always there. Asserting it is nought is asserting something
        # impossible: the baseline is 808 pixels of rules, and it failed on them
        # while the band it was written for was already gone. What answers the
        # question is the DIFFERENCE — 808 against 4357 with the band, 877
        # without it.
        def band_pixels(path):
            return int(
                machine.succeed(
                    f"${pkgs.imagemagick}/bin/magick {path} -crop {pane} +repage "
                    f"-fuzz 2% -fill white -opaque '#{token_hex('surface.selection')}' -fill black "
                    "+opaque white -format '%[fx:int(mean*w*h+0.5)]' info:"
                ).strip()
            )

        cold_band = band_pixels("/tmp/held-0.png")
        warm_band = band_pixels("/tmp/with-controls.png")
        # A BAND IS A WHOLE LINE of the panel — thousands of pixels. Antialiasing
        # around the controls moves this by tens, so the bound is generous and
        # still an order of magnitude under what a band costs.
        assert warm_band - cold_band < 500, (
            f"the panel holds {cold_band} pixels of the band's colour with the "
            f"pointer away and {warm_band} with it on the film — a picture has "
            "no rows to light"
        )

        # AND THE BAND IS REALLY SEMI-TRANSPARENT: the film's own colour is
        # still there under the controls. Opaque, the row would be a strip of
        # chrome across the picture — which is what invariant 14 is about.
        strip = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {foot} +repage "
            f"-fuzz 18% -fill white -opaque '{FILM[held[0]]}' -fill black "
            "+opaque white -format '%[fx:int(mean*w*h+0.5)]' info:"
        ).strip()
        assert int(strip) > 0, (
            "the film does not show through the transport's band at all — it is "
            "opaque, and a strip of chrome across a picture is what invariant 14 "
            "rules out"
        )

        # SIX: THE WHOLE REGION, AND WHAT THE OWNER FOUND IN IT.
        #
        # `C-w` gives the preview the region. Three of the defects he
        # photographed only exist here, so this is where they are pinned.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+w")
        # LONG ENOUGH FOR THE RESPAWN. Giving the region away changes the box,
        # which changes the frame's shape, which is a kill and a new child —
        # 108 to 242 ms measured, and this machine is a software-rendered VM.
        # Photographed sooner, what is on screen is the frame cut for the OLD
        # box, letterboxed into the new one, which is a true picture of a
        # transient and a false picture of the layout.
        machine.sleep(4)
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        shot("25-miniplayer-whole")

        # A RULE AGAINST THE SCREEN'S OWN EDGE. With one panel filling the
        # width there is nothing to its left to be separated from, and the
        # arithmetic put the line in cell nought: *"no modo largo há uma linha
        # vertical na esquerda que continua aparecendo"*. Measured as ink in the
        # first two columns of the region.
        edge = f"2x{h_m - 4 * LINE}+{x_m}+{y_m + 2 * LINE}"
        rule_ink = int(
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {edge} +repage "
                f"-fuzz 2% -fill white -opaque '#{token_hex('rule.strong')}' -fill black "
                "+opaque white -format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )
        assert rule_ink == 0, (
            f"{rule_ink} pixels of a rule are against the screen's own edge in "
            "the wide preview"
        )

        # AND THE PICTURE IS NOT DISTORTED. The film is 240x180 — four to
        # three — and a frame stretched to a region three times as wide as it
        # is tall is what the owner photographed twice: once on the GIF and
        # once after pausing and going wide. Asked of the picture's own colour,
        # which is the picture and nothing else.
        # WHICHEVER FRAME IS UP, and not the one that was up before.
        #
        # Naming a colour was the first shape of this and it measured 17 by 27 —
        # a glyph, not a film. Giving the region away is a respawn, and the film
        # comes back at another frame; asking for the OLD colour then measures
        # whatever stray pixels match it. What is being asserted here is the
        # SHAPE of the picture, so which frame is showing is not the question.
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop "
            f"{w_m}x{h_m}+{x_m}+{y_m} +repage /tmp/whole.png"
        )
        # THE LARGEST CONNECTED RUN OF THE COLOUR, and not its bounding box.
        #
        # `-trim` answers with the box around EVERY matching pixel, and a
        # handful of antialiased ones elsewhere on the screen stretch it: the
        # film measured 356 by 451 while its own rectangle was 356 by 268 and
        # its pixel count 95 408 — a box holding sixty per cent background.
        # Connected components answer the question that was actually being
        # asked, which is "how big is the picture".
        def biggest_blob(path, colour):
            out = machine.succeed(
                f"${pkgs.imagemagick}/bin/magick {path} -fuzz 2% "
                f"-fill white -opaque '{colour}' -fill black +opaque white "
                "-define connected-components:verbose=true "
                "-define connected-components:area-threshold=2000 "
                "-connected-components 8 null: 2>/dev/null || true"
            )
            best = (0, 0, 0)
            for row in out.splitlines():
                if "srgb(255,255,255)" not in row or ":" not in row:
                    continue
                box = row.split(":")[1].split()[0]
                across, down = box.split("+")[0].split("x")
                area = int(row.split()[-2])
                if area > best[2]:
                    best = (int(across), int(down), area)
            return best

        ww, wh = 0, 0
        for colour in FILM:
            w, h, area = biggest_blob("/tmp/whole.png", colour)
            if area > ww * wh:
                ww, wh = w, h
        # A FILM, AND NOT A GLYPH. The picture is thousands of pixels; a stray
        # match is a few hundred.
        # A FILM, AND NOT A GLYPH. The picture is thousands of pixels; a stray
        # match is a few hundred. It also has to be BRIGHT — the band under the
        # controls runs to the panel's real bottom, and when the panel was four
        # lines tall in a whole region that band covered the entire film and
        # darkened it. The fuzz is tight for exactly that reason.
        assert ww * wh > 20000, (
            f"the largest thing matching the film's colours is {ww} by {wh} — "
            "the film is either not in the region or is being covered by the "
            "transport's own band"
        )
        assert abs((ww / wh) - (240 / 180)) < 0.08, (
            f"the film is {ww} by {wh} — {ww / wh:.2f} against the four to "
            "three it was authored at, so it is being stretched"
        )

        # AND `C-w` GIVES THE REGION BACK.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+w")
        machine.sleep(2)

        # AND IT GOES, or it stands in front of every shot after it. The bracket
        # keeps the pattern from matching the `pkill` that carries it —
        # `AGENTS.md` names that one.
        machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
        machine.wait_until_succeeds(
            "! ${pkgs.xwininfo}/bin/xwininfo -display :99 -root -children"
            " | grep -q ronoctua-files || true",
            timeout=30,
        )
        machine.sleep(2)

    with subtest("the brain's line, with the countdown and the two answers"):
        # THE ONE REGION NO BUFFER DRAWS. `nix run .#png` renders a wasip2
        # component's display list; this line is the footer's own surface,
        # below the minibuffer, so a real compositor is the only vehicle.
        machine.succeed(
            "systemd-run --unit=brain "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root "
            "--setenv=RUST_LOG=ronoctua_brain=debug "
            "${ronoctua-brain}/bin/ronoctua-brain"
        )
        machine.wait_until_succeeds(
            "test -S /run/user/0/ronoctua-brain.sock", timeout=30
        )

        # ASKING FOR THE REHEARSAL, and not for the real thing: the announcement
        # and its countdown are the same either way, and a rehearsal changes
        # nothing on a machine that has no update units at all.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 ${pkgs.python3}/bin/python3 -c '"
            "import socket, json, os;"
            "s = socket.socket(socket.AF_UNIX);"
            "s.connect(os.environ[\"XDG_RUNTIME_DIR\"] + \"/ronoctua-brain.sock\");"
            "s.sendall(json.dumps({\"ask\": \"run\", \"task\": \"system-update\","
            " \"rehearsal\": True}).encode() + b\"\\n\");"
            "s.shutdown(socket.SHUT_WR);"
            "print(s.recv(65536).decode()[:200])"
            "'"
        )
        # `brain=1` is the HEIGHT, in the shape `band=1` already has: waiting on
        # a sleep would be either slow or flaky, and on a loaded machine both.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | tail -12 | grep -q 'brain=1'",
            timeout=60,
        )
        shot("9-brain-line", lines=3)

    with subtest("the update's mark on the modeline, and the window it opens"):
        # NOTHING IS ASKED FOR HERE, and that is the point: the shot above left
        # a run in the queue, and the mark is on screen for exactly as long as
        # something is queued or running. It is the sentence `update_window`
        # already carries — "the queue counts, and not only what is running" —
        # photographed. The sixty seconds of the countdown are the window this
        # subtest works in, and it needs three of them.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | tail -12"
            " | grep -q 'segment id=\"update\"'",
            timeout=60,
        )
        # THREE LINES because the mark is in the MODELINE, which is the third
        # from the bottom while the band has the line under the minibuffer.
        shot("11-update-mark", lines=3)

        # AND THE WINDOW, which is a command of its own — `LEADER s w` — for
        # this reason: it opens whether or not anything is running, and what it
        # says when nothing is is "nothing has been said yet".
        control("update-watch")
        machine.sleep(3)
        shot("12-update-window")

    with subtest("the theme chooser, and the file that is the broadcast"):
        # -----------------------------------------------------------------
        # Three things are proved on screen, and none of them by a buffer
        # (`nix run .#png` cannot reach a footer view). One: the list opens
        # on the theme in force, its focused row in that theme's accent. Two:
        # moving the cursor PREVIEWS — the second theme's accent is on the
        # footer and the first's is gone — while the file manager's window,
        # opened beforehand, is untouched. Three: `RET` writes the file, and
        # the window follows the FILE: its focused row is now in the second
        # theme's accent with no restart; and the footer copied the two
        # fragments beside the file. The two accents are far more than the
        # twelve per cent apart; the window is measured on its top half BELOW
        # its own hint line, so neither the footer's lines nor the hint's
        # registers enter the count.
        # -----------------------------------------------------------------
        if THEMES > 1:
            # The region is as tall as the LONGER of the chooser's two lists:
            # the themes, or the desk option's header and its answers — the
            # answers counted from the texts (`DESK_ANSWERS` above), which
            # every answer must have a key in.
            CHOOSER_ROWS = max(THEMES, 1 + DESK_ANSWERS)
            first_accent = "#" + token_hex("state.accent")
            second_accent = "#" + token_hex("state.accent", SECOND)
            control("files-window")
            machine.sleep(2)
            w, h, x, y = niri_geometry()
            upper = f"{w}x{h // 2 - LINE}+{x}+{y + LINE}"

            control("theme-open")
            shot("26-theme-chooser", lines=2 + CHOOSER_ROWS)
            has_colour(first_accent, at_least=20, of="/tmp/26-theme-chooser.png")
            lacks_colour(second_accent, of="/tmp/26-theme-chooser.png")

            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Down")
            machine.sleep(1)
            shot("27-theme-preview", lines=2 + CHOOSER_ROWS)
            has_colour(second_accent, at_least=20, of="/tmp/27-theme-preview.png")
            lacks_colour(first_accent, of="/tmp/27-theme-preview.png")
            # The preview is the footer's alone: the window still wears the first.
            crop(upper, "/tmp/window-during-preview.png")
            has_colour(first_accent, at_least=20, of="/tmp/window-during-preview.png")
            lacks_colour(second_accent, of="/tmp/window-during-preview.png")

            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
            machine.sleep(2)
            machine.succeed("grep -q '\"${second}\"' /root/.config/ronoctua/theme.json")
            shot("28-theme-applied")
            crop(upper, "/tmp/window-after.png")
            has_colour(second_accent, at_least=20, of="/tmp/window-after.png")
            lacks_colour(first_accent, of="/tmp/window-after.png")
            # And the fragments beside the file, for the compositor and the terminal.
            machine.succeed("test -s /root/.config/ronoctua/niri-theme.kdl")
            machine.succeed("test -s /root/.config/ronoctua/ghostty-theme.conf")
            machine.succeed("grep -q 'active-color' /root/.config/ronoctua/niri-theme.kdl")
            # A file that never said a desk is on black, and the fragment says so.
            machine.succeed("grep -q 'background-color \"#000000\"' /root/.config/ronoctua/niri-theme.kdl")

            # THE SECOND COLUMN: the desk. `<right>` crosses to it, `<down>`
            # lands on the theme's own colour and the compositor is shown it
            # at once — the fragment is the preview — and `RET` writes the
            # pair. The desk itself is not on this screen (the window fills
            # it); what is proved is the file niri watches, and the check
            # `themes-have-fragments` proves niri accepts and merges it.
            second_desk = "#" + token_hex("surface.backdrop", SECOND)
            control("theme-open")
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Right")
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Down")
            machine.sleep(1)
            shot("29-theme-desk", lines=2 + CHOOSER_ROWS)
            machine.succeed(f"grep -q 'background-color \"{second_desk}\"' /root/.config/ronoctua/niri-theme.kdl")
            machine.succeed("grep -q '\"black\"' /root/.config/ronoctua/theme.json || ! grep -q backdrop /root/.config/ronoctua/theme.json")
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
            machine.sleep(2)
            machine.succeed("grep -q '\"backdrop\":\"theme\"' /root/.config/ronoctua/theme.json")
            machine.succeed(f"grep -q 'background-color \"{second_desk}\"' /root/.config/ronoctua/niri-theme.kdl")

  '';
}
