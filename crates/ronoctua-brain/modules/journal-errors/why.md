# journal-errors

Um erro escrito no journal é o sistema dizendo que algo deu errado — e ninguém lê o journal. Este
módulo lê por você: todo registro em `err` ou pior, do journal do sistema e do seu, agrupado, e uma
marca na modeline enquanto houver algo que você não viu.

## Desde quando

Desde que começou a vigiar. O seguidor do núcleo guarda o cursor, então o que aconteceu com o
cérebro desligado é ouvido na volta; na PRIMEIRA partida, sem cursor, a passada única do núcleo
sobre o passado lhe entrega os últimos 200 desta inicialização — o bastante para a primeira página
não nascer vazia, sem ler o journal inteiro.

## Um tipo é uma unidade e um modelo

`nginx.service` dizendo `bind() to 0.0.0.0:80 failed` às nove e `…:443 failed` às dez é uma coisa
dando errado duas vezes. Os números e os ids de uma mensagem são dobrados (um pid, uma porta, um
endereço, um uuid, um id hexadecimal) e a unidade mais a mensagem dobrada são o tipo; a linha mostra
a ÚLTIMA mensagem como foi escrita, com quantas vezes aconteceu. Quem disse é a unidade de que o
registro fala (`USER_UNIT`, `UNIT`), senão a que o escreveu, senão o nome do programa; um escopo
(`session-2.scope`) leva o número de um login, então um programa num escopo é chamado pelo próprio
nome.

A lista guarda até cem tipos; passando disso, sai o ouvido há mais tempo, um visto antes de um não
visto.

## Visto, ignorado

Cada linha da página faz UMA coisa (a ação da linha, o `RET`): um erro ainda não visto é marcado
visto; um visto tem o tipo ignorado dali em diante; um tipo ignorado volta a ser ouvido. `Marcar
todos os erros como vistos` é da página. Os quatro também são comandos, pelo nome; os três de linha,
chamados pelo nome sem linha, pedem que se escolha uma na página.

Duas coisas ainda faltam e dependem do buffer do cérebro, que dá uma ação por linha e mostra no
rodapé o rótulo e a nota, não o valor: ignorar um tipo ainda não visto pede dois passos (marcar visto,
depois ignorar na seção dos vistos), e a mensagem longa, cortada na linha, só se lê inteira no
journal. As duas estão no `docs/PENDING.md`.

Um tipo visto que acontece de novo é novidade de novo. Um tipo ignorado continua contado (a página
diz quantas vezes desde então) e nunca volta à marca. O que se viu numa inicialização anterior é
esquecido na partida seguinte: foi lido e acabou; o que ainda não foi visto de uma inicialização
anterior fica numa seção própria.

A lista é estado (`errors.json`, no diretório de estado do módulo), e com ela vai quantas vezes cada
tipo ignorado foi ouvido desde então; o que se ignora é escolha da pessoa (`ignored.json`, no de
configuração, pelo `ronoctua-store`) e só muda quando ela age. Antes de escrever, o arquivo dela é
relido com `parsed`: se não for lido, fica como está, o log diz por quê e ignorar ou voltar a ouvir é
recusado com uma frase — nunca um arquivo quebrado vira uma lista vazia gravada por cima. O que se
escreve é o arquivo como está AGORA com o único tipo que mudou, então o que a pessoa acrescentou ou
tirou à mão continua. Uma rajada de erros vira uma escrita da lista alguns segundos
depois — o relógio só fica armado enquanto algo espera para ser escrito —, e um gesto da pessoa é
escrito na hora, para um cérebro parado um segundo depois não trazer a marca de volta.

## A marca

Enquanto houver um tipo não visto, uma marca na modeline com quantos tipos, na cor de uma vigia (a
do glifo dos gestos); em repouso, nenhuma (inv. 14). Ela fica até tudo ser marcado visto. O clique
abre a página do módulo.

Um journal que o cérebro não consegue ler é dito na página, com as palavras do núcleo, em vez de uma
lista vazia que pareceria uma máquina sem erros.
