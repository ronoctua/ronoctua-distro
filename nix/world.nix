# The glance's places, from Nix to Rust — the twin of `nix/notifications.nix`.
#
# `assets/world.nix` becomes three things here:
#
#   world.rs      the table `ronoctua_world::PLACES` is
#   keys.txt      the text keys BOTH languages have to carry
#   a refusal     a zone `zone1970.tab` does not know takes the build down
#
# The third is the one that matters. Reading a zone NEVER fails — `Zone::parse`
# falls back to UTC and says so, which is right for a file listing and wrong for
# a place somebody named: `America/Sao_Paolo` would draw a clock, in UTC, under
# that label, and nothing on screen could tell.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  tzdata,
  worldFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-world-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-world.py} \
      --world ${writeText "world.json" (builtins.toJSON (import worldFile))} \
      --zone-tab ${tzdata}/share/zoneinfo/zone1970.tab \
      --out $out/world.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The glance's places, generated from assets/world.nix";
    platforms = lib.platforms.all;
  };
}
