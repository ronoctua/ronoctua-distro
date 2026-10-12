# /private — the person's encrypted folder, opened only when they ask.
#
# WHAT IT IS
# ----------
# A sparse LUKS2 image (`/var/lib/ronoctua-private/private.img`) on a btrfs
# subvolume of its own (`@private`, NOCOW), attached to a loop with direct
# I/O, unlocked by its own passphrase (or a FIDO2 key enrolled beside it) and
# mounted at `/private` with an inner btrfs that gives deleted blocks back
# (`discard=async` → the loop punches holes in the image). Everything that
# runs as root is `ronoctua-private-root` (crate `ronoctua-private`), from
# the four units below and nothing else.
#
# WHAT IT IS NOT
# --------------
# No fstab line and no crypttab line, ever: `/private` is never opened at
# boot (`private-never-mounts-at-boot`). No polkit rule: the person's gesture
# goes through the sudo door (`RootVerb::PrivateRoot`), which starts the ROOT
# unit by its fixed name; the request travels through a folder the person
# owns, and the unit validates it (`private-root-is-narrow`).
#
# THE FOUR UNITS
# --------------
# - `ronoctua-private-root.service` — one staged request (create, open,
#   close, enrol or forget a key, a new passphrase, a larger cap). Started
#   only by the door.
# - `ronoctua-private-lock.service` — before sleep: closed, or suspended
#   (`luksSuspend`: the key wiped, its I/O frozen) when something holds it.
# - `ronoctua-private-status.service` — at boot (and after a switch): the
#   status the doors read, the one fact they cannot (is a key enrolled?)
#   included, since the header is root's.
# - `ronoctua-private-subvolume.service` — on a machine installed before the
#   vault existed, `@private` is made before its mount needs it.
#
# NONE OF THEM HAS A MOUNT NAMESPACE OF ITS OWN (but the subvolume's, which
# mounts the top of the disk only for itself): `/private` has to appear for
# everybody, and `ProtectSystem`, `PrivateTmp`, `ProtectHome`,
# `ReadWritePaths` or `ProtectKernelTunables` would each give the unit a
# namespace and keep the mount inside it. So would `PrivateNetwork`: since
# systemd 254 it brings `PrivateMounts` along unless that is said `no` — and
# a vault mounted in the unit's own copy of the tree is a vault nobody sees,
# whose mount dies with the unit while its mapping stays. The three say
# `PrivateMounts = false` outright, and keep the network out by the address
# families they may open (`private-root-is-narrow`).
{
  config,
  lib,
  pkgs,
  utils,
  ...
}:

