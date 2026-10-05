# The emacs-daemon module's own checks, in the shape every module's
# `checks.nix` has (`nix/brain-modules.nix`, `checkArguments`).
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
  unit = module.units.watched.name;
  service = lib.removeSuffix ".service" unit;
in
{
  checks = {
    # THE BUTTON STARTS A UNIT THE MACHINE HAS, AND ONE THAT ASKS NOBODY.
    #
    # The notice's one action is `systemctl --user start <unit>`, the unit
    # the manifest names — and that unit is not this module's: the editor
    # declares it (`services.emacs.enable`, `nix/modules/emacs.nix`). A name
    # that drifted from it is a button that answers "no such unit" on every
    # machine; a unit that became a SYSTEM one is a button that needs polkit
    # and an agent this session does not run — nothing the brain offers by
    # itself may put a question in front of somebody. So this reads the
    # evaluated configuration: the unit exists, it is the person's, and the
    # editor the watch looks for is the one the distro installs.
    emacs-daemon-starts-a-unit-the-person-has =
      let
        facts = pkgs.writeText "emacs-daemon-unit.json" (
          builtins.toJSON {
            inherit unit;
            user = vm.systemd.user.services ? ${service};
            system = vm.systemd.services ? ${service};
            editor = vm.services.emacs.enable;
            execStart = builtins.unsafeDiscardStringContext (
              vm.systemd.user.services.${service}.serviceConfig.ExecStart or ""
            );
          }
        );
      in
      mkCheck "emacs-daemon-starts-a-unit-the-person-has" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the unit's facts arrive as the evaluated configuration, JSON.
        facts = json.load(open("${facts}"))
        unit = facts["unit"]
        wrong = []
        if not unit.endswith(".service"):
            wrong.append(f"the manifest names `{unit}`, which is not a service")
        if not facts["editor"]:
            wrong.append("the distro no longer installs the Emacs daemon the watch looks for")
        if not facts["user"]:
            wrong.append(f"`{unit}` is not a unit of the person's: the button would answer `no such unit`")
        if facts["system"]:
            wrong.append(f"`{unit}` is also a SYSTEM unit: starting it would ask polkit, and nobody is asked")
        # AND IT IS THE DAEMON, and not an editor window: the watch's whole
        # question is whether the server is up.
        if "daemon" not in facts["execStart"]:
            wrong.append(f"`{unit}` does not run a daemon: {facts['execStart']!r}")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the watch's button starts {unit}, the person's own Emacs daemon")
        END
      '';
  };
  slow = [ ];
}
