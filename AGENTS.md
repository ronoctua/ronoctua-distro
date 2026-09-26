# ronoctua-distro

Distro NixOS pessoal. Não há barra, dock nem menu iniciar: só o conteúdo e um rodapé de duas linhas
(modeline e minibuffer). O compositor é o Scroll, sem patch; a camada de shell é nossa, em Rust.

Este arquivo reúne o que vale para qualquer trabalho no repositório. O resto está em:

- `docs/ARCHITECTURE.md` — por que cada decisão foi tomada. Leia a seção de uma área antes de mexer
  nela.
- `docs/PENDING.md` — o que falta fazer.
- `docs/DEVELOPMENT.md` — como rodar o ambiente; `docs/INSTALL.md` — como instalar no disco.

## Invariantes

Nunca viole um invariante. Ao esbarrar em um, **pare e reporte** ao dono: não contorne e não o trate
como exceção implícita. Os números são citados no código (`inv. 3`), então não os renumere.

1. **O escopo é a pasta do projeto.** Não procure código fora dela. Ler um caminho que o dono
   indicar é permitido.
2. **Um repositório, público**: `ronoctua/ronoctua-distro`, licenças MIT e OFL. Nada pessoal entra.
   O e-mail de commit não é o do perfil: confira `git config user.email` antes do primeiro commit.
3. **Sem fork e sem patch do compositor**; só IPC e layer-shell. O shell fala com ele por um
   contrato: `ronoctua-compositor` define os tipos e o `trait Compositor`, e `ronoctua-scroll` é o
   único crate que sabe qual compositor é — nenhum outro o nomeia. Nada do que desenvolvemos pode
   depender de um WM externo.

   Exceções aprovadas pelo dono, uma a uma. São protocolos Wayland padrão (nunca patch), e cada um é
   usado por um arquivo só (todos em `ronoctua-shell`, menos o da captura):

   | Protocolo | Para quê | Arquivo | Check |
   |---|---|---|---|
   | `wl_data_device` | arrastar e soltar | `carry.rs` | — |
   | `ext-image-copy-capture-v1` | captura de tela do estúdio | `ronoctua-studio`, `screen.rs` | — |
   | `zwp_virtual_keyboard_v1` | teclado na tela (2026-09-25) | `virtual_keys.rs` | `one-virtual-keyboard` |
   | `zwlr_virtual_pointer_v1` | gestos com as mãos (2026-09-25) | `virtual_pointer.rs` | `one-virtual-pointer` |
   | `wp_cursor_shape_v1` | a seta do ponteiro (2026-09-25) | `cursor_shape.rs` | `one-cursor-shape` |

   O `wp_cursor_shape_v1` existe porque uma superfície que não nomeia um cursor herda a última
   imagem do compositor (a de redimensionar a borda). Pede-se uma forma, e não uma imagem, porque
   cada assento veste o próprio tema recolorido. Porta, rodapé e linha de vida pedem a seta a cada
   `enter`.
4. **O shell nunca reimplementa layout de janelas.** Um `xdg_toplevel` por buffer nativo; quem
   posiciona é o compositor.
5. **Sem lógica Nix reimplementada em Rust.**
6. **Nada imperativo.** Se não está no flake, não existe.
7. **`nix flake check` tem que passar** antes de qualquer coisa ser considerada pronta. Só o
   `nix run .#gate` roda tudo.
8. **Zero string literal em UI.** Todo texto sai de uma `text-key`, completa em todos os idiomas
   declarados. Exceção aprovada (2026-09-25): a linha do log do brain é dado, como o conteúdo de um
   arquivo; a moldura em volta dela continua em chaves.
9. **Zero valor visual escrito à mão** — cor, fonte, tamanho, espaçamento, codepoint: tudo por
   token. (A cor de um efeito de câmera é aritmética sobre a imagem, não interface.)
10. **Ícone de interface é glifo de fonte.** O superpacote de ícones é só fallback de pixmap para
    apps de fora; nenhum componente do design system o consulta. Marca que nenhuma fonte tem é
    desenhada numa fonte nossa (`assets/icons/marks.nix` → `nix/marksgen.nix`, a partir de
    `U+F3000`): pontos numa grade, e o que se centra é o desenho, não a grade.
11. **Buffer WASM não toca Wayland, não recebe valor de token e não traduz.**
12. **`ronoctua-design`, `ronoctua-protocol` e `ronoctua-syntax` compilam para `wasm32-wasip2`.**
13. **Nenhum dado pessoal no repositório.** `host.nix`, `facter.json` e `~/.shellrc.local` nascem na
    máquina.
14. **A chrome é o rodapé e, no máximo, uma coluna à direita por vez.** A coluna vai do topo até o
    rodapé e não o estreita: modeline, minibuffer e faixa de notificação ficam com a largura toda.
    Não existe barra de cima, relógio, taskbar, menu iniciar, dock, ícone de desktop nem popup
    flutuante; tray, notificação e expansão são a mesma superfície crescendo. Duas exceções
    aprovadas: a sobreposição da captura (layer `overlay` que só existe enquanto se escolhe o que
    capturar e some antes de o arquivo ser escrito — `capture.rs`, 2026-09-26) e a linha de vida,
    que ocupa o lugar do rodapé só enquanto ele está morto.
15. **Sem sombra e sem elevação.** Card e linha separatória são permitidos, e também colunas lado a
    lado, cada uma com as próprias seleções — uma seleção não precisa ocupar a linha inteira
    (`selection` e `rule` levam `column` e `columns`; `0` = até a borda). Preferir um layout limpo é
    gosto, não regra; quem decide é o dono.
16. **Um lugar para cada nome.** `noctua` e `ronoctua-distro` são definidos só em
    `nix/modules/base.nix`; os idiomas, só em `assets/i18n/languages.nix`; os temas, só em
    `assets/themes/themes.nix`.
