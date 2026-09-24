# AdLib music — the one family the player opens that nixpkgs has no decoder
# for. `adplay` is AdPlug's command-line player, and AdPlug reads its files
# through libbinio; neither is packaged upstream, so the three are built here,
# each from a VERSIONED release tarball (`downloads-are-pinned`).
#
# `adplay -O disk -d -` writes raw 16-bit PCM to stdout, which is how the
# player reads it: the same way as every other decoder in
# `crates/ronoctua-player/src/decoders.rs`, with a WAV header in front.
{ pkgs }:

let
  libbinio = pkgs.stdenv.mkDerivation rec {
    pname = "libbinio";
    version = "1.5";
    src = pkgs.fetchurl {
      url = "https://github.com/adplug/libbinio/releases/download/libbinio-${version}/libbinio-${version}.tar.bz2";
      hash = "sha256-OYskaOeDjSJ00fYtvBEufgQ0M4EveuY+8p9csx3G3v0=";
    };
    nativeBuildInputs = [ pkgs.pkg-config ];
  };

  adplug = pkgs.stdenv.mkDerivation rec {
    pname = "adplug";
    version = "2.4";
    src = pkgs.fetchurl {
      url = "https://github.com/adplug/adplug/releases/download/adplug-${version}/adplug-${version}.tar.bz2";
      hash = "sha256-3hhGO/fAy2OaMiitR+aet/eKWhl4AtMl86XtfhxW1X8=";
    };
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ libbinio ];
  };
in
pkgs.stdenv.mkDerivation rec {
  pname = "adplay";
  version = "1.9";
  src = pkgs.fetchurl {
    url = "https://github.com/adplug/adplay-unix/releases/download/v${version}/adplay-${version}.tar.bz2";
    hash = "sha256-lJsmGAkqOq5cJ4qY36MjETDvNaeRs6/KoOvkVEPOgsg=";
  };
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [
    adplug
    libbinio
  ];
  # Only the disk writer: no ALSA, SDL or libao output, because the player
  # never lets a decoder reach a sound card — mpv is the one audio output.
  configureFlags = [
    "--disable-output-alsa"
    "--disable-output-sdl"
    "--disable-output-ao"
    "--disable-output-esound"
    "--disable-output-qsa"
  ];
  meta.mainProgram = "adplay";
}
