# The texts `ronoctua-shell-footer` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Commands — the `M-x` list and the which-key labels

cmd-mx-open = Executar comando…
cmd-buffer-terminal-external = Abrir terminal externo
cmd-monitor-btop = Monitor de processos (btop)
cmd-monitor-bottom = Monitor de processos (bottom)

## which-key prefix groups

group-quit = sair

## The buffer's state — the modeline's third left segment

# WHAT THE BUFFER IS DOING, next to what kind of buffer it is. The id is
# ENGLISH and declared in the component's descriptor; these are what reach the
# screen. `Esc` always returns to `normal`, and every state that is not it is
# drawn in the one colour the modeline reserves for an alternative.
state-normal = normal
state-filter = filtro
state-sheet = ficha
state-detail = detalhe
state-full = completo
state-guard = proteção
state-choose = escolha
state-confirm = confirmação
state-task = tarefa
state-log = registro

## Notifications

# O CATÁLOGO — as notificações da própria distro, com um resumo e um corpo
# cada. Para uma entrada REESCRITA (a captura de tela do compositor), é isto
# que chega à tela. Para uma que nós mesmos levantamos, é o texto padrão e o
# nome da entrada no gerenciador.
notif-screenshot-summary = Tela capturada
notif-screenshot-body = A imagem foi salva no disco.
notif-screenshot-app = tela
notif-studio-photo-summary = Foto tirada
notif-studio-photo-body = A foto foi salva no disco.
notif-studio-recording-summary = Gravação salva
notif-studio-recording-body = A gravação foi salva no disco.
notif-studio-app = câmera
notif-memory-killed-summary = Fechado para liberar memória

## Installer — the interface

echo-no-on = { $sequence } não está ligado a nada
# Same sequence, other case. Shown when the case IS the reason nothing
# matched — Caps Lock or Shift. Without it the screen names the consequence
# and hides the cause.
echo-no-on-box = { $sequence } não está ligado — { $other } está
echo-cancelled = Cancelado
echo-no-window = Nenhuma janela em foco
echo-no-action = { $command } ainda não faz nada
cmd-window-right = Focar à direita
cmd-window-up = Focar acima
cmd-window-down = Focar abaixo
cmd-window-move-right = Mover à direita
cmd-window-move-up = Mover acima
cmd-window-move-down = Mover abaixo
cmd-window-maximize = Maximizar coluna
cmd-window-screen-full = Tela cheia
cmd-window-center = Centralizar coluna
cmd-window-consume = Absorver à direita
cmd-window-expel = Expelir à esquerda
cmd-workspace-next = Próximo espaço
cmd-workspace-previous = Espaço anterior
cmd-workspace-move-next = Levar a janela para o próximo espaço
cmd-workspace-move-previous = Levar a janela para o espaço anterior
echo-capture-failed = A captura não pôde ser feita — o motivo está no log
group-window = janela/buffer
group-terminal = terminal
group-workspace = workspace
cmd-workspace-overview = Panorama dos espaços
cmd-window-float = Alternar flutuante
cmd-workspace-all = Ver todos os espaços de uma vez
cmd-workspace-move = Levar a janela para o espaço

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-keyboard-restore = Restaurar o teclado
echo-keyboard-restored = Teclado restaurado
echo-keyboard-failed = Não consegui restaurar o teclado: { $reason }

## O fim da sessão

echo-session-ending = Deixando seus programas salvarem antes de a sessão acabar — encerre de novo para sair já

## Comandos do compositor — §13, "toda ação do SO tem um comando"

# São os comandos do compositor, cada um alcançável pelo nome no localizador.
# A lista é DECLARADA por nós em `assets/compositor-deeds.nix` — não espelhada
# da ajuda de outro projeto — e o gerador recusa um verbo que o compositor
# pinado não tem. Uma linha acrescentada lá sem uma frase aqui derruba o
# build, que é o contrário de aparecer sem tradução no meio da lista.

