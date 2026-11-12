# oom-watch

Quando a memória acaba, algo é fechado para a máquina não travar — e uma janela que some sem
explicação é o defeito que este projeto passou o tempo removendo. O `systemd-oomd` já avisava pelo
barramento (`org.freedesktop.oom1.Manager.Killed`, a notificação `memory-killed` do rodapé); o OOM
do próprio kernel, não. Este módulo segue o journal e diz os dois.

## Três linhas no journal, uma morte

O módulo pede ao seguidor único do núcleo três coisas, e nada mais:

- a linha do kernel (`_TRANSPORT=kernel`, `Killed process <pid> (<comm>) …, anon-rss:…`), em `err`:
  o pid, o nome e a memória que o processo ocupava; o prefixo diz se foi a máquina inteira (`Out of
  memory`) ou o limite do grupo (`Memory cgroup out of memory`, um `MemoryMax=`);
- a palavra do systemd sobre a unidade que perdeu um processo — `MESSAGE_ID` `fe6faa94…` para o
  kernel, `d989611b…` para o oomd, com `UNIT` (sistema) ou `USER_UNIT` (o gerente da pessoa) —, em
  `notice`;
- a decisão do oomd (`SYSLOG_IDENTIFIER=systemd-oomd`, `Killed <cgroup> due to memory …`, ou
  `Marked <cgroup> for killing due to memory …` num systemd mais novo), em `notice`: o cgroup, a
  pressão que ele mediu e o limite que ela passou, ou a memória em uso.

Nenhuma pede o passado nem `info`: o seguidor lê até `notice`, e o volume do journal mora em `info`.
Em repouso não custa nada além do seguidor que o núcleo já roda. Quando outro módulo pede o passado
(o `journal-errors`, em `err`), a passada única do núcleo lê na prioridade do seguidor, e ele segue
do fim dela: nada escrito antes de o módulo começar a vigiar chega como novidade.

As duas linhas de uma mesma morte — a do kernel e a da unidade, ou a do oomd e a da unidade — chegam
em qualquer ordem (o kernel escreve pelo `/dev/kmsg`, o systemd por um soquete), então uma espera a
outra por três segundos, com o relógio armado só enquanto espera. A linha do kernel sem par é dita
sozinha: um processo fora de qualquer unidade não tem linha do systemd.

## A palavra do systemd sobre o kernel não é uma morte sozinha

O systemd escreve `fe6faa94…` para toda unidade cuja contagem `oom_kill` subiu, e essa contagem soma
os grupos de dentro: uma morte num escopo sob o gerente da pessoa também é uma linha sobre
`user@1000.service` e sobre as fatias acima dele. E depois de um `daemon-reload` ele a escreve de
novo para unidades cuja contagem esqueceu, sem morte nenhuma — medido num journal de verdade:
`user.slice` e `user-1000.slice` logo após "Reloading finished", sem linha do kernel por perto. Por
isso:

- a linha sobre uma unidade que contém unidades (uma fatia, ou o `user@<uid>.service` de um gerente)
  não é metade de nada;
- a linha do kernel pega a linha de unidade MAIS PRÓXIMA no tempo, nunca simplesmente a primeira;
- a linha de unidade sobre o kernel sem linha do kernel é descartada: o kernel escreve a dele em
  toda morte que faz, então uma linha de unidade sem ela é uma contagem relida, não uma morte.

A do oomd (`d989611b…`) é diferente: o systemd a lê de uma marca que o oomd deixa no próprio grupo
que matou, então ela sozinha ainda é uma morte, e o par do oomd se junta pela identidade (a unidade é
a última parte do cgroup que o oomd nomeia).

## Uma vez por morte

O que foi dito fica guardado em disco pela chave de cada linha (pid, cgroup ou unidade) com o
segundo em que foi escrita. O cursor do journal é gravado a cada cinco segundos, então um cérebro
que reiniciou relê alguns segundos; a mesma linha relida não é outra morte.

## E a notificação do barramento

O rodapé já levanta a notificação `memory-killed` quando o sinal do oomd chega. O evento deste
módulo leva a etiqueta dessa entrada do catálogo, o cgroup e o segundo da morte; o rodapé, ao
reconhecê-lo, ACRESCENTA os detalhes à notificação que já levantou (`ronoctua_notify::oom::Kills`)
em vez de levantar uma segunda. O casamento é um para um: uma chegada de um lado só se junta a uma
morte que só o OUTRO lado contou, a mais próxima no tempo — um serviço que o oomd mata duas vezes
num minuto são duas mortes do mesmo cgroup, e cada uma tem a sua notificação. Uma morte do kernel não tem sinal no barramento, e vira notificação
própria. Sem o cérebro, o aviso do barramento continua sozinho, como era.

A pressão de uma morte do kernel é a da máquina no instante em que a linha foi ouvida
(`/proc/pressure/memory`, pelo `ronoctua-memory`), e só quando a linha é recente: uma linha lida uma
hora depois, ao voltar de um reinício, levaria a pressão de uma máquina calma. Num kernel sem PSI o
arquivo não existe, e a frase diz que a pressão não foi medida em vez de dizer zero.

## E a RAM, marcada enquanto está quase toda em uso

Pedido do dono (2026-11-06): um glifo na modeline, com a porcentagem, quando o uso de RAM está alto
— antes de alguma coisa ser fechada, e não só depois. O pulso que o núcleo já toma a cada tique traz
a RAM em uso (`Pulse::ram_used_pct`, o que não está em `MemAvailable`), então a marca não lê nada a
mais. A partir de 85% ela aparece, na cor de aviso, com a porcentagem ao lado do glifo (`percent` no
manifesto: o rodapé escreve o sinal pela língua); a partir de 95%, na de perigo, porque ali o oomd
está perto de escolher o que fechar. Depois de subir, só desce cinco pontos abaixo da linha: um uso
que oscila em cima dela piscaria a marca a cada tique. Em repouso não há marca nenhuma (inv. 14), e
uma RAM que ninguém mediu também não tem. O clique abre o painel de memória (`LEADER s m`), que diz
quem está comendo a memória e se a proteção está armada — o mesmo que a notificação de uma morte
abre.

A marca diz o USO, e não a pressão: a pessoa pediu a porcentagem, e uma porcentagem de RAM é o que
ela compara com o que vê num monitor. A pressão é o que o oomd julga, e pode subir com a RAM ainda
longe do teto (o swap lento devolvendo páginas); quando ele age, a morte é dita por este módulo, com
a pressão daquele momento.

Mas a pressão também LEVANTA a marca (2026-11-12). Numa noite de build, o `nix-daemon` cresceu até
25 GiB: o `MemoryHigh` dele segurou a RAM em 76% e empurrou o resto para o swap, que encheu; a
máquina passou horas esperando pela memória até a tela congelar, e a marca nunca subiu, porque a
RAM nunca chegou a 85%. Por isso ela aparece também enquanto a máquina espera pela memória um quinto
do tempo (o `some avg10` de `/proc/pressure/memory`, que o pulso já traz em `memory_stall`), e na cor
de perigo a partir de dois quintos, seja qual for a RAM; só desce com a espera abaixo de um décimo,
porque uma média de dez segundos oscila. O número ao lado do glifo continua sendo a RAM, e sob o
ponteiro a marca diz, antes da dica, quanto do tempo a máquina está esperando.
