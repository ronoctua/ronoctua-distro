# Interface texts, pt-BR. See the header of `en-GB.ftl`.

## Commands — the `M-x` list and the which-key labels

cmd-buffer-next = Próximo buffer
cmd-buffer-switch = Trocar de buffer
cmd-file-save = Salvar arquivo
cmd-buffer-close = Fechar buffer
cmd-glyphs-nerd-fonts = Navegar glifos
cmd-mx-open = Executar comando…
cmd-buffer-terminal = Abrir terminal
cmd-buffer-terminal-external = Abrir terminal externo
cmd-monitor-btop = Monitor de processos (btop)
cmd-monitor-bottom = Monitor de processos (bottom)
cmd-session-quit = Encerrar sessão

## which-key prefix groups

group-brain = cérebro
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

mode-example = exemplo
buffer-glyphs = glifos
mode-glyphs = glifos
buffer-lucide = lucide
mode-lucide = lucide
buffer-memory = memória
mode-memory = memória
buffer-transfers = transferências
mode-transfers = transferências
buffer-format = formatar
mode-format = formatar
buffer-unicode = unicode
mode-unicode = unicode
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
state-update = atualização

## Notifications

notif-title = Notificações

# A faixa, acima da modeline. O contador só aparece com mais de uma.
notif-queue-position = { $shown }/{ $total }

# O CATÁLOGO — as notificações da própria distro, com um resumo e um corpo
# cada. Para uma entrada REESCRITA (a captura de tela do compositor), é isto
# que chega à tela. Para uma que nós mesmos levantamos, é o texto padrão e o
# nome da entrada no gerenciador.
notif-screenshot-summary = Tela capturada
notif-screenshot-body = A imagem está na área de transferência e salva no disco.
notif-memory-killed-summary = Fechado para liberar memória
notif-memory-killed-body = Um aplicativo foi fechado para a máquina não travar.

# O que cada notificação deixa fazer.
notif-action-edit = Editar a imagem
notif-action-open = Abrir a imagem
notif-action-folder = Abrir a pasta
notif-action-copy-path = Copiar o caminho
notif-action-memory-panel = Abrir o painel de memória

# OS FILTROS QUE A DISTRO JÁ TRAZ. O gerenciador os lista por estes nomes.
notif-rule-important-gets-room = O que é importante ganha espaço
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
notif-type-in-the-footer = Digitar é no rodapé — { $keys } abre por lá.

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
notif-act-in-the-footer = Esta roda pelo rodapé: { $keys }, e abra a notificação.
notif-act-no-footer = O rodapé não está escutando; a ação não foi a lugar nenhum.

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
inst-step-tpm = Selar a chave do disco no TPM

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
echo-no-buffer = O navegador de glifos não foi construído
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
group-window = janela/buffer
group-workspace = workspace
cmd-workspace-overview = Panorama dos espaços
cmd-window-float = Alternar flutuante
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
cmd-notif-filter-typed = Responder a pergunta da ficha do filtro
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

cmd-help-shortcuts = Manual de atalhos
group-help = ajuda
help-chords = SEGURANDO A LEADER
help-emergency = QUANDO A LEADER NÃO RESPONDE
help-leader-alternate = Abrir a lista de comandos

## Compositor actions — §13, "every OS action has a command"

# This section is LONG on purpose: these are the 141 actions niri exposes, each
# reachable by name in the command locator. The list of names is
# GENERATED from the pinned binary (nix/niri-actions.nix); the translations are written
# here, and the `niri-actions-translated` check rejects an action without a key in both
# languages. Bumping niri's tag STOPS the build until whatever appeared is
# translated — which is the opposite of showing up in English in the middle of the list.

