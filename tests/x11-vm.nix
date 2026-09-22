# AN X11 APP KEEPS ITS OWN PID — section 9.1.
#
# "An X11 app opened by the shell shows up in the modeline with the right name,
# not with somebody else's."
#
# WHAT THIS TEST CATCHES
# ----------------------
# Section 7.5 correlates a foreign window to the buffer that opened it by
# PID → cgroup. If the compositor reported the X SERVER's pid instead of the
# client's, that correlation would go on "working": it would find a scope, only
# always the SAME one. Two X11 windows of different buffers would show up as
# the same buffer, with no error and no log anywhere.
#
# So the test is not content with seeing the windows: it reads the pid the
# compositor reports for each and checks the CGROUP that pid is in. Two X11
# apps, two distinct scopes.
#
# WHAT CHANGED, AND WHY THE TEST STILL EXISTS
# -------------------------------------------
# There used to be an `xwayland-satellite` per buffer scope, and this test was
# about the satellites: a shared one would have created every surface in its
# own name. The compositor has XWayland built in now, so there is nothing to
# share and nothing to place — and what makes that SAFE rather than merely
# simpler is a property of wlroots that is worth proving rather than trusting:
# it reads the X client's pid out of the XRes extension and reports THAT.
#
# A promise like "the pid is the app's, not the server's" is exactly the kind
# that is true until an upgrade, and silent when it stops being true. So the
# test survives its own rewrite: same question, one layer down.
#
# WHY `xterm`
# -----------
# It does not go into the distro: it is the test's fixture. What is needed here
# is a program that speaks only X11 — a Wayland app would pass even with
# XWayland switched off entirely, which is precisely the false positive to
# avoid.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  # XWAYLAND ON, WRITTEN OUT. It is the default, and this test is the one
  # place where the default being the default is the whole subject — a config
  # that turned it off would fail here as "the window never arrived", which
  # names the symptom and not the cause.
  configX11 = pkgs.writeText "compositor-x11-test.conf" ''
    # NO ANIMATIONS — see `tests/compositor-xvfb.nix` for the measurement.
    animations enabled no

    default_border none
    gaps inner 0
    gaps outer 0
    titlebar_padding 1
    output * scale 1
    xwayland enable
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-x11";

  nodes.machine = {
    virtualisation.memorySize = 3072;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.xterm
      pkgs.util-linux
    ]
    ++ xvfb.packages;

    # THE FIXTURE ASKS BY FONTCONFIG, and that is why `-fa` is on its command
    # line rather than a font package being on this list.
    #
    # `enableDefaultPackages = false` is what every VM here does — the distro
    # ships one family, and a test that inherited a system's fonts would be
    # measuring somebody else's. `xterm` defaults to the classic
    # `-misc-fixed-…` BITMAP, which is served over the X font path and not by
    # fontconfig: installing the package is not enough, because XWayland
    # serves whatever font path it was given and nothing here gives it one.
    # It then says `cannot load font` and never maps a window, so the count
    # stays at zero and the failure reads as "XWayland did not work" — a wrong
    # diagnosis of a font.
    #
    # `-fa Monospace` asks fontconfig instead, which resolves to the one
    # family this system has.
    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    def open_x11(unit, label):
        """Bring an X11 app up in a scope of its own, as the footer does.

        `-name` AND NOT `-T`: a TITLE belongs to the application and it changes
        it whenever it likes. `xterm -T first` sets one and then replaces it
        with `root@machine: /` the moment the command inside it starts, so a
        test keyed on the title waits ninety seconds for a name that was there
        for a moment.

        AND `-name` AND NOT `-class` either. WM_CLASS is a PAIR — instance,
        then class — and `xterm` writes `XTerm` into the second whatever
        `-class` says. The first is the one an invocation owns, which is
        exactly what is needed to tell two `xterm`s apart.

        `systemd-run` is what `foreign::launch` calls, and what is under test
        is what lives inside the scope rather than who creates it. There is no
        wrapper process any more: the app IS the scope's process, which is one
        of the things the satellite's removal bought.

        `DISPLAY` is the compositor's XWayland, not the `Xvfb` the compositor
        itself runs on — `:99` is where the compositor draws and `:0` is where
        it SERVES. Pointing an X11 app at the first would put it beside the
        compositor instead of inside it, and the window would never arrive.
        """
        machine.succeed(
            f"systemd-run --unit={unit} ${xvfb.env} "
            "--setenv=DISPLAY=:0 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            f"${pkgs.xterm}/bin/xterm -fa Monospace -fs 12 -name {label} -e sleep 600"
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_compositor("compositor", "${scroll}", config="${configX11}")
    sock = socket_of("compositor")

    with subtest("XWayland is the compositor's own, and it is serving"):
        # THE SOCKET IS THE COMPOSITOR'S, and `:0` is the display it takes
        # when nothing else has it. Without this the `xterm` below dies with
        # "cannot open display" and the next subtest fails for another reason.
        try:
            machine.wait_until_succeeds("test -S /tmp/.X11-unix/X0", timeout=90)
        except Exception:
            print(machine.execute("journalctl -u compositor --no-pager")[1])
            raise
        # AND NOTHING ELSE IS SERVING IT. `Xvfb` is on `:99`; a second X server
        # on `:0` would mean the app below connected to something that is not
        # the compositor, and every assertion after it would be about the
        # wrong thing.
        who = machine.succeed("ls /tmp/.X11-unix/")
        assert "X0" in who and "X99" in who, who

    def await_window(label):
        """Waits for a window with this title, by READING the tree.

        NEVER `ronoctua-contract` in a poll, and that is the first of two
        lessons this test paid for. The contract EXERCISES the compositor: its
        `asks` pass turns an output off and on again, which on the X11 backend
        destroys and recreates it. Polled once a second for ninety seconds it
        left fifteen outputs behind (`X11-1` … `X11-15`) and exited non-zero.

        AND NEVER `grep -q` AT THE END OF A PIPE, which is the second and the
        subtler. `-q` exits on the FIRST match, the writer upstream gets
        SIGPIPE, and the driver runs every command under `set -euo pipefail` —
        so the pipeline fails *because the grep succeeded*. The window was
        there the whole time, the pattern matched every second for ninety
        seconds, and the test said it never arrived. The `Broken pipe (os
        error 32)` lines in the log were the only trace.

        `> /dev/null` instead: it reads the input to the end, so nobody is
        killed for being listened to.
        """
        machine.wait_until_succeeds(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree"
            f" | grep '\"instance\": \"{label}\"' > /dev/null",
            timeout=90,
        )


    with subtest("the X11 app becomes a window in the compositor"):
        open_x11("bufx1", "first")
        try:
            await_window("first")
        except Exception:
            print(machine.execute("journalctl -u bufx1 --no-pager")[1])
            print(machine.execute("journalctl -u compositor --no-pager")[1])
            # WHAT THE WINDOW CALLS ITSELF, because every failure of this
            # subtest so far has been about the NAME and not about XWayland:
            # a title that changed under us, then a `-class` that `xterm`
            # ignores. Printing all three turns the next one into a reading
            # rather than an afternoon.
            print("--- what the tree calls its windows:", machine.execute(
                f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree"
                " | grep -oE '\"(class|instance|title)\": \"[^\"]*\"' | sort -u")[1])
            print("--- processes:", machine.execute(
                "ps -eo pid,args | grep -iE 'xterm|Xwayland' | grep -v grep")[1])
            raise

    with subtest("the pid the compositor reports is the APP's, not XWayland's"):
        # THE WHOLE POINT, and the one thing a satellite used to arrange.
        #
        # wlroots reads it out of the XRes extension; if it ever stopped, the
        # pid reported would be XWayland's — one number for every X11 window
        # in the session — and section 7.5 would point every one of them at
        # whichever buffer happened to own that scope.
        said = machine.succeed(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree"
        )
        import json

        def windows(node):
            """Every window, keyed by its WM_CLASS INSTANCE — see `open_x11`."""
            out = []
            who = (node.get("window_properties") or {}).get("instance")
            if node.get("pid") and who:
                out.append((who, node["pid"]))
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                out += windows(kid)
            return out

        found = windows(json.loads(said))
        print(found)
        first = [(n, p) for n, p in found if n == "first"]
        assert first, f"the window named `first` is not in the tree: {found}"
        pid = first[0][1]

        # AND THAT PID IS IN THE APP'S SCOPE. Reading the cgroup is the same
        # question `foreign::scope_from_cgroup` answers in the shell, asked
        # here of a real one.
        cgroup = machine.succeed(f"cat /proc/{pid}/cgroup").strip()
        print(f"first: pid {pid} in {cgroup}")
        assert "bufx1.service" in cgroup, (
            "the compositor reported a pid that is not in the app's scope — "
            f"XWayland's own, most likely.\n  pid {pid} is in {cgroup}"
        )

    with subtest("TWO X11 apps land in DISTINCT cgroups"):
        # This is the test one shared pid does not pass.
        open_x11("bufx2", "second")
        await_window("second")

        said = machine.succeed(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree"
        )
        found = dict(windows(json.loads(said)))
        print(found)
        for label in ("first", "second"):
            assert label in found, f"`{label}` is not in the tree: {found}"

        cgroups = {
            label: machine.succeed(f"cat /proc/{found[label]}/cgroup").strip()
            for label in ("first", "second")
        }
        assert cgroups["first"] != cgroups["second"], (
            "both X11 windows report the SAME pid's cgroup: the correlation "
            f"would point both at one buffer\n{cgroups}"
        )
        for label, unit in (("first", "bufx1"), ("second", "bufx2")):
            assert f"{unit}.service" in cgroups[label], (
                f"`{label}` is in the wrong cgroup: {cgroups[label]}"
            )
  '';
}
