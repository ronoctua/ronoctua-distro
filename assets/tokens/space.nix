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
    # Margin at the screen's edges. The notification icon sits flush against the right
    # edge (4.2), so the right margin is zero on purpose.
    edgeLeft = 1;
    edgeRight = 0;
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
}
