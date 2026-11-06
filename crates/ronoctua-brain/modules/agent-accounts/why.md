# agent-accounts

Cada conta de LLM da pessoa (Claude Code por assinatura ou numa chave de API, ou Codex, as de
`llm-accounts.json`) tem um conjunto na modeline, também em repouso: primeiro o glifo da conta, depois
um glifo por agente trabalhando nela. O glifo da conta é a marca da empresa relida em pontos
(`assets/icons/marks.nix`): a centelha do Claude Code, o "A" do Claude Code numa chave de API e o anel
do Codex, parecidos, não iguais. A regra antiga de não pôr marca registrada na modeline foi revertida
pelo dono (2026-11-02): o robô que havia por conta dizia "um agente", e não de quem, e o conjunto tem de
dizer de quem é com o primeiro glifo. A cor dele é a de quanto dos limites da conta foi usado. O
conjunto é o lugar da conta na lista, e um clique no glifo da conta abre a vista das contas
(`LEADER / a`) naquela conta. O dono aprovou em repouso os três glifos de conta, o agente pronto e a
contagem dos escondidos, uma marca por vez (`assets/brain-resting-marks.nix` e o `AGENTS.md`): a conta
está lá enquanto está calma, e a cor diz quando deixa de estar.

## Os conjuntos

Os conjuntos ficam à esquerda de todos os glifos do canto direito e se encostam, cada um com um fundo
próprio muito perto do da modeline, os dois tons alternados a partir da borda (`surface.mark-set`,
`surface.mark-set-alternate`). Contados da borda, e não do primeiro conjunto: numa linha estreita os
conjuntos caem da esquerda, e os que ficam têm de manter a cor. O lugar e a hora de cair são duas
respostas (`layout::Place`): o conjunto senta no degrau mais longe da borda, mas cai na prioridade de
uma marca do cérebro, depois dos números dos espaços, do player e dos dispositivos. Só há fundo onde a
modeline tem faixa própria; na página de uma mesa e no rodapé velado o conjunto é só os glifos, como
qualquer outra marca.

Os agentes não levam dígito: o lugar no conjunto é onde estão. Todo agente é uma cabeça de robô
(`md-robot`, pedido do dono, 2026-11-06), o mesmo glifo que os terminais e a vista das contas desenham,
e o tom diz o estado: trabalhando (o teal de algo em andamento — o azul é do foco, e de mais nada),
esperando você (o laranja de atenção), pronto (calmo) e erro (perigo): perigo é cor, não outro desenho.
Um agente recém-aberto e parado é desenhado como pronto.

Cada agente guarda um lugar de nove no conjunto da conta enquanto vive: o menor livre quando aparece
pela primeira vez (`Places`). Assim um glifo nunca troca de agente enquanto a pessoa olha, e um novo
não empurra os velhos. Com mais de nove, o décimo leva o do nono lugar consigo: os oito primeiros são
desenhados, e no lugar do nono fica a contagem dos escondidos (`agent-more`, "⋯ N"), no tom mais alto
entre eles, para que um que espera você continue visto. Agente de uma conta que a lista não tem não é
desenhado.

## De onde vêm os agentes

De duas fontes, juntadas por um leitor só (`ronoctua_llm::agents::merged`), que o módulo, a vista das
contas e o clique chamam pela mesma costura (`agents_now`):

- **o gancho gerenciado** que a distro instala no Claude Code grava um registro por sessão
  (`$XDG_RUNTIME_DIR/<nome>-agents/`): quem ela é (a pasta da conta, o processo e o momento em que ele
  nasceu, o nosso terminal quando roda num), onde trabalha e o estado até onde os eventos contam. Vale
  para um Claude Code em qualquer terminal, o de um editor inclusive;
- **o OSC 7501**, o protocolo em que um programa diz o próprio estado ao terminal: dentro de um dos
  nossos terminais o Claude Code diz trabalhando, bloqueado, pronto, no instante em que muda, e o
  guardião do terminal guarda o registro no arquivo de estado dele (`<nome>-terms/<id>.json`). É a
  palavra do próprio programa, então dentro do nosso terminal ela vence a do gancho; fora dele, ou
  enquanto ele não disse nada, vale a do gancho. O módulo lê o arquivo, e nunca o socket do guardião:
  a vigia do núcleo não pode esperar, e um guardião congelado seguraria um `connect`.

Não é agente uma sessão que terminou, um processo que não é mais o que era (o pid e o momento em que
nasceu: um pid é reaproveitado, e `kill(pid, 0)` responderia sim por um estranho), nem uma sessão cujo
terminal sumiu — o arquivo de estado do guardião some com ele. O núcleo vigia as duas pastas por
inotify (`Source::Path`), criadas privadas (`0700`) antes de vigiadas; uma mudança ali relê só os
agentes, e não os números das contas.

## O clique num agente

O clique num agente entrega a sessão dele ao `agents-show` (a chave da instância, `MarkId::key`, e
não o lugar: um número mudaria de agente entre o desenho e o clique). O comando é da vista das contas,
que relê o agente NA HORA pelo mesmo leitor (`agent_of`) e vai aonde ele está agora: num dos nossos
terminais, o `terminal-show <id>` do terminal, que traz a janela que o mostra, ou restaura o grupo dele
numa; fora deles, o `window-of-process <pid>` do rodapé, que traz a janela que o processo, ou um acima
dele, abriu. Um agente que já terminou é dito numa frase, nunca um clique que não leva a lugar
nenhum. As linhas dos agentes na vista das contas (`LEADER / a`) são o gêmeo de teclado dos glifos: o
mesmo glifo, a mesma cor, e o `RET` faz o que o clique faz.

