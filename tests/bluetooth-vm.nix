# Bluetooth in a VM — what a machine with no radio can prove, which is the
# door, the daemon and the drawing, and none of the hardware.
#
# WHAT IS PROVED HERE
# -------------------
# 1. With NO controller, `org.bluez` cannot even be activated: its unit has a
#    `ConditionPathIsDirectory=/sys/class/bluetooth`. That is the state the
#    whole crate has to survive, and a laptop never shows it.
# 2. The wall an ordinary user meets is that absent radio and NOT the bus's
#    policy: a policy that refused would answer `AccessDenied` before any
#    activation was attempted. The policy FILE is read at build time by
#    `bluetooth-bus-policy-is-open`; this is the same fact on a running bus.
# 3. The module writes what differs (`FastConnectable`) and restates no
#    default — and the radio still comes up at boot, through the nixpkgs
#    module's own `AutoEnable`.
# 4. The footer comes up with the radio absent and draws NO device mark —
#    "nothing at rest" is the owner's rule, and a VM is the machine that has
#    nothing.
# 4. `LEADER s b b` opens the view and it DRAWS — on a machine with no radio
#    it says so, rather than showing an empty region.
#
# WHAT THE FOOTER'S PAIRING AGENT CANNOT PROVE HERE: registering one needs a
# daemon, and a daemon needs a controller. That the agent asks for no
# well-known name is read out of the source by
# `bluetooth-bus-policy-is-open`, which is where a fact about code belongs.
#
# WHAT CANNOT BE PROVED HERE: a device's kind glyph, a percentage, a pairing
# question, the danger colour. QEMU has no Bluetooth controller. Those are
# proved on the owner's laptop, by eye — `AGENTS.md`'s rule that a screen is
# looked at before the word "pronto".
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
  ronoctua,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-bluetooth";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/bluetooth.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-bluetooth";
      users.users.root.hashedPassword = lib.mkForce null;

      # A SEAT SESSION for the user: the footer runs from the user's manager,
      # and the point of subtest 2 is that an ordinary session reaches the
      # daemon. Without a session there would be nothing to be ordinary.
      services.getty.autologinUser = config.ronoctua.user;
      users.users.${config.ronoctua.user}.linger = true;

      environment.systemPackages = [
        niri
        ronoctua-shell
        pkgs.bluez
      ]
      ++ xvfb.packages;

      fonts.enableDefaultPackages = false;
      fonts.packages = [ fonts.noctua-iosevka-core ];

      hardware.graphics.enable = true;
      services.dbus.enable = true;
    };

  testScript =
    { nodes, ... }:
    ''
      ${xvfb.prelude}

      import re

      def as_user_may_fail(command):
          """Run `command` in the user's own manager — an ordinary session —
          and hand back what it said WHETHER OR NOT it worked. The refusal is
          the thing being read here, so it must not be an assertion."""
          return machine.execute(
              "systemd-run --machine=${nodes.machine.ronoctua.user}@ --user --pipe --wait --quiet "
              + command
          )

      def footer_journal():
          return machine.succeed("journalctl --no-pager -u footer 2>/dev/null || true")

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("with no controller, org.bluez cannot even be activated"):
          # ACTIVATED BY THE CALL: the unit is `WantedBy=bluetooth.target` and
          # `ConditionPathIsDirectory=/sys/class/bluetooth`, so on a machine
          # with no controller nothing starts it and the call is what tries.
          # An empty answer and a refusal are different things, and this is
          # the one the crate treats as "no radio".
          status, answer = machine.execute(
              "busctl --system call org.bluez / "
              "org.freedesktop.DBus.ObjectManager GetManagedObjects 2>&1"
          )
          print(answer)
          assert status != 0, "a daemon answered on a machine with no controller"
          assert "/org/bluez/hci" not in answer, "QEMU grew a Bluetooth controller"
          # `systemctl show` AND NOT `systemctl status`: the second EXITS 3 for
          # a unit that is not running, and the test driver's shell has
          # `pipefail` on — so the pipeline failed on the very state the line
          # was asserting. `show` always exits 0 and prints the fact.
          state = machine.succeed(
              "systemctl show bluetooth.service -p ActiveState -p ConditionResult"
          )
          print(state)
          assert "ActiveState=inactive" in state or "ConditionResult=no" in state, state

      with subtest("the bus lets an ordinary user through: the wall it meets is the absent radio"):
          machine.wait_until_succeeds(
              "systemctl --machine=${nodes.machine.ronoctua.user}@ --user is-system-running --wait "
              "| grep -qE 'running|degraded'",
              timeout=60,
          )
          # THE WHOLE SECURITY STORY, on the running bus, and readable even
          # here: with no controller, `bluetooth.service` is skipped by its own
          # `ConditionPathIsDirectory`, so the call cannot reach a daemon. What
          # it CAN prove is which wall it met. A policy that refused us would
          # answer `AccessDenied` — the broker checks the policy BEFORE it tries
          # to activate anything — and what comes back instead is the daemon
          # not being startable. One is a door shut on this user; the other is
          # an empty machine, and only the first would be a defect.
          #
          # `busctl` AND NOT `bluetoothctl`: the latter is a shell that waits
          # for a controller to appear and, on a machine that has none, waits
          # for ever — which is a hung test rather than a failed one.
          status, out = as_user_may_fail(
              "${pkgs.systemd}/bin/busctl --system call org.bluez / "
              "org.freedesktop.DBus.ObjectManager GetManagedObjects 2>&1"
          )
          print(out)
          assert status != 0, "a daemon answered on a machine with no controller"
          assert "AccessDenied" not in out and "not allowed" not in out, (
              "the bus refused an ordinary user: the policy narrowed\n" + out
          )
          assert "activate" in out or "unit failed" in out, out
          # And the user is in NO bluetooth group: there is none to be in.
          groups = machine.succeed("groups ${nodes.machine.ronoctua.user}")
          assert "bluetooth" not in groups, f"a bluetooth group appeared: {groups}"

      with subtest("the module writes what differs and restates no default"):
          # THE FILE AS THE MODULE WRITES IT: `key=value`, no spaces. Reading
          # it any other way is asserting the formatter rather than the value.
          conf = machine.succeed("cat /etc/bluetooth/main.conf")
          settings = dict(
              line.split("=", 1)
              for line in conf.splitlines()
              if "=" in line and not line.strip().startswith("[")
          )
          assert settings.get("FastConnectable") == "true", conf
          # `Experimental` stays off: `Battery1` does not need it, and turning
          # it on would register the LE-audio profiles nothing here uses.
          assert settings.get("Experimental") != "true", conf
          # AND THE RADIO COMES UP AT BOOT. It rests on the nixpkgs module's
          # own `powerOnBoot`, which is `Policy.AutoEnable`; nothing of ours
          # writes it, and this is what would say so if it went away.
          assert settings.get("AutoEnable") == "true", conf
          # AND NOTHING RESTATES A DEFAULT. `AutoEnable` is the nixpkgs
          # module's own (`powerOnBoot`), not ours; what ours adds is one key.
          assert "ReconnectAttempts" not in settings, conf
          assert "DiscoverableTimeout" not in settings, conf

      with subtest("the footer comes up with no radio and draws no device mark"):
          start_xvfb()
          start_niri("niri", "${niri}")
          sock = socket_of("niri")
          machine.succeed(
              "systemd-run --unit=footer ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 "
              "--setenv=HOME=/root "
              f"--setenv=NIRI_SOCKET={sock} "
              "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_bluetooth=debug "
              "${ronoctua-shell}/bin/ronoctua-shell"
          )
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -q 'footer layout resolved'", timeout=60
          )
          journal = footer_journal()
          for mark in ("bt-1", "bt-2", "bt-3", "bt-4"):
              assert f'id="{mark}"' not in journal, f"a device mark with no radio: {mark}"

      with subtest("the view opens on a machine with no radio, and draws"):
          machine.succeed(
              "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
              "${ronoctua-shell}/bin/ronoctua-shell command bluetooth-open"
          )
          # THE REGION GREW, which is the only proof the view drew anything:
          # the footer at rest is two lines (53 px), and a `layout resolved`
          # taller than that is the view on screen. Waiting for the line
          # itself would have been satisfied by subtest 4's own.
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -qE 'layout resolved width=[0-9]+ height=[0-9]{3}'",
              timeout=30,
          )
          journal = footer_journal()
          heights = [int(h) for h in re.findall(r"layout resolved width=\d+ height=(\d+)", journal)]
          assert max(heights) > 60, f"the view did not open: {heights}"
          # THE MESSAGE, and not a field glued to it.
          #
          # This used to look for `bluetooth-open refused`, which cannot
          # appear: the line is `info!(view = what, "refused: …")`, and
          # tracing renders the MESSAGE first and the fields after it —
          # `refused: … view="bluetooth-open"`. So the assertion was true of
          # every possible journal, which is AGENTS.md's own trap: a check
          # against the criterion and not against the code closes an empty
          # milestone.
          assert "refused: another view is open" not in journal, journal
    '';
}
