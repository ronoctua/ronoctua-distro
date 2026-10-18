# The texts `ronoctua-shell-power-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

power-time-empty = { $time } restantes
power-time-full = { $time } até encher
power-time-unknown = tempo desconhecido
power-duration = { $h } h { $m } min
power-duration-minutes = { $m } min
power-state-unknown = desconhecido
power-state-charging = carregando
power-state-discharging = descarregando
power-state-empty = vazia
power-state-full = cheia
power-state-pending-charge = esperando para carregar
power-state-pending-discharge = esperando para descarregar
power-profile-power-saver = economia
power-keep-awake = manter acordado: sem sono, sem ação por ociosidade
power-keep-lid = ignorar a tampa: fechá-la não faz nada
# THE DAEMONS' WORDS — logind's kebab-case and UPower's CamelCase land on
# the same key (`power_words::daemon_word`); a word outside this list is shown raw.
power-duration-hours = { $h } h
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
power-what-sleep = o sono
power-what-idle = a ociosidade
power-what-shutdown = o desligamento
power-what-handle-power-key = a tecla de energia
power-what-handle-suspend-key = a tecla de suspender
power-what-handle-hibernate-key = a tecla de hibernar
power-what-handle-lid-switch = a tampa
power-what-handle-reboot-key = a tecla de reiniciar
power-word-sleep = dormir
power-what-profile = o perfil
power-what-lock = a trava
power-what-thresholds = os limites
power-what-action = a ação
power-refused-authority = não permitido para esta sessão
power-refused-absent = o daemon não está lá, ou o hardware não consegue
power-refused-invalid = o valor foi recusado
power-refused-other = recusado
echo-power-refused = { $what } { $why }

## file info editing — everything the info panel can change

# As recusas de energia e de Bluetooth que não tinham nome, com nome: as
# palavras do daemon vão para o journal, nunca para a linha.
power-refused-timed-out = não teve resposta a tempo
power-refused-failed = não pôde ser feito
power-refused-no-bus = não alcançou o barramento do sistema
