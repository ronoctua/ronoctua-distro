# The texts `ronoctua-shell-locale` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

locale-heading-language = Interface
locale-speaking = em uso
locale-from-file = Você escolheu este idioma; ele fica no language.json.
locale-from-system = Este é o idioma da máquina, vindo da configuração do sistema.
locale-from-default = Ninguém escolheu; este é o primeiro idioma declarado.
locale-heading-time = Relógio e data
locale-clock-24h = Relógio de 24 horas
locale-clock-12h = Relógio de 12 horas
locale-order-ymd = Ano primeiro
locale-order-dmy = Dia primeiro
locale-order-mdy = Mês primeiro
locale-heading-machine = Decidido pelo sistema
locale-zone-unknown = desconhecido
locale-zone-is-declared = O fuso é declarado na configuração do sistema; mudá-lo é um rebuild.
locale-decimal = Separador decimal
locale-decimal-is-the-languages = O separador é do idioma, e acompanha ele.
locale-measurement = Medidas
locale-metric = Métrico
locale-not-written = A escolha não pôde ser gravada no disco.
locale-measurement-is-declared = Métrico seja qual for o idioma, e isso é decisão, não padrão.
cmd-locale-open = Idioma e região, no rodapé
cmd-locale-window = Idioma e região, em janela
cmd-locale-up = Linha anterior
cmd-locale-down = Próxima linha
cmd-locale-enter = Escolher
group-locale = idioma/região
# ---------------------------------------------------------------------------
# LEADER s g — as regiões cujas ruas esta máquina tem.
#
# Os nomes são as regiões do mundo como a Geofabrik as publica, e isso é DADO,
# nunca chave: `Île-de-France` não se traduz, e procurar `[geo-region-…]` seria
# procurar a tradução de um nome que alguém escreveu. Chave é a frase em volta.
# ---------------------------------------------------------------------------
geo-title = Regiões
geo-up = cima
geo-down = baixo
geo-enter = instalar
geo-find = filtrar
geo-remove = remover
geo-installed = Instaladas
geo-available = Regiões do mundo
geo-none-installed = Nenhuma região tem as ruas nesta máquina. Um endereço para na cidade mais próxima.
geo-no-world = Esta build não carrega os dados do mundo, então não há catálogo para escolher.
geo-filter = Filtro
geo-filter-none = todas as regiões
geo-pack = { $size } · { $ways } vias · { $version }
geo-have = instalada
geo-get = instalar
geo-in-parts = em partes
geo-only-in-parts = Esta região é publicada em partes. As partes estão nesta lista, e o filtro as encontra pelo nome dela.
geo-sure = Remover as ruas de { $region }?
geo-no = Não, manter
geo-yes = Sim, remover
geo-not-installed = Essa linha não é uma região instalada.
geo-not-removed = A região não pôde ser removida.
geo-not-in-catalogue = Esta região não está no catálogo desta build, então não dá para baixá-la de novo.
ask-geo-find = O nome de uma região, ou parte dele — vazio mostra todas
geo-queued = Na fila. O tamanho é o que o servidor responde, e dá para desistir por lá.
geo-no-home = Não há diretório de dados onde instalar.
cmd-geo-open = Regiões e suas ruas, no rodapé
cmd-geo-window = Regiões e suas ruas, em janela
cmd-geo-up = Linha anterior
cmd-geo-down = Próxima linha
cmd-geo-enter = Instalar a região sob o cursor, ou instalar de novo
cmd-geo-find = Estreitar a lista de regiões
cmd-geo-remove = Remover a região instalada sob o cursor
group-geo = geografia
# PARA QUE SERVE CADA SEÇÃO. Toda uma delas abre com uma frase dizendo o que
# muda e ONDE isso aparece, porque uma tela de ajustes que só os nomeia deixa a
# pessoa adivinhar qual metade da máquina eles alcançam.
locale-interface-does = O idioma que esta interface fala. Muda na hora, no rodapé e em toda janela; o idioma da máquina é definido na configuração do sistema.
locale-time-does = Como esta interface escreve toda data e toda hora que ela desenha — o relógio do rodapé, as datas do gerenciador de arquivos, o relance. Não mexe no relógio da máquina; isso é a seção abaixo.
locale-numbers-does = Como os números são escritos e em que unidade sai a temperatura. Alcançam toda tela daqui, o relance inclusive.
locale-heading-machine-clock = O relógio desta máquina
locale-machine-does = O relógio, o fuso e a data do computador — o que todo programa desta máquina lê, e não só esta interface.
locale-automatic = Mantido certo automaticamente
locale-automatic-on = ligado
locale-automatic-off = desligado
locale-automatic-does = O sistema pergunta a hora a um servidor pela internet e mantém o relógio em dia com ele. É assim que a distro roda; desligar entrega o relógio a você.
locale-automatic-server = Perguntando a { $server }.
locale-automatic-synchronised = O relógio está em dia.
locale-automatic-adrift = Ele ainda não conseguiu falar com nenhum servidor de horário.
locale-automatic-cannot = Não há cliente de horário instalado nesta máquina.
locale-watched = Vigiado pelo brain
locale-watched-does = A vigia do relógio do brain olha algumas vezes por dia e religa o acerto automático se o encontrar desligado. É o mesmo interruptor da página daquela tarefa.
locale-watched-unknown = O brain não está rodando, então isto não dá para ler nem mudar daqui.
locale-machine-kept = Enquanto o relógio é mantido automaticamente, estes não podem ser acertados à mão: a próxima checagem os colocaria de volta.
locale-machine-unreachable = Não deu para perguntar nada sobre o relógio desta máquina; nada aqui pode ser mudado.
# AS DUAS DA MÁQUINA DIZEM O QUE APERTÁ-LAS FAZ. `Data` e `Hora` são as
# linhas da INTERFACE uma seção acima — elas escolhem um FORMATO e mostram
# uma amostra viva dele —, e duas linhas de uma lista com a mesma palavra são
# dois fatos que ninguém distingue.
locale-machine-date = Acertar a data
locale-machine-time = Acertar a hora
locale-machine-zone = Fuso horário
locale-shows-times-in = Mostrar as horas em
# O FORMATO É ESCRITO POR EXTENSO e não é a ordem escolhida pela pessoa: aqui
# se DIGITA um valor, e um pedido que seguisse a preferência de exibição
# pediria `09/21/2026` numa máquina e `21/09/2026` na seguinte.
ask-locale-system-date = A data da máquina, como 2026-09-21
ask-locale-system-time = A hora da máquina, como 14:32 ou 14:32:05
cmd-locale-system-date = Acertar a data desta máquina
cmd-locale-system-time = Acertar a hora desta máquina
cmd-locale-system-zone = Escolher o fuso horário desta máquina
locale-not-a-date = Isso não é uma data nesse formato.
locale-not-a-time = Isso não é uma hora nesse formato.
locale-clock-refused = A máquina recusou a mudança; veja: journalctl -u systemd-timedated
locale-heading-glance = A linha do relance
locale-glance-does = A linha que { $keys } abre acima da modeline. Cada lugar abaixo desenha a hora de lá, e a data, a temperatura e o câmbio que você ligar para ele.
locale-add-place-where = Um fuso, como Europe/Lisbon. Ele nasce mostrando o relógio e nada mais.
locale-no-places = Nenhum lugar é observado; o { $keys } não mostraria nada.
locale-shows-clock = Hora
locale-shows-date = Data
locale-shows-weather = Tempo
locale-shows-rate = Câmbio
locale-on = aparece
locale-off = escondido
locale-heading-sources = Quem é perguntado
locale-asked = perguntado
locale-not-asked = não perguntado
locale-sources-are-asked = Cada busca conta a esse host o endereço desta máquina e quais lugares ela observa. Nada aqui precisa de conta nem de chave.
locale-source-weather = O tempo
locale-source-rate = Câmbio
# Os hosts que uma fonte pergunta, na ordem em que ela pergunta: o segundo só
# é perguntado quando o primeiro não responde.
locale-source-hosts = Pergunta { $hosts }, nessa ordem.
locale-source-answered = Última resposta { $when }, de { $host }.
locale-source-never = Ninguém respondeu ainda.
locale-source-trouble = A última tentativa falhou — { $said }
locale-title = Idioma e região
locale-up = cima
locale-down = baixo
locale-enter = escolher
locale-language = Idioma
locale-clock = Relógio
locale-date = Data
locale-zone-is-yours = Você escolheu este fuso; o da máquina fica na configuração do sistema.
locale-zone-is-the-machines = O fuso da máquina. Escolher um aqui muda o que esta interface mostra, não o que o sistema roda.
locale-heading-numbers = Números e unidades
locale-decimal-language = { $mark } — o do idioma
locale-decimal-point = { $mark } — ponto, seja qual for o idioma
locale-decimal-comma = { $mark } — vírgula, seja qual for o idioma
locale-units-metric = Métrico — °C
locale-units-imperial = Imperial — °F
locale-drop-place = Remover este lugar
locale-add-place = Acrescentar um lugar
locale-add-place-how = um fuso, como Europe/Lisbon
ask-locale-where = Latitude e longitude, dois números
locale-no-such-zone = Não existe fuso com esse nome.
locale-not-a-coordinate = Isso não é uma latitude e uma longitude.
locale-no-place-here = Ponha o cursor num dos lugares do relance primeiro.
locale-place-gone = Esse lugar não está mais no relance.
cmd-locale-less = Valor anterior
cmd-locale-more = Próximo valor
cmd-locale-zone = Escolher o fuso em que esta interface mostra as horas
cmd-locale-place = Acrescentar um lugar ao relance
cmd-locale-where = Dizer onde fica um lugar
locale-less = anterior
locale-more = próximo
locale-where = onde fica
# A MODULE THE SYSTEM SWITCHED OFF (`ronoctua.brain.modules.<id>.enable =
# false`): its row's reason, and what every ask about it is refused with — a
# row's reason sits in a column sixteen cells wide.
brain-why-system-off = Sistema desligou
# And the date-and-time page's line under the clock watch's switch, when the
# system switched that watch off.
locale-watched-system-off = A configuração do sistema desligou esta vigia, então ela não liga daqui; é a configuração que a liga de volta.

## file info editing — everything the info panel can change

# As moedas de um lugar, escolhidas na tela.
locale-currency = Moeda
locale-currency-pair = { $from } em { $into }
locale-currency-none = nenhuma escolhida
locale-not-a-currency-pair = Isso não são duas moedas que as cotações conhecem, de três letras cada, como EUR BRL.
ask-locale-currency = A moeda dele e a moeda em que vale, como EUR BRL
