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
    estrangeiros; nenhum componente do design system o consulta.
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
- **Todo buffer tem ESTADO, e o primeiro é `normal`** — recusado no load se não for. `Esc` sozinho volta UM nível (o buffer é que anda, e relata); **segurar `Esc` + setas gira**; `LEADER b s` escolhe; clique na modeline gira. O toque-contra-retenção é resolvido NO HOST do buffer (keyd pegaria o `Esc` do vim), que avisa o rodapé pelo socket de controle — e o rodapé PERGUNTA na troca de foco. Estrangeira não tem estado. Id inglês, tela por `state-<id>`; `normal` discreto, todo o resto num token SÓ.
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
- **São NOVE vistas, e a precedência vive só em `Footer::view()`** — nenhuma é menu (inv. 14). O localizador (`a`) varre o XDG na PARTIDA em thread; a rede (`r r`) lê o CACHE do NetworkManager.
- **`LEADER .` (e `f f`) é o gerenciador, DUAS portas** — rodapé (teto de 12) e janela (`f w`), UMA
  `files::View`; só o `ronoctua-files` toca o disco. **Letra é QUERY** e vai no FIM do caminho;
  comandos da vista sob `C-` — **nunca `M-`, que é a família dos acordes do compositor**;
  `Backspace` vazio SOBE; o direito só FOCA, e a linha na TELA não é a da listagem.
  **←/→ andam entre as SEÇÕES** (`TAB`/`S-TAB` seguem ligados e fora da lista) e **`S-<setas>` mudam
  de sentido com a seção em foco** (`strong_meaning`; sem resposta própria, a listagem responde) —
  na dica elas levam a frase resolvida e cor própria. **A linha de dicas é GERADA de `OF_THE_VIEW`**,
  uma notação por ação e o `·` do código (`mark.separator`): as 40 frases já existiam nos dois
  `.ftl` e nenhuma tela as lia. **Prévia e fatos têm cursor e TÍTULO** (aceso com o teclado; custa
  uma linha), e a listagem guarda faixa quieta ao perder o foco. **`..` é o primeiro renque** — nunca
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
- **O teclado agora REPETE, e não repetia em porta nenhuma** — as duas criavam com `get_keyboard`, e o
  SCTK só repete com `get_keyboard_with_repeat`; o `repeat_key` das duas era código morto e segurar
  `j` nunca andou na lista. A repetição chega pelo CALLBACK, não pelo método, então cada porta tem um
  `typed` por onde passam as duas entradas. O rodapé cria o `calloop` ANTES do roundtrip: é nele que o
  assento anuncia o teclado e o `LoopHandle` já tem de existir. Ritmo e atraso são do COMPOSITOR.
