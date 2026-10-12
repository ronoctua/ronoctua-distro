# The security centre's foundation — the sudo door, the passwordless window,
# the person's firewall table, fingerprints and the firmware's facts.
#
# WHAT IS THE PERSON'S AND WHAT IS THE DISTRO'S
# ---------------------------------------------
# Invariant 6 has an approved exception here, in the mould of "the person's
# programs" (owner, 2026-09-30, amended 2026-10-11): the firewall RULES the
# person makes live in `/var/lib/ronoctua-security/firewall.json` and are
# applied by a declared unit into a table of our own (`inet ronoctua`); the
# passwordless-sudo window is one line in root's persisted
# `/var/lib/ronoctua-sudo`, which survives a reboot and ends at its end or
# when the person ends it. Everything that makes those two possible — the
# include, the units, the marks the declared firewall obeys, the one polkit
# rule — is declared below. Nothing the person does reaches `/etc`.
#
# THE SUDO DOOR
# -------------
# There is no polkit agent in the session. A gesture that needs root goes
# through `ronoctua_security::authority`: `sudo -n` first, then either the
# typed password piped to `sudo -S -k -p "" -- <helper> <verb>`, or a
# conversation with sudo on a terminal the door owns, where a security key
# (`nix/modules/yubikey.nix`) or a finger (below) may answer before the
# password — the door hears only its own sentinels there. The helper is
# `ronoctua-security-root`, extracted from the workspace so the units below
# that run it as root do not carry the desktop; the session learns its path
# from `RONOCTUA_SECURITY_ROOT`.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.security;
  user = config.ronoctua.user;
  group = config.users.users.${user}.group;
  helper = "${cfg.helper}/bin/ronoctua-security-root";

  # THE MARKS, bits 28–30. The person's table sets them; the declared table
  # obeys them. They miss Tailscale's `0xff0000` and netavark's `0x2000`, and
  # `firewall-marks-are-free` holds these three against the Rust constants in
  # `crates/ronoctua-security/src/firewall.rs` and against both neighbours —
  # a collision between two marks is silent, which is why it is checked.
  markAllowed = "0x10000000";
  markChatty = "0x40000000";
  # Off OR chatty: the quiet log is the one that runs when neither is set.
  markLogBits = "0x60000000";
  refusedPrefix = "ronoctua-refused: ";

  # THE LINK HOOK. NetworkManager runs it as root on every event, with the
  # interface in `$1`, the event in `$2` and the connection in the
  # environment. It hands the helper a UNIT verb — never one of the door's —
  # and the helper checks both names before either is written anywhere.
  #
  # Registered as `pre-up` AND `basic`: `pre-up` makes a public network's
  # table be in place before the interface carries traffic; `basic` is where
  # `down` arrives.
  #
  # A connection NetworkManager did not activate itself (`CONNECTION_EXTERNAL`)
  # is not a network the person joined — loopback, a container's or a VM's
  # bridge, the Android container's, the tailnet — and it never becomes
  # public: born public, a bridge would have its DHCP and DNS dropped.
  #
  # The rest the helper asks of NetworkManager itself, through
  # `ronoctua-network` (never its files): whether this machine SERVES the
  # connection's network (a hotspot: never public, and its clients get the
  # machine's DHCP and DNS), and — the first time it answers — which
  # connections existed, which all become home networks.
  linkHook = pkgs.writeShellScript "ronoctua-firewall-link" ''
    if [ "''${CONNECTION_EXTERNAL:-0}" = 1 ]; then
      exit 0
    fi
    case "$2" in
      pre-up | up) exec ${helper} firewall-apply --link "$1" "$CONNECTION_UUID" up ;;
      down) exec ${helper} firewall-apply --link "$1" "$CONNECTION_UUID" down ;;
    esac
  '';

  # What the units that change the table may touch, and nothing else of the
  # file system. The leading `-`: a path that is not there yet does not stop
  # the unit before it runs (226/NAMESPACE) — tmpfiles makes both.
  tableSandbox = {
    ProtectSystem = "strict";
    ReadWritePaths = [
      "-/var/lib/ronoctua-security"
      "-/run/ronoctua-security"
    ];
    ProtectHome = true;
    PrivateTmp = true;
    NoNewPrivileges = true;
    RestrictAddressFamilies = [
      "AF_NETLINK"
      "AF_UNIX"
    ];
  };
