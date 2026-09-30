#!/usr/bin/env python3
"""The player's channels by theme, from Nix to Rust.

`assets/channels.nix` becomes three things: the table
`ronoctua_broadcast::table` compiles, the text keys both languages have to
carry, and a refusal.

# What is refused, and why here

AN IPTV CATEGORY IPTV-ORG DOES NOT HAVE. The index is filtered by these ids
at run time, and a typo (`movie` for `movies`) would be a category whose TV
row is always empty — which reads as "nothing is on", not as a mistake. The
list it is checked against is a pinned capture of iptv-org's own
`categories.json` (see `nix/broadcast.nix`).

AN EXCLUDED CATEGORY USED AS A SOURCE: `excluded` is what never reaches the
index, and a category asking for it would ask for nothing, forever.

A CATEGORY WITH NO SOURCE, which would be a row that can never show anything.

AND AN ID THAT IS NOT KEBAB-CASE, OR TWICE: it becomes the text key
`player-channels-<id>`, and a Rust `match` arm somebody reads in a log.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ID = re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$")
# An archive.org collection identifier: letters, digits, `_`, `-` and `.`.
COLLECTION = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,99}$")
# A subject is words, and nothing Lucene would read as syntax.
SUBJECT = re.compile(r"^[a-z0-9][a-z0-9 '-]{0,59}$")
# Words for a search: printable, no line break, not too long for a query.
WORDS = re.compile(r"^[^\x00-\x1f\x7f]{1,100}$")

NUMBERS = ["stationsMost", "archiveRows", "livesMost", "recentMost", "favouritesMost"]
FRESH = ["radio", "iptv", "archive", "lives"]


def rust_str(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def rust_list(items: list[str]) -> str:
    return "&[" + ", ".join(rust_str(i) for i in items) + "]"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--channels", required=True)
    ap.add_argument("--iptv-categories", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--out-keys", required=True)
    args = ap.parse_args()

    declared = json.loads(pathlib.Path(args.channels).read_text(encoding="utf-8"))
    upstream = json.loads(pathlib.Path(args.iptv_categories).read_text(encoding="utf-8"))
    known = {c.get("id") for c in upstream if isinstance(c, dict)}
    known.discard(None)
    # A CAPTURE THAT CAME BACK AS SOMETHING ELSE — an error page, an empty
    # list — would refuse every category for the wrong reason.
    if len(known) < 10:
        print(
            f"ERROR: the pinned iptv-org categories.json names {len(known)} ids; its shape changed.",
            file=sys.stderr,
        )
        return 1

    gaps: list[str] = []

    excluded = declared.get("excluded", [])
    for e in excluded:
        if e not in known:
            gaps.append(f"excluded: `{e}` is not a category iptv-org has")

    for name in NUMBERS:
        v = declared.get(name)
        if not isinstance(v, int) or isinstance(v, bool) or v < 1:
            gaps.append(f"{name}: `{v}` is not a positive whole number")
    fresh = declared.get("fresh", {})
    for name in FRESH:
        v = fresh.get(name)
        if not isinstance(v, int) or isinstance(v, bool) or v < 1:
            gaps.append(f"fresh.{name}: `{v}` is not a positive whole number of minutes")
    unknown_fresh = sorted(set(fresh) - set(FRESH))
    if unknown_fresh:
        gaps.append(f"fresh: {unknown_fresh} are not among {FRESH}")

    categories = declared.get("categories", [])
    if not categories:
        gaps.append("categories: there are none")
    seen: list[str] = []
    for n, cat in enumerate(categories):
        cid = cat.get("id", "")
        where = f"categories[{n}] (`{cid}`)"
        if not ID.match(str(cid)):
            gaps.append(f"{where}: the id is not kebab-case")
        seen.append(cid)
        extra = sorted(set(cat) - {"id", "iptv", "archive", "subjects", "lives"})
        if extra:
            gaps.append(f"{where}: {extra} are not fields a category has")
        iptv = cat.get("iptv", [])
        archive = cat.get("archive", [])
        subjects = cat.get("subjects", [])
        lives = cat.get("lives", [])
        for i in iptv:
            if i not in known:
                gaps.append(
                    f"{where}: `{i}` is not an iptv-org category.\n"
                    f"        Its TV row would always be empty. Known: {sorted(known)}"
                )
            if i in excluded:
                gaps.append(f"{where}: `{i}` is excluded, so it can never find a channel")
        for c in archive:
            if not COLLECTION.match(str(c)):
                gaps.append(f"{where}: `{c}` is not shaped like an archive.org collection")
        for s in subjects:
            if not SUBJECT.match(str(s)):
                gaps.append(f"{where}: subject `{s}` is not plain lowercase words")
        if subjects and not archive:
            gaps.append(f"{where}: subjects narrow the archive's collections, and it names none")
        for w in lives:
            if not WORDS.match(str(w)) or not str(w).strip():
                gaps.append(f"{where}: live search `{w}` is empty, too long or has a control character")
        for field, items in (("iptv", iptv), ("archive", archive), ("subjects", subjects), ("lives", lives)):
            if len(set(items)) != len(items):
                gaps.append(f"{where}: `{field}` names something twice")
        if not (iptv or archive or lives):
            gaps.append(f"{where}: no source at all — a row that can never show anything")
    if len(set(seen)) != len(seen):
        gaps.append(f"the same category id is declared twice: {seen}")

    if gaps:
        print("ERROR: assets/channels.nix does not hold up.", file=sys.stderr)
        for gap in gaps:
            print(f"  - {gap}", file=sys.stderr)
        return 1

    lines: list[str] = []
    w = lines.append
    w("// GENERATED by nix/scripts/gen-broadcast.py from assets/channels.nix. Do not edit.")
    w("")
    w("/// Every category of \"channels by theme\", in the order the page lists them.")
    w("pub const CATEGORIES: &[Category] = &[")
    for cat in categories:
        w("    Category {")
        w(f"        id: {rust_str(cat['id'])},")
        w(f"        key: {rust_str('player-channels-' + cat['id'])},")
        w(f"        iptv: {rust_list(cat.get('iptv', []))},")
        w(f"        archive: {rust_list(cat.get('archive', []))},")
        w(f"        subjects: {rust_list(cat.get('subjects', []))},")
        w(f"        lives: {rust_list(cat.get('lives', []))},")
        w("    },")
    w("];")
    w("")
    w("/// iptv-org categories that never reach the index.")
    w(f"pub const EXCLUDED: &[&str] = {rust_list(excluded)};")
    w("")
    for name in FRESH:
        w(f"/// How long the {name} answer is shown without asking again, in minutes.")
        w(f"pub const FRESH_{name.upper()}_MINUTES: u64 = {int(fresh[name])};")
    w("")
    w("/// How many stations one radio list asks for.")
    w(f"pub const STATIONS_MOST: u32 = {int(declared['stationsMost'])};")
    w("/// How many rows one page of the archive's search holds.")
    w(f"pub const ARCHIVE_ROWS: u32 = {int(declared['archiveRows'])};")
    w("/// How many lives one category asks for.")
    w(f"pub const LIVES_MOST: usize = {int(declared['livesMost'])};")
    w("/// How many recently played streams are kept.")
    w(f"pub const RECENT_MOST: usize = {int(declared['recentMost'])};")
    w("/// How many favourites a person can keep.")
    w(f"pub const FAVOURITES_MOST: usize = {int(declared['favouritesMost'])};")
    w("")

    pathlib.Path(args.out).write_text("\n".join(lines), encoding="utf-8")
    keys = sorted("player-channels-" + c["id"] for c in categories)
    pathlib.Path(args.out_keys).write_text("\n".join(keys) + "\n", encoding="utf-8")
    print(f"OK: {len(categories)} categories, {len(keys)} text keys, {len(known)} iptv-org categories known")
    return 0


if __name__ == "__main__":
    sys.exit(main())
