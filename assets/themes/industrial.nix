# INDUSTRIAL — silver, and nothing but silver.
#
# The surfaces sit HIGHER on the ladder than every other theme's — the chrome
# is 0.220 where owl is 0.182 and `darkside` 0.150 — and they carry the least
# colour any theme's grounds carry without being none: the footer is `#181B1E`
# and a window `#1C1F23`, a grey with enough blue in it to read as SILVER.
#
# It was an exact grey first, and the owner read that as warm. A neutral is
# not warm by measurement; it only reads that way next to cool text and a
# steel accent, and what the word "silver" means is a grey that has decided
# which way it leans. `darkside` is the absence of colour read as BLACK; this
# one is a whisper of blue read as METAL.
#
# THE ACCENT IS A STEEL BLUE, AND IT WAS AN AMBER FIRST. Amber is the colour
# this name reaches for — the lamp on a machine — and it cannot be had: the
# film gate refuses any chrome token within 0.25 RGB of `#FF6000` or
# `#FFFF00`, and an amber solid bright enough to mark focus lands between the
# two. Measured, 0.246 against a floor of 0.25, on four tokens at once. What
# is left that still says "machine" is the metal itself.
#
# THE TEXT RAMP IS GREY TOO, so `text.answer` is overridden — the third theme
# to need it, and `cyber.nix`'s header carries the measurement for all of
# them. Here the value of a row one can act on is a pale gold: the one warm
# thing on a cold screen.
#
# THE PRICE OF A LIGHTER CHROME is contrast headroom, and it is paid in warns
# rather than fails: the dim text over the notification band and over the
# selection sit closer than they do on a darker theme, because those grounds
# climb from the chrome too. 0.220 is where that stops being free — at 0.235
# it costs three more.
#
# Same seed schema as `owl.nix`.
{
  # WHICH WAY THE LADDER CLIMBS. Dark: the ground is the darkest thing and
  # every rung walks toward white. See `MODES` in `nix/scripts/gen-theme.py`.
  mode = "dark";
  # A dark ladder asks for the Lc it wants and gets it; see `lift`.
  lift = 0;

  ground = 0.0;
  # THE HIGHEST OF THE NINE, and it is what makes this theme metal rather than
  # black — see the header, and for what it costs.
  chrome = 0.220;

  # A COOL GREY, AND THE SMALLEST AMOUNT OF IT THAT SHOWS. The surfaces were
  # exact greys — red, green and blue equal — and the owner read them as WARM
  # and asked for silver. A perfect neutral is not warm by measurement, but
  # beside cool text and a steel accent it reads that way, and silver is a
  # grey with a little blue in it. This is that little: measured, it is the
  # last pair of numbers whose warnings are the ones this theme already had.
  neutral = {
    hue = 250;
    tint = 0.05;
    ground-tint = 0.12;
  };
  text = {
    hue = 250;
    chroma = 0.00;
  };
  # THE DESK LEANS THE SAME WAY THE SURFACES DO, and since the owner made
  # the theme's own desk the system's DEFAULT it is the thing under every
  # window rather than something one goes and asks for. A grey desk under
  # silver surfaces was the one place the lean stopped.
  backdrop = {
    hue = 250;
    chroma = 0.40;
    step = 0.090;
  };

  chroma = 1.0;

  families = {
    # STEEL, AND NOT THE AMBER THIS NAME ASKS FOR — the header says why the
    # amber cannot exist here.
    accent = {
      hue = 245;
      chroma = 0.85;
      shift = 6;
      torsion = 10;
      paint-chroma = 1.4;
    };
    success = {
      hue = 140;
      chroma = 0.68;
      shift = 16;
    };
    warning = {
      hue = 55;
      chroma = 0.70;
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
      hue = 20;
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
    alternate = {
      hue = 190;
      text-chroma = 0.34;
      text-shift = -6;
    };
    label = {
      hue = 210;
      text-chroma = 0.40;
      text-shift = 6;
    };
    label-read = {
      hue = 210;
      text-chroma = 0.00;
      text-shift = 6;
    };
    answer = {
      hue = 85;
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
      hue = 265;
    };
    slate = {
      hue = 240;
      text-chroma = 0.24;
    };
    sky = {
      hue = 220;
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
