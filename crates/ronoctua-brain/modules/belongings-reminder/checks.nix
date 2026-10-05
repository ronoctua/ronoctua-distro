# The belongings' reminder's own checks, in the shape every module's
# `checks.nix` has (`nix/brain-modules.nix`, `checkArguments`).
{
  mkCheck,
  pkgs,
  pyPrelude,
  ...
}:

{
  checks = {
    # A CLICK ON ITS MARK REACHES THE BELONGINGS.
    #
    # The marks' act is a COMMAND of the footer's, named by the mark's own id
    # (`MarkId::opening`, the one way a mark opens a command), not an action
    # of this module, and the brain never sees the click: the footer
    # runs the command by name. A name that no command of the registry has
    # would fall through `run_with` into a log line — a mark the person
    # clicks and nothing happens, with nothing on the screen to say so. So
    # both names the module writes must be commands the leader registers, and
    # the ones the belongings' view calls its own (`OPEN_COMMAND`,
    # `SYNC_COMMAND`).
    belongings-reminder-marks-reach-the-belongings =
      mkCheck "belongings-reminder-marks-reach-the-belongings" [ pkgs.python3 ]
        ''
          python3 - <<'END'
          import pathlib, re, sys
          exec(open("${pyPrelude}").read())

          # PARSED: two `const` string items of the module (comments off, strings
          # kept), the leader's `Command { id: "…" }` literals in its COMMANDS
          # table, and two `const` items of the view.
          # FILE-WIDE: every item asked about is module-level — two `const`s
          # of the module, two of the view, and the leader's table, found by
          # its own name and cut at its own closing bracket below.
          module = uncommented(pathlib.Path("${./module.rs}").read_text())
          leader = uncommented(without_tests(pathlib.Path("${../../../ronoctua-shell/src/leader.rs}").read_text()))
          view = uncommented(pathlib.Path("${../../../ronoctua-shell/src/belongings.rs}").read_text())
          floor(len(module), 2000, "characters of the module")
          floor(len(leader), 20000, "characters of the leader")

          def const(text, name, where):
              m = re.search(r"const " + name + r": &str = \"([^\"]+)\";", text)
              if not m:
                  sys.exit(f"ERROR: `{name}` went away from {where}")
              return m.group(1)

          named = {"SYNC": const(module, "SYNC", "the module"), "OPEN": const(module, "OPEN", "the module")}
          # THE TABLE BY ITS BRACKETS, from `= &[` to the one that closes it:
          # `body_of` would stop at the first `Command {` inside.
          opened = leader.find("const COMMANDS: &[Command] = &[")
          if opened < 0:
              sys.exit("ERROR: the leader's COMMANDS table went away")
          at = leader.index("= &[", opened) + 3
          depth, end = 0, at
          for end in range(at, len(leader)):
              depth += {"[": 1, "]": -1}.get(leader[end], 0)
              if depth == 0:
                  break
          table = must(leader[at:end], "the leader's COMMANDS")
          registered = set(re.findall(r'id: "([a-z0-9-]+)"', table))
          floor(len(registered), 100, "commands in the leader's table")
          wrong = []
          for what, command in named.items():
              if command not in registered:
                  wrong.append(f"the mark's {what} runs `{command}`, which the leader does not register")
          if named["SYNC"] != const(view, "SYNC_COMMAND", "the belongings' view"):
              wrong.append("the mark's SYNC is not the view's SYNC_COMMAND")
          if named["OPEN"] != const(view, "OPEN_COMMAND", "the belongings' view"):
              wrong.append("the mark's OPEN is not the view's OPEN_COMMAND")
          # PARSED: the commands reach the marks through their ids — the
          # product code (tests cut) calls `.opening(` and hands it both
          # constants, past their own definitions.
          code = without_tests(code_of(module))
          if ".opening(" not in code:
              wrong.append("no mark names its command by its id (`MarkId::opening`)")
          for what in ("SYNC", "OPEN"):
              if len(re.findall(r"\b" + what + r"\b", code)) < 2:
                  wrong.append(f"`{what}` is defined and no mark opens it")
          if wrong:
              for w in wrong:
                  print("ERROR: " + w, file=sys.stderr)
              sys.exit(1)
          print(f"OK: the reminder's marks run {sorted(named.values())}, both commands of the registry and of the view")
          END
        '';
  };
  slow = [ ];
}
