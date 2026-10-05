# clock-sync

O dono pediu que a hora automática fosse o jeito desta distro rodar, que o interruptor estivesse na
tela de data e hora, e que o mesmo interruptor fosse alcançável pelo cérebro: *"deve existir a
atualização automática do relógio/etc do sistema/ronoctua-distro via internet, sendo essa a
configuração padrão em vez do ajuste manual"*.

## Ele não busca a hora

Porque algo já faz isso direito. O `systemd-timesyncd` é um cliente NTP: disciplina o relógio
continuamente, corrige a deriva em vez de dar saltos e sabe até onde confiar numa resposta. Um módulo
que perguntasse a hora a um servidor web a cada seis horas seria um SEGUNDO acertando o relógio desta
máquina, menos exato que o primeiro e discordando dele — e o sintoma de dois donos de um relógio é um
carimbo que anda para trás.

Então ele é a vigia e o interruptor. Lê se a hora automática está ligada, liga quando não está, e DIZ
quando o relógio está largado ou nunca conseguiu concordar com ninguém. É tudo o que "automático" quer
dizer numa máquina que tem um cliente NTP, e desligar este módulo é como alguém diz que quer o relógio
para si.

## Uma vigia agendada, a cada seis horas

A resposta da vigia do Emacs, e a mesma: ganha uma linha que diz quando olhou, um registro do que
achou e o portão que a mantém fora de uma máquina ocupada. Seis horas pelo motivo dela — cada olhada
é uma linha no registro de atividade —, e é o bastante para o que vigia: um cliente que parou ou um
interruptor que alguém desligou. Uma vigia que rodasse mais de uma vez por hora não compila.

## O que ela diz, e o botão

Um aviso enquanto nada mantém o relógio — com a ação `keep` ao lado (`clock-sync-keep`, também pelo
nome) —, ou enquanto o relógio é mantido e nunca falou com um servidor, sem botão: o que falta ali é
uma rede. O botão é o mesmo trabalho que a vigia faz sozinha, e é o ponto: quem desligou a vigia ainda
ganha um toque, e não uma frase sobre uma preferência.

O toque é feito NA HORA, numa thread própria, e não enfileirado. Enfileirado, ele passava pelo portão
(que segura tudo na bateria) e esperava atrás do que estivesse rodando — uma atualização de duas
horas —, e o rodapé dizia "o cérebro aceitou" sobre um botão que não fazia nada por horas. Na thread do
soquete também não: ligar o relógio são duas chamadas no barramento do sistema (o `timedated` pode ter
de subir antes), e um cliente espera o soquete por meio segundo no máximo. Então o toque espera a thread
por um instante (`PATIENCE`): a tempo, a resposta é dele — feito, ou recusado na frase do módulo
(`clock-sync-why-refused`); atrasada, o toque foi aceito e a resposta chega onde uma resposta atrasada
se lê — a linha da faixa diz que foi recusado, e o registro de atividade guarda as palavras da máquina.
Uma rodada agendada recusada é uma rodada que FALHOU, com as palavras da máquina; antes ela ficava
registrada como "funcionou", com a linha no ar e motivo nenhum. Um ensaio olha e não liga nada; um
cérebro que só ensaia recusa o toque.

Uma máquina sem cliente de hora instalado não diz nada: nada aqui pode iniciar um cliente que não
existe, e a linha não teria botão e nunca sairia — uma frase permanente num rodapé cujo trabalho é
ficar vazio. O que essa máquina é continua escrito na tela de data e hora, onde alguém olhando o
relógio vai ler. O check do módulo confere que a distro instala o cliente e que a regra do polkit
deixa a pessoa ligá-lo sem pergunta.

## Um interruptor, duas portas

A linha "Vigiado pelo cérebro" de `LEADER s l` é o interruptor DESTE módulo (é por isso que o
`locale.rs` está no `namedBy`): ela lê o `enabled` da linha dele no pensamento do cérebro e o muda com
`Ask::Set { Knob::Enabled }`, o mesmo pedido da página do módulo. Um dono da verdade, atrás do
cérebro, e duas portas para ela. Antes ela mandava as preferências inteiras (`Ask::Tune`), montadas a
partir do padrão comum, e desligar e religar a vigia a devolvia semanal, com um minuto de contagem.
