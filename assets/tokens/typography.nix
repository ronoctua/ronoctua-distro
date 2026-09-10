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
    # THE FOREIGN TERMINAL'S SIZE, in POINTS and not pixels.
    #
    # Ghostty is not ours: it draws with its own renderer and takes a `font-size`
    # in points (`nix/modules/terminal.nix` writes it, and the manual is explicit
    # that the unit is points). Every other number in this block is logical
    # pixels, which is why this one says so in its name.
    #
    # It is here, and not a literal in the module, because TWO things need it:
    # the module that writes Ghostty's config, and `toolkits::tell_them`, which
    # scales it when the person moves the size of the text. A number written in
    # both places is a number that drifts, and the drift would show as a
    # terminal at a different size from everything around it.
    terminalPoints = 11;
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

  # HOW FAR THE PERSON MAY TAKE THE TEXT, in percent of `size.base`.
  #
  # The base above is what the owner reads all day; this is the room to move
  # it, and it is a token because a size is a visual value (invariant 9) even
  # when the person chooses it. `size.base` stays the anchor: every other size
  # in this file is measured against it, so scaling the base scales the whole
  # arrangement and no sum has to change.
  #
  # # Where the ceiling comes from, and it is arithmetic
  #
  # A cell is `size x 0.600`. On a 1920 px screen at 100% the cell is 10,8 px
  # and there are 177 columns; at 200% it is 21,6 and there are 88. The
  # notification manager and the file manager both put their second column
  # away below 96 (`space.notifications.twoColumnsFrom`,
  # `space.files.panelsFrom`), so 200% is where a 1920 px laptop becomes a
  # one-column machine. That is not a defect — the layout is built to fold —
  # but it is the point past which the screen stops being the screen the rest
  # of this file was measured on, so it is where the range ends.
  #
  # The floor is the other end of the same argument: at 70% the cell is 7,6 px
  # and a line of the footer is 18,3 px tall, so its two lines are 37. It is small, and it is meant to
  # be — somebody with a 4K panel at arm's length asked for it once.
  #
  # THE STEP IS FIVE because the arrows have to arrive at 100 exactly from
  # both directions, and every number in the range has to be reachable by
  # pressing one key repeatedly. Five divides the distance to both ends.
  scale = {
    floor = 70;
    ceiling = 200;
    step = 5;
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
    # THE MULTIPLICATION SIGN, for a resolution: `1920x1200` is written with
    # U+00D7 and not with the letter x. It is a token for the reason the
    # separator above is one — invariant 9 reaches the codepoint — and it is
    # here rather than in a `.ftl` because a resolution is not a sentence: it
    # is one number, a sign, and another number, in every language.
    times = "×";
    # THE BAR, and there is ONE of it in the system.
    #
    # A volume, a microphone's level and anything else that is "this much of
    # that" are the same drawing at different scales, and `chrome::meter` is
    # the one function that builds it. They are here and not in
    # `assets/tokens/icons.nix` for the reason the separator is: that file
    # maps a token to a NERD FONTS GLYPH NAME, and these are ordinary
    # characters of the text face.
    #
    # Not a rectangle of quads, either. Invariant 10 says the icons of this
    # interface are glyphs of the font, and a bar drawn as quads would need
    # its own colour, its own rounding and its own answer to how wide a cell
    # is — three questions the text pipeline has already answered.
    #
    # MEASURED in the real face (`NoctuaIosevkaNerdFontMono-Regular.ttf`,
    # upem 1000): both advance **600**, which is exactly one cell. They are
    # also East_Asian_Width AMBIGUOUS, which `ronoctua_protocol::cells` calls
    # NARROW on purpose — so a bar of n glyphs is n cells and the budget
    # arithmetic never has to know it is a bar.
    "meter-full" = "▮";
    "meter-empty" = "▯";
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
