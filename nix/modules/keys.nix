# INDEPENDENT KEYBOARDS — `ronoctua-keys`, keyd's replacement in that mode.
#
# Together mode is keyd (`keyboard.nix`): one virtual keyboard for all. The
# other mode gives every keyboard unit a virtual twin of its own, so the
# compositor can put each on the seat of its cursor, and that needs a daemon
# that grabs each unit and re-emits it — see `crates/ronoctua-keys`.
#
# THE TWO NEVER RUN TOGETHER: both grab the same keyboards, and the second
# grab fails. `conflicts` says so both ways, so starting one stops the other.
#
# NOT STARTED AT BOOT. keyd is the machine's answer until somebody chooses
# otherwise; the choice is the person's (`keyboards.json`), and the footer
# applies it once per session with the polkit rule below — the Tor pattern: a
# preference of ours, never an `enable` that outlives the choice.
#
# AND A FAILURE GIVES THE KEYBOARD BACK. A daemon that dies closes its
# descriptors, the kernel drops the grabs, and the keyboards reach the
# compositor raw — typing, with no leader. `Restart` brings it back; one that
# will not come back starts keyd (`onFailure`), which is the leader again.
{
  config,
  lib,
  ronoctua,
  ...
}:
let
  directory = "/var/lib/ronoctua/keys";
in
{
  hardware.uinput.enable = true;

  systemd.services.ronoctua-keys = {
    description = "Independent keyboards: one virtual twin per keyboard";
    conflicts = [ "keyd.service" ];
    onFailure = [ "keyd.service" ];
    environment = {
      RONOCTUA_KEYS_USER = config.ronoctua.user;
      RONOCTUA_KEYS_DIRECTORY = directory;
    };
    serviceConfig = {
      ExecStart = "${ronoctua.keys}/bin/ronoctua-keys";
      Restart = "on-failure";
      RestartSec = "1s";
      RuntimeDirectory = "ronoctua-keys";
      RuntimeDirectoryMode = "0755";
      # WHAT IT MAY TOUCH, and nothing else: the input nodes, uinput, and the
      # directory the shell renders its rules into.
      DevicePolicy = "closed";
      DeviceAllow = [
        "char-input rw"
        "/dev/uinput rw"
      ];
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadOnlyPaths = [ directory ];
      PrivateNetwork = true;
      NoNewPrivileges = true;
    };
  };

  systemd.services.keyd.conflicts = [ "ronoctua-keys.service" ];

  # The directory the shell renders the daemon's rules into — the keyd
  # arrangement: the session's user writes it, the root daemon only reads.
  systemd.tmpfiles.rules = [ "d ${directory} 0755 ${config.ronoctua.user} users -" ];

  # WHO MAY SWITCH MODES, AND ONLY THAT: `start` and `stop` of exactly these two
  # units, for the session's user. The condition is PARENTHESISED — without
  # the parentheses `&&` binds tighter than `||`, and the second unit would be
  # open to every subject (`AGENTS.md`, the rescue rule).
  security.polkit.extraConfig = ''
    polkit.addRule(function(action, subject) {
      if (action.id == "org.freedesktop.systemd1.manage-units" &&
          subject.user == "${config.ronoctua.user}" &&
          (action.lookup("unit") == "keyd.service" ||
           action.lookup("unit") == "ronoctua-keys.service") &&
          (action.lookup("verb") == "start" ||
           action.lookup("verb") == "stop")) {
        return polkit.Result.YES;
      }
    });
  '';
}
