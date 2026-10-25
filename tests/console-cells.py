# The kernel's console, read back — for the VM tests of the terminal UIs that
# run on it (`installer-console-vm`, `rescue-vm`). Read into the test script
# itself (`builtins.readFile`), so the driver's type check sees every name;
# each function takes the machine it asks.
#
# WHAT EACH CELL WEARS, from `/dev/vcsaN` (the attribute) and `/dev/vcsuN` (the
# character): no screenshot, no OCR. Two traps the reading has to know, and
# each made a right screen read as a broken one:
#
# - the attribute is the console's VGA index, not the ANSI slot a program
#   asked for: the kernel's `color_table` maps one to the other, and `VGA`
#   below is that table;
# - with a font of more than 256 glyphs the attribute is SHIFTED one bit up,
#   its bit 0 the glyph's ninth: text `(a >> 1) & 15`, ground `a >> 5` (eight
#   grounds); with 256, text `a & 15`, ground `a >> 4`.
#
# AND WHAT THE SCREEN SHOWS, from QEMU's own screen dump (a PPM): the colour a
# slot holds is only seen there.

import base64
import os
import re
import struct
import tempfile

# ANSI slot -> VGA index (the kernel's `color_table`); `ANSI` is the inverse.
VGA = [0, 4, 2, 6, 1, 5, 3, 7, 8, 12, 10, 14, 9, 13, 11, 15]
ANSI = [VGA.index(k) for k in range(16)]


def console_font(m, vt):
    """The font console `vt` holds: (glyphs, width, height) — asked of the
    kernel (`showconsolefont -v -i`), not read from the file it was given.
    `setfont -O` is not the question: with no font named it loads the
    default one."""
    said = m.succeed(f"showconsolefont -C /dev/tty{vt} -v -i")
    found = dict(re.findall(r"(count|width|height)\s*:\s*(\d+)", said))
    assert len(found) == 3, f"showconsolefont said: {said}"
    return int(found["count"]), int(found["width"]), int(found["height"])


def unicode_table(path):
    """A PSF2 file's table: each codepoint, at the glyph position it draws."""
    raw = open(path, "rb").read()
    _, _, header, _, count, size, _, _ = struct.unpack("<4sIIIIIII", raw[:32])
    table = {}
    for at, entry in enumerate(raw[header + count * size :].split(b"\xff")[:count]):
        for c in entry.decode("utf-8"):
            table.setdefault(c, at)
    return table


def held_table(m, vt):
    """The table the kernel holds for console `vt` (`getunimap`): each
    codepoint, at the glyph position it draws. A font's size is no proof of
    which font it is — the VGA's own is 8 × 16 and 256 too —, its table is."""
    table = {}
    for line in m.succeed(f"getunimap -C /dev/tty{vt}").splitlines():
        found = re.match(r"\s*0x([0-9a-fA-F]+)\s+U\+([0-9a-fA-F]+)", line)
        if found:
            table.setdefault(chr(int(found.group(2), 16)), int(found.group(1), 16))
    return table


def holds_the_font(m, vt, path):
    """Whether console `vt` holds the font in `path`: its letters and signs
    the VGA's own font has not — `ã`, `ç`, the dash, the QR's half block — at
    the positions the file draws them."""
    mine, held = unicode_table(path), held_table(m, vt)
    probes = "ãç—▀"
    return all(held.get(c) == mine.get(c) is not None for c in probes), {
        c: (held.get(c), mine.get(c)) for c in probes
    }


def cells(m, vt, glyphs):
    """Each row of console `vt`, as (character, ANSI text slot, ANSI ground
    slot) per cell, under a font of `glyphs` glyphs."""
    raw = base64.b64decode(m.succeed(f"base64 -w0 /dev/vcsa{vt}"))
    uni = base64.b64decode(m.succeed(f"base64 -w0 /dev/vcsu{vt}"))
    rows, cols = raw[0], raw[1]
    grid = []
    for y in range(rows):
        line = []
        for x in range(cols):
            i = y * cols + x
            a = raw[4 + 2 * i + 1]
            fg, bg = ((a >> 1) & 0xF, a >> 5) if glyphs > 256 else (a & 0xF, a >> 4)
            ch = chr(int.from_bytes(uni[4 * i : 4 * i + 4], "little"))
            line.append((ch, ANSI[fg], ANSI[bg]))
        grid.append(line)
    return grid


def text_of(line):
    return "".join(c for c, _, _ in line).rstrip()


def ink(line):
    """The row's first cell with a letter in it: the text keeps an inset
    from the edge, and a cell of the inset wears no tone of its own."""
    return next(cell for cell in line if cell[0].strip())


def screen_dump(m):
    """The screen as QEMU shows it: (width, height, rgb bytes)."""
    path = os.path.join(tempfile.mkdtemp(), "screen.ppm")
    m.send_monitor_command(f"screendump {path}")
    data = open(path, "rb").read()
    fields, at = [], 0
    while len(fields) < 4:
        while data[at : at + 1].isspace():
            at += 1
        end = at
        while not data[end : end + 1].isspace():
            end += 1
        fields.append(data[at:end])
        at = end
    assert fields[0] == b"P6", fields[0]
    width, height = int(fields[1]), int(fields[2])
    return width, height, data[at + 1 :]


def pixel(shot, x, y):
    width, _, rgb = shot
    i = 3 * (y * width + x)
    return "%02X%02X%02X" % (rgb[i], rgb[i + 1], rgb[i + 2])


def token(themes, theme, role):
    """A role's colour in a theme, as the generator wrote it (`RRGGBB`)."""
    import json

    return json.load(open(f"{themes}/{theme}.json"))[role][2:8].upper()


def console_complaints(m):
    """What this boot's journal says against the console's setup, in the
    initrd and in the system: any line of `setfont`'s (it writes only when a
    console refuses a font), of the kernel's font operation, or of the setup
    giving up — the sentences `systemd-vconsole-setup` says then — and the
    setup's unit failed. Empty is the answer.

    On the dummy console the setup succeeds with nothing done, by design,
    and runs again when the console binds (`nix/modules/console.nix`): a
    restart that lands in the middle is no failure either."""
    said = "|".join(
        [
            "setfont",
            "kdfontop",
            "failed with a .system error",
            "Configuration of first virtual console",
            "virtual consoles are busy",
            "No virtual console that can be configured",
            "is not in KD_TEXT",
            "Failed to start Virtual Console Setup",
            "systemd-vconsole-setup.service: Failed",
        ]
    )
    lines = m.succeed(f"journalctl -b --no-pager -o cat | grep -i -E '{said}' || true")
    found = [line for line in lines.splitlines() if line.strip()]
    if m.execute("systemctl is-failed --quiet systemd-vconsole-setup.service")[0] == 0:
        found.append("systemd-vconsole-setup.service failed")
    return found
