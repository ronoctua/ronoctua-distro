# system-update

A máquina se atualiza sozinha, uma vez por semana, à tarde, atrás do portão e com um minuto de
aviso; e a pessoa pode começar a mesma atualização à mão, ou só olhar uma acontecer. O que uma
atualização É — quais passos, em que ordem, com que comando — está declarado uma vez, em
`steps.nix`, e tudo o mais sai dali. O cérebro é dono do QUANDO; a sequência é das unidades.

## Uma lista só

O cérebro já declarou a atualização DUAS vezes: três passos escritos à mão (`update-inputs`,
`build`, `collect-garbage`) contra os doze que a unidade rodava (`flake-update`, `system-rebuild`,
…), e os dois conjuntos eram disjuntos. Todo nome de passo na tela era um nome que a sequência não
usa, e a linha do "parou em" procurava uma chave que não existia. Hoje `steps.nix` é a declaração, e
o `gen-updates.py` (chamado pelo `sequence.nix`) faz dela quatro coisas: a tabela Rust que o módulo
desenha (pelo `tables.nix`), as chaves de texto que os dois idiomas devem, os dois roteiros de shell
das unidades e uma recusa em tempo de build com o id do passo na mensagem. O check `…-has-one-list`
recusa um id de passo escrito à mão em qualquer outro lugar da árvore.

As chaves de um passo são `system-update-step-<id>` e `…-does`, derivadas do id pelo gerador e
levadas no fio ao lado do id (`name_key`, `does_key`): o buffer do cérebro desenhava `update-<id>`,
uma palavra que nenhum módulo declarava. O id do módulo não está escrito no gerador: é o nome desta
pasta, passado como `--task`.

## Duas metades, root e a pessoa, nunca intercaladas

O que precisa de root roda numa unidade de sistema; o resto roda como a pessoa, na unidade de
usuário dela, com o `PATH` da sessão — uma ferramenta instalada à mão mora no `PATH` e na home da
pessoa, e nenhum processo root a acha. A metade da pessoa inicia a de root e espera por ela
(`oneshot`), então é uma sequência só, com uma trava e um status de saída. O gerador recusa um passo
de root depois de um da pessoa: intercalar custaria uma ida ao polkit a cada troca.

Nenhum passo pode INSTALAR: o gerador recusa o substrato `install`, os verbos de instalação, todo
metacaractere de shell e todo jeito de passar um argumento por um shell. Um passo que cita um binário
de fora da flake nasce desligado, não roda como root e declara o binário em `needs`, para a máquina
que não o tem ganhar um passo pulado e dito, não uma falha.

O que está DECLARADO não é instalar à mão (o inv. 6 emendado pelo dono, 2026-10-05): a atualização
traz o que a distro ou a pessoa declarou, e nada além. Quem traz é o rebuild e, para o que o sistema
não faz pela pessoa (os apps do Android, a cópia imperativa que um declarado substitui), o
reconciliador (`ronoctua-software reconcile`), que só mexe no próprio registro. Nenhum argumento da
tabela nomeia um programa a instalar: o que entra é o da declaração, lido pelo programa.

## Nada que ela faça pode perguntar

Não há agente polkit na sessão. A regra de `module.nix` nomeia as duas unidades de root, um verbo
(`start`) e um usuário, e devolve `YES`: o cérebro roda a atualização quando ninguém está olhando, e
ali um pedido de autenticação seria uma falha silenciosa com a cara de uma recusa. Os roteiros não
invocam `sudo`, `pkexec`, `pkttyagent` nem `systemd-run`, e o `systemctl` vem por caminho do store.
O `…-never-asks` transforma essa ausência em requisito.

## Os nomes das unidades moram no manifesto

As quatro unidades — as duas da pessoa, que o cérebro inicia, e as duas de root, que só a metade da
pessoa inicia — estão escritas uma vez, em `manifest.nix`. O `module.nix` as lê de lá para as
unidades, a regra e os roteiros; o Rust as lê da tabela gerada dele (`unit::*`), e a janela da
atualização também. Um nome renomeado num lugar e esquecido no outro deixava uma regra autorizando
uma unidade que não existe ao lado de uma unidade que ninguém podia iniciar.

