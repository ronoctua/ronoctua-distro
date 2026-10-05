# trash-expire

A lixeira é esvaziada do que está nela há tempo suficiente. O trabalho de apagar é do
`ronoctua-empty-trash` (binário do `ronoctua-files`, ao lado da lixeira do gerenciador de arquivos,
cujas regras ele divide); o que está aqui é o QUANDO.

## Não é a varredura do cache, e nunca pode virar

Porque nada na lixeira pode ser refeito. A varredura apaga o que um programa reconstrói e nunca chega
perto do `$XDG_DATA_HOME`; esta apaga a única cópia. A mesma maquinaria, e números deliberadamente
diferentes: quatro dias e não quarenta, e a idade lida do `DeletionDate` que a especificação da
freedesktop escreve em toda nota, nunca adivinhada pelas datas do próprio arquivo — um documento
escrito há um ano e jogado fora hoje de manhã está na lixeira há uma manhã. O único caminho gravável
da unidade é `%S`, e não `%C`: uma varredura que alcançasse a lixeira seria um limpador de cache
apagando a única cópia do arquivo de alguém.

As duas moravam num arquivo só porque são a mesma pergunta feita a dois diretórios — o que a máquina
pode jogar fora sozinha, e depois de quanto tempo. Agora cada uma é um módulo, e dá para desligar uma
sem a outra.

## O resto é a forma da varredura, pelas razões dela

Uma unidade, para a thread do runner ficar livre e as remoções irem em prioridade ociosa; o portão da
pessoa decide se a máquina está ocupada, e o módulo não tem `needs` próprio. Duas unidades, para o
`m e` perguntar "quanto isto levaria?" de algo que não se desfaz. Sem contagem regressiva, e paciência
de minutos: uma lixeira são algumas notas e o que elas nomeiam.

A cadência (`trashEveryDays`) é igual ao número de dias (`trashDays`), ao contrário da do cache: um
arquivo sai assim que pode, e uma cadência mais longa só faria as coisas viverem até o dobro do
número que a pessoa lê. Os dois vêm de `assets/cache.nix`; o número que decide algo irreversível é
pequeno de propósito.

Desistir é o que a para, e aqui importa mais que em qualquer lugar: é a única coisa da árvore que
apaga o que não se refaz, então uma execução que continuasse depois de a pessoa mandar parar seria
arquivo perdido com a permissão retirada.
