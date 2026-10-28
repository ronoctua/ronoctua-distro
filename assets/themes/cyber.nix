# CYBER — grey, and the grey is not warm.
#
# Owl's greys carry no tint either, and this theme exists because that is only
# half of what makes a palette warm: owl's TEXT RAMP is a cream, and the ramp
# is most of the ink on a screen. Here the ramp has no colour at all — the
# body text, the quiet text and the dim text are grey, and the surfaces under
# them are a cold near-black. The colour on screen is the accent, the state
# glyphs, and one value.
#
# THE GREY RAMP BROKE `text.answer`, AND THAT IS WHY THIS THEME HAS THE FIRST
# OVERRIDE IN THE REPOSITORY.
#
# `text.answer` — the value of a row one can act on — is `text.primary`'s own
# primitive with its chroma MULTIPLIED. That works in a theme whose ramp has
# chroma to multiply. Multiply nothing and you get nothing: drafted with a
# cool ramp this theme resolved the two to ΔE 0.010, where the three before it
# sit at 0.055, and the value of every row that answers a key went back to
# wearing the body text's tone — the exact defect the token was added to
# remove. It is a gate now (`text.answer vs text.primary`), and the gate was
# written because of this theme.
#
# AND RAISING THE RAMP'S CHROMA DOES NOT FIX IT. Measured at hue 250, the
# separation peaks around a chroma of 0.45 and then FALLS: near white the blue
# corner of sRGB has so little room that the plain step and the multiplied one
# clip to nearly the same place. The hues with room near white are the warm
# ones, which is the thing this theme is not.
#
# So the ladder is left alone and one token is overridden: `answer` is a
# family of its own, a cyan, and it is the only cold colour in the text. That
# is what `overrides` is for — the theme states what the ladder could not
# reach for it, in one line, with this comment beside it.
#
# THE ACCENT IS A MAGENTA-VIOLET AND NOT A NEON CYAN, which is the first thing
# one reaches for with this name. It cannot be: `#00FFFF`, `#FF00FF`,
# `#FFFF00` and `#FF6000` are the film colours the video preview paints with,
# and no chrome token may come within 0.25 RGB of one of them. The loud neons
# this name evokes are exactly the four that are spoken for.
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
    # A COLD grey and not a neutral one: the hue is barely drawn, but where it
    # is, it leans blue. Owl's greys sit at 85 and are not drawn at all.
    hue = 250;
    tint = 0.08;
    ground-tint = 0.22;
  };
  # THE RAMP HAS NO COLOUR. The hue below is declared so the two seeds read as
  # a pair with the neutral one above, and so that a future theme built from
  # this file has somewhere to turn the knob; at chroma nought it is not drawn.
  text = {
    hue = 250;
    chroma = 0.00;
  };
  backdrop = {
    hue = 250;
    chroma = 0.50;
    step = 0.040;
  };

  chroma = 1.0;

  families = {
    # A MAGENTA-VIOLET, and the header says why it is not the neon cyan this
    # name asks for. The paint steps run above the ladder's for the reason
    # forest and midnight give: the selection band is drawn on tinted greys.
    accent = {
      hue = 318;
      chroma = 0.95;
      torsion = 16;
      paint-chroma = 1.3;
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
      hue = 275;
      chroma = 0.90;
      shift = 4;
    };
    # MORE CHROMA THAN THE OTHER THEMES GIVE IT, and the grey ramp is why. The
    # discreet register has to stay apart from the quiet text and the dim text,
    # and in every other theme the RAMP moves away from it; here the ramp has
    # nowhere to move, so the register does. At the shared 0.12 both pairs sat
    # inside the warn band. Dropped from 0.22 when the owner asked for the hint
    # inks to be greyer, but held above the themes whose ramp carries the gap.
    subtle = {
      hue = 340;
      text-chroma = 0.15;
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
    # THE TUNNEL MARK (`modeline.tunnel`): a green of its own, brighter and more
    # saturated than the drawer's faded green so the two greens read apart on
    # one line, and told from the accent and the device blue by the gates.
    tunnel = {
      hue = 165;
      text-chroma = 0.55;
      text-shift = 8;
    };
    # THE DEV BUILD MARK (`modeline.build`): a muted amber, the caution hue a
    # step quieter than the warning the band and the alert bar wear.
    build = {
      hue = 60;
      text-chroma = 0.55;
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
      hue = 138;
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
    # THE VALUE OF A ROW ONE CAN ACT ON, as a family of its own — see the
    # header, and the `overrides` block at the foot of this file. A cyan,
    # because on a grey ramp the one value that has to say "this answers a
    # key" is the only place a cold colour can go without competing with the
    # accent, and because it is the colour this theme's name is about.
    answer = {
      hue = 195;
      text-chroma = 0.60;
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
    brain = {
      hue = 200;
    };
    slate = {
      hue = 240;
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

  # THE ONE TOKEN THE LADDER CANNOT SOLVE FOR THIS THEME.
  #
  # `text.answer` is a chroma multiplier on the text ramp, and this ramp has no
  # chroma. See the header for the measurement and for why raising the ramp is
  # not the answer. Everything else comes out of the same ladder every other
  # theme uses; this is the single line that does not.
  overrides = {
    text = {
      answer = "answer.text-strong";
    };
  };
}
