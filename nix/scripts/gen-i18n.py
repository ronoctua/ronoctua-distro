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
    gen-i18n.py <ftl-dir> <output.rs> <language>... [--module <id>=<dir> ...]
                [--prefixes <id>=<prefix>,<prefix> ...]

# The brain's modules (brain.md §3)

A brain module carries its own texts, `modules/<id>/i18n/<lang>.ftl`, and
`--module` hands one such folder in. Its keys join the registry and the
bundle — this writes the MERGED `<lang>.ftl`, the base followed by every
fragment, beside `<output.rs>` — and three things are refused on the way,
because a key's owner has to be readable from the key:

- a module key that is also a base key, or another module's;
- a module key outside its namespace: `(cmd-|notif-|echo-|about-|ask-)?`
  followed by one of its prefixes (`--prefixes`, its id by default), alone or
  with more after a `-`, or `brain-task-<id>` and `brain-task-<id>-does`;
- two modules whose prefixes overlap (`made` and `made-up`): two owners of
  one name.

A BASE key inside a module's namespace is NOT refused here. The shell's own
screens may speak in a module's words by design — a page of the same name
beside the module — and whether a file outside the folder may name the module
is `brain-modules-are-sealed`'s question (step 5), where the module's
`namedBy` can answer it. This generator has no such list, so refusing here
would stop every build over a decision the module already wrote down.
"""

from __future__ import annotations

import argparse
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


# The words that open a key belonging to a command, a notification, the
# minibuffer, a hint and a question — each may come before a module's prefix.
OPENERS = ("cmd-", "notif-", "echo-", "about-", "ask-")
# Prefixes no module may own: an opener itself, and the brain's own texts.
RESERVED = {"cmd", "notif", "echo", "about", "ask", "brain"}


def in_namespace(key: str, ident: str, prefixes: list[str]) -> bool:
    """Whether a key is a module's to declare. See the header."""
    if key in (f"brain-task-{ident}", f"brain-task-{ident}-does"):
        return True
    for opener in ("",) + OPENERS:
        if not key.startswith(opener):
            continue
        rest = key[len(opener) :]
        if any(rest == p or rest.startswith(p + "-") for p in prefixes):
            return True
    return False


