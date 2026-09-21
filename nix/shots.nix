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
  # WHICH SCENES TO PHOTOGRAPH. `null` is every one of them, which is what
  # `nix build .#shots` asks for; the flake also exposes one package per scene,
  # and that is what a delivery that changed one screen should run. See
  # `nix/shots-scenes.nix` for the list and for why it exists.
  only ? null,
  # The solved palettes and the theme list: the driver reads the default
  # theme's values when it measures a screenshot.
  themed,
  themes,
  defaultTheme,
  ronoctua-shell,
  ronoctua-brain,
  ronoctua-transfers,
  fonts,
}:

let
  declared = import ./shots-scenes.nix;
  scenes = map (s: s.id) declared;
  needsOf = lib.listToAttrs (
    map (s: {
      name = s.id;
      value = s.needs or [ ];
    }) declared
  );
  # WHAT A SCENE STANDS ON, all the way down.
  #
  # Two levels would be enough today and the closure is what the rule MEANS:
  # asking for a scene asks for everything it cannot run without. The answer is
  # put back into the declared ORDER, because the script runs top to bottom and
  # a need that came after its dependant would be a need that never ran.
  closure =
    let
      step = have: lib.unique (have ++ lib.concatMap (s: needsOf.${s}) have);
      settle = have: if step have == have then have else settle (step have);
    in
    have: lib.filter (s: builtins.elem s (settle have)) scenes;
  wanted =
    if only == null then
      scenes
    else
      let
        unknown = lib.subtractLists scenes only;
      in
      # A NAME NOBODY WROTE IS A RUN THAT PHOTOGRAPHS NOTHING, and silently:
      # the guard in the script would simply never fire. Refused here, where
      # the message can name it.
      if unknown == [ ] then
        closure only
      else
        throw "shots: no such scene: ${lib.concatStringsSep ", " unknown}";
  xvfb = import ../tests/niri-xvfb.nix { inherit pkgs lib; };
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = linePx * 2;
  # HOW WIDE A CELL IS, in pixels, and NOT rounded.
  #
  # `LINE` above is a ceiling because it is only ever used to aim a row, and a
  # row is 27 px tall either way. This one aims a COLUMN: the cell is 10,8 px,
  # and a click is placed by multiplying it, so rounding it to 11 would walk
  # the aim a fifth of a cell further right on every column counted. It is the
  # same arithmetic `rails::Frame` does with the value the tokens give it
  # (`size.base x cellWidthEm`), which is what makes a number measured here
  # land on the cell the shell drew (invariant 9, reaching a screenshot).
  cellPx = typography.size.base * typography.cellWidthEm;

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
  palette = "${themed}/${defaultTheme}.json";
  # AND THE SECOND THEME'S, for the chooser's shots: a preview has to show a
  # colour the default does not have, and the assertion reads it from the
  # generator rather than from a number typed here.
  # With one theme declared the chooser's shots have nothing to prove and the
  # driver skips them (`THEMES > 1`); the path still has to exist.
  #
  # THE ONE A SINGLE `<down>` REACHES, which is the row below the DEFAULT and
  # not the second in the list. Those were the same number while the first
  # declared theme was also the default; they stopped being when the owner
  # asked for the chooser's order and the system's default to be two
  # declarations (`assets/themes/themes.nix`). It wraps at the end, exactly as
  # the cursor does not — so if the default is ever last, this takes the first
  # and the driver's `<down>` would not reach it. That is why the assertion
  # below reads the FILE rather than assuming.
  defaultAt = lib.lists.findFirstIndex (t: t == defaultTheme) 0 themes;
  second =
    if builtins.length themes > 1 then
      builtins.elemAt themes (lib.mod (defaultAt + 1) (builtins.length themes))
    else
      builtins.head themes;
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
  # A DOCUMENT MADE HERE, and nothing of the owner's brought in.
  #
  # The repository is PUBLIC (invariant 2) and nothing personal enters it
  # (invariant 13), so his own `sample.docx` cannot be the fixture. A `.docx`
  # is a zip with one XML part in it, and that is the whole reason the office
  # family was cheap to read — so building one here proves the reader against
  # the FORMAT rather than against a file somebody once sent.
  #
  # STORED and not deflated (`-0`): what is under test is which member holds
  # the words, and the inflate has its own tests in `crate::zip`.
  sampleDocument = pkgs.runCommand "sample.docx" { nativeBuildInputs = [ pkgs.zip ]; } ''
    mkdir -p build/word
    cat > build/word/document.xml <<XML
    <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
      <w:body>
        <w:p><w:r><w:t>A document the preview can read</w:t></w:r></w:p>
        <w:p><w:r><w:t>and a second paragraph under it</w:t></w:r></w:p>
      </w:body>
    </w:document>
    XML
    cd build && zip -q -r -X -0 $out .
  '';

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
      # AND `xwd`, which every crop in this file starts with. It is on the PATH
      # rather than spelled as a store path for the reason written at the head
      # of `testScript`: a single environment string may not pass the kernel's
      # 128 KiB, and this battery is close enough to it that one more scene was
      # what crossed the line.
      pkgs.xwd
      # THE SCREENSHOT'S EDITOR. Without it the `edit` action is not offered —
      # `usable_actions` drops what the machine cannot carry out — and the scene
      # would show three actions where a full install shows four.
      pkgs.swappy
      # `gdbus`, to send the compositor's own `Notify` by hand.
      pkgs.glib
    ]
    ++ xvfb.packages;

    # TOR, INSTALLED AND STOPPED — which is the state the owner reported: he
    # could not find a way to switch the network ON. `Standing` comes from
    # `systemctl show -p LoadState -p ActiveState`, so the unit has to EXIST
    # for the sheet to say anything but "not installed"; and it must not be
    # started, because a machine that boots into Tor is not this distro's
    # arrangement (`nix/modules/tor.nix`, and the polkit rule's two verbs).
    #
    # `services.tor` and not `ronoctua.tor`: this node is a bare NixOS with a
    # handful of packages, not the distro, and importing the module would pull
    # the whole option tree in for one unit file.
    services.tor.enable = true;
    systemd.services.tor.wantedBy = lib.mkForce [ ];

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  # THE THREE TOOLS THIS SCRIPT USES ARE CALLED BY NAME, not by store path.
  #
  # `xdotool`, `magick` and `xwd` are in `environment.systemPackages` above, so
  # the VM has them on its PATH — and the difference is not tidiness. A store
  # path is some sixty characters and this script spells those three a hundred
  # and twenty-seven times between them; the expansion was seven kibibytes.
  #
  # THE CEILING IS THE KERNEL'S. A single argument or environment string may not
  # pass `MAX_ARG_STRLEN`, which is 128 KiB, and the driver hands this script
  # over as one. Crossing it does not say so: the build dies with
  # `executing '…/bash': Argument list too long`, which reads like a broken
  # derivation. Measured the day a scene of four kibibytes was added and the
  # whole battery stopped building.
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

    # EVERY SCENE THERE IS, and the ones this run is for — see
    # `nix/shots-scenes.nix` for the list and for why it exists.
    ALL_SCENES = {${lib.concatMapStringsSep ", " (s: "\"${s}\"") scenes}}
    SCENES = {${lib.concatMapStringsSep ", " (s: "\"${s}\"") wanted}}


    def wanted(scene):
        """Whether this run photographs that scene."""
        # THE ID HAS TO BE ONE OF THE DECLARED ONES, asked here rather than
        # only in the check: a typo in a guard would otherwise skip a subtest
        # for ever and the run would still pass, which is the shape of every
        # empty assertion this file has already been bitten by.
        assert scene in ALL_SCENES, f"no such scene: {scene}"
        return scene in SCENES

    FOOTER = ${toString footerPx}
    LINE = ${toString linePx}
    # THE LINE UNROUNDED, because a HEIGHT is a sum of them and not a multiple
    # of the rounded one. `LINE` is the ceiling (27) and is right for aiming a
    # pointer or cropping a strip; two real lines are 52.2, which the shell
    # asks for as 53. Measuring a surface with the ceiling reads a footer at
    # rest as one pixel too short and refuses it — see the subtest below.
    LINE_EXACT = ${toString (typography.size.base * typography.lineHeight)}
    CELL = ${toString cellPx}
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

    if wanted("clipboard"):
        with subtest("the clipboard with an EMPTY history — the photographed defect"):
            # -----------------------------------------------------------------
            # THIS IS THE STATE THE OWNER SENT A PHOTOGRAPH OF. The panel used to
            # reserve three lines and draw them at `end - 3`, `end - 2` and
            # `end - 1`, where `end` is the top of the region plus the number of
            # rows — and with no history that number is ONE. Nothing clamped them,
            # so `ClipCascade: not integrated` landed on the MINIBUFFER's line and
            # the actions strip landed on the row that says the history is empty.
            #
            # An empty history is the FIRST state to photograph and was the only
            # one nobody had: there was no layout test of the clipboard at all.
            #
            # IT RUNS FIRST, before anything opens a WINDOW: it gives the region
            # back with `Escape`, and an `Escape` with a window focused closes that
            # window — which took the file manager out from under the theme
            # chooser's subtest when this sat in the middle.
            # -----------------------------------------------------------------
            control("clipboard-open")
            shot("30-clipboard-empty", lines=10)
            # THE REGION CARRIES MORE THAN ONE LINE OF INK, and that is the
            # measurement — not a word on it.
            #
            # Before the fix the drawn list was ONE row ("Nothing copied yet") and
            # the other three lines were painted OUTSIDE the region entirely, at
            # `end - 3`, `end - 2` and `end - 1`, which with `grid.lines == 1`
            # land on the modeline, the minibuffer and that single row. So the
            # region itself held one inked row while the screen showed four lines
            # of clipboard. Now every one of them is a ROW, and the region is as
            # tall as they are.
            w, h, x, y = niri_geometry()
            region = f"{w}x{10 * LINE}+{x}+{y + h - 10 * LINE}"
            inked = rows_with_ink(region)
            assert inked > LINE, (
                f"the clipboard's region has {inked} rows of ink with an empty "
                "history; the sections were drawn somewhere else"
            )
            focus_niri()
            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)

    if wanted("history"):
        with subtest("the shell history, and the row the keyboard is on"):
            # -----------------------------------------------------------------
            # WHAT THE OWNER READ ON THE SCREEN: nothing said which line the
            # cursor was on. `Row::focused` is set by the view and read by NO door
            # in this system — neither the footer nor the window draws a band
            # behind the cursor — so the colour IS the cursor, and this view was
            # the one that never set it. Nine others have carried it since
            # `network.rs` arrived at the split: the label takes `state.accent`
            # ("the keyboard is here") and the value goes on saying whether the
            # answer can be changed.
            #
            # THE ASSERTION IS A COLOUR, because the report was. A count of rows
            # or a word on screen would have passed the whole time the defect
            # existed.
            #
            # `atuin` itself is a store path baked into the shell
            # (`RONOCTUA_ATUIN`), so this machine has it without installing
            # anything, and it is not logged in — which is the state a fresh
            # machine is in and the one worth photographing.
            # -----------------------------------------------------------------
            control("atuin-open")
            shot("32-shell-history", lines=12)
            has_colour(
                "#" + token_hex("state.accent"),
                at_least=20,
                of="/tmp/32-shell-history.png",
            )
            focus_niri()
            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)

    if wanted("tor"):
        with subtest("Tor: the section exists with the daemon STOPPED"):
            # -----------------------------------------------------------------
            # THE OTHER HALF OF WHAT THE OWNER REPORTED: he could not find a way
            # to turn Tor on. The section lived behind `if let Some(scene)`, and
            # the control socket only answers when Tor is already running — so a
            # stopped Tor drew no heading, no row and no key.
            #
            # This machine does not run Tor, which is exactly the case being
            # photographed: the heading has to be there anyway, with a row that
            # says what the UNIT is doing.
            #
            # `network-tor-open` AND NOT A COUNT OF `TAB`s. The first version of
            # this pressed `TAB` nine times and the picture came out on the Wi-Fi
            # section — the ring's length depends on what hardware the machine
            # has, and this VM has neither radio nor cable. A guess that produces
            # a picture is worse than no picture.
            # -----------------------------------------------------------------
            # THE LIST FIRST, so the sheet can be told from it. This subtest had
            # NO ASSERTION, and that is how it passed twice on a picture of the
            # wrong screen: first the nine `TAB`s that landed on Wi-Fi, then
            # `fresh_at` looking for a row that a machine without Tor does not
            # draw. Two pictures that are equal prove the door opened nothing.
            control("network-open")
            machine.sleep(1)
            shot("31a-network-list", lines=12)
            focus_niri()
            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)

            control("network-tor-open")
            shot("31-tor-stopped", lines=12)
            list_shot = machine.succeed(
                "magick /tmp/31a-network-list.png -format '%#' info:"
            ).strip()
            sheet_shot = machine.succeed(
                "magick /tmp/31-tor-stopped.png -format '%#' info:"
            ).strip()
            assert list_shot != sheet_shot, (
                "`network-tor-open` drew the same screen as `network-open`: "
                "the sheet was never opened"
            )
            # AND IT IS NOT A BLANK SCREEN EITHER. The sheet is a definition list
            # — the service, the path, the bridges, the onion services and the
            # drawer — so nearly every line of it carries ink, which a view that
            # opened on nothing would not.
            w, h, x, y = niri_geometry()
            tall = LINE * 12
            assert rows_with_ink(f"{w}x{tall}+{x}+{y + h - tall}") > tall // 2, (
                "the Tor sheet came out mostly blank"
            )
            focus_niri()
            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)

    if wanted("footer"):
        with subtest("the footer at rest — two lines, and no bell"):
            # AT REST IS A CLAIM, AND IT WAS NEVER CHECKED.
            #
            # This scene is named for a state it did not put the footer into:
            # run after the others, the network view from the scene above was
            # still up and the surface was 314 px — twelve lines. The shot
            # below crops the last two, so it looked right either way, which is
            # how a subtest photographs something other than its own name for
            # as long as nobody measures.
            #
            # `Esc` closes one level, so a view several deep needs several. The
            # X focus first, because under Xvfb nobody has it by default and a
            # key with nowhere to go is a key that changes nothing.
            focus_niri()
            for _ in range(8):
                machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)
            shot("1-footer-at-rest", lines=2)

            # AND A CLICK ON THE EMPTY MINIBUFFER IS THE LEADER.
            #
            # The owner's rule: *"caso não haja nada aberto no minibuffer,
            # quero que, ao clicar com o botão esquerdo do mouse nele, ocorra o
            # mesmo que ocorreria se eu clicasse a tecla leader"*. This is the
            # scene where nothing IS open, which is the whole precondition.
            #
            # MEASURED BY THE FOOTER'S OWN HEIGHT and not by pixels: the list
            # is the surface growing upwards, so what says it opened is the
            # layout the shell resolved — a fact from outside the picture, which
            # a cursor or a stale frame cannot answer.
            def footer_height():
                said = machine.succeed(
                    "journalctl -u footer --no-pager"
                    " | grep -o 'footer layout resolved [^ ]* height=[0-9]*'"
                    " | tail -1"
                ).strip()
                return int(said.rsplit("=", 1)[1])

            w_f, h_f, x_f, y_f = niri_geometry()
            at_rest = footer_height()
            # AIMED FROM THE FOOTER'S TOP, and in LINES — see
            # `nix/shots-scenes.nix`.
            at_rest_lines = round(at_rest / LINE_EXACT)
            assert 2 <= at_rest_lines <= 4, (
                f"the footer is {at_rest} px, {at_rest_lines} lines"
                " — that is not a footer at rest"
            )
            top = y_f + h_f - at_rest
            # THE LEFT HALF OF THE LINE: the chevron that opens it lives at the
            # right edge and is a target with a meaning of its own.
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_f + w_f // 3} {top + LINE + LINE // 2}"
            )
            machine.succeed("DISPLAY=:99 xdotool click 1")
            machine.sleep(2)
            opened = footer_height()
            assert opened > at_rest, (
                f"the footer stayed {at_rest} px tall after a click on the empty "
                "minibuffer — the shortcut list did not open"
            )
            shot("37-the-minibuffer-clicked", lines=2)

            # AND THE POINTER REACHES EVERY COLUMN OF THAT LIST — see
            # `nix/shots-scenes.nix`. The band is `surface.hover`, and where it
            # is drawn is the only thing on screen that says which item a click
            # would take.
            # ON THE POINTER'S OWN ROW, and not the leftmost pixel of that
            # colour anywhere on the screen: a stray one elsewhere answered for
            # the band, and gave the same number for two different columns.
            def band_x(name, at_y):
                shot(name, lines=None)
                xs = [
                    x
                    for x, y in pixels_of(
                        f"/tmp/{name}.png", "#" + token_hex("surface.hover")
                    )
                    if abs(y - at_y) <= LINE
                ]
                return min(xs) if len(xs) > 20 else None

            # THE SECOND ROW OF THE LIST, which is above the footer's top AT
            # REST: the list is the surface having grown upwards.
            row = y_f + h_f - opened + 2 * LINE
            seen = []
            for n, part in enumerate((8, 3, 2)):
                machine.succeed(
                    f"DISPLAY=:99 xdotool mousemove {x_f + w_f // part} {row}"
                )
                machine.sleep(1)
                seen.append(band_x(f"39-the-list-across-{n}", row - y_f))
            print(f"the band starts at {seen}")
            assert None not in seen, (
                f"the band was at {seen}: the pointer lit nothing in one of the "
                "columns of the shortcut list"
            )
            assert seen[0] < seen[1] < seen[2], (
                f"the band was at {seen} across the width of the list: it is not "
                "following the pointer into the columns"
            )

            # AND `Esc` PUTS IT BACK, which is what makes this a toggle and not
            # a trap: the scenes after this one all begin with nothing open.
            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(2)
            # SMALLER THAN IT WAS OPEN, and not equal to what it was at rest:
            # the brain's line comes and goes on a clock of its own, so the
            # footer at rest is not the same height twice. What is under test
            # is the list closing.
            assert footer_height() < opened, (
                "the footer did not shrink after `Esc` — the shortcut list is still up"
            )

    if wanted("band"):
        with subtest("the discreet band, one line tall"):
            notify("copied to the clipboard")
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep -q 'band=1'", timeout=30
            )
            shot("2-discreet-band", lines=3)

    if wanted("queue"):
        with subtest("the queue, with the counter"):
            for i in (1, 2, 3, 4):
                notify(f"message {i}")
            shot("3-queue-with-counter", lines=3)

    if wanted("loud"):
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

    if wanted("screenshot"):
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
                "magick -size 320x200 canvas:gray30 "
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

    if wanted("notifications"):
        with subtest("the manager, in two columns"):
            control("notif-open")
            shot("6-manager-two-columns")

    if wanted("catalogue"):
        with subtest("the distro's catalogue, in the third column"):
            # X INPUT FOCUS FIRST. With no window manager nobody calls
            # `XSetInputFocus`, so the focus stays on `PointerRoot` and every key
            # `xdotool` injects is lost — no error, no log, nowhere.
            focus_niri()
            # TAB twice: what happened, what was suppressed, what is configured.
            for _ in (1, 2):
                machine.succeed("DISPLAY=:99 xdotool key Tab")
                machine.sleep(1)
            shot("7-distro-catalogue")
            # And one entry's sheet, which is where "use the distro's default or
            # customise" is actually done.
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(1)
            shot("8-adjustment-sheet")
            control("notif-open")

    if wanted("manager"):
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
                "magick -size 480x320 "
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
                "DISPLAY=:99 xdotool type --delay 40 'shot'"
            )
            machine.sleep(1)
            # AND INTO THE DIRECTORY. Filtering alone leaves the cursor on
            # `shots/` — which is a folder, and a folder has no picture inside
            # it, so the panel correctly drew the type's glyph. The file is one
            # level down.
            machine.succeed("DISPLAY=:99 xdotool key Return")
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
                "DISPLAY=:99 xdotool mousemove "
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
                "DISPLAY=:99 xdotool mousemove "
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
                machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)
            control("files-open")
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'many'"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(2)
            shot("16-footer-listing-cut")
            # `files-open` OPENS — it does not toggle, so asking twice reopened it.
            # And what the manager leaves up is a PAGE, which the footer draws BELOW
            # the band and below the modeline: cropping the last lines with it on
            # screen photographs the listing. That is exactly how the brain's line
            # came out as a file listing and no one noticed — green is not proof,
            # and neither is a screenshot nobody opened.
            for _ in range(8):
                machine.succeed("DISPLAY=:99 xdotool key Escape")
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

    if wanted("manager-window"):
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
                "DISPLAY=:99 xdotool type --delay 40 'shots'"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(2)
            # SAME REASON as the footer's: the band needs the pointer.
            w_w, h_w, x_w, y_w = niri_geometry()
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
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
                "DISPLAY=:99 xdotool mousemove "
                f"{x_w + 450} {y_w + h_w - 3 * LINE - 6}"
            )
            machine.sleep(1)
            shot("15-hover-on-the-windows-facts")

            # AND THE SENTENCE ABOUT THAT FACT COMES OUT IN TWO INKS.
            #
            # The owner read a cut label in this very panel and asked for both
            # halves in full on the message line *and* told apart — *"a cor da
            # legenda e do valor devem se diferenciar"*. The line used to be one
            # run in one token, so the only way to see the fix is to count both
            # tokens ON THAT LINE.
            #
            # CROPPED TO THE LINE, and to this window's own half: the whole
            # frame carries `text.subtle.default` in every hint on screen, so a
            # measurement of the frame would pass with the sentence in a single
            # colour. The message line of this door is its FIRST.
            def sentence_inks(into):
                """The two registers on this door's message line."""
                crop(f"{w_w // 2}x{LINE}+{x_w}+{y_w}", into=into)
                return (
                    has_colour(
                        "#" + token_hex("text.subtle.default"), at_least=8, of=into
                    ),
                    has_colour(
                        "#" + token_hex("text.subtle.strong"), at_least=8, of=into
                    ),
                )

            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            here = sentence_inks("/tmp/two-inks-here.png")

            # AND THE SAME LINE OVER ANOTHER FACT, because two inks ALONE prove
            # nothing.
            #
            # A hint carries both registers too — the key in `strong`, the prose
            # around it in `default` — so a measurement of one line would go
            # green with the sentence never drawn at all, which is the shape of
            # every empty assertion this file has been bitten by. What a hint
            # cannot do is CHANGE when the pointer moves from one fact to
            # another: two facts have different names and different values, so
            # the ink they cover differs. The hint would give the same two
            # numbers twice.
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_w + 450} {y_w + h_w - 5 * LINE - 6}"
            )
            machine.sleep(1)
            # A FRESH FRAME, because `crop` cuts the LAST screenshot and the
            # pointer has moved since it was taken.
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            there = sentence_inks("/tmp/two-inks-there.png")

            assert here != there, (
                "the message line reads the same over two different facts "
                f"({here} and {there}) — it is not showing the fact under the "
                "pointer"
            )

            # AND THE CUT LABEL ITSELF, which is the case the owner reported.
            #
            # A panel twenty-four cells wide gives the label a third of them, so
            # `permission` arrives as `permissi…` and `owner:group` as
            # `owner:gr…`. The value always said itself in full on this line and
            # the label said nothing at all — *"ao colocar o cursor do mouse
            # sobre a legenda e sobre o valor, o texto deles deve aparecer por
            # completo no minibuffer"*. The pointer goes on the LABEL column,
            # which is the left third of the panel.
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_w + 340} {y_w + h_w - 5 * LINE - 6}"
            )
            machine.sleep(1)
            shot("22-the-cut-label-says-itself")

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
                machine.succeed("DISPLAY=:99 xdotool click 5")
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
            machine.succeed("DISPLAY=:99 xdotool key BackSpace")
            machine.sleep(1)
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'many'"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(2)
            # THE POINTER OVER THE LISTING and not over a panel: the wheel moves
            # the section it is on, which is the whole point of the four sections.
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_w + 200} {y_w + 6 * LINE}"
            )
            machine.sleep(1)
            for _ in range(60):
                machine.succeed("DISPLAY=:99 xdotool click 5")
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
            machine.succeed("DISPLAY=:99 xdotool key BackSpace")
            machine.sleep(1)
            control("files-window")
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | tail -40"
                " | grep -q 'br.ronoctua.files'",
                timeout=60,
            )
            machine.sleep(3)
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'shots'"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(1)
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'code'"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Return")
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
            machine.succeed("DISPLAY=:99 xdotool key ctrl+comma")
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
                    "magick /tmp/col.png -threshold 20% -trim "
                    "-format '%[fx:page.y+h]' info: 2>/dev/null || echo 0"
                ).strip()
                return at_y + int(got)

            bottom = ink_bottom(panel, 2 * LINE)
            foot = f"{w_t // 4 - 70}x{LINE}+{w_t // 2 + 10}+{bottom - LINE}"
            assert rows_with_ink(foot) >= 1, (
                "there is nothing along the foot of the preview — a line one can "
                "only reach with the keyboard is half a view"
            )

            # AND THE SIDEWAYS BAR MOVES, counted on a four-pixel probe at the
            # left edge and never by `-trim` — see `nix/shots-scenes.nix`.
            def thumb_over_the_left(box):
                probe = f"4x{LINE}+{box.split('+')[1]}+{box.split('+')[2]}"
                return int(
                    machine.succeed(
                        f"magick /tmp/screen.xwd -crop {probe} +repage "
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
                    "DISPLAY=:99 xdotool key ctrl+bracketright"
                )
            machine.sleep(2)
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
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
                "magick /tmp/screen.xwd -crop "
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
                    f"magick {path} -format '%#' info:"
                ).strip()

            assert signature("/tmp/panel-before.png") != signature("/tmp/after.png"), (
                "the preview drew the same pixels after the page key — it did not "
                "scroll"
            )

            # AND THE EXECUTE BIT COLOURS THE GLYPH, in BOTH states — see
            # `nix/shots-scenes.nix`.
            w_x, h_x, x_x, y_x = niri_geometry()
            listing = f"{w_x}x{h_x - 2 * LINE}+{x_x}+{y_x + 2 * LINE}"
            green = "#" + token_hex("state.success")
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            crop(listing, "/tmp/before-chmod.png")
            before = has_colour(green, at_least=0, of="/tmp/before-chmod.png")

            machine.succeed("chmod +x /root/shots/code/sample.nix")
            # THE FOLDER GIVES NOTICE, so no key is needed — `ronoctua_files::notice`
            # is an inotify watch on the open directory. The sleep is the burst
            # the watcher absorbs, not a guess.
            machine.sleep(3)
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            crop(listing, "/tmp/after-chmod.png")
            after = has_colour(green, at_least=0, of="/tmp/after-chmod.png")
            assert after >= before + 8, (
                f"the green went from {before} to {after} pixels after the chmod — "
                "the glyph of a file that can be run is not marked"
            )
            shot("38-the-execute-bit-on-the-glyph")
            # AND IT IS PUT BACK, because the scenes after this one read the
            # same directory.
            machine.succeed("chmod -x /root/shots/code/sample.nix")
            machine.sleep(2)

    if wanted("document"):
        with subtest("a document: a zip of XML reaching the panel as words"):
            # WHY, AND WHY IT IS A PHOTOGRAPH — see `nix/shots-scenes.nix`.
            # THE WINDOW DOOR HAS TO BE OPEN, and this scene does not open one;
            # the line below says so out loud if the closure ever stops.
            machine.succeed("pgrep -f '[b]in/ronoctua-shell files'")
            machine.succeed("mkdir -p /root/shots/papers")
            machine.succeed("cp ${sampleDocument} /root/shots/papers/sample.docx")
            # ONE LEVEL UP, and one only. The subtest above left the window in
            # `/root/shots/code`; `papers` is its sibling, so a second `BackSpace`
            # would leave the manager at `/root`, where the filter finds nothing
            # and the panel photographs an empty directory.
            machine.succeed("DISPLAY=:99 xdotool key BackSpace")
            machine.sleep(1)
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'papers'"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(3)
            shot("33-preview-of-a-document")

            w_d, h_d, x_d, y_d = niri_geometry()
            # THE SAME BOX THE TEXT PREVIEW IS MEASURED IN, and for its reasons.
            paper = f"{w_d // 4 - 70}x{h_d - 4 * LINE}+{w_d // 2 + 10}+{2 * LINE}"
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            drawn = rows_with_ink(paper)
            # A REFUSED PANEL DRAWS ONE DIM LINE. Two paragraphs draw more, and
            # taking `document::of` out of the chain brings this straight back to a
            # sentence — which is what `sem prévia` looked like before any of this.
            assert drawn >= 3, (
                f"the preview drew ink on {drawn} row(s) of pixels — a document "
                "came out as a sentence about not being able to show one"
            )
            # AND IT IS THE CONTENT'S OWN COLOUR, and not the dim tone a refusal
            # wears: the one measurement that tells "it read the file" from "it
            # said it could not".
            crop(paper, "/tmp/paper.png")
            has_colour("#" + token_hex("text.primary"), at_least=20, of="/tmp/paper.png")



    if wanted("miniplayer"):
        with subtest("the miniplayer: it moves, it holds, and it draws over nothing"):
            # A GIF AND FOUR COLOURS NOTHING ELSE HERE WEARS — see
            # `nix/shots-scenes.nix`.
            # THE SAME DOOR THIS SCENE RIDES ON, asked the same way.
            machine.succeed("pgrep -f '[b]in/ronoctua-shell files'")
            FILM = ["#FF00FF", "#00FFFF", "#FFFF00", "#FF6000"]
            machine.succeed(
                "magick -delay 20 -loop 0 -size 240x180 "
                + " ".join(f"xc:'{c}'" for c in FILM)
                + " /root/shots/moving.gif"
            )
            machine.succeed("DISPLAY=:99 xdotool key BackSpace")
            machine.sleep(1)
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'moving'"
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
                    "xwd -display :99 -root -out /tmp/screen.xwd"
                )
                machine.succeed(
                    "magick /tmp/screen.xwd -crop "
                    f"{pane} +repage /tmp/{name}.png"
                )
                return f"/tmp/{name}.png"

            def film_pixels(path, colour):
                """How many pixels of one of the film's colours the panel holds."""
                return int(
                    machine.succeed(
                        f"magick {path} -fuzz 4% "
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
                "DISPLAY=:99 xdotool key ctrl+semicolon"
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
                f"magick /tmp/held-0.png -fuzz 4% "
                f"-fill white -opaque '{FILM[held[0]]}' -fill black +opaque white "
                "-trim -format '%[fx:w] %[fx:h]' info:"
            ).split()
            ink_w, ink_h = int(box[0]), int(box[1])
            assert ink_w > 0 and ink_h > 0, "the picture has no extent at all"
            assert ink_w < ink_h * 2, (
                f"the picture is {ink_w} by {ink_h} — a four-to-three film came out "
                "as a band, which is the frame's height being guessed again"
            )

            # FOUR: THE CONTROLS ARRIVE WITH THE POINTER — see
            # `nix/shots-scenes.nix`.
            foot = f"{w_m // 4 - 70}x{2 * LINE}+{w_m // 2 + 10}+{2 * LINE + (h_m - 4 * LINE) - 2 * LINE}"

            def glyph_pixels(box):
                return int(
                    machine.succeed(
                        f"magick /tmp/screen.xwd -crop {box} "
                        f"+repage -fuzz 12% -fill white -opaque '#{token_hex('text.primary')}' "
                        "-fill black +opaque white "
                        "-format '%[fx:int(mean*w*h+0.5)]' info:"
                    ).strip()
                )

            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
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
                f"magick /tmp/held-0.png -fuzz 4% "
                f"-fill white -opaque '{FILM[held[0]]}' -fill black +opaque white "
                "-trim -format '%[fx:page.x] %[fx:page.y] %[fx:w] %[fx:h]' info:"
            ).split()
            px, py, pw, ph = (int(v) for v in spot)
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_m + (w_m // 2 + 10) + px + pw // 2} "
                f"{y_m + (2 * LINE) + py + ph // 2}"
            )
            machine.sleep(2)
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
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
                    f"magick {path} -fuzz 4% "
                    f"-fill white -opaque '{colour}' -fill black +opaque white "
                    "-trim -format '%[fx:w] %[fx:h]' info:"
                ).split()
                return int(got[0]), int(got[1])

            machine.succeed(
                "magick /tmp/screen.xwd -crop "
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

            # AND NO HOVER BAND ON THE FILM — see `nix/shots-scenes.nix`.
            def band_pixels(path):
                return int(
                    machine.succeed(
                        f"magick {path} -crop {pane} +repage "
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
                f"magick /tmp/screen.xwd -crop {foot} +repage "
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
            machine.succeed("DISPLAY=:99 xdotool key ctrl+w")
            # LONG ENOUGH FOR THE RESPAWN. Giving the region away changes the box,
            # which changes the frame's shape, which is a kill and a new child —
            # 108 to 242 ms measured, and this machine is a software-rendered VM.
            # Photographed sooner, what is on screen is the frame cut for the OLD
            # box, letterboxed into the new one, which is a true picture of a
            # transient and a false picture of the layout.
            machine.sleep(4)
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            shot("25-miniplayer-whole")

            # A RULE AGAINST THE SCREEN'S OWN EDGE. With one panel filling the
            # width there is nothing to its left to be separated from, and the
            # arithmetic put the line in cell nought: *"no modo largo há uma linha
            # vertical na esquerda que continua aparecendo"*. Measured as ink in the
            # first two columns of the region.
            edge = f"2x{h_m - 4 * LINE}+{x_m}+{y_m + 2 * LINE}"
            rule_ink = int(
                machine.succeed(
                    f"magick /tmp/screen.xwd -crop {edge} +repage "
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
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            machine.succeed(
                "magick /tmp/screen.xwd -crop "
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
                    f"magick {path} -fuzz 2% "
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
            machine.succeed("DISPLAY=:99 xdotool key ctrl+w")
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

    if wanted("globe"):
        with subtest("the globe, and the address under it"):
            # A PHOTOGRAPH WITH A COORDINATE IN IT, made here.
            #
            # Nothing of the owner's goes into this battery (invariant 13), and
            # nothing of his is needed: exiftool writes the two tags onto a
            # picture ImageMagick draws, and the coordinate is LONDON — which
            # `assets/world.nix` already declares in the open.
            machine.succeed(
                "magick -size 240x160 "
                "gradient:'#3C6E9F-#D8B36A' /root/where.jpg"
            )
            machine.succeed(
                "${pkgs.exiftool}/bin/exiftool -overwrite_original "
                "-GPSLatitude=51.51 -GPSLatitudeRef=N "
                "-GPSLongitude=-0.13 -GPSLongitudeRef=W "
                "/root/where.jpg"
            )
            # AND THE WINDOW THE SCENE BEFORE LEFT, out of the way: two
            # managers on screen is two pictures in the crop, and the second one
            # is at another directory.
            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)
            control("files-window")
            # WAITED ON THE PROCESS, and not on the journal.
            #
            # The scene above greps the footer's log for the window announcing
            # itself, which is fine there and wrong here: the manager was just
            # killed, so an announcement inside the last lines is the OLD
            # window's, and a `tail` short enough to exclude it is a `tail` that
            # misses the new one behind everything the film scene logged. The
            # process is a fact from outside the picture and the kill above
            # makes it unambiguous — which is the rule `AGENTS.md` records for
            # the white mouse pointer.
            machine.wait_until_succeeds(
                "pgrep -f '[b]in/ronoctua-shell files'", timeout=60
            )
            machine.sleep(3)
            # TYPED AND NOT ENTERED. A letter is a QUERY and it goes on the
            # end of the path; the cursor lands on the first match by itself,
            # which is what puts the photograph in the info panel. `Return`
            # would OPEN it, and an image viewer over the window is not this
            # picture.
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'where'"
            )
            # LONG ENOUGH FOR THE THREAD TO ANSWER: exiftool runs, the atlas is
            # read once (six megabytes, on the first coordinate of the session)
            # and the globe is projected and painted.
            machine.sleep(6)

            # THE STRIP FIRST, and it is not an aside.
            #
            # Below `space.files.panelsFrom` cells the panels are the four-line
            # strip at the bottom, which is where a window at half a 1280-pixel
            # screen puts them — and the strip GROWS for a globe now. It did not
            # for a round, and the owner photographed exactly this window with a
            # coordinate, an atlas, a panel and no globe.
            shot("41-the-globe-in-the-strip")
            w_s, h_s, x_s, y_s = niri_geometry()
            crop(f"{w_s}x{8 * LINE}+{x_s}+{y_s + h_s - 8 * LINE}", into="/tmp/strip.png")
            # THE SEA, AND A THOUSAND PIXELS OF IT.
            #
            # Not `surface.atlas.land`, and the reason was measured on this very
            # crop: at four per cent fuzz the light warm grey of the land is
            # within reach of the ANTIALIASED EDGES of ordinary text, and three
            # hundred and sixty-six glyph pixels answered an assertion about
            # continents. The sea is a dark violet no text in this panel wears,
            # and a disc of it is thousands of pixels — a number letters cannot
            # reach.
            has_colour(
                "#" + token_hex("surface.atlas.ocean"),
                at_least=1000,
                of="/tmp/strip.png",
                fuzz=4,
            )

            # AND THEN MAXIMISED, because the columns are the other half of the
            # layout and the disc gets the room to be a disc.
            control("window-maximize")
            machine.sleep(3)
            w_g, h_g, x_g, y_g = niri_geometry()
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_g + 300} {y_g + 4 * LINE}"
            )
            machine.sleep(2)
            shot("39-the-globe-and-its-address")

            # THE SEA AND THE LAND ARE DRAWN, and it is the assertion this
            # scene exists for.
            #
            # THE INFO COLUMN ONLY, and the reason is the trap this file has
            # fallen into twice: an assertion answered by something else on
            # screen. `surface.atlas.ocean` is a dark violet and so is
            # `surface.selection`, the band under the cursor in the LISTING —
            # crop the listing out and the question can only be answered by the
            # panel the globe is in. The quarter is the panel: the manager puts
            # the facts in the rightmost column.
            #
            # AND THE FUZZ IS TIGHT, because a disc is a filled region rather
            # than a glyph: at the default twelve per cent every light letter on
            # the panel answers for `surface.atlas.land`.
            crop(f"{w_g // 4}x{h_g}+{x_g + 3 * (w_g // 4)}+{y_g}", into="/tmp/globe.png")
            has_colour(
                "#" + token_hex("surface.atlas.ocean"),
                at_least=200,
                of="/tmp/globe.png",
                fuzz=4,
            )
            has_colour(
                "#" + token_hex("surface.atlas.land"),
                at_least=200,
                of="/tmp/globe.png",
                fuzz=4,
            )

            # AND THE DOT IS ON IT. The accent at its loudest, drawn nowhere
            # else in this panel — so this says the coordinate was PLACED and
            # not merely read.
            has_colour(
                "#" + token_hex("mark.atlas.dot"),
                at_least=4,
                of="/tmp/globe.png",
                fuzz=4,
            )

            # AND THE HAND TURNS IT, which is the half no unit test reaches.
            #
            # The owner reported the whole of the mouse as dead — *"ao tentar
            # clicar e arrastar nada ocorre"* — and the cause was that the
            # picture's rows carried a field of NO WIDTH, so the press found
            # nothing to grab. What proves the fix is not a target in a table:
            # it is pressing on the figure, moving, and the pixels being
            # different afterwards.
            #
            # `mousedown`/`mousemove`/`mouseup` and not `click`, because a drag
            # is exactly what `xdotool click` is not — the file already records
            # that `xdotool key` releases what it presses.
            def picture_of(into):
                crop(f"{w_g // 4}x{h_g}+{x_g + 3 * (w_g // 4)}+{y_g}", into=into)

            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_g + 3 * (w_g // 4) + w_g // 8} {y_g + 3 * LINE}"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool mousedown 1")
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove_relative -- 60 20"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool mouseup 1")
            machine.sleep(2)
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            picture_of("/tmp/turned.png")
            shot("42-the-globe-turned-by-the-hand")

            def signature(path):
                return machine.succeed(
                    f"magick {path} -format '%#' info:"
                ).strip()

            assert signature("/tmp/globe.png") != signature("/tmp/turned.png"), (
                "the globe drew the same pixels after a press and a drag across "
                "it — the picture is not answering the pointer"
            )
            # AND IT IS STILL A GLOBE afterwards: a drag that turned it into an
            # empty box would also pass the test above.
            has_colour(
                "#" + token_hex("surface.atlas.ocean"),
                at_least=1000,
                of="/tmp/turned.png",
                fuzz=4,
            )

            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)

    if wanted("facts-wide"):
        with subtest("the facts given the whole region, in columns"):
            # A FILE WITH A GREAT DEAL TO SAY, made here.
            #
            # The whole question is what the facts do when there are more of
            # them than there are lines, and in real life that is a photograph
            # — of which this repository may carry none (invariant 13). So
            # every tag is written onto a picture ImageMagick draws, and every
            # value in it was chosen by this test.
            machine.succeed(
                "magick -size 240x160 "
                "gradient:'#3C6E9F-#D8B36A' /root/facts.jpg"
            )
            machine.succeed(
                "${pkgs.exiftool}/bin/exiftool -overwrite_original "
                "-Make=Ronoctua -Model='Test Camera' -Software=ronoctua-shots "
                "-Artist=nobody -Copyright='public domain' "
                "-ImageDescription='a gradient with a lot written on it' "
                "-UserComment='every tag here was chosen by this test' "
                "-DateTimeOriginal='2026:09:16 12:45:03' "
                "-ExposureTime=1/125 -FNumber=2.8 -ISO=400 -FocalLength=35 "
                "-LensModel='35mm f/2.8' -Orientation=1 -WhiteBalance=0 "
                "-Flash=0 -MeteringMode=5 -ExposureProgram=3 -ColorSpace=1 "
                "-SceneCaptureType=0 -Contrast=0 -Saturation=0 -Sharpness=0 "
                "-XResolution=300 -YResolution=300 -ResolutionUnit=2 "
                # AND WHERE IT WAS MADE, because the expanded sheet gives the
                # globe a COLUMN of its own and that is half of what this scene
                # is for. London, which `assets/world.nix` declares in the open.
                "-GPSLatitude=51.51 -GPSLatitudeRef=N "
                "-GPSLongitude=-0.13 -GPSLongitudeRef=W "
                "/root/facts.jpg"
            )
            # AND A WINDOW OF ITS OWN, for the reason the globe's scene gives:
            # two managers on screen is two panels in the crop, and the other
            # one is at another directory.
            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)
            control("files-window")
            machine.wait_until_succeeds(
                "pgrep -f '[b]in/ronoctua-shell files'", timeout=60
            )
            machine.sleep(3)
            control("window-maximize")
            machine.sleep(2)
            # TYPED AND NOT ENTERED: a letter is a QUERY and the cursor lands
            # on the first match by itself, which is what puts the file in the
            # panels. `Return` would open it.
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'facts'"
            )
            # LONG ENOUGH FOR EXIFTOOL, which is a child process per file.
            machine.sleep(6)

            w_f, h_f, x_f, y_f = niri_geometry()
            # THE POINTER IN THE FACTS' OWN CORNER, and that cell is EMPTY.
            #
            # It is where the button is drawn and it is the one the owner could
            # not reach: *"é impossível clicar no ícone, pois ele desaparece
            # antes de o cursor conseguir chegar nele, porque você implementou o
            # hover … apenas em partes do conteúdo, em vez de implementar na
            # coluna como um todo (incluindo espaços vazios)"*. Aiming at the
            # bottom-right of the panel is aiming at blank space, which is
            # exactly what the old resolution answered `None` for.
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_f + w_f - 3 * CELL} {y_f + h_f - 2 * LINE}"
            )
            machine.sleep(2)
            shot("43-the-facts-and-the-button-in-their-corner")

            # THE THUMB FOLLOWS THE HAND, and over a file WITH a globe.
            #
            # The owner felt this one under his fingers: *"ao clicar e arrastar
            # com o mouse uma scrollbar que tem o globo, a barra da scrollbar se
            # desloca imediatamente muito abaixo da posição de onde está o
            # cursor"*. The thumb was drawn from the panel MINUS the globe's
            # pinned band and the drag divided by the whole panel, so the two
            # disagreed by a picture's height — and only on a file that carries
            # a coordinate, which is why no other scene could have caught it.
            #
            # DRAGGED TO THE MIDDLE and not to an end: at either end both
            # arithmetics land in the same place, and the gap between them is
            # widest half way down.
            bar_x = x_f + w_f - 6
            mid = y_f + h_f // 2
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            crop(f"{w_f // 4}x{h_f}+{x_f + 3 * (w_f // 4)}+{y_f}", into="/tmp/globe-before.png")
            sea_before = has_colour(
                "#" + token_hex("surface.atlas.ocean"),
                at_least=0,
                of="/tmp/globe-before.png",
                fuzz=4,
            )
            # WHAT THIS SCENE DOES NOT PHOTOGRAPH — see
            # `nix/shots-scenes.nix`.

            machine.succeed(
                f"DISPLAY=:99 xdotool mousemove {bar_x} {y_f + 3 * LINE}"
            )
            machine.succeed("DISPLAY=:99 xdotool mousedown 1")
            machine.succeed(
                f"DISPLAY=:99 xdotool mousemove {bar_x} {mid}"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool mouseup 1")
            machine.sleep(2)
            shot("45-the-thumb-under-the-hand-over-a-globe")

            # THE THUMB IS WHERE THE HAND LEFT IT: a line either side of the
            # cursor, in the bar's own column.
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            crop(f"12x{3 * LINE}+{x_f + w_f - 12}+{mid - LINE}", into="/tmp/thumb.png")
            has_colour(
                "#" + token_hex("rule.strong"), at_least=10, of="/tmp/thumb.png", fuzz=4
            )

            # AND THE GLOBE GAVE ITS LINES BACK. Its rows scroll now, so half
            # way down the sheet there is less map than there was — which is
            # the other half of what he asked for: *"ao arrastar a scrollbar ele
            # se move junto liberando espaço para linhas de conteúdo"*.
            crop(f"{w_f // 4}x{h_f}+{x_f + 3 * (w_f // 4)}+{y_f}", into="/tmp/globe-after.png")
            sea_after = has_colour(
                "#" + token_hex("surface.atlas.ocean"),
                at_least=0,
                of="/tmp/globe-after.png",
                fuzz=4,
            )
            print(f"facts-wide: sea in the narrow panel {sea_before} -> {sea_after}")
            assert sea_before > 1000, (
                f"the narrow panel had {sea_before} pixel(s) of sea before the "
                "drag, so there was no globe to scroll"
            )
            assert sea_after < sea_before, (
                f"the globe kept all {sea_after} pixel(s) of its sea after the "
                "sheet was scrolled: its rows are pinned again"
            )

            # MEASURED IN BOTH STATES, which is the only way a pixel count
            # means anything: what the key did is the DIFFERENCE, and a
            # threshold picked without the other number is a threshold picked
            # out of the air.
            #
            # THE LEFT HALF OF THE WINDOW, across the middle of it. Before the
            # key that is the sidebar and the listing; after it, it can only be
            # the facts and the globe, because there is nothing else left on
            # screen.
            half = f"{w_f // 2}x{8 * LINE}+{x_f}+{y_f + h_f // 2}"

            def inks(into):
                machine.succeed(
                    "xwd -display :99 -root -out /tmp/screen.xwd"
                )
                crop(half, into=into)
                return (
                    has_colour(
                        "#" + token_hex("text.dim"), at_least=0, of=into, fuzz=4
                    ),
                    # THE SEA, AND AT FOUR PER CENT. A disc is a filled region,
                    # so a small fuzz is what makes the question unanswerable by
                    # the antialiased edge of a letter — the trap this file has
                    # fallen into once and `AGENTS.md` records.
                    has_colour(
                        "#" + token_hex("surface.atlas.ocean"),
                        at_least=0,
                        of=into,
                        fuzz=4,
                    ),
                )

            dim_before, sea_before = inks("/tmp/columns-before.png")

            # AND THEN THE REGION IS GIVEN AWAY.
            machine.succeed("DISPLAY=:99 xdotool key ctrl+shift+w")
            machine.sleep(3)
            shot("44-the-facts-with-the-whole-region")
            dim_after, sea_after = inks("/tmp/columns-after.png")
            print(
                f"facts-wide: left half dim {dim_before} -> {dim_after}, "
                f"sea {sea_before} -> {sea_after}"
            )

            # THE KEY DID SOMETHING, and the two pictures say so.
            def signature(path):
                return machine.succeed(
                    f"magick {path} -format '%#' info:"
                ).strip()

            assert signature("/tmp/columns-before.png") != signature(
                "/tmp/columns-after.png"
            ), "`C-W` drew the same pixels — the facts were not given the region"

            # AND WHAT IT DID IS PUT FACTS OVER THERE. The labels' ink, in a
            # half of the window that held none of it while the facts were a
            # panel twenty-four cells wide on the other side of the screen.
            assert dim_after > 200, (
                f"the labels' ink went {dim_before} -> {dim_after} in the left "
                "half: the facts did not reach it"
            )

            # AND THE GLOBE IS A COLUMN OF IT. The sea is a dark violet no text
            # in this panel wears, and a disc of it is thousands of pixels — a
            # number letters cannot reach. Before the key the globe was a band
            # at the top of a narrow panel on the far right, so the same crop
            # had none of it.
            assert sea_after > 1000, (
                f"the sea went {sea_before} -> {sea_after} in the left half: the "
                "expanded sheet has no column with the map in it"
            )

            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)

    if wanted("thumbnails"):
        with subtest("the entries side by side, as pictures"):
            # A FOLDER MADE HERE, because this repository carries no pictures
            # (invariant 13) and the whole question is what a picture looks like
            # in a cell.
            #
            # THE COLOUR IS CHOSEN TO BE UNANSWERABLE BY TEXT. Every assertion
            # below counts pixels of it, and `AGENTS.md` records what happens
            # when that colour is near a token's: the glyphs answer. Magenta is
            # in none of the nine themes — they are greys, greens, cyans, a navy
            # and a red — and a filled rectangle of it at four per cent fuzz is
            # a question no antialiased letter can answer.
            machine.succeed("mkdir -p /root/thumbs")
            for name in ("a-picture.png", "b-picture.png"):
                machine.succeed(
                    "magick -size 200x200 "
                    f"xc:'#FF00AA' /root/thumbs/{name}"
                )
            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)
            control("files-window")
            machine.wait_until_succeeds(
                "pgrep -f '[b]in/ronoctua-shell files'", timeout=60
            )
            machine.sleep(3)
            control("window-maximize")
            machine.sleep(2)
            machine.succeed(
                "DISPLAY=:99 xdotool type --delay 40 'thumbs'"
            )
            machine.sleep(2)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(4)

            w_t, h_t, x_t, y_t = niri_geometry()
            # THE LISTING'S OWN COLUMN, which is where a thumbnail may appear
            # and the panels beside it may not: the preview draws the SAME
            # picture, and a crop that held both would pass with the grid
            # turned off.
            listing = f"{w_t // 2}x{h_t - 4 * LINE}+{x_t}+{y_t + 2 * LINE}"

            def pink(into):
                machine.succeed(
                    "xwd -display :99 -root -out /tmp/screen.xwd"
                )
                crop(listing, into=into)
                return has_colour("#FF00AA", at_least=0, of=into, fuzz=4)

            before = pink("/tmp/thumbs-list.png")
            shot("46-the-listing-one-thing-to-a-line")

            # AND THEN THE ENTRIES GO SIDE BY SIDE.
            machine.succeed("DISPLAY=:99 xdotool key ctrl+g")
            # LONG ENOUGH FOR THE WORKERS, which are child processes: the
            # measurement that sized the pool is 109 ms for the smallest file
            # there is, and this machine has no GPU and a software rasteriser.
            machine.sleep(8)
            after = pink("/tmp/thumbs-grid.png")
            shot("47-the-entries-side-by-side-as-pictures")
            print(f"thumbnails: the picture's own colour {before} -> {after}")

            # THE LISTING HELD NONE OF IT ONE THING TO A LINE. A row is a glyph
            # and a name; there is nowhere for a picture to be.
            assert before == 0, (
                f"the listing already had {before} pixel(s) of the picture's own "
                "colour before the key, so this crop is not the listing alone"
            )
            # AND A SCREENFUL OF IT AFTER. Two cells of two hundred pixels
            # scaled into a thumbnail is thousands, which is a number no glyph
            # reaches.
            assert after > 2000, (
                f"the grid drew {after} pixel(s) of the picture: the cells are "
                "still glyphs"
            )

            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)

    if wanted("geo"):
        with subtest("the regions, and the streets this machine does not have"):
            # THE SECOND HALF OF THE GLOBE, and the half a person drives: what
            # is installed, the catalogue of everything the baked world carries,
            # and the filter that is the way through five hundred rows.
            #
            # NOTHING IS INSTALLED HERE, and that is the state worth
            # photographing: the sentence saying an address stops at the
            # nearest town is what somebody sees the first time they open this.
            control("geo-open")
            machine.sleep(2)
            shot("40-the-regions-and-their-streets", lines=12)

            # THE VIEW IS REALLY OPEN, asked OUTSIDE the picture: the footer
            # resolves a layout every time its height changes, and a region this
            # tall is this view asking for all the lines it can have. A crop cannot be
            # answered by the white mouse pointer; a height can be answered by
            # nothing else.
            said = machine.succeed(
                "journalctl -u footer --no-pager"
                " | grep -o 'footer layout resolved [^ ]* height=[0-9]*'"
                " | tail -1"
            ).strip()
            tall = int(said.rsplit("=", 1)[1])
            assert round(tall / LINE_EXACT) > 4, (
                f"the footer is {tall} px — the regions' view did not open"
            )

            # AND A ROW THAT ACTS WEARS THE INK THAT SAYS SO. Every catalogue
            # row is a door, so `text.answer` is most of this picture; a screen
            # of facts would have none of it.
            w_r, h_r, x_r, y_r = niri_geometry()
            crop(f"{w_r}x{12 * LINE}+{x_r}+{y_r + h_r - 12 * LINE}", into="/tmp/geo.png")
            has_colour("#" + token_hex("text.answer"), at_least=20, of="/tmp/geo.png")

            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)

    if wanted("brain"):
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

    if wanted("brain-buffer"):
        with subtest("the brain's own window, with a real daemon behind it"):
            # WHY THIS EXISTS BESIDE `--mind`.
            #
            # `nix run .#png --mind` draws the same component through the same
            # `paint`, in a fifth of a second, and that is what every review of
            # this screen uses. What it cannot prove is that the WINDOW opens:
            # the door, the descriptor's gate, the capability, the thread that
            # holds the daemon's socket, and the host actually pushing a mind
            # in. This is that half, and it is the half that used to be missing
            # in the other direction — the owner could not photograph the brain
            # at all.
            #
            # It needs the `brain` scene, which is what starts the daemon.
            control("brain-open")
            # THE PROCESS, and not a sleep or a pixel. A buffer opens as a
            # window of this same binary re-exec'd with `buffer <wasm>`, so
            # its existence is the thing that says the door worked.
            #
            # AND THE ASSERTION IS FROM OUTSIDE THE SCREEN on purpose: a
            # cursor is white and `text.primary` is `#E0E0E0`, so "there is
            # text here" has been answered by the MOUSE before, on an empty
            # desktop, with the subtest passing.
            machine.wait_until_succeeds(
                "pgrep -f 'ronoctua-shell buffer' >/dev/null", timeout=30
            )
            machine.sleep(2)
            shot("9-brain-buffer")

    if wanted("update"):
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

    if wanted("theme"):
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
                lacks_colour(second_accent, of="/tmp/26-theme-chooser.png", fuzz=6)

                machine.succeed("DISPLAY=:99 xdotool key Down")
                machine.sleep(1)
                shot("27-theme-preview", lines=2 + CHOOSER_ROWS)
                has_colour(second_accent, at_least=20, of="/tmp/27-theme-preview.png")
                lacks_colour(first_accent, of="/tmp/27-theme-preview.png", fuzz=6)
                # THE PREVIEW REACHES THE WINDOW TOO — see `nix/shots-scenes.nix`.
                crop(upper, "/tmp/window-during-preview.png")
                has_colour(second_accent, at_least=20, of="/tmp/window-during-preview.png")
                lacks_colour(first_accent, of="/tmp/window-during-preview.png", fuzz=6)

                machine.succeed("DISPLAY=:99 xdotool key Return")
                machine.sleep(2)
                machine.succeed("grep -q '\"${second}\"' /root/.config/ronoctua/theme.json")
                shot("28-theme-applied")
                crop(upper, "/tmp/window-after.png")
                has_colour(second_accent, at_least=20, of="/tmp/window-after.png")
                lacks_colour(first_accent, of="/tmp/window-after.png")
                second_desk_on_apply = "#" + token_hex("surface.backdrop", SECOND)
                # And the fragments beside the file, for the compositor and the terminal.
                machine.succeed("test -s /root/.config/ronoctua/niri-theme.kdl")
                machine.succeed("test -s /root/.config/ronoctua/ghostty-theme.conf")
                machine.succeed("grep -q 'active-color' /root/.config/ronoctua/niri-theme.kdl")
                # A FILE THAT NEVER SAID A DESK IS ON THE THEME'S OWN, and the
                # fragment says so. It was black until the owner made the theme's
                # desk the default — every theme solves one and every gate holds
                # it apart from the content's black, so a fresh machine shows it
                # rather than having to be asked.
                machine.succeed(
                    f"grep -q 'background-color \"{second_desk_on_apply}\"' "
                    "/root/.config/ronoctua/niri-theme.kdl"
                )

                # THE SECOND COLUMN: the desk. `<right>` crosses to it, `<down>`
                # lands on the theme's own colour and the compositor is shown it
                # at once — the fragment is the preview — and `RET` writes the
                # pair. The desk itself is not on this screen (the window fills
                # it); what is proved is the file niri watches, and the check
                # `themes-have-fragments` proves niri accepts and merges it.
                second_desk = "#" + token_hex("surface.backdrop", SECOND)
                control("theme-open")
                machine.succeed("DISPLAY=:99 xdotool key Right")
                machine.succeed("DISPLAY=:99 xdotool key Up")
                machine.sleep(1)
                shot("29-theme-desk", lines=2 + CHOOSER_ROWS)
                # THE MOVE IS UPWARDS NOW. The cursor opens on the answer in
                # force, and that is the theme's own desk — so what this proves is
                # the OTHER one: `<up>` lands on black and the compositor is shown
                # it at once, because the fragment IS the preview.
                machine.succeed("grep -q 'background-color \"#000000\"' /root/.config/ronoctua/niri-theme.kdl")
                # AND THE FILE ALREADY SAYS IT, which is the documented PRICE of
                # the preview being the broadcast: `theme::show_everywhere` writes
                # the whole pair, because that file is the channel the other doors
                # watch and a second one would be a second thing to keep in step.
                # This line asserted the opposite — "the file still says `theme`
                # until `RET`" — which was true before that change and has not been
                # since; it went unread because `.#shots` had not been run.
                machine.succeed("grep -q '\"backdrop\":\"black\"' /root/.config/ronoctua/theme.json")
                # `Esc` IS WHAT PAYS FOR IT, and that is the promise with teeth:
                # looking at a list may not give a machine a choice it never made.
                # Here a pair WAS committed, so what comes back is that pair —
                # `View::chosen_before`, asked once when the chooser opened.
                machine.succeed("DISPLAY=:99 xdotool key Escape")
                machine.sleep(2)
                machine.succeed("grep -q '\"backdrop\":\"theme\"' /root/.config/ronoctua/theme.json")
                machine.succeed(
                    f"grep -q 'background-color \"{second_desk}\"' "
                    "/root/.config/ronoctua/niri-theme.kdl"
                )
                # AND NOW FOR REAL: the same three keys, and `RET` at the end.
                control("theme-open")
                machine.succeed("DISPLAY=:99 xdotool key Right")
                machine.succeed("DISPLAY=:99 xdotool key Up")
                machine.succeed("DISPLAY=:99 xdotool key Return")
                machine.sleep(2)
                machine.succeed("grep -q '\"backdrop\":\"black\"' /root/.config/ronoctua/theme.json")
                machine.succeed("grep -q 'background-color \"#000000\"' /root/.config/ronoctua/niri-theme.kdl")
    if wanted("glance"):
        with subtest("the glance line: the grounds' air, and the countdown at the border"):
            # WHY: `nix/shots-scenes.nix`, beside the id.
            for _ in range(8):
                machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)
            control("world-glance")
            w_g, h_g, x_g, y_g = niri_geometry()
            # THE POINTER HOLDS THE CLOCK — `world::Showing::hold`.
            machine.succeed(
                f"DISPLAY=:99 xdotool mousemove {x_g + w_g // 2} {y_g + h_g - 2 * LINE - 8}"
            )
            machine.sleep(1)
            # FOUR LINES AND NOT THREE: the nested output's bottom is a row
            # below the footer's. The row is FOUND below, not counted.
            shot("35-the-glance", lines=4)

            # THE MODELINE SAYS WHERE THE ROW IS — see the scene list.
            bar = pixels_of("/tmp/35-the-glance.png", "#" + token_hex("modeline.background"))
            assert bar, "the modeline is not in the picture"
            top = min(y for _, y in bar)
            assert top > 2, "there is no row above the modeline; the glance is not up"
            band = (max(0, top - LINE + 2), top - 2)

            alternate = "#" + token_hex("glance.ground-alternate")
            ground = [(x, y) for x, y in pixels_of("/tmp/35-the-glance.png", alternate)
                      if band[0] <= y <= band[1]]
            assert ground, "the alternate ground drew no pixel; no second place was drawn"
            xs = [x for x, _ in ground]

            # THE INK IS WHAT IS NOT THE GROUND, inside it.
            other = pixels_of("/tmp/35-the-glance.png", alternate, match=False)
            inside = [x for x, y in other
                      if band[0] <= y <= band[1] and min(xs) < x < max(xs)]
            assert inside, "the alternate ground holds no text"
            left = min(inside) - min(xs)
            right = max(xs) - max(inside)
            print(f"the alternate ground: {left} px before the text, {right} px after")
            # WITHIN TWO PIXELS: a side bearing is not the ground's air.
            assert abs(left - right) <= 2, (
                f"the ground's air differs: {left} px left, {right} px right"
            )
            assert max(xs) < w_g - LINE, "the last ground ran to the edge of the line"

            # THE COUNTDOWN IS THE RIGHTMOST INK past the last ground.
            page = "#" + token_hex("glance.ground")
            drawn = pixels_of("/tmp/35-the-glance.png", page, match=False)
            marks = [x for x, y in drawn
                     if band[0] <= y <= band[1] and x > max(xs)]
            assert marks, "nothing is drawn to the right of the last place"
            from_border = w_g - max(marks)
            print(f"the countdown ends {from_border} px from the border")
            assert from_border <= LINE, (
                f"the countdown sits {from_border} px from the border, more than "
                "the one cell the edge reserves"
            )

    if wanted("locale"):
        with subtest("the settings screen: sections, and an explanation that wraps"):
            # WHY: see `nix/shots-scenes.nix`.
            for _ in range(8):
                machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)
            control("locale-open")
            machine.sleep(1)
            shot("36-the-settings", lines=2 + 15)

            # MORE THAN ONE ROW OF PROSE says it wrapped, not that it was cut.
            prose = pixels_of("/tmp/36-the-settings.png", "#" + token_hex("text.dim"))
            rows_of_prose = {y // LINE for _, y in prose}
            print(f"the prose covers {len(rows_of_prose)} rows")
            assert len(rows_of_prose) >= 2, "the explanation fits on one row"

            # AND THE LEVELS RESOLVE IN DIFFERENT TONES: they were one colour,
            # which is what made the block unreadable as a section.
            group = "#" + token_hex("text.group")
            assert group.lower() != ("#" + token_hex("text.secondary")).lower()
            has_colour(group, at_least=10, of="/tmp/36-the-settings.png", fuzz=6)

    if wanted("sweep"):
        with subtest("the rubber band: it draws over the pictures and takes what it covers"):
            # WHY: see `nix/shots-scenes.nix`.
            machine.succeed("mkdir -p /root/sweep")
            for n in range(6):
                machine.succeed(
                    f"magick -size 200x200 xc:'#FF00AA' /root/sweep/p{n}.png"
                )
            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)
            control("files-window")
            machine.wait_until_succeeds(
                "pgrep -f '[b]in/ronoctua-shell files'", timeout=60
            )
            machine.sleep(3)
            control("window-maximize")
            machine.sleep(2)
            machine.succeed("DISPLAY=:99 xdotool type --delay 40 'sweep'")
            machine.sleep(2)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(3)
            machine.succeed("DISPLAY=:99 xdotool key ctrl+g")
            machine.sleep(8)

            w_s, h_s, x_s, y_s = niri_geometry()
            listing = f"{w_s // 2}x{h_s - 4 * LINE}+{x_s}+{y_s + 2 * LINE}"
            accent = "#" + token_hex("state.accent")

            def pinks(into):
                machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
                crop(listing, into=into)
                return has_colour("#FF00AA", at_least=0, of=into, fuzz=4)

            def marks(into):
                machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
                crop(listing, into=into)
                return has_colour(accent, at_least=0, of=into, fuzz=6)

            # THE POINTER OFF THE LISTING FIRST, so nothing is lit.
            machine.succeed(
                f"DISPLAY=:99 xdotool mousemove {x_s + w_s - 40} {y_s + h_s - 40}"
            )
            machine.sleep(1)
            before_pink = pinks("/tmp/sweep-before.png")
            before_marks = marks("/tmp/sweep-before.png")
            shot("41-the-grid-before-the-band")

            # PRESSED ON THE AIR past the last column, dragged down and left.
            top = y_s + 3 * LINE
            machine.succeed(
                f"DISPLAY=:99 xdotool mousemove {x_s + w_s // 2 - 8} {top}"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool mousedown 1")
            machine.succeed(
                f"DISPLAY=:99 xdotool mousemove {x_s + 20} {top + 6 * LINE}"
            )
            machine.sleep(2)
            during_pink = pinks("/tmp/sweep-during.png")
            shot("42-the-band-over-the-pictures")
            machine.succeed("DISPLAY=:99 xdotool mouseup 1")
            machine.sleep(2)
            after_marks = marks("/tmp/sweep-after.png")
            shot("43-what-the-band-took")
            print(
                f"sweep: picture {before_pink} -> {during_pink}, "
                f"marks {before_marks} -> {after_marks}"
            )

            assert before_pink > 2000, (
                f"the grid drew {before_pink} pixel(s) of the picture: this crop "
                "is not the listing side by side"
            )
            # THE BAND IS ON TOP: underneath it would take none of it away.
            assert during_pink < before_pink, (
                f"the picture still has {during_pink} of its {before_pink} "
                "pixels under the band: the band is drawn beneath it"
            )
            # AND IT TOOK WHAT IT COVERED.
            assert after_marks > before_marks, (
                f"the listing had {before_marks} accent pixel(s) and has "
                f"{after_marks}: the band marked nothing"
            )

    if wanted("drop"):
        with subtest("a drop: the file lands in the folder it was let go over"):
            # WHY: see `nix/shots-scenes.nix`.
            # THE QUEUE HAS TO BE UP: a drop ends in it — see
            # `nix/shots-scenes.nix`.
            machine.succeed(
                "systemd-run --unit=transfers "
                "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
                "${ronoctua-transfers}/bin/ronoctua-transfers"
            )
            machine.sleep(2)
            machine.succeed("mkdir -p /root/drag/into")
            machine.succeed("echo x > /root/drag/moved.txt")
            machine.succeed("pkill -f '[b]in/ronoctua-shell files' || true")
            machine.sleep(2)
            control("files-window")
            machine.wait_until_succeeds(
                "pgrep -f '[b]in/ronoctua-shell files'", timeout=60
            )
            machine.sleep(3)
            control("window-maximize")
            machine.sleep(2)
            machine.succeed("DISPLAY=:99 xdotool type --delay 40 'drag'")
            machine.sleep(2)
            machine.succeed("DISPLAY=:99 xdotool key Return")
            machine.sleep(3)

            w_d, h_d, x_d, y_d = niri_geometry()
            # `..`, `into`, `moved.txt`: directories first, then the sort.
            # AND PAST THE SIDEBAR — `+300`, as the manager's own scene aims.
            row = lambda n: y_d + (2 + n) * LINE + LINE // 2
            here = x_d + 300
            machine.succeed(f"DISPLAY=:99 xdotool mousemove {here} {row(2)}")
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool mousedown 1")
            # TWO MOVES: see `nix/shots-scenes.nix`.
            machine.succeed(f"DISPLAY=:99 xdotool mousemove {here + 40} {row(2)}")
            machine.sleep(1)
            machine.succeed(f"DISPLAY=:99 xdotool mousemove {here} {row(1)}")
            machine.sleep(2)
            shot("44-the-folder-a-drop-would-land-in")
            machine.succeed("DISPLAY=:99 xdotool mouseup 1")
            machine.sleep(4)
            shot("45-after-the-drop")

            # A FACT FROM OUTSIDE THE PICTURE.
            machine.wait_until_succeeds("test -e /root/drag/into/moved.txt", timeout=30)
            machine.succeed("test ! -e /root/drag/moved.txt")

    if wanted("header"):
        with subtest("a window's header opens, and the chevron is what opens it"):
            # WHY: see `nix/shots-scenes.nix`.
            for _ in range(8):
                machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)
            control("network-window")
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | tail -40"
                " | grep -q 'br.ronoctua.network'",
                timeout=60,
            )
            machine.sleep(2)
            _, _, x_h, y_h = niri_geometry()
            shot("37-the-header-shut", lines=None)

            # THE WINDOW'S OWN EDGES, MEASURED. niri gives a single window a
            # COLUMN and not the output, so the screen's right border is the
            # desktop — and an assertion aimed there is answered by it. A
            # window clears with `surface.chrome` (`gpu::Gpu::frame`), so its
            # first row says where it starts and stops.
            chrome = "#" + token_hex("surface.chrome")
            row_0 = [x for x, y in pixels_of("/tmp/37-the-header-shut.png", chrome) if y < LINE]
            assert row_0, "no window on the screen's first row"
            left, right = min(row_0), max(row_0)
            print(f"the window's first row runs {left}..{right}")

            # THE CHEVRON IS DRAWN, against that edge. INK is "not the ground",
            # because a glyph is antialiased and may carry no pixel of the
            # token's exact colour.
            ink = [
                x
                for x, y in pixels_of("/tmp/37-the-header-shut.png", chrome, match=False)
                if y < LINE and left <= x <= right
            ]
            assert ink, "nothing at all is drawn on the header's row"
            from_border = right - max(ink)
            print(f"the header's last ink ends {from_border} px from the window's edge")
            assert from_border <= 3 * CELL, (
                f"the header's ink stops {from_border} px short of the edge: "
                "there is no control against it"
            )

            # AND PRESSING IT GROWS THE LINE. Counted ABOVE the footer, whose
            # own hint wears the same token.
            prose = "#" + token_hex("text.subtle.default")

            def teaching(path):
                return {
                    y // LINE
                    for x, y in pixels_of(path, prose)
                    if y < 8 * LINE and left <= x <= right
                }

            before = teaching("/tmp/37-the-header-shut.png")
            print(f"the shut header teaches on {len(before)} row(s)")
            assert len(before) == 1, "the shut header is already more than a line"
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_h + right - CELL // 2} {y_h + LINE // 2}"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool click 1")
            machine.sleep(2)
            shot("38-the-header-open", lines=None)
            after = teaching("/tmp/38-the-header-open.png")
            print(f"the opened header teaches on {len(after)} rows")
            assert len(after) > len(before), (
                "the chevron was pressed and the line did not open"
            )

    if wanted("hints"):
        with subtest("the pointer on the hint line does not take the interface with it"):
            # THE ASSERTION THAT WOULD HAVE CAUGHT IT — see `nix/shots-scenes.nix`.
            for _ in range(8):
                machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.sleep(1)
            control("files-open")
            machine.sleep(2)
            w_p, h_p, x_p, y_p = niri_geometry()
            for n in range(1, 16):
                machine.succeed(
                    "DISPLAY=:99 xdotool mousemove "
                    f"{x_p + 200} {y_p + h_p - n * LINE}"
                )
                # A SECOND ON EACH, because what killed it was not the motion but
                # the FRAME after it: the sentence is resolved from what the last
                # frame drew, so the ring only closes on the paint that follows.
                machine.sleep(1)
            # AND THE FOOTER IS STILL THERE. `is-active` and not a screenshot: an
            # aborted footer leaves its surface behind for an instant, and a picture
            # taken in that instant would pass over a dead process.
            machine.succeed("systemctl is-active footer")
            shot("34-the-pointer-on-the-hint-line", lines=3)

    if wanted("lifeline"):
        with subtest("the line that outlives the footer"):
            # THE SECOND DEFECT THE OWNER NAMED, which was not the crash: *"a
            # ronoctua-distro não sabe lidar com isso, ou seja, acaba deixando o
            # usuário sem interface"*. With the footer down, all four emergency
            # binds are dead by construction — they all write to its own control
            # socket — so nothing on screen could say what had happened.
            #
            # KILLED ON PURPOSE, and with the signal the defect produced: a stack
            # overflow ends in `process::abort`, which is `SIGABRT`.
            machine.succeed("systemctl kill -s SIGABRT footer")
            machine.wait_until_fails("systemctl is-active footer", timeout=30)
            # RAISED BY HAND HERE, because this transient unit is not the one the
            # module declares and carries no `OnFailure=`. That wiring is what
            # `a-dead-footer-is-not-a-dead-desktop` reads, and the firing itself was
            # measured with two transient units of its own; what this subtest proves
            # is the LINE.
            machine.succeed(
                "systemd-run --unit=lifeline ${xvfb.env} "
                "--setenv=WAYLAND_DISPLAY=wayland-1 "
                "--setenv=HOME=/root "
                f"--setenv=NIRI_SOCKET={sock} "
                "--setenv=RONOCTUA_THEME=${defaultTheme} "
                "--setenv=RONOCTUA_THEMES=${themed} "
                "${ronoctua-shell}/bin/ronoctua-shell lifeline footer.service"
            )
            machine.sleep(4)
            shot("35-the-line-that-outlives-the-footer", lines=2)

            # AND IT IS MEASURED. Two rows of ink where the footer used to be — with
            # the footer gone there is nothing else down there that could draw them.
            w_l, h_l, x_l, y_l = niri_geometry()
            machine.succeed("xwd -display :99 -root -out /tmp/screen.xwd")
            band = f"{w_l}x{2 * LINE}+{x_l}+{y_l + h_l - 2 * LINE}"
            drawn = rows_with_ink(band)
            assert drawn >= 2, (
                f"the rescue line drew ink on {drawn} row(s) of pixels — with the "
                "footer gone there is nothing else down there to draw it"
            )
            # THE MARK IS THE ERROR'S COLOUR, which is the one thing on the line
            # that says this is not an ordinary message.
            crop(band, "/tmp/lifeline.png")
            has_colour("#" + token_hex("state.error"), at_least=8, of="/tmp/lifeline.png")

            # AND THE MOUSE REACHES THE OPTIONS, which is the half nobody had
            # tested — the owner said so in as many words: *"tenha certeza de que
            # eles são clicáveis pelo mouse (pois como eu não testei, não sei se o
            # suporte do mouse está implementado)"*.
            #
            # THE FIRST OPTION, whose column is known by construction: the row
            # begins at `MARGIN + 2` cells — `lifeline::MARGIN` is the footer's own
            # `modeline.edgeLeft`, plus the two the warning glyph takes — so a click
            # three and a half cells in lands inside `[l]`. What it does is open the
            # journal in the rescue console, which is a SECOND PROCESS, and that is
            # what makes the click visible from out here.
            options_row = y_l + h_l - LINE + LINE // 2
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_l + int(3.5 * CELL)} {options_row}"
            )
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool click 1")
            # THE BRACKET IS THE ASSERTION, and without it there is none: `pgrep -f`
            # reads the command line of EVERY process, including the shell running
            # the `pgrep`, whose line carries the pattern. `[b]in/…` is the same
            # regex and is not the same text, so the only thing that can answer is
            # a rescue console that really started (`tests/apps-vm.nix` writes the
            # rule out, and the manager's own shot above already spells it).
            machine.wait_until_succeeds(
                "pgrep -f '[b]in/ronoctua-shell rescue'", timeout=30
            )

            # AND THE KEYBOARD REACHES THEM TOO, by the key the line draws —
            # after one more click on its own TOP row, which is what buys the
            # keyboard back. See `nix/shots-scenes.nix`.
            first_row = y_l + h_l - 2 * LINE + LINE // 2
            machine.succeed(
                "DISPLAY=:99 xdotool mousemove "
                f"{x_l + int(3.5 * CELL)} {first_row}"
            )
            machine.succeed("DISPLAY=:99 xdotool click 1")
            machine.sleep(1)
            machine.succeed("DISPLAY=:99 xdotool key Escape")
            machine.wait_until_fails("systemctl is-active lifeline", timeout=30)

            # AND THE CONSOLE IT OPENED IS PUT AWAY, because this subtest raised it.
            machine.succeed("pkill -f '[b]in/ronoctua-shell rescue' || true")


  '';
}
