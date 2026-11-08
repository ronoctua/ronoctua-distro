# The book `packets` (`ronoctua-packets`), in pt-BR (invariant 8).
#
# A BOOK: these sentences are read only by the network protocols' manual
# (`LEADER h`, then the protocols), from their own file beside the texts —
# never at the start of every program. Every declared language
# (`assets/i18n/languages.nix`) has a file here with exactly the same keys,
# the same arguments in each value and no empty value (`nix/scripts/gen-i18n.py`).
#
# What travels on the wire (a request line, a flag, an address) is not here:
# it is data in the crate's tables, the same in every language.

## The list

packets-group-link = Enlace e rede
packets-group-transport = Transporte
packets-group-naming = Nomes e descoberta
packets-group-web = A web
packets-group-mail = E-mail e arquivos
packets-group-security = Segurança e túneis
packets-group-things = Tempo, máquinas e pares

packets-layer-1 = Física (camada 1)
packets-layer-2 = Enlace (camada 2)
packets-layer-3 = Rede (camada 3)
packets-layer-4 = Transporte (camada 4)
packets-layer-5 = Sessão (camada 5)
packets-layer-6 = Apresentação (camada 6)
packets-layer-7 = Aplicação (camada 7)

## The legend

packets-class-lookup = Consulta
packets-class-handshake = Handshake
packets-class-request = Pedido
packets-class-response = Resposta
packets-class-data = Dados
packets-class-control = Controle
packets-class-crypto = Chaves
packets-class-refusal = Recusa

## The window

packets-overview = Visão geral
packets-steps = Passos
packets-step-of = Passo { $n } de { $total }
packets-packet-of = Pacote { $n } de { $total }
packets-ready = Toque para ver acontecer, ou escolha um passo.
packets-col-no = Nº
packets-col-time = Tempo
packets-col-source = Origem
packets-col-destination = Destino
packets-col-protocol = Protocolo
packets-col-info = Info
packets-col-size = Tamanho
packets-bytes = { $n } B
packets-kilobytes = { $n } KB
packets-milliseconds = { $n } ms
packets-speed-half = 0,5×
packets-speed-normal = 1×
packets-speed-faster = 1,5×
packets-speed-double = 2×
packets-view-network = Rede
packets-view-sequence = Sequência
packets-view-anatomy = Anatomia
packets-fact-layer = Camada
packets-fact-group = Grupo
packets-no-packet = Nenhum pacote saiu ainda: toque, ou escolha um passo.
packets-no-headers = Os cabeçalhos deste pacote não estão desenhados: a anatomia abre os pacotes de que cada passo trata.
packets-anatomy-of = Cabeçalhos, do mais externo para dentro
packets-value = Valor
packets-bits = { $n } bits
packets-any-length = tamanho variável

## The devices

packets-device-client = Cliente
packets-device-router = Roteador
packets-device-internet = Internet
packets-device-resolver = Resolvedor DNS
packets-device-web-server = Servidor web
packets-device-root-server = Servidor raiz
packets-device-tld-server = Servidor do TLD .com
packets-device-authoritative = Servidor autoritativo
packets-device-server = Servidor

## A protocol's facts

packets-fact-port = Porta padrão
packets-fact-transport = Transporte
packets-fact-format = Formato dos dados
packets-fact-methods = Métodos comuns
packets-fact-defined = Definido em
packets-fact-records = Tipos de registro
packets-fact-header = Tamanho do cabeçalho
packets-fact-flags = Flags
packets-fact-reliability = Confiabilidade

## A step's facts

packets-label-url = URL
packets-label-method = Método
packets-label-next-hop = Próximo salto
packets-label-resolver = Resolvedor
packets-label-transport = Transporte
packets-label-answer = Resposta
packets-label-server-port = Porta do servidor
packets-label-client-port = Porta do cliente
packets-label-handshake = Handshake
packets-label-request = Pedido
packets-label-status = Status
packets-label-content = Conteúdo
packets-label-state = Estado
packets-label-question = Pergunta
packets-label-asks = Pergunta a
packets-label-referral = Indicação
packets-label-ttl = TTL (segundos)
packets-label-cache = Cache
packets-label-flags = Flags
packets-label-seq = Sequência
packets-label-ack = Confirma
packets-label-window = Janela
packets-label-sent = O cliente envia
packets-label-received = O servidor envia

## The shared vocabulary: more devices, labels and facts

packets-device-laptop = Laptop
packets-device-phone = Celular
packets-device-desktop = Computador
packets-device-switch = Switch
packets-device-access-point = Ponto de acesso
packets-device-isp-router = Roteador do provedor
packets-device-transit-router = Roteador de trânsito
packets-device-hosting-router = Roteador da hospedagem
packets-device-their-router = Roteador deles
packets-device-their-laptop = Laptop deles
packets-device-dhcp-server = Servidor DHCP
packets-device-mail-server = Seu servidor de e-mail
packets-device-recipient-server = Servidor de e-mail do destinatário
packets-device-file-server = Servidor de arquivos
packets-device-proxy = Proxy
packets-device-reverse-proxy = Proxy reverso
packets-device-app-server = Servidor de aplicação
packets-device-socks-server = Servidor SOCKS
packets-device-portal = Portal cativo
packets-device-vpn-gateway = Gateway VPN
packets-device-peer = Par
packets-device-stun-server = Servidor STUN
packets-device-turn-server = Retransmissor TURN
packets-device-signalling = Servidor de sinalização
packets-device-radius-server = Servidor RADIUS
packets-device-time-server = Servidor de hora
packets-device-broker = Broker MQTT
packets-device-sensor = Sensor
packets-device-dashboard = Painel
packets-device-plc = CLP
packets-device-scada = Estação SCADA
packets-device-tracker = Tracker
packets-device-guard = Relé de guarda
packets-device-middle = Relé do meio
packets-device-exit = Relé de saída
packets-device-printer = Impressora
packets-device-ssh-server = Servidor SSH
packets-device-attacker = Atacante

packets-label-port = Porta
packets-label-protocol = Protocolo
packets-label-source = Origem
packets-label-destination = Destino
packets-label-address = Endereço
packets-label-mac = Endereço MAC
packets-label-gateway = Gateway
packets-label-lease = Concessão
packets-label-cipher = Cifra
packets-label-key-exchange = Troca de chaves
packets-label-certificate = Certificado
packets-label-signature = Assinatura
packets-label-identity = Identidade
packets-label-topic = Tópico
packets-label-qos = QoS
packets-label-register = Registrador
packets-label-function = Função
packets-label-value = Valor
packets-label-tunnel = Túnel
packets-label-encryption = Criptografia
packets-label-algorithm = Algoritmo
packets-label-nonce = Nonce
packets-label-session = Sessão
packets-label-stream = Fluxo
packets-label-channel = Canal
packets-label-command = Comando
packets-label-reply = Resposta
packets-label-mailbox = Caixa postal
packets-label-user = Usuário
packets-label-path = Caminho
packets-label-peer = Par
packets-label-piece = Pedaço
packets-label-circuit = Circuito
packets-label-record = Registro
packets-label-hop = Salto
packets-label-offset = Desvio
packets-label-delay = Atraso
packets-label-stratum = Estrato
packets-label-version = Versão
packets-label-size = Tamanho
packets-label-mtu = MTU
packets-label-vlan = VLAN
packets-label-route = Rota
packets-label-as-path = Caminho de AS
packets-label-prefix = Prefixo
packets-label-table = Tabela
packets-label-translation = Tradução
packets-label-public = Endereço público
packets-label-private = Endereço privado
packets-label-broadcast = Broadcast
packets-label-multicast = Multicast
packets-label-scope = Escopo
packets-label-mode = Modo
packets-label-resumption = Retomada
packets-label-round-trips = Idas e voltas
packets-label-frame = Quadro
packets-label-opcode = Opcode
packets-label-upgrade = Upgrade
packets-label-candidate = Candidato
packets-label-media = Mídia
packets-label-relay = Retransmissão
packets-label-share = Compartilhamento
packets-label-dialect = Dialeto
packets-label-account = Conta
packets-label-verdict = Veredito
packets-label-trust = Confiança
packets-label-time = Hora
packets-label-interval = Intervalo
packets-label-hash = Hash
packets-label-swarm = Enxame
packets-label-reason = Motivo
packets-label-error = Erro
packets-label-ttl-hops = Saltos restantes (TTL)

packets-fact-addressing = Endereçamento
packets-fact-encryption = Criptografia
packets-fact-authentication = Autenticação
packets-fact-used-by = Usado por
packets-fact-replaces = Substitui
packets-fact-standard = Padrão
packets-fact-security = Segurança
packets-fact-model = Modelo

## The headers, field by field

packets-hdr-ethernet = Ethernet II
packets-f-eth-dst = MAC de destino
packets-f-eth-dst-about = O endereço de enlace da próxima máquina nesta rede — não do destino final, que só o cabeçalho IP diz.
packets-f-eth-src = MAC de origem
packets-f-eth-src-about = O endereço de enlace da máquina que pôs o quadro no fio.
packets-f-eth-type = EtherType
packets-f-eth-type-about = O que o quadro leva: 0x0800 é IPv4, 0x86DD é IPv6, 0x0806 é ARP.

packets-hdr-ipv4 = IPv4
packets-f-ip4-version = Versão
packets-f-ip4-version-about = Sempre 4 num cabeçalho IPv4.
packets-f-ip4-ihl = IHL
packets-f-ip4-ihl-about = O tamanho do cabeçalho em palavras de 32 bits: 5 são os 20 bytes simples, sem opções.
packets-f-ip4-dscp = DSCP
packets-f-ip4-dscp-about = Uma classe de prioridade para os roteadores que a respeitam — a voz pode passar à frente de um download grande.
packets-f-ip4-ecn = ECN
packets-f-ip4-ecn-about = Deixa um roteador congestionado marcar o pacote em vez de descartá-lo.
packets-f-ip4-length = Tamanho total
packets-f-ip4-length-about = O pacote inteiro, cabeçalho e dados, em bytes.
packets-f-ip4-id = Identificação
packets-f-ip4-id-about = Junta os pedaços de um pacote que precisou ser dividido em fragmentos.
packets-f-ip4-flags = Flags
packets-f-ip4-flags-about = 010 é "não fragmente": um roteador que não consegue passar o pacote inteiro o descarta e avisa.
packets-f-ip4-fragment = Deslocamento do fragmento
packets-f-ip4-fragment-about = Onde os dados deste fragmento ficam no pacote original.
packets-f-ip4-ttl = TTL
packets-f-ip4-ttl-about = Tempo de vida: cada roteador o diminui em um e descarta o pacote no zero, para nenhum pacote circular para sempre.
packets-f-ip4-protocol = Protocolo
packets-f-ip4-protocol-about = O que são os dados: 6 é TCP, 17 é UDP, 1 é ICMP.
packets-f-ip4-checksum = Checksum do cabeçalho
packets-f-ip4-checksum-about = Detecta um cabeçalho danificado no caminho; cada roteador o recalcula, já que o TTL muda.
packets-f-ip4-src = Endereço de origem
packets-f-ip4-src-about = Quem enviou o pacote.
packets-f-ip4-dst = Endereço de destino
packets-f-ip4-dst-about = Para quem é o pacote: cada roteador do caminho olha só este campo para escolher para onde mandá-lo.

packets-f-src-port = Porta de origem
packets-f-src-port-about = A porta de quem envia — num cliente, uma temporária que o sistema escolheu para esta conversa.
packets-f-dst-port = Porta de destino
packets-f-dst-port-about = Para qual programa da máquina que recebe são os dados: 53 é DNS, 80 é HTTP, 443 é HTTPS.

packets-hdr-udp = UDP
packets-f-udp-length = Tamanho
packets-f-udp-length-about = O cabeçalho UDP e seus dados, em bytes.
packets-f-udp-checksum = Checksum
packets-f-udp-checksum-about = Detecta dados danificados no caminho. O UDP confere, mas nunca reenvia.

packets-hdr-tcp = TCP
packets-f-tcp-seq = Número de sequência
packets-f-tcp-seq-about = Com qual byte do fluxo este segmento começa. Mostrado a partir do primeiro, como uma captura mostra.
packets-f-tcp-ack = Número de confirmação
packets-f-tcp-ack-about = O próximo byte que quem envia espera do outro lado: tudo antes dele já chegou.
packets-f-tcp-offset = Deslocamento dos dados
packets-f-tcp-offset-about = O tamanho do cabeçalho em palavras de 32 bits: 5 são 20 bytes; mais que isso são opções, como o tamanho máximo de segmento.
packets-f-tcp-reserved = Reservado
packets-f-tcp-reserved-about = Guardado para o futuro, e sempre zero.
packets-f-tcp-cwr = CWR
packets-f-tcp-cwr-about = Janela de congestionamento reduzida: quem envia soube do congestionamento e desacelerou.
packets-f-tcp-ece = ECE
packets-f-tcp-ece-about = Avisa o outro lado que um roteador do caminho marcou congestionamento.
packets-f-tcp-urg = URG
packets-f-tcp-urg-about = Marca dados urgentes. Quase nunca usado hoje.
packets-f-tcp-ackf = ACK
packets-f-tcp-ackf-about = O número de confirmação vale. Ligado em todo segmento depois do primeiro.
packets-f-tcp-psh = PSH
packets-f-tcp-psh-about = Push: entregue estes dados ao programa já, sem esperar mais.
packets-f-tcp-rst = RST
packets-f-tcp-rst-about = Reset: essa conexão não existe, ou acaba de forma abrupta.
packets-f-tcp-syn = SYN
packets-f-tcp-syn-about = Sincronizar: abre uma conexão e leva o número de sequência inicial de quem envia.
packets-f-tcp-fin = FIN
packets-f-tcp-fin-about = Fim: quem envia não tem mais nada a mandar.
packets-f-tcp-window = Janela
packets-f-tcp-window-about = Quantos bytes a mais quem envia consegue receber agora — o freio que impede um remetente rápido de afogar um destinatário lento.
packets-f-tcp-checksum = Checksum
packets-f-tcp-checksum-about = Detecta um segmento danificado no caminho; o danificado é descartado e enviado de novo.
packets-f-tcp-urgent = Ponteiro de urgência
packets-f-tcp-urgent-about = Onde terminam os dados urgentes, quando URG está ligado.

packets-hdr-dns = DNS
packets-f-dns-id = ID
packets-f-dns-id-about = Um número que quem pergunta escolhe; a resposta o traz de volta, para quem perguntou saber a qual pergunta ela responde.
packets-f-dns-qr = QR
packets-f-dns-qr-about = 0 é uma consulta, 1 é uma resposta.
packets-f-dns-opcode = Opcode
packets-f-dns-opcode-about = O tipo de consulta: 0 é a comum.
packets-f-dns-aa = AA
packets-f-dns-aa-about = Resposta autoritativa: o servidor que responde é dono da zona, e é a fonte do registro.
packets-f-dns-tc = TC
packets-f-dns-tc-about = Truncada: a resposta não coube, e deve ser pedida de novo por TCP.
packets-f-dns-rd = RD
packets-f-dns-rd-about = Recursão desejada: "ache a resposta para mim", o que um cliente pede ao seu resolvedor.
packets-f-dns-ra = RA
packets-f-dns-ra-about = Recursão disponível: o servidor procura respostas por você.
packets-f-dns-z = Z
packets-f-dns-z-about = Bits reservados, hoje usados pelo DNSSEC para mais duas flags.
packets-f-dns-rcode = RCODE
packets-f-dns-rcode-about = Como foi: 0 é sem erro, 3 é "esse nome não existe".
packets-f-dns-qdcount = Perguntas
packets-f-dns-qdcount-about = Quantas perguntas vêm a seguir.
packets-f-dns-ancount = Respostas
packets-f-dns-ancount-about = Quantas respostas vêm a seguir.
packets-f-dns-nscount = Autoridade
packets-f-dns-nscount-about = Para quantos servidores de nomes a resposta aponta — uma indicação preenche esta seção.
packets-f-dns-arcount = Adicionais
packets-f-dns-arcount-about = Quantos registros extras vêm a seguir: os endereços desses servidores, ou as opções do EDNS.
packets-f-dns-question = Pergunta
packets-f-dns-question-about = O nome, o tipo de registro e a classe perguntados.
packets-f-dns-answer = Resposta
packets-f-dns-answer-about = Os registros que respondem, cada um com seu TTL: por quantos segundos pode ser guardado.

packets-hdr-http-request = Pedido HTTP
packets-f-http-request-line = Linha de pedido
packets-f-http-request-line-about = O método, o caminho pedido e a versão do HTTP.
packets-f-http-host = Host
packets-f-http-host-about = Qual site é pedido: um servidor, com um endereço, pode servir muitos sites.
packets-f-http-agent = User-Agent
packets-f-http-agent-about = Qual navegador pede.
packets-f-http-accept = Accept
packets-f-http-accept-about = Quais tipos de conteúdo o navegador aceita.
packets-f-http-blank = Linha em branco
packets-f-http-blank-about = Uma linha vazia encerra os cabeçalhos; o corpo, se houver, vem depois dela.

packets-hdr-http-response = Resposta HTTP
packets-f-http-status-line = Linha de status
packets-f-http-status-line-about = A versão e o status: 200 OK, 301 mudou de lugar, 404 não encontrado, 500 erro no servidor.
packets-f-http-type = Content-Type
packets-f-http-type-about = O que é o corpo, para o navegador saber como mostrá-lo.
packets-f-http-length = Content-Length
packets-f-http-length-about = Quantos bytes tem o corpo, para o navegador saber onde ele termina.
packets-f-http-body = Corpo
packets-f-http-body-about = A própria página.

## More headers, field by field

packets-hdr-http-message = Mensagem HTTP
packets-f-httpm-start = Linha inicial
packets-f-httpm-start-about = O método, o caminho e a versão de um pedido, ou a versão e o status de uma resposta.
packets-f-httpm-header = Cabeçalho
packets-f-httpm-header-about = Um nome, dois-pontos e um valor, um por linha: Host, Content-Type, Cookie e muitos outros.
packets-f-httpm-body = Corpo
packets-f-httpm-body-about = O que a mensagem leva, se leva algo — uma página, uma imagem, os dados de um formulário —; o Content-Length, ou os pedaços em que ela é enviada, dizem onde termina.

packets-hdr-line = Linha de texto
packets-f-line-line = Linha
packets-f-line-line-about = Um comando ou uma resposta, em texto puro terminado por CR LF: um verbo e seus argumentos, ou um status e algumas palavras para gente.

packets-hdr-arp = ARP
packets-f-arp-htype = Tipo de hardware
packets-f-arp-htype-about = O tipo de enlace: 1 é Ethernet.
packets-f-arp-ptype = Tipo de protocolo
packets-f-arp-ptype-about = O tipo de endereço perguntado: 0x0800 é IPv4.
packets-f-arp-hlen = Tamanho do hardware
packets-f-arp-hlen-about = Quantos bytes tem um endereço de enlace: 6 para um MAC.
packets-f-arp-plen = Tamanho do protocolo
packets-f-arp-plen-about = Quantos bytes tem um endereço do protocolo: 4 para IPv4.
packets-f-arp-opcode = Opcode
packets-f-arp-opcode-about = 1 é um pedido, "quem tem este endereço?"; 2 é uma resposta, "está neste MAC".
packets-f-arp-sha = MAC do remetente
packets-f-arp-sha-about = O MAC da máquina que envia esta mensagem.
packets-f-arp-spa = IP do remetente
packets-f-arp-spa-about = O endereço IPv4 de quem envia; quem recebe anota o par no seu cache ARP e se poupa de uma pergunta.
packets-f-arp-tha = MAC do alvo
packets-f-arp-tha-about = O MAC que se procura: só zeros num pedido, já que é ele que se pergunta; numa resposta, o de quem perguntou.
packets-f-arp-tpa = IP do alvo
packets-f-arp-tpa-about = O endereço IPv4 cujo MAC se pergunta; numa resposta, o de quem perguntou.

packets-hdr-ipv6 = IPv6
packets-f-ip6-version = Versão
packets-f-ip6-version-about = Sempre 6 num cabeçalho IPv6.
packets-f-ip6-class = Classe de tráfego
packets-f-ip6-class-about = O DSCP e o ECN do IPv4 num byte só: uma classe de prioridade, e dois bits que um roteador congestionado pode marcar em vez de descartar o pacote.
packets-f-ip6-flow = Rótulo de fluxo
packets-f-ip6-flow-about = Marca os pacotes de um mesmo fluxo, para que os roteadores que dividem o tráfego entre vários caminhos os mantenham todos no mesmo.
packets-f-ip6-length = Tamanho do payload
packets-f-ip6-length-about = O que vem depois do cabeçalho de 40 bytes, em bytes — ao contrário do IPv4, o próprio cabeçalho não entra na conta.
packets-f-ip6-next = Próximo cabeçalho
packets-f-ip6-next-about = O que vem a seguir: 6 é TCP, 17 é UDP, 58 é ICMPv6, ou um cabeçalho de extensão que diz, por sua vez, o que vem depois dele.
packets-f-ip6-hops = Limite de saltos
packets-f-ip6-hops-about = O TTL do IPv4 com um nome honesto: cada roteador o diminui em um e descarta o pacote no zero.
packets-f-ip6-src = Endereço de origem
packets-f-ip6-src-about = Quem enviou o pacote: 128 bits, escritos em grupos de dígitos hexadecimais, como 2001:db8::1.
packets-f-ip6-dst = Endereço de destino
packets-f-ip6-dst-about = Para quem é o pacote — o campo que cada roteador do caminho lê para escolher o próximo salto.

packets-hdr-icmp = ICMP
packets-f-icmp-type = Tipo
packets-f-icmp-type-about = O que é a mensagem: 8 é um echo request, 0 a sua resposta, 3 destino inalcançável, 11 tempo excedido.
packets-f-icmp-code = Código
packets-f-icmp-code-about = Um detalhe do tipo: em destino inalcançável, 1 é host inalcançável e 3 é porta inalcançável.
packets-f-icmp-checksum = Checksum
packets-f-icmp-checksum-about = Detecta uma mensagem danificada no caminho; cobre a mensagem ICMP inteira.
packets-f-icmp-id = Identificador
packets-f-icmp-id-about = Liga as respostas ao programa que perguntou, para que dois pings rodando ao mesmo tempo não misturem as respostas.
packets-f-icmp-seq = Número de sequência
packets-f-icmp-seq-about = Conta os echo requests um a um, para que uma resposta perdida ou atrasada apareça.
packets-f-icmp-data = Dados
packets-f-icmp-data-about = O que quem perguntou pôs ali, devolvido sem mudança; uma mensagem de erro leva no lugar o começo do pacote que a causou.

packets-hdr-ndp = NDP
packets-f-ndp-type = Tipo
packets-f-ndp-type-about = 135 é uma Neighbor Solicitation, "quem tem este endereço?"; 136 uma Neighbor Advertisement, "sou eu".
packets-f-ndp-code = Código
packets-f-ndp-code-about = Sempre 0 na descoberta de vizinhos.
packets-f-ndp-checksum = Checksum
packets-f-ndp-checksum-about = Cobre a mensagem e os endereços IPv6 em volta dela, para que uma mensagem entregue no lugar errado seja detectada.
packets-f-ndp-flags = Flags
packets-f-ndp-flags-about = Num anúncio, R (quem envia é roteador), S (responde a uma solicitação) e O (substitua o que está no cache); tudo zero numa solicitação.
packets-f-ndp-target = Endereço alvo
packets-f-ndp-target-about = O endereço IPv6 cujo endereço de enlace se pergunta, ou se informa.
packets-f-ndp-options = Opções
packets-f-ndp-options-about = O próprio endereço de enlace: o de quem envia numa solicitação, o do alvo num anúncio.

packets-hdr-tls = Registro TLS
packets-f-tls-type = Tipo de conteúdo
packets-f-tls-type-about = O que o registro leva: 22 é uma mensagem de handshake, 23 dados da aplicação, 21 um alerta.
packets-f-tls-version = Versão legada
packets-f-tls-version-about = 0x0303, o número do TLS 1.2, mantido para que middleboxes antigos deixem o registro passar (um primeiro ClientHello pode dizer 0x0301, o do TLS 1.0); a versão de fato falada é combinada numa extensão.
packets-f-tls-length = Tamanho
packets-f-tls-length-about = Quantos bytes do registro vêm a seguir; um registro leva no máximo 16 KiB de dados.
packets-f-tls-hs-type = Tipo de handshake
packets-f-tls-hs-type-about = Qual mensagem do handshake: 1 é ClientHello, 2 ServerHello, 11 Certificate, 15 CertificateVerify, 20 Finished.
packets-f-tls-hs-length = Tamanho do handshake
packets-f-tls-hs-length-about = O tamanho da própria mensagem de handshake, em bytes; uma longa, como uma cadeia de certificados, pode ocupar vários registros.
packets-f-tls-body = Corpo
packets-f-tls-body-about = A própria mensagem — num ClientHello, um número aleatório, as cipher suites oferecidas e extensões como o nome do servidor e os key shares.

packets-hdr-key-share = Key share
packets-f-ks-group = Grupo
packets-f-ks-group-about = Para qual troca de chaves é o share: 0x11EC é X25519MLKEM768, um híbrido pós-quântico; 0x001D é o X25519 puro.
packets-f-ks-length = Tamanho
packets-f-ks-length-about = Quantos bytes tem o share: 1.216 no share híbrido de um cliente, 1.120 no de um servidor.
packets-f-ks-mlkem = Share ML-KEM
packets-f-ks-mlkem-about = A metade pós-quântica: a chave pública ML-KEM-768 do cliente, ou o texto cifrado do servidor, que só o dono daquela chave consegue abrir.
packets-f-ks-x25519 = Share X25519
packets-f-ks-x25519-about = A metade clássica, uma chave pública de curva elíptica de 32 bytes; o segredo combinado resiste enquanto qualquer uma das metades resistir.

packets-hdr-quic-long = QUIC, cabeçalho longo
packets-f-ql-form = Forma do cabeçalho
packets-f-ql-form-about = 1: um cabeçalho longo, usado enquanto a conexão é estabelecida; 0 é o curto, para tudo o que vem depois.
packets-f-ql-fixed = Bit fixo
packets-f-ql-fixed-about = Sempre 1, o que deixa quem recebe distinguir o QUIC de outros protocolos na mesma porta.
packets-f-ql-type = Tipo de pacote
packets-f-ql-type-about = 0 é Initial, 1 0-RTT, 2 Handshake, 3 Retry.
packets-f-ql-reserved = Reservado
packets-f-ql-reserved-about = Dois bits guardados para o futuro, zero — e, como o campo seguinte, escondidos pela proteção de cabeçalho.
packets-f-ql-pnlen = Tamanho do número do pacote
packets-f-ql-pnlen-about = Quantos bytes o número do pacote ocupa, menos um; cifrado pela proteção de cabeçalho, para que ninguém no caminho o leia.
packets-f-ql-version = Versão
packets-f-ql-version-about = Qual versão do QUIC: 1 é a da RFC 9000; um servidor que não a fala responde com as versões que fala.
packets-f-ql-dcil = Tamanho do DCID
packets-f-ql-dcil-about = Quantos bytes tem o ID de conexão de destino: até 20.
packets-f-ql-dcid = ID de conexão de destino
packets-f-ql-dcid-about = Identifica a conexão para quem recebe — não o endereço e a porta —, e por isso uma conexão sobrevive a uma troca de rede.
packets-f-ql-scil = Tamanho do SCID
packets-f-ql-scil-about = Quantos bytes tem o ID de conexão de origem.
packets-f-ql-scid = ID de conexão de origem
packets-f-ql-scid-about = O ID pelo qual quem envia quer ser encontrado: o outro lado o põe nos pacotes que manda de volta.
packets-f-ql-length = Tamanho
packets-f-ql-length-about = Quantos bytes vêm a seguir, número do pacote e payload, para que vários pacotes QUIC possam dividir um mesmo datagrama UDP.
packets-f-ql-pn = Número do pacote
packets-f-ql-pn-about = Conta os pacotes de quem envia e nunca se repete, nem para um frame reenviado, para que uma confirmação nunca seja ambígua.
packets-f-ql-payload = Payload
packets-f-ql-payload-about = Os frames, cifrados: frames CRYPTO levam o handshake do TLS, frames ACK confirmam, e PADDING completa o primeiro Initial de um cliente até 1.200 bytes.

packets-hdr-quic-initial = QUIC, pacote Initial
packets-f-ql-token-length = Tamanho do token
packets-f-ql-token-length-about = Quantos bytes de token vêm a seguir, um token que o servidor deu antes para atestar o endereço do cliente; aqui é 0, então não vem token nenhum.

packets-hdr-quic-short = QUIC, cabeçalho curto
packets-f-qs-form = Forma do cabeçalho
packets-f-qs-form-about = 0: um cabeçalho curto, o que todo pacote usa depois do handshake.
packets-f-qs-fixed = Bit fixo
packets-f-qs-fixed-about = Sempre 1, como no cabeçalho longo.
packets-f-qs-spin = Spin bit
packets-f-qs-spin-about = Inverte uma vez a cada ida e volta, para que um operador no caminho meça o atraso sem ler mais nada; uma ponta pode deixá-lo desligado.
packets-f-qs-reserved = Reservado
packets-f-qs-reserved-about = Dois bits guardados para o futuro, zero, e escondidos pela proteção de cabeçalho.
packets-f-qs-phase = Fase da chave
packets-f-qs-phase-about = Qual geração de chaves cifrou o pacote: inverte a cada vez que as pontas trocam de chave no meio da conexão.
packets-f-qs-pnlen = Tamanho do número do pacote
packets-f-qs-pnlen-about = Quantos bytes o número do pacote ocupa, menos um — escondido pela proteção de cabeçalho.
packets-f-qs-dcid = ID de conexão de destino
packets-f-qs-dcid-about = O ID de conexão, sem o tamanho: foi quem recebe que o escolheu, então sabe quanto ele mede.
packets-f-qs-pn = Número do pacote
packets-f-qs-pn-about = Conta os pacotes de quem envia; só os últimos bytes vão no fio, e quem recebe deduz o resto.
packets-f-qs-payload = Payload
packets-f-qs-payload-about = Os frames, cifrados: frames STREAM com os dados da aplicação, frames ACK e os demais.

packets-hdr-wireguard-init = WireGuard, início do handshake
packets-f-wgi-type = Tipo
packets-f-wgi-type-about = 1 é o início de um handshake, 2 a resposta, 3 uma resposta com cookie, 4 dados de transporte.
packets-f-wgi-reserved = Reservado
packets-f-wgi-reserved-about = Três bytes zero, para que os campos seguintes caiam em fronteiras de 4 bytes.
packets-f-wgi-sender = Índice de quem envia
packets-f-wgi-sender-about = Um número que o iniciador escolheu para esta sessão; o par o põe em todo pacote que manda de volta, e a sessão é achada sem olhar endereços.
packets-f-wgi-ephemeral = Chave efêmera
packets-f-wgi-ephemeral-about = Uma chave pública Curve25519 nova, feita só para este handshake e jogada fora depois: é o que dá o forward secrecy.
packets-f-wgi-static = Chave estática cifrada
packets-f-wgi-static-about = A chave pública de longo prazo do iniciador, cifrada com uma tag de 16 bytes, para que ninguém no caminho saiba quem está se conectando.
packets-f-wgi-timestamp = Timestamp cifrado
packets-f-wgi-timestamp-about = A hora atual, cifrada: um par recusa um timestamp que não seja mais novo que o último, então um handshake gravado não pode ser repetido.
packets-f-wgi-mac1 = MAC1
packets-f-wgi-mac1-about = Um hash com chave sobre a mensagem, feito com a chave pública de quem responde: quem não conhece essa chave não recebe resposta nenhuma.
packets-f-wgi-mac2 = MAC2
packets-f-wgi-mac2-about = Zero, a não ser que quem responde esteja sobrecarregado e tenha mandado um cookie: aí, um hash com chave feito com ele, que prova que o endereço do iniciador é real.

packets-hdr-wireguard-data = WireGuard, dados de transporte
packets-f-wgd-type = Tipo
packets-f-wgd-type-about = 4 são dados de transporte: um pacote IP, cifrado. 1 a 3 são as mensagens do handshake.
packets-f-wgd-reserved = Reservado
packets-f-wgd-reserved-about = Três bytes zero, para que os campos seguintes caiam em fronteiras de 4 bytes.
packets-f-wgd-receiver = Índice de quem recebe
packets-f-wgd-receiver-about = O índice que o par que recebe escolheu no handshake: ele acha as chaves da sessão só por esse número.
packets-f-wgd-counter = Contador
packets-f-wgd-counter-about = Conta os pacotes enviados com estas chaves e é o nonce da cifra deles; um contador já visto é recusado, o que impede repetições.
packets-f-wgd-packet = Pacote cifrado
packets-f-wgd-packet-about = O pacote IP de dentro, completado até um múltiplo de 16 bytes e cifrado com ChaCha20-Poly1305, seguido da sua tag de 16 bytes.