cmd-wm-focus-left = Focar a janela à esquerda
cmd-wm-focus-right = Focar a janela à direita
cmd-wm-focus-up = Focar a janela acima
cmd-wm-focus-down = Focar a janela abaixo
cmd-wm-focus-beginning = Focar a primeira janela da fita
cmd-wm-focus-end = Focar a última janela da fita
cmd-wm-focus-parent = Focar o contêiner em que esta janela está
cmd-wm-focus-child = Focar dentro do contêiner
cmd-wm-focus-tiling = Focar as janelas ladrilhadas
cmd-wm-focus-floating = Focar as janelas flutuantes
cmd-wm-focus-either = Alternar o foco entre ladrilhada e flutuante
cmd-wm-focus-screen-left = Focar a tela à esquerda
cmd-wm-focus-screen-right = Focar a tela à direita
cmd-wm-focus-screen-up = Focar a tela acima
cmd-wm-focus-screen-down = Focar a tela abaixo
cmd-wm-move-left = Mover a janela para a esquerda
cmd-wm-move-right = Mover a janela para a direita
cmd-wm-move-up = Mover a janela para cima
cmd-wm-move-down = Mover a janela para baixo
cmd-wm-move-beginning = Levar a janela ao começo da fita
cmd-wm-move-end = Levar a janela ao fim da fita
cmd-wm-move-out-left = Tirar a janela da coluna, para a esquerda
cmd-wm-move-out-right = Tirar a janela da coluna, para a direita
cmd-wm-move-out-up = Mover a janela para cima dentro da coluna
cmd-wm-move-out-down = Mover a janela para baixo dentro da coluna
cmd-wm-move-screen-left = Levar a janela para a tela da esquerda
cmd-wm-move-screen-right = Levar a janela para a tela da direita
cmd-wm-close = Fechar a janela em foco
cmd-wm-close-others = Fechar todas as janelas menos esta
cmd-wm-full-screen = Encher a tela com esta janela
cmd-wm-full-layout = Encher o layout com esta janela, mantendo-a ladrilhada
cmd-wm-full-application = Deixar o aplicativo decidir o que é tela cheia
cmd-wm-full-global = Encher todas as telas com esta janela
cmd-wm-float = Fazer esta janela flutuar, ou devolvê-la ao layout
cmd-wm-stick = Manter esta janela em todas as áreas de trabalho
cmd-wm-calm = Parar de pedir atenção para esta janela
cmd-wm-workspace-send = Enviar esta janela a um espaço, e ficar onde você está
cmd-wm-opacity-more = Deixar esta janela mais opaca
cmd-wm-opacity-less = Deixar esta janela mais transparente
cmd-wm-opacity-full = Deixar esta janela opaca de novo
cmd-wm-awake-visible = Manter a tela acordada enquanto esta janela estiver visível
cmd-wm-awake-focus = Manter a tela acordada enquanto esta janela estiver em foco
cmd-wm-awake-fullscreen = Manter a tela acordada enquanto esta janela estiver em tela cheia
cmd-wm-awake-open = Manter a tela acordada enquanto esta janela estiver aberta
cmd-wm-awake-none = Deixar a tela dormir, faça esta janela o que fizer
cmd-wm-tearing-on = Deixar esta janela ter tearing, pela menor latência
cmd-wm-tearing-off = Tirar o tearing desta janela
cmd-wm-border-none = Não desenhar borda em volta desta janela
cmd-wm-border-pixel = Desenhar uma borda fina em volta desta janela
cmd-wm-border-normal = Desenhar borda e barra de título em volta desta janela
cmd-wm-border-csd = Deixar esta janela desenhar as próprias decorações
cmd-wm-shadow-on = Desenhar uma sombra sob esta janela
cmd-wm-shadow-off = Não desenhar sombra sob esta janela
cmd-wm-dim-on = Escurecer esta janela quando ela não estiver em foco
cmd-wm-dim-off = Nunca escurecer esta janela
cmd-wm-columns = Janela nova abre numa coluna própria
cmd-wm-rows = Janela nova abre embaixo desta
cmd-wm-transpose = Virar o layout de lado
cmd-wm-width-next = Alargar a coluna para o tamanho seguinte
cmd-wm-width-previous = Estreitar a coluna para o tamanho anterior
cmd-wm-height-next = Deixar a janela mais alta
cmd-wm-height-previous = Deixar a janela mais baixa
cmd-wm-width-set = Definir a largura da coluna, em fração da tela
cmd-wm-height-set = Definir a altura da janela, em fração da tela
cmd-wm-fit-columns = Repartir a tela entre as colunas
cmd-wm-fit-rows = Repartir a coluna entre as janelas dela
cmd-wm-fit-visible = Ajustar as colunas que estão na tela
cmd-wm-maximise = Dar a tela inteira a esta coluna
cmd-wm-align-left = Segurar esta coluna na borda esquerda
cmd-wm-align-right = Segurar esta coluna na borda direita
cmd-wm-align-centre = Segurar esta coluna no meio
cmd-wm-align-top = Segurar esta janela no topo
cmd-wm-align-bottom = Segurar esta janela embaixo
cmd-wm-align-middle = Segurar esta janela no meio da coluna
cmd-wm-align-auto = Deixar o layout posicionar esta coluna de novo
cmd-wm-pin-beginning = Fixar esta coluna à esquerda da tela
cmd-wm-pin-end = Fixar esta coluna à direita da tela
cmd-wm-select = Marcar esta janela, ou desmarcá-la
cmd-wm-select-none = Limpar todas as marcas
cmd-wm-select-workspace = Marcar todas as janelas desta área de trabalho
cmd-wm-select-move = Trazer para cá as janelas marcadas
cmd-wm-select-to-trail = Transformar as janelas marcadas numa trilha
cmd-wm-trailmark = Pôr esta janela na trilha, ou tirá-la
cmd-wm-trailmark-next = Ir à próxima janela da trilha
cmd-wm-trailmark-previous = Ir à janela anterior da trilha
cmd-wm-trailmark-jump = Pular para uma janela da trilha pelo número desenhado nela
cmd-wm-trail-new = Começar uma trilha nova
cmd-wm-trail-next = Usar a próxima trilha
cmd-wm-trail-previous = Usar a trilha anterior
cmd-wm-trail-clear = Esvaziar esta trilha
cmd-wm-trail-delete = Apagar esta trilha
cmd-wm-jump = Pular para uma janela digitando o número desenhado nela
cmd-wm-jump-tiling = Pular para uma janela ladrilhada pelo número desenhado nela
cmd-wm-jump-everywhere = Pular para qualquer janela pelo número desenhado nela
cmd-wm-jump-container = Pular para um contêiner pelo número desenhado nele
cmd-wm-jump-workspaces = Pular para uma área de trabalho pelo número desenhado nela
cmd-wm-jump-floating = Pular para uma janela flutuante pelo número desenhado nela
cmd-wm-overview = Ver a área de trabalho inteira de uma vez
cmd-wm-overview-all = Ver todas as áreas de trabalho de uma vez
cmd-wm-overview-all-on = Mostrar todos os espaços de uma vez
cmd-wm-overview-all-off = Voltar de todos os espaços para um
cmd-wm-zoom-out = Afastar um pouco a área de trabalho
cmd-wm-zoom-in = Aproximar um pouco a área de trabalho
cmd-wm-zoom-reset = Devolver a área de trabalho ao tamanho dela
cmd-wm-content-smaller = Encolher o que está dentro desta janela
cmd-wm-content-bigger = Aumentar o que está dentro desta janela
cmd-wm-content-reset = Devolver o conteúdo desta janela ao tamanho dele
cmd-wm-ring-previous = Voltar pelas janelas em que você acabou de estar
cmd-wm-ring-next = Avançar pelas janelas em que você acabou de estar
cmd-wm-ring-first = Ir à janela mais antiga do anel
cmd-wm-ring-last = Ir à janela mais recente do anel
cmd-wm-ring-set = Pôr esta janela no fim do anel
cmd-wm-ring-remove = Tirar esta janela do anel
cmd-wm-workspace-next = Ir à próxima área de trabalho desta tela
cmd-wm-workspace-previous = Ir à área de trabalho anterior desta tela
cmd-wm-workspace-next-screen = Ir à próxima área de trabalho, em qualquer tela
cmd-wm-workspace-previous-screen = Ir à área de trabalho anterior, em qualquer tela
cmd-wm-workspace-back = Voltar à área de trabalho de onde você veio
cmd-wm-workspace-go = Ir a uma área de trabalho pelo número
cmd-wm-workspace-move = Levar esta janela a uma área de trabalho e ir junto
cmd-wm-workspace-move-next = Levar esta janela à próxima área de trabalho
cmd-wm-workspace-move-previous = Levar esta janela à área de trabalho anterior
cmd-wm-workspace-swap = Trocar esta área de trabalho com outra pelo número
cmd-wm-workspace-split = Mostrar duas áreas de trabalho lado a lado
cmd-wm-workspace-split-reset = Voltar a mostrar uma área de trabalho
cmd-wm-workspace-to-screen-left = Levar esta área de trabalho para a tela da esquerda
cmd-wm-workspace-to-screen-right = Levar esta área de trabalho para a tela da direita
cmd-wm-scratchpad-put = Guardar esta janela na gaveta
cmd-wm-scratchpad-show = Tirar uma janela da gaveta
cmd-wm-scratchpad-jump = Pular para uma janela da gaveta pelo número desenhado nela
cmd-wm-screens-off = Desligar todas as telas
cmd-wm-screens-on = Ligar todas as telas de novo
cmd-wm-keep-shortcuts = Dar os atalhos a esta janela, ou tomá-los de volta
cmd-wm-close-all = Fechar todas as janelas desta área de trabalho
cmd-wm-reload = Reler a configuração do compositor
cmd-wm-move-out-beginning = Tirar a janela da coluna dela, para o começo da rolagem
cmd-wm-move-out-end = Tirar a janela da coluna dela, para o fim da rolagem
cmd-wm-workspace-swap-name = Trocar o número desta área de trabalho com o de outra, sem mexer nas janelas
cmd-wm-workspace-split-v = Dividir a área de trabalho em cima e embaixo, numa fração
cmd-wm-workspace-split-h = Dividir a área de trabalho lado a lado, numa fração
cmd-wm-jump-tiling-all = Pular para uma janela lado a lado em qualquer tela
cmd-wm-jump-floating-all = Pular para uma janela flutuante em qualquer tela
cmd-wm-jump-everywhere-all = Pular para qualquer janela em qualquer tela
cmd-wm-jump-trailmark = Pular para uma janela marcada
cmd-wm-jump-trailmark-all = Pular para uma janela marcada em qualquer tela
cmd-wm-filter-tiling = Mostrar só as janelas lado a lado, nesta tela
cmd-wm-filter-tiling-all = Mostrar só as janelas lado a lado, em todas as telas
cmd-wm-filter-tiling-only = Mostrar só as janelas lado a lado, só nesta área de trabalho
cmd-wm-filter-container = Mostrar só esta coluna, nesta tela
cmd-wm-filter-container-all = Mostrar só esta coluna, em todas as telas
cmd-wm-filter-container-only = Mostrar só esta coluna, só nesta área de trabalho
cmd-wm-filter-floating = Mostrar só as janelas flutuantes, nesta tela
cmd-wm-filter-floating-all = Mostrar só as janelas flutuantes, em todas as telas
cmd-wm-filter-floating-only = Mostrar só as janelas flutuantes, só nesta área de trabalho
cmd-wm-filter-trailmark = Mostrar só as janelas marcadas, nesta tela
cmd-wm-filter-trailmark-all = Mostrar só as janelas marcadas, em todas as telas
cmd-wm-filter-trailmark-only = Mostrar só as janelas marcadas, só nesta área de trabalho
cmd-wm-filter-visible = Mostrar só as janelas à vista, nesta tela
cmd-wm-filter-visible-all = Mostrar só as janelas à vista, em todas as telas
cmd-wm-filter-visible-only = Mostrar só as janelas à vista, só nesta área de trabalho
cmd-wm-filter-reset = Mostrar todas as janelas de novo
cmd-wm-placement-after = Janelas novas abrem depois da que tem o foco
cmd-wm-placement-before = Janelas novas abrem antes da que tem o foco
cmd-wm-placement-beginning = Janelas novas abrem no começo da rolagem
cmd-wm-placement-end = Janelas novas abrem no fim da rolagem
cmd-wm-placement-focus = Janelas novas ganham o foco
cmd-wm-placement-no-focus = Janelas novas abrem sem tomar o foco
cmd-wm-centre-horizontal = Manter a janela com foco centralizada na horizontal
cmd-wm-no-centre-horizontal = Parar de centralizar a janela com foco na horizontal
cmd-wm-centre-vertical = Manter a janela com foco centralizada na vertical
cmd-wm-no-centre-vertical = Parar de centralizar a janela com foco na vertical
cmd-wm-reorder-auto = Reordenar as janelas conforme o foco anda
cmd-wm-no-reorder-auto = Parar de reordenar as janelas conforme o foco anda
cmd-wm-fit-fraction = Janelas novas ocupam uma fração da tela
cmd-wm-fit-split = Janelas novas dividem a coluna em que abrem
cmd-wm-no-fit = Janelas novas ficam do tamanho que pedem
cmd-wm-resize-narrower = Deixar a janela mais estreita
cmd-wm-resize-wider = Deixar a janela mais larga
cmd-wm-resize-shorter = Deixar a janela mais baixa
cmd-wm-resize-taller = Deixar a janela mais alta
cmd-wm-resize-shrink-right = Recolher a borda direita da janela
cmd-wm-resize-grow-right = Estender a borda direita da janela
cmd-wm-resize-shrink-down = Recolher a borda de baixo da janela
cmd-wm-resize-grow-down = Estender a borda de baixo da janela
cmd-wm-resize-shrink-left = Recolher a borda esquerda da janela
cmd-wm-resize-grow-left = Estender a borda esquerda da janela
cmd-wm-resize-shrink-up = Recolher a borda de cima da janela
cmd-wm-resize-grow-up = Estender a borda de cima da janela
cmd-wm-nudge-left = Mover a janela flutuante para a esquerda
cmd-wm-nudge-right = Mover a janela flutuante para a direita
cmd-wm-nudge-up = Mover a janela flutuante para cima
cmd-wm-nudge-down = Mover a janela flutuante para baixo
cmd-wm-toggle-size-this = Encher a tela com esta janela, ou devolver o tamanho dela
cmd-wm-toggle-size-active = Encher a tela com a janela com foco, ou devolver o tamanho dela
cmd-wm-toggle-size-width = Dar uma largura a todas as janelas, ou devolver a delas
cmd-wm-toggle-size-height = Dar uma altura a todas as janelas, ou devolver a delas
cmd-wm-toggle-size-reset = Esquecer os tamanhos alternados
cmd-wm-fit-rows-visible = Dividir a altura entre as janelas à vista, mantendo as proporções
cmd-wm-fit-columns-visible-equal = Dividir a largura igualmente entre as janelas à vista
cmd-wm-fit-rows-visible-equal = Dividir a altura igualmente entre as janelas à vista
cmd-wm-fit-columns-to-end = Ajustar a largura desta janela até o fim, mantendo as proporções
cmd-wm-fit-rows-to-end = Ajustar a altura desta janela até o fim, mantendo as proporções
cmd-wm-fit-columns-to-end-equal = Ajustar a largura desta janela até o fim, igualmente
cmd-wm-fit-rows-to-end-equal = Ajustar a altura desta janela até o fim, igualmente
cmd-wm-fit-columns-to-start = Ajustar a largura do começo até esta janela, mantendo as proporções
cmd-wm-fit-rows-to-start = Ajustar a altura do começo até esta janela, mantendo as proporções
cmd-wm-fit-columns-to-start-equal = Ajustar a largura do começo até esta janela, igualmente
cmd-wm-fit-rows-to-start-equal = Ajustar a altura do começo até esta janela, igualmente
cmd-wm-fit-rows-active = Ajustar a altura da janela com foco
cmd-wm-fit-columns-all = Dividir a largura entre todas as janelas, mantendo as proporções
cmd-wm-fit-rows-all = Dividir a altura entre todas as janelas, mantendo as proporções
cmd-wm-trail-number = Ir para uma trilha numerada
cmd-wm-trail-to-selection = Selecionar as janelas desta trilha
cmd-wm-space-load = Carregar um arranjo salvo
cmd-wm-space-save = Salvar este arranjo sob um número
cmd-wm-space-restore-hide = Restaurar um arranjo salvo, escondendo as outras janelas
cmd-wm-space-restore = Restaurar um arranjo salvo
cmd-wm-mode-split = Dividir a área de trabalho…
cmd-wm-mode-jump = Pular para uma janela…
cmd-wm-mode-filter = Mostrar só algumas janelas…
cmd-wm-mode-placement = Onde janelas novas vão…
cmd-wm-mode-width = Escolher a largura da janela…
cmd-wm-mode-height = Escolher a altura da janela…
cmd-wm-mode-resize = Redimensionar a janela com as setas…
cmd-wm-mode-floating = Mover e redimensionar uma janela flutuante com as setas…
cmd-wm-mode-toggle-width = Alternar todas as janelas para uma largura…
cmd-wm-mode-toggle-height = Alternar todas as janelas para uma altura…
cmd-wm-mode-align = Alinhar a janela…
cmd-wm-mode-fit = Ajustar as janelas…
cmd-wm-mode-trailmark = Marcar janelas…
cmd-wm-mode-trail = Trilhas de janelas marcadas…
cmd-wm-mode-spaces = Arranjos salvos…
cmd-wm-mode-default = Sair do modo do compositor
group-workspace-move = levar a janela
expansion-page = CtrlDir ←→
echo-clipboard-synced = A área de transferência está sincronizando
echo-clipboard-sync-off = A área de transferência fica nesta máquina
echo-clipboard-sync-failed = Não deu para sincronizar: { $reason }
echo-copy-only-text = Por ora só dá para recopiar texto
# --- The application launcher (LEADER a) -----------------------------------
cmd-apps-open = Abrir aplicativo
apps-empty = Nenhum aplicativo encontrado
# The launcher's own commands, and its last row, which looks for what was typed
# among the programs one can install.
cmd-apps-details = Ver o que é este programa e de onde veio
apps-pin = fixar na barra lateral
apps-details = detalhes
cmd-apps-remove = Remover este programa, depois de uma pergunta
apps-remove = remover
cmd-apps-search-to-install = Procurar o que foi digitado entre os programas para instalar
apps-search-to-install = Procurar “{ $query }” para instalar

