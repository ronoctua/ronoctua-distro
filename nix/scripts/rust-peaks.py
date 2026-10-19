#!/usr/bin/env python3
"""Each unit of the Rust build against its class, by what it measured.

    rust-peaks [--json] [--top N] [--write FILE] FACTS

FACTS is what the flake knows at evaluation (`nix run .#rust-peaks` writes
it): every unit of `legacyPackages.<system>.rustUnits` with the path of its
measurement, the class each unit was measured into (`nix/rust/unit-classes.
json`, read raw, so this runs when that file is stale), the class every
planned unit is built by and how it was found (`resolved` and `how`, from
`classesOf` in `nix/build-sizing.nix`: `named`, `carried` from the unit a
lock change renamed, or `default`) and the sizing's numbers (the slot of
each class, the headroom, the class of a unit the data does not name). A
unit's measurement is its derivation's `metrics` output — `peak-kib`, the
largest child's `ru_maxrss` (a link's linker counts), and `seconds` — read
from the store when the unit has been built here; one never built is
unmeasured, said so, never counted as light.

It fails (exit 1) when a unit that is not heavy does not fit the class it is
built by: its measured peak and the headroom more past that class's slot —
the same rule `--write` puts a unit in its class by, so a unit is never
written into a class this then refuses, nor kept in one with no headroom
left. The batteries, `.#sized-build` and a machine's apply start it beside
others by that slot, and that is the shape that has a compiler killed with
no message. The fix is to measure again into the data (`--write`).

`--write FILE` writes the classes as measured: every unit this store
measured goes to the first class whose slot holds its peak and the headroom
more (heavy past the last); a unit not measured here keeps the class it is
built by — its own, or the one it carried from the unit a lock change
renamed —; a unit the index no longer has is dropped. One line per unit,
sorted, so a change reads as the units that moved.

WRITTEN FOR OTHER NUMBERS (the slots, the headroom or the format changed),
a class FILE gave a unit is of numbers that are gone, so `--write` keeps no
class it did not measure here: such a unit is dropped from FILE, and built
as the class of a unit no measurement names until it is measured; a heavy
one stays heavy (built alone, it is never past a slot). It says how many.

A HEAVY UNIT STAYS HEAVY. It is built alone with the heavy cores, and that
is what its measurement is of: the footer's release library peaked at 2279
MiB built at one core beside others and at 1534 MiB alone at twelve. Moved
to a lighter class by its twelve-core peak, it would be built at one core
and peak past that class's slot — and come back. So a heavy unit measured
under a lighter class is said ("could leave heavy"), never moved; leaving is
a measurement of it at one core and a hand edit of FILE.
"""

import argparse
import json
import os
import sys

MIB = 1024
FORMAT = 1


def measure(path):
    """(peak KiB, seconds) of a `metrics` output, or None when there is none
    (or it does not read)."""
    if not path:
        return None
    try:
        with open(os.path.join(path, "peak-kib")) as f:
            peak = int(f.read().strip())
        with open(os.path.join(path, "seconds")) as f:
            seconds = float(f.read().strip())
    except (OSError, ValueError):
        return None
    return peak, seconds


def fits(peak_kib, name, facts):
    """Whether the slot of class `name` holds this peak and the headroom
    more: the one rule `--write` classes by and the report judges by."""
    return peak_kib * (100 + facts["headroomPercent"]) <= facts["slots"][name] * MIB * 100


def class_by(peak_kib, facts):
    """The first class whose slot holds this peak and the headroom more."""
    for name in facts["order"]:
        if fits(peak_kib, name, facts):
            return name
    return "heavy"