packets-hdr-esp = ESP (IPsec)
packets-f-esp-spi = SPI
packets-f-esp-spi-about = Security parameter index: diz a quem recebe qual associação de segurança — quais chaves e algoritmos — protege o pacote.
packets-f-esp-seq = Número de sequência
packets-f-esp-seq-about = Conta os pacotes enviados nesta associação; quem recebe recusa um que já viu.
packets-f-esp-payload = Payload cifrado
packets-f-esp-payload-about = Os dados protegidos: no modo túnel, um pacote IP inteiro; no modo transporte, só o que o pacote original levava.
packets-f-esp-padlen = Tamanho do preenchimento
packets-f-esp-padlen-about = Cifrado junto com o payload: quantos bytes de preenchimento vieram antes dele, para que quem recebe os retire.
packets-f-esp-next = Próximo cabeçalho
packets-f-esp-next-about = O que é o payload, cifrado também: 4 é um pacote IPv4 no modo túnel, 41 um IPv6, 6 um segmento TCP.
packets-f-esp-icv = ICV
packets-f-esp-icv-about = Integrity check value: a tag que prova que o pacote veio do par sem mudança — 16 bytes com AES-GCM.

packets-hdr-ike = IKEv2
packets-f-ike-ispi = SPI do iniciador
packets-f-ike-ispi-about = Um número que o iniciador escolheu; junto com o de quem responde, identifica esta sessão IKE.
packets-f-ike-rspi = SPI de quem responde
packets-f-ike-rspi-about = A metade do nome que cabe a quem responde: zero na primeiríssima mensagem, antes de ele ter respondido.
packets-f-ike-next = Próximo payload
packets-f-ike-next-about = O tipo do primeiro payload: 33 é SA (os algoritmos oferecidos), 34 uma troca de chaves, 46 o payload cifrado e autenticado.
packets-f-ike-version = Versão
packets-f-ike-version-about = 0x20: IKE versão 2.0, com a versão principal nos quatro bits mais altos.
packets-f-ike-exchange = Tipo de troca
packets-f-ike-exchange-about = 34 é IKE_SA_INIT (combinar algoritmos e chaves), 35 IKE_AUTH (provar quem você é), 36 CREATE_CHILD_SA, 37 INFORMATIONAL.
packets-f-ike-flags = Flags
packets-f-ike-flags-about = Se quem envia foi quem começou a sessão, e se a mensagem é uma resposta.
packets-f-ike-msgid = ID da mensagem
packets-f-ike-msgid-about = Conta os pedidos; uma resposta leva o número do seu pedido, para que uma mensagem perdida ou repetida seja percebida.
packets-f-ike-length = Tamanho
packets-f-ike-length-about = A mensagem inteira, cabeçalho e payloads, em bytes.
packets-f-ike-payloads = Payloads
packets-f-ike-payloads-about = Uma corrente de payloads, cada um dizendo qual vem depois: ofertas, key shares, nonces e, depois da primeira troca, todo o resto, cifrado.

packets-hdr-ntp = NTP
packets-f-ntp-li = LI
packets-f-ntp-li-about = Indicador de segundo bissexto: avisa que o último minuto do dia terá 61 ou 59 segundos; 3 quer dizer que o relógio não está sincronizado.
packets-f-ntp-vn = VN
packets-f-ntp-vn-about = A versão do NTP: 4 hoje.
packets-f-ntp-mode = Modo
packets-f-ntp-mode-about = 3 é um cliente perguntando, 4 um servidor respondendo, 5 um broadcast.
packets-f-ntp-stratum = Stratum
packets-f-ntp-stratum-about = A distância até um relógio de referência: 1 é um servidor alimentado por um relógio atômico ou GPS, 2 um que pergunta a um stratum 1, e assim por diante.
packets-f-ntp-poll = Poll
packets-f-ntp-poll-about = A maior espera entre mensagens, como potência de dois em segundos: 6 são 64 segundos.
packets-f-ntp-precision = Precisão
packets-f-ntp-precision-about = O quão fino é o relógio de quem envia, como potência de dois em segundos: −20 é cerca de um microssegundo.
packets-f-ntp-rootdelay = Atraso até a raiz
packets-f-ntp-rootdelay-about = A ida e volta até o relógio de referência no topo da cadeia.
packets-f-ntp-rootdisp = Dispersão até a raiz
packets-f-ntp-rootdisp-about = Quanto erro pode ter se acumulado na descida desde o relógio de referência.
packets-f-ntp-refid = ID de referência
packets-f-ntp-refid-about = Qual fonte o servidor segue: no stratum 1 um código como GPS, nos outros o endereço do servidor acima dele.
packets-f-ntp-refts = Timestamp de referência
packets-f-ntp-refts-about = Quando o relógio do servidor foi acertado ou corrigido pela última vez.
packets-f-ntp-origin = Timestamp de origem
packets-f-ntp-origin-about = A hora de envio do cliente, copiada de volta: diz ao cliente a qual pedido isto responde.
packets-f-ntp-receive = Timestamp de recebimento
packets-f-ntp-receive-about = Quando o pedido chegou ao servidor, pelo relógio do servidor.
packets-f-ntp-transmit = Timestamp de envio
packets-f-ntp-transmit-about = Quando o pacote saiu de quem o enviou; com as horas de origem e de recebimento e o instante em que a resposta chega, o cliente calcula o atraso e o desvio do seu relógio.

packets-hdr-dhcp = DHCP
packets-f-dhcp-op = Op
packets-f-dhcp-op-about = 1 é um pedido de um cliente, 2 uma resposta de um servidor; qual mensagem é, uma opção diz.
packets-f-dhcp-htype = Tipo de hardware
packets-f-dhcp-htype-about = O tipo de enlace: 1 é Ethernet.
packets-f-dhcp-hlen = Tamanho do hardware
packets-f-dhcp-hlen-about = Quantos bytes tem o endereço de hardware: 6 para um MAC.
packets-f-dhcp-hops = Saltos
packets-f-dhcp-hops-about = Por quantos relays a mensagem passou; um cliente envia 0.
packets-f-dhcp-xid = ID da transação
packets-f-dhcp-xid-about = Um número aleatório que o cliente escolhe; toda resposta o traz de volta, para o cliente saber que a resposta é para ele.
packets-f-dhcp-secs = Segundos
packets-f-dhcp-secs-about = Há quanto tempo o cliente começou a pedir um endereço.
packets-f-dhcp-flags = Flags
packets-f-dhcp-flags-about = O bit de broadcast: ligado por um cliente que não consegue receber uma resposta enviada a um endereço que ele ainda não tem.
packets-f-dhcp-ciaddr = Endereço do cliente
packets-f-dhcp-ciaddr-about = O endereço atual do cliente, quando ele renova um que já tem; zero nos outros casos.
packets-f-dhcp-yiaddr = Seu endereço
packets-f-dhcp-yiaddr-about = O endereço que o servidor oferece ao cliente, ou confirma.
packets-f-dhcp-siaddr = Endereço do servidor
packets-f-dhcp-siaddr-about = O próximo servidor que o cliente deve usar para dar boot pela rede, se houver.
packets-f-dhcp-giaddr = Endereço do relay
packets-f-dhcp-giaddr-about = O relay que repassou a mensagem, se houver: o servidor responde por ele, com um endereço da rede do relay.
packets-f-dhcp-chaddr = Endereço de hardware do cliente
packets-f-dhcp-chaddr-about = O MAC do cliente, completado até 16 bytes: é como o servidor o reconhece.
packets-f-dhcp-options = Opções
packets-f-dhcp-options-about = Depois de um magic cookie, uma lista de opções: o tipo da mensagem (DISCOVER, OFFER, REQUEST, ACK), a máscara de sub-rede, o roteador, os servidores DNS, o tempo de lease.

packets-hdr-mqtt = MQTT
packets-f-mqtt-type = Tipo de pacote
packets-f-mqtt-type-about = O que é o pacote: 1 CONNECT, 2 CONNACK, 3 PUBLISH, 8 SUBSCRIBE, 9 SUBACK, 12 PINGREQ.
packets-f-mqtt-flags = Flags
packets-f-mqtt-flags-about = Num PUBLISH, DUP (um reenvio), o nível de QoS e RETAIN (o broker guarda a mensagem para quem assinar depois); valores fixos nos outros tipos.
packets-f-mqtt-remaining = Tamanho restante
packets-f-mqtt-remaining-about = Quantos bytes vêm a seguir, em 1 a 4 bytes de 7 bits cada, para que uma mensagem pequena gaste um byte só com isso.
packets-f-mqtt-variable = Cabeçalho variável
packets-f-mqtt-variable-about = Depende do tipo: o nome do tópico e o identificador do pacote num PUBLISH, o nome do protocolo, as flags e o keep-alive num CONNECT.
packets-f-mqtt-payload = Payload
packets-f-mqtt-payload-about = Num PUBLISH, a própria mensagem, que o broker nunca abre; num SUBSCRIBE, os filtros de tópico.

packets-hdr-mbap = Modbus TCP
packets-f-mbap-transaction = ID da transação
packets-f-mbap-transaction-about = Um número que o cliente escolhe e o servidor copia na resposta, para que vários pedidos possam estar a caminho ao mesmo tempo.
packets-f-mbap-protocol = ID do protocolo
packets-f-mbap-protocol-about = Sempre 0, para Modbus.
packets-f-mbap-length = Tamanho
packets-f-mbap-length-about = Quantos bytes vêm a seguir: o unit ID, o código de função e os dados.
packets-f-mbap-unit = Unit ID
packets-f-mbap-unit-about = Para qual dispositivo é o pedido, quando um gateway o repassa a dispositivos numa linha serial.
packets-f-mbap-function = Código de função
packets-f-mbap-function-about = O que fazer: 3 lê holding registers, 6 escreve um registrador, 16 escreve vários; uma resposta com o bit mais alto ligado, como 0x83, é um erro.
packets-f-mbap-data = Dados
packets-f-mbap-data-about = Os endereços e as quantidades pedidas, ou os valores lidos; num erro, o código de exceção.

packets-hdr-websocket = Frame WebSocket
packets-f-ws-fin = FIN
packets-f-ws-fin-about = 1 marca o último frame de uma mensagem; uma mensagem longa pode ir em vários.
packets-f-ws-rsv = RSV
packets-f-ws-rsv-about = Três bits para extensões, como a compressão por mensagem; zero sem nenhuma.
packets-f-ws-opcode = Opcode
packets-f-ws-opcode-about = 1 é texto, 2 binário, 8 close, 9 ping, 10 pong; 0 continua uma mensagem.
packets-f-ws-mask = MASK
packets-f-ws-mask-about = Ligado em todo frame de um cliente para um servidor, e nunca no sentido contrário.
packets-f-ws-length = Tamanho do payload
packets-f-ws-length-about = O tamanho do payload até 125 bytes; 126 e 127 dizem que um tamanho maior vem a seguir, em 2 ou 8 bytes.
packets-f-ws-key = Chave de máscara
packets-f-ws-key-about = Quatro bytes aleatórios que o cliente mistura ao payload, para que um proxy confuso no caminho não seja enganado a guardar em cache o que ele leva.
packets-f-ws-payload = Payload
packets-f-ws-payload-about = A mensagem: texto em UTF-8 ou dados binários, mascarados quando vêm do cliente.

packets-hdr-bittorrent = Handshake do BitTorrent
packets-f-bt-pstrlen = Tamanho do nome
packets-f-bt-pstrlen-about = O tamanho do campo seguinte: 19.
packets-f-bt-pstr = Nome do protocolo
packets-f-bt-pstr-about = Os 19 caracteres de "BitTorrent protocol", para cada lado saber com quem está falando.
packets-f-bt-reserved = Reservado
packets-f-bt-reserved-about = Oito bytes cujos bits anunciam extensões, como o protocolo de extensões ou a DHT.
packets-f-bt-hash = Info hash
packets-f-bt-hash-about = O hash SHA-1 do dicionário info do torrent: por qual torrent os dois peers estão ali.
packets-f-bt-peer = Peer ID
packets-f-bt-peer-about = 20 bytes que um cliente escolhe para se identificar no swarm, muitas vezes começando pelo nome e pela versão dele.

packets-hdr-wifi = Quadro Wi-Fi (802.11)
packets-f-wifi-fc = Controle do quadro
packets-f-wifi-fc-about = O tipo e o subtipo do quadro — um beacon, uma autenticação, dados — e flags: para ou do access point, retransmissão, economia de energia, protegido.
packets-f-wifi-duration = Duração
packets-f-wifi-duration-about = Por quantos microssegundos o ar vai ficar ocupado, para que as outras estações esperem antes de transmitir.
packets-f-wifi-addr1 = Endereço 1
packets-f-wifi-addr1-about = O receptor: a estação que deve pegar este quadro no ar.
packets-f-wifi-addr2 = Endereço 2
packets-f-wifi-addr2-about = O transmissor: a estação que o enviou pelo ar.
packets-f-wifi-addr3 = Endereço 3
packets-f-wifi-addr3-about = O sentido depende da direção: em geral o BSSID, o MAC do access point, ou a origem ou o destino final além dele.
packets-f-wifi-seq = Controle de sequência
packets-f-wifi-seq-about = O número do quadro e o do fragmento, para que quem recebe descarte um quadro que chegou duas vezes.

packets-hdr-eapol = EAPOL-Key
packets-f-eapol-version = Versão
packets-f-eapol-version-about = A versão do protocolo 802.1X, em geral 2.
packets-f-eapol-type = Tipo
packets-f-eapol-type-about = 3 é uma mensagem de chave, a que o four-way handshake usa; 0 leva EAP.
packets-f-eapol-length = Tamanho
packets-f-eapol-length-about = Quantos bytes de corpo vêm a seguir.
packets-f-eapol-descriptor = Tipo de descritor
packets-f-eapol-descriptor-about = 2: um descritor de chave 802.11, como o WPA2 e o WPA3 usam.
packets-f-eapol-info = Informação da chave
packets-f-eapol-info-about = Bits que distinguem as quatro mensagens: se há MIC, se uma resposta é esperada, se a chave deve ser instalada, se há uma chave de grupo dentro.
packets-f-eapol-keylen = Tamanho da chave
packets-f-eapol-keylen-about = Quantos bytes tem a chave da cifra par a par: 16 para CCMP.
packets-f-eapol-replay = Contador de replay
packets-f-eapol-replay-about = Cresce a cada mensagem que o access point envia; a estação recusa uma antiga, então mensagens gravadas não podem ser repetidas.
packets-f-eapol-nonce = Nonce da chave
packets-f-eapol-nonce-about = Um número aleatório novo: o do access point na mensagem 1, o da estação na mensagem 2; os dois entram na chave da sessão.
packets-f-eapol-mic = MIC da chave
packets-f-eapol-mic-about = Uma verificação com chave sobre a mensagem, feita com uma chave recém-derivada da PMK: prova que quem envia tem a mesma PMK, sem enviá-la.
packets-f-eapol-data = Dados da chave
packets-f-eapol-data-about = Na mensagem 3, a chave de grupo para o tráfego de broadcast, cifrada; na mensagem 2, as escolhas de segurança da estação.

packets-hdr-sae = SAE commit (WPA3)
packets-f-sae-algorithm = Algoritmo de autenticação
packets-f-sae-algorithm-about = 3 é SAE, a troca de senha do WPA3; 0 é a velha autenticação aberta.
packets-f-sae-seq = Sequência da transação
packets-f-sae-seq-about = 1 é o commit, 2 o confirm; cada lado envia os dois.
packets-f-sae-status = Status
packets-f-sae-status-about = 0 é sucesso; outros valores recusam, ou pedem um token anti-clogging ou outro grupo.
packets-f-sae-group = Grupo
packets-f-sae-group-about = Qual curva elíptica: 19 é a curva de 256 bits do NIST, a P-256.
packets-f-sae-scalar = Escalar
packets-f-sae-scalar-about = Um número feito de dois segredos aleatórios novos, diferente a cada vez; com o elemento, compromete o lado com a senha.
packets-f-sae-element = Elemento
packets-f-sae-element-about = Um ponto da curva, feito da senha e de um desses segredos; quem escuta não consegue testar palpites contra ele, o que acaba com o ataque de dicionário offline que o WPA2 permitia.

packets-hdr-radius = RADIUS
packets-f-radius-code = Código
packets-f-radius-code-about = 1 é Access-Request, 2 Access-Accept, 3 Access-Reject, 11 Access-Challenge.
packets-f-radius-id = Identificador
packets-f-radius-id-about = Liga uma resposta ao seu pedido, e deixa o servidor perceber um pedido enviado de novo.
packets-f-radius-length = Tamanho
packets-f-radius-length-about = O pacote inteiro em bytes, atributos incluídos.
packets-f-radius-auth = Authenticator
packets-f-radius-auth-about = Num Access-Request, 16 bytes aleatórios; num Accounting-Request, um hash sobre o pacote e o segredo compartilhado; numa resposta, um hash sobre a resposta, o authenticator do pedido e o segredo compartilhado, que prova que o servidor conhece o segredo.
packets-f-radius-attrs = Atributos
packets-f-radius-attrs-about = Tipo, tamanho e valor, um atrás do outro: o nome do usuário, a senha escondida com o segredo compartilhado, uma mensagem EAP e, numa resposta, a VLAN ou o tempo limite da sessão.

packets-hdr-socks5 = SOCKS5
packets-f-socks-version = Versão
packets-f-socks-version-about = Sempre 5.
packets-f-socks-command = Comando
packets-f-socks-command-about = 1 é CONNECT (abrir uma conexão TCP), 2 BIND (esperar uma), 3 UDP ASSOCIATE (repassar datagramas); numa resposta, o status, 0 para sucesso.
packets-f-socks-reserved = Reservado
packets-f-socks-reserved-about = Sempre zero.
packets-f-socks-atyp = Tipo de endereço
packets-f-socks-atyp-about = O que é o endereço: 1 um endereço IPv4, 3 um nome de domínio, 4 um endereço IPv6.
packets-f-socks-address = Endereço
packets-f-socks-address-about = Onde o proxy deve se conectar; um nome de domínio deixa o proxy, e não o cliente, resolvê-lo, e o DNS do cliente não fica sabendo de nada.
packets-f-socks-port = Porta
packets-f-socks-port-about = A porta a que o proxy deve se conectar nesse endereço: 443 para HTTPS.

packets-hdr-bgp = BGP
packets-f-bgp-marker = Marcador
packets-f-bgp-marker-about = Dezesseis bytes todos em um, mantidos por compatibilidade: marcam onde cada mensagem começa no fluxo TCP.
packets-f-bgp-length = Tamanho
packets-f-bgp-length-about = A mensagem inteira em bytes, cabeçalho incluído: no mínimo 19.
packets-f-bgp-type = Tipo
packets-f-bgp-type-about = 1 é OPEN, 2 UPDATE, 3 NOTIFICATION (um erro, depois do qual a sessão fecha), 4 KEEPALIVE.
packets-f-bgp-message = Mensagem
packets-f-bgp-message-about = O corpo: num OPEN, o número do AS e o hold time; num UPDATE, as rotas retiradas, os atributos do caminho e os prefixos anunciados.

packets-hdr-tor-cell = Célula do Tor
packets-f-tor-circuit = ID do circuito
packets-f-tor-circuit-about = Identifica o circuito só neste enlace: no enlace seguinte, o mesmo circuito tem outro número.
packets-f-tor-command = Comando
packets-f-tor-command-about = O que é a célula: 3 RELAY, 4 DESTROY, 10 CREATE2, 11 CREATED2.
packets-f-tor-payload = Payload
packets-f-tor-payload-about = Sempre 509 bytes, para que o tamanho de uma célula não entregue nada; o payload de uma célula RELAY vai embrulhado numa camada de cifra por relay.

packets-hdr-ssh = Pacote SSH
packets-f-ssh-length = Tamanho do pacote
packets-f-ssh-length-about = Quantos bytes vêm a seguir até o MAC: o tamanho do preenchimento, o payload e o preenchimento.
packets-f-ssh-padlen = Tamanho do preenchimento
packets-f-ssh-padlen-about = Quantos bytes de preenchimento aleatório vêm depois do payload.
packets-f-ssh-payload = Payload
packets-f-ssh-payload-about = A própria mensagem — uma troca de chaves, os dados de um canal, um pedido —, cujo primeiro byte é o tipo, como 20 para KEXINIT.
packets-f-ssh-padding = Preenchimento
packets-f-ssh-padding-about = Pelo menos 4 bytes aleatórios, que levam o pacote a um múltiplo do tamanho de bloco da cifra.
packets-f-ssh-mac = MAC
packets-f-ssh-mac-about = A tag que prova que o pacote não foi alterado; com AES-GCM ou ChaCha20-Poly1305 ela vem da própria cifra, e antes da primeira troca de chaves fica vazia.

packets-hdr-smb2 = SMB2
packets-f-smb-protocol = ID do protocolo
packets-f-smb-protocol-about = O byte 0xFE, depois SMB: marca a mensagem como SMB 2 ou 3.
packets-f-smb-size = Tamanho da estrutura
packets-f-smb-size-about = Sempre 64, o tamanho do próprio cabeçalho.
packets-f-smb-charge = Custo em créditos
packets-f-smb-charge-about = Quantos créditos o pedido custa; uma leitura ou escrita grande custa mais de um.
packets-f-smb-status = Status
packets-f-smb-status-about = Numa resposta, como foi, num código de status NT: 0 é sucesso, 0xC0000022 acesso negado.
packets-f-smb-command = Comando
packets-f-smb-command-about = O que se pede: 0 NEGOTIATE, 1 SESSION_SETUP, 3 TREE_CONNECT, 5 CREATE (abrir um arquivo), 8 READ, 9 WRITE.
packets-f-smb-credits = Créditos
packets-f-smb-credits-about = Solicitados num pedido, concedidos numa resposta: os créditos limitam quantos pedidos um cliente pode ter em andamento.
packets-f-smb-flags = Flags
packets-f-smb-flags-about = Se é uma resposta, se está assinada e se está encadeada ao comando anterior.
packets-f-smb-next = Próximo comando
packets-f-smb-next-about = Onde começa o próximo comando quando vários vão juntos; 0 no último ou no único.
packets-f-smb-msgid = ID da mensagem
packets-f-smb-msgid-about = Conta os pedidos do cliente; uma resposta leva o número do seu pedido.
packets-f-smb-reserved = Reservado
packets-f-smb-reserved-about = Reservado: o cliente deveria mandar 0, mas o Windows manda 0xFEFF; o servidor o ignora.
packets-f-smb-tree = Tree ID
packets-f-smb-tree-about = Para qual compartilhamento é o pedido, pelo número que o servidor deu a ele no TREE_CONNECT.
packets-f-smb-session = Session ID
packets-f-smb-session-about = A qual sessão autenticada o pedido pertence, pelo número que o servidor deu a ela no SESSION_SETUP.
packets-f-smb-signature = Assinatura
packets-f-smb-signature-about = Com a assinatura ligada, um hash com chave sobre a mensagem que prova que ela não foi alterada; zero caso contrário.

## TCP

packets-tcp-name = TCP
packets-tcp-title = Transmission Control Protocol
packets-tcp-tagline = Um fluxo confiável de bytes entre dois programas
packets-tcp-overview = O IP entrega pacotes um a um, e no caminho alguns se perdem, se duplicam ou chegam fora de ordem. O TCP constrói em cima disso um fluxo de bytes confiável e ordenado: abre uma conexão com um handshake de três vias, numera cada byte que envia, espera as confirmações, reenvia o que se perdeu e desacelera quando a rede está congestionada. A web, o e-mail e o SSH andam sobre ele.
packets-tcp-reliability = Números de sequência, confirmações e retransmissão

packets-tcp-1-title = SYN
packets-tcp-1-sub = O cliente pede para conectar
packets-tcp-1-body = O cliente envia um segmento com a flag SYN ligada e um número de sequência inicial sorteado — mostrado aqui como 0, relativo, como uma captura mostra. Diz também quanto consegue receber (sua janela) e o maior segmento que aceita (MSS).
packets-tcp-p1 = Vamos conversar? Meus bytes serão contados a partir daqui.

packets-tcp-2-title = SYN-ACK
packets-tcp-2-sub = O servidor aceita
packets-tcp-2-body = Há um programa escutando na porta 80, então o servidor responde com SYN e ACK ligados: confirma o número de sequência do cliente mais um e envia o seu próprio número inicial. Uma porta sem ninguém escutando responderia RST, e a conexão terminaria aqui.
packets-tcp-p2 = Sim. Recebi seu primeiro número; aqui está o meu.

packets-tcp-3-title = ACK
packets-tcp-3-sub = A conexão está aberta
packets-tcp-3-body = O cliente confirma o número do servidor. Os dois lados agora concordam onde cada fluxo de bytes começa, e a conexão está ESTABLISHED. O handshake custou uma ida e volta antes de qualquer dado andar — por isso o TLS 1.3 e o QUIC se esforçam tanto para economizar idas e voltas.
packets-tcp-p3 = Recebi o seu. Estamos conectados.

packets-tcp-4-title = Dados e confirmações
packets-tcp-4-sub = Cada byte é contado
packets-tcp-4-body = O cliente envia 517 bytes; o servidor os confirma com Ack=518 — "mande o byte 518 agora" — e responde com 1.448 bytes seus, que o cliente confirma por sua vez. Uma confirmação que não chega a tempo faz quem enviou mandar os bytes de novo, e quem recebe usa os números de sequência para pôr tudo em ordem e descartar duplicatas.
packets-tcp-p4 = Bytes 1 a 517 do meu fluxo.
packets-tcp-p5 = Recebi tudo até o 517.
packets-tcp-p6 = Bytes 1 a 1.448 do meu.
packets-tcp-p7 = Recebi tudo até o 1.448.

packets-tcp-5-title = Encerramento
packets-tcp-5-sub = Cada lado diz FIN
packets-tcp-5-body = Sem mais nada a enviar, o cliente manda FIN; o servidor confirma e, quando também termina, manda o seu FIN, que o cliente confirma. Quem fechou primeiro ainda espera um pouco (TIME-WAIT), para que um pacote atrasado desta conexão não seja tomado por parte de uma nova.
packets-tcp-5-state = TIME-WAIT, depois fechada
packets-tcp-p8 = Não tenho mais nada a enviar.
packets-tcp-p9 = O seu lado também fechou. Tchau.

## DNS

packets-dns-name = DNS
packets-dns-title = Domain Name System
packets-dns-tagline = Transforma nomes em endereços
packets-dns-overview = As máquinas se acham por endereço IP, mas as pessoas lembram nomes. O DNS é a lista telefônica da internet: um banco de dados hierárquico e distribuído que responde perguntas como "qual é o endereço de www.example.com?". Nenhum servidor sabe tudo: um resolvedor percorre a hierarquia — da raiz aos servidores do domínio de topo (.com) e daí aos servidores autoritativos do próprio domínio — e guarda cada resposta pelo tempo que o TTL dela permite.
packets-dns-transport = UDP (TCP para respostas grandes)

packets-dns-1-title = A pergunta
packets-dns-1-sub = O cliente pergunta ao seu resolvedor
packets-dns-1-body = Um programa precisa do endereço de www.example.com. O sistema envia um pequeno pacote UDP ao seu resolvedor recursivo, com o bit RD ligado: "por favor, ache a resposta para mim". Se o resolvedor tivesse guardado a resposta de uma pergunta anterior, a história terminaria com a resposta dele.
packets-dns-p1 = Qual é o endereço de www.example.com?

packets-dns-2-title = A raiz
packets-dns-2-sub = Quem cuida do .com?
packets-dns-2-body = O resolvedor começa pelo topo, por um dos 13 nomes de servidores raiz — servidos, juntos, por bem mais de mil máquinas pelo mundo. A raiz não sabe o endereço, mas sabe quem responde pelo .com, e devolve uma indicação: os nomes dos servidores do .com e seus endereços.
packets-dns-p2 = Raiz, qual é o endereço de www.example.com?
packets-dns-p3 = Não sei — pergunte aos servidores do .com. Aqui estão eles.

packets-dns-3-title = O domínio de topo
packets-dns-3-sub = Quem cuida do example.com?
packets-dns-3-body = Em seguida o resolvedor pergunta a um servidor do .com. A resposta é outra indicação, um nível abaixo: os servidores de nomes autoritativos de example.com, como ns1.example.com — com os endereços deles junto ("glue"), para o resolvedor não precisar procurá-los também.
packets-dns-p4 = .com, qual é o endereço de www.example.com?
packets-dns-p5 = Pergunte ao servidor do próprio example.com, ns1.example.com, em 198.51.100.53.

packets-dns-4-title = A resposta autoritativa
packets-dns-4-sub = O servidor do próprio domínio responde
packets-dns-4-body = Enfim o resolvedor pergunta ao servidor que guarda a zona de example.com. Ele responde com o registro A — 203.0.113.10 — e um TTL: por quantos segundos a resposta pode ser reaproveitada. O bit AA vem ligado: esta é a fonte, não uma cópia.
packets-dns-p6 = ns1, qual é o endereço de www.example.com?
packets-dns-p7 = 203.0.113.10 — e pode guardar por uma hora.

packets-dns-5-title = Resposta e cache
packets-dns-5-sub = O cliente recebe seu endereço
packets-dns-5-body = O resolvedor entrega a resposta ao cliente e a guarda. Pela próxima hora (TTL 3600), quem lhe perguntar por www.example.com recebe o endereço na hora, sem ir à raiz, aos servidores do .com nem ao do próprio example.com. É esse cache que faz a maioria das consultas levar poucos milissegundos.
packets-dns-5-cache = Guardada por 3600 segundos
packets-dns-p8 = www.example.com está em 203.0.113.10.

## HTTP

packets-http-name = HTTP
packets-http-title = Hypertext Transfer Protocol
packets-http-tagline = Como a web pede uma página, e a recebe
packets-http-overview = O HTTP é a língua que navegadores e servidores web falam. Um cliente envia um pedido — um método como GET, um caminho e alguns cabeçalhos — e o servidor responde com um código de status, cabeçalhos e quase sempre um corpo: uma página, uma imagem, alguns dados. Cada troca é um pedido e uma resposta. O HTTP/1.1 os escreve como texto simples sobre uma conexão TCP; antes do primeiro byte, o navegador precisa descobrir o endereço do servidor (DNS) e abrir essa conexão (TCP).
packets-http-port = 80 (443 com TLS: HTTPS)
packets-http-format = Texto: uma linha inicial, cabeçalhos, depois o corpo

packets-http-1-title = O pedido
packets-http-1-sub = O navegador o prepara
packets-http-1-body = Você digita http://www.example.com/ na barra de endereço. O navegador divide a URL num esquema (http), num host (www.example.com) e num caminho (/), e vai enviar um GET desse caminho. Mas pacotes viajam para endereços IP, não para nomes — então antes ele precisa do endereço do servidor.

packets-http-2-title = Resolução DNS
packets-http-2-sub = Descobre o endereço do servidor
packets-http-2-body = O laptop pergunta ao seu resolvedor DNS — aqui, 8.8.8.8 — o endereço de www.example.com. A pergunta vai por UDP para a porta 53, e a resposta volta como um registro A: 203.0.113.10. Um resolvedor que já sabe a resposta responde do seu cache em poucos milissegundos.
packets-http-p1 = Qual é o endereço de www.example.com?
packets-http-p2 = É 203.0.113.10.

packets-http-3-title = Conexão TCP
packets-http-3-sub = Abre um canal confiável
packets-http-3-body = O HTTP anda sobre o TCP, então antes de qualquer dado da página, o laptop e o servidor abrem uma conexão com o handshake de três vias: SYN, SYN-ACK, ACK. O laptop escolhe uma porta temporária (52344) e fala com a porta 80 do servidor; cada lado também escolhe um número inicial para contar os bytes que vai enviar.
packets-http-p3 = Vamos abrir uma conexão?
packets-http-p4 = Sim — estou escutando na porta 80.
packets-http-p5 = Conectado.

packets-http-4-title = Troca HTTP
packets-http-4-sub = Pedido e resposta
packets-http-4-body = Pela conexão aberta o navegador envia seu pedido: a linha GET / HTTP/1.1 e cabeçalhos como Host: www.example.com. O servidor responde 200 OK, com cabeçalhos que descrevem o corpo — seu tipo, text/html, e seu tamanho — e então a própria página, dividida em quantos segmentos TCP forem precisos.
packets-http-p6 = Por favor, me envie a página em /.
packets-http-p7 = Aqui está: 200 OK, uma página HTML.

