# The system-update module's own texts: its name and the line under it, its
# commands, the name and the sentence of every step of the sequence, the
# closed words the sequence's table carries, and the view that watches a run.
brain-task-system-update = Atualizar o sistema
brain-task-system-update-does = Atualiza a máquina inteira: junta o que você declarou nas suas outras máquinas, adianta a distro e o nixpkgs e monta o sistema novo para o próximo reinício — nada do que está rodando agora é parado ou reiniciado. Também atualiza seus programas e caches, libera espaço no store e publica o que esta máquina fez. No próximo reinício o sistema novo é experimentado uma vez; se ele não subir, a máquina volta sozinha ao último sistema bom. Quando há um sistema novo esperando, a execução termina pedindo que você reinicie.
brain-task-system-update-brief = Atualiza a distro, o nixpkgs e seus programas, e deixa o sistema novo para o próximo reinício.

# ITS COMMANDS. The whole sequence now, and the two doors of the view that
# watches it — the footer's region and a window.
cmd-system-update = Atualizar o sistema
cmd-system-update-open = A atualização, no rodapé
cmd-system-update-watch = A atualização, numa janela

# EACH STEP'S NAME, and under it the line that reaches the message line when
# the cursor is on it. The name is what it is called; the line under it is
# what it actually runs, and what it costs to be wrong about.
#
# The generator (`gen-updates.py`, beside this folder's `steps.nix`) derives
# both keys from every step's id and hands them to the brain on the wire, so
# a step with no sentence in both languages cannot be written. The order is
# the SEQUENCE'S order, which is the declaration's: sorting these would put
# back the second source the table exists to remove.
system-update-step-flake-update = buscar as versões novas
system-update-step-flake-update-does = Leva a distro e o nixpkgs às versões mais novas; todo o resto fica como foi testado. Numa máquina que segue um checkout local da distro, só o nixpkgs anda. É pulado enquanto a máquina está retida, ou quando a bateria ou o disco impediriam a reconstrução.
system-update-step-system-rebuild = reconstruir o sistema
system-update-step-system-rebuild-does = Monta o sistema novo e o deixa para o próximo reinício, sem parar nada do que está rodando. Se a montagem falhar, a máquina fica como estava e a atualização para aqui.
system-update-step-parts-plan = planejar as partes
system-update-step-parts-plan-does = Registra as partes de que este sistema e o novo precisam, como programas não livres, para a limpeza mais adiante preservá-las. Nunca espera um download.
system-update-step-flatpak-system = atualizar os Flatpaks do sistema
system-update-step-flatpak-system-does = Atualiza os Flatpaks instalados para a máquina inteira, sem instalar nenhum novo. Os seus próprios Flatpaks são atualizados num passo mais adiante.
system-update-step-android-images = atualizar a imagem do sistema Android
system-update-step-android-images-does = Atualiza o sistema em que os seus apps Android rodam, cerca de um gigabyte, e confere o download. Espera enquanto o Android está aberto, e vem desligado até a configuração desta máquina ligá-lo.
system-update-step-old-generations = aparar as gerações antigas do sistema
system-update-step-old-generations-does = Apaga as gerações mais antigas do sistema, mas guarda as mais novas, a que está rodando e as que o menu de boot lista, para você sempre poder voltar. Os seus programas do Nix não são tocados.
system-update-step-store-collect-garbage = liberar espaço no store
system-update-step-store-collect-garbage-does = Apaga do store o que nada mais usa. Nunca remove uma geração; quem decide quais saem é o passo anterior.
system-update-step-store-optimise = deduplicar o store
system-update-step-store-optimise-does = Guarda uma cópia só dos arquivos iguais do store, para economizar espaço. Roda depois da limpeza, para não trabalhar no que ia ser apagado.
system-update-step-journal-vacuum = aparar o registro do sistema
system-update-step-journal-vacuum-does = Apaga as entradas mais antigas do registro do sistema, até um tamanho fixo. Vem desligado, porque apagaria também o registro do motivo de uma atualização ter falhado.
system-update-step-font-cache = refazer o cache de fontes
system-update-step-font-cache-does = Refaz do zero o cache de fontes do sistema que está rodando, para os programas acharem as fontes certas. As fontes do sistema novo entram no cache na primeira atualização depois do reinício.
system-update-step-tldr-cache = atualizar as páginas do tldr
system-update-step-tldr-cache-does = Baixa as páginas mais novas do tldr, exemplos curtos de uso de comandos, para a sua pasta pessoal. Sem rede, o passo é pulado em vez de falhar.
system-update-step-nix-profile = atualizar os seus programas do Nix
system-update-step-nix-profile-does = Atualiza os programas do seu perfil do Nix para o nixpkgs do sistema que está rodando, sem instalar nenhum novo. O nixpkgs do sistema novo chega a eles na primeira atualização depois do reinício.
system-update-step-flatpak-user = atualizar os seus Flatpaks
system-update-step-flatpak-user-does = Atualiza os seus apps Flatpak, os runtimes que eles usam e o catálogo do Flathub, sem instalar nenhum novo.
system-update-step-appimages = atualizar os seus AppImages
system-update-step-appimages-does = Procura uma versão mais nova de cada AppImage da sua pasta Applications e troca o arquivo de uma vez só, então um download que falha deixa o antigo funcionando.
system-update-step-android-apps = atualizar os seus apps Android
system-update-step-android-apps-does = Atualiza os apps Android que vieram do F-Droid por este sistema, conferindo quem assinou cada um. Se o Android estiver aberto, nada é trocado e os apps esperam a próxima atualização.
system-update-step-rust-toolchains = atualizar os toolchains do Rust
system-update-step-rust-toolchains-does = Atualiza os toolchains do Rust que você mesmo instalou na sua pasta pessoal, sem acrescentar nenhum. O Rust já vem com o sistema, então este passo é para a máquina em que você o instalou à mão.
system-update-step-node-global = atualizar os pacotes globais do npm
system-update-step-node-global-does = Atualiza os pacotes do npm que você instalou de forma global, sem acrescentar nenhum. O Node vem com o sistema, mas esses pacotes não; sem eles, o passo é pulado.
system-update-step-pnpm-global = atualizar os pacotes globais do pnpm
system-update-step-pnpm-global-does = Atualiza os pacotes do pnpm que você instalou de forma global. O pnpm os guarda à parte dos do npm, por isso este é um passo separado.
system-update-step-python-tools = atualizar as ferramentas do uv
system-update-step-python-tools-does = Atualiza as ferramentas Python que você instalou com o uv, sem acrescentar nenhuma. O uv vem com o sistema, mas elas não; sem elas, o passo é pulado.
system-update-step-flatpak-unused = remover os runtimes Flatpak que nada usa
system-update-step-flatpak-unused-does = Remove os runtimes e extensões Flatpak de que nenhum app seu precisa mais, deixados para trás pelas atualizações. Os seus apps nunca são tocados.
system-update-step-nix-profile-history = esquecer as gerações antigas do perfil do Nix
system-update-step-nix-profile-history-does = Esquece as gerações antigas do seu perfil do Nix, que mantêm programas antigos no store. Vem desligado, para nada automático tirar a sua volta; a vista de programas oferece isso quando você pede.
system-update-step-appimage-cache = limpar o que os AppImages deixaram para trás
system-update-step-appimage-cache-does = Apaga as cópias desempacotadas de AppImages que sumiram ou foram substituídos, que nada mais limpa. Os AppImages em si nunca são tocados.

