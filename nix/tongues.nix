# The languages of content, from Nix to Rust — the twin of `nix/broadcast.nix`.
#
# `assets/tongues.nix` becomes two things here, and a refusal:
#
#   tongues.rs  the table `ronoctua_player::tongues` compiles
#   keys.txt    the text keys BOTH languages have to carry (`tongue-<code>`)
#   a refusal   a row of the wrong shape, a code twice, a language neither
#               subtitle service searches in — `nix/scripts/gen-tongues.py`
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  tonguesFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-tongues-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-tongues.py} \
      --tongues ${writeText "tongues.json" (builtins.toJSON (import tonguesFile))} \
      --out $out/tongues.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The languages of content, generated from assets/tongues.nix";
    platforms = lib.platforms.all;
  };
}
