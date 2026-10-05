# The system update's own checks, in the shape every module's `checks.nix`
# has: the arguments it asks for by name (`nix/brain-modules.nix`,
# `checkArguments`), and its checks, each named after its id.
{
  mkCheck,
  pkgs,
  lib,
  self,
  pyPrelude,
  texts,
  languages,
  module,
  ...
}:

let
  id = module.id;
  folder = "crates/ronoctua-brain/modules/${id}";
  sequence = pkgs.callPackage ./sequence.nix { };
  steps = (import ./steps.nix).steps;
  vm = self.nixosConfigurations.vm.config;
  noctua = self.nixosConfigurations.noctua.config;
  service = unit: lib.removeSuffix ".service" unit;
  units = lib.mapAttrs (_: u: u.name) module.units;

  # THE TWO RUNNERS, for the checks that RUN one of their functions instead of
  # reading the generator for words.
  #
  # The wiring is placeholders on purpose, and that is not a shortcut: what is
  # exercised reads the machine's `/etc` table, the person's own file and the
  # tools handed in, and nothing else. A store path for `nmcli` or a real unit
  # name would be a dependency these checks have no use for.
  #
  # `programs` IS real, because the generator refuses a step whose package it
  # was not given — the two-way check that keeps `steps.nix` and `module.nix`
  # in step.
  runnersWith =
    wiring:
    sequence.mkScripts {
      programs = {
        nix = pkgs.nix;
        systemd = pkgs.systemd;
        fontconfig = pkgs.fontconfig;
        tealdeer = pkgs.tealdeer;
        flatpak = pkgs.flatpak;
        # A STAND-IN, and on purpose: the real one is the software CLI
        # extracted from the workspace, and naming it here would make every
        # check that reads these scripts wait for the whole Rust build. The
        # generator only needs a store path with a `bin/ronoctua-software`.
        software = pkgs.writeShellScriptBin "ronoctua-software" ''
          echo "stand-in for the software CLI: this runner is a fixture" >&2
          exit 1
        '';
      };
      wiring = {
        shell = pkgs.runtimeShell;
        systemctl = "${pkgs.systemd}/bin/systemctl";
        journalctl = "${pkgs.systemd}/bin/journalctl";
        nmcli = "/nonexistent/nmcli";
        flock = "${pkgs.util-linux}/bin/flock";
        state-dir = "/nonexistent/state";
        config = "/nonexistent/updates.conf";
        system-unit = units.system;
        system-unit-rehearsal = units.system-rehearsal;
      }
      // wiring;
    };
  runners = runnersWith { };

  # THE STEPS BY WHAT THEY DO, never by id: an id written here would be the
  # second list `…-has-one-list` refuses.
  stepThat =
    what: pred:
    let
      found = builtins.filter pred steps;
    in
    if builtins.length found == 1 then
      builtins.head found
    else
      throw "${id}: ${toString (builtins.length found)} steps ${what}, and there has to be one";
  store = vm.ronoctua.store;
  trimStep = stepThat "start ${store.generationsUnit}" (s: (s.unit or null) == store.generationsUnit);
  historyStep = stepThat "tidy the person's Nix history" (
    s:
    (s.package or null) == "software"
    &&
      (s.run or [ ]) == [
        "tidy"
        "nix"
      ]
  );
  functionOf = step: "step_" + builtins.replaceStrings [ "-" ] [ "_" ] step.id;

  # THE SCANNER OF FLAGS THAT DELETE GENERATIONS, the one
  # `nix-gc-keeps-generations` uses on the rest of the tree, applied here to
  # the runners this module generates.
  flagScanner = pkgs.writeText "generation-flags.py" ''
    import re
    SHORT_D = r"(?:^|[\s\"'\[,(])-[a-zA-Z]*d[a-zA-Z]*(?=$|[\s\"'\]),;])"
    FORBIDDEN = [
        (r"--delete-older-than", "deletes every profile's generations older than an age"),
        (r"--delete-old\b", "deletes every old generation"),
        (r"nix-collect-garbage[^\n;&|]*" + SHORT_D, "`-d` deletes every old generation of every profile"),
        (r"wipe-history", "wipes a profile's history"),
        (r"--delete-generations", "deletes generations by hand"),
        (r"\brm\b[^\n;&|]*(?:profiles/|-link\b)", "removes a generation's link by hand"),
    ]
    def offences(text):
        found = [why for pattern, why in FORBIDDEN if re.search(pattern, text, re.M)]
        if "nix-collect-garbage" in text and re.search(r"[\"']-[a-zA-Z]*d[a-zA-Z]*[\"']", text):
            found.append("carries a quoted `-d` beside nix-collect-garbage")
        return found
  '';
