# A PART THAT DOES NOT ARRIVE HOLDS BACK NOTHING (`parts-are-isolated`).
#
# The promise of `nix/modules/parts.nix`, measured on a machine whose parts
# download from a server on the test's network:
#
#   1. three parts are wanted and one's download is missing (404): the other
#      two arrive, the session runs them, and the missing one is recorded as
#      failed and waits to be tried again — a timer armed for its due time,
#      and never a failed unit, which a switch would count as its own failure;
#   2. the person tries it again (one polkit rule, the parts' units and
#      nothing else) once the file is there, and it arrives;
#   3. an update whose new version of a part does not arrive keeps the one
#      before, working; when it does arrive the one before is kept as the
#      way back, and the profile holds two generations, never more;
#   4. a part whose interface changes is reached at its new path only once it
#      is there, and the old path stays for a rollback;
#   5. a download its origin does not have comes from a mirror the person
#      named, by its hash, and only those bytes;
#   6. a part whose output the machine already carries is seeded — its
#      profile set with no derivation and no download, as an ISO's are;
#   7. a part of the person's is evaluated by its own realizer, from the
#      machine flake (`personParts`), and alone: the good entry arrives, and
#      one whose evaluation throws, one that ABORTS — which no `tryEval`
#      catches — and one its build refuses each fail only themselves, said
#      as `eval-failed`.
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
    d1
    e1
    f1
    g1
    ;
  eDownload = parts.downloadOf "e" "1";
  # THE MACHINE FLAKE of a person with four declared programs, as
  # `lib.machineFlake` hands them out (`personParts`): the test copies it,
  # with the parts' file and the programs, into the machine directory.
  fingerprint = c: builtins.hashString "sha256" c;
  personFlake = pkgs.writeText "flake.nix" ''
    {
      outputs = { self }:
        let
          parts = import ./parts-vm-parts.nix (builtins.fromJSON (builtins.readFile ./programs.json));
        in
        {
          personParts = {
            "nix:good" = {
              drv = parts.person1.drvPath;
              out = parts.person1.outPath;
              fingerprint = "${fingerprint "good"}";
            };
            "nix:throws" = throw "nixpkgs has no `gone`";
            "nix:aborts" = abort "fixture: reading this entry aborts";
            "nix:refused" = {
              refused = "refused-by-meta";
              detail = "`x` is unfree, and its declaration carries no consent";
              fingerprint = "${fingerprint "refused"}";
            };
          };
        };
    }
  '';
  personPart = id: {
    wanted = true;
    entry = {
      inherit id;
      fingerprint = fingerprint (lib.removePrefix "nix:" id);
    };
  };

  # What the server has from the start, and what the test hands it later.
  served = pkgs.runCommand "test-parts-served" { } ''
    mkdir -p $out/now $out/later
    printf '%s' ${lib.escapeShellArg (content "a" "1")} > $out/now/${file "a" "1"}
    printf '%s' ${lib.escapeShellArg (content "c" "1")} > $out/now/${file "c" "1"}
    printf '%s' ${lib.escapeShellArg (content "b" "1")} > $out/later/${file "b" "1"}
    printf '%s' ${lib.escapeShellArg (content "a" "2")} > $out/later/${file "a" "2"}
    printf '%s' ${lib.escapeShellArg (content "d" "1")} > $out/later/${file "d" "1"}
    printf '%s' ${lib.escapeShellArg (content "e" "1")} > $out/later/${file "e" "1"}
    # THE PERSON'S MIRROR holds `f` by its hash, as tarballs.nixos.org lays
    # files out; the origin never has it.
    mkdir -p $out/now/mirror/sha256
    printf '%s' ${lib.escapeShellArg (content "f" "1")} \
      > $out/now/mirror/sha256/${builtins.hashString "sha256" (content "f" "1")}
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
      script = "cp -r ${served}/now/* /srv/";
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
        d = {
          wanted = true;
          package = d1;
        };
        e = {
          wanted = true;
          package = e1;
        };
        f = {
          wanted = true;
          package = f1;
        };
        g = {
          wanted = true;
          package = g1;
        };
        person-good = personPart "nix:good";
        person-throws = personPart "nix:throws";
        person-aborts = personPart "nix:aborts";
        person-refused = personPart "nix:refused";
      };

      # THE PERSON'S OPTIONS as root's applier leaves them in the machine
      # directory (`/etc/nixos/person/`), naming one mirror.
      environment.etc."nixos/person/options.json".text = builtins.toJSON {
        format = 1;
        ssh-signing = null;
        download-mirrors = [ "http://server/mirror" ];
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
        # `g`, BUILT: the machine carries its output, and never its
        # derivation (the VM does not instantiate it) nor its download.
        g1
      ];
      environment.etc."ronoctua-test/parts.nix".source = ./parts-vm-parts.nix;
      environment.etc."ronoctua-test/programs.json".text = builtins.toJSON programs;
      environment.etc."ronoctua-test/person-flake.nix".source = personFlake;
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
        for part in ("a1", "a2", "b1", "c1", "d1", "e1", "f1"):
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
    # THE PERSON'S MACHINE FLAKE in the machine directory: plain files, as
    # the machine's own flake is.
    for name, source in (("flake.nix", "person-flake.nix"), ("parts-vm-parts.nix", "parts.nix"),
                         ("programs.json", "programs.json")):
        machine.succeed(f"install -m 0644 /etc/ronoctua-test/{source} /etc/nixos/{name}")
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
        # `g` IS SEEDED: its output was in the store, its derivation never
        # was, and nothing served its download.
        wait_phase("g", "here")
        assert run_as_person("g") == "g 1"
        machine.fail("test -e ${plain g1.drvPath}")
        # THE MIRROR BROUGHT `f`, which its origin never had — by its hash.
        wait_phase("f", "here")
        assert run_as_person("f") == "f 1"
        server.fail("test -e /srv/${file "f" "1"}")
        machine.succeed(
            "journalctl -u ronoctua-part@f.service | grep -F 'brought from http://server/mirror/sha256/'"
        )
        # The session sees the view, and the view only the closed set.
        machine.succeed("test -L /var/lib/ronoctua-parts/view/bin/a")
        machine.fail("test -e /var/lib/ronoctua-parts/view/lib")
        # The failed part waits to be tried again — a timer armed for its
        # due time, never a unit left failed (a switch would count it).
        machine.wait_until_succeeds("systemctl is-active ronoctua-parts-retry.timer", timeout=120)
        machine.fail("systemctl is-failed ronoctua-part@b.service")

    with subtest("a part of the person's is evaluated alone, and a broken one fails only itself"):
        wait_phase("person-good", "here")
        assert run_as_person("person-good") == "person-good 1"
        for part in ("person-throws", "person-aborts", "person-refused"):
            machine.wait_until_succeeds(
                "jq -e '.attempts >= 1 and .\"last-failure\".why == \"eval-failed\"' "
                f"/var/lib/ronoctua-parts/parts/{part}.json",
                timeout=600,
            )
            assert record(part)["wanted"] is None, record(part)
            machine.fail(f"systemctl is-failed ronoctua-part@{part}.service")
        refused = record("person-refused")["last-failure"]["detail"]
        assert refused == "refused-by-meta: `x` is unfree, and its declaration carries no consent", refused
        assert "abort" in record("person-aborts")["last-failure"]["detail"], record("person-aborts")
        # What it evaluated is kept: the planner, run again, does not ask
        # the entry again while its declaration stays the same.
        before = record("person-good")["current"]
        machine.succeed("systemctl start ronoctua-parts.service")
        assert record("person-good")["current"] == before
        assert record("person-good")["declared"] == "${fingerprint "good"}"

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

    with subtest("a file handed in is taken by its hash, and another one is refused"):
        wait_phase("d", "arriving")
        machine.wait_until_succeeds(
            "jq -e '.missing | map(.name) | index(\"test-part-d-1-download\") != null' "
            "/var/lib/ronoctua-parts/parts/d.json",
            timeout=300,
        )
        missing = record("d")["missing"]
        assert missing[0]["urls"] == ["http://server/${file "d" "1"}"], missing
        # THE WRONG BYTES under the right name: refused, said, and the store
        # left as it was.
        machine.succeed(
            "su noctua -c 'mkdir -p /run/ronoctua-parts/handin/d && "
            "printf wrong > /run/ronoctua-parts/handin/d/test-part-d-1-download'"
        )
        machine.succeed("su noctua -c 'systemctl start ronoctua-parts-accept.service'")
        assert record("d")["last-failure"]["why"] == "hand-in-refused", record("d")
        machine.fail(f"test -e {missing[0]['out']}")
        machine.fail("test -e /run/ronoctua-parts/handin/d/test-part-d-1-download")
        # THE RIGHT ONE, downloaded "elsewhere" (the server's copy, fetched by
        # the person): taken, and the part arrives.
        machine.succeed(
            "su noctua -c 'cp ${served}/later/${file "d" "1"} /run/ronoctua-parts/handin/d/test-part-d-1-download'"
        )
        machine.succeed("su noctua -c 'systemctl start ronoctua-parts-accept.service'")
        machine.succeed(f"test -e {missing[0]['out']}")
        wait_phase("d", "here", timeout=300)
        assert run_as_person("d") == "d 1"

    with subtest("a stick carries a download, and nothing built"):
        wait_phase("e", "arriving")
        machine.wait_until_succeeds(
            "jq -e '.missing | length == 1' /var/lib/ronoctua-parts/parts/e.json", timeout=300
        )
        out = record("e")["missing"][0]["out"]
        # ANOTHER MACHINE'S STICK, made here: the download fetched once while
        # the server had it, exported to a folder under /run/media, and gone
        # from this store again. The part's own retries are stopped first, so
        # none of them takes the file from the server meanwhile.
        machine.succeed("systemctl stop ronoctua-part@e.service")
        server.succeed("cp ${served}/later/${file "e" "1"} /srv/")
        machine.succeed("nix build --no-link ${plain eDownload.drvPath}^out")
        machine.succeed(
            "mkdir -p /run/media/noctua/stick && "
            f"nix copy --to file:///run/media/noctua/stick/ronoctua-parts {out}"
        )
        machine.succeed(f"nix-store --delete {out}")
        server.succeed("rm /srv/${file "e" "1"}")
        machine.succeed(
            "su noctua -c 'printf /run/media/noctua/stick > /run/ronoctua-parts/import/from'"
        )
        machine.succeed("su noctua -c 'systemctl start ronoctua-parts-import.service'")
        machine.succeed(f"test -e {out}")
        said = json.loads(machine.succeed("cat /var/lib/ronoctua-parts/stick.json"))
        assert said["verb"] == "import" and said["outcome"] == {"kind": "done", "count": 1}, said
        wait_phase("e", "here", timeout=300)
        assert run_as_person("e") == "e 1"
        # A FOLDER OFF A REMOVABLE MEDIUM is refused, and the refusal said for
        # the page — root goes nowhere else the person names.
        machine.succeed("su noctua -c 'printf /home/noctua > /run/ronoctua-parts/import/from'")
        machine.succeed("su noctua -c 'systemctl start ronoctua-parts-import.service'")
        said = json.loads(machine.succeed("cat /var/lib/ronoctua-parts/stick.json"))
        assert said["outcome"] == {"kind": "refused", "why": "not-a-removable-medium"}, said

    with subtest("the downloads this machine keeps go on a stick for another"):
        # Every part that is here keeps its download rooted.
        for part in ("a", "b", "c", "d", "e", "f"):
            machine.succeed(f"test -n \"$(ls /nix/var/nix/gcroots/ronoctua-parts/kept/{part})\"")
        # THE PERSON'S GESTURE, through the unit the rule lets: the folder
        # named, the unit started, and what root wrote given to the folder's
        # owner.
        machine.succeed("mkdir -p /run/media/noctua/other && chown noctua /run/media/noctua/other")
        machine.succeed(
            "su noctua -c 'printf /run/media/noctua/other > /run/ronoctua-parts/export/to'"
        )
        machine.succeed("su noctua -c 'systemctl start ronoctua-parts-export.service'")
        machine.succeed("test -f /run/media/noctua/other/ronoctua-parts/nix-cache-info")
        assert int(machine.succeed("ls /run/media/noctua/other/ronoctua-parts/*.narinfo | wc -l")) >= 5
        owners = machine.succeed("find /run/media/noctua/other/ronoctua-parts -printf '%u\\n' | sort -u").split()
        assert owners == ["noctua"], owners
        said = json.loads(machine.succeed("cat /var/lib/ronoctua-parts/stick.json"))
        assert said["verb"] == "export" and said["outcome"]["kind"] == "done", said

    with subtest("a part at a new interface is reached at its new path"):
        wait_phase("c", "here", timeout=300)
        assert record("c")["current"]["interface"] == 2
        machine.succeed("test -x /nix/var/nix/profiles/ronoctua/parts/c/v2/bin/c")
        # The old interface stays, for a rollback to find.
        machine.succeed("test -x /nix/var/nix/profiles/ronoctua/parts/c/v1/bin/c")
        assert run_as_person("c") == "c 1"
  '';
}
