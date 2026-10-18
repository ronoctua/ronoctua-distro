#!/usr/bin/env python3
"""Generates the notification catalogue and filter table — section 4.2.

"Every notification and every pre-created filter of the distro, centralised."
That is what this reads, and it emits three things from the one source:

  (a) the Rust table `ronoctua_notify::catalogue`;
  (b) the list of text keys both languages must carry, which the i18n check
      then demands — this is how "all of the distro's notifications exist in
      English and Portuguese" stops depending on discipline;
  (c) a refusal, at BUILD time, of an invalid field, with the entry's id in the
      message.

# Why (c) matters more than it looks

Without it `show = "toast"` is a string that reaches Rust, fails to parse, and
falls into a default — the notification appears, in the wrong place, and
nothing says why. It is the same class of defect as invariant 8's `[key]`, and
the same answer: make the wrong thing impossible before it can be drawn.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

# The vocabularies. They are duplicated from the Rust on purpose and the
# `notifications-table-generated` check compares the two: a name that exists on
# one side only is a field that silently stops applying.
SHOW = {"hidden", "brief", "full"}
LIFE = {"quick", "normal", "long"}
FIELDS = {"app", "summary", "body", "tag", "catalogue", "urgency", "level"}
TEXT_TESTS = {"is", "contains", "starts-with"}
NUMERIC_TESTS = {"at-least", "at-most"}
PROGRAMS = {"open", "edit"}
ARGS = {"file", "folder"}
TOP_LEVEL = 100

# English kebab, as invariant 18 asks of every id.
ID = re.compile(r"^[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*$")

# THE LETTERS THE MANAGER KEEPS FOR ITSELF.
#
# An action is run by its letter, pressed in the manager — the band cannot take
# a key, because the footer at rest does not hold the keyboard. So the view's
# own keys and the actions' share one keyboard, and an action asking for a
# letter the view already answers is an action nobody can reach: pressing it
# would mark everything read, or forget the row.
#
# The way to find that out would be pressing it and watching something else
# happen, which is why it is refused here instead.
problems: list[str] = []


def complain(where: str, what: str) -> None:
    problems.append(f"{where}: {what}")


def rust_str(s: str) -> str:
    """A Rust string literal. The table carries names and text keys only."""
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


# The two tables the icon may name, filled in by `main`.
icons: dict = {}
glyphs: dict = {}


def resolve_glyph(where: str, name: str) -> int | None:
    """A design system TOKEN, or any Nerd Fonts glyph by name.

    The token is the ordinary case and the one every other part of this system
    uses (`icon.camera`), because a token is a decision somebody made once and
    a theme can change. But the owner asked for the other door too: naming a
    glyph of the font directly, out of the ten thousand the browser shows.

    Resolved HERE and not at runtime, which is what makes it free: the footer
    carries the 110 tokens it already had, and the 2,8 MB of `glyphs.json`
    stays a build input. The cost is that only what is DECLARED can name a
    glyph this way — an external service posting a name over the wire is a
    different door, and it does not exist yet (see docs/PENDING.md).
    """
    if name in icons:
        return icons[name]["codepoint"]
    if name in glyphs:
        return glyphs[name]
    complain(
        where,
        f"`icon = {name!r}` is neither an icon token nor a Nerd Fonts glyph. "
        "A token comes from assets/tokens/icons.nix (`camera`); a glyph comes "
        "from the icon browser (`md-bell`), without the `nf-`.",
    )
    return None


def read_terms(where: str, raw: list) -> list[dict]:
    out = []
    for i, t in enumerate(raw):
        at = f"{where}.when[{i}]"
        field = t.get("field")
        if field not in FIELDS:
            complain(at, f"the field `{field}` does not exist (there are: {', '.join(sorted(FIELDS))})")
            continue
        tests = [k for k in t if k != "field"]
        if len(tests) != 1:
            complain(at, f"needs ONE test, got {len(tests)}: {tests}")
            continue
        kind = tests[0]
        value = t[kind]
        if kind in NUMERIC_TESTS:
            # A numeric test on a text field never holds, and it would hold
            # silently: the notification simply never matches.
            if field != "level":
                complain(at, f"`{kind}` only holds for `level`, not for `{field}`")
                continue
            if not isinstance(value, int) or not 0 <= value <= TOP_LEVEL:
                complain(at, f"`{kind}` needs 0..{TOP_LEVEL}, got {value!r}")
                continue
        elif kind in TEXT_TESTS:
            if not isinstance(value, str) or not value:
                complain(at, f"`{kind}` needs text, got {value!r}")
                continue
        else:
            known = ", ".join(sorted(TEXT_TESTS | NUMERIC_TESTS))
            complain(at, f"the test `{kind}` does not exist (there are: {known})")
            continue
        out.append({"field": field, "kind": kind, "value": value})
    return out


def rust_terms(terms: list[dict]) -> str:
    parts = []
    for t in terms:
        variant = {
            "is": "Is",
            "contains": "Contains",
            "starts-with": "StartsWith",
            "at-least": "AtLeast",
            "at-most": "AtMost",
        }[t["kind"]]
        field = "".join(p.capitalize() for p in t["field"].split("-"))
        if t["kind"] in NUMERIC_TESTS:
            value = str(t["value"])
        else:
            value = f"{rust_str(t['value'])}.to_owned()"
        parts.append(
            f"Term {{ field: Field::{field}, test: Test::{variant}({value}) }}"
        )
    return "vec![" + ", ".join(parts) + "]"


# THE SWITCHES, whose Nix name is the Rust field's with kebab for snake, in
# the order the Rust literal has always listed them.
SWITCHES = ("loud", "blink", "vibrate", "audio", "auto-read")


def verdict_of(where: str, raw: dict) -> list[tuple[str, object]]:
    """What the block names, checked: `(field, value)` in the table's order,
    `glyph` as its codepoint — the one record both the Rust literal and the
    brain's catalogue (`gen-brain-catalogue.py`) are written from."""
    out: list[tuple[str, object]] = []
    show = raw.get("show")
    if show is not None:
        if show not in SHOW:
            complain(where, f"`show = {show!r}` does not exist (there are: {', '.join(sorted(SHOW))})")
        else:
            out.append(("show", show))
    life = raw.get("life")
    if life is not None:
        if life not in LIFE:
            complain(where, f"`life = {life!r}` does not exist (there are: {', '.join(sorted(LIFE))})")
        else:
            out.append(("life", life))
    for name in SWITCHES:
        v = raw.get(name)
        if v is not None:
            if not isinstance(v, bool):
                complain(where, f"`{name}` is a boolean, got {v!r}")
            else:
                out.append((name.replace("-", "_"), v))
    level = raw.get("level")
    if level is not None:
        if not isinstance(level, int) or not 0 <= level <= TOP_LEVEL:
            complain(where, f"`level` needs 0..{TOP_LEVEL}, got {level!r}")
        else:
            out.append(("level", level))
    icon = raw.get("icon")
    if icon is not None:
        cp = resolve_glyph(where, icon)
        if cp is not None:
            out.append(("glyph", cp))
    return out


