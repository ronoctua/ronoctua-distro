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
#
# # THE PERSON'S REPOSITORY, over SSH (2026-10-05)
#
# The repository became mandatory, and the installer now applies the person's
# declarations to the machine's flake BEFORE the disk is touched and restores
# their belongings into the new home after `nixos-install`. So this test has
# a third node, `server`: an SSH git server holding a belongings repository
# that a seeder made on the installer node with the same deploy key —
# `ronoctua-belongings init` from a home whose plain part declares ONE
# program (`nix:hello`). The installer then runs in its terminal-less mode
# with that address, the deploy key's FILE and the passphrase, and what is
# asserted is the person's half: the declaration in the machine's `person/`,
# `person-applied.json` naming it, the machine's `flake.nix` and `flake.lock`
# agreeing (this node is a development build: it installs its own tree,
# aimed, and the machine KEEPS that pair — `nix flake metadata
# --no-update-lock-file` must take it as it is), the access and the consent
# in the home, theirs, and the restore's base.
#
# The network gate needs NetworkManager and a way out: the installer node
# runs NM over its test link, with the server as its gateway. WRITTEN, NOT
# RUN in the wave that wrote it (2026-10-05): see docs/PENDING.md.
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
    RONOCTUA_INST_BELONGINGS_ACCESS = "/root/keys/deploy";
    RONOCTUA_INST_BELONGINGS_PASSPHRASE = belongingsPassphrase;
    # WHERE THE DISTRO'S TREE IS, as the ISO says it (`nix/installer/iso.nix`).
    # No revision: this node is a development build, and installs its own.
    RONOCTUA_FLAKE_DIR = "/etc/ronoctua/distro";
  };
  # THE REPOSITORY, on the `server` node, reached by its node name — a host
  # `consent.json` can hold.
  belongings = "git@server:/srv/belongings.git";
  exports = lib.concatStringsSep " " (
    lib.mapAttrsToList (name: value: "${name}=${lib.escapeShellArg value}") answers
  );

  # A DEPLOY KEY, made once at build time: the test's, nobody's. The server
  # authorises its public half; the installer is handed its private half as
  # a FILE (`RONOCTUA_INST_BELONGINGS_ACCESS`).
  deployKey = pkgs.runCommand "installer-vm-deploy-key" { nativeBuildInputs = [ pkgs.openssh ]; } ''
    mkdir -p $out
    ssh-keygen -q -t ed25519 -N "" -C installer-vm -f $out/deploy
  '';

  # THE PERSON'S DECLARATION the repository carries in its plain part: one
  # program, from nixpkgs.
  declared = pkgs.writeText "declared.json" (
    builtins.toJSON {
      format = 1;
      apps = [
        {
          source = "nix";
          id = "nix:hello";
          machines = null;
          edited-at = "2026-10-05T12:00:00Z";
          edited-on = "noctua-seeder";
          attr = "hello";
          unfree = false;
        }
      ];
    }
  );
  belongingsPassphrase = "test-belongings-passphrase";

  common = {
    virtualisation.cores = 4;
    virtualisation.memorySize = 4096;
    # BOTH NODES USE THE SAME DRIVE. It is the whole mechanism: the second one
    # boots what the first one wrote.
    virtualisation.diskImage = "./target.qcow2";
    virtualisation.diskSize = 40 * 1024;
    virtualisation.useEFIBoot = true;
    # A TPM 2.0, which the installation requires (`crates/ronoctua-installer/
    # src/platform.rs`): without one it stops before the first question about
    # the disk. Each node has its own; nothing is sealed over a cable.
    virtualisation.tpm.enable = true;
  };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-installer-executes";
  # THE INSTALL ALONE MAY TAKE THE HOUR its `execute` is given; the default
  # global hour would end the test before the install's own timeout could
  # print what it was doing.
  globalTimeout = 2 * 3600;

  nodes = {
    # THE PERSON'S REPOSITORY: an SSH git server with one bare repository,
    # which the deploy key may read and write.
    server =
      { ... }:
      {
        services.openssh.enable = true;
        users.users.git = {
          isNormalUser = true;
          home = "/srv/git";
          openssh.authorizedKeys.keyFiles = [ "${deployKey}/deploy.pub" ];
        };
        environment.systemPackages = [ pkgs.git ];
        networking.firewall.allowedTCPPorts = [ 22 ];
      };

    installer =
      { nodes, ... }:
      {
        imports = [ common ];

        # THE NETWORK GATE: NetworkManager over the test's link, with the
        # server as the way out — the installer refuses to start without one
        # (`installer-requires-network`).
        networking.networkmanager.enable = true;
        networking.defaultGateway = nodes.server.networking.primaryIPAddress;

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
        # The distro's tree where the ISO keeps it, and the person's two
        # programs on the PATH.
        environment.etc."ronoctua/distro".source = self;
        environment.systemPackages = [
          rust.installer
          rust.belongings
          rust.person
          self.inputs.disko.packages.${system}.disko
          pkgs.nixos-facter
          pkgs.btrfs-progs
          pkgs.cryptsetup
          pkgs.mkpasswd
          pkgs.util-linux
          pkgs.git
          pkgs.openssh
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
    server.start()
    server.wait_for_unit("sshd.service")
    server.succeed("install -d -o git -g users -m 0755 /srv")
    server.succeed("sudo -u git git init -q --bare -b main /srv/belongings.git")

    installer.start()
    installer.wait_for_unit("multi-user.target")
    installer.wait_for_unit("NetworkManager.service")

    with subtest("a seeder fills the repository: one declared program, sealed belongings"):
        # THE SAME DEPLOY KEY the installer is handed, as the file it names.
        installer.succeed(
            "install -d -m 0700 /root/keys /root/seed/.config/ronoctua/belongings-keys"
            " /root/seed/.local/state/ronoctua/person/software /root/seed-run"
            " && install -m 0600 ${deployKey}/deploy /root/keys/deploy"
            " && install -m 0600 ${deployKey}/deploy /root/seed/.config/ronoctua/belongings-keys/deploy"
            " && install -m 0644 ${declared} /root/seed/.local/state/ronoctua/person/software/declared.json"
        )
        installer.succeed(
            "printf '{\"repository\": \"${belongings}\", \"auth\": \"ssh\"}\\n'"
            " > /root/seed/.config/ronoctua/belongings.json"
            " && printf '{\"hosts\": {\"server\": true}}\\n' > /root/seed/.config/ronoctua/consent.json"
            " && install -d -m 0700 /root/seed-run/ronoctua-belongings"
            " && printf %s ${lib.escapeShellArg belongingsPassphrase} > /root/seed-run/ronoctua-belongings/passphrase"
            " && chmod 0600 /root/seed-run/ronoctua-belongings/passphrase"
        )
        seeded = installer.execute(
            "HOME=/root/seed XDG_CONFIG_HOME=/root/seed/.config XDG_STATE_HOME=/root/seed/.local/state"
            " XDG_RUNTIME_DIR=/root/seed-run ronoctua-belongings init --progress 2>&1"
        )
        print(seeded[1])
        assert seeded[0] == 0, seeded[1]
        server.succeed("sudo -u git git -C /srv/belongings.git cat-file -e main:plain/person/software/declared.json")

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

    with subtest("the person's declarations are the machine's, and its flake agrees with its lock"):
        installer.succeed("grep -q 'nix:hello' /mnt/etc/nixos/person/software/declared.json")
        applied = installer.succeed("cat /mnt/etc/ronoctua/person-applied.json")
        print(applied)
        assert "nix:hello" in applied, applied
        # THE AIMED PAIR, KEPT: this node is a development build, so the
        # machine's flake names this node's tree — and its lock was made for
        # that flake, which every `--no-update-lock-file` evaluation needs.
        installer.fail("cmp ${self}/templates/machine/flake.nix /mnt/etc/nixos/flake.nix")
        installer.succeed("grep -q 'path:/nix/store/' /mnt/etc/nixos/flake.nix")
        installer.succeed("test -s /mnt/etc/nixos/flake.lock")
        installer.succeed(
            "nix --extra-experimental-features 'nix-command flakes' flake metadata"
            " --no-update-lock-file --offline path:/mnt/etc/nixos"
        )
        # AND THE TREE IT NAMES IS KEPT by the system built from it.
        installer.succeed("grep -q 'inputs.self.outPath' /mnt/etc/nixos/host.nix")
        installer.succeed("test -s /mnt/etc/nixos/facter.json")

    with subtest("the home has its access, its consent and its belongings, all the person's"):
        uid = installer.succeed("grep '^noctua:' /mnt/etc/passwd | cut -d: -f3").strip()
        home = "/mnt/home/noctua"
        for made, mode in (
            (".config/ronoctua/belongings-keys", "700"),
            (".config/ronoctua/belongings-keys/deploy", "600"),
            (".config/ronoctua/consent.json", None),
            (".local/state/ronoctua/person/software/declared.json", None),
            (".local/state/ronoctua/belongings/last.json", None),
        ):
            owner, got = installer.succeed(f"stat -c '%u %a' {home}/{made}").split()
            assert owner == uid, f"{made} is owned by {owner}, not the user ({uid})"
            assert mode is None or got == mode, f"{made} is {got}, not {mode}"
        installer.succeed(f"grep -q server {home}/.config/ronoctua/consent.json")
        # THE PASSPHRASE AND THE SCRATCH ARE GONE, the key in RAM with them.
        installer.fail("test -e /run/ronoctua-install")

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
        # The machine's flake and lock still agree on the running system:
        # an evaluation that may not move the lock takes them as they are.
        machine.succeed("grep -q 'inputs.ronoctua.url' /etc/nixos/flake.nix")
        machine.succeed(
            "nix --extra-experimental-features 'nix-command flakes' flake metadata"
            " --no-update-lock-file --offline path:/etc/nixos"
        )

    machine.shutdown()
  '';
}
