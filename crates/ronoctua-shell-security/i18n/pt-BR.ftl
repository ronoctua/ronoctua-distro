# The texts `ronoctua-shell-security` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# The view's keys, as the hint line names them (`security::OF_THE_VIEW`).
security-down = desce
security-up = sobe
security-enter = faz
security-left = dedo anterior
security-right = próximo dedo
security-relax = sem senha
security-shields = escudos
security-rule-add = nova regra
security-rule-delete = remove
security-enrol = cadastra
# THE SECURITY CENTRE (`LEADER s e`, `crate::security`): its leader group,
# its commands, its view's keys and questions, and its page.
group-security = segurança
cmd-security-open = Abrir a central de segurança
cmd-security-window = Abrir a central de segurança numa janela
cmd-security-relax = Deixar o administrador agir sem senha por um tempo, ou encerrar isso agora
cmd-security-shields = Recusar toda conexão de entrada, ou parar de recusar
cmd-security-firewall = Ligar ou desligar seu firewall
cmd-security-check-password = Ver se uma senha está em vazamentos conhecidos
cmd-security-check-email = Ver se um endereço de e-mail está em vazamentos conhecidos
cmd-security-check-file = Checar um arquivo no VirusTotal
cmd-security-down = Próxima linha da central de segurança
cmd-security-up = Linha anterior da central de segurança
cmd-security-enter = Fazer o que a linha sob o cursor oferece
cmd-security-left = O dedo anterior a cadastrar
cmd-security-right = O próximo dedo a cadastrar
cmd-security-rule-add = Acrescentar uma regra de firewall
cmd-security-rule-delete = Tirar a regra ou o dedo sob o cursor
cmd-security-enrol = Cadastrar o dedo escolhido
cmd-security-rule = Uma regra de firewall, em palavras
cmd-security-vt-key = Guardar sua chave do VirusTotal
cmd-security-vt-forget = Esquecer sua chave do VirusTotal
cmd-security-home = Tratar como de casa a rede que acabou de se conectar
ask-security-rule = Regra (permitir tcp 8080 de 192.168.1.0/24 em wlan0 casa)
# As mudanças do gerenciador de arquivos que só o administrador faz (dono,
# grupo, permissões), pela mesma porta do sudo.
ask-security-check-password = Senha a checar (só cinco caracteres da impressão dela saem desta máquina)
ask-security-check-email = Endereço de e-mail a checar (ele vai para o XposedOrNot)
ask-security-check-file = Arquivo a checar (a impressão dele vai para o VirusTotal)
echo-security-checking = Checando…
echo-security-file-checked = { $file }: { $verdict }
echo-security-finger-enrolled = Dedo cadastrado.
echo-security-finger-removed = Dedo removido.
echo-security-finger-sudo-changed = Os dedos para o administrador mudaram.
echo-security-firewall-applied = O firewall está como você pediu.
echo-security-key-forgotten = A chave do VirusTotal foi esquecida.
echo-security-key-kept = A chave do VirusTotal está guardada.
echo-security-key-not-a-key = Isso não é uma chave que o VirusTotal aceite.
echo-security-key-not-kept = A chave não pôde ser guardada na sua pasta.
echo-security-nothing-to-delete = Não há regra nem dedo sob o cursor.
echo-security-no-new-network = Nenhuma rede que se conectou espera ser classificada.
echo-security-relaxed = O administrador não pede senha até a hora que está na modeline.
echo-security-relax-ended = O administrador volta a pedir a senha.
echo-security-rule-not-understood = Isso não é uma regra: permitir ou negar, depois tcp ou udp, as portas e, se quiser, de, em e casa, pública ou tudo.
echo-security-shields-up = Toda conexão de entrada é recusada agora.
echo-security-upload-queued = { $file } está a caminho do VirusTotal; a lista de transferências mostra quanto falta.
security-title = Segurança
security-on = ligado
security-off = desligado
security-unknown = desconhecido
security-answer-no = Não
security-answer-yes = Sim
security-busy = Pedindo ao administrador…
security-busy-enrol = Toque o leitor quando ele pedir.
security-heading-now = Agora
security-heading-firewall = Firewall
security-heading-admin = Administrador
security-heading-finger = Digital
security-heading-device = Esta máquina
security-heading-privacy = Privacidade de rede
security-heading-online = Checagens online
security-heading-suggest = Sugestões
security-now-nothing = Nada fora do esperado.
security-now-reading = Lendo…
security-now-firewall-failed = O firewall falhou
security-now-settings-unreadable = As configurações do firewall não podem ser lidas
security-now-firewall-off = Seu firewall está desligado
security-now-sudo-open = Sem senha de administrador até
security-now-shields = Toda conexão de entrada é recusada
security-now-refused = { $count ->
    [one] Uma conexão recusada há pouco
   *[other] { $count } conexões recusadas há pouco
  }
