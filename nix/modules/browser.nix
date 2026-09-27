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
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.browser;

  chrome = pkgs.google-chrome.override {
    commandLineArgs = [
      # The whole point of this module.
      "--password-store=basic"
      # Wayland natively: without it Chrome runs under XWayland, which on a
      # HiDPI screen means blurred text and no fractional scaling.
      "--ozone-platform-hint=auto"
      # NO `--enable-features=WaylandWindowDecorations`, which was here. That
      # feature exists only in ELECTRON — the name is absent from the Chrome
      # binary, so it asked for nothing — and it was a trap besides: Chrome
      # honours the LAST `--enable-features` on its command line, so a web
      # app's own list (`ronoctua-software web-open`) would have been silently
      # replaced by this one, or replaced it. The nixpkgs wrapper still adds
      # the same flag itself under `NIXOS_OZONE_WL`, which is why `web-open`
      # never passes `--enable-features` at all.
    ];
  };

  # THE KIOSK'S BRIDGE, as the footer finds it: the host manifest the
  # extension-packing unit writes (`nix/modules/browser-extension.nix`), which
  # carries our extension's id in its allow-list — the id is born on the
  # machine, so a path to the file is all the footer can be told.
  bridgeManifest = "/etc/opt/chrome/native-messaging-hosts/${(import ../browser-hosts.nix).browser}.json";

  # THE LAUNCHER'S DOOR TO THE KIOSK (`LEADER m b` is the other): the
  # person's own Chrome, without its bars, the footer drawing its tabs. A
  # `.desktop` has room for one language, and that is English (AGENTS.md,
  # invariant 18). `Exec` is the store path, the rule for a program the
  # system calls; `%u` hands an address over when one is opened with it.
  # No `MimeType`: it opens an address when ASKED, and never takes links
  # from the person's default browser. Its glyph comes from
  # `assets/icons/tray-glyphs.nix`, by this id.
  kioskEntry = pkgs.writeTextDir "share/applications/br.ronoctua.kiosk.desktop" ''
    [Desktop Entry]
    Type=Application
    Name=Chrome kiosk
    Comment=Your Chrome without its bars; the footer draws its tabs
    Exec=${ronoctua.shell}/bin/ronoctua-shell command kiosk-open %u
    Terminal=false
    Categories=Network;WebBrowser;
  '';
in
{
  options.ronoctua.browser = {
    enable = lib.mkEnableOption "Google Chrome (0.42 GiB)" // {
      default = true;
    };
    # THE CHROME THIS MACHINE RUNS. The wrapped one above; `tests/kiosk-vm.nix`
    # hands in the real Chrome with its own test flags, because a test node's
    # `pkgs` admits nothing unfree. Whatever is here is what
    # `/run/current-system/sw/bin/google-chrome-stable` is.
    package = lib.mkOption {
      type = lib.types.package;
      default = chrome;
      defaultText = lib.literalExpression "pkgs.google-chrome with --password-store=basic --ozone-platform-hint=auto";
      description = "The Chrome installed, and the one the kiosk and the web apps open.";
    };
  };

  config = {
    environment.systemPackages = lib.mkIf cfg.enable [
      cfg.package
      kioskEntry
    ];

    # WHERE THE FOOTER READS OUR EXTENSION'S ID: the unit's environment, and
    # not the login shell's — a user unit inherits the manager's, which is
    # the trap `RONOCTUA_GRIM` records in `nix/modules/scroll.nix`.
    systemd.user.services.ronoctua-footer.environment.RONOCTUA_BROWSER_MANIFEST =
      lib.mkIf cfg.enable bridgeManifest;

    # The portal is what a file dialog and a screen share go through on Wayland.
    # `xdg.portal` is already enabled by nix/modules/scroll.nix; this only adds the
    # GTK backend, which is the one Chrome talks to.
    xdg.portal.extraPortals = lib.mkIf cfg.enable [
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
