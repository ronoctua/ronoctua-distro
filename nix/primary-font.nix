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
#
# # And the other three styles
#
# A grid of cells draws bold and italic (`ronoctua_render::cells`), and the
# same rule holds for Bold, Italic and BoldItalic: each is asked for by name and
# would win the fallback with its monochrome emoji. So `styles` names which of
# the core faces to cut — the Regular alone by default, the one the renderer
# EMBEDS, whose derivation this must leave byte for byte as it was (a new store
# path there rebuilds every unit above the renderer); the three others for
# `RONOCTUA_TERMINAL_FACES`, which travel on the wrapper (`nix/rust.nix`).
{
  lib,
  stdenvNoCC,
  python3,
  fonts,
  noto-fonts-color-emoji,
  unicode-character-database,
  # Which of `noctua-iosevka-core`'s faces, by their style's name in the file.
  styles ? [ "Regular" ],
}:

let
  py = python3.withPackages (ps: [ ps.fonttools ]);
  fileOf = style: "NoctuaIosevkaNerdFontMono-${style}.ttf";
  # ONE FACE CUT, as a step of the build — and for the Regular alone, exactly
  # the step this file always had.
  cut = style: ''
    python3 ${./scripts/strip-emoji.py} \
      --primary ${fonts.noctua-iosevka-core}/share/fonts/truetype/noctua-iosevka/${fileOf style} \
      --emoji ${noto-fonts-color-emoji}/share/fonts/noto/NotoColorEmoji.ttf \
      --emoji-data ${unicode-character-database}/share/unicode/emoji/emoji-data.txt \
      --out $out/${fileOf style}
  '';
in
stdenvNoCC.mkDerivation {
  name =
    if styles == [ "Regular" ] then
      "ronoctua-primary-font"
    else
      "ronoctua-primary-font-${lib.toLower (lib.concatStringsSep "-" styles)}";
  nativeBuildInputs = [ py ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    ${lib.concatMapStrings cut styles}runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The renderer's Iosevka faces, without the codepoints that belong to the emoji face";
    platforms = lib.platforms.all;
  };
}
