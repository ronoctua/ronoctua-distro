#!/usr/bin/env python3
"""What the belongings keep: `assets/belongings.nix` (as JSON) into Rust.

Writes `belongings.rs` (included by `ronoctua-belongings`'s `kept` module) and
`belongings.json` (the same table, normalised, for the checks).

REFUSES, naming what it refused:
  - a field the table does not know, at the top or in an entry;
  - a path that is not LOGICAL (`config/…`, `state/…`, `home/…`), that climbs
    (`..`), that is absolute, or that has an empty component;
  - a `never` entry that has both a `name` and a `path`, or neither; a `hard`
    one that is a name pattern (a pattern the person cannot override is a
    rule about every folder they will ever add);
  - a name pattern with more than one `*`, or with a `/`;
  - a reason that is not an English kebab id;
  - a census entry whose `kept` is not a boolean;
  - a `largeMB` that is not a positive whole number;
  - a target (`targets.<id>`) whose id is not a kebab id, with a field it
    does not know, no `paths`, a path that is not logical, a `skip` that is
    not a name pattern, a `private` that is not a boolean, a `merge` that is
    not `choose` or `union`, or an offer it does not know;
  - a plain root (`plain.<path>`) that is not logical, with a field it does
    not know, an `at` that is not a plain relative path or a bad reason, two
    roots that overlap (by path or by `at`), or one that overlaps a path
    `never` names;
  - a `chunk`, `budget`, `unlockHours` or `plainMostKiB` that is not positive
    whole numbers, or chunk sizes that are not min < avg < max.

It does NOT decide whether a store's verdict agrees with the lists: that is
the crate's own arithmetic (`kept::verdict`), and a test of the crate compares
the two — so the rule is written once, in the language that runs it.
"""

import argparse
import json
import re
import sys

