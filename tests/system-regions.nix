# THE FOOTER'S VIEWS AS REGIONS OF THEIR OWN CRATES (the 2026-10-12 wave): the
# checks that a view that left the footer took nothing of the footer with it
# and left nothing of itself behind.
#
# A view of the footer becomes a `region::Region` of its own crate, registered
# by the binary (`regions.rs`), and the footer hosts it by its id. Two promises
# hold only if someone keeps them, and the compiler keeps neither:
#
#   - `regions-order-the-footers-threads`: the footer reads the machine for the
#     whole session — the screens' reader the brightness keys order with no
#     view open, the radio whose agent only the footer may register, the two
#     tunnels the modeline draws — and a region ORDERS those threads through
#     the lines the footer hands it as it opens (`Footer::standing`). A region
#     that raised a reader of its own would be a second truth about one
#     machine: a preview applied by a thread nobody else can reach, a modeline
#     a reading behind every order, two polls of the same daemons.
#   - `a-view-stands-on-the-foundation`: a crate whose view the binary
#     registers stands on the shell's foundation and on the other views the
#     design names (`shell-data-design.md` §1.2) — and on the leaf of its
#     daemon's words, which stands on no shell crate — and on nothing else: a
#     sideways edge compiles, and makes a line of one view recompile another.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellPrelude,
  shellManifestPrelude,
}:
{
  regions-order-the-footers-threads = mkCheck "regions-order-the-footers-threads" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # FILE-WIDE: the question is what a region's FILE starts and holds —
    # anywhere in its product (`without_tests`), comments off and strings kept
    # (`uncommented`), since a call is a call wherever it sits. The footer's
    # side is narrowed to `standing`, the one place it hands the lines over.
    registered = uncommented(shell_file("regions.rs").read_text())
    regions = re.findall(r"(\w+)::ENTRY\b", registered)
    floor(len(regions), 5, "regions the binary registers")

    # THE THREADS THE FOOTER KEEPS FOR THE SESSION, by the function that
    # raises each — named by its path, or brought in by a `use` (grouped
    # too) and called bare.
    RAISED = [
        ("ronoctua_display", "watch", "the screens' reader"),
        ("ronoctua_display", "watch_at", "the screens' reader"),
        ("ronoctua_bluetooth", "watch", "the radio's threads and its agent"),
        ("ronoctua_tunnel", "watch", "the tunnels' thread"),
        ("ronoctua_network", "watch", "NetworkManager's watch"),
    ]


    def raises(text, crate, fn):
        """Whether `text` names `crate::fn` — the path, or a `use` of it."""
        if re.search(r"\b" + crate + "::" + fn + r"\b", text):
            return True
        return any(
            re.search(r"\b" + fn + r"\b", group)
            for group in re.findall(r"\buse\s+" + crate + r"::\{([^}]*)\}", text)
        )


    wrong, read = [], 0
    for module in regions:
        text = uncommented(without_tests(shell_file(module + ".rs").read_text()))
        floor(len(text), 1500, "characters of " + module + ".rs")
        read += 1
        for crate, fn, what in RAISED:
            if raises(text, crate, fn):
                wrong.append(f"{module}.rs raises {what} of its own (`{crate}::{fn}`): the footer keeps it, and hands the region its line")
        # THE SCREENS' HANDLE WAITS FOR ITS THREAD (`Handle::finish`); a
        # region holds only a line (`Orders`), which the reader's loop lets go
        # of when the region goes.
        if re.search(r"\bronoctua_display::Handle\b", text):
            wrong.append(f"{module}.rs holds the screens' `Handle`, which waits for the reader — a region holds an `Orders` line")

    # AND THE FOOTER HANDS EVERY LINE OVER as a region opens: a line it keeps
    # to itself is a region that says "no reader" over a reader that is there.
    footer = uncommented(without_tests(shell_source("surface.rs")))
    standing = must(body_of(footer, "standing"), "`Footer::standing`")
    for field, what in (
        ("display_orders", "the screens' reader"),
        ("bluetooth_orders", "the radio's orders"),
        ("tunnel_hand", "the tunnels' thread"),
    ):
        if "self." + field not in standing:
            wrong.append(f"`Footer::standing` does not hand a region the line to {what} (`self.{field}`)")

    if wrong:
        print("ERROR: a region of the footer reads the machine on its own.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {read} regions order the footer's threads through the lines it hands them")
    END
  '';

  a-view-stands-on-the-foundation = mkCheck "a-view-stands-on-the-foundation" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys, tomllib
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())
    exec(open("${shellManifestPrelude}").read())

    # PARSED: every shell crate's `[dependencies]` (`tomllib`); which crate
    # holds a registered region is read from where its file is.
    # FILE-WIDE: the registry (`regions.rs`) is read whole for its entries,
    # comments off: an entry anywhere in it is a region the binary hosts.
    manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
    floor(len(manifests), 11, "shell crates' manifests")
    registered = uncommented(shell_file("regions.rs").read_text())
    regions = re.findall(r"(\w+)::ENTRY\b", registered)
    floor(len(regions), 5, "regions the binary registers")

    FOUNDATION = {
        "ronoctua-shell-kit", "ronoctua-shell-door", "ronoctua-shell-report",
        "ronoctua-shell-keep", "ronoctua-shell-work", "ronoctua-shell-inspect",
    }
    # THE EDGES BETWEEN VIEWS THE DESIGN NAMES (`shell-data-design.md`
    # §1.2): the file manager on git and the security centre — and on the git
    # line, the half of git it draws, which the history shares (lane R5h:
    # `the-file-manager-stands-on-the-foundation`) —, the programs and the
    # update on the belongings, the search on the calculator and the icons,
    # Incus on the containers — and the history (lane R5f) on the same git
    # line the manager draws, below the two of them; and the video catalogue
    # (lane R5i), whose ranking is a page of the player, on the player, which
    # asks it through its seam (`player_seam`). Every other view stands on
    # the foundation alone.
    BETWEEN = {
        "ronoctua-shell-files": {"ronoctua-shell-git", "ronoctua-shell-git-line", "ronoctua-shell-security"},
        "ronoctua-shell-git": {"ronoctua-shell-git-line"},
        "ronoctua-shell-software": {"ronoctua-shell-belongings"},
        "ronoctua-shell-update": {"ronoctua-shell-belongings"},
        "ronoctua-shell-fast-access": {"ronoctua-shell-calc", "ronoctua-shell-icons"},
        "ronoctua-shell-incus": {"ronoctua-shell-containers"},
        "ronoctua-shell-video-metrics": {"ronoctua-shell-player"},
    }

    # THE DIRECT EDGES ARE ENOUGH: the foundation stands on itself alone
    # (`the-shell-foundation-names-no-feature`), and a view the design
    # names is held to the same rule when it is registered — so what each
    # registered view stands on directly is what it stands on at all.
    wrong, crates = [], set()
    # AND THE WORDS OF A DAEMON that a view and the footer both say (lane
    # R5c): a leaf of their own under both, `<the view's crate>-words`, which
    # stands on NO shell crate — so a line of it recompiles the view and the
    # binary that say it, and no other view. One that stands on a shell crate
    # is no leaf, and is refused.
    words = set()
    for name, manifest in sorted(manifests.items()):
        if not name.endswith("-words"):
            continue
        shell = sorted(d for d in manifest.get("dependencies", {}) if d in manifests)
        if shell:
            wrong.append(f"{name} stands on {', '.join(shell)}: a daemon's words are a leaf, on no shell crate")
        else:
            words.add(name)
    for module in regions:
        path = shell_file(module + ".rs")
        crate = path.parent.parent.name
        if crate == "ronoctua-shell":
            wrong.append(f"{module}.rs is the binary's: a registered region lives in its view's own crate")
            continue
        crates.add(crate)
    floor(len(crates), 4, "crates of registered regions")
    for crate in sorted(crates):
        allowed = FOUNDATION | BETWEEN.get(crate, set()) | ({crate + "-words"} & words)
        stands = {d for d in manifests[crate].get("dependencies", {}) if d in manifests}
        floor(len(stands & FOUNDATION), 2, "foundation crates " + crate + " stands on")
        for d in sorted(stands - allowed):
            wrong.append(f"{crate} stands on {d}: a line of {d} would recompile the view of {crate}")

    if wrong:
        print("ERROR: a view's crate stands on more than the foundation.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(crates)} crates of registered views stand on the foundation and on the views the design names")
    END
  '';
}
