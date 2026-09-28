# PAPYRUS — light, and the greys are WARM.
#
# The second light theme. Where `flashbang` is a white screen with cold greys,
# this is a sheet that has aged: everything from the footer down is a grey
# drifted toward brown, and the ink is the only cold thing on it.
#
# `mode` is what tells the ladder to descend and `lift` is what a light
# interface has to add to every Lc anchor; both are `nix/scripts/gen-theme.py`'s
# and the reasoning is there, not here.
#
# Same seed schema as `owl.nix`.
{
  mode = "light";
  lift = 36;

  # THE GROUND AND THE CHROME ARE A PAIR, and both are pinned by measurement.
  #
  # The chrome cannot go below 0.952: at 0.945 it no longer reaches the top of
  # the remapped ladder, `neutral.text-bright` clamps to pure black, and the
  # last rung of every family becomes a ΔE 0.31 step.
  #
  # The ground cannot come down to meet it: `surface.base vs surface.chrome`
  # wants ΔE 0.04, and at this chroma that distance is almost entirely
  # LIGHTNESS. The quieter the page, the further off the ceiling the sheet has
  # to sit. 0.995 and 0.952 leave 0.043.
  ground = 0.995;
  chrome = 0.952;

  neutral = {
    # SIXTY AND NOT EIGHTY. Paper ages toward brown, not toward yellow, and
    # yellow is the hue where the eye is most sensitive and where sRGB is
    # widest — a chroma that is a whisper in blue is still a colour there.
    # Twenty degrees off it, the same number reads as a warm grey.
    hue = 60;
    tint = 0.06;
    # A WHISPER: `surface.header` resolves to `#D4CFCA`, chroma 0.009. Enough
    # that no surface is neutral grey, and not enough for any of them to be a
    # colour. What this theme is about lives in the ink and in the marks.
    ground-tint = 0.09;
  };
  text = {
    hue = 60;
    # 0.32 IS THE FLOOR, and `text.answer` is what sets it. That token is this
    # ramp's `text-strong` with its chroma MULTIPLIED, and the pair has to stay
    # ΔE 0.035 apart: at 0.26 and below there is nothing left to multiply and
    # it fails. Above it the relation is not monotone either — 0.50 and 0.60
    # were measured worse than 0.40, because past a point the plain step and
    # the multiplied one run into the same gamut ceiling and move together.
    chroma = 0.32;
  };
  # A PAPER A SHADE UNDER THE FOOTER, desaturated at the owner's word: the
  # warm grey of the sheet itself (0.3 of what the hue can carry). It is the
  # PAGE of its own palette (the bands follow the ground), so what holds it
  # is the desks around it and the window: at 0.045 it is 0.031 off a window
  # (floor 0.03), 0.094 off the white and 0.068 off the near-white.
  backdrop = {
    hue = 60;
    chroma = 0.3;
    step = 0.045;
  };

  chroma = 1.0;

  families = {
    # THE INK, and it is COLD on purpose — `midnight`'s argument in the mirror.
    # A warm ramp under a warm accent resolves to one colour in two
    # luminances, so the one thing that is not the sheet sits on the far side
    # of the wheel. Well clear of the protan band.
    accent = {
      hue = 245;
      chroma = 0.90;
      torsion = 18;
      # Twice the paint chroma: `max_chroma` collapses toward white, and what
      # tells a selection band from the hover grey and from a rule is almost
      # entirely its colour.
      paint-chroma = 2.0;
    };
    success = {
      hue = 140;
      chroma = 0.65;
      shift = 16;
    };
    warning = {
      hue = 60;
      chroma = 0.64;
      shift = 11;
    };
    # 14 AND NOT 20: a warm theme walks its red toward orange, and at 20 this
    # one came within 0.234 of `#FF6000`, one of the four film colours no
    # chrome token may approach.
    error = {
      hue = 14;
      chroma = 0.90;
      shift = -2;
    };
    group = {
      hue = 310;
      chroma = 0.90;
      shift = 4;
    };
    subtle = {
      hue = 330;
      # 0.16: on the desk's page the inert key came 0.029 from this one at
      # 0.12 (floor 0.03).
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
      # 0.42: 0.058 from the device's mark on the desk's page at 0.36.
      text-chroma = 0.42;
      text-shift = 0;
    };
    # THE CAPTURE'S GUIDES (`capture.guide`): the two lines through the
    # pointer and every outline on the frozen screen. A colour of their own,
    # the accent's complement, so they are never the focused window's border.
    guide = {
      hue = 65;
      text-chroma = 0.9;
      text-shift = 0;
    };
    alternate = {
      hue = 195;
      text-chroma = 0.34;
      text-shift = -6;
    };
    # No `text-shift` on this pair, where the dark themes give it +6: the shift
    # lifts a label clear of the ramp, and the light remap already does that.
    label = {
      hue = 165;
      text-chroma = 0.40;
    };
    label-read = {
      hue = 165;
      text-chroma = 0.00;
    };
    grey = {
      hue = 60;
      text-chroma = 0.0;
      text-shift = -20;
    };
    grey-listed = {
      hue = 60;
      text-chroma = 0.0;
      text-shift = -8;
    };
    # THE BRAIN'S LINE keeps its colour on every page, so it has to stand
    # apart from the desk's: at the accent's own 245 it was 0.028 from the
    # desk and 0.044 from the selection band on the white page (floors 0.04,
    # 0.05). A sea-green, 190, clears both.
    brain = {
      hue = 190;
    };
    # THE VALUE OF A ROW ONE CAN ACT ON, as a family of its own — see the
    # `overrides` block at the foot of this file.
    answer = {
      hue = 30;
      text-chroma = 0.6;
    };
    slate = {
      hue = 265;
      text-chroma = 0.24;
    };
    sky = {
      hue = 215;
      chroma = 1.0;
      shift = 27;
    };
    azure = {
      hue = 250;
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

  # `text.answer` MULTIPLIES `text.primary`'s chroma, and on the desk's page
  # the ink is darker and has less to multiply: 0.025 apart (floor 0.035).
  # A family of its own instead, the answer cyber and flashbang already give.
  overrides = {
    text = {
      answer = "answer.text-strong";
    };
  };
}
