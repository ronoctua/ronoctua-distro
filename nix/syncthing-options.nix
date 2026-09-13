# The Syncthing option table, from Nix to Rust — the twin of `nix/audio-names.nix`.
#
# `assets/syncthing-options.nix` becomes two things here:
#
#   options.rs  the table `ronoctua_syncthing::options::SETTINGS` is
#   keys.txt    the text keys BOTH languages have to carry
#
# and a third that has no file: an entry with a duplicate id, a number with no
# range, a ring that cannot turn or two rows writing the same JSON field takes
# the build down naming it. `keys.txt` is what makes `syncthing-options-have-text`
# possible.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  optionsFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-syncthing-options-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-syncthing-options.py} \
      --options ${writeText "syncthing-options.json" (builtins.toJSON (import optionsFile))} \
      --out $out/options.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Syncthing's options, generated from assets/syncthing-options.nix";
    platforms = lib.platforms.all;
  };
}
