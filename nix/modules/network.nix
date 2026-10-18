# Network — Wi-Fi and Ethernet, tuned for throughput.
#
# ONE MANAGER FOR BOTH MEDIA
# --------------------------
# NetworkManager handles cable and radio through the same API (D-Bus), and it
# is what `ronoctua-network` queries. Having `systemd-networkd` for the cable
# and something else for the radio would give two places where an interface
# can be "connected", and the modeline would have to ask both — which is how
# you find out, too late, that the two disagree.
#
# WHY EACH VALUE IS HERE
# ----------------------
# None of these numbers is a guess: they came from measuring a real 592 GB
# transfer between two machines of this project. What was wrong cost 16× in
# speed, and what remained as a bottleneck afterwards is what these values
# attack. Each block below states what was measured.
{
  config,
  lib,
  pkgs,
  ronoctua ? null,
  ...
}:

let
  cfg = config.ronoctua.network;

  # THE BUS POLICY, and not a check inside the daemon.
  #
  # Who may request the sealing is decided here, by dbus-daemon, before the
  # message ever reaches the process. An `if user == "noctua"` inside the Rust
  # would be a second truth — and the one that fails silently, because a daemon
  # that authorises itself looks like it works right up until it doesn't.
  dbusPolicy = pkgs.writeTextFile {
    name = "ronoctua-network-agent-dbus";
    destination = "/share/dbus-1/system.d/org.ronoctua.agent.conf";
    text = ''
      <!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
       "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
      <busconfig>
        <!-- Only root brings up the nmAgent: it is the one with /dev/tpmrm0. -->
        <policy user="root">
          <allow own="org.ronoctua.agent"/>
        </policy>

        <!-- The distro's user_name asks to store and to forget, and nothing beyond that. -->
        <policy user="${config.ronoctua.user}">
          <allow send_destination="org.ronoctua.agent"
                 send_interface="org.ronoctua.Network1"/>
        </policy>

        <!-- Anything else: nothing. The dbus default is to deny, and
             repeating the denial here documents the intent for whoever
             edits this next. -->
        <policy context="default">
          <deny own="org.ronoctua.agent"/>
          <deny send_destination="org.ronoctua.agent"/>
        </policy>
      </busconfig>
    '';
  };