# WHY A STEP WAS SKIPPED. The path or the name it is about travels beside the
# sentence as data (`Needs::about()`), which is why none of these names one.
system-update-needs-always = Nada impede este passo de se aplicar.
system-update-needs-binary = O programa que ele roda não está instalado para você.
system-update-needs-directory = Falta nesta máquina uma pasta de que ele precisa.
system-update-needs-writable-file = Falta um arquivo que ele precisa mudar, ou não dá para mudá-lo.
system-update-needs-network = Esta máquina não está conectada a uma rede.

# THE VIEW THAT WATCHES A RUN, in both doors: its keys and what each says under
# the pointer, `update now` (the key the view teaches for the whole sequence),
# what starting said, and the word beside each step.
system-update-next-step = próximo
about-system-update-next-step = Desce uma linha na lista: o status, os avisos, o que fazer, as opções e os passos.
system-update-previous-step = anterior
about-system-update-previous-step = Sobe uma linha na lista.
system-update-section = seção
about-system-update-section = Pula para o começo da próxima parte da lista, ou da anterior: avisos, o que fazer, opções, passos.
system-update-output-down = saída abaixo
about-system-update-output-down = Desce a saída uma página, rumo ao fim.
system-update-output-up = saída acima
about-system-update-output-up = Sobe a saída uma página, rumo ao começo.
system-update-output-end = o fim
about-system-update-output-end = Vai para o fim da saída, onde está a execução em curso.
system-update = atualizar agora
echo-system-update-queued = Entrou na fila — começa quando a máquina estiver livre
echo-system-update-started = A atualização começou
echo-system-update-nowhere = Não há nada aqui que consiga começá-la
system-update-verdict-ran = deu certo
system-update-verdict-failed = falhou
system-update-verdict-off = desligado
system-update-verdict-skipped = não se aplica
system-update-verdict-rehearsed = só simulou
system-update-verdict-stopped = parou aqui
system-update-verdict-pending = ainda vem

