# THE FOOTER IN A CRATE OF ITS OWN, AND THE BINARY THAT COMPOSES (the
# 2026-10-12 wave, lane R6): the checks that the footer names no feature, and
# that every window a crate says is one the binary runs.
#
#   - `footer-names-no-feature`: among the shell's crates, the footer
#     (`ronoctua-shell-footer`) stands on the foundation and on the words leaves
#     alone — an allow-list, not a list of features: every other shell crate is
#     one the binary composes (a region, a resident, keys, doors, cameras, the
#     buffers' host, the git line two views draw…) or the binary itself, and a
#     list read from the binary's tables would miss whatever it composes some
#     other way. Not by its manifest, not through any crate below it in cargo's
#     own build graph (`plan.json`, which `rust-plan-is-current` holds to the
#     tree), and not by a path in its code. So a line of a view recompiles the
#     view and the binary, and nothing of the footer; what the footer says of a
#     view's daemon for the session is that view's words leaf, under both. The
#     compiler holds half of it — a footer that names a crate it does not depend
#     on is refused — and this holds the half no compiler asks: that the
#     manifest does not grow the edge back, directly or two crates down.
#
#   - `every-door-is-dispatched`: every crate's table of windows
#     (`pub const DOORS`) and of cameras (`pub const CAMERAS`) is one the
#     binary lists, no word is claimed twice, and no door's word is one of the
#     binary's own subcommands (nor a camera's one of its own cameras, which
#     its list reads first). A table left out is no compile error: the
#     binary would take `ronoctua-shell <door>` for a word it does not know —
#     and with no word it knows, it brings a second footer up.
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
let
  # CARGO'S OWN GRAPH, as the planner committed it: what each unit of the
  # release, loop and test builds links — features unified, renames resolved.
  planFile = ../nix/rust/plan.json;

  # WHAT THE FOOTER MAY STAND ON, among the shell's crates: the foundation —
  # the same six `the-shell-foundation-names-no-feature` holds to standing on
  # itself alone — and every words leaf, a crate `<view>-words` that stands on
  # no shell crate (`a-view-stands-on-the-foundation` refuses one that does).
  # Every other shell crate is no edge of the footer's.
  allowed = ''
    FOUNDATION = {
        "ronoctua-shell-kit", "ronoctua-shell-door", "ronoctua-shell-report",
        "ronoctua-shell-keep", "ronoctua-shell-work", "ronoctua-shell-inspect",
    }

    def footer_may_stand_on(manifests):
        # PARSED: each shell crate's `[dependencies]` (`tomllib`, read by the
        # caller): a `-words` crate is a leaf only while it names no shell crate.
        leaves = {
            name for name, m in manifests.items()
            if name.endswith("-words") and not any(d in manifests for d in m.get("dependencies", {}))
        }
        floor(len(leaves), 8, "words leaves")
        missing = sorted(FOUNDATION - set(manifests))
        if missing:
            print(f"ERROR: {', '.join(missing)} is not a shell crate any more: the foundation this check allows moved", file=sys.stderr)
            raise SystemExit(1)
        return FOUNDATION | leaves
  '';
