#!/usr/bin/env python3
"""What an edit costs the Rust build, before anything is built.

    rebuild-cost [--system S] [--json]                    the working tree against HEAD
    rebuild-cost [--system S] [--json] --against REV      the working tree against REV
    rebuild-cost [--system S] [--json] FILE…              one byte appended to each FILE

The build is one derivation per compilation unit (`nix/rust/units.nix`), so
what a change rebuilds is a property of the build graph: the flake's
`legacyPackages.<system>.rustUnits` names every unit, layer and aggregate
with its derivation, and two evaluations of it — before and after — say
which entries moved. Nothing is built to answer. FILE… is simulated through
the units' own `crateSourceOverrides` (`rustUnitsWith`): every unit whose
sources hold the file gets it one byte longer, as a real edit would.

The time is what each moved entry took the LAST time it was built: its
`metrics/seconds`, read from the store when the old derivation's measurement
is still there — or, failing that, the new derivation's, when it has been
built already (said apart: that is the edited build's own time). An entry
measured by neither is said to be unmeasured, never counted as free. The
sum is the work; the critical path — the longest chain of moved entries,
each waiting on the ones it is built from — is the wait with unlimited
cores.
"""

import argparse
import json
import os
import subprocess
import sys


def die(message):
    print(f"rebuild-cost: {message}", file=sys.stderr)
    sys.exit(1)


# --- evaluating the index --------------------------------------------------------


def nix_eval(ref, attr, apply=None):
    argv = ["nix", "eval", "--json", f"{ref}#{attr}"]
    if apply:
        argv += ["--apply", apply]
    result = subprocess.run(argv, capture_output=True, text=True)
    if result.returncode != 0:
        if "rustUnits" in result.stderr and ("missing" in result.stderr or "does not provide" in result.stderr):
            die(f"{ref} has no {attr}: it predates the build in units (`nix/rust/units.nix`), "
                "and there is nothing to compare with")
        tail = "\n".join(result.stderr.strip().splitlines()[-6:])
        die(f"could not evaluate {ref}#{attr}:\n{tail}")
    return json.loads(result.stdout)


