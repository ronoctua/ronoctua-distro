#!/usr/bin/env python3
"""Normalises the naming of the noctua-iosevka family.

Reason
------
The Nerd Fonts patcher swallowed the "Semi" of "SemiExtended" into the family
name, throwing 27 of the 135 patched faces into NINE invented typographic
families:

    NoctuaIosevkaBoldSemi Nerd Font Mono, NoctuaIosevkaThinSemi Nerd Font Mono, ...

Practical result: asking fontconfig or fontdb for "NoctuaIosevka Nerd Font Mono"
never returns SemiExtended. The UNPATCHED files do not have the problem.

Fixing this naively creates a second failure: the 27 SemiExtended faces
have nameID 17 in {Extended, Extended Italic, Extended Oblique} with
usWidthClass=6, and the 27 genuine Extended faces have EXACTLY the same three
nameID 17 values with usWidthClass=7. Today they do not collide because they are in
different families; once the family is unified, they do.

Approach
--------
We do not patch the 27 files: we recompute the naming of all 135 faces as a
PURE FUNCTION of the metrics (usWeightClass, usWidthClass, fsSelection). That
normalises everything consistently and verifiably, instead of matching string
patterns. The slant discrimination is exact:

    fsSelection bit 9 (OBLIQUE) -> Oblique
    fsSelection bit 0 (ITALIC)  -> Italic
    neither of the two          -> upright

nameID 6 (PostScript) is preserved: it is already unique among the 135 faces, and
touching it would break references by PostScript name.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
from collections import Counter, defaultdict

from fontTools.ttLib import TTFont

# --- canonical tables -------------------------------------------------------

WEIGHTS = {
    100: "Thin",
    200: "ExtraLight",
    300: "Light",
    400: "",  # Regular: implicit
    500: "Medium",
    600: "SemiBold",
    700: "Bold",
    800: "ExtraBold",
    900: "Heavy",
}

WIDTHS = {
    3: "Condensed",
    4: "SemiCondensed",
    5: "",  # Normal: implicit
    6: "SemiExtended",
    7: "Extended",
}

FS_ITALIC = 1 << 0
FS_OBLIQUE = 1 << 9

# The four legacy RIBBI slots. Outside them, nameID 1 carries the rest of the name.
RIBBI = {
    ("", ""): "Regular",
    ("Bold", ""): "Bold",
    ("", "Italic"): "Italic",
    ("Bold", "Italic"): "Bold Italic",
}

# Records to write: Macintosh and Windows, both in English.
PLATFORMS = [(1, 0, 0), (3, 1, 0x409)]


def slant_of(fs_selection: int) -> str:
    """Slant from fsSelection. Oblique takes precedence: the Oblique files
    do NOT set the ITALIC bit, but checking OBLIQUE first keeps the function
    correct even if one day they set both."""
    if fs_selection & FS_OBLIQUE:
        return "Oblique"
    if fs_selection & FS_ITALIC:
        return "Italic"
    return ""


def canonical_names(family_base: str, weight: int, width: int, slant: str) -> dict:
    """A face's canonical naming. A pure function of the metrics."""
    w = WEIGHTS[weight]
    d = WIDTHS[width]

    # nameID 16/17 — typographic family: ONE family, the rest becomes subfamily.
    typo_family = family_base
    typo_subfamily = " ".join(x for x in (d, w, slant) if x) or "Regular"

    # nameID 1/2 — legacy RIBBI. If the combination fits one of the four slots,
    # it uses the base family; otherwise the family carries weight/width/oblique and the
    # subfamily keeps whatever is left of RIBBI.
    ribbi_key = (w if w == "Bold" else "", "Italic" if slant == "Italic" else "")
    if not d and slant != "Oblique" and ribbi_key in RIBBI and w in ("", "Bold"):
        legacy_family = family_base
        legacy_subfamily = RIBBI[ribbi_key]
    else:
        # Oblique does not exist in RIBBI: it goes into the family name and the
        # subfamily keeps only the bold/italic axis.
        extra = " ".join(x for x in (d, w, "Oblique" if slant == "Oblique" else "") if x)
        legacy_family = f"{family_base} {extra}".strip()
        if w == "Bold" and slant == "Italic":
            legacy_subfamily = "Bold Italic"
        elif w == "Bold":
            legacy_subfamily = "Bold"
        elif slant == "Italic":
            legacy_subfamily = "Italic"
        else:
            legacy_subfamily = "Regular"
        # Bold is already in the subfamily; do not repeat it in the family.
        if w == "Bold":
            legacy_family = f"{family_base} {' '.join(x for x in (d, 'Oblique' if slant == 'Oblique' else '') if x)}".strip()

    full = f"{typo_family} {typo_subfamily}".strip()
    if typo_subfamily == "Regular":
        full = typo_family

    return {
        "family": legacy_family,
        "subfamily": legacy_subfamily,
        "full": full,
        "typo_family": typo_family,
        "typo_subfamily": typo_subfamily,
    }


def set_name(font: TTFont, name_id: int, value: str) -> None:
    """Writes a nameID into every platform record."""
    name_table = font["name"]
    for plat_id, enc_id, lang_id in PLATFORMS:
        name_table.setName(value, name_id, plat_id, enc_id, lang_id)


