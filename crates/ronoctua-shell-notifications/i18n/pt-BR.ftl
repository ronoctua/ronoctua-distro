# The texts `ronoctua-shell-notifications` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Notifications

notif-title = Notificações
# OS FILTROS QUE A DISTRO JÁ TRAZ. O gerenciador os lista por estes nomes.
notif-rule-important-gets-room = O que é importante ganha espaço
notif-rule-bluetooth-connected-is-quiet = Aparelho Bluetooth que conecta é dito pela marca
notif-rule-bluetooth-disconnected-is-quiet = Aparelho Bluetooth que sai é dito pela marca
notif-rule-small-things-are-quick = Coisa pequena passa rápido
# O gerenciador: três listas, e um título para cada.
notif-column-happened = Recebidas
notif-column-suppressed = Filtradas
notif-column-catalogue = Notificações e filtros
notif-forget = esquecer
notif-read-all = todas lidas
notif-filters = filtros
notif-any-device = Um dispositivo
# A FICHA DO FILTRO — escrever um seu. A linguagem de regras está completa desde
# o dia em que foi gerada; até agora o único caminho até ela era editar o JSON à
# mão, o que não é um caminho.
notif-draft-name = como chamar
notif-draft-field = olhar para
notif-draft-test = comparar
notif-draft-value = com
notif-draft-unnamed = ainda sem nome
notif-draft-nothing = ainda nada
notif-draft-unready = falta um nome e algo com que comparar
notif-draft-type = digitar esta
notif-draft-add = mais uma condição
notif-draft-drop = tirar esta condição
notif-draft-save = salvar o filtro
notif-new = escrever um filtro
notif-edit = editar este filtro seu
notif-drop = apagar este filtro seu
# O que uma condição olha, e como compara. As palavras são da pessoa, não da
# tabela: `summary` é como o protocolo chama, e `o título` é o que é.
notif-field-app = quem mandou
notif-field-summary = o título
notif-field-body = o corpo
notif-field-tag = uma etiqueta
notif-field-catalogue = o tipo
notif-field-urgency = quão urgente
notif-field-level = o nível
notif-test-is = é exatamente
notif-test-contains = contém
notif-test-starts-with = começa com
notif-test-at-least = é pelo menos
notif-test-at-most = é no máximo
notif-adjust = ajustar este tipo
notif-where = { $at } de { $total }
notif-up = sobe
notif-down = desce
notif-pane-next = próxima coluna
notif-pane-previous = coluna anterior
notif-knob-next = próximo valor
notif-knob-previous = valor anterior
notif-knob-turn = girar
notif-restore = o padrão da distro
notif-default = padrão da distro
notif-mine = seu filtro
notif-shared = a caminho de todo mundo
# ANTES DE UM FILTRO IR PARA TODO MUNDO. O repositório é público, e o que cai
# nele está publicado: o git guarda e um espelho copia.
notif-share = compartilhar com todo mundo
notif-share-answer = responder
notif-share-does = e ele faz
notif-share-public = Isto vai para o repositório, que é PÚBLICO. Tudo o que está acima é publicado junto, e não há como voltar atrás na prática. Nada mais seu vai — nem o histórico, nem os ajustes, nem o nome desta máquina. Quem publica de verdade é o seu commit.
notif-share-not-a-name = Nome no repositório é inglês, minúsculo, com hífen no lugar do espaço (invariante 18) — é a chave sob a qual um ajuste é guardado. Renomeie o filtro e volte.
notif-share-no = não, fica só comigo
notif-share-yes = sim, compartilhar
notif-yours = seu ajuste
notif-nothing-here = nada aqui
notif-yes = sim
notif-no = não
notif-knob-show = aparece
notif-knob-life = fica
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
notif-auto-read = chega já lida
# O QUE O GERENCIADOR SABE SOBRE UM REGISTRO, na coluna ao lado dele. É o que a
# linha não tem espaço para dizer: de onde veio, quando, quanto tempo ficou e — o
# que o dono pediu pelo nome — QUAL FILTRO DECIDIU, que antes vinha colado no fim
# da frase como um id cru.
notif-fact-origin = quem mandou
notif-fact-arrived = quando chegou
notif-about-record = { $when } — { $line }
notif-fact-life = quanto tempo ficou
notif-fact-show = como apareceu
notif-fact-read = já foi lida
notif-fact-filter = filtro que decidiu
notif-fact-no-filter = nenhum casou
notif-see = abrir
notif-glyph-read = Marcar como lida
notif-glyph-unread = Marcar como não lida de novo
notif-glyph-delete = Esquecer de vez

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-notif-open = Notificações
cmd-notif-window = Notificações em uma janela
cmd-notif-read-all = Marcar todas como lidas
cmd-notif-read = Marcar esta notificação como lida, ou como não lida
cmd-notif-forget = Esquecer esta notificação
cmd-notif-act = Rodar a ação da notificação
cmd-notif-see = Abrir a notificação e ler tudo
cmd-notif-filters = Filtros de notificação
cmd-notif-new = Escrever um filtro de notificação seu
cmd-notif-drop = Apagar este filtro seu
cmd-notif-share = Compartilhar este filtro seu com todo mundo
cmd-notif-share-answer = Responder a pergunta de compartilhar
cmd-notif-draft-type = Digitar o que esta linha do filtro guarda
cmd-notif-draft-add = Acrescentar mais uma condição ao filtro
cmd-notif-draft-drop = Tirar esta condição do filtro
cmd-notif-draft-save = Salvar o filtro
ask-notif-filter-name = O nome do filtro
ask-notif-filter-value = O texto que a condição procura
cmd-notif-adjust = Ajustar este tipo de notificação
cmd-notif-up = Uma notificação acima
cmd-notif-down = Uma notificação abaixo
cmd-notif-pane-next = Próxima coluna do gerenciador
cmd-notif-pane-previous = Coluna anterior do gerenciador
cmd-notif-knob-next = Próximo valor do ajuste
cmd-notif-knob-previous = Valor anterior do ajuste
cmd-notif-knob-turn = Gira para o próximo valor; depois do último vem o primeiro
cmd-notif-restore = De volta ao padrão da distro
cmd-notif-dismiss = Dispensar a notificação
group-notification = notificação
# THE NOTIFICATION MANAGER, where one row means something else.
about-notif-edit = Abre este filtro seu para ser reescrito: o nome, as condições e o que ele faz.
