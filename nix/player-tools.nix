# WHAT THE PLAYER RUNS, in one place: the unit (`nix/modules/player.nix`) and
# the nested loop (`nix/dev.nix`) both read this table, because the loop's
# player is started from beside the footer and has no unit to take them from.
#
# Paths and not names — a unit's `PATH` is minimal, and a decoder found or not
# by accident is a format that plays on one machine and is silent on another.
# Each name is the variable `crates/ronoctua-player/src/decoders.rs` reads;
# `player-decoders-exist` checks the two against each other.
#
# yt-dlp is NOT here: it belongs to the media profile, and the unit adds it
# only where that profile is on (`nix/modules/player.nix`).
{ pkgs }:

let
  adplay = import ./adplug.nix { inherit pkgs; };

  # THE AMIGA'S DECODER IS UNFREE, and it is let in BY NAME and alone: a
  # nixpkgs of its own whose predicate admits `uade` and nothing else, so this
  # table never needs `allowUnfree` and no other unfree package can arrive
  # through it. The distro already names three unfree packages
  # (`nix/modules/nixpkgs-config.nix`); this is the fourth, for the formats
  # every Amiga game and demo wrote its music in.
  uade =
    (import pkgs.path {
      inherit (pkgs.stdenv.hostPlatform) system;
      config.allowUnfreePredicate = pkg: (pkg.pname or "") == "uade";
    }).uade;
in
{
  # The Commodore 64.
  RONOCTUA_SIDPLAYFP = "${pkgs.sidplayfp}/bin/sidplayfp";
  # MIDI, and the General MIDI soundfont it is rendered with: GeneralUser GS,
  # 30 MB, which is what makes a MIDI file sound like the instruments it names
  # rather than like sine waves.
  RONOCTUA_FLUIDSYNTH = "${pkgs.fluidsynth}/bin/fluidsynth";
  RONOCTUA_SOUNDFONT = "${pkgs.soundfont-generaluser-gs}/share/soundfonts/GeneralUser-GS.sf2";
  # The streams consoles and game engines play music from.
  RONOCTUA_VGMSTREAM = "${pkgs.vgmstream}/bin/vgmstream-cli";
  # The Atari ST.
  RONOCTUA_SC68 = "${pkgs.sc68}/bin/sc68";
  # AdLib, built here (`nix/adplug.nix`): nixpkgs has no AdPlug.
  RONOCTUA_ADPLAY = "${adplay}/bin/adplay";
  # The consoles' rips (PSF, PSF2, USF, GSF, 2SF, NCSF, SSF) and the ZX
  # Spectrum's chip music.
  RONOCTUA_ZXTUNE = "${pkgs.zxtune}/bin/zxtune123";
  # The Super Audio CD, from its disc image.
  RONOCTUA_SACD = "${pkgs.sacd}/bin/sacd";
  # The Amiga's own replay code, on an emulated Amiga.
  RONOCTUA_UADE = "${uade}/bin/uade123";
}
