# MIDNIGHT — blue where owl is grey, and blue in the GROUNDS.
#
# The other two themes are dark surfaces with a coloured accent on them: owl's
# greys carry no tint at all, and forest's carry a green the eye registers
# without naming. This one is the first whose BACKGROUND is the colour — the
# footer, the minibuffer, the modeline and every window sit on a deep navy,
# and that is what the name is about. It is the theme `neutral.ground-tint`
# was split out for.
#
# THE TEXT RAMP IS WARM, AND THAT IS THE WHOLE OF WHY THIS READS AS BLUE. A
# cool ramp on a cool ground is one colour at two lightnesses: the words stop
# being a different KIND of thing from the surface under them. Worse, the
# ramp's own value token (`text.answer`, the ramp's chroma multiplied) would
# land in the accent's register, and a row one can act on would wear, at rest,
# something close to the colour the cursor uses. So the ramp is owl's cream on
# navy, which is the oldest legible pairing there is, and the accent keeps the
# blue to itself.
#
# Same seed schema as `owl.nix`; the ladder that turns these into a palette is
# `gen-theme.py`'s and is the same for every theme.
{
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  ground = 0.0;
  chrome = 0.182;

  neutral = {
    hue = 258;

    # THE BANDS AND THE RULES STAY NEARLY GREY, and this is the number that
    # cannot rise — the lesson forest paid for. The greys and the accent share
    # a hue here, so chroma in the PAINT walks it at `surface.selection`, and
    # the first thing to give is that band against `surface.hover` and
    # `rule.default`. A ground may be the theme; a signal drawn on a ground
    # has to keep a colour of its own.
    tint = 0.10;

    # THE GROUNDS, and they are the point of this theme. See `NEUTRAL_GROUNDS`
    # in the generator for why the two numbers are separate at all.
    ground-tint = 0.45;
  };

  # OWL'S RAMP, unchanged, and the header says why: a cool ramp on a cool
  # ground collapses into it, and its `answer` step would land in the accent's
  # register.
  text = {
    hue = 85;
    chroma = 0.40;
  };

  # The desk: the same navy, below the chrome. It is a COLOUR here and grey in
  # owl, because in owl the desk had nothing but lightness to separate it from
  # the footer and the owner asked for it plain; here the whole theme is that
  # hue already, so the desk being it too is the arrangement, not a decision
  # about one surface.
  backdrop = {
    hue = 258;
    chroma = 0.75;
    step = 0.030;
  };

  chroma = 1.0;

  families = {
    # THE ACCENT SHARES THE GROUNDS' HUE, which no other theme does on purpose
    # and this one cannot avoid: a blue theme whose accent is not blue is not
    # a blue theme. What pays for it is `paint-chroma` — the accent's PAINT
    # steps run a third above the ladder's, which is what keeps the selection
    # band off the tinted greys it is drawn on. Measured, at the ladder's own
    # chroma that band sits ΔE 0.059 from `rule.default`, inside the warn band;
    # at 1.3 it is 0.079.
    #
    # AND NOT A COLDER BLUE, which was the first attempt: at 235° the solid
    # resolves within 0.20 RGB of pure cyan, and the `film` gate refuses it —
    # the video preview's letterbox colours are not allowed a lookalike in the
    # chrome.
    accent = {
      hue = 258;
      chroma = 0.92;
      torsion = 20;
      paint-chroma = 1.3;
    };
    success = {
      hue = 145;
      chroma = 0.68;
      shift = 16;
    };
    warning = {
      hue = 70;
      chroma = 0.66;
      shift = 12;
    };
    # A hair off owl's red and a hair under its chroma: at 15° and full
    # strength the critical step lands within 0.25 RGB of the film's orange.
    error = {
      hue = 12;
      chroma = 0.85;
      shift = -2;
    };
    group = {
      hue = 310;
      chroma = 0.90;
      shift = 4;
    };
    subtle = {
      hue = 330;
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
    alternate = {
      hue = 190;
      text-chroma = 0.34;
      text-shift = -6;
    };
    # THE LABEL COLUMN — a hue no value wears, and here the values wear two:
    # the warm ramp and, under the cursor, the accent's blue. So the column
    # sits at the other end of the wheel from both, in the green owl also uses.
    label = {
      hue = 150;
      text-chroma = 0.40;
      text-shift = 6;
    };
    # THE LABEL OF A ROW ONE CANNOT ACT ON — the same hue, drained of it. See
    # owl's own seed for the whole reasoning.
    label-read = {
      hue = 150;
      text-chroma = 0.00;
      text-shift = 6;
    };
    # THE VALUE OF A ROW ONE CANNOT ACT ON — grey, and grey is the whole of
    # it. The hue below is the theme's own navy with the colour taken out, so
    # it is not drawn; what makes this work is that everything else on these
    # screens is warm.
    grey = {
      hue = 258;
      text-chroma = 0.0;
      text-shift = -20;
    };
    # THE SAME GREY, FOR A LIST ITEM, at the weight it had before `grey`
    # dropped a second step — see owl's seed. Two weights that no longer move
    # together are two families.
    grey-listed = {
      hue = 258;
      text-chroma = 0.0;
      text-shift = -8;
    };
    # The brain's line is blue-cast everywhere; here the chrome is blue too,
    # so it moves round to a violet — it has to read as a different KIND of
    # line, and on this ground the accent's own hue would not.
    brain = {
      hue = 285;
    };
    slate = {
      hue = 250;
      text-chroma = 0.24;
    };
    sky = {
      hue = 220;
      chroma = 1.0;
      shift = 27;
    };
    azure = {
      hue = 248;
      chroma = 1.0;
      shift = 19;
    };
    cyan = {
      hue = 200;
      chroma = 0.94;
      shift = 9;
    };
    orange = {
      hue = 45;
      chroma = 1.0;
      shift = 10;
    };
  };

  overrides = { };
}
