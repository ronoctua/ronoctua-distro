# Tailscale — the tailnet, and the one flag that makes it usable without root.
#
# WHAT IS PRIVILEGED HERE: NOTHING, AND NOT EVEN A GROUP
# -------------------------------------------------------
# `tailscaled` owns its socket as root and decides for itself who may drive
# it: `--operator=<user>` is what tells the daemon that this person's
# unprivileged calls to the local API are to be honoured. That is upstream's
# own mechanism, it is not a group and not a polkit action, and this module
# writes exactly that one flag.
#
# So `ronoctua-tunnel` talks to `/run/tailscale/tailscaled.sock` with no
# privilege at all, and a machine where the flag is missing meets a 403 that
# the view reports as the daemon refusing — which is honest.
#
# WHAT THIS MODULE DOES NOT DO
# -----------------------------
# It does not log in. A tailnet is somebody's account, and the login is an
# address the daemon hands back for a browser to open — `LEADER s n n`, the
# tailnet section. A machine that has never logged in has `BackendState =
# NeedsLogin`, draws no modeline mark, and costs one socket read every ten
# seconds.
#
# It does not advertise routes or run as an exit node. Both are decisions
# about what this machine offers OTHERS, and neither belongs in a default.
# `useRoutingFeatures = "client"` is what lets this machine USE somebody
# else's exit node, which is the half the view offers.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.tailscale;
  user = config.ronoctua.user;
in
{
  options.ronoctua.tailscale = {
    enable = lib.mkEnableOption "the tailnet and the view that drives it" // {
      default = true;
    };

    trustInterface = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Treat `tailscale0` as a trusted interface, so that machines on the
        tailnet reach this one's services the way machines on the local
        network do. Turning it off leaves the tailnet working for outgoing
        connections and closes it for incoming ones.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.tailscale = {
      enable = true;
      # THE ONE FLAG THAT MATTERS: without it the local API is root's and
      # every call from the view comes back with a refusal.
      #
      # AND IT IS NOT ENOUGH ON ITS OWN. The nixpkgs module applies
      # `extraSetFlags` from `tailscaled-autoconnect.service`, which only runs
      # when an `authKeyFile` is configured — and this machine logs in by
      # hand, from the view. Measured in `tests/network-vm.nix`: with only
      # this line the socket stays root's and the crate reports the daemon
      # ABSENT. The unit below is what actually sets it.
      extraSetFlags = [ "--operator=${user}" ];
      openFirewall = true;
      useRoutingFeatures = "client";
    };

    # THE OPERATOR, SET ON EVERY BOOT, whether or not anybody ever logs in.
    #
    # `tailscale set` edits a preference and needs no tailnet: it works on a
    # daemon that has never joined one, which is exactly the machine this is
    # for. The retry is because the daemon writes its socket a moment after
    # the unit starts, and a `set` that lands first fails with a message
    # about a socket rather than about a preference.
    systemd.services.ronoctua-tailscale-operator = {
      description = "Lets ${user} drive tailscaled without root";
      after = [ "tailscaled.service" ];
      wants = [ "tailscaled.service" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.writeShellScript "ronoctua-tailscale-operator" ''
          set -u
          tailscale=${config.services.tailscale.package}/bin/tailscale

          # ALREADY THE OPERATOR: nothing to do, and nothing to restart. This
          # is every boot after the first, and it is why this unit does not
          # bounce the daemon daily.
          #
          # THE QUESTION IS ASKED AS THE OWNER, and it is the real one: can
          # this person drive the daemon? The preference itself is NOT in
          # `tailscale debug prefs` — measured in `tests/network-vm.nix`,
          # whose whole prefs dump came back without an `OperatorUser` field
          # while the daemon's own journal showed it being set. A guard
          # written against that field never matched, and the unit restarted
          # tailscaled on every single boot.
          asked=${pkgs.util-linux}/bin/runuser
          if $asked -u ${user} -- $tailscale debug prefs > /dev/null 2>&1; then
            exit 0
          fi

          took=no
          for _ in $(${pkgs.coreutils}/bin/seq 1 30); do
            if $tailscale set --operator=${user}; then
              took=yes
              break
            fi
            ${pkgs.coreutils}/bin/sleep 1
          done
          if [ "$took" != yes ]; then
            echo "tailscaled never took the operator; the view will meet a refusal" >&2
            exit 1
          fi

          # AND THE DAEMON READS THE OPERATOR WHEN IT STARTS.
          #
          # The socket is 0666 either way — anybody may CONNECT — and the
          # check is inside: `tailscaled` compares the caller's uid against
          # the operator it read at startup, so a preference set afterwards
          # is not the one it is comparing against. Measured in
          # `tests/network-vm.nix`: the unit finished, the preference was
          # right, the socket was writable, and every call still came back
          # refused, which the crate reports as a daemon that is not there.
          #
          # ONE RESTART, and only when the owner still cannot drive it.
          if $asked -u ${user} -- $tailscale debug prefs > /dev/null 2>&1; then
            exit 0
          fi
          echo "the operator changed to ${user}; restarting the daemon once" >&2
          ${pkgs.systemd}/bin/systemctl restart tailscaled.service
          for _ in $(${pkgs.coreutils}/bin/seq 1 30); do
            $asked -u ${user} -- $tailscale debug prefs > /dev/null 2>&1 && exit 0
            ${pkgs.coreutils}/bin/sleep 1
          done
          echo "${user} still cannot drive tailscaled; the view will meet a refusal" >&2
          exit 1
        ''}";
      };
    };

    networking.firewall.trustedInterfaces = lib.mkIf cfg.trustInterface [ "tailscale0" ];

    # WHAT MUST SURVIVE THE WIPE IS NAMED IN ONE PLACE, and it is not here:
    # `nix/modules/persist.nix` holds the whole list, and a second one would
    # be a second place to change. This module's directory is in it.
  };
}
