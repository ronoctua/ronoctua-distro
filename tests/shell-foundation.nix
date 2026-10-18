# THE SHELL'S FOUNDATION IN CRATES (the 2026-10-12 wave): the checks that the
# split keeps two promises the single crate kept by being one.
#
# The kit, the door, the report, the keep, the work and the inspection panels
# are libraries below the shell's binary, and the lifeline, the overlays, the
# terminal and the buffer's PNG camera are leaves on them. Two things that
# held for free in one crate now hold only if someone keeps them:
#
#   - `shell-tests-keep-the-machine-guards`: `cfg(test)` is true in a crate's
#     own test build alone. The kit's guards — a test that reaches the machine
#     fails (`daemon::about_to_act`), a copy stays in the tests
#     (`clipboard_copy::COPIED`), the file picker remembers nothing — were
#     `cfg(test)`, and the footer's tests ran inside them because they were
#     the same crate. Split, they hold in another crate's tests only through
#     the library's `test-support` feature, named in that crate's
#     `[dev-dependencies]`, and only if the switch is written to read it.
#     Without either, a test of the footer copies onto the person's clipboard
#     and opens the person's compositor, and every test still passes.
#   - `the-shell-foundation-names-no-feature`: the foundation stands on itself
#     alone — no feature, no leaf, not the footer's binary — and the PNG camera
#     on the kit alone. The compiler refuses a cycle, not a sideways edge: the
#     door depending on the overlays compiles, and makes every window's
#     plumbing recompile with a curtain.
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
  shell-tests-keep-the-machine-guards =
    mkCheck "shell-tests-keep-the-machine-guards" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import re, sys, tomllib
        exec(open("${pyPrelude}").read())
        exec(open("${shellPrelude}").read())
        exec(open("${shellManifestPrelude}").read())

        # PARSED: every shell crate's manifest (`tomllib`): its dependencies, its
        # dev-dependencies and the features each turns on, and what each crate's
        # own `test-support` brings along.
        manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
        floor(len(manifests), 11, "shell crates' manifests")
        TS = "test-support"

        def shell_deps(m, table):
            return {d: s for d, s in m.get(table, {}).items() if d in manifests}

        def features(spec):
            return spec.get("features", []) if isinstance(spec, dict) else []

        keeping = {c for c, m in manifests.items() if TS in m.get("features", {})}
        floor(len(keeping), 2, "shell crates that keep something for tests (`test-support`)")

        def brought(crate):
            # A crate's `test-support` and every other crate's it turns on
            # (`ronoctua-shell-kit/test-support`), closed.
            seen, todo = set(), [crate]
            while todo:
                c = todo.pop()
                if c in seen:
                    continue
                seen.add(c)
                for f in manifests[c].get("features", {}).get(TS, []):
                    if f.endswith("/" + TS) and f.split("/")[0] in manifests:
                        todo.append(f.split("/")[0])
            return seen

        wrong, required = [], 0
        for crate, m in sorted(manifests.items()):
            # WHAT ITS TESTS REACH: every shell crate below it, through any path.
            below, todo = set(), list(shell_deps(m, "dependencies"))
            while todo:
                d = todo.pop()
                if d not in below:
                    below.add(d)
                    todo += list(shell_deps(manifests[d], "dependencies"))
            covered = set()
            for d, spec in shell_deps(m, "dev-dependencies").items():
                if TS in features(spec):
                    covered |= brought(d)
            for d in sorted((below & keeping) - covered):
                required += 1
                wrong.append(f"{crate}'s tests reach {d}, and its [dev-dependencies] do not turn on {d}'s `test-support`:"
                             " the guards that keep a test off the machine are compiled out of them")
            required += len((below & keeping) & covered)
            # AND NEVER IN A PRODUCT: `test-support` mints a stub keymap, a fake
            # sudo gate and a door with no compositor, and it records a copy
            # instead of making it.
            for table in ("dependencies", "build-dependencies"):
                for d, spec in shell_deps(m, table).items():
                    if TS in features(spec):
                        wrong.append(f"{crate} turns on {d}'s `test-support` in [{table}]: a product would carry the tests' stubs")
            if TS in m.get("features", {}).get("default", []):
                wrong.append(f"{crate} turns its own `test-support` on by default")
        floor(required, 10, "shell crates' tests that reach a crate keeping something for tests")

        # FILE-WIDE: a switch between the tests and the product is an attribute
        # anywhere in a library's file, outside its test modules, comments off
        # and strings kept (`uncommented`: the feature's name in
        # `any(test, feature = "test-support")` is a literal). EACH test module
        # is blanked by its braces and the product after it is swept
        # (`outside_tests`): `without_tests` cuts from the first one to the end,
        # and a file whose product resumes after a mid-file `mod meter_tests`
        # (`chrome.rs`, `door/touch.rs`) would hide every switch below it.
        #
        # IN A LIBRARY, a bare `cfg(test)` may only leave out an ITEM of its own
        # tests (a helper, a `use`, a test module); a statement, a block, an arm,
        # a variant or a field it leaves out, and every `cfg(not(test))` and
        # `cfg!(test)`, is a switch that another crate's tests would run on the
        # product's side of. Those read `any(test, feature = "test-support")`.
        ITEM = re.compile(r"\s*(?:pub(?:\([^)]*\))?\s+)?(?:fn|const|static|struct|enum|mod|use|impl|type|trait|unsafe|async|extern)\b"
                          r"|\s*thread_local!|\s*macro_rules!")
        libraries = sorted({f.parent.parent for f in shell_files("lib.rs")})
        floor(len(libraries), 9, "shell libraries")
        swept = 0
        switches = 0
        for crate in libraries:
            for f in sorted((crate / "src").rglob("*.rs")):
                swept += 1
                lines = uncommented(outside_tests(f.read_text())).split("\n")
                where = crate.name + "/src/" + shell_name(f)
                for i, line in enumerate(lines):
                    if "test-support" in line:
                        switches += 1
                    if re.search(r"cfg\(\s*not\(\s*test\s*\)\s*\)|cfg!\(\s*(?:not\(\s*)?test\s*\)", line):
                        wrong.append(f"{where}:{i + 1}: a switch on `test` alone, which another crate's tests run on the product's side of")
                    if not re.search(r"#\[cfg\(\s*test\s*\)\]", line):
                        continue
                    # THE NEXT CODE: past the attribute, the blank lines a doc
                    # comment left, and the other attributes of the same item.
                    rest = line[line.index("]") + 1:]
                    j = i
                    while (not rest.strip() or rest.strip().startswith("#[")) and j + 1 < len(lines):
                        j += 1
                        rest = lines[j]
                    if not ITEM.match(rest):
                        wrong.append(f"{where}:{i + 1}: `cfg(test)` leaves out `{rest.strip()[:50]}`, which is not an item of"
                                     " this crate's own tests: another crate's tests run without it")
        floor(swept, 80, "files of the shell's libraries swept")
        floor(switches, 10, "switches read under `test-support`")

        if wrong:
            print("ERROR: a test of the shell would run outside the machine's guards.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            print("", file=sys.stderr)
            print("       `cfg(test)` is true in a crate's own tests only. What a library keeps for", file=sys.stderr)
            print("       tests is `any(test, feature = \"test-support\")`, and every shell crate that", file=sys.stderr)
            print("       stands on it turns that feature on in its [dev-dependencies].", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(manifests)} shell crates, {required} of whose tests reach a library's guards and keep them;"
              f" {swept} library files, {switches} switches, none on `test` alone")
        END
      '';

  the-shell-foundation-names-no-feature =
    mkCheck "the-shell-foundation-names-no-feature" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import sys, tomllib
        exec(open("${pyPrelude}").read())
        exec(open("${shellManifestPrelude}").read())

        # PARSED: every shell crate's `[dependencies]` (`tomllib`), and the
        # shell crates each stands on, through any path.
        # FILE-WIDE: a dependency is a table of a manifest, and the question is
        # the whole graph they make — there is no item of one to narrow to.
        manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
        floor(len(manifests), 11, "shell crates' manifests")

        # THE FOUNDATION: what every door, the footer and every feature stand on.
        # A helper two features use comes down into it; nothing in it names one.
        FOUNDATION = {
            "ronoctua-shell-kit", "ronoctua-shell-door", "ronoctua-shell-report",
            "ronoctua-shell-keep", "ronoctua-shell-work", "ronoctua-shell-inspect",
        }
        # AND WHAT MAY STAND ON THE KIT ALONE: the vocabulary has no shell crate
        # below it, and the buffer's PNG camera draws a frame with the host's own
        # arithmetic and links no window's plumbing and no footer.
        ON_NOTHING = {"ronoctua-shell-kit": set(), "ronoctua-render-png": {"ronoctua-shell-kit"}}

        def below(crate):
            seen, todo = set(), [d for d in manifests[crate].get("dependencies", {}) if d in manifests]
            while todo:
                d = todo.pop()
                if d not in seen:
                    seen.add(d)
                    todo += [x for x in manifests[d].get("dependencies", {}) if x in manifests]
            return seen

        wrong = []
        for crate in sorted(FOUNDATION):
            if crate not in manifests:
                wrong.append(f"{crate} is not a shell crate any more: the foundation this check guards moved")
                continue
            for d in sorted(below(crate) - FOUNDATION):
                wrong.append(f"{crate} stands on {d}, which is not the foundation: a line of {d} would recompile"
                             f" {crate} and every crate on it")
        for crate, allowed in sorted(ON_NOTHING.items()):
            if crate in manifests:
                for d in sorted(below(crate) - allowed):
                    wrong.append(f"{crate} stands on {d}")
            else:
                wrong.append(f"{crate} is not a shell crate any more")
        # NOTHING STANDS ON THE BINARY: it is where the crates meet.
        for crate, m in sorted(manifests.items()):
            if "ronoctua-shell" in m.get("dependencies", {}):
                wrong.append(f"{crate} stands on the shell's binary")
        edges = sum(len(below(c)) for c in manifests)
        floor(edges, 20, "edges between shell crates")

        if wrong:
            print("ERROR: the shell's foundation names what stands on it.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(FOUNDATION)} foundation crates stand on the foundation alone; {edges} edges between {len(manifests)} shell crates")
        END
      '';
}
