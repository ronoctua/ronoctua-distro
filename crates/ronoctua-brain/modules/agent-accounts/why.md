# agent-accounts

Cada conta de LLM da pessoa (Claude Code ou Codex, as de `llm-accounts.json`) tem uma marca na
modeline, também em repouso: um robô (o do Claude Code é o simples, o do Codex o industrial, sem
marca registrada) com o lugar da conta na lista como dígito, na cor de quanto dos limites dela foi
usado. Um clique abre a vista das contas (`LEADER m a`) naquela conta. O dono aprovou estas duas
marcas em repouso, uma por uma (`assets/brain-resting-marks.nix` e o `AGENTS.md`): a conta está lá
enquanto está calma, e a cor diz quando deixa de estar.

## A cor

A cor segue a janela MAIS usada da conta que ainda não reiniciou: as cinco horas e a semana que a
statusline do Claude Code guarda (ou que o Codex escreve nos logs dele), e as semanas por modelo que
o endpoint de uso respondeu por último. Duas leituras da mesma janela são dois momentos dela, e vale
a maior: uma janela só enche até reiniciar. Aviso a partir de 80 %, perigo a partir de 95 % — as duas
linhas são da pessoa, digitadas na página do módulo ou na vista das contas, e as duas portas
perguntam à mesma função (`lines`, `tone_of`): a vista pinta as barras com as cores que a marca
veste. Uma janela sem porcentagem não é 0 %: é "ainda não se sabe", e a marca fica calma.

O dígito é o lugar da conta na lista inteira, e não dentro do tipo: o 3 da modeline é a terceira
conta da vista, e o clique entrega esse número ao `agents-open`. Um número por tipo seria ambíguo
num comando só (o 1 do Claude Code e o 1 do Codex). Por isso são nove no máximo, um dígito; da
décima em diante não há marca, e a página diz. Na modeline as instâncias de uma marca ficam juntas
(a regra do núcleo, `brain::from_the_edge`): com Claude Code em 1 e 3 e Codex em 2, lê-se o robô do
Codex com 2 e, ao lado, os do Claude Code com 1 e 3 — o dígito, e não a posição, é o que nomeia a
conta.

## Ouvir sem relógio

O núcleo lê as fontes de um vigia UMA vez, quando a thread dele sobe, e as contas são da pessoa para
acrescentar a qualquer momento. Então o núcleo vigia os dois arquivos cujo caminho não muda — a
lista e as linhas — e o módulo mantém três vigias próprias (`ronoctua_files::notice`, inotify)
sobre o que cada conta tem, levantadas de novo sempre que o que elas devem cobrir muda:

- **os números** — as pastas em que o `ronoctua-llm` guarda os limites, a última resposta do
  endpoint e a contagem: uma mudança ali é lida de novo, e só;
- **o uso** — as árvores de log dos agentes (os transcripts do Claude Code, as sessões do Codex),
  inteiras: uma mudança ali é a conta sendo USADA;
- **as pastas das contas**, sozinhas: ali só uma árvore de log NASCENDO é uso — e daí em diante ela
  é vigiada inteira. O resto que muda na pasta não é: o `settings.json` que a vista grava ao pôr ou
  devolver a statusline, ao remover a conta, não pode iniciar a unidade sem ninguém usar a conta.

Separadas porque a distro escreve nas duas primeiras pastas: a unidade de uso grava os números, e a
vista grava o `settings.json`. Contada como uso, a escrita da unidade a dispararia de novo para
sempre, e a da vista perguntaria ao endpoint sem uso nenhum. As duas pastas que o núcleo vigia (a
do módulo, com as linhas, e a da lista) são criadas quando o módulo nasce, antes de o núcleo
levantar as vigias: o núcleo vigia um arquivo pela pasta, e um caminho sem pasta fica sem vigia para
sempre. Se a vigia das linhas cai mesmo assim, a página diz, e relê quando ela volta.

Três threads leem (a do núcleo e duas das vigias do módulo), então uma leitura e a guarda dela
acontecem sob uma trava só: uma leitura lenta (o fim dos logs do Codex) nunca é guardada depois de
uma mais nova, devolvendo números velhos à marca. Os caminhos vêm todos do `ronoctua_llm::watched`; o
módulo não escreve o nome de pasta nenhuma (cobra `one-llm-boundary`). As pastas dos números são
criadas antes de vigiadas (uma pasta que não existe nunca é ouvida); as dos agentes são deles, e
nunca são criadas aqui.

## A unidade, só depois de uso

`ronoctua-llm-usage.service` roda `ronoctua-llm refresh`: conta os transcripts desde onde a última
contagem parou e, para uma conta do Claude Code cujo host a pessoa permitiu, pergunta ao endpoint de
uso (não oficial; a vista diz). O cérebro a inicia só depois que uma conta foi usada desde o último
início, e nunca antes de dez minutos depois dele (`AGAIN_AFTER_S`): sem uso, nada roda; usando sem
parar, uma contagem a cada dez minutos. Fora do portão geral: é uma contagem incremental e, no
máximo, um pedido pequeno, e quem trabalha na bateria é justamente quem quer os números. A rede não
é condição (`needs.network = false`): sem rede a contagem acontece, e a recusa do pedido fica ao lado
dos últimos números.

O único relógio é o reinício mais próximo de uma janela mostrada (`next_wake`): a marca vermelha
porque as cinco horas acabaram volta a ficar calma quando elas recomeçam, e a página muda de geração
nesse momento — ela mostra quanto a janela estava cheia, e o hospedeiro só a pede de novo quando a
geração anda. Sem janela conhecida, não há relógio nenhum.

O cérebro depende do `ronoctua-llm` sem a feature `fetch`: o módulo só lê o que foi guardado, então
o daemon não leva cliente TLS, passe de consentimento nem leitor de credenciais. Quem pergunta à rede
é a unidade.

## As linhas, gravadas por três

A página do módulo, a região do rodapé e a janela gravam o mesmo `lines.json`. Cada uma muda UMA
linha sobre o que o arquivo diz na hora (`keep_line`, sobre o `Store::parsed`), nunca sobre a cópia
que leu antes: senão a janela, aberta com 80/95, devolveria o perigo a 95 depois que o rodapé o
pusesse em 90. Um arquivo que não lê é recusado com frase e fica como está.

## O que foi recusado

- **Um temporizador da unidade**: rodaria com a máquina parada, e pediria à rede sem ninguém usar a
  conta.
- **Ler os números no `face`**: o `face` é puro sobre o estado do módulo; ler o log do Codex (até
  alguns megabytes do fim de três arquivos) é trabalho da vigia, na thread dela.
- **Uma fonte por conta no núcleo**: as fontes são lidas uma vez, e uma conta acrescentada depois
  ficaria sem vigia até o cérebro reiniciar.
