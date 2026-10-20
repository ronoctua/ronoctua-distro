# The texts `ronoctua-shell-kit` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Commands — the `M-x` list and the which-key labels

cmd-buffer-next = Próximo buffer
cmd-buffer-switch = Trocar de buffer
cmd-file-save = Salvar arquivo
cmd-buffer-close = Fechar buffer
cmd-buffer-terminal = Abrir terminal
cmd-session-quit = Encerrar sessão

## The modeline's two left segments

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
mode-memory = memória
mode-transfers = transferências
mode-format = formatar
mode-brain = cérebro
mode-update = atualização
buffer-no-name = *sem nome*
mode-unknown = desconhecido

## Notifications

# A faixa, acima da modeline. O contador só aparece com mais de uma.
notif-queue-position = { $shown }/{ $total }
# A LINHA DE DICAS lê cada id das tabelas de teclas do gerenciador como um texto
# curto — a convenção do gerenciador de arquivos; `cmd-<id>` é o rótulo longo do manual.
notif-read = lida ou não lida
notif-act = rodar a ação
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
hint-notif-countdown = quanto tempo ela ainda tem
hint-notif-paused = o tempo está parado enquanto o ponteiro está nela
hint-notif-action = { $action } — um clique escolhe, e um clique no escolhido executa

## Installer — the interface

cmd-window-left = Focar à esquerda
cmd-window-move-left = Mover à esquerda
cmd-workspace-go = Ir para o espaço

## Comandos do compositor — §13, "toda ação do SO tem um comando"

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
echo-copied = Copiado
echo-image-copied = A imagem está na área de transferência
echo-copied-value = Copiado: { $value }
echo-copy-failed = Não consegui copiar — veja o log

## The leader's three sets: the key tapped right after the leader.

leader-set-window = janelas
leader-set-full = tudo
# A data em ordem ISO nos dois idiomas: ela ordena sozinha e não confunde
# dia com mês, que é o único jeito de uma coluna de datas não mentir.
files-when = { $date } { $time }
files-when-utc = { $date } { $time } UTC
# THE SAME INSTANT, WRITTEN OUT. The column's form is `files-when`; this is
# what the pointer asks for, where there is a line and the question was which
# day and which second.
files-when-long = { $date } às { $time }
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
# The panels. `no-room` is what the toggle says when it turns one on at a width
# that cannot hold it: the state flips anyway and the screen says why, which is
# the shape `files-sidebar-only` already has. `says` is one fact in full on the
# message line — a panel is twenty-four cells and a path is two hundred
# characters, so the row that had to be cut is the row somebody wants to read.
files-panel-no-room = não há espaço para o painel nesta largura — ele precisa de { $cells } colunas
files-fact-says = { $label }: { $value }
files-preview-all-shown = a prévia inteira já está à vista
files-playlist-previous = faixa anterior
files-playlist-next = próxima faixa
files-first = o primeiro
files-last = o último
cmd-files-open = Gerenciador de arquivos
cmd-files-window = Gerenciador de arquivos, em janela
group-file = arquivo
# A BARRA LATERAL — de onde se parte.
#
# As cinco primeiras sao ids de `OF_THE_VIEW` e de `menu.rs`; o
# `files-actions-have-text` as cobra a partir do id, sozinho.
files-preview = prévia
files-facts = informações
files-columns = permissões e data
files-sidebar = barra lateral
files-theatre = expandir a prévia
files-sidebar-only = esta tecla age numa linha da barra — { $key } vai até lá
# O cabeçalho da tabela. Clicar num deles ordena por ele; clicar de novo
# inverte. A tecla `s` faz a mesma volta.
files-column-icon = categoria
files-column-name = nome
files-column-perms = permissões
files-column-owner = dono
files-column-group = grupo
files-column-size = tamanho
files-column-modified = modificado
files-explain-owner = { $who } é o dono; clicar troca
cmd-atlas-open = dados do mundo
cmd-atlas-window = dados do mundo numa janela
transfers-pause = segurar ou soltar esta transferência
cmd-transfers-pause = segurar ou soltar
# A TELA EM SI.
brain-title = Brain
brain-why-no-systemctl = Sem systemd aqui
brain-why-runaway = Parou de dar notícia e a espera acabou. Pode muito bem ainda estar rodando.
brain-why-aborted = Você interrompeu. Nada deu errado.
brain-why-postponed = Você disse agora não, e a próxima hora mudou.
brain-why-cpu = O processador estava ocupado
brain-why-battery = Estava na bateria
brain-why-busy = Outra coisa estava rodando
brain-why-network = Não havia rede
brain-why-asleep = A máquina estava dormindo
echo-brain-unreachable = O cérebro não está respondendo

