# The texts `ronoctua-shell-manual` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Help — the shortcut manual (LEADER h s)

cmd-help = Manuais
manual-list = MANUAIS
manual-window-lead = MANUAL:
manual-next = próximo
manual-previous = anterior
manual-open = abrir
manual-window = numa janela
manual-down = descer
manual-up = subir
about-manual-next = O próximo manual da lista.
about-manual-previous = O manual anterior da lista.
about-manual-open = Abre o manual sob o cursor aqui mesmo, no lugar da lista.
about-manual-window = Abre o manual numa janela própria, e o tira do rodapé.
about-manual-down = Desce o manual uma linha.
about-manual-up = Sobe o manual uma linha.
manual-none = Nenhum manual corresponde ao que foi digitado.
manual-filter-empty = Digite para filtrar os manuais.
manual-shortcuts = Atalhos
manual-gestures = Gestos com as mãos
about-manual-shortcuts = Toda tecla que o sistema atende: segurando a leader, os modos do compositor, as teclas de emergência e de mídia.
about-manual-gestures = Todo gesto das mãos que a câmera lê, desenhado, com o que ele faz.
gesture-point-title = Mover o ponteiro
gesture-click-title = Clique esquerdo, e arrastar
gesture-right-click-title = Clique direito
gesture-scroll-title = Rolar
gesture-arrows-title = Setas do teclado
gesture-escape-title = Esc
gesture-focus-title = Mover o foco
gesture-workspace-title = Trocar de espaço de trabalho
gesture-overview-title = Todos os espaços de trabalho
gesture-keyboard-title = Teclado na tela
gesture-player-title = Coluna do player
gesture-close-title = Fechar a janela
gesture-off-title = Desligar os gestos
help-gestures = GESTOS COM AS MÃOS, VISTOS PELA CÂMERA
gesture-point = Indicador e médio
gesture-point-does = Indicador e médio erguidos juntos, polegar dobrado: o ponteiro segue o indicador.
gesture-click = Ponteiro + mão aberta
gesture-click-does = Apontando, a OUTRA mão aberta com os dedos separados: botão esquerdo, segurado enquanto ela ficar aberta.
gesture-right-click = Ponteiro + hang loose
gesture-right-click-does = Apontando, a outra mão só com o polegar e o mínimo esticados: um clique direito.
gesture-scroll = Três erguidos, movendo
gesture-scroll-does = Uma mão, polegar, indicador e médio erguidos: movê-la para cima ou para baixo rola.
gesture-arrows = Os dois indicadores
gesture-arrows-does = Os dois indicadores lado a lado, apontando uma direção: a seta dessa direção.
gesture-escape = Indicadores cruzados
gesture-escape-does = Os dois indicadores cruzados em X: Esc.
gesture-focus = Mão aberta, varrendo
gesture-focus-does = Os cinco dedos erguidos e separados: varrer a mão move o foco nessa direção. A volta não move nada.
gesture-workspace = Duas mãos, três dedos
gesture-workspace-does = As duas mãos com polegar, indicador e médio apontando para cima: o espaço de trabalho acima; para baixo: o de baixo.
gesture-overview = Dez dedos
gesture-overview-does = As duas mãos abertas, lado a lado: todos os espaços de trabalho.
gesture-keyboard = Mãos empilhadas
gesture-keyboard-does = As duas mãos abertas, uma acima da outra: o teclado na tela.
gesture-player = Sinal de rock
gesture-player-does = Indicador e mínimo erguidos: abre a coluna do player, e fecha de novo.
gesture-close = Os dois dedos médios
gesture-close-does = Os dois dedos médios erguidos: fecha a janela em foco.
gesture-off = Braços cruzados
gesture-off-does = Os braços cruzados em X: desliga os gestos e libera a câmera. É o único gesto que desliga, de propósito.
help-chords = SEGURANDO A LEADER
help-emergency = QUANDO A LEADER NÃO RESPONDE
help-leader-alternate = Abrir a lista de comandos
help-media = As teclas do próprio teclado
