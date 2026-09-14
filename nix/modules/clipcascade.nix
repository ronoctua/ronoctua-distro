# ClipCascade's SERVER, for the few machines that are one.
#
# WHY THERE IS A SERVER HERE AT ALL
# ----------------------------------
# The owner's words: *"a maioria dos computadores com a ronoctua-distro rodarão
# como cliente, mas haverão alguns poucos que também funcionarão como
# servidor"*. So this is off by default and costs a machine that does not turn
# it on exactly nothing.
#
# THE CLIENT IS NOT HERE. It is `crates/ronoctua-clipcascade`, spoken natively
# by the footer — see that crate's manifest for why the upstream Python client
# is not packaged: it watches the clipboard itself, and two watchers are two
# histories.
#
# NO DOCKER (invariant 17), AND NO GRADLE
# ----------------------------------------
# Upstream distributes the server as a Docker image and, in the same release,
# as a plain Spring Boot fat JAR. Measured on 3.2.0: `ClipCascade-Server-
# JRE_21.jar` is 75 MiB, `Main-Class: org.springframework.boot.loader.launch.
# JarLauncher`, Spring Boot 3.4.8, and it does NOT bundle a runtime — "JRE_21"
# names what it REQUIRES. So this is a `fetchurl` and a `java -jar`, which is
# provable and needs no build of somebody else's Maven tree.
#
# THE DATABASE IS H2, IN A FILE, ENCRYPTED
# -----------------------------------------
# That is upstream's own default and it is kept: a Postgres for a clipboard
# between three machines is a second daemon to keep alive. The encryption
# password is GENERATED ON THE MACHINE and never appears in this repository
# (invariant 13) — `nix/modules/secrets.nix` is explicit that a password may
# not live here even encrypted, and `nix/modules/aria2.nix` is the shape this
# follows: `install -m600` and never a redirect, because a redirect creates the
# file with the umask's permissions and only then narrows them.
#
# SIGN-UP IS OFF BY DEFAULT. A clipboard server that accepts new accounts from
# anybody who can reach it is one that carries somebody else's clipboard.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.clipcascade.server;

  jar = pkgs.fetchurl {
    url = "https://github.com/Sathvik-Rao/ClipCascade/releases/download/3.2.0/ClipCascade-Server-JRE_21.jar";
    hash = "sha256-JMcO2YFVS05bqInqxn/NDWvaC1ttdZHqZUZFbM36u7U=";
  };

  # THE PASSWORD IS BORN HERE, ONCE, and stays. Regenerating it on every start
  # would lock the server out of its own database: H2's `CIPHER=AES` key IS the
  # password, and a new one does not open an old file.
  prepare = pkgs.writeShellApplication {
    name = "ronoctua-clipcascade-prepare";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.openssl
    ];
    text = ''
      state="/var/lib/clipcascade"
      mkdir -p "$state/database" "$state/logs"
      if [ ! -s "$state/db.password" ]; then
        umask 077
        tmp=$(mktemp "$state/db.XXXXXX")
        openssl rand -hex 32 > "$tmp"
        install -m600 "$tmp" "$state/db.password"
        rm -f "$tmp"
      fi
    '';
  };
