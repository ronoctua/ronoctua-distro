# The person's root half on a booted machine: the apply unit, the update's
# verify, and the approval behind the sudo door.
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
#   2. Nix: `ronoctua-person verify` over a machine flake with five entries —
#      one good, one whose evaluation throws, one whose build fails, one
#      whose pinned download cannot be fetched and one whose download is
#      there with another hash — holds EXACTLY the four broken ones, each for
#      its own reason, the good one never; and an entry mended is let go
#      while the others keep their `since`;
#   3. sudo and the hash: a brain module is approved through the door at the
#      hash `nix/person-hash.nix` computes of the machine's copy, and a
#      request at another hash is refused and spent — each said, with its
#      reason, in the approval's verdict;
#   4. root's lock: the person cannot even open it (so cannot hold it, and
#      stall root's paths), and while the update holds it (`hold`) the
#      person's apply waits and then says `busy` — never swapping the tree
#      under a rebuild.
#
# OFFLINE. The machine flake has no input (a builder of `/bin/sh`, Nix's own
# `<nix/fetchurl.nix>`), the "upstream" is an nginx on this machine behind a
# certificate made here and trusted here, and `ronoctua.invalid` resolves
# nowhere.
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

  # THE "UPSTREAM": a certificate for `pins.test`, made at build time.
  certs = pkgs.runCommand "person-vm-certs" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
      -subj "/CN=pins.test" -addext "subjectAltName=DNS:pins.test" \
      -keyout $out/key.pem -out $out/cert.pem
  '';
  # What the pin says the AppImage is (the empty file), and what upstream
  # serves now (something else).
  served = pkgs.writeTextDir "app.AppImage" "a release that is not the pinned one\n";
  emptyHash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";

  # THE MACHINE FLAKE: no input, five entries, and their fingerprints as the
  # machine flake gives them (one per entry, from the entry alone here).
  machineFlake = pkgs.writeText "flake.nix" ''
    {
      outputs = { self }:
        let
          sh = args: builtins.derivation {
            name = "entry";
            system = "x86_64-linux";
            builder = "/bin/sh";
            inherit args;
          };
          broken = builtins.pathExists ./broken;
        in {
          personEntries = {
            "nix:good" = sh [ "-c" "echo ok > $out" ];
            "nix:eval-broken" = throw "an attribute that is gone";
            "nix:build-broken" = sh [ "-c" (if broken then "exit 3" else "echo mended > $out") ];
            "appimage:gone" = import <nix/fetchurl.nix> {
              url = "https://ronoctua.invalid/gone.AppImage";
              hash = "${emptyHash}";
            };
            "appimage:changed" = import <nix/fetchurl.nix> {
              url = "https://pins.test/app.AppImage";
              hash = "${emptyHash}";
            };
          };
          personFingerprints = builtins.mapAttrs (
            id: _: builtins.hashString "sha256" (id + (if broken then "1" else "2"))
          ) self.personEntries;
        };
    }
  '';

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

        networking.hosts."127.0.0.1" = [ "pins.test" ];
        security.pki.certificateFiles = [ "${certs}/cert.pem" ];
        services.nginx = {
          enable = true;
          virtualHosts."pins.test" = {
            onlySSL = true;
            sslCertificate = "${certs}/cert.pem";
            sslCertificateKey = "${certs}/key.pem";
            root = "${served}";
          };
        };
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


    def held():
        return {h["id"]: h for h in json.loads(machine.succeed(f"cat {machine_dir}/local/held.json"))["held"]}


    start_all()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("nginx.service")
    machine.succeed(f"mkdir -p {machine_dir} && cp ${machineFlake} {machine_dir}/flake.nix && touch {machine_dir}/broken")

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

    with subtest("verify holds exactly the broken entries, each for its reason"):
        out = machine.succeed(f"{nix} {bin} verify --machine {machine_dir}")
        print(out)
        h = held()
        assert set(h) == {"nix:eval-broken", "nix:build-broken", "appimage:gone", "appimage:changed"}, h
        assert h["nix:eval-broken"]["reason"] == "eval-failed", h
        assert h["nix:build-broken"]["reason"] == "build-failed", h
        assert h["appimage:gone"]["reason"] == "fetch-failed", h
        assert h["appimage:changed"]["reason"] == "upstream-changed", h
        assert "ok nix:good" in out, out

    with subtest("a flake that does not evaluate holds nothing and keeps held.json"):
        before = machine.succeed(f"cat {machine_dir}/local/held.json")
        machine.succeed(f"mv {machine_dir}/flake.nix {machine_dir}/flake.good")
        machine.succeed(f"printf '{{ outputs = _: throw \"broken\"; }}' > {machine_dir}/flake.nix")
        machine.fail(f"{nix} {bin} verify --machine {machine_dir}")
        assert machine.succeed(f"cat {machine_dir}/local/held.json") == before
        machine.succeed(f"mv {machine_dir}/flake.good {machine_dir}/flake.nix")

    with subtest("a mended entry is let go, and the others keep their since"):
        since = held()["appimage:gone"]["since"]
        machine.succeed(f"rm {machine_dir}/broken")
        machine.succeed(f"{nix} {bin} verify --machine {machine_dir}")
        h = held()
        assert "nix:build-broken" not in h, h
        assert set(h) == {"nix:eval-broken", "appimage:gone", "appimage:changed"}, h
        assert h["appimage:gone"]["since"] == since, h

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

    with subtest("while the update holds the lock, verify runs inside it and an apply says busy"):
        out = machine.succeed(f"{nix} {bin} hold -- {bin} verify --machine {machine_dir} --rehearse")
        assert "untried nix:good" in out and "still-held appimage:gone fetch-failed" in out, out
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
