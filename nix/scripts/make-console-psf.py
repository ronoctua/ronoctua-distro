#!/usr/bin/env python3
"""Generates a PSF2 console font from the real font file.

Reason
------
Section 6.2 of the bootstrap document demands TTF-nerdfonts be the default
"everywhere... TTY". The Linux console does NOT consume TTF: it wants PSF/PSFU, a
bitmap with at most 512 glyphs. Instead of installing somebody else's console
font (which would make the TTY look like something else), we rasterise our own.

We chose to rasterise directly with FreeType rather than the
fontforge -> BDF -> bdf2psf route: fewer dependencies, fewer opaque steps, and the
repertoire ends up declared here, instead of in an equivalences file.

PSF2 format
-----------
    magic[4]    0x72 0xb5 0x4a 0x86
    version     0
    headersize  32
    flags       1 if there is a unicode table
    length      number of glyphs
    charsize    bytes per glyph = height * ceil(width/8)
    height, width

Then: the bitmaps, and after that the unicode table — for each glyph, the codepoints
in UTF-8, terminated by 0xFF.

What the kernel takes
---------------------
Two rules no format document states, and both were broken for as long as this
font existed — the console fell back to the kernel's own font on every machine:

- the framebuffer console takes 256 or 512 glyphs and refuses any other count
  (`KDFONTOP: Invalid argument`), so the font is PADDED with blank glyphs to
  the next of the two;
- the console erases a cell with glyph POSITION 32, not with the glyph the
  unicode table maps to a space, so printable ASCII sits at its own codepoint:
  with the repertoire from position 0, `@` stood at 32 and every cleared cell
  would have shown it.
"""

from __future__ import annotations

import argparse
import pathlib
import struct
import sys

import freetype

PSF2_MAGIC = b"\x72\xb5\x4a\x86"
PSF2_HAS_UNICODE_TABLE = 0x01
PSF2_SEPARATOR = 0xFF

# --- repertoire -------------------------------------------------------------
# 512 slots at most. Printable ASCII takes its own positions (see the module's
# docstring); the rest fills the free positions in this order.
#
# What goes in, and why:
#   ASCII            the basics
#   Latin-1 Sup      pt-BR needs complete accenting
#   Latin Ext-A      still pt-BR (none here, but there are for other locales) + en-GB
#   box-drawing      section 5.1's aesthetics are made of one-line rules
#   blocks           the installer's progress bars
#   arrows & symbols navigation in the installer

REPERTOIRE: list[int] = []


def add_range(lo: int, hi: int) -> None:
    REPERTOIRE.extend(range(lo, hi + 1))


add_range(0x0020, 0x007E)  # printable ASCII
add_range(0x00A0, 0x00FF)  # Latin-1 Supplement
add_range(0x0100, 0x017F)  # Latin Extended-A
add_range(0x2500, 0x257F)  # Box Drawing
add_range(0x2580, 0x259F)  # Block Elements
REPERTOIRE += [
    0x2190,
    0x2191,
    0x2192,
    0x2193,  # arrows
    0x21B5,  # return
    0x2022,  # bullet
    0x2026,  # ellipsis
    0x2013,
    0x2014,  # dashes
    0x2018,
    0x2019,
    0x201C,
    0x201D,  # quotes
    0x00B7,  # middle dot
    0x2713,
    0x2717,  # check and cross
    0x25A0,
    0x25CF,
    0x25CB,  # shapes
    0x26A0,  # warning
]

# Aliases: extra codepoints pointing at the same glyph. It is what keeps the
# console from showing a square when the text uses a typographic variant. A
# codepoint with a glyph of its own is never also an alias: mapped twice, which
# glyph it draws would be the kernel's choice.
ALIASES: dict[int, list[int]] = {
    0x0020: [0x00A0, 0x2007, 0x202F],  # spaces
    0x002D: [0x2010, 0x2011],  # hyphens
    0x0027: [0x2032],
    0x0022: [0x2033],
}


# The counts the framebuffer console takes, and the most there can be.
COUNTS = (256, 512)
MOST_GLYPHS = COUNTS[-1]

# Printable ASCII, at its own positions.
ASCII = range(0x20, 0x7F)


def layout(codepoints: list[int]) -> list[int | None]:
    """Each position's codepoint (`None`: a blank glyph), as many positions as
    the console takes: printable ASCII at its own codepoint, the rest in the
    free positions in order."""
    count = next(n for n in COUNTS if n >= len(codepoints))
    positions: list[int | None] = [None] * count
    rest = []
    for cp in codepoints:
        if cp in ASCII:
            positions[cp] = cp
        else:
            rest.append(cp)
    free = [i for i, cp in enumerate(positions) if cp is None and i not in ASCII]
    for i, cp in zip(free, rest):
        positions[i] = cp
    return positions


