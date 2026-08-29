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

  # Notifications — the icon at the modeline's right edge (4.2).
  notification = "cod-bell";
  notification-pending = "cod-bell_dot";
  notification-muted = "cod-bell_slash";

  # THE TWO THINGS ONE DOES TO A NOTIFICATION WITH THE MOUSE.
  #
  # It was a hollow circle, and it was reported from the screen as not reading
  # like anything. A circle is a SHAPE, not a verb: it says "here is a control"
  # and leaves what it does to be guessed.
  #
  # An OPEN ENVELOPE is the drawing everybody already reads as "read", and a bin
  # is the drawing everybody already reads as "gone". Neither is a circle, and
  # neither needs a legend.
  notification-mark-read = "md-email_open_outline";
  notification-delete = "cod-trash";

  # HOW LONG THE ONE ON SCREEN HAS LEFT — a pie in ONE cell, eight steps.
  #
  # Not a bar: a bar of a few cells next to a sentence reads as a second piece
  # of content, and this has to be answerable with the corner of the eye and
  # forgettable. Not a number of seconds either — nobody decides anything with
  # "4 s", and it would tick in the middle of a line being read.
  #
  # The names count what is LEFT, in eighths: `countdown-eight` is a full
  # circle and `countdown-one` is the last sliver before it goes. Words and not
  # digits because every stretch of a WIT identifier begins with a letter —
  # `countdown-8` is refused three derivations away, inside a buffer.
  # THE TWELVE FRAMES OF "SOMETHING IS RUNNING", for the modeline's update mark.
  #
  # AN UPDATE, BREATHING. The arrow in a circle is what an update is drawn as
  # everywhere somebody has drawn one; the two frames are the SAME circle,
  # filled and hollow, alternating once a second on the clock the footer already
  # beats.
  #
  # WHY IT STILL MOVES: a still glyph and a hung update look identical, and an
  # update is the one thing here that legitimately takes twenty minutes.
  #
  # WHY TWO FRAMES OF ONE SHAPE and not a rotation: `chrome-icons-match-the-text`
  # measures every one of these against the digit beside it, because "an
  # animation whose frames are 20% shorter than the text next to it does not
  # read as one drawing moving, it reads as the line twitching". Filled against
  # hollow is the same circle at the same size — the only thing that changes is
  # the ink, so it breathes instead of twitching. The font's rotating families
  # are the clock, which is out, and the pie, which is spoken for.
  #
  # NOT the `countdown-*` pie, and the two must not be confused: the pie is how
  # much time is LEFT and empties towards a deadline. This is something running
  # whose end nobody knows. Same subject, opposite question, so they are
  # different drawings.
  #
  # WORDS AND NOT DIGITS in the token's name, and it is not taste: a token
  # becomes a WIT identifier, and a segment that begins with a digit is not one.
  working-one = "md-arrow_up_bold_circle";
  working-two = "md-arrow_up_bold_circle_outline";

  countdown-eight = "md-circle_slice_8";
  countdown-seven = "md-circle_slice_7";
  countdown-six = "md-circle_slice_6";
  countdown-five = "md-circle_slice_5";
  countdown-four = "md-circle_slice_4";
  countdown-three = "md-circle_slice_3";
  countdown-two = "md-circle_slice_2";
  countdown-one = "md-circle_slice_1";

  # TRANSFERS — the queue of everything that takes a while (LEADER t).
  #
  # The segment sits next to the notification's, which is why this one is a
  # Codicon too: two glyphs from different families side by side in the same
  # line read as two different weights, and the modeline is where that shows
  # most.
  transfer = "cod-arrow_swap";

  # THE BAR, and it is two glyphs because a cell grid has no stretching.
  #
  # A progress bar here is not a rectangle somebody resizes: the cell is the
  # unit, so the bar is N cells of one glyph followed by the rest of the other.
  # Invariant 9 reaches the codepoint, which is why these are tokens and not two
  # characters written into the Rust.
  progress-done = "md-rectangle";
  progress-left = "md-rectangle_outline";

  # The stages. `done` and `failed` deliberately reuse `success` and `error`
  # above: the same drawing in two roles is already this file's rule, and a
  # second check mark would be a second thing to keep in step.
  transfer-weighing = "md-progress_clock";
  transfer-running = "cod-sync";
  transfer-paused = "cod-debug_pause";
  transfer-cancelled = "cod-circle_slash";

  # THE ENGINES, one drawing each.
  #
  # A row says WHO is moving before it says how far along it is: a paste and a
  # torrent that look alike are two rows one has to read to tell apart, and the
  # queue's whole point is being able to glance at it. `local` reuses the disk's
  # drawing and `media` the camera's — the same meaning in two places gets the
  # same drawing, which is this file's rule.
  engine-local = "cod-save";
  engine-fetch = "cod-cloud_download";
  engine-media = "cod-device_camera_video";
  # The magnet, because it is what a person clicks to start one — the drawing
  # they already associate with the thing.
  engine-torrent = "cod-magnet";

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
  # FILE TYPES — the eighth view of the expansion (LEADER .), and the window.
  #
  # These are the OTHER end of `assets/icons/mime-glyphs.nix`: freedesktop says
  # what a type is generically (`x-office-spreadsheet`), and that table says
  # which of these draws it. Seventeen generic names cover all 1,232 types the
  # database knows, so the table stays short and the coverage stays total.
  #
  # Codicons where they exist, for the reason the rest of the chrome gives —
  # a column half made of Font Awesome comes out jagged. Where the family has
  # no glyph for the idea (a font, a disc, a pipe), Material fills in.
  # ===================================================================
  # A picture. `file_media` and not a camera: what is being named is the FILE,
  # and a camera is where a photograph came from, not what it is now.
  image = "cod-file_media";
  # A program. `oct-file_binary` says exactly that, and the two Codicon
  # candidates (`run_all`, `gear`) both say "settings" to the eye.
  executable = "oct-file_binary";
  spreadsheet = "cod-table";
  presentation = "md-presentation";
  font = "md-format_font";
  disc = "md-disc";
  pdf = "cod-file_pdf";
  # A symlink, in the LISTING — not a git branch. `git-branch` was the nearest
  # thing that already existed, and reaching for it is how a person learns to
  # distrust an icon: the same drawing in two roles means neither of them.
  link = "cod-link";
  pipe = "md-pipe";
  socket = "md-console_network";

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
  # The WIT `icon` enum is GENERATED from here, and in WIT every segment of an
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
  # THE BRAIN'S OWN MARK, the one glyph on its line, in the left margin.
  #
  # A line that begins with a sentence and a line that begins with a mark are
  # two shapes — `band::generic_glyph` argues it for notifications, and the
  # same holds here: this line appears without being asked for, and the mark is
  # what says WHOSE it is before a word is read.
  brain = "md-brain";
  memory = "md-memory";
  disk = "md-harddisk";
  # A DRIVE ONE CAN PULL OUT, in the file manager's sidebar. It is a different
  # DRAWING and not a different colour, because the difference between an
  # internal disk and a stick is the first thing one looks for in that list —
  # and colour there is already carrying "is it mounted".
  disk-removable = "md-usb_flash_drive";
  # Unmounting, on the sidebar's row. Material rather than Codicon for the
  # reason the file's other blocks give: the family has no eject glyph, and
  # drawing one out of what is left gives a symbol that means nothing.
  eject = "md-eject";
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
