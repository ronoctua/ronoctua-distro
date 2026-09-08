#!/usr/bin/env python3
"""Turn `assets/bluetooth-profiles.nix` into the Rust table and the owed keys.

The same shape as `gen-notifications.py`, and for the same reason: the
declaration is the source of truth, this only translates it, and anything it
cannot translate takes the build down NAMING the entry — a service quietly
dropped here would come out as a raw UUID on screen, which is exactly the
defect the file exists to remove.
"""

import argparse
import json
import re
import sys

# The four hex digits of a SIG-assigned 16-bit id, lower case. BlueZ emits
# lower case; anything else is a manufacturer's own id and never reaches here.
SHORT = re.compile(r"^[0-9a-f]{4}$")
# A text key, in the family the catalogue uses. Not any key: a typo that landed
# in another family would be demanded of both languages and granted by a
# sentence about something else.
KEY = re.compile(r"^bluetooth-profile-[a-z0-9-]+$")


def refuse(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profiles", required=True, help="the catalogue, as JSON")
    parser.add_argument("--out", required=True, help="where the Rust table goes")
    parser.add_argument("--out-keys", required=True, help="where the owed keys go")
    args = parser.parse_args()

    with open(args.profiles, encoding="utf-8") as handle:
        catalogue = json.load(handle)

    if not catalogue:
        refuse("the catalogue is empty")

    rows: list[tuple[str, str | None]] = []
    for short in sorted(catalogue):
        entry = catalogue[short]
        if not SHORT.match(short):
            refuse(
                f"`{short}` is not a 16-bit service id. It has to be four "
                "lower-case hex digits; a manufacturer's own 128-bit id has no "
                "published meaning and is drawn raw."
            )
        if not isinstance(entry, dict):
            refuse(f"`{short}` is not an attribute set")

        unknown = set(entry) - {"key", "hidden"}
        if unknown:
            refuse(f"`{short}` carries a field that does not exist: {sorted(unknown)}")

        key = entry.get("key")
        hidden = entry.get("hidden", False)

        if not isinstance(hidden, bool):
            refuse(f"`{short}`: `hidden` is not a boolean")
        if key is not None and hidden:
            refuse(
                f"`{short}` is both named and hidden. A hidden service is not "
                "drawn at all, so its sentence would be a text nobody reads and "
                "both languages would owe it."
            )
        if key is None and not hidden:
            refuse(
                f"`{short}` is neither named nor hidden. Leaving it out of the "
                "file would have been the way to say `we do not know this one` "
                "— it would then come out raw, which is the truth."
            )
        if key is not None and not KEY.match(key):
            refuse(f"`{short}`: `{key}` is not a `bluetooth-profile-…` key")

        rows.append((short, key))

    named = sorted({key for _, key in rows if key is not None})
    if not named:
        refuse("no service is named — the whole table would draw raw UUIDs")

    with open(args.out, "w", encoding="utf-8") as out:
        out.write("// GENERATED from assets/bluetooth-profiles.nix. Do not edit.\n")
        # A DOC COMMENT AND NOT A PLAIN ONE: the crate warns on a public item
        # without documentation, and a generated file is not exempt.
        out.write("/// Every service id this system knows, and what it means.\n")
        out.write("///\n")
        out.write("/// `None` is a service this system knows and does NOT draw: the\n")
        out.write("/// plumbing every BLE device is obliged to carry. A service that is\n")
        out.write("/// not in this table at all is one nobody named, and it comes out raw.\n")
        out.write("pub const PROFILES: &[(&str, Option<&str>)] = &[\n")
        for short, key in rows:
            said = f'Some("{key}")' if key is not None else "None"
            out.write(f'    ("{short}", {said}),\n')
        out.write("];\n")

    with open(args.out_keys, "w", encoding="utf-8") as out:
        for key in named:
            out.write(f"{key}\n")

    print(
        f"{len(rows)} services, {len(named)} sentences",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
