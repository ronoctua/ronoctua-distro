# video-metrics

A cada seis horas o cérebro manda varrer os canais do YouTube que a pessoa acrescentou ao Métricas de
vídeo (`LEADER v`); enquanto a varredura roda, a modeline diz isso. O trabalho de perguntar é do
`ronoctua-video-metrics scan` (o crate do app, a unidade em `nix/modules/video-metrics.nix`); o que
está aqui é o QUANDO, a marca e a página.

## Por que o cérebro e não um timer

Uma varredura são minutos de perguntas lentas de propósito — os feeds, uma aba plana quando um feed
esconde atraso, os detalhes de uns poucos vídeos com quatro a seis segundos entre eles, as
miniaturas —, e ela precisa esperar o que um timer do systemd não sabe esperar: a rede e uma máquina
quieta. O cérebro já tem a fila, o portão (memória, processador, bateria), o diário e a marca; um
timer seria uma segunda agenda, sem nada disso. A cadência é de seis horas e não um calendário:
vídeo novo não tem hora boa, tem intervalo.

## A rede espera; a economia de energia pula

Sem saída para a internet — o NetworkManager diz que não há rota, ou que há um portal, ou que a
saída é limitada — a varredura fica RETIDA (`Hold::Network`) e começa assim que a rede volta. Um
daemon que não respondeu não é um "não": a varredura começa, e se não houver rede ela mesma para e
registra `offline`. Com a economia de energia ligada a rodada é PULADA e diz por quê
(`brain-why-saver`): é a pessoa pedindo para gastar menos, e a próxima vem seis horas depois. O
"Varrer agora" da porta diz isso antes de pedir: uma rodada pedida e pulada seria uma frase dizendo
que pediu enquanto nada roda. Um cérebro que só ensaia (o do `.#dev`) não varre nada: não existe
varredura que não pergunte ao YouTube — e a porta de uma segunda instância não pede ao systemd, cuja
unidade é a da sessão instalada.

## A marca: enquanto varre, e vermelha quando falhou em todos

Enquanto uma varredura roda, a modeline tem uma marca na cor de quem vigia — começada por este
cérebro, pelo "Varrer agora" da porta (que pede ao cérebro e, sem cérebro, ao systemd) ou por quem
iniciou a unidade à mão: a unidade sempre escreve o relatório onde o cérebro lê
(`%t/ronoctua-brain/video-metrics.json`), e o módulo vigia essa pasta. Um relatório "começado" só
vale enquanto a varredura SEGURA o catálogo (a trava dela, que morre com o processo): o relatório
de uma varredura morta pelo OOM, lido por um cérebro que subiu depois, não acende marca nenhuma — e a
unidade, no `ExecStopPost`, escreve o fim que a varredura morta não escreveu. Depois, quem decide é o
registro do próprio catálogo: uma varredura em que TODOS os canais falharam deixa uma marca na cor
de perigo até uma em que algum responda. Um canal que falhou sozinho é assunto da página do canal,
não da modeline. Em repouso não há marca (inv. 14).

## A página

A última varredura como o catálogo a registrou: quando começou e terminou, quantos canais foram
perguntados, quantos falharam (vermelho quando foram todos) e quantos vídeos entraram. Nada varrido
ainda é uma frase, não uma página de zeros.
