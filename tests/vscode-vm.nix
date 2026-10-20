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
#
# VS Code's window runs as root on the compositor (with `--no-sandbox` and a
# data folder of its own, which VS Code demands of root), because the
# compositor of `compositor-xvfb.nix` is root's; the policy is the machine's,
# read the same for every user. Steps 3 to 5 are the person's, under their
# own manager (`linger`).
#
# TWO HARNESSES FOR THE STAND-IN, and nothing else: the CA is handed to
# `code` by `NODE_EXTRA_CA_CERTS` as well as the machine's bundle (the real
# Marketplace's certificates are public); and the person's settings say
# `"extensions.verifySignature": false` for the install — the Marketplace
# signs every public extension, and the pinned 1.119 refuses one without a
# signature ("Signature verification failed with 'NotSigned' error",
# measured against this stand-in), which no one but Microsoft can make.
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
  # THE DISTRO'S (the FHS environment) and, for the measure, the plain one.
  vscodeFhs = import ../nix/vscode.nix { pkgs = unfree; };
  vscodePlain = unfree.vscode;

  # THE PINNED EXTENSION: rust-analyzer's own VSIX for linux-x64, whose
  # server is a program linked for other distributions.
  rustAnalyzer = pkgs.fetchurl {
    url = "https://github.com/rust-lang/rust-analyzer/releases/download/2026-10-12/rust-analyzer-linux-x64.vsix";
    hash = "sha256-/VR9Si34tElTexo4lqGMEaNT8OxEmsU0cVUcoGlyO8I=";
  };
  vsixes = pkgs.linkFarm "vscode-gallery-vsix" [
    {
      name = "rust-lang.rust-analyzer.vsix";
      path = rustAnalyzer;
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
      "subjectAltName=${lib.concatMapStringsSep "," (n: "DNS:${n}") galleryNames}" > server.ext
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
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 4096;
      virtualisation.cores = 2;
      virtualisation.diskSize = 4096;
      networking.hostName = lib.mkForce "noctua-vscode";
      users.users.root.hashedPassword = lib.mkForce null;
      # THE PERSON'S MANAGER AT BOOT: the path unit is theirs.
      users.users.${config.ronoctua.user}.linger = true;

      ronoctua.editor.vscode = true;
      ronoctua.editor.vscodePackage = vscodeFhs;

      # THE STAND-IN MARKETPLACE, by the real names, over TLS.
      networking.hosts."127.0.0.1" = galleryNames;
      security.pki.certificateFiles = [ "${certs}/ca.crt" ];
      systemd.services.vscode-gallery = {
        description = "A stand-in for the Visual Studio Marketplace";
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${./vscode-gallery.py} ${vsixes} ${certs}/server.crt ${certs}/server.key /var/log/vscode-gallery.jsonl";
      };

      environment.systemPackages = [
        scroll
        pkgs.procps
        pkgs.imagemagick
        pkgs.file
        pkgs.python3
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


      def as_person(command, check=True):
          uid = machine.succeed(f"id -u {user}").strip()
          wrapped = (
              f"runuser -u {user} -- env HOME={home} XDG_RUNTIME_DIR=/run/user/{uid} "
              f"PATH=/run/current-system/sw/bin sh -c " + shlex.quote(command)
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

      with subtest("the policy is the machine's, where the pinned build reads it"):
          policy = json.loads(machine.succeed("cat /etc/vscode/policy.json"))
          assert policy == {"TelemetryLevel": "off"}, policy

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
    '';
}