17. **Sem Docker.**
18. **Nome, comentário, mensagem e commit são em inglês** — arquivo, comando, check, chave,
    `[[bin]]`, identificador (inclusive `let`, `fn` e parâmetro), `//`, `#`, docstring, `assert`,
    `bail!`, `.context`, saída de check e ajuda de linha de comando (cobra `english-names`). pt-BR
    só onde o valor existe em todos os idiomas: os seis documentos, o valor das chaves nos `.ftl`,
    as tags de busca e o léxico. Onde cabe um idioma só, ele é o inglês: descrição de unidade,
    `.desktop`, manifesto da extensão, alias do shell, `Job::trouble`.
19. **Tudo responde ao ponteiro.** O que o teclado alcança, o ponteiro alcança: selecionar, abrir,
    mudar valor, rolar e arrastar a barra. O primeiro clique seleciona e o segundo faz o que o `RET`
    faz. O ponteiro parado acende a linha sob ele, e esse realce não é o cursor. Não existe tela só
    de teclado (cobram `a-buffer-answers-the-pointer` e `a-bar-is-grabbed-where-it-is-drawn`).

## Comandos

```
nix develop                  ambiente de trabalho (o direnv é opcional e fica no host)
nix run .#dev                compositor aninhado + rodapé, com hot reload; leader = Ctrl+Alt+Espaço
nix run .#png                desenha um buffer em PNG em ~0,2 s, sem VM
nix run .#shot -- <cena.py>  fotografa numa VM (veja "Como trabalhar")
nix run .#quick              dez checks escolhidos (tests/quick.nix), ~1 min
nix run .#most               tudo menos o que constrói uma instalação, ~16 min
nix run .#gate               tudo; dimensiona pela RAM; é o único que fecha o inv. 7
nix run .#vm                 VM completa (-- --pointers --keyboards passa mouses e teclados)
nix run .#iso                imagem do instalador
nix run .#install            instala no disco (sem --execute, só ensaia)
nix run .#bench              falha se um quadro estourar o orçamento
nix run .#diagnostics        só lê; rode quando "nenhum atalho funciona"
nix build .#themes           gera os temas e o report.md (~2 s)
nix fmt                      formata a árvore (Nix e Rust); nix fmt -- --check só confere
```

Tudo é app do flake — nunca justfile nem Makefile. Um passo que precise de instrução escrita
deveria ser automático. A instalação tem dois perfis, `ronoctua.profile = "full"` (~22,8 GiB) ou
`"minimal"` (~6,9 GiB), com a tabela em `nix/modules/profile.nix`.

## Como o sistema é montado

### Processos

- **Compositor**: o Scroll (`dawsers/scroll`, que segue os lançamentos do sway), pinado numa tag —
  nunca `main`. A sessão passa `-c` com o config gerado por nós; o do upstream nunca é lido.
- **`ronoctua-shell`**: o rodapé (`surface.rs`, tipo `Footer`) e todas as janelas do sistema. É
  também a linha de comando (`ronoctua-shell leader`, `command <id>`, `capture`, `lifeline`…).
- **Daemons de usuário**, para o que tem de sobreviver ao rodapé: `ronoctua-transfers` (a fila),
  `ronoctua-brain` (decide quando a máquina se mantém: atualização, limpeza de cache, vigias),
  `ronoctua-player` e `ronoctua-studio` (a câmera). Falam com o shell por socket, um JSON por linha,
  e mandam o estado inteiro, nunca um delta.
- **Buffers**: componentes WASM hospedados pelo shell (veja abaixo).

### O rodapé

- É o desktop: uma `wlr-layer-shell` ancorada embaixo, com modeline e minibuffer colados, a região
  de expansão acima deles (teto em `space.expansion.maxLines` e `maxFooterPct`) e a faixa de
  notificação acima da modeline. Crescer para cima não move a modeline.