in
{
  checks = {
    # THERE IS ONE LIST OF UPDATE COMMANDS, AND ONLY ONE.
    #
    # The owner asked for this in as many words: there must be no way to add
    # an update step to the brain and not to the manual updater, or the other
    # way round. It is a property, and this is what makes it one.
    #
    # `steps.nix` declares the sequence. From it `gen-updates.py` makes the
    # Rust table, the text keys and the two shell runners; `module.nix` wraps
    # those in units; the brain reads the same table through `tables.nix`; and
    # `LEADER s u` starts the same units. Nothing along that path names a step.
    #
    # THE DEFECT IS NOT HYPOTHETICAL. The brain used to declare three steps of
    # its own while the table declared twelve, DISJOINT from them: every step
    # name on screen was one the sequence does not use. So: a step's id may
    # appear in the declaration and in the two files that generate from it,
    # and nowhere in any source we write.
    system-update-has-one-list = mkCheck "system-update-has-one-list" [ pkgs.python3 ] ''
      python3 - <<'END'
      import pathlib, re, sys
      exec(open("${pyPrelude}").read())

      root_dir = pathlib.Path("${self}")
      # FILE-WIDE: an id written by hand ANYWHERE in the tree is the
      # offence, which is the whole of what "one list" means. The three
      # declared places are the exception.
      table = uncommented(pathlib.Path("${sequence.table}/updates.rs").read_text())
      floor(len(table), 500, "characters of the generated table")
      ids = sorted(set(re.findall(r'^\s+id: "([a-z0-9-]+)",$', table, re.M)))
      floor(len(ids), 5, "ids in the generated table")

      ALLOWED = {
          "${folder}/steps.nix",
          "${folder}/gen-updates.py",
          "${folder}/sequence.nix",
      }
      for allowed in ALLOWED:
          if not (root_dir / allowed).is_file():
              sys.exit(f"ERROR: {allowed} is allowed to name a step and is not there")

      gaps = []
      swept = 0
      for source in sorted(root_dir.rglob("*")):
          if source.is_dir() or source.suffix not in {".rs", ".nix", ".py"}:
              continue
          rel = source.relative_to(root_dir)
          if str(rel) in ALLOWED or "target" in rel.parts or rel.parts[0] == ".git":
              continue
          try:
              text = source.read_text()
          except (UnicodeDecodeError, OSError):
              continue
          # BOTH COMMENT SHAPES, and not only at the start of a line.
          swept += 1
          for n, line in enumerate(hashless(uncommented(text)).splitlines(), 1):
              for i in ids:
                  if f'"{i}"' in line:
                      gaps.append(f"{rel}:{n}: {i}")

      floor(swept, 100, "files of the tree swept for a step's id")
      if gaps:
          print("ERROR: a step of the update is named outside the declaration.", file=sys.stderr)
          print("", file=sys.stderr)
          for g in gaps:
              print(f"       {g}", file=sys.stderr)
          print("", file=sys.stderr)
          print("       The sequence lives in ${folder}/steps.nix and nowhere else.", file=sys.stderr)
          print("       Whoever needs it READS the generated table; whoever needs to RUN it", file=sys.stderr)
          print("       starts the units. An id written by hand is a second list.", file=sys.stderr)
          raise SystemExit(1)
      print(f"OK: the {len(ids)} steps exist only in the declaration and in what comes out of it")
      END
    '';

    # THE UNITS AND THE PATHS AGREE, and the window names no unit by hand.
    #
    # The four units are written once, in the manifest: `module.nix` reads
    # them from there into the units and the polkit rule, and the Rust reads
    # them from the table generated out of it (`unit::*`). What is still typed
    # twice is where the two halves leave their reports: the root half under
    # `module.nix`'s `stateDir`, the person's half under the runtime directory
    # the runner makes — and the window reads both. So this reads the machine
    # as it is evaluated, and the window's two constants against the module
    # and the runner the generator wrote.
    system-update-names-agree =
      let
        facts = pkgs.writeText "${id}-names.json" (
          builtins.toJSON {
            inherit units;
            system = builtins.attrNames vm.systemd.services;
            user = builtins.attrNames vm.systemd.user.services;
            rule = vm.security.polkit.extraConfig;
          }
        );
      in
      mkCheck "system-update-names-agree" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the evaluated machine arrives as JSON.
        # FILE-WIDE: two module-level constants of the window, a binding of
        # `module.nix` and two lines of the generated runner; none has a
        # function to narrow to. `uncommented` for the Rust and `hashless` for
        # the rest: the needles are the string literals.
        facts = json.load(open("${facts}"))
        root = pathlib.Path("${self}")
        window = uncommented((root / "crates/ronoctua-shell/src/update_window.rs").read_text())
        nix = hashless((root / "${folder}/module.nix").read_text())
        person = hashless(pathlib.Path("${runners}/bin/ronoctua-update").read_text())
        floor(len(window), 2000, "characters of update_window.rs")
        floor(len(nix), 500, "characters of module.nix")
        floor(len(person), 1000, "characters of the person's half")
        wrong = []

        u = facts["units"]
        for word in ("system", "system-rehearsal"):
            if u[word].removesuffix(".service") not in facts["system"]:
                wrong.append(f"the manifest's `{word}` unit, {u[word]}, is not a system unit of the machine")
            if u[word] not in facts["rule"]:
                wrong.append(f"the polkit rule does not name the manifest's `{word}` unit, {u[word]}")
        for word in ("run", "rehearse"):
            if u[word].removesuffix(".service") not in facts["user"]:
                wrong.append(f"the manifest's `{word}` unit, {u[word]}, is not a user unit of the machine")

        # THE WINDOW NAMES NO UNIT: it reads the manifest's through `unit::`.
        for word, name in u.items():
            if f'"{name}"' in window:
                wrong.append(f"update_window.rs spells {name} by hand instead of `unit::`")
        if "unit::SYSTEM" not in window or "unit::RUN" not in window:
            wrong.append("update_window.rs no longer follows the manifest's units")

        # THE ROOT HALF'S REPORT, in the directory the module makes.
        said = re.search(r'const SYSTEM_REPORT: &str = "([^"]+)";', window)
        state = re.search(r'stateDir = "([^"]+)";', nix)
        if not said or not state:
            wrong.append("`SYSTEM_REPORT` or `stateDir` went away — did the pattern change?")
        elif said.group(1).rsplit("/", 1)[0] != state.group(1):
            wrong.append(f"the window reads {said.group(1)}, and the module writes into {state.group(1)}")

        # THE PERSON'S, under the runtime directory the runner makes.
        said = re.search(r'const PERSON_REPORT: &str = "([^"]+)";', window)
        made = re.search(r'^runtime="\$XDG_RUNTIME_DIR/([^"]+)"$', person, re.M)
        if not said or not made:
            wrong.append("`PERSON_REPORT` or the runner's `runtime` went away")
        else:
            leaf = said.group(1)
            if leaf != made.group(1) + "/user.report" or 'report="$runtime"/user.report' not in person:
                wrong.append(f"the window reads $XDG_RUNTIME_DIR/{leaf}, and the runner writes elsewhere")

        if wrong:
            print("ERROR: a name of the update does not agree with itself.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print("OK: the four units are the manifest's, and the window reads where both halves write")
        END
      '';

    # THE SEQUENCE OWES ITS WORDS, IN BOTH LANGUAGES.
    #
    # The generator emits the exact list of keys the table implies — a name
    # and a sentence for every step, and the closed vocabularies for the
    # preconditions, the failure policies and the kinds. A step added without
    # them would draw a bracketed key on the brain's page (invariant 8). The
    # list is DERIVED, so it cannot go stale the way a hand-written one does.
    system-update-has-text = mkCheck "system-update-has-text" [ pkgs.python3 ] ''
      python3 - <<'END'
      import pathlib, re, sys
      exec(open("${pyPrelude}").read())

      owed = [
          line.strip()
          for line in pathlib.Path("${sequence.table}/keys.txt").read_text().splitlines()
          if line.strip()
      ]
      root_dir = pathlib.Path("${texts}")
      failed = 0
      for language in ${builtins.toJSON languages}:
          # FILE-WIDE: a generated list of keys against the merged catalogues,
          # and `hashless` is what makes a key commented out count as missing.
          text = hashless((root_dir / f"{language}.ftl").read_text())
          have = set(re.findall(r"^([a-z][a-z0-9-]*)\s*=", text, re.M))
          for key in owed:
              if key not in have:
                  print(f"ERROR: {language} does not have `{key}`", file=sys.stderr)
                  failed = 1
      # AND EVERY ONE IS THE MODULE'S: a key outside its namespace would be a
      # word the sealed module does not own.
      for key in owed:
          if not key.startswith("${id}-"):
              print(f"ERROR: `{key}` is outside the module's namespace", file=sys.stderr)
              failed = 1
      if failed:
          raise SystemExit(1)
      floor(len(owed), 5, "keys the sequence requires")
      print(f"OK: the sequence's {len(owed)} keys exist in both languages")
      END
    '';

    # AND THE GUARD REALLY DOES REFUSE.
    #
    # `sequence.nix` builds a table that must NOT pass — a step that installs,
    # and a step enabled by default whose program the flake does not put on
    # the machine — and records what the generator said about it. Both
    # sentences are demanded, not just a non-zero exit: a refusal for the WRONG
    # reason is a refusal that will not survive the next edit.
    system-update-refusal-refuses = mkCheck "system-update-refusal-refuses" [ pkgs.gnugrep ] ''
      said=${sequence.negativeFixture}/out_text.txt
      [ -f "$said" ] || {
        echo "ERROR: the negative fixture did not record what the generator said (out_text.txt)." >&2
        exit 1; }
      failed=0
      grep -q "invariant 6" "$said" || {
        echo "ERROR: the refusal does not cite invariant 6." >&2
        failed=1; }
      grep -qi "install" "$said" || {
        echo "ERROR: the refusal does not say the step INSTALLS." >&2
        failed=1; }
      [ "$failed" = 0 ] || { echo "--- what it said ---" >&2; cat "$said" >&2; exit 1; }
      echo "OK: installing and switching on what the flake does not put there are refused, and for the right reason"
    '';

    # THE SEQUENCE NEVER ASKS FOR AUTHENTICATION, and that is a requirement.
    #
    # The owner's words (translated): sometimes the fingerprint reader asks
    # him to approve something, and he does not see it, or he is far from the
    # laptop. The brain runs the update at a good moment, which is almost
    # always one where nobody is looking — and there a prompt is a silent
    # failure shaped like a refusal. So:
    #
    #   1. neither runner invokes `sudo`, `pkexec`, `pkttyagent` or
    #      `systemd-run` (which goes through polkit and is synchronous too);
    #   2. the rule that names the module's two root units returns `YES`, and
    #      neither `AUTH_SELF_KEEP` nor `AUTH_ADMIN`.
    system-update-never-asks =
      let
        machineRunners = noctua.ronoctua.update.package;
        rule = noctua.security.polkit.extraConfig;
      in
      mkCheck "system-update-never-asks" [ pkgs.gnugrep pkgs.python3 ] ''
        failed=0
        for script in ${machineRunners}/bin/ronoctua-update ${machineRunners}/libexec/ronoctua-update-system; do
          if found=$(grep -nE '(^|[^-[:alnum:]_])(sudo|pkexec|pkttyagent|systemd-run)([^-[:alnum:]_]|$)' "$script"); then
            echo "ERROR: $script asks for authentication:" >&2
            echo "$found" >&2
            failed=1
          fi
        done

        python3 - <<'END'
        import re, sys
        exec(open("${pyPrelude}").read())
        # PARSED: the merged polkit rules of the evaluated machine, one
        # JavaScript text; the block is narrowed by the unit it names.
        rule = """${rule}"""
        floor(len(rule), 100, "characters of the merged polkit rules")
        block = re.search(re.escape("${units.system}") + r".*?\}\s*\)\s*;", rule, re.S)
        if not block:
            sys.exit("ERROR: the update's polkit rule went away.")
        text = block.group(0)
        if "polkit.Result.YES" not in text:
            sys.exit("ERROR: the update's rule does not give back YES.")
        for asking in ("AUTH_SELF", "AUTH_ADMIN"):
            if asking in text:
                sys.exit(f"ERROR: the update's rule gives back {asking} — it ASKS.")
        END

        [ "$failed" = 0 ] || exit 1
        echo "OK: the sequence asks nothing of anybody — no sudo, no interactive polkit"
      '';

    # THE SWITCH ON THE UPDATE'S PAGE REACHES THE RUN.
    #
    # It did not: `m t` wrote an entry into `brain.json` and nothing on the
    # machine read it. The fix is a third layer in `turned_on`, read after the
    # machine's table: the person's own flips, which this module writes out of
    # `brain.json` (`STEPS_FILE`, in `module.rs`). This RUNS the generated
    # function, with tables planted under it — a check that grepped the
    # script's source once let a sabotage walk past it.
    system-update-switch-reaches-the-run =
      let
        personal = builtins.filter (s: !s.root) steps;
        onStep =
          (lib.findFirst (s: s.enable) (throw "${id}: no step of the person's ships on") personal).id;
        offStep =
          (lib.findFirst (s: !s.enable) (throw "${id}: no step of the person's ships off") personal).id;
      in
      mkCheck "system-update-switch-reaches-the-run"
        [
          pkgs.bash
          pkgs.gnused
          pkgs.gnugrep
          pkgs.python3
        ]
        ''
          export HOME="$PWD/home"
          export XDG_STATE_HOME="$HOME/.local/state"
          mkdir -p "$XDG_STATE_HOME/ronoctua"

          # THE FILE THE MODULE WRITES IS THE FILE THE SCRIPT READS: the leaf
          # comes from the module's Rust, not from here.
          leaf=$(python3 - <<'END'
          import pathlib, re, sys
          exec(open("${pyPrelude}").read())
          # FILE-WIDE: one module-level constant of `module.rs`; the needle is
          # its string literal, so `uncommented` keeps the strings.
          code = uncommented(pathlib.Path("${self}/${folder}/module.rs").read_text())
          floor(len(code), 1000, "characters of module.rs")
          m = re.search(r'pub const STEPS_FILE: &str = "([^"]+)";', code)
          if not m:
              sys.exit("ERROR: `STEPS_FILE` went away from module.rs")
          print(m.group(1))
          END
          )
          [ -n "$leaf" ] || exit 1
          file="$XDG_STATE_HOME/ronoctua/$leaf"

          script=${runners}/bin/ronoctua-update
          [ -x "$script" ] || { echo "ERROR: the person's half was not built" >&2; exit 1; }
          grep -q "/ronoctua/$leaf" "$script" || {
            echo "ERROR: the person's half does not read ronoctua/$leaf, which the module writes" >&2; exit 1; }
          {
            echo 'set -uo pipefail'
            sed -n '/^turned_on() {/,/^}/p' "$script"
            sed -n '/^default_on() {/,/^}/p' "$script"
          } > lifted.sh
          grep -q 'turned_on' lifted.sh || {
            echo "ERROR: \`turned_on\` is not in the generated script any more" >&2; exit 1; }

          ask() {
            if bash -c ". ./lifted.sh; turned_on $1"; then said=on; else said=off; fi
            if [ "$said" != "$2" ]; then
              echo "ERROR: $1 is $said and should be $2 ($3)" >&2
              failed=1
            fi
          }

          failed=0
          : > "$file"
          ask ${onStep} on "the distro ships it on and nobody has said otherwise"
          ask ${offStep} off "the distro ships it off and nobody has said otherwise"

          # AND NOW THE PERSON SAYS OTHERWISE, in both directions — the line
          # the module writes, `<its id> <step>`.
          printf '%s\n' '${id} ${offStep}' > "$file"
          ask ${offStep} on "the person switched it on"
          ask ${onStep} on "a flip on one step moved another"
          printf '%s\n' '${id} ${onStep}' > "$file"
          ask ${onStep} off "the person switched it off"

          # AND A FLIP BELONGING TO ANOTHER TASK IS NOT THIS SEQUENCE'S.
          printf '%s\n' 'made-up ${onStep}' > "$file"
          ask ${onStep} on "another task's flip was applied to this one"

          [ "$failed" = 0 ] || exit 1
          echo "OK: a step switched on the brain's page is a step the sequence runs"
        '';

    # THE BRAIN READS WHERE THE SCRIPT WRITES, rehearsal included.
    #
    # The runner watches `<REPORTS_LEAF>/<id>.json` under the runtime directory
    # — `<id>-rehearsal.json` for a rehearsal — and the update's script wrote a
    # TSV somewhere else: every run the brain started came back `runaway` two
    # hours later, on a rebuild that had worked. So this reads the runner's
    # constant and the script the generator wrote, and the units the machine
    # gets: the run executes, the rehearsal does not.
    system-update-writes-where-the-brain-reads =
      let
        start =
          unit:
          builtins.unsafeDiscardStringContext
            vm.systemd.user.services.${service unit}.serviceConfig.ExecStart;
        facts = pkgs.writeText "${id}-units.json" (
          builtins.toJSON {
            run = start units.run;
            rehearse = start units.rehearse;
          }
        );
      in
      mkCheck "system-update-writes-where-the-brain-reads" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the two units' command lines, evaluated.
        # FILE-WIDE: from the runner, one constant and one function's body
        # (`body_of`); from the generated script, module-level lines. The
        # needles are strings, so `uncommented`/`hashless` keep them.
        facts = json.load(open("${facts}"))
        runner = uncommented(pathlib.Path("${self}/crates/ronoctua-brain/src/runner.rs").read_text())
        person = hashless(pathlib.Path("${runners}/bin/ronoctua-update").read_text())
        floor(len(runner), 1000, "characters of runner.rs")
        floor(len(person), 1000, "characters of the person's half")
        wrong = []

        leaf = re.search(r'pub const REPORTS_LEAF: &str = "([^"]+)";', runner)
        if not leaf:
            sys.exit("ERROR: `REPORTS_LEAF` went away; it is the name the script has to agree with")
        leaf = leaf.group(1)
        built = must(body_of(runner, "report_in"), "the runner's `report_in`")
        for shape in ('"{task}.json"', '"{task}-rehearsal.json"'):
            if shape not in built:
                wrong.append(f"the runner no longer names a report {shape}")

        if f'brain="$XDG_RUNTIME_DIR/{leaf}"' not in person:
            wrong.append(f"the script does not write under $XDG_RUNTIME_DIR/{leaf}")
        for name in ("${id}.json", "${id}-rehearsal.json"):
            if f'sheet_file="$brain/{name}"' not in person:
                wrong.append(f"the script does not write {name}")
        for needed in ("sheet_ended", "finish()"):
            if needed not in person:
                wrong.append("the script no longer ends the sheet the brain reads")

        # THE RUN ACTS AND THE REHEARSAL DOES NOT — the script picks the
        # sheet's name by `--execute`, so a rehearsal unit that acted would
        # write the real run's sheet.
        if not facts["run"].split()[-1:] == ["--execute"]:
            wrong.append(f"the run's unit does not execute: {facts['run']}")
        if "--execute" in facts["rehearse"].split():
            wrong.append(f"the rehearsal's unit executes: {facts['rehearse']}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: the script writes %t/{leaf}/${id}.json and its rehearsal, where the brain watches")
        END
      '';

    # THE UPDATE KEEPS GENERATIONS: it trims the system's before it collects,
    # the person's history ships off, and neither runner carries a flag that
    # deletes generations.
    #
    # The update's half of `nix-gc-keeps-generations`, moved here with the
    # sequence: the table the machine writes (`/etc/ronoctua/updates.conf`),
    # the order of the root half, and the flags. The steps are found by what
    # they DO, never by id.
    system-update-trims-before-it-collects =
      let
        facts = pkgs.writeText "${id}-trim.json" (
          builtins.toJSON {
            unit = store.generationsUnit;
            trimStep = trimStep.id;
            historyStep = historyStep.id;
          }
        );
        updatesConf = pkgs.writeText "updates.conf" vm.environment.etc."ronoctua/updates.conf".text;
      in
      mkCheck "system-update-trims-before-it-collects" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())
        exec(open("${flagScanner}").read())

        # PARSED: the machine's table and the steps' facts, evaluated.
        # FILE-WIDE: the offence is a FLAG anywhere in a runner, which has no
        # functions; `hashless` takes the comments off.
        facts = json.load(open("${facts}"))
        wrong = []

        # THE SCANNER BITES, or nothing below means anything.
        for planted in ("nix-collect-garbage --delete-older-than 30d", "sudo nix-collect-garbage -d",
                        "nix-collect-garbage --verbose -dv", "nix profile wipe-history",
                        'rm "$dir"/system-*-link'):
            if not offences(planted):
                wrong.append(f"the scanner does not flag a planted `{planted}`")
        for innocent in ("nix-collect-garbage", "nix-collect-garbage --max-freed 1"):
            if offences(innocent):
                wrong.append(f"the scanner flags `{innocent}`, which deletes no generation")

        conf = dict(
            line.split()
            for line in hashless(pathlib.Path("${updatesConf}").read_text()).splitlines()
            if line.strip()
        )
        floor(len(conf), 10, "steps in the machine's updates.conf")
        if conf.get(facts["trimStep"]) != "on":
            wrong.append(f"updates.conf does not have `{facts['trimStep']} on`: the system's generations would grow for ever")
        if conf.get(facts["historyStep"]) != "off":
            wrong.append(f"updates.conf does not have `{facts['historyStep']} off`: the person's own history would be wiped by itself")

        root_half = hashless(pathlib.Path("${runners}/libexec/ronoctua-update-system").read_text())
        person_half = hashless(pathlib.Path("${runners}/bin/ronoctua-update").read_text())
        floor(len(root_half), 1000, "characters of the root half")
        floor(len(person_half), 1000, "characters of the person's half")
        trims = [m.start() for m in re.finditer(re.escape(facts["unit"]), root_half)]
        collects = [m.start() for m in re.finditer(r"nix-collect-garbage", root_half)]
        if not trims or not collects:
            wrong.append("the root half no longer starts the trim, or no longer collects")
        elif min(trims) > min(collects):
            wrong.append("the root half collects BEFORE it trims: what the trimmed generations held stays a week longer")
        for name, text in (("the root half", root_half), ("the person's half", person_half)):
            for why in offences(text):
                wrong.append(f"{name} {why}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            raise SystemExit(1)
        print("OK: the update trims the system's generations before it collects, and deletes no one else's")
        END
      '';

    # A REHEARSAL OF THE UPDATE REHEARSES THE TRIM, FOR REAL (the owner's D5).
    #
    # The trim is the one step whose rehearsal has something to say that its
    # unit's name does not: WHICH generations would go. So its step declares
    # `rehearsal`, and a rehearsal starts the store's rehearsal unit in its
    # place — the same program, profile, count and menus, with no `--execute`
    # — and says what that run said. This RUNS the generated step, lifted out
    # of the root half, over a `systemctl` and a `journalctl` that behave as
    # systemd does after a oneshot ended cleanly — the unit is unloaded, and
    # `show` answers with no invocation id — over a journal that already holds
    # an earlier rehearsal's lines and gets a neighbour's line with this one's:
    #
    #   1. rehearsing, it starts the rehearsal unit and never the trim, says
    #      what THIS run of the rehearsal printed — not the earlier run's, not
    #      the neighbour's — having marked the journal's end before the start
    #      and synced it after, and records `rehearsed`;
    #   2. a journal that names no end is said, and is not read whole;
    #   3. a rehearsal that fails fails the step, as the real trim would;
    #   4. executing, it starts the trim and not the rehearsal;
    #
    # and it reads the machine: the rehearsal unit runs the trim's own command
    # without `--execute` and nothing else different. And the generator refuses
    # the two ways the field could lie — a rehearsal that IS the unit (a
    # rehearsal would run the step for real), and a rehearsal on a program step.
    system-update-rehearses-the-trim =
      let
        # THE JOURNAL IS A FILE of `<unit> <message>` lines, and a cursor is
        # how many lines it had.
        systemctl = pkgs.writeShellScript "fixture-systemctl" ''
          printf 'systemctl %s\n' "$*" >> "$FIXTURE_LOG"
          # WHAT SYSTEMD ANSWERS AFTER A ONESHOT ENDED CLEANLY: nothing. The
          # unit was unloaded, and `show` speaks of one loaded afresh.
          if [ "$1" = show ]; then
            exit 0
          fi
          # THE UNIT RUNS: its line, and a neighbour's written meanwhile.
          if [ "$1" = start ]; then
            printf '%s %s\n' "$2" "would remove system generations 3 4 (this rehearsal)" >> "$FIXTURE_JOURNAL"
            printf '%s %s\n' fixture-neighbour.service "a neighbour's line" >> "$FIXTURE_JOURNAL"
          fi
          exit "''${FIXTURE_EXIT:-0}"
        '';
        journalctl = pkgs.writeShellScript "fixture-journalctl" ''
          printf 'journalctl %s\n' "$*" >> "$FIXTURE_LOG"
          mapfile -t lines < "$FIXTURE_JOURNAL"
          after=0 unit=
          for a in "$@"; do
            case "$a" in
              --show-cursor)
                if [ -z "''${FIXTURE_NO_CURSOR:-}" ]; then
                  echo "-- cursor: fixture-''${#lines[@]}"
                fi
                exit 0
                ;;
              --sync) exit 0 ;;
              --after-cursor=fixture-*) after=''${a#--after-cursor=fixture-} ;;
              --after-cursor=*) exit 1 ;;
              _SYSTEMD_UNIT=*) unit=''${a#_SYSTEMD_UNIT=} ;;
            esac
          done
          for line in "''${lines[@]:after}"; do
            if [ -z "$unit" ] || [ "''${line%% *}" = "$unit" ]; then
              echo "''${line#* }"
            fi
          done
        '';
        lifted = runnersWith {
          systemctl = "${systemctl}";
          journalctl = "${journalctl}";
        };
        trimUnit = vm.systemd.services.${service store.generationsUnit}.serviceConfig.ExecStart;
        rehearsalUnit =
          vm.systemd.services.${service store.generationsRehearsalUnit}.serviceConfig.ExecStart or "";
        facts = pkgs.writeText "${id}-rehearsal.json" (
          builtins.toJSON {
            step = trimStep.id;
            function = functionOf trimStep;
            unit = store.generationsUnit;
            rehearsal = store.generationsRehearsalUnit;
            declared = trimStep.rehearsal or null;
            trim = builtins.unsafeDiscardStringContext trimUnit;
            rehearsing = builtins.unsafeDiscardStringContext rehearsalUnit;
            steps = steps;
          }
        );
      in
      mkCheck "system-update-rehearses-the-trim"
        [
          pkgs.bash
          pkgs.gnused
          pkgs.python3
        ]
        ''
          python3 - <<'END'
          import json, os, pathlib, subprocess, sys
          exec(open("${pyPrelude}").read())

          # PARSED: the steps, the units and the machine's command lines arrive
          # as JSON; the generated root half is RUN, not read.
          # FILE-WIDE: the root half is a shell script, and `body_of` walks
          # Rust braces; the step's function and its helpers are cut out of it
          # by their own `name() {` lines (`cut`, below) and run.
          facts = json.load(open("${facts}"))
          wrong = []
          fn = facts["function"]

          if facts["declared"] != facts["rehearsal"]:
              wrong.append(f"the trim's step rehearses `{facts['declared']}`, and the store's rehearsal is `{facts['rehearsal']}`")
          trim, rehearsing = facts["trim"].split(), facts["rehearsing"].split()
          if "--execute" not in trim:
              wrong.append("the trim's own unit does not act — this check reads the wrong unit")
          if "--execute" in rehearsing:
              wrong.append(f"the trim's REHEARSAL acts: {facts['rehearsing']}")
          if [w for w in trim if w != "--execute"] != rehearsing:
              wrong.append("the rehearsal is not the trim's own command without `--execute`")

          script = pathlib.Path("${lifted}/libexec/ronoctua-update-system").read_text()
          floor(len(script), 1000, "characters of the root half")
          def cut(start):
              lines, keep = [], False
              for line in script.splitlines():
                  if line.startswith(start):
                      keep = True
                  if keep:
                      lines.append(line)
                      if line == "}":
                          break
              return "\n".join(lines)
          one_liners = [l for l in script.splitlines() if l.startswith(("say() {", "record() {"))]
          body = cut(fn + "() {")
          must(body, "the trim's step function in the root half")
          lifted = "\n".join(["set -uo pipefail", *one_liners, cut("turned_on() {"), cut("default_on() {"), body])
          pathlib.Path("lifted.sh").write_text(lifted + "\n")

          EARLIER = "would remove system generations 1 2 (an earlier rehearsal)"
          THIS = "would remove system generations 3 4 (this rehearsal)"
          def run(execute, exit_code, cursor=True):
              log, report = pathlib.Path("calls.log"), pathlib.Path("report.tsv")
              journal = pathlib.Path("journal.txt")
              log.write_text("")
              report.write_text("")
              journal.write_text(f"{facts['rehearsal']} {EARLIER}\n")
              env = dict(os.environ, FIXTURE_LOG=str(log.resolve()), FIXTURE_EXIT=str(exit_code),
                         FIXTURE_JOURNAL=str(journal.resolve()))
              if not cursor:
                  env["FIXTURE_NO_CURSOR"] = "1"
              r = subprocess.run(
                  ["bash", "-c", f'execute={execute}; report="{report.resolve()}"; . ./lifted.sh; {fn}'],
                  capture_output=True, text=True, env=env)
              return r.returncode, r.stdout, log.read_text(), report.read_text()

          def first(calls, word):
              at = [i for i, line in enumerate(calls.splitlines()) if word in line]
              return at[0] if at else None

          # 1. REHEARSING.
          code, said, calls, report = run("false", 0)
          if f"systemctl start {facts['rehearsal']}" not in calls:
              wrong.append(f"a rehearsal did not start {facts['rehearsal']}: {calls!r}")
          if f"systemctl start {facts['unit']}" in calls:
              wrong.append("a REHEARSAL started the real trim")
          if THIS not in said:
              wrong.append(f"what this run of the trim's rehearsal said is not in the sequence's output: {said!r} ({calls!r})")
          if EARLIER in said:
              wrong.append("an EARLIER rehearsal's lines were relayed as this one's")
          if "a neighbour's line" in said:
              wrong.append("another unit's lines were relayed as the rehearsal's")
          order = [first(calls, w) for w in ("--show-cursor", f"systemctl start {facts['rehearsal']}", "--sync", "--after-cursor")]
          if None in order or order != sorted(order):
              wrong.append(f"not: the journal's end marked, the rehearsal started, the journal synced, then read after the mark ({order}, {calls!r})")
          if code != 0 or f"{facts['step']}\trehearsed\t" not in report:
              wrong.append(f"a rehearsal that worked is not recorded `rehearsed` ({code}, {report!r})")
          # 2. A JOURNAL THAT NAMES NO END: said, and never read whole.
          code, said, calls, report = run("false", 0, cursor=False)
          if EARLIER in said or "--after-cursor" in calls:
              wrong.append("with no mark, the rehearsal read the journal from its start")
          if not [l for l in said.splitlines() if l.startswith("!! ")]:
              wrong.append(f"with no mark, the rehearsal did not say its lines could not be read back: {said!r}")
          if code != 0 or f"{facts['step']}\trehearsed\t" not in report:
              wrong.append(f"a rehearsal that worked but could not be read back is not `rehearsed` ({code}, {report!r})")
          # 3. A REHEARSAL THAT FAILS.
          code, said, calls, report = run("false", 1)
          if code == 0 or f"{facts['step']}\tfailed\t" not in report:
              wrong.append(f"a trim whose rehearsal failed was recorded as rehearsed ({code}, {report!r})")
          # 4. EXECUTING.
          code, said, calls, report = run("true", 0)
          if f"systemctl start {facts['unit']}" not in calls or facts["rehearsal"] in calls:
              wrong.append(f"a real run did not start the trim alone: {calls!r}")

          # AND THE GENERATOR REFUSES THE FIELD WHERE IT WOULD LIE.
          def generate(steps):
              pathlib.Path("steps.json").write_text(json.dumps({"steps": steps}))
              pathlib.Path("out").mkdir(exist_ok=True)
              return subprocess.run(
                  [sys.executable, "${./gen-updates.py}", "--updates", "steps.json", "--task", "${id}",
                   "--emit", "table", "--out-rust", "out/updates.rs", "--out-keys", "out/keys.txt"],
                  capture_output=True, text=True)
          clean = generate(facts["steps"])
          if clean.returncode != 0:
              wrong.append("the declared steps were refused: " + clean.stderr)
          same = [dict(s, rehearsal=s["unit"]) if s["id"] == facts["step"] else s for s in facts["steps"]]
          r = generate(same)
          if r.returncode == 0 or "for real" not in r.stderr:
              wrong.append(f"a rehearsal that is the unit itself was not refused: {r.stderr!r}")
          program = next(s for s in facts["steps"] if "package" in s)
          other = [dict(s, rehearsal=facts["rehearsal"]) if s is program else s for s in facts["steps"]]
          r = generate(other)
          if r.returncode == 0 or "`unit` only" not in r.stderr:
              wrong.append(f"a rehearsal on a program step was not refused: {r.stderr!r}")

          if wrong:
              print("ERROR: the update's rehearsal does not rehearse the trim.", file=sys.stderr)
              for w in wrong:
                  print("       " + w, file=sys.stderr)
              raise SystemExit(1)
          print("OK: rehearsing the update rehearses the trim for real, says what would go, and acts on nothing")
          END
        '';

    # AND ON A MACHINE: the same rehearsal under the SYSTEM manager, as root,
    # over a real journal — what the check above stands in for.
    #
    # The relay that read the rehearsal's lines by its invocation id relayed
    # nothing on every rehearsal that worked, because the oneshot that ended
    # cleanly is unloaded at once, and the check's fixture answered for it
    # anyway. Only systemd can say how systemd behaves, so here the root half
    # the update's rehearsal unit runs is run — the generated script, its own
    # `systemctl` and `journalctl` — over generations planted in the system
    # profile, and the store's own rehearsal unit (`nix/modules/store.nix`,
    # imported alone):
    #
    #   1. the rehearsal says, once, exactly the planted generations the trim
    #      would remove — every one past the newest it keeps — and the step is
    #      recorded `rehearsed`;
    #   2. none of them is removed, and the trim itself never started;
    #   3. a second rehearsal says its own list once, and not the first one's
    #      too, which the journal still holds.
    system-update-rehearsal-vm =
      let
        keep = 2;
        planted = map (
          n: pkgs.writeTextDir "share/ronoctua-planted-generation-${toString n}" "${toString n}\n"
        ) (lib.range 1 6);
        stateDir = "/run/ronoctua-update-rehearsal-vm";
        rootHalf = "${runnersWith { state-dir = stateDir; }}/libexec/ronoctua-update-system";
      in
      pkgs.testers.runNixOSTest {
        name = "${id}-rehearsal";
        nodes.machine = {
          imports = [ ../../../../nix/modules/store.nix ];
          virtualisation.memorySize = 1024;
          # THE PLANTED GENERATIONS ARE IN THE MACHINE'S STORE, registered:
          # `nix-env --set` takes nothing the store does not know.
          virtualisation.additionalPaths = planted;
          ronoctua.store.systemGenerations = keep;
        };
        testScript = ''
          import json
          import re

          machine.wait_for_unit("multi-user.target")
          planted = json.loads(${builtins.toJSON (builtins.toJSON (map toString planted))})
          for path in planted:
              machine.succeed(f"nix-env --profile /nix/var/nix/profiles/system --set {path}")
          links = machine.succeed("ls /nix/var/nix/profiles").split()
          numbers = sorted(int(m.group(1)) for m in (re.fullmatch(r"system-(\d+)-link", link) for link in links) if m)
          ours = [n for n in numbers if machine.succeed(f"readlink -f /nix/var/nix/profiles/system-{n}-link").strip() in planted]
          assert len(ours) == len(planted), f"planted {len(planted)} generations, found {ours} among {numbers}"
          newest = numbers[-${toString keep}:]
          doomed = [n for n in ours if n not in newest]
          assert doomed, f"nothing would go: {numbers}"

          def rehearsed():
              said = machine.succeed("${rootHalf} 2>&1")
              print(said)
              lists = [line for line in said.splitlines() if line.startswith("would remove system generations")]
              return said, lists

          with subtest("the rehearsal says which planted generations the trim would remove, once"):
              said, lists = rehearsed()
              assert len(lists) == 1, f"the trim's rehearsal was not said once: {lists!r}"
              named = [int(w) for w in re.findall(r"\d+", lists[0].split("(")[0])]
              assert named == doomed, f"it would remove {named}; past the newest ${toString keep} are {doomed}"
              for n in newest:
                  assert f"keeps generation {n}:" in said, f"the newest generation {n} is not said to stay"
              report = machine.succeed("cat ${stateDir}/system.report")
              assert re.search(r"^${trimStep.id}\trehearsed\t", report, re.M), report

          with subtest("and it removes none, and the trim itself never ran"):
              for n in numbers:
                  machine.succeed(f"test -L /nix/var/nix/profiles/system-{n}-link")
              ran = machine.succeed(
                  "systemctl show -p ActiveEnterTimestampMonotonic --value ${store.generationsUnit}"
              ).strip()
              assert ran in ("", "0"), f"the trim itself was started: {ran}"

          with subtest("a second rehearsal says its own list, and not the first one's too"):
              # WHAT SYSTEMD KEPT OF THE FIRST: nothing, when it unloaded the
              # unit — the case the relay by invocation id lost.
              print(machine.execute("systemctl show -p LoadState -p InvocationID ${store.generationsRehearsalUnit}")[1])
              said, lists = rehearsed()
              assert len(lists) == 1, f"one rehearsal said {len(lists)} lists: {lists!r}"
        '';
      };
  };
  slow = [ ];
}
