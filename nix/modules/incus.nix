# Incus — the person's system containers and virtual machines.
#
# WHAT THE OWNER DECIDED (2026-10-05)
# -----------------------------------
# Incus is invariant 6's exception in the containers' mould (AGENTS.md): the
# daemon, its bridge, its pool and its default profile are declared here;
# what the person creates inside it — instances, images, snapshots, volumes —
# is the person's, kept in `/var/lib/incus` and never in the repository or in
# `/etc`. ON IN THE FULL PROFILE ONLY (`nix/modules/profile.nix`): QEMU, OVMF
# and LXC are a toolchain one installs to USE.
#
# WHAT IS DECLARED, AND WHY EACH
# ------------------------------
# - THE PACKAGE IS PINNED to the LTS, by name: the shell parses what this
#   daemon prints (`ronoctua-incus`, whose fixtures are its shapes), and the
#   feature line adds API extensions every month.
# - THE PRESEED, re-applied at every boot by nixpkgs' `incus-preseed`:
#   * `images.auto_update_interval = 0` — NOTHING REACHES THE NETWORK BY
#     ITSELF. The daemon's default refreshes every cached image every six
#     hours, a request to a host nobody let at that moment. Every request to
#     a remote is a person's gesture, under a consent pass (the crate).
#   * the bridge `incusbr0`, IPv4 with NAT on a FIXED subnet
#     (`ronoctua.incus.subnet`): `auto` would draw a new one at every boot,
#     since the preseed is applied again, and every instance's address with
#     it. No IPv6 on the bridge: a fixed prefix of our own would be one more
#     thing to collide with.
#   * the pool `default` — btrfs where the machine is btrfs (a subvolume per
#     instance, snapshots for free), `dir` elsewhere — at the one source path
#     the daemon accepts under its own folder.
#   * the profile `default`: a NIC on the bridge and the root disk on the
#     pool.
# - THE PERSON IS IN `incus-admin`, which is ROOT-EQUIVALENT: a member can
#   make a privileged container that mounts the host. It is the one group
#   that reaches the daemon's whole socket, and the owner chose it knowingly;
#   `incus` (the restricted socket) cannot make VMs with a console.
# - THE BRIDGE IS TRUSTED (`networking.firewall.trustedInterfaces`), which
#   the security module turns into `/etc/ronoctua-security/local-interfaces`:
#   the person's table lets the instances' DHCP and DNS reach the daemon's
#   dnsmasq even with the shields up. Incus keeps its own `inet incus` table
#   for NAT, and `security.nix`'s assertion (`flushRuleset` off) is what
#   keeps a reload of ours from flushing it.
# - NetworkManager does not manage the bridge, the instances' veths or a VM's
#   taps: each would otherwise appear as a "wired connection" it tries to
#   configure, and the network view would list it.
# - `virt-viewer` for a VM's VGA console (`incus console --type=vga` runs
#   remote-viewer).
#
# WHERE INCUS IS, FOR THE SHELL
# -----------------------------
# `RONOCTUA_INCUS` (the CLI) and `RONOCTUA_INCUS_JOB` (the queue's child),
# from the ENVIRONMENT and never baked in — the containers' reason: QEMU's
# closure would ride in every binary that links the crate, on every profile.
# Three places for the CLI (a login shell, the footer, the queue) and one for
# the job (the queue). This module is the one place either gets a value
# (`one-incus-boundary`).
#
# WHERE ITS BYTES LIVE
# --------------------
# `/var/lib/incus` survives the root's wipe (`nix/modules/persist.nix` binds
# it from `/persist`), and on btrfs `/persist/var/lib/incus` is A SUBVOLUME OF
# ITS OWN, made before the bind and before the daemon: a snapshot of
# `/persist` never descends into it, so every instance's disk and every
# image are never pinned by one — and `filesystem.nix` refuses a snapshot
# target AT that storage ("it is the instances' storage"). A VM's disk can
# only be made without copy-on-write by the daemon, which writes it: the
# crate asks for it (the volume's compression `none` on btrfs), and a folder
# flag here would reach no disk, since every volume is a subvolume, which
# takes no flag from its folder. Whether the ask reaches a VM made from an
# image is not proven (`docs/PENDING.md`).
{
  config,
  lib,
  pkgs,
  utils,
  ...
}:

