# A DEPLOY DOES NOT TEAR DOWN THE LIVE SESSION — the regression guard.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# `nix/modules/scroll.nix` sets `restartIfChanged = false` on the compositor's
# user service so that a `nixos-rebuild switch` which changes `scroll.service`
# does NOT stop it mid-deploy — stopping the compositor disconnects every
# Wayland client at once (the footer, every window, the deploy's own terminal)
# and drops the machine to the TTY. That had happened twice before the guard:
# the unit changes for reasons unrelated to the running compositor (its
# `XCURSOR_PATH` is keyed to the default theme, so moving the default rewrote
# it), and the switch restarted it.
#
# Whether `restartIfChanged = false` actually HOLDS is a fact of three things
# at once — how NixOS renders the marker, whether `sd-switch` honours it for a
# *user* service, and whether `switch-to-configuration` drives the user manager
# at all — and nothing short of a real session taking a real switch answers it.
# So this boots the compositor the way the installed machine does (the real
# `scroll.service`, a user unit under autologin, on an Xvfb screen so no GPU is
# needed), notes the compositor's process, and runs a `switch-to-configuration`
# to a specialisation that changes the unit — the path a rebuild takes.
#
# TWO SWITCHES, AND THE SECOND IS WHY THE FIRST MEANS SOMETHING
# ------------------------------------------------------------
# `touched` changes the unit and keeps `restartIfChanged = false`: the
# compositor's PID must be the SAME afterwards — the deploy left it alone. On
# its own that assertion is weak: it would also pass if the switch never
# touched user services, or never saw a change. So `restarts` makes the SAME
# change with `restartIfChanged` forced back to `true` — the behaviour before
# the guard — and the compositor's PID MUST change. That control proves the
# harness can see a restart, so the first switch's survival is a real result
# and not silence. (The lesson `tests/checks.nix` opens with: a check on the
# mechanism is not a check on the wiring.)
{
  pkgs,
  lib,
  inputs,
  # The same namespace the installed machine's modules read (see the kiosk
  # test): the compositor, the shell it binds, the solved palettes, the
  # cursors, the fonts.
  ronoctua,
}:
pkgs.testers.runNixOSTest {
  name = "ronoctua-scroll-deploy";

  node.specialArgs = { inherit inputs; };

  nodes.machine =
    { config, ... }:
    let
      user = config.ronoctua.user;
    in
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/scroll.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      # The test framework names the node `machine`; base.nix wants the
      # `noctua-` prefix (its own assertion). The node is reached by its attr
      # name in the script, not by this, so forcing it is free.
      networking.hostName = lib.mkForce "noctua-scroll";

      # Autologin with no password, the way the session comes up.
      users.users.root.hashedPassword = lib.mkForce null;
      users.users.${user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };

      nix.settings.substituters = lib.mkForce [ ];
      # base.nix keeps the boot console quiet (`consoleLogLevel = 3`); the test
      # framework wants it loud. This is a test, so let the framework win.
      boot.consoleLogLevel = lib.mkForce 7;
      fonts.enableDefaultPackages = false;
      fonts.packages = [ ronoctua.fonts.noctua-iosevka-core ];

      # THE SCREEN: the compositor on Xvfb, so it comes up with no GPU — the
      # same trick the kiosk test uses (`tests/kiosk-vm.nix`,
      # `tests/compositor-xvfb.nix`). The question here is systemd's, not the
      # renderer's, so the software path is enough.
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

      # TWO WAYS TO CHANGE THE UNIT, one per switch below. Each adds a harmless
      # environment variable, which is all it takes to make the unit differ —
      # the same kind of churn a theme or a binary bump causes.
      #
      #  - `touched` keeps the real `restartIfChanged = false`: the deploy must
      #    leave the compositor running.
      #  - `restarts` forces it back to `true`: the control, which must restart.
      specialisation.touched.configuration = {
        systemd.user.services.scroll.environment.RONOCTUA_DEPLOY_PROBE = "touched";
      };
      specialisation.restarts.configuration = {
        systemd.user.services.scroll.environment.RONOCTUA_DEPLOY_PROBE = "restarts";
        systemd.user.services.scroll.restartIfChanged = lib.mkForce true;
      };
    };

  testScript =
    { nodes, ... }:
    ''
      user = "${nodes.machine.ronoctua.user}"


      def uctl(args):
          """A query of the person's own manager, where the compositor lives."""
          return machine.succeed(f"systemctl --machine={user}@ --user {args}")


      def scroll_pid():
          return uctl("show scroll.service -p MainPID --value").strip()


      machine.wait_for_unit("multi-user.target")

      # THE COMPOSITOR COMES UP AS THE PERSON'S, on the Xvfb screen — the login
      # shell starts `scroll-session.target`, which pulls the compositor in.
      machine.wait_until_succeeds(
          f"systemctl --machine={user}@ --user is-active scroll.service", timeout=240
      )
      machine.wait_until_succeeds(
          f"systemctl --machine={user}@ --user is-active scroll-session.target", timeout=60
      )

      before = scroll_pid()
      assert before and before != "0", f"the compositor has no MainPID: {before!r}"
      print(f"compositor running as pid {before}")

      # 1. A DEPLOY THAT CHANGES THE UNIT LEAVES THE COMPOSITOR ALONE.
      #    Switching to `touched` is the path `nixos-rebuild switch` takes: the
      #    unit differs (a new env var), and `restartIfChanged = false` has to
      #    hold the running compositor in place.
      # Both switches are reached through `/run/booted-system`, not
      # `/run/current-system`: a `test` activation repoints current-system at
      # the specialisation it just activated, and a specialisation has no
      # specialisations of its own, so the second path would vanish. The booted
      # system is the base and holds both, unchanged by a `test`.
      #
      # Printed, not parsed: the switch names scroll.service here as a unit it
      # changed and, under the guard, "NOT restarting" — but the proof is the
      # process below, which does not depend on the wording.
      print(
          machine.succeed(
              "/run/booted-system/specialisation/touched/bin/switch-to-configuration test 2>&1"
          )
      )
      after = scroll_pid()
      assert after == before, (
          f"THE DEPLOY RESTARTED THE COMPOSITOR: pid {before} -> {after}. "
          "restartIfChanged=false is not holding scroll.service — a rebuild "
          "would drop the session to the TTY."
      )
      print(f"OK: the compositor survived the deploy (still pid {after})")

      # 2. CONTROL — the same change with the guard removed DOES restart it, so
      #    the survival above is a real result and not the switch doing nothing.
      #    The only difference between this and `touched` is restartIfChanged.
      print(
          machine.succeed(
              "/run/booted-system/specialisation/restarts/bin/switch-to-configuration test 2>&1"
          )
      )
      machine.wait_until_succeeds(
          f"systemctl --machine={user}@ --user is-active scroll.service", timeout=120
      )
      # Let the restart settle onto a real new process, not the 0 of the gap.
      machine.wait_until_succeeds(
          f'test "$(systemctl --machine={user}@ --user show scroll.service -p MainPID --value)" != "0"',
          timeout=60,
      )
      control = scroll_pid()
      assert control != before, (
          f"the control did NOT restart the compositor (still pid {before}); "
          "the test cannot witness a restart, so assertion 1 proves nothing — "
          "fix the harness before trusting it."
      )
      print(f"OK: the control restarted the compositor as expected ({before} -> {control})")
    '';
}
