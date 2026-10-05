# `~/.cache` becomes a btrfs subvolume of its own — at boot, as root, on a
# real btrfs home, across real reboots.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The conversion (`nix/modules/filesystem.nix`) was proved with a stand-in
# `systemctl` and planted trees; what only a booting machine shows
# (`docs/PENDING.md`, "A folga do disco"):
#
#   - a home with no `~/.cache` gets a subvolume at the first boot, the
#     person's, `0700`;
#   - a plain `~/.cache` full of the person's files is converted at the next
#     BOOT — the machine says `starting` while the unit runs, before any login —
#     with every file and its owner kept, and no note left behind;
#   - the same unit run on a machine that is up converts nothing: it waits for
#     the next boot;
#   - an interrupted conversion is put right only by root, at the next boot:
#     a half copy beside the old directory is undone, and the conversion runs
#     again; an old directory left beside the new subvolume is removed;
#   - and a person's login gets cargo's intermediates under that cache
#     (`CARGO_BUILD_BUILD_DIR`, `nix/cargo-dirs.nix`) and never a
#     `CARGO_TARGET_DIR` of the session's (`cargo-writes-outside-the-tree`).
#
# SLOW (`tests/slow.nix`): three reboots of a machine with a disk of its own.
{
  pkgs,
  lib,
  cargoDirs,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-cache-subvolume";

  nodes.machine = {
    imports = [
      ../nix/modules/filesystem.nix
      ../nix/modules/cache.nix
      # THE BRAIN'S SWITCHES, which `cache.nix` sets the default of: declared
      # here bare, so the module evaluates without the brain it speaks to.
      {
        options.ronoctua.brain.modules = lib.mkOption {
          type = lib.types.anything;
          default = { };
        };
      }
    ];
    _module.args.ronoctua = {
      inherit cargoDirs;
    };
    virtualisation.memorySize = 1536;
    virtualisation.emptyDiskImages = [ 1024 ];
    virtualisation.fileSystems."/home" = {
      device = "/dev/vdb";
      fsType = "btrfs";
      autoFormat = true;
    };
    boot.supportedFilesystems = [ "btrfs" ];
    environment.systemPackages = [ pkgs.btrfs-progs ];
    users.users.alice.isNormalUser = true;
  };

  testScript = ''
    cache = "/home/alice/.cache"
    moving = cache + ".subvolume-in-progress"
    unit = "ronoctua-cache-subvolume-alice.service"

    def is_subvolume(path):
        return machine.succeed(f"stat --format=%i {path}").strip() == "256"

    def boot_again():
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target")

    def plain_cache():
        """A plain directory of the person's, with files, where the subvolume was."""
        machine.succeed(f"btrfs subvolume delete {cache} || rm -rf {cache}")
        machine.succeed(f"mkdir -p {cache}/app && echo mine > {cache}/app/kept && chown -R alice:users {cache}")
        assert not is_subvolume(cache)

    machine.wait_for_unit("multi-user.target")

    with subtest("a home with no cache gets a subvolume at the first boot, the person's"):
        # THE HOME IS ON THE DISK BEFORE THE BOOT, as an installed machine's
        # is. Here the disk is formatted empty while booting and mounted over
        # `/home` after the activation made `/home/alice` on the root, so the
        # first boot finds no home ("nothing to do"); it is made the way
        # NixOS makes it, and the boot that counts is the next one.
        machine.succeed("install -d -m 0700 -o alice -g users /home/alice")
        boot_again()
        machine.wait_for_unit(unit)
        assert is_subvolume(cache), "the first boot made no subvolume"
        assert machine.succeed(f"stat --format='%U %a' {cache}").strip() == "alice 700"

    with subtest("a machine that is up converts nothing: it waits for the next boot"):
        plain_cache()
        # WHAT THIS RUN SAID, by its invocation: the unit's last journal line
        # is systemd's own "Finished", not the script's.
        machine.succeed(f"systemctl restart {unit}")
        said = machine.wait_until_succeeds(
            f"journalctl _SYSTEMD_INVOCATION_ID=$(systemctl show -p InvocationID --value {unit}) "
            "--no-pager -o cat | grep .",
            timeout=30,
        )
        assert "next boot" in said, said
        assert not is_subvolume(cache), "a cache was converted under a running machine"

    with subtest("at the next boot the plain cache is converted, files and owners kept"):
        boot_again()
        machine.wait_for_unit(unit)
        assert is_subvolume(cache), "the boot did not convert the cache"
        assert machine.succeed(f"cat {cache}/app/kept").strip() == "mine"
        assert machine.succeed(f"stat --format=%U {cache}/app/kept").strip() == "alice"
        machine.fail("test -e /var/lib/ronoctua-cache-subvolume/alice")
        machine.fail(f"test -e {moving}")

    with subtest("a half copy beside the old directory is undone by root at boot, and converted again"):
        plain_cache()
        machine.succeed(f"btrfs subvolume create {moving} && echo half > {moving}/half")
        boot_again()
        machine.wait_for_unit(unit)
        said = machine.succeed(f"journalctl -b -u {unit} --no-pager -o cat")
        print(said)
        assert "undid an interrupted conversion" in said, said
        assert is_subvolume(cache) and machine.succeed(f"cat {cache}/app/kept").strip() == "mine"
        machine.fail(f"test -e {cache}/half")
        machine.fail(f"test -e {moving}")

    with subtest("an old directory left beside the new subvolume is removed by root at boot"):
        machine.succeed(f"mkdir -p {moving} && echo old > {moving}/left && chown -R alice:users {moving}")
        boot_again()
        machine.wait_for_unit(unit)
        said = machine.succeed(f"journalctl -b -u {unit} --no-pager -o cat")
        assert "finished an interrupted conversion" in said, said
        machine.fail(f"test -e {moving}")
        assert is_subvolume(cache)

    with subtest("a login's cargo builds go under that cache, and the session sets no target"):
        env = machine.succeed("su - alice -c env")
        build = [l for l in env.splitlines() if l.startswith("CARGO_BUILD_BUILD_DIR=")]
        assert build, "the session does not send cargo's intermediates to the cache"
        assert build[0].startswith("CARGO_BUILD_BUILD_DIR=/home/alice/.cache/${cargoDirs.parent}/${cargoDirs.compiler}/build/"), build
        assert not any(l.startswith("CARGO_TARGET_DIR=") for l in env.splitlines()), (
            "the session sets CARGO_TARGET_DIR, overriding every project's own"
        )
  '';
}
