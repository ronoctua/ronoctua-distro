# The system-update module's own texts: its name and the line under it, its
# commands, the name and the sentence of every step of the sequence, the
# closed words the sequence's table carries, and the view that watches a run.
brain-task-system-update = Atualizar o sistema
brain-task-system-update-does = Junta o que você declarou nas suas outras máquinas, adianta a distro, monta o sistema novo e troca para ele — o próximo boot o experimenta uma vez e volta sozinho para o último bom se ele não subir —, atualiza os seus programas e caches, recupera espaço no store e publica o que esta máquina fez. A troca reinicia os serviços cuja definição mudou; os programas já abertos continuam na versão com que começaram, e um kernel novo espera o próximo boot.

# ITS COMMANDS. The whole sequence now, its rehearsal, and the two doors of
# the view that watches it — the footer's region and a window.
cmd-system-update = Atualizar o sistema
cmd-system-update-rehearsal = Ensaiar a atualização
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
system-update-step-flake-update-does = nix flake update do input da distro, no flake desta máquina: leva a distro à versão publicada mais nova e não mexe em nada além do flake.lock — os outros inputs são os da própria versão, como ela foi testada. Precisa de rede; é pulado e diz isso enquanto a máquina está retida numa revisão, no ISO, cuja pasta da máquina é só leitura, e sempre que a reconstrução seria — na bateria abaixo do piso, ou com o disco quase cheio —, para que a pasta nunca nomeie uma distro que a máquina não montou.
system-update-step-system-rebuild = reconstruir o sistema
system-update-step-system-rebuild-does = ronoctua-machine apply: monta o sistema novo primeiro e só troca para o que montou — um build que falha deixa a máquina e a pasta dela como estavam. Depois da troca, o próximo boot experimenta o sistema novo uma vez e volta sozinho para o último bom se ele não subir. É pulado na bateria abaixo do piso, a não ser que você tenha dito sim, e com o disco quase cheio mesmo depois de coletar o lixo do store. Se falhar, a sequência para aqui.
system-update-step-flatpak-system = atualizar os Flatpaks do sistema
system-update-step-flatpak-system-does = flatpak update --system: atualiza o que a instalação Flatpak da própria máquina tem — o remoto do Flathub, e o que a configuração declarar — e não instala nada. Os seus apps estão na sua instalação e andam na sua metade, mais abaixo. Sem essa instalação o passo é pulado e diz isso.
system-update-step-android-images = atualizar a imagem do sistema Android
system-update-step-android-images-does = ronoctua-software upgrade android-system: a atualização do próprio Waydroid da imagem em que os seus apps Android rodam — cerca de um gigabyte, conferido contra a soma de verificação que o canal dele publica. Espera enquanto o Android está rodando, em vez de pará-lo. Vem desligado: ligá-lo é uma linha na configuração desta máquina.
system-update-step-old-generations = aparar as gerações antigas do sistema
system-update-step-old-generations-does = ronoctua-system-generations: saem as gerações do sistema além das mais novas (quantas, diz a configuração desta máquina) — nunca a que está rodando, a do boot, a de onde a última troca saiu, a atual do perfil ou uma que o menu de boot lista, e a volta continua existindo. Só as do sistema: o seu perfil do Nix nunca é tocado. O menu de boot não lista mais do que ela guarda, então toda entrada continua dando boot. O ensaio da atualização também roda este passo, e diz quais gerações sairiam sem apagar nenhuma.
system-update-step-store-collect-garbage = coletar o lixo do store
system-update-step-store-collect-garbage-does = nix-collect-garbage sem --delete-older-than e sem -d: apaga só o que nada referencia — nunca uma geração. Quais gerações saem, quem decidiu foi a aparagem anterior, por contagem.
system-update-step-store-optimise = deduplicar o store
system-update-step-store-optimise-does = nix store optimise: arquivos iguais viram hard link. Roda depois da coleta, senão otimizaria justamente o que estava para ser apagado.
system-update-step-journal-vacuum = aparar o journal
system-update-step-journal-vacuum-does = journalctl --vacuum-size=256M: joga fora o registro mais antigo, inclusive o desta atualização — que é a resposta para "por que o rebuild falhou?". Vem desligado.
system-update-step-font-cache = refazer o cache de fontes
system-update-step-font-cache-does = fc-cache -rf: refaz do zero e ignora as datas, porque um rebuild pode ter mudado as fontes e um cache velho com data nova é exatamente o caso que traz alguém aqui.
system-update-step-tldr-cache = atualizar as páginas do tldr
system-update-step-tldr-cache-does = tldr --update, como você: a unidade semanal as baixa como `nobody`, e o cache vai parar numa pasta que ninguém lê. Precisa de rede; sem ela o passo é pulado, em vez de esperar o timeout e chamar isso de falha.
system-update-step-nix-profile = atualizar os seus programas do Nix
system-update-step-nix-profile-does = ronoctua-software upgrade nix: os programas do seu perfil do Nix, levados ao nixpkgs do qual o sistema acabou de ser reconstruído. Não instala nenhum, e os que você aceitou mesmo não sendo livres seguem com esse mesmo aceite. Sem perfil o passo é pulado e diz isso.
system-update-step-flatpak-user = atualizar os seus Flatpaks
system-update-step-flatpak-user-does = ronoctua-software upgrade flatpak: a sua instalação Flatpak — os apps e os runtimes que eles usam — e o catálogo do Flathub junto. Não instala nenhum. Sem essa instalação o passo é pulado e diz isso.
system-update-step-appimages = atualizar os seus AppImages
system-update-step-appimages-does = ronoctua-software upgrade appimage: cada AppImage em ~/Applications contra os lançamentos do lugar de onde ele veio. O arquivo novo substitui o velho de uma vez só, então um download que falha deixa o velho funcionando. Sem essa pasta o passo é pulado e diz isso.
system-update-step-android-apps = atualizar os seus apps Android
system-update-step-android-apps-does = ronoctua-software upgrade android: os apps dentro do Android que vieram do F-Droid, cada um conferido contra quem o assinou e a soma de verificação que o índice registra. Os que você instalou de dentro do Android são assunto do próprio Android.
system-update-step-rust-toolchains = atualizar os toolchains do Rust
system-update-step-rust-toolchains-does = rustup update: atualiza os canais que já estão em ~/.rustup, e não instala nenhum. Aqui o Rust vem do flake, então essa pasta não existe e o passo é pulado dizendo isso — ele é para a máquina onde você pôs o rustup à mão.
system-update-step-node-global = atualizar os pacotes globais do npm
system-update-step-node-global-does = npm update --global: move o que já está no prefixo global e não instala nada. O node vem do flake; o prefixo global, não — sem essa pasta o passo é pulado e diz isso.
system-update-step-pnpm-global = atualizar os pacotes globais do pnpm
system-update-step-pnpm-global-does = pnpm update --global: o pnpm tem o store global dele, que não é o do npm — por isso são dois passos, e não um. Sem a pasta dele o passo é pulado e diz isso.
system-update-step-python-tools = atualizar as ferramentas do uv
system-update-step-python-tools-does = uv tool upgrade --all: sobe de versão as ferramentas que estão em $XDG_DATA_HOME/uv/tools, uma venv cada, e não acrescenta nenhuma. O uv vem do flake; elas, não — sem essa pasta o passo é pulado e diz isso.
system-update-step-flatpak-unused = remover os runtimes Flatpak que nada usa
system-update-step-flatpak-unused-does = ronoctua-software tidy flatpak: os runtimes e extensões da sua instalação de que nenhum app seu precisa mais — o que uma atualização deixa para trás. Nenhum app é tocado.
system-update-step-nix-profile-history = esquecer as gerações antigas do perfil do Nix
system-update-step-nix-profile-history-does = ronoctua-software tidy nix: as gerações antigas do seu perfil do Nix, cada uma mantendo no store os programas que tinha. Vem desligado: nada automático apaga a sua volta para um programa que você teve. A vista de programas oferece a mesma limpeza quando você pede, e ligá-lo aqui é uma linha na configuração desta máquina.
system-update-step-appimage-cache = limpar o que os AppImages deixaram para trás
system-update-step-appimage-cache-does = ronoctua-software tidy appimage: o que o appimage-run desempacotou para AppImages que sumiram ou foram substituídos — ele desempacota cada um na primeira vez que abre e nunca limpa depois. Os AppImages em si nunca são tocados.