## O interruptor de um passo chega à execução

`m t` na página do cérebro escrevia no `brain.json` e nada na máquina lia: quem decide se um passo
roda é `/etc/ronoctua/updates.conf`, que vem de uma opção do NixOS e pede rebuild. Agora o módulo,
no `prefs_saved`, escreve as viradas da pessoa em `$XDG_STATE_HOME/ronoctua/update-steps.conf`, na
forma que o roteiro já lê, e o roteiro da pessoa as aplica por último. A metade de root não lê esse
arquivo — root não tem como saber de quem é a sessão —, e a página diz isso nos passos dela.

## O ensaio ensaia a aparagem de verdade (D5)

Ensaiar roda toda precondição de verdade e imprime o que rodaria; de um passo que só inicia uma
unidade, isso era a linha `systemctl start` e nada mais. A aparagem das gerações do sistema é o
passo cujo ensaio tem algo a dizer que o nome da unidade não diz: QUAIS gerações sairiam. Então o
passo declara `rehearsal`, e o ensaio inicia no lugar dela a unidade de ensaio do `store.nix` — o
mesmo programa, com o mesmo perfil, a mesma contagem e os mesmos menus, sem `--execute` —, e traz o
que ela disse para a saída da própria sequência. Uma aparagem que falharia (um menu ilegível, um
perfil que não é o do sistema) faz o ensaio falhar do mesmo jeito.

O mesmo vale para um PROGRAMA que tem uma forma que não muda nada: o passo declara `rehearse`, e o
ensaio roda o mesmo programa com esses argumentos (`apply --rehearse` mede numa cópia o que baixaria
e construiria; `verify --rehearse` tenta e não escreve; `person stage --check`; `record
--rehearse`). Os argumentos passam por todas as recusas do `run`, e o gerador recusa um `rehearse`
igual ao `run` (o ensaio rodaria o passo de verdade) ou vazio. Sem `rehearse`, o ensaio imprime o
comando e não roda nada, que é tudo o que ele pode dizer com honestidade.

O que ela disse é lido do diário a partir de onde ele terminava ANTES do início (um cursor), só as
linhas daquela unidade, depois de um `journalctl --sync`. Não pelo id de invocação: uma oneshot que
terminou bem e que nada referencia é descarregada na hora, e o `systemctl show` responde por uma
unidade carregada de novo, sem id — o repasse não repassava nada justamente quando o ensaio dava
certo. O `--sync` existe porque a saída da unidade chega ao journald por um fluxo que ele pode ainda
não ter esvaziado quando o `systemctl start` volta. E a vista da atualização, nas duas portas, segue
as unidades de ensaio junto com as de verdade: é lá que aparece quais gerações sairiam.

Dois checks provam isso, porque o primeiro sozinho já passou sobre o defeito: o `systemctl` de
mentira do `system-update-rehearses-the-trim` respondia com um id que o systemd não dá. Hoje ele
responde como o systemd (nada, depois de descarregar a unidade), e o `system-update-rehearsal-vm`
roda a metade root do ensaio numa máquina, no systemd e no diário de verdade, sobre gerações
plantadas: a lista sai uma vez, é a das que passam das mais novas, e nenhuma é apagada.

## A máquina que tem o próprio flake (2026-10-05)

A sequência agora tem três tempos, e o gerador recusa qualquer outra ordem
(`system-update-laws-refuse` quebra cada lei uma vez e exige a frase dela):

