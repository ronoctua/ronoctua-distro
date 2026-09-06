# Power — the battery, the charger, the profile, and sleep.
#
# WHAT IS PRIVILEGED HERE: NOTHING OF OURS.
# -----------------------------------------
# This module writes no polkit rule, and that is the design rather than an
# omission — the same argument `nix/modules/disks.nix` makes about udisks2.
# logind ships its own actions and they already answer `yes` to an active
# session: `suspend`, `hibernate`, `power-off`, `reboot`, `inhibit-block-sleep`
# and `inhibit-handle-lid-switch` are `allow_active: yes` in systemd 260's
# `org.freedesktop.login1.policy`, and power-profiles-daemon's
# `switch-profile` is `yes` too. Read out of the built packages by
# `tests/power-vm.nix`, not assumed. A rule of ours could only WIDEN a door
# that is already the right width, and `power-adds-no-polkit-rule-of-its-own`
# keeps it this way in both directions.
#
# AND THERE IS NO AUTHENTICATION AGENT IN THIS SESSION.
# -----------------------------------------------------
# So every call the shell makes is non-interactive (`interactive = false` on
# every logind method; a property write on the profile daemon), and a refusal
# comes back at once with a name instead of a hang. `power-never-asks` is the
# check that turns that absence into a requirement.
#
# THE ONE THING THAT NEEDS A DOOR: THE CHARGE THRESHOLDS.
# --------------------------------------------------------
# `charge_control_{start,end}_threshold` are root-owned sysfs attributes, and
# UPower 1.90 offers only a switch between its configured pair and "full" —
# not a way to CHOOSE the pair from the screen, which is what the owner asked
# for. So a udev rule gives ONE group write access to exactly those two files
# on a battery that has them, and the shell writes the number. `GROUP=` and
# `MODE=` in a rule only reach device nodes; a sysfs attribute needs a `RUN`,
# and the programs it runs are store paths — a unit's `PATH` is minimal, the
# trap `CLAUDE.md` records. `TEST==` keeps the rule inert on a battery that has
# no threshold file, and `change` re-applies it after a resume.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.power;
  # The two attributes, named ONCE — the rule below and the check that reads
  # it both come from here.
  attributes = [
    "charge_control_start_threshold"
    "charge_control_end_threshold"
  ];
  grant =
    attribute:
    ''ACTION=="add|change", SUBSYSTEM=="power_supply", ATTR{type}=="Battery", TEST=="${attribute}", RUN+="${pkgs.coreutils}/bin/chgrp ${cfg.thresholdsGroup} /sys$devpath/${attribute}", RUN+="${pkgs.coreutils}/bin/chmod g+w /sys$devpath/${attribute}"'';
in
{
  options.ronoctua.power = {
    enable = lib.mkEnableOption "the battery, the charger, the power profile and sleep" // {
      default = true;
    };

    # THE DISTRO'S DEFAULT IS PERFORMANCE, by the owner's decision. It is the
    # floor under the person's own `power.json`: the shell applies the file's
    # profile when there is one, this one when there is none, and never
    # touches a profile somebody's application is holding.
    profile = lib.mkOption {
      type = lib.types.enum [
        "performance"
        "balanced"
        "power-saver"
      ];
      default = "performance";
      description = ''
        The power profile the machine comes up in when the person has not
        chosen one (`$XDG_CONFIG_HOME/ronoctua/power.json`). A machine whose
        daemon does not offer it gets the top profile it does offer, and the
        footer says so once.
      '';
    };

    # `users` and not a group of our own: this machine has one user by
    # design (`base.nix`), that user is in `users`, and a second group would
    # be one more line reaching into the identity `base.nix` owns.
    thresholdsGroup = lib.mkOption {
      type = lib.types.str;
      default = "users";
      description = "The group that may write the battery's charge thresholds.";
    };
  };

  config = lib.mkIf cfg.enable {
    services.upower = {
      enable = true;
      # THE LOW LINE IS THE DANGER LINE: 15% is where the modeline turns the
      # glyph red, so the daemon's `Low` — which raises the `battery-low`
      # notification — fires at the same number the screen changes colour at.
      percentageLow = 15;
      percentageCritical = 8;
      percentageAction = 4;
      # Hibernate, not the default `HybridSleep`: the layout keeps a swapfile
      # the size of RAM for exactly this (`nix/disko/default.nix`) and
      # `nix/modules/host.nix` names where to resume from. Not a "risky"
      # action in this nixpkgs — only `Suspend` and `Ignore` are — so no
      # `allowRiskyCriticalPowerAction` is needed.
      criticalPowerAction = "Hibernate";
    };

    services.power-profiles-daemon.enable = true;

    services.udev.extraRules = lib.concatMapStringsSep "\n" grant attributes + "\n";

    # THE FLOOR, in both places `RONOCTUA_THEME` is exported (`niri.nix`):
    # the footer is a user unit and inherits the manager's environment, not a
    # login shell's.
    systemd.user.services.ronoctua-footer.environment.RONOCTUA_POWER_PROFILE = cfg.profile;
    environment.sessionVariables.RONOCTUA_POWER_PROFILE = cfg.profile;
    # `powerprofilesctl` and `upower` reach the person's terminal through the
    # two service modules, which put their packages on the system themselves.
  };
}
