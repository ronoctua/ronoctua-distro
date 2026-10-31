# VM test: the disk layout of section 7.2, end to end.
#
# WHAT THIS PROVES
# ----------------
# That `nix/disko/default.nix` partitions, formats, MOUNTS and BOOTS — not
# only that Nix evaluates. The difference matters: a layout that evaluates but
# does not mount only shows up at real installation time, when the disk has
# already been wiped.
#
# WHAT THIS DOES NOT PROVE
# ------------------------
# Nothing about TPM2, Secure Boot or PCR 15: that is milestone 8, with
# `swtpm`. Here the LUKS passphrase comes from a file, on purpose — automatic
# unlocking is another test, and mixing the two would hide which of them broke.
#
# We use disko's OWN `makeDiskoTest` instead of a hand-made harness: it
# already knows how to bring the installer VM up, run the script in the right
# order and reboot to prove that the system boots from the partitioned disk.
# Rewriting that would be reimplementing disko's logic (invariant 5, in spirit).
# The one piece that is ours is the installed system's BASE (`installed`,
# below), and only until disko's `lib/tests.nix` stops reading the removed
# `boot.initrd.systemd.enable`.
#
# ON BOTH FIRMWARES (`firmware`): UEFI, and BIOS (`disko-vm-bios`), where
# the same layout carries GRUB's own partition and the machine boots GRUB
# from the disk's first sectors — the loader `nix/modules/boot-loader.nix`
# gives a machine whose `host.nix` says `bios`, which the installed system
# here imports as a machine does.
{
  pkgs,
  lib,
  disko,
  # How the VM's firmware starts it: `uefi` or `bios`.
  firmware ? "uefi",
}:

let
  bios = firmware == "bios";
  layout = if bios then ./disko-config-bios-test.nix else ./disko-config-test.nix;
  inherit (disko.lib) testLib;

  # THE INSTALLED SYSTEM'S BASE, handed to `makeDiskoTest` through its own
  # `extendModules` argument (the door disko's `module.nix` uses for
  # `installTest`).
  #
  # Left to itself, the harness builds this base from `installed-system`
  # (disko's `lib/tests.nix`, lines 148-154 at cedd626), which reads
  # `config.boot.initrd.systemd.enable` to choose between the scripted
  # initrd's `preDeviceCommands` and an initrd secret. nixos-unstable removed
  # the scripted initrd on 2026-10-27, and both options with it
  # (`mkRemovedOptionModule`: the value is a `throw`), so merely evaluating
  # that base throws, before any VM starts.
  #
  # This is that module without the question. The `/tmp/secret.key` initrd
  # secret it would plant is left out too: this layout has no key file, so the
  # installed system's initrd carries no secret, as no machine's does. That
  # system is still a bare NixOS (disko's module, the test layout, the
  # harness's instrumentation and, on BIOS, `nix/modules/boot-loader.nix`),
  # not a machine of the distro. The layout, the installer VM, the script and
  # the reboot are still disko's — and disko moved from ff8702b to cedd626
  # (new luks and gpt scripts) in 14d928c, right after ae7224b rolled systemd
  # from 260.2 to 262, so this is not the system of the last green run.
  #
  # DELETE this binding and `extendModules` below once disko's
  # `installed-system` stops reading the option. Master's head (cedd626)
  # still read it on 2026-10-30, and the open PRs #1304 and #1305 do not
  # touch `lib/tests.nix`.
  installed = import (pkgs.path + "/nixos/lib/eval-config.nix") {
    inherit (pkgs.stdenv.hostPlatform) system;
    modules = [
      {
        imports = [
          disko.nixosModules.disko
          (testLib.prepareDiskoConfig (import layout { inherit lib; }) testLib.devices)
        ];
        disko.enableConfig = true;
        documentation.enable = false;
        hardware.enableAllFirmware = lib.mkForce false;
        boot.consoleLogLevel = lib.mkForce 100;
        boot.loader.systemd-boot.enable = lib.mkDefault (!bios);
      }
    ];
  };
