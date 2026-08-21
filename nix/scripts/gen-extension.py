#!/usr/bin/env python3
"""The extension's manifest and its generated table, from the same sources
everything else in this system uses.

Two things are written here:

  `manifest.json` — and it is generated rather than hand-written so the
  permission list is in ONE place with the reasoning next to it, and so the
  check that guards the fingerprint has one file to read.

  `generated.js`  — the texts, in both languages, and the colours and
  measurements, out of `assets/i18n` and `assets/tokens`. Invariants 8 and 9
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
]

LANGUAGES = ["pt-BR", "en-GB"]

# The colours the popup uses, as `group.name` in `assets/tokens/colors.nix`.
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
]


def camel(name: str) -> str:
    """`ext-queue-up` -> `extQueueUp`, `surface.base` -> `surfaceBase`."""
    head, *rest = re.split(r"[-.]", name)
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def texts(i18n: pathlib.Path) -> dict:
    """Every key, in every language, or a failure naming what is missing."""
    out = {}
    missing = []
    for language in LANGUAGES:
        path = i18n / f"{language}.ftl"
        if not path.is_file():
            sys.exit(f"ERRO: falta {path}")
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
            "ERRO: chaves de texto faltando na extensao — o botao mostraria o "
            "nome da chave:\n  " + "\n  ".join(missing)
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
        sys.exit(f"ERRO: `{raw}` nao e uma cor 0xRRGGBBAA")
    r, g, b, a = (int(text[i : i + 2], 16) for i in (0, 2, 4, 6))
    if a == 255:
        return f"#{text[:6]}"
    return f"rgba({r}, {g}, {b}, {a / 255:.3f})"


def colours(path: pathlib.Path) -> dict:
    table = json.loads(path.read_text(encoding="utf-8"))
    out = {}
    for group, name in COLOURS:
        if group not in table or name not in table[group]:
            sys.exit(f"ERRO: o token de cor `{group}.{name}` nao existe")
        out[camel(f"{group}.{name}")] = colour_of(table[group][name])
    return out


def spacing(path: pathlib.Path) -> dict:
    table = json.loads(path.read_text(encoding="utf-8"))
    block = table.get("extension")
    if not isinstance(block, dict):
        sys.exit("ERRO: `space.extension` nao existe em assets/tokens/space.nix")
    return {key: value for key, value in block.items()}


def manifest(version: str) -> dict:
    return {
        "manifest_version": 3,
        # A plain name. It is what the browser's own extension page shows, and
        # nothing about it reaches a website.
        "name": "ronoctua",
        "version": version,
        "description": "Manda os downloads para a fila da ronoctua.",
        # THE WHOLE PERMISSION LIST, and why each one is here:
        #
        #   downloads      the interception: see it start, and stop it
        #   nativeMessaging the only way out — no port, no listener, no secret
        #   activeTab      access to ONE tab, only after the button is pressed,
        #                  and only until it navigates. It is what replaces a
        #                  standing host permission
        #   scripting      to run `find-media.js` under that grant, on demand
        #   contextMenus   the right-click entries
        #   storage        one switch: whether downloads are handed over
        #   tabs           the active tab's URL for the media menu
        "permissions": [
            "downloads",
            "nativeMessaging",
            "activeTab",
            "scripting",
            "contextMenus",
            "storage",
            "tabs",
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
    for arquivo in sorted(source.iterdir()):
        if arquivo.suffix not in {".js", ".html", ".css"} or arquivo.name == "generated.js":
            continue
        found |= set(re.findall(rf"\b{table}\.([A-Za-z_][A-Za-z0-9_]*)", arquivo.read_text(encoding="utf-8")))
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
            "ERRO: a extensao usa nome que esta tabela nao produz — em JavaScript "
            "isso vira `undefined` na tela, sem erro nenhum:\n  " + "\n  ".join(missing)
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--colors", required=True, type=pathlib.Path)
    parser.add_argument("--space", required=True, type=pathlib.Path)
    parser.add_argument("--i18n", required=True, type=pathlib.Path)
    parser.add_argument("--host", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    args = parser.parse_args()

    table = manifest(args.version)
    for forbidden in FORBIDDEN:
        if forbidden in table:
            sys.exit(
                f"ERRO: o manifesto ganhou `{forbidden}` — e uma via de deteccao "
                "por qualquer site; veja o comentario no topo deste arquivo"
            )

    todos = texts(args.i18n)
    cores = colours(args.colors)
    medidas = spacing(args.space)
    # The sources live next to this output, because the derivation copies them
    # into the build directory first.
    cross_check(args.out, {"TEXT": todos[LANGUAGES[0]], "COLOUR": cores, "SPACE": medidas})

    body = "\n".join(
        [
            "// GENERATED by nix/scripts/gen-extension.py. DO NOT EDIT BY HAND.",
            "//",
            "// The words come from assets/i18n/*.ftl and the colours from",
            "// assets/tokens/*.nix — once, there. Editing here means losing the",
            "// change at the next build.",
            "",
            f"export const HOST = {json.dumps(args.host)};",
            "",
            "const ALL = " + json.dumps(todos, ensure_ascii=False, indent=2) + ";",
            "",
            "// The browser's own language decides, and pt-BR is the fallback:",
            "// this is one person's distro and that is the language they read.",
            "const WHICH = (globalThis.chrome && chrome.i18n",
            "  ? chrome.i18n.getUILanguage() : 'pt-BR').toLowerCase();",
            "export const TEXT = WHICH.startsWith('pt') ? ALL['pt-BR'] : ALL['en-GB'];",
            "",
            "export const COLOUR = " + json.dumps(cores, ensure_ascii=False, indent=2) + ";",
            "",
            "export const SPACE = " + json.dumps(medidas, ensure_ascii=False, indent=2) + ";",
            "",
        ]
    )

    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "manifest.json").write_text(
        json.dumps(table, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    (args.out / "generated.js").write_text(body, encoding="utf-8")
    print(f"OK: manifesto e tabela geradas ({len(KEYS)} chaves x {len(LANGUAGES)} idiomas)")


if __name__ == "__main__":
    main()
