# WHAT A VERB OF THE SOFTWARE CENTRE LOGS REACHES ITS JOURNAL FROM THE FIRST
# LINE (owner, 2026-11-01): `ronoctua-software` installed no `tracing`
# subscriber, and every reason a verb logged — why a release was not
# followed, why an install failed — went nowhere. `cli::install_log` writes
# the log on standard error; the unit test
# `a_verbs_log_reaches_standard_error_whatever_filter_it_inherits` holds what
# it writes, and this holds that the program installs it before anything
# else runs, so even a line it cannot parse is logged.
#
#   - `the-software-cli-logs-from-its-first-line`: the program's `main` is
#     the library's `cli::main` and nothing else; that `main`'s first
#     statement is `install_log();`; and `install_log` installs the global
#     subscriber on standard error. Asked of planted sources first.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
}:
{
  the-software-cli-logs-from-its-first-line =
    mkCheck "the-software-cli-logs-from-its-first-line" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the two `main`s and `install_log` read as Rust, comments and
        # strings off (`code_of`), the library's tests off (`without_tests`), each
        # body cut at its own braces (`body_of`).
        def statements(body):
            return [s.strip() for s in re.sub(r"\s+", " ", body).split(";") if s.strip()]

        def judged(program, library):
            wrong = []
            entry = body_of(code_of(program), "main")
            if entry is None or statements(entry) != ["ronoctua_software::cli::main()"]:
                wrong.append("the program's `main` does more than call `ronoctua_software::cli::main()`")
            code = without_tests(code_of(library))
            body = body_of(code, "main")
            if body is None:
                wrong.append("cli.rs has no `main`")
            elif not statements(body) or statements(body)[0] != "install_log()":
                wrong.append("cli.rs's `main` does something before `install_log();`, which goes unlogged")
            log = body_of(code, "install_log")
            if log is None or "set_global_default" not in log or "stderr" not in log:
                wrong.append("`install_log` no longer installs the global subscriber on standard error")
            return wrong

        # 1) THE RULE, ASKED OF PLANTED SOURCES FIRST.
        PROGRAM = "fn main() -> std::process::ExitCode {\n    ronoctua_software::cli::main()\n}\n"
        LOG = "fn install_log() {\n    let log = log_of(None, std::io::stderr);\n    tracing::subscriber::set_global_default(log);\n}\n"
        RIGHT = LOG + "pub fn main() -> ExitCode {\n    install_log();\n    let args = parse();\n    run(args)\n}\n"
        PLANTED = {
            "late": (PROGRAM, LOG + "pub fn main() -> ExitCode {\n    let args = parse();\n    install_log();\n    run(args)\n}\n"),
            "never": (PROGRAM, LOG + "pub fn main() -> ExitCode {\n    let args = parse();\n    run(args)\n}\n"),
            "nowhere": (PROGRAM, "fn install_log() {}\npub fn main() -> ExitCode {\n    install_log();\n    run()\n}\n"),
            "early": ("fn main() -> std::process::ExitCode {\n    work();\n    ronoctua_software::cli::main()\n}\n", RIGHT),
            "right": (PROGRAM, RIGHT),
        }
        for case, (program, library) in PLANTED.items():
            wrong = judged(program, library)
            if (case == "right") == bool(wrong):
                print(f"ERROR: the rule misjudges planted sources ({case}): {wrong or 'they passed'}", file=sys.stderr)
                raise SystemExit(1)

        # 2) THE TREE.
        src = pathlib.Path("${../crates/ronoctua-software/src}")
        library = (src / "cli.rs").read_text()
        floor(len(must(body_of(without_tests(code_of(library)), "main"), "cli.rs's `main`")), 200,
              "characters of the software centre's `main`")
        wrong = judged((src / "bin" / "cli.rs").read_text(), library)
        if wrong:
            print("ERROR: a verb of the software centre may log where nobody reads it.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: the program is the library's `main`, which installs the log on standard error first; {len(PLANTED)} planted sources judged")
        END
      '';
}
