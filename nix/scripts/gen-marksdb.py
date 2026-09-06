#!/usr/bin/env python3
"""Builds the face that holds the marks this repository draws — section 6.3.

WHAT THIS IS FOR
----------------
A mark the interface has to draw and that exists in none of the five faces.
Invariant 10 says a UI icon is a glyph of a font; invariant 9 says a codepoint
is a token. So the mark is drawn INTO a face of ours, and from there on it is
an icon like every other: a name, a codepoint, a cmap that gets checked.

The face is built from `assets/icons/marks.nix`, which declares dots on a grid
— not paths. See the comment at the top of that file for why.

THE THREE GUARDS, AND THE DEFECT EACH ONE IS ABOUT
--------------------------------------------------
1. THE RANGE HAS TO BE FREE IN EVERY OTHER FACE. The face asked for by name
   wins the fallback chain, so a mark sharing a codepoint with a Nerd Fonts
   glyph would come out drawn as ANOTHER symbol. Not tofu — a pretty, wrong
   icon, which nobody catches by looking. It is the measured defect that forced
   `nix/lucidegen.nix` to remap 634 codepoints, and this is the same guard.

2. THE ADVANCE IS COPIED, AND THE BOX IS A CEILING. Measured over the 6896
   `md-` glyphs of the built Iosevka: the ADVANCE is 600 for every single one,
   and that is what decides how many cells a glyph occupies. The ink box is
   not constant at all — 747 distinct boxes, the commonest `(0, 40, 600, 640)`
   on 1992 of them, and `md-battery` is `(28, -114, 572, 794)`. So the advance
   is required to MATCH and the ink is required to FIT INSIDE the reference's
   box: a mark may be smaller than its neighbours, never wider than the cell
   and never taller than the tallest thing beside it.

   The first version of this demanded the box match EXACTLY, on the strength
   of a claim that every icon shares one. It does not, and the guard refused
   legitimate drawings — a mark whose light dots are smaller does not reach
   the box's top edge. Whether a drawing is the right SIZE beside the text is
   a different question and has its own check, `chrome-icons-match-the-text`,
   which measures the ink against the digit.

3. A HOLE HAS TO BE A HOLE. TrueType fills by the non-zero rule: a ring is an
   outer contour and an inner one wound the OTHER WAY, and getting both the
   same way produces a solid dot with no diagnostic at all. The signed area of
   the two contours is computed and their signs are required to differ — which
   is the fact itself, and not a rasterisation of it.
"""

from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

# How many quadratic arcs make a circle.
#
# MEASURED on this construction, not quoted: eight arcs deviate by 0.31% of the
# radius at worst, which at the 22 px this mark is drawn at is 0.005 px — under
# a hundredth of a pixel. Four would be 6.1%, which is a visible flat side.
# (The figures for a CUBIC circle are sixteen times better and do not apply
# here; the first version of this comment quoted them.)
ARCS = 8

# WHAT THE FACE SAYS ABOUT ITSELF.
#
# The drawings are ours — geometry declared in `assets/icons/marks.nix`, not a
# third party's outlines copied — and they are published under the same licence
# as the rest of the repository. A mark drawn here NAMES a product on this
# machine's own screen, which is what every program's icon does; the marks
# remain the trademarks of whoever owns them.
COPYRIGHT = "The marks this repository draws. MIT, as the repository is."
TRADEMARK = (
    "Each mark names a product and remains the trademark of its owner; the "
    "drawings are this repository's own."
)
LICENCE = (
    "Permission is hereby granted, free of charge, to any person obtaining a "
    "copy of this font and associated documentation files, to deal in them "
    "without restriction, subject to the MIT licence carried by the repository "
    "that generated them."
)