cmd-niri-quit = Encerrar o niri
cmd-niri-power-off-monitors = Desligar os monitores (DPMS)
cmd-niri-power-on-monitors = Ligar os monitores (DPMS)
cmd-niri-spawn = Executar um programa
cmd-niri-spawn-sh = Executar um comando pelo shell
cmd-niri-do-screen-transition = Fazer uma transição de tela
cmd-niri-screenshot = Abrir a captura de tela
cmd-niri-screenshot-screen = Capturar a tela em foco
cmd-niri-screenshot-window = Capturar a janela em foco
cmd-niri-toggle-keyboard-shortcuts-inhibit = Alternar o inibidor de atalhos da superfície em foco
cmd-niri-close-window = Fechar a janela em foco
cmd-niri-fullscreen-window = Alternar tela cheia na janela em foco
cmd-niri-toggle-windowed-fullscreen = Alternar tela cheia em janela
cmd-niri-focus-window = Focar uma janela pelo id
cmd-niri-focus-window-in-column = Focar uma janela da coluna pelo índice
cmd-niri-focus-window-previous = Focar a janela anterior
cmd-niri-focus-column-left = Focar a coluna à esquerda
cmd-niri-focus-column-right = Focar a coluna à direita
cmd-niri-focus-column-first = Focar a primeira coluna
cmd-niri-focus-column-last = Focar a última coluna
cmd-niri-focus-column-right-or-first = Focar a coluna à direita, voltando à primeira no fim
cmd-niri-focus-column-left-or-last = Focar a coluna à esquerda, indo à última no começo
cmd-niri-focus-column = Focar uma coluna pelo índice
cmd-niri-focus-window-or-monitor-up = Focar a janela ou o monitor acima
cmd-niri-focus-window-or-monitor-down = Focar a janela ou o monitor abaixo
cmd-niri-focus-column-or-monitor-left = Focar a coluna ou o monitor à esquerda
cmd-niri-focus-column-or-monitor-right = Focar a coluna ou o monitor à direita
cmd-niri-focus-window-down = Focar a janela abaixo
cmd-niri-focus-window-up = Focar a janela acima
cmd-niri-focus-window-down-or-column-left = Focar a janela abaixo ou a coluna à esquerda
cmd-niri-focus-window-down-or-column-right = Focar a janela abaixo ou a coluna à direita
cmd-niri-focus-window-up-or-column-left = Focar a janela acima ou a coluna à esquerda
cmd-niri-focus-window-up-or-column-right = Focar a janela acima ou a coluna à direita
cmd-niri-focus-window-or-workspace-down = Focar a janela ou o espaço abaixo
cmd-niri-focus-window-or-workspace-up = Focar a janela ou o espaço acima
cmd-niri-focus-window-top = Focar a janela do topo
cmd-niri-focus-window-bottom = Focar a janela do fim
cmd-niri-focus-window-down-or-top = Focar a janela abaixo ou a do topo
cmd-niri-focus-window-up-or-bottom = Focar a janela acima ou a do fim
cmd-niri-move-column-left = Mover a coluna para a esquerda
cmd-niri-move-column-right = Mover a coluna para a direita
cmd-niri-move-column-to-first = Mover a coluna para o começo do espaço
cmd-niri-move-column-to-last = Mover a coluna para o fim do espaço
cmd-niri-move-column-left-or-to-monitor-left = Mover a coluna à esquerda ou para o monitor da esquerda
cmd-niri-move-column-right-or-to-monitor-right = Mover a coluna à direita ou para o monitor da direita
cmd-niri-move-column-to-index = Mover a coluna para um índice do espaço
cmd-niri-move-window-down = Mover a janela para baixo na coluna
cmd-niri-move-window-up = Mover a janela para cima na coluna
cmd-niri-move-window-down-or-to-workspace-down = Mover a janela para baixo na coluna ou para o espaço abaixo
cmd-niri-move-window-up-or-to-workspace-up = Mover a janela para cima na coluna ou para o espaço acima
cmd-niri-consume-or-expel-window-left = Absorver ou expelir a janela à esquerda
cmd-niri-consume-or-expel-window-right = Absorver ou expelir a janela à direita
cmd-niri-consume-window-into-column = Absorver a janela da direita para dentro da coluna
cmd-niri-expel-window-from-column = Expelir a janela de baixo da coluna
cmd-niri-swap-window-right = Trocar a janela com a da direita
cmd-niri-swap-window-left = Trocar a janela com a da esquerda
cmd-niri-toggle-column-tabbed-display = Alternar a coluna entre normal e abas
cmd-niri-set-column-display = Definir o modo de exibição da coluna
cmd-niri-center-column = Centralizar a coluna na tela
cmd-niri-center-window = Centralizar a janela na tela
cmd-niri-center-visible-columns = Centralizar todas as colunas visíveis
cmd-niri-focus-workspace-down = Focar o espaço abaixo
cmd-niri-focus-workspace-up = Focar o espaço acima
cmd-niri-focus-workspace = Focar um espaço pelo índice ou nome
cmd-niri-focus-workspace-previous = Focar o espaço anterior
cmd-niri-move-window-to-workspace-down = Levar a janela para o espaço abaixo
cmd-niri-move-window-to-workspace-up = Levar a janela para o espaço acima
cmd-niri-move-window-to-workspace = Levar a janela para um espaço pelo índice ou nome
cmd-niri-move-column-to-workspace-down = Levar a coluna para o espaço abaixo
cmd-niri-move-column-to-workspace-up = Levar a coluna para o espaço acima
cmd-niri-move-column-to-workspace = Levar a coluna para um espaço pelo índice ou nome
cmd-niri-move-workspace-down = Mover o espaço para baixo
cmd-niri-move-workspace-up = Mover o espaço para cima
cmd-niri-move-workspace-to-index = Mover o espaço para um índice do monitor
cmd-niri-set-workspace-name = Dar nome ao espaço em foco
cmd-niri-unset-workspace-name = Tirar o nome do espaço em foco
cmd-niri-focus-monitor-left = Focar o monitor à esquerda
cmd-niri-focus-monitor-right = Focar o monitor à direita
cmd-niri-focus-monitor-down = Focar o monitor abaixo
cmd-niri-focus-monitor-up = Focar o monitor acima
cmd-niri-focus-monitor-previous = Focar o monitor anterior
cmd-niri-focus-monitor-next = Focar o próximo monitor
cmd-niri-focus-monitor = Focar um monitor pelo nome
cmd-niri-move-window-to-monitor-left = Levar a janela para o monitor da esquerda
cmd-niri-move-window-to-monitor-right = Levar a janela para o monitor da direita
cmd-niri-move-window-to-monitor-down = Levar a janela para o monitor de baixo
cmd-niri-move-window-to-monitor-up = Levar a janela para o monitor de cima
cmd-niri-move-window-to-monitor-previous = Levar a janela para o monitor anterior
cmd-niri-move-window-to-monitor-next = Levar a janela para o próximo monitor
cmd-niri-move-window-to-monitor = Levar a janela para um monitor
cmd-niri-move-column-to-monitor-left = Levar a coluna para o monitor da esquerda
cmd-niri-move-column-to-monitor-right = Levar a coluna para o monitor da direita
cmd-niri-move-column-to-monitor-down = Levar a coluna para o monitor de baixo
cmd-niri-move-column-to-monitor-up = Levar a coluna para o monitor de cima
cmd-niri-move-column-to-monitor-previous = Levar a coluna para o monitor anterior
cmd-niri-move-column-to-monitor-next = Levar a coluna para o próximo monitor
cmd-niri-move-column-to-monitor = Levar a coluna para um monitor
cmd-niri-set-window-width = Mudar a largura da janela
cmd-niri-set-window-height = Mudar a altura da janela
cmd-niri-reset-window-height = Voltar a altura da janela para automática
cmd-niri-switch-preset-column-width = Alternar entre as larguras de coluna predefinidas
cmd-niri-switch-preset-column-width-back = Alternar as larguras de coluna, de trás para a frente
cmd-niri-switch-preset-window-width = Alternar entre as larguras de janela predefinidas
cmd-niri-switch-preset-window-width-back = Alternar as larguras de janela, de trás para a frente
cmd-niri-switch-preset-window-height = Alternar entre as alturas de janela predefinidas
cmd-niri-switch-preset-window-height-back = Alternar as alturas de janela, de trás para a frente
cmd-niri-maximize-column = Alternar a coluna maximizada
cmd-niri-maximize-window-to-edges = Alternar a janela maximizada até as bordas
cmd-niri-set-column-width = Mudar a largura da coluna
cmd-niri-expand-column-to-available-width = Expandir a coluna pelo espaço livre
cmd-niri-switch-layout = Alternar entre os layouts de teclado
cmd-niri-show-hotkey-overlay = Mostrar a lista de atalhos do compositor
cmd-niri-move-workspace-to-monitor-left = Levar o espaço para o monitor da esquerda
cmd-niri-move-workspace-to-monitor-right = Levar o espaço para o monitor da direita
cmd-niri-move-workspace-to-monitor-down = Levar o espaço para o monitor de baixo
cmd-niri-move-workspace-to-monitor-up = Levar o espaço para o monitor de cima
cmd-niri-move-workspace-to-monitor-previous = Levar o espaço para o monitor anterior
cmd-niri-move-workspace-to-monitor-next = Levar o espaço para o próximo monitor
cmd-niri-move-workspace-to-monitor = Levar o espaço para um monitor
cmd-niri-toggle-debug-tint = Alternar o tingimento de depuração nas janelas
cmd-niri-debug-toggle-opaque-regions = Alternar a visualização das regiões opacas
cmd-niri-debug-toggle-damage = Alternar a visualização do dano de saída
cmd-niri-toggle-window-floating = Alternar a janela entre flutuante e mosaico
cmd-niri-move-window-to-floating = Levar a janela para o flutuante
cmd-niri-move-window-to-tiling = Levar a janela para o mosaico
cmd-niri-focus-floating = Focar a camada flutuante
cmd-niri-focus-tiling = Focar o mosaico
cmd-niri-switch-focus-between-floating-and-tiling = Alternar o foco entre flutuante e mosaico
cmd-niri-move-floating-window = Mover a janela flutuante na tela
cmd-niri-toggle-window-rule-opacity = Alternar a opacidade da janela em foco
cmd-niri-set-dynamic-cast-window = Transmitir a janela em foco
cmd-niri-set-dynamic-cast-monitor = Transmitir o monitor em foco
cmd-niri-clear-dynamic-cast-target = Parar de transmitir qualquer coisa
cmd-niri-stop-cast = Parar uma transmissão do PipeWire
cmd-niri-toggle-overview = Alternar o panorama dos espaços
cmd-niri-open-overview = Abrir o panorama dos espaços
cmd-niri-close-overview = Fechar o panorama dos espaços
cmd-niri-toggle-window-urgent = Alternar a marca de urgência da janela
cmd-niri-set-window-urgent = Marcar a janela como urgente
cmd-niri-unset-window-urgent = Tirar a marca de urgência da janela
cmd-niri-load-config-file = Recarregar o arquivo de configuração
group-workspace-move = levar a janela
group-icons = ícones
glyphs-search = Digite para buscar um glifo das Nerd Fonts
# A palavra que o navegador ESCREVE no campo de busca — ver `unicode-prefix-*`.
glyphs-prefix-block = bloco
glyphs-prefix-tag = etiqueta
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

