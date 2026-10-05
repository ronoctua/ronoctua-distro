# THE CACHE CATALOGUE'S NUMBERS, as the Rust table this module reads: the
# floor a relief starts under, the "nearly full" line the mark is drawn under,
# their caps, and the gap between two reliefs.
#
# THE SAME TABLE AND THE SAME GENERATOR the flake builds `cache.rs` with
# (`nix/cache.nix`, which refuses the build when the numbers stop holding each
# other) — called here once more, with the same arguments, so it is the same
# derivation. The unit's arguments (`module.nix`) come from the same file, so
# the floor the brain watches and the goal the relief aims at cannot drift
# apart.
{ pkgs, ... }:
let
  policy = pkgs.callPackage ../../../../nix/cache.nix {
    cacheFile = ../../../../assets/cache.nix;
  };
in
{
  "cache.rs" = "${policy}/cache.rs";
}
