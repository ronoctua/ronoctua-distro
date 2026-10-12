# Disk layout — section 7.2, literally.
#
#   p1   1 GiB   ESP (FAT32)          -> /boot
#   p2   rest    LUKS2
#        └─ btrfs
#           @root      -> /            compress=zstd, noatime
#           @home      -> /home
#           @nix       -> /nix         compress=zstd, noatime
#           @persist   -> /persist
#           @log       -> /var/log
#           @snapshots -> /.snapshots
#           @swap      -> swapfile, NOCOW, no compression, >= RAM
#           @private   -> /var/lib/ronoctua-private  (the /private vault's image)
#
# Consumed through `nix/modules/host.nix`, which hands it `ronoctua.disk` and
# `ronoctua.swapSize`; the installer writes those into the machine's
# `host.nix` and runs disko on that flake. It NEVER interprets, generates or
# reimplements Nix logic. Invariant 5.
{
  # `ronoctua.disk`, from the machine's host.nix (outside the repository,
  # invariant 13).
  device ? "/dev/vda",
  # `ronoctua.swapSize`: the machine's RAM, for hibernation (section 7.2).
  # Every caller passes it; the default is the size every machine had before.
  swapSize ? "16G",
  # File holding the LUKS passphrase.
  #
  # NOT an interactive prompt, on purpose. Section 7.1 says the TUI collects the
  # passphrase and the orchestrator "calls disko + nixos-install" — the
  # installer writes the file and then makes it disappear. Without
  # `passwordFile`, disko generates an interactive read path whose script does
  # not even pass its own shellcheck (SC2030/SC2031: `password` modified in a
  # subshell).
  #
  # The file NEVER enters the repository (section 9) and does not survive the
  # installation: the passphrase exists only in the person's head — the TPM
  # holds nothing for the disk (see the LUKS settings below).
  passwordFile ? "/tmp/disko-luks.key",
  ...
}:

{
  disko.devices = {
    disk.main = {
      inherit device;
      type = "disk";
      content = {
        type = "gpt";
        partitions = {
          # The ESP is 1 GiB, not 512 MiB: NixOS keeps kernel+initrd for
          # several generations in there (section 7.2). The generation limit
          # lives in nix/modules/base.nix.
          ESP = {
            priority = 1;
            name = "ESP";
            start = "1M";
            end = "1G";
            type = "EF00";
            content = {
              type = "filesystem";
              format = "vfat";
              mountpoint = "/boot";
              mountOptions = [
                "umask=0077" # the ESP holds the signed UKI
              ];
            };
          };

          luks = {
            size = "100%";
            content = {
              type = "luks";
              name = "cryptroot";
              inherit passwordFile;
              # THE PASSPHRASE IS THE KEY, AND THE TPM HOLDS NOTHING.
              #
              # This asked the TPM to unlock the volume — `tpm2-device=auto`
              # with `tpm2-measure-pcr=yes`, sealed by the installer against
              # PCRs 7+11+12+15. It is gone, and the reason is that the
              # sealing could not do what it said.
              #
              # PCR 15 is what makes a sealing refuse a FORGED volume, and it
              # is extended only when the boot came from a MEASURED UKI
              # (`ConditionSecurity=measured-uki`) — which needs
              # `systemd-stub`, which needs lanzaboote, which needs Secure
              # Boot. That chain is not installed here, so 15 stayed at zero
              # and so did 11 and 12: the sealing matched every time, for any
              # volume, and what was left of it was PCR 7. `tests/tpm-vm.nix`
              # says the same thing from the other side, out of a boot log.
              #
              # So the machine had the CONVENIENCE of an automatic unlock and
              # none of the protection meant to pay for it — and with the
              # session's autologin, a laptop taken powered off would have
              # opened straight into the desktop.
              #
              # The decision, made deliberately: the disk asks for its
              # passphrase once, at boot, and nothing is asked after it. The
              # owner's words — "se eu já preciso digitar a senha para
              # descriptografar o disco, então nenhuma outra senha é
              # necessária após isso". Secure Boot and the TPM unlock are
              # both off, and `docs/ARCHITECTURE.md` carries the argument.
              #
              # NOT TO BE CONFUSED WITH THE WI-FI KEY, which IS sealed in the
              # TPM and stays — a different mechanism (`systemd-creds`,
              # deliberately with no PCRs) and a different question.
              settings = {
                allowDiscards = true;
              };
              content = {
                type = "btrfs";
                extraArgs = [ "-f" ];
                subvolumes = {
                  "@root" = {
                    mountpoint = "/";
                    mountOptions = [
                      "compress=zstd"
                      "noatime"
                    ];
                  };
                  "@home" = {
                    mountpoint = "/home";
                    mountOptions = [
                      "compress=zstd"
                      "noatime"
                    ];
                  };
                  "@nix" = {
                    mountpoint = "/nix";
                    mountOptions = [
                      "compress=zstd"
                      "noatime"
                    ];
                  };
                  "@persist" = {
                    mountpoint = "/persist";
                    mountOptions = [
                      "compress=zstd"
                      "noatime"
                    ];
                  };
                  "@log" = {
                    mountpoint = "/var/log";
                    mountOptions = [
                      "compress=zstd"
                      "noatime"
                    ];
                  };
                  # Whatever snapshots land here may never be OF `/nix` or of a
                  # cache: `nix/modules/filesystem.nix` refuses such a target.
                  "@snapshots" = {
                    mountpoint = "/.snapshots";
                    mountOptions = [
                      "compress=zstd"
                      "noatime"
                    ];
                  };
                  # Swapfile for hibernation (section 7.2).
                  #
                  # WATCH OUT: `nodatacow` in mountOptions does NOT work here, and
                  # the previous version of this file listed it, giving a false
                  # impression of a guarantee. On btrfs, `nodatacow` and `compress`
                  # are FILESYSTEM options, not subvolume ones: the first mount
                  # wins and the rest are ignored in silence. The VM test showed
                  # /swap mounted with `compress=zstd:3` and without `nodatacow`,
                  # despite both being asked for.
                  #
                  # What guarantees hibernation is the file's ATTRIBUTE: NOCOW and
                  # no compression. That is done by `btrfs filesystem mkswapfile`,
                  # which is what disko uses. The test asserts the attribute
                  # (`lsattr`), not the mount options.
                  #
                  # The resume_offset is computed by the installer on the spot
                  # (section 7.1).
                  "@swap" = {
                    mountpoint = "/swap";
                    mountOptions = [ "noatime" ];
                    swap.swapfile.size = swapSize;
                  };
                  # THE 2026-10-05 WAVE: one region per package, after its anchor.
                  # ── anchor: L12 ──
                  # THE /private VAULT'S IMAGE (`nix/modules/private.nix`): a
                  # subvolume of its own, so a snapshot of `@root` never holds
                  # it and the wipe never takes it. NOCOW comes from the
                  # folder's attribute (tmpfiles `+C`), never from a mount
                  # option — on btrfs the first mount's win, as `@swap` above
                  # says. `nofail` and the subvolume unit: a machine installed
                  # before this line existed has no `@private` yet, and the
                  # unit makes it before the mount needs it — never a boot
                  # stopped for want of it.
                  "@private" = {
                    mountpoint = "/var/lib/ronoctua-private";
                    mountOptions = [
                      "noatime"
                      "nofail"
                      "x-systemd.requires=ronoctua-private-subvolume.service"
                    ];
                  };
                  # ── anchor: L13 ──
                  # ── anchor: L20 ──
                };
              };
            };
          };
        };
      };
    };
  };
}
