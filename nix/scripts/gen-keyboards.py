#!/usr/bin/env python3
"""Turn `assets/keyboards.nix` into the Rust vocabulary and the owed keys.

The same shape as `gen-bluetooth-profiles.py`: the declaration is the source of
truth, this only translates it, and anything it cannot translate takes the
build down naming the entry. A choice quietly dropped here is a ring with a
hole in it — and the hole is the way back to "leave this key alone".
"""

import argparse
import json
import re
import sys

# keyd's own vocabulary: a key name, or an action with arguments. Deliberately
# narrow — this is what gets written into a file a daemon parses, and the
# daemon failing takes the leader with it.
KEYD = re.compile(r"^[a-z0-9]+(\([a-z0-9]+(, ?[a-z0-9]+)*\))?$")
TEXT = re.compile(r"^keyboard-(key|choice)-[a-z0-9-]+$")


def refuse(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(1)


def read(entries: list, what: str, allow_empty: bool) -> list[tuple[str, str, bool]]:
    out = []
    seen = set()
    for entry in entries:
        if not isinstance(entry, dict):
            refuse(f"a {what} is not an attribute set")
        unknown = set(entry) - {"key", "text", "holds"}
        if unknown:
            refuse(f"a {what} carries a field that does not exist: {sorted(unknown)}")
        key = entry.get("key")
        text = entry.get("text")
        holds = entry.get("holds", False)
        if key is None or text is None:
            refuse(f"a {what} is missing `key` or `text`")
        if not isinstance(holds, bool):
            refuse(f"`{text}`: `holds` is not a boolean")
        if key == "":
            if not allow_empty:
                refuse(f"`{text}`: only a CHOICE may be empty, and it means `leave it alone`")
        elif not KEYD.match(key):
            refuse(
                f"`{key}` is not something keyd understands. It writes this into "
                "a file the daemon parses, and a daemon that refuses its config "
                "takes the leader with it."
            )
        if not TEXT.match(text):
            refuse(f"`{text}` is not a `keyboard-key-…` or `keyboard-choice-…` key")
        if key in seen:
            refuse(f"`{key}` is declared twice among the {what}s")
        seen.add(key)
        out.append((key, text, holds))
    if not out:
        refuse(f"there are no {what}s")
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keyboards", required=True, help="the vocabulary, as JSON")
    parser.add_argument("--out", required=True)
    parser.add_argument("--out-keys", required=True)
    args = parser.parse_args()

    with open(args.keyboards, encoding="utf-8") as handle:
        declared = json.load(handle)

    leader = declared.get("leader") or {}
    if set(leader) != {"key", "action"}:
        refuse("`leader` has to carry exactly `key` and `action`")
    if not KEYD.match(leader["key"]) or not KEYD.match(leader["action"]):
        refuse("the leader's rule is not something keyd understands")

    keys = read(declared.get("keys", []), "key", allow_empty=False)
    choices = read(declared.get("choices", []), "choice", allow_empty=True)

    # THE WAY BACK HAS TO BE THERE, and it has to be first: somebody who has
    # made a mess of a keyboard finds it by pressing left until the ring stops.
    if choices[0][0] != "":
        refuse(
            "the first choice is not the empty one. `leave it alone` is the way "
            "back out of a remap, and a ring whose way back is in the middle is "
            "a ring somebody has to walk blind."
        )

    with open(args.out, "w", encoding="utf-8") as out:
        out.write("// GENERATED from assets/keyboards.nix. Do not edit.\n")
        out.write("/// The rule that makes the leader, written into EVERY file.\n")
        out.write("///\n")
        out.write("/// keyd takes a device id in one file only, so a keyboard with a\n")
        out.write("/// remap of its own leaves the catch-all — and would leave the\n")
        out.write("/// leader with it. Every rendered file repeats this line.\n")
        out.write(
            f'pub const LEADER: (&str, &str) = ("{leader["key"]}", "{leader["action"]}");\n\n'
        )
        out.write("/// A key this system offers to remap: keyd's word, and ours.\n")
        out.write("pub const KEYS: &[(&str, &str)] = &[\n")
        for key, text, _ in keys:
            out.write(f'    ("{key}", "{text}"),\n')
        out.write("];\n\n")
        out.write("/// What a key can be made into. The FIRST leaves it alone.\n")
        out.write("///\n")
        out.write("/// The third field is `true` when the meaning depends on how long\n")
        out.write("/// the key is held, which the screen says out loud.\n")
        out.write("pub const CHOICES: &[(&str, &str, bool)] = &[\n")
        for key, text, holds in choices:
            out.write(f'    ("{key}", "{text}", {str(holds).lower()}),\n')
        out.write("];\n")

    with open(args.out_keys, "w", encoding="utf-8") as out:
        for _, text, _ in keys + choices:
            out.write(f"{text}\n")

    print(f"{len(keys)} keys, {len(choices)} choices", file=sys.stderr)


if __name__ == "__main__":
    main()
