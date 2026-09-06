# A native buffer in a window of its own — acceptance criterion of milestone 7.
#
# "The first native buffer end to end with an `xdg_toplevel` of its own."
#
# WHAT ONLY THE VM PROVES
# -----------------------
# That the compositor sees a WINDOW, and not a region drawn by us. The
# difference matters: invariant 4 says the shell never reimplements window
# layout, and drawing buffers inside a surface of ours would be exactly that.
# We ask niri — the IPC's `Windows` lists the windows, and the buffer's has to
# be there, with our `app_id`.
#
# And the content has to be ON SCREEN: the component produces a `DisplayList`
# with a text key, the host translates it and draws. The OCR closes the whole
# chain — component, protocol, i18n, tokens, wgpu.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
  bufferExample,
  bufferGlyphs,
  bufferFormat,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-buffer";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
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
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

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
                "journalctl -u buffer --no-pager | grep -q 'component rendered'",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u buffer --no-pager")[1])
            raise

        journal = machine.succeed("journalctl -u buffer --no-pager")
        # THREE commands: title, rule and the key counter — the example only
        # emits the third when there are more than two lines of context, and the
        # host asks for 24. If the `render` gave back empty, everything below
        # would pass with a blank window.
        assert "commands=3" in journal, journal

    with subtest("the COMPOSITOR sees a window, not a region of ours"):
        # The proof of invariant 4. A buffer drawn inside our surface would not
        # show up here.
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 1'",
            timeout=30,
        )
        out_text = machine.succeed(f"${ronoctua-shell}/bin/niri-contract {sock}")
        print(out_text)
        assert "focused: Some" in out_text, out_text

    with subtest("the buffer drew in the window"):
        machine.wait_until_succeeds(
            "journalctl -u buffer --no-pager | grep -q 'buffer drawn on the xdg_toplevel'",
            timeout=60,
        )
        journal = machine.succeed("journalctl -u buffer --no-pager")
        for bad in ["drawing the frame failed", "preparing the text failed", "panicked"]:
            assert bad not in journal, f"o buffer reportou: {bad}"

    with subtest("the component's key came out TRANSLATED on screen"):
        # The whole chain: component -> DisplayList -> i18n -> tokens -> wgpu ->
        # pixel. The component sends `cmd-mx-open`; in pt-BR that is
        # "Executar comando…", and that is what the OCR has to read.
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        # THE WHOLE WINDOW, and not a crop at the top: whoever decides where the
        # window goes is niri (invariant 4), and it does not put it at (0,0) —
        # its column has a width of its own and the window is centered in it.
        # Cropping by an assumed position is assuming layout, which is precisely
        # what the invariant forbids the shell to do.
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/buffer.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/buffer.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        assert "executar" in read_back.lower(), (
            f"the key did not come out translated on screen; OCR read {read_back!r}"
        )

    with subtest("COPY really copies, into the system's clipboard"):
        # THE DEFECT THIS CATCHES, and that nobody saw for months.
        #
        # `Effect::Copy` reached the host and became a `tracing::info!`. The
        # glyph browser's nine copy formats NEVER put anything anywhere, and the
        # feature looked finished — the worst failure mode this project knows.
        #
        # The criterion is the CLIPBOARD, and not the call: a test that asserted
        # "the wl-copy was invoked" would repeat the same defect one level up.
        machine.succeed("systemctl stop buffer")
        machine.succeed(
            "systemd-run --unit=glyphs ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell buffer "
            "${bufferGlyphs}/lib/ronoctua_buffer_glyphs.wasm"
        )
        machine.wait_until_succeeds(
            "journalctl -u glyphs --no-pager | grep -q 'buffer drawn on the xdg_toplevel'",
            timeout=90,
        )

        # With no WM on Xvfb nobody has input focus, and the key would be lost.
        focus_niri()
        machine.sleep(1)
        # The FIRST Enter opens the sheet; the SECOND copies the selected option,
        # which is the first of the list — the glyph's NAME.
        for _ in range(2):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
            machine.sleep(2)

        try:
            machine.wait_until_succeeds(
                "journalctl -u glyphs --no-pager | grep -q 'copied'", timeout=30
            )
        except Exception:
            print(machine.execute("journalctl -u glyphs --no-pager | tail -40")[1])
            raise

        pasted = machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.wl-clipboard}/bin/wl-paste --no-newline"
        ).strip()
        print(f"clipboard: {pasted!r}")
        # A Nerd Fonts glyph NAME: `<set>-<thing>`. Which one is not nailed down —
        # the order of the list with no search depends on the database, and nailing
        # the name down would make the test fail on a font update for the wrong reason.
        assert "-" in pasted and pasted.isascii() and len(pasted) > 3, (
            f"what was pasted does not look like a glyph name: {pasted!r}"
        )

    with subtest("Ctrl+V reads the clipboard and hands it to the buffer"):
        # The other side of the same thing. `Ctrl+V` fell into the host's Ctrl
        # guard and did NOTHING — and there was no way of telling that from
        # "there was nothing copied", which was exactly the owner's doubt.
        #
        # What is asserted here is the HOST's end: it really read the clipboard
        # and delivered the text to the component. That the text goes into the
        # search is proved by `browser-filters`, which drives the component
        # directly — together, the two close the path.
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Escape")
        machine.sleep(1)
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key ctrl+v")
        machine.sleep(2)

        journal = machine.succeed("journalctl -u glyphs --no-pager")
        target_path = f'input=Paste("{pasted}")'
        assert target_path in journal, (
            f"`Ctrl+V` did not hand the text to the buffer; expected {target_path!r} in the log:\n"
            + journal[-2000:]
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
        machine.succeed("systemctl stop glyphs || true")
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
            "journalctl -u format --no-pager | grep -q 'buffer drawn on the xdg_toplevel'",
            timeout=60,
        )
        # The rack has to have crossed. Before it does, the screen legitimately
        # says it has nothing — so waiting for the DRAW alone would measure the
        # empty state and pass.
        machine.wait_until_succeeds(
            "journalctl -u format --no-pager | grep -q 'input=Rack'",
            timeout=60,
        )
        machine.sleep(2)

        journal = machine.succeed("journalctl -u format --no-pager")
        for bad in ["drawing the frame failed", "panicked", "sem udisks2"]:
            assert bad not in journal, f"o buffer de formatar reportou: {bad}"

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

        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/fmt.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/fmt.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/fmt.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/fmt.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR of the format buffer: {read_back!r}")
        lowered = read_back.lower()

        # AND WHAT ONLY THE PIXEL ANSWERS: that the window opened, the renderer
        # drew, and the key the component sent came out translated. This is the
        # shape of the defect that reached the owner's screen last time — a
        # surface that was built, wired and tested, and drew nothing.
        assert "formatar" in lowered, (
            f"the title did not come out translated on screen: {read_back!r}"
        )
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
        # through niri, which nothing else here does.
        def nodes_on_screen(tag):
            machine.succeed(f"${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/{tag}.xwd")
            machine.succeed(
                f"${pkgs.imagemagick}/bin/magick /tmp/{tag}.xwd -crop {w}x{h}+{x}+{y} "
                f"+repage -colorspace gray -negate -resize 200% /tmp/{tag}.png"
            )
            said = machine.succeed(
                f"${pkgs.tesseract}/bin/tesseract /tmp/{tag}.png stdout --psm 6 2>/dev/null"
            ).strip()
            print(f"OCR {tag}: {said!r}")
            return said

        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Down")
        machine.sleep(2)
        moved = nodes_on_screen("fmt2")
        both = read_back + "\n" + moved
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
        assert "tabela" in read_back.lower(), (
            "with the cursor on the free disk, the technical line does not say what is going to be "
            f"written on it: {read_back!r}"
        )
        assert "montado pelo sistema" in moved.lower(), (
            "with the cursor on the root, the technical line does not say why it does not do: "
            f"{moved!r}"
        )
  '';
}
