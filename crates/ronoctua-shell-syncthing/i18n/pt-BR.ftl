# The texts `ronoctua-shell-syncthing` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

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
syncthing-title = Syncthing
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

## file picker — the one picker of a file, a folder or a name to save under

syncthing-pick-folder = Uma pasta desta máquina