# WHY A STEP WAS SKIPPED. The path or the name it is about travels beside the
# sentence as data (`Needs::about()`), which is why none of these names one.
system-update-needs-always = nada pode tornar este passo inaplicável
system-update-needs-binary = este programa não está no seu PATH
system-update-needs-directory = esta pasta não existe
system-update-needs-writable-file = este arquivo não existe, ou não dá para escrever nele
system-update-needs-network = o NetworkManager diz que esta máquina não está conectada

# WHAT A FAILURE DOES TO THE REST OF THE SEQUENCE. Two answers and no third:
# this runs from a unit, and there is nobody to ask.
system-update-on-failure-stop = se falhar, a sequência para aqui
system-update-on-failure-carry-on = se falhar, a sequência segue para o próximo passo

# THE FIVE CLASSES A STEP CAN BE. They decide the order laws.
system-update-kind-move-inputs = move os inputs pinados
system-update-kind-apply-configuration = aplica a configuração
system-update-kind-upgrade-software = atualiza programas que você mesmo instalou
system-update-kind-refresh-cache = refaz um cache
system-update-kind-reclaim-space = recupera espaço em disco

# THE VIEW THAT WATCHES A RUN, in both doors: its keys and what each says under
# the pointer, `update now` (the key the view teaches for the whole sequence),
# what starting said, and the word beside each step.
system-update-next-step = próximo passo
about-system-update-next-step = Vai para o próximo passo, e diz o que ele é.
system-update-previous-step = passo anterior
about-system-update-previous-step = Vai para o passo anterior, e diz o que ele é.
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
system-update-no-output = ainda não disse nada
system-update-verdict-ran = deu certo
system-update-verdict-failed = falhou
system-update-verdict-off = desligado
system-update-verdict-skipped = não se aplica
system-update-verdict-rehearsed = só ensaiou
system-update-verdict-stopped = parou aqui
system-update-verdict-pending = ainda vem

