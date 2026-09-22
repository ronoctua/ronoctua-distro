# The compositor's command table, from Nix to Rust.
#
# `assets/compositor-deeds.nix` becomes three things here, and the third is
# the one that keeps the first honest:
#
#   deeds.rs   the table `ronoctua_scroll::deeds` includes
#   keys.txt   the text keys EVERY language has to carry
#   a refusal  a verb the pinned compositor does not have takes the build down
#
# # Why the compositor's SOURCE and not its binary
#
# There is no `--help` to parse: the commands are free text and the only list
# of them is four C arrays in the source. That turns out to be better than a
# binary — the check costs no compositor, no VM and no GPU, so it runs in the
# ordinary battery instead of in a virtual machine.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  deedsFile,
  scrollSrc,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-deeds-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-deeds.py} \
      --deeds ${writeText "deeds.json" (builtins.toJSON (import deedsFile))} \
      --commands ${scrollSrc}/sway/commands.c \
      --out $out/deeds.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The compositor's command table, generated from assets/compositor-deeds.nix";
    platforms = lib.platforms.all;
  };
}
