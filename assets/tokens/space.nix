# Spacing tokens — section 5.1.
#
# "Separation by whitespace and one-line rules, never by a container."
#
# The scale is in CELLS, not in pixels. Since the cell is exactly 0.600 em
# (see typography.nix), spacing in cells keeps everything aligned to the text's
# grid without measuring anything — which is what the footer needs in order to be integer
# arithmetic.
{
  # Scale in cells.
  cell = {
    "0" = 0;
    "1" = 1;
    "2" = 2;
    "3" = 3;
    "4" = 4;
    "6" = 6;
    "8" = 8;
  };

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

  # The content's margin inside a BUFFER, in cells.
  #
  # It is not box padding (invariant 15): it is where the text starts inside the
  # window the compositor gave.
  #
  # HORIZONTAL YES, VERTICAL NO — and the asymmetry belongs to the FONT, not to taste.
  #
  # Horizontally the ink occupies the whole advance: the Nerd glyphs go from 0 to
  # 600 units in an advance of 600, with no lateral slack at all. With no margin the
  # first character touches the window's edge, and it looks like clipping.
  #
  # Vertically the slack ALREADY COMES IN THE LINE BOX: the font's box is 1.25 em
  # (ascender 0.965 + descender 0.285) and the line height is 1.45, so every
  # run already carries 0.10 em of air above and 0.10 below. One cell of
  # margin on top of that is slack over slack — it becomes a blank line at the
  # top and another at the end of the buffer, which is exactly what showed up on screen.
  buffer = {
    marginX = 2;
    marginY = 0;
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
  notifications = {
    twoColumnsFrom = 80;
  };

  # THE BROWSER EXTENSION'S POPUP, and this is the one block in this file
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
