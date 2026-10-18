# The texts `ronoctua-shell-door` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Notifications

notif-action-open-video = Abrir a gravação
notif-memory-killed-body = Um aplicativo foi fechado para a máquina não travar.
# O que cada notificação deixa fazer.
notif-action-edit = Editar a imagem
notif-action-open = Abrir a imagem
notif-action-folder = Abrir a pasta
notif-action-copy-image = Copiar a imagem
notif-action-copy-path = Copiar o caminho
notif-action-memory-panel = Abrir o painel de memória
notif-shared-written = Está no repositório. Quem publica é o seu commit.
notif-share-failed = Não deu para escrever no repositório.
notif-share-nowhere = Esta máquina não tem repositório para onde compartilhar.
# OS DOIS INTERRUPTORES QUE NÃO FAZEM NADA, e por quê. Um interruptor que não
# interrompe e não diz nada é a armadilha que este projeto já pagou uma vez.
notif-inert-vibrate = sem dispositivo háptico nesta máquina
notif-inert-audio = ainda não implementado — veja docs/PENDING.md

## Installer — the interface

echo-no-opened = Não consegui abrir { $program }

## Comandos do compositor — §13, "toda ação do SO tem um comando"

clipboard-sync-down = não alcança o servidor
ask-clipboard-cap = A maior cópia que sai daqui, em bytes (0 para o limite do servidor)
ask-clipboard-rounds = Com quantas rodadas a chave é derivada
ask-clipboard-salt = O sal, o mesmo em toda máquina desse servidor

## Network — Wi-Fi and Ethernet (LEADER s n)

ask-network-connect = Senha desta rede
network-open = aberta

## O painel de memoria — LEADER s m

cmd-memory-refresh = Atualizar agora
cmd-memory-scan = Varrer
# As teclas da VISTA. O id É a chave, como no gerenciador de arquivos:
# `cmd-` é do registro de comandos, e estas não estão nele.
transfers-down = descer
transfers-up = subir
transfers-cancel = desistir desta transferência
transfers-forget = tirar esta linha da lista
transfers-sweep = tirar da lista tudo o que já acabou
transfers-launch = abrir o que foi instalado
transfers-detail = a transferência, campo a campo
transfers-offline = a fila de transferências não está no ar
cmd-transfers-down = a próxima transferência
cmd-transfers-up = a transferência anterior
cmd-transfers-cancel = desistir
cmd-transfers-forget = tirar da lista
cmd-transfers-sweep = limpar o que acabou
cmd-transfers-launch = abrir o que ela instalou
cmd-transfers-detail = detalhe desta linha
transfers-work-region = instalar as ruas de uma região
transfers-work-software = alterar um programa
brain-log = Registro ao vivo
# E POR QUE UMA EXECUÇÃO NÃO ACONTECEU. `interrompida` NÃO é um erro — é a
# pessoa tendo decidido, e a tela concordando com ela.
brain-why-off = Desligada
# E O QUE CUSTA CADA UMA DAS DUAS RESPOSTAS. `agora não` não é "nunca": a
# corrida sai da fila e a próxima hora muda, que é a única coisa que de fato
# para uma contagem — tirar da fila apenas a devolveria na batida seguinte, e a
# pessoa responderia à mesma pergunta a cada meio minuto.
echo-brain-postponed = Agora não — ela volta na próxima hora dela
echo-brain-proceeding = Começando agora
echo-brain-idle = Não há nada para acontecer
brain-abort = parar
brain-proceed = começar agora

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

# O que as portas dizem quando um daemon recusou, e o que a janela não faz.
door-no-footer = Não foi possível chegar ao rodapé para rodar isto
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
echo-view-busy = outra vista está com o rodapé: Esc fecha ela primeiro
network-tor-open = ajustes

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

keyboard-mode = Teclados
# THE TRANSFER QUEUE.
about-transfers-up = Move o cursor uma linha para cima na fila.
about-transfers-down = Move o cursor uma linha para baixo na fila.
about-transfers-cancel = Pergunta se é para desistir desta transferência: o que já foi escrito seria apagado.
about-transfers-detail = Abre esta transferência, um campo por linha: quem a move, quem pediu, o problema inteiro. Apertado lá, volta à lista.
about-transfers-forget = Tira esta linha da lista. O arquivo que ela escreveu fica onde está.
about-transfers-sweep = Tira da lista, de uma vez, todas as linhas que já acabaram.
about-transfers-launch = Abre o programa que esta linha instalou, pela entrada que ela pôs no lançador.
pointers-on = ligado
pointers-off = desligado
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
ask-player-enqueue = Pôr na fila (um caminho ou um endereço)

## Uma pergunta na linha do cabeçalho de uma janela — o que se digita ali

door-paste-failed = Não foi possível ler o que foi copiado.

## O menu da janela — o nome do app na modeline, clicado (LEADER w o)

