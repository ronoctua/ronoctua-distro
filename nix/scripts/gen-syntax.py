#!/usr/bin/env python3
"""Turns `assets/syntax.nix` into the highlighter's Rust table.

WHY A GENERATOR AND NOT A RUST TABLE
------------------------------------
Because the second wave of languages is DATA. Adding Erlang is one attrset,
with no Rust change, no new role and no new colour — and that is only true if
the declaration is the source and the Rust is derived. Written by hand, every
language would be a diff in a source file, and the day somebody adds one that
claims a type another already claims, nothing would say so.

WHAT IT EMITS
-------------
One self-contained `syntax.rs`: its own `Kind`, `Numbers`, `Property`,
`Delimiter`, `Block`, `Attribute`, `Grammar` and `Language`, plus the sorted
tables `GRAMMARS`, `OF_MIME`, `OF_TAG`, `OF_SHEBANG`, `SNIFF` and `counts`.

Self-contained deliberately, and it is the choice `gen-updates.py` made for the
same reason: a table that leans on hand-written Rust types needs a check
comparing the two vocabularies. Here there is no second copy to compare.

WHAT IT REFUSES
---------------
Thirteen things, each naming the offending id. They are listed in the header of
`assets/syntax.nix` and each `fail()` below says why.
"""

import argparse
import json
import re
import sys
from pathlib import Path

# FLOORS, not exact counts, and the idiom is `gen-mimedb.py`'s: a reader that
# stopped understanding its input gives back a handful of rows, and that reads
# as "the table is small" rather than "the reader broke".
FLOOR_LANGUAGES = 30
FLOOR_MIMES = 100

KINDS = ("code", "markup", "markdown", "diff")
NUMBERS = ("none", "plain", "c")
PROPERTIES = ("none", "colon", "equals")
SNIFF_SHAPES = ("none", "json-object")
SHAPES = ("capitalised-is-type", "screaming-is-constant")

ID = re.compile(r"^[a-z][a-z0-9-]*$")
TAG = re.compile(r"^[a-z0-9+#-]+$")

# WHAT MAY NOT BE CLAIMED. Prose is the load-bearing one: `text/plain` is what
# a file with no better answer resolves to, so a language claiming it would
# colour every essay, every log and every README on the machine.
FORBIDDEN = ("text/plain", "application/octet-stream")
FORBIDDEN_PREFIX = ("inode/",)

# THE LONGEST DELIMITER THE SCANNER COMPARES. It matches fixed byte windows, so
# a multi-byte opener could match inside a character; and four covers every
# real case (`"""`, `r"`, `<!--`, `--[[`, `{-#`).
DELIMITER_MAX = 4

failed = False


def fail(message: str) -> None:
    global failed
    print(f"ERROR: {message}", file=sys.stderr)
    failed = True


def is_word_char(ch: str) -> bool:
    """The scanner's own rule, and the generator has to use the SAME one.

    A keyword the scanner could never match is a dead row that looks
    deliberate — the shape `gen-mimedb.py` already refuses for a glob that
    matches nothing.
    """
    return ch.isalnum() or ch in "_$?"


def fold(mime: str, aliases: dict[str, str]) -> str:
    """Aliases, followed to the end.

    Two hops happen upstream (`text/x-yaml` -> `application/x-yaml` ->
    `application/yaml`), so a single lookup would leave two of the three names
    pointing at a type nothing claims.
    """
    seen = set()
    while mime in aliases and mime not in seen:
        seen.add(mime)
        mime = aliases[mime]
    return mime


