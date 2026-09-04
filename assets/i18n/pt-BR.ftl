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

group-buffer = buffer
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

notif-empty = sem notificações
notif-history = Notificações
notif-critical = CRÍTICA

# A faixa, acima da modeline. O contador só aparece com mais de uma.
notif-queue-position = { $shown }/{ $total }
notif-mark-read = marcar como lida

# O CATÁLOGO — as notificações da própria distro, com um resumo e um corpo
# cada. Para uma entrada REESCRITA (a captura de tela do compositor), é isto
# que chega à tela. Para uma que nós mesmos levantamos, é o texto padrão e o
# nome da entrada no gerenciador.
notif-screenshot-summary = Tela capturada
notif-screenshot-body = A imagem está na área de transferência e salva no disco.
notif-memory-killed-summary = Fechado para liberar memória
notif-memory-killed-body = Um aplicativo foi fechado para a máquina não travar.

# O que cada notificação deixa fazer.
notif-action-edit = editar
notif-action-open = abrir
notif-action-folder = pasta
notif-action-copy-path = copiar o caminho
notif-action-memory-panel = painel de memória

# OS FILTROS QUE A DISTRO JÁ TRAZ. O gerenciador os lista por estes nomes.
notif-rule-important-gets-room = O que é importante ganha espaço
notif-rule-small-things-are-quick = Coisa pequena passa rápido

# O gerenciador: três listas, e um título para cada.
notif-column-happened = Recebidas
notif-column-suppressed = Filtradas
notif-column-catalogue = Notificações e filtros
notif-read = lida
notif-suppressed-by = filtrada por { $rule }
notif-decided-by = por { $rule }
notif-from-catalogue = { $entry }
notif-default = padrão da distro
notif-yours = seu ajuste
notif-nothing-here = nada aqui
notif-yes = sim
notif-no = não
notif-knob-show = aparece
notif-knob-life = fica
notif-window-title = { $pane } — { $n } não lidas
notif-window-keys = TAB coluna · ↑↓ move · ESPAÇO lida · r todas · RET ajustar · ←→ mudar
notif-act-in-the-footer = A ação roda pelo rodapé: LEADER n h, e a letra dela

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

# OS DOIS INTERRUPTORES QUE NÃO FAZEM NADA, e por quê. Um interruptor que não
# interrompe e não diz nada é a armadilha que este projeto já pagou uma vez.
notif-inert-vibrate = sem dispositivo háptico nesta máquina
notif-inert-audio = ainda não implementado — veja docs/PENDING.md

# O que o ponteiro explica na faixa.
hint-notif-bell = clique para abrir o gerenciador de notificações
hint-notif-read = marcar como lida e tirar da tela
hint-notif-countdown = quanto tempo ela ainda tem
hint-notif-action = { $action } — aperte { $key } no gerenciador

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
group-window = janela
group-workspace = workspace
cmd-workspace-overview = Panorama dos espaços
cmd-window-float = Alternar flutuante
cmd-workspace-go = Ir para o espaço
cmd-workspace-move = Levar a janela para o espaço

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-notif-history = Histórico de notificações
cmd-notif-window = Notificações em uma janela
cmd-notif-read-all = Marcar todas como lidas
cmd-notif-read = Marcar esta notificação como lida
cmd-notif-forget = Esquecer esta notificação
cmd-notif-act = Rodar a ação da notificação
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
cmd-clipboard-clipcascade-configure = Configurar o ClipCascade
clipboard-empty = Nada foi copiado ainda
clipboard-pinned = fixada
clipboard-content = Conteúdo
clipboard-types = Tipos
clipboard-size = Tamanho
clipboard-actions = Ações
clipboard-clipcascade = ClipCascade
clipboard-clipcascade-missing = não integrado
echo-copied = Copiado
echo-copy-failed = Não consegui copiar — veja o log
echo-qr-large = O conteúdo é grande demais para um QR code
echo-clipcascade-missing = O ClipCascade ainda não está integrado
echo-copy-only-text = Por ora só dá para recopiar texto
clipboard-image = Imagem

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

## Network — Wi-Fi and Ethernet (LEADER r)

cmd-network-open = Redes
cmd-network-scan = Varrer redes
cmd-network-connect = Conectar à rede
cmd-network-disconnect = Desconectar
cmd-network-forget = Esquecer a rede
cmd-network-wifi-toggle = Alternar o rádio Wi-Fi
cmd-network-link = Mostrar o enlace
group-network = rede

# THE VIEW. States the list shows in place of rows when there is no row to
# show — each says something different, and folding them into one generic
# "no networks" would hide the difference between not finding and not looking.
network-scanning = varrendo…
network-empty = nenhuma rede ao alcance
network-no-adapter = esta máquina não tem rádio Wi-Fi
network-radio-off = rádio desligado
network-connecting = conectando…
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

## Syncthing (LEADER r s)

cmd-syncthing-open = Abrir o Syncthing
cmd-syncthing-state = Estado do Syncthing
cmd-syncthing-pause = Pausar a sincronização
cmd-syncthing-resume = Retomar a sincronização
group-syncthing = syncthing
syncthing-line = Syncthing
syncthing-idle = em dia
syncthing-syncing = sincronizando { $percent }%
syncthing-stopped = parado
echo-syncthing-state = { $connected } conectados · { $percent }% · ↑{ $up } ↓{ $down }
echo-syncthing-stopped = O Syncthing não está respondendo
echo-syncthing-paused = Sincronização pausada
echo-syncthing-resumed = Sincronização retomada

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
files-when = { $y }-{ $m }-{ $d } { $hh }:{ $mm }
files-when-utc = { $y }-{ $m }-{ $d } { $hh }:{ $mm } UTC

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
files-theatre-on = dar a região inteira à prévia
files-theatre-off = devolver o espaço à listagem
files-theatre-nothing = nada está tocando, então não há região para dar
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
files-columns = permissões e dono
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
files-theatre = a região inteira
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
files-column-icon = tipo
files-column-name = nome
files-column-perms = permissões
files-column-owner = dono
files-column-group = grupo
files-column-size = tamanho
files-column-modified = modificado

# O que cada campo diz ao ponteiro. Campo que acende e não explica promete uma
# ação e não descreve nenhuma.
files-explain-owner = { $who } é o dono; clicar troca
files-explain-group = o grupo é { $who }; clicar troca
files-explain-size = { $bytes } bytes exatos
files-explain-measure = clique para medir esta pasta — é uma caminhada
files-explain-when = e { $seconds } segundos, na hora local desta máquina
files-explain-when-utc = e { $seconds } segundos, em UTC — não consegui ler o /etc/localtime

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
files-preview-failed = o que voltou não era uma imagem — o arquivo pode estar quebrado
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
# THE PHOTOGRAPH CARRIES A PLACE AND THIS SCREEN IS NOT PRINTING IT. What
# somebody needs before sending a picture to a stranger is not "hidden": it is
# that the file still knows, and that sending the file sends the place too.
files-value-place-withheld = gravado no arquivo, não mostrado aqui — vai junto se a foto for enviada
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
group-capture = captura

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

transfers-work-fetch = baixar
transfers-work-media = baixar vídeo
transfers-work-audio = baixar áudio
transfers-work-subtitles = baixar legenda
transfers-work-torrent = torrent


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
# A TELA DO CÉREBRO — LEADER s b, e uma janela só dela
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
