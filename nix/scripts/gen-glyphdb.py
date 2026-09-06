#!/usr/bin/env python3
"""Single generator of the glyph layer — sections 6.3 and 6.5.

"Everything else is generated at build time, by the same generator that produces the
icon token table (6.3) — a single source of truth." (section 6.5)

THE TWO AUTHORITIES
-------------------
    glyphnames.json  -> authority on the NAMES  (fa-folder, md-cog)
    the .ttf file    -> authority on what EXISTS (the real codepoints)

They HAVE to come from the same version of Nerd Fonts, otherwise the PUA codepoints
shift silently on an update. This is the trap section 6.3 names,
and there are three guards against it:

    1. the version in the font's nameID 5 has to match the expected one;
    2. the version in glyphnames.json's METADATA too;
    3. every referenced icon token has to resolve to a codepoint that
       EXISTS in the cmap — otherwise the build fails, saying which token and which glyph.

OUTPUTS
-------
    icons.json      token -> {glyph, codepoint, char}   (section 6.3's table)
    glyphs.json     the glyph browser's index           (section 6.5's database)
    coverage.md     report: tag coverage, untranslated tokens
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib
from collections import Counter, defaultdict

from fontTools.pens.boundsPen import BoundsPen
from fontTools.ttLib import TTFont

# THE LEXICON'S CONTRACT IS SHARED with the Unicode database generator: the
# format of `tags.en-pt.toml` is a single fact, and two readers of it would diverge.
from lexicon import SOURCE, search_forms, subtag_of, translate, words_of

# Nerd Fonts source sets: prefix -> (readable name en, pt).
# It feeds the set tag and section 6.5's "by source set" filter.
#
# THESE NAMES ARE THE SAME ONES THE FILTER PANEL SHOWS. There they come from
# the `.ftl` (invariant 8: what speaks on screen is the interface, and translation
# belongs to the host); here they become search TAGS. They are two representations
# of the same fact, and what keeps the two from diverging is `browser-filters`: the
# label the panel shows has to find, when typed into the search, the whole set.
#
# That is why the Portuguese comes ACCENTED: each glyph's `search_terms` gets the
# unaccented form for free (see `no_accent`), so accenting here makes "indentacao"
# AND "indentação" both find it — while writing it unaccented would give only the first.
#
# Keyed by SUBTAG, and a language with no entry falls back to the English —
# which is right, because most of these are brand names that are not translated
# at all. That is also why a name identical in both is written once.
SETS = {
    "cod": {"en": "Codicons"},
    "custom": {"en": "Custom", "pt": "Personalizados"},
    "dev": {"en": "Devicons"},
    "extra": {"en": "Extra"},
    "fa": {"en": "Font Awesome"},
    "fae": {"en": "Font Awesome Extension"},
    "iec": {"en": "IEC Power", "pt": "Símbolos de energia IEC"},
    "indent": {"en": "Indentation", "pt": "Indentação"},
    "indentation": {"en": "Indentation", "pt": "Indentação"},
    "linux": {"en": "Font Logos", "pt": "Logos de distribuições"},
    "md": {"en": "Material Design"},
    "oct": {"en": "Octicons"},
    "pl": {"en": "Powerline"},
    "ple": {"en": "Powerline Extra"},
    "pom": {"en": "Pomicons"},
    "seti": {"en": "Seti-UI"},
    "weather": {"en": "Weather", "pt": "Clima"},
}

def die(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Version guards
# ---------------------------------------------------------------------------
def check_versions(font: TTFont, names_meta: dict, expected: str) -> None:
    raw = font["name"].getDebugName(5) or ""
    m = re.search(r"Nerd Fonts (\d+\.\d+\.\d+)", raw)
    if not m:
        die(f"the font's nameID 5 does not declare a Nerd Fonts version: {raw!r}")
    font_v = m.group(1)
    json_v = names_meta.get("version", "")

    if font_v != expected or json_v != expected:
        print("ERROR: the Nerd Fonts versions diverge.", file=sys.stderr)
        print(f"       expected (flake):     {expected}", file=sys.stderr)
        print(f"       font (nameID 5):      {font_v}", file=sys.stderr)
        print(f"       glyphnames.json:      {json_v}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       The PUA codepoints change between major versions. Update", file=sys.stderr)
        print("       the font and glyphnames.json TOGETHER, or the icon table", file=sys.stderr)
        print("       points at the wrong glyphs with nobody noticing.", file=sys.stderr)
        sys.exit(1)

    print(f"OK: Nerd Fonts {expected} — the font, glyphnames.json and the flake agree")


# ---------------------------------------------------------------------------
# Icon token table (section 6.3)
# ---------------------------------------------------------------------------
def chrome_size(ink_ratio: float | None, base: float, icon: float, tolerance: float) -> float:
    """THE SIZE A CHROME GLYPH IS DRAWN AT beside text of the base size.

    `font.size.icon` was found by measurement: the bell's ink is 600 units
    against the digit's 751, and 22 px is what brings the two to the same
    height. It is the size of every chrome glyph whose ink, drawn there, stays
    within the band the check allows (`chromeTolerance`, both ways) — which
    keeps every drawing that already passed exactly where it was. A vertical
    battery is 908 units tall, taller than the digit at the text's own size,
    and at 22 px it stood 48% above the number beside it: past the band, so
    it comes DOWN to the size that puts its ink at the digit's — 15 px. Whole
    pixels, so the run's metrics stay what the renderer can hint. A glyph
    with no ink (nothing to measure) takes the icon size.
    """
    if not ink_ratio:
        return icon
    if ink_ratio * icon / base <= 1.0 + tolerance / 100.0:
        return icon
    return float(min(icon, int(base / ink_ratio + 0.5)))


def build_icon_table(
    tokens: dict,
    names: dict,
    font: TTFont,
    tray: dict,
    base: float,
    icon: float,
    tolerance: float,
) -> dict:
    out = {}
    missing = []
    cmap = font.getBestCmap()
    glyphs = font.getGlyphSet()

    def ink(cp: int) -> float | None:
        pen = BoundsPen(glyphs)
        glyphs[cmap[cp]].draw(pen)
        return None if pen.bounds is None else pen.bounds[3] - pen.bounds[1]

    digit = ink(ord("0"))

    for token, glyph in sorted(tokens.items()):
        entry = names.get(glyph)
        if entry is None:
            missing.append((token, glyph, "does not exist in glyphnames.json"))
            continue
        cp = int(entry["code"], 16)
        if cp not in cmap:
            missing.append((token, glyph, f"U+{cp:04X} is missing from the font's cmap"))
            continue
        height = ink(cp)
        ratio = None if height is None or not digit else round(height / digit, 4)
        out[token] = {
            "glyph": glyph,
            "codepoint": cp,
            "char": chr(cp),
            # The ink's height against the digit's, and the size that follows
            # from it — the ONE measurement `chrome-icons-match-the-text`
            # re-checks and `gen-tokens.py` hands to the shell.
            "ink": ratio,
            "chrome": chrome_size(ratio, base, icon, tolerance),
        }

    # The tray table may only point at tokens that exist.
    for app_id, token in sorted(tray.items()):
        if token not in tokens:
            missing.append((f"tray[{app_id}]", token, "an icon token that does not exist"))

    if missing:
        print("ERROR: icon tokens that do not resolve:", file=sys.stderr)
        for token, glyph, reason in missing:
            print(f"       icon.{token}  ->  {glyph}", file=sys.stderr)
            print(f"           {reason}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Fix it in assets/tokens/icons.nix. The glyph browser", file=sys.stderr)
        print("       (milestone 7) exists to find the right name without guessing.", file=sys.stderr)
        sys.exit(1)

    print(f"OK: {len(out)} icon tokens resolved against the real cmap")
    return out


# ---------------------------------------------------------------------------
# Tags (section 6.5)
# ---------------------------------------------------------------------------
def tokenize(glyph: str) -> tuple[str, list[str]]:
    """`md-arrow_up_bold` -> ('md', ['arrow', 'up', 'bold'])

    The prefix is Nerd Fonts's (it is the source set); the WORDS come from the
    shared module, so the two generators split a name the same way.
    """
    prefix, _, rest = glyph.partition("-")
    return prefix, words_of(rest)


def build_glyph_index(
    names: dict,
    cmap: set,
    base_cmap: set,
    lexicon: dict,
    overlay: dict,
    token_by_glyph: dict,
    tray_glyphs: set,
    subtags: list[str],
) -> tuple[list, dict]:
    """The browser's index. It covers ALL of the font's Nerd Fonts codepoints,
    including the ones with no name — removing them would be lying about what the
    font has."""
    nerd_cps = cmap - base_cmap

    # name(s) per codepoint; a codepoint may have several names (aliases).
    by_cp = defaultdict(list)
    for glyph, entry in names.items():
        cp = int(entry["code"], 16)
        if cp in nerd_cps:
            by_cp[cp].append(glyph)

    untranslated = Counter()
    # Glyphs that did not get ONE word in Portuguese. They are the lexicon's work
    # queue, and the reason the build stops (see `main`).
    without_pt: list[str] = []
    rows = []

    for cp in sorted(nerd_cps):
        glyphs = sorted(by_cp.get(cp, []))
        primary = glyphs[0] if glyphs else None

        tags = {l: set() for l in subtags}
        sets_seen = set()
        # DOES THIS GLYPH HAVE A REAL WORD IN EACH LANGUAGE?
        #
        # There is no answering that by looking at the tag list: an untranslated
        # token goes in untranslated, so the list NEVER comes out empty — it
        # just fills up with English pretending to be the other language. The
        # only honest way is to mark when a translation actually came from the
        # lexicon or from the overlay.
        found = {l: False for l in subtags}

        for g in glyphs:
            prefix, parts = tokenize(g)
            named = SETS.get(prefix, {})
            sets_seen.add(prefix)
            for language in subtags:
                tags[language].add(named.get(language, named.get(SOURCE, prefix)).lower())

            said, matched = translate(parts, lexicon, subtags, untranslated)
            for language in subtags:
                tags[language].update(said[language])
                found[language] = found[language] or matched[language]

            # Curated overlay: it ADDS, it does not replace.
            ov = overlay.get(g)
            if ov:
                for language in subtags:
                    words = ov.get(language, [])
                    tags[language].update(words)
                    found[language] = found[language] or bool(words)

        # THE ACCENT IS RESOLVED HERE, and not at runtime.
        #
        # `localização` and `localizacao` have to find the same glyph. Normalising
        # in the component would cost walking 10,473 glyphs times ~20 tags on every
        # keystroke; here it costs once, at build time. What goes to the SCREEN
        # stays the accented form, which is how one writes.
        #
        # Only those that DIFFER go in: `mapa` does not get a twin identical to itself.
        search_terms = search_forms(tags)

        rows.append(
            {
                "cp": cp,
                "hex": f"{cp:04X}",
                "char": chr(cp),
                "names": glyphs,
                "name": primary,
                "sets": sorted(sets_seen),
                # ONE FIELD PER LANGUAGE, and not a nested object: the buffer
                # has no JSON parser (invariant 11) — it finds `"name":[` and
                # reads to the next `]`. A nested map would be read as one
                # list of garbage, in silence.
                **{f"tags_{l}": sorted(w) for l, w in tags.items()},
                "search": sorted(search_terms),
                # Section 6.5's filters: "is in use as a token" and "is assigned
                # to some app id".
                "token": token_by_glyph.get(primary),
                "in_tray": primary in tray_glyphs if primary else False,
            }
        )

        # The UNNAMED ones do not count: with no name there is nothing to tokenise, and
        # demanding a translation for a bare codepoint would be demanding the impossible. They
        # stay in the index — removing them would be lying about what the font has.
        # The SOURCE language is never "missing a translation": the token IS
        # the English word. What is reported is every other language.
        if primary and not all(found[l] for l in subtags if l != SOURCE):
            without_pt.append(primary)

    stats = {
        "nerd_codepoints": len(nerd_cps),
        "named": sum(1 for r in rows if r["name"]),
        "unnamed": sum(1 for r in rows if not r["name"]),
        "untranslated": untranslated,
        "no_pt": without_pt,
    }
    return rows, stats


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--font", type=pathlib.Path, required=True)
    ap.add_argument("--font-unpatched", type=pathlib.Path, required=True)
    ap.add_argument("--glyphnames", type=pathlib.Path, required=True)
    ap.add_argument("--tokens", type=pathlib.Path, required=True, help="icons.json")
    ap.add_argument("--tray", type=pathlib.Path, required=True, help="tray.json")
    ap.add_argument("--typography", type=pathlib.Path, required=True, help="typography.json")
    ap.add_argument("--lexicon", type=pathlib.Path, required=True)
    # THE FLOOR COMES FROM OUTSIDE so the negative fixture can exercise the guard without
    # editing the real lexicon — see `nix/glyphgen.nix`.
    ap.add_argument("--min-coverage", type=float, default=90.0)
    # The languages, from `assets/i18n/languages.nix`. The tags are built for
    # every one of them; the lexicon is keyed on the subtag.
    ap.add_argument("--languages", required=True, nargs="+")
    ap.add_argument("--overlay", type=pathlib.Path, required=True)
    ap.add_argument("--nf-version", required=True)
    ap.add_argument("--out", type=pathlib.Path, required=True)
    args = ap.parse_args()

    font = TTFont(args.font, lazy=True)
    cmap = set(font.getBestCmap())
    base_cmap = set(TTFont(args.font_unpatched, lazy=True).getBestCmap())

    raw_names = json.loads(args.glyphnames.read_text())
    names_meta = raw_names.pop("METADATA", {})

    check_versions(font, names_meta, args.nf_version)

    tokens = json.loads(args.tokens.read_text())
    tray = json.loads(args.tray.read_text())
    typography = json.loads(args.typography.read_text())
    sizes = typography["size"]
    lexicon = tomllib.loads(args.lexicon.read_text())
    overlay = tomllib.loads(args.overlay.read_text())

    # The overlay may only speak of glyphs that exist.
    orphans = [g for g in overlay if g not in raw_names]
    if orphans:
        die(
            f"overlay.toml cites {len(orphans)} glyphs that do not exist: {orphans[:5]}\n"
            "       Use the name without the `nf-` prefix (e.g. `fa-folder`, not `nf-fa-folder`)."
        )

    icons = build_icon_table(
        tokens,
        raw_names,
        font,
        tray,
        float(sizes["base"]),
        float(sizes["icon"]),
        float(typography["chromeTolerance"]),
    )
    token_by_glyph = {v["glyph"]: k for k, v in icons.items()}
    tray_glyphs = {icons[t]["glyph"] for t in tray.values() if t in icons}

    subtags = [subtag_of(l) for l in args.languages]
    rows, stats = build_glyph_index(
        raw_names, cmap, base_cmap, lexicon, overlay, token_by_glyph, tray_glyphs, subtags
    )

    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "icons.json").write_text(json.dumps(icons, indent=2, ensure_ascii=False))
    # COMPACT `separators`, and not the default. The default puts `", "` between
    # objects, and the buffer — which has no JSON parser (invariant 11) —
    # splits the database on punctuation. With the space, it found ONE record of
    # 2.8 MB instead of 10,473. Compact also takes 10% off what gets embedded in the
    # component, and that counts against section 5.2's instantiation budget.
    (args.out / "glyphs.json").write_text(
        json.dumps(rows, ensure_ascii=False, separators=(",", ":"))
    )

    # --- coverage.md -------------------------------------------------------
    tot_occ = sum(stats["untranslated"].values())
    total_tokens_pt = sum(
        1 for r in rows for t in r.get(f"tags_{SOURCE}", []) if t in lexicon
    )
    coverage = (
        100.0 * total_tokens_pt / (total_tokens_pt + tot_occ) if (total_tokens_pt + tot_occ) else 0.0
    )

    lines = [
        "# Glyph layer coverage",
        "",
        "Generated by `nix/scripts/gen-glyphdb.py`. Do not edit by hand.",
        "",
        f"- Nerd Fonts: **{args.nf_version}**",
        f"- Nerd Fonts codepoints in the font: **{stats['nerd_codepoints']}**",
        f"- Named in glyphnames.json: **{stats['named']}**",
        f"- Unnamed (codepoint only): **{stats['unnamed']}**",
        f"- Icon tokens: **{len(icons)}**",
        f"- Entries in the pt-BR lexicon: **{len(lexicon)}**",
        f"- Entries in the curated overlay: **{len(overlay)}**",
        f"- Named glyphs with NO Portuguese tag at all: **{len(stats['no_pt'])}**",
        "",
        f"## Tag translation: {coverage:.1f}%",
        "",
        f"Required floor: **>= {args.min_coverage}%** of the token occurrences translated.",
        "",
        "## Untranslated tokens, by frequency",
        "",
        "This is the work queue for growing `assets/lexicon/tags.en-pt.toml`.",
        "Translate in this order, not in whatever order comes along.",
        "",
        "| token | occurrences |",
        "|---|---:|",
    ]
    for tok, n in stats["untranslated"].most_common(60):
        lines.append(f"| `{tok}` | {n} |")
    if len(stats["untranslated"]) > 60:
        lines.append(f"| … | {len(stats['untranslated']) - 60} more tokens |")

    (args.out / "coverage.md").write_text("\n".join(lines) + "\n")

    print(f"OK: {stats['nerd_codepoints']} glyphs indexed "
          f"({stats['named']} named, {stats['unnamed']} by codepoint alone)")
    # --- THE TWO COVERAGE GUARDS -------------------------------------------
    #
    # They live in the GENERATOR, and not in a check that reads the JSON afterwards, for
    # the same reason as the icon token guard just above: what knows which glyph
    # was left untranslated is what built the table. A later check would have to
    # rediscover that from the output — and the output does NOT say, because an
    # untranslated token goes into the tag list untranslated.
    #
    # Noisy on purpose: bumping the Nerd Fonts version starts failing until
    # the new glyphs are translated. It is the same design as the check that demands
    # translation of niri's actions.
    if stats["no_pt"]:
        sample = stats["no_pt"][:12]
        print("", file=sys.stderr)
        print(f"ERROR: {len(stats['no_pt'])} glyphs with NO Portuguese tag at all.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in sample:
            print(f"       {g}", file=sys.stderr)
        if len(stats["no_pt"]) > len(sample):
            print(f"       … and {len(stats['no_pt']) - len(sample)} more", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Anyone searching in Portuguese does NOT find these glyphs. Translate", file=sys.stderr)
        print("       their tokens in assets/lexicon/tags.en-pt.toml — the queue, by", file=sys.stderr)
        print("       frequency, is in the coverage.md this very build generates.", file=sys.stderr)
        print("", file=sys.stderr)
        print("       A brand name is NOT translated: it gets CATEGORY tags", file=sys.stderr)
        print("       (`mysql` -> en: database / pt: banco de dados).", file=sys.stderr)
        sys.exit(1)

    if coverage < args.min_coverage:
        print("", file=sys.stderr)
        print(f"ERROR: translation coverage at {coverage:.1f}%, "
              f"below the floor of {args.min_coverage}%.", file=sys.stderr)
        print("       The work queue, by frequency, is in coverage.md.", file=sys.stderr)
        sys.exit(1)

    print(f"OK: tag translation at {coverage:.1f}% "
          f"({len(stats['untranslated'])} distinct tokens with no translation)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
