# emacs-daemon

O dono pediu: conferir de tempos em tempos se o servidor do Emacs está de pé, e dizer isso na linha do
cérebro com um jeito de iniciá-lo. E a metade mais difícil: se o Emacs que ESTÁ aberto é o cliente
desse servidor, ou uma segunda cópia do editor que carregou a própria configuração e guarda buffers
que a sessão nunca vai ver.

## Uma vigia agendada, e não uma thread em algum lugar

Tudo o que um módulo agendado ganha de graça é o que ela precisa: uma linha numa tela que diz quando
olhou por último, um registro do que achou e o portão que a mantém fora da máquina enquanto alguém
compila. Uma thread seria um segundo agendador sem nada disso.

## A cadência é em horas, e nunca menos de uma

Seis horas, por escolha do dono, contra os cinco minutos com que ela começou. Cinco minutos foram
defendidos pelo custo da olhada — uma listagem do `/proc` e um `emacsclient` curto — e esse argumento
deixou de fora a única coisa cara que uma vigia faz: ela escreve uma linha no registro de atividade a
cada vez, e o registro da página inicial tem cem linhas. A cinco minutos, ela sozinha o enchia duas
vezes por hora, e o registro respondia "o que esta máquina anda fazendo" com duzentas e oitenta e oito
repetições de "nada". O dono leu exatamente isso e disse que a vigia rodava o tempo todo.

O número é do dono, no manifesto; o que o `module.rs` guarda é a classe de mudança: uma vigia que
rodasse mais de uma vez por hora não compila. O lugar desse argumento é o build, porque um teste
deixaria a árvore verde enquanto ninguém o rodasse.

## O que ela diz, e a única coisa que oferece

Um aviso enquanto o daemon está parado — com o botão `start` ao lado — ou enquanto um Emacs que não é
cliente dele está aberto. Nada quando está tudo em ordem, e nada numa máquina sem Emacs nenhum: um
aviso sobre a ausência de algo que ninguém pediu seria um rodapé falando à toa.

O botão é `systemctl --user start` da unidade da própria distro (`services.emacs.enable`, em
`nix/modules/emacs.nix`). É uma unidade de USUÁRIO, que não passa por polkit e não pergunta nada a
ninguém — é isso que deixa o cérebro oferecer um botão. Ele é uma ação do módulo (`emacs-daemon-start`,
também pelo nome), e o cérebro a responde: o rodapé não inicia nada por conta própria. Um cérebro que
só ensaia (o do `.#dev`) recusa, para não subir o daemon da sessão de verdade. O check do módulo
confere que a unidade existe, é da pessoa e roda o daemon.

O aviso de "este Emacs não é o cliente" não tem botão de propósito: o que está aberto é o trabalho de
alguém, com buffers que nada aqui sabe se foram salvos. A tela diz o que é, e a pessoa decide.
