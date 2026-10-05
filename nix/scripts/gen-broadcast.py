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

# And the catalogue's

A THEME NOTHING REACHES: a theme with no desk source (`iptv`, `archive`,
`lives`) must be named by one of the default sources' categories, or it is a
row that never shows anything — the same refusal as a category with no source.

A DEFAULT SOURCE'S CATEGORY UNDER A THEME THAT DOES NOT EXIST, a host that is
not a host's name, a kind the crate does not know, a PeerTube category
PeerTube does not document, a Commons category name the API would read as
syntax.

AN ALIAS TWO THEMES CLAIM, or an adult word written twice: a person's group
named with it would land in whichever theme the table lists first, and
nothing on screen could say why.
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
FRESH = ["radio", "iptv", "archive", "lives", "nasa", "commons", "peertube",
         "playlist", "feed", "youtube", "collection"]
# The catalogue's bounds, and the Rust constant and type each becomes.
CATALOGUE = {
    "sourceItemsMost": ("SOURCE_ITEMS_MOST", "usize"),
    "listMib": ("LIST_MIB", "u64"),
    "feedMib": ("FEED_MIB", "u64"),
    "nativeItemsMost": ("NATIVE_ITEMS_MOST", "usize"),
    "pageRows": ("PAGE_ROWS", "u32"),
    "youtubePlaylistsMost": ("YOUTUBE_PLAYLISTS_MOST", "usize"),
    "youtubeListMost": ("YOUTUBE_LIST_MOST", "usize"),
}
# The kinds of default source, and the `sources::Kind` variant each is. The
# fresh table names each kind's freshness: `iptv-org` is iptv's, and so on.
KINDS = {
    "iptv-org": ("IptvOrg", "iptv"),
    "archive": ("Archive", "archive"),
    "youtube-lives": ("Lives", "lives"),
    "nasa": ("Nasa", "nasa"),
    "commons": ("Commons", "commons"),
    "peertube": ("PeerTube", "peertube"),
}
# What each kind may declare beside its id, kind and hosts.
KIND_FIELDS = {
    "iptv-org": set(),
    "archive": {"collections"},
    "youtube-lives": set(),
    "nasa": {"searches"},
    "commons": {"categories"},
    "peertube": {"categories"},
}
# PeerTube's own category numbers (its API documents 1 to 18).
PEERTUBE_CATEGORIES = range(1, 19)
# A host's name: labels of letters, digits and `-`, dotted.
HOST = re.compile(r"^(?=.{1,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$")
# A Commons category's name: printable, none of the characters a title cannot
# hold (`#<>[]|{}`), no `:` (a namespace), no leading or trailing space.
COMMONS = re.compile(r"^[^\x00-\x1f\x7f#<>\[\]|{}:_ ][^\x00-\x1f\x7f#<>\[\]|{}:_]{0,198}[^\x00-\x1f\x7f#<>\[\]|{}:_ ]$")
# An alias or an adult word: lowercase words (any script), digits, spaces and
# the few marks names carry.
ALIAS = re.compile(r"^[^\x00-\x1f\x7fA-Z]{1,60}$")