1. **A pessoa reúne, ANTES da metade root** (`gather`): puxa a parte em claro dos pertences
   (`ronoctua-belongings plain pull`, sem senha; um conflito fica como esta máquina o tem e nunca
   bloqueia; o que a chave da pessoa não assinou fica retido), adianta as fixações (`pins refresh`),
   assina o resultado com a chave da pessoa (`ronoctua-belongings sign`, só com a identidade que a
   sessão guarda; sem ela, recusa com `no-passphrase` e root fica com a cópia que tinha, porque só
   aceita o que a chave fixada assinou) e entrega as declarações ao aplicador estreito de root
   (`person stage`). Antes, porque o rebuild tem de construir o que a pessoa declarou em TODAS
   as máquinas, e não o da semana passada; como a pessoa, porque o acesso ao repositório, o
   consentimento e a pasta de preparo são dela — root não tem nenhum dos três. Nenhum outro passo da
   pessoa vem antes da metade root.
2. **A metade root** migra (`migrate`, antes de mover os inputs: o lock do layout velho não é o do
   flake da máquina; e `stop`, porque tudo depois fala o layout novo), move SÓ o input da distro
   (`nix flake update ronoctua`: os outros inputs são os da distro, copiados do lock dela, e mover a
   distro move tudo o que a release testou e nada além; pulado e dito enquanto a máquina está retida
   — `not-held`), tenta cada entrada declarada sozinha (`verify`, entre mover e aplicar: contra os
   inputs que o rebuild vai usar, e o que não constrói fica em `local/held.json` em vez de derrubar
   o rebuild) e aplica com o `ronoctua-machine apply` — construir primeiro, trocar só o que
   construiu, e armar o ensaio de boot. A metade inteira roda sob `ronoctua-person hold`: o lock de
   root dos dados da pessoa passa pela entrada padrão a cada filho, então o que o `verify` tentou é o
   que o `apply` constrói. Por isso o rebuild é um FILHO desta metade e não mais a unidade do
   resgate: uma unidade iniciada por `systemctl start` não herdaria o lock e esperaria pelo próprio
   pai até `busy`. E o `hold` espera com PACIÊNCIA (`--patient`, bloqueado no `flock` do kernel, sem
   relógio): uma aplicação da central de programas que já constrói quando a atualização começa
   segura o mesmo lock da construção à troca, e recusar `busy` depois de meio minuto contava como
   falha uma atualização que só esperava a máquina terminar de aplicar. Ela espera, diz no diário
   da unidade o que espera, e roda depois. O relatório da metade root é esvaziado ANTES da espera
   (`ExecStartPre`): durante ela, as portas mostrariam os vereditos da execução anterior como se
   fossem desta.

   **O lock só anda quando o rebuild pode ser tentado.** O passo que move o input pede o mesmo piso
   de bateria e a mesma folga no disco que o rebuild (o gerador recusa um `move-inputs` sem eles).
   Sem isso, uma execução na bateria baixa, ou num disco que a coleta não liberou, movia o lock e
   pulava o rebuild: a pasta da máquina passava a nomear uma distro que o sistema em execução nunca
   construiu, e a próxima aplicação — a da central de programas, a do resgate — construía e trocava
   para ela inteira, sem verificação, naquela bateria ou naquele disco. O
   `system-update-moves-the-lock-only-for-a-rebuild` roda os dois passos juntos sobre uma pasta
   plantada e compara o lock byte a byte.
3. **A pessoa termina**: reconcilia o declarado (`reconcile-declared`, depois do rebuild, para
   trazer o que o sistema NOVO declara) e publica por último (`publish`): o registro da máquina
   (`ronoctua-person record` copia o `state.json`, validado e na forma canônica, para
   `machines/<host>.json` no espelho, e não toca nada quando nada mudou — uma atualização que não
   mudou nada não empurra nada) e a parte em claro (`plain push`). Por último para que o empurrado
   seja o que a execução terminou; e em TODA saída da metade da pessoa (`publish_anyway`): um
   rebuild que falhou é algo que esta máquina fez, e as outras só ficam sabendo se for empurrado.
   O último de todos (2026-10-11) envia ao cache binário DA PESSOA o que a máquina construiu
   (`ronoctua-cachix push`). Ele roda em toda saída, como os outros, mas só envia depois de um
   `apply` que TROCOU a máquina desde o último envio — o registro da máquina diz qual —, então um
   rebuild que falhou não publica nada no cache. Sem cache nomeado, sem token, sem os hosts
   permitidos ou sem nada novo, ele recusa com notícia (`no-cache`, `no-token`, `not-let`,
   `nothing-new`), nunca com falha. O arquivo da máquina é perguntado pelo programa, e não por uma
   precondição `file`: a notícia diz qual das quatro o segurou, e uma precondição diria só que falta
   um arquivo.

