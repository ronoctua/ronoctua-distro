# Syncthing — installed by default and already tuned.
#
# WHY IT SHIPS OUT OF THE BOX
# ---------------------------
# This distro assumes several machines belonging to one person, and the folder
# that travels between them is infrastructure, not an application. Installing
# it afterwards, by hand, would be exactly the "step that needs an instruction
# in the README" that this project avoids.
#
# THE VALUES CAME FROM A MEASUREMENT, NOT FROM A BLOG POST
# --------------------------------------------------------
# A real 592 GB transfer between two machines of this project was measured
# from beginning to end. What was wrong cost 16× in speed; what remained
# afterwards is what these values attack. Each one states what was observed.
#
# WHAT THIS MODULE DOES NOT DECLARE
# ---------------------------------
# No folder and no device. A device id and a folder path are personal data
# (invariant 13) and do not belong in the repository. As a direct consequence,
# `overrideFolders` and `overrideDevices` STAY OFF — see their comment, which
# is the most dangerous point in this file.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.syncthing;
  user_name = config.ronoctua.user;
  configDir = "/home/${user_name}/.local/state/syncthing";

  # Folder and device defaults do NOT fit in `settings`.
  #
  # The nixpkgs module sends each `settings` key to `/rest/config/<key>`,
  # and `defaults` is not an endpoint — the PUT would return 404, curl would
  # exit 0 and the setting would vanish silently. Silence is precisely the
  # failure mode this project hunts down with checks.
  #
  # So it goes through a unit of its own, with `--fail-with-body`: if the
  # endpoint is renamed, this FAILS LOUDLY instead of becoming a value nobody
  # applied.
  tunePatterns = pkgs.writeShellApplication {
    name = "ronoctua-syncthing-tune";
    runtimeInputs = with pkgs; [
      curl
      libxml2
    ];
    text = ''
      api_key=$(xmllint --xpath 'string(configuration/gui/apikey)' ${configDir}/config.xml)

      api() {
        curl --fail-with-body --silent --show-error \
          --header "X-API-Key: $api_key" \
          --header 'Content-Type: application/json' \
          "$@"
      }

      # The API comes up a moment after the process does. Without this wait, the
      # unit would race on the first run and fail for arriving too early.
      for _ in $(seq 1 30); do
        if api --request GET http://${cfg.guiAddr}/rest/system/ping >/dev/null 2>&1; then
          break
        fi
        sleep 1
      done

      # FOLDER DEFAULT — applies to every folder created from here on.
      #
      # `copyRangeMethod = ioctl`: the filesystem is btrfs (section 7.2), and
      # `ioctl` copies blocks by reference (reflink) instead of reading and
      # writing. It does not speed up what goes over the air; it speeds up
      # renaming, duplicating and reusing a block already on disk.
      #
      # `pullerMaxPendingKiB`: how much the PULLING side keeps in flight. The
      # default is conservative for a 1.9 Gbit/s link — with a 20 ms RTT that is
      # ~4.8 MB just to avoid going idle, and asking for too little means
      # spending the time waiting for an answer with the radio doing nothing.
      api --request PATCH \
        --data '{"copyRangeMethod":"ioctl","pullerMaxPendingKiB":262144}' \
        http://${cfg.guiAddr}/rest/config/defaults/folder

      # DEVICE DEFAULT.
      #
      # `numConnections`: the default is ONE TCP connection per device. On a
      # wireless link, whose latency under load goes from 3 ms up to 150 ms, a
      # single flow cannot keep enough window in flight. Three split the work
      # and absorb the queueing.
      api --request PATCH \
        --data '{"numConnections":${toString cfg.connectionsPerDevice}}' \
        http://${cfg.guiAddr}/rest/config/defaults/device
    '';
  };
in
{
  options.ronoctua.syncthing = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Instala e afina o Syncthing. Ligado por padrão.";
    };

    guiAddr = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:8384";
      description = ''
        Onde a interface web escuta. Fica no localhost de propósito: quem
        abre é o command `syncthing-open` do `LEADER r`, na própria
        máquina. Expor na LAN seria uma porta administrativa sem senha.
      '';
    };

    connectionsPerDevice = lib.mkOption {
      type = lib.types.ints.between 1 8;
      default = 3;
      description = ''
        Conexões TCP paralelas por dispositivo. Três porque num enlace sem
        fio um fluxo só não enche a janela; acima disso o ganho some e o
        custo de handshake aparece.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.syncthing = {
      enable = true;
      user = user_name;
      group = "users";
      # The default folder for whoever creates a new one, and where the index lives.
      dataDir = "/home/${user_name}";
      inherit configDir;
      guiAddress = cfg.guiAddr;
      openDefaultPorts = true;

      # ===================================================================
      # THE MOST DANGEROUS LINE IN THIS FILE — read before touching it.
      #
      # These two fields default to `true` in nixpkgs, and `true` means
      # "whatever is not declared in Nix gets DELETED at activation". Since
      # this module declares no folder at all (invariant 13), leaving the
      # default would make the system DELETE EVERY FOLDER on each
      # `nixos-rebuild` — including the ones the person just added through
      # the GUI, with no warning and no undo.
      #
      # Declaring folders here and turning this back on would trade one
      # problem for another: a folder path is personal data.
      # ===================================================================
      overrideFolders = false;
      overrideDevices = false;

      settings.options = {
        # UPnP WAS FAILING EVERY 30 MINUTES, for days, on the measured machine:
        # the router answers `Error 501` and Syncthing tries again, forever.
        # There is no reason to map an external port for a local-network
        # sync.
        natEnabled = false;

        # The relay is a fallback route, and it is extremely slow. With it on,
        # a momentary discovery failure drops the transfer to ~1 MB/s without
        # saying anything. Better not to connect at all than to connect slowly
        # while pretending everything is fine.
        relaysEnabled = false;

        # Discovery: the local one finds machines on the LAN, which is the
        # common case here; the global one stays on for when they are not on
        # the same network. Discovery is not relaying — it only says where the
        # other end is.
        localAnnounceEnabled = true;
        globalAnnounceEnabled = true;

        # No limit, and no throttling inside the LAN.
        maxSendKbps = 0;
        maxRecvKbps = 0;
        limitBandwidthInLan = false;

        # The process is NOT deprioritised.
        #
        # `setLowPriority` puts it at `nice 11`. The measurement showed the CPU
        # 94% idle and zero disk wait during the transfer — so deprioritising
        # protects nobody from anything, and only gets in the way when the
        # machine actually does get busy.
        setLowPriority = false;

        # The package comes from Nix. Auto-upgrade would try to write inside
        # /nix/store, which is read-only: it would fail forever, silently,
        # every 12 hours.
        autoUpgradeIntervalH = 0;

        # Neither telemetry nor crash reporting: invariant 13.
        urAccepted = -1;
        crashReportingEnabled = false;

        # A system service has no browser to open.
        startBrowser = false;
      };
    };

    # Syncthing's ports are opened by `openDefaultPorts`; mDNS stays in the
    # network module. Each service opens the port it uses.

    systemd.services.ronoctua-syncthing-tune = {
      description = "Afina os padrões de pasta e dispositivo do Syncthing";
      # After the nixpkgs init, which is what creates the config.xml the API key
      # is read from.
      after = [
        "syncthing.service"
        "syncthing-init.service"
      ];
      requires = [ "syncthing.service" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        User = user_name;
        Group = "users";
        RemainAfterExit = true;
        ExecStart = lib.getExe tunePatterns;
      };
    };
  };
}
