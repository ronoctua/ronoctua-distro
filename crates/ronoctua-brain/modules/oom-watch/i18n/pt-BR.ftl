# The oom-watch module's own texts: its name and the line under it, its page,
# the record it writes in the activity, the notification of a kill, and the
# hint of the RAM's mark ($count is the share of the RAM in use, in per cent;
# $stall, how much of the time the machine waits on memory).
brain-task-oom-watch = Falta de memória
brain-task-oom-watch-does = Quando a memória da máquina acaba, o kernel ou o systemd-oomd fecha um programa para o resto continuar. A cada vez, isto anota na atividade e avisa você uma vez: o que foi fechado, onde, quanta memória ele usava, quanto a máquina estava travando e quando. Enquanto a RAM está quase toda em uso, ou a máquina fica esperando pela memória, uma marca na modeline mostra quanto da RAM está em uso.
brain-task-oom-watch-brief = Registra cada programa fechado porque a memória acabou.
oom-watch-mark-ram = { $count }% da RAM em uso. Clique para ver o que está usando.
oom-watch-told-stall = A máquina está esperando pela memória { $stall }% do tempo.
oom-watch-heading-journal = Acompanhando o journal
oom-watch-heading-recent = Fechados há pouco
oom-watch-row-none = Nada foi fechado por falta de memória desde que isto começou a vigiar
oom-watch-nowhere = um grupo que o journal não nomeou
oom-watch-by-kernel = O kernel
oom-watch-by-machine = Memória esgotada
oom-watch-by-limit = Limite do grupo
oom-watch-by-oomd = systemd-oomd
oom-watch-by-pressure = oomd: travamento
oom-watch-by-full = oomd: memória cheia
oom-watch-unmeasured = não medida
oom-watch-noted-killed = { $what } foi fechado por falta de memória
notif-oom-watch-killed-summary = { $what } foi fechado por falta de memória
notif-oom-watch-killed-body = { $by ->
        [machine] O kernel o matou: a memória da máquina inteira tinha acabado.
        [limit] O kernel o matou: ele tinha chegado ao limite de memória do seu grupo.
        [kernel] O kernel o matou por falta de memória.
        [pressure] O systemd-oomd o fechou: a máquina estava travada esperando memória { $stall }% do tempo, acima do limite de { $limit }%.
        [full] O systemd-oomd o fechou: a memória e a área de troca estavam quase cheias.
       *[oomd] O systemd-oomd o fechou por falta de memória.
    } { $pid ->
        [0] Onde: { $where }.
       *[other] Processo { $pid }, em { $where }.
    } Memória: { $memory }. { $measured ->
        [yes] A máquina estava travada { $stall }% do tempo. Em { $at }.
       *[no] Em { $at }.
    }
