#!/usr/bin/env python3
"""Which units of the Rust build peak past the slot, by what they measured.

    rust-peaks [--json] [--top N] FACTS

FACTS is what the flake knows at evaluation (`nix run .#rust-peaks` writes
it): every unit of `legacyPackages.<system>.rustUnits` with the path of its
measurement, which units are heavy (`rust.heavyUnits`) and the sizing's
numbers (`nix/build-sizing.nix`). A unit's measurement is its derivation's
`metrics` output — `peak-kib`, the largest child's `ru_maxrss` (a link's
linker counts), and `seconds` — read from the store when the unit has been
built here; one never built is unmeasured, said so, never counted as light.

It fails (exit 1) when a unit that is not heavy measured past the slot: the
batteries, `.#sized-build` and a machine's apply build it beside others, as
one slot, and that is the shape that has a compiler killed with no message.
A heavy unit that measured under the slot is said (it could leave the list),
and so is one that measured past what a heavy unit is planned at alone.
"""

import argparse
import json
import os
import sys

MIB = 1024


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


def report(facts):
    """Everything the text and the JSON say, as data."""
    units = facts["units"]
    heavy = facts["heavy"]
    heavy_ids = {uid: name for name, uid in heavy.items()}
    slot = facts["peakKiB"]
    planned = facts["shellKiB"]
    measured = {}
    for uid, unit in units.items():
        got = measure(unit.get("metrics"))
        if got:
            measured[uid] = {"peak-kib": got[0], "seconds": got[1]}
    past = sorted(
        (uid for uid, m in measured.items() if m["peak-kib"] > slot and uid not in heavy_ids),
        key=lambda u: -measured[u]["peak-kib"],
    )
    light_heavy = sorted(
        name for name, uid in heavy.items() if uid in measured and measured[uid]["peak-kib"] <= slot
    )
    over_plan = sorted(
        name for name, uid in heavy.items() if uid in measured and measured[uid]["peak-kib"] > planned
    )
    unmeasured_heavy = sorted(name for name, uid in heavy.items() if uid not in measured)
    return {
        "units": len(units),
        "measured": measured,
        "slot-kib": slot,
        "heavy-kib": planned,
        "beside-kib": facts["besideKiB"],
        "heavy": {name: {"unit": uid, **measured.get(uid, {})} for name, uid in heavy.items()},
        "past-the-slot": past,
        "heavy-under-the-slot": light_heavy,
        "heavy-past-the-plan": over_plan,
        "heavy-unmeasured": unmeasured_heavy,
    }


def text(r, top):
    lines = []
    lines.append(
        f"measured: {len(r['measured'])} of {r['units']} units "
        "(the rest were never built in this store, or not since they last changed)"
    )
    lines.append(
        f"the slot: {r['slot-kib'] // MIB} MiB a compilation; a heavy unit is planned at "
        f"{r['heavy-kib'] // MIB} MiB alone and {r['beside-kib'] // MIB} MiB a core more"
    )
    lines.append("")
    lines.append(f"the {min(top, len(r['measured']))} heaviest measured:")
    names = {v["unit"]: k for k, v in r["heavy"].items()}
    for uid in sorted(r["measured"], key=lambda u: -r["measured"][u]["peak-kib"])[:top]:
        m = r["measured"][uid]
        mark = f"   heavy: {names[uid]}" if uid in names else ""
        lines.append(f"  {m['peak-kib'] // MIB:>6} MiB {m['seconds']:>8.1f} s  {uid}{mark}")
    lines.append("")
    lines.append("the heavy units:")
    for name in sorted(r["heavy"]):
        h = r["heavy"][name]
        said = f"{h['peak-kib'] // MIB} MiB, {h['seconds']:.1f} s" if "peak-kib" in h else "not measured"
        lines.append(f"  {name}: {h['unit']}: {said}")
    for name in r["heavy-under-the-slot"]:
        lines.append(f"note: {name} is heavy and measured under the slot: it could leave rust.heavyUnits")
    for name in r["heavy-past-the-plan"]:
        lines.append(
            f"note: {name} measured past {r['heavy-kib'] // MIB} MiB, what a heavy unit is planned at "
            "alone: at one core that is too little (shellMiB in nix/build-sizing.nix); with more it may not be"
        )
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description="which units of the Rust build peak past the slot")
    parser.add_argument("facts", help="the JSON the flake writes")
    parser.add_argument("--json", action="store_true", help="print the report as JSON")
    parser.add_argument("--top", type=int, default=15, help="how many of the heaviest to list (default 15)")
    args = parser.parse_args(argv)
    with open(args.facts) as f:
        facts = json.load(f)
    r = report(facts)
    if args.json:
        print(json.dumps(r, indent=2, sort_keys=True))
    else:
        print(text(r, args.top))
    for uid in r["past-the-slot"]:
        print(
            f"ERROR: {uid} measured {r['measured'][uid]['peak-kib'] // MIB} MiB, past the slot "
            f"({r['slot-kib'] // MIB} MiB), and is not heavy: name it in rust.heavyUnits (nix/rust.nix), "
            "or raise peakMiB (nix/build-sizing.nix) by the measure",
            file=sys.stderr,
        )
    return 1 if r["past-the-slot"] else 0


if __name__ == "__main__":
    sys.exit(main())
