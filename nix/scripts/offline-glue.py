"""The glue an installation with no network builds with.

Reads the `.drv` graphs of the reference systems whole and of each probe
below what differs from them, and prints, as JSON, every output a differing
derivation takes from a derivation that does not differ — what the target
has to find already built (`nix/installer/offline.nix`).

The `.drv` files are this build's inputs (each top's path, with its
references, is in the sandbox); nothing is built to read them.
"""

import argparse
import json
import re
import sys


SPECIAL = re.compile(r'["\\]')


def parse_string(text, i):
    """An ATerm string starting at `text[i] == '"'`: (value, next index)."""
    assert text[i] == '"', f"a string was expected at {i}"
    out = []
    i += 1
    while True:
        m = SPECIAL.search(text, i)
        out.append(text[i : m.start()])
        i = m.start()
        if text[i] == '"':
            return "".join(out), i + 1
        nxt = text[i + 1]
        out.append({"n": "\n", "t": "\t", "r": "\r"}.get(nxt, nxt))
        i += 2


def parse(text, i=0):
    """One ATerm value: a string, a list `[…]`, a tuple `(…)` or a
    constructor `Name(…)`."""
    c = text[i]
    if c == '"':
        return parse_string(text, i)
    if c in "[(":
        close = "]" if c == "[" else ")"
        items = []
        i += 1
        while text[i] != close:
            item, i = parse(text, i)
            items.append(item)
            if text[i] == ",":
                i += 1
        return items, i + 1
    start = i
    while text[i].isalnum():
        i += 1
    name = text[start:i]
    args, i = parse(text, i)
    return (name, args), i


DRVS = {}


def drv(path):
    """A derivation's outputs (name → path) and input derivations
    (path → the outputs it takes)."""
    if path not in DRVS:
        with open(path, encoding="utf-8") as f:
            text = f.read()
        if not text.startswith("Derive("):
            sys.exit(f"offline-glue: {path} is not a Derive")
        # Only the first two fields: the environment can be megabytes.
        outs, i = parse(text, len("Derive("))
        ins, _ = parse(text, i + 1)
        outputs = {o[0]: o[1] for o in outs}
        inputs = {d: o for d, o in ins}
        # A fixed output (a download) names its hash algorithm.
        fixed = any(o[2] for o in outs)
        DRVS[path] = (outputs, inputs, fixed)
    return DRVS[path]


def from_a_download(path):
    """Whether a derivation is a package built from a download: one of its
    inputs is a fixed output. The target never builds one — it would need
    the network — so it finds it built, in a probe's closure, and its
    inputs are no glue."""
    return any(drv(x)[2] for x in drv(path)[1])


def closure(tops, descend=lambda _: True):
    seen = set()
    stack = list(tops)
    while stack:
        p = stack.pop()
        if p in seen:
            continue
        seen.add(p)
        if descend(p):
            stack.extend(drv(p)[1])
    return seen


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--references", nargs="+", required=True)
    ap.add_argument("--probes", nargs="+", required=True)
    a = ap.parse_args()

    known = closure(a.references)
    glue = {}
    differing = {}
    for probe in a.probes:
        mine = closure([probe], descend=lambda p: p not in known and not from_a_download(p)) - known
        built = {d for d in mine if not from_a_download(d)}
        differing[probe] = len(built)
        for d in built:
            for x, outs in drv(d)[1].items():
                if x in known:
                    for out in outs:
                        glue[(x, out)] = drv(x)[0][out]
    if not glue:
        sys.exit("offline-glue: no probe differs from the references; the walk read nothing")
    json.dump(
        {
            "differing": differing,
            "glue": [{"drv": d, "output": o, "path": p} for (d, o), p in sorted(glue.items())],
        },
        sys.stdout,
        indent=1,
    )


main()
