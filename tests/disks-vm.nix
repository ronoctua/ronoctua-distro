# Disks, partitions and pendrives — what only a booted machine answers.
#
# WHAT THIS PROVES, AND WHAT IT CANNOT
# ------------------------------------
# There are two halves to the sidebar's device side, and neither the machine
# nor a VM can prove both.
#
# The REMOVABLE half — `HintSystem = false`, mounting with no password,
# unmounting — needs a device the kernel calls removable, and QEMU does not
# hand one out for free: `emptyDiskImages` gives a virtio disk, which udev
# marks as fixed. That half is proved against a real pendrive, and the numbers
# it produced are the fixture of `ronoctua_disks`'s unit tests.
#
# So this proves the OTHER half, which is the one a real machine will not let
# anybody prove safely: **a system volume, refused, with no authentication
# agent in the session**. Doing that on somebody's laptop means unmounting
# their `/boot` and hoping.
#
# Written down because an absence that does not explain itself reads as an
# oversight, and the next person would spend an afternoon adding the subtest
# that cannot work.
#
# AND WHY A `-vm` AT ALL
# ----------------------
# Three of these are facts about a SYSTEM and not about our code: that udisks
# is D-Bus activatable, that its udev rules arrived, and that upstream's polkit
# defaults are what the whole design rests on. A nixpkgs bump can change any of
# them, and the symptom on a desktop is a stick that does not appear with
# nothing anywhere saying why.
{
  pkgs,
  lib,
  ronoctua,
  # The formatter, extracted on its own (`nix/rust.nix`): it is three D-Bus
  # calls, and the workspace it comes out of carries wgpu and the wasm runtime.
  formatDisk,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-disks";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/disks.nix
      ];
      _module.args.ronoctua = ronoctua;
      virtualisation.memorySize = 2048;
      # A disk of its own to format and offer to udisks. Virtio, so the kernel
      # calls it FIXED — which is the point here: it is the `HintSystem = true`
      # case, and the refusal is what is being measured.
      #
      # AND A SECOND ONE, `vdc`, that plays a STICK for the encrypted subtests
      # (archive-luks). udisks' `update_hints` starts every block at
      # `HintSystem = true` and clears it only for a drive that is removable
      # or on USB, FireWire or a memory card — a virtio disk is none of them,
      # and a container that is the system's is not a row the sidebar opens
      # (`Volume::encrypted`). `UDISKS_SYSTEM=0` is the override udisks reads
      # after all of that; the subtest asserts it arrived before relying on it.
      # 64 MiB: LUKS2's header takes 16 of them.
      virtualisation.emptyDiskImages = [
        64
        64
      ];
      services.udev.extraRules = ''
        SUBSYSTEM=="block", KERNEL=="vdc*", ENV{UDISKS_SYSTEM}="0"
      '';
      networking.hostName = lib.mkForce "noctua-disks";
      # No /persist in this VM — the same reason as the file manager's and the
      # rescue's: `base.nix` points the password hash there and nothing here
      # logs in.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };
      users.users.root.hashedPassword = lib.mkForce null;
      # `cryptsetup` to READ what was written (`luksDump`), never to write it:
      # the writing is udisks', through the same calls the shell makes.
      environment.systemPackages = [
        pkgs.dosfstools
        pkgs.cryptsetup
      ];
    };

  testScript =
    { nodes, ... }:
    ''
      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("the service answers on the bus"):
          # D-BUS ACTIVATABLE, and not "the unit is enabled": udisks starts on
          # the first call and a unit nobody ever calls proves nothing.
          objects = machine.succeed(
              "busctl --system call org.freedesktop.UDisks2 /org/freedesktop/UDisks2 "
              "org.freedesktop.DBus.ObjectManager GetManagedObjects"
          )
          assert "block_devices" in objects, (
              f"udisks2 listed no device at all:\n{objects[:400]}"
          )

      with subtest("upstream's policy is installed, with the defaults the design assumes"):
          # THE WHOLE DESIGN RESTS ON THESE TWO NUMBERS. `filesystem-mount` is
          # `yes` for an active local session, which is why the sidebar mounts a
          # stick with no password; `filesystem-mount-system` is
          # `auth_admin_keep`, which is why it does NOT offer to mount an
          # internal partition. A nixpkgs bump that changes the posture has to
          # fail here and not on somebody's desk.
          policy = machine.succeed(
              "cat /run/current-system/sw/share/polkit-1/actions/"
              "org.freedesktop.UDisks2.policy"
          )
          import re
          def allow_active(action):
              block = re.search(
                  r'<action id="org\.freedesktop\.udisks2\.' + action + r'">(.*?)</action>',
                  policy, re.S,
              )
              assert block, f"the action {action} went away from udisks2's policy"
              found = re.search(r"<allow_active>([^<]+)</allow_active>", block.group(1))
              assert found, f"the action {action} does not declare allow_active"
              return found.group(1).strip()

          assert allow_active("filesystem-mount") == "yes", (
              "mounting a removable device stopped being free for the active "
              "session: the sidebar starts asking for a password for a pen drive"
          )
          assert allow_active("filesystem-mount-system") == "auth_admin_keep", (
              "mounting a system volume stopped asking for the administrator: the "
              "refusal the sidebar anticipates no longer holds"
          )
          # AND FORMATTING SPLITS THE SAME WAY, which is what makes the format
          # buffer possible at all: writing a filesystem onto a stick is free
          # for the active session, and writing one onto an internal disk asks
          # for an administrator this session has no agent to answer with. The
          # buffer's own refusals sit IN FRONT of these two — they are the
          # design, not the enforcement.
          assert allow_active("modify-device") == "yes", (
              "formatting a removable device started asking for a password: the format "
              "buffer has no agent at all to answer"
          )
          assert allow_active("modify-device-system") == "auth_admin_keep", (
              "formatting a system disk stopped asking for the administrator"
          )

      with subtest("and no polkit rule of ours speaks of udisks"):
          # The inverse of `rescue-polkit-is-narrow`, and read from the BUILT
          # system rather than from the source: upstream's rule already splits
          # on `HintSystem`, and ours could only widen it.
          rules = machine.succeed(
              "grep -RIl udisks /etc/polkit-1/rules.d/ 2>/dev/null || true"
          ).strip()
          assert not rules, f"somebody wrote a polkit rule for udisks: {rules}"

      with subtest("the udev rules arrived, and an attached disk has the hints"):
          # THIS IS WHAT `services.udev.packages` BUYS, and it can silently not
          # happen. Without the hints every property the sidebar reads comes
          # back empty and every disk looks the same.
          # A PARTITION AND NOT THE WHOLE DISK. udisks does not offer a
          # filesystem written straight onto a disk with no partition table as
          # mountable — it answers "not a mountable filesystem" and never adds
          # the `Filesystem` interface, which is a different refusal wearing the
          # same clothes as the one being measured below. A partition is also
          # what a real stick has.
          machine.succeed("sfdisk /dev/vdb <<< 'label: dos\n,,c'")
          machine.succeed("udevadm settle")
          machine.succeed("mkfs.vfat -n PROVA /dev/vdb1")
          machine.succeed("udevadm settle")
          info = machine.succeed("udisksctl info -b /dev/vdb1")
          assert "IdLabel:" in info and "PROVA" in info, (
              f"udisks did not read the label of /dev/vdb1:\n{info}"
          )
          # AND THE INTERFACE IS THERE, which is what makes the next subtest
          # measure polkit and not a race with udisks' re-probe.
          assert "MountPoints:" in info, (
              f"udisks did not give the Filesystem interface to /dev/vdb1; the next "
              f"subtest would measure the wrong thing:\n{info}"
          )
          assert "HintSystem:" in info, (
              f"udisks did not publish HintSystem, which is the property "
              f"the whole drawing depends on:\n{info}"
          )

      with subtest("with no active session, mounting is refused by name — and does NOT hang"):
          # WHAT THIS MEASURES, AND WHAT IT DOES NOT.
          #
          # It does not measure the `HintSystem` split: a blank secondary disk in
          # a VM comes out NOT system, and the test driver runs as root, who
          # passes every polkit check. The split itself is measured above, by
          # reading the two `allow_active` values out of the installed policy.
          #
          # What it measures is the other property the whole design leans on:
          # `allow_active` only applies to an ACTIVE local session, and there is
          # no authentication agent anywhere in this distro's graphical session.
          # A caller without one gets `allow_any`, which is `auth_admin` — and
          # WITHOUT `auth.no_user_interaction` polkit would wait for an agent
          # that never registers. On screen that is a row pressed that does
          # nothing, for ever.
          #
          # So the `timeout 20` is half the assertion: it has to come BACK, and
          # it has to come back saying which no it was.
          user = "${nodes.machine.ronoctua.user}"
          machine.succeed(f"loginctl show-user {user} > /dev/null 2>&1 || true")
          refusal = machine.fail(
              f"timeout 20 systemd-run --pipe --wait --uid={user} "
              "--setenv=XDG_RUNTIME_DIR=/run/user/1000 "
              "udisksctl mount --no-user-interaction -b /dev/vdb1 2>&1"
          )
          lowered = refusal.lower()
          # ONE ASSERTION COVERS BOTH, and the first draft used two — the
          # second one matched the word "terminated" out of systemd-run's own
          # summary and failed on a run that had answered in 69 ms.
          #
          # A hang would have been killed by `timeout` above, which prints
          # nothing and carries no error name. So a refusal that NAMES itself
          # is proof that the call came back: `auth.no_user_interaction`
          # reached polkit, and polkit answered instead of waiting for an
          # agent that never registers.
          assert "not authorized" in lowered or "notauthorized" in lowered, (
              f"expected polkit's NAMED refusal — with no name, either the call "
              f"it locked up and the timeout killed it, or it never got there:\n{refusal}"
          )

      with subtest("formatting really writes, and the label comes back by the same door"):
          # WHAT THIS PROVES THAT NO UNIT TEST CAN. `ronoctua_disks::format`
          # is three D-Bus calls with a race between them: `Block.Format`
          # writes a partition table, and `PartitionTable` appears on the
          # object through a UEVENT that has not happened yet. A unit test
          # would mock the bus and prove the argument order; this proves that
          # the interface ARRIVES, which is the half that was guessed.
          #
          # The whole disk, so the `whole` path is the one exercised — the
          # partition path is a single call and is the easy one.
          machine.succeed(
              "${formatDisk}/bin/ronoctua-format-disk /dev/vdb vfat PROVAFMT"
          )
          machine.succeed("udevadm settle")
          info = machine.succeed("udisksctl info -b /dev/vdb1")
          assert "PROVAFMT" in info, (
              f"formatting did not write the label asked for on /dev/vdb1:\n{info}"
          )
          # AND THE PARTITION TYPE FOLLOWED THE FILESYSTEM. Without
          # `update-partition-type` an MBR partition holding vfat is typed
          # `0x83`, Linux — the filesystem is right, the byte beside it is
          # wrong, and Windows does not offer to open the stick with nothing
          # anywhere saying why.
          kind = machine.succeed("sfdisk --part-type /dev/vdb 1").strip().lower()
          assert kind in ("b", "c", "0b", "0c"), (
              f"the partition came out with type {kind!r}, and not one of the FAT ones: "
              "`update-partition-type` did not reach udisks"
          )

      # ── archive-luks: AN ENCRYPTED STICK, end to end ──
      #
      # WHAT THIS PROVES THAT NO UNIT TEST CAN: that udisks writes LUKS2 when
      # `encrypt.passphrase` and `encrypt.type` ride the format call; that its
      # answer to a wrong passphrase is read as one (udisks names it no
      # differently from any other failure — `ronoctua_disks::wrong_passphrase`
      # reads its words); that the container's row gives its place to what came
      # out of it while it is open; and that ejecting locks it again. The
      # passphrase travels on stdin to the helper and in the bus call to udisks
      # — never in an argument, which this script could otherwise read back.
      fd = "${formatDisk}/bin/ronoctua-format-disk"
      phrase = "cofre secreto 42"

      with subtest("the stick's disk is not the system's, or nothing below measures a stick"):
          # THE PRECONDITION, by its own name: without it every subtest below
          # fails as "not a locked row" and the cause is three screens up.
          info = machine.succeed("udisksctl info -b /dev/vdc")
          assert re.search(r"HintSystem:\s+false", info), (
              f"the udev rule did not reach udisks: /dev/vdc is a system disk, and "
              f"its container would be the system's own:\n{info}"
          )

      with subtest("upstream's policy lets the active session unlock a stick, and not the system's"):
          assert allow_active("encrypted-unlock") == "yes", (
              "unlocking a removable disk stopped being free for the active session"
          )
          assert allow_active("encrypted-unlock-system") == "auth_admin_keep", (
              "unlocking a system disk stopped asking for the administrator"
          )

      with subtest("formatting with a passphrase writes LUKS2, and the row is locked"):
          machine.succeed(f"printf '%s\\n' '{phrase}' | {fd} /dev/vdc ext4 COFRE --encrypt")
          machine.succeed("udevadm settle")
          dump = machine.succeed("cryptsetup luksDump /dev/vdc1")
          version = re.search(r"^Version:\s*(\d+)", dump, re.M)
          assert version and version.group(1) == "2", f"not LUKS2:\n{dump[:400]}"
          # WHETHER udisks LEFT IT OPEN after writing the filesystem inside is
          # udisks' business; for what follows it is locked.
          rows = machine.succeed(f"{fd} rows")
          if "/dev/vdc1 locked" not in rows:
              machine.succeed(f"{fd} lock /dev/vdc1")
              machine.succeed("udevadm settle")
              rows = machine.succeed(f"{fd} rows")
          assert "/dev/vdc1 locked" in rows, f"the container is not a locked row:\n{rows}"

      with subtest("a wrong passphrase is refused BY NAME, and nothing opens"):
          said = machine.fail(f"printf '%s\\n' 'not the one' | {fd} unlock /dev/vdc1 2>&1")
          assert "WrongPassphrase" in said, (
              f"udisks' refusal of a wrong passphrase was not read as one:\n{said}"
          )
          assert "/dev/vdc1 locked" in machine.succeed(f"{fd} rows")

      with subtest("unlocked, what came out takes the row; mounted; ejected locks it again"):
          clear = machine.succeed(f"printf '%s\\n' '{phrase}' | {fd} unlock /dev/vdc1").strip()
          assert clear.startswith("/dev/dm-"), f"the cleartext is {clear!r}"
          machine.succeed("udevadm settle")
          rows = machine.succeed(f"{fd} rows")
          assert f"{clear} unlocked" in rows, f"what came out is not an open row:\n{rows}"
          assert "/dev/vdc1 " not in rows, f"the open container is still a row:\n{rows}"
          at = machine.succeed(f"{fd} mount {clear}").strip()
          machine.succeed(f"echo inside > '{at}/note.txt'")
          # EJECT: unmount, lock — and the power, which a virtio disk cannot
          # cut, is left alone rather than refused (`Drive.CanPowerOff`).
          machine.succeed(f"{fd} eject {clear}")
          machine.succeed("udevadm settle")
          machine.fail(f"test -e {clear}")
          rows = machine.succeed(f"{fd} rows")
          assert "/dev/vdc1 locked" in rows, f"ejecting did not lock it:\n{rows}"
          # AND WHAT WAS WRITTEN IS THERE, behind the passphrase.
          clear = machine.succeed(f"printf '%s\\n' '{phrase}' | {fd} unlock /dev/vdc1").strip()
          at = machine.succeed(f"{fd} mount {clear}").strip()
          assert machine.succeed(f"cat '{at}/note.txt'").strip() == "inside"
          machine.succeed(f"{fd} lock {clear}")

      print("OK: udisks answers, the policy is upstream's, the hints arrive, "
            "whoever has no active session is refused by name without hanging, "
            "formatting writes the label and the partition type, and an encrypted "
            "stick is written as LUKS2, refuses a wrong passphrase by name, opens, "
            "mounts, and is locked again by ejecting")
    '';
}
