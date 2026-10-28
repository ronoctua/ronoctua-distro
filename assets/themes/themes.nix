# THE THEMES THIS SYSTEM SHIPS — the one place they are named.
#
# Same shape as `assets/i18n/languages.nix`: the data is defined once, and
# everything downstream is generated or interpolated from it — the generator
# (`nix/scripts/gen-theme.py`) solves one palette per entry, the tokens table
# carries every entry, the NixOS option `ronoctua.theme` is an enum over it,
# and the footer's `LEADER s t` lists exactly these — by id, which is why a
# theme has no text key.
#
# # Why a list here and not the directory's contents
#
# Because shipping a theme has to be a DECISION. Each name in `order` must
# have a seed file beside it (`<id>.nix`) and a palette that passes every
# gate — and a seed file that is present and unnamed cannot sit in the tree
# being silently ignored (`themes-are-declared` closes that direction).
#
# # ORDER AND DEFAULT ARE TWO FACTS, AND THEY USED TO BE ONE
#
# This file was a bare list and the FIRST entry was the default: what a fresh
# machine shows, what `nix run .#png` photographs, what the compositor's
# static config is built with. That held only while the owner wanted the
# default to also be the one at the top of the chooser, and they no longer do
# — the list is ordered for READING and the default is a separate choice.
#
# Both still live in this file, which is what invariant 16 is about: a theme
# is named HERE and nowhere else. `builtins.head` is no longer the default in
# any consumer; they take `default` by name.
#
# # Adding one
#
# Write the name in `order` and put `<id>.nix` beside it with the seeds.
# Nothing else: the table, the enum, the chooser and the checks all read this
# file.
{
  # The order the chooser lists them in, and nothing more.
  order = [
    "forest"
    "moss"
    "frost"
    "cyber"
    "midnight"
    "industrial"
    "owl"
    "darkside"
    "evilside"
    # THE FIRST LIGHT ONE, and it goes at the END on purpose. `themes-negative`
    # spoils `builtins.head` and wants the build to fail on the HUE gate, and
    # the screenshot battery photographs the theme after the default. Two
    # things read a position here, and neither wants this one first. (The
    # installer and the rescue console read none: with no `RONOCTUA_THEME`
    # they take `default`, below.)
    "flashbang"
    # The second light one: the same ladder, a warm sheet instead of a white
    # screen. Same reasons as above for being at the end.
    "papyrus"
  ];

  # What a machine shows before anyone has chosen. Must be one of `order`;
  # `themes-are-declared` refuses a name that is not.
  default = "industrial";
}
