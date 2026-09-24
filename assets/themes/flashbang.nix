# FLASHBANG — the first LIGHT theme, and the ladder had to learn to descend.
#
# Nine themes came before this one and every one of them is dark: the ground is
# the pixel switched off, and every rung above it walks toward white. Here the
# ground IS white, and the rungs walk the other way. `mode` below is what says
# so, and `nix/scripts/gen-theme.py` reads it once into a sign.
#
# THE SEED IS NOT A MIRRORED OWL, and two measurements are why.
#
# APCA is not symmetric. The most contrast obtainable against a grey chrome is
# 107.3 Lc at L 0.182, which is owl's, and only 96.1 at L 0.95 — black text
# gives up ten Lc that white text does not, because `_soft_clamp_black` lifts
# it off zero and the dark-on-light exponent is far more sensitive to a ground
# stepping down from white. So the ladder's top rungs (Lc 87, Lc 100) are not
# reachable here, and `LIGHT_LC_CEILING` is what stops the solver from
# answering `#000000` with a shortfall and a last rung twice the size of the
# one before it.
#
# And chroma collapses near white. `max_chroma` is exactly nought at L 1.0, so
# `ground-tint` draws NOTHING on a white ground — the cold grey below lives
# from the chrome down, and the paper is paper.
#
# WHY THE CHROME SITS AT 0.955 AND NOT LOWER, which is the one number the whole
# seed turns on. Lc 87 needs a chrome at L ≥ 0.90; the distance gate between
# the desk and the footer wants 0.10 ΔE, which with a white ground pushes the
# chrome to ≤ 0.90. The two do not both fit. Measured across the range, a
# chrome at 0.90 costs 61 unreachable anchors and one at 0.955 costs none — so
# the chrome stays where the text can be read, and the desk is told from it by
# the smaller step that a light interface has everywhere else too.
#
# Same seed schema as `owl.nix`; the ladder is the same for every theme.
{
  # WHICH WAY THE LADDER CLIMBS. The only seed in the repository that says
  # `light`, and the reason every comment above exists.
  mode = "light";
  # EVERY ANCHOR, LIFTED — see `lift` in `nix/scripts/gen-theme.py` for the
  # measurement. This is the number that makes a light theme legible at all.
  lift = 36;

  # THE PAPER. Pure white, and it is the content's own extreme the way black is
  # owl's: `surface.base` is what the compositor paints where no window is.
  ground = 1.0;
  # The footer, one step off the paper. See the header for why it is not lower.
  chrome = 0.955;

  neutral = {
    # A COLD grey, at the owner's word. The hue is barely drawn and where it is
    # it leans blue, which is what keeps a white screen from reading yellow.
    hue = 250;
    tint = 0.06;
    # It draws nothing at the paper itself (chroma is nought at L 1.0) and
    # everything below it; see the header.
    ground-tint = 0.10;
  };
  text = {
    hue = 250;
    chroma = 0.10;

    #
    # APCA compresses the text ladder near white: Lc 23 above owl's chrome
    # costs ΔL 0.33 and below this one costs ΔL 0.135, while the surface steps
    # are a fixed ΔL in both. So the raised grounds ate the room the quiet text
    # had, and `text.dim` on `surface.header` measured Lc 0.0000 — text that is
    # not there. Measured, this takes the contrast failures from 54 to one.

  };
  backdrop = {
    hue = 250;
    chroma = 0.30;
    step = 0.070;
  };

  chroma = 1.0;

  families = {
    # A BLUE, at the owner's word, and the hue the system already reserves for
    # focus. Well clear of the protan band.
    accent = {
      hue = 264;
      chroma = 0.91;
      torsion = 22;
      # TWICE THE PAINT CHROMA, and the light ground is why. `surface.selection`
      # is this family's `element` rung, and what tells a selection band from
      # the hover grey and from a rule is almost entirely its COLOUR — but
      # `max_chroma` collapses toward white, so at the shared chroma the band
      # measured ΔE 0.0425 from the hover and 0.0277 from the rule, under a
      # floor of 0.05 on both. At 2.0 both clear it. Forest, cyber and midnight
      # raise this knob for the mirror-image reason: tinted greys.
      paint-chroma = 2.0;
    };
    success = {
      hue = 130;
      chroma = 0.65;
      shift = 16;
    };
    warning = {
      hue = 75;
      chroma = 0.64;
      shift = 11;
    };
    error = {
      hue = 10;
      chroma = 0.90;
      shift = -2;
    };
    group = {
      hue = 300;
      chroma = 0.90;
      shift = 4;
    };
    subtle = {
      hue = 321;
      text-chroma = 0.12;
      text-shift = -3;
    };
    # The sentence under an opened line of shortcuts: a COOL grey, quieter
    # than the content and never another shortcut (`text.subtle.explain`).
    explain = {
      hue = 250;
      text-chroma = 0.14;
      text-shift = -10;
    };
    alternate = {
      hue = 183;
      text-chroma = 0.34;
      text-shift = -6;
    };
    # NO `text-shift` ON THIS PAIR, where every dark theme gives it +6. The
    # shift is there to lift a label clear of the ramp, and the light remap
    # already lifts everything — asked for Lc 106 the solver answered 97.0 and
    # reported the shortfall, which is a rung that did not land where it said.
    label = {
      hue = 155;
      text-chroma = 0.40;
    };
    label-read = {
      hue = 155;
      text-chroma = 0.00;
    };
    # THE VALUE OF A ROW ONE CAN ACT ON, as a family of its own — see the
    # `overrides` block at the foot of this file.
    answer = {
      hue = 30;
      text-chroma = 0.60;
    };
    grey = {
      hue = 250;
      text-chroma = 0.0;
      text-shift = -20;
    };
    grey-listed = {
      hue = 250;
      text-chroma = 0.0;
      text-shift = -8;
    };
    brain = {
      hue = 264;
    };
    slate = {
      hue = 274;
      text-chroma = 0.24;
    };
    sky = {
      hue = 226;
      chroma = 1.0;
      shift = 27;
    };
    azure = {
      hue = 236;
      chroma = 1.0;
      shift = 19;
    };
    cyan = {
      hue = 213;
      chroma = 0.94;
      shift = 9;
    };
    orange = {
      hue = 51;
      chroma = 1.0;
      shift = 10;
    };
  };

  # THE TOKEN THE LADDER CANNOT SOLVE FOR THIS THEME.
  #
  # `text.answer` is `text.primary`'s own primitive with its chroma MULTIPLIED,
  # and this ramp has almost none to multiply — the same wall `cyber` hit, and
  # for the same reason. The owner chose the same answer: a family of its own,
  # named in one line with the reason beside it.
  overrides = {
    text = {
      answer = "answer.text-strong";
    };
  };
}
