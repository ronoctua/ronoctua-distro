# `Categories= -> glyph` table — the SECOND step of the app launcher's icon
# chain.
#
# The whole chain, in the order `apps::icon` walks it:
#
#   1. `tray-glyphs.nix`, by desktop id       the app has a glyph of its own
#   2. THIS table, by the first `Categories=` that matches
#   3. superpackage pixmap, at another speed  (the app's own icon)
#   4. `icon.generic`                         never "nothing"
#
# WHY A LIST, AND NOT AN ATTRSET
# ------------------------------
# Because the ORDER decides. A browser declares `Categories=Network;WebBrowser`,
# and both match: with no order, `Network` would win and EVERY browser would turn into the
# network globe. An attrset is ordered by KEY, and `Network` comes before
# `WebBrowser` in the alphabet — the defect would be exactly that, and it would come
# for free. With a list, the author decides, and the most specific comes first.
#
# The values are TOKENS from `assets/tokens/icons.nix`, never glyph names and
# much less codepoints (invariant 9). The build fails if the token does not exist
# there, and `glyphgen` already fails if the token's glyph does not exist in the font.
#
# The categories are those of the freedesktop menu specification. The "additional" ones
# (`WebBrowser`, `TerminalEmulator`, …) only count alongside a main one,
# and that is precisely why they come first here: when they appear, they
# say more.
[
  # --- the additional ones, which are the specific ones -----------------
  {
    category = "WebBrowser";
    token = "browser";
  }
  {
    category = "Email";
    token = "mail";
  }
  {
    category = "TerminalEmulator";
    token = "terminal";
  }
  {
    category = "FileManager";
    token = "folder";
  }
  {
    category = "TextEditor";
    token = "edit";
  }
  {
    category = "IDE";
    token = "code";
  }
  {
    category = "InstantMessaging";
    token = "chat";
  }
  {
    category = "IRCClient";
    token = "chat";
  }
  {
    category = "Chat";
    token = "chat";
  }
  {
    category = "Archiving";
    token = "archive";
  }
  {
    category = "PackageManager";
    token = "package";
  }
  {
    category = "Monitor";
    token = "dashboard";
  }
  {
    category = "Security";
    token = "security";
  }
  {
    category = "Photography";
    token = "camera";
  }
  {
    category = "Music";
    token = "music";
  }
  {
    category = "Player";
    token = "music";
  }
  {
    category = "Calculator";
    token = "calculator";
  }

  # --- the main ones ---------------------------------------------------
  {
    category = "Development";
    token = "code";
  }
  {
    category = "Game";
    token = "game";
  }
  {
    category = "Graphics";
    token = "graphics";
  }
  {
    category = "Video";
    token = "video";
  }
  # `AudioVideo` is the main one for everything that plays media; `Audio` and `Video` are
  # additional to it. Since `Video` has already gone by above, what is left here is sound.
  {
    category = "Audio";
    token = "music";
  }
  {
    category = "AudioVideo";
    token = "music";
  }
  {
    category = "Office";
    token = "document";
  }
  {
    category = "Science";
    token = "science";
  }
  {
    category = "Education";
    token = "book";
  }
  {
    category = "Settings";
    token = "settings";
  }
  {
    category = "Network";
    token = "globe";
  }
  {
    category = "System";
    token = "system";
  }
  {
    category = "Utility";
    token = "tools";
  }
]
