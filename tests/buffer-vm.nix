# A native buffer in a window of its own — acceptance criterion of milestone 7.
#
# "The first native buffer end to end with an `xdg_toplevel` of its own."
#
# WHAT ONLY THE VM PROVES
# -----------------------
# That the compositor sees a WINDOW, and not a region drawn by us. The
# difference matters: invariant 4 says the shell never reimplements window
# layout, and drawing buffers inside a surface of ours would be exactly that.
# We ask the compositor — the IPC's `Windows` lists the windows, and the buffer's has to
# be there, with our `app_id`.
#
# And the content has to be ON SCREEN: the component produces a `DisplayList`
# with a text key, the host translates it and draws. The OCR closes the whole
# chain — component, protocol, i18n, tokens, wgpu.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
  bufferExample,
  bufferFormat,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-buffer";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
      # `wl-paste` to READ the clipboard. The `wl-copy` that writes it arrives
      # through the store path inside the binary.
      pkgs.wl-clipboard
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;

    # FOR THE LAST SUBTEST, and only for it. The formatting buffer is the one
    # whose rows come off a BUS rather than out of an embedded table, so it is
    # the one where "the window opened and drew nothing" is a real outcome —
    # which is exactly what happened to the file manager's sidebar once, and was
    # found on somebody's screen instead of here.
    services.udisks2.enable = true;
    # A disk of its own, so there is something on the list that is not the VM's
    # root. Virtio, so the kernel calls it fixed — the drawing does not care.
    virtualisation.emptyDiskImages = [ 64 ];
  };

  testScript = ''
    ${xvfb.prelude}


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("the buffer comes up and the component renders"):
        machine.succeed(
            "systemd-run --unit=buffer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # pt-BR: the key the component sends has to come out TRANSLATED.
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            "${ronoctua-shell}/bin/ronoctua-shell buffer "
            "${bufferExample}/lib/ronoctua_buffer_example.wasm"
        )
        try:
            machine.wait_until_succeeds(
                "journalctl -u buffer --no-pager | grep 'component rendered' > /dev/null",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u buffer --no-pager")[1])
            raise

        journal = machine.succeed("journalctl -u buffer --no-pager")
        # A FULL FRAME: the note, the list's six items and the form's two
        # fields — the reference app for several keyboards. The title is not
        # counted: it is the window's header, which the distro ships hidden
        # (`Screen::heading` draws nothing at row 0 under `hide_header`). A
        # floor and not a count, because the number is the app's to change;
        # what it guards is a `render` that came back EMPTY, with everything
        # below then passing over a blank window.
        import re as _re
        drawn = [int(n) for n in _re.findall(r"component rendered commands=(\d+)", journal)]
        assert drawn and max(drawn) >= 10, journal

    with subtest("the COMPOSITOR sees a window, not a region of ours"):
        # The proof of invariant 4. A buffer drawn inside our surface would not
        # show up here.
        await_windows(sock, 1, timeout=30)
        # AND IT HAS THE FOCUS.
        #
        # READ, never `ronoctua-contract`: that binary EXERCISES the compositor
        # (it turns outputs off and on and reloads the configuration), so using
        # it to ask a question makes the question change the answer — and in
        # this VM it exits non-zero doing it. The assertion also used to look
        # for the word `Some`, which is how the previous adapter spelt an
        # `Option` and which this one never prints: it would have passed on a
        # compositor focusing nothing at all, for ever.
        who = focused_now(sock)
        assert who is not None, "the compositor focuses no window at all"
        print(f"focused: {who}")

    with subtest("the buffer drew in the window"):
        machine.wait_until_succeeds(
            "journalctl -u buffer --no-pager | grep 'buffer drawn on the xdg_toplevel' > /dev/null",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u buffer --no-pager")
        for bad in ["drawing the frame failed", "preparing the text failed", "panicked"]:
            assert bad not in journal, f"the buffer reported: {bad}"

    with subtest("the component's key came out TRANSLATED on screen"):
        # The whole chain: component -> DisplayList -> i18n -> tokens -> wgpu ->
        # pixel. The needle is a BODY row, never the title: the title
        # (`example-title`) is the window's header, which the distro ships
        # hidden — `Screen::heading` draws nothing at row 0 under
        # `hide_header` — so a needle there asks for a row that is not drawn.
        # A row of the list is drawn with the header shown or hidden, so the
        # verdict does not hang on that default.
        w, h, x, y = window_box(sock, "br.ronoctua.example")
        # THE WINDOW'S OWN RECTANGLE, asked of the compositor.
        #
        # Whoever decides where a window goes is the compositor (invariant 4),
        # so a test may not assume it either. Cropping the whole OUTPUT was the
        # old way of not assuming, and it stopped working when a layout put the
        # window in a column: half the crop became empty desk and
        # `tesseract --psm 6`, which wants one uniform block of text, read
        # nothing. `window_box` reads the rectangle instead — see the prelude.
        #
        # READ UNTIL IT SAYS IT, at most fifteen times one second apart, so a
        # frame the compositor has not shown yet on a loaded machine cannot
        # change the verdict. TWO needles joined by OR, both rows of the list:
        # the row under the cursor's highlight band is the one tesseract may
        # drop, and both are pt-BR only, so an English fallback still fails.
        # pt-BR: the OCR needles are VALUES of text keys on screen
        # (`example-item-six`, `example-item-four`), not messages of this test.
        NEEDLES = ("madrugada", "anoitecer")
        read_back = ""
        for _ in range(15):
            machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
                "-colorspace gray -negate -resize 200% /tmp/buffer.png"
            )
            # DIAGNOSIS BEFORE THE OCR, the shape `modeline-vm` uses: `cores=1`
            # says "uniform image, nothing drawn", which separates a wrong crop
            # from an absent drawing — and from a tesseract that drifted on a
            # rolling nixpkgs — in one run.
            print("the example window: " + machine.succeed(
                "${pkgs.imagemagick}/bin/magick /tmp/buffer.png "
                "-format 'cores=%k mean=%[fx:mean]' info:"
            ))
            read_back = machine.succeed(
                "${pkgs.tesseract}/bin/tesseract /tmp/buffer.png stdout --psm 6 2>/dev/null"
            ).strip()
            print(f"OCR: {read_back!r}")
            if any(n in read_back.lower() for n in NEEDLES):
                break
            machine.sleep(1)
        assert any(n in read_back.lower() for n in NEEDLES), (
            f"the key did not come out translated on screen; OCR read {read_back!r}"
        )

    with subtest("COPY really copies, into the system's clipboard"):
        # THE DEFECT THIS CATCHES, and that nobody saw for months.
        #
        # `Effect::Copy` reached the host and became a `tracing::info!`. The
        # glyph browser's nine copy formats NEVER put anything anywhere, and the
        # feature looked finished — the worst failure mode this project knows.
        # The browser is the native icons app now (`crate::icons`), and the
        # promise is the same: a copy on its page reaches the clipboard.
        #
        # The criterion is the CLIPBOARD, and not the call: a test that asserted
        # "the wl-copy was invoked" would repeat the same defect one level up.
        machine.succeed("systemctl stop buffer")
        machine.succeed(
            "systemd-run --unit=icons ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell icons nerd-fonts"
        )
        machine.wait_until_succeeds(
            "journalctl -u icons --no-pager | grep 'icons: the tables arrived' > /dev/null",
            timeout=90,
        )

        # With no WM on Xvfb nobody has input focus, and the key would be lost.
        focus_compositor()
        machine.sleep(1)
        # `RET` opens the page of the glyph under the cursor, whose cursor
        # starts on its size; `↓` is the first copy format — the glyph's NAME —
        # and `RET` there copies it.
        for key in ("Return", "Down", "Return"):
            machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {key}")
            machine.sleep(2)

        def pasted_now():
            return machine.succeed(
                "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
                "${pkgs.wl-clipboard}/bin/wl-paste --no-newline 2>/dev/null || true"
            ).strip()

        try:
            machine.wait_until_succeeds(
                "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
                "${pkgs.wl-clipboard}/bin/wl-paste --no-newline | grep -q -- '-'",
                timeout=30,
            )
        except Exception:
            print(machine.execute("journalctl -u icons --no-pager | tail -40")[1])
            raise
        pasted = pasted_now()
        print(f"clipboard: {pasted!r}")
        # A Nerd Fonts glyph NAME: `<set>-<thing>`. Which one is not nailed down —
        # the order of the list with no search depends on the database, and nailing
        # the name down would make the test fail on a font update for the wrong reason.
        assert "-" in pasted and pasted.isascii() and len(pasted) > 3, (
            f"what was pasted does not look like a glyph name: {pasted!r}"
        )

    with subtest("Ctrl+V reads the clipboard into the icons app's search"):
        # The other side of the same thing. `Ctrl+V` fell into a Ctrl guard and
        # did NOTHING once — and there was no way of telling that from "there
        # was nothing copied". What is asserted is the DOOR's end: it really
        # read the clipboard and handed the text to the view's search
        # (`icons::View::pasted`, which its unit tests follow from there).
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Escape")
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+v")
        machine.sleep(2)

        journal = machine.succeed("journalctl -u icons --no-pager")
        assert "icons: the clipboard went into the search" in journal and pasted in journal, (
            f"`Ctrl+V` did not hand {pasted!r} to the search; the log:\n" + journal[-2000:]
        )
        print(f"Ctrl+V handed over: {pasted!r}")

    with subtest("the format buffer draws disks that came from the BUS"):
        # THE ONE CHAIN NO OTHER TEST CLOSES, and the reason it is worth a
        # subtest of its own: every other buffer draws something it carries —
        # a glyph table, a counter. This one draws what udisks said, a moment
        # ago, over D-Bus. Between the two there is a capability the host has to
        # honour, a thread it has to raise, a record it has to lower field by
        # field, and a sentence it has to word in the person's language.
        #
        # `format-driven` proves the component's half against a rack handed to
        # it. What it cannot prove is that a rack ever ARRIVES — a capability
        # the host forgot to read comes back as an empty list, the screen says
        # "no disk here to format", and nothing anywhere reports an error. That
        # is the shape of the defect that reached the owner's screen last time.
        machine.succeed("systemctl stop icons || true")
        machine.succeed("systemctl stop buffer || true")
        machine.sleep(1)

        machine.succeed(
            "systemd-run --unit=format ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_disks=debug "
            "${ronoctua-shell}/bin/ronoctua-shell buffer "
            "${bufferFormat}/lib/ronoctua_buffer_format.wasm"
        )
        machine.wait_until_succeeds(
            "journalctl -u format --no-pager | grep 'buffer drawn on the xdg_toplevel' > /dev/null",
            timeout=60,
        )
        # The rack has to have crossed. Before it does, the screen legitimately
        # says it has nothing — so waiting for the DRAW alone would measure the
        # empty state and pass.
        machine.wait_until_succeeds(
            "journalctl -u format --no-pager | grep 'input=Rack' > /dev/null",
            timeout=60,
        )
        machine.sleep(2)

        journal = machine.succeed("journalctl -u format --no-pager")
        # THE THIRD NEEDLE WAS `sem udisks2`, WHICH EXISTS NOWHERE. It was
        # Portuguese, and it was also a guard that could never fire: the
        # sentence the host actually writes when the daemon is absent is
        # `udisks2 is off the bus` (`ronoctua_disks::Manager::read`). A needle
        # nothing can produce is a subtest that passes by construction.
        for bad in ["drawing the frame failed", "panicked", "udisks2 is off the bus"]:
            assert bad not in journal, f"the format buffer reported: {bad}"

        # WHAT THE READING CARRIED, ASKED OF THE JOURNAL AND NOT OF THE PIXEL.
        #
        # The split is deliberate and it was learned here. The OCR below cannot
        # tell "this row was not drawn" from "this row was drawn and tesseract
        # could not read it" — with two rows and a highlight band under one of
        # them it drops a line, and an assertion that cannot tell those apart is
        # an assertion that will one day be wrong in whichever direction is
        # convenient.
        #
        # So the CONTENT is checked where it is exact, and the pixel is asked
        # only what only it can answer.
        rack = [ln for ln in journal.splitlines() if "input=Rack" in ln]
        assert rack, "no disk reading reached the component"
        last = rack[-1]
        # THE MACHINE'S OWN ROOT IS ON THE LIST, carrying the sentence that says
        # why it is not on offer. Both halves matter: omitted, somebody hunts
        # for a row that is not there; offered without the reason, the only
        # signal left is a colour.
        assert "/dev/vda" in last, (
            f"the machine's root went away from the reading instead of coming refused:\n{last}"
        )
        # The compared span is the VALUE of a text key, and a value exists in
        # every declared language — the assertion accepts either.
        # pt-BR: `format-needed-at`, in both .ftl files.
        assert "mora em" in last or "lives at" in last, (
            f"the root crossed over with no reason written beside it:\n{last}"
        )
        # AND THE STICK IS THERE, spare, because a screen that only ever shows
        # what it refuses is a screen nobody can use.
        assert "/dev/vdb" in last, f"the empty disk did not reach the component:\n{last}"
        # AND NOTHING OF ZERO BYTES. Measured here first: udisks publishes a
        # floppy node and EIGHT unattached loop devices, and every one of them
        # came out as a row — nine lines of noise in front of the two disks the
        # machine has.
        assert "loop" not in last, f"the empty loops came back into the list:\n{last}"
        assert "bytes: 0" not in last, (
            f"a device of zero bytes is in the list:\n{last}"
        )

        # THE FORMAT WINDOW'S OWN RECTANGLE, asked of the compositor, for the
        # reason the example subtest gives: a crop of the whole output carries
        # empty desk around this window, and `tesseract --psm 6` may read
        # nothing at all from it. Both readings below use this rectangle.
        w, h, x, y = window_box(sock, "br.ronoctua.format")

        def nodes_on_screen(tag):
            """The format window, read by OCR, with the diagnosis printed first."""
            machine.succeed(f"${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/{tag}.xwd")
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/{tag}.xwd -crop {w}x{h}+{x}+{y} "
                f"+repage -colorspace gray -negate -resize 200% /tmp/{tag}.png"
            )
            # DIAGNOSIS BEFORE THE OCR, as in the example subtest: `cores=1`
            # says "uniform image, nothing drawn".
            print(f"the format window ({tag}): " + machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/{tag}.png "
                "-format 'cores=%k mean=%[fx:mean]' info:"
            ))
            said = machine.succeed(
                f"${pkgs.tesseract}/bin/tesseract /tmp/{tag}.png stdout --psm 6 2>/dev/null"
            ).strip()
            print(f"OCR {tag}: {said!r}")
            return said

        # READ UNTIL A DISK IS ON IT, at most fifteen times one second apart.
        # The journal says the rack crossed, but the frame that carries it may
        # not be on screen yet on a loaded machine, and that must not change
        # the verdict.
        read_back = ""
        for _ in range(15):
            read_back = nodes_on_screen("fmt")
            if "/dev/" in read_back:
                break
            machine.sleep(1)
        lowered = read_back.lower()

        # AND WHAT ONLY THE PIXEL ANSWERS: that the window opened, the renderer
        # drew, and what the bus said reached it. This is the shape of the
        # defect that reached the owner's screen last time — a surface that was
        # built, wired and tested, and drew nothing.
        #
        # THE TITLE IS NOT ASKED FOR. `format-title` is the window's header,
        # which the distro ships hidden (`Screen::heading` draws nothing at row
        # 0 under `hide_header`), so it is not on the pixel at all. The
        # translated key is asked of the body instead, on this same reading:
        # the technical line (`format-tech-whole`) further down.
        assert "/dev/" in read_back, (
            "the buffer opened and drew, but no disk reached the pixel: "
            f"{read_back!r}"
        )
        assert "nenhum disco" not in lowered, (
            f"the screen still says there is no disk at all: {read_back!r}"
        )

        # AND BOTH ROWS ARE REALLY THERE, read by moving the cursor.
        #
        # One reading cannot settle this. The OCR drops the line under the
        # cursor's highlight band — with two rows and a wide band, tesseract
        # takes it for an image region — so a single screenshot always shows one
        # row and never says whether the other was drawn or merely missed.
        #
        # Moving the cursor swaps WHICH row is unreadable, so the union of two
        # readings covers both. It also proves the arrow reached the component
        # through the compositor, which nothing else here does.
        #
        # READ UNTIL THE MOVE IS ON IT, at most fifteen times one second apart,
        # like the first reading: the key is pressed ONCE, and on a loaded
        # machine the frame that follows it may come later than any fixed
        # sleep. The loop stops when what the assertions below ask of this
        # reading is there; if it never is, they fail on the last frame read.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Down")
        moved = ""
        both = read_back
        for _ in range(15):
            moved = nodes_on_screen("fmt2")
            both = read_back + "\n" + moved
            # pt-BR: the value of `format-tech-needed` on screen, as below.
            if "montado pelo sistema" in moved.lower() and all(
                node in both for node in ["/dev/vda", "/dev/vdb"]
            ):
                break
            machine.sleep(1)
        for node in ["/dev/vda", "/dev/vdb"]:
            assert node in both, (
                f"{node} did not appear in either reading — the row "
                f"is not on screen:\n{both!r}"
            )

        # AND THE SECOND REGISTER IS ON THE PIXEL, following the cursor.
        #
        # ASKED FOR FROM THE SCREEN: the rows are written for somebody starting
        # out, and somebody who already knows what they want had nowhere to read
        # what the option actually does. The two readings above are the proof
        # that it MOVES — with the cursor on the empty stick the line talks
        # about partition tables, and with it on the machine's root it talks
        # about why udisks would refuse. A caption could not do both.
        # pt-BR: the OCR needle is the VALUE of a text key on screen
        # (`format-tech-whole`), not a message of this test.
        assert "tabela" in read_back.lower(), (
            "with the cursor on the free disk, the technical line does not say what is going to be "
            f"written on it: {read_back!r}"
        )
        # The compared span is the VALUE of a text key, drawn by the format
        # buffer on the technical line — not a message of this test.
        # pt-BR: `format-tech-needed`, in both .ftl files.
        assert "montado pelo sistema" in moved.lower(), (
            "with the cursor on the root, the technical line does not say why it does not do: "
            f"{moved!r}"
        )
  '';
}
