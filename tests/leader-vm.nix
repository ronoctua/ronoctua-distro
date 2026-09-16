# Leader, which-key and the return to rest — acceptance criterion of milestone 5.
#
# "The leader captures the keyboard via `keyboard-interactivity: exclusive`;
# which-key expands upwards with the list BELOW the minibuffer; `Esc` goes
# back to 2 lines."
#
# WHAT IS VERIFIED ON SCREEN, AND NOT ONLY IN THE LOG
# ---------------------------------------------------
# The surface's HEIGHT. It is the only external sign that the expansion
# happened, and the log does not prove it: the footer may think it expanded
# and the compositor may have refused. We measure it by the footer's band of
# pixels before and after — at rest it is two lines, with the leader active it
# is two plus one per which-key entry.
#
# The keyboard grab is verified through niri's IPC, which exposes each layer's
# `keyboard_interactivity`. That is better than inferring from behavior: it
# says literally what the compositor understood of our request.
#
# HOW THE KEY IS SENT
# -------------------
# By `xdotool`, on Xvfb. niri runs on the winit backend over X (see
# `niri-xvfb.nix`), so a key injected into the X window reaches the compositor
# as a real key and follows the normal path to our surface. There is no
# shortcut: it is the same route as the physical keyboard.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  # THE HEIGHT COMES FROM THE TOKENS, not from a number nailed down here.
  #
  # It used to be `35`, from when the font was 14 px and the line height 1.25.
  # Both changed, and the test started measuring against a value that no longer
  # existed — a test that fails for being out of date teaches people to update
  # the number without reading what it asserts.
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = builtins.ceil (typography.size.base * typography.lineHeight * 2);
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-leader";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
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
    ${xvfb.prelude}


    # The footer's height at rest: two lines (section 4.1), computed from the
    # tokens — see this file's `let`.
    FOOTER = ${toString footerPx}


    def expansion_text():
        """What the expanded region says, read back off the screen.

        The height answers "did the region change"; this answers "what is IN
        it", and some questions are only the second kind — a filter that
        narrows to one file and one that narrows to none leave the region
        exactly as tall.
        """
        w, h, x, y = niri_geometry()
        alt = footer_height()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            # Only what is BELOW the footer's two glued lines, which is the
            # same crop the which-key subtest uses and for the same reason.
            f"-crop {w}x{alt - FOOTER}+0+{h - alt + FOOTER} +repage "
            "-colorspace gray -negate -resize 400% /tmp/expansion.png"
        )
        return machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/expansion.png stdout --psm 6 2>/dev/null"
        ).strip()

    def footer_height_above(floor, attempts=20):
        """The first height above `floor`, waited for.

        Between one height and the next there is a frame with NO buffer —
        `apply_geometry` commits the new size, and the swapchain is rebuilt
        before anything is drawn again — and a shot taken there reads 0 px:
        neither at rest nor expanded, and the assertion that fired on it said
        "the same as at rest" about a 0. The log proves the footer asked; only
        the pixels, polled, prove the compositor agreed.
        """
        seen = None
        for _ in range(attempts):
            seen = footer_height()
            if seen > floor:
                return seen
            machine.sleep(1)
        return seen

    def footer_height():
        """How many px from the bottom up differ from niri's background.

        The compositor's background is a flat color (`#404040` by default);
        our surface is another. Counting the lines that differ from it
        measures the surface's REAL height on screen, which is the only proof
        that the expansion was accepted — the log only proves it was asked for.
        """
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        # A column of pixels well to the left, from the top to the end of the window.
        raw = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop 1x{h}+2+0 +repage -depth 8 txt:-"
        )
        colors = []
        for line in raw.splitlines()[1:]:
            if "#" in line:
                colors.append(line.split("#")[1][:6])
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



    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    def layers():
        """What the COMPOSITOR says about our layer surfaces.

        Asking for `exclusive` and getting `exclusive` are different things;
        only its side answers the second one.
        """
        out_text = machine.succeed(f"${ronoctua-shell}/bin/niri-contract {sock}")
        for line in out_text.splitlines():
            if line.startswith("layers:"):
                return line
        raise AssertionError(f"niri-contract did not list layers:\n{out_text}")


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("the footer comes up at rest, with two lines"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # HOME, because the file manager opens in it and `systemd-run` does
            # not inherit one. Without this it fell back to `/`, whose listing
            # is long enough to sit at the region's ceiling — and a listing at
            # the ceiling measures the same whatever you do to it, so the
            # pixels stop answering the question being asked.
            "--setenv=HOME=/root "
            # pt-BR on purpose, and not the default: that way the list on screen
            # proves the whole chain of invariant 8 — `.ftl` -> generator ->
            # constant -> Fluent -> pixel — in the language that is NOT the fallback.
            "--setenv=LANG=pt_BR.UTF-8 "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            # THE LEADER'S LABEL comes from the environment, put there by whoever
            # defines the bind. Here it is the test that defines it, and that is why
            # it can assert the exact text — without the variable it would fall back
            # to `SPC` and the test would pass without proving the variable is read.
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        # Wait for the PAINTING, not for the control socket.
        #
        # Both happen at startup, but in an order that depends on how long the
        # process takes to come up — and it changed when the binary gained the
        # WASM runtime. Measuring the height right after the socket measured a
        # surface that had not received a `configure` yet: zero pixels, with no error.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'footer layout resolved'",
            timeout=90,
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'control socket opened'",
            timeout=30,
        )
        at_rest = wait_for_height(FOOTER)
        print(f"height at rest: {at_rest} px")

    with subtest("the leader captures the keyboard and expands"):
        # Without this, `xdotool` injects keys that reach nowhere at all.
        focus_niri()

        # THE POINTER TO A CORNER THAT DOES NOT GET IN THE WAY — and there are TWO nuisances.
        #
        # 1. The message line started showing the description of the item UNDER THE
        #    CURSOR. The Xvfb pointer is born in the middle of the screen and the
        #    expansion grows upwards close to it: with a long list, the footer would
        #    reach the cursor and the minibuffer would show a description in place of
        #    the pending sequence.
        # 2. The `footer_height` samples COLUMN x=2 and takes the pixel at the top
        #    as "background". With the cursor drawn there, the background becomes the
        #    cursor's color and the arithmetic gives back the whole window — which is
        #    what happened: 775 px of footer on a surface of 288.
        #
        # The upper RIGHT corner, inside the window: far from the sampled column and
        # far from the footer.
        w0, h0, x0, y0 = niri_geometry()
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x0 + w0 - 5} {y0 + 5}"
        )

        # `ronoctua-shell leader` does NOT bring a footer up: it notifies the one
        # already running. It is the path niri's bind uses (`nix/modules/niri.nix`).
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader active'", timeout=30
        )

        expanded = footer_height_above(FOOTER)
        print(f"height with the leader: {expanded} px")
        # THE FIRST LEVEL, one line per row, below the footer's two. What is
        # asserted is that the surface GREW — the exact number of rows is the
        # keymap's business and moves with it, and a count written here was
        # stale within a month (it said four groups, named `g`, which has not
        # existed for a long time, and `b`, which is now the brain and not the
        # buffer).
        assert expanded > FOOTER, (
            f"the leader did not expand the surface: {expanded} px against {FOOTER} px at rest"
        )

        journal = machine.succeed("journalctl -u footer --no-pager")
        # THE INVARIANT, and not the count: a nailed-down number breaks on every new
        # command and teaches people to update the test without reading what it says.
        # What matters is that the leader opened with entries ON SCREEN.
        import re as _re
        n = [int(m) for m in _re.findall(r"inputs=(\d+)", journal)]
        assert n and n[0] > 0, f"the leader opened with no entries: {n}"
        top_px = n[0]

        # The milestone's literal criterion: the COMPOSITOR reports `exclusive`.
        c = layers()
        print(c)
        assert "ronoctua-footer@Top/Exclusive" in c, c

    with subtest("the which-key list appears on screen"):
        w, h, x, y = niri_geometry()
        alt = footer_height()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            # Only the part BELOW the footer's two lines.
            #
            # THE ORDER CHANGED (the owner's decision): modeline and minibuffer stay
            # GLUED at the top of the footer, and the expansion grows below them —
            # before it grew between the two. Cropping from the old place would read
            # the minibuffer and pass by mistake.
            f"-crop {w}x{alt - FOOTER}+0+{h - alt + FOOTER} +repage "
            "-colorspace gray -negate -resize 400% /tmp/whichkey.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/whichkey.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR of the list: {read_back!r}")
        # TRANSLATED TEXT, not a key. If `group-files` shows up here, the label
        # reached the screen raw and invariant 8 is broken at the last link — the
        # only one no unit test covers.
        lowered = read_back.lower()
        # `janela` — the `w` group's label, which the root list always has.
        # It was `buffer`, until the buffer commands moved under `w` and
        # the `b` prefix became the brain's.
        # pt-BR: the OCR needle is the VALUE of a text key on screen
        # (`group-window`), not a message of this test.
        assert "janela" in lowered, (
            f"the which-key list is not on screen in pt-BR; OCR read {read_back!r}"
        )
        assert "group-" not in lowered, (
            f"the label arrived as a KEY, not as text: {read_back!r}"
        )

    with subtest("a prefix key narrows the list"):
        # `h` AND NOT `i`. This pressed `i` for as long as the three browsers
        # lived at the top level; they moved under `m` (`m i`), `i` stopped
        # being a prefix, and pressing it did nothing at all — so the second
        # `inputs=` never appeared and this subtest had been red ever since,
        # in a battery nobody had run over the tree.
        #
        # `h` and not `b`: `b` today has as many bindings as the root, and
        # "it narrowed" would become a coincidence. What is asserted is that
        # the list became the PREFIX's, and `h` has one item.
        press_key("h")
        journal = machine.succeed("journalctl -u footer --no-pager")
        # What is asserted is that it NARROWED, not a number: how many commands
        # exist at the top changes with the keymap.
        import re as _re
        n = [int(m) for m in _re.findall(r"inputs=(\d+)", journal)]
        assert len(n) >= 2, f"the second `entries=` is missing: {n}"
        assert n[-1] < top_px, f"the list did not narrow: {n[-1]} vs {top_px} at the top"

    with subtest("the minibuffer shows the PENDING SEQUENCE, as in Doom"):
        # The defect this catches: the prefix only showed up in the modeline's mode
        # indicator — a narrow segment that ELIDES. Whoever types a chord looks at
        # the line below, and there was nothing there: pressing the leader and seeing
        # the same screen is indistinguishable from "it did not register the key".
        journal = machine.succeed("journalctl -u footer --no-pager")
        # `LEADER h-`: the label from the environment, and Doom's notation (`-`,
        # and not ` …`). `SPC` here would be the fallback, and would prove the
        # opposite.
        assert "LEADER h-" in journal, journal

        w, h, x, y = niri_geometry()
        # POLLED, like every other read of a height that has just changed:
        # `press_key` returns before the compositor has composed, and a 0 here
        # would put the crop's origin off the window and fail inside magick.
        alt = footer_height_above(FOOTER)
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        # THE FOOTER'S SECOND LINE, counting from its top — and not the last one
        # on the screen.
        #
        # The minibuffer became GLUED to the modeline (the owner's decision), and
        # the expansion moved below the two. With the list open, the screen's last
        # line is the LIST's last line; reading there would find a shortcut label
        # and the test would pass for the wrong reason.
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop {w}x${toString linePx}+0+{h - alt + ${toString linePx}} +repage "
            "-colorspace gray -negate -resize 400% /tmp/mini.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/mini.png stdout --psm 7 2>/dev/null"
        ).strip()
        print(f"OCR of the minibuffer: {read_back!r}")
        assert "leader" in read_back.lower(), (
            f"the pending sequence is not on the minibuffer's line; OCR read {read_back!r}"
        )

    with subtest("a key that leads nowhere SAYS so"):
        # Before: it ended silently. The user had no way of telling "that
        # combination does not exist" from "the key did not arrive".
        press_key("z")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'echo area'", timeout=30
        )
        journal = machine.succeed("journalctl -u footer --no-pager")
        echo_line = [l for l in journal.splitlines() if "echo area" in l][-1]
        print(f"echo: {echo_line}")
        # THE MESSAGE cites the whole sequence, not only the last key:
        # `LEADER h z` is what the user typed, and `z` on its own would not
        # help. The prefix is `h` because the subtest above pressed it — it
        # used to be `i`, which stopped being a prefix when the three browsers
        # moved under `m`.
        assert "LEADER h z" in echo_line, echo_line
        # And in pt-BR, translated — not the raw key.
        assert "echo-" not in echo_line, echo_line

    with subtest("Esc goes back to rest, two lines tall"):
        press_key("Escape")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader closed'", timeout=30
        )
        # And it gives the keyboard back: without this no buffer would receive a key
        # again, and the system would go mute after the first leader.
        c = layers()
        print(c)
        assert "ronoctua-footer@Top/None" in c, c

        came_back = wait_for_height(FOOTER)
        print(f"height after the Esc: {came_back} px")

    with subtest("the expansion STOPS growing, and the rest becomes a page"):
        # INVARIANT 14 IN PIXELS. The `M-x` list has more commands than fit, and with
        # no ceiling it would grow with the registry: every new command would push the
        # footer further up, until it stopped being a footer.
        #
        # What is asserted is the CRITERION — "the footer does not take the screen" —,
        # and not the token's number: repeating the formula here would give a test
        # that agrees with the code by construction and verifies nothing.
        press_key("Escape")
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader active'", timeout=30
        )
        # `colon` is the name of the `:` keysym in xdotool. The key changed from `x`
        # to `:` — Doom's `M-x` one —, and `x` there is the scratch buffer.
        press_key("colon")  # the finder: the longest list in the system
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'command=\"mx-open\"'",
            timeout=30,
        )
        _, h, _, _ = niri_geometry()
        with_mx = footer_height_above(FOOTER)
        print(f"height with M-x open: {with_mx} px of {h} px of screen")
        assert with_mx > FOOTER, "M-x did not expand"
        assert with_mx <= h / 2, (
            f"the expansion took {with_mx} px of {h} px — more than half the screen"
        )
        # THE MOUSE WHEEL SCROLLS THE REGION — with the `M-x` open, which is the
        # scrollbar's case (a long list, with no page counter).
        #
        # `click 4`/`click 5` are the wheel-up and wheel-down buttons on X. What is
        # asserted is the OFFSET, which is the number the bar and the counter come
        # out of: a test of the handler would say that the event arrived, and not
        # that the screen moved.
        def offset():
            import re as _re

            journal = machine.succeed("journalctl -u footer --no-pager")
            seen = _re.findall(r"offset=(\d+)", journal)
            assert seen, "the footer never recorded an offset"
            return int(seen[-1])

        # THE POINTER HAS TO BE OVER THE FOOTER.
        #
        # On Wayland the axis event goes to the surface UNDER the cursor, and it is
        # parked in the upper right corner (see the comment up there, about
        # `footer_height`). Without moving it, the wheel would scroll the buffer
        # behind and the test would start measuring something else — or nothing.
        wv, hv, xv, yv = niri_geometry()
        footer_band = footer_height()
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{xv + wv // 2} {yv + hv - footer_band // 2}"
        )
        machine.sleep(1)

        prior = offset()
        for _ in range(3):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool click 5")
            machine.sleep(1)
        machine.sleep(1)
        after = offset()
        assert after > prior, (
            f"the wheel down did not scroll the region: offset {prior} -> {after}"
        )

        # AND UP GOES BACK. Without this, a swapped sign would pass: scrolling both
        # ways would give the same error in half the cases.
        for _ in range(3):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool click 4")
            machine.sleep(1)
        machine.sleep(1)
        came_back = offset()
        assert came_back < after, (
            f"the wheel up did not come back: offset {after} -> {came_back}"
        )

        # IT PUTS THE CURSOR BACK IN THE CORNER. Leaving it over the footer would
        # spoil the `footer_height` of the following subtests, for the reason the
        # parking comment explains.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{xv + wv - 5} {yv + 5}"
        )
        machine.sleep(1)

        press_key("Escape")
        wait_for_height(FOOTER)

    with subtest("in the file manager, TYPING narrows the list"):
        # THE DEFECT THIS CATCHES, and it was reported from the screen: the
        # list took a letter as a command and not as text. `j` moved, `s`
        # sorted, and typing a file's name did everything except look for it.
        #
        # The unit tests cannot answer this one. They press a `Key` straight
        # into `View::key` and prove the view does the right thing WITH the
        # key; what is asked here is whether the key ARRIVES — through Xvfb,
        # through niri, through the layer surface, with the leader's own
        # modifiers discounted. That is three of the causes `AGENTS.md` lists
        # under "nenhum atalho funciona", and they all look the same on screen.
        # FEW FILES, ON PURPOSE. The region has a ceiling of twelve lines, and a
        # listing that hits it measures the same before and after — the height
        # would stop answering the question. With four visible and three hidden
        # (`.cache` and `.nix-channels` are already there), every state of this
        # subtest is below the ceiling and the pixels say which one it is in.
        machine.succeed(
            "rm -f /root/alpha-*.txt /root/zzz-only.txt /root/.hidden-test; "
            "for n in 1 2 3; do echo x > /root/alpha-$n.txt; done; "
            "echo x > /root/zzz-only.txt; echo x > /root/.hidden-test"
        )
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader active'", timeout=30
        )
        # TWICE, because `.` is a GROUP now and not a command.
        #
        # It opened the manager directly and also hung off a group called `f`,
        # so one thing was reachable by three sequences; the owner asked for the
        # one shape and `.` became the prefix. The first press opens the group's
        # list, the second is the footer's door — the doubling `s n n`, `t t`
        # and `n n` already keep.
        #
        # A prefix cannot also be a command (the registry refuses it), so this
        # is not a second way of saying the same thing: one press no longer
        # reaches a manager at all.
        press_key("period")
        press_key("period")

        # THE LIST IS ON SCREEN when the region is taller than the two lines at
        # rest. Measured and not assumed: the log proves it was asked for, and
        # the pixels prove niri agreed.
        opened = footer_height_above(FOOTER)
        assert opened is not None and opened > FOOTER, (
            f"the manager did not expand: {opened} px, at rest it is {FOOTER}"
        )

        # AND NOW THE LETTERS, with no key before them.
        #
        # THE HEIGHT STOPPED ANSWERING THIS, and it is a decision and not a
        # regression: the preview and the facts are on from the moment the
        # manager opens, and a sheet is ten-odd rows whatever the listing is
        # doing — so the region sits at its ceiling with four files and with
        # one. See `files::View::panel_lines`, which argues it where it is
        # decided.
        #
        # What this subtest was ever asking is whether the LETTER ARRIVED as
        # text rather than as a command, and that is read off the SCREEN. The
        # assertions below already did exactly that; the height was the proxy,
        # and the proxy is the part that went.
        def type_and_settle(word):
            for letter in word:
                press_key(letter)
            machine.sleep(2)

        # THE FIRST ONE IS WHAT THE `Esc` BELOW CLEARS, and it has to be here:
        # `Esc` on an empty query closes the view, and then every height read
        # after it is the footer at rest.
        type_and_settle("zzz")

        # AND IT MATCHED, rather than emptying the list.
        #
        # Narrowing to NOTHING also makes the region shorter, so "it got
        # shorter" alone would pass on a filter that matches nothing — which is
        # exactly what happened while this test was being written, with the
        # manager opening somewhere else entirely.
        #
        # THIS USED TO COMPARE HEIGHTS, and it cannot any more. A query that
        # matched nothing used to drop the path line and the header along with
        # the rows, which made the region one line tall — and took the QUERY off
        # the screen with them, because the query lives at the end of the path.
        # Whoever mistyped a letter could no longer see what they had typed. So
        # "nothing here" became a line of the LISTING, beside the path and the
        # header that name it, and both queries now leave the region the same
        # height. The question was never about height: it is whether the file is
        # ON SCREEN, and that is what is asked here.
        press_key("Escape")
        wait_for_height(opened)
        type_and_settle("zzz")
        with_match = expansion_text()
        press_key("Escape")
        wait_for_height(opened)
        type_and_settle("qqq")
        without = expansion_text()
        print(f"OCR with `zzz`: {with_match!r}\nOCR with `qqq`: {without!r}")
        assert "zzz" in with_match.lower(), (
            f"`zzz` matches `zzz-only.txt` and the file is not on screen: {with_match!r}"
        )
        assert "zzz-only" not in without.lower(), (
            f"`qqq` matches nothing and the list went on showing: {without!r}"
        )
        assert "alpha" not in with_match.lower(), (
            f"`zzz` left the `alpha-*.txt` on screen: the filter did not narrow: {with_match!r}"
        )
        press_key("Escape")
        wait_for_height(opened)
        type_and_settle("zzz")

    with subtest("Esc clears what was typed BEFORE closing the view"):
        # Two things in one press would be one too many: throwing away the query
        # AND the view leaves whoever mistyped a letter with nothing on screen.
        press_key("Escape")
        widened = wait_for_height(opened)
        assert widened == opened, widened

    with subtest("and a command of the view still exists, one modifier along"):
        # `C-h` and not `h`, because `h` is now a letter of the query — and not
        # `M-h` either, which is the whole of the second lesson: in the nested
        # loop the leader's chord family IS `Alt`, so the compositor eats every
        # `M-…` before the footer sees it. Reported from the screen as "M-RET
        # opens the terminal".
        #
        # This is also the end-to-end proof of the modifier inheritance: the
        # manager is opened with a leader that HOLDS Ctrl, and until that was
        # fixed the discount lasted the whole session — every `C-…` arrived as
        # the bare letter and was typed into the query instead.
        #
        # READ OFF THE SCREEN and not off the height, for the reason the
        # subtest above gives: the panels hold the region at its ceiling, so
        # showing ten more files no longer moves it. What is being asked here
        # is whether `C-h` arrived as a COMMAND — and the answer is a hidden
        # file appearing, which is the thing itself rather than a proxy for it.
        before = expansion_text()
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+h")
        machine.sleep(2)
        after = expansion_text()
        print(f"OCR before the C-h: {before!r}\nOCR after: {after!r}")
        # THE FLIP, and not a direction: which way `C-h` goes depends on what
        # the dotfiles were doing when the manager opened, and the question is
        # whether the key ARRIVED. Naming the direction here would make this
        # test a second declaration of the default.
        was_there = "hidden" in before.lower()
        is_there = "hidden" in after.lower()
        if was_there == is_there:
            print("ls -a:", machine.execute("ls -a /root | tr '\n' ' '")[1])
        assert was_there != is_there, (
            f"`C-h` did nothing: `.hidden-test` {'stayed' if was_there else 'stayed out'} "
            f"on screen. Either the Ctrl was discounted, or the key became a filter letter: {after!r}"
        )
        # AND WITH NOTHING TYPED, `Esc` closes — a query on screen it would
        # clear first, and there is none here.
        press_key("Escape")
        wait_for_height(FOOTER)

    with subtest("a complete sequence fires the COMMAND"):
        # Section 13 wants the key to reach a named command, not an anonymous
        # function. The id in the log is the proof that the whole path — key, tree,
        # registry — closed.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader active'", timeout=30
        )
        # `m i n`, and not `i n`: the three browsers moved under MISCELLANEOUS
        # and `i` stopped being a top-level prefix. The old sequence reached
        # nothing, silently.
        press_key("m")
        press_key("i")
        press_key("n")
        # The QUOTES are part of it: `tracing` formats a `Debug` value between
        # quotes, and `command=glyphs-nerd-fonts` matches nothing.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'command=\"glyphs-nerd-fonts\"'",
            timeout=30,
        )
        # And the keyboard goes back to the buffers on its own after running.
        #
        # Through the COMPOSITOR, and not through `wait_for_height`: that helper
        # measures by counting pixels that differ from the background, and from here
        # on there is a WINDOW covering the screen — the "background" it samples
        # becomes the buffer, and the arithmetic gives zero. The question that
        # matters is another one: did the keyboard come back? Only niri answers that.
        c = layers()
        print(c)
        assert "ronoctua-footer@Top/None" in c, c

    with subtest("the command really OPENS the glyph browser"):
        # THE DEFECT THIS CATCHES: five commands showed up in which-key, "ran", and
        # fell into a log `warn`. On screen, nothing. Firing the command proves
        # nothing — what proves it is the WINDOW existing.
        try:
            machine.wait_until_succeeds(
                f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 1'",
                timeout=90,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -30")[1])
            print(machine.execute("systemctl list-units 'ronoctua-buffer-*' --no-pager")[1])
            raise
        # And the `warn` for a command with no action must NOT have shown up: it is
        # the exact symptom of the defect.
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert "a command with no registered action" not in journal, journal

    with subtest("the opened buffer ANNOUNCES its state, and the modeline shows it"):
        # THE HALF OF THE STATE CONTRACT THAT ONLY A COMPOSITOR REACHES.
        #
        # The buffer publishes on a socket named by its own pid, the footer is
        # told on every change and ASKS on every change of focus. Every piece is
        # unit-tested apart — the line protocol in `leader.rs`, the round trip
        # through the WIT in `states-driven`, the `Esc` layer in `buffer.rs` —
        # and none of those has a compositor telling one process about another
        # process's window.
        #
        # It is here and not in `modeline-vm` because this is the test that opens
        # a buffer WINDOW: doing it there would mean that VM growing a GPU stack
        # to assert something this one is already set up for.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'id=\"state\"'",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u footer --no-pager")
        state = [l for l in journal.splitlines() if 'id="state"' in l]
        print(f"state: {state[-1]}")
        # BY ITS TEXT AND NOT BY THE ID. What the segment carries is what the
        # i18n resolved — `normal` reads the same in both languages, and it is
        # the base state every buffer starts in. Asserting the id would pass
        # with the key drawn raw, which is the defect the whole `state-<id>`
        # arrangement exists to prevent.
        assert "text=normal" in state[-1], state[-1]

    with subtest("the SYMBOL browser opens too, and it is another window"):
        # THE DEFECT THIS CATCHES is the same as the subtest above, and that is why
        # it exists again for the second buffer: a command can show up in which-key,
        # "run", and fall into a log `warn`. On screen, nothing.
        #
        # The assertion is about the COUNT: the glyph browser has already opened a
        # window, so after this there have to be TWO. Waiting for "a window exists"
        # would pass with the second command doing nothing.
        #
        # BY NAME AND NOT BY `m i u`, and the reason is a collision this test found:
        # `ronoctua_protocol::LOCAL_PREFIX` is `"m"`, so at the ROOT of a sequence
        # `LEADER m` belongs to the FOCUSED BUFFER's own keymap whenever that buffer
        # published one — and the tree's MISCELLANEOUS group, which is where the
        # three browsers live, is unreachable from any of our buffer windows.
        # Measured here: the footer received the `m` (`the key after the discount
        # … Char('m')`), handed it to the buffer, which redrew, and the leader
        # never narrowed.
        #
        # The subtest above still proves the KEY path, because it runs before any
        # buffer window exists. This one proves the second WINDOW, which is what it
        # says it is about, and it reaches the command the way `LEADER :` does.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell command unicode-symbols"
        )
        # WITHOUT THE QUOTES, and that is not a detail. The subtest above waits
        # for `command="glyphs-nerd-fonts"` because the LEADER logs a `Debug`
        # value, which `tracing` puts between quotes. The control socket logs a
        # `&str`, which it does not. Grepping for the quoted form here waited
        # thirty seconds for a line that had already gone past.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager "
            "| grep -q 'command through the control socket command=unicode-symbols'",
            timeout=30,
        )
        try:
            machine.wait_until_succeeds(
                f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 2'",
                timeout=90,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -30")[1])
            raise
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert "a command with no registered action" not in journal, journal

    with subtest("with the Alt HELD the sequence goes on working"):
        # THE DEFECT THIS CATCHES, and that only shows up in real use.
        #
        # In the nested session the leader IS `Ctrl+Alt+Space`: when the sequence starts,
        # Ctrl and Alt are pressed because the hand has just pressed them. Every key came out
        # with the modifier along, nothing matched, and the screen said
        # `CTRL ALT SPC M-n is not bound` — which names the consequence and hides the
        # cause. The same state happens WITHOUT the hand: leaving for a host shortcut
        # and coming back makes the nested niri miss the Alt release and announce it
        # forever.
        #
        # The normal `press_key()` does not catch this: `xdotool key` releases the key,
        # and between one and the next the Alt has already gone up. Here it stays
        # down the whole time, which is the only way of reproducing what the owner saw.
        def xdo(*args):
            machine.succeed(
                "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool " + " ".join(args)
            )

        machine.succeed("journalctl --rotate && journalctl --vacuum-time=1s")
        # AND THE FOCUS ONCE MORE, BEFORE THE LEADER, for the reason written
        # above the second browser sequence — including the order.
        focus_niri()
        xdo("keydown", "alt")
        try:
            machine.succeed(
                "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
                "${ronoctua-shell}/bin/ronoctua-shell leader"
            )
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep -q 'leader active'", timeout=30
            )
            # `h` AND NOT `m`, for the collision written above the subtest
            # before this one: two buffer windows are open by now, and at the
            # root of a sequence `m` is the FOCUSED BUFFER's prefix, not the
            # tree's. `h` is a group of the tree and reaches it.
            xdo("key", "h")
            machine.sleep(1)
        finally:
            # In the `finally` on purpose: an Alt stuck in Xvfb would spoil any
            # subtest that came afterwards, including in a future where this is
            # no longer the last one.
            xdo("keyup", "alt")

        journal = machine.succeed("journalctl -u footer --no-pager")
        # THE PROOF: `h` opened the group. With the defect, the log carries
        # `is not bound` and the pending sequence never reaches `LEADER h-`.
        assert "LEADER h-" in journal, (
            "with the Alt held, `LEADER h` did not open the help group.\n"
            "The key came out as `M-h` and matched nothing.\n\n" + journal
        )
  '';
}