def descendants(parents: dict[str, list[str]]) -> dict[str, list[tuple[str, int]]]:
    """For each type, its ancestors and how far up each one is.

    Distance is what settles a contest: a language claiming `text/x-csrc`
    directly beats one that inherited it, and a nearer ancestor beats a
    farther one. Without the distance, C++ and C would fight over every header
    and the winner would be whichever was written first.
    """
    out: dict[str, list[tuple[str, int]]] = {}
    for child in parents:
        chain: list[tuple[str, int]] = []
        frontier = [(child, 0)]
        seen = {child}
        while frontier:
            here, depth = frontier.pop(0)
            if depth:
                chain.append((here, depth))
            # EIGHT ROUNDS, the ceiling `ronoctua_files::mime::is_a` already
            # uses. The DAG is not deep and a cycle upstream must not hang a
            # build.
            if depth >= 8:
                continue
            for up in parents.get(here, ()):
                if up not in seen:
                    seen.add(up)
                    frontier.append((up, depth + 1))
        out[child] = chain
    return out


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def variant(identifier: str) -> str:
    """`objective-c` -> `ObjectiveC`. It becomes a Rust name (invariant 18)."""
    return "".join(p.capitalize() for p in identifier.split("-"))


def check_words(lang: dict, field: str, words: list[str]) -> None:
    for w in words:
        if not w or not all(is_word_char(c) for c in w):
            fail(
                f"`{lang['id']}` lists `{w}` under `{field}`, which is not a word by the "
                "scanner's own rule. It could never match, and a row that matches nothing "
                "looks deliberate and nobody audits it."
            )


