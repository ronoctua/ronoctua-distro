# The emacs-daemon module's subtest in `tests/brain-modules-vm.nix`: the watch
# reads the editor the distro installs — its daemon, a client of it and a
# second copy of the editor — off the real process table, and says what each
# makes of the machine. The helpers (`brain`, `mind`, …) are that file's.
#
# WHAT ONLY THE VM PROVES: that the processes of the package
# `nix/modules/emacs.nix` declares are the shapes `ronoctua-memory` tells
# apart. A build that wraps its programs (the pure-GTK one does, for GTK's
# modules) runs a hidden binary under the name it was called by, and the
# watch reads that name: a wrapper that renamed it would leave the watch
# seeing no Emacs at all — and a watch that sees none says nothing, which
# looks exactly like everything being well.
#
# THE CLIENTS ARE TERMINAL ONES: the brain here is root's and the compositor
# root's, and the person's processes are told apart by their arguments, not
# by where they draw (`tests/emacs-vm.nix` proves the frames).
{ pkgs, ... }:

let
  # A terminal for a terminal Emacs: a unit has none. `pty.spawn` stops
  # reading the unit's `/dev/null` at its end instead of passing the end on.
  pty = pkgs.writeText "emacs-daemon-vm-pty.py" ''
    import os
    import pty
    import sys

    status = pty.spawn(sys.argv[1:])
    sys.exit(os.waitstatus_to_exitcode(status))
  '';
in
{
  # THE EDITOR AS THE DISTRO DECLARES IT, its daemon in the person's manager,
  # which is up at boot without a login.
  node = {
    imports = [ ../../../../nix/modules/emacs.nix ];
    users.users.noctua.linger = true;
  };

  script = ''
    with subtest("emacs-daemon: the watch tells the daemon, its client and a second Emacs apart"):
        import re

        person = "noctua"
        # THE PROGRAMS THE MACHINE HAS, whichever package the editor declares.
        bin = "/run/current-system/sw/bin"

        def as_person(unit, command):
            machine.succeed(
                f"systemd-run --user --machine={person}@ --unit={unit} --quiet "
                f"--setenv=TERM=xterm-256color -- ${pkgs.python3}/bin/python3 ${pty} {command}"
            )

        def terminals():
            uid = machine.succeed(f"id -u {person}").strip()
            said = machine.succeed(
                f"runuser -u {person} -- env XDG_RUNTIME_DIR=/run/user/{uid} {bin}/emacsclient --eval "
                "'(mapcar (lambda (f) (terminal-name (frame-terminal f))) (frame-list))'"
            )
            return re.findall(r"\"([^\"]*)\"", said)

        def says():
            """Asks the watch to look now, waits for that look, and returns
            the notices it left."""
            asked = mind()["now"]
            machine.sleep(1)
            said = brain({"ask": "run", "task": "emacs-daemon", "rehearsal": False})
            assert said.get("say") not in ("refused", "declined"), f"the run was refused: {said}"

            def looked(last_try):
                rows = [t for t in mind()["tasks"] if t["id"] == "emacs-daemon"]
                last = (rows[0].get("last") or {}) if rows else {}
                if last_try:
                    print("the watch's row:", rows)
                return last.get("began", 0) > asked and last.get("kind") == "done"

            retry(looked, timeout_seconds=180)
            return sorted(n["what"]["id"] for n in mind()["notices"] if n["module"] == "emacs-daemon")

        machine.wait_for_unit("user@1000.service")
        machine.wait_until_succeeds(
            f"systemctl --user --machine={person}@ is-active emacs.service", timeout=120
        )

        # THE DAEMON AND ITS CLIENT: all in hand, nothing to say.
        as_person("emacs-daemon-vm-client", f"{bin}/emacsclient -t /home/{person}/notes.txt")
        retry(lambda last_try: any(t.startswith("/dev/pts/") for t in terminals()), timeout_seconds=60)
        assert says() == [], "the daemon and its own client made the watch speak"

        # A SECOND EMACS, which loaded a configuration of its own: the daemon
        # is seen up, and the other copy for what it is.
        as_person("emacs-daemon-vm-standalone", f"{bin}/emacs -nw /home/{person}/other.txt")
        machine.sleep(5)
        assert says() == ["not-the-client"], "a second Emacs beside the daemon was not told"

        # THE DAEMON GONE, the second Emacs still open: the line with the button.
        machine.succeed(f"systemctl --user --machine={person}@ stop emacs.service")
        assert says() == ["no-daemon"], "a stopped daemon was not told"

        # NO EMACS AT ALL is no problem, and the machine is left as it was.
        machine.succeed(f"systemctl --user --machine={person}@ stop emacs-daemon-vm-standalone")
        machine.sleep(2)
        assert says() == [], "a machine with no Emacs made the watch speak"
  '';
}
