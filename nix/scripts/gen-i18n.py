#!/usr/bin/env python3
"""Generates the text key registry from the .ftl files — invariant 8.

The invariant says "zero string literals in UI; everything through a text-key, with pt-BR
and en-GB complete". The way to make that hold is not discipline: it is there being no
way to write a key that does not exist.

This generator reads BOTH files, requires them to have exactly the same
set of keys, and emits one Rust constant per key. A wrong key
becomes a COMPILE error in the crate that uses it, and a key present in only one
language becomes a BUILD error here — before any test runs.

It is not a complete Fluent parser, and it should not be: `fluent-bundle` does
that at runtime. Here we only need the SET of keys, which in Fluent are
the column-0 lines with `=`. Continuations (indented lines), comments and
blank lines are ignored.

Usage:
    gen-i18n.py <ftl-dir> <output.rs> <language>...
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# A Fluent message key in column 0. Attributes (`.attr = …`) and terms
# (`-term = …`) are left out on purpose: they are not addressable keys.
KEY = re.compile(r"^([a-z][a-z0-9-]*)\s*=")


def keys(file: Path) -> dict[str, str]:
    """Key -> value, in file order.

    A value may span several lines. Fluent puts a SELECTOR — a plural, or a
    sentence that changes with a state — in an indented block under the key,
    and that is where those choices belong: they are decisions of the LANGUAGE,
    not of the Rust.

    Until this function read them, an indented line was skipped and a message
    whose whole value lived in the block came out EMPTY. That tripped the
    empty-value guard below, which was right to fire and pointed at the wrong
    thing — so no `.ftl` in this repository could carry a plural.

    Attributes (`.label = ...`) are indented too and are still left out: they
    are not addressable keys, and folding them into the parent's value would
    make the guard read an attribute as if it were the message.
    """
    out: dict[str, str] = {}
    current: str | None = None
    for n, line in enumerate(file.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        if line[0].isspace():
            body = line.strip()
            # An attribute, or a blank continuation: neither is the value.
            if current is None or not body or body.startswith("."):
                continue
            out[current] = f"{out[current]} {body}".strip()
            continue
        m = KEY.match(line)
        if not m:
            print(f"{file}:{n}: unrecognised line: {line!r}", file=sys.stderr)
            raise SystemExit(1)
        key = m.group(1)
        if key in out:
            print(f"{file}:{n}: repeated key `{key}`", file=sys.stderr)
            raise SystemExit(1)
        out[key] = line.split("=", 1)[1].strip()
        current = key
    return out


def const_de(key: str) -> str:
    """`cmd-file-open` -> `CMD_FILE_OPEN`."""
    return key.upper().replace("-", "_")


def main() -> int:
    if len(sys.argv) < 4:
        print(__doc__, file=sys.stderr)
        return 2
    source, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    # MANDATORY languages, and they come from `assets/i18n/languages.nix`
    # rather than from a constant here. Naming one forces its file to exist and
    # to be complete — which is the point, and which is why this is a declared
    # list and not a glob of the directory.
    languages = tuple(sys.argv[3:])

    tables: dict[str, dict[str, str]] = {}
    for language in languages:
        path_of = source / f"{language}.ftl"
        if not path_of.exists():
            print(f"ERROR: {path_of} is missing", file=sys.stderr)
            return 1
        tables[language] = keys(path_of)

    # The comparison runs BOTH ways. Only checking "every en-GB key
    # exists in pt-BR" would let an orphan translation through, which is dead
    # translation piling up in the file.
    base, *resto = languages
    failed = False
    for language in resto:
        missing = sorted(set(tables[base]) - set(tables[language]))
        extra = sorted(set(tables[language]) - set(tables[base]))
        for c in missing:
            print(f"ERROR: `{c}` exists in {base} and is missing from {language}", file=sys.stderr)
            failed = True
        for c in extra:
            print(f"ERROR: `{c}` exists in {language} and is missing from {base}", file=sys.stderr)
            failed = True

    # AND THE ARGUMENTS EACH VALUE TAKES ARE THE SAME IN EVERY LANGUAGE.
    #
    # Key parity is not value parity. `display-scale = Scale { $factor }` in
    # en-GB beside `display-scale = Escala` in pt-BR passes every check above:
    # the key is in both files and neither value is empty. What ships is one
    # language silently dropping the number — the row reads "Escala" with
    # nothing after it — or, the other way round, a value asking for an
    # argument no caller passes, which Fluent renders as the literal
    # `{$factor}` on screen.
    #
    # Nothing else in the tree looks at Fluent placeholders: the only other
    # `{ $` in any check is `labels-of-a-view-are-distinct` using it to EXCLUDE
    # keys. So this is the one place it can be caught.
    def arguments_of(value):
        return set(re.findall(r"\{\s*\$([A-Za-z][A-Za-z0-9_-]*)\s*\}", value))

    for language in resto:
        for c, v in tables[base].items():
            other = tables[language].get(c)
            if other is None:
                continue
            mine, theirs = arguments_of(v), arguments_of(other)
            if mine != theirs:
                print(
                    f"ERROR: `{c}` takes {sorted(mine) or 'nothing'} in {base} "
                    f"and {sorted(theirs) or 'nothing'} in {language}",
                    file=sys.stderr,
                )
                failed = True

    # An empty value is worse than a missing key: it passes every set
    # check and shows up as blank space on screen.
    for language, table in tables.items():
        for c, v in table.items():
            if not v:
                print(f"ERROR: `{c}` is empty in {language}", file=sys.stderr)
                failed = True

    if failed:
        return 1

    every = sorted(tables[base])
    lines = [
        "// GENERATED by nix/scripts/gen-i18n.py. Do not edit: it vanishes at the next build.",
        "//",
        "// One constant per text key. Referencing a key that does not exist is a",
        "// compile error — that is how invariant 8 stops depending on",
        "// discipline (the same shape as the tokens in `ronoctua-tokens`).",
        "",
        "/// Known text keys. One per message in the `.ftl` files.",
        "pub mod key {",
    ]
    for c in every:
        example = tables[base][c].replace("\\", "\\\\").replace('"', '\\"')
        # A selector collapses onto one line and would run for hundreds of
        # columns. The doc comment is a reminder of which message this is, not
        # a copy of it.
        if len(example) > 110:
            example = example[:107] + "..."
        lines.append(f"    /// `{base}`: {example}")
        lines.append(f'    pub const {const_de(c)}: &str = "{c}";')
    lines.append("}")
    lines.append("")
    lines.append("/// Every existing key, in order. Used by the checks and by `M-x`.")
    # `static` AND NOT `const`, and the reason is how big this has grown: a
    # `const` array is INLINED at every use site, so a thousand-odd keys become
    # a thousand-odd copies of the table in the binary. Clippy started refusing
    # it when the list crossed its threshold, and it is right — a `static` is
    # one copy, and every reader here takes it by reference.
    lines.append(f"pub static ALL_KEYS: [&str; {len(every)}] = [")
    for c in every:
        lines.append(f'    "{c}",')
    lines.append("];")
    lines.append("")
    lines.append("/// Mandatory languages. An incomplete `.ftl` fails the build.")
    lines.append("///")
    lines.append("/// The FIRST is the base every other is compared against.")
    lines.append(f"pub const LANGUAGES: [&str; {len(languages)}] = [")
    for i in languages:
        lines.append(f'    "{i}",')
    lines.append("];")
    lines.append("")
    # THE BUNDLE LOADER IS GENERATED, and it has to be: `include_str!` takes a
    # literal, so it cannot be put in a loop. Written by hand this was the one
    # place a declared language could still be missing its body — the name
    # would be in `LANGUAGES`, the file would be on disk, and `Texts::fresh`
    # would answer `None` for it, falling back silently.
    lines.append("/// Each language's `.ftl`, embedded in the binary.")
    lines.append("///")
    lines.append("/// Embedded and not read from disk: the installer runs before any")
    lines.append("/// filesystem is mounted (section 7.1), and a shell with no text is")
    lines.append("/// useless. Same reason as the face embedded in the renderer.")
    lines.append("#[must_use]")
    lines.append("pub fn face(language: &str) -> Option<&'static str> {")
    lines.append("    match language {")
    for i in languages:
        lines.append(f'        "{i}" => Some(include_str!(concat!(')
        lines.append('            env!("RONOCTUA_I18N_DIR"),')
        lines.append(f'            "/{i}.ftl"')
        lines.append("        ))),")
    lines.append("        _ => None,")
    lines.append("    }")
    lines.append("}")
    lines.append("")

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"OK: {len(every)} keys in {len(languages)} languages -> {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