## Network — Wi-Fi and Ethernet (LEADER s n)

cmd-theme-open = Temas
cmd-theme-apply = Aplicar este tema e fundo

## Syncthing (LEADER s s)

hint-syncthing-on = Tudo está sendo sincronizado.
hint-syncthing-off = Nada está sendo sincronizado.

## Falta de memoria

notif-oom-app = sistema
notif-oom-pressure = O { $app } estava deixando a maquina travada por falta de memoria.
notif-oom-used = O { $app } encheu a memoria e a area de troca.
notif-oom-orphan = Algo fora dos seus buffers foi fechado: { $cgroup }

## O keymap do buffer em foco

which-buffer = Buffer em foco
hint-state = Estado: { $state } — clique gira, Esc volta, Esc+setas escolhe
which-state = Estado do buffer em foco

## The leader's three sets: the key tapped right after the leader.

cmd-leader-set-window = As mesmas teclas, cada uma abrindo o seu app numa janela em vez do rodapé
cmd-leader-set-full = A árvore inteira de teclas, com cada submenu como era

## O painel de memoria — LEADER s m

cmd-memory-open = Memória do sistema
cmd-example-open = Vários teclados num app, a referência
cmd-format-open = Formatar um disco
group-system = sistema

## Rescue console — what it says

cmd-rescue-open = Console de resgate
cmd-apps-pin = Pôr o aplicativo na barra lateral do gerenciador
files-sidebar-unwritable = não consegui salvar a barra lateral: { $why }
group-geography = geografia
# O `c` deixou de ser um comando e virou grupo: o clipboard captura o que se
# copia, e a tela captura o que se vê.
group-miscellaneous = diversos
transfers-queued-as-me = na fila, COMO VOCÊ — este não é anônimo
transfers-nothing-copied = não há nada copiado
transfers-not-an-address = o que está copiado não é um endereço
transfers-queued = na fila
# ===================================================================
# A TELA DO CÉREBRO — LEADER b, e uma janela só dela
#
# O que a máquina faz sozinha, quando faz, e no que deu.
#
# Nenhuma frase daqui escreve uma data: o buffer as desenha em ISO, porque um
# componente não traduz (invariante 11) e *19 Aug* contra *19 ago* é
# exatamente o tipo de coisa que ele não pode inventar. O que está escrito
# abaixo são as palavras EM VOLTA dos números, nunca os números.
#
# COMO ESTES TEXTOS SÃO ESCRITOS, porque o conjunto anterior foi refeito do
# zero e o dono disse por quê: um título nomeia a coisa que está embaixo dele,
# um rótulo nomeia o campo ao lado, e uma frase diz o que uma coisa faz.
# Nenhum deles é um comentário. `O que ele faz` sobre uma lista de tarefas,
# `A máquina` sobre duas percentagens e `Como foi da última vez` sobre uma
# linha eram todos o mesmo erro — um título falando SOBRE a tela em vez de
# nomear o que está nela.
# ===================================================================

