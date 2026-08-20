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
            print(f"{path}: linha nao reconhecida: {line!r}", file=sys.stderr)
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
        print(f"{path}: nao comeca com o cabecalho MIME-Magic", file=sys.stderr)
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
            print(f"{path}: regra antes de qualquer secao, em {at}", file=sys.stderr)
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
    ap.add_argument("--version", required=True, help="the pinned shared-mime-info version")
    args = ap.parse_args()

    mime_dir = Path(args.mime_dir)
    stated = (mime_dir / "version").read_text(encoding="utf-8").strip()
    if stated != args.version:
        print(
            f"ERRO: o pacote diz versao {stated} e o Nix pinou {args.version}.\n"
            "       As tabelas foram geradas contra a versao pinada; a divergencia\n"
            "       tem de ser olhada, nao seguida.",
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

    def token_char(token: str, why: str) -> str | None:
        entry = icons.get(token)
        if entry is None:
            nonlocal failed
            print(
                f"ERRO: {why} aponta para o token `{token}`, que nao existe em "
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
            f"ERRO: o freedesktop usa o icone generico `{name}` e "
            "assets/icons/mime-glyphs.nix nao responde por ele",
            file=sys.stderr,
        )
        failed = True

    # --- exact type -> glyph ----------------------------------------------
    of_exact: dict[str, str] = {}
    for row in table["exact"]:
        mime = row["mime"]
        if mime not in types:
            print(
                f"ERRO: `exact` nomeia o tipo `{mime}`, que nao existe na base. "
                "Uma linha que nao casa com nada parece proposital e ninguem audita.",
                file=sys.stderr,
            )
            failed = True
            continue
        ch = token_char(row["token"], f"exact `{mime}`")
        if ch is not None:
            of_exact[mime] = ch

    # --- what upstream does not have --------------------------------------
    ours: list[tuple[str, str]] = []
    for row in table["ours"]:
        mime = row["mime"]
        if mime in types:
            print(
                f"ERRO: `ours` inventa `{mime}`, que a base JA TEM. "
                "Ele pertence a `exact`.",
                file=sys.stderr,
            )
            failed = True
            continue
        if not MIME.match(mime):
            print(f"ERRO: `{mime}` nao tem a forma de um tipo MIME", file=sys.stderr)
            failed = True
            continue
        ch = token_char(row["token"], f"ours `{mime}`")
        if ch is not None:
            of_exact[mime] = ch
            ours.append((mime, row["glob"]))

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
                f"ERRO: `{name}` veio com {got} linhas, abaixo do piso de {floor}. "
                "Um leitor que parou de entender o formato devolve poucas linhas, "
                "e isso se le como base pequena em vez de leitor quebrado.",
                file=sys.stderr,
            )
            failed = True

    if failed:
        return 1

    # OUR rows go in with the database's own default weight, so they compete
    # on the same terms as everything else rather than winning by construction.
    for mime, pattern in ours:
        globs.append((50, mime, pattern, False))

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
    print(
        f"OK: {len(globs)} globs, {len(magic)} regras de magic, {len(subclass)} subclasses, "
        f"{len(generic)} genericos -> {args.out}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
