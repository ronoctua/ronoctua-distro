"""The release's arithmetic, apart from the programs it drives.

`nix run .#release` (`nix/release.nix`) moves branches and talks to GitHub;
what it decides — how the ISO travels, what the notes say — is here, where a
check can run it on a fixture. The distro has no binary cache of its own (its
ISO carries its seed); what a person's own cache may receive is
`ronoctua_cachix::closure`'s. `nix run .#fetch-iso` is the other half of the
same contract and lives here too, so the two cannot disagree about a name.

THE CONTRACT (`ronoctua-stick`'s `release.rs` reads the same one):

- the ISO whole, `ronoctua-distro-<label>-x86_64.iso`, while it is smaller
  than one asset may be; otherwise in parts, `<that name>.part-NN`, numbered
  from 00 with no gap, two digits (three past a hundred);
- `SHA256SUMS`, as `sha256sum` writes it: one line per part AND one for the
  whole ISO, so a part that is right on its own but joined in the wrong place
  is still caught.

Subcommands:

  pack ISO DIR [--most-bytes N] [--part-bytes N]
  notes --repository OWNER/NAME --tag T --rev R --date D --assets DIR [--signed]
  fetch --repository OWNER/NAME [--tag T] [--from DIR] [--into DIR]
  menu-room --profile PROFILE --toplevel T --last-good L --limit N
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

# GITHUB REFUSES AN ASSET OF 2 GiB OR MORE. The decimal reading of "2 GB" is
# the smaller one, so a whole ISO is published only below it, and a part is
# 1900 MiB: under both readings, with room, and few enough parts that the
# stick's queue does not restart its bar more often than it must.
MOST_BYTES = 2_000_000_000
PART_BYTES = 1900 * 1024 * 1024

SUMS = "SHA256SUMS"
CHUNK = 4 * 1024 * 1024

# WHERE `fetch` ASKS FOR A RELEASE. Another address only for the check that
# serves a release from this machine (`iso-parts-reassemble`), so the
# download, the redirect to the asset host and the resume run as they ship.
API = os.environ.get("RONOCTUA_FETCH_ISO_API", "https://api.github.com")

ISO_NAME = re.compile(r"ronoctua-distro-([A-Za-z0-9._+-]{1,64})-x86_64\.iso")
PART_NAME = re.compile(r"(ronoctua-distro-[A-Za-z0-9._+-]{1,64}-x86_64\.iso)\.part-([0-9]{2,3})")


def refuse(sentence, *more):
    """Says why, on standard error, and stops with 1."""
    print("refused: " + sentence, file=sys.stderr)
    for line in more:
        print("  " + line, file=sys.stderr)
    raise SystemExit(1)


def is_iso_name(name):
    return ISO_NAME.fullmatch(name) is not None


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            block = f.read(CHUNK)
            if not block:
                return digest.hexdigest()
            digest.update(block)


# ---------------------------------------------------------------------------
# pack: the ISO as release assets
# ---------------------------------------------------------------------------


def part_names(iso_name, count):
    """The parts' names, in order."""
    if count > 1000:
        refuse(f"{count} parts: more than three digits can number")
    width = 2 if count <= 100 else 3
    return [f"{iso_name}.part-{n:0{width}d}" for n in range(count)]


def sums_text(lines):
    """`SHA256SUMS`, as `sha256sum` writes it."""
    return "".join(f"{sha}  {name}\n" for sha, name in lines)


def pack(iso, into, most_bytes=MOST_BYTES, part_bytes=PART_BYTES):
    """The ISO in `into`, whole or in parts, and `SHA256SUMS` beside it.

    Answers the assets to upload, in order. Whole, the asset is a link to
    the ISO (nothing is copied); in parts, every part is written once, and
    the parts and the whole are hashed in the same pass.
    """
    iso = Path(iso)
    name = iso.name
    if not is_iso_name(name):
        refuse(f"{name} is not ronoctua-distro-<label>-x86_64.iso", "the stick and fetch-iso read that name")
    if not iso.is_file():
        refuse(f"{iso} is not a file")
    if not 0 < part_bytes < most_bytes:
        refuse(f"a part of {part_bytes} bytes is not smaller than an asset of {most_bytes}")
    into = Path(into)
    into.mkdir(parents=True, exist_ok=True)
    if any(into.iterdir()):
        refuse(f"{into} is not empty", "the assets of one release go in a folder of their own")
    size = iso.stat().st_size
    if size == 0:
        refuse(f"{iso} is empty")

    if size < most_bytes:
        (into / name).symlink_to(iso.resolve())
        (into / SUMS).write_text(sums_text([(sha256_of(iso), name)]))
        return [into / name, into / SUMS]

    count = (size + part_bytes - 1) // part_bytes
    names = part_names(name, count)
    whole = hashlib.sha256()
    lines = []
    with open(iso, "rb") as source:
        for part_name in names:
            digest = hashlib.sha256()
            left = part_bytes
            with open(into / part_name, "xb") as part:
                while left > 0:
                    block = source.read(min(CHUNK, left))
                    if not block:
                        break
                    part.write(block)
                    digest.update(block)
                    whole.update(block)
                    left -= len(block)
            lines.append((digest.hexdigest(), part_name))
        if source.read(1):
            refuse(f"{iso} grew while it was being cut")
    lines.append((whole.hexdigest(), name))
    (into / SUMS).write_text(sums_text(lines))
    return [into / n for n in names] + [into / SUMS]


