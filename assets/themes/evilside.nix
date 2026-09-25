# EVILSIDE — the blackest of the six, and the red is never the surface.
#
# `darkside`'s arrangement taken one step further down: the chrome is 0.130,
# so the footer is `#070707`, a window `#0B0B0B`, the modeline `#121212` and
# the desk `#040404`. True greys, byte for byte, and darker than `darkside`'s
# — which is what the owner asked for, in the same breath as the rule that
# makes it possible: NO surface carries a tone of any colour. An earlier draft
# put the red in the greys themselves and was wrong about what the name meant.
#
# THE RED IS IN THE ELEMENTS. The accent is a coral, the focused window's
# outline is a burnt red, the discreet register leans pink, the quiet text is
# warm, and the value of every row one can act on is a pale red. That is what
# is left to carry a theme once the surfaces are forbidden a hue — and it is
# more than enough, because those are the things one actually looks at.
#
# THE ACCENT AND THE ERROR ARE BOTH RED, and that is the price of the name.
# The hue gate keeps them 22° apart, which is enough for it; the colour-vision
# report still warns that under tritanopia they sit ΔE 0.014 from each other.
# It is a Warn and never a Fail, for the reason that gate carries everywhere:
# the glyph carries the meaning, not the hue. Worth knowing before choosing
# this theme on a machine where the error state matters more than the look.
#
# HOW BLACK THE DESK CAN BE. The same three rules `darkside`'s header sets
# out, and here they bite hardest: with no chroma anywhere, the desk clears
# them on lightness alone, and that is what puts the floor under the chrome.
# 0.130 is where it stops — below it the desk cannot be both 0.10 from the
# content's black and 0.02 below the footer.
#
# THE RAMP IS A WARM GREY AND NOT A RED ONE. Drafted red at chroma 0.42 it
# read well and broke `text.answer`: that token is a chroma multiplier, and in
# the red corner of sRGB the plain step and the multiplied one both clip near
# white — measured, the separation PEAKS around 0.42 and falls to 0.006 by
# 0.85. So the ramp carries just enough warmth to not be grey, and the red
# that a value needs comes from a family of its own, one rung lower on the
# text ladder where there is chroma to spend.
#
# Same seed schema as `owl.nix`.
{
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  ground = 0.0;
  chrome = 0.130;

  # NOTHING HERE IS DRAWN — both tints are nought, and that is the rule this
  # theme is built on. The hue is declared so the family reads as a pair with
  # `grey`, and so the knob is where a reader expects it.
  neutral = {
    hue = 20;
    tint = 0.00;
    ground-tint = 0.00;
  };
  # WARM, BUT ONLY JUST — see the header. A red ramp is what this theme wants
  # and what `text.answer` cannot survive.
  text = {
    hue = 25;
    chroma = 0.18;
  };
  # THE DESK IS A GREY, like every other surface here, and it clears its three
  # gates on lightness alone — see the header.
  backdrop = {
    hue = 20;
    chroma = 0.00;
    step = 0.027;
  };

  chroma = 1.0;

  families = {
    # A CORAL, 22° off the error's red — the least the hue gate calls OK. The
    # header says what that costs under tritanopia.
    accent = {
      hue = 28;
      chroma = 0.95;
      torsion = 14;
      paint-chroma = 2.2;
    };
    success = {
      hue = 150;
      chroma = 0.68;
      shift = 16;
    };
    warning = {
      hue = 70;
      chroma = 0.66;
      shift = 12;
    };
    error = {
      hue = 6;
      chroma = 0.90;
      shift = -2;
    };
    group = {
      hue = 320;
      chroma = 0.90;
      shift = 4;
    };
    subtle = {
      hue = 340;
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
      hue = 355;
      text-chroma = 0.5;
      text-shift = 0;
    };
    alternate = {
      hue = 190;
      text-chroma = 0.34;
      text-shift = -6;
    };
    # THE LABEL COLUMN, and in this theme it is the coldest thing on screen:
    # every value here is warm or red, so the column that names them has to be
    # the other end of the wheel.
    label = {
      hue = 195;
      text-chroma = 0.40;
      text-shift = 6;
    };
    label-read = {
      hue = 195;
      text-chroma = 0.00;
      text-shift = 6;
    };
    # THE RED A VALUE IS DRAWN IN, one rung below the ramp's own bright step
    # (`text-loud`, in `overrides`) because that is where red still has chroma
    # to spend. At the bright step the best this reaches is ΔE 0.035 from body
    # text, on the floor; here it is 0.095.
    answer = {
      hue = 15;
      text-chroma = 0.90;
      text-shift = -6;
    };
    grey = {
      hue = 20;
      text-chroma = 0.0;
      text-shift = -20;
    };
    grey-listed = {
      hue = 20;
      text-chroma = 0.0;
      text-shift = -8;
    };
    brain = {
      hue = 250;
    };
    slate = {
      hue = 250;
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
      answer = "answer.text-loud";
      # THE ACCENT IS AN ORANGE, AND ORANGE IS SPOKEN FOR.
      #
      # `text.key` is the accent's text rung with its chroma multiplied, and
      # the multiplier is one number for the nine — sized for frost, whose
      # cyan accent has a cyan-grey ramp to get away from. This accent sits at
      # hue 28, and at that multiplier the key lands 0.237 from `#FF6000`,
      # inside the quarter no chrome token may enter: it is one of the four
      # colours the video preview paints with and the screenshot battery
      # asserts never appear. Measured 0.282 here, and the key still reads ΔE
      # 0.086 from the command name beside it, which is over the gate.
      key = {
        of = "accent.text";
        chroma = 1.8;
      };
    };
  };
}
