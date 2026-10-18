# The declared fallback chain, as Rust — section 6.4.
#
# The chain lives in `nix/fontconfig.nix` because two things consume it and they
# must not drift: fontconfig's `.conf`, for foreign apps, and the renderer's own
# table, for our windows. This bakes the second one.
#
# It is the same shape as the tokens and the texts (invariants 8 and 9): the
# data is Nix, the Rust is generated, and a family named here that no face
# provides fails `nix/coverage-fonts.nix` — which reads the very same list.
{
  lib,
  stdenvNoCC,
  python3,
  chainJson,
  # The face asked for by NAME. It is read, never copied: the generator refuses
  # a probe this face already draws, because such a probe would be answered
  # here and would never reach the family it is supposed to be testing.
  primaryFontFile,
}:

stdenvNoCC.mkDerivation {
  name = "ronoctua-chain-rs";
  nativeBuildInputs = [ (python3.withPackages (ps: [ ps.fonttools ])) ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-chain.py} \
      --chain-json ${chainJson} \
      --primary-font ${primaryFontFile} \
      --out $out/chain.rs
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The renderer's fallback chain, generated from the one declaration";
    platforms = lib.platforms.all;
  };
}