def report(facts):
    """Everything the text and the JSON say, as data."""
    kinds = set(facts["kinds"])
    units = {uid: u for uid, u in facts["units"].items() if u["kind"] in kinds}
    data = facts["classes"]
    committed = data.get("units", {})
    slots = {name: facts["slots"][name] * MIB for name in facts["order"]}
    unclassed = facts["unclassed"]
    measured = {}
    for uid, unit in units.items():
        got = measure(unit.get("metrics"))
        if got:
            measured[uid] = {"peak-kib": got[0], "seconds": got[1]}

    known = set(facts["order"]) | {"heavy"}
    resolved = facts["resolved"]
    how = facts["how"]

    def class_of(uid):
        """The class the unit is built by (`classesOf`), or the class of a
        unit no measurement names when that is none of these numbers'."""
        c = resolved.get(uid, unclassed)
        return c if c in known else unclassed

    past = sorted(
        (
            uid
            for uid, m in measured.items()
            if class_of(uid) != "heavy" and not fits(m["peak-kib"], class_of(uid), facts)
        ),
        key=lambda u: -measured[u]["peak-kib"],
    )
    moves = {
        uid: {"from": committed.get(uid), "to": class_by(m["peak-kib"], facts)}
        for uid, m in measured.items()
        if class_by(m["peak-kib"], facts) != committed.get(uid) and class_of(uid) != "heavy"
    }
    could_leave = sorted(
        uid for uid, m in measured.items() if class_of(uid) == "heavy" and class_by(m["peak-kib"], facts) != "heavy"
    )
    by_class = {}
    for name in list(facts["order"]) + ["heavy"]:
        ids = [uid for uid in units if class_of(uid) == name]
        peaks = [measured[u]["peak-kib"] for u in ids if u in measured]
        by_class[name] = {
            "units": len(ids),
            "measured": len(peaks),
            "heaviest-kib": max(peaks, default=0),
            "slot-kib": slots.get(name),
        }
    heavy = sorted(uid for uid in units if class_of(uid) == "heavy")
    stale = (
        data.get("format") != FORMAT
        or data.get("slots") != facts["slots"]
        or data.get("headroomPercent") != facts["headroomPercent"]
    )
    return {
        "units": len(units),
        "measured": measured,
        "classes": by_class,
        "past-the-slot": past,
        "would-move": moves,
        "could-leave-heavy": could_leave,
        "unclassed": sorted(uid for uid in units if how.get(uid, "default") == "default"),
        "carried": sorted(uid for uid in units if how.get(uid) == "carried"),
        "gone": sorted(uid for uid in committed if uid not in units),
        "heavy": {uid: measured.get(uid, {}) for uid in heavy},
        "heavy-past-the-plan": sorted(
            uid for uid in heavy if uid in measured and measured[uid]["peak-kib"] > facts["shellKiB"]
        ),
        "written-for-other-numbers": stale,
        "heavy-kib": facts["shellKiB"],
        "beside-kib": facts["besideKiB"],
        "unclassed-as": unclassed,
    }


def text(r, top):
    lines = []
    lines.append(
        f"measured: {len(r['measured'])} of {r['units']} units "
        "(the rest were never built in this store, or not since they last changed)"
    )
    for name, c in r["classes"].items():
        if c["slot-kib"] is None:
            lines.append(
                f"  heavy: {c['units']} units, alone, planned at {r['heavy-kib'] // MIB} MiB "
                f"and {r['beside-kib'] // MIB} MiB a core more"
            )
        else:
            lines.append(
                f"  {name}: {c['units']} units, slot {c['slot-kib'] // MIB} MiB, "
                f"the heaviest of the {c['measured']} measured {c['heaviest-kib'] // MIB} MiB"
            )
    lines.append("")
    lines.append(f"the {min(top, len(r['measured']))} heaviest measured:")
    for uid in sorted(r["measured"], key=lambda u: -r["measured"][u]["peak-kib"])[:top]:
        m = r["measured"][uid]
        mark = "   heavy" if uid in r["heavy"] else ""
        lines.append(f"  {m['peak-kib'] // MIB:>6} MiB {m['seconds']:>8.1f} s  {uid}{mark}")
    lines.append("")
    lines.append("the heavy units:")
    for uid, m in sorted(r["heavy"].items()):
        said = f"{m['peak-kib'] // MIB} MiB, {m['seconds']:.1f} s" if m else "not measured"
        lines.append(f"  {uid}: {said}")
    for uid in r["heavy-past-the-plan"]:
        lines.append(
            f"note: {uid} measured past {r['heavy-kib'] // MIB} MiB, what a heavy unit is planned at "
            "alone: at one core that is too little (shellMiB in nix/build-sizing.nix); with more it may not be"
        )
    if r["would-move"]:
        lines.append(
            f"note: {len(r['would-move'])} unit(s) measured into another class than nix/rust/unit-classes.json "
            "says (`--write` moves them):"
        )
        for uid, m in sorted(r["would-move"].items())[:top]:
            lines.append(f"  {uid}: {m['from'] or 'unclassed'} -> {m['to']}")
    for uid in r["could-leave-heavy"]:
        lines.append(
            f"note: {uid} is heavy and measured under a lighter class's slot, alone at its heavy cores: "
            "`--write` keeps it heavy (at one core it may peak past that slot); measure it at one core to move it"
        )
    if r["unclassed"]:
        lines.append(
            f"note: {len(r['unclassed'])} unit(s) have no class in nix/rust/unit-classes.json and are "
            f"built as {r['unclassed-as']} until it says one"
        )
    if r["carried"]:
        lines.append(
            f"note: {len(r['carried'])} unit(s) a lock change renamed carry the class of the unit they "
            "replaced (the same crate and version on other dependencies) until they are measured"
        )
    if r["gone"]:
        lines.append(f"note: {len(r['gone'])} unit(s) of nix/rust/unit-classes.json are no unit any more")
    if r["written-for-other-numbers"]:
        lines.append(
            "note: nix/rust/unit-classes.json was written for other numbers than nix/build-sizing.nix's: "
            "no evaluation reads it until `--write` writes it again"
        )
    return "\n".join(lines)


