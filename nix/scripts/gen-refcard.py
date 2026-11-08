#!/usr/bin/env python3
"""GNU's Emacs reference card as data — the card window's (`refcard.rs`).

The card ships with Emacs as plain TeX (`etc/refcards/refcard.tex`, and its
translations beside it). It used to be typeset here, page by page, and the
window drew the pages as pictures: Computer Modern at whatever size the screen
gave, in no font of this system. Now the card's WORDS are read out of the same
source, and the window lays them out itself, in the system's face and in the
theme's inks, as many columns as the screen holds.

THE CARD IS A SMALL LANGUAGE: a dozen macros (`\\section`, `\\key`, `\\metax`,
`\\threecol` inside a `\\paralign`, `\\kbd`, the font switches, the accents of the
translations), and this reads exactly those. Anything else in the card's body is
REFUSED, with its line — a new macro in a new Emacs is a build that fails here,
never a card that silently loses a row.

THE COPYRIGHT NOTICE COMES ALONG. The card is free under the GPL, with the
special permission to distribute it formatted with the notice it carries
(`\\copyrightnotice`); the notice is read out of the card's own definition and
travels in the data, though the window does not draw it (the owner,
2026-11-08).

Out: one JSON document, format 1 —

    {"format": 1, "version": "31", "title": RUNS, "subtitle": RUNS,
     "sections": [{"title": RUNS, "blocks": [BLOCK…]}], "notice": [BLOCK…]}

where a BLOCK is one of `{"kind": "key", "does": RUNS, "keys": RUNS, "indent": bool}`,
`{"kind": "table", "rows": [{"head": bool, "cells": [RUNS, RUNS, RUNS]}]}` (a row
with no cells is the air between two parts of one table), `{"kind": "note",
"text": RUNS}`, `{"kind": "example", "lines": [RUNS…]}`, `{"kind": "centred",
"text": RUNS}` and `{"kind": "gap"}`; and RUNS is a list of `[style, text]`, the
style one of `p` (plain), `b` (bold), `i` (a variable), `k` (keys) and `c` (code).

Usage:
    gen-refcard.py <card.tex> <emacsver.tex> <out.json>
"""

import json
import re
import sys
import unicodedata
from pathlib import Path


class Refused(Exception):
    pass


# THE ACCENTS of the translations, as combining marks: `\\~a` is `a` and a tilde,
# composed by NFC. `\\c` is a control WORD; the others are symbols.
ACCENTS = {"'": "\u0301", "`": "\u0300", "^": "\u0302", "~": "\u0303", '"': "\u0308", "c": "\u0327"}

# THE CHARACTERS a backslash quotes.
QUOTED = {"\\": "\\", "{": "{", "}": "}", "%": "%", "$": "$", "&": "&", "#": "#", "_": "_", " ": " "}

# A FONT SWITCH changes the style until its group closes.
SWITCHES = {"bf": "b", "it": "i", "rm": "p", "tt": "k"}

# THE WIDENED-TABLE WRAPPER the card puts around two of its tables: a group that
# makes the table a few `0`s wider. The width is TeX's; the group's content is
# read as the card's body.
WIDENED = re.compile(r"^\s*\\setbox0=\\hbox\{\\kbd\{0\}\}\\advance\\hsize by \d+\\wd0\s*")


def uncommented(text: str) -> list[tuple[int, str]]:
    """The card's lines, numbered, with every comment cut off — and a line that
    was ONLY a comment dropped, as TeX drops it (it is no blank line)."""
    lines = []
    for number, line in enumerate(text.split("\n"), 1):
        out, i, cut = [], 0, False
        while i < len(line):
            if line[i] == "\\" and i + 1 < len(line):
                out.append(line[i : i + 2])
                i += 2
                continue
            if line[i] == "%":
                cut = True
                break
            out.append(line[i])
            i += 1
        kept = "".join(out)
        if cut and not kept.strip():
            continue
        lines.append((number, kept))
    return lines


