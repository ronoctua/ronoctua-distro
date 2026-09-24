# FROST — cyan and pale blue, and the ice is in the surfaces.
#
# The footer is `#0E191C`, a window `#121D20` and the modeline `#18262A`: a
# slate cyan rather than a blue, with the text ramp cool over it and the
# accent a frosted teal. It sits between `midnight`, whose grounds are a deep
# navy, and `moss`, whose green is turned toward blue — this one is the blue
# turned toward green, which is what cyan is.
#
# THE ACCENT IS NOT A BRIGHT CYAN, AND THE REASON IS NOT THE HUE. The film
# gate refuses any chrome token within 0.25 RGB of `#00FFFF`, and the first
# draft landed at 0.2492 — four ten-thousandths inside the floor. Moving the
# hue AWAY from cyan made it worse, not better (0.237 at 218°, 0.230 at 225°),
# because what the gate measures is how close the pixel is, and a lighter,
# more saturated blue is closer to the corner than a middling one. What fixed
# it was CHROMA: at 0.70 the solid clears comfortably and still reads as ice.
# A measurement worth keeping, because the instinct is to turn the hue.
#
# THE PAINT IS TURNED BACK, as in `moss.nix` and for the same reason: the
# selection band and the ordinary rule sit at one lightness, so only the
# band's chroma separates them, and in this corner there is little to spend.
# The accent one sees is the frosted teal; the tone the band and the borders
# are drawn in is fifteen degrees greener.
#
# THE RAMP IS COOL AND `text.answer` IS OVERRIDDEN — a cool ramp cannot carry
# the chroma multiplier (see `cyber.nix`), so the value of a row one can act
# on is a warm amber instead: the only warm thing on the screen, which is
# exactly what a value that can be changed should be here.
#
# Same seed schema as `owl.nix`.
{
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  ground = 0.0;
  chrome = 0.205;

  neutral = {
    hue = 215;
    tint = 0.01;
    ground-tint = 0.45;
  };
  text = {
    hue = 215;
    chroma = 0.18;
  };
  backdrop = {
    hue = 215;
    chroma = 0.60;
    step = 0.085;
  };

  chroma = 1.0;

  families = {
    # THE CHROMA IS THE FILM GATE'S DOING, not a taste — see the header. The
    # torsion is `moss`'s argument, in the same corner of the gamut.
    accent = {
      hue = 205;
      chroma = 0.70;
      shift = 4;
      torsion = -15;
      paint-chroma = 3.0;
    };
    success = {
      hue = 145;
      chroma = 0.68;
      shift = 16;
    };
    warning = {
      hue = 70;
      chroma = 0.68;
      shift = 12;
    };
    error = {
      hue = 12;
      chroma = 0.85;
      shift = -2;
    };
    group = {
      hue = 300;
      chroma = 0.90;
      shift = 4;
    };
    subtle = {
      hue = 30;
      text-chroma = 0.20;
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
    # The greys here are already blue (215), so the device takes more colour
    # and a step farther round, or it is the text beside it.
    device = {
      hue = 250;
      text-chroma = 0.5;
      text-shift = 0;
    };
    alternate = {
      hue = 160;
      text-chroma = 0.34;
      text-shift = -6;
    };
    label = {
      hue = 320;
      text-chroma = 0.40;
      text-shift = 6;
    };
    label-read = {
      hue = 320;
      text-chroma = 0.00;
      text-shift = 6;
    };
    # THE ONE WARM THING ON THE SCREEN: the value of a row one can act on.
    # The ramp is cool, so the multiplier `text.answer` normally is has
    # nothing to multiply — `cyber.nix` carries the measurement.
    answer = {
      hue = 80;
      text-chroma = 0.55;
    };
    grey = {
      hue = 215;
      text-chroma = 0.0;
      text-shift = -20;
    };
    grey-listed = {
      hue = 215;
      text-chroma = 0.0;
      text-shift = -8;
    };
    brain = {
      hue = 265;
    };
    slate = {
      hue = 235;
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

  overrides = {
    text = {
      answer = "answer.text-strong";
    };
  };
}
