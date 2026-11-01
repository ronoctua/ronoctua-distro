# THE DISTRO'S APPIMAGES FOLLOW THEIR RELEASES, END TO END
# (`distro-apps-follow-a-release-vm`).
#
# WHAT ONLY THIS PROVES
# ---------------------
# Following was proven in parts: the judge against a table of cases, the
# registry over planted pins in evaluated machines
# (`distro-apps-follow-when-on-and-newer`), GitHub's answers recorded in the
# crate's tests. No run had joined them. Here one machine does, with a
# "GitHub" of the test's over HTTPS (`api.github.com` and `github.com`,
# answered by the other node under a certificate authority made for this
# test), and a tiny real AppImage in three releases:
#
#   1. the machine is installed with the distro's pin: the AppImage's part
#      arrives — its download fetched by the build from the release — and
#      the person runs it through the parts' view;
#   2. the person turns following on (their options, and the two hosts let,
#      as the update's page writes them): while GitHub's newest release IS
#      the distro's pin, nothing is pinned;
#   3. a newer release is published: the update's `person-pins` step asks
#      GitHub, judges the release, downloads it with `nix store
#      prefetch-file` under the name the distro's builder fetches it under
#      and checked against GitHub's digest, and writes the followed pin; the
#      person signs; `person-stage` hands it to root's applier, which takes
#      it under the pinned key; the machine's build takes root's copy and the
#      part is built at the followed release — FROM THE BYTES THE STEP
#      FETCHED, which the build never downloads again — and the person runs
#      the new version, the old one kept as the way back;
#   4. a release whose bytes are not its digest is never pinned: the step
#      says why, and the machine keeps the version it has;
#   5. a release whose asset is broken is pinned, signed and applied, and
#      its part does not build: the part keeps the last version that
#      arrived, the person still runs it, and its record says why — what
#      the update's page reads.
#
# WHAT STANDS IN, AND WHY
# -----------------------
# - THE MACHINE'S BUILD. A machine builds its whole system from the machine
#   directory (`ronoctua-machine apply`), which no VM does offline; what
#   decides a followed pin of it is the registry and the parts, so those two
#   modules are evaluated HERE, from root's copies in the machine directory
#   (`tests/distro-apps-follow-vm-machine.nix`), and their plan and report
#   put where the booted system carries them — the part is then the
#   realizer's, for real, with the planner started as a boot starts it. The
#   person's report (`person-applied.json`) is written from root's pin as
#   `nix/modules/person.nix` writes it. The apply itself is
#   `boot-trial-falls-back`'s.
# - THE SIGNATURE. `ronoctua-belongings sign` needs the person's repository
#   and the identity their session keeps; here the person's key signs the
#   mirror as the belongings sign it (`ronoctua-test-pq sign-tree`, the
#   `person-vm` precedent) and the pair is kept where the belongings keep
#   it, which is what the update's `person-stage` carries to root.
# - THE ASSET'S RUNTIME. nixpkgs carries no `appimagetool` and no AppImage
#   runtime: the images are a squashfs of an AppDir (`mksquashfs`) behind an
#   ELF header that says type 2 — all `appimageTools.extract` reads, and the
#   distro's wrapper runs the image unpacked, never its runtime.
#
# NOTHING IS DOWNLOADED from outside the test: the VM has no substituter,
# and the build's inputs are the ones a twin of the part was built with on
# the host (the machine file says why). Nothing is GitHub's but its shape.
{
  pkgs,
  lib,
  # `ronoctua-parts`, extracted (`rust.parts`).
  partsTool,
  # `ronoctua-person`, root's applier (`rust.person`).
  person,
  # `ronoctua-software`, the CLI the update's person half runs (`rust.software`).
  software,
  # `ronoctua-test-pq`: the person's signing key, and a tree signed with it.
  testPq,
}:

