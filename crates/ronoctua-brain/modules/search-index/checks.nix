# The search-index module's own checks, in the shape every module's
# `checks.nix` has: the arguments it asks for by name
# (`nix/brain-modules.nix`, `checkArguments`), and its checks, each named
# after its id.
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
    # THE BRAIN STARTS THE UNIT IT READS, and nothing else starts it.
    #
    # Four things have to agree, and each lives somewhere else: the unit's
    # name (this manifest), the folder the unit writes (`--dir` and its
    # `CacheDirectory`, `nix/modules/index.nix`), the folder the brain reads
    # (`ronoctua_index::DIR` under the cache root) and the status file in it
    # (`ronoctua_index::status::FILE`, written by `serve`). When one moves,
    # the mark never shows and the page says "not running" over an index that
    # runs. And the unit is started by the brain ALONE: a `wantedBy` would
    # start a first walk of a home at login, on battery, in the middle of a
    # call — what the gate is for. Read as the machine gets it: the evaluated
    # configuration, not the file.
    search-index-starts-the-unit-it-reads =
      let
        facts = pkgs.writeText "search-index-unit.json" (
          builtins.toJSON {
            name = module.units.run.name;
            execStart = builtins.unsafeDiscardStringContext unit.serviceConfig.ExecStart;
            cacheDirectory = unit.serviceConfig.CacheDirectory or "";
            wantedBy = unit.wantedBy or [ ];
            nice = unit.serviceConfig.Nice or null;
          }
        );
      in
      mkCheck "search-index-starts-the-unit-it-reads" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the unit arrives as the evaluated configuration, JSON.
        # FILE-WIDE: two constants of the index's crate, comments off and
        # strings kept (`uncommented`) — the needles ARE strings.
        facts = json.load(open("${facts}"))
        index = pathlib.Path("${../../../ronoctua-index/src}")
        lib_rs = uncommented((index / "lib.rs").read_text())
        status_rs = uncommented((index / "status.rs").read_text())
        serve_rs = uncommented((index / "serve.rs").read_text())
        floor(len(lib_rs) + len(status_rs) + len(serve_rs), 3000, "characters of the index's crate")
        wrong = []

        leaf = re.search(r'pub const DIR: &str = "([^"]+)";', lib_rs)
        status_file = re.search(r'pub const FILE: &str = "([^"]+)";', status_rs)
        if not leaf or not status_file:
            sys.exit("ERROR: the index's folder or its status file is no longer a constant")
        folder = f"ronoctua/{leaf.group(1)}"
        if facts["cacheDirectory"] != folder:
            wrong.append(f"{facts['name']} writes the cache folder {facts['cacheDirectory']!r}, "
                         f"and the brain reads {folder!r}")
        if f"--dir %C/{folder}" not in facts["execStart"]:
            wrong.append(f"{facts['name']} is not told --dir %C/{folder}: {facts['execStart']}")
        if " serve " not in f" {facts['execStart']} ":
            wrong.append(f"{facts['name']} does not serve: {facts['execStart']}")
        served = must(body_of(serve_rs, "serve"), "the unit's `serve`")
        if "Phase::Walking" not in served or "Phase::Following" not in served:
            wrong.append("`serve` no longer says when it walks and when it follows")
        if facts["wantedBy"]:
            wrong.append(f"{facts['name']} is wanted by {facts['wantedBy']}: the brain starts it, "
                         "under the gate")
        if not (isinstance(facts["nice"], int) and facts["nice"] > 0):
            wrong.append(f"{facts['name']} is not niced ({facts['nice']})")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the brain starts {facts['name']}, which writes %C/{folder}/{status_file.group(1)}")
        END
      '';
  };
  slow = [ ];
}