class Reader:
    """A cursor over a stretch of the card, which knows its lines."""

    def __init__(self, text: str, numbers: list[int], values: dict[str, str]):
        self.text = text
        self.numbers = numbers
        self.values = values
        self.at = 0

    def line(self) -> int:
        row = self.text.count("\n", 0, self.at)
        return self.numbers[min(row, len(self.numbers) - 1)] if self.numbers else 0

    def refuse(self, why: str):
        raise Refused(f"line {self.line()}: {why}")

    def done(self) -> bool:
        return self.at >= len(self.text)

    def peek(self, n: int = 1) -> str:
        return self.text[self.at : self.at + n]

    def control(self) -> str:
        """The control sequence at the cursor, without its backslash; a word
        eats the spaces after it, as TeX does."""
        assert self.peek() == "\\"
        self.at += 1
        if self.done():
            self.refuse("a backslash ends the card")
        if self.text[self.at].isalpha():
            start = self.at
            while not self.done() and self.text[self.at].isalpha():
                self.at += 1
            word = self.text[start : self.at]
            while self.peek() in (" ", "\t"):
                self.at += 1
            return word
        self.at += 1
        return self.text[self.at - 1]

    def group(self) -> str:
        """The balanced `{…}` at the cursor (spaces before it skipped), without
        its braces."""
        while self.peek() in (" ", "\t", "\n"):
            self.at += 1
        if self.peek() != "{":
            self.refuse(f"a group was expected, not {self.peek()!r}")
        depth, start = 0, self.at
        while not self.done():
            c = self.text[self.at]
            if c == "\\":
                self.at += 2
                continue
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    self.at += 1
                    return self.text[start + 1 : self.at - 1]
            self.at += 1
        self.refuse("a group is never closed")


def merged(runs: list[list[str]]) -> list[list[str]]:
    """Runs with their spaces collapsed — within a run and across two —, the
    empty ones dropped and two of one style made one; no space at either end."""
    joined: list[list[str]] = []
    for style, text in runs:
        if joined and joined[-1][0] == style:
            joined[-1][1] += text
        else:
            joined.append([style, text])
    out: list[list[str]] = []
    for style, text in joined:
        text = re.sub(r"[ \t\n]+", " ", text)
        if out and out[-1][1].endswith(" "):
            text = text.lstrip(" ")
        if text:
            out.append([style, text])
    if out:
        out[0][1] = out[0][1].lstrip()
        out[-1][1] = out[-1][1].rstrip()
    return [r for r in out if r[1]]


def plain(runs: list[list[str]]) -> str:
    return "".join(t for _, t in runs)


