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
  notification-pending = "cod-bell_dot";

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

  # THE ROW THAT ANSWERS `RET`, in the three views that are a list of a label
  # and a value — network, power and Bluetooth.
  #
  # WHY A GLYPH AND NOT A COLOUR. In those views a fact and a control were
  # drawn exactly alike, and the owner asked for the difference. Colour was the
  # other candidate and it is the worse one: the accent is already spent on the
  # cursor, the remaining greys are a weight difference that disappears at a
  # glance, and a person who does not separate tones well would be left with
  # nothing at all. A mark in front of the row says it in any palette.
  #
  # A CODICON, like the rest of the chrome above: it was drawn for editor
  # interface, it is the same weight as the text beside it, and it is quiet —
  # the row is what one reads, the mark only says the row is a door.
  row-acts = "cod-chevron_right";

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
  # THE LINE THAT DOES NOT FIT, and the way to see the rest of it.
  #
  # The minibuffer's line is one line, and a message longer than the screen
  # used to be cut with nothing to do about it. These two are the control on
  # its right edge: `expand` while the line is folded, `collapse` while it is
  # open. Two tokens and not one turned upside down, because a glyph is a
  # DRAWING and the font is where a drawing comes from — rotating one in the
  # renderer would be a second way of making a picture.
  #
  # Chevrons and not a triangle: `selection` is the filled triangle and it
  # already means "this row is marked", and the pair has to read as a
  # direction rather than as a state.
  expand = "cod-chevron_down";
  collapse = "cod-chevron_up";
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
  # THE PREVIEW'S TRANSPORT — the two things one does to a film in the panel.
  #
  # A MATCHED PAIR, from one family, and that is the whole point of changing
  # them. They were `cod-play` and `cod-debug_pause`, which are Codicons but not
  # a pair: the second is a DEBUGGER's pause, drawn with heavier bars, and the
  # owner saw it beside the other two glyphs — *"o ícone de pausa é maior que o
  # resto"*. Two glyphs that alternate in the same cell have to be the same
  # weight, or the row twitches every time the film is held.
  #
  # `md-` because Material has both halves designed together; the transfer
  # queue's `transfer-paused` keeps its Codicon, and that is not a divergence —
  # it is a STATE in a list, not a button one presses.
  play = "md-play";
  pause = "md-pause";
  # And sound OFF, which is what a preview starts as: a panel that speaks when
  # a cursor passes over a file is a panel one turns off.
  volume-off = "md-volume_off";
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
  # A SHAPE — `.stl`, `.3mf`. Freedesktop calls both of them
  # `image-x-generic`, which is correct and useless: in a column of thirty rows
  # a thing one PRINTS is indistinguishable from a photograph, and the two are
  # opened by entirely different programs. `md-cube_outline` and not
  # `md-printer_3d`: what the row names is the FILE, and a printer is where the
  # file might be going, not what it is — the same argument `image` makes one
  # screen up against a camera.
  mesh = "md-cube_outline";

  # ===================================================================
  # NETWORK — the seventh view of the expansion (LEADER s n n).
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
  #
  # AND THE `network_strength` FAMILY, not `wifi_strength`: the two draw the
  # same five steps, and only one of them is the height of the text beside
  # it. The arc family's ink is 77,5% of the digit `0` — the ladder used to
  # sit in a column of its own, where that passed unnoticed, and it now sits
  # shoulder to shoulder with the network's name in the row's label. The
  # triangle family measures 97,6%, and `chrome-icons-match-the-text` is what
  # says so.
  wifi-none = "md-network_strength_outline";
  wifi-weak = "md-network_strength_1";
  wifi-fair = "md-network_strength_2";
  wifi-good = "md-network_strength_3";
  wifi-strong = "md-network_strength_4";
  wifi-off = "md-network_strength_off";
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

  # THE BATTERY'S LADDER — five steps, by WORD, the Wi-Fi block's reason
  # (a WIT identifier segment starts with a letter). Material, because it is
  # the one family with the ladder drawn as a ladder, and its charging
  # variants carry the bolt on the same drawing. The modeline draws one of
  # these only while there is something to say: nothing at 99% and above,
  # nothing at `FullyCharged`, and the alert glyph in the danger colour at
  # 15% and below.
  # ---- The way out ---------------------------------------------------
  # THREE MARKS, and each says something the WI-FI BARS cannot.
  #
  # The bars say how loud the router is; these say whether anything is out
  # there. `no-internet` is a link that leads nowhere — the hotel's portal,
  # the router with no uplink — which is the case that looks fine and is not.
  # `airplane` is somebody's own decision, and is drawn so that "nothing
  # works" has an answer on screen. `vpn` is a shield and not `md-vpn`: that
  # drawing is wide and flat, 66% of the digit's ink at the icon size, and
  # `chrome-icons-match-the-text` is what said so.
  no-internet = "md-web_off";
  airplane = "md-airplane";
  vpn = "md-shield_lock";
  # THE OTHER TWO TUNNELS. There is no Tailscale glyph in any of the six
  # families — `linux-tails` is the Tails OS and not this — so the mark is the
  # four-dot grid its own logo is drawn from, and it earns its place by being
  # unlike every other mark on this line. `linux-tor` IS the onion, and it is
  # the one case where the brand's own mark is in the font.
  # THE MARK THIS REPOSITORY DRAWS — see `assets/icons/marks.nix`.
  #
  # It was `md-dots_grid`, a UNIFORM three-by-three of dots, and the owner read
  # it off the screen as a menu icon: which is what a uniform dot grid is
  # everywhere else in a desktop. Tailscale's own mark is not uniform — the
  # middle row and the dot under its centre are solid, the other five are
  # rings — and that is the whole of what stops it reading as a menu. No face
  # in this system has it, so it is drawn into one of ours.
  tailscale = "ours-tailscale";
  tor = "linux-tor";

  # ---- Bluetooth -----------------------------------------------------
  # THE DEVICE KINDS, for the modeline's marks and the list's rows.
  #
  # A CONNECTED DEVICE IS DRAWN AS WHAT IT IS — headphones as headphones, a
  # keyboard as a keyboard — because that is the only thing about it one
  # reads at a glance: which of the four things on my desk is this. The kind
  # comes from the device's own class or appearance (`Kind::of`), never from
  # its name.
  # THE DRAWING IS CHOSEN BY MEASUREMENT, not by name: `md-gamepad_variant`,
  # `md-laptop`, `md-tablet`, `md-access_point` and `md-television` all mean
  # the right thing and all draw a SLIVER — 53% to 80% of the digit's ink at
  # the icon size, which reads as a smaller kind of thing beside the `NN%`.
  # These are their in-band siblings, and `chrome-icons-match-the-text` is
  # what found the difference.
  bluetooth-device-headphones = "md-headphones";
  bluetooth-device-headset = "md-headset";
  bluetooth-device-speaker = "md-speaker";
  # THE ONE EXCEPTION, and it is in the check by name: a keyboard is a WIDE
  # drawing. Its ink already spans the cell, so no size brings it to the
  # digit's height without it touching the character beside it — and there is
  # no square keyboard in the whole font (the nine in band are caps-lock, an
  # `x`, a cog or the ⌘ key: the wrong thing said clearly).
  bluetooth-device-keyboard = "md-keyboard";
  bluetooth-device-mouse = "md-mouse";
  bluetooth-device-gamepad = "md-gamepad";
  bluetooth-device-tablet = "md-tablet_android";
  bluetooth-device-phone = "md-cellphone";
  bluetooth-device-computer = "md-desktop_tower";
  bluetooth-device-watch = "md-watch";
  bluetooth-device-camera = "md-camera";
  bluetooth-device-printer = "md-printer";
  bluetooth-device-network = "md-router_wireless";
  bluetooth-device-display = "md-television_classic";
  # The radio itself, for the list's first row and for a device of no kind.
  bluetooth-device-unknown = "md-bluetooth";
  bluetooth-connected = "md-bluetooth_connect";
  bluetooth-off = "md-bluetooth_off";
  # ---- Battery -------------------------------------------------------

  # A VERTICAL DRAWING, and the reason the chrome size is measured per glyph:
  # at the icon size these stood 48% above the digit. The generator brings
  # them down to it (`chrome_size` in `gen-glyphdb.py`); the charging ones are
  # wider drawings and already sit at the digit's height.
  #
  # FOUR STEPS AND NOT FIVE, unplugged. `battery-empty` was the fifth and
  # nothing could draw it: the bottom rung is 0-10% and the danger colour
  # takes over at 15%, so a battery that low and off the charger is always
  # the alert glyph. A token no path reaches is a token the theme generator
  # still has to solve, and a comment claiming it is drawn is a comment that
  # sends the next reader looking for the screen it is on.
  battery-low = "md-battery_20";
  battery-half = "md-battery_50";
  battery-high = "md-battery_80";
  battery-full = "md-battery";
  battery-charging-empty = "md-battery_charging_10";
  battery-charging-low = "md-battery_charging_20";
  battery-charging-half = "md-battery_charging_50";
  battery-charging-high = "md-battery_charging_80";
  battery-charging-full = "md-battery_charging_100";
  battery-danger = "md-battery_alert";
  # THE PROFILE'S MARK, only when the machine is NOT at its top profile: a
  # leaf for power-saver, the middle gauge for balanced. `performance` never
  # reaches the modeline — the distro's default says nothing — and its glyph
  # is drawn in the VIEW's own rows, where all three sit one under the other
  # and each says which it is. That was already written here and was not true
  # of any code: the rows drew a tick and nothing else.
  profile-power-saver = "md-leaf";
  profile-balanced = "md-speedometer_medium";
  profile-performance = "md-speedometer";

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
