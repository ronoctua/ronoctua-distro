#!/usr/bin/env python3
"""Turn `assets/audio-names.nix` into the Rust table and the owed keys.

The same shape as `gen-bluetooth-profiles.py`, and for the same reason: the
declaration is the source of truth, this only translates it, and anything it
cannot translate takes the build down NAMING the entry.

What it refuses is chosen to catch the two ways this table can lie:

  * a key outside the `audio-port-` family, which both languages would owe and
    some sentence about something else would grant; and
  * a name that is not already NORMALISED, which would sit in the table being
    matched against nothing for ever — the silent failure, because the port
    would simply keep drawing PipeWire's own description and nobody would know
    the entry was dead.
"""

import argparse
import json
import re
import sys

# What `Route::plain` produces: lower case, and `-` between words.
PLAIN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
# A text key, in this table's family and no other.
KEY = re.compile(r"^audio-port-[a-z0-9-]+$")


def refuse(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--names", required=True, help="the table, as JSON")
    parser.add_argument("--out", required=True, help="where the Rust table goes")
    parser.add_argument("--out-keys", required=True, help="where the owed keys go")
    args = parser.parse_args()

    with open(args.names, encoding="utf-8") as handle:
        table = json.load(handle)

    if not table:
        refuse("the table is empty — every port would draw in PipeWire's words")

    rows: list[tuple[str, str]] = []
    for plain in sorted(table):
        entry = table[plain]
        if not PLAIN.match(plain):
            refuse(
                f"`{plain}` is not a normalised port name. `Route::plain` "
                "lower-cases and joins words with `-`, so an entry written any "
                "other way is matched against nothing and never draws — and "
                "nothing says so, because the port keeps PipeWire's own words."
            )
        if not isinstance(entry, dict):
            refuse(f"`{plain}` is not an attribute set")
        unknown = set(entry) - {"key"}
        if unknown:
            refuse(f"`{plain}` carries a field that does not exist: {sorted(unknown)}")
        key = entry.get("key")
        if not isinstance(key, str):
            refuse(
                f"`{plain}` names no key. Leaving it out of the file entirely is "
                "how to say `we have no word for this one` — it then draws with "
                "PipeWire's description, which is the truth."
            )
        if not KEY.match(key):
            refuse(f"`{plain}`: `{key}` is not an `audio-port-…` key")
        rows.append((plain, key))

    named = sorted({key for _, key in rows})

    with open(args.out, "w", encoding="utf-8") as out:
        out.write("// GENERATED from assets/audio-names.nix. Do not edit.\n")
        out.write("/// Every sound port this system has a word of its own for.\n")
        out.write("///\n")
        out.write("/// Keyed on the NORMALISED name — see `Route::plain`. A port that is\n")
        out.write("/// not here draws with PipeWire's own description, which is the honest\n")
        out.write("/// answer and never a missing-key marker on screen.\n")
        out.write("pub const PORTS: &[(&str, &str)] = &[\n")
        for plain, key in rows:
            out.write(f'    ("{plain}", "{key}"),\n')
        out.write("];\n")

    with open(args.out_keys, "w", encoding="utf-8") as out:
        for key in named:
            out.write(f"{key}\n")

    print(f"{len(rows)} port names, {len(named)} sentences", file=sys.stderr)


if __name__ == "__main__":
    main()