def cell(face: freetype.Face, height: int) -> tuple[int, int, int]:
    """Sizes `face` so that one cell `height` pixels tall is its line box:
    the cell's width, the baseline's row, and how far (26.6) a glyph drawn to
    the box is moved up so that the box's edges are the cell's (`render`).

    THE CELL IS THE FACE'S LINE BOX — ascender to descender, 1.25 em in this
    face — across its advance (0.6 em, the face is monospaced). The height
    was once taken for the em and the width for the em's width too
    (`set_pixel_sizes(width, height)`): the letters came out at 60% of their
    cell with a column of air between them, and the descenders below its
    bottom row, cut. The box is what the face's own block and box-drawing
    glyphs are drawn to (`█` fills it, `▀` its top half), and what makes the
    QR's half blocks square.

    The two axes are scaled apart, each to its integer: a cell is whole pixels
    and 0.6 em is not, and a few percent of width is not something a reader
    sees, where a cut stroke is.
    """
    face.load_char("M", freetype.FT_LOAD_NO_SCALE)
    advance = face.glyph.advance.x
    box = face.ascender - face.descender
    em_y = height * face.units_per_EM / box
    width = max(1, round(advance * em_y / face.units_per_EM))
    em_x = width * face.units_per_EM / advance
    face.set_char_size(round(em_x * 64), round(em_y * 64), 72, 72)
    exact = face.ascender * height / box
    baseline = round(exact)
    return width, baseline, round((baseline - exact) * 64)


# THE GLYPHS DRAWN TO THE BOX — box drawing and block elements. The baseline
# falls between two rows (12.35 at 16 px), and a hinted glyph's edges round
# on their own: `▀` came out a row more than half its cell, and a vertical
# rule a row short of the next line's. These are drawn unhinted, moved up by
# the fraction, so that every edge the face puts on the box or on its half
# lands on a pixel's edge; the text keeps its hinting, where a sub-pixel offset
# is not seen and a stem's weight is.
TO_THE_BOX = range(0x2500, 0x25A0)


def render(
    face: freetype.Face, codepoint: int, width: int, height: int, baseline: int, lift: int
) -> bytes:
    """Rasterises a codepoint into a width x height bitmap, 1 bit per pixel,
    its baseline `baseline` pixels from the top — and one drawn to the box
    (`TO_THE_BOX`) unhinted and `lift` (26.6) higher."""
    row_bytes = (width + 7) // 8
    out = bytearray(row_bytes * height)

    gid = face.get_char_index(codepoint)
    if gid == 0:
        return bytes(out)  # no glyph: empty cell

    boxed = codepoint in TO_THE_BOX
    identity = freetype.Matrix(0x10000, 0, 0, 0x10000)
    face.set_transform(identity, freetype.Vector(0, lift if boxed else 0))
    flags = freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO
    face.load_glyph(gid, flags | (freetype.FT_LOAD_NO_HINTING if boxed else 0))
    bitmap = face.glyph.bitmap
    left = face.glyph.bitmap_left
    top = face.glyph.bitmap_top

    for y in range(bitmap.rows):
        dst_y = baseline - top + y
        if not (0 <= dst_y < height):
            continue
        for x in range(bitmap.width):
            dst_x = left + x
            if not (0 <= dst_x < width):
                continue
            byte = bitmap.buffer[y * bitmap.pitch + (x >> 3)]
            if byte & (0x80 >> (x & 7)):
                out[dst_y * row_bytes + (dst_x >> 3)] |= 0x80 >> (dst_x & 7)

    return bytes(out)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("font", type=pathlib.Path)
    ap.add_argument("--height", type=int, required=True)
    ap.add_argument("--output", type=pathlib.Path, required=True)
    args = ap.parse_args()

    face = freetype.Face(str(args.font))

    width, baseline, lift = cell(face, args.height)

    codepoints = list(dict.fromkeys(REPERTOIRE))  # dedup preserving order
    if len(codepoints) > MOST_GLYPHS:
        print(
            f"ERROR: the repertoire has {len(codepoints)} codepoints; the console's PSF2 "
            f"takes at most 512. Cut the repertoire in REPERTOIRE.",
            file=sys.stderr,
        )
        return 1

    row_bytes = (width + 7) // 8
    charsize = row_bytes * args.height

    positions = layout(codepoints)
    blank = bytes(charsize)
    glyphs = [
        blank if cp is None else render(face, cp, width, args.height, baseline, lift)
        for cp in positions
    ]

    header = PSF2_MAGIC + struct.pack(
        "<IIIIIII",
        0,  # version
        32,  # headersize
        PSF2_HAS_UNICODE_TABLE,  # flags
        len(glyphs),  # length
        charsize,
        args.height,
        width,
    )

    unicode_table = bytearray()
    own = set(codepoints)
    for cp in positions:
        if cp is not None:
            unicode_table += chr(cp).encode("utf-8")
            for alias in ALIASES.get(cp, []):
                if alias not in own:
                    unicode_table += chr(alias).encode("utf-8")
        unicode_table.append(PSF2_SEPARATOR)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(header + b"".join(glyphs) + bytes(unicode_table))

    empty = sum(
        1 for cp, g in zip(positions, glyphs) if cp is not None and cp != 0x20 and not any(g)
    )
    print(
        f"{args.output.name}: {len(glyphs)} glyphs ({len(codepoints)} drawn), "
        f"{width}x{args.height}, {charsize} bytes/glyph, "
        f"{len(unicode_table)} bytes of unicode table"
    )
    if empty:
        print(f"  warning: {empty} empty glyphs (the codepoint is missing from the font)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
