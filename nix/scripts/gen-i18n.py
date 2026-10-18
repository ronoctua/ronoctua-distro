#!/usr/bin/env python3
"""Checks the .ftl fragments and writes the texts the programs read — invariant 8.

The invariant says "zero string literals in UI; everything through a text-key, with pt-BR
and en-GB complete". The way to make that hold is not discipline: it is there being no
way to write a key that does not exist, and no way to ship a language that lacks one.

THE TEXTS ARE IN FRAGMENTS, one per crate that speaks (`crates/<crate>/i18n/<lang>.ftl`),
the browser extension's (`extension/i18n/`) and each brain module's. This generator reads
EVERY declared language's file of every fragment, requires each fragment to have exactly
the same set of keys in every language, no key to be in two fragments, the same arguments
in each value and no empty value, and writes one MERGED `<language>.ftl` per language into
`<out-dir>` — the `texts` derivation (`nix/i18n.nix`), which every program reads at run
time through `RONOCTUA_TEXTS`. A key present in only one language becomes a BUILD error
here, before any test runs.

THE KEYS ARE NOT WRITTEN HERE. Each crate's Rust constants (its `key` module) are written
at Nix EVALUATION by `nix/text-keys.nix`, from the key NAMES of its own fragment alone, so
that editing a sentence rebuilds no crate. That reader and this one look for a key with
the same pattern (`KEY` below).

It is not a complete Fluent parser, and it should not be: `fluent-bundle` does
that at runtime. Here we only need the SET of keys, which in Fluent are
the column-0 lines with `=`. Continuations (indented lines), comments and
blank lines are ignored.

Usage:
    gen-i18n.py <out-dir> <language>... [--fragment <name>=<dir> ...]
                [--module <id>=<dir> ...] [--prefixes <id>=<prefix>,<prefix> ...]

# The brain's modules (brain.md §3)

A brain module carries its own texts, `modules/<id>/i18n/<lang>.ftl`, and
`--module` hands one such folder in. Its keys join the bundle — each merged
`<lang>.ftl` is every fragment, then every module's — and three things are
refused on the way, because a key's owner has to be readable from the key:

- a module key that is also a crate's key, or another module's;
- a module key outside its namespace: `(cmd-|notif-|echo-|about-|ask-)?`
  followed by one of its prefixes (`--prefixes`, its id by default), alone or
  with more after a `-`, or `brain-task-<id>` and `brain-task-<id>-does`;
- two modules whose prefixes overlap (`made` and `made-up`): two owners of
  one name.

A CRATE's key inside a module's namespace is NOT refused here. The shell's own
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


def read_fragment(what: str, folder: Path, languages: tuple[str, ...]) -> tuple[dict, dict] | None:
    """A fragment's table and text by language, or `None` (said) when a language's file is missing."""
    tables, texts = {}, {}
    for language in languages:
        path_of = folder / f"{language}.ftl"
        if not path_of.exists():
            print(f"ERROR: {what} has no {language}.ftl ({path_of})", file=sys.stderr)
            return None
        tables[language] = keys(path_of)
        texts[language] = path_of.read_text(encoding="utf-8")
    return tables, texts


def main() -> int:
    ap = argparse.ArgumentParser(description="The texts the programs read, checked and merged from the .ftl fragments.")
    ap.add_argument("out", type=Path, help="the directory to write the merged <language>.ftl into")
    ap.add_argument("languages", nargs="+", help="the declared languages, the base first")
    ap.add_argument("--fragment", action="append", default=[], help="<name>=<dir>: a crate's (or the extension's) texts")
    ap.add_argument("--module", action="append", default=[], help="<id>=<dir>: a brain module's texts")
    ap.add_argument(
        "--prefixes", action="append", default=[], help="<id>=<p>,<p>: a module's key prefixes (default: its id)"
    )
    args = ap.parse_args()
    out_dir = args.out
    # MANDATORY languages, and they come from `assets/i18n/languages.nix`
    # rather than from a constant here. Naming one forces every fragment's
    # file of it to exist and to be complete — which is the point, and which
    # is why this is a declared list and not a glob of a directory.
    languages = tuple(args.languages)
    crates = pairs(args.fragment, "--fragment")
    modules = pairs(args.module, "--module")
    if not crates:
        print("ERROR: no --fragment: there would be no texts", file=sys.stderr)
        return 2
    # THE ID IS ALWAYS A PREFIX: `cmd-<id>-<action>` and `<id>-why-<word>` are
    # built from it, whatever else a module claims.
    prefixes = {i: list(dict.fromkeys([i] + p.split(","))) for i, p in pairs(args.prefixes, "--prefixes").items()}
    for ident in prefixes:
        if ident not in modules:
            print(f"ERROR: --prefixes names `{ident}`, which no --module hands in", file=sys.stderr)
            return 2
    for ident in modules:
        prefixes.setdefault(ident, [ident])

    # NOT OVER A FRAGMENT: the merged `.ftl` go into `<out-dir>`, and there
    # they would replace a file they were read from. Refused before a byte is
    # written.
    for folder in [*crates.values(), *modules.values()]:
        if out_dir.resolve() == Path(folder).resolve():
            print(f"ERROR: the merged .ftl would overwrite the fragment in {folder}; write them elsewhere", file=sys.stderr)
            return 1

    failed = False
    base_language = languages[0]

    # EACH FRAGMENT ON ITS OWN FIRST, so a message names the file somebody
    # wrote and not the merged one nobody did.
    crate_tables: dict[str, dict[str, dict[str, str]]] = {}
    crate_texts: dict[str, dict[str, str]] = {}
    for name in sorted(crates):
        read = read_fragment(f"`{name}`", Path(crates[name]), languages)
        if read is None:
            failed = True
            continue
        crate_tables[name], crate_texts[name] = read
    fragments: dict[str, dict[str, dict[str, str]]] = {}
    fragment_texts: dict[str, dict[str, str]] = {}
    for ident in sorted(modules):
        for p in prefixes[ident]:
            if p in RESERVED or p.startswith("brain-"):
                print(f"ERROR: module `{ident}` claims the prefix `{p}`, which is not a module's to own", file=sys.stderr)
                failed = True
        read = read_fragment(f"module `{ident}`", Path(modules[ident]), languages)
        if read is None:
            failed = True
            continue
        fragments[ident], fragment_texts[ident] = read
    if failed:
        return 1

    # EVERY LANGUAGE OF A FRAGMENT HAS ITS KEYS AND NO OTHER — both ways: only
    # checking "every en-GB key exists in pt-BR" would let an orphan
    # translation through, which is dead translation piling up in the file.
    def parity(what: str, tables: dict[str, dict[str, str]]) -> bool:
        bad = False
        mine = tables[base_language]
        for language in languages[1:]:
            theirs = tables[language]
            for c in sorted(set(mine) - set(theirs)):
                print(f"ERROR: {what}: `{c}` exists in {base_language} and is missing from {language}", file=sys.stderr)
                bad = True
            for c in sorted(set(theirs) - set(mine)):
                print(f"ERROR: {what}: `{c}` exists in {language} and is missing from {base_language}", file=sys.stderr)
                bad = True
        return bad

    for name in sorted(crate_tables):
        failed |= parity(f"`{name}`", crate_tables[name])
    for ident in sorted(fragments):
        failed |= parity(f"module `{ident}`", fragments[ident])

    # ONE OWNER PER KEY: a key in two fragments is two sentences for one
    # name, and which one a program says would be an accident of the order.
    owner: dict[str, str] = {}
    for name in sorted(crate_tables):
        for c in crate_tables[name][base_language]:
            if c in owner:
                print(f"ERROR: `{c}` is declared by both `{owner[c]}` and `{name}`", file=sys.stderr)
                failed = True
            else:
                owner[c] = name

    # PREFIXES THAT OVERLAP are two owners of one name: `made-up-x` would be
    # both `made`'s and `made-up`'s.
    owners = sorted((p, ident) for ident in modules for p in prefixes[ident])
    for i, (p, a) in enumerate(owners):
        for q, b in owners[i + 1 :]:
            if a != b and (p == q or q.startswith(p + "-") or p.startswith(q + "-")):
                print(f"ERROR: the prefixes `{p}` ({a}) and `{q}` ({b}) overlap", file=sys.stderr)
                failed = True

    for ident in sorted(fragments):
        mine = fragments[ident].get(base_language, {})
        for c in sorted(mine):
            if not in_namespace(c, ident, prefixes[ident]):
                print(
                    f"ERROR: module `{ident}`: `{c}` is outside its namespace "
                    f"({', '.join(prefixes[ident])}, with cmd-/notif-/echo-/about-/ask-, "
                    f"or brain-task-{ident})",
                    file=sys.stderr,
                )
                failed = True
            if c in owner:
                print(f"ERROR: module `{ident}`: `{c}` is also a key of `{owner[c]}`", file=sys.stderr)
                failed = True
            for other in sorted(fragments):
                if other < ident and c in fragments[other].get(base_language, {}):
                    print(f"ERROR: `{c}` is declared by both `{other}` and `{ident}`", file=sys.stderr)
                    failed = True
    if failed:
        return 1

    # ONE TABLE PER LANGUAGE from here on: every rule below holds for every
    # fragment's keys alike.
    tables: dict[str, dict[str, str]] = {language: {} for language in languages}
    for source in (crate_tables, fragments):
        for name in sorted(source):
            for language in languages:
                tables[language].update(source[name][language])

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

    base, *resto = languages
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
    out_dir.mkdir(parents=True, exist_ok=True)

    # THE MERGED BUNDLE: what a program reads at run time, one file per
    # language (`RONOCTUA_TEXTS`) — every fragment by its name, then every
    # module's, each under a line that says whose it is, its messages as
    # written and its COMMENTS LEFT OUT: they are the translators', in the
    # fragments, and every program would parse them at every start (a sixth
    # of the bytes).
    def messages_of(text: str) -> str:
        return "".join(line for line in text.splitlines(keepends=True) if not line.startswith("#"))

    for language in languages:
        merged = ""
        for title, texts in [
            *((f"{name}", crate_texts[name][language]) for name in sorted(crate_texts)),
            *((f"brain module: {ident}", fragment_texts[ident][language]) for ident in sorted(fragment_texts)),
        ]:
            if merged:
                merged += "\n"
            merged += f"# ── {title} ──\n" + messages_of(texts)
            if not merged.endswith("\n"):
                merged += "\n"
        (out_dir / f"{language}.ftl").write_text(merged, encoding="utf-8")

    moved = sum(len(f[base]) for f in fragments.values())
    beside = f" ({moved} of them from the brain's modules: {', '.join(sorted(fragments))})" if fragments else ""
    print(f"OK: {len(every)} keys of {len(crate_tables)} fragments in {len(languages)} languages -> {out_dir}{beside}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
