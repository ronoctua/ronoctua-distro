# Base `app id -> glyph` table — section 4.2.
#
# The first step of the tray icon resolution chain, and the preferred one: a
# glyph from the system font, which is what keeps the visual coherence. If there is no
# entry here, it falls back to a pixmap (step 2) and then to the generic glyph (3).
#
# THREE LAYERS, in this order:
#
#   1. this table             declarative, versioned, the default
#   2. ~/.config/ronoctua/tray-glyphs.toml
#                             the user's override, read at RUNTIME, with
#                             hot reload through inotify. NO rebuild — if
#                             customising required a nixos-rebuild, section
#                             4.2's "easy" requirement would not be
#                             satisfied.
#   3. glyph browser          a command that writes into layer 2. It is the
#                             main path: browse, choose, assign, effective
#                             immediately. It arrives in milestone 7.
#
# The values are TOKENS defined in assets/tokens/icons.nix, never glyph names and
# much less codepoints. The build fails if the token does not exist there, and
# also if the token's glyph does not exist in the font (invariant 9, section 6.3).
{
  # --- browsers ------------------------------------------------------
  "firefox" = "app-firefox";
  "org.mozilla.firefox" = "app-firefox";
  "chromium-browser" = "app-chrome";
  "google-chrome" = "app-chrome";
  "Chromium" = "app-chrome";

  # --- communication -------------------------------------------------
  "org.telegram.desktop" = "app-telegram";
  "discord" = "app-discord";
  "Slack" = "app-slack";
  "thunderbird" = "mail";
  "org.signal.Signal" = "app-signal";

  # --- media ---------------------------------------------------------
  "spotify" = "app-spotify";
  "mpv" = "play";
  "org.videolan.VLC" = "play";

  # --- system --------------------------------------------------------
  "nm-applet" = "wifi";
  "blueman" = "bluetooth";
  "blueman-applet" = "bluetooth";
  "pavucontrol" = "volume";
  "org.freedesktop.UPower" = "battery";
  "syncthing" = "sync";
  "syncthingtray" = "sync";
  "keepassxc" = "key";
  "org.keepassxc.KeePassXC" = "key";

  # --- development ---------------------------------------------------
  "com.mitchellh.ghostty" = "terminal";
  "org.gnome.Nautilus" = "folder";
  "code" = "code";
  "docker" = "container";
  "org.gnome.Console" = "terminal";
}
