# The texts `ronoctua-shell-overlays` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Installer — the interface

cmd-screen-capture = Capturar a tela
capture-drag = arraste para uma região
capture-click = clique numa janela
capture-whole = a tela inteira
about-capture-whole = Guarda a tela com foco inteira, como estava quando a captura começou.
capture-loupe = a lupa
about-capture-loupe = Mostra ou esconde a lupa ao lado do ponteiro: os pixels sob ele, ampliados, com aquele em que ele está contornado.
capture-hide-hint = esconder esta linha
about-capture-hide-hint = Guarda esta linha de atalhos, esteja o ponteiro onde estiver; a mesma tecla a traz de volta.
capture-cancel = cancelar
about-capture-cancel = Desiste da captura e não guarda nada.
capture-size = { $width } × { $height }
capture-at = { $x }, { $y }
cmd-dev-resizer = Redimensionar a tela inteira
resizer-size = { $width } × { $height } px
resizer-grid = { $columns } colunas × { $lines } linhas
resizer-off = tela inteira
about-resizer-off = Encerra o redimensionamento: as cortinas saem e a tela volta inteira.
resizer-narrower = mais estreita
about-resizer-narrower = Uma coluna a menos, ou várias com Shift pressionado.
resizer-wider = mais larga
about-resizer-wider = Uma coluna a mais, ou várias com Shift pressionado.
resizer-shorter = mais baixa
about-resizer-shorter = Uma linha a menos, ou várias com Shift pressionado.
resizer-taller = mais alta
about-resizer-taller = Uma linha a mais, ou várias com Shift pressionado.
resizer-largest = a maior
about-resizer-largest = Do maior tamanho que as cortinas deixam — tudo menos as duas faixas que elas guardam para serem pegas.

## file info editing — everything the info panel can change

# Os tamanhos prontos do redimensionador e a memória dele (`resizer.rs`).
resizer-preset = tamanho pronto
about-resizer-preset = O próximo tamanho pronto de uma tela comum que cabe nesta.
resizer-preset-back = menor
about-resizer-preset-back = O tamanho pronto anterior a este.
resizer-no-preset = nenhum tamanho pronto cabe nesta tela