# OS COMANDOS, e são CURTOS de propósito: são as frases da escada do header,
# onde cada uma tem cerca de uma dúzia de células e tudo o que vem depois de
# uma frase longa cai fora da linha.
cmd-brain-open = Brain
cmd-brain-abort = Interromper
cmd-brain-proceed = Começar já
# A FAIXA SOB O MINIBUFFER — uma linha, e só enquanto uma contagem corre.
#
# Não é a tela do cérebro: é o aviso que chega sem ninguém ter pedido, numa
# linha própria abaixo do par (invariante 14 — é a mesma superfície crescendo,
# não um popup). Três frases, porque é o que cabe: o que está para acontecer, e
# as duas respostas a isso.
#
# O nome da própria tarefa carrega o sujeito — `brain-task-<id>` —, então uma
# tarefa acrescentada amanhã herda esta frase em vez de precisar de uma sua.
brain-band-soon = { $task } — começa em { $seconds } s
brain-band-abort = agora não
brain-band-proceed = agora
hint-brain-countdown = quanto falta para começar
hint-update-mark = clique para ver a atualização acontecer

## `theme-backdrop-near`, is offered only under a light theme.

theme-option-theme = Tema
theme-option-backdrop = Variações
theme-hint = { $apply } aplica · { $left }/{ $right } troca de coluna

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

# As duas marcas da modeline, na linha sob o ponteiro.
hint-battery = { $percent }% { $state }, { $time }, { $rate } W — clique abre energia
hint-profile = perfil: { $profile } — clique abre energia
hint-battery-no-rate = { $percent }% { $state }, { $time } — clique abre energia
notif-battery-critical-body-unset = { $percent }% restantes. A máquina vai agir sozinha em breve.
# WHAT AN INHIBITOR HOLDS — nouns, because "holds sleep" is a thing held and the
# lid's `sleep` above is an action; in Portuguese they are two words.
notif-battery-critical-body-ignore = { $percent }% restantes, e nada vai impedir. Ligue na tomada agora.
echo-power-profile-not-offered = { $wanted } não é oferecido por esta máquina; { $applied } aplicado
# OS TRÊS ALARMES, levantados pelo rodapé quando o nível do UPower muda.
notif-power-app = energia
notif-bluetooth-app = bluetooth
notif-bluetooth-pairing-summary = Algo quer parear
notif-bluetooth-pairing-keys = { $no } responde não, { $yes } responde sim — depois que este aviso sai.
# O QUE A MÁQUINA FEZ, dito na linha do minibuffer (`events.rs`). O resumo é a
# linha; o corpo só é lido quando a linha estava ocupada e o evento veio como
# notificação — e então a última frase diz isso.
notif-privacy-app = privacidade
notif-network-app = rede
notif-minibuffer-was-busy = Isto costuma aparecer no minibuffer, mas ele estava ocupado, então veio como notificação.
notif-network-lost-summary = { $network } caiu
hint-net-off = conectado, e nada do outro lado — clique abre a rede
hint-net-portal = algo está pedindo que você entre — clique abre a rede
hint-airplane = modo avião: todo rádio e cabo desligado — clique liga de volta
hint-vpn = { $name } está de pé — clique abre a rede
hint-tailscale = no tailnet { $tailnet } — clique abre
hint-tor = saindo pelo Tor — clique abre
hint-clip-on = a área de transferência está sincronizando — clique abre
hint-clip-off = a área de transferência não está sincronizando — clique abre
hint-history-on = o histórico do shell está sincronizando — clique abre
hint-history-off = o histórico do shell não está sincronizando — clique abre

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

