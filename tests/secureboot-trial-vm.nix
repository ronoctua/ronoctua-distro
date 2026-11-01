# A BOOT TRIAL WITH SECURE BOOT (`boot-trial-is-blessed-under-secure-boot`).
#
# The distro boots systemd-boot with no signature (`docs/ARCHITECTURE.md`,
# "Secure Boot e TPM2"); a host that turns Secure Boot on imports lanzaboote,
# which REPLACES systemd-boot's builder: every generation becomes a UKI
# (`/boot/EFI/Linux/nixos-generation-<n>-<hash>.efi`), signed with the
# machine's keys, and `loader.conf` names no generation (`default nixos-*`,
# the newest). `ronoctua-machine` changes nothing for it: the trial is the same
# two EFI variables, the entries are what `bootctl list` says — a UKI's
# command line carries its `init=` like an entry file does. This proves it, on
# a machine with lanzaboote and its keys, under an OVMF that enforces them:
#
#   1. the firmware starts in setup mode and takes the machine's keys; from
#      the next boot on it starts only what they signed;
#   2. the update's apply (`apply --next-boot`) of a system with a kernel
#      parameter of its own has lanzaboote sign its UKI, and arms the trial
#      against the last good one's UKI;
#   3. the reboot tries the trial — signed, the firmware enforcing — and the
#      bless makes it the last good system: the variables unset, the boot
#      menu's default is the trial's UKI;
#   4. and a boot with no variable at all takes it, signed.
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


      def restarts():
          """Whether systemd-boot restarts on a start that fails: the trial's line."""
          return "reboot-on-error yes" in machine.succeed("cat /boot/loader/loader.conf").splitlines()


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
          print(machine.succeed("sbctl enroll-keys --yes-this-might-brick-my-machine 2>&1"))
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
          once, back = efi("LoaderEntryOneShot"), efi("LoaderEntryDefault")
          print(json.dumps(entries(), indent=1))
          assert once and once.endswith(".efi") and f"init={STAGED}/init" in boots(once), once
          assert back and back.endswith(".efi") and f"init={good}/init" in boots(back), back
          assert signed(once), "lanzaboote did not sign the trial's UKI"
          assert restarts(), "a trial systemd-boot cannot start would go to the firmware"

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
          assert efi("LoaderEntryOneShot") is None and efi("LoaderEntryDefault") is None
          assert not restarts(), "a verdict takes the restart away with the default"
          # THE MENU'S OWN DEFAULT, with no variable: lanzaboote's newest UKI.
          default = [e["id"] for e in entries() if e.get("isDefault")]
          assert default == [once], (default, once)

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
    '';
}