let
  cfg = config.ronoctua.incus;
  incus = config.virtualisation.incus;
  cli = "${incus.clientPackage}/bin/incus";
  btrfs = config.ronoctua.filesystem.btrfs or false;
  # THE DAEMON'S FOLDER, as nixpkgs' module and the daemon itself name it.
  state = "/var/lib/incus";
  # Its persisted copy, which is a subvolume on btrfs.
  kept = "/persist${state}";
  persisted = config.ronoctua.persist.enable or false;
  bindUnit = "${utils.escapeSystemdPath state}.mount";

  # THE SUBVOLUME, before the bind and before the daemon.
  #
  # A btrfs subvolume's root directory is inode 256: that is the whole test
  # for "already one", so the unit keeps no state of its own. Five cases:
  #   - none there: made;
  #   - an empty folder (what a switch's `tmpfiles` makes from the bind's `d`
  #     line before this runs): replaced;
  #   - a folder with data: converted — copied by reflink into a new
  #     subvolume beside it, swapped in one atomic `mv --exchange`, the old
  #     folder removed — ONLY while the daemon cannot be using it (the bind is
  #     not up), and otherwise left for the next boot;
  #   - a folder that HOLDS SUBVOLUMES — Incus's own, once it has run on a
  #     btrfs pool there: its pool, every instance, image and snapshot. It
  #     stays a folder. `cp` would copy each as a plain folder under the
  #     daemon's btrfs driver, and `rm --one-file-system` cannot remove one
  #     (each has its own device number). Nothing is lost by leaving it: a
  #     snapshot of `/persist` never descends into a nested subvolume, so the
  #     instances' bytes are already outside every one, and the folder keeps
  #     only the database and the logs;
  #   - an interrupted conversion: the subvolume beside it is the draft when
  #     the folder is still the folder, and the OLD folder when the swap
  #     happened; each is finished or undone — and one that holds a
  #     subvolume is left as it is, said in the journal.
  # Not on btrfs, there is nothing to make and the folder stays a folder.
  # Every case that leaves things as they are says so as a warning (`<4>`)
  # and ends well: a failed unit would fail again at every boot, for a state
  # only a person can change.
  makeSubvolume = pkgs.writeShellApplication {
    name = "ronoctua-incus-subvolume";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.btrfs-progs
      pkgs.util-linux
      pkgs.findutils
    ];
    text = ''
      if [ $# -ne 2 ]; then
        echo "ERROR: usage: ronoctua-incus-subvolume <persisted folder> <live folder>" >&2
        exit 2
      fi
      kept=$1 live=$2
      draft="$kept.subvolume-in-progress"

      on_btrfs() { [ "$(stat --file-system --format=%T -- "$1")" = btrfs ]; }
      subvolume() { [ -d "$1" ] && [ ! -L "$1" ] && on_btrfs "$1" && [ "$(stat --format=%i -- "$1")" = 256 ]; }
      empty() { [ -z "$(find "$1" -mindepth 1 -maxdepth 1 -print -quit)" ]; }
      # A SUBVOLUME SOMEWHERE BELOW: on btrfs, inode 256 is a subvolume's
      # root and nothing else.
      holds_subvolumes() { [ -n "$(find "$1" -mindepth 1 -type d -inum 256 -print -quit)" ]; }

      mkdir -p "$(dirname "$kept")"
      if ! on_btrfs "$(dirname "$kept")"; then
        echo "ronoctua: $kept stays a folder: /persist is not btrfs"
        exit 0
      fi

      # AN INTERRUPTED CONVERSION, FIRST.
      if [ -e "$draft" ] || [ -L "$draft" ]; then
        if [ -d "$draft" ] && [ ! -L "$draft" ] && holds_subvolumes "$draft"; then
          echo "<4>ronoctua: $draft holds a subvolume, which neither a removal nor a copy here can carry; left as it is, and $kept with it"
          exit 0
        fi
        # Either way it goes whole, a subvolume as a folder: it holds none
        # (above), so `rm` empties it and removes its root.
        if subvolume "$kept"; then
          echo "ronoctua: removing the folder an interrupted conversion swapped out"
        else
          echo "ronoctua: undoing an interrupted conversion"
        fi
        rm -rf --one-file-system -- "$draft"
      fi

      if subvolume "$kept"; then
        exit 0
      fi
      if [ -L "$kept" ] || { [ -e "$kept" ] && [ ! -d "$kept" ]; }; then
        echo "ERROR: $kept is not a folder; Incus's state has nowhere to go" >&2
        exit 1
      fi
      if [ ! -e "$kept" ] || empty "$kept"; then
        if [ -e "$kept" ]; then
          rmdir -- "$kept"
        fi
        btrfs subvolume create -- "$kept"
        chmod 0711 -- "$kept"
        echo "ronoctua: $kept is a subvolume of its own"
        exit 0
      fi
      if holds_subvolumes "$kept"; then
        echo "<4>ronoctua: $kept stays a folder: it holds Incus's own subvolumes (its pool, its instances and images), which a copy would turn into plain folders; they are already outside every snapshot of /persist"
        exit 0
      fi
      if mountpoint -q -- "$live"; then
        echo "ronoctua: $kept holds data and $live is in use; the next boot converts it"
        exit 0
      fi
      btrfs subvolume create -- "$draft"
      if ! cp -a --reflink=always -- "$kept/." "$draft/"; then
        echo "<4>ronoctua: $kept could not be copied by reflink; it stays a folder"
        rm -rf --one-file-system -- "$draft"
        exit 0
      fi
      chmod --reference="$kept" -- "$draft"
      chown --reference="$kept" -- "$draft"
      mv --exchange --no-target-directory -- "$draft" "$kept"
      rm -rf --one-file-system -- "$draft"
      echo "ronoctua: $kept converted into a subvolume of its own"
    '';
  };

  # EVERY UNIT OF THE DAEMON'S waits for the bind: started before it, the
  # daemon would write into the root's folder, and the bind would then hide
  # that and show the persisted one.
  afterTheBind.unitConfig.RequiresMountsFor = [ state ];
