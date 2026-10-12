# A REHEARSAL OF THE UPDATE CHANGES NOTHING — the design's
# `update-rehearsal-changes-nothing`, run by the update's brain module
# (its `checks.nix` hands in every name it owns: the runners, the root half's
# rehearsal unit, the steps found by what they do), and kept HERE, with the
# other machines of a test, because it plants a machine directory: the
# person's declarations in it are written by `ronoctua-person` alone on a
# real machine (`one-person-boundary`, which sweeps the code of the
# installed system and not its tests).
#
# On a booted machine, with the machine's own runners and the real
# programs: the person's half run as the person, starting the root half's
# rehearsal through polkit with no agent, over a machine flake of its own
# (offline: its distro is a flake on the machine), the person's staged
# declarations, their mirror, the reconciler's ledger and the machine's
# record. Every byte of the machine directory, the record's folder, the
# staging folder and the person's state is the same afterwards; the
# verification and the apply REHEARSED (they ran, changing nothing), the
# machine's record was compared and not copied, and no step says it ran.
{
  pkgs,
  lib,
  # The check's name, the module's to give.
  name,
  # The person, and the machine directory by its one name.
  user,
  machineDir,
  # The machine's own runners (`ronoctua.update.package`): the real
  # programs, by store path.
  runners,
  # The root half's rehearsal unit, as the module's manifest names it.
  rehearsalUnit,
  # The ids of the verification, the apply and the record — found by the
  # module by what they DO.
  steps,
  # The machine flake's input that is the distro (the template's one).
  distroInput,
  # `crates/ronoctua-person/fixtures/good`.
  fixtures,
}:

let
  # THE DISTRO, A FLAKE ON THE MACHINE: one derivation for a system.
  distro = pkgs.writeTextDir "flake.nix" ''
    {
      outputs = { self }: {
        lib.system = builtins.derivation {
          name = "fixture-system";
          system = "x86_64-linux";
          builder = "/bin/sh";
          args = [ "-c" "echo a system of a test > $out" ];
        };
      };
    }
  '';
  # THE MACHINE FLAKE: the distro as its one input, the attribute the
  # machine builds, and one declared entry with its fingerprint — a plain
  # token, as `nix/modules/person.nix` makes them (a sha256 in hex): the
  # verification refuses the whole baseline over one that is not.
  machineFlake = pkgs.writeText "flake.nix" ''
    {
      inputs.${distroInput}.url = "path:/var/lib/fixture-distro";
      outputs = { self, ${distroInput} }: {
        nixosConfigurations.${user}.config.system.build.toplevel = ${distroInput}.lib.system;
        personEntries."nix:ripgrep" = ${distroInput}.lib.system;
        personFingerprints."nix:ripgrep" = "a-fingerprint-of-a-test";
      };
    }
  '';
  record = pkgs.writeText "state.json" ''
    {"format": 1, "last-good": null, "trial": null, "hold": null, "last-apply": null}
  '';
