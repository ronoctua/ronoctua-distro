# Spacing tokens — section 5.1.
#
# "Separation by whitespace and one-line rules, never by a container."
#
# The scale is in CELLS, not in pixels. Since the cell is exactly 0.600 em
# (see typography.nix), spacing in cells keeps everything aligned to the text's
# grid without measuring anything — which is what the footer needs in order to be integer
# arithmetic.
{
  # Spacing between modeline segments, in cells. There is no internal
  # padding: the segments are text, not boxes.
  modeline = {
    segmentGap = 2;
    # Margin at the screen's edges, in cells.
    #
    # THE RIGHT ONE USED TO BE ZERO, and it was wrong. The reasoning was
    # "the notification icon sits flush against the right edge (4.2)", and
    # flush is exactly what it looked like on screen: the unread count ended
    # against the panel's border, with no air, and read as a number that had
    # been cut off rather than one that had ended. It is the same defect
    # `files.edgeRight` already names one block below, in a table.
    #
    # One cell, and not two: this is a status line, not a table. Two would
    # start to look like a margin.
    edgeLeft = 1;
    edgeRight = 1;
    # THE MOST LINES THE OPENED LINE MAY TAKE, under the line itself.
    #
    # The minibuffer's line holds one line of text and what does not fit is
    # cut. Opened, it shows the rest, and it is the size of what is in it —
    # up to this. Past it the text SCROLLS, which is what the bar is for.
    #
    # IT WAS A FIXED HEIGHT, and the argument for that is worth keeping
    # because it is the argument this number now overrides. The footer's
    # height is the same on every output (`Footer::apply_geometry`), so a
    # height that follows the wrap follows the WIDTH: moving the focus between
    # monitors of different widths reflows the buffers on one of them. It is
    # the same reason `band.rows` counts the notification's BODY and ACTIONS
    # rather than wrapping its summary.
    #
    # The owner saw the fixed height on screen — a short sentence opening into
    # a drawer with four empty rows — and chose the other trade. The cost is
    # real, bounded by this number, and paid only while a drawer is open,
    # which is a state somebody entered on purpose and leaves with one key.
    #
    # Six: a hint line is three or four wrapped rows on a normal screen, so
    # the ceiling is rarely reached and the bar is rarely needed; and the
    # fixed part of the footer stays under the expansion's own budget, which
    # counts these lines (`expansion_ceiling`).
    foldLines = 6;
  };

  # The FILE MANAGER's table, in cells.
  #
  # Two different jobs, and that is why they are two names and not one number
  # used twice. `gutter` is the cell BEFORE the first column: the mark lives
  # there, and it is also the air the icon needs so the glyph does not touch the
  # border and read as clipped. `edgeRight` is the air AFTER the last column —
  # without it the modification date ends flush against the screen's edge, which
  # reads as a column that was cut off rather than one that ended.
  #
  # The modeline has an `edgeRight` of its own, and for the same reason — but
  # they stay two numbers: a table is not a status line, and the day one of them
  # wants two cells the other must not follow.
  # -------------------------------------------------------------------
  # The globe
  # -------------------------------------------------------------------
  #
  # IN PIXELS, which is the fourth block in this file that is — the reason
  # `files.pictureAir` gives: what these measure is a picture, and a picture is
  # not made of cells.
  atlas = {
    # Air round the disc, so the rim is not flush with the panel's edge.
    air = 6;
    # THE COORDINATE'S DOT, in TENTHS of a pixel.
    #
    # Tenths because this file generates integers — `files.meshWireWidth` makes
    # the same move one figure along, and for the same reason: a radius of
    # three-and-a-half is a real answer and `u16` is not.
    dotRadius = 35;
    # A line of the globe, in tenths of a pixel.
    stroke = 11;
    # How many lines of the info panel the globe takes.
    #
    # SIX, and the reason is the panel and not the picture: the panel is
    # twenty-four to forty cells wide and the region's ceiling in the footer was
    # twelve lines when this was measured, so a globe that took more would leave
    # no facts. The ceiling is fifteen now and six still holds: what a globe is
    # worth does not change because there is more room under it, and the three
    # lines the region gained are three more FACTS. Below the disc there is one
    # line for the address.
    lines = 6;
    # Region borders are drawn only when the country's box is at least this
    # wide in pixels — below it they are mud.
    regionsFrom = 240;
  };

  files = {
    gutter = 1;
    edgeRight = 1;

    # FROM THIS WIDTH ON, THE SIDEBAR IS OFFERED — the shape
    # `notifications.twoColumnsFrom` below already has, and the same number
    # serves BOTH doors, which is what makes "one implementation, two doors"
    # true of the layout and not only of the code.
    #
    # The arithmetic, and it is arithmetic and not taste. A listing needs
    # `gutter` 1 + icon 2 + gap 1 + the name's floor 12 + `edgeRight` 1 = 17
    # cells before it says anything at all. A sidebar needs a glyph, a cell of
    # air and a name — `screenshot` is ten — plus the rule and the air beside
    # it. Below 48 the two coexist only by leaving the listing with nothing but
    # names; at 48 it still holds the icon, the name and the size beside a
    # sidebar, which is the narrowest arrangement worth having.
    #
    # It is a PREFERENCE, and not what holds: what holds is that nothing leaks
    # off the screen, and `files::sidebar_columns` withdraws the sidebar
    # entirely when honouring it would push a column past the edge.
    sidebarFrom = 48;

    # THE AIR ON EITHER SIDE OF THE SIDEBAR, and it is two numbers because they
    # are two different jobs — the same reason `gutter` and `edgeRight` above
    # are two.
    #
    # THIS FILE HAS NOW BEEN WRITTEN ABOUT THIS DEFECT THREE TIMES, which is why
    # it gets said plainly: a glyph flush against a border does not read as a
    # glyph at the edge, it reads as a glyph that was CUT OFF. `modeline.edgeRight`
    # says it about the bell and the marks beside it; `files.edgeRight` says it about the date
    # column; and the sidebar still shipped with its icon at cell zero and its
    # eject glyph against the rule. The window door never had the defect only
    # because it pays `buffer.marginX` on top of everything.
    #
    # `sidebarGutter` is the cell BEFORE the sidebar's glyph — the screen's own
    # edge. `sidebarEject` is the air BEFORE the eject glyph, so it does not
    # touch the rule that separates the two lists.
    #
    # # AND `sidebarGutter` IS ZERO NOW, which is not a retraction of any of it
    #
    # The paragraph above is still true and the cell is still there: it is
    # `chrome::FIELDS_LEAD` that pays it. When this token was written the
    # footer's files `Grid` was "the one grid in the shell built without a
    # `start()`" — the sentence three lines up — so the sidebar genuinely had
    # nothing between its glyph and the border and had to buy the cell itself.
    # `chrome::INSET` arrived afterwards and every row of the manager, the
    # sidebar's included, now begins `INSET - files.gutter` cells in. A row that
    # ALSO carried a gutter of its own paid twice, and the owner measured the
    # result off the screen: the sidebar's glyph 21 px from the edge with 14 px
    # between it and its own name — *"o espaçamento à esquerda deve ser
    # reduzido"*. One cell in, and the two sides of the glyph are even.
    #
    # It stays a NAMED ZERO rather than disappearing, exactly as `content.insetY`
    # does one block below: the cell has an argument, and the argument is now
    # answered somewhere else. Deleting the name would delete the answer with it.
    sidebarGutter = 0;
    sidebarEject = 1;

    # THE AIR BETWEEN THE TWO GLYPHS at the right-hand end of the path's line,
    # in CELLS.
    #
    # There are two of them side by side while the entries are thumbnails — the
    # one that rings the SIZE and the one that turns the SHAPE over — and with
    # nothing between them the owner read them off the screen as one control:
    # *"você colocou o glifo de mudar o tamanho das thumbnails colado no glifo
    # de trocar de modo, mas obviamente é preciso colocar um pequeno espaço
    # entre eles."* Two glyphs of the same family and weight in adjacent cells
    # have no boundary to see; the air IS the boundary, and it is the same
    # argument this file already makes four times about a glyph flush against a
    # border reading as a glyph that was cut.
    #
    # One cell, which is what `sidebarEject` chose for the same job. Two would
    # cost the path a second letter and start to read as two separate controls
    # rather than a pair.
    layoutAir = 1;

    # THE AIR AROUND THE PREVIEW'S PICTURE, in PIXELS and not in cells — the
    # fourth block in this file measured that way, and for the same reason
    # `panelPixels` two paragraphs down is: it is the size of a PICTURE, and a
    # picture has no cells.
    #
    # `Area::fit_into` centres what it is given inside the box it is given, so
    # the picture was already centred — and it still touched two borders,
    # because whichever of the two dimensions is the tight one ends up exactly
    # as long as the box. Photographed in both doors: against the TOP edge in
    # the window and against the RIGHT rule in the footer. A photograph flush
    # against a rule reads as a photograph that was CUT, which is the sentence
    # this file already carries about the unread counter, the date column and
    # the sidebar's glyph.
    #
    # Six, and it is bounded from both sides. Below about four the air stops
    # being visible at all next to a one-pixel rule; a whole cell (10,8 px in the
    # footer, 14,4 in the window) is more than the gap between two columns of
    # text, and the picture would read as belonging to neither panel. Six is
    # rather more than half of the half-cell the rule already leaves beside it,
    # so what one sees between the line and the ink is about one cell — the same
    # air the text beside it gets.
    pictureAir = 6;

    # THE LARGEST A PREVIEW IS EVER DECODED TO, in PIXELS and not in cells —
    # the third block in this file measured that way, after `rule.thickness`
    # and the browser extension's popup.
    #
    # It has to be pixels: it is the size of a PICTURE, and a picture has no
    # cells. The widest a panel gets is about forty cells, which is 432 px at
    # `font.size.base`, so 512 is headroom and not appetite.
    #
    # It is also what bounds the GPU, and that is the half that is arithmetic
    # rather than taste. `ronoctua-render`'s image pipeline keeps a bounded
    # number of textures between frames, and 512 px of RGBA is 1 MiB each. At
    # 1200 px the same cache would be five times that.
    #
    # The decoder quantises what it is asked for to 128, 256 or 512, so that
    # dragging a window's edge does not decode a phone photograph again on
    # every `configure`.
    panelPixels = 512;

    # WHERE THE LIGHT ON A SHAPE COMES FROM, in whole DEGREES.
    #
    # Degrees and integers because that is what this file can carry — a colour
    # is `0xRRGGBBAA` and a space is a `u16` — and a direction is neither. The
    # alternative was a fourth kind of token, which means a change to the
    # generator, to `tokens.rs` and to `tokens.wit`, all three of which
    # `invalid-token-fails-build` compares. Two integers and a sine in Rust is
    # the smaller thing, and it is still a token (invariant 9).
    #
    # The angles are in CAMERA space, so the light never moves when the piece
    # does: a light fixed in the world puts a piece in its own shadow the moment
    # somebody turns it round, and the gesture meant to show the other side
    # would show a silhouette.
    #
    # Azimuth is anticlockwise from the right, so 155 is up and to the LEFT —
    # where light comes from in every drawing anybody has ever been taught to
    # make, and the reason a right-lit object looks like a photograph of a
    # product while a left-lit one looks like a thing on a table.
    meshLightAzimuth = 155;
    # Above the horizon. High enough that the top faces are the bright ones,
    # which is what says which way is up; not so high that the sides go flat.
    meshLightElevation = 40;

    # HOW MUCH COLOUR SURVIVES IN FULL SHADOW, in hundredths.
    #
    # Not zero. A face turned away should be dark and still be THERE, because
    # its edge against the next face is what shows the shape; at zero the two
    # merge into one black blob and a hollow piece looks solid.
    meshShadow = 22;

    # HOW WIDE AN EDGE IS DRAWN, in tenths of a pixel.
    #
    # A STEP and an IGES file hold no surface — they hold the curves that trim
    # one — so those two are drawn as a cage of lines rather than as a lit
    # solid. See `ronoctua_inspect::mesh::Wires`.
    #
    # Fifteen and not ten: the rasteriser takes two samples per pixel per side,
    # so a line one SAMPLE wide is half a pixel on screen and comes out of the
    # average as a grey ghost instead of an edge.
    meshWireWidth = 15;

    # HOW MUCH COLOUR IS LEFT ON THE FURTHEST EDGE, in hundredths.
    #
    # A cage has no faces, so the far side of the piece is drawn over the near
    # side and nothing says which is which — one flat colour reads as a tangle
    # of lines rather than as an object. Fading with depth is the only cue
    # available.
    #
    # Not zero, for the reason `meshShadow` is not zero: an edge that vanishes
    # takes a piece of the outline with it.
    meshWireFade = 35;

    # HOW TALL THE WAVEFORM IS OVER A COVER, in hundredths of the frame.
    #
    # A sound with artwork shows the artwork; the waves are the thing that says
    # it is RUNNING, and they only have to say it. A fifth leaves the sleeve
    # legible — a square cover in a tall panel still shows whole — while a
    # strip much thinner than this stops reading as movement at a panel's size
    # and becomes a flickering line.
    #
    # It is a fraction and not a number of pixels because the panel's height is
    # not fixed: the same file is 128 pixels tall in the footer and 768 in the
    # window, and a fixed strip would be the whole picture in one and a hairline
    # in the other.
    #
    # With no cover there is no strip: the waveform is given the whole frame,
    # and this number is not asked.
    waveHeight = 20;

    # FROM THIS WIDTH ON, THE TWO PANELS SIT ON THE RIGHT — one column each.
    # Below it they do not disappear: they move to a short strip along the
    # BOTTOM, side by side. The owner's sentence is binary, and so is this
    # number: right-hand columns or bottom strip, never one of each.
    #
    # The arithmetic, and it is arithmetic and not taste. A panel's floor is
    # `Dimensions  3024 x 4032` — the widest ordinary fact, twenty-three cells
    # — plus the air on its right, so twenty-four; two of them plus the rule
    # that separates each from what is on its left is 24 + 1 + 24 + 1 = 50.
    # What is left at 96 is 46 cells, which is EXACTLY what a listing wants to
    # keep the icon, the name at its floor, the size and the date:
    # 1 + (2+1) + (12+1) + (10+1) + (16+1) + 1. One cell narrower and the date
    # goes; that is the point where a listing squeezed between two panels stops
    # being a table and the strip is the better shape.
    #
    # It is a PREFERENCE, and not what holds: what holds is that nothing leaks
    # off the screen, and `files::right_columns` withdraws the pair ENTIRELY —
    # never half of one — when honouring it would push a column past the edge.
    panelsFrom = 96;

    # HOW MANY LINES THE BOTTOM STRIP TAKES, when the panels are down there.
    #
    # Four, and the number is the footer's own ceiling doing the arguing. It was
    # measured against a region of twelve: four left eight, which is the path,
    # the header and six rows of listing — a page one can still work in. Three
    # leaves a preview two lines tall once the file's name is on one of them,
    # and at that size a photograph is a smudge rather than a picture; five
    # leaves five rows, which is where the listing stops being the thing one
    # opened. The ceiling is fifteen now and the strip did not grow with it:
    # what the strip needs is a picture's box, which does not change because
    # there is more room above it — every line the region gained goes to the
    # LISTING, which is what one opened the manager to read.
    #
    # The window door has no ceiling and pays the same four: the strip is a
    # glance, not a second view, and a strip that grew with the window would
    # take the height the listing exists to use.
    stripLines = 4;

    # HOW WIDE A TABULATION IS, in cells, when the preview shows a file's text.
    #
    # Four. It is not taste and it is not the editor's setting: it is what makes
    # a nested block READ as nested in a column that may be twenty-four cells
    # wide. Eight — the terminal's ancient default — spends a third of the
    # narrowest panel on the second level of indentation alone, and code three
    # levels deep would arrive with nothing but whitespace on screen. Two is
    # below the eye's threshold for a step at this cell width; the two levels
    # stop being tellable apart, which is the whole thing indentation is for.
    #
    # It is a TOKEN because it is spacing on a grid of text, which is exactly
    # invariant 9's subject — the same reason `gutter` and `edgeRight` above are
    # tokens rather than a `1` written twice in Rust.
    #
    # And it is applied WHERE THE FILE IS READ, before the highlighter runs, not
    # where it is drawn: the highlighter's ranges are byte offsets into the
    # text, and expanding tabs afterwards would move every one of them.
    tabWidth = 4;

    # HOW WIDE ONE THUMBNAIL IS, in cells, at each of the three steps the size
    # key rings through.
    #
    # THREE NUMBERS AND NOT SIX. Only the width is declared; the height comes
    # out of it in `files::thumb_lines`, as the same box measured on the other
    # axis — a cell is `typography.cellWidthEm` of the font and a line is
    # `typography.lineHeight` of it, so a square picture is
    # `cells x 0.600 / 1.45` lines, and the name's line is added after. Writing
    # the height here as well would be a second visual value that could drift
    # from the first, and the drift is a picture that is not square.
    #
    # WHY THESE THREE. `small` is the smallest at which a photograph is still a
    # photograph rather than a smear: ten cells is six ems across, four lines of
    # picture, and at 100% on this machine that is 108 px — the size the
    # freedesktop thumbnail standard calls `normal`, arrived at independently
    # and a reassurance rather than a reason. `large` is twenty-four because at
    # 14,4 ems a 1920 px window still holds eleven of them across, and a step
    # that shows six items is a preview panel with extra steps. `medium` is the
    # one in between and is where the ring starts.
    #
    # They are CELLS and not pixels, unlike `panelPixels` above, and the
    # difference is which question is being answered: `panelPixels` bounds what
    # the DECODER is asked for, which is pixels because a texture is pixels;
    # this is how much of the text grid one item occupies, which is what every
    # other layout number in this block is.
    # WHY THESE THREE, AND WHY THE SMALLEST IS THIS SMALL.
    #
    # `small` is what a FOOTER can hold. The footer gives the manager twelve
    # lines; one is the path and four are the strip the two panels sit in when
    # they are down there, which leaves seven. A cell is its picture plus the
    # name's line, and the picture is as many lines as the width is ems — six
    # cells is 3,6 em across, which is three lines with the name. Seven lines
    # hold two of those; at ten cells they hold one, and the owner photographed
    # exactly that: *"a versão de rodapé … está mostrando apenas uma linha no
    # meu laptop em vez de duas"*.
    #
    # `large` is twenty because at twelve ems a 1920 px window still holds nine
    # of them across, and a step that shows six items is a preview panel with
    # extra steps. `medium` is the one in between and is where the ring starts —
    # a WINDOW opens there, and `files::View::thumb_item` steps DOWN to what a
    # short door can hold rather than making every door pay for the shortest.
    thumbSmall = 6;
    thumbMedium = 12;
    thumbLarge = 20;

    # HOW MANY THUMBNAILS MAY BE ON SCREEN AT ONCE.
    #
    # It is the GPU's number, and it is here because it is the one that decides
    # the grid: `ronoctua_render::image` keeps one texture and one draw call per
    # picture, and `prepare` DROPS what does not fit **with nothing in any log**
    # — the one silent failure in this feature. So the grid never asks for more
    # cells than this: on a screen that would hold more, the size steps up until
    # it fits, which on a 4K panel is bigger thumbnails and not missing ones.
    #
    # TWO HUNDRED AND FIFTY-SIX, and the number is the two screens this has to
    # work on doing the arguing. Counted out at the base font:
    #
    #   1920x1080  small 16x8 = 128   medium 10x5 =  50   large 7x3 = 21
    #   3840x2160  small 32x16 = 512  medium 20x10 = 200  large 14x7 = 98
    #
    # So 256 lets the smallest step be reachable on a 1080p screen — which a
    # ceiling of 120 would NOT, and the ring would have had two usable steps
    # out of three — and steps 4K's smallest up to its medium, which is the
    # right answer anyway: five hundred pictures at once is a contact sheet, not
    # a folder.
    #
    # The memory is the other half. A small thumbnail decodes to 128 px, which
    # is 64 KiB of RGBA, so a 1080p screenful is 8 MiB; a medium one decodes to
    # 256 px and 4K's two hundred of those are 50 MiB — against the 41 MiB
    # `panelPixels` above already argues is acceptable for the panels alone.
    thumbMost = 256;

    # HOW MANY THUMBNAILS ANIMATE BY THEMSELVES.
    #
    # An animated GIF, an animated WebP and an APNG all decode frame by frame
    # IN PROCESS, so one of them costs a file descriptor and a frame — never a
    # child process, which is what the pointer's clips cost and why those are
    # one at a time.
    #
    # Twelve, and it is measured rather than chosen. One frame of a
    # four-hundred-pixel GIF composited and scaled to a thumbnail is 2,35 ms on
    # this machine, and the decoder holds the full-size canvas it composites
    # into — 625 KiB for that file. Twelve at ten frames a second is 28% of one
    # core and about 7,5 MiB. The number that is NOT acceptable is the one this
    # ceiling exists to refuse: fifty GIFs with their frames decoded up front
    # is 215 MiB, measured on the owner's own sample.
    #
    # Above the ceiling a cell shows its still frame and says so with a mark. A
    # refusal that is drawn is a refusal; one that is silent is a bug.
    thumbAnimators = 12;

    # THE SLACK BETWEEN A CLICK AND A GESTURE, IN CELLS.
    #
    # A press on the listing is not yet a click: it becomes one when the button
    # comes up without the pointer having moved, and a GESTURE — a rubber band
    # from the air, a drag from an item — on the first motion past this. The
    # two have to be told apart at all, because a press can no longer act on
    # its own: until the pointer moves there is nothing that says which of them
    # it is.
    #
    # IN CELLS, because the view may hold no pixel: both doors quantise the
    # pointer before they hand it in, and a threshold in pixels would need a
    # third conversion and be the one number the two doors could get different.
    #
    # Two. A cell is about nine pixels, so this is the eighteen or so every
    # toolkit uses — and one cell would not do: a hand that trembles by a cell
    # on the way up would turn a click into a band, and a band MARKS the entry
    # instead of selecting it. Sideways only: one whole line down is already
    # past any tremor, and a shelf is four to six lines, so demanding a line
    # costs a real sweep nothing.
    sweepSlack = 2;

    # HOW MANY MEBIBYTES OF DECODED THUMBNAIL ARE KEPT IN MEMORY.
    #
    # Sixty-four. The host's cache is what stops a folder being decoded again
    # every time it is scrolled back to, and without a ceiling it is what the
    # icon cache already is — unbounded — except that an icon is a few kibibytes
    # and a thumbnail at the large step is 256. Four hundred of those is a
    # hundred mebibytes of file manager, which is the argument
    # `image::TEXTURE_CEILING` already makes about the clipboard's screenshots.
    #
    # Over the ceiling the policy is the coarse one `Images::dump` already uses:
    # keep what is on screen and let the rest go. A true LRU would need a clock
    # and a sort per frame to save a memcpy the disk cache has already made
    # cheap.
    thumbMemory = 64;

    # HOW LONG ONE CLIP OF A FILM LASTS under the pointer, in tenths of a
    # second.
    #
    # Twenty-five, so two and a half seconds. The pointer's preview walks a film
    # rather than playing it from the start: it seeks to a handful of evenly
    # spaced points and shows a little of each, which says what is IN the film
    # in a way the first frame — a title card, or black — never does.
    #
    # Tenths because this block is integers and a second is too coarse: one
    # second reads as a stutter and four is long enough that nobody waits for
    # the second clip. Measured: seeking anywhere in a ten-minute film and
    # decoding costs 137-143 ms, so the cut between clips is a fifth of what is
    # shown, which reads as a cut rather than a stall.
    thumbClipTenths = 25;
  };

  # The TRANSFER queue's row, in cells.
  #
  # `bar` is how many cells the progress bar occupies. Twelve, and the number is
  # not taste: the bar is TILED — one glyph per cell, invariant 9 reaches the
  # codepoint too — so its resolution is exactly its width. Twelve cells means
  # the smallest visible step is 8.3%, which is the granularity a person reads
  # off a bar anyway; the exact figure is next to it in digits, and that is what
  # answers "is it moving".
  #
  # Wider was tried in the arithmetic and does not pay: at twelve, a row still
  # fits the name, the destination, the bar, the percentage and the size inside
  # eighty columns, which is the narrow terminal every layout here is checked
  # against.
  transfers = {
    bar = 12;
  };

  # WHERE CONTENT STARTS, in cells. ONE NUMBER, EVERY SURFACE.
  #
  # It is not box padding (invariant 15): it is where the text begins inside
  # whatever it was given — a compositor's window, or the footer's expanded
  # region.
  #
  # # It used to be called `buffer.marginX`, and the name was the defect
  #
  # Named after the buffer, it reached the buffers and nothing else. The
  # footer's own region reached for `modeline.edgeLeft` instead, and the file
  # manager's listing for `files.gutter` — so the SAME view, drawn in the
  # footer and drawn in its own window, began at two different distances from
  # the screen's edge. Measured: 10,8 px in the footer against 24,0 px in the
  # window, a factor of 2,2. The owner reported it as "in the minibuffer region
  # it comes out glued to the edge, and the window version has different
  # spacing" — and he was reading two numbers, not one number badly applied.
  #
  # The residue, after this: NONE, and there used to be a fifth of a cell of
  # it. The footer's cell was `font.size.base` and a window's `font.size.large`
  # — 21,6 px against 24,0 px for the same two cells — and that difference was
  # the grid's rather than the layout's, so it was named and allowed. The owner
  # then asked for one measure across the system, the second size went, and two
  # cells are 21,6 px on every surface this shell draws.
  #
  # # The one exception, and it is written down
  #
  # `modeline.edgeLeft`/`edgeRight` still apply to the modeline and the
  # minibuffer, and only to them. Those two are a STATUS LINE and not content —
  # their own comment already argues that two cells there "would start to look
  # like a margin". A status line is peripheral reference; content is what one
  # reads.
  #
  # # HORIZONTAL YES, VERTICAL NO — and the asymmetry belongs to the FONT
  #
  # Horizontally the ink occupies the whole advance: the Nerd glyphs go from 0 to
  # 600 units in an advance of 600, with no lateral slack at all. With no inset the
  # first character touches the edge, and it looks like clipping.
  #
  # Vertically the slack ALREADY COMES IN THE LINE BOX: the font's box is 1.25 em
  # (ascender 0.965 + descender 0.285) and the line height is 1.45, so every
  # run already carries 0.10 em of air above and 0.10 below. One cell on top of
  # that is slack over slack — it becomes a blank line at the top and another at
  # the end, which is exactly what showed up on screen.
  content = {
    inset = 2;
    insetY = 0;
  };

  # WHERE A VALUE BEGINS, in the row's own cells — the network, power and
  # Bluetooth views, which are lists of a label and a value.
  #
  # # Why it is a column and not a right edge
  #
  # It was the right EDGE, and each of the three wrote the arithmetic itself: a
  # short value was pushed to the last cell of the surface and a long one fell
  # back to a third of the row. On a wide window that puts `off` a hundred and
  # fifty columns from `aeroplane mode`, with nothing in between, and the owner
  # read it off the screen: *"as informações estão alinhadas na extrema direita,
  # muito distantes"*. Two ragged edges and no column to run the eye down.
  #
  # A fixed column makes the three views a definition list: every value starts
  # in the same place, so reading down a section is one movement instead of a
  # zigzag. What it costs is that a long label is cut here rather than growing —
  # which is the right trade, because a label is a NAME and names are short by
  # the time they are good.
  #
  # # Why this number
  #
  # Twenty-eight cells is the longest label these three views need once they
  # say what they mean — `endereço desta máquina`, `velocidade máxima` — plus
  # the air after it. It is CLAMPED to half the row, so the narrow footer does
  # not become two columns of nothing; below that clamp the value simply gets
  # the other half.
  #
  # IT IS A MINIMUM, NOT A CEILING. A label longer than this pushes the value
  # right instead of being cut: the column is what makes a section readable
  # down one line, and a label cut to make it is a worse trade than one row
  # whose value sits further along. `power-keep-awake` is 36 cells in English
  # and 50 in Portuguese — a sentence, beside a two-letter value — and the
  # first version of this cut it to `keep awake: no s…`, which is the very
  # defect the comment deleted from `power.rs` had been written about.
  view = {
    labelCells = 28;
    # HOW MUCH ROOM A VALUE GETS, in the row's own cells.
    #
    # `labelCells` says where a value BEGINS; this says how far it may run. On
    # a row drawn straight onto the surface nothing needs it — what is left of
    # the line is the answer — and it exists for the one place where something
    # else does: a CONTAINER (`shelf`) has a right edge of its own, so somebody
    # has to say where that edge is, and the honest answer is "as far as one of
    # its rows needs".
    #
    # # Why forty
    #
    # It is the longest value these lists carry. A tailnet peer's is four
    # facts — `online · Linux · 100.64.12.34 · saída` is 38 cells — and a
    # neighbour's is three, with a manufacturer's name in the middle of it.
    # Below this the container would cut, on a screen with room to spare, the
    # very line somebody opened it to read.
    valueCells = 40;
    # HOW FAR A SUB-SECTION IS SET IN, in cells.
    #
    # # Why a list of settings ever needs a second level
    #
    # Because one of them has repeating parts. The glance's section is a list
    # of PLACES, each with four switches and a way to remove it — and drawn
    # flat, with the section's title and every place's name in the same tone at
    # the same column, there is nothing on the screen saying which rows belong
    # to which place, or that the whole block is one section. The owner read
    # exactly that: the main title was identical to every sub-title under it,
    # and the content below could not be made out as a section at all.
    #
    # # And why the device is air and colour
    #
    # Because those are the two this system has. A title is the size of the
    # body text — hierarchy is never size and never a rule — so what is left is
    # WHERE a row starts and WHAT TONE it is in. Both are already how every
    # other list here says the same thing.
    #
    # TWO CELLS: one is not read as an indent at all next to a label column of
    # twenty-eight, and three starts to look like a column of its own.
    nestCells = 2;
  };

  # A LIST WITH A CEILING OF ITS OWN, inside the network view's one list.
  #
  # # What it is for
  #
  # Wi-Fi is read continuously and the answer changes: a neighbour's router
  # comes and goes, a phone's hotspot appears, the signal reorders them. With
  # the list free to grow, everything under it moved up and down while nobody
  # touched anything — the owner read it off the screen: *"como as redes
  # aparecem e desaparecem, acaba fazendo o resto do conteúdo descer e subir"*.
  #
  # A shelf always draws the SAME number of lines, padding with blanks when it
  # holds fewer, and scrolls inside itself with a bar of its own. Nothing under
  # it ever moves. That is the whole property, and it is why the number is
  # fixed rather than a share of the room: a height that follows the content is
  # the thing being fixed.
  #
  # # Why five
  #
  # Measured against a region of twelve (`expansion.maxLines`, fifteen now), a
  # shelf is a little under half of it: enough to compare a handful of networks
  # at a glance, and short enough that two shelves and the rows between them
  # still fit on one page. It was six, and the owner asked for a little less.
  # The taller ceiling does not move it — a shelf is a fixed window onto a list,
  # and a window that grew with the room would be the height-follows-content
  # this block exists to refuse.
  #
  # ONE NUMBER FOR ALL FIVE. A shelf that was taller than its neighbours would
  # be saying something about its content that is not true — they are all the
  # same kind of thing, and the eye should not have to ask.
  #
  # # AND ITS WIDTH IS NOT WRITTEN HERE, on purpose
  #
  # A container is as wide as one of its rows needs — `view.labelCells` plus
  # `view.valueCells`, plus the air at each edge and the cell its own bar
  # takes. That is arithmetic over tokens that already exist,
  # so it lives in `chrome::SHELF_ROOM` beside the code that draws it, the way
  # `chrome::FIELDS_LEAD` does. A number repeated here would be a second truth
  # that goes stale the day either column moves.
  shelf = {
    lines = 5;
    # THE AIR INSIDE A CONTAINER, in cells: before its first glyph, and after
    # its last text cell and before its own bar. It is spent once more on the
    # other side of that bar, so that on a narrow screen — where the box is
    # clamped rather than measured — the container's bar and the page's are
    # still two bars with a gap between them.
    #
    # The reason is the one every drawing here owes, and it is the same one
    # `files.gutter` gives: ink against a border reads as ink that was CUT, not
    # as ink that ended.
    pad = 1;
  };

  # THE GLANCE'S LINE — `LEADER d`, one row above the modeline.
  #
  # # The air inside a place's ground
  #
  # The places sit side by side on one row and every other one is drawn on a
  # ground of its own, because a separator alone left the eye a row of words
  # with nothing to group them by (see `colors.nix`, `glance`). That ground is
  # a RECTANGLE, and a rectangle that starts where the first letter starts is
  # a rectangle the text is glued to — the same fault `shelf.pad` names one
  # block above and `files.gutter` names further up: ink against an edge reads
  # as ink that was CUT.
  #
  # ONE NUMBER FOR BOTH SIDES, and that is the whole point of it being one:
  # the owner measured the two and they differed — nothing to the left of the
  # text and two cells to the right, because the ground ran on to where the
  # next place began. Spending it twice out of one constant is what makes
  # "the same air on both sides" a thing that cannot drift.
  #
  # AND ONE CELL, not two, for the reason `modeline.edgeRight` gives about
  # itself: this is a status line and two would start to look like a margin.
  # It also makes the arithmetic exact against `modeline.segmentGap`, which is
  # two: a place's ground ends exactly where the next one's begins, so the
  # alternation reads as a band changing tone rather than as two boxes with a
  # gap between them.
  glance = {
    air = 1;
  };

  # HOW FAR THE EXPANSION MAY GROW — the which-key list, the `M-x`
  # candidates, the history, the manual.
  #
  # # Why there is a ceiling
  #
  # Without it the list grows with the content, and an `M-x` with 170 commands would take
  # the whole screen: the footer would stop being a footer, which is what invariant
  # 14 forbids. Whatever goes past the ceiling becomes a PAGE (CtrlRight+arrows).
  #
  # # Why there are TWO numbers
  #
  # `maxLines` on its own is blind to the screen's size: 12 lines of 26 px are a
  # third of a 1080 px monitor and MORE THAN HALF of a 600 px one.
  # `maxFooterPct` sets the second limit, and the smaller of the two wins.
  #
  # And the percentage is of the WHOLE FOOTER, and not just the expansion: the two fixed
  # lines count for whoever is looking at the screen. Measuring only the expansion, 40% became 46%
  # of footer on an 800 px screen — within the number and outside what it means.
  #
  # # Why fifteen and not twelve
  #
  # It was twelve, and the owner asked for three more — *"quero aumentar um
  # pouco o tamanho máximo alcançável pela região do rodapé … quero que esse
  # aumento dê à coluna info do filemanager três linhas novas de dados"*. The
  # facts column takes every line of the region the chrome does not
  # (`View::info_room`), so the raise arrives there one for one; nothing else
  # had to move for it.
  #
  # THE SECOND LIMIT IS WHAT MAKES THIS HONEST. Fifteen lines plus the two
  # fixed ones are 443,7 px, which is 37% of a 1200 px panel and 41% of a
  # 1080 px one — so the percentage keeps binding first exactly where it should,
  # and on a small screen this number is not reached at all. It is a CEILING,
  # not a height: raising it does not make any footer taller than invariant 14
  # allows, it only stops that invariant from being the one thing standing
  # between a person and three more facts on a screen with room for them.
  # `the_ceiling_this_screen_was_raised_for_is_reachable` is what keeps the two
  # numbers in step — raise this one far enough and the percentage swallows it
  # in silence, which would be a token that no longer says what happens.
  expansion = {
    maxLines = 15;
    maxFooterPct = 40;
  };

  # THE NOTIFICATION MANAGER, in cells.
  #
  # From this width on it splits into two columns — what happened on the left,
  # what a filter suppressed on the right; below it, the two stack. The same
  # number serves BOTH doors, which is what makes "one implementation, two
  # doors" true of the layout and not only of the code: the footer's region on a
  # small display and a narrow window get the same answer.
  #
  # Eighty is the narrow terminal every layout here is checked against, and it
  # is the width at which a sentence plus a filter's name stops fitting beside a
  # second list.
  #
  # AND WHERE THE CUT FALLS, as a percentage of the width. `split_at` used to
  # be `columns * 3 / 5` written in Rust, eleven lines under a doc comment
  # saying the cut comes from a token — invariant 9 names proportion and
  # spacing, and `files.sidebarFrom`/`panelsFrom` are the precedent for
  # putting a layout number here.
  #
  # Sixty is not a taste: the left column holds SENTENCES — a summary and a
  # body — and the right one holds what a filter kept out, which is a
  # reference list read rarely and never at length.
  notifications = {
    # FROM WHAT WIDTH THE TWO COLUMNS OF THE LIST ARE WORTH HAVING.
    #
    # It was 80, and the owner photographed what that buys: nine rows reading
    # `bluetooth: Bluetooth Keyboard connect…`, every one of them cut at the
    # same word and none of them telling him which was which. Two columns that
    # cost the sentence are two columns that destroy what the list is for.
    #
    # MEASURED, in the arithmetic `extent_of` does. The left column is
    # `splitAtPct` of the width, less the lead, the gap and the row's furniture
    # (the severity glyph, the mark, the two targets); what is left is the
    # sentence. At 80 that is 37 cells and `bluetooth: Bluetooth Keyboard
    # connected` is 39, so it cuts. At 96 it is 46, and the same sentence fits
    # with air to spare.
    #
    # And 96 is not a new number: it is `files.panelsFrom`, where this system
    # already decided a second column of CONTENT starts being worth its width.
    twoColumnsFrom = 96;
    splitAtPct = 60;

    # HOW MANY LINES OF A BODY AN OPEN RECORD WILL WRAP.
    #
    # A bound on WORK and not a layout decision — the same distinction the file
    # manager's preview draws. A notification body is a sentence or two; two
    # hundred lines is far past anything an application sends, and wrapping a
    # megabyte to draw a screenful is a cost that only shows up on somebody
    # else's machine.
    bodyLines = 200;

    # FROM WHAT WIDTH AN OPEN RECORD PUTS ITS FACTS IN A COLUMN OF THEIR OWN.
    #
    # Not `twoColumnsFrom`: that one is about the LIST, where a column holds
    # one elided sentence and 80 is enough. A fact is a label AND a value on
    # one line — `filter that decided` is nineteen cells and
    # `none of them matched` is twenty — so the column needs about forty, and
    # it gets `100 - splitAtPct` of the width. Forty divided by 0.40 is a
    # hundred, and four cells of margin make it 104.
    #
    # Below that the facts go UNDERNEATH the message and its actions, which is
    # the same answer the list gives when its own two columns do not fit, and
    # the answer the file manager gives when its panels do not.
    factsFrom = 104;

    # HOW MUCH OF A ROW THE SENTENCE KEEPS, whatever else wants the line.
    #
    # A row with buttons has three things on it: what happened, what you can do
    # about it, and the two targets at the end. The buttons are drawn only while
    # this much is left for the sentence — below it the row would be a column of
    # brackets beside an ellipsis, and the open record shows every button anyway.
    # Twenty-four cells is `bluetooth: Bluetooth Keyboa…`, which still says which
    # notification the row is.
    sentenceFloor = 24;
  };

  # THE RULE'S THICKNESS, and this is the SECOND block in this file measured in
  # PIXELS rather than in cells (the browser extension's, below, was the first).
  #
  # It has to be, and the reason is what changed about the rule. It used to be
  # drawn as a line of `─` glyphs — a GLYPH, so it occupied a whole cell, began
  # wherever the text's margin began, and was eighty cells long whatever the
  # window measured. That is three defects in one line: a gap on the left, a
  # spill on the right below eighty-two columns, and a thickness nobody chose.
  #
  # It is a quad now, drawn edge to edge at the BOTTOM of the row it names, and
  # a quad's height is a distance and not a count of cells. One logical pixel is
  # what `colors.nix` has always described it as: "One line, one logical pixel."
  # THE DEVICE PAGE'S SECOND COLUMN — what you can ADJUST about this thing.
  #
  # # Why it is a column and not a level
  #
  # A device page answers two different questions that a person asks at the
  # same time: *what is this, and is it working* (the facts) and *turn it
  # down* (the settings). Down a level, the second question costs a key press
  # and a screen; beside the first, it costs nothing and the answer to "is it
  # connected" stays visible while you change the volume.
  #
  # # Why 88 and not the notifications' 96
  #
  # Because the halves are not the same shape. A notification row is an elided
  # SENTENCE, and cutting it destroys what the list is for; these two columns
  # hold label-and-value rows, which the `view.labelCells` arithmetic already
  # sizes at 28 + the value. Measured against the widest pair on either side:
  # `what it can do` beside `plays sound from here` is 36 cells, and the
  # settings' longest is `volume` beside `72%`. At 88 the left column gets 52
  # and the right 32, and both fit; at 80 the right one has 28 and `wake on
  # its own` (16) plus a value stops fitting with air.
  #
  # Below it the settings go UNDERNEATH the facts, which is the answer the
  # notifications' facts give and the answer the file manager's panels give.
  bluetooth = {
    settingsFrom = 88;
    # The FACTS get the wider half: a fact is a label and a value, and the
    # labels there are sentences (`can be woken by this device`) while the
    # settings' are one or two words.
    splitAtPct = 60;
  };

  rule = {
    thickness = 1;
  };

  # THE BROWSER EXTENSION'S POPUP, and this is the THIRD block in this file
  # measured in PIXELS rather than in cells.
  #
  # Everything else here is a count of terminal cells, because everything else
  # is drawn on a cell grid. A Chrome popup is a web page and has no cells —
  # but invariant 9 does not stop at the browser's edge, so the two numbers it
  # needs live here with the rest rather than inside a stylesheet where nobody
  # would look for them.
  extension = {
    tight = 6;
    gap = 10;
  };
}
