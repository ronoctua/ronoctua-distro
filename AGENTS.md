# ronoctua-distro

Distro NixOS pessoal: sem barra, sem dock, sem menu iniciar — só o conteúdo e um rodapé de duas
linhas. Scroll como compositor, camada de shell própria em Rust. O porquê de cada decisão está em
`docs/ARCHITECTURE.md`; o que ainda falta, em `docs/PENDING.md`.

## Invariantes — nunca violar

1. **Escopo é a pasta do projeto.** Nunca vasculhar atrás de código; ler caminho que o dono APONTOU, sim.
2. **Um repo, PÚBLICO**: `ronoctua/ronoctua-distro`. Nada pessoal — o e-mail do perfil NÃO é o de
   commit: confira `git config user.email` antes do primeiro. MIT + OFL.
3. **Sem fork e sem patch do compositor.** Só IPC e layer-shell. **E o shell fala com ele por um
   CONTRATO**: `ronoctua-compositor` são os tipos e o `trait`, `ronoctua-scroll` é o ÚNICO crate
   da árvore que sabe qual compositor é, e nenhum outro o nomeia. Um WM é externo; o que
   desenvolvemos não pode depender de externo nenhum.
4. **O shell nunca reimplementa layout de janelas.** Um `xdg_toplevel` por buffer nativo.
5. **Sem lógica Nix reimplementada em Rust.**
6. **Nada imperativo.** Se não está no flake, não existe.
7. **`nix flake check` tem que passar** antes de qualquer coisa ser considerada pronta.
8. **Zero string literal em UI.** Tudo por `text-key`, completo em TODOS os idiomas declarados.
9. **Zero valor visual hard-coded** — cor, fonte, tamanho, espaçamento, codepoint: tudo por token.
10. **Ícones da UI são glifos da fonte.** O superpacote é só fallback de pixmap para apps
    estrangeiros; nenhum componente do design system o consulta. Marca que fonte nenhuma tem se
    DESENHA numa nossa (`assets/icons/marks.nix`, `nix/marksgen.nix`, `U+F3000`) — pontos numa
    grade, e o que é DESENHADO é centrado, não a grade.
11. **Buffers WASM não tocam Wayland, não recebem valores de token, não traduzem.**
12. **`ronoctua-design`, `ronoctua-protocol` e `ronoctua-syntax` compilam para `wasm32-wasip2`.**
13. **Nenhum dado pessoal** — nem `host.nix`, `facter.json`, `~/.shellrc.local`: nascem NA máquina.
14. **Chrome é o rodapé e, no máximo, UMA coluna à direita por vez** — do topo até a área do
    rodapé, sem estreitá-lo; sem barra de cima, relógio, taskbar, menu iniciar, dock, ícone de
    desktop ou popup flutuante. Tray, notificação e expansão são A MESMA superfície crescendo. A
    regra anterior dizia "nenhuma chrome além do rodapé", e o dono a corrigiu ao pedir o player: a
    coluna é permitida, e o que não se permite é ela tomar a largura da modeline, do minibuffer ou
    da faixa de notificação.
15. **Sem sombra e sem elevação.** CARD e LINHA SEPARATÓRIA são PERMITIDOS — a preferência por
    um layout mais limpo é preferência, não proibição, e quem decide é o dono. Também é permitido
    pôr COLUNAS LADO A LADO, cada uma com as suas próprias seleções, e **uma seleção não precisa
    ocupar a linha inteira**: `selection` e `rule` levam `column` e `columns` (`0` = até a borda).
    A regra anterior dizia "sem cards" e o tipo a reforçava — `line-command` não tinha onde pôr a
    largura —, então duas colunas com seleções próprias eram indesenháveis. Isso nunca foi decisão
    do dono; era a forma que o tipo carregava, escrita como se fosse lei.
16. `noctua` e `ronoctua-distro` definidos em **um único lugar** cada — `nix/modules/base.nix`;
    os **idiomas**, em `assets/i18n/languages.nix`, e nenhum outro sítio os nomeia.
17. **Sem Docker.**
18. **Todo NOME, COMENTÁRIO, MENSAGEM e COMMIT são inglês** — arquivo, comando, check, chave,
    `[[bin]]`, identificador (`let`, `fn` e parâmetro inclusive), `//`, `#`, docstring, `assert`,
    `bail!`, `.context`, saída de check e ajuda de linha de comando; cobra o `english-names`.
    pt-BR **só onde o valor existe em TODOS os idiomas declarados**: os 6 docs, o VALOR de chave
    em TODOS os `.ftl`, a tag de busca e o léxico. **Onde cabe um idioma só, ele é o INGLÊS** —
    descrição de unidade, `.desktop`, manifesto da extensão, alias do shell, `Job::trouble`.
19. **TODA IMPLEMENTAÇÃO RESPONDE AO PONTEIRO.** O que o teclado alcança, o ponteiro alcança:
    selecionar, abrir, mudar valor, rolar e ARRASTAR a barra. Clique SELECIONA e o segundo faz o
    que o `RET` faz; o ponteiro em repouso ACENDE a linha sob ele, que não é a faixa do cursor.
    Não há tela só de teclado. Cobram o `a-buffer-answers-the-pointer` (buffer que desenha faixa
    responde clique e hover; que desenha barra responde roda e arrasto) e o
    `a-bar-is-grabbed-where-it-is-drawn`.

Ao bater num invariante: **pare e reporte.** Não contorne, não peça exceção implícita.

## Comandos

```
nix develop           entra no ambiente (direnv é atalho opcional, e do HOST)
nix run .#dev         compositor aninhado + rodapé; leader = Ctrl+Alt+Espaço, não Super
nix run .#png         um buffer vira PNG em 0,2 s, sem VM: a tela antes do "pronto"
nix build .#shots-<cena>  fotografa UMA cena e o que ela precisa; .#shots são as 19
nix run .#quick       DEZ escolhidos (tests/quick.nix), ~1 min; subconjunto, não categoria
nix run .#gate        TUDO; dimensiona pela RAM e o earlyoom. Só ele fecha o inv. 7; .#most é a bateria de todo dia, sem instalação
nix run .#vm  VM completa; .#iso o ISO; .#bench falha se estourar; .#diagnostics só lê
nix run .#install     instala (ensaia sem --execute);  ronoctua.profile: full ou minimal
```

Flake apps, nunca justfile nem Makefile. Passo que precise de instrução escrita devia ser automático.

## Como funciona

