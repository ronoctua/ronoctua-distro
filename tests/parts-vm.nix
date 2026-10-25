# A PART THAT DOES NOT ARRIVE HOLDS BACK NOTHING (`parts-are-isolated`).
#
# The promise of `nix/modules/parts.nix`, measured on a machine whose parts
# download from a server on the test's network:
#
#   1. three parts are wanted and one's download is missing (404): the other
#      two arrive, the session runs them, and the missing one is recorded as
#      failed and waits to be tried again — systemd's restart, not a give-up;
#   2. the person tries it again (one polkit rule, the parts' units and
#      nothing else) once the file is there, and it arrives;
#   3. an update whose new version of a part does not arrive keeps the one
#      before, working; when it does arrive the one before is kept as the
#      way back, and the profile holds two generations, never more;
#   4. a part whose interface changes is reached at its new path only once it
#      is there, and the old path stays for a rollback.
#
# THE PARTS ARE SMALL ON PURPOSE: a download (a fixed-output derivation that
# runs curl against the server) and a script around it, built in the VM with
# bash, curl and coreutils it already has — no stdenv, no cache, no network
# but the test's. What is proved is the realizer's handling of the plan, the
# records, the profiles and the units, which is the same for Chrome.
{
  pkgs,
  lib,
  # `ronoctua-parts`, extracted (`rust.parts`).
  partsTool,
}:

let
  plain = builtins.unsafeDiscardStringContext;
  # The programs the parts are built with, as store paths
  # (`tests/parts-vm-parts.nix` says why); the VM holds them through
  # `additionalPaths`.
  programs = {
    system = pkgs.stdenv.hostPlatform.system;
    bash = plain "${pkgs.bash}";
    curl = plain "${pkgs.curl.bin}";
    coreutils = plain "${pkgs.coreutils}";
  };
  parts = import ./parts-vm-parts.nix programs;
  inherit (parts)
    content
    file
    a1
    a2
    b1
    c1
    ;

  # What the server has from the start, and what the test hands it later.
  served = pkgs.runCommand "test-parts-served" { } ''
    mkdir -p $out/now $out/later
    printf '%s' ${lib.escapeShellArg (content "a" "1")} > $out/now/${file "a" "1"}
    printf '%s' ${lib.escapeShellArg (content "c" "1")} > $out/now/${file "c" "1"}
    printf '%s' ${lib.escapeShellArg (content "b" "1")} > $out/later/${file "b" "1"}
    printf '%s' ${lib.escapeShellArg (content "a" "2")} > $out/later/${file "a" "2"}
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-parts";

  nodes.server = {
    networking.firewall.allowedTCPPorts = [ 80 ];
    systemd.tmpfiles.rules = [ "d /srv 0755 root root -" ];
    services.nginx = {
      enable = true;
      virtualHosts.server.root = "/srv";
    };
    systemd.services.parts-served = {
      wantedBy = [ "multi-user.target" ];
      before = [ "nginx.service" ];
      serviceConfig.Type = "oneshot";
      script = "cp ${served}/now/* /srv/";
    };
  };

  nodes.machine = {
    imports = [ (import ../nix/modules/parts.nix { tool = partsTool; }) ];

    # The one option of the distro's `base.nix` the parts' module reads.
    options.ronoctua.user = lib.mkOption {
      type = lib.types.str;
      default = "noctua";
    };

    config = {
      users.users.noctua = {
        isNormalUser = true;
        uid = 1000;
      };
      environment.systemPackages = [ pkgs.jq ];
      # The rule that lets the person start the parts' units.
      security.polkit.enable = true;

      ronoctua.parts = {
        a = {
          wanted = true;
          package = a1;
        };
        b = {
          wanted = true;
          package = b1;
        };
        c = {
          wanted = true;
          package = c1;
          heavy = true;
        };
      };

      # THE UPDATE: a new version of `a` whose download is not there yet, and
      # `c` at a new interface.
      specialisation.next.configuration = {
        ronoctua.parts.a.package = lib.mkForce a2;
        ronoctua.parts.c.interface = lib.mkForce 2;
      };

      # WHAT BUILDING THE PARTS NEEDS, and not the parts: the VM evaluates
      # `parts-vm-parts.nix` itself for their `.drv` files (the file says
      # why).
      virtualisation.additionalPaths = [
        pkgs.bash
        pkgs.curl.bin
        pkgs.coreutils
      ];
      environment.etc."ronoctua-test/parts.nix".source = ./parts-vm-parts.nix;
      environment.etc."ronoctua-test/programs.json".text = builtins.toJSON programs;
      # The test starts the trigger itself, once the derivations exist.
      systemd.services.ronoctua-parts-trigger.wantedBy = lib.mkForce [ ];
      virtualisation.writableStore = true;
      nix.settings = {
        experimental-features = [ "nix-command" ];
        substituters = lib.mkForce [ ];
      };
    };
  };

  testScript = ''
    import json


    def record(part):
        return json.loads(machine.succeed(f"cat /var/lib/ronoctua-parts/parts/{part}.json"))


    def wait_phase(part, phase, timeout=600):
        machine.wait_until_succeeds(
            f"jq -e '.phase == \"{phase}\"' /var/lib/ronoctua-parts/parts/{part}.json",
            timeout=timeout,
        )


    def run_as_person(command):
        return machine.succeed(f"su - noctua -c {command!r}").strip()


    def instantiate():
        # The parts' derivations, evaluated here from the same file the host
        # evaluated: the same `.drv` paths the plan names.
        for part in ("a1", "a2", "b1", "c1"):
            machine.succeed(
                "nix-instantiate --expr 'import /etc/ronoctua-test/parts.nix "
                "(builtins.fromJSON (builtins.unsafeDiscardStringContext "
                "(builtins.readFile /etc/ronoctua-test/programs.json)))' "
                f"-A {part} >/dev/null"
            )


    start_all()
    server.wait_for_unit("nginx.service")
    machine.wait_for_unit("multi-user.target")
    instantiate()
    for drv in ("${plain a1.drvPath}", "${plain b1.drvPath}", "${plain c1.drvPath}"):
        machine.succeed(f"nix-store --check-validity {drv}")
    machine.succeed("systemctl start ronoctua-parts-trigger.service")

    with subtest("a part whose download fails does not hold back the others"):
        wait_phase("a", "here")
        wait_phase("c", "here")
        machine.wait_until_succeeds(
            "jq -e '.attempts >= 1 and .\"last-failure\".why == \"build-failed\"' "
            "/var/lib/ronoctua-parts/parts/b.json",
            timeout=600,
        )
        assert run_as_person("a") == "a 1"
        assert run_as_person("c") == "c 1"
        machine.fail("su - noctua -c 'command -v b'")
        # The session sees the view, and the view only the closed set.
        machine.succeed("test -L /var/lib/ronoctua-parts/view/bin/a")
        machine.fail("test -e /var/lib/ronoctua-parts/view/lib")
        # The failed part waits to be tried again: a restart, not a give-up.
        machine.wait_until_succeeds(
            "systemctl show ronoctua-part@b.service -p SubState --value | grep -qx auto-restart",
            timeout=120,
        )

    with subtest("the person tries a part again, and only the parts' units"):
        server.succeed("cp ${served}/later/${file "b" "1"} /srv/")
        machine.succeed("su noctua -c 'systemctl start ronoctua-part@b.service'")
        wait_phase("b", "here", timeout=300)
        assert run_as_person("b") == "b 1"
        machine.fail("su noctua -c 'systemctl start systemd-tmpfiles-clean.service'")

    with subtest("an update whose part does not arrive keeps the one before"):
        machine.succeed("/run/current-system/specialisation/next/bin/switch-to-configuration test")
        machine.succeed("systemctl start ronoctua-parts-trigger.service")
        machine.wait_until_succeeds(
            "jq -e '.wanted.out == \"${plain a2.outPath}\" and .attempts >= 1' "
            "/var/lib/ronoctua-parts/parts/a.json",
            timeout=600,
        )
        a = record("a")
        assert a["current"]["out"] == "${plain a1.outPath}", a
        assert run_as_person("a") == "a 1"

        server.succeed("cp ${served}/later/${file "a" "2"} /srv/")
        machine.succeed("su noctua -c 'systemctl start ronoctua-part@a.service'")
        wait_phase("a", "here", timeout=300)
        a = record("a")
        assert a["current"]["out"] == "${plain a2.outPath}", a
        assert a["previous"]["out"] == "${plain a1.outPath}", a
        assert run_as_person("a") == "a 2"
        generations = machine.succeed(
            "nix-env --list-generations -p /nix/var/nix/profiles/ronoctua/parts/a/v1"
        ).strip().splitlines()
        assert len(generations) == 2, generations

    with subtest("a part at a new interface is reached at its new path"):
        wait_phase("c", "here", timeout=300)
        assert record("c")["current"]["interface"] == 2
        machine.succeed("test -x /nix/var/nix/profiles/ronoctua/parts/c/v2/bin/c")
        # The old interface stays, for a rollback to find.
        machine.succeed("test -x /nix/var/nix/profiles/ronoctua/parts/c/v1/bin/c")
        assert run_as_person("c") == "c 1"
  '';
}
