# THE EDITING LOOP — ten chosen checks, and not a category.
#
# WHY THIS LIST IS HAND-WRITTEN, AND WHY THAT IS RIGHT
# ---------------------------------------------------
# Because it does not try to be complete. `nix run .#most` is the derived,
# total one; this is the one run in the middle of an edit, and what defines it
# is "what catches defects in a minute", which is judgement and not a property.
#
# What was wrong before was the DESCRIPTION: it said "the checks that do not
# bring up a VM", promising a category. There are over a hundred checks and
# most of them bring up no VM. Nine were born in one day and none of them
# landed here — and nothing said anything, because there was nothing to say: a
# list that does not promise completeness cannot become incomplete.
#
# THE GUARD
# ---------
# `quick-names-exist` demands that every name here exist in `checks`. Without
# it a renamed check leaves the short loop IN SILENCE — the list still has ten
# lines and one of them stops meaning anything.
[
  # Formatting in both languages: Nix, the workspace and the five buffers.
  "rust-fmt"
  "fmt"
  "buffers-fmt"
  # What catches the most defects per second spent.
  "rust-clippy"
  "rust-tests"
  # The tables the compiler does not check: keys and texts.
  "keymap-no-collision"
  "i18n-complete"
  "buffer-commands-have-labels"
  "buffer-keys-have-text"
  # The dependency layer, which if edited costs 4m46 on the next round.
  "deps-layer-is-lean"
]
