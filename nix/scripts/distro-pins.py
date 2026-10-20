#!/usr/bin/env python3
"""Moves the pins of the distro's own AppImages and Android apps.

    nix run .#distro-pins            write assets/distro-pins.json
    nix run .#distro-pins -- --check say what would move; exit 1 if anything would,
                                     or if the file is out of shape (asked first, offline)
    nix run .#distro-pins -- --stale say which pins are behind their latest release,
                                     asking only read-only questions: nothing is
                                     downloaded or written, and a question the
                                     network does not answer is said, never a failure
                                     (what `nix run .#release` says before it publishes)

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
import http.client
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


def file_name_of(entry, version):
    """The store name a pinned file is downloaded under: the one the build's
    `fetchurl` gives it (`nix/distro-downloads.nix`), so the download that
    hashes a release is the build's source and nothing is fetched twice.
    `version` is the AppImage's version, or the apk's version code."""
    if entry.get("source") == "appimage":
        return f"{entry['key']}-{version}.AppImage"
    return f"{entry['package']}_{version}.apk"


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


def latest_appimage(id_, follows, ask):
    """The latest release of the repository an AppImage follows: the address
    of its one asset matching the pattern, and its version. A tag with no
    digit is refused: it moves."""
    release = ask.github_latest(follows["repo"])
    tag = release.get("tag_name", "")
    # A TAG WITH NO NUMBER MOVES (`continuous`, `nightly`, `stable`): its
    # address serves new bytes under the same name, and the hash pinned
    # today fails the day the store copy is collected — the defect
    # `downloads-are-pinned` refuses for every download.
    if not any(c.isdigit() for c in tag):
        raise Refusal(
            f"{id_}: the latest release of {follows['repo']} is tagged `{tag}`, a tag "
            f"that moves; follow a repository whose releases carry their version"
        )
    _, url = pick_asset(id_, release.get("assets", []), follows["assetPattern"])
    return url, version_of_tag(tag)


def latest_apk(id_, package, ask):
    """The version F-Droid suggests of a package: its apk's address, its
    name and its code."""
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
    return f"https://f-droid.org/repo/{package}_{code}.apk", names[0], code


def plan(table, pins, ask):
    """The next pins, and a note for every entry. Pure: every question that
    needs the network is `ask`'s (`github_latest`, `fdroid`, `hash`, `now`).

    `table` is the `apps` of `assets/distro-apps.nix`; `pins` is the current
    `assets/distro-pins.json`. A hash is asked only for an address that is
    new: an unchanged release is never downloaded again, and the one download
    is named as the build names it (`file_name_of`)."""
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
            url, version = latest_appimage(id_, follows, ask)
            if old is not None and old.get("url") == url:
                new["appimage"][id_] = old
                notes.append(f"{id_}: unchanged ({old.get('version')})")
                continue
            new["appimage"][id_] = {
                "url": url,
                "hash": ask.hash(url, file_name_of(entry, version)),
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
            url, name, code = latest_apk(id_, entry["package"], ask)
            if old is not None and old.get("url") == url and old.get("version-code") == code:
                new["android"][id_] = old
                notes.append(f"{id_}: unchanged ({old.get('version')})")
                continue
            new["android"][id_] = {
                "url": url,
                "hash": ask.hash(url, file_name_of(entry, code)),
                "version": name,
                "version-code": code,
            }
            notes.append(
                f"{id_}: pinned {name}"
                if old is None
                else f"{id_}: {old.get('version')} -> {name}"
            )

    for kind, old in (("appimage", old_appimage), ("android", old_android)):
        for id_ in sorted(set(old) - set(new[kind])):
            notes.append(f"{id_}: dropped (the table has no {kind} by that id)")
    return new, notes


def version_components(version):
    """A version cut as Nix's `builtins.compareVersions` cuts it: runs of
    digits and runs of other characters, `.` and `-` only separating."""
    out, i = [], 0
    while i < len(version):
        if version[i] in ".-":
            i += 1
            continue
        j = i
        digits = version[i].isdigit()
        while j < len(version) and version[j] not in ".-" and version[j].isdigit() == digits:
            j += 1
        out.append(version[i:j])
        i = j
    return out


def compare_versions(a, b):
    """-1, 0 or 1, as Nix's `builtins.compareVersions` answers — the rule the
    build follows: numbers by value, `pre` before anything, a missing part
    before a number, and a word before a number (`2.3a` < `2.3.1`)."""

    def less(c1, c2):
        n1 = int(c1) if c1.isdigit() else None
        n2 = int(c2) if c2.isdigit() else None
        if n1 is not None and n2 is not None:
            return n1 < n2
        if c1 == "" and n2 is not None:
            return True
        if c1 == "pre" and c2 != "pre":
            return True
        if c2 == "pre":
            return False
        if n2 is not None:
            return True
        if n1 is not None:
            return False
        return c1 < c2

    x, y = version_components(a), version_components(b)
    for k in range(max(len(x), len(y))):
        c1 = x[k] if k < len(x) else ""
        c2 = y[k] if k < len(y) else ""
        if less(c1, c2):
            return -1
        if less(c2, c1):
            return 1
    return 0


# WHAT THE NETWORK MAY ANSWER WITH, and only that: no route, a refusal, a
# timeout (`OSError`, `urllib`'s errors among them), a cut answer, or one
# that is not JSON (`ValueError`). Anything else is a defect of this tool,
# and goes up as one — never said as silence.
NETWORK_ERRORS = (OSError, ValueError, http.client.HTTPException)


def stale(table, pins, ask):
    """Which pins are OLDER than the latest release of what their entry
    follows, and which are newer than it.

    READ-ONLY: only `github_latest` and `fdroid` are asked — never `hash`, so
    nothing is downloaded —, nothing is written, and a pin is never moved.
    Answers `(behind, ahead, unasked)`: `behind` and `ahead` lists of `(id,
    name, pinned, latest)` — by `compare_versions`, the build's rule, and an
    apk by its version code —, `unasked` a sentence for each entry whose
    latest release could not be told (the network, a tag that moves, a
    pattern that is not one asset). What `nix run .#release` says before it
    publishes; moving a pin is the owner's (`nix run .#distro-pins` and a
    commit)."""
    behind, ahead, unasked = [], [], []
    pins = pins if isinstance(pins, dict) else {}
    for id_ in sorted(table):
        entry = table[id_]
        if not follows_a_source(entry):
            continue
        kind = entry.get("source")
        old = (pins.get(kind) or {}).get(id_) or {}
        try:
            if kind == "appimage":
                _, latest = latest_appimage(id_, entry["follows"]["github"], ask)
                order = compare_versions(old["version"], latest) if old.get("version") else -1
            else:
                _, latest, code = latest_apk(id_, entry["package"], ask)
                pinned_code = old.get("version-code")
                order = -1 if not isinstance(pinned_code, int) else (pinned_code > code) - (pinned_code < code)
        except Refusal as why:
            unasked.append(str(why))
            continue
        except NETWORK_ERRORS as why:
            unasked.append(f"{id_}: its latest release could not be asked ({why})")
            continue
        row = (id_, entry.get("name") or id_, old.get("version") or "nothing", latest)
        if order < 0:
            behind.append(row)
        elif order > 0:
            ahead.append(row)
    return behind, ahead, unasked


def stale_lines(behind, ahead, unasked):
    """What `--stale` prints: each pin behind, by name and both versions, how
    to move them, each pin newer than its latest release, and each entry
    that could not be asked."""
    def counted(rows, what):
        one = len(rows) == 1
        return f"{len(rows)} of the distro's pins {'is' if one else 'are'} {what} {'its' if one else 'their'} latest release:"

    lines = []
    if behind:
        lines.append(counted(behind, "behind"))
        lines += [f"  {name} ({id_}): {pinned} pinned, {latest} released" for id_, name, pinned, latest in behind]
        lines.append(f"to move them: nix run .#distro-pins, review git diff {PINS}, and commit")
    if ahead:
        lines.append(counted(ahead, "newer than"))
        lines += [f"  {name} ({id_}): {pinned} pinned, {latest} the latest" for id_, name, pinned, latest in ahead]
    if not behind and not ahead and not unasked:
        lines.append("every pin that follows a release is at its latest")
    lines += [f"not asked: {why}" for why in unasked]
    return lines


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

    def __init__(self, timeout_s=60):
        self.timeout_s = timeout_s

    def _get(self, url):
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        # A TOKEN FROM THE ENVIRONMENT, never from the command line: GitHub
        # answers sixty unauthenticated questions an hour.
        token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
        if token and url.startswith("https://api.github.com/"):
            request.add_header("Authorization", f"Bearer {token}")
        with urllib.request.urlopen(request, timeout=self.timeout_s) as answer:
            return json.load(answer)

    def github_latest(self, repo):
        return self._get(f"https://api.github.com/repos/{repo}/releases/latest")

    def fdroid(self, package):
        return self._get(f"https://f-droid.org/api/v1/packages/{package}")

    def hash(self, url, name):
        out = subprocess.run(
            ["nix", "store", "prefetch-file", "--json", "--hash-type", "sha256", "--name", name, url],
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
    which = parser.add_mutually_exclusive_group()
    which.add_argument("--check", action="store_true", help="say what would move; write nothing")
    which.add_argument(
        "--stale",
        action="store_true",
        help="say which pins are behind their latest release; download, write and move nothing",
    )
    parser.add_argument("--root", default=".", help="the checkout (default: here)")
    args = parser.parse_args(argv)

    root = pathlib.Path(args.root)
    if not (root / "flake.nix").is_file() or not (root / TABLE).is_file():
        print(f"distro-pins: {root.resolve()} is not a checkout of the distro (no {TABLE})", file=sys.stderr)
        return 2
    try:
        table = json.loads(
            subprocess.run(
                ["nix", "eval", "--json", "--file", str(root / TABLE), "apps"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout
        )
    except subprocess.CalledProcessError as why:
        print(f"distro-pins: {TABLE} did not evaluate: {why.stderr.strip()}", file=sys.stderr)
        return 1
    pins_path = root / PINS
    before = pins_path.read_text()
    if args.stale:
        # READ-ONLY, and never a failure for what the network does not
        # answer: the release asks it before it publishes, and goes on.
        try:
            pinned = json.loads(before)
        except ValueError as why:
            print(f"distro-pins: {PINS} is not JSON: {why}", file=sys.stderr)
            return 1
        for line in stale_lines(*stale(table, pinned, Ask(timeout_s=20))):
            print(line)
        return 0
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