# THE TOP OF THE VIEW, in both doors: one line that says where the update
# stands — the brain's run first, then what waits for you, then what GitHub
# said of the distro's and nixpkgs' branches —, the actions under it, and what
# the focused line is. A countdown's answers and the stop are taught only
# while they mean something.
system-update-proceed-does = Começa agora a atualização em contagem regressiva, sem esperar a contagem.
system-update-abort-does = Adia a atualização em contagem regressiva para o próximo horário dela.
system-update-proceed-rehearsal-does = Começa agora a simulação em contagem regressiva, sem esperar a contagem.
system-update-abort-rehearsal-does = Desiste da simulação em contagem regressiva: nada muda, e a atualização agendada fica como estava.
system-update-stop = parar a atualização
about-system-update-stop = Para a atualização em andamento. Os passos que ela já fez continuam feitos.
system-update-stop-rehearsal = parar a simulação
about-system-update-stop-rehearsal = Para a simulação em andamento. Nada na máquina mudou.
system-update-start-does = Começa uma atualização completa agora, depois de uma contagem regressiva curta que você pode pular ou cancelar.
system-update-start-queued = A atualização já está na fila, então não há o que começar. Ela começa sozinha assim que nada a segurar.
system-update-status-running = Uma atualização está em andamento.
system-update-status-counting-down = Uma atualização vai começar: a contagem regressiva está correndo.
system-update-status-rehearsing = Uma simulação está em andamento: nada na máquina muda.
system-update-status-rehearsal-soon = Uma simulação vai começar: a contagem regressiva está correndo. Nada na máquina vai mudar.
system-update-status-queued = Uma atualização está na fila e começa quando a máquina estiver livre.
system-update-status-waiting-ram = Uma atualização está esperando: a memória está ocupada.
system-update-status-waiting-cpu = Uma atualização está esperando: o processador está ocupado.
system-update-status-waiting-battery = Uma atualização está esperando o carregador.
system-update-status-waiting-busy = Uma atualização está esperando outra tarefa do Brain terminar.
system-update-status-waiting-network = Uma atualização está esperando uma rede que não cobre pela quantidade de dados.
system-update-status-waiting-asleep = Uma atualização está esperando a máquina acordar.
system-update-status-restart = Um sistema novo está pronto: reinicie para usá-lo.
system-update-status-restart-unguarded = Um sistema novo está pronto: reinicie para usá-lo. Se ele não subir, só o menu de boot leva você de volta.
system-update-status-failed = A última atualização falhou. Os passos abaixo mostram onde.
system-update-status-held = Esta máquina está retida numa versão, então as atualizações deixam a distro e o nixpkgs como estão.
system-update-status-checking = Verificando se há atualização disponível…
system-update-status-available-distro = Há uma atualização disponível: a distro tem uma versão nova.
system-update-status-available-nixpkgs = Há uma atualização disponível: o nixpkgs tem versões novas.
system-update-status-available-both = Há uma atualização disponível: a distro e o nixpkgs têm versões novas.
system-update-status-up-to-date = O sistema está em dia.
system-update-status-unchecked = Não foi possível verificar atualizações: { $why }.
system-update-status-unasked = Ainda não foi verificado se há atualização.
system-update-unchecked-offline = esta máquina está sem rede
system-update-unchecked-rate-limited = o GitHub pediu para esperar um pouco
system-update-unchecked-other = o GitHub não respondeu como esperado
system-update-unchecked-no-lock = esta máquina não tem um flake próprio
system-update-unchecked-unreadable = o flake.lock desta máquina não pode ser lido
system-update-unchecked-nothing = o flake desta máquina não segue nenhuma branch no GitHub
system-update-unchecked-unasked-distro = a distro não segue uma branch nomeada no GitHub, então só uma atualização pode dizer
system-update-unchecked-unasked-nixpkgs = o nixpkgs não segue uma branch nomeada no GitHub, então só uma atualização pode dizer
system-update-register-belongings = Escolher abre os seus pertences, onde estão os arquivos alterados em duas máquinas.
system-update-register-asks = Escolher pergunta antes; nada muda até você responder sim.

## ── anchor: L03 ──

