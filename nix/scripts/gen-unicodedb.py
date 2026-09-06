#!/usr/bin/env python3
"""Generator of the Unicode symbol browser's database.

THE TWO AUTHORITIES
-------------------
    UnicodeData.txt  -> authority on the NAMES and the properties
    the .ttf file    -> authority on what actually EXISTS

The database is the INTERSECTION of the two, and that is not thrift: the renderer embeds
ONE face and has no fallback chain (section 6.4, deliberate). A character
the font does not draw would show up as tofu — silent fallback in
different clothes. Showing only what can be seen is the only honest form.

The font read here is the UNPATCHED one: the codepoints Nerd Fonts adds
are in the PUA, already have their own browser (`gen-glyphdb.py`), and have no
Unicode name. Whatever is left of the Iosevka's own PUA also goes, because
`UnicodeData.txt` does not name private-use characters.

OUTPUTS
-------
    unicode.json    the browser's database
    coverage.md    report: translation coverage, work queue
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib
from collections import Counter, defaultdict

import xml.etree.ElementTree as ET

import uharfbuzz as hb
from fontTools.ttLib import TTFont

# THE LEXICON'S CONTRACT IS SHARED with the glyph generator.
from lexicon import SOURCE, search_forms, subtag_of, translate, words_of

# The base a combining mark is drawn on top of.
#
# # Why it comes from here, and not from the component
#
# Because 312 characters of the font have ZERO advance: drawn on their own in a
# grid they take up no room — they vanish, or paint over their neighbour. The
# Unicode convention is to show them over a dotted circle.
#
# The component cannot write that codepoint: invariant 9 forbids a
# codepoint outside the tokens, and 11 does not let the buffer receive a token value.
# So what assembles the display string is the GENERATOR, and the buffer only draws
# what it was given.
DOTTED_CIRCLE = "◌"

# General category -> family, with the name the filter panel shows.
#
# THESE NAMES ARE THE SAME ONES THE `.ftl` SHOWS. There they are keys (because they
# get translated); here they become search TAGS. They are two representations of the same
# fact, and what keeps the two from diverging is the `symbols-filters` check: the
# label the panel shows has to find, when typed into the search, the whole
# family.
#
# Unicode's 25 general categories become SEVEN families because nobody
# searches for "Sk" or for "Modifier Symbol" — one searches for "simbolo". The
# separator and the control fall into "other": that is 27 characters in all, and a
# panel option for each would be more lines than information.
# The FIRST element is the ID — it becomes `unicode-family-<id>`, and that is why it is
# English (invariant 18). The SECOND is the Portuguese search tag: whoever types
# `family:simbolo` still finds it, because a search tag is interface text
# and not a name. The third is the label.
FAMILIES = {
    "L": ("letter", {"en": ["letter"], "pt": ["letra", "letra"]}),
    "N": ("number", {"en": ["number"], "pt": ["numero", "número"]}),
    "S": ("symbol", {"en": ["symbol"], "pt": ["simbolo", "símbolo"]}),
    "P": ("punctuation", {"en": ["punctuation"], "pt": ["pontuacao", "pontuação"]}),
    "M": ("mark", {"en": ["mark"], "pt": ["marca", "marca"]}),
    "Z": ("other", {"en": ["other"], "pt": ["outro", "outro"]}),
    "C": ("other", {"en": ["other"], "pt": ["outro", "outro"]}),
}


def safe(s: str) -> str:
    """Strips from a value the punctuation that would break the component's parser.

    The buffer has no JSON parser (invariant 11): it finds the end of a value
    by looking for the next quote, splits records on `},` and list items
    on `,`. The CLDR keywords carry commas and quotes — `bandeira:
    Brasil` is harmless (the `:` separates nothing), but a comma inside a
    tag would become two tags and a quote would cut the value in half.

    It removes rather than escapes: the component's parser does not understand escaping, and one
    tag more in the search does no harm at all.
    """
    return "".join(" " if c in '"[]{},' else c for c in s).strip()


def die(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Version guard
# ---------------------------------------------------------------------------
def check_version(ucd: pathlib.Path, expected: str) -> None:
    """The UCD version has to be the one the flake declares.

    The header of every UCD file carries the version (`# Blocks-17.0.0.txt`).
    Without this guard, a nixpkgs update would swap the whole database in
    silence: new characters would come in, names would change, and the only clue
    would be the browser showing something different from what it showed yesterday.

    It is the same guard the glyph generator applies to Nerd Fonts, for the same
    reason and with the same severity.
    """
    header = (ucd / "Blocks.txt").read_text(encoding="utf-8").splitlines()[0]
    m = re.search(r"Blocks-(\d+\.\d+\.\d+)\.txt", header)
    if not m:
        die(f"the header of Blocks.txt does not declare a version: {header!r}")
    found_one = m.group(1)
    if found_one != expected:
        print("ERROR: the Unicode Character Database version diverged.", file=sys.stderr)
        print(f"       expected (flake):  {expected}", file=sys.stderr)
        print(f"       found (UCD):       {found_one}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       The symbol browser's database comes from here. Raising the version", file=sys.stderr)
        print("       changes which characters exist and what they are called —", file=sys.stderr)
        print("       update the constant in nix/unicodegen.nix on purpose,", file=sys.stderr)
        print("       and check the translation coverage afterwards.", file=sys.stderr)
        sys.exit(1)
    print(f"OK: Unicode {expected} — the UCD and the flake agree")


# ---------------------------------------------------------------------------
# Reading the UCD
# ---------------------------------------------------------------------------
def read_blocks(ucd: pathlib.Path) -> list[tuple[int, int, str]]:
    not_in = []
    for line in (ucd / "Blocks.txt").read_text(encoding="utf-8").splitlines():
        m = re.match(r"([0-9A-Fa-f]+)\.\.([0-9A-Fa-f]+);\s*(.+)", line)
        if m:
            not_in.append((int(m.group(1), 16), int(m.group(2), 16), m.group(3).strip()))
    return not_in


def read_aliases(ucd: pathlib.Path) -> dict[int, list[str]]:
    """The ALTERNATIVE names from `NameAliases.txt`.

    There are few (16 land in our database), but they are the answer to "I could have sworn
    that character was called something else" — the same role the aliases play in the
    glyph browser. `LINE FEED` finds `U+000A`, which is called
    `<control-000A>`.
    """
    not_in: dict[int, list[str]] = defaultdict(list)
    for line in (ucd / "NameAliases.txt").read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split(";")
        if len(fields) >= 2:
            not_in[int(fields[0], 16)].append(fields[1].strip())
    return not_in


def block_of(cp: int, block_list: list[tuple[int, int, str]]) -> str:
    for a, z, name in block_list:
        if a <= cp <= z:
            return name
    return ""


# ---------------------------------------------------------------------------
# Emoji
# ---------------------------------------------------------------------------
def ler_emoji(path: pathlib.Path) -> list[dict]:
    """The `fully-qualified` emoji from `emoji-test.txt`, with group and subgroup.

    Only the `fully-qualified` ones: the `minimally-qualified` and the `unqualified` are the
    SAME sequence without the presentation selector, and showing them would be showing the
    same emoji twice with identical drawings.
    """
    not_in, group, subgrupo = [], "", ""
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("# group:"):
            group = line.split(":", 1)[1].strip()
        elif line.startswith("# subgroup:"):
            subgrupo = line.split(":", 1)[1].strip()
        elif ";" in line and not line.startswith("#"):
            cps, resto = line.split(";", 1)
            if resto.split("#")[0].strip() != "fully-qualified":
                continue
            # The name comes after the `#` and after the emoji itself: `# 😀 E1.0 grinning face`
            after = resto.split("#", 1)[1].strip() if "#" in resto else ""
            parts = after.split(" ", 2)
            name = parts[2].strip() if len(parts) > 2 else ""
            not_in.append(
                {
                    "seq": [int(x, 16) for x in cps.split()],
                    "group": group,
                    "subgroup": subgrupo,
                    "name": name,
                }
            )
    return not_in


def ler_cldr(cldr: pathlib.Path, language: str) -> dict[str, dict]:
    """Emoji name and keywords, from the CLDR.

    # Why the CLDR and not the lexicon

    Because it is the OFFICIAL source, covers 100% of the emoji in both languages, and already
    carries the vocabulary people search with — `rosto risonho` has
    `engracado`, `feliz`, `lol`, `rindo`. Putting `grinning face` through the lexicon
    would give `rosto` and `sorridente` and nothing more.

    The two directories: `annotations` carries what was written by hand, and
    `annotationsDerived` carries what is derived from the sequences (skin tone, ZWJ) —
    without the second, 👩‍💻 would have no name.
    """
    not_in: dict[str, dict] = {}
    for sub in ("annotations", "annotationsDerived"):
        arq = cldr / sub / f"{language}.xml"
        if not arq.exists():
            continue
        for an in ET.parse(arq).getroot().iter("annotation"):
            cp = an.get("cp") or ""
            d = not_in.setdefault(cp, {"name": "", "keys": []})
            if an.get("type") == "tts":
                d["name"] = (an.text or "").strip()
            else:
                d["keys"] = [x.strip() for x in (an.text or "").split("|") if x.strip()]
    return not_in


def annotation(tab: dict[str, dict], raw_text: str) -> dict:
    """The CLDR keys it WITHOUT the presentation selector (`U+FE0F`)."""
    return tab.get(raw_text) or tab.get(raw_text.replace("\ufe0f", "")) or {}


def shape_to_one_glyph(font_file: pathlib.Path, emojis: list[dict]) -> list[str]:
    """Which sequences do NOT become a single glyph. Returns the list of defects.

    # Why this guard exists

    Because `cmap` does not answer the question. A ZWJ sequence has all its
    codepoints in the cmap and still comes out DECOMPOSED if the font lacks the
    ligature — 👩💻 instead of 👩‍💻. Measured: the monochrome Noto shapes 66% of the
    3,944 and decomposes 1,258; the colour one shapes 100%.

    Without really shaping, the database would promise 3,944 and the screen would deliver pieces.
    """
    font = hb.Font(hb.Face(font_file.read_bytes()))
    bads = []
    for e in emojis:
        raw_text = "".join(chr(c) for c in e["seq"])
        buf = hb.Buffer()
        buf.add_str(raw_text)
        buf.guess_segment_properties()
        hb.shape(font, buf)
        ids = [g.codepoint for g in buf.glyph_infos]
        if len(ids) != 1 or ids[0] == 0:
            bads.append(f"U+{' U+'.join(f'{c:04X}' for c in e['seq'])}  {e['name']}")
    return bads


# The nine groups of `emoji-test.txt` -> (id, en name, pt name).
#
# THESE NAMES ARE THE SAME ONES THE `.ftl` SHOWS in the panel, and here they become
# search TAGS. They are two representations of the same fact, and what keeps the two from
# diverging is the `symbols-filters` check: the label one reads has to find,
# when typed into the search, the whole group.
# The FIRST element is the ID — it builds `unicode-group-<id>` and that is why it is
# English (invariant 18). The other two are SEARCH: English tags and the Portuguese
# label, and both still find the whole group.
EMOJI_GROUPS = {
    "Smileys & Emotion": ("faces", {"en": "smileys emotion carinhas", "pt": "Carinhas e emoções"}),
    "People & Body": ("people", {"en": "people body pessoas", "pt": "Pessoas e corpo"}),
    "Animals & Nature": ("nature", {"en": "animals nature natureza", "pt": "Animais e natureza"}),
    "Food & Drink": ("food", {"en": "food drink comida", "pt": "Comida e bebida"}),
    "Travel & Places": ("travel", {"en": "travel places viagem", "pt": "Viagem e lugares"}),
    "Activities": ("activities", {"en": "activities atividades", "pt": "Atividades"}),
    "Objects": ("objects", {"en": "objects objetos", "pt": "Objetos"}),
    "Symbols": ("symbols", {"en": "symbols simbolos", "pt": "Símbolos"}),
    "Flags": ("flags", {"en": "flags bandeiras", "pt": "Bandeiras"}),
}


# The five skin tone modifiers (Fitzpatrick) -> (id, words per subtag).
#
# The tone does NOT come from the name: in English it arrives as "light skin tone" at the
# end, but in Portuguese the CLDR writes "pele clara", and there are emoji whose name mentions
# skin without having a modifier. What answers is the SEQUENCE — the modifier's codepoint is
# there, or it is not.
#
# Like the families and the groups, these ids are the same ones the `.ftl` shows: the
# label one reads has to find, when typed into the search, the whole tone.
# The FIRST element is the ID (it builds `unicode-tone-<id>`, hence English); the second
# is the Portuguese search form; the third, the label.
TONES = {
    0x1F3FB: ("light", {"en": "light skin tone", "pt": "Clara claro pele tone"}),
    0x1F3FC: ("medium-light", {"en": "medium-light skin tone", "pt": "Clara média medio-claro pele tone"}),
    0x1F3FD: ("medium", {"en": "medium skin tone", "pt": "Média medio pele tone"}),
    0x1F3FE: ("medium-dark", {"en": "medium-dark skin tone", "pt": "Escura média medio-escuro pele tone"}),
    0x1F3FF: ("dark", {"en": "dark skin tone", "pt": "Escura escuro pele tone"}),
}


def tone_of(seq: list[int]) -> str:
    """A sequence's skin tone, or empty.

    The FIRST modifier rules. There are sequences with two — the handshake between
    two people of different tones — and for those the filter returns the first:
    whoever searches for a tone wants to see the emoji that uses it, and it does.
    """
    for cp in seq:
        if cp in TONES:
            return TONES[cp][0]
    return ""


def emoji_lines(emojis: list[dict], cldr: dict[str, dict], subtags: list[str]) -> list[dict]:
    """One record per emoji, with the tags coming from the CLDR.

    `seq` is what gets COPIED and `shows` is what gets DRAWN — here the two are
    the whole sequence, but they part company on combining marks (where the
    drawing carries the dotted circle and the copy does not), and that is why they are
    different fields.
    """
    not_in = []
    for e in emojis:
        raw_text = "".join(chr(c) for c in e["seq"])
        said = {l: annotation(cldr[l], raw_text) for l in subtags}
        gid, g_words = EMOJI_GROUPS.get(e["group"], ("other", {"en": "other outro"}))

        name = safe(said[SOURCE].get("name") or e["name"])
        # The tags: the name and the CLDR keywords in EVERY language, plus the
        # group's name. The phrases go in WHOLE — the buffer matches a word prefix
        # inside the tag, so `risonho` finds `rosto risonho`.
        # The FAMILY also goes in as a tag, as with the others: the `Emoji` label
        # the panel shows has to find, when typed into the search, the whole
        # family — and it is the `symbols-filters` check that ties the two together.
        tone = tone_of(e["seq"])
        t_words = next((v[1] for v in TONES.values() if v[0] == tone), {}) if tone else {}
        tags = {}
        for language in subtags:
            # The group's and the tone's words fall back to the SOURCE, because
            # most of them are not translated at all — a language with no entry
            # gets the English rather than nothing.
            one = {
                g_words.get(language, g_words.get(SOURCE, "")).lower(),
                "emoji",
            } | {safe(k).lower() for k in said[language].get("keys", [])}
            if said[language].get("name"):
                one.add(safe(said[language]["name"]).lower())
            # The TONE goes in as a TAG, for the same reason as the family and
            # the group: the label the panel shows has to find, when typed into
            # the search, the whole tone. The CLDR already writes "pele clara"
            # in the name, but not in every language nor in every sequence —
            # relying on that would be relying on luck.
            if tone:
                one.add(t_words.get(language, t_words.get(SOURCE, tone)).lower())
            one |= {w for k in one for w in words_of(k)}
            tags[language] = one
        tags[SOURCE].add(name.lower())
        tags[SOURCE] |= set(words_of(name))

        not_in.append(
            {
                "cp": e["seq"][0],
                "seq": e["seq"],
                "shows": e["seq"],
                "name": name,
                "aliases": [],
                "cat": "",
                "family": "emoji",
                "group": gid,
                "subgroup": safe(e["subgroup"]),
                # The TONE, only when there is one. Present in every record it would mean writing
                # "nenhum" 2,048 times in a database that is already 2.4 MB.
                "tone": tone,
                "block": "",
                "decomp": "",
                "width": 2,
                # ONE FIELD PER LANGUAGE, and not a nested object: the buffer
                # has no JSON parser (invariant 11).
                **{f"tags_{l}": sorted(x for x in w if x) for l, w in tags.items()},
                "search": sorted(search_forms(tags)),
            }
        )
    return not_in


# ---------------------------------------------------------------------------
# The index
# ---------------------------------------------------------------------------
def build_index(
    ucd: pathlib.Path,
    cmap: dict,
    advance: dict,
    lexicon: dict,
    cmap_emoji: set,
    ja_e_emoji: set,
    subtags: list[str],
) -> tuple[list, dict]:
    block_list = read_blocks(ucd)
    aliases = read_aliases(ucd)

    no_translation: Counter = Counter()
    # Characters that did not get ONE word in Portuguese. They are the lexicon's work
    # queue, and the reason the build stops (see `main`).
    without_pt: list[str] = []
    lines = []

    for line in (ucd / "UnicodeData.txt").read_text(encoding="utf-8").splitlines():
        fields = line.split(";")
        if len(fields) < 6:
            continue
        name = fields[1]
        # A NAME BETWEEN `<>` IS NOT A NAME: it is a marker of the format (`<control>`,
        # `<Private Use, First>`). A character with no proper name has nothing to
        # be searched for, and the ranges marked by `First`/`Last` are not even
        # a single character.
        if not name or name.startswith("<"):
            continue
        cp = int(fields[0], 16)
        # THE DATABASE IS THE UNION OF THE TWO FACES. While the renderer had a single
        # face, `cmap` was enough; with the emoji face it draws more than the
        # Iosevka, and keeping the old intersection would make the database lie by
        # OMISSION — it would hide characters that show up perfectly.
        if cp not in cmap and cp not in cmap_emoji:
            continue
        # WHATEVER IS ALREADY AN EMOJI DOES NOT GO IN TWICE. `U+1F600` has a name in the UCD and
        # is in the emoji face's cmap, so it would land here TOO — and the
        # emoji record is the better of the two: it has group, subgroup and the
        # CLDR keywords in both languages.
        if cp in ja_e_emoji:
            continue

        category = fields[2]
        family, fam_words = FAMILIES.get(category[0], ("other", {"en": ["other"]}))
        block = block_of(cp, block_list)
        other_names = aliases.get(cp, [])

        # THE WIDTH COMES FROM `hmtx`, and not from the category.
        #
        # There are 312 codepoints with zero advance and only 283 with category `Mn`: what
        # decides whether the character takes up room in the grid is the real advance, and
        # inferring it from the category would get 29 of them wrong.
        if cp in cmap:
            av = advance[cmap[cp]]
            width = 0 if av == 0 else (2 if av > 900 else 1)
        else:
            # It came from the emoji face: advance 2550/2048 = 1.245 em against the
            # cell's 0.600, that is, two columns with 3.7% overflow —
            # which the grid's three-column gap absorbs.
            width = 2

        # The words that find this character: those of the NAME and those of the aliases.
        #
        # The BLOCK's are left out, and the measurement is the reason: they cost
        # 700 KB in the database and are nearly all redundant — the 256 characters of the
        # `Arrows` block are already called `… ARROW …`, so "seta" finds them by
        # name. Whoever wants the whole block has the `block:` filter, which matches the
        # real field instead of hoping the word happened to land in the tag.
        tokens = words_of(name)
        for other in other_names:
            tokens += words_of(other)
        tags, found = translate(tokens, lexicon, subtags, no_translation)
        # BOTH FORMS OF THE FAMILY, in every language: the accented LABEL, which
        # is what the panel shows and what the `symbols-filters` check types,
        # and the unaccented form, which is what one writes in a
        # `family:simbolo` filter. Sending only one breaks the other — the first
        # version of this sent only the label, and inverting the tuple swapped
        # it for the unaccented form with nothing warning beyond the check.
        #
        # A LIST, and added WHOLE — never through `words_of`, which splits on
        # `[^A-Za-z0-9]` and would cut `símbolo` into `s` and `mbolo`. That is
        # exactly what a first pass at this did, and `symbols-filters` caught
        # it: the panel showed "Símbolo" and the search found 230 of 3,658.
        for language in subtags:
            tags[language].update(fam_words.get(language, fam_words[SOURCE]))

        # WHAT THE GRID DRAWS, in CODEPOINTS and not in text.
        #
        # NO STRING FIELD CARRIES AN ARBITRARY CHARACTER, and this is not
        # taste: the buffer has no JSON parser (invariant 11) and finds the end
        # of a value by looking for the next quote. For `U+0022`, `json.dumps`
        # writes `"\""`, and the search for the quote stops at the escape — the buffer would read
        # `\` instead of `"`. For `U+005C`, `\\`. Two wrong characters, in
        # silence, and only on those two.
        #
        # With a list of numbers no escaping is possible. `cp` is already the
        # truth; this list exists only for the combining marks, which
        # need the dotted circle in front.
        shows = [ord(DOTTED_CIRCLE), cp] if width == 0 else [cp]

        lines.append(
            {
                "cp": cp,
                "shows": shows,
                "name": name,
                "aliases": other_names,
                "cat": category,
                "family": family,
                "block": block,
                # The canonical decomposition, in hexadecimal. `<compat>` and other
                # markers are left out: they say HOW to decompose, and what the
                # sheet answers is "what parts is this made of".
                "decomp": "" if fields[5].startswith("<") else fields[5],
                **{f"tags_{l}": sorted(w) for l, w in tags.items()},
                "search": sorted(search_forms(tags)),
            }
        )
        # OMITTED WHEN IT IS 1, which is the case for 96% of them: always writing it
        # would cost 86 KB to say "the normal one". Absent, the buffer reads 1.
        if width != 1:
            lines[-1]["width"] = width

        # The SOURCE language is never "untranslated": the token IS the English
        # word. What is reported is every other language.
        if not all(found[l] for l in subtags if l != SOURCE):
            without_pt.append(f"U+{cp:04X} {name}")

    estatisticas = {
        "total": len(lines),
        "untranslated": no_translation,
        "no_pt": without_pt,
        "blocks": len({r["block"] for r in lines if r["block"]}),
        "families": Counter(r["family"] for r in lines),
        "combining": sum(1 for r in lines if r.get("width", 1) == 0),
        "wide": sum(1 for r in lines if r.get("width", 1) == 2),
    }
    return lines, estatisticas


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--ucd", type=pathlib.Path, required=True, help="the UCD's share/unicode")
    ap.add_argument("--font", type=pathlib.Path, required=True, help="the UNPATCHED font")
    ap.add_argument("--lexicon", type=pathlib.Path, required=True)
    ap.add_argument("--emoji-test", type=pathlib.Path, required=True)
    ap.add_argument("--cldr", type=pathlib.Path, required=True, help="the CLDR's common/")
    # The languages, from `assets/i18n/languages.nix`. The tags, the CLDR
    # bundles and the lexicon are all keyed on the subtag.
    ap.add_argument("--languages", required=True, nargs="+")
    ap.add_argument("--font-emoji", type=pathlib.Path, required=True)
    ap.add_argument("--ucd-version", required=True)
    # THE TWO FLOORS COME FROM OUTSIDE so the negative fixture can exercise the
    # guards without editing the real lexicon — see `nix/unicodegen.nix`.
    ap.add_argument("--min-coverage", type=float, default=94.0)
        # ZERO, as in the glyph generator. An exception for "the 69 left over"
    # would be an exception forever: the next version of the UCD brings characters
    # that fall into it with nobody seeing. It is 81 words to empty the list.
    ap.add_argument("--cap-no-pt", type=int, default=0)
    ap.add_argument("--out", type=pathlib.Path, required=True)
    args = ap.parse_args()

    check_version(args.ucd, args.ucd_version)

    font_file = TTFont(args.font, lazy=True)
    cmap = font_file.getBestCmap()
    hmtx = font_file["hmtx"]
    advance = {name: hmtx[name][0] for name in set(cmap.values())}

    # THE EMOJI FACE'S CMAP. The database is the UNION of the two: the renderer draws
    # both, and hiding what the second covers would be lying by omission.
    cmap_emoji = set(TTFont(args.font_emoji, lazy=True).getBestCmap())

    # --- THE EMOJI -------------------------------------------------------
    emojis = ler_emoji(args.emoji_test)
    ja_e_emoji = {e["seq"][0] for e in emojis if len(e["seq"]) == 1}

    lexicon = tomllib.loads(args.lexicon.read_text(encoding="utf-8"))
    subtags = [subtag_of(l) for l in args.languages]
    lines, st = build_index(
        args.ucd, cmap, advance, lexicon, cmap_emoji, ja_e_emoji, subtags
    )
    if not emojis:
        die("emoji-test.txt yielded no fully-qualified emoji at all")

    # THE SHAPING GUARD, before anything else: a sequence that does not become
    # ONE glyph appears decomposed on screen, and `cmap` does not answer that.
    bads = shape_to_one_glyph(args.font_emoji, emojis)
    if bads:
        print("", file=sys.stderr)
        print(f"ERROR: {len(bads)} emoji do not shape into ONE glyph in this font.", file=sys.stderr)
        print("", file=sys.stderr)
        for r in bads[:12]:
            print(f"       {r}", file=sys.stderr)
        if len(bads) > 12:
            print(f"       … and {len(bads) - 12} more", file=sys.stderr)
        print("", file=sys.stderr)
        print("       They would show up DECOMPOSED — 👩💻 instead of 👩‍💻 — because the", file=sys.stderr)
        print("       font does not have the ligature. `cmap` does not answer that: the", file=sys.stderr)
        print("       codepoints are all there.", file=sys.stderr)
        sys.exit(1)

    # ONE CLDR BUNDLE PER LANGUAGE, keyed by subtag — the CLDR ships them that
    # way, one `annotations/<subtag>.xml` per language, so this generalises by
    # asking rather than by growing a second variable.
    cldr = {l: ler_cldr(args.cldr, l) for l in subtags}
    for language in subtags:
        no_cldr = [
            "".join(chr(c) for c in e["seq"])
            for e in emojis
            if not annotation(cldr[language], "".join(chr(c) for c in e["seq"])).get("keys")
        ]
        if no_cldr:
            die(
                f"{len(no_cldr)} emoji with no {language} keyword in the CLDR "
                f"(e.g. {no_cldr[:5]}). Anyone searching in {language} does not find them."
            )
    lines.extend(emoji_lines(emojis, cldr, subtags))
    st["emoji"] = len(emojis)
    st["total"] = len(lines)

    if not lines:
        die("the database came out EMPTY — the font and UnicodeData.txt did not meet")

    args.out.mkdir(parents=True, exist_ok=True)
    # COMPACT `separators`, and not the default. The default puts `", "` between the
    # objects, and the buffer — which has no JSON parser (invariant 11) —
    # splits the database on punctuation. With the space, it finds ONE record of 2 MB
    # instead of thousands. That is what happened with the glyph database.
    (args.out / "unicode.json").write_text(
        json.dumps(lines, ensure_ascii=False, separators=(",", ":")), encoding="utf-8"
    )

    # --- coverage.md ------------------------------------------------------
    hits_without = sum(st["untranslated"].values())
    translated = sum(
        1 for r in lines for t in r.get(f"tags_{SOURCE}", []) if t in lexicon
    )
    total = translated + hits_without
    coverage = 100.0 * translated / total if total else 0.0

    raw_text = [
        "# Unicode symbol database coverage",
        "",
        "Generated by `nix/scripts/gen-unicodedb.py`. Do not edit by hand.",
        "",
        f"- Unicode: **{args.ucd_version}**",
        f"- Characters in the database: **{st['total']}**",
        f"- Emoji: **{st.get('emoji', 0)}**",
        f"- Blocks represented: **{st['blocks']}**",
        f"- Combining marks (zero advance): **{st['combining']}**",
        f"- Double width: **{st['wide']}**",
        f"- With NO Portuguese word at all: **{len(st['no_pt'])}**",
        "",
        "## By family",
        "",
        "| family | characters |",
        "|---|---:|",
    ]
    for fam, n in st["families"].most_common():
        raw_text.append(f"| `{fam}` | {n} |")
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
        "os dois navegadores.",
        "",
        "| word | occurrences |",
        "|---|---:|",
    ]
    for word, n in st["untranslated"].most_common(80):
        raw_text.append(f"| `{word}` | {n} |")
    if len(st["untranslated"]) > 80:
        raw_text.append(f"| … | {len(st['untranslated']) - 80} more words |")
    (args.out / "coverage.md").write_text("\n".join(raw_text) + "\n", encoding="utf-8")

    print(
        f"OK: {st['total']} characters indexed "
        f"({st.get('emoji', 0)} emoji, {st['blocks']} blocks, "
        f"{st['combining']} combining, {st['wide']} wide)"
    )

    # --- THE TWO GUARDS ----------------------------------------------------
    #
    # They live in the GENERATOR, and not in a check that reads the JSON afterwards, for the same
    # reason as the glyph generator: what knows which character was left untranslated
    # is what built the table. The output does NOT say — an untranslated word goes into
    # the tag list untranslated.
    if len(st["no_pt"]) > args.cap_no_pt:
        print("", file=sys.stderr)
        print(
            f"ERROR: {len(st['no_pt'])} characters with NO Portuguese tag at all, "
            f"and the ceiling is {args.cap_no_pt}.",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        for g in st["no_pt"][:12]:
            print(f"       {g}", file=sys.stderr)
        if len(st["no_pt"]) > 12:
            print(f"       … and {len(st['no_pt']) - 12} more", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Anyone searching in Portuguese does NOT find these characters.", file=sys.stderr)
        print("       Translate their words in assets/lexicon/tags.en-pt.toml", file=sys.stderr)
        print("       — the queue, by frequency, is in the coverage.md this", file=sys.stderr)
        print("       same build writes.", file=sys.stderr)
        sys.exit(1)

    if coverage < args.min_coverage:
        print("", file=sys.stderr)
        print(
            f"ERROR: translation coverage at {coverage:.1f}%, "
            f"below the floor of {args.min_coverage}%.",
            file=sys.stderr,
        )
        print("       The work queue, by frequency, is in coverage.md.", file=sys.stderr)
        sys.exit(1)

    print(
        f"OK: tag translation at {coverage:.1f}% "
        f"({len(st['untranslated'])} distinct words with no translation, "
        f"{len(st['no_pt'])} characters with no Portuguese)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