## ── anchor: L03 ──

## ── anchor: L05 ──

## ── anchor: L19 ──

# A LEVA DE 2026-10-05: os passos que juntam antes da metade root, migram,
# conferem, reconciliam e publicam — na ordem da sequência — e o que a vista
# diz dos registros da máquina e de uma rodada na bateria.
system-update-step-person-fetch = trazer o que você declarou em outro lugar
system-update-step-person-fetch-does = ronoctua-belongings plain pull: a parte em claro dos seus pertences — os programas que você declarou e recusou nas outras máquinas, os registros delas — buscada e mesclada à cópia desta máquina, sem senha e sem tocar na parte cifrada — e só o que a sua chave assinou: o resto fica retido, e a vista diz por quê. Um arquivo mudado aqui e em outra máquina fica como esta máquina o tem, e a vista diz isso. Sem repositório, ou sem rede, o passo é pulado e diz isso.
system-update-step-person-pins = adiantar as fixações
system-update-step-person-pins-does = ronoctua-software pins refresh: cada programa declarado que segue as próprias versões — um AppImage, um app Android do F-Droid — fixado na versão e no resumo que ele tem agora. Com a sua opção de segui-los ligada, e numa máquina que confia na sua chave de assinatura, também os apps da distro que seguem as próprias versões (os Crafting Apps), além das fixações que a distro testou — perguntando ao GitHub, que você permitiu, e baixando só uma versão que esta máquina aceitaria; com ela desligada, essas fixações saem e voltam as da distro.
system-update-step-person-sign = assinar o que você declarou
system-update-step-person-sign-does = ronoctua-belongings sign: as suas declarações como estão agora — o que veio das outras máquinas, as fixações recém-adiantadas — assinadas nesta máquina com a sua chave, sem rede e sem pergunta, para que a máquina as aceite. Sem a sua identidade aberta (nenhuma senha digitada nesta sessão), o passo recusa e a máquina fica com as declarações que tinha até a próxima sincronização.
system-update-step-person-stage = entregar as suas declarações à máquina
system-update-step-person-stage-does = ronoctua-software person stage: as suas declarações postas onde o aplicador estreito da máquina as pega, validadas e trocadas inteiras para a pasta da máquina — como dados, nunca como código. Um ensaio só confere que elas seriam aceitas.
system-update-step-machine-migrate = dar à máquina o próprio flake
system-update-step-machine-migrate-does = ronoctua-machine migrate: uma máquina que ainda carrega uma cópia da distro ganha o próprio flake, com a distro como input — montado ao lado da pasta velha e trocado só se montar; uma falha deixa a máquina exatamente como estava. Toda outra máquina pula este passo. Se falhar, a sequência para aqui.
system-update-step-person-verify = experimentar o que você declarou
system-update-step-person-verify-does = ronoctua-person verify: cada programa que você declarou, montado sozinho contra a distro recém-buscada, para que um que não monta mais seja retido com o motivo em vez de derrubar a reconstrução inteira — e cada versão mais nova dos apps da distro que você segue, que só é usada depois de montar aqui, e é tentada de novo em toda atualização até montar. O ensaio dele experimenta e não escreve nada.
system-update-step-person-reconcile = trazer o que está declarado
system-update-step-person-reconcile-does = ronoctua-software reconcile: o que o sistema não faz por você — os apps Android que as suas declarações listam, instalados da cópia da máquina — e a cópia que você instalou à mão de um programa agora declarado, retirada para que as duas não se sobreponham. Mexe só no que ele mesmo pôs.
system-update-step-person-record = registrar o que esta máquina fez
system-update-step-person-record-does = ronoctua-person record: o registro desta máquina — o último sistema bom, o ensaio de boot, uma retenção, como terminou a última aplicação — copiado para a parte em claro dos seus pertences, para as suas outras máquinas saberem. Fica como está quando nada mudou.
system-update-step-person-publish = publicar o que esta máquina fez
system-update-step-person-publish-does = ronoctua-belongings plain push: a parte em claro enviada ao seu repositório — as suas declarações e o registro desta máquina — sem senha e sem tocar na parte cifrada. Por último, para que o enviado seja o que esta rodada deixou.