security-fw-switch = Seu firewall
security-fw-network = Rede em { $iface }
security-fw-served = esta máquina a serve
security-fw-treat-as-home = Tratar esta rede como casa
security-fw-shields = Recusar toda conexão de entrada
security-fw-shields-note = Erguer é na hora; baixar pede sua senha, como todo afrouxamento.
security-fw-log = Conexões recusadas no registro
security-fw-rule-add = Acrescentar uma regra
security-fw-blocked = Recusadas há pouco:
security-fw-blocked-row = { $proto } { $port } de { $source } em { $iface } ({ $count ->
    [one] uma vez
   *[other] { $count } vezes
  })
security-fw-watch-silent = O cérebro não está respondendo, então as conexões recusadas há pouco não podem ser mostradas.
security-fw-ports = Onde esta máquina responde:
security-fw-port-row = { $who }: { $proto } { $port }
security-fw-port-unknown = um programa de outro usuário
security-fw-port-container = container { $name }
security-fw-verdict-open = aberta: o firewall está desligado
security-fw-verdict-allowed = permitida pela sua regra
security-fw-verdict-partly = permitida só em algumas redes, ou de alguns endereços
security-fw-verdict-refused = recusada, a não ser que o sistema a abra
security-fw-verdict-shielded = recusada: todas estão
security-sudo-password = A senha do administrador
security-sudo-asked = pedida toda vez
security-sudo-not-asked-until = não pedida até { $time }
security-sudo-end = Voltar a pedir agora
security-sudo-open-for = Não pedir por { $minutes } minutos
security-sudo-note = Pede sua senha antes, e se encerra sozinha na hora escolhida — reiniciar não a encerra — ou quando você a encerra.
security-sudo-recent = Nesta semana:
security-finger-reader = Leitor
security-finger-enrolled = cadastrado
security-finger-enrol = Cadastrar
security-finger-ring-note = As setas para os lados escolhem o dedo; pede sua senha antes, depois alguns toques.
security-finger-sudo = Dedos para o administrador
security-finger-sudo-note = Num terminal e na barra do sudo um dedo responde no lugar da sua senha, que sempre responde também; nunca no login.
security-finger-ask-remove = Remover seu { $finger }? Ele não poderá ser usado até ser cadastrado de novo.
security-device-secure-boot = Secure Boot
security-device-tpm = Chip de segurança (TPM)
security-device-encryption = O disco do sistema
security-device-encrypted = cifrado
security-device-not-encrypted = não cifrado
security-device-hsi = Nível de segurança do firmware
security-device-firmware-updates = Atualizações de firmware
security-device-firmware-updates-count = { $count ->
    [0] nenhuma conhecida
    [one] uma disponível
   *[other] { $count } disponíveis
  }
