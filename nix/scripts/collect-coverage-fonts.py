#!/usr/bin/env python3
"""Collect the faces the declared fallback chain promises.

The chain in `nix/fontconfig.nix` names FAMILIES; the renderer loads FILES, by
absolute path, because section 6.4 forbids it from asking fontconfig anything.
This is what turns one into the other, and it is a build step rather than a
list of filenames for two reasons.

A filename is not a stable name for a family. `NotoSansHebrew.ttf` registers
as `Noto Sans Hebrew Light`, because it is a variable font whose default
instance is Light and whose `nameID 1` says so; the family a font database
files it under comes from `nameID 16` when that exists. That is exactly the
27-variant defect this repository already paid for once, in
`scripts/fix-font-names.py`, and the answer is the same one: read the metrics
and the name table, never the file name.

And a family with no file has to STOP the build. The chain is a promise that a
script will be drawn; a promise with nothing behind it does not fail loudly, it
draws tofu — which looks like a font problem, on a machine, months later.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import sys

from fontTools.ttLib import TTCollection, TTFont

# What a font file can be. `.ttc`/`.otc` are COLLECTIONS: one file, several
# faces, and the CJK set ships all five language variants that way.
SUFFIXES = (".ttf", ".otf", ".ttc", ".otc")

# The OS/2 `fsSelection` bit that says the face is italic.
ITALIC_BIT = 1 << 0

# The weight the interface asks for. A face further from this loses to one
# nearer it, which is what keeps `Noto Sans` from arriving as its Italic.
WANTED_WEIGHT = 400


def faces_in(path: pathlib.Path):
    """Every face in a file, with the file's own index for each."""
    if path.suffix.lower() in (".ttc", ".otc"):
        collection = TTCollection(str(path), lazy=True)
        return list(enumerate(collection.fonts))
    return [(0, TTFont(str(path), lazy=True))]


def family_of(font) -> str | None:
    """The name a font database files this face under.

    `TYPOGRAPHIC_FAMILY` (16) first and `FAMILY` (1) only when it is absent —
    the rule `fontdb::parse_names` follows, and the renderer's database IS
    fontdb. Guessing differently here would produce a manifest that resolves
    and a renderer that does not.
    """
    name = font["name"]
    return name.getDebugName(16) or name.getDebugName(1)


def how_wrong(font) -> tuple[int, int]:
    """How far this face is from the upright regular one, for sorting."""
    try:
        os2 = font["OS/2"]
        italic = 1 if (os2.fsSelection & ITALIC_BIT) else 0
        weight = int(os2.usWeightClass)
    except (KeyError, AttributeError):
        italic, weight = 0, WANTED_WEIGHT
    return (italic, abs(weight - WANTED_WEIGHT))


def index(directories: list[pathlib.Path]) -> dict[str, tuple[pathlib.Path, tuple]]:
    """family -> (file, how wrong), keeping the least wrong face of each."""
    found: dict[str, tuple[pathlib.Path, tuple]] = {}
    for directory in directories:
        for path in sorted(directory.rglob("*")):
            if not path.is_file() or path.suffix.lower() not in SUFFIXES:
                continue
            try:
                faces = faces_in(path)
            except Exception as bad:  # noqa: BLE001 - a font we cannot read is not fatal
                print(f"  skipping {path.name}: {bad}", file=sys.stderr)
                continue
            for _, font in faces:
                family = family_of(font)
                if not family:
                    continue
                wrong = how_wrong(font)
                if family not in found or wrong < found[family][1]:
                    found[family] = (path, wrong)
    return found


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--chain-json", required=True, help="the generated fallback chain")
    ap.add_argument("--from", dest="sources", action="append", required=True,
                    help="a directory to search; repeatable")
    ap.add_argument("--out", required=True, help="where the faces and the manifest go")
    args = ap.parse_args()

    chain = json.loads(pathlib.Path(args.chain_json).read_text(encoding="utf-8"))
    wanted: list[str] = chain["coverage"]

    directories = [pathlib.Path(s) for s in args.sources]
    print(f"searching {len(directories)} directories for {len(wanted)} families")
    found = index(directories)
    print(f"  {len(found)} families present in all")

    missing = [family for family in wanted if family not in found]
    if missing:
        print("", file=sys.stderr)
        print("ERROR: the declared chain names families that no file provides:", file=sys.stderr)
        for family in missing:
            print(f"  - {family}", file=sys.stderr)
        print("", file=sys.stderr)
        print("      A family in the chain is a promise that its script will be", file=sys.stderr)
        print("      DRAWN. Without a file the promise becomes tofu, which reads", file=sys.stderr)
        print("      on screen as a broken font and not as a missing package.", file=sys.stderr)
        print("      Either the package moved, or the family was renamed", file=sys.stderr)
        print("      upstream: fix `scripts`/`han`/`common` in", file=sys.stderr)
        print("      nix/fontconfig.nix, which is where the chain is declared.", file=sys.stderr)
        return 1

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    # One COPY per file, however many families it answers for: the CJK
    # collection is one file and five families, and copying it five times would
    # put thirty megabytes into the store five times AND load the same faces
    # five times into the database at start-up.
    manifest: dict[str, str] = {}
    copied: dict[pathlib.Path, str] = {}
    for family in wanted:
        source = found[family][0]
        if source not in copied:
            copied[source] = source.name
            shutil.copyfile(source, out / source.name)
        manifest[family] = copied[source]

    total = sum((out / name).stat().st_size for name in set(manifest.values()))
    (out / "manifest.json").write_text(
        json.dumps({"families": manifest}, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"  wrote {len(set(manifest.values()))} files, {total / 1048576:.1f} MB")
    print(f"OK: every one of the {len(wanted)} declared families has a face")
    return 0


if __name__ == "__main__":
    sys.exit(main())
