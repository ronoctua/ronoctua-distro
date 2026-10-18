# Foreign applications — GTK and Qt — wear the theme's MODE, not its colours.
#
# The design system does not reach inside a foreign window (the arrangement
# invariant 10 makes for icons holds for colours too): nothing here carries a
# token, and the browser extension's ten colours are generated elsewhere. What
# the system CAN tell a toolkit is which way round to be.
#
# AND IT IS READ NOW, WHERE IT USED TO BE A CONSTANT. Every theme was dark
# until `flashbang`, so this file answered "prefer dark" and was right nine
# times out of nine; its own header said what the tenth would cost — *"a light
# theme, the day one exists, needs the generator to export a `mode` and this
# module to read it"*. The generator writes a `mode` fragment beside the six
# colour ones now, and this is the module that reads it.
#
# IT IS THE DECLARED THEME'S MODE AND NOT THE RUNNING ONE. What `ronoctua.theme`
# names is what the machine is built with; switching at run time with
# `LEADER s t` is the shell's half, through dconf, because `/etc/xdg` is in the
# store and read-only (`crates/ronoctua-shell-kit/src/toolkits.rs`). The two agree
# on the first boot and the shell owns every change after it.
#
# Three places, because the toolkits ask three questions: GTK 3 and 4 read
# `settings.ini`; a portal-aware app (GTK 4, Chromium, Electron) asks the
# settings portal, which answers out of dconf; Qt reads a platform theme.
{
  config,
  lib,
  # `themed`: the generated palettes. The module arg, as `nix/modules/scroll.nix`
  # takes it — `config.ronoctua` carries the CHOICE and this carries the store.
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua;
  qtWanted = cfg.foreign.qt;

  # THE THEME'S OWN WORD, read from the fragment the generator wrote. A
  # `readFile` of a build output is not import-from-derivation: `ronoctua.themed`
  # is a path this evaluation already has.
  mode = lib.removeSuffix "\n" (builtins.readFile "${ronoctua.themed}/${cfg.theme}/mode");
  dark = mode == "dark";

  # Each toolkit's stock theme for the side we are on. Stock and not ours:
  # nothing here carries a token, and a hand-written GTK theme would be the
  # design system reaching into a foreign window.
  gtkTheme = if dark then "Adwaita-dark" else "Adwaita";
  qtStyle = if dark then "adwaita-dark" else "adwaita";
  scheme = if dark then "prefer-dark" else "prefer-light";

  # The one `settings.ini`, for both GTK majors. The icon theme's name is here
  # too — `nix/modules/icons.nix` owns the switch and this file owns the INI,
  # because two writers of one INI would be two `[Settings]` headers.
  settings = ''
    [Settings]
    gtk-application-prefer-dark-theme=${if dark then "1" else "0"}
    gtk-theme-name=${gtkTheme}
  ''
  + lib.optionalString cfg.icons.superpack ''
    gtk-icon-theme-name=noctua
  '';
in
{
  # QT IS A SWITCH, because telling Qt which way round to be costs Qt:
  # `qt.enable` with the GNOME platform theme installs qgnomeplatform and
  # adwaita-qt for both majors — measured 591 MiB of closure — and the minimal
  # profile ships no Qt application to theme. `nix/modules/profile.nix` turns
  # it on with `full`.
  options.ronoctua.foreign.qt = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = "Give Qt applications the platform theme matching the theme's mode (installs the Qt platform theme packages).";
  };

  config = {
    environment.etc."xdg/gtk-3.0/settings.ini".text = settings;
    environment.etc."xdg/gtk-4.0/settings.ini".text = settings;

    # What the settings portal answers: `programs.dconf.enable` is the compositor module's.
    programs.dconf.profiles.user.databases = [
      {
        settings."org/gnome/desktop/interface" = {
          color-scheme = scheme;
          gtk-theme = gtkTheme;
        }
        // lib.optionalAttrs cfg.icons.superpack { icon-theme = "noctua"; };
      }
    ];

    qt = lib.mkIf qtWanted {
      enable = true;
      platformTheme = "gnome";
      style = qtStyle;
    };
  };
}
