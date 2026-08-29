# Installer ISO — section 7.1.
#
# "The ISO is built by the flake itself (nixosConfigurations.installer ->
# system.build.isoImage)."
#
# The installer runs BEFORE any GPU is configured and does NOT share the
# shell's renderer: it is ratatui on a TTY, a separate stack (section 7.1).
# That is why this ISO carries noctua-iosevka-core (~62 MB, 4 faces) and not
# the whole family (2.1 GB) — an ISO does not close with that. See
# docs/ARCHITECTURE.md.
{
  config,
  lib,
  pkgs,
  modulesPath,
  inputs,
  ronoctua,
  ...
}:

{
  imports = [
    "${modulesPath}/installer/cd-dvd/installation-cd-minimal.nix"
    inputs.disko.nixosModules.disko
    # THE SAME NETWORK LAYER AS THE INSTALLED SYSTEM.
    #
    # The ISO needs it for two reasons, and the second is the binding one: the
    # installer's seventh question SCANS networks over NetworkManager's
    # D-Bus, and the step that builds the system downloads everything over the
    # network. An ISO with a different manager from the final system would mean
    # the footer connects one way during installation and another way afterwards.
    ../modules/network.nix
  ];

  # THE FLAKE TRAVELS ON THE ISO.
  #
  # The installer's plan runs `nixos-install --flake /mnt/etc/nixos#noctua`,
  # and the `inst-step-flake` step copies this there. Without this line
  # `nixos-install` does not find `flake.nix` and stops — and DRY RUN mode never
  # showed that, because it prints the command instead of running it.
  #
  # The tree is small: the fonts are a Release asset and never enter git
  # (see CLAUDE.md), so what goes in here are the `.nix` files, the Rust and the
  # text assets.
  environment.etc."nixos".source = ronoctua.flake;

  # THE SEED — what the target machine would otherwise COMPILE.
  #
  # The list itself is in `flake.nix`, next to where those derivations are
  # defined; what belongs here is why it travels at all.
  #
  # `nixos-install` does not download a system, it builds one. Everything in a
  # NixOS closure that came from nixpkgs arrives prebuilt from the public cache;
  # everything this repository makes has to be compiled on the machine being
  # installed, and that is where the afternoon went. With the seed aboard, the
  # target compiles NOTHING and downloads only what a cache already serves.
  #
  # `iso-image.nix` registers everything in `storeContents` with
  # `nix-store --load-db` at boot, so these arrive as VALID store paths and not
  # as files that happen to be on the medium.
  isoImage.storeContents = ronoctua.seed;

  # -------------------------------------------------------------------
  # ISO identity
  # -------------------------------------------------------------------
  # Naming the ISO has TWO traps in a row, both silent:
  #
  #   1. `isoImage.isoName` was renamed to `image.fileName` in 26.05. The
  #      old name does not fail — it is merely ignored, with a warning.
  #   2. `image.fileName` is not enough either. It ends up correct in the
  #      config (you can check with `nix eval`), but what names the file IS
  #      the derivation, and that is `image.baseName`. With only fileName
  #      forced, the ISO still comes out as "nixos-minimal-..." and nothing
  #      warns.
  #
  # So we set baseName; fileName derives from it.
  image.baseName = lib.mkForce "ronoctua-distro-${config.system.nixos.label}-x86_64";

  isoImage = {
    volumeID = lib.mkForce "RONOCTUA";
    makeEfiBootable = true;
    makeUsbBootable = true;
    # No splash: section 3 holds from the very first pixel.
    squashfsCompression = "zstd -Xcompression-level 12";
  };

  # We do not use ZFS (section 7.2: btrfs, and ZFS pins the kernel). The module
  # ships with the installation profile and warns if the default is not
  # explicit; `false` is the default from 26.11 on and is the safer one.
  boot.zfs.forceImportRoot = false;

  # -------------------------------------------------------------------
  # Font — the core subset, on the TTY and in fontconfig.
  # -------------------------------------------------------------------
  fonts.enableDefaultPackages = false;
  fonts.fontDir.enable = true;
  fonts.packages = [ ronoctua.fonts.noctua-iosevka-core ];
  fonts.fontconfig.defaultFonts = {
    monospace = [ "NoctuaIosevka Nerd Font Mono" ];
    sansSerif = [ "NoctuaIosevka Nerd Font Mono" ];
    serif = [ "NoctuaIosevka Nerd Font Mono" ];
  };

  # The TTY is where the installer lives. PSF2 generated from the real file —
  # the Linux console does not consume TTF. See nix/scripts/make-console-psf.py.
  console = {
    earlySetup = true;
    font = "${ronoctua.fonts.noctua-console-psf}/share/consolefonts/noctua-iosevka-32.psf";
    keyMap = lib.mkDefault (import ../keyboard-xkb.nix).console;
  };

  i18n = {
    defaultLocale = "pt_BR.UTF-8";
    supportedLocales = [
      "pt_BR.UTF-8/UTF-8"
      "en_US.UTF-8/UTF-8"
      "C.UTF-8/UTF-8"
    ];
  };

  # -------------------------------------------------------------------
  # Tools the installer needs
  # -------------------------------------------------------------------
  environment.systemPackages = with pkgs; [
    # The installer. Without it the ISO is a minimal NixOS with loose tools,
    # and section 7.1 is not satisfied by any path.
    ronoctua.installerCrate

    # Declarative partitioning and formatting (section 7.1).
    inputs.disko.packages.${pkgs.stdenv.hostPlatform.system}.disko
    # Hardware detection. "Every question asked of the user that could have
    # been detected is a defect" (section 7.1).
    nixos-facter

    btrfs-progs
    cryptsetup
    tpm2-tools
    sbctl # Secure Boot inspection
    gptfdisk
    parted
    dosfstools
    util-linux
    pciutils
    usbutils
    dmidecode
    git
    jq

    # The executor's `password_hash` calls it by name. Without this package the
    # installer reaches the second-to-last step and stops — after the disk has
    # already been erased, which is the worst place to be missing a dependency.
    mkpasswd
  ];

  # TPM2 available on the ISO already: the installer needs to detect presence and
  # version to decide on the enrolment (section 7.1).
  security.tpm2.enable = true;
  security.tpm2.pkcs11.enable = true;

  # -------------------------------------------------------------------
  # ISO session
  # -------------------------------------------------------------------
  # The minimal ISO already gives a passwordless root on the console. We add
  # neither user nor password here: nothing personal, not even on the ISO (section 9).
  services.getty.autologinUser = lib.mkForce "root";

  # Bilingual and written by hand, and not through the `.ftl` like everything
  # else (invariant 8).
  #
  # It is not an exception: it is that the language is the FIFTH question of the
  # installer, so on this screen it does not exist yet. Showing both lines is the
  # only way for the person to find the command before being able to choose how
  # to talk to it. From there on everything goes through the `.ftl`.
  users.motd = ''

    ronoctua-distro

    To install:         ronoctua-install --execute
    To preview only:    ronoctua-install

    To install:         ronoctua-install --execute
    To preview only:    ronoctua-install

  '';

  # THE AGENT DOES NOT RUN ON THE ISO.
  #
  # It exists to hand the password to NetworkManager when the INSTALLED system
  # boots. On the ISO there is no `/persist` and no blob at all, and the
  # installer seals by calling `systemd-creds` directly — not over the bus.
  # Leaving it enabled would give a service restarting forever with nothing to do.
  ronoctua.network.nmAgent = false;

  networking.hostName = "noctua-installer";
  # NetworkManager comes from `../modules/network.nix`, imported above — together
  # with powersave switched off and the TCP tuning. Repeating the `enable` here
  # would give the impression that the ISO has a network of its own, and it does not.
  services.openssh.enable = true;
  services.openssh.settings.PermitRootLogin = lib.mkForce "prohibit-password";

  nix.settings.experimental-features = [
    "nix-command"
    "flakes"
  ];

  system.stateVersion = "26.05";
}
