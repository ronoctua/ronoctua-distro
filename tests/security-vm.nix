# The security centre's foundation, on a machine with a neighbour.
#
# WHAT ONLY THIS PROVES
# ---------------------
# `ronoctua-security` has its tests against stand-ins: a fake `sudo` that
# records its argv and stdin, a fake `nft` that keeps the script, a fake
# `visudo`. The real ones answer here, and so do the three things no unit test
# can see:
#
#   1. the kernel: a SYN refused by the declared table is logged with OUR
#      prefix, and a rule the person allowed through the door lets the
#      neighbour in — the marks crossing from one table to the other;
#   2. sudo and PAM: the password on stdin opens the door and a wrong one does
#      not; the window's line lets `sudo -n` (and `sudo -E`) in, for every
#      span up to no limit, and what ends it ends it (`relax-end`, an expired
#      `NOTAFTER`, the timer's own unit) while sleeping does not; the boot's
#      look closes an expired window and gives a live one its timer back;
#      fingers answer sudo only in a terminal and only for a listed user, the
#      door's password-only run — which has no terminal — never waits for
#      one, and on a terminal the stack says the sentinels the door's
#      conversation listens for; root's list of security keys is written by
#      the door alone, and a key listed but absent never keeps the password
#      from answering;
#   3. polkit: the person raises the shields with no agent in the session, and
#      the same rule starts nothing else;
#   4. NetworkManager: its dispatcher hands every link to the helper, the
#      networks of before are placed at home once, a new one is born public,
#      and one this machine serves is neither — asked of the daemon itself.
#
# THREE NODES, because a firewall that is only asked from inside the machine
# is never asked at all: `machine` runs the table and a web server, `peer`
# knocks; `roamer` runs NetworkManager, which `machine` does not (its test
# network is the framework's, and NetworkManager would take it).
#
# THE PERSON IS A PROCESS WITH NO TERMINAL, as the footer is: every command of
# theirs runs through `systemd-run --uid` with stdin from `/dev/null` (or the
# password's pipe). Under `su -` it would inherit the test backdoor's console,
# which IS a terminal: systemctl would start a polkit agent that waits on it,
# and sudo would give PAM a tty and ask for a finger.
#
# The fingerprint part drives libfprint's `virtual_device` through its socket
# — no reader, the same fprintd.
{
  pkgs,
  lib,
  securityRoot,
}:

let
  user = "noctua";
  # THE TEST'S OWN PASSWORD — a fixture, never a person's.
  password = "a door, not a window";
  helper = "${securityRoot}/bin/ronoctua-security-root";
  # Where the virtual reader listens: fprintd's own state directory, the one
  # place its sandbox (`ProtectSystem=strict`, `PrivateTmp`) lets it bind.
  reader = "/var/lib/fprint/virtual-device";
  # THE SENTENCES THE DOOR LISTENS FOR (`ronoctua_security::authority`):
  # sudo's `-p` prompt, `pam_u2f`'s cue, the line before `pam_fprintd`.
  # `sudo-door-reads-its-own-sentinels` holds the Rust and the PAM stack
  # equal; here they are what the machine's sudo is asked to say.
  passwordPrompt = "Password (ronoctua-sudo:password): ";
  fingerCue = "A fingerprint may answer for the password (ronoctua-sudo:finger)";
  # A CREDENTIAL AS `pamu2fcfg -n` WRITES ONE — for a key nobody plugs in.
  credential = "owBYtXj1Pq+Zb5c1Kd/AbQ==,04d6f1aa,es256,+presence";
  # SUDO ON A TERMINAL, the way the door's conversation runs it: a pty made
  # the controlling terminal (`pty.fork`), an empty environment but
  # `LC_ALL=C`, `-k -p <prompt>`, and the answer written only after the
  # prompt (sudo flushes the terminal's input when it asks) — the password,
  # or the end of input. Everything the terminal said is printed.
  talker = pkgs.writeScript "sudo-talker" ''
    #!${pkgs.python3}/bin/python3
    import os, pty, select, sys, time

    prompt, answer, args = sys.argv[1], sys.argv[2], sys.argv[3:]
    pid, fd = pty.fork()
    if pid == 0:
        os.execv("${pkgs.coreutils}/bin/env", ["env", "-i", "LC_ALL=C", "/run/wrappers/bin/sudo", "-k", "-p", prompt, "--"] + args)
    said, seen, ends = b"", b"", time.monotonic() + 90
    while True:
        ready, _, _ = select.select([fd], [], [], max(0.0, ends - time.monotonic()))
        if not ready:
            break
        try:
            chunk = os.read(fd, 1024)
        except OSError:
            break
        if not chunk:
            break
        said += chunk
        seen += chunk
        if prompt.encode() in seen:
            seen = seen.split(prompt.encode(), 1)[1]
            os.write(fd, answer.encode() + b"\n" if answer else b"\x04")
    _, status = os.waitpid(pid, 0)
    sys.stdout.write(said.decode(errors="replace"))
    sys.exit(os.waitstatus_to_exitcode(status))
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-security";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../nix/modules/security.nix
        ../nix/modules/yubikey.nix
      ];

      # The module reads `config.ronoctua.user`, which belongs to `base.nix` —
      # and base brings the whole system with it. Declaring the one option it
      # needs is the `syncthing-vm` precedent.
      options.ronoctua.user = lib.mkOption { type = lib.types.str; };

      config = {
        ronoctua.user = user;
        ronoctua.security.helper = securityRoot;

        virtualisation.memorySize = 1536;
        virtualisation.cores = 2;

        users.mutableUsers = false;
        users.users.${user} = {
          isNormalUser = true;
          group = "users";
          uid = 1000;
          extraGroups = [ "wheel" ];
          password = password;
        };

        # THE SECURITY KEYS' MODULE, as every machine has it.
        ronoctua.yubikey.enable = true;

        # Something for the neighbour to knock on: a port the declared firewall
        # does not open.
        systemd.services.listener = {
          wantedBy = [ "multi-user.target" ];
          serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 -m http.server 8080 --directory /var/empty";
        };

        # The reader nobody has to touch.
        systemd.services.fprintd.environment.FP_VIRTUAL_DEVICE = reader;

        environment.systemPackages = [
          pkgs.pamtester
          pkgs.socat
        ];
      };
    };

  nodes.peer =
    { ... }:
    {
      environment.systemPackages = [ pkgs.curl ];
    };

  nodes.roamer =
    { ... }:
    {
      imports = [ ../nix/modules/security.nix ];
      options.ronoctua.user = lib.mkOption { type = lib.types.str; };

      config = {
        ronoctua.user = user;
        ronoctua.security.helper = securityRoot;

        virtualisation.memorySize = 1024;
        environment.systemPackages = [ pkgs.jq ];

        users.users.${user} = {
          isNormalUser = true;
          group = "users";
          uid = 1000;
        };

        networking.networkmanager = {
          enable = true;
          # No profile made up for the framework's interfaces: the only
          # connections are the test's.
          settings.main.no-auto-default = "*";
          unmanaged = [
            "eth0"
            "eth1"
          ];
        };

        # A NETWORK OF BEFORE: a profile NetworkManager has from its first
        # start, as a machine where this lands has its Wi-Fi. A keyfile, root
        # `0600` as NetworkManager wants it, there before the daemon starts —
        # so it is stored whichever event comes first.
        environment.etc."NetworkManager/system-connections/before.nmconnection" = {
          mode = "0600";
          text = ''
            [connection]
            id=before
            type=dummy
            interface-name=dummy0
            autoconnect=false

            [ipv4]
            method=manual
            address1=10.61.0.1/24

            [ipv6]
            method=disabled
          '';
        };
      };
    };

  testScript = ''
    import json
    import shlex
    import time

    user = "${user}"
    password = "${password}"
    helper = "${helper}"
    reader = "${reader}"
    prompt = "${passwordPrompt}"
    finger_cue = "${fingerCue}"
    credential = "${credential}"
    window_state = "/var/lib/ronoctua-security/sudo-window"
    window_line = "/var/lib/ronoctua-sudo/relax"
    knock = "curl --silent --fail --max-time 3 http://machine:8080/"


    # The person, as the footer is: a process of theirs outside any session,
    # with no terminal and stdin from nowhere.
    person = "systemd-run --wait --pipe --quiet --uid=1000 --gid=100 "


    def as_user(command):
        return (
            person
            + "--setenv=PATH=/run/wrappers/bin:/run/current-system/sw/bin "
            + f"-- ${pkgs.bash}/bin/bash -c {shlex.quote(command)} < /dev/null"
        )


    def door(verb, typed=password):
        # EXACTLY what `ronoctua_security::authority` runs after `sudo -n`
        # failed: an environment of nothing but LC_ALL=C, a session of its own
        # with no terminal (`setsid --wait`), and the password with a line
        # break on stdin, then EOF.
        return (
            f"printf '%s\\n' {shlex.quote(typed)} | "
            + person
            + "-- ${pkgs.coreutils}/bin/env -i LC_ALL=C ${pkgs.util-linux}/bin/setsid --wait "
            + f"/run/wrappers/bin/sudo -S -k -p ''' -- {helper} {verb}"
        )


    def on_a_terminal(verb, answer=password):
        # sudo as the door's conversation runs it: on a terminal, the answer
        # after the prompt (the end of input for "").
        return as_user(f"${talker} {shlex.quote(prompt)} {shlex.quote(answer)} {helper} {verb}")


    def window():
        return json.loads(machine.succeed(f"cat {window_state}"))


    def stage(settings):
        text = json.dumps(settings)
        machine.succeed(as_user(f"printf '%s' {shlex.quote(text)} > /run/ronoctua-security/staged/wish.json"))


    def settings(shields=False, rules=True):
        return {
            "format": 1,
            "enabled": True,
            "shields": shields,
            "default_profile": "public",
            "networks": {},
            "log": "quiet",
            "rules": [
                {
                    "id": "r-web",
                    "action": "allow",
                    "proto": "tcp",
                    "ports": [[8080, 8080]],
                    "source": None,
                    "interface": None,
                    "profiles": ["home"],
                    "note": "the test's web server",
                }
            ]
            if rules
            else [],
        }


    def while_scanning(command, marker, finger="finger-a", seconds=40):
        # Runs `command` in the background and touches the virtual reader
        # once a second until it ends. Its status lands in `marker`. `set +e`
        # first: the driver runs every command under `set -euo pipefail`, the
        # subshell inherits it, and a refusal would end it before the `echo`
        # — no marker, for exactly the runs that are meant to fail.
        machine.succeed(f"rm -f {marker} {marker}.out")
        machine.succeed(f"(set +e; {command} > {marker}.out 2>&1; echo $? > {marker}) >/dev/null 2>&1 &")
        for _ in range(seconds):
            machine.execute(f"printf 'SCAN {finger}' | socat - UNIX-CONNECT:{reader}")
            if machine.execute(f"test -e {marker}")[0] == 0:
                break
            time.sleep(1)
        status = int(machine.succeed(f"cat {marker}").strip())
        return status, machine.succeed(f"cat {marker}.out")


    start_all()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("listener.service")
    machine.wait_for_open_port(8080)
    peer.wait_for_unit("multi-user.target")

    with subtest("the person's table is in the kernel from the boot"):
        machine.succeed("systemctl is-active ronoctua-firewall.service")
        table = machine.succeed("nft list table inet ronoctua")
        assert "priority filter - 10" in table, table
        status = json.loads(machine.succeed("cat /run/ronoctua-security/firewall.status"))
        assert status["ok"], status
        # The first table was born from the defaults, and kept.
        machine.succeed("test -e /var/lib/ronoctua-security/firewall.json")

    with subtest("a refused SYN is logged with our prefix, at a rate"):
        peer.fail(knock)
        machine.wait_until_succeeds(
            "journalctl -k --no-pager | grep 'ronoctua-refused: ' | grep -q 'DPT=8080'",
            timeout=30,
        )

    with subtest("a wrong password opens nothing"):
        stage(settings())
        machine.fail(door("firewall-apply", typed="not it"))
        peer.fail(knock)
        # sudo refused before the helper ran, so the wish is still there.
        machine.succeed("test -e /run/ronoctua-security/staged/wish.json")

    with subtest("the person's rule, through the sudo door, lets the neighbour in"):
        said = machine.succeed(door("firewall-apply"))
        assert said.strip().endswith("done"), said
        peer.succeed(knock)
        machine.succeed("nft list table inet ronoctua | grep -q 'comment \"r-web\"'")
        # Consumed: the next `firewall-apply` through the door does not replay it.
        machine.fail("test -e /run/ronoctua-security/staged/wish.json")
        # And the password went to sudo's stdin, never to a log line.
        machine.fail(f"journalctl --no-pager | grep -qF {shlex.quote(password)}")

    with subtest("the person raises the shields with no agent, and the neighbour is refused"):
        machine.succeed(as_user("systemctl start ronoctua-firewall-shields.service"))
        machine.succeed("nft list chain inet ronoctua input | grep -q 'ct state new drop'")
        peer.fail(knock)
        # The same rule is one unit and one verb wide: nothing else starts,
        # and the shields do not stop from here.
        machine.fail(as_user("systemctl start ronoctua-firewall.service"))
        machine.fail(as_user("systemctl restart ronoctua-firewall-shields.service"))
        machine.fail(as_user("systemctl start ronoctua-sudo-window.service"))

    with subtest("lowering them is a loosening, and goes through the door"):
        stage(settings(shields=False))
        machine.succeed(door("firewall-apply"))
        peer.succeed(knock)

    with subtest("the table survives an nftables reload, and comes back after a restart"):
        machine.succeed("systemctl reload nftables.service")
        machine.succeed("nft list table inet ronoctua | grep -q r-web")
        peer.succeed(knock)
        machine.succeed("systemctl restart nftables.service")
        machine.wait_until_succeeds("nft list table inet ronoctua | grep -q r-web", timeout=30)
        peer.succeed(knock)

    with subtest("the window opens through the door, and sudo -n works while it lasts"):
        machine.fail(as_user("sudo -n true"))
        machine.succeed(door("relax 5"))
        machine.succeed(as_user("sudo -n true"))
        # With the wheel rule's SETENV: the window takes the password away,
        # and not `sudo -E`.
        machine.succeed(as_user("sudo -n -E true"))
        opened = window()
        until = opened["until-unix-s"]
        assert opened["span"] == "five" and until > int(time.time()) - 600, opened
        machine.succeed(f"systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire-{until}")
        machine.succeed(f"test \"$(stat -c '%a %U' {window_line})\" = '440 root'")
        machine.succeed(f"test \"$(stat -c '%a %U' {window_state})\" = '644 root'")
        machine.fail("test -e /var/lib/ronoctua-sudo/.relax")
        # NOT TMPFS: the folder sudo includes is on the machine's disk.
        fstype = machine.succeed("findmnt -no FSTYPE -T /var/lib/ronoctua-sudo").strip()
        assert fstype != "tmpfs", fstype

    with subtest("starting the count again needs nothing typed, and moves the end"):
        before = window()["until-unix-s"]
        time.sleep(2)
        machine.succeed(as_user(f"sudo -n -- {helper} relax 5"))
        after = window()["until-unix-s"]
        assert after > before, (before, after)
        timers = machine.succeed("systemctl list-timers --all --no-pager")
        assert f"ronoctua-sudo-expire-{after}" in timers and f"ronoctua-sudo-expire-{before}" not in timers, timers

    with subtest("ending it now needs nothing typed, and closes it"):
        machine.succeed(as_user(f"sudo -n -- {helper} relax-end"))
        machine.fail(as_user("sudo -n true"))
        machine.fail(f"test -e {window_line}")
        machine.fail(f"test -e {window_state}")
        machine.fail("systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire")

    with subtest("an expired line lets nobody in, with no timer at all"):
        line = machine.succeed(f"{helper} print relax-line {user} 1000000000").strip()
        machine.succeed(f"install -m 0440 /dev/null {window_line} && printf '%s\\n' {shlex.quote(line)} > {window_line}")
        machine.fail(as_user("sudo -n true"))
        machine.succeed(f"rm {window_line}")

    with subtest("the timer's own unit closes the window"):
        machine.succeed(door("relax 5"))
        until = window()["until-unix-s"]
        machine.succeed(f"systemctl start ronoctua-sudo-expire-{until}.service")
        machine.fail(as_user("sudo -n true"))
        machine.fail(f"test -e {window_state}")

    with subtest("a day, two days, and no limit"):
        for span, minutes in [("1440", 1440), ("2880", 2880)]:
            started = int(time.time())
            machine.succeed(door(f"relax {span}"))
            until = window()["until-unix-s"]
            assert abs(until - started - minutes * 60) < 120, (span, until, started)
            machine.succeed(as_user("sudo -n true"))
        machine.succeed(door("relax no-limit"))
        opened = window()
        assert opened["until-unix-s"] is None and opened["span"] == "no-limit", opened
        machine.fail(f"grep -q NOTAFTER {window_line}")
        machine.fail("systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire")
        machine.succeed(as_user("sudo -n true"))

    with subtest("sleeping no longer closes it"):
        # It ends at its end or by the person (owner, 2026-10-11): nothing
        # sleep.target pulls in runs the helper.
        wants = machine.succeed("systemctl show -p Wants --value sleep.target").split()
        for unit in wants:
            run = machine.succeed(f"systemctl show -p ExecStart --value {unit} || true")
            assert "ronoctua-security-root" not in run, (unit, run)
        machine.succeed(as_user("sudo -n true"))

    with subtest("the boot keeps a window with no limit, and closes an expired one"):
        # A REBOOT, AS THE BOOT SEES IT: the unit the boot runs, run again
        # (`RemainAfterExit`: a restart). The files live on the disk, which
        # a reboot keeps (`findmnt` above).
        machine.succeed("systemctl is-active ronoctua-sudo-window.service")
        machine.succeed("systemctl restart ronoctua-sudo-window.service")
        machine.succeed(as_user("sudo -n true"))
        assert window()["until-unix-s"] is None
        machine.succeed(as_user(f"sudo -n -- {helper} relax-end"))
        line = machine.succeed(f"{helper} print relax-line {user} 1000000000").strip()
        machine.succeed(f"install -m 0440 /dev/null {window_line} && printf '%s\\n' {shlex.quote(line)} > {window_line}")
        expired = json.dumps({"format": 1, "until-unix-s": 1000000000, "span": "five"})
        machine.succeed(f"printf '%s\\n' {shlex.quote(expired)} > {window_state}")
        machine.succeed("systemctl restart ronoctua-sudo-window.service")
        machine.fail(f"test -e {window_line}")
        machine.fail(f"test -e {window_state}")

    with subtest("the boot gives a live window back its timer"):
        machine.succeed(door("relax 1440"))
        until = window()["until-unix-s"]
        # THE TIMER DIES WITH THE MACHINE: a transient unit is not kept.
        machine.succeed("systemctl stop 'ronoctua-sudo-expire-*.timer'")
        machine.fail(f"systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire-{until}")
        machine.succeed("systemctl restart ronoctua-sudo-window.service")
        machine.succeed(f"systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire-{until}")
        machine.succeed(as_user("sudo -n true"))
        machine.succeed(as_user(f"sudo -n -- {helper} relax-end"))
        machine.fail(as_user("sudo -n true"))

    with subtest("a finger is enrolled through the door, touch by touch"):
        machine.wait_until_succeeds("fprintd-list root 2>&1 | grep -qi 'virtual'", timeout=30)
        status, said = while_scanning(
            door("finger-enroll right-index-finger"), "/tmp/enrolment"
        )
        assert status == 0, said
        assert "step enroll-stage-passed" in said, said
        assert said.strip().endswith("done"), said
        listed = machine.succeed(f"fprintd-list {user}")
        assert "right-index-finger" in listed, listed

    with subtest("a finger answers sudo in a terminal, only for a listed user"):
        pam_with_tty = f"pamtester -I tty=/dev/tty1 sudo {user} authenticate < /dev/null"
        pam_without = f"pamtester sudo {user} authenticate < /dev/null"
        # Not listed yet: the first gate skips the finger, and with no
        # password the stack refuses.
        status, said = while_scanning(pam_with_tty, "/tmp/pam-unlisted")
        assert status != 0, said
        machine.succeed(door("finger-sudo on"))
        machine.succeed(f"grep -qx {user} /var/lib/ronoctua-security/fingerprint-sudo")
        machine.succeed("test \"$(stat -c '%a %U' /var/lib/ronoctua-security/fingerprint-sudo)\" = '644 root'")
        # Listed, and a terminal: the finger alone is enough — and the line
        # before it says the cue the door listens for.
        status, said = while_scanning(pam_with_tty, "/tmp/pam-tty")
        assert status == 0, said
        assert finger_cue in said, said
        # Listed, and NO terminal — the door's case: the finger is never asked,
        # the password is, and there is none.
        status, said = while_scanning(pam_without, "/tmp/pam-no-tty")
        assert status != 0, said
        # And the password still works there.
        machine.succeed(f"printf '%s\\n' {shlex.quote(password)} | pamtester sudo {user} authenticate")

    with subtest("on the door's terminal, a finger answers sudo, and the cue comes first"):
        # Listed, a print enrolled: sudo on a terminal says the cue, then
        # waits for the finger, which answers — no password typed, and the
        # verb ran.
        status, said = while_scanning(on_a_terminal("relax-end", answer=""), "/tmp/talk-finger")
        assert status == 0, said
        assert finger_cue in said, said
        assert prompt not in said, "the password was asked before the finger"
        assert said.strip().endswith("done"), said

    with subtest("the door never waits for a finger, even for a listed user"):
        # Listed, a print enrolled, and nobody touching the reader: in a
        # terminal pam_fprintd would wait for a finger (30 s) before the
        # password. The door has no terminal, so the password answers at once.
        started = time.monotonic()
        machine.succeed(door("finger-sudo off"))
        waited = time.monotonic() - started
        assert waited < 15, f"the door waited {waited:.1f} s: a finger was asked for"
        machine.fail(f"grep -qx {user} /var/lib/ronoctua-security/fingerprint-sudo")

    with subtest("no other PAM service asks for a finger"):
        for service in ["login", "su", "polkit-1", "passwd"]:
            machine.fail(f"grep -q pam_fprintd /etc/pam.d/{service}")
        machine.succeed("grep -q pam_fprintd /etc/pam.d/sudo")

    with subtest("root's list of keys is written by the door alone, and the password still answers"):
        machine.succeed(door(f"u2f-keys {credential}"))
        listed = machine.succeed("cat /var/lib/ronoctua-security/u2f_keys").strip()
        assert listed == f"{user}:{credential}", listed
        machine.succeed("test \"$(stat -c '%a %U' /var/lib/ronoctua-security/u2f_keys)\" = '644 root'")
        machine.fail(as_user("printf 'x\\n' >> /var/lib/ronoctua-security/u2f_keys"))
        # pam_u2f IS IN SUDO, AND ONLY THERE, sufficient.
        machine.succeed("grep -q 'sufficient.*pam_u2f.so' /etc/pam.d/sudo")
        for service in ["login", "su", "polkit-1", "passwd", "ronoctua-unlock"]:
            machine.fail(f"grep -q pam_u2f /etc/pam.d/{service} 2>/dev/null")
        # A KEY LISTED AND NOT PLUGGED IN: on a terminal pam_u2f finds no
        # device and gives up at once, and the password answers at the
        # door's prompt.
        started = time.monotonic()
        said = machine.succeed(on_a_terminal("relax-end"))
        assert time.monotonic() - started < 30, "the key's wait kept the password back"
        assert prompt in said and said.strip().endswith("done"), said
        # A WRONG PASSWORD is refused, and nothing runs.
        said = machine.fail(on_a_terminal("relax-end", answer="not it"))
        assert "done" not in said, said
        # Taken out: the list has no line for the person.
        machine.succeed(door("u2f-keys"))
        machine.fail(f"grep -q '^{user}:' /var/lib/ronoctua-security/u2f_keys")

    with subtest("the finger goes away through the door"):
        machine.succeed(door("finger-delete right-index-finger"))
        listed = machine.succeed(f"fprintd-list {user} 2>&1 || true")
        assert "right-index-finger" not in listed, listed

    with subtest("the helper refuses what is not its own"):
        # Not root.
        said = machine.fail(as_user(f"{helper} relax-end"))
        # A unit's verb, through sudo.
        said = machine.fail(door("shields-up"))
        assert "refused not-allowed" in said, said
        # Nothing it does not name.
        said = machine.fail(door("relax 7"))
        assert "refused invalid" in said, said
        # The window's unit verbs, through sudo.
        said = machine.fail(door("relax-boot"))
        assert "refused not-allowed" in said, said
        said = machine.fail(door("relax-expire 1000000000"))
        assert "refused not-allowed" in said, said

    with subtest("firmware is read, and never refreshed on a clock"):
        machine.fail("systemctl list-dependencies timers.target --plain --no-pager | grep -q fwupd-refresh")
        machine.fail("systemctl is-active fwupd-refresh.timer")
        machine.wait_until_succeeds(
            "busctl --system get-property org.freedesktop.fwupd / org.freedesktop.fwupd HostSecurityId",
            timeout=60,
        )

    roamer.wait_for_unit("NetworkManager.service")
    roamer.wait_for_unit("ronoctua-firewall.service")


    def roamer_status():
        return json.loads(roamer.succeed("cat /run/ronoctua-security/firewall.status"))


    def uuid_of(name):
        return roamer.succeed(f"nmcli -g connection.uuid connection show {name}").strip()


    with subtest("the dispatcher hands a link to the helper, and the networks of before are home"):
        roamer.succeed("nmcli connection up before")
        roamer.wait_until_succeeds("grep -q dummy0 /run/ronoctua-security/links.json", timeout=60)
        # NetworkManager's first answer placed every connection it had then.
        roamer.wait_until_succeeds("test -e /var/lib/ronoctua-security/networks-migrated", timeout=60)
        kept = json.loads(roamer.succeed("cat /var/lib/ronoctua-security/firewall.json"))
        assert kept["networks"].get(uuid_of("before")) == "home", kept
        roamer.wait_until_succeeds(
            "test \"$(jq -r '.profiles.dummy0' /run/ronoctua-security/firewall.status)\" = home",
            timeout=60,
        )
        roamer.fail("nft list set inet ronoctua public | grep -q dummy0")

    with subtest("a network made after is born public, and leaves with its link"):
        roamer.succeed(
            "nmcli connection add type dummy ifname dummy1 con-name after autoconnect no "
            "ipv4.method manual ipv4.addresses 10.62.0.1/24 ipv6.method disabled"
        )
        roamer.succeed("nmcli connection up after")
        roamer.wait_until_succeeds("nft list set inet ronoctua public | grep -q dummy1", timeout=60)
        status = roamer_status()
        assert status["profiles"].get("dummy1") == "public", status
        assert uuid_of("after") in status["unplaced"], status
        kept = json.loads(roamer.succeed("cat /var/lib/ronoctua-security/firewall.json"))
        assert uuid_of("after") not in kept["networks"], "placed once, never again"
        roamer.succeed("nmcli connection down after")
        roamer.wait_until_fails("nft list set inet ronoctua public | grep -q dummy1", timeout=60)
        roamer.fail("grep -q dummy1 /run/ronoctua-security/links.json")

    with subtest("a network this machine serves is asked of NetworkManager, and is never public"):
        # A hotspot's profile: `method=shared`, and a UUID nobody placed. The
        # helper is run the way the dispatcher runs it; whether this machine
        # serves the network is the daemon's answer, not a file's.
        roamer.succeed(
            "nmcli connection add type ethernet ifname eth9 con-name served autoconnect no "
            "ipv4.method shared ipv6.method disabled"
        )
        served = uuid_of("served")
        said = roamer.succeed(f"{helper} firewall-apply --link eth9 {served} up")
        assert said.strip().endswith("done"), said
        status = roamer_status()
        assert status["served"] == ["eth9"], status
        assert "eth9" not in status["profiles"] and served not in status["unplaced"], status
        roamer.fail("nft list set inet ronoctua public | grep -q eth9")
        chain = roamer.succeed("nft list chain inet ronoctua input")
        assert "eth9" in chain and "53" in chain and "67" in chain, chain
        roamer.succeed(f"{helper} firewall-apply --link eth9 {served} down")
        assert roamer_status()["served"] == [], roamer_status()
  '';
}
