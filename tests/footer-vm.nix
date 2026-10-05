# VM test: the footer comes up, creates a graphics context and DRAWS.
#
# WHAT THIS PROVES THAT `cargo build` DOES NOT
# --------------------------------------------
# That the layer-shell surface is accepted by a real compositor, that
# Wayland's pointers reach wgpu, that the embedded face loads, and that a
# frame is drawn with no error. Compiling proves none of that: all four are
# runtime things.
#
# The compositor runs on the wlroots X11 backend over an Xvfb. The why — the
# DRM backend finds no device here and is left with zero outputs — is in
# `tests/compositor-xvfb.nix`, along with the arrangement itself.
#
# GPU: `lavapipe`, Mesa's software Vulkan rasterizer.
# Section 9.3 of the plan decided that — hermetic, deterministic, runs in CI
# with no GPU. And it made the other side explicit: **the performance targets
# of section 5.2 are NEVER measured here**. Nailing down 16 ms over software
# rasterization would measure the software, not the system.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-footer";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.wayland-utils
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;

    # WHAT `OnFailure=` RAISES, standing in for the lifeline: the watchdog
    # subtest proves a hung footer FAILS, and that its failure starts the unit
    # named there — the arrangement `nix/modules/scroll.nix` gives the real
    # footer and the real lifeline.
    systemd.services.lifeline-probe = {
      serviceConfig.Type = "oneshot";
      script = "touch /run/lifeline-raised";
    };
  };

  testScript = ''
    ${xvfb.prelude}


    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the compositor comes up on the X11 backend over Xvfb"):
        start_xvfb()
        start_compositor("compositor", "${scroll}")

    with subtest("the footer creates a graphics context and draws"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )

        # The three proofs, in order: the surface was created, wgpu assembled a
        # context over it, and the layout was resolved for the real width.
        #
        # The `try` exists because `wait_until_succeeds` raises on timeout, and
        # then no print after it runs — without that the failure shows up as a
        # bare timeout, with not one log line from either side.
        # When whoever closes the surface is the COMPOSITOR, the reason is in ITS
        # log, not in the footer's.
        try:
            for marker in [
                "layer-shell surface created",
                "graphics context and renderer ready",
                "footer layout resolved",
            ]:
                machine.wait_until_succeeds(
                    f"journalctl -u footer --no-pager | grep '{marker}' > /dev/null", timeout=60
                )
        except Exception:
            print("--- the footer's journal ---")
            print(machine.execute("journalctl -u footer --no-pager")[1])
            print("--- the compositor journal ---")
            print(machine.execute("journalctl -u compositor --no-pager")[1])
            raise

        # Do NOT call it `log`: that name is already the NixOS test driver's
        # logger object, and overwriting it with a string breaks the type check
        # with a message that does not say so.
        out_text = machine.succeed("journalctl -u footer --no-pager")
        print(out_text)

        # The adapter has to be the software one: if one day this starts saying
        # something else, the test has stopped being hermetic (section 9.3 of the
        # plan) and performance numbers taken from it would not be worth anything.
        assert "llvmpipe" in out_text or "lavapipe" in out_text, (
            f"the footer did not use a software rasteriser; log:\n{out_text}"
        )

        # And NO drawing error. Without this, the footer could be failing every
        # frame and the test would pass on the messages above.
        for bad in [
            "drawing the frame failed",
            "preparing the text failed",
            "no graphics context",
            "panicked",
        ]:
            assert bad not in out_text, f"the footer reported: {bad}"

    with subtest("the footer is still alive after drawing"):
        # A crash right after the first frame would go unnoticed by the asserts
        # above, which only look at the log.
        machine.succeed("systemctl is-active footer")

    with subtest("a footer that hangs is one that dies: the watchdog"):
        # THE UNIT'S OWN THREE SETTINGS (`nix/modules/scroll.nix`), on a
        # transient unit: the footer pets the watchdog from its loop's
        # one-second tick, so it lives while the loop turns — and a `SIGSTOP`,
        # which leaves a process `active` and `OnFailure=` blind, fails it
        # with `Result=watchdog`, the failure that raises the lifeline.
        machine.succeed("systemctl stop footer")
        machine.succeed(
            "systemd-run --unit=hung ${xvfb.env} "
            "-p WatchdogSec=6 -p NotifyAccess=main -p TimeoutAbortSec=2 "
            "-p OnFailure=lifeline-probe.service "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=RUST_LOG=ronoctua_shell=info "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u hung --no-pager | grep 'pets systemd' > /dev/null", timeout=60
        )
        # ALIVE PAST TWO INTERVALS while the loop turns: the pings arrive.
        machine.sleep(14)
        machine.succeed("systemctl is-active hung")
        machine.fail("test -e /run/lifeline-raised")
        machine.succeed("systemctl kill -s STOP hung")
        machine.wait_until_succeeds(
            "systemctl show -p Result --value hung | grep -x watchdog", timeout=60
        )
        # AND THE FAILURE RAISES WHAT `OnFailure=` NAMES — the lifeline, on
        # the real footer unit.
        machine.wait_until_succeeds("test -e /run/lifeline-raised", timeout=30)
  '';
}