let
  cfg = config.ronoctua.private;
  user = config.ronoctua.user;
  group = config.users.users.${user}.group;
  helper = "${cfg.helper}/bin/ronoctua-private-root";

  # THE NAMES, as the crate has them (`ronoctua_private::*`): the check
  # `private-root-is-narrow` holds the two sides together.
  rootUnit = "ronoctua-private-root";
  lockUnit = "ronoctua-private-lock";
  statusUnit = "ronoctua-private-status";
  subvolumeUnit = "ronoctua-private-subvolume";
  imageDir = "/var/lib/ronoctua-private";
  mountPoint = "/private";
  runDir = "/run/ronoctua-private";
  staging = "${runDir}/staged";

  # THE PROGRAMS, by store path, through the units' environment — the root
  # side refuses one that is not handed in rather than guessing it by name
  # (`ronoctua_private::root::Tools::from_env`). Through the environment and
  # not `option_env!`: the workspace then carries no reference to any of
  # them, and the extracted binary none either.
  tools = {
    RONOCTUA_CRYPTSETUP = "${pkgs.cryptsetup}/bin/cryptsetup";
    RONOCTUA_LOSETUP = "${pkgs.util-linux}/bin/losetup";
    RONOCTUA_CRYPTENROLL = "${config.systemd.package}/bin/systemd-cryptenroll";
    RONOCTUA_CRYPTSETUP_ATTACH = "${config.systemd.package}/bin/systemd-cryptsetup";
    RONOCTUA_MKFS_BTRFS = "${pkgs.btrfs-progs}/bin/mkfs.btrfs";
    RONOCTUA_BTRFS = "${pkgs.btrfs-progs}/bin/btrfs";
    RONOCTUA_MOUNT = "${pkgs.util-linux}/bin/mount";
    RONOCTUA_UMOUNT = "${pkgs.util-linux}/bin/umount";
  };

  # WHAT EVERY ONE OF THE THREE ROOT UNITS KEEPS: no new privileges, nothing
  # but the local and netlink sockets (cryptsetup waits on udev over
  # netlink; no IP socket can be opened at all) — and, said above, NO MOUNT
  # NAMESPACE: `PrivateMounts` written `false`, so nothing that would bring
  # one along (`PrivateNetwork`, a namespace path) can do it quietly.
  narrow = {
    Type = "oneshot";
    Environment = lib.mapAttrsToList (name: value: "${name}=${value}") tools;
    PrivateMounts = false;
    NoNewPrivileges = true;
    RestrictAddressFamilies = [
      "AF_UNIX"
      "AF_NETLINK"
    ];
    RestrictRealtime = true;
    LockPersonality = true;
  };

  # THE `@private` MOUNT, when this machine has one (the distro's layout:
  # `nix/disko/default.nix`). The VM and a machine of another layout have
  # none, and then there is nothing for the subvolume unit to make.
  subvolumeMount = config.fileSystems.${imageDir} or null;

  # THE SUBVOLUME, made once on a machine installed before the vault. The
  # top of the btrfs (subvolid 5) is mounted where only this unit sees it
  # (`PrivateMounts`), `@private` created there NOCOW, and the top unmounted
  # again; the mount that waits for this unit then finds it.
  makeSubvolume = pkgs.writeShellApplication {
    name = "ronoctua-private-subvolume";
    runtimeInputs = [
      pkgs.util-linux
      pkgs.btrfs-progs
      pkgs.e2fsprogs
    ];
    text = ''
      device=$1
      top=$RUNTIME_DIRECTORY/top
      mkdir -p "$top"
      mount -t btrfs -o subvolid=5 "$device" "$top"
      trap 'umount "$top"' EXIT
      if [ ! -d "$top/@private" ]; then
        btrfs subvolume create "$top/@private"
        echo "ronoctua: made @private on $device"
      fi
      # AT EVERY BOOT, on the subvolume itself before it is mounted — never
      # by `tmpfiles`, which runs after `local-fs.target` and a `nofail`
      # mount is not ordered before that: it would lay these on the folder
      # underneath. NOCOW for every file made in it (the image is rewritten
      # in place, and copy-on-write under a loop is fragmentation without
      # end); `0711`, its names not listable and the image stat-able by the
      # doors. The root side lays both again when it makes the image.
      chattr +C "$top/@private"
      chmod 0711 "$top/@private"
    '';
  };
