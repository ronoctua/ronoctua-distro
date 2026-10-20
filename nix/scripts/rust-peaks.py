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
unit's measurement is its derivation's `metrics` output, read from the store
when the unit has been built here (one never built is unmeasured, said so,
never counted as light): `peak-kib`, the whole build's peak of what the
budget pays for — the anonymous memory, shmem, swap and page tables of the
builder and every process below it, sampled (`nix/scripts/rust-unit.py`) —,
`largest-kib`, the most one of its processes held, `cores`, what it was
built with, and `seconds`, in format `METRICS_FORMAT`. A measurement of
another format is of another quantity (format 1 was the largest child's RSS,
with no cores; 2, the PSS of the build's processes, whose file pages it
split with the builds beside it): it is refused, with the units named, and
nothing is written.

A MEASUREMENT CLASSES A UNIT ONLY AT THE CORES ITS CLASS BUILDS IT WITH. A
peak depends on the cores — the RSS of the footer's release library peaked at
2279 MiB at one core and 1534 at twelve —, and every class with a slot is
built at one core (`sized_units`), so a unit is judged against a slot, and
put in one, only by a measurement at one core. A heavy unit is built alone with the
cores the budget leaves, and a measurement at those keeps it heavy; one at
one core classes it like any other, and it leaves heavy when a slot holds
it. A unit of a slot class measured at other cores (built by a `nix build`
that was not sized) is neither judged nor moved, and is said by name: it
keeps the class the data gives it until its next sized build — the next
time what it is built from changes — measures it at one core. No command
here measures it sooner: deleting a measurement is refused while a root
holds it, and the roots of `nix/keep-units.nix` hold the units a raw build
most often builds (the release graph's, and the loop's third-party ones).

It fails (exit 1) when a unit that is not heavy, measured at one core, is
past the slot of the class it is built by. The batteries, `.#sized-build`
and a machine's apply start it beside others by that slot, and that is the
shape that has a compiler killed with no message. The fix is to measure
again into the data (`--write`).

THE HEADROOM IS THE MARGIN BETWEEN TWO MEASUREMENTS. `--write` puts a unit
in the first class whose slot holds its peak and the headroom more; this
judges it by the slot itself. So a measurement that wrote a class and the
next one of the same unit, which judges it, may differ by the headroom
before this fails — and the measure is chosen so that two builds of one
unit, alone or beside others, differ by less (docs/ARCHITECTURE.md, section
"O pico é do build inteiro (M1)"). A unit inside its slot but past it with
the headroom fits, and is said: `--write` moves it.

`--write FILE` writes the classes as measured: every unit this store
measured at one core goes to the first class whose slot holds its peak and
the headroom more (heavy past the last), a heavy unit included; a unit not
measured here, or measured at other cores, keeps the class it is built by —
its own, or the one it carried from the unit a lock change renamed —; a
unit the index no longer has is dropped. One line per unit, sorted, so a
change reads as the units that moved.

WRITTEN FOR OTHER NUMBERS (the slots, the headroom or the format changed),
a class FILE gave a unit is of numbers that are gone, so a unit `--write`
did not measure here at one core keeps a slot at least as large as the one
it had: the first class whose slot now holds the old one (heavy past the
last). Lighter than the class of a unit no measurement names, it is dropped
from FILE and built as that class until it is measured: never in a slot
smaller than the one it was written for. It says how many of each.
"""

import argparse
import json
import os
import sys

MIB = 1024
# The format of `nix/rust/unit-classes.json`: 2 is classes by the whole
# build's peak at the cores the class builds with.
FORMAT = 2
# The format of a unit's `metrics` (`METRICS_FORMAT` of
# `nix/scripts/rust-unit.py`): 3 is what the build holds that the budget
# pays for.
METRICS_FORMAT = 3
# The cores every unit of a class with a slot is built with: one
# (`sized_units` in `nix/build-sizing.nix`: "every other unit gets one core").
SLOT_CORES = 1


def measure(path):
    """A unit's measurement — `peak-kib`, `largest-kib`, `seconds`, `cores`
    and its `format` —, None when there is none (the unit was never built
    here, or not since it last changed), or only `format` when it is of
    another format than `METRICS_FORMAT` (1 when it says none: what the
    driver wrote before it said one) or does not read."""
    if not path or not os.path.isdir(path):
        return None

    def read(name):
        with open(os.path.join(path, name)) as f:
            return f.read().strip()

    try:
        fmt = int(read("format")) if os.path.exists(os.path.join(path, "format")) else 1
    except (OSError, ValueError):
        return {"format": "unreadable"}
    if fmt != METRICS_FORMAT:
        return {"format": fmt}
    try:
        return {
            "peak-kib": int(read("peak-kib")),
            "largest-kib": int(read("largest-kib")),
            "seconds": float(read("seconds")),
            "cores": int(read("cores")),
            "format": fmt,
        }
    except (OSError, ValueError):
        return {"format": "unreadable"}


def fits(peak_kib, name, facts):
    """Whether the slot of class `name` holds this peak and the headroom
    more: the rule `--write` classes by."""
    return peak_kib * (100 + facts["headroomPercent"]) <= facts["slots"][name] * MIB * 100


def inside(peak_kib, name, facts):
    """Whether this peak is inside the slot of class `name`: the rule a unit
    is judged by, the headroom below `fits` its margin."""
    return peak_kib <= facts["slots"][name] * MIB


def class_by(peak_kib, facts):
    """The first class whose slot holds this peak and the headroom more."""
    for name in facts["order"]:
        if fits(peak_kib, name, facts):
            return name
    return "heavy"


def measured_class(m, facts):
    """The class a measurement puts its unit in: by its peak when it was
    taken at the cores a class with a slot builds with; none at other cores,
    whose peak says nothing of one core's."""
    return class_by(m["peak-kib"], facts) if m["cores"] == SLOT_CORES else None


