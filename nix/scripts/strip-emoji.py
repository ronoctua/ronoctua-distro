#!/usr/bin/env python3
"""Removes from the PRIMARY face the codepoints the emoji face should draw.

THE PROBLEM
-----------
The primary face is the one asked for by NAME (`Family::Name`), and that is why it
wins the fallback chain on every codepoint it has. The Iosevka has **75** of the
default-presentation emoji — `UP!`, the clock faces, the moon phases, the
wine glass — drawn in monochrome, as text.

The result was not tofu: it was the WRONG emoji. On the symbol browser's screen,
`UP!` came out as a little white box with black letters next to a coloured `ATM`
and `SOON`, and nothing gave the cause away — both "exist" and both "show up".

It was the owner who spotted it, in a screenshot, and the same class of defect had
already cost the entire Lucide remapping: **whoever has the codepoint first wins**.

THE RULE, WHICH IS UNICODE'S AND NOT OURS
-----------------------------------------
`Emoji_Presentation=Yes` means "the default form of this character is the emoji".
Those, and ONLY those, are what the emoji face should draw without being asked.

What stays in the primary face is the TEXT-presentation set: `#`, `*`, the digits
(keycap bases), `©`, `®`, `™`, `‼`, the arrows. They only become emoji with a
following `U+FE0F`, and drawing them coloured by default would break ordinary text.

Measured on 17.0.0: 154 codepoints in both faces, of which **75** are
emoji-presentation (they leave) and **79** text-presentation (they stay).

WHY NOT THE VARIATION SELECTOR
------------------------------
Because it does not work. Tested: `U+1F199 U+FE0F` stays in the Iosevka and becomes TWO
glyphs, because the primary face has both codepoints and `cosmic-text` does not
implement the "prefer the colour font when FE0F is present" rule. Getting one
glyph too many is worse than getting the wrong glyph.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

from fontTools.ttLib import TTFont


def die(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def emoji_presentation(emoji_data: pathlib.Path) -> set[int]:
    """The codepoints with `Emoji_Presentation=Yes`, from `emoji-data.txt`."""
    not_in: set[int] = set()
    for line in emoji_data.read_text(encoding="utf-8").splitlines():
        body = line.split("#")[0]
        if "Emoji_Presentation" not in body:
            continue
        range_of = body.split(";")[0].strip()
        if not range_of:
            continue
        if ".." in range_of:
            a, b = (int(x, 16) for x in range_of.split(".."))
            not_in.update(range(a, b + 1))
        else:
            not_in.add(int(range_of, 16))
    if not not_in:
        die(f"{emoji_data.name} declared no Emoji_Presentation at all")
    return not_in


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--primary", type=pathlib.Path, required=True)
    p.add_argument("--emoji", type=pathlib.Path, required=True)
    p.add_argument("--emoji-data", type=pathlib.Path, required=True)
    p.add_argument("--out", type=pathlib.Path, required=True)
    args = p.parse_args()

    font_file = TTFont(args.primary)
    of_emoji = set(TTFont(args.emoji, lazy=True).getBestCmap())
    presented = emoji_presentation(args.emoji_data)

    prior = set(font_file.getBestCmap())
    give_way = (prior & of_emoji) & presented
    if not give_way:
        die(
            "no codepoint to give_way — either the Iosevka stopped carrying emoji, or\n"
            "       reading emoji-data.txt broke. In either case, carrying on\n"
            "       from here would write a face identical to the input without saying so."
        )

    # IT LEAVES EVERY subtable, and not just `getBestCmap`: a font has
    # several (format 4 for the BMP, 12 for the rest), and leaving the codepoint in
    # one of them would return the wrong glyph in any reader that preferred it.
    for table in font_file["cmap"].tables:
        for cp in list(table.cmap):
            if cp in give_way:
                del table.cmap[cp]

    args.out.parent.mkdir(parents=True, exist_ok=True)
    font_file.save(args.out)

    # --- THE GUARDS, over what was WRITTEN -----------------------------------
    written = set(TTFont(args.out, lazy=True).getBestCmap())

    leftover = (written & of_emoji) & presented
    if leftover:
        die(
            f"{len(leftover)} emoji-presentation codepoints are still in the face "
            f"primary (the first is U+{min(leftover):04X}).\n"
            "       They would be drawn monochrome, and the screen would not have "
            "como denunciar."
        )

    # WHAT MUST NOT HAVE LEFT: TEXT presentation. Losing `#` or the digits
    # from the primary face would break ordinary text, and the symptom would show up far away.
    lost = (prior - written) - presented
    if lost:
        die(
            f"{len(lost)} codepoints with TEXT presentation left the primary "
            f"face: {sorted(f'U+{c:04X}' for c in lost)[:8]}"
        )

    # And the canonical ones, by name, because an empty set also passes the test
    # above — and a bug that zeroed `presented` would take everything with it.
    for cp, name in ((0x23, "#"), (0x30, "0"), (0xA9, "(c)"), (0x2122, "(tm)")):
        if cp not in written:
            die(f"the primary face lost `{name}` (U+{cp:04X})")

    print(
        f"OK: {len(give_way)} codepoints given to the emoji face; "
        f"{len(prior & of_emoji) - len(give_way)} with text presentation stayed"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
