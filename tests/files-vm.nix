# The file manager's PRIVILEGED half — and the refusals, which are the point.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The listing, the menu, the picker, the columns and every operation on one's
# own files are pure and are tested where they live: `ronoctua-files` has
# sixty-five unit tests against a fake tree, and `files::View` has fifteen more
# against a real temporary one. None of that needs a machine.
#
# What needs a machine is the door to root. Four things, and a booted system is
# the only thing that answers any of them:
#
#   1. the unit EXISTS and runs the extracted binary — a `systemd.services`
#      entry that does not evaluate is a `chown` that fails in front of the
#      person with "Unit not found";
#   2. the person reaches it THROUGH THE SUDO DOOR and no other way: the
#      root helper's `files-root` verb starts it with the person's password
#      on stdin (what `ronoctua_security::authority` pipes), a wrong password
#      starts nothing, and NO polkit rule names it — the rule that used to
#      could never ask anybody in a session with no agent;
#   3. the staging directory is the PERSON's. `systemd-tmpfiles` writes one line
#      at boot and carries on — the rescue console already paid for a rule that
#      named a group which did not exist, and the failure surfaced far away as
#      an edit that saved and installed nothing;
#   4. and the helper REFUSES what it should. A gate that has never refused
#      anything has not been tested, and this one guards a program that runs as
#      root.
{
  pkgs,
  lib,
  ronoctua,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-files";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/files.nix
        # THE SUDO DOOR's helper: the only way the person starts the unit.
        ../nix/modules/security.nix
      ];
      _module.args.ronoctua = ronoctua;
      ronoctua.security.helper = ronoctua.securityRoot;
      virtualisation.memorySize = 2048;
      networking.hostName = lib.mkForce "noctua-files";
      # No /persist in this VM — the same reason as the keyboard's and the
      # rescue's: `base.nix` points the password hash there and the test never
      # logs in.
      #
      # THE TEST'S OWN PASSWORD — a fixture, never a person's: the door pipes
      # it to `sudo -S`.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "a door, not a rule";
      };
      users.users.root.hashedPassword = lib.mkForce null;
    };

  # `{ nodes, ... }`: the user's name lives in `config.ronoctua.user` and
  # nowhere else (invariant 16). Written by hand here, this test would go on
  # passing after the system's user changed name.
  testScript =
    { nodes, ... }:
    ''
      import shlex

      user_name = "${nodes.machine.ronoctua.user}"
      helper = "${ronoctua.filesRoot}/bin/ronoctua-files-root"
      door_helper = "${ronoctua.securityRoot}/bin/ronoctua-security-root"
      password = "a door, not a rule"


      def as_user(command):
          return (
              person
              + "--setenv=PATH=/run/wrappers/bin:/run/current-system/sw/bin "
              + f"-- ${pkgs.bash}/bin/bash -c {shlex.quote(command)} < /dev/null"
          )


      def door(typed):
          # EXACTLY what `ronoctua_security::authority` runs after `sudo -n`
          # failed: nothing but LC_ALL=C, a session with no terminal, and the
          # password with a line break on stdin, then EOF.
          return (
              f"printf '%s\\n' {shlex.quote(typed)} | "
              + person
              + "-- ${pkgs.coreutils}/bin/env -i LC_ALL=C ${pkgs.util-linux}/bin/setsid --wait "
              + f"/run/wrappers/bin/sudo -S -k -p ''' -- {door_helper} files-root"
          )


      def stage(verb, path, value):
          # As the PERSON, into the directory that is theirs.
          machine.succeed(as_user(
              f"printf %s {verb} > /run/ronoctua-files/staged/verb && "
              f"printf %s {shlex.quote(path)} > /run/ronoctua-files/staged/path && "
              f"printf %s {shlex.quote(value)} > /run/ronoctua-files/staged/value"
          ))

      machine.start()
      machine.wait_for_unit("multi-user.target")

      # THE PERSON, as the footer is: a process of theirs outside any session,
      # with no terminal — under `su -` it would inherit the test backdoor's
      # console, and systemctl would start a polkit agent waiting on it.
      uid = machine.succeed(f"id -u {user_name}").strip()
      person = f"systemd-run --wait --pipe --quiet --uid={uid} --gid=100 "

      with subtest("the privileged unit exists and runs the extracted binary"):
          machine.succeed("systemctl cat ronoctua-files-root.service > /dev/null")
          started = machine.succeed(
              "systemctl show ronoctua-files-root.service -p ExecStart --value"
          )
          assert helper in started, (
              f"the unit does not run the extracted binary:\n{started}"
          )

      with subtest("and the binary does NOT carry the desktop along"):
          # It runs as root. Handing it the whole workspace would put wgpu, the
          # wasm runtime, the renderer and its three embedded fonts inside a
          # unit that runs as root — a hundred megabytes to perform four
          # syscalls.
          closure = machine.succeed(f"nix-store -qR {helper} | wc -l").strip()
          for unwanted in ["wgpu", "wasmtime", "noctua-iosevka", "mesa"]:
              found = machine.succeed(f"nix-store -qR {helper} | grep -c -- '{unwanted}' || true")
              assert found.strip() == "0", (
                  f"the privileged helper carries {unwanted}"
              )
          print(f"{closure} paths in the helper, and none of them is the desktop's")

      with subtest("the directory the request is written into belongs to the person"):
          # AND THE BOOT DID NOT COMPLAIN. `systemd-tmpfiles` writes one line
          # and carries on: a rule naming a group that does not exist leaves the
          # directory uncreated, and the failure shows up much later as an
          # operation that says nothing and does nothing.
          boot = machine.succeed("journalctl -b -u systemd-tmpfiles-setup --no-pager")
          assert "ronoctua-files" not in boot, (
              "tmpfiles complained about our rule:\n" + boot
          )
          owner = machine.succeed("stat -c %U /run/ronoctua-files/staged").strip()
          assert owner == user_name, (
              f"/run/ronoctua-files/staged belongs to `{owner}`; the manager runs "
              "as the person and would not manage to write"
          )
          mode = machine.succeed("stat -c %a /run/ronoctua-files/staged").strip()
          assert mode == "700", (
              f"the directory is {mode}: another person on the machine could leave "
              "a request for a unit that runs as root"
          )

      with subtest("no polkit rule names the unit, and the person cannot start it past the door"):
          # THE RULE THAT USED TO BE HERE is gone: `AUTH_SELF_KEEP` in a
          # session with no agent refuses at once. What must hold now is that
          # nothing else opens the unit — the person's own `systemctl start`
          # is refused, and so is the door with a wrong password.
          rules = machine.succeed(
              "grep -RIl ronoctua-files-root /etc/polkit-1/rules.d/ 2>/dev/null || true"
          ).strip()
          assert not rules, f"a polkit rule names the manager's unit again: {rules}"
          machine.succeed("mkdir -p /var/test && install -m 0644 /dev/null /var/test/one.txt")
          stage("chown", "/var/test/one.txt", uid)
          machine.fail(as_user("systemctl start ronoctua-files-root.service"))
          machine.fail(door("not the password"))
          owner = machine.succeed("stat -c %U /var/test/one.txt").strip()
          assert owner == "root", f"the file changed owner with no door: `{owner}`"

      with subtest("the door APPLIES what the person asked for"):
          # The person's password on stdin, as the footer's door pipes it: the
          # helper's `files-root` starts the unit, and the unit validates and
          # acts.
          stage("chown", "/var/test/one.txt", uid)
          machine.succeed(door(password))
          became = machine.succeed("stat -c %U /var/test/one.txt").strip()
          assert became == user_name, f"the owner became `{became}`"

      with subtest("and it consumes the request, so it is not repeated"):
          # A request left behind would be replayed by the next
          # `systemctl start` — from anywhere — which turns one authorisation
          # into a standing one.
          left = machine.succeed("ls /run/ronoctua-files/staged | wc -l").strip()
          assert left == "0", f"{left} files of the request were left over"

      with subtest("a group or permissions the kernel refuses go through the same door"):
          # What the manager does when `chmod` or `chgrp` as the person comes
          # back EPERM (`files::kernel_or_root`): the same verb staged, the
          # same door, the same password.
          machine.succeed("install -m 0600 -g root /dev/null /var/test/two.txt")
          machine.fail(as_user("chmod 644 /var/test/two.txt"))
          stage("chmod", "/var/test/two.txt", "644")
          machine.succeed(door(password))
          mode = machine.succeed("stat -c %a /var/test/two.txt").strip()
          assert mode == "644", f"the permissions became {mode}"
          machine.fail(as_user("chgrp 100 /var/test/two.txt"))
          stage("chgrp", "/var/test/two.txt", "100")
          machine.succeed(door(password))
          group = machine.succeed("stat -c %g /var/test/two.txt").strip()
          assert group == "100", f"the group became {group}"

      with subtest("a symbolic link is REFUSED, and not followed"):
          # The sharp one: the person is shown a link, and a swap between the
          # click and the unit makes root act on whatever it points at.
          machine.succeed("install -m 0600 /dev/null /var/test/target")
          machine.succeed("ln -s /var/test/target /var/test/shortcut")
          machine.succeed("printf chmod > /run/ronoctua-files/staged/verb")
          machine.succeed("printf /var/test/shortcut > /run/ronoctua-files/staged/path")
          machine.succeed("printf 777 > /run/ronoctua-files/staged/value")
          machine.fail("systemctl start ronoctua-files-root.service")
          mode = machine.succeed("stat -c %a /var/test/target").strip()
          assert mode == "600", (
              f"the link's target became {mode}: the unit followed the link"
          )

      with subtest("and so are the store and the pseudo-filesystems"):
          for path in ["/proc/sys/kernel/hostname", "/sys/kernel/vmcoreinfo"]:
              machine.succeed("printf chmod > /run/ronoctua-files/staged/verb")
              machine.succeed(f"printf {path} > /run/ronoctua-files/staged/path")
              machine.succeed("printf 777 > /run/ronoctua-files/staged/value")
              machine.fail("systemctl start ronoctua-files-root.service")

      with subtest("where the manager opens comes from the option, and not from a literal"):
          # A directory somebody keeps their work in is a preference of the
          # MACHINE (invariant 13). Baked into the Rust it would be personal
          # data in a public repository.
          where = machine.succeed(
              "systemctl show-environment | grep RONOCTUA_FILES_START || "
              "grep -R RONOCTUA_FILES_START /etc/set-environment"
          )
          assert "${nodes.machine.ronoctua.files.start}" in where, where
    '';
}
