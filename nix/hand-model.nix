# The hand models the gestures (`LEADER m g`) run: where the hands are.
#
# MediaPipe's `hand_landmarker`, float16, VERSION 1 — Apache-2.0 per its
# model card, the same numbered-address rule `nix/segment-model.nix` states
# for `downloads-are-pinned`. The `.task` is a zip of TWO models, and the
# studio reads them as files: the palm detector (192²) and the landmark
# model (224²), unpacked here so nothing at run time reads a zip.
#
# Read at RUN TIME (`RONOCTUA_HAND_MODELS`, a directory), never compiled in,
# and absence is not an error: the studio refuses a reader of hands with a
# sentence, and the rest of the camera works.
{
  fetchurl,
  runCommand,
  unzip,
}:

let
  version = "1";
  task = fetchurl {
    name = "hand_landmarker-float16-${version}.task";
    url = "https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/${version}/hand_landmarker.task";
    hash = "sha256-+8KjAIDDxVcJO13fwzRpgTLrNBBEzO4yLM+LzzYHzeE=";
  };
in
runCommand "hand-models-${version}"
  {
    nativeBuildInputs = [ unzip ];
    passthru = { inherit task; };
  }
  ''
    mkdir -p $out
    unzip -q ${task} hand_detector.tflite hand_landmarks_detector.tflite -d $out
  ''
