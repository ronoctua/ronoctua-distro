# The clock-sync module's own texts: its name and the line under it, the
# three things it may say on the strip, its one action, and what a refused
# try says — to the press, and in the activity log.
brain-task-clock-sync = Vigiar o relógio
brain-task-clock-sync-does = Algumas vezes por dia, confere se alguma coisa está mantendo certo o relógio desta máquina pela internet, e liga de novo se nada estiver. Ele não acerta a hora sozinho: o cliente de horário do próprio sistema já faz isso o tempo todo, e um segundo só discordaria dele. Desligue isto se quiser acertar o relógio à mão.
brain-task-clock-sync-brief = Garante que algo mantém o relógio da máquina certo pela internet.
clock-sync-says-not-automatic = Nada está mantendo certo o relógio desta máquina. Ele vai se atrasar, e todo carimbo de hora daqui se atrasa junto.
clock-sync-says-not-synchronised = O relógio está sendo cuidado, mas ainda não conseguiu falar com nenhum servidor de horário.
cmd-clock-sync-keep = Manter certo
clock-sync-says-refused = Nada está mantendo certo o relógio desta máquina, e ligar a manutenção foi recusado; veja: journalctl -u systemd-timedated
clock-sync-why-refused = Não deu para entregar o relógio à rede; veja: journalctl -u systemd-timedated
clock-sync-noted-refused = Ligar a manutenção do relógio foi recusado: { $said }