## ── anchor: L05 ──

## ── anchor: L19 ──

# A LEVA DE 2026-10-05: os passos que juntam antes da metade root, migram,
# conferem, reconciliam e publicam — na ordem da sequência — e o que a vista
# diz dos registros da máquina e de uma rodada na bateria.
system-update-step-person-fetch = trazer o que você declarou em outro lugar
system-update-step-person-fetch-does = Traz dos seus pertences os programas que você declarou ou recusou nas suas outras máquinas. Só entra o que a sua chave assinou; sem repositório de pertences, o passo é pulado.
system-update-step-person-pins = procurar versões novas dos seus programas
system-update-step-person-pins-does = Anota a versão mais nova de cada programa declarado que segue os próprios lançamentos, como um AppImage ou um app do F-Droid. A não ser que você tenha desligado isso, os apps desse tipo da distro também passam das versões testadas.
system-update-step-person-sign = assinar o que você declarou
system-update-step-person-sign-does = Assina as suas declarações com a sua chave, para esta máquina aceitá-las. Se você não digitou a sua senha nesta sessão, o que mudou aqui espera a próxima sincronização.
system-update-step-person-stage = entregar as suas declarações à máquina
system-update-step-person-stage-does = Entrega as suas declarações à máquina, que as confere e as aceita como dados, nunca como código. Se ela as recusar, a atualização monta com as que já tinha.
system-update-step-machine-migrate = dar à máquina o próprio flake
system-update-step-machine-migrate-does = Dá um flake próprio à máquina que ainda carrega uma cópia da distro, trocando só depois que ele montar. As outras máquinas pulam este passo; se falhar, nada muda e a atualização para aqui.
system-update-step-person-reconcile = trazer o que está declarado
system-update-step-person-reconcile-does = Instala os apps Android que as suas declarações listam, e tira a cópia instalada à mão de um programa que agora é declarado, para os dois não se atrapalharem.
system-update-step-person-record = registrar o que esta máquina fez
system-update-step-person-record-does = Guarda nos seus pertences o estado desta máquina, como o último sistema bom e como foi a última mudança de sistema, para as suas outras máquinas verem. Nada é escrito se nada mudou.
system-update-step-person-publish = publicar o que esta máquina fez
system-update-step-person-publish-does = Envia as suas declarações e o estado desta máquina ao repositório dos seus pertences, sem pedir a sua senha. Roda depois de todos os passos que os mudam, para enviar o que esta atualização deixou.

system-update-needs-file = Falta nesta máquina um arquivo de que ele precisa.
system-update-needs-power = A bateria está baixa demais para este passo, e ninguém aceitou fazer esta atualização na bateria.
system-update-needs-free-space = O disco está quase cheio, mesmo depois de liberar espaço no store.
system-update-needs-not-held = Esta máquina está retida numa versão, então a distro e o nixpkgs ficam onde estão.

system-update-declined-empty = Esta máquina ainda não sincronizou os seus pertences, então não há chave para assinar. A atualização segue.
system-update-declined-no-passphrase = Você não digitou a sua senha nesta sessão, então nada foi assinado aqui. O que as suas outras máquinas assinaram chega assim mesmo; uma mudança feita aqui espera a próxima sincronização.
system-update-declined-busy = Outra mudança nesta máquina estava em andamento, então este passo não mudou nada. A atualização segue com o que a máquina já tinha.
system-update-declined-did-not-boot = Este é o sistema que não subiu quando foi testado. Não há nada novo para montar até a distro ou o nixpkgs andarem, e o resto da atualização segue.
system-update-declined-nothing-to-migrate = Esta máquina já tem o próprio flake, então não havia nada a fazer. A atualização segue.
system-update-verdict-declined = nada novo

# O QUE OS REGISTROS DA MÁQUINA DIZEM, acima dos passos, nas duas portas — só
# o que diz alguma coisa. O ensaio de boot e o motivo de não haver um são as
# palavras da própria máquina (`machine-trial-…`, `machine-no-trial-…`).
system-update-records-conflicts = { $count ->
        [one] Um arquivo dos seus pertences mudou aqui e em outra máquina, e ficou a versão desta máquina. Abra com { $keys }.
       *[other] { $count } arquivos dos seus pertences mudaram aqui e em outra máquina, e ficaram as versões desta máquina. Abra com { $keys }.
    }
