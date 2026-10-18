#!/usr/bin/env python3
"""The extension's manifest and its generated table, from the same sources
everything else in this system uses.

Two things are written here:

  `manifest.json` — and it is generated rather than hand-written so the
  permission list is in ONE place with the reasoning next to it, and so the
  check that guards the fingerprint has one file to read.

  `generated.js`  — the texts, in both languages, and the colours and
  measurements, out of its own fragment (`extension/i18n`) and `assets/tokens`. Invariants 8 and 9
  reach the browser: a word written into the popup would be a word with no
  translation, and a hex code there would be a colour outside the design system.

Both fail loudly rather than emitting something plausible: a missing text key
stops the build, because the alternative is a button whose label is the key's
own name in front of the person.
"""

import argparse
import json
import pathlib
import re
import sys

# THE FIVE THINGS THIS MANIFEST MUST NEVER GROW.
#
# Each is a way for any website to detect the extension, and the whole reason
# the design is what it is. They are listed here as well as in the check so
# that whoever edits the manifest reads them before the build does.
#
#   web_accessible_resources  a page can `fetch` a `chrome-extension://` URL
#   content_scripts           anything left in the DOM is a marker
#   externally_connectable    hands a page `chrome.runtime`
#   host_permissions          standing access is what makes the above possible
#   declarative_net_request   a page infers it from its own headers
FORBIDDEN = (
    "web_accessible_resources",
    "content_scripts",
    "externally_connectable",
    "host_permissions",
    "declarative_net_request",
)

# The keys the popup and the worker name. Every one must exist in BOTH
# languages; the build stops otherwise.
KEYS = [
    "ext-title",
    "ext-queue-up",
    "ext-queue-down",
    "ext-video",
    "ext-audio",
    "ext-subtitles",
    "ext-scan",
    "ext-found-none",
    "ext-page-not-media",
    "ext-sent",
    "ext-failed",
    "ext-intercept-on",
    "ext-intercept-off",
    "ext-send-link",
    "ext-send-media",
    "ext-as-me",
    "ext-anonymous",
]

# The colours the popup uses, as `group.name` in `assets/tokens/colors.nix`.
# The VALUES come from the generated palette (`nix/themes.nix`), a flat map
# of `group.name` -> `0xRRGGBBAA` for the default theme.
COLOURS = [
    ("surface", "base"),
    ("surface", "selection"),
    ("text", "primary"),
    ("text", "secondary"),
    ("text", "dim"),
    ("rule", "default"),
    ("state", "accent"),
    ("state", "success"),
    ("state", "error"),
    ("state", "warning"),
]


def camel(name: str) -> str:
    """`ext-queue-up` -> `extQueueUp`, `surface.base` -> `surfaceBase`."""
    head, *rest = re.split(r"[-.]", name)
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def texts(i18n: pathlib.Path, languages: list[str]) -> dict:
    """Every key, in every language, or a failure naming what is missing."""
    out = {}
    missing = []
    for language in languages:
        path = i18n / f"{language}.ftl"
        if not path.is_file():
            sys.exit(f"ERROR: {path} is missing")
        table = {}
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            key, sep, value = line.partition("=")
            if not sep:
                continue
            table[key.strip()] = value.strip()
        picked = {}
        for key in KEYS:
            if key not in table:
                missing.append(f"{language}: {key}")
                continue
            picked[camel(key)] = table[key]
        out[language] = picked
    if missing:
        sys.exit(
            "ERROR: text keys missing from the extension — the button would show the "
            "key's name:\n  " + "\n  ".join(missing)
        )
    return out


def colour_of(raw: str) -> str:
    """`0xRRGGBBAA` -> a CSS colour.

    Opaque colours come out as `#rrggbb`, which is what a stylesheet reads
    most easily; anything with alpha comes out as `rgba()`, because `#rrggbbaa`
    is newer than some of the surfaces this may end up on.
    """
    text = raw.lower().removeprefix("0x")
    if len(text) != 8 or any(c not in "0123456789abcdef" for c in text):
        sys.exit(f"ERROR: `{raw}` is not a 0xRRGGBBAA colour")
    r, g, b, a = (int(text[i : i + 2], 16) for i in (0, 2, 4, 6))
    if a == 255:
        return f"#{text[:6]}"
    return f"rgba({r}, {g}, {b}, {a / 255:.3f})"


def colours(path: pathlib.Path) -> dict:
    table = json.loads(path.read_text(encoding="utf-8"))
    out = {}
    for group, name in COLOURS:
        key = f"{group}.{name}"
        if key not in table:
            sys.exit(f"ERROR: the colour token `{key}` does not exist")
        out[camel(key)] = colour_of(table[key])
    return out


def spacing(path: pathlib.Path) -> dict:
    table = json.loads(path.read_text(encoding="utf-8"))
    block = table.get("extension")
    if not isinstance(block, dict):
        sys.exit("ERROR: `space.extension` does not exist in assets/tokens/space.nix")
    return {key: value for key, value in block.items()}