def rust_verdict(verdict: list[tuple[str, object]]) -> str:
    """The `Verdict` literal of a checked verdict."""
    out = []
    for field, value in verdict:
        if field in ("show", "life"):
            out.append(f"{field}: Some({field.capitalize()}::{value.capitalize()})")
        elif field == "level":
            out.append(f"level: Some({value})")
        elif field == "glyph":
            out.append(f"glyph: Some('\\u{{{value:X}}}')")
        else:
            out.append(f"{field}: Some({'true' if value else 'false'})")
    return "Verdict { " + ", ".join(out + ["..Verdict::default()"]) + " }"


def read_verdict(where: str, raw: dict) -> str:
    """The `Verdict` literal, from whatever the block names."""
    return rust_verdict(verdict_of(where, raw))


def actions_of(where: str, raw: list, keys: set[str]) -> list[dict]:
    """The actions, checked: `{id, label, doing}` each, `doing` a `{kind, …}`
    — what the Rust literal and the brain's catalogue are written from. Every
    label key lands in `keys`."""
    out = []
    for i, a in enumerate(raw):
        at = f"{where}.actions[{i}]"
        ident = a.get("id")
        if not isinstance(ident, str) or not ID.match(ident):
            complain(at, f"the id `{ident}` is not English kebab")
            continue
        # AN ACTION HAS NO LETTER OF ITS OWN, and asking for one is refused
        # here rather than half-honoured later.
        #
        # The manager's levels already teach a dozen keys and will teach more;
        # a letter owned by a notification is a letter that collides with one
        # of them sooner or later, and the collision is invisible — the person
        # presses it and something else happens. It is also unnecessary: an
        # action is reached with the ARROWS, in the open record, where they are
        # listed one under the other.
        if "key" in a:
            complain(
                at,
                "an action has no letter of its own: the arrows reach it in the open "
                "record, and a letter here would collide with a key of the manager",
            )
        doing = None
        if "command" in a:
            doing = {"kind": "command", "command": a["command"]}
        elif "copy-path" in a:
            doing = {"kind": "copy-path"}
        elif "copy-image" in a:
            doing = {"kind": "copy-image"}
        elif "run" in a:
            program = a["run"]
            if program not in PROGRAMS:
                complain(at, f"`run = {program!r}` does not exist (there are: {', '.join(sorted(PROGRAMS))})")
                continue
            doing = {"kind": "run", "program": program, "args": list(a.get("args", []))}
        else:
            complain(at, "needs `command`, `run`, `copy-path` or `copy-image`")
            continue
        keys.add(f"notif-action-{ident}")
        out.append({"id": ident, "label": f"notif-action-{ident}", "doing": doing})
    return out


