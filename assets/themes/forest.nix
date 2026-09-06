# FOREST — dark, and green where owl is blue.
#
# The greys carry a green tint (chroma is the lever, not hue: at 0.10 relative
# chroma the footer reads as `#111311`, a green the eye registers without
# naming), the text ramp is a cream leaning to green, and the accent is a green
# at 150° — eight degrees below the 158–172° band where an accent collapses
# onto red under protanopia, and twenty degrees from `success`, which moves to
# a yellower green so the two stay two. `error` is pushed DOWN in lightness
# (`shift` −4): under deuteranopia a green accent and a red sit close whatever
# their hues, and lightness is the only lever left — as far as the contrast
# floor allows.
#
# Same seed schema as `owl.nix`; the ladder that turns these into a palette is
# `gen-theme.py`'s and is the same for every theme.
{
  ground = 0.0;
  chrome = 0.182;

  neutral = {
    hue = 150;
    tint = 0.10;
  };
  text = {
    hue = 105;
    chroma = 0.22;
  };
  # The desk: the accent's green, a step below the chrome, at most of what
  # sRGB can show that dark — which is little: the fraction is of a maximum a
  # third of the blue's, and the greys around it already lean green, so at
  # owl's fraction the desk and the footer would read as one (measured: the
  # `surface.backdrop vs surface.chrome` distance).
  backdrop = {
    hue = 150;
    chroma = 0.85;
  };

  # A hair under owl's: the greys already carry chroma here, and solids at full
  # strength read louder on a tinted ground than on a neutral one.
  chroma = 0.95;

  families = {
    accent = {
      hue = 150;
      chroma = 0.70;
      # Two Lc up: a green at this chroma reads a shade darker than owl's blue
      # at the same contrast, and the accent has to carry the focus.
      shift = 2;
      torsion = 15;
      # The band on green-tinted greys: at the same hue only chroma tells them
      # apart, and a green this dark has little of it to give, so the paint
      # steps run at three times the ladder's — the boundary clamps the rest.
      paint-chroma = 3.0;
    };
    success = {
      hue = 122;
      chroma = 0.70;
      shift = 16;
    };
    # Five degrees rounder than owl's amber, away from the yellow-green the
    # success family moved to; the shift is owl's plus the theme's global
    # chroma loss, so it reads at the same weight.
    warning = {
      hue = 80;
      chroma = 0.70;
      shift = 14;
    };
    error = {
      hue = 12;
      chroma = 0.85;
      shift = -4;
    };
    group = {
      hue = 310;
      chroma = 0.75;
      shift = 4;
    };
    subtle = {
      hue = 340;
      text-chroma = 0.12;
      text-shift = -3;
    };
    alternate = {
      hue = 200;
      text-chroma = 0.34;
      text-shift = -6;
    };
    # The label column — see owl's own seed for the whole reasoning. Here the
    # seat is between `alternate` (200) and `group` (310), because 155 is where
    # this theme's ACCENT lives: a green theme cannot spend green on a column.
    label = {
      hue = 255;
      text-chroma = 0.20;
      text-shift = 6;
    };

    # THE VALUE OF A ROW ONE CANNOT ACT ON — grey, and grey is the whole of it.
    #
    # `text-chroma` is nought, so the hue below is not drawn: it is the theme's own green hue, with the colour taken out.
    # The owner asked for it in one line — *"mude a cor apenas dos valores que
    # não são interativos, deixando eles cinza"* — and the reason it works is
    # that everything else on these screens is WARM: against a cream value at
    # the same weight, a neutral one reads as a step back without being any
    # darker. Luminance is left alone on purpose; what changes is only the
    # colour, which is what was asked.
    grey = {
      hue = 140;
      text-chroma = 0.0;
      # EIGHT Lc DOWN THE LADDER, at the owner's word: at the strong step
      # a grey with no colour in it is still a near-white (#E0E0E0), and
      # he read it that way — *"está muito branco"*. Sixteen Lc below the
      # cream it sits beside, and twenty-three above the ramp's own `text`:
      # far enough to be grey, near enough that nothing is harder to read.
      text-shift = -8;
    };
    brain = {
      hue = 230;
      # The same lever, for the same reason: the brain's line has to read as a
      # different KIND of line beside a chrome that already carries a hue.
      paint-chroma = 3.0;
    };
    slate = {
      hue = 200;
      text-chroma = 0.22;
    };
    # The content hues sit a tenth under owl's full chroma: on green-tinted
    # grounds a saturated cyan or orange fights the tint.
    sky = {
      hue = 210;
      chroma = 0.9;
      shift = 27;
    };
    azure = {
      hue = 240;
      chroma = 0.9;
      shift = 19;
    };
    cyan = {
      hue = 190;
      chroma = 0.9;
      shift = 9;
    };
    orange = {
      hue = 55;
      chroma = 0.95;
      shift = 10;
    };
  };

  overrides = { };
}
