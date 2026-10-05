# The clock-sync module's own checks, in the shape every module's `checks.nix`
# has (`nix/brain-modules.nix`, `checkArguments`).
{
  mkCheck,
  pkgs,
  self,
  pyPrelude,
  ...
}:

let
  vm = self.nixosConfigurations.vm.config;
in
{
  checks = {
    # THERE IS A CLOCK TO KEEP, AND THE PERSON MAY KEEP IT.
    #
    # The watch keeps nothing itself: what its `keep` turns on is the
    # system's own time client (`SetNTP` on `timedated`, which starts
    # `systemd-timesyncd`). On a machine with no client installed it says
    # nothing at all — a line with no button that never goes away — so a
    # distro that stopped shipping one would switch this watch into silence
    # without a single test noticing. And the call is the PERSON's, from a
    # session with no polkit agent: without the rule that answers `YES` for
    # `set-ntp` it would fail on every press, far from its cause. This reads
    # both in the evaluated configuration. (How NARROW that rule is, is
    # `timedate-polkit-is-narrow`'s question; this one asks that it exists.)
    clock-sync-has-a-clock-to-keep =
      let
        facts = pkgs.writeText "clock-sync-machine.json" (
          builtins.toJSON {
            client = vm.services.timesyncd.enable;
            user = vm.ronoctua.user;
            rules = vm.security.polkit.extraConfig;
          }
        );
      in
      mkCheck "clock-sync-has-a-clock-to-keep" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the machine's facts arrive as the evaluated configuration,
        # JSON; the rules are the merged polkit JavaScript, split at
        # `polkit.addRule` and read for the one comparison asked about.
        facts = json.load(open("${facts}"))
        floor(len(facts["rules"]), 200, "characters of the merged polkit rules")
        wrong = []
        if not facts["client"]:
            wrong.append("the machine ships no network time client: the watch would say nothing, ever")
        granting = [
            r for r in re.split(r"polkit\.addRule", facts["rules"])[1:]
            if '"org.freedesktop.timedate1.set-ntp"' in r
            and re.search(r'subject\.user\s*==\s*"' + re.escape(facts["user"]) + '"', r)
            and "polkit.Result.YES" in r
        ]
        if not granting:
            wrong.append(f"no rule lets {facts['user']} switch the network time on without a question")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the machine has a time client, and {facts['user']} may switch it on")
        END
      '';
  };
  slow = [ ];
}
