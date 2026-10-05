# reminders

Alarmes e lembretes pedidos com as palavras da pessoa: uma vez ("amanhã 9:00 ligar para a mãe",
"daqui a 2 h com som tirar a pizza"), todo dia, em dias da semana, num dia do mês ou numa data de
todo ano. Na hora, o aviso de nível `alarm` toma a faixa do cérebro — na frente da contagem
regressiva — e só sai quando alguém dispensa ou adia. É o primeiro módulo de execução única: um
lembrete de uma vez toca, fica escrito na atividade e sai da lista.

## Um relógio só, armado para o próximo

Não há temporizador por lembrete nem varredura periódica. O módulo é uma vigia (`schedule = null`) e
responde `next_wake` com o mais próximo dos momentos que ele guarda — o próximo lembrete, o fim de
um adiamento, o próximo toque do sino de um que está tocando — e com nada quando não há nenhum; aí a
thread de vigia dorme sem relógio. Uma ação que muda a lista toca o sino do núcleo, e a thread
pergunta de novo.

O momento é do RELÓGIO DE PAREDE. Um `recv_timeout` conta no relógio monotônico, que para durante a
suspensão, e um alarme das 8:00 armado às 7:00 num notebook que dormiu das 7:10 às 9:00 tocaria às
9:50. A thread de vigia limita a espera a 30 s enquanto há relógio armado e recalcula contra a
parede, então o que venceu durante o sono é visto meio minuto depois de a máquina acordar.

## Tocar, perder, adiar

- O que vence toca: fica marcado tocando desde aquele instante; o que se repete já anda para a
  próxima vez, o de uma vez espera a resposta e então sai.
- O que venceu ANTES de o cérebro começar a vigiar (`started`, ou a primeira olhada) e há mais de
  dez minutos — a máquina estava desligada, ou o módulo — é PERDIDO: não toca (um alarme horas
  atrasado não é o alarme pedido), vai para o aviso `missed` (nível `warn`, dispensável) e para a
  atividade, e o de uma vez sai. Três dias desligada contam como UM perdido por lembrete, não um
  por dia.
- O que venceu com o cérebro VIVO toca, por mais tarde que seja visto: é uma máquina que dormiu, e
  o alarme sobrevive à suspensão — toca quando a tampa abre, com o sino. Um cérebro que atravessou
  a suspensão continua o mesmo processo, e é isso que separa "dormindo" de "desligada".
- O adiado toca de novo quando o tempo acaba, por mais tarde que seja: a pessoa já o viu.

Os adiamentos declarados são 5, 10 e 60 minutos. A faixa oferece o primeiro (`, z`); a página do
módulo oferece os três, numa linha cada, sob cada lembrete que toca — clicáveis, como tudo.

## A hora da pessoa, dos dois lados da troca de horário

Um lembrete guarda o MINUTO LOCAL do dia e a regra (uma vez, dia, semana com os dias, mês com o
dia, ano com a data), e o instante é derivado dia a dia pelo fuso daquele dia — a mesma técnica de
duas passadas do `crate::moment`. Guardado como "o último mais 24 h", um lembrete diário tocaria às
8 metade do ano. O minuto que a primavera pula toca quando o relógio passa por ele (02:30 vira
03:30, uma vez); o que o outono repete toca uma vez, porque a próxima vez é sempre procurada
estritamente depois da última. O dia 31 num mês de 30 toca no último dia; o 29 de fevereiro, no dia
28 dos anos sem ele. A aritmética de calendário é a do `ronoctua_files::when`, a única cópia dela na
árvore.

## As palavras são das línguas

