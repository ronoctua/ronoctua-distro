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
  # The desk under the windows when the person asks for the theme's colour
  # instead of black (`LEADER s t`, second column): the accent's blue, a step
  # below the chrome, at half of what sRGB can show that dark — a night sky,
  # not a surface. The greys above it stay untinted, so it is the one place
  # the hue is the theme's own.
  backdrop = {
    hue = 264;
    chroma = 0.55;
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