# --- The glyph card (Nerd Fonts browser) -----------------------------------
glyphs-sheet-codepoint = Codepoint
glyphs-sheet-decimal = Decimal
glyphs-sheet-block = Bloco
glyphs-sheet-set = Conjunto
glyphs-sheet-token = Token de ícone
glyphs-sheet-tray = Atribuído a um app
glyphs-sheet-tags-pt = Tags (pt)
glyphs-sheet-tags-en = Tags (en)
glyphs-sheet-help = ↑↓ formato · Enter copia · ←→ outro glifo · Esc volta

# The NINE copy formats. They have been commands since milestone 7 and never
# had a key — nobody noticed because nothing could reach them.
glyphs-sheet-copied = ✓ copiado
glyphs-sheet-copy-failed = ✗ não consegui copiar — veja o log
cmd-glyphs-copy-name = Nome
cmd-glyphs-copy-glyph = Glifo
cmd-glyphs-copy-css = Classe CSS
cmd-glyphs-copy-hex = Hexadecimal
cmd-glyphs-copy-utf16 = UTF-16
cmd-glyphs-copy-escape = Escape Unicode
cmd-glyphs-copy-html = Entidade HTML
cmd-glyphs-copy-rust = Literal Rust
cmd-glyphs-copy-token = Token do design system
cmd-glyphs-filter-set = Filtrar pelo conjunto
cmd-glyphs-filter-block = Filtrar pelo bloco
cmd-glyphs-filter-tag = Filtrar por tag
cmd-glyphs-filter-token = Só os que são token
cmd-glyphs-filter-app = Só os atribuídos a um app
cmd-glyphs-filter-clear = Limpar o filtro
cmd-glyphs-filter-panel = Escolher o conjunto
cmd-glyphs-assign = Atribuir a um app id

# --- The source sets, in the filter panel ----------------------------------
#
# A brand name is not translated; what describes one is — which is why these
# are keys and not data. They must be the SAME names the `SETS` table in
# gen-glyphdb.py turns into search tags: `browser-filters` requires that the
# label you read here finds, typed into the search, the whole set.
glyphs-set-all = Todos
glyphs-set-cod = Codicons
glyphs-set-custom = Personalizados
glyphs-set-dev = Devicons
glyphs-set-extra = Extra
glyphs-set-fa = Font Awesome
glyphs-set-fae = Font Awesome Extension
glyphs-set-iec = Símbolos de energia IEC
glyphs-set-indent = Indentação
glyphs-set-indentation = Indentação
glyphs-set-linux = Logos de distribuições
glyphs-set-md = Material Design
glyphs-set-oct = Octicons
glyphs-set-pl = Powerline
glyphs-set-ple = Powerline Extra
glyphs-set-pom = Pomicons
glyphs-set-seti = Seti-UI
glyphs-set-weather = Clima

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
echo-copied-value = Copiado: { $value }
echo-copy-failed = Não consegui copiar — veja o log
echo-qr-large = O conteúdo é grande demais para um QR code
echo-copy-only-text = Por ora só dá para recopiar texto
clipboard-image = Imagem
clipboard-unreadable = Copiado, e ilegível aqui: { $kind }

