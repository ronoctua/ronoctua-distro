# How to bring the compositor up in a VM with no GPU — used by every VM test.
#
# # Why Xvfb and not a TTY
#
# A VM has no card, so the compositor's DRM backend finds no device and comes
# up with ZERO outputs — and a layer surface is only ever created on an
# output, so the footer would never exist and the next step would hang with
# nothing said.
#
# wlroots has an X11 backend that opens a WINDOW on an X server, which is the
# same shape as `nix run .#dev` opening a window on the host's session. So one
# `Xvfb` is enough: one process, no second compositor, and `xwd`, `xwininfo`
# and `xdotool` all work on the root window.
#
# # And why `pixman` rather than a software GL
#
# It is a CPU rasteriser with no driver under it: it opens no DRM node, never
# touches EGL and never reaches GBM, which removes a whole class of "a path
# compiled for NixOS does not exist here" from the test machines. See
# `nix/software-gpu.nix`, which is also why `WLR_RENDERER_ALLOW_SOFTWARE` is
# deliberately absent — that variable is read on the GLES path only.
#
# This is the same arrangement as `nix run .#dev`, so the tested path is the
# one somebody looks at every day.

{ pkgs, lib }:

let
  gpu = import ../nix/software-gpu.nix { inherit pkgs lib; };