def mib(n):
    return f"{n / 1048576:.1f} MiB"


# ---------------------------------------------------------------------------
# notes: the release's body, in English
# ---------------------------------------------------------------------------


def notes(repository, tag, rev, date, assets, signed):
    """The notes, written here and never typed into GitHub's form: a body
    nobody can diff is a body nobody reviews (`nix run .#fonts-release` was
    the first to learn it)."""
    assets = Path(assets)
    sums = (assets / SUMS).read_text()
    lines = [line.split("  ", 1) for line in sums.splitlines() if line.strip()]
    whole = [(sha, name) for sha, name in lines if is_iso_name(name)]
    if len(whole) != 1:
        refuse(f"{SUMS} does not name exactly one whole ISO")
    sha, iso = whole[0]
    parts = [(s, n) for s, n in lines if n != iso]
    files = sorted(p.name for p in assets.iterdir())
    size = sum((assets / n).stat().st_size for n in files if n != SUMS)
    out = [
        f"# ronoctua-distro {tag}",
        "",
        f"Built from [`{rev[:12]}`](https://github.com/{repository}/commit/{rev}) ({date}), the",
        "commit the `release` branch now points at: every installed machine moves to it at its",
        "next update, and the installer below installs it.",
        "",
        "## The installer",
        "",
        f"`{iso}`, {mib(size)}, sha256 `{sha}`.",
        "",
    ]
    if parts:
        out += [
            f"GitHub refuses an asset of 2 GiB or more, so the image travels in {len(parts)} parts.",
            "Fetch, check and join them in one step:",
            "",
            "```",
            f"nix run github:{repository}/release#fetch-iso",
            "```",
            "",
            "or by hand, in the folder they were downloaded to:",
            "",
            "```",
            f"sha256sum --check --ignore-missing {SUMS}",
            f"cat {iso}.part-* > {iso}",
            f"sha256sum --check --ignore-missing {SUMS}",
            "```",
        ]
    else:
        out += [
            "Check it before writing it to a stick:",
            "",
            "```",
            f"sha256sum --check --ignore-missing {SUMS}",
            "```",
        ]
    out += [
        "",
        "Write it whole to a USB stick (`dd`, or the installer stick writer of an installed",
        "machine) and boot it: its console says how to start `ronoctua-install`, which rehearses",
        "before it installs. The installer needs the network and the person's private belongings",
        "repository.",
        "",
    ]
    if signed:
        out += [
            f"The checksums are also in the signed tag `{tag}`: `git verify-tag {tag}` in a clone",
            "shows them under the owner's signature.",
            "",
        ]
    out += [
        "## Checksums",
        "",
        "```",
        sums.rstrip("\n"),
        "```",
        "",
        "Licences: MIT for the code (`LICENSE`), SIL Open Font License 1.1 for the fonts, and the",
        "geographic data's own terms, with the attributions they require (`LICENSES/`).",
    ]
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------
# fetch: the ISO back, from a release or a folder
# ---------------------------------------------------------------------------


def parse_sums(text):
    """`SHA256SUMS`, strictly: a line that is not `<64 hex> <space or *><name>`
    refuses the whole file — half understood is checking something else."""
    out = {}
    for line in text.splitlines():
        line = line.rstrip("\r")
        if not line.strip():
            continue
        m = re.fullmatch(r"([0-9a-fA-F]{64}) [ *]([^/\0]+)", line)
        if m is None or m.group(2) in out:
            refuse(f"{SUMS} has a line that is not one: {line!r}")
        out[m.group(2)] = m.group(1).lower()
    return out


