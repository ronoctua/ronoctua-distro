# The protocol's WIT directory, with the token enum GENERATED inside it.
#
# Section 7.6 wants a "WIT enum of valid names" coming from the same data that
# generates the Rust table. The enum is born in `nix/tokens.nix`; here it is
# joined to the hand-written `.wit`, as a dependency package.
#
# `deps/` and not the same directory: in WIT a directory is ONE package, and
# `ronoctua:tokens` is another. A dependency package lives in `deps/<name>/`.
{
  lib,
  runCommand,
  tokensRs,
  staticWit,
}:

runCommand "ronoctua-wit" { } ''
  mkdir -p $out/deps/tokens
  cp ${staticWit}/*.wit $out/
  cp ${tokensRs}/tokens.wit $out/deps/tokens/
''
