# The protocol's WIT directory, with the token enum GENERATED inside it.
#
# Section 7.6 wants a "WIT enum of valid names" coming from the same data that
# generates the Rust table. The enum is written at EVALUATION by
# `nix/token-names.nix`, from the names alone — the role names, the chrome's
# icon names — and joined here to the hand-written `.wit`, as a dependency
# package.
#
# FROM THE NAMES, AND NOT FROM THE TOKENS' TABLE (`nix/tokens.nix`), which it
# came out of until 2026-10-12. The table is a build of the glyph database and
# its measurements; this directory is what every buffer is compiled against.
# A colour's value, a font's measurement or a brain module's glyph used to
# rebuild all six components; now only a NAME that a buffer could spell does.
#
# `deps/` and not the same directory: in WIT a directory is ONE package, and
# `ronoctua:tokens` is another. A dependency package lives in `deps/<name>/`.
{
  lib,
  runCommand,
  # The generated `tokens.wit`: `nix/token-names.nix`'s `wit`.
  tokensWit,
  staticWit,
}:

runCommand "ronoctua-wit" { } ''
  mkdir -p $out/deps/tokens
  cp ${staticWit}/*.wit $out/
  cp ${tokensWit} $out/deps/tokens/tokens.wit
''
