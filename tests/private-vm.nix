# /private, on a machine with a disk of its own.
#
# WHAT ONLY THIS PROVES
# ---------------------
# `ronoctua-private` has its tests against stand-ins: fake `cryptsetup`,
# `losetup`, `mount` and `umount` that record what they are asked. The real
# ones answer here, and so do the things no unit test can see:
#
#   1. the subvolume unit makes `@private` on a disk that never had it (a
#      machine installed before the vault), NOCOW, before the mount needs it;
#      `/private` underneath is root's `0000` and immutable;
#   2. the door (sudo, the helper's `private-root`) starts the root unit, which
#      makes a sparse LUKS2 image (AES-XTS 512, argon2id), opens it with
#      discards let through and mounts the inner btrfs `discard=async` at
#      `/private`, the person's;
#   3. IT SHRINKS: a gigabyte written and deleted inside `/private` goes back
#      to the machine's disk (`fstrim` → a hole punched in the image);
#   4. a wrong passphrase is refused AS that, in the verdict the door reads;
#   5. the sleep hook (wanted by `sleep.target`, started here — the machine is
#      never slept) closes a vault nothing holds, and SUSPENDS one something
#      holds (`luksSuspend`: the key out of memory), the status saying so;
#      the passphrase resumes it;
#   6. a larger cap lengthens the image, still sparse, and the inner btrfs
#      grows into it.
#
# EVERYDAY BATTERY: one machine from three modules, a blank disk image the
# framework formats, no installation, no reboot, no TPM.
{
  pkgs,
  lib,
  securityRoot,
  privateRoot,
}:

let
  user = "noctua";
  # THE TEST'S OWN PASSWORDS — fixtures, never a person's.
  password = "a door, not a window";
  passphrase = "correct horse battery staple";
  helper = "${securityRoot}/bin/ronoctua-security-root";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-private";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../nix/modules/security.nix
        ../nix/modules/private.nix
      ];

      # The modules read `config.ronoctua.user`, which belongs to `base.nix` —
      # the `security-vm` precedent: declared bare here.
      options.ronoctua.user = lib.mkOption { type = lib.types.str; };

      config = {
        ronoctua.user = user;
        ronoctua.security.helper = securityRoot;
        ronoctua.private.helper = privateRoot;

        # argon2id asks for memory: room for it beside the system.
        virtualisation.memorySize = 2048;
        virtualisation.cores = 2;

        # A DISK OF ITS OWN, blank: the framework makes a btrfs on it, and the
        # subvolume unit — the existing machines' path — makes `@private`.
        virtualisation.emptyDiskImages = [ 6144 ];
        virtualisation.fileSystems."/var/lib/ronoctua-private" = {
          device = "/dev/vdb";
          fsType = "btrfs";
          autoFormat = true;
          options = [
            "subvol=@private"
            "noatime"
            "nofail"
            "x-systemd.requires=ronoctua-private-subvolume.service"
          ];
        };
        # The framework's formatting first: on a real machine the disk was
        # made by the installer long before.
        systemd.services.ronoctua-private-subvolume = {
          after = [ "systemd-makefs@dev-vdb.service" ];
          requires = [ "systemd-makefs@dev-vdb.service" ];
        };
        boot.supportedFilesystems = [ "btrfs" ];

        users.mutableUsers = false;
        users.users.${user} = {
          isNormalUser = true;
          group = "users";
          uid = 1000;
          extraGroups = [ "wheel" ];
          password = password;
        };

        environment.systemPackages = [
          pkgs.btrfs-progs
          pkgs.cryptsetup
          pkgs.e2fsprogs
          pkgs.lvm2
          pkgs.jq
        ];
      };
    };

  testScript = ''
    import json
    import re
    import shlex

    user = "${user}"
    password = "${password}"
    passphrase = "${passphrase}"
    helper = "${helper}"
    image = "/var/lib/ronoctua-private/private.img"
    staged = "/run/ronoctua-private/staged"

    # The person, as the footer is: no terminal, stdin from nowhere.
    person = "systemd-run --wait --pipe --quiet --uid=1000 --gid=100 "


    def as_user(command):
        return (
            person
            + "--setenv=PATH=/run/wrappers/bin:/run/current-system/sw/bin "
            + f"-- ${pkgs.bash}/bin/bash -c {shlex.quote(command)} < /dev/null"
        )


    def stage(op, secret=None, new=None):
        # WHAT `ronoctua_private::stage` WRITES: the request and each secret,
        # `0600` from the first byte (`umask 077`), in the person's folder.
        files = {"request.json": json.dumps({"format": 1, "op": op})}
        if secret is not None:
            files["passphrase"] = secret
        if new is not None:
            files["new-passphrase"] = new
        for name, text in files.items():
            machine.succeed(as_user(f"umask 077; printf '%s' {shlex.quote(text)} > {staged}/{name}"))


    def run_unit():
        # What the helper's `private-root` does: start the unit, by its name.
        return machine.execute("systemctl start ronoctua-private-root.service")[0]


    def status():
        return json.loads(machine.succeed("cat /run/ronoctua-private/status.json"))


    def verdict():
        return json.loads(machine.succeed("cat /run/ronoctua-private/verdict.json"))


    def kib(path):
        return int(machine.succeed(f"du -k {path} | cut -f1").strip())


    machine.wait_for_unit("multi-user.target")

    with subtest("@private is made on a disk that never had it, and /private is shut"):
        machine.succeed("systemctl is-active ronoctua-private-subvolume.service")
        machine.succeed("findmnt -n -o OPTIONS /var/lib/ronoctua-private | grep -q 'subvol=/@private'")
        assert "C" in machine.succeed("lsattr -d /var/lib/ronoctua-private").split()[0]
        assert machine.succeed("stat -c %a /private").strip() == "0"
        assert "i" in machine.succeed("lsattr -d /private").split()[0]
        machine.fail(as_user("touch /private/x"))
        machine.fail("touch /private/x")
        machine.fail("findmnt /private")
        # A oneshot that ends: waited for by what it leaves, not by its state.
        machine.wait_until_succeeds("test -e /run/ronoctua-private/status.json")
        assert status()["state"] == "absent", status()
        # NOTHING OPENS IT AT BOOT: no fstab line, no crypttab line.
        machine.fail("grep -q ronoctua-private /etc/crypttab")
        machine.fail("grep -qE '[[:space:]]/private[[:space:]]' /etc/fstab")

    with subtest("the door makes it: sparse LUKS2, AES-XTS 512, argon2id, the person's"):
        stage({"op": "create", "cap-bytes": 3000000000}, passphrase)
        machine.succeed(
            f"printf '%s\\n' {shlex.quote(password)} | "
            + person
            + "-- ${pkgs.coreutils}/bin/env -i LC_ALL=C ${pkgs.util-linux}/bin/setsid --wait "
            + f"/run/wrappers/bin/sudo -S -k -p ''' -- {helper} private-root | grep -qx done"
        )
        assert status()["state"] == "open", status()
        machine.succeed(f"test -z \"$(ls -A {staged})\"")
        dump = machine.succeed(f"cryptsetup luksDump {image}")
        assert re.search(r"Version:\s+2", dump), dump
        for wanted in ("aes-xts-plain64", "512 bits", "argon2id", "allow-discards"):
            assert wanted in dump, (wanted, dump)
        assert "C" in machine.succeed(f"lsattr {image}").split()[0]
        assert int(machine.succeed(f"stat -c %s {image}")) == 3000000000
        assert kib(image) < 200 * 1024, kib(image)
        mounted = machine.succeed("findmnt -n -o SOURCE,FSTYPE,OPTIONS /private")
        assert "/dev/mapper/ronoctua-private" in mounted and "btrfs" in mounted, mounted
        assert "discard=async" in mounted and "nodev" in mounted and "nosuid" in mounted, mounted
        assert machine.succeed("stat -c %U:%a /private").strip() == f"{user}:700"
        machine.succeed("losetup --list --noheadings -O DIO,BACK-FILE | grep private.img | grep -q '^ *1'")

    with subtest("it shrinks: a gigabyte written and deleted goes back to the disk"):
        machine.succeed(as_user("dd if=/dev/urandom of=/private/big bs=1M count=1024 status=none && sync"))
        full = kib(image)
        assert full > 1000 * 1024, full
        machine.succeed(as_user("rm /private/big && sync"))
        machine.succeed("btrfs filesystem sync /private && fstrim /private && sync")
        empty = kib(image)
        assert empty < full - 900 * 1024, (full, empty)

    with subtest("closing takes away the mount, the mapping and the loop"):
        stage({"op": "close"})
        assert run_unit() == 0, verdict()
        assert status()["state"] == "locked", status()
        machine.fail("findmnt /private")
        machine.fail("test -e /dev/mapper/ronoctua-private")
        assert machine.succeed(f"losetup -j {image}").strip() == ""
        assert "i" in machine.succeed("lsattr -d /private").split()[0]

    with subtest("a wrong passphrase is refused as that"):
        stage({"op": "open"}, "not the one")
        assert run_unit() != 0
        assert verdict()["why"] == "wrong-passphrase", verdict()
        assert verdict()["op"] == "open"
        assert status()["state"] == "locked"
        assert machine.succeed(f"losetup -j {image}").strip() == "", "a loop was left"

    with subtest("no key enrolled, no key that answers: said as that"):
        stage({"op": "open"})
        assert run_unit() != 0
        assert verdict()["why"] == "no-key", verdict()

    with subtest("the sleep hook closes a vault nothing holds"):
        machine.succeed("systemctl list-dependencies --plain sleep.target | grep -q ronoctua-private-lock")
        stage({"op": "open"}, passphrase)
        assert run_unit() == 0, verdict()
        machine.succeed("systemctl start ronoctua-private-lock.service")
        assert status()["state"] == "locked", status()
        machine.fail("findmnt /private")

    with subtest("and suspends one something holds; the passphrase resumes it"):
        stage({"op": "open"}, passphrase)
        assert run_unit() == 0, verdict()
        machine.succeed(as_user("echo kept > /private/note"))
        machine.succeed(
            "systemd-run --unit=holder --uid=1000 --gid=100 --working-directory=/private "
            + "${pkgs.coreutils}/bin/sleep 600"
        )
        machine.succeed("systemctl start ronoctua-private-lock.service")
        assert status()["state"] == "suspended-in-use", status()
        assert "SUSPENDED" in machine.succeed("dmsetup info ronoctua-private")
        # A MANUAL CLOSE is refused: something holds it.
        stage({"op": "close"})
        assert run_unit() != 0
        assert verdict()["why"] == "busy", verdict()
        stage({"op": "open"}, passphrase)
        assert run_unit() == 0, verdict()
        assert status()["state"] == "open", status()
        assert machine.succeed("cat /private/note").strip() == "kept"
        machine.succeed("systemctl stop holder.service")

    with subtest("a new passphrase replaces the old"):
        stage({"op": "passphrase"}, passphrase, "a new one, longer")
        assert run_unit() == 0, verdict()
        stage({"op": "close"})
        assert run_unit() == 0, verdict()
        stage({"op": "open"}, passphrase)
        assert run_unit() != 0 and verdict()["why"] == "wrong-passphrase", verdict()
        stage({"op": "open"}, "a new one, longer")
        assert run_unit() == 0, verdict()

    with subtest("a larger cap lengthens the image, still sparse, and the inside grows"):
        size = "btrfs filesystem usage -b /private | awk '/Device size:/ {print $3}'"
        before = int(machine.succeed(size))
        # OPEN, so the mapping grows too: the passphrase lets cryptsetup at it.
        stage({"op": "grow", "cap-bytes": 4000000000}, "a new one, longer")
        assert run_unit() == 0, verdict()
        assert int(machine.succeed(f"stat -c %s {image}")) == 4000000000
        after = int(machine.succeed(size))
        assert after > before, (before, after)
        assert kib(image) < 1000 * 1024, kib(image)

    with subtest("a request staged by somebody else, or readable by others, is refused"):
        close = shlex.quote(json.dumps({"format": 1, "op": {"op": "close"}}))
        machine.succeed(f"umask 077; printf '%s' {close} > {staged}/request.json")
        assert run_unit() != 0
        assert verdict()["why"] == "invalid", verdict()
        stage({"op": "close"})
        machine.succeed(f"chmod 0644 {staged}/request.json")
        assert run_unit() != 0
        assert verdict()["why"] == "invalid", verdict()
        assert status()["state"] == "open", "a refused request changed something"
  '';
}
