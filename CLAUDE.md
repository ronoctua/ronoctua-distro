# ronoctua-distro

Distro NixOS pessoal: sem barra, sem dock, sem menu iniciar — só o conteúdo e um rodapé de duas
linhas. Niri como compositor, camada de shell própria em Rust. O porquê de cada decisão está em
`docs/ARCHITECTURE.md`; o que ainda falta, em `docs/PENDING.md`.

## Invariantes — nunca violar

1. **Escopo é a pasta do projeto.** Nunca vasculhar atrás de código; ler caminho que o dono APONTOU, sim.
2. **Um repo, PÚBLICO**: `ronoctua/ronoctua-distro`. Nada pessoal — o e-mail do perfil NÃO é o de
   commit: confira `git config user.email` antes do primeiro. MIT + OFL.
3. **Sem fork e sem patch do Niri.** Só IPC e layer-shell.
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
14. **Nenhuma chrome além do rodapé.** Sem barra, relógio, taskbar, menu iniciar, dock, ícone de
    desktop ou popup flutuante. Tray, notificação e expansão são A MESMA superfície crescendo.
15. **Sem cards, sem sombra, sem elevação.**
16. `noctua` e `ronoctua-distro` definidos em **um único lugar** cada — `nix/modules/base.nix`;
    os **idiomas**, em `assets/i18n/languages.nix`, e nenhum outro sítio os nomeia.
17. **Sem Docker.**
18. **Todo NOME, COMENTÁRIO, MENSAGEM e COMMIT são inglês** — arquivo, comando, check, chave,
    `[[bin]]`, identificador (`let`, `fn` e parâmetro inclusive), `//`, `#`, docstring, `assert`,
    `bail!`, `.context`, saída de check e ajuda de linha de comando; cobra o `english-names`.
    pt-BR **só onde o valor existe em TODOS os idiomas declarados**: os 6 docs, o VALOR de chave
    em TODOS os `.ftl`, a tag de busca e o léxico. **Onde cabe um idioma só, ele é o INGLÊS** —
    descrição de unidade, `.desktop`, manifesto da extensão, alias do shell, `Job::trouble`.

Ao bater num invariante: **pare e reporte.** Não contorne, não peça exceção implícita.

## Comandos

```
nix develop           entra no ambiente (direnv é atalho opcional, e do HOST)
nix run .#dev         niri aninhado + rodapé; leader = Ctrl+Alt+Espaço, não Super
nix run .#png         um buffer vira PNG em 0,2 s, sem VM: a tela antes do "pronto"
nix run .#quick       DEZ escolhidos (tests/quick.nix), ~1 min; subconjunto, não categoria
nix run .#gate        TUDO; dimensiona pela RAM e o earlyoom. Só ele fecha o inv. 7; .#most é a bateria de todo dia, sem instalação
nix run .#vm  VM completa; .#iso o ISO; .#bench falha se estourar; .#diagnostics só lê
nix run .#install     instala (ensaia sem --execute);  ronoctua.profile: full ou minimal
```

Flake apps, nunca justfile nem Makefile. Passo que precise de instrução escrita devia ser automático.

## Como funciona

