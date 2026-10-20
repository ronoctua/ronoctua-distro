#!/usr/bin/env python3
"""Turns freedesktop's MIME database into a Rust table.

WHY A GENERATOR, AND NOT A DEPENDENCY
-------------------------------------
`shared-mime-info`'s closure is 75 MiB, and 66 of those are glib and libxml2 —
which only `update-mime-database` needs, and it has already run upstream. What
a file manager needs are the files that command PRODUCED: `globs2`, `magic`,
`subclasses`, `aliases` and `generic-icons`, which come to 125 KB of plain
data.

So the package is a BUILD input and nothing else. The check
`files-closure-has-no-mime-db` asserts it is absent from what actually runs,
which is what keeps the 75 MiB from coming back in through a side door.

WHAT IT REFUSES
---------------
- a generic icon name freedesktop uses and `mime-glyphs.nix` does not answer
  for (a new upstream release announces itself instead of quietly drawing a
  square);
- an icon token that is not in `icons.nix`;
- a row of `exact` naming a type the database does not have — five of the first
  ones written did exactly that;
- a `prefer`, `claims` or `drop` row that no longer applies to the database it
  reads (a type gone, a glob no longer claimed, a tie upstream settled), and a
  `claims` row that would tie with upstream in silence;
- a table that came out shorter than the floors below, which is how a parser
  that silently stopped understanding the format is caught.
"""

import argparse
import json
import re
import sys
from pathlib import Path

# FLOORS, not exact counts. The format changes between releases and the numbers
# grow; what must never happen is a parser quietly returning a handful of rows,
# which reads as "the database is small" instead of "the reader broke".
FLOOR_GLOBS = 900
FLOOR_MAGIC = 300
FLOOR_SUBCLASS = 400
FLOOR_ALIASES = 300
FLOOR_GENERIC = 300

MIME = re.compile(r"^[a-z0-9][a-z0-9.+-]*/[a-zA-Z0-9][a-zA-Z0-9.+_-]*$")


def read_globs(path: Path) -> list[tuple[int, str, str, bool]]:
    """`weight:mime:pattern[:flags]` -> rows.

    The fourth field is a comma-separated flag list, and the only flag that
    exists is `cs`: case-sensitive. Five rows carry it, and they are the ones
    that make `*.C` C++ and `*.c` C — folding those two together would be a
    file manager that cannot tell the two languages apart.
    """
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split(":")
        if len(parts) < 3:
            print(f"{path}: unrecognised line: {line!r}", file=sys.stderr)
            raise SystemExit(1)
        weight, mime, pattern = parts[0], parts[1], parts[2]
        flags = parts[3].split(",") if len(parts) > 3 else []
        out.append((int(weight), mime, pattern, "cs" in flags))
    return out


def read_magic(path: Path) -> list[tuple[int, str, list[dict]]]:
    """The binary `magic` file.

    It is NOT a text file, and reading it line by line is the mistake it
    invites: a rule's value is a raw byte string of a declared length and may
    contain a newline. So it is walked byte by byte.

    A rule reads `[indent]>offset=<2-byte big-endian length><value>` followed by
    an optional `&mask` of the same length, `~wordsize` and `+range`.
    """
    raw = path.read_bytes()
    if not raw.startswith(b"MIME-Magic\0\n"):
        print(f"{path}: does not start with the MIME-Magic header", file=sys.stderr)
        raise SystemExit(1)
    at = len(b"MIME-Magic\0\n")
    out: list[tuple[int, str, list[dict]]] = []
    while at < len(raw):
        if raw[at : at + 1] == b"[":
            end = raw.index(b"]", at)
            priority, mime = raw[at + 1 : end].decode("utf-8").split(":", 1)
            out.append((int(priority), mime, []))
            at = end + 2  # past `]` and the newline
            continue
        if not out:
            print(f"{path}: a rule before any section, at {at}", file=sys.stderr)
            raise SystemExit(1)

        start = at
        while raw[at : at + 1] not in (b">", b""):
            at += 1
        indent = int(raw[start:at] or b"0")
        at += 1  # past `>`
        start = at
        while raw[at : at + 1] != b"=":
            at += 1
        offset = int(raw[start:at])
        at += 1  # past `=`
        length = int.from_bytes(raw[at : at + 2], "big")
        at += 2
        value = raw[at : at + length]
        at += length

        mask = None
        if raw[at : at + 1] == b"&":
            at += 1
            mask = raw[at : at + length]
            at += length
        word = 1
        if raw[at : at + 1] == b"~":
            at += 1
            start = at
            while raw[at : at + 1] not in (b"+", b"\n"):
                at += 1
            word = int(raw[start:at])
        span = 1
        if raw[at : at + 1] == b"+":
            at += 1
            start = at
            while raw[at : at + 1] != b"\n":
                at += 1
            span = int(raw[start:at])
        at += 1  # past the newline

        # A word size other than 1 means the value is a sequence of integers to
        # be byte-swapped on a little-endian host. None of the rules that
        # matter for a listing use it, and half-implementing it would be a
        # match that is right on one architecture. Skipped, and counted.
        if word != 1:
            continue
        out[-1][2].append(
            {"indent": indent, "offset": offset, "value": value, "mask": mask, "range": span}
        )
    return [r for r in out if r[2]]


