#!/usr/bin/env python3
"""`nix run .#texts -- missing <language>` and `-- where <key>`: the translator's help.

The sentences live in fragments — every crate that speaks owns
`crates/<crate>/i18n/<language>.ftl`, the browser extension `extension/i18n/`,
each brain module `crates/ronoctua-brain/modules/<id>/i18n/` — so a language's
work is spread over some sixty files. Read over the CHECKOUT this runs in (its
working tree, not a build), this says:

    missing <language>   every key the base language has and <language> lacks,
                         fragment by fragment, with the base sentence to
                         translate — and every key <language> has that the base
                         does not (an orphan the build would refuse too).
                         Exit 1 when anything is missing or orphaned, 0 when
                         the language is complete.
    where <key>          the fragment that holds a key, and its sentence in
                         every language. Exit 1 when no fragment does.

The build itself refuses an incomplete language (`nix/scripts/gen-i18n.py`); this
is the list of what to write before it stops refusing.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

KEY = re.compile(r"^([a-z][a-z0-9-]*)\s*=\s?(.*)$")


def root_of(start: Path) -> Path:
    """The checkout: the nearest directory up from `start` with the languages' list."""
    for d in (start, *start.parents):
        if (d / "assets/i18n/languages.nix").is_file() and (d / "Cargo.toml").is_file():
            return d
    print(f"ERROR: {start} is not inside a ronoctua-distro checkout (no assets/i18n/languages.nix above it)", file=sys.stderr)
    raise SystemExit(2)


def fragments(root: Path) -> list[Path]:
    """Every fragment folder of the checkout, in a stable order."""
    found = sorted(root.glob("crates/*/i18n")) + sorted(root.glob("crates/ronoctua-brain/modules/*/i18n"))
    if (root / "extension/i18n").is_dir():
        found.append(root / "extension/i18n")
    return [d for d in found if d.is_dir()]


def messages(path: Path) -> dict[str, str]:
    """Key -> its first line, in file order; an indented line continues the value."""
    out: dict[str, str] = {}
    current = None
    if not path.is_file():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        m = KEY.match(line)
        if m:
            current = m.group(1)
            out[current] = m.group(2).strip()
        elif current is not None and line[:1].isspace() and line.strip() and not out[current]:
            out[current] = line.strip()
    return out


def missing(root: Path, base: str, language: str) -> int:
    lacking = orphans = 0
    for folder in fragments(root):
        mine = messages(folder / f"{base}.ftl")
        theirs = messages(folder / f"{language}.ftl")
        rel = folder.relative_to(root)
        gone = [k for k in mine if k not in theirs]
        extra = [k for k in theirs if k not in mine]
        if not (folder / f"{language}.ftl").is_file():
            print(f"{rel}/{language}.ftl does not exist: all {len(mine)} keys are to be written")
        elif gone:
            print(f"{rel}/{language}.ftl lacks {len(gone)}:")
        for k in gone:
            print(f"  {k} = {mine[k]}")
        if extra:
            print(f"{rel}/{language}.ftl has {len(extra)} the base ({base}) does not — remove or add them to {base}.ftl:")
            for k in extra:
                print(f"  {k} = {theirs[k]}")
        lacking += len(gone)
        orphans += len(extra)
    if lacking or orphans:
        print(f"{language}: {lacking} keys to translate, {orphans} orphans")
        return 1
    print(f"{language}: complete, every key of {base} in {len(fragments(root))} fragments")
    return 0


def where(root: Path, languages: list[str], key: str) -> int:
    for folder in fragments(root):
        found = {l: messages(folder / f"{l}.ftl").get(key) for l in languages}
        if any(v is not None for v in found.values()):
            print(folder.relative_to(root))
            for l, v in found.items():
                print(f"  {l}: {v if v is not None else '(missing)'}")
            return 0
    print(f"no fragment holds `{key}`", file=sys.stderr)
    return 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--languages", required=True, help="the declared languages, comma-separated, the base first")
    sub = ap.add_subparsers(dest="what", required=True)
    m = sub.add_parser("missing", help="what a language lacks, fragment by fragment")
    m.add_argument("language")
    w = sub.add_parser("where", help="the fragment that holds a key")
    w.add_argument("key")
    args = ap.parse_args()
    root = root_of(Path.cwd())
    args.languages = args.languages.split(",")
    base = args.languages[0]
    if args.what == "missing":
        if args.language not in args.languages:
            print(f"ERROR: {args.language} is not a declared language ({', '.join(args.languages)})", file=sys.stderr)
            return 2
        if args.language == base:
            print(f"ERROR: {base} is the base: every other language is compared with it", file=sys.stderr)
            return 2
        return missing(root, base, args.language)
    return where(root, args.languages, args.key)


if __name__ == "__main__":
    raise SystemExit(main())