# --- The application launcher (LEADER a) -----------------------------------
cmd-apps-open = Abrir aplicativo
apps-empty = Nenhum aplicativo encontrado

# --- The Unicode symbol browser (LEADER i u) -------------------------------
cmd-unicode-symbols = Navegar símbolos Unicode
unicode-search = Digite para buscar um símbolo Unicode
# As palavras que o navegador ESCREVE no campo de busca. Digitadas, então
# minúsculas e sem acento; o analisador segue aceitando as de todos os idiomas.
unicode-prefix-symbols = simbolos
unicode-prefix-family = familia
unicode-prefix-block = bloco
unicode-prefix-name = nome
unicode-prefix-group = grupo
unicode-prefix-subgroup = subgrupo
unicode-prefix-tone = tom
cmd-unicode-copy-character = Caractere
cmd-unicode-copy-name = Nome oficial
cmd-unicode-copy-hex = Hexadecimal
cmd-unicode-copy-decimal = Decimal
cmd-unicode-copy-escape = Escape Unicode
cmd-unicode-copy-html = Entidade HTML
cmd-unicode-copy-rust = Literal Rust
cmd-unicode-copy-utf16 = UTF-16
cmd-unicode-copy-utf8 = Bytes UTF-8
cmd-unicode-filter-family = Filtrar pela família
cmd-unicode-filter-block = Filtrar pelo bloco
cmd-unicode-filter-category = Filtrar pela categoria
cmd-unicode-filter-symbols = Só os símbolos
cmd-unicode-filter-clear = Limpar o filtro
cmd-unicode-filter-panel = Escolher a família

# The seven families in the panel. The names are the SAME ones the generator
# turns into search tags — `symbols-filters` requires that the label you read
# finds, typed into the search, the whole family.
unicode-family-all = Todos
unicode-family-symbol = Símbolo
unicode-family-letter = Letra
unicode-family-number = Número
unicode-family-mark = Marca
unicode-family-punctuation = Pontuação
unicode-family-other = Outro

unicode-sheet-codepoint = Codepoint
unicode-sheet-decimal = Decimal
unicode-sheet-family = Família
unicode-sheet-category = Categoria
unicode-sheet-block = Bloco
unicode-sheet-decomposition = Decomposição
unicode-sheet-aliases = Também chamado
unicode-sheet-width = Largura
unicode-width-zero = Marca combinante — o apoio pontilhado não é dela
unicode-width-double = Ocupa duas células
unicode-sheet-help = ↑↓ formato · Enter copia · ←→ outro · Esc volta
unicode-sheet-copied = ✓ copiado
unicode-sheet-copy-failed = ✗ não consegui copiar — veja o log

# The emoji family and its NINE groups. The groups only show up in the panel
# when emoji is selected — they are sub-filters of it. The names are the SAME
# ones the generator turns into search tags.
unicode-family-emoji = Emoji
unicode-group-faces = Carinhas e emoções
unicode-group-people = Pessoas e corpo
unicode-group-nature = Animais e natureza
unicode-group-food = Comida e bebida
unicode-group-travel = Viagem e lugares
unicode-group-activities = Atividades
unicode-group-objects = Objetos
unicode-group-symbols = Símbolos
unicode-group-flags = Bandeiras
unicode-sheet-group = Grupo
unicode-sheet-subgroup = Subgrupo
cmd-unicode-filter-group = Filtrar pelo grupo

# === Lucide icon browser — `LEADER i l` ====================================
cmd-lucide-icons = Navegar ícones Lucide
lucide-search = Digite para buscar um ícone Lucide
# As palavras que o navegador ESCREVE no campo de busca — ver `unicode-prefix-*`.
lucide-prefix-category = categoria
lucide-prefix-name = nome

# The COPY FORMS. The first eight are lucide.dev's tabs, in its
# order; the rest are what it does not have and this system uses.
cmd-lucide-copy-svg = SVG
cmd-lucide-copy-jsx = JSX
cmd-lucide-copy-vanilla = HTML (lucide.js)
cmd-lucide-copy-react = React
cmd-lucide-copy-vue = Vue
cmd-lucide-copy-svelte = Svelte
cmd-lucide-copy-preact = Preact
cmd-lucide-copy-solid = Solid
cmd-lucide-copy-angular = Angular
cmd-lucide-copy-font = Fonte de ícones
cmd-lucide-copy-name = Nome
cmd-lucide-copy-component = Componente
cmd-lucide-copy-character = Caractere
cmd-lucide-copy-hex = Codepoint
cmd-lucide-copy-path = Traçado
cmd-lucide-copy-uri = Data URI
cmd-lucide-copy-base64 = Base64
cmd-lucide-copy-css = Regra CSS
cmd-lucide-copy-sprite = Sprite
cmd-lucide-filter-category = Filtrar pela categoria
cmd-lucide-filter-clear = Limpar o filtro
cmd-lucide-filter-panel = Escolher a categoria

# THE CUSTOMISATION. It changes the text that gets COPIED, not the drawing on screen:
# Lucide publishes a single stroke width, and no font would make it vary.
lucide-adjust-size = Tamanho
lucide-adjust-stroke = Espessura
lucide-adjust-absolute = Traço absoluto
lucide-adjust-color = Cor
lucide-adjust-cap = Pontas
lucide-yes = sim
lucide-no = não
lucide-cap-round = redondas
lucide-cap-straight = retas
lucide-cap-square = quadradas

