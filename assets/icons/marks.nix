# The marks this repository DRAWS — section 6.3, the half no font has.
#
# WHY A DRAWING LIVES HERE AT ALL
# -------------------------------
# Invariant 10: a UI icon is a glyph of a font, and no design system component
# consults a pixmap. Invariant 9: a codepoint is a token. Between them there is
# no room for "just this once put a PNG on the modeline" — so when a mark the
# interface has to draw exists in none of the five faces, the answer is to draw
# it INTO a face of ours. `nix/marksgen.nix` builds `NoctuaMarks.ttf` out of
# what is declared here, `nix/glyphgen.nix` resolves `ours-<name>` against it,
# and from there on it is a glyph like any other: a token, a codepoint checked
# against a real cmap, an ink measurement, a chrome size.
#
# The precedent is `nix/lucidegen.nix`, which already builds a face of its own
# and already remaps a range — this one has no upstream font to start from, and
# that is the whole difference.
#
# WHY A GRID AND NOT A PATH
# -------------------------
# Because a path is a thing nobody can read or tune. Every mark here is dots on
# a square grid, which is three lines you can SEE, and the generator turns them
# into contours. The day a mark needs a curve that is not a circle, it will need
# a path — and a reason written next to it.
#
# THE ADVANCE IS NOT OURS TO CHOOSE
# ---------------------------------
# Measured over the 6896 `md-` glyphs of the built face: the ADVANCE is 600 for
# every single one, and that is what decides how many cells a glyph takes. The
# ink box is not constant at all — 747 distinct boxes — so it serves as a
# CEILING and not as a template: the generator requires the advance to match a
# reference glyph's and the ink to fit inside its box. A mark may be smaller
# than its neighbours; wider than the cell it would draw into the letter next
# to it.
{
  # Tailscale — the tailnet's mark on the modeline (`LEADER s n`, section 5).
  #
  # WHAT IT REPLACED, AND WHY THAT WAS NOT ENOUGH. It was `md-dots_grid`, a
  # uniform three-by-three of dots, and the owner read it off the screen as a
  # MENU icon — which is exactly what a uniform dot grid is everywhere else in
  # a desktop. The mark's own drawing is not uniform: the middle row and the
  # dot under its centre are solid and the other five are light, which is the
  # shape that stops it reading as a menu.
  #
  # FOUR DOTS, AND ONLY THE FOUR — a mark of ours, not somebody's logotype.
  #
  # The drawing it grew out of has nine dots on a three-by-three grid: five
  # light and four solid, the middle row plus the one under its centre. Two
  # tries at carrying the light five failed on measurement, and both are worth
  # keeping written down because they are the same lesson twice:
  #
  #   * as RINGS at the brand's own 2.5 they break into scattered marks at
  #     13 px, which is the whole size this is ever drawn at;
  #   * as rings at 1.6 they weigh 75% of a solid dot against the original's
  #     31% — a three-pixel outlined SQUARE, no lighter than its neighbour;
  #   * as smaller solid dots they work, and that is what shipped first.
  #
  # But the light five were never the point. What tells this mark from an
  # ordinary dot grid — which is what the owner rejected — is the SHAPE the
  # solid ones make, and that shape survives alone. Dropping the other five
  # leaves a mark that is legible at 13 px, unmistakably not a menu, and
  # unmistakably not somebody else's logotype: similar, and not the same.
  #
  # That also settles the question this file used to carry a paragraph about.
  # It is our drawing now, so nothing here rests on nominative use.
  #
  # `light` and `hollow` stay in the schema: they are what made the
  # measurement possible, and the next mark may want them.
  tailscale = {
    # ITS OWN OFFSET from the face's base, and it never changes.
    #
    # Counted from the sorted names instead, every existing mark's codepoint
    # moved the day a name sorted before it — and `U+F3000` is written by hand
    # in five places that talk about THIS mark. A new mark takes the next free
    # number here and nothing else moves.
    at = 0;
    # The grid the drawing is measured on, and where its dots sit in it.
    box = 24;
    first = 3;
    step = 9;
    radius = 3;
    # A LIGHT DOT'S OWN RADIUS, never larger than `radius`.
    light = 1.8;
    # The hole of a light dot, as a radius on the same grid. 0 makes it solid.
    hollow = 0;
    # `*` solid, `o` light, a SPACE draws nothing. FIRST ROW IS THE TOP: the
    # grid is written the way it is looked at, not the way the font counts.
    rows = [
      "   "
      "***"
      " * "
    ];
  };
}
