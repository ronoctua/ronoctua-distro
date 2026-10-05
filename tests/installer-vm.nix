# `ronoctua-install --execute` runs against a real disk, and what it writes boots.
#
# # WIRED INTO THE GATE, among the slow ones (`tests/slow.nix`: it partitions a
# # disk and reboots)
#
# It reached step 5 of 10, and getting there cost THREE defects of the
# installer that no dry run could have shown — the working directory that was
# the store, the hardware report `disko` needed before it existed, and the root
# disk that was never formatted. All three are fixed.
#
# Where it stopped was COST, not a defect: `disko --flake` evaluates the whole
# NixOS configuration inside the virtual machine, and 900 s of the driver's
# default timeout was not enough. So the installer's call has an hour
# (`execute`'s own `timeout`), its node twice the memory, and the disk scripts
# of the system it installs are in `system.extraDependencies` — what they run
# is in the store before boot, and the evaluation builds nothing new.
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
# has NO root disk at all — root on tmpfs — so that `/dev/vda`, the shared
# `target.qcow2`, is a disk it can erase without erasing itself.
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

  # THE DISK SCRIPTS of the system the answers describe: `minimal` on this
  # node's disk, the one line of `host.nix` they depend on.
  disks =
    (self.nixosConfigurations.minimal.extendModules {
      modules = [ { ronoctua.disk = answers.RONOCTUA_INST_DISK; } ];
    }).config.system.build;

  # THE PROFILE IS THE REDUCED ONE, and that is what makes this test affordable:
  # 6,9 GiB of closure to carry into a VM instead of 22,8. It is also the profile
  # the ISO seeds, so this exercises the path an install actually takes.
  answers = {
    RONOCTUA_INST_LANGUAGE = "en-GB";
    RONOCTUA_INST_DISK = "/dev/vda";
    RONOCTUA_INST_PROFILE = "minimal";
    RONOCTUA_INST_KEYBOARD = "gb";
    RONOCTUA_INST_HOSTNAME = "noctua-teste";
    RONOCTUA_INST_PASSWORD = "test-password";
    RONOCTUA_INST_PASSPHRASE = "test-passphrase";
    # THE FIFTEENTH ANSWER, which lands in the person's home and never in
    # `host.nix`: an address alone, as the question accepts it.
    RONOCTUA_INST_BELONGINGS = belongings;
  };
  belongings = "git@example.org:someone/belongings.git";
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
  # THE INSTALL ALONE MAY TAKE THE HOUR its `execute` is given; the default
  # global hour would end the test before the install's own timeout could
  # print what it was doing.
  globalTimeout = 2 * 3600;

  nodes = {
    installer =
      { ... }:
      {
        imports = [ common ];

        # THE INSTALLER RUNS WITH NO ROOT DISK AT ALL.
        #
        # It needs a root that is not the disk it is about to erase, and the
        # obvious answer — a second, empty image — does not boot: what formats a
        # blank scratch disk is `virtualisation`'s `autoFormat`, which the
        # systemd initrd does not implement. That gave fsck, mount, "Can't find
        # ext4 filesystem" and a panic at 2,7 s, with the message naming the
        # filesystem instead of the `mkfs` that never ran. Turning the systemd
        # initrd off gave "Attempted to kill init!" at 1,2 s, earlier still.
        #
        # A tmpfs root has no such step. The store arrives over the framework's
        # own mount, `/dev/vda` is left entirely alone for `disko` to erase, and
        # the working directory the installer copies the flake into lives in RAM
        # — which the 4 GiB of this node covers, since the tree is small by
        # design (the fonts are a Release asset).
        virtualisation.useDefaultFilesystems = false;
        virtualisation.fileSystems."/" = {
          device = "tmpfs";
          fsType = "tmpfs";
          options = [ "mode=0755" ];
        };

        # TWICE THE COMMON MEMORY: `disko --flake` and `nixos-install` both
        # evaluate the whole system in here, and the working copy of the flake
        # lives in this node's RAM (a tmpfs root).
        virtualisation.memorySize = lib.mkForce 8192;

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
          # WHAT DISKO RUNS, and what it runs with: the script `--mode
          # destroy,format,mount` builds, and the legacy whole one — of the
          # system the answers describe, on this node's disk (`minimal` alone
          # names no disk: the installer's `host.nix` does). Built here, before
          # boot, so the evaluation inside finds them.
          disks.destroyFormatMount
          disks.diskoScript
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
          device = "/dev/disk/by-label/does-not-exist-and-never-will";
          fsType = "ext4";
        };
      };
  };

  testScript = ''
    installer.start()
    installer.wait_for_unit("multi-user.target")

    with subtest("the installer runs to the end, and the output comes along if it does not"):
        # `< /dev/null` is not tidiness: with a terminal on stdin the binary
        # enters the TUI and waits for a key forever. And the output goes to a
        # file so that a failure arrives with its reason attached.
        # `execute` and NOT `succeed`: a failure here has to arrive WITH its
        # output, and `succeed` raises before the next line can read the log.
        # That is the difference between an iteration that teaches something and
        # one that costs a quarter of an hour to say "it failed".
        # AN HOUR: `disko --flake` evaluates the whole system in here, and the
        # driver's own 900 s was where it stopped.
        # AND IT STREAMS TO THE CONSOLE as well as to the file: a step that
        # hangs is then seen hanging in the log, and not as an hour of silence
        # ended by the timeout (the driver runs this under `pipefail`, so the
        # status is the installer's).
        status, _ = installer.execute(
            "${exports} ronoctua-install --execute < /dev/null 2>&1 | tee /tmp/install.log > /dev/console",
            timeout=3600,
        )
        # NOT named `log`: that name belongs to the driver's own logger, and
        # rebinding it to a string fails the driver's type check before the VM
        # even starts.
        printed = installer.execute("cat /tmp/install.log")[1]
        print(printed)
        assert status == 0, f"the installer exited with {status}:\n{printed}"

    with subtest("it wrote what only it writes"):
        # The generated `host.nix`, with the answers in it. `installer-dryrun`
        # reads the same file; what is new here is that it exists ON DISK,
        # written by a step that ran.
        host = installer.succeed("cat /mnt/etc/nixos/host.nix")
        print(host)
        assert 'networking.hostName = "noctua-teste"' in host
        assert 'ronoctua.profile = "minimal"' in host
        assert 'ronoctua.disk = "/dev/vda"' in host

        # NO SECRET IN IT (section 9): this file goes to backup.
        assert "test-password" not in host
        assert "test-passphrase" not in host

        # The password hash IS written, and into `/persist`, because
        # `mutableUsers = false` means the activator reads it during the install
        # and never again.
        installer.succeed("test -s /mnt/persist/etc/shadow/noctua")

        # And the LUKS key file is gone. It lived in `/tmp` for `disko` to read;
        # left behind it would be the disk's passphrase in plain text on a
        # machine about to reboot into the installed system.
        installer.fail("test -e /tmp/disko-luks.key")

    with subtest("the belongings' repository is in the person's home, theirs, and nowhere else"):
        # THE ADDRESS ALONE, in `~/.config/ronoctua/belongings.json`, given to
        # the user `nixos-install` made — never root's, or the view could not
        # change it; and never in `host.nix`, which describes the machine.
        kept = installer.succeed("cat /mnt/home/noctua/.config/ronoctua/belongings.json")
        assert "${belongings}" in kept, kept
        assert "${belongings}" not in host
        uid = installer.succeed("grep '^noctua:' /mnt/etc/passwd | cut -d: -f3").strip()
        for made in (".config", ".config/ronoctua", ".config/ronoctua/belongings.json"):
            owner = installer.succeed(f"stat -c %u /mnt/home/noctua/{made}").strip()
            assert owner == uid, f"{made} is owned by {owner}, not the user ({uid})"
        mode = installer.succeed("stat -c %a /mnt/home/noctua/.config/ronoctua/belongings.json").strip()
        assert mode == "644", f"belongings.json is {mode}"

    with subtest("the resume_offset came out of the swapfile, and not out of a guess"):
        # Computed AFTER the swap file exists, and the one answer the installer
        # produces instead of asking. Zero would be accepted by the kernel and
        # would fail in use rather than at boot.
        offset = installer.succeed(
            "grep -o 'resume_offset=[0-9]*' /mnt/etc/nixos/host.nix || true"
        ).strip()
        assert offset and not offset.endswith("=0"), f"invalid resume_offset: {offset!r}"

    installer.shutdown()

    with subtest("and what it installed BOOTS"):
        # The half that makes this more than a longer `installer-dryrun`.
        machine.start()
        machine.wait_for_console_text("[Pp]assphrase for|[Ee]nter passphrase")
        machine.send_console("test-passphrase\n")
        machine.wait_for_unit("multi-user.target")

        # It is the machine the answers described, and not this node.
        assert "noctua-teste" in machine.succeed("hostname")
        machine.succeed("test -d /persist")
        # The reduced profile: the applications are not there.
        machine.fail("command -v google-chrome-stable")
        # AND THE BELONGINGS' SETTINGS ARE THE PERSON'S, on the running system.
        assert machine.succeed(
            "stat -c %U /home/noctua/.config/ronoctua/belongings.json"
        ).strip() == "noctua"

    machine.shutdown()
  '';
}