def nix_string(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"').replace("${", "\\${") + '"'


def tree_ref(root):
    return f"git+file://{root}"


def rev_ref(root, rev):
    """A revision of this repository as a flake reference; a reference that
    is already one (it has a `:`) is taken as it is."""
    if ":" in rev:
        return rev
    sha = subprocess.run(["git", "-C", root, "rev-parse", "--verify", f"{rev}^{{commit}}"],
                         capture_output=True, text=True)
    if sha.returncode != 0:
        die(f"{rev} is no revision of {root}")
    return f"git+file://{root}?rev={sha.stdout.strip()}"


# --- the comparison ------------------------------------------------------------------


def moved(before, after):
    """The entries an edit rebuilds: every entry whose derivation changed, and
    every entry the new build has and the old did not. (One the old had and
    the new has not is not rebuilt; it is said apart.)"""
    return sorted(k for k in after if before.get(k, {}).get("drv") != after[k]["drv"])


def gone(before, after):
    return sorted(k for k in before if k not in after)


def seconds_of(entry):
    """The measured seconds of an entry's last build, or None."""
    path = (entry or {}).get("metrics")
    if not path:
        return None
    try:
        with open(os.path.join(path, "seconds")) as f:
            return float(f.read().strip())
    except (OSError, ValueError):
        return None


def critical_path(entries, after, seconds):
    """The longest chain of moved entries, each built after the moved ones it
    is built from, weighed by `seconds` (an unmeasured entry weighs nothing,
    and the chain says so). Returns (seconds, [ids, first built first])."""
    moving = set(entries)
    best = {}

    def longest(k, trail=()):
        if k in best:
            return best[k]
        if k in trail:
            die(f"the index has a cycle through {k}")
        below = [(longest(d, trail + (k,)), d) for d in after[k].get("deps", []) if d in moving]
        own = seconds.get(k) or 0.0
        if below:
            (cost, chain), _ = max(below, key=lambda b: (b[0][0], b[1]))
            best[k] = (cost + own, chain + [k])
        else:
            best[k] = (own, [k])
        return best[k]

    result = (0.0, [])
    for k in sorted(moving):
        candidate = longest(k)
        if candidate[0] > result[0] or (candidate[0] == result[0] and len(candidate[1]) > len(result[1])):
            result = candidate
    return result


def report(before, after):
    """Everything the text and the JSON say, as data."""
    changed = moved(before, after)
    seconds = {k: seconds_of(before.get(k)) for k in changed}
    # The edited build's own time, where the old one was never measured.
    after_only = {k for k in changed if seconds[k] is None and seconds_of(after[k]) is not None}
    for k in after_only:
        seconds[k] = seconds_of(after[k])
    by_flavour = {}
    for k in changed:
        e = after[k]
        flavour = e.get("flavour") or "-"
        by_flavour.setdefault(flavour, {}).setdefault(e["kind"], 0)
        by_flavour[flavour][e["kind"]] += 1
    measured = {k: s for k, s in seconds.items() if s is not None}
    unmeasured = sorted(k for k, s in seconds.items() if s is None and after[k]["kind"] != "aggregate")
    cost, chain = critical_path(changed, after, seconds)
    return {
        "entries": len(after),
        "moved": changed,
        "gone": gone(before, after),
        "byFlavour": by_flavour,
        "relinked": sorted(after[k]["name"] for k in changed if after[k]["kind"] == "bins" and after[k].get("flavour") == "release"),
        "relinkedLoop": sorted(after[k]["name"] for k in changed if after[k]["kind"] == "bins" and after[k].get("flavour") == "loop"),
        "rerun": sorted(after[k]["crate"] for k in changed if after[k]["kind"] == "testRuns"),
        "layers": sorted(k for k in changed if after[k]["kind"] == "layer"),
        "seconds": round(sum(measured.values()), 2),
        "measured": len(measured),
        "measuredAfter": sorted(after_only),
        "unmeasured": unmeasured,
        "criticalPath": {"seconds": round(cost, 2), "chain": chain},
    }


def text(r, what):
    out = [f"{what}: {len(r['moved'])} of {r['entries']} entries rebuilt"]
    for flavour in sorted(r["byFlavour"]):
        kinds = r["byFlavour"][flavour]
        out.append(f"  {flavour:8} " + ", ".join(f"{n} {k}" for k, n in sorted(kinds.items())))
    if r["layers"]:
        out.append("  a third-party LAYER moves: " + ", ".join(r["layers"])
                   + " — every unit built on it is rebuilt")
    if r["relinked"]:
        out.append(f"  programs relinked (release): {', '.join(r['relinked'])}")
    if r["relinkedLoop"]:
        out.append(f"  programs relinked (loop): {', '.join(r['relinkedLoop'])}")
    if r["rerun"]:
        out.append(f"  test runs run again: {', '.join(r['rerun'])}")
    if r["gone"]:
        out.append(f"  entries the edit removes: {', '.join(r['gone'])}")
    out.append(f"  work: {r['seconds']} s measured over {r['measured']} entries"
               + (f" ({len(r['measuredAfter'])} by the edited build's own time)" if r["measuredAfter"] else "")
               + (f"; {len(r['unmeasured'])} never measured (built nowhere yet, or collected)" if r["unmeasured"] else ""))
    cp = r["criticalPath"]
    if cp["chain"] and r["measured"]:
        out.append(f"  critical path: {cp['seconds']} s — " + " → ".join(cp["chain"]))
    elif cp["chain"]:
        out.append("  critical path: not measured (no moved entry was ever built) — " + " → ".join(cp["chain"]))
    return "\n".join(out)


def main(argv=None):
    p = argparse.ArgumentParser(prog="rebuild-cost", description=__doc__.split("\n\n")[0])
    p.add_argument("--system", default="x86_64-linux", help="the flake's system")
    p.add_argument("--against", help="a revision (or a flake reference) to compare the working tree with")
    p.add_argument("--json", action="store_true", help="print the report as JSON")
    p.add_argument("files", nargs="*", help="files of the repository to simulate a one-byte edit of")
    a = p.parse_args(argv)
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if root.returncode != 0:
        die("run this inside the repository")
    root = root.stdout.strip()
    attr = f"legacyPackages.{a.system}"
    if a.files and a.against:
        die("give files to edit, or a revision to compare with, not both")
    if a.files:
        files = []
        for f in a.files:
            rel = os.path.relpath(os.path.abspath(f), root)
            if rel.startswith(".."):
                die(f"{f} is outside the repository")
            files.append(rel)
        before = nix_eval(tree_ref(root), f"{attr}.rustUnits")
        after = nix_eval(tree_ref(root), f"{attr}.rustUnitsWith",
                         apply="f: f [ " + " ".join(nix_string(f) for f in files) + " ]")
        what = "one byte appended to " + ", ".join(files)
    else:
        rev = a.against or "HEAD"
        before = nix_eval(rev_ref(root, rev), f"{attr}.rustUnits")
        after = nix_eval(tree_ref(root), f"{attr}.rustUnits")
        what = f"the working tree against {rev}"
    r = report(before, after)
    print(json.dumps(r, indent=1, sort_keys=True) if a.json else text(r, what))


if __name__ == "__main__":
    main()
