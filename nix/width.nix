# How many cells a character takes — section 4.1, the cell.
#
# The footer's whole layout is integer arithmetic over a cell of `0.600 em`,
# and that arithmetic is only true if something knows how wide a character is.
# Until the fallback chain existed, the answer was "one, always", because the
# only face was monospaced. An ideograph is two, in every monospaced CJK cut
# there has ever been, so the answer had to become a table.
#
# It is Unicode's own, not ours: `East_Asian_Width` from the UCD plus
# `Emoji_Presentation` from the emoji files — the SAME property
# `nix/scripts/strip-emoji.py` cuts the primary face with, so the two cannot
# disagree about what an emoji is.
{
  lib,
  stdenvNoCC,
  python3,
  unicode-character-database,
}:

stdenvNoCC.mkDerivation {
  name = "ronoctua-width-rs";
  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-width.py} \
      --east-asian-width ${unicode-character-database}/share/unicode/EastAsianWidth.txt \
      --emoji-data ${unicode-character-database}/share/unicode/emoji/emoji-data.txt \
      --out $out/width.rs
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The code points that occupy two cells, from Unicode's own files";
    platforms = lib.platforms.all;
  };
}
