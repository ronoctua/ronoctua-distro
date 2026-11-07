#!/usr/bin/env python3
"""The D-Bus configuration with each file listed once (`nix/modules/dbus.nix`).

Takes the directory `makeDBusConf` wrote, `system-path`, and where to write.

`makeDBusConf` gives every package of `services.dbus.packages` its own
directories, and NixOS adds `system-path`, where `environment.pathsToLink` has
already linked the same files. So the broker reads most names twice and says
`Ignoring duplicate name`, at error, at every load: 41 lines a time.

WHAT GOES
---------
A package's directory, when `system-path` holds every file in it with the same
bytes. The question is asked of each directory on its own, against the built
files: a package that left the system's packages, or a file `system-path` took
from another package, simply stays. Nothing here has to be told which package
is which, and `ronoctua-network-agent-dbus`, which `system-path` does not hold,
is kept by the same rule that drops the others.

WHAT THIS BUILD CANNOT SEE STAYS: a root outside the store, and a link that
resolves nowhere here, to a file or to the directory or one above it. Either
can be real on the machine, and nothing was compared.

A REPEATED LINE goes too: two modules naming one package list it twice.

WHAT THE SESSION BUS LOSES ON TOP
---------------------------------
`system-path`'s own service directory. `<standard_session_servicedirs/>` already
reads it through `XDG_DATA_DIRS`, as `/run/current-system/sw/share`, and that
element is what finds a Flatpak's or a person's service, so it has to stay. The
`includedir`s stay as well: `XDG_DATA_DIRS` covers services, not `session.d`.
"""

from __future__ import annotations

import filecmp
import pathlib
import re
import sys

ENTRY = re.compile(r"<(servicedir|includedir)>(/[^<]+?)/((?:etc|share)/dbus-1/[^<]+)</\1>")
SYSTEM_PATH_SERVICES = "<servicedir>{linked}/share/dbus-1/system-services</servicedir>"
SESSION_SERVICES = "<standard_session_servicedirs/>"


def covered(root: str, sub: str, linked: pathlib.Path) -> bool:
    """Whether `linked` holds everything `root/sub` does, byte for byte."""
    if not pathlib.Path(root).is_dir():
        return False
    theirs = pathlib.Path(root, sub)
    if not theirs.is_dir():
        return absent(pathlib.Path(root), sub)
    for file in theirs.rglob("*"):
        if file.is_dir():
            continue
        ours = linked / sub / file.relative_to(theirs)
        if not (file.is_file() and ours.is_file() and filecmp.cmp(file, ours, shallow=False)):
            return False
    return True


def absent(root: pathlib.Path, sub: str) -> bool:
    """Whether nothing is at `root/sub`, which then carries nothing.

    Not a link on the way that resolves nowhere here, to the directory or to
    one above it: on the machine it may hold files, and nothing was compared.
    Nor something there that is not a directory, which nothing was compared
    with either.
    """
    here = root
    for part in pathlib.PurePath(sub).parts:
        here = here / part
        if here.is_symlink() and not here.exists():
            return False
        if not here.exists():
            return True
    return False


def thinned(text: str, linked: pathlib.Path, session: bool) -> str:
    """One conf with what `linked` already carries taken out."""
    seen = set()

    def keep(entry: re.Match) -> str:
        kind, root, sub = entry.groups()
        if entry.group(0) in seen:
            return ""
        seen.add(entry.group(0))
        if root == str(linked):
            return "" if session and kind == "servicedir" else entry.group(0)
        return "" if covered(root, sub, linked) else entry.group(0)

    return ENTRY.sub(keep, text)


def main() -> int:
    whole, linked, out = (pathlib.Path(a) for a in sys.argv[1:])
    system = thinned((whole / "system.conf").read_text(), linked, session=False)
    session = thinned((whole / "session.conf").read_text(), linked, session=True)
    # WHAT THE WHOLE IDEA RESTS ON. Each is the one place left that a dropped
    # directory's files are read from: without it the buses would start, and
    # no service would activate.
    services = SYSTEM_PATH_SERVICES.format(linked=linked)
    if services not in system:
        print(f"ERROR: system.conf does not list {services}", file=sys.stderr)
        return 1
    if SESSION_SERVICES not in session:
        print(f"ERROR: session.conf does not carry {SESSION_SERVICES}", file=sys.stderr)
        return 1
    out.mkdir(parents=True, exist_ok=True)
    (out / "system.conf").write_text(system)
    (out / "session.conf").write_text(session)
    return 0


if __name__ == "__main__":
    sys.exit(main())