def heavy_plan_kib(cores, facts):
    """What a heavy unit is planned at with these cores: `alone:` with the
    heavy numbers, as `sized_units` charges it."""
    return facts["shellKiB"] + (cores - 1) * facts["besideKiB"]


def report(facts):
    """Everything the text and the JSON say, as data."""
    kinds = set(facts["kinds"])
    units = {uid: u for uid, u in facts["units"].items() if u["kind"] in kinds}
    data = facts["classes"]
    committed = data.get("units", {})
    slots = {name: facts["slots"][name] * MIB for name in facts["order"]}
    unclassed = facts["unclassed"]
    measured, other_format = {}, {}
    for uid, unit in units.items():
        got = measure(unit.get("metrics"))
        if got is None:
            continue
        if got["format"] != METRICS_FORMAT:
            other_format[uid] = got["format"]
        else:
            measured[uid] = got

    known = set(facts["order"]) | {"heavy"}
    resolved = facts["resolved"]
    how = facts["how"]

    def class_of(uid):
        """The class the unit is built by (`classesOf`), or the class of a
        unit no measurement names when that is none of these numbers'."""
        c = resolved.get(uid, unclassed)
        return c if c in known else unclassed

    at_one = {uid for uid, m in measured.items() if m["cores"] == SLOT_CORES}
    judged = sorted(uid for uid in at_one if class_of(uid) != "heavy")
    past = sorted(
        (uid for uid in judged if not inside(measured[uid]["peak-kib"], class_of(uid), facts)),
        key=lambda u: -measured[u]["peak-kib"],
    )
    within = sorted(
        uid
        for uid in judged
        if uid not in past and not fits(measured[uid]["peak-kib"], class_of(uid), facts)
    )
    moves = {}
    for uid, m in measured.items():
        to = measured_class(m, facts)
        if to is not None and to != committed.get(uid):
            moves[uid] = {"from": committed.get(uid), "to": to}
    other_cores = sorted(uid for uid in measured if uid not in at_one and class_of(uid) != "heavy")
    could_leave = sorted(
        uid
        for uid, m in measured.items()
        if uid not in at_one and class_of(uid) == "heavy" and class_by(m["peak-kib"], facts) != "heavy"
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
        "judged": len(judged),
        "classes": by_class,
        "past-the-slot": past,
        "within-the-headroom": within,
        "would-move": moves,
        "could-leave-heavy": could_leave,
        "other-cores": other_cores,
        "other-cores-said": other_cores_said(other_cores),
        "other-format": other_format,
        "unclassed": sorted(uid for uid in units if how.get(uid, "default") == "default"),
        "carried": sorted(uid for uid in units if how.get(uid) == "carried"),
        "gone": sorted(uid for uid in committed if uid not in units),
        "heavy": {uid: measured.get(uid, {}) for uid in heavy},
        "heavy-past-the-plan": sorted(
            uid
            for uid in heavy
            if uid in measured and measured[uid]["peak-kib"] > heavy_plan_kib(measured[uid]["cores"], facts)
        ),
        "written-for-other-numbers": stale,
        "heavy-kib": facts["shellKiB"],
        "beside-kib": facts["besideKiB"],
        "slot-cores": SLOT_CORES,
        "unclassed-as": unclassed,
    }


