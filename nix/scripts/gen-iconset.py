#!/usr/bin/env python3
"""The glyph browser's index with the brain modules' tokens — `glyphs.json` of
`RONOCTUA_ICONSET` (`nix/iconset.nix`), the 2026-10-12 wave (B2).

The glyph database (`gen-glyphdb.py`) names each glyph of its index by the
token that draws it (`token`: the row the icon browser shows, the `token:`
filter, copying a glyph as `icon.<token>`), and it knows only the chrome's
tokens: a brain module's glyph is measured into the brain's catalogue instead
(`nix/brain-catalogue.nix`), so that a person's module never moves the table
every crate that draws compiles. Those tokens are names a person writes too —
a module's notification `icon` may say one — so the index the browser reads at
run time names them again, here, out of the same measured tokens the catalogue
carries: data joined to data, and nothing compiled.

THE RULE IS THE GLYPH DATABASE'S, not a second one: a glyph is named by the
last token, in name order, of every token that draws its primary name — the
chrome's and the modules' together, exactly as the database named them when it
measured both. The database's own rows are asked first whether they follow that
rule over the chrome's tokens alone; a database that names its glyphs another
way stops this build, rather than the browser naming a glyph two ways.

WHAT IT REFUSES: a module's token that is also the chrome's (one name, two
drawings — the catalogue refuses it too), and a database whose rows do not
follow the rule.

Usage:
    gen-iconset.py --glyphs <glyphdb glyphs.json> --icons <glyphdb icons.json>
        --modules <the modules' measured icons.json> --out <glyphs.json>
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def fail(what: str) -> None:
    print(f"ERROR: the glyph browser's index: {what}", file=sys.stderr)
    raise SystemExit(1)


def named_by(tokens: dict) -> dict:
    """Glyph name -> the token that names it: the last, in name order, of the
    tokens that draw it (`gen-glyphdb.py`'s `token_by_glyph`, over the table it
    writes in name order)."""
    by_glyph: dict[str, str] = {}
    for token in sorted(tokens):
        by_glyph[tokens[token]["glyph"]] = token
    return by_glyph


def main() -> int:
    ap = argparse.ArgumentParser(description="The glyph browser's index, with the brain modules' tokens.")
    ap.add_argument("--glyphs", type=Path, required=True, help="the glyph database's glyphs.json")
    ap.add_argument("--icons", type=Path, required=True, help="the glyph database's icons.json (the chrome's tokens)")
    ap.add_argument("--modules", type=Path, required=True, help="the brain modules' tokens, measured (icons.json)")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()

    rows = json.loads(args.glyphs.read_text(encoding="utf-8"))
    chrome = json.loads(args.icons.read_text(encoding="utf-8"))
    modules = json.loads(args.modules.read_text(encoding="utf-8"))

    for token in sorted(set(modules) & set(chrome)):
        fail(f"`{token}` is a brain module's glyph token and one of assets/tokens/icons.nix")

    # THE DATABASE'S RULE, asked of its own rows over the chrome's tokens.
    alone = named_by(chrome)
    for row in rows:
        if row.get("token") != alone.get(row.get("name")):
            fail(
                f"the glyph database names {row.get('name')!r} `{row.get('token')}`, and its rule over the"
                f" chrome's tokens says `{alone.get(row.get('name'))}`: it names its glyphs another way than this"
            )

    together = named_by({**chrome, **modules})
    theirs = 0
    for row in rows:
        row["token"] = together.get(row.get("name"))
        if row["token"] in modules:
            theirs += 1

    # THE DATABASE'S OWN SERIALISATION (`gen-glyphdb.py`): compact, the
    # characters as they are.
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(rows, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    named = sum(1 for row in rows if row["token"] is not None)
    print(f"OK: the glyph browser's index names {named} glyphs by a token, {theirs} of them a brain module's")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
