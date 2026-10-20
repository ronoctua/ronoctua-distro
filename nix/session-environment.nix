# THE SESSION'S ENVIRONMENT, named once.
#
# What the compositor's config hands the person's manager and D-Bus's
# activation environment when the session comes up (`nix/modules/scroll.nix`,
# "THE SESSION COMES UP HERE"), and what the VM tests that stand in for a
# session put in their own config (`tests/x11-vm.nix`, `tests/emacs-vm.nix`).
# A copy of the line written by hand in a test is a test that keeps passing
# when the line changes: the X11 test gave `DISPLAY` to its program by hand,
# and the session never exported it.
#
# `DISPLAY` IS THE COMPOSITOR'S XWAYLAND. The pinned compositor sets it in its
# own environment in `server_start` (`sway/server.c`), before the backend
# starts and before the config's deferred `exec`s run (`sway/main.c`), so the
# processes these lines start read it. XWayland is lazy (the compositor's
# default): the socket is taken at that moment and the server starts at the
# first X client, so exporting the name starts nothing. When XWayland cannot
# be created the compositor unsets `DISPLAY`, and `import-environment` skips
# a variable that is not set ("not set, ignoring", exit 0).
#
# A program that speaks both protocols keeps choosing by its own rule — GTK
# and the pure-GTK `emacsclient` read `WAYLAND_DISPLAY` first, Chrome and Qt
# go by `XDG_SESSION_TYPE` — and what speaks only X11 now finds a server.
{ pkgs }:

let
  names = [
    "WAYLAND_DISPLAY"
    "DISPLAY"
    "SCROLLSOCK"
    "SWAYSOCK"
    "XDG_CURRENT_DESKTOP"
    "XDG_SESSION_TYPE"
  ];
  said = builtins.concatStringsSep " " names;
in
{
  inherit names;

  # STORE PATHS AND NOT NAMES: the compositor's children start with the
  # unit's PATH, and a bare `systemctl` here fails far from the cause.
  lines = ''
    exec ${pkgs.systemd}/bin/systemctl --user import-environment ${said}
    exec ${pkgs.dbus}/bin/dbus-update-activation-environment --systemd ${said}
  '';
}
