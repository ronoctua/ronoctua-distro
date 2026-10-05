# The person's containers — Podman, rootless, compatible with Docker.
#
# WHAT THE OWNER DECIDED
# ----------------------
# Containers are Podman's, and Docker's CLI and API are served BY Podman:
# `docker` is podman (`dockerCompat`), `DOCKER_HOST` points at the person's
# rootless socket, and compose is `podman-compose` (`docker-compose` v2 is a
# Docker client, and invariant 17 stays as it is). What the person pulls,
# builds and runs lives in their home — invariant 6's exception for the
# person's programs, never `/etc`.
#
# WHAT IS DECLARED, AND WHY EACH
# ------------------------------
# - NO ROOTFUL SOCKET. nixpkgs wants `podman.socket` in `sockets.target`;
#   a root API socket is a root shell for its group, and nobody here uses it.
#   The ROOTLESS socket (`systemd.user.sockets.podman`, nixpkgs' own) stays:
#   it is what `DOCKER_HOST` names, for third-party tools. The shell never
#   uses it — `ronoctua-containers` runs the CLI.
# - NO AUTOPRUNE. It is a timer that deletes stopped containers and unused
#   images by the clock; removing is the person's, after a question.
# - DNS on the default network, so containers of one compose project find
#   each other by name.
# - crun, events in the journal, overlay storage (native rootless overlay on
#   btrfs), compose through `podman-compose` by store path.
# - Registries in the v2 format, written whole: nixpkgs writes the v1
#   `[registries.search]` table, and v1 and v2 keys in one file are refused
#   by containers-image ("mixing sysregistry v1/v2 is not supported").
# - Subordinate ids: nixpkgs gives every normal user a range already
#   (`autoSubUidGidRange` is `mkDefault true` for them); the assertion below
#   says so if that ever stops being true.
#
# WHERE PODMAN IS, FOR THE SHELL
# ------------------------------
# `RONOCTUA_PODMAN`, from the ENVIRONMENT and never baked in: a store path
# compiled into `ronoctua-shell` would carry podman's closure (conmon, crun,
# netavark, passt) into every profile — the trap `RONOCTUA_SWAPPY` records
# in `nix/modules/media.nix`. THREE PLACES, for that file's reason:
# `sessionVariables` reaches a login shell, the footer and the queue are
# user units with their own environment.
#
# ~/.local/share/containers IS A SUBVOLUME OF ITS OWN (`filesystem.nix`):
# snapshots of `/home` must not hold every image layer the person pulled.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.containers;
  toml = pkgs.formats.toml { };
  podman = lib.getExe config.virtualisation.podman.package;
  people = lib.filterAttrs (_: u: u.isNormalUser) config.users.users;
in
{
  options.ronoctua.containers.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Podman, rootless and compatible with Docker: the `docker` command, the
      person's API socket as `DOCKER_HOST`, compose through `podman-compose`.
      On in the full profile (`nix/modules/profile.nix`).
    '';
  };

  config = lib.mkIf cfg.enable {
    virtualisation.podman = {
      enable = true;
      dockerCompat = true;
      defaultNetwork.settings.dns_enabled = true;
      autoPrune.enable = false;
    };
    # No root API socket (see the header). The rootless one stays.
    systemd.sockets.podman.wantedBy = lib.mkForce [ ];

    virtualisation.containers = {
      containersConf.settings.engine = {
        runtime = "crun";
        events_logger = "journald";
        compose_providers = [ "${pkgs.podman-compose}/bin/podman-compose" ];
        # `podman compose` says which provider it runs on every call; the
        # provider is declared here, so the line says nothing.
        compose_warning_logs = false;
      };
      storage.settings.storage.driver = "overlay";
    };
    environment.etc."containers/registries.conf".source = lib.mkForce (
      toml.generate "registries.conf" {
        unqualified-search-registries = [
          "docker.io"
          "quay.io"
          "ghcr.io"
        ];
        short-name-mode = "permissive";
      }
    );

    environment.systemPackages = [ pkgs.podman-compose ];

    # DOCKER_HOST: for shells (login and interactive)…
    environment.extraInit = ''
      if [ -n "''${XDG_RUNTIME_DIR:-}" ]; then
        export DOCKER_HOST="unix://$XDG_RUNTIME_DIR/podman/podman.sock"
      fi
    '';
    # …and for every user unit, through `environment.d` (the route NixOS
    # takes for `PATH`, `50-systemd-path.conf`): the generator expands the
    # manager's own `XDG_RUNTIME_DIR`. NOT `DefaultEnvironment` in
    # `user.conf`: specifiers are not resolved there, and systemd drops the
    # line ("Failed to resolve specifiers … Invalid slot").
    environment.etc."environment.d/60-ronoctua-docker-host.conf".text = ''
      DOCKER_HOST="unix://''${XDG_RUNTIME_DIR}/podman/podman.sock"
    '';

    environment.sessionVariables.RONOCTUA_PODMAN = podman;
    systemd.user.services = {
      ronoctua-footer.environment.RONOCTUA_PODMAN = podman;
    }
    # AND THE QUEUE's: a pull, a build or a compose `up` is a job it runs,
    # and its unit's environment is its own. Only where the unit exists:
    # naming a unit nobody defined — even under a `mkIf` that is off — makes
    # one, with no `ExecStart`. `or false`: a system without the queue's
    # module (the containers' own VM test) has no such option at all.
    // lib.optionalAttrs (config.ronoctua.transfers.enable or false) {
      ronoctua-transfers.environment.RONOCTUA_PODMAN = podman;
    };

    assertions = [
      {
        assertion = lib.all (u: u.autoSubUidGidRange || u.subUidRanges != [ ]) (lib.attrValues people);
        message =
          "ronoctua.containers: rootless Podman needs subordinate ids for every person; "
          + "these have none: "
          + lib.concatStringsSep ", " (
            lib.attrNames (lib.filterAttrs (_: u: !u.autoSubUidGidRange && u.subUidRanges == [ ]) people)
          );
      }
    ];

    # THE CONTAINERS' OWN SUBVOLUME, per person, beside `~/.cache`'s.
    ronoctua.filesystem.ownSubvolumes.containers = ".local/share/containers";
  };
}