def rust_doing(doing: dict) -> str:
    """The `Doing` literal of a checked action's doing."""
    kind = doing["kind"]
    if kind == "command":
        return f"Doing::Command({rust_str(doing['command'])}.to_owned())"
    if kind == "copy-path":
        return "Doing::CopyPath"
    if kind == "copy-image":
        return "Doing::CopyImage"
    args = []
    for arg in doing["args"]:
        if arg in ARGS:
            args.append(f"Arg::{arg.capitalize()}")
        else:
            args.append(f"Arg::Literal({rust_str(arg)}.to_owned())")
    return f"Doing::Run {{ program: Program::{doing['program'].capitalize()}, args: vec![{', '.join(args)}] }}"


def rust_actions(actions: list[dict]) -> str:
    """The `Vec<Action>` literal of checked actions."""
    parts = [
        f"Action {{ id: {rust_str(a['id'])}.to_owned(), "
        f"label: Label::Key({rust_str(a['label'])}.to_owned()), "
        f"doing: {rust_doing(a['doing'])} }}"
        for a in actions
    ]
    return "vec![" + ", ".join(parts) + "]"


def read_actions(where: str, raw: list, keys: set[str]) -> str:
    return rust_actions(actions_of(where, raw, keys))


def entry_of(ident: str, e: dict, keys: set[str]) -> dict:
    """One entry of the catalogue, checked: `{id, when, apply, tags, actions,
    rewrite}` — the record the Rust table and the brain's catalogue are both
    written from. Its text keys land in `keys`."""
    where = f"entries.{ident}"
    if not ID.match(ident):
        complain(where, "the id is not English kebab (invariant 18)")
    terms = read_terms(where, e.get("when", []))
    if not terms:
        # An entry with no terms would recognise EVERYTHING, and the first
        # one in the table would swallow every notification on the machine.
        complain(where, "an empty `when` would recognise everything; one entry is one notification")
    keys.add(f"notif-{ident}-summary")
    keys.add(f"notif-{ident}-body")
    tags = e.get("tags", [])
    for t in tags:
        if not isinstance(t, str) or not ID.match(t):
            complain(where, f"the tag `{t}` is not English kebab")
    rewrite = e.get("rewrite", False)
    if not isinstance(rewrite, bool):
        complain(where, f"`rewrite` is a boolean, got {rewrite!r}")
    return {
        "id": ident,
        "when": terms,
        "apply": verdict_of(where, e),
        "tags": list(tags),
        "actions": actions_of(where, e.get("actions", []), keys),
        "rewrite": rewrite,
    }


