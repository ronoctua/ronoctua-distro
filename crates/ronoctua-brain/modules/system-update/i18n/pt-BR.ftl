# The system-update module's own texts: its name and the line under it, its
# commands, the name and the sentence of every step of the sequence, the
# closed words the sequence's table carries, and the view that watches a run.
brain-task-system-update = Atualizar o sistema
brain-task-system-update-does = Move os inputs pinados para a frente, monta o sistema novo e troca para ele, e recupera espaço no store. A troca reinicia os serviços cuja definição mudou; os programas já abertos continuam na versão com que começaram, e um kernel novo espera o próximo boot.

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
system-update-step-flake-update-does = nix flake update em /etc/nixos: adianta os inputs pinados e não mexe em nada além do flake.lock. Precisa de rede, e no ISO o /etc/nixos é só leitura — ali o passo é pulado e diz isso.
system-update-step-system-rebuild = reconstruir o sistema
system-update-step-system-rebuild-does = nixos-rebuild switch, pela unidade que já existe: compila, e pode trazer um kernel que nunca deu boot. A volta é a geração anterior, no menu de boot. Se falhar, a sequência para aqui.
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
system-update-step-doom-sync = Alinhar os pacotes do Doom Emacs
system-update-step-doom-sync-does = Resolve a lista de pacotes do Doom contra a configuração que o rebuild acabou de escrever, e compila o que mudou. Até isso rodar, o editor ainda carrega a anterior. Pula numa máquina onde o Doom nunca foi instalado.
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
