# The texts `ronoctua-shell-network` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Network — Wi-Fi and Ethernet (LEADER s n)

cmd-network-open = Redes
cmd-network-scan = Varrer redes
cmd-network-connect = Conectar à rede
cmd-network-disconnect = Desconectar
cmd-network-forget = Esquecer a rede
cmd-network-wifi-toggle = Alternar o rádio Wi-Fi
cmd-network-link = Mostrar o enlace
group-network = rede
# THE VIEW. States the list shows in place of rows when there is no row to
# show — each says something different, and folding them into one generic
# "no networks" would hide the difference between not finding and not looking.
network-scanning = varrendo…
network-empty = nenhuma rede ao alcance
network-no-adapter = esta máquina não tem rádio Wi-Fi
network-radio-off = rádio desligado
network-wired = cabo
network-saved = salva
network-password = Senha de { $network }
network-password-help = Guardada pelo NetworkManager, fora do repositório.
# ECHOES — what the minibuffer says after the action.
echo-network-joining = Conectando a { $network }…
echo-network-connected = Conectado a { $network }
echo-network-failed = Não consegui conectar a { $network }: { $reason }
echo-network-disconnected = Desconectado
echo-network-forgotten = { $network } esquecida
echo-network-radio-on = Rádio Wi-Fi ligado
echo-network-radio-off = Rádio Wi-Fi desligado
echo-network-link = { $network } · { $band } · { $rate } · sinal { $signal }
echo-network-link-wired = { $network } · { $rate }
echo-network-no-link = Sem conexão
echo-network-unavailable = O NetworkManager não está respondendo

## Network — the password sealed in the TPM (psk-flags)

# The one-key question, after the connection succeeds.
network-remember = Guardar a senha de { $network } nesta máquina?
network-remember-no = não, não guardar nada
network-remember-yes = sim, selar no TPM
# The list marker: tells "saved in NetworkManager" apart from "the password exists".
network-no-tpm = no TPM
echo-network-stored = Senha de { $network } selada no TPM
echo-network-no-stored = Conectado. A senha não ficou guardada.
echo-network-store-failed = Conectado, mas não consegui guardar a senha: { $reason }

## Energia — a bateria, o carregador, o perfil e o sono (LEADER s p)

# A SAÍDA — as três marcas, e o que o daemon acredita.
network-way = a internet
network-airplane = modo avião
network-tunnel-drop = RET derruba
cmd-network-airplane = Modo avião (alterna)
network-way-unknown = ainda não decidido
network-way-none = sem saída
network-way-portal = uma página de login está no caminho
network-way-limited = conectado, mas ela não responde
network-way-full = ela responde
# A SAÍDA É UM TÚNEL: nomeado, em vez do palpite de conectividade do daemon.
network-way-vpn = por { $place }
# A VISTA — uma lista, quatro títulos, e as mesmas linhas nas duas portas.
network-heading-overview = num relance
network-heading-links = cabo e Wi-Fi
network-heading-wifi = Wi-Fi
network-heading-remembered = redes lembradas
network-no-remembered = esta máquina não lembra nenhuma rede que não esteja vendo
network-out-of-range = fora de alcance
network-heading-tunnels = túneis
network-radio = rádio Wi-Fi
network-on = ligado
network-off = desligado
network-no-link = esta máquina não tem cabo nem rádio
network-no-traffic = nada entrou nem saiu ainda
network-no-tunnel = nenhum túnel aqui
network-tunnel-raise = RET levanta
network-state-connected = conectado
network-state-connecting = conectando…
network-state-disconnected = sem conexão
network-fact-network = rede Wi-Fi
network-fact-band = banda do Wi-Fi
network-fact-signal = força do sinal
network-fact-rate = velocidade da conexão
network-fact-address = endereço desta máquina
network-import-address = seu endereço nela
network-fact-address-tailnet = IPv4 no tailnet
network-fact-address-tailnet6 = IPv6 no tailnet
network-fact-gateway = roteador
network-fact-dns = servidor DNS
network-fact-received = recebido
network-fact-received-boot = recebido desde o boot
network-fact-sent = enviado
network-fact-sent-boot = enviado desde o boot
network-heading-ports = portas abertas
network-heading-monitor = tráfego
network-no-ports = ninguém escutando
network-port-not-ours = não é nosso para ver
network-ports-more = { $count ->
    [one] e mais um nosso
   *[other] e mais { $count } nossos
  }
