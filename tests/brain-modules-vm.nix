# THE BRAIN'S MODULES ON A BOOTED MACHINE, each proved by its own subtest.
#
# WHY ONE MACHINE AND NOT ONE PER MODULE
# --------------------------------------
# A VM costs its boot and the memory it declares, and the brain is one daemon:
# its modules share the watch thread, the journal feed, the socket and the
# clock. One machine, one brain, and every module that brings a subtest runs it
# there — which is also the only place two modules' watches are seen running
# side by side.
#
# WHERE THE SUBTESTS LIVE
# -----------------------
# In each module's folder, as `vm.nix`, and nowhere else: this file names no
# module (`brain-modules-are-sealed`). It reads the loader's list of folders
# (`nix/brain-modules.nix`) and takes every `vm.nix` it finds.
# Its subtests carry the module's id in their names.
#
# THE BRAIN IS QUIET (`{"quiet": true}`): nothing it schedules starts by
# itself; what a watch hears is still heard.
#
# WHAT A SUBTEST HAS
# ------------------
# The compositor's helpers (`compositor-xvfb.nix`), and these:
#
#   control(command)       `ronoctua-shell command <command>`, as a key would
#   footer_journal()       everything the footer (and its brain) said
#   last_layout()          the footer's last `footer layout resolved` and after
#   settled(test, timeout) waits until `test(last_layout())` holds
#   run_dir                the runtime directory the brain's socket is in
#                          (the footer runs as root here: `/run/user/0`, and
#                          its home is `/root`)
#   brain(ask)             one line to the brain's socket, answered as JSON
#   mind()                 what `ronoctua-brain look` prints, as JSON
#
# A module's `vm.nix` answers `{ node = { … }; script = ''…''; }` (both
# optional but the script), given `{ pkgs, lib, ronoctua-shell, ronoctua }`.
#
# THE BRAIN IS THE FOOTER'S CHILD here, as on a session without its unit: it
# has the footer's environment, and journalctl by its store path, as the
# brain's unit hands it.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
  ronoctua,
  brainModules,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };

  # ONE LINE TO THE BRAIN'S SOCKET, and its one-line answer — what the footer
  # and the buffer do, with nothing of theirs in between.
  ask = pkgs.writeText "brain-ask.py" ''
    import socket
    import sys

    with socket.socket(socket.AF_UNIX) as s:
        s.connect(sys.argv[1])
        s.sendall(sys.argv[2].encode() + b"\n")
        print(s.makefile().readline(), end="")
  '';

  # EVERY MODULE'S SUBTEST, in the folders' order.
  parts = lib.concatMap (
    m:
    let
      file = m.dir + "/vm.nix";
    in
    lib.optional (builtins.pathExists file) (
      {
        node = { };
      }
      // import file {
        inherit
          pkgs
          lib
          ronoctua-shell
          ronoctua
          ;
      }
    )
  ) brainModules.every;
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-brain-modules";

  nodes.machine = {
    imports = [ ../nix/modules/base.nix ] ++ map (p: p.node) parts;
    _module.args.ronoctua = ronoctua;

    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;
    networking.hostName = lib.mkForce "noctua-brain";
    users.users.root.hashedPassword = lib.mkForce null;

    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.util-linux # logger
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
    import shlex

    run_dir = "/run/user/0"


    def brain(ask):
        """One line to the brain's socket, and its one-line answer as JSON."""
        return json.loads(machine.succeed(
            f"${pkgs.python3}/bin/python3 ${ask} {run_dir}/ronoctua-brain.sock "
            + shlex.quote(json.dumps(ask))
        ))


    def mind():
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR={run_dir} ${ronoctua.brain}/bin/ronoctua-brain look"
        ))


    def control(command):
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"${ronoctua-shell}/bin/ronoctua-shell command {command}"
        )


    def footer_journal():
        return machine.succeed("journalctl -u footer --no-pager")


    def last_layout():
        j = footer_journal()
        i = j.rfind("footer layout resolved")
        return j[i:] if i >= 0 else ""


    def settled(test, timeout=60):
        """Waits until the footer's last layout passes `test`; says what it was when it does not."""
        for _ in range(timeout):
            if test(last_layout()):
                return
            machine.sleep(1)
        raise AssertionError("the footer never settled; its last layout:\n" + last_layout()[-3000:])


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")

    with subtest("the footer comes up, and its brain follows the journal"):
        # QUIET: nothing the brain schedules starts by itself; what a watch
        # hears is still heard, and a reminder's clock still rings.
        machine.succeed(
            "install -d /root/.config/ronoctua"
            " && echo '{\"quiet\": true}' > /root/.config/ronoctua/brain.json"
        )
        # THE BRAIN IS THE FOOTER'S CHILD here (no unit in this VM), so it has
        # the footer's environment — and journalctl by its store path, as the
        # brain's unit hands it (`nix/modules/brain.nix`).
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=HOME=/root "
            "--setenv=RONOCTUA_JOURNALCTL=${pkgs.systemd}/bin/journalctl "
            f"--setenv=SCROLLSOCK={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_brain=info,ronoctua_notify=info "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'footer layout resolved' > /dev/null", timeout=90
        )
        # A JOURNAL THE BRAIN COULD NOT READ is said in its log, and every
        # subtest below would then wait for nothing.
        machine.sleep(5)
        assert "the journal is not being followed" not in footer_journal(), footer_journal()[-3000:]
        machine.wait_until_succeeds(f"test -S {run_dir}/ronoctua-brain.sock", timeout=60)
        rows = {t["id"] for t in mind()["tasks"]}
        for wanted in ${
          builtins.toJSON (
            map (m: m.id) (lib.filter (m: builtins.pathExists (m.dir + "/vm.nix")) brainModules.every)
          )
        }:
            assert wanted in rows, f"the brain does not list {wanted}: {sorted(rows)}"

    ${lib.concatMapStrings (p: p.script + "\n") parts}
  '';
}
