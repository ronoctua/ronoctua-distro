# OWL — the palette the system was designed on, re-derived from seeds.
#
# Every number here was MEASURED from the hand-tuned palette of 2026-09-05
# (frozen in `owl.reference.nix`): the hue and relative chroma of each family,
# and the contrast each of its steps was already sitting at. The generator
# solves the same values back from these seeds, and the regression report
# proves it, key by key, to within ΔE_ok 0.03.
#
# What a seed IS: a hue in degrees (OKLCH), a relative chroma in [0, 1] — a
# fraction of the most chroma sRGB can show at that lightness and hue, never an
# absolute — and, per family, a `shift` in Lc added to the solid steps' contrast
# targets (where that hue sits: yellows read lighter, reds darker) and a
# `torsion` in degrees applied to the family's PAINT steps (the ground and the
# borders), because a hue as a ground is not the same hue as a solid: the
# selection band is 22° round from the accent it belongs to.
#
# What a seed is NOT: a lightness or a colour. Those come out of the ladder in
# `gen-theme.py`, which is the same for every theme.
{
  # The content ground: OLED black, the pixel switched off. And the footer's
  # grey, from which every other grey climbs.
  ground = 0.0;
  chrome = 0.182;

  # The greys carry no tint at all; the text ramp is warm — one hue, one
  # relative chroma, three luminances.
  neutral = {
    hue = 85;
    tint = 0.0;
  };
  text = {
    hue = 85;
    chroma = 0.26;
  };
  # The desk under the windows when the person asks for the theme's own
  # instead of black (`LEADER s t`, second column).
  #
  # IT IS GREY, at the owner's word: it was the accent's blue at half of what
  # sRGB can show that dark — a night sky rather than a surface — and they
  # asked for a desk with no colour in it at all. So the chroma is nought and
  # the hue is the neutral ramp's own, which is to say it is not drawn.
  #
  # AND THAT IS WHY THE STEP IS HERE. A coloured desk is told from the footer's
  # chrome by its hue; a grey one has only lightness left, and the mirrored
  # single step is not enough of it — measured, a grey desk one step below the
  # chrome sits at ΔE 0.026 from it, which the palette's own gate calls legal
  # and marginal. Five steps down it is 0.05 from the chrome and still 0.13
  # from the content's black, clear of both floors with room to spare.
  backdrop = {
    hue = 85;
    chroma = 0.0;
    step = 0.05;
  };

  # A global multiplier on every relative chroma. One, because these seeds are
  # the palette's own measurements.
  chroma = 1.0;

  families = {
    # Blue, reserved for focus. The torsion is what puts the selection band at
    # h 287 while the border stays at 264.
    accent = {
      hue = 264;
      chroma = 0.91;
      torsion = 22;
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
    # Purple: the submenu label and the keyword.
    group = {
      hue = 300;
      chroma = 0.90;
      shift = 4;
    };
    # The discreet register — text ABOUT the screen, not the screen's content.
    subtle = {
      hue = 321;
      text-chroma = 0.12;
      text-shift = -3;
    };
    # The key whose meaning changes with the section that has the keyboard.
    alternate = {
      hue = 183;
      text-chroma = 0.34;
      text-shift = -6;
    };
    # THE LABEL COLUMN of the three views that are a list of a label and a
    # value — network, power and Bluetooth.
    #
    # A hue of its own because the label is another KIND of thing than the
    # value beside it: the value is the screen's content and the label is the
    # question the content answers. Inside the family, `text` is a row that
    # ANSWERS `RET` and `text-muted` a row one can only read — luminance for
    # importance, which is this palette's other half of the rule.
    #
    # 155° is the seat between `success` (130) and `alternate` (183), 25° from
    # each and the widest gap this palette had left.
    #
    # AND IT IS SOBER, which the owner asked for after seeing the first one on
    # screen: *"por ser labels, essas novas cores deveriam ser mais sóbrias"*.
    # It started at `alternate`'s chroma with the loud text step, and a whole
    # COLUMN at that weight competes with the values it is naming — a label is
    # the quiet half of its line. Measured against the neutral ramp the pair
    # now sits a little above it (Lc 54 and 29 against the ramp's 48 and 23):
    # quiet enough not to shout, bright enough that the hue is still a hue and
    # not another grey.
    label = {
      hue = 155;
      text-chroma = 0.20;
      text-shift = 6;
    };

    # THE VALUE OF A ROW ONE CANNOT ACT ON — grey, and grey is the whole of it.
    #
    # `text-chroma` is nought, so the hue below is not drawn: it is the neutral ramp's own hue, with the colour taken out.
    # The owner asked for it in one line — *"mude a cor apenas dos valores que
    # não são interativos, deixando eles cinza"* — and the reason it works is
    # that everything else on these screens is WARM: against a cream value at
    # the same weight, a neutral one reads as a step back without being any
    # darker. Luminance is left alone on purpose; what changes is only the
    # colour, which is what was asked.
    grey = {
      hue = 85;
      text-chroma = 0.0;
      # EIGHT Lc DOWN THE LADDER, at the owner's word: at the strong step
      # a grey with no colour in it is still a near-white (#E0E0E0), and
      # he read it that way — *"está muito branco"*. Sixteen Lc below the
      # cream it sits beside, and twenty-three above the ramp's own `text`:
      # far enough to be grey, near enough that nothing is harder to read.
      text-shift = -8;
    };
    # The brain's line: a blue-cast ground with neutral near-white text.
    brain = {
      hue = 264;
    };
    # Content hues — the syntax palette. Content may spend the hues the chrome
    # reserves; see the `syntax` block of `colors.nix`.
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

  # Recipe overrides, same forms as `colors.nix`. None: owl IS the recipes.
  overrides = { };
}