def read_pairs(path: Path) -> list[tuple[str, str]]:
    """Two words per line — `subclasses` and `aliases`."""
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        a, _, b = line.partition(" ")
        if b:
            out.append((a.strip(), b.strip()))
    return out


def read_generic(path: Path) -> list[tuple[str, str]]:
    """`mime:generic-icon-name`."""
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        a, _, b = line.partition(":")
        if b:
            out.append((a.strip(), b.strip()))
    return out


def bytes_of(b: bytes) -> str:
    """A Rust byte-slice literal, always in hex.

    Hex and not a string literal because these are BYTES: a value may be
    invalid UTF-8, may contain a quote, and reading `b"\\x89PNG"` at a glance is
    no easier than reading the numbers.
    """
    return "&[" + ", ".join(f"0x{x:02X}" for x in b) + "]"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mime-dir", required=True, help="share/mime of shared-mime-info")
    ap.add_argument("--glyphs", required=True, help="mime-glyphs.nix, as JSON")
    ap.add_argument("--icons", required=True, help="icons.json from glyphgen")
    ap.add_argument("--out", required=True)
    ap.add_argument(
        "--out-types",
        help="where to write what types exist, for a generator that has to check one",
    )
    ap.add_argument("--version", required=True, help="the pinned shared-mime-info version")
    args = ap.parse_args()

    mime_dir = Path(args.mime_dir)
    stated = (mime_dir / "version").read_text(encoding="utf-8").strip()
    if stated != args.version:
        print(
            f"ERROR: the package says version {stated} and Nix pinned {args.version}.\n"
            "       The tables were generated against the pinned version; the divergence\n"
            "       has to be looked at, not followed.",
            file=sys.stderr,
        )
        return 1

    table = json.loads(Path(args.glyphs).read_text(encoding="utf-8"))
    icons = json.loads(Path(args.icons).read_text(encoding="utf-8"))

    globs = read_globs(mime_dir / "globs2")
    magic = read_magic(mime_dir / "magic")
    subclass = read_pairs(mime_dir / "subclasses")
    aliases = read_pairs(mime_dir / "aliases")
    generic = read_generic(mime_dir / "generic-icons")
    types = {
        t.strip()
        for t in (mime_dir / "types").read_text(encoding="utf-8").splitlines()
        if t.strip()
    }

    failed = False

    def refuse(message: str) -> None:
        nonlocal failed
        print(f"ERROR: {message}", file=sys.stderr)
        failed = True

    def token_char(token: str, why: str) -> str | None:
        entry = icons.get(token)
        if entry is None:
            nonlocal failed
            print(
                f"ERROR: {why} points at the token `{token}`, which does not exist in "
                "assets/tokens/icons.nix",
                file=sys.stderr,
            )
            failed = True
            return None
        return entry["char"]

    # --- generic icon name -> glyph ---------------------------------------
    of_generic: dict[str, str] = {}
    for row in table["generic"]:
        ch = token_char(row["token"], f"generic `{row['name']}`")
        if ch is not None:
            of_generic[row["name"]] = ch

    # EVERY name freedesktop uses has to be answered for. A new release adding
    # `x-office-drawing` would otherwise draw the fallback square on a whole
    # class of files, and nothing would say so.
    used = {name for _, name in generic}
    for name in sorted(used - set(of_generic)):
        print(
            f"ERROR: freedesktop uses the generic icon `{name}` and "
            "assets/icons/mime-glyphs.nix does not answer for it",
            file=sys.stderr,
        )
        failed = True

    # --- exact type -> glyph ----------------------------------------------
    of_exact: dict[str, str] = {}
    for row in table["exact"]:
        mime = row["mime"]
        if mime not in types:
            print(
                f"ERROR: `exact` names the type `{mime}`, which does not exist in the database. "
                "A line that matches nothing looks deliberate and nobody audits it.",
                file=sys.stderr,
            )
            failed = True
            continue
        ch = token_char(row["token"], f"exact `{mime}`")
        if ch is not None:
            of_exact[mime] = ch

    # --- what upstream does not have --------------------------------------
    ours: list[tuple[str, str]] = []
    ours_under: list[tuple[str, str]] = []
    for row in table["ours"]:
        mime = row["mime"]
        if mime in types:
            print(
                f"ERROR: `ours` invents `{mime}`, which the database ALREADY HAS. "
                "It belongs in `exact`.",
                file=sys.stderr,
            )
            failed = True
            continue
        if not MIME.match(mime):
            print(f"ERROR: `{mime}` is not shaped like a MIME type", file=sys.stderr)
            failed = True
            continue
        # WHAT IT DESCENDS FROM, and it is not optional.
        #
        # A type with no subclass entry answers `false` to every `is_a`, and
        # the preview's "is this text?" IS an `is_a`. Without this the `.nix`
        # files on a NixOS machine would be the ones whose content it refused
        # to show — found by a test, not by the owner's screen.
        under = row.get("under")
        if not under:
            refuse(
                f"`ours` invents `{mime}` and does not say what it descends from. "
                "A type with no parent answers `false` to every `is_a`, and the "
                "preview's \"is this text?\" is one."
            )
            continue
        if under not in types:
            refuse(
                f"`ours` says `{mime}` descends from `{under}`, which the database "
                "does not have."
            )
            continue
        ch = token_char(row["token"], f"ours `{mime}`")
        if ch is not None:
            of_exact[mime] = ch
            ours.append((mime, row["glob"]))
            # ONE PAIR PER TYPE, not per glob: TypeScript has two globs.
            if (mime, under) not in ours_under:
                ours_under.append((mime, under))

    # --- who wins a name two types claim ----------------------------------
    #
    # `mime::by_name` keeps the FIRST of an equal pair, so an ambiguity
    # upstream is settled by the order of a generated file — which is to say
    # not settled at all. A row here names the winner, the loser and the
    # pattern, and the LOSER'S ROW IS DROPPED: raising the winner's weight
    # would also beat a future claimant of higher weight, which is more than
    # the row says.
    #
    # Every way this row can rot is an error, because a row that quietly stops
    # applying looks deliberate and nobody audits it.
    for row in table.get("prefer", []):
        pattern, winner, loser = row["glob"], row["mime"], row["over"]
        where = f"`prefer` for `{pattern}`"
        if winner not in types or loser not in types:
            missing = winner if winner not in types else loser
            refuse(f"{where} names `{missing}`, which the database does not have.")
            continue
        claims = [g for g in globs if g[2] == pattern]
        by = {g[1]: g[0] for g in claims}
        if winner not in by or loser not in by:
            absent = winner if winner not in by else loser
            refuse(
                f"{where} says `{absent}` claims it, and in this database it does "
                "not. Upstream changed and the row no longer applies."
            )
            continue
        if by[winner] != by[loser]:
            refuse(
                f"{where} has weights {by[winner]} and {by[loser]}, which are not "
                "equal — upstream has already decided and this row does nothing."
            )
            continue
        globs = [g for g in globs if not (g[1] == loser and g[2] == pattern)]

    # --- a name upstream gives a type its files are not --------------------
    #
    # The row REMOVES upstream's glob: `*.fasl` is compiled Lisp, not the
    # Lisp source upstream calls it. A row that no longer removes anything
    # is refused, like a stale `prefer`.
    for row in table.get("drop", []):
        where = f"`drop` for `{row.get('glob')}`"
        unknown = sorted(set(row) - {"glob", "mime"})
        if unknown:
            refuse(f"{where} has the fields {unknown}, which this generator does not know.")
            continue
        pattern, mime = row["glob"], row["mime"]
        if mime not in types:
            refuse(f"{where} names `{mime}`, which the database does not have.")
            continue
        if not any(g[1] == mime and g[2] == pattern for g in globs):
            refuse(
                f"{where} says `{mime}` claims it, and in this database it does not. "
                "Upstream changed and the row no longer removes anything."
            )
            continue
        globs = [g for g in globs if not (g[1] == mime and g[2] == pattern)]

    # --- a name a type of the database answers here too --------------------
    #
    # Added at the database's own weight, after upstream's rows, like `ours`
    # — but for a type that EXISTS. A row that would tie with another type
    # must say so (`shares`), and say how the bytes tell the two apart
    # (`content`): the tie is then settled by `mime::of_path`, which reads a
    # tied name's head anyway. Every way the row could stop meaning what it
    # says is refused.
    claimed: list[tuple[str, str]] = []
    shared_content: list[tuple[str, str, list[str]]] = []
    for row in table.get("claims", []):
        where = f"`claims` for `{row.get('glob')}`"
        unknown = sorted(set(row) - {"glob", "mime", "shares", "content"})
        if unknown:
            refuse(f"{where} has the fields {unknown}, which this generator does not know.")
            continue
        pattern, mime = row["glob"], row["mime"]
        shares, content = row.get("shares"), row.get("content")
        if mime not in types:
            refuse(f"{where} names `{mime}`, which the database does not have.")
            continue
        if any(g[1] == mime and g[2] == pattern for g in globs):
            refuse(f"{where}: `{mime}` already claims it upstream, and the row does nothing.")
            continue
        others = sorted({g[1] for g in globs if g[2] == pattern})
        if shares is None:
            if others:
                refuse(
                    f"{where} is claimed upstream by {others}. Without `shares` the two "
                    "would tie in silence, and the first in the table would win."
                )
                continue
            if content is not None:
                refuse(f"{where} has `content` and no `shares`: there is no tie to settle.")
                continue
        else:
            if shares not in types:
                refuse(f"{where} shares with `{shares}`, which the database does not have.")
                continue
            if others != [shares]:
                refuse(
                    f"{where} says it shares with `{shares}`, and upstream's claimants are "
                    f"{others}. Upstream changed and the tie is not the one written here."
                )
                continue
            weights = {g[0] for g in globs if g[2] == pattern}
            if weights != {50}:
                refuse(
                    f"{where}: `{shares}` claims it at weight {sorted(weights)}, not the "
                    "50 this row is added at — it would not tie."
                )
                continue
            if (
                not isinstance(content, list)
                or not content
                or not all(
                    isinstance(m, str) and m and m.isascii() and m.isprintable() and len(m) <= 64
                    for m in content
                )
            ):
                refuse(
                    f"{where} shares with `{shares}` and has no `content` that could tell "
                    "them apart: a list of short printable ASCII markers."
                )
                continue
            shared_content.append((pattern, mime, content))
        claimed.append((mime, pattern))

    floors = [
        ("globs", len(globs), FLOOR_GLOBS),
        ("magic", len(magic), FLOOR_MAGIC),
        ("subclasses", len(subclass), FLOOR_SUBCLASS),
        ("aliases", len(aliases), FLOOR_ALIASES),
        ("generic-icons", len(generic), FLOOR_GENERIC),
    ]
    for name, got, floor in floors:
        if got < floor:
            print(
                f"ERROR: `{name}` came with {got} rows, below the floor of {floor}. "
                "A reader that stopped understanding the format gives back few rows, "
                "and that reads as a small base rather than a broken reader.",
                file=sys.stderr,
            )
            failed = True

    if failed:
        return 1

    # OUR rows go in with the database's own default weight, so they compete
    # on the same terms as everything else rather than winning by construction.
    for mime, pattern in ours + claimed:
        globs.append((50, mime, pattern, False))
    # OUR TYPES JOIN THE HIERARCHY, so `is_a` answers about them the way it
    # answers about everything else. Appended rather than merged, because the
    # counts above are UPSTREAM's and our own additions must not mask a reader
    # that stopped understanding the file.
    subclass.extend(ours_under)

    reach = 0
    for _, _, matches in magic:
        for m in matches:
            reach = max(reach, m["offset"] + m["range"] + len(m["value"]))

    out = [
        "// GENERATED by nix/scripts/gen-mimedb.py. Do not edit: it vanishes at the next build.",
        "//",
        f"// From shared-mime-info {args.version}, which is a BUILD input only —",
        "// see `files-closure-has-no-mime-db`.",
        "",
        "/// One name pattern, as `globs2` spells it.",
        "pub struct Glob {",
        "    /// Higher wins. The database uses 10 to 80.",
        "    pub weight: u8,",
        "    /// The type this pattern names.",
        "    pub mime: &'static str,",
        "    /// The pattern itself: `*.txt`, `makefile`, `*.so.[0-9]*`.",
        "    pub pattern: &'static str,",
        "    /// Whether case matters. It does for exactly five rows, and they are",
        "    /// the ones that tell `*.C` from `*.c`.",
        "    pub case_sensitive: bool,",
        "}",
        "",
        "/// One byte comparison inside a magic rule.",
        "pub struct Match {",
        "    /// Nesting: a match at depth n only counts if the last n-1 matched.",
        "    pub indent: u8,",
        "    /// Where to start looking.",
        "    pub offset: u32,",
        "    /// What has to be there.",
        "    pub value: &'static [u8],",
        "    /// Which bits of `value` count, when not all of them do.",
        "    pub mask: Option<&'static [u8]>,",
        "    /// How many bytes past `offset` the value may start at.",
        "    pub range: u32,",
        "}",
        "",
        "/// A type, and what its content looks like.",
        "pub struct Magic {",
        "    /// Higher wins.",
        "    pub priority: u8,",
        "    /// The type.",
        "    pub mime: &'static str,",
        "    /// The comparisons, in file order.",
        "    pub matches: &'static [Match],",
        "}",
        "",
        "/// Every name pattern the database knows, plus ours.",
        f"pub static GLOBS: [Glob; {len(globs)}] = [",
    ]
    for weight, mime, pattern, cs in globs:
        out.append(
            f'    Glob {{ weight: {weight}, mime: "{mime}", '
            f'pattern: {json.dumps(pattern)}, case_sensitive: {str(cs).lower()} }},'
        )
    out.append("];")
    out.append("")
    out.append("/// Every content rule, most specific first.")
    out.append(f"pub static MAGIC: [Magic; {len(magic)}] = [")
    for priority, mime, matches in sorted(magic, key=lambda r: -r[0]):
        inner = ", ".join(
            "Match {{ indent: {}, offset: {}, value: {}, mask: {}, range: {} }}".format(
                m["indent"],
                m["offset"],
                bytes_of(m["value"]),
                f"Some({bytes_of(m['mask'])})" if m["mask"] else "None",
                m["range"],
            )
            for m in matches
        )
        out.append(f'    Magic {{ priority: {priority}, mime: "{mime}", matches: &[{inner}] }},')
    out.append("];")
    out.append("")
    out.append("/// How many bytes of a file the rules can ever look at.")
    out.append(f"pub const MAGIC_REACH: usize = {reach};")
    out.append("")

    def pairs(name: str, doc: str, rows: list[tuple[str, str]]) -> None:
        # SORTED, so the Rust side can binary-search instead of walking five
        # hundred rows per lookup — and a listing asks per file.
        out.append(f"/// {doc}")
        out.append("///")
        out.append("/// Sorted by the first element, for `binary_search_by`.")
        out.append(f"pub static {name}: [(&str, &str); {len(rows)}] = [")
        for a, b in sorted(rows):
            out.append(f'    ("{a}", "{b}"),')
        out.append("];")
        out.append("")

    pairs("SUBCLASS", "`(type, its parent)` — what makes `is a` answerable.", subclass)
    pairs("ALIASES", "`(old name, current name)`.", aliases)
    pairs("GENERIC", "`(type, its generic icon name)`.", generic)

    out.append("/// `(pattern, type, markers)`: a name `mime-glyphs.nix` makes a tie on purpose")
    out.append("/// (`claims` with `shares`), and what in a file's head says it is this type.")
    out.append(f"pub static SHARED_CONTENT: [(&str, &str, &[&str]); {len(shared_content)}] = [")
    for pattern, mime, markers in shared_content:
        inner = ", ".join(json.dumps(m) for m in markers)
        out.append(f"    ({json.dumps(pattern)}, {json.dumps(mime)}, &[{inner}]),")
    out.append("];")
    out.append("")
    out.append("/// `(generic icon name, glyph)` — the end of `assets/icons/mime-glyphs.nix`.")
    out.append(f"pub static ICON_OF_GENERIC: [(&str, char); {len(of_generic)}] = [")
    for name in sorted(of_generic):
        out.append(f"    (\"{name}\", '\\u{{{ord(of_generic[name]):X}}}'),")
    out.append("];")
    out.append("")
    out.append("/// `(type, glyph)` — where the specific beats the generic.")
    out.append(f"pub static ICON_OF_MIME: [(&str, char); {len(of_exact)}] = [")
    for mime in sorted(of_exact):
        out.append(f"    (\"{mime}\", '\\u{{{ord(of_exact[mime]):X}}}'),")
    out.append("];")
    out.append("")
    out.append("/// How many rows UPSTREAM shipped, so a truncated table is a COMPILE error.")
    out.append("///")
    out.append("/// Upstream's count and not the array's: `GLOBS` also carries the rows")
    out.append("/// `mime-glyphs.nix` adds for types freedesktop has no name for, and")
    out.append("/// counting those here would let our own additions mask a reader that")
    out.append("/// stopped understanding the file.")
    out.append("pub mod counts {")
    for name, got, _ in floors:
        out.append(f"    /// Rows `{name}` had upstream.")
        out.append(f"    pub const {name.upper().replace('-', '_')}: usize = {got};")
    out.append("}")
    out.append("")

    Path(args.out).write_text("\n".join(out), encoding="utf-8")

    # WHAT TYPES EXIST, FOR WHOEVER HAS TO CHECK ONE.
    #
    # `assets/syntax.nix` claims MIME types and must not be allowed to invent
    # one, so `gen-syntax.py` has to cross-reference the same list this file
    # reads. Reading `share/mime` a second time there would put the pinned
    # version in two places, and the second one is the one that goes stale.
    # So this derivation stays the only reader, and publishes what it read.
    #
    # `types` is the UNION with our own: `text/x-nix` is not upstream's, and a
    # language that claims it must still pass. The `ours` block of
    # `assets/icons/mime-glyphs.nix` is where a type is invented, and it is the
    # only place.
    if args.out_types:
        Path(args.out_types).write_text(
            json.dumps(
                {
                    "version": args.version,
                    "types": sorted(types | {m for m, _ in ours}),
                    "aliases": sorted([a, b] for a, b in aliases),
                    "subclasses": sorted([a, b] for a, b in subclass),
                },
                indent=1,
                sort_keys=True,
            ),
            encoding="utf-8",
        )
    print(
        f"OK: {len(globs)} globs, {len(magic)} magic rules, {len(subclass)} subclasses, "
        f"{len(generic)} generics -> {args.out}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
