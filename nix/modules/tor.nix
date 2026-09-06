# Tor — the client, its control socket, and the ONE polkit rule this whole
# plan adds.
#
# WHAT IS PRIVILEGED HERE, AND HOW WIDE THE DOOR IS
# --------------------------------------------------
# Two things, and they are different.
#
# TALKING to a running Tor is not privileged at all: the daemon writes 32
# bytes to `/run/tor/control_auth_cookie`, and whoever can read them may send
# commands on the control socket. `CookieAuthFileGroupReadable` plus the owner
# in the `tor` group is the whole of it — no polkit action exists for this and
# none is invented.
#
# STARTING AND STOPPING the unit is privileged, and it is the only new polkit
# rule in this plan: `manage-units`, `tor.service`, the verbs `start` and
# `stop`, that one user. Everything else about the rule is a refusal.
# `tor-polkit-is-narrow` reads the installed rule and this module's own
# `ronoctua_tunnel::tor::UNIT` and compares them — in BOTH directions, so a
# rule that grew and a rule that stopped matching the crate both fail.
#
# There is no polkit agent in this session (see `nix/modules/update.nix`), so
# the rule answers `YES` and never `AUTH_SELF_KEEP`: what buys that width back
# is that the door is one millimetre wide.
#
# WHY IT DOES NOT START AT BOOT
# ------------------------------
# Because a person who has not asked for Tor is not using Tor, and a daemon
# that builds circuits nobody wants is battery and attention spent on nothing.
# The view starts it (`LEADER s n n`, the Tor section), and `ronoctua.tor.
# autostart` is there for whoever disagrees.
#
# WHAT IS DELIBERATELY NOT HERE
# ------------------------------
# TRANSPARENT PROXYING of the whole system. `services.tor.client.transparent`
# would send every connection this machine makes through Tor, which is a
# decision about the machine and not about a window — and the failure mode is
# a laptop that quietly cannot reach anything on its own network. What this
# module offers instead is a SOCKS address the view names and a browser the
# view can start pointed at it.
{
  config,
  lib,
  ...
}:

let
  cfg = config.ronoctua.tor;
  user = config.ronoctua.user;
in
{
  options.ronoctua.tor = {
    enable = lib.mkEnableOption "Tor, its control socket and the view that drives it" // {
      default = true;
    };

    autostart = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Start Tor at boot. Off by the owner's decision: a person who has not
        asked for Tor is not using it, and the view starts it in a second.
        Turning this on costs a daemon building circuits from every boot.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.tor = {
      enable = true;
      client.enable = true;
      controlSocket.enable = true;
      settings = {
        # THE COOKIE, AND WHO MAY READ IT.
        #
        # `CookieAuthFile` IS NOT OPTIONAL HERE, and the default is the trap:
        # with no path Tor writes the cookie inside its DataDirectory, which
        # is `/var/lib/tor` at mode 0700 — so the group flag below opens a
        # file inside a directory nobody in the group may enter, and the view
        # meets "permission denied" on a machine where everything looks
        # configured. Measured in `tests/network-vm.nix`, which failed on
        # exactly this line.
        #
        # `/run/tor` is the runtime directory the unit already creates, at
        # 0710 with the `tor` group — so being in the group opens both the
        # directory and the file.
        CookieAuthentication = true;
        CookieAuthFile = "/run/tor/control_auth_cookie";
        CookieAuthFileGroupReadable = true;
      };
    };

    # NOT AT BOOT, unless asked. `wantedBy = []` is how a unit stays
    # installed and stopped; `systemctl start` is what the view sends.
    systemd.services.tor.wantedBy = lib.mkIf (!cfg.autostart) (lib.mkForce [ ]);

    users.users.${user}.extraGroups = [ "tor" ];

    # THE ONE NEW POLKIT RULE, and its whole width is on these six lines.
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            (action.lookup("verb") == "start" || action.lookup("verb") == "stop") &&
            action.lookup("unit") == "tor.service" &&
            subject.user == "${user}") {
          return polkit.Result.YES;
        }
      });
    '';

    # WHAT MUST SURVIVE THE WIPE IS NAMED IN ONE PLACE, and it is not here:
    # `nix/modules/persist.nix` holds the whole list, and a second one would
    # be a second place to change. This module's directory is in it.
  };
}
