# THE VIEWS THE FOOTER HOSTS (the 2026-10-12 wave): the check that a view
# which became a region of the slot host left the footer.
#
# A feature's view becomes a `region::Region` of its own crate, registered by
# the binary in `regions.rs` and hosted by the footer by its id
# (`surface/host.rs`). What that is worth is that the footer no longer
# compiles it: a line of the camera's settings, the manuals, the transfer
# queue's panel or the notification manager recompiles their crate and the
# binary, and nothing of the footer. The footer is a crate of its own now
# (`ronoctua-shell-footer`, lane R6), which the compiler keeps off every
# feature it does not depend on and `footer-names-no-feature` keeps off every
# feature at all. What this still asks:
#
#   - `a-hosted-region-leaves-the-footer`: for every region the binary
#     registers, no view file of the footer (`surface/<view>.rs`) ranks
#     itself under the region's id any more — the footer drawing it with a
#     field of its own AND hosting it would be the view twice — and no file of
#     the footer's module names a module of the region's crate, nor the crate.
#     The footer's own lines and marks of a region's daemon (the radio, the
#     screens, the way out, the person's programs, the player's pair) say that
#     view's words leaf, under both — and a line that excused one would excuse
#     nothing now (`FOOTER_HALVES`, empty since R6).
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellPrelude,
}:
{
  a-hosted-region-leaves-the-footer = mkCheck "a-hosted-region-leaves-the-footer" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # FILE-WIDE: the question is what the footer's FILES name and rank —
    # every file of its module, its product (`without_tests`) with the
    # comments off and the strings kept (`uncommented`), since a view is
    # ranked by a literal id — and what the binary's registry registers,
    # narrowed to its `ENTRY` paths.
    registry = uncommented(without_tests(shell_file("regions.rs").read_text()))
    entries = re.findall(r"\bronoctua_shell_(\w+)::(\w+)::ENTRY\b", registry)
    floor(len(entries), 4, "regions the binary registers")
    crates = {c.name: c for c in shell_crates()}
    footer = {f: uncommented(without_tests(f.read_text())) for f in shell_module_files("surface")}
    # SEVENTEEN once the calculator, the icons app, the clipboard and git left
    # it for crates of their own (R5f), and falling with each view that does.
    floor(len(footer), 12, "files of the footer's module")
    floor(sum(len(t) for t in footer.values()), 100000, "characters of the footer's module")
    # THE FOOTER'S OWN LINES AND MARKS OF A REGION'S DAEMON, which the R5
    # lanes left in the footer and the footer's crate took out (lane R6): the
    # radio, the screens, the way out, the modeline's marks, the person's
    # programs and the player's pair now say each view's words leaf
    # (`<view crate>-words`), under the view and the footer. An excuse added
    # back is a feature named again: each by the footer's file and the module
    # it names, and one that names nothing is refused below.
    FOOTER_HALVES = set()
    # THE FOOTER'S OWN MODULES are not a region's: `crate::key` in the footer
    # is the footer's key file (every crate that speaks has one, `pub mod
    # key;`), and a region's crate having a module of the same name does not
    # make the footer name it. The crate's own path, `ronoctua_shell_<crate>`,
    # is refused whatever the module.
    footer_crate = crates["ronoctua-shell-footer"]
    footers_own = set(re.findall(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod (\w+)\s*;",
                                 uncommented((footer_crate / "src" / "lib.rs").read_text()), re.M))
    floor(len(footers_own), 10, "modules of the footer's own crate")
    excused = set()
    wrong = []
    checked = 0
    for crate_word, module in entries:
        name = "ronoctua-shell-" + crate_word.replace("_", "-")
        root = crates.get(name)
        if root is None:
            wrong.append(f"`regions.rs` registers `{name}`'s region, and there is no such shell crate")
            continue
        source = root / "src" / f"{module}.rs"
        if not source.is_file():
            wrong.append(f"`regions.rs` registers `{name}::{module}`, and {name} has no `src/{module}.rs`")
            continue
        own = re.search(r'\bpub const ID: &str = "([a-z-]+)";', uncommented(without_tests(source.read_text())))
        if own is None:
            wrong.append(f"{name}/src/{module}.rs writes no `ID` its region is ranked under")
            continue
        rid = own.group(1)
        lib = uncommented((root / "src" / "lib.rs").read_text())
        modules = set(re.findall(r"^\s*pub mod (\w+);", lib, re.M))
        floor(len(modules), 2, f"modules of {name}")
        for f, text in sorted(footer.items()):
            # 1) NO VIEW FILE OF THE FOOTER RANKS ITSELF UNDER THE REGION'S ID.
            if re.search(r'\bpub\(super\) const ID: &str = "' + re.escape(rid) + r'";', text):
                wrong.append(
                    f"surface/{f.name} still ranks itself `{rid}`: the footer draws the region "
                    f"with a field of its own and hosts it from {name} too"
                )
            # 2) NOR NAMES THE REGION'S CRATE, or one of its modules.
            if re.search(r"\bronoctua_shell_" + crate_word + r"\b", text):
                wrong.append(f"surface/{f.name} names the crate {name}, whose region it hosts")
            for m in sorted(modules - footers_own):
                if re.search(r"\bcrate::" + m + r"\b", text):
                    if (f.name, m) in FOOTER_HALVES:
                        excused.add((f.name, m))
                        continue
                    wrong.append(
                        f"surface/{f.name} names `crate::{m}`, a module of {name} — the footer "
                        f"would compile again for a line of the region it hosts"
                    )
        checked += 1
    for f, m in sorted(FOOTER_HALVES - excused):
        wrong.append(f"surface/{f} is excused for naming `crate::{m}`, and names it no more: take it off `FOOTER_HALVES`")

    if wrong:
        print("ERROR: a view the footer hosts has not left the footer.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A region of the slot host is its crate's: the footer knows it by", file=sys.stderr)
        print("       its id, lends it what it holds and does what it asks, and names", file=sys.stderr)
        print("       nothing of it — or a line of the view recompiles the footer again.", file=sys.stderr)
        raise SystemExit(1)
    # AFTER what is wrong is said: a registration that names nothing is
    # one fewer region checked, and the sentence says which.
    floor(checked, 4, "hosted regions checked")
    print(f"OK: {checked} regions the footer hosts, none drawn by a field of its own, none named by it but the {len(excused)} halves of its own lines")
    END
  '';
}