O leitor da resposta (`grammar.rs`) sabe só as FORMAS — número, dois-pontos, barra, e que classe de
palavra pode vir depois de qual. As palavras ("amanhã", "todo", "segunda", "com som") são chaves
`reminders-grammar-*` dos `.ftl` deste módulo, uma linha por classe, as alternativas separadas por
vírgula, viradas em tabela pelo `tables.nix` com o leitor do `gen-i18n.py`. Assim uma terceira
língua é um terceiro arquivo e nada em Rust, e o português mora onde o inv. 18 deixa. Cada língua lê
a resposta sozinha ("a" é "one" em inglês e "às" em português, e um vocabulário misturado teria de
escolher um), e VENCE A LEITURA CUJO "QUANDO" TOMA MAIS do que foi digitado; no empate, a língua
declarada primeiro. A primeira língua que lesse alguma coisa não serve: o inglês lê "amanhã 9:00"
como "9:00" e deixa "amanhã" de texto, e "daqui a 2 h" como 02:00. O cérebro não sabe a língua da
pessoa, e nem precisa: a leitura mais longa é a da língua em que ela escreveu.

O "quando" pode vir no começo ("amanhã 9:00 ligar para a mãe"), no fim ("ligar para a mãe amanhã
às 9") ou em volta do texto ("9:00 ligar para a mãe amanhã"). Um conector na BORDA do "quando" ("at
the office") fica com o texto; só um conector ENTRE duas partes dele é lido — ou "às"/"at" logo antes
de uma hora, que é da hora ("ligar para a mãe às 10 amanhã"). Uma hora sozinha ("9") só é hora
depois de "às"/"at". E um número que nenhuma regra põe no lugar não é adivinhado: um "quando" sem
hora que deixa um número ao lado ("amanhã 8 dentista", "amanhã 9"), ou uma unidade logo depois do
próprio número ("todo 2 horas"), é recusado com `not-understood` — nunca toca às 9:00 com o número
no texto. Com a hora dita, o número é do texto ("todo dia 7:00 2 comprimidos"). O que sobra é o texto do lembrete, exatamente como foi digitado: DADO,
nunca chave (inv. 8), como a linha do log do cérebro.

## Onde fica

`$XDG_CONFIG_HOME/ronoctua/brain/reminders/reminders.json`, pelo `ronoctua-store`: escrito inteiro
e atômico, lido com `parsed` — um arquivo que não faz parse nunca é sobrescrito; a faixa diz isso (o
aviso `unreadable`, `warn`, que não se dispensa: sai quando o arquivo é consertado), a página
também, e toda mudança é recusada até ele ser consertado. O que foi lido antes de ele quebrar
continua tocando da memória, e dispensar ou adiar funciona nela; só não é escrito. O arquivo é VIGIADO (inotify, pela thread de
vigia): editado à mão, é ouvido e adotado — um lembrete plantado já vencido toca na hora —, e a
própria escrita do módulo é reconhecida (o que o arquivo diz é igual ao que ele tem) e deixada
passar. O que toca também fica no arquivo: um cérebro que reinicia no meio de um alarme volta com
ele tocando, porque um reinício não é resposta.

## O som

Opcional por lembrete ("com som", ou a linha "Som" da página). Toca pelo `ronoctua_audio::chime`: o
`pw-play` do PipeWire pelo CAMINHO NO STORE, com `--media-role Notification`, dentro da fronteira do
áudio (`one-pipewire-boundary` vigia os nomes dos programas). Os dois caminhos chegam ao cérebro
pelo AMBIENTE da unidade (`RONOCTUA_PW_PLAY`, `RONOCTUA_CHIME`, em `module.nix`), nunca compilados:
o binário do cérebro continua sem o PipeWire no fecho. O sino é um WAV gerado no build pelo
`chime.py` — duas notas de um sino pequeno, parciais inarmônicos de barra percutida, ataque suave
e fim em zero — sem download e sem arquivo de som no git. Ele toca ao começar e de novo a cada
minuto enquanto ninguém responde, cinco vezes no máximo: um alarme longe de todos não fica soando
numa sala vazia; ele continua TOCANDO na faixa. Uma máquina sem som diz no log por que ficou calada,
e o alarme aparece igual.

## Recusas

Toda recusa é uma palavra do manifesto com a sua frase: `not-understood` (a frase dá exemplos nas
duas línguas), `passed`, `no-such-day`, `unreadable`, `gone` (a linha fala de um lembrete que não
existe mais), `not-ringing`, `full` (500 lembretes).
