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
    # rather than taste. `ronoctua-render`'s image pipeline keeps thirty-nine
    # textures between frames; 512 px of RGBA is 1 MiB each, so 41 MiB. At
    # 1200 px the same thirty-nine would be 224 MiB of footer.
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
    # Four, and the number is the footer's own ceiling doing the arguing.
    # `expansion.maxLines` is twelve; four leaves eight, which is the path, the
    # header and six rows of listing — a page one can still work in. Three
    # leaves a preview two lines tall once the file's name is on one of them,
    # and at that size a photograph is a smudge rather than a picture; five
    # leaves five rows, which is where the listing stops being the thing one
    # opened.
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
  expansion = {
    maxLines = 12;
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
    twoColumnsFrom = 80;
    splitAtPct = 60;
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