packets-http-5-title = A página
packets-http-5-sub = O navegador a desenha
packets-http-5-body = O navegador lê o HTML e monta a página. Cada imagem, folha de estilo e script que ela cita é mais um pedido — muitas vezes pela mesma conexão, que fica aberta para isso. Depois da página desenhada, a conexão pode ficar ociosa um tempo, pronta para o próximo clique.
packets-http-5-state = Ociosa, aberta para o próximo pedido

## Ethernet

packets-ethernet-name = Ethernet
packets-ethernet-title = Ethernet IEEE 802.3
packets-ethernet-tagline = Quadros entre as máquinas de uma rede local
packets-ethernet-overview = O Ethernet é como as máquinas de uma mesma rede local passam dados umas às outras, por um cabo ou através de um switch. Toda placa de rede tem um endereço MAC de 48 bits, e todo quadro leva o do destino e o de quem envia, um tipo que diz o que vai dentro (IPv4, IPv6, ARP) e, no fim, um checksum. Um switch junta muitos cabos: aprende qual endereço está atrás de qual porta lendo o remetente de cada quadro, manda cada quadro só pela porta onde está o destino, e o espalha por todas enquanto ainda não sabe. O Ethernet só alcança a rede local — além dela, quem assume é o IP.
packets-ethernet-addressing = Endereços MAC de 48 bits, um por placa de rede

packets-ethernet-1-title = O quadro
packets-ethernet-1-sub = O laptop embrulha seu pacote
packets-ethernet-1-body = O laptop vai mandar um documento para a impressora, cujo endereço MAC ele já sabe — descobri-lo é trabalho do ARP. Ele embrulha o pacote IP num quadro: o MAC da impressora como destino, o seu como origem, o EtherType 0x0800 para dizer que vem um pacote IPv4 e, depois dos dados, um checksum de 4 bytes (FCS) que deixa quem recebe descartar um quadro danificado no caminho. Um quadro leva de 46 a 1.500 bytes de dados; um pacote menor é completado com enchimento.

packets-ethernet-2-title = Flooding
packets-ethernet-2-sub = O switch ainda não conhece a impressora
packets-ethernet-2-body = O quadro entra no switch pela porta 1, e o switch anota a origem: 00:00:5e:00:53:0a está atrás da porta 1. Depois procura o destino na sua tabela MAC e não acha nada — ele esquece um endereço depois de cinco minutos de silêncio, e a impressora está quieta há mais tempo que isso. Então ele espalha o quadro por todas as outras portas (flooding): o computador e o roteador veem um destino que não é deles e o descartam, e só a impressora fica com ele.
packets-ethernet-2-table = O laptop está atrás da porta 1
packets-ethernet-2-destination = Desconhecido: por todas as outras portas
packets-ethernet-p1 = Do MAC do laptop para o da impressora.
packets-ethernet-p2 = Não sei onde está a impressora — então você também recebe uma cópia.
packets-ethernet-p3 = Uma cópia para você também — e esta é sua.
packets-ethernet-p4 = E mais uma cópia, pela última porta.

packets-ethernet-3-title = Aprendizado
packets-ethernet-3-sub = A resposta da impressora ensina o switch
packets-ethernet-3-body = A impressora responde. O quadro dela entra pela porta 3, então o switch aprende onde está 00:00:5e:00:53:14 — e, como já sabe que o laptop está atrás da porta 1, manda o quadro só por essa porta. Cada quadro ensina ao switch onde está quem o enviou: ninguém precisa preencher a tabela.
packets-ethernet-3-table = Laptop na porta 1, impressora na porta 3
packets-ethernet-p5 = Aqui é a impressora, atrás da porta 3, respondendo ao laptop.

packets-ethernet-4-title = Encaminhamento
packets-ethernet-4-sub = Direto de porta a porta
packets-ethernet-4-body = A partir de agora, os quadros do laptop para a impressora saem só pela porta 3. O computador e o roteador nunca os veem, então seus enlaces ficam livres para o próprio tráfego, e dois pares de máquinas podem conversar pelo switch em velocidade máxima ao mesmo tempo. Cada enlace envia e recebe ao mesmo tempo (full duplex), então os quadros nunca colidem.
packets-ethernet-4-port = Só a porta 3
packets-ethernet-p6 = O documento, direto para a porta 3.

packets-ethernet-5-title = Broadcast
packets-ethernet-5-sub = Um quadro para todas as máquinas
packets-ethernet-5-body = Alguns quadros são para todas as máquinas da rede: o destino deles é ff:ff:ff:ff:ff:ff, o endereço de broadcast. Nenhuma máquina tem esse endereço, então o switch nunca o aprende e sempre manda um quadro assim por todas as portas. O ARP faz suas perguntas desse jeito, como diz o EtherType 0x0806 — e, como toda máquina precisa ler todo broadcast, redes grandes são divididas em VLANs que os mantêm separados.
packets-ethernet-5-scope = Todas as máquinas da rede
packets-ethernet-p7 = Para todos na rede: quem tem 192.168.1.1?

## ARP

packets-arp-name = ARP
packets-arp-title = Address Resolution Protocol
packets-arp-tagline = Descobre o endereço MAC por trás de um endereço IPv4
packets-arp-overview = Um endereço IP diz para onde um pacote vai, mas numa rede local um quadro só pode ser entregue a um endereço MAC. O ARP descobre o endereço MAC por trás de um endereço IPv4 da mesma rede: uma máquina faz uma pergunta em broadcast a todas — quem tem 192.168.1.1? — e a máquina com esse endereço responde. As duas guardam a resposta no seu cache ARP por um tempo, para a pergunta não ser feita a cada pacote. O ARP acredita em toda resposta que recebe, o que o torna fácil de enganar numa rede compartilhada; o IPv6 faz o mesmo trabalho com o Neighbor Discovery.
packets-arp-used-by = IPv4 (o IPv6 usa o Neighbor Discovery)
packets-arp-security = Nenhuma: qualquer máquina pode responder

packets-arp-1-title = O MAC que falta
packets-arp-1-sub = O laptop sabe o IP, não o MAC
packets-arp-1-body = Você roda ping 203.0.113.10. Esse endereço não está na rede do laptop, 192.168.1.0/24, então o pacote precisa ir ao gateway padrão, o roteador em 192.168.1.1. Para pô-lo num quadro Ethernet o laptop precisa do endereço MAC do roteador — e o cache ARP dele não tem nada para 192.168.1.1.
packets-arp-1-cache = Nada para 192.168.1.1

packets-arp-2-title = Quem tem 192.168.1.1?
packets-arp-2-sub = Uma pergunta a todas as máquinas
packets-arp-2-body = O laptop manda em broadcast, para ff:ff:ff:ff:ff:ff, um pedido ARP: quem tem 192.168.1.1? Avise 192.168.1.10. Ele põe o próprio MAC e o próprio IP como os do remetente, e deixa zerado o MAC do alvo — é isso que ele pergunta. O switch manda o pedido por todas as portas; o computador vê um endereço que não é o seu e o ignora.
packets-arp-2-opcode = 1: pedido
packets-arp-p1 = Pessoal: quem tem 192.168.1.1? Avise 192.168.1.10.
packets-arp-p2 = Um broadcast: cada porta recebe uma cópia.
packets-arp-p3 = A pergunta do laptop, para você também.

packets-arp-3-title = A resposta
packets-arp-3-sub = O roteador responde só ao laptop
packets-arp-3-body = O roteador reconhece o próprio endereço e responde com uma resposta ARP, enviada direto ao MAC do laptop: 192.168.1.1 está em 00:00:5e:00:53:01. O laptop guarda o par no seu cache ARP. O roteador já fez o mesmo com o par do laptop, lido do pedido, então não vai precisar perguntar quando responder.
packets-arp-3-opcode = 2: resposta
packets-arp-3-cache = Cada um guarda o par do outro
packets-arp-p4 = 192.168.1.1 sou eu, em 00:00:5e:00:53:01.

packets-arp-4-title = O pacote sai
packets-arp-4-sub = Dois endereços, dois trabalhos
packets-arp-4-body = Agora o ping pode ir. O quadro vai endereçado ao MAC do roteador, mas o pacote IP dentro dele continua dizendo 203.0.113.10: o endereço MAC o leva à próxima máquina do enlace; o endereço IP, ao fim do caminho. A entrada do cache vale por um tempo — de segundos a minutos, conforme o sistema — e depois é confirmada de novo, então o ARP pergunta raramente, não a cada pacote.
packets-arp-p5 = Para 203.0.113.10, entregue ao MAC do roteador.

packets-arp-5-title = ARP gratuito
packets-arp-5-sub = Uma resposta que ninguém pediu
packets-arp-5-body = Uma máquina também pode anunciar o próprio par sem ser perguntada — um ARP gratuito, mandado em broadcast quando ela liga ou quando um endereço passa para outra máquina, para que todos os caches da rede se atualizem de uma vez. Mas nada no ARP prova quem enviou uma resposta: qualquer máquina da rede pode anunciar 192.168.1.1 com o próprio MAC e receber o tráfego destinado ao roteador (ARP spoofing). Switches que conferem cada resposta ARP com os endereços que o DHCP distribuiu, e a criptografia acima dele, como o TLS, são a defesa.
packets-arp-5-trust = Vale a última resposta
packets-arp-p6 = Pessoal: 192.168.1.1 está em 00:00:5e:00:53:01.

## Wi-Fi

packets-wifi-name = Wi-Fi (WPA2)
packets-wifi-title = Wi-Fi Protected Access 2
packets-wifi-tagline = Entrar numa rede sem fio, e manter o ar privado
packets-wifi-overview = O Wi-Fi leva os quadros de uma rede pelo rádio, então qualquer um por perto consegue ouvi-los. O WPA2-Personal os mantém privados com uma senha compartilhada por todos na rede, que nunca vai ao ar: o celular e o ponto de acesso a transformam, cada um, na mesma chave, provam um ao outro que a têm e derivam chaves novas para a conexão no handshake de quatro vias. A partir daí, cada quadro de dados é cifrado e conferido com AES (CCMP). O ponto fraco é a própria senha: uma senha curta pode ser adivinhada offline a partir de um handshake gravado — o que o SAE do WPA3 foi feito para impedir.
packets-wifi-authentication = Uma senha compartilhada pela rede toda (PSK)
packets-wifi-label-ssid = Nome da rede (SSID)
packets-wifi-label-aid = ID de associação

packets-wifi-1-title = Achando a rede
packets-wifi-1-sub = Beacons e probes
packets-wifi-1-body = Umas dez vezes por segundo o ponto de acesso transmite um beacon com o nome da rede (SSID), o canal, as velocidades que oferece e um elemento RSN que diz WPA2: criptografia CCMP e uma chave pré-compartilhada. Um celular que está procurando redes também pode perguntar diretamente com um Probe Request, e o ponto de acesso responde com um Probe Response que diz o mesmo. É assim que a lista de redes do seu celular se preenche.
packets-wifi-p1 = Sou a "example", no canal 6: WPA2, CCMP, senha compartilhada.
packets-wifi-p2 = A rede "example" está aqui?
packets-wifi-p3 = Aqui estou: "example", WPA2 com senha.

packets-wifi-2-title = A senha
packets-wifi-2-sub = Vira a chave compartilhada
packets-wifi-2-body = Você escolhe a rede e digita a senha. O celular nunca a envia: ele a estica com o PBKDF2 — 4.096 rodadas de HMAC-SHA1, com o SSID como sal — numa chave de 256 bits, a PMK, e o ponto de acesso tem a mesma PMK, feita do mesmo jeito a partir da sua própria cópia. Esse esticamento lento torna cada tentativa cara, mas uma senha curta ou comum ainda pode ser descoberta offline por quem gravar um único handshake.
packets-wifi-2-size = 256 bits

packets-wifi-3-title = A entrada
packets-wifi-3-sub = Autenticação e associação
packets-wifi-3-body = O celular e o ponto de acesso trocam dois quadros de Authentication — Open System, uma formalidade herdada do primeiro padrão do Wi-Fi, em que ninguém prova nada — e depois dois de Association: o celular pede para entrar com CCMP e a chave pré-compartilhada, e o ponto de acesso aceita e lhe dá um ID de associação. O celular agora está conectado, mas não é confiável: até as chaves serem combinadas, o ponto de acesso não deixa passar nada além do handshake.
packets-wifi-p4 = Posso me autenticar? Open System.
packets-wifi-p5 = Concedido — esta parte não prova nada.
packets-wifi-p6 = Quero entrar, com CCMP e chave pré-compartilhada.
packets-wifi-p7 = Bem-vindo: você é a associação 1.

packets-wifi-4-title = O handshake de quatro vias
packets-wifi-4-sub = Chaves novas, e a prova da senha
packets-wifi-4-body = O ponto de acesso envia um número aleatório, o ANonce (mensagem 1). O celular escolhe o seu, o SNonce, deriva a chave da sessão — a PTK — da PMK, dos dois números e dos dois endereços MAC, e envia o SNonce com um MIC feito com essa chave (mensagem 2). O ponto de acesso deriva a mesma PTK e confere o MIC — a prova de que o celular sabe a senha —, depois envia a chave de grupo para os broadcasts, cifrada, com um MIC seu (mensagem 3); o celular confirma (mensagem 4), e os dois instalam as chaves.
packets-wifi-p8 = Aqui está meu número aleatório, o ANonce.
packets-wifi-p9 = Aqui está o meu, o SNonce — com um MIC que prova que sei a chave.
packets-wifi-p10 = Seu MIC confere. Aqui está a chave de grupo, cifrada: instale suas chaves.
packets-wifi-p11 = Chaves instaladas.

packets-wifi-5-title = Tráfego cifrado
packets-wifi-5-sub = Cada quadro selado, até o cabo
packets-wifi-5-body = Cada quadro de dados entre o celular e o ponto de acesso agora é cifrado e autenticado com CCMP, sob uma chave tirada da PTK; um número de pacote em cada quadro impede que um quadro antigo seja reenviado. O ponto de acesso o decifra e passa o pacote ao roteador como um quadro Ethernet comum: o WPA2 protege o ar, não o resto do caminho — isso é trabalho do TLS.
packets-wifi-5-scope = Entre o celular e o ponto de acesso
packets-wifi-p12 = Selado com nossa chave de sessão: só você consegue ler.
packets-wifi-p13 = O mesmo pacote, agora no cabo, às claras.

## IP

packets-ip-name = IP (IPv4)
packets-ip-title = Internet Protocol, versão 4
packets-ip-tagline = Leva um pacote de rede em rede, um salto por vez
packets-ip-overview = O IP é o que faz da internet uma rede só: toda máquina tem um endereço, e todo pacote leva o endereço de onde vem e o endereço para onde vai. Nenhuma máquina conhece o caminho inteiro. Cada roteador olha só o destino, acha a rota mais específica que tem para ele e passa o pacote um salto adiante, diminuindo seu TTL em um para que um pacote preso num laço morra em vez de circular para sempre. O IP não promete nada — pacotes podem se perder, se duplicar ou chegar fora de ordem, e consertar isso é trabalho do TCP — e o IPv4 tem só cerca de 4,3 bilhões de endereços, então casas e escritórios usam endereços privados atrás de um único endereço público.
packets-ip-addressing = 32 bits, escritos como quatro números
packets-ip-reliability = Nenhuma: melhor esforço

packets-ip-1-title = Aqui ou em outro lugar?
packets-ip-1-sub = O laptop escolhe o próximo salto
packets-ip-1-body = O laptop tem um pacote para 203.0.113.10. A rede dele é 192.168.1.0/24 — todo endereço que começa com 192.168.1 — e este não começa, então o pacote vai para o gateway padrão, o roteador em 192.168.1.1. O cabeçalho traz os dois endereços, o protocolo 6 do TCP que vai dentro e um TTL de 64: o quadro que o leva é endereçado ao roteador, mas o pacote continua endereçado ao servidor.
packets-ip-p1 = Para 203.0.113.10 — não é da nossa rede, então vou para o roteador.

packets-ip-2-title = O roteador de casa
packets-ip-2-sub = Uma saída só, e uma nova origem
packets-ip-2-body = O roteador lê só o destino. A tabela dele é curta — 192.168.1.0/24 é a rede de casa, e todo o resto, a rota padrão 0.0.0.0/0, vai para o provedor —, então o pacote sai, com o TTL baixado para 63 e o checksum do cabeçalho recalculado. E como 192.168.1.10 é um endereço privado, que não significa nada na internet, o roteador também troca a origem pelo seu endereço público, 198.51.100.23 (o NAT tem um manual só dele).
packets-ip-p2 = Não é da rede de casa: para o provedor, um salto a menos, de um endereço público.

packets-ip-3-title = O prefixo mais longo
packets-ip-3-sub = O roteador do provedor escolhe
packets-ip-3-body = O roteador do provedor conhece rotas para a internet inteira — cerca de um milhão de prefixos — e vários deles contêm 203.0.113.10: a rota padrão, 203.0.112.0/22 e 203.0.113.0/24. Vence o mais específico, o prefixo mais longo, /24, que aponta para o roteador de trânsito. O TTL cai para 62.
packets-ip-p3 = 203.0.113.0/24 é a melhor correspondência: para o roteador de trânsito.

packets-ip-4-title = Entregue
packets-ip-4-sub = O último roteador conhece a rede do servidor
packets-ip-4-body = O roteador de trânsito está ligado à própria 203.0.113.0/24, então entrega o pacote direto a 203.0.113.10, com TTL 61. O servidor vê o próprio endereço como destino e passa os dados ao TCP, como diz o protocolo 6. Se algum enlace do caminho fosse pequeno demais para o pacote, um roteador teria de dividi-lo em fragmentos — mas a flag DF proíbe isso, então ele descartaria o pacote e avisaria quem o enviou, com uma mensagem ICMP, para mandar pacotes menores.
packets-ip-p4 = Para a sua própria rede: aqui está ele, com 61 saltos de sobra.

packets-ip-5-title = A volta
packets-ip-5-sub = Um pacote novo, roteado por conta própria
packets-ip-5-body = A resposta do servidor é um pacote novo, de 203.0.113.10 para 198.51.100.23, que começa de novo com TTL 64. Os roteadores da internet não guardam nada dos pacotes que passaram, então ele é roteado de novo a cada salto e pode até voltar por outro caminho. No roteador de casa a tradução é desfeita, e a resposta chega a 192.168.1.10.
packets-ip-p5 = Minha resposta — ela acha sozinha o caminho de volta.

## IPv6

packets-ipv6-name = IPv6
packets-ipv6-title = Internet Protocol, versão 6
packets-ipv6-tagline = Endereços para tudo, e máquinas que se configuram sozinhas
packets-ipv6-overview = O IPv6 é o protocolo mais novo da internet: seus endereços têm 128 bits, tantos que cada dispositivo pode ter um endereço público só dele e nenhuma rede precisa dividir um. Ele também muda como uma máquina entra numa rede: em vez de esperar receber um endereço, ela pode criar o seu — primeiro um link-local, depois um global a partir do prefixo que o roteador anuncia — depois de conferir que ninguém mais o usa. O ARP também sai de cena: o Neighbor Discovery, um conjunto de mensagens ICMPv6 enviadas a grupos multicast, encontra roteadores, prefixos e os endereços MAC dos vizinhos.
packets-ipv6-addressing = 128 bits, em oito grupos hexadecimais
packets-ipv6-replaces = IPv4, convivendo com ele por enquanto

packets-ipv6-1-title = Endereço link-local
packets-ipv6-1-sub = Um endereço para este enlace, conferido antes
packets-ipv6-1-body = Assim que a interface sobe, o laptop cria um endereço que só funciona neste enlace: o prefixo fe80::/64 e um identificador de interface de 64 bits criado por ele mesmo, que os sistemas modernos fazem parecer aleatório para que o laptop não possa ser seguido de rede em rede. Antes de usá-lo, o laptop faz a Duplicate Address Detection: uma Neighbor Solicitation por esse mesmo endereço, enviada do endereço não especificado :: para o grupo multicast solicited-node do endereço. Se ninguém responder em um segundo, o endereço é dele.
packets-ipv6-1-scope = Só este enlace, nunca roteado
packets-ipv6-p1 = Alguém está usando fe80::8d4f:2a1c:93e6:5b07? Fale agora.

packets-ipv6-2-title = Achando o roteador
packets-ipv6-2-sub = Router Solicitation e Advertisement
packets-ipv6-2-body = O laptop envia uma Router Solicitation para ff02::2, o grupo que todo roteador do enlace escuta. O roteador responde com uma Router Advertisement para todos os nós (ff02::1): o prefixo da rede, 2001:db8:1:1::/64, com a flag A que deixa as máquinas criarem seus próprios endereços nele, a MTU do enlace, o seu endereço MAC e ele mesmo, fe80::1, como saída. Os roteadores também mandam esses anúncios sem ninguém pedir, a cada poucos minutos, para toda máquina do enlace ficar sabendo.
packets-ipv6-p2 = Tem algum roteador aqui?
packets-ipv6-p3 = Sou fe80::1, sua saída. Esta rede é 2001:db8:1:1::/64 — crie um endereço nela.

packets-ipv6-3-title = Endereço global
packets-ipv6-3-sub = SLAAC: o laptop cria o seu
packets-ipv6-3-body = Com o prefixo anunciado e outro identificador de interface próprio, o laptop monta um endereço global: 2001:db8:1:1:3c91:b7e2:41f0:6a5d. Ele confere também este com a Duplicate Address Detection, e daí em diante o endereço é único na internet inteira — nenhum servidor o distribuiu, e nenhum NAT vai reescrevê-lo. Isso é o SLAAC, a autoconfiguração de endereço sem estado: o roteador só diz o prefixo, e não guarda nenhuma lista de quem pegou qual endereço.
packets-ipv6-3-scope = Global: a internet inteira

packets-ipv6-4-title = Descoberta de vizinhos
packets-ipv6-4-sub = O que o ARP fazia, por multicast
packets-ipv6-4-body = O laptop quer imprimir em 2001:db8:1:1::20, que o prefixo diz estar neste enlace, então precisa do endereço MAC da impressora. Em vez de perguntar a todas as máquinas em broadcast, como o ARP, ele envia uma Neighbor Solicitation ao grupo solicited-node da impressora, ff02::1:ff00:20, do qual só participam máquinas cujo endereço termina nos mesmos 24 bits — quase sempre uma só. A impressora responde com uma Neighbor Advertisement que traz o seu MAC, e o laptop o guarda no seu cache de vizinhos.
packets-ipv6-4-cache = Guardado no cache de vizinhos
packets-ipv6-p4 = Quem tem 2001:db8:1:1::20? Meu MAC é 00:00:5e:00:53:0a.
packets-ipv6-p5 = Sou eu — meu MAC é 00:00:5e:00:53:14.

packets-ipv6-5-title = Um pacote para fora
packets-ipv6-5-sub = Fora do enlace, pelo roteador
packets-ipv6-5-body = Agora um pacote para um servidor em outro lugar, 2001:db8:2::10. Ele não está neste enlace, então vai para o roteador, cujo MAC veio junto com o anúncio. O cabeçalho é mais simples que o do IPv4 — 40 bytes fixos e nenhum checksum — e o TTL agora se chama hop limit, 64 aqui, diminuído por cada roteador. Roteadores nunca fragmentam um pacote IPv6: um grande demais para um enlace é descartado com um ICMPv6 Packet Too Big, e quem enviou manda pacotes menores.
packets-ipv6-p6 = Para 2001:db8:2::10, pelo roteador — com 64 saltos de sobra.

## ICMP

packets-icmp-name = ICMP
packets-icmp-title = Internet Control Message Protocol
packets-icmp-tagline = As mensagens do próprio IP: erros, e os testes do ping e do traceroute
packets-icmp-overview = O IP leva pacotes, mas sozinho nunca conta a quem enviou o que deu errado com um deles. O ICMP é como ele conta: quando um roteador descarta um pacote porque o TTL acabou, ou não consegue alcançar o endereço que ele indica, manda uma mensagem ICMP curta de volta a quem enviou. O ICMP viaja direto dentro do IP, como protocolo número 1, sem portas — só um tipo e um código dizendo o que aconteceu. Duas ferramentas do dia a dia são feitas dele: o ping, que pede a uma máquina que devolva um pacote como um eco, e o traceroute, que mapeia os roteadores do caminho. Muitos firewalls descartam ICMP, então o silêncio não prova que uma máquina está fora do ar.
packets-icmp-transport = Direto dentro do IP, como protocolo 1 — sem portas
packets-icmp-label-types = Tipos comuns
packets-icmp-types = 0 echo reply, 3 destino inalcançável, 8 echo request, 11 tempo excedido
packets-icmp-used-by = ping, traceroute e descoberta do MTU do caminho
packets-icmp-label-message = Tipo e código
packets-icmp-label-rtt = Tempo de ida e volta

packets-icmp-1-title = Echo request
packets-icmp-1-sub = O ping pergunta: você está aí?
packets-icmp-1-body = Você digita ping 203.0.113.10. O laptop envia uma mensagem ICMP do tipo 8, código 0 — um echo request — direto dentro de um pacote IP: sem TCP, sem UDP, sem portas. Um identificador separa este ping de outros rodando ao mesmo tempo, e um número de sequência conta os pedidos, um por segundo, para cada resposta casar com sua pergunta. Os dados levam o instante em que o pedido saiu.
packets-icmp-p1 = Você está aí? Me devolva isto, exatamente como está.

packets-icmp-2-title = Echo reply
packets-icmp-2-sub = O servidor devolve o eco
packets-icmp-2-body = O servidor responde com o tipo 0, um echo reply, devolvendo o mesmo identificador, o mesmo número de sequência e os mesmos dados. O laptop subtrai a hora escrita nos dados da hora atual: a ida e volta levou 30 ms. Uma resposta prova que o caminho inteiro funciona nos dois sentidos; a falta dela prova pouco — o pedido ou a resposta pode ter sido descartado no caminho, talvez por um firewall, ou a máquina pode estar fora do ar.
packets-icmp-p2 = Estou aqui — seu eco, sem mudanças.

packets-icmp-3-title = Traceroute: salto 1
packets-icmp-3-sub = Um pacote com direito a um salto só
packets-icmp-3-body = O traceroute mapeia o caminho usando o TTL como sonda. Seu primeiro echo request sai com TTL 1: o roteador de casa o reduz a 0, tem de descartar o pacote e devolve um ICMP Time Exceeded (tipo 11, código 0) do seu próprio endereço, citando o começo do pacote descartado. Essa resposta revela o primeiro salto, e quanto tempo levou para chegar até ele. Cada TTL costuma ser tentado três vezes; aqui aparece uma tentativa de cada.
packets-icmp-p3 = Um eco para 203.0.113.10 — mas meu TTL só me leva um salto.
packets-icmp-p4 = O TTL do seu pacote acabou aqui, em 192.168.1.1.

packets-icmp-4-title = Traceroute: saltos 2 e 3
packets-icmp-4-sub = Um roteador mais longe a cada vez
packets-icmp-4-body = As sondas seguintes saem com TTL 2, depois 3. Cada uma vai um roteador mais longe antes de o TTL acabar, e esse roteador responde Time Exceeded por sua vez: o roteador do provedor, em 198.51.100.1, e depois o da hospedagem, em 192.0.2.1. Os tempos crescem com a distância — 12 ms, depois 28 ms. Um roteador que descarta pacotes vencidos sem responder aparece como uma linha de asteriscos.
packets-icmp-p5 = De novo, com TTL 2.
packets-icmp-p6 = O TTL acabou no salto 2: sou 198.51.100.1.
packets-icmp-p7 = E com TTL 3.
packets-icmp-p8 = Salto 3, aqui: 192.0.2.1.

packets-icmp-5-title = O destino responde
packets-icmp-5-sub = O traçado está completo
packets-icmp-5-body = Com TTL 4 a sonda chega ao próprio 203.0.113.10, que responde com um echo reply em vez de Time Exceeded: o traceroute sabe que chegou, e para. As respostas, em ordem, são o caminho. Sem o -I, o traceroute do Linux envia UDP para portas sem uso e para no Port Unreachable que o destino devolve; o tracert do Windows usa echo requests, como aqui.
packets-icmp-p9 = E com TTL 4.
packets-icmp-p10 = Echo reply — você chegou a mim, o destino.

packets-icmp-6-title = Destino inalcançável
packets-icmp-6-sub = Não há ninguém nesse endereço
packets-icmp-6-body = Agora você dá ping em 203.0.113.99, um endereço da rede da hospedagem onde não há máquina nenhuma. O roteador da hospedagem, o último do caminho, pergunta na sua rede local quem tem esse endereço (ARP), não ouve nada, desiste depois de alguns segundos e responde com o tipo 3, Destination Unreachable — código 1, host inalcançável. Outros códigos dizem que a rede é inalcançável (0), que a porta está fechada (3) ou que um firewall proíbe a passagem (13).
packets-icmp-6-reason = Nenhuma máquina responde nesse endereço
packets-icmp-p11 = Tem alguém em 203.0.113.99?
packets-icmp-p12 = Ninguém responde nesse endereço: host inalcançável.

## NAT

packets-nat-name = NAT
packets-nat-title = Network Address Translation
packets-nat-tagline = Muitos aparelhos atrás de um só endereço público
packets-nat-overview = Existem só cerca de 4,3 bilhões de endereços IPv4 — bem menos que os aparelhos que querem um. Por isso uma casa ou um escritório recebe do provedor um único endereço público e dá aos seus aparelhos endereços privados (192.168.x.x, 10.x.x.x, 172.16–31.x.x), que qualquer rede pode reaproveitar e que nenhum roteador da internet encaminha. O roteador no meio reescreve cada pacote que sai para que ele venha do endereço público, numa porta escolhida pelo roteador, e anota a troca numa tabela de tradução para desfazê-la nas respostas. Funciona tão bem que a maior parte da internet está atrás de um NAT; o IPv6, com endereços para todos os aparelhos, o torna desnecessário.
packets-nat-addressing = Muitos endereços privados dividem um endereço público
packets-nat-label-ranges = Faixas privadas
packets-nat-used-by = Quase toda rede doméstica, de escritório e móvel em IPv4

packets-nat-1-title = Endereços privados
packets-nat-1-sub = O laptop abre uma conexão
packets-nat-1-body = O laptop quer uma página de 198.51.100.20 e envia um SYN do seu próprio endereço, 192.168.1.10, porta 52344. Esse endereço é privado: milhares de outras casas usam exatamente o mesmo, então nenhuma resposta conseguiria achar o caminho de volta até ele. O pacote vai para o gateway — o roteador de casa, que tem dois endereços: 192.168.1.1, voltado para a casa, e 203.0.113.7, o único público, voltado para a internet.
packets-nat-p1 = Vamos conectar — sou 192.168.1.10, porta 52344.

packets-nat-2-title = Tradução
packets-nat-2-sub = O roteador troca a origem
packets-nat-2-body = Antes de passar o SYN adiante, o roteador reescreve a origem: 192.168.1.10:52344 vira 203.0.113.7:40001, uma porta que ele escolhe entre as suas. Ele recalcula os checksums do IP e do TCP, que cobrem os endereços, e anota o par na sua tabela de tradução. Para o servidor, a conexão vem do roteador; o laptop fica invisível.
packets-nat-p2 = O mesmo SYN — mas agora vindo de 203.0.113.7, porta 40001.

packets-nat-3-title = O caminho de volta
packets-nat-3-sub = A tabela diz ao roteador para onde vai
packets-nat-3-body = O servidor responde ao endereço que viu, 203.0.113.7:40001. O roteador procura a porta 40001 na tabela, acha o laptop, reescreve o destino de volta para 192.168.1.10:52344 e passa o SYN-ACK para dentro. A entrada vive enquanto a conexão durar — no UDP, que nunca se despede, até ficar um tempo em silêncio — e então é esquecida.
packets-nat-p3 = Sim — respondendo a 203.0.113.7, porta 40001.
packets-nat-p4 = A porta 40001 é do laptop: segue para 192.168.1.10:52344.

packets-nat-4-title = Um segundo aparelho
packets-nat-4-sub = Outra porta para o celular
packets-nat-4-body = O celular abre uma conexão com o mesmo servidor — e, por acaso, da mesma porta, 52344. O roteador não pode reaproveitar a 40001, ou as respostas se misturariam, então dá ao celular a 40002. É a porta pública que separa as conversas: com cerca de 64 mil portas para escolher, um só endereço público dá conta de milhares de conexões ao mesmo tempo.
packets-nat-p5 = Vamos conectar — sou 192.168.1.11, também da porta 52344.
packets-nat-p6 = Esta sai pela porta 40002, para as respostas não se misturarem.

