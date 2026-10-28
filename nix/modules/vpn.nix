# VPN — a folder of WireGuard servers registered at once, their IP kept from
# leaking by IPv6, and the ONE private key the whole set shares.
#
# WHAT THIS IS FOR
# ----------------
# A commercial WireGuard provider (Surfshark is the one this was written for)
# hands out a `.conf` per server — a hundred and more — each naming a public
# key and an endpoint, each with the SAME placeholder where the private key
# goes, and each routing only IPv4 (`AllowedIPs = 0.0.0.0/0`). Three things
# follow, and this module answers all three:
#
#   1. REGISTERING THEM ONE BY ONE is not a thing a person does a hundred
#      times. `nix run .#vpn-import <folder>` reads the whole folder at once
#      (`nix/scripts/vpn-import.py`).
#
#   2. THE IP LEAKS BY IPv6. A tunnel that routes only IPv4, on a machine with
#      working IPv6, sends every IPv6 packet out the ordinary way — around the
#      tunnel, with the real address on it. The import adds `::/0` to every
#      peer and an IPv6 address to the interface, so IPv6 has a default route
#      INTO the tunnel and nowhere else to go (section "IPv6" below).
#
#   3. THE KEY IS NOT IN THE FILES. The import asks it ONCE, and it is sealed
#      in the TPM2 as one blob (`ronoctua_network::secret::seal_wireguard`);
#      every generated profile carries `private-key-flags=1` and no key of its
#      own, and the agent hands the one key over at activation
#      (`ronoctua_network::agent`). So "insert it once, for all of them" is
#      literally one blob, and the key is in no keyfile, no store path, no
#      repository — it is personal, and section 9 keeps personal data out.
#
# WHY THE PROFILES ARE WRITTEN AT RUN TIME, NOT EVALUATED
# -------------------------------------------------------
# The key cannot be in `/nix/store` (world-readable), so a profile cannot be
# generated in `eval`. `ronoctua-vpn-sync` writes one `.nmconnection` per
# location at run time, from the locations the person registered and the
# agent-owned flag — never the key itself. The locations are PUBLIC (a public
# key and an endpoint a provider publishes); they live in `/var/lib`, out of
# the store and out of git, and they regenerate the whole set on any machine.
#
# WHY A POLKIT UNIT, AND NOT THE SUDO DOOR
# ----------------------------------------
# Writing the key to `/persist` and the locations to `/var/lib` is root's.
# The sudo door is the shell's alone (`ronoctua_security::authority` — the one
# caller of `sudo`), and the import runs in a terminal. So the import stages
# the two files in a folder of the person's own and starts
# `ronoctua-vpn-install.service` through ONE narrow polkit rule — the shape
# tor and the parts realizers already use. The unit runs the root helper's
# `vpn-install` verb (`ronoctua_security::vpn`), which takes the staged files
# the way the binary cache's token is taken (`O_NOFOLLOW`, the person's own
# `0600` file) and seals the key.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.vpn;
  user = config.ronoctua.user;
  helper = "${config.ronoctua.security.helper}/bin/ronoctua-security-root";
  nmcli = "${config.networking.networkmanager.package}/bin/nmcli";

  # The three places the Rust half names (`ronoctua_security::vpn`,
  # `ronoctua_network::secret`). Written here too so the units and tmpfiles
  # agree with the code in one glance; the code is the source of truth.
  stagingDir = "/run/ronoctua-vpn";
  stateDir = "/var/lib/ronoctua-vpn";
  locationsFile = "${stateDir}/locations.json";
  blobDir = "/persist/etc/ronoctua/wireguard";

  # The sync program: reads the locations and writes NetworkManager's
  # WireGuard profiles. Python, like the import and `gen-brain-modules.py` —
  # string templating of a keyfile, no Rust unit for it.
  sync = pkgs.writeShellApplication {
    name = "ronoctua-vpn-sync";
    runtimeInputs = [
      pkgs.python3
      config.networking.networkmanager.package
    ];
    text = ''
      exec python3 ${../scripts/vpn-sync.py} "$@"
    '';
  };