network-per-second = { $bytes }/s
# AS UNIDADES SÃO AS MESMAS EM TODO IDIOMA; o SEPARADOR DECIMAL não é, e é por
# isso que estas são chaves com um número dentro, e não uma string montada no
# crate.
network-rate-kilobits = { $rate } kb/s
network-rate-megabits = { $rate } Mb/s
network-rate-gigabits = { $rate } Gb/s
network-no-rate = não está de pé
network-band = { $ghz } GHz
network-band-unknown = uma frequência incomum
network-heading-lan = máquinas nesta rede
network-lan-nobody = ninguém foi falado ainda
network-lan-gateway = o roteador desta rede
network-lan-this-machine = esta máquina
network-lan-looking = procurando as outras…
network-lan-answered = respondeu
network-lan-refused = esta máquina não pode mandar ping: o sistema não a deixa abrir o socket
network-lan-swept = { $answered ->
    [one] uma máquina respondeu de { $asked } perguntadas, { $range }
   *[other] { $answered } máquinas responderam de { $asked } perguntadas, { $range }
  }
cmd-network-lan = Procurar as outras máquinas daqui
network-lan = procurar
network-section-next = seção
network-section-previous = seção anterior
network-hotspot-start = compartilhar esta conexão
network-hotspot-stop = parar de compartilhar
network-hotspot-cost = o rádio sai da rede em que está: o que ele compartilha é o cabo
network-hotspot-on = esta máquina é a rede agora
network-hotspot-password = senha do ponto de acesso
echo-network-hotspot = compartilhando como { $network }, senha { $password }
echo-network-hotspot-failed = não deu para compartilhar: { $reason }
cmd-network-import = Trazer uma VPN de um arquivo
cmd-network-section-next = Próxima seção da vista de rede
cmd-network-section-previous = Seção anterior da vista de rede
cmd-network-tor-open = Tor
cmd-network-tor-more = Girar este ajuste do Tor para a frente
cmd-network-tor-less = Girar este ajuste do Tor para trás
cmd-network-tor-back = Sair dos ajustes do Tor
network-import-file = arquivo a trazer
network-import-kind = tipo de VPN
network-import-wireguard = WireGuard
network-import-openvpn = OpenVPN
network-import-endpoint = vai para
network-import-dns = mudaria seu DNS para
network-import-everything = tudo o que esta máquina manda passaria por ele
network-import-asks = pede usuário e senha assim que for trazida, e os guarda
network-import-bring = trazer
network-import-leave = deixar onde está
network-import-where = Caminho do arquivo .conf ou .ovpn
network-import-help = Leia antes de trazer: o arquivo decide por onde seu tráfego sai.
echo-network-imported = { $network } trazido
echo-network-not-a-tunnel = { $file } não é arquivo WireGuard nem OpenVPN
echo-network-import-failed = não deu para trazer { $file }: { $reason }
# OS DOIS TÚNEIS. Nenhum fica ligado em repouso: o Tor não sobe no boot e o
# Tailscale precisa de login, então uma máquina que não usa nenhum dos dois não
# desenha nenhum dos títulos.
network-heading-tailscale = Tailscale
network-heading-tor = Tor
network-tailscale-state = a conexão ao tailnet
network-tailscale-running = de pé
network-tailscale-stopped = desligado
network-tailscale-needslogin = esperando um login
network-tailscale-nostate = subindo
network-tailscale-starting = subindo
network-tailscale-needsmachineauth = esperando aprovação no tailnet
network-tailscale-inuseotheruser = outra pessoa nesta máquina está usando
network-tailscale-login = entrar
network-tailscale-logout = sair
network-tailscale-routes = aceitar rotas do tailnet
network-tailscale-shields = recusar o que chega
network-tailscale-online = on-line
network-tailscale-offline = off-line
network-tailscale-exit = tudo sai por aqui
network-tor-state = o serviço do Tor
network-tor-ready = há um circuito
network-tor-not-ready = sem circuito
network-tor-bootstrap = subindo, { $percent }%
network-tor-circuits = { $count ->
    [one] um circuito aberto
   *[other] { $count } circuitos abertos
  }
