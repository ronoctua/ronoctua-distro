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
    # Base size in logical px. EVERYTHING the host draws uses this one — the
    # footer, all four of its own windows, and a buffer's body.
    #
    # 18 and not 14: at 14 px the footer was illegible on a 1920x1200 panel
    # at working distance, and the glyph browser showed twenty tiny
    # lines where many fitted. The layout is integer arithmetic over
    # the cell, so changing the size does not touch any of the sums.
    #
    # THERE USED TO BE A SECOND BODY SIZE, `large = 20`, and windows and
    # buffers drew at it: "the footer is peripheral reference, the buffer is
    # what one reads". The owner photographed the file manager's two doors side
    # by side and asked for one measure — the footer's, which is the one he
    # reads all day. So the two names became one, and with it went
    # `chrome::Door`, which existed only to say that the doors differed here.
    base = 18;
    small = 15;
    # THE SIZE A CHROME GLYPH IS DRAWN AT, next to text of the base size.
    #
    # It is not taste, it is a MEASUREMENT. In the real face the ink of
    # `cod-bell` is 600 units tall and the ink of the digit `0` is 751 — so a bell
    # drawn at the text's size comes out 20% shorter than the number beside it,
    # and reads as a different, smaller thing. It was reported from the screen
    # exactly like that.
    #
    # 22 makes the ink 13,2 px against the digit's 13,5, which is the same
    # thing to the eye. The advance grows past the cell (13,2 against 10,8), so
    # the run has to be CENTRED on its cell — `render::Size::OnTheCell` — and
    # then the ink overflows 0,3 px on each side, while the neighbouring digit's
    # ink only begins 1,4 px into its own cell. They do not touch.
    #
    # The `chrome-icons-match-the-text` check re-measures the glyphs it LISTS
    # against the font on every build, so one of those drawn out of scale
    # fails there instead of on somebody's screen. The list is by hand, and
    # it is the chrome's own glyphs: a token nothing draws inline is not
    # measured, and does not need to be.
    #
    # AND IT IS A CEILING. The vertical battery showed the other half of the
    # rule: 908 units of ink, taller than the digit at the text's own size, and
    # at 22 px it stood 48% above the number beside it. So the rule is written
    # once, in `gen-glyphdb.py`: this size, unless it would put the glyph's ink
    # above the band below — then the size that brings the ink to the digit's.
    # 22 for the bell, 15 for the battery. The shell asks
    # `tokens::icon::chrome_size`, generated from that, for every leading
    # glyph of the modeline, the band and the brain line, and for the two
    # chevrons — the runs centred on a cell. Rows of a listing are text at
    # the base size and are not touched.
    icon = 22;
    # THE RUN that asks for `scale: large` — today only the glyph in the browser.
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
    # 104.4 is exactly FOUR cell lines (4 x 18 x 1.45). The run
    # occupies a whole number of cells, as the grid's `HEIGHT: 2` already does
    # — without that, whatever comes below the glyph falls on top of it.
    #
    # It was 116, which was four cell lines back when a buffer's cell was the
    # 20 px one. The tie is what matters and not the number, so it moved with
    # the body — otherwise a `huge` run would reserve five cells to draw four
    # and a half, and the sheet would grow a band of nothing under the glyph.
    xxlarge = 104.4;
  };

  # TYPOGRAPHIC MARKS the host punctuates WITH, rather than draws as content.
  #
  # HOW FAR FROM THE DIGIT'S HEIGHT a chrome glyph may stand, in percent, both
  # ways. Not "equal": a bell, a swap arrow and a pie are different families
  # of drawing and will not agree to the unit; what the band excludes is what
  # the eye reads as another size. `chrome-icons-match-the-text` measures
  # every listed glyph against it, and `gen-glyphdb.py` brings a glyph down
  # only when the icon size would put its ink ABOVE the band — a drawing
  # under the band is not brought up, because the icon size is the ceiling.
  chromeTolerance = 15;

  # A mark here is not an icon: `assets/tokens/icons.nix` maps a token to a
  # NERD FONTS GLYPH NAME, resolved against the pinned `glyphnames.json`, and
  # `·` is not in that catalogue — it is U+00B7, an ordinary character of the
  # text face. It is a token all the same, because invariant 9 reaches the
  # codepoint, and because this one now carries a COLOUR of its own.
  #
  # `separator` is the dot between one shortcut and the next on the hint line.
  # It used to live inside the `.ftl` values, and the argument for that was
  # written down: a `·` typed in Rust would be a visual literal. What the
  # argument missed is that a separator inside a sentence cannot be given a
  # colour of its own, and cannot be counted — the owner asked for exactly two
  # things the value could not provide: one shortcut between any two dots, and
  # a third tone for the dot itself. Both are properties of the LINE, not of
  # the sentence, so the dot moved out to where the line is assembled and the
  # rule became structural instead of a convention two languages had to
  # remember.
  #
  # Host only. It reaches no WIT enum and no buffer: invariant 11 does not let
  # a token's value cross that boundary, the same as `family.mono` above.
  marks = {
    separator = "·";
    # THE FOUR ARROWS, as a key is DRAWN rather than as it is written down.
    #
    # `ronoctua_keymap::Key::notation` answers `<down>`, and it is right to:
    # that is the wire form, the one `Key::parse` reads back, and it has to
    # survive a round trip through a table and a `.ftl`. It is six cells of
    # markup on a screen, and on a hint line that now teaches forty-one keys
    # the four arrows would spend twenty-four cells saying what four glyphs say.
    #
    # Single-advance characters (U+2190..2193), not the long forms — those are
    # among the twenty-three double-advance ones the glyph browser measured, and
    # a key drawn two cells wide would place the next piece early.
    #
    # The map from a key to its drawing lives in `chrome::key_text`, one place,
    # the same arrangement as `chrome::syntax_color` and `chrome::hint_spans`.
    "arrow-left" = "←";
    "arrow-right" = "→";
    "arrow-up" = "↑";
    "arrow-down" = "↓";
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
