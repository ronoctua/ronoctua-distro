# The keyboard vocabulary, from Nix to Rust — the twin of
# `nix/bluetooth-profiles.nix`, and for the same three reasons: the table, the
# keys both languages owe, and a refusal that names what it refused.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  keyboardsFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-keyboards-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-keyboards.py} \
      --keyboards ${writeText "keyboards.json" (builtins.toJSON (import keyboardsFile))} \
      --out $out/keyboards.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Keyboard remapping vocabulary, generated from assets/keyboards.nix";
    platforms = lib.platforms.all;
  };
}
