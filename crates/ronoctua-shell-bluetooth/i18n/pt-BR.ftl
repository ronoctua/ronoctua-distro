# The texts `ronoctua-shell-bluetooth` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

cmd-bluetooth-open = Bluetooth
cmd-bluetooth-window = Bluetooth numa janela
cmd-bluetooth-scan = Procurar dispositivos por perto
cmd-bluetooth-power = Ligar ou desligar o rádio
cmd-bluetooth-discoverable = Deixar outros dispositivos verem esta máquina
cmd-bluetooth-reconnect = Reconectar os dispositivos confiáveis
cmd-bluetooth-disconnect-all = Desconectar todos os dispositivos
cmd-bluetooth-accept = Dizer sim ao pareamento
cmd-bluetooth-refuse = Dizer não ao pareamento
cmd-bluetooth-device = Abrir o dispositivo sob o cursor
cmd-bluetooth-connect = Conectar ou desconectar o dispositivo
cmd-bluetooth-pair = Parear com o dispositivo
cmd-bluetooth-forget = Esquecer o dispositivo
cmd-bluetooth-trust = Confiar no dispositivo, ou deixar de confiar
cmd-bluetooth-block = Bloquear o dispositivo, ou desbloquear
cmd-bluetooth-rename = Dar outro nome ao dispositivo
cmd-bluetooth-unnamed = Mostrar os dispositivos que não dizem o nome
cmd-bluetooth-up = Uma linha acima
cmd-bluetooth-down = Uma linha abaixo
group-bluetooth = bluetooth
# A LINHA DE DICAS lê cada id da tabela de teclas da vista como um texto curto.
bluetooth-down = desce
bluetooth-up = sobe
bluetooth-connect = conectar
bluetooth-pair = parear
bluetooth-forget = esquecer
bluetooth-trust = confiar
bluetooth-block = bloquear
bluetooth-scan = procurar
bluetooth-unnamed = sem nome
bluetooth-accept = sim
bluetooth-refuse = não
bluetooth-row-kind = { $name } — { $kind }
bluetooth-audio-state = { $what } — { $state }
# AS TRÊS SEÇÕES, e o que uma vazia diz.
bluetooth-heading-adapter = Rádio
bluetooth-heading-paired = Pareados
bluetooth-heading-nearby = Por perto
bluetooth-nothing-here = nada aqui
bluetooth-no-adapter = não há rádio Bluetooth nesta máquina
bluetooth-scanning = procurando dispositivos
bluetooth-unnamed-hidden = { $count } por perto sem nome — aperte h para mostrar
# AS LINHAS DO PRÓPRIO RÁDIO.
bluetooth-adapter-power = o rádio
bluetooth-adapter-discoverable = outros dispositivos veem esta máquina
bluetooth-adapter-pairable = outros dispositivos podem parear com ela
bluetooth-adapter-timeout = deixa de estar visível depois de
bluetooth-adapter-forever = fica visível
bluetooth-adapter-pairable-forever = fica aberto
bluetooth-adapter-alias = esta máquina se chama
bluetooth-power-state-on = ligado
bluetooth-power-state-off = desligado
bluetooth-power-state-off-enabling = ligando
bluetooth-power-state-on-disabling = desligando
bluetooth-power-state-off-blocked = travado pela chave do hardware
# OS FATOS DE UM DISPOSITIVO, um por linha, na ordem em que o detalhe desenha.
bluetooth-fact-address = endereço
bluetooth-fact-address-type = tipo de endereço
bluetooth-fact-alias = chamado de
bluetooth-fact-name = nome próprio
bluetooth-fact-connected = conectado
bluetooth-fact-paired = pareado
bluetooth-fact-bonded = as chaves ficaram guardadas
bluetooth-fact-trusted = confiável
bluetooth-fact-blocked = bloqueado
bluetooth-fact-legacy = pareamento antigo
bluetooth-fact-wake = pode acordar esta máquina
bluetooth-fact-services = serviços lidos
bluetooth-fact-battery = bateria
bluetooth-fact-battery-source = a bateria é informada por
bluetooth-fact-rssi = sinal
bluetooth-fact-tx-power = ele transmite a
bluetooth-fact-class = classe
bluetooth-fact-appearance = aparência
bluetooth-fact-modalias = id de firmware
bluetooth-fact-adapter = pelo rádio
bluetooth-fact-reconnect = quem reconecta
bluetooth-fact-volume = volume que ele informa
bluetooth-fact-codec = codec
bluetooth-fact-audio = perfil de áudio
bluetooth-yes = sim
bluetooth-no = não
bluetooth-rssi-value = { $dbm } dBm
bluetooth-tx-power-value = { $dbm } dBm
bluetooth-percent = { $percent }%
bluetooth-seconds = { $seconds }s
# AS ENUMERAÇÕES DO PRÓPRIO BLUEZ, na língua de quem lê. São listas fechadas,
# não dado livre — o NOME do dispositivo continua sendo dele. O `word_or` cai
# na palavra do daemon se ele ganhar um valor novo, então o novo fica sem
# tradução e nunca `[uma chave entre colchetes]`.
bluetooth-adapter-pairable-timeout = aberto para parear por
bluetooth-fact-manufacturer = fabricante
bluetooth-fact-version = versão do bluetooth
bluetooth-fact-roles = papéis
bluetooth-fact-service = oferece
bluetooth-audio-profile = o que está fazendo
bluetooth-heading-settings = Outras configurações
bluetooth-setting-volume = volume
bluetooth-setting-mute = no mudo
bluetooth-nothing-to-adjust = nada para ajustar aqui
bluetooth-settings = a outra metade
bluetooth-more = mais
bluetooth-less = menos
cmd-bluetooth-settings = Bluetooth: atravessa para a outra metade da página
cmd-bluetooth-more = Bluetooth: mais do que estiver sob o cursor
cmd-bluetooth-less = Bluetooth: menos do que estiver sob o cursor
bluetooth-audio-off = desligado
bluetooth-audio-a2dp-sink = música, em estéreo
bluetooth-audio-a2dp-source = música, vinda dele
bluetooth-audio-headset-head-unit = chamada, com o microfone dele
bluetooth-audio-headset-audio-gateway = chamada, por esta máquina
bluetooth-codec-0 = SBC
bluetooth-codec-1 = áudio MPEG-1/2
bluetooth-codec-2 = AAC
bluetooth-codec-4 = aptX
bluetooth-codec-255 = o do fabricante
bluetooth-address-type-public = fixo
bluetooth-address-type-random = aleatório
bluetooth-reconnect-none = nunca
bluetooth-reconnect-host = quando esta máquina pede
bluetooth-reconnect-device = quando o dispositivo pede
bluetooth-reconnect-any = qualquer um dos dois
bluetooth-transport-idle = parado
bluetooth-transport-pending = começando
bluetooth-transport-active = tocando
bluetooth-transport-suspending = parando
bluetooth-transport-broadcasting = transmitindo
# O QUE UM DISPOSITIVO FAZ — os perfis por trás dos ids de serviço.
bluetooth-profile-headset = chamadas, com o microfone dele
bluetooth-profile-handsfree = chamadas com as mãos livres
bluetooth-profile-audio-sink = toca o som desta máquina
bluetooth-profile-audio-source = manda som para esta máquina
bluetooth-profile-remote-control = os botões dele controlam a reprodução
bluetooth-profile-human-interface = manda teclas e movimentos de ponteiro
bluetooth-profile-file-transfer = navegar pelos arquivos dele
bluetooth-profile-object-push = receber arquivos enviados a ele
bluetooth-profile-phone-book = ler os contatos de um telefone
bluetooth-profile-message-access = ler as mensagens de um telefone
bluetooth-profile-network = compartilhar a conexão dele
bluetooth-profile-battery = informar quanta carga tem
bluetooth-profile-device-information = informar fabricante e modelo
bluetooth-profile-serial = uma porta serial pelo ar
bluetooth-profile-sim-access = ler o SIM de um telefone
bluetooth-fact-kind = o que ele é
bluetooth-heading-profiles = Os serviços que ele anuncia
bluetooth-about-profile = { $id } — { $said }
bluetooth-about-profile-unknown = { $id } — um id do próprio fabricante, sem significado publicado. É desenhado como está, que é o que um buscador aceita.
bluetooth-about-audio-profile = Por onde o som vai: música em estéreo, ou uma chamada com o microfone. Quem decide é o PipeWire, não o aparelho.
bluetooth-about-sink-volume = Quão alto esta máquina toca por ele. RET sobe um passo; as setas movem para os dois lados.
bluetooth-about-mute = Silencia sem perder o número, então voltar devolve o mesmo volume.
bluetooth-door-this-keyboard = configurações deste teclado
bluetooth-door-all-keyboards = configurações de todos os teclados
bluetooth-about-this-keyboard = Abre as configurações de teclado com este selecionado, onde uma tecla pode passar a significar outra.
bluetooth-about-all-keyboards = Abre as configurações de teclado, que listam todos os teclados desta máquina.
# O QUE UMA LINHA QUER DIZER, para a linha de mensagem que o ponteiro consulta.
bluetooth-about-adapter = Por qual rádio desta máquina ele está pareado. Uma máquina com um chip Bluetooth tem um.
bluetooth-about-address = O endereço de hardware dele — os seis bytes que identificam esta unidade, e contra os quais os ajustes são lembrados.
bluetooth-about-address-type = Um endereço público é do fabricante para sempre; um aleatório é gerado, e alguns aparelhos trocam o deles para não serem seguidos.
bluetooth-about-alias = O nome que VOCÊ deu. É o que toda tela daqui mostra; o nome do próprio aparelho está na linha abaixo.
bluetooth-about-appearance = Um número que o aparelho manda para dizer que forma tem — 961 é teclado, 962 é mouse. Os de baixa energia mandam isto no lugar da classe.
bluetooth-about-audio = O que o enlace de som está fazendo agora, como o daemon reporta.
bluetooth-about-battery = Quanta carga ele diz ter. Nem todo aparelho reporta, e nenhum é preciso.
bluetooth-about-battery-source = Quem está reportando essa carga: o serviço padrão de bateria, ou o do próprio fabricante.
bluetooth-about-blocked = Bloqueado quer dizer que esta máquina o recusa, mesmo que ele peça. É como impedir que reconecte sem esquecê-lo.
bluetooth-about-bonded = As chaves combinadas no pareamento continuam guardadas, então os dois podem confiar um no outro sem parear de novo.
bluetooth-about-class = Um número que um aparelho clássico manda para dizer o que é — fone, telefone, computador. Os de baixa energia mandam uma aparência.
bluetooth-about-codec = Como o som é comprimido no caminho. O SBC funciona em tudo; os outros exigem que os dois lados concordem.
bluetooth-about-connected = Se há enlace de pé agora. Um aparelho pareado e desligado está pareado e não conectado.
bluetooth-about-kind = O que este aparelho é, deduzido do que ele diz de si — a classe, a aparência e os serviços que oferece.
bluetooth-about-legacy = Ele pareia do jeito antigo, sem a comparação numérica. Acessórios mais velhos fazem isso, e é menos seguro.
bluetooth-about-modalias = Os números de fabricante e modelo que ele reporta. É por isto que o keyd distingue um teclado, e é o que um buscador acha.
bluetooth-about-name = O nome que o aparelho dá a si mesmo. O que você vê em todo o resto é o apelido acima.
bluetooth-about-paired = Se esta máquina e o aparelho já trocaram chaves. Parear é o que torna possível conectar depois.
bluetooth-about-reconnect = Qual lado pode iniciar a conexão — esta máquina, o aparelho, ou qualquer um dos dois.
bluetooth-about-rssi = Quão alto esta máquina o escuta, em dBm. Mais perto de zero é mais forte; abaixo de uns -90 está no limite.
bluetooth-about-services = Se o daemon já terminou de perguntar ao aparelho o que ele faz. Enquanto não terminou, a lista abaixo pode estar curta.
bluetooth-about-trusted = Um aparelho confiável conecta sem perguntar toda vez. É a diferença entre um teclado que simplesmente funciona e um que pergunta.
bluetooth-about-tx-power = Quão alto o aparelho diz estar transmitindo. Junto com a força do sinal acima, dá uma ideia grosseira de distância.
bluetooth-about-volume = O volume que o APARELHO reporta pelo Bluetooth. O volume em que esta máquina toca está na outra coluna.
bluetooth-about-wake = Se apertar uma tecla nele pode trazer esta máquina de volta do sono.
bluetooth-pairing-answer = y aceita · n recusa

## file info editing — everything the info panel can change

# A internet de um celular, pelo Bluetooth.
bluetooth-fact-internet = usa a internet dele
bluetooth-about-internet = O celular compartilha a conexão pelo Bluetooth. A ligação é do NetworkManager, e a vista da rede também a mostra, com a mesma interface.
bluetooth-internet-through = sim, por { $interface }
echo-bluetooth-internet-on = saindo por { $device } agora
echo-bluetooth-internet-off = não sai mais por { $device }
echo-bluetooth-internet-failed = não deu para usar a internet de { $device }: { $reason }