def rust_entry(r: dict) -> str:
    """The `Entry` literal of a checked entry."""
    return (
        "        Entry {\n"
        f"            id: {rust_str(r['id'])}.to_owned(),\n"
        f"            when: {rust_terms(r['when'])},\n"
        f"            apply: {rust_verdict(r['apply'])},\n"
        "            tags: vec!["
        + ", ".join(f"{rust_str(t)}.to_owned()" for t in r["tags"])
        + "],\n"
        f"            actions: {rust_actions(r['actions'])},\n"
        f"            rewrite: {'true' if r['rewrite'] else 'false'},\n"
        "        },"
    )


# SERDE'S OWN SPELLING of `ronoctua_notify::catalogue::Entry` — the variant's
# Rust name, every field present — for the entries that reach the footer as
# DATA (the brain modules' events, `gen-brain-catalogue.py`). Every field is
# written, `null` for one the entry leaves to the default: the notify crate's
# test re-serialises what it read and demands the same object, so a field
# spelt here in a way the Rust does not know is a failure and never a field
# silently left at its default.
VERDICT_FIELDS = ("show", "life", "loud", "level", "glyph", "blink", "vibrate", "audio", "auto_read")
TEST_VARIANTS = {"is": "Is", "contains": "Contains", "starts-with": "StartsWith", "at-least": "AtLeast", "at-most": "AtMost"}


def serde_doing(doing: dict):
    kind = doing["kind"]
    if kind == "command":
        return {"Command": doing["command"]}
    if kind == "copy-path":
        return "CopyPath"
    if kind == "copy-image":
        return "CopyImage"
    args = [arg.capitalize() if arg in ARGS else {"Literal": arg} for arg in doing["args"]]
    return {"Run": {"program": doing["program"].capitalize(), "args": args}}