in
{
  options.ronoctua.incus = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Incus: the person's system containers and virtual machines, its bridge
        and pool declared, nothing fetched by itself. On in the full profile
        (`nix/modules/profile.nix`).
      '';
    };

    subnet = lib.mkOption {
      type = lib.types.strMatching "([0-9]{1,3}\\.){3}[0-9]{1,3}/[0-9]{1,2}";
      default = "10.218.42.1/24";
      description = ''
        The bridge's address and the instances' network, in CIDR. Fixed, so an
        instance keeps its address across boots; change it in `host.nix` when
        the local network already uses it.
      '';
    };

    subvolumeProgram = lib.mkOption {
      type = lib.types.package;
      readOnly = true;
      internal = true;
      default = makeSubvolume;
      description = ''
        The program that makes the persisted state a subvolume of its own
        (`ronoctua-incus-subvolume <persisted> <live>`), for the VM test that
        runs it against every case it handles.
      '';
    };

    job = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The extracted queue child (`rust.incusJob`), with
        `bin/ronoctua-incus-job`. Set by the flake next to this module's
        import; a test sets it the same way.
      '';
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        assertions = [
          {
            assertion = cfg.job != null;
            message = "ronoctua.incus: the queue's job (ronoctua.incus.job) is not set";
          }
        ];

        virtualisation.incus = {
          enable = true;
          package = pkgs.incus-lts;
          preseed = {
            config."images.auto_update_interval" = "0";
            networks = [
              {
                name = "incusbr0";
                type = "bridge";
                description = "The instances' network";
                config = {
                  "ipv4.address" = cfg.subnet;
                  "ipv4.nat" = "true";
                  "ipv6.address" = "none";
                };
              }
            ];
            storage_pools = [
              {
                name = "default";
                driver = if btrfs then "btrfs" else "dir";
                config.source = "${state}/storage-pools/default";
              }
            ];
            profiles = [
              {
                name = "default";
                description = "A NIC on incusbr0 and the root disk on the default pool";
                devices = {
                  eth0 = {
                    name = "eth0";
                    network = "incusbr0";
                    type = "nic";
                  };
                  root = {
                    path = "/";
                    pool = "default";
                    type = "disk";
                  };
                };
              }
            ];
          };
        };

        users.users.${config.ronoctua.user}.extraGroups = [ "incus-admin" ];

        networking.firewall.trustedInterfaces = [ "incusbr0" ];
        networking.networkmanager.unmanaged = [
          "interface-name:incusbr*"
          "interface-name:veth*"
          "interface-name:tap*"
        ];

        environment.systemPackages = [ pkgs.virt-viewer ];

        environment.sessionVariables.RONOCTUA_INCUS = cli;
        systemd.user.services = {
          ronoctua-footer.environment.RONOCTUA_INCUS = cli;
        }
        # THE QUEUE's, and only where its unit exists (`containers.nix`'s
        # reason: naming a unit nobody defined makes one with no
        # `ExecStart`).
        // lib.optionalAttrs (config.ronoctua.transfers.enable or false) {
          ronoctua-transfers.environment = {
            RONOCTUA_INCUS = cli;
            RONOCTUA_INCUS_JOB = "${cfg.job}/bin/ronoctua-incus-job";
          };
        };
      }

      # THE PERSISTED STATE, a subvolume of its own on btrfs, before anything
      # of the daemon's starts.
      (lib.mkIf persisted {
        systemd.services = {
          ronoctua-incus-subvolume = lib.mkIf btrfs {
            description = "Makes Incus's persisted state a btrfs subvolume of its own";
            wantedBy = [
              bindUnit
              "local-fs.target"
            ];
            before = [
              bindUnit
              "ronoctua-persist-seed.service"
              "local-fs.target"
            ];
            unitConfig = {
              # BEFORE THE ORDINARY ORDER EXISTS, as the seed: the bind is
              # part of `local-fs.target`.
              DefaultDependencies = false;
              RequiresMountsFor = [ "/persist" ];
            };
            # A switch never restarts it under a running daemon; it runs
            # again whenever the bind is started.
            restartIfChanged = false;
            serviceConfig = {
              Type = "oneshot";
              ExecStart = "${makeSubvolume}/bin/ronoctua-incus-subvolume ${
                lib.escapeShellArgs [
                  kept
                  state
                ]
              }";
            };
          };
          incus = afterTheBind;
          incus-user = afterTheBind;
          incus-startup = afterTheBind;
          incus-preseed = afterTheBind;
        };
        systemd.sockets = {
          incus = afterTheBind;
          incus-user = afterTheBind;
        };
      })
    ]
  );
}
