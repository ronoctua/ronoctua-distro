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
{
  pkgs,
  lib,
  disko,
}:

disko.lib.testLib.makeDiskoTest {
  inherit pkgs;
  name = "ronoctua-layout-7-2";

  # The PRODUCTION layout, with a single difference: the swap's size, which in
  # production is >= RAM and here has to fit on the VM's disk. See the header of
  # disko-config-test.nix — the `makeDiskoTest` imports the file with the
  # default arguments and accepts no parameters.
  disko-config = ./disko-config-test.nix;

  # UEFI, as section 2 requires of the target machines.
  efi = true;

  # It boots after installing: proof that the layout not only mounts, but
  # sustains a system.
  testBoot = true;

  # The passphrase comes from a FILE, not from a prompt — it is the same path
  # the installer uses (section 7.1: the TUI collects, the orchestrator calls
  # disko). Here the "installer" is the test VM, so it writes the file.
  extraInstallerConfig = {
    systemd.tmpfiles.rules = [
      "f /tmp/disko-luks.key 0600 root root - senha-de-teste"
    ];
  };

  # Typing the passphrase at boot is NOT a workaround — it is the test of the
  # recovery path of section 7.2.
  #
  # The VM has no TPM2, so the crypttab's `tpm2-device=auto` finds no token
  # ("No valid TPM2 token data found") and systemd falls back to the prompt.
  # That fallback is exactly what 7.2 requires: "with a recovery passphrase
  # ALWAYS enrolled in parallel". If one day the volume opened with no TPM and
  # no passphrase, it would be a serious bug — and this test would start
  # failing here.
  #
  # Unlocking BY TPM2, with PCR 7/11/12/15, is milestone 8, with swtpm.

  # The INSTALLED system needs the tools the asserts use. `lsattr` comes from
  # e2fsprogs and is not in a minimal NixOS.
  extraSystemConfig = {
    environment.systemPackages = [
      pkgs.e2fsprogs
      pkgs.btrfs-progs
      pkgs.util-linux
    ];
  };

  # The variable is `machine`, not `booted_machine`: disko REBINDS `machine`
  # to the VM that booted and uses "booted_machine" only as a display name in
  # the log. Using the display name fails at type check, not at runtime.
  bootCommands = ''
    machine.wait_for_console_text("Please enter passphrase")
    machine.send_console("senha-de-teste\n")
  '';

  extraTestScript = ''
    with subtest("ESP de 1 GiB, nao 512 MiB (secao 7.2)"):
        # NixOS keeps kernel+initrd of SEVERAL generations on the ESP. With 512 MiB
        # it fills up after a few updates, and the symptom is a nixos-rebuild that
        # fails without explaining.
        out = machine.succeed("lsblk -b -n -o NAME,SIZE /dev/vda1")
        print(out)
        size_kib = int(out.split()[-1])
        assert 1_000_000_000 <= size_kib <= 1_200_000_000, \
            f"ESP tem {size_kib} bytes, esperava ~1 GiB"

    with subtest("os 7 subvolumes btrfs da secao 7.2"):
        subs = machine.succeed("btrfs subvolume list /")
        print(subs)
        for sv in ["@root", "@home", "@nix", "@persist", "@log", "@snapshots", "@swap"]:
            assert sv in subs, f"subvolume {sv} ausente"

    with subtest("zstd e noatime onde a 7.2 manda"):
        for mp in ["/", "/nix"]:
            opts = machine.succeed(f"findmnt -n -o OPTIONS {mp}")
            assert "compress=zstd" in opts, f"{mp} sem compress=zstd: {opts}"
            assert "noatime" in opts, f"{mp} sem noatime: {opts}"

    with subtest("/persist existe desde o dia um (impermanencia projetada)"):
        # Section 7.2 says to create it right away and route state there, even with
        # the wipe switched off until milestone 7.
        machine.succeed("test -d /persist")
        assert "@persist" in machine.succeed("findmnt -n -o SOURCE /persist")

    with subtest("swapfile e NOCOW e utilizavel (hibernacao)"):
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
        print(f"atributos do swapfile: {attrs}")
        assert "C" in attrs, (
            f"swapfile sem NOCOW (atributos: {attrs}). "
            "Sem isso a hibernacao da secao 7.2 nao funciona."
        )

        # And the requested size — proof that the `mkswapfile` really allocated it.
        #
        # We do NOT assert the SWAPSPACE2 signature nor activate it with `swapon`:
        # formatting and activating belong to the system's `swapDevices` activation,
        # and this test's config declares no swap. This test covers the LAYOUT.
        size_kib = int(machine.succeed("stat -c %s /swap/swapfile").strip())
        print(f"tamanho do swapfile: {size_kib} bytes")
        assert size_kib >= 200_000_000, (
            f"swapfile de {size_kib} bytes — o mkswapfile nao alocou. "
            "Na primeira vez isso aconteceu porque o teste pedia 16 GiB "
            "(o default de producao) num disco de 4 GiB."
        )

    with subtest("o volume esta mesmo dentro de LUKS2"):
        out = machine.succeed("cryptsetup luksDump /dev/vda2 | head -5")
        print(out)
        assert "LUKS header information" in out
        assert "Version:       \t2" in out or "Version: 2" in out.replace("  ", " ")
  '';
}
