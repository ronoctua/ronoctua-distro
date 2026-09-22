# WHAT `nix build .#shots` PHOTOGRAPHS, one id a scene.
#
# # Why the list exists
#
# Because the battery had one shape — all of it — and the owner read the cost
# off the deliveries: *"os mesmos screenshots dos mesmos locais estão sendo
# gerados toda vez que você testa algo novo … enquanto faria muito mais sentido
# focar os screenshots apenas nas interfaces das coisas recém implementadas ou
# modificadas"*. A run is four minutes and thirty-five pictures, of which one
# delivery changes two; the other thirty-three are noise in the report and
# minutes on the clock.
#
# So every subtest of `nix/shots.nix` carries an id from this list and the
# flake exposes one package per id — `nix build .#shots-manager` photographs
# the file manager and nothing else. `nix build .#shots` is still every one of
# them, and is still what proves the whole screen: SELECTING IS FOR THE LOOP,
# not for the gate.
#
# # And the list is checked against the script
#
# `shots-scenes-are-total` reads both sides: an id here that no subtest uses is
# a package that photographs nothing, and a subtest with no id is a picture
# nobody can ask for on its own. It is `tests/slow.nix`'s argument, one battery
# along.
#
# The ORDER is the order they run in, which is the order the script has them.
#
# # `needs`, and why it is not optional
#
# Most scenes open their own door — an `Escape` or two and a `control(…)` — and
# a run of one of those is a run of one. TWO of them do not: the document
# preview and the miniplayer both ride on the window the manager's own scene
# opened, and selecting either of them alone photographed AN EMPTY DESKTOP.
#
# It was worse than that, and it is why this field exists rather than a
# sentence in a comment: the subtest still PASSED. Its measurement asked
# whether `text.primary` covered twenty pixels of the panel's box, and the
# mouse pointer is white — so the assertion answered with the cursor. A scene
# that photographs nothing and says it worked is the thing this whole battery
# exists to prevent.
#
# So a scene that cannot stand on its own says what it stands on, and asking
# for it brings those along in declared order. `shots-scenes-are-total` reads
# both sides, and it also refuses a scene that opens no door of its own and
# declares no need.
[
  # THE STATE THE OWNER SENT A PHOTOGRAPH OF. The panel used to reserve three
  # lines and draw them at `end - 3`, `end - 2` and `end - 1`, where `end` is
  # the top of the region plus the number of rows — and with no history that
  # number is ONE. Nothing clamped them, so `ClipCascade: not integrated`
  # landed on the MINIBUFFER's line and the actions strip landed on the row
  # that says the history is empty. An empty history is the first state to
  # photograph and was the only one nobody had: there was no layout test of the
  # clipboard at all.
  #
  # IT RUNS FIRST, before anything opens a WINDOW: it gives the region back
  # with `Escape`, and an `Escape` with a window focused closes that window —
  # which took the file manager out from under the theme chooser's subtest when
  # this sat in the middle.
  { id = "clipboard"; }
  # WHAT THE OWNER READ ON THE SCREEN: nothing said which line the cursor was
  # on. `Row::focused` is set by the view and read by NO door in this system —
  # neither the footer nor the window draws a band behind the cursor — so the
  # colour IS the cursor, and this view was the one that never set it. Nine
  # others have carried it since `network.rs` arrived at the split: the label
  # takes `state.accent` ("the keyboard is here") and the value goes on saying
  # whether the answer can be changed.
  #
  # THE SCENE'S ASSERTION IS A COLOUR, because the report was. A count of rows
  # or a word on screen would have passed the whole time the defect existed.
  { id = "history"; }
  # THE OTHER HALF OF WHAT THE OWNER REPORTED: he could not find a way to turn
  # Tor on. The section lived behind `if let Some(scene)`, and the control
  # socket only answers when Tor is already running — so a stopped Tor drew no
  # heading, no row and no key. This machine does not run Tor, which is exactly
  # the case being photographed: the heading has to be there anyway, with a row
  # that says what the UNIT is doing.
  #
  # (The sentence was cut in half by an earlier move of this prose, which left
  # `nix/shots.nix` pointing here at a section that had never been written.
  # Recovered from 3134000.)
  { id = "tor"; }
  # THE ONE SCENE THAT OPENS NOTHING ON PURPOSE: what it photographs is the
  # footer with nothing on it, so anything it opened would be the wrong
  # picture. `alone` says that out loud, because the check's rule is "a scene
  # opens a view or names whose it borrows" and this is neither.
  #
  # ITS TWO MEASUREMENTS ARE AIMED FROM THE FOOTER'S TOP, and in LINES.
  #
  #   - NOT FROM THE SCREEN'S BOTTOM, because the bottom line is not the
  #     minibuffer: the brain's line is glued UNDER it, and in these machines
  #     the brain is up at rest — the journal says `brain=1` and the surface is
  #     three lines, not two. Aiming at the last one clicked the brain's row,
  #     and the assertion failed saying the list had not opened when what had
  #     happened is that nobody had asked for it. From the top the two lines
  #     are where the module's own header puts them: the modeline, then the
  #     minibuffer glued to it; the band and the glance are above both and are
  #     zero at rest, which is what the shot before it is about.
  # AND THE SHORTCUT LIST IS THE ONE LIST THIS SYSTEM DRAWS IN COLUMNS, which
  # is why the pointer is measured in two of them here. `item_at` resolves a
  # point to `column × lines + line` and the count it rejected an index against
  # was `grid.lines` — the height of ONE column. So the first item of the
  # second column was already out of range: every cell outside the first column
  # answered `None`, with no hover band and a click that did nothing, over two
  # thirds of the screen. The owner read it off the screen — *"o hover e
  # suporte ao clique do mouse funciona apenas na primeira coluna de itens"* —
  # and no test of state could see it, because the state was right and the
  # arithmetic that turned a pixel into an item was not.
  #
  #   - IN LINES AND NOT IN PIXELS, because the claim — "this is a footer at
  #     rest" — is about lines. It read `2 * LINE <= at_rest`, and `LINE` is
  #     the line rounded UP: fifty-four against the fifty-three a two-line
  #     footer actually is, so the condition could never hold for the state the
  #     subtest is NAMED for. It passed while the brain's row was up and the
  #     surface was three lines, which in a full battery it is not — the
  #     brain's own scene comes nine scenes later.
  {
    id = "footer";
    alone = true;
  }
  { id = "band"; }
  { id = "queue"; }
  { id = "loud"; }
  # THE WHOLE CAPTURE PATH, DRIVEN FOR REAL. The command is ours end to end:
  # `screen-capture` runs `grim`, `grim` writes a file under
  # `RONOCTUA_SCREENSHOTS`, and the footer raises the notification itself with
  # the `file://` URL in the icon field — which is where the four actions get
  # the image from. Nothing in the scene stands in for anything, which a
  # hand-written `Notify` over the bus would: that is a scene that goes on
  # passing after the thing it stood for has been deleted.
  #
  # And the glyph is `md-monitor_screenshot`, named in
  # `assets/notifications.nix` as a NerdFonts glyph rather than a design system
  # token: nothing in the design system says "screen capture".
  { id = "screenshot"; }
  { id = "notifications"; }
  { id = "catalogue"; }
  { id = "manager"; }
  # THE WINDOW DOOR IS REACHED THROUGH THE FOOTER'S. It creates its own files
  # and opens its own window, and it still cannot stand alone: the `BackSpace`
  # that puts it in `/root/shots` before asking for the window is a key sent to
  # the FOOTER's manager, which the scene above opened. Run without it the
  # window came up somewhere else, the filter found nothing, and the syntax
  # assertion read a panel of the wrong file — `#6CD586 covers 0 pixel(s)`,
  # which is this battery catching exactly what it is for.
  # THE MANAGER'S WINDOW, and the preview that rides on it.
  #
  # ITS SIDEWAYS BAR IS MEASURED AS A COUNT, never by `-trim`. `C-]` moves the
  # preview's line and the thumb has to follow it — where a bar is drawn is
  # where it is grabbed, and a thumb that stood still while the text moved
  # would be lying about where one is. The count is of the bar's own token on a
  # four-pixel probe at the left edge, because `-trim` answers from the CORNER
  # pixel and at column nought the corner IS the thumb: it reported nothing
  # exactly when the thumb was where it should be, and the first version read
  # that "nothing" as a position of minus one — after which `after > before`
  # passed because 3 is more than −1 rather than because anything moved. A
  # count cannot do that: with no bar it is nought at both ends and the
  # assertion fails, and with a bar that does not move it is the same at both
  # ends and the assertion fails.
  # AND THE EXECUTE BIT COLOURS THE GLYPH, measured in BOTH states.
  #
  # The owner asked for the colour to move off the name and onto the
  # type's glyph. One crop cannot say it: `state.success` is somewhere
  # on a screen full of chrome, so what proves it is the SAME box
  # answering nought and then something after a `chmod`.
  #
  # A DELTA AND NOT AN ABSENCE, and the reason is the palette itself:
  # in the default theme `state.success` and `syntax.string` are the
  # SAME INK (`0x6CD586`). Asked whether the green is on the screen at
  # all, this box answers 134 pixels before anything is chmod-ed —
  # the `.nix` in the preview, whose strings wear the other token.
  # Two roles on one primitive is legal (they are never on one line);
  # what it makes illegal is an absolute assertion about either.
  #
  # So what is measured is the SAME box answering more than it did.
  # The whole window, because the listing's own edges are the
  # manager's arithmetic and not this driver's.
  # FOUR: THE CONTROLS ARRIVE WITH THE POINTER, AND NOT BEFORE.
  #
  # The owner asked for exactly this — *"controles que aparecem no hover
  # do mouse"* — and it is the only claim here that has to be written as a
  # negative as well as a positive. Without the first half, a row that was
  # always drawn would pass the second.
  #
  # The transport is the panel's LAST line, which is where the picture
  # stops: `View::transport_rows` takes it off the box, so this row is
  # never ink drawn over the film.
  # THE BOTTOM OF THE PANEL, which is where the controls go — counted in
  # the colour a GLYPH is drawn in and not by a threshold. Hovering used
  # to light the section's own band, and a band is ink: a threshold would
  # have risen for the hover rather than for the control, and passed
  # either way. (The band over a picture is gone now — see
  # `View::band_belongs` — but the measurement stays honest by colour.)
  # AND NO HOVER BAND ON THE FILM.
  #
  # The owner reported it in those words — *"o hover do mouse está
  # destacando o background da linha no preview de vídeos e imagens, o
  # que é totalmente desnecessário"*. A band says which ROW of a list the
  # pointer is on, and a picture is not a list: it lights an arbitrary
  # stripe across the film and covers part of what one came to look at.
  #
  # MEASURED IN BOTH STATES, and that is not caution — it is the only way
  # this can be measured at all.
  #
  # `surface.selection` and `rule.default` are THE SAME VALUE, so a count
  # of that colour inside the panel includes every divider and rule that
  # is always there. Asserting it is nought is asserting something
  # impossible: the baseline is 808 pixels of rules, and it failed on them
  # while the band it was written for was already gone. What answers the
  # question is the DIFFERENCE — 808 against 4357 with the band, 877
  # without it.
  {
    id = "manager-window";
    needs = [ "manager" ];
  }
  # THE WINDOW DOOR IS WHERE BOTH OF THESE HAPPEN. Neither opens one: they walk
  # the directory the scene above left them in.
  # THE DOCUMENT PREVIEW, in one sentence: the owner's folder had eleven
  # documents and the preview opened one of them, the PDF. The other six of the
  # office family are a ZIP OF XML — and this crate has had a zip reader since
  # the day it learnt to draw a 3MF, so what was missing was the twenty lines
  # that say which member holds the words.
  #
  # PHOTOGRAPHED and not only unit-tested, because the unit tests know
  # `text::Read` and only this knows the PANEL: the answer comes back as the
  # same type a `.rs` does, so the panel scrolls it, pages it and hangs a bar on
  # it with no new code at all — and that claim is about a screen.
  {
    id = "document";
    needs = [ "manager-window" ];
  }
  # THE MINIPLAYER, on a GIF and not a video, and that is not a compromise.
  #
  # The GIF path is the `image` crate, decoded in process — so the subtest
  # needs none of the five external programs and runs on a machine with no
  # media group at all. What it proves is the whole player: a frame reaching
  # the panel, the box sized from the FRAME rather than from a still, the
  # transport taking a line of its own, and the hold being real. What it
  # cannot prove is the TIMELINE, which needs a duration, which needs a video,
  # which needs `ffmpeg-full` inside this VM — a closure decision and not a
  # test decision, so it is not taken.
  #
  # AND ITS FOUR COLOURS ARE ONES THIS INTERFACE DOES NOT CONTAIN, measured
  # rather than assumed: the first version used four palette colours and the
  # blue one matched the interface elsewhere within the fuzz — the picture's
  # own box came back as 172x644, most of the panel, while the orange, which
  # is near nothing, came back as 130x97 for exactly 12610 pixels. A
  # measurement that catches its neighbours is not a measurement.
  {
    id = "miniplayer";
    needs = [ "manager-window" ];
  }
  # THE GLOBE, which rides on the window door for the same reason the two above
  # do: it is drawn in the info panel of the manager that scene opened, and on
  # its own it would photograph an empty desktop.
  # WHAT THIS SCENE DOES NOT PHOTOGRAPH is the globe HALF scrolled,
  # and the reason is written here so nobody spends the runs again.
  #
  # It would not decide anything: the picture's box shrinks from the
  # bottom whether the drawing is CUT at its top or SQUEEZED into what
  # is left, so no edge of it tells the two apart — what differs is the
  # scale of what is inside. `globe::tests::            # what_scrolled_away_is_cut_from_the_top_and_never_squeezed` is what
  # decides it, by an arithmetic that cannot answer both ways: a band
  # in the bottom half of a tall box FILLS a short box cut by half and
  # is ABSENT from a short box that was not cut.
  #
  # And the state is hard to reach from here anyway: a turn of the
  # wheel is several lines and carries the whole band off in one go,
  # and a two-line drag of the thumb moved nothing at all. Measured,
  # both, on this scene.
  {
    id = "globe";
    needs = [ "manager-window" ];
  }
  # THE FACTS GIVEN THE WHOLE REGION, which rides on the window door for the
  # reason the three above do: what it photographs is a panel of the manager
  # that scene opened, and alone it would photograph an empty desktop.
  {
    id = "facts-wide";
    needs = [ "manager-window" ];
  }
  # THE LISTING'S OTHER SHAPE. It rides on the window the manager's scene
  # opened, like the three above it, and it makes its own folder: the picture it
  # counts pixels of has to be one this repository does not carry (invariant 13).
  {
    id = "thumbnails";
    needs = [ "manager-window" ];
  }
  # THE REGIONS' VIEW, which opens its own door: `geo-open` is a command, so
  # this scene needs nothing from the ones before it.
  { id = "geo"; }
  { id = "brain"; }
  # THE BRAIN'S OWN WINDOW. It needs the scene above, which is what starts the
  # daemon — without one the buffer opens and draws the screen it draws before
  # a mind arrives, which is the picture this repository already has too many
  # of.
  {
    id = "brain-buffer";
    needs = [ "brain" ];
  }
  { id = "update"; }
  # THREE THINGS PROVED ON SCREEN, and none of them by a buffer (`nix run
  # .#png` cannot reach a footer view). One: the list opens on the theme in
  # force, its focused row in that theme's accent. Two: moving the cursor
  # PREVIEWS — the second theme's accent is on the footer and the first's is
  # gone — while the file manager's window, opened beforehand, is untouched.
  # Three: `RET` writes the file, and the window follows the FILE: its focused
  # row is now in the second theme's accent with no restart, and the footer
  # copied the two fragments beside the file. The two accents are far more than
  # the twelve per cent apart; the window is measured on its top half BELOW its
  # own hint line, so neither the footer's lines nor the hint's registers enter
  # the count.
  { id = "theme"; }
  # THE LINE `LEADER d` OPENS, and it had no picture at all — which is why
  # every defect the owner found in it went unseen: `nix run .#png` reaches a
  # buffer, and this is a footer line.
  #
  # The subtest measures TWO things, both in pixels off the screen. ONE: a
  # place's ground has the SAME AIR on both sides of its text — it used to
  # begin where the first letter began and run on to where the next place
  # started, which is nothing on the left and two cells on the right, and the
  # whole rest of the bar for the last one. TWO: the countdown sits an exact
  # cell from the BORDER — `columns` is floor(width / cell), so a run counted
  # from the left sits a cell PLUS the grid's remainder from it, and that
  # remainder changes with the window. It is what put the countdown out of
  # line with every mark on the modeline below it.
  #
  # AND TWO THINGS THE FIRST RUNS OF IT TAUGHT, both of which look like the
  # feature being broken and are not:
  #
  #   - THE POINTER HAS TO HOLD THE LINE. It lives six seconds and `xwd` plus
  #     `magick` take longer than that on a software-rendered VM, so the first
  #     runs photographed a footer the glance had already left — and measured
  #     the DESKTOP, which wears the same `neutral.chrome` as the alternate
  #     ground. `world::Showing::hold` is what stops the clock, and resting
  #     the pointer on the row is the gesture that does it.
  #   - THE MODELINE SAYS WHERE THE ROW IS. Its ground is drawn on one line
  #     and nothing else wears it, so the rows above it are the glance's —
  #     which needs no arithmetic about the footer's height, and `LINE` is the
  #     cell ROUNDED UP, so arithmetic would be off by a pixel per row.
  #
  # THE REASONS LIVE HERE AND NOT IN THE SCRIPT, and that is not a preference:
  # the whole test script is ONE argument to the kernel, and a kernel argument
  # is capped at 128 KiB. Half of it is already comments; these would be the
  # bytes that broke it. `the-shots-script-fits-in-an-argument` measures it.
  { id = "glance"; }
  # AND THE SETTINGS SCREEN, which had no picture either. What was wrong with
  # it was a sentence cut in half where the room ran out, and a hierarchy
  # drawn in one tone — the section's title and every place under it in
  # `text.group`, at the same column. Neither is something a test that
  # compares state can see; both are a glance at a photograph.
  { id = "locale"; }
  # AND THE HEADER OF A WINDOW, which had no drawer at all.
  #
  # The line that teaches a view's keys is CUT at the width it is given —
  # `chrome::hint_spans` elides, it does not wrap — and the network view's
  # table is 174 cells in English against a window the compositor tiles into half
  # of a 1600-pixel output. So more than half of what that line teaches was
  # unreachable in every one of the ten windows, with an ellipsis where it
  # stopped and nothing to press.
  #
  # The footer's minibuffer has opened into a drawer since `crate::fold` was
  # written, and so has the file manager's window; the other ten never got
  # one. The owner found it on the language and region window — *"a linha do
  # header com os atalhos não tem a feature com o ícone de chevron para
  # expandir/recolher o header"*.
  #
  # WHAT THE PHOTOGRAPH ANSWERS, and a test of state cannot:
  #   - THE CHEVRON IS DRAWN, and on the line's own row, against the right
  #     edge. Ink in the last cell of row 0 is the whole of that claim.
  #   - PRESSING IT GROWS THE LINE. The prose of a hint is `text.subtle`, and
  #     before the press it is on one row; after it, on several. A state test
  #     would compare a `bool` and never look at the rows.
  # THE RUBBER BAND, and the folder a drop lands in.
  #
  # Two scenes for one delivery, and both ride on the window the manager's own
  # scene opened. They make their own folder: what they count pixels of has to
  # be a picture this repository does not carry (invariant 13).
  #
  # WHAT THE PHOTOGRAPHS ANSWER, and no test of state can:
  #
  #   - THE BAND IS DRAWN OVER THE MINIATURE. There is no depth test in this
  #     renderer — quads, then pictures, then text — so a band pushed with the
  #     other backgrounds is invisible in a grid, which is pictures edge to
  #     edge and the one place it matters. What is measured is the PICTURE's
  #     own colour inside one cell, before the drag and during it: a band on
  #     top takes some of it away, and a band underneath takes none.
  #   - AND IT TAKES WHAT IT COVERS. The mark is a glyph in `state.accent`
  #     (`files.rs`: a glyph and not a colour, because the colour is already
  #     saying focus), so counting that tone in the listing's own column before
  #     and after the gesture is counting how many entries the band marked.
  #   - AND A DROP MOVES THE FILE. `test -e` on the destination, which is a
  #     fact from outside the picture — the one kind of assertion a stale frame
  #     or a white mouse pointer cannot answer.
  {
    id = "sweep";
    needs = [ "manager-window" ];
  }
  #
  # AND THE DROP IS TWO MOVES, NOT ONE. A press is not a gesture until the
  # pointer leaves its slack, and the compositor sends `wl_data_device.enter`
  # on the motion AFTER the drag has begun — so one jump does both jobs and the
  # destination is never entered. Measured: the drag started, `enter` never
  # came, and the file stayed where it was. The first move picks the file up;
  # the second carries it.
  #
  # AND THE QUEUE HAS TO BE UP FOR IT. A drop ends in `ronoctua-transfers`
  # (`files::queued` hands the work over its socket), so with no daemon the
  # shell says so on its line and the file stays where it is. The unit is
  # started by this scene rather than by the battery: it is the only one that
  # asks a question the queue answers.
  {
    id = "drop";
    needs = [ "manager-window" ];
  }
  { id = "header"; }
  # THE ASSERTION THAT WOULD HAVE CAUGHT IT, and it is the second time.
  #
  # `Footer::first_of_expansion` counted a row whose existence depended on
  # how TALL the region is, and `Footer::metric` is built on
  # `first_of_expansion`. The pointer coming to rest on a key of the hint
  # line closed the ring: stack overflow, `process::abort`, no modeline
  # and no minibuffer. Two coredumps of 2026-09-15 carry that stack and
  # nothing else, and the owner read it as the interface closing when he
  # hovered a video preview — the video was where the hand was going, the
  # hint line was what it crossed.
  #
  # EVERY LINE OF THE FOOTER, from the bottom up, and not one aimed
  # number: the hint's own row moves with the band, the glance, the
  # opened drawer and the brain's line, so a shot that knew where it was
  # would be photographing a listing row the day any of them appears.
  #
  # THE PREVIEW REACHES THE WINDOW TOO, and this line used to assert
  # the opposite — "the preview is the footer's alone: the window
  # still wears the first". That WAS true, and the owner read it on
  # screen and called it a theme worn by a strip of the display: the
  # minibuffer changed colour while the windows, the desk and the
  # terminals stayed in the old one. The answer was
  # `theme::show_everywhere`, which writes `theme.json` — the channel
  # the other doors already watch — so moving the cursor dresses
  # every door. The assertion is inverted because the promise was.
  #
  # This is `docs/PENDING.md` item 8c's second layer, answered: it was
  # never a broadcast defect. It was a subtest left asserting a
  # promise that the themes delivery deliberately reversed, and it
  # went unread because `.#shots` had not been run since.
  # MEASURED AGAINST THE PANEL, and NOT against the same picture with the
  # controls up.
  #
  # That was the first shape of this and it is self-contradicting: the
  # band is half-transparent, so it DARKENS the film it covers, and the
  # film's own colour stops matching there. The picture then measures
  # shorter exactly BECAUSE the band works — and the assertion two blocks
  # down checks that the film still shows through it. One of the two had
  # to be wrong, and it was this one.
  #
  # What says the controls cost the film nothing is the film filling the
  # panel's own height: four to three in a panel twice as wide as it is
  # tall is bounded by the height, so it reaches the box's edge. A line
  # reserved for the controls would leave it a whole line short.
  #
  # AND THE SECOND THEME'S, for the chooser's shots: a preview has to show a
  # colour the default does not have, and the assertion reads it from the
  # generator rather than from a number typed here.
  # With one theme declared the chooser's shots have nothing to prove and the
  # driver skips them (`THEMES > 1`); the path still has to exist.
  #
  # THE ONE A SINGLE `<down>` REACHES, which is the row below the DEFAULT and
  # not the second in the list. Those were the same number while the first
  # declared theme was also the default; they stopped being when the owner
  # asked for the chooser's order and the system's default to be two
  # declarations (`assets/themes/themes.nix`). It wraps at the end, exactly as
  # the cursor does not — so if the default is ever last, this takes the first
  # and the driver's `<down>` would not reach it. That is why the assertion
  # below reads the FILE rather than assuming.
  # THE OTHER HALF OF WHAT THE OWNER REPORTED: he could not find a way
  # to turn Tor on. The section lived behind `if let Some(scene)`, and
  # the control socket only answers when Tor is already running — so a
  # stopped Tor drew no heading, no row and no key.
  #
  # This machine does not run Tor, which is exactly the case being
  # photographed: the heading has to be there anyway, with a row that
  # says what the UNIT is doing.
  #
  # `network-tor-open` AND NOT A COUNT OF `TAB`s. The first version of
  # this pressed `TAB` nine times and the picture came out on the Wi-Fi
  # section — the ring's length depends on what hardware the machine
  # has, and this VM has neither radio nor cable. A guess that produces
  # a picture is worse than no picture.
  # -----------------------------------------------------------------
  # THE LIST FIRST, so the sheet can be told from it. This subtest had
  # NO ASSERTION, and that is how it passed twice on a picture of the
  # wrong screen: first the nine `TAB`s that landed on Wi-Fi, then
  # `fresh_at` looking for a row that a machine without Tor does not
  # draw. Two pictures that are equal prove the door opened nothing.
  { id = "hints"; }
  # THE LINE THAT OUTLIVES THE FOOTER, and the keyboard reaching its options.
  #
  # `Esc` and not `x`: the owner read the photograph and asked for the key that
  # actually closes it. And the line is CLICKED once more first, on its own top
  # row — `KeyboardInteractivity::OnDemand` is what a click buys, because at
  # rest the surface takes the keyboard from nobody, which is what lets it
  # appear over somebody's work, and the rescue console it has just opened is a
  # window that takes it. Measured: with other windows on the workspace the
  # `Esc` went to the console and the line stayed up; on an empty one it did
  # not, which is the kind of difference a battery run finds and a single scene
  # does not. The TOP row and not an option, because `Lifeline::at` answers
  # `None` for any row but the last — so the click does nothing except what a
  # person's would, put the keyboard back on the line they are reading.
  { id = "lifeline"; }
]
