# The network in a VM — the two tunnels, the ports and the way out, on a
# machine with no radio and no way to the internet.
#
# WHAT IS PROVED HERE
# -------------------
# 1. The connectivity check reaches NetworkManager's own file. Without it the
#    daemon answers `full` for any route at all and the modeline's `no way
#    out` mark could never appear — and the option that writes it is not the
#    one it looks like (`settings.connectivity`, not `connectivity`), which
#    fails at EVALUATION with an error naming three unrelated options.
# 2. TOR IS INSTALLED AND STOPPED. It does not start at boot by the owner's
#    decision, and the owner can start it WITHOUT A PROMPT — which is the one
#    polkit rule this whole plan adds. Both halves are measured: the unit is
#    down at boot, and `systemctl start` from the user's own session succeeds
#    with no agent anywhere.
# 3. The control socket answers our crate, and answers HONESTLY: with no way
#    out there is no circuit, and `circuit-established=0` is the right answer
#    rather than a failure to read.
# 4. Tailscale's daemon is up and says `NeedsLogin`, which is what a machine
#    that has never joined a tailnet says — and it draws NO modeline mark.
# 5. The footer comes up and draws none of the three tunnel marks: nothing at
#    rest is the owner's rule, and a VM is the machine that has nothing.
# 6. `network-facts` reads the sockets: a VM has listeners, and the owner
#    column says `unknown` for the ones that are not ours to see.
#
# WHAT CANNOT BE PROVED HERE: a Wi-Fi network, a signal, a hotspot, a real
# tailnet, a Tor circuit. QEMU has no radio and this machine has no way out.
# Those are proved on the owner's own machine, by eye — `AGENTS.md`'s rule
# that a screen is looked at before the word "pronto".
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
  name = "ronoctua-network";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/network.nix
        ../nix/modules/tailscale.nix
        ../nix/modules/tor.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-network";
      users.users.root.hashedPassword = lib.mkForce null;

      # A SEAT SESSION for the user: the polkit rule is about `subject.user`,
      # and a process outside a session is not the thing being measured.
      services.getty.autologinUser = config.ronoctua.user;
      users.users.${config.ronoctua.user}.linger = true;

      environment.systemPackages = [
        niri
        ronoctua-shell
        pkgs.tor
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

      user = "${nodes.machine.ronoctua.user}"

      def as_user(command):
          """Run `command` in the user's own manager — an ACTIVE session's process."""
          return machine.succeed(
              f"systemd-run --machine={user}@ --user --pipe --wait --quiet " + command
          )

      def as_user_may_fail(command):
          return machine.execute(
              f"systemd-run --machine={user}@ --user --pipe --wait --quiet " + command
          )

      def facts():
          """Every `key=value` the subcommand printed, first one winning.

          BY FIELD AND NOT BY LINE: a line carries several pairs
          (`tor=present established=false bootstrap=0`), and splitting on the
          first `=` alone gives a value with the rest of the line inside it —
          which is how this helper first read `present established=false` as
          the value of `tor` and failed a subtest the product had passed."""
          out = as_user("${ronoctua-shell}/bin/ronoctua-shell network-facts")
          print(out)
          pairs = {}
          for line in out.splitlines():
              for field in line.split():
                  if "=" in field:
                      key, value = field.split("=", 1)
                      pairs.setdefault(key, value)
          return pairs

      machine.start()
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit("NetworkManager.service")

      with subtest("the connectivity check reaches the daemon's own file"):
          # THE OPTION IS NOT THE ONE IT LOOKS LIKE. `connectivity` is not an
          # option of the nixpkgs module at all; `settings.connectivity` is
          # what becomes the `[connectivity]` section of this file.
          conf = machine.succeed("cat /etc/NetworkManager/NetworkManager.conf")
          print(conf)
          assert "[connectivity]" in conf, conf
          assert "nmcheck.gnome.org" in conf, conf
          assert "interval=300" in conf.replace(" ", ""), conf

      with subtest("tor is installed, stopped, and the owner may start it with no prompt"):
          # NOT AT BOOT: the owner's decision, and `wantedBy = []` is how a
          # unit stays installed and down.
          machine.succeed("systemctl show tor.service -p WantedBy | grep -q 'WantedBy=$'")
          state = machine.succeed("systemctl is-active tor.service || true").strip()
          assert state in ("inactive", "failed"), f"tor started on its own: {state}"
          # AND THE ONE POLKIT RULE. There is no agent in this session: a rule
          # that asked would be a rule that refuses, so this succeeding IS the
          # proof that it answers YES.
          machine.succeed(f"loginctl user-status {user} | head -5 || true")
          status, out = as_user_may_fail("${pkgs.systemd}/bin/systemctl start tor.service")
          print(out)
          assert status == 0, f"the owner could not start tor: {out}"
          machine.wait_for_unit("tor.service")
          # AND STOPPING IS THE OTHER HALF OF THE SAME RULE.
          status, out = as_user_may_fail("${pkgs.systemd}/bin/systemctl stop tor.service")
          assert status == 0, f"the owner could not stop tor: {out}"
          machine.succeed("systemctl start tor.service")

      with subtest("the control socket answers, and answers honestly with no way out"):
          # THE UNIT FIRST, AND THEN THE FILE. `systemctl start` above returns
          # as soon as systemd has forked it, so this used to wait fifteen
          # minutes on a path that a unit which never came up was never going
          # to create — and the failure said "file did not appear", which
          # names the symptom and not the cause. `wait_for_unit` fails in
          # seconds and with Tor's own words in the journal.
          try:
              machine.wait_for_unit("tor.service", timeout=120)
          except Exception:
              print(machine.succeed("systemctl status tor.service --no-pager -l || true"))
              print(machine.succeed("journalctl -u tor --no-pager | tail -40 || true"))
              raise
          machine.wait_for_file("/run/tor/control", timeout=120)
          # THE COOKIE IS THE WHOLE OF THE AUTHENTICATION, and the owner reads
          # it by being in the group — not by a rule of ours.
          groups = machine.succeed(f"groups {user}")
          assert "tor" in groups, f"the owner is not in the tor group: {groups}"
          # AS THE OWNER AND NOT AS ROOT. `machine.succeed` runs as root, which
          # reads any file whatever the mode is — so this passed with the
          # group flag removed, which is the whole thing it exists to prove.
          machine.succeed(f"runuser -u {user} -- test -r /run/tor/control_auth_cookie")
          mode = machine.succeed("stat -c '%a %U:%G' /run/tor/control_auth_cookie").strip()
          print(mode)
          assert mode.startswith("640"), f"the cookie is not group-readable: {mode}"
          out = facts()
          assert out["tor"] == "present", "the control socket did not answer"
          # NO WAY OUT, SO NO CIRCUIT: `0` is Tor's own answer and the honest
          # one — a failure to read would say `absent`.
          assert out["established"] == "false", out

      with subtest("tailscale is up, needs a login, and is no mark at all"):
          machine.wait_for_unit("tailscaled.service")
          machine.wait_for_file("/run/tailscale/tailscaled.sock")
          # THE OPERATOR IS WHAT MAKES THE SOCKET REACHABLE, and the nixpkgs
          # module only applies it when an auth key is configured — which this
          # machine has not got, because it logs in by hand. Without the unit
          # below the crate reports the daemon ABSENT on a machine where it is
          # plainly running, which is what this test found.
          machine.wait_for_unit("ronoctua-tailscale-operator.service")
          # WHAT THE SOCKET LOOKS LIKE, printed: when this subtest fails, the
          # answer is almost always in these two lines.
          print(machine.succeed("ls -l /run/tailscale/ || true"))
          print(machine.succeed("journalctl --no-pager -u ronoctua-tailscale-operator || true"))
          # THE OWNER CAN DRIVE THE DAEMON, which is the property that
          # matters and the only one that can be asked.
          #
          # NOT THE SOCKET'S MODE: it is `0666` either way (the module says
          # so), so `test -w` passed with the operator never set — the exact
          # state that made the crate report the daemon absent. And NOT the
          # `OperatorUser` field either: `tailscale debug prefs` does not
          # print it, which this test found by asserting it and failing while
          # the daemon's own journal showed the operator being set.
          machine.succeed(f"runuser -u {user} -- tailscale debug prefs > /dev/null")
          # WHAT THE DAEMON ANSWERS THE OWNER, in its own words: `403` is a
          # refusal (the operator did not take), `200` is an answer, and
          # anything else is our request being wrong. Printed and not
          # asserted, because the assertion below is the product's own.
          print(machine.succeed(
              f"runuser -u {user} -- ${pkgs.curl}/bin/curl -s -o /dev/null -w '%{{http_code}}' "
              "--unix-socket /run/tailscale/tailscaled.sock "
              "-H 'Host: local-tailscaled.sock' "
              "http://local-tailscaled.sock/localapi/v0/status || true"
          ))
          out = facts()
          # THE DAEMON ANSWERED, and what it said is that nobody has joined a
          # tailnet — which draws nothing.
          assert out["tailscale"] in ("NeedsLogin", "NoState", "Stopped"), out
          assert out["running"] == "false", out
          # AND THE VENDOR TABLE REACHED THE USER'S OWN ENVIRONMENT: without
          # it every machine on the local network would have a blank column
          # and nothing would say why.
          assert int(out["vendors"]) > 1000, out

      with subtest("the sockets are read, and what is not ours says so"):
          out = as_user("${ronoctua-shell}/bin/ronoctua-shell network-facts")
          listening = int(facts()["listening"])
          assert listening > 0, "no listening socket at all on a machine with daemons"
          # THE OWNER COLUMN IS HONEST: `/proc/<pid>/fd` is not ours to read
          # for somebody else's process, and the screen says so rather than
          # leaving a blank that reads as "nothing is there".
          assert "owner=unknown" in out, out
          # AND THE VENDOR TABLE IS WHERE THE VARIABLE SAYS — the file, not
          # the variable: an environment entry pointing at nothing is what a
          # blank maker column looks like.
          table = machine.succeed(
              f"systemctl --machine={user}@ --user show-environment "
              "| grep RONOCTUA_NMAP_MAC_PREFIXES"
          ).strip()
          print(table)
          path = table.split("=", 1)[1]
          machine.succeed(f"test -s {path}")

      with subtest("the footer comes up and draws none of the tunnel marks"):
          start_xvfb()
          start_niri("niri", "${niri}")
          sock = socket_of("niri")
          machine.succeed(
              "systemd-run --unit=footer ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 "
              "--setenv=HOME=/root "
              f"--setenv=NIRI_SOCKET={sock} "
              "--setenv=RUST_LOG=ronoctua_shell=debug "
              "${ronoctua-shell}/bin/ronoctua-shell"
          )
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -q 'footer layout resolved'", timeout=60
          )
          # AFTER THE TUNNEL THREAD HAS ASKED AT LEAST ONCE. It polls every
          # ten seconds and the first reading is immediate, but reading the
          # journal before it lands would pass on a footer that simply had not
          # looked yet — which is a test that cannot fail.
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -q 'footer layout resolved'", timeout=60
          )
          machine.sleep(12)
          journal = machine.succeed("journalctl --no-pager -u footer")
          for mark in ("tailscale", "tor", "vpn", "airplane"):
              assert f'id="{mark}"' not in journal, f"a mark with nothing behind it: {mark}"
          # AND `net-off` IS THERE, because this machine really has no way out
          # — the mark's own rule, measured rather than assumed absent.
          assert 'id="net-off"' in journal, "the way-out mark never appeared" 

      with subtest("the view opens on a machine with no radio, and draws"):
          machine.succeed(
              "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
              "${ronoctua-shell}/bin/ronoctua-shell command network-open"
          )
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep -qE 'layout resolved width=[0-9]+ height=[0-9]{3}'",
              timeout=30,
          )
          # THE REGION GREW PAST THE FOOTER'S OWN TWO LINES, and the number
          # is measured rather than assumed: the resting height is two lines
          # of the token font, and anything taller is the view on screen. The
          # assertion used to be `> 60` after a wait whose regex already
          # demanded three digits — a test that could not fail.
          journal = machine.succeed("journalctl --no-pager -u footer")
          import re
          heights = [int(h) for h in re.findall(r"layout resolved width=\d+ height=(\d+)", journal)]
          resting = min(heights)
          assert max(heights) >= resting * 3, (
              f"the view did not open: rested at {resting}, grew to {max(heights)}"
          )
    '';
}
