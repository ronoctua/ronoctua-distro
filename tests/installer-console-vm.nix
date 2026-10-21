# The installer's TUI on the kernel's console — the ISO's console, which only a
# booted machine has.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The TUI's own tests draw into a buffer and see the colours it ASKS for. The
# console takes a 24-bit colour and keeps about one bit of each channel, and
# for as long as the ISO existed that made every option white and the cursor's
# band the black of the ground: no screen said which option the cursor was on.
# Four things, and only the console answers them:
#
#   1. the ISO's font is the one the console holds — it was refused for 498
#      glyphs (`KDFONTOP: Invalid argument`), and every console fell back to
#      the kernel's — and the position the console erases with is blank;
#   2. the cursor's row is a band and a colour of its own, the question a tone
#      no option wears, read from `/dev/vcsa1` (`tests/console-cells.py`);
#   3. the band on the screen is the default theme's `surface.selection`,
#      exactly: the slots hold the tokens, and the theme is the one a fresh
#      machine shows, not the first of the chooser's order;
#   4. leaving gives the console its own palette back.
{
  pkgs,
  ronoctua,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-installer-console";

  nodes.machine = {
    # THE ISO'S CONSOLE: its font, set as the ISO sets it.
    console = {
      earlySetup = true;
      font = ronoctua.consoleFont;
    };
    services.getty.autologinUser = "root";
    # A laptop's screen, and a framebuffer console on it — the test VM's
    # default is the text mode's 80 × 25.
    boot.kernelParams = [ "video=1920x1080" ];
    virtualisation.qemu.options = [ "-vga std" ];
    virtualisation.memorySize = 1024;
    environment.systemPackages = [
      ronoctua.installer
      pkgs.kbd
    ];
  };

  testScript = ''
    ${builtins.readFile ./console-cells.py}
    import struct

    themes, theme = "${ronoctua.themed}", "${ronoctua.defaultTheme}"

    machine.start()
    machine.wait_for_unit("getty@tty1.service")
    machine.wait_until_tty_matches("1", "root@")

    with subtest("the console holds the ISO's font"):
        glyphs, width, height = console_font(1)
        _, _, _, _, count, _, wanted_height, wanted_width = struct.unpack(
            "<4sIIIIIII", open("${ronoctua.consoleFont}", "rb").read(32)
        )
        print(f"the console's font: {glyphs} glyphs, {width}x{height}")
        assert (glyphs, width, height) == (count, wanted_width, wanted_height), (
            f"the console holds a {width}x{height} font of {glyphs}, not the ISO's "
            f"{wanted_width}x{wanted_height} of {count}: "
            + machine.execute("journalctl -b --no-pager | grep -i setfont")[1]
        )

    with subtest("the cursor's row is a band and a colour, the question its own tone"):
        machine.send_chars("clear; ronoctua-install\n")
        machine.wait_until_tty_matches("1", "Esc")
        machine.sleep(2)
        machine.screenshot("installer")
        grid = cells(1, glyphs)
        for line in grid[:6]:
            print([(c, f, b) for c, f, b in line[:3]], text_of(line))
        ground = grid[0][-1][2]
        options = [y for y in range(2, len(grid) - 3) if text_of(grid[y])]
        banded = [y for y in options if grid[y][0][2] != ground]
        assert len(banded) == 1, f"the band is on rows {banded} of {options}"
        here = banded[0]
        other = next(y for y in options if y != here)
        question = grid[1][0][1]
        assert grid[here][0][1] != grid[other][0][1], "the cursor's option has the others' tone"
        assert question != grid[other][0][1], "the question has its options' tone"
        assert question != grid[here][0][1], "the question has the cursor's tone"

        # THE CURSOR FOLLOWS THE KEY: a band drawn once is not a cursor.
        machine.send_key("down")
        machine.sleep(1)
        moved = cells(1, glyphs)
        now = [y for y in options if moved[y][0][2] != ground]
        assert now == [here + 1], f"down moved the band from {here} to {now}"

    with subtest("the band is the default theme's selection, exactly"):
        shot = screen_dump()
        # The last column of the text, inside the band, which runs edge to edge.
        edge = len(grid[0]) * width - 2
        band = pixel(shot, edge, (here + 1) * height + height // 2)
        wanted = token(themes, theme, "surface.selection")
        assert band == wanted, f"the band is #{band}, and {theme}'s selection is #{wanted}"
        floor = pixel(shot, edge, height // 2)
        assert floor == token(themes, theme, "surface.base"), f"the ground is #{floor}"

    with subtest("leaving gives the console its own palette back"):
        machine.send_key("ctrl-c")
        machine.sleep(1)
        machine.send_chars("clear; echo palette-back\n")
        machine.wait_until_tty_matches("1", "palette-back")
        machine.sleep(1)
        shot = screen_dump()
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