notif-action-trail-panel = Abrir as trilhas
ask-software-icon-path = Caminho da imagem
ask-software-file-path = Caminho do arquivo a instalar
software-not-in-launcher = Ainda não está no lançador; o lançador está lendo as pastas de novo.
notif-action-transfers-panel = Abrir a fila
notif-action-software-maintenance = Ver as atualizações
notif-action-software-trials = Ver as experiências
notif-action-software-android = Abrir o Android
echo-brain-no-action = Nenhum módulo do cérebro oferece isso
echo-brain-snoozed = Adiado
calc-shape-arithmetic = Aritmética
calc-shape-conversion = Conversão
calc-shape-percentage = Porcentagem
calc-shape-base = Base numérica
calc-shape-roman = Números romanos
calc-shape-date = Data
calc-shape-zone = Fuso horário
calc-shape-currency = Moeda
# A PREFERENCE THAT COULD NOT BE SAVED is said, never only logged.
echo-preference-not-written = Não foi possível gravar { $file } ({ $why }): o que você mudou vale só até o rodapé reiniciar.

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-any = Todo arquivo
file-picker-filter-images = Imagens
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

## file info editing — everything the info panel can change

transfers-work-scan = enviar um arquivo ao VirusTotal
# A ação ao lado do "uma rede nova é tratada como pública" do vigia.
notif-action-treat-as-home = Tratar como de casa
transfers-work-belongings = pertences
about-consent-allow = Deixa perguntar aos sites que a linha nomeia — o mesmo que pressionar a linha. Fica guardado no consent.json, para todo app.
clipboard-act-stale = O histórico mudou antes de a tecla chegar, e nada foi feito: confira e aperte de novo.
about-line-offer = Toma a primeira resposta oferecida para o que foi digitado.
belongings-repair-part-not-read = Parte do { $file } não se lê aqui: ela continua no arquivo como está escrita, e esta máquina segue sem ela.
transfers-work-incus-launch = criar uma instância
transfers-work-incus-image-copy = copiar uma imagem
transfers-work-incus-export = exportar uma instância
transfers-work-incus-import = importar uma instância
transfers-work-incus-publish = publicar uma instância como imagem
transfers-work-incus-push = enviar um arquivo para uma instância
transfers-work-incus-pull = tirar um arquivo de uma instância
transfers-work-stick-fetch = baixar o instalador

## perguntado, acima da modeline — quem pede, para quê, e o que o sudo espera.

sudo-bar-asks = { $who } pede para { $deed }
sudo-bar-who-security = A central de segurança
sudo-bar-who-security-window = A janela da central de segurança
sudo-bar-who-files = O gerenciador de arquivos
sudo-bar-who-files-window = A janela do gerenciador de arquivos
sudo-bar-who-stick = O pendrive do instalador
sudo-bar-who-stick-window = A janela do pendrive do instalador
sudo-bar-who-belongings = Os pertences
sudo-bar-who-belongings-window = A janela dos pertences
sudo-bar-who-update = A atualização
sudo-bar-who-update-window = A janela da atualização
sudo-bar-deed-relax-minutes = abrir a janela sem senha por { $minutes ->
        [one] um minuto
       *[other] { $minutes } minutos
    }
sudo-bar-deed-relax-hours = abrir a janela sem senha por { $hours ->
        [one] uma hora
       *[other] { $hours } horas
    }
sudo-bar-deed-relax-no-limit = abrir a janela sem senha até você encerrá-la
sudo-bar-deed-relax-end = encerrar a janela sem senha
sudo-bar-deed-firewall = mudar o firewall
sudo-bar-deed-finger-enrol = cadastrar uma digital ({ $finger })
sudo-bar-deed-finger-delete = remover uma digital ({ $finger })
sudo-bar-deed-finger-delete-all = remover todas as digitais
sudo-bar-deed-finger-sudo-on = deixar seus dedos responderem ao administrador
sudo-bar-deed-finger-sudo-off = parar de deixar seus dedos responderem ao administrador
sudo-bar-deed-files-root = mudar o que só o administrador pode mudar
sudo-bar-deed-private = usar o cofre /private
sudo-bar-deed-private-create = criar o cofre /private ({ $size })
sudo-bar-deed-private-open = abrir o cofre /private
sudo-bar-deed-private-lock = trancar o cofre /private
sudo-bar-deed-private-enrol = deixar uma chave de segurança abrir o cofre /private
sudo-bar-deed-private-forget = esquecer a chave que abre o cofre /private
sudo-bar-deed-private-passphrase = trocar a senha do cofre /private
sudo-bar-deed-private-grow = aumentar o cofre /private ({ $size })
sudo-bar-deed-stick = gravar o instalador num pendrive
sudo-bar-deed-approve = aprovar a parte root do seu módulo do brain { $id }
sudo-bar-deed-keys = deixar { $count ->
        [one] uma chave de segurança responder
       *[other] { $count } chaves de segurança responderem
    } ao administrador
sudo-bar-deed-keys-off = parar de deixar suas chaves de segurança responderem ao administrador
sudo-bar-deed-signer-pin = fixar a sua chave de assinatura nesta máquina
sudo-bar-deed-cache-netrc = deixar esta máquina ler o seu cache binário privado { $name }
sudo-bar-deed-cache-netrc-remove = tirar desta máquina o acesso ao seu cache binário privado
