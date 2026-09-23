# The person-segmentation model the camera's background effects run.
#
# MediaPipe's `selfie_segmenter`, float16, VERSION 1 — Apache-2.0 per its
# model card. The address names the version (`.../float16/1/...`), which is
# what `downloads-are-pinned` asks: Google publishes a `latest` beside it that
# moves, and the hash below is only true of the numbered one.
#
# Read at RUN TIME (`RONOCTUA_SEGMENT_MODEL`, in `atRunTime`), never compiled
# in: 249 537 bytes that a model update would otherwise turn into a rebuild of
# every crate, and absence is not an error — the three background effects are
# drawn REFUSING with a sentence, the rest of the camera works.
#
# `ronoctua-segment` runs it with its own interpreter; the Cargo.toml there
# says why no runtime from crates.io could.
{ fetchurl }:

let
  version = "1";
in
fetchurl {
  name = "selfie_segmenter-float16-${version}.tflite";
  url = "https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter/float16/${version}/selfie_segmenter.tflite";
  hash = "sha256-GRrJUprlBu4L7vprLJRaFy2rnQfR6AKikKTkA4ImZYs=";
}
