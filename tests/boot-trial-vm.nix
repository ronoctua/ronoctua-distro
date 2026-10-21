# THE BOOT TRIAL FALLS BACK (`boot-trial-falls-back`).
#
# NixOS 26.05 has no boot counting, so the way back is ours
# (`crates/ronoctua-machine`, `nix/modules/machine.nix`): after a switch,
# `bootctl set-default` names the last good entry and `bootctl set-oneshot`
# the new one, and a while after the system's default target
# `ronoctua-boot-bless.service` reads which of the two booted. This proves it
# on a machine that boots the way an installed one does — systemd-boot, from
# an EFI whose variables PERSIST across reboots (`useBootLoader`,
# `useEFIBoot`; the test keeps the variables file for the whole run) — and
# whose default target is the distro's own, `multi-user.target` (the session
# is a console login, and `graphical.target` is never reached):
#
#   1. the first boot is blessed — the bless unit hangs off the target the
#      boot really reaches: what booted and lasted is the last good system;
#   2. an apply of a system that CANNOT boot (its `init` is no system's:
#      stage 1 cannot activate it, and the kernel panics) arms the trial —
#      default on the last good entry, one-shot on the new one — and a bless
#      in that same boot judges nothing: the trial has not been tried;
#   3. the reboot tries the trial ONCE — the console shows its panic — and
#      comes back on the last good one by itself; the bless marks the trial
#      `failed`, puts the system profile back on the last good system and
#      unsets the EFI default, so `loader.conf` names it;
#   4. the boot after that is the last good one with no variable set;
#   5. a trial whose STAGE 2 never reaches the default target — it ends in
#      emergency mode, as a mount that failed does, PID 1 alive and the
#      watchdog petted — is restarted by its deadline
#      (`ronoctua-boot-trial-deadline.service`), and comes back on the last
#      good one; the next apply does not build the trial that failed again.
#
# The apply's BUILD is a stand-in (`RONOCTUA_NIX` names a `nix` that answers
# a build with the broken system, built here): building a NixOS system inside
# the VM, offline, is not what this is about. Everything after the build —
# `nix-env`, `switch-to-configuration`, the boot loader's own builder,
# `bootctl`, the firmware, the bless unit — is the real one.
{
  pkgs,
  lib,
  # `ronoctua-machine`, extracted (`rust.machine`).
  machineTool,
}:

let
  # A SYSTEM THAT DOES NOT BOOT, made of a good one: every part linked, an
  # `init` of its own, and one more kernel parameter — so it does not boot
  # like the last good one and a trial is armed. The boot loader's builder
  # writes the entry from `boot.json`, which says the same.
  variant =
    name: good: init: param:
    pkgs.runCommand "nixos-system-${name}" { nativeBuildInputs = [ pkgs.jq ]; } ''
      mkdir $out
      for f in ${good}/*; do
        ln -s "$f" "$out/$(basename "$f")"
      done
      rm $out/init $out/kernel-params $out/boot.json
      ln -s ${init} $out/init
      printf '%s ${param}' "$(cat ${good}/kernel-params)" > $out/kernel-params
      jq --arg out "$out" \
        '."org.nixos.bootspec.v1" |= (.init = $out + "/init" | .toplevel = $out | .kernelParams += ["${param}"])' \
        ${good}/boot.json > $out/boot.json
    '';

  # STAGE 1 CANNOT HAND OVER: the `init` it is given is no system's (the
  # systemd stage 1 looks for `prepare-root` beside it and finds none), so
  # the activation fails and `boot.panic_on_fail` panics the kernel.
  broken =
    good: variant "trial-that-does-not-boot" good "${pkgs.coreutils}/bin/false" "ronoctua.trial=broken";

  # STAGE 2 ENDS IN EMERGENCY MODE: the good system's own `init` (stage 1
  # activates the good system), asked for `emergency.target` — where a mount
  # that failed would leave it, the console's root account locked, PID 1
  # alive and petting the watchdog. Only the trial's deadline gets it out.
  stuck = good: variant "trial-stuck-in-stage-2" good "${good}/init" "systemd.unit=emergency.target";

  # THE STAND-IN FOR THE BUILD: `nix build … --out-link OUT` links OUT to the
  # system the test names in `RONOCTUA_TEST_BUILDS` (the apply hands its
  # environment down); `nix-env` beside it is the machine's own.
  fakeNix =
    nix:
    pkgs.runCommand "nix-that-builds-what-the-test-says" { } ''
      mkdir -p $out/bin
      ln -s ${nix}/bin/nix-env $out/bin/nix-env
      cat > $out/bin/nix <<'EOF'
      #!${pkgs.runtimeShell}
      out="" prev=""
      for a in "$@"; do
        if [ "$prev" = --out-link ]; then out=$a; fi
        prev=$a
      done
      exec ${pkgs.coreutils}/bin/ln -sfn "$RONOCTUA_TEST_BUILDS" "$out"
      EOF
      chmod +x $out/bin/nix
    '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-boot-trial";

  nodes.machine =
    { config, ... }:
    {
      imports = [ ../nix/modules/machine.nix ];

      # The one option of the distro's `base.nix` the machine module reads.
      options.ronoctua.user = lib.mkOption {
        type = lib.types.str;
        default = "noctua";
      };

      config = {
        ronoctua.machine.tool = machineTool;
        # A few seconds after the default target, not a minute: the test
        # waits for it four times; and a stuck trial gets half a minute.
        ronoctua.machine.blessAfterSeconds = 5;
        ronoctua.machine.trialBootSeconds = 30;

        virtualisation = {
          useBootLoader = true;
          useEFIBoot = true;
          memorySize = 2048;
          diskSize = 4096;
          additionalPaths = [
            (broken config.system.build.toplevel)
            (stuck config.system.build.toplevel)
            (fakeNix config.nix.package)
          ];
        };
        boot.loader.systemd-boot.enable = true;
        boot.loader.systemd-boot.configurationLimit = 5;
        boot.loader.efi.canTouchEfiVariables = true;
        boot.initrd.systemd.enable = true;
      };
    };

  testScript =
    { nodes, ... }:
    let
      good = nodes.machine.system.build.toplevel;
      fake = fakeNix nodes.machine.nix.package;
    in
    ''
      import json

      BROKEN = "${broken good}"
      STUCK = "${stuck good}"
      VENDOR = "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"


      def state():
          return json.loads(machine.succeed("cat /var/lib/ronoctua-machine/state.json"))


      def efi(name):
          """A loader variable's value, or None when it is not set."""
          path = f"/sys/firmware/efi/efivars/{name}-{VENDOR}"
          status, out = machine.execute(f"od -An -tx1 -v {path}")
          if status != 0:
              return None
          raw = bytes.fromhex("".join(out.split()))[4:]
          return raw.decode("utf-16-le").rstrip("\x00")


      def entry_of(entry_id):
          entries = json.loads(machine.succeed("bootctl list --json=short"))
          return next(e for e in entries if e["id"] == entry_id)


      def apply(builds):
          return machine.execute(
              f"RONOCTUA_NIX=${fake}/bin/nix RONOCTUA_TEST_BUILDS={builds} ronoctua-machine apply 2>&1"
          )


      machine.start(allow_reboot=True)
      machine.wait_for_unit("multi-user.target")
      good = machine.succeed("readlink -f /run/booted-system").strip()

      with subtest("the first boot is blessed: what booted and lasted is the last good system"):
          # WHAT `default.target` IS, asked of PID 1 — the unit the boot
          # reached. `systemctl get-default` reads the link from the files,
          # and on a store-backed `/etc` systemd 260 answers its own name.
          target = machine.succeed("systemctl show -p Id --value default.target").strip()
          assert target == "multi-user.target", target
          wanted = machine.succeed(f"systemctl show -p Wants --value {target}")
          assert "ronoctua-boot-bless.service" in wanted, wanted
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["last-good"]["toplevel"] == good, s
          assert s["trial"] is None, s

      with subtest("an apply arms the trial against the last good entry"):
          machine.succeed("mkdir -p /etc/nixos")
          status, out = apply(BROKEN)
          print(out)
          assert status == 0, out
          s = state()
          assert s["trial"] == {"toplevel": BROKEN, "outcome": "pending"}, s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == BROKEN
          oneshot, default = efi("LoaderEntryOneShot"), efi("LoaderEntryDefault")
          assert f"init={BROKEN}/init" in entry_of(oneshot)["options"], oneshot
          assert f"init={good}/init" in entry_of(default)["options"], default

      with subtest("a bless in the boot that armed the trial judges nothing"):
          machine.succeed("systemctl restart ronoctua-boot-bless.service")
          s = state()
          assert s["trial"]["outcome"] == "pending", s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == BROKEN

      with subtest("the trial is tried once, and the machine comes back on its own"):
          machine.reboot()
          # The one-shot entry booted — its own command line — and the
          # kernel panicked in it.
          machine.wait_for_console_text("Kernel command line: .*ronoctua.trial=broken")
          machine.wait_for_console_text("Kernel panic - not syncing")
          machine.wait_for_unit("multi-user.target")
          assert machine.succeed("readlink -f /run/booted-system").strip() == good
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["trial"] == {"toplevel": BROKEN, "outcome": "failed"}, s
          assert s["last-good"]["toplevel"] == good, s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == good
          assert efi("LoaderEntryOneShot") is None, "the one-shot was spent"
          assert efi("LoaderEntryDefault") is None, "the default is loader.conf's again"
          newest = machine.succeed("readlink /nix/var/nix/profiles/system").strip()
          number = newest.removeprefix("system-").removesuffix("-link")
          loader = machine.succeed("cat /boot/loader/loader.conf")
          assert f"default nixos-generation-{number}.conf" in loader, loader

      with subtest("the boot after that is the last good one, by the menu alone"):
          machine.reboot()
          machine.wait_for_unit("multi-user.target")
          assert machine.succeed("readlink -f /run/booted-system").strip() == good
          machine.wait_for_unit("ronoctua-boot-bless.service")
          assert state()["trial"]["outcome"] == "failed"

      with subtest("the system that did not boot is not built into the machine again"):
          status, out = apply(BROKEN)
          print(out)
          assert status == 1 and out.strip().endswith("refused did-not-boot"), out
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == good
          assert efi("LoaderEntryOneShot") is None

      with subtest("a trial whose stage 2 never comes up is restarted by its deadline"):
          status, out = apply(STUCK)
          print(out)
          assert status == 0, out
          assert state()["trial"] == {"toplevel": STUCK, "outcome": "pending"}
          # Not a trial boot: the deadline of this one ended at once.
          left = machine.succeed(
              "systemctl show -p ActiveState --value ronoctua-boot-trial-deadline.service"
          ).strip()
          assert left == "inactive", left
          machine.reboot()
          # The one-shot entry booted into emergency mode; its deadline said
          # so to the kernel's log, and restarted it.
          machine.wait_for_console_text("the boot trial did not reach multi-user.target in time")
          machine.wait_for_unit("multi-user.target")
          assert machine.succeed("readlink -f /run/booted-system").strip() == good
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["trial"] == {"toplevel": STUCK, "outcome": "failed"}, s
          assert s["last-good"]["toplevel"] == good, s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == good
          assert efi("LoaderEntryDefault") is None
    '';
}
