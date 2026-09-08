# Disks, partitions and pendrives — the file manager sidebar's other half.
#
# WHAT IS PRIVILEGED HERE: NOTHING OF OURS.
# -----------------------------------------
# This module writes no polkit rule, and that is the design rather than an
# omission. udisks2 ships its own actions and they already split on
# `HintSystem`, which is a udev-derived fact: `filesystem-mount` is
# `allow_active: yes` for a removable device, and `filesystem-mount-system` is
# `auth_admin_keep` for an internal one. Ejecting and powering off a removable
# drive are `yes` too. Read out of the built package, not assumed.
#
# `files.nix`, `rescue.nix` and `memory.nix` all write a narrow rule BECAUSE THE
# UNIT IS OURS and nobody else grants it. Here the door belongs to udisks and it
# is already the right width — a rule of ours could only WIDEN it (turning
# `filesystem-mount-system` into `yes` is passwordless mounting of internal
# disks), and it would be a second place to keep in step with somebody else's
# action ids across a nixpkgs bump. polkit parses its rules at runtime, so a
# stale action id is silent. `disks-adds-no-polkit-rule-of-its-own` is what
# keeps it this way, in both directions.
#
# AND THERE IS NO AUTHENTICATION AGENT IN THIS SESSION.
# -----------------------------------------------------
# The only agent in the distro is `pkttyagent`, on the rescue console
# (`nix/rust.nix`, read by `ronoctua-rescue` alone); `pkexec` brings its own to
# the terminal window the file manager opens. So an action that resolves to
# `auth_admin_keep` has nobody to ask — which is why the sidebar does not offer
# to mount a system volume at all, and says so instead, offering the root
# terminal it already has. A graphical agent would be a floating password
# dialog, and invariant 14 refuses one.
#
# Every call carries `auth.no_user_interaction`, which is what turns that case
# from a row that hangs into a refusal with a name.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.disks;

  # THE TOOL LIST IS THE CLOSURE, and this is the one place it is touched.
  #
  # nixpkgs patches udisksd to OVERWRITE its own `PATH` with a `makeBinPath` of
  # the tools it was built against (`force-path.patch`), so that list is exactly
  # what udisks can reach and exactly what the system pays for.
  slim = pkgs.udisks.override {
    # MEASURED, not assumed — the trap `AGENTS.md` records as "meça com
    # `nix why-depends`". `xfsprogs-bin` is 305,4 MiB and `nix why-depends
    # --all` gives one path to it: `udisks → xfsprogs-bin → python3-env`, which
    # is 258,0 MiB of Python for `xfs_scrub`. Set arithmetic over the closure
    # says 18 paths and 212 MiB are reachable through it and nothing else —
    # against 40 MiB for `libblockdev` and under 1 MiB each for `mdadm`,
    # `f2fs-tools` and `nilfs-utils`, which is why this is the only override
    # here and not the first of five.
    #
    # THIS DISTRO IS BTRFS, and no pendrive in the world is XFS.
    #
    # AND IT CANNOT STOP A MOUNT: mounting XFS is a kernel operation, and
    # `mount -t xfs` comes from util-linux, which stays. What leaves with it is
    # checking, repairing, resizing and formatting XFS — four verbs the sidebar
    # does not have and, per `disks-tools-are-pinned`, cannot gain without this
    # line being read again.
    #
    # `emptyDirectory` and not a stub package: `makeBinPath` yields a `PATH`
    # entry that does not exist, which `execvp` skips.
    xfsprogs = pkgs.emptyDirectory;
  };
in
{
  options.ronoctua.disks.enable = lib.mkOption {
    type = lib.types.bool;
    # DEFAULT TRUE IN BOTH PROFILES, and deliberately absent from
    # `nix/modules/profile.nix`. That file is a table of APPLICATIONS — what
    # `minimal` drops is what one installs to USE. A file manager that cannot
    # see a pendrive is a machine one cannot get a file onto, and `minimal`
    # exists precisely to be installed fast and added to afterwards.
    default = true;
    description = ''
      The device half of the file manager's sidebar:
      disks, partitions and pen drives, mounted and unmounted.

      Switching it off leaves the manager unable to see any disk at all. Nothing else in the
      system depends on this.
    '';
  };

  config = lib.mkIf cfg.enable {
    services.udisks2 = {
      enable = true;
      package = slim;
      # `/run/media/$USER/`, which is a tmpfs and ACL-controlled. `/media` is
      # not cleaned up on its own and leaves stale mount points behind.
      mountOnMedia = false;
    };

    # THE KERNEL DRIVERS — AND THIS SWITCH DOES LESS THAN IT LOOKS LIKE.
    #
    # It is worth saying out loud, because a line that quietly does nothing is
    # the shape this project has paid for more than once. `udisksd` overwrites
    # its own `PATH` (see `slim` above), so `system.fsPackages` — which is what
    # this option fills — NEVER REACHES IT. exfat and NTFS mount through the
    # helpers already inside udisks' own closure (`exfat`, `ntfs3g`), with or
    # without this line.
    #
    # It stays for the other path, which is real: `mount /dev/sdb1 /mnt` typed
    # by a person in the terminal, and any future `fileSystems.*` entry, both
    # use the SYSTEM's `PATH`. Its closure cost is zero, because both packages
    # are already there by way of udisks.
    boot.supportedFilesystems = {
      exfat = true;
      ntfs = true;
    };
  };
}