packets-nat-5-title = Pacotes não pedidos
packets-nat-5-sub = Sem entrada, sem passagem
packets-nat-5-body = Uma máquina na internet tenta se conectar a 203.0.113.7, porta 22. Nenhum aparelho de dentro começou essa conversa, então a tabela não tem entrada e o roteador não tem para onde mandar o pacote: ele o descarta. Para deixar uma conexão dessas entrar, o dono configura um redirecionamento de porta (port forwarding) — uma entrada fixa como porta 22 → 192.168.1.10:22. É a mesma barreira que faz dois aparelhos atrás de NATs diferentes precisarem de ajuda para se alcançarem: o STUN, para descobrirem seus endereços públicos e tentarem um caminho direto, ou o TURN, para retransmitir por um servidor quando nenhum caminho direto passa.
packets-nat-5-verdict = Descartado: nenhuma entrada na tabela
packets-nat-p7 = Alguém escutando na porta 22?

## DHCP

packets-dhcp-name = DHCP
packets-dhcp-title = Dynamic Host Configuration Protocol
packets-dhcp-tagline = Como um aparelho ganha seu endereço ao entrar numa rede
packets-dhcp-overview = Um aparelho que é ligado no cabo, ou entra numa rede Wi-Fi, conhece o próprio endereço de hardware (MAC), mas mais nada: nem seu endereço IP, nem o tamanho da rede, nem o roteador que leva para fora dela, nem os servidores DNS. Um servidor DHCP na rede entrega tudo isso, emprestando a cada aparelho um endereço do seu pool por um tempo limitado — uma concessão (lease) —, para que os endereços dos aparelhos que saem voltem ao pool. A troca leva quatro mensagens, muitas vezes lembradas como DORA: Discover, Offer, Request, Acknowledge. Em casa, o servidor DHCP costuma ser o próprio roteador; num escritório, muitas vezes é um servidor à parte, como aqui.
packets-dhcp-port = 67 no servidor, 68 no cliente
packets-dhcp-label-messages = Mensagens
packets-dhcp-label-mask = Máscara de sub-rede
packets-dhcp-label-chosen = Servidor escolhido
packets-dhcp-label-dns = Servidores DNS
packets-dhcp-label-renewal = Renovação (T1)

packets-dhcp-1-title = DISCOVER
packets-dhcp-1-sub = Tem um servidor DHCP aqui?
packets-dhcp-1-body = O laptop acabou de entrar na rede e não tem endereço IP, então envia de 0.0.0.0 para 255.255.255.255 — o endereço de broadcast, que todo aparelho da rede local recebe. O UDP da porta 68 para a 67 o marca como um cliente DHCP falando com servidores: só eles respondem, e os outros o ignoram. Dentro vão o endereço MAC, pelo qual o servidor o reconhecerá, um ID de transação aleatório (xid) para casar as respostas, e as opções que ele quer: máscara, roteador, servidores DNS, tempo de concessão.
packets-dhcp-p1 = Sou novo aqui e não tenho endereço. Tem algum servidor DHCP?

packets-dhcp-2-title = OFFER
packets-dhcp-2-sub = O servidor propõe um endereço
packets-dhcp-2-body = O servidor escolhe um endereço livre do seu pool, 192.168.1.10, e o oferece com tudo o que vem junto: a máscara de sub-rede, o roteador, os servidores DNS e o tempo de concessão, 24 horas. O laptop ainda não pode ser alcançado por IP, então o servidor endereça o quadro ao MAC dele — ou o envia em broadcast, se o laptop pediu isso. O ID de transação mostra a qual DISCOVER isto responde.
packets-dhcp-p2 = Pode ficar com 192.168.1.10, por 24 horas — com estas configurações.

packets-dhcp-3-title = REQUEST
packets-dhcp-3-sub = O laptop aceita, em voz alta
packets-dhcp-3-body = Se vários servidores fizeram ofertas, o laptop escolhe uma e pede o endereço dela — ainda em broadcast, ainda de 0.0.0.0, citando o servidor escolhido. Anunciar a escolha a todos é de propósito: cada outro servidor que fez uma oferta fica sabendo que não foi o escolhido, e devolve o endereço ao pool.
packets-dhcp-p3 = Fico com 192.168.1.10, do servidor 192.168.1.2.

packets-dhcp-4-title = ACK
packets-dhcp-4-sub = O endereço é do laptop
packets-dhcp-4-body = O servidor confirma, e a concessão começa. O laptop configura seu endereço e sua máscara (192.168.1.10/24), manda tudo o que fica fora da sua rede para o roteador em 192.168.1.1 e pergunta nomes a 192.168.1.1. Muitos clientes antes conferem com ARP que nenhum outro aparelho já usa o endereço, e o recusam se algum usar.
packets-dhcp-p4 = Confirmado: 192.168.1.10 é seu por 24 horas.

packets-dhcp-5-title = Renovação
packets-dhcp-5-sub = Na metade da concessão
packets-dhcp-5-body = Doze horas depois — metade da concessão, o momento chamado T1 —, o laptop pede para manter o endereço. Agora que tem um, pergunta direto ao servidor, por unicast, e um ACK recomeça a concessão. Se o servidor não responde, a partir do T2 (87,5% da concessão) o laptop pergunta em broadcast a qualquer servidor; se a concessão acaba, ele precisa parar de usar o endereço e recomeçar com um DISCOVER.
packets-dhcp-p5 = Posso continuar com 192.168.1.10?
packets-dhcp-p6 = Pode — por mais 24 horas.

## BGP

packets-bgp-name = BGP
packets-bgp-title = Border Gateway Protocol
packets-bgp-tagline = Como as redes da internet contam umas às outras onde estão os endereços
packets-bgp-overview = A internet é uma rede de redes: dezenas de milhares de sistemas autônomos (AS) — provedores, empresas de hospedagem, universidades —, cada um com seu próprio número, o número de AS. O BGP é como os roteadores de borda deles contam aos vizinhos quais blocos de endereços (prefixos) conseguem alcançar, e por quais AS. Cada rota leva essa lista, o caminho de AS (AS path), que cresce um AS a cada repasse: ela impede laços e ajuda cada rede a escolher um caminho — embora acordos comerciais pesem mais que a distância. A maioria dos anúncios é aceita na confiança, então um erro ou uma mentira pode desviar o tráfego; o RPKI deixa um roteador conferir quem pode anunciar um prefixo.
packets-bgp-security = Confiança entre vizinhos; o RPKI confere a origem
packets-bgp-label-messages = Mensagens
packets-bgp-label-hold = Hold time
packets-bgp-label-best = Melhor caminho
packets-bgp-label-backup = Caminho reserva
packets-bgp-label-roa = Origem assinada (ROA)
packets-bgp-device-second = Segundo roteador de trânsito

packets-bgp-1-title = OPEN
packets-bgp-1-sub = Dois vizinhos se apresentam
packets-bgp-1-body = Vizinhos BGP não se descobrem sozinhos: os operadores da rede de hospedagem (AS64511) e do seu provedor de trânsito (AS64510) combinam a conexão, e cada um configura o outro. O roteador da hospedagem abre uma conexão TCP para a porta 179, e cada lado envia um OPEN: a versão do BGP (4), seu número de AS, um hold time e seu ID de roteador, com as capacidades que suporta. Se o número de AS não for o configurado, o outro lado responde com uma NOTIFICATION e fecha a sessão.
packets-bgp-p1 = Sou o AS64511 e falo BGP-4. Se ficar 90 segundos sem notícias minhas, pode me dar por perdido.
packets-bgp-p2 = E eu sou o AS64510. Combinado.

packets-bgp-2-title = KEEPALIVE
packets-bgp-2-sub = A sessão está de pé
packets-bgp-2-body = Cada lado aceita o OPEN do outro com um KEEPALIVE — só um cabeçalho de 19 bytes — e a sessão fica Established. Daí em diante, um roteador sem mais nada a dizer envia um KEEPALIVE a cada 30 segundos, um terço do hold time. Se passarem 90 segundos sem nada do outro lado, ele dá a sessão por morta e joga fora todas as rotas que aprendeu por ela.
packets-bgp-p3 = Seu OPEN está de acordo comigo.
packets-bgp-p4 = O seu também — sessão estabelecida.

packets-bgp-3-title = UPDATE
packets-bgp-3-sub = Um prefixo é anunciado
packets-bgp-3-body = A rede de hospedagem anuncia o bloco onde ficam seus servidores, 203.0.113.0/24, num UPDATE cujo caminho de AS é só 64511 — "é aqui que ele começa". O roteador de trânsito põe o próprio número na frente e repassa a rota aos seus outros vizinhos, aqui o provedor e uma segunda rede de trânsito, cada um por uma sessão igual à primeira. O caminho de AS também impede laços: um roteador que acha o próprio número num caminho recusa a rota.
packets-bgp-p5 = 203.0.113.0/24 está aqui, comigo: caminho de AS 64511.
packets-bgp-p6 = 203.0.113.0/24 se alcança por mim: caminho de AS 64510 64511.
packets-bgp-p7 = A mesma notícia para você: caminho de AS 64510 64511.

packets-bgp-4-title = Melhor caminho
packets-bgp-4-sub = O provedor escolhe entre duas rotas
packets-bgp-4-body = A segunda rede de trânsito também repassa a rota, com o seu número na frente: 64505 64510 64511. O provedor agora conhece dois caminhos para 203.0.113.0/24. O BGP consulta primeiro a política da própria rede — uma preferência que o operador define, muitas vezes pelo vizinho mais barato — e, quando ela empata, fica com o caminho de AS mais curto: aqui, pelo AS64510. A outra rota fica guardada como reserva, pronta se a primeira for retirada.
packets-bgp-p8 = Por mim também: caminho de AS 64505 64510 64511.

packets-bgp-5-title = Um anúncio falso
packets-bgp-5-sub = O RPKI pega um sequestro de prefixo
packets-bgp-5-body = Outra rede, o AS64496, anuncia 203.0.113.0/24 como se fosse seu — por engano ou de propósito. O caminho dele, só 64496, é mais curto que o verdadeiro, e um roteador que acreditasse nele mandaria para lá o tráfego da hospedagem. Mas a empresa de hospedagem publicou um ROA, uma declaração assinada de que só o AS64511 pode originar esse prefixo, e o roteador do provedor a confere: o anúncio é inválido, e é descartado. O RPKI confere quem origina um prefixo, não cada salto do caminho, então os filtros entre vizinhos continuam necessários.
packets-bgp-5-verdict = Inválido — descartado
packets-bgp-p9 = 203.0.113.0/24 é meu: caminho de AS 64496.

packets-bgp-6-title = Retirada
packets-bgp-6-sub = A rota é retirada
packets-bgp-6-body = A rede de hospedagem deixa de anunciar 203.0.113.0/24. Ela envia um UPDATE que põe o prefixo na lista de rotas retiradas, e a retirada se espalha como o anúncio se espalhou, de vizinho em vizinho. Por um instante o provedor pode passar para o caminho reserva, até ele também ser retirado; aí não sobra rota nenhuma, e a partir do AS64500 o prefixo fica inalcançável.
packets-bgp-6-state = Nenhuma rota: inalcançável
packets-bgp-p10 = 203.0.113.0/24 foi retirado.
packets-bgp-p11 = Retirado: não alcanço mais 203.0.113.0/24.
packets-bgp-p12 = Retirado para você também.
packets-bgp-p13 = E por mim: retirado.

## UDP

packets-udp-name = UDP
packets-udp-title = User Datagram Protocol
packets-udp-tagline = Datagramas: enviados e esquecidos
packets-udp-overview = O UDP é a camada mais fina que um programa pode pôr sobre o IP: a cada pacote ele acrescenta uma porta de origem e uma de destino, para os dados chegarem ao programa certo, mais um tamanho e um checksum — 8 bytes ao todo. Não há conexão a abrir, nem confirmação, nem ordem, nem retransmissão: cada datagrama viaja sozinho, e um que se perde simplesmente some. É exatamente o que alguns programas querem: uma pergunta que cabe num pacote (DNS), uma chamada ou um jogo, em que dado atrasado não vale nada, ou um protocolo como o QUIC, que constrói a própria confiabilidade por cima.
packets-udp-reliability = Nenhuma: sem confirmação, ordem nem retransmissão
packets-udp-used-by = DNS, DHCP, NTP, chamadas de voz e vídeo, jogos, QUIC
packets-udp-device-game-server = Servidor do jogo
packets-udp-label-pick-udp = UDP para
packets-udp-label-pick-tcp = TCP para

packets-udp-1-title = Um datagrama
packets-udp-1-sub = Sem handshake: a pergunta vai na hora
packets-udp-1-body = O laptop pergunta ao seu resolvedor o endereço de www.example.com. Com TCP, ele gastaria antes uma ida e volta num handshake; com UDP, o primeiro pacote já leva a pergunta. O cabeçalho tem quatro campos: a porta de origem (51000, escolhida pelo sistema), a porta de destino (53, DNS), o tamanho e um checksum — mais nada.
packets-udp-1-handshake = Nenhum: o primeiro pacote já leva os dados
packets-udp-p1 = Uma pergunta para a porta 53 — sem apresentações.

packets-udp-2-title = A resposta
packets-udp-2-sub = Um datagrama de volta
packets-udp-2-body = O resolvedor responde com um datagrama da porta 53 para a 51000, e o sistema do laptop usa essa porta para entregar a resposta ao programa que perguntou. A consulta inteira levou uma ida e volta. Se um dos pacotes se perdesse, nada no UDP perceberia: a biblioteca do resolvedor espera um pouco e simplesmente pergunta de novo — decisão dela, não do UDP.
packets-udp-p2 = Aqui está sua resposta, para a porta 51000.

packets-udp-3-title = Um fluxo de atualizações
packets-udp-3-sub = Um jogo diz onde você está, 20 vezes por segundo
packets-udp-3-body = Um jogo online envia ao seu servidor um pequeno datagrama a cada 50 ms com os últimos movimentos do jogador, e o servidor devolve o estado do mundo ao redor dele. Nenhum dos lados abre nada nem espera confirmação; cada datagrama vale por si. O próprio jogo numera suas atualizações, dentro dos dados, já que o UDP não numera nada.
packets-udp-p3 = Atualização 1041: aqui é onde estou agora.
packets-udp-p4 = O mundo ao seu redor, neste instante.
packets-udp-p5 = Atualização 1042: onde estou agora.

packets-udp-4-title = Um datagrama perdido
packets-udp-4-sub = Some, e ninguém percebe
packets-udp-4-body = A atualização 1043 é descartada no caminho — por um roteador com a fila cheia, ou por uma rajada de interferência no Wi-Fi. O laptop não guardou cópia e não espera confirmação, então nada é reenviado; o servidor só fica sabendo que houve uma 1043 quando a 1044 vem em seguida. Um datagrama cujo checksum falha é tratado do mesmo jeito: é jogado fora em silêncio.
packets-udp-4-state = Perdido, e não reenviado
packets-udp-p6 = Atualização 1043: onde estou agora.

packets-udp-5-title = O aplicativo decide
packets-udp-5-sub = O dado novo torna inútil o perdido
packets-udp-5-body = A atualização 1044 chega, e o servidor vê que falta a 1043. Ele não a pede: a 1044 já traz a posição mais nova, e a antiga, reenviada, só chegaria atrasada. Uma chamada de voz faz o mesmo, disfarçando 20 ms de som perdido em vez de esperar por eles; um cliente DNS pergunta de novo; um programa que precisa de cada byte usa TCP, ou constrói as próprias confirmações, como faz o QUIC.
packets-udp-5-reason = O dado novo substitui o que se perdeu
packets-udp-p7 = Atualização 1044: onde estou agora.
packets-udp-p8 = O mundo ao seu redor — com a sua posição mais nova.

packets-udp-6-title = UDP ou TCP
packets-udp-6-sub = Qual escolher
packets-udp-6-body = Escolha UDP quando uma pergunta e sua resposta cabem num pacote cada, quando dado atrasado é pior que dado perdido — chamadas, vídeo ao vivo, jogos — ou quando um pacote precisa chegar a muitas máquinas de uma vez (broadcast, multicast), o que o TCP não faz. Escolha TCP quando tudo precisa chegar, inteiro e em ordem: páginas, arquivos, e-mail. Com um cabeçalho de 8 bytes contra 20 ou mais do TCP, e sem handshake, o UDP é o mais leve dos dois.
packets-udp-6-udp = Perguntas curtas, chamadas, jogos, broadcast
packets-udp-6-tcp = Páginas, arquivos, e-mail: o que precisa chegar inteiro

## QUIC (HTTP/3)

packets-quic-name = QUIC (HTTP/3)
packets-quic-title = Um transporte multiplexado e seguro sobre UDP
packets-quic-tagline = A confiabilidade do TCP e a criptografia do TLS, juntas numa só ida e volta
packets-quic-overview = O QUIC é um protocolo de transporte que roda sobre UDP e faz num só handshake o que o TCP e o TLS fazem em dois: uma conexão confiável e criptografada fica pronta depois de uma única ida e volta, com o TLS 1.3 embutido em vez de empilhado por cima. Uma conexão carrega muitos streams independentes, então um pacote perdido atrasa só o stream a que pertencia. A conexão é reconhecida pelos seus connection IDs, não por endereços e portas, e por isso sobrevive a uma troca de rede — um celular saindo do Wi-Fi, por exemplo. O HTTP/3 é o HTTP levado sobre o QUIC, e quase tudo num pacote QUIC vai criptografado — até o número do pacote, no cabeçalho.
packets-quic-encryption = Sempre ligada: TLS 1.3, embutido
packets-quic-reliability = Por stream: frames perdidos vão de novo em pacotes novos
packets-quic-device-phone-wifi = Celular, no Wi-Fi
packets-quic-device-phone-mobile = O mesmo celular, nos dados móveis
packets-quic-label-cid = Connection ID

packets-quic-1-title = Initial do cliente
packets-quic-1-sub = O primeiro pacote já leva o TLS
packets-quic-1-body = O celular já sabe — pelo registro DNS HTTPS do site ou por um cabeçalho Alt-Svc anterior — que www.example.com fala HTTP/3, então pula o TCP e envia um pacote QUIC Initial para a porta UDP 443. Dentro, num frame CRYPTO, vai um ClientHello do TLS 1.3: a key share do celular, o nome que ele quer (SNI) e o ALPN h3. O datagrama é completado até 1.200 bytes porque, enquanto o endereço do celular não for comprovado, o servidor só pode devolver até três vezes o que recebeu — assim o QUIC não serve para inundar um endereço forjado. Os pacotes Initial são cifrados com chaves derivadas do connection ID, que qualquer um pode calcular: afastam middleboxes intrometidos, não quem escuta.
packets-quic-p1 = Olá! Minha key share, o site que eu quero e HTTP/3, por favor.

packets-quic-2-title = A resposta do servidor
packets-quic-2-sub = Chaves, certificado e Finished de uma vez
packets-quic-2-body = O servidor escolhe o seu próprio connection ID e responde com um pacote Initial com o seu ServerHello — a parte dele da troca de chaves. Nos mesmos datagramas vêm pacotes Handshake, já cifrados com as chaves novas: o certificado de www.example.com, uma assinatura CertificateVerify que prova que o servidor tem a chave privada do certificado, e o Finished. O TCP teria gastado essa ida e volta inteira só com SYN e SYN-ACK; o QUIC a usou para abrir a conexão e combinar as chaves ao mesmo tempo.
packets-quic-p2 = A minha parte da troca de chaves — e o meu próprio connection ID.
packets-quic-p3 = Meu certificado de www.example.com, minha assinatura, Finished.

packets-quic-3-title = Finished e pedidos
packets-quic-3-sub = Os dados andam depois de uma ida e volta
packets-quic-3-body = O celular confere o certificado contra as autoridades em que confia e envia o seu próprio Finished num pacote Handshake. Logo atrás vai um pacote 1-RTT com o cabeçalho curto — sem versão, sem ID de origem, só o connection ID do servidor — levando dois pedidos HTTP/3: a página no stream 0 e o logo dela no stream 4. Os pedidos saem uma ida e volta depois do primeiro pacote; TCP seguido de TLS 1.3 precisaria de duas.
packets-quic-p4 = Seu certificado confere. Finished.
packets-quic-p5 = Stream 0: GET /. Stream 4: GET /logo.png.

packets-quic-4-title = Streams independentes
packets-quic-4-sub = Uma perda atrasa só o próprio stream
packets-quic-4-body = O pacote com o logo se perde no caminho, mas o seguinte, com a página, chega — e o QUIC entrega a página ao navegador na hora, porque cada stream guarda a sua própria ordem. Sobre TCP, a página teria esperado atrás dos bytes que faltavam: o TCP é um stream só, e esse bloqueio de cabeça de fila (head-of-line blocking) era o que deixava o HTTP/2 lento. O ACK do celular informa que o pacote 1 chegou e o 0 não, então o servidor manda os dados do logo de novo num pacote novo, o número 2 — o QUIC nunca usa um número de pacote duas vezes.
packets-quic-4-state = A página aparece; só o logo espera o reenvio
packets-quic-p6 = Stream 4: 200 e o logo — mas eu me perco no caminho.
packets-quic-p7 = Stream 0: 200 e a página. E o handshake terminou.
packets-quic-p8 = Recebi o seu pacote 1; o 0 nunca chegou.
packets-quic-p9 = O logo de novo: os mesmos bytes, num pacote novo, o número 2.

packets-quic-5-title = Troca de rede
packets-quic-5-sub = A conexão acompanha o celular
packets-quic-5-body = Ao sair de casa, o celular perde o Wi-Fi e passa para os dados móveis: endereço novo e porta nova. Uma conexão TCP fica presa aos seus endereços e portas, então cairia aqui e todo download recomeçaria. O QUIC acha a conexão pelo ID dela: o celular envia o próximo pacote do endereço novo com um ID novo que o servidor lhe deu antes (NEW_CONNECTION_ID), assim quem observa não consegue ligar as duas redes, mas o servidor sabe de que conexão se trata. Antes de mandar muita coisa para lá, o servidor confere se o celular está mesmo no endereço novo: o PATH_CHALLENGE leva oito bytes aleatórios, e o PATH_RESPONSE tem de devolvê-los.
packets-quic-p10 = Ainda sou eu: endereço novo, connection ID novo.
packets-quic-p11 = Prove que você está neste endereço: me devolva estes 8 bytes.
packets-quic-p12 = Aqui estão os seus 8 bytes.

packets-quic-6-title = Retomada em 0-RTT
packets-quic-6-sub = Da próxima vez, o pedido vai primeiro
packets-quic-6-body = Durante a conexão, o servidor também deu ao celular um ticket de sessão (NewSessionTicket). No dia seguinte o celular retoma com ele e envia o pedido num pacote 0-RTT, no mesmo datagrama do ClientHello e cifrado com chaves da sessão anterior — antes de qualquer resposta voltar, então a resposta pode chegar já na primeira leva de pacotes do servidor. Um atacante poderia repetir os dados 0-RTT, por isso os navegadores só põem neles pedidos que podem ser repetidos sem risco, e um servidor pode recusá-los e esperar o handshake terminar.
packets-quic-p13 = Eu de novo, com o meu ticket — e o GET / já a bordo.

## DNSSEC

packets-dnssec-name = DNSSEC
packets-dnssec-title = Extensões de Segurança do DNS
packets-dnssec-tagline = Respostas DNS assinadas, conferidas a partir da raiz
packets-dnssec-overview = O DNS comum não tem como provar que uma resposta é verdadeira: uma resposta forjada que chega primeiro é simplesmente aceita. O DNSSEC acrescenta assinaturas. Cada zona assina seus registros com a própria chave privada e publica a chave pública num registro DNSKEY; a zona-mãe garante essa chave com um registro DS, assinado por sua vez com a chave dela — uma cadeia que termina na raiz, cuja chave todo resolvedor validador já tem. O DNSSEC prova que as respostas são autênticas e não foram alteradas, mas não as esconde: perguntas e respostas continuam andando às claras.
packets-dnssec-security = Autenticidade e integridade, não sigilo
packets-dnssec-label-ksk = Chave de assinatura de chaves (KSK)
packets-dnssec-label-zsk = Chave de assinatura da zona (ZSK)

packets-dnssec-1-title = Pergunta com DO
packets-dnssec-1-sub = O resolvedor também quer as assinaturas
packets-dnssec-1-body = O cliente pergunta ao seu resolvedor por www.example.com como no DNS comum, com o bit AD ligado: quer saber se a resposta foi validada. O resolvedor, que já sabe onde ficam os servidores de example.com (a busca do DNS mostra como ele os acha), pergunta a um deles com o bit DO — "DNSSEC OK" — ligado. O DO vai num registro OPT do EDNS na seção adicional, contado pelo ARCOUNT, e pede ao servidor que mande junto as assinaturas de cada registro.
packets-dnssec-p1 = Qual é o endereço de www.example.com — e ele foi validado?
packets-dnssec-p2 = Qual é o endereço de www.example.com? Mande as assinaturas também.

packets-dnssec-2-title = Registro e assinatura
packets-dnssec-2-sub = A resposta vem assinada
packets-dnssec-2-body = O servidor responde com o registro A e, ao lado dele, um RRSIG: uma assinatura sobre todos os registros A de www.example.com, feita com a chave privada de assinatura da zona. O RRSIG diz o algoritmo (13: ECDSA P-256 com SHA-256), a chave que assinou, pela tag dela, e as datas entre as quais a assinatura vale, para que uma antiga não possa ser reaproveitada para sempre. Para conferi-la, o resolvedor precisa da chave pública da zona.
packets-dnssec-p3 = 203.0.113.10 — assinado com a chave de example.com.

packets-dnssec-3-title = As chaves da zona
packets-dnssec-3-sub = DNSKEY, assinado por si mesmo
packets-dnssec-3-body = O resolvedor pede os registros DNSKEY de example.com e recebe duas chaves públicas: uma chave de assinatura da zona (flags 256), que assinou o registro A, e uma chave de assinatura de chaves (flags 257), que assina só o conjunto de DNSKEYs. Com elas, o RRSIG do registro A confere. Mas qualquer um pode publicar chaves e assinar com elas — o que prova que estas são mesmo de example.com?
packets-dnssec-p4 = Quais são as chaves públicas de example.com?
packets-dnssec-p5 = Aqui estão minhas duas chaves, assinadas pela de assinatura de chaves.

packets-dnssec-4-title = Cadeia de confiança
packets-dnssec-4-sub = Cada zona-mãe garante a filha
packets-dnssec-4-body = A zona-mãe prova: os servidores do .com publicam um registro DS para example.com — um resumo SHA-256 da chave de assinatura de chaves dele — assinado com a chave do próprio .com. O resolvedor confere a chave do .com do mesmo jeito, contra o registro DS de com na zona raiz, assinado pela raiz. A chave da raiz é a âncora de confiança, embutida em todo resolvedor validador, e assim a cadeia se fecha: raiz, com, example.com, www.example.com. As chaves da raiz e do .com são pedidas com tanta frequência que quase sempre já estão no cache.
packets-dnssec-4-trust = A chave da raiz, embutida no resolvedor
packets-dnssec-p6 = Qual é o registro DS de example.com?
packets-dnssec-p7 = O resumo da chave de example.com — assinado pelo .com.
packets-dnssec-p8 = Qual é o registro DS de com?
packets-dnssec-p9 = O resumo da chave do .com — assinado pela raiz.

packets-dnssec-5-title = Resposta validada
packets-dnssec-5-sub = AD: dados autênticos
packets-dnssec-5-body = Todos os elos conferem, então o resolvedor entrega o endereço ao cliente com o bit AD (authentic data) ligado e o guarda, com assinaturas e tudo, pelo TTL do registro. O cliente confia na palavra do resolvedor: o bit AD só é tão seguro quanto o caminho até o resolvedor, e por isso o melhor é falar com um resolvedor validador por um canal cifrado, como o DNS over HTTPS, ou rodá-lo na própria máquina.
packets-dnssec-5-verdict = Seguro
packets-dnssec-p10 = 203.0.113.10 — e todas as assinaturas conferiram.

packets-dnssec-6-title = Uma resposta forjada
packets-dnssec-6-sub = Sem assinatura válida, sem resposta
packets-dnssec-6-body = Uma hora depois o registro expirou e o cliente pergunta de novo. Antes que o servidor verdadeiro responda, um atacante responde no lugar dele, forjando o servidor de example.com como origem e pondo o próprio endereço, 192.0.2.66, no registro. Sem DNSSEC, uma falsificação que acertasse o ID e a porta da consulta seria aceita e entregue a todos que perguntassem (envenenamento de cache); aqui ela não traz assinatura válida da chave de example.com, então é falsa (bogus), e o resolvedor responde SERVFAIL em vez de entregar um endereço que não consegue provar.
packets-dnssec-6-verdict = Bogus: sem assinatura válida
packets-dnssec-p11 = Qual é o endereço de www.example.com, de novo?
packets-dnssec-p12 = Qual é o endereço de www.example.com? Com assinaturas, por favor.
packets-dnssec-p13 = Sou o servidor de example.com, palavra: é 192.0.2.66.
packets-dnssec-p14 = Não consegui validar a resposta: SERVFAIL.

## DoH

packets-doh-name = DoH
packets-doh-title = DNS over HTTPS
packets-doh-tagline = Perguntas DNS dentro de uma conexão web cifrada
packets-doh-overview = O DNS comum anda às claras: toda rede que suas perguntas atravessam pode ver quais nomes você procura, e pode trocar as respostas. O DNS over HTTPS põe exatamente as mesmas mensagens DNS dentro de pedidos HTTPS a um resolvedor que você escolhe, na porta 443, então o TLS as cifra e prova quem responde, e elas andam como qualquer outro tráfego web. A rede no meio do caminho continua vendo com quem você fala, mas não mais o que você pergunta; em troca, o resolvedor escolhido vê todas as perguntas.
packets-doh-format = Mensagens DNS como são, com o tipo application/dns-message

packets-doh-1-title = DNS comum
packets-doh-1-sub = Uma pergunta que qualquer um no caminho lê
packets-doh-1-body = Primeiro o jeito antigo, para comparar: o laptop pergunta ao resolvedor por www.example.com num pacote UDP para a porta 53. Nada nele é cifrado nem autenticado, então o roteador do provedor — e qualquer outro no caminho — pode ler o nome, registrá-lo, bloqueá-lo ou até responder no lugar do resolvedor com um endereço seu.
packets-doh-p1 = Qual é o endereço de www.example.com?
packets-doh-p2 = É 203.0.113.10.

packets-doh-2-title = HTTPS até o resolvedor
packets-doh-2-sub = Uma conexão TLS comum, que fica aberta
packets-doh-2-body = Agora com DoH. O navegador está configurado com o modelo de endereço do resolvedor, https://dns.example.net/dns-query, e abre até ele uma conexão HTTPS comum na porta 443: TCP, depois o handshake do TLS 1.3. O certificado do resolvedor para dns.example.net prova que o navegador fala com o resolvedor que escolheu, e não com um impostor. A conexão fica aberta e é reaproveitada em todas as consultas seguintes, então o custo dela é pago uma vez só.
packets-doh-p3 = Olá, dns.example.net — vamos falar HTTP/2.
packets-doh-p4 = Meu certificado de dns.example.net, e Finished.

packets-doh-3-title = A pergunta num POST
packets-doh-3-sub = Uma mensagem DNS como corpo de um pedido
packets-doh-3-body = A pergunta em si é a mesma mensagem DNS da porta 53 — byte a byte, só que com o ID 0 — enviada como corpo de um POST HTTP para /dns-query, com o tipo application/dns-message. A resposta volta como um 200 com a resposta DNS no mesmo formato e um tempo de cache que não passa do TTL do registro. Dentro da conexão é HTTP/2 comum, então várias consultas podem correr lado a lado nela.
packets-doh-p5 = Uma pergunta DNS para você, num POST: www.example.com, A.
packets-doh-p6 = 200: a resposta DNS, 203.0.113.10.

packets-doh-4-title = Ou num GET
packets-doh-4-sub = A pergunta na URL
packets-doh-4-body = A mesma pergunta também pode ir na URL: a mensagem DNS é codificada em base64url e enviada como GET /dns-query?dns=…. Como o ID é sempre 0, a mesma pergunta gera sempre a mesma URL, então os caches HTTP comuns pelo caminho também podem respondê-la, pelo tempo que o TTL do registro permitir.
packets-doh-p7 = A mesma pergunta, desta vez na URL.
packets-doh-p8 = 200 — e qualquer cache pode guardar isto por uma hora.

packets-doh-5-title = O que a rede vê
packets-doh-5-sub = A quem você pergunta, não o quê
packets-doh-5-body = O roteador do provedor agora só vê registros TLS entre o laptop e 198.51.100.1 na porta 443: que o laptop usa esse resolvedor, quando e quanto — mas não os nomes nem as respostas, e não consegue alterá-los sem o TLS perceber. A confiança passou para o resolvedor, que vê todas as perguntas. E a próxima conexão do navegador, ao próprio www.example.com, ainda mostra esse nome no handshake TLS dela, a não ser que o Encrypted Client Hello o esconda.
packets-doh-5-trust = O resolvedor agora vê todas as perguntas