def inline(reader: Reader, text: str, style: str) -> list[list[str]]:
    """A stretch of running text: the runs it reads as. `style` is the one the
    stretch starts in; a switch inside lasts to the stretch's end."""
    sub = Reader(text, reader.numbers, reader.values)
    sub.at = 0
    # The sub-reader reports the outer reader's line: the card's stretches are
    # short, and the line of the macro that holds them is the one to look at.
    sub.line = reader.line  # type: ignore[method-assign]
    runs: list[list[str]] = []

    def put(chars: str, how: str | None = None):
        runs.append([how or style, chars])

    while not sub.done():
        c = sub.peek()
        if c == "\\":
            name = sub.control()
            if name in SWITCHES:
                style = SWITCHES[name]
            elif name == "kbd":
                runs.extend(inline(sub, sub.group(), "k"))
            elif name == "ldots":
                put("…")
            elif name == "copyright":
                put("©")
            elif name == "TeX":
                put("TeX")
                if sub.peek(2) == "{}":
                    sub.at += 2
            elif name in ("year", "versionemacs"):
                put(reader.values[name])
            elif name in ("null", "/", "-", "small"):
                pass
            elif name == "<":
                end = sub.text.find(">", sub.at)
                if end < 0:
                    sub.refuse("a `\\<` is never closed")
                runs.extend(inline(sub, sub.text[sub.at : end], "i"))
                sub.at = end + 1
            elif name in ACCENTS:
                while name == "c" and sub.peek() == " ":
                    sub.at += 1
                if sub.peek() == "{":
                    base = plain(inline(sub, sub.group(), style))
                elif sub.peek(2) == "\\i":
                    sub.at += 2
                    base = "i"
                else:
                    base = sub.peek()
                    sub.at += 1
                if len(base) != 1:
                    sub.refuse(f"an accent over {base!r}")
                put(unicodedata.normalize("NFC", base + ACCENTS[name]))
            elif name == "i":
                put("ı")
            elif name in QUOTED:
                put(QUOTED[name])
            else:
                sub.refuse(f"the card says `\\{name}`, which this reader does not know")
        elif c == "{":
            runs.extend(inline(sub, sub.group(), style))
        elif c == "}":
            sub.refuse("a `}` closes nothing")
        elif c == "$":
            end = sub.text.find("$", sub.at + 1)
            if end < 0:
                sub.refuse("a `$` is never closed")
            runs.extend(inline(sub, sub.text[sub.at + 1 : end], style))
            sub.at = end + 1
        elif c == "&":
            sub.refuse("a `&` outside a table")
        elif style in ("k", "c"):
            # KEYS AND CODE ARE LITERAL: `M--` is Meta and a minus, not a dash.
            put(c)
            sub.at += 1
        elif c == "~":
            put("\u00a0")
            sub.at += 1
        elif sub.peek(3) == "---":
            put("—")
            sub.at += 3
        elif sub.peek(2) == "--":
            put("–")
            sub.at += 2
        elif sub.peek(2) == "``":
            put("“")
            sub.at += 2
        elif sub.peek(2) == "''":
            put("”")
            sub.at += 2
        elif c == "'":
            put("’")
            sub.at += 1
        elif c == "`":
            put("‘")
            sub.at += 1
        else:
            put(c)
            sub.at += 1
    return runs


# WHAT ENDS A PARAGRAPH, besides a blank line: a line that starts one of these.
VERTICAL = (
    "title", "centerline", "section", "key", "metax", "paralign", "beginexample",
    "beginindentedkeys", "endindentedkeys", "newcolumn", "shortcopyrightnotice",
    "copyrightnotice", "bye", "vskip", "begingroup", "endgroup",
)


class Card:
    def __init__(self):
        self.title: list | None = None
        self.subtitle: list | None = None
        self.sections: list[dict] = []
        self.notice: list[dict] = []
        self.indent = False

    def blocks(self, reader: Reader) -> list[dict]:
        if not self.sections:
            reader.refuse("a block before the first section")
        return self.sections[-1]["blocks"]


def table(reader: Reader, card: Card, body: str) -> dict:
    """A `\\paralign`: its preamble (everything to the first `\\cr`) skipped,
    then `\\threecol` rows; a blank line is the air between two parts."""
    head_end = body.find("\\cr")
    if head_end < 0:
        reader.refuse("a table without its preamble")
    sub = Reader(body[head_end + 3 :], reader.numbers, reader.values)
    sub.line = reader.line  # type: ignore[method-assign]
    rows: list[dict] = []
    while not sub.done():
        c = sub.peek()
        if c in " \t":
            sub.at += 1
        elif c == "\n":
            blank = sub.text[sub.at :].split("\n", 2)
            sub.at += 1
            if len(blank) > 1 and not blank[1].strip() and rows and rows[-1]["cells"]:
                rows.append({"head": False, "cells": [], "indent": False})
        elif c == "\\":
            name = sub.control()
            if name != "threecol":
                sub.refuse(f"a table holds `\\{name}`, not only `\\threecol`")
            cells = [merged(inline(sub, sub.group(), style)) for style in ("p", "k", "k")]
            head = all(cell and all(s == "b" for s, _ in cell) for cell in cells)
            rows.append({"head": head, "cells": cells, "indent": card.indent})
        else:
            sub.refuse(f"a table holds {c!r}")
    while rows and not rows[-1]["cells"]:
        rows.pop()
    return {"kind": "table", "rows": rows}


