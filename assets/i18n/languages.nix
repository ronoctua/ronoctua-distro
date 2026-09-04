# THE LANGUAGES THIS SYSTEM SPEAKS — the one place they are named.
#
# Same shape as the tokens (`assets/tokens/*.nix`), the notifications and the
# updates: the data is defined ONCE, in Nix, and everything else is generated
# or interpolated from it. Before this file the pair was written out by hand in
# ninety-five places, forty-one of which a third language would have had to
# edit — and nothing pointed at the other forty.
#
# # Why a list here and not the directory's contents
#
# Because declaring a language has to be a DECISION. `gen-i18n.py` demands that
# every name here have a complete `.ftl`, so a language that is named and
# missing fails the build; the `languages-are-declared` check closes the other
# direction, so a `.ftl` that is present and unnamed cannot sit in the tree
# being silently ignored — which is exactly what a stray file did before.
#
# # The order matters in one way
#
# The FIRST is the base: the generator compares every other against it, key by
# key, and the installer's menu opens on it. It is `en-GB` for the same reason
# `ronoctua_i18n::DEFAULT` is — a machine that fell back is probably not a
# Brazilian one.
#
# # Adding one
#
# Write the name here, put the complete `.ftl` beside it, and generate its
# glibc locale (`nix/modules/base.nix` derives the list from this one). Nothing
# else: the checks, the drivers, the installer menu, the browser extension and
# the buffers' tags all read this list.
[
  "en-GB"
  "pt-BR"
]
