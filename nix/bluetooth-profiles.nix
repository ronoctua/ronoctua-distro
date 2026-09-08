# The service catalogue, from Nix to Rust — the twin of `nix/notifications.nix`.
#
# `assets/bluetooth-profiles.nix` becomes two things here:
#
#   profiles.rs   the table `ronoctua_bluetooth::PROFILES` is
#   keys.txt      the text keys BOTH languages have to carry
#
# and a third that has no file: an entry that is neither named nor hidden takes
# the build down naming the id. `keys.txt` is what makes
# `bluetooth-profiles-have-text` possible — the same arrangement that stopped
# the notifications from depending on whoever remembered.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  profilesFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-bluetooth-profiles-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-bluetooth-profiles.py} \
      --profiles ${writeText "bluetooth-profiles.json" (builtins.toJSON (import profilesFile))} \
      --out $out/profiles.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Bluetooth service catalogue, generated from assets/bluetooth-profiles.nix";
    platforms = lib.platforms.all;
  };
}