ROOTS = ("config", "state", "home")
ID = re.compile(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$")
TOP = {
    "largeMB", "trees", "files", "never", "stores",
    # the 2026-10-05 wave (format 2)
    "targets", "plain", "plainMostKiB", "chunk", "budget", "unlockHours",
}
NEVER = {"name", "path", "reason", "hard"}
STORE = {"kept", "reason"}
TARGET = {"paths", "skip", "private", "merge", "offers"}
MERGES = {"choose", "union"}
OFFERS = {"ssh-signing", "pam-u2f"}
PLAIN = {"at", "reason"}
CHUNK = {"minKiB", "avgKiB", "maxKiB"}
BUDGET = {"pushMB", "recommendMB", "warnMB", "historyOverTree", "keepDays"}
# ── anchor: L08 ──
# ── anchor: L09 ──


def covers(root, path):
    """Whether `path` is `root` or under it, by component."""
    return path == root or path.startswith(root + "/")


def plain_refusals(plain, nevers):
    """The plain roots against each other and against `never`: two roots never
    overlap (by path or by `at` — a file would have two places, or two files
    one), and none lies under a path `never` names (the access files would
    travel IN CLEAR)."""
    for i, a in enumerate(plain):
        for b in plain[i + 1:]:
            if covers(a["path"], b["path"]) or covers(b["path"], a["path"]):
                refuse(f"plain: `{a['path']}` and `{b['path']}` overlap")
            if covers(a["at"], b["at"]) or covers(b["at"], a["at"]):
                refuse(f"plain: the places `{a['at']}` and `{b['at']}` overlap")
        for kind, value, _, _ in nevers:
            if kind == "Path" and (covers(value, a["path"]) or covers(a["path"], value)):
                refuse(f"plain: `{a['path']}` overlaps `{value}`, which is never kept")


def plain_rust(plain):
    """The plain roots as Rust (`crate::kept::PlainRoot`, by its full path:
    the table's own `use` line is the other packages' too)."""
    lines = [
        "/// The roots of the plain part: kept IN CLEAR under `plain/<at>`.",
        "pub const PLAIN: &[crate::kept::PlainRoot] = &[",
    ]
    for root in plain:
        lines.append(
            f"    crate::kept::PlainRoot {{ path: {rust_str(root['path'])}, "
            f"at: {rust_str(root['at'])}, reason: {rust_str(root['reason'])} }},"
        )
    return lines + ["];", ""]


# ── anchor: L10 ──
# THE PREDEFINED TARGETS: what may not overlap them, and their Rust table.
MERGE_RULES = {"choose": "Choose", "union": "Union"}
OFFERINGS = {"ssh-signing": "SshSigning", "pam-u2f": "PamU2f"}


def target_refusals(targets, plain, nevers, trees, files):
    """The targets against each other and against every other list: a path
    has ONE verdict, so no target path overlaps another target's, a tree, a
    file, a plain root or a path `never` names (the access files would
    travel as a target). And a `merge` names a file (`known_hosts`) or
    `default`, never a path."""
    owned = [(t["id"], p) for t in targets for p in t["paths"]]
    for i, (a_id, a) in enumerate(owned):
        for b_id, b in owned[i + 1:]:
            if covers(a, b) or covers(b, a):
                refuse(f"targets: `{a}` of `{a_id}` and `{b}` of `{b_id}` overlap")
        for kind, other in [("tree", p) for p, _ in trees] + [("file", p) for p, _ in files]:
            if covers(a, other) or covers(other, a):
                refuse(f"targets.{a_id}: `{a}` overlaps the {kind} `{other}`")
        for root in plain:
            if covers(a, root["path"]) or covers(root["path"], a):
                refuse(f"targets.{a_id}: `{a}` overlaps the plain root `{root['path']}`")
        for kind, value, _, _ in nevers:
            if kind == "Path" and (covers(value, a) or covers(a, value)):
                refuse(f"targets.{a_id}: `{a}` overlaps `{value}`, which is never kept")
    for t in targets:
        for name in t["merge"]:
            if not name or "/" in name or name in (".", ".."):
                refuse(f"targets.{t['id']}: the merge of `{name}` names no file (a name, or `default`)")


def targets_rust(targets):
    """The targets as Rust (`crate::kept::Target`, by its full path)."""
    lines = [
        "/// The predefined encrypted targets, by id.",
        "pub const TARGETS: &[crate::kept::Target] = &[",
    ]
    for t in targets:
        paths = ", ".join(rust_str(p) for p in t["paths"])
        skip = ", ".join(rust_str(s) for s in t["skip"])
        merge = ", ".join(
            f"({rust_str(name)}, crate::kept::MergeRule::{MERGE_RULES[rule]})"
            for name, rule in sorted(t["merge"].items())
        )
        offers = ", ".join(f"crate::kept::Offering::{OFFERINGS[o]}" for o in t["offers"])
        lines.append(
            f"    crate::kept::Target {{ id: {rust_str(t['id'])}, paths: &[{paths}], "
            f"skip: &[{skip}], private: {'true' if t['private'] else 'false'}, "
            f"merge: &[{merge}], offers: &[{offers}] }},"
        )
    return lines + ["];", ""]


errors = []


def refuse(msg):
    errors.append(msg)


def logical(where, path):
    """Whether `path` is a logical path, saying why not."""
    if not isinstance(path, str) or not path:
        refuse(f"{where}: `{path}` is not a path")
        return False
    parts = path.split("/")
    if parts[0] not in ROOTS:
        refuse(f"{where}: `{path}` does not begin with a root ({', '.join(ROOTS)})")
        return False
    if path.startswith("/") or any(p in ("", ".", "..") for p in parts):
        refuse(f"{where}: `{path}` climbs, is absolute or has an empty component")
        return False
    return True


def reason(where, value):
    if not isinstance(value, str) or not ID.match(value):
        refuse(f"{where}: the reason `{value}` is not an English kebab id")
        return False
    return True


def rust_str(s):
    return json.dumps(s, ensure_ascii=False)


def whole(where, value):
    """Whether `value` is a positive whole number, saying why not."""
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        refuse(f"{where} `{value}` is not a positive whole number")
        return False
    return True


def numbers(where, table, fields):
    """A section of positive whole numbers with exactly `fields`."""
    if not isinstance(table, dict):
        refuse(f"{where} is not an attrset")
        return {}
    for field in sorted(set(table) - fields):
        refuse(f"{where}: `{field}` is not a field ({', '.join(sorted(fields))})")
    for field in sorted(fields - set(table)):
        refuse(f"{where}: `{field}` is missing")
    return {f: table[f] for f in sorted(fields & set(table)) if whole(f"{where}.{f}", table[f])}


def pattern(where, name):
    if not isinstance(name, str) or not name or "/" in name or name.count("*") > 1:
        refuse(f"{where}: `{name}` is not a name pattern (at most one `*`, no `/`)")
        return False
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--table", required=True)
    ap.add_argument("--out-rust", required=True)
    ap.add_argument("--out-json", required=True)
    a = ap.parse_args()

    table = json.load(open(a.table))
    for field in sorted(set(table) - TOP):
        refuse(f"`{field}` is not a field ({', '.join(sorted(TOP))})")
    for field in sorted(TOP - set(table)):
        refuse(f"`{field}` is missing")

    large = table.get("largeMB")
    if not isinstance(large, int) or isinstance(large, bool) or large <= 0:
        refuse(f"largeMB `{large}` is not a positive whole number")
        large = 1

    reasons = set()
    trees, files = [], []
    for kind, out in (("trees", trees), ("files", files)):
        for path, why in sorted((table.get(kind) or {}).items()):
            if logical(f"{kind}.{path}", path) and reason(f"{kind}.{path}", why):
                out.append((path, why))
                reasons.add(why)

    nevers = []
    for n, entry in enumerate(table.get("never") or []):
        where = f"never[{n}]"
        if not isinstance(entry, dict):
            refuse(f"{where} is not an attrset")
            continue
        for field in sorted(set(entry) - NEVER):
            refuse(f"{where}: `{field}` is not a field ({', '.join(sorted(NEVER))})")
        has_name, has_path = "name" in entry, "path" in entry
        if has_name == has_path:
            refuse(f"{where}: one of `name` and `path`, never both or neither")
            continue
        hard = entry.get("hard", False)
        if not isinstance(hard, bool):
            refuse(f"{where}: `hard` is a boolean")
            continue
        if not reason(where, entry.get("reason")):
            continue
        reasons.add(entry["reason"])
        if has_name:
            name = entry["name"]
            if hard:
                refuse(f"{where}: a name pattern may not be hard")
                continue
            if not isinstance(name, str) or not name or "/" in name or name.count("*") > 1:
                refuse(f"{where}: `{name}` is not a name pattern (at most one `*`, no `/`)")
                continue
            nevers.append(("Name", name, entry["reason"], False))
        else:
            if logical(where, entry["path"]):
                nevers.append(("Path", entry["path"], entry["reason"], hard))

    stores = []
    for path, entry in sorted((table.get("stores") or {}).items()):
        where = f"stores.{path}"
        if not isinstance(entry, dict):
            refuse(f"{where} is not an attrset")
            continue
        for field in sorted(set(entry) - STORE):
            refuse(f"{where}: `{field}` is not a field ({', '.join(sorted(STORE))})")
        if not isinstance(entry.get("kept"), bool):
            refuse(f"{where}: `kept` is a boolean")
            continue
        if logical(where, path) and reason(where, entry.get("reason")):
            stores.append((path, entry["kept"], entry["reason"]))
            reasons.add(entry["reason"])

    # ── the 2026-10-05 wave (format 2) ──────────────────────────────────
    targets = []
    for tid, entry in sorted((table.get("targets") or {}).items()):
        where = f"targets.{tid}"
        if not ID.match(tid):
            refuse(f"{where}: the id `{tid}` is not an English kebab id")
            continue
        if not isinstance(entry, dict):
            refuse(f"{where} is not an attrset")
            continue
        for field in sorted(set(entry) - TARGET):
            refuse(f"{where}: `{field}` is not a field ({', '.join(sorted(TARGET))})")
        paths = entry.get("paths")
        if not isinstance(paths, list) or not paths:
            refuse(f"{where}: `paths` is a list of one or more logical paths")
            continue
        ok = all(logical(f"{where}.paths", p) for p in paths)
        skip = entry.get("skip", [])
        ok = isinstance(skip, list) and all(pattern(f"{where}.skip", s) for s in skip) and ok
        private = entry.get("private", False)
        if not isinstance(private, bool):
            refuse(f"{where}: `private` is a boolean")
            ok = False
        merge = entry.get("merge", {})
        if not isinstance(merge, dict) or any(v not in MERGES for v in merge.values()):
            refuse(f"{where}: `merge` maps a file name or `default` to one of {', '.join(sorted(MERGES))}")
            ok = False
        offers = entry.get("offers", [])
        if not isinstance(offers, list) or any(o not in OFFERS for o in offers):
            refuse(f"{where}: `offers` is a list of {', '.join(sorted(OFFERS))}")
            ok = False
        if ok:
            targets.append({"id": tid, "paths": paths, "skip": skip, "private": private,
                            "merge": merge, "offers": offers})

    plain = []
    for path, entry in sorted((table.get("plain") or {}).items()):
        where = f"plain.{path}"
        if not isinstance(entry, dict):
            refuse(f"{where} is not an attrset")
            continue
        for field in sorted(set(entry) - PLAIN):
            refuse(f"{where}: `{field}` is not a field ({', '.join(sorted(PLAIN))})")
        at = entry.get("at")
        if not isinstance(at, str) or not at or at.startswith("/") or any(
            p in ("", ".", "..") for p in at.split("/")
        ):
            refuse(f"{where}: `at` `{at}` is not a plain relative path")
            continue
        if logical(where, path) and reason(where, entry.get("reason")):
            plain.append({"path": path, "at": at, "reason": entry["reason"]})
            reasons.add(entry["reason"])

    plain_refusals(plain, nevers)
    target_refusals(targets, plain, nevers, trees, files)
    plain_most = table.get("plainMostKiB")
    whole("plainMostKiB", plain_most)
    unlock_hours = table.get("unlockHours")
    whole("unlockHours", unlock_hours)
    chunk = numbers("chunk", table.get("chunk"), CHUNK)
    if len(chunk) == len(CHUNK) and not (chunk["minKiB"] < chunk["avgKiB"] < chunk["maxKiB"]):
        refuse("chunk: the sizes are not ordered (minKiB < avgKiB < maxKiB)")
    budget = numbers("budget", table.get("budget"), BUDGET)

    if errors:
        print("ERROR: assets/belongings.nix is refused:", file=sys.stderr)
        for e in errors:
            print("       " + e, file=sys.stderr)
        sys.exit(1)

    lines = [
        "// GENERATED from assets/belongings.nix by nix/scripts/gen-belongings.py —",
        "// never edited by hand. The types are `crate::kept`'s.",
        "",
        "/// Past this many bytes a path the person adds is said to be large.",
        f"pub const LARGE_BYTES: u64 = {large * 1000 * 1000};",
        "",
        "/// Kept whole, minus what [`NEVER`] names.",
        "pub const TREES: &[Place] = &[",
    ]
    for path, why in trees:
        lines.append(f"    Place {{ path: {rust_str(path)}, reason: {rust_str(why)} }},")
    lines += ["];", "", "/// Kept one by one, outside the trees.", "pub const FILES: &[Place] = &["]
    for path, why in files:
        lines.append(f"    Place {{ path: {rust_str(path)}, reason: {rust_str(why)} }},")
    lines += ["];", "", "/// Never kept.", "pub const NEVER: &[Never] = &["]
    for kind, value, why, hard in nevers:
        lines.append(
            f"    Never {{ pattern: Pattern::{kind}({rust_str(value)}), reason: {rust_str(why)}, "
            f"hard: {'true' if hard else 'false'} }},"
        )
    lines += ["];", "", "/// The census of every preference file (`belongings-manifest-covers-the-stores`).",
              "pub const STORES: &[Census] = &["]
    for path, kept, why in stores:
        lines.append(
            f"    Census {{ path: {rust_str(path)}, kept: {'true' if kept else 'false'}, "
            f"reason: {rust_str(why)} }},"
        )
    lines += ["];", "", "/// Every reason the table gives (`belongings-reason-<id>`).",
              "pub const REASONS: &[&str] = &["]
    for why in sorted(reasons):
        lines.append(f"    {rust_str(why)},")
    lines += ["];", ""]
    # THE FORMAT-2 NUMBERS, as the units their names carry. The targets and
    # the plain roots are emitted by the packages that give them a type.
    kib, mb = 1024, 1000 * 1000
    lines += [
        "/// The plain part's largest file.",
        f"pub const PLAIN_MOST_BYTES: u64 = {plain_most * kib};",
        "/// The smallest chunk.",
        f"pub const CHUNK_MIN_BYTES: u64 = {chunk['minKiB'] * kib};",
        "/// The average chunk.",
        f"pub const CHUNK_AVG_BYTES: u64 = {chunk['avgKiB'] * kib};",
        "/// The largest chunk.",
        f"pub const CHUNK_MAX_BYTES: u64 = {chunk['maxKiB'] * kib};",
        "/// Past this many new bytes a push goes in batches.",
        f"pub const PUSH_MOST_BYTES: u64 = {budget['pushMB'] * mb};",
        "/// Past this the view recommends compacting.",
        f"pub const RECOMMEND_BYTES: u64 = {budget['recommendMB'] * mb};",
        "/// Past this the view warns.",
        f"pub const WARN_BYTES: u64 = {budget['warnMB'] * mb};",
        "/// The history this many times the tree recommends compacting.",
        f"pub const HISTORY_OVER_TREE: u64 = {budget['historyOverTree']};",
        "/// The days of history a compaction keeps.",
        f"pub const KEEP_DAYS: u64 = {budget['keepDays']};",
        "/// The hours the identity may stay in the kernel's keyring.",
        f"pub const UNLOCK_HOURS: u64 = {unlock_hours};",
        "",
    ]
    lines += plain_rust(plain)
    lines += targets_rust(targets)
    with open(a.out_rust, "w") as f:
        f.write("\n".join(lines))

    with open(a.out_json, "w") as f:
        json.dump(
            {
                "largeBytes": large * 1000 * 1000,
                "trees": [{"path": p, "reason": r} for p, r in trees],
                "files": [{"path": p, "reason": r} for p, r in files],
                "never": [{"kind": k.lower(), "value": v, "reason": r, "hard": h} for k, v, r, h in nevers],
                "stores": [{"path": p, "kept": k, "reason": r} for p, k, r in stores],
                "reasons": sorted(reasons),
                "targets": targets,
                "plain": plain,
                "plainMostBytes": plain_most * 1024,
                "chunk": chunk,
                "budget": budget,
                "unlockHours": unlock_hours,
            },
            f,
            indent=1,
            sort_keys=True,
        )


if __name__ == "__main__":
    main()
