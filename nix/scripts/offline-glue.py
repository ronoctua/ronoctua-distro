"""The glue an installation with no network builds with.

Reads the `.drv` graphs of the reference systems whole and of each probe
below what differs from them, and prints, as JSON, every output a
derivation the target builds takes from one it does not build — what the
target has to find already built (`nix/installer/offline.nix`).

The target builds what differs from the references and needs no network:
never a download (a fixed output), and never a package made from one. What
it takes from those is glue as much as what it takes from the references: a
probe's closure holds a package its system REFERENCES, and not one its
build only reads — the linux-firmware the AMD microcode is cut from, the
microcode an initrd is prepended with (2026-10-25: every machine with a CPU
asked the network for one, and only the guest installed).

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
    the network — so it finds it built, and its inputs are no glue."""
    return any(drv(x)[2] for x in drv(path)[1])


def never_built_here(path):
    """A download, or a package made from one: the target finds it built."""
    return drv(path)[2] or from_a_download(path)


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


def name_of(path):
    return path.rsplit("/", 1)[-1][33:].removesuffix(".drv")


def walk(references, probes):
    known = closure(references)
    glue = {}
    differing = {}
    for probe in probes:
        mine = closure([probe], descend=lambda p: p not in known and not never_built_here(p)) - known
        built = {d for d in mine if not never_built_here(d)}
        differing[probe] = len(built)
        for d in built:
            for x, outs in drv(d)[1].items():
                if x not in built:
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


def install(carried_file, tops):
    """An installation from the ISO, as Nix makes it with no network: an
    output the ISO carries is found, and any other is built, its inputs
    asked the same. A download it would fetch, or a package it would
    build from one, is the installation stopping (`iso-builds-only-glue`)."""
    with open(carried_file, encoding="utf-8") as f:
        carried = set(f.read().split())
    if len(carried) < 1000:
        sys.exit(f"offline-glue: {carried_file} names {len(carried)} paths, not an ISO's store")
    wrong = []
    for top in tops:
        built = set()
        stack = [(top, None)]
        while stack:
            d, outs = stack.pop()
            outputs, inputs, fixed = drv(d)
            wanted = list(outputs) if outs is None else outs
            if d in built or all(outputs[o] in carried for o in wanted):
                continue
            built.add(d)
            if fixed:
                wrong.append(f"{name_of(top)}: downloads {name_of(d)}")
            elif from_a_download(d):
                wrong.append(f"{name_of(top)}: builds {name_of(d)} from a download")
            else:
                stack.extend(inputs.items())
        if not built:
            wrong.append(f"{name_of(top)}: the ISO carries it whole, so it shows nothing of a machine unlike the probes")
        print(f"{name_of(top)}: {len(built)} derivations built from what the ISO carries")
    if wrong:
        print("ERROR: an installation from this ISO asks the network:", file=sys.stderr)
        for w in sorted(set(wrong)):
            print("  " + w, file=sys.stderr)
        sys.exit(1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--references", nargs="+")
    ap.add_argument("--probes", nargs="+")
    # Or: the ISO's store (a `closureInfo`'s `store-paths`) and the tops an
    # installation builds, to install them against it.
    ap.add_argument("--carried")
    ap.add_argument("--installs", nargs="+")
    a = ap.parse_args()
    if a.carried and a.installs and not (a.references or a.probes):
        install(a.carried, a.installs)
    elif a.references and a.probes and not (a.carried or a.installs):
        walk(a.references, a.probes)
    else:
        sys.exit("offline-glue: --references and --probes walk, --carried and --installs install")


main()
