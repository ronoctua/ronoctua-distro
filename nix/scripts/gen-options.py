#!/usr/bin/env python3
"""Turn a declared option table into the Rust table and the owed keys.

The same shape as `gen-audio-names.py`, and for the same reason: the
declaration is the source of truth, this only translates it, and anything it
cannot translate takes the build down NAMING the entry.

ONE GENERATOR, FOUR TABLES. It was Syncthing's alone and it is not about
Syncthing: a hundred and thirty-one keys across four objects is the same
problem as Tor's bridges and exit nodes, ClipCascade's client and server, and
Atuin's seventy. What differs between them is two strings — the key prefix and
the list of scopes — so they are arguments rather than a second copy of two
hundred lines, which is the defect this repository keeps naming.

The SCOPES ARE ORDERED ON THE COMMAND LINE and not read off the table, because
the order is the order the screen draws in and `builtins.toJSON` sorts an
attribute set alphabetically. Declaring it at the call site is what keeps a
sheet's shape an argument rather than an accident — the same reason
`assets/themes/themes.nix` grew an `order` beside its list.

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
    against the domain’s own module by e.g. `syncthing-flake-owned-rows-refuse`,
    not here, because this script never sees the module.
"""

import argparse
import json
import re
import sys

KEBAB = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")


def variant_of(scope: str) -> str:
    """The Rust variant name for a scope id: `exit-path` becomes `ExitPath`."""
    return "".join(part.capitalize() for part in scope.split("-"))

# THE KEY IN THE FILE THE DOMAIN OWNS, and the four domains spell them
# three ways: Syncthing is `camelCase` JSON, Tor is `PascalCase` torrc, and
# Atuin is `snake_case` TOML. The underscore was missing and the message
# said "Syncthing" — which is what a rule generalised by copying looks like.
FIELD_KEY = re.compile(r"^[A-Za-z][A-Za-z0-9_]*$")
KINDS = {"switch", "number", "text", "ring", "list", "amount"}
FIELDS = {"id", "key", "within", "kind", "least", "most", "unit", "ring", "fixed", "advanced"}


def refuse(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(1)


def word_of(value: str) -> str:
    """The text key stem for one word of a ring.

    Syncthing writes them three ways — `sendreceive`, `smallestFirst`,
    `copy_file_range` — and an empty string means "none", which is a word on
    the screen like any other and cannot be a key with nothing in it.

    A SPACE BECOMES A HYPHEN, and that was a latent hole rather than a feature
    request: Tor's `Log` takes `notice syslog` as one value, and the key that
    came out was `tor-word-notice syslog`. A text key with a space in it is not
    a key — `gen-i18n.py` reads `^([a-z0-9-]+) *=` and would simply never see
    it, so the screen would draw `[tor-word-notice syslog]` and no check would
    have said why. Anything still not a key after this is REFUSED below rather
    than emitted broken.
    """
    if value == "":
        return "none"
    spaced = re.sub(r"(?<!^)(?=[A-Z])", "-", value)
    spaced = re.sub(r"[\s_]+", "-", spaced)
    return spaced.strip("-").lower()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--domain", required=True, help="the text-key prefix, e.g. `tor`")
    parser.add_argument(
        "--scopes",
        required=True,
        help="the scopes, comma-separated, IN THE ORDER THE SCREEN DRAWS THEM",
    )
    parser.add_argument("--options", required=True, help="the table, as JSON")
    parser.add_argument("--out", required=True, help="where the Rust table goes")
    parser.add_argument("--out-keys", required=True, help="where the owed keys go")
    args = parser.parse_args()

    if not KEBAB.match(args.domain):
        refuse(f"`{args.domain}` is not a kebab-case domain, and the text keys are built from it")
    scopes = {s: variant_of(s) for s in args.scopes.split(",") if s}
    if not scopes:
        refuse("no scopes were given; a table with none has nowhere to draw a row")
    for scope in scopes:
        if not KEBAB.match(scope):
            refuse(f"`{scope}` is not a kebab-case scope")

    with open(args.options, encoding="utf-8") as handle:
        table = json.load(handle)

    unknown = set(table) - set(scopes)
    if unknown:
        refuse(f"the table has a scope that was not declared: {sorted(unknown)}")
    for scope in scopes:
        if scope not in table:
            refuse(f"the table has no `{scope}` — every declared scope is owed")

    rows: list[dict] = []
    seen_ids: dict[str, str] = {}
    seen_fields: dict[tuple[str, str, str], str] = {}
    words: set[str] = set()

    for scope, arm in scopes.items():
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
            if not isinstance(key, str) or not FIELD_KEY.match(key):
                refuse(f"`{ident}`: `{key}` is not a key {args.domain} could spell")

            within = entry.get("within")
            if within is not None and (not isinstance(within, str) or not FIELD_KEY.match(within)):
                refuse(f"`{ident}`: `{within}` is not a section {args.domain} could spell")

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
                    word = word_of(value)
                    # A WORD THAT IS NOT A KEY IS REFUSED, NOT EMITTED.
                    # `gen-i18n.py` reads `^([a-z0-9-]+) *=`, so a stem with a
                    # space, a slash or a colon in it would never be matched in
                    # either `.ftl` — the screen would draw the key in brackets
                    # and `options-have-text` would be looking for something
                    # nobody could have written.
                    if not KEBAB.match(word):
                        refuse(
                            f"`{ident}`: the ring word {value!r} becomes `{word}`, "
                            "which is not a text key anything could carry"
                        )
                    words.add(word)
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

    owed = sorted({f"{args.domain}-opt-{row['id']}" for row in rows})
    owed += sorted({f"{args.domain}-unit-{row['unit']}" for row in rows if row["unit"]})
    owed += sorted({f"{args.domain}-word-{word}" for word in words})

    with open(args.out, "w", encoding="utf-8") as out:
        out.write(f"// GENERATED from assets/{args.domain}-options.nix. Do not edit.\n")
        out.write("\n")
        out.write(f"/// Every option this system offers for `{args.domain}`.\n")
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
                unit = "None" if row["unit"] is None else f'Some("{args.domain}-unit-{row["unit"]}")'
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
            out.write(f'        label: "{args.domain}-opt-{row["id"]}",\n')
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
        f"{args.domain}: {len(rows)} options ({fixed} the flake owns, "
        f"{advanced} in the drawer), {len(owed)} keys",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