## mDNS / DNS-SD

packets-mdns-name = mDNS / DNS-SD
packets-mdns-title = Multicast DNS e descoberta de serviços pelo DNS
packets-mdns-tagline = Achar nomes e serviços na rede local, sem servidor
packets-mdns-overview = Em casa ou num escritório pequeno, quase nunca há um servidor DNS que saiba o nome da impressora. O Multicast DNS deixa cada aparelho responder por si: perguntas sobre nomes terminados em .local vão para um endereço multicast que todos os aparelhos escutam, e quem é dono do nome responde. A descoberta de serviços pelo DNS (DNS-SD) usa as mesmas mensagens para perguntar o que está sendo oferecido — cada impressora, caixa de som ou compartilhamento de arquivos — com registros PTR, SRV e TXT comuns. A Apple chama a dupla de Bonjour; no Linux, ela é o Avahi.
packets-mdns-used-by = Impressoras, caixas de som, espelhamento de tela, compartilhamento de arquivos
packets-mdns-device-lan = Rede local

packets-mdns-1-title = Procurando
packets-mdns-1-sub = Que impressoras há por aqui?
packets-mdns-1-body = Você abre a janela de impressão, e o laptop pergunta à rede inteira de uma vez: um pacote UDP para o endereço multicast 224.0.0.251, porta 5353, pedindo os registros PTR de _ipp._tcp.local — toda impressora que fala IPP. Não há servidor a quem perguntar: a rede entrega o pacote a todos os aparelhos, e cada um que roda mDNS o lê. Pacotes para 224.0.0.251 nunca atravessam um roteador, então a pergunta fica nesta rede.
packets-mdns-p1 = Tem alguma impressora que fala IPP por aqui?

packets-mdns-2-title = A impressora responde
packets-mdns-2-sub = PTR, SRV, TXT e A numa só resposta
packets-mdns-2-body = A impressora reconhece um serviço que oferece e responde — também para o endereço multicast, para que todo aparelho da rede possa atualizar o seu cache de passagem. O registro PTR dá o nome da instância do serviço, printer._ipp._tcp.local; na seção adicional vão o registro SRV com o host e a porta (printer.local:631), o TXT com as propriedades dela, como o caminho da fila rp=ipp/print, e o A com o endereço, 192.168.1.40. Mandar os quatro de uma vez poupa o laptop de mais três perguntas.
packets-mdns-p2 = Eu: printer._ipp._tcp.local, em printer.local, porta 631 — ou seja, 192.168.1.40.

packets-mdns-3-title = Cache e conexão
packets-mdns-3-sub = O laptop vai direto à impressora
packets-mdns-3-body = O laptop guarda cada registro pelo tempo que o TTL dele diz: 75 minutos para o PTR e o TXT, 2 minutos para o SRV e o A, que nomeiam um host e mudam com mais frequência. A impressora agora aparece na janela, e imprimir é uma conversa direta: uma conexão TCP com 192.168.1.40, porta 631, e IPP daí em diante. Enquanto a janela está aberta, o laptop pergunta de novo antes de cada TTL acabar, então uma impressora que saiu da rede some da lista.
packets-mdns-p3 = Abrindo uma conexão com você, na porta 631.

packets-mdns-4-title = Um aparelho chega
packets-mdns-4-sub = O nome desktop.local está livre?
packets-mdns-4-body = Um computador de mesa entra na rede e quer se chamar desktop.local. Como nenhum servidor distribui nomes .local, ele primeiro pergunta se alguém já o usa: três sondas, com um quarto de segundo entre elas, cada uma perguntando por desktop.local e levando, na seção de autoridade, o registro que ele pretende reivindicar. Se alguém respondesse, ele escolheria outro nome, como desktop-2.local, e tentaria de novo; o silêncio quer dizer que o nome é dele.
packets-mdns-p4 = Alguém está usando o nome desktop.local?

packets-mdns-5-title = Anúncio
packets-mdns-5-sub = Aqui estou, e é isto que ofereço
packets-mdns-5-body = Com o nome garantido, o computador se anuncia: uma resposta espontânea para 224.0.0.251 com o seu endereço e o seu serviço de compartilhamento de arquivos, _smb._tcp, na porta 445 — enviada duas vezes, com um segundo entre elas, caso uma se perca. Todo aparelho da rede a guarda no cache, então da próxima vez que alguém procurar compartilhamentos, o computador já estará na lista. Quando ele sai, manda os mesmos registros com TTL 0, uma despedida que os apaga de todos os caches em um segundo.
packets-mdns-p5 = Sou desktop.local, em 192.168.1.30, e compartilho arquivos na porta 445.

## HTTPS

packets-https-name = HTTPS
packets-https-title = HTTP sobre TLS
packets-https-tagline = A web cifrada, com a identidade do servidor provada
packets-https-overview = O HTTPS é o HTTP comum enviado dentro de uma conexão TLS. O TLS acrescenta três coisas que faltam ao HTTP: criptografia, para que ninguém no caminho leia as páginas, senhas ou cookies; integridade, para que ninguém as altere sem ser notado; e autenticação, um certificado que prova que o servidor é mesmo www.example.com. Antes do primeiro pedido, navegador e servidor fazem o handshake do TLS — uma única ida e volta no TLS 1.3 — para combinar chaves novas. O cadeado na barra de endereço quer dizer que o handshake deu certo e o certificado conferiu.
packets-https-authentication = O certificado do servidor, assinado por uma autoridade em que o navegador confia

packets-https-1-title = Endereço e conexão
packets-https-1-sub = DNS e TCP, como no HTTP
packets-https-1-body = Tudo começa como no HTTP comum: o laptop pergunta ao DNS o endereço de www.example.com, recebe 203.0.113.10 e abre uma conexão TCP — com a porta 443 em vez da 80. Nada está protegido ainda: o nome perguntado ao DNS, os endereços e as portas estão à vista de qualquer um no mesmo Wi-Fi, inclusive do atacante.
packets-https-p1 = Qual é o endereço de www.example.com?
packets-https-p2 = É 203.0.113.10.
packets-https-p3 = Vamos conectar, na porta 443?
packets-https-p4 = Sim — estou escutando na 443.
packets-https-p5 = Conectado.

packets-https-2-title = ClientHello
packets-https-2-sub = Olá, e metade de uma troca de chaves
packets-https-2-body = O navegador abre o TLS com um ClientHello: as versões e cifras que aceita, um número aleatório, o nome do site que quer (SNI: www.example.com, para que um servidor possa hospedar muitos sites) e uma key share. A key share aqui é X25519MLKEM768 — uma chave clássica X25519 e uma pós-quântica ML-KEM-768 lado a lado, para que a sessão continue secreta enquanto qualquer uma das duas resistir, mesmo contra um computador quântico que um dia decifre o tráfego gravado. O ClientHello em si não é cifrado: o atacante consegue ler nele o nome do site, a não ser que o Encrypted Client Hello o esconda.
packets-https-p6 = Olá: quero www.example.com, e aqui está a minha key share.

packets-https-3-title = A resposta do servidor
packets-https-3-sub = A key share dele, depois a prova
packets-https-3-body = O servidor escolhe uma cifra e responde com um ServerHello que leva a key share dele: a sua chave X25519 e um texto cifrado ML-KEM que só o navegador consegue abrir. Os dois lados agora calculam o mesmo segredo, e tudo depois do ServerHello vai cifrado: o certificado de www.example.com, uma assinatura CertificateVerify que prova que o servidor tem a chave privada do certificado, e o Finished, um MAC de todo o handshake. O navegador confere se o certificado nomeia www.example.com, está na validade e leva, por certificados intermediários, a uma autoridade em que ele confia — e então manda o seu próprio Finished.
packets-https-p7 = Minha key share — e, cifrados, meu certificado, minha assinatura e o Finished.
packets-https-p8 = Seu certificado confere. Finished.

packets-https-4-title = O pedido cifrado
packets-https-4-sub = GET e 200 OK, dentro do TLS
packets-https-4-body = Agora o navegador envia GET / — exatamente o pedido que o HTTP comum enviaria —, mas dentro de um registro TLS do tipo application data, cifrado e lacrado com as chaves da sessão. O servidor responde 200 OK e a página do mesmo jeito. Na rede, o atacante só vê registros desse tipo e os tamanhos deles; o caminho, os cabeçalhos, os cookies e a página estão todos lá dentro, e a anatomia os abre como só o navegador e o servidor conseguem.
packets-https-p9 = GET / — lacrado num registro TLS.
packets-https-p10 = 200 OK e a página, lacrados do mesmo jeito.

packets-https-5-title = O cadeado
packets-https-5-sub = O que sobra para o atacante
packets-https-5-body = O navegador mostra o cadeado: a conexão está cifrada e o servidor provou o seu nome. O atacante no Wi-Fi ainda vê os endereços, as portas, os horários, os tamanhos e — no ClientHello — o nome do site, mas não quais páginas, nem o que você digita ou recebe. Mudar um único byte quebraria a tag de autenticação do registro e encerraria a conexão, e se passar pelo servidor exigiria um certificado de www.example.com emitido por uma autoridade em que o navegador confia, que só o dono do site consegue obter.
packets-https-5-state = Cifrada, e o nome do servidor provado

## WebSocket

