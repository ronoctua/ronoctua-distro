# The texts `ronoctua-shell-terminal` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# THE TERMINAL'S WINDOW (`crate::terminal`): its three identities' titles,
# until the program names the window itself.
terminal-title = Terminal
terminal-title-monitor = Monitor
terminal-title-rescue = Console de resgate

# WHERE THE TERMINAL LIVES, said on the window's line when it is not where
# a terminal lives: with no scope of its own it runs in the window's process,
# and dies with the window.
terminal-in-process = Este terminal vive na janela: a sessão não lhe deu um escopo próprio, então fechar a janela encerra o programa
# A terminal left running by another build of the system (a switch since):
# its screen comes in a form this window cannot decode yet.
terminal-other-build = Este terminal foi aberto por outra build do sistema, e esta janela não consegue ler a tela dele

# A PASTE THAT COULD RUN A COMMAND (a line break the program would take for
# Enter): asked, the no first.
terminal-paste-unsafe = Este texto colado poderia rodar um comando ao chegar
terminal-paste-refuse = Não colar
terminal-paste-allow = Colar

# A PASTE THE TERMINAL DID NOT WRITE, said on the window's line: the window
# judged it by its copy of the program's modes, the terminal by its own,
# which the program may have changed a moment before — or the terminal
# could not take it at all.
terminal-paste-refused-unsafe = O terminal não aceitou esta colagem: o programa mudou a forma como a lê, e ela poderia rodar um comando ao chegar
terminal-paste-refused = O terminal não conseguiu receber esta colagem

# A PROGRAM ASKING TO READ THE CLIPBOARD (OSC 52): asked, the no first.
terminal-clipboard-read = { $program } pede para ler a área de transferência
terminal-clipboard-read-unnamed = O programa pede para ler a área de transferência
terminal-clipboard-refuse = Recusar
terminal-clipboard-allow = Deixar ler

# WHAT A COPY SAID (`C-S-c`).
terminal-copied = { $count ->
    [one] Um caractere copiado
   *[other] { $count } caracteres copiados
  }
terminal-nothing-selected = Nada está selecionado para copiar

# NO TERMINAL COULD START, and the window has nothing else to show: said for
# as long as a refusal is, and then the window closes.
terminal-could-not-start = O terminal não pôde ser aberto; esta janela vai fechar
# A NEW TERMINAL of the group (`C-S-t`) could not start: the others stay.
terminal-new-failed = Um terminal novo não pôde ser aberto
# `C-S-RET` ON A TERMINAL OF THE WINDOW'S OWN PROCESS: another window cannot
# show what lives and dies in this one.
terminal-detach-here = Este terminal vive nesta janela e não pode ter uma janela própria

# A LINK UNDER `Ctrl` (OSC 8, or an address written as text), opened with a
# click — or not: a scheme this system does not open, or no opener at all.
terminal-link-refused = Este link não é dos que este sistema abre
terminal-link-no-opener = Nada aqui abre links

# THE SEARCH (`C-S-f`), over the last row: what it is, what was found, and
# its three clicks — the next match toward older output, toward newer, and
# the end. `$at` and `$total` are numbers.
terminal-search = Buscar
terminal-search-count = { $at } de { $total }
terminal-search-none = Nenhum resultado
terminal-search-older = Anterior
terminal-search-newer = Seguinte
terminal-search-close = Fechar

# THE GROUP'S COMMANDS (`LEADER :` and `LEADER Alt`), the window's own keys.
cmd-terminal-new = Um terminal novo, ao lado do que está na frente
cmd-terminal-next = O próximo terminal desta janela
cmd-terminal-previous = O terminal anterior desta janela
cmd-terminal-detach = O terminal da frente, numa janela própria