system-update-needs-file = este arquivo não existe
system-update-needs-power = a máquina está na bateria abaixo do piso, e ninguém disse sim a esta rodada
system-update-needs-free-space = o disco está quase cheio, mesmo depois de coletar o lixo do store
system-update-needs-not-held = a máquina está retida numa revisão: a distro fica onde está

system-update-kind-gather = junta o que você declarou
system-update-kind-migrate = migra a pasta da máquina
system-update-kind-verify = experimenta o que você declarou
system-update-kind-reconcile-declared = traz o que está declarado
system-update-kind-publish = publica o que esta máquina fez

system-update-declined-empty = Esta máquina ainda não sincronizou os pertences: não há chave para assinar, e a atualização segue.
system-update-declined-no-passphrase = A sua identidade não está aberta nesta sessão: nada foi assinado aqui. O que as suas outras máquinas assinaram chega a esta assim mesmo; uma mudança feita aqui espera a próxima sincronização assiná-la.
system-update-declined-busy = Outro programa da própria máquina aplicava uma mudança e segurava a cópia das suas declarações: esta atualização constrói as que ela tinha, e as suas chegam na próxima aplicação.
system-update-declined-did-not-boot = Este é o sistema que não subiu quando foi experimentado: não há nada novo a aplicar até a distro andar, e o resto da atualização segue.
system-update-declined-nothing-to-migrate = Esta máquina já tem o próprio flake: não havia o que migrar, e a atualização segue.
system-update-declined-github-not-let = O GitHub não está permitido no seu consentimento: os apps da distro que seguem as próprias versões não foram olhados, e ficam nas versões que tinham; nenhuma fixação sua andou também. Ligar de novo a opção de segui-los permite.
system-update-verdict-declined = nada novo