let
  user = "noctua";
  key = "fixture";
  repo = "fixture-labs/fixture";
  plain = builtins.unsafeDiscardStringContext;

  # THE TABLE THE TEST PLANTS: one AppImage of the distro that follows a
  # repository, in the base group so no profile switch is needed.
  table.${key} = {
    source = "appimage";
    inherit key;
    name = "Fixture";
    follows.github = {
      inherit repo;
      assetPattern = "fixture-*-linux-x86_64.AppImage";
    };
    fallback = false;
    group = "base";
  };
  asset = version: "fixture-${version}-linux-x86_64.AppImage";
  download = version: "/${repo}/releases/download/v${version}/${asset version}";

  # ---------------------------------------------------------------------
  # The releases
  # ---------------------------------------------------------------------
  # A TINY TYPE-2 APPIMAGE: an AppDir whose program says its version, with a
  # launcher entry and an icon, squashed and sealed behind an ELF header
  # whose padding says `AI` and type 2 (`appimage-exec.sh` reads exactly
  # those fields to find the squashfs).
  imageScript = pkgs.writeText "fixture-appimage.py" ''
    import os, pathlib, struct, subprocess, sys, zlib

    version, out = sys.argv[1], pathlib.Path(sys.argv[2])
    app = pathlib.Path("AppDir")
    (app / "usr/bin").mkdir(parents=True)

    def program(path, text):
        path.write_text(text)
        os.chmod(path, 0o755)

    program(app / "usr/bin/fixture", f'#!/bin/sh\necho "fixture {version}"\n')
    program(app / "AppRun", '#!/bin/sh\nexec "$APPDIR/usr/bin/fixture" "$@"\n')
    (app / "fixture.desktop").write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=Fixture upstream\n"
        "Comment=The program of a test\n"
        "Exec=fixture %F\n"
        "Icon=fixture\n"
        "Categories=Utility;\n"
    )

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    pixel = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    (app / "fixture.png").write_bytes(
        b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", pixel)
        + chunk(b"IDAT", zlib.compress(b"\x00\xff\x00\x00")) + chunk(b"IEND", b"")
    )
    os.symlink("fixture.png", app / ".DirIcon")

    subprocess.run(
        ["mksquashfs", "AppDir", "image.squashfs", "-quiet", "-noappend", "-all-root", "-no-xattrs"],
        check=True,
    )
    # An ELF64 header and one null section header: where the section
    # headers end is where the squashfs begins.
    ident = b"\x7fELF" + bytes([2, 1, 1, 0]) + b"AI\x02" + bytes(5)
    header = ident + struct.pack("<HHIQQQIHHHHHH", 2, 0x3E, 1, 0, 0, 64, 0, 64, 56, 0, 64, 1, 0)
    assert len(header) == 64
    out.write_bytes(header + bytes(64) + pathlib.Path("image.squashfs").read_bytes())
  '';
  imageOf =
    version:
    pkgs.runCommand "fixture-${version}.AppImage"
      {
        nativeBuildInputs = [
          pkgs.python3
          pkgs.squashfsTools
        ];
      }
      ''
        python3 ${imageScript} ${version} $out
      '';
  images = {
    "1.0.0" = imageOf "1.0.0";
    "2.0.0" = imageOf "2.0.0";
    # A RELEASE WHOSE ASSET IS BROKEN: an image cut short after its header,
    # as an upload that stopped would leave it.
    "3.0.0" = pkgs.runCommand "fixture-3.0.0.AppImage" { } ''
      head -c 192 ${imageOf "3.0.0"} > $out
    '';
  };

  # WHAT "GITHUB" SERVES, and the distro's pin of the first release: each
  # digest and hash measured here, by a build — never written by hand.
  served =
    pkgs.runCommand "fixture-github"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        images = builtins.toJSON images;
      }
      ''
        python3 - <<'PY'
        import base64, hashlib, json, os, pathlib, shutil

        out = pathlib.Path(os.environ["out"])
        images = json.loads(os.environ["images"])
        repo = "${repo}"

        def asset(version, arch="x86_64"):
            return f"fixture-{version}-linux-{arch}.AppImage"

        def url(version, name):
            return f"https://github.com/{repo}/releases/download/v{version}/{name}"

        def sha(version):
            return hashlib.sha256(pathlib.Path(images[version]).read_bytes()).digest()

        for version, path in images.items():
            at = out / "github" / repo / "releases/download" / f"v{version}"
            at.mkdir(parents=True)
            shutil.copyfile(path, at / asset(version))

        def release(version, digest):
            # The x86_64 image, the aarch64 one and the zsync beside it, as
            # a release of the distro's AppImages carries them: only the
            # first matches the table's pattern.
            def one(name, sha256):
                return {
                    "name": name,
                    "browser_download_url": url(version, name),
                    "size": pathlib.Path(images[version]).stat().st_size,
                    "digest": "sha256:" + sha256.hex(),
                }
            return {
                "tag_name": f"v{version}",
                "draft": False,
                "prerelease": False,
                "assets": [
                    one(asset(version), digest),
                    one(asset(version) + ".zsync", hashlib.sha256(b"zsync").digest()),
                    one(asset(version, "aarch64"), hashlib.sha256(b"aarch64").digest()),
                ],
            }

        api = out / "api"
        api.mkdir()
        answers = {
            "latest-1.json": release("1.0.0", sha("1.0.0")),
            "latest-2.json": release("2.0.0", sha("2.0.0")),
            # A DIGEST THAT IS NOT THE BYTES': the second release's.
            "latest-3-wrong-digest.json": release("3.0.0", sha("2.0.0")),
            "latest-3.json": release("3.0.0", sha("3.0.0")),
        }
        for name, body in answers.items():
            (api / name).write_text(json.dumps(body))

        # THE DISTRO'S PIN, as `nix run .#distro-pins` writes one.
        pins = {
            "format": 1,
            "appimage": {
                "${key}": {
                    "url": url("1.0.0", asset("1.0.0")),
                    "hash": "sha256-" + base64.b64encode(sha("1.0.0")).decode(),
                    "version": "1.0.0",
                    "pinned-at": "2026-10-31T00:00:00Z",
                }
            },
            "android": {},
        }
        (out / "distro-pins.json").write_text(json.dumps(pins))
        PY
      '';

  # A CERTIFICATE AUTHORITY MADE FOR THIS TEST, and one certificate for both
  # of GitHub's names, signed by it. The machine trusts it through
  # `security.pki` — the bundle `ronoctua_http::tls` and Nix read.
  hosts = [
    "api.github.com"
    "github.com"
  ];
  pki = pkgs.runCommand "ronoctua-follow-pki" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    cd $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=ronoctua test authority" \
      -keyout ca.key -out ca.crt
    openssl req -newkey rsa:2048 -nodes -subj "/CN=github.com" -keyout leaf.key -out leaf.csr
    printf 'subjectAltName=%s\n' "${lib.concatMapStringsSep "," (h: "DNS:${h}") hosts}" > san.ext
    openssl x509 -req -in leaf.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 3650 \
      -extfile san.ext -out leaf.crt
  '';

  # ---------------------------------------------------------------------
  # The machine's build, and the twin whose inputs it is given
  # ---------------------------------------------------------------------
  # THE DISTRO'S FILES the registry and the parts read — these, and not the
  # tree (`lean-tree`).
  distro = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../nix/modules/distro-apps.nix
      ../nix/modules/parts.nix
      ../nix/distro-pins.nix
      ../nix/distro-downloads.nix
      ../nix/declared-apps/android.nix
      ../nix/names.nix
      ../nix/parts-names.nix
      ../nix/parts-pkgs.nix
      ../nix/claude-code.nix
      ../assets/distro-apps.nix
    ];
  };
  # The nixpkgs SOURCE, named alike on both sides: the machine file imports
  # it plain, so no overlay of the flake's reaches either evaluation. By its
  # own store path, with that path as its context: interpolating the path
  # value would copy the whole tree into the store again.
  nixpkgs =
    let
      source = toString pkgs.path;
    in
    builtins.appendContext source { ${source}.path = true; };
  machineFile = ./distro-apps-follow-vm-machine.nix;

  # THE TWIN: the registry's own builder (`builders.appimage`) over a local
  # image, from the same evaluation the VM makes. Each derivation the VM
  # builds for a release is the twin's but for the image: what the twin's
  # were built FROM is what the VM is given.
  twinMachine = import machineFile {
    inherit nixpkgs distro table;
    partsTool = plain "${partsTool}";
    pins = {
      format = 1;
      appimage = { };
      android = { };
    };
  };
  twinPkgs = twinMachine.pkgs;
  twin = twinMachine.config.ronoctua.distroApps.builders.appimage {
    inherit key;
    name = "Fixture";
    version = "twin";
    src = images."1.0.0";
    fallback = false;
    program = (import ../nix/parts-names.nix).viewBin "ronoctua-appimage-${key}";
  };
  buildInputsOfTheParts = map (d: d.inputDerivation) [
    # `appimageOf`'s own `extract`, called as it calls it.
    (twinPkgs.appimageTools.extract {
      pname = "ronoctua-appimage-${key}";
      version = "twin";
      src = images."1.0.0";
    })
    twin.fhs
    twin.fhs.fhsenv
    twin.wrapper
    twin.entry
    twin.package
    # The download the distro's pin is fetched with (`nix/distro-downloads.nix`).
    (twinPkgs.fetchurl {
      url = "https://github.com/${repo}/releases/download/twin/${asset "twin"}";
      hash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
      name = "${key}-twin.AppImage";
    })
  ];

  # THE MACHINE'S BUILD, as the VM evaluates it: the machine file over the
  # machine directory. `problems` must stay empty, or the registry refused.
  vmMachine = pkgs.writeText "machine.nix" ''
    let
      present = path: if builtins.pathExists path then path else null;
      machine = import ${machineFile} {
        nixpkgs = "${nixpkgs}";
        distro = "${distro}";
        partsTool = "${partsTool}";
        table = builtins.fromJSON ${builtins.toJSON (builtins.toJSON table)};
        pins = builtins.fromJSON (builtins.readFile ${served}/distro-pins.json);
        options = present /etc/nixos/person/options.json;
        distroPins = present /etc/nixos/person/software/distro-pins.json;
      };
    in
    {
      plan = machine.config.environment.etc."ronoctua/parts-plan.json".text;
      report = machine.config.environment.etc."ronoctua/distro-apps.json".text;
      problems = machine.config.ronoctua.distroApps.problems;
    }
  '';

  # ---------------------------------------------------------------------
  # The update's steps, as the table declares them
  # ---------------------------------------------------------------------
  steps = (import ../crates/ronoctua-brain/modules/system-update/steps.nix).steps;
  # A STEP BY WHAT IT RUNS, never by its id: the sequence's ids live in its
  # declaration alone, and a check refuses one written anywhere else.
  stepRunning =
    words:
    let
      found = builtins.filter (s: lib.take (builtins.length words) (s.run or [ ]) == words) steps;
      s = builtins.head found;
    in
    assert lib.assertMsg (builtins.length found == 1 && (s.package or null) == "software" && !s.root)
      "the update has no one person's step of the software CLI running `${toString words}`: this test runs the wrong table";
    lib.escapeShellArgs ([ "/run/current-system/sw/bin/${s.program}" ] ++ s.run);

  # THE SOFTWARE CLI WRAPPED FOR THIS MACHINE, as `nix/modules/software.nix`
  # wraps it: the one table of its programs.
  softwareCli =
    let
      tools = import ../nix/software-tools.nix {
        inherit lib pkgs;
        nix = pkgs.nix;
        index = pkgs.writeTextDir "index.tsv" "";
        nixpkgsRev = "test";
      };
    in
    pkgs.runCommand "ronoctua-software" { nativeBuildInputs = [ pkgs.makeBinaryWrapper ]; } ''
      makeBinaryWrapper ${software}/bin/ronoctua-software $out/bin/ronoctua-software \
        ${lib.concatStringsSep " " (
          lib.mapAttrsToList (name: value: "--set-default ${name} ${lib.escapeShellArg value}") tools
        )}
    '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-distro-apps-follow";

  nodes.github = {
    networking.firewall.allowedTCPPorts = [ 443 ];
    systemd.tmpfiles.rules = [ "d /srv 0755 root root -" ];
    # THE FIRST ANSWER: the distro's own release is the newest.
    systemd.services.github-served = {
      wantedBy = [ "multi-user.target" ];
      before = [ "nginx.service" ];
      serviceConfig.Type = "oneshot";
      script = ''
        mkdir -p /srv/api
        cp -r ${served}/github /srv/github
        cp ${served}/api/latest-1.json /srv/api/latest.json
      '';
    };
    services.nginx = {
      enable = true;
      appendHttpConfig = ''
        log_format follow '$host $request_method "$request_uri" $status';
        server {
          listen 443 ssl;
          server_name api.github.com;
          ssl_certificate ${pki}/leaf.crt;
          ssl_certificate_key ${pki}/leaf.key;
          access_log /var/log/nginx/follow.log follow;
          default_type application/json;
          location = /repos/${repo}/releases/latest { alias /srv/api/latest.json; }
        }
        server {
          listen 443 ssl;
          server_name github.com;
          ssl_certificate ${pki}/leaf.crt;
          ssl_certificate_key ${pki}/leaf.key;
          access_log /var/log/nginx/follow.log follow;
          root /srv/github;
        }
      '';
    };
  };

  nodes.machine =
    { nodes, ... }:
    {
      imports = [
        (import ../nix/modules/parts.nix { tool = partsTool; })
        ../nix/modules/person-apply.nix
      ];

      # What the two modules read of `base.nix`, `machine.nix` and
      # `person.nix`, declared bare (the `person-vm` precedent).
      options.ronoctua.user = lib.mkOption {
        type = lib.types.str;
        default = user;
      };
      options.ronoctua.machine.dir = lib.mkOption {
        type = lib.types.str;
        default = "/etc/nixos";
      };
      options.ronoctua.person.tool = lib.mkOption { type = lib.types.package; };

      config = {
        ronoctua.person.tool = person;
        users.users.${user} = {
          isNormalUser = true;
          uid = 1000;
        };
        security.polkit.enable = true;

        virtualisation.memorySize = 3072;
        virtualisation.cores = 2;
        virtualisation.writableStore = true;
        # WHAT THE MACHINE'S BUILD NEEDS, and nothing of what it builds: the
        # twin's inputs, and the nixpkgs and distro files it evaluates
        # (named by `vmMachine`).
        virtualisation.additionalPaths = buildInputsOfTheParts;

        nix.settings = {
          experimental-features = [ "nix-command" ];
          substituters = lib.mkForce [ ];
        };

        # GITHUB'S TWO NAMES, pointed at the test's server, under the test's
        # authority.
        security.pki.certificateFiles = [ "${pki}/ca.crt" ];
        networking.hosts.${nodes.github.networking.primaryIPAddress} = hosts;

        environment.systemPackages = [
          softwareCli
          pkgs.jq
        ];
        environment.etc."ronoctua-test/machine.nix".source = vmMachine;

        # THE PLAN IS THE BUILD'S: the test starts the planner once it put
        # the machine's plan where the booted system carries it.
        systemd.services.ronoctua-parts-trigger.wantedBy = lib.mkForce [ ];
      };
    };

  testScript = ''
    import base64
    import json
    import shlex

    HOME = "/home/${user}"
    MIRROR = f"{HOME}/.local/state/ronoctua/person"
    SIGNED = f"{HOME}/.local/state/ronoctua/person-signed"
    RECORD = "/var/lib/ronoctua-parts/parts/${key}.json"
    PROGRAM = "ronoctua-appimage-${key}"
    PQ = "${testPq}/bin/ronoctua-test-pq"
    PINS_STEP = ${
      builtins.toJSON (stepRunning [
        "pins"
        "refresh"
      ])
    }
    STAGE_STEP = ${
      builtins.toJSON (stepRunning [
        "person"
        "stage"
      ])
    }


    def as_person(command):
        # THE PERSON WITH NO TERMINAL, as the update's user unit runs a step.
        return f"su - ${user} -c {shlex.quote(command)} < /dev/null"


    def put(path, text, mode="0644"):
        data = base64.b64encode(text.encode()).decode()
        machine.succeed(f"echo {data} | base64 -d > {path}.new && chmod {mode} {path}.new && mv -f {path}.new {path}")


    def record():
        return json.loads(machine.succeed(f"cat {RECORD}"))


    def runs():
        return machine.succeed(as_person(PROGRAM)).strip()


    def downloads(version):
        asked = github.succeed("cat /var/log/nginx/follow.log")
        wanted = 'github.com GET "${download "VERSION"}" 200'.replace("VERSION", version)
        return sum(1 for line in asked.splitlines() if line == wanted)


    def publish(answer):
        github.succeed(f"cp ${served}/api/{answer} /srv/api/latest.json")
        # SIX HOURS LATER: the answer the last look kept is no longer fresh.
        machine.succeed(f"rm -rf {HOME}/.cache/ronoctua/software/github")


    def refresh():
        code, said = machine.execute(as_person(PINS_STEP) + " 2>&1")
        print(said)
        assert code == 0, f"the update's person-pins step ended {code}"
        return said


    def followed():
        try:
            return json.loads(machine.succeed(f"cat {MIRROR}/software/distro-pins.json"))["pins"]
        except Exception:
            return None


    def sign(counter):
        # WHAT `ronoctua-belongings sign` DOES, with the person's key: the
        # mirror signed as it is now, the pair kept where the belongings keep
        # it — what the update's `person-stage` carries to root.
        machine.succeed(as_person(
            "rm -rf /tmp/signing && mkdir -p /tmp/signing && "
            + f"cp -r {MIRROR} /tmp/signing/person && "
            + f"{PQ} sign-tree {HOME}/signing.key /tmp/signing {counter} && "
            + f"install -d -m 0700 {SIGNED} && "
            + f"install -m 0600 /tmp/signing/signed.json /tmp/signing/signed.sig {SIGNED}/"
        ))


    def stage():
        code, said = machine.execute(as_person(STAGE_STEP) + " 2>&1")
        print(said)
        assert code == 0, f"the update's person-stage step ended {code}"
        verdict = json.loads(machine.succeed("cat /run/ronoctua-person/verdict.json"))
        assert verdict["outcome"] == "applied", verdict
        assert verdict["kept-back"] == [], verdict
        return verdict


    def build():
        """THE MACHINE'S BUILD: the registry and the parts evaluated from the
        machine directory, their plan and report put where the booted
        system carries them, the person's report written from root's pin,
        and the planner started as that boot's trigger starts it."""
        code, said = machine.execute(
            "nix-instantiate --read-write-mode --eval --strict --json /etc/ronoctua-test/machine.nix 2>/tmp/eval.log"
        )
        assert code == 0, machine.succeed("cat /tmp/eval.log")
        out = json.loads(said)
        assert out["problems"] == [], out["problems"]
        machine.succeed("mkdir -p /etc/ronoctua && rm -f /etc/ronoctua/parts-plan.json /etc/ronoctua/distro-apps.json")
        put("/etc/ronoctua/parts-plan.json", out["plan"])
        put("/etc/ronoctua/distro-apps.json", out["report"])
        signer = json.loads(machine.succeed("cat /etc/nixos/local/signer.json"))
        put("/etc/ronoctua/person-applied.json", json.dumps({
            "format": 1,
            "machine": "machine",
            "distro-rev": "test",
            "applied": [],
            "parts": {},
            "skipped": [],
            "declined": {"honoured": [], "refused": [], "unknown": [], "still-present": []},
            "brain": {"included": [], "awaiting-approval": []},
            "signature": {
                "pinned": signer["fingerprint"],
                "counter": signer["counter"],
                "refused": signer.get("refused"),
                "kept-back": signer.get("kept-back", []),
            },
        }))
        plan = json.loads(out["plan"])["parts"]["${key}"]
        report = json.loads(out["report"])["apps"][0]["appimage"]
        # WHAT THE BUILD WILL BUILD — a download included — asked before it
        # runs.
        before = machine.succeed(f"nix-store --realise --dry-run {plan['drv']} 2>&1")
        print(before)
        machine.succeed("systemctl start ronoctua-parts.service")
        return plan, report, before


    start_all()
    github.wait_for_unit("nginx.service")
    github.wait_for_open_port(443)
    machine.wait_for_unit("multi-user.target")

    with subtest("the machine is installed with the distro's pin, and the person runs it"):
        # THE PERSON'S SIGNING KEY, pinned as the installer pins it.
        key = machine.succeed(as_person(f"umask 077 && {PQ} signing-key {HOME}/signing.key")).strip()
        machine.succeed("mkdir -p /etc/nixos")
        put("/root/signer.pub", key + "\n", mode="0600")
        machine.succeed("${person}/bin/ronoctua-person pin --pin /root/signer.pub --machine /etc/nixos")
        plan, report, before = build()
        assert report["pin"] == "distro" and report["version"] == "1.0.0", report
        # THE DISTRO'S PIN IS DOWNLOADED BY THE BUILD (`nix/distro-downloads.nix`).
        assert "-${key}-1.0.0.AppImage.drv" in before, before
        r = record()
        assert r["phase"] == "here" and r["current"]["out"] == plan["out"], r
        assert runs() == "fixture 1.0.0", runs()
        # ITS LAUNCHER, through the view: the table's name, the view's path,
        # never opening a file by name order.
        entry = machine.succeed(
            f"cat /var/lib/ronoctua-parts/view/share/applications/{PROGRAM}.desktop"
        )
        print(entry)
        assert "Name=Fixture\n" in entry, entry
        assert f"Exec=/var/lib/ronoctua-parts/view/bin/{PROGRAM} %F\n" in entry, entry
        assert "X-Ronoctua-Fallback=false" in entry, entry
        machine.succeed(f"test -e /var/lib/ronoctua-parts/view/share/pixmaps/{PROGRAM}.png")
        # The distro's download was fetched by the build, once.
        assert downloads("1.0.0") == 1, github.succeed("cat /var/log/nginx/follow.log")
        base = plan

    with subtest("following is turned on; GitHub's newest is the distro's pin, and nothing is pinned"):
        # AS THE UPDATE'S PAGE WRITES THEM (`ronoctua-shell-update/src/follow.rs`):
        # the person's options, and the two hosts let in their consent.
        machine.succeed(as_person(
            f"mkdir -p {MIRROR}/software {HOME}/.config/ronoctua && "
            + f"printf '%s' '{json.dumps({'format': 1, 'ssh-signing': None, 'distro-apps-follow-releases': True})}' "
            + f"> {MIRROR}/options.json && "
            + f"printf '%s' '{json.dumps({'hosts': {'api.github.com': True, 'github.com': True}})}' "
            + f"> {HOME}/.config/ronoctua/consent.json"
        ))
        refresh()
        assert followed() is None, followed()
        asked = github.succeed("cat /var/log/nginx/follow.log")
        assert 'api.github.com GET "/repos/${repo}/releases/latest" 200' in asked, asked

    with subtest("a newer release is followed, signed, applied by root, built from the bytes the step fetched, and run"):
        publish("latest-2.json")
        refresh()
        pins = followed()
        assert pins is not None and len(pins) == 1, pins
        pin = pins[0]
        assert pin["id"] == "${key}" and pin["version"] == "2.0.0", pin
        assert pin["url"] == "https://github.com${download "2.0.0"}", pin
        assert downloads("2.0.0") == 1, github.succeed("cat /var/log/nginx/follow.log")
        sign(1)
        stage()
        applied = json.loads(machine.succeed("cat /etc/nixos/person/software/distro-pins.json"))
        assert applied["pins"] == pins, applied
        owner = machine.succeed("stat -c %U:%a /etc/nixos/person/software/distro-pins.json").strip()
        assert owner == "root:644", owner
        signer = json.loads(machine.succeed("cat /etc/nixos/local/signer.json"))
        assert signer["counter"] == 1 and signer["kept-back"] == [], signer
        plan, report, before = build()
        assert report["pin"] == "followed" and report["version"] == "2.0.0", report
        assert report["followed-version"] == "2.0.0" and report["distro-version"] == "1.0.0", report
        # THE STEP'S DOWNLOAD IS THE BUILD'S SOURCE: the part is built, and
        # its download is not — the step fetched it under the builder's name,
        # and GitHub was asked for it once, by the step.
        assert "-ronoctua-appimage-${key}-2.0.0.drv" in before, before
        assert "-${key}-2.0.0.AppImage.drv" not in before, before
        r = record()
        assert r["phase"] == "here" and r["current"]["out"] == plan["out"], r
        assert r["previous"]["out"] == base["out"], r
        assert runs() == "fixture 2.0.0", runs()
        assert downloads("2.0.0") == 1, github.succeed("cat /var/log/nginx/follow.log")
        good = plan

    with subtest("a release whose bytes are not its digest is never pinned, and the step says why"):
        publish("latest-3-wrong-digest.json")
        said = refresh()
        assert "hash mismatch" in said, said
        assert followed() == pins, followed()
        assert runs() == "fixture 2.0.0", runs()

    with subtest("a broken release is pinned and applied, its part holds the last that arrived, and says why"):
        publish("latest-3.json")
        refresh()
        pin = followed()[0]
        assert pin["version"] == "3.0.0", pin
        sign(2)
        stage()
        plan, report, before = build()
        assert report["pin"] == "followed" and report["version"] == "3.0.0", report
        assert "-${key}-3.0.0.AppImage.drv" not in before, before
        r = record()
        print(json.dumps(r, indent=2))
        assert r["current"]["out"] == good["out"], r
        assert r["wanted"]["out"] == plan["out"], r
        assert r["last-failure"]["why"] == "build-failed", r
        assert "fixture-3.0.0" in r["last-failure"]["detail"], r
        # NOT A DOWNLOAD THAT FAILED: the bytes are here, and they are broken.
        assert r.get("missing", []) == [], r
        machine.fail("systemctl is-failed ronoctua-part@${key}.service")
        assert runs() == "fixture 2.0.0", runs()
  '';
}
