# A BOOT TRIAL WITH SECURE BOOT (`boot-trial-is-blessed-under-secure-boot`).
#
# The distro boots systemd-boot with no signature (`docs/ARCHITECTURE.md`,
# "Secure Boot e TPM2"); a host that turns Secure Boot on imports lanzaboote,
# which REPLACES systemd-boot's builder: every generation becomes a UKI
# (`/boot/EFI/Linux/nixos-generation-<n>-<hash>.efi`), signed with the
# machine's keys, and `loader.conf` names no generation (`default nixos-*`,
# the newest). `ronoctua-machine` changes nothing for it: the trial is the same
# two lines of `loader.conf` and the same one try, on the UKI's own name
# (`<id>+1.efi` — the signature is of the image, not of its name), and the
# entries are what `bootctl list` says — a UKI's command line carries its
# `init=` like an entry file does. This proves it, on a machine with
# lanzaboote and its keys, under an OVMF that enforces them:
#
#   1. the firmware starts in setup mode and takes the machine's keys — the
#      one write of EFI variables there is, a person's own, made by mounting
#      them writable for it (they are read-only, `nix/modules/boot-loader.nix`);
#      from the next boot on it starts only what they signed;
#   2. the update's apply (`apply --next-boot`) of a system with a kernel
#      parameter of its own has lanzaboote sign its UKI, and arms the trial
#      against the last good one's UKI;
#   3. the reboot tries the trial — signed, the firmware enforcing — and the
#      bless makes it the last good system: its counter off, the boot menu's
#      default the trial's UKI;
#   4. and a boot after it takes it, signed;
#   5. the rescue's rollback (`ronoctua-machine rollback boot`) goes back to
#      the system before it, and the reboot comes up on that system: the
#      rollback makes the newest generation a copy of it, because the menu
#      boots the newest, whatever the profile names.
#
# The keys are the disposable ones (`nix/testing/sb-keys.nix`), copied where
# lanzaboote reads them as `tests/secureboot-vm.nix` does. The apply's BUILD
# is the stand-in (`tests/boot-trial-stand-in.nix`).
{
  pkgs,
  lib,
  # `ronoctua-machine`, extracted (`rust.machine`).
  machineTool,
  sbKeys,
  lanzabooteModule,
}:

let
  standIn = import ./boot-trial-stand-in.nix { inherit pkgs; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-secureboot-trial";

  nodes.machine =
    { config, extendModules, ... }:
    {
      imports = [
        ../nix/modules/machine.nix
        # The machines' own loader, which lanzaboote replaces here.
        ../nix/modules/boot-loader.nix
        lanzabooteModule
      ];

      # The one option of the distro's `base.nix` the machine module reads.
      options.ronoctua.user = lib.mkOption {
        type = lib.types.str;
        default = "person";
      };

      # THE SYSTEM THE UPDATE SETS FOR THE NEXT BOOT: this machine with a
      # kernel parameter of its own, which its UKI's signed command line
      # carries.
      options.test.staged = lib.mkOption {
        type = lib.types.package;
        internal = true;
      };

      config = {
        ronoctua.machine.tool = machineTool;
        ronoctua.machine.blessAfterSeconds = 5;
        ronoctua.machine.trialBootSeconds = 60;

        # LANZABOOTE IN PLACE OF SYSTEMD-BOOT, as a host that turns Secure Boot
        # on writes it (`boot-loader.nix` turns systemd-boot on by default).
        boot.loader.systemd-boot.enable = lib.mkForce false;
        boot.lanzaboote = {
          enable = true;
          pkiBundle = "/etc/secureboot";
          configurationLimit = 5;
        };
        # THE KEYS, WRITABLE, before the boot loader's installation signs with
        # them (`tests/secureboot-vm.nix` says why both).
        system.activationScripts.testSbKeys = {
          deps = [ "etc" ];
          text = ''
            rm -rf /etc/secureboot
            mkdir -p /etc/secureboot
            cp -rL ${sbKeys}/. /etc/secureboot/
            chmod -R u+rwX /etc/secureboot
          '';
        };

        test.staged =
          (extendModules {
            modules = [ { boot.kernelParams = [ "ronoctua.trial=signed" ]; } ];
          }).config.system.build.toplevel;

        environment.systemPackages = [
          pkgs.sbctl
          # `sbverify`: a file's signature against the machine's own key.
          pkgs.sbsigntool
        ];
        virtualisation = {
          useBootLoader = true;
          useEFIBoot = true;
          memorySize = 2048;
          diskSize = 4096;
          additionalPaths = [
            config.test.staged
            (standIn config.nix.package)
          ];
        };
      };
    };

  testScript =
    { nodes, ... }:
    let
      m = nodes.machine;
      fake = standIn m.nix.package;
    in
    ''
      import json

      STAGED = "${m.test.staged}"
      STATE = "${m.ronoctua.machine.stateDir}/state.json"
      MACHINE_DIR = "${m.ronoctua.machine.dir}"
      VENDOR = "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
      PROFILE = "/nix/var/nix/profiles/system"
      # The machine's own key, which signs every file lanzaboote installs.
      DB = "/etc/secureboot/keys/db/db.pem"


      def state():
          return json.loads(machine.succeed(f"cat {STATE}"))


      def efi(name):
          """A loader variable's value, or None when it is not set."""
          path = f"/sys/firmware/efi/efivars/{name}-{VENDOR}"
          status, out = machine.execute(f"od -An -tx1 -v {path}")
          if status != 0:
              return None
          raw = bytes.fromhex("".join(out.split()))[4:]
          return raw.decode("utf-16-le").rstrip("\x00")


      def entries():
          return json.loads(machine.succeed("bootctl list --json=short"))


      def entry(entry_id):
          return next(e for e in entries() if e["id"] == entry_id)


      def boots(entry_id):
          """The kernel command line the entry boots."""
          return entry(entry_id)["options"]


      def conf_line(key):
          """`loader.conf`'s `key` line's value, or None."""
          for line in machine.succeed("cat /boot/loader/loader.conf").splitlines():
              k, _, v = line.partition(" ")
              if k == key:
                  return v.strip()
          return None


      def uki_file(entry_id):
          """The name of the UKI `entry_id`'s file, its boot counter in it."""
          stem = entry_id.removesuffix(".efi")
          names = machine.succeed("ls /boot/EFI/Linux").split()
          return next((n for n in names if n == entry_id or n.startswith(stem + "+")), None)


      def default():
          """The trial's default: `loader.conf`'s, None while it is the newest's."""
          d = conf_line("default")
          return None if d in (None, "nixos-*") else d


      def oneshot():
          """The boot of once: the preferred UKI while it still has its try."""
          p = conf_line("preferred")
          return p if p and (uki_file(p) or "").endswith("+1.efi") else None


      def counted(entry_id):
          """Whether the UKI's file carries a boot counter."""
          return "+" in (uki_file(entry_id) or "")


      def no_variable_written():
          opts = machine.succeed("findmnt -no OPTIONS /sys/firmware/efi/efivars").split(",")
          assert "ro" in opts, opts
          for name in ("LoaderEntryDefault", "LoaderEntryOneShot"):
              assert efi(name) is None, f"{name} is set: {efi(name)!r}"


      def secure_boot():
          """What the firmware says of Secure Boot, as `bootctl` reads it."""
          out = machine.succeed("bootctl status 2>&1 || true")
          line = next((l.strip() for l in out.splitlines() if l.strip().startswith("Secure Boot:")), "")
          return line


      def signed(entry_id):
          """Whether the entry's file carries a signature of the machine's key."""
          path = entry(entry_id).get("path") or f"/boot/EFI/Linux/{entry_id}"
          status, out = machine.execute(f"sbverify --cert {DB} {path} 2>&1")
          print(out)
          return status == 0 and "Signature verification OK" in out


      def resolved(link):
          return machine.succeed(f"readlink -f {link}").strip()


      def apply(builds, *words):
          return machine.execute(
              f"RONOCTUA_NIX=${fake}/bin/nix RONOCTUA_TEST_BUILDS={builds} ronoctua-machine apply {' '.join(words)} 2>&1"
          )


      machine.start(allow_reboot=True)
      machine.wait_for_unit("multi-user.target")
      good = resolved("/run/booted-system")

      with subtest("the firmware starts in setup mode, and takes the machine's keys"):
          said = secure_boot()
          assert said.startswith("Secure Boot: disabled") and "setup" in said, said
          # sbctl's bank, writable (`tests/secureboot-vm.nix` says why).
          machine.succeed("mkdir -p /var/lib/sbctl")
          machine.succeed("cp -rL /etc/secureboot/. /var/lib/sbctl/")
          machine.succeed("chmod -R u+rwX /var/lib/sbctl")
          print(machine.succeed("sbctl setup --migrate 2>&1 || true"))
          # THE KEYS ARE VARIABLES: written by the person, deliberately, with
          # the mount made writable for it and read-only again after.
          no_variable_written()
          machine.succeed("mount -o remount,rw /sys/firmware/efi/efivars")
          print(machine.succeed("sbctl enroll-keys --yes-this-might-brick-my-machine 2>&1"))
          machine.succeed("mount -o remount,ro /sys/firmware/efi/efivars")
          machine.wait_for_unit("ronoctua-boot-bless.service")
          assert state()["last-good"]["toplevel"] == good, state()

      with subtest("the next boot is under Secure Boot, and is blessed"):
          machine.reboot()
          machine.wait_for_unit("multi-user.target")
          said = secure_boot()
          assert said.startswith("Secure Boot: enabled") and "user" in said, said
          assert resolved("/run/booted-system") == good
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["last-good"]["toplevel"] == good, s
          assert s["trial"] is None, s
          selected = efi("LoaderEntrySelected")
          assert selected.endswith(".efi") and f"init={good}/init" in boots(selected), selected
          assert signed(selected), "the UKI that booted is not signed with the machine's key"

      with subtest("the update's apply arms a trial of a signed UKI"):
          machine.succeed(f"mkdir -p {MACHINE_DIR}")
          status, out = apply(STAGED, "--next-boot")
          print(out)
          assert status == 0 and out.strip().endswith("set for the next boot; trial pending"), out
          assert resolved("/run/current-system") == good
          s = state()
          assert s["trial"] == {"toplevel": STAGED, "outcome": "pending"}, s
          once, back = oneshot(), default()
          print(json.dumps(entries(), indent=1))
          assert once and once.endswith(".efi") and f"init={STAGED}/init" in boots(once), once
          assert back and back.endswith(".efi") and f"init={good}/init" in boots(back), back
          assert signed(once), "lanzaboote did not sign the trial's UKI"
          assert counted(once), "a trial systemd-boot cannot start would go to the firmware"
          no_variable_written()

      with subtest("the trial boots signed, and the bless makes it the last good system"):
          machine.reboot()
          machine.wait_for_unit("multi-user.target")
          said = secure_boot()
          assert said.startswith("Secure Boot: enabled"), said
          assert resolved("/run/booted-system") == STAGED
          assert "ronoctua.trial=signed" in machine.succeed("cat /proc/cmdline").split()
          assert efi("LoaderEntrySelected") == once, efi("LoaderEntrySelected")
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["trial"] == {"toplevel": STAGED, "outcome": "good"}, s
          assert s["last-good"]["toplevel"] == STAGED, s
          assert oneshot() is None and default() is None
          assert not counted(once), "blessed: the counter is off the trial's UKI"
          no_variable_written()
          # THE MENU'S OWN DEFAULT, with no variable: lanzaboote's newest UKI.
          menu = [e["id"] for e in entries() if e.get("isDefault")]
          assert menu == [once], (menu, once)

      with subtest("a boot with no variable takes the blessed generation, signed"):
          machine.reboot()
          machine.wait_for_unit("multi-user.target")
          said = secure_boot()
          assert said.startswith("Secure Boot: enabled"), said
          assert resolved("/run/booted-system") == STAGED
          assert efi("LoaderEntrySelected") == once
          assert signed(once)
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["last-good"]["toplevel"] == STAGED, s
          assert s["trial"]["outcome"] == "good", s

      with subtest("the rescue's rollback lasts across a reboot under lanzaboote"):
          # `loader.conf` names no generation (`default nixos-*`): a boot with
          # no variable takes the NEWEST. A rollback that moved the profile
          # back onto the older generation would leave the system it undid the
          # newest, and the reboot would boot it again.
          before = machine.succeed(f"readlink {PROFILE}").strip()
          out = machine.succeed("ronoctua-machine rollback boot 2>&1")
          print(out)
          assert resolved(PROFILE) == good, out
          assert resolved("/run/current-system") == STAGED, "the rollback for the next boot switched now"
          print(machine.succeed("cat /boot/loader/loader.conf"))
          print(json.dumps(entries(), indent=1))
          machine.reboot()
          machine.wait_for_unit("multi-user.target")
          said = secure_boot()
          assert said.startswith("Secure Boot: enabled"), said
          # WHAT BOOTED, first: the system gone back to, not the one undone.
          assert resolved("/run/booted-system") == good, "the reboot came up on the system the rollback undid"
          assert "ronoctua.trial=signed" not in machine.succeed("cat /proc/cmdline").split()
          selected = efi("LoaderEntrySelected")
          assert selected.endswith(".efi") and f"init={good}/init" in boots(selected), selected
          assert signed(selected), "the UKI the rollback booted is not signed with the machine's key"
          # BY A NEW GENERATION, a copy of it: the newest, which the menu boots.
          on = machine.succeed(f"readlink {PROFILE}").strip()
          newest = machine.succeed(f"ls -d {PROFILE}-*-link | sort -t- -k2,2n | tail -n1 | xargs basename").strip()
          assert on != before and on == newest, (on, before, newest)
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["last-good"]["toplevel"] == good, s
    '';
}