**Uma recusa é notícia ou é motivo.** Os programas da máquina terminam uma recusa com a última linha
`refused <palavra>`. Uma palavra que o passo declara em `declines` é notícia (`declined-<palavra>`,
a sequência segue): `did-not-boot` (o sistema cujo ensaio falhou não é tentado de novo até a distro
andar), `nothing-to-migrate` e as quatro do envio ao cache. Qualquer outra é falha que GUARDA a palavra (`failed-<palavra>`), e a
vista diz a frase da máquina para ela, em vez de só "falhou". Uma última linha que não é recusa, ou
uma palavra que não é kebab, é `failed` puro: nunca uma palavra inventada.

## Bateria, disco e rede tarifada: esperar, e nunca perguntar

Os passos que compilam (a migração, a verificação, o rebuild) e o que move o lock do qual o rebuild
constrói pedem `power`: na tomada, ou a média das baterias da máquina (a de um mouse, `scope=Device`,
não conta) no piso ou acima, ou o SIM da pessoa a ESTA execução. Um piso só para a tabela inteira (o gerador recusa dois): as portas perguntam abaixo
dele, e dois pisos seriam duas perguntas. Quarenta por cento: um rebuild desta distro são vinte
minutos de todos os núcleos, um terço da carga de um laptop.

O rebuild (e o passo que move o lock) também pede `free-space` (a parcela do disco do store com teto,
no molde do `assets/cache.nix`): abaixo dela a metade root coleta o lixo do store PRIMEIRO, mede de
novo e só então recusa, dizendo qual; um ensaio só diz que coletaria. Só root, porque só root coleta o
lixo do store.

**Uma bateria só: a da máquina.** "Descarregando" e "quanto de carga" são perguntados às MESMAS
fontes (`ronoctua_memory::machine::machine_batteries`, que pula `scope=Device`), e o módulo, as portas
e o portão geral do cérebro leem por ali. Antes, "descarregando" contava a bateria de um mouse sem fio
(sempre descarregando) e a carga não: um desktop com um mouse Logitech segurava toda atualização
agendada para sempre, e a porta pedia para ligar na tomada uma máquina que estava ligada. O
`systemd-ac-power` dos roteiros já pulava essas fontes.

Uma execução AGENDADA nunca pergunta: o módulo a SEGURA (`needs`) numa rede tarifada
(`brain-hold-network`, perguntado ao NetworkManager pelo `ronoctua-network`; um palpite de "tarifada"
conta, e "não sei" não é "não") ou numa bateria abaixo do piso (`brain-hold-battery`, pelo
`ronoctua-memory`), e ela vai quando a máquina estiver na tomada ou em outra rede. Mesmo um portão
que a pessoa afrouxou segura essas duas: um rebuild baixa gigabytes e compila por vinte minutos.

Uma execução À MÃO abaixo do piso pergunta, com o "não" primeiro e o cursor nascendo nele, na própria
vista (as duas portas): "não, esperar o carregador" / "sim, atualizar na bateria". O sim é um arquivo
no diretório de runtime da pessoa (`ronoctua-update/on-battery`) que a metade da pessoa honra e
APAGA ao sair, como quer que saia; a metade root só TESTA se ele existe, no diretório da única pessoa
que a regra polkit deixa iniciá-la (root não tem sessão para perguntar de quem é a vez) — ele só
pode fazer um passo declarado rodar na bateria, nunca mudar o que roda. Um ensaio nunca toma o sim. E
o sim vai direto ao systemd, não ao cérebro: o cérebro SEGURA na bateria baixa, e um sim dado agora
ficaria na fila dele até o carregador chegar. `ronoctua-update --execute --on-battery` é o mesmo sim
de um terminal. O `system-update-waits-for-the-charger` roda o teste da bateria da metade root sobre
fontes plantadas e as duas metades inteiras, sem nada na entrada padrão.

