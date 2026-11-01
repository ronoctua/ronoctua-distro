# EVERY NAME THE SUPERPACK ANSWERS TO, one per line: the merge's manifest with
# the sizes, contexts and licences taken out, sorted and unique.
#
# usage: python3 names.py MANIFEST OUT
import json
import sys

manifest, out = sys.argv[1], sys.argv[2]
entries = json.load(open(manifest))["entries"]
names = sorted({e["name"] for e in entries})
# A FLOOR, because a manifest whose shape moved gives back an empty list, and
# an empty list is a search that finds nothing and says nothing.
if len(names) < 10000:
    sys.exit(f"only {len(names)} icon names in the superpack's manifest")
# A name that could split a line would split the file's grammar.
bad = [n for n in names if not n or any(c in n for c in "\n\r\t")]
if bad:
    sys.exit(f"{len(bad)} icon names carry a line break or a tab: {bad[:3]}")
with open(out, "w") as f:
    f.writelines(n + "\n" for n in names)
print(f"icon names: {len(names)}")
