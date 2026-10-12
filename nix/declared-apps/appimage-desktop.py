"""The launcher entry of a DECLARED AppImage, written inside its wrapper's build.

`nix/declared-apps/appimage.nix` runs this in `appimageTools.wrapType2`'s own
install step, with the AppImage already unpacked by `appimageTools.extract`.
It reads the `.desktop` the AppImage carries and writes a NEW one: only the keys
a launcher needs are copied from the first group, and `Exec=` is ours — the
wrapper's program and the field code the AppImage asked for, nothing else. An
AppImage is somebody else's file, and its entry is not trusted to choose what
runs.

The icon is the person's own picture when the declaration names one (already
checked against its hash by Nix), else the AppImage's `.DirIcon`, else the
file its `Icon=` names at the top of the image. A link is followed only while it
stays inside the unpacked image.

The entry is `ronoctua-appimage-<key>.desktop`, the id the software centre
gives an AppImage it installs by hand, and it carries `X-Ronoctua-Declared` so
the centre can tell the declared copy from an imperative one.
"""

import argparse
import pathlib
import re
import shutil
import sys

# The keys that survive, from the AppImage's first group. Localised variants of
# the four that have them come along.
KEPT = re.compile(
    r"^(?:(?:Name|GenericName|Comment|Keywords)(?:\[[A-Za-z0-9_@.-]+\])?"
    r"|Categories|MimeType|StartupWMClass|Terminal)$"
)
# The field codes an `Exec=` may pass on: one file or URL, or several.
FIELD_CODES = ("%f", "%F", "%u", "%U")
# What a value may not carry into a line of ours.
CONTROL = re.compile(r"[\x00-\x1f\x7f]")


def clean(value: str) -> str:
    return CONTROL.sub("", value).strip()


def first_group(text: str) -> dict[str, str]:
    """The `[Desktop Entry]` group's keys, the first value of each."""
    fields: dict[str, str] = {}
    inside = False
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("["):
            if inside:
                break
            inside = line == "[Desktop Entry]"
            continue
        if not inside or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        if key not in fields:
            fields[key] = clean(value)
    return fields


def inside(root: pathlib.Path, path: pathlib.Path) -> pathlib.Path | None:
    """`path` resolved, or None when it leaves the unpacked image."""
    try:
        resolved = path.resolve(strict=True)
    except (OSError, RuntimeError):
        return None
    try:
        resolved.relative_to(root.resolve())
    except ValueError:
        return None
    return resolved if resolved.is_file() else None


def kind_of(path: pathlib.Path) -> str | None:
    """`png` or `svg`, by what the file IS and not by its name."""
    head = path.read_bytes()[:512]
    if head.startswith(b"\x89PNG\r\n\x1a\n"):
        return "png"
    if b"<svg" in head:
        return "svg"
    return None


def embedded_icon(root: pathlib.Path, named: str) -> pathlib.Path | None:
    candidates = [root / ".DirIcon"]
    if named and "/" not in named:
        candidates += [root / f"{named}.png", root / f"{named}.svg"]
    for c in candidates:
        found = inside(root, c)
        if found is not None and kind_of(found) is not None:
            return found
    return None


def main() -> int:
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("--contents", required=True, type=pathlib.Path)
    a.add_argument("--out", required=True, type=pathlib.Path)
    a.add_argument("--key", required=True)
    a.add_argument("--name", required=True)
    a.add_argument("--program", required=True)
    a.add_argument("--icon", type=pathlib.Path)
    args = a.parse_args()

    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", args.key):
        print(f"appimage-desktop: `{args.key}` is not a key", file=sys.stderr)
        return 1

    root = args.contents
    entries = sorted(p for p in root.glob("*.desktop") if inside(root, p) is not None)
    fields = first_group(entries[0].read_text(encoding="utf-8", errors="replace")) if entries else {}

    lines = ["[Desktop Entry]", "Type=Application"]
    if not fields.get("Name"):
        lines.append(f"Name={clean(args.name)}")
    for key, value in fields.items():
        if not KEPT.match(key) or not value:
            continue
        if key == "Terminal" and value not in ("true", "false"):
            continue
        lines.append(f"{key}={value}")

    code = next((t for t in fields.get("Exec", "").split() if t in FIELD_CODES), None)
    lines.append(f"Exec={args.program}" + (f" {code}" if code else ""))

    stem = f"ronoctua-appimage-{args.key}"
    picture = args.icon if args.icon is not None else embedded_icon(root, fields.get("Icon", ""))
    if picture is not None:
        kind = "png" if args.icon is not None else kind_of(picture)
        target = args.out / "share" / "pixmaps" / f"{stem}.{kind}"
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(picture, target)
        lines.append(f"Icon={target}")
    lines.append(f"X-Ronoctua-Declared=appimage:{args.key}")

    desktop = args.out / "share" / "applications" / f"{stem}.desktop"
    desktop.parent.mkdir(parents=True, exist_ok=True)
    desktop.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
