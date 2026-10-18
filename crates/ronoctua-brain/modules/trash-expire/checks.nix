# The trash-expire module's own checks, in the shape every module's `checks.nix`
# has: the arguments it asks for by name (`nix/brain-modules.nix`,
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
  unitFacts =
    unit:
    let
      s = vm.systemd.user.services.${service unit}.serviceConfig;
    in
    {
      name = unit.name;
      # READ AS WORDS, WITHOUT ITS STORE PATHS: the command line names the
      # emptying's binary, and a file quoting it would make this check build the
      # whole workspace to compare two strings.
      execStart = builtins.unsafeDiscardStringContext s.ExecStart;
      readWritePaths = s.ReadWritePaths or [ ];
    };
in
{
  checks = {
    # THE EMPTYING TOUCHES THE TRASH AND NOTHING ELSE, and only the real run
    # acts.
    #
    # Where the two units write their reports is the core's check
    # (`the-brain-reads-what-the-units-write`, which reads every manifest);
    # what is this module's alone is what its sandbox opens. A sweep that
    # could reach the trash would be a cache cleaner deleting the only copy of
    # somebody's file, so the writable paths besides the reports are `%S` —
    # never `%C` — and the mounted drives' own trashes (`/run/media/%u`), and
    # the walk's data home is that same `%S`. And the
    # rehearsal deletes nothing: the word that deletes is on the run alone.
    # This reads the units as the machine gets them — the evaluated
    # configuration, not the file.
    trash-expire-touches-only-the-trash =
      let
        facts = pkgs.writeText "trash-expire-units.json" (
          builtins.toJSON {
            inherit (module) id;
            run = unitFacts module.units.run;
            rehearse = unitFacts module.units.rehearse;
          }
        );
      in
      mkCheck "trash-expire-touches-only-the-trash" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the two units arrive as the evaluated configuration, JSON.
        # FILE-WIDE: one constant of the runner, comments off and strings kept
        # (`uncommented`) — the needle IS a string.
        facts = json.load(open("${facts}"))
        runner = uncommented(pathlib.Path("${../../../ronoctua-brain-daemon/src/runner.rs}").read_text())
        floor(len(runner), 1000, "characters of runner.rs")
        wrong = []

        leaf = re.search(r'pub const REPORTS_LEAF: &str = "([^"]+)";', runner)
        if not leaf:
            sys.exit("ERROR: `REPORTS_LEAF` went away; it is the one other directory the emptying may write")
        leaf = leaf.group(1)

        for which, acts in (("run", True), ("rehearse", False)):
            u = facts[which]
            start = u["execStart"]
            if ("--execute" in start.split()) != acts:
                wrong.append(f"{u['name']} " + ("does not act" if acts else "acts: a rehearsal deleted something"))
            if "--data-home %S" not in start:
                wrong.append(f"{u['name']} does not read %S, the one directory its sandbox opens")
            # AND THE MOUNTED DRIVES' OWN TRASHES (`/run/media/<person>`),
            # where the file manager puts what is thrown away on a stick: the
            # walk is told the folder, and the sandbox opens it — never more.
            if "--media /run/media/%u" not in start:
                wrong.append(f"{u['name']} does not walk the mounted drives' trashes it opens")
            if sorted(p.lstrip("-") for p in u["readWritePaths"]) != sorted(["%S", f"%t/{leaf}", "/run/media/%u"]):
                wrong.append(f"{u['name']} may write outside the data home, the drives' trashes and the reports: {u['readWritePaths']}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the emptying's two units open %S, the drives' trashes and the reports alone, and only the run acts")
        END
      '';

    # SWITCHED OFF BY THE SYSTEM, IT GOES — AND THE BRAIN IS TOLD.
    #
    # `ronoctua.brain.modules.<id>.enable = false` is the one switch a machine
    # has for a module, and it has two halves that must move together: the
    # units leave the system (the loader's gate around `module.nix`), and the
    # brain's unit is handed the id (`RONOCTUA_BRAIN_OFF`), so the row is
    # drawn refusing with the reason instead of vanishing — or, worse, staying
    # on the page and starting units that are no longer there. This one,
    # because it is the module that deletes the only copy of something: the
    # machine that switches it off has to be sure it is off. The configuration
    # is the VM's, evaluated twice: as it ships and with the switch off.
    trash-expire-switched-off-by-the-system =
      let
        off = self.nixosConfigurations.vm.extendModules {
          modules = [ { ronoctua.brain.modules.${module.id}.enable = false; } ];
        };
        # AND THE CATALOGUE'S "THROW NOTHING AWAY", which is this switch's
        # default: it once took the units away and told the brain nothing.
        nothing = self.nixosConfigurations.vm.extendModules {
          modules = [ { ronoctua.cache.enable = false; } ];
        };
        facts =
          config:
          let
            services = config.systemd.user.services;
          in
          {
            units = map (u: services ? ${lib.removeSuffix ".service" u.name}) [
              module.units.run
              module.units.rehearse
            ];
            told = services.ronoctua-brain.environment.RONOCTUA_BRAIN_OFF or null;
          };
        both = pkgs.writeText "trash-expire-switched.json" (
          builtins.toJSON {
            inherit (module) id;
            on = facts vm;
            off = facts off.config;
            nothing = facts nothing.config;
          }
        );
      in
      mkCheck "trash-expire-switched-off-by-the-system" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: both configurations arrive evaluated, as JSON.
        facts = json.load(open("${both}"))
        i, on = facts["id"], facts["on"]
        wrong = []
        if on["units"] != [True, True]:
            wrong.append(f"as it ships, the machine lacks the units: {on['units']}")
        if on["told"] is None or i in on["told"].split(","):
            wrong.append(f"as it ships, the brain is told {on['told']!r} — it must be told, and not this")
        for how in ("off", "nothing"):
            gone = facts[how]
            if any(gone["units"]):
                wrong.append(f"switched off ({how}), a unit stayed: {gone['units']}")
            if gone["told"] is None or i not in gone["told"].split(","):
                wrong.append(f"switched off ({how}), the brain is told {gone['told']!r}, and would start units that are gone")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: switched off by the system, by its own switch or by the cache's, {i}'s units leave and the brain is told")
        END
      '';
  };
  slow = [ ];
}
