# THE BOOT LOADER, by how the firmware starts the machine — section 7.2:
# systemd-boot, not GRUB, on UEFI; GRUB on BIOS, where systemd-boot (an EFI
# program) cannot run. Imported by `base.nix`, and by the VM tests of the
# boot trial (`tests/boot-trial-vm.nix`) and of the disk's layout
# (`tests/disko-vm.nix`), on both firmwares, so they boot exactly the loaders
# a machine has.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua;
  mark = import ../boot-trial-mark.nix;
in
{
  options.ronoctua = {
    # HOW THE FIRMWARE STARTS THIS MACHINE, and so which boot loader it has.
    #
    # UEFI is the default and the better one: systemd-boot (section 7.2) and
    # the boot trial on EFI variables. A machine that starts from BIOS — most
    # often one whose setup keeps the legacy mode (CSM) on — has no EFI to
    # run systemd-boot from, so it boots GRUB,
    # the loader BIOS boots, with the same boot trial on GRUB's environment
    # block (`crates/ronoctua-machine/src/loader.rs`). The installer detects
    # it, recommends UEFI and installs either; `host.nix` says `bios`.
    firmware = lib.mkOption {
      type = lib.types.enum [
        "uefi"
        "bios"
      ];
      default = "uefi";
      description = "How the firmware starts this machine: `uefi` (systemd-boot) or `bios` (GRUB).";
    };
  };

  config = {
    # -----------------------------------------------------------------
    # Boot — section 7.2: systemd-boot, not GRUB — on UEFI. On BIOS, GRUB
    # (`ronoctua.firmware`, above): systemd-boot is an EFI program.
    # -----------------------------------------------------------------
    boot.loader.systemd-boot = {
      enable = lib.mkDefault (cfg.firmware == "uefi");
      # The ESP is 1 GiB (section 7.2) because NixOS keeps kernel+initrd for
      # several generations in there. The cap is `nix/modules/store.nix`'s: the
      # menu lists no more generations than the system keeps.
      editor = false; # an editable cmdline at boot destroys the PCR 12 measurement
    };
    boot.loader.efi.canTouchEfiVariables = lib.mkDefault (cfg.firmware == "uefi");
    # GRUB ON A BIOS MACHINE, into the disk's first sectors and its BIOS boot
    # partition (`nix/disko/default.nix`); the disk is the one disko lays out,
    # which hands it to GRUB. `/boot` is the same FAT partition the ESP is on
    # UEFI.
    #
    # THE BOOT TRIAL'S DEFAULT IS A VARIABLE OF OURS, `ronoctua_default` in
    # GRUB's environment block, read after NixOS's own choice and never on a
    # boot of once (`next_entry`, the trial itself): `ronoctua-machine` writes
    # the two, as `bootctl` writes `LoaderEntryDefault` and
    # `LoaderEntryOneShot` on UEFI. NOT NixOS's `default = "saved"`, which puts
    # `savedefault` in every entry: an entry picked by hand at the menu would
    # stay the default for good, where systemd-boot keeps it for that boot.
    #
    # AND A BOOT THAT TRIES says so on the kernel's command line. GRUB leaves
    # Linux no variable to read (systemd-boot leaves `LoaderEntrySelected`),
    # and the trial's deadline has to know before anything mounts — `/boot`,
    # where the environment block is, may be what fails. So every entry's
    # `linux` line carries `${ronoctua_trial}` (NixOS writes the kernel's
    # parameters into it as they are, and GRUB expands a variable there at
    # boot), set to the mark only on a boot of once while a trial is armed:
    # empty, it expands to nothing. EXPORTED: an entry inside a submenu —
    # every numbered generation is — runs in a new environment that copies
    # only exported variables, and there the mark would expand to nothing.
    #
    # NO SPLASH: NixOS's GRUB shows its artwork behind the menu, and section 3
    # wants no logo.
    #
    # AND THE PROTECTIVE MBR MARKED BOOTABLE, after every install of GRUB:
    # some BIOS firmware boots a disk only when an MBR entry is active, and a
    # GPT disk's only entry is the protective one (type 0xEE, at 450), which
    # nothing marks. The mark is the entry's status byte, at 446: 0x80. GRUB's
    # code in the first sector does not read it, and `grub-install` keeps it
    # with the table. Here and not in the disk's layout: these are the disks
    # GRUB was just written to, by the names this machine gives them.
    boot.loader.grub = lib.mkIf (cfg.firmware == "bios") {
      enable = true;
      efiSupport = false;
      splashImage = null;
      extraConfig = ''
        if [ -z "''${boot_once}" ]; then
          if [ -n "''${ronoctua_default}" ]; then
            set default="''${ronoctua_default}"
          fi
        elif [ -n "''${ronoctua_default}" ]; then
          set ronoctua_trial=${mark}
          export ronoctua_trial
        fi
      '';
      extraInstallCommands = ''
        for disk in ${
          lib.escapeShellArgs (lib.filter (d: d != "nodev") config.boot.loader.grub.devices)
        }; do
          entry() { ${pkgs.coreutils}/bin/od -An -tx1 -j"$1" -N1 "$disk" | ${pkgs.coreutils}/bin/tr -d ' '; }
          if [ "$(entry 450)" = ee ] && [ "$(entry 446)" != 80 ]; then
            printf '\200' | ${pkgs.coreutils}/bin/dd of="$disk" bs=1 seek=446 count=1 conv=notrunc status=none
          fi
        done
      '';
    };
    boot.kernelParams = lib.mkIf (cfg.firmware == "bios") [ "\${ronoctua_trial}" ];
  };
}
