# Interface texts, pt-BR. See the header of `en-GB.ftl`.

## Commands — the `M-x` list and the which-key labels

cmd-buffer-next = Próximo buffer
cmd-buffer-switch = Trocar de buffer
cmd-file-open = Abrir arquivo
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

## Modeline states

mode-normal = NORMAL
buffer-no-name = *sem nome*
mode-unknown = desconhecido

## Notifications

notif-empty = sem notificações
notif-history = Notificações
notif-critical = CRÍTICA

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
inst-compose-key-help = A tecla Compose: segurada com outras, faz acento. Sem ela, teclado sem acento nao os alcanca.
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
echo-no-on = { $sequencia } não está ligado a nada
# Same sequence, other case. Shown when the case IS the reason nothing
# matched — Caps Lock or Shift. Without it the screen names the consequence
# and hides the cause.
echo-no-on-box = { $sequencia } não está ligado — { $outra } está
echo-cancelled = Cancelado
echo-no-window = Nenhuma janela em foco
echo-no-action = { $comando } ainda não faz nada
echo-no-buffer = O navegador de glifos não foi construído
echo-no-opened = Não consegui abrir { $programa }
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
group-screen = tela
cmd-workspace-overview = Panorama dos espaços
cmd-window-float = Alternar flutuante
cmd-workspace-go = Ir para o espaço
cmd-workspace-move = Levar a janela para o espaço

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-notif-history = Histórico de notificações
cmd-notif-dismiss = Dispensar a notificação
cmd-keyboard-restore = Restaurar o teclado
group-notification = notificação
echo-keyboard-restored = Teclado restaurado
echo-keyboard-failed = Não consegui restaurar o teclado: { $motivo }

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
expansion-page = CtrlDir ←→

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
network-password = Senha de { $rede }
network-password-help = Guardada pelo NetworkManager, fora do repositório.

# ECHOES — what the minibuffer says after the action.
echo-network-connected = Conectado a { $rede }
echo-network-failed = Não consegui conectar a { $rede }: { $motivo }
echo-network-disconnected = Desconectado
echo-network-forgotten = { $rede } esquecida
echo-network-radio-on = Rádio Wi-Fi ligado
echo-network-radio-off = Rádio Wi-Fi desligado
echo-network-link = { $rede } · { $banda } · { $taxa } · sinal { $sinal }
echo-network-link-wired = { $rede } · { $taxa }
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
syncthing-syncing = sincronizando { $porcento }%
syncthing-stopped = parado
echo-syncthing-state = { $conectados } conectados · { $porcento }% · ↑{ $envio } ↓{ $recepcao }
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
network-remember = Lembrar { $rede }?  s = sim · n = não
# The list marker: tells "saved in NetworkManager" apart from "the password exists".
network-no-tpm = no TPM
echo-network-stored = Senha de { $rede } selada no TPM
echo-network-no-stored = Conectado. A senha não ficou guardada.
echo-network-store-failed = Conectado, mas não consegui guardar a senha: { $motivo }
## Falta de memoria
notif-oom-app = sistema
notif-oom-summary = Fechado para liberar memoria
notif-oom-pressure = O { $app } estava deixando a maquina travada por falta de memoria.
notif-oom-used = O { $app } encheu a memoria e a area de troca.
notif-oom-orphan = Algo fora dos seus buffers foi fechado: { $cgroup }

## O keymap do buffer em foco
which-buffer = Buffer em foco
cmd-editor-save = Salvar o arquivo
cmd-exemplo-reiniciar = Reiniciar o exemplo
