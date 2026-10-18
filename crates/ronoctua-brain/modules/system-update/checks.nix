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
  # A STAND-IN, and on purpose: the real ones are the software CLI, the
  # machine's tool, the person's and the belongings' — extracted from the
  # workspace, and naming them here would make every check that reads these
  # scripts wait for the whole Rust build. The generator only needs a store
  # path with the program under `bin/`.
  standIn =
    name:
    pkgs.writeShellScriptBin name ''
      echo "stand-in for ${name}: this runner is a fixture" >&2
      exit 1
    '';
  runnersOf =
    {
      programs ? { },
      wiring ? { },
    }:
    sequence.mkScripts {
      programs = {
        nix = pkgs.nix;
        systemd = pkgs.systemd;
        fontconfig = pkgs.fontconfig;
        tealdeer = pkgs.tealdeer;
        flatpak = pkgs.flatpak;
        software = standIn "ronoctua-software";
        machine = standIn "ronoctua-machine";
        person = standIn "ronoctua-person";
        belongings = standIn "ronoctua-belongings";
        binary-cache = standIn "ronoctua-cachix";
      }
      // programs;
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
        # THE 2026-10-05 PRECONDITIONS' WIRING, placeholders like the rest.
        ac-power = "/nonexistent/systemd-ac-power";
        power-supplies = "/nonexistent/power_supply";
        df = "/nonexistent/df";
        collect-garbage = "/nonexistent/nix-collect-garbage";
        jq = "/nonexistent/jq";
        machine-state = "/nonexistent/state.json";
        id = "/nonexistent/id";
        person = vm.ronoctua.user;
        runtime-root = "/nonexistent/run-user";
      }
      // wiring;
    };
  runnersWith = wiring: runnersOf { inherit wiring; };
  runners = runnersWith { };

  # THE STEPS AS THE GENERATOR READS THEM, for the checks that build tables
  # of their own or look for a step by what it does.
  stepsJson = pkgs.writeText "${id}-steps.json" (builtins.toJSON { inherit steps; });
  # The machine directory, by its one name (`machine-dir-has-one-name`).
  machineDir = (import ../../../../nix/machine.nix).machineDir;

  # THE RUNNERS OVER A WORLD OF STAND-INS, for the checks that RUN both halves
  # whole (2026-10-05). Every program a step names is a stand-in that writes
  # what it was asked into `$FIXTURE_LOG`; the person's half starts the root
  # half through a `systemctl` that runs it (`$FIXTURE_ROOT_HALF`); mains, the
  # batteries, the disk and the network answer as the fixture says; and every
  # folder the wiring names is RELATIVE, so a run is rooted where the check
  # stands. The machine directory the steps name stays the machine's: a step
  # that reads it is skipped here, and the check that needs one lifts the
  # step's function and plants a directory of its own.
  standInThatLogs = pkgs.writeShellScript "update-stand-in" ''
    printf '%s %s\n' "''${0##*/}" "$*" >> "$FIXTURE_LOG"
    case "''${0##*/} $*" in
      # THE LOCK MOVES, as `nix flake update` would move it: in the folder
      # after `--flake` — so a check can ask whether it moved.
      "nix flake update "*)
        args="$*"
        printf 'moved\n' >> "''${args##* --flake }/flake.lock"
        ;;
      # A REHEARSAL GOES THROUGH EVERY REFUSAL THE RUN DOES.
      "ronoctua-machine apply" | "ronoctua-machine apply --rehearse")
        if [ -n "''${FIXTURE_DID_NOT_BOOT:-}" ]; then
          echo "a trial of this system did not start" >&2
          echo "refused did-not-boot"
          exit 1
        fi
        if [ -n "''${FIXTURE_APPLY_FAILS:-}" ]; then
          echo "refused build-failed"
          exit 1
        fi
        if [ -n "''${FIXTURE_APPLY_CRASHES:-}" ]; then
          echo "something that is no refusal"
          exit 2
        fi
        ;;
    esac
    exit 0
  '';
  # EVERY PROGRAM A STEP RUNS, by package, read from the table: the
  # generator refuses a package no step names, and a step whose package it
  # was not given.
  programsOf = lib.foldl' (
    acc: s:
    if s ? package then
      acc // { ${s.package} = lib.unique ((acc.${s.package} or [ ]) ++ [ s.program ]); }
    else
      acc
  ) { } steps;
  loggingPrograms = lib.mapAttrs (
    package: names:
    pkgs.runCommand "update-stand-in-${package}" { } ''
      mkdir -p $out/bin
      for name in ${lib.escapeShellArgs names}; do
        ln -s ${standInThatLogs} "$out/bin/$name"
      done
    ''
  ) programsOf;
  fixtureTool = name: text: "${pkgs.writeShellScript "update-fixture-${name}" text}";
  fixtureRunners = runnersOf {
    programs = loggingPrograms;
    wiring = {
      systemctl = fixtureTool "systemctl" ''
        printf 'systemctl %s\n' "$*" >> "$FIXTURE_LOG"
        if [ "$1" = start ]; then
          if [ -e "$XDG_RUNTIME_DIR/ronoctua-update/on-battery" ]; then
            printf 'systemctl saw a yes\n' >> "$FIXTURE_LOG"
          fi
          case "$2" in
            ${units.system})
              [ -z "''${FIXTURE_ROOT_FAILS:-}" ] || exit 1
              exec "$FIXTURE_ROOT_HALF" --execute </dev/null
              ;;
            ${units.system-rehearsal})
              [ -z "''${FIXTURE_ROOT_FAILS:-}" ] || exit 1
              exec "$FIXTURE_ROOT_HALF" </dev/null
              ;;
          esac
        fi
        exit 0
      '';
      journalctl = fixtureTool "journalctl" "exit 0";
      nmcli = fixtureTool "nmcli" ''
        if [ -n "''${FIXTURE_ONLINE:-}" ]; then echo connected; else echo disconnected; fi
      '';
      state-dir = "state";
      config = "updates.conf";
      ac-power = fixtureTool "ac-power" ''exit "''${FIXTURE_AC:-0}"'';
      power-supplies = "supplies";
      # THE DISK: `$FIXTURE_FREE_MIB` free, and `$FIXTURE_GC_FREES` once the
      # store's garbage was collected, when the fixture says a collection
      # frees that much.
      df = fixtureTool "df" ''
        free=''${FIXTURE_FREE_MIB:-50000}
        if [ -n "''${FIXTURE_GC_FREES:-}" ] && [ -e "$FIXTURE_LOG.collected" ]; then
          free=$FIXTURE_GC_FREES
        fi
        printf 'Filesystem 1048576-blocks Used Available Capacity Mounted on\n'
        printf 'fixture 100000 50000 %s 50%% /\n' "$free"
      '';
      collect-garbage = fixtureTool "collect-garbage" ''
        printf 'collect-garbage %s\n' "$*" >> "$FIXTURE_LOG"
        : > "$FIXTURE_LOG.collected"
      '';
      jq = "${pkgs.jq}/bin/jq";
      machine-state = "state.json";
      id = fixtureTool "id" ''
        if [ "$*" = "-u ${vm.ronoctua.user}" ]; then echo 1000; exit 0; fi
        exit 1
      '';
      runtime-root = "runtime";
    };
  };

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
    #
    # And, since 2026-10-05, two more: the leaf of the person's yes to a run
    # on battery — written by the window, taken by the person's half, tested
    # by the root half — and the folder of the machine's record, which the
    # view reads through the machine's crate and the root half's hold test
    # through the module's option.
    system-update-names-agree =
      let
        facts = pkgs.writeText "${id}-names.json" (
          builtins.toJSON {
            inherit units;
            system = builtins.attrNames vm.systemd.services;
            user = builtins.attrNames vm.systemd.user.services;
            rule = vm.security.polkit.extraConfig;
            machineState = vm.ronoctua.machine.stateDir;
            # WHAT THE STEP THAT MOVES THE INPUTS RUNS, which names the
            # distro's input it moves (`steps.nix` reads it from the
            # machine template).
            moves = (stepThat "moves the inputs" (s: s.does == "move-inputs")).run;
          }
        );
      in
      mkCheck "system-update-names-agree" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the evaluated machine arrives as JSON.
        # FILE-WIDE: three module-level constants of the window, one of
        # `ronoctua-machine`, two bindings of `module.nix` and lines of both
        # generated halves; none has a function to narrow to. `uncommented`
        # for the Rust and `hashless` for the rest: the needles are the string
        # literals.
        facts = json.load(open("${facts}"))
        root = pathlib.Path("${self}")
        window = uncommented((root / "crates/ronoctua-shell-update/src/update_window.rs").read_text())
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

        # THE PERSON'S YES TO A RUN ON BATTERY (2026-10-05): the leaf the
        # window writes is the one the person's half takes and the root half
        # tests, under the person's runtime directory.
        said = re.search(r'const ON_BATTERY: &str = "([^"]+)";', window)
        root_half = hashless(pathlib.Path("${runners}/libexec/ronoctua-update-system").read_text())
        floor(len(root_half), 1000, "characters of the root half")
        if not said:
            wrong.append("`ON_BATTERY` went away from update_window.rs")
        else:
            leaf = said.group(1)
            if f'consent="$XDG_RUNTIME_DIR/{leaf}"' not in person:
                wrong.append(f"the window writes $XDG_RUNTIME_DIR/{leaf}, and the person's half takes another yes")
            if f'/"$uid"/{leaf}' not in root_half:
                wrong.append(f"the window writes $XDG_RUNTIME_DIR/{leaf}, and the root half tests another yes")

        # AND THE MACHINE'S RECORD the view reads (`ronoctua_machine::STATE_DIR`)
        # is in the folder the machine's units write it to — and the root
        # half's hold test reads it from the module's own option.
        crate = uncommented((root / "crates/ronoctua-machine/src/lib.rs").read_text())
        floor(len(crate), 2000, "characters of ronoctua-machine's lib.rs")
        state_dir = re.search(r'pub const STATE_DIR: &str = "([^"]+)";', crate)
        if not state_dir:
            wrong.append("`STATE_DIR` went away from ronoctua-machine")
        elif state_dir.group(1) != facts["machineState"]:
            wrong.append(f"the view reads the record in {state_dir.group(1)}, and the machine keeps it in {facts['machineState']}")
        if 'machine-state = "''${config.ronoctua.machine.stateDir}/state.json";' not in nix:
            wrong.append("the root half's hold test does not read the record from ronoctua.machine.stateDir")

        # AND THE DISTRO'S INPUT: the one the update moves (the template's,
        # read by `steps.nix`) is the one `ronoctua-machine` judges a hold by
        # and moves in its rehearsal (`DISTRO_INPUT`). Renamed on one side,
        # a held machine would refuse every apply, or a rehearsal measure an
        # input the run never moves.
        moves = facts["moves"]
        moved = moves[moves.index("update") + 1] if "update" in moves[:-1] else None
        distro = re.search(r'pub const DISTRO_INPUT: &str = "([^"]+)";', crate)
        if not distro or not moved:
            wrong.append("`DISTRO_INPUT` went away from ronoctua-machine, or the step that moves the inputs names none")
        elif distro.group(1) != moved:
            wrong.append(f"the update moves the input `{moved}`, and ronoctua-machine judges holds and rehearses by `{distro.group(1)}`")

        if wrong:
            print("ERROR: a name of the update does not agree with itself.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print("OK: the four units are the manifest's, the window reads where both halves write, and the distro's input is one name")
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
        runner = uncommented(pathlib.Path("${self}/crates/ronoctua-brain-daemon/src/runner.rs").read_text())
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
        # THE STEPS' OWN FUNCTIONS, from the first `step_<id>() {` on: the
        # helpers above them (the room a rebuild wants collects garbage
        # before it refuses) are not steps, and their order is not the
        # sequence's.
        first_step = re.search(r"^step_\w+\(\) \{", root_half, re.M)
        if not first_step:
            sys.exit("ERROR: the root half has no step functions any more")
        steps_part = must(root_half[first_step.start():], "the root half's steps")
        trims = [m.start() for m in re.finditer(re.escape(facts["unit"]), steps_part)]
        collects = [m.start() for m in re.finditer(r"nix-collect-garbage", steps_part)]
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
    # THE 2026-10-05 WAVE: the update's new checks, after its anchor.
    # ── anchor: L19 ──

    # THE ORDER LAWS OF 2026-10-05, EACH BROKEN ONCE, AND REFUSED FOR ITS OWN
    # REASON.
    #
    # The sequence gathers as the person before the root half, migrates and
    # verifies as root around the moving of the inputs, reconciles as the
    # person after the rebuild and publishes last; one battery floor for the
    # whole table; room is root's to ask; a rehearsal of a program changes
    # nothing and goes through every refusal `run` does; a refusal read as
    # news is a word. This builds, from the declared table, one table per
    # law that breaks it — the steps found by what they DO, never by id —
    # runs the generator on each and demands the law's own sentence: a
    # refusal for the wrong reason is one that will not survive the next
    # edit. And the declared table passes.
    system-update-laws-refuse = mkCheck "system-update-laws-refuse" [ pkgs.python3 ] ''
      python3 - <<'END'
      import copy, json, pathlib, subprocess, sys

      GEN = "${./gen-updates.py}"
      STEPS = "${stepsJson}"
      TASK = "${id}"

      declared = json.load(open(STEPS))["steps"]
      wrong = []

      def generate(steps, emit="table"):
          pathlib.Path("laws.json").write_text(json.dumps({"steps": steps}))
          pathlib.Path("out").mkdir(exist_ok=True)
          return subprocess.run(
              [sys.executable, GEN, "--updates", "laws.json", "--task", TASK,
               "--emit", "table", "--out-rust", "out/updates.rs", "--out-keys", "out/keys.txt"],
              capture_output=True, text=True)

      # THE STEPS BY WHAT THEY DO, never by id (`…-has-one-list`).
      def at(pred, what):
          found = [i for i, s in enumerate(declared) if pred(s)]
          if not found:
              sys.exit(f"ERROR: no declared step {what}: this check reads the wrong table")
          return found

      def does(word):
          return at(lambda s: s["does"] == word, f"does `{word}`")

      def moved(steps, frm, to):
          steps = copy.deepcopy(steps)
          s = steps.pop(frm)
          steps.insert(to if to <= frm else to - 1, s)
          return steps

      def changed(i, **fields):
          steps = copy.deepcopy(declared)
          steps[i].update(fields)
          return steps

      gathers, migrates, moves = does("gather"), does("migrate"), does("move-inputs")
      verifies, applies = does("verify"), does("apply-configuration")
      reconciles, publishes = does("reconcile-declared"), does("publish")
      roots = at(lambda s: s["root"], "as root")
      person_after = at(lambda s: not s["root"] and s["does"] not in ("gather", "publish"), "of the person's half")
      powered = at(lambda s: any("power" in n for n in s["needs"]), "that needs power")
      rehearsing = at(lambda s: "rehearse" in s and "package" in s, "with a rehearsal of its own")
      units = at(lambda s: "unit" in s, "that starts a unit")
      floor = next(n["power"] for n in declared[powered[0]]["needs"] if "power" in n)

      def with_need(i, need):
          steps = copy.deepcopy(declared)
          steps[i]["needs"] = steps[i]["needs"] + [need]
          return steps

      def needs_dropped(i, kind):
          steps = copy.deepcopy(declared)
          steps[i]["needs"] = [n for n in steps[i]["needs"] if kind not in n]
          return steps

      def needs_replaced(i, kind, value):
          steps = copy.deepcopy(declared)
          steps[i]["needs"] = [({kind: value} if kind in n else n) for n in steps[i]["needs"]]
          return steps

      # EACH LAW BROKEN ONCE, and the sentence that names it demanded.
      CASES = [
          ("a gathering run as root", changed(gathers[0], root=True), "`gather` runs as the person"),
          ("a gathering after the root half", moved(declared, gathers[-1], roots[0] + 1), "`gather` after the root half"),
          ("a person's step before the root half", moved(declared, person_after[0], 0), "only `gather` comes before it"),
          ("a migration of the person's", changed(migrates[0], root=False), "`migrate` is root's"),
          ("a migration after the inputs moved", moved(declared, migrates[0], moves[0] + 1), "`migrate` after `move-inputs`"),
          ("a verification of the person's", changed(verifies[0], root=False), "`verify` is root's"),
          ("a verification before the inputs moved", moved(declared, verifies[0], moves[0]), "`verify` before `move-inputs`"),
          ("a verification after the rebuild", moved(declared, verifies[0], applies[0] + 1), "`verify` after `apply-configuration`"),
          ("a reconciliation as root", changed(reconciles[0], root=True), "`reconcile-declared` is the person's"),
          ("a reconciliation before the rebuild", moved(declared, reconciles[0], applies[0]), "`reconcile-declared` before `apply-configuration`"),
          ("a publication as root", changed(publishes[0], root=True), "`publish` is the person's"),
          ("a step after the publication", moved(declared, person_after[-1], len(declared)), "a step after `publish`"),
          ("two battery floors", needs_replaced(powered[-1], "power", floor + 1), "one battery floor"),
          ("room asked by a person's step", with_need(person_after[0], {"free-space": {"at": "/nix/store", "percent": 5, "cap-gib": 20}}), "`free-space` in a step of the person's"),
          ("a floor nothing fails", needs_replaced(powered[0], "power", 0), "`power` is a battery share"),
          ("a share of the disk with no cap", needs_replaced(applies[0], "free-space", {"at": "/nix/store", "percent": 5}), "`free-space` is {"),
          ("room under a relative path", needs_replaced(applies[0], "free-space", {"at": "nix/store", "percent": 5, "cap-gib": 20}), "`free-space.at` is an absolute path"),
          ("a hold that is not true", with_need(moves[0], {"not-held": False}), "`not-held` only takes `true`"),
          ("a rehearsal that is the run", changed(rehearsing[0], rehearse=declared[rehearsing[0]]["run"]), "a rehearsal would run the step for real"),
          ("an empty rehearsal", changed(rehearsing[0], rehearse=[]), "`rehearse` is empty"),
          ("a rehearsal that installs", changed(rehearsing[0], rehearse=["install", "--check"]), "installs"),
          ("a rehearsal of a unit", changed(units[0], rehearse=["--dry-run"]), "go with a program"),
          ("refusals that are no words", changed(applies[0], declines=["Did Not Boot"]), "distinct kebab-case refusal words"),
          ("a lock moved on a battery the rebuild waits for", needs_dropped(moves[0], "power"), "`move-inputs` without the rebuild's `power`"),
          ("a lock moved on a disk the rebuild waits for", needs_dropped(moves[0], "free-space"), "`move-inputs` without the rebuild's `free-space`"),
      ]

      clean = generate(declared)
      if clean.returncode != 0:
          wrong.append("the declared steps were refused:\n" + clean.stderr)
      for name, table, sentence in CASES:
          r = generate(table)
          if r.returncode == 0:
              wrong.append(f"{name}: the generator ACCEPTED it")
          elif sentence not in r.stderr:
              wrong.append(f"{name}: refused, but not for this law (no {sentence!r}):\n{r.stderr}")

      if wrong:
          print("ERROR: the update's generator does not refuse what the 2026-10-05 laws forbid.", file=sys.stderr)
          for w in wrong:
              print("       " + w, file=sys.stderr)
          raise SystemExit(1)
      print(f"OK: the declared table passes, and each of the {len(CASES)} broken laws is refused with its own sentence")
      END
    '';

    # THE UPDATE RUNS IN ITS ORDER, WHOLE — the generated halves RUN, in a
    # world of stand-ins (`fixtureRunners`):
    #
    #   1. a run gathers (fetch, pins, stage) before the root half starts,
    #      runs the person's half after it, and publishes last;
    #   2. a root half that stops stops the person's half — which still
    #      publishes what this machine did, and only that;
    #   3. a rehearsal runs, of the steps with a rehearsal of their own,
    #      that rehearsal, and of every other step nothing;
    #   4. the rebuild's `refused <word>` it declares is `declined-<word>`,
    #      news and not a failure — its function lifted out of the root half
    #      and run over a planted machine directory — and any other refusal
    #      is a failure.
    system-update-gathers-first-and-publishes-last =
      mkCheck "system-update-gathers-first-and-publishes-last"
        [
          pkgs.python3
          pkgs.bash
          pkgs.coreutils
        ]
        ''
          python3 - <<'END'
          import json, os, pathlib, shutil, subprocess, sys
          exec(open("${pyPrelude}").read())

          # PARSED: the table arrives as JSON, and both generated halves are RUN — in
          # a world of stand-ins that write down what they were asked, read back as
          # lines.
          # FILE-WIDE: the one function the last part runs is cut out of the root half
          # by its own `name() {` line (`cut`), with the helpers it calls; `body_of`
          # walks Rust braces, and this is shell.
          PERSON = "${fixtureRunners}/bin/ronoctua-update"
          ROOT = "${fixtureRunners}/libexec/ronoctua-update-system"
          MACHINE_DIR = "${machineDir}"
          steps = json.load(open("${stepsJson}"))["steps"]
          floor(len(steps), 10, "steps in the table")
          wrong = []

          # THE STEPS BY WHAT THEY DO, never by id (`…-has-one-list`): what each one
          # runs is read from the table, and looked for in what the stand-ins logged.
          def line_of(s, args):
              return f"{s['program']} {' '.join(args)}".rstrip()

          def runs(word):
              return [s for s in steps if s["does"] == word and "package" in s]

          gathering, publishing = runs("gather"), runs("publish")
          rehearsed = [s for s in gathering + publishing if "rehearse" in s]
          printed = [s for s in gathering + publishing if "rehearse" not in s]
          if not gathering or not publishing or not rehearsed or not printed:
              sys.exit("ERROR: the table has no gathering, no publishing, or none with (or without) a rehearsal")
          cache = [s for s in steps if s["does"] == "refresh-cache" and s["enable"] and "package" in s
                   and s["needs"] == [{"always": True}]]
          if not cache:
              sys.exit("ERROR: no step of the person's that always runs, to tell the person's half ran")

          def room(name):
              home = pathlib.Path(name).resolve()
              if home.exists():
                  shutil.rmtree(home)
              for d in ("config/ronoctua", "state/ronoctua/person/software", "runtime", "data", "cache", "work"):
                  (home / d).mkdir(parents=True)
              # WHAT THE GATHERING AND THE PUBLISHING NEED TO RUN AT ALL: a repository
              # configured, a declaration, the mirror — planted; and the network up.
              (home / "config/ronoctua/belongings.json").write_text("{}\n")
              (home / "state/ronoctua/person/software/declared.json").write_text('{"format": 1, "apps": []}\n')
              return home

          def run(home, *args, **fixture):
              log = home / "calls.log"
              log.write_text("")
              env = {
                  "PATH": os.environ["PATH"],
                  "HOME": str(home),
                  "XDG_CONFIG_HOME": str(home / "config"),
                  "XDG_STATE_HOME": str(home / "state"),
                  "XDG_RUNTIME_DIR": str(home / "runtime"),
                  "XDG_DATA_HOME": str(home / "data"),
                  "XDG_CACHE_HOME": str(home / "cache"),
                  "FIXTURE_LOG": str(log),
                  "FIXTURE_ROOT_HALF": ROOT,
                  "FIXTURE_ONLINE": "1",
                  **{k: v for k, v in fixture.items()},
              }
              # NOTHING ON STANDARD INPUT, and a deadline: a half that asked anything
              # would wait for an answer that never comes.
              r = subprocess.run([PERSON, *args], cwd=home / "work", env=env, stdin=subprocess.DEVNULL,
                                 capture_output=True, text=True, timeout=120)
              return r, log.read_text().splitlines()

          def first(calls, needle):
              at = [i for i, c in enumerate(calls) if c == needle or c.startswith(needle)]
              return at[0] if at else None

          # 1. A RUN: the gathering, then the root half, then the person's half, and
          #    the publishing last.
          home = room("run")
          r, calls = run(home, "--execute")
          root_at = first(calls, "systemctl start ronoctua-update-system.service")
          if r.returncode != 0 or root_at is None:
              wrong.append(f"a run did not go through (exit {r.returncode}, root half at {root_at}):\n{r.stdout}{r.stderr}")
          else:
              for s in gathering:
                  at = first(calls, line_of(s, s["run"]))
                  if at is None or at > root_at:
                      wrong.append(f"a gathering step ran at {at}, and the root half started at {root_at}: {calls}")
              cached = first(calls, line_of(cache[0], cache[0]["run"]))
              if cached is None or cached < root_at:
                  wrong.append(f"the person's half did not run after the root half: {calls}")
              tail = calls[-len(publishing):]
              if tail != [line_of(s, s["run"]) for s in publishing]:
                  wrong.append(f"the publishing is not the last thing a run does: {calls[-5:]}")

          # 2. A ROOT HALF THAT STOPS: the person's half stops too — and still
          #    publishes what this machine did.
          home = room("stopped")
          r, calls = run(home, "--execute", FIXTURE_ROOT_FAILS="1")
          root_at = first(calls, "systemctl start ronoctua-update-system.service")
          report = (home / "runtime/ronoctua-update/user.report").read_text()
          if r.returncode == 0:
              wrong.append("a run whose root half stopped ended as if it had not")
          if "system-half\tfailed" not in report:
              wrong.append(f"the stopped root half is not in the report: {report!r}")
          if root_at is None or calls[root_at + 1:] != [line_of(s, s["run"]) for s in publishing]:
              wrong.append(f"after a root half that stopped, the run does not publish — and only that: {calls}")

          # 3. A REHEARSAL: a step with a rehearsal of its own runs it, and every other
          #    step runs nothing.
          home = room("rehearsal")
          r, calls = run(home)
          if first(calls, "systemctl start ronoctua-update-system-rehearsal.service") is None:
              wrong.append(f"a rehearsal did not rehearse the root half: {calls}")
          for s in rehearsed:
              if line_of(s, s["rehearse"]) not in calls:
                  wrong.append(f"a rehearsal did not run `{line_of(s, s['rehearse'])}`: {calls}")
          for s in steps:
              if s.get("root") is False and "package" in s and line_of(s, s["run"]) in calls:
                  wrong.append(f"a rehearsal RAN `{line_of(s, s['run'])}`")
          user_report = (home / "runtime/ronoctua-update/user.report").read_text()
          if "\tran\t" in user_report:
              wrong.append(f"a rehearsal recorded a step as run: {user_report!r}")

          # 4. A REFUSAL THAT IS NEWS: the rebuild's function, lifted out of the root
          #    half and run over a planted machine directory — `refused <word>` it
          #    declares is `declined-<word>` and the sequence goes on; any other
          #    refusal is a failure.
          script = pathlib.Path(ROOT).read_text()
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
          applying = [s for s in steps if s["does"] == "apply-configuration"]
          declining = applying[0]
          if not declining.get("declines"):
              wrong.append("the rebuild declares no refusal that is news")
          else:
              word = declining["declines"][0]
              fn = "step_" + declining["id"].replace("-", "_")
              helpers = ["say() {", "record() {"]
              lifted = "\n".join(
                  ["set -uo pipefail"]
                  + [l for l in script.splitlines() if l.startswith(tuple(helpers))]
                  + [cut(f) for f in ("skip() {", "turned_on() {", "default_on() {", "powered() {",
                                      "battery_at_least() {", "room() {", "roomy() {", fn + "() {")])
              machine = pathlib.Path("machine").resolve()
              machine.mkdir(exist_ok=True)
              (machine / "flake.lock").write_text("{}\n")
              lifted = lifted.replace(MACHINE_DIR + "/", str(machine) + "/")
              pathlib.Path("lifted.sh").write_text(lifted + "\n")
              def apply(**fixture):
                  log, report = pathlib.Path("apply.log").resolve(), pathlib.Path("apply.tsv").resolve()
                  log.write_text("")
                  report.write_text("")
                  env = dict(os.environ, FIXTURE_LOG=str(log), **fixture)
                  r = subprocess.run(["bash", "-c", f'execute=true; report="{report}"; . ./lifted.sh; {fn}'],
                                     capture_output=True, text=True, env=env, stdin=subprocess.DEVNULL, timeout=60)
                  return r.returncode, report.read_text(), log.read_text()
              code, report, log = apply(FIXTURE_DID_NOT_BOOT="1")
              if not log.strip():
                  wrong.append("the lifted rebuild did not run its program — did a precondition skip it?")
              if code != 0 or f"\tdeclined-{word}\t" not in report:
                  wrong.append(f"`refused {word}` is not news ({code}, {report!r})")
              code, report, log = apply(FIXTURE_APPLY_FAILS="1")
              if code == 0 or "\tfailed-build-failed\t" not in report:
                  wrong.append(f"a refusal the step does not declare is not a failure that keeps its word ({code}, {report!r})")
              code, report, log = apply(FIXTURE_APPLY_CRASHES="1")
              if code == 0 or "\tfailed\t" not in report:
                  wrong.append(f"a last line that is no refusal made a word up ({code}, {report!r})")
              code, report, log = apply()
              if code != 0 or "\tran\t" not in report:
                  wrong.append(f"an apply that worked is not `ran` ({code}, {report!r})")

          if wrong:
              print("ERROR: the update does not run in its order.", file=sys.stderr)
              for w in wrong:
                  print("       " + w, file=sys.stderr)
              raise SystemExit(1)
          print("OK: the gathering runs before the root half and the publishing last — on every way out —, "
                "a rehearsal runs only the rehearsals, and a refusal that is news is not a failure")
          END
        '';

    # THE LOCK MOVES ONLY FOR A REBUILD THAT CAN BE TRIED — the step that
    # moves the inputs and the rebuild, lifted out of the root half with the
    # helpers they call and RUN one after the other, as the half runs them,
    # over a planted machine directory, power supplies, disk and record:
    #
    #   1. on a battery below the floor, and on a disk still short after the
    #      store's garbage was collected, BOTH are skipped and the machine's
    #      `flake.lock` is byte for byte what it was — no `nix flake update`
    #      ran; and when the collection frees the room both run, the lock
    #      moved (which is what proves the stand-in would have said so);
    #   2. a disk short of room is collected FIRST, then measured again, then
    #      refused — and a rehearsal collects nothing;
    #   3. a machine held at a revision keeps its lock (`skipped-not-held`, no
    #      `nix` asked), and so does one whose record does not read; one held
    #      at nothing moves it;
    #   4. the rebuild's rehearsal runs its program's rehearsal, and reads
    #      the refusal it declares as news there too.
    system-update-moves-the-lock-only-for-a-rebuild =
      mkCheck "system-update-moves-the-lock-only-for-a-rebuild"
        [
          pkgs.python3
          pkgs.bash
          pkgs.coreutils
        ]
        ''
          python3 - <<'END'
          import json, os, pathlib, shutil, subprocess, sys
          exec(open("${pyPrelude}").read())

          # PARSED: the table arrives as JSON; the two steps' functions are RUN,
          # lifted out of the generated root half with the helpers they call.
          # FILE-WIDE: each function is cut out of the root half by its own
          # `name() {` line (`cut`); `body_of` walks Rust braces, and this is shell.
          ROOT = "${fixtureRunners}/libexec/ronoctua-update-system"
          MACHINE_DIR = "${machineDir}"
          steps = json.load(open("${stepsJson}"))["steps"]
          script = pathlib.Path(ROOT).read_text()
          floor(len(script), 1000, "characters of the root half")
          wrong = []

          # THE TWO STEPS BY WHAT THEY DO, never by id (`…-has-one-list`).
          def doing(word):
              found = [s for s in steps if s["does"] == word]
              if len(found) != 1:
                  sys.exit(f"ERROR: {len(found)} steps do `{word}`, and there has to be one")
              return found[0], "step_" + found[0]["id"].replace("-", "_")
          mover, move_fn = doing("move-inputs")
          applier, apply_fn = doing("apply-configuration")
          battery = next(n["power"] for n in applier["needs"] if "power" in n)

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
          wanted = ["skip() {", "turned_on() {", "default_on() {", "connected() {", "powered() {",
                    "battery_at_least() {", "room() {", "roomy() {", "unheld() {",
                    move_fn + "() {", apply_fn + "() {"]
          bodies = [cut(w) for w in wanted]
          for name, body in zip(wanted, bodies):
              if not body:
                  sys.exit(f"ERROR: the root half has no `{name}` to lift")
          one_liners = [l for l in script.splitlines() if l.startswith(("say() {", "record() {"))]
          lifted = "\n".join(["set -uo pipefail", *one_liners, *bodies])

          ORIGINAL = "the lock the running system was built from\n"

          def run(execute="true", charge=None, state=None, **fixture):
              work = pathlib.Path("lock").resolve()
              if work.exists():
                  shutil.rmtree(work)
              machine = work / "machine"
              machine.mkdir(parents=True)
              (machine / "flake.lock").write_text(ORIGINAL)
              (work / "runtime").mkdir()
              supplies = work / "supplies"
              (supplies / "AC").mkdir(parents=True)
              (supplies / "AC/type").write_text("Mains\n")
              if charge is not None:
                  (supplies / "BAT0").mkdir()
                  (supplies / "BAT0/type").write_text("Battery\n")
                  (supplies / "BAT0/capacity").write_text(f"{charge}\n")
              if state is not None:
                  (work / "state.json").write_text(state)
              # THE MACHINE DIRECTORY, wherever the two functions name it — the
              # lock they test and the flake the update moves.
              (work / "lifted.sh").write_text(lifted.replace(MACHINE_DIR, str(machine)) + "\n")
              log, report = work / "calls.log", work / "report.tsv"
              log.write_text("")
              report.write_text("")
              env = dict(os.environ, FIXTURE_LOG=str(log), FIXTURE_ONLINE="1", **fixture)
              r = subprocess.run(
                  ["bash", "-c", f'execute={execute}; report="{report}"; . ./lifted.sh; {move_fn} || true; {apply_fn}'],
                  cwd=work, env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=60)
              return {
                  "code": r.returncode,
                  "said": r.stdout,
                  "report": report.read_text(),
                  "calls": log.read_text(),
                  "lock": (machine / "flake.lock").read_text(),
              }

          def verdict(out, step):
              for line in out["report"].splitlines():
                  if line.startswith(step["id"] + "\t"):
                      return line.split("\t")[1]
              return None

          # THE LOCK'S BYTES, and the call: the stand-in moves the lock it is
          # pointed at, as `nix flake update` would.
          def moved(out):
              return out["lock"] != ORIGINAL or "nix flake update" in out["calls"]

          def bytes_moved(out):
              return out["lock"] != ORIGINAL

          # 1. ON A LOW BATTERY: neither runs, and the lock stays.
          out = run(charge=battery - 10, FIXTURE_AC="1")
          if (verdict(out, mover), verdict(out, applier)) != ("skipped-power", "skipped-power"):
              wrong.append(f"on battery below the floor: {out['report']!r}")
          if moved(out):
              wrong.append(f"on battery below the floor the lock moved for a rebuild that was skipped: {out['calls']!r}")
          #    ON A DISK STILL SHORT AFTER THE COLLECTION: the same.
          out = run(FIXTURE_FREE_MIB="100")
          if (verdict(out, mover), verdict(out, applier)) != ("skipped-free-space", "skipped-free-space"):
              wrong.append(f"on a disk short of room: {out['report']!r}")
          if moved(out):
              wrong.append(f"on a disk short of room the lock moved for a rebuild that was skipped: {out['calls']!r}")
          # 2. …COLLECTED FIRST, and only then refused.
          if "collect-garbage" not in out["calls"]:
              wrong.append(f"a disk short of room was refused before the store's garbage was collected: {out['calls']!r}")
          #    AND WHEN THE COLLECTION FREES THE ROOM, both run and the lock moves.
          out = run(FIXTURE_FREE_MIB="100", FIXTURE_GC_FREES="50000")
          if (verdict(out, mover), verdict(out, applier)) != ("ran", "ran") or not bytes_moved(out):
              wrong.append(f"room freed by the collection, and the lock did not move or the rebuild did not run: {out['report']!r} {out['calls']!r}")
          if "collect-garbage" not in out["calls"]:
              wrong.append("the room was found without collecting")
          #    A REHEARSAL COLLECTS NOTHING.
          out = run(execute="false", FIXTURE_FREE_MIB="100", FIXTURE_GC_FREES="50000")
          if "collect-garbage" in out["calls"]:
              wrong.append(f"a rehearsal collected the store's garbage: {out['calls']!r}")
          if verdict(out, mover) != "skipped-free-space" or moved(out):
              wrong.append(f"a rehearsal short of room: {out['report']!r}")

          # 3. A HOLD keeps the lock where it is, and the rebuild still applies
          #    what the folder holds.
          held = '{"format": 1, "last-good": null, "trial": null, "hold": {"rev": "613862c", "since": "2026-10-05T12:00:00Z"}, "last-apply": null}'
          out = run(state=held)
          if verdict(out, mover) != "skipped-not-held" or moved(out):
              wrong.append(f"a held machine moved its lock: {out['report']!r} {out['calls']!r}")
          if verdict(out, applier) != "ran":
              wrong.append(f"a held machine did not apply what its folder holds: {out['report']!r}")
          out = run(state="{")
          if verdict(out, mover) != "skipped-not-held" or moved(out):
              wrong.append(f"a record that does not read let the lock move: {out['report']!r}")
          out = run(state=held.replace('{"rev": "613862c", "since": "2026-10-05T12:00:00Z"}', "null"))
          if verdict(out, mover) != "ran" or not moved(out):
              wrong.append(f"a machine held at nothing did not move its lock: {out['report']!r}")

          # 4. THE REBUILD'S REHEARSAL runs its program's rehearsal — and its
          #    declared refusal is news there too; another is a failure.
          if "rehearse" not in applier:
              wrong.append("the rebuild has no rehearsal of its own")
          else:
              rehearsing = f"{applier['program']} {' '.join(applier['rehearse'])}"
              out = run(execute="false")
              if rehearsing not in out["calls"] or verdict(out, applier) != "rehearsed":
                  wrong.append(f"a rehearsal did not run `{rehearsing}` and record it: {out['report']!r} {out['calls']!r}")
              if moved(out):
                  wrong.append("a rehearsal moved the lock")
              word = applier["declines"][0]
              out = run(execute="false", FIXTURE_DID_NOT_BOOT="1")
              if out["code"] != 0 or verdict(out, applier) != f"declined-{word}":
                  wrong.append(f"in a rehearsal, `refused {word}` is not news: {out['report']!r}")
              out = run(execute="false", FIXTURE_APPLY_FAILS="1")
              if out["code"] == 0 or verdict(out, applier) != "failed-build-failed":
                  wrong.append(f"in a rehearsal, a refusal the step does not declare is not a failure: {out['report']!r}")

          if wrong:
              print("ERROR: the update moves the lock on a run whose rebuild cannot be tried.", file=sys.stderr)
              for w in wrong:
                  print("       " + w, file=sys.stderr)
              raise SystemExit(1)
          print("OK: the lock moves only when the rebuild can be tried — on mains or above the floor, with room "
                "found after collecting, and never while held — and a rehearsal collects and moves nothing")
          END
        '';

    # A SCHEDULED RUN WAITS FOR THE CHARGER, AND NOTHING EVER ASKS (the
    # design's `update-never-asks`, beside `system-update-never-asks`, which
    # holds the authentication half).
    #
    # The brain holds a scheduled run on a metered network or a battery
    # below the floor (the module's own test, `needs`); this holds the
    # runners: the root half's test of the battery — lifted and run over
    # planted supplies — takes mains, or the machine's batteries averaged
    # against the floor (a mouse's counted for nothing), or the person's yes
    # to THIS run, and the yes only when the run acts; every step that needs
    # power asks before it runs anything, and says it skipped; and the
    # person's half takes a yes only from `--execute --on-battery` (the
    # doors' question writes the same file), hands it to the root half while
    # the run lasts and takes it away however it ends. Both halves run with
    # nothing on standard input, and end by themselves.
    system-update-waits-for-the-charger =
      mkCheck "system-update-waits-for-the-charger"
        [
          pkgs.python3
          pkgs.bash
          pkgs.coreutils
        ]
        ''
          python3 - <<'END'
          import json, os, pathlib, shutil, subprocess, sys
          exec(open("${pyPrelude}").read())

          # PARSED: the table arrives as JSON; the root half's test of the battery is
          # RUN, lifted out of it, and the person's half is run whole.
          # FILE-WIDE: `powered` and each step's function are cut out of the root half
          # by their own `name() {` lines (`cut`); `body_of` walks Rust braces, and
          # this is shell.
          PERSON = "${fixtureRunners}/bin/ronoctua-update"
          ROOT = "${fixtureRunners}/libexec/ronoctua-update-system"
          steps = json.load(open("${stepsJson}"))["steps"]
          wrong = []
          script = pathlib.Path(ROOT).read_text()
          floor(len(script), 1000, "characters of the root half")

          floors = sorted({n["power"] for s in steps for n in s["needs"] if "power" in n})
          if len(floors) != 1:
              sys.exit(f"ERROR: the table declares {floors} as its battery floor; one is the law")
          floor = floors[0]

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

          # 1. THE ROOT HALF'S OWN TEST, lifted out of it and run over planted power
          #    supplies: mains, or the machine's batteries averaged against the floor
          #    (a mouse's battery counted for nothing), or the person's yes — and the
          #    yes only in a run that acts, never in a rehearsal.
          lifted = "\n".join(["set -uo pipefail", "say() { printf '%s\\n' \"$*\"; }",
                              cut("powered() {"), cut("battery_at_least() {")])
          if "powered() {" not in lifted or "battery_at_least() {" not in lifted:
              sys.exit("ERROR: the root half has no `powered` to lift: does a root step still need power?")
          work = pathlib.Path("charger").resolve()
          pathlib.Path("powered.sh").write_text(lifted + "\n")

          def plant(batteries, yes=False):
              if work.exists():
                  shutil.rmtree(work)
              (work / "supplies").mkdir(parents=True)
              (work / "runtime").mkdir()
              for name, (capacity, scope) in batteries.items():
                  d = work / "supplies" / name
                  d.mkdir()
                  (d / "type").write_text("Battery\n")
                  (d / "capacity").write_text(f"{capacity}\n")
                  if scope:
                      (d / "scope").write_text(f"{scope}\n")
              # A MAINS ADAPTER IS A SUPPLY TOO, and not a battery.
              (work / "supplies/AC").mkdir()
              (work / "supplies/AC/type").write_text("Mains\n")
              if yes:
                  (work / "runtime/1000/ronoctua-update").mkdir(parents=True)
                  (work / "runtime/1000/ronoctua-update/on-battery").write_text("")

          def powered(execute, mains):
              env = dict(os.environ, FIXTURE_AC="0" if mains else "1")
              r = subprocess.run(["bash", "-c", f"execute={execute}; . {pathlib.Path('powered.sh').resolve()}; powered {floor}"],
                                 cwd=work, env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=30)
              return r.returncode == 0, r.stdout

          low, high = floor - 10, floor + 10
          CASES = [
              ("on mains, however flat", {"BAT0": (5, "System")}, False, "true", True, True),
              ("on battery above the floor", {"BAT0": (high, None)}, False, "true", False, True),
              ("on battery below the floor, nobody said yes", {"BAT0": (low, None)}, False, "true", False, False),
              ("a mouse's full battery does not lift a flat machine", {"BAT0": (low, None), "hidpp_battery_0": (100, "Device")}, False, "true", False, False),
              ("two batteries are averaged", {"BAT0": (low, None), "BAT1": (floor + 20, None)}, False, "true", False, True),
              ("a battery whose charge cannot be read is not enough", {}, False, "true", False, False),
              ("below the floor, the person said yes to this run", {"BAT0": (low, None)}, True, "true", False, True),
              ("a rehearsal does not take the yes", {"BAT0": (low, None)}, True, "false", False, False),
          ]
          for name, batteries, yes, execute, mains, expected in CASES:
              plant(batteries, yes)
              got, said = powered(execute, mains)
              if got != expected:
                  wrong.append(f"{name}: powered is {got}, and should be {expected} ({said!r})")

          # 2. EVERY STEP THAT NEEDS POWER ASKS IT, with the one floor, before it runs
          #    anything — and is SKIPPED, said, when it does not hold.
          for s in steps:
              if not any("power" in n for n in s["needs"]):
                  continue
              body = cut("step_" + s["id"].replace("-", "_") + "() {")
              ask = body.find(f"if ! powered {floor}; then")
              act = body.find('say ">> $id:')
              if ask < 0 or act < 0 or ask > act or f'skip "$id" power' not in body:
                  wrong.append(f"a step that needs power does not ask before it acts, or does not say it skipped: {body[:300]}")

          # 3. THE PERSON'S HALF TAKES A YES ONLY FROM `--execute --on-battery`, hands
          #    it to the root half while the run lasts, and takes it away however the
          #    run ends — so the next run, a scheduled one, never finds one. Nothing is
          #    read from standard input, and each run ends by itself.
          def run(args, stale=False, fails=False):
              home = pathlib.Path("person").resolve()
              if home.exists():
                  shutil.rmtree(home)
              for d in ("runtime", "config", "state", "data", "cache", "work"):
                  (home / d).mkdir(parents=True)
              yes = home / "runtime/ronoctua-update/on-battery"
              if stale:
                  yes.parent.mkdir(parents=True)
                  yes.write_text("")
              log = home / "calls.log"
              log.write_text("")
              env = {"PATH": os.environ["PATH"], "HOME": str(home), "XDG_RUNTIME_DIR": str(home / "runtime"),
                     "XDG_CONFIG_HOME": str(home / "config"), "XDG_STATE_HOME": str(home / "state"),
                     "XDG_DATA_HOME": str(home / "data"), "XDG_CACHE_HOME": str(home / "cache"),
                     "FIXTURE_LOG": str(log), "FIXTURE_ROOT_HALF": ROOT}
              if fails:
                  env["FIXTURE_ROOT_FAILS"] = "1"
              r = subprocess.run([PERSON, *args], cwd=home / "work", env=env, stdin=subprocess.DEVNULL,
                                 capture_output=True, text=True, timeout=120)
              return r, log.read_text(), yes.exists()

          r, calls, left = run(["--execute", "--on-battery"])
          if "saw a yes" not in calls:
              wrong.append(f"`--execute --on-battery` did not hand the root half a yes: {calls!r}")
          if left:
              wrong.append("the yes outlived the run it was given to")
          r, calls, left = run(["--execute", "--on-battery"], fails=True)
          if left:
              wrong.append("a run that stopped left its yes behind")
          r, calls, left = run(["--on-battery"])
          if "saw a yes" in calls or left:
              wrong.append("a REHEARSAL took a yes to a run on battery")
          r, calls, left = run(["--execute"])
          if "saw a yes" in calls:
              wrong.append("a run with nobody's yes handed the root half one")
          r, calls, left = run(["--execute"], stale=True)
          if left:
              wrong.append("a yes a door left was still there after the run it was given to")

          if wrong:
              print("ERROR: the update does not wait for the charger, or asks.", file=sys.stderr)
              for w in wrong:
                  print("       " + w, file=sys.stderr)
              raise SystemExit(1)
          print(f"OK: below {floor}% on battery the steps that compile wait for the charger or the person's yes "
                "to this run, a rehearsal never takes one, and nothing is ever asked")
          END
        '';

    # A REHEARSAL OF THE UPDATE CHANGES NOTHING (the design's
    # `update-rehearsal-changes-nothing`), on a booted machine, with the
    # machine's own runners and the real programs: the person's half run as
    # the person, starting the root half's rehearsal through polkit with no
    # agent, over a machine flake of its own (offline: its distro is a flake
    # on the machine), the person's staged declarations, their mirror, the
    # reconciler's ledger and the machine's record. Every byte of the
    # machine directory, the record's folder, the staging folder and the
    # person's state is the same afterwards; the verification and the apply
    # REHEARSED (they ran, changing nothing), the machine's record was
    # compared and not copied, and no step says it ran.
    system-update-rehearsal-changes-nothing = import ../../../../tests/update-rehearsal-vm.nix {
      inherit pkgs lib machineDir;
      name = "${id}-rehearsal-changes-nothing";
      user = vm.ronoctua.user;
      runners = vm.ronoctua.update.package;
      rehearsalUnit = units.system-rehearsal;
      steps = {
        verify = (stepThat "verify the declarations" (s: s.does == "verify")).id;
        apply = (stepThat "apply the configuration" (s: s.does == "apply-configuration")).id;
        # The machine's record, by what it runs: the person's binary cache
        # publishes with a rehearsal of its own too (`binary-cache-push`).
        record =
          (stepThat "publish the machine's record with a rehearsal of its own" (
            s: s.does == "publish" && s ? rehearse && (s.run or [ ]) == [ "record" ]
          )).id;
      };
      distroInput = builtins.head (
        builtins.attrNames (import ../../../../templates/machine/flake.nix).inputs
      );
      fixtures = ../../../../crates/ronoctua-person/fixtures/good;
    };
  };
  slow = [
    # ── anchor: L19 ──
  ];
}