def choose(names):
    """From the assets' names, the image: ("whole", iso, [iso]) or
    ("parts", iso, [part, …]) — the same rules the stick reads by."""
    wholes = [n for n in names if is_iso_name(n)]
    parts = {}
    for n in names:
        m = PART_NAME.fullmatch(n)
        if m:
            parts.setdefault(m.group(1), []).append((int(m.group(2)), n))
    if SUMS not in names:
        refuse(f"there is no {SUMS}")
    if len(wholes) == 1 and not parts:
        return "whole", wholes[0], wholes
    if wholes or len(parts) != 1:
        refuse("the release does not hold exactly one ISO, whole or in parts")
    iso, numbered = next(iter(parts.items()))
    numbered.sort()
    first = numbered[0][0]
    if first > 1 or len(numbered) < 2 or [n for n, _ in numbered] != list(range(first, first + len(numbered))):
        refuse(f"the parts of {iso} are not numbered from 00 or 01 with no gap")
    return "parts", iso, [n for _, n in numbered]


def curl(*words):
    """`curl`, by the name the launcher put on the PATH: failing on an HTTP
    error, following GitHub's redirect to its asset host, retrying a dropped
    transfer."""
    return subprocess.run(
        ["curl", "--fail", "--location", "--silent", "--show-error", "--retry", "5", *words],
        check=False,
    ).returncode


def release_of(repository, tag):
    """GitHub's answer for the release (the latest, or `tag`)."""
    which = f"tags/{tag}" if tag else "latest"
    url = f"{API}/repos/{repository}/releases/{which}"
    answer = subprocess.run(
        ["curl", "--fail", "--location", "--silent", "--show-error", "--retry", "3",
         "--header", "Accept: application/vnd.github+json", url],
        capture_output=True,
        check=False,
    )
    if answer.returncode != 0:
        refuse(f"GitHub did not answer for {url}", answer.stderr.decode(errors="replace").strip())
    data = json.loads(answer.stdout)
    if data.get("draft") or data.get("prerelease"):
        refuse(f"{data.get('tag_name')} is a draft or a pre-release")
    return {
        a["name"]: a["browser_download_url"]
        for a in data.get("assets", [])
        if a.get("state", "uploaded") == "uploaded"
    }, data.get("tag_name", tag or "latest")


def fetch(repository, tag=None, source=None, into="."):
    """Downloads (or reads from `source`), checks every part, joins, checks
    the whole, and only then names the ISO. A part that fails its line is
    removed so the next run fetches it again; a download that stopped is
    resumed; the folder of the download goes when the ISO is whole."""
    into = Path(into)
    into.mkdir(parents=True, exist_ok=True)
    if source is not None:
        source = Path(source)
        if not source.is_dir():
            refuse(f"{source} is not a folder")
        located = {p.name: p for p in source.iterdir()}
        label = str(source)
    else:
        urls, label = release_of(repository, tag)
        located = dict(urls)
    kind, iso, wanted = choose(list(located))
    scratch = into / f"{iso}.download"

    def local(name, expect=None):
        """The asset on this disk: as it is in `source`, or downloaded — a
        piece already whole from an earlier run is not fetched again, and
        one that stopped half way is resumed."""
        if source is not None:
            return located[name]
        scratch.mkdir(exist_ok=True)
        at = scratch / name
        if expect is None:
            at.unlink(missing_ok=True)
        elif at.is_file() and sha256_of(at) == expect:
            return at
        had = at.is_file()
        code = curl("--continue-at", "-", "--output", str(at), located[name])
        # WHAT WAS THERE MAY BE WRONG, not only short: a run that died
        # between a download and its check leaves a piece at its full length
        # that is not the release's. Resumed, it stays wrong (or the host
        # refuses the range: an HTTP error, 22, or no ranges at all, 33), so
        # a piece that was there and does not come out right is fetched
        # anew, once. A transfer that dropped keeps what arrived.
        if had and (code in (22, 33) or (code == 0 and expect is not None and sha256_of(at) != expect)):
            at.unlink(missing_ok=True)
            code = curl("--output", str(at), located[name])
        if code != 0:
            refuse(f"{name} did not download", f"run it again: what arrived is kept and resumed ({scratch})")
        return at

    sums = parse_sums(Path(local(SUMS)).read_text())
    if sums.get(iso) is None:
        refuse(f"{SUMS} has no line for {iso}", "a part is checked on its own and the whole again")
    target = into / iso
    if target.exists():
        if sha256_of(target) == sums[iso]:
            print(f"{target} is already here, and it is the release's ({sums[iso]})")
            return target
        refuse(f"{target} exists and is not the release's ISO", "move it away first: this never writes over a file")

    pieces = []
    for name in wanted:
        line = sums.get(name)
        if line is None:
            refuse(f"{SUMS} has no line for {name}")
        at = Path(local(name, line))
        if sha256_of(at) != line:
            if source is None:
                at.unlink()
            refuse(f"{name} is not the release's: its sha256 differs from {SUMS}",
                   "it was removed; run it again to fetch it anew" if source is None else f"in {source}")
        pieces.append(at)
        print(f"checked {name}")

    # WHOLE AND DOWNLOADED, it is already the ISO: moved, not copied.
    if kind == "whole" and source is None:
        os.replace(pieces[0], target)
        shutil.rmtree(scratch)
        print(f"{target}: checked against the {SUMS} of {label} ({sums[iso]})")
        return target

    partial = into / f".{iso}.partial"
    partial.unlink(missing_ok=True)
    whole = hashlib.sha256()
    with open(partial, "xb") as out:
        for at in pieces:
            with open(at, "rb") as f:
                while True:
                    block = f.read(CHUNK)
                    if not block:
                        break
                    out.write(block)
                    whole.update(block)
    if whole.hexdigest() != sums[iso]:
        partial.unlink()
        refuse(f"the parts joined are not {iso}: the whole's sha256 differs from {SUMS}",
               "every part matched its own line, so the parts or their order were swapped")
    os.replace(partial, target)
    if scratch.exists():
        shutil.rmtree(scratch)
    print(f"{target}: whole and checked against the {SUMS} of {label} ({sums[iso]})")
    return target


