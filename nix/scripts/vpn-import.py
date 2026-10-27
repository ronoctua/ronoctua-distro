#!/usr/bin/env python3
"""Register a whole folder of WireGuard servers at once — `nix run .#vpn-import`.

A commercial provider hands out a `.conf` per server, each with the SAME
placeholder where the private key goes and each routing only IPv4. This reads
the whole folder, asks the private key ONCE, adds IPv6 so the real address does
not leak around the tunnel, and hands the lot to root to seal and install
(`nix/modules/vpn.nix`).

THE KEY IS NEVER WRITTEN TO THE REPOSITORY OR THE STORE. It is typed here, put
in a `0600` file in the person's own runtime folder, and taken by the install
unit through the one narrow polkit rule; root seals it in the TPM2 as one blob
for every server (`ronoctua_security::vpn`). The locations — public keys and
endpoints a provider publishes — are not secret and travel as JSON.

    nix run .#vpn-import -- ~/main/temp/vpn
"""

from __future__ import annotations

import base64
import getpass
import json
import os
import subprocess
import sys
from pathlib import Path

# The person's own runtime folder the install unit takes from
# (`ronoctua_security::vpn::STAGING_DIR`, made `0700` by tmpfiles).
STAGING_DIR = Path("/run/ronoctua-vpn")
KEY_FILE = STAGING_DIR / "private.key"
LOCATIONS_FILE = STAGING_DIR / "locations.json"

# The unit that seals the key and installs the locations, started through the
# one narrow polkit rule (`nix/modules/vpn.nix`).
INSTALL_UNIT = "ronoctua-vpn-install.service"

# The interface's IPv6 address, a unique-local one invented only to ANCHOR the
# `::/0` route into the tunnel: a provider whose servers do not egress IPv6
# then black-holes IPv6 in the tunnel, which is no leak. The same literal the
# sync falls back to (`nix/scripts/vpn-sync.py`).
ULA6 = "fdce:1a2b:3c4d::2/64"

# The placeholder every provider file carries where the key goes — read past,
# never used.
PLACEHOLDER = "<insert_your_private_key_here>"

# Country codes to names, for a location's label. Only cosmetic: an unknown
# code falls back to itself, upper-cased. The city code is left as the provider
# writes it (`nyc`, `par`), upper-cased.
COUNTRIES = {
    "al": "Albania", "ar": "Argentina", "au": "Australia", "at": "Austria",
    "be": "Belgium", "ba": "Bosnia and Herzegovina", "br": "Brazil",
    "bg": "Bulgaria", "ca": "Canada", "cl": "Chile", "co": "Colombia",
    "cr": "Costa Rica", "hr": "Croatia", "cy": "Cyprus", "cz": "Czechia",
    "dk": "Denmark", "ee": "Estonia", "fi": "Finland", "fr": "France",
    "ge": "Georgia", "de": "Germany", "gr": "Greece", "hk": "Hong Kong",
    "hu": "Hungary", "is": "Iceland", "in": "India", "id": "Indonesia",
    "ie": "Ireland", "il": "Israel", "it": "Italy", "jp": "Japan",
    "kz": "Kazakhstan", "lv": "Latvia", "lt": "Lithuania", "lu": "Luxembourg",
    "my": "Malaysia", "mx": "Mexico", "md": "Moldova", "nl": "Netherlands",
    "nz": "New Zealand", "mk": "North Macedonia", "no": "Norway",
    "pa": "Panama", "py": "Paraguay", "ph": "Philippines", "pl": "Poland",
    "pt": "Portugal", "ro": "Romania", "rs": "Serbia", "sg": "Singapore",
    "sk": "Slovakia", "si": "Slovenia", "za": "South Africa", "kr": "South Korea",
    "es": "Spain", "se": "Sweden", "ch": "Switzerland", "tw": "Taiwan",
    "th": "Thailand", "tr": "Turkey", "ua": "Ukraine", "ae": "United Arab Emirates",
    "uk": "United Kingdom", "gb": "United Kingdom", "us": "United States",
    "vn": "Vietnam",
}


def is_wireguard_key(text: str) -> bool:
    """44 base64 characters ending in `=` — a Curve25519 key. The same check
    the root helper makes (`ronoctua_security::vpn::is_wireguard_key`)."""
    if len(text) != 44 or not text.endswith("="):
        return False
    try:
        return len(base64.b64decode(text, validate=True)) == 32
    except (ValueError, Exception):
        return False


def label_of(ident: str) -> str:
    """"United States — NYC" from "us-nyc"; the id itself when it is not that
    shape."""
    parts = ident.split("-", 1)
    if len(parts) == 2 and parts[0] in COUNTRIES:
        return f"{COUNTRIES[parts[0]]} — {parts[1].upper()}"
    return ident