lucide-sheet-name = Nome
lucide-sheet-component = Componente
lucide-sheet-codepoint = Codepoint
lucide-sheet-categories = Categorias
lucide-sheet-aliases = Também chamado
lucide-sheet-tags = Tags
lucide-sheet-help = ↑↓ linha · ←→ ajusta onde há a marca, senão troca de ícone · Enter copia · Tab restaura · Esc volta
lucide-sheet-copied = ✓ copiado
lucide-sheet-copy-failed = ✗ não consegui copiar — veja o log

# The CATEGORIES. The label is read on screen AND typed into the search: the
# `lucide-filters` check requires that it find, on its own, the whole category — which is why
# each one is ONE word, and the same one the lexicon uses to translate the category.
lucide-category-all = Todas
lucide-category-accessibility = Acessibilidade
lucide-category-account = Conta
lucide-category-animals = Animais
lucide-category-arrows = Setas
lucide-category-brands = Marcas
lucide-category-buildings = Edifícios
lucide-category-charts = Gráficos
lucide-category-communication = Comunicação
lucide-category-connectivity = Conectividade
lucide-category-cursors = Cursores
lucide-category-design = Design
lucide-category-development = Desenvolvimento
lucide-category-devices = Dispositivos
lucide-category-emoji = Emoji
lucide-category-files = Arquivos
lucide-category-finance = Finanças
lucide-category-food-beverage = Comida
lucide-category-gaming = Jogos
lucide-category-home = Casa
lucide-category-layout = Layout
lucide-category-mail = Correio
lucide-category-math = Matemática
lucide-category-medical = Médico
lucide-category-multimedia = Multimídia
lucide-category-nature = Natureza
lucide-category-navigation = Navegação
lucide-category-notifications = Notificações
lucide-category-people = Pessoas
lucide-category-photography = Fotografia
lucide-category-science = Ciência
lucide-category-seasons = Estações
lucide-category-security = Segurança
lucide-category-shapes = Formas
lucide-category-shopping = Compras
lucide-category-social = Social
lucide-category-sports = Esportes
lucide-category-sustainability = Sustentabilidade
lucide-category-text = Texto
lucide-category-time = Tempo
lucide-category-tools = Ferramentas
lucide-category-transportation = Transporte
lucide-category-travel = Viagem
lucide-category-weather = Clima

# The label of the section that separates the emoji sub-filters from the rest of the panel.
# Small and dimmed on purpose: it is a caption, not an option.
unicode-panel-emoji = só emoji
# The SKIN TONES. Like the families and the groups, the label is read on screen AND
# typed into the search — the `symbols-filters` check requires that it find the whole tone.
unicode-tone-light = Pele clara
unicode-tone-medium-light = Pele clara média
unicode-tone-medium = Pele média
unicode-tone-medium-dark = Pele escura média
unicode-tone-dark = Pele escura

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
syncthing-typed-in-the-footer = Isso se digita no minibuffer do rodapé.
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
network-remember = Lembrar { $network }?  s = sim · n = não
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
cmd-example-restart = Reiniciar o exemplo

## O painel de memoria — LEADER s m

cmd-memory-open = Memória do sistema
cmd-format-open = Formatar um disco
cmd-memory-full = Visão completa
cmd-memory-guard = Proteção contra travamento
cmd-memory-tab = Alternar RAM e disco
cmd-memory-refresh = Atualizar agora
cmd-memory-scan = Varrer
cmd-memory-scan-cancel = Parar a varredura
cmd-memory-stop = Encerrar o que está em foco
cmd-memory-sweep-store = Coletar o lixo do store
cmd-memory-sweep-generations = Remover gerações antigas
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
rescue-restart-niri = Reiniciar o compositor
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

files-format-zstd = .tar.zst — o que se guarda
files-format-gzip = .tar.gz — o que se envia para qualquer lugar
files-format-zip = .zip — o que se envia para quem usa Windows

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
files-compress = comprimir
files-extract = extrair
files-extract-tar = extrair o tar
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
files-ask-format = Comprimir em qual formato?
files-ask-name = Renomear para:
files-hidden = ocultos
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
files-does-open = abre no aplicativo padrão do tipo MIME; pasta entra na listagem
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
files-does-extract-tar = extrai o tar aqui mesmo, sem criar pasta
files-does-restore = devolve ao caminho de origem que a lixeira gravou
files-does-trash = move para a lixeira XDG; não atravessa sistema de arquivos
files-does-delete = unlink(2) ou rmdir(2) direto: sem lixeira e sem volta
files-does-shred = sobrescreve antes de apagar; num btrfs com cópia-na-escrita isso não garante nada

# E os do menu da barra lateral, que falam com o udisks.
sidebar-does-open = entra na pasta; se for dispositivo desmontado, abre a ficha dele
sidebar-does-mount = Filesystem.Mount do udisks2 — ele escolhe o ponto e as opções
sidebar-does-unmount = Filesystem.Unmount sem forçar: recusa se algo ainda estiver aberto ali
sidebar-does-eject = desmonta e corta a energia do disco (Drive.PowerOff): é o que deixa tirar
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

transfers-empty = nada em andamento
transfers-offline = a fila de transferências não está no ar
# A linha de dica, com a tecla resolvida na hora — nunca escrita aqui.
transfers-hint = { $pause } segura, { $cancel } desiste, { $sweep } limpa o que acabou

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
cmd-transfers-hold = segurar ou soltar
cmd-transfers-give-up = desistir
cmd-transfers-drop = tirar da lista
cmd-transfers-clear = limpar o que acabou
cmd-transfers-detail = detalhe desta linha

