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
  - a `largeMB` that is not a positive whole number.

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
TOP = {"largeMB", "trees", "files", "never", "stores"}
NEVER = {"name", "path", "reason", "hard"}
STORE = {"kept", "reason"}

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
            },
            f,
            indent=1,
            sort_keys=True,
        )


if __name__ == "__main__":
    main()