in
{
  options.ronoctua.network = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Switches the distro's network layer on. Switching it off leaves the machine with
        no network manager at all — it only makes sense in a test.
      '';
    };

    # WITHOUT THIS, THE DAEMON CANNOT TELL A WAY OUT FROM A DEAD END.
    #
    # NetworkManager answers `Connectivity = FULL` for any route at all when
    # it has no URI to check — so a hotel's login page and a router whose
    # uplink is down both read as "the internet is there", and the modeline's
    # mark could never appear. The check is one HTTP GET every five minutes
    # to a host whose whole purpose is answering it.
    #
    # THE COST IS WRITTEN DOWN because it is real: it is a request that says
    # this machine is on, to a server the person did not choose. Turning it
    # off is a supported answer, and what it costs is the mark.
    connectivityCheck = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let NetworkManager check, every five minutes, whether the way out
        actually leads anywhere — one small HTTP request to
        `nmcheck.gnome.org`. It is what separates "connected" from
        "connected to a login page", and the footer's `no way out` mark
        cannot exist without it: with the check off the daemon answers
        `full` for any route at all. Off means one fewer request and one
        fewer thing on screen.
      '';
    };

    regulatoryDomain = lib.mkOption {
      type = lib.types.str;
      default = "BR";
      example = "US";
      description = ''
        The radio's regulatory domain, in ISO 3166-1 alpha-2.

        It is not paperwork: it decides which channels and what power the radio
        may use. With no domain the kernel assumes the most restrictive in the world,
        and precisely the upper part of 5 GHz goes away, where the wide and
        empty channels are.
      '';
    };

    nmAgent = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Switches on the secret nmAgent: the Wi-Fi password comes out of the profile file
        and goes on to live sealed in the TPM2.

        Switching it off goes back to the world's behaviour — `psk=<password>` inside the
        keyfile. It exists as an option because a machine with no TPM has nowhere
        to seal, and failing the boot over that would be worse than storing it in a file.
      '';
    };

    tuneTcp = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Applies the TCP stack tuning described in this module. It exists so
        that it can be switched off in a test that wants the kernel's default.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # WHO MADE THE THING AT A HARDWARE ADDRESS — `nmap`'s own table, which is
    # a text file this system already has (`nix/modules/cli.nix`). The view
    # reads it to put a maker beside each machine on the local network.
    #
    # FROM THE ENVIRONMENT AND NOT BAKED INTO THE BINARY: a store path in
    # `option_env!` is a reference the BINARY carries, so a nmap release would
    # rebuild the whole Rust workspace for a table nothing else reads — the
    # measured trap `RONOCTUA_ICONS` carries a comment about. `nmap` is in
    # both profiles (`nix/modules/cli.nix`), so what this buys is the rebuild
    # and not the closure; unset, the column is blank, which is what an
    # unregistered prefix looks like anyway.
    #
    # BOTH PLACES, for the reason `media.nix` gives: `sessionVariables`
    # reaches a login shell, and the footer is a systemd USER UNIT, which
    # inherits the manager's environment rather than the shell's.
    environment.sessionVariables.RONOCTUA_NMAP_MAC_PREFIXES = "${pkgs.nmap}/share/nmap/nmap-mac-prefixes";
    systemd.user.services.ronoctua-footer.environment.RONOCTUA_NMAP_MAC_PREFIXES =
      "${pkgs.nmap}/share/nmap/nmap-mac-prefixes";

    # -------------------------------------------------------------------
    # NetworkManager
    # -------------------------------------------------------------------
    networking.networkmanager = {
      enable = true;

      # THE CHECK, and the daemon's own default interval spelled out: five
      # minutes is what upstream uses, and writing it here is what makes the
      # cost of the option legible where the option is.
      #
      # `settings.connectivity` AND NOT `connectivity`: the nixpkgs module has
      # no option of that name — what it has is `settings`, which is written
      # straight into `NetworkManager.conf` as its own `[connectivity]`
      # section. The wrong one fails at EVALUATION, with a "did you mean"
      # naming three options that have nothing to do with it.
      settings.connectivity = lib.mkIf cfg.connectivityCheck {
        uri = "http://nmcheck.gnome.org/check_network_status.txt";
        interval = 300;
      };

      # THE SINGLE BIGGEST WIN ON THE RADIO, and the easiest to miss.
      #
      # With power saving on, the radio sleeps between frames and wakes at
      # the next beacon; latency rises and throughput halves, with nothing
      # on screen saying why. On a distro whose use case is syncing hundreds
      # of gigabytes, the arithmetic does not work out.
      wifi.powersave = false;

      # Stable MAC while scanning.
      #
      # The default draws a random MAC per scan, which is good for not being
      # tracked on a public network. The cost is that the router sees a new
      # client every time: fresh DHCP lease, cold ARP, and a few seconds
      # before traffic flows again. On a machine that lives on known
      # networks, the cost is daily and the benefit is nil.
      wifi.scanRandMacAddress = false;

      # `wpa_supplicant`, and not `iwd`.
      #
      # iwd connects and roams faster, and it is tempting. But nixpkgs itself
      # marks the iwd backend as experimental under NetworkManager, and what
      # it speeds up is the MOMENT of connecting — not the throughput once
      # connected, which is what this module is after. Trading reliability
      # for a second at connection time would be paying dearly in the wrong
      # place.
      wifi.backend = "wpa_supplicant";

      # Same reasoning as the scan MAC, now for the cable.
      ethernet.macAddress = "permanent";

      # The caching resolver belongs to systemd; without this line NM would
      # write its own `/etc/resolv.conf` and resolved's cache would sit idle.
      dns = "systemd-resolved";
    };

    # Local DNS cache. It does not change throughput on a large file, but
    # it removes one round trip per resolved name — and Syncthing resolves
    # the discovery server often.
    services.resolved = {
      enable = true;
      # Discovery by name on the LAN, which is how this project's machines
      # find each other when Syncthing's local announcement is not enough.
      #
      # Through `settings`, and not through the `llmnr` option: it was renamed
      # in nixpkgs and today only emits a warning. A rename warning is the
      # state right before removal — and when that comes, the build breaks
      # somewhere that has nothing to do with networking.
      settings.Resolve.LLMNR = "true";
    };

    # The regulatory database has to exist on the system for the
    # `ieee80211_regdom` below to mean anything.
    hardware.wirelessRegulatoryDatabase = true;
    boot.extraModprobeConfig = ''
      options cfg80211 ieee80211_regdom=${cfg.regulatoryDomain}
    '';

    # -------------------------------------------------------------------
    # TCP stack
    #
    # What the measurement showed: with the link saturated, latency to the
    # router went from 1.3 ms up to 105 ms — pure queueing. cubic answers
    # that by filling the queue until it drops a packet; BBR estimates
    # bandwidth and RTT and PACES the sending, keeping the queue short.
    # -------------------------------------------------------------------
    boot.kernelModules = lib.mkIf cfg.tuneTcp [ "tcp_bbr" ];

    boot.kernel.sysctl = lib.mkIf cfg.tuneTcp {
      # BBR requires a qdisc that can pace. Without `fq` it performs worse
      # than cubic — the classic mistake of enabling only half of it.
      "net.core.default_qdisc" = "fq";
      "net.ipv4.tcp_congestion_control" = "bbr";

      # Maximum window. The measured link does 1.9 Gbit/s with an RTT that
      # reaches 20 ms under load: that is ~4.8 MB in flight just to avoid
      # going idle. The kernel default is nowhere near, and the result is a
      # flow waiting for an ACK with the radio doing nothing.
      "net.core.rmem_max" = 16777216;
      "net.core.wmem_max" = 16777216;
      "net.ipv4.tcp_rmem" = "4096 131072 16777216";
      "net.ipv4.tcp_wmem" = "4096 65536 16777216";

      # THE TRAP THAT COSTS THE MOST DURING A SYNC.
      #
      # By default the kernel throws the window back to the start after a
      # short pause in the connection. Syncthing pauses between files all
      # the time — so every new file restarts in slow start, and in a folder
      # with thousands of files that is most of the elapsed time.
      "net.ipv4.tcp_slow_start_after_idle" = 0;

      # Discovers the MTU when something on the path swallows ICMP. Without
      # this, a tunnel in the middle becomes a connection that opens and
      # then stalls silently.
      "net.ipv4.tcp_mtu_probing" = 1;

      # At 2.4 Gbit/s of PHY, the softirq's default queue of 1000 frames
      # overflows before the process gets scheduled — and a packet dropped
      # here is indistinguishable from congestion to the other side.
      "net.core.netdev_max_backlog" = 16384;
      "net.core.somaxconn" = 4096;

      # Saves one round trip when opening each connection, in both roles.
      "net.ipv4.tcp_fastopen" = 3;
    };

    # -------------------------------------------------------------------
    # Firewall
    # -------------------------------------------------------------------
    # mDNS: this is how machines find each other by name on the LAN. The
    # Syncthing ports live in its own module — each service opens the port
    # it uses, otherwise this list becomes a dump nobody audits.
    networking.firewall.allowedUDPPorts = [ 5353 ];

    # -------------------------------------------------------------------
    # THE SECRET AGENT — the Wi-Fi password leaves the profile file.
    #
    # Without it, NetworkManager demands `psk=<password>` inside the
    # keyfile. With it, the keyfile carries `psk-flags=1` and the password
    # lives sealed in the TPM2.
    # -------------------------------------------------------------------
    services.dbus.packages = lib.mkIf cfg.nmAgent [ dbusPolicy ];

    # -------------------------------------------------------------------
    # The console question, for the boot where nobody is logged in.
    #
    # NOT part of the agent, and the reason is the specification: NetworkManager
    # only lets an agent interact when it passes `ALLOW_INTERACTION`, and at
    # boot-time autoconnect it does not. So this is a separate service that
    # behaves like a PERSON — it waits, notices the network did not come up, and
    # asks on the console through the same mechanism that asks for the disk's
    # passphrase.
    #
    # `Type = "oneshot"` and no `Restart`: it asks once. A service that kept
    # coming back would put a password prompt on a console every minute of a
    # machine that is simply out of range.
    # -------------------------------------------------------------------
    systemd.services.ronoctua-network-console = lib.mkIf cfg.nmAgent {
      description = "Asks for the Wi-Fi password on the console when nobody is logged in";
      after = [
        "NetworkManager.service"
        "ronoctua-network-agent.service"
        "systemd-user-sessions.service"
      ];
      wants = [ "NetworkManager.service" ];
      # `multi-user.target`, and NOT `graphical.target`: this exists precisely
      # for the boot that does not reach a graphical session.
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        # A program of its own (`rust.networkConsole`): the footer's bundle
        # would put the desktop in a root unit's closure.
        ExecStart = "${ronoctua.networkConsole}/bin/ronoctua-network-console";
        # It waits before asking, and the ask itself has a timeout. The margin
        # is what keeps a hung prompt from holding the boot forever.
        TimeoutStartSec = 300;
        # It reads logind's session directory, talks to NM on the system bus,
        # and seals through the TPM — the same surface as the agent.
        ProtectSystem = "strict";
        ReadWritePaths = [ "/persist/etc/ronoctua/wifi" ];
        DeviceAllow = [ "/dev/tpmrm0 rw" ];
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    systemd.services.ronoctua-network-agent = lib.mkIf cfg.nmAgent {
      description = "NetworkManager's secret agent (password sealed in the TPM2)";
      # AFTER NetworkManager: registering with `AgentManager` fails if it is
      # not on the bus yet.
      after = [
        "NetworkManager.service"
        "dbus.service"
      ];
      requires = [ "NetworkManager.service" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${ronoctua.networkAgent}/bin/ronoctua-network-agent";
        # ALWAYS RESTARTS, with room between attempts.
        #
        # If it dies, the network does not die with it: NetworkManager falls
        # back to asking the password of whoever is looking. But a dead agent
        # is a machine that will not reconnect on its own at the next boot,
        # and that deserves to come back up instead of waiting to be noticed.
        Restart = "always";
        RestartSec = 5;

        # It needs the TPM and `/persist`, and nothing else.
        ProtectSystem = "strict";
        ReadWritePaths = [ "/persist/etc/ronoctua/wifi" ];
        DeviceAllow = [ "/dev/tpmrm0 rw" ];
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    # The blob directory, with owner and mode set before the daemon needs it.
    systemd.tmpfiles.rules = lib.mkIf cfg.nmAgent [
      "d /persist/etc/ronoctua 0755 root root -"
      "d /persist/etc/ronoctua/wifi 0700 root root -"
    ];

    # The tools for looking at the link when the screen is not enough. The
    # `ronoctua-network` crate reads everything over D-Bus and does not
    # depend on them; they exist for the person, at the terminal.
    environment.systemPackages = with pkgs; [
      iw
      ethtool
    ];
  };
}
