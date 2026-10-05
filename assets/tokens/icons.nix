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
  # AND THE SAME CONTROL THE OTHER WAY. A row that is read keeps the target and
  # changes the drawing: the envelope with the unread mark, which is the verb
  # this one has and the open one does not. Marking read used to be a one-way
  # door — the row simply lost its envelope — so a slip of the hand was read for
  # fifteen days. One control, two drawings, and the drawing says which way it
  # goes.
  #
  # `md-email_outline`, the plain closed envelope, was the first choice and the
  # scale check refused it: it draws at 78% of the digit's height against the
  # open one's 95%, so the pair would have read as two different sizes of the
  # same idea.
  notification-mark-read = "md-email_open_outline";
  # OPEN IT WHOLE, in the manager: beside the envelope on the band, and what
  # the band cannot hold is what the open record is for. An arrow out of a box
  # into a larger one, which is the verb and not the place.
  notification-open = "md-arrow_expand";
  notification-mark-unread = "md-email_mark_as_unread";
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
  # The globe, because what is being fetched is a PLACE — and it is the same
  # drawing the geography view puts beside a region, which is this file's rule:
  # the same meaning gets the same drawing.
  engine-atlas = "cod-globe";

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
  # THE DOT BETWEEN ONE SHORTCUT AND THE NEXT, for a BUFFER.
  #
  # The minibuffer already draws one, and it draws it from `mark.separator` —
  # a codepoint, which is a token value the wall does not let a component name
  # (invariants 9 and 11). So a buffer teaching its own keys had nothing to put
  # between two pairs but empty cells, and the owner read the result beside the
  # file manager's line and said what it is: the same control, not looking like
  # the same control.
  #
  # An ICON is the shape that crosses. It resolves to the same dot the hint
  # line uses, and a buffer names the token rather than the character — which
  # is the whole of what the invariant asks.
  separator = "cod-circle_small_filled";
  list = "cod-list_unordered";
  history = "cod-history";

  # THE CLIPBOARD'S PIN. It was the literal `"*"` in `surface.rs`, which is a
  # codepoint written by hand into a view — invariant 9 with no exception
  # claimed for it, and the only glyph in the tree that was.
  pinned = "cod-pinned";
  # AND THE TWO THE CLIPBOARD'S SYNC DRAWS on the modeline, which are the same
  # pair Syncthing's mark uses: a thing travelling, and a thing that is not.
  clip-on = "md-clipboard_check_outline";
  clip-off = "md-clipboard_off_outline";
  # AND THE SHELL HISTORY'S PAIR. `history` above is the file manager's — a
  # list of where you have been — and this is about whether the history is
  # CROSSING THE NETWORK, which is a different fact and owes a different glyph.
  shell-history-on = "md-history";
  shell-history-off = "md-cloud_off_outline";

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
  # THE PREVIEW'S OTHER BUTTON: give it the whole region, and give the region
  # back. `C-w` does the same thing and always has; the owner asked for the
  # gesture to exist for the mouse too — *"quero que haja um botão de expandir
  # (e de reduzir, quando estiver expandindo) para o preview"*.
  #
  # A MATCHED PAIR from the same family as `play`/`pause` above, and for the
  # reason written there: the two alternate in ONE cell, so a difference in
  # weight between them is a row that twitches every time it is pressed.
  # `md-fullscreen` is U+F0293 and `md-fullscreen_exit` U+F0294 — neighbours,
  # drawn together.
  #
  # NOT the `expand`/`collapse` chevrons: those are the drawer's, and the
  # owner's own words reserve them — *"não quero mais o chevron, pois isso é
  # utilizado em elementos de descolapsar/colapsar"*. This is not a drawer.
  theatre = "md-fullscreen";
  theatre-off = "md-fullscreen_exit";

  # THE LISTING'S OTHER SHAPE: one thing per line, or all of them side by side
  # in thumbnails. The owner asked for the mouse to reach it — *"será possível
  # ativá-lo e desativá-lo por meio de um glifo clicável (que muda de acordo
  # com o modo)"* — and it sits at the right-hand end of the path's line.
  #
  # A MATCHED PAIR, the third in this file, and the argument is the one written
  # at `play`/`pause` and again at `theatre`: the two alternate in ONE cell, so
  # a difference in weight between them is a glyph that twitches every time the
  # mode changes. `md-view_list` and `md-view_grid` are the same family and the
  # same designer.
  #
  # THE GLYPH SAYS WHERE IT GOES, not where it is — `theatre_field` already
  # works that way, and a control that draws its current state is a control one
  # has to press to find out what it does.
  #
  # NOT `md-dots_grid`, which is the obvious choice and is taken twice over: it
  # is the reference glyph `nix/marksgen.nix` measures our own marks against,
  # and `assets/icons/marks.nix` records the owner reading a uniform grid of
  # dots off the screen as a MENU, which is what it is everywhere else on a
  # desktop.
  view-list = "md-view_list";
  view-grid = "md-view_grid";

  # HOW BIG THE THUMBNAILS ARE, as a glyph one clicks to ring through. The owner
  # asked for it beside the one above — *"quero que apareça um novo glifo ao lado
  # esquerdo do glifo trocador de modos, e esse glifo a cada clique altera as
  # thumbnails para um outro tamanho"*.
  #
  # A DESIGNED TRIO from the same family as the pair above, and it is the same
  # argument a third time: three glyphs that take turns in ONE cell must be the
  # same weight, or the row twitches at every press. `md-image_size_select_*` are
  # neighbours in Material's own sheet — a picture frame with a small, a medium
  # and a large selection inside it — so the drawing itself says which of three
  # it is, which no single "resize" glyph could.
  view-small = "md-image_size_select_small";
  view-medium = "md-image_size_select_actual";
  view-large = "md-image_size_select_large";
  sync = "cod-sync";

  # SYNCTHING'S TWO, and they are neighbours in one face on purpose.
  #
  # `md-sync` is U+F04E6 and `md-sync_off` is U+F04E8 — the same designer, the
  # same metrics. The sound view's volume ladder was dropped because two faces
  # measured 15 px against 22 and the modeline twitched at every step; only one
  # of these two is ever drawn at a time, and even so they are a matched pair.
  #
  # NOT the `sync` above, which the toolkit row and the update window already
  # draw: changing that one to make this pair match would have redrawn two
  # screens nobody asked about.
  syncthing = "md-sync";
  syncthing-off = "md-sync_off";
  save = "cod-save";
  trash = "cod-trash";
  edit = "cod-edit";
  copy = "cod-copy";
  add = "cod-add";
  remove = "cod-remove";
  close = "cod-close";

  # THE ON-SCREEN KEYBOARD'S KEYS THAT HAVE NO CHARACTER (`LEADER m k`). The
  # rest of its legends are not tokens at all: they are what the chosen
  # layout types, read from xkb (`crate::osk`).
  osk-backspace = "md-backspace_outline";
  osk-return = "md-keyboard_return";
  osk-shift = "md-apple_keyboard_shift";
  osk-caps = "md-apple_keyboard_caps";
  osk-tab = "md-keyboard_tab";
  osk-settings = "md-keyboard_settings";
  osk-hide = "md-keyboard_close";
  osk-language = "cod-globe";
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

  # SILENCE, and it is the modeline's mark and nothing else.
  #
  # `md-volume_off` above is the film preview's "this panel does not speak";
  # this one is "the machine does not". Two names because they are two facts,
  # and the day one moves the other must not follow it.
  #
  # THERE IS NO LADDER, and that is measured rather than chosen. A quiet, a
  # medium and a loud speaker exist in Material and the generator sizes each
  # glyph against the digit beside it: `md-volume_low` came out at **15 px**,
  # `md-volume_medium` at **19** and `md-volume_high` at **22**. Three glyphs
  # taking turns in one cell would resize the row every time somebody touched
  # the volume — the same twitch the `play`/`pause` comment above exists to
  # avoid, and there it was only two glyphs. So a row draws ONE glyph and the
  # BAR says how loud, and the mark is present or absent rather than one of
  # three.
  volume-mute = "md-volume_mute";

  # THE MICROPHONE, and it is a PRIVACY mark before it is an audio one.
  #
  # On the modeline it is drawn whenever anything holds a capture stream —
  # derived from the graph, never guessed — so what it says is "something is
  # listening", and that is worth a cell on its own. Present or absent, again:
  # a second glyph for "not listening" would be a mark that is always there,
  # which the modeline does not do at rest.
  microphone = "md-microphone";

  # A SCREEN'S BRIGHTNESS, on the line that flashes when the brightness keys
  # are pressed. One glyph and a bar, for the volume's reason just above.
  brightness = "md-brightness_6";

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

  # THE TWO KINDS OF TASK, and they are ONE DRAWING APART on purpose.
  #
  # `md-repeat` is an arrow that comes back round; `md-repeat_once` is the same
  # arrow with a `1` in it. A person who has read one has read the other, which
  # is the whole job: the list has two kinds of row and nothing in the words
  # tells them apart — both are a name, a moment and a standing.
  #
  # Two glyphs from different families would have been two things to learn, and
  # the difference between them would have read as a difference of importance
  # rather than of kind. A colour says it again (`text.once`), because a glyph
  # two cells wide is a small thing to hang a distinction on.
  task-routine = "md-repeat";
  task-once = "md-repeat_once";

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

  # THE POINTER SETTINGS (`LEADER s i`). The cursor's glyph is drawn in the
  # colour that cursor wears (`cursor.fill.*`), so a cursor's heading on the
  # screen is the same colour as the arrow on the desktop — which is how a
  # person tells which of the listed cursors is the one in their hand.
  pointers-cursor = "md-cursor_default";
  pointers-mouse = "md-mouse";
  pointers-touchpad = "md-gesture_two_tap";
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

  # THE CAMERA STUDIO (`LEADER m c`, `LEADER s c`).
  #
  # The app's controls are drawn OVER the picture, one glyph per control and
  # every one of them a key too (invariant 19): a camera's face is the whole
  # window, so a control has no room for a word and a word over a face would
  # be read before the face. Each glyph is what the control DOES, not what the
  # app is: `camera_iris` takes a photo, `record_rec` records.
  studio-photo = "md-camera_iris";
  studio-record = "md-record_rec";
  studio-stop = "md-stop";
  studio-effect = "md-auto_fix";
  studio-background = "md-blur";
  studio-camera-turn = "md-camera_switch";
  studio-mirror = "md-flip_horizontal";
  studio-scene = "md-view_dashboard";
  studio-mixer = "md-tune_vertical";
  studio-fit = "md-fit_to_screen";
  studio-fill = "md-fullscreen";
  studio-pin = "md-pin";
  studio-unpin = "md-pin_off";
  studio-virtual = "md-webcam";
  studio-stream = "md-broadcast";
  studio-script = "md-script_text_play";
  studio-next = "md-skip_next";
  studio-settings = "cod-settings_gear";
  # THE PRIVACY MARK on the modeline: SOMETHING HAS A CAMERA OPEN — any
  # program, found in `/proc`, ours included. Present or absent, like the
  # microphone's.
  camera-in-use = "md-webcam";
  # And the one that says the studio is RECORDING or STREAMING, drawn in the
  # danger tone: a mark that is also a warning.
  recording = "md-record_rec";
  # HAND GESTURES ARE ON (`LEADER m g`): the camera is reading hands. Just an
  # OPEN HAND, the owner's choice — not the waving one, whose lines of motion
  # are noise at one cell, nor `md-gesture_tap`, the notification's "has
  # actions".
  gestures = "md-hand_back_right";
  # THE SCREEN IS CUT by the resizer (`LEADER / r`): the corner grip, which is
  # what gets dragged.
  resizer = "md-resize_bottom_right";
  # THE DRAWER HOLDS SOMETHING: windows put away (the compositor's
  # scratchpad), which no workspace shows and no focus reaches — this mark is
  # where the pointer finds them. Present or absent, like every mark here.
  drawer = "md-archive";
  # A TRAIL EXISTS: lists of windows somebody marked to walk between. A path
  # of stops, which is what a trail is.
  trail = "md-map_marker_path";
  # WINDOWS ARE SELECTED, to be acted on together: a set of boxes ticked.
  windows-selected = "md-checkbox_multiple_marked_outline";

  # THE PLAYER (`LEADER p`, the column on the right, the modeline's mark).
  #
  # The transport is drawn as glyphs under the picture, one cell each and every
  # one a key too (invariant 19). Material, because the family has every half
  # of every pair designed together: `repeat`, `repeat_once` and `repeat_off`
  # are one drawing in three states, and a ring drawn from three families would
  # twitch at every press.
  # THE MODELINE MARK, and a PLAIN triangle: the outlined circle drew the
  # triangle a third of the cell, and at the modeline's size the owner could
  # not read it as "playing". It pairs with `pause`, which never had a ring.
  player = "md-play";
  player-previous = "md-skip_previous";
  player-next = "md-skip_next";
  player-stop = "md-stop";
  player-shuffle = "md-shuffle_variant";
  player-shuffle-off = "md-shuffle_disabled";
  player-repeat = "md-repeat";
  player-repeat-one = "md-repeat_once";
  player-repeat-off = "md-repeat_off";
  # THE PICTURE ALONE in the door, and the door filling the screen. Only-
  # the-picture is a switch, lit while it is on, like shuffle and repeat;
  # filling the screen is the file manager's `theatre` pair, because it is
  # the same gesture — the glyph says where the button GOES.
  player-only = "md-fit_to_screen";
  # EMPTY THE QUEUE, the glyph beside its title: a list with a line through.
  player-clear-queue = "md-playlist_remove";
  # THE SMALL WINDOW IN THE CORNER — picture-in-picture —, drawn as what it
  # does: a frame with a smaller one in its bottom-right corner, which is
  # where the window goes. A switch, lit while it is on.
  player-pip = "md-picture_in_picture_bottom_right";
  player-volume = "md-volume_high";
  player-subtitles = "md-subtitles_outline";
  player-lyrics = "md-music_note";
  player-queue = "md-playlist_music";
  player-follow = "md-television_play";
  # THE DOOR IT SWAPS TO (`player-swap`), drawn as where it goes: the column
  # in a window, the window in the column and in the footer.
  player-column = "md-dock_right";
  player-window = "md-dock_window";
  # THE VOLUME A STEP UP AND DOWN, on the column's bar: the speaker with a
  # plus and with a minus, one drawing in two states — the mute between
  # them is the same speaker.
  player-louder = "md-volume_plus";
  player-quieter = "md-volume_minus";
  # THE MEDIA IN THE FILE (`player-tracks`): the sounds, pictures and
  # subtitles one file carries, drawn as layers stacked in one — shown only
  # while the file offers a choice among them.
  player-tracks = "md-layers_outline";
  # PLAY THIS ONE TOO (`player-also`), at the end of a row of the media in the
  # file: the same layers with a plus — one more lane of the same file, beside
  # the one in use.
  player-also = "md-layers_plus";
  # A GROUP OF THE QUEUE — a named playlist, a set — folded and open: the
  # plain triangle every tree draws, pointing at what it holds. Not the
  # drawer's chevrons (`expand`, `collapse`), which open a LINE downward.
  player-folded = "md-menu_right";
  player-unfolded = "md-menu_down";
  # WHAT A LANE IS, first in its row: a film's picture, a sound. Subtitles
  # are `player-subtitles`, the glyph the transport already draws them with.
  player-lane-picture = "md-movie_outline";
  player-lane-sound = "md-waveform";
  # ONE OF THE PLAYER'S OWN FILES in its file browser: a set (`.rset`) —
  # media played together, drawn as what overlaps — and a queue (`.rqueue`),
  # which is `player-queue`.
  player-set = "md-set_all";
  # A SUBTITLE LANE on the Subtitles page, inside its colour chip: the filled
  # caption box while it is shown over the picture, the outlined one while it
  # is hidden — one drawing in two states, and the chip is the switch.
  player-subtitle-shown = "md-subtitles";
  player-subtitle-hidden = "md-subtitles_outline";
  # THE FORMS the pictures of a set share a screen in, on the Layout page's
  # ring: Material's own drawings of a screen's views — the mosaic that
  # chooses for itself, one square, columns, rows, a grid, one large over a
  # strip, and the picture in a corner of another.
  player-form-auto = "md-view_compact";
  player-form-single = "md-square_outline";
  player-form-side = "md-view_column";
  player-form-stacked = "md-view_agenda";
  player-form-grid = "md-view_grid";
  player-form-strip = "md-view_dashboard";
  player-form-corner = "md-picture_in_picture_bottom_right";
  # A PICTURE ONE PLACE EARLIER OR LATER, at the end of its row on the Layout
  # page — the pointer's twin of the arrows: the plain triangles, pointing
  # the way it moves.
  player-layout-earlier = "md-menu_left";
  player-layout-later = "md-menu_right";
  # AN ENTRY OF THE QUEUE ONE PLACE EARLIER OR LATER, at the end of the
  # cursor's row — the pointer's twin of the arrows there. Up and down, the
  # way it moves in the list; never the triangles, which a group's row
  # already carries at its head, meaning folded or open.
  player-move-earlier = "md-arrow_up";
  player-move-later = "md-arrow_down";
  # WHERE A PICTURE GOES, on a lane's page: a monitor, the laptop's own
  # screen, and a monitor that is not there.
  player-screen = "md-monitor";
  player-screen-built-in = "md-laptop";
  player-screen-absent = "md-monitor_off";
  # WHERE A SOUND GOES, on a lane's page, by the kind of output: the
  # machine's default — whichever it picks — headphones, speakers, a
  # Bluetooth device, and any other.
  player-sink-default = "md-speaker_multiple";
  player-sink-headphones = "md-headphones";
  player-sink-speakers = "md-speaker";
  player-sink-bluetooth = "md-bluetooth_audio";
  player-sink-other = "md-volume_high";
  # WHAT KIND OF SOUND an ambient recording is, first in its row on the
  # ambient sounds' page — by its group in `assets/ambience.nix`: water that
  # falls or runs, and a storm's rain and thunder. A group this door has no
  # glyph for wears the plain sound wave (`player-lane-sound`'s drawing).
  # A SUGGESTION'S FIELD, to be written or not (`player_suggest`): a box
  # ticked or empty at the head of its row — the glyph a click turns.
  player-import-on = "md-checkbox_marked_outline";
  player-import-off = "md-checkbox_blank_outline";
  # A SUBTITLE TIMED FOR THIS VERY FILE — found by its fingerprint, not by
  # its name (`player_find`): a seal in front of its release's name.
  player-hash-match = "md-check_decagram";
  # A LANGUAGE taken out of a subtitle search, at the end of its row.
  player-language-remove = "md-close";
  # A STATION, A CHANNEL, A FILM KEPT AMONG THE FAVOURITES — or not — at the
  # head of its row (`player_directory`): the star a click turns, filled when
  # it is kept.
  player-favourite = "md-star";
  player-favourite-off = "md-star_outline";
  ambience-water = "md-water";
  ambience-storm = "md-weather_lightning_rainy";
  ambience-sound = "md-waveform";

  # THE COLUMN'S SIZES (`crate::column`), on its control line: four boxes
  # from the widest to the narrowest, the way the edge moves. Material draws
  # them as one family of lettered tags (`size_xl`, `m`, `s`, `xs`); there is
  # no `size_l` in Nerd Fonts 3.4.0, so the widest is `xl`. In the bar only
  # one fits: a double chevron pointing where the column grows back.
  column-large = "md-size_xl";
  column-medium = "md-size_m";
  column-small = "md-size_s";
  column-bar = "md-size_xs";
  column-widen = "md-chevron_double_left";
  # CLOSING THE COLUMN, left of the sizes and apart from them by a rule: a
  # cross, the one every window has. Only the column goes; what plays in it
  # goes on playing.
  column-close = "md-close";

  # THE PERSON'S OWN PROGRAMS (`LEADER i`, `crate::software`).
  #
  # The view's own mark, and the two modeline marks it may raise: a trial
  # running (a flask, because a trial is an experiment that leaves nothing),
  # and Android running (it costs memory, which is why it is shown at all).
  software = "md-package_variant";
  software-trial = "md-flask_outline";
  software-android = "md-android_studio";
  # WHERE A PROGRAM COMES FROM, one glyph per source, in front of its name on
  # a row. A source's own logo where the font has one — Nix's snowflake,
  # Flathub's — and a plain drawing of what it is where it has none: an
  # AppImage is one closed package, a web app is a page. `system` is a program
  # the flake installed, which no source here can change.
  software-source-nix = "md-nix";
  software-source-flatpak = "linux-flathub";
  software-source-appimage = "md-package_variant_closed";
  software-source-android = "md-android";
  software-source-web = "md-web";
  software-source-system = "md-cog_outline";
  # WHAT SETS ONE FOUND PROGRAM APART, one glyph per badge. A glyph is never
  # the whole of it: the sheet says each one in words (`software-badge-*`),
  # because a drawing alone is something only whoever chose it can read.
  software-badge-verified = "md-check_decagram";
  software-badge-unfree = "md-lock_outline";
  software-badge-sandboxed = "md-shield_outline";
  software-badge-command-line = "md-console";
  software-badge-arm-only = "md-chip";
  software-badge-broken = "md-alert";
  software-badge-insecure = "md-alert_octagon";
  software-badge-end-of-life = "md-calendar_remove";
  software-badge-anti-features = "md-eye_off";
  software-badge-library = "md-bookshelf";
  software-badge-installed = "md-check_circle_outline";

  # THE CHROME KIOSK'S MODELINE (`crate::kiosk`): a browser's own controls,
  # drawn as the codicons a browser's toolbar is known by — one family, so
  # back, forward and reload are one drawing in three directions. The bars
  # toggle names what the click WILL do (show Chrome's bars / hide them), and
  # the address line is a link, crossed out while it is hidden. A pinned tab
  # reuses `pinned`.
  kiosk-back = "cod-arrow_left";
  kiosk-forward = "cod-arrow_right";
  kiosk-reload = "cod-refresh";
  kiosk-new-tab = "cod-add";
  kiosk-more = "cod-ellipsis";
  kiosk-extensions = "cod-extensions";
  kiosk-zoom-in = "cod-zoom_in";
  kiosk-zoom-out = "cod-zoom_out";
  kiosk-show-chrome = "cod-screen_normal";
  kiosk-hide-chrome = "cod-screen_full";
  kiosk-address = "md-link_variant";
  kiosk-address-off = "md-link_variant_off";
  kiosk-audible = "md-volume_high";
  kiosk-offline = "cod-debug_disconnect";

  # THE THIRD KIND OF TASK: a brain module that runs nothing on a schedule and
  # LISTENS — to the journal, a path, a clock. An eye, beside the two arrows
  # of `task-routine` and `task-once`: it is not one more way of coming round,
  # it is a different kind of thing, and its row says "watching" instead of a
  # date.
  task-watch = "md-eye_outline";
  # A ROW OF THE BRAIN'S LOG THAT FOLDS SEVERAL, shut and open: the plain
  # triangle every tree draws, pointing at what it holds — the same drawing
  # `player-folded` is, because it is the same gesture.
  brain-group = "md-menu_right";
  brain-group-open = "md-menu_down";

  # THE MAXIMUM ALERT BAR (`crate::alert`): the battery's own alarm drawing in
  # its margin — the modeline's `battery-danger`, which says the same thing —
  # and the close button at its right edge, the drawing every other close
  # button of this system wears.
  alert-battery = "md-battery_alert";
  alert-close = "md-close";

  # AN ENCRYPTED DISK in the file manager's sidebar: the closed padlock while
  # it is locked (dim, like any disk nothing can be reached on yet), the open
  # one on the row of what came out of it. The disk's own glyph gives way to
  # them: that it is locked is what decides what pressing the row does.
  device-locked = "md-lock";
  device-unlocked = "md-lock_open_variant";

  # THE ICONS APP (`LEADER m i`): the glyph in front of its header's title —
  # shapes, which is what one comes looking for.
  icons-app = "md-shape_outline";

  # THE CALCULATOR (`LEADER =`). Its title's glyph is the keypad itself; the
  # key that takes the last character back is the key every keyboard prints
  # for it; the converter's swap is the two arrows that trade places, one up
  # and one down, because From is over To; and the category ring turns with
  # the two triangles every ring of this system turns with.
  calc = "md-calculator_variant";
  calc-backspace = "md-backspace_outline";
  calc-swap = "md-swap_vertical";
  calc-earlier = "md-chevron_left";
  calc-later = "md-chevron_right";

  # THE SEEKS BESIDE THE CLOCK (`player_media::clock_group`): ten seconds
  # back and ahead, the drawing that carries its number — the step is
  # `player::SEEK_STEP`, ten.
  player-rewind = "md-rewind_10";
  player-forward = "md-fast_forward_10";
  # SAVE THE QUEUE AGAIN into the file it came from, which it no longer is:
  # the disk with the mark every editor draws for "changed, not saved".
  player-save-again = "md-content_save_alert";
  # THE PICTURE FOLDED AWAY on the "Now" title, and back: drawn as what a
  # press does — lines drawn together, lines let apart.
  player-picture-fold = "md-unfold_less_horizontal";
  player-picture-unfold = "md-unfold_more_horizontal";
  # THE COUNTRIES' ORDER on their title, drawn as the order they are in: the
  # most first, or A to Z.
  player-countries-by-count = "md-sort_numeric_descending";
  player-countries-by-name = "md-sort_alphabetical_ascending";
  # THE LYRICS TRANSLATED under their verses, and not: on the Lyrics title.
  player-lyrics-translate = "md-translate";
  player-lyrics-untranslate = "md-translate_off";

  # THE CATALOGUE'S LISTING (`player_catalog`): the title's two controls — the
  # layout it is drawn in (`L`), a glyph for each so the control says which it
  # is now, and the listing on the whole screen (`F`) — and the adult
  # category's lock, shut and open.
  catalog-layout-list = "md-view_list";
  catalog-layout-lines = "md-view_agenda";
  catalog-layout-columns = "md-view_column";
  catalog-layout-posters = "md-view_grid";
  catalog-layout-big = "md-view_module";
  catalog-fullscreen = "md-fullscreen";
  catalog-live = "md-television_classic";
  catalog-locked = "md-lock";
  catalog-unlocked = "md-lock_open_variant";

  # A VIDEO WHOSE THUMBNAIL HAS NOT ARRIVED YET, in the grid's cell where the
  # picture goes (`video_metrics.rs`): the outline of a film, dim, until the
  # scan fetches the picture.
  video-no-thumb = "md-video_outline";

  # THE PASSWORDLESS WINDOW, on the modeline beside the hour it ends: a lock
  # standing open, which is exactly what sudo is for as long as it lasts. The
  # brain's shield says something is off; this says what and until when.
  security-sudo-open = "md-lock_open_variant";
  # THE CENTRE'S OWN THREE: the shield beside "nothing is off" and the
  # firewall on, the shield struck through beside the firewall off, and the
  # fingerprint beside each finger enrolled. Material, as the brain's shield
  # with the alert is, so the modeline's mark and the page's read as one set.
  security-shield = "md-shield";
  security-shield-off = "md-shield_off";
  security-fingerprint = "md-fingerprint";

  # THE PERSON'S CONTAINERS (`LEADER s o`, `crate::containers`): the view's
  # own glyph, and one per kind in its first column. Plain drawings of what
  # each thing is — a box, pods as boxes together, compose's layers, an image
  # as a closed package, a volume as a disk, a network as a network — and no
  # brand: the same drawing whoever made the image.
  containers = "md-cube_outline";
  containers-kind-containers = "md-cube_outline";
  containers-kind-pods = "md-hexagon_multiple_outline";
  containers-kind-compose = "md-layers_outline";
  containers-kind-images = "md-package_variant_closed";
  containers-kind-volumes = "md-harddisk";
  containers-kind-networks = "md-lan";
  # THE QUEUE'S ENGINE for a pull, a build or compose — the same box.
  engine-container = "md-cube_outline";

  # FAST-ACCESS'S STRIPS: the two marks that page a strip of icons when some
  # are hidden beyond it — the calculator's own arrows, so they draw alike.
  fast-access-earlier = "md-chevron_left";
  fast-access-later = "md-chevron_right";
  # AND THE BOX OF "DO THE SAME FROM NOW ON" under a web video's question:
  # ticked or empty, the glyph a press turns — the player's suggestion boxes.
  fast-access-ticked = "md-checkbox_marked_outline";
  fast-access-unticked = "md-checkbox_blank_outline";

  # THE QUEUE'S LAST THREE ENGINES, one drawing each, so the queue's window
  # tells them apart as the footer's words do: a program is the software
  # center's own package, a file sent to VirusTotal a shield being searched,
  # and the belongings' sync an archive under a lock — they leave encrypted.
  engine-software = "md-package_variant";
  engine-scan = "md-shield_search";
  engine-belongings = "md-archive_lock";

}
