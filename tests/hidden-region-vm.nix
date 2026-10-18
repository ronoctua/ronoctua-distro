# VM test: a region of the footer that lives HIDDEN, end to end.
#
# A leader command that acts for a view without its page — a check of the
# security centre's, a run of the update by hand — brings the view's region to
# life hidden (`Closed::Behind`): unranked and undrawn, its threads heard, its
# question answered, and let go once it waits for nothing (`Region::lingers`).
# The unit tests prove each part (`region.rs`, `security_region.rs`,
# `update_region.rs`); only a running footer proves the parts meet — the
# region's thread waking the footer's loop through the region's own channel,
# what the thread brought said on the minibuffer, the page asked for and
# given when the region is free, the region dropped when it is done.
#
# NOTHING HERE NEEDS ROOT OR A NETWORK. The check of an address asks a host
# the VM cannot reach, and the refusal is what comes back and is drawn on the
# page — which is exactly the path a real answer takes. The update by hand
# finds no brain and no unit, says so, and has nothing to follow.
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
  name = "ronoctua-hidden-region";

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

    import re


    def journal():
        return machine.succeed("journalctl -u footer --no-pager")


    def command(*words):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell command " + " ".join(words)
        )


    def wait_for(pattern, timeout=60):
        # A line of the footer's journal matching `pattern` — and the journal
        # printed when it never comes, so a failure says what happened instead.
        try:
            machine.wait_until_succeeds(
                f"journalctl -u footer --no-pager | grep -E '{pattern}' > /dev/null",
                timeout=timeout,
            )
        except Exception:
            print(journal()[-4000:])
            raise


    def count(pattern):
        return len(re.findall(pattern, journal()))


    def bound(sequence):
        # THE COMMAND A SEQUENCE OF THE WHOLE TREE RUNS, as the keymap's dump
        # says it — so this test spells no brain module's id (the update's is
        # its module's, `brain-modules-are-sealed`).
        dump = machine.succeed("${ronoctua-shell}/bin/ronoctua-shell check-keymap --dump")
        for line in dump.splitlines():
            cols = line.split("\t")
            if len(cols) > 3 and cols[0] == "full" and cols[1] == sequence and cols[2] == "command":
                return cols[3]
        raise Exception(f"no command at `{sequence}` in the keymap")


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("the footer comes up at rest"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=HOME=/root "
            "--setenv=LANG=en_GB.UTF-8 "
            f"--setenv=SCROLLSOCK={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        wait_for("footer layout resolved", timeout=90)
        wait_for("control socket opened", timeout=30)

    with subtest("a check from the leader lives hidden, and its answer is seen"):
        command("security-check-email")
        wait_for('a region lives hidden.*region=security')
        # ITS QUESTION IS ON THE LINE, and nothing ranks the page yet: the
        # region that holds the footer's lines is left where it is.
        assert count('a hidden region shown.*region=security') == 0, journal()[-3000:]
        echoes = count("echo area")
        # THE ANSWER, under the question's id — the minibuffer's own path
        # (`run_with` with the typed text), which goes to the region that
        # asked it before any view is asked.
        command("security-check-email", "nobody@example.invalid")
        # "checking" said, from the hidden region, on the footer's line.
        wait_for("echo area")
        assert count("echo area") > echoes, journal()[-3000:]
        # AND THE CHECK'S THREAD came back — a refusal, the VM reaches no
        # host — through the region's own channel, and what came is drawn on
        # the page, which the hidden region asked for and was given.
        wait_for('a hidden region shown.*region=security', timeout=120)

    with subtest("a run by hand with nothing to follow is said, and let go"):
        echoes = count("echo area")
        command(bound("s u n"))
        wait_for('a region lives hidden.*region=update')
        # NO BRAIN TOOK IT, so there is no run to follow: the sentence of the
        # door it went through is said, and the region is let go at once.
        wait_for('a hidden region let go.*region=update')
        assert count("echo area") > echoes, journal()[-3000:]
        assert count('a hidden region shown.*region=update') == 0, journal()[-3000:]

    with subtest("nothing went to a command with no action"):
        assert "a command with no registered action" not in journal(), journal()[-3000:]
  '';
}