system-update-records-hold = Retida em { $rev } desde { $since }.
system-update-records-staged = Um sistema novo está pronto para o próximo reinício; até lá a máquina fica com o atual. Esse reinício testa o novo uma vez.
system-update-records-staged-unguarded = Um sistema novo está pronto para o próximo reinício; até lá a máquina fica com o atual. Se ele não subir, só o menu de boot leva você de volta.
system-update-records-checkout = Esta máquina segue um checkout local da distro, então as atualizações movem só o nixpkgs. Os commits feitos ali só chegam ao sistema por uma ação que você aprova com o sudo.

# UMA RODADA À MÃO COM A BATERIA BAIXA pergunta antes, com o não primeiro;
# uma agendada espera o carregador e não pergunta a ninguém.
system-update-ask-on-battery = A bateria está em { $percent }%, abaixo dos { $floor }% necessários para montar o sistema. Atualizar na bateria mesmo assim?
system-update-ask-on-battery-unknown = A máquina está na bateria, a carga não pode ser lida, e uma atualização monta o sistema inteiro. Atualizar na bateria mesmo assim?
system-update-answer-no = não, esperar o carregador
system-update-answer-yes = sim, atualizar na bateria
echo-system-update-on-battery = A atualização começou na bateria, como você disse
echo-system-update-not-started = Não começou: ligue a máquina na tomada, ou responda sim

# UMA RODADA À MÃO QUE O CÉREBRO SEGURA NUMA REDE TARIFADA pergunta se é para
# gastar esta, com o não primeiro; o não a deixa esperando na fila.
system-update-ask-metered = Esta conexão cobra pela quantidade de dados, e o sistema novo pode ter gigabytes, então a atualização espera outra rede. Atualizar nesta conexão mesmo assim?
system-update-answer-no-metered = não, esperar outra rede
system-update-answer-yes-metered = sim, atualizar nesta conexão
echo-system-update-on-metered = A atualização começou nesta conexão, como você disse
echo-system-update-kept-waiting = Continua na fila e começa sozinha quando não houver rede tarifada nem bateria baixa no caminho

# A LEVA DE 2026-10-11: o envio para o cache binário da pessoa, por último.
system-update-step-binary-cache-push = enviar para o seu cache binário
system-update-step-binary-cache-push-does = Envia um sistema novo que esta máquina montou para o seu próprio cache no Cachix, usando o seu token. Um cache público nunca recebe nada seu nem um programa não livre.
system-update-declined-no-token = O seu cache binário não tem token de envio nesta máquina, então nada foi enviado. Adicione um na página do cache binário da atualização.
system-update-declined-not-let = Um site de que o envio precisa não está permitido, então nada foi enviado. Nomear o cache de novo permite os sites dele.
system-update-declined-nothing-new = Nenhuma atualização montou um sistema novo desde o último envio, então não há nada novo para enviar.
system-update-declined-no-cache = Esta máquina não tem um cache binário seu configurado, então não há para onde enviar.

# THE RUN THAT SET A NEW SYSTEM FOR THE NEXT BOOT ends saying so, once —
# promising the boot trial's way back only when one is armed.
notif-system-update-restart-summary = Reinicie para usar o sistema novo
notif-system-update-restart-body = A atualização montou um sistema novo e o deixou para o próximo reinício: nada do que roda agora foi parado. Até você reiniciar, esta máquina roda o mesmo de antes; esse reinício experimenta o novo uma vez, e volta sozinho para o último bom se ele não subir.
notif-system-update-restart-unguarded-summary = Reinicie para usar o sistema novo
notif-system-update-restart-unguarded-body = A atualização montou um sistema novo e o deixou para o próximo reinício: nada do que roda agora foi parado. Até você reiniciar, esta máquina roda o mesmo de antes; esse reinício não tem volta além do menu de boot: se o novo não subir, escolha lá o anterior.

