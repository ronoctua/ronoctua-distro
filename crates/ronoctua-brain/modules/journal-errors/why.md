# journal-errors

Um erro escrito no journal é o sistema dizendo que algo deu errado — e ninguém lê o journal. Este
módulo lê por você: todo registro em `err` ou pior, do journal do sistema e do seu, agrupado, os
avisos numa lista à parte, e uma marca na modeline enquanto houver algo que você não viu.

## Desde quando

Desde o começo desta inicialização, ou de onde parou. Ligado, ele ouve o seguidor do núcleo, que
guarda o cursor, então o que aconteceu com o cérebro desligado é ouvido na volta. Na PRIMEIRA
partida do cérebro, sem cursor, a passada única do núcleo sobre o passado entrega os últimos 200
erros e os últimos 100 avisos desta inicialização aos módulos ligados nela — o bastante para a
primeira página não nascer vazia, sem ler o journal inteiro.

## Leitura à mão

O módulo vem desligado, e o cursor é de todos: ele anda enquanto o módulo está desligado, e o que
passou não volta por ali. Por isso o módulo guarda o próprio lugar, a última linha que leu, e o
núcleo lê para ele, numa passada só dele, o que veio depois: sozinho quando o módulo liga, e em
`Rodar agora` enquanto ele fica desligado. A opção só existe com ele desligado, porque ligado ele
ouve tudo na hora. Sem lugar nenhum, porque nunca leu nada, a passada lê esta inicialização. Ela não
toca o cursor de todos, e uma linha que ela e o seguidor leem é ouvida uma vez só.

Só uma linha move o lugar. Uma passada vazia ou que falhou não diz onde o journal está, e apagar o
lugar por causa dela faria a leitura seguinte recontar a inicialização inteira. Enquanto a passada
lê, quem segura o lugar é o seguidor, e ela só o move se nada chegou depois que ela começou, ou se
ela leu tudo o que chegou e passou dali: sem isso, ligar e desligar de novo leria as mesmas linhas
outra vez.

Uma passada pedida fica devida até terminar, e isso vai no arquivo. Se o cérebro para no meio dela,
na partida seguinte ele a lê de novo, do mesmo lugar até a primeira linha que o seguidor trouxe
enquanto ela lia: dali em diante já foi ouvido, ou o seguidor traz de novo. Sem isso, o trecho que
só ela lia se perderia calado.

Cada fonte guarda até 1000 linhas da passada, as mais novas, e a página diz quando havia mais. Uma
passada interrompida conta o que leu, continua dali na próxima e diz por que parou.

## Um tipo é uma unidade e um modelo

`nginx.service` dizendo `bind() to 0.0.0.0:80 failed` às nove e `…:443 failed` às dez é uma coisa
dando errado duas vezes. Os números e os ids de uma mensagem são dobrados (um pid, uma porta, um
endereço, um uuid, um id hexadecimal, um hash do Nix, uma rev curta, um nome do `mktemp`) e a
unidade mais a mensagem dobrada são o tipo; a linha mostra a ÚLTIMA mensagem como foi escrita, com
quantas vezes aconteceu. Quem disse é a unidade de que o registro fala (`USER_UNIT`, `UNIT`), senão a
que o escreveu, senão o nome do programa; um escopo (`session-2.scope`) leva o número de um login,
então um programa num escopo é chamado pelo próprio nome.

Só a primeira linha conta: o que uma queda escreve embaixo é uma pilha, diferente a cada vez. Se a
mensagem nem chega (o `journalctl` manda `null` acima de 4096 bytes), o tipo sai do programa e do
sinal. A instância de uma unidade só de números também dobra, senão cada queda seria um tipo com o
PID no nome (`systemd-coredump@#.service`). E uma unidade que falha escreve duas linhas, `Failed with
result` e `Failed to start`: pelo `MESSAGE_ID`, as duas são um tipo só, porque quem lê vê uma falha.

Um arquivo escrito antes de um dobramento mudar é relido pelo dobramento de hoje, então o que você
ignorou continua ignorado, e duas linhas antigas de uma queda viram uma. Uma palavra que o
dobramento anterior cortou em pedaços, dobrando cada número sozinho (um hash do Nix, uma rev curta,
um nome do `mktemp`), não se refaz pelo modelo: refaz-se pela mensagem guardada junto, quando ela,
dobrada do jeito antigo, dá o mesmo modelo. Uma falha de unidade num
arquivo antigo é reconhecida pelas palavras do systemd, porque ali não ficou o `MESSAGE_ID`.

A lista guarda até cem tipos; passando disso, sai o ouvido há mais tempo, um visto antes de um não
visto, um mais leve antes de um mais grave.

## Gravidade

Quatro graus, cada um com a sua cor e com a palavra no rodapé da linha, porque só cor não alcança
todo olho: crítico (uma queda, ou prioridade 0 a 2), erro (prioridade 3), merece atenção e aviso.
Uma senha digitada errada, que o `sudo` escreve com prioridade de crítico, merece atenção: foi
alguém errando a senha, e não algo quebrado.
Fica fora do tipo: um tipo que piora é o mesmo tipo, com o mesmo id, e a linha diz o pior desde a
última vez que foi vista.

Os campos vêm primeiro, porque campo é o que o programa quis dizer: `MESSAGE_ID` de coredump,
`UNIT_RESULT`, `JOB_RESULT`. Só um aviso sem campo nenhum é lido como texto, e a tabela do kernel vale
só para linha do kernel: `fatal I/O error` no X11 é um cliente sem display, não um disco.

## Avisos

Um aviso comum fica numa lista à parte, fechada, com até quarenta tipos e fora da marca. À parte
porque warning é o que o journal mais diz: numa lista só, uma hora barulhenta expulsaria erros que
ninguém leu. Por isso são dois pedidos ao núcleo, cada um com o seu anel, e erro nenhum depende de
sobrar lugar entre avisos.