- **O rodapé é o desktop.** `wlr-layer-shell` embaixo: modeline e minibuffer COLADAS, expansão com teto e página abaixo, **faixa de notificação acima**, e nenhuma chrome (inv. 14). Ancorado embaixo, crescer para cima não move a modeline. Clique só faz o que tecla faz.
- **Todo buffer tem ESTADO, e o primeiro é `normal`** — recusado no load se não for. `Esc` sozinho volta UM nível (o buffer é que anda, e relata); **segurar `Esc` + setas gira**; `LEADER w s` escolhe; clique na modeline gira. O toque-contra-retenção é resolvido NO HOST do buffer (keyd pegaria o `Esc` do vim), que avisa o rodapé pelo socket de controle — e o rodapé PERGUNTA na troca de foco. Estrangeira não tem estado. Id inglês, tela por `state-<id>`; `normal` discreto, todo o resto num token SÓ.
- **A fronteira com o niri é só IPC** — `ronoctua-niri` é o único crate que fala `niri-ipc`. **E a
  leader vem de FORA**: em repouso o rodapé não captura teclado, senão nenhum buffer receberia tecla.
  Quem a vê é o niri, que roda `ronoctua-shell leader` no socket de controle
  (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`); só então a superfície expande e pede exclusividade.
  `ronoctua-shell command <id>` roda qualquer id — é por onde entram os QUATRO binds SEM leader.
- **`Ctrl+Alt+Del` é o RESGATE** (7 toques em 2 s ainda forçam): janela flutuante, ou VT 8 sem niri.
- **Todo comando é `kebab-case`, e a chave é `cmd-<id>`**; o registro recusa divergirem. As 141 ações
  do niri entram como `niri-<nome>`, GERADAS, alcançáveis por nome (`LEADER :`). **O teclado tem UM
  lugar** (`nix/keyboard-xkb.nix`); o xkb **não recusa** layout inexistente — cai no `us` mudo, e
  quem impede é `keyboard-layout-exists`.
- **Três navegadores, um léxico** (`i n`, `i u`, `i l`). Só o da Lucide personaliza; a prévia é o SVG COPIADO, e a marca sai da MESMA `affordance` que a tecla lê.
- **São TREZE vistas, e a precedência vive só em `Footer::view()`** — nenhuma é menu (inv. 14), e
  **TODA porta pergunta a ele**: o ramo de teclas, o `expansion()` e o `region_is_free` das onze
  portas. Isso não era verdade e três revisores acharam o mesmo defeito em três vistas: cada um
  lia o PRÓPRIO campo, as ordens divergiam, e clicar no sino com a energia aberta dava o teclado a
  uma e a tela à outra. Com todos perguntando, no máximo um ramo casa e a ORDEM em que eles estão
  escritos deixa de ser resposta; cobra o `the-footer-draws-what-the-keyboard-drives`. A pergunta do
  minibuffer é a única exceção, e é deliberada — pergunta é LINHA, não região. O localizador (`a`)
  varre o XDG na PARTIDA em thread; a rede (`s n n`) lê o CACHE do NetworkManager.
- **`LEADER .` (e `f f`) é o gerenciador, DUAS portas** — rodapé (teto de 12) e janela (`f w`), UMA
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
  layout. O chevron fica com a última célula da linha 0; a barra pega da linha 1 para baixo. **`..` é o primeiro renque** — nunca
  oculta, nunca marcável, e toda célula dela responde a saída. Permissão
  PERGUNTA nos DOIS caminhos. **Rótulo de `Choice` é TEXTO, não chave**; texto abre no `emacsclient`.
  **Prévia e infos nascem LIGADAS** (`C-v`/`C-i` ainda escondem) e por isso a região sobe ao teto
  com o gerenciador aberto — a ALTURA deixou de responder ao que foi digitado, e o `leader-vm` lê
  a TELA. Sem foco, painel não pede linha nenhuma. **A pasta DÁ NOTÍCIA** (`ronoctua_files::notice`,
  o nome `watch` já é das cópias): inotify na pasta aberta e na da barra, um watch e nunca recursivo,
  rajada absorvida na thread (a 1ª mudança sai na hora, o resto 1×/s — senão copiar N arquivos custa
  N `reread()`, que é O(N²)). Responde com `reread()` SECO: `stale()` é o mtime da PASTA e não vê
  arquivo reescrito no lugar nem o balde de 2 s do vfat. O rodapé FICA com o `stat` de 1 s — é o piso
  para NFS, que o inotify não vê —; a janela não ganhou relógio (em repouso não acorda NENHUMA vez,
  que é o custo de ter várias abertas), só se o observador não subir. Fechar DESCANSA o watch, e os
  dois lugares que largam a vista chamam — o segundo é perder o FOCO. A ficha ROLA (`C-.`, e a barra diz que cortou);
  permissão, dono e grupo nascem OCULTOS na listagem (`C-d`), e no painel de infos os campos são
  os MESMOS da linha — mesma `Hit`, mesma máquina. **A prévia mostra TEXTO** quando o arquivo é
  texto (`is_a(mime, "text/plain")`, 277 tipos pelo DAG do freedesktop), com realce e rolagem: setas
  quando ela tem o foco, `C-,` pagina e dá a volta, `C-[`/`C-]` movem a linha para os lados, e a
  barra aparece sozinha porque a seção passou a ter `total`. **Barra nenhuma
  desenha por cima do conteúdo** — a que rola vertical tira uma CÉLULA e a que rola
  horizontal tira uma LINHA, e só quando existem; a listagem e a lateral já
  recuavam por `EDGE_RIGHT`. **A linha longa tem barra de baixo** (`Scroll::bar_across`,
  o mesmo `thumb` com os eixos trocados): teclado sem mouse é meia vista, e o
  arrasto passou a ter EIXO. Tetos: 1 MiB, 2 000 linhas, 512 bytes
  por linha — o último porque `.js` minificado é UMA linha de 2 MB e o `elide_end` a percorreria por
  cluster a cada frame. **Arquivo sem imagem E sem texto diz o que a pasta diz** (`sem prévia`); a
  frase sobre imagem morreu. **A prévia é um MINIPLAYER** (2026-08-30, a pedido do dono, que
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
  (`ronoctua_render::MOST_SCALE`), que o `render-png --scale` lê também. Medido no niri sob Xvfb com a
  saída em `scale 2`, na coluna da janela de energia: pixels acesos 20,19% → 11,35%, pixels de tinta
  cheia 1,27% → 2,63%, halo 93,7% → 76,9% dos acesos. Cobra o `every-window-knows-its-scale`, que
  também LÊ o `hidpi.rs` — a varredura pulava todo arquivo sem `CompositorHandler`, e era lá que o
  defeito estava.
- **O teclado agora REPETE, e não repetia em porta nenhuma** — as duas criavam com `get_keyboard`, e o
  SCTK só repete com `get_keyboard_with_repeat`; o `repeat_key` das duas era código morto e segurar
  `j` nunca andou na lista. A repetição chega pelo CALLBACK, não pelo método, então cada porta tem um
  `typed` por onde passam as duas entradas. O rodapé cria o `calloop` ANTES do roundtrip: é nele que o
  assento anuncia o teclado e o `LoopHandle` já tem de existir. Ritmo e atraso são do COMPOSITOR.
- **A barra lateral é uma SEÇÃO** (`files::Section`), com extensão, deslocamento e conteúdo próprios — como a listagem, a prévia e os fatos. `sidebar_columns` a RETIRA quando não cabe (`space.files.sidebarFrom`); a marca vai em `layout.from`, nunca em 0; e `header` usa o MESMO `split` das linhas. Padrões por CHAVE (`nix/files-sidebar.nix` sobre `nix/user-dirs.nix` — **uma fonte para as dez pastas**, e de lá sai também o `screenshot-path` do niri, que sem isso escrevia em `~/Pictures`). O ajuste do dono vence, em `$XDG_CONFIG_HOME`: padrão da distro OCULTA (volta), o dele REMOVE — e as frases diferem. `C-l` esconde, `C-e` desmonta, `C-u` tira, `s` do menu e `C-s` do localizador fixam: **LETRAS, porque Ctrl DIREITO com seta é paginar e a janela não o rastreia**. **Atravessar é o anel do `Tab`, e só ele** — `C-o` era um segundo caminho e saiu a pedido do dono.
- **Só o `ronoctua-disks` fala udisks2, e ele NÃO escreve regra polkit** — a do upstream já separa removível (`allow_active: yes`) de sistema (`auth_admin_keep`) pelo `HintSystem`, e uma nossa só poderia ALARGAR. **Não há agente polkit na sessão** (só `pkttyagent`, no resgate): toda chamada leva `auth.no_user_interaction`. E a razão é do DONO, não do desenho: ele nem sempre vê o pedido de digital, ou está longe do laptop — pedido não visto vira recusa no relatório, sobre algo que ninguém viu. **Nada que o sistema faça sozinho pode PERGUNTAR**, e volume de SISTEMA não oferece montar — manda ao terminal root. `HintIgnore` vira `quiet` e anda no `C-h` dos dotfiles. O `override` tira só o `xfsprogs` (538,3 → 343,2 MiB; 258 eram Python do `xfs_scrub`, e montar XFS é do KERNEL). Sinal do `ObjectManager` numa thread, mandando o `Rack` INTEIRO — o relógio do rodapé só bate com a vista aberta, e a janela não tem relógio nenhum.
- **`LEADER s f` é formatar, e são TRÊS perguntas** (`buffers/format`, `capability::disks`): qual disco, para que serve, tem certeza. Linguagem de gente — o vfat diz *"abre em qualquer computador"*, não FAT32, e **o XFS aparece DESABILITADO dizendo que falta o programa**, alcançável e recusando. **Toda linha tem DOIS registros**: o rótulo, e uma linha discreta SOB O CURSOR com o nome real e os limites (na lista, o tamanho em bytes; no confirmar, o gesto inteiro). Nome técnico é CHAVE, nunca literal no componente. Duas recusas que o hospedeiro não enxerga (de fora, recusa e componente morto são o mesmo "nenhum plano"), e por isso são do `format-driven`: disco que o sistema precisa é DESENHADO e o `Enter` nele não anda; e o primeiro `Enter` do confirmar VOLTA, porque `não` é a primeira linha. **Sem estado `working`** — o `SPC w s` o ofereceria e mentiria; a espera é na lista. O alvo é id que o buffer RECEBEU, reconferido contra o `Rack` mandado **e contra `needed()` de novo**. Disco inteiro é `dos` até 2 TiB (`gpt` acima) mais UMA partição, com `update-partition-type` — sem ele o vfat sai tipado `0x83` e o Windows não abre o pendrive, sem nada dizendo por quê.
- **`LEADER t` é a fila, e ela é de uma UNIDADE** (`ronoctua-transfers`): três processos olham a mesma
  lista e a cópia sobrevive ao rodapé reiniciar. Socket JSON por linha, um por USUÁRIO. `t t` no
  rodapé, `t w` no buffer wasip2 — e o cursor é o **id**, que a lista reordena. `t a`/`t v`/`t u`/`t l`
  pegam o COPIADO. Desistir APAGA o pedaço. `c c` clipboard, `c s` captura.
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
  não se toca: `gtk-update` viraria `Gtk-update`.
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
- **`LEADER s n` é rede; `LEADER s s`, Syncthing** (os dois moraram em `r`, que morreu — rede é do SISTEMA, ao lado de memória e disco). Só o `ronoctua-network` fala NetworkManager; `overrideFolders` FICA `false` (`true` apaga as pastas a cada rebuild). A senha do Wi-Fi não fica em arquivo: `psk-flags` mais o agente entregando do TPM2 (**sem PCR** — com PCR, atualizar o kernel a quebra).
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
  Nenhum dos dois sinaliza: uma thread pergunta a cada 10 s. **Condição de regra polkit com `&&` e
  `||` LEVA PARÊNTESES** — o `&&` liga mais forte, então `id && user && unit==a || unit==b` lê como
  `(id && user && unit==a) || unit==b`: tudo depois da primeira alternativa liberado para QUALQUER
  ação e QUALQUER sujeito. O `rescue.nix` foi assim por um commit, e nada via — o
  `polkit-rules-are-the-ones-we-know` lia id de ação e resultado, e os dois estavam na lista
  auditada; agora ele recusa a condição. **Tor não sobe no boot** e a ÚNICA
  regra polkit nova do sistema é `start`/`stop` de `tor.service` (`tor-polkit-is-narrow`).
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
  definições. **Linha que responde ao `RET` tem TOM próprio** (`text.label-acts` no rótulo, `text.primary` no
  valor); a informativa usa os tons de fato. A marca desenhada na frente foi RECUSADA pelo dono e a
  diferença é só cor — um comentário dizia que havia um `icon.row-acts`, e ele nunca existiu.
  **Item de LISTA inverte os pesos** do fato: o nome da coisa é a metade forte. **Ar antes de todo
  título** (`chrome::AIR`, um `heading` sem campos), e o título recua para fora das linhas dele.
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
- **Cor NENHUMA é escrita à mão: `colors.nix` é RECEITA** (`text.primary = "neutral.text-strong"`) e o valor sai de `nix/scripts/gen-theme.py`, por tema, das SEMENTES em `assets/themes/<id>.nix` (matiz, croma relativo, shift, torção) — a lista é `assets/themes/themes.nix`, o primeiro é o padrão, e nenhum outro sítio nomeia um tema (inv. 16). A escada é a paleta do owl feita regra; toda relação que era prosa é GATE (contraste APCA, distância ΔE_ok, matiz ≥ 14°, faixa protan 158–172°), e o owl gerado é comparado chave a chave com `owl.reference.nix` (≤ 0,03, ou `deviations` com motivo). `nix build .#themes` deixa o `report.md`. **Mexer no gerador reconstrói o workspace** (o caminho do `tokens.rs` muda), então afine ANTES de compilar Rust.
- **Afinar uma cor é mexer na SEMENTE, e o laço custa 2 s**: `nix build .#themes --no-link
  --print-out-paths` e leia o `report.md` do caminho impresso — L, C, H e o hex de cada primitiva,
  os valores de cada token, e a linha 5 com o placar dos portões (`Fail 0`). **Direto do shell,
  nunca por `nix develop`**: medido, 2 s contra 41 s, porque o devshell arrasta junto o `tokens.rs`,
  o WIT e os seis buffers. Os botões são OKLCH, e cada um responde a uma pergunta: `hue` (graus) diz
  OUTRO TIPO de coisa, `text-chroma` diz quanto a matiz aparece (`0.0` é cinza de verdade, e a matiz
  declarada nem é desenhada), `text-shift` sobe ou desce a escada inteira em Lc — luminância é OUTRA
  IMPORTÂNCIA. Escolher o DEGRAU (`text-muted` 23, `text` 48, `text-loud` 79, `text-strong` 87) é o
  passo grosso; o `shift` é o fino.
- **Papel novo = família em TODOS os temas** (`assets/themes/*.nix`, menos a `.reference`): sem isso
  o gerador recusa com `no family '<nome>' in the seed`. E papel novo NÃO entra na referência
  congelada — ela mede só as 52 chaves do dia em que foi capturada.
- **Hex existe em três sítios, e nenhum é de autoria**: o `tokens.rs` GERADO (quem consome é a GPU,
  que toma RGBA8), o `report.md` e o fixture congelado. Hex numa maquete do scratchpad ou numa frase
  de relatório é MEDIÇÃO — pixel amostrado para provar o que está na tela —, nunca escolha. Quem
  escolhe diz matiz, croma e contraste, e o solver resolve.
- **`LEADER s t` escolhe o tema, e mover o cursor PRÉ-VISUALIZA só no rodapé**; `RET` grava `$XDG_CONFIG_HOME/ronoctua/theme.json`, `Esc` volta ao gravado. A lista mostra SÓ o id. **A segunda coluna (`←/→`) é o FUNDO do compositor — preto ou `surface.backdrop`, o degrau neutro ABAIXO do chrome no matiz da semente `backdrop`** —, e a prévia dele é o próprio niri: o fragmento é escrito ao mover o cursor e reescrito no `Esc`; `RET` grava o PAR. **O arquivo É o broadcast**: as cinco portas vigiam `theme.json` por inotify (`Notice::on_file`) e repintam; `RONOCTUA_THEME` (opção `ronoctua.theme`) é o piso, o primeiro tema é o chão. O rodapé COPIA os fragmentos do tema (`$RONOCTUA_THEMES/<id>/`): `niri-theme.kdl` (borda E fundo — `window-rule` mais `layout { background-color }`, que o niri FUNDE ao incluir; dois `layout` no MESMO arquivo ele recusa; o config o inclui `optional=true` no FIM e recarrega) e `ghostty-theme.conf` (`config-file = ?…`, processo novo — e a entrada `.desktop` é REESCRITA sem o `--gtk-single-instance=true` do upstream: flag vence arquivo). **O Ghostty só lê `~/.config/ghostty/config`** — `/etc/xdg` ele NÃO lê —, e é lá que o home-manager põe o config do módulo. GTK/Qt vestem o MODO escuro (Qt só no perfil `full`: custa 591 MiB); a extensão, o padrão do build.
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
  posiciona o niri (inv. 4); não conhecem Wayland, cor nem idioma (inv. 11). Estado vive na
  `Instance`. Cinco primitivas: texto, régua, grade, seleção e **`svg`** (TRAVA `vector`), e **campo
  de record cruza por POSIÇÃO** e só depois pelo nome.
- **`LEADER b b` é o cérebro** (`ronoctua-brain`; `b a`/`b p` são as duas respostas da faixa — o `b` era o grupo BUFFER, e buffer é janela (inv. 4), então os dois grupos viraram `w`: `w w` troca, `w n` próximo, `w d` fecha, `w s` estado, `w o`/`w O` terminal), o daemon que decide QUANDO a máquina mexe em si
  mesma — molde do `ronoctua-transfers`: unidade de usuário, `session.slice`, socket JSON por linha,
  estado inteiro e nunca delta. A porta para features novas é o `trait Task`, e ela é uma promessa de
  CINCO lugares: `src/tasks/<id>.rs`, uma linha em `tasks()`, duas chaves nos dois idiomas, uma linha
  na home do buffer, um padrão em `prefs`. Quatro dos métodos são PUROS — é o que faz um ano de
  agenda caber em milissegundos de teste. Ele **não** é uma segunda fronteira com o `/proc` (é o
  `ronoctua-memory`, que ganhou `cpu_pressure` e `on_battery`), não segura teclado e não roda nada
  privilegiado: pede ao systemd. Capacidade `brain` PRÓPRIA — `system` é "quanta RAM, quem come";
  esta termina com a máquina reconstruindo o próprio sistema operacional. `tune` é efeito e não
  demanda, porque demanda é verbo mais ÍNDICE e preferência carrega VALOR.
- **A atualização é declarada, e o brain só escolhe a hora** — `assets/updates.nix` gera a tabela
  Rust, as chaves que os dois idiomas devem, os dois roteiros e a recusa. O brain LÊ essa tabela
  (`build.rs`); ele já declarou os passos por conta e os ids saíram **disjuntos** dos da sequência.
  A colisão com o invariante 6 é estrutural: passo cujo programa a flake não põe **não pode nascer
  ligado**, não roda como root e tem de declarar a própria pré-condição; e passo nenhum instala.
  **Nada disso PERGUNTA** — regra polkit devolvendo `YES`, sem `sudo`, sem `pkexec`, sem
  `systemd-run`; e há check. **A faixa do brain fica COLADA abaixo do minibuffer** (não na última
  linha da superfície, que se moveria com a expansão): ícone, frase, contagem no relógio de 1 s que
  o rodapé já bate, e dois botões que têm tecla.
- **`C-v` e `C-i` são prévia e fatos no gerenciador** — colunas à direita a partir de
  `space.files.panelsFrom`, faixa embaixo abaixo disso, e **as duas viram juntas**. Só o
  `ronoctua-inspect` decodifica (crate PRÓPRIO: o `ronoctua-files` constrói o binário que roda como
  ROOT). Cinco programas externos, e só o `ffmpeg` faz pixel — medido: o `magick` estourou num DNG
  de 17 MB e o ffmpeg fez o mesmo em 0,21 s. RAW é a prévia EMBUTIDA pelo `exiftool`, porque o
  ffmpeg decodifica os planos lineares e sai quase preto. Todo caminho externo devolve imagem no
  **stdout** e quem decodifica é o `image`: `Rgba` nasce num lugar só. **Recusa é VALOR, não ausência** — painel em branco e painel recusado são
  iguais na tela, e só um tem frase. GPS não é mostrado, e a linha DIZ que o arquivo o carrega.
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
- **`LEADER s k` são os TECLADOS** (`ronoctua-keyboards` + `buffers/keyboard`), e é o único ajuste
  POR DISPOSITIVO que existe: o niri tem **um** `input { keyboard { … } }` para todos e não expõe
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
- **`LEADER s p` é energia** (`ronoctua-power`, a fronteira com UPower, power-profiles-daemon e logind — molde do `ronoctua-disks`, três threads e o snapshot inteiro): uma `View` (`power.rs`), duas portas (`s p p`/`s p w`), oito seções como linhas. **Modeline em repouso é muda**: bateria só abaixo de 99% e fora de `FullyCharged` (limite de carga a 80% diz `FullyCharged` para sempre), `%` só a ≤30, `state.error` a ≤15 (regra literal, carregando ou não); perfil só quando NÃO é o topo que o daemon oferece — **o padrão da distro é `performance`** (`ronoctua.power.profile`, piso; `power.json` vence; aplicado UMA vez por sessão, nunca sobre um hold e **nunca quando ninguém nomeou** — o `nix run .#dev` não mexe no perfil do dono). Limites de carga escrevem no sysfs por regra udev (grupo `users`); as travas de sono/tampa são fds na thread de ordens do RODAPÉ (a janela encaminha `power-keep-*`); `q s/h/r/p` abrem a vista ARMADA e `RET` confirma. **Nenhuma regra polkit nossa** (logind e PPD já dão `yes`; há check). Alarmes `battery-low/critical/charged` por TRANSIÇÃO (`alarms`), pelo catálogo.
- **`LEADER s d` são as telas** (`ronoctua-display`, a fronteira com o niri por IPC e com o kernel
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
  número que o niri escolheu no lugar de "escolha você"). **Duas portas escrevem o MESMO arquivo, e por isso `Keep` é MESCLA e não escrita** — a ordem
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
  falha muda que o `config_failed` do niri existe para impedir, um arquivo ao lado, e sai pela mesma
  porta — uma frase na lista. Esquecer TIRA a entrada do arquivo (`Preferences::set`, não
  `insert`): mapa que só cresce vira arquivo listando todo monitor que já passou pela máquina, cada
  um dizendo nada. **O bloco mais específico vem primeiro no
  fragmento**: o niri casa saída por conector OU por descrição e fica com o PRIMEIRO, e o nosso
  arquivo tem as duas espécies de chave — um bloco velho de `DP-1` dizendo `off` apagava o monitor
  novo que entrou naquela porta, sem nada na vista alcançando ele. **O arquivo é o que sobrevive ao reboot e
  o IPC é o que faz o compositor concordar com ele** — recarregar sozinho NÃO desfaz um transitório,
  porque o niri preserva a mudança quando o arquivo relido não mudou. **E fechar a porta com uma
  prévia de pé a devolve ANTES de sair** (`Handle::finish`, que manda e ESPERA a thread): sair sem
  isso deixava a mudança valendo até o niri reiniciar, e uma porta nova não ajuda porque o
  `previewed` é da thread que aplicou. Brilho interno é udev no
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
  nenhum cache tem** (shell, niri, ghostty-vt, buffers, fontes): o alvo baixa, não compila.

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
  por outra porta — meça com `nix why-depends`, não confie no `mkIf`. **`option_env!` com caminho de
  store é referência, e quem COMPILA a paga**: ícones e os buffers vêm do AMBIENTE, senão editar
  buffer reconstrói o workspace; ela sobe uma derivação (`rust.shell`) e nunca some.
- **`Color` é PAPEL, não valor**: `u16` que resolve em `themes::TABLE[tema][papel]` ao desenhar; `==` compara papel; `to_rgba8()` não é `const`. Tudo que uma CRIANÇA pinta com o valor (waveform do ffmpeg, `currentColor` de SVG) leva o tema na CHAVE do cache, senão a textura velha fica no GPU. Só UM teste chama `theme::set` (no tokens) — os testes correm em paralelo. Consumidor Nix lê o STORE (`${themed}/…`), nunca `colors.nix`: lá não há valor.
- **Nunca peça `Family::Monospace`**: fallback, **4 447 ms** contra 0,76 ms. **Buffer não tem parser
  de JSON**: campo nenhum leva caractere cru. **Token vira WIT**: segmento começa por LETRA.
- **Dentro de `''…''` valem DOIS idiomas**: `#`/`//` é do script embutido, mas `${...}` é do Nix e vive até em linha comentada; **`` ` `` é substituição de comando ATÉ no comentário**; e a fence FECHA num `''` qualquer — inclusive dentro de comentário (`'''`, `''$`, `''\` escapam).
- **Chave de texto tem SETE derivados**, só um é o `.ftl`; **pergunta do instalador tem 13** — os dois no ARCHITECTURE. **`to_linear()` desfaz a curva sRGB**; **célula = 0,600 × fonte**; **caixa 1,25 em** contra entrelinha 1,45. `tiny-skia` entrega RGBA PRÉ-MULTIPLICADO. **Uma run ocupa `heights.of(escala)` CÉLULAS, e a escala alarga tanto quanto sobe**: `Large` são DUAS, e a régua em `line+1` virava sublinhado. Título é do tamanho do CORPO — hierarquia é cor e ar, nunca tamanho nem régua; os dois `DisplayList::nothing_*` cobram isso em TODO buffer, em TODO estado.
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
- **`rustc` morto por `signal: 15` SEM diagnóstico é o `earlyoom` do HOST** (`-m 10,5`, `rustc` no `--prefer`), e `--max-jobs` não salva: o que estoura é jobs × cores. Por isso o gate mede e limita o produto. **`cannot download … from any mirror` NÃO é arquivo removido**: o `fetchurl` repete SEM retomar, e o android-studio (1,37 GB, unfree, um espelho, fora de todo cache) reseta aos ~150 MB — quem baixa é o `nix/prefetch.nix`, que o gate roda ANTES de compilar. **`pkill`/`pgrep` casa o PRÓPRIO comando**, e **`git checkout <arquivo>` apaga o não-commitado** (o store guarda cópia: `ls -dt /nix/store/*-source`).
- **Modificador segurado mata o keymap.** No aninhado a leader É `Ctrl+Alt+Espaço`: DESCONTE o que já estava em baixo ao ganhar o foco E ao começar a sequência. Caps Lock idem — `not bound` mudo. **E no aninhado ele fica preso PARA SEMPRE**: modificador que desce aqui e sobe lá fora — o host comeu um acorde e levou o foco junto — nunca é relatado, porque o `winit` só diz `Focused(false)`, o smithay DESCARTA evento sintético e nunca encaminha `ModifiersChanged`, e o `WinitEvent::Focus(_)` do niri é `()`. Apertar de novo NÃO limpa (o libxkbcommon conta referência: 1 → 2 → 1) e recarregar o keymap também não (`update_xkb_state` REPETE as teclas que o smithay julga em baixo). Como o niri casa bind por IGUALDADE, um bit sobrando mata TODOS os binds da sessão — leader inclusive — e o único conserto é reiniciar o niri. Por isso o aninhado tem keymap PRÓPRIO, SEM Super (`nix/keyboard-nested.nix`; cobra o `nested-keyboard-has-no-super`): Super é o único modificador que o aninhado dispensa, e é o que todo acorde do host carrega. Em produção Super FICA — lá ele É a leader. **Um acorde sobrevive exatamente aos modificadores que já estão DENTRO dele** — por isso a leader do aninhado é `Ctrl+Alt` (o que a família `Super+Ctrl+…` do host prende) e ganhou uma segunda forma com Shift; trocar de modificador só muda a exposição, acrescentar é que fecha. **Os ACORDES não têm essa saída**: `Ctrl+<tecla>` já é tecla de acorde (`LEADER+Ctrl+←` MOVE a coluna), então crescer o prefixo para `Ctrl+Alt` colide com 41 binds e o niri recusa o arquivo inteiro — e com o Ctrl preso alguns acordes não morrem, fazem a AÇÃO ERRADA. Com resíduo que não seja Shift, a saída é reiniciar o `.#dev`. **Não tente consertar do lado do cliente**: o `zwp_virtual_keyboard` do smithay escreve nos CLIENTES e nunca no estado do assento, e o IPC do niri não expõe modificador nenhum — de dentro, preso e segurado de verdade são indistinguíveis.
- **"Nenhum atalho funciona" tem três causas com a MESMA tela** — tecla que não chega, modificador preso, foco não concedido: `nix run .#diagnostics` ANTES de supor. No Xvfb ninguém tem foco (`xdotool windowfocus`), e **`xdotool key` SOLTA a tecla**: segurar exige `keydown`.
- **`LD_LIBRARY_PATH` vence o `DT_RUNPATH`**: pôr `/usr/lib64` lá para achar UMA lib traz a `libc` do host junto, e ela só combina com o `ld.so` da MESMA build (`undefined symbol ... GLIBC_PRIVATE`). O driver vem do Nix (`mesa`); do host, só para NVIDIA. **ICD Vulkan ausente custa 650 ms mudo** — filtre por vendor. **`memory_init_cow`** é obrigatório (p99 95× pior); **`Queue::present`** precisa do frame, que o `Drop` DESCARTA sem erro.
- **O backend DRM do niri rejeita EGL por software**: numa VM sem GPU sobe com **zero output**, e layer-shell sem output nunca é criada. Use **winit** com `Xvfb` + `LD_LIBRARY_PATH` das libs X. **`RUST_LOG` nosso filtra o niri**, que sobe MUDO — ache o display por `niri.<display>.<pid>.sock`.
- **Quem tem o codepoint primeiro GANHA** — a face pedida por nome vence o fallback. Custou 634 da PUA (Lucide → `U+F2000`) e 75 emoji da Iosevka (`Emoji_Presentation`); `FE0F` vira dois glifos.
- **As fontes são asset de Release, nunca git** — 4,1 GB; o input é **`file+https://`**, senão o Nix infere tarball e descompacta no store. **O console não consome TTF**; **`/etc/nixos` e `/etc/fonts/conf.d` são read-only** (`localConf`). Genérico é `sans-serif`; ISO é `image.baseName`. Glifo **não** leva `nf-`.
- **`systemd.oomd.enable` JÁ é `true` e não vigia nada** — arma é `ManagedOOMMemoryPressure=kill` numa slice. `session.slice` é IRMÃ da `app.slice`: vigiar só a segunda tira o desktop do conjunto por construção, e `enableRootSlice`/`enableUserSlices` desfazem isso. **`memory.min` é limitado por TODOS os ancestrais** — quatro drop-ins ou vale zero. Porcentagem o systemd resolve na MÁQUINA. **`OOMScoreAdjust` em unidade de usuário é ignorado em silêncio**: suba o app com `choom`. **`grep -r` não segue symlink**, e todo `/etc` do NixOS é symlink: `-R`.
- **No btrfs, `nodatacow`/`compress` são do SISTEMA DE ARQUIVOS**: a 1ª montagem vence em silêncio; hibernação depende do NOCOW (`lsattr` → `C`). **PCR 15 exige UKI medida.**
- **`#[repr(C)]` promete ARRANJAR, não conferir** — layout errado de FFI não dá diagnóstico: confira com `sizeof` em C. Ghostty: `Point` tem união (24B), `GridRef` é *sized struct*, `Cell` é `uint64_t`. No libtorrent isso virou check (`torrent-layout-matches-cxx`), e o `-fvisibility=hidden` esconde o `extern "C"` TAMBÉM — precisa de `__attribute__((visibility("default")))`. **Componente wasip2 não pode ser membro do workspace do host** (`rust-lld` recusa o version script mudo); **`char` é embutido do WIT**. **API gráfica mudou muito — leia a fonte.**
  **nixpkgs 26.05:** `noto-fonts-emoji` → `noto-fonts-color-emoji`; `nixfmt-rfc-style` → `nixfmt`; `xorg.libX11` → `libx11`; `pkgs.system` → `pkgs.stdenv.hostPlatform.system`. **Atributo dinâmico em duas sentenças** é erro de avaliação: um attrset só.
- **`take` é TETO, nunca contagem — e o `usize::MAX` é sentinela.** `View::rows` pede "todas"; enquanto
  `take` só limitava um iterador finito isso era grátis, mas no dia em que a função passou a
  MATERIALIZAR linhas virou pedido de 2⁶⁴ `Row` vazias. Vinte e sete testes crescendo esse `Vec` ao
  mesmo tempo comeram os 14,6 GiB em segundos, e **o `systemd-oomd` mata o cgroup INTEIRO da janela**
  (`app.slice`, desde a migração do Omarchy 4): o agente morre junto e não sobra rastro na sessão —
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
- **Spawn em unidade do systemd: use CAMINHO, nunca nome** — o `PATH` é mínimo e a falha aparece longe da causa. O caminho do store vem por `option_env!`, posto no `nix/rust.nix`. E **`min_cols` é piso de PREFERÊNCIA**; o que não cede é "nada vaza da tela". `Shrink`≠`removable`. **Upstream do Niri: `niri-wm/niri`**, por calendário — pinar tag, nunca `main`. Blur da 26.04 não.

## Como trabalhar — o dono cobrou o TEMPO duas vezes, e a causa era sempre a mesma

Estas regras existem porque tarefas de poucas linhas levaram mais de uma hora. Elas não são sobre
pressa: são sobre não gastar relógio no que não prova nada. **Valem para qualquer agente**, e por
isso moram aqui, no único arquivo que chega sozinho ao contexto.

- **ENTREGUE A CORREÇÃO E A TELA; O CHECK É OFERTA.** Faça o que foi pedido, mostre o PNG, e **pare**
  — dizendo qual check você acrescentaria, o que ele pega e quanto custa. Quem decide o escopo é o
  dono. Não vale para o que o pedido claramente inclui, nem para deixar sem conserto um check que
  você ACABOU de ver falhar: aí é reportar, não seguir em frente.
- **SÓ OS CHECKS DO QUE MUDOU** (drivers do que você tocou, `nix fmt -- --check`, `english-names`:
  ~3 min). O `most` são **16 minutos** e fica para antes do commit; o `gate` é do dono. E **congele a
  árvore antes de disparar bateria** — o Nix a fotografa na avaliação, e editar um comentário no meio
  obriga a refazer tudo. Diga sempre no relatório qual recorte rodou e qual não.
- **ITERE NO `cargo`, NUNCA NO NIX.** Tocar `crates/ronoctua-protocol` invalida `rust.workspace`, de
  que **todo** driver depende: cada rodada paga um build release do workspace. Lógica nova nasce com
  `#[cfg(test)]` ao lado e roda em `cargo test -p <crate>` (segundos). Ao Nix, uma vez — ele prova a
  travessia do WIT e o pixel, não serve para descobrir semântica.
- **`nix run .#png` ANTES DA PALAVRA "PRONTO".** Verde não é prova, e desde que olhar custa 0,2 s não
  há desculpa. Teste que compara ESTADO não vê espaço em branco, sobreposição nem faixa de seleção.
- **ARQUIVO NOVO PRECISA DE `git add -N`** — o que o git não rastreia não entra na fonte do flake, e
  o erro aparece como "can't find bin" minutos depois.
- **O que eu subo, eu encerro** (daemon de teste, VM, processo solto), depois de conferir que não é
  trabalho de verdade — e sem tocar no que é do dono.

## Documentação — seis arquivos, em pt-BR (o resto é inglês — inv. 18)

- `CLAUDE.md` — este. **Não tem teto de linhas** desde 2026-08-22, e por decisão do dono: é o único
  que chega sozinho ao contexto, então é aqui que regra de trabalho mora. O que o teto defendia
  continua valendo e agora é de quem escreve: **corte antes de acrescentar**. Sem log de progresso,
  changelog nem narrativa. `docs/ARCHITECTURE.md` — o porquê do que não se deve refazer.
- `docs/PENDING.md` — o que falta. **Ao concluir, remova de lá**: se mudar como o sistema funciona,
  escreva uma linha aqui ou no ARCHITECTURE; se não, só apague. `README.md` é placeholder, por decisão.
- `INSTALL.md` (disco) e `DEVELOPMENT.md` (ambiente): NÃO liste atalho — a leader os mostra. **Release
  publicada é em INGLÊS**; o `fonts-release` grava o corpo pronto.
