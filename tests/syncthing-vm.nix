# VM test: a real Syncthing, and the view that reads it.
#
# WHAT THIS PROVES THAT THE UNIT TESTS DO NOT
# -------------------------------------------
# The unit tests are pure by construction — `rest.rs` and `unit.rs` are the
# only modules that touch anything, and neither of them decides. That is what
# makes a build sandbox able to hold every rule down, and it is also the limit:
# nothing in the sandbox has ever spoken to Syncthing.
#
# Here it does. A real `syncthing.service`, started by our own module and tuned
# by our own unit, answering our own crate over the loopback — and then the
# view drawing rows off what came back.
#
# It is also the only place the POLKIT RULE is exercised end to end.
# `syncthing-polkit-is-narrow` reads the rule's text; this stops the unit
# through it, as the owner, with no agent in the session and no `sudo`.
#
# GPU: `lavapipe`, for the reason `tests/footer-vm.nix` gives — and the same
# consequence: no performance number taken here is worth anything.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-syncthing";

  nodes.machine = {
    imports = [
      ../nix/modules/syncthing.nix
    ];

    # A TOP-LEVEL `options` MAKES EVERY SIBLING A `config` ONE, and NixOS says
    # so with a message that names the sibling rather than the cause. So the
    # whole node goes inside `config`.
    options.ronoctua.user = lib.mkOption { type = lib.types.str; };
    config = {
      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;

      # THE MODULE READS `config.ronoctua.user`, which belongs to base — and
      # base brings the whole system with it. Declaring the one option it needs
      # is what `syncthing-tuned` does for the same reason.
      ronoctua.user = "noctua";

      users.users.noctua = {
        isNormalUser = true;
        group = "users";
        uid = 1000;
      };

      environment.systemPackages = [
        niri
        ronoctua-shell
        pkgs.curl
      ]
      ++ xvfb.packages;

      fonts.enableDefaultPackages = false;
      fonts.packages = [ fonts.noctua-iosevka-core ];

      hardware.graphics.enable = true;
      services.dbus.enable = true;
      security.polkit.enable = true;
    };
  };

  testScript = ''
    ${xvfb.prelude}

    # EVERYTHING THE VIEW RUNS, RUNS AS THE OWNER. Root would pass the polkit
    # rule for the wrong reason and prove nothing about it.
    def as_owner(command, **kw):
        return machine.succeed(
            "systemd-run --wait --pipe --uid=1000 --gid=100 "
            "--setenv=HOME=/home/noctua "
            "--setenv=XDG_STATE_HOME=/home/noctua/.local/state "
            "--setenv=XDG_CONFIG_HOME=/home/noctua/.config "
            f"--setenv=RUST_LOG=ronoctua_syncthing=debug {command}",
            **kw,
        )

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the module's Syncthing comes up and is tuned"):
        machine.wait_for_unit("syncthing.service")
        machine.wait_for_open_port(8384)
        # THE TUNING UNIT IS THE PART THAT COULD FAIL SILENTLY. It PATCHes two
        # endpoints that are not `settings` keys, and `--fail-with-body` is
        # what turns a renamed endpoint into a failure instead of a value
        # nobody applied.
        machine.wait_for_unit("ronoctua-syncthing-tune.service")

        # THE KEY IS READ THE WAY THE CRATE READS IT — between the tags, with
        # no XML parser. `xmllint` is in the TUNING UNIT's own `runtimeInputs`
        # and nowhere else, so asking for it here fails with exit 127 and a
        # message about a command, not about a key.
        key = machine.succeed(
            "sed -n 's:.*<apikey>\\(.*\\)</apikey>.*:\\1:p' "
            "/home/noctua/.local/state/syncthing/config.xml"
        ).strip()
        # PARSED, NOT MATCHED. Syncthing 2.x answers INDENTED JSON, so a test
        # that looked for `"copyRangeMethod":"ioctl"` failed on a service that
        # was tuned exactly right — and the failure printed the whole object,
        # with the value it was looking for visible in it.
        import json

        folder = json.loads(
            machine.succeed(
                f"curl -s -H 'X-API-Key: {key}' "
                "http://127.0.0.1:8384/rest/config/defaults/folder"
            )
        )
        assert folder["copyRangeMethod"] == "ioctl", folder
        assert folder["pullerMaxPendingKiB"] == 262144, folder
        device = json.loads(
            machine.succeed(
                f"curl -s -H 'X-API-Key: {key}' "
                "http://127.0.0.1:8384/rest/config/defaults/device"
            )
        )
        assert device["numConnections"] == 3, device

    with subtest("niri comes up on the winit backend over Xvfb"):
        start_xvfb()
        start_niri("niri", "${niri}")

    with subtest("the window opens on the real service and draws rows"):
        # A FOLDER AND A MACHINE, so the list has something in it. Both are
        # invented here: nothing in this repository may carry a real folder
        # path or a real device id (invariant 13).
        machine.succeed("mkdir -p /home/noctua/papers && chown noctua:users /home/noctua/papers")
        machine.succeed(
            f"curl -s -X PUT -H 'X-API-Key: {key}' -H 'Content-Type: application/json' "
            """--data '{"id":"papers","label":"papers","path":"/home/noctua/papers","type":"sendreceive","devices":[]}' """
            "http://127.0.0.1:8384/rest/config/folders/papers"
        )
        machine.succeed(
            f"curl -s -X PUT -H 'X-API-Key: {key}' -H 'Content-Type: application/json' "
            """--data '{"deviceID":"AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH","name":"the desk"}' """
            "http://127.0.0.1:8384/rest/config/devices/"
            "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH"
        )

        # THE WINDOW RUNS AS ROOT HERE, and `footer-vm` does the same. niri's
        # socket is in root's `XDG_RUNTIME_DIR`, so a window started as the
        # owner cannot reach it — the message is `Could not find wayland
        # compositor`, which says nothing about permissions.
        #
        # This subtest is about DRAWING. The one that is about privilege runs
        # as the owner, below, and is the only one that has to.
        machine.succeed(
            "systemd-run --unit=syncthing-window ${xvfb.env} "
            "--setenv=HOME=/home/noctua "
            "--setenv=XDG_STATE_HOME=/home/noctua/.local/state "
            "--setenv=XDG_CONFIG_HOME=/home/noctua/.config "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug,ronoctua_syncthing=debug "
            "${ronoctua-shell}/bin/ronoctua-shell syncthing"
        )

        try:
            for marker in [
                "graphics context ready",
            ]:
                machine.wait_until_succeeds(
                    "journalctl -u syncthing-window --no-pager | "
                    f"grep -q '{marker}'",
                    timeout=90,
                )
        except Exception:
            print("--- the window's journal ---")
            print(machine.execute("journalctl -u syncthing-window --no-pager")[1])
            print("--- niri's ---")
            print(machine.execute("journalctl -u niri --no-pager")[1])
            raise

        said = machine.succeed("journalctl -u syncthing-window --no-pager")
        # HERMETIC, and the same reason `footer-vm` gives: a number taken off a
        # real card would not be worth anything here.
        assert "llvmpipe" in said or "lavapipe" in said, said
        for bad in [
            "drawing the frame failed",
            "preparing the text failed",
            "no graphics context",
            "panicked",
        ]:
            assert bad not in said, f"the window reported: {bad}"

        # AND IT IS STILL ALIVE after the first frame. A crash right after it
        # would go unnoticed by everything above, which only reads a log.
        machine.succeed("systemctl is-active syncthing-window")

    with subtest("the one privileged thing works, as the owner, with no agent"):
        # THIS IS WHAT THE POLKIT RULE IS FOR, and it is the only thing in the
        # whole view that needs one. No `sudo`, no `pkexec`, no polkit agent in
        # the session — `nix/modules/update.nix` explains why there is none.
        as_owner("${pkgs.systemd}/bin/systemctl stop syncthing.service")
        machine.wait_until_fails("systemctl is-active syncthing.service")
        as_owner("${pkgs.systemd}/bin/systemctl start syncthing.service")
        machine.wait_for_unit("syncthing.service")

    with subtest("and the rule is the only door: a wider verb is refused"):
        # `restart` is not one of the two verbs, and the rule says nothing
        # about it — so polkit falls through to its default, which for a
        # system unit is `auth_admin`, and with no agent that is a refusal.
        machine.fail(
            "systemd-run --wait --pipe --uid=1000 --gid=100 "
            "${pkgs.systemd}/bin/systemctl restart syncthing.service"
        )
        # AND IT IS STILL RUNNING — the refusal did not take it down.
        machine.succeed("systemctl is-active syncthing.service")
  '';
}