security-device-firmware-note = Só leitura: firmware não é instalado daqui.
security-device-updates = Atualizações automáticas
security-device-lock = Bloqueio de tela
security-device-lock-none = nenhum: este sistema não tem tela de bloqueio
security-device-autologin = A partida da máquina
security-device-autologin-gate = a senha do disco é a única porta
security-privacy-mac = Endereço de hardware em { $name }
security-privacy-dns = DNS cifrado em { $name }
security-privacy-none = Nenhuma rede está ativa.
security-privacy-ssh = Servidor SSH
security-privacy-ssh-on = algo responde na porta 22
security-privacy-note = Isto é lido, não mudado aqui: as configurações de cada conexão são da rede.
security-online-key-kept = guardada
security-online-key-none = nenhuma: cole uma da sua conta do VirusTotal
security-online-vt-forget = Esquecer a chave do VirusTotal
security-online-check-file = Checar um arquivo
security-online-file-note = Só a impressão dele (SHA-256) vai para o VirusTotal. Um arquivo que ele nunca viu só é enviado se você disser sim.
security-online-check-email = Checar um endereço de e-mail
security-online-email-note = O próprio endereço vai para o XposedOrNot.
security-online-email-clean = { $address } não está em nenhum vazamento conhecido.
security-online-email-found = { $address } está em { $count ->
    [one] um vazamento
   *[other] { $count } vazamentos
  }: { $names }
security-online-check-password = Checar uma senha
security-online-password-note = Só cinco caracteres da impressão dela (SHA-1) saem desta máquina; o resto é comparado aqui.
security-online-password-clean = Essa senha não está em nenhum vazamento conhecido.
security-online-password-found = Essa senha foi encontrada { $count ->
    [one] uma vez
   *[other] { $count } vezes
  } em vazamentos: não a use.
security-vt-ask-upload = O VirusTotal nunca viu { $file }. Enviar o próprio arquivo?
security-vt-upload-warning = Um arquivo enviado ao VirusTotal é compartilhado com os parceiros dele e pode ser baixado pelos assinantes pagos. Não envie nada privado.
security-rule-word-from = de
security-rule-word-on = em
security-rule-word-home = casa
security-rule-word-public = pública
security-rule-word-everywhere = tudo
security-rule-word-any = qualquer
security-about-show = Vai até onde isto está
security-about-profile = Casa ou pública: uma rede pública não deixa entrar o que suas regras de casa deixariam
security-about-log = Quantas conexões recusadas vão para o registro
security-about-rule = Mudar esta regra, em palavras; a tecla de apagar a tira
security-about-allow-home = Permite esta porta nas redes de casa, com sua senha
security-about-allow-public = Permite esta porta também nas redes públicas, com sua senha
security-about-finger = Remove este dedo, depois de uma pergunta
security-about-finger-sudo = Deixa um dedo responder ao administrador — num terminal e na barra do sudo — no lugar da sua senha
security-about-report = Abre o relatório do arquivo na página do VirusTotal
security-about-url = Abre a página no seu navegador
security-about-answer = Responde à pergunta
security-suggest-vpn = Usar uma VPN: importe o arquivo dela na vista de rede
security-suggest-tor = Passar pelo Tor
security-suggest-tailnet = Chegar às suas máquinas pela sua tailnet
security-suggest-hibp = Have I Been Pwned, no seu navegador (nenhum endereço vai junto)
# The passwordless window's longer spans, and the one with no limit.
security-sudo-open-for-hours = Não pedir por { $hours } horas
security-sudo-open-no-limit = Não pedir até eu mandar
security-sudo-not-asked-no-limit = não pedida até você encerrar
security-sudo-until-you-end-it = você encerrar
cmd-private-passphrase = A senha do /private
cmd-private-new-passphrase = Uma senha nova para o /private
cmd-private-repeat-passphrase = A mesma senha outra vez
cmd-private-cap = Até quanto o /private pode crescer (64G, 1,5T…)
security-heading-private = /private
security-private-about = Uma pasta que só você abre: cifrada com uma senha só dela, ela ocupa no disco só o que guarda e se tranca sozinha quando a máquina dorme.
security-private-missing = Esta máquina não tem as unidades do cofre, então aqui não existe /private.
security-private-unreadable = Não deu para ler o que o /private está fazendo.
security-private-create = Criar o /private
security-private-create-value = até { $size }
security-private-state = Estado
security-private-locked = trancado
security-private-open-at = aberto, em /private
security-private-suspended = suspenso em uso: a chave saiu da memória ao dormir
security-private-open = Abrir com a senha
security-private-open-key = Abrir com a chave de segurança
security-private-resume = Retomar com a senha
security-private-lock = Trancar agora
security-private-show = Mostrar no gerenciador de arquivos
security-private-size = No disco
security-private-size-value = { $real } de até { $cap }
security-private-grow = Aumentar o teto
security-private-cipher = Cifra
security-private-cipher-value = LUKS2, AES-XTS com chave de 512 bits, argon2id
security-private-key = Chave de segurança
security-private-key-enrolled = cadastrada: um toque abre o /private
security-private-enrol = Cadastrar uma chave de segurança
security-private-forget = Esquecer a chave de segurança
security-private-passphrase = Trocar a senha
security-private-note = Ele se tranca sozinho quando a máquina dorme. Se algo ainda o estiver usando, ele é suspenso: a chave sai da memória, e a senha o retoma.
security-private-overcommit = O teto é maior que o espaço livre neste disco: se o disco encher, gravar no /private falha.
security-private-busy = Trabalhando no /private…
security-private-touch = Toque agora na sua chave de segurança.
echo-private-not-a-size = Isso não é um tamanho: escreva como 64G ou 1,5T.
echo-private-too-small = O /private precisa de um teto de pelo menos { $size }.
echo-private-mismatch = As duas senhas são diferentes, então nada mudou.
echo-private-already-locked = O /private já está trancado.
security-now-private-suspended = O /private está suspenso em uso