# WHY THE MACHINE'S OWN PROGRAM (`ronoctua-machine`) DID NOT DO WHAT IT WAS
# ASKED — an apply, a migration, a bless or a hold: on the step under the
# cursor in both doors, in the gestures of the view, and as the reason of a
# run of this task that stopped at one of its steps.
system-update-machine-refused-not-root = Só o root pode mudar a máquina, então nada foi feito.
system-update-machine-refused-busy = Outra mudança nesta máquina está em andamento. Tente de novo quando ela acabar.
system-update-machine-refused-nothing-to-migrate = Esta máquina já tem o próprio flake, então não há nada a fazer.
system-update-machine-refused-build-failed = Não deu para montar o sistema novo, então nada mudou. Se um download falhou, a página das partes o mostra e diz como trazê-lo.
system-update-machine-refused-switch-failed = A troca para o sistema novo falhou. O próximo reinício o testa uma vez, e volta ao último sistema bom se ele não subir.
system-update-machine-refused-next-boot-failed = Não deu para deixar o sistema novo para o próximo reinício. Nada do que roda mudou, mas o menu de boot pode não listá-lo, e o próximo reinício pode subir o sistema atual.
system-update-machine-refused-did-not-boot = Este é o sistema que não subiu quando foi testado, ou que foi desfeito. A máquina fica no sistema atual até chegar uma versão mais nova da distro, ou até você pedir nesta página para testá-lo de novo.
system-update-machine-refused-last-good-would-fall-out = Mais um sistema tiraria o último sistema bom do menu de boot. Reinicie antes, para o sistema em teste ser confirmado.
system-update-machine-refused-held = Esta máquina está retida numa versão diferente da que está configurada nela, então nada foi montado.
system-update-machine-refused-no-failed-trial = Nenhum sistema falhou no teste de boot desta máquina, então não há o que tentar de novo.
system-update-machine-refused-checkout-moved = A distro ficou onde estava: numa máquina que segue um checkout local, só uma ação que você aprova com o sudo a move.
system-update-machine-refused-unreadable = Um arquivo da configuração desta máquina não pode ser lido.
system-update-machine-refused-failed = Um programa de que a máquina precisava falhou. O registro do sistema diz qual.

# ROOT'S APPLIER REFUSED YOUR DECLARATIONS: its reason, as a sentence — on the
# step under the cursor, in both doors, and in the run's notification.
notif-system-update-declarations-refused-summary = Suas declarações não chegaram a esta máquina
notif-system-update-declarations-refused-body = O aplicador da máquina as recusou: { $why } Esta atualização montou as que a máquina já tinha.
system-update-applier-refused-unknown = Esta versão não sabe dizer o motivo; o registro do sistema o tem.
system-update-applier-refused-nothing-staged = Não havia declarações para entregar.
system-update-applier-refused-link = As suas declarações contêm um link simbólico, então a máquina não aceita nenhuma delas.
system-update-applier-refused-hard-link = Um arquivo das suas declarações tem um segundo nome, um hard link, então a máquina não aceita nenhuma delas.
system-update-applier-refused-special = As suas declarações contêm um pipe, um socket ou um dispositivo, então a máquina não aceita nenhuma delas.
system-update-applier-refused-misplaced = Nas suas declarações, há um arquivo onde deveria haver uma pasta, ou uma pasta onde deveria haver um arquivo.
system-update-applier-refused-not-owned = Algo nas suas declarações não pertence a você, então a máquina não aceita nenhuma delas.
system-update-applier-refused-writable = Outra pessoa pode alterar algo nas suas declarações, então a máquina não aceita nenhuma delas.
system-update-applier-refused-bad-name = Um nome nas suas declarações não é texto simples.
system-update-applier-refused-not-a-module = Uma pasta entre os seus módulos do Brain não se chama my- seguido de uma palavra.
system-update-applier-refused-too-large = Um arquivo das suas declarações, ou todas elas juntas, é maior do que a máquina aceita.
system-update-applier-refused-too-many = As suas declarações têm mais entradas, ícones, módulos ou arquivos do que a máquina aceita.
system-update-applier-refused-too-deep = As suas declarações têm mais pastas dentro de pastas do que a máquina aceita.
system-update-applier-refused-not-png = Um ícone das suas declarações não é um PNG.
system-update-applier-refused-missing-icon = Uma entrada das suas declarações cita um ícone que não veio junto.
system-update-applier-refused-unreadable = Algo nas suas declarações não pôde ser lido.
system-update-applier-refused-unwritable = A máquina não conseguiu escrever na própria pasta, ou alguém além do root pode alterá-la.
system-update-applier-refused-restaged = As suas declarações mudaram de novo enquanto a máquina as lia. A próxima atualização as pega.
system-update-applier-refused-not-a-nonce = A marca que separa uma entrega da seguinte não pôde ser lida.
system-update-applier-refused-not-signed = As suas declarações não estão assinadas, e esta máquina só aceita o que a sua chave assinou.
system-update-applier-refused-bad-signature = As suas declarações não foram assinadas pela chave em que esta máquina confia para você.
system-update-applier-refused-rolled-back = Estas declarações são mais antigas que as últimas que esta máquina aceitou, e uma cópia mais antiga nunca é aceita de novo.
system-update-applier-refused-bad-key = A chave em que esta máquina confia para você não é uma chave válida.
# THE 2026-10-30 WAVE: the Android apps that wait, and why.
system-update-declined-android-in-use = O Android estava aberto, então nenhum app dele foi trocado com você usando. Eles esperam a próxima atualização.
system-update-declined-fdroid-not-let = O F-Droid não está permitido, então os seus apps Android não foram verificados e ficam nas versões que têm. Permita o f-droid.org e a próxima atualização os atualiza.

