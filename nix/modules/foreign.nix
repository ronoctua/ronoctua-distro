# Foreign applications — GTK and Qt — wear the theme's MODE, not its colours.
#
# The design system does not reach inside a foreign window (the arrangement
# invariant 10 makes for icons holds for colours too): nothing here carries a
# token, and the browser extension's ten colours are generated elsewhere. What
# the system CAN tell a toolkit is which way round to be — and every theme this
# system ships is dark, so the answer is "prefer dark" and each toolkit's stock
# dark theme. A light theme, the day one exists, needs the generator to export
# a `mode` and this module to read it; until then the mode is a constant and
# this file says so.
#
# Three places, because the toolkits ask three questions: GTK 3 and 4 read
# `settings.ini`; a portal-aware app (GTK 4, Chromium, Electron) asks the
# settings portal, which answers out of dconf; Qt reads a platform theme.
{ config, lib, ... }:

let
  cfg = config.ronoctua;
  qtWanted = cfg.foreign.qt;
  # The one `settings.ini`, for both GTK majors. The icon theme's name is here
  # too — `nix/modules/icons.nix` owns the switch and this file owns the INI,
  # because two writers of one INI would be two `[Settings]` headers.
  settings = ''
    [Settings]
    gtk-application-prefer-dark-theme=1
    gtk-theme-name=Adwaita-dark
  ''
  + lib.optionalString cfg.icons.superpack ''
    gtk-icon-theme-name=noctua
  '';
in
{
  # QT IS A SWITCH, because telling Qt to be dark costs Qt: `qt.enable` with
  # the GNOME platform theme installs qgnomeplatform and adwaita-qt for both
  # majors — measured 591 MiB of closure — and the minimal profile ships no
  # Qt application to theme. `nix/modules/profile.nix` turns it on with `full`.
  options.ronoctua.foreign.qt = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = "Give Qt applications the dark platform theme (installs the Qt platform theme packages).";
  };

  config = {
    environment.etc."xdg/gtk-3.0/settings.ini".text = settings;
    environment.etc."xdg/gtk-4.0/settings.ini".text = settings;

    # What the settings portal answers: `programs.dconf.enable` is niri's.
    programs.dconf.profiles.user.databases = [
      {
        settings."org/gnome/desktop/interface" = {
          color-scheme = "prefer-dark";
          gtk-theme = "Adwaita-dark";
        }
        // lib.optionalAttrs cfg.icons.superpack { icon-theme = "noctua"; };
      }
    ];

    qt = lib.mkIf qtWanted {
      enable = true;
      platformTheme = "gnome";
      style = "adwaita-dark";
    };
  };
}
