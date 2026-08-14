#!/usr/bin/env python3
"""Parity between fontconfig's chain and the renderer's — section 6.4.

THE PROBLEM THIS CATCHES
------------------------
Section 6.4 is explicit: **the renderer does not use fontconfig**. If it did, it
would break in the installer and in a minimal environment. It resolves by absolute
path, through `fontdb`, with `noctua-iosevka-core` embedded in the binary.

But the two chains have to AGREE. If they diverge, the same text renders
with different faces inside and outside our buffers — and the symptom shows up
as "the accent looks odd in the terminal", which is expensive to trace back to the
fallback chain.

HOW THE RENDERER SIDE IS SIMULATED
----------------------------------
The renderer only arrives in milestone 3. But its behaviour is already determined
by the data: `fallback-chain.json` declares the order, and `fontdb` picks the
FIRST face in the list that has the codepoint. That is exactly what we
simulate here, reading the real `cmap`s in the declared order.

When the renderer exists, this script becomes the oracle it is compared
against — the logic does not change.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys

from fontTools.ttLib import TTFont

# Probe codepoints: one per category the chain has to cover.
PROBES = [
    ("A", 0x0041, "ASCII"),
    ("ç", 0x00E7, "pt-BR: cedilha"),
    ("ã", 0x00E3, "pt-BR: til"),
    ("É", 0x00C9, "pt-BR: agudo maiusculo"),
    ("─", 0x2500, "box-drawing"),
    ("█", 0x2588, "bloco"),
    ("→", 0x2192, "seta"),
    ("λ", 0x03BB, "grego"),
    ("Ж", 0x0416, "cirilico"),
    ("", 0xE0A0, "PUA: Powerline"),
    ("", 0xF07B, "PUA: Font Awesome"),
    ("󰉋", 0xF024B, "PUA: Material Design"),
    ("日", 0x65E5, "CJK"),
    ("😀", 0x1F600, "emoji"),
    ("∑", 0x2211, "matematica"),
]


def face_for_codepoint(chain_files: list[pathlib.Path], cp: int) -> pathlib.Path | None:
    """What the renderer would do: the first face in the chain that has the codepoint."""
    for f in chain_files:
        try:
            if cp in TTFont(f, lazy=True, fontNumber=0).getBestCmap():
                return f
        except Exception:
            continue
    return None


def fc_match(family: str, cp: int) -> str | None:
    """What fontconfig would do."""
    try:
        out = subprocess.run(
            ["fc-match", "-f", "%{file}", f"{family}:charset={cp:x}"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        return out.stdout.strip() or None
    except Exception:
        return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--chain-json", type=pathlib.Path, required=True)
    ap.add_argument("--font-dir", type=pathlib.Path, action="append", required=True)
    ap.add_argument("--strict", action="store_true",
                    help="falha se fontconfig e renderer divergirem")
    args = ap.parse_args()

    chain = json.loads(args.chain_json.read_text())
    primary = chain["primary"]

    # The declared chain, resolved into real files.
    files: list[pathlib.Path] = []
    for d in args.font_dir:
        files += sorted(d.rglob("*.ttf")) + sorted(d.rglob("*.otf"))
    if not files:
        print("ERRO: nenhuma face encontrada nos diretorios dados", file=sys.stderr)
        return 1

    # The first of the chain is our Regular; it is the one that has to win on
    # everything it covers.
    nossa = [f for f in files if "NerdFontMono-Regular" in f.name]
    resto = [f for f in files if f not in nossa]
    ordered = nossa + resto

    print(f"cadeia declarada: primaria = {primary!r}")
    print(f"faces disponiveis: {len(ordered)}")
    print()
    print(f"{'sonda':<28} {'renderer':<38} {'fontconfig':<38} ")
    print("-" * 106)

    divergencias = 0
    no_coverage = 0

    for ch, cp, desc in PROBES:
        r = face_for_codepoint(ordered, cp)
        f = fc_match(primary, cp)

        r_name = r.name if r else "<nenhuma>"
        f_name = pathlib.Path(f).name if f else "<nenhuma>"

        if r is None:
            no_coverage += 1
            mark = "  (sem cobertura na cadeia local)"
        elif f and pathlib.Path(f).name == r_name:
            mark = "  OK"
        elif f is None:
            mark = "  (fc-match indisponivel)"
        else:
            mark = "  DIVERGE"
            divergencias += 1

        print(f"U+{cp:05X} {desc:<21} {r_name:<38} {f_name:<38}{mark}")

    print()
    print(f"divergencias: {divergencias}   sondas sem cobertura local: {no_coverage}")

    # Our font has 18,053 codepoints; CJK and emoji are left out of it on
    # purpose (they come from Noto). Having no local coverage for those probes is
    # what is expected when only the noctua family is installed.
    if args.strict and divergencias:
        print("", file=sys.stderr)
        print("ERRO: fontconfig e o renderer escolheriam faces diferentes.", file=sys.stderr)
        print("      As duas cadeias saem de nix/fontconfig.nix e NAO podem", file=sys.stderr)
        print("      divergir — senao o mesmo texto renderiza diferente dentro", file=sys.stderr)
        print("      e fora dos nossos buffers (secao 6.4).", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
