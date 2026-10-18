# The texts `ronoctua-shell-incus` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

incus-refused-no-incus = O Incus não faz parte desta instalação, ou o daemon dele não está rodando.
incus-refused-not-in-group = O Incus só responde ao grupo incus-admin; saia da sessão e entre de novo para o grupo valer.
incus-refused-gone = Não está mais lá.
incus-refused-busy = O Incus ainda está ocupado com isso; tente de novo quando terminar.
incus-refused-not-let = Esse servidor, ou um host de onde ele baixa, ainda não foi permitido; permita-os primeiro.
incus-refused-invalid = O Incus não aceita esse nome, caminho ou limite — ou o nome já está em uso, ou o arquivo já existe.
incus-refused-failed = O Incus não conseguiu; o log do sistema diz por quê.
# O INCUS (LEADER s x) — `incus.rs`, as palavras dele (`incus_words.rs`), as
# threads e a janela. Nome, alias, endereço, a descrição de uma imagem e o
# nome de um snapshot são palavras do próprio daemon e são desenhados como
# vieram; tecla nunca é texto aqui: uma sequência chega como `{ $keys }`.
group-incus = Incus
incus-title = Incus
cmd-incus-open = Incus: seus containers e máquinas virtuais
cmd-incus-window = Incus, numa janela
cmd-incus-new = Um novo container ou máquina virtual do Incus
cmd-incus-images = As imagens do Incus, aqui e nos catálogos
incus-down = descer
incus-up = subir
incus-enter = abrir
incus-left = esquerda
incus-right = direita
incus-next-kind = próximo tipo
incus-previous-kind = tipo anterior
incus-next-tab = próxima aba
incus-previous-tab = aba anterior
incus-launch = nova
incus-start-stop = ligar/desligar
incus-restart = reiniciar
incus-freeze = congelar/descongelar
incus-delete = apagar
incus-console = console
incus-screen = tela
incus-shell = shell
incus-push = enviar
incus-pull = trazer
incus-export = exportar
incus-import = importar
incus-copy-image = guardar uma cópia
cmd-incus-down = A próxima linha
cmd-incus-up = A linha de cima
cmd-incus-enter = Abre a linha; num fato, copia
cmd-incus-left = A coluna da esquerda, ou volta do detalhe
cmd-incus-right = A coluna da direita, ou o detalhe da linha
cmd-incus-next-kind = O próximo tipo: instâncias, imagens, remotos, perfis, redes, armazenamento, projetos
cmd-incus-previous-kind = O tipo anterior
cmd-incus-next-tab = A próxima aba da instância
cmd-incus-previous-tab = A aba anterior da instância
cmd-incus-launch = Uma instância nova: tipo, nome, imagem e limites, depois lançada pela fila
cmd-incus-start-stop = Liga, ou desliga
cmd-incus-restart = Reinicia
cmd-incus-freeze = Congela, ou descongela
cmd-incus-delete = Apaga — uma instância, uma imagem ou um snapshot — depois de uma pergunta
cmd-incus-console = O console de texto dela, no terminal
cmd-incus-screen = A tela de uma máquina virtual, no remote-viewer
cmd-incus-shell = Um shell dentro dela, no terminal
cmd-incus-push = Envia um arquivo para dentro dela, pela fila
ask-incus-push = Arquivo a enviar
cmd-incus-push-to = Onde, lá dentro, fica o arquivo enviado
ask-incus-push-to = Onde, lá dentro
cmd-incus-pull = Traz um arquivo de dentro dela, pela fila
ask-incus-pull = Caminho lá dentro a trazer
cmd-incus-pull-to = Onde, aqui, fica o arquivo trazido, nunca por cima de outro
ask-incus-pull-to = Gravar como
cmd-incus-export = Exporta para um arquivo, pela fila
ask-incus-export = Exportar para um arquivo que ainda não existe
cmd-incus-import = Importa uma instância de um arquivo, pela fila
ask-incus-import = Arquivo a importar
cmd-incus-copy-image = Guarda aqui uma cópia da imagem do catálogo, pela fila
cmd-incus-name = O nome da instância nova
ask-incus-name = Nome da instância nova
cmd-incus-image = A imagem da instância nova
ask-incus-image = Imagem (remoto:alias, ou o alias de uma daqui)
cmd-incus-cpu = Os processadores dela
ask-incus-cpu = Processadores (nada digitado: sem limite)
cmd-incus-memory = A memória dela
ask-incus-memory = Memória, com a unidade (nada digitado: sem limite)
cmd-incus-processes = Os processos dela
ask-incus-processes = Processos (nada digitado: sem limite)
cmd-incus-snapshot = Um snapshot dela, com um nome
ask-incus-snapshot = Nome do snapshot
cmd-incus-publish = Publica como uma imagem aqui, pela fila
ask-incus-publish = Alias da imagem nova
incus-kind-instances = Instâncias
incus-kind-images = Imagens
incus-kind-remotes = Remotos
incus-kind-profiles = Perfis
incus-kind-networks = Redes
incus-kind-storage = Armazenamento
incus-kind-projects = Projetos
incus-empty-instances = Nenhuma instância ainda: uma nova nasce de uma imagem, daqui ou do catálogo de um remoto.
incus-empty-images = Nenhuma imagem aqui. O catálogo de um remoto guarda a cópia de uma, e uma instância lançada guarda a que usou.
incus-empty-remotes = Nenhum remoto: o cliente não conhece servidor de onde trazer imagens.
incus-empty-profiles = Nenhum perfil.
incus-empty-networks = Nenhuma rede.
incus-empty-storage = Nenhum pool de armazenamento.
incus-empty-projects = Nenhum projeto.
incus-projects-note = Esta vista trabalha no projeto padrão; os outros, o comando incus alcança.
incus-tab-overview = Visão geral
incus-tab-snapshots = Snapshots
incus-tab-limits = Limites
incus-tab-actions = Ações
incus-status-running = ligada
incus-status-stopped = desligada
incus-status-frozen = congelada
incus-status-error = com erro
incus-status-other = mudando
incus-machine-container = container
incus-machine-vm = máquina virtual
incus-list-address = { $status } · { $address }
incus-reading = Perguntando ao Incus…
incus-events-lost = Os eventos do Incus pararam (o daemon reiniciou?), então uma mudança feita em outro lugar pode ainda não aparecer; eles são reabertos sozinhos.
incus-working = { $what } { $name }: { $stage }
incus-row-name = Nome
incus-row-kind = Tipo
incus-row-status = Estado
incus-row-ipv4 = Endereço IPv4
incus-row-ipv6 = Endereço IPv6
incus-row-cpu = Tempo de processador
incus-row-memory = Memória em uso
incus-row-processes = Processos rodando
incus-row-disk = Disco em uso
incus-row-image = Feita de
incus-row-profiles = Perfis
incus-row-made = Criada
incus-row-project = Projeto
incus-row-description = Descrição
incus-row-alias = Alias
incus-row-size = Tamanho
incus-row-fingerprint = Impressão digital
incus-row-address = Endereço
incus-row-protocol = Fala
incus-row-type = Tipo de rede
incus-row-managed = Gerida pelo Incus
incus-row-driver = Driver
incus-row-nth = { $label } { $n }
incus-yes = sim
incus-no = não
incus-seconds = { $count ->
        [one] 1 segundo
       *[other] { $count } segundos
    }
