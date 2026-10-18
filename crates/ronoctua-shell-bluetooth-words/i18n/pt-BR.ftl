# The texts `ronoctua-shell-bluetooth-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Bluetooth — o rádio, os dispositivos e o que cada um faz (LEADER s b)

ask-bluetooth-rename = Um nome para ele
bluetooth-device = abrir
bluetooth-rename = renomear
bluetooth-power = rádio
bluetooth-discoverable = visível
bluetooth-title = Bluetooth
bluetooth-no-daemon = o serviço de Bluetooth não está respondendo
# OS TIPOS, da classe ou da aparência que o próprio dispositivo diz.
bluetooth-kind-headphones = fones de ouvido
bluetooth-kind-headset = headset
bluetooth-kind-speaker = caixa de som
bluetooth-kind-keyboard = teclado
bluetooth-kind-mouse = mouse
bluetooth-kind-gamepad = controle
bluetooth-kind-tablet = tablet
bluetooth-kind-phone = telefone
bluetooth-kind-computer = computador
bluetooth-kind-watch = relógio
bluetooth-kind-camera = câmera
bluetooth-kind-printer = impressora
bluetooth-kind-network = rede
bluetooth-kind-display = tela
bluetooth-kind-unknown = dispositivo
# PAREAMENTO — a pergunta que o outro lado faz, e as duas respostas.
bluetooth-pairing-confirm = { $name } quer parear. Ele mostra { $code }?
bluetooth-pairing-passkey = digite { $code } em { $name }
bluetooth-pairing-pin = { $name } quer um PIN
bluetooth-pairing-passkey-here = { $name } quer seis dígitos digitados aqui
bluetooth-pairing-authorize = { $name } quer parear
bluetooth-pairing-service = { $name } quer usar { $service }
# O QUE DEU ERRADO, dito como o nome da porta que fechou.
echo-bluetooth-refused = { $what }: { $why }
bluetooth-what-connect = a conexão
bluetooth-what-pair = o pareamento
bluetooth-what-forget = esquecer o dispositivo
bluetooth-what-radio = o rádio
bluetooth-what-device = o dispositivo
bluetooth-refused-authority = esta sessão não tem permissão
bluetooth-refused-absent = não existe nesta máquina
bluetooth-refused-away = ninguém respondeu
bluetooth-refused-busy = outra coisa está fazendo isso
bluetooth-refused-rejected = o outro lado disse não
bluetooth-refused-other = não funcionou

## file info editing — everything the info panel can change

bluetooth-refused-timed-out = não teve resposta a tempo
bluetooth-refused-no-bus = o barramento do sistema não respondeu
