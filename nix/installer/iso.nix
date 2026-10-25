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
  options,
  lib,
  pkgs,
  utils,
  modulesPath,
  inputs,
  ronoctua,
  ...
}:

let
  # WHERE THE DISTRO'S TREE IS on the ISO — named once, here; the installer
  # reads it from `RONOCTUA_FLAKE_DIR`.
  distroDir = "/etc/ronoctua/distro";

  # THE IMAGE AS NIXPKGS BUILDS IT, from `iso-image.nix`'s own definition:
  # the one below is it with one file more, and naming it through
  # `config.system.build.isoImage` would be the definition reading itself.
  nixpkgsIsoImage =
    (lib.findFirst (d: lib.hasSuffix "/installer/cd-dvd/iso-image.nix" d.file && d.value ? isoImage)
      (throw "iso-image.nix no longer defines system.build.isoImage; the medium check needs it")
      options.system.build.definitionsWithLocations
    ).value.isoImage;

  # The mount of the store on the medium, as systemd names it in the initrd.
  roStore = "${utils.escapeSystemdPath "/sysroot/nix/.ro-store"}.mount";
in
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

  # THE DISTRO'S TREE TRAVELS ON THE ISO — see the L20 region below, where it
  # is put (it is not the machine directory any more: a machine has its own
  # flake, made from the tree's template).

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
  #
  # ALWAYS, whole (see the note after the SSH settings below).
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

  # -------------------------------------------------------------------
  # The medium checks itself before anything uses it
  # -------------------------------------------------------------------
  # A STICK CAN HOLD THE ISO'S NAME AND LENGTH AND NOT ITS BYTES. A copy onto
  # a Ventoy stick, pulled before the kernel wrote it back, kept everything
  # past 2.13 GiB from the ISO that was there before: the kernel and the
  # initrd sit near the start, so it booted, and the store did not mount —
  # `Failed to mount /sysroot/nix/.ro-store` and an emergency shell that says
  # nothing of why. A copy broken in the middle of the store would mount, and
  # fail later, inside an installation that has already erased a disk.
  #
  # So the image carries the store's sha256 beside it
  # (`/nix-store.squashfs.sha256`, `sha256sum`'s line, which a person can
  # check by hand), and the initrd hashes the store against it before the
  # store is mounted, and stops with a sentence when they differ. The hash
  # is computed where the store is made, inside the image's build: the
  # initrd cannot carry it, since the initrd is inside the store.
  #
  # It reads the whole store once per boot: ~40 s from a USB 3 stick (one
  # read 5 GB in 40 s), ~15 s in QEMU from the host's cache, and only the
  # hashing after `copytoram` (which runs first, so what is checked is what
  # will be mounted).
  system.build.isoImage = lib.mkForce (
    nixpkgsIsoImage.overrideAttrs (old: {
      squashfsCommand = old.squashfsCommand + ''

        sha256sum nix-store.squashfs > nix-store.squashfs.sha256
        addPath nix-store.squashfs.sha256 nix-store.squashfs.sha256
      '';
    })
  );

  # The unit lives in the systemd initrd, and a scripted one would drop it
  # without a word.
  assertions = [
    {
      assertion = config.boot.initrd.systemd.enable;
      message = "the installer ISO's medium check is an initrd systemd unit: boot.initrd.systemd.enable must stay on";
    }
  ];

  boot.initrd.systemd.services.medium-check = {
    description = "Check that the stick holds this ISO whole";
    requiredBy = [ roStore ];
    before = [ roStore ];
    after = [ "copytoram.service" ];
    unitConfig.RequiresMountsFor = "/sysroot/iso";
    # THE PROGRESS GOES THROUGH JOURNALD (standard output, kept as fd 3:
    # inside the pipe below, standard output is the hash), which also keeps
    # systemd's start-job spinner off the console. THE REFUSAL IS WRITTEN TO
    # THE CONSOLE DIRECTLY, and to the journal through standard error:
    # forwarded, it reached the console after systemd's `[DEPEND]` lines and
    # was scattered among them.
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      StandardOutput = "journal+console";
      StandardError = "journal";
    };
    path = [ pkgs.coreutils ];
    script = ''
      set -o pipefail
      store=/sysroot/iso/nix-store.squashfs
      said=/sysroot/iso/nix-store.squashfs.sha256
      exec 3>&1
      say() {
        printf '%s\n' "$*" >&3
      }
      refuse() {
        printf '%s\n' "$*" >/dev/console
        printf '%s\n' "$*" >&2
      }
      broken() {
        refuse "The stick does not hold this ISO whole: $1."
        refuse "A copy that never finished reaching the stick does this (one removed before it was"
        refuse "written back). Write the ISO again; this writes it and reads it back from the stick:"
        refuse "  nix run github:ronoctua/ronoctua-distro/release#fetch-iso -- --into <the stick's folder>"
        refuse "and this checks a copy made any other way:"
        refuse "  nix run github:ronoctua/ronoctua-distro/release#fetch-iso -- --check <the ISO on the stick>"
        exit 1
      }
      [ -r "$said" ] || broken "nix-store.squashfs.sha256 is missing"
      [ -r "$store" ] || broken "nix-store.squashfs is missing"
      want=
      read -r want _ <"$said" || true
      [ -n "$want" ] || broken "nix-store.squashfs.sha256 is empty"
      size=$(stat -L -c %s "$store")
      say "Checking that the stick holds this ISO whole ($(( size / 1048576 )) MiB)..."
      # A TENTH AT A TIME, so the console shows it moving: one `dd` per
      # tenth into a single `sha256sum`.
      blocks=$(( (size + 4194303) / 4194304 ))
      tenth=$(( (blocks + 9) / 10 ))
      if ! got=$(
        for i in 1 2 3 4 5 6 7 8 9 10; do
          dd if="$store" bs=4M iflag=fullblock skip=$(( (i - 1) * tenth )) count="$tenth" status=none || exit 1
          say "  $(( i * 10 ))%"
        done | sha256sum
      ); then
        broken "reading nix-store.squashfs failed (journalctl -b says where)"
      fi
      got=''${got%% *}
      [ "$got" = "$want" ] || broken "nix-store.squashfs is not the one this ISO was built with (sha256 $got, not $want)"
      say "The stick holds this ISO whole (sha256 $want)."
    '';
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

  # The TTY is where the installer lives. Its font — PSF2 generated from the
  # real file, the Linux console does not consume TTF — is
  # `nix/modules/console.nix`'s, as on every machine; the layout is the
  # distro's default until the installer asks.
  console.keyMap = lib.mkDefault (import ../keyboard-xkb.nix).console;

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

    # THE PERSON'S TWO PROGRAMS, which the installer runs by name: the
    # belongings' CLI (the plain part copied out, the deploy key, the probe,
    # the passphrase's check, the restore) and root's applier and verifier of
    # the person's declarations — extracted (`nix/rust.nix`), so the ISO does
    # not carry the workspace. And `ssh`, which git reaches an SSH repository
    # through.
    ronoctua.belongingsCli
    ronoctua.personCli
    openssh
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
  #
  # ONE BLOCK PER DECLARED LANGUAGE, in `languages.nix`'s order: the second
  # block is pt-BR, not a copy of the first (a sweep into English once made
  # it one, and the banner said the same thing twice). The value exists in
  # every language, which is where invariant 18 lets pt-BR be.
  users.motd = ''

    ronoctua-distro

    To install:         ronoctua-install --execute
    To preview only:    ronoctua-install

    Para instalar:      ronoctua-install --execute
    Só ver a prévia:    ronoctua-install

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

  # THE 2026-10-05 WAVE: one region per package, after its anchor.
  # ── anchor: L02 ──
  # SSH POST-QUANTUM FIRST, the installer's sshd and client both — the same
  # table as the installed system (`assets/ssh.nix`, `nix/modules/ssh.nix`),
  # which this ISO does not import. The sshd takes a root login, so after the
  # hybrids it keeps a server's narrower list, not the client's. And the sshd
  # that lets a person install from another machine keeps ONE host key,
  # ed25519: NixOS would also make an RSA one at boot (`no-rsa-host-key`).
  programs.ssh.kexAlgorithms =
    (import ../../assets/ssh.nix).postQuantum ++ (import ../../assets/ssh.nix).classical;
  services.openssh.settings.KexAlgorithms =
    (import ../../assets/ssh.nix).postQuantum ++ (import ../../assets/ssh.nix).serverClassical;
  services.openssh.hostKeys = (import ../../assets/ssh.nix).hostKeys;
  # ── anchor: L16 ──
  # THE ISO ALWAYS CARRIES ITS SEED (the `storeContents` line above): the
  # distro has no binary cache of its own, so what only this repository makes
  # — the shell, the compositor, the fonts, the icons, the index — and the
  # flake inputs' sources that evaluating a machine's flake needs travel on
  # the medium, and the target compiles none of it.
  #
  # THE PERSON'S CACHE IS NOT THE ISO'S: this ISO knows no person. When the
  # person's options name one, the installer hands it to `nixos-install` on
  # its command line — the cache and its key, and the netrc of a private one
  # (`ronoctua-installer`'s plan) — and the installed machine declares it
  # itself (`nix/modules/binary-cache.nix`). So nothing here declares a
  # substituter or a key (`binary-cache-declared-once`).
  # ── anchor: L20 ──
  # THE DISTRO'S TREE, and what the installer is told about it.
  #
  # The installer makes the machine's OWN flake from the tree's template
  # (`templates/machine`) and installs the distro as an input of it; it reads
  # where the tree is, and which revision it is, from these three variables
  # — the login shell's, where the installer is run. The revision is what it
  # compares with the `release` branch: a release at another revision starts
  # the release's installer instead (`crates/ronoctua-installer/src/release.rs`).
  # A tree with changes says `-dirty`, and a development installer never
  # follows the release.
  #
  # NOT the machine directory's name: the ISO's own `/etc/nixos` is nobody's
  # machine, and the template is what becomes one.
  environment.etc.${lib.removePrefix "/etc/" distroDir}.source = ronoctua.flake;
  environment.variables = {
    RONOCTUA_FLAKE_DIR = distroDir;
    RONOCTUA_FLAKE_REV = ronoctua.flake.rev or ronoctua.flake.dirtyRev or "unknown";
    RONOCTUA_FLAKE_LAST_MODIFIED = toString (ronoctua.flake.lastModified or 0);
  };
  # (The person's two programs and `ssh` are in `environment.systemPackages`
  # above: one list, defined once.)

  system.stateVersion = "26.05";
}
