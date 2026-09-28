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
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  # The content ground: OLED black, the pixel switched off. And the footer's
  # grey, from which every other grey climbs.
  ground = 0.0;
  chrome = 0.182;

  # The greys carry no tint at all; the text ramp is warm — one hue, one
  # relative chroma, three luminances.
  neutral = {
    hue = 85;
    tint = 0.0;
    # OWL'S GREYS ARE GREY, grounds included — the same number as `tint`, and
    # the two are separate so that a theme which wants a coloured background
    # can have one without tinting the bands and rules drawn on it. See
    # `NEUTRAL_GROUNDS` in the generator.
    ground-tint = 0.0;
  };
  text = {
    hue = 85;
    # MORE OF THE HUE, at the owner's word: the values of a row one can act on
    # and of a row one can only read "are still very similar to the human eye",
    # and they asked for the warm one to carry more colour.
    #
    # The ceiling is the FROZEN REFERENCE and not a contrast gate: above 0.40
    # `text.secondary` drifts 0.04 from `owl.reference.nix`, which allows 0.03.
    # A deviation with a reason is the sanctioned route past it and was not
    # needed — what this knob buys is small either way (see below).
    #
    # AND IT IS THE SMALLER HALF OF THE FIX. Measured, the pair is told apart
    # almost entirely by LUMINANCE: of ΔE 0.0838 between the two values, 0.0794
    # is lightness and 0.0268 is colour. Warming the ramp from 0.26 to 0.40
    # moves the pair by 8%; dropping the grey a second step moves it by 50%.
    chroma = 0.40;
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
      # Not the complement (84): that is a hair from one of the film's
      # pure colours at this chroma.
      hue = 62;
      text-chroma = 0.9;
      text-shift = 0;
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
    # question the content answers.
    #
    # THE DISTINCTION USED TO BE LUMINANCE — `text` for a row that answers
    # `RET`, `text-muted` for one that can only be read. It is CHROMA now, and
    # the two halves of that live in two seeds: this one carries the hue, and
    # `label-read` below is the same step drained of it. The owner asked for
    # the change twice, and the second time the measurement said why: at the
    # old pair the two labels were ΔE 0.0236 apart, which is under the 0.03
    # this palette's own regression gate calls a noticeable difference. A
    # column nobody can tell from the column beside it is not a distinction.
    #
    # 155° is the seat between `success` (130) and `alternate` (183), 25° from
    # each and the widest gap this palette had left.
    #
    # IT IS STILL THE QUIET HALF OF ITS LINE, and that is not the same as the
    # dim half — which is what the first attempt confused. The owner asked for
    # sobriety once (*"por ser labels, essas novas cores deveriam ser mais
    # sóbrias"*), and what was loud then was the LOUD TEXT STEP: a whole column
    # at that weight competes with the values it is naming. The step is what
    # came down, and it stays down — Lc 54 against the ramp's 48.
    #
    # The chroma went the other way, from 0.20 to 0.40, because it is the only
    # axis left: both labels sit at one luminance now, so colour is the whole
    # of the difference between them. 0.40 is a muted green (#81B291) and
    # nowhere near a state colour; the ladder allows more if a screen ever asks
    # for it.
    label = {
      hue = 155;
      text-chroma = 0.40;
      text-shift = 6;
    };

    # THE LABEL OF A ROW ONE CANNOT ACT ON — the same hue, drained of it.
    #
    # It was `label.text-muted`, twenty-five Lc BELOW the label beside it, and
    # the owner read that off the screen: a whole column of questions rendered
    # dark, as if the rows one can only read were half switched off. A label is
    # the quiet half of its line, not the dim half — what it has to say is
    # WHICH question this is, and that has to be legible whether or not the
    # answer can be changed.
    #
    # So the distinction moves from LUMINANCE to CHROMA. Same hue, same step,
    # same shift — a third of the colour. Against the acting label it reads as
    # the same word with the life taken out of it, which is exactly what it is;
    # against the neutral ramp it is still a hue and not another grey.
    #
    # It is a family and not a step because the ladder gives one chroma per
    # family: two chromas at one luminance is two families, and this is the
    # cheaper half of the pair.
    #
    # AND THE CHROMA IS NOUGHT, so the hue below is not drawn — it is kept at
    # the acting label's own so that the two seeds read as a pair, and so that
    # raising it is one edit if a theme ever wants a tinted fact label. The
    # owner asked twice for more distinction, and a hue against no hue at one
    # luminance is the most this axis has to give.
    label-read = {
      hue = 155;
      text-chroma = 0.00;
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
      # SIXTEEN Lc DOWN THE LADDER, and it took two askings to get here.
      #
      # The first was eight, at the owner's word: at the strong step a grey
      # with no colour in it is still a near-white (#E0E0E0), and he read it
      # that way — *"está muito branco"*.
      #
      # The second was eight more, and the reason is a MEASUREMENT. The owner
      # said the two values "are still very similar to the human eye" and asked
      # for the warm one to carry more colour; measured, that is the smaller
      # half of the answer. Of the ΔE 0.0838 between the pair, 0.0794 is
      # lightness and 0.0268 is colour — so warming the ramp moves them 8% and
      # dropping this grey one more step moves them 50%. Both were done; this
      # is the half that does the work.
      #
      # AND IT STOPS HERE. At −16 the value sits at Lc 63; a third step would
      # put it at 59, and APCA's floor for body text is 60. This is the thing
      # one came to READ, and dimming it past legibility would be the same
      # complaint the owner made about the labels, moved one column right.
      text-shift = -20;
    };

    # THE SAME GREY, FOR A LIST ITEM, AND IT DID NOT FOLLOW THE DROP.
    #
    # `grey` went a second step down so the ANSWER of a row could be told from
    # the answer of a row one can act on. A list item's value shares the family
    # and would have gone with it — from Lc 40 to 32, and APCA's floor for text
    # of any size is 30. The thing this palette was being tuned for is a pair
    # in a sheet; dragging a different screen's list under the floor to get
    # there is not a trade, it is an accident.
    #
    # So the weight that moved keeps its own seed and this one keeps the eight
    # it already had. The ladder gives ONE shift per family, and two weights
    # that no longer move together are two families.
    grey-listed = {
      hue = 85;
      text-chroma = 0.0;
      text-shift = -8;
    };
    # The brain's line: a blue-cast ground with neutral near-white text.
    brain = {
      # 250: at the accent's own 264 the selection band on the black page
      # came within 0.047 of this line (floor 0.05).
      hue = 250;
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
