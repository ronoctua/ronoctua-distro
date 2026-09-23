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

  # EVERY CURSOR IS DRAWN INTO THE PICTURE, in the VM and nowhere else.
  #
  # The compositor puts a seat's cursor on the GPU's cursor PLANE when it can,
  # and QEMU's window does not draw that plane where the guest put it: with an
  # absolute device present — the tablet that carries the host's pointer in —
  # `gd_mouse_set` (`ui/gtk.c`) returns early and the window shows the cursor
  # image at the HOST pointer's position. So a second mouse passed through to
  # the guest moved seat0's cursor where nobody could see it, and its clicks
  # landed there, read as "they act on the touchpad's cursor". Found testing
  # two seats: the second seat's cursor was visible only because the plane
  # holds one, and the other was drawn into the picture.
  #
  # Drawn by the compositor, every cursor is where the guest says it is. The
  # installed system keeps the plane: on real hardware it is the right answer.
  systemd.user.services.scroll.environment.WLR_NO_HARDWARE_CURSORS = "1";

  # QEMU'S OWN POINTERS STEP ASIDE WHEN THE HOST'S ARE HERE.
  #
  # `nix run .#vm -- --pointers` hands the host's mice and touchpads to the
  # guest one by one, and QEMU grabs them — so the tablet that carries the
  # host's pointer in, and the PS/2 mouse beside it, receive nothing for as
  # long as the VM runs. Left to libinput they are pointers like any other,
  # each a cursor that no hand can move: measured, a sixth cursor for five
  # devices. The runner says it passed them through with a kernel flag, and
  # this is the only reader of it. Without the flag nothing changes, because
  # then the tablet IS the host's mouse.
  services.udev.extraRules = ''
    SUBSYSTEM=="input", IMPORT{cmdline}="ronoctua_pointers"
    SUBSYSTEM=="input", ENV{ronoctua_pointers}=="1", ATTRS{name}=="QEMU QEMU USB Tablet|VirtualPS/2 VMware VMMouse|ImExPS/2 Generic Explorer Mouse", ENV{LIBINPUT_IGNORE_DEVICE}="1"
  '';

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
