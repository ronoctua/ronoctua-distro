# The sound files freedesktop has no type for, and the player opens.
#
# WHY THIS EXISTS
# ---------------
# `crates/ronoctua-player/src/decoders.rs` plays what only DeaDBeeF opened —
# SID, AdLib, Amiga, console rips — and the file manager decides what opens a
# file by its MEDIA TYPE. `shared-mime-info` 2.4 knows `audio/prs.sid`,
# `audio/midi`, `audio/x-psf` and a few more; everything else here resolves to
# `application/octet-stream`, and a type nobody claims is a file no program
# offers to open. So each family gets ONE invented type, joined to the file
# manager's table (`assets/icons/mime-glyphs.nix`, which imports `rows`), to the
# system database (`share/mime/packages/ronoctua-player.xml`, which
# `nix/modules/player.nix` writes from this) and to the player's desktop entry.
#
# WHAT IS LEFT OUT, and it is most of the decision
# ------------------------------------------------
# A glob here is a claim over EVERY file with that name, whatever program wrote
# it. The decoders take more extensions than this file claims, and the gap is
# deliberate — a name the player can play when handed one, but not one it may
# take from every other program:
#
# - claimed by freedesktop for something else: `.smf` (a formula), `.sgd` and
#   `.iso` (disc images — an SACD image is found by its bytes, never its name),
#   `.psw` (a word processor), `.gsf` (a font), `.stc` (a spreadsheet
#   template), `.asc` (armoured PGP), `.mdi` (a scanned document), `.awb`
#   (AMR-WB audio, which libmpv already plays); and `.stp`, which is `ours` for
#   STEP models;
# - claimed by nobody here and by somebody out there: `.cff` (a font format),
#   `.bam` (genome alignments), `.mib` (SNMP), `.dfm` (Delphi forms), `.dsp`
#   (source projects), `.str`, `.bnk`, `.acb`, `.xa`, `.txth` (a TEXT header
#   that describes raw audio beside it, not audio itself), and every name of
#   two letters, which is a coincidence waiting for a file.
#
# `under` makes each one a kind of something the database knows, so `is_a`
# answers about it the way it answers about everything else; `token` is the
# glyph, which for all of them is the one every sound gets.
let
  family = mime: under: globs: {
    inherit mime under globs;
    token = "music";
  };
in
rec {
  families = [
    # SID tunes in the two spellings upstream does not list; `.sid` and `.psid`
    # are `audio/prs.sid` already, and this descends from it.
    (family "audio/x-rsid" "audio/prs.sid" [
      "*.rsid"
      "*.mus"
    ])
    # MIDI wrapped in RIFF.
    (family "audio/x-rmid" "audio/midi" [ "*.rmi" ])
    # What `vgmstream` reads: the streamed music of console games.
    (family "audio/x-game-stream" "application/octet-stream" [
      "*.adx"
      "*.ahx"
      "*.hca"
      "*.brstm"
      "*.bcstm"
      "*.bfstm"
      "*.bwav"
      "*.idsp"
      "*.fsb"
      "*.xwb"
      "*.xwav"
      "*.nus3bank"
      "*.lopus"
      "*.at3"
      "*.at9"
      "*.wem"
      "*.vag"
      "*.ss2"
      "*.genh"
    ])
    # Atari ST.
    (family "audio/x-sc68" "application/octet-stream" [
      "*.sc68"
      "*.sndh"
    ])
    # Amiga, which names its music by PREFIX (`mdat.title`) as often as by
    # extension. Only the prefixes nothing else would start a name with.
    (family "audio/x-amiga-music" "application/octet-stream" [
      "*.tfmx"
      "*.bp3"
      "*.sndmon"
      "*.fc13"
      "*.fc14"
      "*.smod"
      "*.hipc"
      "*.hip7"
      "*.dwold"
      "*.mug2"
      "mdat.*"
      "tfmx.*"
      "cust.*"
      "custom.*"
      "smod.*"
      "sndmon.*"
    ])
    # Console rips of the PSF family past the two upstream knows.
    (family "audio/x-console-rip" "application/octet-stream" [
      "*.psf2"
      "*.minipsf2"
      "*.usf"
      "*.miniusf"
      "*.minigsf"
      "*.2sf"
      "*.mini2sf"
      "*.ncsf"
      "*.minincsf"
      "*.ssf"
      "*.minissf"
      "*.minidsf"
    ])
    # Sound chips of 8-bit computers: the ZX Spectrum's AY and the Atari's YM.
    (family "audio/x-chiptune" "application/octet-stream" [
      "*.psg"
      "*.vtx"
      "*.pt2"
      "*.pt3"
      "*.sqt"
    ])
    # The OPL2/OPL3 of the AdLib card.
    (family "audio/x-adlib" "application/octet-stream" [
      "*.a2m"
      "*.a2t"
      "*.d00"
      "*.hsc"
      "*.rad"
      "*.dro"
      "*.cmf"
      "*.ksm"
      "*.lds"
      "*.rix"
      "*.sa2"
      "*.sqx"
      "*.xad"
      "*.laa"
      "*.jbm"
      "*.rol"
    ])
  ];

  # One row per glob, in `mime-glyphs.nix`'s `ours` shape.
  rows = builtins.concatMap (
    f:
    map (glob: {
      inherit (f) mime under token;
      inherit glob;
    }) f.globs
  ) families;

  types = map (f: f.mime) families;
}
