# The belongings' identity kept for the session, on a machine of its own.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The crate's tests keep the identity in a vault of their own memory, and the
# kernel's keyring in a thread keyring of the test's: neither is the user
# keyring of a person's session, run the way the session runs the CLI. Here:
#
#   1. the CLI run as a USER SERVICE — as the transfer queue runs it — keeps
#      the identity a typed passphrase opened in the user keyring (`@u`), and
#      a second sync, another user service, finds it and asks nothing: user
#      services can reach `@u` (the risk the design named);
#   2. the identity is in no file anywhere: the home, the runtime folder, the
#      repository, `/var`, `/etc` — grep's own word for it, never its exit
#      code alone (2 is "a file could not be read", even with a match);
#   3. the development loop's copy (`RONOCTUA_INSTANCE=dev`) forgetting —
#      what turning `keep_unlocked` off there queues — leaves the session's
#      identity and marker alone;
#   4. the end of the graphical session forgets it
#      (`ronoctua-belongings-forget.service`, stopped with the session), the
#      marker with it, and the next sync is refused for want of a passphrase;
#   5. the sleep's system unit, running AS THE PERSON with no settings and no
#      runtime variable, forgets it too (started here: the machine is never
#      slept, the `private-vm` precedent).
#
# A real sync: an SSH remote on the machine itself (`git@localhost`), the
# deploy key the CLI makes, the host let in `consent.json` — the belongings'
# own path, post-quantum key exchange and all.
#
# EVERYDAY BATTERY: one machine from one module, no disk, no reboot, no TPM.
{
  pkgs,
  lib,
  belongingsCli,
}:

