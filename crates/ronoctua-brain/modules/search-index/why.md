# search-index

O cérebro liga o super-índice do fast-access (`ronoctua-index.service`) quando ele não está
rodando, e a modeline diz isso só enquanto o índice é montado do zero. O trabalho de indexar é da
unidade (`ronoctua-index serve`, declarada em `nix/modules/index.nix`): ela reconcilia com o disco e
depois acompanha os eventos do kernel enquanto a sessão durar. O que está aqui é o QUANDO, a marca e
a página.

## Por que o cérebro e não a sessão

A unidade não tem `wantedBy` de propósito. A primeira passada por uma pasta pessoal são minutos de
disco e de processador, e começar isso no login, na bateria, no meio de uma chamada é exatamente o
que o portão do cérebro (memória, processador, bateria) existe para segurar. Então o cérebro a liga
quando ela não está rodando, por dentro do portão; depois disso a própria unidade cuida de si
(`Restart=on-failure`), e uma partida que acabou de falhar só é tentada de novo meia hora depois.

## Como ele sabe

A unidade escreve `status.json` ao lado do índice quando muda de fase — percorrendo, desde a
partida; acompanhando, depois que a passada gravou — e depois de cada gravação. O cérebro ouve a
pasta (inotify, nunca relógio) e lê o arquivo; um status cujo processo já não é a unidade não é
unidade rodando (um pid se recicla, então o nome do processo também é conferido).

## Uma marca, só enquanto monta do zero

A marca existe enquanto a unidade percorre um índice que começou VAZIO: a primeira passada, ou um
índice de outro formato que foi apagado. Acompanhar mudanças depois disso são segundos por dia e
não merece marca (invariante 14). Enquanto a marca existe, o módulo pede para acordar a cada cinco
minutos — uma unidade morta no meio da passada não escreve mais nada, e a marca não pode sobreviver
a ela —; fora disso, nenhum relógio.

## A página

Diz onde o índice está (o estado, quantos documentos, quanto pesa no disco, a última gravação) e o
que ele lê (as pastas, se o que os arquivos dizem é lido, os ocultos, o que fica de fora). Cada
linha do que ele lê muda o `index.json` da pessoa, mesclado sobre o que o arquivo diz agora; um
arquivo que não se lê fica como está e a página diz isso. A unidade vigia o arquivo e percorre de
novo.