# As marcas de dispositivo da modeline, na linha sob o ponteiro.
hint-bluetooth-device = { $kind }: { $name } — clique abre o bluetooth
hint-bluetooth-device-battery = { $kind }: { $name }, { $percent }% restantes — clique abre o bluetooth
keys-region-taken = outro teclado está com o rodapé aberto; este espera até ele fechar

## Telas — os monitores, a imagem deles e a luz (LEADER s d)

display-no-light-here = O brilho de nenhuma tela pode ser mudado daqui
state-keyboard-editing = um teclado
cmd-keyboard-open = Teclados: o que cada tecla faz
# ---------------------------------------------------------------------------
# LEADER g — the glance: what time it is somewhere, and the weather and money.
# ---------------------------------------------------------------------------
world-place-europe-london = Londres
world-place-america-sao-paulo = Brasília
world-temperature = { $degrees } °C
world-rate = 1 { $from } = { $much } { $to }
cmd-world-glance = Um relance: a hora, o tempo e o câmbio
world-temperature-f = { $degrees } °F
audio-media-none = Este build não tem tocador de mídia para conversar.
cmd-volume-up = Aumentar o volume
cmd-volume-down = Baixar o volume
cmd-volume-mute = Silenciar a máquina
cmd-mic-mute = Silenciar o microfone
cmd-media-stop = Parar de tocar
cmd-brightness-up = Tela mais clara
cmd-brightness-down = Tela mais escura
cmd-media-play = Tocar, ou pausar
cmd-media-next = Próxima faixa
cmd-media-previous = Faixa anterior
hint-volume = { $device } a { $percent }% — som
hint-volume-muted = { $device }, mudo — som
hint-microphone = Algo está ouvindo — som
sound-muted = mudo
# =====================================================================
# LEADER d — development: tools for whoever builds this distro.
# =====================================================================
# The screen resizer (`LEADER / r`, `resizer.rs`): two curtains cut the
# screen, and everything is laid out in the room they leave. Its line on the
# bottom curtain says the room in pixels and in the grid, and each key's word
# is short because the line is one line; `about-` is what the pointer reads.
group-dev = desenvolvimento
hint-resizer = A tela está cortada em { $width } × { $height } px — { $columns } colunas por { $lines } linhas. Clique para tê-la inteira de novo.
echo-resizer-on = A tela pode ser redimensionada: arraste a borda da direita, a de baixo ou o canto, ou clique numa cortina e use as teclas que a linha dela ensina.
echo-resizer-off = A tela está inteira de novo.
echo-resizer-failed = Não deu para redimensionar a tela: { $why }
hint-camera-in-use = Uma câmera está aberta por { $programs }. Clique para os ajustes da câmera.
hint-gestures = Os gestos com as mãos estão ligados, e a câmera lê as suas mãos. Clique para desligá-los.
compositor-mode-split = dividir
compositor-mode-jump = pular
compositor-mode-filter = filtro
compositor-mode-placement = posição
compositor-mode-width = largura
compositor-mode-height = altura
compositor-mode-resize = redimensionar
compositor-mode-floating = flutuante
compositor-mode-toggle-width = alternar largura
compositor-mode-toggle-height = alternar altura
compositor-mode-align = alinhar
compositor-mode-fit = ajustar
compositor-mode-trailmark = marcar
compositor-mode-trail = trilha
compositor-mode-spaces = arranjos
hint-compositor-mode = O compositor está num modo de teclas: as teclas querem dizer o que a linha abaixo diz. Clique para sair dele.
hint-recording = O estúdio está gravando ou transmitindo. Clique para os ajustes da câmera.
hint-workspace = Espaço { $number } — clique leva até ele, o botão direito mostra todos os espaços de uma vez
hint-workspace-current = Espaço { $number }, o da frente — clique encaixa ele inteiro na tela, ou desfaz; o botão direito mostra todos os espaços de uma vez
overview-back = volta à visão normal
overview-mouse = um clique numa janela entra nela
cmd-onscreen-keyboard = Teclado na tela
cmd-gestures = Gestos com as mãos
cmd-gestures-calibrate = Calibrar os gestos com as mãos
osk-key-esc = Esc
osk-key-ctrl = Ctrl
osk-key-alt = Alt
osk-key-altgr = AltGr
osk-key-super = Super
osk-key-fn = Fn
osk-key-home = Home
osk-key-end = End
osk-key-page-up = PgUp
osk-key-page-down = PgDn
osk-key-insert = Ins
osk-key-delete = Del
osk-key-function = F{ $number }
osk-key-leader = Leader
osk-setting-languages = Idiomas
osk-setting-height = Altura das teclas
osk-setting-width = Largura
osk-width-small = Pequeno
osk-width-medium = Médio
osk-width-whole = Tela inteira
about-osk-language = Passa para o próximo idioma: { $languages }
about-osk-settings = Os ajustes do próprio teclado; aperte de novo para voltar às teclas
about-osk-hide = Guarda o teclado
about-osk-leader = A leader, como a tecla dela a abriria
about-osk-fn = Teclas de função, Insert e Delete, e Home, End e as páginas nas setas
about-osk-modifier = Um toque: vale para a próxima tecla. Dois toques: fica ligada até outro toque
about-osk-choice = { $name } — toque para oferecer este idioma no teclado, ou para deixar de oferecer
about-osk-height = A altura de cada tecla
about-osk-width = A largura do teclado
echo-osk-last-language = Um idioma sempre fica ligado.
echo-osk-types-nothing = O compositor não oferece teclado virtual, então estas teclas não chegam a janela nenhuma.
echo-gestures-on = Gestos ligados: a câmera lê as suas mãos.
echo-gestures-off = Gestos desligados, e a câmera foi liberada.
echo-gestures-move-nothing = Gestos ligados, mas o compositor não oferece ponteiro virtual, então o ponteiro não vai se mover.
echo-gestures-calibrate-top-left = Aponte para o canto superior esquerdo da tela e abra a outra mão.
echo-gestures-calibrate-top-right = Agora o canto superior direito, e abra a outra mão.
echo-gestures-calibrate-bottom-right = Agora o canto inferior direito, e abra a outra mão.
echo-gestures-calibrate-bottom-left = Por último, o canto inferior esquerdo, e abra a outra mão.
echo-gestures-calibrated = Calibrado: o ponteiro agora segue os cantos que você mostrou.
echo-gestures-calibration-failed = Os cantos ficaram perto demais; o ponteiro mantém a área que tinha.
echo-gesture-escape = Gesto: Esc.
echo-gesture-arrow = Gesto: uma seta.
echo-gesture-focus = Gesto: o foco mudou.
echo-gesture-workspace = Gesto: o espaço de trabalho mudou.
echo-gesture-overview = Gesto: todos os espaços de trabalho.
echo-gesture-keyboard = Gesto: o teclado na tela.
echo-gesture-player = Gesto: a coluna do player.
echo-gesture-close = Gesto: a janela foi fechada.
osk-height-low = Baixa
osk-height-medium = Média
osk-height-tall = Alta

## O menu da janela — o nome do app na modeline, clicado (LEADER w o)