in
{
  footer-names-no-feature = mkCheck "footer-names-no-feature" [ pkgs.python3 ] ''
    python3 - ${planFile} <<'END'
    import json, re, sys, tomllib
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())
    exec(open("${shellManifestPrelude}").read())
    ${allowed}

    FOOTER = "ronoctua-shell-footer"
    manifests = {m.parent.name: m for m in shell_manifests()}
    if FOOTER not in manifests:
        print(f"ERROR: there is no {FOOTER}: the footer is a crate of its own", file=sys.stderr)
        raise SystemExit(1)
    parsed = {name: tomllib.loads(m.read_text()) for name, m in manifests.items()}
    allowed = footer_may_stand_on(parsed) | {FOOTER}
    # EVERY OTHER SHELL CRATE is refused: the features, the binary, the
    # buffers' PNG camera — whatever the binary composes, by a table or not.
    refused = set(parsed) - allowed
    floor(len(refused), 30, f"shell crates {FOOTER} may not stand on")
    wrong = []

    # 1) ITS MANIFEST. PARSED: the footer's `Cargo.toml` (`tomllib`) — its
    # dependencies, dev-dependencies, build-dependencies and target tables,
    # each by the package it names (a renamed one by its `package`).
    data = parsed[FOOTER]
    tables = [(t, data.get(t, {})) for t in ("dependencies", "dev-dependencies", "build-dependencies")]
    for target in data.get("target", {}).values():
        tables += [(t, target.get(t, {})) for t in ("dependencies", "dev-dependencies", "build-dependencies")]
    declared, on_foundation = 0, set()
    for name, table in tables:
        for dep, spec in table.items():
            declared += 1
            pkg = spec.get("package", dep) if isinstance(spec, dict) else dep
            if pkg in refused:
                wrong.append(f"{FOOTER}'s [{name}] names {pkg}, which is neither the foundation nor a words leaf")
            if pkg in FOUNDATION:
                on_foundation.add(pkg)
    floor(declared, 20, f"dependencies {FOOTER} declares")
    floor(len(on_foundation), 3, f"foundation crates {FOOTER} declares")

    # 2) CARGO'S GRAPH. PARSED: `plan.json` — every unit of the footer in the
    # release, loop and test graphs, and every unit it reaches through
    # `deps`: a shell crate two crates down recompiles the footer as surely as
    # one in its manifest.
    plan = json.load(open(sys.argv[1]))
    seen_units = 0
    for graph in ("release", "loop", "test"):
        units = plan["graphs"][graph]["units"]
        roots = [k for k, u in units.items() if u["pkg"] == FOOTER]
        if not roots:
            wrong.append(f"the {graph} graph of plan.json builds no unit of {FOOTER}")
            continue
        reached, todo = set(), list(roots)
        while todo:
            key = todo.pop()
            if key in reached:
                continue
            reached.add(key)
            todo += [d["key"] for d in units[key]["deps"]]
        seen_units += len(reached)
        for pkg in sorted({units[k]["pkg"] for k in reached} & refused):
            wrong.append(f"the {graph} build of {FOOTER} links {pkg}, which is neither the foundation nor a words leaf")
    floor(seen_units, 300, f"units {FOOTER}'s builds reach")

    # 3) ITS CODE.
    # FILE-WIDE: every file of the footer's crate, comments and literals off
    # (`code_of`) — a path names a crate wherever it sits in the code, and a
    # sentence about one does not.
    crate_dir = next(c for c in shell_crates() if c.name == FOOTER)
    files = sorted((crate_dir / "src").rglob("*.rs"))
    floor(len(files), 25, f"files of {FOOTER}")
    snake = {name.replace("-", "_"): name for name in refused}
    for f in files:
        text = code_of(f.read_text())
        for word in sorted(set(re.findall(r"\bronoctua_(?:shell|render)\w*", text))):
            if word in snake:
                wrong.append(f"{f.relative_to(crate_dir)} names {snake[word]} by its path")

    if wrong:
        print("ERROR: the footer stands on more than the foundation and the words leaves.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       The footer hosts a feature's region by its id, handed it by the", file=sys.stderr)
        print("       binary; what it says of a view's daemon is that view's words leaf,", file=sys.stderr)
        print("       under both. A shell crate below the footer recompiles it for every", file=sys.stderr)
        print("       line of that crate.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {FOOTER} stands on none of the {len(refused)} shell crates outside the foundation and the words leaves ({seen_units} units of its three builds, {len(files)} files)")
    END
  '';

  every-door-is-dispatched = mkCheck "every-door-is-dispatched" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # PARSED: each crate's tables (`pub const DOORS: &[…Door] = &[…];` and
    # `pub const CAMERAS: &[…Camera] = &[…];`, read between their brackets
    # by the prelude's `_matched`) and the binary's lists of them
    # (`const DOORS: &[&[Door]]` in `main.rs`, `const CAMERAS:
    # &[&[Camera]]` in `photograph.rs`) — product code, tests cut, comments
    # off, strings kept (a door's word is a literal).
    # FILE-WIDE: a table is a module-level constant of whichever file of a
    # crate holds it.
    def table(text, name):
        head = re.search(r"\bpub const " + name + r":\s*&\[[^\]]*\]\s*=\s*&\[", text)
        return _matched(text, head.end() - 1) if head else None

    def words(body, field):
        # A literal, or a constant the crate's own module writes
        # (`resizer::CURTAIN_WORD`), read where it is written.
        out = []
        for raw in re.findall(field + r":\s*([^,\n]+),", body):
            raw = raw.strip()
            m = re.fullmatch(r'"([a-z][a-z0-9-]*)"', raw)
            if m:
                out.append(m.group(1))
                continue
            const = raw.split("::")[-1]
            hit = None
            for f in shell_files("**/*.rs"):
                w = re.search(r"\bpub const " + const + r': &str = "([a-z][a-z0-9-]*)";', uncommented(f.read_text()))
                if w:
                    hit = w.group(1)
                    break
            out.append(hit or ("<" + raw + ">"))
        return out

    doors, cameras = {}, {}
    for crate in shell_crates():
        lib = crate / "src" / "lib.rs"
        if not lib.is_file():
            continue
        text = uncommented(without_tests(lib.read_text()))
        d, c = table(text, "DOORS"), table(text, "CAMERAS")
        if d is not None:
            doors[crate.name] = words(d, "name")
        if c is not None:
            cameras[crate.name] = words(c, "door")
    floor(len(doors), 25, "crates with a table of doors")
    floor(len(cameras), 15, "crates with a table of cameras")

    def listed(text, name):
        # The crates whose table the binary's list names, by their path.
        head = re.search(r"\bconst " + name + r":[^=]*=\s*&\[", text)
        if head is None:
            print(f"ERROR: the binary keeps no list of the crates' {name}", file=sys.stderr)
            raise SystemExit(1)
        return set(re.findall(r"\bronoctua_shell_(\w+)::" + name + r"\b", _matched(text, head.end() - 1)))

    main = uncommented(without_tests(shell_file("main.rs").read_text()))
    photo = uncommented(without_tests(shell_file("photograph.rs").read_text()))
    door_lists = listed(main, "DOORS")
    camera_lists = listed(photo, "CAMERAS")
    # THE BINARY'S OWN SUBCOMMANDS, by the arms of `main`'s match.
    own = set(re.findall(r'\bSome\("([a-z][a-z0-9-]*)"\)\s*=>', must(body_of(main, "main"), "`fn main`")))
    floor(len(own), 8, "the binary's own subcommands")
    # AND ITS OWN CAMERAS (`const OWN: &[Camera]` in `photograph.rs`), the
    # first table its list reads.
    own_head = re.search(r"\bconst OWN:\s*&\[Camera\]\s*=\s*&\[", photo)
    own_cameras = words(_matched(photo, own_head.end() - 1), "door") if own_head else []
    floor(len(own_cameras), 2, "the binary's own cameras")

    wrong = []
    for crate, names in sorted(doors.items()):
        if crate.removeprefix("ronoctua-shell-").replace("-", "_") not in door_lists:
            wrong.append(f"{crate} has a table of doors ({', '.join(names)}) the binary does not list in `main.rs`'s DOORS")
    for crate, names in sorted(cameras.items()):
        if crate.removeprefix("ronoctua-shell-").replace("-", "_") not in camera_lists:
            wrong.append(f"{crate} has a table of cameras ({', '.join(names)}) the binary does not list in `photograph.rs`'s CAMERAS")
    for listed_name in sorted(door_lists - {c.removeprefix("ronoctua-shell-").replace("-", "_") for c in doors}):
        wrong.append(f"`main.rs` lists ronoctua_shell_{listed_name}::DOORS, and that crate keeps no table of doors")
    for listed_name in sorted(camera_lists - {c.removeprefix("ronoctua-shell-").replace("-", "_") for c in cameras}):
        wrong.append(f"`photograph.rs` lists ronoctua_shell_{listed_name}::CAMERAS, and that crate keeps no table of cameras")
    seen = {}
    for crate, names in sorted(doors.items()):
        for n in names:
            if n.startswith("<"):
                wrong.append(f"{crate}'s door {n} is a word this check cannot read")
            elif n in own:
                wrong.append(f"{crate}'s door `{n}` is one of the binary's own subcommands, which `main` runs first")
            elif n in seen:
                wrong.append(f"the door `{n}` is {seen[n]}'s and {crate}'s: the first table listed would win")
            seen.setdefault(n, crate)
    seen = {n: "the binary" for n in own_cameras}
    for crate, names in sorted(cameras.items()):
        for n in names:
            if n.startswith("<"):
                wrong.append(f"{crate}'s camera {n} is a word this check cannot read")
            elif n in seen:
                wrong.append(f"the camera `{n}` is {seen[n]}'s and {crate}'s: the first table listed would win")
            seen.setdefault(n, crate)

    if wrong:
        print("ERROR: a window of a crate is not one the binary runs.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {sum(len(v) for v in doors.values())} doors of {len(doors)} crates and {sum(len(v) for v in cameras.values())} cameras of {len(cameras)}, every table dispatched and every word one door's")
    END
  '';
}
