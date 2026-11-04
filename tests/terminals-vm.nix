# THE TERMINALS AS THE SESSION SEES THEM: the footer, its brain and the
# compositor around held terminals.
#
# `terminal-vm` proves a terminal outlives its window. This proves what the
# rest of the session does with that: the group's strip in the window that
# holds two, the modeline's mark for the terminals no window shows (its
# count, and its colour when one asks for the person), the view that lists
# them and opens their group again, a program's notification on the
# minibuffer's line, an agent of Claude Code lighting its account's set, and
# `terminal-show` bringing an agent's window forward.
#
# WHAT IS READ, AND WHERE
# -----------------------
# What the footer DRAWS is read off its own log, never guessed: every layout
# it resolves is followed by one line per segment placed (`segment id=…
# column=… text=…`), so the mark's count and an account set's glyphs are
# text a test compares. What only pixels say is read off the screen and
# compared with the default theme's solved colours (`themed`): the strip's
# lines, and the mark's colour — a log line saying "caution" would prove the
# footer meant it, not that it drew it. Who has the keyboard is the
# compositor's answer (`focused` in its tree), and which window shows a
# terminal is the holder's (`viewer` in its state file).
#
# THE GLYPHS ARE THE GENERIC TOKENS (`assets/tokens/icons.nix`), read from
# the generated table: `terminals-away`, and the program states
# `program-working` / `program-blocked`, the Nerd Font glyphs an agent's
# marks are drawn with. No brain module is named here
# (`brain-modules-are-sealed`): a set is found by its shape on the modeline,
# `brain:<module>/<n>`, and `n` is the account's place in the list.
#
# THE AGENT IS A FAKE CLAUDE CODE (`fakeClaude`), and only what the distro
# reads of the real one: it asks the terminal whether it speaks OSC 7501
# (`ESC ] 7501 ; ?`) and waits for the answer, runs the managed hook
# (`ronoctua-llm hook`) with the payloads Claude Code sends it, and reports
# its state over OSC 7501. No VM can carry Claude Code itself.
#
# THE KEYS ARE TYPED where a key is what is proved: `LEADER RET b` then
# `RET` open the view and the group. The second terminal of the group is the
# window's shortcut (`LEADER Alt t`), sent on its socket as the footer sends
# it. `terminal-show` and `agents-show` are commands through the control
# socket, as the modeline's click runs them.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
  # The accounts' CLI (`rust.llm`): its `hook` is what Claude Code runs.
  llm,
  # The generated icon table (`icons.json`): a glyph's character by token.
  glyphdb,
  # The solved palettes and the default theme, which every window here is
  # told to wear: the colours a pixel is compared with.
  themed,
  defaultTheme,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  # A CELL UNROUNDED: a segment is placed by multiplying it.
  cellPx = typography.size.base * typography.cellWidthEm;
  palette = "${themed}/${defaultTheme}.json";

  # A PROGRAM A TEST DRIVES FROM OUTSIDE — the same as `terminal-vm`'s,
  # whose comment says what each of its files under `/tmp/held/` is. Here
  # it is also the session's `SHELL`, so a terminal the group starts runs it.
  # A read that timed out keeps its part (see `terminal-vm`'s).
  held = pkgs.writeShellScript "held-program" ''
    set -u
    name=''${RONOCTUA_TERMINAL:-here}
    dir=/tmp/held
    ${pkgs.coreutils}/bin/mkdir -p "$dir"
    printf '%s' "$PATH" > "$dir/$name.path"
    : > "$dir/$name.ran"
    ${pkgs.coreutils}/bin/rm -f "$dir/$name.in"
    ${pkgs.coreutils}/bin/mkfifo "$dir/$name.in"
    exec 3<>"$dir/$name.in"
    echo "$name" >> "$dir/started"
    part=""
    while :; do
      ${pkgs.coreutils}/bin/date +%s%N >> "$dir/$name.beat"
      if IFS= read -r -t 1 line <&3; then
        line="$part$line"
        part=""
        eval "$line"
        printf '%s\n' "$line" >> "$dir/$name.ran"
      else
        part="$part$line"
      fi
    done
  '';

  # A FAKE CLAUDE CODE, run as the program of a held terminal:
  # `<llm> <account folder> <session> <room>`. In the order the real one
  # does it: the probe, and nothing said over OSC 7501 unless it is
  # answered; the hook's start and prompt (which tie the session to its
  # account's folder and its terminal); "working"; then — only once the
  # test writes `<room>/block` — "blocked" on a permission, with NO hook
  # beside it, so the session's file still says working and only the
  # terminal can have said it waits. Each step leaves a file in `<room>`.
  fakeClaude = pkgs.writeText "fake-claude.py" ''
    import json
    import os
    import select
    import subprocess
    import sys
    import time
    import tty

    llm, account, session, room = sys.argv[1:5]
    terminal = sys.stdin.fileno()
    # RAW, so the terminal's answer arrives as it is sent, with no line to
    # end it.
    tty.setraw(terminal)


    def say(sequence):
        os.write(sys.stdout.fileno(), sequence)


    def left(name, what=b"said\n"):
        # WHOLE OR NOT AT ALL: the test waits for the name to exist and then
        # reads it, and a file between its open and its write reads empty.
        part = os.path.join(room, name + ".part")
        with open(part, "wb") as f:
            f.write(what)
        os.replace(part, os.path.join(room, name))


    def hook(event, **more):
        payload = dict(session_id=session, hook_event_name=event, cwd="/root", **more)
        subprocess.run(
            [llm, "hook"],
            input=json.dumps(payload).encode(),
            env=dict(os.environ, CLAUDE_CONFIG_DIR=account),
            check=True,
        )


    say(b"\x1b]7501;?\x1b\\")
    answer = b""
    until = time.monotonic() + 30
    while time.monotonic() < until and not answer.endswith((b"\x1b\\", b"\x07")):
        ready, _, _ = select.select([terminal], [], [], 1)
        if ready:
            answer += os.read(terminal, 256)
    left("probe", answer)
    if b"\x1b]7501;?:" in answer:
        hook("SessionStart", source="startup")
        hook("UserPromptSubmit")
        say(b"\x1b]7501;state=working:app=claude-code\x1b\\")
        left("working")
        while not os.path.exists(os.path.join(room, "block")):
            time.sleep(0.2)
        say(b"\x1b]7501;state=blocked:kind=permission:app=claude-code\x1b\\")
        left("blocked")
    while True:
        time.sleep(60)
  '';

  # TWO ACCOUNTS of Claude Code, so "its account's set" is a claim: the
  # agent runs in the SECOND one's folder, and the first's set must stay
  # without it.
  accounts = pkgs.writeText "llm-accounts.json" (
    builtins.toJSON {
      format = 1;
      accounts = [
        {
          id = "claude-1";
          kind = "claude";
          name = "First";
          dir = "/root/.claude-first";
          default = true;
        }
        {
          id = "claude-2";
          kind = "claude";
          name = "Second";
          dir = "/root/.claude-second";
          default = false;
        }
      ];
    }
  );
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-terminals";

  nodes.machine = {
    # THE FOOTER, ITS BRAIN AND UP TO THREE TERMINAL WINDOWS at once, each
    # drawing through lavapipe, beside the compositor and the holders.
    virtualisation.memorySize = 3072;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.imagemagick
      pkgs.xwd
      pkgs.python3
      pkgs.util-linux
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    import json
    from datetime import timedelta
    ${xvfb.prelude}


    REGISTRY = "/run/user/0/ronoctua-terms"
    HELD = "/tmp/held"
    ROOM = "/tmp/fake-claude"
    AGENTS = "/run/user/0/ronoctua-agents"
    SESSION = "5e55100a-0000-4000-8000-000000000001"
    LINE = ${toString linePx}
    CELL = ${toString cellPx}
    TOKENS = json.load(open("${palette}"))
    ICONS = json.load(open("${glyphdb.db}/icons.json"))
    AWAY = ICONS["terminals-away"]["char"]
    WORKING = ICONS["program-working"]["char"]
    WAITING = ICONS["program-blocked"]["char"]
    # WHAT EVERY WINDOW STARTED HERE IS GIVEN: the session's runtime folder
    # and GPU, the home, and the default theme — the one the pixels are
    # compared with (with none, the first theme of the list is worn).
    SESSION_ENV = (
        "${xvfb.env} "
        "--setenv=WAYLAND_DISPLAY=wayland-1 "
        "--setenv=HOME=/root "
        "--setenv=RONOCTUA_THEME=${defaultTheme} "
    )


    def hex_of(token):
        """`#RRGGBB` of a token of the default theme; the hash is part of it for ImageMagick."""
        return "#" + TOKENS[token][2:8]


    def until(test, why, seconds=60):
        """Asks `test()` once a second until it holds; fails saying `why` — a sentence, or a
        function that says it with what holds at the end — and the footer's tail.
        A question that cannot be answered YET is asked again, not failed: a holder starts
        its program before it writes its state file, and the footer opens its control
        socket before it logs its first layout. What it last raised is said at the end."""
        raised = None
        for _ in range(seconds):
            try:
                if test():
                    return
                raised = None
            except Exception as e:
                raised = e
            machine.sleep(duration=timedelta(seconds=1))
        try:
            said = why() if callable(why) else why
        except Exception as e:
            said = f"(and saying why raised {e!r})"
        if raised is not None:
            said = f"{said}\n--- the question last raised ---\n{raised!r}"
        raise AssertionError(f"{said}\n--- the footer's last words ---\n{footer_log()[-4000:]}")


    def control(command):
        """A command of the registry through the control socket, as a key or a click runs it."""
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"${ronoctua-shell}/bin/ronoctua-shell command {command}"
        )


    def footer_log():
        """The footer's journal, colours off. `--all`: a segment's line carries glyphs, and
        journalctl shortens a line it does not call printable to a blob."""
        import re

        return re.sub(
            r"\x1b\[[0-9;]*m", "", machine.succeed("journalctl -u footer --no-pager -o cat --all")
        )


    def modeline():
        """The footer's last WHOLE layout: its width, its columns, and each segment placed
        there, `id -> (column, text)`. Whole: a layout says how many segments it placed
        (`segments=`), and one whose lines the journal has not all written yet is passed
        over for the one before — or "the mark is gone" would be read off half a list."""
        import re

        lines = footer_log().splitlines()
        heads = [i for i, line in enumerate(lines) if "footer layout resolved" in line]
        assert heads, "the footer has resolved no layout"
        for at in reversed(range(len(heads))):
            head = lines[heads[at]]
            width = re.search(r"\bwidth=(\d+)", head)
            columns = re.search(r"\bcolumns=(\d+)", head)
            count = re.search(r"\bsegments=(\d+)", head)
            assert width is not None and columns is not None and count is not None, head
            end = heads[at + 1] if at + 1 < len(heads) else len(lines)
            said = [
                m
                for m in (
                    re.search(r'segment id="([^"]+)" column=(\d+) text=(.*)$', line)
                    for line in lines[heads[at] + 1 : end]
                )
                if m is not None
            ]
            if len(said) == int(count.group(1)):
                placed = {m.group(1): (int(m.group(2)), m.group(3)) for m in said}
                return int(width.group(1)), int(columns.group(1)), placed
        raise AssertionError("no layout of the footer's was logged whole")


    def segment(name):
        """What the modeline draws for one segment, or `None` while it is not there."""
        found = modeline()[2].get(name)
        return None if found is None else found[1].strip()


    def account_sets():
        """The brain's sets on the modeline, by their number — an account's place in the list."""
        import re

        out = {}
        for name, (_, text) in modeline()[2].items():
            m = re.fullmatch(r"brain:[a-z0-9-]+/(\d+)", name)
            if m is not None:
                out[int(m.group(1))] = text
        return out


    def shoot():
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")


    def pixels(token, box, fuzz=12):
        """How many pixels of `box` (w, h, x, y) on the last shot are that token's colour.
        Twelve per cent catches a glyph's antialiased body, and no grey answers for orange."""
        w, h, x, y = box
        colour = hex_of(token)
        return int(
            machine.succeed(
                f"magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage -fuzz {fuzz}% "
                f"-fill '#000000' +opaque '{colour}' -fill '#FFFFFF' -opaque '{colour}' "
                "-format '%[fx:int(mean*w*h+0.5)]' info:"
            ).strip()
        )


    def rows(token, box):
        """For each row of pixels of `box` on the last shot, the share of it in that token's
        colour. A line drawn across a window is a row mostly of one colour, which no text is."""
        w, h, x, y = box
        colour = hex_of(token)
        said = machine.succeed(
            f"magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage -fuzz 4% "
            f"-fill '#000000' +opaque '{colour}' -fill '#FFFFFF' -opaque '{colour}' "
            f"-crop {w}x1 +repage -format '%[fx:mean]\\n' info:"
        )
        return [float(v) for v in said.split()]


    def tree():
        return json.loads(
            machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree")
        )


    def window_of(pid):
        """The compositor's node of the window of process `pid`, or `None`."""

        def look(node):
            if node.get("pid") == pid:
                return node
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                found = look(kid)
                if found is not None:
                    return found
            return None

        return look(tree())


    def terminal_windows():
        """The pids of every terminal window the compositor lists."""
        found = []

        def walk(node):
            if node.get("pid") and node.get("app_id") == "br.ronoctua.terminal":
                found.append(node["pid"])
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                walk(kid)

        walk(tree())
        return found


    def focused_pid():
        """The process of the window the keyboard is in, by the compositor's word."""

        def look(node):
            if node.get("pid") and node.get("focused"):
                return node["pid"]
            for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                found = look(kid)
                if found is not None:
                    return found
            return None

        return look(tree())


    def focus(pid):
        machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[pid={pid}] focus'")
        until(lambda: focused_pid() == pid, f"the compositor did not focus the window of {pid}")


    def close_window(pid):
        """The window of `pid` closed as a person closes one: the compositor asks it to."""
        machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg '[pid={pid}] kill'")


    def main_pid(unit):
        return int(machine.succeed(f"systemctl show -p MainPID --value {unit}").strip())


    def state(terminal):
        return json.loads(machine.succeed(f"cat {REGISTRY}/{terminal}.json"))


    def viewer(terminal):
        """Which window's process shows a terminal, by its holder's word; `None` for none."""
        return state(terminal)["viewer"]


    def started(count):
        """The ids of the held programs, in the order they began — once there are `count`."""
        machine.wait_until_succeeds(
            f"test $(wc -l < {HELD}/started) -ge {count}", timeout=timedelta(minutes=1)
        )
        return machine.succeed(f"cat {HELD}/started").split()


    def tell(terminal, line):
        """`line`, run by the program in `terminal` — and waited for: it writes what it ran."""
        import shlex

        ran = f"{HELD}/{terminal}.ran"
        before = int(machine.succeed(f"wc -l < {ran}").strip())
        # A TIMEOUT, because a FIFO nobody reads any more takes no writer.
        machine.succeed(
            "timeout 10 bash -c "
            + shlex.quote(f"printf '%s\\n' {shlex.quote(line)} > {HELD}/{terminal}.in")
        )
        machine.wait_until_succeeds(
            f"test $(wc -l < {ran}) -gt {before}", timeout=timedelta(seconds=30)
        )


    def top_of(pid, tall=64):
        """The top `tall` rows of a window, on the screen: where its strip is drawn, with room
        for whatever the window draws above its grid."""
        node = window_of(pid)
        assert node is not None, f"there is no window of {pid}"
        r = node["rect"]
        _, _, x0, y0 = compositor_geometry()
        return (r["width"], tall, x0 + r["x"], y0 + r["y"])


    def mark_box(name):
        """Where a segment of the modeline is drawn: its cells, on the modeline's line —
        counted from the right edge, as the modeline places them."""
        width, columns, placed = modeline()
        assert name in placed, f"{name} is not on the modeline: {placed}"
        column, text = placed[name]
        _, h0, x0, y0 = compositor_geometry()
        left = x0 + width - (columns - column) * CELL
        return (int(len(text) * CELL) + 1, LINE, int(left), y0 + h0 - 2 * LINE)


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_user_manager()
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("the footer comes up, with its brain and two accounts"):
        machine.succeed(
            f"mkdir -p {HELD} {ROOM} /root/.claude-first /root/.claude-second /root/.config/ronoctua"
        )
        machine.succeed("install -m 0600 ${accounts} /root/.config/ronoctua/llm-accounts.json")
        machine.succeed(
            "systemd-run --unit=footer "
            + SESSION_ENV
            + f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "--setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "--setenv=RONOCTUA_JOURNALCTL=${pkgs.systemd}/bin/journalctl "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_brain=info,ronoctua_notify=info "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'control socket opened' > /dev/null",
            timeout=timedelta(seconds=90),
        )
        # THE POINTER IN THE MIDDLE OF THE ROOM: away from a window's strip,
        # whose line under it would light, and from the modeline, whose
        # hint would speak.
        w0, h0, x0, y0 = compositor_geometry()
        machine.succeed(
            f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove {x0 + w0 // 2} {y0 + h0 // 2}"
        )
        # BOTH ACCOUNTS' SETS, at rest: what the agent below is asked to
        # light one of.
        until(
            lambda: {1, 2} <= set(account_sets()),
            lambda: f"the two accounts' sets never came to the modeline: {account_sets()}",
            seconds=120,
        )

    with subtest("a window holding two terminals draws its strip at its top"):
        # PROVES the group: the window's "new terminal" shortcut — `LEADER
        # Alt t`, sent as the footer sends it; `C-T` typed is the program's —
        # starts a second terminal IN THAT WINDOW (one viewer for both, one
        # group), and from two the window draws a line per terminal across its top —
        # the one in front in `text.primary`, the other in `text.dim`, each
        # about half the width. Read off the pixels, before and after, so a
        # window that drew a strip for one terminal fails as surely as one
        # that drew none.
        machine.succeed(
            "systemd-run --unit=group "
            + SESSION_ENV
            + "--setenv=SHELL=${held} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell terminal"
        )
        first = started(1)[0]
        group = main_pid("group")
        until(lambda: viewer(first) == group and window_of(group) is not None, "the first terminal's window never came")
        # DRAWN, before its top is read for a strip it must not have.
        machine.wait_until_succeeds(
            "journalctl -u group --no-pager | grep 'terminal drawn on the xdg_toplevel' > /dev/null",
            timeout=timedelta(minutes=1),
        )

        # AND ON THE SCREEN: most rows of its top in the terminal's own
        # ground. A draw logged is not yet a frame the compositor showed, and
        # a top read off the bare desk has no strip whatever the window does.
        def top_shown():
            shoot()
            shares = sorted(rows("terminal.background", top_of(group)))
            return shares[len(shares) // 2] >= 0.5

        until(top_shown, "the window's top never showed the terminal's ground")
        band = top_of(group)
        alone = (max(rows("text.primary", band)), max(rows("text.dim", band)))
        assert alone[0] < 0.35 and alone[1] < 0.35, f"a window of ONE terminal drew a strip: {alone}"

        focus(group)
        shortcut(group, "terminal-new")
        second = started(2)[1]
        until(lambda: viewer(second) == group, "the terminal the shortcut started is not in its window")
        assert state(first)["group"] == state(second)["group"], (state(first), state(second))

        def strip_drawn():
            shoot()
            band = top_of(group)
            return max(rows("text.primary", band)) >= 0.35 and max(rows("text.dim", band)) >= 0.35

        until(strip_drawn, "two terminals in one window, and no strip across its top")

    with subtest("closing the window shows the mark: the terminals' glyph and 2"):
        # PROVES the census: while a window shows them the mark is not
        # there, and the moment none does it reads the background
        # terminals' glyph and their count — from the footer's own record of
        # what it placed on the modeline, not from the screen's guess.
        until(lambda: segment("terminals-away") is None, "the mark is up while a window shows both terminals")
        close_window(group)
        until(lambda: viewer(first) is None and viewer(second) is None, "the closed window's terminals still have a viewer")
        until(
            lambda: segment("terminals-away") == f"{AWAY} 2",
            lambda: f"the mark does not read {AWAY!r} 2: {segment('terminals-away')!r}",
        )

    with subtest("a bell in a background terminal turns the mark to caution"):
        # PROVES attention reaches the modeline: the mark is drawn in
        # `text.secondary` and no caution, a `printf '\a'` in one of the
        # terminals no window shows, and the same cells are drawn in
        # `state.caution`. The holder counting the bell is checked on the
        # way, so a failure says which side lost it.
        def drawn_at_rest():
            shoot()
            box = mark_box("terminals-away")
            return pixels("text.secondary", box) >= 4 and pixels("state.caution", box) == 0

        until(drawn_at_rest, "the mark at rest is not drawn in text.secondary alone")
        tell(first, "printf '\\a'")
        until(lambda: state(first)["attention"]["bell"] >= 1, "the holder did not count the bell")

        def drawn_asking():
            shoot()
            return pixels("state.caution", mark_box("terminals-away")) >= 4

        until(drawn_asking, "a background terminal rang, and the mark is not drawn in state.caution")

    with subtest("LEADER RET b opens the view, and RET one window holding the group"):
        # PROVES the way back by the keyboard: the leader, `RET b` (the
        # group's folded key: the footer's door), and the view opens in the
        # footer; `RET` on its first terminal opens ONE window with both of
        # the group — one viewer for the two, and the mark gone.
        before = footer_log().count("leader active")
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        until(lambda: footer_log().count("leader active") > before, "the leader did not open")
        focus_compositor()
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key --delay 150 Return b")
        until(
            lambda: "a region opened region=terminals " in footer_log(),
            "LEADER RET b did not open the background terminals' view",
        )
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key Return")
        until(
            lambda: viewer(first) is not None and viewer(first) == viewer(second),
            "RET did not open one window holding both terminals",
        )
        shown = viewer(first)
        until(
            lambda: terminal_windows() == [shown],
            lambda: f"the windows are {terminal_windows()}, not the one of {shown}",
        )
        until(lambda: segment("terminals-away") is None, "the mark stayed with every terminal shown")

    with subtest("an OSC 9 from a background terminal reaches the minibuffer's line"):
        # PROVES a program's notification crosses from a terminal nobody is
        # looking at: the window closed, `OSC 9` from one of the two, and
        # the footer decides it as the terminals' notification and places it
        # on the minibuffer's line — both read from its log, by its id.
        close_window(shown)
        until(lambda: viewer(first) is None and viewer(second) is None, "the closed window's terminals still have a viewer")
        tell(second, "printf '\\033]9;%s\\007' 'kettle boiled'")
        until(lambda: (state(second).get("notice") or {}).get("body") == "kettle boiled", "the holder did not keep the notice")

        def on_the_line():
            import re

            said = footer_log()
            ours = set(re.findall(r'notification decided id=(\d+) .*catalogue="terminal-notice"', said))
            lined = set(re.findall(r'notification placed id=(\d+) placed="line"', said))
            return bool(ours & lined)

        until(on_the_line, "the terminal's notification was not placed on the minibuffer's line")

    with subtest("a fake Claude Code in a held terminal lights its account's set"):
        # PROVES the agent's way to the modeline: the terminal answers the
        # OSC 7501 probe; the hook ties the session to the SECOND account's
        # folder and to the terminal; "working" lights the working glyph in
        # set 2 and nothing in set 1; then "blocked" said ONLY over OSC 7501
        # — the session's file still says working — turns it to the waiting
        # glyph, which only the terminal's word can do.
        machine.succeed(
            "systemd-run --unit=agent "
            + SESSION_ENV
            + "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell terminal -- "
            "${pkgs.python3}/bin/python3 ${fakeClaude} ${llm}/bin/ronoctua-llm "
            f"/root/.claude-second {SESSION} {ROOM}"
        )
        machine.wait_until_succeeds(f"test -e {ROOM}/probe", timeout=timedelta(minutes=1))
        probe = machine.succeed(f"cat -v {ROOM}/probe")
        assert "]7501;?:states=" in probe, f"the terminal did not answer the OSC 7501 probe: {probe!r}"
        machine.wait_until_succeeds(f"test -e {ROOM}/working", timeout=timedelta(minutes=1))
        machine.wait_until_succeeds(f"test -e {AGENTS}/{SESSION}.json", timeout=timedelta(seconds=30))
        record = json.loads(machine.succeed(f"cat {AGENTS}/{SESSION}.json"))
        agent = record["terminal"]
        assert agent, f"the hook did not say which terminal the agent runs in: {record}"
        assert record["account-dir"] == "/root/.claude-second", record
        until(lambda: viewer(agent) == main_pid("agent"), "the agent's terminal is not the one its window shows")

        until(
            lambda: WORKING in account_sets().get(2, ""),
            lambda: f"no working agent in the second account's set: {account_sets()}",
        )
        first_set = account_sets().get(1, "")
        assert WORKING not in first_set and WAITING not in first_set, f"the agent lit the FIRST account's set: {first_set!r}"

        machine.succeed(f"touch {ROOM}/block")
        machine.wait_until_succeeds(f"test -e {ROOM}/blocked", timeout=timedelta(seconds=30))
        until(
            lambda: WAITING in account_sets().get(2, "") and WORKING not in account_sets().get(2, ""),
            lambda: f"the agent blocked over OSC 7501, and its set does not say it waits: {account_sets()}",
        )
        record = json.loads(machine.succeed(f"cat {AGENTS}/{SESSION}.json"))
        assert record["status"] == "working", (
            f"the session's file says {record['status']!r}, so the hook — not the terminal — said it waits"
        )

    with subtest("terminal-show and the agent's click bring the agent's window forward"):
        # PROVES the way to an agent: with another window holding the
        # keyboard, `terminal-show <its terminal>` focuses the window that
        # shows it — and so does `agents-show <session>`, what a click on
        # its glyph runs. The other window is the group's, opened by the
        # same `terminal-show` while the group had none: one show opens,
        # the other focuses. The compositor says where the keyboard is.
        agent_window = viewer(agent)
        assert agent_window is not None
        control(f"terminal-show {first}")
        until(
            lambda: viewer(first) is not None and viewer(first) == viewer(second) and window_of(viewer(first)) is not None,
            "terminal-show of a background terminal opened no window for its group",
        )
        others = viewer(first)
        focus(others)
        control(f"terminal-show {agent}")
        until(
            lambda: focused_pid() == agent_window,
            lambda: f"terminal-show left the keyboard in {focused_pid()}, not in the agent's {agent_window}",
        )
        focus(others)
        control(f"agents-show {SESSION}")
        until(
            lambda: focused_pid() == agent_window,
            lambda: f"the agent's click left the keyboard in {focused_pid()}, not in the agent's {agent_window}",
        )
  '';
}
