# The LLM accounts module's own checks, in the shape every module's
# `checks.nix` has: the arguments it asks for by name (`nix/brain-modules.nix`,
# `checkArguments`), and its checks, each named after its id.
{
  mkCheck,
  pkgs,
  lib,
  self,
  pyPrelude,
  module,
  ...
}:

let
  vm = self.nixosConfigurations.vm.config;
  service = unit: lib.removeSuffix ".service" unit.name;
  unit = vm.systemd.user.services.${service module.units.run};
in
{
  checks = {
    # THE BRAIN STARTS THE UNIT IT DECLARES, AND NOTHING ELSE DOES.
    #
    # Three things have to agree, each written somewhere else: the unit's
    # name (this manifest), the program it runs (the accounts' CLI, the very
    # store path `nix/modules/llm.nix` puts on the PATH) and its verb
    # (`refresh`, the one that counts and asks). And no clock may start it: a
    # `wantedBy` or a timer would count and ask with nobody using an account —
    # the "no clock at rest" this module exists to keep. Read as the machine
    # gets it: the evaluated configuration, not the file.
    agent-accounts-starts-the-unit-it-declares =
      let
        facts = pkgs.writeText "agent-accounts-unit.json" (
          builtins.toJSON {
            name = module.units.run.name;
            type = unit.serviceConfig.Type or "";
            execStart = builtins.unsafeDiscardStringContext unit.serviceConfig.ExecStart;
            cli = builtins.unsafeDiscardStringContext "${vm.ronoctua.llm.cli}";
            wantedBy = unit.wantedBy or [ ];
            startAt = unit.startAt or [ ];
            timers = builtins.attrNames (
              lib.filterAttrs (_: t: (t.unitConfig.Unit or "") == module.units.run.name) vm.systemd.user.timers
            );
            timerNamed = vm.systemd.user.timers ? ${service module.units.run};
            nice = unit.serviceConfig.Nice or null;
          }
        );
      in
      mkCheck "agent-accounts-starts-the-unit-it-declares" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, sys
        # PARSED: the unit arrives as the evaluated configuration, JSON.
        facts = json.load(open("${facts}"))
        wrong = []
        if facts["type"] != "oneshot":
            wrong.append(f"{facts['name']} is not a oneshot ({facts['type']!r}): the run ends when the count does")
        if facts["execStart"] != facts["cli"] + "/bin/ronoctua-llm refresh":
            wrong.append(f"{facts['name']} does not run the accounts' CLI's `refresh` by its store path: "
                         f"{facts['execStart']!r} (the CLI is {facts['cli']})")
        if facts["wantedBy"] or facts["startAt"]:
            wrong.append(f"{facts['name']} is started by {facts['wantedBy'] + facts['startAt']}: the "
                         "brain starts it after use, never on a clock")
        if facts["timers"] or facts["timerNamed"]:
            wrong.append(f"a timer starts {facts['name']}: no clock at rest")
        if not (isinstance(facts["nice"], int) and facts["nice"] > 0):
            wrong.append(f"{facts['name']} is not niced ({facts['nice']})")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the brain starts {facts['name']}, the CLI's refresh, and no clock does")
        END
      '';

    # THE MODULE NAMES NO PATH OF THE AGENTS', AND ITS MARKS OPEN A COMMAND.
    #
    # Where an agent keeps its state is `ronoctua_llm`'s to say (`watched`,
    # `one-llm-boundary`): a path spelled in the module would stay on the old
    # layout the day the crate learnt a new one, and the marks would go quiet.
    # The one file it names is its own lines'. Its marks open a command the
    # shell's registry has, and every agent the crate knows has its mark.
    #
    # NOT ASKED HERE: whether the bar in the view and the mark on the modeline
    # agree. That is behaviour, proven by the view's
    # `a_bar_and_its_mark_wear_one_colour` — one reading handed to the module
    # (`windows_said`, `mark_tone`) and to the view (`windows_of`,
    # `window_tint`), compared for every window, moment and pair of lines —
    # and this check only refuses the tree without it.
    agent-accounts-opens-a-command-and-names-no-path =
      mkCheck "agent-accounts-opens-a-command-and-names-no-path" [ pkgs.python3 ]
        ''
          python3 - <<'END'
          import pathlib, re, sys
          exec(open("${pyPrelude}").read())

          # PARSED: the module's code without its tests (`without_tests`),
          # strings kept where the needle is one (`uncommented`); the crate's
          # `Kind::id` by its body; the registry's ids as `leader.rs` writes
          # them (`id: "…"`); the manifest as Nix read it.
          # FILE-WIDE: the view's file, for the test that proves the colours.
          kept = uncommented(without_tests(pathlib.Path("${./module.rs}").read_text()))
          leader = uncommented(pathlib.Path("${../../../ronoctua-shell/src/leader.rs}").read_text())
          view = pathlib.Path("${../../../ronoctua-shell/src/agents.rs}").read_text()
          llm = uncommented(without_tests(pathlib.Path("${../../../ronoctua-llm/src/lib.rs}").read_text()))
          floor(len(kept), 4000, "characters of the module's code")
          floor(len(leader), 100000, "characters of leader.rs")
          wrong = []

          # THE ONE STRING THAT IS A FILE is its own lines'; any other path is
          # a layout of somebody else's written here.
          literals = re.findall(r'"([^"\n]*)"', kept)
          floor(len(literals), 5, "string literals in the module")
          for literal in literals:
              if "/" in literal or literal.endswith((".json", ".jsonl")) and literal != "lines.json":
                  wrong.append(f"the module writes a path of its own: {literal!r}")

          opens = re.search(r'const OPENS: &str = "([a-z-]+)";', kept)
          if not opens:
              wrong.append("the module no longer says which command its marks open")
          elif f'id: "{opens.group(1)}"' not in leader:
              wrong.append(f"the marks open `{opens.group(1)}`, which the shell's registry does not have")

          if "fn a_bar_and_its_mark_wear_one_colour" not in view:
              wrong.append("the view's test that hands the module and the view one reading and "
                           "compares the colours went away")

          # The agents, by the crate's own `Kind::id` — narrowed to `impl
          # Kind` first: `Why::id` comes before it in the file.
          kinds = must(scope(llm, r"impl Kind\b"), "ronoctua_llm: `impl Kind`")
          kind_id = must(body_of(kinds, "id"), "ronoctua_llm: `Kind::id`")
          agents = re.findall(r'=> "([a-z]+)"', kind_id)
          floor(len(agents), 2, "agents the crate knows")
          marks = ${builtins.toJSON (builtins.attrNames module.marks)}
          for agent in agents:
              if agent not in marks:
                  wrong.append(f"the agent `{agent}` has no mark: its accounts would draw nothing")

          if wrong:
              for w in wrong:
                  print("ERROR: " + w, file=sys.stderr)
              sys.exit(1)
          print(f"OK: the module names no path but its lines', its marks open `{opens.group(1)}`, "
                f"the colours' test is there, and {len(agents)} agents have a mark")
          END
        '';
  };
  slow = [ ];
}
