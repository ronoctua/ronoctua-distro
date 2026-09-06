# Power in a VM — what a machine with no battery and no platform profile can
# prove, which is most of the RULES and none of the hardware.
#
# WHAT IS PROVED HERE
# -------------------
# 1. UPower is on the bus with no battery, and the footer draws NO battery
#    segment — "nothing at rest" is the owner's rule, and a VM is the machine
#    that has nothing.
# 2. power-profiles-daemon is up and, with no platform driver, offers only
#    `balanced` and `power-saver`; the footer, asked for the distro's default
#    `performance`, applies the top it CAN get (`balanced`) and says so once —
#    the fallback the crate's `top()` promises. Asked for `power-saver` through
#    the floor variable, it applies that, and the profile MARK appears.
# 3. logind's actions and the inhibitor locks answer an active session without
#    a prompt, and refuse a process outside one BY NAME — the two-sided proof
#    `disks-vm.nix` makes for udisks.
# 4. The polkit policies the module relies on say `yes` — read off the
#    installed files, so a nixpkgs bump that changed them fails here.
# 5. The udev rule for the thresholds is installed and parses.
#
# WHAT CANNOT BE PROVED HERE: a battery's glyph, its number, its danger colour,
# a threshold actually written. QEMU has no ACPI battery. Those are proved on
# the owner's laptop, by eye — `CLAUDE.md`'s rule that a screen is looked at.
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
  name = "ronoctua-power";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/power.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-power";
      users.users.root.hashedPassword = lib.mkForce null;

      # A SEAT SESSION for the user, so that `allow_active` has an active session
      # to be about: the footer runs from the user's manager, and polkit maps a
      # user unit to the user's display session — which is what these tests
      # measure, not assume.
      services.getty.autologinUser = config.ronoctua.user;
      users.users.${config.ronoctua.user}.linger = true;

      environment.systemPackages = [
        niri
        ronoctua-shell
        pkgs.imagemagick
        pkgs.tesseract
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

      def as_user(command):
          """Run `command` in the user's own manager — an ACTIVE session's process."""
          return machine.succeed(
              "systemd-run --machine=${nodes.machine.ronoctua.user}@ --user --pipe --wait --quiet " + command
          )

      def footer_journal():
          return machine.succeed("journalctl --no-pager -u footer 2>/dev/null || true")

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("the three daemons are on the bus, and the module turned them on"):
          # BOTH DAEMONS ARE BUS-ACTIVATED: nothing starts them until somebody
          # asks, and waiting for the unit first is waiting for nobody. The
          # call is the activation, and the unit is up after it.
          machine.succeed(
              "busctl --system call org.freedesktop.UPower /org/freedesktop/UPower "
              "org.freedesktop.UPower EnumerateDevices"
          )
          machine.wait_for_unit("upower.service")
          profiles = machine.succeed("powerprofilesctl list")
          print(profiles)
          machine.wait_for_unit("power-profiles-daemon.service")
          # NO PLATFORM DRIVER IN QEMU: `performance` is not offered, and that is
          # the case the fallback exists for.
          assert "performance" not in profiles, "QEMU grew a platform profile; the fallback has nothing to prove"
          assert "balanced" in profiles and "power-saver" in profiles, profiles

      with subtest("the policies the module relies on say yes, read off the installed files"):
          login1 = machine.succeed("cat /run/current-system/sw/share/polkit-1/actions/org.freedesktop.login1.policy")
          for action in ("suspend", "hibernate", "power-off", "reboot", "inhibit-block-sleep", "inhibit-handle-lid-switch"):
              block = re.search(r'id="org\.freedesktop\.login1\.' + action + r'".*?</action>', login1, re.S)
              assert block, f"login1 lost the action {action}"
              assert "<allow_active>yes</allow_active>" in block.group(0), f"login1's {action} no longer says yes to an active session"
          # THE FILE IS NAMED AFTER THE DAEMON, not after the action it declares.
          ppd = machine.succeed("cat ${pkgs.power-profiles-daemon}/share/polkit-1/actions/power-profiles-daemon.policy")
          block = re.search(r'id="org\.freedesktop\.UPower\.PowerProfiles\.switch-profile".*?</action>', ppd, re.S)
          assert block and "<allow_active>yes</allow_active>" in block.group(0), "switch-profile no longer says yes"

      with subtest("the udev rule for the thresholds is installed and parses"):
          rules = machine.succeed("cat /etc/udev/rules.d/99-local.rules")
          assert "charge_control_end_threshold" in rules and "charge_control_start_threshold" in rules, rules
          machine.succeed("udevadm verify /etc/udev/rules.d/99-local.rules")

      with subtest("an active session takes an inhibitor without a prompt; a process outside one is refused by name"):
          # WITHOUT THE `|| true` IT IS A WAIT: `is-system-running --wait`
          # answers non-zero while starting AND when degraded, so the word is
          # what to test — `|| true` made every attempt succeed at once.
          machine.wait_until_succeeds(
              "systemctl --machine=${nodes.machine.ronoctua.user}@ --user is-system-running --wait "
              "| grep -qE 'running|degraded'",
              timeout=60,
          )
          # A `sleep` inhibitor from the USER'S manager: the same place the footer
          # runs from. `busctl` holds the fd for the length of the call, which is
          # enough to prove the door opens.
          as_user(
              "busctl --system call org.freedesktop.login1 /org/freedesktop/login1 "
              "org.freedesktop.login1.Manager Inhibit ssss sleep ronoctua-test proof block"
          )
          # And from a process with NO session at all: `NotAuthorized`, by name,
          # at once — never a hang waiting for an agent that is not there.
          status, out = machine.execute(
              "systemd-run --uid=${nodes.machine.ronoctua.user} --pipe --wait --quiet busctl --system call "
              "org.freedesktop.login1 /org/freedesktop/login1 org.freedesktop.login1.Manager "
              "Inhibit ssss sleep ronoctua-test proof block 2>&1"
          )
          assert status != 0, "a session-less process took an inhibitor"
          # polkit's own words for it: "Access denied as the requested operation
          # requires interactive authentication. However, interactive
          # authentication has not been enabled by the calling program." — the
          # `InteractiveAuthorizationRequired` name, which `refusal()` reads as
          # NotAuthorized. Never a hang.
          assert "NotAuthorized" in out or "interactive authentication" in out.lower(), out

      with subtest("the footer, asked for performance, applies balanced and says so once"):
          # SOMETHING TO FALL BACK FROM: the daemon starts on `balanced`, which
          # is the top it offers, so the footer would find its target already
          # active and have nothing to apply — and the "once" below counts an
          # apply. Start the machine on power-saver.
          machine.succeed("powerprofilesctl set power-saver")
          machine.wait_until_succeeds("[ \"$(powerprofilesctl get)\" = power-saver ]", timeout=30)
          machine.succeed("mkdir -p /root/.config/ronoctua")
          machine.succeed(
              "printf '%s' '{\"tasks\":{\"system-update\":{\"enabled\":false}}}' > /root/.config/ronoctua/brain.json"
          )
          start_xvfb()
          start_niri("niri", "${niri}")
          sock = socket_of("niri")
          machine.succeed(
              "systemd-run --unit=footer ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 "
              "--setenv=HOME=/root "
              "--setenv=RONOCTUA_POWER_PROFILE=performance "
              f"--setenv=NIRI_SOCKET={sock} "
              "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_power=debug "
              "${ronoctua-shell}/bin/ronoctua-shell"
          )
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -q 'the chosen power profile is not offered here'",
              timeout=60,
          )
          journal = footer_journal()
          assert journal.count("applying the chosen power profile") == 1, "the default was applied more than once, or never"
          machine.wait_until_succeeds("[ \"$(powerprofilesctl get)\" = balanced ]", timeout=30)
          # NO BATTERY, NO SEGMENT — and no profile mark either, because
          # `balanced` is the top this machine offers. The mark WAS there while
          # the machine sat on power-saver (the first frame, and the echo's
          # repaint right after the order went out), so what has to be mute is
          # the LAST layout, once the daemon's answer has come back.
          def last_layout():
              j = footer_journal()
              i = j.rfind("footer layout resolved")
              return j[i:] if i >= 0 else ""
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -q 'id=\"profile\"'", timeout=60
          )
          settled = ""
          for _ in range(30):
              settled = last_layout()
              if settled and 'id="profile"' not in settled:
                  break
              machine.sleep(1)
          journal = footer_journal()
          assert 'id="battery"' not in journal, "a battery segment on a machine with no battery"
          assert settled and 'id="profile"' not in settled, (
              "a profile mark at the machine's top profile:\n" + settled
          )
          machine.succeed("systemctl stop footer")

      with subtest("asked for power-saver, the footer applies it and the profile mark appears"):
          machine.succeed("rm -f /run/user/0/ronoctua-*.sock")
          sock = socket_of("niri")
          machine.succeed(
              "systemd-run --unit=footer-saver ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 "
              "--setenv=HOME=/root "
              "--setenv=RONOCTUA_POWER_PROFILE=power-saver "
              f"--setenv=NIRI_SOCKET={sock} "
              "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_power=debug "
              "${ronoctua-shell}/bin/ronoctua-shell"
          )
          machine.wait_until_succeeds("[ \"$(powerprofilesctl get)\" = power-saver ]", timeout=60)
          machine.wait_until_succeeds(
              "journalctl -u footer-saver --no-pager | grep -q 'id=\"profile\"'", timeout=60
          )
          journal = machine.succeed("journalctl -u footer-saver --no-pager")
          assert "not offered" not in journal, "power-saver is offered; nothing to fall back from"

      with subtest("a process of the user's own session switches the profile without a prompt — the door the footer uses"):
          # THE FOOTER ABOVE RAN AS ROOT, which polkit lets through unasked; this
          # is the `switch-profile` action from an ACTIVE session's user process,
          # which is what `allow_active: yes` promises and what a real login is.
          # BY PATH: the user manager's PATH is minimal, and a bare name fails
          # at EXEC — the trap CLAUDE.md records for every spawn in a unit.
          as_user("${pkgs.power-profiles-daemon}/bin/powerprofilesctl set balanced")
          machine.wait_until_succeeds("[ \"$(powerprofilesctl get)\" = balanced ]", timeout=30)
          as_user("${pkgs.power-profiles-daemon}/bin/powerprofilesctl set power-saver")
          machine.wait_until_succeeds("[ \"$(powerprofilesctl get)\" = power-saver ]", timeout=30)

      with subtest("the critical action UPower reports is a word the view can speak"):
          # The module asks for Hibernate; a VM without a resume device is
          # answered with UPower's own fallback. Either way it is one of the
          # words `power_words::daemon_word` knows — never a sentence. (The
          # owner's laptop answers `Sleep`, a word UPower 1.90 added.)
          word = machine.succeed(
              "busctl call org.freedesktop.UPower /org/freedesktop/UPower "
              "org.freedesktop.UPower GetCriticalAction"
          ).strip()
          known = ('s "Hibernate"', 's "HybridSleep"', 's "PowerOff"', 's "Sleep"', 's "Ignore"')
          assert word in known, word
    '';
}