# THE LIST, 2026-11-08: the view as one list under the status — what the
# machine wants said, what to do, the options and the steps — and what each
# thing is, said where the door explains it (the window's foot, the footer's
# explanation line), never inside the list.
system-update-section-notices = Avisos
system-update-section-notices-about = O que esta máquina quer que você saiba sobre as atualizações. Cada aviso diz o que aconteceu e, quando dá, o que você pode fazer.
system-update-section-do = O que fazer
system-update-section-do-about = O que você pode fazer agora: atualizar, simular uma atualização sem mudar nada ou ver quando a máquina se atualiza sozinha.
system-update-section-options = Opções
system-update-section-options-about = Escolhas que valem para toda atualização. Sob o ponteiro, cada uma diz o que o clique nela faz.
system-update-section-steps = Passos
system-update-section-steps-about = O que uma atualização faz, na ordem. A caixa de cada passo diz se ele roda: um passo desligado é pulado.
system-update-steps-count = { $on ->
        [0] nenhum de { $all } ligado
        [one] { $on } de { $all } ligado
       *[other] { $on } de { $all } ligados
    }
system-update-status-name = Se há atualização
system-update-status-explained = Pergunta ao GitHub se a distro ou o nixpkgs têm versões mais novas do que as que esta máquina roda. Perguntar de novo: o GitHub responde em alguns segundos, e esta linha muda com a resposta.
system-update-status-stands = Onde a atualização está
system-update-last-worked = Última atualização que deu certo: { $when }.
system-update-start-queued-short = já está na fila
system-update-rehearse = simular, sem mudar nada
about-system-update-rehearse = Simula uma atualização inteira sem mudar nada, para você ver o que ela baixaria e montaria.
system-update-rehearse-does = Roda a atualização inteira só de faz de conta: cada passo diz o que faria, e nada na máquina muda. Simular: começa depois da mesma contagem regressiva de uma atualização, e no fim diz quanto baixaria e quantos pacotes montaria.
system-update-rehearse-busy = Simular espera: uma atualização já está na fila ou em andamento. Quando ela terminar, dá para simular.
system-update-rehearse-busy-short = espera a atualização atual
system-update-rehearse-pending = A próxima execução já vai ser uma simulação, então não há o que pedir de novo.
system-update-rehearse-pending-short = já pedida
system-update-rehearse-running = Uma simulação já está em andamento. Quando ela terminar, dá para simular de novo.
system-update-rehearse-running-short = em andamento
system-update-rehearse-asked = Simulação pedida: começa depois da contagem regressiva, sem mudar nada.
echo-system-update-rehearse-started = A simulação começou, sem mudar nada
echo-system-update-rehearsal-stopped = A simulação parou, e nada mudou
system-update-rehearsed = Simulação de { $when }: nada mudou. A atualização baixaria { $size } e montaria { $builds ->
        [0] nenhum pacote
        [one] 1 pacote
       *[other] { $builds } pacotes
    }.
system-update-schedule = agendamento
about-system-update-schedule = Abre no Brain a tarefa da atualização, onde você escolhe quando a máquina se atualiza sozinha.
system-update-schedule-does = O Brain decide quando esta máquina se atualiza sozinha: de quantos em quantos dias, em que parte do dia, e só quando a máquina está livre. Abrir: a tarefa da atualização aparece no Brain, onde você muda isso.
system-update-schedule-every = { $days ->
        [1] todo dia
        [7] toda semana
        [14] a cada duas semanas
        [30] todo mês
       *[other] a cada { $days } dias
    }
system-update-schedule-part = { $part ->
        [night] de madrugada
        [morning] de manhã
        [afternoon] à tarde
        [evening] à noite
       *[any] a qualquer hora
    }
system-update-schedule-next = { $days ->
        [0] próxima hoje, { $time }
        [1] próxima amanhã, { $time }
       *[other] próxima { $weekday } { $date }
    }
system-update-schedule-summary = { $every }, { $part } · { $next }
system-update-schedule-off = desligado — só quando você pedir
system-update-schedule-quiet = o Brain está quieto — só quando você pedir
system-update-schedule-overdue = atrasado — começa quando a máquina estiver livre
system-update-schedule-no-brain = o Brain não está respondendo
system-update-weekday-short = { $day ->
        [0] dom
        [1] seg
        [2] ter
        [3] qua
        [4] qui
        [5] sex
       *[6] sáb
    }
