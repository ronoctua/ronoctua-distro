# Google Chrome, and the reason it never asks for a password.
#
# WHY IT ASKS AT ALL
# ------------------
# Chrome stores saved passwords through a "password store". On Linux it probes
# for one at startup: if it finds a Secret Service on D-Bus (gnome-keyring,
# kwallet), it uses it — and a keyring that has never been unlocked prompts for
# a password before the first window appears. That prompt is not Chrome's; it is
# the keyring's, and no Chrome setting turns it off.
#
# So there are two halves, and BOTH are needed:
#
#   1. `--password-store=basic` — stop probing, store locally.
#   2. no gnome-keyring anywhere in the closure — because a Secret Service that
#      exists will eventually be found by something else, and the prompt comes
#      back through a door nobody remembers opening.
#
# WHAT IT COSTS, PLAINLY
# ----------------------
# `basic` means the passwords Chrome saves are readable by any process running
# as this user. The keyring would not have helped much on a machine already
# unlocked and logged in, but it is a real difference and it is not being traded
# away by accident: the protection that remains is the LUKS volume, and that one
# only counts while the machine is off.
#
# The check `browser-has-no-keyring` refuses a closure that brings gnome-keyring
# back — otherwise this comment is the only thing standing between here and a
# password prompt returning by way of some GTK dependency.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  chrome = pkgs.google-chrome.override {
    commandLineArgs = [
      # The whole point of this module.
      "--password-store=basic"
      # Wayland natively: without it Chrome runs under XWayland, which on a
      # HiDPI screen means blurred text and no fractional scaling.
      "--ozone-platform-hint=auto"
      "--enable-features=WaylandWindowDecorations"
    ];
  };
in
{
  options.ronoctua.browser.enable = lib.mkEnableOption "Google Chrome (0.42 GiB)" // {
    default = true;
  };

  config = {
    environment.systemPackages = lib.mkIf config.ronoctua.browser.enable [ chrome ];

    # The portal is what a file dialog and a screen share go through on Wayland.
    # `xdg.portal` is already enabled by nix/modules/niri.nix; this only adds the
    # GTK backend, which is the one Chrome talks to.
    xdg.portal.extraPortals = lib.mkIf config.ronoctua.browser.enable [
      pkgs.xdg-desktop-portal-gtk
    ];

    # THE KEYRING STAYS FORCED OFF EVEN WITHOUT CHROME.
    #
    # It is written as a negative because leaving it unmentioned is how it comes
    # back, and that is true of a machine with no browser too: several desktop
    # packages enable a Secret Service as a side effect, and the prompt would
    # then appear with nothing on screen to explain it. Turning the guard off
    # with the browser would make the reduced profile the one where it returns.
    services.gnome.gnome-keyring.enable = lib.mkForce false;
    programs.seahorse.enable = lib.mkForce false;

    # `security.pam.services.*.enableGnomeKeyring` is what would unlock it at
    # login. Forced off for the same reason.
    security.pam.services.login.enableGnomeKeyring = lib.mkForce false;
  };
}