transfers-window-title = transferências
transfers-window-detail = a transferência, campo a campo
transfers-window-empty = nada na fila
transfers-window-unknown = não se sabe
transfers-window-hint = m p segura · m c desiste · m d detalha · m l limpa
transfers-window-hint-detail = m d volta à lista · m p segura · m c desiste
transfers-window-confirm-cancel = desistir apaga o pedaço já escrito. Enter confirma, qualquer outra tecla desiste da pergunta.
transfers-window-asked = pedido enviado

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
# ONE CLICK UPDATES EVERYTHING — the sequence in `assets/updates.nix`
#
# The sequence is declared once and generated from there: the table, the two
# runners, and this list of keys. `gen-updates.py` derives `update-<id>` and
# `update-does-<id>` from every step's id and emits them to `keys.txt`, so a
# step with no sentence in both languages cannot be written.
#
# The order below is the SEQUENCE'S order, which is the declaration's order —
# there is no `order` field, on purpose, and sorting these alphabetically would
# put back the second source the table exists to remove.
# ===================================================================
cmd-system-update = Atualizar o sistema
cmd-system-update-rehearsal = Ensaiar a atualização

# EACH STEP'S NAME, and under it the line that reaches the message line when the
# cursor is on it — the same `{prefix}-does-<id>` shape the file manager and
# the format buffer use. The name is what it is called; the line under it is
# what it really runs, and what it costs to be wrong about.
update-flake-update = buscar as versões novas
update-does-flake-update = nix flake update em /etc/nixos: adianta os inputs pinados e não mexe em nada além do flake.lock. Precisa de rede, e no ISO o /etc/nixos é só leitura — ali o passo é pulado e diz isso.
update-system-rebuild = reconstruir o sistema
update-does-system-rebuild = nixos-rebuild switch, pela unidade que já existe: compila, e pode trazer um kernel que nunca deu boot. A volta é a geração anterior, no menu de boot. Se falhar, a sequência para aqui.
update-store-collect-garbage = coletar o lixo do store
update-does-store-collect-garbage = nix-collect-garbage sem --delete-older-than: apaga o que nada referencia, e as gerações antigas ficam — apagá-las no mesmo fôlego seria apagar a volta.
update-store-optimise = deduplicar o store
update-does-store-optimise = nix store optimise: arquivos iguais viram hard link. Roda depois da coleta, senão otimizaria justamente o que estava para ser apagado.
update-old-generations = remover as gerações antigas
update-does-old-generations = nix-collect-garbage --delete-older-than 30d: apaga a volta. Depois dele o menu de boot não tem para onde voltar, e é por isso que ele vem desligado.
update-journal-vacuum = aparar o journal
update-does-journal-vacuum = journalctl --vacuum-size=256M: joga fora o registro mais antigo, inclusive o desta atualização — que é a resposta para "por que o rebuild falhou?". Vem desligado.
update-font-cache = refazer o cache de fontes
update-does-font-cache = fc-cache -rf: refaz do zero e ignora as datas, porque um rebuild pode ter mudado as fontes e um cache velho com data nova é exatamente o caso que traz alguém aqui.
update-tldr-cache = atualizar as páginas do tldr
update-does-tldr-cache = tldr --update, como você: a unidade semanal as baixa como `nobody`, e o cache vai parar numa pasta que ninguém lê. Precisa de rede; sem ela o passo é pulado, em vez de esperar o timeout e chamar isso de falha.
update-rust-toolchains = atualizar os toolchains do Rust
update-does-rust-toolchains = rustup update: atualiza os canais que já estão em ~/.rustup, e não instala nenhum. Aqui o Rust vem do flake, então essa pasta não existe e o passo é pulado dizendo isso — ele é para a máquina onde você pôs o rustup à mão.
update-node-global = atualizar os pacotes globais do npm
update-does-node-global = npm update --global: move o que já está no prefixo global e não instala nada. O node vem do flake; o prefixo global, não — sem essa pasta o passo é pulado e diz isso.
update-pnpm-global = atualizar os pacotes globais do pnpm
update-does-pnpm-global = pnpm update --global: o pnpm tem o store global dele, que não é o do npm — por isso são dois passos, e não um. Sem a pasta dele o passo é pulado e diz isso.
update-python-tools = atualizar as ferramentas do uv
update-does-python-tools = uv tool upgrade --all: sobe de versão as ferramentas que estão em $XDG_DATA_HOME/uv/tools, uma venv cada, e não acrescenta nenhuma. O uv vem do flake; elas, não — sem essa pasta o passo é pulado e diz isso.

# WHY A STEP WAS SKIPPED. The path or the name it is about travels beside the
# sentence as data (`Needs::about()`), which is why none of these names one.
update-needs-always = nada pode tornar este passo inaplicável
update-needs-binary = este programa não está no seu PATH
update-needs-directory = esta pasta não existe
update-needs-writable-file = este arquivo não existe, ou não dá para escrever nele
update-needs-network = o NetworkManager diz que esta máquina não está conectada

# WHAT A FAILURE DOES TO THE REST OF THE SEQUENCE. Two answers and no third:
# this runs from a unit, and there is nobody to ask.
update-on-failure-stop = se falhar, a sequência para aqui
update-on-failure-carry-on = se falhar, a sequência segue para o próximo passo

# THE FOUR CLASSES A STEP CAN BE. They decide the order laws, and the view
# groups by them.
update-kind-move-inputs = move os inputs pinados
update-kind-apply-configuration = aplica a configuração
update-kind-refresh-cache = refaz um cache
update-kind-reclaim-space = recupera espaço em disco


