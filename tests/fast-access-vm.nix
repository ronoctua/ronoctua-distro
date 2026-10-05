# The search over everything (fast-access) on a BOOTED machine with a real
# compositor: the index its own unit writes, read by the footer's view and by
# the windows; the leader's sequence and the chord that open it; the two
# windows' identities and how the generated rules place them; the window kept
# ready after it closes.
#
# WHAT ONLY THE VM PROVES
# -----------------------
#   1. THE UNIT INDEXES A PLANTED HOME, AND THE DOORS READ WHAT IT WROTE:
#      `ronoctua-index.service` — the unit `nix/modules/index.nix` declares,
#      started in a person's own user manager as the brain starts it — walks
#      their home and commits to their cache folder; the footer's view and
#      the window, pointed at that cache, find a planted file by a word of its
#      name and another by a word written inside it, read back off the screen.
#   2. `LEADER d` OPENS THE FOOTER'S VIEW: the leader's sequence as the
#      compositor's bind runs it (`ronoctua-shell leader`), then `d` — the
#      group folded into one key — and the view is up and takes the keys.
#   3. `Super+d` OPENS IT TOO: the chord the shell's own table generates
#      (`nix/compositor-binds.nix`), loaded into a real compositor — never a
#      bind written here.
#   4. `LEADER SPC` OPENS A FLOATING WINDOW over the whole room,
#      `br.ronoctua.fast-access-float`, by the rules the shell generates
#      (`crate::floating`); and `fast-access` alone opens
#      `br.ronoctua.fast-access`, TILED.
#   5. `Esc` CLOSES every door: the view, and a window with its process.
#   6. A WINDOW KEPT READY (`park` in `fast-access.json`) does not end: its
#      role is given up — it leaves the compositor's tree — the process stays,
#      and it listens on its socket; the next `ronoctua-shell fast-access`
#      hands it the words and ends, and the SAME process comes back as a
#      window with them typed.
#
# WHAT IS NOT HERE: the network. Every scope this test types answers on the
# machine (names, contents, sums); the web, YouTube and the words are each
# behind a host the person lets, and their requests are the unit tests'
# (`online-stops-on-the-next-key`).
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  index,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };

  # THE GENERATED CHORDS AND WINDOW RULES on top of the minimal test config,
  # as `nix/modules/scroll.nix` splices them: what is proved is what the
  # shell EMITS.
  windowRules = pkgs.callPackage ../nix/compositor-window-rules.nix { inherit ronoctua-shell; };
  chords =
    (pkgs.callPackage ../nix/compositor-binds.nix { inherit ronoctua-shell; }).forPrefix
      "Super";
  configSession = pkgs.runCommand "scroll-test-fast-access.conf" { } ''
    cat ${xvfb.configCompositor} ${chords} ${windowRules} > $out
    grep -q 'command fast-access-open' $out || {
      echo "ERROR: the chord of fast-access did not come out of the shell's table." >&2; exit 1; }
  '';

  # THE PERSON whose home the unit walks, in their own user manager.
  person = "ana";
  home = "/home/${person}";

  # NO PACKAGES TO INSTALL: the unit relists them from this file, and an
  # empty one is a list with nothing in it — the walk of the home is what is
  # proved here.
  softwareIndex = pkgs.writeTextDir "index.tsv" "";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-fast-access";

  nodes.machine = {
    imports = [ (import ../nix/modules/index.nix { inherit index; }) ];
    _module.args.ronoctua = { inherit softwareIndex; };

    virtualisation.memorySize = 3072;
    virtualisation.cores = 2;

    # THE PERSON, with a user manager that runs without a login: the brain
    # starts the unit there, and so does this test.
    users.users.${person} = {
      isNormalUser = true;
      uid = 1000;
      linger = true;
    };

    environment.systemPackages = [
      scroll
      ronoctua-shell
      index
      pkgs.imagemagick
      pkgs.tesseract
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

    HOME = "${home}"
    PLANTED = f"{HOME}/planted"
    # WHAT THE UNIT WRITES: `%C/ronoctua/index` in the person's manager.
    CACHE = f"{HOME}/.cache"
    INDEX = f"{CACHE}/ronoctua/index"
    SOCKET = "/run/user/0/ronoctua-fast-access.sock"
    FLOAT = "br.ronoctua.fast-access-float"
    TILED = "br.ronoctua.fast-access"
    # EVERY DOOR READS THE INDEX THE UNIT WROTE: the person's cache.
    READS = f"--setenv=XDG_CACHE_HOME={CACHE} "


    def door(unit, *words):
        """A window of the search, as a unit of its own."""
        quoted = " ".join("'" + w + "'" for w in words)
        machine.succeed(
            f"systemd-run --unit={unit} ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=HOME=/root "
            + READS
            + "--setenv=XDG_CONFIG_HOME=/root/.config "
            "--setenv=LANG=en_GB.UTF-8 "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            f"${ronoctua-shell}/bin/ronoctua-shell fast-access {quoted}"
        )


    def ocr(w, h, x, y):
        """What a box of the screen says."""
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/box.png"
        )
        return machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/box.png stdout --psm 6 2>/dev/null"
        ).strip().lower()


    def read_window(sock, app_id):
        return ocr(*window_box(sock, app_id))


    def read_footer():
        """The lower half of the screen, where the footer's region grows."""
        w, h, x, y = compositor_geometry()
        return ocr(w, h // 2, x, y + h - h // 2)


    def wait_for_window(sock, app_id, unit):
        try:
            machine.wait_until_succeeds(
                f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree | grep -F '\"{app_id}\"' > /dev/null",
                timeout=90,
            )
        except Exception:
            print(machine.execute(f"journalctl -u {unit} --no-pager")[1])
            raise


    def wait_for_words(read, words, unit):
        """The OCR, polled until every word is read — the answers arrive after
        the first frame (the index answers on a thread)."""
        said = ""
        for _ in range(30):
            said = read()
            if all(w in said for w in words):
                return said
            machine.sleep(1)
        print(machine.execute(f"journalctl -u {unit} --no-pager")[1])
        raise AssertionError(f"the screen never said {words}; OCR read {said!r}")


    def said_times(unit, text):
        """How many times a unit's journal said `text`."""
        return int(
            machine.succeed(
                f"journalctl -u {unit} --no-pager | grep -cF '{text}' || true"
            ).strip() or "0"
        )


    def wait_said(unit, text, times):
        machine.wait_until_succeeds(
            f"test $(journalctl -u {unit} --no-pager | grep -cF '{text}') -ge {times}",
            timeout=60,
        )


    def leader():
        """The leader, as the compositor's bind runs it."""
        before = said_times("footer", "leader active")
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        wait_said("footer", "leader active", before + 1)


    def key(*keys):
        machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key " + " ".join(keys))
        machine.sleep(1)


    def type_words(words):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool type --delay 80 '{words}'")


    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the unit indexes a planted home, in the person's own manager"):
        machine.succeed(
            f"install -d -o ${person} -g users {PLANTED} {PLANTED}/papers && "
            f"printf 'quarterly figures\\n' > {PLANTED}/papers/harvest-report.txt && "
            f"printf 'the lighthouse keeper wrote this\\n' > {PLANTED}/papers/notes.txt && "
            f"chown -R ${person}:users {PLANTED}"
        )
        machine.wait_for_unit("user@1000.service")
        machine.succeed("systemctl --user -M ${person}@ start ronoctua-index.service")
        try:
            machine.wait_until_succeeds(
                f"${index}/bin/ronoctua-index query --dir {INDEX} harvest | grep -F harvest-report.txt",
                timeout=180,
            )
        except Exception:
            print(machine.execute("journalctl _SYSTEMD_USER_UNIT=ronoctua-index.service --no-pager")[1])
            raise
        # AND IT SAYS WHERE IT IS, for the brain's mark and page.
        machine.succeed(f"test -s {INDEX}/status.json")
        found = machine.succeed(f"${index}/bin/ronoctua-index query --dir {INDEX} lighthouse")
        assert "notes.txt" in found, found

    start_xvfb()
    start_compositor("compositor", "${scroll}", config="${configSession}")
    sock = socket_of("compositor")

    with subtest("the footer comes up"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=HOME=/root "
            + READS
            + "--setenv=XDG_CONFIG_HOME=/root/.config "
            "--setenv=LANG=en_GB.UTF-8 "
            f"--setenv=SCROLLSOCK={sock} "
            "--setenv=RONOCTUA_LEADER=LEADER "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'control socket opened' > /dev/null",
            timeout=90,
        )
        focus_compositor()
        # THE POINTER OUT OF THE REGION'S WAY: the line under it would explain
        # what it rests on instead of what the test reads.
        w0, h0, x0, y0 = compositor_geometry()
        machine.succeed(
            f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove {x0 + w0 - 5} {y0 + 5}"
        )

    with subtest("LEADER d opens the footer's view, which finds a planted file"):
        leader()
        key("d")
        wait_said("footer", "fast-access opened in the region", 1)
        type_words("harvest")
        said = wait_for_words(read_footer, ["harvest", "report"], "footer")
        print(f"OCR: {said!r}")
        key("Escape")
        wait_said("footer", "fast-access closed in the region", 1)

    with subtest("Super+d opens it too, through the generated chord"):
        focus_compositor()
        key("super+d")
        try:
            wait_said("footer", "fast-access opened in the region", 2)
        except Exception:
            print(machine.execute("journalctl -u compositor --no-pager | tail -40")[1])
            raise
        key("Escape")
        wait_said("footer", "fast-access closed in the region", 2)

    with subtest("LEADER SPC opens the search floating over the whole room"):
        leader()
        key("space")
        wait_for_window(sock, FLOAT, "footer")
        open_now = windows_by_app_id(sock)
        print(json.dumps(open_now, indent=1))
        floated = open_now[FLOAT]
        assert floated["floating"], "the generated rule did not make the search float"
        ow, oh, _, _ = compositor_geometry()
        w, h = floated["rect"]["width"], floated["rect"]["height"]
        # THE WHOLE ROOM, give or take the footer and the borders the
        # compositor keeps: never a corner.
        assert w >= ow * 0.8 and h >= oh * 0.6, (w, h, ow, oh)
        type_words("2+2")
        said = wait_for_words(lambda: read_window(sock, FLOAT), ["4"], "footer")
        print(f"OCR: {said!r}")
        pid = floated["pid"]
        key("Escape")
        machine.wait_until_fails(f"kill -0 {pid}", timeout=30)

    with subtest("the window opens tiled, and finds a name and what a file says"):
        door("search", "harvest")
        wait_for_window(sock, TILED, "search")
        open_now = windows_by_app_id(sock)
        print(json.dumps(open_now, indent=1))
        assert not open_now[TILED]["floating"], (
            "the plain search window floats: the float's rule caught it too"
        )
        said = wait_for_words(lambda: read_window(sock, TILED), ["harvest", "report"], "search")
        print(f"OCR: {said!r}")
        # AND WHAT A FILE SAYS: a word that is in no name.
        key("ctrl+u")
        type_words("lighthouse")
        wait_for_words(lambda: read_window(sock, TILED), ["notes"], "search")

    with subtest("Esc closes the window, and its process with it"):
        pid = windows_by_app_id(sock)[TILED]["pid"]
        key("Escape")
        machine.wait_until_fails(f"kill -0 {pid}", timeout=30)

    with subtest("a window kept ready comes back, the same process"):
        machine.succeed(
            "mkdir -p /root/.config/ronoctua && "
            "printf '{\"park\": true}' > /root/.config/ronoctua/fast-access.json"
        )
        door("search-kept", "harvest")
        wait_for_window(sock, TILED, "search-kept")
        pid = windows_by_app_id(sock)[TILED]["pid"]
        key("Escape")
        # PUT AWAY: out of the tree, alive, and listening.
        machine.wait_until_fails(
            f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree | grep -F '\"{TILED}\"'",
            timeout=30,
        )
        machine.succeed(f"kill -0 {pid}")
        machine.wait_until_succeeds(f"test -S {SOCKET}", timeout=30)
        # THE NEXT DOOR HANDS IT THE WORDS AND ENDS.
        machine.succeed(
            "HOME=/root XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "timeout 20 ${ronoctua-shell}/bin/ronoctua-shell fast-access lighthouse"
        )
        wait_for_window(sock, TILED, "search-kept")
        back = windows_by_app_id(sock)[TILED]["pid"]
        assert back == pid, f"another process came up ({back}), not the kept one ({pid})"
        wait_for_words(lambda: read_window(sock, TILED), ["notes"], "search-kept")
        # AND THE SOCKET IS GONE while the window is up: it listens only
        # while it is put away.
        machine.fail(f"test -S {SOCKET}")
  '';
}
