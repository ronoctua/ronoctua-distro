# The texts `ronoctua-shell-buffers` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## The modeline's two left segments

# THE BUFFER'S NAME (the window title) and its MAJOR MODE (the kind of buffer).
# Both are addressed by an id that is ENGLISH and kebab-case (invariant 18); what
# reaches the screen is the value here. That separation is why `glifos` used to
# leak onto an English-language screen: the id WAS the label.
buffer-example = exemplo
buffer-memory = memória
buffer-transfers = transferências
buffer-format = formatar
buffer-brain = cérebro

## O painel de memoria — LEADER s m

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
cmd-memory-full = Visão completa
cmd-memory-guard = Proteção contra travamento
cmd-memory-tab = Alternar RAM e disco
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
memory-vram = Vídeo
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
transfers-answer = responder
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
transfers-window-title = transferências
transfers-window-empty = nada na fila
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
transfers-engine-software = programas
cmd-brain-run = Rodar agora
cmd-brain-rehearse = Ensaiar
cmd-brain-reset-steps = Voltar aos passos
cmd-brain-interval = De quanto em quanto
cmd-brain-window = Mudar o horário
cmd-brain-switch = Desligar
cmd-brain-keys = Todas as teclas
cmd-brain-log = Registro ao vivo
cmd-brain-row-also = A outra ação da linha
brain-nothing = Nada configurado ainda
brain-tasks = Tarefas
brain-activity = Atividade
brain-no-activity = Nada aconteceu ainda
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
brain-why-ram = A memória estava ocupada
# QUAL PARTE DO DIA uma tarefa pode começar. As horas são os mesmos dígitos em
# todo idioma; as palavras em volta não.
brain-part-any = qualquer hora
brain-part-night = 02:00 e 05:00
brain-part-morning = 08:00 e 12:00
brain-part-afternoon = 13:00 e 18:00
brain-part-evening = 19:00 e 23:00

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

# OS TECLADOS — o que uma tecla pode virar, e o que deu errado.
keyboard-trouble-file = o arquivo de regras de teclado não pôde ser lido ou escrito
keyboard-trouble-no-place = esta máquina não tem onde guardar regras de teclado — elas são escritas no sistema instalado
keyboard-trouble-refused = o keyd não aceitou estas regras, então nada mudou
keyboard-trouble-restart = as regras foram escritas, mas o keyd não reiniciou
keyboard-trouble-mode = os teclados não puderam trocar de modo, e ficam como estavam
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

## Telas — os monitores, a imagem deles e a luz (LEADER s d)

# OS TECLADOS (`LEADER s k`) — quais esta máquina tem, e o que cada tecla de
# cada um faz. Remapear é por teclado porque o keyd distingue dispositivos; o
# layout e a repetição são do compositor, um só para todos eles.
buffer-keyboard = Teclados
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
about-transfers-answer = Fica com a resposta em que o cursor está.
about-brain-act = Faz o que o aviso da frente oferece: a primeira ação do módulo que o levantou.
about-brain-dismiss = Dispensa o aviso da frente. Ele só volta se o módulo dele o levantar de novo.
about-brain-snooze = Adia o aviso da frente, pelo tempo que o primeiro adiamento dele diz.
brain-outcome-noted = Anotado
brain-notices = Avisos
brain-actions = O que ele oferece
brain-watching = Vigiando
brain-activity-last = Último:
brain-activity-first = Primeiro
brain-activity-times = Vezes
brain-since = Desde

## file info editing — everything the info panel can change

transfers-work-archive-add = pôr no arquivo compactado
transfers-work-archive-remove = tirar do arquivo compactado
transfers-work-archive-test = testar o arquivo compactado
transfers-work-archive-convert = converter o arquivo compactado
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
transfers-engine-scan = VirusTotal
# The queue's words for the containers' jobs (`ronoctua-transfers`).
transfers-engine-container = containers
# As palavras da fila para uma sincronização.
transfers-engine-belongings = pertences
example-code = Um trecho de código
transfers-engine-incus = Incus
transfers-work-incus = Incus
transfers-engine-stick = pendrive do instalador
transfers-work-stick-write = gravar o instalador num pendrive

## AS CHAVES DA CENTRAL DE SEGURANÇA, e a contagem da janela sem senha.

transfers-engine-seal = criptografia
transfers-work-seal = cifrar
transfers-work-unseal = decifrar
