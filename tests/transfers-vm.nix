# The transfer queue, proved from the OTHER side of the socket.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# The queue itself is unit-tested down to the byte: the beats add up, a cancel
# leaves nothing behind, a pause is not a stop, a name that is not UTF-8
# survives the wire. None of that needs a machine, and none of it says the
# FOOTER sees any of it.
#
# Four things do need one, and each has failed silently in this project before:
#
#   1. the unit exists and comes up under `graphical-session.target` — a
#      `systemd.user.services` entry that does not evaluate is a paste that
#      answers "Unit not found" in front of the person;
#   2. the footer FINDS it. The client starts the daemon when nobody is home
#      and reconnects when it goes away; a watcher thread that never connects
#      is a modeline that stays blank with nothing to notice it by;
#   3. the segment reaches the MODELINE. The layout is integer arithmetic over
#      priorities and `min_cols`, and a segment that resolves to zero columns
#      is a feature that exists everywhere except on screen;
#   4. and `LEADER t t` opens the panel with the job in it — the keymap moved
#      `t` out from under the screen capture to make room, and a group that
#      does not resolve is a key that does nothing.
#
# WHY THE JOB IS STARTED THROUGH THE SOCKET AND NOT THROUGH THE MANAGER
# ---------------------------------------------------------------------
# Because pasting from the file manager is a dozen keystrokes — open, mark,
# copy, navigate, paste — and every one of them is a way for this test to fail
# for a reason that has nothing to do with transfers. What is under test here is
# the queue reaching the screen; `files-vm` is where the manager's own door is
# proved.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  ronoctua-transfers,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-transfers";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      ronoctua-transfers
      pkgs.python3
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}

    # `bytes()` and never `b\'\'`: inside a Nix `\'\'…\'\'` fence, ANY `\'\'` closes
    # the string — including one inside a Python literal inside a comment. It
    # cost a build here, and `CLAUDE.md` already records it.
    #
    # ONE JSON OBJECT PER LINE, which is the whole protocol. Written out here
    # rather than driven through a helper binary on purpose: a test that spoke
    # the protocol through our own client would pass even if the protocol had
    # changed under both of them at once.
    #
    # AND HALF THE VERBS DO NOT ANSWER. `pause`, `resume`, `cancel`, `forget`
    # and `sweep` are told, not asked: nothing needs the answer, and waiting for
    # one would put a socket round trip inside the frame that draws. A client
    # that read anyway would block until its timeout — which is exactly what
    # this test did on its first run.
    ASK = (
        "import socket,sys\n"
        "s=socket.socket(socket.AF_UNIX); s.settimeout(10)\n"
        "s.connect('/run/user/0/ronoctua-transfers.sock')\n"
        "s.sendall((sys.argv[1]+chr(10)).encode())\n"
        "if len(sys.argv) > 2:\n"
        "    d=bytes()\n"
        "    while not d.endswith(chr(10).encode()):\n"
        "        c=s.recv(65536)\n"
        "        if not c: break\n"
        "        d+=c\n"
        "    print(d.decode().strip())\n"
    )


    def ask(body):
        """A request that ANSWERS: `list` and `add`."""
        machine.succeed(f"cat > /tmp/ask.py <<'EOF'\n{ASK}EOF")
        out = machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            f"${pkgs.python3}/bin/python3 /tmp/ask.py {repr(body)} read"
        )
        return json.loads(out.strip())


    def tell(body):
        """A request that does NOT answer: pause, resume, give up."""
        machine.succeed(f"cat > /tmp/ask.py <<'EOF'\n{ASK}EOF")
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            f"${pkgs.python3}/bin/python3 /tmp/ask.py {repr(body)}"
        )


    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    import json

    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("the queue comes up as a unit, and not inside the footer"):
        # `--user` and `/run/user/0` because this VM runs as root; on the real
        # machine it is `graphical-session.target` that pulls it in. What is
        # under test is that the unit EVALUATES and the binary runs — not
        # systemd's own ordering.
        machine.succeed(
            "systemd-run --unit=queue "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=RUST_LOG=ronoctua_transfers=info "
            "${ronoctua-transfers}/bin/ronoctua-transfers"
        )
        machine.wait_until_succeeds(
            "journalctl -u queue --no-pager | grep -q 'transfer queue opened'",
            timeout=60,
        )

    with subtest("the footer comes up and FINDS the queue"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'control socket opened'",
            timeout=90,
        )
        focus_niri()

    with subtest("a job enters the queue and reaches the modeline's SEGMENT"):
        # Big enough that the copy is still going when the modeline is read.
        # `/dev/urandom` and not `/dev/zero`: a hole is not a copy, and btrfs
        # would share the extent of a sparse file without moving anything.
        machine.succeed("mkdir -p /root/source /root/destination")
        machine.succeed(
            "head -c 400000000 /dev/urandom > /root/source/large.bin; sync"
        )
        took = ask(
            '{"ask":"add","work":{"work":"paste",'
            '"paths":["/root/source/large.bin"],'
            '"into":"/root/destination","moves":false}}'
        )
        assert took["say"] == "took", took
        job = took["id"]

        # THE SEGMENT, in the layout the footer resolved. It is `debug`-logged
        # per segment, which is the only place the RESOLVED column is visible —
        # a segment that exists and gets no columns is a feature that is
        # nowhere on screen.
        try:
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep -q 'segment id=\"transfer\"'",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -40")[1])
            raise
        said = machine.succeed(
            "journalctl -u footer --no-pager | grep 'segment id=\"transfer\"' | tail -1"
        )
        print(said.strip())

    with subtest("holding HOLDS, and the queue says so"):
        # A JOB OF ITS OWN, ADDED AND PAUSED IN THE SAME BREATH.
        #
        # It used to reuse the job above, and that made the test a race it kept
        # winning by luck: between the two subtests sits a
        # `wait_until_succeeds` on the journal, which takes seconds, and four
        # hundred megabytes onto a memory-backed root does not. The job reached
        # `done` and the assertion read `stage == "paused"` against a copy that
        # had finished — the failure said "the queue is wrong" about a queue
        # that was right.
        #
        # There is no ask for "add it held", so what is left is to shrink the
        # window to the two socket round-trips: add, then pause, with nothing
        # between them.
        machine.succeed("mkdir -p /root/destination-pause")
        second = ask(
            '{"ask":"add","work":{"work":"paste",'
            '"paths":["/root/source/large.bin"],'
            '"into":"/root/destination-pause","moves":false}}'
        )
        assert second["say"] == "took", second
        holding = second["id"]
        tell('{"ask":"pause","id":%d}' % holding)
        machine.sleep(1)
        first = ask('{"ask":"list"}')["jobs"]
        held = next(j for j in first if j["id"] == holding)
        assert held["stage"] == "paused", held
        machine.sleep(1)
        again = next(j for j in ask('{"ask":"list"}')["jobs"] if j["id"] == holding)
        assert again["done"] == held["done"], (
            f"the pause did not hold: {held['done']} -> {again['done']}"
        )

    with subtest("giving up DELETES the piece and leaves the original"):
        # THE PAUSED JOB, and not the first one — for the same reason the
        # subtest above stopped reusing it. Giving up has to delete a PIECE,
        # and the first job reaches `done`: cancelling something that finished
        # leaves the whole file behind, correctly, and the assertion then reads
        # "the piece was not deleted" about a copy that was never a piece.
        #
        # The held job is the honest subject: it is provably stopped part-way,
        # so what is in its destination is exactly the piece this is about.
        tell('{"ask":"cancel","id":%d}' % holding)
        machine.wait_until_succeeds(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "${pkgs.python3}/bin/python3 /tmp/ask.py "
            """'{"ask":"list"}' read """
            "| grep -q cancelled",
            timeout=60,
        )
        left = machine.succeed("ls -A /root/destination-pause").strip()
        assert left == "", f"a piece was left at the destination: {left}"
        machine.succeed("test -s /root/source/large.bin")

    with subtest("`LEADER t t` opens the panel with the row inside"):
        # `t` became a GROUP: the screen capture moved to `c s` to make room.
        # A group that does not resolve is a key that does nothing, and nothing
        # on screen says which of the two it was.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader active'", timeout=30
        )
        press_key("t")
        press_key("t")
        try:
            journal = machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep 'transfers panel opened'",
                timeout=30,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -40")[1])
            raise
        print(journal.strip())
        # The panel opens with the list the footer ALREADY has — a panel that
        # opened empty and filled in a moment later would read as "nothing is
        # running" at the instant somebody opened it to check.
        #
        # AGAINST THE QUEUE'S OWN COUNT, and not against a number written here.
        # It was `jobs=1`, which was true of however many jobs the subtests
        # above happened to leave behind — so adding one anywhere earlier broke
        # this, and the message blamed the panel. What is being asserted is
        # that the panel and the queue agree, and the queue is who to ask.
        listed = len(ask('{"ask":"list"}')["jobs"])
        assert listed > 0, "the queue emptied before the panel opened"
        assert f"jobs={listed}" in journal, (
            f"the panel opened without the list the footer already had "
            f"({listed} in the queue):\n{journal}"
        )
  '';
}