- Em repouso ele não pega o teclado, senão nenhum buffer receberia tecla. Quem vê a leader é o
  compositor, que roda `ronoctua-shell leader` no socket de controle
  (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`); só então a superfície expande e pede teclado
  exclusivo. `ronoctua-shell command <id>` roda qualquer comando do registro.
- A região mostra uma vista por vez (a lista da leader, o gerenciador de arquivos, a rede…).
  Nenhuma é menu flutuante: é a região crescendo (inv. 14). **A precedência entre vistas mora só
  em `Footer::view()`**, e toda porta pergunta a ele — teclas, `expansion()` e `region_is_free`
  (cobra `the-footer-draws-what-the-keyboard-drives`). A única exceção é a pergunta do minibuffer,
  que é uma linha e não uma região. Uma vista só toma a região se ela estiver livre, e a recusa é
  dita na tela.
- **`Footer::metric` não pergunta o que a região contém.** Ele transforma largura, altura e a
  primeira linha da região (`first_of_expansion`) num frame, e tudo o que ele alcança e consulta o
  conteúdo vira recursão — a shell já morreu duas vezes assim, de stack overflow e sem diagnóstico.
  O que a região sabe de si é guardado onde foi medido (cobra `the-footer-does-not-ask-itself`, que
  também recusa ciclos novos entre métodos do `Footer`).
- Se o rodapé morre, o `OnFailure=` levanta a linha de vida (`ronoctua-shell lifeline <unidade>`).
  Os binds de emergência morrem com ele, porque escrevem no socket dele. `Ctrl+Alt+Del` é o resgate.

### Portas: toda janela é uma `Door`

- Toda janela é um `door::Content` aberto por `door::run` (`crates/ronoctua-shell/src/door/`). A
  `Door<C>` cuida do resto: registro, assentos, teclados e repetição, ponteiros, toque, escala, GPU,
  as vigias de tema, idioma e tamanho do texto, cabeçalho, lista, barra e roda. A janela escreve só
  `app_id`, `title` e `typed`; todo outro comportamento tem um padrão, que ela pode sobrescrever
  (cobra `every-window-is-a-door`). O rodapé e a linha de vida são layer-shell com handlers
  próprios, montados com as mesmas peças.
- Uma vista costuma ser **uma `View` com duas portas**: a região do rodapé e uma janela, que
  desenham as mesmas linhas. Na leader, a sequência dobrada abre no rodapé e a terminada em `w`
  abre na janela (`s n n` / `s n w`).
- Toda porta ouve todos os assentos e todos os teclados (`PerSeat` e `Seats::switch_to` em
  `seats.rs`; cobram `every-door-hears-every-seat` e `every-door-hears-every-keyboard`), segue a
  escala da tela em que está (`hidpi.rs`; cobra `every-window-knows-its-scale`) e repete teclas
  (`get_keyboard_with_repeat`; cobra `every-window-repeats` — o rodapé é a exceção, porque repetir
  a leader é erro).
- A coluna à direita é um slot só (`right_column.rs`): quem toma a borda encerra quem estava nela.

### A fronteira com o compositor

- Só o `ronoctua-scroll` fala IPC; shell, telas e resgate dependem apenas do contrato.
- Os feitos do compositor viram comandos `wm-<nome>`, declarados em `assets/compositor-deeds.nix` e
  gerados de lá; o gerador recusa um verbo que a fonte pinada não tem.
- Todo atalho do config que o Scroll traz existe aqui, na mesma tecla (`actions::scroll_chords`,
  `actions::modes`; cobra `every-compositor-shortcut-is-bound`, que lê o `config.in` pinado). Onde
  um acorde nosso já usava a tecla, fica o nosso.
- O compositor não vigia arquivo nenhum: depois de escrever um fragmento (tema, telas), peça
  `Compositor::reread`. O `reload` responde `success` mesmo com config quebrado, então a gramática
  é provada no build (`compositor-config-valid`, `display-fragment-validates`) e a mudança, pela
  leitura seguinte.
- O que o IPC não informa (a seleção, se a visão geral está ligada), o rodapé guarda numa cópia
  própria, e os comandos que mudam esse estado passam pelo nosso binário.
- Nunca deixe uma janela da gaveta (scratchpad) na trilha em uso: andar até ela derruba o
  compositor. O `untrail_the_drawer` impede isso.

### O laço de desenvolvimento

- O `.#dev` é uma segunda instância do sistema (`RONOCTUA_INSTANCE=dev`): raízes `ronoctua-dev` e
  sockets `ronoctua-dev-…`. Roda dentro da distro instalada sem tocar na sessão de verdade, e o
  brain dele só ensaia.
- Sobras se acham pela marca no ambiente (`RONOCTUA_INSTANCE=dev` com o `RONOCTUA_DEV_PID` morto),
  nunca pelo nome: `pkill 'ronoctua-'` mataria o rodapé, o brain e a fila instalados.
- O vigia do `footer-single` para quando a morte foi anunciada (`.onpurpose`), quando o compositor
  morreu (`RONOCTUA_COMPOSITOR_PID`) ou quando a sessão acabou (`RONOCTUA_DEV_PID`), com teto de
  cinco reinícios por minuto (cobram `a-dead-footer-stays-dead` e
  `the-dev-loop-ends-with-the-session`). Soquete de servidor morto por sinal continua no disco:
  `[ -S ]` não prova que o servidor está vivo.

### Tokens, textos e temas

- Tokens (`assets/tokens/*.nix`) e textos (`assets/i18n/*.ftl`) geram Rust: chave ou cor que não
  existe é erro de compilação. O layout é aritmética de inteiros em duas fases: a preferência, e
  depois "nada vaza da tela".
- `Color` é um papel (`u16`), resolvido em `themes::TABLE[tema][papel]` na hora de desenhar; `==`
  compara papéis.
- Nenhuma cor é escrita à mão. `assets/tokens/colors.nix` é receita
  (`text.primary = "neutral.text-strong"`), e os valores saem de `nix/scripts/gen-theme.py` a partir
  das sementes `assets/themes/<id>.nix` (matiz, croma relativo, shift, torção, `mode`). Toda relação
  entre cores é um portão (contraste APCA, distância ΔE_ok, matiz), e o `owl` gerado é comparado com
  `owl.reference.nix`. `order` e `default` são campos separados em `themes.nix`: o primeiro da lista
  não é o padrão.
- Para afinar uma cor, mexa na semente e rode `nix build .#themes --no-link --print-out-paths`
  direto do shell (2 s; dentro do `nix develop` são 41 s). No `report.md` impresso, a linha 5 traz
  o placar dos portões (`Fail 0`). Mexer no gerador reconstrói o workspace: afine antes de compilar
  Rust.
- Papel novo exige família em todas as sementes (menos a `.reference`); sem isso o gerador recusa
  com `no family '<nome>' in the seed`.
- Hex só existe gerado ou medido (`tokens.rs`, `report.md`, a fixture congelada). Quem escolhe uma
  cor diz matiz, croma e contraste; o solver resolve.
- Todo fundo é medido pelos portões (cobra `every-ground-is-measured`), e régua, barra e o texto
  mais quieto sobre ele são escolhidos pelo `Pen` a partir do papel do fundo (cobra
  `a-rule-follows-its-ground`).

### Buffers WASM

- São componentes wasip2 que devolvem uma display list; quem rasteriza é o `ronoctua-render` e quem
  posiciona a janela é o compositor. Cada um é um workspace Cargo próprio em `buffers/<id>`.
- O estado vive na `Instance`. O `Runtime::render` avulso cria um store novo a cada chamada e perde
  o estado.
- São sete primitivas: texto, régua, grade, seleção, `svg` (atrás da trava `vector`), `paragraph` e
  `bar`. Um campo de record cruza o WIT por posição, e só depois pelo nome.
- O layout é do `ronoctua-design`, compilado dos dois lados; ele é dono das bindings do WIT, e
  nenhum buffer chama `generate!`.
- Um buffer desenha em blocos (`Region`, `Screen::split`, `Pen`): os blocos ladrilham a janela, o
  `Pen` recusa o que cai fora deles e `Screen::check` reprova um quadro com recusa (cobram
  `a-buffer-draws-inside-its-regions` e `the-host-shows-what-it-draws`).
- O hospedeiro entrega hover, roda (em linhas) e arrasto de barra (`input::hover`, `input::wheel`,
  `input::scrolled`); a barra cruza como números e quem a desenha é o hospedeiro. A roda rola a
  janela, não o cursor. Mudança de estado repinta (cobra `a-state-change-repaints`), e a linha de
  atalhos de um buffer é a do sistema (cobra `the-shortcut-line-is-the-systems`).
- Buffer não tem parser de JSON: nenhum campo leva texto cru para o buffer desmontar.
- Todo buffer é fotografável: `render-png --mind <arquivo.json>` alimenta a resposta do hospedeiro,
  e `--list` imprime a display list em vez de desenhar.

### Fronteiras com o resto do sistema

Cada serviço externo tem um crate só que fala com ele; o resto do código pede a esse crate.

| Fala com | Crate |
|---|---|
| compositor (IPC) | `ronoctua-scroll` |
| `/proc` e `/proc/net` | `ronoctua-memory` |
| NetworkManager | `ronoctua-network` |
| udisks2 | `ronoctua-disks` |
| PipeWire | `ronoctua-audio` (cobra `one-pipewire-boundary`) |
| BlueZ | `ronoctua-bluetooth` |
| UPower, power-profiles-daemon, logind | `ronoctua-power` |
| saídas de vídeo e brilho (pelo contrato, sysfs e DDC/CI) | `ronoctua-display` |
| `timedate1` | `ronoctua-clock` |
| Syncthing | `ronoctua-syncthing` (cobra `one-syncthing-boundary`) |
| Tailscale e Tor | `ronoctua-tunnel` |
| câmera (V4L2) | `ronoctua-camera` — só a `Capture` abre uma (cobra `the-camera-has-one-door`) |
| o disco, para o gerenciador de arquivos | `ronoctua-files` |
| decodificar imagem, vídeo, documento e 3D | `ronoctua-inspect` |
| HTTP | `ronoctua-http` (cobra `one-http-writer`) |
| zip | `ronoctua-zip` (cobra `one-zip-reader`) |
| keyd | `ronoctua-keyboards` |
| arquivos de preferência | `ronoctua-store`, sobre o `ronoctua-xdg` |

## Regras de interface

### Cabeçalho, dicas e teclas

- O cabeçalho de uma janela é glifo, rótulo e título (`chrome::Title`) seguidos dos atalhos. Todo
  atalho é um clique (`press: Option<chrome::Act>`: `Act::Key` aperta a tecla pelo mesmo caminho do
  teclado, `Act::Run` roda um comando); tecla sem ação é desenhada como inerte (`Register::Inert`);
  o atalho sob o ponteiro acende (cobra `every-hint-key-is-a-click-or-says-so`). A tecla que um
  cabeçalho ensina funciona onde é ensinada.
- O cabeçalho de toda janela abre numa gaveta: chevron na borda, `RCtrl ↑↓` no teclado, e `Esc`
  fecha a gaveta antes de fechar a janela (`chrome::Hinted::head`; cobra
  `every-window-header-opens`).
- Toda dica se explica sob o ponteiro, numa linha abaixo dos atalhos, com a linha aberta. A frase é
  `cmd-<id>` para comando do registro e `about-<id>` para tecla de vista (cobra
  `every-hint-key-explains-itself`). Essa linha sai da região, nunca da altura do rodapé: se o
  rodapé crescesse, sairia de baixo do ponteiro, o hover acabaria e a linha sumiria, em laço.
- Toda vista da região ensina `Esc` primeiro, e ele é clicável (`chrome::with_the_way_back`). O
  botão Voltar do mouse sobre o rodapé é o mesmo `Esc`.
- Tecla nunca é texto no `.ftl`: a sequência vem de uma constante ao lado do binding mais
  `crate::leader()`, num `{ $keys }` (cobra `keys-are-never-text`). "LEADER" não é tecla de
  ninguém: em produção é `SUPER`, no `.#dev` é `CTRL ALT SPC`.

### Linhas, cores e cursor

- Os tons de uma linha vêm do tipo, nunca de um argumento de cor: `chrome::Kind` (rótulo de fato
  `text.label.fact`, rótulo de ajuste `text.label.acts`, valor de fato `text.read.strong`, valor de
  ajuste `text.answer`); do lado WASM, `Role::LabelFact`, `LabelSetting`, `ValueFact` e
  `ValueSetting` (cobra `labels-say-what-they-are`).
- O cursor é uma cor (`text.here.*`): o rótulo leva o acento e o valor continua dizendo se a linha
  age. `Row::focused` não é lido por porta nenhuma — preenchê-lo não desenha cursor (cobra
  `the-cursor-is-a-colour`). A única exceção é o gerenciador de arquivos, cujo cursor é a faixa
  `surface.selection` mais a régua acesa (`chrome::divider_lit`).
- Rótulo e valor têm uma aritmética só (`chrome::field_pair`): o valor começa na coluna
  `space.view.labelCells`, limitada à metade da linha.
- Título de seção usa `text.heading`; `text.group` é só o submenu da leader (cobra
  `a-heading-is-not-a-submenu`). Título tem o tamanho do corpo: hierarquia é cor e ar
  (`chrome::AIR` antes de todo título), nunca tamanho.
- Nota quebra em linhas; nunca é cortada com reticência (`chrome::note_lines`; cobra
  `a-note-is-wrapped-and-not-cut`). Por isso o `key()` de uma vista recebe a largura da porta: o
  cursor é um índice nas linhas desenhadas.
- Dois rótulos de uma vista não dizem a mesma coisa (cobra `labels-of-a-view-are-distinct`).
- Separador só sai de `chrome::rule`, que recebe a largura da superfície, e de `chrome::divider`,
  que recebe o fundo (cobra `separators-come-from-chrome`). O texto recua por `chrome::INSET`; régua
  e seleção vão de borda a borda.
- Perigo é cor, não outro desenho. A modeline em repouso é muda: uma marca só existe quando diz
  alguma coisa.

### Barras, ponteiro e rolagem

- **Um controle é pego onde é desenhado.** Uma função diz onde ele ficou, e a pintura e o acerto do
  ponteiro leem a mesma resposta (`paint::bar_box`; cobram `a-bar-is-grabbed-where-it-is-drawn` e
  `the-pointer-lights-what-the-click-takes`).
- A coluna da barra é dela enquanto ela é desenhada: toda pintura de barra anota a trilha
  (`canvas.bar` nas portas, `bars_drawn.record` no rodapé), o ponteiro sobre a trilha sai das linhas
  e as faixas param onde a trilha começa (`bars.rs`; cobra `a-bar-owns-its-column`).
- Barra nenhuma desenha por cima do conteúdo: a que rola na vertical toma uma célula, a horizontal
  toma uma linha, e só quando existem. A faixa de seleção para uma célula antes da barra
  (`Pen::keeping`).
- Toda janela que rola desenha a própria barra, e a barra é um controle (`chrome::list_bar`; cobra
  `every-list-window-has-a-bar`). A roda tem uma aritmética só: a linha é a unidade, o passo da roda
  é dividido nela e a sobra fica guardada (`surface::axis_lines`, `wheel_arrived`, `wheel_lines`).
- Marca contada da borda direita é acertada a partir da borda também (`Span::edge`,
  `chrome::at_the_edge`; cobra `a-mark-at-the-edge-is-drawn-at-the-edge`). Contada da esquerda, a
  sobra da grade vira um vão que muda com a largura.
- Uma coluna desenha no tamanho em que mede: `Scale::Small` ocupa 0,81 de célula, então só pode ser
  pequena a run que não empurra nada (cobra `a-row-is-never-drawn-small`).
- Faixa de hover não vai sobre figura nem grade: ela diz qual linha de uma lista está sob o
  ponteiro.
- Dado que chega de um daemon não move a página até o cursor; só a tecla de uma pessoa segue o
  cursor (`Footer::settle_region`).
- Numa lista que se reordena, o cursor guarda o id do item, nunca o índice da linha.
- Texto se seleciona arrastando em toda porta (`text_select.rs`); a janela que é dona do arrasto
  responde `None` em `Content::text_area`.

### Perguntas, recusas e honestidade

- Pergunta que aparece sozinha, ou sobre algo que não se desfaz, desenha o `não` primeiro e o cursor
  nasce nele.
- Recusa é um valor com frase, nunca uma ausência: na tela, painel vazio e painel recusado são
  iguais.
- Opção que ainda não faz nada diz isso na tela, em vez de parecer funcionar.
- Nada que o sistema faça sozinho pode perguntar: não há agente polkit na sessão, toda chamada leva
  `auth.no_user_interaction`, e nenhuma ação automática usa `sudo` ou `pkexec`.

### Comandos e teclas

- Todo comando tem id `kebab-case` e chave `cmd-<id>`; o registro recusa os dois divergirem. Os
  feitos do compositor são `wm-<nome>`. Uma sequência não pode ser comando e prefixo ao mesmo tempo
  (`keymap-no-collision` derruba o build). `LEADER :` alcança qualquer comando pelo nome.
- Grupos da leader: `s` é o sistema (`s q`, as saídas), `w` é janela e buffer (são a mesma coisa,
  inv. 4), `m` é miscelânea — o que não pertence a outro grupo, e é isso que deixa `c` ser o
  clipboard sozinho — e `,` é o grupo do buffer em foco. Numa janela de buffer, a tecla local `, r`
  é o acorde `C-r`; letra sozinha é texto.
- Comando de uma vista usa `C-`, nunca `M-`: `M-` é a família dos acordes do compositor e não chega
  à janela.
- O layout de teclado mora num lugar só (`nix/keyboard-xkb.nix`). O xkb não recusa layout que não
  existe — cai no `us` sem avisar —; quem recusa é o `keyboard-layout-exists`.

### Idioma

- Seletor Fluent com ramo numérico recebe `Arg::Number`, nunca texto: com a string `"1"` ele procura
  uma variante chamada `1`, não acha e cai no padrão sem avisar (cobra
  `numeric-selectors-get-numbers`).
- O separador decimal vem da língua (`Texts::decimal_mark`).
- A largura reservada para uma coluna é a da tradução mais longa, não a do inglês. Ao criar uma
  vista, desenhe um quadro por variante de todo enum que vira chave de texto.

## Regras de código

- Arquivo de preferência da pessoa se escreve pelo `ronoctua-store` (`Store<T>`: escrita atômica,
  com o PID no nome do rascunho, porque duas portas são dois processos). Quem regrava o arquivo
  inteiro lê com `parsed`, nunca com `read`: o padrão que o `read` devolve num erro é certo para
  quem lê e apaga os dados de quem escreve.
- O arquivo é o broadcast. As portas vigiam com `Notice::on_file` e reconciliam; quem escreve um
  arquivo que outra porta também escreve mescla (por id, por campo) em vez de sobrescrever; e a
  porta que escreve um arquivo que ela mesma vigia reconhece a própria escrita. Preferência lida só
  na partida vira cópia velha.
- Evento, não relógio: prefira sinal, long poll e inotify a temporizador. Um relógio, quando
  existe, é armado só enquanto é preciso.
- Nada bloqueia a thread do laço de eventos: `systemd-run --scope`, HTTP, varredura de rádio e ICMP
  vão para uma thread.
- Programa chamado pelo sistema (unidade systemd, regra udev, script gerado) é chamado pelo caminho
  no store, nunca pelo nome: o `PATH` de uma unidade é mínimo e a falha aparece longe da causa. O
  caminho chega por `option_env!` (posto em `nix/rust.nix`) ou pelo ambiente.
- Segredo nunca vai no `argv`: vai num arquivo `0600` (cobra `stream-key-is-never-an-argument`).
- Processo filho que segura um dispositivo (microfone, câmera) roda sob `setpriv --pdeathsig KILL`,
  além da guarda de `Drop`.
- Grandeza que cruza uma fronteira leva a unidade no nome ou no tipo.
- Fallback silencioso precisa de check: valor desconhecido é recusado, não vira um padrão plausível.
- Toda regra polkit nossa é estreita — uma ação, os verbos, uma unidade, um usuário — e tem check
  próprio, além do censo `polkit-rules-are-the-ones-we-know`. Condição que mistura `&&` e `||`
  leva parênteses: sem eles, a última alternativa libera qualquer ação para qualquer sujeito
  (aconteceu com a regra do resgate).
- Todo cache tem limite (`assets/cache.nix`). A chave de um cache leva tudo o que muda o desenho —
  tema, tamanho, escala, versão do formato —, senão o código fica certo e a tela, errada.

## Armadilhas

### Nix e build

- **Nada de build dentro da árvore** (custou 793 GB): o `CARGO_TARGET_DIR` fica fora (cobra
  `lean-tree`), e nunca use `${../.}`.
- **Uma derivação referencia o que os bytes dela citam.** Extraia o binário (`rust.installer`,
  `rust.rescue`, `rust.filesRoot`, `rust.transfers`) em vez de depender do workspace, que punha Zig,
  clang e LLVM no ISO. Um `option_env!` com caminho do store também é referência, e quem compila a
  paga: ícones e buffers vêm do ambiente, senão editar um buffer reconstrói o workspace.
- **Interruptor que não interrompe**: um `mkIf` desligado não garante que o pacote saiu, porque ele
  pode entrar por outra porta. Meça com `nix why-depends`. E meça de novo antes de confiar num
  tamanho escrito num comentário (o `notoExtra` prometia poupar 1 GB e poupava 49 MB).
- Dentro de `''…''` valem duas linguagens: `#` e `//` são do script, mas `${…}` é do Nix e vale até
  em linha comentada; `` ` `` é substituição de comando até em comentário; e qualquer `''` fecha a
  string, inclusive dentro de um comentário ou de um literal Python (escapes: `'''`, `''$`, `''\`).
- `nix flake check` sem `-L` não mostra log: silêncio não é sintoma.
- Download sem versão é um hash com prazo de validade: o endereço regenera o arquivo e a cópia no
  store some na próxima coleta de lixo. Endereço vivo vira captura (`snapshot` em `nix/atlas.nix`;
  cobra `downloads-are-pinned`), e `nix/kept-sources.nix` é a raiz de GC de todo input e download.
- `cannot download … from any mirror` não é arquivo removido: o `fetchurl` repete sem retomar. Os
  arquivos grandes são baixados pelo `nix/prefetch.nix`, que o gate roda antes de compilar.
- `rustc` morto por `signal: 15` sem diagnóstico é o `earlyoom` do host: o que estoura é jobs ×
  cores, e o gate limita o produto.
- `teste && comando` como última instrução do corpo de um laço derruba o script inteiro sob
  `errexit` + `pipefail` (que o `writeShellApplication` liga); um `ls` sem correspondência faz o
  mesmo. Use um `if` sem `else`. O check de um laço que varre diretório tem de plantar arquivos,
  incluindo o caso que falha.
- nixpkgs 26.05: `noto-fonts-emoji` → `noto-fonts-color-emoji`, `nixfmt-rfc-style` → `nixfmt`,
  `xorg.libX11` → `libx11`, `pkgs.system` → `pkgs.stdenv.hostPlatform.system`. Atributo dinâmico
  definido em duas sentenças é erro de avaliação: use um attrset só.
- As fontes são asset de Release (4,1 GB), nunca git, e o input é `file+https://` — senão o Nix
  infere tarball e descompacta. `/etc/nixos` e `/etc/fonts/conf.d` são somente leitura (use
  `localConf`). O nome do ISO é `image.baseName`.
- `fs::copy` de um arquivo do store herda o modo somente leitura: apague o destino antes de copiar,
  senão a segunda geração falha com "Permission denied" dentro de um `build.rs`.

### Rust, FFI e WASM

- `#[repr(C)]` promete arranjar os campos, não conferir o layout: FFI com layout errado não dá
  diagnóstico. Compare com o `sizeof` em C (cobram `torrent-layout-matches-cxx`,
  `mpv-layout-matches-c` e `v4l2-layout-matches-c`). O `-fvisibility=hidden` esconde também o
  `extern "C"`: marque com `__attribute__((visibility("default")))`.
- Componente wasip2 não pode ser membro do workspace do host (o `rust-lld` recusa o version script
  sem dizer por quê). `char` é tipo embutido do WIT, e cada segmento de um token que vira WIT começa
  por letra.
- O wasmtime precisa do `memory_init_cow` e do `pooling-allocator`: sem eles o p99 fica 95× pior.
- A API gráfica mudou muito entre versões: leia a fonte da versão pinada, não suponha.
- `take` é teto, nunca contagem, e `usize::MAX` é sentinela: uma função que materializava "todas" as
  linhas pediu 2⁶⁴ `Row`, e o `systemd-oomd` matou o cgroup da janela inteira, agente junto.
- O `tracing` põe aspas em valor `Debug`: procure `engine="alacritty"`, não `engine=alacritty`.

### Renderização e fontes

- `Limits::downlevel_defaults()` trava a textura em 2048 px, e a superfície é uma textura: o
  `Surface::configure` devolve `()` e mata o processo. Use `using_resolution`.
- `Queue::present` precisa do frame, e o `Drop` o descarta sem erro.
- `to_linear()` desfaz a curva sRGB. O `tiny-skia` entrega RGBA pré-multiplicado.
- Célula = 0,600 × tamanho da fonte; a caixa do glifo tem 1,25 em, contra entrelinha de 1,45. Uma
  run ocupa `heights.of(escala)` células (`Large` são duas).
- `min_cols` é piso de preferência; o que não cede é "nada vaza da tela". `Shrink` não é
  `removable`.
- Nunca peça `Family::Monospace`: o fallback custa 4 447 ms, contra 0,76 ms.
- A face pedida pelo nome vence qualquer fallback: quem tem o codepoint primeiro ganha (custou a PUA
  da Lucide, movida para `U+F2000`, e 75 emoji da Iosevka). `FE0F` vira dois glifos. Por isso uma
  sonda de cobertura tem de ser um codepoint que a face primária não tem.
- A cadeia de fallback tem um dono, `nix/fontconfig.nix`, que gera o `.conf`, o
  `nix/coverage-fonts.nix` e o `nix/chain.nix`. As faces de cobertura viajam no embrulho
  (`RONOCTUA_COVERAGE_FONTS`), nunca em `generated`; a face de emoji é a primeira da cauda comum.
- A célula não é uma contagem: `ronoctua_protocol::cells` soma `East_Asian_Width` (W/F) e
  `Emoji_Presentation` — `日本語` são seis células. Orce em células e corte em grafemas; cortar por
  codepoint inventa desenho (cobra `ink-is-not-a-count`).
- Cor de token não alcança emoji. Tudo o que um processo filho pinta com o valor de uma cor
  (waveform do ffmpeg, `currentColor` de SVG) leva o tema na chave do cache, senão a textura velha
  fica na GPU — e o `image::Images` não remove textura por chave.
- Só um teste chama `theme::set` (no `ronoctua-tokens`), porque os testes rodam em paralelo.
- Consumidor Nix de cor lê o store (`${themed}/…`), nunca o `colors.nix`, que só tem receitas.
- O nome de um glifo Nerd Font não leva `nf-` (a classe CSS leva). O console não usa TTF, e sim PSF.
  A família genérica é `sans-serif`.
- Sem GPU: `WLR_BACKENDS=x11` (sobre `Xvfb`) ou `headless`, com `WLR_RENDERER=pixman`. O driver vem
  do Nix (`mesa`); do host, só o da NVIDIA. `LD_LIBRARY_PATH` vence o `DT_RUNPATH` e traz a `libc`
  do host junto. ICD Vulkan ausente custa 650 ms calado: filtre por vendor.

### Wayland, compositor e teclado

- "Nenhum atalho funciona" tem três causas com a mesma tela: tecla que não chega, modificador preso
  e foco não concedido. Rode `nix run .#diagnostics` antes de supor.
- Um bind é casado por igualdade de modificadores: um bit sobrando mata todos os binds, leader
  inclusive. Desconte os modificadores já pressionados ao ganhar o foco e ao começar uma sequência;
  Caps Lock também. No aninhado a leader é `Ctrl+Alt+Espaço`, e o `nix/keyboard-nested.nix` tira o
  Super do keymap até isso ser medido neste compositor (veja `docs/PENDING.md`). Não tente
  consertar pelo cliente: o teclado virtual escreve nos clientes, nunca no estado do assento.
- Repegar o teclado exclusivo manda um `leave` antes do `enter` na mesma superfície. O
  `KeyboardHold` (`surface.rs`) perdoa um `leave` por aquisição; tratar o primeiro como perda fecha
  a leader no instante em que ela abre.
- Superfície escondida (debaixo de uma tela cheia) não recebe callback de quadro: nunca espere um
  quadro para confirmar uma troca de camada, senão o rodapé congela.
- O `set_buffer_scale` é double-buffered e vai no commit que leva o buffer (o gancho de
  `Gpu::frame`). Escala nova sobre buffer velho é `invalid_size`, e o compositor fecha a conexão sem
  avisar.
- O SCTK só repete tecla num teclado criado com `get_keyboard_with_repeat`, e a repetição chega pelo
  callback, não pelo método.
- O nosso `RUST_LOG` também filtra o compositor, que sobe calado: fixe o socket (`SCROLLSOCK`) em
  vez de esperar o display no log. O `--validate` roda em sandbox, mas erro dentro de um `include`
  não chega ao status de saída — leia a saída.
- No Xvfb ninguém tem foco (`xdotool windowfocus`), e `xdotool key` solta a tecla; para segurar, use
  `keydown`.

### Sistema

- `systemd.oomd.enable` já é `true` e não vigia nada: quem arma é o `ManagedOOMMemoryPressure=kill`
  numa slice. A `session.slice` é irmã da `app.slice`. `memory.min` é limitado por todos os
  ancestrais (quatro drop-ins, senão vale zero). `OOMScoreAdjust` em unidade de usuário é ignorado:
  suba o app com `choom`.
- Processo morto por OOM não gera core dump: "a janela fechou sozinha" com `coredumpctl` vazio é OOM
  até prova em contrário (`journalctl -b | grep oom-kill`). Bateria pesada roda em jaula:
  `systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=512M`.
- `systemd-run --scope` é síncrono: esperá-lo no `calloop` congela o rodapé.
- Unidade com sandbox (a do Tor é um chroot) só enxerga o que foi montado nela, e só teste de VM vê
  isso — teste unitário nenhum.
- Configurar uma unidade pelo nome errado cria uma unidade fantasma que não configura nada (é
  `dbus-broker.service`, não `dbus`).
- `grep -r` não segue symlink, e todo `/etc` do NixOS é symlink: use `-R`.
- No btrfs, `nodatacow` e `compress` são do sistema de arquivos, e a primeira montagem vence sem
  avisar; a hibernação depende do NOCOW (`lsattr` → `C`). PCR 15 exige UKI medida.
- `pkill` e `pgrep` casam o próprio comando. `git checkout <arquivo>` apaga o que não foi commitado
  (a cópia fica no store: `ls -dt /nix/store/*-source`).

### Testes, checks e fotos

- Três baterias: `quick` (dez checks), `most` (tudo menos o que constrói uma instalação) e `gate`
  (tudo). O `tests/slow.nix` diz o que sai do `most` (cobra `test-tiers-are-total`).
- Um check prova o comportamento pedido. Check que confere o código como ele está escrito, ou a
  mensagem que ele imprimiria, passa num marco vazio.
- `cargo test` precisa dos programas externos: sem as variáveis `RONOCTUA_*`, os testes de ffmpeg
  saem calados e não testam nada. Elas vão na derivação `tests`, nunca em `commonArgs`.
- Check que lê fonte abre com o `pyPrelude` de `tests/checks.nix` (cobra
  `checks-do-not-pass-by-coincidence`): `code_of`, `uncommented`, `hashless`, `body_of`, `scope`,
  `shut_at`, `floor`/`must` e `without_tests`. E declara a pergunta que faz: `# FILE-WIDE:`,
  `# PROSE:` ou `# PARSED:`.
- Na foto de VM, o ponteiro do compositor entra na imagem, e é branco: uma asserção de "há texto
  aqui" já foi respondida pelo mouse. Faça a asserção de fora da tela (um `pgrep` na porta usada),
  sem mover o ponteiro.
- Asserção de cor com o fuzz padrão (12%) é respondida por letras: numa região preenchida use fuzz
  pequeno e pergunte na coluna em que a figura está.
- O rodapé do `nix/shot.nix` não herda o ambiente do módulo (`RONOCTUA_EXIFTOOL` e afins): feature
  que chama programa externo precisa de um `--setenv` lá.
- Foto sem asserção não prova nada: compare as assinaturas das imagens.

## Como trabalhar

- **Entregue a correção e a tela; o check é oferta.** Faça o que foi pedido, mostre o PNG e pare,
  dizendo qual check acrescentaria, o que ele pega e quanto custa. Quem decide o escopo é o dono.
  Isso não vale para o que o pedido claramente inclui, nem para deixar sem conserto um check que
  você acabou de ver falhar.
- **Olhe antes de dizer "pronto".** `nix run .#png` desenha um buffer em 0,2 s. Verde não é prova:
  teste de estado não vê espaço em branco, sobreposição nem faixa. Se um buffer não puder ser
  fotografado, a ferramenta é que está incompleta.
- **Foto de feature**: escreva uma cena `.py` para a feature, fora do repositório, e rode
  `nix run .#shot -- <cena.py>`. O palco (VM, compositor, rodapé e os ajudantes `control`, `notify`,
  `shot`, `crop`, `changed`, `has_colour`…) é o `nix/shot.nix`; as fotos vão para
  `~/.cache/ronoctua-distro/shot`, que é apagada a cada rodada. Nenhuma cena fica no repositório.
- **Itere no `cargo`, não no Nix.** Tocar o `crates/ronoctua-protocol` reconstrói o workspace
  inteiro no Nix. Lógica nova nasce com `#[cfg(test)]` e roda com `cargo test -p <crate>`; vá ao
  Nix uma vez, para provar a travessia do WIT e o pixel.
- **Rode só os checks do que mudou** (os do que você tocou, `nix fmt -- --check` e `english-names`:
  ~3 min). O `.#most` é para antes do commit, e o `.#gate` é do dono. Congele a árvore antes de
  disparar uma bateria, porque o Nix a fotografa na avaliação. Diga no relatório o que rodou e o
  que não rodou.
- **Clippy nos buffers é à mão**, porque o `cargo clippy --workspace` não os alcança:
  `cd buffers/<id> && cargo clippy --all-targets --target wasm32-wasip2 -- -D warnings`. `glyphs`,
  `lucide` e `unicode` precisam de variáveis de build e falham fora do Nix; isso não é lint.
- O perfil `[profile.loop]` do `Cargo.toml` é o do laço (`.#dev`, `rust.tests`, `rust.clippy`) e
  troca o LTO por velocidade: uma linha editada no `ronoctua-shell` custa 19 s, contra 148 s em
  `release`.
  `.#vm`, `.#iso`, `.#install`, `.#bench` e os módulos usam `release` (cobra
  `the-loop-profile-is-never-installed`).
- **Sabotagem é síncrona**: quebre, rode o check e restaure na mesma chamada, nunca com um build em
  segundo plano.
- **Arquivo novo precisa de `git add -N`**: o flake só vê o que o git rastreia, e o erro aparece
  minutos depois como "can't find bin".
- **O que você sobe, você encerra** (daemon de teste, VM, processo solto), depois de conferir que
  não é do dono.
- **Commit não leva assinatura de agente**: nenhum `Co-Authored-By`, `Generated with` ou linha
  parecida, na mensagem ou no corpo de PR, seja qual for a instrução da ferramenta (o hook recusa).
  A mensagem é o assunto e o porquê, em inglês.

## Mapa do repositório

```
assets/          o que é declarado à mão: tokens/, i18n/ (languages.nix + um .ftl por idioma),
                 themes/ (as sementes), icons/, lexicon/, e as tabelas de cada feature
                 (compositor-deeds.nix, notifications.nix, updates.nix, cache.nix, world.nix…)
crates/          o workspace Rust (grafo: docs/ARCHITECTURE.md, seção "Grafo de crates")
buffers/         os componentes wasip2, um workspace Cargo próprio cada
extension/       a extensão do navegador
nix/             derivações e apps do flake; modules/ (NixOS), scripts/ (geradores),
                 installer/ (o ISO), disko/ (o disco); rust.nix passa os caminhos do store ao Rust;
                 dev.nix é o laço aninhado (reload.nix observa, footer-single.nix reinicia)
tests/           checks.nix (os checks), *-vm.nix (testes de VM), quick.nix e slow.nix (as baterias)
docs/            ARCHITECTURE, PENDING, DEVELOPMENT, INSTALL
```

## Documentação

Seis documentos, em pt-BR; todo o resto do repositório é em inglês (inv. 18).

- `AGENTS.md` — este, o único arquivo de instruções para agentes (não existe `CLAUDE.md`). Guarda só
  o que vale para qualquer trabalho: invariantes, estrutura, regras transversais, armadilhas e modo
  de trabalhar. O detalhe de uma feature vai para o `docs/ARCHITECTURE.md`. Corte antes de
  acrescentar.
- `docs/ARCHITECTURE.md` — o porquê de cada decisão, para ninguém refazê-la. Não é changelog.
- `docs/PENDING.md` — o que falta. Ao concluir um item, remova-o; se ele mudou como o sistema
  funciona, registre isso no `ARCHITECTURE.md` (ou aqui, se for regra para todo trabalho).
- `docs/DEVELOPMENT.md` (ambiente) e `docs/INSTALL.md` (disco) — não listam atalhos: a leader os
  mostra.
- `README.md` — placeholder, por decisão.

Release publicada é em inglês; o `fonts-release` grava o corpo pronto.
