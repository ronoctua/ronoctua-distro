# The store looks after itself — on a machine, not in a fixture.
#
# WHAT ONLY THIS PROVES
# ---------------------
# `nix/modules/store.nix` was proved with the real `nix-env` on planted
# profiles in a build sandbox and by reading the evaluated units; what no
# sandbox can show (`docs/PENDING.md`, "A folga do disco"):
#
#   - THE FLOOR AS NIX READS IT: the unit measures the filesystem under
#     `/nix/store` at boot, and the `min-free`/`max-free` Nix itself reports
#     are the catalogue's percentages of what `stat -f` says, capped — read by
#     a PERSON's `nix`, which is what skips an `!include` it cannot read. A
#     floor left `0600`, or gone, is no floor for them, and the unit puts it
#     back readable.
#   - THE TRIM ON PLANTED GENERATIONS, as the program runs on the machine:
#     the newest N, the running one, the booted one through its
#     SPECIALISATION, the one the last switch left, and one a boot menu lists
#     stay; the rest goes — rehearsed first, which removes nothing; and a
#     person's profile is never the trim's, whatever path it is handed, and
#     what her generations hold (paths added at run time, rooted by nothing
#     else) outlives a collection that takes a path beside them.
#   - A REAL SWITCH: to a specialisation and back, which writes what it went
#     away from before `/run/current-system` moves, runs the trim at the
#     activation without failing the switch, and a rollback that the trim
#     kept.
{
  pkgs,
  lib,
}:

let
  catalogue = import ../assets/cache.nix;
  # PLANTED SYSTEMS: small store paths shaped as a system is where the trim
  # looks — a `specialisation/` folder — one of them with a specialisation of
  # its own, which is what is BOOTED in the trim's subtest.
  fake =
    n:
    pkgs.runCommand "ronoctua-fake-system-${toString n}" { } ''
      mkdir -p $out/specialisation
      echo ${toString n} > $out/generation
    '';
  special = pkgs.runCommand "ronoctua-fake-system-2-special" { } ''
    mkdir -p $out
    echo special > $out/generation
  '';
  second = pkgs.runCommand "ronoctua-fake-system-2" { } ''
    mkdir -p $out/specialisation
    echo 2 > $out/generation
    ln -s ${special} $out/specialisation/quiet
  '';
  systems = [
    (fake 1)
    second
  ]
  ++ map fake (lib.range 3 8);
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-store";

  nodes.machine = {
    imports = [ ../nix/modules/store.nix ];
    virtualisation.memorySize = 1536;
    virtualisation.useBootLoader = false;
    users.users.alice.isNormalUser = true;
    nix.settings.experimental-features = [ "nix-command" ];
    # A SPECIALISATION TO SWITCH TO, and back.
    specialisation.other.configuration.environment.etc."ronoctua-other".text = "other";
    system.extraDependencies = systems ++ [ special ];
  };

  testScript = ''
    import re

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("ronoctua-store-floor.service")

    def nix_says(user, name):
        out = machine.succeed(f"su - {user} -c 'nix config show {name}' 2>/dev/null || su - {user} -c 'nix show-config' | grep '^{name} '")
        return int(out.strip().split()[-1])

    with subtest("the floor is the catalogue's share of the store's disk, as a person's nix reads it"):
        block, blocks = map(int, machine.succeed("stat --file-system --format='%S %b' /nix/store").split())
        total = block * blocks
        gib = 1024 ** 3
        want_min = min(total * ${toString catalogue.diskFloorPercent} // 100, ${toString catalogue.diskFloorCapGiB} * gib)
        want_max = min(total * ${toString catalogue.diskGoalPercent} // 100, ${toString catalogue.diskGoalCapGiB} * gib)
        mode = machine.succeed("stat --format=%a /run/ronoctua-store/free.conf").strip()
        assert mode == "644", f"the floor is {mode}: a person's nix skips an !include it cannot read"
        assert nix_says("alice", "min-free") == want_min, (nix_says("alice", "min-free"), want_min)
        assert nix_says("alice", "max-free") == want_max, (nix_says("alice", "max-free"), want_max)

    with subtest("a floor left 0600, or gone, is no floor — and the unit puts it back"):
        machine.succeed("chmod 600 /run/ronoctua-store/free.conf")
        assert nix_says("alice", "min-free") == 0, "a floor alice cannot read still counted for her"
        machine.succeed("systemctl restart ronoctua-store-floor.service")
        assert nix_says("alice", "min-free") > 0, "the unit did not write the floor readable again"
        machine.succeed("rm /run/ronoctua-store/free.conf")
        assert nix_says("alice", "min-free") == 0, "a floor that is gone still counted"
        machine.succeed("systemctl restart ronoctua-store-floor.service")
        assert nix_says("alice", "min-free") > 0, "the unit did not put the floor back"

    # THE PLANTED PROFILE: eight generations, the trim keeping the newest
    # three (`--keep 3`); the running one is 1, the booted one is 2's
    # specialisation, the last switch left 3, and a boot menu lists 4. So 5
    # goes, and nothing else.
    gens = ${builtins.toJSON (map toString systems)}
    profile = "/tmp/planted/system"

    with subtest("planted generations: the trim keeps what it must, rehearsed first"):
        machine.succeed("mkdir -p /tmp/planted /tmp/esp/loader/entries /tmp/previous")
        for path in gens:
            machine.succeed(f"nix-env --profile {profile} --set {path}")
        links = lambda: sorted(int(n) for n in re.findall(r"system-(\d+)-link", machine.succeed("ls /tmp/planted")))
        assert links() == list(range(1, 9)), links()
        machine.succeed("printf 'linux /nixos-generation-4/kernel\\n' > /tmp/esp/loader/entries/nixos-generation-4.conf")
        machine.succeed(f"ln -sfn {gens[2]} /tmp/previous/previous-system")
        program = machine.succeed("systemctl cat ronoctua-system-generations.service | grep -o '/nix/store/[^ ]*/bin/ronoctua-system-generations' | head -1").strip()
        args = (f"--profile {profile} --keep 3 --running {gens[0]} "
                f"--booted {gens[1]}/specialisation/quiet --previous /tmp/previous/previous-system --menu /tmp/esp")
        said = machine.succeed(f"{program} {args}")
        print(said)
        assert "would remove system generations 5" in said and "rehearsal" in said, said
        assert links() == list(range(1, 9)), f"the rehearsal removed something: {links()}"
        said = machine.succeed(f"{program} {args} --execute")
        print(said)
        assert links() == [1, 2, 3, 4, 6, 7, 8], f"the trim kept {links()}"

    with subtest("a person's profile is never the trim's"):
        # HER GENERATIONS HOLD PATHS ADDED NOW, which nothing else roots: the
        # planted systems are in this machine's closure
        # (`system.extraDependencies`), and a collection would keep them
        # whatever her profile did. One more path is added and held by
        # nothing — garbage, which the collection below must take.
        def added(name):
            return machine.succeed(
                f"head -c 65536 /dev/urandom > /tmp/{name} && nix-store --add /tmp/{name} && rm /tmp/{name}"
            ).strip()
        mine = [added(f"alice-{n}") for n in range(1, 5)]
        garbage = added("nobody")
        machine.succeed("mkdir -p /nix/var/nix/profiles/per-user/alice")
        for path in mine:
            machine.succeed(f"nix-env --profile /nix/var/nix/profiles/per-user/alice/profile --set {path}")
        dead = set(machine.succeed("nix-store --gc --print-dead 2>/dev/null").split())
        assert garbage in dead, "the unrooted path is not garbage: the collection below proves nothing"
        assert not dead & set(mine), f"her generations do not root what they hold: {dead & set(mine)}"
        program = machine.succeed("systemctl cat ronoctua-system-generations.service | grep -o '/nix/store/[^ ]*/bin/ronoctua-system-generations' | head -1").strip()
        status, said = machine.execute(f"{program} --profile /nix/var/nix/profiles/per-user/alice/profile --keep 2 --execute")
        assert status == 2, f"the trim took a person's profile: {status} {said}"
        status, said = machine.execute(f"{program} --profile /nix/var/nix/profiles/per-user/alice/system --keep 2 --execute")
        assert status == 2, f"a person's profile named `system` was taken: {status} {said}"
        kept = re.findall(r"profile-(\d+)-link", machine.succeed("ls /nix/var/nix/profiles/per-user/alice"))
        assert sorted(map(int, kept)) == [1, 2, 3, 4], kept
        # AND THE STORE'S OWN COLLECTION frees garbage, never what a person's
        # generation holds — after the trim refused her profile twice.
        machine.succeed("nix-store --gc")
        machine.fail(f"test -e {garbage}")
        for path in mine:
            machine.succeed(f"test -e {path}")

    with subtest("a switch writes what it left, trims at the activation, and does not fail"):
        before = machine.succeed("readlink -f /run/current-system").strip()
        machine.succeed("/run/current-system/specialisation/other/bin/switch-to-configuration test")
        left = machine.succeed("readlink -f /run/ronoctua-generations/previous-system").strip()
        assert left == before, f"the switch did not write what it left: {left} != {before}"
        assert machine.succeed("cat /etc/ronoctua-other").strip() == "other"
        machine.succeed(f"{before}/bin/switch-to-configuration test")
        result = machine.succeed(
            "systemctl show -p Result --value ronoctua-system-generations-at-activation.service"
        ).strip()
        assert result == "success", f"the trim at the activation ended {result}"

    with subtest("a rollback reaches the generation the trim kept"):
        # The planted profile again: back from 8 to 7, which the count kept.
        machine.succeed(f"nix-env --profile {profile} --rollback")
        assert machine.succeed(f"readlink {profile}").strip() == "system-7-link"
  '';
}