# ===================================================================
# A TELA DO CÉREBRO — LEADER b b, e uma janela só dela
#
# O que a máquina faz SOZINHA, quando, e no que deu da última vez. Nenhuma
# frase daqui escreve uma data: o buffer as desenha em ISO, porque um
# componente não traduz (inv. 11) e *19 ago* contra *19 Aug* é exatamente o que
# ele não pode inventar. O que está escrito abaixo são as palavras EM VOLTA dos
# números, nunca os números.
#
# SÃO DOIS REGISTROS, e o segundo é a razão da tela existir. `brain-hold-*` diz
# o que uma rodada está ESPERANDO — máquina ocupada, não rodada quebrada — e
# `brain-why-*` diz por que uma não aconteceu de jeito nenhum. Nenhum dos dois
# diz que alguém foi perguntado e não respondeu: o caminho da atualização nunca
# pergunta, e o `updates-never-ask` é o check que prova. Motivo nesta tela é da
# máquina, ou é decisão que o dono tomou de propósito.
# ===================================================================
cmd-brain-open = O que a máquina faz sozinha
cmd-brain-run = Rodar agora
cmd-brain-rehearse = Ensaiar, sem mudar nada
cmd-brain-abort = Interromper
cmd-brain-proceed = Começar já, sem a contagem
cmd-brain-toggle-step = Ligar ou desligar este passo
cmd-brain-reset-steps = Voltar aos passos que a distro traz
cmd-brain-interval = De quanto em quanto tempo
cmd-brain-window = Em que parte do dia
cmd-brain-switch = Ligar ou desligar esta tarefa

brain-title = O que a máquina faz sozinha
brain-nothing = não há nada que a máquina faça sozinha
brain-hint = Enter abre · m r roda agora · m e ensaia · m o liga e desliga
brain-hint-update = m t liga o passo · m i a cada · m w quando · m a interrompe · Esc volta

# OS TRÊS BLOCOS da página de trás, e os rótulos na beirada esquerda deles.
# Aquela coluna tem dezesseis células e a unidade ao lado tem dez: são curtos
# porque as colunas são, e uma palavra mais longa aqui é uma palavra que alguém
# lê com o fim cortado.
brain-steps = o que roda
brain-when = quando roda
brain-doing = agora
brain-outcome = resultado
brain-last = a última vez
brain-every = a cada
brain-days = dias
brain-part = parte do dia
brain-next = próxima
brain-waiting = motivo
brain-soon = começa em
brain-failed-at = parou no passo
brain-rehearsal = um ensaio — não muda nada
brain-never = nunca
brain-changed = você mudou
brain-needs-root = roda como root, com a permissão da máquina inteira

# A ÚNICA TAREFA QUE EXISTE, e sob o cursor a linha discreta dizendo o que
# aquilo quer dizer de verdade — a regra que os menus do formatador já seguem,
# em que toda linha carrega um segundo registro.
brain-task-system-update = Atualizar o sistema
brain-task-system-update-does = a máquina traz o próprio software para frente e monta o sistema novo como entrada de boot, sem trocar: nada muda debaixo de uma sessão aberta, e quem passa a valer é o próximo boot — uma hora que você escolhe.

# OS PASSOS DELA, na ordem em que rodam. O comando de verdade anda ao lado de
# cada um como dado, sob o cursor, então estes dizem PARA QUE o passo serve, e
# não o que ele digita.

# ONDE A TAREFA ESTÁ AGORA. São catorze células na última coluna da página
# inicial, então é uma frase curta cada.
brain-standing-idle = esperando
brain-standing-queued = na fila
brain-standing-running = rodando

# E COMO A ÚLTIMA RODADA TERMINOU. `falhou` é a palavra honesta para uma rodada
# que não chegou ao fim; se foi defeito ou decisão é trabalho do motivo, na
# linha logo abaixo.
brain-outcome-skipped = não rodou
brain-outcome-done = deu certo
brain-outcome-failed = falhou

# O QUE A RODADA ESTÁ ESPERANDO — as frases mais importantes desta tela. NADA
# QUEBROU E NADA SE PERDEU: a hora chegou, a máquina não estava em condição, e
# ela espera a próxima. Cada uma diz o que está no caminho, que é justamente
# aquilo que alguém poderia resolver se quisesse.
brain-hold-ram = RAM ocupada
brain-hold-cpu = CPU ocupada
brain-hold-battery = na bateria
brain-hold-busy = outra rodando
brain-hold-network = sem rede
brain-hold-asleep = dormindo

# E POR QUE UMA NÃO ACONTECEU DE JEITO NENHUM. As duas primeiras vão numa
# coluna de dezessete células, ao lado do nome da linha; as outras têm a
# largura de uma frase. `aborted` NÃO é erro — é o dono tendo decidido, e a
# tela concordando com ele.
brain-why-off = desligada
brain-why-no-systemctl = sem systemd aqui
brain-why-runaway = parou de dar notícia e a espera acabou — pode muito bem ainda estar rodando
brain-why-aborted = você interrompeu — nada deu errado
brain-why-postponed = você disse agora não, e a próxima hora mudou

# EM QUE PARTE DO DIA ela pode começar. As horas são os mesmos dígitos em
# qualquer língua; as palavras em volta, não.
brain-part-any = a qualquer hora, assim que a máquina estiver livre
brain-part-night = madrugada, das 02:00 às 05:00
brain-part-morning = manhã, das 08:00 às 12:00
brain-part-afternoon = tarde, das 13:00 às 18:00
brain-part-evening = noite, das 19:00 às 23:00

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

# A ATUALIZAÇÃO OLHADA DE PERTO — a marca na modeline e a janela que ela abre.
#
# A marca aparece só enquanto algo roda, e some sozinha. A janela existe
# separada dela porque uma marca que só se pode clicar é uma marca que o
# teclado não alcança.
cmd-update-watch = Acompanhar a atualização
hint-update-mark = clique para ver a atualização acontecer
echo-update-queued = Entrou na fila — começa quando a máquina estiver livre
echo-update-started = A atualização começou
echo-update-nowhere = Não há nada aqui que consiga começá-la
update-hint = ↑↓ um passo · C-j C-k a saída · G o fim · Esc fecha
update-no-output = ainda não disse nada
update-verdict-ran = deu certo
update-verdict-failed = falhou
update-verdict-off = desligado
update-verdict-skipped = não se aplica
update-verdict-rehearsed = só ensaiou
update-verdict-stopped = parou aqui
update-verdict-pending = ainda vem

