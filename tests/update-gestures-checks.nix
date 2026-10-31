# THE UPDATE PAGE'S GESTURES ON THE MACHINE (owner, 2026-10-31): hold the
# machine on the distro revision it runs, let go, and try again the system
# whose boot trial failed — each a click on the update's records, asked in the
# sudo bar.
#
#   - `machine-gestures-pass-the-sudo-door`: the three units the gestures run
#     (`ronoctua-machine-hold`, `-unhold`, `-try-again`) are what the system
#     declares under that prefix and no other; each runs its ONE verb of the
#     machine's package with no other word, with the machine's table, its
#     verdict line in the file of root's the helper reads (the update's folder
#     under `/run`, made by the unit), and no start limit; no polkit rule names
#     any of them; the helper's three verbs take no word and each starts its
#     own unit, waited for, the verdict file read where the units write it;
#     and the shell reaches them only through the sudo door's gesture — no
#     file of the shell names a unit of theirs or starts one.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  self,
  mkCheck,
  pyPrelude,
  shellPrelude,
}:
let
  vm = self.nixosConfigurations.vm.config;
  plain = v: builtins.unsafeDiscardStringContext (toString v);
  gestures = lib.filterAttrs (n: _: lib.hasPrefix "ronoctua-machine-" n) vm.systemd.services;
  facts = pkgs.writeText "machine-gestures-facts.json" (
    builtins.toJSON {
      rules = plain vm.security.polkit.extraConfig;
      runDir = vm.ronoctua.machine.runDir;
      machinePackage = plain vm.ronoctua.machine.package;
      table = lib.mapAttrs (_: plain) vm.ronoctua.machine.environment;
      units = lib.mapAttrs (_: s: {
        run = plain s.serviceConfig.ExecStart;
        type = s.serviceConfig.Type or null;
        out = s.serviceConfig.StandardOutput or null;
        err = s.serviceConfig.StandardError or null;
        dir = s.serviceConfig.RuntimeDirectory or null;
        env = lib.mapAttrs (_: plain) s.environment;
        startLimit = s.unitConfig.StartLimitIntervalSec or null;
        restarts = s.restartIfChanged;
        wanted = s.wantedBy;
      }) gestures;
    }
  );