def serde_entry(r: dict) -> dict:
    """The entry as `serde_json` reads it into the notify crate's `Entry`."""
    apply = {f: None for f in VERDICT_FIELDS}
    for field, value in r["apply"]:
        if field in ("show", "life"):
            apply[field] = value.capitalize()
        elif field == "glyph":
            apply[field] = chr(value)
        else:
            apply[field] = value
    return {
        "id": r["id"],
        "when": [
            {
                "field": "".join(p.capitalize() for p in t["field"].split("-")),
                "test": {TEST_VARIANTS[t["kind"]]: t["value"]},
            }
            for t in r["when"]
        ],
        "apply": apply,
        "tags": r["tags"],
        "actions": [
            {"id": a["id"], "label": {"Key": a["label"]}, "doing": serde_doing(a["doing"])} for a in r["actions"]
        ],
        "rewrite": r["rewrite"],
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--notifications", type=pathlib.Path, required=True)
    ap.add_argument(
        "--shared",
        type=pathlib.Path,
        required=True,
        help="assets/shared/notifications.json: the filters somebody shared",
    )
    ap.add_argument("--icons", type=pathlib.Path, required=True, help="the glyph database's icons.json")
    ap.add_argument(
        "--glyphs",
        type=pathlib.Path,
        required=True,
        help="the glyphdb's glyphs.json: the 10,473 Nerd Fonts names",
    )
    ap.add_argument("--out", type=pathlib.Path, required=True)
    ap.add_argument(
        "--out-keys",
        type=pathlib.Path,
        required=True,
        help="one text key per line, for the i18n check to demand",
    )
    args = ap.parse_args()

    source = json.loads(args.notifications.read_text(encoding="utf-8"))
    shared = json.loads(args.shared.read_text(encoding="utf-8"))
    if not isinstance(shared, list):
        complain("shared", "it is a LIST of filters; the shell writes it")
        shared = []
    global icons, glyphs
    icons = json.loads(args.icons.read_text(encoding="utf-8"))
    glyphs = {
        g["name"]: g["cp"]
        for g in json.loads(args.glyphs.read_text(encoding="utf-8"))
        if g.get("name")
    }
    keys: set[str] = set()

    entries = []
    for ident in sorted(source.get("entries", {})):
        entries.append(rust_entry(entry_of(ident, source["entries"][ident], keys)))

    rules = []
    seen: set[str] = set(source.get("entries", {}))
    # THE DISTRO'S OWN FIRST, THEN WHAT SOMEBODY SHARED — and the order is the
    # rule: the first filter that matches decides, so what was declared here
    # keeps its say over what arrived later. A shared one is a filter somebody
    # wrote on their machine and sent to the repository; the only thing that
    # makes it real is this build.
    declared = [(f"rules[{i}]", r) for i, r in enumerate(source.get("rules", []))]
    declared += [(f"shared[{i}]", r) for i, r in enumerate(shared)]
    for where, r in declared:
        ident = r.get("id")
        if not isinstance(ident, str) or not ID.match(ident):
            complain(where, f"the id `{ident}` is not English kebab")
            continue
        # The person's adjustments are keyed by id, and an entry and a rule
        # sharing one would make an adjustment reach both.
        if ident in seen:
            complain(where, f"the id `{ident}` already belongs to another entry or rule")
        seen.add(ident)
        # A TEXT KEY ONLY FOR THE DISTRO'S OWN. A shared filter is named by
        # whoever wrote it, in whatever language they wrote it in, and demanding
        # a translation of it in both `.ftl` would make sharing one impossible.
        # The screen falls back to the id, which IS the name they typed.
        if where.startswith("rules["):
            keys.add(f"notif-rule-{ident}")
        terms = read_terms(f"{where}.{ident}", r.get("when", []))
        rules.append(
            "        Rule {\n"
            f"            id: {rust_str(ident)}.to_owned(),\n"
            f"            when: {rust_terms(terms)},\n"
            f"            apply: {read_verdict(where, r.get('apply', {}))},\n"
            "        },"
        )

    if problems:
        print("ERROR: assets/notifications.nix does not pass:", file=sys.stderr)
        for p in problems:
            print(f"       {p}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       The table is the single source of the distro's notifications", file=sys.stderr)
        print("       (section 4.2). A wrong field here would become a", file=sys.stderr)
        print("       notification in the wrong place, with nothing saying why.", file=sys.stderr)
        return 1

    out = [
        "// GENERATED by nix/scripts/gen-notifications.py. DO NOT EDIT BY HAND.",
        "//",
        "// The catalogue lives in assets/notifications.nix — once, there (section 4.2).",
        "// Editing here means losing the change at the next build.",
        "",
        "/// The distro's notifications, by id.",
        "fn generated_entries() -> Vec<Entry> {",
        "    vec![",
        *entries,
        "    ]",
        "}",
        "",
        "/// The distro's filters, IN DECLARED ORDER — the first that catches decides.",
        "fn generated_rules() -> Vec<Rule> {",
        "    vec![",
        *rules,
        "    ]",
        "}",
        "",
    ]
    args.out.write_text("\n".join(out), encoding="utf-8")
    args.out_keys.write_text("\n".join(sorted(keys)) + "\n", encoding="utf-8")
    print(
        f"notifications: {len(entries)} entries, {len(rules)} filters, "
        f"{len(keys)} text keys required"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