def text(r, top):
    lines = []
    lines.append(
        f"measured: {len(r['measured'])} of {r['units']} units "
        "(the rest were never built in this store, or not since they last changed); "
        f"{r['judged']} of them at the {r['slot-cores']} core a class with a slot builds with, and judged by it"
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
    lines.append(f"the {min(top, len(r['measured']))} heaviest measured (the whole build's peak, and its cores):")
    for uid in sorted(r["measured"], key=lambda u: -r["measured"][u]["peak-kib"])[:top]:
        m = r["measured"][uid]
        mark = "   heavy" if uid in r["heavy"] else ""
        lines.append(f"  {m['peak-kib'] // MIB:>6} MiB {m['cores']:>3} core(s) {m['seconds']:>8.1f} s  {uid}{mark}")
    lines.append("")
    lines.append("the heavy units:")
    for uid, m in sorted(r["heavy"].items()):
        said = f"{m['peak-kib'] // MIB} MiB at {m['cores']} core(s), {m['seconds']:.1f} s" if m else "not measured"
        lines.append(f"  {uid}: {said}")
    for uid in r["heavy-past-the-plan"]:
        m = r["measured"][uid]
        lines.append(
            f"note: {uid} measured {m['peak-kib'] // MIB} MiB at {m['cores']} core(s), past what a heavy unit is "
            f"planned at alone with those ({r['heavy-kib'] // MIB} MiB and {r['beside-kib'] // MIB} MiB a core more: "
            "shellMiB and besideMiB in nix/build-sizing.nix)"
        )
    if r["would-move"]:
        lines.append(
            f"note: {len(r['would-move'])} unit(s) measured at one core into another class than "
            "nix/rust/unit-classes.json says (`--write` moves them):"
        )
        for uid, m in sorted(r["would-move"].items())[:top]:
            lines.append(f"  {uid}: {m['from'] or 'unclassed'} -> {m['to']}")
    for uid in r["could-leave-heavy"]:
        m = r["measured"][uid]
        lines.append(
            f"note: {uid} is heavy and measured under a lighter class's slot alone at {m['cores']} cores: "
            "`--write` keeps it heavy, since at one core it may peak past that slot (the RSS of the footer's "
            "release library: 1534 MiB at twelve cores, 2279 at one). To move it, name it in that class in "
            "nix/rust/unit-classes.json by hand: its next build is at one core, and `--write` classes it by that"
        )
    if r["within-the-headroom"]:
        lines.append(
            f"note: {len(r['within-the-headroom'])} unit(s) are inside the slot of their class but past it "
            "with the headroom (`--write` moves them), e.g. " + ", ".join(r["within-the-headroom"][:3])
        )
    if r["other-cores-said"]:
        lines.append(f"note: {r['other-cores-said']}")
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


def other_cores_said(other, shown=10):
    """The units of a class with a slot measured at other cores than one, as
    one sentence — how many, and by name —, or None when there is none.
    It gives no command: the store refuses to delete what a root holds, and
    the roots of `nix/keep-units.nix` (on a checkout and on a machine) hold
    the release graph's units and the loop's third-party ones, so their next
    sized build measures them again, the next time they change."""
    if not other:
        return None
    names = ", ".join(other[:shown])
    if len(other) > shown:
        names += f" and {len(other) - shown} more (`--json` lists them all)"
    return (
        f"{len(other)} unit(s) of a class with a slot were measured at other cores than the {SLOT_CORES} it "
        "builds them with — by a `nix build` that was not sized (`nix flake check`, `.#vm`, `.#iso`, `.#png`, a "
        f"`nix build` of its own), at the cores of nix.conf: {names}. Neither judged nor moved, each keeps the "
        "class nix/rust/unit-classes.json gives it until its next sized build (`.#sized-build`, a battery, a "
        "machine's apply), the next time what it is built from changes, measures it at one core (no command "
        "here measures it sooner: deleting a measurement is refused while a root holds it, and the roots of "
        "nix/keep-units.nix hold the release graph's units and the loop's third-party ones)"
    )


def other_format_said(r):
    """The refusal of measurements of another format, as one sentence, or
    None when there is none."""
    if not r["other-format"]:
        return None
    shown = ", ".join(f"{u} (format {f})" for u, f in sorted(r["other-format"].items())[:3])
    return (
        f"{len(r['other-format'])} unit(s) hold a measurement of another format than {METRICS_FORMAT}, "
        f"which this does not read as one: {shown}. Format 1 is the largest process's RSS, with no "
        "cores; 2, the PSS of the build's processes, whose file pages it split with the builds beside it. "
        "Rebuild them with the driver of this tree (nix/scripts/rust-unit.py)"
    )


def carried_over(name, old_slots, facts):
    """The class a unit of FILE keeps, not measured here at one core, when
    FILE was written for other numbers: the first class whose slot now holds
    the slot `name` had there (heavy past the last, and heavy stays heavy;
    a class FILE gives no slot, heavy) — or None when that is no heavier
    than the class of a unit no measurement names, which it is built as."""
    rank = {c: i for i, c in enumerate(list(facts["order"]) + ["heavy"])}
    old = old_slots.get(name) if isinstance(old_slots, dict) else None
    if name == "heavy" or not isinstance(old, int):
        to = "heavy"
    else:
        to = next((c for c in facts["order"] if facts["slots"][c] >= old), "heavy")
    return to if rank[to] > rank[facts["unclassed"]] else None


def written(facts, r, old):
    """The new data, as text: one unit a line, sorted; the units of FILE that
    kept a slot at least as large as the one they were written for, because
    FILE was written for other numbers and this store did not measure them
    at one core; and those it dropped, built as the class of a unit no
    measurement names, which is no lighter."""
    committed = old.get("units", {})
    stale = r["written-for-other-numbers"]
    known = set(facts["order"]) | {"heavy"}
    resolved, how = facts["resolved"], facts["how"]
    units, kept_over, unmeasured = {}, [], []
    kept = {u for u in committed if u not in r["gone"]}
    carried = {u for u in r["carried"]}
    for uid in sorted(set(r["measured"]) | kept | carried):
        m = r["measured"].get(uid)
        by = measured_class(m, facts) if m else None
        if by is not None:
            units[uid] = by
        elif stale:
            # A class of other numbers is none of these: the unit keeps a
            # slot no smaller than the one it was written for.
            if uid in committed:
                to = carried_over(committed[uid], old.get("slots"), facts)
                if to is None:
                    unmeasured.append(uid)
                else:
                    units[uid] = to
                    kept_over.append(uid)
        elif how.get(uid) == "carried" and resolved.get(uid) in known:
            units[uid] = resolved[uid]
        elif uid in committed:
            units[uid] = committed[uid]
    head = [
        "{",
        f'"about":{json.dumps("GENERATED by `nix run .#rust-peaks -- --write` from the units measured peaks (metrics/peak-kib: what the whole build held that the budget pays for, at one core): the class of each unit (a unit a lock change renamed, not measured since, with the class of the unit it replaced; a heavy unit measured only at its own cores, heavy), read by nix/build-sizing.nix")},',
        f'"format":{FORMAT},',
        f'"headroomPercent":{facts["headroomPercent"]},',
        f'"slots":{json.dumps(facts["slots"], sort_keys=True, separators=(",", ":"))},',
        '"units":{',
    ]
    body = [f"{json.dumps(uid)}:{json.dumps(c)}" for uid, c in units.items()]
    return "\n".join(head) + "\n" + ",\n".join(body) + "\n}\n}\n", units, kept_over, unmeasured


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
    refused = other_format_said(r)
    if args.write:
        if refused:
            print(f"ERROR: {refused}: nothing is written", file=sys.stderr)
            return 1
        new, units, kept_over, unmeasured = written(facts, r, facts["classes"])
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
        if kept_over or unmeasured:
            print(
                f"{args.write}: was written for other numbers than nix/build-sizing.nix's: of the units this "
                f"store did not measure at one core, {len(kept_over)} keep the first class whose slot holds the "
                f"one they were written for (the heavy ones stay heavy), and {len(unmeasured)} are dropped from "
                f"it and built as {facts['unclassed']} until measured"
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
            f"ERROR: {uid} measured {r['measured'][uid]['peak-kib'] // MIB} MiB at one core, past the slot of "
            f"the class it is built by ({c}, {facts['slots'][c]} MiB): run `nix run .#rust-peaks -- --write` "
            "to measure it into nix/rust/unit-classes.json",
            file=sys.stderr,
        )
    if refused:
        print(f"ERROR: {refused}", file=sys.stderr)
    return 1 if r["past-the-slot"] or refused else 0


if __name__ == "__main__":
    sys.exit(main())
