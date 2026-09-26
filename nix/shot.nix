# ONE FEATURE'S PHOTOGRAPHS, on a real compositor: `nix run .#shot -- <scene>`.
#
# # A tool, not a battery
#
# The owner's rule (2026-09-25): the regions worth photographing are decided
# WHEN a feature is built, and a photograph of a feature built before is not
# kept, rerun or grown. So nothing here names a scene. This file is the STAGE
# — the machine, the compositor, the footer, and the helpers a scene is
# written with — and the scene is a Python file the person or agent building
# the feature writes where they like, OUTSIDE this repository, runs, reads,
# and throws away. The pictures land in `$XDG_CACHE_HOME/ronoctua-distro/shot`,
# emptied at the start of every run.
#
# It replaced a battery of forty-two permanent scenes that re-photographed the
# whole system for every change, grew until its script no longer fit in one
# kernel argument, and needed two checks to keep its bookkeeping honest.
#
# # Why a VM, and not a script on the developer's machine
#
# Because a screenshot is only worth something if it shows what the SYSTEM
# does, and the system is a layer-shell surface under the compositor: Xvfb,
# the compositor on its X11 backend, `xwd` for the pixels — the path
# `tests/compositor-xvfb.nix` already has.
#
# # What a scene can use
#
# Everything the test driver offers (`machine`, `subtest`), and the helpers
# below: `control(<command id>)` runs a command in the footer, `notify(...)`
# raises a notification, `shot(<name>, lines=)` photographs the footer (or the
# whole screen) into the output, `crop`, `changed`, `pixels_of`, `has_colour`,
# `lacks_colour`, `rows_with_ink` measure, `token_hex(<token>)` is a colour of
# the default theme, and `LINE`, `LINE_EXACT`, `CELL`, `FOOTER` are the grid.
# Two things photographs taught: the compositor's cursor is WHITE and shows
# up in every crop that contains it, so an assertion that "something is drawn
# here" is better asked from outside the screen (a journal line, a `pgrep`);
# and `has_colour`'s default fuzz is wide enough for a letter to answer for a
# filled shape — measure a region with a small fuzz.
{
  pkgs,
  lib,
  scroll,
  # The solved palettes and the theme list: a scene measures against the
  # default theme's values.
  themed,
  defaultTheme,
  ronoctua-shell,
  ronoctua-brain,
  ronoctua-transfers,
  ronoctua-player,
  fonts,
}:

let
  xvfb = import ../tests/compositor-xvfb.nix { inherit pkgs lib; };
  # WHERE A CAPTURE LANDS, from the same entry `nix/modules/scroll.nix` reads.
  screenshotDir = (import ./user-dirs.nix).screenshot;
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = linePx * 2;
  # HOW WIDE A CELL IS, in pixels, and NOT rounded: a click is placed by
  # multiplying it, and rounding would walk the aim a fifth of a cell further
  # right on every column counted.
  cellPx = typography.size.base * typography.cellWidthEm;
  # THE TOKENS' VALUES, as the generator solved them, for the default theme —
  # a store path the driver opens when it runs.
  palette = "${themed}/${defaultTheme}.json";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-shot";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      ronoctua-brain
      ronoctua-transfers
      ronoctua-player
      pkgs.imagemagick
      pkgs.libnotify
      pkgs.xdotool
      pkgs.xwininfo
      # AND `xwd`, which every crop starts with.
      pkgs.xwd
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
    import os as _os

    # The default theme's palette, `key -> 0xRRGGBBAA`.
    TOKENS = _json.load(open("${palette}"))


    def token_hex(key, table=None):
        """`RRGGBB` of a token, for ImageMagick. The alpha is dropped."""
        return (table or TOKENS)[key][2:8]

    ${xvfb.prelude}

    FOOTER = ${toString footerPx}
    LINE = ${toString linePx}
    # THE LINE UNROUNDED, because a HEIGHT is a sum of them and not a multiple
    # of the rounded one.
    LINE_EXACT = ${toString (typography.size.base * typography.lineHeight)}
    CELL = ${toString cellPx}


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
        whole compositor window, which is what the manager's screenshots want.
        """
        w, h, x, y = compositor_geometry()
        machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
        if lines is None:
            crop = f"{w}x{h}+{x}+{y}"
        else:
            tall = LINE * lines
            crop = f"{w}x{tall}+{x}+{y + h - tall}"
        machine.succeed(
            f"magick /tmp/screen.xwd -crop {crop} +repage "
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
            f"magick /tmp/screen.xwd -crop {box} +repage {into}"
        )


    def changed(a, b):
        """The share of the screen that differs between two pictures."""
        return float(machine.succeed(
            f"magick /tmp/{a}.png /tmp/{b}.png -compose difference "
            "-composite -threshold 0 -format '%[fx:mean]' info:"
        ))


    def pixels_of(path, hex_value, match=True):
        """Every (x, y) of `path` that is that colour — or that is NOT, with
        `match=False`.

        Exact, never a fuzz: this answers WHERE a drawing is, and a fuzz
        wide enough for a glyph's edge answers with the letter beside it.

        `match=False` is how INK is found on a known ground: a glyph is
        antialiased and may have no pixel of the exact colour at all.
        """
        machine.succeed(f"magick {path} -depth 8 txt:- > /tmp/pixels.txt")
        want = hex_value.lower()[:7]
        found = []
        for pixel in machine.succeed("cat /tmp/pixels.txt").splitlines()[1:]:
            head, _, colour = pixel.partition(": ")
            said = colour.split()
            # NOT A PIXEL, or a colour spelt with fewer fields than usual:
            # reading `[1]` of one was an IndexError six minutes into a run.
            if "," not in head or len(said) < 2:
                continue
            if (said[1].lower()[:7] == want) == match:
                x, y = head.split(",")
                found.append((int(x), int(y)))
        return found


    def has_colour(hex_value, at_least=1, of="/tmp/panel-before.png", fuzz=12):
        """How many pixels of `of` are that colour.

        A LOOSE MATCH on purpose: the surface is drawn through wgpu and the
        text through a shaper, so a glyph's interior is the token's colour and
        its edges are blends of it. Twelve per cent is wide enough to catch the
        body of a letter and narrow enough that green does not answer for
        purple — the two furthest apart in this palette are `syntax.string`
        and `syntax.keyword`, and they differ by far more than that.

        AND IT IS LOOSE ENOUGH TO LIE ABOUT A FILLED SHAPE. Twelve per cent is
        a Euclidean distance of about fifty-three in eight-bit units, and
        `surface.atlas.land` (`#D7D2CA`) sits twenty-eight from
        `text.primary` (`#E0E0E0`) — so a panel of ordinary WORDS answers
        "there are continents here". What needs the width is a GLYPH, whose
        edges are blends; a disc's interior is the flat token, so an assertion
        about a filled region passes a small `fuzz` and stops being answerable
        by text.
        """
        n = int(
            machine.succeed(
                f"magick {of} -fuzz {fuzz}% "
                f"-fill '#000000' +opaque '{hex_value}' -fill '#FFFFFF' "
                f"-opaque '{hex_value}' -format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )
        assert n >= at_least, (
            f"{hex_value} covers {n} pixel(s) of the panel, and {at_least} were owed"
        )
        return n


    def lacks_colour(hex_value, of, fuzz=12):
        """The other half of `has_colour`: not one pixel of `of` is that colour.
        What proves a theme LEFT the screen.

        `fuzz` IS A PARAMETER, and twelve is not always right. Twelve per cent
        absorbs the antialiasing of ONE colour; telling two colours APART needs
        less, and the theme chooser is where that showed. With `cyber` in force
        its two titles are drawn in `text.group`, `#96A6F7`, and every theme
        within four rows of it has an accent inside twelve per cent of that —
        midnight 9.5, industrial 10.2, owl 6.6, darkside 5.9, all measured. So
        the assertion "the row below has not appeared yet" was reading the
        chooser's own headings and failing about the wrong thing."""
        n = int(
            machine.succeed(
                f"magick {of} -fuzz {fuzz}% "
                f"-fill '#000000' +opaque '{hex_value}' -fill '#FFFFFF' "
                f"-opaque '{hex_value}' -format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )
        assert n == 0, f"{hex_value} still covers {n} pixel(s) of {of}"


    def rows_with_ink(box):
        """How many rows of pixels in `box` have anything drawn on them."""
        crop(box, "/tmp/ink.png")
        out = machine.succeed(
            "magick /tmp/ink.png -resize 1x! "
            "-threshold 6% -format '%[fx:int(mean*h+0.5)]' info:"
        )
        return int(out.strip())


    machine.start()
    machine.wait_for_unit("multi-user.target")
    # THE ONE TEST THAT NEEDS THE BRAIN TO SPEAK: it photographs its line.
    start_xvfb(brain=True)
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

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
        f"--setenv=SCROLLSOCK={sock} "
        f"--setenv=SWAYSOCK={sock} "
        "--setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
        "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_notify=debug "
        "--setenv=RONOCTUA_SWAPPY=${pkgs.swappy}/bin/swappy "
        # AND WHAT TAKES A CAPTURE, with where it lands — the two
        # `nix/modules/scroll.nix` puts on this unit in production. The
        # compositor has no screenshot of its own, so without these the
        # `screen-capture` command echoes "nothing to do" and the scene that
        # photographs the capture's notification photographs an empty band.
        "--setenv=RONOCTUA_GRIM=${pkgs.grim}/bin/grim "
        "--setenv=RONOCTUA_SCREENSHOTS=${screenshotDir} "
        # AND THE TAG READER, which `nix/modules/media.nix` puts on the footer's
        # unit in production. Without it `ronoctua-inspect` answers
        # `Refusal::Missing` for every photograph, so a file with GPS in it
        # draws a panel of plain facts — and the globe, which is the whole
        # subject of one scene here, could not be photographed at all.
        "--setenv=RONOCTUA_EXIFTOOL=${pkgs.exiftool}/bin/exiftool "
        # The theme's floor and the generator's fragments, as the module
        # exports them — the chooser's shots copy a fragment and assert it.
        "--setenv=RONOCTUA_THEME=${defaultTheme} "
        "--setenv=RONOCTUA_THEMES=${themed} "
        "${ronoctua-shell}/bin/ronoctua-shell"
    )
    machine.wait_until_succeeds(
        "journalctl -u footer --no-pager | grep -q 'footer layout resolved'",
        timeout=90,
    )


    # THE SCENE, which this file does not know: the path the app was given.
    _scene = _os.environ.get("RONOCTUA_SHOT_SCENE")
    assert _scene, "no scene: run `nix run .#shot -- <scene.py>`"
    exec(compile(open(_scene).read(), _scene, "exec"))
  '';
}