# O QUE OS REGISTROS DA MÁQUINA DIZEM, acima dos passos, nas duas portas — só
# o que diz alguma coisa. O ensaio de boot e o motivo de não haver um são as
# palavras da própria máquina (`machine-trial-…`, `machine-no-trial-…`).
system-update-records-conflicts = { $count ->
        [one] Um arquivo dos seus pertences mudou aqui e em outra máquina, e ficou como esta máquina o tem. Abra com { $keys }.
       *[other] { $count } arquivos dos seus pertences mudaram aqui e em outra máquina, e ficaram como esta máquina os tem. Abra com { $keys }.
    }
system-update-records-held = { $id } está retido: { $why }. A reconstrução o deixa de fora até ele mudar.
system-update-held-eval-failed = ele não avalia
system-update-held-fetch-failed = o download dele não pôde ser buscado
system-update-held-upstream-changed = o download dele está lá, e é outro arquivo
system-update-held-build-failed = ele não montou
system-update-records-hold = Esta máquina está retida em { $rev } desde { $since }: a atualização deixa a distro onde está.

# UMA RODADA À MÃO COM A BATERIA BAIXA pergunta antes, com o não primeiro;
# uma agendada espera o carregador e não pergunta a ninguém.
system-update-ask-on-battery = A bateria está em { $percent }%, abaixo dos { $floor }% de que uma atualização precisa para montar o sistema. Atualizar na bateria mesmo assim?
system-update-ask-on-battery-unknown = A máquina está numa bateria cuja carga não dá para ler, e uma atualização monta o sistema. Atualizar na bateria mesmo assim?
system-update-answer-no = não, esperar o carregador
system-update-answer-yes = sim, atualizar na bateria
echo-system-update-on-battery = A atualização começou na bateria, como você disse
echo-system-update-not-started = Não começou: ligue a máquina na tomada, ou responda sim

# UMA RODADA À MÃO QUE O CÉREBRO SEGURA NUMA REDE TARIFADA pergunta se é para
# gastar esta, com o não primeiro; o não a deixa esperando na fila.
system-update-ask-metered = Esta conexão é tarifada — paga por byte — e a atualização espera outra antes de baixar o sistema novo, que pode ter gigabytes. Atualizar nesta conexão mesmo assim?
system-update-answer-no-metered = não, esperar outra rede
system-update-answer-yes-metered = sim, atualizar nesta conexão
echo-system-update-on-metered = A atualização começou nesta conexão, como você disse
echo-system-update-kept-waiting = Continua na fila, e começa sozinha quando nada do que ela espera estiver no caminho — uma rede tarifada, a bateria baixa

# A LEVA DE 2026-10-11: o envio para o cache binário da pessoa, por último.
system-update-step-binary-cache-push = enviar para o seu cache binário
system-update-step-binary-cache-push-does = ronoctua-cachix push: o que esta máquina construiu, enviado para o seu próprio cache no Cachix — só depois de uma atualização que trocou a máquina, e só com o seu token. Um cache público recebe só as saídas da própria distro, nunca um caminho seu nem um programa não livre; um privado recebe o sistema; nenhum recebe o que o cache.nixos.org já serve. Sem cache nomeado, o passo é pulado. O ensaio diz o que iria, e não envia nada.
system-update-declined-no-token = O seu cache binário não tem token de envio nesta máquina: nada foi enviado. Dê um a ele na seção de cache binário da atualização.
system-update-declined-not-let = Um host que o envio alcança não está permitido no seu consentimento: nada foi enviado. Nomear o cache de novo os permite.
system-update-declined-nothing-new = Nenhuma atualização trocou esta máquina desde o último envio: não há nada novo para enviar.
system-update-declined-no-cache = Esta máquina não nomeia cache binário seu: não há para onde enviar.