cmd-window-menu = Tudo o que dá para fazer com a janela em foco, numa grade de controles
window-menu-none = Nenhuma janela está em foco.
window-menu-fullscreen = Tela cheia
window-menu-unfullscreen = Sair da tela cheia
window-menu-float = Flutuar
window-menu-tile = Voltar ao layout
window-menu-stick = Fixar em todo espaço
window-menu-unstick = Soltar dos espaços
window-menu-maximise = Maximizar
window-menu-centre = Centralizar
window-menu-put-away = Guardar na gaveta
window-menu-resize = Redimensionar pelas teclas
window-menu-send-to = enviar ao espaço
window-menu-move = mover
window-menu-size = tamanho
window-menu-fullscreen-kinds = tipos de tela cheia
window-menu-close = Fechar
window-menu-close-others = Fechar as outras
window-menu-kill = Forçar o fechamento
about-window-menu-kill = Mata o programa da janela na hora. O que ele não tinha salvado se perde.
window-menu-workspace = Espaço { $n }
window-menu-workspace-empty = Espaço { $n }, vazio
window-menu-next = O próximo
window-menu-previous = O anterior
window-menu-left = Esquerda
window-menu-right = Direita
window-menu-up = Acima
window-menu-down = Abaixo
window-menu-out-left = Para fora da coluna, à esquerda
window-menu-out-right = Para fora da coluna, à direita
window-menu-out-up = Para fora da coluna, acima
window-menu-out-down = Para fora da coluna, abaixo
window-menu-beginning = Para o começo
window-menu-end = Para o fim
window-menu-pin-beginning = Prender no começo
window-menu-pin-end = Prender no fim
window-menu-screen-left = Para a tela da esquerda
window-menu-screen-right = Para a tela da direita
window-menu-width-next = Próxima largura
window-menu-width-previous = Largura anterior
window-menu-height-next = Próxima altura
window-menu-height-previous = Altura anterior
window-menu-toggle-size = Tamanho cheio, ou de volta
window-menu-reset-size = Voltar o tamanho
window-menu-full-layout = Ocupar a área acima do rodapé, ou sair dela
window-menu-full-application = Tela cheia só para o programa, ou sair
window-menu-full-global = Cobrir todas as telas, ou sair
window-menu-align = alinhar
window-menu-align-auto = Deixar o layout posicionar
window-menu-align-bottom = Embaixo
window-menu-align-left = Borda esquerda
window-menu-align-middle = Meio da coluna
window-menu-align-right = Borda direita
window-menu-align-top = Em cima
window-menu-awake-focus = Acordada com foco
window-menu-awake-fullscreen = Acordada em tela cheia
window-menu-awake-none = Deixar a tela dormir
window-menu-awake-open = Acordada enquanto aberta
window-menu-awake-visible = Acordada enquanto visível
window-menu-behaviour = comportamento
window-menu-border-csd = Decorações próprias
window-menu-border-none = Sem borda
window-menu-border-normal = Borda e título
window-menu-border-pixel = Borda fina
window-menu-calm = Parar de pedir atenção
window-menu-content = zoom do conteúdo
window-menu-content-bigger = Maior
window-menu-content-reset = Tamanho próprio
window-menu-content-smaller = Menor
window-menu-dim-off = Nunca escurecer
window-menu-dim-on = Escurecer fora de foco
window-menu-float-keys = Mover pelas teclas
window-menu-grow-down = Empurrar a base
window-menu-grow-left = Empurrar a esquerda
window-menu-grow-right = Empurrar a direita
window-menu-grow-up = Empurrar o topo
window-menu-height = altura
window-menu-keep-shortcuts = Deixar tomar os atalhos
window-menu-look = aparência
window-menu-narrower = Mais estreita
window-menu-nudge-down = Deslocar para baixo
window-menu-nudge-left = Deslocar à esquerda
window-menu-nudge-right = Deslocar à direita
window-menu-nudge-up = Deslocar para cima
window-menu-opacity-full = Totalmente opaca
window-menu-opacity-less = Mais transparente
window-menu-opacity-more = Mais opaca
window-menu-out-beginning = Para fora, no começo
window-menu-out-end = Para fora, no fim
window-menu-select = Selecionar
window-menu-shadow-off = Sem sombra
window-menu-shadow-on = Sombra
window-menu-share = { $percent }%
window-menu-shorter = Mais baixa
window-menu-shrink-down = Recolher a base
window-menu-shrink-left = Recolher a esquerda
window-menu-shrink-right = Recolher a direita
window-menu-shrink-up = Recolher o topo
window-menu-take-to = levar ao espaço
window-menu-taller = Mais alta
window-menu-tearing-off = Sem tearing
window-menu-tearing-on = Permitir tearing
window-menu-wider = Mais larga
window-menu-width = largura
cmd-wm-float-off = Pôr esta janela de volta no layout — e fora da gaveta, se veio dela
cmd-drawer-open = O que a gaveta guarda, para tirar uma janela dela
window-menu-drawer = gaveta
window-menu-put-back = Guardar de volta na gaveta
window-menu-take-out = Tirar da gaveta de vez
window-menu-drawer-one = { $app } — { $title }
window-menu-drawer-next = A próxima para fora
window-menu-drawer-jump = Escolher pelo número
about-window-menu-take-out-this = Tira esta janela da gaveta, sobre o espaço da frente.
hint-drawer = { $count ->
        [one] Uma janela na gaveta. Clique para vê-la; o botão direito a tira.
       *[other] { $count } janelas na gaveta. Clique para vê-las; o botão direito tira a próxima.
    }
window-menu-drawer-empty = A gaveta está vazia.
cmd-trail-open = As trilhas: esta janela numa delas, andar por elas, criar e apagar
cmd-selection-open = As janelas selecionadas, e o que dá para fazer com todas juntas
window-menu-selection = seleção
window-menu-trail = trilha
window-menu-trail-on = Pôr na trilha
window-menu-trail-off = Tirar da trilha
window-menu-trail-next-window = Próxima janela dela
window-menu-trail-previous-window = Janela anterior dela
window-menu-trail-to-selection = Selecionar as janelas dela
window-menu-trail-new = Nova trilha
window-menu-trail-number = Trilha { $n }
window-menu-trail-in-use = Trilha { $n }, em uso
window-menu-trail-next = Próxima trilha
window-menu-trail-previous = Trilha anterior
window-menu-trail-clear = Esvaziar
window-menu-trail-delete = Apagar
window-menu-yes-delete-trail = Sim, apagar esta trilha
window-menu-unselect = Desmarcar
window-menu-select-workspace = Selecionar o espaço inteiro
window-menu-select-move = Trazer para cá
window-menu-select-to-trail = Virar uma trilha
window-menu-select-none = Limpar a seleção
hint-selection-uncounted = Janelas selecionadas, mais do que este rodapé conseguiu contar. Clique para ver o que fazer com elas.
hint-trail-current = Trilha { $number }, a que está em uso.
hint-trail-number = Trilha { $number }. Clique para usá-la.
hint-selection = { $count ->
        [one] Uma janela selecionada. Clique para ver o que fazer com ela.
       *[other] { $count } janelas selecionadas. Clique para ver o que fazer com elas.
    }
hint-trail = { $trails ->
        [one] Uma trilha
       *[other] { $trails } trilhas
    }, { $windows ->
        [0] nenhuma janela na que está em uso
        [one] uma janela na que está em uso
       *[other] { $windows } janelas na que está em uso
    }. Clique para ver as trilhas.
