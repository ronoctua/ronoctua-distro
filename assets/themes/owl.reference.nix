# THE PALETTE AS IT WAS HAND-TUNED ON 2026-09-05 — a FIXTURE, not a token file.
#
# The generator (`nix/scripts/gen-theme.py`) re-derives the `owl` theme from a
# few seeds, and the regression report measures every generated value against
# the literal that was on screen the day the seeds were written: ΔE_ok per key,
# tolerance 0.03. Over it is a build failure — unless the key is listed under
# `deviations` below with its own ceiling and the reason, which is how a
# generated value is allowed to differ from a hand value ON PURPOSE and not by
# accident. The 52 values are those literals, frozen; the order is alphabetical
# because that is how they were captured (`nix-instantiate --json`), and the
# order carries no meaning here.
#
# NOTHING BUT `nix/themes.nix` MAY IMPORT THIS FILE. It is not a source of
# colour: the tokens are recipes in `assets/tokens/colors.nix` and the values
# come out of the generator. The `colours-are-computed` check refuses any
# other import.
{
  colors = {
    border = {
      inactive = "0x5C2E33FF";
    };
    brain = {
      background = "0x161C28FF";
      countdown = "0xC3CEE2FF";
      dim = "0x93A0B8FF";
      text = "0xF4F6FAFF";
    };
    modeline = {
      background = "0x1E1E1EFF";
      state = {
        alternate = "0x9ECE6AFF";
        normal = "0x6E6656FF";
      };
    };
    notification = {
      background = "0x242424FF";
      countdown = "0x6E6656FF";
      loud = "0xE0AF68FF";
      quiet = "0xA59B87FF";
      read = "0x6E6656FF";
      suppressed = "0x6E6656FF";
    };
    rule = {
      default = "0x2A2A2AFF";
      strong = "0x3D3D3DFF";
    };
    state = {
      accent = "0x7AA2F7FF";
      critical = "0xFF5370FF";
      error = "0xF7768EFF";
      success = "0x9ECE6AFF";
      warning = "0xE0AF68FF";
    };
    surface = {
      base = "0x000000FF";
      buffer = "0x161616FF";
      chrome = "0x121212FF";
      hover = "0x222222FF";
      mesh = "0xD8D4CEFF";
      selection = "0x282448FF";
      transport = "0x121212CC";
      wave = "0xBFD3E6E6";
    };
    syntax = {
      added = "0x9ECE6AFF";
      attribute = "0xE0AF68FF";
      comment = "0x565F89FF";
      escape = "0x89DDFFFF";
      function = "0x7AA2F7FF";
      keyword = "0xBB9AF7FF";
      number = "0xFF9E64FF";
      property = "0x7DCFFFFF";
      removed = "0xF7768EFF";
      string = "0x9ECE6AFF";
      tag = "0xF7768EFF";
      type = "0x2AC3DEFF";
    };
    text = {
      dim = "0x6E6656FF";
      group = "0xBB9AF7FF";
      inverse = "0x000000FF";
      primary = "0xE9DEC8FF";
      secondary = "0xA59B87FF";
      subtle = {
        alternate = "0x749690FF";
        default = "0x706473FF";
        faint = "0x564250FF";
        strong = "0x9F92A1FF";
      };
    };
  };

  # WHERE THE GENERATED OWL IS ALLOWED TO LEAVE THE HAND VALUE, and by how much.
  deviations = {
    # The discreet register's `text-shift` (−3 Lc) was measured on `strong`
    # and, being one number per family, applies to all three text steps. The
    # hand `faint` sat at Lc 10.7 on the chrome; the ladder's faint step less
    # the shift lands at 8.1, and 0.030 of the 0.0345 is that lightness — the
    # rest is the hue (337 by hand, 321 on the register's own). A shift per
    # step would be a knob for one token; the separator is owed nothing but
    # to be told apart, which the `distance` gate still proves.
    "text.subtle.faint" = {
      max = 0.04;
      reason = "the register's text-shift was measured on strong and applies to faint too; the hand faint sat 2.6 Lc higher, at a hue 16 degrees off the register";
    };
    # THE FROZEN VALUE IS A RED, AND THE OWNER ASKED FOR IT NOT TO BE.
    #
    # This is the one kind of deviation this file exists to carry: not a
    # ladder that could not reach a hand value, but a hand value the owner has
    # since decided against — *"não gosto nem um pouco das atuais cores de
    # bordas das janelas"*. The whole distance is the HUE: the reference is
    # `error.border`, an outline in a family that means something went wrong,
    # and the window's border now comes from the accent like everything else
    # that marks focus. The `distance` gate is what proves the pair still
    # reads as two; this one only records that the old value was left behind
    # on purpose.
    "border.inactive" = {
      max = 0.12;
      reason = "the frozen value is the error family's red; the owner asked for both window borders to follow the theme and then for the unfocused one to be quieter still, so this one is the accent's element rung with its chroma drained";
    };
  };
}
