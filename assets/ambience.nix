# The player's ambient sounds: REAL RECORDINGS, each one approved by the
# owner's ear, cut into a loop that goes round with no seam.
#
# ONE DECLARATION, read by `nix/ambience.nix`, which makes each loop from
# its original and writes the index the player reads (`RONOCTUA_AMBIENCE`).
# That build refuses:
#
#   - a licence other than CC0 or the public domain;
#   - a source that is not a recording — a generator's name anywhere here
#     (`anoisesrc`, `sine`, `aevalsrc`, `anullsrc`, `lavfi`): nothing is
#     synthesised, and the originals are opened by file name and nothing else;
#   - an original whose sha256 is not the one approved;
#   - a seam that is heard: a dip below the loop's own quietest, a silence,
#     a step the recording does not make on its own, or an event (a thunder)
#     inside the crossfade;
#   - gain that would need compression to reach `loudnessLufs` (the peaks the
#     recording has are the recording);
#   - a `route` that does not play the loop sample for sample.
#
# A NEW SOUND comes in only with the owner's approval by ear. Its original
# goes into the Release `ambience-v1` (`nix run .#ambience-release`), and its
# id becomes the text key `player-ambience-<id>`, which both languages carry;
# its group, `player-ambience-group-<group>`.
#
# THE WINDOWS were chosen by measurement, never by listening: EBU R128
# momentary loudness (`ebur128`, 100 ms steps) over each original, and the
# window whose two ends — the crossfade and two seconds either side — sit at
# the same background level with no event above it. The numbers each choice
# rests on are beside it.
{
  # HOW MANY SOUND AT ONCE: one mpv each, and a fifth is refused with the
  # number.
  atOnce = 4;

  # HOW mpv GOES ROUND, measured by the build with the pinned mpv on every
  # loop (three plays, two seams; the numbers are in the index):
  #
  #   loop-file  the file looped by mpv itself       identical, sample for sample
  #   window     the file twice in mpv's playlist    identical, sample for sample
  #   amovie     `av://lavfi:amovie=…:loop=0`        4 608 samples (96 ms) lost
  #                                                  at every seam
  #
  # `loop-file`, because it is exact and needs nothing kept up: mpv goes round
  # on its own, and a window would be one more event to follow per sound.
  route = "loop-file";

  # THE CEILING ON A LOOP'S PEAKS, in dBTP, after its gain.
  truePeakDb = -1;

  sounds = [
    {
      # A small stream falling over stones: steady, no event from 10 s to
      # 350 s. Seam ends at -53.4 / -53.4 LUFS momentary, peaks within
      # 1.0 dB of them.
      id = "small-waterfall";
      group = "water";
      author = "Wildhorsemann";
      page = "https://freesound.org/people/Wildhorsemann/sounds/346748/";
      licence = "CC0-1.0";
      original = "346748__wildhorsemann__small-stream.wav";
      sha256 = "503de252283d4e54bd16e6b3d77b5325014f8072f1db1f395c96c9f7288d71d8";
      # 120 s of loop and 6 s laid over its start.
      fromSeconds = 140;
      toSeconds = 266;
      fadeSeconds = 6;
      # Recorded at -53 LUFS: +25 dB of gain, peaks at -2.3 dBTP after it.
      loudnessLufs = -28;
    }
    {
      # Heavy rain, and thunder four times inside the loop and never near its
      # seam. Seam ends at -21.2 / -21.2 LUFS momentary, peaks within 0.7 dB.
      id = "rain-and-thunder";
      group = "storm";
      author = "felix.blume";
      page = "https://freesound.org/people/felix.blume/sounds/447510/";
      licence = "CC0-1.0";
      original = "447510__felixblume__rain-and-thunder-in-thailand.wav";
      sha256 = "e44c23b27b17e5e7e1ead7673b99d13c1c2fd86c4c18da53c5b1a2d40581f6bc";
      # 180 s of loop and 6 s laid over its start.
      fromSeconds = 741;
      toSeconds = 927;
      fadeSeconds = 6;
      loudnessLufs = -28;
    }
    {
      # A storm whose rain rises and falls, thunder all through it: the
      # quietest background it holds twice, 140 s apart. Seam ends at -33.2 /
      # -33.3 LUFS momentary; the drops rise to 6.3 dB above that, a thunder
      # 15 to 25 dB.
      id = "thunderstorm";
      group = "storm";
      author = "gepetto3548";
      page = "https://freesound.org/people/gepetto3548/sounds/689986/";
      licence = "CC0-1.0";
      original = "689986__gepetto3548__excellent-thunder-rain-25-sept-21.wav";
      sha256 = "2c72fa7b42a4fb4892c5048ebe6afdf02c34faad396469b98a86f802718a31f5";
      # 140 s of loop and 6 s laid over its start.
      fromSeconds = 123;
      toSeconds = 269;
      fadeSeconds = 6;
      loudnessLufs = -28;
    }
  ];
}
