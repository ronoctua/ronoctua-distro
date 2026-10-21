# VM test: the screen resizer (`LEADER / r`, `crates/ronoctua-shell-overlays/src/resizer.rs`).
#
# WHAT ONLY A COMPOSITOR PROVES. The arithmetic of the cut is unit-tested; what
# no unit test reaches is that the two curtains' exclusive zones actually
# leave the workspace the room the file says — the compositor is the one that
# lays everything out in what the zones leave — that a ready size (`p`) and
# the corner's drag move that room, and that ending the resizer gives the
# screen back whole and takes both files with it.
#
# The room is read from the WORKSPACE's own rectangle (`get_workspaces`),
# which is what every window is laid out in: the curtains are measured by
# what they do to the rest of the screen, never by their own size.
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
  name = "ronoctua-resizer";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}

    import json

    SOCK = "/run/user/0/scroll-ipc.0.compositor.sock"
    CUT = "/run/user/0/ronoctua-resizer.json"
    KEEPER = "/run/user/0/ronoctua-resizer.pid"
    # THE LAST SIZE IS KEPT IN THE HOME (`$XDG_STATE_HOME`, or
    # `~/.local/state`), and a `systemd-run` system unit has no `HOME`: with
    # none, the store has nowhere to write and the next resizer starts at the
    # largest room. A session always has one, so the units are given root's.
    LAST = "/root/.local/state/ronoctua/resizer-last.json"

    def workspace_rect():
        said = machine.succeed(
            f"SCROLLSOCK={SOCK} SWAYSOCK={SOCK} ${scroll}/bin/scrollmsg -t get_workspaces"
        )
        focused = [w for w in json.loads(said) if w.get("focused")]
        assert focused, f"no focused workspace: {said}"
        r = focused[0]["rect"]
        return (r["width"], r["height"])

    def cut():
        return json.loads(machine.succeed(f"cat {CUT}"))

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the compositor comes up, and the screen is whole"):
        start_xvfb()
        start_compositor("compositor", "${scroll}")
        # ROOM FOR THE READY SIZES: the backend's window starts 1024 wide,
        # which holds none of them beside a curtain, and the X11 backend reads
        # a resized window as the output's new mode.
        which = focus_compositor()
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool windowsize {which} 1600 1000")
        machine.wait_until_succeeds(
            f"SCROLLSOCK={SOCK} ${scroll}/bin/scrollmsg -t get_workspaces "
            "| grep -q '\"width\": 1600'",
            timeout=60,
        )
        whole = workspace_rect()
        print(f"the whole screen: {whole}")

    with subtest("the keeper raises two curtains, and the workspace is the room"):
        machine.succeed(
            "systemd-run --unit=resizer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={SOCK} --setenv=SWAYSOCK={SOCK} "
            "--setenv=RUST_LOG=ronoctua_shell=info "
            "${ronoctua-shell}/bin/ronoctua-shell resizer"
        )
        machine.wait_until_succeeds(f"test -s {CUT} && test -s {KEEPER}", timeout=60)
        # A CURTAIN IS THE KEEPER'S OWN PROGRAM, started by its path
        # (`current_exe`): the wrapped binary, `.ronoctua-shell-wrapped` — so
        # it is counted by its word, not by the wrapper's name.
        machine.wait_until_succeeds(
            "test $(pgrep -fc '[r]esizer-curtain (right|bottom)$') -eq 2", timeout=60
        )
        c = cut()
        machine.wait_until_succeeds(
            f"SCROLLSOCK={SOCK} ${scroll}/bin/scrollmsg -t get_workspaces "
            f"| grep -q '\"width\": {c['width_px']}'",
            timeout=60,
        )
        room = workspace_rect()
        assert room == (c["width_px"], c["height_px"]), (
            f"the workspace is {room}, and the cut says {c}"
        )
        assert room[0] < whole[0] and room[1] < whole[1], (room, whole)

    with subtest("a ready size, from the bottom curtain's own key"):
        # THE CURTAIN TAKES THE KEYS ON DEMAND: a click on it, then `p`. The
        # bottom curtain is the last strip of the compositor's window.
        (w, h, x, y) = compositor_geometry()
        focus_compositor()
        machine.succeed(
            f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove {x + 10} {y + h - 3} click 1"
        )
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key p")
        # THE FILE MOVES WHEN THE KEY LANDS: polled here, in Python, because
        # what is compared is two readings of it.
        ready = c
        for _ in range(30):
            ready = cut()
            if ready != c:
                break
            machine.sleep(1)
        assert (ready["width_px"], ready["height_px"]) in [
            (1024, 768), (1280, 720), (1366, 768), (1440, 900), (1600, 900), (1920, 1080),
        ], f"`p` landed on {ready}, which is no ready size"
        machine.wait_until_succeeds(
            f"SCROLLSOCK={SOCK} ${scroll}/bin/scrollmsg -t get_workspaces "
            f"| grep -q '\"width\": {ready['width_px']}'",
            timeout=30,
        )

    with subtest("Esc gives the screen back whole, and the files go"):
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Escape")
        machine.wait_until_fails(f"test -e {CUT}", timeout=30)
        machine.wait_until_fails(f"test -e {KEEPER}", timeout=30)
        machine.wait_until_succeeds(
            "test $(pgrep -fc '[r]esizer-curtain (right|bottom)$') -eq 0", timeout=30
        )
        assert workspace_rect() == whole, "the screen did not come back whole"

    with subtest("turned on again, it starts at the last size"):
        last = json.loads(machine.succeed(f"cat {LAST}"))
        assert (last["width_px"], last["height_px"]) == (
            ready["width_px"],
            ready["height_px"],
        ), f"the kept size is {last}, not the last one {ready}"
        machine.succeed(
            "systemd-run --unit=resizer-again ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={SOCK} --setenv=SWAYSOCK={SOCK} "
            "${ronoctua-shell}/bin/ronoctua-shell resizer"
        )
        machine.wait_until_succeeds(f"test -s {CUT}", timeout=60)
        again = cut()
        assert (again["width_px"], again["height_px"]) == (
            ready["width_px"],
            ready["height_px"],
        ), f"it started at {again}, not at the last size {ready}"
  '';
}
