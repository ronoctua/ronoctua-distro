# The software centre on a BOOTED SESSION, offline.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# The person logs in on the first console, the login starts the compositor as
# a user unit, the compositor's own `exec` lines start the session target, and
# the footer and the queue come up as the units the machine really has. Every
# order below goes to THAT queue, which runs `ronoctua-software run --progress`
# in a scope of its own — the path a click takes, and nothing typed by hand in
# between. Five things, each one a defect this feature found or could bring:
#
#   1. THE SESSION'S `PATH` AND `XDG_DATA_DIRS` REACH THE FOOTER AND THE QUEUE,
#      read from `/proc/<pid>/environ` of the running processes. With the
#      default unit `PATH` a program from the person's profile never opened
#      from the launcher, and a Flatpak's export is only in the launcher when
#      its directory is in `XDG_DATA_DIRS`.
#   2. A PROGRAM ADDED TO THE PERSON'S PROFILE IS IN THE LAUNCHER WITHOUT
#      RESTARTING THE FOOTER. The profile is a link into the store, whose
#      modification time never moves; the stamp follows the link. The FIRST
#      `nix profile add` only makes the profile appear, which any stamp sees,
#      so the case is the SECOND: a directory there before and after, with the
#      same modification time, that resolves somewhere else — and the test
#      asserts that this is what happened. Each entry's `Exec=` is a BARE NAME,
#      so the launch also proves the footer resolves it through the session's
#      `PATH`.
#   3. AN APP THAT EXITS WITH 3 RUNS ONCE, IN ITS SCOPE. Before the scope
#      probe, any failure status was read as "the scope was not created" and
#      the program was started a second time, directly. Without a scope it runs
#      once anyway, so the scope is asserted too — else this passes for the
#      wrong reason. Both come from the launch's own verdict, which the footer
#      logs after any relaunch would have been started.
#   4. A WEB APP OPENS CHROME WITH `--profile-directory` AND `--app=` AND
#      NOTHING ELSE — no `--kiosk`, no `--start-fullscreen`, no
#      `--user-data-dir`, no `--enable-features`. Chrome is a stand-in that
#      writes down its arguments, installed at the stable path the `.desktop`
#      names.
#   5. AN APPIMAGE IS INTEGRATED, OPENED AND REMOVED WITHOUT A TRACE: its file,
#      `.desktop`, icon, sidecar and `appimage-run`'s extraction.
#
# THE APPIMAGE IS BUILT HERE, and it is a real type 2 image for everybody who
# reads one: an ELF with `AI\x02` at byte 8, a squashfs starting where the
# section table ends, a `.desktop`, `.DirIcon` and a hicolor icon inside. Its
# RUNTIME is a stand-in — a small program that says so — because the type 2
# runtime is not in nixpkgs and nothing on this system runs it: `appimage-run`
# reads the header with `readelf` and unpacks the payload itself, and binfmt
# hands a direct `exec` to `appimage-run` as well. What is NOT proved, then, is
# an image mounting itself with FUSE, which this system never asks for.
#
# WHAT THE STAGE CHANGES, and nothing else: the compositor draws into Xvfb
# (there is no card), nothing reaches a substituter, and Chrome is the
# stand-in. The real one cannot be in this machine at all — it is unfree, and
# a test node takes the flake's `pkgs`, which does not allow that — so the
# browser module is off and the two units get the one variable
# `nix/software-tools.nix` gives them when it is on, read from that table.
# What this leaves unproved is the CLI wrapper's own `--set-default` of that
# variable: here `web-open` inherits it from the footer. Every other unit,
# variable and wrapper under test is the modules' own.
{
  pkgs,
  lib,
  inputs,
  ronoctua,
}:

