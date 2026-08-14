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

from fontTools.ttLib import TTFont

# THE LEXICON'S CONTRACT IS SHARED with the Unicode database generator: the
# format of `tags.en-pt.toml` is a single fact, and two readers of it would diverge.
from lexicon import search_forms, palavras, translate

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
SETS = {
    "cod": ("Codicons", "Codicons"),
    "custom": ("Custom", "Personalizados"),
    "dev": ("Devicons", "Devicons"),
    "extra": ("Extra", "Extra"),
    "fa": ("Font Awesome", "Font Awesome"),
    "fae": ("Font Awesome Extension", "Font Awesome Extension"),
    "iec": ("IEC Power", "Símbolos de energia IEC"),
    "indent": ("Indentation", "Indentação"),
    "indentation": ("Indentation", "Indentação"),
    "linux": ("Font Logos", "Logos de distribuições"),
    "md": ("Material Design", "Material Design"),
    "oct": ("Octicons", "Octicons"),
    "pl": ("Powerline", "Powerline"),
    "ple": ("Powerline Extra", "Powerline Extra"),
    "pom": ("Pomicons", "Pomicons"),
    "seti": ("Seti-UI", "Seti-UI"),
    "weather": ("Weather", "Clima"),
}

def die(msg: str) -> None:
    print(f"ERRO: {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Version guards
# ---------------------------------------------------------------------------
def check_versions(font: TTFont, names_meta: dict, expected: str) -> None:
    raw = font["name"].getDebugName(5) or ""
    m = re.search(r"Nerd Fonts (\d+\.\d+\.\d+)", raw)
    if not m:
        die(f"o nameID 5 da fonte nao declara versao do Nerd Fonts: {raw!r}")
    font_v = m.group(1)
    json_v = names_meta.get("version", "")

    if font_v != expected or json_v != expected:
        print("ERRO: versoes do Nerd Fonts divergem.", file=sys.stderr)
        print(f"       esperada (flake):     {expected}", file=sys.stderr)
        print(f"       fonte (nameID 5):     {font_v}", file=sys.stderr)
        print(f"       glyphnames.json:      {json_v}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Os codepoints da PUA mudam entre versoes maiores. Atualize", file=sys.stderr)
        print("       a fonte e o glyphnames.json JUNTOS, ou a tabela de icones", file=sys.stderr)
        print("       aponta para os glifos errados sem ninguem notar.", file=sys.stderr)
        sys.exit(1)

    print(f"OK: Nerd Fonts {expected} — fonte, glyphnames.json e flake concordam")


# ---------------------------------------------------------------------------
# Icon token table (section 6.3)
# ---------------------------------------------------------------------------
def build_icon_table(tokens: dict, names: dict, cmap: set, tray: dict) -> dict:
    out = {}
    missing = []

    for token, glyph in sorted(tokens.items()):
        entry = names.get(glyph)
        if entry is None:
            missing.append((token, glyph, "nao existe no glyphnames.json"))
            continue
        cp = int(entry["code"], 16)
        if cp not in cmap:
            missing.append((token, glyph, f"U+{cp:04X} ausente no cmap da fonte"))
            continue
        out[token] = {"glyph": glyph, "codepoint": cp, "char": chr(cp)}

    # The tray table may only point at tokens that exist.
    for app_id, token in sorted(tray.items()):
        if token not in tokens:
            missing.append((f"tray[{app_id}]", token, "token de icone inexistente"))

    if missing:
        print("ERRO: tokens de icone que nao resolvem:", file=sys.stderr)
        for token, glyph, motivo in missing:
            print(f"       icon.{token}  ->  {glyph}", file=sys.stderr)
            print(f"           {motivo}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Corrija em assets/tokens/icons.nix. O navegador de glifos", file=sys.stderr)
        print("       (marco 7) existe para achar o nome certo sem adivinhar.", file=sys.stderr)
        sys.exit(1)

    print(f"OK: {len(out)} tokens de icone resolvidos contra o cmap real")
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
    return prefix, palavras(rest)


def build_glyph_index(
    names: dict,
    cmap: set,
    base_cmap: set,
    lexicon: dict,
    overlay: dict,
    token_by_glyph: dict,
    tray_glyphs: set,
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

        tags_en, tags_pt, sets_seen = set(), set(), set()
        # DOES THIS GLYPH HAVE ANY REAL PORTUGUESE WORD?
        #
        # There is no answering that by looking at `tags_pt`: an untranslated token goes
        # in there untranslated, so the list NEVER comes out empty — it just fills up
        # with English pretending to be Portuguese. The only honest way is to mark
        # when a translation actually came from the lexicon or from the overlay.
        has_pt = False

        for g in glyphs:
            prefix, parts = tokenize(g)
            set_en, set_pt = SETS.get(prefix, (prefix, prefix))
            sets_seen.add(prefix)
            tags_en.add(set_en.lower())
            tags_pt.add(set_pt.lower())

            en, pt, matched = translate(parts, lexicon, untranslated)
            tags_en.update(en)
            tags_pt.update(pt)
            has_pt = has_pt or matched

            # Curated overlay: it ADDS, it does not replace.
            ov = overlay.get(g)
            if ov:
                tags_en.update(ov.get("en", []))
                tags_pt.update(ov.get("pt", []))
                has_pt = has_pt or bool(ov.get("pt"))

        # THE ACCENT IS RESOLVED HERE, and not at runtime.
        #
        # `localização` and `localizacao` have to find the same glyph. Normalising
        # in the component would cost walking 10,473 glyphs times ~20 tags on every
        # keystroke; here it costs once, at build time. What goes to the SCREEN
        # stays the accented form, which is how one writes.
        #
        # Only those that DIFFER go in: `mapa` does not get a twin identical to itself.
        search_terms = search_forms(tags_pt)

        rows.append(
            {
                "cp": cp,
                "hex": f"{cp:04X}",
                "char": chr(cp),
                "names": glyphs,
                "name": primary,
                "sets": sorted(sets_seen),
                "tags_en": sorted(tags_en),
                "tags_pt": sorted(tags_pt),
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
        if primary and not has_pt:
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
    ap.add_argument("--lexicon", type=pathlib.Path, required=True)
    # THE FLOOR COMES FROM OUTSIDE so the negative fixture can exercise the guard without
    # editing the real lexicon — see `nix/glyphgen.nix`.
    ap.add_argument("--min-coverage", type=float, default=90.0)
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
    lexicon = tomllib.loads(args.lexicon.read_text())
    overlay = tomllib.loads(args.overlay.read_text())

    # The overlay may only speak of glyphs that exist.
    orfaos = [g for g in overlay if g not in raw_names]
    if orfaos:
        die(
            f"overlay.toml cita {len(orfaos)} glifos inexistentes: {orfaos[:5]}\n"
            "       Use o nome sem o prefixo `nf-` (ex.: `fa-folder`, nao `nf-fa-folder`)."
        )

    icons = build_icon_table(tokens, raw_names, cmap, tray)
    token_by_glyph = {v["glyph"]: k for k, v in icons.items()}
    tray_glyphs = {icons[t]["glyph"] for t in tray.values() if t in icons}

    rows, stats = build_glyph_index(
        raw_names, cmap, base_cmap, lexicon, overlay, token_by_glyph, tray_glyphs
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
        1 for r in rows for t in r["tags_en"] if t in lexicon
    )
    coverage = (
        100.0 * total_tokens_pt / (total_tokens_pt + tot_occ) if (total_tokens_pt + tot_occ) else 0.0
    )

    lines = [
        "# Cobertura da camada de glifos",
        "",
        "Gerado por `nix/scripts/gen-glyphdb.py`. Nao editar a mao.",
        "",
        f"- Nerd Fonts: **{args.nf_version}**",
        f"- Codepoints Nerd Fonts na fonte: **{stats['nerd_codepoints']}**",
        f"- Com nome no glyphnames.json: **{stats['named']}**",
        f"- Sem nome (so codepoint): **{stats['unnamed']}**",
        f"- Tokens de icone: **{len(icons)}**",
        f"- Entradas no lexico pt-BR: **{len(lexicon)}**",
        f"- Entradas no overlay curado: **{len(overlay)}**",
        f"- Glifos nomeados SEM nenhuma tag em portugues: **{len(stats['no_pt'])}**",
        "",
        f"## Traducao de tags: {coverage:.1f}%",
        "",
        f"Piso exigido: **>= {args.min_coverage}%** das ocorrencias de token traduzidas.",
        "",
        "## Tokens sem traducao, por frequencia",
        "",
        "Esta e a fila de trabalho para crescer `assets/lexicon/tags.en-pt.toml`.",
        "Traduzir nesta ordem, nao na ordem que vier.",
        "",
        "| token | ocorrencias |",
        "|---|---:|",
    ]
    for tok, n in stats["untranslated"].most_common(60):
        lines.append(f"| `{tok}` | {n} |")
    if len(stats["untranslated"]) > 60:
        lines.append(f"| … | mais {len(stats['untranslated']) - 60} tokens |")

    (args.out / "coverage.md").write_text("\n".join(lines) + "\n")

    print(f"OK: {stats['nerd_codepoints']} glifos indexados "
          f"({stats['named']} nomeados, {stats['unnamed']} so por codepoint)")
    # --- THE TWO COVERAGE GUARDS -------------------------------------------
    #
    # They live in the GENERATOR, and not in a check that reads the JSON afterwards, for
    # the same reason as the icon token guard just above: what knows which glyph
    # was left untranslated is what built the table. A later check would have to
    # rediscover that from the output — and the output does NOT say, because an
    # untranslated token goes into `tags_pt` untranslated.
    #
    # Noisy on purpose: bumping the Nerd Fonts version starts failing until
    # the new glyphs are translated. It is the same design as the check that demands
    # translation of niri's actions.
    if stats["no_pt"]:
        amostra = stats["no_pt"][:12]
        print("", file=sys.stderr)
        print(f"ERRO: {len(stats['no_pt'])} glifos sem NENHUMA tag em portugues.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in amostra:
            print(f"       {g}", file=sys.stderr)
        if len(stats["no_pt"]) > len(amostra):
            print(f"       … e mais {len(stats['no_pt']) - len(amostra)}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Quem procura em portugues NAO acha estes glifos. Traduza os", file=sys.stderr)
        print("       tokens deles em assets/lexicon/tags.en-pt.toml — a fila, por", file=sys.stderr)
        print("       frequencia, esta no coverage.md que este mesmo build gera.", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Nome de marca NAO se traduz: ele ganha tags de CATEGORIA", file=sys.stderr)
        print("       (`mysql` -> en: database / pt: banco de dados).", file=sys.stderr)
        sys.exit(1)

    if coverage < args.min_coverage:
        print("", file=sys.stderr)
        print(f"ERRO: cobertura de traducao em {coverage:.1f}%, "
              f"abaixo do piso de {args.min_coverage}%.", file=sys.stderr)
        print("       A fila de trabalho, por frequencia, esta no coverage.md.", file=sys.stderr)
        sys.exit(1)

    print(f"OK: traducao de tags em {coverage:.1f}% "
          f"({len(stats['untranslated'])} tokens distintos sem traducao)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