in
{
  options.ronoctua.private = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        The person's encrypted folder at `/private`: a sparse LUKS2 image
        opened only when they ask (`LEADER s e o`), locked at sleep. Nothing
        exists until the person creates it from the security centre.
      '';
    };

    helper = lib.mkOption {
      type = lib.types.package;
      description = ''
        The extracted root side (`rust.privateRoot`), with
        `bin/ronoctua-private-root`. Set by the flake next to this module's
        import; a test sets it the same way.
      '';
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        systemd.tmpfiles.rules = [
          # WHERE ROOT WRITES WHAT THE DOORS READ, root's: a person who could
          # plant a link here would have root write through it.
          "d ${runDir} 0755 root root -"
          # WHERE THE PERSON WRITES WHAT THEY WANT DONE: `0700` and theirs —
          # nobody else may put a request where a unit that runs as root
          # will read it.
          "d ${staging} 0700 ${user} ${group} -"
        ]
        # THE IMAGE'S FOLDER: its names not listable, the image (root's
        # `0600`) stat-able — the doors read how much it takes with a `stat`,
        # never by asking root. NOCOW for what is made in it. By `tmpfiles`
        # only where it is a plain folder; where it is the `@private` mount,
        # the subvolume unit lays both on the mount's root before it is
        # mounted (a `tmpfiles` line would race the `nofail` mount).
        ++ lib.optionals (subvolumeMount == null) [
          "d ${imageDir} 0711 root root -"
          "h ${imageDir} - - - - +C"
        ]
        ++ [
          # `/private` UNDERNEATH: `0000` and immutable, so nothing lands in
          # it in clear while the vault is locked. AT BOOT ONLY (`!`): a
          # switch runs tmpfiles again, and with the vault mounted these two
          # lines would close the person's own open folder to them.
          "d! ${mountPoint} 0000 root root -"
          "h! ${mountPoint} - - - - +i"
        ];

        # THE ONE THE DOOR STARTS. Never wanted by anything: it runs when
        # the person's gesture starts it, and only then.
        systemd.services.${rootUnit} = {
          description = "Opens, closes or changes the /private vault, as the person staged it";
          unitConfig.RequiresMountsFor = [ imageDir ];
          serviceConfig = narrow // {
            ExecStart = "${helper} run ${staging}";
            # Root's own `0700` folder for the two programs that take a
            # passphrase only as a file; gone when the unit stops.
            RuntimeDirectory = "ronoctua-private-root";
            RuntimeDirectoryMode = "0700";
            # A FIDO2 key waits for a touch, argon2id costs seconds: room for
            # both, and never forever.
            TimeoutStartSec = "5min";
          };
        };

        # BEFORE SLEEP. A oneshot without `RemainAfterExit`, so it runs at
        # every suspend (a unit that stayed active would be skipped the
        # second time); `sleep.target` is never active at a switch.
        systemd.services.${lockUnit} = {
          description = "Locks the /private vault before sleep";
          wantedBy = [ "sleep.target" ];
          before = [ "sleep.target" ];
          serviceConfig = narrow // {
            ExecStart = "${helper} lock";
            TimeoutStartSec = "1min";
          };
        };

        # THE FIRST STATUS, at boot — and again after every switch, which
        # starts it again; it only reads and writes the status file.
        #
        # AFTER THE IMAGE'S MOUNT, which `local-fs.target` does not wait for
        # (it is `nofail`): read before it, the image is not there yet and the
        # status says "absent" — both doors would offer to create a vault
        # that exists.
        systemd.services.${statusUnit} = {
          description = "Says what state the /private vault is in";
          wantedBy = [ "multi-user.target" ];
          after = [
            "local-fs.target"
            "systemd-tmpfiles-setup.service"
          ];
          unitConfig.RequiresMountsFor = [ imageDir ];
          serviceConfig = narrow // {
            ExecStart = "${helper} status";
          };
        };
      }

      (lib.mkIf (subvolumeMount != null) {
        # BEFORE ITS MOUNT, which the layout ties to it
        # (`x-systemd.requires=`, with `nofail`: a machine that could not make
        # it still boots, and the root side refuses to put an image anywhere
        # but on that mount).
        systemd.services.${subvolumeUnit} = {
          description = "Makes the @private subvolume on a machine installed before it existed";
          unitConfig.DefaultDependencies = false;
          after = [ "${utils.escapeSystemdPath subvolumeMount.device}.device" ];
          requires = [ "${utils.escapeSystemdPath subvolumeMount.device}.device" ];
          before = [
            "${utils.escapeSystemdPath imageDir}.mount"
            "shutdown.target"
          ];
          conflicts = [ "shutdown.target" ];
          # A switch on a live machine starts it with the new mount; the boot
          # finds it done and it says nothing.
          restartIfChanged = false;
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = "${makeSubvolume}/bin/ronoctua-private-subvolume ${lib.escapeShellArg subvolumeMount.device}";
            RuntimeDirectory = subvolumeUnit;
            PrivateMounts = true;
            PrivateNetwork = true;
            NoNewPrivileges = true;
          };
        };
      })
    ]
  );
}