def pairs(values: list[str], flag: str) -> dict[str, str]:
    """`id=value` arguments, each id once."""
    out: dict[str, str] = {}
    for v in values:
        ident, sep, rest = v.partition("=")
        if not sep or not ident or not rest:
            print(f"ERROR: {flag} {v!r} is not `<id>=<value>`", file=sys.stderr)
            raise SystemExit(2)
        if ident in out:
            print(f"ERROR: {flag} names `{ident}` twice", file=sys.stderr)
            raise SystemExit(2)
        out[ident] = rest
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="The text key registry, from the .ftl files.")
    ap.add_argument("source", type=Path, help="the base .ftl directory")
    ap.add_argument("out", type=Path, help="the keys.rs to write; the merged .ftl go beside it")
    ap.add_argument("languages", nargs="+", help="the declared languages, the base first")
    ap.add_argument("--module", action="append", default=[], help="<id>=<dir>: a brain module's texts")
    ap.add_argument(
        "--prefixes", action="append", default=[], help="<id>=<p>,<p>: a module's key prefixes (default: its id)"
    )
    args = ap.parse_args()
    source, out_path = args.source, args.out
    # MANDATORY languages, and they come from `assets/i18n/languages.nix`
    # rather than from a constant here. Naming one forces its file to exist and
    # to be complete — which is the point, and which is why this is a declared
    # list and not a glob of the directory.
    languages = tuple(args.languages)
    modules = pairs(args.module, "--module")
    # THE ID IS ALWAYS A PREFIX: `cmd-<id>-<action>` and `<id>-why-<word>` are
    # built from it, whatever else a module claims.
    prefixes = {i: list(dict.fromkeys([i] + p.split(","))) for i, p in pairs(args.prefixes, "--prefixes").items()}
    for ident in prefixes:
        if ident not in modules:
            print(f"ERROR: --prefixes names `{ident}`, which no --module hands in", file=sys.stderr)
            return 2
    for ident in modules:
        prefixes.setdefault(ident, [ident])

    # NOT OVER THE BASE: the merged `.ftl` go beside `<output.rs>`, and there
    # they would replace the files they were read from. Refused before a byte
    # is written.
    if out_path.parent.resolve() == source.resolve():
        print("ERROR: the merged .ftl would overwrite the base; write keys.rs elsewhere", file=sys.stderr)
        return 1

    tables: dict[str, dict[str, str]] = {}
    texts: dict[str, str] = {}
    for language in languages:
        path_of = source / f"{language}.ftl"
        if not path_of.exists():
            print(f"ERROR: {path_of} is missing", file=sys.stderr)
            return 1
        tables[language] = keys(path_of)
        texts[language] = path_of.read_text(encoding="utf-8")

    failed = False

    # THE MODULES' FRAGMENTS, each checked on its own first, so a message
    # names the module's file and not the merged one nobody wrote.
    fragments: dict[str, dict[str, dict[str, str]]] = {}
    fragment_texts: dict[str, dict[str, str]] = {}
    for ident in sorted(modules):
        for p in prefixes[ident]:
            if p in RESERVED or p.startswith("brain-"):
                print(f"ERROR: module `{ident}` claims the prefix `{p}`, which is not a module's to own", file=sys.stderr)
                failed = True
        fragments[ident], fragment_texts[ident] = {}, {}
        for language in languages:
            path_of = Path(modules[ident]) / f"{language}.ftl"
            if not path_of.exists():
                print(f"ERROR: module `{ident}` has no {language}.ftl ({path_of})", file=sys.stderr)
                failed = True
                continue
            fragments[ident][language] = keys(path_of)
            fragment_texts[ident][language] = path_of.read_text(encoding="utf-8")
    if failed:
        return 1

    # PREFIXES THAT OVERLAP are two owners of one name: `made-up-x` would be
    # both `made`'s and `made-up`'s.
    owners = sorted((p, ident) for ident in modules for p in prefixes[ident])
    for i, (p, a) in enumerate(owners):
        for q, b in owners[i + 1 :]:
            if a != b and (p == q or q.startswith(p + "-") or p.startswith(q + "-")):
                print(f"ERROR: the prefixes `{p}` ({a}) and `{q}` ({b}) overlap", file=sys.stderr)
                failed = True

    base_language = languages[0]
    for ident in sorted(fragments):
        mine = fragments[ident].get(base_language, {})
        for language in languages[1:]:
            theirs = fragments[ident].get(language, {})
            for c in sorted(set(mine) - set(theirs)):
                print(f"ERROR: module `{ident}`: `{c}` exists in {base_language} and is missing from {language}", file=sys.stderr)
                failed = True
            for c in sorted(set(theirs) - set(mine)):
                print(f"ERROR: module `{ident}`: `{c}` exists in {language} and is missing from {base_language}", file=sys.stderr)
                failed = True
        for c in sorted(mine):
            if not in_namespace(c, ident, prefixes[ident]):
                print(
                    f"ERROR: module `{ident}`: `{c}` is outside its namespace "
                    f"({', '.join(prefixes[ident])}, with cmd-/notif-/echo-/about-/ask-, "
                    f"or brain-task-{ident})",
                    file=sys.stderr,
                )
                failed = True
            if c in tables[base_language]:
                print(f"ERROR: module `{ident}`: `{c}` is also a base key", file=sys.stderr)
                failed = True
            for other in sorted(fragments):
                if other < ident and c in fragments[other].get(base_language, {}):
                    print(f"ERROR: `{c}` is declared by both `{other}` and `{ident}`", file=sys.stderr)
                    failed = True
    if failed:
        return 1

    # ONE TABLE PER LANGUAGE from here on: every rule below holds for the
    # module keys exactly as for the base's.
    for ident in sorted(fragments):
        for language in languages:
            tables[language].update(fragments[ident][language])

    # The comparison runs BOTH ways. Only checking "every en-GB key
    # exists in pt-BR" would let an orphan translation through, which is dead
    # translation piling up in the file.
    base, *resto = languages
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

    # THE MERGED BUNDLE, beside the registry: `face()` embeds `<lang>.ftl`
    # from this directory, so the keys above and the texts the binary carries
    # come out of one run and cannot disagree. The base comes first, whole and
    # untouched — with no module the file IS the base, byte for byte.
    for language in languages:
        merged = texts[language]
        if merged and not merged.endswith("\n"):
            merged += "\n"
        for ident in sorted(fragment_texts):
            merged += f"\n# ── brain module: {ident} ──\n\n" + fragment_texts[ident][language]
            if not merged.endswith("\n"):
                merged += "\n"
        (out_path.parent / f"{language}.ftl").write_text(merged, encoding="utf-8")

    moved = sum(len(f[base]) for f in fragments.values())
    beside = f" ({moved} of them from the brain's modules: {', '.join(sorted(fragments))})" if fragments else ""
    print(f"OK: {len(every)} keys in {len(languages)} languages -> {out_path}{beside}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
