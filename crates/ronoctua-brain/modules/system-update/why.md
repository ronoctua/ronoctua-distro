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

## As duas portas e a marca

`update_window.rs`, no shell, é a vista que olha uma execução nas duas portas (a região do rodapé e
uma janela), a marca animada da modeline e a porta que começa uma atualização à mão — pelo cérebro
quando ele está lá, direto pelo systemd quando não está: o cérebro é um agendador, não um porteiro.
É o único arquivo de fora que fala deste módulo (`namedBy`), e é dono dos três comandos; o `leader.rs`
e o rodapé os alcançam pelas tabelas dele, sem nomeá-los. A janela se fotografa (`photograph --door
update`) sobre relatórios e diário dados, nunca os da máquina, e na foto o `u` dela não começa nada:
diz o que teria pedido.