in
rec {
  # The software Vulkan, from the file that names it once — it is read by
  # `nix run .#png` and by two checks that have nothing to do with a
  # compositor, which is why it does not live here.
  inherit (gpu) icd;

  # NOTHING GOES ON `LD_LIBRARY_PATH`: the compositor links `libxcb`, `libX11`
  # and `libxkbcommon` directly, measured with `ldd` on the built binary, so
  # there is nothing to forge a path for.

  # BOTH PROCESSES RENDER IN SOFTWARE, for two different reasons: the
  # compositor composites with `pixman`, a CPU rasteriser with no driver under
  # it, and the footer draws with `wgpu` through lavapipe. Neither falls back
  # to software on its own.
  env = lib.concatStringsSep " " [
    "--setenv=XDG_RUNTIME_DIR=/run/user/0"
    gpu.env
  ];

  packages = [
    pkgs.xorg-server # Xvfb
    pkgs.xwininfo
    pkgs.xdotool
  ]
  ++ gpu.packages;

  # The Xvfb screen, DELIBERATELY roomy.
  #
  # The compositor's X window has a size of its own (1280x720 to start with)
  # and `output * mode …` does NOT resize it — it changes the output's logical
  # resolution, not the X window. A screen of exactly the size one expected of
  # the window cuts off whatever goes beyond it, and what is left outside is
  # precisely the bottom part: the footer. The symptom is a screenshot in
  # which the footer simply does not exist, with no error anywhere.
  #
  # With room to spare, the window fits whole, and whoever crops finds out the
  # real geometry through `xwininfo` (see `compositor_geometry` below).
  width = 1600;
  height = 1000;

  # The compositor's test config, written to a FILE by Nix.
  #
  # Not by a heredoc in the testScript: the script's indentation would go into
  # the file.
  configCompositor = pkgs.writeText "compositor-test.conf" ''
    # NO ANIMATIONS, and this is a decision rather than a test's convenience.
    #
    # `animations enabled` defaults to YES and the default style is `scale`,
    # which scales a window's CONTENT while it animates and fades it in from
    # transparent. Nothing else in this distro moves on its own — the footer
    # does not animate, invariant 15 refuses shadow and elevation — so motion
    # arriving from the compositor is chrome nobody asked for.
    #
    # AND IT DOES NOT ALWAYS END. Measured in a VM on the X11 backend: a new
    # window sat at 45% brightness with its text unreadable, identical four
    # seconds apart, because the animation had nothing driving it to
    # completion. The screen shows a window that is permanently half-drawn and
    # nothing anywhere says why — `buffer-vm` read a blank screen off it and
    # called the renderer broken.
    animations enabled no

    default_border none
    gaps inner 0
    gaps outer 0
    titlebar_padding 1

    # WRITTEN OUT ALTHOUGH IT IS THE DEFAULT. Twenty-two crops in the
    # screenshot battery assume that a logical pixel is a device pixel, and a
    # scale arriving from anywhere would break all of them at once.
    output * scale 1

    # NO `output * bg` ON PURPOSE: it would spawn a second process in every
    # VM to paint a black that an unpainted desk already is. The tests that
    # photograph the desk pass a config of their own.

    # No `output * mode …` either: that changes the output's LOGICAL
    # resolution, not the size of the X window, and leaving the two different
    # only confuses whoever crops a screenshot.
  '';

  # The testScript's prelude: the Python every compositor test shares.
  #
  # They are FUNCTIONS, and not snippets interpolated at the point of use, for a
  # mechanical reason: Nix's interpolation does not reindent. A multi-line
  # snippet pasted inside a `with subtest():` would go in at column 0 and give
  # an `IndentationError` — an error that points at the generated testScript,
  # not at the file that wrote it. Defined here, at column 0, they can be called
  # from any indentation.
  #
  # # THE REASONS LIVE HERE AND NOT IN THE DOCSTRINGS
  #
  # Every byte of the string below travels as ONE KERNEL ARGUMENT in every
  # test that uses it, and the kernel's wall for one is 128 KiB. Past it the
  # build dies with
  # `Argument list too long`, naming neither the file nor the size. So a
  # helper keeps the sentence that says what it answers, and the argument for
  # why it answers that way is written here, in Nix, which never reaches the
  # kernel.
  #
  # `start_compositor` gives it a `PATH` with a SHELL. Every `exec` the
  # compositor runs is `execlp("sh", "sh", "-c", …)`, and a transient unit's
  # `PATH` here is four store paths with no `sh` in any: every `exec` of every
  # compositor test failed with "execve failed" in a log nobody read. The
  # binds that go through our binary (the view of every workspace's mode, the
  # chords `through_shell` writes) are exactly that, and the session and
  # `.#dev` both hand the compositor a `PATH` that has one.
  #
  # `_the_compositors_window` — it finds the window BY BEING THE ONLY CHILD of
  # the X root, never by its title. This compositor maps a window with no name
  # at all (`0x200004 (has no name): ()  1024x768+0+0`), so a helper that
  # grepped a name said "the compositor's window is not mapped" about a window
  # that was plainly there; the failure is in `modeline-vm`'s log with the
  # `xwininfo` output under it. A title is the application's to change, while
  # being the only child of a root nothing else draws on is a property of the
  # arrangement: `start_xvfb` starts that display for the compositor and for
  # nothing else, and XWayland serves a display of its own. A SECOND child
  # means something arrived that this prelude does not know about, which is
  # worth refusing rather than guessing between.
  #
  # UNLESS THE COMPOSITOR WAS ASKED FOR MORE OUTPUTS. `start_compositor(…,
  # outputs=2)` sets `WLR_X11_OUTPUTS`, and the X11 backend then opens one X
  # window per output — two children of the root, both at `+0+0`, the second
  # on top. So the count that is owed is the count that was ASKED, remembered
  # by `start_compositor`, and anything else is still refused: a caller that
  # asked for one output keeps the refusal it had. With several, the windows
  # are told apart by their X id, which grows in the order the X server made
  # them — the backend's order of creation, NOT the compositor's order on its
  # screen: measured with two, the compositor put `X11-2` at x=0 and `X11-1`
  # at x=1024, so `output=0` is not "the one on the left", and which output
  # name each X id carries was not measured either. `output=` past the count
  # is refused with a sentence. A test that photographs two outputs moves the
  # second off the first (`xdotool windowmove`); the Xvfb screen is 1600 wide
  # and a window starts 1024, so it overlaps unless it is also resized
  # (`xdotool windowsize`, which the backend reads as the output's new mode).
  # Neither is done here yet (`docs/PENDING.md`, 0.00031).
  #
  # `windows_now` — it asks `get_tree` and counts in Python rather than
  # running `ronoctua-contract`, for two reasons. The contract binary is a
  # CONFORMANCE probe that exercises what it checks: its `asks` pass turns an
  # output off and on, its `reread` pass reloads the configuration, and polled
  # once a second by `wait_until_succeeds` it left fifteen outputs behind on
  # the X11 backend and failed a later pass. And `contract | grep -q` cannot
  # work here at all: `-q` exits on the first match, the writer upstream takes
  # SIGPIPE, and the driver runs every command under `set -euo pipefail` — so
  # the pipeline fails BECAUSE the grep succeeded. That false negative reads
  # exactly like a window that never arrived; three tests wore it at once,
  # with a `Broken pipe (os error 32)` panic as the only trace.
  #
  # `windows_by_app_id` — `get_tree` answers with the ROOT NODE of a tree
  # (outputs, then workspaces, then containers), not a flat list, so the walk
  # IS the reading: `{w["app_id"]: w for w in json.loads(out)}` iterates the
  # root dict's keys and dies with `'str' object has no attribute 'get'`,
  # which names the symptom and nothing else. And FLOATING is ancestry, not a
  # field: there is no `is_floating` to read, and reading a name that is not
  # there is falsy for ever — a test asking "did the rule make it float?"
  # would answer no for every window in every run and look like a rule that
  # never matched. A window is floating when it hangs under a `floating_nodes`
  # list, its own `type` being `floating_con`.
  #
  # `window_box` — invariant 4 says the shell does not place windows, so a
  # test may not assume where one went either; the tree is the only thing that
  # decides. Cropping the compositor's whole output instead worked only while
  # one window filled the screen: with a window 512 wide inside a 1024 output,
  # half of every crop was empty desk and `tesseract --psm 6` — which expects
  # one uniform block of text — read nothing at all. The window was drawn, the
  # pixels were there, and the test said the screen was blank.
  #
  # `footer_band` — the footer's height is not a constant: two fixed lines
  # plus whatever the open list took. A crop of a guessed band is mostly desk,
  # and the same `--psm 6` reads NOTHING from an image that is 94% empty —
  # measured, a seventeen-line band around a short list came out at 1.6% mean
  # brightness and the OCR returned an empty string, which reads exactly like
  # a footer that never drew. The footer prints `footer layout resolved
  # width=… height=…` on every relayout, so the last one is the height on
  # screen; nothing new is computed.
  prelude = ''
    # WHICH COMPOSITOR IS RUNNING, set by `start_compositor` and read by
    # `windows_now`. `None` until then, so asking before one is up fails with
    # a name rather than with a path made of the word "None".
    _compositor_package = None
    # HOW MANY OUTPUTS it was asked for, which is how many X windows it owes.
    _compositor_outputs = 1


    def start_xvfb():
        machine.succeed("mkdir -p /run/user/0 && chmod 700 /run/user/0")
        # AND NOTHING THE BRAIN DOES STARTS BY ITSELF: `{"quiet": true}`.
        #
        # The footer wakes a brain beside it when there is no systemd unit for
        # one — which in these VMs is always — and the update's default window
        # is the AFTERNOON, 13h to 18h. So the same battery passed at noon and
        # failed at half past one: the brain announced, its line took a row,
        # and three tests that assert "the footer at rest is two lines"
        # measured three. Nothing in the code had changed; the CLOCK had. Nor
        # may it make room on its disk: a small test disk crosses the floor
        # easily, and the relief's collection of the store would take the
        # paths a test registered and never rooted.
        #
        # QUIET, AND NO MODULE NAMED: this used to switch the two off by their
        # ids, which put a module's id in four test files and would have
        # forgotten the next module that starts something. A quiet brain
        # still publishes its marks and notices, and a run somebody asks for
        # still runs — which is how a scene that photographs the brain's line
        # makes it speak, at the moment it chooses rather than the clock's.
        machine.succeed("mkdir -p /root/.config/ronoctua")
        machine.succeed(
            "printf '%s' '{\"quiet\":true}' > /root/.config/ronoctua/brain.json"
        )
        machine.succeed(
            "systemd-run --unit=xvfb "
            "${pkgs.xorg-server}/bin/Xvfb :99 -screen 0 ${toString width}x${toString height}x24"
        )
        machine.wait_until_succeeds("test -S /tmp/.X11-unix/X99", timeout=60)


    def stop_compositor(unit):
        """Bring the compositor down and clean the sockets up.

        It does NOT remove its sockets on exit, so waiting for a file to
        disappear hangs forever. The right signal is the unit going inactive;
        the cleanup is so the next one can reuse the same display name.
        """
        machine.succeed(f"systemctl stop {unit}")
        machine.wait_until_fails(f"systemctl is-active {unit}", timeout=30)
        machine.succeed("rm -f /run/user/0/wayland-* /run/user/0/scroll-ipc.*.sock")


    def start_compositor(unit, package, display="wayland-1", config="${configCompositor}", outputs=1):
        """Bring the compositor up on the X11 backend over Xvfb, and await it.

        `outputs` is how many screens it has: one X window each
        (`WLR_X11_OUTPUTS`), picked by `output=` in the helpers below.

        `config` defaults to the minimal one above. A test passes its own when
        what it is proving IS a piece of the configuration — `terminal-vm` adds
        the generated window rules, because a rule that is not loaded proves
        nothing about a window floating.

        THE SOCKET'S PATH IS CHOSEN HERE. The compositor honours `SCROLLSOCK`
        when the file does not exist yet, so naming it up front means nothing
        has to glob for it afterwards — and `socket_of` becomes a fact rather
        than a search.
        """
        # THE PACKAGE, REMEMBERED. `windows_now` needs `scrollmsg` and the
        # prelude takes no compositor of its own — asking for one would be a
        # second way to name the build that is running, and the two could
        # differ. This is the build that was started.
        global _compositor_package, _compositor_outputs
        _compositor_package = package
        _compositor_outputs = outputs

        sock = f"/run/user/0/scroll-ipc.0.{unit}.sock"
        machine.succeed(
            f"systemd-run --unit={unit} ${env} "
            "--setenv=DISPLAY=:99 "
            "--setenv=WLR_BACKENDS=x11 "
            f"--setenv=WLR_X11_OUTPUTS={outputs} "
            f"--setenv=SCROLLSOCK={sock} "
            f"--setenv=SWAYSOCK={sock} "
            "--setenv=PATH=${pkgs.bash}/bin "
            f"{package}/bin/scroll -c {config}"
        )
        try:
            machine.wait_until_succeeds(f"test -S /run/user/0/{display}", timeout=90)
        except Exception:
            print(machine.execute("journalctl -u xvfb --no-pager")[1])
            print(machine.execute(f"journalctl -u {unit} --no-pager")[1])
            raise

        # IF `DISPLAY` DID NOT REACH IT, it falls back to looking for a DRM
        # device, finds none, and is left with zero outputs — the next step
        # would hang without saying why. These two turn that into a message.
        journal = machine.succeed(f"journalctl -u {unit} --no-pager")
        for broken in ("Failed to create renderer", "Unable to create backend"):
            assert broken not in journal, (
                f"the compositor said {broken!r}: it did not get the X display"
            )


    def _the_compositors_window(output=0):
        """The X window of one output, as `(id, w, h, x, y)`.

        BY BEING THE ONLY CHILD of the X root (or one of exactly as many as
        outputs were asked for), never by its title; any other count is
        refused rather than guessed between. The argument is in the Nix
        comment above this string.
        """
        import re

        tree_out = machine.succeed("${pkgs.xwininfo}/bin/xwininfo -display :99 -root -children")
        found = []
        for line in tree_out.splitlines():
            m = re.search(
                r"(0x[0-9a-fA-F]+).*?(\d+)x(\d+)\+-?\d+\+-?\d+\s+\+(-?\d+)\+(-?\d+)",
                line,
            )
            if m:
                found.append((m.group(1),) + tuple(int(g) for g in m.groups()[1:]))
        if not found:
            raise AssertionError(
                f"the compositor's window is not mapped in X:\n{tree_out}"
            )
        if len(found) != _compositor_outputs:
            raise AssertionError(
                f"{len(found)} windows are mapped on :99 for "
                f"{_compositor_outputs} output(s), so which one is the "
                f"compositor's is a guess:\n{tree_out}"
            )
        if not 0 <= output < _compositor_outputs:
            raise AssertionError(
                f"output={output} was asked of a compositor started with "
                f"{_compositor_outputs} output(s); they are counted from 0, in "
                "the order the backend made their X windows"
            )
        return sorted(found, key=lambda w: int(w[0], 16))[output]


    def compositor_geometry(output=0):
        """Where the compositor's window sits on the X root: (w, h, x, y).

        Discovered, not assumed: the X window has a size of its own, and
        assuming it fills the screen puts the footer's crop outside of it.
        """
        return _the_compositors_window(output)[1:]


    def focus_compositor(output=0):
        """Give X INPUT FOCUS to the compositor's window.

        With no window manager nobody calls `XSetInputFocus`, so the focus
        stays on `PointerRoot` and every key injected by `xdotool` is lost —
        no error, no log, nowhere. Doing it by hand is the equivalent of
        clicking on the window.
        """
        which = _the_compositors_window(output)[0]
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool windowfocus {which}")
        return which


    def windows_now(sock):
        """How many windows the compositor has, READ and nothing else.

        `get_tree` changes nothing, and the counting is in Python, where
        nothing can take a signal for being read. Why not `ronoctua-contract`
        is in the Nix comment above this string.
        """
        import json

        assert _compositor_package, (
            "windows_now was asked before start_compositor ran, so there is "
            "no compositor and no `scrollmsg` to ask"
        )
        said = machine.succeed(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} {_compositor_package}/bin/scrollmsg"
            " -t get_tree"
        )

        def count(node):
            # A WINDOW IS A NODE WITH A PID — the same rule `read.rs` uses, so
            # the two cannot disagree about what a window is. Workspaces and
            # outputs have none.
            n = 1 if node.get("pid") else 0
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                n += count(kid)
            return n

        return count(json.loads(said))


    def focused_now(sock):
        """The id of the focused window, or `None` — READ and nothing else.

        The same reason `windows_now` exists: `ronoctua-contract` is a
        conformance probe that turns outputs off and on, and a test that only
        wants to know what has the focus should not be exercising the
        compositor to find out.
        """
        import json

        assert _compositor_package, (
            "focused_now was asked before start_compositor ran"
        )
        said = machine.succeed(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} {_compositor_package}/bin/scrollmsg"
            " -t get_tree"
        )

        def look(node):
            if node.get("pid") and node.get("focused"):
                return node.get("id")
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                found = look(kid)
                if found is not None:
                    return found
            return None

        return look(json.loads(said))


    def footer_band(unit="footer"):
        """How tall the footer is right now, in pixels, from its own log.

        ASKED, never guessed: the height is two fixed lines plus whatever the
        open list took. The argument is in the Nix comment above this string.
        """
        import re

        said = machine.succeed(f"journalctl -u {unit} --no-pager")
        heights = re.findall(r"footer layout resolved .*?height=(\d+)", said)
        assert heights, (
            "the footer has not said what height it resolved; is it up?\n"
            + said[-2000:]
        )
        return int(heights[-1])


    def windows_by_app_id(sock):
        """Every window, keyed by `app_id`, with what a test asks about it.

        A WALK, because `get_tree` answers a tree; and `floating` is COMPUTED
        from ancestry, because no field carries it. Both arguments are in the
        Nix comment above this string.
        """
        import json

        said = machine.succeed(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} {_compositor_package}/bin/scrollmsg"
            " -t get_tree"
        )

        found = {}

        def walk(node, floating):
            if node.get("pid") and node.get("app_id"):
                found[node["app_id"]] = {
                    "floating": floating or node.get("type") == "floating_con",
                    "rect": node.get("rect"),
                    "pid": node.get("pid"),
                    "name": node.get("name"),
                }
            for kid in node.get("nodes", []):
                walk(kid, floating)
            for kid in node.get("floating_nodes", []):
                walk(kid, True)

        walk(json.loads(said), False)
        return found


    def window_box(sock, app_id=None):
        """Where the compositor put a window: `(w, h, x, y)`, from the tree.

        READ and never assumed — the tree is the only thing that decides where
        a window went. The argument is in the Nix comment above this string.
        """
        import json

        assert _compositor_package, "window_box was asked before start_compositor ran"
        said = machine.succeed(
            f"SCROLLSOCK={sock} SWAYSOCK={sock} {_compositor_package}/bin/scrollmsg"
            " -t get_tree"
        )

        def look(node):
            if node.get("pid") and (app_id is None or node.get("app_id") == app_id):
                r = node.get("rect") or {}
                return (r.get("width"), r.get("height"), r.get("x"), r.get("y"))
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                found = look(kid)
                if found:
                    return found
            return None

        box = look(json.loads(said))
        # AN EMPTY PYTHON STRING MAY NOT BE WRITTEN HERE. Two single quotes
        # CLOSE the Nix string this whole prelude lives in — a comment is no
        # shelter, the fence does not care — and the file then stops parsing
        # with `syntax error, unexpected 'if'` pointing at a line that reads
        # perfectly. It is the trap `AGENTS.md` names, met from the Python
        # side. So the two cases are named instead of being an inline
        # conditional with an empty branch.
        whose = "any window" if app_id is None else f"a window of {app_id}"
        assert box and all(v is not None for v in box), (
            f"there is no {whose} in the tree:\n{said}"
        )
        return box


    def await_windows(sock, how_many, timeout=60):
        """Waits until the compositor has exactly this many windows."""
        import time

        until = time.time() + timeout
        seen = -1
        while time.time() < until:
            seen = windows_now(sock)
            if seen == how_many:
                return
            time.sleep(1)
        raise AssertionError(
            f"the compositor has {seen} window(s), not {how_many}, after {timeout}s"
        )


    def socket_of(unit):
        """Path to the IPC socket of an already running compositor.

        It is the path `start_compositor` CHOSE, not one found by globbing:
        the compositor takes `SCROLLSOCK` when the file does not exist yet, so
        the name is a fact of the launch rather than the result of a search —
        and two compositors on one machine cannot be confused for each other.
        """
        return f"/run/user/0/scroll-ipc.0.{unit}.sock"
  '';
}
