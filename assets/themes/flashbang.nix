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
  # SILVER, at the owner's word (2026-09-28): the greys' own cool hue at 0.3
  # of what it can carry. It is the PAGE of its own palette (the bands
  # follow the ground), so what holds it is the desks around it and the
  # window: at 0.048 it is 0.032 off a window (floor 0.03), 0.094 off the
  # white and 0.073 off the near-white; a darker page cannot pay the text
  # its floors.
  backdrop = {
    hue = 250;
    chroma = 0.3;
    step = 0.048;
  };

  chroma = 1.0;

  families = {
    # A BLUE, at the owner's word, and the hue the system already reserves for
    # focus — a STEEL one now, restrained (0.55): silver with a blue edge.
    # Well clear of the protan band. The torsion turns the bands to 240, not
    # away to 286, which was lilac: at 252 the selection band on the white
    # page came 0.048 from a rule (floor 0.05).
    accent = {
      hue = 258;
      chroma = 0.55;
      torsion = -18;
      # The key letter is this family's text rung at 2.4 × its chroma; at
      # the shared 0.35 it was the loudest blue on the screen.
      text-chroma = 0.22;
      # TWICE THE PAINT CHROMA, and the light ground is why. `surface.selection`
      # is this family's `element` rung, and what tells a selection band from
      # the hover grey and from a rule is almost entirely its COLOUR — but
      # `max_chroma` collapses toward white, so at the shared chroma the band
      # measured ΔE 0.0425 from the hover and 0.0277 from the rule, under a
      # floor of 0.05 on both. At 1.9 both clear it (0.050 off the rule at
      # 1.8). Forest, cyber and midnight raise this knob for the mirror-image
      # reason: tinted greys.
      paint-chroma = 1.9;
    };
    # THE STATES, restrained to half their chroma or so: the hue still says
    # which state, the silver says the theme.
    success = {
      hue = 130;
      chroma = 0.5;
      shift = 16;
    };
    warning = {
      hue = 75;
      chroma = 0.5;
      shift = 11;
    };
    error = {
      hue = 10;
      chroma = 0.6;
      shift = -2;
    };
    # AN INDIGO, not a violet: 288 is 30° off the accent and no lilac. The
    # text rung keeps 0.36 so a submenu is still told from a key (0.065; at
    # 0.25 it was 0.054, under the 0.06 floor).
    group = {
      hue = 288;
      chroma = 0.5;
      text-chroma = 0.36;
      shift = 4;
    };
    subtle = {
      hue = 321;
      # 0.16: on the desk's page the inert key came 0.028 from this one.
      text-chroma = 0.16;
      text-shift = -3;
    };
    # The sentence under an opened line of shortcuts: a COOL grey, quieter
    # than the content and never another shortcut (`text.subtle.explain`).
    explain = {
      hue = 250;
      text-chroma = 0.14;
      text-shift = -10;
    };
    # A CONNECTED BLUETOOTH DEVICE on the modeline (`modeline.device`): a
    # faded blue, so a device reads as a device beside the other marks.
    # The greys here are a cold blue already (250), so the device takes more
    # colour, or it is the text beside it: 0.55 (0.45 comes within 0.038 of
    # it, under the 0.04 floor).
    device = {
      hue = 235;
      text-chroma = 0.55;
      text-shift = 0;
    };
    # The camera's marks (`modeline.watching`): a faded pink leaning to red.
    watching = {
      # 340: 0.057 from the error's red on the desk's page at 350.
      hue = 340;
      text-chroma = 0.35;
      text-shift = 0;
    };
    # THE DRAWER'S MARK (`modeline.drawer`): a faded green, the device blue's
    # quietness in another hue — a place windows are kept, not an alarm.
    drawer = {
      hue = 150;
      text-chroma = 0.36;
      text-shift = 0;
    };
    # THE SUBTITLE LANES, drawn on a film: the same in every seed (the
    # generator refuses two that differ). 109 is lane one's yellow; 0.70
    # keeps the closest pair 0.10 apart.
    subtitle = {
      hue = 109;
      chroma = 0.70;
    };
    # THE CAPTURE'S GUIDES (`capture.guide`): the two lines through the
    # pointer and every outline on the frozen screen. A colour of their own,
    # the accent's complement, so they are never the focused window's border.
    guide = {
      # Not the complement (84): that is a hair from one of the film's
      # pure colours at this chroma.
      hue = 62;
      text-chroma = 0.9;
      text-shift = 0;
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
      text-chroma = 0.45;
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
    # THE BRAIN'S LINE keeps its colour on every page, and a silver line on
    # a silver desk is no line: at 258 it was 0.005 from the desk. A pale
    # sage (150, a fifth more chroma) is the quietest hue that clears it.
    brain = {
      hue = 150;
      paint-chroma = 1.2;
    };
    slate = {
      hue = 274;
      text-chroma = 0.24;
    };
    # THE CONTENT'S HUES (the syntax palette), at 0.6 of what they were: a
    # page of code in a silver theme is not the loudest thing on it.
    sky = {
      hue = 226;
      chroma = 0.6;
      shift = 27;
    };
    azure = {
      hue = 236;
      chroma = 0.6;
      shift = 19;
    };
    cyan = {
      hue = 213;
      chroma = 0.6;
      shift = 9;
    };
    orange = {
      hue = 51;
      chroma = 0.6;
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
