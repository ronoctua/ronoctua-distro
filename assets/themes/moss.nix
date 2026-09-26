# MOSS — forest's colour, taken toward blue and faded.
#
# It sits beside `forest` and is not a variant of it: forest's surfaces are a
# dark green that the eye reads as green, and these are a FADED blue-green —
# the footer is `#0B1210` and a window `#0E1614`, a tone with as much blue in
# it as green and a third of the saturation forest carries. The accent is a
# muted sea-green rather than a bright one, because a washed palette with a
# loud accent is two themes on one screen.
#
# THE TORSION IS NEGATIVE HERE, AND IT IS THE WHOLE REASON THIS THEME EXISTS
# AT ALL. `torsion` turns a family's PAINT steps away from its solid, and
# every other theme turns them forward. This one turns them BACK by twenty
# degrees, and the measurement behind that is worth keeping.
#
# `surface.selection` and `rule.default` sit at almost exactly the same
# lightness — the ladder puts `element` at 0.102 above the chrome and `border`
# at 0.103 — so the ONLY thing separating a selected row's band from an
# ordinary rule is the band's chroma. In the blue-green corner of sRGB, at
# that lightness, there is very little chroma to be had: measured, the band
# clamped at ΔE 0.047 against a floor of 0.05 and STAYED there when
# `paint-chroma` was raised from 3.0 to 4.0 — the gamut, not the seed, was the
# limit. Dropping `neutral.tint` to nearly nothing did not close it either.
#
# Turning the paint back toward green, where there is room, does. The accent
# one SEES is still the blue-green at 175°; what moved is the tone the band
# and the borders are drawn in, which is a thing nobody names and everybody
# would notice missing.
#
# Same seed schema as `owl.nix`.
{
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  ground = 0.0;
  chrome = 0.175;

  # A THIRD OF FOREST'S TINT IN THE GROUNDS, which is what "faded" is: the
  # same family of colour, less of it. The paint's own tint is lower again,
  # for the reason every theme whose greys and accent share a hue has to keep
  # it low — see the header for why that was not enough here.
  neutral = {
    hue = 175;
    tint = 0.03;
    ground-tint = 0.35;
  };
  text = {
    hue = 95;
    chroma = 0.32;
  };
  backdrop = {
    hue = 175;
    chroma = 0.55;
    step = 0.040;
  };

  chroma = 1.0;

  families = {
    # MUTED, AND TURNED BACK. The chroma is low for an accent because the
    # theme is a washed one; it stops at 0.72 and not lower because the row
    # under the cursor tells a door from a fact by draining THIS colour
    # (`text.here`), and a drained nothing is nothing. The negative torsion is
    # the header's whole argument.
    accent = {
      hue = 175;
      chroma = 0.72;
      shift = 6;
      torsion = -20;
      paint-chroma = 3.0;
    };
    success = {
      hue = 130;
      chroma = 0.68;
      shift = 16;
    };
    warning = {
      hue = 78;
      chroma = 0.66;
      shift = 12;
    };
    error = {
      hue = 12;
      chroma = 0.85;
      shift = -2;
    };
    group = {
      hue = 305;
      chroma = 0.90;
      shift = 4;
    };
    subtle = {
      hue = 340;
      text-chroma = 0.14;
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
      hue = 355;
      text-chroma = 0.9;
      text-shift = 0;
    };
    alternate = {
      hue = 30;
      text-chroma = 0.34;
      text-shift = -6;
    };
    label = {
      hue = 270;
      text-chroma = 0.40;
      text-shift = 6;
    };
    label-read = {
      hue = 270;
      text-chroma = 0.00;
      text-shift = 6;
    };
    grey = {
      hue = 175;
      text-chroma = 0.0;
      text-shift = -20;
    };
    grey-listed = {
      hue = 175;
      text-chroma = 0.0;
      text-shift = -8;
    };
    brain = {
      hue = 250;
    };
    slate = {
      hue = 200;
      text-chroma = 0.24;
    };
    sky = {
      hue = 220;
      chroma = 1.0;
      shift = 27;
    };
    azure = {
      hue = 250;
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

  overrides = { };
}
