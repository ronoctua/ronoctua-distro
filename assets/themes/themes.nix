# THE THEMES THIS SYSTEM SHIPS — the one place they are named.
#
# Same shape as `assets/i18n/languages.nix`: the data is a LIST, defined once,
# and everything downstream is generated or interpolated from it — the
# generator (`nix/scripts/gen-theme.py`) solves one palette per entry, the
# tokens table carries every entry, the NixOS option `ronoctua.theme` is an
# enum over it, and the footer's `LEADER s t` lists exactly these — by id,
# which is why a theme has no text key.
#
# # Why a list here and not the directory's contents
#
# Because shipping a theme has to be a DECISION. Each name here must have a
# seed file beside it (`<id>.nix`) and a palette that passes every gate — and
# a seed file that is present and unnamed cannot sit in the tree being
# silently ignored (`themes-are-declared` closes that direction).
#
# # The order matters in one way
#
# The FIRST is the default: what a machine shows before anyone has chosen, what
# `nix run .#png` photographs, and what the compositor's static config is
# built with. `owl` is the palette the system was designed on.
#
# # Adding one
#
# Write the name here and put `<id>.nix` beside it with the seeds. Nothing
# else: the table, the enum, the chooser and the checks all read this list.
[
  "owl"
  "forest"
]