in
pkgs.testers.runNixOSTest {
  inherit name;
  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.diskSize = 4096;
    # THE STORE'S ROOM IS THE DISK'S, so the rebuild's room is measured
    # on a disk and not on a tmpfs of memory.
    virtualisation.writableStoreUseTmpfs = false;
    nix.settings.experimental-features = [
      "nix-command"
      "flakes"
    ];
    users.users.${user} = {
      isNormalUser = true;
      uid = 1000;
      linger = true;
    };
    # THE ROOT HALF'S REHEARSAL, as the module declares it, and the one
    # rule that lets the person start it — with no agent.
    systemd.services.${lib.removeSuffix ".service" rehearsalUnit}.serviceConfig = {
      Type = "oneshot";
      ExecStart = "${runners}/libexec/ronoctua-update-system";
    };
    security.polkit.enable = true;
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (action.id == "org.freedesktop.systemd1.manage-units"
            && action.lookup("unit") == "${rehearsalUnit}"
            && action.lookup("verb") == "start"
            && subject.user == "${user}") {
          return polkit.Result.YES;
        }
      });
    '';
    # ROOT'S FOLDER OF THE PERSON'S DATA, and the staging folder in it.
    systemd.tmpfiles.rules = [
      "d /run/ronoctua-person 0755 root root -"
      "d /run/ronoctua-person/staged 0700 ${user} users -"
    ];
    environment.etc."ronoctua/person-applied.json".source = "${fixtures}/person-applied.json";
  };
  testScript = ''
    import re

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("user@1000.service")
    machine.succeed(
        "mkdir -p /var/lib/fixture-distro ${machineDir}/person/software ${machineDir}/local /var/lib/ronoctua-machine",
        "cp ${distro}/flake.nix /var/lib/fixture-distro/flake.nix",
        "cp ${machineFlake} ${machineDir}/flake.nix",
        "cp ${fixtures}/declared.json ${machineDir}/person/software/declared.json",
        "chmod -R u+w ${machineDir} /var/lib/fixture-distro",
        "cd ${machineDir} && nix flake lock",
        "cp ${record} /var/lib/ronoctua-machine/state.json",
        "chmod 0644 /var/lib/ronoctua-machine/state.json",
    )
    person = "runuser -u ${user} -- env HOME=/home/${user} XDG_RUNTIME_DIR=/run/user/1000 "
    machine.succeed(
        person + "mkdir -p /home/${user}/.local/state/ronoctua/person/software /home/${user}/.local/state/ronoctua/software /run/ronoctua-person/staged/person/software",
        person + "cp ${fixtures}/declared.json /home/${user}/.local/state/ronoctua/person/software/declared.json",
        person + "cp ${fixtures}/declared.json /run/ronoctua-person/staged/person/software/declared.json",
        person + "sh -c 'echo rehearsal-0001 > /run/ronoctua-person/staged/nonce'",
        person + "sh -c 'echo {} > /home/${user}/.local/state/ronoctua/software/declared-applied.json'",
    )

    # ROOT'S LOCK OF THE PERSON'S DATA (`/run/ronoctua-person/machine.lock`)
    # is made by the first program that takes it, the verification's
    # rehearsal included: a lock is not data, so the staging folder is
    # what is compared, and not the folder that holds the lock.
    PLACES = "${machineDir} /var/lib/ronoctua-machine /run/ronoctua-person/staged /home/${user}/.local/state/ronoctua /etc/ronoctua/person-applied.json"
    def snapshot():
        return machine.succeed(
            f"find {PLACES} -printf '%p %y %m %U %s %T@\\n' | sort; "
            f"find {PLACES} -type f -exec sha256sum {{}} + | sort"
        )

    before = snapshot()
    assert "declared.json" in before and "flake.lock" in before, before
    code, said = machine.execute(person + "${runners}/bin/ronoctua-update 2>&1")
    print(said)
    after = snapshot()

    with subtest("the rehearsal changed nothing"):
        if before != after:
            gone = sorted(set(before.splitlines()) - set(after.splitlines()))
            came = sorted(set(after.splitlines()) - set(before.splitlines()))
            raise Exception(f"a rehearsal changed the machine: before {gone}, after {came}")
        machine.succeed("test ! -e /run/ronoctua-update/rehearsal")

    with subtest("and it rehearsed for real what has a rehearsal of its own"):
        root = machine.succeed("cat /run/ronoctua-update/system.report")
        mine = machine.succeed("cat /run/user/1000/ronoctua-update/user.report")
        print(root, mine)
        for step in ("${steps.verify}", "${steps.apply}"):
            assert re.search(rf"^{step}\trehearsed\t", root, re.M), f"{step} did not rehearse: {root}"
        assert re.search(r"^${steps.record}\trehearsed\t", mine, re.M), mine
        assert "would-record" in said, said
        assert "\tran\t" not in root + mine, f"a rehearsal ran a step: {root}{mine}"
        assert code == 0, f"the rehearsal ended {code}"
  '';
}
