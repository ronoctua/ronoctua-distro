# The texts `ronoctua-shell-containers` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# THE PERSON'S CONTAINERS (LEADER s o) — `containers.rs`, its words
# (`containers_words.rs`), its threads and its window. A status line, a name
# and a log line are podman's own words and are drawn as they came; a key is
# never text here: every sequence arrives as `{ $keys }`.
containers-title = Containers
group-containers = containers
cmd-containers-open = Seus containers: pods, compose, imagens, volumes e redes
cmd-containers-window = Seus containers, numa janela
containers-down = descer
containers-up = subir
containers-enter = abrir
containers-left = esquerda
containers-right = direita
containers-next-kind = próximo tipo
containers-previous-kind = tipo anterior
containers-next-tab = próxima aba
containers-previous-tab = aba anterior
containers-start-stop = iniciar/parar
containers-restart = reiniciar
containers-remove = remover
containers-logs = log
containers-shell = shell
containers-inspect = inspecionar
containers-pull = baixar
containers-build = construir
containers-login = entrar
containers-prune = limpar
containers-reveal = mostrar valores
cmd-containers-down = A próxima linha
cmd-containers-up = A linha de cima
cmd-containers-enter = Abrir a linha; num fato, copiá-lo
cmd-containers-left = A coluna à esquerda, ou voltar do detalhe
cmd-containers-right = A coluna à direita, ou o detalhe da linha
cmd-containers-next-kind = O próximo tipo: containers, pods, compose, imagens, volumes, redes
cmd-containers-previous-kind = O tipo anterior
cmd-containers-next-tab = A próxima aba do detalhe
cmd-containers-previous-tab = A aba anterior do detalhe
cmd-containers-start-stop = Iniciar ou parar; um projeto compose sobe, ou desce depois de uma pergunta
cmd-containers-restart = Reiniciar
cmd-containers-remove = Remover, depois de uma pergunta
cmd-containers-logs = O log, enquanto cresce
cmd-containers-shell = Um shell dentro dele, no terminal
cmd-containers-inspect = Tudo o que o podman sabe dele, realçado
cmd-containers-pull = Baixar uma imagem, pela fila
ask-containers-pull = Imagem a baixar
cmd-containers-build = Construir uma imagem a partir de uma pasta, pela fila
ask-containers-build = Pasta a construir
cmd-containers-login = Entrar num registro: o nome dele e o seu usuário, depois a senha numa linha oculta
ask-containers-login = Registro e usuário (quay.io alice)
cmd-containers-login-password = A senha do login no registro que está sendo digitado, numa linha oculta
ask-containers-login-password = A senha do registro
cmd-containers-prune = Remover o que nada usa, depois de uma pergunta
cmd-containers-reveal = Mostrar os valores do ambiente, ou escondê-los de novo
containers-kind-containers = Containers
containers-kind-pods = Pods
containers-kind-compose = Compose
containers-kind-images = Imagens
containers-kind-volumes = Volumes
containers-kind-networks = Redes
containers-empty-containers = Nenhum container. Baixar põe uma imagem aqui, e o `podman run` ou o compose faz um.
containers-empty-pods = Nenhum pod.
containers-empty-compose = Nenhum projeto compose. Um aparece quando o compose faz os containers dele.
containers-empty-images = Nenhuma imagem.
containers-empty-volumes = Nenhum volume.
containers-empty-networks = Nenhuma rede.
containers-tab-overview = Visão geral
containers-tab-logs = Log
containers-tab-stats = Uso
containers-tab-inspect = Inspeção
containers-tab-ports = Portas
containers-tab-mounts = Montagens
containers-tab-env = Ambiente
containers-state-created = criado
containers-state-running = rodando
containers-state-paused = pausado
containers-state-exited = parado
containers-state-stopping = parando
containers-state-unknown = num estado que o podman não nomeou
containers-why-no-podman = Containers não fazem parte desta instalação: o podman não está aqui.
containers-why-unstartable = Não foi possível iniciar o podman.
containers-why-failed = O podman não conseguiu; o log do sistema diz por quê.
containers-why-garbled = O podman respondeu algo que não é o que foi pedido.
containers-why-not-found = Não está mais lá.
containers-why-refused = O registro recusou o login.
containers-why-too-large = A resposta do podman é grande demais para mostrar.
containers-reading = Perguntando ao podman…
containers-working = { $what } { $name }: { $stage }
containers-pod-value = { $status } · { $count }
containers-project-value = { $count ->
        [one] { $up } de 1 rodando
       *[other] { $up } de { $count } rodando
    }
containers-image-unnamed = sem nome ({ $id })
containers-heading-members = Os containers dele
containers-row-name = Nome
containers-row-image = Imagem
containers-row-state = Estado
containers-row-status = O podman diz
containers-row-created = Feito em
containers-row-started = Iniciado em
containers-row-finished = Terminou em
containers-row-exit = Terminou com
containers-row-command = Roda
containers-row-restart = Reinício
containers-row-restarts = Reiniciado
containers-row-network = Rede
containers-row-pod = Pod
containers-row-project = Projeto compose
containers-row-service = Serviço
containers-row-id = Id
containers-row-folder = Pasta
containers-row-file = Arquivo compose
containers-row-size = Tamanho
containers-row-used-by = Usada por
containers-row-platform = Feita para
containers-row-driver = Driver
containers-row-mountpoint = Guardado em
containers-row-subnet = Endereços
containers-row-dns = Nomes dentro dela
containers-used-by = { $count ->
        [0] nenhum container
        [one] um container
       *[other] { $count } containers
    }