in
{
  machine-gestures-pass-the-sudo-door =
    mkCheck "machine-gestures-pass-the-sudo-door" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())
        exec(open("${shellPrelude}").read())

        # PARSED: the evaluated system arrives as JSON — the units under the
        # prefix, the machine's table and package, the merged polkit rules.
        # FILE-WIDE: the helper's constants and the verbs' grammar are read from
        # files whole (strings kept, `uncommented`); each arm is narrowed to its
        # own body (`scope`), and the shell is swept file by file, its tests out.
        got = json.load(open("${facts}"))
        helper = uncommented(pathlib.Path("${../crates/ronoctua-security/src/bin/root.rs}").read_text())
        verbs = outside_tests(uncommented(pathlib.Path("${../crates/ronoctua-security/src/verb.rs}").read_text()))
        machine = outside_tests(uncommented(pathlib.Path("${../crates/ronoctua-machine/src/lib.rs}").read_text()))
        wrong = []

        # 1. THE UNITS: three, each its verb of the machine's and nothing more.
        wanted = {"hold": "MachineHold", "unhold": "MachineUnhold", "try-again": "MachineTryAgain"}
        declared = sorted(got["units"])
        if declared != sorted(f"ronoctua-machine-{v}" for v in wanted):
            wrong.append(f"the system declares {declared} under ronoctua-machine-; the gestures are hold, unhold and try-again")
        run_dir = got["runDir"]
        for name, u in sorted(got["units"].items()):
            verb = name.removeprefix("ronoctua-machine-")
            if u["run"] != f"{got['machinePackage']}/bin/ronoctua-machine {verb}":
                wrong.append(f"{name} runs {u['run']!r}; it may run <the machine's package>/bin/ronoctua-machine {verb}, no other word")
            # THE VERB TAKES NO WORD on the machine's side either: the CLI reads it
            # alone, never as a verb that names a revision.
            if not re.search(r'\[\s*"' + re.escape(verb) + r'"\s*\]\s*=>', machine):
                wrong.append(f"ronoctua-machine does not read `{verb}` alone")
            if u["type"] != "oneshot":
                wrong.append(f"{name} is {u['type']!r}: the helper waits for it, so it is a oneshot")
            if u["out"] != f"truncate:{run_dir}/{name}.service.said":
                wrong.append(f"{name} writes its verdict line to {u['out']!r}, not {run_dir}/{name}.service.said")
            if u["err"] != "journal":
                wrong.append(f"{name}'s steps go to {u['err']!r}: they belong in the journal, not in the verdict")
            if u["dir"] != run_dir.rsplit("/", 1)[-1]:
                wrong.append(f"{name} does not make the update's folder its verdict goes to ({u['dir']!r})")
            for k, v in got["table"].items():
                if u["env"].get(k) != v:
                    wrong.append(f"{name} is not told {k} as the machine's table says")
            if u["startLimit"] != "0":
                wrong.append(f"{name}'s StartLimitIntervalSec is {u['startLimit']!r}: only a gesture starts it, and a limit refuses the sixth")
            if u["restarts"]:
                wrong.append(f"a switch may restart {name}")
            if u["wanted"]:
                wrong.append(f"{name} is wanted by {u['wanted']}: only the helper starts it")

        # 2. NO POLKIT RULE NAMES THEM: the gesture passes the password.
        floor(len(got["rules"]), 500, "characters of the merged polkit rules")
        for name in declared:
            if name in got["rules"]:
                wrong.append(f"a polkit rule names {name}: root with no password")

        # 3. THE HELPER: each verb's words are the verb alone, and its arm starts
        # its own unit through `machine_gesture`, which reads the verdict where
        # the units write it.
        said_dir = re.search(r'const MACHINE_SAID_DIR: &str = "([^"]+)";', helper)
        if not said_dir or said_dir.group(1) != run_dir:
            wrong.append(f"the helper reads verdicts in {said_dir and said_dir.group(1)!r}, the units write them in {run_dir}")
        for verb, variant in wanted.items():
            word = f"machine-{verb}"
            if not re.search(r'\[\s*"' + re.escape(word) + r'"\s*\]\s*=>\s*Self::' + variant + r"\b", verbs):
                wrong.append(f"the door's `{word}` is not read alone as RootVerb::{variant}")
            if not re.search(r"Self::" + variant + r'\s*=>\s*vec!\[\s*"' + re.escape(word) + r'"\s*\]', verbs):
                wrong.append(f"RootVerb::{variant} is not said as `{word}` alone")
            arm = scope(helper, r"RootVerb::" + variant + r"\s*=>")
            arm = must(arm, f"the helper's `{variant}` arm")
            const = re.search(r"machine_gesture\(\s*tools::given\(tools\.systemctl\.as_ref\(\)\)\?\s*,\s*(MACHINE_\w+_UNIT)\s*,?\s*\)", arm)
            if not const:
                wrong.append(f"the helper's `{variant}` arm does not start its unit through `machine_gesture`")
                continue
            unit = re.search(r"const " + const.group(1) + r': &str = "([^"]+)";', helper)
            if not unit or unit.group(1) != f"ronoctua-machine-{verb}.service":
                wrong.append(f"the helper's `{variant}` starts {unit and unit.group(1)!r}, not ronoctua-machine-{verb}.service")
        gesture = must(body_of(helper, "machine_gesture"), "the helper's `machine_gesture`")
        started = re.findall(r'run_program\(\s*systemctl\s*,\s*&\[([^\]]*)\]\s*\)', gesture)
        if started != ['"start", unit']:
            wrong.append(f"`machine_gesture` runs systemctl with {started}; it may `start` its unit, waited for, and nothing else")
        if "remove_file(&said)" not in gesture.replace(" ", "") or gesture.find("remove_file") > gesture.find("run_program"):
            wrong.append("`machine_gesture` does not take an old verdict away before it starts the unit")

        # 4. THE SHELL REACHES THEM THROUGH THE DOOR ALONE: the gesture runs the
        # helper's verb, and no file of the shell names a unit of theirs.
        door = code_of(without_tests(shell_file("machine_gesture.rs").read_text()))
        if not re.search(r"\bgesture\.run\(\s*what\.verb\(\)\.into\(\)", door):
            wrong.append("machine_gesture.rs does not run the helper's verb through the sudo bar's gesture")
        swept = 0
        for f in shell_files():
            swept += 1
            text = outside_tests(uncommented(f.read_text()))
            for name in declared:
                if name in text:
                    wrong.append(f"{shell_name(f)} names {name}: the shell starts it only through the sudo door")
        floor(swept, 100, "shell files swept")

        if wrong:
            print("ERROR: the update page's gestures on the machine are wider than the sudo door.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {declared} run their one verb of the machine's with its table and no word, waited for by the helper's wordless verbs, their verdict in {run_dir}; no polkit rule names them, and the shell reaches them through the door alone ({swept} files)")
        END
      '';
}
