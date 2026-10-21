# THE MIGRATION KEEPS THE MACHINE (`migration-keeps-the-machine`).
#
# A machine installed before the machine flake carries a COPY of the whole
# distro in its machine directory, with `host.nix` and `facter.json` beside
# it. `ronoctua-machine migrate` makes that folder the machine's own flake —
# the template's `flake.nix`, the machine's files byte for byte — and only
# when the new flake LOCKS AND BUILDS; otherwise nothing changes.
#
# On the machine it runs on, the root is wiped at every boot and the machine
# directory is a BIND MOUNT from `/persist`, and so is the machine's record
# under `/var/lib`: a mount point cannot be renamed, and a rename between
# two mounts fails. This plants exactly that, an old layout with the
# person's data and root's record of it in it, and proves:
#
#   1. a distro that does not build leaves every byte where it was, and
#      nothing of the attempt behind;
#   2. a distro that builds gives the template's flake (pointed at it), the
#      new lock — kept too as the one an apply that fails puts back —,
#      `host.nix`, `facter.json` and the secrets byte for byte, `person/` and
#      `local/` untouched — and the old layout whole under
#      `/var/lib/ronoctua-machine/legacy-<stamp>`, the mount still there;
#   3. the new folder is a flake that builds, and a second migration has
#      nothing to do.
#
# THE DISTRO IS A FLAKE OF ITS OWN, from `path:` — the real `nix flake lock`
# and `nix build` run against it, offline. It is small on purpose: its
# `lib.machineFlake` answers the one attribute a machine flake is asked for
# with a derivation `/bin/sh` builds in the sandbox (or one that fails).
# Evaluating the real distro inside a VM would need its every input and the
# network; what is proved here is the migration's handling of the folder and
# of Nix, which is the same either way.
{
  pkgs,
  lib,
  # `ronoctua-machine`, extracted (`rust.machine`).
  machineTool,
}:

let
  distro =
    builds:
    pkgs.writeTextDir "flake.nix" ''
      {
        outputs = { self }: {
          lib.machineFlake = { dir, contract ? 1, ... }: {
            nixosConfigurations.noctua.config.system.build.toplevel = derivation {
              name = "nixos-system-migrated";
              system = "${pkgs.stdenv.hostPlatform.system}";
              builder = "/bin/sh";
              args = [ "-c" "${
                if builds then "echo built > $out" else "echo this distro does not build >&2; exit 1"
              }" ];
            };
          };
        };
      }
    '';
  good = distro true;
  bad = distro false;
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-migration";

  nodes.machine = {
    imports = [ ../nix/modules/machine.nix ];

    # The one option of the distro's `base.nix` the machine module reads.
    options.ronoctua.user = lib.mkOption {
      type = lib.types.str;
      default = "noctua";
    };

    config = {
      ronoctua.machine.tool = machineTool;
      virtualisation.additionalPaths = [
        good
        bad
      ];
      nix.settings = {
        experimental-features = [
          "nix-command"
          "flakes"
        ];
        # Offline: no registry is fetched for a `path:` flake.
        flake-registry = "";
      };
    };
  };

  testScript = ''
    import json


    def files(where):
        """Every file under `where`, with its hash, as one text."""
        return machine.succeed(
            f"cd {where} && find . -type f -print0 | sort -z | xargs -0 sha256sum"
        )


    def migrate(distro):
        return machine.execute(
            f"RONOCTUA_MACHINE_DISTRO=path:{distro} ronoctua-machine migrate 2>&1"
        )


    machine.wait_for_unit("multi-user.target")

    # THE OLD LAYOUT, on a bind mount from /persist, as a wiped root has it.
    machine.succeed(
        "mkdir -p /persist/etc/nixos /persist/var/lib/ronoctua-machine /etc/nixos /var/lib/ronoctua-machine",
        "mount --bind /persist/etc/nixos /etc/nixos",
        "mount --bind /persist/var/lib/ronoctua-machine /var/lib/ronoctua-machine",
        "cd /etc/nixos && mkdir -p nix/modules crates/ronoctua-shell secrets person/software local",
        "echo '{ }' > /etc/nixos/nix/modules/base.nix",
        "echo 'fn main() {}' > /etc/nixos/crates/ronoctua-shell/main.rs",
        "echo '{ outputs = _: { }; }' > /etc/nixos/flake.nix",
        "printf '{ ... }:\\n{\\n  networking.hostName = \"old\";\\n}\\n' > /etc/nixos/host.nix",
        "printf '{\"version\": 2}\\n' > /etc/nixos/facter.json",
        "printf 'secret: ENC[...]\\n' > /etc/nixos/secrets/secrets.yaml",
        "chmod 0600 /etc/nixos/secrets/secrets.yaml",
        "printf '{\"format\": 1, \"apps\": []}\\n' > /etc/nixos/person/software/declared.json",
        "printf '{\"format\": 1, \"held\": []}\\n' > /etc/nixos/local/held.json",
    )
    before = files("/etc/nixos")
    host = machine.succeed("sha256sum < /etc/nixos/host.nix")
    facter = machine.succeed("sha256sum < /etc/nixos/facter.json")
    secret = machine.succeed("sha256sum < /etc/nixos/secrets/secrets.yaml")
    person = files("/etc/nixos/person") + files("/etc/nixos/local")

    with subtest("a distro that does not build changes nothing"):
        status, out = migrate("${bad}")
        print(out)
        assert status == 1 and out.strip().endswith("refused build-failed"), out
        assert files("/etc/nixos") == before, "the old layout changed"
        machine.fail("test -e /etc/nixos/.next")
        machine.fail("ls -d /var/lib/ronoctua-machine/legacy-*")

    with subtest("a distro that builds gives the machine its own flake"):
        status, out = migrate("${good}")
        print(out)
        assert status == 0, out
        names = machine.succeed("ls -A /etc/nixos").split()
        assert names == ["facter.json", "flake.lock", "flake.nix", "host.nix", "local", "person", "secrets.yaml"], names
        assert machine.succeed("sha256sum < /etc/nixos/host.nix") == host
        assert machine.succeed("sha256sum < /etc/nixos/facter.json") == facter
        assert machine.succeed("sha256sum < /etc/nixos/secrets.yaml") == secret
        assert machine.succeed("stat -c %a /etc/nixos/secrets.yaml").strip() == "600"
        assert files("/etc/nixos/person") + files("/etc/nixos/local") == person
        flake = machine.succeed("cat /etc/nixos/flake.nix")
        assert 'inputs.ronoctua.url = "path:${good}";' in flake, flake
        assert "machineFlake" in flake, flake
        assert "${good}" in machine.succeed("cat /etc/nixos/flake.lock")
        # The lock it built is the one a failed apply puts back: the system
        # that runs was built from the old layout and carries none.
        machine.succeed("cmp /etc/nixos/flake.lock /var/lib/ronoctua-machine/applied.lock")
        # The mount point is still the mount: the folder was never renamed.
        machine.succeed("findmnt --mountpoint /etc/nixos")
        legacy = machine.succeed("ls -d /var/lib/ronoctua-machine/legacy-*").split()
        assert len(legacy) == 1, legacy
        assert files(legacy[0]) == before, "the old layout is not whole in " + legacy[0]

    with subtest("the new folder builds, and there is nothing more to migrate"):
        machine.succeed(
            "nix build --no-link path:/etc/nixos#nixosConfigurations.noctua.config.system.build.toplevel"
        )
        status, out = migrate("${good}")
        assert status == 1 and out.strip().endswith("refused nothing-to-migrate"), out

    with subtest("the sources the machine's flake was evaluated from outlive a collection"):
        # WHAT `keep-sources` ROOTS — at every boot, and the apply after
        # every build — is what `nix flake archive` names off the lock: here
        # the machine's flake and the distro. Nothing else holds them: no
        # derivation of a system cites them, and before the roots a
        # collection would take them (a later update fetching them again).
        names = json.loads(machine.succeed("nix flake archive --dry-run --json path:/etc/nixos"))
        sources = [names["path"]] + [i["path"] for i in names["inputs"].values()]
        assert len(sources) == 2, sources
        dead = set(machine.succeed("nix-store --gc --print-dead 2>/dev/null").split())
        assert dead & set(sources), "no source is garbage before the roots: this proves nothing"
        status, out = machine.execute("ronoctua-machine keep-sources 2>&1")
        print(out)
        assert status == 0 and out.strip().endswith("sources kept"), out
        dead = set(machine.succeed("nix-store --gc --print-dead 2>/dev/null").split())
        assert not dead & set(sources), f"a source is still garbage: {dead & set(sources)}"
        machine.succeed("nix-store --gc")
        for path in sources:
            machine.succeed(f"nix-store --check-validity {path}")
  '';
}
