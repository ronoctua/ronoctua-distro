# THE FILE MANAGER, A FEATURE CRATE OF ITS OWN (the 2026-10-12 wave, lane
# R5h): its view, its window, its region of the footer, its archives and its
# camera live in `ronoctua-shell-files`, and the footer hosts it as a region
# laid out in sections (`region::Shape::Sections`). Three things keep the move
# worth having, and none of them is a compiler's question:
#
#   - `the-file-manager-carries-out-once`: what the view asks — a step, an
#     operation, a thread's report — is carried out by ONE handler
#     (`files_manager::Manager`), which both doors call over their doorway;
#     neither door matches a step or an operation of its own. Two handlers of
#     the same steps is how the two doors came to do different things.
#   - `the-file-manager-stands-on-the-foundation`: the manager stands on the
#     foundation and the git line below it, and on no other feature (the
#     security centre's vault flow is handed in by the binary), and no crate
#     but the binary stands on it — or a line of it would recompile another
#     feature, and a line of another feature would recompile it.
#   - `the-footer-names-no-file-manager`: the footer's module names no part of
#     the manager — it asks the shown region in sections what each part is,
#     and its only word for the manager is the rank it is ranked under.
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
  the-file-manager-carries-out-once = mkCheck "the-file-manager-carries-out-once" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # PARSED: the manager's one handler, narrowed with `body_of` to
    # `Manager::carry_out`, and each door's file, its product with the
    # comments and the literals off (`code_of`, `outside_tests`). FILE-WIDE
    # for the doors: an arm over a step or an operation anywhere in either
    # door is a second handler, wherever it is written.
    manager = code_of(outside_tests(shell_file("files_manager.rs").read_text()))
    floor(len(manager), 4000, "characters of files_manager.rs")
    wrong = []

    # 1) THE ONE HANDLER answers every step the view gives — by name, no
    # wildcard: a step added without its arm does not compile, and an arm
    # `_ =>` would carry the next one out as nothing, in both doors at once.
    carry = must(body_of(manager, "carry_out"), "`Manager::carry_out`")
    report = code_of(must(scope(code_of(outside_tests(shell_file("report.rs").read_text())),
                                r"pub enum Step\b"), "`report::Step`"))
    steps = re.findall(r"^\s{4}([A-Z]\w*)\b", report, re.M)
    floor(len(steps), 6, "variants of `report::Step`")
    for v in steps:
        if not re.search(r"\bStep::" + v + r"\b", carry):
            wrong.append(f"`Manager::carry_out` does not carry out `Step::{v}` by its name")
    if re.search(r"(?m)^\s*_\s*=>", carry):
        wrong.append("`Manager::carry_out` has a wildcard arm over the steps")
    # AND IT GOES THROUGH THE DOORWAY: what a step does in a door is asked of
    # the door it is in, never done to one door's own state.
    for needed in ("way.page(Paging::Follow)", "way.fold(", "way.paste()", "way.ask(", "way.close()"):
        if needed not in carry:
            wrong.append(f"`Manager::carry_out` no longer asks the doorway (`{needed}`)")

    # 2) NEITHER DOOR CARRIES OUT A STEP OF ITS OWN: no arm over a step and no
    # operation (`Doing::`) in the window's file or the region's — each hands
    # the view's step to the one handler (`.carry_out(`).
    for door in ("files_window.rs", "files_region.rs"):
        text = code_of(outside_tests(shell_file(door).read_text()))
        floor(len(text), 4000, "characters of " + door)
        for m in re.finditer(r"\bStep::\w+(\([^)]*\))?\s*=>", text):
            line = text[: m.start()].count("\n") + 1
            wrong.append(f"{door}:{line}: an arm over a step (`{m.group(0).strip()}`)")
        for m in re.finditer(r"\bDoing::\w+", text):
            line = text[: m.start()].count("\n") + 1
            wrong.append(f"{door}:{line}: an operation handled in the door (`{m.group(0)}`)")
        if ".carry_out(" not in text:
            wrong.append(f"{door} never hands a step to `Manager::carry_out`")

    # 3) AND THE FOOTER CARRIES OUT NONE: its module neither matches a step of
    # the manager's nor names one of its operations.
    footer = code_of(without_tests(shell_source("surface.rs")))
    floor(len(footer), 100000, "characters of the footer's module")
    for spelled in ("report::Step", "report::Doing", "Step::Do(", "Doing::Open"):
        if spelled in footer:
            wrong.append(f"the footer carries out a step of the manager's (`{spelled}`)")

    if wrong:
        print("ERROR: the file manager's steps have more than one handler.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       The view's step is carried out by `Manager::carry_out`, once,", file=sys.stderr)
        print("       over the door's doorway: the two doors do the same things", file=sys.stderr)
        print("       because there is one place that does them.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(steps)} steps carried out by one handler, and both doors hand theirs to it")
    END
  '';

  the-file-manager-stands-on-the-foundation =
    mkCheck "the-file-manager-stands-on-the-foundation" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import sys, tomllib
        exec(open("${pyPrelude}").read())
        exec(open("${shellManifestPrelude}").read())

        # PARSED: every shell crate's `[dependencies]` and `[dev-dependencies]`
        # (`tomllib`), and the shell crates each stands on, through any path.
        # FILE-WIDE: a manifest is read whole, by that parser, for its two
        # dependency tables — never searched.
        manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
        floor(len(manifests), 12, "shell crates' manifests")
        MANAGER, LINE, BIN = "ronoctua-shell-files", "ronoctua-shell-git-line", "ronoctua-shell"
        FOUNDATION = {
            "ronoctua-shell-kit", "ronoctua-shell-door", "ronoctua-shell-report",
            "ronoctua-shell-keep", "ronoctua-shell-work", "ronoctua-shell-inspect",
        }
        wrong = []
        for crate in (MANAGER, LINE, BIN):
            if crate not in manifests:
                print(f"ERROR: no shell crate `{crate}`", file=sys.stderr)
                raise SystemExit(1)

        def below(crate, tables=("dependencies",)):
            seen, todo = set(), []
            for t in tables:
                todo += [d for d in manifests[crate].get(t, {}) if d in manifests]
            while todo:
                d = todo.pop()
                if d not in seen:
                    seen.add(d)
                    todo += [x for x in manifests[d].get("dependencies", {}) if x in manifests]
            return seen

        # THE MANAGER: the foundation and the git line, and no other feature —
        # its tests included, which unify features over the same graph.
        for d in sorted(below(MANAGER, ("dependencies", "dev-dependencies")) - FOUNDATION - {LINE}):
            wrong.append(f"{MANAGER} stands on {d}: a line of {d} would recompile the file manager")
        if LINE not in below(MANAGER):
            wrong.append(f"{MANAGER} no longer stands on {LINE}: where does it read its git line?")
        # THE GIT LINE: the kit and the report, the vocabulary it is drawn in.
        for d in sorted(below(LINE) - {"ronoctua-shell-kit", "ronoctua-shell-report"}):
            wrong.append(f"{LINE} stands on {d}: the line both features draw would recompile with it")
        # AND NOTHING BUT THE BINARY STANDS ON THE MANAGER: a feature is a leaf.
        for crate, m in sorted(manifests.items()):
            if crate == BIN or crate == MANAGER:
                continue
            if MANAGER in m.get("dependencies", {}) or MANAGER in m.get("dev-dependencies", {}):
                wrong.append(f"{crate} stands on {MANAGER}: a line of the manager would recompile it")
        if MANAGER not in manifests[BIN].get("dependencies", {}):
            wrong.append(f"the binary does not stand on {MANAGER}: nothing registers the manager")

        if wrong:
            print("ERROR: the file manager stands on more than the foundation.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            print("", file=sys.stderr)
            print("       The manager is a feature crate: it stands on the foundation and on", file=sys.stderr)
            print("       the git line below it, what another feature does for it is handed", file=sys.stderr)
            print("       in by the binary (`vault::Flow`), and only the binary stands on it.", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {MANAGER} stands on {len(below(MANAGER))} crates of the foundation and the git line, and only the binary on it")
        END
      '';

  the-footer-names-no-file-manager = mkCheck "the-footer-names-no-file-manager" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # FILE-WIDE: the question is what the footer's module NAMES, anywhere in
    # its product (`without_tests` of the module's text, every file of it),
    # with the comments off and the strings kept (`uncommented`): a module of
    # the manager is named by a path, and its id would be named by a string.
    footer = uncommented(without_tests(shell_source("surface.rs")))
    floor(len(footer), 100000, "characters of the footer's module")
    files = [c for c in shell_crates() if c.name == "ronoctua-shell-files"]
    if len(files) != 1:
        print("ERROR: no shell crate `ronoctua-shell-files`", file=sys.stderr)
        raise SystemExit(1)
    lib = uncommented((files[0] / "src" / "lib.rs").read_text())
    modules = set(re.findall(r"^\s*pub mod (\w+);", lib, re.M))
    floor(len(modules), 5, "modules of the file manager's crate")
    # THE FOOTER'S OWN MODULES are not the manager's: `crate::key` in the
    # footer is the footer's key file (every crate that speaks has one), so a
    # `crate::` path names the manager only for a module the footer's crate
    # does not declare itself. `ronoctua_shell_files::` is refused for all.
    footer_crate = [c for c in shell_crates() if c.name == "ronoctua-shell-footer"]
    if len(footer_crate) != 1:
        print("ERROR: no shell crate `ronoctua-shell-footer`", file=sys.stderr)
        raise SystemExit(1)
    own = set(re.findall(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod (\w+)\s*;",
                         uncommented((footer_crate[0] / "src" / "lib.rs").read_text()), re.M))
    floor(len(own), 10, "modules of the footer's own crate")
    wrong = []
    for m in sorted(modules):
        path = r"\bronoctua_shell_files::" if m in own else r"\b(?:crate|ronoctua_shell_files)::"
        for hit in re.finditer(path + m + r"\b", footer):
            line = footer[: hit.start()].count("\n") + 1
            wrong.append(f"the footer names the manager's `{m}` (line {line} of the module's text)")
    # ITS ONLY WORD FOR THE MANAGER IS ITS RANK: the id appears once, in
    # `PRECEDENCE`, and nowhere else of the module. (The binary's own word
    # that opens the manager's WINDOW — `"files".to_owned()`, its command
    # line — is a window's, as every door opens another by its words.)
    ranks = re.findall(r'"files"(?!\.to_owned\(\))', footer)
    if len(ranks) != 1 or 'Rank::Region("files")' not in footer:
        wrong.append(f'the id `"files"` is written {len(ranks)} times in the footer, not once as its rank')

    if wrong:
        print("ERROR: the footer names the file manager.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       The footer hosts the manager as a region laid out in sections and", file=sys.stderr)
        print("       asks it what each part is (`sections::Sectioned`): a line of the", file=sys.stderr)
        print("       manager never recompiles the footer, and the footer never decides", file=sys.stderr)
        print("       anything the manager's doors do not both do.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: the footer names none of the manager's {len(modules)} modules, and ranks it once")
    END
  '';
}
