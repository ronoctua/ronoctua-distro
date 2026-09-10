# The sound-port table, from Nix to Rust — the twin of `nix/bluetooth-profiles.nix`.
#
# `assets/audio-names.nix` becomes two things here:
#
#   names.rs   the table `ronoctua_audio::PORTS` is
#   keys.txt   the text keys BOTH languages have to carry
#
# and a third that has no file: an entry whose name is not normalised, or whose
# key is outside the family, takes the build down naming it. `keys.txt` is what
# makes `audio-names-have-text` possible.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  namesFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-audio-names-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-audio-names.py} \
      --names ${writeText "audio-names.json" (builtins.toJSON (import namesFile))} \
      --out $out/names.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Sound port names, generated from assets/audio-names.nix";
    platforms = lib.platforms.all;
  };
}
