"""Old ISOs leave the store: every ISO this host built, except the newest of
each name and any that something roots.

An ISO is ~5 GiB, and every build and VM-test iteration of the installer
makes one. Nothing roots them — a VM test's driver refers to the ISO it boots,
and nothing roots the driver either — so they stay until a garbage
collection, and a full collection also throws away every build the batteries
would otherwise reuse. Measured on 2026-10-23: eight of them, 38 GiB, on a
pool at 93%.

So the apps that build ISOs (`.#iso`, `.#release`, `.#gate`, `.#most`,
`.#sized-build`) end by running this, and it deletes ONLY ISOs, each with
what refers to it (the drivers), never anything else:

- an ISO that is rooted — `--out-link`, a profile, anything
  `nix-store --query --roots` names — is kept, and so is one a rooted path
  refers to: a root is how a person says "keep this";
- the newest of each store name is kept — the build that just finished, and
  `-complete` (the developer's whole ISO) is a name of its own;
- one a running build holds is kept: `nix-store --delete` refuses a live path,
  and that refusal is a verdict, not a failure.

Subcommands:

  sweep [--keep N] [--rehearse]   find, choose, delete; prints one line per ISO
  choose [--keep N]               the choice alone, on planted JSON from stdin
                                  (what `iso-sweep-keeps-what-it-must` runs)
"""

import argparse
import glob
import json
import subprocess
import sys

# WHAT AN ISO OF THIS DISTRO IS NAMED IN THE STORE: `image.baseName`
# (`nix/installer/iso.nix`) plus `.iso`, the default one and `-complete` alike.
STORE_GLOB = "/nix/store/*-ronoctua-distro-*.iso"


def name_of(path):
    """The store name: the path without `/nix/store/` and its hash."""
    return path.rsplit("/", 1)[-1].split("-", 1)[1]


def choose(isos, keep):
    """Which ISOs go, as `(path, verdict, reason)` in the order given.

    `isos` is a list of `{path, registered, rooted, referred_by_rooted}`;
    `registered` is the store's registration time (what "newest" means).
    The verdict is `keep` or `delete`; deleting can still be refused later by
    the store (a live path), and that is said then.
    """
    newest = {}
    for iso in sorted(isos, key=lambda i: (-i["registered"], i["path"])):
        newest.setdefault(name_of(iso["path"]), []).append(iso["path"])
    kept_newest = {p for paths in newest.values() for p in paths[:keep]}
    out = []
    for iso in isos:
        path = iso["path"]
        if iso["rooted"]:
            out.append((path, "keep", "rooted"))
        elif iso["referred_by_rooted"]:
            out.append((path, "keep", "a rooted path refers to it"))
        elif path in kept_newest:
            out.append((path, "keep", "the newest of its name"))
        else:
            out.append((path, "delete", "older, and nothing roots it"))
    return out


def run(*argv):
    return subprocess.run(argv, capture_output=True, text=True, check=False)


def roots_of(paths):
    """Whether any of `paths` is a garbage-collector root's target."""
    if not paths:
        return False
    said = run("nix-store", "--query", "--roots", *paths)
    return bool(said.stdout.strip())


def referrers_of(path):
    """Everything that refers to `path`, transitively, itself included."""
    said = run("nix-store", "--query", "--referrers-closure", path)
    return [line for line in said.stdout.split() if line]


def found():
    """Every ISO in the store, with what `choose` needs to know of it."""
    paths = sorted(glob.glob(STORE_GLOB))
    if not paths:
        return [], {}
    info = run("nix", "path-info", "--json", "--json-format", "1", *paths)
    try:
        table = json.loads(info.stdout or "{}")
    except json.JSONDecodeError:
        table = {}
    if isinstance(table, list):
        table = {row.get("path"): row for row in table}
    isos, closures = [], {}
    for path in paths:
        row = table.get(path) or {}
        closure = referrers_of(path)
        closures[path] = closure
        isos.append(
            {
                "path": path,
                "registered": int(row.get("registrationTime") or 0),
                "size": int(row.get("narSize") or 0),
                "rooted": roots_of([path]),
                "referred_by_rooted": roots_of([p for p in closure if p != path]),
            }
        )
    return isos, closures


def sweep(keep, rehearse):
    isos, closures = found()
    freed = 0
    for path, verdict, reason in choose(isos, keep):
        if verdict == "keep":
            print(f"kept {path} ({reason})")
            continue
        size = next(i["size"] for i in isos if i["path"] == path)
        if rehearse:
            print(f"would delete {path} ({size >> 20} MiB, {reason})")
            continue
        # THE ISO WITH WHAT REFERS TO IT, in one call: the store deletes the
        # referrers first, and refuses the whole set if any of it is live.
        said = run("nix-store", "--delete", *closures[path])
        if said.returncode == 0:
            freed += size
            print(f"deleted {path} ({size >> 20} MiB)")
        else:
            print(f"kept {path} (in use: the store refused to delete it)")
    if not rehearse:
        print(f"iso-sweep: {freed >> 20} MiB freed")


def main():
    parser = argparse.ArgumentParser(prog="iso-sweep")
    sub = parser.add_subparsers(dest="command", required=True)
    s = sub.add_parser("sweep")
    s.add_argument("--keep", type=int, default=1)
    s.add_argument("--rehearse", action="store_true")
    c = sub.add_parser("choose")
    c.add_argument("--keep", type=int, default=1)
    args = parser.parse_args()
    if args.keep < 1:
        parser.error("--keep is at least 1: the newest build is the one just made")
    if args.command == "sweep":
        sweep(args.keep, args.rehearse)
    else:
        for path, verdict, reason in choose(json.load(sys.stdin), args.keep):
            print(f"{verdict} {path} ({reason})")


if __name__ == "__main__":
    main()
