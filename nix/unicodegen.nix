# The Unicode symbol browser's database.
#
# # The two authorities
#
#     UnicodeData.txt  -> the NAMES and the properties
#     the .ttf file    -> what actually EXISTS
#
# The database is the intersection. It is not thrift: the renderer embeds ONE face and has
# no fallback chain (section 6.4, deliberate), so a character outside it
# would show up as tofu — silent fallback in different clothes.
#
# # Why the UNPATCHED font
#
# Because the codepoints Nerd Fonts adds are in the PUA, already have
# their own browser (`glyphgen.nix`) and have no Unicode name. It is the mirror of what
# the glyph generator does: there the database is `cmap - base_cmap`, here it is `base_cmap`
# crossed with the UCD.
#
# # Why the version is pinned here
#
# Because nixpkgs's `unicode-character-database` moves on its own, and bumping the
# version changes which characters exist and what they are called. Without the constant,
# the whole database would change silently on a `nix flake update`.
{
  lib,
  stdenvNoCC,
  python3,
  unicode-character-database,
  unicode-emoji,
  cldr-annotations,
  noto-fonts-color-emoji,
  fonts,
  lexiconFile,
}:

let
  # THE EXPECTED VERSION, WRITTEN BY HAND.
  #
  # Taking it from `lib.getVersion unicode-character-database` would be a guard that
  # does not guard: it would follow the package, and a `nix flake update` would swap the
  # whole database with nothing to flag it — which is exactly what it exists to prevent.
  #
  # The generator checks this number against the `Blocks.txt` header of the package
  # itself (see `check_version`), and the `assert` below catches the most common case
  # before the build even begins.
  upstreamVersion = "17.0.0";

  fromPackage = lib.getVersion unicode-character-database;

  # `uharfbuzz` besides `fontTools`: the guard that requires an emoji in ONE glyph
  # has to actually SHAPE — the `cmap` does not answer whether there is a ligature.
  py = python3.withPackages (ps: [
    ps.fonttools
    ps.uharfbuzz
  ]);

  ucd = "${unicode-character-database}/share/unicode";

  mkGen =
    {
      name,
      declaredVersion ? upstreamVersion,
      expectFailure ? false,
    }:
    stdenvNoCC.mkDerivation {
      inherit name;
      nativeBuildInputs = [ py ];
      dontUnpack = true;
      dontConfigure = true;

      buildPhase =
        let
          # A shell FUNCTION, and the scripts' DIRECTORY rather than the file: the
          # generator imports `lexicon.py`, the module the two generators
          # share. Both things for the same reason as `glyphgen.nix`.
          cmd = ''
            gen() {
              python3 ${./scripts}/gen-unicodedb.py \
                --ucd ${ucd} \
                --font ${fonts.corrected}/TTF/NoctuaIosevka-Regular.ttf \
                --lexicon ${lexiconFile} \
                --emoji-test ${unicode-emoji}/share/unicode/emoji/emoji-test.txt \
                --cldr ${cldr-annotations}/share/unicode/cldr/common \
                --font-emoji ${noto-fonts-color-emoji}/share/fonts/noto/NotoColorEmoji.ttf \
                --ucd-version ${declaredVersion} \
                --out "$1"
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            echo "### fixture negativo: o gerador DEVE falhar ###"

            # NO pipe: in `cmd | tee` the exit code is `tee`'s, which
            # is always 0 — the test would report success with the generator failing
            # correctly. That is how it went in `glyphgen.nix`.
            ${cmd}
            set +e
            gen "$out" > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt

            if [ "$rc" -eq 0 ]; then
              echo "ERRO: o gerador ACEITOU um UCD de outra versao." >&2
              exit 1
            fi
            echo "OK: o gerador recusou, como devia (codigo $rc)"
            runHook postBuild
          ''
        else
          ''
            runHook preBuild
            ${cmd}
            gen $out
            runHook postBuild
          '';

      dontInstall = true;

      meta = {
        description = "Base do navegador de simbolos Unicode";
        platforms = lib.platforms.all;
      };
    };
in
# THE MESSAGE BEFORE THE BUILD. Without it, a version bump in nixpkgs would fail
# deep inside the generator, in a log nobody reads until they go looking.
assert lib.assertMsg (fromPackage == upstreamVersion) ''
  o unicode-character-database do nixpkgs esta em ${fromPackage}, e o
  nix/unicodegen.nix declara ${upstreamVersion}.

  Subir a versao muda quais caracteres existem e como eles se chamam. Atualize
  a constante de proposito e confira a cobertura de traducao depois.
'';
{
  inherit upstreamVersion;

  db = mkGen { name = "ronoctua-unicodedb"; };

  # THE NEGATIVE FIXTURE: a declared version that is not the UCD's.
  #
  # It proves the version guard exists and speaks. Without it, the guard would be a
  # promise — and the glyph one has already shown that an unverified promise is the same
  # thing as an absent guard.
  versionMismatchFails = mkGen {
    name = "ronoctua-unicodedb-fixture-negativo";
    declaredVersion = "1.0.0";
    expectFailure = true;
  };
}
