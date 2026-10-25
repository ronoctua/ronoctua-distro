# The person's root half on a booted machine: the apply unit and the
# approval behind the sudo door.
#
# WHAT ONLY THIS PROVES
# ---------------------
# `ronoctua-person`'s tests run the same code against a `nix` of their own
# making, and `person-apply-refuses-links` runs the applier as an ordinary
# user. Here the real ones answer:
#
#   1. polkit and systemd: the person, with no agent and no terminal, starts
#      `ronoctua-person-apply.service`, and the unit — as root, inside its
#      sandbox — copies the staged tree into the machine directory; the same
#      person cannot start the approval's unit, nor restart the apply;
#   1a. the person's signature: with no key pinned the declarations arrive
#      and a brain module never does (`no-key-pinned`); the key is pinned
#      through the sudo door (`person-signer-pin`, the pin's own unit), and
#      then the tree the person signed with it brings the module in;
#   2. sudo and the hash: a brain module is approved through the door at the
#      hash `nix/person-hash.nix` computes of the machine's copy, and a
#      request at another hash is refused and spent — each said, with its
#      reason, in the approval's verdict;
#   3. root's lock: the person cannot even open it (so cannot hold it, and
#      stall root's paths), and while the update holds it (`hold`) the
#      person's apply waits and then says `busy` — never swapping the tree
#      under a rebuild.
#
# OFFLINE: nothing here reaches a network.
#
# THE PERSON IS A PROCESS WITH NO TERMINAL, as the footer is (the
# `security-vm` precedent): `systemd-run --uid` with stdin from nowhere.
{
  pkgs,
  lib,
  person,
  securityRoot,
  # `ronoctua-test-pq`: makes the person's signing key and signs a staged
  # tree as the belongings sign one.
  testPq,
}:

let
  user = "noctua";
  # THE TEST'S OWN PASSWORD — a fixture, never a person's.
  password = "a door, not a window";
  bin = "${person}/bin/ronoctua-person";
  helper = "${securityRoot}/bin/ronoctua-security-root";
  pq = "${testPq}/bin/ronoctua-test-pq";
  hasher = ../nix/person-hash.nix;

  emptyHash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";

  # THE PERSON'S DECLARATIONS, as their software stages them: the two pins,
  # and one brain module.
  entry = key: url: ''
    {"source": "appimage", "id": "appimage:${key}", "machines": null,
     "edited-at": "2026-10-05T12:00:00Z", "edited-on": "machine", "key": "${key}",
     "name": "App", "pin": {"url": "${url}", "hash": "${emptyHash}", "version": "1",
     "pinned-at": "2026-10-05T12:00:00Z"}, "follows": null, "icon": null}
  '';
  declared = pkgs.writeText "declared.json" ''
    {"format": 1, "apps": [
      ${entry "gone" "https://ronoctua.invalid/gone.AppImage"},
      ${entry "changed" "https://pins.test/app.AppImage"}
    ]}
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-person";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../nix/modules/security.nix
        ../nix/modules/person-apply.nix
      ];

      # The modules read `config.ronoctua.user`, which belongs to `base.nix`
      # — and base brings the whole system with it (the `security-vm`
      # precedent); and the machine directory and the tool, whose modules
      # (`machine.nix`, `person.nix`) carry the whole machine flake's
      # consumer. Declaring the three options read is that precedent again.
      options.ronoctua.user = lib.mkOption { type = lib.types.str; };
      options.ronoctua.machine.dir = lib.mkOption {
        type = lib.types.str;
        default = "/etc/nixos";
      };
      options.ronoctua.person.tool = lib.mkOption { type = lib.types.package; };

      config = {
        ronoctua.user = user;
        ronoctua.security.helper = securityRoot;
        ronoctua.person.tool = person;

        virtualisation.memorySize = 2048;
        virtualisation.cores = 2;

        users.mutableUsers = false;
        users.users.${user} = {
          isNormalUser = true;
          group = "users";
          uid = 1000;
          extraGroups = [ "wheel" ];
          inherit password;
        };

        nix.settings.experimental-features = [
          "nix-command"
          "flakes"
        ];

      };
    };

  testScript = ''
    import json
    import shlex

    user = "${user}"
    password = "${password}"
    bin = "${bin}"
    helper = "${helper}"
    pq = "${pq}"
    nix = "RONOCTUA_NIX=${pkgs.nix}/bin/nix"
    machine_dir = "/etc/nixos"

    person = "systemd-run --wait --pipe --quiet --uid=1000 --gid=100 "


    def as_user(command):
        return (
            person
            + "--setenv=PATH=/run/wrappers/bin:/run/current-system/sw/bin "
            + f"-- ${pkgs.bash}/bin/bash -c {shlex.quote(command)} < /dev/null"
        )


    def door(verb, typed=password):
        # What `ronoctua_security::authority` runs: nothing but LC_ALL=C, a
        # session of its own with no terminal, the password on stdin.
        return (
            f"printf '%s\\n' {shlex.quote(typed)} | "
            + person
            + "-- ${pkgs.coreutils}/bin/env -i LC_ALL=C ${pkgs.util-linux}/bin/setsid --wait "
            + f"/run/wrappers/bin/sudo -S -k -p ''' -- {helper} {verb}"
        )


    start_all()
    machine.wait_for_unit("multi-user.target")
    machine.succeed(f"mkdir -p {machine_dir}")

    with subtest("the person stages a tree and the apply unit swaps it in, through polkit"):
        machine.succeed(as_user(
            "mkdir -p /run/ronoctua-person/staged/person/software "
            + "/run/ronoctua-person/staged/brain-modules/my-mod && "
            + "cp ${declared} /run/ronoctua-person/staged/person/software/declared.json && "
            + "printf '{ ... }: { }\\n' > /run/ronoctua-person/staged/brain-modules/my-mod/module.nix && "
            + "printf 'vm-stage-0001\\n' > /run/ronoctua-person/staged/nonce && "
            + "systemctl start ronoctua-person-apply.service"
        ))
        verdict = json.loads(machine.succeed("cat /run/ronoctua-person/verdict.json"))
        assert verdict["outcome"] == "applied", verdict
        assert verdict["nonce"] == "vm-stage-0001", verdict
        machine.succeed(f"test -f {machine_dir}/person/software/declared.json")
        owner = machine.succeed(f"stat -c %U:%a {machine_dir}/person/software/declared.json").strip()
        assert owner == "root:644", owner
        # NO KEY IS PINNED YET: the code the brain would compile waits for the
        # person's signature, said in the verdict.
        left = {l["module"]: l for l in verdict["left-out"]}
        assert left.get("my-mod", {}).get("why") == "no-key-pinned", verdict
        machine.fail(f"test -e {machine_dir}/person/brain/my-mod")

    with subtest("the key is pinned through the door, and what the person signed arrives"):
        key = machine.succeed(as_user(
            f"umask 077 && {pq} signing-key /home/{user}/signing.key"
        )).strip()
        machine.succeed(door(f"person-signer-pin {shlex.quote(key)}"))
        signer = json.loads(machine.succeed(f"cat {machine_dir}/local/signer.json"))
        assert signer["key"] == key, signer
        machine.fail("test -e /run/ronoctua-person/signer-pin")
        machine.succeed(as_user(
            f"{pq} sign-tree /home/{user}/signing.key /run/ronoctua-person/staged 1 && "
            + "rm -f /run/ronoctua-person/staged/nonce && "
            + "printf 'vm-stage-0002\\n' > /run/ronoctua-person/staged/nonce && "
            + "systemctl start ronoctua-person-apply.service"
        ))
        verdict = json.loads(machine.succeed("cat /run/ronoctua-person/verdict.json"))
        assert verdict["outcome"] == "applied", verdict
        assert verdict["nonce"] == "vm-stage-0002", verdict
        assert verdict["left-out"] == [] and verdict["kept-back"] == [], verdict
        machine.succeed(f"test -f {machine_dir}/person/brain/my-mod/module.nix")

    with subtest("the same person may start nothing else of it, nor hold root's lock"):
        machine.fail(as_user("systemctl restart ronoctua-person-apply.service"))
        machine.fail(as_user("systemctl start ronoctua-person-approve.service"))
        lock = machine.succeed("stat -c %U:%a /run/ronoctua-person/machine.lock").strip()
        assert lock == "root:600", lock
        machine.fail(as_user("flock --nonblock /run/ronoctua-person/machine.lock true"))

    with subtest("a module is approved through the door at the hash root computes"):
        shown = machine.succeed(
            "${pkgs.nix}/bin/nix eval --raw --impure --extra-experimental-features nix-command "
            + f"--expr 'import ${hasher} \"{machine_dir}/person/brain/my-mod\"'"
        ).strip()
        machine.succeed(as_user(f"{bin} request-approval my-mod {shlex.quote(shown)}"))
        machine.succeed(door(f"person-module-approve my-mod {shlex.quote(shown)}"))
        approved = json.loads(machine.succeed(f"cat {machine_dir}/local/approved.json"))
        assert approved["modules"]["my-mod"]["hash"] == shown, approved
        machine.fail("test -e /run/ronoctua-person/approve/request.json")
        said = json.loads(machine.succeed(as_user("cat /run/ronoctua-person/approve-verdict.json")))
        assert said["outcome"] == "approved" and said["computed"] == shown, said

    with subtest("a request at another hash is refused, and spent"):
        before = machine.succeed(f"cat {machine_dir}/local/approved.json")
        machine.succeed(as_user(f"{bin} request-approval my-mod sha256-not-what-root-computes"))
        out = machine.fail(door("person-module-approve my-mod sha256-not-what-root-computes"))
        assert "refused" in out, out
        assert machine.succeed(f"cat {machine_dir}/local/approved.json") == before
        machine.fail("test -e /run/ronoctua-person/approve/request.json")
        said = json.loads(machine.succeed(as_user("cat /run/ronoctua-person/approve-verdict.json")))
        assert said["outcome"] == "refused" and said["denied"] == "changed", said
        assert said["computed"] == shown and said["hash"] == "sha256-not-what-root-computes", said

    with subtest("a request that is not what the door was given is refused, and both are spent"):
        # ANOTHER PROCESS OF THE PERSON'S swapped the request while sudo read
        # the password: root's pin holds what the door was given.
        before = machine.succeed(f"cat {machine_dir}/local/approved.json")
        machine.succeed(as_user(f"{bin} request-approval my-mod sha256-swapped-in"))
        out = machine.fail(door(f"person-module-approve my-mod {shlex.quote(shown)}"))
        assert "refused" in out, out
        assert machine.succeed(f"cat {machine_dir}/local/approved.json") == before
        machine.fail("test -e /run/ronoctua-person/approve/request.json")
        machine.fail("test -e /run/ronoctua-person/approve-pin")
        said = json.loads(machine.succeed(as_user("cat /run/ronoctua-person/approve-verdict.json")))
        assert said["outcome"] == "refused" and said["denied"] == "invalid-request", said
        # AND THE DOOR'S WORDS ARE AN ID AND A HASH, or the helper refuses
        # before any unit starts.
        machine.fail(door("person-module-approve"))
        machine.fail(door("person-module-approve not-a-person-module x"))

    with subtest("while the update holds the lock, what it runs runs inside it and an apply says busy"):
        machine.succeed(f"{nix} {bin} hold -- ${pkgs.coreutils}/bin/true")
        machine.succeed(
            f"systemd-run --unit=update-holds {bin} hold -- "
            # Both programs by store path: the transient unit's PATH has no
            # coreutils, and a bare `touch` was "command not found".
            + "${pkgs.bash}/bin/sh -c '${pkgs.coreutils}/bin/touch /run/held-now; exec ${pkgs.coreutils}/bin/sleep 120'"
        )
        machine.wait_until_succeeds("test -e /run/held-now")
        machine.succeed(as_user(
            "printf 'vm-stage-0003\\n' > /run/ronoctua-person/staged/nonce"
        ))
        machine.fail(as_user("systemctl start ronoctua-person-apply.service"))
        verdict = json.loads(machine.succeed("cat /run/ronoctua-person/verdict.json"))
        assert verdict["outcome"] == "refused" and verdict["refused"]["why"] == "busy", verdict
        machine.succeed("systemctl stop update-holds")
        machine.succeed(as_user("systemctl start ronoctua-person-apply.service"))
        verdict = json.loads(machine.succeed("cat /run/ronoctua-person/verdict.json"))
        assert verdict["nonce"] == "vm-stage-0003" and verdict["outcome"] != "refused", verdict
  '';
}