def check_one(lang: dict) -> None:
    ident = lang["id"]
    if not ID.match(ident):
        fail(f"`{ident}` is not lower-case kebab; it becomes a Rust variant (invariant 18)")

    for field, allowed in (
        ("kind", KINDS),
        ("numbers", NUMBERS),
        ("property", PROPERTIES),
        ("sniff-shape", SNIFF_SHAPES),
    ):
        if lang[field] not in allowed:
            fail(
                f"`{ident}` sets `{field} = {lang[field]!r}`, which is not one of "
                f"{list(allowed)}. The Rust matches this exhaustively, so a value invented "
                "here would be a row that silently means nothing."
            )
    for shape in lang["shapes"]:
        if shape not in SHAPES:
            fail(f"`{ident}` names the shape `{shape}`, which is not one of {list(SHAPES)}")

    # NOTHING CAN EVER SELECT IT.
    if not (lang["mime"] or lang["tags"] or lang["sniff"] or lang["shebang"]):
        fail(
            f"`{ident}` declares no mime, no tag, no sniff and no shebang. "
            "Nothing could ever select it, so it is a table row that never runs."
        )

    # A `code` LANGUAGE WITH NOTHING TO COLOUR.
    #
    # Unless its whole surface is a marker: TeX is `\\command` and prose, and
    # prose is not coloured, so an attribute prefix and no words is a complete
    # answer rather than an empty one. Everything else with three empty lists
    # would draw the grey wall the highlighter exists to remove.
    if (
        lang["kind"] == "code"
        and lang["attribute"] is None
        and not (lang["keywords"] or lang["types"] or lang["constants"])
    ):
        fail(
            f"`{ident}` is `kind = \"code\"`, declares no attribute prefix, and has no "
            "keyword, no type and no constant. It would draw a grey wall, which is what "
            "the highlighter exists to remove."
        )

    for field in ("keywords", "types", "constants"):
        check_words(lang, field, lang[field])

    # THE SAME WORD IN TWO LISTS. The first would win in silence, and which one
    # is first is an implementation detail nobody should have to know.
    seen: dict[str, str] = {}
    for field in ("keywords", "types", "constants"):
        for w in lang[field]:
            if w in seen:
                fail(
                    f"`{ident}` lists `{w}` under both `{seen[w]}` and `{field}`. "
                    "The first would win in silence."
                )
            seen[w] = field

    for d in lang["strings"]:
        for side in ("open", "close"):
            text = d[side]
            if not text:
                fail(f"`{ident}` has a string delimiter with an empty `{side}`")
                continue
            if len(text.encode("utf-8")) > DELIMITER_MAX or not text.isascii():
                fail(
                    f"`{ident}`'s delimiter {text!r} is over {DELIMITER_MAX} bytes or not "
                    "ASCII. The scanner compares fixed byte windows, and a multi-byte "
                    "opener could match inside a character."
                )
            if "\n" in text:
                fail(f"`{ident}` has a delimiter carrying a newline: {text!r}")

    # A COMMENT MARKER THAT ALSO OPENS A STRING. "Comment beats quotes" is a
    # locked rule, so that string could never be reached: a dead rule that
    # looks deliberate.
    openers = {d["open"] for d in lang["strings"]}
    for marker in lang["line-comment"]:
        if marker in openers:
            fail(
                f"`{ident}` uses `{marker}` as a line comment AND as a string opener. "
                "Comment beats quotes, so that string is unreachable."
            )

    block = lang["block-comment"]
    if block is not None:
        if not block["open"] or not block["close"]:
            fail(f"`{ident}` has a block comment with an empty `open` or `close`")
        elif block["nests"] and block["open"] == block["close"]:
            fail(
                f"`{ident}` says its block comment nests and opens and closes with the same "
                f"{block['open']!r}. Nothing could tell a second open from a close."
            )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--syntax", required=True, help="assets/syntax.nix, as JSON")
    ap.add_argument("--types", required=True, help="types.json from the MIME derivation")
    ap.add_argument("--out-rust", required=True)
    args = ap.parse_args()

    declared = json.loads(Path(args.syntax).read_text(encoding="utf-8"))["languages"]
    db = json.loads(Path(args.types).read_text(encoding="utf-8"))
    known: set[str] = set(db["types"])
    aliases: dict[str, str] = dict(a for a in db["aliases"])
    parents: dict[str, list[str]] = {}
    for child, parent in db["subclasses"]:
        parents.setdefault(child, []).append(parent)
        parents.setdefault(parent, [])
    ancestry = descendants(parents)

    # --- one at a time -----------------------------------------------------
    ids: set[str] = set()
    tags: dict[str, str] = {}
    for lang in declared:
        check_one(lang)
        if lang["id"] in ids:
            fail(f"`{lang['id']}` is declared twice")
        ids.add(lang["id"])
        for tag in lang["tags"]:
            if not TAG.match(tag):
                fail(f"`{lang['id']}` has the fence tag `{tag}`, which is not a plain word")
            if tag in tags:
                fail(
                    f"`{lang['id']}` and `{tags[tag]}` both answer to the fence tag `{tag}`. "
                    "A fence would be coloured by whichever came first."
                )
            tags[tag] = lang["id"]

    # --- the claims, folded and expanded -----------------------------------
    # A type is claimed DIRECTLY at distance 0; every descendant of a claimed
    # type inherits it at its own distance. Nearer wins, and a tie is refused.
    direct: dict[str, str] = {}
    for lang in declared:
        for mime in lang["mime"]:
            if mime in FORBIDDEN or mime.startswith(FORBIDDEN_PREFIX):
                fail(
                    f"`{lang['id']}` claims `{mime}`. That is prose or a container, and "
                    "colouring prose is worse than not colouring it."
                )
                continue
            if mime not in known:
                fail(
                    f"`{lang['id']}` claims `{mime}`, which shared-mime-info "
                    f"{db['version']} does not have.\n"
                    "       A type is invented in the `ours` block of "
                    "assets/icons/mime-glyphs.nix, with its glob and its icon token —\n"
                    "       inventing it here would be a SECOND place deciding what a file is."
                )
                continue
            folded = fold(mime, aliases)
            if folded in direct:
                fail(
                    f"`{lang['id']}` and `{direct[folded]}` both claim `{folded}`. "
                    "The first one written would win in silence."
                )
                continue
            direct[folded] = lang["id"]

    resolved: dict[str, tuple[str, int]] = {m: (i, 0) for m, i in direct.items()}
    for mime, chain in ancestry.items():
        folded = fold(mime, aliases)
        if folded in direct:
            continue
        best: list[tuple[str, int]] = []
        for ancestor, depth in chain:
            owner = direct.get(fold(ancestor, aliases))
            if owner is not None:
                best.append((owner, depth))
        if not best:
            continue
        nearest = min(d for _, d in best)
        winners = sorted({o for o, d in best if d == nearest})
        if len(winners) > 1:
            fail(
                f"`{folded}` is inherited at the same distance by {winners}. "
                "Neither is nearer, so nothing decides."
            )
            continue
        resolved[folded] = (winners[0], nearest)

    # Every alias of a resolved type resolves to the same language, so the
    # index answers the name a file actually arrived under.
    for name, target in aliases.items():
        settled = fold(target, aliases)
        if settled in resolved and name not in resolved:
            resolved[name] = resolved[settled]

    if len(declared) < FLOOR_LANGUAGES:
        fail(
            f"only {len(declared)} languages, below the floor of {FLOOR_LANGUAGES}. "
            "A reader that stopped understanding its input gives back few rows."
        )
    if len(resolved) < FLOOR_MIMES:
        fail(
            f"only {len(resolved)} MIME types resolved, below the floor of {FLOOR_MIMES}. "
            "The subclass expansion is what makes this table small and its reach large; "
            "this few means the expansion did not run."
        )

    if failed:
        return 1

    out = emit(declared, resolved, tags)
    Path(args.out_rust).write_text("\n".join(out) + "\n", encoding="utf-8")
    print(
        f"OK: {len(declared)} languages, {len(resolved)} MIME types, {len(tags)} fence tags "
        f"-> {args.out_rust}"
    )
    return 0


