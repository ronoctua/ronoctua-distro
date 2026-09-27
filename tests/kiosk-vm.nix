# THE CHROME KIOSK, with the REAL Chrome, on a booted session, offline.
#
# WHY A REAL CHROME, AND WHY ONLY HERE
# ------------------------------------
# Everything between the footer and Chrome is two programs this repository
# does not write — Chrome and its policy engine — and a protocol it only
# half controls. A stand-in would agree with our own reading of all three.
# Chrome is unfree and a test node's `pkgs` admits nothing unfree, so it is
# let in BY NAME here, and alone: the `uade` precedent in
# `nix/player-tools.nix`. It is the same pinned build the system installs,
# handed in through `ronoctua.browser.package` with flags for a machine with
# no card and no first run — and never one of the four `web-apps-are-never-
# kiosk` refuses.
#
# WHAT IT PROVES, in order, each a thing that fails in silence:
#
#   A. the extension is FORCE-INSTALLED from `file://`, offline, by the
#      policy the packing unit writes — the step that used to be the owner's
#      to confirm, and that could never have worked: the directory holding
#      `update.xml` was `0700 root`;
#   B. a download Chrome starts reaches the QUEUE — through Chrome's own call
#      of the transfers host, whose first argument is the extension's origin
#      and which used to die on it;
#   C. `kiosk-open` opens a window whose title carries the nonce (read from
#      the compositor's window events), the footer pairs it (`kiosk.json`)
#      and the page goes to its address;
#   D. the compositor's application fullscreen HIDES Chrome's bars: the page's
#      `innerHeight` is the height of the window's content (inside the
#      border), and `kiosk-chrome-ui` gives back at least 60 px of bars and
#      takes them away again;
#   E. the footer drives the tabs: a new tab at an address, the previous one,
#      a move, the next one, and the zoom (the page's `devicePixelRatio`);
#   F. a footer that restarts is found again by the host, and drives on;
#   G. closing the kiosk ends the host: it lives exactly as long as the port.
#
# THE PAGES report what they see in their TITLE (`X <innerHeight>
# <innerWidth> <devicePixelRatio>`), which the compositor's tree carries —
# so nothing is read out of Chrome but what any window manager sees.
#
# MEASURED FIRST, in the experiments of this feature (Chrome 151, Scroll, a
# 512x768 tile): with the application fullscreen the page's `innerHeight` was
# 768 against a window of 768, and 681 without it — 87 px of tab strip and
# toolbar.
{
  pkgs,
  lib,
  inputs,
  ronoctua,
}:

