# A SENTENCE THAT TEACHES KEYS TEACHES THE LEADER WITH THEM (owner,
# 2026-11-01): `bindings::keys` answers the keys after the leader alone
# (`t t`), and `bindings::taught` the leader's label with them (`SUPER t t`).
# A door that put the first in a sentence taught a sequence nobody can type —
# the video catalogue's download sentence did. So `keys` is asked inside the
# kit alone, where `taught` is made of it.
#
#   - `keys-are-taught-with-the-leader`: no source of a crate but
#     `ronoctua-shell-kit` names `bindings::keys` (a call, or the function
#     handed on), brings it in by a `use`, renames `bindings`, or brings all
#     of it in. Asked of a planted tree first — each way of reaching `keys`,
#     the kit's own call, and a sentence that only names it — so the sweep is
#     known to refuse what it is for before it is believed about the tree.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
}:
{
  keys-are-taught-with-the-leader = mkCheck "keys-are-taught-with-the-leader" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys, tempfile
    exec(open("${pyPrelude}").read())

    # FILE-WIDE: a call may sit anywhere in a crate, its tests included — a
    # test holding `t t` holds a sentence nobody can type. `code_of` takes the
    # comments and the strings off, so a sentence naming the function is no
    # call.
    KIT = "ronoctua-shell-kit"
    NAMED = re.compile(r"\bbindings\s*::\s*keys\b")
    GROUPED = re.compile(r"\bbindings\s*::\s*\{[^}]*\bkeys\b[^}]*\}")
    RENAMED = re.compile(r"\bbindings\s+as\s+\w+")
    ALL = re.compile(r"\bbindings\s*::\s*\*")

    def judged(crates):
        wrong, swept = [], 0
        for f in workspace_sources(crates):
            rel = f.relative_to(crates)
            if rel.parts[0] == KIT:
                continue
            swept += 1
            code = code_of(f.read_text())
            if NAMED.search(code):
                wrong.append(f"{rel} names `bindings::keys`: a sentence takes `bindings::taught`, the leader with the keys")
            if GROUPED.search(code):
                wrong.append(f"{rel} brings `keys` in from `bindings`, to call it bare")
            if RENAMED.search(code):
                wrong.append(f"{rel} renames `bindings`: a `keys` asked through the new name would not be seen")
            if ALL.search(code):
                wrong.append(f"{rel} brings all of `bindings` in, `keys` with it: name what it uses")
        return wrong, swept

    # 1) THE RULE, ASKED OF A PLANTED TREE FIRST.
    PLANTED = {
        "called": "fn f() -> Option<String> { ronoctua_shell_kit::bindings::keys(\"transfers-open\", None) }\n",
        "handed": "fn f() { let ask = bindings::keys; ask(\"transfers-open\", None); }\n",
        "grouped": "use ronoctua_shell_kit::bindings::{label, keys};\nfn f() { keys(\"transfers-open\", None); }\n",
        "renamed": "use ronoctua_shell_kit::bindings as b;\nfn f() { b::keys(\"transfers-open\", None); }\n",
        "all": "use ronoctua_shell_kit::bindings::*;\nfn f() { keys(\"transfers-open\", None); }\n",
        "tested": "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { bindings::keys(\"t\", None); }\n}\n",
        "taught": "fn f() -> Option<String> { ronoctua_shell_kit::bindings::taught(\"transfers-open\", None) }\n",
        "said": "// `bindings::keys` is the kit's own\nfn f() -> &'static str { \"bindings::keys(\" }\n",
    }
    RIGHT = {"taught", "said"}
    with tempfile.TemporaryDirectory() as planted:
        root = pathlib.Path(planted) / "crates"
        kit = root / KIT / "src"
        kit.mkdir(parents=True)
        (root / KIT / "Cargo.toml").write_text("")
        (kit / "bindings.rs").write_text("pub fn keys() {}\npub fn taught() { keys(); bindings::keys(); }\n")
        for case, text in PLANTED.items():
            src = root / f"ronoctua-shell-{case}" / "src"
            src.mkdir(parents=True)
            (root / f"ronoctua-shell-{case}" / "Cargo.toml").write_text("")
            (src / "lib.rs").write_text(text)
        wrong, swept = judged(root)
        if swept != len(PLANTED):
            print(f"ERROR: the planted tree's {len(PLANTED)} crates were swept as {swept}", file=sys.stderr)
            raise SystemExit(1)
        for case in PLANTED:
            caught = [w for w in wrong if w.startswith(f"ronoctua-shell-{case}/")]
            if (case in RIGHT) == bool(caught):
                print(f"ERROR: the rule misjudges a planted crate ({case}): {caught or 'it passed'}", file=sys.stderr)
                raise SystemExit(1)

    # 2) THE TREE.
    wrong, swept = judged(pathlib.Path("${../crates}"))
    floor(swept, 300, "sources swept for a door that asks `bindings::keys`")
    if wrong:
        print("ERROR: a sentence could teach keys without the leader.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {swept} sources outside the kit never ask `bindings::keys`; {len(PLANTED)} planted crates judged")
    END
  '';
}