def emit(declared: list[dict], resolved: dict[str, tuple[str, int]], tags: dict[str, str]) -> list[str]:
    of = {lang["id"]: variant(lang["id"]) for lang in declared}
    out = [
        "// GENERATED by nix/scripts/gen-syntax.py. Do not edit: it vanishes at the next build.",
        "//",
        "// From `assets/syntax.nix`, which is where a language is declared, and from the",
        "// MIME database `nix/mimegen.nix` publishes, which is where a type exists.",
        "",
        "/// How a language is read as a whole.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum Kind {",
    ]
    for k in KINDS:
        out += [f"    /// `{k}`.", f"    {variant(k)},"]
    out += ["}", "", "/// What shape a numeral takes.", "#[derive(Debug, Clone, Copy, PartialEq, Eq)]", "pub enum Numbers {"]
    for n in NUMBERS:
        out += [f"    /// `{n}`.", f"    {variant(n)},"]
    out += ["}", "", "/// What marks the key side of a pair.", "#[derive(Debug, Clone, Copy, PartialEq, Eq)]", "pub enum Property {"]
    for pr in PROPERTIES:
        out += [f"    /// `{pr}`.", f"    {variant(pr)},"]
    out += [
        "}",
        "",
        "/// A shape the scanner may look for, on top of the word lists.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum SniffShape {",
    ]
    for s in SNIFF_SHAPES:
        out += [f"    /// `{s}`.", f"    {variant(s)},"]
    out += [
        "}",
        "",
        "/// One way of opening quoted text.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct Delimiter {",
        "    /// What opens it.",
        "    pub open: &'static str,",
        "    /// What closes it.",
        "    pub close: &'static str,",
        "    /// What makes the next character literal; empty when there is none.",
        "    pub escape: &'static str,",
        "    /// Whether it may reach the next line.",
        "    pub spans_lines: bool,",
        "    /// Whether `#` may repeat between the marker and the quote (Rust's raw form).",
        "    pub hashes: bool,",
        "    /// The most characters it may hold; `None` is no limit.",
        "    ///",
        "    /// It is what stops Rust's `&'a str` from opening a string that never",
        "    /// closes and painting the rest of the file one colour.",
        "    pub max: Option<u16>,",
        "}",
        "",
        "/// A comment that reaches across lines.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct Block {",
        "    /// What opens it.",
        "    pub open: &'static str,",
        "    /// What closes it.",
        "    pub close: &'static str,",
        "    /// Whether a second `open` inside it has to be closed too.",
        "    pub nests: bool,",
        "}",
        "",
        "/// A marker ABOUT the thing rather than the thing.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct Attribute {",
        "    /// What starts it.",
        "    pub prefix: &'static str,",
        "    /// What ends it; `None` means it ends with the word.",
        "    pub close: Option<&'static str>,",
        "}",
        "",
        "/// Everything the scanner needs about one language.",
        "///",
        "/// The scanner takes THIS and never a [`Language`], which is what makes it",
        "/// testable with a grammar written as a Rust literal — no generated table, no",
        "/// environment variable, no Nix.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct Grammar {",
        "    /// Its id, as `assets/syntax.nix` spells it.",
        "    pub id: &'static str,",
        "    /// Its name, which is a proper noun and is not translated.",
        "    pub label: &'static str,",
        "    /// How it is read as a whole.",
        "    pub kind: Kind,",
        "    /// What starts a comment that ends with the line; longest first.",
        "    pub line_comments: &'static [&'static str],",
        "    /// The comment that reaches across lines, when there is one.",
        "    pub block_comment: Option<Block>,",
        "    /// The ways of opening quoted text; longest opener first.",
        "    pub strings: &'static [Delimiter],",
        "    /// What starts an interpolation inside quoted text.",
        "    pub interpolation: &'static [&'static str],",
        "    /// What opens a fenced block; markdown only.",
        "    pub fences: &'static [&'static str],",
        "    /// The marker ABOUT a thing, when there is one.",
        "    pub attribute: Option<Attribute>,",
        "    /// Its reserved words, SORTED for `binary_search`.",
        "    pub keywords: &'static [&'static str],",
        "    /// Its type names, SORTED.",
        "    pub types: &'static [&'static str],",
        "    /// Its literal constants, SORTED.",
        "    pub constants: &'static [&'static str],",
        "    /// What shape a numeral takes.",
        "    pub numbers: Numbers,",
        "    /// What marks the key side of a pair.",
        "    pub property: Property,",
        "    /// Whether a Capitalised word is a type.",
        "    pub capitalised_is_type: bool,",
        "    /// Whether a SCREAMING word is a constant.",
        "    pub screaming_is_constant: bool,",
        "    /// Whether a word before `(` is a function.",
        "    pub call: bool,",
        "}",
        "",
        "/// The languages the highlighter knows.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]",
        "pub enum Language {",
    ]
    for lang in declared:
        out += [f"    /// {lang['label']}.", f"    {of[lang['id']]},"]
    out += [
        "}",
        "",
        "impl Language {",
        "    /// Where it sits in [`GRAMMARS`].",
        "    ///",
        "    /// A `match` and not a search: the enum is generated FROM the table in",
        "    /// the table's own order, so this cannot drift, and the compiler folds it",
        "    /// to a load.",
        "    #[must_use]",
        "    pub const fn index(self) -> usize {",
        "        match self {",
    ]
    for n, lang in enumerate(declared):
        out.append(f"            Self::{of[lang['id']]} => {n},")
    out += ["        }", "    }", "}", ""]

    def slist(name: str, items) -> str:
        return "&[" + ", ".join(rust_str(i) for i in items) + "]"

    def delim(d) -> str:
        mx = "None" if d["max"] is None else f"Some({int(d['max'])})"
        return (
            "Delimiter { open: %s, close: %s, escape: %s, spans_lines: %s, hashes: %s, max: %s }"
            % (
                rust_str(d["open"]),
                rust_str(d["close"]),
                rust_str(d["escape"]),
                "true" if d["spans-lines"] else "false",
                "true" if d["hashes"] else "false",
                mx,
            )
        )

    out += [
        "/// One grammar per language, in the order `assets/syntax.nix` writes them.",
        "///",
        "/// That order is `guess`'s order, and it is the declaration's own — two",
        "/// sources for one order is one too many.",
        f"pub static GRAMMARS: [Grammar; {len(declared)}] = [",
    ]
    for lang in declared:
        # LONGEST OPENER FIRST, decided here and not in the scanner: `"""` has
        # to be tried before `"`, or a docstring opens and closes an empty
        # string twice and the rest of the file goes uncoloured.
        strings = sorted(lang["strings"], key=lambda d: -len(d["open"]))
        comments = sorted(lang["line-comment"], key=lambda c: -len(c))
        block = lang["block-comment"]
        block_rs = (
            "None"
            if block is None
            else "Some(Block { open: %s, close: %s, nests: %s })"
            % (rust_str(block["open"]), rust_str(block["close"]), "true" if block["nests"] else "false")
        )
        attr = lang["attribute"]
        attr_rs = (
            "None"
            if attr is None
            else "Some(Attribute { prefix: %s, close: %s })"
            % (
                rust_str(attr["prefix"]),
                "None" if attr["close"] is None else f"Some({rust_str(attr['close'])})",
            )
        )
        out += [
            "    Grammar {",
            f"        id: {rust_str(lang['id'])},",
            f"        label: {rust_str(lang['label'])},",
            f"        kind: Kind::{variant(lang['kind'])},",
            f"        line_comments: {slist('c', comments)},",
            f"        block_comment: {block_rs},",
            "        strings: &[" + ", ".join(delim(d) for d in strings) + "],",
            f"        interpolation: {slist('i', lang['interpolation'])},",
            f"        fences: {slist('f', lang['fences'])},",
            f"        attribute: {attr_rs},",
            f"        keywords: {slist('k', sorted(lang['keywords']))},",
            f"        types: {slist('t', sorted(lang['types']))},",
            f"        constants: {slist('n', sorted(lang['constants']))},",
            f"        numbers: Numbers::{variant(lang['numbers'])},",
            f"        property: Property::{variant(lang['property'])},",
            f"        capitalised_is_type: {'true' if 'capitalised-is-type' in lang['shapes'] else 'false'},",
            f"        screaming_is_constant: {'true' if 'screaming-is-constant' in lang['shapes'] else 'false'},",
            f"        call: {'true' if lang['call'] else 'false'},",
            "    },",
        ]
    out += ["];", ""]

    rows = sorted((m, of[i]) for m, (i, _) in resolved.items())
    out += [
        "/// Every MIME type that resolves to a language, SORTED for `binary_search`.",
        "///",
        "/// Small because it is expanded from claims: one claim on `application/xml`",
        "/// brings forty-nine descendants, and the alias table hands over the rest.",
        f"pub static OF_MIME: [(&str, Language); {len(rows)}] = [",
    ]
    out += [f"    ({rust_str(m)}, Language::{v})," for m, v in rows]
    out += ["];", ""]

    trows = sorted((t, of[i]) for t, i in tags.items())
    out += [
        "/// What a markdown fence may call a language, SORTED.",
        f"pub static OF_TAG: [(&str, Language); {len(trows)}] = [",
    ]
    out += [f"    ({rust_str(t)}, Language::{v})," for t, v in trows]
    out += ["];", ""]

    shebangs = sorted(
        (name, of[lang["id"]]) for lang in declared for name in lang["shebang"]
    )
    out += [
        "/// What a `#!` line may name, SORTED.",
        f"pub static OF_SHEBANG: [(&str, Language); {len(shebangs)}] = [",
    ]
    out += [f"    ({rust_str(n)}, Language::{v})," for n, v in shebangs]
    out += ["];", ""]

    guessable = [lang for lang in declared if lang["guessable"]]
    out += [
        "/// What to look for in text that arrived with no usable name.",
        "///",
        "/// IN DECLARATION ORDER, which is the order `assets/syntax.nix` writes: the",
        "/// first language whose markers all appear wins. A language that is mostly",
        "/// prose declares `guessable = false` and is not here at all, because",
        "/// colouring prose is worse than not colouring it.",
        f"pub static SNIFF: [(Language, &[&str], SniffShape); {len(guessable)}] = [",
    ]
    for lang in guessable:
        out.append(
            "    (Language::%s, %s, SniffShape::%s),"
            % (of[lang["id"]], slist("s", lang["sniff"]), variant(lang["sniff-shape"]))
        )
    out += ["];", ""]

    out += [
        "/// What came out, so a truncated generation is a COMPILE error.",
        "pub mod counts {",
        "    /// Languages declared.",
        f"    pub const LANGUAGES: usize = {len(declared)};",
        "    /// MIME types that resolve to one.",
        f"    pub const MIMES: usize = {len(rows)};",
        "    /// Fence tags.",
        f"    pub const TAGS: usize = {len(trows)};",
        "}",
    ]
    return out


if __name__ == "__main__":
    raise SystemExit(main())
