# ── anchor: L25 ──
# THE FIRST BOOT FINISHES WHAT THE INSTALLATION CANNOT: the person's
# belongings, which need the repository's server. The installer leaves a
# record and the passphrase sealed in the TPM in `/persist/etc/ronoctua/
# first-boot` (`ronoctua_installer::first_boot`), and this unit runs the
# installation's own sequence against the machine once it reaches the
# network: the plain part copied out and the machine's choice signed in,
# the person's key pinned, their declarations applied, their home restored
# — or the repository made, when it was empty.
#
# Like a part, it never holds the boot and never fails it: a timer starts it
# after the boot and every hour after it ends, a network coming up starts it
# at once, and a step that failed is tried ever later by systemd. No network
# is no failure (the repository did not answer: exit 10), and neither is a
# passphrase the TPM did not give back (12, the person's to finish). Done,
# the record and the sealed passphrase are gone and the unit's condition is
# false for good.
{ tool }:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  dir = "/persist/etc/ronoctua/first-boot";
  unit = "ronoctua-first-boot.service";
in
{
  config = {
    systemd.services.ronoctua-first-boot = {
      description = "Finish the person's belongings once the machine reaches the network";
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
      unitConfig.ConditionPathExists = "${dir}/record.json";
      # Started by a timer and by the network: the default ceiling (five
      # starts in ten seconds) protects nothing here.
      startLimitIntervalSec = 0;
      # The person's two programs, which it runs by name, `setpriv` to run
      # them as the person, and `systemd-creds` to open the passphrase. The
      # two programs BY THEIR PATHS: a named program carries `passthru.bin`
      # (its name), which `makeBinPath` takes for a `bin` output — the PATH
      # got `ronoctua-belongings/bin`, relative, and nothing ran.
      path = [
        "${config.ronoctua.belongings.cli}"
        "${config.ronoctua.person.tool}"
        pkgs.util-linux
        config.systemd.package
      ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${tool}/bin/ronoctua-first-boot";
        # FAILED (1): tried again, ever later.
        Restart = "on-failure";
        RestartSec = "1min";
        RestartSteps = 8;
        RestartMaxDelaySec = "6h";
        # OFFLINE (10): the network coming up and the timer try again —
        # waiting is not failing. NEEDS-YOU (12): the TPM did not give the
        # passphrase back, and no retry would end differently — the person
        # restores from the belongings' view. Neither is a failed unit.
        SuccessExitStatus = [
          10
          12
        ];
        RestartPreventExitStatus = [
          10
          12
        ];
      };
    };

    # AFTER THE BOOT, never in it: a oneshot a target wants is waited for,
    # and a restore takes minutes.
    systemd.timers.ronoctua-first-boot = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnBootSec = "30s";
        # And an hour after each try ends: a repository that did not answer
        # with a network up is asked again.
        OnUnitInactiveSec = "1h";
        Unit = unit;
      };
    };

    # A NETWORK THAT CAME UP tries at once, whatever the backoff had left.
    networking.networkmanager.dispatcherScripts = [
      {
        type = "basic";
        source = pkgs.writeShellScript "ronoctua-first-boot-network" ''
          [ "$2" = up ] || exit 0
          [ -e ${dir}/record.json ] || exit 0
          exec ${config.systemd.package}/bin/systemctl start --no-block ${unit}
        '';
      }
    ];
  };
}
