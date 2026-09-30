# FOREST — dark, and green where owl is blue.
#
# The SURFACES are the green — a dark one, and the owner asked for it twice:
# first for more green in the grounds, then for the green to be darker. Both
# are the same two knobs, `chrome` and `neutral.ground-tint`, pulled in
# opposite directions; the footer is `#061008` and a window `#08150A`. The
# bands and the rules drawn ON those grounds keep a colour of their own, which
# is what `neutral.tint` is for and why it is the smaller number. The text
# ramp is a cream leaning to green, and the accent is a green at 150° — eight degrees below the 158–172° band where an accent collapses
# onto red under protanopia, and twenty degrees from `success`, which moves to
# a yellower green so the two stay two. `error` is pushed DOWN in lightness
# (`shift` −4): under deuteranopia a green accent and a red sit close whatever
# their hues, and lightness is the only lever left — as far as the contrast
# floor allows.
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
  # LOWER THAN OWL'S 0.182, at the owner's word — every surface in this theme
  # climbs from here, so this is the one number that makes the greens darker
  # rather than merely different.
  chrome = 0.160;

  neutral = {
    hue = 150;
    # THE BANDS AND THE RULES, and this is the number that cannot rise. Here
    # the greys and the ACCENT are both at 150°, so chroma in the paint walks
    # it straight at `surface.selection`, and that band against `rule.default`
    # is the first thing to give. It came DOWN when the chrome did: a lower
    # chrome leaves the band less room, and paying for it here — where almost
    # nothing is drawn — is cheaper than paying for it in the grounds. A
    # signal drawn on a ground has to keep a colour of its own.
    tint = 0.05;
    # THE GROUNDS, and they are the theme. The owner asked for this one to be
    # greener where it is a BACKGROUND — *"mais verde nos backgrounds, como os
    # das janelas dos app, minibuffer e modeline"* — and against those same
    # grounds the selection band has room to spare, where against the paint it
    # has almost none. So the tint falls off as the ladder climbs out of the
    # background and into the signals; see `NEUTRAL_GROUNDS`.
    #
    # IT WENT UP WHEN THE CHROME WENT DOWN, and the two are one change: the
    # owner asked for these surfaces to be a DARKER green, and darker on its
    # own is just blacker. The chrome fell from 0.182 to 0.160 and the tint of
    # the grounds rose, so the footer is both lower and greener than it was —
    # and the theme came out with one warning FEWER than it had, because the
    # paint's own tint fell at the same time and took the selection band's
    # tightest pair with it.
    ground-tint = 0.60;
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
    # AND IT HAD TO DROP WHEN THE CHROME WENT GREEN. The desk is a green a
    # step below the footer, and it had only that step plus a little chroma to
    # separate them; with `ground-tint` raised, the footer moved onto it and
    # the pair fell under the gate. One more step of L is the whole of the
    # answer, and it is the lever this seed exists for.
    step = 0.030;
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
      # 5 AND NOT 15: on the darkest pages the band came 0.045 from a rule
      # (floor 0.05); five degrees nearer the solid it keeps 0.05 on all four.
      torsion = 5;
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
      hue = 330;
      text-chroma = 0.9;
      text-shift = 0;
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
      hue = 255;
      text-chroma = 0.00;
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
      hue = 140;
      text-chroma = 0.0;
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
