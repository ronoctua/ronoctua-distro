# The cache-sweep module's own checks, in the shape every module's `checks.nix`
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
      # sweep's binary, and a file quoting it would make this check build the
      # whole workspace to compare two strings.
      execStart = builtins.unsafeDiscardStringContext s.ExecStart;
      readWritePaths = s.ReadWritePaths or [ ];
    };
in
{
  checks = {
    # THE SWEEP WALKS THE CACHE AND NOTHING ELSE, and only the real run acts.
    #
    # Where the two units write their reports is the core's check
    # (`the-brain-reads-what-the-units-write`, which reads every manifest);
    # what is this module's alone is what its sandbox opens: the one writable
    # path besides the reports is `%C`, and the walk's root is that same `%C`
    # — a sandbox and a walk that disagreed about which directory that is
    # would fail in a way that reads as a bug in the walk. And the rehearsal
    # deletes nothing: the word that deletes is on the run alone. This reads
    # the units as the machine gets them — the evaluated configuration, not
    # the file.
    cache-sweep-touches-only-the-cache =
      let
        facts = pkgs.writeText "cache-sweep-units.json" (
          builtins.toJSON {
            inherit (module) id;
            run = unitFacts module.units.run;
            rehearse = unitFacts module.units.rehearse;
          }
        );
      in
      mkCheck "cache-sweep-touches-only-the-cache" [ pkgs.python3 ] ''
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
            sys.exit("ERROR: `REPORTS_LEAF` went away; it is the one other directory the sweep may write")
        leaf = leaf.group(1)

        for which, acts in (("run", True), ("rehearse", False)):
            u = facts[which]
            start = u["execStart"]
            if ("--execute" in start.split()) != acts:
                wrong.append(f"{u['name']} " + ("does not act" if acts else "acts: a rehearsal deleted something"))
            if "--root %C" not in start:
                wrong.append(f"{u['name']} does not walk %C, the one directory its sandbox opens")
            if sorted(p.lstrip("-") for p in u["readWritePaths"]) != sorted(["%C", f"%t/{leaf}"]):
                wrong.append(f"{u['name']} may write outside the cache and the reports: {u['readWritePaths']}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the sweep's two units open %C and the reports alone, and only the run acts")
        END
      '';

    # THE CATALOGUE'S "THROW NOTHING AWAY" SWITCHES THE MODULE OFF — and the
    # brain is told.
    #
    # `ronoctua.cache.enable = false` is the default of this module's own
    # switch, and never a second gate around its units. It was one: the units
    # left and the brain, told nothing, went on scheduling the sweep, started a
    # unit that was no longer there and wrote a failure every cadence. The
    # configuration is the VM's, evaluated twice: as it ships and with the
    # cache's switch off.
    cache-sweep-switched-off-with-the-cache =
      let
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
        both = pkgs.writeText "cache-sweep-switched.json" (
          builtins.toJSON {
            inherit (module) id;
            on = facts vm;
            nothing = facts nothing.config;
          }
        );
      in
      mkCheck "cache-sweep-switched-off-with-the-cache" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: both configurations arrive evaluated, as JSON.
        facts = json.load(open("${both}"))
        i, on, nothing = facts["id"], facts["on"], facts["nothing"]
        wrong = []
        if on["units"] != [True, True]:
            wrong.append(f"as it ships, the machine lacks the units: {on['units']}")
        if on["told"] is None or i in on["told"].split(","):
            wrong.append(f"as it ships, the brain is told {on['told']!r} — it must be told, and not this")
        if any(nothing["units"]):
            wrong.append(f"with the cache's switch off, a unit stayed: {nothing['units']}")
        if nothing["told"] is None or i not in nothing["told"].split(","):
            wrong.append(f"with the cache's switch off, the brain is told {nothing['told']!r}, and would start units that are gone")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: with the cache's switch off, {i}'s units leave and the brain is told")
        END
      '';
  };
  slow = [ ];
}