## AS CHAVES DA CENTRAL DE SEGURANÇA, e a contagem da janela sem senha.

security-heading-keys = Chaves de segurança
security-keys-sudo = Respondem ao administrador
security-keys-on = sim, { $count ->
        [one] uma chave
       *[other] { $count } chaves
    }
security-keys-off = não
security-keys-off-listed = não; a sua lista tem { $count ->
        [one] uma chave
       *[other] { $count } chaves
    }
security-keys-key = Chave { $n }
security-keys-bring = Trazer as chaves das suas outras máquinas
security-keys-away = { $count ->
        [one] uma chave
       *[other] { $count } chaves
    }
security-keys-register = Cadastrar uma chave
security-keys-touch-now = Toque na chave que você está cadastrando.
security-keys-stop = Parar o cadastro
security-keys-full = No máximo { $count } chaves respondem ao administrador.
security-keys-note = Uma chave responde ao administrador no lugar da sua senha — num terminal e na barra do sudo —, nunca no login; a sua senha sempre responde também. Desligar as tira desta máquina e as guarda na sua lista, que viaja com os pertences.
security-keys-not-here = As chaves de segurança não respondem ao administrador nesta máquina (ronoctua.yubikey.enable está desligado).
security-keys-ask-remove = Remover a chave { $key }?
security-keys-remove-warning = Ela para de responder ao administrador aqui e sai da sua lista, então também não responde em nenhuma outra máquina.
security-about-key = Remove esta chave, depois de perguntar
security-about-key-register = Espera um toque da chave e a deixa responder ao administrador — aqui, e onde os pertences a levarem
security-about-key-stop = Para de esperar o toque
security-about-keys-switch = Suas chaves respondem ao administrador aqui, ou param — na hora, sem rebuild
security-about-keys-bring = Deixa as chaves cadastradas nas suas outras máquinas responderem aqui também
security-about-relax-again = Abre a janela de novo pelo mesmo tempo, a partir de agora
security-sudo-left = Tempo restante
security-sudo-again = Recomeçar a contagem
echo-security-relax-again = A contagem recomeçou.
echo-security-keys-registered = A chave foi cadastrada: ela responde ao administrador agora.
echo-security-keys-removed = A chave foi removida.
echo-security-keys-off = Suas chaves de segurança não respondem mais ao administrador aqui.
echo-security-keys-on = Suas chaves de segurança voltaram a responder ao administrador aqui.
echo-security-keys-brought = As chaves das suas outras máquinas respondem ao administrador aqui agora.
cmd-security-encrypt = Cifrar um arquivo ou uma pasta: para você, uma senha ou a chave de alguém
cmd-security-quantum = A seção pós-quântica da central de segurança: o que cada canal faz contra um computador quântico
security-heading-encrypt = Cifrar
security-encrypt-for-you = Para você
security-encrypt-file = Cifrar arquivos…
security-encrypt-folder = Cifrar uma pasta…
security-encrypt-open = Decifrar um arquivo…
security-encrypt-note = Cada um vai para a fila de transferências. Nada é sobrescrito e o original fica; uma pasta vira nome.tar.age. Para você e para a chave de alguém é ML-KEM-768 com X25519; com senha é scrypt.
security-encrypt-pick-files = O que cifrar
security-encrypt-pick-folder = A pasta a cifrar
security-encrypt-pick-open = O .age a decifrar
security-about-seal-folder = Escolha uma pasta no seletor de arquivos; ela é cifrada como um único .tar.age
security-about-seal-open = Escolha um .age no seletor de arquivos; o cabeçalho dele diz o que o abre
security-about-seal-mode = Cifra o que foi escolhido deste jeito, na fila de transferências
security-heading-quantum = Pós-quântico
security-quantum-note = Uma gravação feita hoje pode ser lida no dia em que um computador quântico quebrar a chave dela, e uma assinatura, então, pode ser forjada. Cada canal diz onde está.
security-quantum-status-post-quantum = pós-quântico
security-quantum-status-symmetric = simétrico
security-quantum-status-classical = clássico
security-quantum-status-unknown = não se sabe
security-quantum-channel-belongings = Pertences
security-quantum-channel-belongings-transport = Transporte dos pertences
security-quantum-channel-seal-to-key = Cifrar para uma chave
security-quantum-channel-seal-passphrase = Cifrar com senha
security-quantum-channel-private = Cofre privado
security-quantum-channel-root-disk = Disco do sistema
security-quantum-channel-https = Nosso HTTPS
security-quantum-channel-openssl = OpenSSL
security-quantum-channel-ssh = SSH
security-quantum-channel-chrome = Chrome
security-quantum-channel-syncthing = Syncthing
security-quantum-channel-tailscale = Tailscale
security-quantum-channel-tor = Tor
security-quantum-channel-clipcascade = ClipCascade
security-quantum-channel-sops = Segredos da máquina
security-quantum-channel-extension = Extensão do navegador
security-quantum-channel-f-droid = F-Droid
security-quantum-channel-nix-cache = Cache binário
security-quantum-channel-secure-boot = Chaves do Secure Boot
security-quantum-why-belongings-pq = Cifrados para a sua própria chave: ML-KEM-768 com X25519, e as duas precisam cair.
security-quantum-why-belongings-unsynced = Ainda não sincronizados nesta máquina: o repositório é cifrado com ML-KEM-768 e X25519, e a primeira sincronização também.
security-quantum-why-belongings-new = Nenhum configurado ainda; um repositório configurado agora é cifrado com ML-KEM-768 e X25519.
security-quantum-why-belongings-transport = SSH e HTTPS só com troca de chaves pós-quântica: um servidor que não tem nenhuma é recusado.
security-quantum-why-seal-to-key = O mlkem768x25519 do age: para você, ou para a chave age1pq1 de alguém.
security-quantum-why-seal-passphrase = scrypt e ChaCha20-Poly1305: um computador quântico reduz pela metade a força de uma chave, e uma senha longa continua de pé.
security-quantum-why-private = LUKS2 com AES-XTS e chave de 512 bits, a senha esticada pelo argon2id.
security-quantum-why-root-disk = O mesmo para o disco do sistema: LUKS2, AES-XTS-512, argon2id.
security-quantum-why-root-disk-unread = Não foi possível ler se o disco do sistema está cifrado.
security-quantum-why-https-all = { $hosts ->
        [one] O único host ouvido combinou X25519MLKEM768.
       *[other] Todos os { $hosts } hosts ouvidos combinaram X25519MLKEM768.
    }
