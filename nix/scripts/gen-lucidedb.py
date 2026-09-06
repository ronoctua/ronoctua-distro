#!/usr/bin/env python3
"""Generator of the Lucide icon browser's database.

THE TWO AUTHORITIES
-------------------
    icons/<name>.json  -> authority on the SET: which icons exist,
                          with which tags, categories and aliases
    icons/<name>.svg   -> authority on the DRAWING, and it is where every
                          copy output comes from (the SVG, the JSX, the data URI)
    Lucide.ttf         -> authority on what can be SHOWN on screen

The `.ttf` is not a fourth source of truth: it only answers "which glyph draws
this icon". If an icon in the set has no glyph, the build stops — the
alternative would be an empty cell in the grid with nobody knowing why.

WHY THE FONT IS REMAPPED
------------------------
Lucide lives at `U+E038..U+E6BB`, and the project's Iosevka is patched with
Nerd Fonts, which occupies **634 of those same codepoints**. The primary face
always wins the fallback chain, so 38% of the icons would come out drawn
as another symbol — not as tofu, which you see, but as the WRONG icon, which
you do not.

This generator rewrites the `cmap` into a free range of plane 15 and CHECKS that
it is free in the other embedded faces. The source codepoint travels along,
in `lcp`, because it is what serves whoever pastes into a page with the real Lucide
font — they are two different facts, from two different sources, and not two
copies of one.

OUTPUTS
-------
    lucide.json       the browser's database
    NoctuaLucide.ttf  the remapped face, which the renderer embeds
    coverage.md      report: translation coverage, work queue
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib
from collections import Counter

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable

# THE LEXICON'S CONTRACT IS SHARED with the other two generators.
from lexicon import SOURCE, STOPWORDS, search_forms, subtag_of, translate, words_of

# The elements Lucide's SVGs use, and each one's attributes.
#
# THE LIST IS CLOSED ON PURPOSE. Measured on 0.563.0: 7 elements, 15
# attributes, and nothing else. Accepting whatever comes would make the copy emit,
# silently, an SVG that does not draw what the screen shows; rejecting the unknown
# makes the next version of Lucide STOP the build, which is where it can be fixed.
ELEMENTS = {
    "path": ("d", "fill"),
    "circle": ("cx", "cy", "r", "fill"),
    "ellipse": ("cx", "cy", "rx", "ry", "fill"),
    "rect": ("x", "y", "width", "height", "rx", "ry", "fill"),
    "line": ("x1", "y1", "x2", "y2"),
    "polyline": ("points", "fill"),
    "polygon": ("points", "fill"),
}

# The `<svg>`'s own attributes — they do NOT travel in the database: they are the
# customisation, and what assembles them is the buffer, with the values the person chose.
DO_SVG = {
    "xmlns",
    "class",
    "width",
    "height",
    "viewBox",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
}

# The separators of the `nodes` encoding.
#
# The buffer has no JSON parser (invariant 11), and the symbol database has already
# shown the cost of forgetting that. Measured: Lucide's attribute values
# use only ` ,-.0123456789` and path command letters. None of these three
# appears there, and the `check_separators` guard rejects the version in which one does.
BETWEEN_NODES = ";"
BETWEEN_ATTRIBUTES = "|"
BETWEEN_KEY_AND_VALUE = "="

# What would break the component's parser, which finds the end of a string value
# by looking for the next quote and splits lists on commas.
FORBIDDEN = '"[]{},'


def die(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def safe(s: str) -> str:
    """Strips from a value the punctuation that would break the component's parser.

    It removes rather than escapes: the component's parser does not understand escaping, and one
    tag more in the search does no harm at all. Same function as the symbol generator's, and
    for the same reason.
    """
    return "".join(" " if c in FORBIDDEN else c for c in s).strip()


# ---------------------------------------------------------------------------
# The SVGs
# ---------------------------------------------------------------------------
def ler_svg(path: pathlib.Path) -> list[tuple[str, list[tuple[str, str]]]]:
    """A Lucide `.svg` becomes its list of nodes.

    Regex and not XML on purpose: the format is machine-generated, always the same,
    and a complete parser would accept shapes Lucide does not emit — which
    would hide precisely the change this generator needs to flag.
    """
    raw_text = path.read_text(encoding="utf-8")
    nodes: list[tuple[str, list[tuple[str, str]]]] = []
    for m in re.finditer(r"<([a-z]+)\b([^>]*?)/?>", raw_text):
        element = m.group(1)
        if element == "svg":
            continue
        if element not in ELEMENTS:
            die(
                f"{path.name}: unknown SVG element <{element}>.\n"
                f"       The accepted ones are {sorted(ELEMENTS)}.\n"
                "       A new version of Lucide started using something else, and\n"
                "       accepting it in silence would make the copy emit an SVG that\n"
                "       does not draw what the screen shows."
            )
        attributes = []
        for key, value in re.findall(r'([\w:-]+)="([^"]*)"', m.group(2)):
            if key in DO_SVG and key not in ELEMENTS[element]:
                continue
            if key not in ELEMENTS[element]:
                die(
                    f"{path.name}: an unexpected attribute `{key}` in "
                    f"<{element}>.\n"
                    f"       The accepted ones are {list(ELEMENTS[element])}."
                )
            # THE COMMA BECOMES A SPACE. In the SVG grammar the two are the SAME
            # separator (`wsp | comma-wsp`), so the drawing does not change; and the
            # comma is the only character in the inventory that would break the
            # component parser's list. Measured: one file in 1,668 uses it.
            attributes.append((key, value.replace(",", " ")))
        nodes.append((element, attributes))
    if not nodes:
        die(f"{path.name}: no drawing node at all.")
    return nodes


def codificar(nodes) -> str:
    """The nodes become ONE string, which is how the component receives them."""
    parts = []
    for element, attributes in nodes:
        chunk = [element]
        chunk += [f"{k}{BETWEEN_KEY_AND_VALUE}{v}" for k, v in attributes]
        parts.append(BETWEEN_ATTRIBUTES.join(chunk))
    return BETWEEN_NODES.join(parts)


def check_separators(nodes, name: str) -> None:
    """No value may contain what separates the values.

    Without this the failure is mute: the component cuts the `d` in half and draws —
    or copies — half an icon, and nothing in the build flags it.
    """
    for element, attributes in nodes:
        for key, value in attributes:
            for c in (BETWEEN_NODES, BETWEEN_ATTRIBUTES, BETWEEN_KEY_AND_VALUE, *FORBIDDEN):
                if c in value:
                    die(
                        f"{name}: the attribute `{key}` of <{element}> contains "
                        f"`{c}`, which is a separator of the encoding.\n"
                        "       The component would cut the value there, and the copy "
                        "would come out truncated, in silence."
                    )


# ---------------------------------------------------------------------------
# The font
# ---------------------------------------------------------------------------
def font_glyphs(font_file: TTFont) -> dict[str, list[int]]:
    """Glyph name -> the PUA codepoints that reach it."""
    by_name: dict[str, list[int]] = {}
    for cp, glyph in sorted(font_file.getBestCmap().items()):
        if cp >= 0xE000:
            by_name.setdefault(glyph, []).append(cp)
    return by_name


def match_glyphs(name_list, nodes_by_name, by_name) -> dict[str, str]:
    """Icon -> the name of the glyph that draws it.

    # The twins

    The tool that generates the font merges IDENTICAL drawings into a single glyph, and the
    surviving name is one of theirs: on 0.563.0, `clock` and `clock-4` have the
    same stroke and the font keeps only `clock-4`. Untreated, `clock` would be left
    with no glyph.

    The rule is general and comes from the DRAWING, not from a list of exceptions: whoever is
    left without a glyph looks for the icon whose nodes are identical to its own and uses its glyph. A
    version that merges another pair keeps working; one that loses a real icon
    stops the build.
    """
    by_drawing: dict[str, list[str]] = {}
    for name in name_list:
        by_drawing.setdefault(json.dumps(nodes_by_name[name]), []).append(name)

    not_in: dict[str, str] = {}
    missing = []
    for name in name_list:
        if name in by_name:
            not_in[name] = name
            continue
        twins = [
            other
            for other in by_drawing[json.dumps(nodes_by_name[name])]
            if other != name and other in by_name
        ]
        if twins:
            not_in[name] = twins[0]
        else:
            missing.append(name)
    if missing:
        die(
            f"{len(missing)} icons of the set have no glyph in the font: "
            f"{missing[:10]}\n"
            "       The font and the icon set come from different versions.\n"
            "       Check the version declared in nix/lucidegen.nix."
        )
    return not_in


def free_range(base: int, how_many: int, others: list[pathlib.Path]) -> None:
    """The destination range has to be empty in ALL the other faces.

    This is the guard that exists because of the measured defect: the source Lucide
    collides with 634 Nerd Fonts codepoints in the Iosevka, and the primary face wins
    the fallback. The symptom is not tofu — it is the WRONG icon drawn with
    confidence, which nobody notices by looking.
    """
    for face in others:
        font_file = TTFont(face, lazy=True)
        cmap = font_file.getBestCmap()
        taken = [cp for cp in range(base, base + how_many) if cp in cmap]
        if taken:
            die(
                f"the range U+{base:04X}..U+{base + how_many - 1:04X} collides with "
                f"{len(taken)} codepoints of {face.name} "
                f"(the first is U+{taken[0]:04X}).\n"
                "       The primary face beats the fallback chain: these\n"
                "       icons would come out drawn as ANOTHER symbol, which\n"
                "       there is no seeing it by looking."
            )


def remapear(entry: pathlib.Path, out_path: pathlib.Path, assignment: dict[str, int]) -> None:
    """Writes the face with the new `cmap`, and only that.

    GSUB goes out along with it: the source font matches the icon's name by ligature
    (`home` -> the little house), and a live ligature would turn the word `home` written
    anywhere in the system into a drawing.
    """
    font_file = TTFont(entry)
    subtabela = CmapSubtable.newSubtable(12)
    subtabela.platformID, subtabela.platEncID = 3, 10
    subtabela.format, subtabela.reserved = 12, 0
    subtabela.length, subtabela.language, subtabela.nGroups = 0, 0, 0
    subtabela.cmap = {cp: glyph for glyph, cp in assignment.items()}

    cmap = newTable("cmap")
    cmap.tableVersion = 0
    cmap.tables = [subtabela]
    font_file["cmap"] = cmap
    for table in ("GSUB", "GPOS"):
        if table in font_file:
            del font_file[table]

    # A FAMILY NAME OF ITS OWN. Without it the face would be called `lucide` and would
    # compete with a Lucide installed on the system — and the renderer builds its bank from
    # the embedded faces, so the name has to be unmistakable.
    name_list = font_file["name"]
    name_list.names = []
    for identifier, value in (
        (1, "Noctua Lucide"),
        (2, "Regular"),
        (3, "Noctua Lucide"),
        (4, "Noctua Lucide"),
        (6, "NoctuaLucide-Regular"),
    ):
        name_list.setName(value, identifier, 3, 1, 0x409)
    font_file.save(out_path)


# ---------------------------------------------------------------------------
def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--icons", type=pathlib.Path, required=True)
    p.add_argument("--font", type=pathlib.Path, required=True)
    p.add_argument("--lexicon", type=pathlib.Path, required=True)
    # The languages, from `assets/i18n/languages.nix`. The tags are built for
    # every one of them; the lexicon is keyed on the subtag.
    p.add_argument("--languages", required=True, nargs="+")
    p.add_argument(
        "--other",
        type=pathlib.Path,
        action="append",
        default=[],
        help="the embedded face the target range must not invade",
    )
    p.add_argument("--base", required=True, help="start of the range, in hexadecimal")
    p.add_argument("--min-coverage", type=float, default=0.0)
    p.add_argument("--cap-no-pt", type=int, default=0)
    p.add_argument("--out", type=pathlib.Path, required=True)
    args = p.parse_args()

    base = int(args.base, 16)
    args.out.mkdir(parents=True, exist_ok=True)

    # --- the set ------------------------------------------------------------
    # BY NAME, and not by file name: sorting `clock-1.json` against
    # `clock.json` compares `-` with `.` and interleaves what the grid shows in
    # alphabetical order. The grid is a list one walks with the eye.
    name_list = sorted(m.stem for m in args.icons.glob("*.json"))
    if not name_list:
        die(f"no icon in {args.icons}")

    nodes_by_name = {}
    for name in name_list:
        svg = args.icons / f"{name}.svg"
        if not svg.exists():
            die(f"{name}: it has metadata and no drawing.")
        nodes = ler_svg(svg)
        check_separators(nodes, name)
        nodes_by_name[name] = nodes

    # --- the font -----------------------------------------------------------
    font_file = TTFont(args.font, lazy=True)
    by_name = font_glyphs(font_file)
    glyph_of = match_glyphs(name_list, nodes_by_name, by_name)

    extra = set(by_name) - {glyph_of[n] for n in name_list}
    if extra:
        die(
            f"{len(extra)} glyphs of the font have no icon in the set: "
            f"{sorted(extra)[:10]}\n"
            "       The font and the set come from different versions."
        )

    free_range(base, len(name_list), args.other)
    assignment = {glyph_of[name]: base + i for i, name in enumerate(name_list)}
    remapear(args.font, args.out / "NoctuaLucide.ttf", assignment)

    # EACH ICON'S SOURCE CODEPOINT, twins included.
    #
    # A shared glyph has TWO codepoints in the source font, and each
    # icon has to carry its own — otherwise copying the character sends both to
    # the same drawing, and whoever pastes `clock` gets `clock-4`. The order lines up
    # because the font assigns the PUA in name order; measured on 0.563.0, where
    # `clock` sits between `clipboard-list` and `cloud`, and `clock-4` between
    # `clock-3` and `clock-5`.
    source: dict[str, int] = {}
    for glyph, cps in by_name.items():
        donos = sorted(n for n in name_list if glyph_of[n] == glyph)
        for owner, cp in zip(donos, cps):
            source[owner] = cp
        for owner in donos[len(cps) :]:
            source[owner] = cps[-1]

    # --- the tags -----------------------------------------------------------
    lexicon = tomllib.loads(args.lexicon.read_text(encoding="utf-8"))
    subtags = [subtag_of(l) for l in args.languages]
    no_translation: Counter[str] = Counter()
    without_pt = []
    categories: Counter[str] = Counter()
    records = []
    total_aliases = 0
    # THE COVERAGE DENOMINATOR, counted where the tokens are counted. Deriving it
    # from the output tags would give a different number: `translate` deduplicates into sets
    # and adds synonyms, so the sum would be over a population that is not
    # the one we want to measure.
    occurrences_total = 0

    for i, name in enumerate(name_list):
        meta = json.loads((args.icons / f"{name}.json").read_text(encoding="utf-8"))
        cats = [safe(c) for c in meta.get("categories", [])]
        categories.update(cats)
        # THE ALIAS IS A NAME, and not a tag: whoever searches for `home` wants the little house, and
        # Lucide calls that an alias with a deprecation warning. It enters the
        # search with the same weight as the name (see `word_score` in the buffer).
        aliases_of = []
        for alias in meta.get("aliases", []):
            aliases_of.append(safe(alias["name"] if isinstance(alias, dict) else alias))
        total_aliases += len(aliases_of)

        # An icon's VOCABULARY: the words of the name, of the aliases, of
        # the categories and the tags Lucide wrote.
        tokens = list(words_of(name))
        for a in aliases_of:
            tokens += words_of(a)
        for c in cats:
            tokens += words_of(c)
        for t in meta.get("tags", []):
            tokens += words_of(t)

        occurrences_total += sum(1 for t in tokens if t not in STOPWORDS)
        tags, found = translate(tokens, lexicon, subtags, no_translation)
        # The SOURCE language is never "untranslated": the token IS the English
        # word. What gets reported is every other language.
        if not all(found[l] for l in subtags if l != SOURCE):
            without_pt.append(name)
        search_terms = search_forms(tags)

        record = {
            "name": name,
            # WHAT THE SCREEN DRAWS — the remapped face's codepoint.
            "cp": base + i,
            # WHAT LUCIDE PUBLISHES — it serves whoever pastes into a page with
            # their font. Two facts, from two sources; not two copies of one.
            "lcp": source[name],
            "cats": cats,
            "nodes": codificar(nodes_by_name[name]),
        }
        if aliases_of:
            record["aliases"] = aliases_of
        # ONE FIELD PER LANGUAGE, and not a nested object: the buffer has no
        # JSON parser (invariant 11) — it finds `"name":[` and reads to the
        # next `]`. A nested map would be read as one list of garbage.
        for language, words in tags.items():
            record[f"tags_{language}"] = sorted(words)
        if search_terms:
            record["search"] = sorted(search_terms)
        records.append(record)

    (args.out / "lucide.json").write_text(
        json.dumps(records, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )

    occurrences = sum(no_translation.values())
    kinds = len(no_translation)
    coverage = (
        100.0 * (1 - occurrences / occurrences_total) if occurrences_total else 100.0
    )

    raw_text = [
        "# Lucide icon database coverage",
        "",
        "Generated by `nix/scripts/gen-lucidedb.py`. Do not edit by hand.",
        "",
        f"- Icons in the database: **{len(records)}**",
        f"- Categories: **{len(categories)}**",
        f"- Aliases: **{total_aliases}**",
        f"- Drawing range: **U+{base:04X}..U+{base + len(records) - 1:04X}**",
        f"- With NO Portuguese word at all: **{len(without_pt)}**",
        "",
        "## By category",
        "",
        "| category | icons |",
        "|---|---:|",
    ]
    for category, n in categories.most_common():
        raw_text.append(f"| `{category}` | {n} |")
    raw_text += [
        "",
        f"## Tag translation: {coverage:.1f}%",
        "",
        f"Required floor: **>= {args.min_coverage}%**.",
        "",
        "## Untranslated words, by frequency",
        "",
        "This is the queue for growing `assets/lexicon/tags.en-pt.toml`.",
        "Translate in this order, not in whatever order comes along — the same lexicon serves",
        "all three browsers.",
        "",
        "| word | occurrences |",
        "|---|---:|",
    ]
    for word, n in no_translation.most_common(120):
        raw_text.append(f"| `{word}` | {n} |")
    if len(no_translation) > 120:
        raw_text.append(f"| … | {len(no_translation) - 120} more words |")
    (args.out / "coverage.md").write_text("\n".join(raw_text) + "\n", encoding="utf-8")

    print(
        f"OK: {len(records)} icons indexed "
        f"({len(categories)} categories, {total_aliases} aliases, "
        f"range U+{base:04X}..U+{base + len(records) - 1:04X})"
    )

    if len(without_pt) > args.cap_no_pt:
        print("", file=sys.stderr)
        print(
            f"ERROR: {len(without_pt)} icons with NO Portuguese tag at all, "
            f"and the ceiling is {args.cap_no_pt}.",
            file=sys.stderr,
        )
        for name in without_pt[:12]:
            print(f"       {name}", file=sys.stderr)
        if len(without_pt) > 12:
            print(f"       … and {len(without_pt) - 12} more", file=sys.stderr)
        print("       Anyone searching in Portuguese does NOT find these icons.", file=sys.stderr)
        print("       The queue, by frequency, is in coverage.md.", file=sys.stderr)
        sys.exit(1)

    if coverage < args.min_coverage:
        print("", file=sys.stderr)
        print(
            f"ERROR: translation coverage at {coverage:.1f}%, "
            f"below the floor of {args.min_coverage}%.",
            file=sys.stderr,
        )
        print("       The work queue is in coverage.md.", file=sys.stderr)
        sys.exit(1)

    print(
        f"OK: tag translation at {coverage:.1f}% "
        f"({kinds} distinct words with no translation)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
