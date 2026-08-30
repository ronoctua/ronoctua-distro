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
  ronoctua-shell,
  ronoctua-brain,
  fonts,
}:

let
  xvfb = import ../tests/niri-xvfb.nix { inherit pkgs lib; };
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = linePx * 2;

  # A SYNTAX COLOUR, AS THE TOKEN FILE WRITES IT.
  #
  # Read here rather than typed into the test script, and that is what makes
  # the measurement below worth taking: it ties the photograph to
  # `assets/tokens/colors.nix`. A colour hard-coded in Rust, or a role-to-token
  # map that lost an arm, fails in this build instead of on the owner's screen
  # — and changing the theme moves the assertion with it rather than breaking
  # it (invariant 9, reaching a screenshot).
  #
  # `0xRRGGBBAA` becomes `RRGGBB`: the alpha is always opaque here, and
  # ImageMagick wants six digits.
  colours = (import ../assets/tokens/colors.nix).syntax;
  colourOf = name: builtins.substring 2 6 colours.${name};

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
      "# a comment, which is the one colour that recedes"
      "{ pkgs, lib, ... }:"
      "let"
      "  name = \"ronoctua\";"
      "  size = 42;"
      "  deep = { a = { b = [ 1 2 3 ]; }; };"
      "in"
      "{"
      "  enable = true;"
      "  packages = with pkgs; [ git ripgrep fd ];"
      "  imports = [ ./base.nix ./desktop.nix ];"
      "  settings = {"
      "    theme = \"tokyo-night\";"
      "    lines = 12;"
      "  };"
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
    ${xvfb.prelude}

    FOOTER = ${toString footerPx}
    LINE = ${toString linePx}


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


    def crop(box):
        """Cuts `box` out of the last screenshot, for measuring."""
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {box} +repage "
            "/tmp/measured.png"
        )


    def has_colour(hex_value, at_least=1):
        """How many pixels of `/tmp/measured.png` are that colour.

        A LOOSE MATCH on purpose: the surface is drawn through wgpu and the
        text through a shaper, so a glyph's interior is the token's colour and
        its edges are blends of it. Twelve per cent is wide enough to catch the
        body of a letter and narrow enough that green does not answer for
        purple — the two furthest apart in this palette are `syntax.string`
        and `syntax.keyword`, and they differ by far more than that.
        """
        n = int(
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/measured.png -fuzz 12% "
                f"-fill '#000000' +opaque '{hex_value}' -fill '#FFFFFF' "
                f"-opaque '{hex_value}' -format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )
        assert n >= at_least, (
            f"{hex_value} covers {n} pixel(s) of the panel, and {at_least} were owed"
        )
        return n


    def rows_with_ink(box):
        """How many rows of pixels in `box` have anything drawn on them."""
        crop(box)
        out = machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/measured.png -resize 1x! "
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
        "${ronoctua-shell}/bin/ronoctua-shell"
    )
    machine.wait_until_succeeds(
        "journalctl -u footer --no-pager | grep -q 'footer layout resolved'",
        timeout=90,
    )

    with subtest("the footer at rest, with the bell"):
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
        control("notif-history")
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
        control("notif-history")

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
        # Nix fence, and CLAUDE.md names that trap in as many words: inside the
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
        panel = f"{w_t // 4}x{h_t - 4 * LINE}+{w_t // 2}+{2 * LINE}"

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
        crop(panel)
        has_colour("#${colourOf "string"}", at_least=20)
        has_colour("#${colourOf "keyword"}", at_least=20)

        # AND IT SCROLLS. `C-,` pages the preview and the bar follows.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+comma")
        machine.sleep(2)
        shot("20-preview-of-text-paged")

        # AND IT IS MEASURED TOO: the panel's own pixels changed. Restoring
        # `total: 1` leaves the key with nothing to move and this fails.
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop "
            f"{panel} +repage /tmp/after.png"
        )
        same = machine.succeed(
            "${pkgs.imagemagick}/bin/magick compare -metric AE /tmp/measured.png "
            "/tmp/after.png null: 2>&1 || true"
        ).strip()
        assert same not in ("0", "0 (0)"), (
            "the preview drew exactly the same pixels after the page key — "
            "it did not scroll"
        )

        # AND IT GOES, or it stands in front of every shot after it. The bracket
        # keeps the pattern from matching the `pkill` that carries it —
        # `CLAUDE.md` names that one.
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

  '';
}
