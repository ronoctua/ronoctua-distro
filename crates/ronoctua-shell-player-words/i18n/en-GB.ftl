# The texts `ronoctua-shell-player-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

player-toggle = play/pause
player-next = next
# What joins the sources of a set nobody named, in the queue and in a
# hint: in quotes, so the spaces around it stay.
player-set-joiner = {" + "}
tongue-pt-pt = Portuguese (Portugal)
tongue-fr = French
tongue-de = German
tongue-it = Italian
tongue-nl = Dutch
tongue-sv = Swedish
tongue-no = Norwegian
tongue-da = Danish
tongue-fi = Finnish
tongue-is = Icelandic
tongue-pl = Polish
tongue-cs = Czech
tongue-sk = Slovak
tongue-sl = Slovenian
tongue-hr = Croatian
tongue-sr = Serbian
tongue-bs = Bosnian
tongue-mk = Macedonian
tongue-bg = Bulgarian
tongue-ro = Romanian
tongue-hu = Hungarian
tongue-el = Greek
tongue-tr = Turkish
tongue-ru = Russian
tongue-uk = Ukrainian
tongue-be = Belarusian
tongue-lt = Lithuanian
tongue-lv = Latvian
tongue-et = Estonian
tongue-ka = Georgian
tongue-hy = Armenian
tongue-az = Azerbaijani
tongue-sq = Albanian
tongue-ca = Catalan
tongue-eu = Basque
tongue-gl = Galician
tongue-eo = Esperanto
tongue-ar = Arabic
tongue-he = Hebrew
tongue-fa = Persian
tongue-ku = Kurdish
tongue-ur = Urdu
tongue-hi = Hindi
tongue-bn = Bengali
tongue-ta = Tamil
tongue-te = Telugu
tongue-ml = Malayalam
tongue-si = Sinhala
tongue-th = Thai
tongue-vi = Vietnamese
tongue-id = Indonesian
tongue-ms = Malay
tongue-tl = Tagalog
tongue-my = Burmese
tongue-km = Khmer
tongue-zh-cn = Chinese (simplified)
tongue-zh-tw = Chinese (traditional)
tongue-zh = Chinese
tongue-ko = Korean
tongue-sw = Swahili
tongue-af = Afrikaans
hint-player = { $what }. A click plays or pauses; the right button opens the column, or closes it.
hint-player-next = Next: { $what }. The right button opens the player here.
hint-player-next-shuffled = Next, in the shuffled order: { $what }. The right button opens the player here.
hint-player-next-end = This is the last item in the queue: next stops the player. The right button opens the player here.
# The modeline's pair while only the ambient sounds are on (`player_mark`).
hint-player-ambience = Hold the ambient sounds, or let them go on
hint-player-next-ambience = Only the ambient sounds are on: the queue has nothing next
