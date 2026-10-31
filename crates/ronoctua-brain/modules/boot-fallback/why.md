# boot-fallback

O ensaio de boot é da máquina (`ronoctua-machine`, `nix/modules/machine.nix`): um `apply` o arma,
o próximo boot tenta o sistema novo uma vez, e um minuto depois de o boot chegar ao alvo padrão o
`ronoctua-boot-bless.service` lê qual sistema subiu. Quando não foi o novo, a máquina já voltou
sozinha ao último sistema bom — e a pessoa não estava olhando: o boot que falhou foi um pânico ou um
prazo estourado, e o que voltou parece igual a qualquer outro. O rollback do console de resgate
(`ronoctua-machine rollback`) deixa o ensaio do mesmo jeito, porque desarma o que deixa para trás, e
aí foi a pessoa que voltou. Este módulo só diz isso, uma vez, no minibuffer.

## Um vigia, e nenhum relógio

Ele não roda nada (`schedule = null`). Escuta, por inotify, a pasta do registro da máquina
(`ronoctua_machine::STATE_DIR`, `0755`: a pessoa lê), que o bless reescreve inteira com um
`rename`, e lê o `state.json` pelo crate que o escreve. Sem relógio: o bless acontece uma vez por
boot, e é ele que move a pasta.

## Um evento, e não uma marca

Não há o que a pessoa fazer: a máquina já voltou, e a próxima atualização só tenta de novo quando a
distro ou o nixpkgs que a máquina rola andarem e montarem outro sistema (o bless devolveu à pasta o
lock do último bom, e o `apply` recusa construir de novo o sistema que falhou, `did-not-boot`). Uma
marca na modeline diria uma coisa que não pede gesto, e a modeline em repouso é muda (inv. 14). Um
evento é uma linha no minibuffer, que vira notificação quando a linha está ocupada.

## Uma vez por ensaio

O registro continua dizendo `failed` até o próximo `apply` armar outro ensaio, e o cérebro começa
de novo a cada login. Então o que foi dito fica guardado na pasta de estado do módulo (o toplevel
do ensaio que falhou): uma falha é dita na primeira vez que é vista — no bless, ou no login
seguinte, se ninguém estava logado — e nunca mais. Um ensaio só pendente não é notícia.
