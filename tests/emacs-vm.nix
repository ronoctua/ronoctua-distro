# THE EDITOR ON THE DISTRO'S COMPOSITOR: the daemon, its two kinds of client,
# and what becomes of it when a session ends.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# `nix/modules/emacs.nix` declares one package, and its daemon is a unit of
# the person's manager at `default.target` — up before any session, with no
# display at all. Whether that package then opens a WAYLAND window for a
# client, which shell the compositor sees the window come through, and what
# the daemon does when the compositor it drew on goes away are facts of GTK,
# of Emacs and of this compositor together, and nothing short of all three
# running answers them:
#
#   1. the daemon is up at `default.target` with no display in its
#      environment, before any compositor exists;
#   2. a terminal client (`emacsclient -t`, the `e` alias and `$EDITOR`)
#      works with no display;
#   3. a graphical client started the way the session starts one — a unit of
#      the person's manager, whose environment is what the compositor's
#      session line imported (`WAYLAND_DISPLAY`, and XWayland's `DISPLAY`
#      beside it) — opens a frame through `xdg_shell`, with the `app_id` the
#      launcher entries the registry lists declare, while XWayland (lazy, as
#      in the session) never starts;
#   4. the daemon survives that frame being closed;
#   5. what the daemon does when the session's compositor ends — with its
#      frame closed, with its frame open, and when it never drew on that
#      compositor — and that the next session gets a frame either way;
#   6. the nested loop's client (`nix/dev.nix`, the very script) opens its
#      frame in the session the loop was started in, never in the nested
#      compositor, with the session's two displays and not the loop's, and
#      the nested compositor's end leaves the daemon alone;
#   7. a stop of the person's manager (a power-off, a reboot, the last
#      logout) with a frame open and an edit unsaved stops the daemon first:
#      `kill-emacs` runs, with its hook and its auto-save, every time.
#
# WHAT IS MEASURED, AND HELD
# --------------------------
# Section 5 is a measurement (`docs/ARCHITECTURE.md`, the Emacs section): a
# GTK Emacs never lets go of a display it opened a frame on — `frame.c`
# keeps the terminal of a GTK build alive after its last frame — so a
# compositor that ends takes that display with it, and with it the daemon.
# The test holds what was measured: a daemon that drew on the compositor
# dies with it and its unit brings it back; one that never drew there lives
# on. Should GTK or Emacs learn to let a display go, the first assertion
# fails, and the paragraph that says buffers are lost with the session is
# the one to rewrite.
#
# WHY THE COMPOSITOR IS A UNIT OF THE PERSON'S, AND NOT ROOT'S
# ------------------------------------------------------------
# The other VM tests run it as root (`compositor-xvfb.nix`); the daemon here
# is the person's, and its socket and the compositor's have to be in the same
# runtime directory, as they are in the session (`scroll.service`, a user
# unit). Its config carries the session's own line that imports the displays
# into the manager (`nix/session-environment.nix`, the one the installed
# config splices in), which is the only way a client started from the
# session learns them.
#
# Section 7 runs the compositor under the session's own unit name
# (`scroll.service`, `nix/modules/scroll.nix`), because that name is what the
# daemon's unit is ordered after — once per manager, which it stops each
# time. The earlier sections give each compositor a name of its own.
#
# The brain's watch on the daemon proves its half in its own folder (its
# `vm.nix`, run by `tests/brain-modules-vm.nix`).
{
  pkgs,
  lib,
  scroll,
  fonts,
  ronoctua,
  # The nested loop's editor client and the variable it reads the session's
  # display from (`passthru.editor` of `nix/dev.nix`).
  loopEditor,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  gpu = import ../nix/software-gpu.nix { inherit pkgs lib; };
  sessionEnvironment = import ../nix/session-environment.nix { inherit pkgs; };

  # THE SESSION'S LINE, the installed config's own, and XWayland as the
  # session has it: lazy (the compositor's default, written out), so a frame
  # that went through X would have started it — with `DISPLAY` exported, a
  # client that chose X would find it. The rest is the minimal test config's.
  session = pkgs.writeText "compositor-emacs-test.conf" ''
    animations enabled no
    default_border none
    gaps inner 0
    gaps outer 0
    titlebar_padding 1
    output * scale 1
    scrollnag_command -
    xwayland enable
    ${sessionEnvironment.lines}
  '';

  # THE NESTED LOOP'S COMPOSITOR, as `nix/dev.nix` runs it: a window of the
  # session's compositor (the Wayland backend) with its own lazy XWayland,
  # saying its two displays the way the loop's config does, and importing
  # nothing into the manager.
  nestedSession = pkgs.writeText "compositor-emacs-nested.conf" ''
    animations enabled no
    default_border none
    output * scale 1
    scrollnag_command -
    xwayland enable
    exec sh -c 'printf "%s %s" "$WAYLAND_DISPLAY" "$DISPLAY" > "$NESTED_DISPLAY_FILE"'
  '';

  # A TERMINAL FOR A TERMINAL CLIENT. A unit has none, and `emacsclient -t`
  # opens its frame on the terminal it is given; this hands it a pty and waits
  # for it. Python's and not `script`'s: the unit's stdin is `/dev/null`, and
  # `pty.spawn` stops reading it at its end instead of passing the end on.
  pty = pkgs.writeText "emacs-vm-pty.py" ''
    import os
    import pty
    import sys

    status = pty.spawn(sys.argv[1:])
    sys.exit(os.waitstatus_to_exitcode(status))
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-emacs";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/emacs.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-emacs";
      users.users.root.hashedPassword = lib.mkForce null;

      # THE PERSON'S MANAGER AT BOOT, with nobody logged in: `default.target`
      # is reached with no session and no display, which is the daemon's
      # whole question.
      users.users.${config.ronoctua.user}.linger = true;

      environment.systemPackages = [
        scroll
        pkgs.procps
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
      emacs = machine.services.emacs.package;
    in
    ''
      ${xvfb.prelude}
      import json
      import re
      import shlex

      user = "${machine.ronoctua.user}"
      # THE DAEMON'S UNIT, as the editor's module names it.
      daemon_unit = "${machine.systemd.user.services.emacs.name}"
      client = "${emacs}/bin/emacsclient"
      # THE NESTED LOOP'S CLIENT, and the variable the loop keeps the
      # session's display in.
      loop_client = "${loopEditor.client}"
      loop_variable = "${loopEditor.variable}"
      loop_x_variable = "${loopEditor.xVariable}"
      # The launcher entries the registry lists for the editor.
      entries = ${builtins.toJSON machine.ronoctua.apps.emacs.desktopIds}


      def manager(args):
          return machine.succeed(f"systemctl --user --machine={user}@ {args}")


      def facts(unit):
          """A unit of the person's manager, as systemd tells it."""
          said = manager(
              f"show {unit} -p ActiveState -p MainPID -p NRestarts -p ExecMainCode "
              "-p ExecMainStatus -p Result -p WantedBy"
          )
          return dict(line.split("=", 1) for line in said.splitlines() if "=" in line)


      def daemon():
          """`(pid, restarts)` of the daemon, once systemd says it is active."""
          machine.wait_until_succeeds(
              f"systemctl --user --machine={user}@ is-active {daemon_unit}", timeout=120
          )
          f = facts(daemon_unit)
          return int(f["MainPID"]), int(f["NRestarts"])


      def in_session(unit, command, *env):
          """`command` as a program the footer opens: a unit of the person's
          manager, whose environment is the manager's — what the session's
          line imported, and nothing else. It stays loaded when it ends, so
          its exit can be read."""
          sets = " ".join(f"--setenv={e}" for e in env)
          machine.succeed(
              f"systemd-run --user --machine={user}@ --unit={unit} --quiet "
              f"--property=RemainAfterExit=yes {sets} -- {command}"
          )


      def ask_emacs(expr):
          """What the daemon answers to `expr`, as `emacsclient --eval` prints it."""
          return machine.succeed(
              f"runuser -u {user} -- env XDG_RUNTIME_DIR={run_dir} {client} --eval "
              + shlex.quote(expr)
          ).strip()


      FRAMES = (
          "(mapcar (lambda (f) (format \"%s %s\" (framep f) "
          "(terminal-name (frame-terminal f)))) (frame-list))"
      )


      def frames():
          """Every frame of the daemon, as `<window system> <terminal>`."""
          return re.findall(r"\"([^\"]*)\"", ask_emacs(FRAMES))


      def start_session(n, unit=None):
          """A compositor of the person's, on the Xvfb, as a session. Its unit
          is `session<n>` unless named; `--collect`, so a compositor that left
          failed never holds the name."""
          unit = unit or f"session{n}"
          sock = f"{run_dir}/scroll-ipc.{unit}.sock"
          machine.succeed(f"rm -f {run_dir}/wayland-1 {run_dir}/wayland-1.lock {sock}")
          machine.succeed(
              f"systemd-run --user --machine={user}@ --unit={unit} --quiet --collect "
              "--setenv=DISPLAY=:99 --setenv=WLR_BACKENDS=x11 --setenv=WLR_X11_OUTPUTS=1 "
              "${gpu.env} "
              f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
              "--setenv=XDG_SESSION_TYPE=wayland --setenv=PATH=${pkgs.bash}/bin "
              "${scroll}/bin/scroll -c ${session}"
          )
          machine.wait_until_succeeds(f"test -S {run_dir}/wayland-1", timeout=90)
          machine.wait_until_succeeds(f"test -S {sock}", timeout=60)
          # THE SESSION'S LINE RAN: the display is in the manager.
          machine.wait_until_succeeds(
              f"systemctl --user --machine={user}@ show-environment"
              " | grep -x WAYLAND_DISPLAY=wayland-1 > /dev/null",
              timeout=60,
          )
          return unit, sock


      def session_display():
          """XWayland's display, as the session line put it in the manager."""
          env = manager("show-environment")
          found = re.search(r"^DISPLAY=(:[0-9]+)$", env, re.M)
          assert found, "the session did not export XWayland's display:\n" + env
          return found.group(1)


      def end_session(unit):
          manager(f"stop {unit}")
          machine.wait_until_fails(
              f"systemctl --user --machine={user}@ is-active {unit}", timeout=30
          )


      def windows(sock):
          """Every window of the compositor, as the tree has it."""
          said = json.loads(machine.succeed(
              f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree"
          ))
          found = []

          def walk(node):
              if node.get("pid"):
                  found.append(node)
              for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                  walk(kid)

          walk(said)
          return found


      def start_nested():
          """The nested loop's compositor, a window of the session's: its
          unit, its socket and the two displays it says it took."""
          unit = "nested"
          sock = f"{run_dir}/scroll-ipc.{unit}.sock"
          said = f"{run_dir}/nested-display"
          machine.succeed(f"rm -f {said} {sock}")
          machine.succeed(
              f"systemd-run --user --machine={user}@ --unit={unit} --quiet --collect "
              "--setenv=WAYLAND_DISPLAY=wayland-1 --setenv=WLR_BACKENDS=wayland "
              "${gpu.env} "
              f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
              f"--setenv=NESTED_DISPLAY_FILE={said} "
              "--setenv=XDG_SESSION_TYPE=wayland --setenv=PATH=${pkgs.bash}/bin "
              "${scroll}/bin/scroll -c ${nestedSession}"
          )
          machine.wait_until_succeeds(f"test -s {said}", timeout=90)
          machine.wait_until_succeeds(f"test -S {sock}", timeout=60)
          display, x_display = (machine.succeed(f"cat {said}").split() + [""])[:2]
          assert display and display != "wayland-1", f"the nested compositor took `{display}`"
          return unit, sock, display, x_display


      def window_in(places, pid, timeout=90):
          """`(name, window)`: the first of `places` (name → socket) whose
          compositor has a window process `pid` drew."""
          for _ in range(timeout):
              for name, sock in places.items():
                  mine = [w for w in windows(sock) if w.get("pid") == pid]
                  if mine:
                      return name, mine[0]
              machine.sleep(1)
          raise AssertionError(f"no window of pid {pid} in {sorted(places)} after {timeout}s")


      def journal_cursor():
          said = machine.succeed("journalctl -n 1 --show-cursor --no-pager -o cat")
          return said.strip().splitlines()[-1].split("cursor: ", 1)[1]


      def window_of(sock, pid, timeout=90):
          """The window process `pid` drew, once the compositor has it."""
          for _ in range(timeout):
              mine = [w for w in windows(sock) if w.get("pid") == pid]
              if mine:
                  return mine[0]
              machine.sleep(1)
          raise AssertionError(
              f"no window of pid {pid} after {timeout}s; the compositor has: "
              + json.dumps([(w.get("pid"), w.get("app_id"), w.get("shell")) for w in windows(sock)])
              + "\n" + machine.execute(f"journalctl --no-pager _UID=$(id -u {user}) | tail -60")[1]
          )


      def open_frame(sock, n):
          """`emacsclient -c <file>` from the session, as the file manager runs
          it (`launch::editor_command`); the window, checked to be native."""
          pid, _ = daemon()
          in_session(f"gui{n}", f"{client} -c /home/{user}/notes-{n}.txt")
          w = window_of(sock, pid)
          print(f"frame of session {n}: " + json.dumps(
              {k: w.get(k) for k in ("app_id", "shell", "pid", "name", "window_properties")}
          ))
          assert w.get("shell") == "xdg_shell", (
              f"the frame came through `{w.get('shell')}`, not the compositor's own shell: {w}"
          )
          # THE NAME THE LAUNCHER ENTRIES DECLARE (`StartupWMClass=Emacs`),
          # ignoring case. Nothing in the shell matches an Emacs window to its
          # entry: `ronoctua_apps::declared_window` is asked only about
          # Chrome's windows (`daemon::mode_label`), and the modeline shows the
          # `app_id`'s last dotted part, `emacs`. The class agreeing is what a
          # program that does match windows to entries by it would need.
          classes = {}
          for entry in entries:
              text = machine.succeed(f"cat ${emacs}/share/applications/{entry}")
              m = re.search(r"^StartupWMClass=(.*)$", text, re.M)
              if m:
                  classes[entry] = m.group(1).strip()
          assert classes, f"no entry of {entries} declares a window class"
          for entry, wm_class in classes.items():
              assert (w.get("app_id") or "").lower() == wm_class.lower(), (
                  f"the window says `{w.get('app_id')}` and {entry} declares `{wm_class}`"
              )
          # AND NO X CLIENT: the lazy XWayland never started.
          assert machine.execute("pgrep -x Xwayland")[0] != 0, (
              "XWayland is running: something drew through X\n"
              + machine.succeed("ps -eo pid,user,args")
          )
          # EMACS AGREES: a frame of the pure-GTK window system. (The terminal
          # is GDK's name for the display, not the socket's.)
          assert any(f.startswith("pgtk ") for f in frames()), frames()
          return w


      def fate(before, since):
          """What a session's end did to the daemon: `died` and what was said."""
          machine.sleep(5)
          after = daemon()
          said = machine.succeed(
              f"journalctl --no-pager --since=@{since} "
              f"_SYSTEMD_USER_UNIT={daemon_unit} + USER_UNIT={daemon_unit}"
          )
          dumps = machine.execute(f"coredumpctl list --no-pager --since=@{since}")[1]
          died = after[0] != before[0] or after[1] != before[1]
          print(f"before {before}, after {after}, died: {died}\n{said}\n{dumps}")
          return died


      def now():
          return int(machine.succeed("date +%s").strip())


      machine.start()
      machine.wait_for_unit("multi-user.target")
      uid = machine.succeed(f"id -u {user}").strip()
      run_dir = f"/run/user/{uid}"
      machine.wait_for_unit(f"user@{uid}.service")
      start_xvfb()

      with subtest("the daemon is up at default.target, with no display"):
          pid, _ = daemon()
          f = facts(daemon_unit)
          assert "default.target" in f["WantedBy"].split(), f
          # NO SESSION YET, and nothing in the daemon's environment names one.
          assert machine.execute(
              f"systemctl --user --machine={user}@ is-active graphical-session.target"
          )[0] != 0
          environ = machine.succeed(f"tr '\\0' '\\n' < /proc/{pid}/environ")
          for name in ("WAYLAND_DISPLAY", "DISPLAY"):
              assert not re.search(rf"^{name}=", environ, re.M), f"the daemon has {name}:\n{environ}"
          # THE DISTRO'S PACKAGE, and the daemon is answering.
          exe = machine.succeed(f"readlink /proc/{pid}/exe").strip()
          assert exe.startswith("${emacs}/"), exe
          assert ask_emacs("(daemonp)") == "t"
          print("the daemon's frames before any client: " + json.dumps(frames()))

      with subtest("a terminal client works with no display"):
          before = daemon()
          in_session(
              "tty", f"${pkgs.python3}/bin/python3 ${pty} {client} -t /home/{user}/tty.txt",
              "TERM=xterm-256color",
          )
          retry(lambda last_try: any(f.startswith("t /dev/pts/") for f in frames()), timeout_seconds=60)
          # ITS FRAME GOES, and the client with it, as `C-x C-c` would.
          ask_emacs(
              "(dolist (f (frame-list)) (when (string-prefix-p \"/dev/pts/\" "
              "(or (terminal-name (frame-terminal f)) \"\")) (delete-frame f)))"
          )
          machine.wait_until_succeeds(
              f"systemctl --user --machine={user}@ show tty -p SubState | grep -x SubState=exited > /dev/null",
              timeout=30,
          )
          f = facts("tty")
          assert f["ExecMainStatus"] == "0", f"the terminal client did not end cleanly: {f}"
          assert daemon() == before, "the daemon did not outlive its terminal client"

      with subtest("a graphical client opens a native frame, and the daemon outlives it"):
          session, sock = start_session(1)
          # THE SESSION EXPORTS XWAYLAND'S DISPLAY TOO, and the pure-GTK client
          # still takes the Wayland one: `open_frame` asks for `xdg_shell` and
          # an XWayland that never started.
          machine.wait_until_succeeds(
              f"systemctl --user --machine={user}@ show-environment"
              " | grep -x 'DISPLAY=:[0-9]*' > /dev/null",
              timeout=60,
          )
          session_x = session_display()
          before = daemon()
          w = open_frame(sock, 1)
          # AND THE CLIENT HAD BOTH, so the Wayland frame was its own choice.
          client_env = machine.succeed(
              f"tr '\\0' '\\n' < /proc/{facts('gui1')['MainPID']}/environ"
          )
          assert re.search(rf"^DISPLAY={session_x}$", client_env, re.M), client_env
          # CLOSED THE WAY A PERSON CLOSES A WINDOW: the compositor asks it to.
          machine.succeed(
              f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg '[con_id={w['id']}] kill'"
          )
          retry(lambda last_try: not [x for x in windows(sock) if x.get("pid") == before[0]], timeout_seconds=30)
          machine.wait_until_succeeds(
              f"systemctl --user --machine={user}@ show gui1 -p SubState | grep -x SubState=exited > /dev/null",
              timeout=30,
          )
          assert daemon() == before, "the daemon did not outlive its frame"
          assert not [f for f in frames() if f.startswith("pgtk ")], frames()

      with subtest("a session's end takes a daemon that drew on it, and the next session gets a frame"):
          # THE FRAME IS CLOSED, and the display it was on is still held.
          before, since = daemon(), now()
          end_session(session)
          assert fate(before, since), (
              "the daemon outlived the end of a compositor it had drawn on, its frame closed; "
              "the Emacs section of docs/ARCHITECTURE.md says the session takes it"
          )
          session, sock = start_session(2)
          open_frame(sock, 2)
          # AND WITH ITS FRAME OPEN.
          before, since = daemon(), now()
          end_session(session)
          assert fate(before, since), (
              "the daemon outlived the end of a compositor its frame was open on"
          )

      with subtest("a session the daemon never drew on ends without it"):
          session, sock = start_session(3)
          machine.sleep(5)
          before, since = daemon(), now()
          end_session(session)
          assert not fate(before, since), (
              "the daemon died with a compositor it had never drawn on"
          )
          # AND IT STILL ANSWERS A TERMINAL, with no session at all.
          assert ask_emacs("(daemonp)") == "t"

      with subtest("the nested loop's client opens in the session, and the nested end leaves the daemon"):
          # A DAEMON THAT HOLDS NO DISPLAY: the case in which a client naming
          # the nested compositor would tie the daemon to it.
          manager(f"restart {daemon_unit}")
          daemon()
          assert ask_emacs("(x-display-list)") == "nil", ask_emacs("(x-display-list)")
          session, sock = start_session(4)
          session_x = session_display()
          nested, nested_sock, nested_display, nested_x = start_nested()
          print(f"the nested compositor took {nested_display} and {nested_x}")
          assert nested_x and nested_x != session_x, (
              f"the nested compositor serves X on `{nested_x}`, the session on `{session_x}`"
          )
          before = daemon()
          # AS THE LOOP'S FOOTER STARTS IT: in the loop's environment — the
          # nested compositor's two displays, the loop's two marks (the
          # owner's PID one no process has), and the session's two displays
          # kept aside.
          in_session(
              "loop", f"{loop_client} -c /home/{user}/loop.txt",
              f"WAYLAND_DISPLAY={nested_display}",
              f"DISPLAY={nested_x}",
              f"{loop_variable}=wayland-1",
              f"{loop_x_variable}={session_x}",
              "RONOCTUA_INSTANCE=dev",
              "RONOCTUA_DEV_PID=4194304",
          )
          where, w = window_in({"the session": sock, "the nested loop": nested_sock}, before[0])
          print(f"the loop's frame, in {where}: " + json.dumps(
              {k: w.get(k) for k in ("app_id", "shell", "pid")}
          ))
          assert where == "the session", (
              f"the loop's client opened its frame in {where} ({nested_display}): the daemon "
              "now holds the nested display, and every frame after it opens in there"
          )
          assert w.get("shell") == "xdg_shell", w
          # THE CLIENT IS NOT THE LOOP'S: the sweep at the next loop's start
          # kills what carries `RONOCTUA_INSTANCE=dev`, and would close the
          # frame with it.
          client_pid = int(facts("loop")["MainPID"])
          environ = machine.succeed(f"tr '\\0' '\\n' < /proc/{client_pid}/environ")
          assert re.search(r"^WAYLAND_DISPLAY=wayland-1$", environ, re.M), environ
          assert re.search(rf"^DISPLAY={session_x}$", environ, re.M), environ
          assert not re.search(r"^RONOCTUA_(INSTANCE|DEV_PID)=", environ, re.M), environ
          # AND THE FRAME'S ENVIRONMENT IS THE SESSION'S, which is what a
          # program Emacs starts from it inherits.
          said = ask_emacs(
              "(let (f) (dolist (x (frame-list)) (when (eq (framep x) 'pgtk) (setq f x))) "
              "(list (getenv \"WAYLAND_DISPLAY\" f) (getenv \"RONOCTUA_INSTANCE\" f)))"
          )
          assert said == '("wayland-1" nil)', said
          # THE NESTED COMPOSITOR ENDS, as a loop does, and the daemon never
          # drew there.
          before, since = daemon(), now()
          end_session(nested)
          assert not fate(before, since), (
              "the daemon died with the nested compositor, where the loop's client never drew"
          )
          end_session(session)

      with subtest("a stop of the person's manager with a frame open stops the daemon first, every time"):
          # A RACE IS ASKED MORE THAN ONCE: unordered, the daemon and the
          # compositor are stopped at once, and which goes first changes from
          # one stop to the next.
          STOPS = 6
          held = []
          for n in range(STOPS):
              daemon()
              # THE SESSION'S OWN UNIT NAME, which the daemon is ordered after.
              session, sock = start_session(5 + n, unit="scroll")
              open_frame(sock, f"stop{n}")
              name = f"notes-stop{n}.txt"
              hook = f"/home/{user}/hook-{n}"
              # ONLY THE EXIT CAN WRITE THE AUTO-SAVE (the periodic ones off),
              # and a hook says whether `kill-emacs` ran at all.
              said = ask_emacs(
                  "(progn (setq auto-save-interval 0 auto-save-timeout nil) "
                  f"(add-hook 'kill-emacs-hook (lambda () (write-region \"ran\" nil \"{hook}\"))) "
                  f"(with-current-buffer \"{name}\" (insert \"EDIT-{n}\") (buffer-modified-p)))"
              )
              assert said == "t", f"the edit did not take: {said}"
              cursor = journal_cursor()
              machine.succeed(f"systemctl stop user@{uid}.service")
              machine.wait_until_fails(f"systemctl is-active user@{uid}.service", timeout=60)
              autosave = machine.execute(f"cat '/home/{user}/#{name}#'")[1].strip()
              ran = machine.execute(f"test -e {hook}")[0] == 0
              journal = machine.succeed(
                  f"journalctl --no-pager -o cat --after-cursor={shlex.quote(cursor)} _UID={uid}"
              )
              # GDK'S WAY OUT (`gdk/wayland/gdkeventsource.c`): each of its three
              # sentences comes before an `_exit(1)`.
              lost = [
                  line for line in journal.splitlines()
                  if "Lost connection to Wayland compositor" in line
                  or "Error reading events from display" in line
                  or "dispatching to Wayland display" in line
              ]
              order = [
                  line for line in journal.splitlines()
                  if line.startswith("Stopped ") and ("Emacs:" in line or "/bin/scroll " in line)
              ]
              ok = autosave == f"EDIT-{n}" and ran and not lost
              held.append(ok)
              print(
                  f"stop {n}: auto-save {autosave!r}, kill-emacs-hook ran: {ran}, "
                  f"the display lost first: {lost}; held: {ok}; " + json.dumps(order)
              )
              machine.succeed(f"systemctl start user@{uid}.service")
          print(f"THE EDIT HELD IN {sum(held)} OF {STOPS} STOPS")
          assert all(held), (
              f"the edit held in {sum(held)} of {STOPS} stops of the manager: the daemon "
              "lost its display before its SIGTERM's `kill-emacs` ran"
          )
    '';
}
