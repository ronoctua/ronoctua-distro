# WHAT A SOUND PORT IS CALLED — one place, and generated from here.
#
# ALSA names the sockets on a card, and PipeWire hands the names on: `[Out]
# Speaker`, `[In] Mic1`, `analog-output-headphones`. Two of those are English
# and one is not a word at all, and none of them is translated by anybody
# upstream. This file is where our own words for them live, in the same shape
# as `assets/bluetooth-profiles.nix`: it becomes the Rust table, the list of
# text keys BOTH languages have to carry, and the refusal of an entry that
# names nothing.
#
# # THE KEY IS THE NORMALISED NAME, NOT THE RAW ONE
#
# `ronoctua_audio::Route::plain` strips the `[Out] ` / `[In] ` that the UCM
# scheme puts in front — the direction is a FIELD, and saying it twice would
# put "Output" in a list already headed by it — then lower-cases and squeezes
# the separators. `[Out] Speaker`, `analog-output-speaker` and `Speaker` all
# arrive here as `speaker`.
#
# That is also why the direction is not part of the key: a headset's
# microphone and its earpieces are two routes with two names, and a name that
# means one thing going out and another coming in has not turned up.
#
# # WHY THE LIST STOPS WHERE IT DOES, AND WHAT HAPPENS PAST IT
#
# It covers what the machines here actually report, plus the obvious siblings.
# It is NOT a table of every port ALSA can name, and it never will be: the
# HDMI ones carry a number (`HDMI / DisplayPort 3`), a USB interface names
# itself, and a table of sentences nobody wrote is a table nobody can check.
#
# A name that is not here is drawn with **PipeWire's own description**, in
# PipeWire's own words. That is the honest answer and it is the same rule the
# notification list keeps for a foreign application's label: our words where
# we have them, theirs where we do not — never `[audio-port-…]` in the middle
# of a list.
{
  # ----------------------------------------------------------------- output
  speaker = {
    key = "audio-port-speaker";
  };
  speakers = {
    key = "audio-port-speaker";
  };
  headphones = {
    key = "audio-port-headphones";
  };
  headphone = {
    key = "audio-port-headphones";
  };
  # The socket that cannot tell a headset from headphones, which is most
  # laptops with one hole.
  headset = {
    key = "audio-port-headset";
  };
  # A line out is not a speaker: nothing comes out of the machine itself.
  line-out = {
    key = "audio-port-line-out";
  };
  # ------------------------------------------------------------------ input
  # `Mic1` and `Mic2` are what the UCM calls the two on this laptop, and the
  # DESCRIPTION beside them says which is which — `Digital Microphone` and
  # `Stereo Microphone`. Naming them "microphone" here would lose that, so
  # they are deliberately absent: two rows saying the same word would be worse
  # than two rows in English.
  internal-mic = {
    key = "audio-port-internal-mic";
  };
  internal-microphone = {
    key = "audio-port-internal-mic";
  };
  mic = {
    key = "audio-port-microphone";
  };
  microphone = {
    key = "audio-port-microphone";
  };
  headset-mic = {
    key = "audio-port-headset-mic";
  };
  headphone-mic = {
    key = "audio-port-headset-mic";
  };
  line-in = {
    key = "audio-port-line-in";
  };
}
