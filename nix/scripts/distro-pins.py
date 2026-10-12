#!/usr/bin/env python3
"""Moves the pins of the distro's own AppImages and Android apps.

    nix run .#distro-pins            write assets/distro-pins.json
    nix run .#distro-pins -- --check say what would move; exit 1 if anything would,
                                     or if the file is out of shape (asked first, offline)

Run from the root of a checkout. What moves:

- an AppImage that FOLLOWS a GitHub repository (`follows.github` in
  `assets/distro-apps.nix`): its latest release's one asset matching
  `assetPattern`, hashed by `nix store prefetch-file`;
- an Android app from F-Droid (`from = "f-droid"`): the version F-Droid
  suggests, its apk hashed the same way.

What does not move: an entry that follows nothing (`follows = null`, `from =
"direct"`) keeps the pin written by hand, and one without a pin is refused
with its id. A pin whose entry left the table is dropped. Nothing is
committed: review `git diff assets/distro-pins.json` and commit it by hand.

The network questions go through one object (`Ask`), so the planning is a
pure function a check runs offline (`distro-pins-move-only-what-follows`).
"""

import argparse
import datetime
import fnmatch
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import urllib.request

TABLE = "assets/distro-apps.nix"
PINS = "assets/distro-pins.json"
FORMAT = 1
# GitHub refuses a request without one. Not the distro's name: that is
# defined in one place (invariant 16), and a tool's own name says enough.
USER_AGENT = "distro-pins"


class Refusal(Exception):
    """A pin this tool will not write, with the sentence that says why."""


