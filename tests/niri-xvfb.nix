# How to bring niri up in a VM with no GPU — used by `footer-vm` and by
# `niri-contract`.
#
# WHY Xvfb, AND NOT THE TTY
# -------------------------
# niri's DRM backend REJECTS software EGL renderers. In
# `src/backend/tty.rs` of v26.04:
#
#     ensure!(!egl_device.is_software(), "software EGL renderers are skipped")
#
# In a VM with no GPU the only EGL is llvmpipe, so niri comes up, creates the
# socket and is left with ZERO outputs — without failing and without warning.
# A layer-shell surface needs an output; with none, it never comes to exist,
# and the symptom is a test that hangs waiting.
#
# There is no flag and no variable that switches the check off. niri has a
# `Headless` backend, but its `main.rs` passes `headless: false` fixed — only
# its internal tests reach it. Getting there would require a patch, and
# invariant 3 forbids that.
#
# The WINIT backend does not have the rejection (`is_software` appears only in
# `tty.rs`), and niri chooses it by itself when `DISPLAY` or `WAYLAND_DISPLAY`
# is set. Since niri's winit comes with X11 enabled, an `Xvfb` is enough — one
# process, with no second compositor. As a bonus niri runs through
# `systemd-run` like any service, with its own journal; the DRM backend
# required a logind seat and, because of that, autologin on a real tty.
#
# This is the same arrangement as `nix run .#dev` (nested niri), so the tested
# path is the working path.
{ pkgs, lib }:

rec {
  # `lavapipe`: Mesa's software Vulkan rasterizer. Section 9.3 of the plan
  # decided that — hermetic, deterministic, runs in CI with no GPU. And it made
  # the other side explicit: the performance targets of section 5.2 are NEVER
  # measured here.
  icd = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";

  # winit opens the X libraries through `dlopen`, not through linking — so they
  # are not in niri's RPATH, and the nixpkgs package is right not to have them
  # (normal use is pure Wayland). Without this the symptom is a panic in
  # `EventLoopCreation` saying `libXcursor.so.1: cannot open shared object
  # file`, which looks like a broken package and is not.
  #
  # And it is an environment variable of the TEST, not a patch in niri:
  # invariant 3 stays intact.
  #
  # The NEW names, from the top of nixpkgs — and not `with pkgs.xorg`.
  #
  # The `xorg` set was deprecated in 26.05 and warns on every evaluation: it
  # was five lines of noise in every VM check. And the `with` hides the use
  # from a `grep` for `xorg.libX11` — that is how these five survived a
  # cleanup that swapped the other twelve.
  libsX = lib.makeLibraryPath [
    pkgs.libx11
    pkgs.libxcursor
    pkgs.libxi
    pkgs.libxrandr
    pkgs.libxcb
    pkgs.libxkbcommon
  ];

  # It holds for both processes: niri needs software EGL (llvmpipe) for winit,
  # the footer needs software Vulkan (lavapipe) for wgpu. Without forcing it,
  # Mesa tries the hardware driver, fails, and does NOT fall back to software
  # on its own.
  env = lib.concatStringsSep " " [
    "--setenv=XDG_RUNTIME_DIR=/run/user/0"
    "--setenv=LIBGL_ALWAYS_SOFTWARE=1"
    "--setenv=VK_DRIVER_FILES=${icd}"
    "--setenv=VK_ICD_FILENAMES=${icd}"
  ];

  packages = [
    pkgs.mesa
    pkgs.xorg-server # Xvfb
    pkgs.xwininfo
    pkgs.xdotool
  ];

  # The Xvfb screen, DELIBERATELY roomy.
  #
  # niri's winit window has a size of its own (1280x800 in v26.04) and the
  # `output "winit" { mode … }` does NOT resize it — it changes the output's
  # logical resolution, not the X window. A screen of exactly the size one
  # expected of the window cuts off whatever goes beyond it, and what is left
  # outside is precisely the bottom part: the footer. The symptom is a
  # screenshot in which the footer simply does not exist, with no error anywhere.
  #
  # With room to spare, the window fits whole, and whoever crops finds out the
  # real geometry through `xwininfo` (see `niri_geometry` in the prelude).
  width = 1600;
  height = 1000;

  # niri's config, written to a FILE by Nix.
  #
  # Not by a heredoc in the testScript: the script's indentation goes into the
  # file and niri's KDL parser refuses it. And `{ a; b; }` on a single line does
  # not pass either — the nodes need lines of their own.
  configNiri = pkgs.writeText "niri-test.kdl" ''
    prefer-no-csd

    layout {
        gaps 0
        border {
            off
        }
        focus-ring {
            off
        }
    }

    hotkey-overlay {
        skip-at-startup
    }

    // No `output "winit" { mode … }` on purpose: on the winit backend that
    // changes the output's LOGICAL resolution, not the size of the X window,
    // and leaving the two different only confuses whoever crops a screenshot.
  '';

  # The testScript's prelude: three Python functions.
  #
  # They are FUNCTIONS, and not snippets interpolated at the point of use, for a
  # mechanical reason: Nix's interpolation does not reindent. A multi-line
  # snippet pasted inside a `with subtest():` would go in at column 0 and give
  # an `IndentationError` — an error that points at the generated testScript,
  # not at the file that wrote it. Defined here, at column 0, they can be called
  # from any indentation.
  prelude = ''
    def start_xvfb():
        machine.succeed("mkdir -p /run/user/0 && chmod 700 /run/user/0")
        machine.succeed(
            "systemd-run --unit=xvfb "
            "${pkgs.xorg-server}/bin/Xvfb :99 -screen 0 ${toString width}x${toString height}x24"
        )
        machine.wait_until_succeeds("test -S /tmp/.X11-unix/X99", timeout=60)


    def stop_niri(unit):
        """Bring a niri down and clean the socket up.

        niri does NOT remove the socket on exit, so waiting for the file to
        disappear hangs forever. The right signal is the unit going inactive;
        the cleanup is so the next niri can reuse the same display name.
        """
        machine.succeed(f"systemctl stop {unit}")
        machine.wait_until_fails(f"systemctl is-active {unit}", timeout=30)
        machine.succeed("rm -f /run/user/0/wayland-* /run/user/0/niri.*.sock")


    def start_niri(unit, package, display="wayland-1", config="${configNiri}"):
        """Bring a niri up on the winit backend over Xvfb and await the socket.

        `config` defaults to the minimal one above. A test passes its own when
        what it is proving IS a piece of the configuration — `terminal-vm` adds
        the generated `window-rule`s, because a rule that is not loaded proves
        nothing about a window floating.
        """
        machine.succeed(
            f"systemd-run --unit={unit} ${env} "
            "--setenv=DISPLAY=:99 "
            "--setenv=LD_LIBRARY_PATH=${libsX} "
            "--setenv=RUST_LOG=niri=debug "
            f"{package}/bin/niri --config {config}"
        )
        try:
            machine.wait_until_succeeds(f"test -S /run/user/0/{display}", timeout=90)
        except Exception:
            print(machine.execute("journalctl -u xvfb --no-pager")[1])
            print(machine.execute(f"journalctl -u {unit} --no-pager")[1])
            raise

        # If the DISPLAY did not reach niri, it goes back to the DRM backend and is
        # left with zero outputs — the next step would hang without saying why. This
        # assert turns that into a message.
        journal = machine.succeed(f"journalctl -u {unit} --no-pager")
        assert "software EGL renderers are skipped" not in journal, (
            "o niri caiu no backend DRM; DISPLAY nao chegou nele"
        )


    def niri_geometry():
        """Where niri's window sits on the X root: (width, height, x, y).

        Discovered, not assumed: the winit window has a size of its own, and
        assuming it fills the screen puts the footer's crop outside of it.
        """
        import re

        tree_out = machine.succeed("${pkgs.xwininfo}/bin/xwininfo -display :99 -root -children")
        for line in tree_out.splitlines():
            if '"niri"' not in line:
                continue
            m = re.search(r"(\d+)x(\d+)\+-?\d+\+-?\d+\s+\+(-?\d+)\+(-?\d+)", line)
            assert m, f"nao consegui ler a geometria da janela do niri: {line!r}"
            return tuple(int(g) for g in m.groups())
        raise AssertionError(f"a janela do niri nao esta mapeada no X:\n{tree_out}")


    def focus_niri():
        """Give X INPUT FOCUS to niri's window.

        With no window manager nobody calls `XSetInputFocus`, so the focus
        stays on `PointerRoot` and every key injected by `xdotool` is lost —
        no error, no log, nowhere. Doing it by hand is the equivalent of
        clicking on the window.
        """
        import re

        tree_out = machine.succeed("${pkgs.xwininfo}/bin/xwininfo -display :99 -root -children")
        for line in tree_out.splitlines():
            if '"niri"' in line:
                m = re.search(r"(0x[0-9a-fA-F]+)", line)
                assert m, f"nao achei o id da janela do niri: {line!r}"
                machine.succeed(
                    f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool windowfocus {m.group(1)}"
                )
                return m.group(1)
        raise AssertionError(f"a janela do niri nao esta mapeada:\n{tree_out}")


    def socket_of(unit):
        """Path to the IPC socket of an already running niri.

        The name carries the PID inside (`niri.<display>.<pid>.sock`), so
        matching on the unit's PID is exact — with two niris on the same
        machine, taking "the most recent one" would be a race.
        """
        return machine.succeed(
            f"pid=$(systemctl show -p MainPID --value {unit}); "
            "ls /run/user/0/niri.*.$pid.sock"
        ).strip()
  '';
}