network-tor-identity = pedir uma identidade nova
# O TOR, INTEIRO. A seção é desenhada com o Tor rodando ou não, e isso é o
# conserto e não uma novidade: ela vivia atrás de "o socket de controle
# respondeu", o que só é verdade com o Tor já de pé — então um Tor parado não
# tinha título, nem linha, nem tecla, e o único interruptor que existia estava
# escrito para PARAR. Não havia como ligar o Tor em lugar nenhum do shell.
network-tor-running = de pé
network-tor-stopped = parado
network-tor-moving = mudando
network-tor-absent = não instalado nesta máquina
network-tor-unknown = não dá para perguntar
network-tor-start = ligar
network-tor-stop = desligar
network-tor-autostart = subir junto com a sessão
network-tor-settings = ajustes
network-tor-sheet = Tor
network-tor-browser = abrir um navegador pelo Tor
# O QUE O FLAKE FIXA. A linha é desenhada, é alcançável e recusa, dizendo de
# onde vem o valor — a forma que a linha desabilitada do XFS já tem no
# formatar. Editá-la seria uma tecla cujo efeito o torrc gerado desfaz antes de
# ela ser apertada.
network-tor-from-flake = definido pela configuração do sistema
network-tor-heading-service = O serviço
network-tor-heading-path = Por onde sai
network-tor-heading-bridges = Pontes
network-tor-heading-onion = Serviços onion
network-tor-heading-advanced = Raramente mexido
network-tor-advanced-about = Estes mudam como o Tor monta circuitos. Os padrões são o que a maioria quer.
ask-tor-setting = O valor novo — vazio limpa
ask-tor-lines = Uma linha para acrescentar — vazio limpa todas
network-tor-back = voltar
network-tor-edit = mudar
network-tor-more = mais
network-tor-less = menos
network-tor-drawer = a gaveta
echo-tor-written = Os ajustes do Tor foram gravados
echo-tor-refused = O Tor não aceitou: { $reason }
echo-tor-not-running = gravado; o Tor vai ler quando subir
network-fact-name = nome no tailnet
network-fact-tailnet = nome do seu tailnet
network-fact-magic-dns = MagicDNS
network-fact-version = versão do Tor
network-fact-socks = endereço SOCKS
# AS PALAVRAS DA LINHA DE DICAS — o id lido como chave de texto, curtas porque
# a linha é uma só. A forma longa é `cmd-<id>`, que o manual lê.
network-up = sobe
network-down = desce
network-scan = procura
network-wifi-toggle = rádio
network-disconnect = desconecta
network-forget = esquece
network-link = enlace
network-title = rede
cmd-network-window = Rede numa janela
cmd-network-up = Uma linha acima
cmd-network-down = Uma linha abaixo
echo-network-tunnel-up = { $network } está subindo
echo-network-tunnel-down = { $network } caiu
echo-network-deleted = { $network } apagado

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-tunnels = Túneis (.conf, .ovpn)
network-pick-tunnel = Um túnel para trazer
network-pick-vpn-folder = Uma pasta de configs WireGuard para cadastrar

## file info editing — everything the info panel can change

# A vista da rede: a ida e volta até o roteador, medida quando se pede.
network-latency = ida e volta até o roteador
network-latency-measure = medir
network-latency-measuring = medindo…
network-latency-ms = de { $fastest } a { $slowest } ms ({ $answered } de { $asked })
network-latency-silent = sem resposta
network-latency-refused = não deu para medir: { $error }
# Os nomes das máquinas da rede, pelo systemd-resolved.
network-lan-names-refused = os nomes não são conhecidos: { $why }
lan-resolve-why-no-bus = o barramento do sistema não respondeu
lan-resolve-why-no-resolved = o systemd-resolved não está rodando
lan-resolve-why-failed = o systemd-resolved não soube responder
# Uma VPN cujo arquivo pede usuário e senha.
ask-network-vpn-user = Usuário desta VPN
ask-network-vpn-password = Senha desta VPN
echo-network-vpn-stored = { $network } vai subir sem perguntar
echo-network-vpn-store-failed = a senha da VPN não foi guardada: { $reason }
# Cadastrar uma pasta inteira de servidores WireGuard, graficamente — a chave uma vez.
network-vpn-register = Cadastrar VPNs de uma pasta…
ask-network-vpn-register-key = Chave privada WireGuard (uma vez, para a pasta toda)
echo-network-vpn-registered = { $count } servidor(es) de VPN cadastrado(s) — suba um da lista
echo-network-vpn-not-a-key = isso não é uma chave privada WireGuard (44 caracteres base64)
echo-network-vpn-no-servers = nenhum servidor WireGuard foi encontrado nessa pasta
echo-network-vpn-unreadable = não foi possível ler essa pasta
echo-network-vpn-install-failed = o cadastro da VPN não foi concluído
# Filtrar os locais cadastrados por país ou cidade.
network-vpn-filter = Filtrar por país…
network-vpn-filter-on = Mostrando só: { $filter } — RET para mudar
network-vpn-filter-none = nenhum local corresponde
ask-network-vpn-filter = País ou cidade para mostrar (vazio: todos)
# As configurações de runtime da VPN, alternadas da vista sem rebuild.
network-vpn-ipv6 = Levar o IPv6 pelo túnel
network-vpn-killswitch = Kill switch (bloquear tudo fora do túnel)
network-vpn-dns = Resolvedores para todos os locais
network-vpn-on = ligado
network-vpn-off = desligado
network-vpn-dns-provider = o do próprio provedor
ask-network-vpn-dns = Resolvedores de toda VPN, separados por vírgula (vazio: o de cada um)
echo-network-vpn-settings = as configurações da VPN foram salvas
echo-network-vpn-settings-failed = as configurações da VPN não puderam ser preparadas
# Subir um local (via root) e esquecer um.
echo-network-vpn-connecting = conectando a { $place } — a modeline mostra o país quando ele responder
echo-network-vpn-forgotten = { $place } foi esquecido
echo-network-vpn-forget-failed = não foi possível esquecer esse local
# O enlace de um celular na vista da rede, e as recusas do próprio crate da rede.
network-fact-phone = celular
