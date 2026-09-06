# The modeline reflects the focused buffer — acceptance criterion of milestone 4.
#
# "The daemon consumes the event stream; the modeline reflects the focused
# buffer in a screenshot with OCR."
#
# WHY OCR, AND NOT ONLY THE LOG
# -----------------------------
# The log proves that the daemon RECEIVED the event and that the layout was
# resolved with the right text. It does not prove that the text reached the
# screen: an error of color, of atlas, of coordinate or of shaping would leave
# the modeline empty with the log intact. They are two different links, and
# the milestone requires both.
#
# HOW THE SCREEN IS CAPTURED
# --------------------------
# Through X, with `xwd -root`, and not through `wlr-screencopy`. niri runs on
# the winit backend inside Xvfb (see `niri-xvfb.nix`), so the winit window
# ALREADY contains everything it composes — including our layer-shell surface.
# Capturing X's root is one call, with no capture protocol in the middle.
#
# The OCR is done only on the FOOTER'S BAND, enlarged. Tesseract on 14 px text
# is not very reliable; on the cropped and enlarged band, with `--psm 7` (a
# single line), it reads well. And the assert is about the focused
# application's name — a short lowercase word, OCR's easy case — and not about
# the whole line.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  # THE HEIGHT COMES FROM THE TOKENS, not from a nailed-down number. It used
  # to be `35`, from when the font was 14 px and the line height 1.25 — both
  # changed, and the test started measuring against a value that no longer exists.
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = builtins.ceil (typography.size.base * typography.lineHeight * 2);

  # The test client. `foot` draws through `wl_shm`, so it needs no GPU at all —
  # the only requirement that matters here. Its `app_id` is `foot`, which is
  # what the modeline has to show in the `major-mode`.
  client = pkgs.foot;
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-modeline";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      client
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


    # The footer's height in px: two lines of 17.5 (section 4.1), rounded.
    FOOTER = ${toString footerPx}


    def modeline_ocr():
        w, h, x, y = niri_geometry()
        print(f"niri's window: {w}x{h}+{x}+{y}")
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")

        window = f"-crop {w}x{h}+{x}+{y} +repage"
        # Geometry in PIXELS, never `100%x35`: in ImageMagick the `%` applies
        # to the WHOLE geometry, so `100%x35` is 100% of the width by 35% of
        # the HEIGHT — 280 px instead of 35. The crop comes out ten times
        # bigger and takes the windows along, with no error at all.
        band = f"{window} -crop {w}x{FOOTER}+0+{h - FOOTER} +repage"

        # Diagnosis BEFORE the OCR, always. `cores=1` says "uniform band,
        # nothing drawn" and immediately separates the two causes of an empty
        # OCR: a wrong crop or an absent drawing. Without this each hypothesis
        # costs a whole VM round.
        print("the footer's band: " + machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd {band} "
            "-format 'cores=%k media=%[fx:mean]' info:"
        ))

        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd {band} "
            # Only the band's top line: the bottom one is the minibuffer, empty
            # at rest, and it goes into the OCR as noise.
            f"-crop {w}x18+0+0 +repage "
            # Enlarge before binarizing: tesseract makes many mistakes at 14 px
            # and gets it right on the same image 4x bigger.
            "-colorspace gray -negate -resize 400% "
            "/tmp/modeline.png"
        )
        return machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/modeline.png stdout --psm 7 2>/dev/null"
        ).strip()


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")

    with subtest("the footer comes up connected to niri's IPC"):
        sock = socket_of("niri")
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # HOME, because the brain the footer wakes reads the person's
            # preferences from under it. Without one there is no user config at
            # all, the update task keeps its default afternoon window, and this
            # test measured a three-line footer between 13h and 18h.
            "--setenv=HOME=/root "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=trace "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'footer layout resolved'",
            timeout=60,
        )

        # Without NIRI_SOCKET the daemon degrades instead of failing — correct
        # behavior, but one that would make this test pass without testing
        # anything. This assert is what guarantees that the IPC did connect.
        assert "no niri IPC" not in machine.succeed(
            "journalctl -u footer --no-pager"
        ), "the footer did not connect to the IPC; NIRI_SOCKET did not arrive"

    with subtest("with no window, the modeline does NOT invent a buffer"):
        # The initial state matters: it is what shows up when the compositor comes up.
        #
        # There used to be a `*scratch*` there, inherited from Emacs — where the
        # scratch is a REAL buffer. Here there is nothing behind it: the modeline
        # announced an open buffer that was never implemented. With no window,
        # the segment disappears — and with nothing unread the bell is not
        # there either, so the line is EMPTY. Nothing at rest is the owner's
        # rule for this line, and this is where it is measured.
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert "scratch" not in journal and "rascunho" not in journal, journal
        assert "segments=0" in journal, journal

    with subtest("opening a window changes the modeline"):
        machine.succeed(
            "systemd-run --unit=client ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # An ABSOLUTE path to the command: the systemd-run unit does not
            # inherit PATH, and `sh` on its own fails with "No such file or
            # directory" — an error that looks like foot's and is not.
            "${client}/bin/foot --title=ronoctua-teste "
            "${pkgs.coreutils}/bin/sleep 600"
        )
        try:
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep -q 'text=ronoctua-teste'",
                timeout=60,
            )
        except Exception:
            # The decisive question: did the COMPOSITOR see the window? If it
            # reports zero, the problem is the client; if it reports one, the
            # problem is ours. Without this the two hypotheses stay tied.
            print("--- what niri says exists ---")
            print(machine.execute(
                f"${ronoctua-shell}/bin/niri-contract {sock}")[1])
            print("--- client ---")
            print(machine.execute("systemctl status client --no-pager")[1])
            print(machine.execute("journalctl -u client --no-pager")[1])
            print("--- niri ---")
            print(machine.execute("journalctl -u niri --no-pager")[1])
            print("--- footer ---")
            print(machine.execute("journalctl -u footer --no-pager")[1])
            raise

        # The `major-mode` comes from the app id, not from the title — they
        # are different fields of the same event, and swapping one for the
        # other would pass the assert above.
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert 'id="mode" column=' in journal and "text=foot" in journal, journal

    with subtest("what is ON SCREEN is the focused buffer's modeline"):
        # The link the log does not cover: color, atlas, coordinate and shaping.
        read_back = modeline_ocr()
        print(f"OCR: {read_back!r}")

        # `foot` is the focused buffer's app id. A short lowercase word: the
        # case where the OCR is reliable. Asserting the whole line would be
        # asserting tesseract's quality, not the footer's.
        assert "foot" in read_back.lower(), (
            f"the modeline on screen does not show the focused buffer.\n"
            f"OCR leu: {read_back!r}\n"
            f"If the log shows the right text and this fails, the problem is in\n"
            f"drawing — not in the daemon."
        )

    with subtest("a FOREIGN window gets no state segment"):
        # THE SEGMENT THE BRANCH LEFT BEHIND. The context segment used to be the
        # git branch of the focused window's process, read by walking `/proc`
        # from the pid the compositor gives; it was removed because in
        # production it never even appeared for a buffer — the footer's unit has
        # no `WorkingDirectory`, so its children start in `$HOME`.
        #
        # What is in its place is the STATE, and this is the half of it that
        # belongs here: `foot` is up from the subtest above and it is FOREIGN,
        # so there must be no state segment at all. That is the decision and not
        # an omission — the compositor gives a title, an app id and a pid, and
        # none of the three says what `vim` is in the middle of.
        #
        # The other half — one of OUR buffers showing its state — lives in
        # `leader-vm`, which is the test that opens a buffer window. Opening one
        # here would mean this VM growing a GPU stack to assert something that
        # one already has the setup for.
        journal = machine.succeed("journalctl -u footer --no-pager")
        lines = journal.splitlines()
        last_layout = max(
            i for i, l in enumerate(lines) if "footer layout resolved" in l
        )
        block = lines[last_layout:]
        assert not any('id="state"' in l for l in block), (
            "a foreign window got a state: niri does not tell us what\n"
            "state `vim` is in, and writing `normal` would be asserting what\n"
            "nobody checked.\n" + "\n".join(block[:10])
        )

    with subtest("closing the window gives the modeline back to the empty state"):
        # Proof that the daemon's state SHRINKS, and does not only grow: an
        # `State` that only accumulates would pass everything above.
        machine.succeed("systemctl stop client")
        # Back to NO segment: with no focused window there is no name and no
        # mode to show, and with nothing unread there is no bell.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | tail -20 | grep -q 'segments=0'",
            timeout=60,
        )
  '';
}