let
  # THE STABLE PATH the units name — never a store path — read from the table
  # that gives it, so a change there reaches this test. Only this entry is
  # read, and a table's values are lazy: the arguments nothing here reads are
  # null.
  chromeStable =
    (import ../nix/software-tools.nix {
      inherit lib pkgs;
      nix = null;
      index = null;
      nixpkgsRev = null;
      chrome = true;
    }).RONOCTUA_CHROME;

  # WHERE THE PERSON'S DOWNLOADS ARE, from the one table that says so.
  downloads = (import ../nix/user-dirs.nix).download;

  # ONE PICTURE, written byte by byte so nothing is downloaded: a 64 px square
  # PNG, the icon of the web app and of the AppImage.
  icon = pkgs.runCommand "ronoctua-probe-icon.png" { nativeBuildInputs = [ pkgs.python3 ]; } ''
    python3 - "$out" <<'EOF'
    import struct
    import sys
    import zlib

    side = 64
    row = bytes([0]) + bytes([0x33, 0x66, 0xCC, 0xFF]) * side


    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))


    png = bytes([0x89]) + "PNG\r\n".encode() + bytes([0x1A, 0x0A])
    png += chunk("IHDR".encode(), struct.pack(">IIBBBBB", side, side, 8, 6, 0, 0, 0))
    png += chunk("IDAT".encode(), zlib.compress(row * side))
    png += chunk("IEND".encode(), bytes())
    open(sys.argv[1], "wb").write(png)
    EOF
  '';

  # WHAT `nix profile add` INSTALLS: a program and its launcher entry, whose
  # `Exec=` is the program's bare name — the launcher can only find it through
  # the footer's `PATH`. Two of them, for two generations.
  profileProbe =
    name:
    pkgs.symlinkJoin {
      inherit name;
      paths = [
        (pkgs.writeShellScriptBin name ''
          echo ran >> "$HOME/${name}.ran"
        '')
        (pkgs.writeTextDir "share/applications/${name}.desktop" ''
          [Desktop Entry]
          Type=Application
          Name=${name}
          Exec=${name}
        '')
      ];
    };
  firstProbe = profileProbe "ronoctua-probe-first";
  secondProbe = profileProbe "ronoctua-probe-second";

  # AN APP THAT ENDS WITH A FAILURE STATUS, and counts how many times it ran.
  exitThree = pkgs.writeShellScript "ronoctua-probe-exit" ''
    echo run >> "$HOME/probe-exit.runs"
    exit 3
  '';
  exitDesktop = pkgs.writeText "ronoctua-probe-exit.desktop" ''
    [Desktop Entry]
    Type=Application
    Name=Exit Probe
    Exec=${exitThree}
  '';

  # CHROME, AS FAR AS A WEB APP CAN TELL: it writes down how it was called.
  fakeChrome = pkgs.writeShellScriptBin "google-chrome-stable" ''
    {
      printf 'argv0=%s\n' "$0"
      for a in "$@"; do printf 'arg=%s\n' "$a"; done
      printf 'end\n'
    } >> "$HOME/chrome-argv.log"
  '';

  # A TYPE 2 APPIMAGE, made offline. See the header for what its runtime is.
  appImage =
    pkgs.runCommandCC "Probe_Image-1.0-x86_64.AppImage"
      {
        nativeBuildInputs = [
          pkgs.squashfsTools
          pkgs.python3
        ];
      }
      ''
        cat > runtime.c <<'EOF'
        #include <stdio.h>
        int main(void) {
          fputs("a stand-in runtime: this AppImage opens through appimage-run\n", stderr);
          return 1;
        }
        EOF
        cc -Os -o runtime runtime.c

        mkdir -p AppDir/usr/share/icons/hicolor/64x64/apps
        cat > AppDir/AppRun <<'EOF'
        #!/bin/sh
        echo ran >> "$HOME/probe-appimage.ran"
        EOF
        chmod 755 AppDir/AppRun
        cat > AppDir/probe-image.desktop <<'EOF'
        [Desktop Entry]
        Type=Application
        Name=Probe Image
        Comment=An AppImage made for a test
        Exec=probe-image %F
        Icon=probe-image
        Categories=Utility;
        X-AppImage-Version=1.0
        EOF
        cp ${icon} AppDir/usr/share/icons/hicolor/64x64/apps/probe-image.png
        ln -s usr/share/icons/hicolor/64x64/apps/probe-image.png AppDir/.DirIcon
        ln -s usr/share/icons/hicolor/64x64/apps/probe-image.png AppDir/probe-image.png
        mksquashfs AppDir payload.squashfs -all-root -noappend -quiet

        # THE PAYLOAD STARTS WHERE THE SECTION TABLE ENDS — the offset both
        # `appimage-run` and the crate compute — so the table has to be the
        # last thing in the runtime, and the build refuses when it is not.
        python3 - "$out" <<'EOF'
        import struct
        import sys

        runtime = bytearray(open("runtime", "rb").read())
        shoff = struct.unpack_from("<Q", runtime, 0x28)[0]
        shentsize, shnum = struct.unpack_from("<HH", runtime, 0x3A)
        end = shoff + shentsize * shnum
        if end != len(runtime):
            raise SystemExit(f"the section table ends at {end}, the runtime at {len(runtime)}")
        runtime[8:11] = "AI".encode() + bytes([2])
        payload = open("payload.squashfs", "rb").read()
        if not payload.startswith("hsqs".encode()):
            raise SystemExit("the payload is not a squashfs")
        open(sys.argv[1], "wb").write(bytes(runtime) + payload)
        EOF
        chmod 755 "$out"
      '';

  # THE QUEUE'S OWN PROTOCOL, one JSON object per line — spoken here and not
  # through our client, so a protocol changed under both would still fail.
  queueClient = pkgs.writeText "ask-the-queue.py" ''
    import socket
    import sys

    path, ask = sys.argv[1], sys.argv[2]
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(30)
    s.connect(path)
    s.sendall((ask + "\n").encode())
    data = bytes()
    while not data.endswith("\n".encode()):
        chunk = s.recv(65536)
        if not chunk:
            break
        data += chunk
    print(data.decode().strip())
  '';

  # NO MACHINE UNDER TEST SCHEDULES A REBUILD OF ITSELF: the reason is in
  # `tests/compositor-xvfb.nix`.
  brainPrefs = pkgs.writeText "brain.json" ''{"tasks":{"system-update":{"enabled":false}}}'';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-software";

  # `flatpak.nix` IMPORTS nix-flatpak's modules through `inputs`, and an
  # import cannot read `_module.args`.
  node.specialArgs = { inherit inputs; };

  nodes.machine =
    { config, ... }:
    let
      user = config.ronoctua.user;
      home = "/home/${user}";
    in
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/update.nix
        ../nix/modules/transfers.nix
        ../nix/modules/media.nix
        ../nix/modules/scroll.nix
        ../nix/modules/icons.nix
        ../nix/modules/home.nix
        ../nix/modules/browser.nix
        ../nix/modules/software.nix
        ../nix/modules/flatpak.nix
        ../nix/modules/appimage.nix
        inputs.home-manager.nixosModules.home-manager
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 3072;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-software";
      users.users.root.hashedPassword = lib.mkForce null;
      # One attrset: a dynamic attribute in two sentences does not evaluate.
      users.users.${user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };

      # THE REDUCED PROFILE's answers where they would only add weight; the
      # AppImage support stays on, as in the full one.
      ronoctua.media.enable = false;
      ronoctua.icons.superpack = false;
      ronoctua.browser.enable = false;
      # The update sequence is not under test, and a machine under test does
      # not rebuild itself.
      ronoctua.update.enable = false;

      # CHROME AT ITS STABLE PATH, and the variable the software tools give
      # the two units when the browser is on — see the header.
      environment.systemPackages = [ fakeChrome ];
      systemd.user.services.ronoctua-footer.environment.RONOCTUA_CHROME = chromeStable;
      systemd.user.services.ronoctua-transfers.environment.RONOCTUA_CHROME = chromeStable;

      # OFFLINE, and saying so: nothing waits on a substituter it cannot reach.
      nix.settings.substituters = lib.mkForce [ ];
      virtualisation.additionalPaths = [
        firstProbe
        secondProbe
      ];

      fonts.enableDefaultPackages = false;
      fonts.packages = [ ronoctua.fonts.noctua-iosevka-core ];

      # THE SCREEN. There is no card, so the compositor opens a window on an X
      # server instead — the arrangement `tests/compositor-xvfb.nix` explains.
      # Up before any login, so the session the login starts finds it.
      systemd.services.xvfb = {
        description = "The screen this test's compositor draws on";
        wantedBy = [ "multi-user.target" ];
        before = [ "systemd-user-sessions.service" ];
        serviceConfig = {
          # `-ac`: the compositor is the person's, the server is root's.
          ExecStart = "${pkgs.xorg-server}/bin/Xvfb :99 -screen 0 1600x1000x24 -ac -nolisten tcp";
          ExecStartPost = "${pkgs.bash}/bin/bash -c 'until [ -S /tmp/.X11-unix/X99 ]; do sleep 0.1; done'";
        };
      };
      systemd.user.services.scroll.environment = {
        WLR_BACKENDS = "x11";
        DISPLAY = ":99";
        WLR_RENDERER = "pixman";
      };

      systemd.tmpfiles.rules = [
        "d ${home}/.config 0755 ${user} users -"
        "d ${home}/.config/ronoctua 0755 ${user} users -"
        "L+ ${home}/.config/ronoctua/brain.json - - - - ${brainPrefs}"
      ];
    };

  testScript =
    { nodes, ... }:
    ''
      import json
      import re
      import shlex

      user = "${nodes.machine.ronoctua.user}"
      home = f"/home/{user}"
      FOOTER = "ronoctua-footer.service"
      QUEUE = "ronoctua-transfers.service"
      SHELL = "${ronoctua.shell}/bin/ronoctua-shell"
      STABLE_CHROME = "${chromeStable}"
      # THE WRAPPER, and not the bare binary it wraps: both end the same way.
      SOFTWARE = "${nodes.machine.ronoctua.software.package}/bin/ronoctua-software"
      DOWNLOADS = f"{home}/${downloads}"
      SCANNED = "application catalogue scanned"
      RESCANNING = "the application directories changed; rescanning"
      # tracing colours its fields, and a colour code sits between `apps` and
      # `=` in what the journal keeps.
      ANSI = re.compile(r"\x1b\[[0-9;]*m")


      def user_ctl(args):
          return machine.succeed(f"systemctl --machine={user}@ --user {args}")


      def as_user(command):
          """A command in the person's own manager: the environment every unit
          of theirs inherits."""
          return machine.succeed(
              f"systemd-run --machine={user}@ --user --pipe --wait --quiet --collect {command}"
          )


      def journal_of(unit):
          raw = machine.succeed(
              f"journalctl --no-pager -a -o cat _SYSTEMD_USER_UNIT={unit} || true"
          )
          return ANSI.sub("", raw)


      def count(said, unit=FOOTER):
          return sum(1 for line in journal_of(unit).splitlines() if said in line)


      def diagnose():
          for unit in ("scroll.service", "scroll-session.target", FOOTER, QUEUE):
              print(machine.execute(f"systemctl --machine={user}@ --user status {unit} --no-pager -l")[1])
          print(machine.execute("loginctl list-sessions --no-pager")[1])
          print(machine.execute("systemctl status xvfb getty@tty1 --no-pager -l")[1])
          print(machine.execute(f"journalctl --no-pager -a -o cat _UID=$(id -u {user}) | tail -120")[1])


      def environ(unit):
          """The running process's own environment, as the kernel keeps it."""
          pid = user_ctl(f"show -p MainPID --value {unit}").strip()
          assert pid not in ("", "0"), f"{unit} has no process"
          raw = machine.succeed(f"tr '\\0' '\\n' < /proc/{pid}/environ")
          env = {}
          for line in raw.splitlines():
              if "=" in line:
                  key, value = line.split("=", 1)
                  env.setdefault(key, value)
          return int(pid), env


      def command(line):
          """A command in the RUNNING footer, through its control socket."""
          machine.succeed(
              f"runuser -u {user} -- env XDG_RUNTIME_DIR={runtime} "
              f"WAYLAND_DISPLAY={display} {SHELL} command {line}"
          )


      def opened(desktop_id):
          stem = desktop_id.removesuffix(".desktop")
          return sum(
              1
              for line in journal_of(FOOTER).splitlines()
              if "opening the application" in line and stem in line
          )


      def wait_for(what, timeout=90):
          for _ in range(timeout):
              if what():
                  return True
              machine.sleep(1)
          return what()


      def launch(desktop_id):
          """Opens an entry by its id, as the launcher does.

          An entry the catalogue does not have yet is not opened: the miss
          asks the footer for a sweep and says so. Then it is asked again,
          once the sweep has landed — which is the whole of "without a
          restart"."""
          before = opened(desktop_id)
          scans = count(SCANNED)
          command(f"software-launch-latest {desktop_id}")
          wait_for(lambda: opened(desktop_id) > before or count(SCANNED) > scans)
          if opened(desktop_id) == before:
              command(f"software-launch-latest {desktop_id}")
          assert wait_for(lambda: opened(desktop_id) > before), (
              f"the launcher never opened {desktop_id}:\n" + journal_of(FOOTER)[-3000:]
          )


      def open_listed(desktop_id):
          """Opens an entry the catalogue must ALREADY have: one command, and
          no second chance — a miss would ask for a sweep of its own, and
          that sweep would hide one that never happened."""
          before = opened(desktop_id)
          command(f"software-launch-latest {desktop_id}")
          assert wait_for(lambda: opened(desktop_id) > before, timeout=30), (
              f"{desktop_id} was not in the launcher:\n" + journal_of(FOOTER)[-3000:]
          )


      def swept_since(scans, what):
          """The footer swept the launcher BY ITSELF when a job ended."""
          assert wait_for(lambda: count(SCANNED) > scans), (
              f"the footer did not sweep the launcher when {what} ended:\n"
              + journal_of(FOOTER)[-3000:]
          )


      def ask_queue(body):
          out = machine.succeed(
              f"runuser -u {user} -- ${pkgs.python3}/bin/python3 ${queueClient} "
              f"{runtime}/ronoctua-transfers.sock {shlex.quote(json.dumps(body))}"
          )
          return json.loads(out.strip())


      def queue_order(order, label, timeout=300):
          """One order, through the queue — which runs the CLI exactly as it
          runs it for a click — and the job as it ended."""
          said = ask_queue(
              {"ask": "add", "work": {"work": "software", "order": order, "label": label}}
          )
          assert said.get("say") == "took", f"the queue did not take the order: {said}"
          number = said["id"]
          for _ in range(timeout):
              jobs = ask_queue({"ask": "list"}).get("jobs", [])
              job = next((j for j in jobs if j.get("id") == number), None)
              if job is not None and job.get("stage") in ("done", "failed", "cancelled"):
                  print(json.dumps(job, indent=1))
                  return job
              machine.sleep(1)
          raise AssertionError(f"the order {label} did not end in {timeout} s")


      def desktop_fields(path):
          fields = {}
          for line in machine.succeed(f"cat {path}").splitlines():
              if "=" in line and not line.startswith("["):
                  key, value = line.split("=", 1)
                  fields.setdefault(key, value)
          return fields


      def is_png(path):
          magic = machine.succeed(f"head -c 8 {shlex.quote(path)} | od -An -tx1").split()
          return magic == ["89", "50", "4e", "47", "0d", "0a", "1a", "0a"]


      def restarts():
          return int(user_ctl(f"show -p NRestarts --value {FOOTER}").strip())


      machine.start()
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit("xvfb.service")
      uid = machine.succeed(f"id -u {user}").strip()
      runtime = f"/run/user/{uid}"

      with subtest("the login brings the session up: compositor, footer and queue"):
          try:
              machine.wait_until_succeeds(
                  f"systemctl --machine={user}@ --user is-active {FOOTER}", timeout=300
              )
              machine.wait_until_succeeds(
                  f"systemctl --machine={user}@ --user is-active {QUEUE}", timeout=60
              )
              # `grep > /dev/null` and never `grep -q`: `-q` leaves on the first
              # match, the writer takes SIGPIPE, and `pipefail` fails the line
              # because the grep succeeded (`tests/compositor-xvfb.nix`).
              for said in ("control socket opened", SCANNED):
                  machine.wait_until_succeeds(
                      f"journalctl -a -o cat _SYSTEMD_USER_UNIT={FOOTER} "
                      f"| grep '{said}' > /dev/null",
                      timeout=120,
                  )
          except Exception:
              diagnose()
              raise
          manager = user_ctl("show-environment")
          found = re.search(r"^WAYLAND_DISPLAY=(\S+)$", manager, re.M)
          assert found, f"the compositor never exported its display:\n{manager}"
          display = found.group(1)
          machine.succeed(f"test -S {runtime}/ronoctua-{display}.sock")
          footer_pid, _ = environ(FOOTER)
          # FROM HERE: a restart while the session came up is the session's
          # business, and one after it is this test's.
          restarts_at_start = restarts()
          print(f"footer pid {footer_pid}, display {display}, restarts {restarts_at_start}")

      with subtest("the session's PATH and XDG_DATA_DIRS reach the footer and the queue"):
          profile_bins = [
              f"/etc/profiles/per-user/{user}/bin",
              f"{home}/.nix-profile/bin",
          ]
          exports = [
              f"{home}/.local/share/flatpak/exports/share",
              "/var/lib/flatpak/exports/share",
          ]
          environs = {}
          for unit in (FOOTER, QUEUE):
              pid, env = environ(unit)
              environs[unit] = env
              print(f"{unit} pid={pid}")
              print(f"  PATH={env.get('PATH')}")
              print(f"  XDG_DATA_DIRS={env.get('XDG_DATA_DIRS')}")
          for unit, env in environs.items():
              path = env.get("PATH", "").split(":")
              data = env.get("XDG_DATA_DIRS", "").split(":")
              assert f"{home}/.nix-profile/bin" in path, (
                  f"{unit}: the person's Nix profile is not on PATH: {env.get('PATH')}"
              )
              print(f"  {unit}: per-user profile on PATH: {profile_bins[0] in path}")
              missing = [e for e in exports if e not in data]
              assert not missing, (
                  f"{unit}: XDG_DATA_DIRS lacks the Flatpak exports {missing}: "
                  f"{env.get('XDG_DATA_DIRS')}"
              )
              assert f"{home}/.nix-profile/share" in data, (
                  f"{unit}: XDG_DATA_DIRS lacks the Nix profile: {env.get('XDG_DATA_DIRS')}"
              )
          # AND THE QUEUE NAMES THE CLI BY ITS WRAPPER, which is what it runs.
          software = environs[QUEUE].get("RONOCTUA_SOFTWARE", "")
          assert software == SOFTWARE, f"the queue runs {software}, not the wrapper {SOFTWARE}"
          # NOT A REHEARSAL: that is the nested loop's, and here every order acts.
          for unit, env in environs.items():
              assert "RONOCTUA_SOFTWARE_REHEARSE" not in env, f"{unit} rehearses"

      with subtest("a program added to the profile is in the launcher, with no restart"):
          before = restarts()
          entries = f"{home}/.nix-profile/share/applications"

          def resolved():
              """Where the profile's entries resolve, and that directory's
              modification time — what the stamp reads."""
              where = machine.succeed(f"readlink -f {entries}").strip()
              mtime = machine.succeed(f"stat -L -c %Y {entries}").strip()
              return where, mtime

          # THE FIRST GENERATION: the profile did not exist and now does, which
          # any stamp sees. It is here so the footer's stamp is TAKEN with the
          # directory present, by the sweep this launch asks for.
          as_user("/run/current-system/sw/bin/nix profile add ${firstProbe}")
          launch("ronoctua-probe-first.desktop")
          # THE BARE NAME RAN: resolved through the footer's own PATH.
          machine.wait_until_succeeds(f"test -s {home}/ronoctua-probe-first.ran", timeout=60)
          first = resolved()

          # THE SECOND GENERATION, the case the stamp is for.
          rescans = count(RESCANNING)
          as_user("/run/current-system/sw/bin/nix profile add ${secondProbe}")
          print(as_user("/run/current-system/sw/bin/nix profile list"))
          second = resolved()
          print(f"{entries}: {first} -> {second}")
          # THE PREMISE, measured and not assumed: the directory was there
          # both times, its modification time is the store's and did not move,
          # and only where it resolves changed. Otherwise a stamp that reads
          # the time alone would pass this too.
          assert first[1] == second[1], f"the modification time moved: {first} -> {second}"
          assert first[0] != second[0], f"the profile still resolves to {first[0]}"
          launch("ronoctua-probe-second.desktop")
          machine.wait_until_succeeds(f"test -s {home}/ronoctua-probe-second.ran", timeout=60)
          assert count(RESCANNING) > rescans, (
              "the launcher found the program without noticing the profile move"
          )
          pid, _ = environ(FOOTER)
          assert pid == footer_pid, f"the footer restarted: {footer_pid} -> {pid}"
          assert restarts() == before, "the footer's unit restarted"

      with subtest("a web app opens Chrome with the profile and the address, and nothing else"):
          order = {
              "order": "web-create",
              "spec": {
                  "name": "Probe Mail",
                  "url": "https://mail.example.org/inbox",
                  "profile": "Default",
                  "icon": {"from": "file", "path": "${icon}"},
              },
          }
          scans = count(SCANNED)
          job = queue_order(order, "Probe Mail")
          assert job["stage"] == "done", job
          assert job["outcome"]["rehearsed"] is False, job
          # THE LAUNCHER HAS IT BEFORE ANYBODY ASKS: the job's end is the sweep.
          swept_since(scans, "the web app's order")
          desktop = f"{home}/.local/share/applications/ronoctua-web-probe-mail.desktop"
          fields = desktop_fields(desktop)
          print(fields)
          # THE ADDRESS NEVER GOES IN THE COMMAND: the file is the truth.
          assert fields["Exec"] == "/run/current-system/sw/bin/ronoctua-software web-open probe-mail", fields
          # The crate's constant and the table's path are one path.
          assert fields["TryExec"] == STABLE_CHROME, fields
          url = fields["X-Ronoctua-Web-Url"]
          assert url.startswith("https://mail.example.org/inbox"), fields
          assert fields["StartupWMClass"].startswith("chrome-mail.example.org"), fields
          assert is_png(fields["Icon"]), fields
          open_listed("ronoctua-web-probe-mail.desktop")
          machine.wait_until_succeeds(f"grep '^end$' {home}/chrome-argv.log > /dev/null", timeout=60)
          said = machine.succeed(f"cat {home}/chrome-argv.log").splitlines()
          print(said)
          argv0 = [line.split("=", 1)[1] for line in said if line.startswith("argv0=")]
          args = [line.split("=", 1)[1] for line in said if line.startswith("arg=")]
          # THE CHROME THE LAUNCHER LOOKS FOR is the one that opened.
          assert argv0 == [fields["TryExec"]], argv0
          assert args == ["--profile-directory=Default", f"--app={url}"], args
          for flag in ("--kiosk", "--start-fullscreen", "--user-data-dir", "--enable-features"):
              assert not any(a.startswith(flag) for a in args), f"{flag} reached Chrome: {args}"

      with subtest("an AppImage is integrated, opened and removed without a trace"):
          person = f"{DOWNLOADS}/Probe_Image-1.0-x86_64.AppImage"
          machine.succeed(f"runuser -u {user} -- mkdir -p {DOWNLOADS}")
          machine.succeed(f"runuser -u {user} -- cp ${appImage} {person}")
          touched = " ".join(
              [
                  f"{home}/Applications",
                  f"{home}/.local/share/applications",
                  f"{home}/.local/share/ronoctua/software/appimage",
                  f"{home}/.cache/appimage-run",
                  f"{home}/.cache/ronoctua/software/staging",
              ]
          )

          def files_there():
              return machine.succeed(
                  f"find {touched} -mindepth 1 -not -type d 2>/dev/null | sort || true"
              ).split()

          before = files_there()
          scans = count(SCANNED)
          job = queue_order(
              {"order": "install-file", "path": person, "consent": {"unverified": True}},
              "Probe Image",
          )
          assert job["stage"] == "done", job
          assert job["outcome"]["rehearsed"] is False, job
          swept_since(scans, "the AppImage's order")
          key = job["outcome"]["key"]["key"]
          assert key == "probe-image", job
          image = f"{home}/Applications/{key}.AppImage"
          desktop = f"{home}/.local/share/applications/ronoctua-appimage-{key}.desktop"
          kept = f"{home}/.local/share/ronoctua/software/appimage/{key}"
          machine.succeed(f"test -x {image}")
          fields = desktop_fields(desktop)
          print(fields)
          assert fields["Exec"] == f'"/run/current-system/sw/bin/appimage-run" "{image}" %F', fields
          assert fields["TryExec"] == "/run/current-system/sw/bin/appimage-run", fields
          assert fields["Name"] == "Probe Image", fields
          assert fields["X-Ronoctua-AppImage"] == key, fields
          assert fields["Icon"].startswith(kept + "/"), fields
          assert is_png(fields["Icon"]), fields
          sidecar = json.loads(machine.succeed(f"cat {kept}/app.json"))
          print(sidecar)
          sha = machine.succeed(f"sha256sum {image}").split()[0]
          assert sidecar["sha256"] == sha, sidecar
          assert sidecar["verified"] is False, sidecar
          # THE PERSON'S FILE STAYS where it was: it was copied, not taken.
          machine.succeed(f"test -f {person}")

          open_listed(f"ronoctua-appimage-{key}.desktop")
          try:
              machine.wait_until_succeeds(f"test -s {home}/probe-appimage.ran", timeout=180)
          except Exception:
              print(machine.execute(f"journalctl --no-pager -a -o cat _UID={uid} | tail -60")[1])
              raise
          extraction = f"{home}/.cache/appimage-run/{sha}"
          machine.succeed(f"test -d {extraction}")

          job = queue_order(
              {"order": "remove", "key": {"source": "appimage", "key": key}}, "Probe Image"
          )
          assert job["stage"] == "done", job
          assert job["outcome"]["rehearsed"] is False, job
          for gone in (image, desktop, kept, extraction):
              machine.fail(f"test -e {gone}")
          after = files_there()
          assert after == before, (
              f"left behind: {sorted(set(after) - set(before))}, "
              f"taken: {sorted(set(before) - set(after))}"
          )
          named = machine.succeed(
              f"find {home} -path {DOWNLOADS} -prune -o -iname '*probe-image*' -print"
          ).split()
          assert not named, f"still named after it: {named}"
          print("empty directories left: " + machine.succeed(
              f"find {touched} -type d -empty 2>/dev/null || true"
          ))

      # LAST OF THE FIVE, because it is the one that asks the footer's scopes
      # to work at all, and a failure here must not hide what the others say.
      with subtest("an app that exits with 3 runs once, in its scope"):
          machine.succeed(f"runuser -u {user} -- mkdir -p {home}/.local/share/applications")
          machine.succeed(
              f"runuser -u {user} -- cp ${exitDesktop} "
              f"{home}/.local/share/applications/ronoctua-probe-exit.desktop"
          )
          launch("ronoctua-probe-exit.desktop")

          def about_the_probe(said):
              return [
                  line
                  for line in journal_of(FOOTER).splitlines()
                  if "ronoctua-probe-exit" in line and said in line
              ]

          # THE LAUNCH'S OWN VERDICT, one per launch: `foreign buffer opened`
          # in a scope, `opened WITHOUT a cgroup` without one. The footer logs
          # it once the launch has returned, so after any relaunch it started:
          # one in the scope was waited for before the verdict, and one outside
          # it makes the verdict the second kind.
          def verdicts():
              return about_the_probe("foreign buffer opened") + about_the_probe(
                  "opened WITHOUT a cgroup"
              )

          assert wait_for(verdicts, timeout=60), (
              "the launch never came back:\n" + journal_of(FOOTER)[-3000:]
          )
          said = verdicts()
          print(said)
          print(about_the_probe("the application ended"))
          # AND THE SCOPE WAS THERE: without one the app is started directly
          # and once anyway, which would pass this for the wrong reason.
          assert not about_the_probe("opened WITHOUT a cgroup"), said
          assert len(said) == 1, said
          assert about_the_probe("status=exit status: 3"), "the footer never heard the app's own status"
          runs = int(machine.succeed(f"wc -l < {home}/probe-exit.runs").strip())
          assert runs == 1, f"the app ran {runs} times"

      with subtest("the footer never restarted"):
          pid, _ = environ(FOOTER)
          assert pid == footer_pid, f"the footer restarted: {footer_pid} -> {pid}"
          assert restarts() == restarts_at_start, (
              f"the footer's unit restarted: {restarts_at_start} -> {restarts()}"
          )
    '';
}
