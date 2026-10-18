#!/usr/bin/env python3
"""The declared fallback chain, as Rust.

Section 6.4 forbids the renderer from consulting fontconfig, and the reason is
not taste: fontconfig is not there in the installer or in a minimal
environment, and a renderer that needed it would draw tofu exactly where there
is nobody to debug it. So the chain is DECLARED — but declared twice, in Nix
for fontconfig and in Rust for the renderer, it would be two declarations, and
the whole point of `nix/fontconfig.nix` is that there is one.

This is what closes that: the same attrset that writes fontconfig's `.conf`
writes this file, and the renderer's `Chain` is generated from it. Editing the
chain therefore rebuilds the renderer and every unit above it, which is correct
— the chain IS code.

# What it does NOT generate

The families it names have to exist as FILES, and that is
`nix/coverage-fonts.nix`, from the same `coverage` list. Two derivations, one
declaration; neither can drift from the other without the build saying so.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

# The scripts `unicode_script::Script` knows. A key here that is not one of
# these would compile to a variant that does not exist, and the error would
# point at generated code rather than at the table somebody edited — so it is
# checked here, where the name of the offending key can still be printed.
#
# It is deliberately not the WHOLE enum: this is the set the chain may name,
# and it comes from what the Noto packages can actually answer for.
KNOWN = {
    "Adlam", "Arabic", "Armenian", "Bengali", "Bopomofo", "Braille", "Buhid",
    "Chakma", "Cherokee", "Deseret", "Devanagari", "Ethiopic", "Georgian",
    "Gothic", "Grantha", "Gujarati", "Gurmukhi", "Han", "Hangul", "Hanunoo",
    "Hebrew", "Hiragana", "Javanese", "Kannada", "Katakana", "Khmer", "Lao",
    "Malayalam", "Mongolian", "Myanmar", "Oriya", "Runic", "Sinhala", "Syriac",
    "Tagalog", "Tagbanwa", "Tai_Le", "Tai_Tham", "Tai_Viet", "Tamil", "Telugu",
    "Thaana", "Thai", "Tibetan", "Tifinagh", "Vai", "Yi",
}



def rust_string(s: str) -> str:
    """A Rust string literal. Family names have no escapes today; refuse one."""
    if '"' in s or "\\" in s:
        raise SystemExit(f"a family name needs escaping and this generator does not do it: {s!r}")
    return f'"{s}"'


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--chain-json", required=True)
    ap.add_argument("--primary-font", required=True,
                    help="the face asked for by name, which beats every fallback")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    chain = json.loads(pathlib.Path(args.chain_json).read_text(encoding="utf-8"))
    scripts: dict[str, dict] = chain["scripts"]
    han: dict[str, str] = chain["han"]
    han_scripts: dict[str, str] = chain["hanScripts"]
    common: list[str] = chain["common"]

    unknown = sorted(set(scripts) - KNOWN)
    if unknown:
        print("ERROR: nix/fontconfig.nix names scripts unicode-script does not have:", file=sys.stderr)
        for name in unknown:
            print(f"  - {name}", file=sys.stderr)
        print("      The key has to be a `unicode_script::Script` variant, spelt", file=sys.stderr)
        print("      its way — `Tai_Le`, not `TaiLe`.", file=sys.stderr)
        return 1

    unknown_han = sorted(set(han_scripts) - KNOWN)
    if unknown_han:
        print("ERROR: `hanScripts` names scripts unicode-script does not have:", file=sys.stderr)
        for name in unknown_han:
            print(f"  - {name}", file=sys.stderr)
        return 1

    # A script routed to a locale nobody declared would resolve, silently, to
    # the `other` face — Japanese kana drawn with Simplified Chinese glyphs.
    for name, whose in sorted(han_scripts.items()):
        if whose != "follow" and whose not in han:
            print(f"ERROR: `hanScripts.{name}` sends its script to the locale "
                  f"{whose!r}, which `han` does not name.", file=sys.stderr)
            print("      It would fall through to the default face, which draws", file=sys.stderr)
            print("      the characters and draws them wrong.", file=sys.stderr)
            return 1

    overlap = sorted(set(scripts) & set(han_scripts))
    if overlap:
        print("ERROR: a script is claimed twice, by `scripts` and by the Han unification:", file=sys.stderr)
        for name in overlap:
            print(f"  - {name}", file=sys.stderr)
        print("      Two answers to one question is one answer too many; the", file=sys.stderr)
        print("      CJK faces cover these, so remove them from `scripts`.", file=sys.stderr)
        return 1

    if "other" not in han:
        print("ERROR: the Han unification has no `other`, and every locale that", file=sys.stderr)
        print("       is not named would then have no CJK face at all.", file=sys.stderr)
        return 1

    # A PROBE THE PRIMARY FACE DRAWS PROVES NOTHING.
    #
    # The primary face is asked for by NAME, and a face asked for by name beats
    # every fallback on every codepoint it has — that is the rule the Lucide
    # remapping exists because of. So a probe it answers for would be checked,
    # would pass, and would say nothing about whether the promised family is
    # reachable at all.
    #
    # It is not hypothetical: `U+0531` was the Armenian probe, and the Iosevka
    # draws 91 of the 96 Armenian codepoints.
    from fontTools.ttLib import TTFont  # noqa: PLC0415 - only this check needs it

    primary = set(TTFont(args.primary_font, lazy=True).getBestCmap())
    useless = []
    for name in sorted(scripts):
        codepoint = int(scripts[name]["probe"].removeprefix("U+"), 16)
        if codepoint in primary:
            useless.append((name, scripts[name]["probe"]))
    if useless:
        print("ERROR: these probes are codepoints the PRIMARY face already draws:", file=sys.stderr)
        for name, probe in useless:
            print(f"  - {name}: {probe}", file=sys.stderr)
        print("      A face asked for by name beats every fallback, so the probe", file=sys.stderr)
        print("      would be answered by the Iosevka and the check that reads it", file=sys.stderr)
        print("      would pass without ever reaching the promised family.", file=sys.stderr)
        print("      Pick a codepoint of the same script that the Iosevka lacks.", file=sys.stderr)
        return 1

    lines: list[str] = []
    w = lines.append

    w("// GENERATED by nix/scripts/gen-chain.py from nix/fontconfig.nix. Do not edit.")
    w("//")
    w("// The declared fallback chain: which family answers for a script the")
    w("// primary face does not have. Section 6.4 — DECLARED, never discovered:")
    w("// the renderer does not consult fontconfig, because fontconfig is not")
    w("// there in the installer or in a minimal environment.")
    w("")
    w("/// The families tried when the script says nothing.")
    w("const COMMON: &[&str] = &[")
    for family in common:
        w(f"    {rust_string(family)},")
    w("];")
    w("")
    w("/// Never tried. Empty, and it is upstream's shape rather than ours.")
    w("const FORBIDDEN: &[&str] = &[];")
    w("")
    w("/// The CJK face for a locale — see `han` in nix/fontconfig.nix.")
    w("fn han_unification(locale: &str) -> &'static [&'static str] {")
    w("    match locale {")
    for tag in sorted(k for k in han if k != "other"):
        w(f"        {rust_string(tag)} => &[{rust_string(han[tag])}],")
    w(f"        _ => &[{rust_string(han['other'])}],")
    w("    }")
    w("}")
    w("")
    w("impl cosmic_text::fallback::Fallback for Chain {")
    w("    fn common_fallback(&self) -> &[&'static str] {")
    w("        COMMON")
    w("    }")
    w("")
    w("    fn forbidden_fallback(&self) -> &[&'static str] {")
    w("        FORBIDDEN")
    w("    }")
    w("")
    w("    fn script_fallback(")
    w("        &self,")
    w("        script: unicode_script::Script,")
    w("        locale: &str,")
    w("    ) -> &[&'static str] {")
    w("        use unicode_script::Script;")
    w("        match script {")
    for name in sorted(han_scripts):
        whose = han_scripts[name]
        argument = "locale" if whose == "follow" else rust_string(whose)
        w(f"            Script::{name} => han_unification({argument}),")
    for name in sorted(scripts):
        w(f"            Script::{name} => &[{rust_string(scripts[name]['family'])}],")
    w("            _ => &[],")
    w("        }")
    w("    }")
    w("}")
    w("")
    w("/// Every family this chain promises, and a codepoint only it should draw.")
    w("///")
    w("/// What makes the table above a promise instead of a wish:")
    w("/// `ronoctua-test-faces` asks the built database who draws each probe.")
    w("pub const PROMISED: &[(&str, char)] = &[")
    for name in sorted(scripts):
        entry = scripts[name]
        codepoint = int(entry["probe"].removeprefix("U+"), 16)
        w(f"    ({rust_string(entry['family'])}, '\\u{{{codepoint:04X}}}'),")
    w("];")
    w("")
    w("/// Every family the database has to be able to answer for, in any locale.")
    w("pub const COVERAGE: &[&str] = &[")
    for family in chain["coverage"]:
        w(f"    {rust_string(family)},")
    w("];")
    w("")

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"OK: {len(scripts)} scripts, {len(han_scripts)} answered by the Han "
          f"unification over {len(han)} locales, {len(common)} common, "
          f"{len(chain['coverage'])} families promised")
    return 0


if __name__ == "__main__":
    sys.exit(main())
