# THE STAGE FOR OTHER PEOPLE'S APPS: `nix run .#shot -- --apps <scene>`.
#
# The same tool as `nix/shot.nix` — a machine, the compositor over Xvfb, and a
# scene the person building a feature writes OUTSIDE this repository — but for
# the question that stage cannot answer: what does an app FROM FLATHUB look
# like on this system. So it carries no footer and three things the plain
# stage does not:
#
# * **Flatpak with a user installation**, and the portal that answers a
#   sandboxed app's "is the system dark?" (`xdg-desktop-portal-gtk`, the same
#   backend `nix/modules/scroll.nix` configures).
# * **The distro's dark question, asked the way the distro asks it**: the two
#   dconf keys `nix/modules/foreign.nix` writes — `color-scheme` and
#   `gtk-theme` — and nothing else. `GTK_THEME` is absent on purpose: it is
#   the variable that switches libadwaita's own stylesheet off, and proving
#   that is one of the things this stage exists for.
# * **Room and a route out.** A GNOME runtime is about a gigabyte installed;
#   the disk is sparse, so the size is a ceiling and not a cost. The test
#   instrumentation clears the static gateway and nameservers so a test cannot
#   reach the internet by accident; this stage asks for them on purpose, by
#   DHCP on QEMU's user network. That only works when the driver runs outside
#   the build sandbox — which is what `nix run` does — and never inside
#   `nix flake check`, where there is no network to reach.
{
  pkgs,
  lib,
  scroll,
}:

let
  xvfb = import ../tests/compositor-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-shot-apps";

  nodes.machine = {
    virtualisation.memorySize = 4096;
    virtualisation.cores = 4;
    virtualisation.diskSize = 16384;

    # THE ROUTE OUT, asked for on purpose (see the header).
    networking.interfaces.eth0.useDHCP = lib.mkForce true;
    networking.nameservers = lib.mkForce [ "10.0.2.3" ];

    services.flatpak.enable = true;
    xdg.portal = {
      enable = true;
      extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
      config.common.default = [ "gtk" ];
    };
    # THE TWO KEYS `nix/modules/foreign.nix` WRITES, and only those: the
    # settings portal serves `color-scheme` to libadwaita and GTK 4, and
    # `gtk-theme` to GTK 3 (and to flatpak, which installs the theme extension
    # matching it as a related ref).
    programs.dconf = {
      enable = true;
      profiles.user.databases = [
        {
          settings."org/gnome/desktop/interface" = {
            color-scheme = "prefer-dark";
            gtk-theme = "Adwaita-dark";
          };
        }
      ];
    };

    environment.systemPackages = [
      scroll
      pkgs.imagemagick
      pkgs.xwd
      pkgs.xdotool
      pkgs.xwininfo
      # `gdbus`, to ask the portal what it answers.
      pkgs.glib
      pkgs.dconf
      pkgs.curl
      pkgs.jq
    ]
    ++ xvfb.packages;

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    import os as _os

    ${xvfb.prelude}

    # A SESSION, as a person's would have one: a bus, the portal and its gtk
    # backend on it, and the dconf service that answers them.
    # The programs a scene starts the session with, by path.
    PORTAL = "${pkgs.xdg-desktop-portal}/libexec/xdg-desktop-portal"
    PORTAL_GTK = "${pkgs.xdg-desktop-portal-gtk}/libexec/xdg-desktop-portal-gtk"
    DCONF_SERVICE = "${pkgs.dconf.lib}/libexec/dconf-service"
    FLATPAK = "${pkgs.flatpak}/bin/flatpak"
    # The portal a sandboxed app asks to start ANOTHER sandbox: GTK's image
    # loaders (glycin) run every decode through `flatpak-spawn --sandbox`, and
    # without it a GTK 3 app aborts on its first icon — measured, gedit exits
    # 134 on `image-missing.svg`.
    FLATPAK_PORTAL = "${pkgs.flatpak}/libexec/flatpak-portal"
    SESSION = (
        "--setenv=XDG_RUNTIME_DIR=/run/user/0 "
        "--setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
        "--setenv=WAYLAND_DISPLAY=wayland-1 "
        "--setenv=XDG_CURRENT_DESKTOP=scroll:sway:wlroots "
        "--setenv=XDG_DATA_DIRS=/root/.local/share/flatpak/exports/share:/var/lib/flatpak/exports/share:/run/current-system/sw/share "
        # Where NixOS's portal looks for its backends, and the dconf module
        # every GIO program needs to READ the keys above instead of an empty
        # in-memory default.
        "--setenv=NIX_XDG_DESKTOP_PORTAL_DIR=/run/current-system/sw/share/xdg-desktop-portal/portals "
        "--setenv=GIO_EXTRA_MODULES=${pkgs.dconf.lib}/lib/gio/modules "
        "--setenv=HOME=/root "
    )


    def in_session(unit, command):
        """Start `command` as a transient unit inside the session above."""
        machine.succeed(f"systemd-run --unit={unit} {SESSION} {command}")


    def session_run(command, timeout=900):
        """Run `command` to its end inside the session, and return its output."""
        return machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 "
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "WAYLAND_DISPLAY=wayland-1 HOME=/root "
            "XDG_DATA_DIRS=/root/.local/share/flatpak/exports/share:/var/lib/flatpak/exports/share:/run/current-system/sw/share "
            "GIO_EXTRA_MODULES=${pkgs.dconf.lib}/lib/gio/modules "
            f"{command}",
            timeout=timeout,
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_compositor("compositor", "${scroll}")
    sock = socket_of("compositor")
    # The bus gets the session's environment too, so it finds the activation
    # files under `XDG_DATA_DIRS` as a person's bus would.
    machine.succeed(
        f"systemd-run --unit=bus {SESSION} "
        "${pkgs.dbus}/bin/dbus-daemon --session --nofork "
        "--address=unix:path=/run/user/0/bus"
    )
    machine.wait_until_succeeds("test -S /run/user/0/bus", timeout=30)

    _scene = _os.environ.get("RONOCTUA_SHOT_SCENE")
    assert _scene, "no scene: run `nix run .#shot -- --apps <scene.py>`"
    exec(compile(open(_scene).read(), _scene, "exec"))
  '';
}
