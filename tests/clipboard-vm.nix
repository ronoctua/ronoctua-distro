# The clipboard history, proved ON SCREEN and on the clipboard itself.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# That the capture happens. The queue is unit-tested (a password does not go
# in, a pinned entry survives the ceiling), but none of that says that the
# daemon SEES what was copied: that depends on `wl_data_device`, on a real
# compositor, and on the read not hanging the event loop.
#
# And it proves the whole route: copying two things from OUTSIDE, opening the
# view, stepping to the oldest one, and re-copying it — with `wl-paste`
# answering. A check that only looked at the queue would not tell "it kept" from
# "it shows and gives back".
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  first = "ronoctua-first-copied";
  second = "ronoctua-second-copied";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-clipboard";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.wl-clipboard
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


    def copy_text(body):
        """Copies from OUTSIDE, as any program would."""
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"${pkgs.wl-clipboard}/bin/wl-copy {body}"
        )
        machine.sleep(2)


    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("the footer comes up"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            f"--setenv=SCROLLSOCK={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'control socket opened' > /dev/null",
            timeout=90,
        )
        focus_compositor()

    with subtest("the daemon SEES what was copied from outside"):
        # The capture is native (`wl_data_device`), and not a `wl-paste --watch`:
        # no process per copy, and the MIME types arrive along with it — which is
        # what the secret filter needs.
        copy_text("${first}")
        copy_text("${second}")
        try:
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager "
                "| grep -c 'new entry in the clipboard history' "
                "| grep -E '^[2-9]' > /dev/null",
                timeout=30,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -40")[1])
            raise

    with subtest("`LEADER c` opens the view with both entries"):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'leader active' > /dev/null", timeout=30
        )
        # `c` IS A LEAF, not a prefix. It was the other way round for one
        # delivery, and the screen capture has lived under `m s` since — the
        # `AGENTS.md` line says why in the owner's own terms: miscellany is the
        # drawer for what belongs to no group, and that is what frees `c` to be
        # the clipboard ALONE, a command and not a prefix.
        #
        # THE SECOND `c` WAS NOT A NO-OP. It reached the view that the first
        # one had just opened, where `c` is `clipboard-sync` — so it opened the
        # minibuffer's "where is the server" question, and the `<down>` and
        # `RET` below were answering THAT. The `RET` submitted an empty server
        # and nothing was ever recopied.
        press_key("c")
        journal = machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'clipboard opened'", timeout=30
        )
        print(journal.strip())
        assert "inputs=2" in journal, (
            f"the history should have both entries; log:\n{journal}"
        )

    with subtest("the copied text is ON SCREEN"):
        # The queue keeping is not the same thing as the region showing. The OCR
        # closes the chain: capture -> queue -> expansion -> span -> pixel.
        w, h, x, y = compositor_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/clip.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/clip.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        assert "copied" in read_back.lower(), (
            f"the copied content is not on screen; OCR read {read_back!r}"
        )

    with subtest("walking to the oldest and recopying it gives THAT one back"):
        # The route that matters: choosing in the history and putting it back on
        # the clipboard. Re-copying the TOP would prove nothing — it is already
        # there.
        press_key("Down")
        press_key("Return")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'recopied from the history' > /dev/null",
            timeout=30,
        )
        pasted = machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.wl-clipboard}/bin/wl-paste --no-newline"
        ).strip()
        print(f"clipboard: {pasted!r}")
        assert pasted == "${first}", (
            f"expected the oldest entry (${first}), got {pasted!r}"
        )

    with subtest("a copied image enters the history and BECOMES A PREVIEW"):
        # What only the VM proves: that the binary offer is read (and not
        # discarded as "it is not UTF-8"), that the image is decoded, and that it
        # reaches the texture pipeline. The decoding itself has a unit test; what
        # is missing is the path — capture -> queue -> preview.
        machine.succeed("systemctl restart footer")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'control socket opened' > /dev/null",
            timeout=60,
        )
        focus_compositor()

        # A PNG of a known size, so the log can be checked.
        machine.succeed(
            "${pkgs.imagemagick}/bin/magick -size 64x32 xc:red /tmp/vermelho.png"
        )
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${pkgs.wl-clipboard}/bin/wl-copy --type image/png < /tmp/vermelho.png"
        )
        machine.sleep(2)

        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'leader active' > /dev/null", timeout=30
        )
        # `c` IS A LEAF, not a prefix. It was the other way round for one
        # delivery, and the screen capture has lived under `m s` since — the
        # `AGENTS.md` line says why in the owner's own terms: miscellany is the
        # drawer for what belongs to no group, and that is what frees `c` to be
        # the clipboard ALONE, a command and not a prefix.
        #
        # THE SECOND `c` WAS NOT A NO-OP. It reached the view that the first
        # one had just opened, where `c` is `clipboard-sync` — so it opened the
        # minibuffer's "where is the server" question, and the `<down>` and
        # `RET` below were answering THAT. The `RET` submitted an empty server
        # and nothing was ever recopied.
        press_key("c")

        try:
            journal = machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep 'preview decoded'", timeout=30
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -30")[1])
            raise
        print(journal.strip())
        # THE REAL DIMENSIONS: a decoder that gave back 0x0 or the wrong size
        # would pass a test that only looked at "it decoded".
        assert "width=64" in journal and "height=32" in journal, (
            f"the preview came out at the wrong size; log:\n{journal}"
        )
  '';
}
