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
#      good one; the next apply does not build the trial that failed again;
#   6. a trial whose kernel the LOADER cannot start falls back with no
#      person at the menu: systemd-boot restarts (the `reboot-on-error` the
#      trial posts in `loader.conf`), GRUB boots its `fallback` in the same
#      boot — and either way the last good system is the one boot that
#      follows.
#
# The apply's BUILD is a stand-in (`RONOCTUA_NIX` names a `nix` that answers
# a build with the broken system, built here): building a NixOS system inside
# the VM, offline, is not what this is about. Everything after the build —
# `nix-env`, `switch-to-configuration`, the boot loader's own builder,
# `bootctl`, the firmware, the bless unit — is the real one.
#
# AND THE SAME ON A BIOS MACHINE (`firmware = "bios"`,
# `boot-trial-falls-back-on-bios`): GRUB, the loader a BIOS machine boots
# (`nix/modules/boot-loader.nix`), with the trial in GRUB's environment block —
# `next_entry` for the boot of once, which GRUB's menu clears itself, and
# `ronoctua_default` for the default — read off the block with
# `grub-editenv`, and each entry found in `grub.cfg` by its `init=`. The
# script is one; only where the loader keeps the two differs.
{
  pkgs,
  lib,
  # `ronoctua-machine`, extracted (`rust.machine`).
  machineTool,
  # How the VM's firmware starts it: `uefi` (systemd-boot) or `bios` (GRUB).
  firmware ? "uefi",
}:

let
  # A SYSTEM THAT DOES NOT BOOT, made of a good one: every part linked, an
  # `init` of its own, and one more kernel parameter — so it does not boot
  # like the last good one and a trial is armed. The boot loader's builder
  # writes the entry from `boot.json`, which says the same.
  #
  # THE `init` IS COPIED, NEVER LINKED, as a real system's is a file of its
  # own: GRUB's builder writes `init=` resolved (`Cwd::abs_path`), and a link
  # would put another derivation's path in the menu — the entry would boot
  # the variant and name the file it points at. What a variant takes AWAY
  # (`away`) is what stage 1 would have found beside it, and a `kernel` of
  # its own is the one both builders copy beside the menu.
  variant =
    name: good:
    {
      init,
      param,
      away ? [ ],
      kernel ? null,
    }:
    pkgs.runCommand "nixos-system-${name}" { nativeBuildInputs = [ pkgs.jq ]; } ''
      mkdir $out
      for f in ${good}/*; do
        ln -s "$f" "$out/$(basename "$f")"
      done
      rm $out/init $out/kernel-params $out/boot.json ${lib.concatMapStringsSep " " (f: "$out/${f}") away}
      cp ${init} $out/init
      ${lib.optionalString (kernel != null) "ln -sfn ${kernel} $out/kernel"}
      printf '%s ${param}' "$(cat ${good}/kernel-params)" > $out/kernel-params
      jq --arg out "$out" \
        '."org.nixos.bootspec.v1" |= (.init = $out + "/init" | .toplevel = $out | .kernelParams += ["${param}"]${
          lib.optionalString (kernel != null) ''| .kernel = "${kernel}"''
        })' \
        ${good}/boot.json > $out/boot.json
    '';

  # STAGE 1 CANNOT HAND OVER: the `init` it is given is no system's (the
  # systemd stage 1 looks for `prepare-root` beside it and finds none — the
  # variant takes it away), so the activation fails and `boot.panic_on_fail`
  # panics the kernel.
  broken =
    good:
    variant "trial-that-does-not-boot" good {
      init = "${pkgs.coreutils}/bin/false";
      param = "ronoctua.trial=broken";
      away = [ "prepare-root" ];
    };

  # STAGE 2 ENDS IN EMERGENCY MODE: the good system's own `init` (stage 1
  # activates the good system), asked for `emergency.target` — where a mount
  # that failed would leave it, the console's root account locked, PID 1
  # alive and petting the watchdog. Only the trial's deadline gets it out.
  stuck =
    good:
    variant "trial-stuck-in-stage-2" good {
      init = "${good}/init";
      param = "systemd.unit=emergency.target";
    };

  # THE LOADER CANNOT START IT: its kernel is no kernel — no Linux header
  # for GRUB, no PE image for systemd-boot —, as a kernel the loader cannot
  # read or an initrd too big for its memory would be. No kernel runs, so
  # neither the deadline nor the bless can help: only the loader can.
  unloadable =
    good:
    variant "trial-the-loader-cannot-start" good {
      init = "${good}/init";
      param = "ronoctua.trial=unloadable";
      away = [ "kernel" ];
      kernel = pkgs.writeText "not-a-kernel" "no boot loader starts this\n";
    };

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
  name = "ronoctua-boot-trial" + lib.optionalString (firmware == "bios") "-bios";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/machine.nix
        # The machines' own loader, by the firmware.
        ../nix/modules/boot-loader.nix
      ];

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

        ronoctua.firmware = firmware;
        # `grub-editenv`, to read GRUB's block from the test.
        environment.systemPackages = lib.optional (firmware == "bios") pkgs.grub2;
        virtualisation = {
          useBootLoader = true;
          useEFIBoot = firmware == "uefi";
          memorySize = 2048;
          diskSize = 4096;
          additionalPaths = [
            (broken config.system.build.toplevel)
            (stuck config.system.build.toplevel)
            (unloadable config.system.build.toplevel)
            (fakeNix config.nix.package)
          ];
        };
        boot.loader.systemd-boot.configurationLimit = 5;
        boot.loader.grub.configurationLimit = 5;
        # GRUB on the serial line too, which is the console the test reads:
        # it says there when it falls back.
        boot.loader.grub.extraConfig = lib.mkBefore ''
          serial
          terminal_output --append serial
        '';
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
      UNLOADABLE = "${unloadable good}"
      VENDOR = "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
      GRUB = "${firmware}" == "bios"
      # What GRUB puts on the command line of a boot that tries.
      MARK = "${import ../nix/boot-trial-mark.nix}"


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


      def grub_block():
          """GRUB's environment block: its variables, set and not empty."""
          status, out = machine.execute("grub-editenv /boot/grub/grubenv list")
          if status != 0:
              return {}
          pairs = (line.split("=", 1) for line in out.splitlines() if "=" in line)
          return {k: v for k, v in pairs if v}


      def grub_menu():
          """GRUB's entries, as `install-grub.pl` writes them: each title's
          path (the submenus' and its own, by `>`) and its `linux` line."""
          menu, open_, entry = {}, [], None
          for line in machine.succeed("cat /boot/grub/grub.cfg").splitlines():
              line = line.strip()
              title = line.split('"')[1] if line.count('"') >= 2 else None
              if line.startswith("submenu ") and title:
                  open_.append(title)
              elif line.startswith("menuentry ") and title:
                  entry = ">".join([*open_, title])
                  open_.append(None)
              elif line.startswith("linux ") and entry:
                  menu[entry] = line
              elif line == "}" and open_:
                  if open_.pop() is None:
                      entry = None
          return menu


      def default():
          """The trial's default: `LoaderEntryDefault`, or GRUB's `ronoctua_default`."""
          return grub_block().get("ronoctua_default") if GRUB else efi("LoaderEntryDefault")


      def oneshot():
          """The boot of once: `LoaderEntryOneShot`, or GRUB's `next_entry`."""
          return grub_block().get("next_entry") if GRUB else efi("LoaderEntryOneShot")


      def boots(entry_id):
          """The kernel command line the entry boots."""
          if GRUB:
              return grub_menu()[entry_id]
          entries = json.loads(machine.succeed("bootctl list --json=short"))
          return next(e for e in entries if e["id"] == entry_id)["options"]


      def restarts():
          """Whether systemd-boot restarts on a start that fails: the line
          the trial posts in `loader.conf`, and takes away with it."""
          conf = machine.succeed("cat /boot/loader/loader.conf").splitlines()
          return "reboot-on-error yes" in conf


      def boots_so_far():
          """How many boots ran a kernel, by the journal (it is on the disk)."""
          return len(json.loads(machine.succeed("journalctl --list-boots -o json")))


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
          once, back = oneshot(), default()
          assert f"init={BROKEN}/init" in boots(once), once
          assert f"init={good}/init" in boots(back), back
          if not GRUB:
              assert restarts(), "a trial systemd-boot cannot start would go to the firmware"

      with subtest("a bless in the boot that armed the trial judges nothing"):
          machine.succeed("systemctl restart ronoctua-boot-bless.service")
          s = state()
          assert s["trial"]["outcome"] == "pending", s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == BROKEN

      with subtest("the trial is tried once, and the machine comes back on its own"):
          machine.reboot()
          # The one-shot entry booted — its own command line, which GRUB
          # marks as a boot that tries — and the kernel panicked in it.
          marked = f"{MARK} .*" if GRUB else ""
          machine.wait_for_console_text(f"Kernel command line: .*{marked}ronoctua.trial=broken")
          machine.wait_for_console_text("Kernel panic - not syncing")
          machine.wait_for_unit("multi-user.target")
          assert machine.succeed("readlink -f /run/booted-system").strip() == good
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["trial"] == {"toplevel": BROKEN, "outcome": "failed"}, s
          assert s["last-good"]["toplevel"] == good, s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == good
          assert oneshot() is None, "the one-shot was spent"
          assert default() is None, "the default is the menu's own again"
          if not GRUB:
              assert not restarts(), "a verdict takes the restart away with the default"
          if GRUB:
              # The menu's first entry is the newest generation: the last good.
              first = next(iter(grub_menu().values()))
              assert f"init={good}/init" in first, first
          else:
              # systemd-boot's configured default is the newest generation, the
              # last good. It is named by its content hash now
              # (`nixos-<sha256>.conf`), not `nixos-generation-N`, so check what
              # that default entry BOOTS, not what it is called.
              loader = machine.succeed("cat /boot/loader/loader.conf")
              default_line = next(
                  (l for l in loader.splitlines() if l.startswith("default ")), None
              )
              assert default_line, f"loader.conf has no `default`:\n{loader}"
              default_entry = default_line.split(None, 1)[1].strip()
              assert f"init={good}/init" in boots(default_entry), (
                  f"loader.conf's default ({default_entry}) does not boot the "
                  f"last good system:\n{loader}"
              )

      with subtest("the boot after that is the last good one, by the menu alone"):
          machine.reboot()
          machine.wait_for_unit("multi-user.target")
          assert machine.succeed("readlink -f /run/booted-system").strip() == good
          # A boot that tries nothing is not marked as one.
          assert MARK not in machine.succeed("cat /proc/cmdline").split()
          machine.wait_for_unit("ronoctua-boot-bless.service")
          assert state()["trial"]["outcome"] == "failed"

      with subtest("the system that did not boot is not built into the machine again"):
          status, out = apply(BROKEN)
          print(out)
          assert status == 1 and out.strip().endswith("refused did-not-boot"), out
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == good
          assert oneshot() is None

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
          assert default() is None

      with subtest("a trial the loader cannot start falls back with no one at the menu"):
          status, out = apply(UNLOADABLE)
          print(out)
          assert status == 0, out
          assert state()["trial"] == {"toplevel": UNLOADABLE, "outcome": "pending"}
          before = boots_so_far()
          machine.reboot()
          # The loader says it could not start the trial: GRUB falls back to
          # its first entry, systemd-boot restarts the machine.
          # NO `timeout`: with one, the driver reads one line of the console
          # a second, and a reboot prints thousands.
          said = "Falling back to" if GRUB else "Failed to start boot entry. Rebooting"
          machine.wait_for_console_text(said)
          machine.wait_for_unit("multi-user.target")
          assert machine.succeed("readlink -f /run/booted-system").strip() == good
          # One boot reached the journal: the last good one, not as a boot
          # that tries. (The proof the trial's kernel never ran is the
          # loader's line above.)
          assert boots_so_far() == before + 1, (before, boots_so_far())
          assert MARK not in machine.succeed("cat /proc/cmdline").split()
          machine.wait_for_unit("ronoctua-boot-bless.service")
          s = state()
          assert s["trial"] == {"toplevel": UNLOADABLE, "outcome": "failed"}, s
          assert s["last-good"]["toplevel"] == good, s
          assert machine.succeed("readlink -f /nix/var/nix/profiles/system").strip() == good
          assert oneshot() is None and default() is None
          if not GRUB:
              assert not restarts()
    '';
}