- **O rodapé é o desktop.** `wlr-layer-shell` embaixo: modeline e minibuffer COLADAS, expansão com teto e página abaixo, **faixa de notificação acima**, e além dele no máximo uma coluna à direita (inv. 14). Ancorado embaixo, crescer para cima não move a modeline. Clique só faz o que tecla faz.
- **Todo buffer tem ESTADO, e o primeiro é `normal`** — recusado no load se não for. `Esc` sozinho volta UM nível (o buffer é que anda, e relata); **segurar `Esc` + setas gira**; `LEADER w s` escolhe; clique na modeline gira. O toque-contra-retenção é resolvido NO HOST do buffer (keyd pegaria o `Esc` do vim), que avisa o rodapé pelo socket de controle — e o rodapé PERGUNTA na troca de foco. Estrangeira não tem estado. Id inglês, tela por `state-<id>`; `normal` discreto, todo o resto num token SÓ.
- **A fronteira com o compositor é um CONTRATO, e só o adaptador fala IPC** — `ronoctua-compositor`
  são os tipos neutros e o `trait Compositor`, com zero socket e zero JSON; `ronoctua-scroll` é o
  único crate que sabe qual compositor é. Shell, display e resgate dependem só do contrato, e é isso
  que faz "não depender de externo" ser estrutura em vez de intenção. **E a
  leader vem de FORA**: em repouso o rodapé não captura teclado, senão nenhum buffer receberia tecla.
  Quem a vê é o compositor, que roda `ronoctua-shell leader` no socket de controle
  (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`); só então a superfície expande e pede exclusividade.
  `ronoctua-shell command <id>` roda qualquer id — é por onde entram os QUATRO binds SEM leader.
- **Todo atalho do config que o Scroll traz existe aqui** (`actions::scroll_chords` e
  `actions::modes`): a sessão passa `-c` e aquele arquivo nunca é lido, então os ~380 atalhos (os
  15 modos inclusos) são gerados da nossa tabela, na MESMA tecla. Onde um acorde nosso já tinha a tecla,
  o nosso fica (regra do dono) e o do Scroll que ficou sem tecla ganha outra (`S-h/j/k/l`, `i`,
  `m`, `s`, `C--`/`C-=`); `Super+Alt+…` ganha substituto no `.#dev` (`C-S-…`), porque lá o prefixo
  É o Alt. `button8` pede o prefixo (nu, tomaria o Voltar de todo navegador), `floating_modifier`
  é o prefixo (arrastar com Super, ou Alt no `.#dev`), e os programas que o arquivo chama viram os
  nossos (lançador, gerenciador, sair, captura, brilho). **Modo não se vê sem barra**, então o
  rodapé ouve o evento `mode` (`Event::ModeChanged`): marca na modeline (clique sai) e as teclas
  do modo na linha de dicas, da mesma tabela. Cobra o `every-compositor-shortcut-is-bound`, que lê
  o `config.in` PINADO e acusa atalho que falte — inclusive o próximo que o upstream acrescentar.
- **`Ctrl+Alt+Del` é o RESGATE** (7 toques em 2 s ainda forçam): janela flutuante, ou VT 8 sem
  compositor.
- **Rodapé morto tem uma LINHA que sobrevive a ele** (`ronoctua-shell lifeline <unidade>`): uma
  SEGUNDA layer-shell, no lugar do rodapé, com o que aconteceu e quatro opções — ver os logs (abre o
  journal no console de resgate), reiniciar, IA (desenhada e RECUSANDO, ainda não existe) e fechar.
  Não é uma segunda chrome (inv. 14) e é decisão do dono: é a linha do RODAPÉ desenhada por outro
  processo, viva exatamente enquanto não há chrome nenhuma. **Quem detecta é o `OnFailure=`**, que
  custa ZERO com tudo bem — nada de thread, relógio ou pergunta; e foi MEDIDO, porque a documentação
  diz que ele só dispara no estado `failed` e uma unidade com `Restart=` sem teto nunca chega lá: com
  duas unidades transientes ele disparou em toda falha, três de três. **O brain fica de fora** — ele
  não desenha, e isso está escrito no contrato dele. No laço aninhado não há unidade, então o
  `footer-single.nix` vigia o PID e levanta a mesma linha; ele distingue queda de recarga por um
  arquivo `.onpurpose`, senão a linha nasceria a cada save. E isso importa porque **com o rodapé
  morto os QUATRO binds de emergência estão mortos junto**: os quatro escrevem no socket dele.
- **O vigia do laço aninhado sabe PARAR, e o `.#dev` avisa a morte que pede.** O `footer-single`
  mantém um subshell que ressuscita o rodapé caído; ele é assíncrono (logo `SIG_IGN` em `SIGINT`, e
  isso sobrevive ao `execve`) e reparentado, então Ctrl+C nunca o alcançou — e ninguém guardava o PID
  dele. Sem condição de parada nenhuma, toda saída do `.#dev` virava um laço: o dono viu uma volta a
  cada **2,01 s durante uma hora**, com `Error: raise the lifeline` no terminal. Agora ele escreve o
  próprio PID em `<pidfile>.watcher` (o `shutdown` o mata pelo nome), e pergunta TRÊS coisas antes de
  repetir: se a morte foi anunciada (`.onpurpose`, que o `dev.nix` passou a escrever ANTES do kill),
  se o compositor sob o qual aquele rodapé nasceu ainda vive — **pelo PID que o `dev.nix` EXPORTA
  (`RONOCTUA_COMPOSITOR_PID`), nunca por `[ -S ]`: soquete de servidor morto por sinal FICA no
  disco, medido uma hora depois** — e se a sessão que pediu o rodapé ainda existe
  (`RONOCTUA_DEV_PID`). Mais um teto de
  cinco reinícios por minuto. O `shutdown` também encerra o que o RODAPÉ subiu ao lado (transfers,
  brain: `PGID == PID`, alcançáveis só pelo `/proc/<pid>/exe` apontando para esta build) e poda na
  PARTIDA os logs de sessões velhas — eram 65 arquivos e 55 MB. Cobram o `a-dead-footer-stays-dead`
  e o `the-dev-loop-ends-with-the-session`.
- **O laço é uma SEGUNDA cópia do sistema, com nome próprio** (`RONOCTUA_INSTANCE=dev`,
  `ronoctua_xdg::instance`/`name`/`runtime_file`): toda raiz nossa vira `ronoctua-dev` e todo
  socket nosso `ronoctua-dev-…`, então o `.#dev` roda DENTRO da distro instalada sem reescrever o
  `scroll-pointers.conf` que o compositor de verdade inclui nem falar com a fila e o cérebro da
  sessão de verdade. **O cérebro de uma instância só ENSAIA** — senão seriam dois agendadores
  reconstruindo o sistema. O que não é nosso (Lixeira, miniaturas do freedesktop, apps, histórico
  do shell, o segredo do aria2 e o PO token, que são da máquina) segue compartilhado de propósito.
  **E sobra se acha pela MARCA, nunca pelo nome**: o laço encerra na partida o processo que traz
  `RONOCTUA_INSTANCE=dev` no ambiente e cujo `RONOCTUA_DEV_PID` morreu — um `pkill 'ronoctua-'`
  levaria o rodapé, o cérebro e a fila instalados, que têm os mesmos nomes. O `.#vm` encerra só a
  VM que já está no MESMO disco (o caminho na linha de comando do QEMU).
- **A aritmética do rodapé não pergunta ao conteúdo do rodapé.** `Footer::metric` é o fundo: ele
  transforma largura, altura e a PRIMEIRA LINHA da região num frame, e essa primeira linha é o
  `first_of_expansion`. Nada que o `metric` alcance pode perguntar o que a região CONTÉM — e a shell
  já morreu DUAS vezes por isso, as duas de stack overflow sem diagnóstico nenhum além do
  `process::abort`. A segunda foi `first_of_expansion → explain_rows → grid_full → expansion_len →
  screen_columns → metric`, e bastava o ponteiro parar sobre uma tecla da dica. O que a região tem a
  dizer sobre si é LEMBRADO onde foi medido (`region_can_spare_a_row`, escrito no `grid`), nunca
  perguntado de dentro da soma. Cobra o `the-footer-does-not-ask-itself`, que também recusa qualquer
  ciclo novo entre os métodos do `Footer` — as cinco recursões que existem são declaradas.
- **Todo comando é `kebab-case`, e a chave é `cmd-<id>`**; o registro recusa divergirem. Os 212
  feitos do compositor entram como `wm-<nome>`, DECLARADOS em `assets/compositor-deeds.nix` e
  GERADOS de lá — este compositor não enumera nada, seus comandos são texto livre espalhado por
  quatro tabelas em C, então a tabela é NOSSA e o gerador recusa um verbo que a fonte pinada não
  tem. Alcançáveis por nome (`LEADER :`, e SÓ ele — o `x`
  paralelo saiu a pedido do dono; duas teclas para uma coisa gastam duas linhas da lista, que é o
  recurso escasso ali). **`m` é MISCELÂNEA**, o gaveteiro do que não pertence a nenhum outro grupo
  (a captura de tela e o `m i` dos três navegadores), e é o que libera `c` para ser o clipboard
  SOZINHO — comando, não prefixo. **`s q` são as saídas**, porque o que elas fazem é o que a máquina
  faz a SI MESMA, que é a frase que define o grupo `s`. E `w` se chama `window/buffer`: janela e
  buffer são a mesma coisa (inv. 4), e o rótulo dizia só metade. **O teclado tem UM
  lugar** (`nix/keyboard-xkb.nix`); o xkb **não recusa** layout inexistente — cai no `us` mudo, e
  quem impede é `keyboard-layout-exists`.
  **E A LISTA DA LEADER RESPONDE O PONTEIRO EM TODAS AS COLUNAS** (`surface::page_length`): ela é
  a ÚNICA lista do sistema desenhada em colunas, e o número contra o qual o `item_at` recusava um
  índice era `grid.lines` — a altura de UMA coluna. Como um item da segunda coluna vale `coluna ×
  linhas + linha`, o primeiro deles já é `lines`, então toda célula fora da primeira coluna
  respondia `None`: sem faixa de hover e com o clique não fazendo nada, em dois terços da tela. O
  corte agora é o mesmo que o DESENHO faz (`expansion_window(offset, grid.per_page())`), porque o
  que o ponteiro alcança é o que foi desenhado.
  **E AS TRÊS TINTAS DE UMA LINHA DELA ESTÃO NUM DEGRAU SÓ**: a tecla (`text.key`), o submenu
  (`text.group`) e o comando (`text.secondary`) resolvem todas a **Lc 48** sobre o chrome, nos nove
  temas, e o que as separa é MATIZ. Eram 52, 56 e 48 — o submenu emprestava `group.solid`, que é o
  degrau com que uma família PREENCHE um botão, e a tecla era `state.accent` —, então o que só leva
  a outro nível gritava mais alto que o que roda, e a coluna da tecla gritava em toda linha. O dono
  leu na tela. Portão nenhum media isso: as três eram medidas contra os FUNDOS e nunca uma contra a
  outra; agora três `DISTANCE_GATES` guardam os pares, porque sem luminância a distinção é só o
  croma e um tema pode drenar o croma em silêncio (no croma partilhado, a tecla do frost ficava a
  ΔE 0,020 do comando). O croma vem do MULTIPLICADOR da receita e nunca do `text-chroma` da
  semente: a rampa de texto de uma família tem outros inquilinos — a do acento tem quatro, e subi-la
  levou a onda do frost a 0,198 de `#00FFFF`, que é uma das quatro cores do filme. **E `text.group`
  virou DOIS tokens**, porque um nome fazia dois trabalhos: o submenu, que é o que ele significa, e
  o TÍTULO DE SEÇÃO, que vinte e seis sítios em doze vistas tinham tomado emprestado. Título é o que
  as linhas penduram embaixo e hierarquia aqui é cor e ar (inv. 15), então ele FICA no sólido
  (`text.heading`, o mesmo hex de sempre em todos os temas) e só a lista desceu. Cobra o
  `a-heading-is-not-a-submenu`: `TEXT_GROUP` só existe no braço `e.is_group`, e todo construtor
  cujo NOME diz que desenha título (`heading`, `headed`, `title`, `drawer`, e os que são closure)
  leva `TEXT_HEADING` e nenhum segundo tom de texto. As DUAS isenções — a linha de dicas de uma
  janela e o renque de nomes de coluna da listagem — são nomeadas e o motivo é REMEDIDO.
- **TODA dica se explica sob o ponteiro** (`chrome::hint_marks` → `hint_sentence`): uma linha
  ABAIXO dos atalhos, nas onze portas, viva só enquanto há hover (escolha do dono). A frase é
  `cmd-<id>` para comando do registro e `about-<id>` para tecla de vista, nunca as duas
  (`every-hint-key-explains-itself`, que achou 10 ids desenhando `[syncthing-down]` na TELA). **A
  linha sai da REGIÃO e nunca da altura do rodapé**: o rodapé é ancorado embaixo, então uma linha
  acrescentada sob o minibuffer o levanta de baixo do ponteiro — o hover acaba, a linha some, e
  roda. É o laço que o `Fold::held` descreve, com o ponteiro no mesmo lugar da corrente.
  **E a frase tem REGISTRO PRÓPRIO** (`Register::Explain` → `text.subtle.explain`): era a prosa
  da dica, e o dono leu na tela que ela parecia mais um atalho da linha de cima. É a ÚNICA cor
  ACROMÁTICA do sistema (`chroma = 0.0`), porque é o terceiro assento: a dica é uma matiz, o
  conteúdo é a rampa quente, e matiz nenhuma é o que sobra. Croma zero sozinho não bastava —
  em cyber, industrial e darkside os cinzas já não têm cor e o degrau do meio virava
  `text.secondary` exato (ΔE 0,0000, medido) —, então ela leva LUMINÂNCIA própria também
  (`neutral.text-loud`), com portão contra os dois registros da dica e os três da rampa.
- **Três navegadores, um léxico** (`m i n`, `m i u`, `m i l`). Só o da Lucide personaliza; a prévia é o SVG COPIADO, e a marca sai da MESMA `affordance` que a tecla lê.
- **São TREZE vistas, e a precedência vive só em `Footer::view()`** — nenhuma é menu (inv. 14), e
  **TODA porta pergunta a ele**: o ramo de teclas, o `expansion()` e o `region_is_free` das onze
  portas. Isso não era verdade e três revisores acharam o mesmo defeito em três vistas: cada um
  lia o PRÓPRIO campo, as ordens divergiam, e clicar no sino com a energia aberta dava o teclado a
  uma e a tela à outra. Com todos perguntando, no máximo um ramo casa e a ORDEM em que eles estão
  escritos deixa de ser resposta; cobra o `the-footer-draws-what-the-keyboard-drives`. A pergunta do
  minibuffer é a única exceção, e é deliberada — pergunta é LINHA, não região. O localizador (`a`)
  varre o XDG na PARTIDA em thread; a rede (`s n n`) lê o CACHE do NetworkManager.
- **`LEADER .` é o gerenciador, e é um SUBMENU de duas portas** — `. .` no rodapé (teto de 15,
  `space.expansion.maxLines`; eram 12, e o dono pediu três linhas a mais de FATOS: a coluna de infos
  recebe toda linha da região que a chrome não leva, então o aumento chega lá uma a uma. O teto é um
  `min` com `maxFooterPct`, e é ele que continua mandando em tela pequena — quem prova que o número
  ainda quer dizer alguma coisa na tela para a qual foi levantado é o
  `the_ceiling_this_screen_was_raised_for_is_reachable`, senão subir o token compraria zero em
  silêncio) e `. w` em janela. O `LEADER f` MORREU a pedido do dono: uma coisa alcançável por três
  sequências, e a letra não dizia nada que o `.` não diga. E o `.` teve de deixar de ser comando
  para virar prefixo — o registro recusa as duas coisas na mesma sequência. UMA
  `files::View`; só o `ronoctua-files` toca o disco. **Letra é QUERY** e vai no FIM do caminho;
  comandos da vista sob `C-` — **nunca `M-`, que é a família dos acordes do compositor**;
  `Backspace` vazio SOBE; o direito só FOCA, e a linha na TELA não é a da listagem.
  **←/→ andam entre as SEÇÕES** (`TAB`/`S-TAB` seguem ligados e fora da lista) e **`S-<setas>` mudam
  de sentido com a seção em foco** (`strong_meaning`; sem resposta própria, a listagem responde) —
  na dica elas levam a frase resolvida e cor própria. **A linha de dicas é GERADA de `OF_THE_VIEW`**,
  uma notação por ação e o `·` do código (`mark.separator`): as 40 frases já existiam nos dois
  `.ftl` e nenhuma tela as lia. **Prévia e fatos têm cursor, e a marca de foco é a RÉGUA acesa**
  (`chrome::divider_lit`; título e barra acesa foram recusados — e a régua que a FAIXA não tem é
  desenhada, senão a prévia ficaria sem marca), e a listagem guarda faixa quieta ao perder o foco. **A gaveta mede a altura UMA VEZ, ao abrir**
  (`fold::settle`): altura que segue o conteúdo é altura que o ponteiro move, e isso é um laço de
  layout. O chevron fica com a última célula da linha 0; a barra pega da linha 1 para baixo.
  **O nome que não cabe é alcançável por TRÊS caminhos**: o ponteiro o põe inteiro no minibuffer
  (`Field::explain`, e só quando foi cortado), o cursor chegando nele o ECOA por 3 s (`Step::Say`,
  o mesmo `ECHO_SECONDS` das duas portas), e ele ANDA de lado uma célula por vez (`slide.rs`, 1 s
  parado em cada ponta, 200 ms por célula, e volta de uma vez — escolha do dono). O relógio é
  ARMADO e só existe enquanto um nome anda. **`..` é o primeiro renque** — nunca
  oculta, nunca marcável, e toda célula dela responde a saída. Permissão
  PERGUNTA nos DOIS caminhos, **e cada DÍGITO do número explica o que ele é** — de quem é aquele
  terço e o que aquele valor permite (`files-explain-octal-digit` + `files-octal-can`), com os
  parênteses guardando a frase geral; primeiro o número não dizia nada, depois dizia UMA frase para
  os três dígitos, que é a definição de octal e não uma resposta sobre o arquivo, e **UMA célula acende UM campo**
  (`files::lit_field`, um `rposition`, e o `field_at` escrito em cima dele): o `covers` dá de
  propósito o vão DEPOIS de um campo a ele, então onde os campos são colados — as dez letras, o
  `owner:group` — a célula do meio é coberta por dois, e os pintores, que perguntavam `covers` por
  campo, acendiam os dois. O que o olho vê aceso é agora, por construção, o que o clique pega; cobra
  o `the-pointer-lights-what-the-click-takes`. **Rótulo de `Choice` é TEXTO, não chave**; texto abre no `emacsclient`.
  **O TIPO SAI DO NOME, e o EMPATE sai dos bytes** (`mime::named` → `Named::sure`): a base
  reivindica `*.ogg` por QUATRO tipos no mesmo peso, porque a extensão nomeia um CONTÊINER, e o
  `by_name` ficava com o primeiro da tabela — o dono fotografou um vídeo Theora com o glifo de
  música. Empate é o nome dizendo que não sabe, e só os bytes sabem; o conteúdo só é aceito se
  REIVINDICAR aquele nome (senão é arquivo mentindo sobre a extensão, e a extensão é o que a
  pessoa digitou). O custo é uma leitura de 4 KiB por nome empatado — medido, 8 de 64 na pasta do
  dono, e `.jpg`/`.rs`/`.pdf` continuam sem tocar no arquivo. **As SEIS grafias de playlist têm UM
  glifo** (`list`), porque `audio/x-mpegurl` e `application/vnd.apple.mpegurl` reivindicam os mesmos
  nomes: qual vence é decidido pelos bytes, e isso não pode aparecer na tela.
  **O bit `x` colore o GLIFO e não o nome** (`glyph_colour_of`, a pedido do dono), com frase sob o
  ponteiro (`files-explain-executable`) — cor sem palavra é legenda que ninguém lê. Medido: o verde
  sobre a faixa da seleção dá 7,06:1 no pior tema, então não precisa do `lift_onto_a_band`.
  **A prévia ENCOLHE e nunca cresce** (`Area::centre`, que já existia para o navegador de ícones):
  o dono leu na tela no `sample-img.ico` dele, um ícone de 390 bytes esticado no painel inteiro —
  aumentar inventa pixel, e num ícone de 16 inventa todos menos 256. Filme e GIF não perdem nada:
  o quadro já nasce DO tamanho da caixa. **Clique no minibuffer VAZIO é a leader**
  (`on_the_empty_minibuffer`): "vazio" é o `Footer::view()` respondendo `NoneOf`, que é o mesmo
  lugar que decide todo o resto; o chevron e os segmentos da modeline são perguntados ANTES e
  ficam com o sentido que têm. **Prévia e infos nascem LIGADAS** (`C-v`/`C-i` ainda escondem) e por isso a região sobe ao teto
  com o gerenciador aberto — a ALTURA deixou de responder ao que foi digitado, e o `leader-vm` lê
  a TELA. Sem foco, painel não pede linha nenhuma. **A pasta DÁ NOTÍCIA** (`ronoctua_files::notice`,
  o nome `watch` já é das cópias): inotify na pasta aberta e na da barra, um watch e nunca recursivo,
  rajada absorvida na thread (a 1ª mudança sai na hora, o resto 1×/s — senão copiar N arquivos custa
  N `reread()`, que é O(N²)). Responde com `reread()` SECO: `stale()` é o mtime da PASTA e não vê
  arquivo reescrito no lugar nem o balde de 2 s do vfat. O rodapé FICA com o `stat` de 1 s — é o piso
  para NFS, que o inotify não vê —; a janela não ganhou relógio (em repouso não acorda NENHUMA vez,
  que é o custo de ter várias abertas), só se o observador não subir. Fechar DESCANSA o watch, e os
  dois lugares que largam a vista chamam — o segundo é perder o FOCO. A ficha ROLA (`C-.`, e a barra diz que cortou);
  **A LISTAGEM TEM DUAS FORMAS, e a segunda são MINIATURAS** (`C-g`, e o glifo clicável no fim da
  linha do caminho — o dono pediu as duas). Nasce desligada. A grade é a `ronoctua_design::Grid`, a
  mesma dos três navegadores de ícones, e não uma segunda: o `at`/`index_at` dela é a MESMA soma nos
  dois sentidos, e o sintoma que ela existe para impedir está escrito no topo dela — *"o clique cair
  num item enquanto a tela mostrava outro"*, que numa listagem apaga o arquivo errado. `Section`
  ganhou UM campo (`grid`) e duas conversões (`line_of`, `row_at`), e `offset`/`total` continuam em
  ENTRADAS: a roda, o arrasto, o `follow` e o `scroll_listing_to` já falam entradas, e dar a eles uma
  segunda unidade é como um número passa a significar duas coisas. **A célula responde no VÃO**,
  porque uma miniatura é quase toda célula vazia e `View::hit` acabava em `field_at` — o mesmo
  defeito do botão de expandir, sessenta vezes por tela. **E a faixa é o RETÂNGULO da célula**, não a
  linha inteira: uma faixa de largura total sobre uma prateleira acende sete arquivos para dizer que
  o ponteiro está em um. A linha dos nomes some na grade (não há coluna para nomear) e `C-G` gira
  três tamanhos — `space.files.thumbSmall/Medium/Large`, em CÉLULAS, com a altura DERIVADA da largura
  pela razão célula/linha da fonte: declarar as duas seria um segundo valor visual que pode divergir
  do primeiro, e a divergência é uma figura que não é quadrada.
  **UM FORMATO É O QUE `pixels::how` RESPONDE**, e isso é uma prova e não uma diligência: as seis
  rasterizadas em processo, SVG, RAW pelo exiftool, todo vídeo (pôster), todo áudio (capa, e a onda
  quando não há capa), PDF, e todo o resto de `image/*` pelo ffmpeg. **A PEÇA 3D é DESENHADA** no
  trabalhador (`mesh::read` + `solid::draw`, 18–44 ms medidos) — o `stage.rs` existe para GIRAR uma
  peça e guarda o corpo para isso, e sessenta corpos seriam a memória do gerenciador. O que não tem
  figura fica com o glifo do tipo, que cobre os 1 232 tipos do freedesktop por construção.
  **E O TIPO SAI DO QUE O ARQUIVO ABRE**, nunca do que ele É: `View::opens` é uma segunda tabela ao
  lado de `types`, e a diferença é o symlink — o GLIFO diz que é um link, que é o fato que uma
  listagem não pode omitir, e a MINIATURA mostra a fotografia, porque *"um link para uma fotografia é
  uma fotografia quando se olha para ela"*. Custa um glob por LINK numa leitura de diretório.
  **O trabalho é um POOL, e não a thread da prévia** (`thumbs.rs`): o `inspector` esvazia a fila até o
  mais NOVO, que é certo para um painel sobre o arquivo sob o cursor e jogaria fora 63 das 64 células.
  É o molde do `apps::painter` — pedidos todos queridos, resposta por chave — com várias mãos, porque
  aqui o custo é um PROCESSO. Medido na pasta do dono (64 arquivos, 36 extensões): uma miniatura custa
  ~110 ms seja qual for o arquivo (109 ms num JPEG de 22 KB, 167 num MP4 de **355 MB** — paga-se o
  processo, não a figura), e a pasta inteira sai em 8 129 ms numa mão, 1 681 em seis e 1 293 em doze.
  Metade da máquina, piso de duas e teto de oito. **Só o que está VISÍVEL é pedido** (o precedente
  literal é o `request_visible_icons`) e **`Some(None)` é uma RESPOSTA** — "este arquivo não dá
  figura" —, senão um documento é perguntado a cada quadro.
  **E A GRADE CABE NA PORTA EM QUE ESTÁ.** O tamanho é limitado dos dois lados: por baixo pelo teto
  de figuras do renderizador, e por cima por `THUMB_SHELVES` — **duas prateleiras**. O rodapé dá doze
  linhas, uma é o caminho e quatro são a faixa dos painéis, o que deixa sete; ao degrau médio isso é
  UMA prateleira com o resto da região em branco, que é uma listagem PIORADA — sem noção de página,
  sem para onde descer, e com uma rolagem que anda uma tela por vez. O dono fotografou: *"a versão de
  rodapé … está mostrando apenas uma linha no meu laptop em vez de duas"*. Então o degrau DESCE até
  caber, e uma janela alta não paga por isso.
  **O GLIFO SÓ APARECE ONDE NÃO HÁ FIGURA, e ele ENCHE a sala dela.** Duas coisas que o dono
  fotografou na mesma tela — *"os glifos dos itens que não tem thumbnail aparecem minúsculos"* e *"os
  glifos estão aparecendo por cima das thumbnails"* — e são uma pergunta só feita duas vezes: esta
  célula tem figura? A resposta decide se o glifo é desenhado e quanta sala ele tem. O tamanho viaja
  no CAMPO (`Field::tall`) e não na linha, e isso não é arbitrário: o rodapé achata as quatro seções
  numa linha só, então uma altura por LINHA fez `trash` e `owner:group` ficarem do tamanho de um
  glifo de miniatura ao lado dele — fotografado também. `files::run_size` é o único lugar onde linhas
  viram um tamanho, e as duas portas o chamam.
  **O PONTEIRO MARCA VÁRIOS, ARRASTA E SOLTA** (a pedido do dono, e nas DUAS formas da listagem).
  `Ctrl`+clique marca um item sem abrir, `Shift`+clique toma o renque inteiro de onde o cursor estava
  até onde se clicou, o clique no AR põe as marcas no chão (o mesmo código do `C-A`), e o segundo
  botão sobre um item NÃO marcado limpa as marcas e age naquele item — clique direito que operasse em
  silêncio sobre uma seleção que o olho não tem é como se apaga a pasta errada. **O RETÂNGULO é
  desenhado POR CIMA das miniaturas** (`files::sweep_px` no grupo de cima do `Quads::draw_range`,
  `surface.sweep`, a segunda faixa com alfa do sistema): sem teste de profundidade, um retângulo
  empilhado com os outros fundos é invisível numa grade, que é figura de borda a borda e o único
  lugar onde ele importa. Uma função diz o que ele cobre (`View::sweep_box`) e o pintor e a seleção
  (`View::swept`) a LEEM — é `[[um-controle-e-pego-onde-e-desenhado]]` num gesto que, escrito duas
  vezes, apaga o que a banda não estava por cima.
  **E ARRASTAR É `wl_data_device`**, não um gesto interno: `carry.rs` é a fronteira, o formato é o
  `text/uri-list` do freedesktop, e por isso o arrasto atravessa para outra janela nossa e para
  qualquer programa estrangeiro — nos dois sentidos. A soltura vira `Work::Paste` na FILA
  (`Doing::Drop`, nunca `Doing::Paste`, que gastaria o clipboard de alguém), e `Move` é preferido a
  `Copy` a menos que a fonte só ofereça cópia. **Um cliente que é as DUAS pontas não lê o próprio
  cano**: `receive` pede ao compositor que mande a FONTE escrever, e a fonte é este mesmo laço —
  medido, `dropped=0` seguido de `Broken pipe`. Quando a `Carrying` tem fonte viva, a lista vem dela
  (`Carrying::offering`); de fora, o cano é lido com prazo.
  **E UM DESTINO DE DROP É UMA LINHA, NUNCA UM CAMPO** (`View::row_under`): o `hit` acaba em
  `field_at` e uma PASTA não tem tamanho para desenhar, então depois do nome dela a linha não
  responde nada — medido na bateria de fotos, a pasta nunca acendeu e o arquivo caiu no vazio,
  enquanto o ARQUIVO uma linha abaixo respondia em toda parte porque tem tamanho. É o mesmo defeito
  que as miniaturas já consertaram (`Section::row_at`), e aqui o que se mira é para onde os arquivos
  de alguém vão. A aritmética da linha é a MESMA do `hit` (`row_of_line`), para que as duas nunca
  discordem sobre qual linha é a primeira entrada; hover e clique não mudaram — eles são sobre um
  campo, e o que o olho vê aceso continua sendo o que o clique pega.
  **E NÃO HÁ FAIXA DE HOVER SOBRE UMA GRADE.** A regra é mais velha que o modo e o `band_belongs` já
  a dizia: uma faixa diz que LINHA DE UMA LISTA está sob o ponteiro, e uma fotografia não é lista —
  lado a lado a região inteira é fotografia. O dono fotografou o que desenhar uma assim mesmo parece:
  *"a linha do hover do mouse aparece e está enorme. Porém não deveria aparecer nenhuma linha"*. O que
  continua dizendo onde o ponteiro está é o CAMPO que ele acende, o nome sob a célula. A faixa do
  CURSOR fica: ela é onde o teclado está, e uma grade sem nada aceso é uma grade que não se percorre.
  **O ANEL VENCE DEPOIS DE TOCADO.** Enquanto ninguém o girou, o desenho escolhe um tamanho que CABE
  na porta; depois disso quem pediu manda — *"ao apertar Ctrl+Shift+g os tamanhos não estão sendo
  alterados no modo rodapé"* é o que um teto silencioso parece. E ele anda a partir do que está NA
  TELA, não do que foi pedido por último: numa porta baixa o pedido nunca desenhado era o do meio, e
  a primeira tecla pulava de pequeno para grande.
  **E HÁ UM GLIFO PARA O TAMANHO**, à esquerda do da forma, a pedido do dono. Ele desenha QUAL dos
  três é — ao contrário do vizinho, que desenha para onde VAI: um interruptor tem para onde ir, um
  anel de três não, então o que ele pode dizer com utilidade é onde ESTÁ. Some quando as entradas
  voltam a ser uma por linha, e devolve as células ao caminho: um controle que não faz nada não fica
  segurando espaço. **Com uma célula de AR entre os dois** (`space.files.layoutAir`): dois glifos da
  mesma família e do mesmo peso em células vizinhas não têm fronteira nenhuma que se veja, e o dono
  os leu na tela como um controle só — *"colado no glifo de trocar de modo"*. **E o da FORMA não se
  mexe ao trocar de modo**: o do tamanho nasce à ESQUERDA dele, e o par não é alinhado à direita
  como um bloco — um controle que escorrega uma célula no instante em que é apertado sai de baixo do
  ponteiro que o apertou, que é o defeito que o botão de expandir a ficha já teve uma vez. O zero
  desse ar é recusado pelo COMPILADOR, porque toda asserção sobre a distância entre os dois é
  escrita em função dele e ficaria verde com eles colados de novo.
  **O NOME TEM DUAS LINHAS**, quebradas por `layout::wrap` — que parte por CLUSTER uma palavra sem
  espaços, que é o que um nome de arquivo é. E nunca três: o `wrap` para de EMPILHAR no teto dele e
  guarda o resto na mão, então responde com uma linha a mais do que se pediu, e ela cai na prateleira
  de baixo. O corte é nosso, e o que sobrou volta para a última linha para ser elidido ali — reticência
  onde o nome para, não um nome que simplesmente acaba. **No menor degrau ele também encolhe**
  (`Ink::Small`, do `paint::scale_in_px`, que é o único resolvedor que o `one-text-size` deixa ler um
  tamanho fixo), e a QUEBRA é medida nesse tamanho: contar células da grade para uma run menor deixa
  um quinto de cada linha vazio. **E a ENTRELINHA das duas é a da fonte pequena, não a da grade**
  (`Size::Stacked`, a quarta forma de `ronoctua_render::Size`): um nome que quebra é UMA coisa lida
  para baixo, e a três quartos do corpo duas linhas a uma linha da grade de distância deixam um
  quarto de linha de nada entre elas — o dono mediu na tela (*"a distância entre as duas linhas dos
  nomes … está muito grande"*), e são 4,35 px dos 26,1. A run continua NOMEANDO a linha da grade que
  ocuparia — a porta segue com uma `Row` por linha, e nenhum `hit`, faixa ou elisão muda —; o número
  diz quantas linhas atrás o bloco começou, e quem o desenha na entrelinha dele é o renderizador.
  `Stacked(px, 0)` é `Px(px)` exato, então nome de uma linha não se mexe, e a expressão vale nos dois
  sentidos: bloco MAIOR que a grade desce pela mesma conta.
  **E DUAS LINHAS DE NOME CUSTAVAM UMA PRATELEIRA.** A menor célula passou a ter 4 linhas (2 de
  figura, 2 de nome) e o rodapé com os painéis abertos deixava 7 — que não cabem duas. O teto da
  região subiu de 12 para 15 e agora sobram 10, que cabem: as três linhas que o dono pediu para a
  coluna de infos pagaram a segunda prateleira de passagem. Onde ainda não couberem — tela pequena,
  em que a percentagem manda —, o degrau escolhido é o MENOR: uma prateleira de nove arquivos e não
  de cinco.
  **E NUMA CÉLULA UM SOM SEM CAPA MOSTRA O GLIFO, não a onda.** A onda é uma figura de um SOM: lê-se
  num painel e é um borrão cinza em seis células. Quem diz isso é a AUSÊNCIA DE TINTA — o `wave` do
  pedido vai vazio, e o `pixels` recusa desenhar ondas sem ela. O painel continua passando a tinta e
  continua tendo as suas.
  **E O CACHE FOI O QUE FEZ ISSO NÃO ACONTECER.** O código estava certo e a tela do dono continuou
  mostrando ondas em três arquivos — *"as waves continuam"* —, porque as figuras já estavam no disco
  sob uma chave que diz QUAL arquivo, QUE tamanho e QUE tema, e nada sobre o que foi DESENHADO
  dentro delas. **Cache é a única coisa capaz de estar certa no fonte e errada na tela.** A metade
  durável do conserto é de quem pede (`View::grid_tint`, um lugar só, nomeia também a tinta da onda
  — que NÃO sai do tema: a grade não passa nenhuma seja qual for o tema, e isso é decisão de quem
  chama); o `VERSION` do nosso formato subiu para 2 e é o que joga fora tudo o que já estava escrito
  sob a regra velha, inclusive o que ninguém pensou.
  **E UMA FOTOGRAFIA COM FILME DENTRO ANIMA SOB O PONTEIRO.** Ela é um `image/*` por toda outra
  resposta — só o `Moving::inside` a distingue —, então um portão pelo media type passava por cima de
  todas elas. O trabalhador lê isso dos bytes do próprio arquivo (uma janela XMP e a cauda), sem
  `ffprobe`, e o passeio a toca INTEIRA em vez de espalhar quatro pontos por três segundos.
  **E O CACHE É O DO FREEDESKTOP**, o mesmo que o GNOME Files, o Thunar e o Dolphin escrevem:
  `$XDG_CACHE_HOME/thumbnails/<tamanho>/<md5 da URI>.png`, com a URI e o mtime da origem em chunks
  `tEXt` e o arquivo a `0600`. O trabalho é o MESMO trabalho — quem já abriu a pasta noutro
  gerenciador já pagou os 109 ms, e quem abre aqui primeiro pagou por ele. Custa um **MD5 escrito à
  mão** (os sete vetores da RFC 1321 são um teste) e a inserção dos `tEXt` no PNG à mão, porque o
  `image` codifica pixels e não expõe chunk — comprimento, tipo, dados e checksum, quinze linhas.
  Os nossos três degraus de decodificação (128, 256, 512) **são três dos quatro tamanhos que o padrão
  nomeia**, o que torna a troca gratuita em vez de uma reamostragem; um teste guarda isso.
  **O que NÃO vai lá é o que NÓS pintamos** — a onda de um áudio, uma peça 3D iluminada pela nossa
  lâmpada: não são miniaturas do arquivo em nenhum sentido que outro programa reconheça, mudam com o
  tema, e o padrão não tem onde dizer isso. Essas ficam no nosso formato, cujo cabeçalho carrega a
  tinta.
  **E a SEGUNDA visita é de graça**: `ronoctua_inspect::cache` guarda a figura em
  `$XDG_CACHE_HOME/ronoctua`, que é a QUARTA raiz XDG e nasceu aqui — um cache é o que se pode apagar
  e refazer do que ainda existe, que o estado não é. O nome é um FNV-1a de 64 bits do caminho (dez
  linhas, sem dependência), e **o caminho inteiro vai DENTRO do arquivo**: colisão e arquivo alterado
  dão a mesma resposta, que é MISS, e é isso que torna dez linhas de hash honestas em vez de uma
  aposta. Provado na tela com os cinco leitores apontando para `/nonexistent` — as figuras
  apareceram.
  **A ANIMAÇÃO TEM UMA REGRA SÓ: o que anima em PROCESSO anima sozinho; o que precisa de um PROCESSO
  anima sob o PONTEIRO.** GIF, WebP animado e APNG têm iterador preguiçoso dentro do `image`, então
  custam um descritor e um quadro; vídeo custa um ffmpeg. A fronteira não é arbitrária — é a única que
  existe. O animador segura até `space.files.thumbAnimators` iteradores, **um quadro na memória cada**
  e um relógio só; medido, 2,35 ms por quadro e 625 KiB de canvas por arquivo aberto, contra os
  **215 MiB** que guardar os quadros de cinquenta GIFs custaria — que é o que o dono pediu para isto
  não custar, nessas palavras. Um arquivo mais rápido que a taxa da célula é PULADO e não atrasado:
  ele leva exatamente o que o autor dele disse para dar a volta. Acima do teto a célula mostra o
  quadro parado **e diz isso com uma marca** — recusa desenhada é recusa; muda é defeito. Sob o
  ponteiro há um SEGUNDO `motion::Player` que passeia o filme em quatro pontos espalhados
  (`space.files.thumbClipTenths` em cada), porque o primeiro quadro é o que menos diz sobre um filme;
  **os quadros são o relógio** e nenhuma porta ganhou temporizador. Som NUNCA: som é do arquivo que se
  ESCOLHEU, e a mão atravessando uma pasta não é uma escolha.
  **E o teto de figuras da GPU é honrado dos DOIS lados**: o `Images::CEILING` conta uma tela de
  miniaturas (`space.files.thumbMost`) e a grade **sobe o tamanho** em vez de pedir mais células do
  que o renderizador desenha — porque o `prepare` descarta o excedente **sem nada em log nenhum**, que
  é o único defeito mudo desta feature. Contado: 1080p cabe 128 pequenas e 4K caberia 512, então 256
  deixa a menor alcançável na tela do dono e sobe a de 4K para a média, que é a resposta certa de
  qualquer jeito.
  **A linha que NOMEIA as colunas nasce escondida**, e `C-D` a mostra — o par shiftado do
  `C-d` como `C-S` é do `C-s`: um decide QUAIS colunas são desenhadas, o outro se os nomes delas
  são. A linha custa uma das doze do rodapé e o que ela carrega é alcançável sem ela (`C-s` e `C-S`
  ordenam, e a ordem se lê nas linhas). **E a seta é a única coisa ali com o acento** — o texto da
  coluna ordenada sobe um degrau da MESMA escala acromática das outras (`text.dim` → `text.primary`)
  porque uma palavra inteira no tom mais alto do sistema grita um nome de coluna para quem está
  lendo nomes de arquivo; o que se procura de relance é para que lado a ordem vai. São dois campos
  com o MESMO alvo: o que o olho lê como uma coisa é uma coisa de se clicar.
  Permissão, dono, grupo e DATA nascem OCULTOS na listagem (`C-d`) — o que a listagem mostra sem
  ser pedida é por onde se NAVEGA, e uma data de modificação é coisa que se olha —, e no painel de
  infos os campos são
  os MESMOS da linha — mesma `Hit`, mesma máquina. **E a ficha EXPANDE** (`C-W`, o par shiftado do
  `C-w` da prévia): a região inteira vira a ficha, e lá os fatos se dividem em quantas COLUNAS a
  largura couber — nunca mais do que eles precisam para caber nas linhas, senão uma coluna que
  ninguém enche é espaço em branco com uma borda irregular no meio. **A coluna é
  `space.view.labelCells + PANEL_FLOOR`** — onde toda lista do sistema começa o valor, mais a menor
  largura em que um fato ainda se lê (`Dimensions 3024 × 4032`, 23 células). Medida contra o TETO
  do painel ela dava quatro colunas de 43 na tela do dono, que leu e recusou: *"em vez de 4 colunas
  pequenas … 3 colunas maiores, pois assim o conteúdo das labels e valores maiores aparecerão com
  mais caracteres em tela"*. Colunas só na expandida: um painel ao lado da listagem já É uma coluna
  de página. A ordem é POR COLUNA — enche a primeira de cima a baixo e então a seguinte —, que é o
  que faz ler PARA BAIXO ser ler a ficha na ordem dela. O rótulo ganhou teto
  (`space.view.labelCells`): um terço de duzentas células poria uma palavra a sessenta e seis
  células do que ela nomeia. **E o GLOBO vira uma COLUNA ali**, a primeira, da altura do painel e
  com o endereço na última linha dela; de faixa no topo ele seria uma tira da largura da tela com um
  disco no meio e os fatos começando uma figura abaixo — o dono fotografou *"um enorme espaço em
  branco disperdiçado acima das colunas de conteúdo"*, e o mapa nem aparecia, porque o rodapé media
  o globo DENTRO da guarda do preview e com a região na ficha não há caixa de preview. Só vira
  coluna se ainda sobrar coluna para os fatos. **A ÚLTIMA LINHA é uma linha em branco do CONTEÚDO,
  nunca uma reserva do painel**: a ficha tem uma linha a mais do que tem fatos, e ela só entra em
  cena depois que tudo o mais foi rolado — é aí que o botão do canto não cobre nada. A primeira
  forma tirou a linha do PAINEL e o dono mediu o custo (*"você fez a coluna perder uma linha de
  informação em um espaço que já é pequeno"*): reserva custa um fato em TODA tela, linha no fim
  custa um só onde o botão cobriria algo. **E o hover do botão é a COLUNA INTEIRA, vãos inclusive**
  (`View::pane_at`): as duas portas perguntavam a seção a uma resolução funda demais — o rodapé ao
  `hit`, que exige um CAMPO, a janela ao `band_row`, que recusa linha sem campos —, e a ficha é
  quase toda célula vazia, então a resposta virava "lugar nenhum" no caminho da mão e o ícone sumia
  antes de ser alcançado. Agora é `Panes::section_at` mais a faixa da seção, e só os CAMPOS deixam
  de ser consultados. A região é UMA — a outra tecla TOMA em vez de abrir um segundo painel largo —,
  painel escondido RECUSA e diz por quê, e com a região dada não há régua vertical nenhuma,
  **nem a do FOCO**: `Panes::rules` já recusava e o `files::focus_rule` era a segunda porta para a
  linha em cell zero que o dono fotografou uma vez (*"no modo largo há uma linha vertical na esquerda
  que continua aparecendo"*), porque os dois pintores desenham a régua da seção focada mesmo quando
  `rules` não deu nenhuma. **As DUAS metades de um fato explicam, e em DUAS
  cores**: a legenda levava `explain: None` e o valor levava a frase inteira, e o dono leu uma
  legenda cortada e pediu as duas por completo no minibuffer *e distinguíveis*. Agora as duas levam a
  MESMA frase (uma só, construída uma vez — duas não teriam como divergir) e a linha de mensagem
  aprendeu a levar PEDAÇOS: o que NOMEIA o fato sai em `Register::Prose` e o que É o fato em
  `Register::Said`, que é o par que a linha de dicas já põe lado a lado e que todo tema já mantém
  separado. Qual metade é qual se PROVA: a legenda chega no argumento `$label`, e `$label` é
  interpolado por uma chave só em toda a árvore. E isso apagou o SEGUNDO pintor daquela linha — ela
  tinha dois, um por `hint_spans` e outro achatando tudo numa string com um token escrito à mão.
  **E a data sob o ponteiro é a data INTEIRA** (`chrome::when_long`): era o resto do minuto colado a
  uma coluna que o olho já tinha lido (*"e 20 segundos"*), e agora é o mês por extenso com o segundo,
  na ordem que a pessoa escolheu — doze meses numa chave só, por seletor Fluent, porque quais números
  ganham palavra é assunto do idioma. A linha da ficha leva as MESMAS duas chaves da coluna
  (`Fact::says`, que não é um `Doable`: numa data não se FAZ nada), e o `Civil` ganhou o segundo que
  ele já calculava e jogava fora. **A prévia mostra DOCUMENTO** — 13 tipos,
  `document.rs`: os seis do office são zip de XML, planilha vira TABELA, `.rtf` tem leitor próprio
  (`rtf.rs`, e o `\uc` é pago pelo `\'hh` ao lado — sem isso toda linha de lista perdia a primeira
  letra; o `\listtext` é maquinária e sai, porque a marca dele é PUA e a PUA é dos NOSSOS ícones),
  e os DOIS OLE que têm texto são lidos AQUI (`ole.rs` abre o compound file; `ppt.rs` anda os
  registros, `doc.rs` a tabela de pedaços). Medido contra o binário que o flake PINA — e a medição
  anterior estava errada, feita contra um `catdoc` de outro nixpkgs: `catppt` devolve **zero bytes
  com saída zero** e `catdoc` devolve **zero bytes com saída 69** nos arquivos do dono, que leu a
  frase na tela duas vezes. Só o `xls2csv` funciona (364 057 bytes), então são SEIS programas e o
  pacote fica por causa de um só. Na apresentação só as transparências contam — 14 dos 24 átomos de
  texto são do slide-mestre; no documento o texto vem pela tabela de pedaços, porque `fcMin..fcMac`
  acerta o Word 6 e erra o 97. **E a playlist tem
  QUATRO grafias** (`m3u`, `pls`, `xspf`, `asx`); a do iRiver é a única recusada por nome, porque é
  binária e sem especificação. **A prévia mostra TEXTO** quando o arquivo é
  texto (`is_a(mime, "text/plain")`, 277 tipos pelo DAG do freedesktop), com realce e rolagem: setas
  quando ela tem o foco, `C-,` pagina e dá a volta, `C-[`/`C-]` movem a linha para os lados, e a
  barra aparece sozinha porque a seção passou a ter `total`. **Barra nenhuma
  desenha por cima do conteúdo** — a que rola vertical tira uma CÉLULA e a que rola
  horizontal tira uma LINHA, e só quando existem; a listagem e a lateral já
  recuavam por `EDGE_RIGHT`. **A linha longa tem barra de baixo** (`Scroll::bar_across`,
  o mesmo `thumb` com os eixos trocados): teclado sem mouse é meia vista, e o
  arrasto passou a ter EIXO. Tetos: 1 MiB, 2 000 linhas, 512 bytes
  por linha — o último porque `.js` minificado é UMA linha de 2 MB e o `elide_end` a percorreria por
  cluster a cada frame. **TODA linha da prévia passa por UMA função** (`text::flatten`): ela era
  privada do leitor de texto, e o de DOCUMENTOS montava as suas sem ela — duas réguas de novo, e
  esta com dente. O painel orça em CÉLULAS (o tab vale UMA) e o shaper o leva ao próximo stop, então
  linha com tab era medida cabendo e desenhada vários cells maior, POR CIMA da coluna ao lado; o
  dono fotografou no `sample.rtf`, cuja lista é indentada por `\tab`. **Arquivo sem imagem E sem
  texto diz o que a pasta diz** (`sem prévia`); a frase sobre imagem morreu. **Recusa é alinhada à
  ESQUERDA e o `sem prévia` fica centrado** — a segunda é um rótulo de duas palavras no meio de um
  painel vazio, a primeira é uma FRASE que quebra em quatro linhas, e centrada dá a cada linha uma
  margem diferente. **A prévia é um MINIPLAYER** (2026-08-30, a pedido do dono, que
  reverte a frase "um reprodutor que ninguém pediu"): GIF pelo `image`, vídeo por um ffmpeg VIVO
  cuspindo RGBA cru; um player (`motion.rs`), uma thread, um quadro por vez, e **o quadro chegando
  É o relógio** (nenhuma porta ganhou timer). **O TAMANHO DO QUADRO É NOSSO**, não do arquivo:
  `scale`+`pad` para o `Fit` que o painel mediu, então `bytes` é aritmética. Perguntar largura ao
  ffprobe teria trocado o defeito por outro — vídeo de celular diz 640×360 e RENDERIZA 256×456.
  **E o quadro tem o FORMATO DA CAIXA** (largura quantizada, altura seguindo a caixa), senão a
  imagem é enquadrada DUAS vezes: 380×300 virava quadro 384×128, o ffmpeg letterboxava um 16:9 ali
  dentro e a porta enquadrava de novo. Como o quadro já vem enquadrado, a porta o desenha
  PREENCHENDO a caixa (`preview_fills`); só a foto parada ainda passa pelo `fit_into`. O GIF é
  compostado no mesmo tamanho, para as duas fontes darem UMA forma de quadro.
  Letterbox TRANSPARENTE: a cor seria uma das DUAS (rodapé e janela), e a textura é uma só.
  **O relógio é o de PAREDE e quem atrasa perde quadro** (sono fixo depois do trabalho corria 17%
  devagar, e acumulava). **Pausar é PARAR DE LER** — o ffmpeg trava no pipe, 0% de CPU, e o som para
  junto porque sai do MESMO filho (`-f pulse default`, uma segunda saída); `SIGSTOP` estalaria o
  áudio. Buscar, ligar o som e redimensionar são respawn (108-242 ms); o teto de pausa é 5 min.
  **Som em filme mudo MATA a imagem** — o ffmpeg recusa o comando inteiro —, então a tecla recusa e
  o glifo não aparece. **Fim é MENSAGEM** (`Beat::Ended`): ausência de acordada não se desenha.
  Nenhuma leitura sem prazo (`poll`, 50 ms) e nenhum `kill` à mão (guarda de `Drop`): o
  `read_exact` de antes matava a thread do player pelo resto da sessão, mudo. **Os controles ficam
  POR CIMA do filme** — a imagem leva o painel inteiro — numa faixa `surface.transport` (alfa `CC`,
  a única do sistema): sem teste de profundidade, a ordem é retângulo→imagem→texto, então a cauda da
  lista desenha depois das imagens (`Quads::draw_range`). **TUDO some sem o ponteiro**, linha do
  tempo inclusive; as CÉLULAS ficam, senão a trilha mudaria de tamanho no hover e o cursor escaparia
  de baixo do clique. A linha dos controles tem campo de fundo SEMPRE — sem ele o `hit` devolvia
  `None` ali e os controles fugiam da mão que ia pegá-los. Trilha centrada na linha e com ar igual
  dos dois lados, o que obriga a MEDIR o relógio (`1:23 / 5:40` e `3:44 / 10:34` não têm a mesma
  largura). `C-;` `C-'` `C--` `C-=`, e `C-w` dá a região INTEIRA (derivado, então se desfaz sozinho)
  — **para QUALQUER prévia**, sobrevivendo à troca de arquivo: as setas andam nos ARQUIVOS, e sem
  prévia a região diz o nome (ou o caminho, se for pasta). Lá a prévia pede TODAS as linhas; pedindo
  as 4 de `preview_lines` ela ficava no topo, com os controles na quarta linha e a faixa deles
  cobrindo o filme. A faixa vai até o fundo REAL do painel — o resto de linha ficava sem tinta e sem
  hover —, e o `hit` dá esse resto à última linha; a linha dos controles é `foot: true`, contada de
  BAIXO PARA CIMA, então o resto cai ACIMA dela, dentro da imagem — é o `Span::edge` no outro eixo,
  e a mesma razão. **TODO o transporte sai do mesmo pé** (`transport_top`): ancorar só a LINHA
  deixou a trilha contada do topo, a um resto de distância dos glifos. **A trilha usa as CÉLULAS dos
  campos** (`Section::fields_px`) e não os trilhos da
  seção, que são mais largos: num painel largo a diferença era a folga até o relógio. **Clique na
  prévia NÃO abre** — em filme alterna o play, e o alvo cobre a linha inteira dos controles.
  **Respawn devolve o filme como estava**:
  `Ask::Play` leva se começa SEGURADO (fora do `Reel`, senão pausar viraria respawn) e adianta a
  marca para onde a imagem está; sem isso o `C-w` despausava e retrocedia.
  **Faixa de hover não vai sobre figura** — ela diz que LINHA de uma lista está sob o ponteiro, e
  foto não é lista; e a pergunta é por CÉLULA, a mesma entrada que a faixa usa. **FOTO COM FILME DENTRO** (motion photo): a prévia mostra a foto 1,5 s, depois o filme, e
  repete — `Beat::Still` é quem devolve a foto, porque ausência de quadro não se desenha. Achada
  pelos BYTES e não por fabricante: janela XMP de 128 KB (o `Item:Length` do Google) e, senão, um
  `ftyp` na cauda — que cobre a Samsung sem um parser que eu não possa testar. Apple é OUTRO arquivo,
  e o par se confirma pelo `ContentIdentifier`, nunca pelo nome. Lida por `subfile` do ffmpeg, sem
  copiar os megabytes. **A foto e o quadro são DUAS texturas** (`:frame`): com uma só, a volta do
  laço acharia a chave presente, não subiria nada, e a tela ficaria no último quadro para sempre.
  **Arquivo que se
  mexe não ganha foto** (o pôster era um ffmpeg inteiro para uma imagem trocada 84 ms depois) e
  **enquanto a resposta não chega o painel fica VAZIO**, nunca "sem prévia". Uma textura reenviada
  sob a MESMA chave, e agora a MESMA textura — chave por quadro encheria as 39 fatias com o filme
  de um arquivo. **As seções são
  INDEPENDENTES**: `Panes::section_at` diz em que seção um cell está, e é quem a
  faixa, a roda e o foco perguntam — painel acende a si mesmo, e o gesto rola só
  a seção sob o ponteiro (a listagem é da PORTA; **figura** não rola, e engole).
  Barra lateral tem `side_offset` PRÓPRIO: linha `n` deixou de ser a linha `n`
  dela. **Clique SELECIONA; o segundo é que abre** — na listagem e na lateral.
- **A COORDENADA VIRA UM GLOBO, e o globo não é do gerenciador** (`ronoctua-atlas`): crate PURO —
  `no_std`, **zero dependências**, devolve um `Ink` e nunca uma cor, o molde exato do
  `ronoctua-syntax` —, mais `shell/globe.rs`, que tem a thread, o `tiny-skia` e o
  `chrome::ink_color` (um `match` exaustivo, ao lado do `syntax_color`). O gerenciador é o PRIMEIRO
  chamador; o relance, o `LEADER s l` e o instalador já seguram coordenada e não têm o que mostrar
  — o instalador diz no próprio comentário que joga fora *"as coordenadas e os códigos de país de
  um mapa que ele não desenha"*. **A tabela é assada na build** (`nix/atlas.nix`: Natural Earth em
  GeoJSON + GeoNames `cities500`) e entra pelo `atRunTime` — nunca pelo `generated`, que custaria os
  2m15–5m46 de reconstrução; **ausência não é erro** (regra do `RONOCTUA_COVERAGE_FONTS`), e aí a
  ficha mostra a coordenada sem globo, com frase. **10,70 MB medidos**, com a quantização em `i16`
  (611 m na longitude, 305 m na latitude) que fica ABAIXO da resolução do dado 1:50 m — e o
  admin-1 vem a 1:10 m porque o 1:50 m tem só 294 feições e Londres saía sem região. Medido de
  ponta a ponta contra a tabela real: *"United Kingdom / Westminster / London a 320 m"*, offline.
  **O horizonte é a única geometria de verdade nova**: descartar os vértices invisíveis desenha uma
  CORDA através do planeta (a África ganha um lado reto e lê-se como dado ruim), então cada aresta é
  cortada por bissecção no grande círculo e o caminho anda pelo ARO entre a saída e a entrada.
  **Zoom é escala e pan é centro** — o passo do pan divide pelo zoom e pelo `cos φ`, senão um globo
  aproximado sai da tela num gesto. **A figura são as PRIMEIRAS LINHAS DO CONTEÚDO da ficha, e elas ROLAM.**
  Eram `Section::above` — fixas, como o caminho e o cabeçalho da listagem —, e isso custou duas
  coisas que o dono achou: banda fixa deixa o trilho do polegar (`Section::visible`) mais curto que o
  painel enquanto o ARRASTO divide pelo painel (`Room::panel`), então pegar a barra num arquivo com
  globo a jogava para longe da mão; e o globo custava seis das doze linhas do rodapé enquanto o
  arquivo estivesse sob o cursor. Rolando, ele devolve as linhas — e foi isso que aposentou o `C-g`,
  que era um segundo caminho para o que arrastar a barra já faz (*"nesse caso o atalho de exibir e
  esconder o globo não precisaria existir"*). **E a figura MANTÉM O TAMANHO enquanto sai**, o que custou
  DOIS consertos. Nada neste renderizador recorta — imagem meio fora do painel é desenhada POR CIMA
  da listagem, não cortada —, então o corte tem de acontecer onde a figura é PINTADA: ela é sempre
  projetada na altura inteira e o `tiny_skia` a assenta transladada de `−from`, recortando ao
  pixmap. Só isso não bastou: a textura era pedida no tamanho da caixa COM o ar que o `picture_in`
  cede nos quatro lados, e o `Area::centre` a reduzia para caber na caixa SEM ele — fator
  `(h − 2·ar)/h`, dois por cento numa caixa alta e um terço numa baixa, invisível enquanto a caixa
  não se mexia. Medido na sabotagem: 0,911 → 0,889 → 0,852 → 0,778 conforme as linhas saíam. A
  subtração mora num lugar só (`files::globe_texture`), que é o que a prévia sempre fez
  (`measured_preview`) e a porta do globo nunca fazia. **E o deslocamento da ficha é limitado na LEITURA** (`facts_offset`): o que
  o torna grande demais não é uma escrita, é a GRADE mudando embaixo dele — `C-W` transforma uma
  coluna de quarenta linhas em três de catorze —, e limitar na leitura ainda guarda o lugar para
  quando a região voltar. **A chave da
  textura tem QUATRO partes** (dono, assunto, tamanho, tema): sem o dono, duas vistas com a mesma
  coordenada no mesmo tamanho e tema reescreveriam a textura uma da outra a cada quadro, sem nada em
  log nenhum — por isso o `Owner` existe com a segunda variante antes de ela ser alcançada. E o
  `Images::CEILING` subiu, porque o `prepare` DESCARTA o que não cabe em silêncio: era o único
  defeito desta entrega sem sintoma. **A atribuição do GeoNames (CC-BY) viaja em quatro lugares**,
  um deles DENTRO da própria tabela, para que uma cópia do arquivo nunca se separe da licença.
- **`LEADER s g` são as RUAS POR REGIÃO, e elas não são do gerenciador** (`geo.rs`, duas portas:
  `s g g` no rodapé, `s g w` em janela): o que está instalado, o catálogo das **555 regiões** que a
  tabela assada carrega — e as **29 que a Geofabrik publica SEM shapefile**
  (Brasil, Alemanha, França, Rússia, EUA, os continentes) são DESENHADAS
  recusando, dizendo que a região sai em partes: deixá-las de fora foi o que
  fez o dono não achar o Brasil na lista, e de fora linha ausente e tecla morta
  são a mesma coisa. O rótulo de uma parte CARREGA o país (`Sudeste, Brazil`),
  que é o que a torna legível e o que faz o filtro achá-la pelo nome dele, um filtro que se digita no MINIBUFFER (a janela diz isso) e `d` para
  remover, atrás de uma pergunta que abre no NÃO. **Um pacote é uma região da Geofabrik**, e o que
  se guarda dele são DUAS das 101 camadas (`roads`, `places`) e DUAS das dez colunas (`name`,
  `fclass`) — medido em Andorra: 6,3 MB no fio, 13,8 MB abertos, 34% nas duas camadas, 27,8% das
  8 799 vias com nome. **Quem baixa e assa é a FILA** (`Work::Atlas`, motor próprio): o `-latest-`
  é REDIRECIONAMENTO e o nosso HTTP não segue um — e o nome DATADO para onde ele aponta é a versão
  do pacote, a única que existe. Um trabalho só, do jeito do `Work::Media`, porque o estado do meio
  (um zip que não é de ninguém) é o que uma fila existe para não deixar. **O leitor de `.shp`/`.dbf`
  mora no `ronoctua-atlas-packs`** com as cinco armadilhas no topo (cabeçalho big-endian e registros
  little-endian; comprimento em PALAVRAS de 16 bits; o `.dbf` casando por POSIÇÃO e por nada mais —
  contagens diferentes são RECUSA, porque a alternativa é nomear cada via com o nome da vizinha e
  isso se lê como verdade; campo de largura fixa com espaço; e o shape tipo 0, que é legal e não tem
  geometria). **O zip DESCEU de crate** (`ronoctua-zip`): um pacote é um zip e o `ronoctua-inspect`
  não podia ser dependência de quem assa — cobra o `one-zip-reader`, que também recusa desenho no
  caminho da fila. **O pacote responde por GRADE** (célula de 0,01°, segmento arquivado em toda
  célula que TOCA, teto de 2 km) e os segmentos ficam no DISCO: só o índice e os nomes sobem à
  memória. **E o pacote só ACRESCENTA**: a rua e o bairro entram no endereço, e o bairro só
  substitui a cidade quando está mais PERTO — um pacote nunca piora a resposta que havia sem ele.
- **A prévia 3D GIRA, e são ONZE formatos** (`ronoctua-inspect`: `mesh/` lê — um por leitor —, `solid`
  desenha; `shell/stage.rs` é a thread, molde do `motion.rs`). **Nove são MALHA** (STL, 3MF, OBJ, PLY,
  OFF, glTF, GLB, COLLADA, AMF) e **dois não têm triângulo NENHUM**: STEP e IGES são B-Rep. **O STEP é
  MALHADO** (`mesh/brep.rs` anda as faces, `surface.rs` sabe ir e voltar, `patch.rs` triangula); o
  IGES fica no arame, que é o desenho das arestas — não tenho `.iges` para provar contra. **Uma
  câmera, um `Body` com DUAS metades**: preenche e depois traça, no mesmo z-buffer.
  Malhar uma face é levar o contorno dela ao `(u,v)` da SUPERFÍCIE dela, triangular o plano que sai
  (com furos, por ponte) e mapear de volta — todo vértice sai de `Surface::at`, logo fica EXATAMENTE
  sobre a superfície. **Cada aresta é amostrada UMA VEZ**, guardada pela id: duas faces que se
  encontram têm de receber os MESMOS pontos. **A emenda desdobra ANDANDO** (`atan2` responde numa
  volta só). **Só cinco superfícies têm inversa fechada**; extrusão, revolução e NURBS usam UMA
  Gauss-Newton genérica, semeada pelo ponto ANTERIOR do contorno — vizinhos no contorno são vizinhos
  na superfície, e é isso que tira o ramo errado. **A genérica é medida contra a fechada** — é o que
  torna a metade cega confiável. **Toda resposta é reposta pela ida**: a inversa da esfera responde
  para qualquer ponto, inclusive fora dela.
  **O refino é UNIFORME** (1→4, k vezes) porque dividir só aresta interna NÃO CONVERGE: o ear clipping
  faz leque, e a aresta mais longa da lasca é a do contorno — medido, o resíduo ficou entre 1,7× e
  9,3× o alvo com qualquer número de passadas. O preço é que duas faces deixam de se encontrar
  exatamente (um milésimo do raio, sub-pixel), e por isso **B-Rep não relata volume, área nem
  "fechada"** — nem triângulos, que seriam do malhador. **A MELHOR orelha, não a primeira**, e
  **a profundidade sai dos TRIÂNGULOS, não da face** (o aro já vem denso), com a tolerância como
  FRAÇÃO da peça — as três juntas: 359 mil → 27 mil triângulos e 39 → **18 ms** no arquivo do dono.
  Teto de 300 mil triângulos por PEÇA, dividido entre as faces; é limite de QUADRO.
  **Face que não sai vira contorno ELA SOZINHA**, com fato e frase na prévia (última linha, por cima
  da figura — a ordem é retângulo→imagem→texto). Toda falha cai nesse degrau, que é um desenho que já
  funcionava: 1 face de 106 no arquivo do dono, 69 de 2 536 num eixo traseiro.
  Seis tipos MIME já existem na base; cinco nascem no `ours` **com `under`** (`model/x-ply`,
  `model/x-off`, `model/vnd.collada+xml` — o nome REGISTRADO —, `model/x-amf`, `model/step`).
  **`*.obj` tinha DOIS reivindicantes no mesmo peso** e o `by_name` fica com o primeiro: resolvia para
  `application/x-tgif`, sem prévia e sem fatos. Conserta o bloco `prefer`, que REMOVE a linha do
  perdedor e recusa por tipo inexistente, por tipo que não reivindica o padrão e por pesos que já
  diferem.
  Armadilha por leitor, no topo de cada arquivo: **STL** decide a forma por ARITMÉTICA
  (`84+50n==len`); **3MF** guarda a geometria noutra parte (`p:path`) com transformação que MUDA a
  forma; **OBJ** tem índice NEGATIVO relativo ao fim da lista naquela linha; **PLY** obriga a entender
  a propriedade que não se quer, para pular pela largura certa; **glTF** é ÁRVORE (sem compor os nós
  tudo empilha na origem), sua unidade é o METRO, `sparse` é APLICADO, e é o único que recebe CAMINHO
  — logo o único que recusa `..`, absoluto e esquema; **COLLADA** obedece `unit` e `up_axis` (`Y_UP` é
  o padrão, e ignorá-lo deita a peça) e seu `<p>` é ENTRELAÇADO; **AMF** é zip OU XML cru sob a mesma
  extensão; **STEP** tem CIRCLE inteiro que só o `EDGE_CURVE` recorta (`same_sense` diz por qual
  lado), cabeçalho que NÃO é UTF-8 — lido como Latin-1 e **MARCADO**, porque campo adivinhado não vira
  fato na tela —, e **spline em DUAS grafias**: a racional é entidade COMPLEXA, e ler só a simples
  devolvia CORDA no lugar da curva — o que no arame é aproximação e no preenchido é fatal, porque a
  corda não está na superfície e a inversa recusa (28 das 77 faces do buggy, e o sintoma apontava para
  a superfície, que estava certa). Draco é `Refusal::Unsupported` com frase própria, nunca meia peça.
  A normal do arquivo é sempre ignorada e recalculada. Unidades viram mm na entrada; teto sobre o que
  SAI da descompressão. Rasterizador em processo (precedente: `from_svg`), **fundo TRANSPARENTE** (uma
  textura, duas portas), luz fixa na CÂMERA. **44 ms** no Benchy de 225 mil triângulos, **9 ms** no
  OBJ de 50 mil; face de trás NÃO é descartada — vale 1,5×, e abre buracos em malha com faces
  invertidas. O corpo mora na THREAD (o `Looked` é comparado campo a campo). Arrastar gira, roda dá
  zoom, `S-<setas>` giram, `C-k`/`C-j` zoom, `C-r` reenquadra; as setas PURAS continuam andando nos
  ARQUIVOS. **A porta pergunta `live_frame`**, não `moving_frame`. **`shift` só vinha do keysym** —
  certo para caractere, e `Shift+Left` é `Left`: subia um diretório. **Forma não é TEXTO**, e três
  tipos descendem de `text/plain`: sem o `shape.is_none()` o `.obj` de 4,3 MB era lido, realçado e
  jogado fora. Fatos saem da MESMA leitura, com sanidade por arestas ORDENADAS.
  O `fluent-bundle` não localiza número: `Arg::Fraction` formata onde o idioma é conhecido. **Direção
  de arrasto é da `View::spin`**, que fala em ONDE A MÃO FOI (`right`/`down`): yaw maior escorrega a
  peça para a ESQUERDA, então puxar para a esquerda é yaw MENOR — escrito em dois sítios, saiu
  invertido num.
- **Todo cursor é um ASSENTO, e toda porta ouve todos eles** (`seats.rs`). O compositor desenha um
  cursor por *seat* e cada um move o próprio foco — duas janelas focadas ao mesmo tempo, uma por
  mão —, e o `ronoctua-pointers` põe cada mouse e cada touchpad num assento próprio ao ser plugado
  (`seat0` fica com os teclados; os outros são `ronoctua-cursor-N`). As 15 portas guardavam UM
  ponteiro (`if self.pointer.is_none()`), e o dono mediu o resultado: o segundo cursor andava e
  nada respondia. Agora cada porta agrupa o estado de ponteiro num `PerSeat` e o
  `Seats::switch_to` o troca no começo de todo `pointer_frame`, então a lógica da porta roda
  inalterada sobre o estado DO cursor que falou — dois arrastos, dois hovers, dois restos de roda,
  sem um enxergar o outro. O hover de cada um é pintado na cor dele (`cursor.hover.*`) só quando
  há mais de um; sozinho, é o `surface.hover` de sempre. O arrasto de arquivo usa o
  `wl_data_device` DO assento que apertou, e um clique só lê os modificadores do teclado do mesmo
  assento. Cobra o `every-door-hears-every-seat`. **E cinco limites são do compositor, e não se
  cruzam sem patch** (inv. 3): `focus_follows_mouse` é global; só o foco de UM assento ganha a
  borda `client.focused`; um cursor que sai de uma janela a desativa mesmo com outro em cima;
  uma superfície `exclusive` toma o teclado de TODOS os assentos; app estrangeiro que ouve um
  assento só (GTK, Qt) responde só ao primeiro; e **dois mouses do MESMO modelo são um cursor
  só** — o identificador é `vendor:produto:nome` e o `attach` recebe identificador, então nada
  deste lado os separa (medido na VM: dois virtio-mouse, um assento).
  **E O TECLADO ESCREVE ONDE O ÚLTIMO CURSOR A CLICAR FOCOU** — o padrão, escolha do dono
  (`typing.rs`, a linha `A digitação vai para`, que também aceita o cursor 1 ou um fixo): os teclados ficam no `seat0`, e só o foco DELE recebe tecla.
  Seguir outro cursor NÃO move teclado nenhum — o evento `focus` do compositor sai para QUALQUER
  assento, e comando por IPC roda no `seat0` (`execute_command` sem assento), então quando o
  cursor seguido foca uma janela o rodapé pede ao `seat0` que foque a mesma. Só na MUDANÇA do
  cursor seguido, senão uma janela escolhida pelo teclado (`LEADER w n`) voltaria sozinha.
  Medido na VM: o cursor 4 clicou o terminal da direita e o `xyz` caiu nele.
  **Detecção e foco-segue-o-ponteiro nascem DESLIGADOS** (escolha do dono): um cursor para
  todas as mãos, e um clique foca.
  **O ponteiro virtual do keyd não é uma mão** (`plan::follows_the_keyboards`, vendor `0x0FAC`):
  tomado por "o primeiro ponteiro" ele segurava o `seat0` com um cursor parado num canto, e o
  mouse de verdade ia para o cursor 3 — medido na VM. **E um aparelho físico é UM cursor, por
  quantas interfaces tiver** (`plan::one_physical`, o mesmo `vendor:produto`): o touchpad chega
  como `…_Touchpad` e `…_Mouse`, e a segunda fica parada — um cursor para cada deixava um
  cursor morto ao lado do touchpad. **E a `.#vm` só mostra vários cursores com `-- --pointers`**:
  o QEMU junta todo ponteiro do host num aparelho emulado, e cada um passado como
  `virtio-input-host-pci` chega ao convidado como ele mesmo (e fica PRESO a ele até ela fechar) —
  menos o touchpad EMBUTIDO, que fica com o host e é o que sai da janela — com TODAS as
  interfaces dele (o mesmo pai no `/sys`): o `…Mouse` que nunca se move, passado sozinho, era
  um cursor parado. **Os ponteiros do
  próprio QEMU são EMULADOS** (`RONOCTUA_POINTERS_EMULATED`, `nix/modules/vm.nix`): um cursor
  só, DESLIGADO enquanto houver uma mão de verdade e ligado quando são o único ponteiro, e a
  linha do `LEADER s i` os liga. **Cursor que nada move não é desenhado**: o assento desenha
  o cursor enquanto tiver QUALQUER ponteiro, até desligado, e não há *detach* — então ele
  ganha `hide_cursor 100` NO assento dele, depois do `seat *` que o compositor mescla em todos.
  **O número de um cursor é MEMÓRIA, e a tela não o desenha** (`plan::Shown`): um dispositivo
  ausente guarda o dele para voltar ao mesmo cursor, e desenhado como é isso abria buracos
  (cursor 1, cursor 3). As duas telas numeram os cursores que EXISTEM 1, 2, 3… e traduzem de
  volta pela MESMA numeração — a do teclado, na fronteira do hospedeiro, guardada com o que
  foi mandado. **E `Todo mouse e touchpad move um cursor só` é o interruptor de verdade**
  (`together`): põe todo dispositivo no cursor 1 SEM esquecer onde cada um estava — o
  `Dispositivo novo ganha cursor próprio` só decide onde um NOVO vai.
  **`LEADER s i` são os ajustes** (`pointers.rs`, duas portas: `s i i` e `s i w`): detecção
  automática, cada cursor (ligado, cor, que dispositivos move — `RET` num dispositivo o passa ao
  próximo cursor ou a um novo), todo ajuste que o libinput diz que AQUELE dispositivo tem (os que
  ele não tem não são desenhados), e o que vale para todos: tamanho, esconder parado ou digitando,
  foco segue o ponteiro. **Cada ordem lê o `pointers.json`, muda uma coisa e escreve** — nenhuma
  porta guarda cópia entre ordens —, e só a thread do RODAPÉ detecta dispositivo novo. Tirar um
  dispositivo de um assento é RELOAD (o compositor não tem *detach*); pôr num novo é IPC na hora.
- **Os teclados têm DOIS MODOS, e o interruptor é UM** (`LEADER s k`, a primeira linha):
  **Juntos** é o keyd como sempre — um teclado virtual, no `seat0`, um layout para todos — e
  **Independentes** troca o keyd pelo `ronoctua-keys` (`nix/modules/keys.nix`, `Conflicts=` dos
  dois lados): cada teclado UNIDADE é agarrado (`EVIOCGRAB`) e reemitido por um gêmeo uinput
  próprio (`ronoctua keyboard <identidade>`, vendor 0, e a identidade é o `phys`+`uniq` — dois
  do mesmo modelo são dois), que o `ronoctua-pointers` prende ao assento do cursor dele, com
  layout, repetição, remaps e tecla leader PRÓPRIOS. **Trocar de modo é NA HORA**, a pedido
  do dono: a troca já perguntou, com 15 s para voltar sozinha, por medo de um remapeador que não
  sobe — e essa máquina sem teclado não acontece, porque **morrer DEVOLVE o teclado**: o kernel
  solta o grab quando o descritor fecha, e um que não volta chama o keyd (`OnFailure=`). **E
  nesse modo o rodapé NUNCA pede o teclado Wayland**: uma layer `exclusive` toma o de TODOS os
  assentos (`layer_shell.c:130`), então o toque da leader de um teclado vai ao rodapé pelo
  SOCKET do daemon, que DESVIA só aquele teclado enquanto a região está aberta (`keys_link.rs`,
  com o xkb DAQUELE teclado) — os outros continuam digitando nas janelas deles, e a linha do
  minibuffer veste a cor do cursor dono. **Toda porta ouve todo teclado** (`Seats<_,
  WlKeyboard>`, cobra o `every-door-hears-every-keyboard`), e um buffer sabe DE QUEM é a próxima
  tecla (`keyboard-seat`): o `buffers/example` (`example-open`, só por nome) é a REFERÊNCIA, uma
  seleção e um campo por teclado, cada um em `cursor.selection.*` (`ronoctua_design::Carets`).
  O `--keyboards` da `.#vm` passa os externos e deixa o embutido e toda chave de segurança (a
  interface de smartcard, `0b`) com o host.
- **As cores dos cursores são as MESMAS em todo tema** (`cursor.fill.*`, oito, de uma matiz fixa;
  o nono repete o primeiro): a cor é como a pessoa sabe qual mão é qual, e trocar de tema não
  pode repintá-la. Só o hover é por tema, porque deita na superfície do tema. As formas são o
  Bibata Modern recolorido na build (`nix/cursors.nix`, a mesma fonte que o nixpkgs pina) — oito
  temas `ronoctua-pointer-N`, 43 MB com quatro tamanhos (24, 32, 48, 64; com 16 e 96 eram 92).
- **Toda porta segue a ESCALA da tela em que está** (`hidpi.rs`, uma regra e dez chamadas):
  `scale_factor_changed` estava VAZIA nas nove daquele dia, ninguém chamava `set_buffer_scale`, e num monitor que
  o compositor dirige em escala 2 cada porta entregava um buffer do tamanho LÓGICO que o compositor
  esticava — letra borrada, e nada na tela dizia por quê (as colunas estavam certas, o layout também;
  lê-se como fonte ruim). Três coisas têm de concordar ou o compositor recusa ou estica: o que a
  superfície diz que é a escala, o tamanho do buffer, e o tamanho em que os glifos são rasterizados.
  **Só isso é físico**; `Span` é coluna e linha, `Quad` é retângulo sobre a janela, ponteiro chega em
  lógico, e `metrics::columns` não se mexe — a mesma tela mostra as mesmas colunas nítida ou não. O
  `Text` recebe a escala e multiplica; quem desenha retângulo ou imagem passa o viewport LÓGICO, senão
  a barra cai a uma fração do lugar. **O `set_buffer_scale` viaja no commit que leva o buffer** — ele
  é double-buffered, e um quadro pulado (`Occluded`, `Timeout`, `Outdated`, `Lost`) o deixaria
  pendente para o próximo commit qualquer, aplicando escala nova sobre buffer velho: `invalid_size`, e
  o compositor fecha a conexão sem dizer nada. Por isso vai no gancho de `Gpu::frame`, que só roda em
  quadro que apresenta. **O rodapé tem uma escala POR MONITOR** (uma superfície por saída), casada
  pela superfície como o `configure` já casa. **E toda porta guarda DOIS tamanhos lógicos**: `asked`,
  o que o compositor configurou, e `width`/`height`, o que o buffer CABE mostrar — iguais em qualquer
  máquina que exista, e diferentes exatamente onde o teto de textura da GPU corta. Um número só não
  responde às duas perguntas: pedir à GPU é função do que o COMPOSITOR disse, e diagramar é função do
  que o BUFFER tem; realimentar o próprio resultado é catraca (janela cortada uma vez em escala 3 pede
  o tamanho a que foi cortada e nunca mais cresce). `followed` DEVOLVE esse tamanho (`#[must_use]`) —
  por uma rodada ele jogou fora o que `resized` respondia, e as dez portas diagramavam contra o
  tamanho anterior à mudança de escala, justamente no caminho para o qual o módulo existe: janela
  arrastada entre dois monitores, sem `configure` depois. O teto do fator mora num lugar só
  (`ronoctua_render::MOST_SCALE`), que o `render-png --scale` lê também. Medido sob Xvfb com a
  saída em `scale 2`, na coluna da janela de energia: pixels acesos 20,19% → 11,35%, pixels de tinta
  cheia 1,27% → 2,63%, halo 93,7% → 76,9% dos acesos. Cobra o `every-window-knows-its-scale`, que
  também LÊ o `hidpi.rs` — a varredura pulava todo arquivo sem `CompositorHandler`, e era lá que o
  defeito estava.
- **O teclado agora REPETE, e não repetia em porta nenhuma** — as duas criavam com `get_keyboard`, e o
  SCTK só repete com `get_keyboard_with_repeat`; o `repeat_key` das duas era código morto e segurar
  `j` nunca andou na lista. A repetição chega pelo CALLBACK, não pelo método, então cada porta tem um
  `typed` por onde passam as duas entradas. O rodapé cria o `calloop` ANTES do roundtrip: é nele que o
  assento anuncia o teclado e o `LoopHandle` já tem de existir. Ritmo e atraso são do COMPOSITOR.
- **A barra lateral é uma SEÇÃO** (`files::Section`), com extensão, deslocamento e conteúdo próprios — como a listagem, a prévia e os fatos. `sidebar_columns` a RETIRA quando não cabe (`space.files.sidebarFrom`); a marca vai em `layout.from`, nunca em 0; e `header` usa o MESMO `split` das linhas. Padrões por CHAVE (`nix/files-sidebar.nix` sobre `nix/user-dirs.nix` — **uma fonte para as onze pastas**, e de lá sai também o `RONOCTUA_SCREENSHOTS` que o rodapé lê, que sem isso escrevia onde calhasse). O ajuste do dono vence, em `$XDG_CONFIG_HOME`: padrão da distro OCULTA (volta), o dele REMOVE — e as frases diferem. `C-l` esconde, `C-e` desmonta, `C-u` tira, `s` do menu e `C-s` do localizador fixam: **LETRAS, porque Ctrl DIREITO com seta é paginar e a janela não o rastreia**. **Atravessar é o anel do `Tab`, e só ele** — `C-o` era um segundo caminho e saiu a pedido do dono.
- **Só o `ronoctua-disks` fala udisks2, e ele NÃO escreve regra polkit** — a do upstream já separa removível (`allow_active: yes`) de sistema (`auth_admin_keep`) pelo `HintSystem`, e uma nossa só poderia ALARGAR. **Não há agente polkit na sessão** (só `pkttyagent`, no resgate): toda chamada leva `auth.no_user_interaction`. E a razão é do DONO, não do desenho: ele nem sempre vê o pedido de digital, ou está longe do laptop — pedido não visto vira recusa no relatório, sobre algo que ninguém viu. **Nada que o sistema faça sozinho pode PERGUNTAR**, e volume de SISTEMA não oferece montar — manda ao terminal root. `HintIgnore` vira `quiet` e anda no `C-h` dos dotfiles. O `override` tira só o `xfsprogs` (538,3 → 343,2 MiB; 258 eram Python do `xfs_scrub`, e montar XFS é do KERNEL). Sinal do `ObjectManager` numa thread, mandando o `Rack` INTEIRO — o relógio do rodapé só bate com a vista aberta, e a janela não tem relógio nenhum.
- **`LEADER s f` é formatar, e são TRÊS perguntas** (`buffers/format`, `capability::disks`): qual disco, para que serve, tem certeza. Linguagem de gente — o vfat diz *"abre em qualquer computador"*, não FAT32, e **o XFS aparece DESABILITADO dizendo que falta o programa**, alcançável e recusando. **Toda linha tem DOIS registros**: o rótulo, e uma linha discreta SOB O CURSOR com o nome real e os limites (na lista, o tamanho em bytes; no confirmar, o gesto inteiro). Nome técnico é CHAVE, nunca literal no componente. Duas recusas que o hospedeiro não enxerga (de fora, recusa e componente morto são o mesmo "nenhum plano"), e por isso são do `format-driven`: disco que o sistema precisa é DESENHADO e o `Enter` nele não anda; e o primeiro `Enter` do confirmar VOLTA, porque `não` é a primeira linha. **Sem estado `working`** — o `SPC w s` o ofereceria e mentiria; a espera é na lista. O alvo é id que o buffer RECEBEU, reconferido contra o `Rack` mandado **e contra `needed()` de novo**. Disco inteiro é `dos` até 2 TiB (`gpt` acima) mais UMA partição, com `update-partition-type` — sem ele o vfat sai tipado `0x83` e o Windows não abre o pendrive, sem nada dizendo por quê.
- **`LEADER t` é a fila, e ela é de uma UNIDADE** (`ronoctua-transfers`): três processos olham a mesma
  lista e a cópia sobrevive ao rodapé reiniciar. Socket JSON por linha, um por USUÁRIO. `t t` no
  rodapé, `t w` no buffer wasip2 — e o cursor é o **id**, que a lista reordena. `t a`/`t v`/`t u`/`t l`
  pegam o COPIADO. Desistir APAGA o pedaço. `c` clipboard, `m s` captura.
- **QUATRO motores, e o motor sai do `Work`** (`Work::engine()`), nunca do pedido: disco, aria2
  (JSON-RPC no loopback, HTTP na mão — sem tokio; segredo em `%t` `0600`, NUNCA em `argv`), yt-dlp
  (grupo próprio, pausar É `SIGSTOP`; vem do AMBIENTE — `option_env!` o poria no perfil mínimo) e
  **libtorrent por FFI**, UMA sessão para todas as linhas. **v1 e v2** por 62,7 MiB — menos que o rqbit, que não tem v2. O `unsafe` mora em UM módulo, e o `sizeof` é MEDIDO pelo compilador C++ e comparado por check. Semear é porta aberta (6881-6889, sem UPnP); torrent pronto CONTINUA semeando, e desistir é o que para. Magnet chega por `.desktop`.
- **Baixar COMO VOCÊ é um prefixo, não um interruptor**: `t v` é anônimo, `t i v` é você — a tecla
  diz qual das duas, e a identificada não se desfaz. **OAuth2 morreu**: o yt-dlp o RECUSA (`_base.py:655`); sobrou cookie, e o `--password-store=basic` que já forçávamos É o `basictext` que ele pede. Arquivo GANHA do navegador (o YouTube gira cookie e desloga). aria2 leva **cabeçalho**, nunca `--load-cookies`, que é da sessão inteira.
- **PO token é DISPOSITIVO, não identidade** — vale para TODO download de mídia, inclusive anônimo; só o token COLADO à mão fica atrás do `t i`. `poToken = "on-demand"` (padrão `none`): script que roda por pedido e SAI — a metade servidora do upstream não é instalada, e há check. Deno, não Node, e o Deno **já vinha com o yt-dlp** (`why-depends`, não `mkIf`); o Node NÃO entra (302,8 → 53,6 MiB) porque a referência vinha de shebang de CLI que ninguém roda.
- **A extensão não deixa rastro em página nenhuma**: zero `web_accessible_resources`,
  `content_scripts`, `externally_connectable`, `host_permissions` e `declarativeNetRequest` — as
  cinco vias de detecção, e há check. Injeta só sob `activeTab`; a chave que dá o id nasce NA máquina (inv. 13). **O app não depende dela** — cada coisa do popup tem tecla, e há check dos DOIS lados.
- **O arquivo da notificação também é BROADCAST** (o precedente do `theme.json`): o rodapé vigia o
  histórico e as preferências por inotify e RECONCILIA por id (`Queue::took_from_disk`), em vez de
  reler. Sem isso a janela era um segundo escritor e o rodapé passava por cima dela na notificação
  seguinte: um `d` na janela voltava atrás sozinho. O que chegou DEPOIS do arquivo sobrevive — senão
  a janela apagaria o que acabou de chegar.
- **`LEADER n` é notificação, e TUDO sai de `assets/notifications.nix`** — catálogo, filtros e as
  chaves que o i18n cobra. A faixa fica ACIMA da modeline (1 linha, até 3 com corpo e ações);
  **nada fica para sempre, nem a crítica** (`quick`/`normal`/`long`), com pizza de 8 passos e
  envelope na BORDA DIREITA; contador `2/5` só com mais de uma; sem glifo próprio, o da severidade.
  **O ponteiro em QUALQUER lugar da faixa para o relógio**, e a pizza vira PAUSA na mesma célula
  (`Queue::hold`/`release`): quem está lendo não pode ver a notificação sumir na última palavra. Um
  INSTANTE, não uma flag — soltar devolve o vão exato empurrando o `since`, senão a notificação
  pagaria a espera inteira de uma vez; e o vão conta a partir de `max(hold, since)`, senão marcar
  uma como lida com a mão parada ali daria à seguinte uma vida maior que a dela. Enquanto segura,
  o `tick` responde "nada mudou", então quem pede o quadro é o próprio ponteiro — e a dica é relida
  DEPOIS do quadro, porque ela sai do `band_placed`, que é o que foi DESENHADO. Ponteiro esquecido
  em cima segura para sempre: é o preço do pedido, e o gesto que desfaz é tirar a mão.
  Filtro é `campo + teste`, sem regex, e a PRIMEIRA que casa decide. Suprimida não aparece **e não
  conta**. **O sino só existe com não lida**; clicar abre; `n n` no rodapé, `n w` em janela, `n f`
  nos filtros, uma `View` só, **dica GERADA das tabelas nas duas portas** (`chrome::hint_of`).
  15 dias em `$XDG_STATE_HOME`; seu ajuste em `$XDG_CONFIG_HOME` vence a distro. **`vibrate` e
  `audio` não fazem nada — e a linha de mensagem DIZ** (`INERT`, com check). **Uma AÇÃO é botão**:
  tom próprio (`notification.acts`, a matiz do `label.text` — não há caixa nem borda, inv. 15, então
  a cor é o único que pode dizer) e MAIÚSCULA inicial no `.ftl`. A promessa é METADE em cada lado:
  o teste diz que os dois papéis diferem, e o portão do tema (`notification.quiet vs .acts`, ΔE ≥
  0,04) diz que os papéis resolvem em tintas diferentes em TODO tema — um papel desenhado duas
  vezes passa no portão, e dois papéis numa primitiva passam no teste. Rótulo de app ESTRANGEIRO
  não se toca: `gtk-update` viraria `Gtk-update`. **E uma ação se separa da outra pelo `·` da dica**
  (`mark.separator`, no tom do `Register::Separator`): quatro rótulos num tom só com duas células
  entre eles leem como UMA frase — *Edit the image Open the image Open the folder* —, e rótulo de
  ação É uma frase, então nada nas palavras diz onde uma acaba. Só ENTRE, nunca antes da primeira,
  e o ponto não é alvo. No registro ABERTO não há ponto nenhum: lá cada ação é uma LINHA, e a linha
  já é a fronteira.
- **Um registro se chama `(id, quando chegou)`** (`ronoctua_notify::RecordKey`), nunca só a `id`: o
  contador do barramento nasce em 1 a cada partida e o das nossas nasce em `u32::MAX` e desce,
  enquanto o histórico dura 15 dias — a mesma `id` existe várias vezes em disco, e `replaces_id`
  a repete dentro de uma sessão. Marcar lida e esquecer varriam por `id ==`, e como a lista é do
  mais novo para o mais velho as colisões estavam **sempre abaixo** da selecionada. `Espaço`
  ALTERNA (o envelope aberto vira o de não lida) e a reconciliação entre as portas copia `read`
  nos DOIS sentidos, senão a outra porta desfaz a marca.
- **Ação de notificação NÃO tem letra própria** (o gerador recusa uma): a vista já ensina uma dúzia
  de teclas e vai ensinar mais, e uma letra que a notificação possui colide com alguma delas mais
  cedo ou mais tarde — e colide em silêncio, porque a pessoa aperta e outra coisa acontece. A letra
  também só funcionava no GERENCIADOR: a faixa não toma teclado. O que alcança uma ação é o CURSOR
  no registro aberto. **A linha da lista só MARCA que há ações** (`icon.notification-has-actions`,
  a mão tocando — três pontinhos era o que a elisão ao lado já dizia, e os dois
  colavam; a célula é reservada para a coluna inteira, com o AR antes do glifo) — um rótulo de ação é uma frase e uma linha é uma
  linha —, e abrir lista todas, uma sob a outra. **Ação de comando fecha o gerenciador antes de
  rodar**, senão o `region_is_free` recusa e nada acontece; e **a janela ENCAMINHA** o comando pelo
  socket de controle em vez de mandar você ir ao rodapé fazer à mão.
- **A vista tem QUATRO níveis** (molde do bluetooth: um campo diz onde se está, um `match` em
  `rows()` desenha, `Esc` volta um): a lista, o registro ABERTO (`RET` — corpo quebrado em linhas,
  botões desenhados, e os fatos numa coluna à direita, ou embaixo no estreito, pelo mesmo
  `two_columns` da lista), a ficha de um tipo e a ficha de um FILTRO. No registro aberto o cursor **só
  para nas AÇÕES** — linha de mensagem não é coisa em que se age, e andar por
  elas uma a uma sem nada acender é o que o dono leu na tela —, e o `RET` roda
  a que estiver sob ele; sem ação nenhuma, aí sim as setas rolam o corpo. **Os
  fatos descem para baixo da mensagem** quando a coluna deles não cabe
  (`space.notifications.factsFrom`, mais alto que o `twoColumnsFrom` da lista:
  um fato é rótulo E valor numa linha). O `RET` contextual —
  frase e cor mudando (`Register::Alternate`), como o `S-<seta>` do gerenciador — ficou onde ele
  realmente muda: no catálogo, onde ajustar o da distro e editar o SEU são coisas diferentes. **Estreito EMPILHA as duas colunas** ("Filtered out" abaixo de
  "Received"), e uma função só (`place_of`) diz o que é cada linha desenhada, para o desenho, as
  teclas e o ponteiro. **`auto-read` faz nascer lida** — as quatro do bluetooth nascem — e é sobre
  o SINO, não sobre a faixa. **A lista é ORDEM DE CHEGADA, mais nova em cima**, e nada mais a
  ordena: as não lidas iam agrupadas acima, então uma lida há cinco segundos ficava ABAIXO de uma de
  ontem — a lista não respondia *"o que acabou de acontecer"*, que é a pergunta com que se abre. As
  duas idades continuam distintas pelo TOM, que não custa posição. **Clique numa linha é o `RET`
  dela** (`open_here`, uma função para a tecla e para o mouse); os dois controles da borda direita
  são perguntados ANTES, porque clique neles é aquele controle. **E o ponteiro sobre uma linha diz
  QUANDO chegou e a frase INTEIRA** (`View::about_at`) — as duas coisas que não cabem na linha. Não
  é campo de `Row`: lado a lado uma linha desenhada são DOIS registros, um por coluna, então a
  resposta depende da CÉLULA, e quem resolve as duas é o `place_of`, o mesmo que o clique pergunta.
- **Os filtros da pessoa se escrevem na TELA** (`n f`, `n` para um novo, `RET` para editar o seu,
  `d` para apagar): nome, condições (`campo`/`teste`/`valor`) e o veredito, com as setas girando o
  que se gira e o **minibuffer do rodapé** recebendo o que se digita (a janela DIZ isso). O nome de
  um filtro seu é DADO — texto que você digitou —, e por isso a tela o mostra como está em vez de
  procurar uma chave que não existe. Tom próprio (`text.mine`) e palavra própria, porque "seu"
  antes queria dizer *você ajustou este*.
- **E um filtro pode ir para TODOS** (`p`, e só depois de uma pergunta que abre no `não`): o shell
  escreve `assets/shared/notifications.json` no repositório — na forma do `assets/notifications.nix`,
  não na do serde — e o build o junta à tabela. **Isso é PUBLICAR**: o repositório é público
  (inv. 2) e nada pessoal entra nele (inv. 13), então vai um por vez, nomeado, com a tela dizendo
  a frase e mostrando o texto exato; quem publica de verdade é o seu commit. O caminho do
  repositório vem da opção `ronoctua.repo` (`$RONOCTUA_REPO`), NUNCA de procurar. **E nada pergunta
  à rede**: "já chegou?" se responde contra a tabela COMPILADA — a única estrada do conteúdo da
  distro até uma máquina é um build pelo flake, então o build é a resposta (`Book::took` aposenta
  a cópia local).
- **A modeline diz se há SAÍDA** (`ronoctua_network::Snapshot`, um `watch` desde a partida): `net-off`
  quando um link se acha conectado e o caminho não leva a lugar nenhum (portal, uplink morto — e a
  marca só existe porque `ronoctua.network.connectivityCheck` liga o teste do NM, senão ele responde
  `full` para qualquer rota), `airplane` quando a rede está desligada (clique VOLTA, `s n a`), `vpn`
  quando há túnel de pé (`Vpn` OU `wireguard`, que responde `false` ali). Nada conectado = marca
  nenhuma: não estar na rede não é defeito. As três abrem `s n n`, e a vista repete o que a marca
  disse.
- **Tecla NUNCA é texto no `.ftl`** (`keys-are-never-text`): a sequência vem de uma constante ao
  lado do binding mais o `crate::leader()`, num `{ $keys }`. Escrita, ela envelhece no dia em que a
  tecla anda (`n h` ensinou `n h` por semanas depois de virar `n n`) e a palavra "LEADER" não é
  tecla de ninguém — em produção é `SUPER`, no aninhado é `CTRL ALT SPC`.
- **`LEADER s n` é rede; `LEADER s s`, Syncthing** (os dois moraram em `r`, que morreu — rede é do SISTEMA, ao lado de memória e disco). Só o `ronoctua-network` fala NetworkManager; `overrideFolders` FICA `false` (`true` apaga as pastas a cada rebuild).
- **`LEADER s s` é o Syncthing INTEIRO** (`ronoctua-syncthing`, a única fronteira): uma `View`
  (`syncthing.rs`), duas portas (`s s s` rodapé, `s s w` janela), **quatro níveis** (a lista, uma
  pasta, uma máquina, os ajustes) e `Esc` sobe um. Por anos a distro instalava e afinava o
  Syncthing e não oferecia tela nenhuma: quatro teclas, uma das quais abria uma PÁGINA WEB no
  navegador. Acrescentar pasta, deixar máquina entrar ou mudar qualquer opção era sair do sistema —
  a chrome de fora que o inv. 14 nega. A página fica a uma tecla (`s s a`) porque tem gráficos e
  histórico que lista de rótulo e valor não substitui.
  **É EVENTO, não relógio**: `/rest/events` é LONG POLL — medido contra a 2.1.3, que é a versão que
  o flake pina, uma mudança real volta em **17 ms** e o cliente bloqueado gasta **zero** tique de
  CPU em 25 s. Nenhum temporizador novo. **Mudança RELÊ tudo em vez de fundir o evento**: são vinte
  tipos de evento, alguns com o objeto inteiro e outros com um campo, e a fusão é onde moram os
  defeitos — o `metadata` do PipeWire ensinou isso caro. Dez chamadas de poucos ms na loopback, e só
  quando algo mudou; um piso de 0,5 s (`FLOOR`) impede que uma transferência vire laço.
  **As 121 opções são DECLARADAS** (`assets/syncthing-options.nix` → tabela Rust + as chaves que os
  dois idiomas devem + o check que compara com o esquema REAL nos dois sentidos). Três naturezas:
  **o flake fixa 14** — desenhadas e RECUSANDO, com a frase dizendo de onde vem o valor (inv. 6; e
  DUAS delas são padrão do nixpkgs e não do nosso módulo, o que só o
  `syncthing-flake-owned-rows-refuse` viu); as suas; e as **avançadas, numa GAVETA** com o chevron —
  que é do colapso pela palavra do dono (*"não quero mais o chevron, pois isso é utilizado em
  elementos de descolapsar/colapsar"*), e nenhuma outra linha da vista o desenha.
  **Os CONVITES só existem quando existem**, e eram a única coisa que só a página mostrava.
  **Uma marca na modeline, e ela é a única cuja existência é ESCOLHA**: `Never`, `WhenOff`
  (padrão — acende quando nada está viajando, pausado OU serviço fora do ar, que do outro lado da
  tela são o mesmo fato) e `WhenOn`. Máquina sem nada configurado NÃO acende: marca que nasce acesa
  no primeiro boot não quer dizer nada no segundo dia.
  **Parar a unidade é a ÚNICA coisa privilegiada da vista** — tudo o mais vai pela loopback e não
  pergunta a ninguém —, e é por isso que o módulo ganhou regra polkit: uma ação, dois verbos,
  uma unidade, um usuário, medidos pelo `syncthing-polkit-is-narrow`.
  **O que se digita, digita-se no rodapé**, e a janela DIZ isso; comando que pergunta é comando que
  recebe (`s s d`, `s s f`, e o `RET` que carrega o valor), porque comando sem tecla nenhuma é
  comando que só quem leu o código sabe que existe — o teste do keymap achou três.
- **UM HTTP à mão, e é um crate** (`ronoctua-http`): eram dois — o do `ronoctua-tunnel`, cuja
  primeira linha dizia ser *"a forma que o `shell::syncthing` já tem"* — e o `one-http-writer`
  achou um TERCEIRO no `aria2.rs`, cuja resposta ainda era lida como `String` antes de ser cortada.
  Uma cópia morreu em vez de nascer. O `ronoctua-world` deixou de depender do túnel para ter um
  decodificador de chunk. A senha do Wi-Fi não fica em arquivo: `psk-flags` mais o agente entregando do TPM2 (**sem PCR** — com PCR, atualizar o kernel a quebra).
- **A rede é UMA `View` com DUAS portas** (`s n n` rodapé, `s n w` janela) e DEZ seções como
  linhas, no molde da energia: relance, enlaces, Wi-Fi, redes lembradas, túneis, tailnet, Tor,
  portas, esta rede e monitor. **Dica GERADA da `OF_THE_VIEW`**; senha e caminho se digitam no
  RODAPÉ, e a janela DIZ isso (molde da pergunta de pareamento). **Cinco marcas na modeline** —
  `airplane`, `net-off`, `vpn`, `tailscale`, `tor` —, e todas somem em repouso. **`TAB` anda no
  ANEL das seções** e o clique numa marca abre a vista com o TÍTULO daquela seção no topo: as duas
  saem do mesmo campo (`View::anchor`, honrado UMA vez, senão a região recusaria as setas depois).
  **Lista que se lê continuamente vai numa PRATELEIRA** (`Shelf`: Wi-Fi, lembradas, portas,
  vizinhos, pares): altura FIXA de `space.shelf.lines` para TODAS, completada com vazio, e **CAIXA
  própria** (`chrome::shelf_box`) — fundo discreto (`surface.shelf`, 0,018 acima do chrome), largura
  de UMA linha dela (`markerCells + labelCells + valueCells`, mais ar e a barra), barra na sua
  ÚLTIMA célula e uma célula livre antes da barra da página. As duas barras moravam na mesma coluna
  e o dono as viu colidir; depois o fundo, que era a largura da superfície e era empilhado DEPOIS,
  pintava por cima do polegar da página — *"ela acaba desaparecendo e mudando de tamanho"*. O fundo
  vai antes de toda faixa, e a faixa de hover é CORTADA na caixa. A seta na borda rola a PRATELEIRA;
  **a roda é da PÁGINA** — ela chegou a rolar a prateleira sob o ponteiro, pela regra do gerenciador,
  mas as seções dele são COLUNAS lado a lado e uma prateleira é uma faixa dentro de uma lista, então
  com cinco delas a página parou de rolar. A frase vazia da seção mora DENTRO da prateleira: fora
  dela, zero itens desenhava uma linha e um item desenhava cinco, e tudo abaixo andava quatro. O
  TOTAL viaja no `Row`, senão contá-lo custaria montar a lista duas vezes. **Rede lembrada FORA de alcance existia em lugar nenhum** —
  `saved` era uma palavra ao lado de uma rede visível, então esquecer uma que não se está perto era
  impossível.
- **`ronoctua-tunnel` é a fronteira dos dois túneis que não são do NM**: Tailscale pelo local API
  (HTTP sobre socket, `--operator=<user>` é o que dispensa root; toda preferência vai MASCARADA, ou
  o daemon zera o resto) e Tor pelo socket de controle (cookie em `/run/tor`, **`CookieAuthFile`
  não é opcional** — sem ele o cookie nasce em `/var/lib/tor` a 0700 e ninguém do grupo entra).
  Nenhum dos dois sinaliza: uma thread pergunta a cada 10 s.
  **E a unidade do Tor é um CHROOT**: o nixpkgs dá a ela `RootDirectory=/run/tor/root` com só
  `/etc` e o store montados, então o `%include` de `/var/lib/ronoctua/tor` respondia *"Error reading
  included configuration file or directory"* e o daemon **recusava subir** — toda a configuração que
  a vista escreve era inerte numa máquina de verdade. O `RootDirectoryStartOnly=true` é o que deixava
  isso mudo: o `ExecStartPre` roda FORA do chroot, então o `tor --verify-config` imprimia
  "Configuration was valid" um décimo de segundo antes de o processo real falhar na mesma linha.
  Conserta `serviceConfig.BindReadOnlyPaths = [ cfg.fragmentDirectory ]`, e quem achou foi o
  `network-vm` — **teste de VM é o que enxerga sandbox de unidade; unitário nenhum vê**. **Condição de regra polkit com `&&` e
  `||` LEVA PARÊNTESES** — o `&&` liga mais forte, então `id && user && unit==a || unit==b` lê como
  `(id && user && unit==a) || unit==b`: tudo depois da primeira alternativa liberado para QUALQUER
  ação e QUALQUER sujeito. O `rescue.nix` foi assim por um commit, e nada via — o
  `polkit-rules-are-the-ones-we-know` lia id de ação e resultado, e os dois estavam na lista
  auditada; agora ele recusa a condição. **Tor não sobe no boot**, e a regra polkit dele é
  `start`/`stop` de `tor.service` — uma ação, dois verbos, uma unidade, um usuário. **A seção do Tor
  é desenhada com ele PARADO**, e isso é conserto e não novidade: ela vivia atrás de `if let
  Some(scene)`, e o socket de controle só responde com o Tor de pé — então um Tor parado não tinha
  título, nem linha, nem tecla, e o único interruptor que existia estava escrito `false`. Não havia
  como ligá-lo em lugar nenhum do shell. Quem decide agora é `tor::Standing` (`LoadState` +
  `ActiveState` numa chamada: `is-active` responde `inactive` tanto para parado quanto para
  INEXISTENTE). **Subir com a sessão é preferência NOSSA** (`tor.json`), nunca `systemctl enable` —
  o check proíbe um terceiro verbo, e `enable` sobrevive ao reboot. **E a CONFIGURAÇÃO é um
  fragmento de torrc** que o módulo `%include` e a vista escreve, seguido de `SIGNAL RELOAD` pelo
  socket — IPC, não systemd, então a regra não cresceu uma linha. O `%include` sai no TOPO do torrc
  (o Nix ordena chaves e `%` vem antes das letras), logo **o que o flake declara vence**, e essas
  linhas são desenhadas RECUSANDO. O `SOCKSPort` vem de `nix/tor.nix`, lido pelos dois lados: era
  literal no crate contra um módulo que não o declarava, e os dois casavam por sorte. **OITO módulos
  nossos escrevem uma regra**, e o censo assinado é o `polkit-rules-are-the-ones-we-know`; cada uma
  ganha além dele um check próprio sobre QUAL unidade e QUAIS verbos (`tor-polkit-is-narrow`,
  `syncthing-polkit-is-narrow`, `timedate-polkit-is-narrow`, que leem a regra instalada e a
  constante do crate nos DOIS sentidos).
- **`ronoctua-memory::net` é a fronteira do `/proc/net`** (sockets, ARP) e o `ronoctua-lan` faz a
  varredura ICMP **sem privilégio nenhum** (`SOCK_DGRAM`+`IPPROTO_ICMP`, `ping_group_range`). Duas
  medições viraram desenho: o endereço em `/proc/net/tcp` é LITTLE-ENDIAN (lido ao contrário,
  `127.0.0.1` vira `1.0.0.127`), e o prazo da varredura começa DEPOIS dos envios — o primeiro
  pacote a um vizinho desconhecido espera o ARP, três segundos medidos, e o prazo começado antes
  achava zero máquinas numa rede com cinco. **Porta sem dono DIZ que não é nossa**: `/proc/<pid>/fd`
  de outro usuário não se lê, e uma coluna vazia leria como "não tem ninguém ali".
- **VPN se traz de arquivo, e se LÊ antes** (`s n v`): `AllowedIPs = 0.0.0.0/0` manda tudo por outra
  máquina e uma linha `DNS` muda todo nome que se procura — as duas viram frase na tela antes do
  `RET`. Quem importa é o `nmcli` (a tradução são trinta ajustes que o upstream mantém); a chave
  privada NUNCA sai do arquivo para a prévia.
- **Rótulo e valor têm UMA aritmética** (`chrome::field_pair`), e as CINCO vistas que são lista de
  rótulo+valor a chamam. Eram três cópias com três respostas — valor curto na borda DIREITA da
  superfície, valor longo no terço, fato no terço sempre —, e numa janela larga isso punha `off` a
  cento e cinquenta colunas de `modo avião`. Agora o valor começa numa COLUNA fixa
  (`space.view.labelCells`, limitada a metade da linha), o que faz das cinco uma lista de
  definições. **Linha que responde ao `RET` tem TOM próprio** (`text.label.acts` no rótulo,
  `text.answer` no valor); a informativa usa os tons de fato. **E SOB O CURSOR ela continua
  dizendo isso** (`text.here`): o acento tomava as DUAS metades, então porta e fato viravam a
  mesma linha justamente no instante em que se olha para ela. Agora o rótulo leva o acento — é
  ele que diz "o teclado está aqui", porque janela nenhuma desenha faixa atrás do cursor e linha
  sem valor perderia o sinal — e o valor segue dizendo se a resposta muda, no acento inteiro ou
  no acento drenado. A marca desenhada na frente foi RECUSADA pelo dono e a
  diferença é só cor — um comentário dizia que havia um `icon.row-acts`, e ele nunca existiu.
  **E o `Row::focused` é lido por PORTA NENHUMA**: quem escreve uma vista nova o preenche achando
  que desenhou o cursor, e não desenhou nada — a cor É o cursor. O `history.rs` nasceu assim e o
  dono leu na tela ("não estão mostrando o item corrente selecionado em uma cor diferente"); a
  seção de sincronização do `clipboard_view.rs` tinha o mesmo, e escrever o check achou um TERCEIRO
  no `LEADER t t`. Cobra o `the-cursor-is-a-colour`: **vista sem `TEXT_HERE_*` é vista sem cursor**,
  com UMA exceção — o gerenciador, cujo cursor é faixa (`surface.selection`) mais régua acesa —, e a
  exceção **se prova**: o check lê as duas portas dele e cai se a faixa sumir. Ele chegou a aceitar
  `state.accent` na função que monta a linha, para tolerar as notificações, que acentuavam as DUAS
  metades; o dono mandou consertar em vez de tolerar, e a cláusula saiu. **E `state.accent` e
  `text.here.acts` são o MESMO hex** (`accent.solid`): a divisão é sobre o que a linha DIZ, e só o
  `text.here.read` muda pixel — asserção de foto ali passa dos dois jeitos.
  **Item de LISTA inverte os pesos** do fato: o nome da coisa é a metade forte. **Ar antes de todo
  título** (`chrome::AIR`, um `heading` sem campos), e o título recua para fora das linhas dele.
  **E A NOTA QUEBRA, nas OITO** (`chrome::note_lines`, cobra o `a-note-is-wrapped-and-not-cut`):
  cada uma tinha o seu `sentence` terminando em `elide_end`, que é uma linha com reticência onde a
  sala acabou — e o que a reticência leva é sempre o FIM, que é onde está a consequência. Só a
  `locale` quebrava; as outras seis (`audio`, `power`, `bluetooth`, `network`, `syncthing`, `geo`)
  ficaram escritas como dívida, e a OITAVA — a das telas — não estava na lista de ninguém: ela
  cortava justamente *"The compositor could not read its configuration…"*, que é a frase que aquela
  tela existe para desenhar no pior dia. Todas são pagas por UMA função, para que nenhuma vista
  quebre por uma regra e corte por outra. **E a `locale` quebra em linhas que o cursor PULA e as outras seis não**:
  ali `RET` numa linha informativa COPIA o que ela diz (`chrome::information_of`), então uma linha
  de nota é uma linha que alguém pega, e pular todas deixaria uma seção cujo único conteúdo é uma
  nota com nada sob o título — o que o `a_heading_is_never_alone` existe para recusar.
  **E POR ISSO `key()` RECEBE A LARGURA DA PORTA.** O cursor é um índice nas linhas DESENHADAS e
  uma nota que quebra muda quantas existem, então decidir a `DECIDING_WIDTH` — oitenta colunas fixas
  — passou a ser uma tecla agindo numa linha diferente da que a porta desenhou. É
  `[[um-controle-e-pego-onde-e-desenhado]]` chegando pelo eixo do teclado; o bluetooth já recebia a
  largura, e as outras seis passaram a receber. `DECIDING_WIDTH` sobrou como largura dos TESTES.
- **Toda janela que rola desenha a barra dela** (`chrome::list_bar`, e o `on_the_list_bar` +
  `list_offset_at` que a fazem um CONTROLE): cinco janelas não desenhavam nenhuma — só o
  gerenciador — e a roda era o único caminho. Cobra o `every-list-window-has-a-bar`, que achou a
  quinta sozinho. **A RODA é uma aritmética só** (`surface::axis_lines` + `wheel_arrived` +
  `wheel_lines`, e o rodapé junto): a LINHA é a primitiva e o detente é ela dividida — as cinco
  janelas escreveram `(pixels / linha) as i64 * 3`, que joga a fração fora e multiplica o resto por
  três, então gesto lento não movia nada e depois pulava. Sobra fica guardada; a diferença entre
  rolar e paginar vive no PASSO do modo, não no acumulador.
  **Chegada de daemon NÃO segue o cursor** (`Footer::settle_region`): as três da rede e a do rádio
  chamavam `follow_cursor`, e um relógio de 10 s jogava a página de volta para onde o cursor
  estava. Chegada ajusta; tecla de gente é que segue — e nem o cursor que o `settle` moveu conta,
  porque quem o moveu foi a chegada. **O separador decimal é da LÍNGUA**
  (`Texts::decimal_mark`): estava escrito nos dois formatadores de tamanho, então uma tela inglesa
  dizia `44,2 GB`.
- **O CABEÇALHO DE TODA JANELA ABRE** (`chrome::Hinted::head`, cobra o
  `every-window-header-opens`): a linha 0 de uma janela ensina as teclas da vista, e ela é CORTADA
  na largura que recebe — o `hint_spans` elide, não quebra. A tabela da rede tem 174 células em
  inglês contra uma janela que o compositor põe em metade de uma saída, então mais da metade do que
  aquela linha ensinava era inalcançável, em DEZ portas ao mesmo tempo, com uma reticência onde
  parava e nada para apertar. O rodapé tem a gaveta desde que o `crate::fold` foi escrito e o
  gerenciador tem a dele; as outras dez não tinham nenhuma, e o dono achou na janela de idioma e
  região. **Um chevron na borda direita, `RCtrl ↑↓` no teclado e `Esc` que fecha a gaveta antes de
  fechar a janela** — a MESMA notação que a gaveta ensina dentro de si (`minibuffer-fold-keys`),
  porque o gesto é um gesto onde quer que essa linha seja desenhada. O `Ctrl` DIREITO é rastreado
  pelo keysym e não pelos modificadores, como o rodapé já faz: o xkb funde os dois num bit só.
  **E o desenho, o alvo e a tecla saem de UM lugar** — eram dez cópias do mesmo `if let Some(text)
  = message … else … hint_spans`, e a gaveta não estava em nenhuma. O gerenciador segue com a
  própria (o controle dele está no anel do `Tab`), e o check PROVA essa exceção em vez de a isentar.
- **Dois rótulos de UMA vista não dizem a mesma coisa** (`labels-of-a-view-are-distinct`): havia
  TRÊS `tailnet` e TRÊS `address`, cada um um fato diferente com a mesma palavra, numa lista onde a
  palavra é a única coisa que diz qual fato é. Verbo da dica e formato com `{ $ }` não contam.
- **Arquivo de gente tem UM jeito de ser escrito** (`ronoctua-store`, sobre o `ronoctua-xdg`, que
  fica com ZERO dependência porque três daemons o linkam só pelas raízes): `Store<T>` é raiz +
  escrita atômica + "lê ou cai no padrão e diz por quê". **O rascunho da escrita atômica leva o
  PID** (`<arquivo>.<pid>.new`): era um nome só, o que bastava enquanto cada arquivo tinha um
  escritor, e as telas acabaram com isso — duas portas são dois PROCESSOS gravando o mesmo
  `display.json`. Dois abrindo o mesmo rascunho com truncate misturam as escritas, e o rename
  instala a mistura; em JSON, mistura é arquivo que não parseia, e não parsear vira "nada foi
  escolhido". **E quem escreve o arquivo inteiro de volta pergunta `parsed`, não `read`** — o
  padrão é resposta certa para quem lê e destruição para quem escreve. Cada domínio fica com o SEU arquivo, e não
  há banco: **o arquivo É o broadcast** (um banco precisaria de um segundo canal de aviso de
  mudança), você os edita à mão, e um arquivo que não parseia custa UMA feature em vez de todas.
  A varredura que decidiu isso achou TRÊS escritas atômicas, DUAS que não eram atômicas (as do
  brain) e o "leu, falhou, loga e cai no padrão" escrito SETE vezes.
- **Tokens e textos são GERADOS do Nix** (`assets/tokens/*.nix`, `*.ftl`): chave ou cor inexistente é erro de **compilação** — inv. 8 e 9. **Layout é aritmética de inteiros**, em duas fases: preferência, e "nada vaza da tela".
- **Cor NENHUMA é escrita à mão: `colors.nix` é RECEITA** (`text.primary = "neutral.text-strong"`) e o valor sai de `nix/scripts/gen-theme.py`, por tema, das SEMENTES em `assets/themes/<id>.nix` (matiz, croma relativo, shift, torção) — a lista é `assets/themes/themes.nix`, e nenhum outro sítio nomeia um tema (inv. 16). **ORDEM e PADRÃO são DOIS fatos** naquele arquivo (`order` e `default`), e eram um só: o primeiro da lista era o padrão. Deixaram de ser quando o dono quis o topo da lista e o tema de fábrica diferentes; `builtins.head` não significa padrão em consumidor nenhum, e `counts::DEFAULT_THEME` é o índice GERADO do nome declarado. A escada é a paleta do owl feita regra; toda relação que era prosa é GATE (contraste APCA, distância ΔE_ok, matiz ≥ 14°, faixa protan 158–172°), e o owl gerado é comparado chave a chave com `owl.reference.nix` (≤ 0,03, ou `deviations` com motivo). `nix build .#themes` deixa o `report.md`. **Mexer no gerador reconstrói o workspace** (o caminho do `tokens.rs` muda), então afine ANTES de compilar Rust.
- **Afinar uma cor é mexer na SEMENTE, e o laço custa 2 s**: `nix build .#themes --no-link
  --print-out-paths` e leia o `report.md` do caminho impresso — L, C, H e o hex de cada primitiva,
  os valores de cada token, e a linha 5 com o placar dos portões (`Fail 0`). **Direto do shell,
  nunca por `nix develop`**: medido, 2 s contra 41 s, porque o devshell arrasta junto o `tokens.rs`,
  o WIT e os seis buffers. Os botões são OKLCH, e cada um responde a uma pergunta: `hue` (graus) diz
  OUTRO TIPO de coisa, `text-chroma` diz quanto a matiz aparece (`0.0` é cinza de verdade, e a matiz
  declarada nem é desenhada), `text-shift` sobe ou desce a escada inteira em Lc — luminância é OUTRA
  IMPORTÂNCIA —, e `backdrop.step` diz o quanto o fundo do compositor desce
  abaixo do chrome. **O cinza tem DOIS tints** (`neutral.tint` e `neutral.ground-tint`,
  `NEUTRAL_GROUNDS` no gerador): um FUNDO é onde o tema pode ser o tema, e a faixa ou a régua
  desenhada sobre ele é um SINAL, cuja cor tem de continuar sendo dele. Num tema em que os cinzas
  e o acento dividem a matiz (o forest é 150° nos dois), croma na tinta anda direto para cima da
  `surface.selection` — o que cede primeiro é ela contra `surface.hover` e `rule.default`. Escolher o DEGRAU (`text-muted` 23, `text` 48, `text-loud` 79, `text-strong` 87) é o
  passo grosso; o `shift` é o fino.
  **E TODO FUNDO É MEDIDO, ou diz por escrito que nada é desenhado nele** (`every-ground-is-measured`).
  `surface.container` e `surface.header` existiram uma entrega inteira sem aparecer em portão nenhum
  — quinze tons desenhados neles nos nove temas e nada no `gen-theme.py` olhando para um só. Medido
  no dia em que entraram: `rule.default` sobre o header dá ΔE 0,025 (uma linha que não se vê) e
  `text.subtle.faint` dá **Lc 0,0000** nos dois fundos altos (texto que não existe). Daí três coisas:
  o header ganhou um degrau próprio (`neutral.chrome-band`, porque o `chrome-loud` é também o
  `notification.background`, que está congelado na referência) — sem ele a faixa do ponteiro e o
  header mediam 0,0043 em darkside, a mesma cor duas vezes; nasceu o `rule.raised`, mesmo hex do
  `strong` e nome próprio, porque a divisão é sobre o que a linha DIZ; e **quem escolhe é o `Pen`**,
  a partir do papel com que a região foi PREENCHIDA (`on_ground`) — régua, barra e o degrau mais
  quieto do texto. Escolher errado deixou de ser possível em vez de ser possível e cobrado; cobra
  o `a-rule-follows-its-ground`.
- **Papel novo = família em TODOS os temas** (`assets/themes/*.nix`, menos a `.reference`): sem isso
  o gerador recusa com `no family '<nome>' in the seed`. E papel novo NÃO entra na referência
  congelada — ela mede só as 52 chaves do dia em que foi capturada.
- **Hex existe em três sítios, e nenhum é de autoria**: o `tokens.rs` GERADO (quem consome é a GPU,
  que toma RGBA8), o `report.md` e o fixture congelado. Hex numa maquete do scratchpad ou numa frase
  de relatório é MEDIÇÃO — pixel amostrado para provar o que está na tela —, nunca escolha. Quem
  escolhe diz matiz, croma e contraste, e o solver resolve.
- **`LEADER s t` escolhe o tema, e mover o cursor PRÉ-VISUALIZA em TODA porta** — era só no rodapé,
  e o dono leu na tela: o minibuffer trocava de cor e as janelas, a mesa e os terminais ficavam no
  tema velho, ou seja, um tema vestido por uma tira da tela. A prévia ESCREVE O ARQUIVO
  (`theme::show_everywhere`), que é o canal que as outras portas já vigiam — um segundo canal seria
  uma segunda coisa a manter em dia, e toda leitora teria de aprender uma precedência. O preço é
  que o arquivo que significa "escolhido" é escrito antes de alguém escolher: rodapé que morre no
  meio deixa o par pré-visualizado gravado, que é PREFERÊNCIA e não dado, e é exatamente o que o
  `RET` teria escrito. **`Esc` é o que paga**, e ele precisa saber se HAVIA arquivo antes
  (`View::chosen_before`, perguntado uma vez na abertura): a partir da primeira prévia o arquivo
  existe sempre, e olhar uma lista não pode dar a uma máquina uma escolha que ela nunca fez.
  **E a vigia do tema NÃO junta rajada** (`notice::Pace::AtOnce`, a única assim): a do gerenciador
  segura o rabo da rajada porque a resposta dela é um `readdir` por evento, e a desta é um REPINTE —
  medido, a primeira mudança sai em 5 ms e cada uma atrás dela levava **253**, que é o `QUIET` de um
  quarto de segundo esperando um silêncio que não vem enquanto a mão anda na lista. O `PACE` é
  CONSTANTE NOMEADA para um teste segurar; e a MESA é PEDIDA, não esperada: o desk é um
  `output * bg` num fragmento que o compositor inclui, e **ele não vigia arquivo nenhum** — não há
  inotify nele —, então escrever e parar aí deixaria a mesa na cor velha pelo resto da sessão. Pedir
  é IPC: escreve e então `Compositor::reread`, um ida-e-volta de socket e não um `scrollmsg`, que é
  PROCESSO e bloqueia. O compositor anterior vigiava, e o defeito era mais sutil — a mesa chegava
  *"às vezes mais rápido, às vezes mais devagar"*, que é a cadência de um watcher, e o dono leu isso
  na tela. **E o `reload` responde `success` haja o que houver**: medido contra um vivo, fragmento
  quebrado e config principal quebrado voltaram os dois `{"success": true}`, porque
  `load_config` devolve `config->active || !config->validating || config_load_success` e
  `config->active` é verdade em toda sessão de pé. Então o `Ok` diz que ele ACEITOU o pedido e nada
  mais; quem prova a gramática é o portão de build (`compositor-config-valid`,
  `display-fragment-validates`) e quem prova a mudança é a LEITURA seguinte. `RET` grava `$XDG_CONFIG_HOME/ronoctua/theme.json`, `Esc` volta ao gravado. A lista mostra SÓ o id. **As DUAS colunas são uma lista** — título em `text.heading` e os itens sob ele, como toda vista do sistema —, e entre elas vai uma RÉGUA vertical (`chrome::divider`) na costura em que a grade já as desenha (`colon`), que é a mesma coluna pela qual o ponteiro decide em qual lista caiu. Antes só a direita tinha título, e em `text.dim`; o dono leu na tela que a região não era da forma do resto. A linha na TELA deixou de ser o índice do item: `theme_row`/`desk_row` respondem o que cada linha é, e clique e cursor perguntam a elas. **E os dois títulos ficam sobre os NOMES**, não sobre a marca: a coluna da direita carrega o próprio check DENTRO do texto (a margem do ícone é da esquerda), então o título dela paga a mesma célula — `BLANK_MARK`, e as duas frases saem de UMA expressão para não divergirem de novo. **A segunda coluna (`←/→`) é o FUNDO do compositor — preto ou `surface.backdrop`, um degrau ABAIXO do chrome no matiz E NA DISTÂNCIA da semente `backdrop` (`hue`, `chroma`, `step`)**; no owl ele é CINZA (`#080808`, croma zero, cinco degraus abaixo) a pedido do dono; no forest e no midnight ele é a cor do tema. **São ONZE temas**: owl (cinza, acento azul), forest (superfícies verdes escuras), moss (o mesmo verde puxado para o azul e desbotado), industrial (cinza PURO e o chrome mais ALTO dos escuros — a mesma ausência de cor do darkside lida como METAL em vez de preto), frost (ciano gelo), midnight (fundo navio, rampa QUENTE — rampa fria sobre fundo frio vira uma cor só em duas luminâncias), cyber (cinza frio, rampa SEM cor), darkside e evilside (os dois mais escuros, e as SUPERFÍCIES deles não têm cor nenhuma — cinzas exatos, a pedido do dono; o vermelho do evilside está só nos elementos), e **DOIS CLAROS — flashbang (branco e cinza frio) e papyrus (folha envelhecida, cinzas quentes)**.
  **A POLARIDADE É UM CAMPO DA SEMENTE** (`mode`), e não uma derivação de `ground` vs `chrome`: um
  tema claro pode querer a mesa mais ESCURA que o rodapé, e a derivação leria "escuro". Dela sai um
  sinal que o `_solve_text` e o `_paint` passam ao `solve_lightness` — que já aceitava `direction`
  desde sempre e nunca foi perguntado —, e com ele o portão da escada, a rampa do chart, a ordinal e
  a tabela ANSI (num tema claro `ansi.black` é a TINTA e `ansi.white` é quase o papel; `bright` é
  MENOS contraste, não mais). **O APCA não é simétrico e o croma colapsa perto do branco**, e é daí
  que vêm os dois números que só o claro tem: `lift` (todo anchor de Lc sobe, porque Lc 23 acima do
  chrome do owl custa ΔL 0,33 e abaixo de um chrome em 0,955 custa 0,135 — os fundos levantados
  comiam o texto quieto, e `text.dim` sobre `surface.header` media **Lc 0,0000**) e
  `LIGHT_LC_CEILING`, que **remapeia** a escada em vez de a cortar: cortada em 90, os três degraus
  de topo de toda família caíam na mesma cor (ΔE 0,0000, 34 vezes). **A MESA é o único degrau que
  NÃO espelha** — `chrome − step` nos dois —, porque espelhada ela ficaria entre o chrome e o papel,
  onde só há 0,045 de espaço e ela media 0,0155 do papel. E **dois pisos cedem no claro, nomeados
  aos pares** (`LIGHT_CONTRAST_FLOORS`, `LIGHT_DISTANCE_FLOORS`): eles foram lidos de um tema
  escuro, e a escada clara é estruturalmente mais curta. O `mode` sai também como SÉTIMO FRAGMENTO,
  porque uma paleta não pode ser perguntada de que lado está — todo portão mede MAGNITUDE.
  **E `chart.*` e `level.*` passaram a ser medidos contra um FUNDO**, que nenhum portão fazia: o
  categórico era comparado só com os outros sete e o ordinal só com o vizinho, então um conjunto
  perfeitamente distinto e perfeitamente invisível passava nos dois. **A rampa ordinal é gatilhada
  nas DUAS pontas e em sentidos OPOSTOS** — `level.ten` tem de ler (Lc ≥ 45) e `level.zero` tem de
  SER o fundo (Lc ≤ 10, medido 0,0000 nos dez), que é o que o pé de uma rampa ordinal significa;
  escrever a isenção como medição é o que impede que ela vire silêncio. `chart.*` leva o piso do
  texto mais quieto do sistema (15/20, o do `text.dim`), e uma marca cheia precisa de menos que uma
  letra. Nas três superfícies de conteúdo (`buffer`, `chrome`, `container`): nada abaixo de 15, e
  nove das 240 leituras entre 15 e 20 — sempre a ponta escura, sempre num card.
  **E um SEGUNDO tema claro custou uma semente**, que é o que a máquina acima existe para que custe:
  o `papyrus` saiu com TRÊS falhas na primeira build. Duas eram dele (o vermelho a 0,234 de
  `#FF6000`, porque um tema quente anda com o vermelho para o laranja; e o `text.answer`, que é
  MULTIPLICADOR e por isso não é monótono — 0,40 no croma da rampa passa e 0,50 e 0,60 voltam a
  falhar, que é a parede do `cyber` vista do lado quente). A terceira foi o chrome baixo demais para
  alcançar o teto remapeado: `text-bright` grampeava no PRETO, e as 14 advertências de alcance vinham
  com outras 14 dizendo que o último degrau de toda escada virara um salto de ΔE 0,31 — uma causa,
  duas colunas do relatório.
  **E METADE DO CROMA DE UM AMARELO NÃO É METADE DO AMARELO.** Essa matiz fica onde o olho é mais
  sensível e onde o sRGB é mais largo, então um croma que em azul seria um sussurro ainda é COR ali:
  os fundos do papyrus só pararam de ler como amarelo ao mexer nos DOIS knobs — um quarto do croma
  do rascunho E a matiz vinte graus para fora (80 → 60), que é para onde o papel envelhece.
  **E o piso do croma da rampa é o `text.answer`**, não o gosto: abaixo de 0,32 o multiplicador não
  tem o que multiplicar e o par reprova, o que põe um chão em quão dessaturado um tema pode ser. **`text.answer` é um MULTIPLICADOR de croma, e só funciona onde a rampa tem croma para multiplicar** — no canto amarelo do sRGB. Rampa cinza ou vermelha: o degrau simples e o multiplicado batem no mesmo teto perto do branco, e a separação PIORA se você subir o croma. Cinco temas precisaram do `overrides` da semente por causa disso, que é para o que ele serve; cobra o portão `text.answer vs text.primary`. **A mesa NÃO pode ser preta** — a semente recusa e o portão pede ΔE 0,1 do preto do conteúdo —, e não precisa: `Backdrop::Black` é o PADRÃO, e a mesa do tema é a segunda coluna do `LEADER s t`; as duas iguais seriam uma escolha que não muda nada. **O `step` é da semente porque um fundo sem cor só tem a luminância para se separar do chrome**: um degrau só mede ΔE 0,026 dali, que o portão chama de legal e marginal —, e a prévia dele é o próprio compositor: o fragmento é escrito ao mover o cursor e reescrito no `Esc`; `RET` grava o PAR. **O arquivo É o broadcast**: as cinco portas vigiam `theme.json` por inotify (`Notice::on_file`) e repintam; `RONOCTUA_THEME` (opção `ronoctua.theme`) é o piso, e o chão é o tema `default` declarado. **E o FUNDO nasce no do TEMA**, não no preto: era `Backdrop::Black`, e o dono pediu o contrário — todo tema resolve uma mesa e todo portão a mantém distinta do preto do conteúdo, então mostrá-la de fábrica não custa nada. O rodapé COPIA os fragmentos do tema (`$RONOCTUA_THEMES/<id>/`): `scroll-theme.conf` (borda E fundo — `client.focused` mais `output * bg`, e aqui não há ordem a acertar: bloco de saída é MESCLADO campo a campo, o último vence, então o fragmento ganha do `output * bg` que o config já tem; o config o inclui no FIM, e ausência não é fatal por construção) e `ghostty-theme.conf` (`config-file = ?…`, processo novo — e a entrada `.desktop` é REESCRITA sem o `--gtk-single-instance=true` do upstream: flag vence arquivo). **O Ghostty só lê `~/.config/ghostty/config`** — `/etc/xdg` ele NÃO lê —, e é lá que o home-manager põe o config do módulo. GTK/Qt vestem o MODO DO TEMA (Qt só no perfil `full`: custa 591 MiB); a extensão, o padrão do build.
- **Layout é do `ronoctua-design`** — UM crate compilado dos dois lados (ele é dono das bindings do
  WIT; nenhum buffer chama `generate!`). **DOIS TRILHOS**: texto recua por `chrome::INSET`, régua e
  seleção sangram de borda a borda e não passam pela margem. **UM cabeçalho**, de três células —
  ar, título, ar — e **régua nenhuma**; `Command::Rule { line }` é a fronteira no FIM da linha.
  `chrome` é o recuo das duas portas; só modeline, minibuffer e faixa ficam com `modeline.edgeLeft`.
  **Separador NENHUM se escreve à mão**: `chrome::rule` recebe a LARGURA da superfície (não há
  parâmetro de recuo) e `chrome::divider` recebe o FUNDO (não uma altura) — os oito sites erravam
  ou de um jeito (`x: margin*cell` com `width: self.width`: buraco à esquerda, cortado à direita)
  ou do outro (`linhas × altura_de_linha`, que com 26,5 px sempre para antes da borda). Quem cobra
  é o `separators-come-from-chrome`.
  **E UM CONTROLE É PEGO ONDE ELE É DESENHADO.** Uma função diz onde o controle ficou e as duas
  portas — a que PINTA e a que ACERTA — a LEEM. A barra de um buffer era pintada em pixels a
  partir da borda (`right = width_px`, para tocá-la) e pega em CÉLULAS a partir da margem: na
  última coluna o ponteiro responde duas colunas além da que o componente desenhou, a comparação
  nunca casava, e todo arrasto caía na linha de trás — medido na tela do dono, 81 desenhada contra
  83 relatada. Não é um bug que volta: é a ausência de uma junta. Agora é `paint::bar_box`, e as
  duas leituras são `quads_of` e `bar_grabbed` — o molde do `chrome::hint_spans`/`hint_marks`. O
  mesmo vale para a escada de atalhos do brain (`State::ladder`, uma caminhada lida três vezes:
  a altura, a tinta e o ponteiro). Cobra o `a-bar-is-grabbed-where-it-is-drawn`.
  **E UMA COLUNA DESENHA NO TAMANHO EM QUE ELA MEDE**: um buffer conta CÉLULAS e `Scale::Small`
  anda 0,81 de uma, então o que vem depois começa onde o ORÇAMENTO acaba e não onde a tinta parou
  — vão proporcional à frase, que o dono leu como *"espaços desproporcionais entre os atalhos"* e
  como um ponto separatório com mais ar de um lado. Medido: `Switch step` orçou 118,6 px e desenhou
  96. `Row::put` recusa (contando no `Screen`, que o `check` reprova), e pequeno fica só para a run
  que não empurra nada. Cobra o `a-row-is-never-drawn-small`.
- **O realce de sintaxe é um crate, e não é de app nenhum** (`ronoctua-syntax`): `no_std` + `alloc`,
  zero dependências, 37 linguagens declaradas em `assets/syntax.nix` que resolvem **158 tipos MIME**
  — porque a reivindicação é por TIPO e o gerador a expande pelo DAG de subclasses do freedesktop
  (`application/xml` traz 49 descendentes). Devolve PAPEL, nunca cor; quem traduz é
  `chrome::syntax_color`, num lugar só, e o `Palette::syntax` do design faz o mesmo do lado do
  buffer. Uma linha por vez, com um `State` de **8 bytes** que a seguinte recebe — é o que faz rolar
  custar as linhas VISÍVEIS. Tipo que o freedesktop não tem nasce no `ours` do `mime-glyphs.nix`,
  **com `under`**, e nunca aqui (o `*.ts` é `video/mp2t` lá, e está certo). Nos buffers é feature
  DESLIGADA: a tabela é 55 KB de fonte contra 80-100 KB de `.wasm`.
- **Buffers nativos são componentes wasip2.** Devolvem display list; rasteriza o `ronoctua-render`,
  posiciona o compositor (inv. 4); não conhecem Wayland, cor nem idioma (inv. 11). Estado vive na
  `Instance`. **SETE primitivas**: texto, régua, grade, seleção, `svg` (TRAVA `vector`), e as duas
  que o dono mandou acrescentar — **`paragraph`** e **`bar`**. E **campo de record cruza por
  POSIÇÃO** e só depois pelo nome.
  **E A REGRA DE QUE UM BUFFER NÃO TEM HOVER TAMBÉM ERA MINHA E TAMBÉM ESTAVA ERRADA.** Estava
  escrita no `buffer.rs`: *"There is no hover: highlighting the item under the mouse would ask for a
  frame per pixel of movement, and here the highlight already has an owner — it is the cursor"*. As
  duas metades são falsas. O ponteiro anda em PIXELS e o buffer conta CÉLULAS, então o host só
  entrega quando a CÉLULA muda — algumas dezenas por travessia de tela, e toda outra janela do
  sistema já fazia exatamente isso (`files_window`, `network_window`, `power_window` guardam um
  `under_pointer`). E o cursor NÃO é o mesmo realce: o cursor é onde está o TECLADO, o hover é onde
  um clique cairia, e toda vista do sistema desenha os dois em tons diferentes. Agora a parede leva
  `input::hover(position)` e `input::unhover`, e o `Role::Hover` alcança o `surface.hover` que papel
  nenhum alcançava. Cobra o `a-buffer-can-be-hovered`.
  **E UM BUFFER DESENHA EM BLOCOS, não em `u16` soltos** (`ronoctua_design::Region` + `Screen::split`
  + `Pen`). O rodapé do brain foi desenhado POR CIMA do histórico e havia uma faixa do fundo da
  página entre ele e a borda de baixo — o dono fotografou as duas. Nenhuma era um erro de conta:
  não havia conta. Um buffer endereçava a janela por `u16`, o `go_to` não tinha limite, a baixada
  era um `map` 1:1, e o hospedeiro não recortava nada — linha fora da janela era diagramada e morria
  no rasterizador **sem uma linha de log**, que era o único defeito desta árvore sem sintoma. Os
  blocos LADRILHAM (união exata, interseção vazia, o resto da divisão vai para um `Slice::Rest`), o
  `Pen` **RECUSA** o que cai fora em vez de aparar (aparar esconde o defeito e põe a tinta num
  lugar plausível), e o `Screen::check` reprova o quadro que tiver uma recusa. **Nada disso cruza**:
  é gramática do lado do buffer, provada no `cargo test` dele. Cobram o
  `a-buffer-draws-inside-its-regions` e o `the-host-shows-what-it-draws`.
  **E A LINHA DE ATALHOS DE UM BUFFER É A DO SISTEMA.** Ela não era, e o dono leu as duas lado a
  lado. A COR: a dica do minibuffer é `text.subtle.*`, uma família de matiz deliberadamente fora da
  rampa quente, e o `Palette::color` era uma tabela FECHADA de dez tokens sem nenhum deles — então
  um buffer que ensinava teclas só tinha a rampa de CONTEÚDO (`accent` + `dim`). Entraram
  `Role::Key`, `Role::Prose` e `Role::Faint`. O APERTO: uma frase é CHAVE e este lado não mede uma
  sentença que não tem, então cada uma recebia coluna fixa de treze células — o `Input::Phrases`
  entrega o TEXTO (o mesmo caminho que o campo de busca dos três navegadores usa, e quem traduz
  continua sendo o host), e o header MEDE. O `·` virou `icon.separator`, que é token e não
  codepoint. Cobra o `the-shortcut-line-is-the-systems`.
  **E UMA MUDANÇA DE ESTADO REPINTA.** O `RET` numa tarefa trocava o título da modeline e deixava a
  página velha na tela até a próxima seta — o host tratava `Effect::StateChanged` como escrituração
  e o guest tinha esquecido o `Redraw`. Os dois foram consertados, e o do HOST é o que fecha a
  classe: um estado É uma tela. Cobra o `a-state-change-repaints`.
    **A REGRA DE QUE UM BUFFER NÃO PODE ROLAR ERA MINHA E ESTAVA ERRADA.** O `docs/PENDING.md` dizia
  que nenhuma das cinco era uma região rolável e que inventar uma seria um sexto vocabulário; o dono
  disse que não, e relendo, o raciocínio também não se sustenta: um buffer JÁ desenha a janela de
  lista que quiser — ele é dono do deslocamento, emite as linhas que quer, e o hospedeiro não
  recorta nada. Nada faltava no DESENHO. Faltavam duas coisas, e nenhuma é uma região:
  **o POLEGAR**, porque ele é uma fração de um trilho em PIXELS e este lado conta células (feito de
  blocos seria uma sexta barra parecida com as cinco de verdade e igual a nenhuma) — então a barra
  atravessa como NÚMEROS e o hospedeiro a desenha com o `ronoctua_design::Scroll`, que compila dos
  dois lados desde que foi escrito; e **a RODA** (`input::wheel`, em LINHAS e nunca em pixels, pela
  aritmética única do `surface::axis_lines`), porque uma barra que ninguém pega é o retrato de um
  controle. **A RODA ROLA A JANELA e não o cursor** (escolha do dono): o rodapé de um buffer é
  sobre o que o cursor toca, então uma roda que movesse o cursor trocaria a explicação enquanto
  alguém a lê. Quem SEGUE o cursor é a tecla, nunca a chegada nem a roda — a mesma divisão que o
  `Footer::settle_region` faz no hospedeiro, aqui num campo (`State::follows`).
  **E O ARRASTO É `input::scrolled`**: o hospedeiro sabe onde pintou cada `bar` — ele fez o polegar
  com os três números que ela carrega —, então ele resolve o ponteiro numa LINHA (`Scroll::offset_at`,
  a inversa que já existia) e o buffer só diz de qual lista ela é, pela célula, como faz com a roda.
  ABSOLUTO e nunca um delta: um arrasto é onde a mão ESTÁ, e uma soma de deltas descola dela.
  **E a FAIXA para uma célula antes da barra** (`Pen::keeping`): a faixa é sangrada e a barra é
  desenhada depois, então a barra ficava POR CIMA do item selecionado — o dono relatou exatamente
  isso. O FUNDO do bloco continua indo até a borda: uma barra é um sinal desenhado sobre ele. **E `paragraph` pela mesma razão do outro lado**: um buffer segura uma CHAVE, o `text`
  desenha uma linha e o hospedeiro a elide no orçamento — certo para um rótulo e inútil para uma
  descrição, que cortada na largura de uma coluna é uma descrição que ninguém lê. Quebrar linha
  precisa da string TRADUZIDA, que este lado nunca tem. O retângulo atravessa e o hospedeiro o
  preenche com o mesmo `layout::wrap` de um nome de arquivo comprido.
- **NADA EM CACHE FICA PARA SEMPRE** (`assets/cache.nix` → `nix/cache.nix` → `ronoctua-cache-sweep`
  + `brain/tasks/sweep.rs`). O dono perguntou como o cache funcionava e a resposta honesta era que
  **nenhum dos dois de disco era limpo alguma vez** — sem timer, sem tmpfiles, sem varredura. A
  regra dele: *"Todos os caches devem ter limite, jamais podem ficar no sistema para sempre."*
  **O alcance é TODO o `$XDG_CACHE_HOME`, por decisão dele e contra a minha ressalva** — que fica
  registrada: lá moram 254 GB de target do cargo e 36 GB do yay, e nossas miniaturas são 4,6 MB.
  As miniaturas nunca foram o problema, e uma regra que só governasse o que nós escrevemos teria
  liberado quatro megabytes.
  **E A UNIDADE É A ENTRADA NA RAIZ, inteira** — a pedido dele, e a razão é boa: um cache raramente é
  um monte de arquivos independentes. É um índice e o que o índice nomeia, um manifesto e seus
  blocos, um banco e seu log. Levar os que ninguém leu e deixar o resto não libera um cache, deixa
  um programa segurando METADE de um, que é uma classe de defeito que ninguém depura olhando uma
  listagem. Quase todo programa guarda o cache dele numa pasta própria, e é isso que torna a regra
  grossa a SEGURA: a unidade de deleção vira a unidade que o programa criou. **Uma pasta tem a idade
  do arquivo MAIS NOVO dela**, então cache em uso nunca é levado, por mais antigo que seja o resto.
  **E ela libera muito menos, que é o preço de ser segura**: medido em ensaio, **4,2 GiB (52 pastas
  e 2 arquivos)** contra os 150,3 GiB que a regra arquivo-a-arquivo levaria — a diferença é quase
  toda os dois maiores caches da máquina, que têm cem gigabytes de lixo E algo tocado esta semana.
  **A IDADE É O ÚLTIMO USO, e é o MAIOR entre `atime` e `mtime`** — com UMA exceção, a pasta VAZIA,
  que é datada só pelo `mtime`: o `atime` de uma pasta anda quando QUALQUER UM a lista, e quem a
  lista com mais certeza é esta varredura. Tomando o maior dos dois, toda pasta vazia pareceria
  recém-usada no instante em que fosse olhada — para sempre. Achado por um teste que plantou uma de
  três meses e a viu sobreviver. O `atime` sozinho é armadilha num
  sistema montado `noatime`, onde ele é carimbado na criação e nunca mais anda: lá, o arquivo
  ESCRITO hoje traz um acesso do dia em que foi feito, e a regra o apagaria chamando-o de velho. O
  `max` é o que faz "último uso" querer dizer a mesma coisa nas duas montagens. Aqui é btrfs com
  `relatime` e os acessos andam — medido.
  **As recusas são o desenho**, porque isto apaga arquivo de gente: nunca segue link simbólico
  (`symlink_metadata`, sempre — e um link para `/home` seguido seria a pior coisa que este código
  pode fazer), nunca sai do sistema de arquivos onde começou, só arquivo REGULAR, **carimbo no
  futuro FICA** (a regra do `notify::prune`: o relógio pode ter estado errado), diretório que já
  estava vazio fica, e **ensaia** sem `--execute`. O `ProtectSystem=strict` + `ProtectHome=read-only`
  + `ReadWritePaths=%C` da unidade é a segunda tranca na mesma porta — a que segura se o código
  estiver errado. Cobra o `the-sweep-only-looks-at-links`.
  **É UMA UNIDADE E NÃO ALGO QUE O BRAIN FAZ**: `Task::run` bloqueia a thread do runner, e uma
  caminhada de um quarto de milhão de arquivos ali é um daemon que não desenha e não dá para parar;
  e a unidade ganha `IOSchedulingClass=idle`, que é o argumento que o `memory.nix` já faz sobre a
  coleta do store. **Duas unidades e não um flag** — a segunda conta e não apaga, que é o que o
  `m e` pergunta, e aqui vale mais do que na atualização: *"quanto isto levaria?"* é a pergunta que
  se quer respondida antes de deixar algo apagar pastas inteiras de cache.
  **O PORTÃO DE RAM/CPU/BATERIA É O QUE JÁ EXISTE** (`runner::judge` contra o `Gate` do arquivo do
  dono), e a varredura NÃO tem opinião própria — o pedido de "esse cheque também nas outras rotinas"
  já estava satisfeito, e o que faltava era a varredura herdá-lo. O check recusa uma segunda.
  **E VENCIDA, A JANELA ABRE** (`schedule.rs`): passado um intervalo inteiro além do dia devido, a
  parte do dia vira o dia todo. Num laptop que dorme às 4 da manhã, `night` significaria NUNCA — e a
  atualização automática tinha o mesmo buraco. Uma janela perdida ainda espera a próxima noite; é
  uma preferência sobre um bom momento, não uma condição.
  **UMA TAREFA ESCOLHE A PRÓPRIA CADÊNCIA** (`Task::defaults`, a sexta porta, aberta de propósito):
  `TaskPrefs::default()` é um struct só para todas — semanal, à tarde, um minuto de contagem — e
  isso é certo para quem reconstrói o sistema e errado para uma varredura. Responder dentro do
  `due()` seria MENTIRA NA TELA, porque a linha do brain desenha o que o `Prefs` resolveu.
  **E A GRADE RENOVA O QUE JÁ TEM** (`cache::Kept::stale`, `space` nenhum: `assets/cache.nix`): a
  miniatura do cache é desenhada NA HORA e, se está lá há mais de `freshDays`, refeita atrás da que
  já está na tela. Ninguém espera nada, e **toda mudança de regra passa a chegar à tela sozinha** —
  que é exatamente o defeito da onda desta mesma sessão, onde o código estava certo e o disco não
  sabia. `freshDays < keepDays` é a relação inteira: o que se olha é renovado aos dez e por isso
  nunca chega aos vinte. O build recusa o par que a quebra.
- **`LEADER b` é o cérebro, SEM submenu** (`ronoctua-brain` — o `b` era o grupo BUFFER, e buffer é
  janela (inv. 4), então os dois grupos viraram `w`: `w w` troca, `w n` próximo, `w d` fecha, `w s`
  estado; e o terminal saiu do `w` para o `RET`: `RET RET` o externo, `RET /` o da distro). O dono pediu a tecla direta, e uma sequência não pode ser comando e
  prefixo ao mesmo tempo — o `keymap-no-collision` derruba o BUILD —, então `b a` e `b p` saíram. O
  que elas faziam já existia dentro do buffer (`m a`, `m p`), e **a faixa passou a imprimir as
  teclas do BUFFER**. Os dois continuam comandos do registro, então `LEADER :` os alcança pelo nome
  de qualquer lugar, e a faixa continua clicável; o que mudou é que o caminho mais curto de teclado
  virou dois passos. Estão em `leader::BY_NAME_ONLY`, uma lista de NOMES e não um prefixo — `brain-`
  como prefixo isentaria o próximo comando que alguém esquecer de amarrar. É o daemon que decide
  QUANDO a máquina mexe em si
  mesma — molde do `ronoctua-transfers`: unidade de usuário, `session.slice`, socket JSON por linha,
  estado inteiro e nunca delta. A porta para features novas é o `trait Task`, e ela é uma promessa de
  CINCO lugares: `src/tasks/<id>.rs`, uma linha em `tasks()`, duas chaves nos dois idiomas, uma linha
  na home do buffer, um padrão em `prefs`. **Uma tarefa diz que TIPO é** (`Task::kind`, rotineira ou
  de execução única) e a listagem a desenha com glifo e cor próprios — `md-repeat` contra
  `md-repeat_once`, um desenho de diferença, e `text.once` ao lado. Existe antes da primeira
  tarefa de execução única de propósito: acrescentar a distinção no dia em que uma aparece muda o
  sentido de toda linha da página de uma vez. **E uma pode ter cadência em MINUTOS**
  (`TaskPrefs::every_minutes`, e o `schedule::next_moment` curto-circuita o calendário inteiro):
  dias é a unidade certa para um rebuild e inútil para uma VIGIA — o daemon do Emacs pode cair às
  dez da manhã. Quatro dos métodos são PUROS — é o que faz um ano de
  agenda caber em milissegundos de teste. Ele **não** é uma segunda fronteira com o `/proc` (é o
  `ronoctua-memory`, que ganhou `cpu_pressure` e `on_battery`), não segura teclado e não roda nada
  privilegiado: pede ao systemd. Capacidade `brain` PRÓPRIA — `system` é "quanta RAM, quem come";
  esta termina com a máquina reconstruindo o próprio sistema operacional. `tune` é efeito e não
  demanda, porque demanda é verbo mais ÍNDICE e preferência carrega VALOR.
  **A TELA TEM QUATRO REGIÕES FIXAS E UM CORPO ENTRE ELAS**, e as três de fora nasceram de coisas
  que o dono leu e recusou. **O HEADER** é uma linha com fundo próprio (`Role::Header`): glifo, nome,
  e então TODOS os atalhos da página, GERADOS do `KEYMAP` — o `the-brain-header-is-generated` recusa
  um par que não esteja amarrado, e o defeito que ele fecha é o que o dono relatou duas vezes (`m r`
  e `m o` existiam, faziam o que diziam, e não apareciam em lugar nenhum). Abre em várias linhas com
  um chevron, e a altura é a que os pares PRECISAM, nunca o teto. Substituiu um `heading` de três
  linhas cujo conteúdo era um glifo e um título — e, mais que isso, uma linha de atalhos que **não
  era desenhada**: o `foot()` montava uma tabela com CINCO `Column::fill`, o `Table::resolve` admite
  um, nada colocava, e a página não ensinava tecla nenhuma a ninguém. **O AVISO** é uma linha em
  `Role::Danger`, sem título, e **só existe quando há algo no caminho** — era uma seção com duas
  percentagens e a palavra `waiting`, e um estado sem sujeito não é um estado. **O RODAPÉ** é fixo na
  última linha visível, com fundo próprio e **SEIS LINHAS SEMPRE** (`FOOT_LINES`): nome, a linha que
  o buffer segura (um comando, um caminho), a FRASE em três linhas, a régua e os valores. Altura
  fixa a pedido do dono — *"um tamanho sempre fixo e não variável"* —, porque altura que segue o
  cursor move tudo o que está acima dela a cada seta. **A FRASE é o erro da máquina quando há um**,
  e a descrição quando não: como PARÁGRAFO, inteiro, quebrado pelo hospedeiro. Ele era desenhado por
  último e a altura era a soma das partes limitada a seis — a soma dava sete, e o erro era a parte
  que caía fora; o dono relatou as mensagens tendo sumido. **E os valores são a ÚLTIMA linha**,
  contados de baixo, senão sobrava uma linha em branco embaixo deles. Ele é medido ANTES de o corpo
  ser posto: altura que sai do que o corpo deixou é altura em função de altura, que é o laço que o
  `Fold::held` descreve.
  **E o CABEÇALHO MEDE O NOME em vez de reservar 26 células** (`title_cells`): o nome do HOME vem
  por `Input::Phrases` e o de uma tarefa pelo `TaskState::name_text` do mind — traduzidos pelo
  hospedeiro, para MEDIR; o que se desenha continua sendo a chave. Reservado às cegas, a escada de
  atalhos começava na célula 30 para um título de 5, que é o *"repleto de espaços desproporcionais"*
  que o dono fotografou. A coluna do nome no log segue a mesma regra (`RunRecord::task_text`).
  **E o CURSOR de uma página de tarefa anda numa lista PLANA de `Spot`** — a agenda, os passos, o
  histórico — e não em três regiões com `Tab` entre elas. Uma lista, um `RET`, uma pergunta para o
  rodapé, que é o molde do `place_of` das notificações. O índice de um SPOT não é o índice de uma
  LINHA (a lista de linhas tem brancos e títulos), e confundir os dois pôs a faixa numa coisa e o
  rodapé em outra — cobra o `the_band_and_the_footer_agree`. **E a janela dessa lista conta LINHAS
  DE TELA** (`row_lines`): a descrição vale três linhas e UMA linha de lista, e medida em linhas de
  lista a janela ficava dois curta — o cursor descia ao segundo e ao terceiro item do histórico e a
  página não andava, *"e somente após mover novamente a seleção para baixo, ao quarto item"*. A
  janela, o `follow`, a barra e o CLIQUE somam alturas.
  **E o ponteiro alcança tudo** (inv. 19): clique seleciona, o segundo faz o que o `RET` faz —
  abrir a tarefa, girar o anel, ligar o passo —, e um par ensinado do cabeçalho é um CONTROLE
  (acende sob o ponteiro e roda o comando). Por isso o `m t` morreu: era o único atalho que
  dependia de ONDE o cursor está, então aparecia ensinado com o cursor num item do histórico.
  **E a rubrica `History` diz o alcance** (`3 / 10`, do `journal::RECENT` que atravessa no mind),
  como `Steps` e `Activity` já dizem o deles; **e uma execução que deu certo NÃO devolve a
  descrição da tarefa** ao rodapé — ela já está três linhas acima, e o dono leu o mesmo parágrafo
  duas vezes na mesma tela.
  **E O RODAPÉ É UM SÓ DESENHO, sem régua e sem sobra.** A régua saiu a pedido do
  dono — ela custava à frase uma linha de que a frase precisava (*"por ter espaço vazio abaixo do
  texto, então o texto DEVE ocupar todo o espaço disponível"*) e não sabia se aparecia: desenhada
  na ÚLTIMA linha do bloco ela toma a borda da janela (a cláusula de resto do `paint::Rows::bottom`),
  então numa altura era um traço colado no fundo sem nada embaixo e noutra não estava lá.
  **A frase toma tudo o que resta** — só a linha dos valores é guardada, e só quando há valores —
  **e ROLA** (`Command::Paragraph { first-row }`): quem conta as linhas é o HOSPEDEIRO, porque é ele
  que tem a string traduzida e é ele que quebra; este lado guarda o DESLOCAMENTO e recebe de volta
  só números que o hospedeiro já limitou. Ele desenha o polegar na última célula do retângulo (por
  isso a frase orça uma célula a menos, como toda lista aqui), a roda sobre as palavras volta como
  `input::scrolled` e o arrasto no polegar é o mesmo `bar_box` de sempre. O deslocamento é esquecido
  com o ASSUNTO (`Told::subject`), senão a próxima linha abriria já rolada além do próprio fim.
  **E o chevron existe sempre que ele FAZ alguma coisa** — há teclas escondidas, ou ele está ABERTO
  e o controle é o que fecha: era só a primeira, e no instante em que tudo cabia nas filas abertas
  ele sumia e não havia volta.
  **E o bloco do PÉ toca o pé** (`Command::Anchor`): a superfície guarda `floor(altura/linha)`
  linhas inteiras e um resto de menos de uma, que caía sob a última — *"um espaço após o `Last
  run` … que varia de tamanho conforme a altura disponível"*, nove pixels medidos. O fundo já
  tomava o resto; a TINTA não. Agora o buffer diz de que linha para baixo o quadro é ancorado
  (`Screen::split`, quando o último bloco é fixo) e o hospedeiro conta de BAIXO (`Span::foot`), com
  o fundo de cima esticando até a costura — senão o resto vira uma tira da cor da página entre o
  container do log e o rodapé.
- **O EMACS TEM UMA VIGIA, e ela é a primeira tarefa que só OLHA** (`tasks/emacs.rs`). Três perguntas:
  o daemon responde (`emacsclient -e`, um processo, e `run` é o único método impuro), há um Emacs
  que NÃO é cliente (`ronoctua_memory::emacs`, porque o `/proc` tem uma fronteira só), e o daemon é
  mais velho que o último `doom sync`. Um Emacs autônomo parece o cliente na tela e não é: carregou
  uma cópia própria da configuração, os buffers não são os da sessão, e fechar perde eles —
  `emacsclient` começa com `emacs`, então um teste de prefixo na ordem errada chama TODO cliente de
  autônomo, e há teste. O daemon fora do ar ganha BOTÃO (`systemctl --user start emacs.service`,
  unidade de USUÁRIO, sem polkit e sem perguntar); o Emacs autônomo **não ganha nenhum**, porque o
  que está aberto é o trabalho de alguém. E uma máquina sem Emacs nenhum não é uma máquina com
  problema — `worth_saying` responde isso antes de qualquer frase.
- **E O RELÓGIO TEM OUTRA, que é a segunda que só olha** (`tasks/clock.rs`, id `clock-sync`).
  Ela **não busca a hora**: o `systemd-timesyncd` já é um cliente NTP, disciplina o relógio de
  forma contínua e sabe até onde confiar numa resposta — uma tarefa que perguntasse a hora a um
  servidor web a cada seis horas seria um SEGUNDO acertando este relógio, menos exato que o
  primeiro e discordando dele. Ela confere, RELIGA o automático quando o encontra desligado (o
  único lugar daqui que escreve, e a ensaio ela não escreve) e diz quando o relógio está à deriva.
  Cadência em MINUTOS pela razão da vigia do Emacs, e a mesma asserção de compilação contra uma
  vigia que afogue o log de atividade.
- **A atualização é declarada, e o brain só escolhe a hora** — `assets/updates.nix` gera a tabela
  Rust, as chaves que os dois idiomas devem, os dois roteiros e a recusa. O brain LÊ essa tabela
  (`build.rs`); ele já declarou os passos por conta e os ids saíram **disjuntos** dos da sequência.
  A colisão com o invariante 6 é estrutural: passo cujo programa a flake não põe **não pode nascer
  ligado**, não roda como root e tem de declarar a própria pré-condição; e passo nenhum instala.
  **Nada disso PERGUNTA** — regra polkit devolvendo `YES`, sem `sudo`, sem `pkexec`, sem
  `systemd-run`; e há check. **E o passo `doom-sync` é `binary` de propósito**: `ronoctua-doom-sync`
  vem do flake, mas nomeá-lo como `package` poria o Emacs e os quinze programas do `doom doctor` —
  0,6 GiB — dentro do perfil MÍNIMO, que não tem editor nenhum. `binary` é procurado pelo nome, o
  gerador então o força desligado (inv. 6), e quem o liga é o `nix/modules/emacs.nix`, que é o
  módulo que SABE que o Emacs está instalado. **O interruptor por passo alcança a execução**: o
  `turned_on` do script gerado lê três camadas — o padrão da distro, a tabela da máquina
  (`/etc/ronoctua/updates.conf`), e por cima delas os GIROS da pessoa, que o brain escreve de
  `brain.json` em `$XDG_STATE_HOME/ronoctua/update-steps.conf`. Antes ele escrevia um arquivo que
  ninguém lia: a tecla existia, gravava, e a sequência rodava exatamente o que rodava antes. **A
  camada da pessoa é só da metade DELA** — a metade root roda como root, que não tem como achar o
  arquivo de ninguém, e as formas de dar-lhe um acabam todas num diretório de root que um membro de
  `users` pode escrever. Os seis passos root ficam com a opção do Nix e a página os DESENHA
  recusando, que é a resposta que a vista do Syncthing já dá.
  **A faixa do brain fica COLADA abaixo do minibuffer** (não na última
  linha da superfície, que se moveria com a expansão): ícone, frase, contagem no relógio de 1 s que
  o rodapé já bate, e dois botões que têm tecla. **E ela desenha também um AVISO PERMANENTE**
  (`task::Notice`, levantado por `Report::say`): uma VIGIA não tem outro resultado senão uma frase, e
  uma frase que só aparecesse numa tela que alguém tem de abrir seria uma vigia que ninguém ouve. Uma
  contagem regressiva ganha a linha quando há as duas — entre "esta máquina se reconstrói em quarenta
  segundos" e "o daemon do seu editor caiu", a do prazo é a que espera menos. O aviso carrega CHAVES
  e um id de COMANDO, nunca frases nem ações, e **pode não ter botão**: o do Emacs aberto sem daemon
  não oferece nada, porque o que está aberto é o trabalho de alguém e nada aqui sabe se foi salvo.
- **`C-v` e `C-i` são prévia e fatos no gerenciador** — colunas à direita a partir de
  `space.files.panelsFrom`, faixa embaixo abaixo disso, e **as duas viram juntas**. Só o
  `ronoctua-inspect` decodifica (crate PRÓPRIO: o `ronoctua-files` constrói o binário que roda como
  ROOT). Cinco programas externos, e só o `ffmpeg` faz pixel — medido: o `magick` estourou num DNG
  de 17 MB e o ffmpeg fez o mesmo em 0,21 s. RAW é a prévia EMBUTIDA pelo `exiftool`, porque o
  ffmpeg decodifica os planos lineares e sai quase preto. Todo caminho externo devolve imagem no
  **stdout** e quem decodifica é o `image`: `Rgba` nasce num lugar só. **Recusa é VALOR, não ausência** — painel em branco e painel recusado são
  iguais na tela, e só um tem frase. **O arquivo DIZ tudo o que carrega**, coordenada inclusive — a
  regra que a escondia caiu a pedido do dono, e o `docs/ARCHITECTURE.md` guarda o argumento que foi
  feito por ela e por que ele não se sustentava.
- **A ficha de um aparelho tem DUAS METADES**: os fatos, e o que dá para ajustar NELE. Lado a lado
  acima de `space.bluetooth.settingsFrom`, empilhadas abaixo — e **as setas ←/→ atravessam** nos
  dois casos, `TAB`/`S-TAB` junto (molde do gerenciador; `notifications::two_columns` é o molde do
  cursor por coluna). **`S-<setas>` é que mexem no VALOR** — volume e o anel do perfil —, porque
  com duas metades a direção não tem o que dizer e alcançar a segunda coluna não pode depender de
  saber do `TAB`. **Lado a lado, só a metade com o teclado carrega o ALVO da linha**: a outra é
  desenhada e não age, senão `RET` mexeria no que o olho não está. **O clique é o `RET` da linha
  em que cai**, e leva a CÉLULA (`View::aim`) para escolher a metade antes de montar as linhas —
  nada aqui é de mão única (volume anda, chave volta, porta abre), e o que esquece ou bloqueia é
  LETRA NUA, que clique nenhum alcança. A exceção é a pergunta de pareamento (`View::asking`): ali
  o ponteiro só mira, senão `sim` para um aparelho desconhecido ficaria a um clique. O que a coluna
  oferece sai do `Kind`: volume e mudo por
  `wpctl` — **e volume é NÓ, não cartão** (`set-profile` recebe device, `set-volume` recebe node;
  o headset é device 84 e sink 83) —, o perfil A2DP/HFP, o `WakeAllowed`, e num teclado a porta
  para o `LEADER s k`. **O que o PipeWire guarda não é o que a pessoa lê**: `channelVolumes` é
  amplitude linear e todo mixer mostra a RAIZ CÚBICA dela (0,367886 é 72%).
  **`?` DENTRO DO LAÇO responde pelo DUMP, não pelo objeto**: `card_of` e `sink_of` desistiam no
  objeto 0 de todo `pw-dump` real (o core, com `props` e sem `media.class`), então a coluna nunca
  teve volume nem perfil em máquina nenhuma — e as fixtures, recortadas nas chaves lidas, passavam.
  **`View::fresh_at` não lia o cartão** — abrir pela marca da modeline, que é o caminho mais
  curto, dava ficha sem perfil de áudio; a lista dá uma linha por FRASE, e o encanamento (`1800`,
  `1801`) não é desenhado.
- **`LEADER d` é o RELANCE** (`assets/world.nix` → `ronoctua-world` → `shell/world.rs`): uma linha
  acima da modeline que some sozinha, com hora, data, clima e câmbio dos lugares declarados.
  **Ela fica ENTRE a faixa e a modeline** — acima da modeline pela razão da faixa (a modeline é a
  única linha que diz onde você está), e abaixo da faixa porque notificação chega na hora dela e
  não pode empurrar uma linha que alguém está lendo. Uma linha SÓ, nunca função da largura, e
  quando não cabe o `layout::resolve` da modeline DERRUBA lugares inteiros: meia temperatura é
  pior que nenhuma, e o principal encolhe em vez de sair. **Não conta no teto da expansão**, como
  a faixa. Relógio e data custam ZERO rede (`RONOCTUA_ZONEINFO` no embrulho, o leitor de TZif já
  existia); clima e câmbio são **a primeira TLS do workspace** — rustls sobre `std::net`, sem
  tokio e sem hyper, provedor `ring` (o padrão pede cmake e Go), e **raízes do SISTEMA**
  (`/etc/ssl/certs`), nunca `webpki-roots`, que envelhece dentro do binário. Uma thread busca e
  escreve `world.json` em `$XDG_STATE_HOME` — **o arquivo é o broadcast**, e quem desenha lê o
  arquivo: quem desenha recebe microssegundos ou nada. **O que não chegou não é desenhado** e não
  reserva espaço, senão a linha pularia de lado quando a resposta viesse. Sem lugar pedindo rede,
  **nenhuma thread sobe e nenhum socket abre**. O prazo é `Instant` (monotônico). Medido ao vivo:
  Londres 15,3 °C e 1 GBP = 6,9233 BRL; Londres 23:05 contra Brasília 19:05.
  **E os lugares são do DONO**, não da build: `assets/world.nix` é o chão e
  `$XDG_CONFIG_HOME/ronoctua/world.json` vence INTEIRO — lista mesclada entrada a entrada com a
  de outra pessoa é lista que ninguém prevê. O `chosen` distingue "lista vazia minha" de "nunca
  escolhi", senão desligar o último lugar traria os dois da distro de volta na próxima partida,
  para sempre. O `LEADER s l` tem os quatro interruptores POR LUGAR e **nomeia os dois hosts**
  (`api.open-meteo.com`, `api.frankfurter.dev`), cada um com o seu — desligar uma fonte é decisão
  sobre uma PARTE, não sobre uma abstração, e com os dois desligados nenhuma thread sobe.
  **UMA FONTE É UMA LISTA DE PROVEDORES** (`assets/world.nix`, `sources.<qual>.from`): eles são
  perguntados EM ORDEM e só até um responder, então o segundo não custa nada numa máquina em que o
  primeiro está de pé. Cada um leva um `id`, porque um fallback não é troca de host — a resposta do
  met.no é uma previsão de cem passos e a do Open-Meteo é um número —, e o `id` escolhe o LEITOR em
  Rust; um declarado sem leitor é um teste que cai, nunca uma surpresa em execução. **A cadência é
  por FONTE** (era o `min` das duas, o que perguntava o câmbio doze vezes mais do que o arquivo
  declara) e **uma falha é retentada em MINUTOS** e não na cadência: a falha mais provável é a
  PRIMEIRA passada, porque a thread nasce com o rodapé e o rodapé nasce antes da rede — meia hora
  sem temperatura e nada na tela dizendo por quê é exatamente "o weather não funciona".
  **E O QUE ACONTECEU FICA ESCRITO** (`Cache::sources`, um `Attempt` por fonte: quem respondeu,
  quando, e o que a última tentativa disse). O relance NÃO desenha isso — uma linha lida de relance
  não é lugar de erro —; quem desenha é a seção "Who is asked" do `LEADER s l`, que é onde a
  ausência de um número deixa de ser muda.
  Lugar que não mostra nada não é desenhado e **não é recusado**: recusar a quarta tecla de
  quatro é uma tela que briga. Lugar do dono não tem chave de texto e desenha a ZONA —
  `[world-place-…]` leria como defeito, e pedir tradução de uma cidade que a pessoa acabou de
  acrescentar é pedir à pessoa errada.
  **Lugar se ACRESCENTA e se REMOVE** (`a` e a linha `Remove`), e o que se digita é o FUSO, no
  minibuffer do rodapé — a janela DIZ isso, como a rede já faz com senha e caminho. Um fuso é
  aceito se existir ARQUIVO dele (`a_real_zone`, que também recusa `..` e caminho absoluto): fuso
  que se lê é fuso que se mostra. Lugar novo nasce com o relógio; clima precisa de COORDENADA e
  por isso ligá-lo PERGUNTA onde é, em vez de virar um interruptor que não faz nada — não há
  geocodificador aqui, e um seria um terceiro serviço de rede.
- **A BARRA DO RELANCE TEM A PIZZA DA NOTIFICAÇÃO**, do mesmíssimo `band::countdown_glyph`: dois
  contadores num rodapé não podem vir a parecer diferentes. **O ponteiro em qualquer lugar dela
  para o relógio** e a pizza vira PAUSA (`world::Showing`, que é o `Queue::hold`/`release` de
  novo: soltar empurra o `since`, senão a linha sumiria no instante em que a mão saísse; e o vão
  conta de `max(hold, since)`). **O FUNDO ALTERNA por lugar** — o primeiro fica com o da faixa,
  o segundo dois degraus abaixo (`glance.ground` e `.ground-alternate`, com portão de ΔE): com
  vários lugares lado a lado, um separador deixa o olho com uma fileira de palavras e nada que as
  agrupe, e mudar o chão é o único recurso que o invariante 15 deixa.
  **E o fundo tem o MESMO AR dos dois lados** (`Stripe::round`, `space.glance.air`, cobra o
  `a-glance-stripe-has-equal-air`): ele começava onde a primeira letra começava e corria até onde
  o próximo lugar começava — nada à esquerda, duas células à direita, e o último tomava o resto da
  barra. Um número gasto duas vezes é o que faz "o mesmo ar dos dois lados" não poder divergir.
  **E A PIZZA É ANCORADA À BORDA** (`chrome::from_the_edge`): `columns` é `floor(largura/célula)`,
  então uma run contada da ESQUERDA fica a uma célula MAIS o resto da grade da borda — de 0 a 1
  célula, mudando com a largura —, enquanto toda marca da modeline fica a uma célula exata dela. É
  o `Span::edge`, e ele vale para os três aglomerados de direita das linhas que cercam a modeline:
  a faixa, o relance e a do brain. **A METADE DO PONTEIRO VAI JUNTO** (`chrome::at_the_edge`,
  `Piece::edge`, e o `at` de cada linha compara pela leitura que a COLOCOU): mover a tinta sem
  mover o alvo é `[[um-controle-e-pego-onde-e-desenhado]]` chegando pelo outro eixo, e o envelope
  da faixa é um controle. Cobra o `a-mark-at-the-edge-is-drawn-at-the-edge`.
- **`LEADER s l` é IDIOMA E REGIÃO** (`locale.rs`, uma `View` e DUAS portas — `s l l` no rodapé,
  `s l w` em janela): cinco seções, e a
  divisão delas É o desenho. **O idioma é NOSSO** e troca AO VIVO — `language.json`, na forma de
  três fontes do `ronoctua-power/prefs` (arquivo > ambiente > chão), difundido como o tema:
  `Notice::on_file` com `Pace::AtOnce` (a segunda assim), e as dez portas trocam o `Rc<Texts>` e
  repintam. `LANG` continua sendo a resposta da MÁQUINA e leva um rebuild (inv. 6), e é por isso
  que o arquivo existe: escolher entre duas traduções não é decisão do tamanho de um rebuild.
  **O relógio (12/24 h) e a ORDEM da data também são nossos** (`locale.json`), e alcançam TODA
  data do sistema porque `chrome::when_text` os lê de um valor do PROCESSO
  (`locale::prefs::current`) — a mesma forma do tema, e não um parâmetro em cinco assinaturas.
  A chave `files-when` virou `{ $date } { $time }`. **As medidas e o separador decimal
  a vista só DIZ**, cada um apontando quem decide: uma tela que oferecesse os quatro e honrasse
  dois seria pior que uma que oferece dois. Trocar de idioma também move a locale do RENDERER
  (`Text::set_locale`) — é ela que decide, entre as quatro faces Han unificadas, se o ideograma
  sai japonês ou chinês.
  **E O RELÓGIO DA MÁQUINA É UMA SEÇÃO, que não existia** (`ronoctua-clock`, a única fronteira com
  o `timedate1`): a tela mudava o relógio, a data e o fuso da INTERFACE e nada do computador, e o
  dono chamou isso de configurações incompletas. Agora há `Kept right automatically` (o `SetNTP`
  do systemd, e o cliente é o `systemd-timesyncd`), a data e a hora da máquina, e o fuso DELA. **O
  automático é o padrão, e o que ele mantém não se digita**: o daemon recusa enquanto o NTP está
  ligado — a próxima checagem poria de volta —, então as duas linhas são DESENHADAS recusando e
  dizem por quê, em vez de sumirem. **O fuso passou a ser da pessoa**: `ronoctua.time.zone` nasce
  `null`, o que deixa `time.timeZone = null` e o `/etc/localtime` para o timedated; preenchida, o
  flake FIXA, o NixOS põe `NIXOS_STATIC_TIMEZONE=1`, a linha é desenhada RECUSANDO e o módulo diz
  isso à tela por `RONOCTUA_TIME_ZONE`. O instalador deixou de escrever `time.timeZone` e SEMEIA o
  `/etc/localtime` no alvo. Custa uma regra polkit de TRÊS ações e um usuário
  (`timedate-polkit-is-narrow`; `set-local-rtc` fica de fora porque nada aqui a chama).
  **E A VIGIA É UMA TAREFA DO BRAIN** (`clock-sync`, a segunda que só olha): ela não busca a hora —
  quem busca é o timesyncd, e dois clientes seriam dois relógios discordando —, ela CONFERE, religa
  o automático quando o encontra desligado e diz quando o relógio está à deriva. **Um interruptor,
  duas portas**: o `enabled` da tarefa é a verdade, o daemon é quem escreve, e esta tela o lê por
  `Ask::Look` e o muda por `Ask::Tune` — a página da tarefa mexe no mesmo campo.
  **E AS FRASES QUEBRAM** (`Builder::sentence` → `layout::wrap`, uma `Row` por linha, cobra o
  `a-note-is-wrapped-and-not-cut`): elas dizem o que um ajuste faz e ONDE ele aparece, e o que a
  reticência levava era sempre o FIM, que é onde está a consequência. Por isso o cursor conta as
  linhas em que se PODE PARAR e não as desenhadas — uma nota quebrada é mais linhas numa tela
  estreita, e um cursor sobre as desenhadas nomearia um ajuste diferente a cada largura.
- **UMA LINHA POR AJUSTE, e o valor é um ANEL** — `S-<setas>` o giram, `RET` o gira para a frente
  (porque `RET` é o que um CLIQUE vira, e uma linha só alcançável por seta com shift seria linha
  que o mouse não usa). Era uma linha por VALOR — `24-hour` acima de `12-hour`, três para a data,
  uma por idioma —, e o dono leu na tela e disse o que é: **aquilo não são vários ajustes, é um
  ajuste e suas alternativas**. **E o cursor NASCE numa linha que age**: ele nascia em zero, que é
  o primeiro TÍTULO — toda seção abre com um —, então nada ficava aceso e o `RET` caía em
  `Target::Nothing`. De fora isso é uma tela onde tecla nenhuma faz nada, que foi exatamente o
  relato. **O fuso, o separador decimal e as medidas deixaram de ser FATOS**: o fuso desta
  interface é o que ELA lê um carimbo em (e a linha se chama `Show times in`, porque o fuso que
  alguém quer dizer com "fuso" é o da MÁQUINA, uma seção abaixo), o separador pode vencer o idioma,
  e a unidade alcança o °C/°F do relance.
  **E A HIERARQUIA SÃO TRÊS NÍVEIS, por COR e AR** (invariante 15: nunca tamanho, nunca régua): a
  seção em `text.heading` na margem, o sub-título de um lugar em `text.secondary` recuado de
  `space.view.nestCells` com o fuso entre parênteses em `text.dim`, e as linhas dele recuadas
  junto. Os dois primeiros eram a MESMA tinta na MESMA coluna, e o dono leu que o título principal
  era idêntico a todos os de baixo e que o bloco não se lia como uma seção. **Toda seção abre com
  uma frase** dizendo o que muda e onde aparece. **E AÇÃO NÃO TEM VALOR**: `Remove this place`
  levava o fuso do lugar e `Add a place` levava uma instrução, as duas lidas como a RESPOSTA da
  linha — o que aquela coluna é em todo o resto da tela. **E o clique SELECIONA, o segundo é que
  age** (inv. 19): o rodapé agia no PRIMEIRO e a janela não agia nunca.
- **`LEADER s k` são os TECLADOS** (`ronoctua-keyboards` + `buffers/keyboard`), e é o único ajuste
  POR DISPOSITIVO que existe: o compositor tem **um** `input type:keyboard { … }` para todos e não expõe
  dispositivo nenhum por IPC, então layout, acentos e repetição são globais **por construção** — a
  tela DIZ isso. Quem distingue teclado é o `keyd`, e a regra dele é: **um id em UM arquivo só**.
  Como o arquivo da distro reivindica `*`, um teclado com regra própria SAI do coringa — e sai da
  leader junto, se o arquivo dele não a repetir. Por isso `/etc/keyd` é link para
  `/var/lib/ronoctua/keyd`, que é do usuário (tmpfiles), a regra da leader é declarada UMA vez
  (`assets/keyboards.nix`, lida pelo módulo e pelo shell) e **todo** arquivo renderizado a carrega;
  cobra o `keyd-rules-carry-the-leader`. Antes de instalar, `keyd check` — recusar é melhor que um
  keyd morto —, e o reinício passa pela regra polkit que já existia. O buffer não traduz e não
  escreve: recebe teclados, teclas e significados (`input::keyboards`) e devolve três strings
  (`effect::remap`, `capability::keyboard`), que o host confere contra o que mandou.
  **`kbd` não faz um teclado**: o botão de energia, a tampa, o barramento de vídeo, o
  alto-falante do PC e o controle do headset carregam o handler — em 2026-09-08, nove
  dispositivos desta máquina o carregavam e DOIS eram teclados. O filtro é o alfabeto, como
  o próprio keyd faz.
- **`LEADER s b` é o Bluetooth** (`ronoctua-bluetooth`, a fronteira com o BlueZ): uma `View`
  (`bluetooth.rs`), duas portas (`s b b`/`s b w`), três níveis (lista → dispositivo → rádio) e
  LETRAS NUAS lidas da `OF_THE_VIEW` pelas duas. Tudo por `GetManagedObjects` (propriedade opcional
  ausente é ausência, e `Get` nela é ERRO); regra de sinal por INTERFACE, porque `InterfacesAdded`
  sai de `/`; QUATRO threads (sinais e `NameOwnerChanged` → bandeira → leitura ≤4×/s → ordens, que
  também servem o agente de pareamento). **Nenhuma regra polkit e nem grupo** — a política do próprio bluez já abre a porta
  (`bluetooth-bus-policy-is-open`) —, e o agente NÃO reivindica nome. Modeline: até quatro marcas
  (`bt-1..4`), uma por CONECTADO, glifo do que a coisa é (classe/aparência, nunca o nome), `%` só a
  ≤30 e perigo a ≤15; clique abre aquele dispositivo. `Esc` na pergunta de pareamento responde NÃO — e quando a vista não pode abrir (o
  gerenciador está no rodapé), a pergunta vira NOTIFICAÇÃO com os dois botões — e o corpo NOMEIA as
  duas teclas, porque o cartão vive 20 s e o agente segura a pergunta 55.
  **O AGENTE MORA NA THREAD QUE OUVE O `NameOwnerChanged`** e se reinscreve a cada troca de dono do
  nome: registrado uma vez só, um `nixos-rebuild` que toque o `main.conf` reiniciava o `bluetoothd` e
  o rodapé passava o resto da sessão sem conseguir PERGUNTAR — dizendo "o outro lado disse não". O
  loop aninhado não registra (`RONOCTUA_NESTED`): o `RequestDefaultAgent` dá as perguntas ao ÚLTIMO,
  e ele as roubava da sessão real sem devolver. **A dica ensina o NÍVEL** (`Where`), porque a tela do
  rádio não tem dispositivo e ensinava seis teclas mortas — o dono fotografou.
  **O PERFIL DE ÁUDIO É DO PIPEWIRE** (`audio.rs`): o `MediaTransport1` diz o que ESTÁ tocando e não
  o que a coisa PODE fazer, então A2DP contra HFP sai do `pw-dump` e entra pelo `wpctl` — os dois já
  em `sw/bin` pelo `services.pipewire`, medido com `nix path-info -r` (o `pactl` arrastaria o cliente
  do PulseAudio). A placa casa pelo ENDEREÇO (`bluez_card.AA_BB_…`); o caminho do BlueZ não existe lá.
- **Uma pergunta que aparece sozinha responde NÃO no `RET`.** O pareamento do Bluetooth desenha o
  `não` PRIMEIRO e o cursor nasce nele (molde do formatar): a pergunta abre por cima do que estava na
  tela, e a tecla seguinte não pode parear um dispositivo desconhecido. **Perigo é COR, não desenho**
  — a bateria a ≤15% continua mostrando o glifo de carregando quando está na tomada; trocar o desenho
  escondia justamente a pergunta que importa aos 9%.
- **Uma vista só toma o rodapé se ele estiver livre** (`region_is_free`), e são ONZE portas: com
  qualquer vista aberta as outras recusam **e DIZEM**. Eram seis portas perguntando sobre UMA vista
  (o gerenciador de arquivos), o que fazia a promessa valer para um décimo dos casos — clicar na
  bateria com o rádio aberto construía uma segunda vista viva DEBAIXO da desenhada, e os dois `Esc`
  para sair eram o único sinal. A **dica** sai do mesmo `Footer::view()`, um lugar só.
- **Todo teclado repete** (`get_keyboard_with_repeat`), e nenhum repetia: SCTK só repete para um
  teclado criado assim, então nove janelas tinham `repeat_key` como código morto — segurar seta andava
  uma linha. Cobra o `every-window-repeats`; o rodapé é a exceção escrita (repetir leader é erro).
- **`LEADER s a` é o SOM** (`ronoctua-audio`, a única fronteira com o PipeWire): uma `View`
  (`audio.rs`), duas portas (`s a a` no rodapé, `s a w` em janela), cinco seções — saída, entrada,
  programas, a placa e os ajustes. **É EVENTO, não relógio**: `pw-dump --monitor` imprime o grafo
  uma vez e depois só o que mudou; medido, o delta depois de um `wpctl` chega em **6 ms** e o filho
  parado não gasta CPU. Nenhum temporizador novo, e o `wpctl` que escreve NÃO relê — a thread do
  monitor conta sozinha, senão segurar uma seta seria um `fork` por quadro.
  **O número que o PipeWire guarda não é o que a pessoa lê**: `channelVolumes` é amplitude linear e
  o `wpctl` fala a RAIZ CÚBICA dela, nos dois sentidos (medido: 0,056895 lê 0,38, e `set-volume
  0,384614` grava 0,056895). Converte-se UMA vez, na fronteira; o `pactl` mostra uma terceira
  convenção e não é a nossa. **O `ronoctua-bluetooth` deixou de ter `pw-dump` próprio** — eram dois
  leitores de um daemon, e o `one-pipewire-boundary` cobra.
  **O medidor do microfone é a única coisa que abre um microfone** (`pw-cat`, 8 kHz mono, 16 KB/s):
  ele existe só sob a linha em foco, a tela DIZ que está ouvindo, e é um VALOR com guarda de `Drop`
  — não um interruptor. **E o medidor acende a marca do microfone e aparece na seção de programas**,
  como qualquer outro: esconder o nosso seria a única mentira capaz de tornar a marca inútil.
  A guarda de `Drop` cobre todo caminho ORDINÁRIO e nenhum sinal, então o filho roda sob
  `setpriv --pdeathsig KILL` — o truque do `setsid --ctty` do terminal, um binário auditado no lugar
  de um `pre_exec` inseguro. Medido antes: rodapé morto com `SIGKILL`, `pw-cat` vivo em
  `epoll_wait` com o aparelho aberto; um cano de saída sem leitor NÃO o mata. Isso faz a linha do nível existir só sob a fonte em foco, e por isso **toda
  linha em que o cursor para tem uma identidade** (`Spot`): mover o cursor por ÍNDICE através da
  reconstrução o deixa uma linha fora, e **a decisão do medidor vem ANTES da reconstrução**, senão
  as linhas ainda carregam o nível da fonte que se acabou de deixar.
  **O `metadata` do PipeWire chega PARCIAL, como os `params`** — um `default` mudado traz UM item —,
  e substituir o array inteiro apagava os outros três: lido na tela, entre duas fotos, como o
  marcador "em uso" sumindo do microfone padrão.
  **Duas marcas na modeline, e em repouso ela segue muda**: `muted` só quando o sink PADRÃO está
  mudo (um HDMI mudo que ninguém usa não é a máquina em silêncio) e `microphone` só quando existe
  fluxo de ENTRADA, que é privacidade e sai do grafo, não de um palpite. Não há escada de volume:
  o gerador mede a tinta e `md-volume_low` sai a 15 px contra 22 do alto, o que faria a linha tremer
  a cada passo — a BARRA (`chrome::meter`, dois `mark`s de uma célula) é que diz quanto.
  **O aparelho escolhido é do PipeWire** (`default.configured.*`) e não guardamos cópia; o
  `audio.json` fica com o passo, o teto acima de 100% e se a linha aparece.
- **`LEADER p` é o PLAYER, e ele é UM processo** (`ronoctua-player`, unidade de usuário): um
  decodificador alimenta todas as vistas pelo anel de memória comum (`ronoctua-frames`, que subiu do
  estúdio), e um áudio toca — o mesmo vídeo no rodapé, na janela e na coluna está junto por
  construção. **O motor é libmpv por FFI escrita à mão** (`ronoctua-player-engine/src/mpv.rs`, o
  único `unsafe`, `mpv-layout-matches-c`), e é um CRATE À PARTE porque o workspace unifica features:
  com uma feature, o shell linkaria o libmpv no dia em que o daemon fosse compilado ao lado. O
  `mpv-unwrapped` é sobrescrito com o `ffmpeg-full`. **Três medições viraram desenho**: a render API
  de software espera a hora do quadro se `BLOCK_FOR_TARGET_TIME` não for 0 (31 ms "por quadro"); o
  `rgb0` deixa o quarto byte em ZERO, que o renderizador lê como transparente — todo quadro chegava e
  nenhum pixel aparecia —, então o motor o põe em 255; e o contexto de render tem de existir ANTES do
  `loadfile`, senão o `vo=libmpv` larga o vídeo para sempre (o vídeo "em sincronia" tocava e não
  mostrava nada). **O que só o DeaDBeeF abria vem de decodificadores de linha de comando**
  (`decoders.rs` + `nix/player-tools.nix`, `player-decoders-exist`): SID, MIDI com GeneralUser GS,
  vgmstream, sc68, AdLib (`nix/adplug.nix` — o nixpkgs não tem), zxtune (PSF, PSF2, USF, GSF, 2SF,
  NCSF, SSF e o ZX Spectrum) e o SACD — que só reivindica um `.iso` cujo setor 510 diz
  `SACDMTOC`, porque `.iso` também é DVD (`Decoder::mark`). E o AMIGA é o `uade`, que é UNFREE e entra SOZINHO: o
  `nix/player-tools.nix` importa um nixpkgs cujo predicado admite `uade` e nada mais, e o nome do
  arquivo do Amiga põe o formato NA FRENTE (`mdat.Turrican`, `Decoder::prefixed`). Todo WAV escrito em cano diz
  tamanho ZERO e o leitor chama o primeiro pacote de corrompido, então o cabeçalho é consertado na
  fronteira (`decoders::endless`); o `fluidsynth` não abre FIFO para WAV e sai em RAW no stdout, e o
  zxtune recusa FIFO também — escreve arquivo no CACHE, que o motor lê enquanto cresce
  (`Growing`). **O fluxo de um decodificador BUSCA** (`Decoded::seek_to`): para frente decodifica e
  descarta, para trás reinicia — sem isso a linha de posição não andava num SID e a máquina que
  segue tocava seis segundos atrás para sempre. **E a busca de SONDA não é lugar**: o demuxer pula
  para depois do tamanho que o cabeçalho sem fim declara (4 294 967 304, medido); decodificar até lá
  custava a música inteira e RECUSAR fazia o mpv dar o fluxo por acabado, então o fluxo vai lá sem
  decodificar nada e a volta reinicia o decodificador. A máquina que SEGUE baixa inteiro o arquivo
  que só um decodificador nosso lê, antes de tomar a trava do motor. E seguir NÃO custa a fila: a
  daemon guarda a fila e o lugar desta máquina, não os grava enquanto segue, e os devolve PAUSADOS
  ao sair. **A coluna segue a tela com FOCO**: ela ouve o compositor pelo contrato e, quando o foco
  vai para outra tela, refaz a superfície lá (GPU inclusive — superfície nova é textura nova).
  **A COLUNA** (`player_window.rs`, `Door::Column`) é layer-shell TOP|RIGHT|BOTTOM com zona exclusiva
  da própria largura (`space.column.width`): a zona é recortada do que as superfícies criadas ANTES
  deixaram, então ela termina onde o rodapé começa — e um rodapé que sobe depois dela seria
  estreitado, por isso ele a levanta de novo na partida. **Uma só, e é um SLOT**
  (`right_column.rs`, `Tenant`): quem toma a borda encerra quem a tinha, e a próxima coluna entra
  como uma variante, nunca como um segundo arquivo — senão seriam duas lado a lado, que o inv. 14 nega. A janela (`p w`) é o
  mesmo anfitrião com `xdg_toplevel`. **Vídeos juntos** dividem UMA faixa sob o principal, lado a
  lado, dentro do mesmo teto. **Letra**: `.lrc` ao lado, a tag, e o LRCLIB; **SponsorBlock** pede
  pelo PREFIXO do SHA-256 do id (o serviço não sabe o vídeo). Os dois nascem DESLIGADOS e a linha
  nomeia o host. **Outra máquina** (`p m` pareia, a vista junta): só a tailnet — o daemon escuta nos
  endereços dela e recusa quem está fora das faixas dela —, código de seis dígitos por dois minutos,
  chave em `player-peers.json` (0600), relógio medido uma vez (oito idas, a mediana), mídia por HTTP
  com `Range` na mesma porta (`ronoctua.player.port`, aberta só na interface da tailnet). Carregar
  pela tailnet atrasa ~0,7 s, e o empurrão de 5% pagava isso em catorze segundos: por 4 s depois de
  um load quem segue SALTA, e fica a ±70 ms desde o primeiro segundo (medido). **MPRIS**
  (`org.mpris.MediaPlayer2.<instância>`) é o que faz as teclas de mídia e o `playerctl` chegarem nele,
  e é a especificação INTEIRA menos `Playlists` (que é de biblioteca de listas SALVAS, e aqui há
  fila e disco): a FILA é a `TrackList` (ler, acrescentar, tirar, ir), o id de uma faixa é o LUGAR
  dela e toda mudança da fila sai como `TrackListReplaced`, os `Can*` dizem o que a fila deixa,
  `Raise` levanta a coluna, `Quit` para e sai — e o `client::ensure` pede a unidade de volta, porque
  na máquina instalada o daemon não mora ao lado do shell —, e `SupportedMimeTypes` NOMEIA os tipos
  da entrada (curinga não é tipo).
  **A marca da modeline ALTERNA a coluna, como o `p c`**, e "há coluna de pé?" confere a LINHA DE
  COMANDO do PID (`player_window::is_a_column`, que aceita o `.ronoctua-shell-wrapped` do
  embrulho), não só se ele vive: PID é reciclado, e um `column.pid` velho fazia o clique mandar
  `TERM` a um processo alheio e não abrir nada. **E as marcas da DIREITA são acertadas pela BORDA**
  (`segment_at` lê o `edge` que o desenho usou, com o `column_at_the_edge` da faixa): desenhadas
  da borda e acertadas da esquerda, só a metade esquerda do glifo do pause respondia.
  **A imagem pede a PROPORÇÃO do vídeo** (`player_window::fitted`, do `State::video`) e não a da
  faixa: pedida na forma da faixa, o player a devolvia com tarjas PRETAS dentro do quadro e o tom de
  superfície atrás nunca aparecia. **Uma vista nova recebe o quadro que está na tela**
  (`Viewer::owed`, e o `Poke` acorda a thread de render): vídeo pausado e capa de mp3 não mandam
  quadro novo, e a coluna aberta sobre um ficava preta até algo mudar. **A imagem SOZINHA** (`o`, ou
  `ronoctua-shell player only`) e **na tela inteira** (`m`, ou `player maximized`; da coluna, abre a
  janela assim): `Stage`, com a imagem tomando a superfície, tarja preta, e o cabeçalho e o
  transporte POR CIMA em `surface.transport` a cada atividade, sumindo 3 s depois da última — nunca
  com a mão sobre eles, e controle escondido não é alvo. `Esc` sai do palco antes de fechar.
  **Clique na imagem pausa, DOIS cliques põem na tela inteira** (e de volta): o primeiro já pausou,
  o segundo toca de novo, então o que o duplo deixa é a tela mudada e o filme como estava. **A JANELA
  PEQUENA NO CANTO** (`i`, ou `player pip`, `Stage::Small`) é pedida ao compositor pelo CONTRATO
  (`Compositor::float_in_corner`, `Corner`): flutuante, fixa em todo workspace, `space.player.pip*`
  (pixels, 384×216 e 16 de ar) no canto de baixo à direita da SALA que as janelas têm — a área do
  workspace, que já desconta o rodapé e a coluna; o canto da TELA poria a imagem sobre o rodapé. Só
  depois do primeiro quadro: janela não mapeada não está na árvore para ser achada. **Tela inteira e
  canto, pedidos da coluna ou do rodapé, são um INTERRUPTOR** (`player_stage.rs`,
  `player-stage.pid`): cada clique no glifo da coluna abria uma janela nova, e o dono as contou. A
  janela aberta para um palco É aquele palco — sair dele a fecha, e é o caminho de volta a quem
  pediu. **O volume é da linha da barra do tempo**: glifo que emudece e trilho de
  `space.player.volumeBar` células de 0 a 130%, clique e arrasto; `-`, `=` e `0` no teclado.
  **Os arquivos são UMA grade** (`Builder::grid`): até três colunas onde cabem (duas com nome
  comprido, `space.player.browseColumn`), `..` primeiro e o cursor nascendo na primeira ENTRADA.
  **`RET` e o duplo clique ACRESCENTAM à fila** e `P` toca sozinho; o que já está na fila PERGUNTA
  antes de entrar de novo (a pergunta abre no `não`), e toda ação diz o que fez no minibuffer. O
  caminho digitado é uma LINHA da mesma tela (`Target::Typed`), e o "em sincronia" escolhe pela mesma
  tela com outro propósito (`Purpose::Follow`) — uma interface, e quem a usa diz para quê.
  **A coluna abria POR CIMA da janela** porque a zona exclusiva só conta com a superfície MAPEADA
  (`wlr_scene_layer_surface_v1_get_box`), então é o commit do primeiro quadro que re-arranja as
  janelas — e esse commit não leva estado de layer, e sem estado o `handle_surface_commit` não chama
  `transaction_commit_dirty`: o arranjo fica calculado e não enviado até o próximo clique. Dizer a
  zona de novo NÃO serve (o wlroots ignora valor igual ao que já tem; foi o primeiro conserto, e não
  fazia nada). O conserto é o `Compositor::settle` (um `nop` pelo IPC, que termina todo comando em
  `transaction_commit_dirty`), pedido quando o PRIMEIRO QUADRO é mostrado (o frame callback
  pedido nele). Um `sync` depois de apresentar foi a segunda tentativa e CORRIA: o driver pode fazer o
  commit do quadro numa thread dele, e o `sync` chegava antes — a VM mostrou a janela recebendo de
  novo a largura velha.
  **E o teste do `player-vm` passou com o defeito**: comparava a última largura com a PRIMEIRA (o
  padrão da janela antes de ser ladrilhada), e `[760, 512, 512]` era a janela nunca avisada. Agora
  compara com a largura de ANTES da coluna; a sabotagem foi feita num compositor sem cabeça local
  (`760 512 512` sem a chamada, `… 242` com ela, animações ligadas e desligadas).
  **O `.desktop` roda `ronoctua-shell play`** (toca e levanta a coluna), e o `apps::opens` prefere as
  entradas `br.ronoctua.*` — senão quem abre um filme é quem o diretório listou primeiro. **A lista
  de tipos é LIDA do `shared-mime-info` na build** (todo `audio/` e `video/`, mais os de
  `application/` e os nossos), e os padrões vão no `mimeapps.list` de `$XDG_DATA_DIRS`, abaixo de
  toda escolha de alguém. **O que só um decodificador abre ganha UM tipo por família**
  (`assets/player-types.nix`, lido pelo gerenciador, pela base do sistema e pela entrada), e a
  maior parte daquele arquivo é o que ficou DE FORA: extensão que a base já dá a outra coisa
  (`.iso`, `.gsf`) ou que é de outro programa lá fora (`.cff`, `.bam`) só toca quando entregue.
  **XSPF e ASX o daemon abre** (`ronoctua-playlist`, o leitor da prévia descido a crate): o libmpv
  não os conhece e termina na hora. **E o que o libgme lê** (VGM, NSF, SPC, GBS, HES, KSS, AY, SAP,
  GYM) **leva o demuxer NOMEADO** (`engine::load`): o ffmpeg os reconhece com nota 25 e o libmpv exige
  26 — todos respondiam "formato não reconhecido", e o plano os contava como grátis; medido num VGM
  que o `ffprobe` lia sem uma palavra. Baixar o limiar para todo arquivo deixaria um palpite fraco
  vencer noutro lugar. **O player segue a mídia no perfil** (`profile.nix`): libmpv com `ffmpeg-full`
  mais os decodificadores são **1 682 MiB** de fecho, então o `minimal` não o leva, e a tecla diz
  "não instalado" (`client::NotInstalled`, pelo código 5 do `systemctl`) em vez de "não responde".
  **O SponsorBlock tem uma linha por CATEGORIA** (`prefs::CATEGORIES`, as nove que são trecho de
  tempo). **Duas máquinas se provam em duas VMs** (`player-remote-vm`): a API local do Tailscale é a
  única coisa trocada — umas linhas de Python no soquete dele, e endereços da faixa da tailnet na
  rede de teste —, e dali para frente é o código de verdade: código de seis dígitos (o errado é
  recusado), filme por HTTP, e o seguidor abaixo de 80 ms.
  **Os arquivos são um NÍVEL da vista** (`f`, a linha `Navegar nos arquivos…`, `LEADER p l`;
  `player_browse.rs`): um anel com música, vídeos e clipes musicais (a ONZE pasta,
  `XDG_MUSIC_VIDEO_DIR`; sem ela declarada, a de MÚSICA que a máquina declara, nunca um nome
  inventado), as pastas e só os arquivos que o player toca. **O glifo sai NA LEITURA da pasta**
  (`Listing::glyphs`): o `mime::by_name` custa 1,2 ms por nome em debug, as linhas são montadas várias
  vezes por quadro e de novo a cada posição que o player relata, e perguntar a cada quadro congelou
  a `~/main/temp` do dono com uma música tocando — medido, 53 ms por tecla contra 0,5. `RET` entra na pasta ou toca
  o arquivo SOZINHO, `a` põe na fila (a pasta inteira), `DEL` sobe e cai na pasta de onde veio, `Esc`
  volta à vista. **Quem abre uma PASTA é o SHELL, não o daemon** — o libmpv tomaria figura e texto
  juntos, e o arquivo de um decodificador nosso chegaria pelo caminho e não pelo protocolo dele —, e
  o mesmo `player_browse::glyph_if_played` responde à listagem, à pergunta de caminho e ao `ronoctua-shell
  play`, senão uma pasta tocaria o que a listagem não mostrou.
  **O minibuffer COLA** (`C-v`, e o `C-y` do Emacs): o topo do histórico que o `zwlr_data_control`
  já alimenta, numa linha só. Não está no `gesture_of`, porque nas listas que o dividem `C-v` é a
  prévia do gerenciador.
- **`LEADER m c` é a CÂMERA, e `LEADER s c` os ajustes dela** (`camera.rs`/`camera_window.rs` e
  `camera_settings.rs`, duas portas: `s c c`/`s c w`). **Um PROCESSO é dono do vídeo**
  (`ronoctua-studio`, unidade de usuário): uma captura V4L2 serve UM leitor, e gravação, transmissão
  e câmera virtual têm de sobreviver à janela. App e ajustes são CLIENTES do socket dele (JSON por
  linha, estado inteiro), e só a `Capture` do `ronoctua-camera` abre câmera — valor com `Drop` e
  `setpriv --pdeathsig`, cobrado pelo `the-camera-has-one-door`. Em repouso toda fonte está FECHADA
  e a luz da câmera apaga. **A câmera é achada pelo que o KERNEL diz, nunca pelo modelo**: nó que
  não tem `VIDEO_CAPTURE` (metadados) sai, formato só cinza é infravermelho, e a identidade é
  `card|bus|interface`, porque uma webcam USB expõe cor e IR com o MESMO `card` e o MESMO `bus`. O
  nosso `v4l2loopback` (rótulo `ronoctua`, nó 42) nunca é fonte. **A prévia viaja por MEMÓRIA**
  (`memfd` por `SCM_RIGHTS`, três fatias com seqlock, `pread`/`pwrite`, sem `mmap`): pela câmera
  virtual, uma cena com a TELA seria legível por todo programa da máquina enquanto a pessoa só
  olhava a prévia. **Efeitos são DECLARADOS** (`assets/camera-effects.nix`) e rodam em Rust por
  fonte, AO VIVO; cor de efeito é aritmética, não token (inv. 9 é sobre a UI). **O fundo sai de um
  interpretador TFLite NOSSO** (`ronoctua-segment`): o `tract` recusa o modelo (pesos fp16 atrás de
  `DEQUANTIZE` e um operador CUSTOM), são onze operações, e o teste compara contra o MediaPipe
  (pior diferença 5,8e-5). **Cenas e roteiros** moram em `studio/scenes.json` e trocar de cena é o
  compositor LER outro estado: o encoder não reinicia (o `camera-vm` compara o PID). **A chave de
  transmissão nunca vai ao `argv`** (`-/rtmp_playpath <arquivo 0600>`, `stream-key-is-never-an-argument`).
  **A tela é capturada por `ext-image-copy-capture-v1`**, que é protocolo Wayland padrão e não IPC
  nem layer-shell — a exceção ao inv. 3 que o dono aprovou com o plano, do mesmo tipo do
  `wl_data_device`. Marcas `camera` (varredura de fds em `/proc`, 2 s, ~8 ms) e `recording`.
  **Controle de vários valores ABRE UM MENU** (efeito, fundo, câmera, cena): clique esquerdo abre,
  direito anda um passo, `C-<letra>` abre pelo teclado, e o menu que não cabe em altura vira
  COLUNAS em vez de rolar — sem deslocamento, a isenção do `every-list-window-has-a-bar` continua
  provada. `camera::MenuBox` é onde ele é desenhado E acertado. **O ponteiro sobre um botão põe o
  que ele faz no lugar da linha de atalhos**, e isso é recalculado DEPOIS de pintar: o ponteiro que
  traz os controles de volta chega quando ainda não há botão nenhum. **A câmera é lida numa thread
  que guarda só o quadro MAIS NOVO**: lida entre dois efeitos, um efeito lento deixava quadros
  velhos na fila do ffmpeg e o atraso crescia até segundos — agora efeito lento custa quadros,
  nunca tempo. A tela de ajustes tem `Voltar tudo ao padrão`, que pergunta DUAS vezes na própria
  linha e guarda o que a pessoa DIGITOU (cenas, endereço e chave da transmissão).
- **A LINHA ACIMA DA MODELINE TEM DOIS INQUILINOS**: o relance (`LEADER d`) e o retorno do volume,
  e o último a pedir fica com ela. Uma aritmética só (`above_rows`/`above_line_at`), um `Showing` só
  — a pizza, a pausa no ponteiro e o prazo são da FILEIRA e não do que está nela, senão seriam dois
  contadores num rodapé que tem um. Na linha estreita cai a barra primeiro (ela repete o número),
  depois o nome do aparelho, e o número nunca — cortado, `38%` vira `3…`, que é um número falso.
- **As teclas de mídia são a segunda família de binds NUS** (`actions::media`, ao lado de
  `emergency`), e são sete: volume, mudo, mudo-do-microfone, play, faixa. Não entram na lista dos
  quatro de resgate porque o argumento daquela é RECUPERAÇÃO; o desta é outro e mais simples —
  **buffer nenhum recebe um `XF86Audio*`**, então o que se toma é uma tecla que ninguém tinha. Até
  aqui a distro não tratava nenhuma: o `gen-default-binds.py` as recusa por construção dizendo que
  "esperam o sistema tratar as teclas de mídia", e o sistema é este. As três de mídia falam com o
  `playerctl`, que vem do AMBIENTE (`mkIf` do `media.nix`) — e a ausência dele é uma FRASE no
  rodapé, nunca silêncio.
- **`LEADER s p` é energia** (`ronoctua-power`, a fronteira com UPower, power-profiles-daemon e logind — molde do `ronoctua-disks`, três threads e o snapshot inteiro): uma `View` (`power.rs`), duas portas (`s p p`/`s p w`), oito seções como linhas. **Modeline em repouso é muda**: bateria só abaixo de 99% e fora de `FullyCharged` (limite de carga a 80% diz `FullyCharged` para sempre), `%` só a ≤30, `state.error` a ≤15 (regra literal, carregando ou não); perfil só quando NÃO é o topo que o daemon oferece — **o padrão da distro é `performance`** (`ronoctua.power.profile`, piso; `power.json` vence; aplicado UMA vez por sessão, nunca sobre um hold e **nunca quando ninguém nomeou** — o `nix run .#dev` não mexe no perfil do dono). Limites de carga escrevem no sysfs por regra udev (grupo `users`); as travas de sono/tampa são fds na thread de ordens do RODAPÉ (a janela encaminha `power-keep-*`); `s q s/h/r/p` abrem a vista ARMADA e `RET` confirma. **Nenhuma regra polkit nossa** (logind e PPD já dão `yes`; há check). Alarmes `battery-low/critical/charged` por TRANSIÇÃO (`alarms`), pelo catálogo.
- **`LEADER s d` são as telas** (`ronoctua-display`, a fronteira com o compositor por IPC e com o kernel
  por `/sys` e `/dev/i2c-*`): uma `View` (`display.rs`), DUAS portas (`s d d` rodapé, `s d w` janela),
  três níveis (lista → ficha de uma tela → disposição), resolução, taxa, escala, giro, lugar, VRR,
  brilho e o TAMANHO DO TEXTO. **A janela tem RELÓGIO ARMADO, e é a única assim** — QUATRO outras batem SEMPRE, pela vida da
  janela (rede e atualização a cada segundo, notificações a cada dois, o hospedeiro de buffer a cada
  segundo com o painel de memória aberto), e a do gerenciador arma uma só quando o inotify NÃO
  subiu. A contagem regressiva precisa bater e o relógio do rodapé não a
  alcança (são dois processos, duas `View`s); este é armado só enquanto uma pergunta com relógio
  está de pé, e se desarma sozinho. **Mudança que
  pode cegar a tela é PROVISÓRIA**: aplica, pergunta, e volta sozinha em 15 s se ninguém responder —
  o cursor nasce em "voltar atrás" e o relógio é o MONOTÔNICO, senão um passo de NTP para trás
  congelava a contagem para sempre. **Esquecer (`d`) é outra natureza de pergunta**: não aplica nada,
  não tem relógio, e espera — o risco ali é perder escolhas, não perder a tela. **Ligar uma tela apagada é DECISÃO, não prévia** — é a única linha da vista cujas duas direções
  têm naturezas diferentes, e a regra mora no braço dela. **Ordem que não muda nada não é ordem**
  (`keep_for` compara o `Wanted` que montou com o do arquivo): `a` sobre uma posição já vazia
  mandava um `Keep` por repetição de tecla. **E as duas linhas de round trip guardam o que pediram**
  (`text_pending`, `place_pending`), senão segurar a seta anda um passo por ida-e-volta em vez de um
  por tecla. **"Manter" grava UM
  botão, o que foi perguntado** (e o que foi PEDIDO, senão sair pelo fundo da escada gravaria o
  número que o compositor escolheu no lugar de "escolha você"). **Duas portas escrevem o MESMO arquivo, e por isso `Keep` é MESCLA e não escrita** — a ordem
  leva um `Wanted` inteiro, clonado do retrato daquela porta com um campo mudado, e o mapa era
  escrito por cima INTEIRO: guardar uma escala numa tela apagava o que a outra porta tinha acabado
  de guardar em OUTRA, e guardar um giro na MESMA devolvia a escala que aquela porta viu por último.
  `Eyes` guarda a BASE (o que a porta foi mostrada) e mescla campo a campo — meu onde mudei, do
  disco onde não. Esquecer continua limpando tudo porque aí todo campo difere da base. **E um
  `display.json` que não parseia não é sobrescrito, **e a lista DIZ isso** (`Store::parsed`,
  `Snapshot::settings_failed`): o `read` cai no padrão, que é certo para quem lê e fatal para quem
  escreve — uma tecla de tamanho de texto bastava para gravar `{"screens":{}}` e esvaziar o
  fragmento junto. Recusar salva o arquivo e não basta: com o padrão no lugar, TODA tela desenha
  como se nada tivesse sido escolhido, e o que a pessoa vê são os ajustes dela sumidos. É a mesma
  falha muda que uma vista deve à pessoa, e sai pela porta de sempre — uma frase na lista. **E não
  há gêmea dela para o config do COMPOSITOR**: ele não relata isso em lugar nenhum que um cliente
  ouça (medido: `reload` responde `success` com fragmento quebrado E com config principal
  quebrado, e `--validate` só devolve o veredito do arquivo PRINCIPAL, porque
  `load_include_configs` é `void`). Quem responde no lugar é o portão de build —
  `display-fragment-validates` põe o texto exato que o escritor emite na frente de um compositor de
  verdade, com uma metade quebrada ao lado. Esquecer TIRA a entrada do arquivo (`Preferences::set`, não
  `insert`): mapa que só cresce vira arquivo listando todo monitor que já passou pela máquina, cada
  um dizendo nada. **ORDEM NÃO IMPORTA no fragmento, e isso é propriedade deste compositor**: bloco
  de saída é MESCLADO — `*`, depois o conector, depois o identificador, campo a campo com o último
  vencendo —, então os nossos, que são conector e identificador, ganham por construção. O anterior
  ficava com o PRIMEIRO que casasse, e como o nosso arquivo tem as duas espécies de chave um bloco
  velho de `DP-1` dizendo `off` apagava o monitor novo que entrou naquela porta. **O preço da mescla
  é escrever `enable` EXPLÍCITO**: silêncio no bloco mais específico agora significa o que um bloco
  menos específico disse. **O arquivo é o que sobrevive ao reboot e o IPC é o que faz o compositor
  concordar com ele** — e escrever sozinho não faz NADA, porque ele não vigia arquivo. **Desfazer é
  UM comando**: `reload` libera e reconstrói `config->output_configs`, então toda prévia deixa de
  existir de uma vez. **E fechar a porta com uma prévia de pé a devolve ANTES de sair**
  (`Handle::finish`, que manda e ESPERA a thread): sair sem isso deixava a mudança valendo até o
  compositor reiniciar, e uma porta nova não ajuda porque o `previewed` é da thread que aplicou. Brilho interno é udev no
  sysfs; externo é DDC/CI escrito por nós (`rustix` + `/dev/i2c-N`), sem `ddcutil`. **Cache de pixmap LEVA O TAMANHO NA CHAVE; esvaziar não alcança a GPU** — o `Rgba` é
  rasterizado num número de PIXELS e esses números saem do `text::size()`. Esvaziar o mapa do
  hospedeiro parece o conserto e não é: quem DESENHA é a textura em `image::Images`, que **não tem
  remoção por chave** (o `clear()` só limpa o lote do quadro e diz que as texturas ficam; o `dump()`
  só age acima de 39 e guarda o que está na tela). Então o ícone era redecodificado a 36 px, o
  `missing()` respondia "já subiu" sobre o de 18, e o localizador desenhava esticado pelo resto da
  sessão. `apps::pixmap_key` e `buffer::svg_key` carregam o lado, e o `svg_key` carrega **o token
  também** — a tinta é `tema × token`, e só o tema estava lá. O `.clear()` que sobrou é teto de
  memória, não correção. **O tamanho do
  texto vale para o shell E para os apps**, e o canal é a DESCRIÇÃO DA FONTE, não o multiplicador
  (`toolkits.rs`). Medido, não suposto: o `qgnomeplatform` assina `changed::font-name` e
  `changed::monospace-font-name` e **não** o `text-scaling-factor` — então o Qt nunca seguiu o
  multiplicador, por mais que o perfil `full` pague 591 MiB pelo tema de plataforma. E o GTK desenha
  a `pontos × (gtk-xft-dpi/96)`, com o `gtk-xft-dpi` vindo do multiplicador, então escrever os DOIS
  escala o GTK duas vezes. Como não existe canal que alcance o Qt sem alcançar o GTK, o tamanho vai
  na fonte, o multiplicador é ZERADO (resíduo dele multiplicaria o que agora viaja na fonte: o
  quadrado do que a pessoa pediu), e a escala sai SEMPRE da descrição intocada, guardada em
  `toolkits.json` — reescalar o que já está no dconf compõe, e 125% duas vezes é 156%. **O Ghostty
  tem `font-size` próprio**: ganha `ghostty-size.conf` (arquivo SÓ dele — o do tema é copiado
  inteiro do store e apagaria o tamanho), incluído por último, mais um `SIGUSR2`, que é o que a
  1.3.1 recarrega e não está em manual nenhum — foi lido no binário, e o
  `ghostty-reloads-on-a-signal` reancora se a versão mudar. Provado de ponta a ponta pelo
  `+show-config`: 11 → 22 no teto. A aritmética do passo é do token, uma só
  (`tokens::text::stepped`, que recebe de ONDE se parte — a vista segura o que pediu até o arquivo
  voltar), e a faixa é travada contra o schema do GNOME (0,5–3,0) no `gen-tokens.py`.
- **`LEADER s m` é o painel de memória**, e a capacidade `system` é a trava: o host amostra
  (`ronoctua-memory`, única fronteira com `/proc`) e manda `input::survey`; o buffer desenha e pede
  por `effect::ask`. Relógio de 1 s lê pseudo-arquivo; varredura e `systemctl` vão para thread.
- **Duas instalações, uma opção só.** `ronoctua.profile` = `full` (22,8 GiB) ou `minimal` (6,9);
  `nix/modules/profile.nix` é a tabela, tudo `mkDefault`. **O ISO leva no `storeContents` o que
  nenhum cache tem** (shell, compositor, ghostty-vt, buffers, fontes): o alvo baixa, não compila.

## Mapa do repositório

```
assets/tokens/  tokens em Nix (verdade);  assets/icons/  app id e categoria → glifo;  assets/lexicon/  à mão
assets/i18n/  languages.nix (a LISTA) + um .ftl por idioma, todo texto de UI (inv. 8)
nix/fonts.nix  5 derivações + as 27 variantes + PSF
nix/fontconfig.nix     fallback do sistema;  nix/primary-font.nix  a do renderer, sem emoji
nix/graphics-host.nix  driver para host não-NixOS;  nix/rust.nix  os caminhos do store
nix/dev.nix  loop aninhado; reload.nix observa, footer-single.nix reinicia;  crates/  grafo no ARCHITECTURE
nix/modules/  NixOS;  nix/disko/  disco;  nix/installer/  ISO;  buffers/  wasip2, workspace PRÓPRIO cada um
tests/checks.nix       todos os checks; m1..m8 trazem o critério como comentário
```

## Armadilhas concretas já encontradas

- **NADA de build na árvore — 793 GB.** `CARGO_TARGET_DIR` fora (`lean-tree`); **nunca `${../.}`**.
  **Uma derivação referencia o que os BYTES dela citam**: extraia o binário (`rust.installer`,
  `rust.rescue`, `rust.filesRoot`, `rust.transfers`) — o workspace inteiro punha Zig+clang+LLVM no ISO.
  **Interruptor que não interrompe**: `notoExtra` e `icons.superpack` desligavam e o pacote entrava
  por outra porta — meça com `nix why-depends`, não confie no `mkIf`. **E o número que justifica
  um interruptor envelhece**: o `notoExtra` prometia poupar "mais de 1 GB" e poupava **49,1 MB**
  (medido, `nix path-info -S`) desde que o `noto-fonts-extra` foi absorvido no `noto-fonts` na
  26.05. A opção morreu em 2026-09-08 e o `noto-fonts` é incondicional: um perfil ficava sem
  árabe, hebraico, devanágari e tailandês por cinquenta megabytes que lhe disseram serem mil.
  Meça de novo antes de confiar num tamanho escrito num comentário. **`option_env!` com caminho de
  store é referência, e quem COMPILA a paga**: ícones e os buffers vêm do AMBIENTE, senão editar
  buffer reconstrói o workspace; ela sobe uma derivação (`rust.shell`) e nunca some.
- **`Color` é PAPEL, não valor**: `u16` que resolve em `themes::TABLE[tema][papel]` ao desenhar; `==` compara papel; `to_rgba8()` não é `const`. Tudo que uma CRIANÇA pinta com o valor (waveform do ffmpeg, `currentColor` de SVG) leva o tema na CHAVE do cache, senão a textura velha fica no GPU. Só UM teste chama `theme::set` (no tokens) — os testes correm em paralelo. Consumidor Nix lê o STORE (`${themed}/…`), nunca `colors.nix`: lá não há valor.
- **Nunca peça `Family::Monospace`**: fallback, **4 447 ms** contra 0,76 ms. **Buffer não tem parser
  de JSON**: campo nenhum leva caractere cru. **Token vira WIT**: segmento começa por LETRA.
- **Dentro de `''…''` valem DOIS idiomas**: `#`/`//` é do script embutido, mas `${...}` é do Nix e vive até em linha comentada; **`` ` `` é substituição de comando ATÉ no comentário**; e a fence FECHA num `''` qualquer — inclusive dentro de comentário (`'''`, `''$`, `''\` escapam).
- **Seletor Fluent com ramo NUMÉRICO recebe `Arg::Number`, nunca texto.** O Fluent escolhe a
  variante pelo TIPO do argumento: com a string `"1"` ele procura uma variante chamada
  literalmente `1`, não acha, e cai no padrão — em silêncio. Já custou `1 screens` em toda máquina
  que é um laptop (o `display.rs` guarda a medição, e a frase dele termina em *"with every check
  green"*), custou o mês por extenso uma vez, e custou os três dígitos do octal dizerem
  `pode ler, alterar e executar` — o `777` escrito numa frase sobre `644`. Quem achou foi um teste
  que IMPRIMIU as três frases; nenhuma asserção de "não está vazio" veria. Cobra o
  `numeric-selectors-get-numbers`, que lê as chaves com ramo numérico dos `.ftl` e recusa
  `t_with`/`t_marked` sobre elas.
- **Chave de texto tem SETE derivados**, só um é o `.ftl`; **pergunta do instalador tem 13** — os dois no ARCHITECTURE. **`to_linear()` desfaz a curva sRGB**; **célula = 0,600 × fonte**; **caixa 1,25 em** contra entrelinha 1,45. `tiny-skia` entrega RGBA PRÉ-MULTIPLICADO. **Uma run ocupa `heights.of(escala)` CÉLULAS, e a escala alarga tanto quanto sobe**: `Large` são DUAS, e a régua em `line+1` virava sublinhado. Título é do tamanho do CORPO — hierarquia é cor e ar, nunca tamanho nem régua; os dois `DisplayList::nothing_*` cobram isso em TODO buffer, em TODO estado.
- **Download sem versão é hash com prazo de validade.** GeoNames e Geofabrik regeneram os arquivos
  TODO DIA sob o mesmo endereço, e o hash fixado só valia enquanto o store guardava a cópia: uma
  coleta de lixo no host (2026-09-22) a levou e o `.#dev` morreu em `hash mismatch` sem nada ter
  mudado aqui. Endereço vivo vira CAPTURA (`snapshot` no `nix/atlas.nix`, `id_` = bytes
  originais), e o `downloads-are-pinned` recusa o resto. E o store não é a única cópia de nada:
  `nix/kept-sources.nix` é uma RAIZ de GC com todo input do flake e todo download, registrada
  pelo `.#dev` e pelas baterias — as fontes apontam para uma release que ainda não existe.
- **Fallback silencioso precisa de check**: a tabela de tray era validada no build e não chegava ao
  Rust. **Há DUAS réguas de célula, e elas respondem a perguntas diferentes**:
  `ronoctua_protocol::cells` conta CLUSTERES — reserva, confere e corta, uma definição para os dois
  lados —, e o campo `width` do banco é o AVANÇO (emoji vale 2 células, e só a fonte sabe). Cortar por
  CODEPOINT inventa desenho; medir avanço com a contagem espreme a coluna vizinha. Cobra o
  `ink-is-not-a-count`. **Cor de token não alcança emoji**.
- **`Limits::downlevel_defaults()` trava a textura em 2048 px** e a superfície É textura: `Surface::configure` devolve `()` e **mata o processo** — use `using_resolution`. **`systemd-run --scope` é SÍNCRONO**: no `calloop` de uma thread, esperar congela o rodapé. **Check contra o CÓDIGO, não o CRITÉRIO,** fecha marco vazio.
- **`cargo test` não tinha os cinco programas, e todo teste de ffmpeg era VAZIO** — eles saem por
  `let Ok(x) = ffmpeg() else { return }`, e nem o devshell nem a derivação `tests` definiam as
  variáveis. Medido: o teste de rotação fecha em 0,00 s sem elas e 0,75 s com elas — nunca construiu
  os três vídeos que compara. Vão na derivação `tests` e NÃO em `commonArgs`: a build do workspace
  não pode referenciar o ffmpeg.
- **Três baterias, e só o gate fecha**: `quick` (dez escolhidos), `most` (tudo que não constrói instalação), `gate` (tudo). `tests/slow.nix` diz o que sai do `most`, e `test-tiers-are-total` cobra a divisão. **`nix flake check` SEM `-L` não mostra log** — silêncio não é sintoma. **`nix fmt` sem argumento formata a árvore**; `--check` confere.
- **`teste && comando` como ÚLTIMA instrução de um corpo de laço derruba a sessão inteira.** Com
  `errexit` e `pipefail` — que é o que o `writeShellApplication` liga —, o teste falso faz o
  compound sair 1, o laço herda, o pipeline herda, e o script morre. O dono viu isso como um
  `.#dev` que imprimia o banner, imprimia o `config:` e voltava ao prompt **sem dizer nada**. Um
  `if` sem `else` sai 0 e é o conserto. O irmão dessa armadilha é o `ls` sem correspondência, que
  sai não-zero e faz o mesmo. **E o check não via nada disso**: ele rodava num sandbox com o `/tmp`
  VAZIO, então todo laço da poda retornava 0 por não rodar — a regra entrou verde e quebrou na
  primeira máquina de verdade. Quem varre diretório tem de PLANTAR arquivo no teste, e plantar o
  caso que dói (mais de dez logs do MESMO dia, que é o que faz a idade reprovar e o corpo terminar
  em falso).
- **`rustc` morto por `signal: 15` SEM diagnóstico é o `earlyoom` do HOST** (`-m 10,5`, `rustc` no `--prefer`), e `--max-jobs` não salva: o que estoura é jobs × cores. Por isso o gate mede e limita o produto. **`cannot download … from any mirror` NÃO é arquivo removido**: o `fetchurl` repete SEM retomar, e o android-studio (1,37 GB, unfree, um espelho, fora de todo cache) reseta aos ~150 MB — quem baixa é o `nix/prefetch.nix`, que o gate roda ANTES de compilar. **`pkill`/`pgrep` casa o PRÓPRIO comando**, e **`git checkout <arquivo>` apaga o não-commitado** (o store guarda cópia: `ls -dt /nix/store/*-source`).
- **Modificador segurado mata o keymap.** No aninhado a leader É `Ctrl+Alt+Espaço`: DESCONTE o que já estava em baixo ao ganhar o foco E ao começar a sequência. Caps Lock idem — `not bound` mudo. **Um bind é casado por IGUALDADE** (`modifiers ^ binding->modifiers`), então um bit sobrando mata TODOS os binds da sessão, leader inclusive. **O que o prendia PARA SEMPRE era do compositor anterior, e não deste**: lá o `winit` só dizia `Focused(false)`, o smithay descartava evento sintético e o estado de modificador era CONTADO (pressões contra soltas), então uma tecla que descia aqui e subia lá fora nunca voltava. O backend Wayland do wlroots faz o contrário: implementa `.modifiers` e passa a máscara do HOST direto (`wlr_keyboard_notify_modifiers`), e o handler de tecla manda `update_state = false` — o estado aqui é ESPELHO do de lá, nunca derivado. **Isso ainda não foi MEDIDO neste compositor**, e por isso o `nix/keyboard-nested.nix` continua de pé: ele tira o Super do keymap do aninhado, que é o único modificador que o laço dispensa e o que todo acorde do host carrega. A medição é um gesto (host chord com Super, perder e recuperar o foco, tentar a leader) e está no `docs/PENDING.md`; passando, somem o arquivo, o check `nested-survives-a-stray-modifier` e o segundo bind com Shift. Em produção Super FICA — lá ele É a leader. **Não tente consertar do lado do cliente**: o `zwp_virtual_keyboard` escreve nos CLIENTES e nunca no estado do assento, e o IPC não expõe modificador nenhum — de dentro, preso e segurado de verdade são indistinguíveis.
- **"Nenhum atalho funciona" tem três causas com a MESMA tela** — tecla que não chega, modificador preso, foco não concedido: `nix run .#diagnostics` ANTES de supor. No Xvfb ninguém tem foco (`xdotool windowfocus`), e **`xdotool key` SOLTA a tecla**: segurar exige `keydown`.
- **`LD_LIBRARY_PATH` vence o `DT_RUNPATH`**: pôr `/usr/lib64` lá para achar UMA lib traz a `libc` do host junto, e ela só combina com o `ld.so` da MESMA build (`undefined symbol ... GLIBC_PRIVATE`). O driver vem do Nix (`mesa`); do host, só para NVIDIA. **ICD Vulkan ausente custa 650 ms mudo** — filtre por vendor. **`memory_init_cow`** é obrigatório (p99 95× pior); **`Queue::present`** precisa do frame, que o `Drop` DESCARTA sem erro.
- **Sem GPU, escolha o backend E o renderizador**: `WLR_BACKENDS=x11` (sobre `Xvfb`) ou `headless`, com `WLR_RENDERER=pixman` — o segundo é o que tira EGL, GBM e o nó DRM do caminho. Sem eles o DRM sobe com zero output e uma layer-shell sem output nunca é criada. **`RUST_LOG` nosso filtra o compositor**, que sobe MUDO — então não espere o display do log dele: **fixe o socket** (`SCROLLSOCK` é honrado quando o arquivo ainda não existe) e peça o `WAYLAND_DISPLAY` a ele por um `exec` do próprio config. **E `--validate` roda em sandbox** com esses dois: bom sai 0, quebrado sai não-zero — mas **erro dentro de um `include` NÃO chega ao status**, porque `load_include_configs` é `void`; leia a SAÍDA.
- **Quem tem o codepoint primeiro GANHA** — a face pedida por nome vence o fallback. Custou 634 da PUA (Lucide → `U+F2000`) e 75 emoji da Iosevka (`Emoji_Presentation`); `FE0F` vira dois glifos.
- **A CADEIA DE FALLBACK É DECLARADA E TEM UM DONO SÓ**: `nix/fontconfig.nix`. De lá saem TRÊS
  coisas — o `.conf` do fontconfig, o `nix/coverage-fonts.nix` (família → arquivo, e **recusa a
  build** se uma família não tiver face) e o `nix/chain.nix`, que gera o `impl
  cosmic_text::Fallback` do renderer. Por um ano a segunda cadeia foi só um comentário: o
  `chainJson` era gerado e lido por nada, o banco eram QUATRO faces embutidas, e todo japonês,
  árabe, hebraico, devanágari e coreano da tela era **tofu** — mudo até no log, porque o aviso do
  cosmic-text cai em `log::debug` e nada aqui liga o `log` ao `tracing`. O motor nunca faltou: o
  cosmic-text 0.19 traz **harfrust**, `unicode-bidi` e `unicode-script`, e o `Shaping::Advanced`
  já era pedido. Faltavam FACES. Pedimos a CJK **monoespaçada** (a tabela do upstream pede a
  proporcional, que uma grade não coloca), e as 44 faces viajam no EMBRULHO
  (`RONOCTUA_COVERAGE_FONTS`, 54 MB) — nunca em `generated`, que reconstruiria os 33 crates.
  Ausência não é erro: instalador e resgate seguem com as quatro, e `Text::coverage` diz quantas
  subiram. **A face de emoji é a PRIMEIRA da cauda comum** — atrás do `Noto Sans Symbols 2` ela
  perdia `U+1F550`, e o relógio voltava a sair monocromático. **Sonda que a face primária desenha
  não prova nada** (a primária é pedida por NOME e ganha de toda cadeia): o gerador recusa uma, e
  a do armênio é `U+FB13` porque a Iosevka tem 91 dos 96 codepoints armênios.
- **A CÉLULA NÃO É UMA CONTAGEM** — `ronoctua_protocol::cells` soma `East_Asian_Width` (W/F) mais
  `Emoji_Presentation`, de tabela gerada dos arquivos do Unicode (`nix/width.nix`, 123 faixas).
  `日本語` são três grafemas e SEIS células. `A` (ambíguo) fica ESTREITO: é cirílico, grego e o
  box-drawing de que a interface é feita. `cells_of` continua grafemas — é por onde se CORTA — e
  por isso `elide_end`, `elide_middle`, `window` e `wrap` **orçam em células e cortam em
  grafemas** (`prefix_of`/`suffix_of`); `take(max)` de grafemas contra orçamento de células
  transborda para cima da coluna vizinha. `ronoctua_render::layout::width` era uma SEGUNDA cópia
  e virou a função do protocolo. Medido: o ideograma anda **1,667 célula** das 2 reservadas (o
  hangul 1,533) — sobra vão, nada se sobrepõe; o check recusa só acima de 2,02.
- **`LEADER c` é uma VISTA agora** (`clipboard_view.rs`), e era a única modal do rodapé que não
  tinha módulo. Ela não tinha `OF_THE_VIEW` nem `hint()`, era o único braço `_ => None` do `match`
  de dicas, e a tira de ações era refeita à mão a partir dos rótulos LONGOS `cmd-*`, numa cor só,
  com o `·` escrito no fonte Rust. **E desenhava TRÊS LINHAS por número absoluto** (`end-3`,
  `end-2`, `end-1`) sobre um `grid.lines` que vale **1** com o histórico vazio — então elas caíam no
  minibuffer e na própria linha da lista. O dono fotografou. Valia para qualquer histórico com menos
  de quatro itens, e **não havia um único teste de layout do clipboard**. Agora nada calcula linha:
  a linha carrega as DUAS metades como campos (histórico à esquerda, fatos do item à direita, a
  régua na costura), a dica sai do `chrome::hint_of` na linha do minibuffer com elisão e gaveta, e o
  `Click::Candidate` — que roteava para o `M-x` e fazia clique em linha não fazer NADA — virou alvo
  próprio. **E TODA tecla passa pela vista, as setas inclusive**: `<down>` era tratado no rodapé e
  mexia no `clip_cursor`, que conta ENTRADAS, deixando o cursor da VISTA onde estava — três setas e
  um `RET` copiavam a entrada ZERO, e tudo desenhado abaixo da lista (a seção inteira do
  ClipCascade) não tinha caminho pelo teclado, só pelo mouse. O `Key::simple` ainda comia o shift,
  então anel nenhum girava. Os seis ajustes do ClipCascade que são NOSSOS — imagens, aviso, tamanho,
  sal, rodadas e a marca — são linhas dessa seção; preferência que ninguém alcança tem um valor só,
  para sempre.
- **UMA PORTA NÃO APRENDE A PRÓPRIA PRÉVIA DE VOLTA COMO ESCOLHA.** A prévia do tema ESCREVE o
  `theme.json` (é o que a faz alcançar as outras portas), o rodapé vigia esse mesmo arquivo, e o
  `theme_changed` movia `committed` para o que o arquivo dissesse — inclusive para a própria escrita,
  um instante depois. Uma seta bastava: o `Esc` ficava sem nada para onde voltar e virava tecla que
  não faz nada. Medido no `.#shots`, com o `sym: Esc` no journal e o arquivo idêntico antes e depois.
  A vista guarda o par que MOSTROU (`previewing`/`previewed`), e só o que não for isso é escolha de
  outro. **E foto sem asserção não prova nada**: o subteste do Tor passou DUAS vezes sobre a tela
  errada — nove `TAB`s que caíram no Wi-Fi, depois um `fresh_at` procurando uma linha que máquina
  sem Tor não desenha. A ficha é um NÍVEL, não uma linha (`fresh_on_the_tor_sheet`), e o subteste
  agora compara a assinatura das duas imagens.
- **`LEADER s h` é o HISTÓRICO DO SHELL** (`ronoctua-atuin` + `history.rs`): relance, a conta,
  **o que sai desta máquina**, a busca, as teclas e a gaveta. Por anos a distro instalava e afinava
  o `atuin` e não oferecia tela nenhuma — e o `home.nix` recusava a sincronização por escrito
  (*"NO SYNC … every command, including the ones with a token in argv"*). O dono pediu o contrário,
  e duas coisas que aquela frase não pesava são por que dá para oferecer sem o risco mudar: a
  sincronização do Atuin é **ponta a ponta**, e com `ronoctua.atuin.server.enable` o servidor pode
  ser desta casa. O que NÃO mudou é o comando com token no argv — e é por isso que o
  `secrets_filter` e o `history_filter` são **linhas desenhadas** da seção que liga a
  sincronização, nunca padrões dentro da gaveta: salvaguarda que ninguém lê não é salvaguarda.
  **Nasce desligada.** O `config.toml` é SEMEADO uma vez (a forma do `~/.shellrc.local`) e a vista o
  edita uma LINHA por vez — `programs.atuin.settings` do home-manager escreve um symlink do store,
  read-only, e escolher entre dois modos de busca não é decisão do tamanho de um rebuild.
  **A chave (`atuin key`) é MOSTRADA**, atrás de uma pergunta que abre no NÃO: ela é o que decifra o
  histórico e a segunda máquina não sincroniza sem ela, então escondê-la mandaria a pessoa ao
  terminal. E a thread só sobe na primeira abertura: `atuin status` é PROCESSO, não socket, e uma
  máquina que nunca abre a vista não paga um fork a cada 30 s — mas a MODELINE não pode esperar por
  isso, então o retrato nasce de `at_rest()`, que lê só ARQUIVOS (o `config.toml`, o `atuin.json` e
  o token de sessão em `$XDG_DATA_HOME/atuin/session`) e não roda nada. Sem ele a marca ficava muda
  até alguém abrir a vista, que é uma marca respondendo por ausência.
- **O ANEL DAS TRÊS MARCAS SAI DO RETRATO, nunca de um campo lido na partida.** `clip_prefs` e
  `atuin_prefs` eram cópias feitas uma vez: girar o anel gravava o arquivo e a modeline seguia
  desenhando a resposta velha até o rodapé reiniciar. O arquivo é o broadcast — as duas portas
  escrevem, e cada uma tem `Notice::on_file` sobre o seu, tomando **só a marca** (o resto do
  retrato é da thread, e uma porta que o reconstruísse de um arquivo seria uma segunda leitura).
- **O ClipCascade: o `salt` e o `hash_rounds` são da PESSOA, e nenhum endpoint os serve.** Medido no
  upstream instalado (3.2.0): `core/config.py` os inicia em `""` e `664937`, o `cli/login.py` os
  PERGUNTA, e toda máquina de uma instalação tem de receber os mesmos dois à mão. O `/max-size`
  responde `{"maxsize": n}` e mais nada. Um cliente que os pedisse ao servidor derivaria uma chave
  que não decifra nada, **e o único sintoma seria um clipboard vazio** — foi o que este crate fez
  por uma entrega, acertando por acaso porque os dois valores lidos caíam nos padrões do upstream.
  Os dois são linhas da vista, e `MOST_BY_DEFAULT` é 1 MiB porque é o fallback do upstream.
- **O clipboard LÊ O QUE LHE OFERECEM** (`ronoctua_clipboard::decode`): UTF-16 nas duas ordens com
  e sem BOM, a assinatura UTF-8 (que sai), e `STRING` como Latin-1, que é a ICCCM e não um
  costume. Era `String::from_utf8` seco, e uma cópia de app Java ou Wine **sumia** — nem mojibake
  nem erro: não entrava no histórico, e um `debug!` era o único rastro. O que ele não sabe ler
  vira `Content::Unreadable` e uma LINHA na lista com o tipo, porque histórico vazio e tipo
  recusado são iguais na tela. `COMPOUND_TEXT` fica de fora de propósito: é com estado e
  sequências de escape, que sairiam como letras. E o `Backspace` do minibuffer apaga um GRAFEMA,
  não um `char` — a máscara de senha ao lado já contava grafemas.
- **As fontes são asset de Release, nunca git** — 4,1 GB; o input é **`file+https://`**, senão o Nix infere tarball e descompacta no store. **O console não consome TTF**; **`/etc/nixos` e `/etc/fonts/conf.d` são read-only** (`localConf`). Genérico é `sans-serif`; ISO é `image.baseName`. Glifo **não** leva `nf-`.
- **`systemd.oomd.enable` JÁ é `true` e não vigia nada** — arma é `ManagedOOMMemoryPressure=kill` numa slice. `session.slice` é IRMÃ da `app.slice`: vigiar só a segunda tira o desktop do conjunto por construção, e `enableRootSlice`/`enableUserSlices` desfazem isso. **`memory.min` é limitado por TODOS os ancestrais** — quatro drop-ins ou vale zero. Porcentagem o systemd resolve na MÁQUINA. **`OOMScoreAdjust` em unidade de usuário é ignorado em silêncio**: suba o app com `choom`. **`grep -r` não segue symlink**, e todo `/etc` do NixOS é symlink: `-R`.
- **No btrfs, `nodatacow`/`compress` são do SISTEMA DE ARQUIVOS**: a 1ª montagem vence em silêncio; hibernação depende do NOCOW (`lsattr` → `C`). **PCR 15 exige UKI medida.**
- **`#[repr(C)]` promete ARRANJAR, não conferir** — layout errado de FFI não dá diagnóstico: confira com `sizeof` em C. Ghostty: `Point` tem união (24B), `GridRef` é *sized struct*, `Cell` é `uint64_t`. No libtorrent isso virou check (`torrent-layout-matches-cxx`), e o `-fvisibility=hidden` esconde o `extern "C"` TAMBÉM — precisa de `__attribute__((visibility("default")))`. **Componente wasip2 não pode ser membro do workspace do host** (`rust-lld` recusa o version script mudo); **`char` é embutido do WIT**. **API gráfica mudou muito — leia a fonte.**
  **nixpkgs 26.05:** `noto-fonts-emoji` → `noto-fonts-color-emoji`; `nixfmt-rfc-style` → `nixfmt`; `xorg.libX11` → `libx11`; `pkgs.system` → `pkgs.stdenv.hostPlatform.system`. **Atributo dinâmico em duas sentenças** é erro de avaliação: um attrset só.
- **`take` é TETO, nunca contagem — e o `usize::MAX` é sentinela.** `View::rows` pede "todas"; enquanto
  `take` só limitava um iterador finito isso era grátis, mas no dia em que a função passou a
  MATERIALIZAR linhas virou pedido de 2⁶⁴ `Row` vazias. Vinte e sete testes crescendo esse `Vec` ao
  mesmo tempo comeram os 14,6 GiB em segundos, e **o `systemd-oomd` mata o cgroup INTEIRO da janela**
  (`app.slice`, num host que o vigia): o agente morre junto e não sobra rastro na sessão —
  seis vezes em vinte minutos. Processo morto por OOM **não gera core dump**; "o terminal fechou
  sozinho" + `coredumpctl` vazio = suspeite de OOM primeiro (`journalctl -b | grep 'oom-kill'`).
  Bateria pesada vai em jaula: `systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=512M`.
- **O gerenciador tem QUATRO seções, e `Section` é a única definição de cada uma** — onde começa
  (`from`..`to`, `top`/`lines`), o que tem (`rows`, já no deslocamento dela) e quanto tem (`total`).
  `region_rows` achata, `hit` lê, as barras e a roda perguntam. Quem decidiu isso em vários lugares
  produziu, um a um, TODOS os defeitos que o dono reportou: clicar em pasta subindo um nível, a
  lateral sem hover nas duas primeiras linhas, a faixa acendendo a listagem, e o topo da prévia se
  mexendo ao rolar a listagem (era `chrome - offset`). **A faixa de baixo não é da região**: tem
  linhas próprias, o `row_count` não a conta e as portas a desenham por `strip_rows`.
- **Duas grandezas que atravessam em UNIDADES diferentes, e um formatador só.**
  `Vital::RamUsed` viaja em POR CENTO e os dois `*Stall` em POR MILLE — o protocolo diz isso em cada
  variante — e o brain dividia os dois por dez: uma máquina com 31% da memória desenhava **3,1%**, no
  rodapé da home e em toda página de tarefa. Três por cento é plausível, então lia-se como fato. O
  teste era VERDE porque perguntava `percent(310)`, que é a pergunta feita na unidade que faz a função
  acertar. Agora a unidade sai da VARIANTE (`reading(mind, of)`), o que torna escolher a errada
  indizível. Grandeza que cruza a parede leva a unidade no nome ou no tipo.
- **Orçamento de coluna é da LÍNGUA MAIS LONGA, nunca do inglês** — e o `*-driven` só mede o que foi
  DESENHADO (`every_label_fits` lê o que `driven::note` viu). A coluna `note` do brain reservava 12 e
  `brain-hold-cpu` sai `Processador ocupado`, 19 células; a bateria só acusou `ram` porque nenhuma
  fixture segurava por processador. Com uma fixture por motivo de espera, o mesmo check passou a
  acusar CINCO rótulos cortados — dois deles em INGLÊS. Ao criar uma vista, desenhe UM quadro por
  variante do enum que vira chave.
- **REPEGAR o teclado exclusivo manda um `leave` ANTES do `enter`.** Este compositor responde a uma
  RE-aquisição de `keyboard_interactivity` exclusiva com um `leave` e então um `enter` na MESMA
  superfície — medido no `leader-vm`, 7,6 ms entre os dois —, e no fim o rodapé está com o teclado.
  A primeira aquisição da sessão não tem `leave` nenhum: o que produz o par é LARGAR o teclado e
  pedi-lo de volta, que é toda leader depois da primeira, e é pior numa máquina sem janela nenhuma
  — o primeiro minuto da sessão. Tomar a primeira metade por perda fecha a leader no instante em que
  ela abre, e na tela é uma lista que pisca e some. Quem guarda é o `KeyboardHold` (`surface.rs`):
  perdoa UM `leave` por aquisição, e o `enter` que fecha o par não perdoa mais nenhum.
- **Spawn em unidade do systemd: use CAMINHO, nunca nome** — o `PATH` é mínimo e a falha aparece longe da causa. O caminho do store vem por `option_env!`, posto no `nix/rust.nix`. E **`min_cols` é piso de PREFERÊNCIA**; o que não cede é "nada vaza da tela". `Shrink`≠`removable`. **Upstream do compositor: `dawsers/scroll`**, que segue os lançamentos do sway — pinar tag, nunca `main`.

## Como trabalhar — o dono cobrou o TEMPO duas vezes, e a causa era sempre a mesma

Estas regras existem porque tarefas de poucas linhas levaram mais de uma hora. Elas não são sobre
pressa: são sobre não gastar relógio no que não prova nada. **Valem para qualquer agente**, e por
isso moram aqui, no único arquivo que chega sozinho ao contexto.

- **ENTREGUE A CORREÇÃO E A TELA; O CHECK É OFERTA.** Faça o que foi pedido, mostre o PNG, e **pare**
  — dizendo qual check você acrescentaria, o que ele pega e quanto custa. Quem decide o escopo é o
  dono. Não vale para o que o pedido claramente inclui, nem para deixar sem conserto um check que
  você ACABOU de ver falhar: aí é reportar, não seguir em frente.
- **`cargo clippy --workspace` NÃO alcança os buffers, e check nenhum os lê.** O `nix/rust.nix` tira
  `buffers/` da fonte de propósito (um comentário editado lá recompilava o workspace inteiro) e o
  `nix/buffers.nix` os CONSTRÓI — build não é clippy. Rode à mão no buffer que tocar:
  `cd buffers/<id> && cargo clippy --all-targets --target wasm32-wasip2 -- -D warnings`. Achou SETE
  no brain, e duas eram uma FEATURE ausente: `brain-next` e `brain-soon` vivas nos dois idiomas com
  nada desenhando. Os três de tabela assada (`glyphs`, `lucide`, `unicode`) pedem variáveis de build
  e falham fora do Nix — isso não é lint.
- **SÓ OS CHECKS DO QUE MUDOU** (drivers do que você tocou, `nix fmt -- --check`, `english-names`:
  ~3 min). O `most` são **16 minutos** e fica para antes do commit; o `gate` é do dono. E **congele a
  árvore antes de disparar bateria** — o Nix a fotografa na avaliação, e editar um comentário no meio
  obriga a refazer tudo. Diga sempre no relatório qual recorte rodou e qual não.
- **ITERE NO `cargo`, NUNCA NO NIX.** Tocar `crates/ronoctua-protocol` invalida `rust.workspace`, de
  que **todo** driver depende: cada rodada paga um build do workspace. Lógica nova nasce com
  `#[cfg(test)]` ao lado e roda em `cargo test -p <crate>` (segundos). Ao Nix, uma vez — ele prova a
  travessia do WIT e o pixel, não serve para descobrir semântica.
- **O LAÇO TEM PERFIL PRÓPRIO, e quem instala não o vê** (`[profile.loop]`, na raiz do
  `Cargo.toml`). O `release` é afinado para o binário que roda por meses — `lto = "thin"` e
  `codegen-units = 1` —, e `crates/ronoctua-shell` tem 178 mil linhas num crate só: com uma unidade
  de codegen, a geração de código dele roda numa THREAD. Medido nesta máquina de 12 núcleos, **uma
  linha editada custava 148 s e passou a custar 19**; e um build FRIO no perfil do laço, com os 483
  crates de terceiros junto, são 139 s — menos que um incremental no perfil de produção. Usam o laço:
  o `.#dev`, o `rust.tests` e o `rust.clippy` (o hook e o `most`). Ficam em `release`: `.#vm`,
  `.#iso`, `.#install`, `.#bench` e os módulos. **É o lado RÁPIDO que tem nome**, para o esquecimento
  cair no lado seguro — um `iso` sem LTO não tem sintoma nenhum; cobra o
  `the-loop-profile-is-never-installed`. O preço é uma camada de dependências POR PERFIL (paga uma
  vez por `Cargo.lock`) e um segundo diretório no `CARGO_TARGET_DIR`.
- **O hot reload do `.#dev` estava MORTO, e o sintoma era o laço parecer lento.** A linha tinha
  `--bin ronoctua-rescue` sem `-p ronoctua-rescue`, e `--bin` nomeia alvo DENTRO dos pacotes que o
  `-p` escolheu: o cargo recusava o comando inteiro, o laço caía no `else` dizendo "a compilação
  falhou" e seguia com o rodapé velho. Toda edição de Rust, de toda sessão. O
  `dev-rebuilds-what-sits-beside` não pegou porque conferia o NOME; agora confere o PACOTE, contra
  os manifestos que declaram cada binário.
- **O PONTEIRO ENTRA NA FOTO, e ele é BRANCO.** O compositor compõe o próprio cursor na saída, então o
  `xwd -root` o pega e todo recorte que o contém carrega uma seta clara. `text.primary` é `#E0E0E0`,
  e 12% de fuzz cobre a distância: uma asserção de "há texto aqui" foi respondida pelo MOUSE, sobre
  uma mesa VAZIA, e o subteste passou. O conserto NÃO é tirar o ponteiro antes de medir — tentei, e
  mover o ponteiro entre duas cenas mudou qual porta recebia as teclas, quebrando o miniplayer três
  cenas adiante. O conserto é a asserção ser de FORA da tela (`pgrep` na porta que a cena usa), que
  é o que um cursor não tem como responder.
- **O rodapé da bateria de fotos NÃO herda o ambiente do módulo.** Os leitores externos
  (`RONOCTUA_EXIFTOOL` e os cinco irmãos) vêm do `nix/modules/media.nix`, que os põe na unidade
  `ronoctua-footer` e nas variáveis de sessão; o `shots.nix` sobe o rodapé com `systemd-run
  --unit=footer` e recebe NENHUMA das duas coisas. O sintoma é uma ficha de fotografia sem uma única
  tag: `Refusal::Missing` não desenha nada, e lê-se como arquivo sem metadado. Quem fotografa um
  caminho que chama programa externo acrescenta o `--setenv` ali.
- **Asserção de cor com o fuzz padrão é respondida por LETRA.** Doze por cento é distância
  euclidiana de ~53 em oito bits — `surface.atlas.land` (`#D7D2CA`) está a 28 de `text.primary`
  (`#E0E0E0`), então "há continentes aqui" foi respondido por um painel de PALAVRAS. O fuzz largo
  existe para o glifo, cujas bordas são mistura; região preenchida passa com fuzz pequeno, e é isso
  que a torna inrespondível por texto. O irmão dessa armadilha é o RECORTE: `surface.atlas.ocean` é
  violeta escuro e `surface.selection` também, então a pergunta tem de ser feita na coluna em que a
  figura está, nunca na tela toda.
- **Cena de foto ou abre a própria porta, ou declara de quem ela usa** (`needs`/`alone` em
  `nix/shots-scenes.nix`): o `document` e o `miniplayer` andam na janela que o `manager-window`
  abriu, e sozinhos fotografavam a mesa. Pedir uma cena traz o fecho transitivo dela, na ordem
  declarada; e o `shots-scenes-are-total` recusa cena que não chama `control(`/`notify(`/`systemd*`
  e não diz de que depende.
- **`nix run .#png` ANTES DA PALAVRA "PRONTO".** Verde não é prova, e desde que olhar custa 0,2 s não
  há desculpa. Teste que compara ESTADO não vê espaço em branco, sobreposição nem faixa de seleção.
  **E TODO buffer é fotografável — se um não for, a ferramenta é que está incompleta.** Eu relatei
  não ter conseguido fotografar a página do brain e o dono disse que isso está errado; estava. O
  `render-png` carregava o componente, dirigia as entradas e não tinha como entregar a RESPOSTA do
  hospedeiro, então o brain — cuja tela inteira é a `mind` — só sabia desenhar a tela de antes de
  chegar qualquer coisa. `--mind <arquivo.json>` come a JSON do PRÓPRIO daemon (uma linha do socket
  serve) e converte pelo caminho de produção. `--list` imprime a display list em vez de desenhar,
  que é a pergunta que uma foto não responde: uma faixa que ninguém vê porque há uma run em cima,
  uma linha desenhada depois do fim da janela. As outras três telas que dependem do hospedeiro —
  disco, fila, memória — continuam desenhando vazias, e a porta para elas é a mesma.
- **UM CHECK QUE LÊ FONTE ABRE COM O PRELÚDIO, e isso é cobrado** (`pyPrelude` em
  `tests/checks.nix`; cobra o `checks-do-not-pass-by-coincidence`). São 126 os que leem fonte e 125
  estão nele. Oito ferramentas, uma resposta por pergunta: `code_of` (tira comentários E literais,
  para quando a agulha é CÓDIGO), `uncommented` (tira só os comentários, para quando a agulha É um
  literal — um programa, um tipo MIME, uma notação de tecla), `hashless` (o `#` do Nix, do Python,
  do shell e do `.ftl`), `body_of` (o corpo de um item, por CHAVES e a partir do NOME — nunca de uma
  assinatura, que muda quando um parâmetro entra), `scope` (o bloco cujo CABEÇALHO casa, porque um
  nome só é único dentro de algo), `shut_at` (os argumentos de uma chamada, pelos PARÊNTESES dela) e
  `floor`/`must` (piso de contagem: captura vazia é RUÍDO, não um verde) e `without_tests` (o
  arquivo SEM o `#[cfg(test)]`, porque um teste USA o que testa: o check do ponteiro achou o
  `Input::Click` dentro do teste depois de ele ter sido apagado do produto).
  **E o check DIZ que tipo de pergunta faz**: `# FILE-WIDE:` quando a resposta é sobre o arquivo
  inteiro, `# PROSE:` quando o assunto SÃO as palavras (uma atribuição, uma referência a uma seção,
  a frase ao lado de uma constante), `# PARSED:` quando o arquivo é decodificado e não varrido (um
  PNG, um JSON, uma tabela que o compilador escreveu). Uma isenção que a regra deduz é uma que
  ninguém revisa.
- **SABOTAGEM É SÍNCRONA, sempre.** Quebrar, rodar o check, restaurar — os três na MESMA chamada, e
  nunca com o build em segundo plano: o arquivo fica sabotado enquanto ele roda, e a próxima coisa
  que compilar pega a versão quebrada. Aconteceu nesta sessão: um `cargo build` no meio pegou o
  `render-png` sem o `--mind`, a foto seguinte saiu com a tela vazia, e a causa estava dois passos
  atrás. Se o check é lento, ele é lento — espere.
- **ARQUIVO NOVO PRECISA DE `git add -N`** — o que o git não rastreia não entra na fonte do flake, e
  o erro aparece como "can't find bin" minutos depois.
- **O que eu subo, eu encerro** (daemon de teste, VM, processo solto), depois de conferir que não é
  trabalho de verdade — e sem tocar no que é do dono.

## Documentação — seis arquivos, em pt-BR (o resto é inglês — inv. 18)

- `AGENTS.md` — este, e **`CLAUDE.md` é UMA linha (`@AGENTS.md`) que o importa**. O conteúdo mora
  aqui desde 2026-09-08, a pedido do dono: o nome do arquivo que um assistente lê sozinho é do
  ASSISTENTE, e as regras não são — `AGENTS.md` é o nome que a maioria lê, e o ponteiro serve o
  Claude Code sem duplicar nada. Nada além do `@` vai no ponteiro (uma regra escrita lá só um leria);
  cobra o `docs-exist`, que também confere que o `@` aponta para um arquivo que existe — import que
  não resolve perde TODA regra de uma vez e é indistinguível de um projeto sem instruções.
  **Não tem teto de linhas** desde 2026-08-22, e por decisão do dono: é o único
  que chega sozinho ao contexto, então é aqui que regra de trabalho mora. O que o teto defendia
  continua valendo e agora é de quem escreve: **corte antes de acrescentar**. Sem log de progresso,
  changelog nem narrativa. `docs/ARCHITECTURE.md` — o porquê do que não se deve refazer.
- `docs/PENDING.md` — o que falta. **Ao concluir, remova de lá**: se mudar como o sistema funciona,
  escreva uma linha aqui ou no ARCHITECTURE; se não, só apague. `README.md` é placeholder, por decisão.
- `INSTALL.md` (disco) e `DEVELOPMENT.md` (ambiente): NÃO liste atalho — a leader os mostra. **Release
  publicada é em INGLÊS**; o `fonts-release` grava o corpo pronto.
