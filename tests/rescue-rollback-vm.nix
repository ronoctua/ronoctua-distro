# THE ROLLBACK THROUGH THE RESCUE (`rescue-rolls-back-the-trial`).
#
# The rescue console's way back (`ronoctua-rescue-rollback@switch|boot`,
# `ronoctua-machine rollback`, `crates/ronoctua-machine/src/rollback.rs`) on a
# machine that boots the way an installed one does — systemd-boot, from an EFI
# whose variables persist across reboots — with a boot trial armed:
#
#   1. an apply switches to a new system that does not boot like the last good
#      one, and arms its trial: the next boot would try it once;
#   2. the rescue opens as `Ctrl+Alt+Del` opens it, and its row "go back to the
#      previous generation" — pressed twice, as a row that cannot be taken back
#      is — goes back NOW: the system that runs is the last good one again, the
#      profile is on a new generation that is a copy of it, the newest, the
#      trial is recorded `failed` and the boot loader's variables are gone;
#   3. the reboot comes up on that generation, by the boot's identity — a new
#      boot, the entry the loader says it started boots the last good system,
#      and the command line is not the trial's —, and the next apply does not
#      build the system rolled away from again;
#   4. the same for a system the UPDATE set for the next boot (`apply
#      --next-boot`): the row "previous generation at next boot" puts the
#      profile back on the system that runs, switches nothing, disarms the
#      trial, and the reboot comes up on what ran;
#   5. and a row that fails says why WHOLE, on the console's terminal, until a
#      key: going back with nowhere to go back to — the profile a copy of the
#      first generation's system — shows `ronoctua-machine`'s own sentence,
#      not a line pointing at the journal.
#
# The new systems are this machine with a kernel parameter of their own, so
# their trial is real, and the first with `ronoctua-machine` of ANOTHER store
# path — as the system an update brings is, the distro having moved: every
# unit that runs it is defined differently there, the rescue's way back
# included, and switching back must not stop the unit doing the switching.
#
# The apply's BUILD is the stand-in (`tests/boot-trial-stand-in.nix`). The
# AUTHORIZATION is the real one: the row asks systemd itself, polkit asks the
# person's password on the console's terminal through the console's own agent
# (`auth_self_keep`, `nix/modules/rescue.nix`), and the test types it there.
{
  pkgs,
  lib,
  ronoctua,
}:

