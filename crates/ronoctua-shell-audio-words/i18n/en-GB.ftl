# The texts `ronoctua-shell-audio-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

audio-nothing-plays = There is nothing to play through.
audio-nothing-listens = There is no microphone.
audio-no-pipewire = The sound server is not answering.
echo-audio-refused = The sound server { $why } ({ $words })
audio-refused-gone = no longer has that device
audio-refused-absent = is not answering
audio-refused-invalid = would not take that value
audio-refused-other = refused