cmd-wm-float-centre = Levar esta janela flutuante ao meio da área das janelas
window-menu-drawer-out = { $name } — fora
about-window-menu-put-back-this = Guarda de volta na gaveta esta janela que saiu dela.
notif-trail-drawer-summary = { $window } saiu da trilha
notif-trail-drawer-body = A janela { $id } ({ $window }) foi para a gaveta enquanto estava na trilha em uso, então foi tirada da trilha. Andar por uma trilha até uma janela da gaveta derruba o compositor — um defeito do Scroll, não deste sistema — e isto impede que aconteça.
window-menu-no = Não
window-menu-yes-kill = Sim, forçar o fechamento
window-menu-yes-close-others = Sim, fechar todas as outras janelas
hint-window-menu = Clique para abrir as opções de tudo o que dá para fazer com esta janela
software-nothing-to-open = Nada foi instalado desde que o rodapé começou.
notif-software-app = programas
hint-software-trial = { $count ->
        [one] Um programa está em experiência, e nada dele está instalado. Clique para vê-lo.
       *[other] { $count } programas estão em experiência, e nada deles está instalado. Clique para vê-los.
    }
hint-software-android = O Android está rodando, e custa memória enquanto roda. Clique para o Android.
notif-software-trial-ended-summary = { $app } fechou
notif-software-trial-ended-body = Nada dele foi instalado; sai do disco na próxima limpeza.
notif-action-software-launch-latest = Abrir
# =====================================================================
# O CHROME QUIOSQUE — `LEADER m b`: o Chrome da própria pessoa sem as
# barras dele, a modeline que se adapta a ele (`adapt.rs`, `kiosk.rs`), a
# vista (`kiosk_view.rs`) e o grupo `,`. Tecla nunca é texto aqui.
# =====================================================================
cmd-kiosk-open = Abrir o seu Chrome como quiosque, sem as barras dele
cmd-kiosk-back = Voltar, na aba do quiosque
cmd-kiosk-forward = Avançar, na aba do quiosque
cmd-kiosk-reload = Recarregar a aba do quiosque
cmd-kiosk-tab = Trazer esta aba do quiosque para a frente
cmd-kiosk-tab-next = Próxima aba do quiosque
cmd-kiosk-tab-previous = Aba anterior do quiosque
cmd-kiosk-tab-close = Fechar a aba do quiosque
cmd-kiosk-tab-left = Mover a aba do quiosque uma casa para a esquerda
cmd-kiosk-tab-right = Mover a aba do quiosque uma casa para a direita
cmd-kiosk-tab-move = Pôr uma aba do quiosque em outro lugar
cmd-kiosk-new-tab = Abrir um endereço numa aba nova do quiosque
cmd-kiosk-address = Editar o endereço da aba do quiosque
cmd-kiosk-zoom = O zoom do quiosque, em passos
cmd-kiosk-zoom-in = Aumentar o zoom da página do quiosque
cmd-kiosk-zoom-out = Diminuir o zoom da página do quiosque
cmd-kiosk-zoom-reset = Voltar o zoom do quiosque para 100%
cmd-kiosk-chrome-ui = Mostrar ou esconder as barras do próprio Chrome no quiosque
cmd-kiosk-address-line = Mostrar ou esconder a linha de endereço acima da modeline
cmd-kiosk-more = Tudo o que o quiosque faz
cmd-kiosk-tabs = Todas as abas do quiosque
cmd-kiosk-extensions = As extensões do quiosque
cmd-kiosk-extension = Abrir a página de uma extensão numa aba nova
cmd-kiosk-extension-options = Abrir as opções de uma extensão
cmd-kiosk-extension-details = Os detalhes de uma extensão no Chrome
cmd-kiosk-release = Fazer do quiosque uma janela comum do Chrome de novo
cmd-modeline-adapt-kiosk = Adaptar a modeline ao quiosque, ou não
cmd-footer-hide-push = Esconder a modeline e o minibuffer, empurrando as janelas quando algo abre; de novo, mostrá-los
cmd-footer-hide-cover = Esconder a modeline e o minibuffer, pondo o que abre por cima das janelas; de novo, mostrá-los
echo-footer-opaque = Esta tela não desenha o rodapé transparente, então as barras ocultas deixam uma faixa opaca.
cmd-kiosk-list-down = Próxima linha da lista do quiosque
cmd-kiosk-list-up = Linha anterior da lista do quiosque
cmd-kiosk-list-enter = A linha da lista do quiosque: a aba para a frente, ou a página da extensão
cmd-kiosk-tab-earlier = Mover a aba sob o cursor uma casa para trás
cmd-kiosk-tab-later = Mover a aba sob o cursor uma casa para a frente
cmd-wm-full-application-on = Dizer à janela que ela está em tela cheia, no próprio lugar
cmd-wm-full-application-off = Dizer à janela que ela não está mais em tela cheia
kiosk-list-down = baixo
kiosk-list-up = cima
kiosk-list-enter = abrir
kiosk-tab-earlier = para trás
kiosk-tab-later = para a frente
kiosk-tab-close = fechar
kiosk-new-tab = aba nova
kiosk-extension-options = opções
kiosk-extension-details = detalhes
ask-kiosk-address = O endereço, ou palavras para buscar
ask-kiosk-new-tab = Endereço ou palavras para a aba nova (vazio abre uma em branco)
kiosk-tabs-more = +{ $count }
kiosk-zoom-percent = { $percent }%
kiosk-tab-front = { $host }, na frente
kiosk-menu-back = Voltar
kiosk-menu-forward = Avançar
kiosk-menu-reload = Recarregar
kiosk-menu-new-tab = Aba nova
kiosk-menu-address = Editar o endereço
kiosk-menu-tabs = Abas
kiosk-menu-tab-previous = Aba anterior
kiosk-menu-tab-next = Próxima aba
kiosk-menu-tab-left = Aba para a esquerda
kiosk-menu-tab-right = Aba para a direita
kiosk-menu-tab-close = Fechar a aba
kiosk-menu-extensions = Extensões
kiosk-menu-zoom = Zoom
kiosk-menu-zoom-in = Maior
kiosk-menu-zoom-out = Menor
kiosk-menu-zoom-reset = Tamanho real
kiosk-menu-show-chrome = Mostrar as barras do Chrome
kiosk-menu-hide-chrome = Esconder as barras do Chrome
kiosk-menu-address-show = Mostrar a linha de endereço
kiosk-menu-address-hide = Esconder a linha de endereço
kiosk-menu-release = Janela comum
kiosk-heading-pinned = Fixadas
kiosk-heading-others = As outras
kiosk-heading-disabled = Desligadas
kiosk-no-extensions = Este Chrome não tem nenhuma extensão além da nossa.
window-menu-kiosk = Quiosque
hint-kiosk-back = Uma página para trás nesta aba
hint-kiosk-forward = Uma página para a frente nesta aba
hint-kiosk-reload = Carregar esta página de novo
hint-kiosk-tabs-before = { $count ->
        [one] Mais uma aba à esquerda
       *[other] Mais { $count } abas à esquerda
    } — o clique lista todas
hint-kiosk-tabs-after = { $count ->
        [one] Mais uma aba à direita
       *[other] Mais { $count } abas à direita
    } — o clique lista todas
