# EVERY NAME THE SUPERPACK ANSWERS TO, one per line (`RONOCTUA_ICON_NAMES`),
# for the places that have the theme in the store: the nested loop, the
# photograph stage and the tests.
#
# A web app's icon can come from "the distro's collection", and what a person
# types there is a NAME — `whatsapp`, `firefox`. Answering which names exist by
# walking the theme would open 335 000 entries across hundreds of directories
# on every keystroke; the merge already wrote them all down, once, in its
# manifest, and the superpack keeps the list beside it
# (`nix/icon-superpack/names.py`): 26 807 names in 535 KB, measured on
# 2026-09-26.
#
# A MACHINE NEVER READS THIS COPY: it reads the list in the superpack's part,
# by its stable path (`ronoctua.icons.names`, `nix/modules/icons.nix`). A
# system that cited this file would build the 0,61 GiB theme to make it, and a
# failure of that build would stop an installation (`parts-build`).
{
  runCommand,
  iconSuperpack,
}:

runCommand "ronoctua-icon-names" { } ''
  cp ${iconSuperpack}/share/doc/noctua-icons/names $out
''
