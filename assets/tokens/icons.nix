# Icon tokens — section 6.3.
#
# An icon is a token: `icon.<name>` → codepoint. But THIS file maps
# token → GLYPH NAME, never a codepoint.
#
# WHY A NAME AND NOT A CODEPOINT
# ------------------------------
# Section 6.3 names the trap: the Nerd Fonts PUA codepoints change
# between major versions. The NAMES are stable. So the codepoint is resolved
# at build time, cross-checking this file against the pinned `glyphnames.json` and
# against the real font's `cmap` — and the build FAILS if the glyph does not exist in
# both. See nix/glyphgen.nix and nix/scripts/gen-glyphdb.py.
#
# Hard-coding "" here would violate invariant 9 and would rot at the next
# Nerd Fonts update, silently.
#
# NAME SHAPE: without the `nf-` prefix. The keys of glyphnames.json are
# `fa-folder`, `cod-terminal`, `md-wifi`. The `nf-` only appears in the CSS class
# (`nf nf-fa-folder`), which the glyph browser offers for copying.
{
  # ===================================================================
  # System chrome — the two-line footer (section 4).
  #
  # We prefer Codicons (`cod-`) here: they were designed for editor UI,
  # have consistent weight among themselves and do not clash next to text.
  # ===================================================================

  # Notifications — the icon flush against the modeline's right edge (4.2).
  notification = "cod-bell";
  notification-pending = "cod-bell_dot";
  notification-muted = "cod-bell_slash";

  # Buffer state, in the left cluster.
  modified = "cod-circle_filled";
  readonly = "cod-lock";
  orphan = "cod-question";

  # Severity — used by notifications and by the minibuffer.
  error = "cod-error";
  warning = "cod-warning";
  info = "cod-info";
  success = "cod-check";

  # Minibuffer and which-key.
  search = "cod-search";
  filter = "cod-filter";
  # WHERE THE SELECTION IS in a grid of coloured content.
  #
  # In a monochrome buffer the token's colour already says it — the focused item turns
  # `state-accent` and that is that. Emoji do NOT obey: their colour is inside the
  # glyph (CBDT), and `state-accent` does not change a pixel. The symbol grid
  # had 3,944 cells in which the selection was invisible.
  #
  # A filled triangle pointing RIGHT, to the LEFT of the focused cell.
  #
  # Below does not fit: the row is 78.3 px and the emoji's ink 49.8 — but the
  # next cell line starts at 52.2, INSIDE the drawing's box. The marker
  # came out on top of the emoji, which is worse than having no marker.
  # The left fits because the horizontal step was widened along with it (see the
  # symbol browser), and there it has 15 px of slack.
  selection = "cod-triangle_right";
  # THE SIDE ARROWS CHANGE THIS VALUE.
  #
  # In a list where every row looks the same — label on the left, value on the
  # right — nothing tells apart the row the arrow adjusts from the one it ignores. In the
  # Lucide sheet there are 24 rows, 4 of them adjustable, and the key did
  # different things without warning: on some it changed the value, on others it swapped the icon, and on
  # the colour one it did nothing.
  #
  # Bidirectional on purpose: `chevron_right` alone would promise a single direction.
  adjust = "cod-arrow_both";
  prompt = "cod-chevron_right";
  ellipsis = "cod-ellipsis";
  list = "cod-list_unordered";
  history = "cod-history";

  # Generic — step 3 of section 4.2's resolution chain, when neither the table
  # nor the pixmap resolves. It has to be neutral and legible in one cell.
  generic = "cod-primitive_square";

  # ===================================================================
  # Common objects
  # ===================================================================
  folder = "cod-folder";
  folder-open = "cod-folder_opened";
  file = "cod-file";
  terminal = "cod-terminal";
  settings = "cod-settings_gear";
  key = "cod-key";
  code = "cod-code";
  mail = "cod-mail";
  play = "cod-play";
  sync = "cod-sync";
  save = "cod-save";
  trash = "cod-trash";
  edit = "cod-edit";
  copy = "cod-copy";
  add = "cod-add";
  remove = "cod-remove";
  close = "cod-close";
  window = "cod-window";

  # ===================================================================
  # Application categories — the second step of the launcher's icon chain
  # (`assets/icons/category-glyphs.nix`).
  #
  # Codicons again, and for the same reason as the rest of the chrome: they were
  # designed for editor UI and have consistent weight among themselves. A
  # list half made of Font Awesome would become a jagged column.
  # ===================================================================
  browser = "cod-browser";
  globe = "cod-globe";
  chat = "cod-comment_discussion";
  game = "cod-game";
  graphics = "cod-paintcan";
  camera = "cod-device_camera";
  video = "cod-device_camera_video";
  music = "cod-music";
  document = "cod-note";
  science = "cod-beaker";
  book = "cod-book";
  security = "cod-shield";
  system = "cod-server";
  dashboard = "cod-dashboard";
  tools = "cod-tools";
  archive = "cod-archive";
  package = "cod-package";
  calculator = "fa-calculator";

  # ===================================================================
  # NETWORK — the seventh view of the expansion (LEADER r r).
  #
  # Four signal steps, and not a percentage: the exact number wobbles
  # every second and nobody decides anything with "67%". What one decides with
  # signal is "can I connect from here?", and four steps answer that.
  #
  # Material Design (`md-`) here, and not Codicon: the Codicons do not have the
  # signal-bar family, and drawing four steps out of what is left
  # would give a scale that does not look like a scale.
  # ===================================================================
  # NAMES BY WORD, and not `wifi-0`..`wifi-4`.
  #
  # The WIT `icone` enum is GENERATED from here, and in WIT every segment of an
  # identifier has to begin with a LETTER: `wifi-0` is rejected with
  # "invalid character in identifier '0'" — three derivations later, inside
  # a buffer that has nothing to do with icons. The generator now rejects that
  # here, with the token's name in the message (see nix/scripts/gen-tokens.py).
  wifi-none = "md-wifi_strength_outline";
  wifi-weak = "md-wifi_strength_1";
  wifi-fair = "md-wifi_strength_2";
  wifi-good = "md-wifi_strength_3";
  wifi-strong = "md-wifi_strength_4";
  wifi-off = "md-wifi_off";
  ethernet = "md-ethernet";
  # The network asks for a key. A CLOSED padlock and only that: an open network does not get
  # an open padlock, it gets the absence of a padlock — the column already says so by being
  # empty, and one more icon on every open row would be noise.
  network-secured = "md-lock";

  # ===================================================================
  # System and hardware — the modeline's context segments
  # ===================================================================
  wifi = "md-wifi";
  bluetooth = "fa-bluetooth";
  volume = "md-volume_high";
  battery = "md-battery";
  monitor = "md-monitor";
  keyboard = "md-keyboard";
  cpu = "md-chip";
  memory = "md-memory";
  disk = "md-harddisk";
  power = "md-power";

  # ===================================================================
  # Development
  # ===================================================================
  git-branch = "oct-git_branch";
  git-commit = "oct-git_commit";
  rust = "md-language_rust";
  nix = "md-nix";
  container = "md-docker";

  # ===================================================================
  # Applications — they feed the base tray table (assets/icons/).
  #
  # Brands change names in Nerd Fonts more often than the rest;
  # if the build fails pointing here, it is probably that.
  # ===================================================================
  app-firefox = "fa-firefox";
  app-chrome = "fa-chrome";
  app-telegram = "fa-telegram";
  app-discord = "fa-discord";
  app-slack = "fa-slack";
  app-spotify = "fa-spotify";
  app-signal = "md-signal";
}