packets-websocket-name = WebSocket
packets-websocket-title = The WebSocket Protocol
packets-websocket-tagline = Um canal de mão dupla duradouro entre uma página e seu servidor
packets-websocket-overview = No HTTP simples o navegador pergunta e o servidor responde: o servidor nunca fala primeiro. O WebSocket transforma uma conexão HTTP num canal que fica aberto e leva mensagens nos dois sentidos, sempre que um dos lados tem algo a dizer. Ele começa como um pedido HTTP comum que pede um upgrade; quando o servidor aceita, a mesma conexão TCP passa a levar pequenos quadros em vez de pedidos e respostas. Chats, painéis ao vivo, editores compartilhados e jogos no navegador são feitos sobre ele.
packets-websocket-port = 80 (ws://), 443 (wss://, sobre TLS)
packets-websocket-format = Quadros com mensagens de texto (UTF-8) ou binárias
packets-websocket-used-by = Chats, cotações e placares ao vivo, editores compartilhados, jogos no navegador
packets-websocket-label-mask = Chave de máscara

packets-websocket-1-title = O pedido de upgrade
packets-websocket-1-sub = Um pedido HTTP pede para trocar de protocolo
packets-websocket-1-body = A página abre ws://www.example.com/chat. Por uma nova conexão TCP o navegador envia um GET comum com dois cabeçalhos a mais, Upgrade: websocket e Connection: Upgrade, além do Sec-WebSocket-Key — 16 bytes aleatórios em base64 — e da versão, 13. Começar como HTTP deixa o WebSocket usar as próprias portas da web e passar pelos mesmos firewalls e proxies; com wss:// tudo isso viaja dentro do TLS.
packets-websocket-p1 = Podemos transformar esta conexão num WebSocket? Aqui está minha chave.

packets-websocket-2-title = Trocando de protocolo
packets-websocket-2-sub = O servidor aceita, e prova que entendeu
packets-websocket-2-body = O servidor responde 101 Switching Protocols em vez de 200 OK. Para mostrar que fala mesmo WebSocket — e que não é um cache repetindo uma resposta velha —, ele junta um GUID fixo à chave do cliente, calcula o SHA-1 do resultado e o devolve em base64 como Sec-WebSocket-Accept. O navegador confere esse valor; a partir do próximo byte, a conexão leva quadros WebSocket, não mais HTTP.
packets-websocket-p2 = Combinado. Aqui está sua chave, com hash: prova de que entendi.

packets-websocket-3-title = Um quadro mascarado
packets-websocket-3-sub = A página envia uma mensagem
packets-websocket-3-body = Toda mensagem viaja em quadros: um cabeçalho de 2 a 14 bytes, depois o payload. O opcode diz o que ele leva — 1 é texto, 2 é binário — e o FIN marca o último quadro de uma mensagem. Todo quadro que sai do navegador é mascarado: o payload passa por um XOR com uma chave aleatória nova de 4 bytes, levada no próprio quadro, para que uma página mal-intencionada não consiga escrever bytes que um proxy antigo no caminho tomaria por um pedido HTTP — guardando em cache uma resposta forjada para ele.
packets-websocket-3-opcode = 1 — texto
packets-websocket-p3 = Me mande cada mudança no preço de EXA.

packets-websocket-4-title = O servidor empurra
packets-websocket-4-sub = Mensagens que ninguém pediu
packets-websocket-4-body = Agora o servidor fala sempre que tem novidade: cada vez que o preço muda, ele manda um quadro, sem nenhum pedido antes. O HTTP simples não consegue isso — nele o servidor só responde —, então antes do WebSocket as páginas perguntavam de novo a cada poucos segundos (polling) ou deixavam um pedido pendurado até haver novidade. Os quadros do servidor nunca são mascarados: o ataque que a máscara impede só pode partir de uma página.
packets-websocket-p4 = EXA agora está a 42,17.
packets-websocket-p5 = E agora a 42,19.

packets-websocket-5-title = Ping e pong
packets-websocket-5-sub = Ainda tem alguém aí?
packets-websocket-5-body = Uma conexão pode ficar em silêncio por minutos, e um roteador ou proxy no caminho pode esquecê-la — ou o outro lado pode sumir sem avisar. Por isso o servidor manda um ping (opcode 9) de tempos em tempos, e o navegador tem de responder com um pong (opcode 10) com os mesmos dados; ele faz isso sozinho, sem a página saber. Um pong que nunca chega diz ao servidor que a conexão morreu.
packets-websocket-5-interval = Escolhido pelo servidor, muitas vezes de 20 a 30 segundos
packets-websocket-p6 = Ping. Ainda está aí?
packets-websocket-p7 = Pong. Ainda aqui.

packets-websocket-6-title = Encerramento
packets-websocket-6-sub = Quadros de close, depois o TCP
packets-websocket-6-body = Para terminar, o navegador manda um quadro de close (opcode 8) com um código de status: 1000 é um encerramento normal, 1001 quer dizer que está indo embora, como quando se sai da página. O servidor responde com um quadro de close seu e então fecha a conexão TCP; nenhum lado envia nada depois do seu close, então nenhuma mensagem ainda a caminho se perde. Uma conexão que simplesmente cai é informada à página como 1006, um código que nunca vai pelo fio.
packets-websocket-6-state = Fechada de forma limpa
packets-websocket-p8 = Estou encerrando: 1000, tudo certo.
packets-websocket-p9 = Encerrando também: 1000. Tchau.

## WebRTC

packets-webrtc-name = WebRTC
packets-webrtc-title = Web Real-Time Communication
packets-webrtc-tagline = Chamadas e dados direto de navegador para navegador
packets-webrtc-overview = O WebRTC deixa dois navegadores enviarem um ao outro som, vídeo e dados diretamente, sem um servidor carregando a chamada. A parte difícil é achar um caminho entre duas máquinas que estão, ambas, atrás de roteadores: por um servidor de sinalização escolhido pelo app, elas trocam descrições de si mesmas (SDP), descobrem seus endereços públicos com um servidor STUN, testam pares de endereços até um passar (ICE) e guardam um retransmissor TURN como último recurso. O que passa depois é sempre criptografado: as chaves são combinadas com DTLS, e a mídia viaja como SRTP.
packets-webrtc-port = Nenhuma fixa: escolhida a cada chamada; STUN e TURN escutam na 3478 (5349 sobre TLS)
packets-webrtc-transport = UDP; TCP ou TLS por um retransmissor TURN quando o UDP está bloqueado
packets-webrtc-encryption = Sempre ligada: DTLS para as chaves e os dados, SRTP para a mídia

packets-webrtc-1-title = Oferta e resposta
packets-webrtc-1-sub = Pelo servidor de sinalização
packets-webrtc-1-body = Os dois navegadores ainda não se alcançam, então conversam pelo servidor do próprio app, em geral por um WebSocket. Quem liga envia uma oferta escrita em SDP: a mídia que quer enviar, os codecs que conhece, um fragmento de usuário e uma senha para as verificações que virão e o fingerprint do seu certificado; o outro lado devolve uma resposta do mesmo tipo. O WebRTC não diz como elas viajam — só o que precisam levar.
packets-webrtc-p1 = Oferta: áudio e vídeo nestes codecs, e o fingerprint do meu certificado.
packets-webrtc-p2 = Uma chamada para você: aqui está a oferta de quem liga.
packets-webrtc-p3 = Resposta: áudio e vídeo também, estes codecs, e o meu fingerprint.
packets-webrtc-p4 = Responderam: aqui está a descrição deles.

packets-webrtc-2-title = Qual é o meu endereço?
packets-webrtc-2-sub = Perguntando a um servidor STUN
packets-webrtc-2-body = O laptop só conhece seu endereço privado, 192.168.1.10, que ninguém lá fora alcança. Então ele envia um STUN Binding Request a um servidor na internet, que responde com o endereço e a porta de onde o pedido chegou: 198.51.100.7, porta 54321 — o mapeamento que o roteador fez. Esse par vira um candidato server-reflexive (srflx) e, como todo candidato, vai para o outro lado pelo servidor de sinalização; o outro laptop faz o mesmo.
packets-webrtc-p5 = De que endereço você me vê chegando?
packets-webrtc-p6 = De 198.51.100.7, porta 54321.

packets-webrtc-3-title = Um retransmissor de reserva
packets-webrtc-3-sub = Um endereço próprio num servidor TURN
packets-webrtc-3-body = Alguns roteadores não deixam passar caminho direto nenhum — um NAT simétrico dá a cada destino uma porta diferente, que o outro lado não tem como adivinhar. Para esse caso o laptop também pede a um servidor TURN, com credenciais que o app lhe deu, que reserve um endereço de retransmissão: 203.0.113.30, porta 49152. O que o outro lado mandar para lá é repassado ao laptop, ao custo de um desvio e da banda do retransmissor, por isso o ICE dá a esse candidato a menor prioridade.
packets-webrtc-p7 = Por favor, guarde um endereço para mim e repasse o que chegar nele.
packets-webrtc-p8 = Seu: 203.0.113.30, porta 49152, por 600 segundos.

packets-webrtc-4-title = Verificações de conectividade
packets-webrtc-4-sub = Testando pares de endereços
packets-webrtc-4-body = Cada lado junta seus candidatos com os do outro e os testa, dos melhores para os piores, com STUN Binding Requests assinados com a senha do SDP. Os dois enviam quase ao mesmo tempo: cada verificação que sai abre um mapeamento no próprio roteador, por onde a verificação do outro lado consegue entrar — é assim que se atravessam dois NATs. Aqui o par direto responde, então a chamada vai de par a par; se todos os pares diretos tivessem falhado, o par pelo retransmissor TURN teria sido escolhido.
packets-webrtc-4-verdict = O par direto funciona: sem retransmissor
packets-webrtc-p9 = Você me ouve por este caminho? Assinado com a sua senha.
packets-webrtc-p10 = Sim — e você me alcança a partir de 198.51.100.7, porta 54321.

packets-webrtc-5-title = Handshake DTLS
packets-webrtc-5-sub = Combinando chaves, de par a par
packets-webrtc-5-body = No caminho escolhido os navegadores rodam o DTLS, o TLS adaptado ao UDP; o lado que respondeu com a=setup:active faz o papel de cliente. Os certificados são autoassinados — nenhuma autoridade responde por eles —, então cada um confere se o do outro bate com o fingerprint recebido no SDP: é um canal de sinalização confiável que mantém a chamada privada. O handshake rende as chaves do SRTP (DTLS-SRTP), e o canal de dados viaja dentro da própria conexão DTLS.
packets-webrtc-5-trust = O certificado bate com o fingerprint do SDP
packets-webrtc-p11 = Olá: vamos combinar as chaves. Eu faço o papel de cliente.
packets-webrtc-p12 = Aqui está meu certificado; agora me mostre o seu.

packets-webrtc-6-title = A mídia passa
packets-webrtc-6-sub = SRTP, direto entre os laptops
packets-webrtc-6-body = Som e vídeo agora vão direto de um laptop ao outro como SRTP: pacotes RTP — um tipo de payload, um número de sequência, um timestamp — criptografados e autenticados com as chaves do DTLS. Nada espera por um pacote perdido como o TCP esperaria: o som perdido é simplesmente disfarçado, e no vídeo quem recebe pode pedir o pacote de novo ou uma imagem nova. As verificações STUN continuam a cada poucos segundos, para os roteadores não esquecerem o caminho e cada lado saber que o outro ainda quer a mídia.
packets-webrtc-p13 = Vinte milissegundos da minha voz, criptografados.
packets-webrtc-p14 = Um pedaço da imagem da minha câmera, criptografado.

## HTTP proxy

packets-proxy-name = Proxy HTTP
packets-proxy-title = Forward proxy
packets-proxy-tagline = Um intermediário que busca a web por você
packets-proxy-overview = Um forward proxy é um intermediário que o navegador é configurado para usar: em vez de se conectar sozinho a cada servidor web, ele envia todo pedido ao proxy, que busca a página em nome dele. Empresas e escolas usam um para compartilhar um cache, filtrar o que pode ser visitado e manter um registro — e muitas vezes o firewall só deixa o proxy sair. No HTTP simples o proxy lê, e pode alterar, cada pedido e cada resposta; no HTTPS ele só consegue abrir um túnel e repassar os bytes criptografados, sem lê-los.
packets-proxy-port = Nenhuma fixa; 3128 e 8080 são comuns
packets-proxy-used-by = Redes de empresas e escolas, filtros de conteúdo, caches compartilhados
packets-proxy-label-sees = O proxy vê

packets-proxy-1-title = Pedindo ao proxy
packets-proxy-1-sub = A URL inteira vai para o proxy
packets-proxy-1-body = O navegador está configurado para usar proxy.example na porta 3128 — à mão, por uma política da empresa ou por um script que ele baixa (PAC). Então, para http://www.example.com/, ele nem procura o endereço do site nem se conecta a ele: conecta-se ao proxy e envia o pedido com a URL inteira, GET http://www.example.com/, onde de outro modo mandaria só o caminho. É pela URL completa que o proxy sabe aonde ir.
packets-proxy-p1 = Por favor, busque http://www.example.com/ para mim.

packets-proxy-2-title = O proxy busca
packets-proxy-2-sub = Um pedido próprio, para o site
packets-proxy-2-body = O proxy procura www.example.com por conta própria, abre sua própria conexão com ele e envia um pedido comum do caminho /. Em geral ele acrescenta Via, com o próprio nome, e muitas vezes X-Forwarded-For com o endereço do laptop, para o site saber que houve um proxy no meio. Para o site, o cliente é o proxy: ele vê o endereço do proxy, não o do laptop.
packets-proxy-p2 = GET / — para um cliente meu, em 192.168.1.10.
packets-proxy-p3 = Aqui está — e qualquer um pode guardá-la por dez minutos.

packets-proxy-3-title = Resposta e cache
packets-proxy-3-sub = Uma cópia fica para o próximo
packets-proxy-3-body = O proxy devolve a página ao laptop e, como o Cache-Control permite, guarda uma cópia. Quem estiver atrás do proxy e pedir a mesma página nos próximos dez minutos recebe essa cópia na hora, com um cabeçalho Age dizendo a idade dela, e o site não é consultado de novo. Numa rede de centenas de máquinas, isso economiza tempo e banda.
packets-proxy-3-state = A segunda resposta vem da cópia, com 120 segundos
packets-proxy-p4 = Aqui está sua página, buscada para você.
packets-proxy-p5 = http://www.example.com/ de novo, por favor.
packets-proxy-p6 = Da minha cópia, de dois minutos atrás.

packets-proxy-4-title = Filtragem
packets-proxy-4-sub = Alguns sites são recusados
packets-proxy-4-body = Como todo pedido diz qual é o site, o proxy pode conferi-lo com as regras da rede antes de ir a qualquer lugar. games.example.net está na lista de bloqueio, então o próprio proxy responde 403 Forbidden, com uma página sua explicando o motivo, e o site nunca é contatado. É no mesmo lugar que fica o registro de quem pediu o quê, e quando.
packets-proxy-4-reason = O site está na lista de bloqueio da rede
packets-proxy-p7 = Por favor, busque http://games.example.net/.
packets-proxy-p8 = Recusado: esse site é bloqueado nesta rede.

packets-proxy-5-title = CONNECT para HTTPS
packets-proxy-5-sub = O navegador pede um túnel
packets-proxy-5-body = Para https://www.example.com/, o navegador não pode entregar ao proxy uma URL para buscar: o pedido precisa ser criptografado só para o site. Então ele envia CONNECT www.example.com:443, pedindo ao proxy que abra uma conexão TCP com esse host e essa porta. O proxy ainda pode recusar pelo nome; quando aceita, conecta-se e responde 200 Connection established.
packets-proxy-p9 = Abra um túnel até www.example.com, porta 443.
packets-proxy-p10 = Conectado. Daqui em diante só repasso bytes.

packets-proxy-6-title = Um túnel às cegas
packets-proxy-6-sub = TLS de ponta a ponta, através do proxy
packets-proxy-6-body = O navegador começa o TLS com o site por essa mesma conexão, e o proxy copia os bytes nos dois sentidos sem entendê-los. Ele sabe só o nome e a porta do CONNECT, o nome do servidor no ClientHello e quanto passa; não consegue ler, alterar nem guardar as páginas em cache. Só um proxy em cuja autoridade certificadora o laptop foi configurado para confiar consegue olhar dentro, se passando pelo site — como fazem algumas redes de empresa.
packets-proxy-6-sees = O nome e a porta, e quantos bytes passam
packets-proxy-p11 = Olá, www.example.com: vamos combinar as chaves.
packets-proxy-p12 = Olá: aqui está minha parte da chave, e meu certificado, criptografado.

## Reverse proxy

packets-reverse-proxy-name = Proxy reverso
packets-reverse-proxy-title = Proxy reverso e balanceador de carga
packets-reverse-proxy-tagline = Uma porta de entrada para muitos servidores
packets-reverse-proxy-overview = Um proxy reverso fica na frente dos servidores de um site e responde por todos eles: o nome do site aponta para ele, e os servidores atrás dele nunca são alcançados de fora. Ele termina a conexão TLS — o certificado do site mora ali — e repassa cada pedido, em geral como HTTP simples por uma rede privada, a um de vários servidores de aplicação. No caminho ele distribui a carga, guarda arquivos estáticos em cache, para de mandar trabalho a servidores que param de responder e é o lugar único para aplicar limites e regras de segurança. nginx, HAProxy e Caddy costumam ser usados assim, e toda CDN é um.
packets-reverse-proxy-port = 443 e 80 do lado de fora; qualquer porta atrás dele
packets-reverse-proxy-used-by = nginx, HAProxy, Caddy, Envoy, CDNs

packets-reverse-proxy-1-title = O TLS termina aqui
packets-reverse-proxy-1-sub = O proxy guarda o certificado do site
packets-reverse-proxy-1-body = www.example.com resolve para 203.0.113.10 — o proxy reverso, não um servidor de aplicação. O cliente abre o TLS com ele, e o proxy apresenta o certificado de www.example.com, cuja chave privada ele guarda. A criptografia que o cliente vê termina aqui, então um só lugar renova os certificados e faz a criptografia por todos os servidores atrás dele.
packets-reverse-proxy-1-encryption = Do cliente até o proxy
packets-reverse-proxy-p1 = Olá, www.example.com: vamos combinar as chaves.
packets-reverse-proxy-p2 = Olá: eu sou www.example.com, e aqui está meu certificado.

packets-reverse-proxy-2-title = Repassado à aplicação
packets-reverse-proxy-2-sub = HTTP simples na rede privada
packets-reverse-proxy-2-body = O proxy decifra o pedido e o repassa a um servidor de aplicação num endereço privado, 10.0.0.11, que ninguém na internet alcança. Como agora o servidor de aplicação vê o proxy como cliente, o proxy acrescenta X-Forwarded-For com o endereço real do cliente e X-Forwarded-Proto: https (Forwarded é a forma padronizada dos dois). A resposta volta pelo mesmo caminho, e o proxy a criptografa para o cliente.
packets-reverse-proxy-p3 = Meu carrinho, por favor.
packets-reverse-proxy-p4 = GET /cart, para um cliente em 198.51.100.7 que veio por HTTPS.
packets-reverse-proxy-p5 = Aqui está o carrinho. É privado: não guarde cópia.
packets-reverse-proxy-p6 = Seu carrinho, criptografado para você.

packets-reverse-proxy-3-title = Distribuindo a carga
packets-reverse-proxy-3-sub = O próximo pedido, o próximo servidor
packets-reverse-proxy-3-body = O próximo pedido vai para o outro servidor de aplicação, 10.0.0.12: este proxy os reveza (round robin), embora pudesse escolher o que tem menos conexões abertas. Acrescentar um servidor é acrescentar uma linha à lista do proxy, e os visitantes nem percebem. O que a sessão de um visitante precisa tem então de ser compartilhado pelos servidores — num banco de dados ou num cookie —, ou o proxy precisa mandar esse visitante sempre ao mesmo.
packets-reverse-proxy-3-algorithm = Round robin: cada pedido ao próximo servidor
packets-reverse-proxy-p7 = O produto 42, por favor.
packets-reverse-proxy-p8 = GET /products/42 — sua vez.

packets-reverse-proxy-4-title = Arquivos estáticos do cache
packets-reverse-proxy-4-sub = Nenhum servidor de aplicação é consultado
packets-reverse-proxy-4-body = O logotipo do site é igual para todos e vem marcado Cache-Control: public, max-age=86400, então o proxy guarda uma cópia por um dia. Todo pedido por ele é respondido só pelo proxy, com um cabeçalho Age dizendo a idade da cópia, e os servidores de aplicação ficam livres para as páginas que só eles sabem montar. O carrinho, marcado como privado, nunca é guardado.
packets-reverse-proxy-4-state = Respondido só pelo proxy
packets-reverse-proxy-p9 = O logotipo, por favor.
packets-reverse-proxy-p10 = Do meu cache: de uma hora atrás.

packets-reverse-proxy-5-title = Health checks
packets-reverse-proxy-5-sub = Todos os servidores ainda respondem?
packets-reverse-proxy-5-body = A cada poucos segundos o proxy faz a cada servidor de aplicação uma pergunta pequena, como GET /healthz. 10.0.0.12 responde 503 Service Unavailable — sobrecarregado, ou no meio de uma atualização —, então o proxy o tira do revezamento e manda todos os pedidos para 10.0.0.11 até ele voltar a responder 200. Os visitantes não veem página de erro nenhuma, só um site que continua funcionando.
packets-reverse-proxy-5-verdict = Fora do revezamento até responder 200
packets-reverse-proxy-p11 = Tudo bem com você?
packets-reverse-proxy-p12 = Agora não: 503. Tente de novo em 30 segundos.

## SOCKS5

packets-socks5-name = SOCKS5
packets-socks5-title = SOCKS Protocol Version 5
packets-socks5-tagline = Um proxy para qualquer conexão TCP
packets-socks5-overview = O SOCKS é um proxy para qualquer tipo de conexão, não só a web: um programa pede ao servidor SOCKS que se conecte a algum lugar em seu nome, e daí em diante o servidor copia os bytes nos dois sentidos sem lê-los. Ele trabalha abaixo da aplicação — não entende HTTP, e-mail nem SSH, só os carrega —, então o destino vê o endereço do servidor SOCKS, não o do cliente. O ssh -D transforma uma conexão SSH num servidor SOCKS, e o Tor oferece um na porta 9050. O SOCKS em si não criptografa nada.
packets-socks5-transport = TCP; UDP pelo UDP ASSOCIATE
packets-socks5-authentication = Nenhuma, usuário e senha (às claras) ou GSSAPI
packets-socks5-used-by = ssh -D, Tor (porta 9050), configurações de proxy dos navegadores
packets-socks5-label-methods = Métodos oferecidos
packets-socks5-label-seen = O site vê

packets-socks5-1-title = A saudação
packets-socks5-1-sub = Que formas de login você aceita?
packets-socks5-1-body = O programa — um navegador configurado para usar um proxy SOCKS, por exemplo — abre uma conexão TCP com a porta 1080 do servidor e envia quatro bytes: a versão 5, quantos métodos oferece e os métodos, 0x00 (sem autenticação) e 0x02 (usuário e senha). O servidor responde com o que escolheu; 0xFF significaria que nenhum serve, e a conexão terminaria ali. Este servidor quer usuário e senha.
packets-socks5-p1 = SOCKS 5. Posso entrar sem nada, ou com usuário e senha.
packets-socks5-p2 = Usuário e senha, por favor.

packets-socks5-2-title = O login
packets-socks5-2-sub = Usuário e senha, às claras
packets-socks5-2-body = Como manda a RFC 1929, o cliente envia um byte de versão 1, depois o usuário e a senha, cada um precedido do seu tamanho. O servidor responde status 0x00, sucesso; com qualquer outro, ele tem de fechar a conexão. Nada disso é criptografado, então qualquer um no caminho consegue ler a senha — por isso o SOCKS muitas vezes só é oferecido numa rede confiável, ou dentro do SSH.
packets-socks5-2-encryption = Nenhuma: a senha passa às claras
packets-socks5-p3 = Sou user1, e minha senha é k7#qL2.
packets-socks5-p4 = Bem-vindo: status 0, sucesso.

packets-socks5-3-title = O pedido CONNECT
packets-socks5-3-sub = Para onde, e em qual porta
packets-socks5-3-body = Agora o cliente diz aonde quer ir: comando 1, CONNECT, para o tipo de endereço 3 — um nome de domínio, www.example.com — na porta 443. Recebendo um nome em vez de um endereço, o próprio servidor SOCKS faz a consulta DNS, então a rede do cliente nunca vê qual nome foi consultado; o Tor depende disso para as consultas não vazarem. O servidor então abre sua própria conexão TCP com o destino.
packets-socks5-p5 = Me conecte a www.example.com, porta 443.
packets-socks5-p6 = Vamos abrir uma conexão?
packets-socks5-p7 = Sim: estou escutando na porta 443.

packets-socks5-4-title = Sucesso
packets-socks5-4-sub = O servidor dá o retorno
packets-socks5-4-body = Com a conexão de pé, o servidor responde com REP 0x00 (succeeded, sucesso) e o endereço e a porta de onde se conectou: 198.51.100.80, porta 40312. Outras respostas dizem o que deu errado — 0x04 host inalcançável, 0x05 conexão recusada. Daqui em diante o SOCKS não tem mais nada a dizer: a conexão do cliente com a porta 1080 fica emendada à conexão do servidor com www.example.com.
packets-socks5-4-state = Emendadas: daqui em diante, só bytes repassados
packets-socks5-p8 = Pronto: você está conectado, por 198.51.100.80, porta 40312.

packets-socks5-5-title = O repasse
packets-socks5-5-sub = Bytes copiados nos dois sentidos, sem leitura
packets-socks5-5-body = O navegador agora fala TLS com www.example.com como se estivesse conectado direto ao site, e o servidor SOCKS copia cada byte de uma conexão para a outra. Ele não precisa entender nenhum deles: o mesmo repasse carrega HTTPS, SSH, e-mail ou um jogo. O site vê a conexão vindo de 198.51.100.80 — o servidor SOCKS — e nunca o endereço do laptop.
packets-socks5-5-encryption = TLS entre o navegador e o site; o SOCKS não acrescenta nenhuma
packets-socks5-p9 = Olá, www.example.com: vamos combinar as chaves.
packets-socks5-p10 = O olá do cliente, repassado byte a byte.
packets-socks5-p11 = Olá: aqui está minha parte da chave, e meu certificado, criptografado.
packets-socks5-p12 = A resposta do site, devolvida byte a byte.

## Captive portal

packets-captive-portal-name = Portal cativo
packets-captive-portal-title = Portal cativo: detecção e login
packets-captive-portal-tagline = A página entre o Wi-Fi público e a internet
packets-captive-portal-overview = No Wi-Fi de um hotel, de um café ou de um aeroporto, entrar na rede não basta: até você aceitar os termos ou fazer login numa página web — o portal cativo —, o gateway não deixa nada sair, a não ser para essa página. Os aparelhos descobrem que estão presos pedindo uma página conhecida por HTTP simples: o gateway responde em nome desse site com um redirecionamento para o portal, e o sistema o abre para você. Depois que você aceita, o gateway deixa passar o endereço MAC do seu aparelho. O HTTPS não pode ser redirecionado assim, e por isso uma página segura simplesmente falha até você fazer login.
packets-captive-portal-port = 80 para a sondagem; o portal em si, em geral na 443
packets-captive-portal-used-by = Hotéis, cafés, aeroportos, trens
packets-captive-portal-security = Uma rede aberta: qualquer um por perto pode escutar, a não ser que ela use o Enhanced Open — confie no HTTPS ou numa VPN
packets-captive-portal-device-gateway = Gateway
packets-captive-portal-label-api = API do portal (opção 114)
packets-captive-portal-label-expected = Esperado

packets-captive-portal-1-title = Entrando na rede
packets-captive-portal-1-sub = Um endereço, e talvez o do portal
packets-captive-portal-1-body = O celular entra na rede aberta — sem senha, então o enlace de rádio em geral nem é criptografado — e pede um endereço por DHCP, como em qualquer rede. O gateway lhe dá 10.20.0.57 e se apresenta como a saída e como o servidor DNS. Uma rede que segue a RFC 8910 também manda a opção 114, o endereço da API do portal, para que um celular que a entenda possa simplesmente perguntar se está preso, sem ter de descobrir do jeito difícil.
packets-captive-portal-p1 = Fico com o endereço que você ofereceu, 10.20.0.57 — e me fale de algum portal cativo.
packets-captive-portal-p2 = É seu por uma hora. A saída é 10.20.0.1, e aqui está a API do portal.

packets-captive-portal-2-title = A sondagem
packets-captive-portal-2-sub = A internet está mesmo aí?
packets-captive-portal-2-body = Logo depois de entrar, o sistema pede por HTTP simples uma página que conhece bem — aqui, http://check.example.com/generate_204, que deveria responder 204 No Content, sem nada dentro (o Android pergunta a um servidor do Google, os aparelhos da Apple a captive.apple.com). O gateway intercepta o pedido, embora ele seja endereçado a outra máquina, e responde em nome dela: 302 Found, vá ao portal. Qualquer resposta diferente da esperada diz ao celular que ele está preso.
packets-captive-portal-p3 = A internet está aí? Espero um 204, e nada mais.
packets-captive-portal-p4 = Falando como check.example.com: vá primeiro ao portal.

packets-captive-portal-3-title = O HTTPS não se redireciona
packets-captive-portal-3-sub = Uma página segura simplesmente falha
packets-captive-portal-3-body = Enquanto isso, uma aba do navegador tenta https://www.example.com/. O gateway poderia interceptar essa conexão também, mas não consegue responder como www.example.com: não tem certificado para esse nome, e o navegador recusaria um falso. Então ele reseta a conexão, ou a deixa expirar, e a página falha com um erro vago — por isso a sondagem usa HTTP simples, a única coisa que um gateway consegue redirecionar.
packets-captive-portal-3-reason = O gateway não tem certificado para www.example.com
packets-captive-portal-p5 = Me conecte a www.example.com, porta 443.
packets-captive-portal-p6 = Reset: nada passa antes do portal.

packets-captive-portal-4-title = O portal abre
packets-captive-portal-4-sub = Um navegador pequeno mostra a página de login
packets-captive-portal-4-body = Ao ver o redirecionamento, o celular abre sozinho uma pequena janela de navegador e carrega a página do portal: os termos de uso, um campo para o número do quarto ou um código, às vezes um login. O portal é o único lugar para onde o gateway deixa o tráfego passar (o "walled garden"), e ele é servido por HTTPS, com um certificado de verdade para portal.example.net.
packets-captive-portal-4-state = Por enquanto, só o portal é alcançável
packets-captive-portal-p7 = Me mostre a página de login.
packets-captive-portal-p8 = Aqui: nossos termos, e um campo para o número do seu quarto.

packets-captive-portal-5-title = Aceitando os termos
packets-captive-portal-5-sub = O gateway é avisado para deixar o celular passar
packets-captive-portal-5-body = A pessoa marca a caixa e envia o formulário. O portal registra isso e avisa o gateway — pela interface do próprio fabricante, ou por uma mensagem RADIUS — para deixar passar o aparelho de endereço MAC 00:00:5e:00:53:2a, em geral por um tempo limitado. O gateway reconhece o celular por esse endereço, e por isso os celulares, embora inventem um MAC aleatório por privacidade, costumam manter o mesmo para cada rede.
packets-captive-portal-5-verdict = Liberado por 24 horas
packets-captive-portal-p9 = Aceito os termos. Quarto 214.
packets-captive-portal-p10 = Obrigado: você está on-line.

packets-captive-portal-6-title = On-line
packets-captive-portal-6-sub = Agora a sondagem dá certo
packets-captive-portal-6-body = O celular pede de novo http://check.example.com/generate_204. Desta vez o gateway deixa o pedido sair até o servidor de verdade, que responde 204 No Content, exatamente como esperado. O sistema fecha a janela do portal e marca a rede como conectada; até o tempo acabar, toda conexão — HTTPS inclusive — sai normalmente.
packets-captive-portal-6-state = Conectado à internet
packets-captive-portal-p11 = A internet está aí agora? Espero um 204.
packets-captive-portal-p12 = 204 No Content: exatamente o que você esperava.

## SMTP

packets-smtp-name = SMTP
packets-smtp-title = Simple Mail Transfer Protocol
packets-smtp-tagline = Como um e-mail sai de você e chega ao servidor do destinatário
packets-smtp-overview = O SMTP leva o e-mail de servidor em servidor, como uma cadeia de agências dos correios. Seu programa de e-mail entrega a mensagem ao servidor do seu provedor (submissão, porta 587), que procura no DNS onde o domínio do destinatário recebe e-mail — o registro MX — e a repassa a esse servidor (relay, porta 25). Cada salto é o mesmo diálogo curto em texto: quem sou eu, de quem é, para quem é, depois a mensagem, até que uma resposta 250 passe a responsabilidade adiante. O SMTP só envia; ler o e-mail é trabalho do IMAP ou do POP3.
packets-smtp-port = 587 para enviar (465 com TLS desde o início); 25 entre servidores
packets-smtp-format = Comandos em texto; respostas com um código de três dígitos

packets-smtp-1-title = Submissão
packets-smtp-1-sub = Seu programa chama o seu servidor
packets-smtp-1-body = Você envia uma mensagem para bob@example.com. Seu programa de e-mail abre uma conexão TCP com a porta 587 do servidor do seu provedor, mail.example.net, que fala primeiro, com uma saudação 220. O programa se apresenta com EHLO, e o servidor lista o que sabe fazer — uma linha por extensão, entre elas STARTTLS.
packets-smtp-p1 = Olá, sou 192.168.1.10. O que você sabe fazer?
packets-smtp-p2 = Sou mail.example.net. Aceito comandos em pipelining, mensagens de até 50 MB, e posso passar para TLS.

packets-smtp-2-title = Cifrar, depois entrar
packets-smtp-2-sub = STARTTLS e AUTH
packets-smtp-2-body = O STARTTLS transforma a conexão em claro numa conexão cifrada: o servidor responde 220, os dois fazem um handshake TLS e o programa diz EHLO de novo, já que nada dito antes da cifra é confiável. Só agora o servidor oferece AUTH, e o programa entra com o endereço e a senha de alice — o AUTH PLAIN apenas os codifica em Base64, então depende do TLS em volta. A resposta 235 quer dizer que alice pode enviar por este servidor, que se recusa a repassar e-mail de estranhos.
packets-smtp-p3 = Vamos cifrar antes que eu diga algo privado.
packets-smtp-p4 = Sou alice@example.net, e aqui está minha senha — dentro do TLS.
packets-smtp-p5 = Bem-vinda, alice. Pode enviar.

packets-smtp-3-title = Envelope e mensagem
packets-smtp-3-sub = De quem é, para quem é, e então a mensagem
packets-smtp-3-body = MAIL FROM e RCPT TO são o envelope: para onde entregar e para onde avisar de uma falha — não as linhas From e To que o leitor vê, que fazem parte da mensagem. Como o servidor ofereceu PIPELINING, o programa os envia junto com DATA de uma vez, e o servidor responde 250, 250 e 354, "pode mandar". A mensagem vem em seguida — cabeçalhos, uma linha em branco, o corpo — e termina com uma linha que tem só um ponto. O servidor responde 250: pôs a mensagem na fila e, a partir de agora, é responsável por ela.
packets-smtp-p6 = De alice@example.net, para bob@example.com — e lá vai a mensagem.
packets-smtp-p7 = Remetente certo, destinatário certo. Pode mandar, e termine com um ponto.
packets-smtp-p8 = Aqui está a mensagem. Um ponto sozinho a encerra.
packets-smtp-p9 = Na fila. Agora é comigo.

packets-smtp-4-title = O servidor do destinatário
packets-smtp-4-sub = Uma consulta MX no DNS
packets-smtp-4-body = O endereço do destinatário termina em example.com, então mail.example.net pergunta ao DNS pelos registros MX desse domínio: os servidores que recebem o e-mail dele, cada um com uma preferência — o menor número é tentado primeiro, os outros são reservas. A resposta indica mx.example.com, com o endereço junto. Se esse servidor estivesse fora do ar, mail.example.net tentaria o próximo, ou guardaria a mensagem na fila e tentaria de novo por dias antes de desistir.
packets-smtp-p10 = Quais servidores recebem e-mail de example.com?
packets-smtp-p11 = mx.example.com, preferência 10 — em 203.0.113.25.

packets-smtp-5-title = Relay
packets-smtp-5-sub = De servidor para servidor, na porta 25
packets-smtp-5-body = mail.example.net se conecta à porta 25 de mx.example.com e repete o diálogo: EHLO, STARTTLS quando oferecido, MAIL FROM, RCPT TO, DATA — mas sem AUTH, pois mx.example.com aceita e-mail de qualquer um, desde que seja para os seus próprios usuários. Antes de enviar, seu servidor acrescentou uma DKIM-Signature: uma assinatura digital sobre a mensagem, feita com uma chave que só example.net tem. Com o 250 final, a mensagem passa a ser responsabilidade de mx.example.com.
packets-smtp-p12 = Olá, sou mail.example.net. O que você sabe fazer?
packets-smtp-p13 = Uma mensagem para bob@example.com, assinada por example.net.
packets-smtp-p14 = Aceita. Agora é comigo.

packets-smtp-6-title = Verificações e caixa postal
packets-smtp-6-sub = SPF, DKIM e a caixa de entrada de bob
packets-smtp-6-body = Qualquer um pode se dizer qualquer remetente, então o servidor que recebe confere. SPF: o registro SPF de example.net — um registro TXT no DNS — lista 198.51.100.25 entre os servidores que podem enviar o e-mail dele? DKIM: a assinatura confere com a chave pública que example.net publica no DNS, provando que a mensagem não foi alterada no caminho? As duas passam, e a mensagem vai para a caixa postal de bob, de onde o IMAP ou o POP3 vai buscá-la.

## POP3

packets-pop3-name = POP3
packets-pop3-title = Post Office Protocol, versão 3
packets-pop3-tagline = Baixa o seu e-mail e o tira do servidor
packets-pop3-overview = O POP3 trata o servidor como uma caixa postal dos correios: você passa lá, recolhe o que chegou, e a caixa fica vazia de novo. Um programa de e-mail entra, lista as mensagens que esperam, baixa cada uma e quase sempre a apaga do servidor, então o e-mail passa a morar naquele único aparelho. É simples e exige pouco do servidor — mas assim um laptop e um celular não conseguem dividir uma caixa postal, nem suas pastas e marcas de lido, e é por isso que a maioria das pessoas passou para o IMAP. O SMTP traz o e-mail; o POP3 só o retira.
packets-pop3-port = 995 com TLS; 110 em claro
packets-pop3-format = Comandos em texto; toda resposta começa com +OK ou -ERR
packets-pop3-model = Baixar e apagar: o e-mail vai para um aparelho só

packets-pop3-1-title = TLS primeiro
packets-pop3-1-sub = Cifrado desde o primeiro byte
packets-pop3-1-body = O programa de e-mail se conecta à porta 995 de mail.example.net, e os dois fazem um handshake TLS antes de qualquer palavra de POP3 — o TLS implícito, que a RFC 8314 recomenda em vez de cifrar depois uma conexão em claro na porta 110. Então o servidor cumprimenta o programa com +OK. Toda resposta do POP3 começa com +OK ou -ERR: uma palavra que um programa consegue ler, com texto livre depois dela para as pessoas.
packets-pop3-p1 = +OK — aqui é mail.example.net, pronto quando você estiver.

packets-pop3-2-title = Entrando
packets-pop3-2-sub = USER, depois PASS
packets-pop3-2-body = O programa envia o nome de usuário, que o servidor confirma com +OK, e depois a senha — linhas de comando simples, mantidas em segredo só pelo TLS em volta delas. Com a senha certa, o servidor tranca a caixa postal para esta sessão, para que nenhum outro cliente a altere ao mesmo tempo, e a sessão entra no estado TRANSACTION.
packets-pop3-p2 = Sou alice.
packets-pop3-p3 = E esta é a minha senha.
packets-pop3-p4 = Bem-vinda. A caixa postal é só sua até você sair.

packets-pop3-3-title = O que está esperando
packets-pop3-3-sub = STAT e LIST
packets-pop3-3-body = O STAT pede os totais: 2 mensagens, 3.271 bytes ao todo. O LIST dá o número e o tamanho de cada mensagem, uma por linha, e uma linha só com um ponto encerra a lista. Esses números valem só nesta sessão; um programa que deixa o e-mail no servidor usa o UIDL para obter identificadores que nunca mudam.
packets-pop3-p5 = Quanto está esperando?
packets-pop3-p6 = 2 mensagens, 3.271 bytes.
packets-pop3-p7 = Liste as mensagens, com os tamanhos.
packets-pop3-p8 = Número 1: 1.205 bytes. Número 2: 2.066 bytes. É só.

packets-pop3-4-title = Baixando
packets-pop3-4-sub = RETR 1
packets-pop3-4-body = O RETR 1 pede a primeira mensagem inteira: cabeçalhos e corpo como o SMTP os entregou, com um ponto sozinho na última linha. O programa a salva no laptop, e buscaria a mensagem 2 do mesmo jeito. Uma linha da mensagem que por acaso comece com um ponto é enviada com um segundo ponto na frente, para não ser confundida com o fim.
packets-pop3-p9 = Me mande a mensagem 1.
packets-pop3-p10 = Aqui está, 1.205 bytes, terminando com um ponto.

packets-pop3-5-title = Apagar e sair
packets-pop3-5-sub = O e-mail sai do servidor
packets-pop3-5-body = O DELE 1 só marca a mensagem: se a conexão caísse agora, nada se perderia. O QUIT leva a sessão ao estado UPDATE, e só então o servidor remove o que foi marcado e destranca a caixa postal. A mensagem agora existe só no laptop — um celular que olhe a mesma caixa postal depois não vai encontrá-la.
packets-pop3-5-mailbox = A mensagem 1 saiu do servidor
packets-pop3-p11 = Apague a mensagem 1.
packets-pop3-p12 = Marcada para apagar.
packets-pop3-p13 = Terminei. Tchau.
packets-pop3-p14 = Apagada de vez. Tchau.

## IMAP

packets-imap-name = IMAP
packets-imap-title = Internet Message Access Protocol
packets-imap-tagline = Sua caixa postal fica no servidor, igual em todo aparelho
packets-imap-overview = Com o IMAP o e-mail mora no servidor, e cada aparelho é uma janela para ele. O laptop e o celular mantêm cada um a sua conexão, pedem os cabeçalhos de que precisam e só baixam uma mensagem quando ela é aberta; pastas e flags como \Seen ficam guardadas no servidor, então uma mensagem lida num aparelho aparece como lida em todos. Cada comando leva uma tag que a resposta repete, então um programa pode enviar vários sem esperar, e o IDLE deixa o servidor avisar um aparelho de e-mail novo no instante em que ele chega.
packets-imap-port = 993 com TLS; 143 com STARTTLS
packets-imap-format = Comandos em texto com tag; linhas que começam com * trazem novidades
packets-imap-model = O e-mail fica no servidor; todo aparelho vê o mesmo

packets-imap-1-title = Entrando
packets-imap-1-sub = TLS na porta 993, depois a1 LOGIN
packets-imap-1-body = O laptop se conecta à porta 993 e faz primeiro um handshake TLS; depois o servidor o cumprimenta e lista o que sabe fazer. Cada comando começa com uma tag que o programa escolhe — a1, a2, a3 — e o servidor termina a resposta a ele com a mesma tag, então vários comandos podem estar em andamento ao mesmo tempo. O a1 LOGIN envia o nome de usuário e a senha, protegidos dentro do TLS.
packets-imap-p1 = Bem-vindo. Falo IMAP4rev2 e sei fazer IDLE.
packets-imap-p2 = a1: sou alice, e aqui está minha senha.
packets-imap-p3 = a1 feito: você entrou.

packets-imap-2-title = Escolhendo uma caixa
packets-imap-2-sub = a2 SELECT INBOX
packets-imap-2-body = Uma caixa postal é uma pasta no servidor — INBOX, Enviados, Arquivo — e o programa seleciona aquela em que vai trabalhar. O servidor a descreve em linhas sem tag, marcadas com *: existem 5 mensagens, estas são as flags que uma mensagem pode ter. Depois vem o a2 OK com tag, com READ-WRITE, porque esta sessão pode alterar a caixa.
packets-imap-p4 = a2: abra a INBOX.
packets-imap-p5 = A INBOX tem 5 mensagens. a2 feito, e você pode alterá-la.

packets-imap-3-title = A lista de mensagens
packets-imap-3-sub = Só os envelopes, e as flags
packets-imap-3-body = Para montar a lista, o programa não baixa as mensagens: o a3 FETCH 1:5 (FLAGS ENVELOPE) pede só o remetente, o assunto e a data de cada uma, e as flags dela. As mensagens 1 e 2 têm \Seen — já lidas, talvez no celular — e a 3, a 4 e a 5 não. É por isso que uma caixa com milhares de mensagens abre rápido: os corpos esperam no servidor até serem necessários.
packets-imap-p6 = a3: as flags e o envelope das mensagens 1 a 5.
packets-imap-p7 = Aqui estão: 1 e 2 foram lidas, 3 a 5 são novas. a3 feito.

packets-imap-4-title = Lendo uma mensagem
packets-imap-4-sub = a4 FETCH 3 BODY[]
packets-imap-4-body = Alice abre a mensagem 3, e só agora o texto inteiro dela é baixado: BODY[] é a mensagem completa, 2.066 bytes, enviada como um literal — primeiro o tamanho, depois exatamente essa quantidade de bytes. Buscá-la liga a flag \Seen no servidor, não só no laptop, e o servidor avisa disso na mesma resposta. Um programa que quer ler sem marcar pede BODY.PEEK[] no lugar.
packets-imap-p8 = a4: a mensagem 3 inteira, por favor.
packets-imap-p9 = Mensagem 3, 2.066 bytes — e agora ela está marcada como lida.

packets-imap-5-title = Todo aparelho em dia
packets-imap-5-sub = IDLE: o servidor avisa o celular
packets-imap-5-body = O celular entrou antes e também selecionou a INBOX; agora envia IDLE — "me avise quando algo mudar" — e espera na conexão aberta em vez de perguntar a cada poucos minutos. O servidor logo lhe conta que a mensagem 3 está \Seen, já que o laptop a leu, e, quando chega uma mensagem nova pelo SMTP, que agora existem 6 mensagens. A lista do celular fica em dia sem consultas repetidas, o que poupa a bateria.
packets-imap-p10 = b7: vou esperar. Me avise quando algo mudar.
packets-imap-p11 = A mensagem 3 foi lida.
packets-imap-p12 = E-mail novo: agora são 6 mensagens.

packets-imap-6-title = Saindo
packets-imap-6-sub = a5 LOGOUT — e nada é apagado
packets-imap-6-body = O a5 LOGOUT encerra a sessão do laptop: o servidor diz BYE, confirma com a5 OK e fecha a conexão. Nada foi apagado — as seis mensagens e todas as flags continuam no servidor, e o que o laptop baixou é só uma cópia que ele pode buscar de novo. Esse é o modelo do IMAP: o servidor guarda a verdade, e cada aparelho a mostra.
packets-imap-6-state = 6 mensagens, todas ainda no servidor
packets-imap-p13 = a5: estou saindo.
packets-imap-p14 = Tchau. a5 feito.

## FTP

packets-ftp-name = FTP
packets-ftp-title = File Transfer Protocol
packets-ftp-tagline = Arquivos por duas conexões — e nada cifrado
packets-ftp-overview = O FTP move arquivos entre um cliente e um servidor, e é um dos protocolos mais antigos da internet: a primeira versão dele rodava antes de o TCP/IP existir. Ele usa duas conexões — uma conexão de controle na porta 21 para comandos e respostas, e uma conexão de dados nova para cada arquivo ou listagem. Foi pensado para uma rede de pessoas que confiavam umas nas outras, então o nome de usuário, a senha e os arquivos viajam todos em texto claro. Hoje o SFTP e o FTPS fazem o mesmo trabalho com cifra, e os navegadores nem falam mais FTP.
packets-ftp-port = 21 para os comandos; outra porta para cada transferência
packets-ftp-security = Nenhuma: nomes, senhas e arquivos passam em texto claro
packets-ftp-label-replaced-by = Substituído por

packets-ftp-1-title = A conexão de controle
packets-ftp-1-sub = Porta 21, e uma saudação
packets-ftp-1-body = O laptop, no Wi-Fi de um café, abre uma conexão TCP com a porta 21 de ftp.example.com, e o servidor o cumprimenta com 220. Essa conexão vai levar só comandos e respostas curtas, cada uma começando com um código de três dígitos — o esquema que o SMTP depois tomou emprestado. Os arquivos em si vão viajar por outra conexão.
packets-ftp-1-channel = Controle: comandos e respostas
packets-ftp-p1 = 220 — ftp.example.com, às suas ordens.

packets-ftp-2-title = Login em texto claro
packets-ftp-2-sub = USER e PASS, legíveis no caminho
packets-ftp-2-body = O laptop envia USER alice; o servidor responde 331, pedindo a senha; o laptop envia PASS s3cret, e 230 quer dizer que entrou. Nada disso é cifrado: qualquer um que consiga ver o tráfego — outro cliente num Wi-Fi aberto, qualquer roteador no caminho — lê a senha com a mesma facilidade que o servidor. Só isso já descarta o FTP puro na internet.
packets-ftp-2-encryption = Nenhuma
packets-ftp-p2 = Sou alice.
packets-ftp-p3 = 331 — e a sua senha?
packets-ftp-p4 = Minha senha é s3cret. Quem estiver escutando também sabe agora.
packets-ftp-p5 = 230 — você entrou.

packets-ftp-3-title = Modo passivo
packets-ftp-3-sub = PASV: o servidor indica uma porta para os dados
packets-ftp-3-body = Antes que um arquivo possa andar, é preciso uma segunda conexão. O laptop envia PASV, e o servidor responde 227 com seis números: o endereço dele, 203.0.113.21, e uma porta escrita em dois bytes — 195 × 256 + 149 = 50069 — na qual ele agora espera. No modo original, o ativo, era o servidor que se conectava de volta ao cliente, coisa que firewalls e roteadores domésticos bloqueiam; o EPSV (RFC 2428) envia só a porta, e funciona com IPv6.
packets-ftp-p6 = Modo passivo, por favor: onde eu me conecto para os dados?
packets-ftp-p7 = 227 — conecte em 203.0.113.21, porta 50069.

packets-ftp-4-title = A conexão de dados
packets-ftp-4-sub = Uma segunda conexão TCP leva o arquivo
packets-ftp-4-body = O laptop abre uma nova conexão TCP com a porta 50069 e então pede pela conexão de controle: RETR example.tar.gz. O servidor responde 150, envia os bytes do arquivo pela conexão de dados e a fecha — o fechamento marca o fim do arquivo — e então avisa 226 pela conexão de controle. Cada arquivo e cada listagem de diretório custa uma nova conexão de dados como esta.
packets-ftp-4-channel = Dados: uma conexão nova para cada arquivo
packets-ftp-p8 = Abrindo uma segunda conexão, para os dados.
packets-ftp-p9 = Me mande example.tar.gz.
packets-ftp-p10 = 150 — está vindo pela conexão de dados.
packets-ftp-p11 = O arquivo em si, em texto claro como todo o resto.
packets-ftp-p12 = 226 — transferência concluída.

packets-ftp-5-title = O que o substituiu
packets-ftp-5-sub = SFTP e FTPS
packets-ftp-5-body = As falhas do FTP estão no projeto: tudo passa em texto claro, e uma segunda conexão numa porta combinada no meio da conversa atrapalha firewalls e NAT, que precisam ler a conexão de controle para deixá-la passar. O SFTP, parte do SSH, move arquivos por uma única conexão cifrada na porta 22; o FTPS mantém o FTP, mas envolve as duas conexões em TLS. Os navegadores abandonaram o FTP em 2021.

## SMB

packets-smb-name = SMB
packets-smb-title = Server Message Block
packets-smb-tagline = O compartilhamento de arquivos do Windows: pastas num servidor, abertas como se fossem locais
packets-smb-overview = O SMB deixa um computador trabalhar nos arquivos de outro como se fossem seus: abrir, ler, gravar, travar, listar. É assim que o Windows compartilha pastas e impressoras; o Samba o fala no Linux, e o macOS também o fala. Por uma única conexão TCP com a porta 445, o cliente combina um dialeto, prova quem é o usuário, se conecta a um compartilhamento e então trabalha nos arquivos por handles, cada mensagem levando um código de comando e um ID de mensagem. O SMB 3 assina e cifra, então os arquivos atravessam a rede lacrados.
packets-smb-transport = TCP (QUIC, na porta 443, no Windows recente)
packets-smb-device-dc = Controlador de domínio

packets-smb-1-title = Negociação
packets-smb-1-sub = Combinando um dialeto
packets-smb-1-body = O computador abre uma conexão TCP com a porta 445 de fs1.example.com e envia NEGOTIATE, listando os dialetos que fala, do 2.0.2 ao 3.1.1, e as cifras que conhece. O servidor escolhe o mais novo que os dois têm — 3.1.1, com AES-128-GCM — e os dois passam a incluir um hash da conversa nas chaves que virão, então um intermediário que a alterasse — riscando as cifras fortes da lista, por exemplo — seria pego. O SMB1 nem é oferecido: feito para redes de confiança, não tem cifra, e suas falhas deixaram o worm WannaCry se espalhar em 2017.
packets-smb-p1 = Falo SMB do 2.0.2 ao 3.1.1. Qual você fala?
packets-smb-p2 = Então 3.1.1, com AES-128-GCM.

packets-smb-2-title = Abertura de sessão
packets-smb-2-sub = Provando quem é o usuário, com Kerberos
packets-smb-2-body = Alice entrou no computador num domínio Windows, então ele já tem um ticket Kerberos do controlador de domínio. Ele pede ao controlador um ticket para este servidor, cifs/fs1.example.com, e o entrega no SESSION_SETUP; o servidor o abre com a própria chave secreta e descobre que é alice, sem que a senha dela jamais passe pela rede. Os dois lados agora dividem uma chave de sessão, da qual o SMB 3.1.1 deriva as chaves que assinam e cifram. Fora de um domínio, o NTLM faz esse trabalho com um desafio e uma resposta.
packets-smb-2-protocol = Kerberos; NTLM fora de um domínio
packets-smb-p3 = Um ticket para o servidor de arquivos cifs/fs1.example.com, por favor.
packets-smb-p4 = Aqui está. Só o fs1 consegue abri-lo.
packets-smb-p5 = Aqui está o meu ticket do controlador de domínio.
packets-smb-p6 = Bem-vinda, alice. Esta resposta vai assinada com a nossa chave nova.

packets-smb-3-title = Conexão ao compartilhamento
packets-smb-3-sub = Conectando a um compartilhamento
packets-smb-3-body = Um compartilhamento é uma pasta que o servidor oferece, com nome de caminho: \\fs1.example.com\share. O TREE_CONNECT pede por ela; o servidor confere se alice pode usá-la e responde com um tree ID que os próximos comandos vão levar. Este compartilhamento exige cifra, então daqui em diante toda mensagem vai lacrada com AES-128-GCM, e não pode ser lida nem alterada no caminho.
packets-smb-p7 = Me conecte a \\fs1.example.com\share.
packets-smb-p8 = A árvore 5 é sua — e tudo nela viaja cifrado.

packets-smb-4-title = Abrir e ler
packets-smb-4-sub = CREATE, depois READ
packets-smb-4-body = Abrir um arquivo é um CREATE, mesmo para um arquivo que já existe: o pedido diz o que alice quer fazer com ele, e a resposta é um file ID, um handle que todo comando seguinte cita. O READ pede até 64 KiB a partir do offset 0, e o servidor manda o arquivo inteiro, 48.213 bytes, cifrado. Um arquivo grande exige muitas leituras assim, várias ao mesmo tempo, tantas quanto os créditos que o servidor concedeu permitem.
packets-smb-p9 = Abra example.docx, para leitura.
packets-smb-p10 = Aberto: tem 48.213 bytes. Aqui está o handle dele.
packets-smb-p11 = Leia até 65.536 bytes a partir do início.
packets-smb-p12 = Aqui estão: todos os 48.213.

packets-smb-5-title = Fechar
packets-smb-5-sub = O handle é liberado
packets-smb-5-body = O CLOSE libera o handle, e com ele qualquer trava que alice tinha no arquivo. A sessão e a árvore continuam abertas, então abrir o próximo arquivo custa uma única ida e volta. Se a conexão cair, o SMB 3 consegue se reconectar e retomar o arquivo aberto — um durable handle — muitas vezes sem o usuário perceber.
packets-smb-5-state = Sessão e árvore abertas, arquivo fechado
packets-smb-p13 = Terminei com example.docx.
packets-smb-p14 = Fechado.

## TLS 1.3

packets-tls-name = TLS 1.3
packets-tls-title = Transport Layer Security, versão 1.3
packets-tls-tagline = Torna uma conexão privada, e prova quem está do outro lado
packets-tls-overview = O TLS transforma uma conexão comum numa conexão privada: ninguém no caminho consegue ler o que passa por ela nem alterá-lo sem ser notado, e o cliente sabe que está falando com o servidor que pediu. Ele começa com um handshake em que os dois lados combinam uma cifra e uma chave nova — por Diffie-Hellman, então a chave em si nunca passa pelo fio — e o servidor prova sua identidade com um certificado. A versão 1.3 faz tudo isso numa única ida e volta e já cifra a maior parte do handshake; cada registro depois dele é selado com uma cifra AEAD, que esconde os dados e detecta qualquer alteração neles. O HTTPS é simplesmente HTTP dentro do TLS, na porta 443.
packets-tls-port = 443 para HTTPS, e qualquer porta em que um protocolo o leve
packets-tls-authentication = Certificados X.509, ou uma chave pré-compartilhada
packets-tls-label-sni = Nome do servidor (SNI)

packets-tls-1-title = ClientHello
packets-tls-1-sub = O cliente oferece, e manda metade de uma chave
packets-tls-1-body = Com a conexão TCP aberta, o cliente fala primeiro. Seu ClientHello leva 32 bytes aleatórios, as cipher suites que ele suporta, o nome do site que ele quer (SNI, para que um endereço possa servir muitos sites) e, desde já, um key share: a metade pública de um par de chaves X25519 novo, enviada apostando que o servidor aceita X25519, o que economiza uma ida e volta inteira. Ainda não há chave, então esta mensagem vai às claras: qualquer um no caminho pode ler o nome do site.
packets-tls-p1 = Olá. Estas são as cifras que eu falo, o site que eu quero e a minha metade de uma chave.

packets-tls-2-title = ServerHello
packets-tls-2-sub = Os dois lados agora têm o mesmo segredo
packets-tls-2-body = O servidor escolhe uma suite da lista do cliente e responde com seus próprios bytes aleatórios e seu próprio share X25519. Cada lado combina sua chave privada com o share público do outro e chega ao mesmo segredo, que nunca passou pelo fio; dele e de um hash dos dois hellos, os dois derivam as chaves do handshake. Tudo depois desta mensagem vai cifrado.
packets-tls-2-state = Chaves do handshake dos dois lados
packets-tls-p2 = Vamos de AES-128-GCM. Aqui está a minha metade da chave.

packets-tls-3-title = O voo cifrado do servidor
packets-tls-3-sub = Certificado, assinatura, Finished
packets-tls-3-body = Já sob as chaves do handshake, o servidor envia o EncryptedExtensions (o resto das suas escolhas, como o protocolo h2) e o seu Certificate: uma cadeia que o cliente confere até uma autoridade em que confia, contra o nome que pediu e as datas. O CertificateVerify é uma assinatura sobre toda a conversa até ali, feita com a chave privada do certificado — prova de que o servidor tem essa chave, e não só uma cópia do certificado — e o Finished, um MAC sobre cada mensagem do handshake, prova que nada foi alterado no caminho. Por fora, cada registro diz só 23 (dados de aplicação); a anatomia mostra o que vai dentro, como uma captura com as chaves da sessão mostraria.
packets-tls-3-encryption = Sob as chaves do handshake
packets-tls-p3 = Daqui em diante estamos cifrados. Aqui está o resto do que escolhi.
packets-tls-p4 = Meu certificado, e a autoridade que responde por ele.
packets-tls-p5 = Minha assinatura sobre tudo até aqui, feita com a chave do certificado.
packets-tls-p6 = Finished: este é o handshake como eu o vi.

packets-tls-4-title = Finished do cliente e dados
packets-tls-4-sub = Uma ida e volta, e então o pedido
packets-tls-4-body = O cliente confere o Finished do servidor e envia o seu, e o pedido pode vir logo atrás: o handshake inteiro custou uma ida e volta. Cada registro daqui em diante é dado de aplicação sob chaves de tráfego novas, cifrado com AES-128-GCM e seguido de uma tag de 16 bytes calculada sobre ele. Se um único byte muda no caminho, a tag deixa de bater e quem recebe fecha a conexão em vez de aceitá-lo.
packets-tls-p7 = Finished também. Minha visão do handshake bate com a sua.
packets-tls-p8 = Meu pedido, selado.
packets-tls-p9 = A resposta, selada.

packets-tls-5-title = Ticket de sessão
packets-tls-5-sub = Um caminho mais rápido na próxima vez
packets-tls-5-body = Depois do handshake o servidor envia um NewSessionTicket: um ticket opaco e por quantos segundos ele pode ser usado, do qual os dois lados derivam uma chave de retomada. Na próxima vez, o cliente oferece essa chave no seu ClientHello e o servidor pode pular o certificado e a assinatura — em geral ainda com uma troca X25519 nova, para que o tráfego passado continue seguro se o ticket um dia vazar. O cliente pode até mandar dados já no primeiro voo (0-RTT), mas um atacante poderia repetir esses dados, então só pedidos que podem ser repetidos sem risco devem usá-lo.
packets-tls-p10 = Um ticket, para você voltar mais rápido.

packets-tls-6-title = close_notify
packets-tls-6-sub = O fim, dito de propósito
packets-tls-6-body = Ao terminar, o cliente envia um alerta close_notify, cifrado como qualquer outro registro, e o servidor responde com o seu. Sem ele, um atacante poderia cortar a conexão no meio e deixar uma página ou um arquivo truncado sem nenhum dos lados perceber; o close_notify diz que o fim foi intencional. Então o TCP fecha a conexão com os seus FINs.
packets-tls-6-state = Fechada de forma limpa, depois o FIN do TCP
packets-tls-p11 = Terminei. Este fim é de propósito.
packets-tls-p12 = Eu também. Tchau.

## Post-quantum key exchange (ML-KEM hybrid)

packets-mlkem-name = Híbrido ML-KEM
packets-mlkem-title = Troca de chaves pós-quântica: X25519MLKEM768
packets-mlkem-tagline = Uma chave que um computador quântico futuro não recupera de uma gravação
packets-mlkem-overview = As trocas de chaves de hoje, o X25519 entre elas, se apoiam em problemas que um computador quântico grande o bastante conseguiria resolver — e o tráfego gravado agora poderia ser decifrado nesse dia. O ML-KEM (FIPS 203, antes chamado Kyber) é um mecanismo de encapsulamento de chave baseado em problemas de reticulados que nenhum algoritmo quântico conhecido resolve com eficiência. Como ele é novo, é usado em modo híbrido: o X25519MLKEM768 roda o ML-KEM-768 e o X25519 lado a lado e combina os dois segredos, então a chave se sustenta enquanto qualquer um dos dois se sustentar. Custa pouco mais de 2 KB por handshake e quase nenhum tempo, e os navegadores, servidores e o OpenSSH modernos o combinam primeiro.
packets-mlkem-security = Se sustenta enquanto o ML-KEM ou o X25519 se sustentar
packets-mlkem-used-by = Chrome, Firefox, Cloudflare, OpenSSH 10 e este sistema
packets-mlkem-label-tls = O TLS deste sistema
packets-mlkem-label-ssh = O SSH deste sistema

packets-mlkem-1-title = Colha agora, decifre depois
packets-mlkem-1-sub = Gravado hoje, lido amanhã
packets-mlkem-1-body = Um handshake clássico combina sua chave com X25519: seguro contra qualquer computador de hoje, mas um computador quântico grande o bastante, rodando o algoritmo de Shor, poderia recuperar a chave a partir dos dois shares públicos que qualquer gravação do handshake contém. Quem grava tráfego cifrado agora — um Estado, quem opera uma rede — pode guardá-lo e decifrá-lo no dia em que essa máquina existir. Para segredos que precisam continuar secretos por anos, a troca de chaves tem de resistir a essa máquina desde já, antes de ela ser construída.
packets-mlkem-1-verdict = Seguro hoje; aberto a um computador quântico futuro

packets-mlkem-2-title = Um key share híbrido
packets-mlkem-2-sub = O ClientHello cresce
packets-mlkem-2-body = O cliente oferece primeiro o grupo X25519MLKEM768 (0x11EC). Seu key share leva duas chaves públicas lado a lado: uma chave de encapsulamento ML-KEM-768 de 1.184 bytes seguida de um share X25519 de 32 — 1.216 bytes onde o X25519 sozinho ocupa 32 — e a maioria dos clientes junta também um share X25519 simples, para que um servidor que não conhece o híbrido ainda responda numa ida e volta. O ClientHello passa de 1.700 bytes e já não cabe num segmento TCP; alguns middleboxes antigos, que esperavam recebê-lo inteiro num só pacote, quebraram com isso e atrasaram a adoção.
packets-mlkem-p1 = A primeira parte do meu hello: minhas chaves não cabem num pacote só.
packets-mlkem-p2 = O resto do meu hello: agora você o tem inteiro, com as chaves.

packets-mlkem-3-title = O servidor encapsula
packets-mlkem-3-sub = Um segredo selado para a chave do cliente
packets-mlkem-3-body = O ML-KEM não é uma troca Diffie-Hellman, mas um encapsulamento de chave: com a chave de encapsulamento do cliente, o servidor gera um segredo novo de 32 bytes e um texto cifrado de 1.088 bytes que só a chave privada do cliente consegue abrir. Ao lado dele o servidor põe seu próprio share X25519, e envia os dois no key share do seu ServerHello. O cliente desencapsula o texto cifrado para obter o mesmo segredo ML-KEM, e calcula o segredo X25519 como de costume.
packets-mlkem-p3 = Combinado: o híbrido. Aqui está um segredo selado para a sua chave, e o meu share X25519.

packets-mlkem-4-title = Dois segredos, uma chave
packets-mlkem-4-sub = Se sustenta enquanto qualquer um se sustentar
packets-mlkem-4-body = Os dois lados agora têm dois segredos de 32 bytes, um do ML-KEM e outro do X25519, e põem os dois, juntos, no key schedule do TLS, onde antes ia um só. Para ler a sessão, um atacante precisa quebrar os dois: um computador quântico que derrote o X25519 ainda enfrenta o ML-KEM, e uma falha descoberta um dia no jovem ML-KEM ainda deixa o X25519 de pé. A assinatura do certificado continua clássica por enquanto, mas uma assinatura só precisa resistir enquanto a conexão dura: quebrá-la depois não deixa ninguém decifrar uma gravação.
packets-mlkem-4-verdict = Seguro enquanto qualquer um dos dois se sustentar
packets-mlkem-p4 = Todo o resto como sempre, cifrado com as chaves do híbrido.
packets-mlkem-p5 = Finished. Nossas chaves batem.

packets-mlkem-5-title = Neste sistema
packets-mlkem-5-sub = Pós-quântico primeiro, nunca obrigatório
packets-mlkem-5-body = Este sistema oferece o híbrido primeiro onde quer que combine uma chave. Seu próprio cliente HTTPS é o rustls com prefer-post-quantum: X25519MLKEM768 primeiro e X25519 simples ao lado, então um servidor sem o híbrido continua sendo alcançado, de forma clássica. Seu SSH oferece primeiro o mlkem768x25519-sha256 — o mesmo ML-KEM-768 com X25519, padrão desde o OpenSSH 10.0 — e depois o sntrup761x25519-sha512, um híbrido mais antigo, antes de qualquer troca clássica.

## SSH

packets-ssh-name = SSH
packets-ssh-title = Secure Shell
packets-ssh-tagline = Um shell numa máquina distante, por uma conexão que ninguém consegue ler
packets-ssh-overview = O SSH dá a você um shell em outra máquina — e copia arquivos, encaminha portas e leva os pushes do git — por uma conexão que ninguém no caminho consegue ler nem alterar. Ele roda sobre o TCP e é construído em três camadas: um transporte que combina as chaves e prova a identidade do servidor com a sua chave de host, a autenticação do usuário em cima dele, e canais que levam um shell, uma transferência de arquivos e portas encaminhadas lado a lado. Ao contrário do TLS, ele não precisa de autoridade certificadora: o cliente guarda a chave de cada servidor na primeira vez que a vê, e recusa uma que mude.
packets-ssh-authentication = Uma chave pública, uma chave de segurança ou uma senha
packets-ssh-label-host-key = Chave de host
packets-ssh-label-fingerprint = Fingerprint

packets-ssh-1-title = Troca de versões
packets-ssh-1-sub = Cada lado diz qual é o seu software
packets-ssh-1-body = Você digita ssh admin@ssh.example.com; o ssh resolve o nome e abre uma conexão TCP com a porta 22, e antes de qualquer coisa cada lado envia uma linha de texto simples: SSH-2.0 seguido do nome do seu software. O 2.0 é a versão do protocolo — o SSH 1 foi aposentado há muito tempo — e o nome deixa cada lado contornar manias conhecidas do outro. Essas linhas são o único texto que o SSH envia: tudo depois delas são pacotes binários, cada um com o seu tamanho e o seu padding.
packets-ssh-p1 = Eu falo SSH 2.0; sou o OpenSSH 10.0.
packets-ssh-p2 = Eu também falo, e também sou.

packets-ssh-2-title = Negociação de algoritmos
packets-ssh-2-sub = Cada lado lista o que fala
packets-ssh-2-body = Cada lado envia um KEXINIT: depois de um cookie aleatório, para cada tarefa — troca de chaves, chave de host, cifra, MAC, compressão — os algoritmos que suporta, em ordem de preferência. Em cada tarefa vence o primeiro algoritmo da lista do cliente que o servidor também tenha, então não é preciso nenhuma ida e volta a mais. Aqui são o mlkem768x25519-sha256 para a troca de chaves, o ssh-ed25519 para a chave do servidor e o chacha20-poly1305@openssh.com para a cifra, que autentica cada pacote por conta própria, então nenhum MAC separado é usado.
packets-ssh-p3 = Aqui está tudo o que eu sei fazer, o melhor primeiro.
packets-ssh-p4 = E aqui está a minha lista; vence o primeiro da sua que eu tiver.

packets-ssh-3-title = Troca de chaves
packets-ssh-3-sub = Um segredo comum, pós-quântico
packets-ssh-3-body = O cliente envia o KEX_HYBRID_INIT: uma chave pública ML-KEM-768 nova e um share X25519, 1.216 bytes ao todo. O servidor encapsula um segredo para a chave ML-KEM, junta o seu próprio share X25519 e, a partir dos dois segredos e de tudo o que foi dito até ali, calcula o hash da troca H, que vira o identificador desta sessão. A resposta dele leva a sua chave pública de host, o texto cifrado com o seu share e uma assinatura de H feita com a chave de host — e então o NEWKEYS: a partir do próximo pacote, ele cifra.
packets-ssh-p5 = Minhas duas chaves públicas: uma pós-quântica, outra clássica.
packets-ssh-p6 = Seu segredo selado, meu share, minha chave de host e minha assinatura sobre tudo isso. Chaves novas daqui em diante.

packets-ssh-4-title = A identidade do servidor
packets-ssh-4-sub = O known_hosts, ou a pergunta da primeira vez
packets-ssh-4-body = O cliente confere a assinatura com a chave de host, então sabe que o servidor tem a metade privada dessa chave — mas é a chave certa? O ssh a procura no ~/.ssh/known_hosts: se bate, está resolvido, e uma chave diferente interrompe a conexão com um aviso bem visível, porque alguém pode estar se passando pelo servidor. Numa primeira conexão não há com o que comparar, então o ssh mostra o fingerprint da chave e pede que você a aceite — de preferência depois de conferi-lo com um que o dono do servidor lhe passou por outro meio. Então o cliente envia o seu próprio NEWKEYS, e daqui em diante todo pacote vai cifrado.
packets-ssh-p7 = Confio na sua chave. Chaves novas daqui em diante.

packets-ssh-5-title = Autenticação do usuário
packets-ssh-5-sub = Provar quem você é, com uma chave
packets-ssh-5-body = Dentro do canal cifrado o cliente pede o serviço ssh-userauth e então envia um USERAUTH_REQUEST: usuário admin, método publickey, a sua chave pública e uma assinatura sobre o identificador desta sessão. O servidor acha essa chave no ~/.ssh/authorized_keys do usuário e confere a assinatura, então nenhuma senha passa pelo fio, e um servidor que se passasse por este receberia uma assinatura inútil em qualquer outra sessão. Com uma chave de segurança (sk-ssh-ed25519), a metade privada nunca sai do dispositivo, e cada assinatura pede um toque.
packets-ssh-p8 = Sou o admin: aqui estão minha chave pública e minha assinatura para esta sessão.
packets-ssh-p9 = Pode entrar.

packets-ssh-6-title = Canal e shell
packets-ssh-6-sub = Uma sessão se abre; o prompt aparece
packets-ssh-6-body = Uma conexão SSH leva muitas conversas, chamadas canais: o cliente abre um do tipo session, e o servidor o confirma com o seu próprio número de canal e quantos dados aceita por enquanto (a sua janela). O cliente pede um terminal (pty-req) e um shell nele; daí em diante, o que você digita sobe como CHANNEL_DATA e o que o shell imprime desce do mesmo jeito. Encaminhamento de portas, scp e sftp são só outros canais na mesma conexão.
packets-ssh-p10 = Vamos abrir um canal: uma sessão.
packets-ssh-p11 = Canal aberto: me mande até 2 MB, e eu aviso quando houver espaço para mais.
packets-ssh-p12 = Um shell, por favor.
packets-ssh-p13 = O seu prompt.

## WireGuard

packets-wireguard-name = WireGuard
packets-wireguard-title = VPN WireGuard
packets-wireguard-tagline = Um túnel VPN pequeno, rápido e calado para estranhos
packets-wireguard-overview = O WireGuard liga uma máquina a uma rede privada distante por um túnel cifrado: cada pacote IP que o laptop envia para essa rede viaja selado dentro de um datagrama UDP até um gateway, que o abre e o repassa. Ele é pequeno de propósito — alguns milhares de linhas no kernel Linux, um conjunto fixo de algoritmos modernos e nada a negociar — e os pares são conhecidos só pelas suas chaves públicas, como as authorized keys do SSH. Um handshake de uma ida e volta monta chaves que se renovam a cada dois minutos, e a porta não responde a ninguém que já não tenha a chave de um par.
packets-wireguard-authentication = A chave pública de cada par, listada na configuração do outro
packets-wireguard-defined = O artigo do WireGuard (NDSS 2017); no Linux desde o 5.6
packets-wireguard-label-rekey = Novo handshake após

packets-wireguard-1-title = Pares e chaves
packets-wireguard-1-sub = Cada lado conhece a chave pública do outro
packets-wireguard-1-body = O WireGuard não tem nomes de usuário, certificados nem negociação: cada máquina tem um par de chaves, e a sua configuração lista os seus pares pelas chaves públicas deles. Com cada chave vai uma lista de IPs permitidos — o laptop manda o tráfego para 10.0.0.0/24 à chave do gateway, e o gateway aceita da chave do laptop só pacotes cuja origem seja 10.8.0.2. Esse casamento, chamado cryptokey routing, é todo o controle de acesso; e, a não ser que haja um PersistentKeepalive, subir a interface não envia nada, porque o WireGuard fica calado até haver um pacote para levar.

packets-wireguard-2-title = Início do handshake
packets-wireguard-2-sub = Noise IK, num pacote só
packets-wireguard-2-body = O primeiro pacote para 10.0.0.20 faz o laptop começar um handshake (tipo 1): uma chave pública efêmera nova, a sua própria chave pública estática cifrada para a do gateway e um timestamp cifrado. Como o laptop já conhece a chave do gateway, o handshake segue o padrão Noise IK e só precisa desta mensagem e de uma resposta; o timestamp deixa o gateway recusar uma cópia gravada e reenviada depois. O MAC1 é calculado com a chave pública do gateway, então só quem conhece essa chave consegue mandar um pacote que o gateway sequer olhe.
packets-wireguard-p1 = Vamos conversar: quem eu sou, selado para a sua chave, e a hora, para você saber que isto não é uma repetição.

packets-wireguard-3-title = Resposta do handshake
packets-wireguard-3-sub = Chaves para os dois sentidos
packets-wireguard-3-body = O gateway confere o MAC1, decifra a chave estática do laptop, a encontra entre os seus pares e confere se o timestamp é mais novo que o último que viu. Ele responde (tipo 2) com a sua própria chave efêmera, e os dois lados misturam quatro resultados Diffie-Hellman — chaves efêmeras e estáticas, cruzadas — num par de chaves de transporte, uma para cada sentido. O gateway ainda não manda dados: o primeiro pacote de dados do laptop vai confirmar que esta resposta chegou.
packets-wireguard-p2 = Conheço você. Aqui está a minha chave efêmera; agora temos chaves para os dois sentidos.

packets-wireguard-4-title = Dados de transporte
packets-wireguard-4-sub = Pacotes IP inteiros dentro do UDP
packets-wireguard-4-body = O ping do laptop — um pacote IP comum de 10.8.0.2 para 10.0.0.20 — recebe um preenchimento, é cifrado com ChaCha20-Poly1305 e vai como uma mensagem do tipo 4: o índice do gateway, um contador de 64 bits e o pacote selado. O gateway acha a sessão pelo índice, abre o pacote, recusa qualquer contador que já tenha visto, confere se a origem interna é um dos IPs permitidos do laptop e só então entrega o pacote interno à sua rede. A resposta faz o mesmo caminho de volta, selada com a chave do outro sentido.
packets-wireguard-p3 = Um pacote IP inteiro, selado: meu ping para 10.0.0.20.
packets-wireguard-p4 = Um ping de 10.8.0.2, saído do túnel.
packets-wireguard-p5 = Pong, de volta para 10.8.0.2.
packets-wireguard-p6 = A resposta, selada com a chave deste sentido.

packets-wireguard-5-title = Keepalives e chaves novas
packets-wireguard-5-sub = Manter o NAT aberto, renovar as chaves
packets-wireguard-5-body = Um roteador doméstico esquece um mapeamento UDP depois de algum tempo parado, muitas vezes menos de dois minutos, e aí nada do gateway alcançaria o laptop; com PersistentKeepalive = 25, o laptop envia uma mensagem do tipo 4 vazia — 32 bytes, um cabeçalho e uma tag — sempre que passam 25 segundos sem nada mais enviado. Depois de dois minutos de uso, o próximo pacote dispara um handshake novo, então uma chave roubada da memória abre só uns dois minutos de tráfego. E se o laptop muda de rede, o próximo pacote autenticado dele informa ao gateway o seu novo endereço.
packets-wireguard-p7 = Nada a dizer — só mantendo o caminho aberto.

packets-wireguard-6-title = Calado para estranhos
packets-wireguard-6-sub = Sem chave, sem resposta
packets-wireguard-6-body = Um scanner envia inícios de handshake para a porta 51820, procurando servidores VPN. Ele não conhece a chave pública do gateway, então o seu MAC1 está errado, e o gateway descarta o pacote depois de um hash barato — sem decifrar nada e sem responder, nem mesmo com um erro ICMP. Para quem não tem a chave de nenhum par, a porta é indistinguível de uma que um firewall descarta, e uma enxurrada de lixo quase não custa nada ao gateway.
packets-wireguard-6-verdict = Descartado, sem resposta: o MAC1 está errado
packets-wireguard-p8 = Tem alguém escutando aqui?

## IPsec

packets-ipsec-name = IPsec
packets-ipsec-title = IP Security: IKEv2 e ESP
packets-ipsec-tagline = Túneis cifrados para pacotes IP inteiros, embutidos no sistema operacional
packets-ipsec-overview = O IPsec protege os próprios pacotes IP, abaixo de qualquer aplicação: um laptop entrando na rede de um escritório, ou dois escritórios ligados pela internet como se um cabo corresse entre eles. Ele tem duas partes. O IKEv2 é a conversa que combina os algoritmos, prova quem é cada lado e cria associações de segurança (SAs) — conjuntos de chaves, cada um conhecido por um índice, o SPI; o ESP então leva o tráfego, cada pacote cifrado, autenticado e numerado. Ele vive no kernel da maioria dos sistemas operacionais, roteadores e firewalls, e por isso é a língua comum das VPNs corporativas.
packets-ipsec-transport = O ESP é o protocolo IP 50 — ou vai dentro do UDP 4500 quando há um NAT no caminho
packets-ipsec-authentication = Certificados, uma chave pré-compartilhada ou EAP
packets-ipsec-label-selectors = Seletores de tráfego

packets-ipsec-1-title = IKE_SA_INIT
packets-ipsec-1-sub = Propostas, Diffie-Hellman, nonces
packets-ipsec-1-body = O laptop abre com o IKE_SA_INIT na porta UDP 500: uma proposta de algoritmos (AES-GCM, SHA-256, Curve25519), o seu valor público Diffie-Hellman, um nonce aleatório e dois hashes dos endereços e portas que ele acredita estar usando. O gateway escolhe dentro da proposta e responde com o seu próprio valor público e nonce; a partir do segredo Diffie-Hellman comum e dos dois nonces, cada lado deriva as chaves da IKE SA, o canal protegido que o próprio IKE vai usar. Nada está autenticado ainda: os dois compartilham um segredo, mas nenhum sabe quem está do outro lado.
packets-ipsec-p1 = Aqui está o que eu sei usar, a minha metade de uma chave e onde eu acho que estou.
packets-ipsec-p2 = Vamos usar estes. Minha metade da chave — e você não está onde pensa: há um NAT entre nós.

packets-ipsec-2-title = IKE_AUTH
packets-ipsec-2-sub = Quem é cada lado, e o primeiro túnel
packets-ipsec-2-body = Os hashes de NAT não bateram — o roteador de casa reescreveu o endereço do laptop —, então daqui em diante o IKE passa para a porta UDP 4500, por onde os pacotes ESP também vão passar. Sob as chaves da IKE SA, o laptop envia a sua identidade, o seu certificado e o AUTH — uma assinatura sobre a sua primeira mensagem e o nonce do gateway, que amarra quem ele é a esta troca específica —, pede um endereço interno (CP) e propõe a primeira CHILD_SA com seletores de tráfego: qual tráfego o túnel vai levar. O gateway prova a sua identidade do mesmo jeito, dá ao laptop o 10.8.0.2 e estreita os seletores para 10.8.0.2/32 ↔ 10.0.0.0/24.
packets-ipsec-p3 = Este sou eu, assinado. Posso ter um endereço lá dentro, e um túnel para a sua rede?
packets-ipsec-p4 = E este sou eu. Seu endereço é 10.8.0.2; o túnel alcança 10.0.0.0/24.

packets-ipsec-3-title = ESP, em modo túnel
packets-ipsec-3-sub = Um pacote inteiro, selado dentro de outro
packets-ipsec-3-body = O laptop abre uma conexão com 10.0.0.20. O kernel vê que o destino casa com os seletores da CHILD_SA, então pega o pacote interno inteiro, de 10.8.0.2 para 10.0.0.20, cifra-o com a chave da CHILD_SA e o embrulha em ESP: o SPI que o gateway escolheu, um número de sequência, o texto cifrado e um ICV de 16 bytes — tudo dentro do UDP 4500 para atravessar o NAT. O gateway acha a SA pelo SPI, confere o ICV, decifra e encaminha o pacote interno como se o laptop estivesse na sua própria rede.
packets-ipsec-3-mode = Túnel: um pacote IP inteiro dentro de outro
packets-ipsec-p5 = Um pacote inteiro, selado: o número 1 desta SA.
packets-ipsec-p6 = Uma conexão de 10.8.0.2, saída do túnel.

packets-ipsec-4-title = O caminho de volta
packets-ipsec-4-sub = Números de sequência contra repetição
packets-ipsec-4-body = O servidor responde a 10.8.0.2, um endereço cuja rota leva de volta ao gateway, e o gateway sela a resposta na CHILD_SA que vai para o laptop, sob o SPI que o laptop escolheu e a chave própria desse sentido. Cada pacote ESP de uma SA leva um número de sequência um acima do anterior, e quem recebe guarda uma janela deslizante dos que já viu: um pacote gravado e reenviado, ou velho demais, é descartado antes mesmo de o seu ICV ser conferido. Enquanto o túnel está quieto, o laptop envia um keepalive de NAT de um byte pelo UDP 4500 a cada 20 segundos, para que o roteador mantenha o seu mapeamento.
packets-ipsec-p7 = Sim, vamos conectar — com 10.8.0.2.
packets-ipsec-p8 = A resposta, selada: o número 1 deste sentido.

packets-ipsec-5-title = Renovação de chaves
packets-ipsec-5-sub = Chaves novas antes de as velhas se desgastarem
packets-ipsec-5-body = Chaves não são usadas para sempre: depois de uma hora, ou de uma quantidade fixa de dados, o laptop roda um CREATE_CHILD_SA dentro da IKE SA, com nonces novos e, para ter forward secrecy, uma troca Diffie-Hellman nova. A CHILD_SA nova ganha SPIs novos e o tráfego passa para ela; depois uma troca INFORMATIONAL com um payload Delete aposenta a antiga. A própria IKE SA é renovada do mesmo jeito, com menos frequência, então um túnel pode ficar de pé por dias sem que as suas chaves envelheçam.
packets-ipsec-p9 = Hora de chaves novas: aqui estão nonces novos e um key share novo.
packets-ipsec-p10 = Combinado. SPIs novos, chaves novas.
packets-ipsec-p11 = A SA antiga pode ir embora.

## WPA3

packets-wpa3-name = WPA3
packets-wpa3-title = Wi-Fi Protected Access 3
packets-wpa3-tagline = Uma senha de Wi-Fi provada, nunca enviada, e inútil para quem escuta
packets-wpa3-overview = O WPA3 decide quem pode entrar numa rede Wi-Fi protegida por senha e dá a cada aparelho as suas próprias chaves para cifrar o que ele envia pelo ar. No WPA2, quem gravasse um aparelho entrando na rede podia levar a gravação para casa e testar senhas contra ela, milhões por segundo, até uma servir. O WPA3-Personal troca isso pelo SAE, Simultaneous Authentication of Equals (a troca "Dragonfly"): os dois lados provam que sabem a senha sem enviá-la, nem nada contra o que um palpite possa ser testado. Cada conexão ganha uma chave nova, então uma senha descoberta depois não abre gravações antigas, e os quadros que gerenciam a conexão também são protegidos.
packets-wpa3-authentication = SAE: os dois lados provam a senha sem enviá-la
packets-wpa3-security = Uma chave nova a cada conexão; quadros de gerenciamento protegidos

packets-wpa3-1-title = O beacon
packets-wpa3-1-sub = A rede diz o que exige
packets-wpa3-1-body = Umas dez vezes por segundo o ponto de acesso envia um beacon, e o elemento RSN dele — a parte que trata de segurança — diz o que é preciso para entrar: SAE para autenticar, CCMP para cifrar e quadros de gerenciamento protegidos, obrigatórios. Uma rede em modo de transição também aceita WPA2 com o mesmo nome, para aparelhos antigos, e por isso deixa a fraqueza do WPA2 ao alcance de um atacante.
packets-wpa3-1-mode = WPA3-Personal, quadros de gerenciamento protegidos obrigatórios
packets-wpa3-p1 = Sou a rede example: entre com SAE e proteja seus quadros de gerenciamento.

packets-wpa3-2-title = Commit do SAE
packets-wpa3-2-sub = Cada lado se compromete com um segredo
packets-wpa3-2-body = O celular e o ponto de acesso transformam, cada um, a senha e os dois endereços MAC num ponto secreto de uma curva elíptica, o elemento da senha. Cada um sorteia então dois números aleatórios e envia um commit: um escalar e um elemento construídos a partir deles e desse ponto. Para quem escuta, parecem números aleatórios, novos a cada vez, e não revelam nada da senha; e nenhum lado começa por direito — são iguais, daí o nome.
packets-wpa3-p2 = Meu escalar e meu elemento: feitos da senha, mas não a mostram.
packets-wpa3-p3 = E os meus, feitos com números aleatórios novos, só meus.

packets-wpa3-3-title = Confirm do SAE
packets-wpa3-3-sub = Prova sem a senha
packets-wpa3-3-body = Com o próprio número aleatório e o commit do outro, cada lado calcula um segredo compartilhado, que só sai igual se os dois partiram da mesma senha. Cada um envia um confirm — um HMAC da troca inteira, com uma chave tirada desse segredo — que prova que ele tem a senha sem revelá-la. O atacante que gravou tudo não consegue testar palpites em casa: cada palpite exige uma nova troca, ao vivo, com o ponto de acesso. E a PMK que sai daqui serve só a esta conexão, então descobrir a senha depois não abre gravação nenhuma (forward secrecy).
packets-wpa3-3-verdict = Mesma senha, mesmo segredo: uma PMK combinada
packets-wpa3-p4 = Prova de que calculei o mesmo segredo: eu sei a senha.
packets-wpa3-p5 = O meu também confere. Temos uma PMK nova.

packets-wpa3-4-title = Associação
packets-wpa3-4-sub = O celular entra na rede
packets-wpa3-4-body = Autenticado, o celular envia um Association Request dizendo o que escolheu do beacon — SAE, CCMP e quadros de gerenciamento protegidos — e o ponto de acesso responde com status 0 e um ID de associação. O celular agora faz parte da rede, mas a porta dele continua fechada: o ponto de acesso não deixa passar dado nenhum dele até os dois lados terem as chaves de sessão.
packets-wpa3-4-state = Associado; nenhum dado até as chaves existirem
packets-wpa3-p6 = Posso entrar, com SAE, CCMP e quadros de gerenciamento protegidos?
packets-wpa3-p7 = Bem-vindo: você é a associação 1.

packets-wpa3-5-title = O handshake de quatro vias
packets-wpa3-5-sub = Chaves de sessão a partir da PMK
packets-wpa3-5-body = O ponto de acesso envia um ANonce aleatório; o celular responde com o seu SNonce, e os dois misturam a PMK, os dois nonces e os dois endereços MAC nas chaves de sessão. Da mensagem 2 em diante, cada mensagem leva um MIC feito com essas chaves, para cada lado ver que o outro chegou às mesmas; a mensagem 3 ainda entrega, cifradas, as chaves de grupo para broadcasts e para os quadros de gerenciamento protegidos (GTK, IGTK). É o handshake do WPA2, mas a PMK por baixo dele é uma chave aleatória nova, e não uma feita da senha: gravá-lo não dá nada para adivinhar.
packets-wpa3-5-state = Chaves instaladas; os dados passam cifrados
packets-wpa3-p8 = Aqui está o meu nonce.
packets-wpa3-p9 = Aqui está o meu — e um MIC que mostra que tenho a PMK.
packets-wpa3-p10 = Instale as chaves; aqui estão as chaves de grupo, cifradas.
packets-wpa3-p11 = Chaves instaladas.

packets-wpa3-6-title = Gerenciamento protegido
packets-wpa3-6-sub = Um adeus forjado é ignorado
packets-wpa3-6-body = O atacante forja um quadro de desautenticação, fingindo ser o ponto de acesso, para derrubar o celular da rede — o jeito clássico de cortar uma conexão, ou de fazer um aparelho se reconectar enquanto alguém grava. O WPA3 exige que esses quadros venham protegidos com as chaves de sessão, e este não vem, então o celular o descarta. Na dúvida, o celular pode perguntar ao ponto de acesso, pelo enlace protegido, se a associação terminou mesmo (SA Query).
packets-wpa3-6-verdict = Sem proteção: descartado
packets-wpa3-p12 = Sou o ponto de acesso: você está desconectado.

## RADIUS

packets-radius-name = RADIUS / 802.1X
packets-radius-title = Remote Authentication Dial-In User Service
packets-radius-tagline = Cada pessoa com o seu acesso à rede, verificado por um servidor
packets-radius-overview = Numa rede de escritório ou de universidade, uma senha de Wi-Fi única, compartilhada por todos, não serve: pessoas saem, laptops se perdem, e cada pessoa deveria ter o próprio acesso. Com o 802.1X, o ponto de acesso não deixa nada passar até um servidor RADIUS central verificar quem está se conectando — por um certificado ou uma senha, cada pessoa com o seu. O laptop (o supplicant) fala EAP com o ponto de acesso (o autenticador), que o repassa, sem lê-lo, dentro do RADIUS ao servidor; o servidor decide, entrega ao ponto de acesso uma chave só para esta sessão e diz em qual rede (VLAN) pôr o laptop. O eduroam, nas universidades do mundo todo, funciona assim.
packets-radius-port = UDP 1812; accounting na 1813
packets-radius-model = Supplicant, autenticador e servidor de autenticação
packets-radius-device-directory = Diretório

packets-radius-1-title = Quem é você?
packets-radius-1-sub = O ponto de acesso pergunta; o servidor vai decidir
packets-radius-1-body = O laptop já entrou na rede de rádio, mas o ponto de acesso mantém a porta dele fechada para tudo menos EAP, levado pelo EAPOL. Ele pede uma identidade, e o laptop responde user@example.org. O ponto de acesso não julga a resposta: põe-na, sem mudar nada, num Access-Request do RADIUS para o servidor na UDP 1812, com um Message-Authenticator calculado a partir de um segredo que só os dois conhecem.
packets-radius-p1 = Quem é você?
packets-radius-p2 = Sou user@example.org.
packets-radius-p3 = Alguém diz ser user@example.org — aqui está a mensagem EAP dele.

packets-radius-2-title = O certificado do servidor
packets-radius-2-sub = TLS, levado dentro do EAP
packets-radius-2-body = O servidor começa o EAP-TLS: um handshake TLS 1.3 completo levado dentro de mensagens EAP, que o ponto de acesso repassa sem conseguir lê-las. A resposta do servidor não cabe numa mensagem só, então viaja em fragmentos, cada um confirmado. O laptop confere o certificado — assinado pela CA em que foi configurado para confiar, e com o nome radius.example.org; um laptop que pulasse essa conferência entraria em qualquer ponto de acesso falso que copiasse o nome da rede.
packets-radius-p4 = Vamos começar o TLS: aqui está meu key share.
packets-radius-p5 = Aqui está meu certificado, assinado pela sua CA — e quero ver o seu.

packets-radius-3-title = O certificado do laptop
packets-radius-3-sub = O servidor confere quem ele é
packets-radius-3-body = Agora o laptop prova quem é do mesmo jeito: o próprio certificado, uma assinatura feita com a chave privada dele (CertificateVerify) e a mensagem Finished, que fecha o handshake. O servidor confere o certificado contra a CA da organização e depois pergunta ao diretório se a conta ainda existe e de quais grupos ela faz parte. Um laptop perdido é barrado revogando o certificado dele ou desativando a conta — ninguém mais precisa mudar nada.
packets-radius-3-verdict = Certificado válido, conta ativa
packets-radius-p6 = Aqui está meu certificado, e a prova de que tenho a chave privada dele.
packets-radius-p7 = user@example.org ainda existe? Em quais grupos?
packets-radius-p8 = Ativa, no grupo staff.

packets-radius-4-title = Access-Accept
packets-radius-4-sub = Uma chave e uma VLAN
packets-radius-4-body = O servidor responde Access-Accept. Dentro vão o EAP-Success para o laptop, a chave da sessão — a MSK, que o laptop e o servidor derivaram do handshake TLS, cifrada com o segredo compartilhado — e a VLAN do grupo do laptop: 20. O ponto de acesso deriva a PMK dessa chave, faz o handshake de quatro vias que define a criptografia do Wi-Fi e abre a porta; só então o laptop pede um endereço IP, por DHCP, na VLAN 20.
packets-radius-4-encryption = PMK a partir da MSK, depois o handshake de quatro vias
packets-radius-p9 = Pode deixar entrar: aqui está a chave da sessão; ponha na VLAN 20.
packets-radius-p10 = Você entrou.

packets-radius-5-title = Accounting
packets-radius-5-sub = A sessão é registrada
packets-radius-5-body = Com o laptop na rede, o ponto de acesso envia um Accounting-Request — Acct-Status-Type Start — para a UDP 1813, e o servidor o confirma. Depois vêm atualizações intermediárias e um registro Stop, com o tempo conectado e os bytes trafegados: o registro que uma organização usa para responder quem esteve na rede, e quando.
packets-radius-p11 = A sessão de user@example.org começou.
packets-radius-p12 = Registrado.

## Tor

packets-tor-name = Tor
packets-tor-title = The onion router
packets-tor-tagline = Três relés entre você e o site, e nenhum deles conhece as duas pontas
packets-tor-overview = O Tor esconde quem fala com quem. Em vez de se conectar direto a um site, o cliente manda o tráfego por três relés mantidos por voluntários — um de guarda, um do meio e um de saída — embrulhado em três camadas de criptografia, uma para cada. Cada relé tira a sua camada e só conhece o relé anterior e o seguinte: o de guarda sabe quem você é, mas não aonde vai; o de saída sabe aonde você vai, mas não quem você é. O site vê o endereço do relé de saída, nunca o seu.
packets-tor-port = 9001 ou 443 entre os relés; 9050 para os programas locais (SOCKS)
packets-tor-encryption = Uma camada por relé, três ao todo
packets-tor-security = Nenhum relé sabe ao mesmo tempo quem você é e aonde você vai

packets-tor-1-title = Escolhendo três relés
packets-tor-1-sub = Um caminho a partir do consenso
packets-tor-1-body = A cada hora, as autoridades de diretório do Tor publicam um consenso: a lista dos relés, suas chaves e o que cada um aceita fazer. O cliente escolhe nele um caminho: um relé de guarda, que mantém por meses para que um relé hostil tenha poucas chances de ser o seu primeiro salto; um do meio; e um de saída cuja política permita a porta 443. Dois deles nunca podem ser da mesma família nem da mesma rede /16, para que um mesmo operador dificilmente tenha as duas pontas.

packets-tor-2-title = O relé de guarda
packets-tor-2-sub = A primeira chave
packets-tor-2-body = O cliente abre uma conexão TLS com o relé de guarda e envia CREATE2, com a primeira metade de um handshake ntor: uma chave pública nova, e a identidade e a chave onion que espera que o relé tenha. O relé responde CREATED2 com a sua metade e uma prova de que tem aquela chave. Com as duas metades, os dois calculam K1, uma chave que mais ninguém tem: a primeira camada.
packets-tor-p1 = Aqui está a minha metade de uma troca de chaves.
packets-tor-p2 = E aqui está a minha: agora temos K1 em comum.

packets-tor-3-title = Estendendo o circuito
packets-tor-3-sub = Uma chave com cada relé, através dos outros
packets-tor-3-body = Para incluir o relé do meio, o cliente manda o próximo handshake dentro de uma célula EXTEND2, cifrada com K1; o relé de guarda abre a sua própria conexão com o do meio, envia CREATE2 com um novo ID de circuito e devolve a resposta como EXTENDED2. O relé do meio só vê o de guarda e nunca sabe quem é o cliente, mas K2 fica combinada com o cliente, porque o de guarda só repassou valores públicos e não consegue forjar a prova do relé do meio. O mesmo se repete, através do de guarda e do do meio, para chegar ao relé de saída e combinar K3.
packets-tor-p3 = Guarda, estenda este circuito até aquele relé e repasse este handshake.
packets-tor-p4 = Alguém quer uma chave com você — quem, eu não digo.
packets-tor-p5 = Aqui está a minha metade, para quem pediu.
packets-tor-p6 = O relé do meio respondeu: aqui está a metade dele.

packets-tor-4-title = Três camadas
packets-tor-4-sub = Cada relé descasca uma
packets-tor-4-body = O cliente pede ao relé de saída que abra uma conexão com www.example.com:443 — uma célula BEGIN — e a cifra três vezes: com K3, depois K2, depois K1. O relé de guarda tira K1 e encontra uma célula que não consegue ler, então a passa adiante; o do meio tira K2 e faz o mesmo; o de saída tira K3 e lê BEGIN. O nome viaja dentro do circuito e quem o resolve é o relé de saída, então nem o DNS do cliente entrega o destino.
packets-tor-p7 = Três camadas: só o relé de saída vai ler o que tem dentro.
packets-tor-p8 = Uma camada a menos, e continua ilegível: vou passando.
packets-tor-p9 = Mais uma camada a menos; a última é sua, relé de saída.

packets-tor-5-title = A saída se conecta
packets-tor-5-sub = O site vê o relé de saída
packets-tor-5-body = O relé de saída abre uma conexão TCP comum com o servidor web, a partir do próprio endereço: os logs do site mostram 203.0.113.99, nunca o endereço do cliente. Ele responde CONNECTED, e na volta cada relé acrescenta a sua camada — o de saída K3, o do meio K2, o de guarda K1 —, que o cliente então tira, as três. O TLS do cliente com o site corre dentro do circuito, de ponta a ponta: o relé de saída o repassa, mas não consegue lê-lo — sem HTTPS, conseguiria.
packets-tor-p10 = Vamos nos conectar: até onde você sabe, eu sou 203.0.113.99.
packets-tor-p11 = Olá, 203.0.113.99.
packets-tor-p12 = Conectado — embrulhado de novo, uma camada a cada relé na volta.

packets-tor-6-title = Serviços onion
packets-tor-6-sub = E o Tor neste sistema
packets-tor-6-body = Um site também pode viver dentro do Tor, como serviço onion: o endereço .onion dele é a própria chave pública, e cliente e serviço montam, cada um, um circuito até um relé de encontro (rendezvous), de modo que nenhum dos dois sabe onde o outro está e nenhum relé de saída participa. Neste sistema o Tor fica desligado até você ligá-lo na seção dele na vista de rede; aí ele oferece um endereço SOCKS local — o que a seção dele mostra — aos programas que você apontar para ele, enquanto o resto da máquina segue com a conexão comum.
packets-tor-6-identity = Um endereço .onion é a própria chave pública do serviço
packets-tor-6-address = Um endereço SOCKS local, mostrado na vista de rede

## NTP

packets-ntp-name = NTP
packets-ntp-title = Network Time Protocol
packets-ntp-tagline = Mantém todo relógio na hora certa
packets-ntp-overview = O relógio de um computador se desvia um segundo ou mais por dia, e ainda assim certificados, logs, builds e códigos de verificação em duas etapas dependem da hora certa. O NTP pergunta a hora a um servidor de hora — e, como a resposta demora a chegar, mede quanto tempo levou a viagem e a desconta, com quatro carimbos de tempo. Os servidores de hora formam uma hierarquia, contada em estratos: relógios atômicos e GPS no topo, servidores que os seguem abaixo. Pela internet ele mantém um relógio a poucos milissegundos da hora certa, e o ajusta aos poucos, em vez de fazê-lo saltar.
packets-ntp-format = 48 bytes: quatro carimbos de tempo, o estrato e a precisão
packets-ntp-security = Sem autenticação por padrão; o NTS (RFC 8915) a acrescenta
packets-ntp-device-stratum-one = Servidor de estrato 1

packets-ntp-1-title = O pedido
packets-ntp-1-sub = O laptop anota quando pergunta: T1
packets-ntp-1-body = O laptop envia um pequeno pacote UDP para a porta 123 do servidor de hora, no modo 3: um cliente perguntando. No carimbo de transmissão ele escreve a hora do seu próprio relógio no instante em que o pacote sai — T1, 10:00:00,000. Os carimbos do NTP contam os segundos desde 1º de janeiro de 1900, com uma fração fina o bastante para menos de um nanossegundo.
packets-ntp-p1 = Que horas são? Pelo meu relógio, estou perguntando às 10:00:00,000.

packets-ntp-2-title = Os carimbos do servidor
packets-ntp-2-sub = T2 quando chega, T3 quando sai
packets-ntp-2-body = Pelo seu relógio preciso, o servidor de hora anota quando o pedido chegou, T2 = 10:00:00,062, e quando a resposta sai, T3 = 10:00:00,063. Ele também copia o T1 do laptop na resposta, para o laptop saber a qual pedido ela responde, e acrescenta o seu estrato: 2, a dois passos de um relógio de referência. A resposta vai no modo 4: um servidor respondendo.
packets-ntp-p2 = Você perguntou às 10:00:00,000; chegou aqui às ,062, e estou respondendo às ,063.

packets-ntp-3-title = A conta
packets-ntp-3-sub = Quatro carimbos dão o desvio e o atraso
packets-ntp-3-body = A resposta chega em T4 = 10:00:00,025 pelo relógio do laptop. A ida e volta, menos o milissegundo do próprio servidor, é o atraso: (T4 − T1) − (T3 − T2) = 25 − 1 = 24 ms. O desvio supõe que a viagem levou o mesmo tempo em cada sentido: ((T2 − T1) + (T3 − T4)) / 2 = (62 + 38) / 2 = 50 ms, então o relógio do laptop está 50 ms atrasado. Um caminho mais lento num sentido do que no outro é o único erro que esse método não consegue ver.

packets-ntp-4-title = O estrato
packets-ntp-4-sub = De onde vem a hora do próprio servidor
packets-ntp-4-body = O servidor de hora também é um cliente: a cada dois minutos, mais ou menos, ele pergunta a um servidor de estrato 1, do mesmo jeito. Um servidor de estrato 1 fica ligado a um relógio de referência — um receptor GPS, um sinal de rádio ou um relógio atômico, o estrato 0 — e o seu ID de referência o nomeia: GPS. Cada degrau abaixo soma um, então o servidor de hora é estrato 2 e o laptop, quando passa a segui-lo, estrato 3; o número diz quão longe um relógio está da fonte.
packets-ntp-p3 = Que horas são? Nisto eu também sou cliente.
packets-ntp-p4 = Estrato 1, direto do GPS: aqui estão os seus carimbos.

packets-ntp-5-title = Ajustando aos poucos
packets-ntp-5-sub = O relógio é corrigido devagar, sem saltos
packets-ntp-5-body = O laptop não faz o relógio pular 50 ms para a frente: ele o faz andar um pouco mais rápido até a diferença sumir, porque um relógio que salta — pior, um que volta para trás — confunde logs, timers e builds. Só um erro grande, em geral na hora do boot, é corrigido de uma vez. O laptop continua perguntando, primeiro a cada 64 segundos e, quando o relógio se firma, até a cada 17 minutos; quatro minutos depois, o desvio caiu para 0 ms.
packets-ntp-5-state = Ajustado aos poucos, nunca voltou para trás
packets-ntp-p5 = Eu de novo. Pelo meu relógio, estou perguntando às 10:04:16,000.
packets-ntp-p6 = Chegou aqui às ,012, e estou respondendo às ,013.

## MQTT

packets-mqtt-name = MQTT
packets-mqtt-title = MQ Telemetry Transport
packets-mqtt-tagline = Publicar e assinar, para pequenos aparelhos em redes instáveis
packets-mqtt-overview = O MQTT conecta pequenos aparelhos — sensores, interruptores, medidores — que podem dormir, funcionar a bateria e ficar em redes pouco confiáveis. Nenhum cliente fala direto com outro: cada um mantém uma conexão TCP com um broker, publica mensagens em tópicos como home/kitchen/temperature e assina os tópicos que lhe interessam; o broker entrega cada mensagem a cada assinante. Quem publica e quem assina nunca precisam saber o endereço um do outro, nem estar online ao mesmo tempo. O cabeçalho fixo tem dois bytes no mínimo, o que cabe num microcontrolador.
packets-mqtt-model = Publicar/assinar, por meio de um broker

packets-mqtt-1-title = Conectando
packets-mqtt-1-sub = O sensor se apresenta ao broker
packets-mqtt-1-body = O sensor abre uma conexão TCP com o broker — porta 1883, ou 8883 com TLS, que é o que qualquer coisa fora de um laboratório deveria usar — e envia CONNECT: seu ID de cliente, um keep-alive de 60 segundos e uma última vontade (last will) para o broker publicar se ele sumir. O broker responde CONNACK com o código 0: aceito. A conexão fica aberta; o sensor só precisa mandar alguma coisa, nem que seja um PINGREQ, a cada keep-alive.
packets-mqtt-p1 = Sou kitchen-sensor. Se eu sumir, avise a todos que estou offline.
packets-mqtt-p2 = Aceito.

packets-mqtt-2-title = Assinando
packets-mqtt-2-sub = O painel pede um tópico
packets-mqtt-2-body = O painel, conectado do mesmo jeito, assina home/kitchen/temperature com QoS 1, e o broker confirma com SUBACK, concedendo esse QoS. Uma assinatura pode usar curingas: + vale por um nível (home/+/temperature, todos os cômodos) e # por tudo o que vem abaixo (home/#, a casa inteira). O painel não sabe que o sensor existe; só conhece o tópico.
packets-mqtt-p3 = Me mande tudo de home/kitchen/temperature.
packets-mqtt-p4 = Assinado, com QoS 1.

packets-mqtt-3-title = Publicando
packets-mqtt-3-sub = Uma leitura, repassada
packets-mqtt-3-body = O sensor publica 21.5 em home/kitchen/temperature com QoS 1 e a flag retain, e o broker responde PUBACK: até isso chegar, o sensor guarda a mensagem e vai enviá-la de novo. O broker a repassa a cada assinatura que combina com o tópico — aqui, o painel, que confirma por sua vez. Com QoS 0 ela iria uma vez só, sem confirmação; o QoS 2 acrescenta uma segunda rodada (PUBREC, PUBREL, PUBCOMP) para que ela chegue exatamente uma vez.
packets-mqtt-p5 = Temperatura da cozinha: 21.5 — guarde como a mais recente.
packets-mqtt-p6 = Recebido; pode esquecer.
packets-mqtt-p7 = Novidade em home/kitchen/temperature: 21.5.
packets-mqtt-p8 = Recebido.

packets-mqtt-4-title = A mensagem retida
packets-mqtt-4-sub = Quem chega recebe o último valor
packets-mqtt-4-body = Mais tarde o celular se conecta e assina home/kitchen/#. Não precisa esperar a próxima leitura: o broker guardou a última mensagem publicada com a flag retain em cada tópico e a envia na hora, marcada RETAIN. Um tópico guarda uma só mensagem retida, trocada a cada nova: o último valor conhecido dele.
packets-mqtt-p9 = Tudo o que estiver em home/kitchen, por favor.
packets-mqtt-p10 = Assinado.
packets-mqtt-p11 = O último que tenho: 21.5, guardado de antes.

packets-mqtt-5-title = A última vontade
packets-mqtt-5-sub = O silêncio, anunciado
packets-mqtt-5-body = A bateria do sensor acaba. Nada chega dele por uma vez e meia o keep-alive, então o broker derruba a conexão e publica a última vontade que o sensor deixou ao se conectar: offline, em home/kitchen/status — para o celular, cujo home/kitchen/# a abrange. Um sensor que sai direito, com DISCONNECT, tem a última vontade descartada; e como esta foi retida, quem assinar depois também fica sabendo que o sensor sumiu.
packets-mqtt-p12 = kitchen-sensor sumiu: offline.

## Modbus/TCP

packets-modbus-name = Modbus/TCP
packets-modbus-title = Modbus sobre TCP/IP
packets-modbus-tagline = Lê e escreve os números que fazem as máquinas funcionarem
packets-modbus-overview = O Modbus é a língua simples do controle industrial: desde 1979, uma estação pede a um dispositivo — um CLP, um medidor, um inversor de motor — que leia ou escreva valores numerados, e o dispositivo responde. Todo o modelo de dados dele são quatro tabelas: bobinas, ou coils (bits que podem ser escritos, como o liga-desliga de uma bomba), entradas discretas (bits que só podem ser lidos) e holding registers e registradores de entrada (números de 16 bits, graváveis e só de leitura). O Modbus/TCP embrulha os mesmos pedidos num cabeçalho MBAP de sete bytes e os envia à porta 502. Ele foi feito para linhas seriais fechadas, e isso se nota: não há autenticação nem criptografia nenhuma.
packets-modbus-model = Cliente e servidor: uma estação pergunta, um dispositivo responde
packets-modbus-security = Nenhuma: sem autenticação, sem criptografia

packets-modbus-1-title = Lendo registradores
packets-modbus-1-sub = A estação pede quatro valores
packets-modbus-1-body = Com uma conexão TCP aberta para a porta 502, a estação SCADA envia a função 03, ler holding registers: unidade 1, a partir do endereço 0, quatro deles. O CLP responde com oito bytes, quatro números de 16 bits. O que eles significam — o nível de um tanque em centímetros, uma temperatura em décimos de grau — está no mapa de registradores do CLP, não no fio; o ID de transação só casa cada resposta com a sua pergunta.
packets-modbus-1-function = 03, ler holding registers
packets-modbus-p1 = Unidade 1: leia os holding registers de 0 a 3.
packets-modbus-p2 = Aqui estão: 482, 215, 0 e 1.

packets-modbus-2-title = Escrevendo uma bobina
packets-modbus-2-sub = A estação liga a bomba
packets-modbus-2-body = Para ligar a bomba, a estação envia a função 05, escrever uma bobina: bobina 10, valor 0xFF00, que quer dizer ligada (0x0000 é desligada). O CLP aciona a saída e responde ecoando o pedido, byte a byte — esse eco é toda a confirmação. Doze bytes, e algo se move no mundo físico.
packets-modbus-2-function = 05, escrever uma bobina (coil)
packets-modbus-p3 = Ligue a bobina 10.
packets-modbus-p4 = Bobina 10 ligada.

packets-modbus-3-title = Uma exceção
packets-modbus-3-sub = Um endereço que não existe
packets-modbus-3-body = A estação pede quatro registradores a partir do endereço 500, que este CLP não tem. Ele responde com o código da função mais 0x80 — 0x83 — e o código de exceção 02, endereço de dados inválido. Outros códigos dizem que a função não é suportada (01), que um valor está fora da faixa (03) ou que o próprio dispositivo falhou (04).
packets-modbus-3-reason = Endereço de dados inválido
packets-modbus-p5 = Leia quatro registradores a partir do endereço 500.
packets-modbus-p6 = Exceção 02: esse endereço não existe.

packets-modbus-4-title = Ninguém pergunta quem
packets-modbus-4-sub = Quem está na rede é obedecido
packets-modbus-4-body = Um laptop ligado no mesmo switch — de um terceirizado, ou controlado por um invasor — envia a mesma função 05 com 0x0000, e a bomba para. O CLP não tem como distingui-lo da estação SCADA: o Modbus não tem usuários, senhas nem assinaturas, e tudo viaja às claras, então quem alcança a porta 502 lê e escreve qualquer valor. Malwares feitos para atacar instalações reais já usaram exatamente isso.
packets-modbus-4-verdict = Obedecido: ninguém pergunta quem mandou
packets-modbus-p7 = Desligue a bobina 10.
packets-modbus-p8 = Bobina 10 desligada.

packets-modbus-5-title = Mantendo à parte
packets-modbus-5-sub = A rede é que precisa defendê-lo
packets-modbus-5-body = Como o protocolo não se defende, a rede precisa defendê-lo: a rede de controle fica separada da rede do escritório e da internet, e um firewall entre elas só deixa a estação SCADA alcançar a porta 502 — zonas e conduítes, nas palavras da IEC 62443. Uma variante mais nova, o Modbus/TCP Security, embrulha as mesmas mensagens em TLS, com certificados, na porta 802, mas os dispositivos já instalados vão falar Modbus puro por décadas.
packets-modbus-5-scope = Só a estação SCADA alcança a porta 502

## BitTorrent

packets-bittorrent-name = BitTorrent
packets-bittorrent-title = O protocolo BitTorrent
packets-bittorrent-tagline = Um arquivo grande, compartilhado por todos que o baixam
packets-bittorrent-overview = O BitTorrent compartilha um arquivo grande entre todos que o querem, em vez de cada um baixá-lo de um mesmo servidor. O arquivo é cortado em pedaços de tamanho igual, cujos hashes se conhecem de antemão: um cliente baixa pedaços diferentes de muitos pares ao mesmo tempo, confere cada um contra o seu hash e logo oferece aos outros o que já tem. Quanto mais gente baixa, mais fontes existem — o enxame traz a própria oferta. E o arquivo não é nomeado por um lugar, mas pelo seu info hash, o hash da sua descrição, então dá para confiar nele venha de quem vier.
packets-bittorrent-port = 6881 por tradição; qualquer porta serve
packets-bittorrent-model = Par a par: quem baixa também envia
packets-bittorrent-device-seed = Seed

packets-bittorrent-1-title = O info hash
packets-bittorrent-1-sub = O que o arquivo é, não onde está
packets-bittorrent-1-body = Você abre um link magnet. O coração dele é o info hash: o SHA-1 do dicionário info do torrent, que dá o nome e o tamanho do arquivo, o tamanho dos pedaços — 256 KiB aqui, então 2.800 pedaços — e o hash de cada pedaço. Um arquivo .torrent já traz esse dicionário; a partir de um link magnet, o cliente o busca nos primeiros pares e o confere contra o info hash.

packets-bittorrent-2-title = Achando pares
packets-bittorrent-2-sub = O tracker conhece o enxame
packets-bittorrent-2-body = O cliente se anuncia ao tracker citado no link: um GET HTTP com o info hash, o seu peer ID, a porta em que escuta e quanto ainda falta (left). O tracker responde em bencode com os pares que compartilham aquele info hash — seis bytes cada, endereço e porta — e quanto esperar antes de se anunciar de novo. Sem tracker, o cliente acha pares na DHT, uma tabela espalhada pelos próprios pares.
packets-bittorrent-p1 = Estou entrando no enxame deste info hash. Quem mais está nele?
packets-bittorrent-p2 = Dois pares, com os endereços. Volte daqui a 30 minutos.

packets-bittorrent-3-title = O handshake
packets-bittorrent-3-sub = Temos o mesmo arquivo?
packets-bittorrent-3-body = O cliente abre uma conexão TCP com o seed e envia o handshake: a string BitTorrent protocol, oito bytes reservados que anunciam as extensões que ele fala, o info hash e o seu peer ID. O seed responde com o mesmo info hash — um par de outro torrent desligaria — e depois um bitfield, um bit por pedaço: os 2.800 ligados, já que um seed tem o arquivo inteiro. O cliente faz o mesmo com cada par que o tracker indicou.
packets-bittorrent-p3 = Vamos compartilhar este torrent? Aqui estão o info hash dele e o meu ID.
packets-bittorrent-p4 = Mesmo torrent. Aqui está o meu ID.
packets-bittorrent-p5 = Tenho todos os pedaços.

packets-bittorrent-4-title = Pedindo blocos
packets-bittorrent-4-sub = De vários pares ao mesmo tempo
packets-bittorrent-4-body = O cliente diz que está interessado e espera ser liberado (unchoke) — um par só envia para alguns outros de cada vez. Depois pede blocos de 16 KiB, vários de uma vez, ao seed e ao outro par em paralelo, escolhendo primeiro os pedaços que menos pares têm (rarest first), o que mantém cada pedaço vivo no enxame. Cada bloco volta numa mensagem PIECE com o número do pedaço e o deslocamento.
packets-bittorrent-p6 = Quero alguns dos seus pedaços.
packets-bittorrent-p7 = Pode pedir.
packets-bittorrent-p8 = Pedaço 0, a partir do byte 0: 16 KiB, por favor.
packets-bittorrent-p9 = Pedaço 1913, a partir do byte 0: 16 KiB, por favor.
packets-bittorrent-p10 = Aqui está aquele bloco do pedaço 0.
packets-bittorrent-p11 = Aqui está aquele bloco do pedaço 1913.

packets-bittorrent-5-title = Conferindo e compartilhando
packets-bittorrent-5-sub = Cada pedaço conferido, depois oferecido
packets-bittorrent-5-body = Quando os dezesseis blocos do pedaço 0 chegam, o cliente calcula o hash deles e o compara com o do dicionário info (SHA-1 nos torrents v1, SHA-256 nos v2); um pedaço que não confere é jogado fora e buscado de novo, então um par mentiroso não consegue estragar o arquivo. O cliente anuncia HAVE 0, e o par, que não o tem, o pede. Os pares enviam primeiro para quem envia para eles (tit-for-tat), então compartilhar é o que mantém rápido o próprio download do cliente.
packets-bittorrent-5-verdict = O hash confere: o pedaço fica e é compartilhado
packets-bittorrent-p12 = Agora tenho o pedaço 0, conferido.
packets-bittorrent-p13 = Então me mande o pedaço 0, a partir do byte 0.
packets-bittorrent-p14 = Aqui está: 16 KiB do pedaço 0.