hint-kiosk-tab = { $title } — o clique traz para a frente, arrastar muda de lugar, o botão do meio fecha, o direito lista todas as abas
hint-kiosk-tab-current = { $title } — a aba da frente; o clique lista todas as abas, o botão do meio fecha
hint-kiosk-new-tab = Uma aba nova, num endereço ou numa busca
hint-kiosk-ext = { $name } — o clique abre a página dela numa aba nova, o botão direito as opções
hint-kiosk-extensions = { $names } — o clique lista todas as extensões
hint-kiosk-extensions-none = Nenhuma extensão além da nossa — o clique lista
hint-kiosk-zoom = A página a { $percent }% — a roda muda, o clique abre os passos, o botão direito volta a 100%
hint-kiosk-address-line-on = A linha de endereço está à mostra — o clique esconde
hint-kiosk-address-line-off = A linha de endereço está escondida — o clique mostra acima da modeline
hint-kiosk-chrome-hidden = As barras do Chrome estão escondidas — o clique mostra (modo normal)
hint-kiosk-chrome-shown = As barras do Chrome estão à mostra — o clique esconde de novo
hint-kiosk-more = Tudo o que o quiosque faz
hint-kiosk-offline-bridge = A ponte com o navegador não está instalada nesta máquina: o quiosque só mostra e esconde as barras do Chrome
hint-kiosk-offline-chrome = O Chrome não se conectou — a extensão ronoctua pode estar desligada
hint-kiosk-offline-version = A extensão é de outra versão que o rodapé: reinicie o Chrome
hint-kiosk-address = { $url } — o clique edita o endereço
echo-kiosk-offline = O Chrome não está conectado, então o quiosque ainda não faz isso.
echo-kiosk-unmatched = A janela do quiosque não respondeu; ela abriu como uma janela comum do Chrome.
echo-kiosk-no-history = Nada para trás nesta aba.
echo-kiosk-no-future = Nada para a frente nesta aba.
echo-kiosk-not-a-kiosk = A janela em foco não é um quiosque.
echo-kiosk-address-refused = Esse endereço não abre aqui: só http e https.
echo-kiosk-refused = O Chrome recusou: { $why }
echo-kiosk-no-chrome = O Chrome está desligado neste sistema.
echo-kiosk-no-bridge = A ponte com o navegador não está instalada, então um quiosque não pode ser pareado.
echo-kiosk-no-page = Essa extensão não tem página própria; os detalhes dela abriram no lugar.
echo-kiosk-no-extension = Nenhuma extensão com esse id está neste Chrome.
echo-kiosk-no-nonce = Não foi possível ler um número aleatório para parear um quiosque.
echo-kiosk-no-tab = O quiosque não tem essa aba.
echo-modeline-will-not-parse = O modeline.json não é lido, e gravar substituiria o que há nele.
# THE BRAIN'S MODULES — what a notice answers to, on the strip under the
# minibuffer and on the brain's page; the page's notices, a watch, and the
# activity log folded.
cmd-brain-act = Fazer o que ele oferece
cmd-brain-dismiss = Dispensar
cmd-brain-snooze = Adiar
brain-band-dismiss = dispensar
brain-band-snooze = { $minutes } min
brain-band-more = +{ $count }
hint-brain-more = todos os avisos, na página do Brain
echo-brain-no-notice = O cérebro não tem nada a dizer agora
cmd-leader-set-default = As teclas como são, cada uma abrindo o seu app no rodapé
cmd-footer-bars-cycle = O próximo dos três modos das barras: à mostra, escondidas empurrando as janelas, escondidas por cima delas
cmd-leader-close = Fechar a lista da leader, como o Esc dela faz
echo-bars-shown = Modeline e minibuffer à mostra
echo-bars-push = Modeline e minibuffer escondidos; o que abre empurra as janelas
echo-bars-cover = Modeline e minibuffer escondidos; o que abre fica por cima das janelas
cmd-wm-fold-keys-hold = Deixar Ctrl com as setas andar pela linha do minibuffer enquanto uma dica longa demais para ela está nela
cmd-wm-fold-keys-free = Devolver Ctrl com as setas às janelas
# THE MAXIMUM ALERT BAR (alert.rs). The title is drawn in capitals.
alert-battery-low = Bateria fraca!!!
alert-battery-low-body = Restam { $percent }%, na bateria, e o carregador não está ligado.
hint-alert-close = Fecha este alerta; ele volta se a bateria cair mais um degrau
cmd-alert-dismiss = Fechar a barra de alerta; ela volta se o que a causou piorar
# THE FOOTER'S SETTINGS (LEADER s r, footer_settings.rs).
group-footer-settings = rodapé
footer-settings-title = Rodapé
cmd-footer-settings-open = Os ajustes do rodapé: as barras, o minibuffer livre e o quiosque
cmd-footer-settings-window = Os ajustes do rodapé, numa janela
cmd-footer-settings-up = Linha anterior
cmd-footer-settings-down = Próxima linha
cmd-footer-settings-enter = Mudar o ajuste sob o cursor
footer-settings-up = cima
footer-settings-down = baixo
footer-settings-enter = mudar
cmd-footer-hours = Mostrar as horas no minibuffer livre, ou parar de mostrar
footer-settings-bars = As barras
footer-settings-bars-mode = Modeline e minibuffer
footer-settings-bars-shown = à mostra
footer-settings-bars-push = escondidos, empurrando
footer-settings-bars-cover = escondidos, por cima
footer-settings-bars-note = Escondidas, as barras voltam na borda de baixo da tela; empurrando, o que abre afasta as janelas, e por cima, fica sobre elas. Um toque de { $keys } na raiz da leader passa ao próximo dos três.
footer-settings-free = O minibuffer livre
footer-settings-hours = Horas
footer-settings-hours-note = A hora, à direita do minibuffer, enquanto nada mais é dito nele.
footer-settings-kiosk = Quiosque
footer-settings-kiosk-adapt = Adaptar a modeline a um quiosque
footer-settings-on = ligado
footer-settings-off = desligado

## file info editing — everything the info panel can change

# Formatting with encryption (the format buffer, and its host).
# The format buffer's third question, as the modeline names its state.
state-encrypt = cifra
hint-security-until = O administrador não pede senha até { $time }; um clique abre onde isso se encerra
fast-access-unopened = Nada abre { $what }.
echo-capture-outside-the-room = Nada foi capturado: o retângulo ficou inteiro sob as cortinas do redimensionador, fora do espaço que elas deixam.
# O estado do gerenciador de arquivos na modeline: uma pergunta sobre quem
# pode mexer num arquivo, em alerta.
state-permission = permissão
hint-security-no-limit = O administrador não pede senha até você encerrar; um clique abre onde isso se encerra

## perguntado, acima da modeline — quem pede, para quê, e o que o sudo espera.

sudo-bar-title = Administrador
sudo-bar-password = A sua senha
sudo-bar-password-instead = A sua senha, no lugar
sudo-bar-touch = Toque na sua chave de segurança, ou digite a sua senha
sudo-bar-finger = Ponha o dedo no leitor, ou digite a sua senha
sudo-bar-wrong = Essa não era a sua senha. { $asked }
sudo-bar-checking = Conferindo…
sudo-bar-cancelling = Cancelando…
sudo-bar-answer = responder
sudo-bar-use-password = usar a senha
sudo-bar-cancel = cancelar
about-sudo-bar-answer = Entrega ao sudo o que você digitou, só para este gesto
about-sudo-bar-use-password = Para de esperar a chave ou o dedo, e pede a sua senha
about-sudo-bar-cancel = Não faz nada, e a tela que pediu diz isso