# THE TERMINALS NO WINDOW SHOWS (`crate::terminals`, `LEADER RET b`): the
# group of the leader, the view's title, its commands and the words its hint
# teaches them by.
group-terminals = em segundo plano
terminals-title = Terminais sem janela
cmd-terminals-open = Os terminais sem janela, no rodapé
cmd-terminals-window = Os terminais sem janela, numa janela
cmd-terminals-up = O terminal anterior
cmd-terminals-down = O próximo terminal
cmd-terminals-enter = Mostrar este terminal, numa janela com o resto do grupo
cmd-terminals-end = Encerrar este terminal — com pergunta antes
terminals-up = sobe
terminals-down = desce
terminals-enter = mostrar
terminals-end = encerrar
# Its headings: the terminals that were alone in their windows, and each
# window's group — numbered, so two windows of the same size read apart.
# `$number` and `$count` are numbers.
terminals-alone = Sozinhos nas suas janelas
terminals-group = Janela { $number }, com { $count } terminais
# Nothing in the background.
terminals-none = Todo terminal tem uma janela: nenhum está em segundo plano.
# How long a terminal has been without a window. Numbers.
terminals-since-moments = agora há pouco
terminals-since-minutes = { $minutes } min
terminals-since-hours = { $hours } h
terminals-since-days = { $days } d
# What the program in it says it is doing (OSC 7501), after its glyph.
terminals-working = trabalhando
terminals-blocked = esperando você
terminals-done = concluído
terminals-idle = pronto
terminals-failed = falhou
# What it asked for while nobody was looking, when its program says nothing.
terminals-rang = tocou o sino
terminals-notified = mandou uma notificação
# Its program ended with no window to see it: kept until it is shown once.
# `$code` is a number.
terminals-ended = terminou
terminals-ended-code = terminou com { $code }
# ENDING ONE (`d`), asked first, the no first: one whose program runs, and
# one whose program ended already. `$name` is what its row calls it.
terminals-end-question = Encerrar { $name }? O programa é desligado, e o que ele escreveu vai junto.
terminals-close-question = Fechar { $name }? O que ele escreveu vai junto.
terminals-answer-no = Não, manter
terminals-answer-yes = Sim, encerrar
# What ending one is said as, and why it could not be.
terminals-ending = Encerrando { $name }
terminals-not-ended = Não deu para encerrar { $name }: nenhuma thread pôde ser iniciada para avisá-lo
terminals-no-registry = Esta sessão não guarda terminais: ela não tem pasta de execução

# THE TERMINAL'S SETTINGS (`crate::terminal_settings`, `LEADER RET s`), in
# `terminal.json`: the group of the leader, the view's title and commands,
# the words its hint teaches them by, its two sections, its rows and every
# value of each ring.
group-terminal-settings = configurações
terminal-settings-title = Configurações do terminal
cmd-terminal-settings-open = As configurações do terminal, no rodapé
cmd-terminal-settings-window = As configurações do terminal, numa janela
cmd-terminal-settings-up = A configuração anterior
cmd-terminal-settings-down = A próxima configuração
cmd-terminal-settings-less = O valor anterior desta configuração
cmd-terminal-settings-more = O próximo valor desta configuração
cmd-terminal-settings-enter = Passar esta configuração para o próximo valor
terminal-settings-up = sobe
terminal-settings-down = desce
terminal-settings-less = menos
terminal-settings-more = mais
terminal-settings-enter = mudar
terminal-settings-new = Para todo terminal novo
terminal-settings-windows = Em toda janela de terminal
terminal-settings-new-note = Um terminal novo pega as três primeiras ao começar; um que já roda fica com as que tinha, e um programa nele pode pedir outro cursor.
terminal-settings-windows-note = Toda janela de terminal pega as duas últimas na hora, e o zoom de uma janela é contado a partir deste tamanho.
terminal-settings-scrollback = Histórico que cada um guarda
terminal-settings-cursor = O cursor
terminal-settings-blinks = O cursor pisca
terminal-settings-copy = O que se seleciona é copiado
terminal-settings-size = Tamanho do texto
# `$megabytes` and `$percent` are numbers.
terminal-settings-megabytes = { $megabytes } MB
terminal-settings-cursor-block = um bloco
terminal-settings-cursor-bar = uma barra
terminal-settings-cursor-underline = um sublinhado
terminal-settings-yes = sim
terminal-settings-no = não
terminal-settings-copy-primary = para o que o clique do meio cola
terminal-settings-copy-clipboard = para a área de transferência
terminal-settings-copy-both = para os dois
terminal-settings-copy-off = para lugar nenhum
terminal-settings-size-system = o do sistema ({ $percent } %)
terminal-settings-size-own = { $percent } %
# Nothing of the file read: said under the values, which are the defaults.
terminal-settings-unread = { $file } não se lê como foi escrito: o que aparece são os padrões, e nada é gravado por cima dele até que se leia.
# A change that did not land, said where it was made.
terminal-settings-unparsed = { $file } não se lê como foi escrito; nada foi mudado por cima dele
terminal-settings-not-written = Não deu para gravar { $file }: { $why }
