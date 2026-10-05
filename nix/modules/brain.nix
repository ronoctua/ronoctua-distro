# The brain — the daemon that decides WHEN the machine does something to itself.
#
# WHY IT IS A UNIT AND NOT PART OF THE FOOTER
# -------------------------------------------
# The same argument `nix/modules/transfers.nix` makes about the queue, plus one
# that is stronger here. The footer restarts on failure in one second, which is
# right for a screen; a schedule that forgot what it had already done every time
# somebody's compositor hiccuped would run an update twice, or never. And a
# countdown the person can abort has to survive the thing drawing it.
#
# It also outlives the SESSION in intent, not only in code: the whole point is
# that the update happens at a good moment, and a good moment is usually one
# where nobody is looking at the screen.
#
# WHAT IT MAY NOT DO
# ------------------
# It runs nothing privileged itself. It asks systemd to start the units
# the update's `module.nix` declares, and that call is allowed by a polkit rule
# that names one verb, two units and one user — and RETURNS YES, because there
# is no polkit agent in this session and a rule that asked would be a rule that
# fails when nobody is at the laptop.
#
# ITS MODULES (brain.md §1)
# -------------------------
# Each folder under `crates/ronoctua-brain/modules/` brings its own NixOS half,
# `module.nix`, and it is imported here — gated by
# `ronoctua.brain.modules.<id>.enable`, which exists for every module and is on
# by default. The gate is put around the module's `config` by the loader
# (`nix/brain-modules.nix`), so a module turned off takes every unit it declares
# with it; and the daemon is told which ones the flake turned off
# (`RONOCTUA_BRAIN_OFF`), so their rows are drawn refusing with that reason
# rather than missing — a module that vanished from the page would read as one
# that was never there.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.brain;
  # THE LOADER, over the same folder every generator reads. Imported here and
  # not handed in by the flake because `imports` may depend on nothing a
  # module is given — and with the languages this NixOS module's siblings
  # read (`host.nix`, `base.nix`), from the one list.
  brainModules = import ../brain-modules.nix {
    inherit lib;
    languages = import ../../assets/i18n/languages.nix;
  };
  off = builtins.filter (id: !cfg.modules.${id}.enable) brainModules.ids;
in
{
  imports = brainModules.nixosModules;

  options.ronoctua.brain.enable =
    lib.mkEnableOption "the brain: what the machine does on its own, and when"
    // {
      default = true;
    };

  options.ronoctua.brain.modules = lib.genAttrs brainModules.ids (id: {
    enable = lib.mkEnableOption "the brain module `${id}`: ${brainModules.modules.${id}.what}" // {
      default = true;
    };
  });

  config = lib.mkIf cfg.enable {
    systemd.user.services.ronoctua-brain = {
      description = "ronoctua — the brain";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];

      # WHERE `systemctl` IS, and why it arrives here.
      #
      # `AGENTS.md`: a program spawned from a systemd unit is found by PATH, and
      # a unit's PATH is minimal — the failure then appears far from its cause.
      # The store path comes from the environment rather than from `option_env!`
      # for the reason `RONOCTUA_YT_DLP` records next door: a path compiled in is
      # a closure reference whether the feature is on or not.
      environment.RONOCTUA_SYSTEMCTL = "${pkgs.systemd}/bin/systemctl";
      # AND `journalctl`, for the one follower of the journal the core runs
      # when a module watches it (`ronoctua-journal`). The same package as
      # `systemctl`, so the closure gains nothing.
      environment.RONOCTUA_JOURNALCTL = "${pkgs.systemd}/bin/journalctl";
      # THE MODULES THE FLAKE TURNED OFF, by id, comma-separated; empty when
      # none is. Set even then, so an absent variable means "not started by
      # this module" (the nested loop, a test) and never "all on".
      environment.RONOCTUA_BRAIN_OFF = lib.concatStringsSep "," off;

      serviceConfig = {
        ExecStart = "${ronoctua.brain}/bin/ronoctua-brain";
        Restart = "on-failure";
        RestartSec = 1;

        # `session.slice` AND NOT `app.slice`, and this is not a preference.
        #
        # `nix/modules/memory.nix` arms `ManagedOOMMemoryPressure=kill` on
        # `app.slice` only, so anything under `session.slice` is outside the
        # candidate set BY CONSTRUCTION. A scheduler killed by the pressure it
        # exists to watch for would be the machine forgetting its own plans at
        # exactly the moment it is under load.
        Slice = "session.slice";
        OOMPolicy = "stop";
        TimeoutStopSec = 5;

        # It watches a clock and a report file; it compiles nothing and it
        # draws nothing. Idle scheduling costs it nothing and keeps it out of
        # the way of whatever the person is actually doing.
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
      };
    };
  };
}
