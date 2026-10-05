# The offline English dictionary, from the Open English WordNet to two sorted
# TSV files — `ronoctua_words::wordnet` reads them by binary search, a few
# pages per word, and keeps nothing resident.
#
# # Why a conversion and not the XML
#
# The package is one gzipped LMF document, 12 MB packed and 100 MB of XML:
# answering "what does `house` mean" from it is parsing all of it. Sorted
# rows keyed by the word answer the same question in three reads.
#
# # English only, and the full profile only
#
# nixpkgs has no Portuguese thesaurus (`mythes` ships no data), so every other
# tongue is asked of Wiktionary with the person's consent. The files reach a
# machine through `nix/modules/words.nix`, whose switch the profile table sets
# (`nix/modules/profile.nix`): the path is read from the environment, never
# baked into a binary, so the switch is real.
#
# What the generator refuses is in `nix/scripts/gen-words.py`'s header.
{
  lib,
  stdenvNoCC,
  python3,
  open-english-wordnet,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-words";
  inherit (open-english-wordnet) version;

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    python3 ${./scripts/gen-words.py} \
      --wordnet ${open-english-wordnet}/share/wordnet \
      --out $out
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The offline English dictionary, sorted for a binary search (Open English WordNet, CC BY 4.0)";
    license = lib.licenses.cc-by-40;
    platforms = lib.platforms.all;
  };
}
