#!/usr/bin/env python3
"""Turn `assets/syncthing-options.nix` into the Rust table and the owed keys.

The same shape as `gen-audio-names.py`, and for the same reason: the
declaration is the source of truth, this only translates it, and anything it
cannot translate takes the build down NAMING the entry.

What it refuses is chosen to catch the ways a table of a hundred options can
lie without anybody noticing:

  * a duplicate `id`, which would make two rows share one label and one text
    key — and the screen would then say the same word about two different
    settings, which is what `labels-of-a-view-are-distinct` refuses one layer
    up;
  * a duplicate `(scope, within, key)`, which is two rows writing the same JSON
    field: whichever the cursor is on wins, and the other one silently lies;
  * a `number` with no range, or a range that is backwards — a row whose arrows
    either do nothing or run away;
  * a `ring` with fewer than two words, which is a ring that cannot turn;
  * `fixed` on anything the flake does not actually fix. That one is checked
    against `nix/modules/syncthing.nix` by `syncthing-flake-owned-rows-refuse`,
    not here, because this script never sees the module.
"""

import argparse
import json
import re
import sys

KEBAB = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
JSON_KEY = re.compile(r"^[A-Za-z][A-Za-z0-9]*$")
KINDS = {"switch", "number", "text", "ring", "list", "amount"}
# The four objects `/rest/config` carries, and the two default templates.
SCOPES = {
    "folder": "Folder",
    "device": "Device",
    "global": "Global",
    "interface": "Interface",
}
FIELDS = {"id", "key", "within", "kind", "least", "most", "unit", "ring", "fixed", "advanced"}