def vertical(reader: Reader, card: Card, notice: bool = False):
    """The card's body: one block after another."""
    while not reader.done():
        c = reader.peek()
        if c in " \t":
            reader.at += 1
            continue
        if c == "\n":
            reader.at += 1
            # A BLANK LINE is the air between two groups of a section.
            rest = reader.text[reader.at :]
            if rest.split("\n", 1)[0].strip() == "" and not notice and card.sections:
                card.blocks(reader).append({"kind": "gap"})
            continue
        if c == "{":
            start = reader.at
            body = reader.group()
            widened = WIDENED.match(body)
            if widened:
                sub = Reader(body[widened.end() :], reader.numbers, reader.values)
                sub.line = reader.line  # type: ignore[method-assign]
                vertical(sub, card, notice)
                continue
            reader.at = start
            paragraph(reader, card, notice)
            continue
        if c == "\\":
            start = reader.at
            name = reader.control()
            if name == "title":
                title = merged(inline(reader, reader.group(), "p"))
                card.title = card.title or title
            elif name == "centerline":
                text = merged(inline(reader, reader.group(), "p"))
                if notice:
                    card.notice.append({"kind": "centred", "text": text})
                elif card.subtitle is None and card.title is not None and not card.sections:
                    card.subtitle = text
                # A SECOND PAGE'S title and version are the first page's again.
            elif name == "section":
                card.sections.append({"title": merged(inline(reader, reader.group(), "p")), "blocks": []})
            elif name in ("key", "metax"):
                does = merged(inline(reader, reader.group(), "p"))
                keys = merged(inline(reader, reader.group(), "k"))
                if not does or not keys:
                    reader.refuse(f"a `\\{name}` without a description or a key")
                card.blocks(reader).append({"kind": "key", "does": does, "keys": keys, "indent": card.indent})
            elif name == "paralign":
                if not reader.text.startswith("to \\hsize", reader.at):
                    reader.refuse("a `\\paralign` not `to \\hsize`")
                reader.at += len("to \\hsize")
                card.blocks(reader).append(table(reader, card, reader.group()))
            elif name == "beginexample":
                end = reader.text.find("\\endexample", reader.at)
                if end < 0:
                    reader.refuse("an example is never ended")
                code = reader.text[reader.at : end].strip("\n")
                lines = [merged_code(inline(reader, line, "c")) for line in code.split("\n")]
                reader.at = end + len("\\endexample")
                card.blocks(reader).append({"kind": "example", "lines": lines})
            elif name == "beginindentedkeys":
                card.indent = True
            elif name == "endindentedkeys":
                card.indent = False
            elif name == "copyrightnotice":
                if card.notice:
                    reader.refuse("the notice twice")
                body = reader.values["notice"]
                sub = Reader(body, [reader.line()] * (body.count("\n") + 1), reader.values)
                vertical(sub, card, notice=True)
            elif name == "vskip":
                # THE NOTICE'S SPACE above it: TeX's, not the window's.
                if not notice or not re.match(r"1ex plus 2 fill", reader.text[reader.at :]):
                    reader.refuse("a `\\vskip` outside the notice")
                reader.at += len("1ex plus 2 fill")
            elif name in ("begingroup", "endgroup", "small") and notice:
                pass
            elif name in ("newcolumn", "shortcopyrightnotice", "bye"):
                # THE PAPER'S: a column break, the short notice at the foot
                # of a page (the whole one is the card's last block), the end.
                pass
            else:
                reader.at = start
                paragraph(reader, card, notice)
            continue
        paragraph(reader, card, notice)