def rust_str(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def rust_list(items: list[str]) -> str:
    return "&[" + ", ".join(rust_str(i) for i in items) + "]"


def folded_words(text: str) -> list[str]:
    """The words the crate matches an alias by: lower case, split at every
    character that is neither a letter, a digit nor `+`. (The crate also takes
    the accents off; the generator only needs to see two aliases collide, so
    it compares them the same way Python can without a table.)"""
    return [w for w in re.split(r"[^\w+]+", text.lower()) if w and w != "_"]


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
        # A KEY THAT IS ALREADY ANOTHER'S: `player-channels-count` is the
        # page's count, and `player-channels-source-…` a source's name.
        if cid == "count" or str(cid).startswith("source"):
            gaps.append(f"{where}: `player-channels-{cid}` is a key of something else")
        seen.append(cid)
        extra = sorted(set(cat) - {"id", "iptv", "archive", "subjects", "lives", "aliases"})
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
    if len(set(seen)) != len(seen):
        gaps.append(f"the same category id is declared twice: {seen}")

    # THE ALIASES: words a person's own group is matched by. Two themes that
    # claim one would send the group to whichever is listed first.
    claimed: dict[tuple[str, ...], str] = {}
    for cat in categories:
        cid = cat.get("id", "")
        for alias in cat.get("aliases", []):
            if not ALIAS.match(str(alias)) or not folded_words(str(alias)):
                gaps.append(f"`{cid}`: alias `{alias}` is not lowercase words")
                continue
            words = tuple(folded_words(alias))
            if words in claimed and claimed[words] != cid:
                gaps.append(f"alias `{alias}` is claimed by `{claimed[words]}` and `{cid}`")
            claimed[words] = cid

    adult = declared.get("adultWords", [])
    if not adult:
        gaps.append("adultWords: there are none, and a person's adult group would be shown to anyone")
    for word in adult:
        if not ALIAS.match(str(word)) or len(folded_words(str(word))) != 1:
            gaps.append(f"adultWords: `{word}` is not one lowercase word")
    if len(set(adult)) != len(adult):
        gaps.append("adultWords: a word is written twice")

    catalogue = declared.get("catalogue", {})
    for name in CATALOGUE:
        v = catalogue.get(name)
        if not isinstance(v, int) or isinstance(v, bool) or v < 1:
            gaps.append(f"catalogue.{name}: `{v}` is not a positive whole number")
    unknown_bounds = sorted(set(catalogue) - set(CATALOGUE))
    if unknown_bounds:
        gaps.append(f"catalogue: {unknown_bounds} are not among {sorted(CATALOGUE)}")

    # THE DEFAULT SOURCES, and the categories each asks for.
    theme_ids = {cat.get("id") for cat in categories}
    sources = declared.get("sources", [])
    natives_of: dict[str, list[dict]] = {}
    named_themes: set[str] = set()
    source_ids: list[str] = []
    kinds_seen: list[str] = []
    for n, src in enumerate(sources):
        sid = src.get("id", "")
        kind = src.get("kind", "")
        where = f"sources[{n}] (`{sid}`)"
        source_ids.append(sid)
        kinds_seen.append(kind)
        if not ID.match(str(sid)):
            gaps.append(f"{where}: the id is not kebab-case")
        if kind not in KINDS:
            gaps.append(f"{where}: `{kind}` is not one of {sorted(KINDS)}")
            continue
        extra = sorted(set(src) - {"id", "kind", "hosts", "posterHosts"} - KIND_FIELDS[kind])
        if extra:
            gaps.append(f"{where}: {extra} are not fields a `{kind}` source has")
        hosts = src.get("hosts", [])
        if not hosts:
            gaps.append(f"{where}: no host — its consent row would let nothing")
        for h in list(hosts) + list(src.get("posterHosts", [])):
            if not HOST.match(str(h)):
                gaps.append(f"{where}: `{h}` is not a host's name")

        natives: list[dict] = []
        def native(key: str, theme: str, ask: str, narrow: list[str], audio: bool) -> None:
            if theme not in theme_ids:
                gaps.append(f"{where}: `{key}` is under theme `{theme}`, which is not declared")
            named_themes.add(theme)
            natives.append({"key": key, "theme": theme, "ask": ask, "narrow": narrow, "audio": audio})

        # THE DESKS' SOURCES, read from the themes: the catalogue asks what
        # the desks asked, one category per theme and thing asked.
        if kind == "iptv-org":
            owner: dict[str, str] = {}
            for cat in categories:
                for i in cat.get("iptv", []):
                    if i in owner:
                        gaps.append(f"{where}: iptv-org `{i}` is in `{owner[i]}` and `{cat['id']}`; a category has one theme")
                    owner[i] = cat["id"]
                    native(i, cat["id"], i, [], False)
        elif kind == "archive":
            for cat in categories:
                for c in cat.get("archive", []):
                    native(f"{cat['id']}:{c}", cat["id"], c, list(cat.get("subjects", [])), False)
            for m, col in enumerate(src.get("collections", [])):
                c, theme, media = col.get("id", ""), col.get("theme", ""), col.get("media", "")
                if not COLLECTION.match(str(c)):
                    gaps.append(f"{where}: collections[{m}] `{c}` is not shaped like an archive.org collection")
                if media not in ("movies", "audio"):
                    gaps.append(f"{where}: collections[{m}] media `{media}` is neither `movies` nor `audio`")
                if sorted(set(col) - {"id", "theme", "media"}):
                    gaps.append(f"{where}: collections[{m}] has fields it does not use")
                native(f"{theme}:{c}", theme, c, [], media == "audio")
        elif kind == "youtube-lives":
            for cat in categories:
                for w in cat.get("lives", []):
                    native(f"{cat['id']}:{w}", cat["id"], w, [], False)
        elif kind == "nasa":
            for m, s in enumerate(src.get("searches", [])):
                words, theme = s.get("words", ""), s.get("theme", "")
                if not WORDS.match(str(words)) or not str(words).strip() or str(words) != str(words).strip():
                    gaps.append(f"{where}: searches[{m}] `{words}` is empty, padded or has a control character")
                native(words, theme, words, [], False)
        elif kind == "commons":
            for m, c in enumerate(src.get("categories", [])):
                name, theme = c.get("name", ""), c.get("theme", "")
                if not COMMONS.match(str(name)):
                    gaps.append(f"{where}: categories[{m}] `{name}` is not a Commons category's name (no `Category:`, no `_`)")
                native(name, theme, name, [], False)
        elif kind == "peertube":
            for m, c in enumerate(src.get("categories", [])):
                cid, theme = c.get("id"), c.get("theme", "")
                if not isinstance(cid, int) or isinstance(cid, bool) or cid not in PEERTUBE_CATEGORIES:
                    gaps.append(f"{where}: categories[{m}] `{cid}` is not a category PeerTube documents (1 to 18)")
                    continue
                native(str(cid), theme, str(cid), [], False)
        keys = [x["key"] for x in natives]
        if len(set(keys)) != len(keys):
            gaps.append(f"{where}: a category is named twice")
        if not natives:
            gaps.append(f"{where}: it asks for nothing")
        natives_of[sid] = natives
    if len(set(source_ids)) != len(source_ids):
        gaps.append(f"a default source id is declared twice: {source_ids}")
    if len(set(kinds_seen)) != len(kinds_seen):
        gaps.append(f"a kind of default source is declared twice: {kinds_seen}")
    clash = sorted(set(source_ids) & theme_ids)
    if clash:
        gaps.append(f"{clash} are both a theme and a source: their keys would collide")

    # EVERY THEME IS REACHED: by a desk source, or by a default source's
    # category. A theme nothing names is a row that never shows anything.
    for cat in categories:
        desk = cat.get("iptv") or cat.get("archive") or cat.get("lives")
        if not desk and cat.get("id") not in named_themes:
            gaps.append(f"`{cat.get('id')}`: no source at all — a row that can never show anything")

    if gaps:
        print("ERROR: assets/channels.nix does not hold up.", file=sys.stderr)
        for gap in gaps:
            print(f"  - {gap}", file=sys.stderr)
        return 1

    lines: list[str] = []
    w = lines.append
    w("// GENERATED by nix/scripts/gen-broadcast.py from assets/channels.nix. Do not edit.")
    w("")
    w("/// Every category of \"channels by theme\" a DESK asks for (radio and TV's")
    w("/// pages), in the order the page lists them. Themes only the catalogue")
    w("/// reaches are in [`THEMES`] and not here.")
    w("pub const CATEGORIES: &[Category] = &[")
    for cat in categories:
        if not (cat.get("iptv") or cat.get("archive") or cat.get("lives")):
            continue
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
    w("/// Every theme of the catalogue, in the order a page lists them.")
    w("pub const THEMES: &[Theme] = &[")
    for cat in categories:
        w("    Theme {")
        w(f"        id: {rust_str(cat['id'])},")
        w(f"        key: {rust_str('player-channels-' + cat['id'])},")
        w(f"        aliases: {rust_list(cat.get('aliases', []))},")
        w("    },")
    w("];")
    w("")
    w("/// The catalogue's default sources, in the order a page lists them.")
    w("pub const DEFAULT_SOURCES: &[DefaultSource] = &[")
    for src in sources:
        variant, fresh_name = KINDS[src["kind"]]
        hosts = list(src.get("hosts", []))
        poster_hosts = list(src.get("posterHosts", []))
        posters_on = bool(poster_hosts) and all(h in hosts for h in poster_hosts)
        w("    DefaultSource {")
        w(f"        id: {rust_str(src['id'])},")
        w(f"        kind: SourceKind::{variant},")
        w(f"        key: {rust_str('player-channels-source-' + src['id'])},")
        w(f"        hosts: {rust_list(hosts)},")
        w(f"        poster_hosts: {rust_list(poster_hosts)},")
        w(f"        posters_on: {'true' if posters_on else 'false'},")
        w(f"        fresh_minutes: {int(fresh[fresh_name])},")
        w("        natives: &[")
        for x in natives_of[src["id"]]:
            w("            Native {")
            w(f"                key: {rust_str(x['key'])},")
            w(f"                theme: {rust_str(x['theme'])},")
            w(f"                ask: {rust_str(x['ask'])},")
            w(f"                narrow: {rust_list(x['narrow'])},")
            w(f"                audio: {'true' if x['audio'] else 'false'},")
            w("            },")
        w("        ],")
        w("    },")
    w("];")
    w("")
    w("/// Words that send a person's group to the adult store.")
    w(f"pub const ADULT_WORDS: &[&str] = {rust_list(adult)};")
    w("")
    w("/// iptv-org categories that never reach the index.")
    w(f"pub const EXCLUDED: &[&str] = {rust_list(excluded)};")
    w("")
    for name in FRESH:
        w(f"/// How long the {name} answer is shown without asking again, in minutes.")
        w(f"pub const FRESH_{name.upper()}_MINUTES: u64 = {int(fresh[name])};")
    w("")
    for name, (const, kind) in CATALOGUE.items():
        w(f"/// `catalogue.{name}` of `assets/channels.nix`.")
        w(f"pub const {const}: {kind} = {int(catalogue[name])};")
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
    keys = sorted(
        ["player-channels-" + c["id"] for c in categories]
        + ["player-channels-source-" + s["id"] for s in sources]
    )
    pathlib.Path(args.out_keys).write_text("\n".join(keys) + "\n", encoding="utf-8")
    natives = sum(len(v) for v in natives_of.values())
    print(
        f"OK: {len(categories)} themes, {len(sources)} default sources asking {natives} categories, "
        f"{len(keys)} text keys, {len(known)} iptv-org categories known"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
