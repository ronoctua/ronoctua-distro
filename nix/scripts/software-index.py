"""Turn the evaluated package list into the search index the CLI and the shell read.

One package per line, TAB-separated, sorted by attribute (so a lookup by
attribute is a binary search and the file never needs to be held whole):

    attr  pname  version  flags  program  license  homepage  description

`flags` is a run of letters: `u` unfree, `b` broken, `i` insecure, `n` not
available on this platform, `l` a library or language package (a nested set
such as `python3Packages.*`). The first line is a header,
`#ronoctua-software-nix-index<TAB>1<TAB><count>`, so a reader can refuse a
file written by another version instead of reading its columns wrong.

It refuses, rather than writes, an index that is not whole: fewer than
`FLOOR` packages means the evaluation lost most of nixpkgs, and a short index
is a search that says "nothing found" about programs that exist.
"""

import json
import sys

FORMAT = "1"
FLOOR = 50_000


def flags(r):
    out = ""
    if r["unfree"]:
        out += "u"
    if r["broken"]:
        out += "b"
    if r["insecure"]:
        out += "i"
    if not r["available"]:
        out += "n"
    if r["library"]:
        out += "l"
    return out


def main(source, target):
    with open(source, encoding="utf-8") as f:
        text = json.load(f)
    # `nix-instantiate --eval --json` over a `toJSON` string gives the string.
    records = json.loads(text) if isinstance(text, str) else text
    rows = {}
    for r in records:
        attr = r["attr"]
        if not attr or "\t" in attr or "\n" in attr:
            continue
        rows[attr] = "\t".join(
            [
                attr,
                r["pname"],
                r["version"],
                flags(r),
                r["program"],
                r["license"],
                r["homepage"],
                r["description"],
            ]
        )
    if len(rows) < FLOOR:
        sys.exit(f"only {len(rows)} packages evaluated; the index would lie by omission")
    with open(target, "w", encoding="utf-8") as out:
        out.write(f"#ronoctua-software-nix-index\t{FORMAT}\t{len(rows)}\n")
        for attr in sorted(rows):
            out.write(rows[attr] + "\n")
    print(f"{len(rows)} packages")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
