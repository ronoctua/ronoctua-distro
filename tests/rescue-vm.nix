# The rescue console's SECOND door — `Ctrl+Alt+Del` with no compositor at all.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The first door (the floating window) is proved in `tests/terminal-vm.nix`,
# next to the process monitor's, because it is the same mechanism. This one
# cannot be: there is no compositor here, no Xvfb, no GPU and no session. That
# is the whole point — the console has to answer on a machine where none of that
# came up.
#
# Three things, and only a booted machine answers any of them:
#
#   1. `ctrl-alt-del.target` is OURS. systemd ships it as an alias of
#      `reboot.target`; a unit of the same name in `/etc/systemd/system`
#      replaces it. Whether that actually happens is a systemd question, not a
#      Nix one, and evaluating the config would not answer it;
#   2. it does NOT reboot. The assertion is the boot id, before and after: a
#      test that only looked for the console would pass on a machine that
#      rebooted and came back;
#   3. it DREW. Read from `/dev/vcs8`, which is the console's own text — no
#      OCR, no screenshot, no font. If the sidebar is there, the whole chain
#      is: the unit, the VT switch, ratatui, and the translated text.
{
  pkgs,
  lib,
  ronoctua,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-rescue";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/rescue.nix
      ];
      _module.args.ronoctua = ronoctua;
      virtualisation.memorySize = 2048;
      # `fgconsole` and `chvt` — the unit uses them by store path, and the test
      # asks the same questions from the shell.
      environment.systemPackages = [ pkgs.kbd ];
      networking.hostName = lib.mkForce "noctua-rescue";
      # No /persist in this VM — the same reason as the keyboard's: `base.nix`
      # points the password hash there and the test never logs in.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };
      users.users.root.hashedPassword = lib.mkForce null;
    };

  # `{ nodes, ... }`: the user's name lives in `config.ronoctua.user` and nowhere
  # else (invariant 16). Written by hand here, this test would go on passing
  # after the system's user changed name.
  testScript =
    { nodes, ... }:
    ''
      user_name = "${nodes.machine.ronoctua.user}"

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("ctrl-alt-del.target is OURS, and not an alias for reboot"):
          # THE DEFECT THIS CATCHES: a unit that does not override the alias.
          # Nothing would say so — the file would sit in /etc/systemd/system being
          # ignored, and the first press would reboot the machine.
          # `systemctl show` resolves the symlink, so the question is asked of
          # the NAME the kernel starts and answered by what it actually leads to.
          wants = machine.succeed("systemctl show ctrl-alt-del.target -p Wants --value")
          following = machine.succeed("systemctl show ctrl-alt-del.target -p Id --value").strip()
          print(f"{following}: {wants}")
          assert "ronoctua-rescue.service" in wants, (
              f"ctrl-alt-del.target leads to `{following}`, and not to the rescue:\n" + wants
          )

      with subtest("the hard exit is still systemd's"):
          # Taking the key over means taking what it did. If the burst stopped
          # rebooting too, a machine whose everything is frozen would have no way
          # out at all — and nothing would say so until that day.
          burst = machine.succeed(
              "systemctl show --property=CtrlAltDelBurstAction --value"
          ).strip()
          assert burst == "reboot-force", f"the seven taps became `{burst}`"

      with subtest("one press OPENS the console, and the machine does NOT restart"):
          before = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()

          # What the kernel does when the key is pressed on a console the
          # compositor is not holding — started here by hand, because injecting a
          # key below evdev would be testing the kernel and not our system.
          machine.succeed("systemctl start ctrl-alt-del.target")
          machine.wait_until_succeeds(
              "systemctl is-active --quiet ronoctua-rescue.service", timeout=30
          )

          after = machine.succeed("cat /proc/sys/kernel/random/boot_id").strip()
          assert before == after, (
              "the machine RESTARTED: the target still leads to reboot.target"
          )

      with subtest("it runs as the person, and not as root"):
          # A root console one keypress away from any locked machine is a bigger
          # hole than the one this console closes.
          who = machine.succeed(
              "systemctl show ronoctua-rescue.service -p User --value"
          ).strip()
          assert who == user_name, f"the rescue runs as `{who}`"

      with subtest("the virtual console's screen has the sidebar"):
          # `/dev/vcs8` IS the console's text. No screenshot, no OCR, no font: if
          # the sidebar is in there, then the unit came up, the VT switched, the
          # terminal took raw mode, ratatui drew and the text came out translated.
          #
          # `Terminal` and not `State`: it is the one entry that reads the same
          # in both languages, so the assertion does not depend on which locale
          # the VM happened to boot with.
          machine.wait_until_succeeds(
              "tr -d '\\0' < /dev/vcs8 | grep -q Terminal", timeout=30
          )
          screen = machine.succeed("tr -d '\\0' < /dev/vcs8")
          print(screen)

      with subtest("and the virtual console in focus is the 8th"):
          # Opening on a VT nobody is looking at is opening nothing.
          at = machine.succeed("fgconsole").strip()
          assert at == "8", f"the rescue opened on vt8 and the screen stayed on {at}"

      with subtest("the units it can ask for exist"):
          # A row whose unit does not exist fails in front of the person, and the
          # message systemd gives ("Unit not found") points nowhere useful.
          for unit in [
              "ronoctua-rescue-rollback@switch.service",
              "ronoctua-rescue-rollback@boot.service",
              "ronoctua-rescue-rebuild.service",
              "ronoctua-rescue-host.service",
          ]:
              machine.succeed(f"systemctl cat {unit} > /dev/null")

      with subtest("the directory the editor writes into belongs to the person"):
          # AND THE BOOT DID NOT COMPLAIN. `systemd-tmpfiles` writes one line
          # and carries on: the rule named a group that does not exist, the
          # directory was never created, and the failure would have shown up
          # much later as an edit that saved and installed nothing.
          boot = machine.succeed("journalctl -b -u systemd-tmpfiles-setup --no-pager")
          assert "ronoctua-rescue" not in boot, (
              "tmpfiles complained about our rule:\n" + boot
          )
          owner = machine.succeed("stat -c %U /run/ronoctua-rescue").strip()
          assert owner == user_name, (
              f"/run/ronoctua-rescue e de `{owner}`; o editor roda como a pessoa "
              "and would not manage to write"
          )

      with subtest("leaving gives the screen back to where it came from"):
          machine.succeed("systemctl stop ronoctua-rescue.service")
          machine.wait_until_succeeds("test $(fgconsole) != 8", timeout=30)
    '';
}