security-quantum-why-https-some = { $hosts ->
        [one] O único host ouvido ainda combina uma chave clássica: { $names }.
       *[other] { $classical ->
            [one] Um dos { $hosts } hosts ouvidos ainda combina uma chave clássica: { $names }.
           *[other] { $classical } dos { $hosts } hosts ouvidos ainda combinam uma chave clássica: { $names }.
        }
    }
security-quantum-why-https-none-heard = Nenhum host ouvido ainda: o primeiro pedido de cada um diz o que ele combina.
security-quantum-why-openssl = OpenSSL { $version } oferece X25519MLKEM768 primeiro: o git por HTTPS, o curl e o que mais o usa.
security-quantum-why-openssl-old = OpenSSL { $version } não tem grupo pós-quântico por padrão.
security-quantum-why-ssh = O OpenSSH { $version } oferece { $kex } primeiro, uma troca de chaves pós-quântica; um servidor sem nenhuma combina uma chave clássica.
security-quantum-why-ssh-classical = O OpenSSH { $version }, como está configurado, não oferece troca de chaves pós-quântica (KexAlgorithms).
security-quantum-why-ssh-not-first = O OpenSSH { $version }, como está configurado, oferece { $kex } primeiro e uma troca pós-quântica só depois dela: um servidor que conhece as duas combina a clássica (KexAlgorithms).
security-quantum-why-ssh-unread = O ssh não disse o que oferece.
security-quantum-why-chrome = Chrome { $version } combina X25519MLKEM768 por padrão.
security-quantum-why-chrome-old = Chrome { $version } combina uma chave clássica por padrão.
security-quantum-why-syncthing = Compilado com Go { $go }, que oferece X25519MLKEM768, e o Syncthing não o restringe; um aparelho com uma versão mais velha combina X25519.
security-quantum-why-syncthing-pinned = Syncthing { $version } restringe a troca de chaves a curvas clássicas.
security-quantum-why-syncthing-old-go = Syncthing { $version } é compilado com ou para um Go anterior ao 1.24: só X25519.
security-quantum-why-tailscale = O WireGuard combina as chaves com X25519, e o Tailscale não tem modo pós-quântico.
security-quantum-why-tor = Os circuitos do Tor são combinados com X25519; o Tor ainda não tem handshake pós-quântico.
security-quantum-why-clipcascade = AES-256-GCM com uma chave da sua senha, levado pelo nosso TLS.
security-quantum-why-sops = Os segredos da máquina são cifrados para uma chave X25519 feita da chave SSH do host.
security-quantum-why-extension = A extensão do navegador é assinada com RSA-2048, e o formato dela não tem outra assinatura: uma falsificação precisa de um computador quântico no momento do ataque.
security-quantum-why-f-droid = O F-Droid assina o índice com RSA: o que um computador quântico faria dele é uma falsificação, não uma leitura.
security-quantum-why-nix-cache = As assinaturas do cache binário são ed25519: uma falsificação precisa de um computador quântico no momento do ataque.
security-quantum-why-secure-boot = As chaves do Secure Boot são RSA-2048: o firmware não verifica outra coisa.
security-quantum-why-not-built = Esta compilação não disse qual versão entrega.