def refuse(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(1)


def word_of(value: str) -> str:
    """The text key stem for one word of a ring.

    Syncthing writes them three ways — `sendreceive`, `smallestFirst`,
    `copy_file_range` — and an empty string means "none", which is a word on
    the screen like any other and cannot be a key with nothing in it.
    """
    if value == "":
        return "none"
    spaced = re.sub(r"(?<!^)(?=[A-Z])", "-", value)
    return spaced.replace("_", "-").lower()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--options", required=True, help="the table, as JSON")
    parser.add_argument("--out", required=True, help="where the Rust table goes")
    parser.add_argument("--out-keys", required=True, help="where the owed keys go")
    args = parser.parse_args()

    with open(args.options, encoding="utf-8") as handle:
        table = json.load(handle)

    unknown = set(table) - set(SCOPES)
    if unknown:
        refuse(f"the table has a scope that does not exist: {sorted(unknown)}")
    for scope in SCOPES:
        if scope not in table:
            refuse(f"the table has no `{scope}` — every one of the four is owed")

    rows: list[dict] = []
    seen_ids: dict[str, str] = {}
    seen_fields: dict[tuple[str, str, str], str] = {}
    words: set[str] = set()

    for scope, arm in SCOPES.items():
        entries = table[scope]
        if not isinstance(entries, list) or not entries:
            refuse(f"`{scope}` is not a non-empty list")
        for entry in entries:
            if not isinstance(entry, dict):
                refuse(f"`{scope}` holds something that is not an attribute set")
            extra = set(entry) - FIELDS
            if extra:
                refuse(f"`{entry.get('id')}` carries a field that does not exist: {sorted(extra)}")

            ident = entry.get("id")
            if not isinstance(ident, str) or not KEBAB.match(ident):
                refuse(f"`{ident}` is not a kebab-case id, and the text key is built from it")
            if ident in seen_ids:
                refuse(
                    f"`{ident}` is declared twice ({seen_ids[ident]} and {scope}). Two rows "
                    "would share one label, and the screen would say the same word about two "
                    "different settings."
                )
            seen_ids[ident] = scope

            key = entry.get("key")
            if not isinstance(key, str) or not JSON_KEY.match(key):
                refuse(f"`{ident}`: `{key}` is not a JSON key Syncthing could spell")

            within = entry.get("within")
            if within is not None and (not isinstance(within, str) or not JSON_KEY.match(within)):
                refuse(f"`{ident}`: `{within}` is not a JSON key")

            field = (scope, within or "", key)
            if field in seen_fields:
                refuse(
                    f"`{ident}` writes the same field as `{seen_fields[field]}` "
                    f"({scope}.{within + '.' if within else ''}{key}). Whichever row the cursor "
                    "is on wins, and the other one silently lies."
                )
            seen_fields[field] = ident

            kind = entry.get("kind")
            if kind not in KINDS:
                refuse(f"`{ident}`: `{kind}` is not one of {sorted(KINDS)}")

            unit = entry.get("unit")
            if unit is not None and (kind != "number" or not KEBAB.match(unit)):
                refuse(f"`{ident}`: `{unit}` is not a kebab-case unit on a number")

            if kind == "number":
                least, most = entry.get("least"), entry.get("most")
                if not isinstance(least, int) or not isinstance(most, int):
                    refuse(
                        f"`{ident}` is a number with no range. Without one the arrows either "
                        "do nothing or run away, and neither says so."
                    )
                if least >= most:
                    refuse(f"`{ident}`: the range {least}..{most} is backwards or empty")
            elif "least" in entry or "most" in entry:
                refuse(f"`{ident}` is a `{kind}` and carries a range")

            if kind == "ring":
                ring = entry.get("ring")
                if not isinstance(ring, list) or len(ring) < 2:
                    refuse(f"`{ident}` is a ring with fewer than two words: it cannot turn")
                if len(set(ring)) != len(ring):
                    refuse(f"`{ident}` has the same word twice in its ring")
                for value in ring:
                    if not isinstance(value, str):
                        refuse(f"`{ident}` has a ring word that is not a string")
                    words.add(word_of(value))
            elif "ring" in entry:
                refuse(f"`{ident}` is a `{kind}` and carries a ring")

            rows.append(
                {
                    "id": ident,
                    "key": key,
                    "within": within,
                    "scope": arm,
                    "kind": kind,
                    "least": entry.get("least", 0),
                    "most": entry.get("most", 0),
                    "unit": entry.get("unit"),
                    "ring": entry.get("ring", []),
                    "fixed": bool(entry.get("fixed", False)),
                    "advanced": bool(entry.get("advanced", False)),
                }
            )

    owed = sorted({f"syncthing-opt-{row['id']}" for row in rows})
    owed += sorted({f"syncthing-unit-{row['unit']}" for row in rows if row["unit"]})
    owed += sorted({f"syncthing-word-{word}" for word in words})

    with open(args.out, "w", encoding="utf-8") as out:
        out.write("// GENERATED from assets/syncthing-options.nix. Do not edit.\n")
        out.write("\n")
        out.write("/// Every option Syncthing exposes that this system offers.\n")
        out.write("///\n")
        out.write("/// In the order the screen draws them, which is the order the table\n")
        out.write("/// declares — an attribute set would sort alphabetically, and the\n")
        out.write("/// sheet's shape is an argument, not an accident.\n")
        out.write("pub const SETTINGS: &[Setting] = &[\n")
        for row in rows:
            within = (
                "None" if row["within"] is None else f'Some("{row["within"]}")'
            )
            if row["kind"] == "switch":
                kind = "Kind::Switch"
            elif row["kind"] == "number":
                unit = "None" if row["unit"] is None else f'Some("syncthing-unit-{row["unit"]}")'
                kind = (
                    f'Kind::Number {{ least: {row["least"]}, most: {row["most"]}, '
                    f"unit: {unit} }}"
                )
            elif row["kind"] == "text":
                kind = "Kind::Text"
            elif row["kind"] == "list":
                kind = "Kind::List"
            elif row["kind"] == "amount":
                kind = "Kind::Amount"
            else:
                words_of = ", ".join(f'"{w}"' for w in row["ring"])
                kind = f"Kind::Ring(&[{words_of}])"
            out.write("    Setting {\n")
            out.write(f'        id: "{row["id"]}",\n')
            out.write(f'        label: "syncthing-opt-{row["id"]}",\n')
            out.write(f'        key: "{row["key"]}",\n')
            out.write(f"        within: {within},\n")
            out.write(f"        scope: Scope::{row['scope']},\n")
            out.write(f"        kind: {kind},\n")
            out.write(f"        fixed: {str(row['fixed']).lower()},\n")
            out.write(f"        advanced: {str(row['advanced']).lower()},\n")
            out.write("    },\n")
        out.write("];\n")

    with open(args.out_keys, "w", encoding="utf-8") as out:
        for key in owed:
            out.write(f"{key}\n")

    fixed = sum(1 for row in rows if row["fixed"])
    advanced = sum(1 for row in rows if row["advanced"])
    print(
        f"{len(rows)} options ({fixed} the flake owns, {advanced} in the drawer), "
        f"{len(owed)} keys",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
