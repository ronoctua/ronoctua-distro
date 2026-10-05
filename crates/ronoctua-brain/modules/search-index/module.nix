# The search-index module's NixOS half: NOTHING, and on purpose.
#
# The unit it starts — `ronoctua-index.service`, the name its manifest
# declares — is the index's own, in `nix/modules/index.nix`, with its limits,
# its sandbox and the watches it needs; a person who switches this module off
# (`ronoctua.brain.modules.search-index.enable = false`) takes away the start
# and the mark, never the unit they can still start by hand. The module's own
# check holds the two names together, against the evaluated unit.
{ ... }:
{ }
