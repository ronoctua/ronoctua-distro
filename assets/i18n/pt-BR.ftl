# Interface texts, pt-BR. See the header of `en-GB.ftl`.

## Commands — the `M-x` list and the which-key labels

cmd-buffer-next = Próximo buffer
cmd-buffer-switch = Trocar de buffer
cmd-file-save = Salvar arquivo
cmd-buffer-close = Fechar buffer
cmd-mx-open = Executar comando…
cmd-buffer-terminal = Abrir terminal
cmd-buffer-terminal-external = Abrir terminal externo
cmd-monitor-btop = Monitor de processos (btop)
cmd-monitor-bottom = Monitor de processos (bottom)
cmd-session-quit = Encerrar sessão

## which-key prefix groups

group-quit = sair

## The modeline's two left segments

# THE BUFFER'S NAME (the window title) and its MAJOR MODE (the kind of buffer).
# Both are addressed by an id that is ENGLISH and kebab-case (invariant 18); what
# reaches the screen is the value here. That separation is why `glifos` used to
# leak onto an English-language screen: the id WAS the label.
buffer-example = exemplo
# THE NATIVE WINDOWS, which are not buffers and reach the same place: the
# modeline turns any `br.ronoctua.<id>` into `mode-<id>`, and it does not care
# whether the window is a wasm component or a process of ours.
mode-files = arquivos
mode-monitor = monitor
mode-notifications = notificações
mode-power = energia
mode-bluetooth = bluetooth
mode-display = telas
mode-network = rede
mode-history = histórico do shell
mode-rescue = resgate
mode-terminal = terminal
mode-capture = captura de tela

mode-example = exemplo
buffer-memory = memória
mode-memory = memória
buffer-transfers = transferências
mode-transfers = transferências
buffer-format = formatar
mode-format = formatar
buffer-brain = cérebro
mode-brain = cérebro
mode-update = atualização
buffer-no-name = *sem nome*
mode-unknown = desconhecido

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

notif-title = Notificações

# A faixa, acima da modeline. O contador só aparece com mais de uma.
notif-queue-position = { $shown }/{ $total }

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
notif-action-open-video = Abrir a gravação
notif-memory-killed-summary = Fechado para liberar memória
notif-memory-killed-body = Um aplicativo foi fechado para a máquina não travar.

# O que cada notificação deixa fazer.
notif-action-edit = Editar a imagem
notif-action-open = Abrir a imagem
notif-action-folder = Abrir a pasta
notif-action-copy-image = Copiar a imagem
notif-action-copy-path = Copiar o caminho
notif-action-memory-panel = Abrir o painel de memória

# OS FILTROS QUE A DISTRO JÁ TRAZ. O gerenciador os lista por estes nomes.
notif-rule-important-gets-room = O que é importante ganha espaço
notif-rule-bluetooth-connected-is-quiet = Aparelho Bluetooth que conecta é dito pela marca
notif-rule-bluetooth-disconnected-is-quiet = Aparelho Bluetooth que sai é dito pela marca
notif-rule-small-things-are-quick = Coisa pequena passa rápido

# O gerenciador: três listas, e um título para cada.
notif-column-happened = Recebidas
notif-column-suppressed = Filtradas
notif-column-catalogue = Notificações e filtros
# A LINHA DE DICAS lê cada id das tabelas de teclas do gerenciador como um texto
# curto — a convenção do gerenciador de arquivos; `cmd-<id>` é o rótulo longo do manual.
notif-read = lida ou não lida
notif-forget = esquecer
notif-read-all = todas lidas
notif-filters = filtros
notif-any-device = Um dispositivo

# A FICHA DO FILTRO — escrever um seu. A linguagem de regras está completa desde
# o dia em que foi gerada; até agora o único caminho até ela era editar o JSON à
# mão, o que não é um caminho.
notif-draft-name = como chamar
notif-draft-field = olhar para
notif-draft-test = comparar
notif-draft-value = com
notif-draft-unnamed = ainda sem nome
notif-draft-nothing = ainda nada
notif-draft-unready = falta um nome e algo com que comparar
notif-draft-type = digitar esta
notif-draft-add = mais uma condição
notif-draft-drop = tirar esta condição
notif-draft-save = salvar o filtro
notif-new = escrever um filtro
notif-edit = editar este filtro seu
notif-drop = apagar este filtro seu

# O que uma condição olha, e como compara. As palavras são da pessoa, não da
# tabela: `summary` é como o protocolo chama, e `o título` é o que é.
notif-field-app = quem mandou
notif-field-summary = o título
notif-field-body = o corpo
notif-field-tag = uma etiqueta
notif-field-catalogue = o tipo
notif-field-urgency = quão urgente
notif-field-level = o nível
notif-test-is = é exatamente
notif-test-contains = contém
notif-test-starts-with = começa com
notif-test-at-least = é pelo menos
notif-test-at-most = é no máximo
notif-act = rodar a ação
notif-adjust = ajustar este tipo
notif-where = { $at } de { $total }
notif-up = sobe
notif-down = desce
notif-pane-next = próxima coluna
notif-pane-previous = coluna anterior
notif-knob-next = próximo valor
notif-knob-previous = valor anterior
notif-restore = o padrão da distro
notif-default = padrão da distro
notif-mine = seu filtro
notif-shared = a caminho de todo mundo

# ANTES DE UM FILTRO IR PARA TODO MUNDO. O repositório é público, e o que cai
# nele está publicado: o git guarda e um espelho copia.
notif-share = compartilhar com todo mundo
notif-share-answer = responder
notif-share-does = e ele faz
notif-share-public = Isto vai para o repositório, que é PÚBLICO. Tudo o que está acima é publicado junto, e não há como voltar atrás na prática. Nada mais seu vai — nem o histórico, nem os ajustes, nem o nome desta máquina. Quem publica de verdade é o seu commit.
notif-share-not-a-name = Nome no repositório é inglês, minúsculo, com hífen no lugar do espaço (invariante 18) — é a chave sob a qual um ajuste é guardado. Renomeie o filtro e volte.
notif-share-no = não, fica só comigo
notif-share-yes = sim, compartilhar
notif-shared-written = Está no repositório. Quem publica é o seu commit.
notif-share-failed = Não deu para escrever no repositório.
notif-share-nowhere = Esta máquina não tem repositório para onde compartilhar.
notif-yours = seu ajuste
notif-nothing-here = nada aqui
notif-yes = sim
notif-no = não
notif-knob-show = aparece
notif-knob-life = fica

# Como os campos da apresentação aparecem na tela.
notif-show-hidden = não mostrar
notif-show-brief = uma linha
notif-show-full = até três linhas
notif-life-quick = rápida
notif-life-normal = normal
notif-life-long = longa
notif-loud = chamativa
notif-level = nível
notif-blink = piscar
notif-vibrate = vibrar
notif-audio = tocar som
notif-auto-read = chega já lida

# O QUE O GERENCIADOR SABE SOBRE UM REGISTRO, na coluna ao lado dele. É o que a
# linha não tem espaço para dizer: de onde veio, quando, quanto tempo ficou e — o
# que o dono pediu pelo nome — QUAL FILTRO DECIDIU, que antes vinha colado no fim
# da frase como um id cru.
notif-fact-origin = quem mandou
notif-fact-arrived = quando chegou
notif-about-record = { $when } — { $line }
notif-fact-life = quanto tempo ficou
notif-fact-show = como apareceu
notif-fact-read = já foi lida
notif-fact-filter = filtro que decidiu
notif-fact-no-filter = nenhum casou
notif-see = abrir

# OS DOIS INTERRUPTORES QUE NÃO FAZEM NADA, e por quê. Um interruptor que não
# interrompe e não diz nada é a armadilha que este projeto já pagou uma vez.
notif-inert-vibrate = sem dispositivo háptico nesta máquina
notif-inert-audio = ainda não implementado — veja docs/PENDING.md

# O que o ponteiro explica no SINO, na modeline — não na faixa.
# O ZERO EXATO VEM PRIMEIRO. O Fluent casa um número exato antes da categoria
# de plural, e a categoria `one` do português cobre o 0 também — sem o ramo
# `[0]`, "0 filtradas" saía como "uma filtrada".
hint-notif-bell = { $unread ->
        [0] nenhuma não lida
        [one] uma não lida
       *[other] { $unread } não lidas
    }, { $filtered ->
        [0] nenhuma filtrada
        [one] uma filtrada
       *[other] { $filtered } filtradas
    } — clique abre o gerenciador
hint-notif-read = marcar como lida e tirar da tela
hint-notif-open = tirar da faixa e abrir inteira no gerenciador
notif-see-whole = ver inteira
notif-glyph-read = Marcar como lida
notif-glyph-unread = Marcar como não lida de novo
notif-glyph-delete = Esquecer de vez
hint-notif-countdown = quanto tempo ela ainda tem
hint-notif-paused = o tempo está parado enquanto o ponteiro está nela
hint-notif-action = { $action } — clique, ou abra a notificação

## Installer — the six questions (section 7.1)

inst-title = Instalar o ronoctua
inst-disk = Disco de destino
inst-disk-help = Será apagado por inteiro, sem recuperação.
inst-hostname = Nome da máquina
inst-hostname-help = Começa com `noctua-`. Minúsculas, dígitos e hífen.
inst-password = Senha do usuário
inst-password-help = Para entrar no sistema depois de instalado.
inst-passphrase = Frase-secreta do disco
inst-passphrase-help = Destrava o disco quando o TPM se recusar. Guarde fora do computador.
inst-language = Idioma
inst-language-help = Da interface e do teclado.
inst-keyboard = Layout do teclado
inst-keyboard-help = Escolha o do seu teclado. E nele que voce vai digitar a frase-secreta.
inst-keyboard-variant = Variante do layout
inst-keyboard-variant-help = Separa teclados do mesmo pais. Sem variante e uma resposta valida.
inst-compose-key = Tecla de acentuacao
inst-compose-key-help = A tecla Compose: aperta e solta, depois duas teclas, e sai o acento. Sem ela, teclado sem acento nao os alcanca. O padrao e o Caps Lock, que os dois Shifts juntos assumem.
inst-system-locale = Formato do sistema
inst-system-locale-help = Data, numero e unidade. Nao e o idioma da interface, que ja foi escolhido.
inst-timezone = Fuso horario
inst-timezone-help = Onde a maquina fica. E o que decide a hora do relogio e do log.
inst-profile = Instalacao
inst-profile-help = `full` traz tudo e e a recomendada. `minimal` deixa de fora Android, compiladores, navegador, editor, midia e as variantes de fonte — instala em minutos, e cada grupo volta com uma linha no host.nix.
inst-resume-offset = Deslocamento do swap
inst-resume-offset-help = Calculado depois que o disco existir; não se digita.

## Installer — the steps

inst-step-key = Guardar a frase-secreta para o disko
inst-step-partition = Particionar e criptografar o disco
inst-step-flake = Copiar o flake para o sistema alvo
inst-step-host-nix = Gravar a descrição desta máquina
inst-step-facter = Detectar o hardware
inst-step-password = Gravar o hash da senha em /persist
inst-step-install = Construir e instalar o sistema
inst-step-zone = Definir o fuso horário desta máquina
inst-step-target = Criar a pasta de configuração do sistema alvo

## Installer — the interface

inst-review = Revisar antes de executar
inst-destructive = Os passos marcados apagam o disco
inst-confirm = Para confirmar, digite o caminho do disco
inst-confirm-error = Não confere com o disco escolhido
inst-to-apply = Para valer, com o disco sendo apagado: --execute
inst-dryrun = Ensaio: nada foi tocado
inst-running = Instalando
inst-done = Instalação concluída. Tire o pendrive e reinicie.
inst-failed = A instalação parou neste passo
inst-no-disk = Nenhum disco encontrado nesta máquina
inst-keys-edit = Enter segue · Esc volta · Ctrl+C sai
inst-keys-choose = ↑↓ escolhem · Enter segue · Esc volta
inst-keys-review = Enter executa · Esc volta · Ctrl+C sai
inst-keys-quit = Ctrl+C sai
echo-no-on = { $sequence } não está ligado a nada
# Same sequence, other case. Shown when the case IS the reason nothing
# matched — Caps Lock or Shift. Without it the screen names the consequence
# and hides the cause.
echo-no-on-box = { $sequence } não está ligado — { $other } está
echo-cancelled = Cancelado
echo-no-window = Nenhuma janela em foco
echo-no-action = { $command } ainda não faz nada
echo-argument-refused = { $command } não aceita “{ $argument }”
echo-no-buffer = Essa tela não foi construída neste sistema
echo-no-opened = Não consegui abrir { $program }
cmd-window-left = Focar à esquerda
cmd-window-right = Focar à direita
cmd-window-up = Focar acima
cmd-window-down = Focar abaixo
cmd-window-move-left = Mover à esquerda
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
echo-capture-failed = A captura não pôde ser feita — o motivo está no log
group-window = janela/buffer
group-terminal = terminal
group-workspace = workspace
cmd-workspace-overview = Panorama dos espaços
cmd-window-float = Alternar flutuante
cmd-workspace-all = Ver todos os espaços de uma vez
cmd-workspace-go = Ir para o espaço
cmd-workspace-move = Levar a janela para o espaço

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-notif-open = Notificações
cmd-notif-window = Notificações em uma janela
cmd-notif-read-all = Marcar todas como lidas
cmd-notif-read = Marcar esta notificação como lida, ou como não lida
cmd-notif-forget = Esquecer esta notificação
cmd-notif-act = Rodar a ação da notificação
cmd-notif-see = Abrir a notificação e ler tudo
cmd-notif-filters = Filtros de notificação
cmd-notif-new = Escrever um filtro de notificação seu
cmd-notif-drop = Apagar este filtro seu
cmd-notif-share = Compartilhar este filtro seu com todo mundo
cmd-notif-share-answer = Responder a pergunta de compartilhar
cmd-notif-draft-type = Digitar o que esta linha do filtro guarda
cmd-notif-draft-add = Acrescentar mais uma condição ao filtro
cmd-notif-draft-drop = Tirar esta condição do filtro
cmd-notif-draft-save = Salvar o filtro
ask-notif-filter-name = O nome do filtro
ask-notif-filter-value = O texto que a condição procura
cmd-notif-adjust = Ajustar este tipo de notificação
cmd-notif-up = Uma notificação acima
cmd-notif-down = Uma notificação abaixo
cmd-notif-pane-next = Próxima coluna do gerenciador
cmd-notif-pane-previous = Coluna anterior do gerenciador
cmd-notif-knob-next = Próximo valor do ajuste
cmd-notif-knob-previous = Valor anterior do ajuste
cmd-notif-restore = De volta ao padrão da distro
cmd-notif-dismiss = Dispensar a notificação
cmd-keyboard-restore = Restaurar o teclado
group-notification = notificação
echo-keyboard-restored = Teclado restaurado
echo-keyboard-failed = Não consegui restaurar o teclado: { $reason }

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
group-icons = ícones
expansion-page = CtrlDir ←→
# THE KEYS THAT WALK THE LINE ONCE IT IS OPEN, and the sentence that teaches
# them. They are a chord of the RIGHT Ctrl, which is the one family no view
# sees — so they are in no registry and in no manual, and this sentence is the
# only thing on screen that says they exist. It is the lesson the file
# manager's `C-o` cost this project — implemented, working, and named nowhere,
# which is the same as not having it; the key is gone now and the lesson is
# why that view's hint line is generated out of its key table. And it is the
# reason this sentence is APPENDED to the hint rather than standing on its own:
# a key taught on a line nobody has opened teaches nothing.
#
# The notation is a value and not a `key_that_runs`, exactly as
# `expansion-page` is: a right-Ctrl chord is not a command of the registry, so
# there is no table to read it back out of, and the two words in it are
# translated.
minibuffer-fold-keys = CtrlDir ↑↓
minibuffer-fold-hint = · { $keys } andam pelo resto desta linha
about-fold-walk = Anda pelo resto desta linha aberta, uma linha por vez; voltar além da primeira a fecha.

# --- Clipboard ---
cmd-clipboard-open = Área de transferência
cmd-clipboard-copy-of-new = Copiar de novo
cmd-clipboard-remove = Remover do histórico
cmd-clipboard-pin = Fixar / desfixar
cmd-clipboard-qrcode = Gerar QR code
cmd-clipboard-clear = Limpar o histórico
clipboard-empty = Nada foi copiado ainda
clipboard-history = Histórico
clipboard-item = Item
clipboard-heading-sync = Sincronização
clipboard-content-text = texto
clipboard-sync-state = sincronizando
clipboard-sync-server = servidor
clipboard-sync-account = conta
clipboard-sync-forget = esquecer esta conta
clipboard-sync-images = imagens também, não só texto
clipboard-sync-notify = avisar quando algo chegar
clipboard-sync-cap = maior cópia que sai daqui
clipboard-sync-cap-server = { $size }, que é o limite do servidor
clipboard-sync-cap-unset = o que o servidor aguentar
clipboard-sync-salt = sal (o mesmo em todas as máquinas)
clipboard-sync-salt-none = nenhum
clipboard-sync-rounds = rodadas com que a chave é derivada
clipboard-setting-on = ligado
clipboard-setting-off = desligado
clipboard-mark = marca na modeline
clipboard-mark-never = nunca
clipboard-mark-when-off = quando não estiver sincronizando
clipboard-mark-when-on = enquanto estiver sincronizando
clipboard-sync-on = sim
clipboard-sync-off = agora não
clipboard-sync-down = não alcança o servidor
clipboard-sync-unset = não definido
clipboard-down = próximo
clipboard-up = anterior
clipboard-copy-of-new = copiar de novo
clipboard-remove = remover
clipboard-pin = fixar
clipboard-qrcode = código QR
clipboard-clear = limpar
clipboard-sync = sincronizar
clipboard-more = mudar
clipboard-less = voltar
cmd-clipboard-more = Gira este ajuste para a frente
cmd-clipboard-less = Gira este ajuste para trás
cmd-clipboard-down = Próxima entrada
cmd-clipboard-up = Entrada anterior
cmd-clipboard-sync = Sincronizar a área de transferência entre máquinas
ask-clipboard-server = O endereço do servidor ClipCascade
ask-clipboard-cap = A maior cópia que sai daqui, em bytes (0 para o limite do servidor)
ask-clipboard-rounds = Com quantas rodadas a chave é derivada
ask-clipboard-salt = O sal, o mesmo em toda máquina desse servidor
ask-clipboard-account = A conta nesse servidor
ask-clipboard-password = A senha dessa conta
echo-clipboard-synced = A área de transferência está sincronizando
echo-clipboard-sync-off = A área de transferência fica nesta máquina
echo-clipboard-sync-failed = Não deu para sincronizar: { $reason }
clipboard-pinned = fixada
clipboard-content = Conteúdo
clipboard-types = Tipos
clipboard-size = Tamanho
echo-copied = Copiado
echo-image-copied = A imagem está na área de transferência
echo-copied-value = Copiado: { $value }
echo-copy-failed = Não consegui copiar — veja o log
echo-qr-large = O conteúdo é grande demais para um QR code
echo-copy-only-text = Por ora só dá para recopiar texto
clipboard-image = Imagem
clipboard-unreadable = Copiado, e ilegível aqui: { $kind }

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

cmd-network-open = Redes
cmd-network-scan = Varrer redes
cmd-network-connect = Conectar à rede
ask-network-connect = Senha desta rede
cmd-network-disconnect = Desconectar
cmd-network-forget = Esquecer a rede
cmd-network-wifi-toggle = Alternar o rádio Wi-Fi
cmd-network-link = Mostrar o enlace
cmd-theme-open = Temas
cmd-theme-apply = Aplicar este tema e fundo
group-network = rede

# THE VIEW. States the list shows in place of rows when there is no row to
# show — each says something different, and folding them into one generic
# "no networks" would hide the difference between not finding and not looking.
network-scanning = varrendo…
network-empty = nenhuma rede ao alcance
network-no-adapter = esta máquina não tem rádio Wi-Fi
network-radio-off = rádio desligado
network-wired = cabo
network-saved = salva
network-open = aberta
network-password = Senha de { $network }
network-password-help = Guardada pelo NetworkManager, fora do repositório.

# ECHOES — what the minibuffer says after the action.
echo-network-connected = Conectado a { $network }
echo-network-failed = Não consegui conectar a { $network }: { $reason }
echo-network-disconnected = Desconectado
echo-network-forgotten = { $network } esquecida
echo-network-radio-on = Rádio Wi-Fi ligado
echo-network-radio-off = Rádio Wi-Fi desligado
echo-network-link = { $network } · { $band } · { $rate } · sinal { $signal }
echo-network-link-wired = { $network } · { $rate }
echo-network-no-link = Sem conexão
echo-network-unavailable = O NetworkManager não está respondendo

## Syncthing (LEADER s s)

cmd-syncthing-open = Syncthing
cmd-syncthing-web = A página do próprio Syncthing
cmd-syncthing-state = Estado do Syncthing
cmd-syncthing-pause-all = Pausar a sincronização
cmd-syncthing-resume-all = Retomar a sincronização
group-syncthing = syncthing
echo-syncthing-state = { $connected } conectados · { $percent }% · ↑{ $up } ↓{ $down }
echo-syncthing-stopped = O Syncthing não está respondendo
echo-syncthing-paused = Sincronização pausada
echo-syncthing-resumed = Sincronização retomada

# Syncthing (LEADER s s) — one view, two doors, and the mark the
# modeline may draw. The option labels below are owed by
# `assets/syncthing-options.nix` through the generator; `syncthing-options-have-text`
# is what says so.
echo-audio-will-not-parse = O audio.json não parseia, e escrever o substituiria. Conserte ou remova o arquivo.
syncthing-title = Syncthing
hint-syncthing-on = Tudo está sendo sincronizado.
hint-syncthing-off = Nada está sendo sincronizado.
syncthing-no-service = O Syncthing não está respondendo. Nada está sendo sincronizado.
syncthing-gone = Ela não está mais aí.
syncthing-state = estado
syncthing-state-down = fora do ar
syncthing-state-paused = tudo pausado
syncthing-state-syncing = sincronizando
syncthing-state-idle = em dia
syncthing-connected = máquinas conectadas
syncthing-connected-of = { $connected } de { $total }
syncthing-traffic = transferido
syncthing-traffic-both = ↑ { $up }  ↓ { $down }
syncthing-invitations = pedindo para entrar
syncthing-wants-in = uma máquina em { $address }
syncthing-offered-by = uma pasta oferecida por { $who }
syncthing-folders = pastas
syncthing-no-folders = Nenhuma pasta compartilhada ainda.
syncthing-devices = máquinas
syncthing-no-devices = Nenhuma outra máquina pareada ainda.
syncthing-service = o serviço
syncthing-settings = mais ajustes
syncthing-global = o serviço inteiro
syncthing-interface = a interface web
syncthing-interface-about = A página que `w` abre. Ela fica nesta máquina: na rede seria uma porta administrativa sem senha.
syncthing-options = ajustes
syncthing-advanced = raramente vale mexer
syncthing-advanced-about = Nada no flake repõe estas: elas são suas, e sobrevivem a um rebuild.
syncthing-open = abrir
syncthing-do = fazer
syncthing-on = ligado
syncthing-off = desligado
syncthing-shared = compartilhada
syncthing-not-shared = não compartilhada
syncthing-mark = marca na modeline
syncthing-mark-never = nunca
syncthing-mark-when-off = quando nada sincroniza
syncthing-mark-when-on = enquanto sincroniza
syncthing-unit = o serviço do sistema
syncthing-unit-running = de pé
syncthing-unit-stopped = parado
syncthing-unit-moving = mudando
syncthing-unit-unknown = não dá para saber
syncthing-unit-absent = não instalado nesta máquina
syncthing-start-unit = iniciar o serviço
syncthing-stop-unit = parar o serviço
syncthing-restart = reiniciar
syncthing-pause-everything = pausar tudo
syncthing-resume-everything = retomar tudo
syncthing-version = versão
syncthing-this-machine = esta máquina
syncthing-uptime = de pé há
syncthing-listening = escutando em
syncthing-web = interface web
syncthing-for-days = { $n } dias
syncthing-for-hours = { $n } horas
syncthing-for-minutes = { $n } minutos
syncthing-for-seconds = { $n } segundos
syncthing-folder-id = id da pasta
syncthing-device-id = id da máquina
syncthing-size = tamanho
syncthing-errors = coisas que não conseguiu
syncthing-shared-with = compartilhada com
syncthing-actions = o que dá para fazer
syncthing-rescan = ler o disco agora
# THE REST OF THE HINT LINE'S WORDS. Eight of this view's ten keys had only
# a `cmd-` label, and the line reads the SHORT form: the screen said
# `[syncthing-down] · [syncthing-up] · …`, the missing-key marker, in both
# doors of `LEADER s s`. Found by `every-hint-key-explains-itself`, which
# walks every id of every table.
syncthing-up = subir
syncthing-down = descer
syncthing-less = anterior
syncthing-more = próximo
syncthing-enter = abrir
syncthing-pause = pausar
syncthing-add = acrescentar
syncthing-forget = remover
syncthing-override = mandar esta cópia
syncthing-revert = desfazer mudanças locais
syncthing-forget-folder = parar de compartilhar
syncthing-forget-device = esquecer esta máquina
syncthing-address = alcançada em
syncthing-running = rodando
syncthing-device-connected = conectada
syncthing-device-away = não conectada
syncthing-device-paused = pausada
syncthing-state-scanning = lendo o disco
syncthing-state-preparing = vendo o que mover
syncthing-state-cleaning = limpando versões antigas
syncthing-state-failing = algo está errado
syncthing-state-unknown = fazendo algo novo
ask-syncthing-enter = Valor novo
cmd-syncthing-add-device = Acrescentar uma máquina
ask-syncthing-add-device = Id da outra máquina
cmd-syncthing-add-folder = Compartilhar uma pasta
ask-syncthing-add-folder = Onde a pasta fica neste disco
echo-syncthing-flake-owns = O flake escreve esta. Mudar aqui duraria até o próximo rebuild.
echo-syncthing-refused = O Syncthing recusou: { $reason }
echo-syncthing-not-running = O Syncthing não está rodando.
echo-syncthing-not-allowed = O sistema não deixou isso acontecer.
echo-syncthing-unknown = Não funcionou, e o log diz por quê.
echo-syncthing-will-not-parse = O syncthing.json não parseia, e escrever o substituiria. Conserte ou remova o arquivo.
cmd-syncthing-window = Syncthing, em janela
cmd-syncthing-up = uma linha acima
cmd-syncthing-down = uma linha abaixo
cmd-syncthing-less = o valor anterior
cmd-syncthing-more = o próximo valor
cmd-syncthing-enter = fazer o que a linha faz
cmd-syncthing-pause = pausar ou retomar
cmd-syncthing-rescan = ler o disco agora
cmd-syncthing-add = acrescentar
cmd-syncthing-forget = remover, ou dizer não

# What every option Syncthing exposes is called, in our words.
syncthing-opt-folder-label = nome
syncthing-opt-folder-path = onde fica
syncthing-opt-folder-type = o que faz
syncthing-opt-folder-paused = pausada
syncthing-opt-rescan-interval = reler a cada
syncthing-opt-watch-for-changes = notar mudanças na hora
syncthing-opt-watch-delay = esperar antes de notar
syncthing-opt-watch-timeout = desistir de vigiar depois de
syncthing-opt-ignore-permissions = ignorar permissões
syncthing-opt-keep-conflicts = cópias de um conflito
syncthing-opt-pull-order = o que buscar primeiro
syncthing-opt-block-pull-order = que parte do arquivo primeiro
syncthing-opt-copy-range-method = como copiar um bloco
syncthing-opt-filesystem-type = sistema de arquivos
syncthing-opt-marker-name = pasta marcadora
syncthing-opt-auto-normalize = consertar nomes estranhos
syncthing-opt-case-sensitive = maiúscula difere de minúscula
syncthing-opt-junctions-as-dirs = junções são pastas
syncthing-opt-ignore-delete = nunca apagar nada
syncthing-opt-disable-sparse = escrever buracos como zeros
syncthing-opt-disable-fsync = não esperar o disco
syncthing-opt-block-indexing = não indexar
syncthing-opt-mod-time-window = relógios podem diferir em
syncthing-opt-max-concurrent-writes = escritas ao mesmo tempo
syncthing-opt-copy-ownership = herdar dono da pasta
syncthing-opt-sync-ownership = sincronizar dono
syncthing-opt-send-ownership = enviar dono
syncthing-opt-sync-xattrs = sincronizar atributos estendidos
syncthing-opt-send-xattrs = enviar atributos estendidos
syncthing-opt-scan-progress-interval = relatar progresso da leitura a cada
syncthing-opt-puller-max-pending = manter em voo
syncthing-opt-puller-delay = esperar entre buscas
syncthing-opt-puller-pause = pausa entre buscas
syncthing-opt-copiers = threads que copiam
syncthing-opt-hashers = threads que somam
syncthing-opt-folder-group = grupo
syncthing-opt-least-free-space = manter livre neste disco
syncthing-opt-versioning = guardar versões antigas
syncthing-opt-versioning-cleanup = limpar versões antigas a cada
syncthing-opt-versioning-path = onde ficam as versões antigas
syncthing-opt-xattr-permitted = atributos estendidos a guardar
syncthing-opt-device-name = nome
syncthing-opt-device-paused = pausada
syncthing-opt-compression = comprimir
syncthing-opt-auto-accept-folders = aceitar pastas que oferecer
syncthing-opt-introducer = deixar apresentar outras
syncthing-opt-skip-introduction-removals = manter o que deixar de apresentar
syncthing-opt-untrusted = não pode ler os arquivos
syncthing-opt-device-send-limit = enviar no máximo a
syncthing-opt-device-receive-limit = receber no máximo a
syncthing-opt-connections = conexões com ela
syncthing-opt-max-request = maior pedido
syncthing-opt-remote-interface-port = porta da interface web dela
syncthing-opt-device-group = grupo
syncthing-opt-device-addresses = onde encontrá-la
syncthing-opt-allowed-networks = só nestas redes
syncthing-opt-ignored-folders = pastas a ignorar dela
syncthing-opt-nat-traversal = abrir porta no roteador
syncthing-opt-relays = passar por um retransmissor
syncthing-opt-announce-locally = anunciar nesta rede
syncthing-opt-announce-globally = anunciar na internet
syncthing-opt-send-limit = enviar no máximo a
syncthing-opt-receive-limit = receber no máximo a
syncthing-opt-limit-inside-the-lan = limitar dentro desta rede também
syncthing-opt-low-priority = rodar em prioridade baixa
syncthing-opt-auto-upgrade = procurar versão nova a cada
syncthing-opt-usage-report = enviar números de uso
syncthing-opt-crash-reporting = enviar relatórios de falha
syncthing-opt-open-a-browser = abrir navegador ao iniciar
syncthing-opt-folder-concurrency = pastas trabalhando ao mesmo tempo
syncthing-opt-keep-temporaries = guardar arquivos pela metade por
syncthing-opt-reconnect-interval = tentar de novo a cada
syncthing-opt-progress-interval = relatar progresso a cada
syncthing-opt-announce-lan-addresses = anunciar endereços locais também
syncthing-opt-overwrite-names = renomear máquinas ao conectar
syncthing-opt-send-full-index = enviar índice inteiro após atualizar
syncthing-opt-temp-index-min-blocks = compartilhar arquivo pela metade após
syncthing-opt-incoming-request-buffer = buffer para pedidos que chegam
syncthing-opt-connection-limit-enough = conexões que já bastam
syncthing-opt-connection-limit-max = conexões no máximo
syncthing-opt-priority-tcp-lan = prioridade: TCP nesta rede
syncthing-opt-priority-tcp-wan = prioridade: TCP fora
syncthing-opt-priority-quic-lan = prioridade: QUIC nesta rede
syncthing-opt-priority-quic-wan = prioridade: QUIC fora
syncthing-opt-priority-relay = prioridade: por retransmissor
syncthing-opt-priority-upgrade-threshold = trocar por rota melhor em
syncthing-opt-traffic-class = classe de tráfego
syncthing-opt-announce-port = porta do anúncio
syncthing-opt-announce-group = grupo do anúncio
syncthing-opt-nat-lease = aluguel da porta no roteador
syncthing-opt-nat-renewal = renovar a porta do roteador a cada
syncthing-opt-nat-timeout = dar ao roteador
syncthing-opt-stun-keepalive-start = primeiro STUN após
syncthing-opt-stun-keepalive-least = STUN não antes de
syncthing-opt-relay-reconnect = tentar o retransmissor a cada
syncthing-opt-pre-releases = oferecer versões de teste
syncthing-opt-audit = escrever registro de auditoria
syncthing-opt-audit-file = arquivo do registro de auditoria
syncthing-opt-report-delay = esperar antes do primeiro relatório
syncthing-opt-report-insecurely = relatar sem conferir o certificado
syncthing-opt-report-address = para onde vão os relatórios
syncthing-opt-crash-address = para onde vão as falhas
syncthing-opt-releases-address = onde as versões novas são anunciadas
syncthing-opt-listen-addresses = escutar em
syncthing-opt-discovery-servers = a quem perguntar onde está uma máquina
syncthing-opt-stun-servers = servidores STUN
syncthing-opt-always-local-networks = tratar estas como locais
syncthing-opt-least-free-home = manter livre no disco de casa
syncthing-opt-interface-address = onde escuta
syncthing-opt-interface-enabled = a interface web existe
syncthing-opt-interface-tls = por HTTPS
syncthing-opt-interface-theme = tema dela
syncthing-opt-interface-user = nome de usuário
syncthing-opt-interface-auth-mode = como confere quem é você
syncthing-opt-interface-basic-auth-prompt = pedir senha ao navegador
syncthing-opt-interface-metrics-without-auth = métricas sem senha
syncthing-opt-interface-insecure-admin = administrar sem senha
syncthing-opt-interface-insecure-host-check = não conferir o cabeçalho Host
syncthing-opt-interface-insecure-frames = deixar outras páginas embutirem
syncthing-opt-interface-session-life = uma sessão dura
syncthing-opt-interface-socket-permissions = permissões do socket

# What a number is counted in.
syncthing-unit-seconds = segundos
syncthing-unit-hours = horas
syncthing-unit-minutes = minutos
syncthing-unit-kbps = kbit/s
syncthing-unit-kib = KiB
syncthing-unit-connections = conexões
syncthing-unit-writes = por vez
syncthing-unit-threads = threads
syncthing-unit-folders = por vez
syncthing-unit-copies = cópias
syncthing-unit-blocks = blocos

# The words a ring turns through.
syncthing-word-none = nenhuma
syncthing-word-sendreceive = enviar e receber
syncthing-word-sendonly = só enviar
syncthing-word-receiveonly = só receber
syncthing-word-receiveencrypted = receber, cifrado
syncthing-word-random = em qualquer ordem
syncthing-word-alphabetic = por nome
syncthing-word-smallest-first = menores primeiro
syncthing-word-largest-first = maiores primeiro
syncthing-word-oldest-first = mais antigos primeiro
syncthing-word-newest-first = mais novos primeiro
syncthing-word-standard = do jeito de sempre
syncthing-word-in-order = em ordem
syncthing-word-copy-file-range = copy_file_range
syncthing-word-ioctl = por referência (reflink)
syncthing-word-sendfile = sendfile
syncthing-word-duplicate-extents = duplicar extents
syncthing-word-all = o que funcionar
syncthing-word-basic = o comum
syncthing-word-fake = um de mentira, para teste
syncthing-word-trashcan = uma lixeira
syncthing-word-simple = algumas cópias numeradas
syncthing-word-staggered = mais das recentes
syncthing-word-external = entregar a um programa
syncthing-word-metadata = só as descrições
syncthing-word-always = sempre
syncthing-word-never = nunca
syncthing-word-static = um nome e uma senha
syncthing-word-ldap = um diretório LDAP
## Installer — the seventh question (network)

inst-wifi = Rede Wi-Fi
inst-wifi-help = A senha fica na máquina instalada, nunca no repositório.
inst-wifi-none = sem Wi-Fi — vou usar cabo
inst-wifi-scanning = Procurando redes…
inst-wifi-empty = Nenhuma rede ao alcance
inst-wifi-password = Senha da rede
inst-wifi-password-short = a senha de uma rede WPA tem ao menos 8 caracteres
inst-step-seal-wifi = Selar a senha do Wi-Fi no TPM
inst-step-wifi = Gravar o perfil da rede Wi-Fi

## Network — the password sealed in the TPM (psk-flags)

# The one-key question, after the connection succeeds.
network-remember = Guardar a senha de { $network } nesta máquina?
network-remember-no = não, não guardar nada
network-remember-yes = sim, selar no TPM
# The list marker: tells "saved in NetworkManager" apart from "the password exists".
network-no-tpm = no TPM
echo-network-stored = Senha de { $network } selada no TPM
echo-network-no-stored = Conectado. A senha não ficou guardada.
echo-network-store-failed = Conectado, mas não consegui guardar a senha: { $reason }
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
leader-set-window = janelas
cmd-leader-set-window = As mesmas teclas, cada uma abrindo o seu app numa janela em vez do rodapé
leader-set-full = tudo
cmd-leader-set-full = A árvore inteira de teclas, com cada submenu como era

## O painel de memoria — LEADER s m

cmd-memory-open = Memória do sistema
cmd-example-open = Vários teclados num app, a referência
example-title = Vários teclados
example-note = Cada teclado move a sua seleção e digita no seu campo, na cor do seu cursor.
example-list = Uma lista
example-form = Um formulário
example-empty = vazio
example-item-one = Manhã
example-item-two = Meio-dia
example-item-three = Tarde
example-item-four = Anoitecer
example-item-five = Noite
example-item-six = Madrugada
example-field-name = Nome
example-field-note = Nota
cmd-example-clear = Esvaziar o formulário
cmd-format-open = Formatar um disco
cmd-memory-full = Visão completa
cmd-memory-guard = Proteção contra travamento
cmd-memory-tab = Alternar RAM e disco
cmd-memory-refresh = Atualizar agora
cmd-memory-scan = Varrer
cmd-memory-scan-cancel = Parar a varredura
cmd-memory-stop = Encerrar o que está em foco
cmd-memory-sweep-store = Coletar o lixo do store
cmd-memory-sweep-generations = Aparar as gerações antigas do sistema
cmd-memory-sweep-journal = Aparar o journal
cmd-memory-tolerant = Afrouxar nesta sessão
cmd-memory-default = Voltar ao padrão
cmd-memory-persist = Gravar como permanente

memory-title = Memória
memory-ram = RAM
memory-zram = zram
memory-swap = Troca
memory-pressure = Travamento
memory-disk = Disco
memory-guard = Proteção
memory-kernel = Kernel
memory-cached = Cache de arquivo
memory-anonymous = Dos programas
memory-shared = Compartilhada
memory-dirty = A gravar
memory-desktop = Desktop
memory-apps = Aplicativos
memory-system = Sistema
memory-consumers = Quem está consumindo
memory-scanning = Varrendo…
memory-nothing-yet = Nada varrido ainda
memory-asked = Pedido enviado; a próxima leitura mostra o resultado.

memory-help-overview = m f completo · m o proteção · m s varrer · m r atualizar
memory-help-full = m v troca RAM/disco · m s varrer · m k encerrar · m g m n m j faxina
memory-help-guard = m t afrouxar · m d padrão · m p gravar · Esc volta

memory-guard-description = Quando a memória acaba, a máquina pode parar de responder por minutos. Esta proteção fecha UM aplicativo antes disso — nunca o desktop, que fica de fora por construção.
memory-guard-detail = Ela mede quanto tempo se passa esperando memória, e não quantos bytes sobraram: só age depois de um travamento sustentado, nunca de um pico.
memory-guard-armed = armada
memory-guard-off = desarmada
memory-guard-unknown = não sei dizer
memory-guard-strict = padrão
memory-guard-tolerant = tolerante
memory-guard-limit = Age acima de
memory-guard-for = Depois de
memory-guard-ceiling = Teto dos apps
memory-guard-acted = Agiu (neste boot)
memory-guard-persisted = Ao reiniciar
memory-guard-session-only = Esta troca vale só até reiniciar. m p grava no host.nix, e depois falta um nixos-rebuild switch.
memory-guard-cost = No modo tolerante a máquina pode ficar lenta por até um minuto antes de alguém ser fechado.

memory-confirm-stop = Encerrar o que está em foco? A mesma tecla de novo confirma; Esc cancela.
memory-confirm-sweep = Apagar o que não está em uso? A mesma tecla de novo confirma; Esc cancela.
memory-confirm-persist = Gravar este perfil no host.nix? A mesma tecla de novo confirma; Esc cancela.

group-system = sistema
memory-vram = Vídeo

## Rescue console — Ctrl+Alt+Del

rescue-title = Resgate
rescue-section-state = Estado
rescue-section-actions = Ações
rescue-section-windows = Janelas
rescue-section-terminals = Terminal
rescue-section-config = Configuração
rescue-section-log = Registro
rescue-section-system = Sistema

## Rescue console — the actions

rescue-restart-footer = Reiniciar o rodapé
rescue-restart-compositor = Reiniciar o compositor
rescue-restart-session = Reiniciar a sessão gráfica
rescue-restart-audio = Reiniciar o áudio
rescue-restore-keyboard = Restaurar o teclado
rescue-close-window = Fechar a janela em foco
rescue-refresh-fonts = Reconstruir o cache de fontes
rescue-open-terminal = Abrir um terminal
rescue-kill-window = Matar a janela escolhida
rescue-kill-hog = Matar quem mais come memória
rescue-edit-host = Editar a configuração desta máquina
rescue-rebuild-switch = Aplicar a configuração
rescue-rollback-now = Voltar para a geração anterior
rescue-rollback-boot = Geração anterior no próximo boot
rescue-restart-network = Reiniciar a rede
rescue-reboot = Reiniciar a máquina
rescue-poweroff = Desligar a máquina

## Rescue console — what it says

rescue-keys = ↑↓ escolhe · Enter roda · Tab troca de seção · Esc sai
rescue-running = rodando…
rescue-ran = pronto
rescue-failed = falhou
rescue-confirm = A mesma tecla de novo confirma; Esc cancela.
rescue-needs-password = pede a sua senha
rescue-read-only = leitura
rescue-generated-by = Gerado por { $source }. Editar aqui some no próximo rebuild.
rescue-no-windows = O compositor não respondeu — ou não há janela nenhuma.
rescue-empty = nada a mostrar
rescue-back = Sair do terminal devolve esta tela.
rescue-generation = geração { $number }
rescue-door-console = console
rescue-door-window = janela

cmd-rescue-open = Console de resgate

# ===================================================================
# O gerenciador de arquivos — LEADER . (rodapé) e a janela
#
# As explicações de permissão são a razão pela qual este arquivo passou a ter
# SELETOR. Numa pasta, `x` não é "executar", é "entrar", e `r` sem `x` lista
# nomes que não se alcança — dizer "executar" ali é dizer algo falso. A escolha
# entre as duas frases é do IDIOMA, então mora aqui e não no Rust.
# ===================================================================

# O que a primeira letra da coluna diz.
files-kind-directory = Uma pasta: um lugar, e não uma coisa.
files-kind-file = Um arquivo comum.
files-kind-symlink = Um atalho: aponta para outro caminho, e apagá-lo não apaga o alvo.
files-kind-fifo = Um cano nomeado: um programa escreve, outro lê.
files-kind-socket = Um soquete: dois programas conversando por aqui.
files-kind-block = Um dispositivo de bloco — um disco, ou parte de um.
files-kind-char = Um dispositivo de caractere.

files-perm-owner-read = { $kind ->
        [dir] { $set ->
                [yes] { $who } pode ver os nomes que há nesta pasta
               *[no] { $who } NÃO pode ver os nomes que há nesta pasta
            }
       *[file] { $set ->
                [yes] { $who } pode ler este arquivo
               *[no] { $who } NÃO pode ler este arquivo
            }
    }
files-perm-owner-write = { $kind ->
        [dir] { $set ->
                [yes] { $who } pode criar e apagar coisas dentro desta pasta
               *[no] { $who } NÃO pode criar nem apagar nada nesta pasta
            }
       *[file] { $set ->
                [yes] { $who } pode alterar este arquivo
               *[no] { $who } NÃO pode alterar este arquivo
            }
    }
files-perm-owner-execute = { $kind ->
        [dir] { $set ->
                [yes] { $who } pode ENTRAR nesta pasta
               *[no] { $who } NÃO pode entrar nesta pasta — nem no que está dentro
            }
       *[file] { $set ->
                [yes] { $who } pode rodar este arquivo como programa
               *[no] { $who } não pode rodar este arquivo
            }
    }
files-perm-group-read = { $kind ->
        [dir] { $set ->
                [yes] o grupo { $who } pode ver os nomes que há nesta pasta
               *[no] o grupo { $who } NÃO pode ver os nomes que há nesta pasta
            }
       *[file] { $set ->
                [yes] o grupo { $who } pode ler este arquivo
               *[no] o grupo { $who } NÃO pode ler este arquivo
            }
    }
files-perm-group-write = { $kind ->
        [dir] { $set ->
                [yes] o grupo { $who } pode criar e apagar coisas dentro desta pasta
               *[no] o grupo { $who } NÃO pode criar nem apagar nada nesta pasta
            }
       *[file] { $set ->
                [yes] o grupo { $who } pode alterar este arquivo
               *[no] o grupo { $who } NÃO pode alterar este arquivo
            }
    }
files-perm-group-execute = { $kind ->
        [dir] { $set ->
                [yes] o grupo { $who } pode ENTRAR nesta pasta
               *[no] o grupo { $who } NÃO pode entrar nesta pasta
            }
       *[file] { $set ->
                [yes] o grupo { $who } pode rodar este arquivo como programa
               *[no] o grupo { $who } não pode rodar este arquivo
            }
    }
files-perm-other-read = { $kind ->
        [dir] { $set ->
                [yes] qualquer pessoa da máquina pode ver os nomes desta pasta
               *[no] mais ninguém pode ver os nomes desta pasta
            }
       *[file] { $set ->
                [yes] qualquer pessoa da máquina pode ler este arquivo
               *[no] mais ninguém pode ler este arquivo
            }
    }
files-perm-other-write = { $kind ->
        [dir] { $set ->
                [yes] qualquer pessoa da máquina pode criar e apagar coisas aqui
               *[no] mais ninguém pode criar nem apagar nada aqui
            }
       *[file] { $set ->
                [yes] qualquer pessoa da máquina pode alterar este arquivo
               *[no] mais ninguém pode alterar este arquivo
            }
    }
files-perm-other-execute = { $kind ->
        [dir] { $set ->
                [yes] qualquer pessoa da máquina pode entrar nesta pasta
               *[no] mais ninguém pode entrar nesta pasta
            }
       *[file] { $set ->
                [yes] qualquer pessoa da máquina pode rodar este arquivo
               *[no] mais ninguém pode rodar este arquivo
            }
    }

# Os três bits que não são sobre uma pessoa. A frase começa pelo que
# surpreende: quem aponta para um `s` está perguntando sobre o `s`.
files-perm-setuid = { $set ->
        [yes] Roda como { $who }, e não como quem o executou.
       *[no] O bit está ligado numa posição que não executa — quase sempre um engano.
    }
files-perm-setgid = { $kind ->
        [dir] { $set ->
                [yes] O que nascer aqui dentro herda o grupo { $group }.
               *[no] O bit está ligado numa posição que não executa.
            }
       *[file] { $set ->
                [yes] Roda com o grupo { $group }, e não com o de quem o executou.
               *[no] O bit está ligado numa posição que não executa.
            }
    }
files-perm-sticky = { $set ->
        [yes] Só quem é dono de um arquivo pode apagá-lo daqui — é o que faz o /tmp funcionar.
       *[no] O bit está ligado numa posição que não executa.
    }

# Os quatro modos que respondem "o que isto devia ser".
files-preset-file = arquivo comum — você escreve, o resto lê
files-preset-private = só você, e mais ninguém
files-preset-program = programa — todos rodam, só você altera
files-preset-private-dir = pasta só sua
files-preset-readonly = ninguém altera, nem você

# O tamanho. A vírgula está AQUI porque o separador decimal é do idioma.
files-size-bytes = { $whole } B
files-size-kib = { $whole },{ $tenth } KiB
files-size-mib = { $whole },{ $tenth } MiB
files-size-gib = { $whole },{ $tenth } GiB
files-size-tib = { $whole },{ $tenth } TiB

# A data em ordem ISO nos dois idiomas: ela ordena sozinha e não confunde
# dia com mês, que é o único jeito de uma coluna de datas não mentir.
files-when = { $date } { $time }
files-when-utc = { $date } { $time } UTC
# THE SAME INSTANT, WRITTEN OUT. The column's form is `files-when`; this is
# what the pointer asks for, where there is a line and the question was which
# day and which second.
files-when-long = { $date } às { $time }

files-order-name = nome
files-order-size = tamanho
files-order-modified = modificado
files-order-kind = tipo

# As linhas do menu de ações. O id e a chave não podem divergir — um teste
# compara os dois, porque a linha copiada com o id trocado COMPILA e mostra as
# palavras da linha vizinha.
files-open = abrir
files-open-map = abrir o lugar num mapa
files-open-with = abrir com
files-terminal = terminal aqui
files-root-terminal = abrir como root
files-copy-path = copiar o caminho
files-copy = copiar
files-cut = recortar
files-paste = colar aqui
files-rename = renomear
files-info = informações
files-mode = permissões
files-owner = dono
files-group = grupo
files-compress = comprimir…
files-extract = extrair
files-trash = mover para a lixeira
files-delete = apagar de vez
files-shred = apagar sobrescrevendo

# THE VIEW'S OWN KEYS, and every one of these sentences is now ON SCREEN.
#
# They live in a table (`OF_THE_VIEW`) rather than inside a `match`, because
# the list is what the check reads: a key that only exists in a `match` arm is
# a key nothing can enumerate. For a long time that was the only reader —
# `files::OF_THE_VIEW` is in no registry, so none of these was in `M-x` or in
# the manual, and forty translated sentences were carried by both languages and
# drawn by nothing. `View::hint` reads them, in the table's order.
#
# SO THEY ARE SHORT AND THEY CARRY NO COMMA. The owner's rule for that line —
# one shortcut between any two separators, the key first, and the words after
# it brief. A comma inside one of these reads on screen as the end of the
# chunk, which is what the separator is for.
files-down = descer
files-up = subir
files-enter = abrir
files-parent = subir uma pasta
files-hint = digite para filtrar
# The panels. `no-room` is what the toggle says when it turns one on at a width
# that cannot hold it: the state flips anyway and the screen says why, which is
# the shape `files-sidebar-only` already has. `says` is one fact in full on the
# message line — a panel is twenty-four cells and a path is two hundred
# characters, so the row that had to be cut is the row somebody wants to read.
files-panel-no-room = não há espaço para o painel nesta largura — ele precisa de { $cells } colunas
files-fact-says = { $label }: { $value }
# WHAT THE PREVIEW SAYS WHEN THERE IS NO PICTURE. Not a refusal — those come
# from `ronoctua-inspect` and win when there is one — but the answer for a
# directory, a broken link, and a file nothing has been asked about. It used to
# be a large icon of the file's type, which said what the listing's own icon
# column already said and made an empty panel look full.
files-no-preview = sem prévia
# AND WHAT THE FACTS KEY SAYS WHEN THERE IS NOTHING TO SCROLL. From the
# outside, a key that refuses and a key that is dead look the same.
files-facts-all-shown = todos os fatos já estão à vista
files-preview-all-shown = a prévia inteira já está à vista
files-preview-cut = só as primeiras { $lines } linhas estão aqui — o arquivo continua
# O TRANSPORTE DA PRÉVIA. O que o ponteiro parado sobre um controle diz, e o que
# o gerenciador responde quando uma tecla faz algo que não se vê.
files-play-hold = segurar onde está
files-play-resume = deixar seguir
files-sound-on = ouvir
files-sound-off = em silêncio
files-sound-none = este arquivo não tem som nenhum
files-entry-not-here = esta entrada está noutro lugar, e nada aqui a busca: { $path }
files-entry-at = { $path }
files-entry-titled = { $name } · { $path }
files-entry-refused = este nome não pode ser um caminho: { $path }
files-list-nothing = não há nesta lista nada que se possa tocar daqui
files-list-at-start = esta é a primeira da lista
files-list-at-end = esta é a última da lista
files-playlist-previous = faixa anterior
files-playlist-next = próxima faixa
files-no-sound = esta máquina não tem som para tocá-lo — a imagem segue sem ele
files-film-stuck = nada saiu dele por { $seconds } s — foi parado
files-film-seek = ir para { $at } de { $total }
files-theatre-on = a prévia está com a região inteira
files-theatre-off = a listagem está com o espaço de volta
files-theatre-nothing = nada está selecionado, então não há o que expandir
files-wide-hidden = esse painel está escondido, então não há o que expandir
files-facts-wide-on = a ficha está com a região inteira, em quantas colunas ela couber
files-facts-wide-off = a ficha está de volta na coluna dela
# A OUTRA FORMA DA LISTAGEM. O que o glifo no fim da linha do caminho diz sob o
# ponteiro — para onde ele VAI, nunca onde ele está.
files-grid-on = os arquivos estão lado a lado, em miniaturas
files-grid-off = os arquivos voltaram a ser um por linha
# O QUE O ANEL DE TAMANHO DIZ QUE ELE PEGOU. Três chaves e não um seletor: um
# seletor escolhe o ramo pelo TIPO do argumento, e a linha a que isto chega é o
# `Step::Say`, cujos argumentos são strings por construção — então um número
# chegaria como texto e toda tela leria o ramo padrão.
files-grid-small = miniaturas pequenas
files-grid-medium = miniaturas médias
files-grid-large = miniaturas grandes
files-grid-moves = este se mexe — está parado porque outros estão se mexendo
files-grid-bigger = mostrar maiores
files-grid-none = os itens estão um por linha, então não há miniatura para dimensionar
files-no-root-helper = trocar o dono precisa do sistema instalado — a unidade ronoctua-files-root nao esta aqui
files-yes = sim, aplicar
files-no = não, deixar como está
files-items = { $n } itens
files-confirm-mode = Trocar as permissões de { $what } para { $mode }?
files-no-editor = o Emacs nao esta instalado neste perfil
files-menu-on = ações em { $name }
files-menu-on-many = ações em { $n } marcados
files-ask-application = Abrir com qual aplicativo?
files-ask-owner = Trocar o DONO para quem?
files-ask-group = Trocar o GRUPO para qual?
files-ask-mode = Trocar as PERMISSÕES para qual conjunto?
files-ask-name = Renomear para:
files-sort = ordenação
files-sort-reverse = inverter a ordem
files-mark = marcar
files-mark-all = marcar tudo
files-unmark-all = desmarcar tudo
files-first = o primeiro
files-last = o último
files-menu = menu de ações
files-measure = medir
files-pane-next = próxima seção
files-pane-previous = seção anterior
# E AS MESMAS DUAS SETAS, onde os itens estão lado a lado. Coisa diferente,
# palavra diferente: elas atravessam as SEÇÕES numa linha e andam pelos ITENS
# numa prateleira.
files-grid-next = próximo arquivo
files-grid-previous = arquivo anterior

files-type-is = { $named }
files-type-disagrees = o nome diz { $named }, o conteúdo diz { $found }

cmd-apps-pin = Pôr o aplicativo na barra lateral do gerenciador
cmd-files-open = Gerenciador de arquivos
cmd-files-window = Gerenciador de arquivos, em janela
group-file = arquivo

files-nothing-here = esta pasta está vazia

# A BARRA LATERAL — de onde se parte.
#
# As cinco primeiras sao ids de `OF_THE_VIEW` e de `menu.rs`; o
# `files-actions-have-text` as cobra a partir do id, sozinho.
files-preview = prévia
files-facts = informações
files-facts-scroll = página das informações
files-columns = permissões e data
files-header = nomes das colunas
files-sidebar = barra lateral
files-sidebar-focus = ir para a barra lateral, e voltar
# AS TECLAS DA PRÓPRIA PRÉVIA, e a razão de estarem aqui em vez de saírem do
# `menu.rs` é a do `leader::OF_CLIPBOARD_VIEW`: chave que só existe dentro de um
# braço de `match` é chave que nada consegue enumerar. São estas que o
# `LEADER :` lista, então cada uma diz o que FAZ, não em que tecla está.
files-preview-scroll = página da prévia
files-preview-left = prévia para a esquerda
files-preview-right = prévia para a direita
# O transporte do filme. `files-play` é uma tecla para os dois porque é isso
# que ela é: apertar com o filme rodando é o que pausa.
files-play = tocar ou pausar
files-rewind = voltar
files-forward = avançar
files-sound = som
files-theatre = expandir a prévia
files-facts-wide = expandir a ficha
files-grid = miniaturas
files-grid-size = tamanho da miniatura
# As da peça. `files-turn-left` é a direção da MÃO e não um eixo — veja o
# `View::spin`, que é onde essa convenção está escrita.
files-turn-left = girar à esquerda
files-turn-right = girar à direita
files-turn-up = girar para cima
files-turn-down = girar para baixo
files-zoom-in = aproximar
files-zoom-out = afastar
files-reframe = reenquadrar
files-eject = desmontar
files-unpin = tirar da barra
files-pin = pôr na barra lateral
files-unhide = trazer de volta uma linha oculta da barra
files-sidebar-hidden-ask = Trazer qual de volta?
files-sidebar-trash = lixeira
files-sidebar-open = abrir { $name }
files-sidebar-mount = montar { $name } e entrar
files-sidebar-unmount = desmontar { $name }
files-sidebar-launch = rodar { $name }
files-sidebar-pinned = { $name } está na barra lateral agora
files-sidebar-hidden = { $name } saiu da barra — { $key } traz de volta
files-sidebar-gone = { $name } não está mais na barra lateral
files-sidebar-only = esta tecla age numa linha da barra — { $key } vai até lá
files-sidebar-nothing-hidden = não há nada oculto para trazer de volta
files-sidebar-unwritable = não consegui salvar a barra lateral: { $why }
files-mount-failed = não consegui montar { $name }: { $why }
files-mount-no-driver = este sistema não lê um sistema de arquivos { $type }
files-mount-needs-admin = { $name } é do sistema; { $key } abre um terminal root
files-unmount-busy = { $name } está em uso — feche o que está aberto ali
files-unmount-needed = { $name } é onde o sistema mora ({ $why }); desmontar não é oferecido

# O MENU DA BARRA LATERAL. Cada id vira `sidebar-<id>`, e o check cobra os dois.
sidebar-open = abrir
sidebar-mount = montar
sidebar-unmount = desmontar
sidebar-eject = ejetar com segurança
sidebar-information = informações
sidebar-unpin = tirar da barra lateral
sidebar-format = formatar
sidebar-menu-on = o que fazer com { $name }
sidebar-fact-where = montado em
sidebar-fact-node = dispositivo
sidebar-fact-kind = sistema de arquivos
sidebar-fact-size = tamanho
sidebar-fact-drive = disco
sidebar-fact-needed = é onde o sistema mora
sidebar-not-mounted = não está montado
# POR QUE UM DISCO NÃO É OFERECIDO PARA FORMATAR. A frase é montada no
# hospedeiro e atravessa pronta: o buffer não tem pacote de idioma (inv. 11).
format-needed-at = o sistema mora em { $at }
format-needed-fstab = o /etc/fstab manda montar este no boot
format-needed-swap = é a memória de troca do sistema
format-refused-busy = { $name } está em uso — feche o que está aberto ali e tente de novo
format-refused-admin = { $name } é do sistema, e não dá para pedir permissão aqui
format-refused-other = não consegui formatar { $name }: { $why }
# O BUFFER DE FORMATAR — `LEADER s f`. Três perguntas, cada uma respondida com
# as setas e o Enter. A linguagem é deliberadamente simples: a linha do vfat diz
# para que serve, não como se chama. Quem já sabe que é FAT32 não perde nada;
# quem não sabe recebe uma resposta em vez de uma palavra para procurar.
format-title = Formatar um disco
format-empty = nenhum disco para formatar aqui
format-hint-list = ↑↓ escolher · Enter continuar · Esc fechar
format-hint-choose = ↑↓ escolher · escreva um nome · Enter continuar · Esc voltar
format-hint-confirm = ↑↓ escolher · Enter responder · Esc voltar
format-whole = o disco inteiro
format-part = uma parte do disco
format-blank = vazio
format-choose-title = Para que este disco vai servir?
format-recipe-universal = abre em qualquer computador — pendrive, câmera, televisão
format-recipe-large = igual, mas aceita arquivos grandes (acima de 4 GB)
format-recipe-windows = para usar no Windows
format-recipe-linux = para usar no Linux
format-recipe-modern = para o Linux, guardando fotografias do disco
format-recipe-missing = XFS não está aqui: esta distro não traz o programa que o cria
format-name = nome
format-name-limit = até 11 letras ou números
format-confirm-title = Isto apaga tudo o que está aí
format-confirm-whole = o disco inteiro será apagado, com todas as suas partes
format-confirm-part = esta parte do disco será apagada
format-confirm-no = não, voltar
format-confirm-yes = sim, apagar e formatar
format-working = escrevendo no disco — não desligue o computador nem tire o disco
format-done = pronto
# A LINHA DE DETALHE — o mesmo buffer respondendo a uma segunda pessoa.
#
# As linhas de cima são para quem não sabe o que é FAT32 e não deveria precisar
# saber. Estas são para quem sabe: o nome de verdade, os limites, e o que o
# gesto vai fazer com o disco. Uma linha, sob o cursor, na cor mais discreta que
# existe — quem está começando nunca precisa ler, e quem já sabe nunca precisa
# adivinhar.
format-tech-universal = vfat (FAT32) · até 4 GiB por arquivo · sem dono nem permissão
format-tech-large = exFAT · sem limite prático de arquivo · sem dono nem permissão
format-tech-windows = NTFS · dono e permissão do Windows · escrita pelo ntfs-3g
format-tech-linux = ext4 · dono e permissão Unix · com journal
format-tech-modern = Btrfs · subvolumes, snapshots, checksum e compressão
format-tech-missing = XFS · o mkfs.xfs não está instalado nesta distro
format-tech-whole = tabela de partições nova (dos até 2 TiB, gpt acima) e UMA partição
format-tech-part = só esta partição é reescrita; a tabela do disco não muda
format-tech-needed = montado pelo sistema em execução · o udisks recusaria, e esta tela nem oferece
format-tech-nothing = nenhum disco sob o cursor
# O SEGUNDO REGISTRO DE CADA AÇÃO — o que ela FAZ, para quem já sabe o que quer.
#
# O rótulo é para quem procura um verbo ("apagar de vez"); esta linha é para
# quem quer saber exatamente o que vai acontecer: qual chamada, quais opções, o
# que dá para desfazer. Aparece na linha de mensagem quando a linha está sob o
# CURSOR — não só sob o ponteiro, porque o que o mouse faz o teclado faz.
files-does-open = abre no aplicativo padrão do tipo MIME; pasta e arquivo compactado entram na listagem
# THE ONE ACTION IN THIS MENU THAT LEAVES THE MACHINE, and the sentence says
# so before the key is pressed — the same courtesy the glance's two hosts get.
files-does-open-map = abre no mapa do navegador a coordenada gravada neste arquivo — a coordenada sai desta máquina
files-does-open-map-nowhere = este arquivo não tem coordenada gravada
files-does-open-with = escolhe entre os `.desktop` que declaram este tipo MIME
files-does-terminal = abre o terminal nesta pasta, como você
files-does-root-terminal = abre o terminal como root, pelo pkexec
files-does-copy-path = põe o caminho absoluto na área de transferência; não abre nada
files-does-copy = marca para copiar; colar cria a cópia pela fila, que sobrevive ao rodapé
files-does-cut = marca para mover; no mesmo sistema de arquivos é rename(2), instantâneo
files-does-paste = executa o que foi marcado; entre sistemas de arquivos é cópia e depois remoção
files-does-rename = renomeia no lugar com rename(2); não atravessa sistema de arquivos
files-does-pin = acrescenta uma linha à barra lateral, gravada em $XDG_CONFIG_HOME
files-does-unhide = passa a listar também o que começa com ponto
files-does-info = tamanho, dono, modo, tempos e tipo; não toca no conteúdo
files-does-mode = altera os bits de permissão com chmod(2)
files-does-owner = altera o dono com chown(2); pergunta a senha de root
files-does-group = altera o grupo com chown(2)
files-does-compress = cria um arquivo compactado pela fila, sem bloquear a listagem
files-does-extract = extrai para uma pasta com o nome do arquivo
files-does-restore = devolve ao caminho de origem que a lixeira gravou
files-does-trash = move para a lixeira XDG; não atravessa sistema de arquivos
files-does-delete = unlink(2) ou rmdir(2) direto: sem lixeira e sem volta
files-does-shred = sobrescreve antes de apagar; num btrfs com cópia-na-escrita isso não garante nada

# E os do menu da barra lateral, que falam com o udisks.
sidebar-does-open = entra na pasta; se for dispositivo desmontado, abre a ficha dele
sidebar-does-mount = Filesystem.Mount do udisks2 — ele escolhe o ponto e as opções
sidebar-does-unmount = Filesystem.Unmount sem forçar: recusa se algo ainda estiver aberto ali
sidebar-does-eject = desmonta, tranca de novo um disco cifrado e corta a energia (Drive.PowerOff): é o que deixa tirar
sidebar-does-information = rótulo, nó, sistema de arquivos, tamanho e disco
sidebar-does-unpin = tira a linha; padrão da distro volta oculto, o seu é removido
sidebar-does-format = abre o buffer de formatar já apontado para este alvo




files-disks-not-watching = os discos não vão se atualizar sozinhos: { $why }
files-nothing-to-choose = não há nada para escolher
files-copied = copiado
files-not-copied = não consegui copiar
files-nothing-opens = nenhum aplicativo declara abrir { $type }
files-no-pkexec = o `pkexec` não foi definido em nix/rust.nix
files-no-systemctl = o `systemctl` não foi definido em nix/rust.nix
files-refused = { $why }

# O cabeçalho da tabela. Clicar num deles ordena por ele; clicar de novo
# inverte. A tecla `s` faz a mesma volta.
files-column-icon = categoria
files-column-name = nome
files-column-perms = permissões
files-column-owner = dono
files-column-group = grupo
files-column-size = tamanho
files-column-modified = modificado

# O que cada campo diz ao ponteiro. Campo que acende e não explica promete uma
# ação e não descreve nenhuma.
# THE WHOLE NAME, and only the name.
#
# The line stands in for a column that had to cut, so it says what the column
# would have said and nothing else: a sentence around it would push the end of
# a long name back off the edge, which is the thing this exists to undo.
files-explain-name = { $name }
# THE EXECUTE BIT, which is the one thing the type's glyph says besides the
# type. The owner asked for the colour to move off the name and onto the
# glyph, and a colour with no words is a legend nobody reads — so the pointer
# resting on a green glyph says what the green MEANS.
#
# It names the bit and not what running it would do: this manager does not run
# a file by pressing RET on it, and a sentence promising that would be a
# sentence about another program.
files-explain-executable = este arquivo tem o bit de execução — o sistema pode rodá-lo
files-explain-owner = { $who } é o dono; clicar troca
files-explain-group = o grupo é { $who }; clicar troca
files-explain-size = { $bytes } bytes exatos
files-explain-measure = clique para medir esta pasta — é uma caminhada
# THE NUMBER SAYS WHAT IT IS. Each of the ten letters explains itself, and the
# form most people actually type explained nothing at all — read off the screen:
# the pointer rested on it and the line stayed empty.
#
# One sentence for three digits and for four: naming the three audiences stays
# true when a leading digit arrives, and the special bit that brought it already
# has a sentence of its own on the letter beside it.
files-explain-octal = { $octal } é a mesma permissão escrita como número — um dígito para o dono, um para o grupo, um para todos os outros
files-octal-can = { $kind ->
        [dir] { $bits ->
                [0] não pode fazer nada com ele
                [1] pode entrar nele, mas não listá-lo
                [2] pode criar e apagar coisas nele, sem poder entrar nem listar
                [3] pode entrar nele e criar e apagar coisas nele
                [4] pode listar os nomes que há nele
                [5] pode listar os nomes que há nele e entrar
                [6] pode listar os nomes que há nele e criar e apagar coisas
               *[7] pode listar, entrar, criar e apagar
            }
       *[file] { $bits ->
                [0] não pode fazer nada com ele
                [1] pode executá-lo
                [2] pode alterá-lo
                [3] pode alterá-lo e executá-lo
                [4] pode lê-lo
                [5] pode lê-lo e executá-lo
                [6] pode lê-lo e alterá-lo
               *[7] pode lê-lo, alterá-lo e executá-lo
            }
    }
files-explain-octal-digit = { $part ->
        [special] { $digit } é o primeiro dígito de { $octal }: os bits especiais — setuid, setgid e o sticky —, cada um explicado na letra que o carrega.
        [owner] { $digit } é o dígito do dono em { $octal }: { $name } { $can }.
        [group] { $digit } é o dígito do grupo em { $octal }: o grupo { $name } { $can }.
       *[other] { $digit } é o último dígito de { $octal }: todos os outros { $can }.
    }
files-explain-when = { $when }, na hora local desta máquina
files-explain-when-utc = { $when }, em UTC — não consegui ler o /etc/localtime

# A ficha de detalhe.
files-detail-path = caminho
files-detail-type = tipo
files-detail-mode = permissão
files-detail-owner = dono:grupo
files-detail-size = tamanho
files-detail-modified = modificado
files-detail-links = nomes deste inode
files-detail-target = aponta para
files-detail-broken = aponta para (não existe)
files-restore = devolver de onde veio
files-no-trash = não sei onde fica a lixeira; nada foi movido

# ===================================================================
# WHAT IS INSIDE A FILE — the preview and the facts panel
#
# `ronoctua-inspect` is the border with a file's CONTENT, and it does not
# translate: it answers with a text KEY and its arguments, and whoever holds
# the bundle turns the pair into a sentence (invariant 8). Every number below
# arrives as DATA — what is written here is only the words around it.
# ===================================================================

# WHY THERE IS NOTHING TO SHOW. Drawn where the content would be, centred, so
# each one has to say whether there is anything to be done about it: "it
# failed" is not an answer, it is the absence of one.
#
# THE PANEL IS NOT AN IMAGE PANEL, and the sentence that said so is gone. It
# used to answer a markdown file with "there is no picture inside this kind of
# file", which is a true statement about a question nobody asked — the panel
# shows what is IN a file, and a file of letters is full of content. What is
# left here is a file that has neither picture nor text, and it now says what a
# folder says: `files-no-preview`, one sentence for one fact.
files-preview-not-text = este arquivo não é texto, apesar do que o nome diz
files-preview-empty = não há nada dentro deste arquivo
files-preview-unreadable = não deu para abrir este arquivo para leitura
files-preview-missing = { $program } não está instalado, e é ele que lê este formato
files-preview-too-big = grande demais para uma prévia ({ $amount })
# NOT "a picture". This is every reader's failure, not the raster path's: a
# Word document whose piece table does not add up, a presentation with no
# records, a program that ran and answered nothing. The owner read it off the
# screen against a `.doc` — a panel telling him a perfectly good document was
# "not a picture" is a sentence about the wrong thing twice over.
files-preview-failed = não deu para ler este arquivo — ele pode estar quebrado
files-preview-slow = passou de { $seconds } s e foi parado — não é o tamanho, é algo travado
# NÃO "quebrado". O arquivo está inteiro e é este leitor que não dá conta —
# dizer "quebrado" manda a pessoa consertar algo que não tem defeito.
files-preview-draco = as formas neste arquivo estão comprimidas, e este leitor não abre isso
files-preview-no-synth = este arquivo guarda uma partitura, não uma gravação, e não há aqui com que tocá-la
files-preview-playlist-unread = isto é uma lista do que tocar, numa grafia que este leitor não abre
files-preview-own-newer = isto foi salvo por um player mais novo que este
# POR CIMA da figura, na última linha dela, e só quando alguma face não encheu.
# Peça meio contornada é peça sobre a qual alguém precisa ser avisado — e o
# painel é onde a pessoa está olhando.
files-preview-outlined = { $n ->
        [one] uma face em contorno
       *[other] { $n } faces em contorno
    }

# THE PANEL'S LEFT COLUMN. Short nouns, and short on purpose: the column is as
# wide as the widest label here, so a needless word costs every other row.
files-fact-dimensions = dimensões
files-fact-duration = duração
files-fact-container = formato
files-fact-motion = foto com movimento
files-fact-video = vídeo
files-fact-audio = áudio
files-fact-rate = quadros
files-fact-sampling = amostragem
files-fact-channels = canais
files-fact-bitrate = taxa de bits
files-fact-title = título
files-fact-artist = artista
files-fact-album = álbum
files-fact-album-artist = artista do álbum
files-fact-track = faixa
files-fact-disc = disco
files-fact-year = ano
files-fact-genre = gênero
files-fact-composer = compositor
files-fact-publisher = editora
files-fact-isrc = ISRC
files-fact-encoder = codificador
files-fact-depth = bits por amostra
files-fact-layout = arranjo dos canais
files-fact-compression = compressão
files-fact-cover = capa
files-fact-camera = câmera
files-fact-lens = lente
files-fact-taken = feita em
files-fact-iso = ISO
files-fact-shutter = obturador
files-fact-aperture = abertura
files-fact-focal = focal
files-fact-place = lugar

# THE GLOBE AND WHAT IT ANSWERS — shared, and the prefix says so.
#
# These are not the file manager's words even though it is the first caller:
# the glance, the locale view and the installer all hold coordinates and could
# ask the same question. A sentence with an app's prefix is an app owning a
# word that is not its.
#
# WHICH LINES THERE ARE IS THE SELECTOR and the ORDER is the language's. A
# separator written into a value could not be counted and could not be given a
# register — see `chrome::Piece::separator`.
globe-address =
    { $has ->
        [street] { $street }, { $place } — { $region }, { $country }
        [place] { $place } — { $region }, { $country }
        [region] { $region }, { $country }
       *[country] { $country }
    }
globe-address-near = perto de { $place }, a cerca de { $km } km
globe-address-nowhere = nenhum lugar com nome perto desta coordenada
globe-behind = este lugar está do outro lado do globo
atlas-absent = os dados do mundo não estão nesta máquina
# THE ATTRIBUTION, ON SCREEN. GeoNames is CC BY 4.0 and the street packs are
# ODbL: both REQUIRE it, and this repository is MIT + OFL — so it is said in
# four places, of which this is the one a person actually reads. See
# `LICENSE-DATA.txt` for the other three.
atlas-credits = lugares do GeoNames (CC BY 4.0) · fronteiras do Natural Earth (domínio público) · ruas dos colaboradores do OpenStreetMap (ODbL)
atlas-heading = Dados do mundo
atlas-baseline = fronteiras e lugares
atlas-present = nesta máquina
atlas-offered = ruas, por região
atlas-not-yet = não está nesta máquina
atlas-not-yet-said = baixar as ruas de uma região ainda não está pronto — faltam o seletor, a fila e o leitor
atlas-open = abrir
atlas-previous = anterior
atlas-next = próximo
atlas-back = voltar
cmd-atlas-open = dados do mundo
cmd-atlas-window = dados do mundo numa janela
group-geography = geografia
files-explain-globe = arraste para girar, a roda para aproximar
files-explain-globe-dot = onde este arquivo diz que foi feito — Enter enquadra
files-explain-globe-address = { $value } — aperte para abrir esta coordenada no mapa do navegador.
globe-north = mover o globo para o norte
globe-south = mover o globo para o sul
globe-east = mover o globo para o leste
globe-west = mover o globo para o oeste
files-fact-where = onde
files-fact-pages = páginas

# AND THE RIGHT ONE, when the number needs a word around it. A unit is language
# business — where it goes, and whether it is a word at all.
files-value-pixels = { $width } × { $height }
files-value-fps = { $fps } fps
files-value-hertz = { $khz } kHz
# ONE, TWO, OR A COUNT. Which numbers earn a word of their own is a decision of
# the LANGUAGE, and this is the shape Fluent has for it — `$n` has to arrive as
# a NUMBER (`Arg::Number`), or every branch but the last is unreachable.
files-value-channels = { $n ->
        [1] mono
        [2] estéreo
       *[other] { $n } canais
    }
files-value-kbps = { $kbps } kbit/s
files-value-of-total = { $at } de { $total }
files-value-bits = { $n ->
        [one] { $n } bit
       *[other] { $n } bits
    }
files-value-lossless = nada foi descartado
files-value-lossy = encolhido descartando som
files-value-millimetres = { $mm } mm
files-value-seconds = { $s } s
files-value-coordinates = { $lat }, { $lon }
# The other selector, and a plural one: in pt-BR `one` covers 0 as well.
files-value-pages = { $n ->
        [one] { $n } página
       *[other] { $n } páginas
    }

# === A forma dentro de um arquivo de malha — `.stl` e `.3mf` ===============
#
# O gerenciador lê os triângulos e os gira; estas são as linhas que o painel de
# infos põe ao lado da figura. São o que alguém prestes a IMPRIMIR a peça lê:
# que tamanho tem, quanto material leva e se a superfície é fechada — que é a
# primeira pergunta de um fatiador e aquela sobre a qual ele recusa.
files-fact-format = formato
files-fact-unit = unidade
files-fact-objects = objetos
files-fact-triangles = triângulos
files-fact-vertices = vértices
files-fact-bounds = tamanho
files-fact-volume = volume
files-fact-area = superfície
files-fact-watertight = superfície fechada
files-fact-flipped = faces invertidas
files-fact-degenerate = triângulos chatos
files-fact-application = feito com
files-fact-designer = designer
files-fact-licence = licença
files-fact-created = criado em
files-fact-schema = escrito conforme
files-fact-source-name = nome de origem
files-fact-solids = sólidos
files-fact-shells = cascas
files-fact-faces = faces
files-fact-edges = arestas
files-fact-outlined = faces sem preenchimento

files-value-stl-binary = STL, em bytes
files-value-stl-ascii = STL, em palavras
files-value-three-mf = 3MF
files-value-obj = OBJ
# O mesmo formato escrito de dois jeitos, e qual dos dois decide se o arquivo
# abre num editor de texto. O painel diz, pela mesma razão que diz do STL.
files-value-ply-ascii = PLY, em palavras
files-value-ply-binary = PLY, em bytes
files-value-off = OFF
files-value-gltf = glTF
files-value-glb = glTF, em um arquivo só
files-value-collada = COLLADA
files-value-amf = AMF
files-value-step = STEP
files-value-iges = IGES
# NÃO é medida. A unidade do glTF é o metro pela especificação dele, e o
# arquivo não diz nada — então o painel diz de onde veio o número.
files-value-unit-by-convention = metro, pelo formato
files-value-count = { $n }
# AS MEDIDAS SÃO SEMPRE EM MILÍMETROS. Um 3MF pode declarar polegadas ou
# metros e o leitor converte na entrada, então esta unidade não é um palpite.
files-value-box = { $w } × { $d } × { $h } mm
files-value-cubic-centimetres = { $n } cm³
files-value-square-centimetres = { $n } cm²
files-value-mesh-closed = sim
# NÃO "não". O que dá para agir é quantas arestas estão abertas, porque é o
# número que uma ferramenta de reparo relata e que muda enquanto ela trabalha.
files-value-mesh-open = não — { $n } arestas abertas

# === A fila de transferências — `LEADER t` =================================
#
# Sete operações iam para uma thread sem dizer nada na tela: colar, apagar,
# sobrescrever, lixeira, comprimir, extrair e medir pasta. Agora todas entram na
# mesma fila, e é ela que a modeline resume e o painel detalha.
group-transfer = transferências
# O `c` deixou de ser um comando e virou grupo: o clipboard captura o que se
# copia, e a tela captura o que se vê.
group-miscellaneous = diversos

cmd-transfers-open = a fila de transferências
# As teclas da VISTA. O id É a chave, como no gerenciador de arquivos:
# `cmd-` é do registro de comandos, e estas não estão nele.
transfers-down = descer
transfers-up = subir
transfers-pause = segurar ou soltar esta transferência
transfers-cancel = desistir desta transferência
transfers-forget = tirar esta linha da lista
transfers-sweep = tirar da lista tudo o que já acabou
transfers-launch = abrir o que foi instalado
transfers-detail = a transferência, campo a campo
transfers-answer = responder
transfers-give-up-no = Não, deixar seguir
transfers-give-up-yes = Sim, desistir

transfers-empty = nada em andamento
transfers-offline = a fila de transferências não está no ar

# O que a operação É.
transfers-work-copy = copiar
transfers-work-move = mover
transfers-work-delete = apagar
transfers-work-shred = sobrescrever
transfers-work-trash = mandar para a lixeira
transfers-work-restore = devolver da lixeira
transfers-work-compress = comprimir
transfers-work-extract = extrair
transfers-work-measure = medir

# Onde ela está.
transfers-stage-weighing = pesando
transfers-stage-running = em andamento
transfers-stage-paused = segurada
transfers-stage-done = pronta
transfers-stage-failed = falhou
transfers-stage-cancelled = desistida

# Quem move. Por enquanto só o disco desta máquina.
transfers-engine-local = disco

# O sujeito, quando é mais de um.
transfers-and-more = { $name } e mais { $rest }
# Para onde vai. A seta é o único desenho aqui, e não é chrome: é a preposição.
transfers-into = → { $where }
# Quantos itens, para as operações que contam coisas e não bytes.
transfers-items = { $done } de { $total }

# ---------------------------------------------------------------------------
# A MESMA FILA, EM JANELA PRÓPRIA — `LEADER t w`, o buffer wasip2.
#
# Não é uma segunda implementação: é o mesmo socket, as mesmas linhas, com
# espaço para todas de uma vez e uma tela de detalhe que a região do rodapé não
# comporta. As teclas de dentro vivem sob `SPC m`, como as de todo buffer.
# ---------------------------------------------------------------------------
cmd-transfers-window = a fila de transferências, em janela
cmd-transfers-add = baixar o que está na área de transferência
cmd-transfers-video = baixar o vídeo do que está copiado
cmd-transfers-audio = baixar só o áudio do que está copiado
cmd-transfers-subtitles = baixar só a legenda do que está copiado
cmd-transfers-add-as-me = baixar o que está copiado, como eu
cmd-transfers-video-as-me = baixar o vídeo, como eu
cmd-transfers-audio-as-me = baixar só o áudio, como eu
cmd-transfers-subtitles-as-me = baixar só a legenda, como eu
# O grupo que custa o anonimato — e é por isso que ele é um prefixo, e não um
# interruptor guardado em outro lugar.
group-identified = como eu (custa o anonimato)
transfers-queued-as-me = na fila, COMO VOCÊ — este não é anônimo
transfers-window-field-identity = quem pediu
transfers-identity-mine = você
transfers-identity-anonymous = ninguém
transfers-nothing-copied = não há nada copiado
transfers-not-an-address = o que está copiado não é um endereço
transfers-queued = na fila
cmd-transfers-down = a próxima transferência
cmd-transfers-up = a transferência anterior
cmd-transfers-pause = segurar ou soltar
cmd-transfers-cancel = desistir
cmd-transfers-forget = tirar da lista
cmd-transfers-sweep = limpar o que acabou
cmd-transfers-launch = abrir o que ela instalou
cmd-transfers-detail = detalhe desta linha

transfers-window-title = transferências
transfers-window-detail = a transferência, campo a campo
transfers-window-empty = nada na fila
transfers-window-unknown = não se sabe
transfers-window-confirm-cancel = Desistir desta transferência? O que já foi escrito no destino é apagado.

transfers-window-field-engine = quem move
transfers-window-field-stage = situação
transfers-window-field-errand = operação
transfers-window-field-subject = no momento
transfers-window-field-done = feito
transfers-window-field-total = total
transfers-window-field-trouble = problema
transfers-window-field-answer = resposta

# Os motores que ainda vão chegar, e as operações de rede. Estão aqui desde já
# porque o buffer nomeia cada uma como constante — e uma palavra sem tradução
# reprova no check em vez de aparecer na tela.
transfers-engine-fetch = link
transfers-engine-media = mídia
transfers-engine-torrent = torrent
transfers-engine-atlas = região

transfers-work-fetch = baixar
transfers-work-media = baixar vídeo
transfers-work-audio = baixar áudio
transfers-work-subtitles = baixar legenda
transfers-work-torrent = torrent
transfers-work-region = instalar as ruas de uma região
transfers-engine-software = programas
transfers-work-software = alterar um programa


# ---------------------------------------------------------------------------
# A EXTENSÃO DO NAVEGADOR.
#
# São QUINZE chaves e o `nix/scripts/gen-extension.py` cobra todas nas duas
# línguas: uma que falte não vira texto em inglês, vira o nome da própria chave
# num botão. O popup não escreve palavra nenhuma — ele monta a tela a partir
# daqui.
# ---------------------------------------------------------------------------
ext-title = ronoctua
ext-queue-up = a fila está no ar
ext-queue-down = a fila não está no ar
ext-video = vídeo, na melhor qualidade
ext-audio = só o áudio
ext-subtitles = só a legenda
ext-scan = procurar mídia nesta página
ext-found-none = nada encontrado nesta página
ext-page-not-media = esta aba não tem endereço para baixar
ext-sent = mandado para a fila
ext-failed = a fila recusou
ext-intercept-on = downloads vão para a fila
ext-intercept-off = downloads ficam no navegador
ext-send-link = baixar este link pela ronoctua
ext-send-media = baixar a mídia desta página
ext-as-me = como você — não é anônimo
ext-anonymous = anônimo


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
cmd-brain-run = Rodar agora
cmd-brain-rehearse = Ensaiar
cmd-brain-abort = Interromper
cmd-brain-proceed = Começar já
cmd-brain-reset-steps = Voltar aos passos
cmd-brain-interval = De quanto em quanto
cmd-brain-window = Mudar o horário
cmd-brain-switch = Desligar
cmd-brain-keys = Todas as teclas
cmd-brain-log = Registro ao vivo
cmd-brain-row-also = A outra ação da linha

# A TELA EM SI.
brain-title = Brain
brain-nothing = Nada configurado ainda
brain-tasks = Tarefas
brain-activity = Atividade
brain-no-activity = Nada aconteceu ainda
brain-log = Registro ao vivo
brain-no-log = Nada registrado ainda
brain-schedule = Agenda
brain-steps = Passos
brain-history = Histórico

# O QUE O HEADER ENSINA sobre as teclas da própria página. O resto sai de
# `cmd-*`.
brain-do-open = Abrir
brain-do-back = Voltar
brain-do-pane = Outra metade
brain-do-change = Alterar isto
brain-do-keys = Todas as teclas
about-brain-do-open = Abre a tarefa sob o cursor: a agenda, os passos e o que ela fez nas últimas vezes.
about-brain-do-pane = Leva o teclado para a outra metade da página.
about-brain-do-change = Muda o ajuste ou o passo sob o cursor.
about-brain-do-back = Volta à lista de todas as tarefas.
about-brain-keys = Abre esta linha para ver toda tecla que a página atende, ou a fecha de novo.
about-brain-run = Roda esta tarefa agora, diga o que disser a agenda dela.
about-brain-rehearse = Ensaia esta tarefa: cada passo é percorrido e nada na máquina muda.
about-brain-switch = Desliga esta tarefa, ou a religa; desligada, ela nunca roda sozinha.
about-brain-interval = Passa para a próxima cadência em que esta tarefa roda.
about-brain-window = Passa para a próxima parte do dia em que esta tarefa pode rodar.
about-brain-reset-steps = Devolve todo passo desta tarefa ao que a distro traz.
about-brain-proceed = Começa agora a tarefa em contagem regressiva, sem esperar a contagem.
about-brain-abort = Para a tarefa que está rodando ou em contagem regressiva.
about-brain-log = Mostra o registro do próprio brain enquanto ele é escrito — cada tick, o que ele decidiu, cada aviso e erro — ou volta.
about-brain-row-also = Faz a outra coisa que a linha sob o cursor oferece: num erro ainda não visto, ignorar o tipo dele daqui em diante.

# AS LINHAS DA AGENDA. Cada uma é o NOME de um campo, então cada uma é um
# substantivo — e o valor ao lado é a resposta, nunca a metade de uma frase
# que começou no rótulo.
brain-enabled = Roda sozinha
brain-on = Sim
brain-off = Não, só quando você pedir
brain-every = Roda a cada
brain-days = dias
brain-part = Roda entre
brain-next = Próxima vez
brain-soon = Começa em
brain-run-now = Rodar agora
brain-rehearse-it = Ensaiar
brain-rehearsal = Ensaiando agora
brain-never = Nunca
brain-needs-root = Roda como root
brain-flake-owned = No flake
brain-changed = Alterado por você

# OS CAMPOS DO RODAPÉ.
brain-last-run = Última vez

# AS TAREFAS. `brain-task-<id>` nomeia uma e `<id>-does` diz o que ela faz — a
# segunda é um parágrafo no rodapé, então tem espaço para ser uma explicação
# inteira em vez de uma linha.

# ONDE UMA TAREFA ESTÁ AGORA. Uma frase curta cada: elas ficam numa coluna de
# doze células na página inicial.
brain-standing-idle = Esperando
brain-standing-queued = Na fila
brain-standing-running = Rodando

# E COMO UMA EXECUÇÃO TERMINOU. `falhou` é a palavra honesta para uma execução
# que não terminou; se foi defeito ou decisão é trabalho do motivo.
brain-outcome-skipped = Não rodou
brain-outcome-done = Deu certo
brain-outcome-failed = Falhou
brain-outcome-held = Esperou

# O QUE ESTÁ NO CAMINHO, na coluna ao lado do nome da tarefa.
brain-hold-ram = Memória ocupada
brain-hold-cpu = Processador ocupado
brain-hold-battery = Na bateria
brain-hold-busy = Uma de cada vez
brain-hold-network = Sem rede
brain-hold-asleep = Dormindo

# E A MESMA COISA COMO AVISO, na linha sob o header — o único lugar desta tela
# que pode alarmar, e só enquanto alguma coisa está mesmo no caminho.
#
# UMA FRASE INTEIRA POR MOTIVO, e não uma frase com o motivo enfiado dentro:
# um `Run` carrega uma chave e nada mais (invariante 11). E é a melhor das
# duas de qualquer jeito — "o processador está ocupado demais" diz a coisa,
# "segurado: cpu" faz alguém adivinhar.
brain-held-ram = Nada começa enquanto esta parte da memória estiver em uso. Segue sozinho quando a máquina estiver mais quieta.
brain-held-cpu = Nada começa enquanto o processador estiver ocupado assim. Segue sozinho quando a máquina estiver mais quieta.
brain-held-battery = Nada começa na bateria. Ligue a máquina na tomada, ou mude isso no brain.json.
brain-held-other = Alguma coisa está no caminho. A tarefa que ela segura diz qual.

# E POR QUE UMA EXECUÇÃO NÃO ACONTECEU. `interrompida` NÃO é um erro — é a
# pessoa tendo decidido, e a tela concordando com ela.
brain-why-off = Desligada
brain-why-no-systemctl = Sem systemd aqui
brain-why-runaway = Parou de dar notícia e a espera acabou. Pode muito bem ainda estar rodando.
brain-why-aborted = Você interrompeu. Nada deu errado.
brain-why-postponed = Você disse agora não, e a próxima hora mudou.
brain-why-ram = A memória estava ocupada
brain-why-cpu = O processador estava ocupado
brain-why-battery = Estava na bateria
brain-why-busy = Outra coisa estava rodando
brain-why-network = Não havia rede
brain-why-asleep = A máquina estava dormindo

# QUAL PARTE DO DIA uma tarefa pode começar. As horas são os mesmos dígitos em
# todo idioma; as palavras em volta não.
brain-part-any = qualquer hora
brain-part-night = 02:00 e 05:00
brain-part-morning = 08:00 e 12:00
brain-part-afternoon = 13:00 e 18:00
brain-part-evening = 19:00 e 23:00

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

# E O QUE CUSTA CADA UMA DAS DUAS RESPOSTAS. `agora não` não é "nunca": a
# corrida sai da fila e a próxima hora muda, que é a única coisa que de fato
# para uma contagem — tirar da fila apenas a devolveria na batida seguinte, e a
# pessoa responderia à mesma pergunta a cada meio minuto.
echo-brain-postponed = Agora não — ela volta na próxima hora dela
echo-brain-proceeding = Começando agora
echo-brain-idle = Não há nada para acontecer
echo-brain-unreachable = O cérebro não está respondendo

# THE UPDATE, WATCHED — the modeline's mark, the leader's group for the view
# that watches a run, and the countdown's two answers that view teaches too.
#
# The mark shows only while something runs and goes on its own. The update's
# commands, its steps and the view's own words are its brain module's texts,
# in that module's folder.
group-update = atualização
brain-abort = parar
brain-proceed = começar agora
hint-update-mark = clique para ver a atualização acontecer

## The theme chooser — LEADER s t. The themes are listed by id and have no
## key; these are the options beside them — the desk under the windows, the
## ground of the footer, the ground of the windows and the borders — their
## answers, and the hint. The absolute desk is `theme-backdrop-black` under a
## dark theme and `theme-backdrop-white` under a light one; the near-white,
## `theme-backdrop-near`, is offered only under a light theme.
theme-option-theme = Tema
theme-option-backdrop = Variações
theme-option-footer = Rodapé
theme-option-windows = Janelas
theme-option-borders = Bordas
theme-backdrop-black = Preto
theme-backdrop-white = Branco
theme-backdrop-near = Quase branco
theme-backdrop-theme = Cor do tema
theme-ground-theme = Do tema
theme-ground-desk = Do fundo
theme-borders-standard = Padrão
theme-borders-discreet = Discretas
theme-hint = { $apply } aplica · { $left }/{ $right } troca de coluna
echo-theme-unsaved = não deu para gravar o tema: { $error }
## What the person's theme file says that was refused, said where the file is
## read. `$file` is the file's name, `$field` its own key (theme, backdrop,
## footer, windows, borders) and `$value` what it said — the file's words.
echo-theme-refused = { $file } diz que { $field } é “{ $value }”, que não é uma opção aqui — recusado
echo-theme-unreadable = não deu para ler { $file }, então todas as escolhas nele foram recusadas: { $error }
## A write the chooser REFUSED: the file is there and does not read whole,
## and writing the whole choice over it would lose what it says. `$why` is
## the sentence that says what did not read (`echo-theme-refused` or
## `echo-theme-unreadable`).
echo-theme-kept = { $file } não foi gravado, para nada nele se perder; corrija ou apague à mão: { $why }

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

cmd-power-open = Energia
cmd-power-window = Energia em uma janela
cmd-power-profile-performance = Perfil de energia: desempenho
cmd-power-profile-balanced = Perfil de energia: equilibrado
cmd-power-profile-saver = Perfil de energia: economia
cmd-power-keep-awake = Manter a máquina acordada (alterna)
cmd-power-keep-lid = Ignorar a tampa (alterna)
cmd-power-thresholds = Limites de carga
cmd-power-suspend = Suspender
cmd-power-hibernate = Hibernar
cmd-power-reboot = Reiniciar
cmd-power-off = Desligar
cmd-power-enter = Aplicar, armar ou confirmar a linha
cmd-power-less = Baixar o limite sob o cursor
cmd-power-more = Subir o limite sob o cursor
cmd-power-up = Uma linha acima
cmd-power-down = Uma linha abaixo
group-power = energia

# A LINHA DE DICAS lê cada id da tabela de teclas da vista como um texto curto.
power-enter = aplicar
power-less = menos
power-more = mais
power-up = sobe
power-down = desce

power-title = Energia

# As duas marcas da modeline, na linha sob o ponteiro.
hint-battery = { $percent }% { $state }, { $time }, { $rate } W — clique abre energia
hint-profile = perfil: { $profile } — clique abre energia

# As seções da vista, um cabeçalho cada.
power-heading-battery = Bateria
power-heading-adapter = Carregador
power-heading-profile = Perfil
power-heading-thresholds = Limites de carga
power-heading-awake = Manter acordado
power-heading-lid = Tampa e ociosidade
power-heading-actions = Ações
power-heading-peripherals = Periféricos

power-no-battery = sem bateria nesta máquina
power-no-adapter = nenhum carregador conhecido
power-plugged = na tomada
power-unplugged = fora da tomada
power-no-daemon = o power-profiles-daemon não está rodando

power-fact-charge = carga
power-fact-time = tempo
power-fact-rate = taxa
power-fact-energy = energia
power-fact-health = saúde
power-fact-health-daemon = saúde, pelo firmware
power-percent = { $percent }%
power-fact-cycles = ciclos
power-fact-voltage = tensão
power-fact-temperature = temperatura
power-fact-technology = química
power-fact-make = fabricante
power-fact-supply = fonte
power-fact-behaviour = comportamento de carga
power-fact-charge-type = tipo de carga
power-fact-idle = ocioso

power-time-empty = { $time } restantes
power-time-full = { $time } até encher
power-time-unknown = tempo desconhecido
power-duration = { $h } h { $m } min
power-duration-minutes = { $m } min
power-rate = { $rate } W
power-energy = { $now } / { $full } Wh
power-health = { $health }% ({ $full } de { $design } Wh)
power-voltage = { $v } V
power-temperature = { $t } °C

power-state-unknown = desconhecido
power-state-charging = carregando
power-state-discharging = descarregando
power-state-empty = vazia
power-state-full = cheia
power-state-pending-charge = esperando para carregar
power-state-pending-discharge = esperando para descarregar

power-technology-unknown = desconhecida
power-technology-lithium-ion = íon de lítio
power-technology-lithium-polymer = polímero de lítio
power-technology-lithium-iron-phosphate = fosfato de ferro-lítio
power-technology-lead-acid = chumbo-ácido
power-technology-nickel-cadmium = níquel-cádmio
power-technology-nickel-metal-hydride = níquel-hidreto metálico

power-profile-performance = desempenho
power-profile-balanced = equilibrado
power-profile-power-saver = economia
power-profile-wanted = (sua escolha)
power-profile-not-offered = não oferecido por esta máquina
power-degraded = desempenho degradado: { $why }
power-inhibited = desempenho impedido: { $why }
power-hold = { $who } segura { $profile }: { $why }
power-battery-aware = baixar o perfil quando a bateria estiver acabando
power-on = ligado
power-off-label = desligado

power-threshold-start = começa a carregar abaixo de ({ $supply })
power-threshold-end = para de carregar em ({ $supply })
power-threshold-pending = { $value }% — RET grava
power-threshold-none = não ajustável
power-preset-longevity = 40–80: um laptop que vive na mesa
power-preset-desk = 60–80: quase sempre na tomada
power-preset-full = carga cheia: uma viagem
power-thresholds-unsupported = esta bateria não tem limites de carga
power-thresholds-readonly = os limites são só leitura para você — a regra udev não chegou a esta bateria

power-keep-awake = manter acordado: sem sono, sem ação por ociosidade
power-keep-lid = ignorar a tampa: fechá-la não faz nada
power-inhibitor = { $who } segura { $what }: { $why }

power-lid-closed = a tampa está fechada
power-lid-open = a tampa está aberta
power-lid-action = fechar a tampa
power-lid-action-external = fechar a tampa, na tomada
power-lid-action-docked = fechar a tampa, no dock
power-lid-action-value = { $action }
power-action-unset = o padrão
power-power-key = a tecla de energia
power-idle-action = { $action } depois de { $time }
power-idle-never = nada
power-critical-action = no nível crítico, o UPower vai
power-blocked = bloqueado agora

power-action-suspend = suspender
power-action-suspend-then-hibernate = suspender e depois hibernar
power-action-hybrid-sleep = suspender e hibernar ao mesmo tempo
power-action-hibernate = hibernar
power-action-reboot = reiniciar
power-action-off = desligar
power-action-confirm = { $action }? RET confirma, Esc cancela
power-cannot = não disponível
# THE DAEMONS' WORDS — logind's kebab-case and UPower's CamelCase land on
# the same key (`power_words::daemon_word`); a word outside this list is shown raw.
power-duration-hours = { $h } h
hint-battery-no-rate = { $percent }% { $state }, { $time } — clique abre energia
notif-battery-critical-body-unset = { $percent }% restantes. A máquina vai agir sozinha em breve.
power-word-ignore = não fazer nada
power-word-poweroff = desligar
power-word-reboot = reiniciar
power-word-halt = parar
power-word-kexec = reiniciar pelo kexec
power-word-suspend = suspender
power-word-hibernate = hibernar
power-word-hybrid-sleep = entrar em sono híbrido
power-word-suspend-then-hibernate = suspender e depois hibernar
power-word-lock = bloquear a tela
power-word-factory-reset = restaurar de fábrica
power-word-soft-reboot = reiniciar sem desligar
# THE KERNEL'S TWO CHARGE VOCABULARIES — `charge_behaviour` and `charge_type`,
# from the `power_supply` class; the raw word shows only for one we do not know.
power-behaviour-auto = carregar normalmente
power-behaviour-inhibit-charge = manter a carga onde está
power-behaviour-inhibit-charge-awake = manter a carga enquanto acordado
power-behaviour-force-discharge = usar a bateria
power-charge-type-unknown = desconhecida
power-charge-type-na = não se aplica
power-charge-type-trickle = lenta
power-charge-type-fast = rápida
power-charge-type-standard = padrão
power-charge-type-adaptive = adaptativa
power-charge-type-custom = personalizada
power-charge-type-long-life = vida longa
power-charge-type-bypass = direto da tomada
# WHAT AN INHIBITOR HOLDS — nouns, because "holds sleep" is a thing held and the
# lid's `sleep` above is an action; in Portuguese they are two words.
notif-battery-critical-body-ignore = { $percent }% restantes, e nada vai impedir. Ligue na tomada agora.
power-what-sleep = o sono
power-what-idle = a ociosidade
power-what-shutdown = o desligamento
power-what-handle-power-key = a tecla de energia
power-what-handle-suspend-key = a tecla de suspender
power-what-handle-hibernate-key = a tecla de hibernar
power-what-handle-lid-switch = a tampa
power-what-handle-reboot-key = a tecla de reiniciar
power-word-sleep = dormir

power-peripheral = { $kind }: { $model }
power-kind-unknown = dispositivo
power-kind-line-power = carregador
power-kind-battery = bateria
power-kind-ups = nobreak
power-kind-monitor = monitor
power-kind-mouse = mouse
power-kind-keyboard = teclado
power-kind-pda = PDA
power-kind-phone = telefone
power-kind-media-player = tocador de mídia
power-kind-tablet = tablet
power-kind-computer = computador
power-kind-gaming-input = controle
power-kind-pen = caneta
power-kind-touchpad = touchpad
power-kind-modem = modem
power-kind-network = dispositivo de rede
power-kind-headset = headset
power-kind-speakers = caixas de som
power-kind-headphones = fones de ouvido
power-kind-video = dispositivo de vídeo
power-kind-other-audio = dispositivo de áudio
power-kind-remote-control = controle remoto
power-kind-printer = impressora
power-kind-scanner = scanner
power-kind-camera = câmera
power-kind-wearable = vestível
power-kind-toy = brinquedo
power-kind-bluetooth = dispositivo Bluetooth

# O que as portas dizem quando um daemon recusou, e o que a janela não faz.
door-no-footer = Não foi possível chegar ao rodapé para rodar isto
power-what-profile = o perfil
power-what-lock = a trava
power-what-thresholds = os limites
power-what-action = a ação
power-refused-authority = não permitido para esta sessão
power-refused-absent = o daemon não está lá, ou o hardware não consegue
power-refused-invalid = o valor foi recusado
power-refused-other = recusado
echo-power-refused = { $what } { $why }
echo-power-profile-not-offered = { $wanted } não é oferecido por esta máquina; { $applied } aplicado

# OS TRÊS ALARMES, levantados pelo rodapé quando o nível do UPower muda.
notif-power-app = energia
notif-bluetooth-app = bluetooth
notif-bluetooth-pairing-summary = Algo quer parear
notif-bluetooth-pairing-keys = { $yes } responde sim, { $no } responde não — depois que este aviso sai.
notif-bluetooth-pairing-body = Abra o Bluetooth para responder.
notif-action-bluetooth-accept = Sim, parear
notif-action-bluetooth-refuse = Não, recusar
notif-bluetooth-connected-summary = { $device } conectado
notif-bluetooth-connected-body = Está pronto para usar.
notif-bluetooth-disconnected-summary = { $device } desconectado
notif-bluetooth-disconnected-body = Pode ter sido desligado, ficado sem bateria ou saído do alcance.
notif-bluetooth-battery-low-summary = { $device } está acabando
notif-bluetooth-battery-low-body = Restam { $percent }%. Carregue quando puder.
notif-battery-low-summary = Bateria baixa
notif-battery-low-body = { $percent }% restantes. Ligue na tomada quando puder.
notif-battery-critical-summary = Bateria crítica
notif-battery-critical-body = { $percent }% restantes. A máquina vai { $action } sozinha em breve.
notif-battery-charged-summary = Bateria carregada
notif-battery-charged-body = Pode desligar da tomada.
notif-action-power-panel = Abrir o painel de energia
notif-action-bluetooth-panel = Abrir o Bluetooth

# O QUE A MÁQUINA FEZ, dito na linha do minibuffer (`events.rs`). O resumo é a
# linha; o corpo só é lido quando a linha estava ocupada e o evento veio como
# notificação — e então a última frase diz isso.
notif-privacy-app = privacidade
notif-network-app = rede
notif-minibuffer-was-busy = Isto costuma aparecer no minibuffer, mas ele estava ocupado, então veio como notificação.
notif-camera-in-use-summary = Uma câmera está aberta por { $programs }
notif-camera-in-use-body = Ela fica aberta até esse programa soltá-la.
notif-camera-free-summary = Nenhum programa está mais com uma câmera aberta
notif-camera-free-body = Todo programa que tinha uma aberta a soltou.
notif-microphone-in-use-summary = Um microfone está escutando
notif-microphone-in-use-body = Um programa o abriu. A vista de som diz qual.
notif-microphone-free-summary = Nenhum microfone está mais escutando
notif-microphone-free-body = Todo programa que escutava parou.
notif-studio-recording-started-summary = A câmera está gravando
notif-studio-recording-started-body = Ela grava até alguém parar.
notif-studio-recording-stopped-summary = A câmera parou de gravar
notif-studio-recording-stopped-body = A gravação terminou.
notif-studio-streaming-started-summary = A câmera está transmitindo
notif-studio-streaming-started-body = O que ela filma sai ao vivo até a transmissão parar.
notif-studio-streaming-stopped-summary = A transmissão terminou
notif-studio-streaming-stopped-body = Nada mais está saindo.
notif-network-connected-summary = Conectado a { $network }
notif-network-connected-body = A máquina tem uma saída por ela.
notif-network-lost-summary = { $network } caiu
notif-network-lost-body = A máquina não está mais conectada por ela.
notif-airplane-on-summary = Modo avião ligado
notif-airplane-on-body = Todo rádio e cabo está desligado.
notif-airplane-off-summary = Modo avião desligado
notif-airplane-off-body = Os rádios e o cabo voltaram.
notif-vpn-up-summary = { $vpn } está no ar
notif-vpn-up-body = O que sai da máquina passa pelo túnel.
notif-vpn-down-summary = { $vpn } caiu
notif-vpn-down-body = O que sai da máquina não passa mais pelo túnel.
notif-tailnet-up-summary = Na tailnet
notif-tailnet-up-body = Esta máquina alcança as outras que estão nela.
notif-tailnet-down-summary = Fora da tailnet
notif-tailnet-down-body = As outras máquinas dela estão fora de alcance.
notif-tor-up-summary = Saindo pelo Tor
notif-tor-up-body = Há um circuito pelo Tor.
notif-tor-down-summary = Não está mais saindo pelo Tor
notif-tor-down-body = Não há mais circuito pelo Tor.
notif-charger-plugged-summary = Carregador conectado
notif-charger-plugged-body = A energia vem do carregador.
notif-charger-unplugged-summary = Carregador removido
notif-charger-unplugged-body = A máquina está na bateria.

# A SAÍDA — as três marcas, e o que o daemon acredita.
network-way = a internet
network-airplane = modo avião
network-tunnel-drop = RET derruba
cmd-network-airplane = Modo avião (alterna)
echo-network-refused = A rede recusou: { $why }
echo-view-busy = outra vista está com o rodapé: Esc fecha ela primeiro
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
network-way-unknown = ainda não decidido
network-way-none = sem saída
network-way-portal = uma página de login está no caminho
network-way-limited = conectado, mas ela não responde
network-way-full = ela responde

# A VISTA — uma lista, quatro títulos, e as mesmas linhas nas duas portas.
network-heading-overview = num relance
network-heading-links = cabo e Wi-Fi
network-heading-wifi = Wi-Fi
network-heading-remembered = redes lembradas
network-no-remembered = esta máquina não lembra nenhuma rede que não esteja vendo
network-out-of-range = fora de alcance
network-heading-tunnels = túneis
network-radio = rádio Wi-Fi
network-on = ligado
network-off = desligado
network-no-link = esta máquina não tem cabo nem rádio
network-no-traffic = nada entrou nem saiu ainda
network-no-tunnel = nenhum túnel aqui
network-tunnel-raise = RET levanta
network-state-connected = conectado
network-state-connecting = conectando…
network-state-disconnected = sem conexão
network-fact-network = rede Wi-Fi
network-fact-band = banda do Wi-Fi
network-fact-signal = força do sinal
network-fact-rate = velocidade da conexão
network-fact-address = endereço desta máquina
network-import-address = seu endereço nela
network-fact-address-tailnet = IPv4 no tailnet
network-fact-address-tailnet6 = IPv6 no tailnet
network-fact-gateway = roteador
network-fact-dns = servidor DNS
network-fact-received = recebido
network-fact-received-boot = recebido desde o boot
network-fact-sent = enviado
network-fact-sent-boot = enviado desde o boot
network-heading-ports = portas abertas
network-heading-monitor = tráfego
network-no-ports = ninguém escutando
network-port-not-ours = não é nosso para ver
network-ports-more = { $count ->
    [one] e mais um nosso
   *[other] e mais { $count } nossos
  }
network-per-second = { $bytes }/s
number-decimal-mark = ,
# AS UNIDADES SÃO AS MESMAS EM TODO IDIOMA; o SEPARADOR DECIMAL não é, e é por
# isso que estas são chaves com um número dentro, e não uma string montada no
# crate.
network-rate-kilobits = { $rate } kb/s
network-rate-megabits = { $rate } Mb/s
network-rate-gigabits = { $rate } Gb/s
network-no-rate = não está de pé
network-band = { $ghz } GHz
network-band-unknown = uma frequência incomum
network-heading-lan = máquinas nesta rede
network-lan-nobody = ninguém foi falado ainda
network-lan-gateway = o roteador desta rede
network-lan-this-machine = esta máquina
network-lan-looking = procurando as outras…
network-lan-answered = respondeu
network-lan-refused = esta máquina não pode mandar ping: o sistema não a deixa abrir o socket
network-lan-swept = { $answered ->
    [one] uma máquina respondeu de { $asked } perguntadas, { $range }
   *[other] { $answered } máquinas responderam de { $asked } perguntadas, { $range }
  }
cmd-network-lan = Procurar as outras máquinas daqui
network-lan = procurar
network-import = trazer
network-section-next = seção
network-section-previous = seção anterior
network-hotspot-start = compartilhar esta conexão
network-hotspot-stop = parar de compartilhar
network-hotspot-cost = o rádio sai da rede em que está: o que ele compartilha é o cabo
network-hotspot-on = esta máquina é a rede agora
network-hotspot-password = senha do ponto de acesso
echo-network-hotspot = compartilhando como { $network }, senha { $password }
echo-network-hotspot-failed = não deu para compartilhar: { $reason }
cmd-network-import = Trazer uma VPN de um arquivo
cmd-network-section-next = Próxima seção da vista de rede
cmd-network-section-previous = Seção anterior da vista de rede
cmd-network-tor-open = Tor
cmd-network-tor-more = Girar este ajuste do Tor para a frente
cmd-network-tor-less = Girar este ajuste do Tor para trás
cmd-network-tor-back = Sair dos ajustes do Tor
ask-network-import = Caminho do arquivo .conf ou .ovpn
network-import-file = arquivo a trazer
network-import-kind = tipo de VPN
network-import-wireguard = WireGuard
network-import-openvpn = OpenVPN
network-import-endpoint = vai para
network-import-dns = mudaria seu DNS para
network-import-everything = tudo o que esta máquina manda passaria por ele
network-import-asks = pede usuário e senha assim que for trazida, e os guarda
network-import-bring = trazer
network-import-leave = deixar onde está
network-import-where = Caminho do arquivo .conf ou .ovpn
network-import-help = Leia antes de trazer: o arquivo decide por onde seu tráfego sai.
echo-network-imported = { $network } trazido
echo-network-not-a-tunnel = { $file } não é arquivo WireGuard nem OpenVPN
echo-network-import-failed = não deu para trazer { $file }: { $reason }

# OS DOIS TÚNEIS. Nenhum fica ligado em repouso: o Tor não sobe no boot e o
# Tailscale precisa de login, então uma máquina que não usa nenhum dos dois não
# desenha nenhum dos títulos.
network-heading-tailscale = Tailscale
network-heading-tor = Tor
network-tailscale-state = a conexão ao tailnet
network-tailscale-running = de pé
network-tailscale-stopped = desligado
network-tailscale-needslogin = esperando um login
network-tailscale-nostate = subindo
network-tailscale-starting = subindo
network-tailscale-needsmachineauth = esperando aprovação no tailnet
network-tailscale-inuseotheruser = outra pessoa nesta máquina está usando
network-tailscale-login = entrar
network-tailscale-logout = sair
network-tailscale-routes = aceitar rotas do tailnet
network-tailscale-shields = recusar o que chega
network-tailscale-online = on-line
network-tailscale-offline = off-line
network-tailscale-exit = tudo sai por aqui
network-tor-state = o serviço do Tor
network-tor-ready = há um circuito
network-tor-not-ready = sem circuito
network-tor-bootstrap = subindo, { $percent }%
network-tor-circuits = { $count ->
    [one] um circuito aberto
   *[other] { $count } circuitos abertos
  }
network-tor-identity = pedir uma identidade nova

# O TOR, INTEIRO. A seção é desenhada com o Tor rodando ou não, e isso é o
# conserto e não uma novidade: ela vivia atrás de "o socket de controle
# respondeu", o que só é verdade com o Tor já de pé — então um Tor parado não
# tinha título, nem linha, nem tecla, e o único interruptor que existia estava
# escrito para PARAR. Não havia como ligar o Tor em lugar nenhum do shell.
network-tor-running = de pé
network-tor-stopped = parado
network-tor-moving = mudando
network-tor-absent = não instalado nesta máquina
network-tor-unknown = não dá para perguntar
network-tor-start = ligar
network-tor-stop = desligar
network-tor-autostart = subir junto com a sessão
network-tor-settings = ajustes
network-tor-sheet = Tor
network-tor-browser = abrir um navegador pelo Tor
# O QUE O FLAKE FIXA. A linha é desenhada, é alcançável e recusa, dizendo de
# onde vem o valor — a forma que a linha desabilitada do XFS já tem no
# formatar. Editá-la seria uma tecla cujo efeito o torrc gerado desfaz antes de
# ela ser apertada.
network-tor-from-flake = definido pela configuração do sistema
network-tor-heading-service = O serviço
network-tor-heading-path = Por onde sai
network-tor-heading-bridges = Pontes
network-tor-heading-onion = Serviços onion
network-tor-heading-advanced = Raramente mexido
network-tor-advanced-about = Estes mudam como o Tor monta circuitos. Os padrões são o que a maioria quer.
ask-tor-setting = O valor novo — vazio limpa
ask-tor-lines = Uma linha para acrescentar — vazio limpa todas
network-tor-open = ajustes
network-tor-back = voltar
network-tor-edit = mudar
network-tor-more = mais
network-tor-less = menos
network-tor-drawer = a gaveta
echo-tor-written = Os ajustes do Tor foram gravados
echo-tor-refused = O Tor não aceitou: { $reason }
echo-tor-not-running = gravado; o Tor vai ler quando subir

# A TABELA, de `assets/tor-options.nix` pelo gerador. Uma chave que ninguém
# escreveu desenha como `[tor-opt-…]` na tela, e é por isso que o
# `tor-options-have-text` as conta.
tor-opt-socks-port = endereço SOCKS
tor-opt-cookie-authentication = autenticação por cookie
tor-opt-cookie-file = onde fica o cookie
tor-opt-cookie-group-readable = o grupo tor pode ler o cookie
tor-opt-exit-nodes = sair por
tor-opt-entry-nodes = entrar por
tor-opt-exclude-nodes = nunca usar
tor-opt-exclude-exit-nodes = nunca sair por
tor-opt-strict-nodes = tratar essas listas como regra, não preferência
tor-opt-circuit-lifetime = usar um circuito por
tor-opt-use-bridges = usar pontes
tor-opt-bridge-lines = pontes
tor-opt-bridge-transport = transporte
tor-opt-firewall-ports = portas que esta rede deixa sair
tor-opt-fascist-firewall = só sair pelas portas 80 e 443
tor-opt-onion-directory = onde a chave onion é guardada
tor-opt-onion-ports = portas que este serviço onion atende
tor-opt-onion-single-hop = um salto só — rápido, e não anônimo
tor-opt-safe-socks = recusar pedidos SOCKS que vazariam o nome
tor-opt-new-circuit-period = considerar um circuito novo a cada
tor-opt-circuit-build-timeout = desistir de montar um circuito após
tor-opt-entry-guards = quantas guardas de entrada
tor-opt-use-entry-guards = continuar entrando pelos mesmos relés
tor-opt-distinct-subnets = manter os relés de um circuito em redes diferentes
tor-opt-ipv6 = usar IPv6
tor-opt-prefer-ipv6 = preferir IPv6
tor-opt-connection-padding = tráfego de cobertura
tor-opt-reduced-padding = menos tráfego de cobertura
tor-opt-avoid-disk-writes = escrever no disco o mínimo possível
tor-opt-dormant-timeout = ficar ocioso após
tor-opt-dormant-on-start = começar ocioso
tor-opt-socks-proxy = alcançar a rede por um proxy SOCKS
tor-opt-https-proxy = alcançar a rede por um proxy HTTPS
tor-opt-log-level = o que o Tor escreve no log
tor-unit-seconds = segundos
tor-unit-guards = guardas
tor-word-none = nenhum
tor-word-obfs4 = obfs4
tor-word-snowflake = snowflake
tor-word-meek-lite = meek
tor-word-auto = como o Tor decidir
tor-word-0 = desligado
tor-word-1 = ligado
tor-word-notice-syslog = o de sempre
tor-word-info-syslog = mais
tor-word-debug-syslog = tudo
network-fact-name = nome no tailnet
network-fact-tailnet = nome do seu tailnet
network-fact-magic-dns = MagicDNS
network-fact-version = versão do Tor
network-fact-socks = endereço SOCKS
echo-network-login = abra { $url } para terminar de entrar
# AS PALAVRAS DA LINHA DE DICAS — o id lido como chave de texto, curtas porque
# a linha é uma só. A forma longa é `cmd-<id>`, que o manual lê.
network-up = sobe
network-down = desce
network-connect = conecta
network-scan = procura
network-wifi-toggle = rádio
network-disconnect = desconecta
network-forget = esquece
network-link = enlace
network-title = rede
cmd-network-window = Rede numa janela
cmd-network-up = Uma linha acima
cmd-network-down = Uma linha abaixo
echo-network-tunnel-up = { $network } está subindo
echo-network-tunnel-down = { $network } caiu
echo-network-deleted = { $network } apagado

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

cmd-bluetooth-open = Bluetooth
cmd-bluetooth-window = Bluetooth numa janela
cmd-bluetooth-scan = Procurar dispositivos por perto
cmd-bluetooth-power = Ligar ou desligar o rádio
cmd-bluetooth-discoverable = Deixar outros dispositivos verem esta máquina
cmd-bluetooth-reconnect = Reconectar os dispositivos confiáveis
cmd-bluetooth-disconnect-all = Desconectar todos os dispositivos
cmd-bluetooth-accept = Dizer sim ao pareamento
cmd-bluetooth-refuse = Dizer não ao pareamento
cmd-bluetooth-device = Abrir o dispositivo sob o cursor
cmd-bluetooth-connect = Conectar ou desconectar o dispositivo
cmd-bluetooth-pair = Parear com o dispositivo
cmd-bluetooth-forget = Esquecer o dispositivo
cmd-bluetooth-trust = Confiar no dispositivo, ou deixar de confiar
cmd-bluetooth-block = Bloquear o dispositivo, ou desbloquear
cmd-bluetooth-rename = Dar outro nome ao dispositivo
ask-bluetooth-rename = Um nome para ele
ask-bluetooth-accept = O código que ele pede — um PIN, ou seis dígitos
cmd-bluetooth-unnamed = Mostrar os dispositivos que não dizem o nome
cmd-bluetooth-up = Uma linha acima
cmd-bluetooth-down = Uma linha abaixo
group-bluetooth = bluetooth

# A LINHA DE DICAS lê cada id da tabela de teclas da vista como um texto curto.
bluetooth-down = desce
bluetooth-up = sobe
bluetooth-device = abrir
bluetooth-connect = conectar
bluetooth-pair = parear
bluetooth-forget = esquecer
bluetooth-trust = confiar
bluetooth-block = bloquear
bluetooth-rename = renomear
bluetooth-scan = procurar
bluetooth-power = rádio
bluetooth-discoverable = visível
bluetooth-unnamed = sem nome
bluetooth-accept = sim
bluetooth-refuse = não

bluetooth-title = Bluetooth
bluetooth-row-kind = { $name } — { $kind }
bluetooth-audio-state = { $what } — { $state }

# As marcas de dispositivo da modeline, na linha sob o ponteiro.
hint-bluetooth-device = { $kind }: { $name } — clique abre o bluetooth
hint-bluetooth-device-battery = { $kind }: { $name }, { $percent }% restantes — clique abre o bluetooth

# AS TRÊS SEÇÕES, e o que uma vazia diz.
bluetooth-heading-adapter = Rádio
bluetooth-heading-paired = Pareados
bluetooth-heading-nearby = Por perto
bluetooth-nothing-here = nada aqui
bluetooth-no-adapter = não há rádio Bluetooth nesta máquina
bluetooth-no-daemon = o serviço de Bluetooth não está respondendo
bluetooth-scanning = procurando dispositivos
bluetooth-unnamed-hidden = { $count } por perto sem nome — aperte h para mostrar

# AS LINHAS DO PRÓPRIO RÁDIO.
bluetooth-adapter-power = o rádio
bluetooth-adapter-discoverable = outros dispositivos veem esta máquina
bluetooth-adapter-pairable = outros dispositivos podem parear com ela
bluetooth-adapter-timeout = deixa de estar visível depois de
bluetooth-adapter-forever = fica visível
bluetooth-adapter-pairable-forever = fica aberto
bluetooth-adapter-alias = esta máquina se chama
bluetooth-power-state-on = ligado
bluetooth-power-state-off = desligado
bluetooth-power-state-off-enabling = ligando
bluetooth-power-state-on-disabling = desligando
bluetooth-power-state-off-blocked = travado pela chave do hardware

# OS FATOS DE UM DISPOSITIVO, um por linha, na ordem em que o detalhe desenha.
bluetooth-fact-address = endereço
bluetooth-fact-address-type = tipo de endereço
bluetooth-fact-alias = chamado de
bluetooth-fact-name = nome próprio
bluetooth-fact-connected = conectado
bluetooth-fact-paired = pareado
bluetooth-fact-bonded = as chaves ficaram guardadas
bluetooth-fact-trusted = confiável
bluetooth-fact-blocked = bloqueado
bluetooth-fact-legacy = pareamento antigo
bluetooth-fact-wake = pode acordar esta máquina
bluetooth-fact-services = serviços lidos
bluetooth-fact-battery = bateria
bluetooth-fact-battery-source = a bateria é informada por
bluetooth-fact-rssi = sinal
bluetooth-fact-tx-power = ele transmite a
bluetooth-fact-class = classe
bluetooth-fact-appearance = aparência
bluetooth-fact-modalias = id de firmware
bluetooth-fact-adapter = pelo rádio
bluetooth-fact-reconnect = quem reconecta
bluetooth-fact-volume = volume que ele informa
bluetooth-fact-codec = codec
bluetooth-fact-audio = perfil de áudio
bluetooth-yes = sim
bluetooth-no = não
bluetooth-rssi-value = { $dbm } dBm
bluetooth-tx-power-value = { $dbm } dBm
bluetooth-percent = { $percent }%
bluetooth-seconds = { $seconds }s

# OS TIPOS, da classe ou da aparência que o próprio dispositivo diz.
bluetooth-kind-headphones = fones de ouvido
bluetooth-kind-headset = headset
bluetooth-kind-speaker = caixa de som
bluetooth-kind-keyboard = teclado
bluetooth-kind-mouse = mouse
bluetooth-kind-gamepad = controle
bluetooth-kind-tablet = tablet
bluetooth-kind-phone = telefone
bluetooth-kind-computer = computador
bluetooth-kind-watch = relógio
bluetooth-kind-camera = câmera
bluetooth-kind-printer = impressora
bluetooth-kind-network = rede
bluetooth-kind-display = tela
bluetooth-kind-unknown = dispositivo

# AS ENUMERAÇÕES DO PRÓPRIO BLUEZ, na língua de quem lê. São listas fechadas,
# não dado livre — o NOME do dispositivo continua sendo dele. O `word_or` cai
# na palavra do daemon se ele ganhar um valor novo, então o novo fica sem
# tradução e nunca `[uma chave entre colchetes]`.
bluetooth-adapter-pairable-timeout = aberto para parear por
bluetooth-fact-manufacturer = fabricante
bluetooth-fact-version = versão do bluetooth
bluetooth-fact-roles = papéis
bluetooth-fact-service = oferece
bluetooth-audio-profile = o que está fazendo
bluetooth-heading-settings = Outras configurações
bluetooth-setting-volume = volume
bluetooth-setting-mute = no mudo
bluetooth-nothing-to-adjust = nada para ajustar aqui
bluetooth-settings = a outra metade
bluetooth-more = mais
bluetooth-less = menos
cmd-bluetooth-settings = Bluetooth: atravessa para a outra metade da página
cmd-bluetooth-more = Bluetooth: mais do que estiver sob o cursor
cmd-bluetooth-less = Bluetooth: menos do que estiver sob o cursor
bluetooth-audio-off = desligado
bluetooth-audio-a2dp-sink = música, em estéreo
bluetooth-audio-a2dp-source = música, vinda dele
bluetooth-audio-headset-head-unit = chamada, com o microfone dele
bluetooth-audio-headset-audio-gateway = chamada, por esta máquina
bluetooth-codec-0 = SBC
bluetooth-codec-1 = áudio MPEG-1/2
bluetooth-codec-2 = AAC
bluetooth-codec-4 = aptX
bluetooth-codec-255 = o do fabricante
bluetooth-address-type-public = fixo
bluetooth-address-type-random = aleatório
bluetooth-reconnect-none = nunca
bluetooth-reconnect-host = quando esta máquina pede
bluetooth-reconnect-device = quando o dispositivo pede
bluetooth-reconnect-any = qualquer um dos dois
bluetooth-transport-idle = parado
bluetooth-transport-pending = começando
bluetooth-transport-active = tocando
bluetooth-transport-suspending = parando
bluetooth-transport-broadcasting = transmitindo

# O QUE UM DISPOSITIVO FAZ — os perfis por trás dos ids de serviço.
bluetooth-profile-headset = chamadas, com o microfone dele
bluetooth-profile-handsfree = chamadas com as mãos livres
bluetooth-profile-audio-sink = toca o som desta máquina
bluetooth-profile-audio-source = manda som para esta máquina
bluetooth-profile-remote-control = os botões dele controlam a reprodução
bluetooth-profile-human-interface = manda teclas e movimentos de ponteiro
bluetooth-profile-file-transfer = navegar pelos arquivos dele
bluetooth-profile-object-push = receber arquivos enviados a ele
bluetooth-profile-phone-book = ler os contatos de um telefone
bluetooth-profile-message-access = ler as mensagens de um telefone
bluetooth-profile-network = compartilhar a conexão dele
bluetooth-profile-battery = informar quanta carga tem
bluetooth-profile-device-information = informar fabricante e modelo
bluetooth-profile-serial = uma porta serial pelo ar
bluetooth-profile-sim-access = ler o SIM de um telefone

# OS TECLADOS — o que uma tecla pode virar, e o que deu errado.
keyboard-trouble-file = o arquivo de regras de teclado não pôde ser lido ou escrito
keyboard-trouble-no-place = esta máquina não tem onde guardar regras de teclado — elas são escritas no sistema instalado
keyboard-trouble-refused = o keyd não aceitou estas regras, então nada mudou
keyboard-trouble-restart = as regras foram escritas, mas o keyd não reiniciou
keyboard-trouble-mode = os teclados não puderam trocar de modo, e ficam como estavam
keyboard-mode = Teclados
keyboard-mode-together = juntos, como um só
keyboard-mode-independent = cada um por si
keyboard-independent-note = Cada teclado digita pelo seu próprio cursor, com seu layout, suas teclas e sua leader.
keyboard-unit = Um teclado
keyboard-unplugged = desconectado
keyboard-cursor = cursor
keyboard-types-through = Digita pelo cursor
keyboard-layout = Layout
keyboard-variant = Variante
keyboard-leader-key = Tecla leader
keyboard-repeat-rate = Taxa de repetição
keyboard-repeat-delay = Atraso da repetição
keyboard-machines = o da máquina
keyboard-compositors = o do compositor
keys-region-taken = outro teclado está com o rodapé aberto; este espera até ele fechar

bluetooth-fact-kind = o que ele é
bluetooth-heading-profiles = Os serviços que ele anuncia
bluetooth-about-profile = { $id } — { $said }
bluetooth-about-profile-unknown = { $id } — um id do próprio fabricante, sem significado publicado. É desenhado como está, que é o que um buscador aceita.
bluetooth-about-audio-profile = Por onde o som vai: música em estéreo, ou uma chamada com o microfone. Quem decide é o PipeWire, não o aparelho.
bluetooth-about-sink-volume = Quão alto esta máquina toca por ele. RET sobe um passo; as setas movem para os dois lados.
bluetooth-about-mute = Silencia sem perder o número, então voltar devolve o mesmo volume.
bluetooth-door-this-keyboard = configurações deste teclado
bluetooth-door-all-keyboards = configurações de todos os teclados
bluetooth-about-this-keyboard = Abre as configurações de teclado com este selecionado, onde uma tecla pode passar a significar outra.
bluetooth-about-all-keyboards = Abre as configurações de teclado, que listam todos os teclados desta máquina.
# O QUE UMA LINHA QUER DIZER, para a linha de mensagem que o ponteiro consulta.
bluetooth-about-adapter = Por qual rádio desta máquina ele está pareado. Uma máquina com um chip Bluetooth tem um.
bluetooth-about-address = O endereço de hardware dele — os seis bytes que identificam esta unidade, e contra os quais os ajustes são lembrados.
bluetooth-about-address-type = Um endereço público é do fabricante para sempre; um aleatório é gerado, e alguns aparelhos trocam o deles para não serem seguidos.
bluetooth-about-alias = O nome que VOCÊ deu. É o que toda tela daqui mostra; o nome do próprio aparelho está na linha abaixo.
bluetooth-about-appearance = Um número que o aparelho manda para dizer que forma tem — 961 é teclado, 962 é mouse. Os de baixa energia mandam isto no lugar da classe.
bluetooth-about-audio = O que o enlace de som está fazendo agora, como o daemon reporta.
bluetooth-about-battery = Quanta carga ele diz ter. Nem todo aparelho reporta, e nenhum é preciso.
bluetooth-about-battery-source = Quem está reportando essa carga: o serviço padrão de bateria, ou o do próprio fabricante.
bluetooth-about-blocked = Bloqueado quer dizer que esta máquina o recusa, mesmo que ele peça. É como impedir que reconecte sem esquecê-lo.
bluetooth-about-bonded = As chaves combinadas no pareamento continuam guardadas, então os dois podem confiar um no outro sem parear de novo.
bluetooth-about-class = Um número que um aparelho clássico manda para dizer o que é — fone, telefone, computador. Os de baixa energia mandam uma aparência.
bluetooth-about-codec = Como o som é comprimido no caminho. O SBC funciona em tudo; os outros exigem que os dois lados concordem.
bluetooth-about-connected = Se há enlace de pé agora. Um aparelho pareado e desligado está pareado e não conectado.
bluetooth-about-kind = O que este aparelho é, deduzido do que ele diz de si — a classe, a aparência e os serviços que oferece.
bluetooth-about-legacy = Ele pareia do jeito antigo, sem a comparação numérica. Acessórios mais velhos fazem isso, e é menos seguro.
bluetooth-about-modalias = Os números de fabricante e modelo que ele reporta. É por isto que o keyd distingue um teclado, e é o que um buscador acha.
bluetooth-about-name = O nome que o aparelho dá a si mesmo. O que você vê em todo o resto é o apelido acima.
bluetooth-about-paired = Se esta máquina e o aparelho já trocaram chaves. Parear é o que torna possível conectar depois.
bluetooth-about-reconnect = Qual lado pode iniciar a conexão — esta máquina, o aparelho, ou qualquer um dos dois.
bluetooth-about-rssi = Quão alto esta máquina o escuta, em dBm. Mais perto de zero é mais forte; abaixo de uns -90 está no limite.
bluetooth-about-services = Se o daemon já terminou de perguntar ao aparelho o que ele faz. Enquanto não terminou, a lista abaixo pode estar curta.
bluetooth-about-trusted = Um aparelho confiável conecta sem perguntar toda vez. É a diferença entre um teclado que simplesmente funciona e um que pergunta.
bluetooth-about-tx-power = Quão alto o aparelho diz estar transmitindo. Junto com a força do sinal acima, dá uma ideia grosseira de distância.
bluetooth-about-volume = O volume que o APARELHO reporta pelo Bluetooth. O volume em que esta máquina toca está na outra coluna.
bluetooth-about-wake = Se apertar uma tecla nele pode trazer esta máquina de volta do sono.

# PAREAMENTO — a pergunta que o outro lado faz, e as duas respostas.
bluetooth-pairing-confirm = { $name } quer parear. Ele mostra { $code }?
bluetooth-pairing-passkey = digite { $code } em { $name }
bluetooth-pairing-pin = { $name } quer um PIN
bluetooth-pairing-passkey-here = { $name } quer seis dígitos digitados aqui
bluetooth-pairing-authorize = { $name } quer parear
bluetooth-pairing-service = { $name } quer usar { $service }
bluetooth-pairing-answer = y aceita · n recusa

# O QUE DEU ERRADO, dito como o nome da porta que fechou.
echo-bluetooth-refused = { $what }: { $why }
bluetooth-what-connect = a conexão
bluetooth-what-pair = o pareamento
bluetooth-what-forget = esquecer o dispositivo
bluetooth-what-radio = o rádio
bluetooth-what-device = o dispositivo
bluetooth-refused-authority = esta sessão não tem permissão
bluetooth-refused-absent = não existe nesta máquina
bluetooth-refused-away = ninguém respondeu
bluetooth-refused-busy = outra coisa está fazendo isso
bluetooth-refused-rejected = o outro lado disse não
bluetooth-refused-other = não funcionou

## Telas — os monitores, a imagem deles e a luz (LEADER s d)

cmd-display-open = Telas
cmd-display-window = Telas numa janela
cmd-display-down = Uma linha abaixo
display-back = voltar
cmd-display-up = Uma linha acima
cmd-display-less = O valor sob o cursor, um passo atrás
cmd-display-more = O valor sob o cursor, um passo adiante
cmd-display-enter = Abrir, copiar ou responder
cmd-display-automatic = Deixar o compositor escolher este ajuste
cmd-display-forget = Esquecer o que foi escolhido para esta tela
cmd-display-arrange = Onde as telas ficam
group-display = Telas

display-down = abaixo
display-up = acima
display-less = menos
display-more = mais
display-leftwards = para a esquerda
display-rightwards = para a direita
display-enter = abrir
display-enter-answers = responder
display-enter-copies = copiar
display-automatic = deixar escolher
display-forget = esquecer
display-arrange = arrumar


display-title = Telas
display-heading-screens = Telas
display-heading-every = Todas elas
display-heading-picture = A imagem
display-heading-light = A luz
display-heading-session = A sessão
display-heading-arrangement = Onde elas ficam
display-heading-forgetting = Esquecer os ajustes desta tela?
display-heading-keeping = Fica assim?

display-nothing-here = Nenhuma tela conectada.
display-reading = Perguntando ao compositor…
display-no-mode = o compositor não informa nenhum
display-no-thread-short = as telas não podem ser lidas aqui
display-no-watch = esta porta não vê o arquivo mudar
display-no-thread = As telas não podem ser lidas nesta máquina; nada aqui vai mudar.
display-no-compositor = Não há compositor para perguntar, então não há o que mostrar.
display-went-away = Essa tela sumiu.

display-settings-failed = O `display.json` não parseia, então nada do que foi escolhido aparece aqui. O arquivo está intacto — conserte ou remova.
display-one-screen-only = Há uma tela só, então não há o que arrumar.

display-this-one = { $name } — esta
display-arrangement = Disposição
display-text-size = Tamanho do texto

# Sob o cursor na linha do tamanho: o que a mudança alcança na hora e o que
# espera. Diz o que foi MEDIDO — ver docs/PENDING.md — e nunca promete um
# aplicativo que ninguém observou.
display-text-reaches = o shell e as janelas dele seguem na hora; aplicativos GTK e Qt seguem conforme redesenham
display-text-reopens = { $names } ficam no tamanho antigo até reabrir

display-fact-connector = Soquete
display-fact-filed = Os ajustes dela seguem
display-fact-size = Tela
display-filed-by-itself = o monitor, em qualquer soquete que ele esteja
display-filed-silent = o soquete — o monitor não diz nada sobre si
display-filed-no-serial = o soquete — o monitor não dá série, então um igual responderia por ele

display-on = Acionada
display-resolution = Resolução
display-rate = Taxa de atualização
display-scale = Escala
display-turn = Giro
display-place = Posição
display-vary = Taxa variável
display-brightness = Brilho
display-first = Abre com o foco aqui

display-vary-cannot = o hardware não consegue
display-vary-always = sempre
display-vary-on-demand = só quando uma janela pede
display-light-is-roots = o arquivo é do root — a regra que o abre não rodou aqui
display-light-on-its-buttons = O brilho deste monitor está nos botões dele.

display-turn-and-picture = { $set } (a imagem está { $picture })
display-turn-normal = normal
display-turn-rotated-90 = um quarto de volta
display-turn-rotated-180 = de cabeça para baixo
display-turn-rotated-270 = três quartos de volta
display-turn-flipped = espelhada
display-turn-flipped-90 = espelhada, um quarto de volta
display-turn-flipped-180 = invertida na vertical
display-turn-flipped-270 = espelhada, três quartos de volta

display-value-yes = sim
display-value-no = não
display-value-off = não está sendo acionada
display-value-automatic = o compositor escolhe
display-value-percent = { $percent }%
display-value-rate = { $hz } Hz
display-value-inches = { $inches } polegadas
display-value-place = x={ $x } y={ $y }
display-value-screens = { $n ->
        [0] nenhuma tela
        [one] uma tela
       *[other] { $n } telas
    }

display-left-of = à esquerda de { $other }
display-right-of = à direita de { $other }
display-above = acima de { $other }
display-below = abaixo de { $other }
display-arrange-how = As setas levam a tela sob o cursor até a próxima borda de uma vizinha — ao lado dela, alinhada com ela — e Shift com ↑ ou ↓ a põe acima ou abaixo.

display-ask = { $what } em { $screen }.
display-ask-seconds = Volta sozinho em { $seconds }s.
display-ask-forget = Tudo o que foi escolhido para { $screen } volta ao que o compositor decidir.
display-ask-nothing-yet = Nada mudou ainda.
display-leave-it = Deixar como está
display-forget-it = Esquecer tudo
display-place-and-picture = { $set } (está em { $picture })
display-scale-and-picture = { $set } (a imagem está em { $picture })
display-screen-gone = { $screen } não está mais aí; a mudança foi junto.
display-screen-gone-alone = { $screen } não está mais aí; nada tinha sido mudado.
display-only-one-lit = sim — e a única tela acesa
display-dark-not-ours = não — e nada aqui a desligou
display-only-one-offered = a única que este monitor oferece
display-put-it-back = Voltar como estava
display-keep-it = Manter assim
display-what-power-off = A tela foi desligada
display-what-resolution = A resolução mudou
display-what-rate = A taxa de atualização mudou
display-what-scale = A escala mudou
display-what-turn = A imagem foi girada

display-refused-no-compositor = não há compositor para perguntar
display-refused-no-screen = essa tela não está mais aí
display-refused-no-light = o brilho desta tela está nos botões dela
display-no-light-here = O brilho de nenhuma tela pode ser mudado daqui
display-refused-not-writable = o arquivo é do root, e a regra que o abre não rodou neste painel
display-refused-nowhere = não há onde escrever
display-refused-other = não deu
echo-display-refused-plain = As telas recusaram: { $why }

# OS TECLADOS (`LEADER s k`) — quais esta máquina tem, e o que cada tecla de
# cada um faz. Remapear é por teclado porque o keyd distingue dispositivos; o
# layout e a repetição são do compositor, um só para todos eles.
buffer-keyboard = Teclados
mode-keyboard = teclados
state-keyboard-editing = um teclado
cmd-keyboard-open = Teclados: o que cada tecla faz
cmd-keyboard-next = Teclados: a próxima linha
cmd-keyboard-previous = Teclados: a linha anterior
cmd-keyboard-list = Teclados: volta para a lista
keyboard-title = Teclados
keyboard-one = O que cada tecla faz
keyboard-none = o kernel não está reportando teclado nenhum
keyboard-built-in = parte desta máquina
keyboard-attached = ligado ou pareado
keyboard-changed = mudadas
keyboard-untouched = nada mudado
keyboard-becomes = faz
keyboard-the-key = a tecla
keyboard-unchanged = ela mesma
keyboard-holds = uma coisa segurando, outra tocando
keyboard-global = O layout, a tecla de acentos e a repetição são do compositor — um de cada, para todos os teclados.
keyboard-same-model = Dois teclados deste modelo obedecem a uma regra só: o keyd os distingue por marca e modelo, e estes dois têm os mesmos.
keyboard-is-the-leader = Esta é a leader. Movê-la tira a tecla que abre tudo, neste teclado.
keyboard-key-capslock = Caps Lock
keyboard-key-leftalt = o Alt da esquerda
keyboard-key-leftmeta = o Super da esquerda
keyboard-key-rightalt = AltGr
keyboard-key-rightmeta = o Super da direita
keyboard-key-menu = a tecla Menu
keyboard-key-insert = Insert
keyboard-choice-default = ela mesma
keyboard-choice-esc = Escape
keyboard-choice-control = Control
keyboard-choice-control-esc = Control segurando, Escape tocando
keyboard-choice-meta = Super
keyboard-choice-alt = Alt
keyboard-choice-backspace = Backspace
keyboard-choice-capslock = Caps Lock
keyboard-choice-noop = nada

# ---------------------------------------------------------------------------
# LEADER s l — language, clock, date and what the system decides.
# ---------------------------------------------------------------------------
language-en-gb = English
language-pt-br = Português
locale-heading-language = Interface
locale-speaking = em uso
locale-from-file = Você escolheu este idioma; ele fica no language.json.
locale-from-system = Este é o idioma da máquina, vindo da configuração do sistema.
locale-from-default = Ninguém escolheu; este é o primeiro idioma declarado.
locale-heading-time = Relógio e data
locale-clock-24h = Relógio de 24 horas
locale-clock-12h = Relógio de 12 horas
locale-clock-sample-24 = { $hh }:{ $mm }
locale-clock-sample-12 = { $hh }:{ $mm } { $half }
locale-clock-exact-24 = { $hh }:{ $mm }:{ $ss }
locale-clock-exact-12 = { $hh }:{ $mm }:{ $ss } { $half }
locale-clock-am = da manhã
locale-clock-pm = da tarde
locale-order-ymd = Ano primeiro
locale-order-dmy = Dia primeiro
locale-order-mdy = Mês primeiro
locale-date-ymd = { $y }-{ $m }-{ $d }
locale-date-dmy = { $d }/{ $m }/{ $y }
locale-date-mdy = { $m }/{ $d }/{ $y }

# THE TWELVE MONTHS, AS ONE KEY. A Fluent selector, because which numbers get a
# word of their own is the LANGUAGE's business — the same argument the channel
# counts already make. The month arrives as a NUMBER: `"01"` does not match `[1]`.
#
# Lower case, because Portuguese does not capitalise a month and these are
# written inside a sentence.
locale-month =
    { $m ->
        [1] janeiro
        [2] fevereiro
        [3] março
        [4] abril
        [5] maio
        [6] junho
        [7] julho
        [8] agosto
        [9] setembro
        [10] outubro
        [11] novembro
       *[12] dezembro
    }

# A DATE WRITTEN OUT, one key per order.
#
# The order is still the person's: somebody who chose "month first" and got
# "3 de setembro" would read a preference being ignored. Each language writes
# its three the way that language writes them, and the day and the year come
# UNPADDED — `03 de setembro` is a column's spelling, and this is not a column.
locale-date-long-ymd = { $y }, { $d } de { $month }
locale-date-long-dmy = { $d } de { $month } de { $y }
locale-date-long-mdy = { $month } { $d }, { $y }
locale-heading-machine = Decidido pelo sistema
# AS PALAVRAS DO PRÓPRIO COMANDO, desenhadas na linha de dicas: o `z` pede o
# fuso em que ESTA INTERFACE lê um carimbo, que não é o da máquina.
locale-zone = O fuso desta interface
locale-zone-unknown = desconhecido
locale-zone-is-declared = O fuso é declarado na configuração do sistema; mudá-lo é um rebuild.
locale-decimal = Separador decimal
locale-decimal-is-the-languages = O separador é do idioma, e acompanha ele.
locale-measurement = Medidas
locale-metric = Métrico
locale-not-written = A escolha não pôde ser gravada no disco.
locale-measurement-is-declared = Métrico seja qual for o idioma, e isso é decisão, não padrão.
cmd-locale-open = Idioma e região, no rodapé
cmd-locale-window = Idioma e região, em janela
cmd-locale-up = Linha anterior
cmd-locale-down = Próxima linha
cmd-locale-enter = Escolher
group-locale = idioma/região

# ---------------------------------------------------------------------------
# LEADER s g — as regiões cujas ruas esta máquina tem.
#
# Os nomes são as regiões do mundo como a Geofabrik as publica, e isso é DADO,
# nunca chave: `Île-de-France` não se traduz, e procurar `[geo-region-…]` seria
# procurar a tradução de um nome que alguém escreveu. Chave é a frase em volta.
# ---------------------------------------------------------------------------
geo-title = Regiões
mode-geo = regiões
geo-up = cima
geo-down = baixo
geo-enter = instalar
geo-find = filtrar
geo-remove = remover
geo-installed = Instaladas
geo-available = Regiões do mundo
geo-none-installed = Nenhuma região tem as ruas nesta máquina. Um endereço para na cidade mais próxima.
geo-no-world = Esta build não carrega os dados do mundo, então não há catálogo para escolher.
geo-filter = Filtro
geo-filter-none = todas as regiões
geo-pack = { $size } · { $ways } vias · { $version }
geo-have = instalada
geo-get = instalar
geo-in-parts = em partes
geo-only-in-parts = Esta região é publicada em partes. As partes estão nesta lista, e o filtro as encontra pelo nome dela.
geo-sure = Remover as ruas de { $region }?
geo-no = Não, manter
geo-yes = Sim, remover
geo-not-installed = Essa linha não é uma região instalada.
geo-not-removed = A região não pôde ser removida.
geo-not-in-catalogue = Esta região não está no catálogo desta build, então não dá para baixá-la de novo.
ask-geo-find = O nome de uma região, ou parte dele — vazio mostra todas
geo-queued = Na fila. O tamanho é o que o servidor responde, e dá para desistir por lá.
geo-no-home = Não há diretório de dados onde instalar.
cmd-geo-open = Regiões e suas ruas, no rodapé
cmd-geo-window = Regiões e suas ruas, em janela
cmd-geo-up = Linha anterior
cmd-geo-down = Próxima linha
cmd-geo-enter = Instalar a região sob o cursor, ou instalar de novo
cmd-geo-find = Estreitar a lista de regiões
cmd-geo-remove = Remover a região instalada sob o cursor
group-geo = geografia

# ---------------------------------------------------------------------------
# LEADER g — the glance: what time it is somewhere, and the weather and money.
# ---------------------------------------------------------------------------
world-place-europe-london = Londres
world-place-america-sao-paulo = Brasília
world-temperature = { $degrees } °C
world-rate = 1 { $from } = { $much } { $to }
cmd-world-glance = Um relance: a hora, o tempo e o câmbio
# PARA QUE SERVE CADA SEÇÃO. Toda uma delas abre com uma frase dizendo o que
# muda e ONDE isso aparece, porque uma tela de ajustes que só os nomeia deixa a
# pessoa adivinhar qual metade da máquina eles alcançam.
locale-interface-does = O idioma que esta interface fala. Muda na hora, no rodapé e em toda janela; o idioma da máquina é definido na configuração do sistema.
locale-time-does = Como esta interface escreve toda data e toda hora que ela desenha — o relógio do rodapé, as datas do gerenciador de arquivos, o relance. Não mexe no relógio da máquina; isso é a seção abaixo.
locale-numbers-does = Como os números são escritos e em que unidade sai a temperatura. Alcançam toda tela daqui, o relance inclusive.
locale-heading-machine-clock = O relógio desta máquina
locale-machine-does = O relógio, o fuso e a data do computador — o que todo programa desta máquina lê, e não só esta interface.
locale-automatic = Mantido certo automaticamente
locale-automatic-on = ligado
locale-automatic-off = desligado
locale-automatic-does = O sistema pergunta a hora a um servidor pela internet e mantém o relógio em dia com ele. É assim que a distro roda; desligar entrega o relógio a você.
locale-automatic-server = Perguntando a { $server }.
locale-automatic-synchronised = O relógio está em dia.
locale-automatic-adrift = Ele ainda não conseguiu falar com nenhum servidor de horário.
locale-automatic-cannot = Não há cliente de horário instalado nesta máquina.
locale-watched = Vigiado pelo brain
locale-watched-does = A vigia do relógio do brain olha algumas vezes por dia e religa o acerto automático se o encontrar desligado. É o mesmo interruptor da página daquela tarefa.
locale-watched-unknown = O brain não está rodando, então isto não dá para ler nem mudar daqui.
locale-machine-kept = Enquanto o relógio é mantido automaticamente, estes não podem ser acertados à mão: a próxima checagem os colocaria de volta.
locale-machine-unreachable = Não deu para perguntar nada sobre o relógio desta máquina; nada aqui pode ser mudado.
# AS DUAS DA MÁQUINA DIZEM O QUE APERTÁ-LAS FAZ. `Data` e `Hora` são as
# linhas da INTERFACE uma seção acima — elas escolhem um FORMATO e mostram
# uma amostra viva dele —, e duas linhas de uma lista com a mesma palavra são
# dois fatos que ninguém distingue.
locale-machine-date = Acertar a data
locale-machine-time = Acertar a hora
locale-machine-zone = Fuso horário
locale-shows-times-in = Mostrar as horas em
# O FORMATO É ESCRITO POR EXTENSO e não é a ordem escolhida pela pessoa: aqui
# se DIGITA um valor, e um pedido que seguisse a preferência de exibição
# pediria `09/21/2026` numa máquina e `21/09/2026` na seguinte.
ask-locale-system-date = A data da máquina, como 2026-09-21
ask-locale-system-time = A hora da máquina, como 14:32 ou 14:32:05
ask-locale-system-zone = O fuso horário da máquina, como America/Sao_Paulo
cmd-locale-system-date = Acertar a data desta máquina
cmd-locale-system-time = Acertar a hora desta máquina
cmd-locale-system-zone = Escolher o fuso horário desta máquina
locale-not-a-date = Isso não é uma data nesse formato.
locale-not-a-time = Isso não é uma hora nesse formato.
locale-clock-refused = A máquina recusou a mudança; veja: journalctl -u systemd-timedated
locale-heading-glance = A linha do relance
locale-glance-does = A linha que { $keys } abre acima da modeline. Cada lugar abaixo desenha a hora de lá, e a data, a temperatura e o câmbio que você ligar para ele.
locale-add-place-where = Um fuso, como Europe/Lisbon. Ele nasce mostrando o relógio e nada mais.
locale-no-places = Nenhum lugar é observado; o { $keys } não mostraria nada.
locale-shows-clock = Hora
locale-shows-date = Data
locale-shows-weather = Tempo
locale-shows-rate = Câmbio
locale-on = aparece
locale-off = escondido
locale-heading-sources = Quem é perguntado
locale-asked = perguntado
locale-not-asked = não perguntado
locale-sources-are-asked = Cada busca conta a esse host o endereço desta máquina e quais lugares ela observa. Nada aqui precisa de conta nem de chave.
locale-source-weather = O tempo
locale-source-rate = Câmbio
# Os hosts que uma fonte pergunta, na ordem em que ela pergunta: o segundo só
# é perguntado quando o primeiro não responde.
locale-source-hosts = Pergunta { $hosts }, nessa ordem.
locale-source-answered = Última resposta { $when }, de { $host }.
locale-source-never = Ninguém respondeu ainda.
locale-source-trouble = A última tentativa falhou — { $said }
locale-title = Idioma e região
locale-up = cima
locale-down = baixo
locale-enter = escolher
locale-language = Idioma
locale-clock = Relógio
locale-date = Data
locale-zone-is-yours = Você escolheu este fuso; o da máquina fica na configuração do sistema.
locale-zone-is-the-machines = O fuso da máquina. Escolher um aqui muda o que esta interface mostra, não o que o sistema roda.
locale-heading-numbers = Números e unidades
locale-decimal-language = { $mark } — o do idioma
locale-decimal-point = { $mark } — ponto, seja qual for o idioma
locale-decimal-comma = { $mark } — vírgula, seja qual for o idioma
locale-units-metric = Métrico — °C
locale-units-imperial = Imperial — °F
locale-drop-place = Remover este lugar
locale-add-place = Acrescentar um lugar
locale-add-place-how = um fuso, como Europe/Lisbon
ask-locale-zone = Fuso do relógio desta interface, como America/Sao_Paulo
ask-locale-place = Fuso do lugar a acrescentar, como Europe/Lisbon
ask-locale-where = Latitude e longitude, dois números
locale-no-such-zone = Não existe fuso com esse nome.
locale-not-a-coordinate = Isso não é uma latitude e uma longitude.
locale-no-place-here = Ponha o cursor num dos lugares do relance primeiro.
locale-place-gone = Esse lugar não está mais no relance.
cmd-locale-less = Valor anterior
cmd-locale-more = Próximo valor
cmd-locale-zone = Escolher o fuso em que esta interface mostra as horas
cmd-locale-place = Acrescentar um lugar ao relance
cmd-locale-where = Dizer onde fica um lugar
locale-less = anterior
locale-more = próximo
# THE TWO LETTERS, and they had no short text at all: the hint line read
# `[locale-place]` and `[locale-where]` on screen, which is the missing-key
# marker doing duty as a word. `cmd-locale-place` has been there all along —
# it is the LONG form, and the line reads the short one.
locale-place = acrescentar lugar
locale-where = onde fica
world-temperature-f = { $degrees } °F

# =====================================================================
# SOM — `LEADER s a`, duas portas, e as marcas que a modeline desenha
# =====================================================================
group-audio = som
audio-title = Som

cmd-audio-open = Som, no rodapé
cmd-audio-window = Som, em janela
cmd-audio-up = Linha anterior
cmd-audio-down = Próxima linha
cmd-audio-enter = Usar este
cmd-audio-less = Mais baixo, ou o valor anterior
cmd-audio-more = Mais alto, ou o próximo valor
cmd-audio-mute = Silenciar, ou devolver o som
cmd-audio-default = Tocar por este de agora em diante

audio-up = subir
audio-down = descer
audio-enter = usar
audio-less = baixar
audio-more = aumentar
audio-mute = silenciar
audio-default = usar sempre

# Os títulos, na ordem em que se lê.
audio-heading-output = Tocando por
audio-heading-input = Ouvindo por
audio-heading-apps = Programas
audio-heading-card = A placa
audio-heading-settings = Ajustes

audio-chosen-elsewhere = Escolhido: { $device }, que não está aqui
audio-nothing-plays = Não há por onde tocar.
audio-nothing-listens = Não há microfone.
audio-nothing-runs = Nenhum programa está tocando nem gravando.
audio-no-pipewire = O servidor de som não responde.
audio-muted = mudo
audio-in-use = em uso

audio-profile = O que a placa está fazendo
audio-socket-out = Sai pela
audio-socket-in = Grava pela
audio-port-none = Nenhuma tomada em uso.
audio-unplugged = nada conectado
audio-card-off = Esta placa está desligada.

audio-listening-now = ouvindo agora

audio-setting-step = Uma tecla move
audio-setting-over = Permitir acima do cheio
audio-setting-flash = Mostrar a linha acima da modeline
audio-on = sim
audio-off = não
audio-step-points = { $points }%

audio-media-none = Este build não tem tocador de mídia para conversar.

# As tomadas para as quais temos palavra própria. O resto sai nas do PipeWire.
audio-port-speaker = Alto-falantes
audio-port-headphones = Fones de ouvido
audio-port-headset = Fone com microfone
audio-port-headset-mic = Microfone do fone
audio-port-internal-mic = Microfone interno
audio-port-microphone = Microfone
audio-port-line-in = Entrada de linha
audio-port-line-out = Saída de linha

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

echo-audio-refused = O servidor de som { $why } ({ $words })
audio-refused-gone = não tem mais esse aparelho
audio-refused-absent = não está respondendo
audio-refused-invalid = não aceitou esse valor
audio-refused-other = recusou
help-media = As teclas do próprio teclado

# As duas janelas que não são buffers wasm e chegam à mesma modeline.
# O `daemon::mode_label` transforma `br.ronoctua.<id>` em `mode-<id>` seja o
# que for a janela; sem estas a modeline desenhava `[mode-audio]`.
mode-audio = som
mode-syncthing = syncthing
mode-pointers = ponteiros
mode-locale = idioma/região

# O ATUIN — O HISTÓRICO DO SHELL, E SE ELE SAI DESTA MÁQUINA.
#
# O `nix/modules/home.nix` recusava a sincronização por escrito, e o dono pediu
# a outra resposta. O que não mudou é o comando com token no argv, e é por isso
# que o `secrets_filter` e o `history_filter` são linhas DESENHADAS desta vista
# em vez de padrões que ninguém lê.
atuin-title = Histórico do shell
atuin-heading-glance = Num relance
atuin-heading-account = A conta
atuin-heading-leaving = O que sai desta máquina
atuin-heading-search = A busca
atuin-heading-keys = As teclas
atuin-heading-advanced = Raramente mexido
atuin-advanced-about = Estes mudam como a busca se comporta. Os padrões são o que a maioria quer.
atuin-absent = o atuin não está instalado nesta máquina
atuin-not-logged-in = sem conta
atuin-state = o histórico
atuin-on = ligado
atuin-off = desligado
atuin-syncing = sincronizando
atuin-not-syncing = fica nesta máquina
atuin-server = servidor
atuin-account = conta
atuin-entries = entradas
atuin-last-sync = última sincronização
atuin-mark = marca na modeline
atuin-mark-never = nunca
atuin-mark-when-off = quando não estiver sincronizando
atuin-mark-when-on = enquanto estiver sincronizando
atuin-never = ainda não
atuin-not-set = não definido
atuin-sync-now = sincronizar agora
atuin-sync = sincronizar agora
atuin-log-in = entrar num servidor
atuin-log-out = sair e esquecer a chave
atuin-show-key = mostrar a chave de criptografia
atuin-key-about = Esta chave é o que decifra o seu histórico. O servidor nunca a vê, e uma segunda máquina não sincroniza sem ela.
atuin-key-ask = Mostrar a chave na tela?
atuin-from-flake = definido pela configuração do sistema
atuin-down = próxima
atuin-up = anterior
atuin-enter = mudar
atuin-more = mais
atuin-less = menos
atuin-back = voltar
atuin-drawer = a gaveta
cmd-atuin-open = Histórico do shell
cmd-atuin-window = Histórico do shell numa janela
cmd-atuin-sync = Sincronizar o histórico do shell agora
cmd-atuin-down = Próxima linha
cmd-atuin-up = Linha anterior
cmd-atuin-enter = Mudar este ajuste
cmd-atuin-more = Girar este ajuste para a frente
cmd-atuin-less = Girar este ajuste para trás
cmd-atuin-back = Sair do histórico do shell
ask-atuin-server = O endereço do servidor de sincronização
ask-atuin-account = A conta nesse servidor
ask-atuin-password = A senha dessa conta
ask-atuin-key = A chave da sua primeira máquina — vazio se esta for a primeira
ask-atuin-setting = O valor novo
echo-atuin-synced = O histórico foi sincronizado
echo-atuin-refused = o atuin disse: { $reason }
echo-atuin-logged-out = Saiu, e a chave foi esquecida
group-atuin = histórico

# The table, from `assets/atuin-options.nix` through the generator.
atuin-opt-accept-past-line-end = deixar o cursor passar do fim da linha
atuin-opt-auto-sync = sincronizar sozinho
atuin-opt-command-chaining = ler `a && b` como dois comandos
atuin-opt-common-prefix = prefixos para ignorar
atuin-opt-common-subcommands = subcomandos a manter
atuin-opt-ctrl-n-shortcuts = Ctrl+1..9 escolhem um resultado
atuin-opt-cwd-filter = pastas nunca gravadas
atuin-opt-daemon-enabled = rodar o daemon em segundo plano
atuin-opt-daemon-sync-frequency = o daemon sincroniza a cada
atuin-opt-enter-accept = Enter executa, em vez de editar
atuin-opt-exit-mode = sair da busca devolve
atuin-opt-filter-mode = buscar em
atuin-opt-history-filter = comandos nunca gravados
atuin-opt-ignored-commands = comandos a descartar
atuin-opt-inline-height = altura da busca
atuin-opt-invert = desenhar a busca de cabeça para baixo
atuin-opt-keymap-mode = as teclas da busca
atuin-opt-local-timeout = desistir do banco local após
atuin-opt-max-preview-height = altura da prévia
atuin-opt-network-connect-timeout = desistir de conectar após
atuin-opt-network-timeout = desistir do servidor após
atuin-opt-prefers-reduced-motion = menos movimento
atuin-opt-prefix-key = a tecla de prefixo da busca
atuin-opt-scroll-context-lines = linhas mantidas ao redor do cursor
atuin-opt-scroll-exits = rolar além do fim sai
atuin-opt-search-mode = como a consulta casa
atuin-opt-search-style = como a busca é desenhada
atuin-opt-secrets-filter = nunca gravar comando com cara de segredo
atuin-opt-show-help = mostrar as dicas de tecla
atuin-opt-show-preview = mostrar o comando inteiro
atuin-opt-show-tabs = mostrar as abas
atuin-opt-store-failed = gravar comandos que falharam
atuin-opt-strip-trailing-whitespace = tirar espaços no fim
atuin-opt-sync-frequency = sincronizar a cada
atuin-opt-sync-records = usar o armazenamento de registros
atuin-opt-up-key-filter = a seta para cima busca em
atuin-opt-update-check = procurar versão nova
atuin-opt-word-chars = o que conta como palavra
atuin-opt-word-jump-mode = como o salto de palavra funciona
atuin-opt-workspaces = tratar um repositório git como espaço de trabalho
atuin-unit-lines = linhas
atuin-unit-seconds = segundos
atuin-word-auto = como o atuin decidir
atuin-word-compact = compacta
atuin-word-directory = esta pasta
atuin-word-emacs = emacs
atuin-word-full = cheia
atuin-word-fulltext = em qualquer lugar da linha
atuin-word-fuzzy = aproximada
atuin-word-global = tudo
atuin-word-host = esta máquina
atuin-word-prefix = o começo da linha
atuin-word-return-original = o que você tinha digitado
atuin-word-return-query = o que você buscou
atuin-word-session = esta sessão
atuin-word-skim = skim
atuin-word-subl = sublime
atuin-word-vim-insert = vim, inserção
atuin-word-vim-normal = vim, normal
atuin-word-workspace = este projeto

# ---------------------------------------------------------------------------
# WHAT A SHORTCUT DOES, IN FULL — the line under the hint.
#
# The hint line teaches forty keys in three words each, which is all a line
# holds. Resting the pointer on one of them opens a second line underneath with
# the whole sentence, and this is where those sentences live.
#
# TWO NAMESPACES, AND THE OTHER ONE IS `cmd-`. A key that is a command of the
# registry already has its long form there — that is what which-key and the
# manual read, and a hundred of these ids have one. What is left are the keys
# that belong to a VIEW and to no registry: the file manager's, the transfer
# panel's, and one of the notification manager's. Writing `cmd-files-preview`
# would say that id is a registered command, which is exactly what `cmd-` means.
#
# `every-hint-key-explains-itself` refuses an id with neither and an id with
# both — two sentences for one key is two places to keep in step.
#
# THE FILE MANAGER'S LISTING.
about-files-up = Move o cursor uma linha para cima, na seção que estiver com o teclado.
about-files-down = Move o cursor uma linha para baixo, na seção que estiver com o teclado.
about-files-first = Leva o cursor para a primeira linha da listagem.
about-files-last = Leva o cursor para a última linha da listagem.
about-files-parent = Sai desta pasta para a que a contém.
about-files-enter = Abre o que está sob o cursor: entra na pasta, ou abre o arquivo no programa que o lê.
about-files-menu = Abre a lista de tudo o que se pode fazer com o que está sob o cursor, cada linha mostrando a tecla que a executa.
about-files-mark = Marca a linha sob o cursor, ou tira a marca dela — o que está marcado é sobre o que a próxima ação age.
about-files-mark-all = Marca de uma vez todas as entradas desta pasta.
about-files-unmark-all = Tira a marca de todas as entradas desta pasta.
about-files-columns = Mostra ou esconde as permissões, o dono, o grupo e a data de modificação em cada linha da listagem.
about-files-header = Mostra ou esconde a linha que nomeia as colunas da listagem e carrega a seta que diz por qual delas ela está ordenada. Nasce escondida.
about-files-sort = Muda por qual campo a listagem é ordenada: o nome, o tamanho, a data ou o tipo.
about-files-sort-reverse = Inverte a ordem, do último para o primeiro.
about-files-measure = Soma o que uma pasta guarda em disco, percorrendo-a.
about-files-pane-next = Dá o teclado à próxima seção — a listagem, a barra lateral, a prévia e as informações, em anel.
about-files-pane-previous = Dá o teclado à seção anterior a esta.
# THE TWO COLUMNS ON THE RIGHT, and the sidebar on the left.
about-files-preview = Mostra ou esconde a coluna de prévia — a imagem, o filme, o texto ou a forma que o arquivo sob o cursor guarda.
about-files-facts = Mostra ou esconde a coluna de informações sobre o arquivo sob o cursor: o tamanho, as datas, o tipo e o que o próprio arquivo carrega.
about-files-facts-scroll = Vira para a próxima página da coluna de informações, e volta ao topo ao chegar ao fim.
about-files-sidebar = Mostra ou esconde a coluna de lugares — as suas dez pastas, e os discos que estiverem conectados.
about-files-eject = Desmonta o disco removível desta linha, para que ele possa ser retirado com segurança.
about-files-unpin = Tira este lugar da barra lateral. Um que a distro traz volta oculto; um seu, some.
# THE PREVIEW: the text, the film and the shape.
about-files-preview-scroll = Vira para a próxima página da prévia, e volta ao topo ao chegar ao fim.
about-files-preview-left = Desliza o texto da prévia para o lado, em direção ao começo das linhas longas.
about-files-preview-right = Desliza o texto da prévia para o lado, em direção ao fim das linhas longas.
about-files-theatre = Dá à prévia a região inteira, e devolve a região. Sem nada selecionado não há prévia para expandir.
about-files-facts-wide = Dá à ficha a região inteira, e devolve a região. Expandida, ela é disposta em quantas colunas a largura couber, então cada valor aparece mais por inteiro.
about-files-grid-next = Vai para o próximo arquivo da prateleira. Com os itens lado a lado, as setas laterais andam por eles; o TAB continua atravessando para a próxima seção.
about-files-grid-previous = Vai para o arquivo anterior da prateleira. Com os itens lado a lado, as setas laterais andam por eles; o Shift TAB continua voltando uma seção.
about-files-grid = Mostra os arquivos lado a lado, como figuras, em vez de um por linha. O glifo no fim da linha do caminho faz o mesmo, e desenha para qual forma ele levaria. Nasce desligado.
about-files-grid-size = Gira entre os três tamanhos em que uma miniatura é desenhada. Numa tela que caberia mais figuras do que o gerenciador desenha de uma vez, o degrau menor é o que fica de fora.
about-files-play = Toca o que a prévia está mostrando, ou pausa onde está.
about-files-rewind = Volta um trecho no que está tocando.
about-files-forward = Avança um trecho no que está tocando.
about-files-sound = Dá som ao filme, ou tira. Um filme sem trilha não tem som para dar.
about-files-playlist-previous = Mostra a entrada anterior a esta na lista de reprodução.
about-files-playlist-next = Mostra a próxima entrada desta lista de reprodução.
about-globe-north = Gira o globo para o norte, na direção do polo de cima.
about-globe-south = Gira o globo para o sul, na direção do polo de baixo.
about-globe-east = Gira o globo para leste, trazendo à vista a terra que está à direita.
about-globe-west = Gira o globo para oeste, trazendo à vista a terra que está à esquerda.
about-files-turn-left = Gira a forma para a esquerda, em torno do eixo vertical.
about-files-turn-right = Gira a forma para a direita, em torno do eixo vertical.
about-files-turn-up = Inclina a forma para longe de você, para vê-la por baixo.
about-files-turn-down = Inclina a forma em sua direção, para vê-la por cima.
about-files-zoom-in = Aproxima a forma.
about-files-zoom-out = Afasta a forma.
about-files-reframe = Devolve a forma ao centro, no tamanho em que começou.
# THE TRANSFER QUEUE.
about-transfers-up = Move o cursor uma linha para cima na fila.
about-transfers-down = Move o cursor uma linha para baixo na fila.
about-transfers-pause = Segura esta transferência onde está, ou deixa que ela siga.
about-transfers-cancel = Pergunta se é para desistir desta transferência: o que já foi escrito seria apagado.
about-transfers-detail = Abre esta transferência, um campo por linha: quem a move, quem pediu, o problema inteiro. Apertado lá, volta à lista.
about-transfers-answer = Fica com a resposta em que o cursor está.
about-transfers-forget = Tira esta linha da lista. O arquivo que ela escreveu fica onde está.
about-transfers-sweep = Tira da lista, de uma vez, todas as linhas que já acabaram.
about-transfers-launch = Abre o programa que esta linha instalou, pela entrada que ela pôs no lançador.
# THE NOTIFICATION MANAGER, where one row means something else.
about-notif-edit = Abre este filtro seu para ser reescrito: o nome, as condições e o que ele faz.

# =====================================================================
# THE LINE THAT OUTLIVES THE FOOTER — when the interface itself falls
# =====================================================================
#
# The owner moved the pointer across the hint line and everything closed. He
# named two defects and only one of them was the crash: *"o outro é o erro de
# que a ronoctua-distro não sabe lidar com isso, ou seja, acaba deixando o
# usuário sem interface"*. These are the words of the answer to the second one.
#
# It is a second layer-shell row raised by systemd's `OnFailure=`, and it is not
# a second chrome (invariant 14): it is the footer's own row drawn by another
# process, alive exactly while there is no chrome at all.
lifeline-fell = A interface fechou sozinha.
lifeline-fell-and-came-back = A interface fechou sozinha e voltou.
# THE UNIT'S OWN WORDS, and they are DATA — what the machine said, the way a
# filename is. `{ $why }` is systemd's `Result=`: `core-dump`, `signal`,
# `oom-kill`, `exit-code`.
lifeline-why = { $unit } · { $why }
# AND THE UNIT ALONE, when systemd has nothing to say about it — a
# transient unit is gone by the time the line asks. A dot between one
# thing and nothing separates nothing.
lifeline-unit = { $unit }
lifeline-logs = ver os logs
lifeline-restart = reiniciar a interface
lifeline-ai = perguntar a uma IA
lifeline-close = fechar
lifeline-logs-opened = o journal está aberto num terminal
lifeline-restarting = pedi ao systemd para subir de novo
lifeline-would-not-start = o systemd não subiu; os logs dizem por quê
lifeline-no-journalctl = o journal não abriu; o journalctl não está nesta máquina
# THE NESTED LOOP HAS NO UNIT. `nix/footer-single.nix` starts the footer as an
# ordinary background child, so there is nothing for systemd to restart — and
# saying so beats answering about a unit it has never heard of.
lifeline-not-a-unit = aqui não há unidade do systemd; o laço aninhado sobe o rodapé à mão

# =====================================================================
# POINTERS — `LEADER s i`: mice, touchpads and every cursor
# =====================================================================
group-pointers = ponteiros
pointers-title = Mouses e touchpads

cmd-pointers-open = Mouses e touchpads, no rodapé
cmd-pointers-window = Mouses e touchpads, numa janela
cmd-pointers-up = Linha anterior
cmd-pointers-down = Próxima linha
cmd-pointers-enter = Mudar isto, ou usar
cmd-pointers-less = O valor anterior
cmd-pointers-more = O próximo valor
cmd-pointers-forget = Esquecer este dispositivo

pointers-up = sobe
pointers-down = desce
pointers-enter = muda
pointers-less = anterior
pointers-more = próximo
pointers-forget = esquece

pointers-heading-cursors = Cursores
pointers-heading-mice = Mouses
pointers-heading-touchpads = Touchpads
pointers-heading-general = Todos os cursores
pointers-heading-away = Desconectados

pointers-about-cursors = Cada mouse e cada touchpad pode mover um cursor próprio, cada um com a sua cor, cada um focando a sua janela. Ponha dois dispositivos num cursor só para que o movam juntos.
pointers-about-mice = O que se ajusta em “Todo mouse” vale para cada mouse que não ajusta aquilo para si.
pointers-about-touchpads = O que se ajusta em “Todo touchpad” vale para cada touchpad que não ajusta aquilo para si.
pointers-about-general = O foco segue o ponteiro para todos os cursores ao mesmo tempo: o compositor tem um ajuste só para todos eles.
pointers-about-away = Dispositivos já vistos, guardados com o cursor e os ajustes deles para quando voltarem.
pointers-about-twins = Dois dispositivos do mesmo modelo têm o mesmo nome para o compositor, então sempre movem um cursor juntos.

pointers-detect = Dispositivo novo ganha cursor próprio
pointers-together = Todo mouse e touchpad move um cursor só
pointers-own-cursor = um cursor próprio
pointers-emulated = Os ponteiros da própria máquina virtual se movem
pointers-about-emulated = O tablet e os mouses da máquina virtual trazem o ponteiro do hospedeiro para dentro. Com os mouses do hospedeiro entregues a esta máquina eles ficam desligados, para nenhum cursor ficar parado; ligue-os para levar o ponteiro para fora da janela de novo.
pointers-not-drawn = não desenhado: nada nele se move
pointers-cursor = Cursor { $number }
pointers-cursor-on = Este cursor está ligado
pointers-colour = Cor
pointers-colour-value = cor { $number }
pointers-on-cursor = Move
pointers-on-cursor-value = o cursor { $number }
pointers-new-cursor = um cursor próprio
pointers-every-mouse = Todo mouse
pointers-every-touchpad = Todo touchpad
pointers-device-on = Este dispositivo está ligado
pointers-no-devices = Nenhum mouse ou touchpad está conectado.
pointers-no-compositor = O compositor não está respondendo, então nenhum dispositivo pode ser lido ou ajustado.
pointers-settings-failed = O pointers.json não pôde ser lido e foi deixado como está: as escolhas mostradas são as de fábrica até ele ser consertado ou removido.

pointers-setting-events = Desligado com um mouse conectado
pointers-setting-speed = Velocidade
pointers-setting-profile = Aceleração
pointers-setting-natural = Rolagem natural
pointers-setting-left = Canhoto
pointers-setting-tap = Tocar para clicar
pointers-setting-tap-map = Toque com dois e três dedos
pointers-setting-tap-drag = Tocar e arrastar
pointers-setting-drag-lock = O arrasto sobrevive a levantar o dedo
pointers-setting-finger-drag = Arrastar com os dedos
pointers-setting-click = Apertar clica com
pointers-setting-click-map = Aperto com dois e três dedos
pointers-setting-middle = Esquerdo e direito juntos são o botão do meio
pointers-setting-scroll = Rola com
pointers-setting-scroll-button = Botão de rolar
pointers-setting-scroll-lock = O botão de rolar fica apertado
pointers-setting-dwt = Desligado enquanto se digita
pointers-setting-dwtp = Desligado enquanto se usa o trackpoint
pointers-setting-factor = Distância da rolagem

pointers-size = Tamanho
pointers-size-value = { $pixels } px
pointers-hide-after = Esconder parado por
pointers-never = nunca
pointers-seconds = { $seconds } s
pointers-hide-typing = Esconder enquanto se digita
pointers-focus = O foco segue o ponteiro
pointers-focus-no = não, um clique foca
pointers-focus-yes = sim
pointers-focus-always = sempre
pointers-about-typing = A borda colorida marca a janela em que os teclados digitam, e só o clique do cursor 1 a move. Escolha “o último cursor a escolher uma janela” e o clique de todo cursor a move — e a digitação junto: o compositor desenha a borda só para o foco dos teclados.
pointers-typing = A digitação vai para
pointers-typing-first = a janela do cursor 1
pointers-typing-cursor = a janela do cursor { $number }
pointers-typing-latest = o último cursor a escolher uma janela
pointers-heading-keyboards = Teclados
pointers-about-keyboards = No modo Independentes cada teclado digita por um cursor próprio, com o seu layout e a sua tecla leader. Isso se ajusta na tela dos teclados.
pointers-open-keyboards = Abrir os ajustes dos teclados

pointers-on = ligado
pointers-off = desligado
pointers-flat = constante
pointers-adaptive = mais rápido com a mão mais rápida
pointers-lrm = direito, depois o do meio
pointers-lmr = o do meio, depois o direito
pointers-lock-off = não
pointers-lock-on = por um instante
pointers-lock-sticky = até o próximo toque
pointers-fingers-off = desligado
pointers-fingers-three = três dedos
pointers-fingers-four = quatro dedos
pointers-click-none = sem botões
pointers-click-areas = onde se aperta
pointers-click-fingers = quantos dedos apertam
pointers-scroll-none = nada
pointers-scroll-two = dois dedos
pointers-scroll-edge = a borda
pointers-scroll-button = um botão segurado
pointers-button-none = nenhum
pointers-button-middle = o botão do meio
pointers-button-side = o botão lateral
pointers-button-extra = o botão extra
pointers-factor-value = × { $factor }
pointers-speed-value = { $speed }
pointers-is-first = o cursor em que os teclados estão

# =====================================================================
# LEADER d — development: tools for whoever builds this distro.
# =====================================================================
# The screen resizer (`LEADER / r`, `resizer.rs`): two curtains cut the
# screen, and everything is laid out in the room they leave. Its line on the
# bottom curtain says the room in pixels and in the grid, and each key's word
# is short because the line is one line; `about-` is what the pointer reads.
group-dev = desenvolvimento
cmd-dev-resizer = Redimensionar a tela inteira
hint-resizer = A tela está cortada em { $width } × { $height } px — { $columns } colunas por { $lines } linhas. Clique para tê-la inteira de novo.
echo-resizer-on = A tela pode ser redimensionada: arraste a borda da direita, a de baixo ou o canto, ou clique numa cortina e use as teclas que a linha dela ensina.
echo-resizer-off = A tela está inteira de novo.
echo-resizer-failed = Não deu para redimensionar a tela: { $why }
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

# =====================================================================
# THE CAMERA STUDIO — `LEADER m c` (the app) and `LEADER s c` (settings)
# =====================================================================
group-camera = câmera
mode-camera = câmera
camera-title = Câmera
camera-settings-title = Ajustes da câmera

cmd-camera-open = A câmera, numa janela
cmd-camera-settings-open = Ajustes da câmera, no rodapé
cmd-camera-settings-window = Ajustes da câmera, numa janela
cmd-camera-settings-up = Linha anterior
cmd-camera-settings-down = Próxima linha
cmd-camera-settings-less = O valor anterior
cmd-camera-settings-more = O próximo valor
cmd-camera-settings-enter = Mudar isto, ou usar
cmd-camera-settings-remove = Remover esta cena ou este roteiro
cmd-camera-photo = Tirar uma foto do que a câmera mostra
cmd-camera-record = Começar ou parar de gravar
cmd-camera-effect-next = O próximo efeito na câmera
cmd-camera-effect-previous = O efeito anterior na câmera
cmd-camera-background-next = O próximo fundo: desfocado, trocado, removido ou como está
cmd-camera-param-less = Menos do efeito
cmd-camera-param-more = Mais do efeito
cmd-camera-camera-next = Mostrar a próxima câmera
cmd-camera-mirror = Mostrar a câmera espelhada, ou não
cmd-camera-scene-next = Pôr a próxima cena no ar
cmd-camera-scene-previous = Pôr a cena anterior no ar
cmd-camera-script = Rodar o primeiro roteiro, ou parar o que está rodando
cmd-camera-next-cue = A próxima cena do roteiro que está rodando
cmd-camera-virtual = Oferecer a imagem aos outros programas como câmera, ou parar
cmd-camera-stream = Começar ou parar a transmissão ao vivo
cmd-camera-scene-add = Uma cena nova, cópia da que está no ar
cmd-camera-scene-rename = Renomear a cena no ar
cmd-camera-script-add = Um roteiro novo de cenas
cmd-camera-stream-url = O endereço para onde a transmissão vai
cmd-camera-stream-key = A chave da transmissão
ask-camera-scene-add = Nome da cena nova
ask-camera-scene-rename = Novo nome da cena no ar
ask-camera-script-add = Roteiro — nome: cena segundos, cena segundos, … (acrescente “loop” para repetir)
ask-camera-stream-url = Endereço da transmissão (rtmp://… ou srt://…)
ask-camera-stream-key = Chave da transmissão

camera-settings-up = sobe
camera-settings-down = desce
camera-settings-less = anterior
camera-settings-more = próximo
camera-settings-enter = muda
camera-settings-remove = remove
camera-photo = foto
camera-record = grava
camera-effect-next = efeito
camera-effect-previous = efeito anterior
camera-background-next = fundo
camera-param-less = menos
camera-param-more = mais
camera-camera-next = câmera
camera-mirror = espelho
camera-scene-next = cena
camera-scene-previous = cena anterior
camera-script = roteiro
camera-next-cue = próxima deixa
camera-virtual = câmera virtual
camera-stream = transmite
camera-menu-effect = escolher efeito
camera-menu-background = escolher fundo
camera-menu-camera = escolher câmera
camera-menu-scene = escolher cena
camera-mixer = som
camera-fit = caber ou preencher
camera-pin = fixa
about-camera-menu-effect = Abre a lista de efeitos, para escolher um direto.
about-camera-menu-background = Abre a lista de fundos, para escolher um direto.
about-camera-menu-camera = Abre a lista de câmeras, para escolher uma direto.
about-camera-menu-scene = Abre a lista de cenas, para escolher uma direto.
camera-button-menu = clique: escolher na lista · clique direito: o próximo
about-camera-mixer = Abrir ou fechar o som: os níveis e os volumes do microfone e da máquina.
about-camera-fit = Mostrar a imagem inteira com faixas, ou preencher a janela cortando a imagem.
about-camera-pin = Manter os controles sobre a imagem, ou deixá-los sumir quando ninguém os usa.

camera-effect-none = nenhum
camera-effect-grey = Cinza
camera-effect-sepia = Sépia
camera-effect-negative = Negativo
camera-effect-posterize = Posterizado
camera-effect-thermal = Térmico
camera-effect-night = Visão noturna
camera-effect-tone = Tom
camera-effect-pixelate = Pixelado
camera-effect-blur = Desfoque
camera-effect-sketch = Esboço
camera-effect-vignette = Vinheta
camera-effect-symmetry = Simetria
camera-effect-kaleidoscope = Caleidoscópio
camera-effect-chroma-key = Tela verde
camera-effect-background-blur = Fundo desfocado
camera-effect-background-replace = Fundo trocado
camera-effect-background-remove = Sem fundo
camera-param-amount = Intensidade
camera-param-levels = Níveis
camera-param-gain = Ganho
camera-param-brightness = Brilho
camera-param-contrast = Contraste
camera-param-saturation = Saturação
camera-param-temperature = Calor
camera-param-size = Tamanho do bloco
camera-param-radius = Raio
camera-param-strength = Força
camera-param-segments = Segmentos
camera-param-hue = Cor da tela
camera-param-tolerance = Tolerância
camera-param-steady = Firmeza da borda

studio-scene-camera = Câmera
studio-scene-screen = Tela
studio-scene-screen-and-camera = Tela e câmera

studio-trouble-no-camera = Nenhuma câmera está conectada.
studio-trouble-camera-busy = A câmera está sendo usada por { $program }.
studio-trouble-camera-silent = A câmera parou de mandar imagens; ela será tentada de novo.
studio-trouble-no-model = O modelo que encontra a pessoa não está instalado, então o fundo não pode ser desfocado, trocado nem removido.
studio-trouble-no-hand-model = Os modelos que encontram as mãos não estão instalados, então os gestos não podem ser lidos.
studio-trouble-no-backdrop = Nenhuma imagem foi escolhida para o fundo trocado (Ajustes da câmera, Efeitos).
studio-trouble-no-ffmpeg = O ffmpeg não pôde rodar, então nenhuma câmera pode ser aberta.
studio-trouble-no-screen = A tela não pode ser capturada aqui.
studio-trouble-no-window = Nenhuma janela aberta corresponde à janela desta cena.
studio-trouble-no-virtual = O dispositivo da câmera virtual não existe: o módulo do kernel não está carregado.
studio-trouble-virtual-busy = Outro programa está escrevendo na câmera virtual.
studio-trouble-encoder = A gravação não pôde continuar: { $why }
studio-trouble-stream = A transmissão parou: { $why }
studio-trouble-no-stream-target = Defina antes o endereço e a chave da transmissão (Ajustes da câmera, Transmissão).
studio-trouble-name = Esse nome já está em uso, ou é a última cena.
studio-trouble-write = Um arquivo não pôde ser escrito: { $why }
studio-not-running = O estúdio da câmera não está respondendo.
studio-photo-saved = Foto salva: { $file }
studio-recording-saved = Gravação salva: { $file }
studio-countdown = { $seconds }
studio-recording-for = Gravando { $time }
studio-streaming-for = Ao vivo { $time }
studio-level = { $percent }%
studio-muted = mudo
studio-sound-microphone = Microfone
studio-sound-desktop = O som da máquina

camera-heading-cameras = Câmeras
camera-about-cameras = Toda câmera que esta máquina vê, com os modos e os controles que o driver dela declara. O que muda aqui fica guardado para aquela câmera e volta a cada vez que ela é conectada.
camera-default = A câmera que o app abre
camera-default-first = a primeira câmera colorida
camera-infrared = infravermelho
camera-cannot-stream = não pode ser aberta
camera-about-cannot-stream = O driver desta câmera só oferece captura multiplanar, que precisa do libcamera. Câmeras de alguns laptops chegam assim; um repasse como o v4l2-relayd as transforma numa câmera que todo programa abre.
camera-mode = Modo
camera-mode-chosen = escolhido para ela
camera-mode-chosen-is = escolhido para ela: { $mode }
camera-mirrored = Mostrada espelhada
camera-shuttered = A lente está coberta pelo obturador da própria câmera.
camera-factory = Voltar todos os controles ao de fábrica

camera-heading-effects = Efeitos
camera-about-effects = O que é feito na imagem da câmera, ao vivo — no app, e nas fotos e gravações. A câmera virtual leva este efeito, nenhum ou um próprio: veja a seção dela.
camera-effect = Efeito
camera-background = Fundo
camera-backdrop = Imagem atrás de você

camera-heading-virtual = Câmera virtual
camera-about-virtual = Outros programas — uma chamada no navegador, uma reunião — podem escolher a câmera “ronoctua” e receber esta imagem, com o efeito escolhido para ela abaixo. Ela só envia enquanto está ligada.
camera-virtual-now = Ligada agora
camera-virtual-at-start = Ligada a cada início
camera-virtual-sends = Ela envia
camera-sends-program = a cena no ar
camera-sends-camera = só a câmera

camera-heading-capture = Fotos e gravações
camera-about-capture = Uma gravação é escrita enquanto acontece e sobrevive a uma queda; com MP4 escolhido, ela vira MP4 quando termina.
camera-size = Tamanho
camera-rate = Quadros por segundo
camera-fps = { $fps } por segundo
camera-container = Gravações são salvas como
camera-container-mp4 = MP4
camera-container-mkv = Matroska
camera-encoder = Codificadas pela
camera-encoder-auto = placa de vídeo, ou o processador
camera-encoder-hardware = placa de vídeo
camera-encoder-software = o processador
camera-quality = Qualidade (menor é melhor)
camera-countdown = Contagem antes da foto
camera-at-once = nenhuma
camera-seconds = { $seconds } s
camera-cursor = O ponteiro nas capturas de tela
camera-pictures = As fotos vão para
camera-videos = As gravações vão para

camera-heading-sound = Som
camera-about-sound = O microfone só é aberto enquanto algo grava ou transmite, e a marca do microfone na modeline diz isso.
camera-microphone = Gravar o microfone
camera-microphone-gain = Volume do microfone
camera-desktop = Gravar o som da máquina
camera-desktop-gain = Volume da máquina

camera-heading-scenes = Cenas
camera-about-scenes = Uma cena é o que está no ar: a câmera, a tela, uma janela, uma imagem, em camadas. Arraste uma camada pequena no app para movê-la e gire a roda sobre ela para redimensioná-la — durante uma gravação também.
camera-no-studio = O estúdio da câmera não está respondendo; cenas e roteiros aparecem quando ele responder.
camera-on-air = no ar
camera-scene-add = Cena nova
camera-scene-rename = Renomear a cena no ar

camera-heading-scripts = Roteiros
camera-about-scripts = Um roteiro põe cenas no ar em ordem, cada uma por tantos segundos ou até a “próxima deixa”.
camera-script-bad = Um roteiro se escreve “nome: cena segundos, cena segundos, …”, com cenas que existem.
camera-running = rodando
camera-script-add = Roteiro novo

camera-heading-stream = Transmissão
camera-about-stream = Uma transmissão ao vivo para qualquer serviço que aceite RTMP ou SRT. A chave fica num arquivo que só você lê, e nunca aparece na tela nem numa lista de processos.
camera-stream-url = Endereço
camera-stream-key = Chave
camera-stream-now = Ao vivo agora
camera-not-set = não definido
camera-set = definida

camera-heading-privacy = Privacidade
camera-about-privacy = A modeline mostra uma câmera sempre que algum programa tem uma câmera aberta — o nosso inclusive.
camera-mark = A marca da câmera
camera-mark-when-in-use = sempre que uma câmera está aberta
camera-mark-never = nunca

camera-heading-defaults = Padrões
camera-about-defaults = Volta todo ajuste desta tela a como veio, e desliga o efeito e o fundo. Suas cenas, o endereço da transmissão e a chave dela ficam.
camera-defaults = Voltar tudo ao padrão
camera-defaults-confirm = Mais uma vez para confirmar
camera-defaults-done = Todo ajuste voltou a como veio.
camera-heading-app = O app da câmera
camera-open-app = Abrir a câmera
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

# =====================================================================
# O PLAYER — `LEADER p`, a coluna à direita, a marca na modeline
# =====================================================================
group-player = player
mode-player = player
mode-software = programas
mode-camera-settings = ajustes da câmera
mode-manual = manual
player-title = Player

cmd-player-open = O player, no rodapé
cmd-player-window = O player, numa janela
cmd-player-column = A coluna do player, neste espaço de trabalho (ou fechá-la)
cmd-player-swap = Trocar janela e coluna
cmd-player-quit = Fechar o player em todo lugar: o que toca para, e toda janela, coluna e vista dele se fecha
cmd-player-play-file = Tocar um arquivo ou um endereço agora; a fila fica
cmd-player-enqueue = Pôr um arquivo ou um endereço na fila
cmd-player-together = Tocar outra coisa junto com o que toca: uma imagem, um som ou legendas
cmd-player-toggle = Tocar ou pausar
cmd-player-next = O próximo da fila
cmd-player-previous = O anterior da fila
cmd-player-stop = Parar
cmd-player-shuffle = Embaralhar a fila, ou parar de embaralhar
cmd-player-repeat = Repetir nada, a fila, ou este
cmd-player-up = Linha anterior
cmd-player-down = Próxima linha
cmd-player-less = O valor um passo atrás
cmd-player-more = O valor um passo à frente
cmd-player-enter = O que um clique na linha faz
cmd-player-remove = Tirar da fila, ou do que toca junto
cmd-player-browse = Escolher arquivos para a fila: música, vídeos, tudo o que o player toca
cmd-player-add = Pôr na fila o que a linha toca — uma rádio, um canal, um filme
cmd-player-play-now = Tocar agora o que a linha toca — logo depois do que toca; a fila fica
cmd-player-clear = Tirar tudo da fila
cmd-player-quieter = O volume um passo abaixo
cmd-player-louder = O volume um passo acima
cmd-player-only = Só o que toca — o vídeo ou a capa — na região, na janela ou na coluna
cmd-player-maximize = O que toca, na tela inteira — e de volta
cmd-player-pip = O que toca, numa janela pequena no canto, por cima das outras — e de volta
cmd-player-mute = Silenciar o player, ou devolver o som dele
cmd-player-tracks = Escolher entre as mídias do arquivo: os sons e as legendas dele
cmd-player-fold = Dobrar ou desdobrar a playlist ou o conjunto sob o cursor
cmd-player-earlier = Leva a entrada sob o cursor — um arquivo, uma playlist, um conjunto — um lugar para antes na lista dela
cmd-player-later = Leva a entrada sob o cursor — um arquivo, uma playlist, um conjunto — um lugar para depois na lista dela
cmd-player-also = Tocar também esta mídia do arquivo, ao lado da que está em uso: uma faixa só dela
cmd-player-rename = Outro nome para o item sob o cursor, ou para o que toca
cmd-player-save-set = Salvar o que toca como conjunto (.rset): os arquivos e como tocam juntos
cmd-player-save-queue = Salvar a fila num arquivo (.rqueue)
cmd-player-subtitles = As legendas sobre a imagem
cmd-player-lane-shown = Liga ou desliga a faixa sob o cursor: uma legenda aparece ou some, uma imagem ou um som toca ou para
cmd-player-ambience = Os sons ambientes
cmd-player-ambience-silence = Silenciar todos os sons ambientes
cmd-player-timeline = Todas as faixas do que toca, num relógio só
cmd-player-layout = Como as imagens do que toca dividem a tela
cmd-player-metadata = Os metadados do que toca: etiquetas, capa e fluxos
cmd-player-suggestions = Etiquetas para o arquivo que toca, sugeridas pela internet
cmd-player-find-subtitles = Legendas para o filme que toca, da internet
cmd-player-find-lyrics = Letras para a música que toca, da internet
cmd-player-internet = O que o player pergunta à internet, e as suas chaves
cmd-player-every-screen = O que toca na tela inteira de cada tela para onde vai; de novo, e elas fecham
cmd-player-radio = Rádios do mundo: as estações de todos os países
cmd-player-tv = TVs do mundo: os canais de todos os países
cmd-player-channels = Canais por tema: TV ao vivo, filmes e lives
cmd-player-search = Buscar pelo nome na lista desta página
cmd-player-favourite = Guardar o que está sob o cursor nos favoritos, ou tirar
cmd-player-nearer = Trazer o globo para mais perto
cmd-player-further = Levar o globo para mais longe
cmd-player-world = O globo de volta como a página o enquadra: o mundo, ou o país

# ---------------------------------------------------------------------
# THE RIGHT COLUMN'S OWN LINE (`crate::column`): its sizes, and the keys
# that change them, after its tenant's own
# ---------------------------------------------------------------------
column-wider = mais larga
column-narrower = mais estreita
column-close = fechar
about-column-wider = Um tamanho mais larga; da barra, volta ao tamanho que tinha
about-column-narrower = Um tamanho mais estreita, até a barra
about-column-large = Tamanho grande
about-column-medium = Tamanho médio
about-column-small = Tamanho pequeno
about-column-bar = Barra
about-column-close = Fechar a coluna; o que ela mostra continua
# The close glyph's own sentence, under the pointer on the control line —
# as short as the sizes', because the line is a small column's.
about-column-close-glyph = Fechar
column-bar-not-offered = Esta coluna ainda não tem nada feito para a barra
column-no-room = Esta tela não tem espaço para uma coluna mais larga
# The two refusals again, under the pointer on the control line: the room
# there is what a small column leaves beside its glyphs.
about-column-no-room = Sem espaço aqui
about-column-bar-not-made = Sem barra ainda

player-up = sobe
player-down = desce
player-less = recua
player-more = avança
player-enter = escolhe
player-toggle = tocar/pausar
player-stop = parar
player-next = próximo
player-previous = anterior
player-remove = remove
player-browse = arquivos
player-add = pôr na fila
player-play-now = tocar já
player-clear = limpar a fila
player-quieter = mais baixo
player-louder = mais alto
player-only = só a mídia
player-maximize = tela inteira
player-pip = janela pequena
player-mute = mudo
player-swap = janela/coluna
# The swap's own sentence in the footer, where it opens the window in the
# view's place — the command's says window and column.
player-swap-footer = O player numa janela, no lugar do rodapé
player-tracks = mídias
player-fold = dobrar
player-earlier = antes
player-later = depois
player-also = tocar também
player-rename = renomear
player-subtitles = legendas
player-lane-shown = liga ou desliga
player-timeline = linha do tempo
player-search = buscar
player-favourite = favorito
player-nearer = mais perto
player-further = mais longe
player-world = inteiro
player-layout = disposição
player-every-screen = cada tela

ask-player-play-file = Tocar agora (um caminho ou um endereço)
ask-player-enqueue = Pôr na fila (um caminho ou um endereço)
ask-player-together = Para tocar junto (um caminho ou um endereço)
ask-player-rename = Um nome novo (vazio: o próprio)
ask-player-save-set = O nome do conjunto
ask-player-save-queue = O nome da fila
ask-player-lane-start = Começa em (ou +1,5)
ask-player-lane-until = Toca até (ou +1,5)
ask-player-lane-title = Nome dela (vazio: o do arquivo)
ask-player-tag-title = Título
ask-player-tag-artist = Artista
ask-player-tag-album = Álbum
ask-player-tag-album-artist = Artista do álbum
ask-player-tag-track = Número da faixa
ask-player-tag-disc = Número do disco
ask-player-tag-date = Data
ask-player-tag-genre = Gênero
ask-player-tag-composer = Compositor
ask-player-tag-lyricist = Letrista
ask-player-tag-publisher = Gravadora
ask-player-tag-isrc = ISRC
ask-player-tag-show = Série
ask-player-tag-season = Temporada
ask-player-tag-episode = Episódio
ask-player-tag-director = Direção
ask-player-tag-name = Nome da etiqueta nova
ask-player-tag-value = O valor dela
ask-player-tag-value-all = O valor dela (substitui tudo o que ela tem)

player-heading-now = Agora
player-heading-media = Mídias do arquivo
player-heading-media-set = Mídias dos arquivos
player-heading-listen = Ouvir e assistir
player-heading-features = Recursos variados
# One key a page (`player_page::PageId::key`): the row that opens it says it,
# and the page is titled by it.
player-page-files = Arquivos
player-page-files-together = Algo para tocar junto
player-page-files-open = Abrir conjunto ou fila
player-page-together = Tocar junto
player-page-internet = Internet
player-page-machines = Outras máquinas
player-page-sound = Som
player-page-picture = Imagem
player-page-subtitles = Legendas
player-page-gone = O que esta página mostrava não existe mais.
# The channels by theme, one name a category: `assets/channels.nix` declares
# the ids, `nix/broadcast.nix` lists the keys, and `channels-have-text` holds
# both languages to that list.
player-channels-action = Ação
player-channels-anime = Anime
player-channels-cartoons = Desenhos
player-channels-classic = Clássicos
player-channels-comedy = Comédia
player-channels-documentary = Documentários
player-channels-films = Filmes
player-channels-kids = Infantil
player-channels-music = Música
player-channels-news = Notícias
player-channels-relax = Relaxar
player-channels-science = Ciência
player-channels-scifi-horror = Ficção científica e terror
player-channels-series = Séries
player-channels-sports = Esportes
player-channels-travel = Viagem
player-channels-general = Geral
player-channels-entertainment = Entretenimento
player-channels-culture = Cultura
player-channels-lifestyle = Estilo de vida
player-channels-cooking = Culinária
player-channels-outdoor = Ao ar livre
player-channels-auto = Carros
player-channels-business = Negócios
player-channels-weather = Previsão do tempo
player-channels-public = Televisão pública
player-channels-legislative = Parlamentos
player-channels-religious = Religião
player-channels-shop = Compras
player-channels-interactive = Interativos
# Os sons ambientes, um nome por id de `assets/ambience.nix`; um som
# sem nome nesta porta é dito como tal, nunca só pelo id.
player-ambience-small-waterfall = Pequena cachoeira
player-ambience-rain-and-thunder = Chuva e trovão
player-ambience-thunderstorm = Trovoada com chuva
player-ambience-unnamed = Um som sem nome neste player ({ $id })
player-ambience-group-water = Água
player-ambience-group-storm = Tempestades
player-ambience-group-unnamed = Sons de um tipo sem nome neste player ({ $group })
# THE AMBIENT SOUNDS' PAGE (`player_ambience`) and its credits.
player-page-ambience = Sons ambientes
player-page-ambience-credits = Créditos
player-about-ambience = Gravações de água e de tempestades que tocam junto da fila, sem parar, até serem desligadas. As teclas da fila nunca as param.
player-ambience-held = em pausa
player-ambience-hold = Pausar todos
player-ambience-silence = Silenciar todos
player-ambience-silenced = Todos os sons ambientes foram desligados.
player-ambience-count = { $count ->
        [one] { $count } som
       *[other] { $count } sons
    }
player-about-credits = Cada som aqui é uma gravação que alguém deu a todos: quem a fez, onde a publicou e sob que licença.
player-credit-by = Gravado por { $author }
player-credit-page = Publicado em { $page }
player-licence-cc0 = CC0: doado ao domínio público
player-licence-public-domain = Domínio público
player-not-running = O player não está rodando.
# Um player MAIS ANTIGO que o resto do sistema, que pode descartar o que
# não lê (`player::older`): dito, com a linha que o reinicia.
player-old-daemon = Este player é mais antigo que o resto do sistema e pode não entender o que se pede a ele. Reiniciar traz o novo; a fila fica guardada.
player-restart = Reiniciar o player
player-ask-restart = Reiniciar o player? O que toca para; a fila fica guardada.
player-restarting = Reiniciando o player…
player-pair-no-answer = A outra máquina não respondeu a tempo. Confira o nome, o código e se ela está ligada.
player-nothing = Nada carregado.
player-playing = Tocando
player-state = Estado
player-state-playing = tocando
player-state-paused = pausado
player-position = Posição
player-volume = Volume
player-muted = mudo
player-speed = Velocidade
player-repeat = Repetir
player-repeat-nothing = nada
player-repeat-queue = a fila
player-repeat-one = este
player-shuffle = Embaralhar
player-track-in-use = em uso
player-subtitles-off = Sem legendas
# THE SUBTITLES' PAGE (`player_subtitles`): the lanes over the picture, and
# where more come from; and a lane's own page (`player_lane`).
player-page-files-subtitle = Adicionar arquivo de legenda
player-page-lane = Faixa
player-about-subtitles = Até seis legendas ficam sobre a imagem ao mesmo tempo, cada uma na sua cor.
player-hidden = escondida
player-subtitle-bottom = embaixo
player-subtitle-top = em cima
player-lane-merged-plain = Com outra legenda sobre a imagem, ela veste a sua cor e perde os próprios estilos.
player-heading-beside = Ao lado do arquivo
player-heading-inside = Dentro do arquivo
player-subtitles-beside-none = Nenhum arquivo de legenda ao lado tem o nome dele.
player-subtitles-inside-none = O arquivo não traz legendas próprias.
player-subtitle-added = { $name } vai sobre a imagem.
player-subtitles-shown = { $count ->
        [one] { $count } visível
       *[other] { $count } visíveis
    }
player-lane-source = Arquivo
player-lane-on = Toca
player-lane-stack = Onde fica
player-lane-remove = Tirar
player-page-timeline = Linha do tempo
player-about-timeline = Todas as faixas do que toca, num relógio só. Arraste uma banda para mudar quando a faixa começa; um clique nela abre a página da faixa.
player-page-layout = Disposição
player-about-layout = Como as imagens do que toca dividem a tela: a forma, e o lugar de cada imagem — mostrada ou escondida, movida pelas setas no fim da linha dela. As telas de cada imagem se escolhem na página dela.
player-layout-form = Forma
player-form-auto = automática
player-form-single = uma imagem
player-form-side = lado a lado
player-form-stacked = empilhadas
player-form-grid = grade
player-form-strip = grande e faixa
player-form-corner = no canto
player-heading-layout-pictures = Imagens
player-layout-shown = mostrada
player-layout-cover = capa
player-layout-nothing = Nada do que toca tem imagem para dispor.
player-layout-no-room = sem espaço
player-layout-left-out = Uma imagem que ficaria pequena demais aqui nesta forma fica de fora; uma porta maior, ou outra forma, a mostra.
player-feature-every-screen = Mostrar em cada tela
player-every-screen-opened = O que toca ocupa { $count ->
        [one] uma tela inteira
       *[other] { $count } telas inteiras
    }; o mesmo de novo as fecha.
player-every-screen-closed = As janelas de tela inteira foram fechadas.
player-every-screen-one = Há uma tela só, e o que toca já está nela.
player-every-screen-nothing = Nada do que toca tem imagem para as telas.
player-every-screen-failed = Não foi possível abrir as janelas de tela inteira: { $why }
player-heading-lane-screens = Telas
player-lane-every-screen = Em todas as telas
player-screen-built-in = Tela embutida ({ $connector })
player-screen-named = { $description } ({ $connector })
player-screen-absent = não conectada
player-screen-last = Uma imagem vai para pelo menos uma tela; para não mostrá-la em nenhuma, esconda-a.
player-screens-unknown = Esta porta ainda não sabe o nome das telas.
player-heading-lane-sound = Saída de som
player-sink-default = Saída padrão
player-sink-headphones = fone
player-sink-speakers = alto-falantes
player-sink-bluetooth = Bluetooth
player-sink-other = outra saída
player-sink-gone = não está lá
player-sink-twin = { $name } ({ $n })
player-sinks-none = Nenhuma outra saída é conhecida ainda.
player-lane-start = Começa em
player-lane-start-refused = “{ $said }” não é um tempo: m:ss, h:mm:ss, segundos ou +/- segundos.
player-lane-until = Toca até
player-lane-until-end = o fim
player-lane-until-too-soon = Ela tem de parar depois de onde começa no arquivo ({ $from }).
player-lane-until-back = Ela volta a tocar até o fim.
player-lane-title = Nome
player-lane-over = Sobre a imagem
player-lane-over-none = nenhuma imagem toca
player-lane-volume = Volume próprio
player-lane-mute = Sem som
player-lane-picture-shown = Imagem na tela
player-lane-cover-shown = Capa na tela
about-player-lane-toggle = Liga ou desliga esta faixa
about-player-lane-name = A página desta faixa
about-player-lane-band = Arraste muda o início; um clique abre a página dela
about-player-deck-seek = Clique ou arraste para percorrer o que toca
about-player-playhead = Onde o que toca está agora: arraste para percorrer
about-player-lane-mute = Silencia esta faixa, ou devolve o som dela
about-player-lane-volume = O volume desta faixa: clique ou arraste, ou gire a roda sobre a linha dela
about-player-deck-volume = O volume do player: clique ou arraste, ou gire a roda
player-lane-about = { $name }: { $about }
player-lane-twin = { $name } · { $kind ->
        [video] imagem
        [audio] som
       *[subtitle] legenda
    }
player-tracks-none = Sem mídias a escolher aqui.
# One of the file's media that another lane plays — added to play too.
player-track-too = também toca
player-track-plays-already = Essa já está tocando.
player-also-added = { $name } também toca.
# A window opened for the picture alone (`player_stage`) has no rows to
# bring back: a key that needs them says where they are.
player-stage-has-no-rows = Aqui só há a imagem; as linhas do player estão no rodapé, numa janela ou na coluna.
player-panel-no-room = Sem espaço para a imagem aqui; a tecla { $keys } a mostra sozinha.
player-heading-lyrics = Letra
player-lyrics-source = Origem
player-lyrics-from-file = arquivo .lrc
player-lyrics-from-tag = tags do arquivo
player-lyrics-from-host = { $host }
player-heading-queue = Fila
player-queue-empty = A fila está vazia.
# Um item que o player ainda lê da rede: aparece, e ainda não toca.
player-resolving = { $name } — lendo…
# O que junta as fontes de um conjunto sem nome, na fila e numa dica:
# entre aspas, para os espaços em volta ficarem.
player-set-joiner = {" + "}
player-about-together = O que toca, faixa por faixa num relógio só: as imagens, os sons e as legendas, cada um do seu arquivo.
# What a lane of what plays is doing, beside its glyph and its name.
player-lane-apart = desvio de { $ms } ms
player-lane-pending = aguardando
player-lane-ended = terminou
player-lane-preparing = preparando
player-lane-refused = não toca
player-lane-switched-off = desligada
# Uma faixa do que toca: de que tipo ela é — duas faixas de um arquivo têm
# um nome só — e a que distância do relógio, ou que está desligada.
player-together-lanes = { $count ->
        [one] uma faixa
       *[other] { $count } faixas
    }
player-together-added = { $name } toca junto com o que toca.
player-lyrics-online = Letras do { $host }
player-sponsorblock = Pular trechos patrocinados ({ $host })
player-skip-sponsor = Patrocínios
player-skip-selfpromo = Autopromoção
player-skip-interaction = Pedidos de like ou inscrição
player-skip-intro = Aberturas
player-skip-outro = Créditos e telas finais
player-skip-preview = Prévias e recapitulações
player-skip-hook = Ganchos no começo
player-skip-music-offtopic = Conversa dentro de um clipe musical
player-skip-filler = Enrolação e desvios
player-trouble-write = Não foi possível gravar os ajustes do player: { $why }
cmd-player-pair = Parear com outra máquina da tailnet
cmd-player-machines = As outras máquinas da tailnet: tocar junto com uma
ask-player-pair = O nome da outra máquina, como a tela dela o mostra
ask-player-pair-code = O código que a outra máquina mostra
player-no-tailnet = Esta máquina não está numa tailnet, então nenhuma outra pode tocar junto com ela.
player-about-machines = Toque junto com outra máquina da tailnet: como a tela dela, como o alto-falante dela, ou os dois. Pareie uma vez com o código que a outra mostra.
player-join = juntar
player-joined-as = { $role }, { $ms } ms de diferença
player-guest = pode se juntar a esta
player-invite = Mostrar um código para parear
player-pair = Parear com uma máquina…
player-role-both = imagem e som
player-role-screen = só a imagem
player-role-sound = só o som
player-trouble-remote = A outra máquina: { $why }
player-quit-everywhere = Fechar o player em todo lugar
# The player's own files, and another name for what plays: rows of "More
# features", each asking on the door's line.
player-feature-save-set = Salvar conjunto…
player-feature-save-queue = Salvar fila…
player-feature-rename = Renomear…
player-renamed = Agora se chama { $name }.
player-renamed-back = { $was } voltou ao próprio nome.
player-saving = Gravando { $name } em { $folder }.
player-about-open = Um conjunto toca agora, logo depois do que toca. Uma fila toma o lugar da fila.
player-ask-replace-queue = Pôr { $name } no lugar da fila? A fila como está agora sai.
player-queue-opened = { $name } agora é a fila.
player-open-not-ours = { $name } não é um conjunto (.rset) nem uma fila (.rqueue).
player-heading-question = Uma pergunta
player-answer-no = Não
player-answer-yes = Sim
player-added = { $name } foi posto na fila.
player-playing-now = Tocando { $name } agora; a fila fica.
player-ask-clear = Tirar tudo da fila?
player-ask-quit = Fechar o player em todo lugar? O que toca para, e toda janela, coluna e vista dele se fecha. A fila fica guardada para a próxima vez.
player-queue-cleared = A fila está vazia agora.
player-not-installed = O player não está instalado nesta máquina: o perfil dela o deixa de fora (ronoctua.player.enable).
player-column-refused = A coluna não pôde ser levantada: { $why }
player-browse-nothing-to-play = Nada nesta pasta é algo que o player toque.
player-trouble-unplayable = Não foi possível tocar isto: { $why }
player-trouble-no-decoder = Este formato precisa de { $program }, que não está instalado.
player-trouble-unsupported = Nenhum decodificador aqui lê { $format }.
player-trouble-no-engine = O player não conseguiu subir o motor: { $why }
# Uma frase para cada problema que o player diz (`player::trouble_sentence`).
player-trouble-not-understood = O player não entendeu o pedido — ele e o resto do sistema são de versões diferentes: { $why }
player-trouble-too-many-leaves = A fila comporta no máximo { $most } itens.
player-trouble-too-many-depth = Playlists vão no máximo { $most } níveis.
player-trouble-too-many-lanes = No máximo { $most } faixas tocam juntas.
player-trouble-too-many-subtitles = No máximo { $most } legendas vão sobre uma imagem.
player-trouble-too-many-subtitle-pictures = Legendas desenhadas como imagem: no máximo { $most } sobre uma imagem.
player-trouble-too-many-voices = No máximo { $most } arquivos tocam ao mesmo tempo num conjunto.
player-trouble-too-many-pictures = No máximo { $most } imagens aparecem ao mesmo tempo.
player-trouble-too-many-ambience = No máximo { $most } sons ambientes tocam juntos.
player-trouble-no-such-entry = Esse item não está mais na fila.
player-trouble-not-a-playlist = Só uma playlist guarda itens.
player-trouble-into-itself = Uma playlist não entra nela mesma.
player-trouble-nameless = Uma playlist precisa de um nome.
player-trouble-unreadable = { $path } não pôde ser lido.
player-trouble-unwritable = { $path } não pôde ser gravado: { $why }
player-trouble-nowhere-to-save = { $path }: nenhuma pasta aceitou o que foi buscado para ele — nem ao lado dele, nem a do próprio player.
player-trouble-no-runtime-dir = Esta sessão não tem pasta de execução (XDG_RUNTIME_DIR), onde o player guarda os rascunhos e o que buscou.
player-trouble-too-large = { $path } é maior que qualquer arquivo que o player grava.
player-trouble-not-ours = { $path } não é um arquivo do player, diga o nome o que disser.
player-trouble-newer = { $path } foi salvo por um player mais novo (versão { $found }).
player-trouble-damaged = { $path } está danificado: algo nele não é o que o formato diz.
player-trouble-empty = { $path } não aponta nada que toque.
player-trouble-missing = { $program } não está nesta máquina, e isto precisa dele.
player-trouble-sink-gone = { $device } não está lá; a faixa toca na saída padrão.
player-trouble-unreachable = { $what } não respondeu: { $why }
player-trouble-stream-down = { $tried ->
        [one] { $title }: o endereço dele falhou.
       *[other] { $title }: os { $tried } endereços dele falharam.
    }
player-trouble-listing-absent = { $what }: esta máquina não tem o yt-dlp para listá-la.
player-trouble-listing-unstartable = { $what }: o yt-dlp não pôde ser iniciado.
player-trouble-listing-slow = { $what }: o yt-dlp demorou demais e foi parado.
player-trouble-listing-failed = { $what }: o yt-dlp não conseguiu listá-la — sem rede, uma lista que foi removida ou um endereço que ele não conhece.
player-trouble-listing-garbled = { $what }: o yt-dlp respondeu algo que não é uma lista.
player-trouble-listing-too-large = { $what }: a lista é maior que o player lê.
player-trouble-listing-not-an-address = { $what } não é um endereço da web.
player-trouble-listing-not-a-list = { $what } é um vídeo só, não uma lista.
player-trouble-listing-nothing-to-ask = { $what }: não havia palavras para buscar.
player-trouble-subtitle = As legendas de { $path } não puderam ser lidas: { $why }.
player-subtitle-refusal-too-large = maior que { $most } MiB, o que nenhum arquivo de legendas é
player-subtitle-refusal-unknown = não é um formato de legenda que o player conheça
player-subtitle-refusal-damaged = o arquivo está danificado
player-subtitle-refusal-empty = o arquivo não tem legendas
player-subtitle-refusal-untimed = letra sem tempos: nada diz quando mostrar
player-subtitle-refusal-too-many-cues = mais de { $most } legendas num arquivo só
player-subtitle-refusal-too-slow = ler levou mais de { $most } s
player-subtitle-refusal-streamed = na rede, elas aparecem só sozinhas, nunca empilhadas com outras
player-subtitle-refusal-no-index = o .sub do VobSub precisa do .idx ao lado
player-trouble-no-picture = { $path } não tem sobre o que aparecer: nenhum filme da fila tem o nome dela, nada com imagem está tocando e nenhum filme ao lado tem o nome dela.
player-trouble-nothing-playing = Nada está tocando.
player-trouble-no-such-lane = Essa faixa não faz mais parte do que toca.
player-trouble-wrong-kind = Isso não vale para esta faixa.
player-trouble-bad-value = Esse valor não cabe nesta faixa.
player-trouble-last-lane = Um conjunto fica com pelo menos uma faixa.
player-trouble-following = Tocando junto com { $host }: quem decide é aquela máquina.
player-trouble-stream-plays-alone = Um stream da rede toca sozinho.
player-trouble-no-ambience = Esta máquina ainda não tem sons ambientes: eles vêm com o player quando as gravações forem publicadas.
player-trouble-no-such-sound = Não há som ambiente chamado { $id }.
player-trouble-sound-unplayable = { $sound } não tocou: { $why }
player-trouble-tags = { $name } — { $why }
player-trouble-tag-not-a-file = Isto não é um arquivo nesta máquina: as etiquetas dele só podem ser lidas.
player-trouble-tag-gone = O arquivo não está mais lá.
player-trouble-tag-changed-since = O arquivo mudou desde que foi lido; nada foi gravado. Ele é lido de novo: repita a mudança.
player-trouble-tag-hard-linked = O arquivo tem outros nomes (hard links), que uma cópia nova perderia.
player-trouble-tag-read-only-folder = A pasta dele não aceita escrita.
player-trouble-tag-read-only-file = O arquivo está marcado como somente leitura.
player-trouble-tag-not-yours = Outro usuário é dono deste arquivo.
player-trouble-tag-no-room = Não há espaço para a cópia: precisa de { $need }, há { $free }.
player-trouble-tag-unreadable = Não foi possível ler as etiquetas.
player-trouble-tag-unwritable = A gravação falhou; o arquivo ficou como estava.
player-trouble-tag-no-writer = Nada aqui grava as etiquetas deste tipo de arquivo ({ $container }).
player-trouble-tag-missing = { $program ->
        [mkvextract] O mkvextract não está instalado: arquivos Matroska só são lidos por inteiro e gravados com o perfil de mídia.
        [mkvmerge] O mkvmerge não está instalado: arquivos Matroska só são lidos por inteiro e gravados com o perfil de mídia.
        [mkvpropedit] O mkvpropedit não está instalado: arquivos Matroska só são lidos por inteiro e gravados com o perfil de mídia.
       *[other] O { $program } não está instalado.
    }
player-trouble-tag-program-failed = O { $program } falhou; o arquivo ficou como estava.
player-trouble-tag-timed-out = O { $program } demorou demais e foi parado.
player-trouble-tag-too-big = Grande demais para isto: { $bytes }, no máximo { $most }.
player-trouble-tag-not-an-image = Isso não é uma imagem que o player consiga pôr num arquivo.
player-trouble-tag-bad-number = Não é um número (1, ou 1/12).
player-trouble-tag-bad-date = Não é uma data (2024, 2024-05 ou 2024-05-31).
player-trouble-tag-bad-isrc = Não é um ISRC (duas letras, três letras ou dígitos, sete dígitos).
player-trouble-tag-bad-language = Não é um código de língua (pt, por, pt-BR).
player-trouble-tag-read-only-view = Lido para mostrar: muda pelo campo dele, ou pela etiqueta do próprio arquivo, mais acima.
player-trouble-tag-bad-name = Não é um nome que esta etiqueta aceite.
player-trouble-tag-not-text = O valor dela é dado, não texto: não é gravado aqui.
player-trouble-tag-not-global = Ela pertence a uma trilha, um capítulo ou um anexo, ou está aninhada com outras etiquetas: aqui só se gravam as etiquetas do arquivo inteiro, cada uma sozinha.
player-trouble-tag-no-such-thing = Essa imagem ou esse fluxo não está mais no arquivo.
player-trouble-tag-taken = Já existe uma capa ao lado do arquivo com os dois nomes que ela poderia ter.
player-trouble-tag-cancelled = Parado; o arquivo ficou como estava.
player-container-mpeg = áudio MPEG
player-container-aac = AAC
player-container-flac = FLAC
player-container-ogg-vorbis = Ogg Vorbis
player-container-opus = Opus
player-container-speex = Speex
player-container-ogg-other = Ogg
player-container-mp4 = MP4
player-container-wav = WAV
player-container-aiff = AIFF
player-container-ape = Monkey's Audio
player-container-mpc = Musepack
player-container-wavpack = WavPack
player-container-matroska = Matroska
player-container-webm = WebM
player-container-avi = AVI
player-container-mpeg-ts = MPEG-TS
player-container-flv = vídeo Flash
player-container-asf = ASF
player-container-other = outro contêiner
player-trouble-no-such-draft = Esse rascunho já foi gravado ou descartado.
player-trouble-too-many-drafts = No máximo { $most } campos abertos no editor ao mesmo tempo.
player-trouble-too-many-writes = No máximo { $most } mudanças esperando gravação.
player-trouble-too-many-takes = No máximo { $most } downloads esperando.
player-service-lrclib = LRCLIB
player-service-sponsorblock = SponsorBlock
player-service-musicbrainz = MusicBrainz
player-service-coverartarchive = Cover Art Archive
player-service-wikidata = Wikidata
player-service-tmdb = TMDB
player-service-opensubtitles = OpenSubtitles
player-service-subdl = SubDL
player-service-podnapisi = Podnapisi
player-trouble-lookup-no-key = { $service } precisa da sua chave: guarde-a na página Internet.
player-trouble-lookup-key-refused = { $service } recusou a chave, o token ou a conta.
player-trouble-lookup-wrong-key = Essa é a chave v3 do TMDB; guarde o Token de Leitura da API no lugar dela.
player-trouble-lookup-quota = { $service }: os downloads de hoje acabaram.
player-trouble-lookup-too-many-asks = { $service } pediu para ir mais devagar; tente de novo daqui a pouco.
player-trouble-lookup-nothing-found = { $service } não achou nada.
player-trouble-lookup-nothing-to-ask = Nada com que perguntar a { $service }: sem título e sem palavras.
player-trouble-lookup-unreachable = Não foi possível falar com { $service }.
player-trouble-lookup-malformed = { $service } respondeu algo que este player não lê.
player-trouble-lookup-too-large = { $service } respondeu mais do que uma resposta deveria ter.
player-trouble-lookup-declined = { $service } recusou.
player-trouble-lookup-unreadable = Não foi possível ler o arquivo para tirar a impressão dele.
player-trouble-lookup-too-small = O arquivo é pequeno demais para a impressão; foi perguntado pelo nome.
player-trouble-lookup-dormant = { $service } está dormindo nesta versão: o domínio dele não responde desde junho de 2026.
player-trouble-off = Não perguntado: { $host } ainda não foi permitido.
player-trouble-exists = { $path } já existe.
player-trouble-no-such-candidate = Esse resultado é de uma busca anterior.
player-source-radio = radio-browser
player-source-iptv = iptv-org
player-source-archive = o Internet Archive
player-trouble-directory-unreachable = Não foi possível falar com { $source }.
player-trouble-directory-declined = { $source } recusou.
player-trouble-directory-too-many-asks = { $source } pediu para ir mais devagar; tente de novo daqui a pouco.
player-trouble-directory-nothing-found = { $source } não achou nada.
player-trouble-directory-malformed = { $source } respondeu algo que este player não lê.
player-trouble-directory-too-large = { $source } respondeu mais do que uma lista deveria ter.
player-trouble-directory-nothing-playable = Nada ali é algo que este player toque.
player-trouble-directory-nothing-to-ask = Nada com que perguntar a { $source }: digite algumas palavras.
player-trouble-directory-not-a-name = Isso não é um nome que { $source } dá; nada foi enviado.
player-trouble-no-such-category = Este tema não tem isso.
player-trouble-no-such-pick = Isso não está mais em nenhuma lista: abra de novo para guardar.
player-trouble-picks-no-home = Não há pasta pessoal onde guardar a lista.
player-trouble-picks-unreadable = A lista no disco não pôde ser lida, e ficou como estava.
player-trouble-picks-newer = A lista foi gravada por um player mais novo.
player-trouble-picks-unwritable = Não foi possível gravar a lista.
player-heading-metadata = Metadados
player-heading-technical = Técnico
player-meta-file = Do arquivo
player-meta-reading = Lendo o que o arquivo diz de si…
player-meta-still-reading = O player ainda está lendo este arquivo — ele pode estar num disco lento. Ler de novo pergunta outra vez.
player-field-blank = Nada escrito neste campo.
player-field-edit = Editar no editor
player-meta-fixed = Nada disto muda aqui. { $why }
player-meta-refresh = Ler de novo
player-meta-refreshing = Lendo o arquivo de novo.
player-meta-count = { $count }
player-meta-joiner = { " · " }
player-list-joiner = {", "}
player-meta-cover-size = { $width } × { $height }
player-meta-cover-none = nenhuma
player-meta-cover-some = no arquivo
player-meta-cover-nothing = Este arquivo não traz capa.
player-page-cover = Imagem da capa
player-page-files-cover = Outra imagem para a capa
player-page-streams = Fluxos
player-page-chapters = Capítulos
player-page-every-tag = Todas as etiquetas
player-cover-save = Salvar ao lado do arquivo
player-cover-remove = Tirar do arquivo
player-ask-remove-cover = Tirar a capa de { $name }? Nenhuma cópia dela fica guardada.
player-cover-putting = Pondo { $picture } em { $name }.
player-cover-saving = Salvando a capa ao lado de { $name }.
player-cover-removing = Tirando a capa de { $name }.
player-cover-saved = A capa foi salva como { $name }.
player-page-suggestions = Sugestões
player-page-candidate = Sugestão
player-page-find-subtitles = Buscar legendas
player-page-find-lyrics = Buscar letras
player-page-languages = Escolher as línguas
player-page-radio = Rádios do mundo
player-page-radio-genres = Gêneros
player-page-radio-languages = Línguas das rádios
player-page-stations = Estações
player-page-tv = TVs do mundo
player-page-tv-channels = Canais de TV
player-page-channels = Canais por tema
player-page-favourites = Favoritos
player-page-recent = Tocados há pouco
player-about-radio = Estações de rádio do mundo inteiro, do radio-browser: por país, por gênero, por língua ou pelo nome.
player-about-tv = Canais de TV do mundo inteiro, da lista pública do iptv-org: por país ou pelo nome.
player-about-favourites = O que você guardou com a estrela: estações, canais, filmes e lives.
player-about-recent = As transmissões que tocaram há pouco, como o player se lembra delas.
player-favourites-none = Nada guardado ainda.
player-recent-none = Nada tocado ainda.
player-radio-search = Buscar estações pelo nome
player-radio-genre-search = Achar um gênero pelo nome
player-radio-language-search = Achar uma língua pelo nome
player-about-radio-genres = Os gêneros com mais estações. Qualquer outro se acha pelo nome, como o radio-browser o escreve.
player-about-radio-languages = As línguas com mais estações. Qualquer outra se acha pelo nome, como o radio-browser a escreve — em inglês.
player-tv-search = Buscar canais pelo nome
player-heading-countries = Países
player-picks-radio = Estações de rádio
player-picks-tv = Televisão
player-picks-archive = Filmes do acervo
player-picks-live = Transmissões ao vivo
player-stations-count = { $count ->
        [one] uma estação
       *[other] { $count } estações
    }
player-channels-count = { $count ->
        [one] um canal
       *[other] { $count } canais
    }
player-globe-placeless = Sem lugar no mapa ({ $count })
player-globe-no-country = De país nenhum
player-globe-no-place = Sem lugar no mapa
player-globe-place = { $count } · { $place }
player-globe-one = { $name } · { $country }
player-radio-bitrate = { $kbps } kbps
player-radio-hls = transmissão adaptativa
player-stream-silent = não respondeu
player-tv-lines = { $lines } linhas
player-tv-geo-blocked = só em alguns países
player-tv-sometimes-off = nem sempre no ar
player-tv-shown = { $shown } mostrados dos { $total } encontrados.
player-stations-page-of = Página { $page } de { $pages } — { $total } estações ao todo, as mais tocadas primeiro.
player-stations-page = Página { $page } desta lista — { $most } por página, as mais tocadas primeiro.
player-stations-more = Mais estações
player-stations-earlier = Estações anteriores
player-listed-at = Lista de { $date }
player-listed-updating = Atualizando…
player-listing-nothing = Nada listado ainda.
player-listing-again = Tentar de novo
player-listing-every = Mostrando { $shown } de { $total }: mostrar todos
player-searched-title = Com o nome “{ $words }”
player-favourite-added = Guardado nos favoritos.
player-favourite-removed = Tirado dos favoritos.
player-about-internet = Nada aqui chega à internet até você permitir, e cada linha diz a quem pergunta. As do primeiro título perguntam sozinhas; as outras só quando você pressiona uma linha que as nomeia, e pressionar permite.
player-heading-on-their-own = Perguntados sozinhos
player-heading-when-asked = Perguntados ao pressionar
player-heading-keys = Suas chaves
player-heading-results = Resultados
player-consent-allowed = permitido
player-consent-not-allowed = não permitido
player-consent-by-the-switch = e a chave acima pergunta sozinha
player-consent-ask = Perguntar a { $hosts }
player-consent-first = pressionar permite
player-consent-let = Permitir { $host } e perguntar de novo
player-consent-hosts = pergunta a { $hosts }
player-key-sent-to = enviada só a { $host }
player-directory-archive = Filmes do Internet Archive
player-directory-lives = Lives do YouTube
player-key-opensubtitles = Chave do OpenSubtitles
player-key-opensubtitles-account = Conta do OpenSubtitles
player-key-tmdb = Chave de acesso do TMDB (token)
player-key-subdl = Chave do SubDL
player-key-words-opensubtitles = a chave do OpenSubtitles
player-key-words-opensubtitles-account = a conta do OpenSubtitles
player-key-words-tmdb = a chave de acesso do TMDB
player-key-words-subdl = a chave do SubDL
player-key-kept = guardada
player-key-none = não guardada
player-key-subdl-address = O SubDL só aceita a chave no endereço, então ela aparece nos registros do próprio SubDL.
player-ask-forget-key = Esquecer { $what }? O player não guarda outra cópia.
player-key-checking = Guardada. Perguntando a { $service } se aceita…
player-key-accepted = { $service } aceitou a chave.
player-key-forgotten = Chave esquecida.
player-key-nothing-to-forget = Não há o que esquecer: este computador não guarda { $what }.
player-key-trouble-no-home = Não há pasta pessoal onde guardá-la.
player-key-trouble-empty = Nada foi digitado.
player-key-trouble-malformed = Isso tem uma quebra de linha ou outro caractere de controle.
player-key-trouble-too-long = Longa demais para uma chave.
player-key-trouble-unwritable = Não foi possível gravar a chave.
player-key-trouble-unreadable = Não foi possível ler a chave guardada.
player-prefs-will-not-parse = O player.json não é lido, e gravar substituiria o que há nele.
player-desk-asking = Perguntando a { $hosts }…
player-desk-found = { $count ->
        [one] Um resultado
       *[other] { $count } resultados
    }
player-desk-searched-for = Buscado: { $words }
player-desk-by-fingerprint = Encontrado pela impressão digital do arquivo.
player-desk-by-id = Consultado pelo id que o arquivo traz.
player-desk-nothing-found = Nada encontrado.
player-take-taking = Buscando…
player-take-handed = Gravando no arquivo…
player-take-saved = Salvo como { $name }.
player-about-suggestions = Etiquetas para este arquivo, vindas da internet. A linha que pergunta diz a quem, e pressioná-la permite. Nada é gravado até você escolher o que levar.
player-suggest-source = Sugerido por
player-suggest-came-from = Estes vieram do serviço { $service }.
player-suggest-poster-needs-token = O pôster só vem com o TMDB e o seu token; o Wikidata quase nunca tem.
player-suggest-other-words = Buscar com outras palavras
player-suggest-reading = lendo os detalhes…
player-suggest-busy = perguntando…
player-about-candidate = Escolha o que gravar: um campo em branco no arquivo já começa escolhido. O que o arquivo diz agora está sob cada valor.
player-suggest-now = agora: { $value }
player-suggest-same = o arquivo diz o mesmo
player-suggest-cover = Capa
player-suggest-taken = será gravada
player-suggest-left = fica de fora
player-suggest-picture-suggested = Sugerida
player-suggest-picture-current = Atual
player-suggest-picture-none = Nenhuma capa veio
player-suggest-fetching = Buscando a capa…
player-suggest-nothing-chosen = Nada escolhido para gravar
player-suggest-write = { $count ->
        [one] Gravar um campo
       *[other] Gravar { $count } campos
    }
player-suggest-write-cover = { $count ->
        [0] Gravar a capa
        [one] Gravar um campo e a capa
       *[other] Gravar { $count } campos e a capa
    }
player-suggest-writing = Gravando o que foi escolhido em { $name }…
player-about-find-subtitles = Legendas para este filme vindas da internet, nas línguas abaixo. A sincronizada para este mesmo arquivo vem marcada. Escolher uma a baixa ao lado do filme e a põe sobre a imagem.
player-find-needs-a-key = Buscar legendas exige a sua chave do OpenSubtitles ou do SubDL: hoje não há fonte sem chave.
player-find-languages = Línguas
player-find-unknown-language = { $code } não é uma língua que este player conheça.
player-find-language-there = { $language } já está na busca.
player-find-language-added = { $language } também entra na busca.
player-find-language-removed = { $language } saiu da busca.
player-find-language-last = A busca de legendas precisa de pelo menos uma língua.
player-find-language-searched = { $language } está na busca; d, ou o glifo no fim da linha, a tira.
player-find-type-language = Digitar uma língua
player-about-languages = As legendas são buscadas em toda língua ligada. A lista tem todas as línguas em que os serviços buscam.
player-find-hash-match = sincronizada para este arquivo
player-find-hearing = para surdos
player-find-machine = tradução automática
player-find-downloads = { $count ->
        [one] um download
       *[other] { $count } downloads
    }
player-find-downloads-left = Downloads restantes hoje
player-find-taking = Baixando { $name }…
player-lyrics-one-source = Uma fonte, o LRCLIB: sem campo de língua e sem tradução.
player-lyrics-to = Salvar em
player-lyrics-to-beside = um .lrc ao lado da música
player-lyrics-to-tag = a letra da própria música
player-find-synced = sincronizada
player-find-plain = só o texto
player-find-instrumental = instrumental
player-find-same-length = a duração da música
player-find-off-by = { $seconds } s a mais ou a menos
player-ask-replace-lrc = Substituir { $name }? O que ele tem não é guardado.
player-lyrics-taking = Salvando a letra ao lado de { $name }…
player-lyrics-into-tag = Gravando a letra em { $name }…
ask-player-find-words = Buscar
ask-player-search-radio = Estações com o nome
ask-player-search-tv = Canais com o nome
ask-player-search-genre = Gênero
ask-player-search-language = Língua (em inglês, como o radio-browser a escreve)
ask-player-find-language = Língua, pelo nome ou pelo código (japonês, pt-BR)
ask-player-key-opensubtitles = Chave da API do OpenSubtitles
ask-player-key-tmdb = Token de Leitura da API do TMDB
ask-player-key-subdl = Chave da API do SubDL
ask-player-account-user = Usuário do OpenSubtitles
ask-player-account-password = Senha do OpenSubtitles
tongue-en = Inglês
tongue-pt-br = Português (Brasil)
tongue-pt-pt = Português (Portugal)
tongue-pt = Português
tongue-es = Espanhol
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
tongue-ja = Japonês
tongue-ko = Coreano
tongue-sw = Suaíli
tongue-af = Africâner
player-streams-none = Nenhum fluxo deste arquivo pôde ser lido.
player-chapters-none = Este arquivo não tem capítulos.
player-tags-none = Nenhuma ferramenta leu etiqueta deste arquivo.
player-stream-label = { $index } · { $kind }
player-stream-kind-video = imagem em movimento
player-stream-kind-cover = imagem de capa
player-stream-kind-audio = trilha de som
player-stream-kind-subtitle = trilha de legenda
player-stream-kind-data = dados no tempo
player-stream-kind-attachment = arquivo anexo
player-stream-default = padrão
player-stream-forced = forçada
player-stream-hearing = para quem ouve pouco
player-chapter-untitled = sem título
player-origin-id3v2 = Etiquetas ID3v2
player-origin-id3v1 = Etiquetas ID3v1
player-origin-ape = Etiquetas APEv2
player-origin-vorbis = Comentários Vorbis
player-origin-mp4 = Etiquetas MP4
player-origin-riff-info = Etiquetas RIFF INFO
player-origin-aiff-text = Trechos de texto AIFF
player-origin-matroska-global = Etiquetas Matroska do arquivo inteiro
player-origin-matroska-track = Etiquetas Matroska de uma trilha
player-origin-matroska-other = Etiquetas Matroska de outras partes
player-origin-container = O contêiner, como o ffprobe lê
player-origin-stream-n = Fluxo { $index }, como o ffprobe lê
player-origin-exiftool = exiftool, { $group }
player-origin-stream = O que o stream diz de si
player-tag-value-cut = { $size } ao todo · { $head }…
player-tag-value-binary = { $size } de dados
player-tag-add = Acrescentar uma etiqueta
player-tag-bad-name = “{ $typed }” não é um nome para { $where }: { $rule }.
player-tag-name-rule-vorbis = letras, dígitos e pontuação, sem = nem acentos
player-tag-name-rule-ape = de 2 a 255 letras, dígitos ou pontuação, sem acentos
player-tag-name-rule-riff = quatro letras ou dígitos, como ISFT
player-tag-name-rule-aiff = NAME, AUTH, ANNO ou (c)
player-tag-name-rule-mp4 = uma palavra, ou ----:mean:name
player-tag-name-rule-ffmpeg = qualquer coisa sem =
player-tag-name-rule-plain = uma linha, de até um kibibyte
player-sheet-unread = Não foi possível ler { $source }. { $why }
player-sheet-source-lofty = as etiquetas
player-sheet-source-ffprobe = os fluxos e os capítulos
player-sheet-source-exiftool = a lista de todas as etiquetas
player-sheet-source-matroska = as etiquetas próprias do Matroska
player-tag-empty = vazio
player-tag-already-empty = { $field }: nada a apagar, o campo já está vazio.
player-tag-more-lines = { $lines } linhas · { $first }
player-draft-open = aberto no editor
player-draft-opened = Aberto no editor: salve lá, e aqui se pergunta antes de gravar.
player-draft-says = O que o editor salvou
player-draft-kept = { $field }: segue aberto no editor; volta a perguntar ao salvar.
player-ask-drop-draft = { $field }: jogar fora o rascunho aberto no editor? O arquivo fica como está.
player-draft-dropped = { $field }: o rascunho foi jogado fora.
player-ask-write-draft = { $field } de { $name }: o editor salvou algo novo. Gravar no arquivo?
player-ask-clear-tag = { $field } de { $name }: apagar?
player-tag-writing = { $field } de { $name }: gravando no arquivo.
player-tag-clearing = { $field } de { $name }: apagando.
player-tag-named-writing = { $tag } de { $name }: gravando no arquivo.
player-tag-named-removing = { $tag } de { $name }: tirando do arquivo.
player-ask-remove-tag = { $values ->
        [one] { $tag } de { $name }: tirar do arquivo?
       *[other] { $tag } de { $name }: tirar os { $values } valores dela?
    }
player-retag-writing = Gravando no arquivo… { $percent }%
player-retag-waiting = { $count ->
        [one] Mais uma mudança esperando.
       *[other] Mais { $count } mudanças esperando.
    }
player-tag-written = A última mudança foi gravada no arquivo.
player-tech-container = Contêiner
player-tech-length = Duração
player-tech-bit-rate = Taxa de bits
player-tech-video-codec = Codec de vídeo
player-tech-width = Largura
player-tech-height = Altura
player-tech-fps = Quadros por segundo
player-tech-audio-codec = Codec de áudio
player-tech-sample-rate = Taxa de amostragem
player-tech-channels = Canais
player-tech-channel-layout = Disposição dos canais
player-tech-bits-per-sample = Bits por amostra
player-tech-programs = Programas
player-tech-value-kbps = { $kbps } kb/s
player-tech-value-px = { $px } px
player-tech-value-hz = { $hz } Hz
player-tag-title = Título
player-tag-artist = Artista
player-tag-album = Álbum
player-tag-album-artist = Artista do álbum
player-tag-track = Número da faixa
player-tag-disc = Número do disco
player-tag-date = Data
player-tag-genre = Gênero
player-tag-composer = Compositor
player-tag-lyricist = Letrista
player-tag-comment = Comentário
player-tag-lyrics = Texto da letra
player-tag-publisher = Gravadora
player-tag-isrc = ISRC
player-tag-description = Descrição
player-tag-show = Série
player-tag-season = Temporada
player-tag-episode = Episódio
player-tag-director = Direção
hint-player = { $what }. Um clique toca ou pausa; o botão direito abre a coluna, ou a fecha.
hint-player-next = A seguir: { $what }. O botão direito abre o player aqui.
hint-player-next-shuffled = A seguir, na ordem embaralhada: { $what }. O botão direito abre o player aqui.
hint-player-next-end = Este é o último item da fila: avançar faz o player parar. O botão direito abre o player aqui.
# The modeline's pair while only the ambient sounds are on (`player_mark`).
hint-player-ambience = Pausar os sons ambientes, ou deixá-los continuar
hint-player-next-ambience = Só os sons ambientes estão ligados: a fila não tem o que vem depois
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

## O caminho de volta, primeiro na linha de toda vista: Esc, e um clique nele
view-back = voltar
about-view-back = Volta um nível e, no primeiro, fecha esta vista — o mesmo que apertar Esc.

## Uma pergunta na linha do cabeçalho de uma janela — o que se digita ali
line-cancel = cancelar
about-line-cancel = Deixa a pergunta sem resposta — o mesmo que apertar Esc.
line-answer = responder
about-line-answer = Responde com o que está digitado na linha — o mesmo que apertar Enter.
door-paste-failed = Não foi possível ler o que foi copiado.

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
notif-action-trail-panel = Abrir as trilhas
window-menu-no = Não
window-menu-yes-kill = Sim, forçar o fechamento
window-menu-yes-close-others = Sim, fechar todas as outras janelas
hint-window-menu = Clique para abrir as opções de tudo o que dá para fazer com esta janela

# THE PERSON'S OWN PROGRAMS (LEADER i) — `software.rs`, its words
# (`software_words.rs`) and the queue's phases. A key is never text here:
# every sequence arrives as `{ $keys }`.
software-title = Programas
group-software = programas
cmd-software-open = Seus programas: procurar, instalar, experimentar e remover
cmd-software-window = Seus programas, numa janela
cmd-software-trials = Os programas em experiência
cmd-software-maintenance = Atualizações, espaço e voltar atrás, dos seus programas
cmd-software-android = O Android e os apps dele
cmd-software-launch-latest = Abrir o que foi instalado por último
software-down = descer
software-up = subir
software-enter = abrir
software-next-source = próxima fonte
software-previous-source = fonte anterior
software-less = anterior
software-more = seguinte
software-install = instalar
software-try-it = experimentar
software-launch = iniciar
software-update = atualizar
software-edit = editar
software-remove = remover
software-stop = parar
software-paste = colar
cmd-software-down = A próxima linha
cmd-software-up = A linha de cima
cmd-software-enter = Fazer o que a linha faz; num fato, copiá-lo
cmd-software-next-source = Mostrar a próxima fonte, ou a próxima prateleira de ícones
cmd-software-previous-source = Mostrar a fonte anterior, ou a prateleira de ícones anterior
cmd-software-less = O valor anterior desta linha
cmd-software-more = O próximo valor desta linha
cmd-software-install = Instalar só para você, pela fonte mostrada
cmd-software-try-it = Rodar sem instalar
cmd-software-launch = Iniciar o programa
cmd-software-update = Atualizar este agora
cmd-software-edit = Mudar este web app
cmd-software-remove = Remover, depois de uma pergunta
cmd-software-stop = Parar
cmd-software-paste = Colar na busca o que foi copiado
cmd-software-web-new = Um web app a partir de um endereço
ask-software-web-new = Endereço do site
cmd-software-web-address = Mudar o endereço do web app
ask-software-web-address = Endereço que ele abre
cmd-software-web-name = Mudar o nome do web app
ask-software-web-name = Nome no lançador
cmd-software-icon-path = Uma imagem sua como ícone do web app
ask-software-icon-path = Caminho da imagem
cmd-software-file-path = Instalar um arquivo .AppImage, .apk ou .flatpakref
ask-software-file-path = Caminho do arquivo a instalar
software-type-on-a-row = Para escrever aqui, use { $keys } na linha.
software-type-to-search = Digite um nome, ou o que ele faz, e a busca começa sozinha.
software-reading = Lendo o que há neste computador…
software-asking = perguntando…
software-on = ligado
software-off = desligado
software-joined = { $first } · { $second }
software-echo-queued = { $app } foi para a fila — { $keys } mostra.
software-source-all = todas as fontes
software-source-nix = Nix
software-source-flatpak = Flatpak
software-source-appimage = AppImage
software-source-android = Android
software-source-web = web app
software-source-system = o sistema
software-origin-mine = instalado por você
software-origin-declared = declarado no flake
software-search = Procurar
software-search-empty = digite um nome, ou o que ele faz
software-filter = Mostrando
software-heading-installed = Instalados para você
software-note-yours = O que se instala aqui é só seu: o sistema e o repositório continuam como o flake diz.
software-note-none = Nada instalado por você ainda. Os programas do sistema estão no lançador — { $keys }.
software-note-none-here = Nada desta fonte.
software-heading-trying = Em experiência
software-heading-looking-after = Manutenção
software-updates = Atualizações, espaço e voltar atrás
software-updates-count = { $count ->
        [one] uma disponível
       *[other] { $count } disponíveis
    }
software-up-to-date = em dia
software-heading-more = Mais
software-web-new = Novo web app
software-web-new-value = a partir de um endereço
software-trials = Experiências
software-trials-count = { $count ->
        [0] nenhuma rodando
        [one] uma rodando
       *[other] { $count } rodando
    }
software-android-state = Sistema Android
software-android-absent = não há nesta máquina
software-android-not-prepared = não preparado
software-android-preparing = preparando
software-android-running = rodando
software-android-stopped = parado
software-advanced = Avançado
software-advanced-value = fontes, permissões, histórico
software-heading-search = Procurando um programa
software-make-web-app = Fazer um web app dele
software-open-once = Abrir uma vez, sem criar
software-install-file = Instalar este arquivo
software-not-a-package-file = Isso não é um arquivo .AppImage, .apk ou .flatpakref.
software-note-type = Digite um nome, ou o que ele faz. Um endereço vira um web app; o caminho de um .AppImage, .apk ou .flatpakref instala esse arquivo.
software-heading-results = Encontrados
software-heading-by-description = Encontrados pelo que fazem
software-note-nothing-found = Nada com esse nome. Outra grafia, ou o que ele faz, pode encontrá-lo.
software-heading-asked = Onde procurou
software-note-offline = Sem rede: mostrando o que este computador já sabe.
software-status-from = com base em { $from }
software-status-off = desligada no Avançado
software-row-working = Agora
software-queued = esperando na fila
software-working-now = trabalhando
software-working = { $percent } % · { $phase }
software-row-launch = Abrir o programa
software-no-window = sem janela: roda num terminal
software-row-install = Instalar para você
software-install-with = pelo { $source }
software-recommended = recomendado
software-size-download = { $size } para baixar
software-row-try = Experimentar sem instalar
software-try-terminal = abre um terminal onde se digita { $program }
software-try-window = abre numa janela e não guarda nada
software-row-traces = O que pode deixar
software-row-network = Rede na experiência
software-row-keep = Ficar com ele de vez
software-keep-value = instala o que foi experimentado, sem baixar de novo
software-row-stop = Parar a experiência
software-row-source = Fonte
software-also-in = também em { $sources }
software-row-version = Versão
software-row-size = Tamanho
software-row-update = Atualização
software-update-to = { $version } está pronta
software-row-permissions = Permissões
software-permissions-value = o que ele pode acessar
software-row-edit = Mudar este web app
software-row-declare = Tornar permanente
software-declare-copies = copia a linha para o host.nix
software-nothing-to-declare = Esta fonte não tem linha no flake; ele continua seu como está.
software-note-declare-unfree = Não é software livre: só a linha não basta, a máquina também precisa permitir este pacote.
software-row-remove = Remover deste computador
software-heading-about = Sobre ele
software-heading-details = Detalhes
software-row-reference = Nome dele na fonte
software-row-publisher = Feito por
software-row-licence = Licença
software-row-site = Site
software-row-needs = Precisa de
software-row-installed-size = Instalado, ocupa
software-row-permission = Pode acessar
software-row-anti-features = O F-Droid avisa sobre
software-note-trial = Experimentar roda agora e não instala nada; ele sai do disco na próxima limpeza.
software-note-why-nix = Vem do mesmo nixpkgs do sistema, atualiza junto e segue o tema dele.
software-note-why-flatpak = Quem publica é o desenvolvedor e ele roda isolado; o primeiro também baixa uma base compartilhada.
software-note-why-appimage = Um arquivo só, do desenvolvedor; roda com todas as suas permissões.
software-note-why-android = Roda dentro do Android, que se prepara uma vez.
software-note-why-web = É o próprio site, numa janela só dele; é sempre a versão do próprio site.
software-heading-trials = O que você está experimentando
software-try-new = Experimentar algo novo
software-try-new-value = digite o nome
software-note-no-trials = Nada está em experiência.
software-trial-running = aberto há { $minutes } min
software-trial-closed = fechado · no disco até a próxima limpeza
software-heading-web = Um novo web app
software-web-address = Endereço
software-web-no-address = nenhum ainda
software-web-name = Nome
software-web-icon = Ícone no lançador
software-web-no-icon-yet = nenhum escolhido ainda
software-web-profile = Perfil do Chrome
software-web-category = Tipo
software-web-fetching = Lendo o nome e os ícones do site…
software-web-host-name = o nome fica sendo o endereço do site até alguém digitar um
software-web-create = Criar
software-web-save = Salvar as mudanças
software-web-once = Abrir uma vez, como vai ficar
software-web-remove = Remover este web app
software-web-no-icon = Escolha um ícone antes: o lançador precisa de um.
software-note-web = Um web app é o site numa janela sem nada do Chrome; os logins são os do seu perfil do Chrome.
software-heading-icon = O ícone dele
software-shelf = De onde
software-shelf-site = o próprio site
software-shelf-dashboard = Dashboard Icons
software-shelf-collection = a coleção da distro
software-shelf-file = um arquivo seu
software-icon-looking-for = Procurando
software-icon-typed = Escolher uma imagem nos arquivos
software-icon-clipboard = Usar a imagem copiada
software-icon-files = Escolher uma no gerenciador de arquivos
software-note-drop = Ou solte uma imagem aqui.
software-icon-now = Escolhido agora
software-icon-chosen = escolhido
software-note-no-icons = Nenhum ícone com esse nome aqui.
software-heading-android = Android
software-note-android = Apps Android rodam dentro de um sistema Android ao lado deste; ele custa memória enquanto roda.
software-android-image = Imagem
software-android-gapps = com Google Play (cerca de 1,2 GB)
software-android-vanilla = sem Google (cerca de 0,8 GB)
software-note-gapps = O Google Play pede que esta máquina seja registrada uma vez como aparelho não certificado; esta página mostra como, depois de preparar o Android.
software-android-prepare = Preparar o Android
software-note-preparing = Baixando e preparando a imagem do Android — cerca de um gigabyte, uma vez. A fila mostra; nenhuma porcentagem é inventada aqui.
software-heading-register = Registro para o Google Play
software-note-uncertified = A Play Store só funciona depois que este aparelho for registrado na página do Google para aparelhos não certificados: copie o id abaixo e cole lá.
software-android-id = Id do aparelho
software-android-id-unknown = conhecido depois que o Android rodar
software-android-register = Página do Google
software-android-windows = Janelas
software-android-one-per-app = uma por app
software-android-one-for-all = uma para o Android todo
software-note-windows-next-session = A mudança de janelas vale na próxima vez que o Android iniciar.
software-heading-android-apps = Dentro do Android
software-note-no-android-apps = Nenhum app instalado dentro do Android ainda.
software-android-apk = Instalar um arquivo APK
software-android-aurora = Google Play sem conta
software-android-aurora-value = Aurora Store, pelo F-Droid
software-android-erase = Apagar o Android
software-cannot-undo = isto não se desfaz
software-heading-updates = Atualizações
software-updates-all = Tudo
software-web-always-latest = sempre a versão do próprio site
software-follows-system = segue o sistema (nixpkgs { $rev })
software-look-again = Procurar atualizações agora
software-updates-auto = Automaticamente
software-auto-with = com a atualização do sistema — { $keys }
software-heading-space = Espaço
software-clean = Liberar espaço agora
software-clean-generations = Versões antigas da sua lista Nix
software-clean-flatpak = Runtimes Flatpak sem uso
software-clean-appimage = Sobras de AppImage
software-clean-trials = Programas experimentados
software-clean-trials-value = saem sozinhos na próxima limpeza
software-heading-generations = Voltar atrás (Nix)
software-note-no-generations = Sua lista Nix ainda não tem versão anterior.
software-generation = Versão { $n }
software-in-use = em uso
software-older-versions = Versões mais antigas
software-heading-sources = Fontes
software-online = Procurar na internet
software-online-off = desligada, junto com a busca na internet
software-switch-flathub = Flathub
software-switch-fdroid = F-Droid
software-switch-appimagehub = AppImageHub e GitHub
software-switch-dashboard = Dashboard Icons, para web apps
software-nix-index = Índice do Nix
software-prefer = Recomendar primeiro
software-prefer-nix-first = Nix, depois um Flatpak verificado
software-prefer-flatpak-first = Flatpak, depois Nix
software-ask-above = Perguntar antes de baixar
software-ask-above-size = { $size } ou mais
software-ask-never = nunca
software-heading-trial-defaults = Ao experimentar
software-default-traces = O que a experiência deixa
software-default-network = Rede na experiência lacrada
software-heading-web-defaults = Novos web apps
software-web-profile-default = Perfil do Chrome que usam
software-profile-last-used = o último que o Chrome usou
software-heading-kept = O que fica guardado
software-consents = Não livres permitidos
software-consents-count = { $count ->
        [0] nenhum
        [one] um
       *[other] { $count }
    }
software-history = Histórico
software-history-value = as últimas centenas de coisas feitas
software-heading-permissions-list = Permissões dos Flatpaks
software-heading-dark = Flatpaks escuros
software-dark-gtk4 = GTK 4 e libadwaita
software-dark-follows = seguem a escolha do sistema
software-dark-gtk3 = GTK 3
software-dark-gtk3-value = escuros, pelo Adwaita-dark do sistema
software-dark-gtk3-missing = claros: a extensão Adwaita-dark não está instalada
software-dark-qt = Qt e KDE
software-dark-qt-value = sem as configurações Qt da sessão
software-dark-qt-leaks = as configurações Qt da sessão chegam a eles
software-dark-forced = Forçando um tema
software-dark-none-forced = nenhum app força o GTK_THEME
software-dark-forces = força o GTK_THEME — veja as permissões
software-note-gtk-theme = O GTK_THEME faz um Flatpak perder o próprio estilo; nada aqui o define, e um app que o carrega aparece em vermelho.
software-heading-declarative = Declarar
software-note-declare = Para manter um programa depois de reinstalar, Tornar permanente copia a linha dele para o host.nix; a de um Flatpak vai em ronoctua.software.declaredFlatpaks, que o nix-flatpak instala.
software-heading-permissions = O que { $app } pode acessar
software-note-no-permissions = Não pede nada além da própria janela.
software-note-permissions-restart = Uma mudança chega ao app na próxima vez que ele abrir.
software-heading-asks = O que ele pede
software-gtk-theme-forced = forçado nele: perde o estilo próprio
software-sure-reset = Voltar { $app } ao que ele pede? Toda mudança feita aqui vai embora.
software-yes-reset = Sim, restaurar
software-sure-unset-gtk-theme = Tirar o GTK_THEME de { $app }? Ele volta ao estilo próprio, e nada aqui põe o GTK_THEME de volta.
software-yes-unset-gtk-theme = Sim, tirar
software-heading-consents = Pacotes não livres que você permitiu
software-note-consents = O Nix instala estes mesmo sem licença livre, e os atualiza com esse mesmo sim.
software-note-no-consents = Nenhum ainda.
software-allowed = permitido
software-heading-history = O que foi feito
software-note-no-history = Nada feito ainda.
software-failed = não funcionou
software-no = Não
software-sure-remove = Remover { $app }?
software-yes-remove = Sim, remover e manter o que ele guardou
software-yes-remove-data = Sim, e apagar também o que ele guardou
software-sure-web-remove = Remover o web app { $app }? O site e o seu login continuam no Chrome.
software-yes-remove-web = Sim, remover o web app
software-sure-unfree = { $app } não é software livre. Permitir que o Nix instale este?
software-yes-unfree = Sim, permitir
software-sure-unverified = Nada garante o que { $app } baixa. Instalar mesmo assim?
software-yes-trust = Sim, confio em quem fez
software-sure-download = { $app } baixa { $size }. Continuar?
software-yes-download = Sim, baixar
software-sure-arm = { $app } é feito só para chips de celular e pode não rodar aqui. Instalar mesmo assim?
software-yes-arm = Sim, instalar mesmo assim
software-sure-rollback = Voltar para a versão { $n } da sua lista Nix?
software-yes-rollback = Sim, voltar
software-sure-clean = Liberar o que nada usa? As versões antigas vão embora, e voltar para elas deixa de ser possível.
software-yes-clean = Sim, liberar
software-sure-android = Preparar o Android { $image }? Baixa uma vez e fica até você apagar.
software-yes-android = Sim, preparar
software-sure-erase-android = Apagar o Android, os apps e tudo o que eles guardaram?
software-yes-erase = Sim, apagar
software-sure-foreign = { $file } não foi verificado por nenhum catálogo e roda com as suas permissões. Instalar?
software-sure-leave = Sair sem salvar?
software-yes-leave = Sim, descartar as mudanças
software-traces-scratch = nenhum — pasta de rascunho, some ao sair
software-traces-home = sua pasta e suas configurações
software-traces-sandbox = lacrado — nenhum arquivo seu
software-badge-verified = verificado pelo desenvolvedor
software-badge-unfree = não é software livre
software-badge-sandboxed = roda isolado dos seus arquivos
software-badge-command-line = um comando, sem janela
software-badge-arm-only = feito só para chips de celular
software-badge-broken = quebrado nesta versão do sistema
software-badge-insecure = marcado como inseguro
software-badge-end-of-life = a base dele não recebe mais atualizações
software-badge-anti-features = o F-Droid avisa sobre o que ele faz
software-badge-library = uma biblioteca, não um programa
software-badge-installed = já instalado
software-phase-resolving = entendendo o pedido
software-phase-waiting = esperando outra ordem da mesma fonte
software-phase-downloading = baixando
software-phase-verifying = conferindo
software-phase-unpacking = desempacotando
software-phase-building = compilando
software-phase-installing = instalando
software-phase-integrating = pondo no lançador
software-phase-removing = removendo
software-phase-starting = iniciando
software-phase-indexing = lendo o catálogo
software-phase-preparing = preparando
software-order-install = instalado
software-order-install-file = instalado de um arquivo
software-order-remove = removido
software-order-upgrade = atualizado
software-order-tidy = espaço liberado
software-order-try = experimentado
software-order-stop-trial = experiência parada
software-order-keep = mantido depois de experimentar
software-order-prepare = preparado
software-order-refresh = catálogo buscado de novo
software-order-rollback = voltou atrás
software-order-web-create = web app criado
software-order-web-edit = web app mudado
software-order-android-session = Android iniciado ou parado
software-order-android-erase = Android apagado
software-order-permission = permissão mudada
software-order-android-windows = janelas do Android mudadas
software-grant-files = Seus arquivos
software-grant-network = Rede
software-grant-sound = Som
software-grant-devices = Câmeras e outros dispositivos
software-grant-display = Desenha pelo
software-grant-unset-gtk-theme = Tirar o GTK_THEME
software-grant-reset = Voltar ao que ele pede
software-reach-read-write = ler e escrever
software-reach-read-only = só ler
software-reach-nothing = nenhum — só o que você entregar
software-display-wayland = Wayland
software-display-x11 = X11, pelo Xwayland
software-category-audiovideo = som e vídeo
software-category-audio = som
software-category-video = vídeo
software-category-development = programação
software-category-education = aprendizado
software-category-game = jogos
software-category-graphics = imagens e desenho
software-category-network = a internet
software-category-office = escritório
software-category-science = ciência
software-category-settings = configurações
software-category-system = ferramentas do sistema
software-category-utility = utilitários
software-refusal-unfree-needs-consent = Não é software livre, e você ainda não permitiu.
software-refusal-unverified-needs-consent = Nada garante este download, e você não disse que confia nele.
software-refusal-big-needs-consent = É maior do que o limite para avisar, e você não disse sim.
software-refusal-insecure = Marcado como inseguro no nixpkgs; não se instala daqui.
software-refusal-broken = Quebrado nesta versão do nixpkgs; não compila.
software-refusal-unavailable = O nixpkgs tem, mas não para este tipo de computador.
software-refusal-source-absent = Este computador não tem o que essa fonte precisa.
software-refusal-source-unprepared = Essa fonte ainda não foi preparada.
software-refusal-declared-in-the-flake = Vem com o sistema (o flake); só o flake o muda.
software-refusal-arm-only = Feito só para chips de celular; não roda neste computador.
software-refusal-offline = Sem rede para perguntar.
software-refusal-rate-limited = O serviço pediu para esperar antes de perguntar de novo.
software-refusal-integrity = O que chegou não confere com a soma ou a assinatura, então foi descartado.
software-refusal-too-large = A resposta era maior do que qualquer coisa razoável para esse pedido.
software-refusal-not-an-appimage = Esse arquivo não é um AppImage.
software-refusal-legacy-appimage = Um AppImage antigo, do tipo 1, que este sistema não roda.
software-refusal-dwarfs-appimage = Um AppImage empacotado com DwarFS, que este sistema não abre.
software-refusal-not-an-apk = Esse arquivo não é um pacote Android.
software-refusal-url-scheme = Só endereços http:// e https:// viram web apps.
software-refusal-url-credentials = Um endereço com nome ou senha dentro não é guardado.
software-refusal-url-invalid = Isso não é um endereço.
software-refusal-duplicate-window = Outro web app já abre essa mesma janela nesse perfil.
software-refusal-no-browser = Web apps precisam do Chrome, que está desligado neste sistema.
software-refusal-profile-unknown = O Chrome não tem perfil com esse nome.
software-refusal-icon-unusable = Essa imagem não serve de ícone: ilegível, ou pequena demais.
software-refusal-profile-is-nix-env = Seu perfil Nix foi feito pelo antigo nix-env, que o nix profile se recusa a mudar.
software-refusal-conflict = Dois pacotes querem o mesmo arquivo no seu perfil Nix.
software-refusal-not-cancellable-now = Esta parte do trabalho não pode ser interrompida.
software-refusal-unknown-item = Nada com esse nome.
software-refusal-tool-failed = O programa que fazia isso falhou; as palavras dele estão na fila.
software-refusal-no-update-info = Não diz nada sobre atualizações; veja no site.
software-refusal-cannot-try = Só um pacote Nix roda sem instalar, e só um que tenha algo para rodar.
software-refusal-session-down = O Android não está rodando.
software-refusal-will-not-parse = O software.json não é legível, e escrever o substituiria.
software-refusal-wrong-user = Isso roda como outro usuário, não este.
software-not-in-launcher = Ainda não está no lançador; o lançador está lendo as pastas de novo.
software-nothing-to-open = Nada foi instalado desde que o rodapé começou.
software-no-such-level = “{ $words }” não nomeia nenhuma página dos programas.
software-no-copied-image = Nenhuma imagem foi copiada.
software-image-not-kept = Não foi possível gravar a imagem copiada num arquivo.
notif-software-app = programas
hint-software-trial = { $count ->
        [one] Um programa está em experiência, e nada dele está instalado. Clique para vê-lo.
       *[other] { $count } programas estão em experiência, e nada deles está instalado. Clique para vê-los.
    }
hint-software-android = O Android está rodando, e custa memória enquanto roda. Clique para o Android.
notif-software-installed-summary = { $app } está instalado
notif-software-installed-body = Já está no lançador, só para você.
notif-software-web-created-summary = { $app } está pronto
notif-software-web-created-body = O web app já está no lançador.
notif-software-removed-summary = { $app } foi removido
notif-software-removed-body = A entrada dele saiu do lançador.
notif-software-updated-summary = { $app } foi atualizado
notif-software-updated-body = A versão nova começa na próxima vez que ele abrir.
notif-software-failed-summary = { $app } não deu certo
notif-software-failed-body = { $why }
notif-software-trial-ended-summary = { $app } fechou
notif-software-trial-ended-body = Nada dele foi instalado; sai do disco na próxima limpeza.
notif-software-android-ready-summary = O Android está pronto
notif-software-android-ready-body = Os apps dentro dele se instalam pela página dele.
notif-action-software-launch-latest = Abrir
notif-action-transfers-panel = Abrir a fila
notif-action-software-maintenance = Ver as atualizações
notif-action-software-trials = Ver as experiências
notif-action-software-android = Abrir o Android

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

# The core's own words: why a row has no moment, why an action was refused,
# and why a module's journal is not being followed.
brain-why-quiet = Só quando pedida
brain-why-unknown = Não sabe fazer isso
brain-why-refused = A máquina recusou
brain-why-rehearsing = Este cérebro só ensaia: não muda nada na máquina
brain-journal-why-no-program = Este cérebro não recebeu o journal para ler
brain-journal-why-unstartable = Não foi possível abrir o leitor do journal
brain-journal-why-unreadable = Não foi possível ler o journal
brain-journal-why-ended = O journal deixou de ser seguido; vai tentar de novo

# THE BRAIN'S MODULES — what a notice answers to, on the strip under the
# minibuffer and on the brain's page; the page's notices, a watch, and the
# activity log folded.
cmd-brain-act = Fazer o que ele oferece
cmd-brain-dismiss = Dispensar
cmd-brain-snooze = Adiar
about-brain-act = Faz o que o aviso da frente oferece: a primeira ação do módulo que o levantou.
about-brain-dismiss = Dispensa o aviso da frente. Ele só volta se o módulo dele o levantar de novo.
about-brain-snooze = Adia o aviso da frente, pelo tempo que o primeiro adiamento dele diz.
brain-band-dismiss = dispensar
brain-band-snooze = { $minutes } min
brain-band-more = +{ $count }
hint-brain-more = todos os avisos, na página do Brain
echo-brain-no-notice = O cérebro não tem nada a dizer agora
echo-brain-not-offered = O aviso não oferece isso
echo-brain-no-action = Nenhum módulo do cérebro oferece isso
echo-brain-dismissed = Dispensado
echo-brain-snoozed = Adiado
echo-brain-taken = O cérebro aceitou
echo-brain-refused = O cérebro recusou: { $why }
brain-outcome-noted = Anotado
brain-notices = Avisos
brain-actions = O que ele oferece
brain-watching = Vigiando
brain-activity-last = Último:
brain-activity-first = Primeiro
brain-activity-times = Vezes
brain-since = Desde

# ICONS — the library the icons app and fast-access share (`ronoctua-iconset`).
# The words a person TYPES before a colon to narrow a search: the search
# language is interface, so every language's word is accepted.
icons-prefix-block = bloco
icons-prefix-tag = etiqueta
icons-prefix-token = token
icons-prefix-app = app
icons-prefix-symbols = simbolos
icons-prefix-family = familia
icons-prefix-category = categoria
icons-prefix-cat = cat
icons-prefix-name = nome
icons-prefix-group = grupo
icons-prefix-subgroup = subgrupo
icons-prefix-tone = tom
# The value words of the enumeration filters (`family:letter`), typed after
# the prefix. One word each, without accents: they are typed, not read.
icons-value-family-symbol = simbolo
icons-value-family-letter = letra
icons-value-family-number = numero
icons-value-family-mark = marca
icons-value-family-punctuation = pontuacao
icons-value-family-other = outro
icons-value-family-emoji = emoji
icons-value-group-faces = carinhas
icons-value-group-people = pessoas
icons-value-group-nature = natureza
icons-value-group-food = comida
icons-value-group-travel = viagem
icons-value-group-activities = atividades
icons-value-group-objects = objetos
icons-value-group-symbols = simbolos
icons-value-group-flags = bandeiras
icons-value-tone-light = claro
icons-value-tone-medium-light = medio-claro
icons-value-tone-medium = medio
icons-value-tone-medium-dark = medio-escuro
icons-value-tone-dark = escuro
# The panels' labels. Typed into the search, each finds its whole category —
# the generators put the same words into the tags.
icons-set-all = Todos os conjuntos
icons-set-cod = Codicons
icons-set-custom = Personalizados
icons-set-dev = Devicons
icons-set-extra = Extra
icons-set-fa = Font Awesome
icons-set-fae = Font Awesome Extension
icons-set-iec = Símbolos de energia IEC
icons-set-indent = Indentação
icons-set-indentation = Indentação
icons-set-linux = Logos de distribuições
icons-set-md = Material Design
icons-set-oct = Octicons
icons-set-pl = Powerline
icons-set-ple = Powerline Extra
icons-set-pom = Pomicons
icons-set-seti = Seti-UI
icons-set-weather = Clima
icons-category-all = Todas as categorias
icons-category-accessibility = Acessibilidade
icons-category-account = Conta
icons-category-animals = Animais
icons-category-arrows = Setas
icons-category-brands = Marcas
icons-category-buildings = Edifícios
icons-category-charts = Gráficos
icons-category-communication = Comunicação
icons-category-connectivity = Conectividade
icons-category-cursors = Cursores
icons-category-design = Design
icons-category-development = Desenvolvimento
icons-category-devices = Dispositivos
icons-category-emoji = Emoji
icons-category-files = Arquivos
icons-category-finance = Finanças
icons-category-food-beverage = Comida
icons-category-gaming = Jogos
icons-category-home = Casa
icons-category-layout = Layout
icons-category-mail = Correio
icons-category-math = Matemática
icons-category-medical = Médico
icons-category-multimedia = Multimídia
icons-category-nature = Natureza
icons-category-navigation = Navegação
icons-category-notifications = Notificações
icons-category-people = Pessoas
icons-category-photography = Fotografia
icons-category-science = Ciência
icons-category-seasons = Estações
icons-category-security = Segurança
icons-category-shapes = Formas
icons-category-shopping = Compras
icons-category-social = Social
icons-category-sports = Esportes
icons-category-sustainability = Sustentabilidade
icons-category-text = Texto
icons-category-time = Tempo
icons-category-tools = Ferramentas
icons-category-transportation = Transporte
icons-category-travel = Viagem
icons-category-weather = Clima
icons-family-all = Todas as famílias
icons-family-symbol = Símbolo
icons-family-letter = Letra
icons-family-number = Número
icons-family-mark = Marca
icons-family-punctuation = Pontuação
icons-family-other = Outro
icons-family-emoji = Emoji
icons-group-faces = Carinhas e emoções
icons-group-people = Pessoas e corpo
icons-group-nature = Animais e natureza
icons-group-food = Comida e bebida
icons-group-travel = Viagem e lugares
icons-group-activities = Atividades
icons-group-objects = Objetos
icons-group-symbols = Símbolos
icons-group-flags = Bandeiras
icons-tone-light = Pele clara
icons-tone-medium-light = Pele clara média
icons-tone-medium = Pele média
icons-tone-medium-dark = Pele escura média
icons-tone-dark = Pele escura
# Why there are no icons to show.
icons-why-no-variable = As tabelas de ícones não foram entregues a este programa (RONOCTUA_ICONSET não está definida).
icons-why-unreadable = Uma tabela de ícones falta ou não pode ser lida: { $file }.
icons-why-garbled = Uma tabela de ícones não está no formato que o gerador escreve: { $file }.
# THE CALCULATOR'S ENGINE (`ronoctua-calc`): why an expression has no value,
# the converter's categories, the shapes of a quick answer, and every unit's
# name — chosen by the number (`$count`, a number), as `assets/units.nix` declares.
calc-why-empty = Não há nada para calcular.
calc-why-unexpected = Algo aqui está fora do lugar.
calc-why-unbalanced = Um parêntese foi aberto e não fechado, ou fechado sem ter sido aberto.
calc-why-unknown-word = Uma palavra que não é unidade, moeda, função nem fuso horário.
calc-why-incompatible = São grandezas de tipos diferentes e não se combinam.
calc-why-divide-by-zero = Uma divisão por zero.
calc-why-too-large = O resultado é grande demais para caber.
calc-why-domain = A função não tem valor aí.
calc-why-no-rates = Converter entre moedas precisa das cotações, e ainda não há nenhuma.
calc-why-unknown-currency = As cotações não trazem esta moeda.
calc-why-unknown-zone = Nenhum fuso horário tem esse nome.
calc-why-bad-date = Essa data não existe.
calc-why-bad-roman = Isso não é um número romano (de 1 a 3999, na forma usual).
calc-why-bad-digit = Um algarismo que a base do número não tem.
calc-rates-why-wrong-pass = A permissão dada é para outro site, não o do Banco Central Europeu.
calc-rates-why-unreachable = Não foi possível alcançar o Banco Central Europeu.
calc-rates-why-failed = O Banco Central Europeu respondeu com uma falha.
calc-rates-why-garbled = O que voltou não é o arquivo de cotações do Banco Central Europeu.
calc-category-length = Comprimento
calc-category-mass = Massa
calc-category-volume = Volume
calc-category-area = Área
calc-category-temperature = Temperatura
calc-category-time = Tempo
calc-category-speed = Velocidade
calc-category-data = Dados
calc-category-energy = Energia
calc-category-power = Potência
calc-category-pressure = Pressão
calc-category-angle = Ângulo
calc-category-frequency = Frequência
calc-category-fuel-economy = Consumo de combustível
calc-category-cooking = Culinária
calc-shape-arithmetic = Aritmética
calc-shape-conversion = Conversão
calc-shape-percentage = Porcentagem
calc-shape-base = Base numérica
calc-shape-roman = Números romanos
calc-shape-date = Data
calc-shape-zone = Fuso horário
calc-shape-currency = Moeda
unit-metre = { $count ->
        [one] metro
       *[other] metros
    }
unit-kilometre = { $count ->
        [one] quilômetro
       *[other] quilômetros
    }
unit-centimetre = { $count ->
        [one] centímetro
       *[other] centímetros
    }
unit-millimetre = { $count ->
        [one] milímetro
       *[other] milímetros
    }
unit-micrometre = { $count ->
        [one] micrômetro
       *[other] micrômetros
    }
unit-nanometre = { $count ->
        [one] nanômetro
       *[other] nanômetros
    }
unit-mile = { $count ->
        [one] milha
       *[other] milhas
    }
unit-yard = { $count ->
        [one] jarda
       *[other] jardas
    }
unit-foot = { $count ->
        [one] pé
       *[other] pés
    }
unit-inch = { $count ->
        [one] polegada
       *[other] polegadas
    }
unit-nautical-mile = { $count ->
        [one] milha náutica
       *[other] milhas náuticas
    }
unit-astronomical-unit = { $count ->
        [one] unidade astronômica
       *[other] unidades astronômicas
    }
unit-light-year = { $count ->
        [one] ano-luz
       *[other] anos-luz
    }
unit-kilogram = { $count ->
        [one] quilograma
       *[other] quilogramas
    }
unit-gram = { $count ->
        [one] grama
       *[other] gramas
    }
unit-milligram = { $count ->
        [one] miligrama
       *[other] miligramas
    }
unit-microgram = { $count ->
        [one] micrograma
       *[other] microgramas
    }
unit-tonne = { $count ->
        [one] tonelada
       *[other] toneladas
    }
unit-pound = { $count ->
        [one] libra
       *[other] libras
    }
unit-ounce = { $count ->
        [one] onça
       *[other] onças
    }
unit-stone = { $count ->
        [one] stone
       *[other] stones
    }
unit-carat = { $count ->
        [one] quilate
       *[other] quilates
    }
unit-short-ton = { $count ->
        [one] tonelada curta
       *[other] toneladas curtas
    }
unit-long-ton = { $count ->
        [one] tonelada longa
       *[other] toneladas longas
    }
unit-cubic-metre = { $count ->
        [one] metro cúbico
       *[other] metros cúbicos
    }
unit-litre = { $count ->
        [one] litro
       *[other] litros
    }
unit-decilitre = { $count ->
        [one] decilitro
       *[other] decilitros
    }
unit-centilitre = { $count ->
        [one] centilitro
       *[other] centilitros
    }
unit-millilitre = { $count ->
        [one] mililitro
       *[other] mililitros
    }
unit-cubic-centimetre = { $count ->
        [one] centímetro cúbico
       *[other] centímetros cúbicos
    }
unit-gallon = { $count ->
        [one] galão
       *[other] galões
    }
unit-imperial-gallon = { $count ->
        [one] galão imperial
       *[other] galões imperiais
    }
unit-quart = { $count ->
        [one] quarto de galão
       *[other] quartos de galão
    }
unit-pint = { $count ->
        [one] pinta
       *[other] pintas
    }
unit-fluid-ounce = { $count ->
        [one] onça líquida
       *[other] onças líquidas
    }
unit-cubic-foot = { $count ->
        [one] pé cúbico
       *[other] pés cúbicos
    }
unit-cubic-inch = { $count ->
        [one] polegada cúbica
       *[other] polegadas cúbicas
    }
unit-square-metre = { $count ->
        [one] metro quadrado
       *[other] metros quadrados
    }
unit-square-kilometre = { $count ->
        [one] quilômetro quadrado
       *[other] quilômetros quadrados
    }
unit-square-centimetre = { $count ->
        [one] centímetro quadrado
       *[other] centímetros quadrados
    }
unit-square-millimetre = { $count ->
        [one] milímetro quadrado
       *[other] milímetros quadrados
    }
unit-hectare = { $count ->
        [one] hectare
       *[other] hectares
    }
unit-acre = { $count ->
        [one] acre
       *[other] acres
    }
unit-square-foot = { $count ->
        [one] pé quadrado
       *[other] pés quadrados
    }
unit-square-inch = { $count ->
        [one] polegada quadrada
       *[other] polegadas quadradas
    }
unit-square-yard = { $count ->
        [one] jarda quadrada
       *[other] jardas quadradas
    }
unit-square-mile = { $count ->
        [one] milha quadrada
       *[other] milhas quadradas
    }
unit-kelvin = { $count ->
        [one] kelvin
       *[other] kelvins
    }
unit-celsius = { $count ->
        [one] grau Celsius
       *[other] graus Celsius
    }
unit-fahrenheit = { $count ->
        [one] grau Fahrenheit
       *[other] graus Fahrenheit
    }
unit-rankine = { $count ->
        [one] grau Rankine
       *[other] graus Rankine
    }
unit-second = { $count ->
        [one] segundo
       *[other] segundos
    }
unit-millisecond = { $count ->
        [one] milissegundo
       *[other] milissegundos
    }
unit-microsecond = { $count ->
        [one] microssegundo
       *[other] microssegundos
    }
unit-nanosecond = { $count ->
        [one] nanossegundo
       *[other] nanossegundos
    }
unit-minute = { $count ->
        [one] minuto
       *[other] minutos
    }
unit-hour = { $count ->
        [one] hora
       *[other] horas
    }
unit-day = { $count ->
        [one] dia
       *[other] dias
    }
unit-week = { $count ->
        [one] semana
       *[other] semanas
    }
unit-month = { $count ->
        [one] mês
       *[other] meses
    }
unit-year = { $count ->
        [one] ano
       *[other] anos
    }
unit-metre-per-second = { $count ->
        [one] metro por segundo
       *[other] metros por segundo
    }
unit-kilometre-per-hour = { $count ->
        [one] quilômetro por hora
       *[other] quilômetros por hora
    }
unit-mile-per-hour = { $count ->
        [one] milha por hora
       *[other] milhas por hora
    }
unit-knot = { $count ->
        [one] nó
       *[other] nós
    }
unit-foot-per-second = { $count ->
        [one] pé por segundo
       *[other] pés por segundo
    }
unit-bit = { $count ->
        [one] bit
       *[other] bits
    }
unit-byte = { $count ->
        [one] byte
       *[other] bytes
    }
unit-kilobit = { $count ->
        [one] quilobit
       *[other] quilobits
    }
unit-megabit = { $count ->
        [one] megabit
       *[other] megabits
    }
unit-gigabit = { $count ->
        [one] gigabit
       *[other] gigabits
    }
unit-kibibit = { $count ->
        [one] kibibit
       *[other] kibibits
    }
unit-mebibit = { $count ->
        [one] mebibit
       *[other] mebibits
    }
unit-kilobyte = { $count ->
        [one] quilobyte
       *[other] quilobytes
    }
unit-megabyte = { $count ->
        [one] megabyte
       *[other] megabytes
    }
unit-gigabyte = { $count ->
        [one] gigabyte
       *[other] gigabytes
    }
unit-terabyte = { $count ->
        [one] terabyte
       *[other] terabytes
    }
unit-petabyte = { $count ->
        [one] petabyte
       *[other] petabytes
    }
unit-kibibyte = { $count ->
        [one] kibibyte
       *[other] kibibytes
    }
unit-mebibyte = { $count ->
        [one] mebibyte
       *[other] mebibytes
    }
unit-gibibyte = { $count ->
        [one] gibibyte
       *[other] gibibytes
    }
unit-tebibyte = { $count ->
        [one] tebibyte
       *[other] tebibytes
    }
unit-pebibyte = { $count ->
        [one] pebibyte
       *[other] pebibytes
    }
unit-joule = { $count ->
        [one] joule
       *[other] joules
    }
unit-kilojoule = { $count ->
        [one] quilojoule
       *[other] quilojoules
    }
unit-megajoule = { $count ->
        [one] megajoule
       *[other] megajoules
    }
unit-calorie = { $count ->
        [one] caloria
       *[other] calorias
    }
unit-kilocalorie = { $count ->
        [one] quilocaloria
       *[other] quilocalorias
    }
unit-watt-hour = { $count ->
        [one] watt-hora
       *[other] watts-hora
    }
unit-kilowatt-hour = { $count ->
        [one] quilowatt-hora
       *[other] quilowatts-hora
    }
unit-electronvolt = { $count ->
        [one] elétron-volt
       *[other] elétrons-volt
    }
unit-british-thermal-unit = { $count ->
        [one] unidade térmica britânica
       *[other] unidades térmicas britânicas
    }
unit-watt = { $count ->
        [one] watt
       *[other] watts
    }
unit-kilowatt = { $count ->
        [one] quilowatt
       *[other] quilowatts
    }
unit-megawatt = { $count ->
        [one] megawatt
       *[other] megawatts
    }
unit-horsepower = horsepower
unit-metric-horsepower = { $count ->
        [one] cavalo-vapor
       *[other] cavalos-vapor
    }
unit-pascal = { $count ->
        [one] pascal
       *[other] pascals
    }
unit-hectopascal = { $count ->
        [one] hectopascal
       *[other] hectopascals
    }
unit-kilopascal = { $count ->
        [one] quilopascal
       *[other] quilopascals
    }
unit-megapascal = { $count ->
        [one] megapascal
       *[other] megapascals
    }
unit-bar = { $count ->
        [one] bar
       *[other] bares
    }
unit-millibar = { $count ->
        [one] milibar
       *[other] milibares
    }
unit-atmosphere = { $count ->
        [one] atmosfera
       *[other] atmosferas
    }
unit-pound-per-square-inch = { $count ->
        [one] libra por polegada quadrada
       *[other] libras por polegada quadrada
    }
unit-millimetre-of-mercury = { $count ->
        [one] milímetro de mercúrio
       *[other] milímetros de mercúrio
    }
unit-torr = { $count ->
        [one] torr
       *[other] torrs
    }
unit-radian = { $count ->
        [one] radiano
       *[other] radianos
    }
unit-degree = { $count ->
        [one] grau
       *[other] graus
    }
unit-gradian = { $count ->
        [one] grado
       *[other] grados
    }
unit-arcminute = { $count ->
        [one] minuto de arco
       *[other] minutos de arco
    }
unit-arcsecond = { $count ->
        [one] segundo de arco
       *[other] segundos de arco
    }
unit-turn = { $count ->
        [one] volta
       *[other] voltas
    }
unit-hertz = hertz
unit-kilohertz = quilohertz
unit-megahertz = megahertz
unit-gigahertz = gigahertz
unit-revolution-per-minute = { $count ->
        [one] rotação por minuto
       *[other] rotações por minuto
    }
unit-litre-per-100-km = { $count ->
        [one] litro por 100 quilômetros
       *[other] litros por 100 quilômetros
    }
unit-kilometre-per-litre = { $count ->
        [one] quilômetro por litro
       *[other] quilômetros por litro
    }
unit-mile-per-gallon = { $count ->
        [one] milha por galão
       *[other] milhas por galão
    }
unit-mile-per-imperial-gallon = { $count ->
        [one] milha por galão imperial
       *[other] milhas por galão imperial
    }
unit-cup = { $count ->
        [one] xícara
       *[other] xícaras
    }
unit-customary-cup = { $count ->
        [one] xícara americana
       *[other] xícaras americanas
    }
unit-metric-cup = { $count ->
        [one] xícara métrica
       *[other] xícaras métricas
    }
unit-tablespoon = { $count ->
        [one] colher de sopa
       *[other] colheres de sopa
    }
unit-customary-tablespoon = { $count ->
        [one] colher de sopa americana
       *[other] colheres de sopa americanas
    }
unit-teaspoon = { $count ->
        [one] colher de chá
       *[other] colheres de chá
    }
unit-customary-teaspoon = { $count ->
        [one] colher de chá americana
       *[other] colheres de chá americanas
    }

# As recusas do player que o protocolo 3 trouxe.
player-trouble-already-queued = Isto já está na fila: { $name }.
player-trouble-changed-on-disk = O arquivo da fila mudou no disco desde que foi aberto ou salvo: { $path }
player-trouble-no-origin = Esta fila nunca foi aberta de um arquivo nem salva como um, então não há onde salvá-la de novo.
player-trouble-no-sound = Este arquivo não tem som para tocar.
player-trouble-not-yet = O player ainda não faz isto.
player-trouble-said = { $program } disse: { $text }
# Um texto de várias linhas, editado no lugar (as duas portas).
text-area-keep = guardar
about-text-area-keep = Guardar o que está escrito aqui.
text-area-discard = sair
about-text-area-discard = Sair do texto; se ele mudou e não foi guardado, você é perguntado antes.
ask-text-area-discard = Jogar fora o que foi escrito e não foi guardado?
text-area-no = Não, continuar escrevendo
text-area-yes = Sim, jogar fora
about-text-area = Um texto de várias linhas, digitado e mudado aqui.
text-area-not-kept = Nada guarda este texto aqui ainda; ele continua aberto como foi escrito.

# THE LEADER'S SETS AND THE BARS' MODES (2026-09-30): the leader pressed again
# walks the sets, and a tap of Ctrl at the root walks the bars' three modes.
leader-set-default = padrão
cmd-leader-set-default = As teclas como são, cada uma abrindo o seu app no rodapé
cmd-footer-bars-cycle = O próximo dos três modos das barras: à mostra, escondidas empurrando as janelas, escondidas por cima delas
cmd-leader-close = Fechar a lista da leader, como o Esc dela faz
echo-bars-shown = Modeline e minibuffer à mostra
echo-bars-push = Modeline e minibuffer escondidos; o que abre empurra as janelas
echo-bars-cover = Modeline e minibuffer escondidos; o que abre fica por cima das janelas
# THE MINIBUFFER'S DRAWER by name, and the compositor's transient keys for it.
cmd-minibuffer-fold-open = Abrir a linha do minibuffer para ler o resto, ou descer por ela
cmd-minibuffer-fold-close = Subir pela linha aberta do minibuffer, ou fechá-la
cmd-wm-fold-keys-hold = Deixar Ctrl com as setas andar pela linha do minibuffer enquanto uma dica longa demais para ela está nela
cmd-wm-fold-keys-free = Devolver Ctrl com as setas às janelas
# THE MAXIMUM ALERT BAR (alert.rs). The title is drawn in capitals.
alert-battery-low = Bateria fraca!!!
alert-battery-low-body = Restam { $percent }%, na bateria, e o carregador não está ligado.
hint-alert-close = Fecha este alerta; ele volta se a bateria cair mais um degrau
cmd-alert-dismiss = Fechar a barra de alerta; ela volta se o que a causou piorar
# A PREFERENCE THAT COULD NOT BE SAVED is said, never only logged.
echo-preference-not-written = Não foi possível gravar { $file } ({ $why }): o que você mudou vale só até o rodapé reiniciar.
# In a window the change is not in force anywhere: the file is the only road.
echo-setting-not-written = Não foi possível gravar { $file } ({ $why }), e nada mudou.
# THE FOOTER'S SETTINGS (LEADER s r, footer_settings.rs).
group-footer-settings = rodapé
# The modeline's name for its window (`br.ronoctua.footer-settings`).
mode-footer-settings = ajustes do rodapé
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

player-channels-education = Educação
player-channels-space = Espaço
player-channels-history = História
player-channels-nature = Natureza e animais
player-channels-art = Arte e dança
player-channels-gaming = Jogos
player-channels-audiobooks = Audiolivros
player-channels-source-iptv = Canais de TV do iptv-org
player-channels-source-archive = Internet Archive
player-channels-source-lives = Ao vivo no YouTube
player-channels-source-nasa = Biblioteca de imagens e vídeos da NASA
player-channels-source-commons = Wikimedia Commons
player-channels-source-peertube = PeerTube, pelo SepiaSearch

# A MODULE THE SYSTEM SWITCHED OFF (`ronoctua.brain.modules.<id>.enable =
# false`): its row's reason, and what every ask about it is refused with — a
# row's reason sits in a column sixteen cells wide.
brain-why-system-off = Sistema desligou
# And the date-and-time page's line under the clock watch's switch, when the
# system switched that watch off.
locale-watched-system-off = A configuração do sistema desligou esta vigia, então ela não liga daqui; é a configuração que a liga de volta.

## file picker — the one picker of a file, a folder or a name to save under
file-picker-filter-any = Todo arquivo
file-picker-filter-images = Imagens
file-picker-filter-media = O que o player toca
file-picker-filter-lanes = O que toca junto
file-picker-filter-saved = Conjuntos e filas
file-picker-filter-subtitles = Legendas
file-picker-filter-packages = Programas para instalar
file-picker-filter-tunnels = Túneis (.conf, .ovpn)
file-picker-row-play-folder = Tocar esta pasta
file-picker-row-enqueue-folder = Pôr esta pasta na fila
file-picker-ask-overwrite = { $name } já existe em { $folder }. Gravar por cima?
file-picker-answer-no = Não
file-picker-answer-yes = Sim, gravar por cima
file-picker-type-note = Digite para estreitar a pasta; comece com / ou ~ para digitar um caminho.
file-picker-places = Lugares
file-picker-refused = Esta pasta não pôde ser lida: { $why }
file-picker-empty = Nada aqui para escolher, e nenhuma pasta.
file-picker-no-match = Nada aqui contém “{ $typed }”.
file-picker-name = Nome
file-picker-typed = Digitado
file-picker-shows = Mostra
file-picker-here = Escolher esta pasta
file-picker-take = { $count ->
    [one] Levar o arquivo marcado
   *[other] Levar os { $count } arquivos marcados
  }
file-picker-home = Pasta pessoal
file-picker-no-address = Esta página não aceita um endereço da web.
file-picker-nothing-there = Não há nada em { $typed }.
file-picker-no-name = Digite um nome antes.
file-picker-bad-name = “{ $name }” não pode ser o nome de um arquivo.
file-picker-is-a-folder = { $name } é uma pasta.
file-picker-down = descer
file-picker-up = subir
file-picker-left = esquerda
file-picker-right = direita
file-picker-enter = abrir ou escolher
file-picker-play-now = tocar já
file-picker-mark = marcar
file-picker-erase = apagar
file-picker-parent = pasta de cima
file-picker-filter = o que mostra
file-picker-play-folder = tocar a pasta
file-picker-enqueue-folder = pasta na fila
file-picker-paste = colar
file-picker-hidden = ocultos
about-file-picker-down = A próxima linha — numa grade, a célula abaixo desta
about-file-picker-up = A linha anterior — numa grade, a célula acima desta
about-file-picker-left = A célula anterior, numa grade
about-file-picker-right = A próxima célula, numa grade
about-file-picker-enter = Abrir a pasta, ou escolher o arquivo, sob o cursor
about-file-picker-play-now = Escolher para tocar agora, logo depois do que toca; a fila fica
about-file-picker-mark = Marcar o arquivo sob o cursor, ou desmarcar, para levar vários de uma vez
about-file-picker-erase = Tirar a última letra do que foi digitado
about-file-picker-parent = Subir para a pasta de cima
about-file-picker-filter = Trocar o que a pasta mostra: os arquivos da página, ou todo arquivo
about-file-picker-play-folder = Tocar a pasta aberta, tudo o que há nela
about-file-picker-enqueue-folder = Pôr a pasta aberta na fila, tudo o que há nela
about-file-picker-paste = Digitar o que foi copiado — um caminho, ou um endereço file://
about-file-picker-hidden = Mostrar os arquivos ocultos, ou escondê-los de novo
player-page-pick-save-set = Salvar o que toca como conjunto
player-page-pick-save-queue = Salvar a fila
player-added-several = { $count } arquivos postos na fila.
player-playing-several = Tocando { $count } arquivos agora.
syncthing-pick-folder = Uma pasta desta máquina
network-pick-tunnel = Um túnel para trazer
software-pick-package = Um arquivo para instalar — um AppImage, um APK ou uma referência Flatpak
camera-pick-backdrops = Imagens para trás de você: as setas passam pelas escolhidas

## file info editing — everything the info panel can change
files-detail-name = nome
files-fact-edits = { $label }: { $value } — pressione para mudar
files-fact-takes-place-out = { $label }: { $value } — pressione para tirar o lugar do arquivo
files-meta-title = título
files-meta-author = autoria
files-meta-description = descrição
files-meta-keywords = palavras-chave
files-meta-copyright = direitos autorais
files-meta-taken = feito em
files-fact-tags = etiquetas
files-fact-details = detalhes
files-tags-absent = o leitor de etiquetas não está instalado aqui
files-tags-stopped = o leitor de etiquetas parou antes de responder
files-tag-cannot = { $label } não pode ser mudado aqui: { $why }
files-tag-several-lines = { $label } tem várias linhas; ele é editado no editor do player, não numa linha só
files-ask-modified = Modificado em (ano-mês-dia hora:minuto:segundo):
files-modified-bad = Isso não é uma data que eu consiga ler. Escreva assim: 2026-09-30 14:05, os segundos são opcionais.
files-ask-tag = { $field }:
files-ask-meta = { $detail }:
files-ask-location = { $n ->
        [one] Tirar o lugar de { $name }? Isso reescreve o arquivo e não se desfaz.
       *[other] Tirar o lugar destes { $n } arquivos? Isso reescreve cada um e não se desfaz.
    }
files-location-copy = escrever uma cópia sem o lugar, ao lado
files-location-overwrite = tirar do próprio arquivo — não se desfaz
files-location-copied = { $n ->
        [one] Uma cópia sem o lugar: { $name }
       *[other] { $n } cópias sem o lugar, cada uma ao lado do original
    }
files-location-removed = { $n ->
        [one] O lugar saiu de { $name }.
       *[other] O lugar saiu de { $n } arquivos.
    }
files-amend-too-long = Isso é mais longo do que um detalhe pode ser ({ $most } bytes); nada foi escrito.
files-amend-bad-date = Isso não é uma data: escreva assim: 2026-09-30 14:05. Nada foi escrito.
files-amend-not-written = O exiftool não escreveu nada: este arquivo não aceita isso aqui. Ele continua como estava.
files-amend-taken = Já existe um arquivo com o nome da cópia; nada foi escrito.
files-remove-location = tirar o lugar onde foi feito
files-does-remove-location = pergunta antes; depois reescreve o arquivo — ou escreve uma cópia ao lado — sem a coordenada de GPS. Tirado do próprio arquivo, não se desfaz
files-trash-on-volume = Indo para a lixeira do próprio { $volume }, não para a da pasta pessoal: o espaço continua ocupado em { $volume } até essa lixeira o liberar — o que ela faz sozinha, como a da pasta pessoal, enquanto o disco está conectado.
files-trash-left-on-volume = { $volume } saiu com arquivos na própria lixeira ({ $n }): o espaço deles continua ocupado nele até essa lixeira os liberar, o que só acontece enquanto ele está conectado.

# A LINHA DO GIT sob o caminho do gerenciador de arquivos, e o navegador git
# que o `C-h` abre (`git_line.rs`, `git_browser.rs`). O que vem entre aspas é
# dado: um ramo, um assunto, um caminho.
files-git = histórico
# A janela do navegador na modeline (`br.ronoctua.git`, `daemon::mode_label`).
mode-git = histórico do git
about-files-git = Abre o histórico do repositório: as mudanças, os commits, os ramos, os stashes e os remotos, ao lado do que cada um mudou.
files-git-title = Histórico de { $name }
files-git-detached = solto em { $id }
files-git-unborn = { $branch }, ainda sem commits
files-git-changed = { $n ->
        [one] 1 mudado
       *[other] { $n } mudados
    }
files-git-staged = { $n ->
        [one] 1 preparado
       *[other] { $n } preparados
    }
files-git-untracked = { $n ->
        [one] 1 não rastreado
       *[other] { $n } não rastreados
    }
files-git-conflicted = { $n ->
        [one] 1 em conflito
       *[other] { $n } em conflito
    }
files-git-clean = nada mudou
files-git-list-item = { $item },
files-git-subject = “{ $subject }”
files-git-line-explain = Em { $branch }: { $changed } mudados, { $staged } preparados, { $untracked } não rastreados. { $keys } ou um clique abre o histórico.
files-git-line-refused = Esta pasta está num repositório que o git não quis ler. { $keys } abre o histórico, que diz por quê.
files-git-age-now = agora mesmo
files-git-age-minutes = { $n ->
        [one] há um minuto
       *[other] há { $n } minutos
    }
files-git-age-hours = { $n ->
        [one] há uma hora
       *[other] há { $n } horas
    }
files-git-age-days = { $n ->
        [one] ontem
       *[other] há { $n } dias
    }
files-git-age-months = { $n ->
        [one] há um mês
       *[other] há { $n } meses
    }
files-git-age-years = { $n ->
        [one] há um ano
       *[other] há { $n } anos
    }
# Por que o git não respondeu (`ronoctua_git::Why`), uma por motivo.
files-git-why-no-git = Esta sessão não recebeu o git, então o repositório não pode ser lido.
files-git-why-no-ssh = Este remoto é alcançado por SSH e esta sessão não recebeu o ssh.
files-git-why-no-askpass = Este remoto pede um token e falta o ajudante que o entrega.
files-git-why-not-a-repo = Esta pasta não está num repositório.
files-git-why-unstartable = O git não pôde ser iniciado.
files-git-why-failed = O git não conseguiu ler isto.
files-git-why-garbled = O git respondeu algo que este navegador não entende.
files-git-why-needs-credentials = O remoto pede um nome e uma senha ou uma chave, e buscar nunca pede nenhum dos dois.
files-git-why-refused = O remoto recusou as credenciais que recebeu.
files-git-why-unreachable = O remoto não pôde ser alcançado.
files-git-why-too-large = É maior do que uma vista mostra.
# As colunas do navegador.
files-git-kind-changes = Mudanças
files-git-kind-history = Histórico
files-git-kind-refs = Ramos e etiquetas
files-git-kind-stashes = Stashes
files-git-kind-remotes = Remotos
files-git-kind-count = { $kind } { $n }
files-git-file-history-title = Histórico de { $path }
files-git-reading = Lendo…
files-git-empty-changes = Nada mudou na árvore de trabalho.
files-git-empty-history = Ainda não há commits.
files-git-empty-refs = Nenhum ramo e nenhuma etiqueta.
files-git-empty-stashes = Nada guardado no stash.
files-git-empty-remotes = Nenhum remoto.
files-git-empty-file = Nenhum commit mexeu neste arquivo.
files-git-pick-item = Escolha algo à esquerda para ver aqui.
files-git-pick-file = Escolha um arquivo para ver o que mudou nele.
files-git-diff-no-files = Um remoto não muda arquivo nenhum.
files-git-diff-nothing = O conteúdo não mudou.
files-git-diff-staged = Preparado
files-git-diff-unstaged = Não preparado
files-git-diff-untracked = Ainda fora do repositório
files-git-diff-binary = Não é texto: não há o que mostrar linha a linha.
files-git-diff-no-content = Só os detalhes do arquivo mudaram, não o que há nele.
files-git-diff-link = Um link simbólico, e para onde ele aponta:
files-git-diff-special = Não é um arquivo — é um pipe, um socket ou um dispositivo —, então nada nele é lido.
files-git-blame-title = Quem escreveu cada linha de { $path }
files-git-files = { $n ->
        [one] 1 arquivo mudado
       *[other] { $n } arquivos mudados
    }
files-git-remote-branches = Os ramos dele, da última busca
files-git-no-remote-branches = Nenhum buscado ainda.
files-git-fact-path = arquivo
files-git-fact-state = estado
files-git-fact-from = renomeado de
files-git-fact-name = nome
files-git-fact-kind = tipo
files-git-fact-upstream = segue
files-git-fact-fetch = busca de
files-git-fact-push = envia para
files-git-fact-stash = stash
files-git-fact-commit = commit
files-git-fact-author = autoria
files-git-fact-date = data
files-git-fact-parents = pais
files-git-ref-branch = um ramo
files-git-ref-remote = um ramo de remoto
files-git-ref-tag = uma etiqueta
files-git-mark-branch = ramo
files-git-mark-remote = remoto
files-git-mark-tag = etiqueta
files-git-change-added = acrescentado
files-git-change-modified = mudado
files-git-change-deleted = apagado
files-git-change-renamed = renomeado
files-git-change-copied = copiado
files-git-change-type = mudou de tipo
files-git-change-unmerged = não mesclado
files-git-state-untracked = ainda fora do repositório
files-git-state-conflicted = em conflito
files-git-state-staged = preparado: { $what }
files-git-state-unstaged = não preparado: { $what }
# O que o navegador diz depois de uma ação.
files-git-fetched = Buscado: os ramos dos remotos estão em dia.
files-git-fetch-refused = Nada foi buscado. { $why }
files-git-fetching = Já há uma busca em andamento.
files-git-no-file = Não há arquivo sob o cursor.
files-git-blame-gone = O arquivo não está mais na árvore de trabalho, então ninguém escreveu as linhas dele ali.
files-git-nothing-to-copy = Não há commit sob o cursor.
files-git-no-version = Não há versão anterior deste arquivo para abrir.
files-git-kept = Aberto { $path } como { $id } o tem — uma cópia que não pode ser mudada.
files-git-keep-failed = A cópia não pôde ser feita: { $why }
files-git-keep-why-disk = O disco não a aceitou: está cheio, ou o cache não pode ser escrito ali.
# As teclas do navegador: a palavra que a linha de atalhos mostra, e o que o
# ponteiro parado sobre ela ouve.
files-git-down = descer
files-git-up = subir
files-git-enter = abrir
files-git-next = próxima coluna
files-git-previous = coluna anterior
files-git-page-down = página abaixo
files-git-page-up = página acima
files-git-changes = mudanças
files-git-history = histórico
files-git-refs = ramos
files-git-stashes = stashes
files-git-remotes = remotos
files-git-file-history = histórico do arquivo
files-git-blame = quem escreveu
files-git-copy = copiar id
files-git-open-version = abrir esta versão
files-git-fetch = buscar
files-git-terminal = terminal
files-git-left = esquerda
files-git-right = direita
about-files-git-down = Desce o cursor uma linha; no diff, move o texto uma linha.
about-files-git-up = Sobe o cursor uma linha; no diff, move o texto uma linha.
about-files-git-enter = Entra uma coluna: de um item para o que ele é, de um arquivo para o que mudou nele.
about-files-git-next = Dá o teclado à coluna da direita.
about-files-git-previous = Dá o teclado à coluna da esquerda.
about-files-git-page-down = Desce uma tela inteira na coluna que está com o teclado.
about-files-git-page-up = Sobe uma tela inteira na coluna que está com o teclado.
about-files-git-changes = Lista o que a árvore de trabalho tem que o último commit não tem.
about-files-git-history = Lista os commits, do mais novo ao mais antigo.
about-files-git-refs = Lista os ramos, os ramos dos remotos e as etiquetas.
about-files-git-stashes = Lista o que foi guardado no stash.
about-files-git-remotes = Lista os remotos e onde cada um está.
about-files-git-file-history = Lista só os commits que mudaram o arquivo sob o cursor, atravessando as renomeações.
about-files-git-blame = Mostra quem escreveu por último cada linha do arquivo como a árvore de trabalho o tem; a mesma tecla volta ao diff.
about-files-git-copy = Copia o id do commit sob o cursor.
about-files-git-open-version = Abre o arquivo como o commit o tinha, numa cópia que não pode ser mudada.
about-files-git-fetch = Traz para cá os commits novos dos remotos, sem mudar nenhum dos seus. Nunca pede senha.
about-files-git-terminal = Abre um terminal na pasta de cima do repositório.
about-files-git-left = Move o texto do diff para a esquerda.
about-files-git-right = Move o texto do diff para a direita, para ler uma linha longa.

# The archive rows of the file manager's menu (`ronoctua_files::menu`).
files-archive-test = testar o arquivo compactado
files-archive-convert = converter para outro formato…
files-archive-add = pôr o que foi copiado neste arquivo compactado
files-archive-extract = extrair aqui, ao lado do arquivo compactado
files-archive-remove = tirar do arquivo compactado
files-does-archive-test = lê cada membro e confere com as somas (7zz t, unrar t, bsdtar -t), pela fila
files-does-archive-convert = extrai numa pasta oculta ao lado e compacta de novo no formato escolhido na ficha
files-does-archive-add = 7zz a: o que foi copiado entra no topo do arquivo, cifrado com a senha dele quando tem uma
files-does-archive-extract = tira os membros escolhidos para a pasta do próprio arquivo; nada lá é substituído
files-does-archive-remove = 7zz d: reescreve o arquivo sem eles — não tem volta
# An archive's formats and levels, by `ronoctua_archive::Format::id` and `Level::id`.
files-archive-zip = zip
files-archive-7z = 7z
files-archive-tar = tar
files-archive-tar-gz = tar.gz
files-archive-tar-xz = tar.xz
files-archive-tar-zst = tar.zst
files-archive-tar-bz2 = tar.bz2
files-archive-tar-lz4 = tar.lz4
files-archive-rar = RAR
files-archive-iso = imagem ISO
files-archive-other = outro formato
files-archive-level-store = sem compressão
files-archive-level-fastest = o mais rápido
files-archive-level-normal = normal
files-archive-level-maximum = máximo
files-archive-level-ultra = ultra
# Why an archive's work did not happen (`ronoctua_archive::Why`).
files-archive-why-no-tool = { $name }: esta sessão não recebeu o programa de que este arquivo precisa
files-archive-why-unsupported = { $name }: este formato não faz isso — um tar não tem senha e só muda convertendo
files-archive-why-needs-password = { $name } é cifrado e precisa da senha
files-archive-why-wrong-password = Essa senha não abre { $name }
files-archive-why-corrupt = { $name } está danificado, ou não é um arquivo compactado
files-archive-why-exists = { $name } já existe; nada foi substituído
files-archive-why-unstartable = o programa para { $name } não pôde ser iniciado
files-archive-why-failed = o trabalho em { $name } não terminou
files-archive-why-stopped = o trabalho em { $name } foi parado, e o que ele tinha escrito foi apagado
# The sheet `compress` and `convert` open.
files-compress-title = Comprimir { $what }
files-convert-title = Converter { $name }
files-sheet-format = formato
files-sheet-level = nível
files-sheet-password = senha
files-sheet-password-none = nenhuma
files-sheet-password-set = definida, AES-256
files-sheet-hide = esconder também os nomes
files-sheet-split = dividir em volumes
files-sheet-split-none = não
files-sheet-yes = sim
files-sheet-no = não
files-sheet-go = comprimir em { $name }
files-sheet-convert-go = converter em { $name }
# Walking into an archive, and the passwords it asks.
files-archive-reading = lendo { $name }…
files-archive-opening = tirando { $name } para abrir…
files-archive-add-top = os membros entram no topo do arquivo — suba até ele para adicionar
files-archive-passwords-differ = as duas senhas são diferentes; nada foi definido
files-archive-nothing-chosen = nada escolhido para tirar
# The panels inside an archive: what its listing says of a member, and why
# nothing is previewed.
files-archive-fact-packed = compactado
files-archive-fact-encrypted = cifrado
files-archive-preview = um membro é mostrado quando é aberto, depois de sair do arquivo compactado
ask-files-archive-password = A senha do arquivo compactado
ask-files-archive-password-wrong = Não era essa — a senha do arquivo compactado
ask-files-archive-password-new = Uma senha para o novo arquivo compactado
ask-files-archive-password-again = A mesma senha de novo
transfers-work-archive-add = pôr no arquivo compactado
transfers-work-archive-remove = tirar do arquivo compactado
transfers-work-archive-test = testar o arquivo compactado
transfers-work-archive-convert = converter o arquivo compactado
# An encrypted disk (LUKS) in the sidebar.
files-encrypted-locked = { $name } é cifrado e está trancado — montar pede a frase-senha
files-encrypted-unlocked = { $name } está destrancado; ejetar tranca de novo antes de cortar a energia
files-encrypted-fact = cifra
files-encrypted-fact-locked = trancado — LUKS
files-encrypted-fact-unlocked = destrancado — LUKS
files-unlock-working = destrancando { $name }…
files-unlock-wrong = essa frase-senha não abre { $name }
files-unlock-failed = { $name } não pôde ser destrancado: { $why }
files-unlock-locked = { $name } foi trancado de novo
ask-files-unlock = A frase-senha que destranca este disco
sidebar-lock = trancar
sidebar-does-lock = desmonta o que saiu dele e tranca o contêiner (Encrypted.Lock); a energia continua ligada
# Formatting with encryption (the format buffer, and its host).
# The format buffer's third question, as the modeline names its state.
state-encrypt = cifra
format-encrypt-title = Cifrar?
format-encrypt-no = não — quem tiver o disco na mão consegue ler
format-encrypt-yes = sim — só abre com uma frase-senha
format-tech-encrypt-no = o sistema de arquivos é escrito às claras
format-tech-encrypt-yes = LUKS2 · a frase-senha é pedida duas vezes, na linha desta janela
format-encrypt-differ = as duas frases-senha são diferentes; nada foi escrito
format-encrypt-let-go = nenhuma frase-senha foi dada; nada foi escrito
format-encrypted = cifrado
ask-format-encrypt-passphrase = Uma frase-senha para o disco
ask-format-encrypt-again = A mesma frase-senha de novo

# THE ICONS APP (`LEADER m i`) — `crate::icons`, `crate::icons_page`,
# `crate::icons_window`. One app for the three collections the three buffers
# used to be: Nerd Fonts, Lucide and Unicode.
cmd-icons-open = Ícones
cmd-icons-window = Ícones, numa janela
cmd-icons-nerd-fonts = Navegar pelos glifos das Nerd Fonts
cmd-icons-lucide = Navegar pelos ícones Lucide
cmd-icons-unicode = Navegar pelos caracteres Unicode
cmd-icons-colour = A cor do ícone Lucide
ask-icons-colour = A cor do traço (uma cor CSS, ou currentColor)
# What the modeline names the app's window (`br.ronoctua.icons`).
mode-icons = ícones
icons-title = Ícones
# The three screens, as their tabs read.
icons-screen-nerd-fonts = Nerd Fonts
icons-screen-lucide = Lucide
icons-screen-unicode = Unicode
about-icons-tab-nerd-fonts = Mostra os glifos das Nerd Fonts
about-icons-tab-lucide = Mostra os ícones Lucide
about-icons-tab-unicode = Mostra os caracteres Unicode e os emoji
# What the tab line says where nothing is typed yet, per screen.
icons-search-nerd-fonts = digite para buscar um glifo
icons-search-lucide = digite para buscar um ícone
icons-search-unicode = digite para buscar um caractere
icons-count = { $shown } de { $total }
icons-loading = Lendo as tabelas de ícones…
icons-none = Nada aqui corresponde a “{ $query }”.
# The grid's keys, and the page's.
icons-page = abrir
about-icons-page = Abre a página do ícone sob o cursor: o que ele é, o tamanho e todas as formas de copiá-lo
icons-copy = copiar
about-icons-copy = Copia o ícone sob o cursor, como ele é desenhado
icons-less = menos
about-icons-less = Diminui o ajuste sob o cursor, ou mostra o ícone anterior
icons-more = mais
about-icons-more = Aumenta o ajuste sob o cursor, ou mostra o próximo ícone
icons-enter = copiar
about-icons-enter = Copia no formato sob o cursor, ou muda o ajuste sob ele
icons-reset = restaurar
about-icons-reset = Devolve todos os ajustes ao que eram
icons-filters = filtros
about-icons-filters = Abre ou fecha o painel de filtros: os conjuntos, as categorias e as famílias, cada escolha escrita na busca
icons-choose = escolher
about-icons-choose = Filtra pela escolha sob o cursor, mantendo as palavras digitadas
icons-change = mudar
about-icons-change = Aumenta o ajuste sob o cursor — do máximo, volta ao mínimo
# O título da segunda seção do painel: filtros que só os emoji têm.
icons-panel-emoji = Só emoji
icons-colour-closed = A cor do Lucide é do app de ícones: abra-o com { $keys } e depois a página de um ícone Lucide.
# The icon's page: its sections, its facts and its settings.
icons-heading-facts = O que é
icons-heading-adjust = Ajustes
icons-heading-copy = Copiar como
icons-fact-name = Nome
icons-fact-codepoint = Codepoint
icons-fact-set = Conjunto
icons-fact-token = Token de ícone
icons-fact-tray = Atribuído a um app
# One row of tags per language, the key completed with its subtag —
# `buffer-tag-rows-have-text` asks for one per declared language.
icons-sheet-tags-en = Tags (en)
icons-sheet-tags-pt = Tags (pt)
icons-fact-component = Componente
icons-fact-categories = Categorias
icons-fact-aliases = Também chamado
icons-fact-family = Família
icons-fact-category = Categoria
icons-fact-block = Bloco
icons-fact-decomposition = Decomposição
icons-fact-width = Largura
icons-fact-group = Grupo
icons-fact-subgroup = Subgrupo
icons-fact-tone = Tom de pele
icons-width-zero = Marca combinante — o apoio pontilhado não é dela
icons-width-one = Uma célula
icons-width-double = Ocupa duas células
icons-yes = sim
icons-no = não
icons-adjust-size = Tamanho
icons-adjust-stroke = Espessura
icons-adjust-absolute = Traço absoluto
icons-adjust-colour = Cor
icons-adjust-cap = Pontas
icons-size-px = { $px } px
icons-cap-round = redondas
icons-cap-butt = retas
icons-cap-square = quadradas
# The copy formats, per collection (`ronoctua_iconset::copy`).
icons-copy-nerd-fonts-name = Nome
icons-copy-nerd-fonts-glyph = Glifo
icons-copy-nerd-fonts-css = Classe CSS
icons-copy-nerd-fonts-hex = Hexadecimal
icons-copy-nerd-fonts-utf16 = UTF-16
icons-copy-nerd-fonts-escape = Escape Unicode
icons-copy-nerd-fonts-html = Entidade HTML
icons-copy-nerd-fonts-rust = Literal Rust
icons-copy-nerd-fonts-token = Token do design system
icons-copy-unicode-character = Caractere
icons-copy-unicode-name = Nome oficial
icons-copy-unicode-hex = Hexadecimal
icons-copy-unicode-decimal = Decimal
icons-copy-unicode-escape = Escape Unicode
icons-copy-unicode-html = Entidade HTML
icons-copy-unicode-rust = Literal Rust
icons-copy-unicode-utf16 = UTF-16
icons-copy-unicode-utf8 = Bytes UTF-8
icons-copy-lucide-svg = SVG
icons-copy-lucide-jsx = JSX
icons-copy-lucide-vanilla = HTML (lucide.js)
icons-copy-lucide-react = React
icons-copy-lucide-vue = Vue
icons-copy-lucide-svelte = Svelte
icons-copy-lucide-preact = Preact
icons-copy-lucide-solid = Solid
icons-copy-lucide-angular = Angular
icons-copy-lucide-font = Fonte de ícones
icons-copy-lucide-name = Nome
icons-copy-lucide-component = Componente
icons-copy-lucide-character = Caractere
icons-copy-lucide-hex = Codepoint
icons-copy-lucide-path = Traçado
icons-copy-lucide-uri = Data URI
icons-copy-lucide-base64 = Base64
icons-copy-lucide-css = Regra CSS
icons-copy-lucide-sprite = Sprite

# THE CALCULATOR (`LEADER =`, `crate::calc`): the leader's group and its two
# doors, the window's name and the modeline's name for it.
group-calc = calculadora
cmd-calc-open = A calculadora, no rodapé
cmd-calc-window = A calculadora, numa janela própria
calc-title = Calculadora
# The modeline's name for its window (`br.ronoctua.calc`).
mode-calc = calculadora
calc-mode-basic = Básica
calc-mode-scientific = Científica
calc-mode-convert = Conversão
calc-sum = Conta
calc-amount = Quantia
calc-empty = digite uma conta, ou use as teclas
calc-equals-exact = = { $value }
calc-equals-approx = ≈ { $value }
calc-history-title = Histórico
calc-history-empty = Nenhum resultado guardado ainda: { $keys } guarda um.
calc-from = De
calc-to = Para
calc-find = Procurar unidade — { $side }:
calc-find-none = Nenhuma unidade desta categoria tem esse nome.
calc-category-currency = Moeda
calc-rates-for = Converter moedas precisa das cotações do dia do Banco Central Europeu.
calc-rates-asking = Perguntando a { $host } as cotações do dia…
calc-rates-of = Cotações de { $date }, do Banco Central Europeu.
calc-unkept-unparsed = { $file } não pode ser lido, então este resultado não foi guardado; o arquivo ficou como estava.
calc-unkept = Este resultado não pôde ser guardado em { $file }: { $why }
calc-float-unreached = O compositor não pôde ser chamado para mover esta janela.
# The header's words, and what each one does under the pointer.
calc-equals = guardar
about-calc-equals = Guarda o resultado no histórico e continua a partir dele; num resultado escolhido no histórico, traz a conta dele de volta.
calc-mode = modo
about-calc-mode = Vai para o próximo modo, em volta dos três: básica, científica, conversão.
calc-clear = limpar
about-calc-clear = Esvazia a conta.
calc-copy = copiar
about-calc-copy = Copia o resultado — o escolhido no histórico, ou o que está na linha.
calc-history = histórico
about-calc-history = Escolhe um resultado do histórico, um mais antigo a cada vez.
calc-take = escolher
about-calc-take = Escolhe a unidade sob o cursor para o lado que está sendo escolhido.
calc-ring-next = próxima categoria
about-calc-ring-next = Vai para a próxima categoria de unidades.
calc-ring-previous = categoria anterior
about-calc-ring-previous = Vai para a categoria de unidades anterior.
about-calc-from = Escolhe a unidade da quantia: o que for digitado em seguida a procura.
about-calc-to = Escolhe a unidade para a qual a quantia é convertida: o que for digitado em seguida a procura.
calc-swap = trocar
about-calc-swap = Troca as duas unidades: o que era convertido de passa a ser convertido para.
calc-float = compacta ou normal
about-calc-float = Deixa a janela compacta, flutuando sobre as outras — ou a devolve ao layout entre elas.
calc-keys-narrow = As teclas das funções precisam de mais espaço que este. Digite-as pelo nome: sen, cos, √, ln…
# The keypad's legends: what is not a digit or the decimal mark.
calc-key-add = +
calc-key-subtract = −
calc-key-multiply = ×
calc-key-divide = ÷
calc-key-open = (
calc-key-close = )
calc-key-percent = %
calc-key-power = xʸ
calc-key-square = x²
calc-key-reciprocal = x⁻¹
calc-key-factorial = n!
calc-key-degree = °
calc-key-sin = sen
calc-key-cos = cos
calc-key-tan = tg
calc-key-asin = arcsen
calc-key-acos = arccos
calc-key-atan = arctg
calc-key-ln = ln
calc-key-log = log
calc-key-sqrt = √
calc-key-cbrt = ∛
calc-key-exp = exp
calc-key-pi = π
calc-key-e = e
calc-key-clear = C
calc-key-equals = =

# Why a file's tags cannot be written, said precisely (`ronoctua_tags::Why`).
player-tag-format-id3v2 = Uma etiqueta ID3v2
player-tag-format-id3v1 = Uma etiqueta ID3v1
player-tag-format-ape = Uma etiqueta APEv2
player-tag-format-vorbis = Um comentário Vorbis
player-tag-format-mp4 = Uma etiqueta MP4
player-tag-format-riff-info = Uma lista RIFF INFO
player-tag-format-aiff-text = Um bloco de texto AIFF
player-trouble-tag-no-field-in = { $format } não tem lugar para { $field }.
player-trouble-tag-no-pictures-in = { $format } não guarda imagens: a capa deste arquivo não pode ser posta nem tirada.
player-trouble-tag-streams-only-matroska = O idioma, o nome e as marcas de uma faixa só são gravados em Matroska, e este arquivo é { $container }.
player-trouble-tag-attachment = Um anexo — uma imagem, uma fonte — fica guardado no arquivo e não toca: não tem idioma nem nome de faixa para mudar.
player-trouble-tag-read-only = { $format }: aqui só se lê, e nunca se muda à parte.
player-trouble-tag-only-the-front-cover = Aqui só a capa da frente é trocada ou tirada; as outras imagens do arquivo ficam como estão.
# Why the lyrics were not translated (`ronoctua_words::Why`).
player-trouble-translation = A letra não foi traduzida: { $why }
player-trouble-translation-empty = não há palavras para traduzir.
player-trouble-translation-too-long = a música é maior do que o tradutor aceita de uma vez.
player-trouble-translation-unknown-tongue = o tradutor não conhece esse idioma.
player-trouble-translation-wrong-pass = o consentimento dado é para outro endereço que não o do tradutor.
player-trouble-translation-no-key = o tradutor precisa de uma chave, e nenhuma foi guardada.
player-trouble-translation-key-refused = o tradutor recusou a chave.
player-trouble-translation-quota = a cota de hoje do tradutor acabou.
player-trouble-translation-paced = pedido vezes demais; tente de novo daqui a pouco.
player-trouble-translation-not-found = o tradutor não achou nada para isso.
player-trouble-translation-no-dictionary = não há dicionário para este idioma.
player-trouble-translation-unreachable = não foi possível alcançar o tradutor.
player-trouble-translation-failed = o tradutor respondeu com uma falha.
player-trouble-translation-garbled = o tradutor respondeu algo que não é uma tradução.
# Why a press was not kept in the consent file (`ronoctua_consent::Unkept`).
player-consent-unkept-no-home = Não há pasta pessoal onde guardar o consentimento.
player-consent-unkept-unparseable = O arquivo de consentimento (consent.json) não pôde ser lido; ficou como estava, e nada foi permitido.
player-consent-unkept-unwritable = O arquivo de consentimento não pôde ser gravado; nada foi permitido.

player-lyrics-unsynced = Traduzida inteira, não verso a verso:
# AS PORTAS DO PLAYER (2026-09-30). O arquivo da fila no título dela.
player-queue-unsaved = { $name } · não salva
player-queue-new-unsaved = nova, não salva
player-save-queue-again = salvar de novo
cmd-player-save-queue-again = Salvar a fila de novo no arquivo de onde ela veio
player-saved-again = A fila foi salva de novo em { $name }.
ask-player-already-queued = { $name } já está na fila como { $number }. Pôr de novo?
ask-player-already-queued-somewhere = { $name } já está na fila. Pôr de novo?
ask-player-queue-changed-on-disk = { $path } mudou no disco desde que a fila foi aberta ou salva. Gravar por cima?
# Os saltos ao lado do relógio, e um clique duplo num lado da imagem.
player-rewind = voltar
player-forward = avançar
cmd-player-rewind = Voltar dez segundos
cmd-player-forward = Avançar dez segundos
player-seeked-back = −{ $seconds } s
player-seeked-ahead = +{ $seconds } s
# A imagem recolhida, no título "Agora".
player-picture-fold = recolher a imagem
cmd-player-picture-fold = Recolher a imagem e dar as linhas dela às linhas de baixo, ou mostrá-la de novo
player-no-picture-to-fold = Não há imagem aqui para recolher.
# Um vídeo de um site: página própria.
player-page-website = Vídeo de um site
player-about-website = Um vídeo ou uma música do YouTube, ou de um dos muitos outros sites que o yt-dlp conhece, toca pelo endereço da página, digitado ou colado na linha.
player-website-play = Tocar agora…
player-website-enqueue = Pôr na fila…
ask-player-website-play = Tocar agora (o endereço de uma página)
ask-player-website-enqueue = Pôr na fila (o endereço de uma página)
cmd-player-website = Um vídeo de um site: tocar agora ou pôr na fila, pelo endereço
player-website-recent = Endereços tocados há pouco
player-website-none = Nenhum endereço tocado daqui ainda.
player-website-not-an-address = “{ $typed }” não é o endereço de uma página.
player-website-forgotten = Esquecido: { $address }
player-file-no-home = não há pasta pessoal onde guardá-lo
player-file-unparseable = ele existe e não pôde ser lido, então ficou como estava
player-file-unwritable = não foi possível gravá-lo
# Um campo de várias linhas mudado aqui, nesta porta.
player-field-edit-here = Editar aqui
# Os países de uma página do mundo: os com mais primeiro, ou de A a Z.
player-countries-order = ordem
cmd-player-countries-order = Os países de A a Z pelo nome, ou os com mais primeiro
player-countries-by-count = Os países com mais primeiro.
player-countries-by-name = Os países de A a Z, pelo nome.
# A letra traduzida sob os versos.
player-lyrics-translate = traduzir
cmd-player-lyrics-translate = Traduzir a letra sob os versos, ou parar
player-lyrics-translate-to = Traduzir para
player-lyrics-translating = Traduzindo…
player-lyrics-translated-by = Traduzida por
player-lyrics-none-to-translate = Não há letra para traduzir.
player-lyrics-let = Deixar { $host } traduzi-la
player-consent-translation = Traduzir a letra

# CANAIS POR TEMA, no catálogo local (`player_catalog`): uma fonte por vez,
# seus temas e categorias, e a listagem que uma linha deles abre.
player-about-catalog = Os canais, filmes e vídeos de cada fonte, lidos para esta máquina e filtrados na hora. Uma fonte é lida de novo quando aparece depois do prazo dela.
player-page-catalog = Listagem
player-page-sources = Fontes
player-catalog-source = Canais de
player-catalog-ingesting = Lendo esta fonte…
player-catalog-stale = Lida { $date }; é hora de ler de novo.
player-catalog-never = Ainda não lida.
player-catalog-read-again = Ler esta fonte de novo
player-catalog-reading-now = Lendo { $name }…
player-catalog-search = Buscar pelo título
player-catalog-searched = “{ $words }”
player-catalog-searching = Buscando “{ $words }” no arquivo…
player-catalog-search-archive = Buscar “{ $words }” no arquivo inteiro
player-catalog-themes = Temas
player-catalog-categories = Categorias
player-catalog-everything = Tudo
player-catalog-ungrouped = Sem categoria
player-catalog-empty = Ainda não há nada aqui.
player-catalog-nothing-found = Nada corresponde.
player-catalog-adult = Só para adultos
player-catalog-letter = Inicial
player-catalog-letter-all = Todas
player-catalog-page = Página
player-catalog-page-of = { $page } de { $pages }
player-catalog-page-not-a-number = “{ $said }” não é um número de página.
player-catalog-per-page = Por página
player-catalog-sort = Ordem
player-catalog-sort-title = Pelo título
player-catalog-sort-popular = Mais vistos primeiro
player-catalog-sort-newest = Mais novos primeiro
player-catalog-sort-oldest = Mais antigos primeiro
player-catalog-country = País
player-catalog-country-all = Todos os países
player-catalog-language = Língua
player-catalog-language-all = Todas as línguas
player-catalog-quality = Altura da imagem
player-catalog-quality-any = Qualquer altura
player-catalog-quality-lines = { $lines } linhas ou mais
player-catalog-layout = Exibição
player-catalog-layout-list = Lista
player-catalog-layout-lines = Duas linhas
player-catalog-layout-columns = Colunas
player-catalog-layout-posters = Capas
player-catalog-layout-big = Capas grandes
player-catalog-scope-in = Buscar em
player-catalog-scope-category = Esta lista
player-catalog-scope-every-category = Toda a fonte
player-catalog-scope-from = De
player-catalog-scope-source = Esta fonte
player-catalog-scope-every-source = Todas as fontes
player-catalog-shown = { $from }–{ $to } de { $total }
player-catalog-kind-live = ao vivo
player-catalog-kind-audio = áudio
player-catalog-item-gone = Não está mais no catálogo.
ask-player-catalog-letter = Inicial
ask-player-catalog-page = Número da página
ask-player-catalog-search = Títulos com
# O CATÁLOGO ADULTO, atrás da senha do sistema da própria pessoa.
player-adult-locked = Trancado: a sua senha do sistema abre
player-adult-checking = conferindo…
ask-player-adult-password = A sua senha do sistema
player-adult-open = O catálogo adulto fica aberto até passarem 15 minutos sem tecla e sem clique.
player-adult-lock = Trancar agora
player-adult-locked-again = O catálogo adulto está trancado.
player-adult-wrong = Essa não é a sua senha.
player-adult-no-service = Este sistema não tem o serviço ronoctua-unlock: o catálogo adulto fica fechado.
player-adult-shut = Trancado
player-adult-unavailable = Não foi possível conferir a senha.
player-adult-honest = Uma tranca nestas portas, não uma cifra: a lista é um arquivo neste disco como qualquer outro.
# AS FONTES DO CATÁLOGO (`player_sources`).
player-about-sources = Cole o link de uma lista IPTV, de um feed de podcast ou de vídeos, de um canal ou playlist do YouTube, ou de uma coleção do archive.org. O endereço fica privado nesta máquina e aparece só pelo host.
player-source-add = Adicionar uma fonte…
ask-player-source-url = Link de uma lista, um feed, um canal ou uma coleção
player-source-add-adult = Adicionar uma fonte ao catálogo adulto…
ask-player-adult-source-url = Link de uma lista para o catálogo adulto
player-source-adding = Lendo { $name }…
player-source-address = Endereço
player-source-plain = Sem criptografia: este endereço é http, e o que ele responde — a lista, a conta no endereço dela e as capas que ela guarda nesse mesmo site — atravessa a rede às claras.
player-source-kind = Tipo
player-source-kind-iptv-org = Índice de TV do iptv-org
player-source-kind-archive = Coleções do Internet Archive
player-source-kind-youtube-lives = Buscas de lives no YouTube
player-source-kind-nasa = Biblioteca da NASA
player-source-kind-commons = Categorias do Wikimedia Commons
player-source-kind-peertube = Vídeos do PeerTube
player-source-kind-playlist = Lista IPTV (M3U)
player-source-kind-feed = Feed (RSS ou Atom)
player-source-kind-youtube = Canal ou playlist do YouTube
player-source-kind-collection = Coleção do archive.org
player-source-posters = Buscar as capas
player-source-hosts = de { $hosts }
player-source-hosts-any = de onde a lista disser
player-source-posters-on = As capas dela serão buscadas.
player-source-posters-off = As capas dela não serão buscadas.
player-source-adult = No catálogo adulto
player-source-adult-on = Ela vai para o catálogo adulto.
player-source-adult-off = Ela sai do catálogo adulto.
player-source-adult-hidden = Fontes do catálogo adulto
player-source-remove = Remover…
ask-player-source-remove = Remover { $name } e tudo o que ela trouxe?
player-source-removed = { $name } foi removida.
# POR QUE UMA FONTE NÃO DEU NADA (`ronoctua_broadcast::sources::Refused`).
player-trouble-source-not-an-address = Isso não é um endereço da web.
player-trouble-source-not-a-list = Isso é um vídeo ou uma transmissão, não uma lista deles.
player-trouble-source-unknown = Isso não é nenhuma das fontes lidas aqui: uma lista M3U, um feed RSS ou Atom, um canal ou playlist do YouTube, uma coleção do archive.org.
player-trouble-source-not-let = O host dela ainda não foi permitido.
player-trouble-source-plain-http = Ela foi mandada de um endereço criptografado para um que não é, o que nunca é seguido.
player-trouble-source-unreachable = Não foi possível alcançar o host dela.
player-trouble-source-declined = O host dela recusou a pergunta.
player-trouble-source-too-many-asks = O host dela pediu para ser perguntado menos.
player-trouble-source-nothing-found = Não há nada nesse endereço.
player-trouble-source-malformed = A resposta dela não é o que isto lê.
player-trouble-source-too-large = A resposta dela é maior do que o que é lido.
player-trouble-source-nothing-playable = Ela foi lida, e nada nela toca.
player-trouble-source-no-yt-dlp = Esta instalação não tem o yt-dlp para listá-la.
player-trouble-source-listing-failed = O yt-dlp não conseguiu listá-la.
# OS ARQUIVOS DO CATÁLOGO (`ronoctua_broadcast::catalog::Trouble`).
player-trouble-catalog-no-home = Não há pasta de cache para guardar o catálogo.
player-trouble-catalog-absent = Nada foi lido para o catálogo ainda.
player-trouble-catalog-other-schema = O catálogo é de outra versão e está sendo refeito.
player-trouble-catalog-unreadable = Não foi possível ler o catálogo.
player-trouble-catalog-unwritable = Não foi possível gravar o catálogo.
player-trouble-catalog-too-large = Isso passaria do teto do catálogo, então nada mudou.
player-trouble-sources-no-home = Não há pasta de configuração para guardar as suas fontes.
player-trouble-sources-unreadable = Não foi possível ler o arquivo das suas fontes.
player-trouble-sources-unparseable = O arquivo das suas fontes não é um que isto lê; ele fica como está, e nada é acrescentado a ele.
player-trouble-sources-unwritable = Não foi possível gravar o arquivo das suas fontes.
player-trouble-no-such-source = Essa fonte não existe.
# AS TECLAS DE UMA LISTAGEM.
cmd-player-listing-earlier = A página anterior da listagem
cmd-player-listing-later = A página seguinte da listagem
cmd-player-listing-first = A primeira página da listagem
cmd-player-listing-last = A última página da listagem
cmd-player-listing-full = A listagem sozinha na tela inteira
cmd-player-listing-layout = A próxima disposição da listagem
cmd-player-listing-source = A próxima fonte do catálogo
cmd-player-listing-letter = Uma inicial da listagem, digitada
cmd-player-listing-order = A próxima ordem da listagem
# AS TECLAS DE UMA LISTAGEM, COMO A LINHA DE DICAS AS DIZ.
player-listing-earlier = página anterior
player-listing-later = página seguinte
player-listing-first = primeira página
player-listing-last = última página
player-listing-full = tela inteira
player-listing-layout = exibição
player-listing-source = próxima fonte
player-listing-letter = inicial
player-listing-order = ordem

# THE VIDEO CATALOGUE (LEADER v, video_metrics.rs and its window).
group-video-metrics = vídeos
mode-video-metrics = métricas de vídeo
video-metrics-title = Métricas de vídeo
cmd-video-metrics-open = Métricas de vídeo: seus canais do YouTube, os vídeos pontuados
cmd-video-metrics-window = Métricas de vídeo, numa janela
cmd-video-metrics-scan = Varrer agora todos os canais atrás de vídeos novos
cmd-video-metrics-up = Linha anterior
cmd-video-metrics-down = Próxima linha
cmd-video-metrics-less = O valor sob o cursor um passo abaixo, ou o vídeo anterior na grade
cmd-video-metrics-more = O valor sob o cursor um passo acima, ou o vídeo seguinte na grade
cmd-video-metrics-enter = Abrir ou mudar o que está sob o cursor
cmd-video-metrics-page-back = A página de vídeos anterior
cmd-video-metrics-page-on = A página de vídeos seguinte
cmd-video-metrics-layout = Os vídeos em lista, ou numa grade de miniaturas
cmd-video-metrics-search = Buscar nas palavras dos vídeos — títulos, descrições, etiquetas e as suas
cmd-video-metrics-player = Abrir no player
cmd-video-metrics-kiosk = Abrir no Chrome, como quiosque
cmd-video-metrics-download = Baixar, pela fila de transferências
cmd-video-metrics-hide = Ocultar das listas, ou mostrar de novo
cmd-video-metrics-delete = Apagar de vez: ele nunca volta
cmd-video-metrics-add-channel = Acrescentar um canal do YouTube, pelo endereço ou pelo @handle
cmd-video-metrics-channel-score = Digitar a pontuação de um canal
cmd-video-metrics-points = Digitar os pontos de uma regra
cmd-video-metrics-days = Digitar em quantos dias a novidade se apaga
cmd-video-metrics-band = Digitar uma faixa de duração
cmd-video-metrics-words = Digitar as palavras que uma regra procura
cmd-video-metrics-second-tags = Digitar as suas etiquetas de um vídeo
cmd-video-metrics-weight = Digitar o peso de uma nota
video-metrics-up = acima
video-metrics-down = abaixo
video-metrics-less = menos
video-metrics-more = mais
video-metrics-enter = abrir
video-metrics-page-back = página anterior
video-metrics-page-on = próxima página
video-metrics-layout = lista ou grade
video-metrics-search = buscar
video-metrics-player = player
video-metrics-kiosk = Chrome
video-metrics-download = baixar
video-metrics-hide = ocultar
video-metrics-delete = apagar
ask-video-metrics-add-channel = Endereço ou @handle do canal
ask-video-metrics-channel-score = Pontuação, de 0 a 10 000
ask-video-metrics-points = Pontos
ask-video-metrics-days = Apaga-se em quantos dias
ask-video-metrics-band = De quanto a quanto (4:00-20:00)
ask-video-metrics-words = Palavras a procurar
ask-video-metrics-search = Buscar nos vídeos
ask-video-metrics-second-tags = Suas etiquetas, separadas por vírgulas
ask-video-metrics-weight = Pontos para esta nota
video-metrics-heading-scan = Varredura
video-metrics-last-scan = Última varredura
video-metrics-never = nunca
video-metrics-scan-running = varrendo desde { $when }
video-metrics-scan-cut = parou antes de terminar ({ $when })
video-metrics-scan-added = Vídeos novos
video-metrics-scan-failed = Canais que falharam
video-metrics-failed-of = { $failed } de { $channels }
video-metrics-pending = { $pending ->
        [1] Um vídeo espera os detalhes: a próxima varredura pergunta por ele.
       *[other] { $pending } vídeos esperam os detalhes: as próximas varreduras perguntam por eles, poucos de cada vez.
    }
video-metrics-scan-now = Varrer agora
video-metrics-ask-youtube = Perguntar ao YouTube
about-video-metrics-ask-youtube = Se uma varredura pode perguntar ao YouTube: desligado, nada é perguntado até você ligar de novo.
video-metrics-ask-youtube-note = Acrescentar um canal deixa { $hosts } serem perguntados — feeds e miniaturas, nada mais. A permissão é desta máquina, endereço por endereço: desligada aqui, nada nela pergunta a eles, nem as fontes do YouTube do player.
video-metrics-on = ligado
video-metrics-off = desligado
video-metrics-heading-catalogue = O catálogo
video-metrics-videos = Vídeos
video-metrics-channels = Canais
video-metrics-rules = Regras
video-metrics-weights = Pesos das notas
video-metrics-heading-left-out = Deixados de fora
video-metrics-skip-before-reference = Mais antigos que a última varredura do canal
video-metrics-skip-first-scan = Além da primeira varredura de um canal
video-metrics-skip-blocked-short = Shorts, enquanto estão bloqueados
video-metrics-skip-unavailable = Privados, removidos ou sumidos
video-metrics-skip-deleted = Apagados por você
video-metrics-reading = Lendo o catálogo…
video-metrics-why-not-yet = Isto ainda não tem resposta.
video-metrics-why-no-database = Esta compilação não tem catálogo de vídeos.
video-metrics-why-not-there = Nada foi catalogado ainda: acrescente um canal, e a próxima varredura preenche.
video-metrics-why-unreadable = O catálogo de vídeos está lá e não pode ser lido.
video-metrics-why-other-schema = O catálogo de vídeos foi escrito por uma versão mais nova; ele fica como está.
video-metrics-why-busy = Uma varredura está segurando o catálogo agora; tente de novo daqui a pouco.
video-metrics-why-not-found = Isto não está mais no catálogo.
video-metrics-why-failed = O catálogo não pôde ser escrito.
video-metrics-why-invalid = O catálogo não pode guardar isso: uma regra precisa de palavras ou de uma faixa que termine depois de começar, e a novidade precisa de pelo menos um dia e de pontos não negativos.
video-metrics-stop-bot-check = O YouTube perguntou se isto é um robô, e a varredura parou; a próxima tenta de novo.
video-metrics-stop-too-many-requests = O YouTube disse que foram pedidos demais, e a varredura parou; a próxima tenta de novo.
video-metrics-stop-offline = Não havia rede, e a varredura parou.
video-metrics-stop-not-let = Perguntar ao YouTube está desligado, e a varredura não perguntou nada.
video-metrics-add-a-channel = Acrescentar um canal…
video-metrics-no-channels = Nenhum canal ainda: acrescente um pelo endereço ou pelo @handle.
video-metrics-channel-row = { $score } · { $videos ->
        [1] um vídeo
       *[other] { $videos } vídeos
    } · varrido { $when }
video-metrics-channel-added = { $name } foi acrescentado: os vídeos mais novos chegam na próxima varredura.
video-metrics-channel-removed = O canal e os vídeos dele foram removidos.
video-metrics-add-absent = Esta instalação não tem yt-dlp, que diz de quem é o canal de um endereço.
video-metrics-add-unstartable = O yt-dlp não pôde ser iniciado.
video-metrics-add-slow = O yt-dlp demorou demais para dizer de quem é o canal.
video-metrics-add-failed = O yt-dlp não conseguiu dizer de quem é o canal.
video-metrics-add-garbled = O yt-dlp respondeu algo que não nomeia canal nenhum.
video-metrics-add-not-an-address = Isso não é um endereço do YouTube, um @handle nem uma id de canal.
video-metrics-add-gone = O YouTube não tem canal ali.
video-metrics-not-let-no-home = O canal entrou, e não há uma pasta pessoal onde guardar o consentimento para perguntar ao YouTube: uma varredura não vai perguntar.
video-metrics-not-let-unparseable = O canal entrou, e o arquivo de consentimento não é legível, então ficou como está: uma varredura não vai perguntar ao YouTube.
video-metrics-not-let-unwritable = O canal entrou, e o consentimento para perguntar ao YouTube não pôde ser gravado: uma varredura não vai perguntar.
video-metrics-address = Endereço
video-metrics-handle = Handle
video-metrics-added = Acrescentado
video-metrics-reference = Varrido até
video-metrics-reference-none = ainda não
video-metrics-last-error = Último erro
video-metrics-its-videos = Os vídeos dele
video-metrics-score = Pontuação
about-video-metrics-score = A pontuação de um canal, de 0 a 10 000, entra em todo vídeo dele: as setas movem de cem em cem, e o RET digita.
video-metrics-its-shorts = Os Shorts dele
video-metrics-shorts-follow = como em todo lugar
video-metrics-shorts-blocked = bloqueados
video-metrics-shorts-allowed = permitidos
video-metrics-its-rules = As regras dele
video-metrics-heading-remove = Remover
video-metrics-remove-channel = Remover este canal
ask-video-metrics-remove-channel = Remover { $name }? Os vídeos dele vão junto, e nenhum volta.
video-metrics-keep-channel = Não, manter o canal
video-metrics-remove-channel-it = Sim, remover o canal e os vídeos
video-metrics-block-shorts = Bloquear os Shorts do YouTube
video-metrics-block-shorts-note = Shorts bloqueados nunca são catalogados, e os que já estão ficam ocultos; um canal pode escolher outra coisa na página dele.
video-metrics-heading-score = Como um vídeo é pontuado
video-metrics-rule-term = Palavras a procurar
video-metrics-rule-band = Faixa de duração
video-metrics-rule-preferred = Duração preferida
video-metrics-rule-short = Penalidade de Short
video-metrics-rule-recency = Novidade
video-metrics-rule-live = Ao vivo ou estreia
video-metrics-none = não há
video-metrics-points-value = { $points ->
        [1] { $points } ponto
       *[other] { $points } pontos
    }
video-metrics-days-value = { $days ->
        [1] um dia
       *[other] { $days } dias
    }
video-metrics-recency-value = { $points }, somem em { $days }
video-metrics-rule-in = { $value } · { $scope }
video-metrics-scope-own = deste canal
video-metrics-scope-everywhere = em todo lugar
video-metrics-band-value = { $from } a { $to }
video-metrics-band-joiner = a
video-metrics-band-row = Duração de { $band }
video-metrics-preferred-row = Duração preferida: { $band }
video-metrics-term-row = “{ $words }”
video-metrics-add-band = Acrescentar uma faixa de duração…
video-metrics-add-band-here = Acrescentar uma faixa de duração para este canal…
video-metrics-bands-note = Estas faixas são as de todo lugar; uma faixa acrescentada aqui começa a lista própria deste canal, e as de todo lugar deixam de contar para ele.
video-metrics-add-term = Acrescentar palavras…
video-metrics-add-term-here = Acrescentar palavras para este canal…
video-metrics-rule-inherited-note = Esta é a regra de todo lugar; mudar aqui cria uma só para { $name }.
video-metrics-rule-own-note = Esta é a regra própria de { $name }: ela conta no lugar da de todo lugar.
video-metrics-rule-own-off-note = Esta é a regra própria de { $name }, desligada: nada deste tipo conta para { $name }, nem a regra de todo lugar. Remover a regra traz a de todo lugar de volta.
video-metrics-recency-points = Pontos hoje
video-metrics-recency-days = Apaga-se em
video-metrics-rule-points = Pontos
about-video-metrics-points = Os pontos que ela soma — negativos subtraem: as setas movem de cem em cem, e o RET digita.
video-metrics-rule-length = Duração
video-metrics-term-words = Palavras
video-metrics-term-match = Casamento
video-metrics-match-word = palavras inteiras
video-metrics-match-phrase = as palavras em ordem
video-metrics-match-prefix = palavras que começam assim
video-metrics-field-title = No título
video-metrics-field-description = Na descrição
video-metrics-field-tags = Nas etiquetas
video-metrics-field-second-description = Na sua descrição
video-metrics-field-second-tags = Nas suas etiquetas
video-metrics-rule-on = Em vigor
video-metrics-remove-rule = Remover esta regra
ask-video-metrics-remove-rule = Remover esta regra? Ela não pode ser trazida de volta.
video-metrics-keep-rule = Não, manter a regra
video-metrics-remove-rule-it = Sim, remover a regra
video-metrics-weights-note = O que uma nota sua soma à pontuação de um vídeo, em todos os canais.
video-metrics-rating-very-negative = Muito negativa
video-metrics-rating-negative = Negativa
video-metrics-rating-positive = Positiva
video-metrics-rating-very-positive = Muito positiva
video-metrics-channel = Canal
video-metrics-every-channel = todos os canais
video-metrics-filter-shorts = Shorts
video-metrics-shorts-any = com e sem
video-metrics-shorts-only = só Shorts
video-metrics-shorts-without = sem Shorts
video-metrics-rating = Sua nota
video-metrics-rating-any = qualquer
video-metrics-rating-none = sem nota
video-metrics-filter-hidden = Ocultos
video-metrics-hidden-without = não aparecem
video-metrics-hidden-only = só os ocultos
video-metrics-hidden-any = aparecem também
video-metrics-filter-text = Busca
video-metrics-search-none = nada
video-metrics-published = Publicado
video-metrics-since-any = a qualquer hora
video-metrics-since-day = no último dia
video-metrics-since-week = na última semana
video-metrics-since-month = nos últimos 30 dias
video-metrics-since-year = no último ano
video-metrics-filter-layout = Disposição
video-metrics-layout-list = lista
video-metrics-layout-grid = miniaturas
video-metrics-page = Página
video-metrics-page-of = { $page } de { $pages } · { $videos ->
        [1] um vídeo
       *[other] { $videos } vídeos
    }
video-metrics-by-score = Por pontuação
video-metrics-no-videos = Nenhum vídeo corresponde.
video-metrics-video-row = { $score } · { $channel } · { $age }
video-metrics-age = { $days ->
        [0] hoje
        [1] ontem
       *[other] há { $days } dias
    }
video-metrics-the-video = O vídeo
video-metrics-duration = Duração do vídeo
video-metrics-kind = Tipo
video-metrics-kind-short = um Short
video-metrics-kind-long = um vídeo
video-metrics-kind-live = ao vivo ou uma estreia
video-metrics-its-points = Os pontos dele
video-metrics-size = Imagem
video-metrics-size-value = { $width } × { $height }
video-metrics-languages = Línguas
video-metrics-age-limit = Idade mínima
video-metrics-captions = Legendas
video-metrics-categories = Categorias
video-metrics-chapters = Capítulos
video-metrics-views = Visualizações
video-metrics-first-seen = Visto pela primeira vez
video-metrics-tags = Etiquetas do YouTube
video-metrics-description = Descrição do YouTube
about-video-metrics-copy = O RET põe este valor na área de transferência.
video-metrics-yours = O que é seu
video-metrics-your-tags = Suas etiquetas
video-metrics-your-description = Sua descrição
about-video-metrics-second-description = Uma descrição sua, editada aqui: C-s guarda, e uma regra pode procurar as palavras dela.
video-metrics-description-kept = Sua descrição foi guardada.
video-metrics-with-it = Com ele
video-metrics-hide-row = Nas listas
video-metrics-shown = aparece
video-metrics-hidden = oculto
ask-video-metrics-hide = Ocultar { $title }? Ele continua no catálogo, e "Ocultos: aparecem também" encontra.
video-metrics-leave-shown = Não, deixar aparecendo
video-metrics-hide-it = Sim, ocultar
ask-video-metrics-delete = Apagar { $title } de vez? Ele nunca volta, nem na próxima varredura.
video-metrics-keep-video = Não, manter o vídeo
video-metrics-delete-it = Sim, apagar de vez
video-metrics-deleted = O vídeo foi apagado, e nunca volta.
ask-video-metrics-again = Ele já está na fila como { $name }. Acrescentar de novo?
video-metrics-not-again = Não, uma vez basta
video-metrics-again = Sim, acrescentar de novo
video-metrics-player-unreachable = O player não respondeu; nada foi acrescentado.
video-metrics-downloading = Baixando { $title }: a fila mostra ({ $keys }).
video-metrics-scan-asked = Uma varredura foi pedida; a modeline marca enquanto ela roda.
video-metrics-scan-started = O cérebro não a assumiu, então a varredura começou direto.
video-metrics-scan-nowhere = Nada aqui pode começar uma varredura: nem o cérebro nem o systemd responderam.
video-metrics-scan-saver = A economia de energia está ligada, e nenhuma varredura começa enquanto ela estiver: nada foi pedido.
video-metrics-scan-other-instance = Esta é uma segunda cópia do sistema: o cérebro dela só ensaia, e a unidade da varredura é a da sessão instalada, então nada foi pedido aqui.
video-metrics-typed-not-a-number = Isso não é um número inteiro.
video-metrics-typed-not-days = Isso não é um número inteiro de dias, um ou mais.
video-metrics-typed-not-a-band = Escreva uma faixa como duas durações, como 4-20 ou 0:30-3:00.
video-metrics-typed-no-words = Não há palavras nisso para procurar.
video-metrics-typed-no-row = Não há linha aqui para isso: ponha o cursor no que isso muda e digite lá.
brain-why-saver = A economia de energia estava ligada

mode-security = central de segurança
# The file manager's menu row that checks a file with VirusTotal.
files-virustotal = checar no VirusTotal
files-does-virustotal = manda a impressão digital (SHA-256) ao VirusTotal, na janela da central de segurança; o arquivo em si só se você disser sim
# The security centre's refusals (`ronoctua_security::Why`), one per reason.
security-refusal-no-helper = Esta sessão não recebeu o ajudante de segurança, então nada aqui pode agir como administrador.
security-refusal-wrong-password = Essa não é a sua senha.
security-refusal-needs-password = Isto precisa da sua senha.
security-refusal-not-allowed = A sua conta não pode agir como administrador.
security-refusal-unstartable = Um programa de que isto precisa não pôde ser iniciado.
security-refusal-failed = Não funcionou; o log do sistema diz por quê.
security-refusal-invalid = O que foi pedido não é algo que a central de segurança aceite.
security-refusal-no-reader = Não há leitor de digitais.
security-refusal-unreadable = Não foi possível ler.
security-refusal-busy = Outro programa está usando o leitor de digitais.
security-refusal-timed-out = Nada tocou o leitor a tempo.
# The view's keys, as the hint line names them (`security::OF_THE_VIEW`).
security-down = desce
security-up = sobe
security-enter = faz
security-left = dedo anterior
security-right = próximo dedo
security-relax = sem senha
security-shields = escudos
security-rule-add = nova regra
security-rule-delete = remove
security-enrol = cadastra
# THE SECURITY CENTRE (`LEADER s e`, `crate::security`): its leader group,
# its commands, its view's keys and questions, and its page.
group-security = segurança
cmd-security-open = Abrir a central de segurança
cmd-security-window = Abrir a central de segurança numa janela
cmd-security-relax = Deixar o administrador agir sem senha por um tempo, ou encerrar isso agora
cmd-security-shields = Recusar toda conexão de entrada, ou parar de recusar
cmd-security-firewall = Ligar ou desligar seu firewall
cmd-security-check-password = Ver se uma senha está em vazamentos conhecidos
cmd-security-check-email = Ver se um endereço de e-mail está em vazamentos conhecidos
cmd-security-check-file = Checar um arquivo no VirusTotal
cmd-security-down = Próxima linha da central de segurança
cmd-security-up = Linha anterior da central de segurança
cmd-security-enter = Fazer o que a linha sob o cursor oferece
cmd-security-left = O dedo anterior a cadastrar
cmd-security-right = O próximo dedo a cadastrar
cmd-security-rule-add = Acrescentar uma regra de firewall
cmd-security-rule-delete = Tirar a regra ou o dedo sob o cursor
cmd-security-enrol = Cadastrar o dedo escolhido
cmd-security-password = Sua senha, para o administrador
cmd-security-rule = Uma regra de firewall, em palavras
cmd-security-vt-key = Guardar sua chave do VirusTotal
cmd-security-vt-forget = Esquecer sua chave do VirusTotal
cmd-security-home = Tratar como de casa a rede que acabou de se conectar
ask-security-password = Sua senha
ask-security-rule = Regra (permitir tcp 8080 de 192.168.1.0/24 em wlan0 casa)
ask-security-vt-key = Sua chave de API do VirusTotal
# As mudanças do gerenciador de arquivos que só o administrador faz (dono,
# grupo, permissões), pela mesma porta do sudo.
ask-files-root-password = Sua senha, para o que só o administrador pode mudar
ask-security-check-password = Senha a checar (só cinco caracteres da impressão dela saem desta máquina)
ask-security-check-email = Endereço de e-mail a checar (ele vai para o XposedOrNot)
ask-security-check-file = Arquivo a checar (a impressão dele vai para o VirusTotal)
hint-security-until = O administrador não pede senha até { $time }; um clique abre onde isso se encerra
transfers-engine-scan = VirusTotal
transfers-work-scan = enviar um arquivo ao VirusTotal
echo-security-checking = Checando…
echo-security-file-checked = { $file }: { $verdict }
echo-security-finger-enrolled = Dedo cadastrado.
echo-security-finger-removed = Dedo removido.
echo-security-finger-sudo-changed = Os dedos para o administrador mudaram.
echo-security-firewall-applied = O firewall está como você pediu.
echo-security-key-forgotten = A chave do VirusTotal foi esquecida.
echo-security-key-kept = A chave do VirusTotal está guardada.
echo-security-key-not-a-key = Isso não é uma chave que o VirusTotal aceite.
echo-security-key-not-kept = A chave não pôde ser guardada na sua pasta.
echo-security-nothing-to-delete = Não há regra nem dedo sob o cursor.
echo-security-no-new-network = Nenhuma rede que se conectou espera ser classificada.
# A ação ao lado do "uma rede nova é tratada como pública" do vigia.
notif-action-treat-as-home = Tratar como de casa
echo-security-relaxed = O administrador não pede senha até a hora que está na modeline.
echo-security-relax-ended = O administrador volta a pedir a senha.
echo-security-rule-not-understood = Isso não é uma regra: permitir ou negar, depois tcp ou udp, as portas e, se quiser, de, em e casa, pública ou tudo.
echo-security-shields-up = Toda conexão de entrada é recusada agora.
echo-security-upload-queued = { $file } está a caminho do VirusTotal; a lista de transferências mostra quanto falta.
security-title = Segurança
security-on = ligado
security-off = desligado
security-unknown = desconhecido
security-answer-no = Não
security-answer-yes = Sim
security-busy = Pedindo ao administrador…
security-busy-enrol = Toque o leitor quando ele pedir.
security-heading-now = Agora
security-heading-firewall = Firewall
security-heading-admin = Administrador
security-heading-finger = Digital
security-heading-device = Esta máquina
security-heading-privacy = Privacidade de rede
security-heading-online = Checagens online
security-heading-suggest = Sugestões
security-now-nothing = Nada fora do esperado.
security-now-reading = Lendo…
security-now-firewall-failed = O firewall falhou
security-now-settings-unreadable = As configurações do firewall não podem ser lidas
security-now-firewall-off = Seu firewall está desligado
security-now-sudo-open = Sem senha de administrador até
security-now-shields = Toda conexão de entrada é recusada
security-now-refused = { $count ->
    [one] Uma conexão recusada há pouco
   *[other] { $count } conexões recusadas há pouco
  }
security-fw-switch = Seu firewall
security-fw-network = Rede em { $iface }
security-fw-profile-home = casa: confiável
security-fw-profile-public = pública: estranhos por perto
security-fw-served = esta máquina a serve
security-fw-treat-as-home = Tratar esta rede como casa
security-fw-shields = Recusar toda conexão de entrada
security-fw-shields-note = Erguer é na hora; baixar pede sua senha, como todo afrouxamento.
security-fw-log = Conexões recusadas no registro
security-fw-log-off = não registradas
security-fw-log-quiet = algumas por minuto
security-fw-log-chatty = muitas por minuto
security-fw-rule-add = Acrescentar uma regra
security-fw-blocked = Recusadas há pouco:
security-fw-blocked-row = { $proto } { $port } de { $source } em { $iface } ({ $count ->
    [one] uma vez
   *[other] { $count } vezes
  })
security-fw-watch-silent = O cérebro não está respondendo, então as conexões recusadas há pouco não podem ser mostradas.
security-fw-ports = Onde esta máquina responde:
security-fw-port-row = { $who }: { $proto } { $port }
security-fw-port-unknown = um programa de outro usuário
security-fw-port-container = container { $name }
security-fw-verdict-open = aberta: o firewall está desligado
security-fw-verdict-allowed = permitida pela sua regra
security-fw-verdict-partly = permitida só em algumas redes, ou de alguns endereços
security-fw-verdict-refused = recusada, a não ser que o sistema a abra
security-fw-verdict-shielded = recusada: todas estão
security-sudo-password = A senha do administrador
security-sudo-asked = pedida toda vez
security-sudo-not-asked-until = não pedida até { $time }
security-sudo-end = Voltar a pedir agora
security-sudo-open-for = Não pedir por { $minutes } minutos
security-sudo-note = Pede sua senha antes, e se encerra sozinha na hora escolhida, quando a máquina dorme e na próxima partida.
security-sudo-recent = Nesta semana:
security-finger-reader = Leitor
security-finger-enrolled = cadastrado
security-finger-enrol = Cadastrar
security-finger-ring-note = As setas para os lados escolhem o dedo; pede sua senha antes, depois alguns toques.
security-finger-sudo = Dedos para o administrador num terminal
security-finger-sudo-note = Só num terminal: aqui, e em qualquer outro lugar, é a sua senha.
security-finger-ask-remove = Remover seu { $finger }? Ele não poderá ser usado até ser cadastrado de novo.
security-finger-left-thumb = polegar esquerdo
security-finger-left-index-finger = indicador esquerdo
security-finger-left-middle-finger = médio esquerdo
security-finger-left-ring-finger = anelar esquerdo
security-finger-left-little-finger = mínimo esquerdo
security-finger-right-thumb = polegar direito
security-finger-right-index-finger = indicador direito
security-finger-right-middle-finger = médio direito
security-finger-right-ring-finger = anelar direito
security-finger-right-little-finger = mínimo direito
security-finger-step-completed = Cadastrado.
security-finger-step-stage-passed = Bom. Toque o leitor de novo.
security-finger-step-retry-scan = Toque o leitor de novo.
security-finger-step-swipe-too-short = Curto demais; toque de novo, por mais tempo.
security-finger-step-not-centred = Fora do centro; toque de novo, centralizado.
security-finger-step-remove-and-retry = Levante o dedo e toque de novo.
security-finger-step-too-fast = Rápido demais; toque de novo, mais devagar.
security-finger-step-duplicate = Este dedo já está cadastrado.
security-finger-step-data-full = O leitor não guarda mais dedos.
security-finger-step-failed = O leitor não conseguiu cadastrar.
security-finger-step-disconnected = O leitor sumiu.
security-finger-step-unknown = O leitor disse algo inesperado.
security-device-secure-boot = Secure Boot
security-device-secure-boot-enabled = ligado
security-device-secure-boot-disabled = desligado
security-device-secure-boot-setup-mode = em modo de configuração: as chaves podem mudar
security-device-secure-boot-unsupported = não é uma partida UEFI
security-device-tpm = Chip de segurança (TPM)
security-device-tpm-absent = nenhum
security-device-tpm-present = versão { $version }
security-device-encryption = O disco do sistema
security-device-encrypted = cifrado
security-device-not-encrypted = não cifrado
security-device-hsi = Nível de segurança do firmware
security-device-firmware-updates = Atualizações de firmware
security-device-firmware-updates-count = { $count ->
    [0] nenhuma conhecida
    [one] uma disponível
   *[other] { $count } disponíveis
  }
security-device-firmware-note = Só leitura: firmware não é instalado daqui.
security-device-updates = Atualizações automáticas
security-device-lock = Bloqueio de tela
security-device-lock-none = nenhum: este sistema não tem tela de bloqueio
security-device-autologin = A partida da máquina
security-device-autologin-gate = a senha do disco é a única porta
security-privacy-mac = Endereço de hardware em { $name }
security-privacy-mac-permanent = o do próprio aparelho: as redes podem seguir você
security-privacy-mac-preserve = como o aparelho tem
security-privacy-mac-random = novo a cada vez
security-privacy-mac-stable = próprio para esta rede
security-privacy-mac-explicit = um que você escolheu
security-privacy-mac-default = a escolha do sistema
security-privacy-dns = DNS cifrado em { $name }
security-privacy-dns-yes = sempre
security-privacy-dns-opportunistic = quando o servidor oferece
security-privacy-dns-no = nunca
security-privacy-dns-default = a escolha do sistema
security-privacy-none = Nenhuma rede está ativa.
security-privacy-ssh = Servidor SSH
security-privacy-ssh-on = algo responde na porta 22
security-privacy-note = Isto é lido, não mudado aqui: as configurações de cada conexão são da rede.
security-online-vt-key = Chave do VirusTotal
security-online-key-kept = guardada
security-online-key-none = nenhuma: cole uma da sua conta do VirusTotal
security-online-vt-forget = Esquecer a chave do VirusTotal
security-online-check-file = Checar um arquivo
security-online-file-note = Só a impressão dele (SHA-256) vai para o VirusTotal. Um arquivo que ele nunca viu só é enviado se você disser sim.
security-online-check-email = Checar um endereço de e-mail
security-online-email-note = O próprio endereço vai para o XposedOrNot.
security-online-email-clean = { $address } não está em nenhum vazamento conhecido.
security-online-email-found = { $address } está em { $count ->
    [one] um vazamento
   *[other] { $count } vazamentos
  }: { $names }
security-online-check-password = Checar uma senha
security-online-password-note = Só cinco caracteres da impressão dela (SHA-1) saem desta máquina; o resto é comparado aqui.
security-online-password-clean = Essa senha não está em nenhum vazamento conhecido.
security-online-password-found = Essa senha foi encontrada { $count ->
    [one] uma vez
   *[other] { $count } vezes
  } em vazamentos: não a use.
security-online-refusal-no-key = { $service } precisa da sua chave: cole-a na linha dele.
security-online-refusal-key-refused = { $service } recusou a chave.
security-online-refusal-too-many-asks = { $service } pede para perguntar mais devagar; tente de novo em um minuto.
security-online-refusal-nothing-found = { $service } não achou nada.
security-online-refusal-nothing-to-ask = Isso não é algo que se pergunte ao { $service }.
security-online-refusal-unreachable = Não foi possível falar com { $service }.
security-online-refusal-malformed = { $service } respondeu algo que isto não lê.
security-online-refusal-too-large = Grande demais para { $service }.
security-online-refusal-unreadable = O arquivo não pôde ser lido.
security-online-refusal-not-let = { $service } não pode ser perguntado: o endereço dele não está liberado.
security-service-virustotal = VirusTotal
security-service-xposedornot = XposedOrNot
security-service-pwnedpasswords = Pwned Passwords
security-vt-ask-upload = O VirusTotal nunca viu { $file }. Enviar o próprio arquivo?
security-vt-upload-warning = Um arquivo enviado ao VirusTotal é compartilhado com os parceiros dele e pode ser baixado pelos assinantes pagos. Não envie nada privado.
security-vt-state-asking = perguntando…
security-vt-state-unknown = nunca visto, não enviado
security-vt-state-sending = enviando, { $percent }%
security-vt-state-pending = enviado; esperando a análise
security-vt-verdict-clean = limpo: nenhum de { $engines } motores achou nada
security-vt-verdict-suspicious = suspeito: { $found } de { $engines } motores
security-vt-verdict-malicious = malicioso: { $found } de { $engines } motores
security-rule-word-allow = permitir
security-rule-word-deny = negar
security-rule-word-from = de
security-rule-word-on = em
security-rule-word-home = casa
security-rule-word-public = pública
security-rule-word-everywhere = tudo
security-rule-word-any = qualquer
security-about-show = Vai até onde isto está
security-about-profile = Casa ou pública: uma rede pública não deixa entrar o que suas regras de casa deixariam
security-about-log = Quantas conexões recusadas vão para o registro
security-about-rule = Mudar esta regra, em palavras; a tecla de apagar a tira
security-about-allow-home = Permite esta porta nas redes de casa, com sua senha
security-about-allow-public = Permite esta porta também nas redes públicas, com sua senha
security-about-finger = Remove este dedo, depois de uma pergunta
security-about-finger-sudo = Deixa um dedo responder ao administrador num terminal, com sua senha
security-about-report = Abre o relatório do arquivo na página do VirusTotal
security-about-url = Abre a página no seu navegador
security-about-answer = Responde à pergunta
security-suggest-vpn = Usar uma VPN: importe o arquivo dela na vista de rede
security-suggest-tor = Passar pelo Tor
security-suggest-tailnet = Chegar às suas máquinas pela sua tailnet
security-suggest-hibp = Have I Been Pwned, no seu navegador (nenhum endereço vai junto)

# THE PERSON'S CONTAINERS (LEADER s o) — `containers.rs`, its words
# (`containers_words.rs`), its threads and its window. A status line, a name
# and a log line are podman's own words and are drawn as they came; a key is
# never text here: every sequence arrives as `{ $keys }`.
containers-title = Containers
group-containers = containers
mode-containers = containers
cmd-containers-open = Seus containers: pods, compose, imagens, volumes e redes
cmd-containers-window = Seus containers, numa janela
containers-down = descer
containers-up = subir
containers-enter = abrir
containers-left = esquerda
containers-right = direita
containers-next-kind = próximo tipo
containers-previous-kind = tipo anterior
containers-next-tab = próxima aba
containers-previous-tab = aba anterior
containers-start-stop = iniciar/parar
containers-restart = reiniciar
containers-remove = remover
containers-logs = log
containers-shell = shell
containers-inspect = inspecionar
containers-pull = baixar
containers-build = construir
containers-login = entrar
containers-prune = limpar
containers-reveal = mostrar valores
cmd-containers-down = A próxima linha
cmd-containers-up = A linha de cima
cmd-containers-enter = Abrir a linha; num fato, copiá-lo
cmd-containers-left = A coluna à esquerda, ou voltar do detalhe
cmd-containers-right = A coluna à direita, ou o detalhe da linha
cmd-containers-next-kind = O próximo tipo: containers, pods, compose, imagens, volumes, redes
cmd-containers-previous-kind = O tipo anterior
cmd-containers-next-tab = A próxima aba do detalhe
cmd-containers-previous-tab = A aba anterior do detalhe
cmd-containers-start-stop = Iniciar ou parar; um projeto compose sobe, ou desce depois de uma pergunta
cmd-containers-restart = Reiniciar
cmd-containers-remove = Remover, depois de uma pergunta
cmd-containers-logs = O log, enquanto cresce
cmd-containers-shell = Um shell dentro dele, no terminal
cmd-containers-inspect = Tudo o que o podman sabe dele, realçado
cmd-containers-pull = Baixar uma imagem, pela fila
ask-containers-pull = Imagem a baixar
cmd-containers-build = Construir uma imagem a partir de uma pasta, pela fila
ask-containers-build = Pasta a construir
cmd-containers-login = Entrar num registro: o nome dele e o seu usuário, depois a senha numa linha oculta
ask-containers-login = Registro e usuário (quay.io alice)
cmd-containers-login-password = A senha do login no registro que está sendo digitado, numa linha oculta
ask-containers-login-password = A senha do registro
cmd-containers-prune = Remover o que nada usa, depois de uma pergunta
cmd-containers-reveal = Mostrar os valores do ambiente, ou escondê-los de novo
containers-kind-containers = Containers
containers-kind-pods = Pods
containers-kind-compose = Compose
containers-kind-images = Imagens
containers-kind-volumes = Volumes
containers-kind-networks = Redes
containers-empty-containers = Nenhum container. Baixar põe uma imagem aqui, e o `podman run` ou o compose faz um.
containers-empty-pods = Nenhum pod.
containers-empty-compose = Nenhum projeto compose. Um aparece quando o compose faz os containers dele.
containers-empty-images = Nenhuma imagem.
containers-empty-volumes = Nenhum volume.
containers-empty-networks = Nenhuma rede.
containers-tab-overview = Visão geral
containers-tab-logs = Log
containers-tab-stats = Uso
containers-tab-inspect = Inspeção
containers-tab-ports = Portas
containers-tab-mounts = Montagens
containers-tab-env = Ambiente
containers-state-created = criado
containers-state-running = rodando
containers-state-paused = pausado
containers-state-exited = parado
containers-state-stopping = parando
containers-state-unknown = num estado que o podman não nomeou
containers-why-no-podman = Containers não fazem parte desta instalação: o podman não está aqui.
containers-why-unstartable = Não foi possível iniciar o podman.
containers-why-failed = O podman não conseguiu; o log do sistema diz por quê.
containers-why-garbled = O podman respondeu algo que não é o que foi pedido.
containers-why-not-found = Não está mais lá.
containers-why-refused = O registro recusou o login.
containers-why-too-large = A resposta do podman é grande demais para mostrar.
containers-reading = Perguntando ao podman…
containers-working = { $what } { $name }: { $stage }
containers-pod-value = { $status } · { $count }
containers-project-value = { $count ->
        [one] { $up } de 1 rodando
       *[other] { $up } de { $count } rodando
    }
containers-image-unnamed = sem nome ({ $id })
containers-heading-members = Os containers dele
containers-row-name = Nome
containers-row-image = Imagem
containers-row-state = Estado
containers-row-status = O podman diz
containers-row-created = Feito em
containers-row-started = Iniciado em
containers-row-finished = Terminou em
containers-row-exit = Terminou com
containers-row-command = Roda
containers-row-restart = Reinício
containers-row-restarts = Reiniciado
containers-row-network = Rede
containers-row-pod = Pod
containers-row-project = Projeto compose
containers-row-service = Serviço
containers-row-id = Id
containers-row-folder = Pasta
containers-row-file = Arquivo compose
containers-row-size = Tamanho
containers-row-used-by = Usada por
containers-row-platform = Feita para
containers-row-driver = Driver
containers-row-mountpoint = Guardado em
containers-row-subnet = Endereços
containers-row-dns = Nomes dentro dela
containers-used-by = { $count ->
        [0] nenhum container
        [one] um container
       *[other] { $count } containers
    }
containers-network-value = { $network }: { $address }
containers-on = ligado
containers-off = desligado
containers-open = abrir
containers-logs-waiting = Esperando o log…
containers-logs-empty = Ele não escreveu nada.
containers-logs-ended = O log termina aqui: o podman parou de acompanhá-lo.
containers-stats-waiting = Esperando a primeira contagem do podman…
containers-stats-not-running = Só um container rodando usa a máquina.
containers-stats-every = Contado de novo a cada { $seconds } segundos, enquanto isto aparece.
containers-stat-cpu = Processador
containers-stat-memory = Memória
containers-stat-memory-share = Do limite de memória dele
containers-stat-network = Rede entrando / saindo
containers-stat-disk = Disco lendo / escrevendo
containers-stat-pids = Processos
containers-ports-none = Ele não publica porta nenhuma: nada fora dele o alcança.
containers-port-label = { $published } para { $inside }
containers-port-local = só esta máquina
containers-port-open = aberta: o firewall está desligado
containers-port-allowed = liberada
containers-port-home-only = liberada só em casa
containers-port-shields = recusada: escudos erguidos
containers-port-refused = recusada pelo firewall
containers-port-unknown = regras ilegíveis
containers-port-firewall = As regras do firewall
containers-ports-blocked = Quem está em outra máquina não alcança uma porta que o firewall recusa; só esta máquina alcança. Uma regra sua a libera.
containers-ports-unknown = Não foi possível ler as regras do firewall, então não se sabe o que alcança estas portas.
containers-port-narrowed = liberada só para alguns endereços
containers-ports-shields = Os escudos estão erguidos: o firewall descarta toda conexão nova vinda de outra máquina, digam o que disserem as regras, até que sejam baixados.
containers-ports-narrowed = Uma regra sua libera só os endereços ou a conexão que ela nomeia; quem mais tentar não alcança estas portas.
containers-ports-open = O firewall está desligado: qualquer um numa rede em que esta máquina está alcança estas portas.
containers-inspect-env = Valores do ambiente
containers-row-nth = { $label } { $n }
containers-mounts-none = Ele não monta nada desta máquina.
containers-mount-value = { $source } · { $how } · { $kind }
containers-mount-writable = pode escrever
containers-mount-read-only = só leitura
containers-env-none = O ambiente dele está vazio.
containers-env-values = Valores
containers-env-hidden = escondidos
containers-env-shown = à mostra
containers-env-mask = ••••••
containers-ask-remove = Remover { $name }?
containers-ask-remove-running = Ele está rodando: é parado antes.
containers-ask-remove-pod = Os containers dele são removidos junto.
containers-ask-remove-used = Os containers feitos a partir dela são removidos junto.
containers-ask-prune-system = Remover todo container e todo pod parados, toda rede que nenhum container usa, toda imagem sem nome e o cache de construção?
containers-ask-prune-images = Remover toda imagem sem nome?
containers-ask-prune-volumes = Remover todo volume que nenhum container usa — e o que há dentro deles?
containers-ask-down = Derrubar { $name }? Os containers dele são parados e removidos; os volumes ficam.
containers-answer-no = Não
containers-answer-yes = Sim
containers-did-start = Iniciando
containers-did-stop = Parando
containers-did-restart = Reiniciando
containers-did-pause = Pausando
containers-did-unpause = Retomando
containers-did-remove = Removendo
containers-did-prune = Limpando
containers-echo-asked = { $what } { $name }…
containers-echo-done = { $what } { $name }: pronto.
containers-echo-refused = { $what } { $name }: { $why }
containers-echo-queued = { $what } { $name } foi para a fila — { $keys } a mostra.
containers-no-queue = A fila de transferências não responde, então nada foi para a fila.
containers-refused-not-here = Isso não é algo que se faça com { $kind }.
containers-refused-not-running = Só um container rodando tem um shell para abrir.
containers-refused-no-file = O compose não disse onde está o arquivo deste projeto.
containers-refused-no-folder = { $path } não é uma pasta.
containers-refused-no-containerfile = { $path } não tem um Containerfile nem um Dockerfile para construir.
containers-refused-login-words = Digite o registro e o seu usuário nele, com um espaço entre eles: quay.io alice.
containers-refused-no-login = Nenhum login espera uma senha: comece pelo registro e pelo usuário.
containers-echo-logging-in = Entrando em { $registry } como { $user }…
containers-echo-logged-in = Entrou em { $registry } como { $user }.
containers-echo-login-refused = O login em { $registry } não deu certo: { $why }
# The queue's words for the containers' jobs (`ronoctua-transfers`).
transfers-engine-container = containers
transfers-work-container-pull = baixar a imagem
transfers-work-container-build = construir a imagem
transfers-work-container-compose-up = subir o projeto
transfers-work-container-compose-down = derrubar o projeto
transfers-work-container-compose-pull = baixar as imagens do projeto

# OS PERTENCES (`LEADER s v`, `crate::belongings`): a configuração da
# pessoa, guardada cifrada num repositório privado que ela indica.
group-belongings = pertences
# O nome da janela na modeline (`br.ronoctua.belongings`).
mode-belongings = pertences
belongings-title = Pertences
cmd-belongings-open = Pertences: a configuração guardada no seu próprio repositório
cmd-belongings-window = Pertences, numa janela
cmd-belongings-sync = Sincronizar os pertences agora: pede a senha
cmd-belongings-up = Linha anterior
cmd-belongings-down = Próxima linha
cmd-belongings-less = O valor anterior da linha
cmd-belongings-more = O próximo valor da linha
cmd-belongings-enter = O que a linha sob o cursor faz
cmd-belongings-keep-local = Manter o lado desta máquina no conflito: ele vai por cima do do repositório
cmd-belongings-take-remote = Levar o lado do repositório no conflito: o desta máquina fica guardado à parte
cmd-belongings-keep-both = Manter os dois lados: o do repositório fica ao lado deste, com o nome da outra máquina
cmd-belongings-apply = Aplicar as decisões e terminar a sincronização
cmd-belongings-abandon = Desistir da sincronização preparada: nada é escrito, e a senha é esquecida
cmd-belongings-all-local = Todos os conflitos: manter o lado desta máquina
cmd-belongings-all-remote = Todos os conflitos: levar o lado do repositório
cmd-belongings-repository = Definir o endereço do repositório
cmd-belongings-add = Guardar outro arquivo ou pasta da sua home
cmd-belongings-auth = Chegar ao repositório por uma chave SSH de implantação, ou por um token
cmd-belongings-keygen = Gerar a chave SSH de implantação desta máquina
cmd-belongings-copy-key = Copiar a metade pública da chave de implantação, para entregá-la ao repositório
cmd-belongings-token = Guardar um token do repositório, legível só por você
cmd-belongings-user = O nome de usuário que o servidor do repositório pede
cmd-belongings-passphrase = Trocar a senha: pede a atual e depois a nova duas vezes
cmd-belongings-period = De quanto em quanto tempo lembrar de sincronizar
belongings-up = subir
belongings-down = descer
belongings-less = menos
belongings-more = mais
belongings-enter = fazer
belongings-keep-local = manter o meu
belongings-take-remote = levar o deles
belongings-keep-both = manter os dois
belongings-sync = sincronizar
ask-belongings-sync = Senha dos pertences
ask-belongings-repository = O endereço do repositório: git@servidor:caminho, ssh://… ou https://…
ask-belongings-add = Um arquivo ou pasta da sua home para guardar
ask-belongings-token = O token, como o servidor do repositório o deu
ask-belongings-user = O nome de usuário que o servidor pede
ask-belongings-passphrase = A senha em uso
ask-belongings-passphrase-new = A senha nova
ask-belongings-passphrase-again = A senha nova de novo
ask-belongings-sync-again = A senha de novo — um repositório novo fica com ela para sempre
about-belongings-conflict = Mostra o que difere entre os dois lados, quando os dois são texto
about-belongings-rule = Tira isto do que é guardado em todas as máquinas que você sincroniza
about-belongings-kept = Deixa de guardar isto: o arquivo fica onde está
about-belongings-excluded = Volta a guardar isto
about-belongings-never = Guarda mesmo assim: vai para todas as máquinas que você sincroniza
belongings-status = Estado
belongings-repository = Repositório
belongings-none-yet = nenhum ainda
belongings-last-sync = Última sincronização
belongings-never-here = nunca nesta máquina
belongings-next = Próximo lembrete
belongings-due-now = agora
belongings-no-reminder = sem lembretes
belongings-kept-now = Guardado agora
belongings-files-weigh = { $n ->
    [one] um arquivo, { $size }
   *[other] { $n } arquivos, { $size }
    }
belongings-last-run = Última execução
belongings-first-run = Esta máquina ainda não sincronizou com este repositório. A primeira sincronização restaura o que o repositório guarda; onde um arquivo daqui difere, o lado do repositório já vem escolhido.
belongings-prefs-refused = O belongings.json não se lê como configuração, então nada é escrito por cima dele: corrija à mão, ou apague.
belongings-no-places = Esta sessão não tem home ou diretório de runtime, e os pertences não têm onde morar.
belongings-outcome-synced = Em dia: { $pulled } escritos aqui, { $pushed } enviados, { $deleted } apagados.
belongings-outcome-waiting = { $n ->
    [one] Esperando sua decisão sobre um conflito.
   *[other] Esperando suas decisões sobre { $n } conflitos.
    }
belongings-outcome-abandoned = A sincronização preparada foi abandonada.
belongings-prepared-gone = Uma sincronização foi preparada aqui e se perdeu — a sessão terminou desde então —, então nada espera por você: sincronize de novo.
belongings-outcome-keyed = Uma chave de implantação foi gerada.
belongings-outcome-rekeyed = A senha foi trocada.
belongings-large = { $name } pesa { $size }: toda sincronização o carrega.
belongings-unreadable = { $n ->
    [one] Um arquivo daqui não pôde ser lido e ficou como estava dos dois lados.
   *[other] { $n } arquivos daqui não puderam ser lidos e ficaram como estavam dos dois lados.
    }
belongings-unwritten = { $n ->
    [one] Um arquivo do repositório não pôde ser escrito aqui; a próxima sincronização tenta de novo.
   *[other] { $n } arquivos do repositório não puderam ser escritos aqui; a próxima sincronização tenta de novo.
    }
belongings-sync-heading = Sincronizar
belongings-sync-now = Sincronizar agora
belongings-apply = Aplicar as decisões
belongings-decided-of = { $n } de { $of } decididos
belongings-abandon = Desistir desta sincronização
belongings-waiting-note = A sincronização está preparada e espera você: nada foi escrito aqui ainda.
belongings-plan-waits = Uma sincronização está preparada e espera as suas decisões: aplique-a ou desista dela antes.
belongings-conflicts = { $n ->
    [one] Um conflito
   *[other] { $n } conflitos
    }
belongings-conflicts-note = Mudou aqui e no repositório desde a última sincronização. Decida cada um: manter o desta máquina, levar o do repositório ou — no que você acrescentou — manter os dois.
belongings-all-keep-local = Todos: manter o desta máquina
belongings-all-take-remote = Todos: levar o do repositório
belongings-undecided-left = { $n ->
    [one] Um conflito ainda está sem decisão.
   *[other] { $n } conflitos ainda estão sem decisão.
    }
belongings-undecided = sem decisão
belongings-chosen = escolhido
belongings-sides = aqui: { $here } · repositório: { $there }, mudou em { $machine }
belongings-side = { $size }, { $when }
belongings-side-deleted = apagado
belongings-decided-keep-local = manter o desta máquina
belongings-decided-take-remote = levar o do repositório
belongings-decided-keep-both = manter os dois
belongings-decide-keep-local = Manter o desta máquina
belongings-decide-take-remote = Levar o do repositório
belongings-decide-keep-both = Manter os dois
belongings-diff-title = Diferença em { $name }
belongings-diff-note = As linhas que começam com - são desta máquina; as que começam com + são do repositório.
belongings-kept = O que é guardado
belongings-what-leaks = Tudo é cifrado com a sua senha antes de sair desta máquina. O repositório mostra quantos arquivos há, os tamanhos e quando cada sincronização aconteceu; nunca os nomes, os lugares ou o conteúdo.
belongings-added-by-you = acrescentado por você
belongings-added-elsewhere = acrescentado em outra máquina
belongings-taken-out = tirado por você
belongings-add = Guardar mais alguma coisa
belongings-large-note = O que você acrescenta é guardado inteiro, pastas e links inclusive. Acima de { $size }, a página avisa: toda sincronização o carrega.
belongings-never = Nunca guardado
belongings-auth-ssh = uma chave SSH de implantação
belongings-auth-token = um token HTTPS
belongings-access = Acesso
belongings-auth = Chega por
belongings-deploy-key = Chave de implantação
belongings-copy-it = copiar a chave pública
belongings-make-it = gerar uma
belongings-deploy-note = Dê a esta chave pública acesso de escrita ao repositório, como chave de implantação. A metade privada fica nesta máquina, legível só por você, e nunca é guardada nos pertences.
belongings-keygen-note = Esta máquina ainda não tem chave de implantação. Gerar uma leva um instante na fila; depois a metade pública aparece aqui para copiar.
belongings-token = Token
belongings-token-is-kept = guardado, legível só por você
belongings-user = Nome de usuário
belongings-token-note = O token fica num arquivo que só você lê, e chega ao git só pelo caminho — nunca numa linha de comando.
belongings-passphrase = Senha
belongings-change-passphrase = Trocar a senha
belongings-passphrase-note = A senha abre um arquivo pequeno, e esse arquivo abre o resto: trocá-la cifra de novo esse arquivo, em todas as máquinas de uma vez. Nada recupera uma senha perdida.
belongings-period = Lembrete
belongings-remind-every = Lembrar de sincronizar
belongings-period-days = { $n ->
    [one] todo dia
   *[other] a cada { $n } dias
    }
belongings-period-never = nunca
belongings-needs-key = Ainda não há chave de implantação: gere uma em Acesso e dê a ela acesso de escrita ao repositório.
belongings-needs-token = Ainda não há token: guarde um em Acesso.
belongings-nothing-prepared = Não há sincronização preparada.
belongings-no-conflict-here = O cursor não está num conflito.
belongings-both-only-added = Os dois lados só podem ser mantidos no que você mesmo acrescentou.
belongings-no-difference = A diferença deste arquivo não está mais aqui; sincronize de novo para vê-la.
belongings-not-text = Este arquivo não é texto, então não há diferença para mostrar: decida pelo tamanho e pela hora.
belongings-key-already = Esta máquina já tem uma chave de implantação.
belongings-no-key-yet = Ainda não há chave de implantação.
belongings-passphrase-first = A primeira sincronização define a senha: ainda não há nenhuma para trocar.
belongings-empty-secret = Nada foi digitado.
belongings-token-kept = O token está guardado.
belongings-passphrases-differ = As duas senhas novas diferem, então nada mudou.
belongings-first-passphrases-differ = As duas senhas diferem, então nada foi enviado.
belongings-not-written = Um arquivo dos pertences não pôde ser escrito; o log do sistema diz por quê.
belongings-never-kept = Isso nunca é guardado: é como esta máquina chega ao repositório.
belongings-outside-home = Só o que está na sua home pode ser guardado.
belongings-now-kept = { $name } agora é guardado nos pertences.
belongings-stopped-keeping = { $name } não é mais guardado nos pertences; ele fica onde está.
belongings-queued = Enviado para a fila: { $what }.
belongings-op-prepare = a sincronização
belongings-op-apply = a sincronização, com as suas decisões
belongings-op-abandon = desistir da sincronização
belongings-op-keygen = gerar a chave de implantação
belongings-op-rekey = a senha nova
belongings-invalid-empty = Nada foi digitado.
belongings-invalid-credentials-in-address = O endereço traz usuário e senha: dê só o endereço, e a chave ou o token em Acesso.
belongings-invalid-plain-http = Um endereço http:// manda o acesso às claras: use https:// ou SSH.
belongings-invalid-not-remote = Um caminho local ou file:// não é um repositório remoto.
belongings-invalid-unknown = O endereço é git@servidor:caminho, ssh://… ou https://….
belongings-refused-no-repository = Nenhum repositório definido: dê o endereço em Estado.
belongings-refused-no-passphrase = A senha não estava lá, ou outra pessoa podia ler o arquivo dela; ela não foi usada.
belongings-refused-wrong-passphrase = Essa senha não abre estes pertences.
belongings-refused-needs-access = O repositório quer um acesso que esta máquina não recebeu: uma chave de implantação ou um token, em Acesso.
belongings-refused-refused = O repositório recusou a chave ou o token desta máquina.
belongings-refused-unreachable = Não foi possível chegar ao repositório.
belongings-refused-raced = Outra máquina continuou sincronizando enquanto esta sincronizava; tente de novo.
belongings-refused-not-ours = Esse repositório não é de pertences, ou é de uma versão mais nova.
belongings-refused-empty = O repositório está vazio: a primeira sincronização define a senha.
belongings-refused-corrupt = Algo no repositório não se decifra ou não se lê.
belongings-refused-undecided = Um conflito ainda está sem decisão.
belongings-refused-no-session = Não há sincronização preparada para aplicar.
belongings-refused-no-tools = Esta instalação não tem git ou ssh-keygen.
belongings-refused-failed = Um arquivo daqui não pôde ser lido ou escrito; o log do sistema diz qual.
belongings-refused-busy = Outra execução dos pertences está em andamento: espere ela terminar.
belongings-reason-preference = as suas configurações
belongings-reason-programs = os programas que você instalou
belongings-reason-draft = um rascunho, ou o número de um programa rodando
belongings-reason-generated = refeito das suas configurações em cada máquina
belongings-reason-access = como esta máquina chega ao repositório
belongings-reason-own-state = o que os pertences guardam sobre si mesmos
belongings-reason-identity = a identidade da própria máquina: duas iguais colidiriam
belongings-reason-machine = o estado desta máquina
belongings-reason-history = um histórico desta máquina
belongings-reason-cache = um cache, refeito quando preciso
# A linha do menu do gerenciador (`C-m`, `k`).
files-belongings = guardar nos pertences, ou deixar de guardar
files-does-belongings = acrescenta ao que os pertences guardam no seu repositório e devolvem numa máquina nova; quando já é guardado, tira — o arquivo fica onde está
# As palavras da fila para uma sincronização.
transfers-engine-belongings = pertences
transfers-work-belongings = pertences
# A décima quinta pergunta do instalador.
inst-belongings = Repositório dos pertences
inst-belongings-help = Opcional. O repositório privado em que sua configuração fica guardada, cifrada: git@servidor:caminho, ssh://… ou https://…, nunca com senha dentro. A primeira sincronização depois da instalação devolve as suas configurações.
inst-step-belongings = Escrever o repositório dos pertences na sua home

# A lookup or a directory whose host consent.json does not let: nothing
# was sent. The engine says it as the consent row (`Trouble::Off`); this is
# the sentence of the refusal itself, where it is said apart from that row.
player-trouble-lookup-not-let = Nada perguntado a { $service }: endereço não permitido.
player-trouble-directory-not-let = Nada perguntado a { $source }: endereço não permitido.
# A web app's page whose host consent.json would not let (`Refusal::NotLet`).
software-refusal-not-let = O host dele não foi permitido, então nada foi enviado: um catálogo se permite pela linha dele em Fontes, o endereço de um web app ao digitá-lo.
software-consent-flathub = A busca do Flathub, as páginas dos programas dele e as figuras.
software-consent-fdroid = O índice do F-Droid, os ícones dele e os pacotes que ele serve.
software-consent-appimagehub = O catálogo de AppImages, e os lançamentos do GitHub para onde ele aponta.
software-consent-dashboard = O índice de ícones de onde sai a figura de um web app.

# THE ROW THAT ASKS FOR A HOST (`consent_rows.rs`), the same in every app:
# what pressing it does, what it is for, and why a press was not kept
# (`ronoctua_consent::Unkept`).
consent-ask = Perguntar a { $hosts }
consent-hosts-and = { $hosts } e { $last }
consent-note = { $what } Pressionar permite.
consent-allow = permitir
about-consent-allow = Deixa perguntar aos sites que a linha nomeia — o mesmo que pressionar a linha. Fica guardado no consent.json, para todo app.
consent-unkept-no-home = Não há pasta pessoal onde guardar a permissão para perguntar a { $hosts }.
consent-unkept-unparseable = O arquivo das permissões (consent.json) não se lê; ficou como estava, e { $hosts } não foi permitido.
consent-unkept-unwritable = A permissão para perguntar a { $hosts } não pôde ser gravada.
# THE WORDS OF `ronoctua-words`, for every door that translates or looks a
# word up: why it answered nothing, the parts of speech, who translated it
# and where a meaning came from.
words-why-empty = Não há nada para traduzir ou procurar.
words-why-too-long = O texto é mais longo do que o tradutor aceita de uma vez.
words-why-unknown-tongue = O tradutor não conhece essa língua.
words-why-wrong-pass = A permissão dada é para outro site, não o que isto perguntaria.
words-why-no-key = O tradutor precisa de uma chave, e nenhuma está guardada.
words-why-key-refused = O tradutor recusou a chave.
words-why-quota = A cota de hoje do tradutor acabou.
words-why-paced = Perguntado vezes demais; ele responde daqui a pouco.
words-why-not-found = A palavra não está no dicionário.
words-why-no-dictionary = Não há dicionário desta língua nesta máquina, e perguntar ao Wiktionary não foi permitido.
words-why-unreachable = O site não pôde ser alcançado.
words-why-failed = O site respondeu com uma falha.
words-why-garbled = O que voltou não é o que foi pedido.
words-part-noun = substantivo
words-part-proper-noun = nome próprio
words-part-verb = verbo
words-part-adjective = adjetivo
words-part-adverb = advérbio
words-part-pronoun = pronome
words-part-preposition = preposição
words-part-conjunction = conjunção
words-part-interjection = interjeição
words-part-numeral = numeral
words-part-determiner = determinante
words-part-phrase = expressão
words-part-affix = afixo
words-part-other = outro
words-provider-mymemory = MyMemory
words-provider-deepl = DeepL
words-provider-libretranslate = LibreTranslate
words-source-wordnet = Open English WordNet, nesta máquina
words-source-wiktionary = Wiktionary
words-translated-by = Traduzido por { $provider }
# FAST-ACCESS (`LEADER d`, `crate::fast_access`): the search of everything.
fast-access-title = Busca
# What the modeline names the search's windows (`br.ronoctua.fast-access`,
# and the one over everything).
mode-fast-access = busca
mode-fast-access-float = busca
group-fast-access = busca
cmd-fast-access-open = Buscar em tudo, no rodapé
cmd-fast-access-window = Buscar em tudo, numa janela
cmd-fast-access-column = Buscar em tudo, na coluna da direita
cmd-fast-access-float = Buscar em tudo, sobre a tela inteira
cmd-fast-access-launch = Abrir um aplicativo pelo id
cmd-fast-access-open-path = Abrir um arquivo com o que o abre
cmd-fast-access-reveal = Mostrar um arquivo no gerenciador de arquivos
cmd-fast-access-man = Ler uma página de manual num terminal
fast-access-placeholder = digite para buscar · { $slash } lista como estreitar a busca
fast-access-empty = Seus aplicativos, comandos, arquivos e o que eles dizem, páginas de manual, programas para instalar, ícones e contas — digite uma palavra. Uma barra e uma letra estreitam a busca.
fast-access-nothing = Nada encontrado para “{ $words }”.
fast-access-searching = Buscando…
fast-access-reading-icons = Lendo as tabelas de ícones…
fast-access-icons-refused = As tabelas de ícones não puderam ser lidas.
# The scopes a slash and a letter narrow to (`fast_access::Scope`): a noun,
# and what it searches.
fast-access-scope-all = tudo
fast-access-scope-audio = áudio
fast-access-scope-audio-does = arquivos de áudio, pelo nome e pelo que está escrito neles
fast-access-scope-video = vídeos
fast-access-scope-video-does = arquivos de vídeo nesta máquina
fast-access-scope-youtube = YouTube
fast-access-scope-youtube-does = seu catálogo de vídeos, e o próprio YouTube depois de uma pausa
fast-access-scope-images = imagens
fast-access-scope-images-does = imagens nesta máquina
fast-access-scope-documents = documentos
fast-access-scope-documents-does = documentos, livros e textos
fast-access-scope-names = nomes
fast-access-scope-names-does = arquivos e pastas só pelo nome
fast-access-scope-contents = conteúdo
fast-access-scope-contents-does = o que está escrito dentro dos arquivos
fast-access-scope-manuals = manuais
fast-access-scope-manuals-does = as páginas de manual do sistema e dos seus programas
fast-access-scope-programs = programas
fast-access-scope-programs-does = programas que dá para instalar
fast-access-scope-commands = comandos
fast-access-scope-commands-does = os comandos desta área de trabalho, com as teclas
fast-access-scope-web = web
fast-access-scope-web-does = Wikipedia e DuckDuckGo, depois de uma pausa
fast-access-scope-dictionary = dicionário
fast-access-scope-dictionary-does = o que uma palavra quer dizer
fast-access-scope-synonyms = sinônimos
fast-access-scope-synonyms-does = outras palavras para uma palavra
fast-access-scope-translate = traduzir
fast-access-scope-translate-does = uma tradução — um código de língua primeiro manda para ela ({ $prefix } en …)
fast-access-scope-icons = ícones
fast-access-scope-icons-does = glifos do Nerd Fonts, figuras do Lucide e caracteres Unicode
fast-access-scope-math = contas
fast-access-scope-math-does = contas, unidades, bases, datas e fusos — “2+2” funciona sem isso
fast-access-scope-currency = moedas
fast-access-scope-currency-does = dinheiro de uma moeda para outra, nas cotações de hoje
fast-access-scope-zones = fusos horários
fast-access-scope-zones-does = que horas são em outro lugar
fast-access-scope-recent = recentes
fast-access-scope-recent-does = o que você abriu daqui, o mais usado primeiro
fast-access-scope-prefixes = estreitar
fast-access-scope-prefixes-does = todos os jeitos de estreitar uma busca
# The sections of the results (`fast_access::SectionId`).
fast-access-section-prefixes = Estreitar a busca
fast-access-section-math = Conta
fast-access-section-currency = Moeda
fast-access-section-zones = Fusos horários
fast-access-section-words = Palavras
fast-access-section-recent = Recentes
fast-access-section-apps = Aplicativos
fast-access-section-commands = Comandos
fast-access-section-icons = Ícones
fast-access-section-files = Arquivos
fast-access-section-folders = Pastas
fast-access-section-contents = O que os arquivos dizem
fast-access-section-videos = Vídeos
fast-access-section-youtube = YouTube
fast-access-section-web = A web
fast-access-section-manuals = Páginas de manual
fast-access-section-packages = Programas para instalar
fast-access-section-settings = Esta busca
fast-access-strip-nerd-fonts = Nerd Fonts
fast-access-strip-lucide = Lucide
fast-access-strip-unicode = Unicode
fast-access-strip-none = { $collection }: nada corresponde
fast-access-strip-about = { $collection } · { $name }
fast-access-strip-earlier = { $collection }: a página anterior
fast-access-strip-later = { $collection }: a próxima página
# What the index answered nothing for (`ronoctua_index::Why`).
fast-access-index-why-no-engine = Esta compilação não tem índice.
fast-access-index-why-not-built = O índice ainda não foi montado; os arquivos aparecem à medida que ele percorre suas pastas.
fast-access-index-why-unreadable = O índice não pôde ser lido.
fast-access-index-why-other-format = O índice é de outra versão; está sendo montado de novo.
fast-access-index-why-busy = O índice está ocupado.
fast-access-index-why-full = O índice está cheio.
fast-access-index-why-failed = O índice não conseguiu responder isso.
# The web (`ronoctua_websearch`).
fast-access-web-wikipedia = Wikipedia
fast-access-web-duckduckgo = DuckDuckGo
fast-access-web-searxng = SearXNG
fast-access-web-brave = Brave Search
fast-access-web-kiosk = Buscar “{ $words }” na web, no Chrome
fast-access-web-for = Uma busca na web pelo que você digitou.
fast-access-web-why-empty = Não há palavras para buscar.
fast-access-web-why-wrong-pass = A permissão dada é para outro site.
fast-access-web-why-no-key = O Brave Search precisa de uma chave, e nenhuma está guardada.
fast-access-web-why-key-refused = O Brave Search recusou a chave.
fast-access-web-why-paced = Perguntado vezes demais; ele responde daqui a pouco.
fast-access-web-why-unreachable = O site não pôde ser alcançado.
fast-access-web-why-failed = O site respondeu com uma falha.
fast-access-web-why-garbled = O que voltou não é uma lista de resultados.
# YouTube (`/y`).
fast-access-youtube-for = Uma busca no YouTube pelo que você digitou.
fast-access-thumbs-for = As imagens dos vídeos encontrados.
fast-access-youtube-local = no seu catálogo
fast-access-youtube-online = no YouTube
fast-access-shorts-shown = Shorts: mostrados
fast-access-shorts-hidden = Shorts: deixados de fora
fast-access-youtube-why-absent = Esta instalação não tem yt-dlp.
fast-access-youtube-why-unstartable = O yt-dlp não pôde ser iniciado.
fast-access-youtube-why-slow = O YouTube demorou demais para responder.
fast-access-youtube-why-failed = O YouTube não respondeu à busca.
fast-access-youtube-why-garbled = O que voltou não é uma lista de vídeos.
fast-access-youtube-why-too-large = A resposta era grande demais.
fast-access-youtube-why-not-an-address = Isso não é um endereço.
fast-access-youtube-why-not-a-list = Isso é um vídeo, não uma lista.
fast-access-youtube-why-nothing-to-ask = Não há palavras para buscar.
fast-access-views = { $n ->
        [one] { $n } visualização
       *[other] { $n } visualizações
    }
# Words (`/d`, `/s`, `/t`).
fast-access-words-for = Procurando a palavra no Wiktionary.
fast-access-translate-for = Uma tradução do que você digitou.
fast-access-translate-into = Para { $tongue }
fast-access-synonyms-none = Nenhuma outra palavra para ela.
# Sums, currencies and zones.
fast-access-math-also = O mesmo, em outras unidades
fast-access-zone-none = Nenhum fuso horário tem esse nome.
# The detail pane (`fast_access_detail`).
fast-access-detail-name = Nome
fast-access-detail-address = Endereço
fast-access-detail-channel = Canal
fast-access-detail-duration = Duração
fast-access-detail-views = Visto
fast-access-detail-from = Encontrado
fast-access-detail-keys = Teclas
fast-access-detail-command = Comando
fast-access-detail-program = Programa
fast-access-detail-section = Seção
fast-access-detail-description = Sobre
fast-access-detail-source = De
fast-access-detail-zone = Fuso
fast-access-detail-time = Agora lá
fast-access-detail-scope = Busca
fast-access-detail-short = Um Short
fast-access-detail-version = Versão
fast-access-detail-licence = Licença
fast-access-detail-site = Site
fast-access-detail-runs = Executa
fast-access-detail-download = Download
fast-access-act-open = Abrir
fast-access-act-run = Executar
fast-access-act-copy = Copiar
fast-access-act-copy-address = Copiar o endereço
fast-access-act-play-player = Tocar no player
fast-access-act-play-kiosk = Abrir no quiosque do Chrome
fast-access-act-install = Instalar, pela fila
fast-access-act-centre = Mostrar na central de programas
fast-access-act-terminal = Ler num terminal
fast-access-act-narrow = Estreitar a busca a isto
fast-access-act-kiosk = Abrir no quiosque do Chrome
fast-access-no-shell = Este shell não sabe qual é o próprio programa, então não consegue abrir uma janela.
fast-access-unopened = Nada abre { $what }.
# The question a web video asks (`fast_access::Question::PlayIn`).
fast-access-play-in = Tocar “{ $title }” no:
fast-access-play-player = player
fast-access-play-kiosk = quiosque do Chrome
fast-access-play-remember = e fazer o mesmo daqui em diante
# The settings of the search, at the foot of the empty page.
fast-access-setting-park = Deixar a janela pronta depois de fechar
fast-access-setting-door = Vídeos da web tocam no
fast-access-door-ask = perguntado toda vez
fast-access-setting-searxng = Seu SearXNG, consultado junto dos outros
fast-access-searxng-none = nenhum
fast-access-searxng-bad = “{ $address }” não é um endereço https com um host.
fast-access-setting-brave = Sua chave do Brave Search
fast-access-brave-none = nenhuma
fast-access-brave-kept-short = guardada
fast-access-brave-kept = A chave do Brave Search está guardada, legível só por você.
fast-access-brave-forgotten = A chave do Brave Search foi esquecida.
fast-access-web-key-no-home = Não há pasta pessoal onde guardar a chave.
fast-access-web-key-empty = Nada foi digitado.
fast-access-web-key-malformed = A chave tem uma quebra de linha ou um caractere de controle.
fast-access-web-key-unwritable = Não foi possível gravar a chave.
fast-access-yes = sim
fast-access-no = não
fast-access-unkept = As configurações da busca (fast-access.json) não puderam ser gravadas: { $why }
ask-fast-access-tag = O novo valor da etiqueta
ask-fast-access-meta = O novo valor do detalhe
ask-fast-access-searxng = O endereço do seu SearXNG (https://…), vazio para nenhum
ask-fast-access-brave = Sua chave do Brave Search, vazio para esquecê-la
fast-access-recent-none = Nada foi aberto daqui ainda.
# The keys the header teaches (`fast_access::OF_THE_VIEW`).
fast-access-enter = abrir
about-fast-access-enter = Faz aquilo para que serve a linha do cursor: abre, executa, toca ou copia — e a busca fecha, menos na coluna.
fast-access-secondary = o outro jeito
about-fast-access-secondary = Faz a segunda coisa que a linha oferece: mostra um arquivo no gerenciador, um vídeo no outro player, a página de um programa.
fast-access-copy = copiar
about-fast-access-copy = Copia o que a linha é: um caminho, um endereço, uma resposta, um caractere.
fast-access-detail = detalhe
about-fast-access-detail = Leva as teclas para o detalhe ao lado dos resultados, e de volta.
fast-access-whole = inteiro
about-fast-access-whole = Dá ao detalhe a área inteira, e devolve.
fast-access-shorts = Shorts
about-fast-access-shorts = Mostra os Shorts do YouTube entre os vídeos encontrados, ou os deixa de fora.
fast-access-clear = limpar
about-fast-access-clear = Esvazia o que foi digitado.
fast-access-page = página
about-fast-access-page = Mostra os próximos ícones de uma faixa — ou os anteriores.

# THE RANKING OF THE VIDEO CATALOGUE, a page of the player (LEADER p k,
# player_ranking.rs). Titles and channel names are data.
player-page-ranking = Vídeos por pontuação
player-page-ranked-video = Este vídeo
cmd-player-ranking = Vídeos por pontuação: os melhores do catálogo de vídeos primeiro, a um clique da fila
player-ranking-reading = Lendo o catálogo de vídeos…
player-ranking-none = Nada pontuado ainda: o catálogo de vídeos ({ $keys }) adiciona os seus canais, e as varreduras dele enchem esta lista.
player-ranking-empty = Nenhum vídeo pontuado ainda: as varreduras não acharam nenhum, ou todos estão ocultos.
player-ranking-layout-grid = Miniaturas
player-ranking-added = Adicionado: { $title }
player-ranking-title = Título do vídeo
player-ranking-channel = Canal
player-ranking-score = Pontuação
player-ranking-length = Tempo de vídeo
player-ranking-published = Publicado
player-ranking-actions = Com este vídeo
player-ranking-play-now = Tocar agora
player-ranking-add = Pôr na fila
player-ranking-kiosk = Assistir no Chrome
player-ranking-download = Baixar
player-ranking-hide = Ocultar do ranking
player-ranking-delete = Apagar do catálogo
ask-player-ranking-delete = Apagar “{ $title }” do catálogo de vídeos para sempre? Ele não volta, nem numa varredura.
player-ranking-hidden = Oculto: { $title }. Os vídeos ocultos do catálogo de vídeos o trazem de volta.
player-ranking-deleted = Apagado para sempre: { $title }
player-ranking-not-changed = { $title } não mudou: { $why }
player-ranking-downloading = Baixando { $title }; { $keys } mostra as transferências.
player-ranking-downloading-plain = Baixando { $title }.
player-ranking-not-downloaded = A fila de transferências não aceitou { $title }: ela não está rodando.
# THE RANKING'S KEYS.
cmd-player-ranking-more = A página deste vídeo: tocar, pôr na fila, Chrome, baixar, ocultar, apagar
cmd-player-ranking-earlier = A página anterior do ranking
cmd-player-ranking-later = A página seguinte do ranking
cmd-player-ranking-first = A primeira página do ranking
cmd-player-ranking-last = A última página do ranking
cmd-player-ranking-layout = O ranking em lista, ou em miniaturas
# THE RANKING'S KEYS, AS THE HINT LINE SAYS THEM.
player-ranking-more = mais
player-ranking-earlier = página anterior
player-ranking-later = página seguinte
player-ranking-first = primeira página
player-ranking-last = última página
player-ranking-layout = lista ou miniaturas

mode-clipboard = clipboard

# Os tamanhos prontos do redimensionador e a memória dele (`resizer.rs`).
resizer-preset = tamanho pronto
about-resizer-preset = O próximo tamanho pronto de uma tela comum que cabe nesta.
resizer-preset-back = menor
about-resizer-preset-back = O tamanho pronto anterior a este.
resizer-no-preset = nenhum tamanho pronto cabe nesta tela
echo-capture-outside-the-room = Nada foi capturado: o retângulo ficou inteiro sob as cortinas do redimensionador, fora do espaço que elas deixam.

# As telas (`display.rs`): o arranjo vertical, a profundidade de cor, os
# valores gravados de uma tela apagada, e toda recusa com nome.
display-rise = pôr acima
display-sink = pôr abaixo
cmd-display-rise = A tela sob o cursor para cima, acima da vizinha
cmd-display-sink = A tela sob o cursor para baixo, abaixo da vizinha
display-depth = Profundidade de cor
display-depth-bits = { $bits } bits por cor
display-depth-asked = Profundidade pedida
display-dark-shows-filed = O compositor não está desenhando nesta tela: cada linha mostra o que está gravado para ela, e volta quando ela acender.
display-refused-compositor = o compositor respondeu com um erro
display-refused-unreadable = o arquivo das telas não abre, e gravar o substituiria — conserte ou apague
display-refused-panel = o painel não aceitou o brilho
display-refused-monitor = o monitor não respondeu

# As recusas de energia e de Bluetooth que não tinham nome, com nome: as
# palavras do daemon vão para o journal, nunca para a linha.
power-refused-timed-out = não teve resposta a tempo
power-refused-failed = não pôde ser feito
power-refused-no-bus = não alcançou o barramento do sistema
bluetooth-refused-timed-out = não teve resposta a tempo
bluetooth-refused-no-bus = o barramento do sistema não respondeu

# As recusas da rede e dos túneis, cada uma pelo nome: as palavras dos
# daemons vão para o journal, nunca para a linha.
network-refused-authority = esta sessão não tem permissão
network-refused-absent = o NetworkManager não está lá, ou a conexão sumiu
network-refused-invalid = um ajuste não foi aceito
network-refused-secrets = nenhuma senha foi dada, ou ela foi recusada
network-refused-failed = a conexão não pôde ser feita
network-refused-timed-out = não houve resposta a tempo
network-refused-no-bus = o barramento do sistema não respondeu
network-refused-no-radio = esta máquina não tem rádio Wi-Fi
network-refused-out-of-range = essa rede não está mais ao alcance
network-refused-no-device = não há dispositivo desse tipo
network-refused-no-profile = não há perfil salvo com esse nome
network-refused-import = o arquivo não é um túnel que o NetworkManager importe
network-refused-vault = o TPM não guardou nem devolveu a senha
network-refused-other = não funcionou
tunnel-refused-no-daemon = o daemon dele não está rodando
tunnel-refused-authority = o socket dele não é desta sessão
tunnel-refused-unit = o serviço dele não quis subir nem descer
tunnel-refused-not-written = o arquivo não pôde ser gravado
tunnel-refused-config = o Tor não leu a configuração
tunnel-refused-daemon = o daemon disse não
tunnel-refused-other = não funcionou
echo-tunnel-refused-line = O Tor não leu esta linha: { $line }

# O estado do gerenciador de arquivos na modeline: uma pergunta sobre quem
# pode mexer num arquivo, em alerta.
state-permission = permissão

# A janela do clipboard: a segunda tela do histórico do rodapé, seguido
# pelo socket de controle.
group-clipboard = clipboard
cmd-clipboard-window = O histórico do clipboard numa janela
clipboard-window-waiting = O histórico é do rodapé, e está a caminho.
clipboard-window-no-footer = O rodapé não está rodando, e o histórico fica nele: nada a mostrar.
clipboard-no-room = Não sobra espaço na tela para desenhá-lo: role de volta para cima, ou dê mais linhas à lista.
clipboard-act-stale = O histórico mudou antes de a tecla chegar, e nada foi feito: confira e aperte de novo.

# O "pedir a uma IA" da linha de vida, pelo Claude Code: o que sairia é
# juntado, mostrado num terminal, e perguntado com o "não" primeiro.
lifeline-ai-gathering = juntando o que iria: a unidade, como terminou, o journal e o core dump
lifeline-ai-no-report = o relatório não pôde ser escrito, e nada saiu desta máquina
lifeline-ai-shown = o que iria para o Claude Code está aberto num terminal; nada saiu ainda — enviar?
lifeline-ai-no = não, fica aqui
lifeline-ai-yes = sim, perguntar ao Claude Code
lifeline-ai-kept = nada saiu desta máquina, e o relatório foi apagado
lifeline-ai-opened = o Claude Code está aberto num terminal, com o relatório
lifeline-ai-no-claude = o Claude Code não está nesta máquina; ele vem com os agentes do perfil de desenvolvimento
lifeline-ai-prompt = A interface do meu computador (a unidade de usuário do systemd { $unit }) falhou. O arquivo { $path } tem a unidade, como ela terminou, as últimas linhas do journal e o core dump. Leia, me diga o que provavelmente aconteceu e o que eu posso fazer.
lifeline-report-title = O que aconteceu com a interface
lifeline-report-unit = Unidade
lifeline-report-result = Como terminou
lifeline-report-journal = As últimas linhas do journal
lifeline-report-core = O core dump
lifeline-report-nothing = (nada)

example-code = Um trecho de código
# A vista da rede: a ida e volta até o roteador, medida quando se pede.
network-latency = ida e volta até o roteador
network-latency-measure = medir
network-latency-measuring = medindo…
network-latency-ms = de { $fastest } a { $slowest } ms ({ $answered } de { $asked })
network-latency-silent = sem resposta
network-latency-refused = não deu para medir: { $error }
# Os nomes das máquinas da rede, pelo systemd-resolved.
network-lan-names-refused = os nomes não são conhecidos: { $why }
lan-resolve-why-no-bus = o barramento do sistema não respondeu
lan-resolve-why-no-resolved = o systemd-resolved não está rodando
lan-resolve-why-failed = o systemd-resolved não soube responder
# Uma VPN cujo arquivo pede usuário e senha.
ask-network-vpn-user = Usuário desta VPN
ask-network-vpn-password = Senha desta VPN
echo-network-vpn-stored = { $network } vai subir sem perguntar
echo-network-vpn-store-failed = a senha da VPN não foi guardada: { $reason }
# A internet de um celular, pelo Bluetooth.
bluetooth-fact-internet = usa a internet dele
bluetooth-about-internet = O celular compartilha a conexão pelo Bluetooth. A ligação é do NetworkManager, e a vista da rede também a mostra, com a mesma interface.
bluetooth-internet-through = sim, por { $interface }
echo-bluetooth-internet-on = saindo por { $device } agora
echo-bluetooth-internet-off = não sai mais por { $device }
echo-bluetooth-internet-failed = não deu para usar a internet de { $device }: { $reason }
# As moedas de um lugar, escolhidas na tela.
locale-currency = Moeda
locale-currency-pair = { $from } em { $into }
locale-currency-none = nenhuma escolhida
locale-not-a-currency-pair = Isso não são duas moedas que as cotações conhecem, de três letras cada, como EUR BRL.
ask-locale-currency = A moeda dele e a moeda em que vale, como EUR BRL
# Uma pergunta que oferece respostas para o que se digita.
line-offers = { $keys }: { $offers }
about-line-offer = Toma a primeira resposta oferecida para o que foi digitado.
studio-trouble-no-face = Um texto da cena não tem tipo de letra com que ser desenhado.
studio-trouble-rehearsal = Este é o estúdio do laço de desenvolvimento: ele ensaia, e não grava foto, gravação nem transmissão.
camera-virtual-effect = Ela passa por
camera-virtual-effect-preview = o efeito da prévia
# O enlace de um celular na vista da rede, e as recusas do próprio crate da rede.
network-fact-phone = celular
network-refusal-no-phone = o NetworkManager não tem um dispositivo Bluetooth para este celular: ele não está pareado, ou não compartilha a conexão
network-refusal-no-vpn = não existe mais uma VPN com esse nome
network-refusal-not-vpn = esse perfil não é de uma VPN

player-page-queue = A fila inteira
player-queue-entries = { $count ->
        [one] um item
       *[other] { $count } itens
    }
player-lane-language = Língua mostrada
player-lane-switched-off-length = desligada · { $length }
player-translator = Tradutor
player-translator-mymemory = MyMemory
player-translator-deepl = DeepL
player-translator-libretranslate = LibreTranslate
player-translator-address = Endereço do servidor
player-translator-not-an-address = “{ $typed }” não é um endereço https:// com um host: nada poderia ser perguntado ali.
player-key-deepl = Chave do DeepL
player-key-libretranslate = Chave do servidor
player-key-words-deepl = a chave do DeepL
player-key-words-libretranslate = a chave do servidor LibreTranslate
player-key-kept-said = Guardada: { $what }.
ask-player-translator-address = Endereço do servidor LibreTranslate (https://…)
ask-player-key-deepl = Chave de autenticação do DeepL
ask-player-key-libretranslate = Chave do servidor LibreTranslate
player-host-let = { $host } está permitido agora. Ponha de novo o que foi recusado.
player-stream-language-row = Língua do fluxo
player-stream-title-row = Nome do fluxo
player-stream-default-row = Marcado como padrão
player-stream-forced-row = Marcado como forçado
player-stream-language-of = a língua do fluxo { $index }
player-stream-title-of = o nome do fluxo { $index }
player-chapter-title-of = o título do capítulo em { $at }
ask-player-stream-language = A língua do fluxo, como código (en, pt-BR, por)
ask-player-stream-title = O nome do fluxo
ask-player-chapter-title = O título do capítulo
player-trouble-tag-chapters-only-matroska = O título de um capítulo só é gravado em Matroska, e este arquivo é { $container }.
player-trouble-tag-remux-would-drop = Este arquivo é grande demais para ser gravado no lugar, e a cópia que o grava perderia itens que ele guarda além destes campos. Os campos ficam como estão.
player-origin-matroska-track-of = Etiquetas Matroska do fluxo { $index } · { $kind }
player-origin-matroska-track-lang = Etiquetas Matroska do fluxo { $index } · { $kind } · { $lang }
player-origin-matroska-track-uid = Etiquetas Matroska da trilha de UID { $uid }
