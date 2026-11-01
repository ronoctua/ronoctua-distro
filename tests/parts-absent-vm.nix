# A MACHINE WITH NO PART REALISED YET WORKS (`parts-absent-vm`).
#
# An app that fails to install never breaks the distro's boot: measured on a
# machine that wants two parts and has neither — one whose build fails in
# its own builder, one whose download no network of the machine reaches —
# and realizes them as every machine does, from the boot's own trigger:
#
#   1. the machine boots to the end with nothing failed (`is-system-running`
#      says `running`, never `degraded`): both realizers ran during the boot
#      and failed, each said in its record and neither a failed unit;
#   2. the record of the broken part says the error itself — Nix's
#      `error:` lines and what they indent, the builder's words among them —
#      and never the list of what Nix would build before it nor the outputs
#      it would have made, which stay, with the whole of what Nix said, in
#      the realizer's journal;
#   3. the base works without its parts: the person's session runs, a part's
#      program is simply not there, and a unit of the base that names a part
#      by its stable path (as the footer names Claude Code's) runs and says
#      the part is arriving;
#   4. an update whose new version of the broken part fails too switches
#      with no unit failed, and the part is tried at its new version.
#
# THE PARTS ARE SMALL ON PURPOSE (`tests/parts-absent-vm-parts.nix`): built in
# the VM with the bash, curl and coreutils it already has. The derivations
# are instantiated by a unit ordered before the boot's trigger, so the first
# boot's realizers find them as a machine's do after its installation.
{
  pkgs,
  lib,
  # `ronoctua-parts`, extracted (`rust.parts`).
  partsTool,
}:

let
  plain = builtins.unsafeDiscardStringContext;
  programs = {
    system = pkgs.stdenv.hostPlatform.system;
    bash = plain "${pkgs.bash}";
    curl = plain "${pkgs.curl.bin}";
    coreutils = plain "${pkgs.coreutils}";
  };
  parts = import ./parts-absent-vm-parts.nix programs;
  instantiate = pkgs.writeShellScript "parts-absent-instantiate" ''
    for part in broken1 broken2 offline; do
      ${pkgs.nix}/bin/nix-instantiate --expr \
        'import /etc/ronoctua-test/parts.nix (builtins.fromJSON (builtins.unsafeDiscardStringContext (builtins.readFile /etc/ronoctua-test/programs.json)))' \
        -A "$part" >/dev/null
    done
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-parts-absent";

  nodes.machine =
    { config, ... }:
    {
      imports = [ (import ../nix/modules/parts.nix { tool = partsTool; }) ];

      # The one option of the distro's `base.nix` the parts' module reads.
      options.ronoctua.user = lib.mkOption {
        type = lib.types.str;
        default = "noctua";
      };

      config = {
        virtualisation.memorySize = 1024;
        users.users.noctua = {
          isNormalUser = true;
          uid = 1000;
        };
        environment.systemPackages = [ pkgs.jq ];
        security.polkit.enable = true;

        ronoctua.parts = {
          broken = {
            wanted = true;
            package = parts.broken1;
          };
          offline = {
            wanted = true;
            kind = "data";
            package = parts.offline;
          };
        };

        # THE UPDATE: a new version of the broken part, broken too.
        specialisation.next.configuration.ronoctua.parts.broken.package = lib.mkForce parts.broken2;

        # A UNIT OF THE BASE THAT NAMES A PART by its stable path, as the
        # footer's names Claude Code's: it runs whether the part is here or
        # not, and says which.
        systemd.services.reads-a-part = {
          wantedBy = [ "multi-user.target" ];
          environment.PART = config.ronoctua.parts.broken.path;
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
          };
          script = ''
            if [ -x "$PART/bin/broken" ]; then
              echo "the part is here"
            else
              echo "the part is arriving"
            fi
          '';
        };

        # THE PARTS' DERIVATIONS, as an installation leaves them in the store:
        # instantiated from the same file the host evaluated, before the
        # boot's trigger starts the planner.
        systemd.services.parts-absent-instantiate = {
          wantedBy = [ "multi-user.target" ];
          before = [ "ronoctua-parts-trigger.service" ];
          requiredBy = [ "ronoctua-parts-trigger.service" ];
          after = [ "nix-daemon.socket" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = instantiate;
          };
        };
        environment.etc."ronoctua-test/parts.nix".source = ./parts-absent-vm-parts.nix;
        environment.etc."ronoctua-test/programs.json".text = builtins.toJSON programs;
        virtualisation.additionalPaths = [
          pkgs.bash
          pkgs.curl.bin
          pkgs.coreutils
        ];
        virtualisation.writableStore = true;
        nix.settings = {
          experimental-features = [ "nix-command" ];
          substituters = lib.mkForce [ ];
        };
      };
    };

  testScript = ''
    import json
    from datetime import timedelta


    def record(part):
        return json.loads(machine.succeed(f"cat /var/lib/ronoctua-parts/parts/{part}.json"))


    def tried(part, drv):
        machine.wait_until_succeeds(
            f"jq -e '.wanted.drv == \"{drv}\" and .attempts >= 1 and .\"last-failure\" != null' "
            f"/var/lib/ronoctua-parts/parts/{part}.json",
            timeout=timedelta(minutes=5),
        )


    machine.start()

    with subtest("the machine boots to the end while its parts fail"):
        tried("broken", "${plain parts.broken1.drvPath}")
        tried("offline", "${plain parts.offline.drvPath}")
        state = machine.succeed("systemctl is-system-running --wait || true").strip()
        failed = machine.succeed("systemctl --failed --no-legend --plain | cat").strip()
        assert state == "running" and failed == "", f"the boot ended {state}:\n{failed}"
        for part in ("broken", "offline"):
            r = record(part)
            assert r["current"] is None and r["phase"] in ("arriving", "waiting"), r
            assert r["last-failure"]["why"] == "build-failed", r
            machine.fail(f"systemctl is-failed ronoctua-part@{part}.service")
            machine.fail(f"test -e /nix/var/nix/profiles/ronoctua/parts/{part}")

    with subtest("the record says the error itself, and the journal all of it"):
        detail = record("broken")["last-failure"]["detail"]
        assert detail.startswith("error:"), detail
        assert "the planted failure: this part does not build" in detail, detail
        assert "will be built" not in detail and "Output paths" not in detail, detail
        whole = machine.succeed("journalctl -b -u ronoctua-part@broken.service -o cat")
        assert "will be built" in whole and "the planted failure" in whole, whole

    with subtest("the base works without its parts"):
        assert machine.succeed("su - noctua -c 'echo here'").strip() == "here"
        machine.fail("su - noctua -c 'command -v broken'")
        said = machine.succeed("journalctl -b -u reads-a-part.service -o cat")
        assert "the part is arriving" in said, said
        machine.succeed("systemctl is-active reads-a-part.service")

    with subtest("an update whose part fails again switches with nothing failed"):
        machine.succeed("/run/current-system/specialisation/next/bin/switch-to-configuration test")
        tried("broken", "${plain parts.broken2.drvPath}")
        failed = machine.succeed("systemctl --failed --no-legend --plain | cat").strip()
        assert failed == "", f"units failed after the update:\n{failed}"
        assert record("broken")["current"] is None
  '';
}