let
  standIn = import ./boot-trial-stand-in.nix { inherit pkgs; };
  # The person's password, which polkit asks for on the console.
  password = "a-passphrase-for-the-test";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-rescue-rollback";

  nodes.machine =
    { config, extendModules, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/rescue.nix
        ../nix/modules/machine.nix
      ];

      # A SYSTEM THE PERSON GOES BACK FROM, switched to now; and one the
      # update set for the next boot.
      options.test = {
        away = lib.mkOption {
          type = lib.types.package;
          internal = true;
        };
        waiting = lib.mkOption {
          type = lib.types.package;
          internal = true;
        };
      };

      config = {
        _module.args.ronoctua = ronoctua;
        ronoctua.machine.tool = ronoctua.machine;
        # A few seconds after the default target, not a minute: the test waits
        # for the bless at every boot.
        ronoctua.machine.blessAfterSeconds = 5;
        # THE MACHINE FLAKE'S LOCK, which every system carries as an installed
        # one does: an apply that does not move its `nixpkgs` switches now.
        ronoctua.machine.source = ./fixtures/machine-nixpkgs-lock;

        test.away =
          (extendModules {
            modules = [
              {
                boot.kernelParams = [ "ronoctua.trial=rolled-away" ];
                # The same program at another store path: the units that run
                # it are another system's.
                ronoctua.machine.tool = lib.mkForce (
                  pkgs.symlinkJoin {
                    name = "ronoctua-machine-of-another-revision";
                    paths = [ ronoctua.machine ];
                  }
                );
              }
            ];
          }).config.system.build.toplevel;
        test.waiting =
          (extendModules {
            modules = [ { boot.kernelParams = [ "ronoctua.trial=waiting" ]; } ];
          }).config.system.build.toplevel;

        # The test driver names the node; `base.nix` wants its prefix.
        networking.hostName = lib.mkForce "${config.ronoctua.hostnamePrefix}rescue";
        # No /persist in this VM: `base.nix` points the password hash there.
        users.users.${config.ronoctua.user} = {
          hashedPasswordFile = lib.mkForce null;
          inherit password;
        };
        users.users.root.hashedPassword = lib.mkForce null;

        virtualisation = {
          useBootLoader = true;
          useEFIBoot = true;
          memorySize = 2048;
          diskSize = 6144;
          additionalPaths = [
            config.test.away
            config.test.waiting
            (standIn config.nix.package)
          ];
        };
        boot.loader.systemd-boot.configurationLimit = 5;
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
      from datetime import timedelta

      AWAY = "${m.test.away}"
      WAITING = "${m.test.waiting}"
      STATE = "${m.ronoctua.machine.stateDir}/state.json"
      MACHINE_DIR = "${m.ronoctua.machine.dir}"
      # The lock the running system carries, which the folder starts from.
      CARRIED = "/run/current-system/${m.ronoctua.machine.environment.RONOCTUA_MACHINE_SYSTEM_LOCK}"
      VENDOR = "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
      PROFILE = "/nix/var/nix/profiles/system"
      PASSWORD = "${password}"


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


      def boots(entry_id):
          """The kernel command line the entry boots."""
          entries = json.loads(machine.succeed("bootctl list --json=short"))
          return next(e for e in entries if e["id"] == entry_id)["options"]


      def menu_default():
          """The entry `loader.conf` names, which a boot with no variable takes."""
          loader = machine.succeed("cat /boot/loader/loader.conf")
          line = next((l for l in loader.splitlines() if l.startswith("default ")), None)
          assert line, f"loader.conf has no `default`:\n{loader}"
          return line.split(None, 1)[1].strip()


      def resolved(link):
          return machine.succeed(f"readlink -f {link}").strip()


      def generation():
          """The system profile's generation, by its link's name."""
          return machine.succeed(f"readlink {PROFILE}").strip()


      def newest():
          """The newest generation's link, which the profile must be on."""
          return machine.succeed(f"ls -d {PROFILE}-*-link | sort -t- -k2,2n | tail -n1 | xargs basename").strip()


      def boot_id():
          return machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()


      def apply(builds, *words):
          return machine.execute(
              f"RONOCTUA_NIX=${fake}/bin/nix RONOCTUA_TEST_BUILDS={builds} ronoctua-machine apply {' '.join(words)} 2>&1"
          )


      def screen():
          """The rescue console's text, off its virtual console."""
          return machine.succeed("tr -d '\\0' < /dev/vcs8 | fold -w 160")


      def restart():
          """The machine off and on again: `machine.reboot()` presses
          `Ctrl+Alt+Del`, which opens the rescue here. The firmware keeps its
          variables, so a one-shot left set is read by the boot after."""
          machine.shutdown()
          machine.start()


      def runs(unit):
          """How each start of the unit in this boot ended, by systemd's journal
          (`done`, `failed`…): a oneshot that ended is unloaded, and `systemctl
          show` no longer knows it ran."""
          out = machine.succeed(f"journalctl -b -u {unit} -o json --output-fields=JOB_TYPE,JOB_RESULT")
          lines = (json.loads(line) for line in out.splitlines() if line.strip())
          return [e["JOB_RESULT"] for e in lines if e.get("JOB_TYPE") == "start" and "JOB_RESULT" in e]


      def through_the_rescue(row, unit):
          """Opens the rescue console as `Ctrl+Alt+Del` does, goes to its System
          section, to its `row`-th row, and presses Enter on it twice: the first
          arms it, the second runs it. The console hands its terminal to polkit's
          agent, which asks the person's password there, and the test types it;
          how the unit's run ended."""
          before = len(runs(unit))
          machine.send_key("ctrl-alt-delete")
          machine.wait_until_succeeds(
              "systemctl is-active --quiet ronoctua-rescue.service", timeout=timedelta(seconds=30)
          )
          machine.wait_until_succeeds(
              "tr -d '\\0' < /dev/vcs8 | grep -q Terminal", timeout=timedelta(seconds=30)
          )
          # The sidebar opens on State; up wraps to System, the last.
          machine.send_key("up")
          machine.send_key("ret")
          for _ in range(row):
              machine.send_key("down")
          machine.send_key("ret")
          machine.send_key("ret")
          # POLKIT'S QUESTION, on the plain terminal the console left — the
          # agent's own words, not the console's (its rows say "asks for your
          # password", with no colon).
          machine.wait_until_succeeds(
              "tr -d '\\0' < /dev/vcs8 | grep -q 'assword:'", timeout=timedelta(seconds=60)
          )
          print(screen())
          machine.screenshot(f"rescue-asks-{unit.split('@')[1].split('.')[0]}")
          machine.send_chars(PASSWORD + "\n")
          for _ in range(120):
              ended = runs(unit)
              if len(ended) > before:
                  print(screen())
                  print(machine.succeed(f"journalctl -b --no-pager -u {unit}"))
                  return ended[before]
              machine.sleep(timedelta(seconds=1))
          raise AssertionError(f"the rescue's row did not run {unit}:\n{screen()}")


      machine.start()
      machine.wait_for_unit("multi-user.target")
      good = resolved("/run/booted-system")
      machine.wait_for_unit("ronoctua-boot-bless.service")
      assert state()["last-good"]["toplevel"] == good, state()
      first = generation()

      with subtest("an apply switches to a new system and arms its trial"):
          machine.succeed(f"mkdir -p {MACHINE_DIR} && cp {CARRIED} {MACHINE_DIR}/flake.lock")
          status, out = apply(AWAY)
          print(out)
          assert status == 0, out
          assert resolved("/run/current-system") == AWAY
          assert resolved(PROFILE) == AWAY
          s = state()
          assert s["trial"] == {"toplevel": AWAY, "outcome": "pending"}, s
          assert f"init={AWAY}/init" in boots(efi("LoaderEntryOneShot"))
          assert f"init={good}/init" in boots(efi("LoaderEntryDefault"))

      with subtest("the rescue goes back now, and disarms the trial"):
          ended = through_the_rescue(0, "ronoctua-rescue-rollback@switch.service")
          # NOT STOPPED BY THE SWITCH IT RAN, into a system whose unit is
          # another revision's.
          assert ended == "done", ended
          assert resolved("/run/current-system") == good, "the system that runs is not the last good one"
          # BACK BY A NEW GENERATION, a copy of the last good one: the newest.
          back = generation()
          assert resolved(PROFILE) == good
          assert back != first and back == newest(), (back, first, newest())
          s = state()
          assert s["trial"] == {"toplevel": AWAY, "outcome": "failed"}, s
          assert efi("LoaderEntryOneShot") is None, "the next boot would still try the system left"
          assert efi("LoaderEntryDefault") is None
          assert f"init={good}/init" in boots(menu_default())

      with subtest("the reboot comes up on the generation it came back to"):
          before = boot_id()
          restart()
          machine.wait_for_unit("multi-user.target")
          assert boot_id() != before, "no new boot"
          assert resolved("/run/booted-system") == good
          selected = efi("LoaderEntrySelected")
          assert f"init={good}/init" in boots(selected), selected
          assert "ronoctua.trial=" not in machine.succeed("cat /proc/cmdline")
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["trial"] == {"toplevel": AWAY, "outcome": "failed"}, s
          assert s["last-good"]["toplevel"] == good, s
          assert efi("LoaderEntryOneShot") is None and efi("LoaderEntryDefault") is None

      with subtest("the system rolled away from is not built into the machine again"):
          status, out = apply(AWAY)
          print(out)
          assert status == 1 and out.strip().endswith("refused did-not-boot"), out
          assert generation() == back

      with subtest("a system the update set for the next boot is dropped at the next boot"):
          status, out = apply(WAITING, "--next-boot")
          print(out)
          assert status == 0 and out.strip().endswith("set for the next boot; trial pending"), out
          assert resolved(PROFILE) == WAITING
          assert resolved("/run/current-system") == good
          ended = through_the_rescue(1, "ronoctua-rescue-rollback@boot.service")
          assert ended == "done", ended
          # BACK ON THE SYSTEM THAT RUNS, which nothing switched, by a new
          # generation: the newest, past the one that waited.
          again = generation()
          assert resolved(PROFILE) == good
          assert again not in (first, back) and again == newest(), (again, first, back, newest())
          assert resolved("/run/current-system") == good
          s = state()
          assert s["trial"] == {"toplevel": WAITING, "outcome": "failed"}, s
          assert efi("LoaderEntryOneShot") is None and efi("LoaderEntryDefault") is None
          assert f"init={good}/init" in boots(menu_default())
          before = boot_id()
          restart()
          machine.wait_for_unit("multi-user.target")
          assert boot_id() != before, "no new boot"
          assert resolved("/run/booted-system") == good
          assert f"init={good}/init" in boots(efi("LoaderEntrySelected"))
          assert "ronoctua.trial=" not in machine.succeed("cat /proc/cmdline")
          machine.wait_for_unit("ronoctua-boot-bless.service")
          assert state()["trial"] == {"toplevel": WAITING, "outcome": "failed"}, state()

      with subtest("a row that fails says why, whole, until a key"):
          # The profile is a copy of a copy of the first generation's system:
          # there is nowhere to go back to — not the system rolled away from,
          # nor the one that waited, both after the first.
          ended = through_the_rescue(0, "ronoctua-rescue-rollback@switch.service")
          assert ended == "failed", ended
          machine.wait_until_succeeds(
              "tr -d '\\0' < /dev/vcs8 | grep -q 'no generation to go back to'",
              timeout=timedelta(seconds=30),
          )
          print(screen())
          machine.screenshot("rescue-says-why")
          # Unfolded: a sentence the console printed sits on one row of it.
          said = machine.succeed("tr -d '\\0' < /dev/vcs8")
          assert "ronoctua-rescue-rollback@switch.service: failed" in said, said
          assert generation() == again
          # A key gives the console back.
          machine.send_key("spc")
          machine.wait_until_succeeds(
              "tr -d '\\0' < /dev/vcs8 | grep -q Terminal", timeout=timedelta(seconds=30)
          )
    '';
}
