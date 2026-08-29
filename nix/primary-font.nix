# The PRIMARY face the renderer embeds — the Iosevka, minus what belongs to the
# emoji face.
#
# # Why it is not the `noctua-iosevka-core` file directly
#
# Because the primary face is the one asked for by NAME, and that is why it wins
# the fallback chain on every codepoint it has. The Iosevka has 75 of the
# default-presentation emoji — `UP!`, the clock faces, the moon phases — drawn
# in monochrome. They would come out as text next to the coloured ones, with no
# tofu to give it away: both "exist" and both "show up".
#
# It is the same class of defect that forced the Lucide remapping, on the other
# side of the boundary: **whoever has the codepoint first wins**. There the new
# face invaded the primary one; here the primary one invades the new.
#
# The rule is Unicode's: `Emoji_Presentation=Yes` means "the default form of
# this character is the emoji". The TEXT-presentation ones (`#`, digits, `©`, the
# arrows) stay, because drawing them coloured by default would break ordinary text.
#
# # Why only the renderer's copy
#
# Because the two OTHER readings of the font are authorities on something else:
# `glyphgen` asks "which glyphs did Nerd Fonts add" and `unicodegen` asks
# "which characters exist". Neither changes because of this — the database
# still covers all 75, just through the other face.
{
  lib,
  stdenvNoCC,
  python3,
  fonts,
  noto-fonts-color-emoji,
  unicode-character-database,
}:

let
  py = python3.withPackages (ps: [ ps.fonttools ]);
  file = "NoctuaIosevkaNerdFontMono-Regular.ttf";
in
stdenvNoCC.mkDerivation {
  name = "ronoctua-primary-font";
  nativeBuildInputs = [ py ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    python3 ${./scripts}/strip-emoji.py \
      --primary ${fonts.noctua-iosevka-core}/share/fonts/truetype/noctua-iosevka/${file} \
      --emoji ${noto-fonts-color-emoji}/share/fonts/noto/NotoColorEmoji.ttf \
      --emoji-data ${unicode-character-database}/share/unicode/emoji/emoji-data.txt \
      --out $out/${file}
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The renderer's Iosevka, without the codepoints that belong to the emoji face";
    platforms = lib.platforms.all;
  };
}
