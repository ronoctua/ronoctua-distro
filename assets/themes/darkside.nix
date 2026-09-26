# DARKSIDE — black, and nothing but black under everything.
#
# The chrome sits at 0.150 where the first three themes sit at 0.182, and the
# neutral ramp has NO TINT at either end: the footer is `#0B0B0B`, a window
# `#0F0F0F`, the modeline `#171717` and the desk `#040404`. Every one of them
# is a true grey — red, green and blue equal, byte for byte — because the
# owner asked for these two themes to carry no colour in the surfaces at all.
# The colour is in what is drawn ON them: the accent, the state glyphs, the
# window's outline and the value of a row one can act on.
#
# HOW BLACK THE DESK CAN BE, AND WHY IT IS NOT BLACKER. Three rules hold it
# off `#000000`, and all three were hit while tuning this file: the seed is
# refused if `chrome - step` reaches the ground, and two gates want the desk
# 0.10 from the content's black and 0.02 from the chrome. With no chroma to
# spend, the desk has only lightness to satisfy them with — which is what puts
# a floor under how dark the chrome itself can be, and it is the reason this
# one stops at 0.150 while `evilside` reaches 0.130.
#
# The rule is not in anyone's way: the compositor's background is BLACK BY
# DEFAULT (`Backdrop::Black`), and the theme's own desk is the second column
# of `LEADER s t` — the opt-in. What the floor prevents is the two columns
# resolving to the same pixel, which would leave a choice that changes
# nothing.
#
# THE TEXT RAMP IS GREY TOO, and `text.answer` is overridden for it, exactly
# as in `cyber.nix` — read that file's header for the measurement. Here the
# value of a row one can act on is a mint green, the one cold colour in the
# text.
#
# Same seed schema as `owl.nix`.
{
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  ground = 0.0;
  # LOWER THAN THE FIRST THREE THEMES (0.182), and this is the whole of
  # "predominantly black": every paint step climbs from the chrome, so
  # dropping it drops the footer, the windows, the bands and the rules
  # together. `evilside` goes lower still; this one keeps the room a slightly
  # roomier desk needs. See the header for what puts the floor there.
  chrome = 0.150;

  # NOTHING HERE IS DRAWN. Both tints are nought, so the surfaces are true
  # greys and the hue below is declared only so the family reads as a pair
  # with `grey`, and so a future theme built from this file has somewhere to
  # turn the knob.
  neutral = {
    hue = 280;
    tint = 0.00;
    ground-tint = 0.00;
  };
  # THE RAMP HAS NO COLOUR EITHER — see the header, and `cyber.nix` for the
  # measurement that made `text.answer` an override rather than a multiplier.
  text = {
    hue = 280;
    chroma = 0.00;
  };
  # THE DESK, and with no chroma it has only LIGHTNESS to clear three gates
  # with — see the header. That is also what stops the chrome going lower.
  backdrop = {
    hue = 280;
    chroma = 0.00;
    step = 0.042;
  };

  chroma = 1.0;

  families = {
    accent = {
      hue = 292;
      chroma = 0.92;
      torsion = 16;
      paint-chroma = 1.4;
    };
    success = {
      hue = 150;
      chroma = 0.68;
      shift = 16;
    };
    warning = {
      hue = 75;
      chroma = 0.66;
      shift = 12;
    };
    error = {
      hue = 12;
      chroma = 0.85;
      shift = -2;
    };
    group = {
      hue = 330;
      chroma = 0.90;
      shift = 4;
    };
    # THE ONE WARM THING IN THE FILE, and it is a placement rather than a
    # taste: at 200 the discreet register sat 10° from the brain's line and
    # the two failed the gate that keeps them apart. Almost nothing is drawn
    # in it, and what is, is a separator.
    subtle = {
      hue = 30;
      text-chroma = 0.22;
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
    device = {
      hue = 235;
      text-chroma = 0.36;
      text-shift = 0;
    };
    # The camera's marks (`modeline.watching`): a faded pink leaning to red.
    watching = {
      hue = 10;
      text-chroma = 0.36;
      text-shift = 0;
    };
    # THE DRAWER'S MARK (`modeline.drawer`): a faded green, the device blue's
    # quietness in another hue — a place windows are kept, not an alarm.
    drawer = {
      hue = 150;
      text-chroma = 0.36;
      text-shift = 0;
    };
    # THE CAPTURE'S GUIDES (`capture.guide`): the two lines through the
    # pointer and every outline on the frozen screen. A colour of their own,
    # the accent's complement, so they are never the focused window's border.
    guide = {
      # Not the complement (112): that is a hair from the film's pure yellow.
      hue = 145;
      text-chroma = 0.9;
      text-shift = 0;
    };
    alternate = {
      hue = 180;
      text-chroma = 0.34;
      text-shift = -6;
    };
    label = {
      hue = 145;
      text-chroma = 0.40;
      text-shift = 6;
    };
    label-read = {
      hue = 145;
      text-chroma = 0.00;
      text-shift = 6;
    };
    answer = {
      hue = 160;
      text-chroma = 0.45;
    };
    grey = {
      hue = 280;
      text-chroma = 0.0;
      text-shift = -20;
    };
    grey-listed = {
      hue = 280;
      text-chroma = 0.0;
      text-shift = -8;
    };
    brain = {
      hue = 232;
    };
    slate = {
      hue = 265;
      text-chroma = 0.24;
    };
    sky = {
      hue = 225;
      chroma = 1.0;
      shift = 27;
    };
    azure = {
      hue = 255;
      chroma = 1.0;
      shift = 19;
    };
    cyan = {
      hue = 195;
      chroma = 0.94;
      shift = 9;
    };
    orange = {
      hue = 45;
      chroma = 1.0;
      shift = 10;
    };
  };

  overrides = {
    text = {
      answer = "answer.text-strong";
    };
  };
}
