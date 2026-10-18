# The video-metrics module's own checks, in the shape every module's
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
  scan = vm.systemd.user.services.${service module.units.run};
in
{
  checks = {
    # THE BRAIN READS WHERE THE SCAN WRITES, and the scan is idle.
    #
    # The runner watches `<REPORTS_LEAF>/<id>.json` under the runtime
    # directory and starts the unit the manifest names; the unit is
    # `nix/modules/video-metrics.nix`'s, and its `--report` is built from the
    # folder's name. Three places one name has to agree, and when one of them
    # moves the scan runs out its patience and comes back `runaway` while the
    # modeline says it is still scanning. This reads the unit as the machine
    # gets it — the evaluated configuration, not the file: its report, ended
    # by the unit after a killed scan, the directory made first, a oneshot at
    # idle priority, and yt-dlp by store path where the media is.
    video-metrics-scan-writes-where-the-brain-reads =
      let
        facts = pkgs.writeText "video-metrics-unit.json" (
          builtins.toJSON {
            inherit (module) id;
            name = module.units.run.name;
            # READ AS WORDS, WITHOUT ITS STORE PATHS: the command line names
            # the scanner's binary, and a file quoting it would make this
            # check build the workspace to compare two strings.
            execStart = builtins.unsafeDiscardStringContext scan.serviceConfig.ExecStart;
            execStopPost = builtins.unsafeDiscardStringContext (scan.serviceConfig.ExecStopPost or "");
            type = scan.serviceConfig.Type or "";
            nice = scan.serviceConfig.Nice or null;
            io = scan.serviceConfig.IOSchedulingClass or "";
            runtimeDirectory = scan.serviceConfig.RuntimeDirectory or "";
            ytDlp = builtins.unsafeDiscardStringContext (scan.environment.RONOCTUA_YT_DLP or "");
            media = vm.ronoctua.media.enable;
          }
        );
      in
      mkCheck "video-metrics-scan-writes-where-the-brain-reads" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the unit arrives as the evaluated configuration, JSON.
        # FILE-WIDE: from the runner, one constant and one function's body
        # (`body_of`), comments off and strings kept (`uncommented`) — the
        # needles ARE strings.
        facts = json.load(open("${facts}"))
        runner = uncommented(pathlib.Path("${../../../ronoctua-brain-daemon/src/runner.rs}").read_text())
        floor(len(runner), 1000, "characters of runner.rs")
        wrong = []

        leaf = re.search(r'pub const REPORTS_LEAF: &str = "([^"]+)";', runner)
        if not leaf:
            sys.exit("ERROR: `REPORTS_LEAF` went away; it is the name the scan has to agree with")
        leaf = leaf.group(1)
        built = must(body_of(runner, "report_in"), "the runner's `report_in`")
        if '"{task}.json"' not in built:
            wrong.append("the runner no longer names a report `{task}.json`")

        report = f"{facts['id']}.json"
        start = facts["execStart"]
        if f"--report %t/{leaf}/{report}" not in start:
            wrong.append(f"{facts['name']} does not write %t/{leaf}/{report}, which the brain watches")
        if " scan " not in f" {start} ":
            wrong.append(f"{facts['name']} does not run a scan: {start}")
        # A SCAN KILLED BEFORE ITS END — stopped, the memory killer — is ended
        # by the unit, on the same report, or the brain waits out its patience
        # with the modeline saying "scanning".
        after = facts["execStopPost"]
        if " scan-stopped " not in f" {after} " or f"--report %t/{leaf}/{report}" not in after:
            wrong.append(f"{facts['name']} does not end %t/{leaf}/{report} after a killed scan: {after!r}")
        if facts["runtimeDirectory"] != leaf:
            wrong.append(f"{facts['name']} does not make %t/{leaf} before it writes there")
        if facts["type"] != "oneshot":
            wrong.append(f"{facts['name']} is `{facts['type']}`, and the brain watches a oneshot end")
        if not (isinstance(facts["nice"], int) and facts["nice"] > 0):
            wrong.append(f"{facts['name']} is not niced ({facts['nice']})")
        if facts["io"] != "idle":
            wrong.append(f"{facts['name']} does not yield the disk (IOSchedulingClass={facts['io']})")
        if facts["media"] and not re.fullmatch(r"/nix/store/[^/]+/bin/yt-dlp", facts["ytDlp"]):
            wrong.append(f"{facts['name']} is not handed yt-dlp by store path: {facts['ytDlp']!r}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the scan writes %t/{leaf}/{report}, makes the directory first, and runs idle")
        END
      '';
  };
  slow = [ ];
}
