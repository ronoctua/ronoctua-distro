# The texts `ronoctua-shell-audio` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Syncthing (LEADER s s)

# Syncthing (LEADER s s) — one view, two doors, and the mark the
# modeline may draw. The option labels below are owed by
# `assets/syncthing-options.nix` through the generator; `syncthing-options-have-text`
# is what says so.
echo-audio-will-not-parse = audio.json will not parse, and writing would replace it. Fix or remove the file.
# =====================================================================
# SOUND — `LEADER s a`, two doors, and the marks the modeline draws
# =====================================================================
group-audio = sound
audio-title = Sound
cmd-audio-open = Sound, in the footer
cmd-audio-window = Sound, in a window
cmd-audio-up = Previous row
cmd-audio-down = Next row
cmd-audio-enter = Use this one
cmd-audio-less = Quieter, or the previous value
cmd-audio-more = Louder, or the next value
cmd-audio-mute = Silence this, or give it back
cmd-audio-default = Play through this one from now on
audio-up = up
audio-down = down
audio-enter = use
audio-less = quieter
audio-more = louder
audio-mute = silence
audio-default = make it the one
# The headings, and the order they are read in.
audio-heading-output = Playing through
audio-heading-input = Listening with
audio-heading-apps = Programs
audio-heading-card = The card
audio-heading-settings = Settings
audio-chosen-elsewhere = Chosen: { $device }, which is not here
audio-nothing-runs = No program is playing or recording.
audio-muted = silent
audio-in-use = in use
audio-profile = What the card is doing
audio-socket-out = Playing out of
audio-socket-in = Recording from
audio-port-none = No socket is in use.
audio-unplugged = nothing plugged in
audio-card-off = This card is switched off.
audio-listening-now = listening now
audio-setting-step = One press moves
audio-setting-over = Allow more than full
audio-setting-flash = Show the line above the modeline
audio-on = yes
audio-off = no
audio-step-points = { $points }%
# The ports we have our own word for. What is not here draws in PipeWire's.
audio-port-speaker = Speakers
audio-port-headphones = Headphones
audio-port-headset = Headset
audio-port-headset-mic = Headset microphone
audio-port-internal-mic = Built-in microphone
audio-port-microphone = Microphone
audio-port-line-in = Line in
audio-port-line-out = Line out