## `theme-backdrop-near`, is offered only under a light theme.

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
echo-theme-unsaved = não deu para gravar o tema: { $error }

## footer, windows, borders) and `$value` what it said — the file's words.

echo-theme-refused = { $file } diz que { $field } é “{ $value }”, que não é uma opção aqui — recusado
echo-theme-unreadable = não deu para ler { $file }, então todas as escolhas nele foram recusadas: { $error }

## `echo-theme-unreadable`).

echo-theme-kept = { $file } não foi gravado, para nada nele se perder; corrija ou apague à mão: { $why }

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

network-import = trazer
ask-network-import = Caminho do arquivo .conf ou .ovpn
network-connect = conecta

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

ask-bluetooth-accept = O código que ele pede — um PIN, ou seis dígitos

## Telas — os monitores, a imagem deles e a luz (LEADER s d)

mode-keyboard = teclados
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
locale-clock-sample-24 = { $hh }:{ $mm }
locale-clock-sample-12 = { $hh }:{ $mm } { $half }
locale-clock-exact-24 = { $hh }:{ $mm }:{ $ss }
locale-clock-exact-12 = { $hh }:{ $mm }:{ $ss } { $half }
locale-clock-am = da manhã
locale-clock-pm = da tarde
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
# AS PALAVRAS DO PRÓPRIO COMANDO, desenhadas na linha de dicas: o `z` pede o
# fuso em que ESTA INTERFACE lê um carimbo, que não é o da máquina.
locale-zone = O fuso desta interface
mode-geo = regiões
ask-locale-system-zone = O fuso horário da máquina, como America/Sao_Paulo
ask-locale-zone = Fuso do relógio desta interface, como America/Sao_Paulo
ask-locale-place = Fuso do lugar a acrescentar, como Europe/Lisbon
# THE TWO LETTERS, and they had no short text at all: the hint line read
# `[locale-place]` and `[locale-where]` on screen, which is the missing-key
# marker doing duty as a word. `cmd-locale-place` has been there all along —
# it is the LONG form, and the line reads the short one.
locale-place = acrescentar lugar
# As duas janelas que não são buffers wasm e chegam à mesma modeline.
# O `daemon::mode_label` transforma `br.ronoctua.<id>` em `mode-<id>` seja o
# que for a janela; sem estas a modeline desenhava `[mode-audio]`.
mode-audio = som
mode-syncthing = syncthing
mode-pointers = ponteiros
mode-locale = idioma/região
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
about-files-columns = Mostra ou esconde as permissões, o dono, o grupo e a data de modificação em cada linha da listagem.
# THE TWO COLUMNS ON THE RIGHT, and the sidebar on the left.
about-files-preview = Mostra ou esconde a coluna de prévia — a imagem, o filme, o texto ou a forma que o arquivo sob o cursor guarda.
about-files-facts = Mostra ou esconde a coluna de informações sobre o arquivo sob o cursor: o tamanho, as datas, o tipo e o que o próprio arquivo carrega.
about-files-sidebar = Mostra ou esconde a coluna de lugares — as suas dez pastas, e os discos que estiverem conectados.
about-files-theatre = Dá à prévia a região inteira, e devolve a região. Sem nada selecionado não há prévia para expandir.
about-files-playlist-previous = Mostra a entrada anterior a esta na lista de reprodução.
about-files-playlist-next = Mostra a próxima entrada desta lista de reprodução.
about-transfers-pause = Segura esta transferência onde está, ou deixa que ela siga.
mode-camera = câmera
mode-player = player
mode-software = programas
mode-camera-settings = ajustes da câmera
mode-manual = manual

## O caminho de volta, primeiro na linha de toda vista: Esc, e um clique nele

view-back = voltar
about-view-back = Volta um nível e, no primeiro, fecha esta vista — o mesmo que apertar Esc.

## Uma pergunta na linha do cabeçalho de uma janela — o que se digita ali

line-cancel = cancelar
about-line-cancel = Deixa a pergunta sem resposta — o mesmo que apertar Esc.
line-answer = responder
about-line-answer = Responde com o que está digitado na linha — o mesmo que apertar Enter.

## O menu da janela — o nome do app na modeline, clicado (LEADER w o)

ask-software-web-new = Endereço do site
cmd-software-web-address = Mudar o endereço do web app
ask-software-web-address = Endereço que ele abre
cmd-software-web-name = Mudar o nome do web app
ask-software-web-name = Nome no lançador
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
echo-brain-not-offered = O aviso não oferece isso
echo-brain-dismissed = Dispensado
echo-brain-taken = O cérebro aceitou
echo-brain-refused = O cérebro recusou: { $why }
# THE LEADER'S SETS AND THE BARS' MODES (2026-09-30): the leader pressed again
# walks the sets, and a tap of Ctrl at the root walks the bars' three modes.
leader-set-default = padrão
# THE MINIBUFFER'S DRAWER by name, and the compositor's transient keys for it.
cmd-minibuffer-fold-open = Abrir a linha do minibuffer para ler o resto, ou descer por ela
cmd-minibuffer-fold-close = Subir pela linha aberta do minibuffer, ou fechá-la
# The modeline's name for its window (`br.ronoctua.footer-settings`).
mode-footer-settings = ajustes do rodapé

