# The installer's TUI on the kernel's console — the ISO's console, which only a
# booted machine has — and the console's font on the two firmwares a machine
# boots from.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The TUI's own tests draw into a buffer and see the colours it ASKS for. The
# console takes a 24-bit colour and keeps about one bit of each channel, and
# for as long as the ISO existed that made every option white and the cursor's
# band the black of the ground: no screen said which option the cursor was on.
# And the console's font was refused (498 glyphs), and loading one where the
# console cannot take it is an `ERROR` in the journal (`nix/modules/console.nix`).
# Only a booted console answers:
#
#   1. on UEFI (`uefi`, firmware OVMF, as the ISO boots on a real machine)
#      the console holds the 15 × 32 — the font the ISO's module loads — and on
#      the VGA text console of a BIOS boot with no graphics driver (`vga`,
#      `nomodeset`) the 8 × 16 of 256 glyphs, the one that console takes;
#   2. on both, this boot's journal — initrd included — has no line of
#      `setfont`'s, of the kernel's font operation or of the setup giving up;
#   3. the cursor's row is a band and a colour of its own, the question a tone
#      no option wears, read from `/dev/vcsa1` (`tests/console-cells.py`), and
#      the band on the screen is the default theme's `surface.selection`,
#      exactly — the slots hold the tokens, and the theme is the one a fresh
#      machine shows;
#   4. leaving gives the console its own palette back, and a cleared line is
#      blank (the console erases with position 32).
{
  pkgs,
  ronoctua,
}:

let
  # The console as every machine and the ISO have it.
  console = {
    imports = [ ronoctua.consoleModule ];
    _module.args.ronoctua = { inherit (ronoctua) fonts; };
    services.getty.autologinUser = "root";
    virtualisation.memorySize = 1024;
    environment.systemPackages = [
      ronoctua.installer
      pkgs.kbd
    ];
  };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-installer-console";

  nodes.uefi = {
    imports = [ console ];
    # A laptop's screen on UEFI firmware: the kernel's frame buffer from the
    # firmware's, then the graphics driver's.
    boot.kernelParams = [ "video=1920x1080" ];
    virtualisation.qemu.options = [
      "-vga std"
      "-bios ${pkgs.OVMF.fd}/FV/OVMF.fd"
    ];
  };

  # A BIOS boot that never leaves the VGA text console: no graphics driver.
  nodes.vga = {
    imports = [ console ];
    boot.kernelParams = [ "nomodeset" ];
  };

  testScript = ''
    ${builtins.readFile ./console-cells.py}

    themes, theme = "${ronoctua.themed}", "${ronoctua.defaultTheme}"

    def header(path):
        _, _, _, _, count, _, height, width = struct.unpack("<4sIIIIIII", open(path, "rb").read(32))
        return count, width, height

    start_all()
    for m in (uefi, vga):
        m.wait_for_unit("getty@tty1.service")
        m.wait_until_tty_matches("1", "root@")

    with subtest("each console holds the font it takes, and the boot says nothing against it"):
        for m, font, firmware in (
            (uefi, "${ronoctua.fonts.consoleFont}", "UEFI"),
            (vga, "${ronoctua.fonts.consoleFontVga}", "BIOS, VGA text"),
        ):
            held = console_font(m, 1)
            print(f"{firmware}: {held[0]} glyphs, {held[1]}x{held[2]}")
            assert held == header(font), f"{firmware}: the console holds {held}, not {header(font)}"
            ours, probes = holds_the_font(m, 1, font)
            assert ours, f"{firmware}: the console's table is not the font's: {probes}"
            complaints = console_complaints(m)
            assert not complaints, f"{firmware}: " + "\n".join(complaints)
        assert vga.execute("test -d /sys/firmware/efi")[0] != 0, "the VGA node booted UEFI"
        uefi.succeed("test -d /sys/firmware/efi")

    glyphs, width, height = console_font(uefi, 1)

    with subtest("the cursor's row is a band and a colour, the question its own tone"):
        uefi.send_chars("clear; ronoctua-install\n")
        uefi.wait_until_tty_matches("1", "Esc")
        uefi.sleep(2)
        uefi.screenshot("installer")
        grid = cells(uefi, 1, glyphs)
        for line in grid[:6]:
            print([(c, f, b) for c, f, b in line[:3]], text_of(line))
        ground = grid[0][-1][2]
        options = [y for y in range(2, len(grid) - 3) if text_of(grid[y])]
        banded = [y for y in options if grid[y][0][2] != ground]
        assert len(banded) == 1, f"the band is on rows {banded} of {options}"
        here = banded[0]
        other = next(y for y in options if y != here)
        # The tones are read on the letters; the band, on the edge.
        question = ink(grid[1])[1]
        assert ink(grid[here])[1] != ink(grid[other])[1], "the cursor's option has the others' tone"
        assert question != ink(grid[other])[1], "the question has its options' tone"
        assert question != ink(grid[here])[1], "the question has the cursor's tone"

        # THE CURSOR FOLLOWS THE KEY: a band drawn once is not a cursor.
        uefi.send_key("down")
        uefi.sleep(1)
        moved = cells(uefi, 1, glyphs)
        now = [y for y in options if moved[y][0][2] != ground]
        assert now == [here + 1], f"down moved the band from {here} to {now}"

    with subtest("the band is the default theme's selection, exactly"):
        shot = screen_dump(uefi)
        # The screen's last column, past the text's inset: the band runs edge to edge.
        edge = len(grid[0]) * width - 2
        band = pixel(shot, edge, (here + 1) * height + height // 2)
        wanted = token(themes, theme, "surface.selection")
        assert band == wanted, f"the band is #{band}, and {theme}'s selection is #{wanted}"
        floor = pixel(shot, edge, height // 2)
        assert floor == token(themes, theme, "surface.base"), f"the ground is #{floor}"

    with subtest("leaving gives the console its own palette back"):
        uefi.send_key("ctrl-c")
        uefi.sleep(1)
        uefi.send_chars("clear; echo palette-back\n")
        uefi.wait_until_tty_matches("1", "palette-back")
        uefi.sleep(1)
        shot = screen_dump(uefi)
        inks = {
            pixel(shot, x, y)
            for y in range(0, height)
            for x in range(0, width * len("palette-back"))
        }
        # The kernel's own slot 7 is #AAAAAA; the installer's held the text's
        # token, which no theme solves to that grey.
        assert "AAAAAA" in inks, f"the shell's text is in {sorted(inks)}"
        # AND A CLEARED LINE IS BLANK: the console erases with position 32.
        row = {
            pixel(shot, x, 5 * height + y)
            for x in range(0, len(grid[0]) * width, 7)
            for y in range(height)
        }
        assert row == {"000000"}, f"a cleared row shows {sorted(row)}"
  '';
}
