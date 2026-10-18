#!/usr/bin/env python3
"""The brain's modules as DATA — `brain-catalogue.json`, the 2026-10-12 wave (B2).

What the footer draws of a module, and nothing it compiles: the marks a module
may put on the modeline (each with its glyph's token and the key of its hint),
its notices, the actions a person may ask of it by name (the command, the
label's key, whether it acts on a row, the question it asks first), the
events it may tell — each with the notification it raises, as an entry of the
notification catalogue —, its reasons, its text prefixes, every text key it
needs, and the GLYPHS its marks are drawn with, measured against the real
face (codepoint, the size beside the text, the ink across) by the same code
that measures the chrome's own tokens.

The footer reads it at run time (`RONOCTUA_BRAIN_CATALOGUE`, which the
wrappers set): `ronoctua_brain::catalogue` resolves a mark's glyph and lists
the actions the leader registers, and `ronoctua_notify` merges the events'
notifications into its book. So a module the person writes — or a distro
module's new mark — moves this file and the daemon, never the footer's crates.

`nix/brain-modules.nix` reads the folders and hands this one record per module
(`forCatalogue`); `nix/brain-catalogue.nix` hands it the measured glyphs, the
chrome's icon table and the Nerd Fonts names (a notification's `icon`), the
distro's notification catalogue (whose ids a module's event may not take), and
every key name the build's texts hold.

WHAT IT REFUSES, by the module's name: a mark whose glyph was not measured, a
module's glyph token that is also one of the chrome's, a key the module needs
that no text holds, a notification the notification catalogue's own rules
refuse (`gen-notifications.py`, read here and not written twice), and an event
whose tag is also an entry or a filter of `assets/notifications.nix`.

Usage:
    gen-brain-catalogue.py --modules <records.json> --measured <icons.json>
        --icons <glyphdb icons.json> --glyph-names <glyphdb glyphs.json>
        --notifier <gen-notifications.py> --notifications <notifications.json>
        --shared <shared filters.json> --keys <key names.json> --out <file>
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

# THE FILE'S FORMAT. `ronoctua_brain::catalogue::VERSION` and
# `ronoctua_notify::brain::VERSION` read this one; a file of another refuses
# itself at the footer's start rather than drawing half of it.
VERSION = 1


def fail(where: str, what: str) -> None:
    print(f"ERROR: brain catalogue: {where}: {what}", file=sys.stderr)
    raise SystemExit(1)


def load_notifier(path: Path):
    """`gen-notifications.py` itself: one set of rules for an entry, whether
    it is compiled into the table or read at run time."""
    spec = importlib.util.spec_from_file_location("gen_notifications", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    ap = argparse.ArgumentParser(description="The brain's modules as data, for the footer to read at run time.")
    ap.add_argument("--modules", type=Path, required=True, help="the loader's records (`forCatalogue`), as JSON")
    ap.add_argument("--measured", type=Path, required=True, help="the modules' glyph tokens, measured (icons.json)")
    ap.add_argument("--icons", type=Path, required=True, help="the chrome's icon table (the glyph database's icons.json)")
    ap.add_argument("--glyph-names", type=Path, required=True, help="the glyph database's glyphs.json")
    ap.add_argument("--notifier", type=Path, required=True, help="gen-notifications.py, whose rules an entry obeys")
    ap.add_argument("--notifications", type=Path, required=True, help="assets/notifications.nix, as JSON")
    ap.add_argument("--shared", type=Path, required=True, help="assets/shared/notifications.json")
    ap.add_argument("--keys", type=Path, required=True, help="every key name the build's texts hold, as a JSON list")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()

    # BY ID, the order the daemon's registry holds them in (`gen-brain-modules.py`
    # sorts every module, the person's among the distro's): the leader
    # registers the actions in this order, and a check compares the two.
    records = sorted(json.loads(args.modules.read_text(encoding="utf-8")), key=lambda m: m["id"])
    measured = json.loads(args.measured.read_text(encoding="utf-8"))
    chrome = json.loads(args.icons.read_text(encoding="utf-8"))
    keys = set(json.loads(args.keys.read_text(encoding="utf-8")))
    distro = json.loads(args.notifications.read_text(encoding="utf-8"))
    shared = json.loads(args.shared.read_text(encoding="utf-8"))

    # A MODULE'S TOKEN THAT IS ALSO THE CHROME'S would be one name for two
    # drawings: the chrome's table and this file would each answer for it.
    for token in sorted(set(measured) & set(chrome)):
        fail(token, "a brain module's glyph token is also one of assets/tokens/icons.nix")

    notifier = load_notifier(args.notifier)
    # A NOTIFICATION'S `icon` is a chrome token, a module's own glyph token
    # or a Nerd Fonts name, as in the table: the chrome's first.
    notifier.icons = {**measured, **chrome}
    notifier.glyphs = {
        g["name"]: g["cp"]
        for g in json.loads(args.glyph_names.read_text(encoding="utf-8"))
        if g.get("name")
    }
    # THE IDS A MODULE'S EVENT MAY NOT TAKE: an entry of the file would be two
    # recognitions of one tag (the first in the book wins), and a filter's id
    # the key of the person's adjustments to both.
    taken = set(distro.get("entries", {})) | {r.get("id") for r in distro.get("rules", [])} | {
        r.get("id") for r in shared if isinstance(r, dict)
    }

    modules = []
    tokens: set[str] = set()
    seen: set[str] = set()
    for m in records:
        ident = m["id"]
        if ident in seen:
            fail(ident, "two records of one module")
        seen.add(ident)
        if m.get("origin") not in ("distro", "person"):
            fail(ident, f"its origin is {m.get('origin')!r}; a module is the distro's or the person's")

        needs: set[str] = {f"brain-task-{ident}", f"brain-task-{ident}-does"}

        marks = []
        for name, spec in sorted(m["marks"].items()):
            token = f"{ident}-{spec['glyph']}"
            if token not in measured:
                fail(ident, f"the mark `{name}`'s glyph `{token}` was not measured")
            needs.add(spec["hint"])
            marks.append(
                {
                    "name": name,
                    "glyph": token,
                    "hint": spec["hint"],
                    "instances": spec["instances"],
                    "resting": spec["resting"],
                }
            )
        for g in m["glyphs"]:
            token = f"{ident}-{g}"
            if token not in measured:
                fail(ident, f"the glyph `{token}` was not measured")
            tokens.add(token)

        notices = [
            {"id": n, "level": spec["level"], "dismiss": spec["dismiss"], "snooze": spec["snooze"]}
            for n, spec in sorted(m["notices"].items())
        ]

        actions = []
        for a, spec in sorted(m["actions"].items()):
            label = f"cmd-{ident}-{a}"
            needs.add(label)
            if spec.get("asks"):
                needs.add(spec["asks"])
            actions.append(
                {
                    "id": a,
                    "command": f"{ident}-{a}",
                    "label": label,
                    "row": spec["row"],
                    "asks": spec.get("asks"),
                }
            )

        events = []
        for e, spec in sorted(m["events"].items()):
            tag = f"{ident}-{e}"
            entry = None
            if spec.get("notification") is not None:
                if tag in taken:
                    fail(ident, f"the event `{tag}` is also an entry or a filter of assets/notifications.nix")
                owed: set[str] = set()
                record = notifier.entry_of(tag, spec["notification"], owed)
                needs |= owed
                entry = notifier.serde_entry(record)
            events.append({"name": e, "tag": tag, "notification": entry})

        whys = [{"word": w, "key": f"{ident}-why-{w}"} for w in m["whys"]]
        needs |= {w["key"] for w in whys}

        for key in sorted(needs - keys):
            fail(ident, f"no text of the build has `{key}`")

        modules.append(
            {
                "id": ident,
                "origin": m["origin"],
                "name-key": f"brain-task-{ident}",
                "does-key": f"brain-task-{ident}-does",
                "prefixes": m["prefixes"],
                "marks": marks,
                "notices": notices,
                "actions": actions,
                "events": events,
                "whys": whys,
                "keys": sorted(needs),
            }
        )

    if notifier.problems:
        print("ERROR: a brain module's notification does not pass:", file=sys.stderr)
        for p in notifier.problems:
            print(f"       {p}", file=sys.stderr)
        return 1

    glyphs = [
        {
            "token": token,
            "glyph": measured[token]["glyph"],
            "codepoint": measured[token]["codepoint"],
            "size": float(measured[token]["chrome"]),
            "ink": measured[token]["ink"],
            "across": measured[token]["across"],
        }
        for token in sorted(tokens)
    ]
    catalogue = {"version": VERSION, "modules": modules, "glyphs": glyphs}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(catalogue, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    told = sum(1 for m in modules for e in m["events"] if e["notification"] is not None)
    print(
        f"OK: the brain's catalogue: {len(modules)} modules, {sum(len(m['marks']) for m in modules)} marks, "
        f"{sum(len(m['actions']) for m in modules)} actions, {told} notifications, {len(glyphs)} glyphs"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
