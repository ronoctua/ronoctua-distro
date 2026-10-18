# THE KEYMAP IN CONTRIBUTIONS (the 2026-10-12 wave): the check that the
# binary assembles every crate's keys.
#
# The default keymap was one function of the footer's leader, and a key that
# existed was a line of it. Now each crate says its keys beside its view, in a
# `bindings::Contribution` (`pub const KEYS`; the footer's `pub const BASE`),
# and the binary assembles them from ONE list, `keymap::CONTRIBUTIONS`. A
# contribution left out of that list is a crate whose commands do not exist:
# its keys lead nowhere, `LEADER :` does not find them and its rows say "does
# nothing yet" — and nothing else notices once the crates are libraries (the
# compiler's dead-code warning, which catches it inside one binary crate, does
# not fire for a `pub` item of a library).
#
#   - `every-contribution-is-assembled` reads every contribution the shell's
#     product code declares and the list the binary assembles, and refuses one
#     declared and not listed, one listed twice, and an entry that names no
#     declared contribution.
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
  every-contribution-is-assembled = mkCheck "every-contribution-is-assembled" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # PARSED: what each crate declares (`keymap_contributions`, the prelude:
    # every `Contribution` constant of the shell's product code) against the
    # entries of the binary's list, read between its brackets.
    # FILE-WIDE: a contribution is a module-level item of whichever file of
    # the shell holds the view, and the list is one module-level constant of
    # `keymap.rs` — neither is a question about a function.
    declared = keymap_contributions()
    floor(len(declared), 20, "contributions the shell declares")

    keymap = uncommented(without_tests(shell_file("keymap.rs").read_text()))
    head = re.search(r"pub const CONTRIBUTIONS:[^=]*=\s*&\[", keymap)
    listed = re.findall(r"&\s*((?:[a-z_][a-z0-9_]*::)+)([A-Z_]+)\b", _matched(keymap, head.end() - 1) if head else "")
    floor(len(listed), 20, "entries of `keymap::CONTRIBUTIONS`")

    def owners(f):
        # The names an entry may reach a contribution by: its module
        # (`crate::stick::KEYS`) or its crate (`ronoctua_shell_stick::KEYS`).
        module = shell_name(f).removesuffix(".rs").removesuffix("/mod").replace("/", "::")
        at = len(f.parts) - 1 - f.parts[::-1].index("src")
        crate = f.parts[at - 1].replace("-", "_")
        return {module.split("::")[-1], crate}

    wrong = []
    used = [False] * len(listed)
    for f, name, _ in declared:
        where = shell_name(f)
        hits = [i for i, (path, n) in enumerate(listed)
                if n == name and set(path.rstrip(":").split("::")) & owners(f)]
        if not hits:
            wrong.append(f"`{name}` of {where} is declared and not assembled")
        elif len(hits) > 1:
            wrong.append(f"`{name}` of {where} is assembled {len(hits)} times")
        for i in hits:
            used[i] = True
    for i, (path, name) in enumerate(listed):
        if not used[i]:
            wrong.append(f"`{path}{name}` is assembled and names no contribution the shell declares")

    if wrong:
        print("ERROR: the binary does not assemble the keys every crate says.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A crate's keys exist only in `keymap::CONTRIBUTIONS`: one left out", file=sys.stderr)
        print("       leads nowhere, and `LEADER :` does not find its commands.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(declared)} contributions, each assembled once")
    END
  '';
}