def merged_code(runs: list[list[str]]) -> list[list[str]]:
    """A line of an example: its spaces KEPT — the indentation is the code's."""
    out: list[list[str]] = []
    for style, text in runs:
        if out and out[-1][0] == style:
            out[-1][1] += text
        elif text:
            out.append([style, text])
    if out:
        out[-1][1] = out[-1][1].rstrip()
    return [r for r in out if r[1]]


def paragraph(reader: Reader, card: Card, notice: bool):
    """Running text, to a blank line or a line that starts a block."""
    start = reader.at
    while not reader.done():
        nl = reader.text.find("\n", reader.at)
        if nl < 0:
            reader.at = len(reader.text)
            break
        # THE PARAGRAPH STOPS ON its last line's newline, which the body then
        # reads — and a blank line after it is the body's air.
        following = reader.text[nl + 1 :].split("\n", 1)[0]
        word = re.match(r"\s*\\([a-zA-Z]+)", following)
        if not following.strip() or (word and word.group(1) in VERTICAL):
            reader.at = nl
            break
        reader.at = nl + 1
    text = merged(inline(reader, reader.text[start : reader.at], "p"))
    if not text:
        return
    block = {"kind": "note", "text": text}
    (card.notice if notice else card.blocks(reader)).append(block)


def aired(blocks: list[dict]) -> list[dict]:
    """No air at either end of a section, and never two in a row."""
    out: list[dict] = []
    for b in blocks:
        if b["kind"] == "gap" and (not out or out[-1]["kind"] == "gap"):
            continue
        out.append(b)
    while out and out[-1]["kind"] == "gap":
        out.pop()
    return out


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    card_tex, version_tex, out = map(Path, sys.argv[1:])
    source = card_tex.read_text(encoding="latin-1")
    versions = version_tex.read_text(encoding="latin-1")
    values = {}
    for name in ("versionemacs", "year"):
        found = re.search(r"\\def\\" + name + r"\{([^}]*)\}", versions)
        if not found:
            print(f"ERROR: {version_tex} does not define \\{name}", file=sys.stderr)
            return 1
        values[name] = found.group(1).strip()
    notice = re.search(r"\\def\\copyrightnotice\{(.*?)\n\\endgroup\}", source, re.S)
    if not notice:
        print(f"ERROR: {card_tex} has no \\copyrightnotice, and the card is not shown without it", file=sys.stderr)
        return 1
    values["notice"] = "\n".join(t for _, t in uncommented(notice.group(1)))
    lines = uncommented(source)
    first = next((i for i, (_, t) in enumerate(lines) if t.startswith("\\title{")), None)
    if first is None:
        print(f"ERROR: {card_tex} has no \\title, where the card's body starts", file=sys.stderr)
        return 1
    body = lines[first:]
    reader = Reader("\n".join(t for _, t in body), [n for n, _ in body], values)
    card = Card()
    try:
        vertical(reader, card)
    except Refused as why:
        print(f"ERROR: {card_tex}, {why}", file=sys.stderr)
        return 1
    for section in card.sections:
        section["blocks"] = aired(section["blocks"])
    rows = sum(
        1 if b["kind"] == "key" else len([r for r in b.get("rows", []) if r["cells"]])
        for s in card.sections
        for b in s["blocks"]
    )
    # A CARD THIS SMALL is a card this reader lost, not a card.
    if len(card.sections) < 20 or rows < 120 or not card.title or not card.notice:
        print(
            f"ERROR: {card_tex} read as {len(card.sections)} sections and {rows} rows, "
            "too few to be the card",
            file=sys.stderr,
        )
        return 1
    document = {
        "format": 1,
        "version": values["versionemacs"],
        "title": card.title,
        "subtitle": card.subtitle or [],
        "sections": card.sections,
        "notice": card.notice,
    }
    out.write_text(json.dumps(document, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"OK: {len(card.sections)} sections, {rows} rows -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
