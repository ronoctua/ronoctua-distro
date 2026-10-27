#!/usr/bin/env python3
"""NetworkManager's WireGuard profiles, from the registered locations.

Root's half of the register (`nix/modules/vpn.nix`'s `ronoctua-vpn-sync`): it
reads the public locations the import wrote (`RONOCTUA_VPN_LOCATIONS`), writes
one keyfile per server under `RONOCTUA_VPN_OUT`, `0600`, prunes the ones no
longer named, and reloads NetworkManager (`RONOCTUA_NMCLI`).

NO KEY IS EVER HERE. Each profile carries `private-key-flags=1` and no key of
its own; NetworkManager asks the agent for it at activation
(`ronoctua_network::agent`). The IPv6 leak guard (`RONOCTUA_VPN_IPV6`) and the
DNS override (`RONOCTUA_VPN_DNS`) are baked in as the files are written.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import uuid
from pathlib import Path

# The id a profile's name begins with, so the sync owns exactly its own files
# and prunes none of anybody's.
PREFIX = "ronoctua-vpn-"
# The interface a location takes. `rvpn-<id>`, and the kill switch's `rvpn-*`
# rule is written against this (`nix/modules/vpn.nix`).
IFACE_PREFIX = "rvpn-"
# A Linux interface name is at most 15 bytes; past that the id is replaced by a
# short hash of itself, so two long ids never collide on one name.
IFACE_MOST = 15

# A fixed namespace, so a location's uuid is the same on every regeneration and
# NetworkManager keeps one profile per server rather than making a new one each
# time. Not a secret, not personal — one constant of this distro.
NAMESPACE = uuid.UUID("5f3a9d2c-1b64-4e7a-8c0d-6a2f1e4b7c90")


def env(name: str, default: str = "") -> str:
    return os.environ.get(name, default)


def iface_of(ident: str) -> str:
    name = IFACE_PREFIX + ident
    if len(name) <= IFACE_MOST:
        return name
    # A short hash of the id keeps the name unique and within the limit.
    digest = uuid.uuid5(NAMESPACE, ident).hex[:8]
    return IFACE_PREFIX + digest


def profile(location: dict, ipv6: bool, dns_override: list[str]) -> str:
    """One `.nmconnection`'s text for a location."""
    ident = location["id"]
    pub = location["publicKey"]
    endpoint = location["endpoint"]
    address4 = location.get("address4", "10.14.0.2/16")
    address6 = location.get("address6", "fdce:1a2b:3c4d::2/64")
    dns = dns_override or location.get("dns", [])

    allowed = ["0.0.0.0/0"] + (["::/0"] if ipv6 else [])
    uid = uuid.uuid5(NAMESPACE, ident)

    lines = [
        "[connection]",
        f"id={PREFIX}{ident}",
        f"uuid={uid}",
        "type=wireguard",
        f"interface-name={iface_of(ident)}",
        # The person raises a location from the view; it never comes up on its
        # own (one would at boot, before anyone chose it).
        "autoconnect=false",
        "",
        "[wireguard]",
        # THE KEY IS THE AGENT'S: flags=1 (agent-owned), and no `private-key`
        # line. This is the whole of "the key is in no keyfile".
        "private-key-flags=1",
        "",
        f"[wireguard-peer.{pub}]",
        f"endpoint={endpoint}",
        "allowed-ips=" + ";".join(allowed) + ";",
        "",
        "[ipv4]",
        f"address1={address4}",
        "method=manual",
    ]
    if dns:
        lines.append("dns=" + ";".join(dns) + ";")
        # The tunnel's DNS wins, so names are not looked up around it.
        lines.append("dns-priority=-1")
    lines.append("")
    lines.append("[ipv6]")
    if ipv6:
        lines.append(f"address1={address6}")
        lines.append("method=manual")
        if dns:
            lines.append("dns-priority=-1")
    else:
        # IPv6 LEFT TO THE ORDINARY ROUTE — the leak the `ipv6` option warns of.
        lines.append("method=ignore")
    lines.append("")
    return "\n".join(lines)


def write_profile(out: Path, name: str, text: str) -> None:
    """Writes `0600` from creation, replacing what was there."""
    path = out / f"{name}.nmconnection"
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        os.write(fd, text.encode("utf-8"))
    finally:
        os.close(fd)
    # A profile left from a wider umask before this run is narrowed.
    os.chmod(path, 0o600)


def main() -> int:
    locations_path = Path(env("RONOCTUA_VPN_LOCATIONS"))
    out = Path(env("RONOCTUA_VPN_OUT", "/etc/NetworkManager/system-connections"))
    nmcli = env("RONOCTUA_NMCLI", "nmcli")
    ipv6 = env("RONOCTUA_VPN_IPV6", "1") == "1"
    dns_override = [d for d in env("RONOCTUA_VPN_DNS").split(",") if d]

    locations: list[dict] = []
    if locations_path.is_file():
        try:
            doc = json.loads(locations_path.read_text("utf-8"))
        except (OSError, ValueError) as e:
            print(f"ronoctua-vpn-sync: {locations_path} is not readable JSON: {e}", file=sys.stderr)
            return 1
        if doc.get("format") != 1 or not isinstance(doc.get("locations"), list):
            print(f"ronoctua-vpn-sync: {locations_path} is not the shape this distro reads", file=sys.stderr)
            return 1
        locations = doc["locations"]

    out.mkdir(parents=True, exist_ok=True)

    wanted: set[str] = set()
    for loc in locations:
        if not all(k in loc for k in ("id", "publicKey", "endpoint")):
            print(f"ronoctua-vpn-sync: a location is missing id/publicKey/endpoint; skipped", file=sys.stderr)
            continue
        name = f"{PREFIX}{loc['id']}"
        write_profile(out, name, profile(loc, ipv6, dns_override))
        wanted.add(name)

    # PRUNE what is no longer registered — only this module's own files.
    pruned = 0
    for path in out.glob(f"{PREFIX}*.nmconnection"):
        if path.stem not in wanted:
            try:
                path.unlink()
                pruned += 1
            except OSError as e:
                print(f"ronoctua-vpn-sync: could not remove {path}: {e}", file=sys.stderr)

    # NetworkManager rereads the folder. A machine where the daemon is not up
    # yet is not a failure: it reads the folder when it starts.
    try:
        subprocess.run([nmcli, "connection", "reload"], check=False, stdout=subprocess.DEVNULL)
    except OSError as e:
        print(f"ronoctua-vpn-sync: could not reload NetworkManager: {e}", file=sys.stderr)

    print(f"ronoctua-vpn-sync: {len(wanted)} profile(s) written, {pruned} pruned")
    return 0


if __name__ == "__main__":
    sys.exit(main())
