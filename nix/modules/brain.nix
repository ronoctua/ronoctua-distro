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
# `nix/modules/update.nix` declares, and that call is allowed by a polkit rule
# that names one verb, two units and one user — and RETURNS YES, because there
# is no polkit agent in this session and a rule that asked would be a rule that
# fails when nobody is at the laptop.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.brain;
in
{
  options.ronoctua.brain.enable =
    lib.mkEnableOption "the brain: what the machine does on its own, and when"
    // {
      default = true;
    };

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