in
{
  options.ronoctua.vpn = {
    enable = lib.mkEnableOption "the VPN register, its IPv6 leak guard and the view that drives it" // {
      default = true;
    };

    locations = lib.mkOption {
      type = lib.types.str;
      default = locationsFile;
      description = ''
        The file the register writes and the sync reads: the public locations
        (a name, an endpoint, a public key, the DNS and addresses per server),
        never the private key. A string path read at RUN TIME, not a Nix path:
        a machine with none built against it has nothing to evaluate.
      '';
    };

    ipv6 = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Give every profile an IPv6 address and route `::/0` into the tunnel, so
        the real IPv6 address cannot leak around a tunnel that would otherwise
        carry only IPv4. On by the owner's decision: the leak is the whole
        reason this module exists. Off leaves IPv6 to the ordinary route, which
        is the leak.
      '';
    };

    killSwitch = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Block ALL traffic that is not through a VPN tunnel (or the local
        network, or the WireGuard handshake). Off by default, because when it
        is on and no tunnel is up there is NO internet at all — which is the
        point of a kill switch, and a surprise to meet by accident. The leak
        guard (`ipv6`) runs without it; this is the harder promise on top.
      '';
    };

    dns = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [
        "1.1.1.1"
        "1.0.0.1"
      ];
      description = ''
        Resolvers to use for every location instead of the ones its `.conf`
        named (the provider's). Empty keeps each location's own — which, for
        the leak, is what matters: the tunnel's DNS, reached through it.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = config.ronoctua.network.enable;
        message = "ronoctua.vpn needs ronoctua.network: the profiles are NetworkManager's, and the key is handed over by its agent.";
      }
      {
        assertion = config.ronoctua.security.enable;
        message = "ronoctua.vpn needs ronoctua.security: the install verb is the security centre's root helper, and the polkit rule rides its base.";
      }
      {
        assertion = config.ronoctua.persist.enable;
        message = "ronoctua.vpn needs ronoctua.persist: the sealed key and the locations live in /persist and /var/lib, which the wipe must route around.";
      }
    ];

    # THE FOLDERS, each with the owner the code checks for.
    #  - the staging folder is the PERSON'S, 0700: only they (or root) write in
    #    it, and the install verb takes only files of that uid's
    #    (`ronoctua_security::vpn::take_staged`). On tmpfs; tmpfiles remakes it
    #    every boot.
    #  - the state folder and the key's folder are root's, 0700.
    systemd.tmpfiles.rules = [
      "d ${stagingDir} 0700 ${user} ${config.users.users.${user}.group} -"
      # 0755: the locations and the settings are PUBLIC (no key), and the view
      # reads the settings to draw the toggles. The key is in /persist, not here.
      "d ${stateDir} 0755 root root -"
      "d ${blobDir} 0700 root root -"
    ];

    # INSTALL: the person's staged key and locations, taken and sealed. Started
    # by the person through the polkit rule below; it runs the root helper's
    # `vpn-install` verb, which is a UNIT verb — never the door's. Hardened the
    # way the Wi-Fi agent is: the TPM, `/persist`'s blob folder, the state
    # folder and the staging folder, and nothing else of the file system.
    systemd.services.ronoctua-vpn-install = {
      description = "Takes the person's staged WireGuard key and locations, and seals the key in the TPM2";
      # The profiles follow from the locations; the sync watches the file. But
      # the person who registered is WAITING ON THIS unit to see the list fill,
      # so run the sync HERE too, `--wait`, before this returns: root starts it
      # (the person may not), it writes the profiles and reloads NetworkManager,
      # and only then does the view read again with the locations on it. The
      # path unit fires the sync on its own as well; this only waits for a run
      # that has seen the new locations.
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${helper} vpn-install ${user}";
        ExecStartPost = "${config.systemd.package}/bin/systemctl start --wait ronoctua-vpn-sync.service";
        ProtectSystem = "strict";
        ReadWritePaths = [
          "-${blobDir}"
          "-${stateDir}"
          "-${stagingDir}"
        ];
        DeviceAllow = [ "/dev/tpmrm0 rw" ];
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    # SYNC: one `.nmconnection` per location, written 0600, the stale ones
    # pruned, then `nmcli connection reload`. No key is ever in these files
    # (`private-key-flags=1`); the IPv6 leak guard and the DNS are baked in
    # here (`nix/scripts/vpn-sync.py`).
    systemd.services.ronoctua-vpn-sync = {
      description = "Writes NetworkManager's WireGuard profiles from the registered locations";
      after = [ "NetworkManager.service" ];
      wants = [ "NetworkManager.service" ];
      # Watched by the path unit for changes; run once at boot too, so a
      # machine whose /etc profiles were wiped but whose locations persisted
      # comes back with them.
      wantedBy = [ "multi-user.target" ];
      environment = {
        RONOCTUA_VPN_LOCATIONS = cfg.locations;
        RONOCTUA_VPN_OUT = "/etc/NetworkManager/system-connections";
        RONOCTUA_NMCLI = nmcli;
        RONOCTUA_NFT = "${pkgs.nftables}/bin/nft";
        # THE DEFAULTS, when no runtime settings are set — the module's options.
        # The shell changes the LIVE settings (`settings.conf`); the sync reads
        # the staged change, persists it, and applies it.
        RONOCTUA_VPN_IPV6 = if cfg.ipv6 then "1" else "0";
        RONOCTUA_VPN_DNS = lib.concatStringsSep "," cfg.dns;
        RONOCTUA_VPN_KILLSWITCH = if cfg.killSwitch then "1" else "0";
        RONOCTUA_VPN_SETTINGS = "${stateDir}/settings.conf";
        RONOCTUA_VPN_SETTINGS_STAGING = "${stagingDir}/settings.conf";
        # The locations the shell staged to FORGET — one id per line. The sync
        # drops them from the locations and prunes their profiles.
        RONOCTUA_VPN_FORGET = "${stagingDir}/forget";
      };
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${sync}/bin/ronoctua-vpn-sync";
        ProtectSystem = "strict";
        # The profiles, the live settings it persists, and the staged change it
        # spends. It runs `nft` for the kill switch (as root, over netlink — no
        # address-family restriction that would block it).
        ReadWritePaths = [
          "/etc/NetworkManager/system-connections"
          stateDir
          stagingDir
        ];
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    # A CHANGE LANDED — the locations the install wrote, or a settings change the
    # shell staged: regenerate. The file lands, the path fires, the sync runs,
    # so neither the register nor a toggle needs privilege of its own.
    systemd.paths.ronoctua-vpn-sync = {
      description = "Regenerates the WireGuard profiles when the locations or settings change";
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathModified = [
          cfg.locations
          "${stagingDir}/settings.conf"
          "${stagingDir}/forget"
        ];
        Unit = "ronoctua-vpn-sync.service";
      };
    };

    # THE ONE NEW POLKIT RULE, its whole width on these lines: `YES`, `start`,
    # one unit, one user. The same shape as the firewall's shields and tor's
    # start — there is no polkit agent in this session, so `YES` is the answer
    # that works, and what buys it is that the door is one unit wide. (Bringing
    # a profile UP needs no rule: NetworkManager grants an active local session
    # `network-control` already.)
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (
          action.id == "org.freedesktop.systemd1.manage-units" &&
          action.lookup("verb") == "start" &&
          action.lookup("unit") == "ronoctua-vpn-install.service" &&
          subject.user == "${user}"
        ) {
          return polkit.Result.YES;
        }
      });
    '';

    # THE KILL SWITCH IS RUN-TIME NOW, not declarative: the sync adds or removes
    # its own `inet` table (`ronoctua_vpn_ks`, loopback + the tunnels + the LAN
    # + the WireGuard handshake pass, everything else dropped) as the live
    # settings say (`nix/scripts/vpn-sync.py`), so a toggle from the shell takes
    # without a rebuild, and a reboot re-applies it because the sync runs at
    # boot. A table of its own, so it never touches the person's firewall
    # (`nix/modules/security.nix`). `ronoctua.vpn.killSwitch` is only the default
    # for a machine that has never set it from the shell.

    # WHAT MUST SURVIVE THE WIPE IS NAMED IN ONE PLACE — `nix/modules/persist.nix`
    # — and the state folder is in it. The sealed key is under `/persist`
    # already, and `/etc/NetworkManager/system-connections` is on that list too.

    # `nix run .#vpn-import <folder>` is wired in `flake.nix`'s apps.
  };
}