def stamp(now):
    """The person's data's stamp: exactly `YYYY-MM-DDTHH:MM:SSZ`, in UTC."""
    return now.astimezone(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def version_of_tag(tag):
    """`v1.2.3` is version `1.2.3`; any other tag is its own version."""
    if len(tag) > 1 and tag[0] in "vV" and tag[1].isdigit():
        return tag[1:]
    return tag


def pick_asset(id_, assets, pattern):
    """The ONE asset of a release whose name matches the pattern."""
    hits = [a for a in assets if fnmatch.fnmatchcase(a.get("name", ""), pattern)]
    if len(hits) != 1:
        names = ", ".join(sorted(a.get("name", "?") for a in hits)) or "none"
        raise Refusal(
            f"{id_}: the latest release has {len(hits)} assets matching `{pattern}` "
            f"({names}); the pattern has to name exactly one"
        )
    url = hits[0].get("browser_download_url", "")
    if not url.startswith("https://"):
        raise Refusal(f"{id_}: the asset's address is not https: {url!r}")
    return hits[0]["name"], url


def plan(table, pins, ask):
    """The next pins, and a note for every entry. Pure: every question that
    needs the network is `ask`'s (`github_latest`, `fdroid`, `hash`, `now`).

    `table` is the `apps` of `assets/distro-apps.nix`; `pins` is the current
    `assets/distro-pins.json`. A hash is asked only for an address that is
    new: an unchanged release is never downloaded again."""
    if pins.get("format") != FORMAT:
        raise Refusal(f"{PINS} says format {pins.get('format')!r}; this tool writes {FORMAT}")
    old_appimage = pins.get("appimage", {})
    old_android = pins.get("android", {})
    new = {"format": FORMAT, "appimage": {}, "android": {}}
    notes = []

    for id_ in sorted(table):
        entry = table[id_]
        source = entry.get("source")
        if source == "appimage":
            old = old_appimage.get(id_)
            follows = (entry.get("follows") or {}).get("github")
            if not follows:
                if old is None:
                    raise Refusal(
                        f"{id_}: follows nothing and has no pin — write its pin by hand "
                        f"(`nix store prefetch-file --json <url>` gives the hash)"
                    )
                new["appimage"][id_] = old
                notes.append(f"{id_}: kept (follows nothing)")
                continue
            release = ask.github_latest(follows["repo"])
            tag = release.get("tag_name", "")
            # A TAG WITH NO NUMBER MOVES (`continuous`, `nightly`, `stable`):
            # its address serves new bytes under the same name, and the hash
            # pinned today fails the day the store copy is collected — the
            # defect `downloads-are-pinned` refuses for every download.
            if not any(c.isdigit() for c in tag):
                raise Refusal(
                    f"{id_}: the latest release of {follows['repo']} is tagged `{tag}`, a tag "
                    f"that moves; follow a repository whose releases carry their version"
                )
            _, url = pick_asset(id_, release.get("assets", []), follows["assetPattern"])
            version = version_of_tag(tag)
            if old is not None and old.get("url") == url:
                new["appimage"][id_] = old
                notes.append(f"{id_}: unchanged ({old.get('version')})")
                continue
            new["appimage"][id_] = {
                "url": url,
                "hash": ask.hash(url),
                "version": version,
                "pinned-at": stamp(ask.now()),
            }
            notes.append(
                f"{id_}: pinned {version}"
                if old is None
                else f"{id_}: {old.get('version')} -> {version}"
            )
        elif source == "android":
            old = old_android.get(id_)
            if entry.get("from") != "f-droid":
                if old is None:
                    raise Refusal(
                        f"{id_}: comes from a direct address and has no pin — write its pin by hand"
                    )
                new["android"][id_] = old
                notes.append(f"{id_}: kept (a direct address)")
                continue
            package = entry["package"]
            info = ask.fdroid(package)
            code = info.get("suggestedVersionCode")
            if not isinstance(code, int) or code <= 0:
                raise Refusal(f"{id_}: F-Droid suggests no version of {package}")
            names = [
                p.get("versionName")
                for p in info.get("packages", [])
                if p.get("versionCode") == code
            ]
            if not names or not names[0]:
                raise Refusal(f"{id_}: F-Droid lists no name for version {code} of {package}")
            url = f"https://f-droid.org/repo/{package}_{code}.apk"
            if old is not None and old.get("url") == url and old.get("version-code") == code:
                new["android"][id_] = old
                notes.append(f"{id_}: unchanged ({old.get('version')})")
                continue
            new["android"][id_] = {
                "url": url,
                "hash": ask.hash(url),
                "version": names[0],
                "version-code": code,
            }
            notes.append(
                f"{id_}: pinned {names[0]}"
                if old is None
                else f"{id_}: {old.get('version')} -> {names[0]}"
            )

    for kind, old in (("appimage", old_appimage), ("android", old_android)):
        for id_ in sorted(set(old) - set(new[kind])):
            notes.append(f"{id_}: dropped (the table has no {kind} by that id)")
    return new, notes


def text_of(pins):
    """The file's text: `format` first, ids in order, two spaces, and a
    newline at the end — the same bytes for the same pins."""
    return json.dumps(pins, indent=2, ensure_ascii=False) + "\n"


# The fields of a pin, by kind: what `plan` writes, in its order.
PIN_FIELDS = {
    "appimage": ["url", "hash", "version", "pinned-at"],
    "android": ["url", "hash", "version", "version-code"],
}


def follows_a_source(entry):
    """Whether this tool moves the entry's pin (else it is moved by hand)."""
    if entry.get("source") == "appimage":
        return bool((entry.get("follows") or {}).get("github"))
    return entry.get("source") == "android" and entry.get("from") == "f-droid"


def offline_problems(table, text):
    """What is wrong with the pins file's TEXT against the table, asking
    nothing of the network: the file is in this tool's own form, every pin
    belongs to an entry of its kind and has a pin's fields, and every entry
    of a pinned kind has its pin — moved by this tool when it follows a
    source, written by hand when it does not. A sentence per problem."""
    wrong = []
    try:
        pins = json.loads(text)
    except ValueError as why:
        return [f"{PINS} is not JSON: {why}"]
    if not isinstance(pins, dict) or sorted(pins) != ["android", "appimage", "format"]:
        return [f"{PINS} is not `format`, `appimage` and `android`"]
    if pins["format"] != FORMAT:
        wrong.append(f"{PINS} says format {pins['format']!r}; this tool writes {FORMAT}")
    if text_of(pins) != text:
        wrong.append(f"{PINS} is not in this tool's form (`nix run .#distro-pins` rewrites it)")
    for kind, fields in PIN_FIELDS.items():
        pinned = pins[kind]
        if not isinstance(pinned, dict):
            wrong.append(f"{PINS}: `{kind}` is not a table of pins")
            continue
        for id_ in sorted(pinned):
            if (table.get(id_) or {}).get("source") != kind:
                wrong.append(f"{PINS}: `{id_}` is pinned as {kind}, and the table has no {kind} by that id")
            pin = pinned[id_]
            if not isinstance(pin, dict) or list(pin) != fields:
                wrong.append(f"{PINS}: the pin of `{id_}` does not have the fields {', '.join(fields)}")
        for id_ in sorted(i for i, e in table.items() if e.get("source") == kind):
            if id_ in pinned:
                continue
            how = "run `nix run .#distro-pins`" if follows_a_source(table[id_]) else "write it by hand"
            wrong.append(f"`{id_}` has no pin in {PINS}: {how}")
    return wrong


class Ask:
    """The network, for real."""

    def _get(self, url):
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        # A TOKEN FROM THE ENVIRONMENT, never from the command line: GitHub
        # answers sixty unauthenticated questions an hour.
        token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
        if token and url.startswith("https://api.github.com/"):
            request.add_header("Authorization", f"Bearer {token}")
        with urllib.request.urlopen(request, timeout=60) as answer:
            return json.load(answer)

    def github_latest(self, repo):
        return self._get(f"https://api.github.com/repos/{repo}/releases/latest")

    def fdroid(self, package):
        return self._get(f"https://f-droid.org/api/v1/packages/{package}")

    def hash(self, url):
        out = subprocess.run(
            ["nix", "store", "prefetch-file", "--json", "--hash-type", "sha256", url],
            check=True,
            capture_output=True,
            text=True,
        )
        return json.loads(out.stdout)["hash"]

    def now(self):
        return datetime.datetime.now(datetime.timezone.utc)


def main(argv):
    parser = argparse.ArgumentParser(
        prog="distro-pins",
        description="Moves the pins of the distro's own AppImages and Android apps.",
    )
    parser.add_argument("--check", action="store_true", help="say what would move; write nothing")
    parser.add_argument("--root", default=".", help="the checkout (default: here)")
    args = parser.parse_args(argv)

    root = pathlib.Path(args.root)
    if not (root / "flake.nix").is_file() or not (root / TABLE).is_file():
        print(f"distro-pins: {root.resolve()} is not a checkout of the distro (no {TABLE})", file=sys.stderr)
        return 2
    table = json.loads(
        subprocess.run(
            ["nix", "eval", "--json", "--file", str(root / TABLE), "apps"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    pins_path = root / PINS
    before = pins_path.read_text()
    if args.check:
        # FIRST WHAT NEEDS NO NETWORK: a file a hand edited out of shape, or
        # an entry without its pin, is said before anything is asked.
        problems = offline_problems(table, before)
        for problem in problems:
            print(f"distro-pins: {problem}", file=sys.stderr)
        if problems:
            return 1
    try:
        new, notes = plan(table, json.loads(before), Ask())
    except Refusal as why:
        print(f"distro-pins: {why}", file=sys.stderr)
        return 1
    for note in notes:
        print(note)
    after = text_of(new)
    if after == before:
        print("Nothing moved.")
        return 0
    if args.check:
        print(f"{PINS} would change.")
        return 1
    # WRITTEN WHOLE: a draft beside the file, renamed over it.
    fd, draft = tempfile.mkstemp(prefix=".distro-pins.", dir=pins_path.parent)
    with os.fdopen(fd, "w") as f:
        f.write(after)
    os.chmod(draft, 0o644)
    os.replace(draft, pins_path)
    print(f"Wrote {PINS}. Review it with `git diff {PINS}`, then commit it by hand.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
