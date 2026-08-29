#!/usr/bin/env python3
"""Generates the Rust token table AND the WIT enum — sections 7.6 and 7.8.

"Tokens defined ONCE in Nix; the build generates (a) the shell's Rust table and
(b) a WIT enum of valid names. A non-existent token becomes a compile error in the
guest." (section 7.6)

Both come from here, from the SAME data. Two lists would diverge, and the divergence
would show up as the wrong colour on screen — not as an error.

# Why the WIT enum matters more than it seems

Without it, `color` in the protocol is a free string: a buffer names
`color.nonexistent`, compiles, and the host falls into a silent fallback. That was
exactly the state, and it is the same class as invariant 8's `[key]`.
With the enum, `wit-bindgen` generates a Rust enum in the guest and the wrong name does
not exist — the error comes from the compiler, not from the screen.

Invariant 9 says: no colour, font, size, spacing or icon codepoint
outside the tokens. The way to make that hold is not discipline, it is there being no
way to write a loose value: the `ronoctua-tokens` crate has no public
colour constructor, only the constants generated here.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys


def ident(name: str) -> str:
    """`surface.base` -> `SURFACE_BASE`; `app-firefox` -> `APP_FIREFOX`."""
    return re.sub(r"[^A-Za-z0-9]+", "_", name).strip("_").upper()


# WIT reserved words. An identifier that collides needs the `%` prefix.
# Without this, a token called `list` takes the parser down with "expected an
# identifier or string, found keyword `list`" — pointing at the enum's line
# and not at the token that caused it.
WIT_RESERVED = {
    "bool", "borrow", "char", "constructor", "enum", "error", "export", "f32",
    "f64", "flags", "from", "func", "future", "import", "include", "interface",
    "list", "option", "own", "package", "record", "resource", "result", "s16",
    "s32", "s64", "s8", "static", "stream", "string", "tuple", "type", "u16",
    "u32", "u64", "u8", "use", "variant", "with", "world", "as",
}


# Every segment of a WIT identifier starts with a LETTER. `wifi-0` is rejected
# with "invalid character in identifier '0'" — and the rejection happens inside a
# BUFFER's `wit_bindgen::generate!`, three derivations later, in a file
# that has nothing to do with tokens. It cost a whole session to find out that the
# cause was an icon's name.
VALID_SEGMENT = re.compile(r"^[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*$")


def wit_ident(name: str) -> str:
    """`surface.base` -> `surface-base`; `list` -> `%list`.

    WIT requires `kebab-case` and accepts neither dots nor capitals. The conversion is
    deterministic and reversible, so the host goes back from `surface-base` to
    `SURFACE_BASE` without needing a second table.

    It rejects here what WIT would reject over there, with the TOKEN'S NAME in the message:
    the parser's error points at the generated enum's line, which does not say where
    the name came from.
    """
    id = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    if not VALID_SEGMENT.match(id):
        print(
            f"ERROR: the token `{name}` becomes `{id}`, which is not a WIT identifier.\n"
            "      In WIT every hyphen-separated piece starts with a LETTER — "
            "`wifi-0` will not do, `wifi-weak` will.\n"
            "      The WIT `icon` enum is generated from these names; without this "
            "refusal the build would break inside a buffer, far from here.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    return f"%{id}" if id in WIT_RESERVED else id


def parse_color(v: str) -> tuple[int, int, int, int]:
    m = re.fullmatch(r"0x([0-9A-Fa-f]{8})", v)
    if not m:
        raise SystemExit(f"ERROR: malformed colour: {v!r} (expected 0xRRGGBBAA)")
    n = int(m.group(1), 16)
    return (n >> 24) & 0xFF, (n >> 16) & 0xFF, (n >> 8) & 0xFF, n & 0xFF


def flatten(prefix: str, obj, out: dict) -> None:
    for k, v in obj.items():
        key = f"{prefix}.{k}" if prefix else k
        if isinstance(v, dict):
            flatten(key, v, out)
        else:
            out[key] = v


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--colors", type=pathlib.Path, required=True)
    ap.add_argument("--typography", type=pathlib.Path, required=True)
    ap.add_argument("--space", type=pathlib.Path, required=True)
    ap.add_argument("--icons", type=pathlib.Path, required=True, help="icons.json do glyphdb")
    ap.add_argument(
        "--tray",
        type=pathlib.Path,
        required=True,
        help="tray-glyphs.json: app id -> icon token",
    )
    ap.add_argument(
        "--categories",
        type=pathlib.Path,
        required=True,
        help="category-glyphs.json: LISTA ordenada de {category, token}",
    )
    ap.add_argument("--out", type=pathlib.Path, required=True)
    ap.add_argument(
        "--out-wit",
        type=pathlib.Path,
        required=True,
        help="WIT enum of valid token names (section 7.6's (b))",
    )
    args = ap.parse_args()

    colors_raw = json.loads(args.colors.read_text())
    typo = json.loads(args.typography.read_text())
    space_raw = json.loads(args.space.read_text())
    icons = json.loads(args.icons.read_text())

    colors: dict = {}
    flatten("", colors_raw, colors)
    spaces: dict = {}
    flatten("", space_raw, spaces)

    # --- invariant 15's guard ----------------------------------------------
    # "There is no shadow token in the design system." If someone adds one,
    # the build stops here, with the reason on screen — and not six months later in a
    # UI review.
    forbidden = [k for k in colors if re.search(r"shadow|elevation|elevat", k, re.I)]
    if forbidden:
        print("ERROR: a shadow/elevation token found:", file=sys.stderr)
        for p in forbidden:
            print(f"       {p}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Invariant 15 says: no cards, no shadow, no elevation.", file=sys.stderr)
        print("       'There is no shadow token in the design system' (section 5.1).", file=sys.stderr)
        return 1

    L = []
    A = L.append
    A("// GENERATED by nix/scripts/gen-tokens.py. DO NOT EDIT BY HAND.")
    A("//")
    A("// The tokens are defined in assets/tokens/*.nix — once, there (section 7.6).")
    A("// Editing here means losing the change at the next build.")
    A("")

    # --- colours -----------------------------------------------------------
    A("/// Design system colours. RGBA format, 8 bits per channel.")
    A("///")
    A("/// There is no public `Color` constructor on purpose: invariant 9 says")
    A("/// that no colour lives outside the tokens, and the way to guarantee that")
    A("/// is for there to be no way to write one.")
    A("pub mod color {")
    A("    use super::Color;")
    for k in sorted(colors):
        r, g, b, a = parse_color(colors[k])
        A(f"    /// `color.{k}`")
        A(f"    pub const {ident(k)}: Color = Color::rgba({r}, {g}, {b}, {a});")
    A("}")
    A("")

    # --- typography --------------------------------------------------------
    A("/// Typography. No crate names the font directly (section 6.2).")
    A("pub mod font {")
    A(f'    /// `font.family.mono` — the system font, patched.')
    A(f'    pub const FAMILY_MONO: &str = "{typo["family"]["mono"]}";')
    A(f'    /// `font.family.ui` — the same one: the system is monospaced by design.')
    A(f'    pub const FAMILY_UI: &str = "{typo["family"]["ui"]}";')
    for k, v in sorted(typo["size"].items()):
        A(f"    /// `font.size.{k}`, in logical px.")
        A(f"    pub const SIZE_{k.upper()}: f32 = {float(v)};")
    A("")
    A("    /// Cell width, as a fraction of the font size.")
    A("    ///")
    A("    /// **Measured, not assumed**: the 135 faces have ONE advance per width,")
    A("    /// and at the normal width it is 600/1000 em — including in the 10,473")
    A("    /// Nerd Fonts glyphs, which are one cell wide.")
    A("    ///")
    A("    /// It is what makes the footer's layout integer arithmetic: there is no")
    A("    /// need to measure text to place a segment (sections 4.1 and 5.2).")
    A(f"    pub const CELL_WIDTH_EM: f32 = {float(typo['cellWidthEm'])};")
    A("")
    A("    /// Line height, as a multiple of the font size.")
    A(f"    pub const LINE_HEIGHT: f32 = {float(typo['lineHeight'])};")
    A("}")
    A("")

    # --- spacing -----------------------------------------------------------
    A("/// Spacing, in CELLS — not in pixels.")
    A("///")
    A("/// Spacing in cells keeps everything aligned to the text grid without")
    A("/// measuring anything, which is what the footer needs (section 5.1).")
    A("pub mod space {")
    for k in sorted(spaces):
        A(f"    /// `space.{k}`, in cells.")
        A(f"    pub const {ident(k)}: u16 = {int(spaces[k])};")
    A("}")
    A("")

    # --- icons -------------------------------------------------------------
    A("/// UI icons — glyphs from the font, never SVG (section 6.3, invariant 10).")
    A("///")
    A("/// The codepoints are resolved at BUILD time against the `cmap` of the")
    A("/// real font file. A token pointing at a non-existent glyph takes the")
    A("/// build down before reaching here.")
    A("pub mod icon {")
    for k in sorted(icons):
        e = icons[k]
        A(f"    /// `icon.{k}` — `{e['glyph']}` (U+{e['codepoint']:04X})")
        A(f"    pub const {ident(k)}: char = '\\u{{{e['codepoint']:X}}}';")
    A("}")
    A("")

    # --- app id -> glyph, for the app launcher ----------------------------
    #
    # The table had existed since the icons milestone and NEVER reached the Rust: the
    # `assets/icons/tray-glyphs.nix` was validated at build time (`gen-glyphdb.py`
    # takes down anyone pointing at a non-existent token) and stopped there. The `Chain` in
    # `ronoctua-notify` has the field `glyphs: &BTreeMap<String, char>` and never
    # had anyone to build it.
    #
    # Emitted as a SORTED slice of pairs, and not as a map: there are 29 entries,
    # the linear search costs less than building a `BTreeMap` at startup, and the
    # fixed order makes the generator's output reproducible.
    tray = json.loads(args.tray.read_text(encoding="utf-8"))
    missing = sorted(t for t in set(tray.values()) if t not in icons)
    if missing:
        # It should not happen — `gen-glyphdb.py` already rejects it. The guard is
        # here because the two generators read the SAME file through different
        # paths, and one of them can be changed without the other.
        print(f"ERROR: the tray points at a token that does not exist: {missing}", file=sys.stderr)
        return 1
    A("/// A known application -> glyph, from `assets/icons/tray-glyphs.nix`.")
    A("///")
    A("/// The FIRST step of the icon chain (section 4.2). Whatever is not")
    A("/// here falls back to the desktop entry category and then to the generic one.")
    A("pub mod tray {")
    A("    /// `(app id, glyph)` pairs, sorted by id.")
    A("    pub const TABLE: &[(&str, char)] = &[")
    for app in sorted(tray):
        cp = icons[tray[app]]["codepoint"]
        A(f"        (\"{app}\", '\\u{{{cp:X}}}'),")
    A("    ];")
    A("}")
    A("")

    # --- Categories= -> glyph, the chain's second step --------------------
    #
    # IN FILE ORDER, and not sorted. That is the whole point: a browser
    # declares `Network;WebBrowser`, both match, and sorting by name would make
    # `Network` win — every browser would turn into the network globe. That is why the
    # source is a LIST in Nix, and that is why it comes out of here as a slice.
    categories = json.loads(args.categories.read_text(encoding="utf-8"))
    missing = sorted({c["token"] for c in categories} - set(icons))
    if missing:
        print(f"ERROR: a category points at a token that does not exist: {missing}", file=sys.stderr)
        return 1
    seen: set[str] = set()
    for c in categories:
        # A repeated category would be a dead line: the first would already have
        # won, and whoever edited the second would see no effect at all.
        if c["category"] in seen:
            print(f"ERROR: repeated category: {c['category']}", file=sys.stderr)
            return 1
        seen.add(c["category"])
    A("/// The desktop entry `Categories=` -> glyph, IN PRECEDENCE ORDER.")
    A("///")
    A("/// From `assets/icons/category-glyphs.nix`. The order is the file's and not")
    A("/// alphabetical: `Network;WebBrowser` matches twice, and what has to")
    A("/// win is the specific one.")
    A("pub mod category {")
    A("    /// `(category, glyph)` pairs, in the order they should be tried.")
    A("    pub const TABLE: &[(&str, char)] = &[")
    for c in categories:
        cp = icons[c["token"]]["codepoint"]
        A(f"        (\"{c['category']}\", '\\u{{{cp:X}}}'),")
    A("    ];")
    A("}")
    A("")

    # --- WIT -> Rust bridge, with no fallback -----------------------------
    #
    # The guest names the colour by ENUM: an invalid name does not compile over there. On
    # this side the case name arrives in `kebab-case`, and the conversion is exhaustive —
    # `None` means a protocol violation, not "use a default".
    #
    # The difference matters: an `other => TEXT_PRIMARY` makes the wrong buffer
    # draw nicely, and the error only shows up when someone finds the colour odd.
    A("/// A design system colour, by the name WIT uses.")
    A("///")
    A("/// Generated from the SAME data as WIT's `enum color`. A name that arrives")
    A("/// here and does not resolve is a protocol violation — the guest cannot")
    A("/// produce it, because over there it is an enum.")
    A("#[must_use]")
    A("pub fn color_by_wit_name(name: &str) -> Option<Color> {")
    A("    Some(match name {")
    for k in sorted(colors):
        A(f'        "{wit_ident(k).lstrip("%")}" => color::{ident(k)},')
    A("        _ => return None,")
    A("    })")
    A("}")
    A("")
    A("/// A design system icon, by the name WIT uses. See `color_by_wit_name`.")
    A("#[must_use]")
    A("pub fn icon_by_wit_name(name: &str) -> Option<char> {")
    A("    Some(match name {")
    for k in sorted(icons):
        A(f'        "{wit_ident(k).lstrip("%")}" => icon::{ident(k)},')
    A("        _ => return None,")
    A("    })")
    A("}")
    A("")

    # --- counts, for the tests to assert on -------------------------------
    A("/// How many tokens of each kind were generated. The tests assert on")
    A("/// this to catch an empty or truncated generation.")
    A("pub mod counts {")
    A(f"    /// The number of colour tokens.")
    A(f"    pub const COLORS: usize = {len(colors)};")
    A(f"    /// The number of spacing tokens.")
    A(f"    pub const SPACES: usize = {len(spaces)};")
    A(f"    /// The number of icon tokens.")
    A(f"    pub const ICONS: usize = {len(icons)};")
    A("}")
    A("")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(L))

    # --- (b) the WIT enum --------------------------------------------------
    #
    # WIT names are `kebab-case` and do not accept dots: `color.surface.base`
    # becomes `surface-base`. The conversion is the same on both sides, so the host
    # knows how to go back without an extra table.
    W = []
    B = W.append
    B("// GENERATED by nix/scripts/gen-tokens.py. DO NOT EDIT BY HAND.")
    B("//")
    B("// Section 7.6's (b): 'a WIT enum of valid names; a non-existent token")
    B("// becomes a compile error in the guest'. From the same data as the Rust.")
    B("")
    B("package ronoctua:tokens@0.1.0;")
    B("")
    B("interface tokens {")
    B("    /// Design system colours, by NAME. The host resolves the value.")
    B("    ///")
    B("    /// A buffer cannot name a colour that does not exist: a name")
    B("    /// that is not here does not compile.")
    B("    enum color {")
    for k in sorted(colors):
        B(f"        // color.{k}")
        B(f"        {wit_ident(k)},")
    B("    }")
    B("")
    B("    /// UI icons — glyphs from the font (invariant 10).")
    B("    enum icon {")
    for k in sorted(icons):
        B(f"        {wit_ident(k)},")
    B("    }")
    B("}")
    B("")
    args.out_wit.parent.mkdir(parents=True, exist_ok=True)
    args.out_wit.write_text("\n".join(W))

    print(
        f"OK: {len(colors)} cores, {len(spaces)} espacamentos, "
        f"{len(icons)} icons -> {args.out.name} and {args.out_wit.name}"
    )
    print("OK: no shadow/elevation token (invariant 15)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