- **A barra lateral é uma SEÇÃO** (`files::Section`), com extensão, deslocamento e conteúdo próprios — como a listagem, a prévia e os fatos. `sidebar_columns` a RETIRA quando não cabe (`space.files.sidebarFrom`); a marca vai em `layout.from`, nunca em 0; e `header` usa o MESMO `split` das linhas. Padrões por CHAVE (`nix/files-sidebar.nix` sobre `nix/user-dirs.nix` — **uma fonte para as dez pastas**, e de lá sai também o `screenshot-path` do niri, que sem isso escrevia em `~/Pictures`). O ajuste do dono vence, em `$XDG_CONFIG_HOME`: padrão da distro OCULTA (volta), o dele REMOVE — e as frases diferem. `C-l` esconde, `C-e` desmonta, `C-u` tira, `s` do menu e `C-s` do localizador fixam: **LETRAS, porque Ctrl DIREITO com seta é paginar e a janela não o rastreia**. **Atravessar é o anel do `Tab`, e só ele** — `C-o` era um segundo caminho e saiu a pedido do dono.
- **Só o `ronoctua-disks` fala udisks2, e ele NÃO escreve regra polkit** — a do upstream já separa removível (`allow_active: yes`) de sistema (`auth_admin_keep`) pelo `HintSystem`, e uma nossa só poderia ALARGAR. **Não há agente polkit na sessão** (só `pkttyagent`, no resgate): toda chamada leva `auth.no_user_interaction`. E a razão é do DONO, não do desenho: ele nem sempre vê o pedido de digital, ou está longe do laptop — pedido não visto vira recusa no relatório, sobre algo que ninguém viu. **Nada que o sistema faça sozinho pode PERGUNTAR**, e volume de SISTEMA não oferece montar — manda ao terminal root. `HintIgnore` vira `quiet` e anda no `C-h` dos dotfiles. O `override` tira só o `xfsprogs` (538,3 → 343,2 MiB; 258 eram Python do `xfs_scrub`, e montar XFS é do KERNEL). Sinal do `ObjectManager` numa thread, mandando o `Rack` INTEIRO — o relógio do rodapé só bate com a vista aberta, e a janela não tem relógio nenhum.
- **`LEADER s f` é formatar, e são TRÊS perguntas** (`buffers/format`, `capability::disks`): qual disco, para que serve, tem certeza. Linguagem de gente — o vfat diz *"abre em qualquer computador"*, não FAT32, e **o XFS aparece DESABILITADO dizendo que falta o programa**, alcançável e recusando. **Toda linha tem DOIS registros**: o rótulo, e uma linha discreta SOB O CURSOR com o nome real e os limites (na lista, o tamanho em bytes; no confirmar, o gesto inteiro). Nome técnico é CHAVE, nunca literal no componente. Duas recusas que o hospedeiro não enxerga (de fora, recusa e componente morto são o mesmo "nenhum plano"), e por isso são do `format-driven`: disco que o sistema precisa é DESENHADO e o `Enter` nele não anda; e o primeiro `Enter` do confirmar VOLTA, porque `não` é a primeira linha. **Sem estado `working`** — o `SPC b s` o ofereceria e mentiria; a espera é na lista. O alvo é id que o buffer RECEBEU, reconferido contra o `Rack` mandado **e contra `needed()` de novo**. Disco inteiro é `dos` até 2 TiB (`gpt` acima) mais UMA partição, com `update-partition-type` — sem ele o vfat sai tipado `0x83` e o Windows não abre o pendrive, sem nada dizendo por quê.
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
- **`LEADER n` é notificação, e TUDO sai de `assets/notifications.nix`** — catálogo, filtros e as chaves que o i18n cobra. A faixa fica ACIMA da modeline (1 linha, até 3 com corpo e ações); **nada fica para sempre, nem a crítica** (`quick`/`normal`/`long`), com pizza de 8 passos e envelope na BORDA DIREITA para marcar lida; contador `2/5` só com mais de uma; sem glifo próprio, o da severidade. Filtro é `campo + teste`, sem regex, e a PRIMEIRA que casa decide — o gerenciador diz qual. Suprimida não aparece **e não conta**. Clicar no sino abre; `n h` no rodapé (duas colunas), `n w` em janela, uma `View` só; na lista, envelope e lixeira por linha (`Espaço` e `d`, que o gerador recusa como letra de ação). 15 dias em `$XDG_STATE_HOME`; seu ajuste em `$XDG_CONFIG_HOME` vence a distro. **`vibrate` e `audio` não fazem nada — e a linha de mensagem DIZ** (`INERT`, com check).
- **`LEADER r` é rede e Syncthing.** Só o `ronoctua-network` fala NetworkManager; `overrideFolders` FICA `false` (`true` apaga as pastas a cada rebuild). A senha do Wi-Fi não fica em arquivo: `psk-flags` mais o agente entregando do TPM2 (**sem PCR** — com PCR, atualizar o kernel a quebra).
- **Tokens e textos são GERADOS do Nix** (`assets/tokens/*.nix`, `*.ftl`): chave ou cor inexistente é erro de **compilação** — inv. 8 e 9. **Layout é aritmética de inteiros**, em duas fases: preferência, e "nada vaza da tela".
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
- **`LEADER s b` é o cérebro** (`ronoctua-brain`), o daemon que decide QUANDO a máquina mexe em si
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
- **Modificador segurado mata o keymap.** No aninhado a leader É `Ctrl+Alt+Espaço`: DESCONTE o que já estava em baixo ao ganhar o foco E ao começar a sequência. Caps Lock idem — `not bound` mudo.
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