def parse_conf(text: str) -> dict | None:
    """The public facts of a WireGuard `.conf`: address, DNS, public key,
    allowed IPs, endpoint. The private key is read past. `None` when it is not
    a WireGuard file or names no peer."""
    if "[Interface]" not in text:
        return None
    section = None
    address4 = None
    address6 = None
    dns: list[str] = []
    public_key = None
    allowed: list[str] = []
    endpoint = None
    for raw in text.splitlines():
        line = raw.strip()
        low = line.lower()
        if low == "[interface]":
            section = "interface"
            continue
        if low == "[peer]":
            section = "peer"
            continue
        if "=" not in line or line.startswith("#"):
            continue
        key, value = (p.strip() for p in line.split("=", 1))
        key = key.lower()
        if section == "interface":
            if key == "address":
                for part in (p.strip() for p in value.split(",")):
                    if ":" in part:
                        address6 = part
                    elif part:
                        address4 = part
            elif key == "dns":
                dns = [d.strip() for d in value.split(",") if d.strip()]
            # PrivateKey is read past — the module header says why.
        elif section == "peer":
            if key == "publickey":
                public_key = value
            elif key == "allowedips":
                allowed = [a.strip() for a in value.split(",") if a.strip()]
            elif key == "endpoint":
                endpoint = value
    if not public_key or not endpoint:
        return None
    return {
        "address4": address4 or "10.14.0.2/16",
        "address6": address6 or ULA6,
        "dns": dns,
        "publicKey": public_key,
        "allowedIPs": allowed,
        "endpoint": endpoint,
    }


def with_ipv6(allowed: list[str]) -> list[str]:
    """The allowed IPs, with the two defaults ensured — IPv4 as the file has
    it, IPv6 added so the leak is closed. The sync still gates IPv6 on the
    module's option; this keeps the fact in the registered file."""
    out = list(allowed)
    for net in ("0.0.0.0/0", "::/0"):
        if net not in out:
            out.append(net)
    return out


def write_private(path: Path, data: bytes) -> None:
    """`0600` from creation, replacing what was there."""
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        os.write(fd, data)
    finally:
        os.close(fd)
    os.chmod(path, 0o600)


def main(argv: list[str]) -> int:
    if len(argv) != 1 or argv[0] in ("-h", "--help"):
        print(
            "usage: nix run .#vpn-import -- <folder>\n"
            "Registers every WireGuard .conf in <folder> at once, asks the\n"
            "private key once, adds IPv6 so the IP does not leak, and installs\n"
            "them. The provider's placeholder key is ignored.",
            file=sys.stderr,
        )
        return 2
    folder = Path(argv[0]).expanduser()
    if not folder.is_dir():
        print(f"vpn-import: {folder} is not a folder", file=sys.stderr)
        return 1
    if not STAGING_DIR.is_dir():
        print(
            f"vpn-import: {STAGING_DIR} is not there — is ronoctua.vpn enabled on this machine?",
            file=sys.stderr,
        )
        return 1

    confs = sorted(folder.glob("*.conf"))
    if not confs:
        print(f"vpn-import: no .conf files in {folder}", file=sys.stderr)
        return 1

    locations = []
    skipped = []
    for conf in confs:
        ident = conf.stem
        try:
            text = conf.read_text("utf-8")
        except OSError as e:
            skipped.append((ident, f"unreadable: {e}"))
            continue
        facts = parse_conf(text)
        if facts is None:
            skipped.append((ident, "not a WireGuard file, or names no peer"))
            continue
        locations.append(
            {
                "id": ident,
                "name": label_of(ident),
                "endpoint": facts["endpoint"],
                "publicKey": facts["publicKey"],
                "dns": facts["dns"],
                "address4": facts["address4"],
                "address6": facts["address6"],
                "allowedIPs": with_ipv6(facts["allowedIPs"]),
            }
        )

    if not locations:
        print("vpn-import: no WireGuard servers found to register", file=sys.stderr)
        for ident, why in skipped:
            print(f"  skipped {ident}: {why}", file=sys.stderr)
        return 1

    # THE KEY, ONCE — for every server at once. It is never echoed, never an
    # argument, and leaves this process only as a `0600` staged file.
    print(f"Found {len(locations)} server(s) in {folder}.")
    key = getpass.getpass("WireGuard private key (inserted once, for all of them): ").strip()
    if not is_wireguard_key(key):
        print("vpn-import: that is not a WireGuard private key (44 base64 characters)", file=sys.stderr)
        return 1

    doc = {
        "format": 1,
        "provider": "surfshark",
        "locations": locations,
    }
    try:
        write_private(KEY_FILE, (key + "\n").encode("utf-8"))
        write_private(LOCATIONS_FILE, (json.dumps(doc, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    except OSError as e:
        print(f"vpn-import: could not stage the files in {STAGING_DIR}: {e}", file=sys.stderr)
        return 1

    # ROOT TAKES IT FROM HERE, through the one narrow polkit rule: it seals the
    # key and installs the locations; the sync then writes the profiles.
    try:
        result = subprocess.run(
            ["systemctl", "start", INSTALL_UNIT],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as e:
        print(f"vpn-import: could not start {INSTALL_UNIT}: {e}", file=sys.stderr)
        return 1
    finally:
        # The staged copies are the install's to spend; delete any the install
        # left, so a refused run leaves no key behind.
        for path in (KEY_FILE, LOCATIONS_FILE):
            try:
                path.unlink()
            except FileNotFoundError:
                pass

    if result.returncode != 0:
        print(f"vpn-import: {INSTALL_UNIT} refused:\n{result.stderr.strip()}", file=sys.stderr)
        return 1

    print(f"Registered {len(locations)} server(s). Raise one from the network view.")
    for ident, why in skipped:
        print(f"  skipped {ident}: {why}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
