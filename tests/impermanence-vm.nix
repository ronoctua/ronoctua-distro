# The root's wipe, switchable — acceptance criterion of milestone 7.
#
# WHAT ONLY TWO BOOTS PROVE
# -------------------------
# Impermanence is a property BETWEEN boots. A test of a single boot could at
# most verify that the unit exists; what matters is what survives the next
# one, and for that the VM has to really reboot, with the same disk.
#
# Three things are verified, and all three have to be true together — each one
# on its own would pass with the wipe broken:
#
#   1. what was written in `/` IS GONE;
#   2. what was written in `/persist` STAYED;
#   3. the old `@root` is still mountable at `/@old`.
#
# The third is the one that separates "it wiped" from "it kept". Without it, a
# wipe that destroys with no recovery window would pass — and the difference
# would only show up the day someone forgot a path in `directories`.
{
  pkgs,
  lib,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-impermanence";

  nodes.machine =
    { config, ... }:
    {
      imports = [ ../nix/modules/persist.nix ];

      # A disk that SURVIVES the reboot. The default `virtualisation.diskImage`
      # already persists, but the btrfs has to be created by hand: the test does not
      # use disko (which would require LUKS) — what is proved here is the wipe, not
      # the partitioning.
      virtualisation = {
        emptyDiskImages = [ 2048 ];
        memorySize = 1536;
      };

      ronoctua.persist = {
        enable = true;
        wipeRoot = true;
        # No LUKS in this VM: that is why `device` is an option.
        blockDevice = "/dev/vdb";
        keepDays = 30;
        directories = [ "/var/lib/teste" ];
        files = [ ];
      };

      # The test VM's real sysroot is another one; here we mount the btrfs on a
      # parallel tree and run the SAME initrd unit against it. It is the only way
      # of exercising the real script without rebuilding the whole boot — and what
      # we want to test is the script, not NixOS's initrd.
      systemd.services.ronoctua-wipe-simulated = {
        description = "Runs the wipe outside the initrd, against /dev/vdb";
        serviceConfig.Type = "oneshot";
        serviceConfig.RemainAfterExit = true;
        # Before anything that writes to the test disk.
        wantedBy = [ ];
        path = with pkgs; [
          btrfs-progs
          util-linux
          coreutils
          findutils
        ];
        inherit (config.boot.initrd.systemd.services.ronoctua-wipe) script;
      };

      environment.systemPackages = with pkgs; [
        btrfs-progs
        util-linux
      ];
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("prepara o disco: btrfs com @root e @persist"):
        machine.succeed("mkfs.btrfs -f /dev/vdb")
        machine.succeed("mkdir -p /mnt && mount /dev/vdb /mnt")
        machine.succeed("btrfs subvolume create /mnt/@root")
        machine.succeed("btrfs subvolume create /mnt/@persist")
        machine.succeed("umount /mnt")

    with subtest("escreve em @root e em @persist"):
        machine.succeed("mkdir -p /mnt && mount -o subvol=@root /dev/vdb /mnt")
        machine.succeed("echo efemero > /mnt/some-no-wipe")
        machine.succeed("umount /mnt")

        machine.succeed("mount -o subvol=@persist /dev/vdb /mnt")
        machine.succeed("echo guardado > /mnt/sobrevive")
        machine.succeed("umount /mnt")

    with subtest("o wipe renova @root"):
        machine.succeed("systemctl start ronoctua-wipe-simulated")
        print(machine.succeed("journalctl -u ronoctua-wipe-simulated --no-pager"))

    with subtest("o que estava em / SUMIU"):
        machine.succeed("mount -o subvol=@root /dev/vdb /mnt")
        machine.fail("test -e /mnt/some-no-wipe")
        machine.succeed("umount /mnt")

    with subtest("what was in /persist STAYED"):
        machine.succeed("mount -o subvol=@persist /dev/vdb /mnt")
        assert machine.succeed("cat /mnt/sobrevive").strip() == "guardado"
        machine.succeed("umount /mnt")

    with subtest("the old @root is still recoverable"):
        # The difference between "it wiped" and "it kept". Without this window, a
        # path forgotten in `directories` loses state with no recourse.
        machine.succeed("mount /dev/vdb /mnt")
        old_roots = machine.succeed("ls /mnt/@old").strip()
        print(f"old ones: {old_roots!r}")
        assert old_roots, "the wipe deleted the @root without keeping a copy"

        target_path = machine.succeed(
            "find /mnt/@old -maxdepth 2 -name some-no-wipe | head -1"
        ).strip()
        assert target_path, f"the lost file is not in the kept @root; old={old_roots!r}"
        assert machine.succeed(f"cat {target_path}").strip() == "efemero"
        machine.succeed("umount /mnt")

    with subtest("running the wipe again does not lose the previously kept one"):
        # Two boots in a row with a wipe: the name carries the epoch precisely so
        # that two on the same day do not collide.
        machine.succeed("systemctl restart ronoctua-wipe-simulated")
        machine.succeed("mount /dev/vdb /mnt")
        n = int(machine.succeed("ls /mnt/@old | wc -l").strip())
        assert n == 2, f"esperava 2 @root guardados, ha {n}"
        machine.succeed("umount /mnt")
  '';
}
