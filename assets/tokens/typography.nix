# Typography tokens — sections 5.1, 6.2 and 4.1.
#
# NO CRATE NAMES THE FONT DIRECTLY (section 6.2). The family's name
# appears once, here, and reaches the Rust as a generated token.
{
  family = {
    # The patched one, which is the default everywhere (section 6.2).
    mono = "NoctuaIosevka Nerd Font Mono";
    # The UI is the same font: the whole system is monospaced by design
    # (terminal aesthetics, section 5.1).
    ui = "NoctuaIosevka Nerd Font Mono";
  };

  size = {
    # Base size in logical px. The footer uses this one.
    #
    # 18 and not 14: at 14 px the footer was illegible on a 1920x1200 panel
    # at working distance, and the glyph browser showed twenty tiny
    # lines where many fitted. The layout is integer arithmetic over
    # the cell, so changing the size does not touch any of the sums.
    base = 18;
    small = 15;
    # The buffers' content. LARGER than the footer on purpose: the footer is
    # peripheral reference, the buffer is what one reads.
    large = 20;
    # THE RUN that asks for `escala: grande` — today only the glyph in the browser.
    #
    # There the content IS the icon's shape: a 20 px glyph is a text
    # character, and what one is looking for in a grid of 10,473 is the DRAWING. At 40 px the
    # shape can be told apart without bringing your face closer to the screen, and hundreds
    # still fit.
    #
    # It does NOT hold for the whole buffer. That is how it started, and the browser's header and
    # footer grew along with it — interface text at 32 point,
    # which is uncomfortable to read and overflowed the line as well. What grows is the
    # run; the cell grid stays the `large` one.
    #
    # What chooses between `large` and `xlarge` is the HOST, from the name the
    # run declares — invariant 11 does not let the value cross over.
    xlarge = 40;
    # The glyph's SHEET — the glyph, there, is the subject of the whole screen.
    #
    # 40 px is the GRID's size, where it shares space with hundreds of
    # siblings. On the sheet there is only one, and what one wants is to see the drawing: the curves,
    # the stroke's thickness, whether it is outlined or filled.
    #
    # 116 is exactly FOUR cell lines (4 x 20 x 1.45). The run
    # occupies a whole number of cells, as the grid's `ALTURA: 2` already does
    # — without that, whatever comes below the glyph falls on top of it.
    xxlarge = 116;
  };

  # -------------------------------------------------------------------
  # THE CONSTANT THAT HOLDS UP THE FOOTER'S LAYOUT
  # -------------------------------------------------------------------
  # Measured, not assumed: the 135 faces have ONE advance per width, and at
  # normal width it is 600/1000 em = exactly 0.600 em — including in the
  # 10,473 Nerd Fonts glyphs, which are one cell wide (Mono variant).
  #
  # With that, positioning segments in the modeline is INTEGER ARITHMETIC:
  # there is no need to measure text. It is what buys the "one frame" budget
  # of section 5.2 for which-key and completion.
  #
  # The `fonts-metrics` check reasserts this on every build. If the font is
  # swapped for another build, it fails and forces a review of the layout BEFORE
  # the footer falls out of line silently.
  cellWidthEm = 0.6;

  # Line height in multiples of the font size. The footer has
  # EXACTLY two of these lines (section 4.1).
  #
  # 1.45 and not 1.25: with the lines touching, the two-line footer looked like
  # a single block, and the which-key list became a wall of text.
  lineHeight = 1.45;
}