def written(facts, r, old):
    """The new data, as text: one unit a line, sorted; and the units of FILE
    dropped because it was written for other numbers and this store did not
    measure them."""
    committed = old.get("units", {})
    stale = r["written-for-other-numbers"]
    known = set(facts["order"]) | {"heavy"}
    resolved, how = facts["resolved"], facts["how"]
    units, unmeasured = {}, []
    kept = {u for u in committed if u not in r["gone"]}
    carried = {u for u in r["carried"]}
    for uid in sorted(set(r["measured"]) | kept | carried):
        if uid in r["measured"]:
            units[uid] = "heavy" if committed.get(uid) == "heavy" else class_by(r["measured"][uid]["peak-kib"], facts)
        elif stale:
            # A class of other numbers is none: kept only when it is heavy.
            if committed.get(uid) == "heavy":
                units[uid] = "heavy"
            elif uid in committed:
                unmeasured.append(uid)
        elif how.get(uid) == "carried" and resolved.get(uid) in known:
            units[uid] = resolved[uid]
        elif uid in committed:
            units[uid] = committed[uid]
    head = [
        "{",
        f'"about":{json.dumps("GENERATED by `nix run .#rust-peaks -- --write` from the units measured peaks (metrics/peak-kib): the class of each unit (a unit a lock change renamed, not measured since, with the class of the unit it replaced), read by nix/build-sizing.nix")},',
        f'"format":{FORMAT},',
        f'"headroomPercent":{facts["headroomPercent"]},',
        f'"slots":{json.dumps(facts["slots"], sort_keys=True, separators=(",", ":"))},',
        '"units":{',
    ]
    body = [f"{json.dumps(uid)}:{json.dumps(c)}" for uid, c in units.items()]
    return "\n".join(head) + "\n" + ",\n".join(body) + "\n}\n}\n", units, unmeasured


def main(argv=None):
    parser = argparse.ArgumentParser(description="each unit of the Rust build against its class")
    parser.add_argument("facts", help="the JSON the flake writes")
    parser.add_argument("--json", action="store_true", help="print the report as JSON")
    parser.add_argument("--top", type=int, default=15, help="how many of the heaviest to list (default 15)")
    parser.add_argument("--write", metavar="FILE", help="write the classes as measured into FILE")
    args = parser.parse_args(argv)
    with open(args.facts) as f:
        facts = json.load(f)
    r = report(facts)
    if args.write:
        new, units, unmeasured = written(facts, r, facts["classes"])
        old = facts["classes"].get("units", {})
        moved = sum(1 for u, c in units.items() if u in old and old[u] != c)
        added = sum(1 for u in units if u not in old)
        dropped = sum(1 for u in old if u not in units)
        with open(args.write + ".new", "w") as f:
            f.write(new)
        os.replace(args.write + ".new", args.write)
        counts = {c: sum(1 for v in units.values() if v == c) for c in list(facts["order"]) + ["heavy"]}
        print(
            f"{args.write}: {len(units)} units ("
            + ", ".join(f"{n} {c}" for c, n in counts.items())
            + f"); {moved} moved, {added} new, {dropped} dropped"
        )
        if unmeasured:
            print(
                f"{args.write}: was written for other numbers than nix/build-sizing.nix's, so the "
                f"{len(unmeasured)} unit(s) this store did not measure are dropped from it and built as "
                f"{facts['unclassed']} until measured (the heavy ones stay heavy)"
            )
        return 0
    if args.json:
        print(json.dumps(r, indent=2, sort_keys=True))
    else:
        print(text(r, args.top))
    for uid in r["past-the-slot"]:
        c = facts["resolved"].get(uid, facts["unclassed"])
        if c not in facts["slots"]:
            c = facts["unclassed"]
        print(
            f"ERROR: {uid} measured {r['measured'][uid]['peak-kib'] // MIB} MiB, which with "
            f"{facts['headroomPercent']}% more is past the slot of the class it is built by "
            f"({c}, {facts['slots'][c]} MiB): run `nix run .#rust-peaks -- --write` to measure it into "
            "nix/rust/unit-classes.json",
            file=sys.stderr,
        )
    return 1 if r["past-the-slot"] else 0


if __name__ == "__main__":
    sys.exit(main())