containers-network-value = { $network }: { $address }
containers-on = ligado
containers-off = desligado
containers-open = abrir
containers-logs-waiting = Esperando o log…
containers-logs-empty = Ele não escreveu nada.
containers-logs-ended = O log termina aqui: o podman parou de acompanhá-lo.
containers-stats-waiting = Esperando a primeira contagem do podman…
containers-stats-not-running = Só um container rodando usa a máquina.
containers-stats-every = Contado de novo a cada { $seconds } segundos, enquanto isto aparece.
containers-stat-cpu = Processador
containers-stat-memory = Memória
containers-stat-memory-share = Do limite de memória dele
containers-stat-network = Rede entrando / saindo
containers-stat-disk = Disco lendo / escrevendo
containers-stat-pids = Processos
containers-ports-none = Ele não publica porta nenhuma: nada fora dele o alcança.
containers-port-label = { $published } para { $inside }
containers-port-local = só esta máquina
containers-port-open = aberta: o firewall está desligado
containers-port-allowed = liberada
containers-port-home-only = liberada só em casa
containers-port-shields = recusada: escudos erguidos
containers-port-refused = recusada pelo firewall
containers-port-unknown = regras ilegíveis
containers-port-firewall = As regras do firewall
containers-ports-blocked = Quem está em outra máquina não alcança uma porta que o firewall recusa; só esta máquina alcança. Uma regra sua a libera.
containers-ports-unknown = Não foi possível ler as regras do firewall, então não se sabe o que alcança estas portas.
containers-port-narrowed = liberada só para alguns endereços
containers-ports-shields = Os escudos estão erguidos: o firewall descarta toda conexão nova vinda de outra máquina, digam o que disserem as regras, até que sejam baixados.
containers-ports-narrowed = Uma regra sua libera só os endereços ou a conexão que ela nomeia; quem mais tentar não alcança estas portas.
containers-ports-open = O firewall está desligado: qualquer um numa rede em que esta máquina está alcança estas portas.
containers-inspect-env = Valores do ambiente
containers-row-nth = { $label } { $n }
containers-mounts-none = Ele não monta nada desta máquina.
containers-mount-value = { $source } · { $how } · { $kind }
containers-mount-writable = pode escrever
containers-mount-read-only = só leitura
containers-env-none = O ambiente dele está vazio.
containers-env-values = Valores
containers-env-hidden = escondidos
containers-env-shown = à mostra
containers-env-mask = ••••••
containers-ask-remove = Remover { $name }?
containers-ask-remove-running = Ele está rodando: é parado antes.
containers-ask-remove-pod = Os containers dele são removidos junto.
containers-ask-remove-used = Os containers feitos a partir dela são removidos junto.
containers-ask-prune-system = Remover todo container e todo pod parados, toda rede que nenhum container usa, toda imagem sem nome e o cache de construção?
containers-ask-prune-images = Remover toda imagem sem nome?
containers-ask-prune-volumes = Remover todo volume que nenhum container usa — e o que há dentro deles?
containers-ask-down = Derrubar { $name }? Os containers dele são parados e removidos; os volumes ficam.
containers-answer-no = Não
containers-answer-yes = Sim
containers-did-start = Iniciando
containers-did-stop = Parando
containers-did-restart = Reiniciando
containers-did-pause = Pausando
containers-did-unpause = Retomando
containers-did-remove = Removendo
containers-did-prune = Limpando
containers-echo-asked = { $what } { $name }…
containers-echo-done = { $what } { $name }: pronto.
containers-echo-refused = { $what } { $name }: { $why }
containers-echo-queued = { $what } { $name } foi para a fila — { $keys } a mostra.
containers-no-queue = A fila de transferências não responde, então nada foi para a fila.
containers-refused-not-here = Isso não é algo que se faça com { $kind }.
containers-refused-not-running = Só um container rodando tem um shell para abrir.
containers-refused-no-file = O compose não disse onde está o arquivo deste projeto.
containers-refused-no-folder = { $path } não é uma pasta.
containers-refused-no-containerfile = { $path } não tem um Containerfile nem um Dockerfile para construir.
containers-refused-login-words = Digite o registro e o seu usuário nele, com um espaço entre eles: quay.io alice.
containers-refused-no-login = Nenhum login espera uma senha: comece pelo registro e pelo usuário.
containers-echo-logging-in = Entrando em { $registry } como { $user }…
containers-echo-logged-in = Entrou em { $registry } como { $user }.
containers-echo-login-refused = O login em { $registry } não deu certo: { $why }
transfers-work-container-pull = baixar a imagem
transfers-work-container-build = construir a imagem
transfers-work-container-compose-up = subir o projeto
transfers-work-container-compose-down = derrubar o projeto
transfers-work-container-compose-pull = baixar as imagens do projeto
