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
#      not; the window's line lets `sudo -n` (and `sudo -E`) in and every lock
#      closes it (`relax-end`, an expired `NOTAFTER`, the sleep hook, the
#      timer's own unit); fingers answer sudo only in a terminal and only for
#      a listed user, and the door — which has no terminal — never waits for
#      one;
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
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-security";

  nodes.machine =
    { ... }:
    {
      imports = [ ../nix/modules/security.nix ];

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
        machine.fail(as_user("systemctl start ronoctua-sudo-close.service"))

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
        until = int(machine.succeed("cat /run/ronoctua-security/sudo-until").strip())
        assert until > int(time.time()) - 600, until
        machine.succeed(f"systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire-{until}")
        machine.succeed("test \"$(stat -c '%a %U' /run/ronoctua-sudo/relax)\" = '440 root'")
        machine.fail("test -e /run/ronoctua-sudo/.relax")

    with subtest("ending it now needs nothing typed, and closes it"):
        machine.succeed(as_user(f"sudo -n -- {helper} relax-end"))
        machine.fail(as_user("sudo -n true"))
        machine.fail("test -e /run/ronoctua-sudo/relax")
        machine.fail("test -e /run/ronoctua-security/sudo-until")
        machine.fail("systemctl list-timers --all --no-pager | grep -q ronoctua-sudo-expire")

    with subtest("an expired line lets nobody in, with no timer at all"):
        line = machine.succeed(f"{helper} print relax-line {user} 1000000000").strip()
        machine.succeed(f"install -m 0440 /dev/null /run/ronoctua-sudo/relax && printf '%s\\n' {shlex.quote(line)} > /run/ronoctua-sudo/relax")
        machine.fail(as_user("sudo -n true"))
        machine.succeed("rm /run/ronoctua-sudo/relax")

    with subtest("the timer's own unit closes the window"):
        machine.succeed(door("relax 5"))
        until = int(machine.succeed("cat /run/ronoctua-security/sudo-until").strip())
        machine.succeed(f"systemctl start ronoctua-sudo-expire-{until}.service")
        machine.fail(as_user("sudo -n true"))

    with subtest("sleeping closes it"):
        machine.succeed(door("relax 15"))
        machine.succeed(as_user("sudo -n true"))
        # `sleep.target` refuses to be started by hand (RefuseManualStart), and
        # a guest that really suspends needs a firmware that offers S3. So the
        # hook's WIRING is read from systemd — the target pulls it in and
        # waits for it — and its WORK is run the way the target runs it.
        for prop in ["WantedBy", "Before"]:
            value = machine.succeed(f"systemctl show -p {prop} --value ronoctua-sudo-close.service")
            assert "sleep.target" in value.split(), (prop, value)
        wants = machine.succeed("systemctl show -p Wants --value sleep.target")
        assert "ronoctua-sudo-close.service" in wants.split(), wants
        machine.succeed("systemctl start ronoctua-sudo-close.service")
        machine.fail(as_user("sudo -n true"))
        machine.fail("test -e /run/ronoctua-sudo/relax")
        machine.fail("test -e /run/ronoctua-security/sudo-until")

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
        # Listed, and a terminal: the finger alone is enough.
        status, said = while_scanning(pam_with_tty, "/tmp/pam-tty")
        assert status == 0, said
        # Listed, and NO terminal — the door's case: the finger is never asked,
        # the password is, and there is none.
        status, said = while_scanning(pam_without, "/tmp/pam-no-tty")
        assert status != 0, said
        # And the password still works there.
        machine.succeed(f"printf '%s\\n' {shlex.quote(password)} | pamtester sudo {user} authenticate")

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
