# The home is not bound from /persist, and what was is given back — two boots.
#
# # What it proves
#
# `nix/modules/persist.nix` bound three of the person's directories from
# `/persist/home/<user>` over their own place in the home, and `tmpfiles` made
# each of them again as `0755 root root` at every boot: after the FIRST reboot
# of an installed machine no preference of the footer could be written — the
# bars' mode came back visible on every boot, with a line in a log as the only
# word about it. The fix takes the home out of the list (it is `@home`, which
# the wipe never touches) and returns once what `/persist/home` kept
# (`ronoctua-persist-home-return`).
#
# Only a reboot proves either half: the loss happened BETWEEN boots, and the
# return runs at a boot. So one node, one persistent disk for `/persist`, and
# two boots:
#
#   1. the person writes `modeline.json` the way `ronoctua_store::Store` does —
#      a draft beside it, renamed over it — with the bars' mode `cover`, and a
#      legacy of the old arrangement is planted: `/persist/home/<user>` with
#      root's directories and a file in each, and the home's parents made
#      root's the way the bind's mount points made them;
#   2. after the reboot: the preference is what was written, the home's
#      directories and their parents are the person's, nothing is mounted over
#      them, the legacy files are back in the home and the person's, a SECOND
#      write the same way succeeds — the one that failed for ever before — and
#      `/persist/home` was kept under a new name, never deleted.
{
  pkgs,
  lib,
  ronoctua,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-persist-home";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/persist.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation = {
        memorySize = 1024;
        # `/persist` ON A DISK OF ITS OWN, which survives the reboot like the
        # root does and is up before anything that binds from it.
        emptyDiskImages = [ 512 ];
        fileSystems."/persist" = {
          device = "/dev/vdb";
          fsType = "ext4";
          autoFormat = true;
          neededForBoot = true;
        };
      };
      networking.hostName = lib.mkForce "noctua-persist";
      users.users.root.hashedPassword = lib.mkForce null;

      # THE MACHINE'S OWN STATE STILL GOES TO /persist — the list keeps its
      # system entries — and nothing of the home.
      ronoctua.persist.directories = [ "/var/lib/ronoctua-persist-probe" ];

      environment.systemPackages = [ pkgs.util-linux ];
    };

  testScript =
    { nodes, ... }:
    let
      user = nodes.machine.ronoctua.user;
    in
    ''
      import shlex

      user = "${user}"
      home = f"/home/{user}"

      def as_user(command):
          return machine.succeed(f"su - {user} -s /bin/sh -c {shlex.quote(command)}")

      def write_like_the_store(path, text):
          # A DRAFT BESIDE IT, with the PID in its name, renamed over the file:
          # what `ronoctua_store::Store::write` does. It is the RENAME that a
          # root-owned directory refuses.
          as_user(
              f"mkdir -p $(dirname {path}) && "
              f"printf %s {shlex.quote(text)} > {path}.$$.draft && mv {path}.$$.draft {path}"
          )

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("the home is not in the list, and nothing is bound over it"):
          mounts = machine.succeed("findmnt -rn -o TARGET").split()
          assert not any(m.startswith(home) for m in mounts), mounts
          # AND NOTHING TO RETURN on a machine that never had the legacy.
          machine.succeed("systemctl show -P ConditionResult ronoctua-persist-home-return | grep -qx no")

      with subtest("the person writes the bars' mode, the way the footer does"):
          write_like_the_store(f"{home}/.config/ronoctua/modeline.json", '{"hidden":"cover"}')

      with subtest("the legacy of the old arrangement is planted"):
          for d in [".config/ronoctua", ".local/state/ronoctua"]:
              machine.succeed(f"mkdir -p /persist/home/{user}/{d}")
              machine.succeed(f"echo kept > /persist/home/{user}/{d}/legacy.json")
          machine.succeed("chown -R root:root /persist/home")
          # THE PARENTS THE MOUNT POINTS MADE, root's.
          machine.succeed(f"mkdir -p {home}/.local/state && chown root:root {home}/.config {home}/.local {home}/.local/state")

      machine.shutdown()
      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("the return ran, once, and said so"):
          machine.succeed("systemctl is-active ronoctua-persist-home-return")
          print(machine.succeed("journalctl -b -u ronoctua-persist-home-return --no-pager"))

      with subtest("the preference survived the reboot"):
          said = machine.succeed(f"cat {home}/.config/ronoctua/modeline.json").strip()
          assert said == '{"hidden":"cover"}', said

      with subtest("every directory and parent is the person's, and nothing is mounted there"):
          for d in [".config", ".config/ronoctua", ".local", ".local/state", ".local/state/ronoctua"]:
              owner = machine.succeed(f"stat -c %U {home}/{d}").strip()
              assert owner == user, f"{d} is {owner}'s"
          mounts = machine.succeed("findmnt -rn -o TARGET").split()
          assert not any(m.startswith(home) for m in mounts), mounts

      with subtest("what /persist kept is back in the home, and the person's"):
          for d in [".config/ronoctua", ".local/state/ronoctua"]:
              assert machine.succeed(f"cat {home}/{d}/legacy.json").strip() == "kept"
              owner = machine.succeed(f"stat -c %U {home}/{d}/legacy.json").strip()
              assert owner == user, f"{d}/legacy.json is {owner}'s"

      with subtest("a second write succeeds — the one that failed for ever"):
          write_like_the_store(f"{home}/.config/ronoctua/modeline.json", '{"hidden":"push"}')
          write_like_the_store(f"{home}/.local/state/ronoctua/probe.json", "{}")
          said = machine.succeed(f"cat {home}/.config/ronoctua/modeline.json").strip()
          assert said == '{"hidden":"push"}', said

      with subtest("/persist/home was kept under a new name, never deleted"):
          machine.fail("test -e /persist/home")
          kept = machine.succeed("ls -d /persist/home-returned-*").split()
          assert len(kept) == 1, kept
          machine.succeed(f"test -f {kept[0]}/{user}/.config/ronoctua/legacy.json")

      with subtest("a third boot returns nothing again"):
          machine.shutdown()
          machine.start()
          machine.wait_for_unit("multi-user.target")
          machine.succeed("systemctl show -P ConditionResult ronoctua-persist-home-return | grep -qx no")
          assert len(machine.succeed("ls -d /persist/home-returned-*").split()) == 1
    '';
}