in
{
  options.ronoctua.clipcascade.server = {
    enable = lib.mkEnableOption ''
      the ClipCascade server on this machine.

      Off by default: most machines running this distro are clients. What the
      footer needs to SYNC is the client, which is built in and needs no
      option — see `crates/ronoctua-clipcascade`.
    '';

    port = lib.mkOption {
      type = lib.types.port;
      default = 8080;
      description = "What the server listens on.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Open the port. OFF by default, and deliberately: a machine that serves
        only its own tailnet does not need a hole in the firewall, and one that
        serves the LAN is a decision rather than a side effect.
      '';
    };

    address = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = ''
        What the server binds to. The loopback by default — put it behind a
        reverse proxy with TLS, or reach it over the tailnet.

        THE CLIENT REFUSES PLAIN HTTP over anything but a loopback, and the
        reason is in `ronoctua_clipcascade::open_socket`: the clipboard is
        encrypted end to end, so the contents would survive — but the session
        cookie would not, and a stolen cookie is the account.
      '';
    };

    signup = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let new accounts be created. Off: a clipboard server that accepts
        anybody who can reach it is one that carries somebody else's clipboard.
        Turn it on, make the accounts, turn it off.
      '';
    };

    accounts = lib.mkOption {
      type = lib.types.int;
      default = -1;
      description = "The most accounts to allow. `-1` is no limit.";
    };

    messageSize = lib.mkOption {
      type = lib.types.int;
      default = 1;
      description = ''
        The largest clipboard entry the server carries, in MiB. The client
        reads this back from `/max-size` and refuses a larger one BEFORE
        sending it, because the broker's refusal arrives as a STOMP `ERROR`
        that ends the whole session.
      '';
    };

    peerToPeer = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Offer P2P mode, where the server only does the signalling and the
        clipboard goes machine to machine over WebRTC.

        OFF, AND THE CLIENT HERE DOES NOT SPEAK IT. `ronoctua-clipcascade`
        implements `P2S` — the server relays — because P2P is a whole WebRTC
        stack for a clipboard that is already end-to-end encrypted. Turning
        this on would make this server offer a mode its own distro's client
        cannot use, which is why it is named and defaulted off rather than
        left out.
      '';
    };

    logLevel = lib.mkOption {
      type = lib.types.enum [
        "INFO"
        "DEBUG"
        "TRACE"
      ];
      default = "INFO";
      description = "How much the server writes to its log.";
    };

    sessionTimeout = lib.mkOption {
      type = lib.types.str;
      default = "525960m";
      description = ''
        How long a login lasts. Upstream's default is a year, and it is kept:
        the client holds a cookie and has deliberately NOT kept the password,
        so a short timeout would mean typing it again every week.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = !cfg.peerToPeer;
        message = ''
          ronoctua.clipcascade.server.peerToPeer offers a mode this distro's
          own client does not speak: `ronoctua-clipcascade` implements P2S.
          Turning it on would leave the footer unable to sync with the server
          on its own machine.
        '';
      }
      {
        assertion = cfg.address == "127.0.0.1" || cfg.openFirewall;
        message = ''
          ronoctua.clipcascade.server binds to ${cfg.address} but the firewall
          is closed, so nothing can reach it. Either bind to 127.0.0.1 and put
          a reverse proxy in front, or set openFirewall.
        '';
      }
    ];

    systemd.services.clipcascade = {
      description = "ClipCascade — the clipboard server";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      serviceConfig = {
        Type = "simple";
        # THE PASSWORD IS WRITTEN BEFORE THE SERVER READS IT, and by a separate
        # program: `ExecStartPre` failing stops the unit, where a shell `&&`
        # inside `ExecStart` would start the server with no database at all.
        # The shape is `nix/modules/aria2.nix`'s.
        ExecStartPre = "+${prepare}/bin/ronoctua-clipcascade-prepare";
        ExecStart = pkgs.writeShellScript "clipcascade-start" ''
          # THE PASSWORD IS READ FROM THE FILE INTO THE ENVIRONMENT, never
          # passed on the command line: `argv` is readable by every process on
          # the machine through `/proc/*/cmdline`.
          CC_SERVER_DB_PASSWORD="$(cat /var/lib/clipcascade/db.password)"
          export CC_SERVER_DB_PASSWORD
          exec ${pkgs.jdk21_headless}/bin/java -jar ${jar}
        '';
        WorkingDirectory = "/var/lib/clipcascade";
        StateDirectory = "clipcascade";
        StateDirectoryMode = "0700";
        DynamicUser = true;
        Restart = "on-failure";
        RestartSec = 5;

        # THE HARDENING IS THE SAME LIST `nix/modules/aria2.nix` carries. A
        # Java server reachable from the network is the last place to leave a
        # default open.
        NoNewPrivileges = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
        ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = false; # the JVM writes what it executes
        SystemCallArchitectures = "native";
      };

      environment = {
        CC_PORT = toString cfg.port;
        CC_SIGNUP_ENABLED = lib.boolToString cfg.signup;
        CC_MAX_USER_ACCOUNTS = toString cfg.accounts;
        CC_MAX_MESSAGE_SIZE_IN_MiB = toString cfg.messageSize;
        CC_P2P_ENABLED = lib.boolToString cfg.peerToPeer;
        CC_SESSION_TIMEOUT = cfg.sessionTimeout;
        CC_SERVER_LOGGING_LEVEL = cfg.logLevel;
        CC_SERVER_DB_USERNAME = "clipcascade";
        CC_SERVER_DB_URL = "jdbc:h2:file:./database/clipcascade;CIPHER=AES;MODE=PostgreSQL";
        CC_SERVER_DB_DRIVER = "org.h2.Driver";
        CC_SERVER_DB_HIBERNATE_DIALECT = "org.hibernate.dialect.H2Dialect";
        # SPRING'S OWN BIND ADDRESS. `CC_PORT` moves the port and nothing in
        # upstream's table moves the interface, so this is the Spring property
        # underneath it — measured against the `application.properties` the
        # fat JAR carries.
        SERVER_ADDRESS = cfg.address;
      };
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
  };
}
