# Each name once on both buses, and every name still answers.
#
# # Two machines, and one difference between them
#
# `nix/modules/dbus.nix` filters a configuration, and what it can get wrong is
# quiet: a bus that starts, and a name that no longer activates. So `fixed` and
# `plain` declare the same services and only `fixed` imports the module. `plain`
# is the control: zero lines of `Ignoring duplicate name` mean something only
# beside a machine that writes them, on both buses, at the start and at a reload.
#
# The GTK portal is listed twice on purpose, as two modules of the distro once
# listed it: a package repeated is one more way to say a name twice.
#
# # What only the session bus reads
#
# The session bus keeps `<standard_session_servicedirs/>`, and through it
# `XDG_DATA_DIRS`. A service in a Flatpak's exports, a directory no package
# lists, has to be found there: the last step plants one and asks.
{ pkgs }:

let
  user = "alice";

  node =
    imports:
    { ... }:
    {
      inherit imports;
      services.upower.enable = true;
      services.flatpak.enable = true;
      programs.dconf.enable = true;
      xdg.portal = {
        enable = true;
        extraPortals = [
          pkgs.xdg-desktop-portal-gtk
          pkgs.xdg-desktop-portal-gtk
        ];
        config.common.default = "*";
      };
      # A LINGERING PERSON, because a session bus needs a user manager and a
      # test has no login.
      users.users.${user} = {
        isNormalUser = true;
        linger = true;
      };
    };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-dbus-names";

  nodes = {
    fixed = node [ ../nix/modules/dbus.nix ];
    plain = node [ ];
  };

  testScript = ''
    import json

    DUPLICATE = "Ignoring duplicate name"
    BUSES = {
        "system": "_SYSTEMD_UNIT=dbus-broker.service",
        "session": "_SYSTEMD_USER_UNIT=dbus-broker.service",
    }
    EXPORTS = "/var/lib/flatpak/exports/share/dbus-1/services"
    EXPORTED = "org.example.Exported"
    AS_PERSON = "runuser -u ${user} -- env XDG_RUNTIME_DIR=/run/user/1000"
    SESSION = f"{AS_PERSON} busctl --user"


    def lines(machine):
        """The broker's complaints so far, by bus. `--sync` first: a zero must
        not be a line journald had not written yet."""
        machine.succeed("journalctl --sync")
        return {
            bus: int(
                machine.succeed(
                    f"journalctl -b -o cat {match} | grep -c '{DUPLICATE}' || true"
                ).strip()
            )
            for bus, match in BUSES.items()
        }


    def activatable(machine, bus):
        listed = machine.succeed(
            f"{bus} --json=short call org.freedesktop.DBus /org/freedesktop/DBus "
            "org.freedesktop.DBus ListActivatableNames"
        )
        return set(json.loads(listed)["data"][0])


    def reload_both(machine):
        machine.succeed("systemctl reload dbus-broker.service")
        machine.succeed(f"{AS_PERSON} systemctl --user reload dbus-broker.service")


    start_all()
    for machine in (fixed, plain):
        machine.wait_for_unit("multi-user.target")
        machine.wait_for_unit("user@1000.service")
        # Nothing opens the session bus here, and the broker reads its
        # directories when it starts.
        machine.succeed(f"{SESSION} --no-pager list")

    with subtest("the control: the plain machine repeats itself on both buses"):
        before = lines(plain)
        print(f"plain, at the start: {before}")
        for bus, said in before.items():
            assert said > 0, f"plain's {bus} bus wrote no duplicate: this test has stopped measuring"

    with subtest("the fixed machine says nothing on either bus, at the start"):
        said = lines(fixed)
        print(f"fixed, at the start: {said}")
        assert not any(said.values()), f"the fixed machine still repeats itself: {said}"

    with subtest("a reload is one more load: the control again, and silence"):
        reload_both(plain)
        after = lines(plain)
        print(f"plain, after a reload: {after}")
        for bus in BUSES:
            assert after[bus] > before[bus], f"plain's {bus} bus read nothing at the reload"
        reload_both(fixed)
        said = lines(fixed)
        print(f"fixed, after a reload: {said}")
        assert not any(said.values()), f"the fixed machine repeats itself at a reload: {said}"

    for machine in (fixed, plain):
        with subtest(f"{machine.name}: the names are listed, and they answer"):
            system = activatable(machine, "busctl")
            for name in (
                "org.freedesktop.login1",
                "org.freedesktop.UPower",
                "org.freedesktop.Flatpak.SystemHelper",
            ):
                assert name in system, f"the system bus lost {name}: {sorted(system)}"
            session = activatable(machine, SESSION)
            for name in ("org.freedesktop.portal.Desktop", "ca.desrt.dconf"):
                assert name in session, f"the session bus lost {name}: {sorted(session)}"
            # AS THE PERSON: a policy file lost is a call refused, and root is
            # refused nothing.
            machine.succeed(
                f"{AS_PERSON} busctl call org.freedesktop.UPower /org/freedesktop/UPower "
                "org.freedesktop.UPower EnumerateDevices"
            )
            machine.succeed(
                f"{SESSION} call ca.desrt.dconf /ca/desrt/dconf org.freedesktop.DBus.Peer Ping"
            )

        with subtest(f"{machine.name}: a Flatpak's exports still reach the session bus"):
            machine.succeed(f"mkdir -p {EXPORTS}")
            machine.succeed(
                f"printf '[D-BUS Service]\\nName={EXPORTED}\\nExec=/run/current-system/sw/bin/true\\n' "
                f"> {EXPORTS}/{EXPORTED}.service"
            )
            reload_both(machine)
            session = activatable(machine, SESSION)
            assert EXPORTED in session, f"the session bus did not read the exports: {sorted(session)}"
  '';
}