in
{
  options.ronoctua.security = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The security centre's foundation: the sudo door, the window, the person's firewall table, fingerprints and fwupd (read only).";
    };

    helper = lib.mkOption {
      type = lib.types.package;
      description = ''
        The extracted root helper (`rust.securityRoot`), with
        `bin/ronoctua-security-root`. Set by the flake next to this module's
        import; a test sets it the same way.
      '';
    };
  };

  # FINGERPRINTS ANSWER NOTHING BY DEFAULT.
  #
  # nixpkgs turns `fprintAuth` on in EVERY PAM service the moment fprintd is
  # enabled — the login, the screen of the rescue console, polkit, `su`. This
  # extends the option so each service is born with it off, and only `sudo`
  # below turns it back on, behind two gates. Declared outside `config`
  # because an option cannot be conditional; with the module off, fprintd is
  # off too and the default changes nothing.
  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule {
        config.fprintAuth = lib.mkDefault false;
      }
    );
  };

  config = lib.mkIf cfg.enable {
    # -----------------------------------------------------------------
    # The firewall: nftables, and a table of the person's
    # -----------------------------------------------------------------
    #
    # nftables and not iptables: the person's table is replaced with one
    # atomic `nft -f`; a reload of the declared ruleset deletes only the
    # declared tables (`flushRuleset` is false with tables in use), so ours
    # survives a `switch`; and Podman's netavark follows the backend.
    networking.nftables.enable = true;

    # AND A RELOAD MUST NOT FLUSH THE WHOLE RULESET. With `flushRuleset` off
    # (the default for this stateVersion while nobody sets `ruleset`), a
    # reload deletes only the declared tables and ours survives a `switch`.
    # One module setting `networking.nftables.ruleset` flips that default,
    # and every reload would then take the person's table away until the
    # next boot — `ronoctua-firewall` is `partOf` nftables, and `PartOf=`
    # passes stops and restarts on, never a reload. So it is refused here.
    assertions = [
      {
        assertion = !config.networking.nftables.flushRuleset;
        message = ''
          networking.nftables.flushRuleset is on: every nftables reload would flush
          the person's firewall table (inet ronoctua), and nothing applies it again
          until the next boot. Declare tables with networking.nftables.tables
          instead of networking.nftables.ruleset (nix/modules/security.nix).
        '';
      }
    ];

    # THE DECLARED TABLE OBEYS THE MARKS. `extraInputRules` is the tail of
    # `input-allow`, reached by a NEW or untracked packet the declared ports
    # did not accept. First: what the person allowed. Last: what is about to be
    # refused, logged with a rate — upstream's `logRefusedConnections` has
    # none, and stays off.
    networking.firewall.extraInputRules = lib.mkMerge [
      (lib.mkBefore ''
        meta mark & ${markAllowed} == ${markAllowed} accept comment "ronoctua: allowed by the person"
      '')
      (lib.mkAfter ''
        meta mark & ${markLogBits} == 0 tcp flags syn / fin,syn,rst,ack limit rate 6/minute burst 12 packets log level info prefix "${refusedPrefix}"
        meta mark & ${markLogBits} == ${markChatty} tcp flags syn / fin,syn,rst,ack limit rate 60/minute burst 120 packets log level info prefix "${refusedPrefix}"
      '')
    ];
    networking.firewall.logRefusedConnections = false;

    # THE TABLE AT BOOT, and again whenever nftables is restarted. A oneshot
    # that stays active, so a `switch` does not run it again for nothing.
    # `-`: a stored file that is not valid must not make `switch` report a
    # failure it did not have — the helper leaves the kernel's table as it was,
    # says so in `firewall.status` and in the journal at `err`.
    systemd.services.ronoctua-firewall = {
      description = "Applies the person's firewall table";
      after = [ "nftables.service" ];
      partOf = [ "nftables.service" ];
      wantedBy = [ "nftables.service" ];
      serviceConfig = tableSandbox // {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "-${helper} firewall-apply";
      };
    };

    # SHIELDS: the one change a person may make with no password, because it
    # only ever takes away. The unit runs one verb, and the rule below lets the
    # person start this unit and nothing else.
    systemd.services.ronoctua-firewall-shields = {
      description = "Raises the shields: every new incoming connection is refused";
      after = [ "ronoctua-firewall.service" ];
      serviceConfig = tableSandbox // {
        Type = "oneshot";
        ExecStart = "${helper} shields-up";
      };
    };

    # THE ONLY NEW POLKIT RULE: `YES`, `start`, one unit, one user. There is no
    # agent to ask, so `YES` is the only answer that works; what makes it
    # acceptable is that the door is one unit wide and tightens only.
    # `security-polkit-is-narrow` holds every word of it.
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (
          action.id == "org.freedesktop.systemd1.manage-units" &&
          action.lookup("verb") == "start" &&
          action.lookup("unit") == "ronoctua-firewall-shields.service" &&
          subject.user == "${user}"
        ) {
          return polkit.Result.YES;
        }
      });
    '';

    # THE MACHINE'S OWN VIRTUAL NETWORKS are not the person's table's
    # business: the interfaces the declared firewall trusts (the Android
    # container's bridge, …), read by the helper. `tailscale0` stays out — the
    # tailnet is a network other machines arrive from, and the shields cover
    # it.
    environment.etc."ronoctua-security/local-interfaces".text = lib.concatMapStrings (i: i + "\n") (
      lib.filter (i: i != "tailscale0" && i != "lo") config.networking.firewall.trustedInterfaces
    );

    # PROFILES FOLLOW EACH NETWORK BY EVENT.
    networking.networkmanager.dispatcherScripts = lib.mkIf config.networking.networkmanager.enable [
      {
        source = linkHook;
        type = "pre-up";
      }
      {
        source = linkHook;
        type = "basic";
      }
    ];

    # -----------------------------------------------------------------
    # The passwordless window
    # -----------------------------------------------------------------
    #
    # AFTER the wheel rule (`extraConfig` is written last), so the window's
    # `NOPASSWD` wins while it lasts. sudo skips a name with a dot in it,
    # which is what makes the helper's `.relax` draft invisible until `visudo`
    # has passed it. `sudoers-includes-one-dir` holds this line.
    #
    # `!pam_silent` FIRST: since 1.9.16 sudo hands PAM `PAM_SILENT` by
    # default, and `pam_echo` then says nothing — the line that tells the
    # sudo door (and a person in a terminal) that a finger is about to be
    # asked would never be said. It brings back sudo's behaviour of before:
    # a module's messages reach whoever is authenticating.
    # `sudo-door-reads-its-own-sentinels` holds it.
    security.sudo.extraConfig = ''
      Defaults !pam_silent
      @includedir /var/lib/ronoctua-sudo
    '';

    # THE WINDOW SURVIVES A REBOOT, and ends only at its end or by the
    # person's gesture (owner, 2026-10-11) — so nothing closes it before the
    # machine sleeps any more. What a reboot does lose is the transient timer
    # that takes the line away at the end: at every boot this looks again
    # (`relax::boot`) — an expired or orphaned line goes (sudo's own
    # `NOTAFTER` refused it already), a live timed one gets its timer back,
    # one with no limit stays. `RemainAfterExit`, so a switch does not run it
    # again; `-`, so a failure here never fails a switch (the `NOTAFTER`
    # still holds). `sudo-window-survives-a-reboot` holds all of it.
    systemd.services.ronoctua-sudo-window = {
      description = "Ends an expired passwordless sudo window, and arms the end of a live one";
      wantedBy = [ "multi-user.target" ];
      unitConfig.RequiresMountsFor = [
        "/var/lib/ronoctua-sudo"
        "/var/lib/ronoctua-security"
      ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "-${helper} relax-boot";
        ProtectSystem = "strict";
        ReadWritePaths = [
          "-/var/lib/ronoctua-sudo"
          "-/var/lib/ronoctua-security"
        ];
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
        # The manager's socket alone: `systemctl` and `systemd-run` for the
        # timer.
        RestrictAddressFamilies = [ "AF_UNIX" ];
      };
    };

    # WHERE THINGS LIVE. The window's folder is root's and persisted
    # (`nix/modules/persist.nix`), so it outlives a reboot; nobody else may
    # write in it — sudo reads it as sudoers. `/run` keeps the firewall's
    # status. The staging folder is the person's and private — the
    # files-root pattern: nobody else may leave a wish for a program that
    # runs as root.
    systemd.tmpfiles.rules = [
      "d /var/lib/ronoctua-sudo 0755 root root -"
      "d /run/ronoctua-security 0755 root root -"
      "d /run/ronoctua-security/staged 0700 ${user} ${group} -"
      "d /var/lib/ronoctua-security 0755 root root -"
    ];

    # The shell's door finds the helper here: a variable and not a compiled
    # path, because the helper is extracted FROM the workspace the shell is
    # built in. BOTH PLACES, for `media.nix`'s reason: `sessionVariables`
    # reaches a login shell and what the compositor starts; the footer is a
    # user unit, which inherits the manager's environment and not the shell's.
    environment.sessionVariables.RONOCTUA_SECURITY_ROOT = helper;
    systemd.user.services.ronoctua-footer.environment.RONOCTUA_SECURITY_ROOT = helper;

    # -----------------------------------------------------------------
    # Fingerprints
    # -----------------------------------------------------------------
    #
    # fprintd is D-Bus activated: a machine with no reader never runs it.
    services.fprintd.enable = true;

    # ONLY SUDO, AND ONLY IN A TERMINAL, AND ONLY FOR A LISTED USER.
    #
    # The first gate is the list `FingerSudo` writes (on and off without a
    # rebuild); a missing file is an error, and `onerr=fail` makes an error a
    # "no". The second keeps fingers to terminals: the door's password-only
    # `sudo -S` has no tty, so fprintd never makes it wait there; its
    # conversation gives sudo a terminal of its own, where a finger answers.
    # Then the CUE: one `pam_echo` line saying, in the words the door listens
    # for (`ronoctua_security::authority::FINGER_CUE`), that a finger is
    # about to be asked — fprintd's own words are translated and never read.
    # Each gate jumps over whatever is between it and `pam_fprintd`, cue
    # included — so the three sit DIRECTLY before it, one, two and three
    # below its order (rules are a hundred apart).
    # `pam-fingerprint-is-gated` reads every PAM service to hold all of it.
    security.pam.services.sudo = {
      fprintAuth = true;
      rules.auth = {
        ronoctua-finger-listed = {
          order = config.security.pam.services.sudo.rules.auth.fprintd.order - 3;
          control = "[success=ignore default=3]";
          modulePath = "${config.security.pam.package}/lib/security/pam_listfile.so";
          args = [
            "item=user"
            "sense=allow"
            "file=/var/lib/ronoctua-security/fingerprint-sudo"
            "onerr=fail"
          ];
        };
        ronoctua-finger-tty = {
          order = config.security.pam.services.sudo.rules.auth.fprintd.order - 2;
          control = "[success=ignore default=2]";
          modulePath = "${config.security.pam.package}/lib/security/pam_succeed_if.so";
          args = [
            "quiet"
            "tty"
            "=~"
            "/dev/*"
          ];
        };
        ronoctua-finger-cue = {
          order = config.security.pam.services.sudo.rules.auth.fprintd.order - 1;
          control = "optional";
          modulePath = "${config.security.pam.package}/lib/security/pam_echo.so";
          args = [ "A fingerprint may answer for the password (ronoctua-sudo:finger)" ];
        };
      };
    };

    # -----------------------------------------------------------------
    # Firmware: read, never installed
    # -----------------------------------------------------------------
    #
    # The page shows the HSI level and the updates fwupd knows of. Installing
    # firmware would be a new kind of imperative act, and it stays out; so does
    # the daily metadata refresh, which would be the machine reaching the
    # network on a clock for something it never acts on. Enabling fwupd brings
    # one upstream polkit rule (the refresh's user may read and refresh the
    # remotes), signed in `polkit-rules-are-the-ones-we-know` as upstream.
    services.fwupd.enable = true;
    systemd.timers.fwupd-refresh.wantedBy = lib.mkForce [ ];

    # THE 2026-10-05 WAVE: one region per package, after its anchor.
    # ── anchor: L06 ──
    # ── anchor: L12 ──
    # ── anchor: L17 ──
    # ── anchor: L22 ──
    # WHAT THIS GENERATION SHIPS, for the post-quantum section (`LEADER s e
    # q`): the versions the workspace's copy carries too, AND which optional
    # channels this configuration turned on — which the workspace, one build
    # every configuration shares, cannot know (`nix/pq-facts.nix`). Asked of
    # the option that turns each one on — nixpkgs' own where there is one (a
    # host that enables Tor by hand has Tor), ours for Chrome, which nixpkgs
    # only packages (`nix/modules/software.nix` asks the same). The footer
    # reads the file this names before its own baked copy
    # (`ronoctua_security::quantum::built`), and the windows it opens inherit
    # it.
    systemd.user.services.ronoctua-footer.environment.RONOCTUA_PQ_FACTS = "${import ../pq-facts.nix {
      inherit pkgs;
      ships = {
        chrome = config.ronoctua.browser.enable or false;
        syncthing = config.services.syncthing.enable;
        tailscale = config.services.tailscale.enable;
        tor = config.services.tor.enable;
        android = config.virtualisation.waydroid.enable;
      };
    }}";
  };
}
