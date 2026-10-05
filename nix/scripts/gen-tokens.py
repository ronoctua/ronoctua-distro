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


#: The pages each theme is solved on, in the order of the runtime table's
#: second index (`ronoctua_tokens::theme::Ground`): the base first.
PAGES = ("chrome", "backdrop", "near", "absolute")


def load_themes(path: pathlib.Path) -> list[dict]:
    """`themes.json` from `nix/themes.nix`: the ordered list of themes, each
    `{"id": …, "mode": "dark" | "light", "colors": {key: "0xRRGGBBAA"}}`.

    Every theme has to name exactly the same colours. The generator over
    there already guarantees it; it is checked again here because a table
    with a hole in it would be the wrong colour on screen, never an error."""
    raw = json.loads(path.read_text())
    if not isinstance(raw, list) or not raw:
        raise SystemExit(f"ERROR: {path} is not a non-empty list of themes")
    themes = []
    for entry in raw:
        if not isinstance(entry, dict) or "id" not in entry or "colors" not in entry:
            raise SystemExit(f"ERROR: a theme in {path} is not {{id, colors}}: {entry!r:.80}")
        if not re.fullmatch(r"[a-z][a-z0-9-]*", entry["id"]):
            raise SystemExit(f"ERROR: the theme id {entry['id']!r} is not kebab-case")
        # THE MODE IS REFUSED WHEN IT IS NOT ONE OF THE TWO, never read as
        # dark: a light theme generated as dark would name its white desk
        # "black" on screen, and nothing would fail.
        if entry.get("mode") not in ("dark", "light"):
            raise SystemExit(f"ERROR: the theme {entry['id']!r} says its mode is {entry.get('mode')!r}")
        flat: dict = {}
        flatten("", entry["colors"], flat)
        # AND ONE PALETTE PER PAGE, in `PAGES` order after the base's: the
        # same roles solved on each desk (`gen-theme.py`, `Theme.on_page`).
        # A page that is missing, or one that names other roles than the
        # base, is refused: a hole in the table is the wrong colour on screen.
        pages = entry.get("pages")
        if not isinstance(pages, dict) or sorted(pages) != sorted(PAGES[1:]):
            raise SystemExit(f"ERROR: the theme {entry['id']!r} does not carry the pages {PAGES[1:]}")
        palettes = [flat]
        for page in PAGES[1:]:
            colours: dict = {}
            flatten("", pages[page], colours)
            if set(colours) != set(flat):
                raise SystemExit(f"ERROR: the theme {entry['id']!r} on {page} names other roles than its base: {sorted(set(colours) ^ set(flat))}")
            palettes.append(colours)
        themes.append({"id": entry["id"], "mode": entry["mode"], "colors": flat, "palettes": palettes})
    first = set(themes[0]["colors"])
    for theme in themes[1:]:
        if set(theme["colors"]) != first:
            print(
                f"ERROR: the theme `{theme['id']}` does not name the same colours as `{themes[0]['id']}`:",
                sorted(set(theme["colors"]) ^ first),
                file=sys.stderr,
            )
            raise SystemExit(1)
    ids = [t["id"] for t in themes]
    if len(set(ids)) != len(ids):
        raise SystemExit(f"ERROR: a theme id repeats: {ids}")
    return themes


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--themes",
        type=pathlib.Path,
        required=True,
        help="themes.json from nix/themes.nix: [{id, mode, colors}], in the chooser's order",
    )
    ap.add_argument(
        "--default-theme",
        required=True,
        help="the id of the theme a fresh machine shows; see assets/themes/themes.nix",
    )
    ap.add_argument("--typography", type=pathlib.Path, required=True)
    ap.add_argument("--space", type=pathlib.Path, required=True)
    ap.add_argument("--icons", type=pathlib.Path, required=True, help="icons.json from the glyphdb")
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
        help="category-glyphs.json: an ORDERED list of {category, token}",
    )
    ap.add_argument(
        "--brain-marks",
        type=pathlib.Path,
        help="brain-marks.json: the icon tokens that are a brain module's glyphs",
    )
    ap.add_argument("--out", type=pathlib.Path, required=True)
    ap.add_argument(
        "--out-wit",
        type=pathlib.Path,
        required=True,
        help="WIT enum of valid token names (section 7.6's (b))",
    )
    args = ap.parse_args()

    themes = load_themes(args.themes)
    typo = json.loads(args.typography.read_text())
    space_raw = json.loads(args.space.read_text())
    icons = json.loads(args.icons.read_text())
    brain_marks = json.loads(args.brain_marks.read_text()) if args.brain_marks else []
    strays = sorted(t for t in brain_marks if t not in icons)
    if strays:
        # `nix/glyphgen.nix` merges them into the table it measures, so a
        # token missing here is the two derivations reading different lists.
        print(f"ERROR: brain module glyphs missing from the icon table: {strays}", file=sys.stderr)
        return 1

    # The names are the FIRST theme's: every theme has the same set (checked
    # in `load_themes`), so the constants, the bridge and the WIT enum come
    # out identical whichever theme is looked at.
    colors: dict = themes[0]["colors"]
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

    # --- colours: one ROLE per key, and the values in a table per theme ------
    #
    # The constant is an index, not bytes: `Color::to_rgba8` looks it up in the
    # theme the process is in. Sorted, so the index is stable across generator
    # runs and the WIT enum (also sorted) lists the same names in the same order.
    roles = sorted(colors)
    A("/// Design system colours: one constant per ROLE, resolved through the")
    A("/// current theme (`theme::set`) when drawn.")
    A("///")
    A("/// There is no public `Color` constructor on purpose: invariant 9 says")
    A("/// that no colour lives outside the tokens, and the way to guarantee that")
    A("/// is for there to be no way to write one.")
    A("pub mod color {")
    A("    use super::Color;")
    for i, k in enumerate(roles):
        A(f"    /// `color.{k}`")
        A(f"    pub const {ident(k)}: Color = Color::at({i});")
    A("")
    A("    /// The token names, by role — what `Color::name` and `Debug` read.")
    A(f"    pub(crate) const NAMES: [&str; {len(roles)}] = [")
    for k in roles:
        A(f'        "{k}",')
    A("    ];")
    A("}")
    A("")

    # --- typography --------------------------------------------------------
    A("/// Typography. No crate names the font directly (section 6.2).")
    A("pub mod font {")
    A(f'    /// `font.family.mono` — the system font, patched.')
    A(f'    pub const FAMILY_MONO: &str = "{typo["family"]["mono"]}";')
    A(f'    /// `font.family.ui` — the same one: the system is monospaced by design.')
    A(f'    pub const FAMILY_UI: &str = "{typo["family"]["ui"]}";')
    # THE ONE SIZE THAT IS NOT IN PIXELS, taken out of the loop so its doc
    # cannot say the wrong unit. Ghostty is a foreign renderer and takes points.
    for k, v in sorted(typo["size"].items()):
        if k == "terminalPoints":
            continue
        A(f"    /// `font.size.{k}`, in logical px.")
        A(f"    pub const SIZE_{k.upper()}: f32 = {float(v)};")
    A("    /// `font.size.terminalPoints` — the foreign terminal's size, in POINTS.")
    A("    ///")
    A("    /// Ghostty draws with its own renderer and its `font-size` is in")
    A("    /// points, not the logical pixels every other size here is in. Both")
    A("    /// `nix/modules/terminal.nix` and `toolkits::tell_them` read it, which")
    A("    /// is why it is a token and not a literal in either.")
    A(f'    pub const TERMINAL_POINTS: f32 = {float(typo["size"]["terminalPoints"])};')
    A("")
    # THE RANGE THE PERSON MAY MOVE THE BASE IN, in percent. `u16` and not
    # `f32`: a percentage of a size is a whole number of steps, and a float
    # here would invite a scale nobody can arrive at by pressing a key.
    #
    # AND THE THREE HAVE TO AGREE WITH EACH OTHER, which nothing else checks:
    #
    #   - a floor above the ceiling makes `u16::clamp` PANIC, and it panics
    #     inside `resolve_at_start`, so the shell does not come up at all;
    #   - a step of zero divides by zero;
    #   - an end that is not a multiple of the step is an end the arrows
    #     cannot leave: `stepped` snaps back to it for ever, and the person
    #     is stuck at a size they can see and not change.
    #
    # None of those is a wrong PIXEL, which is what makes them worth refusing
    # here rather than finding on a screen.
    scale = typo["scale"]
    floor, ceiling, step = (int(scale[k]) for k in ("floor", "ceiling", "step"))
    if step < 1:
        raise SystemExit(f"font.scale.step is {step}; a step of nothing divides by zero")
    # A FLOOR OF ZERO PASSES EVERY OTHER GATE HERE — `0 % 5` is 0 and 100 is
    # still inside `0..200` — and a text size of zero makes a cell zero wide,
    # which `metrics::columns` divides by. The number is a percentage of a
    # size somebody has to read, so a quarter of the design's base is already
    # generous.
    if floor < 25:
        raise SystemExit(
            f"font.scale.floor is {floor}; below a quarter of the base the text "
            "stops being text, and a floor of zero divides the cell by nothing"
        )
    if floor > ceiling:
        raise SystemExit(f"font.scale.floor {floor} is above the ceiling {ceiling}")
    for name, end in (("floor", floor), ("ceiling", ceiling)):
        if end % step:
            raise SystemExit(
                f"font.scale.{name} is {end}, which is not a multiple of the step "
                f"{step} — the arrows would not be able to leave it"
            )
    if not floor <= 100 <= ceiling or 100 % step:
        raise SystemExit(
            f"100% is not reachable in {floor}..{ceiling} by steps of {step}, "
            "so the design's own size would not be a size anybody can return to"
        )
    # AND THE RANGE FITS WHAT THE FOREIGN TOOLKITS ACCEPT.
    #
    # The size does not stop at our own windows: `toolkits.rs` carries it to
    # GTK, Qt and Ghostty through `org.gnome.desktop.interface`. The GNOME
    # schema declares its scaling key as
    #
    #     <key name="text-scaling-factor" type="d"><range min="0.5" max="3.0"/>
    #
    # (`gsettings-desktop-schemas`, `org.gnome.desktop.interface.gschema.xml`),
    # and the font-size keys are read by toolkits that will simply refuse or
    # clamp a value they think absurd.
    #
    # `dconf write` DOES NOT VALIDATE against the schema — it writes the raw
    # GVariant — so a range widened here would leave our own windows correct
    # and every foreign application quietly disagreeing with them, which is
    # the one failure this whole feature exists to avoid. Nothing downstream
    # can catch it: the shell would be right.
    #
    # The bounds are the schema's, in percent.
    FOREIGN_FLOOR, FOREIGN_CEILING = 50, 300
    for name, end in (("floor", floor), ("ceiling", ceiling)):
        if not FOREIGN_FLOOR <= end <= FOREIGN_CEILING:
            raise SystemExit(
                f"font.scale.{name} is {end}%, outside the {FOREIGN_FLOOR}.."
                f"{FOREIGN_CEILING}% that the foreign toolkits accept — our own "
                "windows would follow it and GTK, Qt and Ghostty would not"
            )
    for k, v in sorted(typo["scale"].items()):
        A(f"    /// `font.scale.{k}`, in percent of `font.size.base`.")
        A(f"    pub const SCALE_{k.upper()}: u16 = {int(v)};")
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
    # TYPOGRAPHIC MARKS. A `char` and not a `&str`: every one of these is a
    # single character the host punctuates with, and a `char` is what refuses
    # the day somebody writes a word here. Host only, like `font` above — no
    # WIT enum, because invariant 11 does not let a value cross to a buffer.
    A("/// Marks the host punctuates with. Not icons: these are ordinary")
    A("/// characters of the text face, not Nerd Fonts glyph names.")
    A("pub mod mark {")
    for k, v in sorted(typo.get("marks", {}).items()):
        # ONE CHARACTER, and the type is what says so. A mark that grew into a
        # word would place as one cell and draw as several, which is the class
        # of defect `ink-is-not-a-count` exists for one layer down.
        if len(v) != 1:
            raise SystemExit(f"a mark is one character, and `{k}` is {v!r}")
        escaped = v.replace("\\", "\\\\").replace("'", "\\'")
        A(f"    /// `mark.{k}`")
        A(f"    pub const {ident(k)}: char = '{escaped}';")
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
    # THE CHROME SIZE, MEASURED. `font.size.icon` is the ceiling and the common
    # case; a glyph whose drawing already tops the digit at the text's size is
    # drawn smaller, and how much smaller was decided by `gen-glyphdb.py`
    # against the real face — see `chrome_size` there. Only the exceptions are
    # written out, grouped by size, so the function reads as the list of
    # glyphs that are NOT at the icon size.
    icon_px = float(typo["size"]["icon"])
    # ONE ARM PER CODEPOINT: two tokens may name the same glyph (`battery`
    # and `battery-full` both point at `md-battery`), and a repeated pattern
    # is an error under `-D warnings`.
    by_size: dict[float, dict[int, str]] = {}
    for k in sorted(icons):
        px = float(icons[k]["chrome"])
        if px != icon_px:
            cp = icons[k]["codepoint"]
            by_size.setdefault(px, {})[cp] = f"'\\u{{{cp:X}}}'"
    A("")
    A("    /// THE SIZE A CHROME GLYPH IS DRAWN AT beside text of the base size:")
    A("    /// `font::SIZE_ICON` brings a small drawing up to the digit's ink, and")
    A("    /// a drawing that already stands taller than the digit comes DOWN to")
    A("    /// it instead. Measured against the real face by the glyph generator,")
    A("    /// never by hand; the check `chrome-icons-match-the-text` re-measures")
    A("    /// every chrome glyph at the size written here.")
    A("    pub fn chrome_size(glyph: char) -> f32 {")
    if by_size:
        A("        match glyph {")
        for px in sorted(by_size):
            A(f"            {' | '.join(by_size[px][cp] for cp in sorted(by_size[px]))} => {px:.1f},")
        A("            _ => super::font::SIZE_ICON,")
        A("        }")
    else:
        # NO EXCEPTIONS TODAY, no `match` either: a match with only a wildcard
        # arm is `clippy::match_single_binding`, and the workspace denies
        # warnings — the build would fail inside generated code, far from the
        # token edit that emptied the list.
        A("        let _ = glyph;")
        A("        super::font::SIZE_ICON")
    A("    }")
    # THE INK ACROSS, MEASURED — where each glyph's drawing begins and ends,
    # from the pen's origin, in ems (`gen-glyphdb.py`, `across`). One arm per
    # pair of numbers, the codepoints that share it together, for the reason
    # `chrome_size` gives: a repeated pattern is an error under `-D warnings`.
    by_ink: dict[tuple[float, float], dict[int, str]] = {}
    for k in sorted(icons):
        span = icons[k].get("across")
        if span is None:
            continue
        cp = icons[k]["codepoint"]
        by_ink.setdefault((float(span[0]), float(span[1])), {})[cp] = f"'\\u{{{cp:X}}}'"
    seen: set[int] = set()
    A("")
    A("    /// WHERE A GLYPH'S INK BEGINS AND ENDS across, from the pen's origin, in")
    A("    /// ems of its size: `(left, right)`. Measured against the real face by")
    A("    /// the glyph generator, never by hand — the cells cannot say it, because")
    A("    /// one drawing fills its advance and the next starts a fifth of the way")
    A("    /// in. `None` for a glyph with no ink, or one that is not an icon token.")
    A("    #[must_use]")
    A("    pub fn ink_across(glyph: char) -> Option<(f32, f32)> {")
    A("        match glyph {")
    for left, right in sorted(by_ink):
        # A CODEPOINT TWO TOKENS SHARE is measured once and written once.
        cps = [cp for cp in sorted(by_ink[(left, right)]) if cp not in seen]
        seen.update(cps)
        if cps:
            A(
                f"            {' | '.join(by_ink[(left, right)][cp] for cp in cps)} => "
                f"Some(({left!r}, {right!r})),"
            )
    A("            _ => None,")
    A("        }")
    A("    }")
    # THE BRAIN'S MARKS, by token NAME.
    # A mark crosses the brain's socket as the token of its glyph
    # (`made-up-full`), and the modeline draws it from that. Only a module's
    # own tokens answer: a name that happened to match a chrome token would be a
    # module drawing a glyph it never declared.
    A("")
    A("    /// A BRAIN MODULE'S GLYPH, by its token name (`<module>-<name>`), and")
    A("    /// only a module's: a name that is not one of their glyphs is `None`,")
    A("    /// even when the chrome has a token of that name.")
    A("    #[must_use]")
    A("    pub fn brain_mark(token: &str) -> Option<char> {")
    if brain_marks:
        A("        match token {")
        for k in sorted(brain_marks):
            A(f'            "{k}" => Some({ident(k)}),')
        A("            _ => None,")
        A("        }")
    else:
        # NO MODULE DECLARES A GLYPH YET, and a `match` with only a wildcard
        # arm is `clippy::match_single_binding` — `chrome_size`'s reason.
        A("        let _ = token;")
        A("        None")
    A("    }")
    # AND A TEST OF THE ANSWER, generated with it: every module's token finds
    # its own glyph, and a CHROME token's name finds nothing — which is the
    # half that holds, and is proved, while no module declares a glyph yet.
    chrome = sorted(k for k in icons if k not in brain_marks)
    A("")
    A("    #[cfg(test)]")
    A("    mod brain_mark_answers {")
    A("        #[test]")
    A("        fn only_a_modules_own_glyph_answers() {")
    if chrome:
        A(f'            assert_eq!(super::brain_mark("{chrome[0]}"), None);')
    A('            assert_eq!(super::brain_mark(""), None);')
    for k in sorted(brain_marks):
        A(f'            assert_eq!(super::brain_mark("{k}"), Some(super::{ident(k)}));')
    A("        }")
    A("    }")
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

    # --- the themes' VALUES ------------------------------------------------
    #
    # `[theme][role]`, in the order of `assets/themes/themes.nix` and the sorted
    # order of the roles above. Every theme has every role (`load_themes`
    # checked), so the table is rectangular by construction.
    ids = [t["id"] for t in themes]
    if args.default_theme not in ids:
        raise SystemExit(
            f"ERROR: --default-theme {args.default_theme!r} is not one of the declared themes: {ids}"
        )
    default_index = ids.index(args.default_theme)
    A("/// The themes' VALUES, in `assets/themes/themes.nix` order. Only `Color`")
    A("/// reads the table.")
    A("///")
    A("/// THE ORDER IS NOT THE DEFAULT. It was, for as long as the owner wanted")
    A("/// the theme at the top of the chooser to be the one a fresh machine")
    A("/// shows; the two are separate declarations now, and `DEFAULT` is the")
    A("/// index of the one that is named `default` over there.")
    A("pub mod themes {")
    A("    /// The ids, in order.")
    A(f"    pub const IDS: [&str; {len(themes)}] = [{', '.join(repr(t['id']) for t in themes).replace(chr(39), chr(34))}];")
    A("    /// Which way each theme's ladder climbs, in the same order: `true` for")
    A("    /// a LIGHT theme (the paper is the ground and the rungs walk toward")
    A("    /// black). A colour cannot be asked which side it is on, and the shell")
    A("    /// has to know: the absolute desk is black in one and white in the other.")
    A(f"    pub const LIGHT: [bool; {len(themes)}] = [{', '.join('true' if t['mode'] == 'light' else 'false' for t in themes)}];")
    A("    /// The pages each theme is solved on, in the table's second index:")
    A("    /// the base (the chrome), then each desk (`theme::Ground`).")
    A(f"    pub const PAGES: [&str; {len(PAGES)}] = [{', '.join(chr(34) + p + chr(34) for p in PAGES)}];")
    A("    /// `[theme][page][role]` -> RGBA, roles in `color`'s order.")
    A(f"    pub static TABLE: [[[[u8; 4]; super::counts::COLORS]; {len(PAGES)}]; {len(themes)}] = [")
    for theme in themes:
        A(f"        // {theme['id']}")
        A("        [")
        for page, palette in zip(PAGES, theme["palettes"]):
            A(f"            // {theme['id']} on {page}")
            A("            [")
            for k in roles:
                r, g, b, a = parse_color(palette[k])
                A(f"                [{r}, {g}, {b}, {a}], // {k}")
            A("            ],")
        A("        ],")
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
    A("    /// The number of themes.")
    A(f"    pub const THEMES: usize = {len(themes)};")
    A("    /// The number of pages each theme is solved on: the base and the desks.")
    A(f"    pub const PAGES: usize = {len(PAGES)};")
    A("    /// The index of the theme a fresh machine shows.")
    A("    ///")
    A("    /// NOT nought. See `themes::IDS`, and `assets/themes/themes.nix` for")
    A("    /// why the order and the default stopped being one fact.")
    A(f"    pub const DEFAULT_THEME: usize = {default_index};")
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
        f"OK: {len(colors)} colours in {len(themes)} theme(s) × {len(PAGES)} pages, {len(spaces)} spacings, "
        f"{len(icons)} icons -> {args.out.name} and {args.out_wit.name}"
    )
    print("OK: no shadow/elevation token (invariant 15)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
