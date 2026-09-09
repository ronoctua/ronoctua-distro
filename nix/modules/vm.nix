# Development VM — section 7.12, speed 2.
#
# "nixos-rebuild build-vm — validates the system configuration."
# It is not where spacing gets adjusted: the nested compositor exists for that
# (speed 1). This VM proves the CONFIGURATION holds together and boots.
{
  config,
  lib,
  pkgs,
  ...
}:

{
  # No encrypted disk and no TPM here: the VM exists to validate the
  # configuration, not secure boot. The LUKS + TPM2 + PCR 15 path is
  # exercised by milestone 8's VM test, with swtpm.
  fileSystems."/" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "btrfs";
    options = [
      "compress=zstd"
      "noatime"
    ];
  };

  # /persist exists from day one (section 7.2), even in the VM — that is what
  # makes state routing get exercised before impermanence is switched on.
  fileSystems."/persist" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "btrfs";
    options = [
      "subvol=@persist"
      "compress=zstd"
      "noatime"
    ];
    neededForBoot = true;
  };

  # The VM boots the kernel directly through qemu, with no boot manager. Turn
  # BOTH off: with systemd-boot out, GRUB reverts to its `enable = true` default
  # and evaluation fails asking for `boot.loader.grub.devices`.
  boot.loader.systemd-boot.enable = lib.mkForce false;
  boot.loader.grub.enable = lib.mkForce false;
  boot.loader.efi.canTouchEfiVariables = lib.mkForce false;

  # Empty password in the VM. In production the installer writes the hash to
  # /persist and the base module points there (section 9: nothing personal in
  # the repository).
  #
  # A single attrset: with a dynamic key, `a.${k}.x` and `a.${k}.y` in two
  # statements is an evaluation error ("dynamic attribute already defined").
  users.users.${config.ronoctua.user} = {
    hashedPasswordFile = lib.mkForce null;
    initialPassword = "noctua";
  };

  virtualisation.vmVariant = {
    virtualisation = {
      memorySize = 4096;
      cores = 4;
      diskSize = 16384;
      graphics = true;
      qemu.options = [
        # wgpu speaks Vulkan. With no host GPU, it falls back to lavapipe — which is
        # exactly what section 9.3 of the plan prescribes for correctness
        # tests. Performance is NEVER measured here.
        "-vga none"
        "-device virtio-gpu-pci"
      ];
    };
  };

  # NO ANDROID IN THE DEV VM.
  #
  # Android Studio is a ~1,5 GB unfree download from Google's CDN, and it is the
  # only thing in this closure that can fail for a reason outside the flake — it
  # already did, with `Connection reset by peer`, and it took the whole
  # `nix flake check` with it, including checks that have nothing to do with it.
  #
  # The VM exists to test the DESKTOP. Mobile work happens on the installed
  # machine, where `ronoctua.dev.android` is on by default. Keeping a flaky
  # third-party download out of the gate is what makes the gate mean something.
  ronoctua.dev.android = false;
}