def die(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def circle(pen: TTGlyphPen, cx: float, cy: float, r: float, clockwise: bool) -> None:
    """One closed contour, as quadratic arcs.

    `clockwise` is in the FONT's coordinates, where y grows upwards — so a
    clockwise contour walks decreasing angles.
    """
    step = (2 * math.pi / ARCS) * (-1 if clockwise else 1)
    # The control point of a quadratic arc sits on the bisector, further out
    # than the radius by exactly this much.
    reach = r / math.cos(abs(step) / 2)

    def at(angle: float, radius: float) -> tuple[float, float]:
        # SNAPPED. `sin(-2pi)` is 2.4e-16, not 0, and that residue survives the
        # rounding at some heights and not at others — so the last point of a
        # contour came out a hair off the first, `TTGlyphPen` kept both, and
        # five of the fourteen contours ended with a zero-length segment. A
        # degenerate outline that rasterises fine is still a degenerate
        # outline, and it made the bottom row silently different from the rest.
        snap = lambda v: 0.0 if abs(v) < 1e-9 else v
        return (cx + snap(radius * math.cos(angle)), cy + snap(radius * math.sin(angle)))

    pen.moveTo(at(0.0, r))
    for i in range(ARCS):
        a = i * step
        pen.qCurveTo(at(a + step / 2, reach), at(a + step, r))
    pen.closePath()


def signed_area(points: list[tuple[float, float]]) -> float:
    """The shoelace sum: positive counter-clockwise, negative clockwise."""
    total = 0.0
    for i, (x0, y0) in enumerate(points):
        x1, y1 = points[(i + 1) % len(points)]
        total += x0 * y1 - x1 * y0
    return total / 2.0


def windings(glyph, glyph_set) -> list[float]:
    """The signed area of every contour of a written glyph."""
    coords, ends = glyph.getCoordinates(glyph_set)[0], glyph.endPtsOfContours
    out, start = [], 0
    for end in ends:
        out.append(signed_area([tuple(p) for p in coords[start : end + 1]]))
        start = end + 1
    return out


def reference(font_file: pathlib.Path, codepoint: int, expected: str) -> dict:
    """The em, the advance and the box the marks have to land in."""
    font = TTFont(font_file, lazy=True)
    cmap = font.getBestCmap()
    if codepoint not in cmap:
        die(
            f"the reference glyph U+{codepoint:04X} is not in {font_file.name}.\n"
            "       It is the glyph whose box every mark copies: without it\n"
            "       there is nothing to copy, and a guessed box draws a mark of\n"
            "       another size beside its neighbours."
        )
    glyph_set = font.getGlyphSet()
    name = cmap[codepoint]
    # THE NAME, AND NOT ONLY THE NUMBER. A Nerd Fonts bump moves codepoints —
    # that is the whole reason `glyphnames.json` exists — and a reference given
    # as a bare hex would then silently measure some other glyph's box. The
    # name is what is stable.
    if name != expected:
        die(
            f"U+{codepoint:04X} is `{name}` in {font_file.name} and the mark "
            f"generator was told to measure `{expected}`.\n"
            "       Nerd Fonts moved a codepoint; look up the new one in\n"
            "       glyphnames.json and fix `reference` in nix/marksgen.nix."
        )
    pen = BoundsPen(glyph_set)
    glyph_set[name].draw(pen)
    if pen.bounds is None:
        die(f"the reference glyph {name} has no ink to measure.")
    return {
        "upem": font["head"].unitsPerEm,
        "ascender": font["hhea"].ascent,
        "descender": font["hhea"].descent,
        "advance": int(glyph_set[name].width),
        "bounds": tuple(round(v) for v in pen.bounds),
        "name": name,
    }


def free_range(base: int, how_many: int, others: list[pathlib.Path]) -> None:
    """The destination range has to be empty in ALL the other faces."""
    for face in others:
        cmap = TTFont(face, lazy=True).getBestCmap()
        taken = [cp for cp in range(base, base + how_many) if cp in cmap]
        if taken:
            die(
                f"the range U+{base:04X}..U+{base + how_many - 1:04X} collides with "
                f"{len(taken)} codepoints of {face.name} "
                f"(the first is U+{taken[0]:04X}).\n"
                "       The face asked for by name beats the fallback chain: the\n"
                "       mark would come out drawn as ANOTHER symbol, and there is\n"
                "       no seeing that by looking."
            )


def read_marks(path: pathlib.Path) -> dict:
    """The declaration, checked for the things a grid can get wrong."""
    marks = json.loads(path.read_text())
    if not marks:
        die("assets/icons/marks.nix declares no mark at all.")
    taken: dict[int, str] = {}
    for name, mark in sorted(marks.items()):
        for field in ("box", "first", "step", "radius", "hollow", "rows", "at"):
            if field not in mark:
                die(f"mark `{name}` declares no `{field}`; see assets/icons/marks.nix.")
        rows = mark["rows"]
        side = len(rows)
        if any(len(row) != side for row in rows):
            die(f"mark `{name}`: the grid is {side} rows and is not square.")
        if any(ch not in "*o " for row in rows for ch in row):
            die(f"mark `{name}`: a row has something other than `*`, `o` and a space.")
        if not any(ch != " " for row in rows for ch in row):
            die(f"mark `{name}`: every cell of the grid is empty; there is nothing to draw.")
        reach = mark["first"] + (side - 1) * mark["step"] + mark["radius"]
        if reach > mark["box"]:
            die(
                f"mark `{name}`: the dots reach {reach} on a box of {mark['box']} — "
                "the drawing would be cut off."
            )
        if not 0 <= mark["hollow"] < mark.get("light", mark["radius"]):
            die(
                f"mark `{name}`: the hollow is {mark['hollow']} against a light "
                f"radius of {mark.get('light', mark['radius'])}; it has to be "
                "smaller, and 0 makes a solid dot."
            )
        if not 0 < mark.get("light", mark["radius"]) <= mark["radius"]:
            die(
                f"mark `{name}`: the light radius is {mark.get('light')} against a "
                f"radius of {mark['radius']}; a light dot is never the bigger one."
            )
        # EACH MARK OWNS ITS OWN OFFSET, and that is why it is declared rather
        # than counted. Assigning `base + i` over the sorted names moved every
        # existing mark's codepoint the day a name sorted before it — and
        # `U+F3000` is written by hand in five places that talk about the
        # Tailscale mark. Nothing would have broken loudly; the five would just
        # have started naming the wrong glyph.
        if mark["at"] in taken:
            die(
                f"marks `{taken[mark['at']]}` and `{name}` both declare offset "
                f"{mark['at']}; each one owns its own."
            )
        taken[mark["at"]] = name
    return marks


def draw(mark: dict, box: dict) -> TTGlyphPen:
    """One mark, placed in the reference glyph's own box.

    CENTRED ON WHAT IS DRAWN, and not on the declared grid. A mark may use only
    part of its grid — four dots of a three-by-three — and centring the grid
    then leaves the drawing sitting low or to one side of its box, which on a
    modeline reads as a glyph that has slipped. The cells that draw something
    are what gets centred.
    """
    left, bottom, right, top = box["bounds"]
    side = min(right - left, top - bottom)
    rows = mark["rows"]
    # The ink's own extent on the grid, in grid units.
    used = [
        (r, c)
        for r, row in enumerate(rows)
        for c, ch in enumerate(row)
        if ch != " "
    ]
    r0, r1 = min(r for r, _ in used), max(r for r, _ in used)
    c0, c1 = min(c for _, c in used), max(c for _, c in used)
    reach = mark["radius"]
    wide = (c1 - c0) * mark["step"] + 2 * reach
    tall = (r1 - r0) * mark["step"] + 2 * reach
    # One scale for both axes, so a dot stays a circle.
    scale = side / max(wide, tall, 1)
    # Where the ink's own top-left corner has to land for it to be centred.
    x0 = left + (right - left - wide * scale) / 2 - (mark["first"] + c0 * mark["step"] - reach) * scale
    y0 = bottom + (top - bottom - tall * scale) / 2 - (mark["first"] + (len(rows) - 1 - r1) * mark["step"] - reach) * scale
    pen = TTGlyphPen(None)
    for row_at, row in enumerate(rows):
        for column, kind in enumerate(row):
            cx = x0 + (mark["first"] + column * mark["step"]) * scale
            # THE FIRST ROW IS THE TOP, and the font counts from the bottom.
            up = len(rows) - 1 - row_at
            cy = y0 + (mark["first"] + up * mark["step"]) * scale
            # A LIGHT DOT HAS TWO KNOBS, and only the screen decides between
            # them: it may be SMALLER (`light`) or HOLLOW (`hollow`), or both.
            # A ring is faithful to the drawing at any size; a smaller dot is
            # faithful to its TONE at the size a modeline actually draws.
            # A SPACE IS A CELL OF THE GRID WITH NOTHING IN IT. The grid is
            # what the eye reads — the centres, and the shape they make — and
            # a mark may use only part of it.
            if kind == " ":
                continue
            light = kind == "o"
            r = (mark.get("light", mark["radius"]) if light else mark["radius"]) * scale
            circle(pen, cx, cy, r, clockwise=True)
            if light and mark["hollow"] > 0:
                circle(pen, cx, cy, mark["hollow"] * scale, clockwise=False)
    return pen


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--marks", type=pathlib.Path, required=True, help="marks.json")
    p.add_argument(
        "--like",
        type=pathlib.Path,
        required=True,
        help="the face whose icon box and em-square the marks copy",
    )
    p.add_argument(
        "--reference",
        required=True,
        help="the codepoint of the glyph the marks are measured against, in hexadecimal",
    )
    p.add_argument(
        "--reference-is",
        required=True,
        help="that glyph's name, which is the half that does not move",
    )
    p.add_argument(
        "--other",
        type=pathlib.Path,
        action="append",
        default=[],
        help="a face the range has to be free in; may be repeated",
    )
    p.add_argument("--base", required=True, help="start of the range, in hexadecimal")
    p.add_argument("--out", type=pathlib.Path, required=True)
    args = p.parse_args()

    marks = read_marks(args.marks)
    base = int(args.base, 16)
    free_range(base, max(m["at"] for m in marks.values()) + 1, args.other)
    box = reference(args.like, int(args.reference, 16), args.reference_is)
    print(
        f"OK: advance {box['advance']} and a ceiling of {box['bounds']}, from "
        f"{box['name']} on a {box['upem']}-unit em-square"
    )

    order = [".notdef"] + sorted(marks)
    glyphs, metrics, cmap = {}, {}, {}
    empty = TTGlyphPen(None)
    glyphs[".notdef"] = empty.glyph()
    metrics[".notdef"] = (box["advance"], 0)

    for name in sorted(marks):
        pen = draw(marks[name], box)
        glyphs[name] = pen.glyph()
        metrics[name] = (box["advance"], 0)
        cmap[base + marks[name]["at"]] = name

    builder = FontBuilder(box["upem"], isTTF=True)
    builder.setupGlyphOrder(order)
    builder.setupCharacterMap(cmap)
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=box["ascender"], descent=box["descender"])
    # A FAMILY NAME OF ITS OWN, for the reason `nix/lucidegen.nix` writes down:
    # the renderer builds its bank out of the embedded faces, and a name that
    # could belong to something installed on the machine is a name that can be
    # answered by the wrong face.
    builder.setupNameTable(
        {
            "familyName": "Noctua Marks",
            "styleName": "Regular",
            "uniqueFontIdentifier": "NoctuaMarks-Regular",
            "fullName": "Noctua Marks",
            "psName": "NoctuaMarks-Regular",
            "version": "1.000",
            # THE LICENCE TRAVELS WITH THE FILE. This face is published in a
            # public repository and shipped inside an ISO, and the only place a
            # font can carry its terms is its own name table.
            "copyright": COPYRIGHT,
            "trademark": TRADEMARK,
            "licenseDescription": LICENCE,
            "licenseInfoURL": "https://opensource.org/licenses/MIT",
        }
    )
    # AND THE BITS `FontBuilder` LEAVES AT ZERO, which are not harmless.
    #
    # `fsType` defaults to 4 — "preview and print, editing restricted" — which
    # is the wrong stamp on a face an MIT repository publishes; the sibling
    # face `NoctuaLucide.ttf` carries 0. `fsSelection` at 0 says the face is
    # not even Regular, and `usWinAscent`/`usWinDescent` at 0 is a malformed
    # metric that a shaper other than ours would read.
    builder.setupOS2(
        sTypoAscender=box["ascender"],
        sTypoDescender=box["descender"],
        usWinAscent=box["ascender"],
        usWinDescent=abs(box["descender"]),
        fsType=0,
        fsSelection=0x40,
        achVendID="Noct",
    )
    builder.setupPost()
    args.out.mkdir(parents=True, exist_ok=True)
    face = args.out / "NoctuaMarks.ttf"
    builder.save(face)

    # --- THE GUARDS, over what was WRITTEN ---------------------------------
    written = TTFont(face)
    glyph_table, glyph_set = written["glyf"], written.getGlyphSet()
    out = {}
    for name in sorted(marks):
        cp = base + marks[name]["at"]
        if cp not in written.getBestCmap():
            die(f"mark `{name}` did not reach the cmap of the face just written.")
        pen = BoundsPen(glyph_set)
        glyph_set[name].draw(pen)
        got = tuple(round(v) for v in pen.bounds)
        left, bottom, right, top = box["bounds"]
        if not (got[0] >= left and got[1] >= bottom and got[2] <= right and got[3] <= top):
            die(
                f"mark `{name}` draws at {got}, outside the box the icons beside "
                f"it live in, {box['bounds']}.\n"
                "       A mark may be smaller than its neighbours; wider than the\n"
                "       cell it would draw into the letter next to it."
            )
        if int(glyph_set[name].width) != box["advance"]:
            die(
                f"mark `{name}` advances {glyph_set[name].width} and every icon "
                f"in this font advances {box['advance']}.\n"
                "       The advance is what decides how many cells a glyph takes,\n"
                "       and it is the ONE thing measured to be constant."
            )
        areas = windings(glyph_table[name], glyph_set)
        holes = sum(1 for row in marks[name]["rows"] for ch in row if ch == "o")
        holes = holes if marks[name]["hollow"] > 0 else 0
        got_holes = sum(1 for a in areas if a > 0)
        if got_holes != holes:
            die(
                f"mark `{name}` has {got_holes} contours wound against the outline "
                f"and the drawing asks for {holes}.\n"
                "       TrueType fills by the non-zero rule: a hole wound the same\n"
                "       way as its outer contour is a solid dot, silently."
            )
        out[name] = {"codepoint": cp, "char": chr(cp)}
        print(
            f"OK: ours-{name} -> U+{cp:05X}, {len(areas)} contours, "
            f"{got_holes} of them holes"
        )

    (args.out / "marks.json").write_text(json.dumps(out, indent=2, sort_keys=True) + "\n")
    (args.out / "report.md").write_text(
        "# Marks this repository draws\n\n"
        f"{box['upem']} units to the em-square, advance {box['advance']}, "
        f"box {box['bounds']} — "
        f"copied from `{box['name']}`.\n\n"
        + "".join(f"- `ours-{n}` at `U+{v['codepoint']:05X}`\n" for n, v in sorted(out.items()))
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