in
testLib.makeDiskoTest {
  inherit pkgs;
  name = "ronoctua-layout-7-2" + lib.optionalString bios "-bios";

  # The PRODUCTION layout, with a single difference: the swap's size, which in
  # production is >= RAM and here has to fit on the VM's disk. See the header of
  # disko-config-test.nix — the `makeDiskoTest` imports the file with the
  # default arguments and accepts no parameters; so BIOS is a file of its own.
  disko-config = layout;

  # The installed system's base (see `installed`, above).
  extendModules = installed.extendModules;

  # UEFI, as section 2 recommends of the target machines — or BIOS, which
  # they may still start from.
  efi = !bios;

  # It boots after installing: proof that the layout not only mounts, but
  # sustains a system.
  testBoot = true;

  # The passphrase comes from a FILE, not from a prompt — it is the same path
  # the installer uses (section 7.1: the TUI collects, the orchestrator calls
  # disko). Here the "installer" is the test VM, so it writes the file.
  extraInstallerConfig = {
    systemd.tmpfiles.rules = [
      "f /tmp/disko-luks.key 0600 root root - test-password"
    ];
  };

  # Typing the passphrase at boot is NOT a workaround — it is the test of how
  # the machine unlocks its disk: the passphrase is the key, and the crypttab
  # asks no TPM (`nix/disko/default.nix` says why). If one day the volume
  # opened with no passphrase, it would be a serious bug — and this test would
  # start failing here.

  # The INSTALLED system needs the tools the asserts use. `lsattr` comes from
  # e2fsprogs and is not in a minimal NixOS.
  extraSystemConfig = {
    environment.systemPackages = [
      pkgs.e2fsprogs
      pkgs.btrfs-progs
      pkgs.util-linux
    ];
  }
  # THE MACHINES' OWN LOADER on BIOS: GRUB, with the boot trial's lines.
  // lib.optionalAttrs bios {
    imports = [ ../nix/modules/boot-loader.nix ];
    ronoctua.firmware = "bios";
  };

  # The variable is `machine`, not `booted_machine`: disko REBINDS `machine`
  # to the VM that booted and uses "booted_machine" only as a display name in
  # the log. Using the display name fails at type check, not at runtime.
  bootCommands = ''
    machine.wait_for_console_text("Please enter passphrase")
    machine.send_console("test-password\n")
  '';

  extraTestScript = ''
    with subtest("an ESP of 1 GiB, not 512 MiB (section 7.2)"):
        # NixOS keeps kernel+initrd of SEVERAL generations on the ESP. With 512 MiB
        # it fills up after a few updates, and the symptom is a nixos-rebuild that
        # fails without explaining.
        out = machine.succeed("lsblk -b -n -o NAME,SIZE /dev/vda1")
        print(out)
        size_kib = int(out.split()[-1])
        assert 1_000_000_000 <= size_kib <= 1_200_000_000, \
            f"the ESP has {size_kib} bytes, expected ~1 GiB"

    with subtest("section 7.2's 7 btrfs subvolumes"):
        subs = machine.succeed("btrfs subvolume list /")
        print(subs)
        for sv in ["@root", "@home", "@nix", "@persist", "@log", "@snapshots", "@swap"]:
            assert sv in subs, f"the subvolume {sv} is not there"

    with subtest("zstd and noatime where 7.2 says"):
        for mp in ["/", "/nix"]:
            opts = machine.succeed(f"findmnt -n -o OPTIONS {mp}")
            assert "compress=zstd" in opts, f"{mp} has no compress=zstd: {opts}"
            assert "noatime" in opts, f"{mp} has no noatime: {opts}"

    with subtest("/persist exists from day one (impermanence by design)"):
        # Section 7.2 says to create it right away and route state there, even with
        # the wipe switched off until milestone 7.
        machine.succeed("test -d /persist")
        assert "@persist" in machine.succeed("findmnt -n -o SOURCE /persist")

    with subtest("the swapfile is NOCOW and usable (hibernation)"):
        # This is NOT tested through mount options. On btrfs, `nodatacow` and
        # `compress` belong to the FILE SYSTEM, not to the subvolume: the first
        # mount wins and the rest are ignored silently. That is how this test found
        # out that @swap's `mountOptions` did nothing — /swap showed up with
        # `compress=zstd:3` and without `nodatacow`.
        #
        # What actually guarantees hibernation is the file's ATTRIBUTE: a swapfile
        # on btrfs has to be NOCOW (C) and not compressed. The
        # `btrfs filesystem mkswapfile` takes care of that, and it is THAT which is
        # asserted.
        machine.succeed("test -f /swap/swapfile")

        attrs = machine.succeed("lsattr /swap/swapfile").split()[0]
        print(f"swapfile attributes: {attrs}")
        assert "C" in attrs, (
            f"the swapfile has no NOCOW (attributes: {attrs}). "
            "Without it section 7.2's hibernation does not work."
        )

        # And the requested size — proof that the `mkswapfile` really allocated it.
        #
        # We do NOT assert the SWAPSPACE2 signature nor activate it with `swapon`:
        # formatting and activating belong to the system's `swapDevices` activation,
        # and this test's config declares no swap. This test covers the LAYOUT.
        size_kib = int(machine.succeed("stat -c %s /swap/swapfile").strip())
        print(f"swapfile size: {size_kib} bytes")
        assert size_kib >= 200_000_000, (
            f"a swapfile of {size_kib} bytes — mkswapfile did not allocate. "
            "The first time this happened because the test asked for 16 GiB "
            "(production's default) on a 4 GiB disk."
        )

    with subtest("the volume really is inside LUKS2"):
        out = machine.succeed("cryptsetup luksDump /dev/vda2 | head -5")
        print(out)
        assert "LUKS header information" in out
        assert "Version:       \t2" in out or "Version: 2" in out.replace("  ", " ")
  ''
  + lib.optionalString bios ''

    with subtest("BIOS: GRUB from the disk's first sectors, its partition the third"):
        # Started from BIOS, by GRUB: no EFI at all.
        machine.fail("test -d /sys/firmware/efi")
        # GRUB's own partition, numbered after the ESP and the LUKS volume —
        # the installer names the volume `2` (`plan::luks_partition`) — and
        # placed in the gap before the ESP.
        kind = machine.succeed("lsblk -n -o PARTTYPE /dev/vda3").strip()
        assert kind == "21686148-6449-6e6f-744e-656564454649", f"vda3 is {kind}"
        start = int(machine.succeed("cat /sys/class/block/vda3/start").strip())
        esp = int(machine.succeed("cat /sys/class/block/vda1/start").strip())
        assert start < esp, f"GRUB's partition starts at {start}, after the ESP at {esp}"
        # The boot code GRUB wrote into the disk's first sector.
        machine.succeed("head -c 512 /dev/vda | grep -a -q GRUB")
        # And the protective MBR's entry marked active (its status byte, at
        # 446), which some BIOS firmware wants before it boots a disk.
        active = machine.succeed("od -An -tx1 -j446 -N1 /dev/vda").strip()
        assert active == "80", f"the protective MBR's entry is {active}, not active"
        # The menu is the machines' own, and a boot that tries nothing is
        # not marked as one.
        menu = machine.succeed("cat /boot/grub/grub.cfg")
        assert "ronoctua_default" in menu, "the menu has no boot trial"
        cmdline = machine.succeed("cat /proc/cmdline").split()
        assert "ronoctua.trying" not in cmdline, cmdline
  '';
}