incus-minutes = { $count ->
        [one] 1 minuto
       *[other] { $count } minutos
    }
incus-hours = { $hours } h { $minutes } min
incus-snapshot-take = Tirar um snapshot
incus-snapshot-take-value = com um nome
incus-snapshots-none = Nenhum snapshot ainda.
incus-limit-cpu = Processadores
incus-limit-memory = Memória
incus-limit-processes = Processos
incus-limit-none = sem limite
incus-limit-from-profile = { $value }, de um perfil
incus-limit-cpus = { $count ->
        [one] 1 processador
       *[other] { $count } processadores
    }
incus-limit-count = { $count ->
        [one] 1 processo
       *[other] { $count } processos
    }
incus-limits-note = Um limite posto aqui é da própria instância e vale sobre o dos perfis; responder sem digitar nada tira o da própria instância. Um limite que vem de um perfil se muda no perfil.
incus-limits-vm-processes = Uma máquina virtual não tem limite de processos: o próprio kernel dela os conta.
incus-use-console = Console de texto
incus-use-screen = Tela
incus-use-shell = Shell
incus-use-push = Enviar um arquivo
incus-use-pull = Trazer um arquivo
incus-use-export = Exportar para um arquivo
incus-use-publish = Publicar como imagem
incus-use-in-terminal = no terminal
incus-use-in-viewer = no remote-viewer
incus-use-on-queue = pela fila
incus-use-not-running = só enquanto roda
incus-image-launch = Lançar uma instância dela
incus-image-launch-value = uma instância nova
incus-heading-catalogue = As imagens dele
incus-catalogue-reading = Perguntando a { $remote } pelas imagens…
incus-catalogue-empty = Ele não oferece nada para esta máquina.
incus-offered-value = { $kind } · { $size }
incus-consent-catalogue = A lista de imagens do remoto vem de lá, e as imagens de onde uma instância nasce também.
incus-consent-launch = A imagem de onde a instância nova nasce vem de lá.
incus-protocol-simplestreams = simplestreams
incus-protocol-oci = um registro OCI
incus-heading-volumes = Os volumes dele
incus-volumes-none = Nenhum volume.
incus-new-title = Uma instância nova
incus-field-machine = Tipo
incus-field-name = Nome
incus-field-image = Imagem
incus-unchosen = ainda não escolhido
incus-image-hint = A imagem é remoto:alias — os remotos daqui são { $remotes } — ou o alias de uma imagem daqui.
incus-create = Lançar
incus-create-value = criada e ligada pela fila
incus-ask-delete = Apagar { $name }?
incus-ask-delete-running = Ela está ligada: é desligada antes.
incus-ask-delete-snapshots = Os snapshots dela vão junto.
incus-ask-delete-image = Apagar a imagem { $name }?
incus-ask-delete-snapshot = Apagar o snapshot { $snapshot } de { $name }?
incus-ask-restore = Restaurar { $name } para { $snapshot }? O que mudou nela desde então se perde.
incus-answer-no = Não
incus-answer-yes = Sim
incus-did-start = Ligando
incus-did-stop = Desligando
incus-did-restart = Reiniciando
incus-did-freeze = Congelando
incus-did-unfreeze = Descongelando
incus-did-delete = Apagando
incus-did-limits = Pondo os limites de
incus-did-snapshot = Tirando o snapshot
incus-did-restore = Restaurando
incus-did-delete-snapshot = Apagando o snapshot
incus-did-delete-image = Apagando a imagem
incus-echo-asked = { $what } { $name }…
incus-echo-done = { $what } { $name }: feito.
incus-echo-refused = { $what } { $name }: { $why }
incus-echo-queued = { $what } { $name } foi para a fila — { $keys } a mostra.
incus-no-queue = A fila de transferências não está respondendo, então nada foi para ela.
incus-refused-not-here = Isso não é algo que se faça com { $kind }.
incus-refused-not-running = Ela não está ligada.
incus-refused-not-vm = Só uma máquina virtual tem tela.
incus-refused-unchosen = Escolha um nome e uma imagem antes.
incus-refused-name = { $name } não é um nome que o Incus aceite: letras, dígitos e hífens, começando por uma letra.
incus-refused-name-taken = Já existe uma instância chamada { $name }.
incus-refused-no-image = Nenhuma imagem remota e nenhuma daqui se chama { $image }.
incus-refused-number = { $said } não é um número inteiro maior que zero.
incus-refused-size = { $said } não é um tamanho: diga a unidade, como em 512MiB ou 2GiB.
incus-refused-snapshot-name = { $name } não é um nome de snapshot que o Incus aceite: sem espaços e sem barras.
incus-refused-no-file = { $path } não é um arquivo daqui.
incus-refused-inside = { $path } não é um caminho lá dentro: um começa por barra.
incus-refused-exists = { $path } já existe; nada é gravado por cima.
incus-refused-no-folder = A pasta de { $path } não existe.
incus-refused-alias = { $alias } não é um alias que o Incus aceite: sem espaços.
incus-refused-limit-profile = { $limit }: esse limite de { $name } vem de um perfil, e { $name } não tem um próprio para tirar.
