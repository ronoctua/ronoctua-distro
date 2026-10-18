# THE TOOLS AS REGIONS OF THEIR OWN CRATES (the 2026-10-12 wave): the
# calculator, the icons app, the clipboard's history and git, each a view of
# its own crate — its window, its region of the footer and the one errand
# handler the two share — and the checks that keep what that bought.
#
#   - `the-tools-stand-on-the-foundation`: a tool's crate stands on the
#     foundation alone (the kit, the door, git on the git line the file
#     manager draws too) — never on another feature, the footer's binary or a
#     leaf. The compiler refuses a cycle, not a sideways edge: the calculator
#     standing on the icons app compiles, and makes every line of the icons
#     recompile the calculator and every crate on it.
#   - `a-region-leaves-nothing-behind`: a view that became a region of the
#     slot host left no half of itself in the footer — no `surface/<view>.rs`
#     ranked under its id, no line in the footer's own regions (`CONCRETE`),
#     no field `region_open` reads for it. Two halves would be the view
#     written twice, and the footer would recompile for a line of it.
#   - `the-clipboard-history-has-one-keeper`: one watcher of the clipboard,
#     one history — the footer's. The clipboard's crate (its view, its window,
#     its region) watches nothing and changes no history of its own: every
#     change is an act (`clipboard_wire::Act`) aimed at the footer's, and the
#     footer carries out a window's act and its region's by the one function.
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
  the-tools-stand-on-the-foundation = mkCheck "the-tools-stand-on-the-foundation" [ pkgs.python3 ] ''
    python3 - <<'END'
    import sys, tomllib
    exec(open("${pyPrelude}").read())
    exec(open("${shellManifestPrelude}").read())

    # PARSED: every shell crate's `[dependencies]` (`tomllib`), and the
    # shell crates each stands on, through any path.
    # FILE-WIDE: a dependency is a table of a manifest, and the question is
    # the whole graph they make — there is no item of one to narrow to.
    manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
    floor(len(manifests), 15, "shell crates' manifests")

    # WHAT EACH TOOL MAY STAND ON — the foundation, and for git the git
    # line the file manager draws (`ronoctua-shell-git-line`, below the
    # manager and the history both) with the report vocabulary it is
    # drawn in, which is the foundation too.
    TOOLS = {
        "ronoctua-shell-calc": {"ronoctua-shell-kit", "ronoctua-shell-door"},
        "ronoctua-shell-icons": {"ronoctua-shell-kit", "ronoctua-shell-door"},
        "ronoctua-shell-clipboard": {"ronoctua-shell-kit", "ronoctua-shell-door"},
        "ronoctua-shell-git": {
            "ronoctua-shell-kit",
            "ronoctua-shell-door",
            "ronoctua-shell-git-line",
            "ronoctua-shell-report",
        },
    }

    def below(crate):
        seen, todo = set(), [d for d in manifests[crate].get("dependencies", {}) if d in manifests]
        while todo:
            d = todo.pop()
            if d not in seen:
                seen.add(d)
                todo += [x for x in manifests[d].get("dependencies", {}) if x in manifests]
        return seen

    wrong = []
    for crate, allowed in sorted(TOOLS.items()):
        if crate not in manifests:
            wrong.append(f"{crate} is not a shell crate any more: the tool this check guards moved")
            continue
        under = below(crate)
        for d in sorted(under - allowed):
            wrong.append(f"{crate} stands on {d}: a line of {d} would recompile {crate} and every crate on it")
        for d in sorted(allowed - under):
            wrong.append(f"{crate} no longer stands on {d}, which this check lets it: the list is stale")
    if wrong:
        print("ERROR: a tool of the shell stands on more than the foundation.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(TOOLS)} tools stand on the foundation alone")
    END
  '';

  a-region-leaves-nothing-behind = mkCheck "a-region-leaves-nothing-behind" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # PARSED: the regions the binary registers (`hosted_regions`, from
    # `regions.rs`), the footer's own regions (`CONCRETE`, the ids its
    # `surface/<view>.rs` rank themselves under), and `region_open`'s lines
    # — with the strings kept (`uncommented`): an id is a literal.
    hosted = hosted_regions()
    floor(len(hosted), 4, "regions the slot host hosts")
    footer = uncommented(without_tests(shell_source("surface.rs")))
    floor(len(footer), 100000, "characters of the footer's module")
    wrong = []

    # 1) NO `surface/<view>.rs` RANKS ITSELF UNDER A HOSTED ID.
    ranked = {}
    for f in shell_module_files("surface"):
        m = re.search(r'pub\(super\) const ID: &str = "([a-z-]+)";', uncommented(f.read_text()))
        if m:
            ranked[m.group(1)] = f.stem
    # NONE left (the player and the video catalogue were the last two, lane
    # R5i): each view that became a region of the slot host took its file
    # with it. What is read is every file of the footer's module.
    floor(len(list(shell_module_files("surface"))), 10, "files of the footer's module")
    for rid in sorted(hosted):
        if rid in ranked:
            wrong.append(f"`{rid}` is a region of its own crate and the footer still has `surface/{ranked[rid]}.rs` for it")

    # 2) NOR A LINE IN THE FOOTER'S OWN REGIONS.
    concrete = re.search(r"const CONCRETE: &\[&str\] = &\[(.*?)\];", footer, re.S)
    if concrete is None:
        wrong.append("the footer declares no `CONCRETE`; this check has stopped reading it")
    else:
        own = {ranked.get(stem, stem) for stem in re.findall(r"\b(\w+)::ID\b", concrete.group(1))}
        own |= set(re.findall(r'"([a-z-]+)"', concrete.group(1)))
        # EMPTY NOW (every view is a region of the slot host): what is proved is
        # that the list is there and names no hosted region.
        for rid in sorted(set(hosted) & own):
            wrong.append(f"`{rid}` is a region of the slot host and a region of the footer's own (`CONCRETE`)")

    # 3) NOR A FIELD THE PRECEDENCE READS FOR IT: `region_open` answers a
    # hosted id from the host alone.
    asked = must(body_of(footer, "region_open"), "`region_open`")
    for rid in sorted(hosted):
        if re.search(r'"' + re.escape(rid) + r'"\s*=>', asked):
            wrong.append(f"`region_open` reads a field of its own for the region `{rid}`")
    if not re.search(r"\bself\.regions\.is_open\(\w+\)", asked):
        wrong.append("`region_open` no longer asks the slot host for its regions")

    if wrong:
        print("ERROR: a view that became a region left a half of itself in the footer.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(hosted)} regions of their own crates, none drawn by the footer too")
    END
  '';

  the-clipboard-history-has-one-keeper =
    mkCheck "the-clipboard-history-has-one-keeper" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import re, sys
        exec(open("${pyPrelude}").read())
        exec(open("${shellPrelude}").read())

        # FILE-WIDE: the clipboard's crate, every file of it — what it may not do
        # is anywhere in it — product code only, comments and literals off.
        crate = [c for c in shell_crates() if c.name == "ronoctua-shell-clipboard"]
        if len(crate) != 1:
            print("ERROR: no `ronoctua-shell-clipboard` among the shell's crates", file=sys.stderr)
            raise SystemExit(1)
        # A FILE OF TESTS (`clipboard_view/tests.rs`) is not the crate: a test
        # plants the history it reads.
        files = sorted(f for f in (crate[0] / "src").rglob("*.rs") if f.name != "tests.rs")
        floor(len(files), 4, "files of the clipboard's crate")
        wrong = []
        # A HISTORY'S CHANGES and a WATCHER, by their names in the clipboard's
        # boundary (`ronoctua_clipboard::Queue`) and in the door (`clipboard`,
        # the one `zwlr_data_control` watcher).
        CHANGES = re.compile(r"\.(register|remove|toggle_pinned|clear)\(")
        for f in files:
            code = code_of(without_tests(f.read_text()))
            # THE WINDOW'S CAMERA (`shoot`) builds the history it photographs —
            # a fixture, followed by nothing — and is the one place that may.
            shot = body_of(code, "shoot")
            if shot:
                code = code.replace(shot, " ")
            for m in CHANGES.finditer(code):
                wrong.append(f"{f.name} changes a history itself (`.{m.group(1)}(`): the history is the footer's")
            if re.search(r"\bclipboard::watch\b", code):
                wrong.append(f"{f.name} watches the clipboard: a second watcher is a second history")
        region = code_of(without_tests(shell_file("clipboard_region.rs").read_text()))
        carry = must(body_of(region, "carry_out"), "clipboard_region::carry_out")
        acts = set(re.findall(r"\bAct::(\w+)", carry))
        floor(len(acts), 6, "acts the region's errands ask the footer for")
        if "way.clipboard(" not in carry:
            wrong.append("the clipboard's errands no longer ask the footer's history (`Doorway::clipboard`)")

        # PARSED: the footer's service — ONE function carries out every act, with
        # no `_ =>`, and both doors' paths reach it: a window's act over the
        # socket (`clipboard_asked`) and the region's (`clipboard_acted`).
        footer = code_of(without_tests(shell_source("surface.rs")))
        service = must(body_of(footer, "clipboard_carry_out"), "Footer::clipboard_carry_out")
        wire = code_of(without_tests(shell_file("clipboard_wire.rs").read_text()))
        declared = must(scope(wire, r"pub enum Act\b"), "clipboard_wire::Act")
        variants = set(re.findall(r"^\s{4}([A-Z]\w*)\b", declared, re.M))
        floor(len(variants), 8, "variants of `clipboard_wire::Act`")
        for v in sorted(variants):
            if not re.search(r"\bAct::" + v + r"\b", service):
                wrong.append(f"`Act::{v}` is not carried out by the footer's `clipboard_carry_out`")
        if re.search(r"(?m)^\s*_\s*=>", service):
            wrong.append("`clipboard_carry_out` has a `_ =>` arm, and the next act would be dropped unsaid")
        for door in ("clipboard_asked", "clipboard_acted"):
            b = must(body_of(footer, door), "Footer::" + door)
            if "self.clipboard_carry_out(" not in b:
                wrong.append(f"`{door}` carries out an act by a path of its own, beside `clipboard_carry_out`")
        owed = must(body_of(footer, "region_owed"), "Footer::region_owed")
        if not re.search(r"Effect::Clipboard\([^)]*\)\s*=>\s*self\.clipboard_acted\(", owed):
            wrong.append("a region's clipboard act (`Effect::Clipboard`) is not carried out by `clipboard_acted`")

        if wrong:
            print("ERROR: the clipboard's history has two keepers.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(files)} files of the clipboard's crate change no history; {len(variants)} acts, one keeper")
        END
      '';
}