## file info editing — everything the info panel can change

# A janela do navegador na modeline (`br.ronoctua.git`, `daemon::mode_label`).
mode-git = histórico do git
# What the modeline names the app's window (`br.ronoctua.icons`).
mode-icons = ícones
# The modeline's name for its window (`br.ronoctua.calc`).
mode-calc = calculadora
calc-rates-for = Converter moedas precisa das cotações do dia do Banco Central Europeu.
mode-video-metrics = métricas de vídeo
brain-why-saver = A economia de energia estava ligada
mode-security = central de segurança
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
ask-security-vt-key = Sua chave de API do VirusTotal
security-fw-profile-home = casa: confiável
security-fw-profile-public = pública: estranhos por perto
security-fw-log-off = não registradas
security-fw-log-quiet = algumas por minuto
security-fw-log-chatty = muitas por minuto
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
security-device-secure-boot-enabled = ligado
security-device-secure-boot-disabled = desligado
security-device-secure-boot-setup-mode = em modo de configuração: as chaves podem mudar
security-device-secure-boot-unsupported = não é uma partida UEFI
security-device-tpm-absent = nenhum
security-device-tpm-present = versão { $version }
security-privacy-mac-permanent = o do próprio aparelho: as redes podem seguir você
security-privacy-mac-preserve = como o aparelho tem
security-privacy-mac-random = novo a cada vez
security-privacy-mac-stable = próprio para esta rede
security-privacy-mac-explicit = um que você escolheu
security-privacy-mac-default = a escolha do sistema
security-privacy-dns-yes = sempre
security-privacy-dns-opportunistic = quando o servidor oferece
security-privacy-dns-no = nunca
security-privacy-dns-default = a escolha do sistema
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
security-vt-state-asking = perguntando…
security-vt-state-unknown = nunca visto, não enviado
security-vt-state-sending = enviando, { $percent }%
security-vt-state-pending = enviado; esperando a análise
security-vt-verdict-clean = limpo: nenhum de { $engines } motores achou nada
security-vt-verdict-suspicious = suspeito: { $found } de { $engines } motores
security-vt-verdict-malicious = malicioso: { $found } de { $engines } motores
security-rule-word-allow = permitir
security-rule-word-deny = negar
mode-containers = containers
# O nome da janela na modeline (`br.ronoctua.belongings`).
mode-belongings = pertences
# THE ROW THAT ASKS FOR A HOST (`consent_rows.rs`), the same in every app:
# what pressing it does, what it is for, and why a press was not kept
# (`ronoctua_consent::Unkept`).
consent-ask = Perguntar a { $hosts }
consent-hosts-and = { $hosts } e { $last }
consent-note = { $what } Pressionar permite.
consent-allow = permitir
consent-unkept-no-home = Não há pasta pessoal onde guardar a permissão para perguntar a { $hosts }.
consent-unkept-unparseable = O arquivo das permissões (consent.json) não se lê; ficou como estava, e { $hosts } não foi permitido.
consent-unkept-unwritable = A permissão para perguntar a { $hosts } não pôde ser gravada.
# What the modeline names the search's windows (`br.ronoctua.fast-access`,
# and the one over everything).
mode-fast-access = busca
mode-fast-access-float = busca
mode-clipboard = clipboard
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
# Uma pergunta que oferece respostas para o que se digita.
line-offers = { $keys }: { $offers }
network-refusal-no-phone = o NetworkManager não tem um dispositivo Bluetooth para este celular: ele não está pareado, ou não compartilha a conexão
network-refusal-no-vpn = não existe mais uma VPN com esse nome
network-refusal-not-vpn = esse perfil não é de uma VPN
# THE 2026-10-11 WAVE, the sudo door's root side. Why a root gesture
# did not happen (`ronoctua_security::Why`), beside the others.
security-refusal-cancelled = Cancelado: nada foi feito.
security-refusal-no-key = Nenhuma chave de segurança está conectada.
security-sudo-no-limit = sem prazo
mode-stick = pendrive do instalador

## perguntado, acima da modeline — quem pede, para quê, e o que o sudo espera.

security-refusal-no-footer = O rodapé, onde o administrador é perguntado, não está rodando: nada foi feito.
mode-incus = Incus
# The modeline's name for its window (`br.ronoctua.agents`).
mode-agents = contas de LLM
