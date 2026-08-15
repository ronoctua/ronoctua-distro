# `ronoctua-install --execute` runs against a real disk, and what it writes boots.
#
# # NOT WIRED INTO THE GATE YET — read `docs/PENDING.md` before touching it
#
# This file is a work in progress kept deliberately: the harness is right and the
# two boot failures found so far are recorded here rather than rediscovered. It
# is NOT in `tests/checks.nix`, because a red check that everyone learns to
# ignore is worse than an honest gap.
#
# # Why this test and not more of `installer-dryrun`
#
# The dry run PRINTS the commands. It proves the plan's shape — the order, the
# labels, that no secret reaches `host.nix` — and it proves nothing at all about
# the commands working, because it never runs one. The destructive path had never
# executed anywhere, and the first machine to try it would have found out with
# its disk already erased.
#
# # The harness is nixpkgs' own
#
# `nixos/tests/installer.nix` solves exactly this and it would be the fifth
# invariant's mistake to reimplement it: TWO nodes sharing one disk image, the
# second with `virtualisation.useBootLoader`, started after the first has
# shut down.
#
# The arrangement that makes it work is the one worth naming here: the installer
# boots its root off `/dev/vdb`, a scratch image, so that `/dev/vda` — the shared
# `target.qcow2` — is a disk it can erase without erasing itself.
#
# # AND IT RUNS OFFLINE, which is the other half of what this proves
#
# `substituters` is forced empty. So the seed the ISO carries has to be enough:
# if the target reached for the network to fetch a derivation or to evaluate the
# flake, this test is where it says so, instead of on somebody's laptop.
#
# # The first attempt hung for 900 s printing nothing, and this is why
#
# `bin/install.rs` picks its mode with `stdin().is_terminal() && stdout()...`,
# and the test driver's shell IS a terminal. The binary went into the TUI, drew
# on the alternate screen nobody reads and waited for a key until the timeout —
# which explains the hang and the silence at once, since the plan is only printed
# in the terminal-less path. `< /dev/null` is the whole fix.
#
# Everything the installer prints goes to a FILE inside the VM and is read back
# from there, rather than being trusted to the driver's own capture. Each
# iteration of this test costs about a quarter of an hour; the cheapest thing to
# do with that is make sure a failure arrives with its output attached.
{
  pkgs,
  lib,
  self,
  rust,
  system,
}:

let
  # The system the installer will build on the target. It is here so its whole
  # closure enters the VM through `system.extraDependencies` — with no
  # substituters, what is not in the store before boot cannot appear later.
  target = self.nixosConfigurations.minimal.config.system.build.toplevel;

  # THE PROFILE IS THE REDUCED ONE, and that is what makes this test affordable:
  # 6,9 GiB of closure to carry into a VM instead of 22,8. It is also the profile
  # the ISO seeds, so this exercises the path an install actually takes.
  answers = {
    RONOCTUA_INST_LANGUAGE = "en-GB";
    RONOCTUA_INST_DISK = "/dev/vda";
    RONOCTUA_INST_PROFILE = "minimal";
    RONOCTUA_INST_KEYBOARD = "gb";
    RONOCTUA_INST_HOSTNAME = "noctua-teste";
    RONOCTUA_INST_PASSWORD = "senha-de-teste";
    RONOCTUA_INST_PASSPHRASE = "frase-secreta-de-teste";
  };
  exports = lib.concatStringsSep " " (
    lib.mapAttrsToList (name: value: "${name}=${lib.escapeShellArg value}") answers
  );

  common = {
    virtualisation.cores = 4;
    virtualisation.memorySize = 4096;
    # BOTH NODES USE THE SAME DRIVE. It is the whole mechanism: the second one
    # boots what the first one wrote.
    virtualisation.diskImage = "./target.qcow2";
    virtualisation.diskSize = 40 * 1024;
    virtualisation.useEFIBoot = true;
  };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-installer-executes";

  nodes = {
    installer =
      { ... }:
      {
        imports = [ common ];

        # The installer's OWN root, on a second disk. Without this it would be
        # partitioning the device it is running from.
        virtualisation.emptyDiskImages = [ 3072 ];
        virtualisation.rootDevice = "/dev/vdb";

        # THE SCRATCH DISK ARRIVES BLANK, and what formats it is
        # `virtualisation`'s `autoFormat` — which the SYSTEMD initrd does not
        # implement. With it on, the boot goes fsck, then mount, then
        # "Can't find ext4 filesystem", then a kernel panic 2,7 s in, and the
        # message names the filesystem rather than the missing `mkfs`.
        #
        # Only on this node: the installed system is the one that wants the
        # systemd initrd, and it is a different machine.
        boot.initrd.systemd.enable = lib.mkForce false;

        # NO SUBSTITUTERS. What the machine does not already have, it cannot get
        # — which is the point: an install that needs the network here is an
        # install that needs it on real hardware too.
        nix.settings.substituters = lib.mkForce [ ];
        nix.settings.connect-timeout = 1;

        # The target's closure, and the flake's inputs as sources. The first is
        # what `nixos-install` copies; the second is what evaluating the flake
        # needs, and `flake.lock` names them without containing them.
        system.extraDependencies = [
          target
        ]
        ++ builtins.filter (p: p != null) (map (i: i.outPath or null) (builtins.attrValues self.inputs));

        # WHAT THE ISO GIVES THE INSTALLER, repeated here because an ISO module
        # cannot be a VM node: it brings `installation-cd-minimal` with it.
        environment.etc."nixos".source = self;
        environment.systemPackages = [
          rust.installer
          self.inputs.disko.packages.${system}.disko
          pkgs.nixos-facter
          pkgs.btrfs-progs
          pkgs.cryptsetup
          pkgs.mkpasswd
          pkgs.util-linux
        ];
      };

    machine =
      { ... }:
      {
        imports = [ common ];

        # It boots what is on the disk, and nothing of this configuration's own.
        virtualisation.useBootLoader = true;
        virtualisation.useDefaultFilesystems = false;
        virtualisation.efi.keepVariables = false;
        # A root that does not exist, so that anything reading it fails loudly
        # rather than quietly booting this node instead of the installed system.
        virtualisation.fileSystems."/" = {
          device = "/dev/disk/by-label/nao-existe-e-nunca-sera-usado";
          fsType = "ext4";
        };
      };
  };

  testScript = ''
    installer.start()
    installer.wait_for_unit("multi-user.target")

    with subtest("o instalador roda ate o fim, e a saida vem junto se nao rodar"):
        # `< /dev/null` is not tidiness: with a terminal on stdin the binary
        # enters the TUI and waits for a key forever. And the output goes to a
        # file so that a failure arrives with its reason attached.
        status = installer.succeed(
            "${exports} ronoctua-install --execute < /dev/null > /tmp/install.log 2>&1; echo $?"
        ).strip()
        # NOT named `log`: that name belongs to the driver's own logger, and
        # rebinding it to a string fails the driver's type check before the VM
        # even starts.
        printed = installer.succeed("cat /tmp/install.log")
        print(printed)
        assert status == "0", f"o instalador saiu com {status}"

    with subtest("ele escreveu o que so ele escreve"):
        # The generated `host.nix`, with the answers in it. `installer-dryrun`
        # reads the same file; what is new here is that it exists ON DISK,
        # written by a step that ran.
        host = installer.succeed("cat /mnt/etc/nixos/host.nix")
        print(host)
        assert 'networking.hostName = "noctua-teste"' in host
        assert 'ronoctua.profile = "minimal"' in host
        assert 'ronoctua.disk = "/dev/vda"' in host

        # NO SECRET IN IT (section 9): this file goes to backup.
        assert "senha-de-teste" not in host
        assert "frase-secreta-de-teste" not in host

        # The password hash IS written, and into `/persist`, because
        # `mutableUsers = false` means the activator reads it during the install
        # and never again.
        installer.succeed("test -s /mnt/persist/etc/shadow/noctua")

        # And the LUKS key file is gone. It lived in `/tmp` for `disko` to read;
        # left behind it would be the disk's passphrase in plain text on a
        # machine about to reboot into the installed system.
        installer.fail("test -e /tmp/disko-luks.key")

    with subtest("o resume_offset saiu do swapfile, e nao de um palpite"):
        # Computed AFTER the swap file exists, and the one answer the installer
        # produces instead of asking. Zero would be accepted by the kernel and
        # would fail in use rather than at boot.
        offset = installer.succeed(
            "grep -o 'resume_offset=[0-9]*' /mnt/etc/nixos/host.nix || true"
        ).strip()
        assert offset and not offset.endswith("=0"), f"resume_offset invalido: {offset!r}"

    installer.shutdown()

    with subtest("e o que ele instalou BOOTA"):
        # The half that makes this more than a longer `installer-dryrun`.
        machine.start()
        machine.wait_for_console_text("[Pp]assphrase for|[Ee]nter passphrase")
        machine.send_console("frase-secreta-de-teste\n")
        machine.wait_for_unit("multi-user.target")

        # It is the machine the answers described, and not this node.
        assert "noctua-teste" in machine.succeed("hostname")
        machine.succeed("test -d /persist")
        # The reduced profile: the applications are not there.
        machine.fail("command -v google-chrome-stable")

    machine.shutdown()
  '';
}