def manifest(version: str) -> dict:
    return {
        "manifest_version": 3,
        # A plain name. It is what the browser's own extension page shows, and
        # nothing about it reaches a website.
        "name": "ronoctua",
        "version": version,
        "description": "Sends downloads to ronoctua's queue, and bridges its kiosk windows.",
        # THE WHOLE PERMISSION LIST, and why each one is here:
        #
        #   downloads      the interception: see it start, and stop it
        #   nativeMessaging the only way out — no port, no listener, no secret
        #   activeTab      access to ONE tab, only after the button is pressed,
        #                  and only until it navigates. It is what replaces a
        #                  standing host permission
        #   scripting      to run `find-media.js` under that grant, on demand
        #   contextMenus   the right-click entries
        #   storage        one switch: whether downloads are handed over; and
        #                  which windows are kiosks, across a worker restart
        #   tabs           the active tab's URL for the media menu; and a
        #                  kiosk's tabs — titles and addresses the footer draws
        #                  in the modeline, and the orders it sends them
        #   management     the kiosk's list of extensions — `getAll` and
        #                  nothing else: never turned on, off or away
        #   search         the kiosk's address line searches the person's own
        #                  engine for words that are not an address
        "permissions": [
            "downloads",
            "nativeMessaging",
            "activeTab",
            "scripting",
            "contextMenus",
            "storage",
            "tabs",
            "management",
            "search",
        ],
        "background": {"service_worker": "worker.js", "type": "module"},
        "action": {"default_popup": "popup.html"},
        # WHICH NATIVE HOST is not a manifest key at all: Chrome decides that
        # on the host's own side, where the manifest in
        # `/etc/opt/chrome/native-messaging-hosts/` names exactly which
        # extension ids may start it. The name travels to the JavaScript
        # through `generated.js`, so there is still only one literal.
        "minimum_chrome_version": "116",
    }


def used(source: pathlib.Path, table: str) -> set:
    """Every `TABLE.name` the extension's own sources actually name."""
    found = set()
    for entry in sorted(source.iterdir()):
        if entry.suffix not in {".js", ".html", ".css"} or entry.name == "generated.js":
            continue
        found |= set(re.findall(rf"\b{table}\.([A-Za-z_][A-Za-z0-9_]*)", entry.read_text(encoding="utf-8")))
    return found


def cross_check(source: pathlib.Path, tables: dict) -> None:
    """Refuses a name the sources use and this file does not produce.

    THE LIKELIEST BUG IN THE WHOLE EXTENSION, and the one with no symptom: a
    mistyped key is `undefined` in JavaScript, which puts the word `undefined`
    on a button and logs nothing. The generator knows both sides, so it is the
    only place that can catch it — and it catches it at BUILD time.
    """
    missing = []
    for name, produced in tables.items():
        for wanted in sorted(used(source, name)):
            if wanted not in produced:
                missing.append(f"{name}.{wanted}")
    if missing:
        sys.exit(
            "ERROR: the extension uses a name this table does not produce — in JavaScript "
            "that becomes `undefined` on screen, with no error at all:\n  " + "\n  ".join(missing)
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--colors", required=True, type=pathlib.Path)
    parser.add_argument("--space", required=True, type=pathlib.Path)
    parser.add_argument("--i18n", required=True, type=pathlib.Path)
    parser.add_argument("--host", required=True)
    # The kiosk's bridge (`ronoctua-browser`), from the same table as `--host`:
    # `nix/browser-hosts.nix`.
    parser.add_argument("--browser-host", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    # The languages, from `assets/i18n/languages.nix`. The FIRST is the base:
    # it is what the cross-check reads and what the popup falls back to.
    parser.add_argument("--languages", required=True, nargs="+")
    args = parser.parse_args()

    table = manifest(args.version)
    for forbidden in FORBIDDEN:
        if forbidden in table:
            sys.exit(
                f"ERROR: the manifest gained `{forbidden}` — it is a detection channel "
                "by any site; see the comment at the top of this file"
            )

    all_texts = texts(args.i18n, args.languages)
    colours_of = colours(args.colors)
    measures = spacing(args.space)
    # The sources live next to this output, because the derivation copies them
    # into the build directory first.
    cross_check(
        args.out,
        {"TEXT": all_texts[args.languages[0]], "COLOUR": colours_of, "SPACE": measures},
    )

    body = "\n".join(
        [
            "// GENERATED by nix/scripts/gen-extension.py. DO NOT EDIT BY HAND.",
            "//",
            "// The words come from extension/i18n/*.ftl and the colours from",
            "// assets/tokens/*.nix — once, there. Editing here means losing the",
            "// change at the next build.",
            "",
            f"export const HOST = {json.dumps(args.host)};",
            f"export const BROWSER_HOST = {json.dumps(args.browser_host)};",
            "",
            "const ALL = " + json.dumps(all_texts, ensure_ascii=False, indent=2) + ";",
            "",
            "// The browser's own language decides, and the base language is the",
            "// fallback — the same one `ronoctua_i18n::DEFAULT` falls back to.",
            "//",
            "// A LOOKUP AND NOT A TERNARY. The ternary this replaced could only",
            "// ever answer one of two, so a third language would have been",
            "// generated into ALL, shipped in the extension, and never chosen.",
            "// The match is on the SUBTAG (`pt` finds `pt-BR`), because a browser",
            "// reports `pt-PT` and `en-US` and neither is a name in this list.",
            "const ALL_TAGS = " + json.dumps(args.languages) + ";",
            "const WHICH = (globalThis.chrome && chrome.i18n",
            f"  ? chrome.i18n.getUILanguage() : {json.dumps(args.languages[0])}).toLowerCase();",
            "const PICKED = ALL_TAGS.find((t) => t.toLowerCase() === WHICH)",
            "  || ALL_TAGS.find((t) => WHICH.split('-')[0] === t.split('-')[0].toLowerCase())",
            f"  || {json.dumps(args.languages[0])};",
            "export const TEXT = ALL[PICKED];",
            "",
            "export const COLOUR = " + json.dumps(colours_of, ensure_ascii=False, indent=2) + ";",
            "",
            "export const SPACE = " + json.dumps(measures, ensure_ascii=False, indent=2) + ";",
            "",
        ]
    )

    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "manifest.json").write_text(
        json.dumps(table, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    (args.out / "generated.js").write_text(body, encoding="utf-8")
    print(
        f"OK: manifest and table generated ({len(KEYS)} keys x {len(args.languages)} languages)"
    )


if __name__ == "__main__":
    main()
