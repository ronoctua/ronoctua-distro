# `xwayland-satellite` per buffer scope — section 9.1.
#
# "An X11 app opened by the shell shows up in the modeline with the right name,
# not with the satellite's."
#
# WHAT THIS TEST CATCHES
# ----------------------
# A SHARED satellite makes every X11 window report its pid. The PID -> cgroup
# correlation of section 7.5 would go on "working": it would find a scope, only
# always the SAME one. Two X11 windows of different buffers would show up as
# the same buffer, with no error and no log.
#
# That is why the test is not content with seeing the window: it checks each
# one's SCOPE. Two X11 apps, two distinct scopes, and each satellite inside the
# scope of the app it serves.
#
# WHY `xterm`
# -----------
# It does not go into the distro: it is the test's fixture. What is needed here
# is a program that speaks only X11, to prove that the satellite is standing —
# a Wayland app would pass even with no satellite at all, which is precisely
# the false positive to avoid.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-x11";

  nodes.machine = {
    virtualisation.memorySize = 3072;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.xwayland-satellite
      pkgs.xterm
      pkgs.util-linux
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    def open_x11(unit, label):
        """Bring an X11 buffer up through the same path the footer uses.

        `ronoctua-shell satellite <display> <program>` IS the scope's process:
        it is what spawns the satellite and the app in the SAME cgroup. Calling
        `systemd-run` here is what `open_foreign` does — what is under test is
        what lives inside the scope, not who creates it.
        """
        machine.succeed(
            f"systemd-run --unit={unit} ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            f"${ronoctua-shell}/bin/ronoctua-shell satellite {unit[-1]} "
            f"${pkgs.xterm}/bin/xterm -T {label} -e sleep 600"
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("the satellite comes up and creates the X socket"):
        open_x11("bufx1", "first")
        try:
            machine.wait_until_succeeds(
                "journalctl -u bufx1 --no-pager | grep -q 'satellite ready'",
                timeout=90,
            )
        except Exception:
            print(machine.execute("journalctl -u bufx1 --no-pager")[1])
            raise
        # The socket HAS to exist: without it the `xterm` dies with "cannot open
        # display", and the following test would fail for another reason.
        machine.succeed("test -S /tmp/.X11-unix/X1")
        # And the DEGRADATION must not have kicked in: it is correct (a Wayland app
        # works with no satellite) and would mask precisely what this test exists
        # to prove.
        journal = machine.succeed("journalctl -u bufx1 --no-pager")
        assert "no satellite" not in journal, journal

    with subtest("the X11 app becomes a window in the compositor"):
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 1'",
            timeout=90,
        )

    with subtest("TWO X11 buffers land in DISTINCT cgroups"):
        # This is the test a shared satellite does not pass.
        open_x11("bufx2", "second")
        machine.wait_until_succeeds(
            "journalctl -u bufx2 --no-pager | grep -q 'satellite ready'",
            timeout=90,
        )
        machine.succeed("test -S /tmp/.X11-unix/X2")

        # `cgroup.procs` and not `systemd-cgls`: the second draws a TREE with `|`,
        # `-` and the command on the same line, and filtering by "digits only"
        # matches nothing — the list comes out empty and the assert blames the
        # right code for the wrong reason.
        cgroups = {}
        for unit in ["bufx1", "bufx2"]:
            pids = machine.succeed(
                f"cat /sys/fs/cgroup/system.slice/{unit}.service/cgroup.procs"
            ).split()
            commands = {
                p: machine.succeed(f"tr '\\0' ' ' < /proc/{p}/cmdline || true").strip()
                for p in pids
            }
            print(f"{unit}: {commands}")

            sat = [p for p, c in commands.items() if "xwayland-satellite" in c]
            assert sat, f"{unit} has no satellite of its own: {commands}"
            assert any("xterm" in c for c in commands.values()), (
                f"{unit} does not have the app: {commands}"
            )
            # THE SATELLITE'S CGROUP is what matters: it is its pid that niri reports
            # for every X11 window (the Wayland client is the satellite, not the
            # xterm). The correlation of section 7.5 reads that cgroup.
            cgroups[unit] = machine.succeed(f"cat /proc/{sat[0]}/cgroup").strip()

        assert cgroups["bufx1"] != cgroups["bufx2"], (
            "both satellites are in the SAME cgroup: the PID -> "
            f"the cgroup would point both windows at the same buffer\n{cgroups}"
        )
        for unit, cg in cgroups.items():
            assert f"{unit}.service" in cg, (
                f"{unit}'s satellite is in the wrong cgroup: {cg}"
            )

    with subtest("both X11 windows reached the compositor"):
        # With a single satellite, the second window would show up with the first
        # one's pid — and the modeline would show the same buffer twice.
        machine.wait_until_succeeds(
            f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'windows: 2'",
            timeout=90,
        )
  '';
}