Uma execução À MÃO numa rede tarifada também pergunta — na porta que a pediu, e só quando o cérebro a
SEGURA por isso: o pedido vai ao cérebro como sempre, a porta SEGUE a execução no `Mind` que já
recebe (`update_window::Followed`), e quando a linha dela aparece segurada por `HOLD_METERED` (a
espera do módulo, que o portão geral não produz) a vista abre a pergunta, com o "não" primeiro: "não,
esperar outra rede" a deixa na fila, esperando como uma agendada; "sim, atualizar nesta conexão" a
tira da fila do cérebro e a começa direto pelo systemd — e, na bateria abaixo do piso, pergunta a
bateria em seguida, porque um sim não é o outro. Segue-se a decisão do próprio cérebro, e não uma
segunda leitura da rede: a porta pergunta exatamente quando o cérebro segura. Antes, `u` num hotspot
dizia "entrou na fila — começa quando a máquina estiver livre" e o cérebro a segurava enquanto o
hotspot durasse, sem perguntar a ninguém.

## O que a vista diz dos registros da máquina

Acima dos passos, nas duas portas, e só o que diz alguma coisa, separado do segundo registro por uma
linha de ar: quantos arquivos dos pertences ficaram em conflito no último pull (do repositório que a
pessoa nomeia AGORA — a base de outro é a de uma primeira vez), com as teclas que os abrem (um clique
na linha abre `LEADER s v` no rodapé, ou a janela dos pertences a partir da janela); cada programa
declarado que a verificação segurou, com o motivo; o ensaio de boot e o porquê de não haver um, nas
palavras da máquina (`machine-trial-…`, `machine-no-trial-…`); e a retenção, com a revisão curta e o
dia. Cada registro é lido pelo crate que o escreve (`ronoctua-machine`, `ronoctua-person`,
`ronoctua-belongings`), e a vista os relê quando o mtime de um deles anda, como já relia os
relatórios. Um arquivo ausente é uma resposta vazia; um que não lê não diz nada, em vez de inventar.
Uma frase clicável que quebra em várias linhas é UM controle: o ponteiro sobre qualquer uma acende
todas, nas duas portas (`update_window::one_control` no rodapé, que acende por linha).

## As duas portas e a marca

`update_window.rs`, no shell, é a vista que olha uma execução nas duas portas (a região do rodapé e
uma janela), a marca animada da modeline e a porta que começa uma atualização à mão — pelo cérebro
quando ele está lá, direto pelo systemd quando não está: o cérebro é um agendador, não um porteiro.
É o único arquivo de fora que fala deste módulo (`namedBy`), e é dono dos três comandos; o `leader.rs`
e o rodapé os alcançam pelas tabelas dele, sem nomeá-los. A janela se fotografa (`photograph --door
update`) sobre relatórios e diário dados, nunca os da máquina, e na foto o `u` dela não começa nada:
diz o que teria pedido. A cena inteira — relatórios, diário, os registros da máquina e a bateria — se
fotografa com `photograph --door update-scene --scene <arquivo>`, andada por teclas e ponteiro; o sim
da bateria e o clique nos pertences ficam guardados e impressos, nunca feitos (`"metered": true` faz o
cérebro da cena segurar o `u` por rede tarifada). A porta do rodapé se fotografa com a câmera dele
(`photograph --door footer --room <pasta>`): a sala ganhou `machine/`, `machine-state/` e `run/`, onde
a vista lê a pasta da máquina, o registro e o relatório da metade root enquanto a câmera fotografa —
caminhos absolutos que só as raízes XDG não plantavam.