system-update-step-off-shipped = desligado de fábrica
system-update-step-off-here = desligado nesta máquina
system-update-step-off-by-you = desligado por você
system-update-step-on-here = ligado nesta máquina
system-update-step-on-by-you = ligado por você
system-update-step-root-short = como administrador
system-update-switch-why-shipped-off = A distro entrega este passo desligado.
system-update-switch-why-off-here = Esta máquina deixa este passo desligado: ela não tem aquilo de que ele cuida, ou a configuração dela o desliga (ronoctua.update.steps). Não foi você que o desligou.
system-update-switch-why-off-by-you = Você desligou este passo.
system-update-switch-why-on-here = Esta máquina liga este passo: ela tem aquilo de que ele cuida, ou a configuração dela o liga (ronoctua.update.steps). Não foi você que o ligou.
system-update-switch-why-on-by-you = Você ligou este passo.
system-update-switch-root = Este passo roda como administrador, então só a configuração desta máquina o liga ou desliga, em ronoctua.update.steps. Daqui ele não muda.
system-update-switch-will = { $on ->
        [on] Ligar: a próxima atualização vai rodar este passo.
       *[off] Desligar: a próxima atualização vai pular este passo.
    }
system-update-switch-will-now = { $on ->
        [on] Ligar: esta atualização ainda não chegou aqui, e vai rodar este passo.
       *[off] Desligar: esta atualização ainda não chegou aqui, e vai pular este passo.
    }
system-update-switched = { $on ->
        [on] { $step }: ligado. A próxima atualização vai rodar este passo.
       *[off] { $step }: desligado. A próxima atualização vai pular este passo.
    }
system-update-switch-no-brain = O Brain não está respondendo, e é ele que guarda quais passos estão ligados. Nada mudou.
system-update-output-open = ver a saída
about-system-update-output-open = Mostra o que a última atualização escreveu, passo a passo, com as cores dela.
system-update-output-close = voltar à lista
about-system-update-output-close = Fecha a saída e volta à lista da atualização.
system-update-output-line-down = linha abaixo
about-system-update-output-line-down = Desce a saída uma linha.
system-update-output-line-up = linha acima
about-system-update-output-line-up = Sobe a saída uma linha.
system-update-output-does = O que a última atualização escreveu, passo a passo, como o registro do sistema guardou, com as cores dela. Abrir: a saída toma o lugar da lista, e Voltar traz a lista de volta.
system-update-output-none = Nenhuma atualização deixou saída no registro do sistema ainda, então não há o que mostrar.
system-update-output-none-short = nada ainda
system-update-output-glance = { $lines ->
        [one] uma linha da última atualização
       *[other] { $lines } linhas da última atualização
    }
system-update-output-system-hidden = A parte que roda como administrador não aparece aqui: para ler o registro do sistema é preciso estar no grupo wheel ou systemd-journal. O resultado de cada passo continua na lista.
system-update-output-title = Saída da última atualização
system-update-output-live = Saída ao vivo
system-update-output-live-does = O que esta atualização escreve enquanto roda, com as cores dela, sempre mostrando a última linha. Some quando nada está rodando; a saída inteira fica em Opções, em ver a saída da última atualização.
system-update-output-waiting = Nada escrito ainda: as linhas aparecem aqui assim que a atualização escrever.
system-update-output-system-hidden-short = sem a parte do administrador
system-update-back = Voltar
system-update-back-does = Voltar: a lista da atualização aparece de novo no lugar desta página.
system-update-conflicts-name = arquivos em conflito nos pertences
system-update-gesture-try-again-name = testar de novo o sistema que não subiu
system-update-gesture-hold-name = reter nesta versão
system-update-gesture-let-go-name = soltar a retenção
system-update-act-recheck = pergunta ao GitHub de novo
system-update-act-start = começa a atualização
system-update-act-proceed = começa já
system-update-act-not-now = adia
system-update-act-stop = para a atualização
system-update-act-stop-rehearsal = para a simulação
system-update-act-let-go = desiste da simulação
system-update-act-rehearse = começa a simulação
system-update-act-open-brain = abre a tarefa no Brain
system-update-act-open-page = abre a página
system-update-act-back = volta à lista
system-update-act-switch-on = liga o passo
system-update-act-switch-off = desliga o passo
system-update-act-open-belongings = abre os seus pertences
system-update-act-ask = pergunta antes
system-update-act-answer = responde