let
  user = "noctua";
  uid = 1000;
  # THE TEST'S OWN PASSPHRASE — a fixture, never a person's.
  passphrase = "a passphrase only this test types";
  repository = "git@localhost:belongings.git";
  cli = "${belongingsCli}/bin/ronoctua-belongings";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-belongings-unlock";

  nodes.machine =
    { ... }:
    {
      imports = [ ../nix/modules/belongings.nix ];

      # What `belongings.nix` reads of other modules, declared bare here (the
      # `security-vm` precedent): the person, the queue's switch, and the
      # flake's `lastModified` for `/etc/ronoctua/built-s`.
      options.ronoctua.user = lib.mkOption { type = lib.types.str; };
      options.ronoctua.transfers.enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
      };

      config = {
        _module.args.inputs.self.lastModified = 0;
        ronoctua.user = user;
        ronoctua.belongings.cli = belongingsCli;

        # THE PERSON, whose user manager runs without anybody logged in.
        users.users.${user} = {
          isNormalUser = true;
          inherit uid;
          linger = true;
        };
        # THE REMOTE'S ACCOUNT: a bare repository in its home, reached by the
        # deploy key alone.
        users.users.git = {
          isNormalUser = true;
          home = "/srv/git";
          createHome = true;
        };
        services.openssh = {
          enable = true;
          settings.PasswordAuthentication = false;
        };
        environment.systemPackages = [
          pkgs.git
          pkgs.keyutils
        ];

        # A GRAPHICAL SESSION'S STAND-IN: `graphical-session.target` refuses
        # to be started by hand, and is pulled in and let go by a target that
        # binds it — what the compositor's session does.
        systemd.user.targets.stand-in-session = {
          bindsTo = [ "graphical-session.target" ];
          wants = [ "graphical-session.target" ];
          before = [ "graphical-session.target" ];
        };
      };
    };

  testScript = ''
    import hashlib, shlex

    tag = hashlib.sha256(${builtins.toJSON repository}.encode()).hexdigest()[:16]
    description = f"ronoctua-belongings:ronoctua:{tag}"
    run = "/run/user/${toString uid}"
    marker = f"{run}/ronoctua-belongings/unlocked"
    env = f"XDG_RUNTIME_DIR={run} DBUS_SESSION_BUS_ADDRESS=unix:path={run}/bus"

    def person(cmd, ok=True):
        line = f"su - ${user} -c {shlex.quote(env + ' ' + cmd)}"
        return machine.succeed(line) if ok else machine.fail(line)

    def service(cmd, ok=True):
        """As the queue runs the CLI: a transient USER service."""
        return person(f"systemd-run --user --wait --collect --pipe --quiet {cmd}", ok)

    def typed():
        person(
            f"install -d -m 0700 {run}/ronoctua-belongings && "
            f"(umask 077 && printf %s {shlex.quote(${builtins.toJSON passphrase})} > {run}/ronoctua-belongings/passphrase)"
        )

    def sync(ok=True):
        out = service("${cli} prepare --progress", ok)
        if ok:
            assert "BELONGINGS\tdone\t0" in out, out
        return out

    def kept():
        return machine.execute(
            f"su - ${user} -c {shlex.quote(env + ' systemd-run --user --wait --collect --pipe --quiet keyctl search @u user ' + description)}"
        )[0] == 0

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("sshd.service")
    machine.wait_for_unit("user@${toString uid}.service")

    with subtest("the remote, the deploy key, the settings and the consent"):
        machine.succeed("su - git -c 'git init -q --bare -b main belongings.git'")
        person("${cli} keygen")
        machine.succeed(
            "install -d -o git -g users -m 0700 /srv/git/.ssh && "
            "install -o git -g users -m 0600 "
            "/home/${user}/.config/ronoctua/belongings-keys/deploy.pub /srv/git/.ssh/authorized_keys"
        )
        person(
            "printf '%s' '{\"repository\": \"${repository}\", \"auth\": \"ssh\"}' "
            "> ~/.config/ronoctua/belongings.json && "
            "printf '%s' '{\"hosts\": {\"localhost\": true}}' > ~/.config/ronoctua/consent.json"
        )
        person("systemctl --user start stand-in-session.target")
        person("systemctl --user is-active ronoctua-belongings-forget.service")

    with subtest("a typed passphrase opens, and the identity is kept for the session"):
        typed()
        sync()
        machine.fail(f"test -e {run}/ronoctua-belongings/passphrase")
        machine.succeed(f"test -s {marker}")
        assert kept(), "the identity was not kept in the user keyring"

    with subtest("a second sync, another user service, asks nothing"):
        sync()

    with subtest("the identity is in no file"):
        identity = service(
            f"sh -c 'keyctl link @u @s >/dev/null && keyctl pipe %user:{description}'"
        ).strip()
        assert identity.startswith("AGE-SECRET-KEY-PQ-1"), "what is kept is not an identity"

        def nowhere(paths, whole):
            # GREP'S OWN WORD: `-l` prints every file that holds it, read or
            # not by the end, and 2 means only that something could not be
            # read — a match included. `-D skip`: a socket or a FIFO is no
            # file, and reading one waits forever.
            code, out = machine.execute(
                f"grep -rlFs -D skip -- {shlex.quote(identity)} {paths}"
            )
            assert out.strip() == "", f"the identity is in: {out}"
            if whole:
                assert code == 1, f"grep could not read all of {paths} ({code})"

        # WHERE THE CLI, ITS UNITS AND THE REMOTE WRITE: read whole, and
        # nothing there holds it.
        nowhere("/home /srv /run/user /tmp", whole=True)
        # AND THE REST OF THE LIVE SYSTEM, where a file may vanish mid-scan.
        nowhere("/run /var /etc", whole=False)

    with subtest("the development loop's forget leaves the session's identity"):
        person(
            "systemd-run --user --wait --collect --pipe --quiet "
            "--setenv=RONOCTUA_INSTANCE=dev ${cli} forget"
        )
        assert kept(), "the development loop's forget took the session's identity"
        machine.succeed(f"test -s {marker}")

    with subtest("the end of the session forgets it"):
        person("systemctl --user stop stand-in-session.target")
        machine.wait_until_fails(
            f"su - ${user} -c {shlex.quote(env + ' systemctl --user is-active ronoctua-belongings-forget.service')}"
        )
        assert not kept(), "the identity outlived the session"
        machine.fail(f"test -e {marker}")
        out = sync(ok=False)

    with subtest("the sleep's unit forgets it, as the person"):
        typed()
        sync()
        assert kept()
        machine.succeed("systemctl start ronoctua-belongings-forget-sleep.service")
        assert not kept(), "the identity outlived the sleep"
        machine.fail(f"test -e {marker}")
        sync(ok=False)
  '';
}
