# The texts `ronoctua-shell-audio-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

audio-nothing-plays = Não há por onde tocar.
audio-nothing-listens = Não há microfone.
audio-no-pipewire = O servidor de som não responde.
echo-audio-refused = O servidor de som { $why } ({ $words })
audio-refused-gone = não tem mais esse aparelho
audio-refused-absent = não está respondendo
audio-refused-invalid = não aceitou esse valor
audio-refused-other = recusou
