# The texts `ronoctua-shell-player-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

player-toggle = tocar/pausar
player-next = próximo
# O que junta as fontes de um conjunto sem nome, na fila e numa dica:
# entre aspas, para os espaços em volta ficarem.
player-set-joiner = {" + "}
tongue-pt-pt = Português (Portugal)
tongue-fr = Francês
tongue-de = Alemão
tongue-it = Italiano
tongue-nl = Holandês
tongue-sv = Sueco
tongue-no = Norueguês
tongue-da = Dinamarquês
tongue-fi = Finlandês
tongue-is = Islandês
tongue-pl = Polonês
tongue-cs = Tcheco
tongue-sk = Eslovaco
tongue-sl = Esloveno
tongue-hr = Croata
tongue-sr = Sérvio
tongue-bs = Bósnio
tongue-mk = Macedônio
tongue-bg = Búlgaro
tongue-ro = Romeno
tongue-hu = Húngaro
tongue-el = Grego
tongue-tr = Turco
tongue-ru = Russo
tongue-uk = Ucraniano
tongue-be = Bielorrusso
tongue-lt = Lituano
tongue-lv = Letão
tongue-et = Estoniano
tongue-ka = Georgiano
tongue-hy = Armênio
tongue-az = Azerbaijano
tongue-sq = Albanês
tongue-ca = Catalão
tongue-eu = Basco
tongue-gl = Galego
tongue-eo = Esperanto
tongue-ar = Árabe
tongue-he = Hebraico
tongue-fa = Persa
tongue-ku = Curdo
tongue-ur = Urdu
tongue-hi = Hindi
tongue-bn = Bengali
tongue-ta = Tâmil
tongue-te = Telugo
tongue-ml = Malaiala
tongue-si = Cingalês
tongue-th = Tailandês
tongue-vi = Vietnamita
tongue-id = Indonésio
tongue-ms = Malaio
tongue-tl = Tagalo
tongue-my = Birmanês
tongue-km = Khmer
tongue-zh-cn = Chinês (simplificado)
tongue-zh-tw = Chinês (tradicional)
tongue-zh = Chinês
tongue-ko = Coreano
tongue-sw = Suaíli
tongue-af = Africâner
hint-player = { $what }. Um clique toca ou pausa; o botão direito abre a coluna, ou a fecha.
hint-player-next = A seguir: { $what }. O botão direito abre o player aqui.
hint-player-next-shuffled = A seguir, na ordem embaralhada: { $what }. O botão direito abre o player aqui.
hint-player-next-end = Este é o último item da fila: avançar faz o player parar. O botão direito abre o player aqui.
# The modeline's pair while only the ambient sounds are on (`player_mark`).
hint-player-ambience = Pausar os sons ambientes, ou deixá-los continuar
hint-player-next-ambience = Só os sons ambientes estão ligados: a fila não tem o que vem depois
