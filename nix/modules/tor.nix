# Tor — the client, its control socket, the configuration the view owns, and
# the ONE polkit rule this whole plan adds.
#
# WHAT IS PRIVILEGED HERE, AND HOW WIDE THE DOOR IS
# --------------------------------------------------
# Two things, and they are different.
#
# TALKING to a running Tor is not privileged at all: the daemon writes 32
# bytes to `/run/tor/control_auth_cookie`, and whoever can read them may send
# commands on the control socket. `CookieAuthFileGroupReadable` plus the owner
# in the `tor` group is the whole of it — no polkit action exists for this and
# none is invented. CONFIGURING Tor rides on that same cookie: the view writes
# a file and sends `SIGNAL RELOAD`, which is why nothing below had to widen.
#
# STARTING AND STOPPING the unit is privileged, and it is the only new polkit
# rule in this plan: `manage-units`, `tor.service`, the verbs `start` and
# `stop`, that one user. Everything else about the rule is a refusal.
# `tor-polkit-is-narrow` reads the installed rule and the crate's own
# `ronoctua_tunnel::tor::UNIT` and compares them — in BOTH directions, so a
# rule that grew and a rule that stopped matching the crate both fail.
#
# There is no polkit agent in this session (see the update's `module.nix`), so
# the rule answers `YES` and never `AUTH_SELF_KEEP`: what buys that width back
# is that the door is one millimetre wide.
#
# WHY IT DOES NOT START AT BOOT
# ------------------------------
# Because a person who has not asked for Tor is not using Tor, and a daemon
# that builds circuits nobody wants is battery and attention spent on nothing.
# The view starts it — `LEADER s n n` or `s n w`, the Tor section, which is
# drawn whether Tor is running or not — and `ronoctua.tor.autostart` is the
# flake's floor for whoever wants it from boot.
#
# That sentence was FALSE until this commit, and worth recording as a defect
# rather than quietly fixing: the section lived behind `if let Some(scene)`,
# and the control socket only answers when Tor is already up. So a stopped Tor
# drew no heading, no row and no key, the one switch that existed was
# hard-coded to STOP, and `Order::Tor(true)` was reachable from nowhere in the
# shell. The owner reported exactly that, and he was right.
#
# HOW THE CONFIGURATION IS WRITTEN WITHOUT BECOMING IMPERATIVE
# -------------------------------------------------------------
# `%include` points at a directory this system owns. The view writes one file
# into it and signals a reload; the file survives the reboot, the signal makes
# the running daemon agree with it. `SETCONF` was the obvious route and is
# wrong twice: it does not survive a restart, and `SAVECONF` writes into a
# torrc that lives in `/nix/store` and is read-only by construction
# (invariant 6). See `ronoctua_tunnel::tor::fragment`.
#
# AND THE FLAKE STILL WINS. Nix sorts attribute names, `%` sorts before
# letters, so `%include` is the FIRST line of the generated torrc — and Tor
# takes the last value it sees. Everything declared below therefore overrides
# anything the view writes, which is invariant 6 on the screen: those rows are
# drawn REFUSING, saying where the value comes from.
#
# THE DIRECTORY MUST EXIST BEFORE TOR STARTS. `%include` on a path that is not
# there is not ignored — Tor refuses to start, and the symptom is a unit that
# fails with a parse error about a file nobody remembers configuring. The
# tmpfiles rule and the `after` below are what make that impossible, and they
# are not tidiness.
#
# WHAT IS DELIBERATELY NOT HERE
# ------------------------------
# TRANSPARENT PROXYING of the whole system. `services.tor.client.transparent`
# would send every connection this machine makes through Tor, which is a
# decision about the machine and not about a window — and the failure mode is
# a laptop that quietly cannot reach anything on its own network. What this
# module offers instead is a SOCKS address the view NAMES, and which
# `network-tor-browser` points a browser at.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.tor;
  user = config.ronoctua.user;
  # THE ONE PLACE THE ADDRESS AND THE DIRECTORY ARE WRITTEN. `nix/rust.nix`
  # reads the same file and bakes both into the shell, which is what stops the
  # screen naming a port nothing listens on.
  facts = import ../tor.nix;
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

        This is the FLOOR, not the last word: the view keeps its own
        `tor.json`, which the footer reads when the session comes up. A
        preference that needed a rebuild to change would not be a preference.
      '';
    };

    socks = {
      address = lib.mkOption {
        type = lib.types.str;
        default = facts.socks.address;
        description = ''
          What Tor's SOCKS proxy binds to. The loopback, and changing it puts
          an open proxy on the network.
        '';
      };
      port = lib.mkOption {
        type = lib.types.port;
        default = facts.socks.port;
        description = ''
          Tor's SOCKS port.

          IT IS DECLARED HERE BECAUSE IT WAS NOT DECLARED ANYWHERE. The crate
          carried `127.0.0.1:9050` as a literal and this module set no
          `SOCKSPort` at all, so the two agreed because the nixpkgs default
          happened to be the same number — and nothing would have said
          otherwise the day either moved. `tor-socks-is-declared-once`
          compares them in both directions now.
        '';
      };
    };

    fragmentDirectory = lib.mkOption {
      type = lib.types.str;
      default = facts.fragmentDirectory;
      description = ''
        The directory whose torrc fragment the view owns. `%include`d by the
        generated torrc, group-writable by `tor` — the shape
        `/var/lib/ronoctua/keyd` already has for the keyboards.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.tor = {
      enable = true;
      client = {
        enable = true;
        socksListenAddress = {
          addr = cfg.socks.address;
          port = cfg.socks.port;
          # ONE CIRCUIT PER DESTINATION. It is the upstream default and it is
          # restated rather than inherited, because leaving it out would be
          # every site on the machine sharing one exit — which is the whole
          # of what Tor is for, undone by an omission.
          IsolateDestAddr = true;
        };
      };
      controlSocket.enable = true;
      settings = {
        # WHAT THE VIEW OWNS. Read FIRST, so everything below overrides it.
        "%include" = cfg.fragmentDirectory;

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

    # THE PLUGGABLE TRANSPORTS, so a censored network is reachable at all.
    #
    # They are PACKAGES here and lines in the fragment there: the nixpkgs
    # option for `ClientTransportPlugin` is a single submodule, and torrc takes
    # several — obfs4 and snowflake at once is the ordinary case, since which
    # one gets through is not known in advance. The view writes both lines and
    # reads these paths out of the environment, the way every other external
    # program in this tree is found.
    # `pkgs.obfs4` ships the `lyrebird` binary (the maintained obfs4 fork). On
    # nixos-unstable the top-level `pkgs.lyrebird` attribute was taken over by
    # an unrelated "Voice Changer" GUI app, so the transport is named `obfs4`
    # here now; the binary the view finds on PATH is still `lyrebird`.
    environment.systemPackages = [
      pkgs.obfs4
      pkgs.snowflake
    ];

    # THE DIRECTORY THE TORRC INCLUDES.
    #
    # `0775 root tor` and not the owner's own: `%include` is read by TOR, which
    # runs as `tor`, and written by the VIEW, which runs as the owner — who is
    # in that group four lines below. It is the arrangement `/etc/keyd` already
    # has, for the same reason.
    systemd.tmpfiles.rules = [
      "d ${cfg.fragmentDirectory} 0775 root tor - -"
    ];

    # NOT AT BOOT, unless asked. `wantedBy = []` is how a unit stays
    # installed and stopped; `systemctl start` is what the view sends.
    systemd.services.tor = {
      wantedBy = lib.mkIf (!cfg.autostart) (lib.mkForce [ ]);
      # AND IT STARTS AFTER THE DIRECTORY EXISTS. `%include` on a path that is
      # not there makes Tor REFUSE TO START — it is not skipped — and the
      # symptom is a unit that fails with a parse error about a file nobody
      # remembers configuring. On a first boot, with `autostart` on, the race
      # is real.
      after = [ "systemd-tmpfiles-setup.service" ];

      # AND THE DIRECTORY HAS TO BE INSIDE TOR'S ROOT, which is not the
      # machine's.
      #
      # nixpkgs runs this unit CHROOTED — `RootDirectory = /run/tor/root`,
      # with `BindReadOnlyPaths = [ "/etc" ] ++ [ storeDir ]` and nothing else.
      # A path under `/var/lib` does not exist in there at all, so `%include`
      # answered "Error reading included configuration file or directory" and
      # Tor refused to start. Every setting this view writes was therefore
      # INERT on a real machine: the fragment was written, the reload was sent,
      # and the daemon had never read the file.
      #
      # `RootDirectoryStartOnly = true` is why it was so quiet: `ExecStartPre`
      # runs OUTSIDE the chroot, so `tor --verify-config` printed
      # "Configuration was valid" a tenth of a second before the real process
      # failed on the same line. Measured in `tests/network-vm.nix`, whose
      # journal shows the two one after the other.
      #
      # READ-ONLY, because Tor only reads it: the view writes as the owner,
      # outside this namespace.
      serviceConfig.BindReadOnlyPaths = [ cfg.fragmentDirectory ];
    };

    users.users.${user}.extraGroups = [ "tor" ];

    # THE ONE NEW POLKIT RULE, and its whole width is on these six lines.
    #
    # It did NOT grow when Tor became configurable, and that is the point of
    # doing the configuration through the control socket: `systemctl reload`
    # would have needed a third verb, and `tor-polkit-is-narrow` refuses one —
    # `enable`, in particular, survives a reboot, which is a different kind of
    # power from starting something now.
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