O aviso que diz que algo terminou mal, uma unidade que falhou, um disco com problema, não é comum:
vai para a lista e conta na marca. E um tipo que sobe leva o que já tinha contado. Um aviso comum de
um tipo que já está na lista conta na linha dele e não acende a marca. A seção mostra
quantos tipos guarda, e os de outra inicialização somem na partida seguinte, porque ninguém pediu
para ler aquilo.

Uma linha que não mexe na contagem da marca (um aviso, uma linha filtrada, um tipo ignorado) chega à
página no máximo uma vez a cada 30 segundos, e os números da linha dela acompanham nesse ritmo; um
erro chega na hora, dentro do meio segundo que o núcleo deixa entre uma publicação e outra. São elas
o grosso do journal, e a marca não tem nada a dizer sobre elas. A lista é escrita alguns segundos
depois (5 para um erro, os 30 para as outras), mas o arquivo não espera esse tempo quando o cursor
anda: ele vai à frente do cursor do journal, no ritmo do cursor, porque um cursor adiante da lista
perderia a linha num reinício.

## Visto, ignorado

Cada linha da página faz UMA coisa (a ação da linha, o `RET`) e pode oferecer outra na própria tecla:
um erro ainda não visto é marcado visto, ou tem o tipo ignorado; um visto, ou um aviso, tem o tipo
ignorado, ou vira um filtro a partir da própria linha; um tipo ignorado volta a ser ouvido. `Marcar
todos os erros como vistos` é da página. Todas também são comandos, pelo nome; as de linha, chamadas
pelo nome sem linha, pedem que se escolha uma na página.

Com o cursor numa linha, o rodapé diz a gravidade, a primeira vez, quantas vezes nesta inicialização
e a mensagem inteira, que a linha corta.

Um tipo visto que acontece de novo é novidade de novo, salvo num aviso comum, que só conta na
linha dele. Um tipo ignorado continua contado (a página
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
escrito na hora, para um cérebro parado um segundo depois não trazer a marca de volta. A lista também
é escrita antes do cursor do journal, para ele nunca dizer que uma linha foi ouvida sem ela estar na
lista, e ao desligar, porque módulo desligado não acorda.

## Filtrados

Um erro pode ser ruído. Ignorar tipo a tipo seriam dezenas de toques, e um nome novo voltaria. Um
filtro é um trecho de texto: toda linha cujo `<unidade>: <mensagem>` o contém, com maiúsculas e
minúsculas dobradas, é ouvida e contada no filtro e nunca vira um tipo — fora da lista e fora da
marca. Vale só para a primeira linha da mensagem: uma palavra num quadro da pilha não esconde uma
queda.

Criar um filtro é a ação `filter` da página, que pede um trecho, ou `Filtrar por esta linha`, que
começa da própria linha para você cortar até sobrar o que importa. A linha de um filtro o desliga
(`unfilter`) ou o religa (`refilter`); o seu próprio sai da página ao ser desligado, um de fábrica
fica, desligado, para religar. Ligar ou religar varre também o que já foi ouvido: os tipos que o
trecho pega saem da lista e dos avisos na hora, a contagem deles nesta inicialização somada ao
filtro. Desligar não é retroativo — o que já foi escondido acabou, e uma linha daquele tipo é
novidade de novo.

Cinquenta e nove trechos vêm ligados de fábrica, como o módulo vem desligado (o `starts_off` do
manifesto): estão na página desde a primeira partida e um toque desliga cada um. É um primeiro
rascunho da sua lista, não uma regra sobre o que importa, e cada trecho é longo o bastante para
nomear uma linha, porque um curto engoliria a próxima falha que dividisse duas palavras com ele. Uma
falha de verdade nunca entra, e os testes seguram a lista contra uma tabela delas. Até cinquenta
trechos seus, fora os de fábrica; passando disso, sai o que você pôs há mais tempo.

Todo filtro pode dizer por que existe, embaixo do nome e no rodapé. Os de fábrica já vêm com a
frase, e a outra tecla da linha pede o seu porquê, já preenchido com o que você escreveu; vazio
limpa, e um de fábrica volta à frase dele.

O contador é só desta inicialização. Um número que só cresce não diz nada depois de um mês (chegou a
3536), então o que a lista guarda vale para o boot em que foi escrito, sobrevive a um reinício do
cérebro nesse boot e zera no seguinte. Uma linha de outra inicialização, que um cursor antigo traga,
é escondida do mesmo jeito e não entra na conta. O título da seção mostra o total.

O que a lista guarda de cada filtro — quantas linhas pegou e a última — é estado (`errors.json`), com
ela; o que você filtra é sua escolha (`filters.json`, no diretório de configuração, pelo
`ronoctua-store`) e só muda quando você age, relido com `parsed` antes de escrever como o dos tipos
ignorados. Um trecho de fábrica deixado como está não vai no seu arquivo: fica ligado por ser de
fábrica, e só o que você desligou, justificou ou acrescentou é escrito.

## A marca

Enquanto houver um tipo não visto, uma marca na modeline com quantos tipos, na cor de uma vigia (a
do glifo dos gestos), ou na de crítico enquanto um deles for crítico; em repouso, nenhuma (inv. 14).
Os avisos comuns nunca contam nela. Ela fica até tudo ser marcado visto. O clique abre a página do
módulo. O glifo da caveira é só dela: na lista de tarefas o módulo continua com o olho das vigias.

Um journal que o cérebro não consegue ler é dito na página, com as palavras do núcleo, em vez de uma
lista vazia que pareceria uma máquina sem erros.