## The theme chooser — LEADER s t. The themes are listed by id and have no
## key; these are the option beside them (the desk under the windows), its
## answers — one `theme-backdrop-<id>` per answer — and the hint.
theme-option-theme = Tema
theme-option-backdrop = Fundo
theme-backdrop-black = Preto
theme-backdrop-theme = A cor do tema
theme-hint = { $apply } aplica · { $left }/{ $right } troca de coluna · { $close } volta
echo-theme-unsaved = não deu para gravar o tema: { $error }

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
power-answer-in-the-footer = A trava é do rodapé: { $awake }, ou { $lid }
power-what-profile = o perfil
power-what-lock = a trava
power-what-thresholds = os limites
power-what-action = a ação
power-refused-authority = não permitido para esta sessão
power-refused-absent = o daemon não está lá, ou o hardware não consegue
power-refused-invalid = o valor foi recusado
power-refused-other = recusado
echo-power-refused = { $what } { $why } ({ $words })
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

# A SAÍDA — as três marcas, e o que o daemon acredita.
network-way = a internet
network-airplane = modo avião
network-tunnel-drop = RET derruba
cmd-network-airplane = Modo avião (alterna)
echo-network-refused = a rede recusou: { $error }
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
network-lan-refused = esta máquina não pode mandar ping: { $error }
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
network-import-asks = vai pedir senha na primeira vez que subir
network-import-bring = RET traz
network-import-leave = Esc deixa onde está
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
network-answer-in-the-footer = Senha se digita no rodapé: abra a rede por lá.
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
bluetooth-answer-in-the-footer = a pergunta é respondida no rodapé
bluetooth-type-in-the-footer = o nome é digitado no rodapé

# O QUE DEU ERRADO, dito como o nome da porta que fechou.
echo-bluetooth-refused = { $what }: { $why } ({ $words })
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
display-config-failed = O compositor não conseguiu ler a configuração dele. O que está na tela agora não volta depois de reiniciar.

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
display-arrange-how = As setas movem a tela sob o cursor de um lado para o outro.

display-ask = { $what } em { $screen }.
display-ask-seconds = Volta sozinho em { $seconds }s.
display-ask-forget = Tudo o que foi escolhido para { $screen } volta ao que o compositor decidir.
display-ask-nothing-yet = Nada mudou ainda.
display-leave-it = Deixar como está
display-forget-it = Esquecer tudo
display-screen-is-dark = o compositor não está desenhando nela
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
display-refused-not-writable = o arquivo é do root, e a regra que o abre não rodou neste painel
display-refused-nowhere = não há onde escrever
display-refused-other = não deu
echo-display-refused = As telas recusaram: { $why } ({ $words })
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
locale-zone = Fuso horário
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
geo-ask-filter = Região:
geo-queued = Na fila. O tamanho é o que o servidor responde, e dá para desistir por lá.
geo-no-home = Não há diretório de dados onde instalar.
geo-in-the-footer = O que se digita, digita-se no rodapé — { $keys } abre isto lá.
cmd-geo-open = Regiões e suas ruas, no rodapé
cmd-geo-window = Regiões e suas ruas, em janela
cmd-geo-up = Linha anterior
cmd-geo-down = Próxima linha
cmd-geo-enter = Instalar a região sob o cursor, ou instalar de novo
cmd-geo-find = Estreitar a lista de regiões
cmd-geo-remove = Remover a região instalada sob o cursor
group-geo = geografia

# ---------------------------------------------------------------------------
# LEADER d — the glance: what time it is somewhere, and the weather and money.
# ---------------------------------------------------------------------------
world-place-europe-london = Londres
world-place-america-sao-paulo = Brasília
world-temperature = { $degrees } °C
world-rate = 1 { $from } = { $much } { $to }
cmd-world-glance = Um relance: a hora, o tempo e o câmbio
locale-heading-places = Os lugares do relance
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
locale-sources-are-asked = Cada busca conta a esse host o endereço desta máquina e quais lugares ela observa.
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
locale-ask-zone = Fuso do relógio desta interface:
locale-ask-place = Fuso do lugar a acrescentar:
locale-ask-where = Latitude e longitude, dois números:
locale-no-such-zone = Não existe fuso com esse nome.
locale-not-a-coordinate = Isso não é uma latitude e uma longitude.
locale-in-the-footer = Digitar é no rodapé — { $keys } abre isto lá.
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
mode-locale = idioma/região

# O ATUIN — O HISTÓRICO DO SHELL, E SE ELE SAI DESTA MÁQUINA.
#
# O `nix/modules/home.nix` recusava a sincronização por escrito, e o dono pediu
# a outra resposta. O que não mudou é o comando com token no argv, e é por isso
# que o `secrets_filter` e o `history_filter` são linhas DESENHADAS desta vista
# em vez de padrões que ninguém lê.
atuin-typed-in-the-footer = Isso se digita no minibuffer do rodapé.
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
about-files-hidden = Mostra ou esconde as entradas cujo nome começa com um ponto.
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
about-transfers-cancel = Desiste desta transferência. O que já foi escrito é apagado.
about-transfers-forget = Tira esta linha da lista. O arquivo que ela escreveu fica onde está.
about-transfers-sweep = Tira da lista, de uma vez, todas as linhas que já acabaram.
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
# DRAWN, AND REFUSING. The owner asked for the option to exist and for its
# implementation to wait, so it says which of the two it is.
lifeline-ai-not-yet = essa ainda não existe — está aqui para ter onde nascer
# THE NESTED LOOP HAS NO UNIT. `nix/footer-single.nix` starts the footer as an
# ordinary background child, so there is nothing for systemd to restart — and
# saying so beats answering about a unit it has never heard of.
lifeline-not-a-unit = aqui não há unidade do systemd; o laço aninhado sobe o rodapé à mão