## A cor

A cor segue a janela MAIS usada da conta que ainda não reiniciou: as janelas que o plano tem — cinco
horas e uma semana, ou só a semana (o Ultra do Claude) — que a statusline do Claude Code guarda (ou
que o Codex escreve nos logs dele), e as semanas por modelo que
o endpoint de uso respondeu por último. Duas leituras da mesma janela são dois momentos dela, e vale
a maior: uma janela só enche até reiniciar. Aviso a partir de 80 %, perigo a partir de 95 % — as duas
linhas são da pessoa, digitadas na página do módulo ou na vista das contas, e as duas portas
perguntam à mesma função (`lines`, `tone_of`): a vista pinta as barras com as cores que a marca
veste. Uma janela sem porcentagem não é 0 %: é "ainda não se sabe", e a marca fica calma.

Uma chave de API não tem janela: paga por token, e a statusline dela recebe `rate_limits` nulo. A
Anthropic também não tem interface que diga o saldo dos créditos. O que se sabe é o momento em que o
Claude Code diz que eles acabaram (uma resposta que falha com `billing_error`), guardado pelo
`ronoctua_llm::credits`. Então a marca dela é perigo enquanto os créditos estão esgotados e calma no
resto, e a página e a vista dizem "esgotados" no lugar das janelas, que diriam "ainda não se sabe"
para sempre. A vista e a marca perguntam à mesma função (`credits_tone`).

O conjunto é o lugar da conta na lista inteira, e não dentro do tipo: o terceiro conjunto da modeline
é a terceira conta da vista, e o clique no glifo da conta entrega esse número ao `agents-open`. Um
número por tipo seria ambíguo num comando só (o 1 do Claude Code e o 1 do Codex). Por isso são nove no
máximo; da décima em diante não há conjunto, e a página diz.

## Ouvir sem relógio

O núcleo lê as fontes de um vigia UMA vez, quando a thread dele sobe, e as contas são da pessoa para
acrescentar a qualquer momento. Então o núcleo vigia os caminhos que não mudam — a lista, as linhas e
as duas pastas dos agentes — e o módulo mantém três vigias próprias (`ronoctua_files::notice`, inotify)
sobre o que cada conta tem, levantadas de novo sempre que o que elas devem cobrir muda:

- **os números** — as pastas em que o `ronoctua-llm` guarda os limites, a última resposta do
  endpoint, os créditos de uma chave de API e a contagem: uma mudança ali é lida de novo, e só;
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
uma mais nova, devolvendo números velhos à marca. Os caminhos vêm todos do `ronoctua_llm`
(`watched`, `sessions::folder`) e do `ronoctua_terminals` (o registro dos terminais); o módulo não
escreve o nome de pasta nenhuma (cobra `one-llm-boundary`). As pastas dos números são criadas antes de
vigiadas (uma pasta que não existe nunca é ouvida), e as duas dos agentes também, privadas; as de log
dos agentes são deles, e nunca são criadas aqui.

## A unidade, só depois de uso

`ronoctua-llm-usage.service` roda `ronoctua-llm refresh`: conta os transcripts desde onde a última
contagem parou e, para uma conta do Claude Code cujo host a pessoa permitiu, pergunta ao endpoint de
uso (não oficial; a vista diz). O cérebro a inicia só depois que uma conta foi usada desde o último
início, e nunca antes de dez minutos depois dele (`AGAIN_AFTER_S`): sem uso, nada roda; usando sem
parar, uma contagem a cada dez minutos. Fora do portão geral: é uma contagem incremental e, no
máximo, um pedido pequeno, e quem trabalha na bateria é justamente quem quer os números. A rede não
é condição (`needs.network = false`): sem rede a contagem acontece, e a recusa do pedido fica ao lado
dos últimos números.

O relógio é o reinício mais próximo de uma janela mostrada (`next_wake`): a marca vermelha porque as
cinco horas acabaram volta a ficar calma quando elas recomeçam, e a página muda de geração nesse
momento — ela mostra quanto a janela estava cheia, e o hospedeiro só a pede de novo quando a geração
anda. Sem janela conhecida, não há relógio nenhum.

E mais um, só enquanto é preciso (dono, 2026-11-02): um agente FORA dos nossos terminais (um Ghostty,
o terminal de um editor) morto sem `SessionEnd` — um `kill -9`, o terminal fechado por cima — não
escreve nada que alguém ouça; dentro de um dos nossos, o arquivo do guardião muda quando o programa
muda. Então, enquanto um agente de fora está desenhado (num conjunto, ou contado no `agent-more`), o
processo dele é perguntado de novo a cada minuto (`LIVENESS_S`, contado da última leitura dos
agentes). Sem nenhum desenhado, nenhum relógio. Um `pidfd` no núcleo tiraria até esse.

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
- **O censo dos terminais pelos sockets**: ele pergunta a cada guardião se está vivo, e um congelado
  seguraria a vigia do núcleo, que não pode esperar. O arquivo de estado basta; quem limpa o de um
  guardião morto é o censo (`Registry::census`), fora de uma vigia que não espera.
- **Um relógio de vida sempre armado**: dentro dos nossos terminais, e sem agente nenhum, ele acordaria
  a máquina para nada.
