# VM test: a bind from /persist that NixOS mounts in the systemd initrd boots.
#
# WHAT THIS PROVES
# ----------------
# That the binds `nix/modules/persist.nix` makes of the paths NixOS mounts
# before it switches root (`/var/lib/nixos`) come up in the initrd, seeded
# there. The first boot of the first real installation (2026-10-23) stopped
# right after the disk's passphrase: the fstab line required
# `ronoctua-persist-seed.service`, which only stage 2 had, and
# `initrd-parse-etc` ("Mountpoints Configured in the Real Root") failed.
#
# Three boots: the first, on an empty `/persist`; the installer's case, where
# the root holds `/var/lib/nixos` and `/persist` holds nothing of it — the
# initrd's seed copies it before the bind; and a third that binds the kept
# copy and seeds nothing.
#
# WHAT THIS DOES NOT PROVE
# ------------------------
# The wipe (`impermanence-vm`) or the home (`persist-home-vm`). And the mounts
# are the VM's own (`virtualisation.fileSystems`), with the module's binds
# carried in by `ronoctua.persist.binds`: which lines the reference machine's
# fstabs hold, and that each unit they require exists where they are mounted,
# is `fstab-requires-what-its-stage-has`.
{
  pkgs,
  lib,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-persist-initrd";

  nodes.machine =
    { config, ... }:
    {
      imports = [ ../nix/modules/persist.nix ];

      # THE DISTRO'S STAGE 1 (`nix/modules/host.nix`), where the bind is read.

      virtualisation = {
        memorySize = 1024;
        # `/persist` ON A DISK OF ITS OWN, up in the initrd as on a machine.
        emptyDiskImages = [ 512 ];
        fileSystems = {
          "/persist" = {
            device = "/dev/vdb";
            fsType = "ext4";
            autoFormat = true;
            neededForBoot = true;
          };
        }
        // config.ronoctua.persist.binds;
      };

      # ONE BIND THE INITRD MOUNTS, AND ONE IT DOES NOT.
      ronoctua.persist = {
        directories = [
          "/var/lib/nixos"
          "/var/lib/ronoctua-persist-probe"
        ];
        files = [ ];
      };
    };

  testScript = ''
    def seeded():
        return machine.succeed("journalctl -b --no-pager -u ronoctua-persist-seed.service")

    machine.start()
    machine.wait_for_unit("multi-user.target", timeout=300)

    with subtest("the first boot binds both, the one the initrd mounts included"):
        machine.succeed("mountpoint -q /var/lib/nixos")
        machine.succeed("mountpoint -q /var/lib/ronoctua-persist-probe")

    with subtest("the installer's case: the root holds /var/lib/nixos, /persist does not"):
        # THE ROOT'S OWN COPY, under the bind: a bind of `/` alone shows it.
        machine.succeed("mkdir -p /mnt/root && mount --bind / /mnt/root")
        machine.succeed("echo installed > /mnt/root/var/lib/nixos/marker")
        machine.succeed("umount /mnt/root")
        machine.succeed("rm -rf /persist/var/lib/nixos")
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target", timeout=300)
        machine.succeed("mountpoint -q /var/lib/nixos")
        machine.succeed("grep -qx installed /persist/var/lib/nixos/marker")
        machine.succeed("grep -qx installed /var/lib/nixos/marker")
        log = seeded()
        assert "/var/lib/nixos seeded into /persist" in log, log
        assert "command not found" not in log, log

    with subtest("a third boot binds the kept copy and seeds nothing"):
        machine.succeed("echo kept > /var/lib/nixos/marker")
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target", timeout=300)
        machine.succeed("grep -qx kept /var/lib/nixos/marker")
        log = seeded()
        assert "seeded into /persist" not in log, log
  '';
}
