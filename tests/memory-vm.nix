# The memory panel on screen — `LEADER s m`, end to end.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The other buffers carry their own database: open the window and the content
# is already there. This one opens EMPTY and stays empty unless the host
# samples the machine and pushes the numbers in. So the chain here is longer
# than the sibling's, and no unit test reaches its far end:
#
#   /proc, cgroups  ->  ronoctua-memory  ->  Input::Survey (WIT)
#      ->  the component decides  ->  DisplayList  ->  i18n + tokens
#      ->  wgpu  ->  pixel  ->  OCR
#
# `memory-driven` proves the middle of that with a synthetic survey. What is
# left, and what only a machine can answer, is whether the HOST reads a real
# `/proc` and whether the clock actually ticks — a timer that never fires gives
# a window that stays blank, which is indistinguishable from a component that
# draws nothing.
#
# WHY IT LOOKS FOR `RAM` AND `GiB`
# --------------------------------
# `RAM` is a text KEY the host translates, and it reads the same in both
# languages: it proves the translation chain without depending on an accent the
# OCR may lose. `GiB` can only appear if a real number crossed the boundary —
# the component formats it from bytes it received. One says "the panel drew",
# the other says "the machine spoke".
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
  bufferMemory,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-memory-panel";

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


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("the panel comes up, and the host starts sampling"):
        machine.succeed(
            "systemd-run --unit=panel ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # pt-BR: the keys the component sends have to come out translated.
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            "${ronoctua-shell}/bin/ronoctua-shell buffer "
            "${bufferMemory}/lib/ronoctua_buffer_memory.wasm"
        )
        try:
            machine.wait_until_succeeds(
                "journalctl -u panel --no-pager | grep -q 'component rendered'",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u panel --no-pager")[1])
            raise

        journal = machine.succeed("journalctl -u panel --no-pager")
        for bad in ["panicked", "refused: the buffer did not declare the capability"]:
            assert bad not in journal, f"the panel reported: {bad}"

    with subtest("the COMPOSITOR sees a window, not a region of ours"):
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 1'",
            timeout=30,
        )

    with subtest("the machine spoke, and what it said is on screen"):
        # The clock ticks once a second; the first survey does not wait for it.
        machine.wait_until_succeeds(
            "journalctl -u panel --no-pager | grep -q 'buffer drawn on the xdg_toplevel'",
            timeout=60,
        )

        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        # THE WHOLE WINDOW, by the geometry niri reports — cropping by an
        # assumed position would be assuming layout, which invariant 4 forbids.
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/panel.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/panel.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")

        # The key that came out translated…
        assert "ram" in read_back.lower(), (
            f"the key `memory-ram` did not reach the screen; OCR read {read_back!r}"
        )
        # …and a number that can only exist if the host read the machine. A
        # panel that drew its labels and no numbers is a panel whose clock never
        # ticked — and it looks fine until one tries to read it.
        assert "gib" in read_back.lower() or "mib" in read_back.lower(), (
            f"nothing sized on screen: the host's sample did not arrive; OCR read {read_back!r}"
        )
  '';
}
