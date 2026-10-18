# The texts `ronoctua-shell-display-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Telas — os monitores, a imagem deles e a luz (LEADER s d)

display-refused-no-compositor = não há compositor para perguntar
display-refused-no-screen = essa tela não está mais aí
display-refused-no-light = o brilho desta tela está nos botões dela
display-refused-not-writable = o arquivo é do root, e a regra que o abre não rodou neste painel
display-refused-nowhere = não há onde escrever
display-refused-other = não deu
echo-display-refused-plain = As telas recusaram: { $why }

## file info editing — everything the info panel can change

display-refused-compositor = o compositor respondeu com um erro
display-refused-unreadable = o arquivo das telas não abre, e gravar o substituiria — conserte ou apague
display-refused-panel = o painel não aceitou o brilho
display-refused-monitor = o monitor não respondeu
