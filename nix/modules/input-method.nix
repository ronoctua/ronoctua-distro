# The input method — fcitx5, with Mozc (Japanese), Rime (Chinese) and Hangul
# (Korean) — the owner's choice of 2026-11-02, installed on every machine.
#
# # Why one is needed at all
#
# Japanese, Chinese and Korean are not typed a key a character: an input method
# reads the keys, composes, and hands the window the text it settled on. A
# window takes that through `zwp_text_input_v3` (the shell's own door,
# `crates/ronoctua-shell-door/src/door/text_input.rs`, and every GTK 4 or
# Qt 6 program), and the compositor relays it to an input method that speaks
# `zwp_input_method_v2` — the pinned compositor has the relay (`sway/server.c`),
# and nothing on the machine spoke the other half until this module. Without
# it a window that enables its text input is enabled for nobody.
#
# # Why the nixpkgs module, and the Wayland front-end
#
# `i18n.inputMethod` is how NixOS declares fcitx5 and its engines, and nothing
# here reimplements it. `waylandFrontend` because this session is Wayland
# only: the module then sets no `GTK_IM_MODULE` and no `QT_IM_MODULE`, which
# would route those programs through fcitx5's own toolkit plugins instead of
# the protocol their toolkits already speak (fcitx5's own guide for Wayland
# compositors says the same).
#
# # The engines are THERE, not switched on
#
# fcitx5 starts with one input method in its group, the machine's keyboard
# layout, and its switch (`Ctrl+Space`, its default) only acts with two
# methods in the group. A person who writes
# Japanese adds Mozc in fcitx5's own configuration ("Fcitx 5 Configuration"),
# once, and from then on `Ctrl+Space` turns it on and off. Putting the three in
# every group would have taken `Ctrl+Space` — Emacs's mark — from every
# program on every machine, for the people who write none of the three.
#
# # Why a unit of its own
#
# NixOS installs fcitx5 and leaves starting it to the desktop's autostart,
# which this session does not run. So it is a user unit of the graphical
# session, as the footer is: it comes up once the compositor has exported
# `WAYLAND_DISPLAY` (`graphical-session.target`, `nix/modules/scroll.nix`),
# goes with the session, and comes back if it dies. The two windows that must
# work when nothing else does — the rescue console and the monitor — never
# enable a text input, so a dead or hung input method costs them nothing.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.inputMethod;
in
{
  options.ronoctua.inputMethod.enable = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Run fcitx5 in the session, with the Mozc, Rime and Hangul engines
      installed (the person adds an engine to fcitx5's group to use it).
    '';
  };

  config = lib.mkIf cfg.enable {
    i18n.inputMethod = {
      enable = true;
      type = "fcitx5";
      fcitx5 = {
        waylandFrontend = true;
        addons = [
          pkgs.fcitx5-mozc
          pkgs.fcitx5-rime
          pkgs.fcitx5-hangul
        ];
        # FCITX5'S KEYS THAT ARE NOT COMPOSING, given back to the programs.
        # While a window's text input is enabled every key passes through
        # fcitx5 first, and two of its addons take a key of their own there
        # whatever the group holds (read at the pinned 5.1.23):
        # `Control+semicolon` opens its clipboard history — this machine
        # has one already, the footer's — and `Control+Shift+U` /
        # `Control+Alt+Shift+U` type a character by its number. In a
        # terminal all three are the program's. EMPTY, AND NOT A SECTION:
        # fcitx5 reads `Key=` as a list of none, while a `[Key]` with no
        # entries is no option at all, and the default stays.
        settings.addons = {
          clipboard.globalSection.TriggerKey = "";
          unicode.globalSection = {
            TriggerKey = "";
            DirectUnicodeMode = "";
          };
        };
      };
    };

    systemd.user.services.ronoctua-input-method = {
      description = "fcitx5 — the session's input method";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      serviceConfig = {
        # BY ITS STORE PATH, the package the nixpkgs module built with the
        # engines in it (a unit's `PATH` is minimal). `--replace`: an
        # instance some program started by hand gives way to the session's.
        ExecStart = "${config.i18n.inputMethod.package}/bin/fcitx5 --replace";
        Restart = "on-failure";
        RestartSec = 1;
        Slice = "session.slice";
      };
    };

    # X PROGRAMS FIND IT TOO, through XWayland's input method protocol. TWICE,
    # because two things start programs: the nixpkgs module's
    # `environment.variables` reaches the shells, and `environment.d` the
    # user manager — every unit and every scope the footer opens (the route
    # `nix/modules/cache.nix` takes, and NixOS itself for `PATH`).
    environment.etc."environment.d/60-ronoctua-input-method.conf".text = ''
      XMODIFIERS=@im=fcitx
    '';
  };
}
