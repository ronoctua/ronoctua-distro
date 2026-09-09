#!/usr/bin/env python3
"""The places `LEADER d` glances at, from Nix to Rust.

`assets/world.nix` becomes three things: the table the line draws, the text
keys both languages have to carry, and a refusal.

# What is refused, and why here

A ZONE THAT DOES NOT EXIST. `Zone::read_from` never fails — it falls back to
UTC and says so, which is right for a file listing whose zone went missing and
wrong for a place somebody NAMED. A typo in `America/Sao_Paolo` would draw a
clock, in UTC, labelled Brasília, and nothing on screen could say otherwise.
`zone1970.tab` is the same file the installer's list comes from and the same
one `locale-exists` checks the machine's own zone against.

A COORDINATE OFF THE PLANET, for the same reason: the weather service answers
something for every pair of numbers it is given, so an unchecked longitude is a
temperature from somewhere nobody asked about.

AND A PLACE THAT SHOWS NOTHING. Four switches all off is a place that occupies
a name's worth of the line and says nothing — which reads as a bug, not as a
choice.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

# The four things a place can show. The order is the order they are drawn in,
# and it is also the order they are DROPPED in when the line does not fit —
# see `priority` below.
SHOWS = ["clock", "date", "weather", "rate"]

# The lives a band may have, from `ronoctua_notify::presentation`. Named rather
# than a number so the two cannot drift: a glance that outlived a critical
# notification would be a glance nobody asked to keep.
LIVES = ["quick", "normal", "long"]

# ISO 4217 is three capitals. Not a list of the world's currencies — that is a
# table that ages — but the SHAPE, which is what catches `usd`, `R$` and `eur `.
CURRENCY = re.compile(r"^[A-Z]{3}$")

# An IANA zone name, as `zone1970.tab` writes it.
ZONE = re.compile(r"^[A-Za-z][A-Za-z0-9_+-]*(?:/[A-Za-z0-9_+-]+)+$")


def zones_of(tab: pathlib.Path) -> set[str]:
    """Every zone `zone1970.tab` names, from its third column."""
    found: set[str] = set()
    for line in tab.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) >= 3:
            # The column may hold several, comma-separated.
            found.update(p.strip() for p in parts[2].split(",") if p.strip())
    return found


def check(place: dict, where: str, zones: set[str], gaps: list[str]) -> None:
    zone = place.get("zone", "")
    if not ZONE.match(zone):
        gaps.append(f"{where}: `{zone}` is not shaped like an IANA zone name")
    elif zone not in zones:
        gaps.append(
            f"{where}: `{zone}` is not a zone `zone1970.tab` knows.\n"
            f"        A clock would still be drawn — in UTC, under that name — "
            f"because\n        reading a zone never fails, it falls back."
        )

    at = place.get("at", {})
    lat, lon = at.get("latitude"), at.get("longitude")
    if not isinstance(lat, (int, float)) or not -90 <= lat <= 90:
        gaps.append(f"{where}: latitude `{lat}` is not a latitude")
    if not isinstance(lon, (int, float)) or not -180 <= lon <= 180:
        gaps.append(f"{where}: longitude `{lon}` is not a longitude")

    shows = place.get("shows", {})
    unknown = sorted(set(shows) - set(SHOWS))
    if unknown:
        gaps.append(f"{where}: `shows` has {unknown}, and the four are {SHOWS}")
    if not any(shows.get(s) for s in SHOWS):
        gaps.append(
            f"{where}: shows nothing at all. A place with all four off takes a\n"
            f"        name's worth of the line and says nothing, which reads as a\n"
            f"        fault rather than as a choice — leave it out instead."
        )

    if shows.get("rate"):
        for field in ("currency", "rate-in"):
            value = place.get(field, "")
            if not CURRENCY.match(str(value)):
                gaps.append(f"{where}: `{field}` is `{value}`, and ISO 4217 is three capitals")
        if place.get("currency") == place.get("rate-in"):
            gaps.append(
                f"{where}: asks what one {place.get('currency')} is worth in "
                f"{place.get('rate-in')}, and the answer is one."
            )


def key_of(zone: str) -> str:
    """The text key a zone's name is drawn from.

    `Europe/London` -> `world-place-europe-london`. A key is
    `[a-z][a-z0-9-]*`, which a zone name is not — it has a slash and capitals.
    """
    return "world-place-" + zone.lower().replace("/", "-").replace("_", "-")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--world", required=True)
    ap.add_argument("--zone-tab", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--out-keys", required=True)
    args = ap.parse_args()

    world = json.loads(pathlib.Path(args.world).read_text(encoding="utf-8"))
    zones = zones_of(pathlib.Path(args.zone_tab))
    if not zones:
        print("ERROR: zone1970.tab yielded no zones; its shape changed.", file=sys.stderr)
        return 1

    places = [world["primary"], *world["also"]]
    gaps: list[str] = []
    for n, place in enumerate(places):
        check(place, "primary" if n == 0 else f"also[{n - 1}]", zones, gaps)

    if world.get("life") not in LIVES:
        gaps.append(f"life: `{world.get('life')}` is not one of {LIVES}")

    seen = [p.get("zone") for p in places]
    if len(set(seen)) != len(seen):
        gaps.append(f"the same zone is named twice: {seen}")

    if gaps:
        print("ERROR: assets/world.nix does not hold up.", file=sys.stderr)
        for gap in gaps:
            print(f"  - {gap}", file=sys.stderr)
        return 1

    lines: list[str] = []
    w = lines.append
    w("// GENERATED by nix/scripts/gen-world.py from assets/world.nix. Do not edit.")
    w("//")
    w("// The places `LEADER d` glances at, and what each of them shows.")
    w("")
    w("/// One place, and which of the four things it draws.")
    w("#[derive(Debug, Clone, Copy, PartialEq)]")
    w("pub struct Place {")
    w("    /// The IANA zone, checked against `zone1970.tab` at build time.")
    w("    pub zone: &'static str,")
    w("    /// The text key its name is drawn from.")
    w("    pub key: &'static str,")
    w("    /// Where it is, for the weather.")
    w("    pub latitude: f64,")
    w("    /// Where it is, for the weather.")
    w("    pub longitude: f64,")
    w("    /// What one of this is worth, in [`Place::rate_in`].")
    w("    pub currency: &'static str,")
    w("    /// And in what.")
    w("    pub rate_in: &'static str,")
    for s in SHOWS:
        w(f"    /// Draw the {s}?")
        w(f"    pub {s}: bool,")
    w("}")
    w("")
    w("/// The places, primary first. The order is the order they are drawn.")
    w(f"pub const PLACES: &[Place] = &[")
    for place in places:
        shows = place.get("shows", {})
        at = place["at"]
        w("    Place {")
        w(f"        zone: \"{place['zone']}\",")
        w(f"        key: \"{key_of(place['zone'])}\",")
        w(f"        latitude: {float(at['latitude'])}_f64,")
        w(f"        longitude: {float(at['longitude'])}_f64,")
        w(f"        currency: \"{place.get('currency', '')}\",")
        w(f"        rate_in: \"{place.get('rate-in', '')}\",")
        for s in SHOWS:
            w(f"        {s}: {str(bool(shows.get(s))).lower()},")
        w("    },")
    w("];")
    w("")
    w("/// How long the line stays, by the name the notifications use.")
    w(f"pub const LIFE: &str = \"{world['life']}\";")
    w("")
    for what, source in sorted(world["sources"].items()):
        w(f"/// The host asked for the {what}.")
        w(f"pub const {what.upper()}_HOST: &str = \"{source['host']}\";")
        w(f"/// How often the {what} is asked for, in minutes.")
        w(f"pub const {what.upper()}_EVERY_MINUTES: u64 = {int(source['every-minutes'])};")
        w("")

    pathlib.Path(args.out).write_text("\n".join(lines), encoding="utf-8")

    keys = sorted({key_of(p["zone"]) for p in places})
    pathlib.Path(args.out_keys).write_text("\n".join(keys) + "\n", encoding="utf-8")
    print(f"OK: {len(places)} places, {len(keys)} text keys, life `{world['life']}`")
    return 0


if __name__ == "__main__":
    sys.exit(main())
