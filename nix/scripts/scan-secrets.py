#!/usr/bin/env python3
"""Fails if there is a secret pattern in the tree — section 9.

"Other people are going to install this distro. The repository has to be
publishable as it is." No password, password hash, passphrase, private SSH
key, wifi PSK, token, e-mail or real name.

# Why by shape, and not by word

Searching for the word "password" would flag the tests themselves: `installer-dryrun`
sets `RONOCTUA_INST_PASSWORD=test-password`, and that is a fixture, not a secret.
What is looked for here is the SHAPE of a real secret — a private key
block, a crypt hash prefix, an age key, a known token prefix.
A real secret has a recognisable shape; a fixture does not.

# Why the patterns are assembled by concatenation

If the pattern were written out whole, this file would flag itself on every
run — and the obvious way out would be to exclude this very file from the scan,
which would open a hole in exactly the easiest place to hide something.

Usage:
    scan-secrets.py <directory>
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Assembled in pieces on purpose — see the header.
_BEGIN = "-----" + "BEGIN"
_PRIV = "PRIVATE" + " KEY"

PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("private key", re.compile(rf"{_BEGIN}[ A-Z]*{_PRIV}")),
    # `$y$` yescrypt, `$6$` sha512, `$2b$` bcrypt, `$argon2…`. The suffix requires
    # a long body so it does not match a bare `$6$` in some text.
    ("password hash", re.compile(r"\$(y|6|2[aby]|argon2[a-z]*)\$[./A-Za-z0-9$,=+-]{16,}")),
    ("age key", re.compile("AGE-SECRET-" + "KEY-1[0-9A-Z]{50,}")),
    ("service token", re.compile(r"\b(ghp_|github_pat_|xoxb-|sk-ant-|AKIA)[A-Za-z0-9_-]{16,}")),
    ("wifi PSK", re.compile(r"\bpsk\s*=\s*[\"']?[^\s\"']{8,}")),
    # E-mail: section 9 admits none. Obvious placeholders pass.
    ("e-mail address", re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")),
]

# Placeholders that are NOT personal data.
EXEMPT = re.compile(
    r"(example\.(com|org)|example\.com|<[^>]*>|foo@bar|localhost|@\{|"
    r"your-user|user@|noreply@|"
    # `user@1000.service` is a systemd UNIT name, and it appears in every
    # cgroup path. Without this exemption, any test that mentions a
    # session cgroup becomes "e-mail in the repository".
    r"user@\d+\.service|"
    # AND SO IS EVERY OTHER TEMPLATE INSTANCE: `foo@bar.service` is the
    # syntax systemd gives one, and it cannot be a person's address — the
    # suffix is a unit type, not a top-level domain. Without this, writing a
    # `ronoctua-memory-profile@tolerant.service` into a polkit rule reads as
    # personal data in the repository, which is the opposite of what this
    # scanner is for.
    r"@[A-Za-z0-9_.-]+\.(service|slice|scope|socket|timer|target|mount|path))"
)

# THE LICENCE OF A THIRD PARTY IS NOT OURS TO EDIT.
#
# `LICENSE-OFL.txt` opens with Iosevka's copyright line, and that line carries
# the author's e-mail. The OFL REQUIRES the notice to be preserved intact —
# stripping the address to please this scan would break the very licence that
# makes redistributing the faces legitimate.
#
# The exemption is (file, RULE), and not the file on its own: a private key or a
# service token planted in `LICENSE-OFL.txt` still has to be caught. Only the
# e-mail rule is lifted, and only there.
EXEMPT_PER_FILE = {"LICENSE-OFL.txt": {"e-mail address"}}

# There is no reason to read these: they are binaries or a build tree.
IGNORAR_DIR = {".git", "target", "result", ".direnv", "__pycache__"}
IGNORAR_EXT = {
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".pdf", ".zip", ".gz", ".xz",
    ".ttf", ".otf", ".woff", ".woff2", ".psf", ".bin", ".fst", ".wasm",
    ".qcow2", ".img", ".iso",
}


def scan(root_dir: Path) -> list[str]:
    """Findings, one per suspicious line."""
    hits: list[str] = []
    for f in sorted(root_dir.rglob("*")):
        if not f.is_file() or f.is_symlink():
            continue
        if any(p in IGNORAR_DIR for p in f.parts):
            continue
        if f.suffix.lower() in IGNORAR_EXT:
            continue

        try:
            raw_text = f.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            # A binary with no known extension. A cleartext secret
            # inside a binary is not the failure mode this is after.
            continue

        rel = f.relative_to(root_dir)
        for n, line in enumerate(raw_text.splitlines(), 1):
            if EXEMPT.search(line):
                continue
            exempt = EXEMPT_PER_FILE.get(f.name, frozenset())
            for name, pattern in PATTERNS:
                if name in exempt:
                    continue
                m = pattern.search(line)
                if m:
                    # The SNIPPET is not printed: if it is a real secret, the
                    # CI log would publish it again. Only the shape and the place.
                    hits.append(f"{rel}:{n}: {name} ({len(m.group(0))} characters)")
                    break
    return hits


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    root_dir = Path(sys.argv[1])
    if not root_dir.is_dir():
        print(f"ERROR: {root_dir} is not a directory", file=sys.stderr)
        return 2

    hits = scan(root_dir)
    if hits:
        print("ERROR: a secret pattern in the tree (section 9).", file=sys.stderr)
        print("", file=sys.stderr)
        for a in hits:
            print(f"  {a}", file=sys.stderr)
        print("", file=sys.stderr)
        print(
            "The repository has to be publishable as it stands. Personal data and\n"
            "a secret live in /persist or in the local host.nix, never here.",
            file=sys.stderr,
        )
        return 1

    print("OK: no secret pattern in the tree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