# ---------------------------------------------------------------------------
# menu-room: the way back stays in the boot menu (`.#apply-here`)
# ---------------------------------------------------------------------------


def falls_out(generations, adds, limit, last_good):
    """Whether making a new generation (`adds`) would push `last_good` out of
    a menu that lists the newest `limit` generations — `ronoctua-machine`'s
    `boot::falls_out`, the guard its `apply` refuses on. A last good system
    the menu does not list NOW is not pushed out by anything."""
    limit = max(1, limit)
    numbers = sorted(n for n, _ in generations)
    ours = [n for n, top in generations if top == last_good]
    listed = lambda ns: ns[max(0, len(ns) - limit):]
    if not any(n in listed(numbers) for n in ours):
        return False
    if adds:
        numbers.append(numbers[-1] + 1 if numbers else 1)
    return not any(n in listed(numbers) for n in ours)


def generations_of(profile):
    """The profile's generations (`<profile>-<n>-link`), by number, each with
    the system it names."""
    profile = Path(profile)
    pattern = re.compile(re.escape(profile.name) + r"-([0-9]+)-link")
    found = []
    for entry in profile.parent.iterdir():
        m = pattern.fullmatch(entry.name)
        if m:
            found.append((int(m.group(1)), os.path.realpath(entry)))
    return sorted(found)


def menu_room(profile, toplevel, last_good, limit):
    """Whether the profile can take `toplevel` and the boot menu still list
    `last_good`. Taking the system the profile already names adds nothing."""
    toplevel, last_good = os.path.realpath(toplevel), os.path.realpath(last_good)
    current = os.path.realpath(profile) if os.path.lexists(profile) else None
    return not falls_out(generations_of(profile), current != toplevel, limit, last_good)


def main(argv):
    p = argparse.ArgumentParser(prog="release.py")
    sub = p.add_subparsers(dest="command", required=True)
    a = sub.add_parser("pack")
    a.add_argument("iso")
    a.add_argument("into")
    a.add_argument("--most-bytes", type=int, default=MOST_BYTES)
    a.add_argument("--part-bytes", type=int, default=PART_BYTES)
    a = sub.add_parser("notes")
    a.add_argument("--repository", required=True)
    a.add_argument("--tag", required=True)
    a.add_argument("--rev", required=True)
    a.add_argument("--date", required=True)
    a.add_argument("--assets", required=True)
    a.add_argument("--signed", action="store_true")
    a = sub.add_parser("fetch")
    a.add_argument("--tag")
    a.add_argument("--from", dest="source")
    a.add_argument("--into", default=".")
    a.add_argument("--repository", required=True)
    a = sub.add_parser("menu-room")
    a.add_argument("--profile", required=True)
    a.add_argument("--toplevel", required=True)
    a.add_argument("--last-good", required=True)
    a.add_argument("--limit", type=int, required=True)
    args = p.parse_args(argv)

    if args.command == "pack":
        for asset in pack(args.iso, args.into, args.most_bytes, args.part_bytes):
            print(asset)
    elif args.command == "notes":
        sys.stdout.write(notes(args.repository, args.tag, args.rev, args.date, args.assets, args.signed))
    elif args.command == "fetch":
        if args.tag and args.source:
            refuse("--tag and --from name two different places")
        fetch(args.repository, args.tag, args.source, args.into)
    elif args.command == "menu-room":
        if not menu_room(args.profile, args.toplevel, args.last_good, args.limit):
            print(f"the last good system would leave a boot menu of {args.limit}", file=sys.stderr)
            raise SystemExit(1)


if __name__ == "__main__":
    main(sys.argv[1:])
