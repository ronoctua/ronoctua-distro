# The colour roles the generator SHAPES — named here, and only named.
#
# `assets/tokens/colors.nix` holds the roles a RECIPE solves: one primitive of
# one family each. These are the others: whole groups the generator lays out
# by rule (`shaped_tokens` in `nix/scripts/gen-theme.py`) — a context's six
# steps, the terminal's sixteen, the chart's eight hues at equal distance, a
# cursor per seat, the eleven rungs of the ordinal ramp. What each one IS stays
# the generator's: the step, the hue, the lightness. This file holds the NAME,
# and nothing else, because a name is what the Rust is compiled against
# (`ronoctua_tokens::color`, the WIT enum) and a value never is.
#
# # Why the names are written here and not left to the generator
#
# The role list is read at EVALUATION (`nix/token-names.nix`): the attribute
# names of the two files, flattened. That is what lets a seed, a recipe's
# value or the generator's arithmetic change without one Rust derivation
# changing — the palettes are data the binaries read at run time
# (`palettes.bin`, `RONOCTUA_THEMES`). A name the generator alone knew would
# exist only after a build had run it, and the Rust would have to wait on that
# build, and be rebuilt whenever it ran again.
#
# THE TWO LISTS ARE HELD EQUAL BY THE GENERATOR: it is handed the evaluated
# role list (`--roles`) and refuses a role it shapes that is not named here,
# and a name here that it does not shape — with the name on screen. A new
# shaped role is a line in `gen-theme.py` AND a name here; the build says which
# half is missing.
#
# Every leaf is `true`: there is nothing to say about a shaped role but that it
# exists. Nested, never camelCase, for the reason `colors.nix` gives.
let
  # One attribute per name, each `true`.
  named =
    names:
    builtins.listToAttrs (
      map (name: {
        inherit name;
        value = true;
      }) names
    );
  # THE EIGHT, as the generator spells them (`WORDS[1..8]`).
  eight = named [
    "one"
    "two"
    "three"
    "four"
    "five"
    "six"
    "seven"
    "eight"
  ];
  # A CONTEXT'S SIX (`CONTEXT_SHAPE`): its solid, the solid hovered, its
  # ground, its border, what sits on the solid, and its strongest text.
  context = named [
    "solid"
    "hover"
    "ground"
    "border"
    "on"
    "text"
  ];
in
{
  # THE CONTEXTS (`CONTEXT_FAMILIES`), each the same six, and the neutral one
  # its own six (`NEUTRAL_SHAPE`).
  context = {
    accent = context;
    success = context;
    warning = context;
    error = context;
    group = context;
    subtle = context;
    alternate = context;
    brain = context;
    neutral = named [
      "surface"
      "surface-raised"
      "fg"
      "fg-muted"
      "border"
      "border-strong"
    ];
  };

  # THE TERMINAL (`TERMINAL`): its six own roles and the ANSI sixteen.
  terminal =
    named [
      "background"
      "foreground"
      "cursor"
      "cursor-text"
      "selection-background"
      "selection-foreground"
    ]
    // {
      ansi = named [
        "black"
        "red"
        "green"
        "yellow"
        "blue"
        "magenta"
        "cyan"
        "white"
        "bright-black"
        "bright-red"
        "bright-green"
        "bright-yellow"
        "bright-blue"
        "bright-magenta"
        "bright-cyan"
        "bright-white"
      ];
    };

  # THE CATEGORICAL SET (`CHART_COUNT`).
  chart = eight;

  # ONE CURSOR PER SEAT (`CURSOR_COUNT`): its fill, its hover band and its
  # selection band, and the one dark rim they share.
  cursor = {
    fill = eight;
    hover = eight;
    selection = eight;
    outline = true;
  };

  # THE ORDINAL RAMP, eleven rungs from the accent's ground.
  level = named [
    "zero"
    "one"
    "two"
    "three"
    "four"
    "five"
    "six"
    "seven"
    "eight"
    "nine"
    "ten"
  ];
}