let
  # THE STABLE PATH the footer opens Chrome by, from the table that gives it.
  chromeStable =
    (import ../nix/software-tools.nix {
      inherit lib pkgs;
      nix = null;
      index = null;
      nixpkgsRev = null;
      chrome = true;
    }).RONOCTUA_CHROME;

  # CHROME, ADMITTED BY NAME and alone — see the header.
  unfree = import pkgs.path {
    inherit (pkgs.stdenv.hostPlatform) system;
    config.allowUnfreePredicate = p: lib.getName p == "google-chrome";
  };
  chrome = unfree.google-chrome.override {
    commandLineArgs = [
      # A machine that never saw Chrome: no welcome page, no question.
      "--no-first-run"
      "--no-default-browser-check"
      # No card: Chrome's compositor in software, through `wl_shm`.
      "--disable-gpu"
      # The two the system's own wrapper passes (`nix/modules/browser.nix`).
      "--password-store=basic"
      "--ozone-platform-hint=auto"
    ];
  };

  hosts = import ../nix/browser-hosts.nix;

  # THE PAGES, and a file to download. Each page says its size in its title,
  # at load, at every resize and twice a second (a zoom is not a resize).
  pages = pkgs.runCommand "ronoctua-kiosk-pages" { } ''
    mkdir -p $out
    for p in a b c; do
      P=$(echo $p | tr a-z A-Z)
      cat > $out/$p.html <<EOF
    <!doctype html><meta charset="utf-8"><title>$P</title>
    <body style="margin:0"><h1>$P</h1>
    <script>
    function say(){document.title='$P '+innerHeight+' '+innerWidth+' '+devicePixelRatio;}
    onload=say; onresize=say; setInterval(say, 500);
    </script>
    EOF
    done
    head -c 300000 /dev/zero > $out/file.bin
  '';

  # THE QUEUE'S OWN PROTOCOL, spoken here and not through our client — the
  # reason `tests/software-vm.nix` gives.
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

  # NO MACHINE UNDER TEST SCHEDULES A REBUILD OF ITSELF (`tests/compositor-xvfb.nix`).
  brainPrefs = pkgs.writeText "brain.json" ''{"tasks":{"system-update":{"enabled":false}}}'';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-kiosk";

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
        ../nix/modules/browser-extension.nix
        inputs.home-manager.nixosModules.home-manager
      ];
      _module.args.ronoctua = ronoctua;

      # Chrome and the session: 4 GiB, measured to hold both with room.
      virtualisation.memorySize = 4096;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-kiosk";
      users.users.root.hashedPassword = lib.mkForce null;
      # One attrset: a dynamic attribute in two sentences does not evaluate.
      users.users.${user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };

      ronoctua.media.enable = false;
      ronoctua.icons.superpack = false;
      ronoctua.update.enable = false;
      # THE REAL CHROME, at the stable path everything opens it by.
      ronoctua.browser.package = chrome;
      # The variable the software tools give the footer when the browser is
      # on (`software.nix` is not under test here).
      systemd.user.services.ronoctua-footer.environment.RONOCTUA_CHROME = chromeStable;

      # OFFLINE, and saying so.
      nix.settings.substituters = lib.mkForce [ ];

      fonts.enableDefaultPackages = false;
      fonts.packages = [ ronoctua.fonts.noctua-iosevka-core ];

      environment.systemPackages = [ pkgs.python3 ];

      # THE SCREEN: the compositor on Xvfb (`tests/compositor-xvfb.nix`).
      systemd.services.xvfb = {
        description = "The screen this test's compositor draws on";
        wantedBy = [ "multi-user.target" ];
        before = [ "systemd-user-sessions.service" ];
        serviceConfig = {
          ExecStart = "${pkgs.xorg-server}/bin/Xvfb :99 -screen 0 1600x1000x24 -ac -nolisten tcp";
          ExecStartPost = "${pkgs.bash}/bin/bash -c 'until [ -S /tmp/.X11-unix/X99 ]; do sleep 0.1; done'";
        };
      };
      systemd.user.services.scroll.environment = {
        WLR_BACKENDS = "x11";
        DISPLAY = ":99";
        WLR_RENDERER = "pixman";
      };

      # THE PAGES, on the loopback: nothing leaves the machine.
      systemd.services.kiosk-pages = {
        description = "The pages the kiosk is sent to";
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 -m http.server 8000 --bind 127.0.0.1 --directory ${pages}";
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
      import time

      user = "${nodes.machine.ronoctua.user}"
      home = f"/home/{user}"
      FOOTER = "ronoctua-footer.service"
      QUEUE = "ronoctua-transfers.service"
      SHELL = "${ronoctua.shell}/bin/ronoctua-shell"
      CHROME = "${chromeStable}"
      MSG = "${ronoctua.scroll}/bin/scrollmsg"
      PAGES = "http://127.0.0.1:8000"
      NONCE_TITLE = re.compile(r"^[0-9a-f]{32} - Google Chrome$")
      ANSI = re.compile(r"\x1b\[[0-9;]*m")


      def user_ctl(args):
          return machine.succeed(f"systemctl --machine={user}@ --user {args}")


      def as_user(command, unit):
          """A program started as the person, in their manager's environment —
          which the session filled with WAYLAND_DISPLAY and XDG_SESSION_TYPE."""
          return machine.succeed(
              f"systemd-run --machine={user}@ --user --collect --unit={unit} -- {command}"
          )


      def journal_of(unit):
          raw = machine.succeed(
              f"journalctl --no-pager -a -o cat _SYSTEMD_USER_UNIT={unit} || true"
          )
          return ANSI.sub("", raw)


      def diagnose():
          for unit in ("scroll.service", FOOTER, QUEUE):
              print(machine.execute(f"systemctl --machine={user}@ --user status {unit} --no-pager -l")[1])
          print(machine.execute(f"journalctl --no-pager -a -o cat _UID=$(id -u {user}) | tail -150")[1])


      def wait_for(what, timeout=60):
          end = time.time() + timeout
          while time.time() < end:
              found = what()
              if found:
                  return found
              time.sleep(0.5)
          return what()


      def command(line):
          """A command in the RUNNING footer, through its control socket."""
          machine.succeed(
              f"runuser -u {user} -- env XDG_RUNTIME_DIR={runtime} "
              f"WAYLAND_DISPLAY={display} {SHELL} command {line}"
          )


      def ask_queue(body):
          out = machine.succeed(
              f"runuser -u {user} -- ${pkgs.python3}/bin/python3 ${queueClient} "
              f"{runtime}/ronoctua-transfers.sock {shlex.quote(json.dumps(body))}"
          )
          return json.loads(out.strip())


      def windows():
          said = machine.succeed(f"SCROLLSOCK={sock} SWAYSOCK={sock} {MSG} -t get_tree")
          out = []

          def walk(node):
              if node.get("pid"):
                  out.append(node)
              for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                  walk(kid)

          walk(json.loads(said))
          return out


      def page(con, prefix):
          """What the page in window `con` says: (innerHeight, innerWidth,
          devicePixelRatio, the height of the window's CONTENT), when its title
          is `prefix`'s. The content is `window_rect`, inside the border the
          system draws (`default_border pixel 2`): `rect` is the container
          with that border, 4 px taller than anything Chrome can fill."""
          for w in windows():
              if w["id"] != con:
                  continue
              m = re.match(rf"{prefix} (\d+) (\d+) ([0-9.]+) - Google Chrome$", w.get("name") or "")
              if m:
                  return (
                      int(m.group(1)),
                      int(m.group(2)),
                      float(m.group(3)),
                      w["window_rect"]["height"],
                  )
          return None


      def settled(con, prefix, test=lambda p: True, timeout=30):
          seen = wait_for(lambda: (lambda p: p if p and test(p) else None)(page(con, prefix)), timeout)
          assert seen, (
              f"the kiosk never showed {prefix} as asked: "
              f"{[(w['id'], w.get('name'), w['rect'], w.get('window_rect')) for w in windows()]}"
          )
          return seen


      def host_pids():
          return machine.execute(f"pgrep -u {user} -f 'bin/ronoctua-browser '")[1].split()


      machine.start()
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit("xvfb.service")
      machine.wait_for_unit("kiosk-pages.service")
      machine.wait_for_unit("ronoctua-browser-extension.service")
      uid = machine.succeed(f"id -u {user}").strip()
      runtime = f"/run/user/{uid}"
      ID = machine.succeed("cat /var/lib/ronoctua-browser/id").strip()

      with subtest("the login brings the session up: compositor, footer and queue"):
          try:
              machine.wait_until_succeeds(
                  f"systemctl --machine={user}@ --user is-active {FOOTER}", timeout=300
              )
              machine.wait_until_succeeds(
                  f"systemctl --machine={user}@ --user is-active {QUEUE}", timeout=60
              )
              machine.wait_until_succeeds(
                  f"journalctl -a -o cat _SYSTEMD_USER_UNIT={FOOTER} "
                  "| grep 'control socket opened' > /dev/null",
                  timeout=120,
              )
          except Exception:
              diagnose()
              raise
          manager = user_ctl("show-environment")
          print(manager)
          found_display = re.search(r"^WAYLAND_DISPLAY=(\S+)$", manager, re.M)
          found_sock = re.search(r"^SCROLLSOCK=(\S+)$", manager, re.M)
          assert found_display and found_sock, f"the compositor exported neither:\n{manager}"
          display = found_display.group(1)
          sock = found_sock.group(1)
          # THE SESSION TYPE, which Chrome's `--ozone-platform-hint=auto`
          # reads — and not `WAYLAND_DISPLAY`. Without it Chrome chose X11,
          # found no `DISPLAY` and exited, in this test and in a session.
          assert re.search(r"^XDG_SESSION_TYPE=wayland$", manager, re.M), (
              f"the compositor never exported the session type:\n{manager}"
          )
          # The footer is told where our extension's id is written — and it
          # is the environment `kiosk-open` starts Chrome with (a scope keeps
          # its caller's).
          footer_env = machine.succeed(
              f"tr '\\0' '\\n' < /proc/{user_ctl('show -p MainPID --value ' + FOOTER).strip()}/environ"
          )
          print(footer_env)
          assert "RONOCTUA_BROWSER_MANIFEST=/etc/opt/chrome/native-messaging-hosts/${hosts.browser}.json" in footer_env, footer_env
          assert "XDG_SESSION_TYPE=wayland" in footer_env.splitlines(), footer_env

      with subtest("A: the policy force-installs the extension, from file://, offline"):
          started = time.time()
          as_user(f"{CHROME} about:blank", "chrome-first")
          installed = wait_for(
              lambda: machine.execute(
                  f"test -d {home}/.config/google-chrome/Default/Extensions/{ID}"
              )[0] == 0,
              timeout=60,
          )
          if not installed:
              diagnose()
              print("chrome's own log:\n" + journal_of("chrome-first.service")[-4000:])
              print(machine.execute("ls -la /var/lib/ronoctua-browser /etc/opt/chrome/policies/managed")[1])
          assert installed, f"Chrome did not install {ID} within 60 s"
          print(f"installed in {time.time() - started:.1f} s")

      with subtest("B: a download Chrome starts lands in the queue, through Chrome's own call of the host"):
          as_user(f"{CHROME} {PAGES}/file.bin", "chrome-download")

          def queued():
              jobs = ask_queue({"ask": "list"}).get("jobs", [])
              return [j for j in jobs if "/file.bin" in json.dumps(j)]

          jobs = wait_for(queued, timeout=60)
          assert jobs, (
              "the queue never received the download: the host Chrome starts "
              f"(argv[1] = chrome-extension://{ID}/) did not take it\n" + journal_of(QUEUE)[-2000:]
          )
          print(json.dumps(jobs, indent=1))

      with subtest("C: kiosk-open opens a window with the nonce in its title, pairs it, and goes"):
          machine.succeed(
              f"systemd-run --unit=kiosk-watch --setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
              f"${pkgs.bash}/bin/bash -c '{MSG} -t subscribe -m \"[\\\"window\\\"]\" > /tmp/windows.log'"
          )
          machine.wait_until_succeeds("test -e /tmp/windows.log", timeout=30)
          command(f"kiosk-open {PAGES}/a.html")

          def titles():
              out = []
              for line in machine.succeed("cat /tmp/windows.log").splitlines():
                  try:
                      out.append(json.loads(line).get("container", {}).get("name") or "")
                  except ValueError:
                      pass
              return out

          nonce_titles = wait_for(lambda: [t for t in titles() if NONCE_TITLE.match(t)], timeout=60)
          assert nonce_titles, f"no window ever carried a nonce in its title: {titles()}"
          nonce = nonce_titles[0].split(" ")[0]
          kiosk = wait_for(
              lambda: next((w for w in windows() if (w.get("name") or "").startswith("A ")), None),
              timeout=60,
          )
          assert kiosk, f"the kiosk never went to its page: {titles()}"
          assert kiosk["app_id"] == "google-chrome", kiosk["app_id"]
          con = kiosk["id"]
          after = titles()
          assert after.index(nonce_titles[0]) < max(
              i for i, t in enumerate(after) if t.startswith("A ")
          ), "the page left before the compositor saw the nonce"
          pairs = machine.succeed(f"cat {runtime}/ronoctua-kiosk.json")
          assert nonce in pairs, f"the footer did not keep the pair: {pairs}"
          print("paired lines:", [l for l in journal_of(FOOTER).splitlines() if "paired" in l])

      with subtest("D: the application fullscreen hides Chrome's bars, and kiosk-chrome-ui shows them"):
          hidden = settled(con, "A", lambda p: abs(p[0] - p[3]) <= 2)
          print(f"bars hidden: innerHeight {hidden[0]}, window content {hidden[3]}")
          command("kiosk-chrome-ui")
          shown = settled(con, "A", lambda p: p[0] <= hidden[0] - 60)
          print(f"bars shown: innerHeight {shown[0]}, window content {shown[3]}")
          command("kiosk-chrome-ui")
          again = settled(con, "A", lambda p: abs(p[0] - p[3]) <= 2)
          assert again[0] == hidden[0], f"{again} after {hidden}"

      with subtest("E: new tabs, the previous one, a move, the next one, and the zoom"):
          command(f"kiosk-new-tab {PAGES}/b.html")
          settled(con, "B")
          command(f"kiosk-new-tab {PAGES}/c.html")
          settled(con, "C")
          command("kiosk-tab-previous")
          settled(con, "B")
          command("kiosk-tab-previous")
          settled(con, "A")
          # [A, B, C] with A in front; A moves right: [B, A, C]. The next tab
          # is C only if A really moved — without the move it would be B.
          command("kiosk-tab-right")
          time.sleep(2)
          command("kiosk-tab-next")
          settled(con, "C")
          command("kiosk-zoom-in")
          zoomed = settled(con, "C", lambda p: abs(p[2] - 1.1) < 0.01)
          print(f"zoomed: {zoomed}")

      with subtest("F: a footer that restarts is found again, and drives on"):
          hosts_before = host_pids()
          assert hosts_before, "no kiosk host is running"
          opened = journal_of(FOOTER).count("control socket opened")
          user_ctl(f"restart {FOOTER}")
          machine.wait_until_succeeds(
              f"test $(journalctl -a -o cat _SYSTEMD_USER_UNIT={FOOTER} "
              f"| grep -c 'control socket opened') -gt {opened}",
              timeout=120,
          )
          assert host_pids() == hosts_before, "the host did not survive the footer's restart"
          # [B, A, C] with C in front: the previous one is A.
          deadline = time.time() + 60
          while page(con, "A") is None and time.time() < deadline:
              command("kiosk-tab-previous")
              time.sleep(3)
          settled(con, "A")

      with subtest("G: closing the kiosk ends the host"):
          machine.succeed(f"SCROLLSOCK={sock} SWAYSOCK={sock} {MSG} '[con_id={con}] kill'")
          ended = wait_for(lambda: not host_pids(), timeout=60)
          assert ended, f"the host outlived its kiosk: {host_pids()}"
          machine.succeed("systemctl stop kiosk-watch.service || true")
    '';
}
