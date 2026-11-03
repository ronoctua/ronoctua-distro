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
    # the boot trial on its boot counting. A machine that starts from BIOS — most
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
      # THE BOOT TRIAL IS SYSTEMD-BOOT'S BOOT COUNTING: every new entry is
      # written with ONE try (`<id>+1.conf`), and `loader.conf` prefers the
      # profile's generation and falls back to the newest entry that is not
      # bad. `ronoctua-machine` arms the trial in those files — `default` on
      # the last good entry, the new one `preferred` with its try — and
      # blesses an entry by taking its counter off
      # (`crates/ronoctua-machine/src/loader.rs`). One try, because the trial
      # is a boot of once: an entry nobody blessed is bad at the boot after,
      # and that boot is the last good system's. A trial systemd-boot cannot
      # start restarts the machine on its own: a counted entry's
      # `reboot-on-error` is on by default.
      bootCounting = {
        enable = true;
        tries = 1;
      };
    };
    # THE MACHINE NEVER WRITES AN EFI VARIABLE WHILE IT RUNS. A write is a
    # call into the firmware, and a firmware may answer it badly: on a
    # ThinkPad Z16 Gen 2 every one — a write or a removal, by `bootctl`,
    # `efibootmgr` or the kernel — reset the machine two to five seconds
    # later (2026-11-03). So:
    #
    # - the boot trial is in systemd-boot's files, above;
    # - `bootctl` registers nothing in the firmware's boot menu when systemd-
    #   boot is updated (`--variables=no`): the entry the installation made,
    #   or the fallback `\EFI\BOOT\BOOTX64.EFI` it always keeps, starts the
    #   machine;
    # - the variables are mounted READ-ONLY early in stage 2 — what still
    #   tries to write one (`systemd-sleep`'s `HibernateLocation`, which
    #   `resume=` makes needless; a `systemctl reboot --firmware-setup`) is
    #   refused instead of resetting the machine;
    # - the kernel keeps no crash report in them (`efi_pstore`): it writes one
    #   on a panic through its own way in, and the boot after deletes it.
    #
    # Reading them — `LoaderEntrySelected`, `LoaderBootCountPath`, Secure
    # Boot's state — is what a read-only mount still allows.
    boot.loader.efi.canTouchEfiVariables = false;
    systemd.services.ronoctua-efi-variables-read-only = lib.mkIf (cfg.firmware == "uefi") {
      description = "Mounts the EFI variables read-only";
      wantedBy = [ "sysinit.target" ];
      before = [ "sysinit.target" ];
      unitConfig = {
        DefaultDependencies = false;
        ConditionPathIsMountPoint = "/sys/firmware/efi/efivars";
      };
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.util-linux}/bin/mount -o remount,ro /sys/firmware/efi/efivars";
      };
      restartIfChanged = false;
    };
    # The verdict is the machine's own, after the default target and a while
    # (`ronoctua-boot-bless.service`): systemd's would bless an entry at
    # `boot-complete.target`, a few seconds into the boot.
    systemd.services.systemd-bless-boot.enable = lib.mkIf (cfg.firmware == "uefi") false;
    # GRUB ON A BIOS MACHINE, into the disk's first sectors and its BIOS boot
    # partition (`nix/disko/default.nix`); the disk is the one disko lays out,
    # which hands it to GRUB. `/boot` is the same FAT partition the ESP is on
    # UEFI.
    #
    # THE BOOT TRIAL'S DEFAULT IS A VARIABLE OF OURS, `ronoctua_default` in
    # GRUB's environment block, read after NixOS's own choice and never on a
    # boot of once (`next_entry`, the trial itself): `ronoctua-machine` writes
    # the two, as it writes `loader.conf`'s `default` and `preferred` on
    # UEFI. NOT NixOS's `default = "saved"`, which puts
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
    # AND A TRIAL GRUB CANNOT START FALLS BACK IN THE SAME BOOT. The deadline
    # and the bless see only a kernel that ran; a kernel GRUB cannot load — a
    # file it cannot read, or no kernel at all — returns to GRUB's menu, the
    # boot of once spent, the menu's timeout gone, and the machine would wait
    # there for a person. (An INITRD that does not load stops nothing: GRUB
    # starts the kernel without it, which panics, and `panic=10` and the
    # trial bring the machine back.) GRUB's
    # `fallback` is what it boots instead, and it takes only the NUMBER of an
    # entry of the menu the failure returns to (`grub-core/normal/menu.c`):
    # the top one, while the trial is inside a submenu, where no entry boots
    # the last good system — `default` still names the trial. So on a boot
    # of once with a trial armed, and only then, the menu's first entry is
    # ours, named as the last good generation is (its title is the last part
    # of the path), and it reads the menu again as a boot that tries
    # nothing: `next_entry` is already spent, so `ronoctua_default` is the
    # default, and the mark is unset first. NOT exported: a submenu's own
    # `fallback` would name an entry of the submenu. And the reading again
    # says it is one (`ronoctua_fell`, exported into it): were `next_entry`
    # not spent — the block could not be written —, it would be a boot of
    # once again, and fall back into itself until GRUB ran out of memory.
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
        if [ -n "''${ronoctua_fell}" ]; then
          set boot_once=
        fi
        if [ -z "''${boot_once}" ]; then
          if [ -n "''${ronoctua_default}" ]; then
            set default="''${ronoctua_default}"
          fi
        elif [ -n "''${ronoctua_default}" ]; then
          set ronoctua_trial=${mark}
          export ronoctua_trial
          set ronoctua_back="''${ronoctua_default}"
          insmod regexp
          regexp --set=1:ronoctua_back '([^>]*)$' "''${ronoctua_default}"
          menuentry "''${ronoctua_back}" --class nixos --unrestricted {
            unset ronoctua_trial
            set ronoctua_fell=1
            export ronoctua_fell
            configfile "''${prefix}/grub.cfg"
          }
          set fallback=0
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
    # GRUB's trial mark, expanded by GRUB at boot; and on UEFI, no crash
    # report kept in an EFI variable (above).
    boot.kernelParams =
      lib.optional (cfg.firmware == "bios") "\${ronoctua_trial}"
      ++ lib.optional (cfg.firmware == "uefi") "efi_pstore.pstore_disable=1";
  };
}