def process(path: pathlib.Path, family_base: str, write: bool = True) -> dict:
    # lazy=True is far cheaper and is enough to read the name table. We only open
    # in full mode when there is actually something to write.
    font = TTFont(path, lazy=True)
    os2 = font["OS/2"]
    weight = os2.usWeightClass
    width = os2.usWidthClass

    if weight not in WEIGHTS:
        raise SystemExit(f"{path.name}: usWeightClass {weight} is outside the canonical table")
    if width not in WIDTHS:
        raise SystemExit(f"{path.name}: usWidthClass {width} is outside the canonical table")

    slant = slant_of(os2.fsSelection)
    names = canonical_names(family_base, weight, width, slant)

    name_table = font["name"]
    before_typo_family = name_table.getDebugName(16) or name_table.getDebugName(1)
    ps_name = name_table.getDebugName(6)

    # Which nameIDs actually diverge from the canonical one.
    wanted = {
        1: names["family"],
        2: names["subfamily"],
        4: names["full"],
        16: names["typo_family"],
        17: names["typo_subfamily"],
    }
    differs = any(name_table.getDebugName(nid) != want for nid, want in wanted.items())
    font.close()

    # It ONLY rewrites what changes. Without this, the build rewrote all 540 files
    # even when 486 of them were already correct — and a WOFF2 costs ~49 s
    # because fontTools recompresses the whole font with brotli. Rewriting
    # everything took 3.7 hours; rewriting only what changes takes seconds.
    if write and differs:
        full = TTFont(path)
        for nid, want in wanted.items():
            set_name(full, nid, want)
        # nameID 6 (PostScript) preserved on purpose: it is already unique.
        full.save(path)
        full.close()

    return {
        "file": path.name,
        "weight": weight,
        "width": width,
        "slant": slant or "upright",
        "was_typo_family": before_typo_family,
        "typo_family": names["typo_family"],
        "typo_subfamily": names["typo_subfamily"],
        "postscript": ps_name,
        "changed": differs,
        "written": bool(write and differs),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("directory", type=pathlib.Path)
    ap.add_argument("--family", required=True, help='ex.: "NoctuaIosevka Nerd Font Mono"')
    ap.add_argument("--expect-count", type=int, default=135)
    ap.add_argument("--report", type=pathlib.Path)
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="works out and runs the strict assertions without writing to the files",
    )
    ap.add_argument(
        "--inventory",
        action="store_true",
        help=(
            "only inventories: it does not write and does NOT require a single family. "
            "It is the WOFF2 mode — see the note in nix/fonts.nix on why "
            "they are not corrected."
        ),
    )
    args = ap.parse_args()

    files = sorted(
        p for p in args.directory.iterdir() if p.suffix.lower() in (".ttf", ".otf", ".woff2")
    )
    if len(files) != args.expect_count:
        print(
            f"ERROR: expected {args.expect_count} files in {args.directory}, found {len(files)}",
            file=sys.stderr,
        )
        return 1

    write = not (args.dry_run or args.inventory)
    rows = [process(p, args.family, write) for p in files]

    # --- assertions ---------------------------------------------------------
    # In inventory mode the fonts are NOT fixed, so demanding a unique family
    # would be a lie. The remaining invariants still hold.
    if not args.inventory:
        families = {r["typo_family"] for r in rows}
        if families != {args.family}:
            print(f"ERROR: expected 1 typographic family ({args.family!r}),", file=sys.stderr)
            print(f"      achei {len(families)}: {sorted(families)}", file=sys.stderr)
            return 1

    pairs = Counter((r["typo_family"], r["typo_subfamily"]) for r in rows)
    dups = {k: v for k, v in pairs.items() if v > 1}
    if dups:
        print("ERROR: duplicated (family, subfamily) pairs — a collision:", file=sys.stderr)
        for (fam, sub), n in sorted(dups.items()):
            print(f"      {n}x  {fam!r} / {sub!r}", file=sys.stderr)
            for r in rows:
                if (r["typo_family"], r["typo_subfamily"]) == (fam, sub):
                    print(
                        f"           {r['file']}  (weight={r['weight']} width={r['width']} {r['slant']})",
                        file=sys.stderr,
                    )
        return 1

    ps_names = Counter(r["postscript"] for r in rows)
    ps_dups = {k: v for k, v in ps_names.items() if v > 1}
    if ps_dups:
        print(f"ERROR: duplicated PostScript names: {ps_dups}", file=sys.stderr)
        return 1

    changed = sum(1 for r in rows if r["changed"])
    written = sum(1 for r in rows if r["written"])
    by_family = defaultdict(int)
    for r in rows:
        by_family[r["was_typo_family"]] += 1

    if args.inventory:
        print(f"{args.directory.name}: {len(rows)} faces inventoried [no correction, by design]")
        print(f"  typographic families present:    {len(by_family)}")
        for fam, n in sorted(by_family.items()):
            print(f"      {n:3d}x  {fam}")
        print(f"  PostScript names:                {len(ps_names)} distinct  [OK]")
    else:
        dry_note = " [dry-run: nothing written]" if args.dry_run else ""
        print(f"{args.directory.name}: {len(rows)} faces normalised into {args.family!r}{dry_note}")
        print(f"  source typographic families:     {len(by_family)}")
        print(f"  faces that diverged from the canonical one: {changed}")
        print(f"  faces actually rewritten:        {written}")
        print(f"  (family, subfamily) pairs:       {len(pairs)} distinct  [OK]")
        print(f"  PostScript names:                {len(ps_names)} distinct  [OK]")

    if args.report:
        args.report.write_text(
            json.dumps(
                {
                    "family": args.family,
                    "count": len(rows),
                    "source_families": dict(sorted(by_family.items())),
                    "corrected": changed,
                    "faces": rows,
                },
                indent=2,
                ensure_ascii=False,
            )
        )

    return 0


if __name__ == "__main__":
    sys.exit(main())
