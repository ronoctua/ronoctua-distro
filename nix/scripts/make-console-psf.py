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
# 512 slots at most. The order matters: slot 0 is the fallback glyph.
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
# console from showing a square when the text uses a typographic variant.
ALIASES: dict[int, list[int]] = {
    0x0020: [0x00A0, 0x2007, 0x202F],  # spaces
    0x002D: [0x2010, 0x2011],  # hyphens
    0x0027: [0x2032],
    0x0022: [0x2033],
}


def render(face: freetype.Face, codepoint: int, width: int, height: int) -> bytes:
    """Rasterises a codepoint into a width x height bitmap, 1 bit per pixel."""
    row_bytes = (width + 7) // 8
    out = bytearray(row_bytes * height)

    gid = face.get_char_index(codepoint)
    if gid == 0:
        return bytes(out)  # no glyph: empty cell

    face.load_glyph(gid, freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO)
    bitmap = face.glyph.bitmap
    left = face.glyph.bitmap_left
    top = face.glyph.bitmap_top

    # Baseline: FreeType gives the ascender in 26.6; we use the face's metric.
    baseline = face.size.ascender >> 6

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

    # The font is monospaced at 0.600 em (verified in the inventory). Cell
    # width = 0.6 * height, rounded up.
    width = max(1, round(args.height * 0.6))
    face.set_pixel_sizes(width, args.height)

    codepoints = list(dict.fromkeys(REPERTOIRE))  # dedup preserving order
    if len(codepoints) > 512:
        print(
            f"ERROR: the repertoire has {len(codepoints)} codepoints; the console's PSF2 "
            f"takes at most 512. Cut the repertoire in REPERTOIRE.",
            file=sys.stderr,
        )
        return 1

    row_bytes = (width + 7) // 8
    charsize = row_bytes * args.height

    glyphs = [render(face, cp, width, args.height) for cp in codepoints]

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
    for cp in codepoints:
        unicode_table += chr(cp).encode("utf-8")
        for alias in ALIASES.get(cp, []):
            unicode_table += chr(alias).encode("utf-8")
        unicode_table.append(PSF2_SEPARATOR)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(header + b"".join(glyphs) + bytes(unicode_table))

    blank = sum(1 for g in glyphs if not any(g))
    print(
        f"{args.output.name}: {len(glyphs)} glyphs, {width}x{args.height}, "
        f"{charsize} bytes/glyph, {len(unicode_table)} bytes of unicode table"
    )
    if blank:
        print(f"  warning: {blank} empty glyphs (the codepoint is missing from the font)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
