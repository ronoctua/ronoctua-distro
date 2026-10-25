# VS CODE ON THE DISTRO'S COMPOSITOR: its telemetry off by the machine's
# policy, a Wayland window of its own, the person's extensions recorded,
# installed for real from a stand-in Marketplace — and why it runs in its FHS
# environment.
#
# WHAT ONLY THE VM PROVES
# -----------------------
#   1. VS Code — the distro's, in its FHS environment (`nix/vscode.nix`) —
#      started as the session starts it (`NIXOS_OZONE_WL=1`,
#      `XDG_SESSION_TYPE=wayland` and a `WAYLAND_DISPLAY`), takes the policy
#      the module writes (`/etc/vscode/policy.json`, bound into the
#      environment): its main log, at trace level, says
#      `PolicyConfiguration#changed` with `telemetry.telemetryLevel` `off` —
#      over a person's `settings.json` that asks for `all`; and the same start
#      with the file moved aside says nothing of the kind;
#   2. its window is the compositor's own (`xdg_shell`, `app_id` `code`),
#      never an X window through XWayland;
#   3. the person's user path unit waits, active and not failed, on a
#      manifest whose folder does not exist yet; VS Code's manifest written
#      IN PLACE, as VS Code writes it, fires it, and the list follows the
#      rule — a new install enters, an uninstall here leaves, another
#      machine's id stays, an id another machine took out stays out though
#      this machine has it; an extension installed from a file never enters;
#   4. `ronoctua-vscode install`, the person's command on the `PATH`, asks
#      nothing while the Marketplace's hosts are not let, and says which;
#   5. the queue's child (`ronoctua-belongings vscode-extensions`), with the
#      hosts let, installs for real — VS Code's own `code`, from a stand-in
#      Marketplace (`tests/vscode-gallery.py`, reached by the real host names,
#      its certificate signed by a CA this machine trusts) — a pinned VSIX
#      (rust-analyzer 2026-10-12, linux-x64) enters, VS Code writes its
#      manifest, the record follows, and an id the Marketplace does not have
#      fails with VS Code's own words; every host VS Code reached is one the
#      install minted a pass for;
#   6. rust-analyzer's own server — a program linked for other distributions
#      (`/lib64/ld-linux-x86-64.so.2`) — is refused on the machine by NixOS's
#      stub loader (127); with the plain package the extension cannot run it
#      ("Failed to execute … --version") and no server starts; in the FHS
#      environment it runs: why the distro installs that one. (Two harnesses
#      of the measure's own are in the script, where it opens the project.)
#   7. in the distro's environment, which says `ID=linux` (`nix/vscode.nix`,
#      the rest of the machine's `os-release` kept), rust-analyzer runs the
#      server it ships — no setting, no network, nothing copied or patched —
#      as the person's session starts VS Code (a unit of their manager, with
#      its environment);
#   8. its own NixOS patch, in the environment as it was (`ID=nixos`), on a
#      machine whose Nix is an installed one's (`NIX_PATH` from
#      `nixosSystem`, the registries of `nix/modules/nix-registries.nix`, the
#      toolchain kept): with no network and a cold cache it builds, the
#      global flake registry being the store's; with the network, Nix reaches
#      the binary cache and never `channels.nixos.org`;
#   9. pointed at the toolchain's `rust-analyzer` (`rust-analyzer.server.path`,
#      the alternative `docs/INSTALL.md` gives), the server that runs is the
#      toolchain's, and nothing is copied or patched;
#  10. the built-in Copilot Chat, which VS Code updates from the Marketplace
#      by itself (the stand-in offers a newer one), never enters the person's
#      list;
#  11. at start-up — a fresh profile, no extension of the person's, the
#      telemetry off by the policy, every name sent to this machine (a name
#      server that answers each with 127.0.0.1, the stand-in writing down
#      every TLS greeting and answering the whole conversation, the
#      window's cross-origin preflights included) — VS Code reaches only its CDN, the Marketplace
#      and its asset hosts (the built-in Copilot Chat, updated to the end), and
#      never Google's dictionary redirector nor its mirror, which the stand-in
#      answers too: the distro's `code` maps the redirector to nothing
#      (`nix/vscode.nix`, Chromium's `--host-resolver-rules`), whose switch
#      the program that runs carries — the launcher's entry, the person's
#      `code` and `ronoctua-vscode`'s being that one; `chat.disableAIFeatures`
#      leaves the Copilot Chat question as it is; `extensions.autoCheckUpdates`
#      off asks the Marketplace nothing until a file the product recommends an
#      extension for opens, and `extensions.ignoreRecommendations` quiets that.
#
# VS Code's window runs as root on the compositor (with `--no-sandbox` and a
# data folder of its own, which VS Code demands of root), because the
# compositor of `compositor-xvfb.nix` is root's; the policy is the machine's,
# read the same for every user. Steps 3 to 5 are the person's, under their
# own manager (`linger`).
#
# THE HARNESSES, and nothing else: the CA is handed to
# `code` by `NODE_EXTRA_CA_CERTS` as well as the machine's bundle (the real
# Marketplace's certificates are public); and the person's settings say
# `"extensions.verifySignature": false` for the install, as the start-up
# measure's profiles do for the Copilot Chat update — the Marketplace signs
# every public extension, and the pinned 1.119 refuses one without a
# signature ("Signature verification failed with 'NotSigned' error",
# measured against this stand-in), which no one but Microsoft can make. The
# person's VS Code runs on an X display of its own, Xvfb; and opening a
# project skips the trust question (`--disable-workspace-trust`), which a
# person answers once, by hand.
#
# The settings at `telemetry.telemetryLevel` are kept in the test's output
# (`settings.png`), for the eye: what VS Code draws for a managed setting.
{
  pkgs,
  lib,
  scroll,
  fonts,
  vscodeTool,
  belongingsTool,
  # `ronoctua-parts`, extracted (`rust.parts`): VS Code is a part.
  partsTool,
  # The flake's own nixpkgs, which an installed machine's registry locks.
  nixpkgsInput,
  ronoctua,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  # VS CODE, ADMITTED BY NAME and alone — it and its FHS environment (`code`):
  # the `pkgs` a test is handed cannot admit an unfree package.
  unfree = import pkgs.path {
    inherit (pkgs.stdenv.hostPlatform) system;
    config.allowUnfreePredicate =
      p:
      lib.elem (lib.getName p) [
        "vscode"
        "code"
      ];
  };
  # THE DISTRO'S TOOLCHAIN, whose `rust-analyzer` the registry's `rust` entry
  # puts on the PATH of both profiles (`nix/modules/dev.nix`).
  toolchain = import ../nix/toolchain.nix { inherit pkgs; };
  # THE DISTRO'S (the FHS environment), and for the measures the plain one
  # and the environment as it was before it said `ID=linux` — the one
  # rust-analyzer's own NixOS patch runs in.
  vscodeFhs = import ../nix/vscode.nix { pkgs = unfree; };
  vscodeFhsNixos = import ../nix/vscode.nix {
    pkgs = unfree;
    nixosId = true;
  };
  vscodePlain = unfree.vscode;

  # THE PINNED EXTENSION: rust-analyzer's own VSIX for linux-x64, whose
  # server is a program linked for other distributions.
  rustAnalyzer = pkgs.fetchurl {
    url = "https://github.com/rust-lang/rust-analyzer/releases/download/2026-10-12/rust-analyzer-linux-x64.vsix";
    hash = "sha256-/VR9Si34tElTexo4lqGMEaNT8OxEmsU0cVUcoGlyO8I=";
  };
  # A COPILOT CHAT NEWER THAN THE ONE THE PINNED BUILD CARRIES — its own
  # version with the last number one up, its own engine —, as the Marketplace
  # offers one: VS Code updates its built-in from it, which is the
  # conversation the start-up measure follows to the end, and the update the
  # person's list must never take for theirs.
  copilotChat =
    pkgs.runCommand "github.copilot-chat-newer.vsix" { nativeBuildInputs = [ pkgs.python3 ]; }
      ''
        python3 - ${vscodePlain}/lib/vscode/resources/app/extensions/copilot/package.json "$out" <<'EOF'
        import json, sys, zipfile
        built_in = json.load(open(sys.argv[1]))
        major, minor, patch = built_in["version"].split(".")[:3]
        version = f"{major}.{minor}.{int(patch) + 1}"
        package = {"name": built_in["name"], "publisher": built_in["publisher"], "version": version,
                   "engines": {"vscode": built_in["engines"]["vscode"]},
                   "displayName": built_in.get("displayName", built_in["name"])}
        manifest = ('<?xml version="1.0" encoding="utf-8"?>'
                    '<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">'
                    f'<Metadata><Identity Language="en-US" Id="{package["name"]}" Version="{version}" '
                    f'Publisher="{package["publisher"]}"/></Metadata></PackageManifest>')
        with zipfile.ZipFile(sys.argv[2], "w") as z:
            z.writestr("extension.vsixmanifest", manifest)
            z.writestr("extension/package.json", json.dumps(package))
        EOF
      '';
  vsixes = pkgs.linkFarm "vscode-gallery-vsix" [
    {
      name = "rust-lang.rust-analyzer.vsix";
      path = rustAnalyzer;
    }
    {
      name = "github.copilot-chat.vsix";
      path = copilotChat;
    }
  ];

  # THE STAND-IN'S CERTIFICATE, for every name it answers, and the CA that
  # signs it (this test's own; the machine trusts it).
  galleryNames = [
    "marketplace.visualstudio.com"
    "www.vscode-unpkg.net"
    "main.vscode-cdn.net"
    "rust-lang.gallery.vsassets.io"
    "rust-lang.gallerycdn.vsassets.io"
    "nope.gallery.vsassets.io"
    "nope.gallerycdn.vsassets.io"
    "github.gallery.vsassets.io"
    "github.gallerycdn.vsassets.io"
  ];
  # AND THE NAMES ONLY THE MEASURES REACH, by the name server the script
  # starts (the hosts file never names them): Nix's two, for the patch on an
  # online machine, and Google's two that VS Code's Electron would ask at
  # start-up without the distro's rule (the dictionary's redirector and the
  # mirror it sends to) — so a request that slips past the rule is heard,
  # and not only its name.
  measuredNames = [
    "channels.nixos.org"
    "cache.nixos.org"
    "redirector.gvt1.com"
    "r1---sn-ronoctua.gvt1.com"
  ];
  certs = pkgs.runCommand "vscode-gallery-certs" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -sha256 \
      -subj "/CN=ronoctua test gallery CA" \
      -addext "basicConstraints=critical,CA:TRUE" \
      -addext "keyUsage=critical,keyCertSign,cRLSign" \
      -keyout ca.key -out $out/ca.crt
    openssl req -newkey rsa:2048 -nodes -sha256 -subj "/CN=marketplace.visualstudio.com" \
      -keyout $out/server.key -out server.csr
    printf '%s\n' \
      "basicConstraints=CA:FALSE" \
      "keyUsage=critical,digitalSignature,keyEncipherment" \
      "extendedKeyUsage=serverAuth" \
      "subjectAltName=${
        lib.concatMapStringsSep "," (n: "DNS:${n}") (galleryNames ++ measuredNames)
      }" > server.ext
    openssl x509 -req -in server.csr -CA $out/ca.crt -CAkey ca.key -CAcreateserial \
      -days 3650 -sha256 -extfile server.ext -out $out/server.crt
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-vscode";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        (import ../nix/modules/vscode.nix { inherit vscodeTool; })
        # VS CODE IS A PART (not free): the parts' module realizes it — here
        # SEEDED, its output carried by the machine (`additionalPaths`).
        (import ../nix/modules/parts.nix { tool = partsTool; })
        # THE MACHINE'S REGISTRIES, from the module every machine has them
        # from: `nixpkgs` locked to the system's, the global one the store's.
        ../nix/modules/nix-registries.nix
      ];
      _module.args.ronoctua = ronoctua;
      _module.args.inputs.nixpkgs = nixpkgsInput;

      virtualisation.memorySize = 4096;
      virtualisation.cores = 2;
      virtualisation.diskSize = 4096;
      networking.hostName = lib.mkForce "noctua-vscode";
      users.users.root.hashedPassword = lib.mkForce null;
      # THE PERSON'S MANAGER AT BOOT: the path unit is theirs.
      users.users.${config.ronoctua.user}.linger = true;

      ronoctua.editor.vscode = true;
      ronoctua.editor.vscodePackage = vscodeFhs;
      # THE PART'S OUTPUT, carried: the realizer sets its profile with no
      # derivation and no download, as an ISO's part is.
      virtualisation.additionalPaths = [ config.ronoctua.parts.vscode.package ];

      # THE STAND-IN MARKETPLACE, by the real names, over TLS.
      networking.hosts."127.0.0.1" = galleryNames;
      security.pki.certificateFiles = [ "${certs}/ca.crt" ];
      systemd.services.vscode-gallery = {
        description = "A stand-in for the Visual Studio Marketplace";
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${./vscode-gallery.py} ${vsixes} ${certs}/server.crt ${certs}/server.key /var/log/vscode-gallery.jsonl /var/log/vscode-sni.jsonl";
      };
      # EVERY NAME IS THIS MACHINE, for the start-up measure only: started by
      # the script when it measures, it answers every name with 127.0.0.1 and
      # writes each one down — so every host VS Code tries reaches the
      # stand-in, which writes down the name of each TLS greeting. Stopped,
      # a lookup fails at once, as with no network at all.
      networking.nameservers = [ "127.0.0.1" ];
      systemd.services.vscode-dns = {
        description = "Answers every name with this machine and writes each one down";
        serviceConfig.ExecStart = "${pkgs.dnsmasq}/bin/dnsmasq --keep-in-foreground --no-resolv --no-hosts --address=/#/127.0.0.1 --listen-address=127.0.0.1 --bind-interfaces --log-queries --log-facility=-";
      };
      # NOTHING ELSE OF THE MACHINE'S ASKS A NAME while it measures.
      services.timesyncd.enable = false;
      # NO CORE IS KEPT: VS Code stopped by its unit leaves on a trap, and
      # a core of each start filled this machine's disk (measured: "No space
      # left on device", the collector itself at 3 GiB of memory).
      systemd.coredump.settings.Coredump = {
        Storage = "none";
        ProcessSizeMax = 0;
      };

      # AND THE REST OF AN INSTALLED MACHINE'S NIX — what rust-analyzer's own
      # NixOS patch meets: `nixosSystem` sets `nixpkgs.flake.source`, so the
      # session's `NIX_PATH` is `nixpkgs=flake:nixpkgs` (no channel is ever
      # fetched); and every machine keeps the devshell's toolchain, its
      # stdenv the very one `import <nixpkgs> {}` makes (`nix/modules/dev.nix`
      # keeps this same list). A test's machine has neither by itself.
      nixpkgs.flake.source = nixpkgsInput.outPath;
      system.extraDependencies = toolchain.kept;

      environment.systemPackages = [
        scroll
        pkgs.procps
        pkgs.imagemagick
        pkgs.file
        pkgs.python3
        # THE RUST THE REGISTRY PUTS ON THE PATH, rust-analyzer in it.
        toolchain.rust
      ]
      ++ xvfb.packages;

      fonts.enableDefaultPackages = false;
      fonts.packages = [ fonts.noctua-iosevka-core ];
      hardware.graphics.enable = true;
      services.dbus.enable = true;
    };

  testScript =
    { nodes, ... }:
    let
      machine = nodes.machine;
    in
    ''
      ${xvfb.prelude}
      import json
      import shlex

      user = "${machine.ronoctua.user}"
      home = "${machine.users.users.${machine.ronoctua.user}.home}"
      code = "${vscodeFhs}/bin/code"
      plain = "${vscodePlain}/bin/code"
      belongings = "${belongingsTool}/bin/ronoctua-belongings"
      ca = "${certs}/ca.crt"
      list = f"{home}/.config/Code/User/ronoctua-extensions.json"
      manifest = f"{home}/.vscode/extensions/extensions.json"
      gallery_log = "/var/log/vscode-gallery.jsonl"
      sni_log = "/var/log/vscode-sni.jsonl"


      def as_person(command, check=True):
          uid = machine.succeed(f"id -u {user}").strip()
          wrapped = (
              f"runuser -u {user} -- env HOME={home} XDG_RUNTIME_DIR=/run/user/{uid} "
              # THE SESSION'S PATH, the parts' view after the system's (VS
              # Code is a part, `nix/modules/parts.nix`).
              f"PATH=/run/current-system/sw/bin:/var/lib/ronoctua-parts/view/bin sh -c "
              + shlex.quote(command)
          )
          return machine.succeed(wrapped) if check else machine.execute(wrapped)


      def manager(args):
          return machine.succeed(f"systemctl --user --machine={user}@ {args}")


      def entry(ident, source="gallery"):
          """One entry of VS Code's manifest, shaped as VS Code writes it."""
          folder = f"{ident}-1.0.0"
          place = f"{home}/.vscode/extensions/{folder}"
          return {
              "identifier": {"id": ident, "uuid": "8d4c2c27-0000-4000-8000-000000000000"},
              "version": "1.0.0",
              "location": {"$mid": 1, "fsPath": place, "path": place, "scheme": "file"},
              "relativeLocation": folder,
              "metadata": {"installedTimestamp": 1790000000000, "source": source,
                           "id": "8d4c2c27-0000-4000-8000-000000000000",
                           "publisherDisplayName": "someone", "targetPlatform": "undefined",
                           "isPreReleaseVersion": False, "hasPreReleaseVersion": False,
                           "preRelease": False, "pinned": False, "updated": False,
                           "private": False},
          }


      def write_manifest(entries):
          """VS Code's manifest, written IN PLACE — as VS Code writes it."""
          text = json.dumps(entries)
          as_person(
              f"mkdir -p {home}/.vscode/extensions && printf '%s' {shlex.quote(text)} > {manifest}"
          )


      def write_list(ids):
          text = json.dumps({"format": 1, "extensions": ids})
          as_person(f"mkdir -p {home}/.config/Code/User && printf '%s' {shlex.quote(text)} > {list}")


      def listed():
          return json.loads(machine.succeed(f"cat {list}"))["extensions"]


      def settled(want):
          machine.wait_until_succeeds(
              f"python3 -c 'import json,sys; sys.exit(json.load(open(\"{list}\"))[\"extensions\"] != {json.dumps(want)})'",
              timeout=60,
          )


      def logged(data):
          """What VS Code's main process logged under its data folder — a
          command that reads nothing (never the driver's stdin) while there is
          no log yet."""
          return f"find {data}/logs -name main.log -exec cat {{}} +"


      def main_log(data):
          return machine.succeed(logged(data) + " 2>/dev/null || true")


      def start_code(unit, data, program=code, *extra):
          """VS Code as the session starts it, on the compositor, as root:
          with the session's `XDG_SESSION_TYPE`, which is what Electron's
          `auto` platform reads (without it, measured, it takes X11 and, with
          no X display, ends)."""
          args = " ".join(extra)
          machine.succeed(
              f"systemd-run --unit={unit} ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 --setenv=NIXOS_OZONE_WL=1 "
              "--setenv=XDG_SESSION_TYPE=wayland --setenv=HOME=/root "
              f"{program} --verbose --log trace --no-sandbox --disable-gpu "
              f"--user-data-dir {data} --skip-welcome --skip-release-notes {args}"
          )


      machine.start()
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit("vscode-gallery.service")
      # THE PART ARRIVES, seeded, and the session's view holds its `code`.
      machine.wait_until_succeeds(
          "test -x /nix/var/nix/profiles/ronoctua/parts/vscode/v1/bin/code"
          " && test -e /var/lib/ronoctua-parts/view/bin/code",
          timeout=300,
      )

      with subtest("the policy is the machine's, where the pinned build reads it"):
          policy = json.loads(machine.succeed("cat /etc/vscode/policy.json"))
          assert policy == {"TelemetryLevel": "off"}, policy

      with subtest("the launcher's entry, the person's `code` and their command all run the environment's"):
          # THE ONE `code` THAT CARRIES THE RULE (`nix/vscode.nix`): what the
          # launcher's entries name (by the `PATH`), what a terminal finds,
          # and what `ronoctua-vscode` was built with.
          fhs = machine.succeed(f"readlink -f {code}").strip()
          found = as_person("readlink -f $(command -v code)").strip()
          assert found == fhs, (found, fhs)
          entries = machine.succeed(
              "grep -h '^Exec=' /var/lib/ronoctua-parts/view/share/applications/code.desktop"
              " /var/lib/ronoctua-parts/view/share/applications/code-url-handler.desktop"
          ).splitlines()
          print(entries)
          assert entries and all(e.startswith("Exec=code ") for e in entries), entries
          command = as_person("readlink -f $(command -v ronoctua-vscode)").strip()
          machine.succeed(f"grep -aF '{code}' {command} > /dev/null")

      with subtest("the record waits on a folder that is not there, and does not fail"):
          machine.wait_until_succeeds(
              f"systemctl --user --machine={user}@ is-active ronoctua-vscode-record.path", timeout=120
          )
          machine.fail(f"test -e {home}/.vscode")
          said = manager("show ronoctua-vscode-record.service -p Result -p ActiveState")
          assert "Result=success" in said, said
          machine.fail(f"test -e {list}")

      with subtest("a manifest written in place records the person's list, Marketplace ids only"):
          write_manifest([entry("Golang.go"), entry("ms-python.python"), entry("local.hand-made", "vsix")])
          machine.wait_until_succeeds(f"test -e {list}", timeout=60)
          settled(["golang.go", "ms-python.python"])

      with subtest("another machine's id stays; one it took out stays out; an uninstall here leaves"):
          # ANOTHER MACHINE'S LIST LANDS: `far.away` added there, `ms-python`
          # taken out there — though this machine still has it.
          write_list(["far.away", "golang.go"])
          machine.sleep(3)
          settled(["far.away", "golang.go"])
          write_manifest([entry("ms-python.python")])
          settled(["far.away"])
          said = manager("show ronoctua-vscode-record.path -p ActiveState -p Result")
          assert "ActiveState=active" in said and "Result=success" in said, said

      with subtest("the command asks nothing while the Marketplace's hosts are not let, and says which"):
          write_list(["nope.missing", "rust-lang.rust-analyzer"])
          status, out = as_person("RONOCTUA_VSCODE= ronoctua-vscode install 2>&1", check=False)
          print(out)
          assert status == 1, status
          assert "refused not-let" in out, out
          hosts = [line.strip() for line in out.splitlines() if line.startswith("  ")]
          assert "marketplace.visualstudio.com" in hosts, hosts
          assert "rust-lang.gallery.vsassets.io" in hosts, hosts
          machine.fail(f"test -s {gallery_log}")

      with subtest("the queue's child installs for real, with the hosts let, and says what failed"):
          consent = json.dumps({"hosts": {h: True for h in hosts}})
          as_person(
              f"mkdir -p {home}/.config/ronoctua && printf '%s' {shlex.quote(consent)} > {home}/.config/ronoctua/consent.json"
          )
          # THE STAND-IN CANNOT SIGN (the header): only here.
          as_person(
              f"printf '%s' '{{\"extensions.verifySignature\": false}}' > {home}/.config/Code/User/settings.json"
          )
          status, out = as_person(
              f"RONOCTUA_VSCODE={code} NODE_EXTRA_CA_CERTS={ca} {belongings} vscode-extensions --progress 2>&1",
              check=False,
          )
          print(out)
          print(machine.execute(f"cat {gallery_log}")[1])
          done = json.loads(machine.succeed(f"cat {home}/.local/state/ronoctua/belongings/offer-extensions.json"))
          print(done)
          assert done["state"] == "done", done
          assert done["entered"] == ["rust-lang.rust-analyzer"], done
          assert [f["id"] for f in done["failed"]] == ["nope.missing"], done
          assert "not found" in done["failed"][0]["said"].lower(), done
          assert "BELONGINGS\tstep\tinstalling\t0\t2" in out, out
          assert status != 0, "a run with a failure ended as done"
          installed = json.loads(machine.succeed(f"cat {manifest}"))
          assert any(e["identifier"]["id"].lower() == "rust-lang.rust-analyzer" for e in installed), installed
          settled(["nope.missing", "rust-lang.rust-analyzer"])
          seen = json.loads(machine.succeed(f"cat {home}/.local/state/ronoctua/vscode-seen.json"))
          assert "rust-lang.rust-analyzer" in seen["extensions"], seen

      with subtest("every host VS Code reached is one the install let"):
          requests = [json.loads(l) for l in machine.succeed(f"cat {gallery_log}").splitlines()]
          reached = sorted({r["host"] for r in requests})
          print(f"the hosts VS Code reached: {reached}")
          assert reached, "the stand-in heard nothing"
          assert set(reached) <= set(hosts), f"{sorted(set(reached) - set(hosts))} were not let"

      with subtest("rust-analyzer's server cannot run on the machine itself"):
          server = machine.succeed(
              f"ls -d {home}/.vscode/extensions/rust-lang.rust-analyzer-*/server/rust-analyzer"
          ).strip()
          print(machine.succeed(f"file -L {server}"))
          status, out = as_person(f"{server} --version 2>&1", check=False)
          print(f"the server, run here: {status} {out!r}")
          assert status != 0, "the server ran without its interpreter"

      start_xvfb()
      start_compositor("compositor", "${scroll}")
      sock = socket_of("compositor")

      with subtest("VS Code takes the policy over the person's own setting, in its environment"):
          machine.succeed("mkdir -p /root/code-on/User")
          machine.succeed(
              "printf '%s' '{\"telemetry.telemetryLevel\": \"all\"}' > /root/code-on/User/settings.json"
          )
          start_code("code-on", "/root/code-on")
          try:
              # NO `grep -q` AT THE END OF A PIPE: it leaves early, the writer
              # takes SIGPIPE, and under `pipefail` a match reads as a failure.
              machine.wait_until_succeeds(
                  logged("/root/code-on")
                  + " | grep 'PolicyConfiguration#changed' | grep 'telemetry.telemetryLevel'"
                  " | grep off > /dev/null",
                  timeout=180,
              )
          except Exception:
              print(main_log("/root/code-on"))
              print(machine.execute("journalctl -u code-on --no-pager | tail -80")[1])
              raise

      with subtest("its window is the compositor's own"):
          machine.wait_until_succeeds(
              f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree | grep '\"app_id\"' > /dev/null",
              timeout=120,
          )
          windows = windows_by_app_id(sock)
          print(f"the windows: {windows}")
          assert "code" in windows, windows
          tree = machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree")
          assert '"shell": "xdg_shell"' in tree, tree
          assert '"shell": "xwayland"' not in tree, "a window came through XWayland"

      # EVERY `pgrep -f` PATTERN HAS ONE LETTER IN BRACKETS: the driver runs
      # it under `sh -c`, whose own command line holds the pattern, and a
      # plain one finds that shell — the process it looks for always "there".

      with subtest("the settings, at the managed setting (for the eye)"):
          focus_compositor()
          w, h, x, y = window_box(sock)
          cw, ch, cx, cy = compositor_geometry()
          # THE EDITOR'S AREA FIRST: 1.119 opens with its chat's input in
          # focus, which takes the keys (measured: the query landed there).
          machine.sleep(10)
          machine.succeed(
              f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove {cx + x + w // 4} {cy + y + h // 3} click 1"
          )
          machine.sleep(2)
          machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key --clearmodifiers ctrl+comma")
          machine.sleep(8)
          machine.succeed(
              "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 50 telemetry.telemetryLevel"
          )
          machine.sleep(5)
          machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
          machine.succeed(
              f"magick /tmp/screen.xwd -crop {w}x{h}+{cx + x}+{cy + y} +repage /tmp/settings.png"
          )
          machine.copy_from_machine("/tmp/settings.png", "")
          machine.succeed("systemctl stop code-on")
          machine.wait_until_fails("pgrep -f 'user-data-dir /root/code-o[n]'", timeout=60)

      with subtest("without the file, the same start says nothing of the kind"):
          machine.succeed("mv /etc/vscode/policy.json /root/policy.aside")
          start_code("code-off", "/root/code-off")
          machine.wait_until_succeeds(
              logged("/root/code-off") + " | grep 'PolicyConfiguration#initialize' > /dev/null",
              timeout=180,
          )
          machine.sleep(10)
          log = main_log("/root/code-off")
          changed = [l for l in log.splitlines() if "PolicyConfiguration#changed" in l]
          assert not any("telemetry.telemetryLevel" in l for l in changed), changed
          machine.succeed("systemctl stop code-off")
          machine.wait_until_fails("pgrep -f 'user-data-dir /root/code-of[f]'", timeout=60)
          machine.succeed("mv /root/policy.aside /etc/vscode/policy.json")

      # A RUST PROJECT, which rust-analyzer starts its server for.
      machine.succeed(
          "mkdir -p /root/project/src && "
          "printf '%s\\n' '[package]' 'name = \"hello\"' 'version = \"0.1.0\"' 'edition = \"2021\"' > /root/project/Cargo.toml && "
          "printf '%s\\n' 'fn main() {}' > /root/project/src/main.rs"
      )
      opened = (
          f"--extensions-dir {home}/.vscode/extensions --disable-workspace-trust "
          "/root/project/src/main.rs /root/project"
      )
      # TWO HARNESSES FOR THE MEASURE, both of the test's own:
      # - rust-analyzer, seeing `ID=nixos` in `/etc/os-release` (which the
      #   environment shows too), copies its server aside and patches it with
      #   `nix-build` against `<nixpkgs>` — its own way round NixOS, which
      #   fails here ("nix-build: command not found", measured) and would
      #   hide the question. Its `server.path` pointed at the server it
      #   ships turns that off: what runs, or not, is that program;
      # - root's window, inside the environment, has no capabilities
      #   (bubblewrap leaves none), so the person's `0700` home is opened for
      #   passing through. The person's own VS Code never needs either.
      machine.succeed(f"chmod 0711 {home}")
      for data in ("/root/code-plain", "/root/code-fhs"):
          machine.succeed(
              f"mkdir -p {data}/User && printf '%s' "
              + shlex.quote(json.dumps({"rust-analyzer.server.path": server}))
              + f" > {data}/User/settings.json"
          )


      def extension_log(data):
          return machine.execute(
              f"find {data}/logs -path '*rust-lang.rust-analyzer*' -name '*.log' -exec cat {{}} + 2>/dev/null"
          )[1]


      with subtest("with the plain package, the extension's server never starts"):
          start_code("code-plain", "/root/code-plain", plain, opened)
          # THE EXTENSION RAN AND TRIED: a server that never starts is the
          # program's, not an extension that never woke.
          try:
              machine.wait_until_succeeds(
                  "find /root/code-plain/logs -path '*rust-lang.rust-analyzer*' -name '*.log'"
                  " -exec cat {} + 2>/dev/null | grep 'Failed to execute' > /dev/null",
                  timeout=180,
              )
          finally:
              print(extension_log("/root/code-plain")[-3000:])
          running = machine.execute("pgrep -af 'server/rust-analyze[r]'")[1]
          print(f"the plain package's server: {running!r}")
          assert "server/rust-analyzer" not in running, "the plain package started the server"
          machine.succeed("systemctl stop code-plain")
          machine.wait_until_fails("pgrep -f 'user-data-dir /root/code-plai[n]'", timeout=60)

      with subtest("in the FHS environment, it runs"):
          start_code("code-fhs", "/root/code-fhs", code, opened)
          try:
              machine.wait_until_succeeds("pgrep -f 'server/rust-analyze[r]' > /dev/null", timeout=180)
          finally:
              print(extension_log("/root/code-fhs")[-3000:])
          print(machine.succeed("pgrep -af 'server/rust-analyze[r]'"))
          machine.succeed("systemctl stop code-fhs")
          machine.wait_until_fails("pgrep -f 'user-data-dir /root/code-fh[s]'", timeout=60)

      # ── THE PERSON'S OWN VS CODE, AS THEIR SESSION STARTS IT ──────────────
      #
      # Everything below runs VS Code as the person: a transient unit of their
      # manager, whose environment is the session's (the profiles' `PATH`, and
      # `NIX_PATH` from `sessionVariables`), on an X display of their own — the
      # window's protocol is not the question here.
      import re

      said = manager("show-environment")
      print(said)
      assert "NIX_PATH=nixpkgs=flake:nixpkgs" in said, "the manager has no NIX_PATH as an installed machine's"
      machine.succeed(
          f"systemd-run --user --machine={user}@ --unit=person-xvfb "
          "${pkgs.xorg-server}/bin/Xvfb :98 -screen 0 1280x800x24 -nolisten tcp"
      )
      machine.wait_until_succeeds("test -S /tmp/.X11-unix/X98", timeout=60)


      def start_as_person(unit, data, extra="", program=code, setenv=""):
          """VS Code as the person's session starts it: a unit of their
          manager, with the manager's environment."""
          machine.succeed(
              f"systemd-run --user --machine={user}@ --unit={unit} --setenv=DISPLAY=:98 {setenv} "
              f"{program} --verbose --log trace --disable-gpu --user-data-dir {data} "
              f"--skip-welcome --skip-release-notes {extra}"
          )


      def stop_as_person(unit, data):
          machine.execute(f"systemctl --user --machine={user}@ stop {unit}")
          machine.wait_until_fails(f"pgrep -f 'user-data-dir.{data[:-1]}[{data[-1]}]'", timeout=60)


      def servers():
          """Every rust-analyzer server running: (pid, the program it runs)."""
          found = []
          for pid in machine.execute("pgrep -f 'rust-analyze[r]'")[1].split():
              exe = machine.execute(f"readlink /proc/{pid}/exe")[1].strip()
              if exe.split("/")[-1].endswith("rust-analyzer"):
                  found.append((pid, exe))
          return found


      def unit_log(unit):
          return machine.execute(f"journalctl --no-pager _SYSTEMD_USER_UNIT={unit}.service | tail -40")[1]


      # A RUST PROJECT OF THE PERSON'S, which rust-analyzer starts its server for.
      project = f"{home}/project"
      as_person(
          f"mkdir -p {project}/src && "
          f"printf '%s\\n' '[package]' 'name = \"hello\"' 'version = \"0.1.0\"' 'edition = \"2021\"' > {project}/Cargo.toml && "
          f"printf '%s\\n' 'fn main() {{}}' > {project}/src/main.rs"
      )
      # THE PERSON ANSWERS THE TRUST QUESTION ONCE, BY HAND: the harness's.
      in_project = f"--disable-workspace-trust {project}/src/main.rs {project}"
      storage = "User/globalStorage/rust-lang.rust-analyzer"


      def extension_said(data):
          """The rust-analyzer extension's own log, once it chose a server or
          gave up on one."""
          machine.wait_until_succeeds(
              f"find {data}/logs -path '*rust-lang.rust-analyzer*' -name '*.log' -exec cat {{}} + 2>/dev/null"
              " | grep -E 'Bootstrap error|Using server binary' > /dev/null",
              timeout=300,
          )
          return extension_log(data)


      def server_of(program):
          """The pid of the rust-analyzer server that runs `program`."""
          machine.wait_until_succeeds(
              f"for p in $(pgrep -f 'rust-analyze[r]'); do readlink /proc/$p/exe; done | grep -x '{program}'",
              timeout=120,
          )
          return next(pid for pid, exe in servers() if exe == program)


      def lines(text):
          return [l for l in text.splitlines() if l.strip()]


      with subtest("in the distro's environment, rust-analyzer runs the server it ships: offline, no setting"):
          data = f"{home}/code-shipped"
          start_as_person("code-shipped", data, in_project)
          said = extension_said(data)
          assert f"Using server binary at {server}" in said, said[-3000:]
          pid = server_of(server)
          machine.fail(f"test -e {data}/{storage}/rust-analyzer")
          # THE ENVIRONMENT SAYS `ID=linux`, and the rest is the machine's.
          inside = lines(machine.succeed(f"cat /proc/{pid}/root/etc/os-release"))
          host = lines(machine.succeed("cat /etc/os-release"))
          print(inside)
          assert "ID=linux" in inside and "ID=nixos" in host, (inside, host)
          assert [l for l in inside if not l.startswith("ID=")] == [l for l in host if not l.startswith("ID=")]
          stop_as_person("code-shipped", data)

      # RUST-ANALYZER'S OWN NIXOS PATCH, in the environment as it was before
      # it said `ID=linux`: what a VS Code that shows `ID=nixos` meets on a
      # machine of ours.
      code_nixos = "${vscodeFhsNixos}/bin/code"

      with subtest("its own NixOS patch builds offline with a cold cache: the global registry is the store's"):
          # THE MACHINE'S NIX READS THE GLOBAL REGISTRY FROM THE STORE.
          registry = as_person("nix config show flake-registry").strip()
          assert registry.startswith("/nix/store/") and registry.endswith("-flake-registry.json"), registry
          as_person(f"rm -rf {home}/.cache/nix")
          data = f"{home}/code-patch-offline"
          start_as_person("code-patch-offline", data, in_project, program=code_nixos)
          try:
              said = extension_said(data)
          finally:
              print(unit_log("code-patch-offline"))
          assert f"Using server binary at {data}/{storage}/rust-analyzer" in said, said[-3000:]
          patched = machine.succeed(f"readlink {data}/{storage}/rust-analyzer").strip()
          assert patched.startswith("/nix/store/") and patched.endswith("-rust-analyzer"), patched
          # ITS LOADER IS NOW NIXPKGS' GLIBC.
          print(machine.succeed(f"file -L {patched}"))
          machine.succeed(f"file -L {patched} | grep -E 'interpreter /nix/store/[^ ]*glibc[^ ]*/ld-linux'")
          server_of(patched)
          machine.fail(f"test -e {home}/.cache/nix/flake-registry.json")
          stop_as_person("code-patch-offline", data)
          # AND IT GOES, so the next start builds it again — once no server
          # runs it: one can outlive its window by a moment, and a store path
          # a process runs is alive (measured: "Cannot delete path").
          machine.execute("pkill -f 'rust-analyze[r]'")
          machine.wait_until_fails(
              f"for p in $(pgrep -f 'rust-analyze[r]'); do readlink /proc/$p/exe; done | grep -x '{patched}'",
              timeout=60,
          )
          machine.succeed(f"rm -rf {data} && nix-store --delete {patched}")

      # FROM HERE ON EVERY NAME IS THIS MACHINE: the stand-in answers Nix's
      # two hosts as the internet would (the global registry; a cache without
      # the output), VS Code's and Google's, and writes down every TLS
      # greeting.
      machine.succeed("systemctl start vscode-dns")
      machine.wait_until_succeeds("journalctl -u vscode-dns | grep -i started > /dev/null", timeout=30)


      def heard(since):
          """Every name looked up, every TLS greeting and every request the
          stand-in answered since `since` (the logs emptied then)."""
          looked = machine.succeed(f"journalctl -u vscode-dns --since @{since} -o cat --no-pager")
          names = {m.group(2) for m in re.finditer(r"query\[(\w+)\] (\S+) from", looked)}
          greeted = {json.loads(l)["sni"] for l in machine.succeed(f"cat {sni_log}").splitlines() if l.strip()}
          requests = [json.loads(l) for l in machine.succeed(f"cat {gallery_log}").splitlines() if l.strip()]
          return names | greeted | {r["host"] for r in requests}, requests


      def from_now():
          machine.succeed(f": > {gallery_log}; : > {sni_log}")
          return machine.succeed("date +%s").strip()


      with subtest("online, the patch asks the cache and never channels.nixos.org"):
          data = f"{home}/code-patch-online"
          since = from_now()
          start_as_person("code-patch-online", data, in_project, program=code_nixos)
          said = extension_said(data)
          assert f"Using server binary at {data}/{storage}/rust-analyzer" in said, said[-3000:]
          server_of(machine.succeed(f"readlink {data}/{storage}/rust-analyzer").strip())
          reached, requests = heard(since)
          stop_as_person("code-patch-online", data)
          nix_side = sorted(h for h in reached if h.endswith("nixos.org"))
          print(f"what Nix reached: {nix_side}; all: {sorted(reached)}")
          assert set(nix_side) <= {"cache.nixos.org"}, nix_side

      with subtest("pointed at the toolchain's rust-analyzer, the server starts, and nothing is patched"):
          data = f"{home}/code-toolchain"
          as_person(
              f"mkdir -p {data}/User && printf '%s' "
              + shlex.quote(json.dumps({"rust-analyzer.server.path": "rust-analyzer"}))
              + f" > {data}/User/settings.json"
          )
          toolchains = as_person("readlink -f $(command -v rust-analyzer)").strip()
          assert toolchains.startswith("/nix/store/") and "rust-analyzer" in toolchains, toolchains
          start_as_person("code-toolchain", data, in_project)
          said = extension_said(data)
          assert "Using server binary at rust-analyzer" in said, said[-3000:]
          machine.wait_until_succeeds(
              f"for p in $(pgrep -f 'rust-analyze[r]'); do readlink /proc/$p/exe; done | grep -x '{toolchains}'",
              timeout=120,
          )
          print(f"servers: {servers()}")
          machine.fail(f"test -e {data}/{storage}/rust-analyzer")
          stop_as_person("code-toolchain", data)

      # ── AN UPDATED BUILT-IN IS NOT THE PERSON'S ───────────────────────────
      with subtest("the Copilot Chat VS Code updates from the Marketplace never enters the person's list"):
          before = listed()
          # THE PERSON'S OWN PROFILE AND EXTENSIONS, whose settings say
          # `verifySignature` off (the stand-in cannot sign; the header).
          data = f"{home}/.config/Code"
          start_as_person("code-default", data)
          machine.wait_until_succeeds(f"grep -qi 'github.copilot-chat' {manifest}", timeout=240)
          stop_as_person("code-default", data)
          entries = json.loads(machine.succeed(f"cat {manifest}"))
          updated = [e for e in entries if e["identifier"]["id"].lower() == "github.copilot-chat"]
          print(updated)
          assert updated and updated[0]["metadata"].get("isBuiltin") is True, updated
          # THE RECORD, over the manifest as VS Code left it — started here,
          # and waited for (a oneshot), so no earlier run answers for it.
          manager("start ronoctua-vscode-record.service")
          assert listed() == before, f"{listed()} != {before}"

      # ── WHAT VS CODE ASKS ITS VENDOR AT START-UP ─────────────────────────
      as_person(f"printf '%s\\n' 'print(1)' > {home}/hello.py")
      # A Python file, which the product's own recommendations name.
      a_python_file = f"--disable-workspace-trust {home}/hello.py"


      def heard_at_start(unit, settings, extra="", seconds=50):
          """A fresh profile of the person's, no extension of theirs, started
          and left alone: every host it reached, and every request."""
          data = f"{home}/{unit}"
          as_person(f"mkdir -p {data}/User {data}-extensions")
          as_person("printf '%s' " + shlex.quote(json.dumps(settings)) + f" > {data}/User/settings.json")
          since = from_now()
          start_as_person(unit, data, f"--extensions-dir {data}-extensions {extra}",
                          setenv=f"--setenv=NODE_EXTRA_CA_CERTS={ca}")
          try:
              machine.wait_until_succeeds(
                  f"find {data}/logs -name main.log -exec cat {{}} + 2>/dev/null | grep 'PolicyConfiguration#initialize' > /dev/null",
                  timeout=180,
              )
          except Exception:
              print(unit_log(unit))
              raise
          machine.sleep(seconds)
          reached, requests = heard(since)
          # WHAT EACH OF ITS PROCESSES WAS STARTED WITH, read while they run.
          ran = machine.succeed(
              f"for p in $(pgrep -f 'user-data-dir.{data[:-1]}[{data[-1]}]'); do"
              " tr '\\0' '\\n' < /proc/$p/cmdline; echo '--- end'; done"
          ).split("--- end\n")
          stop_as_person(unit, data)
          asked = sorted({(r["method"], r["host"], r["path"].split("?")[0], r["code"]) for r in requests})
          print(f"== {unit} {settings} {extra}")
          print(f"reached: {sorted(reached)}")
          for a in asked:
              print(f"  {a}")
          started = [[a for a in r.splitlines() if a] for r in ran if r.strip()]
          return reached, {(h, p) for _, h, p, _ in asked}, started


      # THE STAND-IN CANNOT SIGN: only here, so the update goes to its end.
      unsigned = {"extensions.verifySignature": False}
      # WHAT IT REACHES, AND WHERE: its CDN (the control lists, the chat
      # participants' list, the emergency alert), the Marketplace and its asset
      # hosts (the built-in Copilot Chat's latest version, its manifest and its
      # package — the fallback of the latest version too, though the stand-in
      # never sends it there). NEVER Google's dictionary redirector nor the
      # mirror it sends to, though the stand-in answers both as Google does:
      # the distro's `code` maps the redirector to nothing (`nix/vscode.nix`).
      marketplace = {"marketplace.visualstudio.com", "www.vscode-unpkg.net",
                     "github.gallery.vsassets.io", "github.gallerycdn.vsassets.io"}
      google = {"redirector.gvt1.com", "r1---sn-ronoctua.gvt1.com"}
      vendor = marketplace | {"main.vscode-cdn.net"}
      copilot = ("marketplace.visualstudio.com", "/_apis/public/gallery/vscode/github/copilot-chat/latest")
      rule = "--host-resolver-rules=MAP redirector.gvt1.com ~NOTFOUND"

      with subtest("at start-up, VS Code reaches its vendor, and never Google's dictionary"):
          reached, asked, started = heard_at_start("start-shipped", unsigned, seconds=60)
          # THE DICTIONARY FIRST: without the rule, this is what fails.
          assert not reached & google, f"VS Code asked Google's dictionary: {sorted(reached & google)}"
          assert reached <= vendor, f"VS Code reached {sorted(reached - vendor)} too"
          for wanted in (copilot,
                         ("main.vscode-cdn.net", "/extensions/marketplace.json"),
                         ("main.vscode-cdn.net", "/extensions/chat.json"),
                         ("main.vscode-cdn.net", "/core/stable.json")):
              assert wanted in asked, f"{wanted} was not asked: {sorted(asked)}"
          # THE UPDATE WENT TO ITS END — the shared process's network, beside
          # the rule: the newer Copilot Chat's package came.
          assert any("copilot-chat" in p and p.endswith("/Microsoft.VisualStudio.Services.VSIXPackage")
                     for _, p in asked), sorted(asked)
          # AND THE RULE IS ON THE COMMAND LINE of the program that ran — the
          # Electron that `code` starts, no child of it (`--type=`) and not
          # the CLI's Node (`cli.js`).
          for args in started:
              print(" ".join(args)[:400])
          main = [a for a in started
                  if a[0].endswith("/lib/vscode/code")
                  and not any(x.startswith("--type=") or x.endswith("/cli.js") for x in a)]
          assert main, "VS Code's main process was not found"
          assert all(rule in a for a in main), main
          # WHERE CHROMIUM RESOLVES NAMES, said for the record.
          print([rule in a for a in started if "--utility-sub-type=network.mojom.NetworkService" in a])

      with subtest("chat.disableAIFeatures does not stop the Copilot Chat question"):
          reached, asked, _ = heard_at_start("start-no-ai", {**unsigned, "chat.disableAIFeatures": True})
          assert copilot in asked, sorted(asked)
          assert not reached & google, sorted(reached)

      with subtest("extensions.autoCheckUpdates off asks the Marketplace nothing"):
          reached, asked, _ = heard_at_start("start-no-check", {**unsigned, "extensions.autoCheckUpdates": False})
          assert not reached & marketplace, sorted(reached)
          assert reached <= vendor, sorted(reached)

      with subtest("but a file the product recommends an extension for asks it"):
          reached, asked, _ = heard_at_start(
              "start-no-check-py", {**unsigned, "extensions.autoCheckUpdates": False}, a_python_file)
          assert reached & marketplace, sorted(reached)
          assert not reached & google, sorted(reached)

      with subtest("and extensions.ignoreRecommendations quiets that"):
          reached, asked, _ = heard_at_start(
              "start-quiet-py",
              {**unsigned, "extensions.autoCheckUpdates": False, "extensions.ignoreRecommendations": True},
              a_python_file)
          assert not reached & marketplace, sorted(reached)
          assert reached <= vendor, sorted(reached)
    '';
}
