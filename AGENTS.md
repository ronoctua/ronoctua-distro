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
   usado por um arquivo só (todos nos crates `ronoctua-shell-*`, menos os dois da captura):

   | Protocolo | Para quê | Arquivo | Check |
   |---|---|---|---|
   | `wl_data_device` | arrastar e soltar | `carry.rs` (recebem: `surface/mod.rs`, o rodapé; `files_window.rs`; `software_window.rs`) | — |
   | `ext-image-copy-capture-v1` | captura de tela do estúdio | `ronoctua-studio`, `screen.rs` | — |
   | `zwlr_screencopy_v1` | a mesma captura, quando falta o de cima (2026-10-04) | `ronoctua-studio`, `screen.rs` | — |
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
6. **Nada imperativo.** Se não está no flake, não existe. As exceções aprovadas pelo dono — cada uma
   dado, nunca código, e nunca no repositório ou em `/etc`:
   - **Programas da pessoa** (`ronoctua-software`): instala/experimenta/atualiza/remove, só para o
     usuário, pacotes do perfil Nix, Flatpaks `--user`, AppImages, web apps e apps Android sob `$HOME`;
     o suporte é declarado no flake. Fora da home só a imagem do Android (`/var/lib/waydroid`, unidade
     root estreita) e o store.
   - **Regras e janela de segurança, e os containers**: o firewall base é declarado; as regras da
     pessoa ficam em `/var/lib/ronoctua-security` e uma unidade declarada as aplica na tabela `inet
     ronoctua`; a janela de sudo sem senha é uma linha em `/var/lib/ronoctua-sudo` (acaba no `NOTAFTER`
     ou quando a pessoa fecha, e uma vencida sai no boot); as chaves `pam_u2f` são do root
     (`/var/lib/ronoctua-security/u2f_keys`), escritas só pela porta do sudo; Podman só na home.
   - **O `/private`**: só pelo gesto, pela porta do sudo, a unidade `ronoctua-private-root`
     cria/abre/tranca uma imagem LUKS em `/var/lib/ronoctua-private` (subvolume `@private`) montada em
     `/private`; `ronoctua-private-lock` a tranca antes de dormir. Nunca no flake, `/etc`, `fstab` ou
     `crypttab`.
   - <!-- anchor: L03 --> **A máquina tem o próprio flake, e a pessoa declara**: `/etc/nixos` é o flake
     da máquina, a distro como input na branch `release` (`lib.machine`, `templates/machine`), com os
     pins que a distro testou (`machine-uses-the-distro-pins`); o nome da pasta só em `nix/machine.nix`
     (`machine-dir-has-one-name`). O que a pessoa declara (`person/software/*.json`) é dado: cada
     pacote/AppImage/app Android é uma PARTE (`personParts`) que o realizador avalia sozinho; web apps e
     Flatpaks `--user` vão para ela, nunca para o `systemPackages`.
   - <!-- anchor: L06 --> **As declarações da pessoa chegam como dados**: `ronoctua-person-apply.service`
     (uma regra polkit de verbo/unidade/usuário) valida o JSON em `/run/ronoctua-person/staged` e o
     troca para `person/` na forma canônica (link, especial, formato novo e tamanho acima do teto são
     recusados). Só os módulos brain levam código, e o `module.nix` só entra num build aprovado pela
     porta do sudo no hash de `nix/person-hash.nix` (`ronoctua-person-approve.service`).
   - <!-- anchor: L13 --> **O Incus da pessoa**: daemon, ponte `incusbr0`, pool e perfil `default`
     declarados (`nix/modules/incus.nix`, perfil `full`); instâncias/imagens/volumes em
     `/var/lib/incus`. Não busca sozinho (`images.auto_update_interval = 0`), e toda chamada a um remoto
     leva o `Pass` de cada host (`hosts_of`; cobra `incus-asks-with-a-pass`). `incus-admin` ≡ root.
   - <!-- anchor: L19 --> **A atualização instala só o declarado**: `ronoctua.apps` e `person/software/`
     entram pelo rebuild e pelo reconciliador (`ronoctua-software reconcile`); nenhum argumento nomeia o
     que instalar. A ordem é lei do gerador (`steps.nix`, `system-update-laws-refuse`): a pessoa reúne,
     a metade root migra/move só o input da distro e aplica (`ronoctua-machine apply`), a pessoa
     reconcilia por último; depois `parts-plan`/`realize-parts` enraíza as partes. Agendada espera rede
     tarifada/bateria baixa e nunca pergunta; à mão pergunta com o "não" primeiro.
   - **O cache binário da pessoa**: escolha no `options.json`, token de envio só na parte cifrada; fora
     da home só o netrc do root (`/var/lib/ronoctua-binary-cache/netrc`, `0600`, `binary-cache-netrc`),
     removido quando o token/cache saem; nunca no flake, store ou `/etc`.
   - **Apps da distro seguindo releases**: só com `distro-apps-follow-releases` ligada (o gesto que
     permite `api.github.com`/`github.com`), o `person-pins` leva cada AppImage com `follows.github` à
     release nova (dado assinado da pessoa); os bytes são download dela, a versão é o pin da PARTE,
     nunca no `systemPackages` nem num cache público, e um download que falha fica na geração anterior
     (cobra `distro-apps-follow-when-on-and-newer`).
   - **Extensões do VS Code e o Quicklisp**: só pelo gesto (com o `Pass` de `hosts_of`), vão para
     `~/.vscode/extensions`/`~/quicklisp` a partir de lista-dado ou arquivos pinados; nunca no flake,
     `/etc` ou repositório.
   - <!-- anchor: L25 --> **O que precisa de download chega como PARTE, fora do sistema**: declarada no
     flake (`ronoctua.parts.<id>`, fora de `mkIf`), realizada só pelo `ronoctua-parts` (root) num perfil
     Nix do root; o fecho do sistema nunca a contém (o plano `/etc/ronoctua/parts-plan.json` a nomeia
     como texto, lida por `ronoctua_parts::here`, e a tela diz que ela está chegando). Cada parte é um
     `ronoctua-part@<id>`: uma que falta é retentada mais tarde e fica na geração anterior; a sessão a
     vê por `/var/lib/ronoctua-parts/view` (`bin/` + `share/`, nunca `lib/`). **Não livre é aprovado**
     (cobra `unfree-never-refuses`) mas **nunca entra no ISO publicado** (cobra
     `iso-carries-nothing-undistributable`). **A instalação nunca usa rede** (`nix/installer/offline.nix`,
     todo `nix` `--offline`); o ISO completo `.#iso-complete` é do desenvolvedor e **nunca publicado**
     (cobra `the-complete-iso-is-never-released`).

7. **`nix flake check` tem que passar** antes de qualquer coisa ser considerada pronta. Só o
   `nix run .#gate` roda tudo. Emenda aprovada pelo dono (2026-10-21): o gate faz as perguntas do
   `nix flake check` em avaliadores que cabem na RAM — os checks nas fases 0 a 4, e o resto (o lock,
   cada app, pacote, shell, módulo, template e sistema) na fase 5, um avaliador por grupo —, porque
   o flake num processo só passou de 9 GiB com um quarto dos checks numa máquina de 14 GiB.
8. **Zero string literal em UI.** Todo texto sai de uma `text-key`, completa em todos os idiomas
   declarados. Exceções aprovadas: a linha do log do brain (2026-09-25), o texto que a pessoa digita
   num lembrete e a amostra de código que um buffer mostra (o `example`, 2026-10-04) são dado, como
   o conteúdo de um arquivo, e também o nome próprio de um programa — o `name` do registro e o
   `Name=` de um `.desktop`, igual em todo idioma (2026-10-20, aprovado no plano da leva); a moldura
   em volta deles continua em chaves.
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
14. **A chrome é o rodapé e, no máximo, uma coluna à direita por espaço de trabalho.** Cada tela
    mostra a coluna do espaço que está nela, e um inquilino existe uma vez só (abri-lo em outro espaço
    o muda de lugar). A coluna vai do topo ao rodapé e não o estreita (modeline, minibuffer e faixa de
    notificação ficam com a largura toda); em qualquer tamanho ela é um inquilino numa faixa, nunca
    dock nem taskbar — não lista nem lança programas. Não existe barra de cima, taskbar, menu iniciar,
    dock, ícone de desktop nem popup flutuante; tray, notificação e expansão são a mesma superfície
    crescendo, e a barra de alerta máximo (bateria fraca) toma o lugar da faixa enquanto existe. A
    barra do sudo (emenda do dono, 2026-10-11) é a mesma superfície crescendo acima da faixa e da
    modeline, só enquanto um gesto de root é perguntado (`sudo_bar.rs`). Três exceções aprovadas: a
    sobreposição da captura (layer `overlay` até o arquivo ser escrito — `capture.rs`, 2026-09-26); as
    cortinas do redimensionador (`LEADER / r`, duas superfícies `overlay` enquanto ele está ligado —
    `resizer.rs`, 2026-09-27); e a linha de vida, que ocupa o lugar do rodapé só enquanto ele está
    morto.
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
    só onde o valor existe em todos os idiomas: os seis documentos e o `why.md` de cada módulo do
    brain, o valor das chaves nos `.ftl`,
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
nix run .#png                desenha um buffer, ou uma porta nativa (--door), em PNG em ~0,2 s, sem VM
nix run .#shot -- <cena.py>  fotografa numa VM (veja "Como trabalhar"); --apps é o palco do Flatpak
nix run .#quick              os checks escolhidos (tests/quick.nix), ~1 min
nix run .#most               tudo menos o que constrói uma instalação, ~16 min
nix run .#gate               tudo; dimensiona pela RAM (e as VMs pelo que declaram); fecha o inv. 7
nix run .#vm                 VM completa (-- --pointers --keyboards passa mouses e teclados)
nix run .#iso                imagem do instalador (instala sem rede)
nix run .#iso-complete       o ISO completo do desenvolvedor, com as partes não livres; nunca publicado
nix run .#install            instala no disco (sem --execute, só ensaia)
nix run .#bench              falha se um quadro estourar o orçamento
nix run .#diagnostics        só lê; rode quando "nenhum atalho funciona"
nix build .#themes           gera os temas e o report.md (~20 s)
nix fmt                      formata a árvore (Nix e Rust); nix fmt -- --check só confere
nix run .#rust-plan          regenera nix/rust/plan.json (o plano do cargo) depois de um Cargo.toml
nix run .#rebuild-cost       o que a árvore (ou -- <arquivo>) reconstrói, e quanto, sem construir
nix run .#sized-build -- …   nix build dimensionado à máquina; .#rust-peaks (-- --write) e as classes
nix run .#texts -- missing <idioma>   o que falta traduzir, por fragmento (-- where <chave>)
```

<!-- anchor: L04 -->

```
nix run .#distro-pins        move os pins dos AppImages e apps Android da distro; o .#release o roda antes
```

Todo programa que a distro instala para a pessoa escolher é uma entrada `ronoctua.apps.<id>`
(`nix/modules/distro-apps.nix`), no módulo dono e **fora de qualquer `mkIf`**, nunca em
`systemPackages`. O que segura um programa segue `ronoctua.distroApps.installed.<id>`, não o
interruptor do grupo. Id publicado entra no livro-razão de `assets/distro-apps.nix` e só sai por
`formerly`/`retired`.

<!-- anchor: L16 -->

```
nix run .#release            publica: pins movidos (seu único commit), gate, release (só para frente), ISO
nix run .#fetch-iso          baixa o ISO da versão, confere cada parte e o inteiro, junta
nix run .#iso-sweep          apaga os ISOs que nada enraíza, menos o mais novo de cada nome
nix run .#adopt-machine      máquina do layout antigo → flake próprio (migrate + apply, com sudo)
nix run .#apply-here         a máquina com o checkout no lugar do input; o lock fica como estava
```

A branch `release` é o que toda máquina segue: só o `.#release` a move, e ele é do dono, como o
`.#gate`. A distro não tem cache binário (o ISO leva a semente); o cache é da PESSOA:
nome/chave/visibilidade no `options.json` (parte assinada), token só na parte cifrada, e só
`nix/modules/binary-cache.nix` declara substituter/chave/netrc (cobra `binary-cache-declared-once`).
O token nunca vai em argumento, processo longo ou log (cobra `binary-cache-token-never-prints`). Um
cache público recebe só saídas livres — um não livre vai no `atRunTime` (cobra
`release-cache-holds-only-free-builds`).

Tudo é app do flake — nunca justfile nem Makefile; um passo que precise de instrução escrita deveria
ser automático. A instalação tem dois perfis, `ronoctua.profile = "full"` (~37 GiB) ou `"minimal"`
(~12,5 GiB), com a tabela em `nix/modules/profile.nix`. Os dois trazem o trabalho na distro: o
toolchain do devshell no store (`nix/toolchain.nix`, fonte única), o rust, o Common Lisp e os agentes
no PATH, o KVM e o `nix-direnv`.

## Como o sistema é montado

### Processos

- **Compositor**: o Scroll (`dawsers/scroll`), pinado numa tag, nunca `main`; a sessão passa `-c`
  com o config gerado por nós, nunca o do upstream.
- **`ronoctua-shell`**: o binário fino que compõe os ~50 crates `ronoctua-shell-*` (base, uma
  funcionalidade por crate, o rodapé `ronoctua-shell-footer`) e a linha de comando (`leader`,
  `command <id>`, `photograph`…; a tabela `DOORS` de cada crate).
- **Daemons de usuário** (sobrevivem ao rodapé): `ronoctua-transfers`, `ronoctua-brain`,
  `ronoctua-player`, `ronoctua-studio`. Falam por socket, um JSON por linha, sempre a mensagem
  inteira (nunca delta); o grande que quase ninguém mostra não vai no estado — ele leva a geração e
  a porta pede fora do laço.
- **`ronoctua-software`**: não é daemon — biblioteca e CLI dos programas da pessoa (`LEADER i`); toda
  ordem é um `Work::Software` que a fila roda como filho, e instalar para a pessoa é DECLARAR
  (`Order::Declare`, grava em `declared.json`).
- **`ronoctua-browser`**: host de native messaging do Chrome quiosque; vive e morre com a extensão,
  conecta no rodapé-servidor (`ronoctua[-dev]-browser.sock`) e manda o estado inteiro.
- **Buffers**: componentes WASM hospedados pelo shell.

### O rodapé

- É o desktop: `wlr-layer-shell` ancorada embaixo, modeline e minibuffer colados, a região de
  expansão acima (teto `space.expansion.maxLines`/`maxFooterPct`; o filme do player vai a
  `playerStagePct`), a faixa de notificação acima da modeline. Crescer para cima não move a modeline.
- Em repouso não pega o teclado; a leader é vista pelo compositor, que roda `ronoctua-shell leader`
  no socket de controle (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`), e só então a superfície expande
  e pede teclado exclusivo. `ronoctua-shell command <id>` roda qualquer comando do registro.
- Uma vista por vez, nunca menu flutuante (inv. 14). **A precedência mora só em `PRECEDENCE`**
  (`surface/mod.rs`), que `Footer::view()` lê; todo despachante é um `match` sobre `view()`, uma
  linha por vista (cobra `the-footer-draws-what-the-keyboard-drives`). A vista é a região do crate
  (`regions.rs`) e só toma a região livre.
- **`Footer::metric` não pergunta o que a região contém** — consultar o conteúdo dali vira recursão;
  o que a região sabe de si é guardado onde foi medido (cobra `the-footer-does-not-ask-itself`).
- **Dois modos ocultos** (`hidden.rs`): um `Ctrl` na raiz da leader gira normal → empurra → cobre
  (`hidden::next_mode`, `footer-bars-cycle`); nos dois o rodapé fica na camada `overlay`.
- **A modeline se adapta a um programa** (`adapt.rs`): o que desenha responde ao ponteiro, e todo
  segmento roda um comando do registro (cobra `footer-mouse-reaches-nothing-new`).
- Se o rodapé morre, o `OnFailure=` levanta a linha de vida (`ronoctua-shell lifeline <unidade>`);
  `Ctrl+Alt+Del` é o resgate.
- **Uma vista que segue um daemon lê ao vivo, não do cache.** Quando o rodapé vigia um daemon por
  um fluxo de sinais de longa vida — um `MessageIterator` de `PropertiesChanged` numa conexão que
  vive com o processo — e desenha a partir das MESMAS propriedades, leia-as SEM cache
  (`CacheProperties::No` nos proxies do snapshot, um `Get` ao vivo por leitura). O próprio vigia
  consome o fluxo que manteria o cache de propriedade em dia, então uma leitura em cache congela no
  estado de quando o processo subiu, e a vista (e a modeline) passam a mostrar o passado — atualizando
  só quando a página é reaberta, que relê do zero. O snapshot que alimenta a vista é a verdade de
  agora, não a de quando o rodapé nasceu.

### Portas: toda janela é uma `Door`

- Toda janela é um `door::Content` aberto por `door::run`; a `Door<C>` cuida de registro, assentos,
  teclados, ponteiros, escala, GPU, vigias de tema/idioma/texto/data, cabeçalho, lista, barra e roda
  — a janela escreve só `app_id`, `title`, `typed` (cobra `every-window-is-a-door`). Toda janela
  está na `DOORS` do crate, toda câmera na `CAMERAS` (cobra `every-door-is-dispatched`).
- Uma vista costuma ser **uma `View` com duas portas** (região do rodapé + janela) que desenham as
  mesmas linhas; na leader a sequência dobrada abre no rodapé e a terminada em `w` na janela, e o
  grupo com as duas vira uma tecla só (`bindings::folds`, fixa em `the_collapsed_groups_are_these`).
- **A vista é uma região** (`region::Region`): UM `carry_out` sobre `&mut dyn region::Doorway` que
  janela e rodapé implementam (cobra `errands-are-handled-alike`); lê o que o rodapé empresta, sem
  reabrir a thread do daemon (cobra `regions-order-the-footers-threads`).
- **As duas portas fazem as mesmas coisas**: cada nível, tecla, colar, soltar arquivo e **digitar** —
  nenhuma manda a pessoa ir à outra, uma lacuna é defeito, e o que uma porta DIZ é dito nela.
  `Ui::ask`/`Ui::paste`/`Ui::launch` fora do laço; colam a seleção viva (`pasting.rs`), nunca o
  histórico; a mensagem sai com o prazo da classe (`Ui::say`/`Ui::say_for`/`Ui::hold`), nunca
  `Ui::message` direto (cobra `door-messages-expire`).
- Toda porta ouve todos os assentos e teclados (cobram `every-door-hears-every-seat`,
  `every-door-hears-every-keyboard`), segue a escala da tela (`hidpi.rs`; cobra
  `every-window-knows-its-scale`) e repete teclas (cobra `every-window-repeats` — o rodapé é a
  exceção).
- As colunas à direita têm uma base só, `column::Column<T>`; o inquilino (`column::Tenant`) desenha
  na `column::Area` que recebe, um slot por espaço (`right_column.rs`), estacionada fora da tela
  (`Shell::Parked`).

### Módulos do brain

- Um módulo é uma pasta `crates/ronoctua-brain/modules/<id>/` (`manifest.nix`, `module.rs`,
  `module.nix`, `i18n/<idioma>.ftl`, `checks.nix`, `why.md`; opcional `tables.nix`, `shared.rs`); o
  id é o nome da pasta, e `nix/brain-modules.nix` recusa pasta incompleta ou campo desconhecido.
- O rodapé linka só a biblioteca `ronoctua-brain`; todo `module.rs` é do `[[bin]]` `ronoctua-brain`
  (cobram `brain-daemon-is-a-leaf`, `brain-library-links-no-module`). O que o rodapé desenha é o
  catálogo (`RONOCTUA_BRAIN_CATALOGUE`), dado de runtime (cobra
  `brain-catalogue-is-what-the-daemon-compiled`).
- Nada de fora cita o id, chave, glifo, unidade ou ação de um módulo, salvo o que o `namedBy` lista
  (cobra `brain-modules-are-sealed`). Uma marca abre um comando só por `MarkId::opening(<comando>)`;
  preferência resolve sobre `Prefs::of_module`, toda mudança um `Ask::Set` do núcleo.

### A fronteira com o compositor

- Só o `ronoctua-scroll` fala IPC; shell, telas e resgate dependem do contrato, uma conexão por
  programa (`daemon::open_the_compositor`).
- Feitos do compositor viram comandos `wm-<nome>`, declarados em `assets/compositor-deeds.nix`; o
  gerador recusa verbo que a fonte pinada não tem.
- Todo atalho do config do Scroll existe aqui, na mesma tecla (cobra
  `every-compositor-shortcut-is-bound`); onde um acorde nosso já usava a tecla, fica o nosso.
- O compositor não vigia arquivo: depois de escrever um fragmento, peça `Compositor::reread` — o
  `reload` responde `success` mesmo quebrado, então a gramática é provada no build
  (`compositor-config-valid`, `display-fragment-validates`).
- O que o IPC não informa (seleção, visão geral), o rodapé guarda numa cópia própria, e os comandos
  que a mudam passam pelo nosso binário.
- Nunca deixe uma janela da gaveta na trilha em uso (derruba o compositor): `untrail_the_drawer`
  impede.

### O laço de desenvolvimento

- O `.#dev` é uma segunda instância (`RONOCTUA_INSTANCE=dev`, raízes e sockets `ronoctua-dev`),
  dentro da distro instalada sem tocar na sessão real; o brain dele só ensaia.
- Sobras se acham pela marca no ambiente (`RONOCTUA_INSTANCE=dev` com `RONOCTUA_DEV_PID` morto),
  nunca pelo nome: `pkill 'ronoctua-'` mataria o instalado.
- O vigia do `footer-single` para na morte anunciada (`.onpurpose`), no compositor morto
  (`RONOCTUA_COMPOSITOR_PID`) ou no fim da sessão (`RONOCTUA_DEV_PID`), com teto de cinco
  reinícios/min (cobram `a-dead-footer-stays-dead`, `the-dev-loop-ends-with-the-session`); `[ -S ]`
  não prova servidor vivo.
- **Trocar o binário do rodapé não precisa de reboot nem logout.** Depois de um `nixos-rebuild
  switch`, o rodapé novo entra reiniciando só o serviço de usuário dele —
  `systemctl --user daemon-reload && systemctl --user restart ronoctua-footer.service` —, que o
  reexecuta enquanto o compositor e as janelas seguem de pé (a barra só pisca). O `switch` em geral
  já reinicia esse serviço; o comando é a garantia, e é instantâneo perto de reiniciar a máquina.
  Dá para rodá-lo de um shell sem sessão gráfica desde que o ambiente tenha `XDG_RUNTIME_DIR`.

### Tokens, textos e temas

- Os NOMES geram Rust (papéis de cor e ícones, `nix/token-names.nix`; as chaves de cada crate,
  `nix/text-keys.nix`): chave ou cor que não existe é erro de compilação. Os VALORES — frases,
  paletas, o catálogo do brain — são dado lido em tempo de execução (`RONOCTUA_TEXTS`,
  `RONOCTUA_THEMES`, `RONOCTUA_BRAIN_CATALOGUE`), e o que falta é recusado numa linha em inglês que
  nomeia a variável. O layout é aritmética de inteiros em duas fases: a preferência, e depois "nada
  vaza da tela".
- **Uma frase pertence ao crate que a diz** (o `i18n/<idioma>.ftl` dele; dita por vários, ao crate
  mais próximo sobre o qual todos ficam, nunca ao `ronoctua-i18n`), e num checkout o `ronoctua-keys`
  escreve as chaves e os textos fora da árvore como a avaliação (cobra `keys-agree`).
- `Color` é um papel (`u16`), resolvido na paleta carregada (`ronoctua_tokens::palettes`,
  `[tema][página][papel]`) na hora de desenhar; `==` compara papéis.
- **As faixas seguem o fundo.** Todo tema é resolvido em quatro páginas — o chrome e as mesas
  `backdrop`, `near` e `absolute` —, com texto, faixas e réguas medidos a partir de cada uma. Quem
  fica sobre uma mesa VESTE a página dela (`theme::set_ground`) e pinta como no chrome; nunca ponha
  um tom da página do chrome sobre uma mesa. Rodapé e coluna vestem a resposta `footer` do
  `theme.json`; portas, buffers e terminal, a `windows`. Faixa, cérebro, relance, bordas e mesas
  são iguais em toda página.
- Nenhuma cor à mão: `assets/tokens/colors.nix` é receita (`text.primary = "neutral.text-strong"`),
  e os valores saem de `nix/scripts/gen-theme.py` das sementes `assets/themes/<id>.nix`; toda relação
  é um portão (APCA, ΔE_ok, matiz). `order` e `default` são campos separados em `themes.nix`. (Afinar
  cor e adicionar papel: `docs/ARCHITECTURE.md`.)
- Hex só existe gerado ou medido (`palettes.bin`, `report.md`); quem escolhe uma cor diz matiz,
  croma e contraste e o solver resolve. As faixas de legenda (`subtitle.*`) são a exceção de página,
  medidas contra uma imagem escura fixa e só vistas num chip de `subtitle.edge` (`chrome::chip`).
- Todo fundo é medido pelos portões (cobra `every-ground-is-measured`); régua, barra e o texto mais
  quieto sobre ele saem do `Pen` pelo papel do fundo (cobra `a-rule-follows-its-ground`).

### Buffers WASM

- Componentes wasip2 que devolvem display list; rasteriza o `ronoctua-render`, posiciona o
  compositor. Cada um é um workspace Cargo em `buffers/<id>`.
- O estado vive na `Instance`; `Runtime::render` avulso cria store novo e perde o estado.
- Sete primitivas (texto, régua, grade, seleção, `svg` atrás da trava `vector`, `paragraph`, `bar`);
  um campo de record cruza o WIT por posição, depois por nome.
- O layout é do `ronoctua-design`, dono das bindings do WIT — nenhum buffer chama `generate!`.
- Desenha em blocos (`Region`, `Screen::split`, `Pen`) que ladrilham a janela; o `Pen` recusa o que
  cai fora e `Screen::check` reprova o quadro com recusa (cobram `a-buffer-draws-inside-its-regions`,
  `the-host-shows-what-it-draws`).
- O hospedeiro entrega hover/roda/arrasto de barra (`input::hover`/`wheel`/`scrolled`); a roda rola a
  janela, não o cursor; mudança de estado repinta (cobra `a-state-change-repaints`), e a linha de
  atalhos é a do sistema (cobra `the-shortcut-line-is-the-systems`).
- Buffer não tem parser de JSON: nenhum campo leva texto cru para desmontar. Todo buffer é
  fotografável (`render-png --mind <arquivo.json>`, `--list`).

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
| câmera (V4L2) | `ronoctua-camera` — só a `Capture` abre uma para quadros (cobra `the-camera-has-one-door`); ler a lista abre cada nó e pisca a luz, então só depois de um gesto da pessoa, nunca na partida, nem pelo WirePlumber da sessão (`ronoctua.camera.pipewireCameras`, desligado; cobra `camera-vm`) |
| o disco, para o gerenciador de arquivos | `ronoctua-files` |
| decodificar imagem, vídeo, documento e 3D | `ronoctua-inspect` |
| HTTP (redirecionamento e gzip só em `web`) | `ronoctua-http` (cobra `one-http-writer`) |
| zip | `ronoctua-zip` (cobra `one-zip-reader`) |
| XML que vem de fora (a guarda de profundidade) | `ronoctua-xml` |
| tags e capas de um arquivo de mídia (lofty, mkvtoolnix, remux do ffmpeg) | `ronoctua-tags` |
| LRCLIB, SponsorBlock, MusicBrainz, Cover Art Archive, Wikidata, TMDB, OpenSubtitles, SubDL e as chaves da pessoa | `ronoctua-lookup` |
| radio-browser, iptv-org e archive.org | `ronoctua-broadcast` |
| yt-dlp: toda linha de comando dele | `ronoctua-ytdlp` |
| keyd | `ronoctua-keyboards` |
| arquivos de preferência | `ronoctua-store`, sobre o `ronoctua-xdg` |
| perfil Nix da pessoa, Flatpak, AppImage, Waydroid, web apps do Chrome (o `Local State`); o espelho da declaração da pessoa (`$XDG_STATE_HOME/ronoctua/person/`), a preparação dela para o root e o razão do reconciliador (`declared-applied.json`) | `ronoctua-software` (cobram `one-software-boundary` e `reconcile-touches-only-the-ledger`) |
| o Chrome em execução: a extensão, o native messaging, o `Preferences` de um perfil e as janelas quiosque | `ronoctua-browser` |
| o consentimento por host (`consent.json`) e o `Pass` de toda requisição | `ronoctua-consent` (cobram `the-network-waits-for-consent` e `network-asks-with-a-pass`) |
| MyMemory, DeepL, LibreTranslate, o WordNet offline e o Wiktionary | `ronoctua-words` (cobra `words-ask-with-a-pass`) |
| Wikipedia, DuckDuckGo, SearXNG e Brave Search | `ronoctua-websearch` |
| o super-índice do fast-access (tantivy) | `ronoctua-index` |
| o journal do systemd (`journalctl --follow`) | `ronoctua-journal` |
| a paleta do console do kernel (os 16 slots de um VT, `ESC ] P`) | `ronoctua-vt` |
| podman | `ronoctua-containers` (cobra `one-podman-boundary`) |
| PAM | `ronoctua-pam` (cobra `one-pam-boundary`) |
| systemd-resolved (`resolve1`) | `ronoctua-resolve` (cobra `one-resolve-boundary`) |
| git | `ronoctua-git` (cobra `one-git-driver`) |
| 7zz, bsdtar e unrar | `ronoctua-archive` (cobra `one-archive-driver`) |
| `sudo` (a porta do sudo), o firewall da pessoa, a lista de chaves em que o `pam_u2f` confia, o `pamu2fcfg` (o cadastro de uma chave) e as checagens online da segurança | `ronoctua-security` (cobra `sudo-door-is-one`) |
| o repositório cifrado da configuração da pessoa, e a identidade dela guardada pela sessão no keyring do kernel (`@u`, só no `unlock.rs`) | `ronoctua-belongings` (cobram `belongings-ask-with-a-pass` e `one-keyring-boundary`) |
| o YouTube por HTTP simples (feeds e miniaturas) e o catálogo SQLite | `ronoctua-video-metrics` (cobra `one-sqlite-boundary`) |
| as taxas de câmbio do BCE | `ronoctua-calc` (`rates.rs`; cobra `rates-ask-with-a-pass`) |
| as identidades age pós-quânticas da pessoa (`AGE-SECRET-KEY-PQ-1…`, `age1pq1…`) e as assinaturas ML-DSA da pessoa; o `age` em si só no `ronoctua-seal`, filho da fila | `ronoctua-pq` (cobra `transfers-never-link-age`) |
| `cryptsetup`, `losetup` e a imagem do `/private` | `ronoctua-private` (cobra `one-private-boundary`) |
| os Releases da distro no GitHub e a escrita crua num pendrive | `ronoctua-stick` (cobra `one-stick-boundary`) |
| o Incus (a API REST pelo socket e o CLI) | `ronoctua-incus` (cobra `one-incus-boundary`) |
| o estado do Claude Code e do Codex, e o endpoint de uso da Anthropic | `ronoctua-llm` (cobra `one-llm-boundary`) |
| `bootctl`, as variáveis EFI, o `reboot-on-error` do `loader.conf`, o `grub-editenv` e o `grubenv` (BIOS), e o diretório da máquina | `ronoctua-machine` (cobra `one-bootctl-driver`) |
| os dados declarados da pessoa (`declared.json`, `declined.json`, os relatórios) e a cópia root deles (`person/`, `local/approved.json`, `local/signer.json` no diretório da máquina) | `ronoctua-person` (cobra `one-person-boundary`) |
| o Cachix: o cache binário da pessoa, o token de envio, o netrc do root e o envio depois da atualização | `ronoctua-cachix` (cobra `binary-cache-token-never-prints`) |
| o VS Code: o manifesto das extensões instaladas, a lista da pessoa que viaja nos pertences e o `code --install-extension` | `ronoctua-vscode` (cobra `one-vscode-boundary`) |
| as partes: os perfis delas (`/nix/var/nix/profiles/ronoctua/parts`), os registros, a vista da sessão (`/var/lib/ronoctua-parts`) e as raízes do plano | `ronoctua-parts` (cobram `one-parts-boundary` e `parts-rule-is-narrow`) |

<!-- anchor: L01 --> **Cifra e assinatura novas da pessoa são pós-quânticas** (`ronoctua-pq`, o
único que conhece o X-Wing): recipiente `age1pq1…` que nunca mistura com clássico (selar e abrir
recusam), formato decidido pelos vetores e pelo `age` do Go (cobram `pq-xwing-vectors`,
`pq-age-interop`), ML-DSA-65 para assinar; identidade e semente só em `0600` ou memória que se apaga,
nada as imprime (cobra `pq-identity-never-prints`). A máquina confia na chave que root fixou
(`local/signer.json`) ou na que a senha abriu, nunca na do repositório. A parte em claro dos
pertences viaja assinada, com contador que só cresce; o não-assinado fica retido com o motivo
(cobram `belongings-tampered-plain-is-held`, `belongings-plain-rollback-is-refused`,
`person-apply-refuses-unsigned-tree`, `brain-unsigned-module-never-compiled`).

<!-- anchor: L02 --> **Transporte pós-quântico primeiro.** TLS é rustls sobre `aws-lc-rs` com
`prefer-post-quantum`, o provedor passado ao builder e nunca instalado como padrão do processo
(`ronoctua_http::tls::provider`); um host sem o híbrido continua alcançado (cobra
`tls-is-post-quantum-first`). `aws-lc-rs` é o único provedor das primitivas salvo exceções escritas
(cobra `one-crypto-implementation`); o `ring` do lock não compila — pergunte ao `cargo tree`. SSH
negocia PQ primeiro e todo sshd tem só a chave ed25519 (cobram `ssh-kex-is-pq-first`,
`no-rsa-host-key`); o transporte dos pertences é SÓ PQ (`pq_only`; cobram `belongings-ssh-is-pq-only`,
`belongings-https-is-pq-only`).

<!-- anchor: L05 --> **Só o `ronoctua-machine apply` muda o sistema** (resgate, atualização, central
de programas): constrói antes, troca depois, arma o ensaio de boot (systemd-boot, ou GRUB na BIOS).
Nenhum outro crate roda `bootctl`/`grub-editenv`, escreve `LoaderEntry*`/`loader.conf`/`grubenv`,
mexe no perfil do sistema ou chama `switch-to-configuration` (cobra `one-bootctl-driver`). Build
falho devolve o lock em execução (cobra `apply-reverts-the-lock-on-build-failure`); o bless pende do
alvo padrão, nunca do `graphical.target` (cobra `update-apply-rule-is-narrow`), e julga pela
identidade do boot, não pelo relógio (cobra `generations-keep-the-boot-trial`).

<!-- anchor: L06 --> **Só o binário `ronoctua-person` (root) escreve o `person/` e o `local/` da
máquina** (`apply`, `approve`, `pin`); ler é de todos (cobra `one-person-boundary`). Todo arquivo da
pessoa se lê com `ronoctua_person::parse` (formato novo recusado antes dos campos) e se escreve com
`ronoctua_person::to_json`; o que root abre sob a pasta da pessoa é relativo a ela, com `O_NOFOLLOW`
(cobram `person-apply-refuses-links`, `person-apply-rule-is-narrow`). Quem mexe nesses dados segura
`ronoctua_person::root::Lock` (um `0600` de root).

<!-- anchor: L10 --> **Os pertences levam as chaves da pessoa, nunca o acesso a elas**: os `targets`
(`assets/belongings.nix`) viajam selados, os privados voltam com os bits do dono só
(`kept::kept_mode`), e a chave de implantação nunca viaja (`crate::sift`); um alvo nunca fica sobre
arquivo do Home Manager (cobra `belongings-targets-avoid-home-manager`). A identidade que a senha
abre fica só no keyring do kernel pela sessão (marcador `runtime/unlocked`), esquecida no sono/fim.
**Uma chave de segurança responde ao sudo, e só a ele**: `pam_u2f` sempre `sufficient`, só no serviço
`sudo`, atrás de uma porta de terminal, `origin` fixo (`u2f::ORIGIN`), lista do root
(`u2f::AUTHFILE`), nunca `nouserok`; só o verbo `u2f-keys` da porta a escreve (cobra
`pam-u2f-is-only-sufficient`). O dedo segue o mesmo molde.

<!-- anchor: L12 --> **`/private` não se toca num laço de eventos.** Suspenso ao dormir com algo em
uso, quem o tocar espera no kernel até a senha retomá-lo. Listar/`stat`/vigiar pergunta antes
`ronoctua_private::may_touch`; dentro do cofre aberto, a leitura roda numa thread que o laço espera
por um instante só.

<!-- anchor: L13 -->

<!-- anchor: L14 --> O `ronoctua-llm` só EMBRULHA a statusline de uma conta do Claude Code e devolve
a da pessoa byte a byte (cobra `statusline-chains`). O token OAuth é lido só no `usage.rs`, em
memória zerada, nunca renovado (cobra `llm-tokens-never-linger`). Quem vigia uma conta pergunta os
caminhos ao `ronoctua_llm::watched`.

<!-- anchor: L17 -->

## Regras de interface

### Cabeçalho, dicas e teclas

- O cabeçalho é glifo, rótulo e título (`chrome::Title`) + atalhos, e todo atalho é um clique
  (`chrome::Act`: `Act::Key`, `Act::Run`); tecla sem ação é inerte, a sob o ponteiro acende (cobra
  `every-hint-key-is-a-click-or-says-so`). A tecla que um cabeçalho ensina funciona onde é ensinada.
- O cabeçalho abre numa gaveta (chevron, `RCtrl ↑↓`); `Esc` fecha a gaveta antes da janela (cobra
  `every-window-header-opens`).
- Toda dica se explica sob o ponteiro, numa linha abaixo dos atalhos: `cmd-<id>` para comando do
  registro, `about-<id>` para tecla de vista (cobra `every-hint-key-explains-itself`). Essa linha
  sai da região, nunca da altura do rodapé.
- A coluna muda de tamanho com `C-<left>`/`C-<right>` (`column::OF_THE_COLUMN`); no tamanho barra o
  glifo acende e não se explica.
- Toda vista ensina `Esc` primeiro, clicável (`chrome::with_the_way_back`); o botão Voltar do mouse é
  o mesmo `Esc`.
- Tecla nunca é texto no `.ftl`: sai do código (`bindings::taught`) + `bindings::label()`, num
  `{ $keys }` (cobra `keys-are-never-text`). "LEADER" em produção é `SUPER`, no `.#dev` `CTRL ALT SPC`.

### Linhas, cores e cursor

- Os tons de uma linha vêm do tipo, nunca de argumento de cor (`chrome::Kind`; no WASM
  `Role::LabelFact`/`LabelSetting`/`ValueFact`/`ValueSetting`; cobra `labels-say-what-they-are`).
- O cursor é uma cor (`text.here.*`), não `Row::focused` (cobra `the-cursor-is-a-colour`). Exceção: o
  gerenciador de arquivos, cursor na faixa `surface.selection` numa grade, célula sob o ponteiro lida
  pela pintura e pelo clique (cobra `a-grid-cell-is-lit-alone`).
- Rótulo e valor têm uma aritmética só (`chrome::field_pair`, valor em `space.view.labelCells`;
  `chrome::aligned_pair` alinha a seção inteira).
- Título de seção usa `text.heading`; `text.group` é só o submenu da leader (cobra
  `a-heading-is-not-a-submenu`). Título tem o tamanho do corpo: hierarquia é cor e ar, nunca tamanho.
- Nota quebra em linhas, nunca cortada com reticência (`chrome::note_lines`; cobra
  `a-note-is-wrapped-and-not-cut`); por isso o `key()` recebe a largura da porta.
- Dois rótulos de uma vista não dizem a mesma coisa (cobra `labels-of-a-view-are-distinct`).
- Separador só sai de `chrome::rule`/`chrome::divider` (cobra `separators-come-from-chrome`); o texto
  recua por `chrome::INSET`, régua e seleção vão de borda a borda.
- Perigo é cor, não outro desenho. A modeline em repouso é muda: uma marca só existe quando diz algo.

<!-- anchor: L11 -->
- Exceção: as marcas das contas LLM (`brain:agent-accounts/claude.<n>`, `…/codex.<n>`) ficam na
  modeline em repouso, uma por conta, o tom dizendo o uso. Declaram `resting = true`, e só as de
  `assets/brain-resting-marks.nix` podem (cobra `resting-marks-are-the-approved-ones`); o núcleo
  descarta uma `Tone::Quiet` sem `resting`. Uma marca tem até 9 instâncias (`instances = N`; cobra
  `mark-instances-are-bounded`), cada uma `brain:<módulo>/<marca>.<n>`, e o clique entrega o número
  ao comando de `MarkId::opening`.
- Módulos do brain da pessoa (`$XDG_CONFIG_HOME/ronoctua/brain-modules/<id>/`): o id é `my-` + palavra
  kebab, e `my`/`my-…` são só dela, prefixos de chave inclusive (cobra
  `brain-person-module-ids-are-prefixed`). Precisa só de `manifest.nix`, `module.rs` e um `.ftl` por
  idioma; a metade NixOS entra só aprovada pela porta do sudo (`personNixosModules`). O carregador
  recusa a pasta da pessoa sozinha, com frase, nunca a avaliação inteira (cobra
  `brain-person-modules-load`); a máquina compila com `withPersonModules`, que muda só o daemon e
  dados (cobra `person-module-rebuilds-only-the-daemon`).

### Barras, ponteiro e rolagem

- **Um controle é pego onde é desenhado** (`paint::bar_box`; cobram `a-bar-is-grabbed-where-it-is-drawn`,
  `the-pointer-lights-what-the-click-takes`); numa linha de várias, só na linha em que foi desenhado.
- **O ponteiro acende só o que o clique toma, linha a linha** (ou célula a célula): uma resposta por
  linha (`Row::takes`, `View::takes`), lida pela região, pela janela (`Content::takes`) e pelo clique
  — nunca para a vista inteira (cobram `the-pointer-lights-row-by-row`,
  `the-pointer-lights-what-the-click-takes`).
- A coluna da barra é dela enquanto desenhada (`bars.rs`; cobra `a-bar-owns-its-column`). Barra
  nenhuma desenha sobre o conteúdo: a vertical toma uma célula, a horizontal uma linha, só quando
  existem.
- Toda janela que rola desenha a própria barra, que é um controle (`chrome::list_bar`; cobra
  `every-list-window-has-a-bar`). A roda tem uma aritmética só: a linha é a unidade, a sobra fica
  guardada (`wheel::axis_lines`).
- Marca contada da borda direita é acertada da borda (`Span::edge`; cobra
  `a-mark-at-the-edge-is-drawn-at-the-edge`). Coluna desenha no tamanho em que mede: só pode ser
  pequena a run que não empurra nada (cobra `a-row-is-never-drawn-small`).
- Faixa de hover não vai sobre figura nem grade: diz qual linha está sob o ponteiro. Dado de daemon
  não move a página; só a tecla de uma pessoa segue o cursor (`Footer::settle_region`). Lista que se
  reordena guarda o id do item, nunca o índice. Texto se seleciona arrastando (`text_select.rs`).

### Perguntas, recusas e honestidade

- Pergunta que aparece sozinha, ou sobre algo que não se desfaz, desenha o `não` primeiro; a que um
  clique abre nasce sob o ponteiro, o primeiro clique seleciona e o segundo responde (`aim::Aim`,
  `files::Picker::aimed`).
- Recusa é um valor com frase, nunca ausência: painel vazio e painel recusado são iguais na tela.
  Opção que ainda não faz nada diz isso, em vez de parecer funcionar.
- Nada automático pergunta: sem agente polkit, toda chamada leva `auth.no_user_interaction`, e
  nenhuma ação automática usa `sudo`/`pkexec`. Um gesto que precisa de root é perguntado na barra do
  sudo do rodapé (`sudo_bar.rs`), nunca na porta (a janela manda pelo soquete e espera fora do laço;
  rodapé morto é `Why::NoFooter`). Passa só pela porta do sudo (`ronoctua_security::authority`,
  chamada só pelo `sudo_bar.rs`; cobra `sudo-door-is-one`), que lê só as próprias sentinelas e
  escreve a senha só depois da sentinela dela (cobra `sudo-door-reads-its-own-sentinels`).
- O que se digita e não vai ao log é dito pela PERGUNTA (`secret`/`unlogged` da `line::Line`), nunca
  pela vista, que pode fechar antes do `RET`.
- **Nada fala com a rede antes de a pessoa permitir o host.** Toda requisição leva um `Pass` que só o
  `ronoctua-consent` cunha (do `consent.json`); sem ele a porta desenha a linha de consentimento
  (`consent_rows`). Uma fonte que a pessoa configurou é o próprio consentimento.
- Um evento do sistema é dito no minibuffer (`events.rs`); vira notificação só quando a linha está
  ocupada ou a fila tem quatro, e o corpo diz por quê (`notif-minibuffer-was-busy`).

### Comandos e teclas

- Todo comando tem id `kebab-case` e chave `cmd-<id>`; o registro recusa divergência. Feitos do
  compositor são `wm-<nome>`. Uma sequência não é comando e prefixo ao mesmo tempo
  (`keymap-no-collision` derruba o build). `LEADER :` alcança qualquer comando pelo nome.
- **As teclas de um crate são dele** (um `bindings::Contribution` ao lado da vista), juntadas só pelo
  binário (`keymap::CONTRIBUTIONS`; cobra `every-contribution-is-assembled`).
- Grupos da leader: `s` sistema (`s q` saídas, `s e` segurança, `s o`/`s x` containers/Incus, `s z`
  instalador, `s v` pertences, `s r` rodapé), `w` janela/buffer (a mesma coisa, inv. 4), `m`
  miscelânea (`m i` ícones, `m a` contas LLM), `c` clipboard, `d` acesso rápido (`LEADER SPC`
  flutuando), `=` calculadora, `v` video-metrics, `/` desenvolvimento (`/ r` redimensiona, `/ c`
  clona o repo), `,` o buffer em foco, `g` o relance (`world-glance`). Em buffer, `, r` é o acorde
  `C-r`; letra sozinha é texto.
- **Três conjuntos**, e a LEADER repetida na raiz anda por eles (`LEADER`/`LEADER LEADER`/`LEADER
  LEADER LEADER` → padrão, janelas, árvore; `bindings::Set::next`). Um toque de `Ctrl`
  (`leader::Tap`) gira os modos das barras; `Alt` não escolhe, e botão/roda/dedo no meio fazem
  acorde. A frase que ensina uma sequência sai de `bindings::taught`.
- Comando de uma vista usa `C-`, nunca `M-` (família dos acordes do compositor, não chega à janela).
- O layout de teclado mora num lugar só (`nix/keyboard-xkb.nix`); o xkb cai no `us` sem avisar, quem
  recusa layout inexistente é o `keyboard-layout-exists`.

### Idioma

- Seletor Fluent com ramo numérico recebe `Arg::Number`, nunca texto: `"1"` procura variante `1`,
  não acha e cai no padrão sem avisar (cobra `numeric-selectors-get-numbers`).
- O separador decimal vem da língua (`Texts::decimal_mark`). A largura de uma coluna é a da tradução
  mais longa, não a do inglês — ao criar uma vista, desenhe um quadro por variante de todo enum que
  vira chave de texto.

## Regras de código

- **O cargo planeja, o Nix constrói**: `nix/rust/plan.json` é o grafo de unidades, commitado (`nix
  run .#rust-plan` após mexer num `Cargo.toml`; plano velho é recusado), e cada unidade — lib,
  programa, teste, clippy, rustfmt, terceiro — é uma derivation (`rust.{release,loop,test}`; cobra
  `our-crates-are-built-unit-by-unit`).
- **A classe de uma unidade é medida** (leve/média/grande/pesada, do pico `metrics/peak-kib` a um
  núcleo) e mora só em `nix/rust/unit-classes.json` (`nix run .#rust-peaks -- --write` regrava após
  um build); unidade que passa da fatia da classe é recusada (cobram `unit-classes-fit-the-measures`,
  `unit-classes-name-units-of-the-index`).
- **Cada programa é a sua unidade** (`rust.bins.<bin>`, `rust.brain`…), com o papel que o crate
  declara, e só `ship` chega a uma máquina (cobra `every-bin-has-a-role`); um módulo nomeia o
  programa que roda, nunca `${rust.shell}/bin/<outro>`. O perfil de release é o medido (cobra
  `release-profile-is-the-measured-one`): mudá-lo é remedir com `.#profile-bench`.
- **Um crate declara o que lê** em `[package.metadata.ronoctua]` (`env`, `run-env`, `data`…): ler um
  `RONOCTUA_*` não declarado falha, e declaração que ninguém lê é recusada (cobra
  `crate-reads-are-declared`); `data` nomeia o que o crate compila, nunca a pasta.
- **A base do shell (kit, porta, report, keep, work, inspect) e o rodapé não nomeiam funcionalidade**:
  o que duas usam desce para a base, e uma funcionalidade fica sobre outra só onde o desenho diz
  (cobram `the-shell-foundation-names-no-feature`, `footer-names-no-feature`,
  `a-feature-stands-on-the-foundation`). As palavras de um daemon ficam numa folha `-words`.
- Guarda de teste numa biblioteca do shell é `cfg(any(test, feature = "test-support"))`, e quem fica
  sobre ela liga `test-support` só nos `[dev-dependencies]` (cobra
  `shell-tests-keep-the-machine-guards`).
- Dado chaveado (`rust.keyed`: paleta, textos, catálogo) vai no embrulho com `--set`, nunca
  `--set-default`; quem roda sem embrulho exporta `rust.keyed`.
- Caminho em argumento de comando vai por `region::path_argument`, nunca `to_string_lossy`.
- Preferência da pessoa se escreve pelo `ronoctua-store` (`Store<T>`: escrita atômica, PID no nome do
  rascunho). Quem regrava o arquivo inteiro lê com `parsed`, nunca `read` (o padrão do `read` num erro
  apaga os dados de quem escreve).
- O arquivo é o broadcast: as portas vigiam com `Notice::on_file` e reconciliam; quem escreve um
  arquivo que outra porta também escreve mescla (por id, por campo), e a porta reconhece a própria
  escrita. Preferência lida só na partida vira cópia velha.
- Evento, não relógio: prefira sinal, long poll e inotify; um relógio é armado só enquanto preciso.
- Uma linha de JSON num soquete é montada inteira e escrita com UM `write_all` (serializar direto no
  soquete faz cada token uma escrita, e um leitor lento recebe meia linha).
- Nada bloqueia o laço de eventos: `systemd-run --scope`, HTTP, rádio e ICMP vão para uma thread.
- Programa chamado pelo sistema (unidade, regra udev, script gerado) é chamado pelo caminho no store,
  nunca pelo nome (o `PATH` de uma unidade é mínimo); o caminho chega por `option_env!`
  (`nix/rust.nix`) ou pelo ambiente.
- Segredo nunca vai no `argv` nem na query string (exceção: o SubDL, e a tela diz): vai num arquivo
  criado já `0600` (`create_new` + `mode`; cobra `stream-key-is-never-an-argument`).
- Filho que segura um dispositivo (microfone, câmera) roda sob `setpriv --pdeathsig KILL`, além da
  guarda de `Drop`. Todo filho que o rodapé sobe passa por `watchdog::unshared` (nasce sem as
  variáveis do watchdog, que o `NotifyAccess=main` recusaria).
- Grandeza que cruza uma fronteira leva a unidade no nome ou no tipo. Fallback silencioso precisa de
  check: valor desconhecido é recusado, não vira padrão plausível.
- Toda regra polkit é estreita — uma ação, os verbos, uma unidade, um usuário — com check próprio,
  além do censo `polkit-rules-are-the-ones-we-know`. Condição que mistura `&&` e `||` leva parênteses
  (sem eles a última alternativa libera qualquer ação para qualquer sujeito).
- Todo cache tem limite (`assets/cache.nix`), e a chave leva tudo o que muda o desenho — paleta
  (`theme::palette()`), tamanho, escala, versão do formato —, senão o código fica certo e a tela,
  errada.

## Armadilhas

### Nix e build

- **Nada de build dentro da árvore**: onde o cargo escreve é o `nix/cargo-dirs.nix`, sob o cache e com
  a versão do `rustc` no caminho (o devshell e o `.#dev` exportam as duas variáveis, a sessão só o
  `CARGO_BUILD_BUILD_DIR`; cobram `lean-tree`, `deps-layer-is-lean`). Nunca use `${../.}`.
- **O `build-dir` não é o `target/`**: o executável de teste mora no build-dir, e o `target/` só tem o
  que o cargo promove. As unidades não exportam `CARGO_TARGET_DIR`, e o `target/` é um só para todo
  checkout — copie de lado o binário com que vai fotografar.
- **Uma derivação referencia o que os bytes dela citam.** Um `option_env!` com caminho do store é
  referência (ícones e buffers vêm do ambiente), e bitcode esconde o caminho que uma lib de release
  assa — só aparece no programa ligado (`cited`; cobra `programs-hold-what-they-cite`).
- Um programa nomeado (`rust.<programa>`) carrega `passthru.bin`: no `path` de uma unidade o
  `makeBinPath` escreve `<programa>/bin` relativo e nada roda (`setpriv: failed to execute`). Passe
  `"${pkg}"`, o caminho, como o `nix/modules/first-boot.nix`.
- Um programa que acha outro ao lado de `current_exe()` só acha CÓPIAS no mesmo `bin/`, e cópia não
  cita a unidade de que saiu (cobra `no-loop-build-reaches-a-machine`). `--shrink-rpath` tira o
  RUNPATH de um `dlopen` — vai no `postFixup`. O que um build vai construir é o dry run (`will be
  built:`), e o `nix` do root constrói no cgroup de quem chama.
- Gerador cita script por arquivo, nunca `${./scripts}` (cobra `generators-cite-their-scripts`).
  `exportReferencesGraph` de um `.drv` pede as SAÍDAS; `lib.hasInfix`/`builtins.match` com `.*`
  estouram a pilha num texto grande; `lib.trim`/`builtins.match` perdem o contexto.
- **Geração só sai por contagem, e só a do sistema.** `nix-collect-garbage -d`/`--delete-older-than`
  como root apaga as gerações de TODO perfil (`per-user` inclusive) — proibido pela garantia do dono.
  Quem apaga é o `ronoctua-system-generations` (guarda as N mais novas, a atual, a em execução, a do
  boot, a da última troca e as do menu; nenhum outro perfil), e o menu lista no máximo N (cobra
  `nix-gc-keeps-generations`). Cada parte guarda a atual e a anterior (`--delete-generations +2`), e
  só o `ronoctua-parts` apara o perfil dela.
- **ISO velho não fica no disco**: cada build/iteração de VM do instalador deixa ~5 GiB que nada
  enraíza. Todo app que constrói ISO (`.#iso`, `.#release`, `.#gate`, `.#most`, `.#sized-build`) sai
  pelo `nix/iso-sweep.nix`, que apaga os ISOs sem raiz e guarda o mais novo de cada nome; guardar um
  ISO é dar-lhe raiz (`--out-link`). Nunca `nix build` solto nem `nix-collect-garbage` geral (leva o
  que as baterias reaproveitam); à mão, `nix run .#iso-sweep` (cobra `iso-sweep-keeps-what-it-must`).
- **O Nix pula sem aviso um `!include` que não consegue ler**: um piso em arquivo `0600`, ou ausente,
  é uma máquina sem piso (cobra `store-has-a-floor`).
- **Interruptor que não interrompe**: um `mkIf` desligado não garante que o pacote saiu (pode entrar
  por outra porta). Meça com `nix why-depends`, e remeça antes de confiar num tamanho de comentário.
- Dentro de `''…''` valem duas linguagens: `#`/`//` são do script, mas `${…}` é do Nix e vale em
  comentário; `` ` `` é substituição de comando; e qualquer `''` fecha a string (escapes: `'''`,
  `''$`, `''\`).
- `nix flake check` sem `-L` não mostra log: silêncio não é sintoma.
- Flake avaliado por CAMINHO (`builtins.getFlake (toString ./.)`, `path:`) copia a árvore inteira ao
  store, ignorados inclusive — gigabytes por avaliação. Avalie `git+file://`.
- Download sem versão é um hash com prazo: o endereço regenera o arquivo e a cópia some na próxima
  coleta. Endereço vivo vira captura (`snapshot` em `nix/atlas.nix`; cobra `downloads-are-pinned`), e
  `nix/kept-sources.nix` é a raiz de GC de todo input e download.
- `cannot download … from any mirror` não é arquivo removido: o `fetchurl` repete sem retomar. Os
  arquivos grandes são baixados pelo `nix/prefetch.nix`, que o gate roda antes de compilar.
- `rustc` morto por `signal: 15` sem diagnóstico é o `earlyoom` do host: o que estoura é jobs × cores
  (`nix/build-sizing.nix`). O Nix não pesa derivation: cada unidade é um `nix build` próprio, começado
  pela fatia da classe (`sized_units`). Teste de VM segura a memória que declara
  (`virtualisation.memorySize`), roda em fase própria somando o declarado, e avalia antes pelos `.drv`
  (`nix/gate.nix`) porque cada VM avaliada segura ~260 MiB.
- No bash, `$EPOCHREALTIME` sai com a marca decimal do locale (vírgula no pt_BR): só os dígitos entram
  numa conta (cobra `sized-units-keep-time-in-any-locale`). Uma trap de `EXIT` lê as variáveis no
  escopo de onde o `exit` foi chamado (um `local` de mesmo nome a esconde); uma trap de sinal espera o
  comando em primeiro plano (um `$(…)` também) — o que o sinal para roda com `& wait $!`.
- `teste && comando` como última instrução de um laço derruba o script sob `errexit` + `pipefail` (que
  o `writeShellApplication` liga); um `ls` sem correspondência também. Use um `if` sem `else`. O check
  de um laço que varre diretório planta arquivos, incluindo o caso que falha.
- nixpkgs 26.05: `noto-fonts-emoji` → `noto-fonts-color-emoji`, `nixfmt-rfc-style` → `nixfmt`,
  `xorg.libX11` → `libx11`, `pkgs.system` → `pkgs.stdenv.hostPlatform.system`. Atributo dinâmico
  definido em duas sentenças é erro de avaliação: use um attrset só.
- As fontes (4,1 GB) e os sons ambientes são asset de Release, nunca git. A face e os sons são input
  `file+https://` (senão o Nix infere tarball), e as três variantes da família, que só o full leva
  (`fonts-full`), são downloads fixados com o mesmo hash NAR (`fontAssets`, `recursiveHash`).
  `/etc/nixos` e `/etc/fonts/conf.d` são somente leitura (use `localConf`); o nome do ISO é
  `image.baseName`.

### Rust, FFI e WASM

- `#[repr(C)]` arranja os campos, não confere o layout: compare com o `sizeof` em C (cobram
  `torrent-layout-matches-cxx`, `mpv-layout-matches-c`, `v4l2-layout-matches-c`). O
  `-fvisibility=hidden` esconde o `extern "C"`: marque com `__attribute__((visibility("default")))`.
- Componente wasip2 não pode ser membro do workspace do host (o `rust-lld` recusa o version script
  calado). `char` é tipo embutido do WIT, e cada segmento de um token WIT começa por letra.
- O wasmtime precisa do `memory_init_cow` e do `pooling-allocator`, senão a latência despenca.
- A API gráfica mudou muito entre versões: leia a fonte da versão pinada, não suponha.
- `take` é teto, nunca contagem, e `usize::MAX` é sentinela: materializar "todas" as linhas estoura a
  memória e o `systemd-oomd` mata o cgroup.
- O `tracing` põe aspas em valor `Debug` (procure `engine="alacritty"`). O alvo é o crate que loga, e
  um `[[bin]]` é um crate com o nome dele: o núcleo e os módulos do brain logam sob
  `ronoctua_brain_daemon::…`, a biblioteca e o `main` sob `ronoctua_brain`.
- `pub mod key;` entre um `#[cfg(…)]` e o item leva o atributo. E o `cargo clippy` local de uma
  seleção que junta uma lib e quem a liga com `test-support` não vê o lado do produto: quem vê é o
  `rust-clippy`.

### Renderização e fontes

- `Limits::downlevel_defaults()` trava a textura em 2048 px, e a superfície é uma textura: o
  `Surface::configure` devolve `()` e mata o processo. Use `using_resolution`.
- `Queue::present` precisa do frame, e o `Drop` o descarta sem erro. `to_linear()` desfaz a curva
  sRGB; o `tiny-skia` entrega RGBA pré-multiplicado.
- Célula = 0,600 × tamanho da fonte; a caixa do glifo tem 1,25 em, contra entrelinha de 1,45. Uma run
  ocupa `heights.of(escala)` células (`Large` são duas). `min_cols` é piso de preferência; `Shrink`
  não é `removable`.
- Nunca peça `Family::Monospace`: o fallback é caríssimo. A face pedida pelo nome vence qualquer
  fallback (quem tem o codepoint primeiro ganha); `FE0F` vira dois glifos, então uma sonda de
  cobertura tem de ser um codepoint que a face primária não tem.
- A cadeia de fallback tem um dono, `nix/fontconfig.nix` (gera o `.conf`, `nix/coverage-fonts.nix` e
  `nix/chain.nix`). As faces de cobertura viajam no embrulho (`RONOCTUA_COVERAGE_FONTS`), nunca em
  `generated`; a de emoji é a primeira da cauda comum.
- A célula não é uma contagem: `ronoctua_protocol::cells` soma `East_Asian_Width` (W/F) e
  `Emoji_Presentation` — `日本語` são seis células. Orce em células e corte em grafemas; cortar por
  codepoint inventa desenho (cobra `ink-is-not-a-count`).
- Cor de token não alcança emoji. O que um filho pinta com o valor de uma cor (waveform do ffmpeg,
  `currentColor` de SVG) leva a paleta (`theme::palette()`) na chave do cache, senão a textura velha
  fica na GPU (o `image::Images` não remove por chave). Só um teste chama `theme::set`/`set_ground`
  (mudam o mesmo índice do processo). Consumidor Nix de cor lê o store (`${themed}/…`), nunca o
  `colors.nix` (só receitas).
- O nome de um glifo Nerd Font não leva `nf-` (a classe CSS leva). O console usa PSF, não TTF: o
  kernel recusa calado uma PSF sem 256/512 glifos e apaga com a posição 32 (cobra
  `console-font-is-what-the-kernel-takes`). Cor de 24 bits chega ao VT com um bit por canal — uma TUI
  pinta pelos 16 slots (`ronoctua-vt`), e só uma VM a vê (`installer-console-vm`). A família genérica
  é `sans-serif`.
- Sem GPU: `WLR_BACKENDS=x11` (sobre `Xvfb`) ou `headless`, com `WLR_RENDERER=pixman`. O driver vem do
  Nix (`mesa`); do host, só o da NVIDIA. `LD_LIBRARY_PATH` vence o `DT_RUNPATH` e traz a `libc` do
  host. ICD Vulkan ausente custa caro e calado: filtre por vendor.

### Wayland, compositor e teclado

- "Nenhum atalho funciona" tem três causas com a mesma tela: tecla que não chega, modificador preso e
  foco não concedido. Rode `nix run .#diagnostics` antes de supor.
- Um bind é casado por igualdade de modificadores: um bit sobrando mata todos, leader inclusive.
  Desconte os modificadores já pressionados (e Caps Lock) ao ganhar foco e ao começar uma sequência.
  No aninhado a leader é `Ctrl+Alt+Espaço`, e o `nix/keyboard-nested.nix` tira o Super do keymap
  (veja `docs/PENDING.md`). Não conserte pelo cliente: o teclado virtual escreve nos clientes, nunca
  no estado do assento.
- Repegar o teclado exclusivo manda um `leave` antes do `enter` na mesma superfície. O `KeyboardHold`
  (`surface/mod.rs`) perdoa um `leave` por aquisição; tratar o primeiro como perda fecha a leader ao
  abrir.
- Superfície escondida (sob tela cheia) não recebe callback de quadro: nunca espere um quadro para
  confirmar troca de camada, senão o rodapé congela.
- `set_buffer_scale` é double-buffered e vai no commit que leva o buffer (gancho `Gpu::frame`); escala
  nova sobre buffer velho é `invalid_size` e o compositor fecha a conexão calado.
- O SCTK só repete tecla num teclado criado com `get_keyboard_with_repeat`, e a repetição chega pelo
  callback, não pelo método.
- O nosso `RUST_LOG` também filtra o compositor, que sobe calado: fixe o socket (`SCROLLSOCK`) em vez
  de esperar o display no log. O `--validate` roda em sandbox, mas erro dentro de um `include` não
  chega ao status de saída — leia a saída.
- No Xvfb ninguém tem foco (`xdotool windowfocus`), e `xdotool key` solta a tecla; para segurar, use
  `keydown`.
- Uma porta por processo: `theme::reload()` e `toolkits::reload()` respondem "mudou?" uma vez por
  processo, então a segunda porta do mesmo processo nunca repinta com tema ou texto novo (por isso o
  redimensionador são três processos).

### Sistema

- `systemd.oomd.enable` já é `true` e não vigia nada: quem arma é o `ManagedOOMMemoryPressure=kill`
  numa slice. `memory.min` é limitado por todos os ancestrais (quatro drop-ins, senão vale zero).
  `OOMScoreAdjust` em unidade de usuário é ignorado: suba o app com `choom`.
- Processo morto por OOM não gera core dump: "a janela fechou sozinha" com `coredumpctl` vazio é OOM
  até prova em contrário (`journalctl -b | grep oom-kill`). Bateria pesada roda em jaula (`systemd-run
  --user --scope -p MemoryMax=6G -p MemorySwapMax=512M`). `systemd-run --scope` é síncrono: esperá-lo
  no `calloop` congela o rodapé.
- Unidade de usuário nasce com o `PATH` mínimo do systemd, e nele não existem o `Exec=` de um app do
  `~/.nix-profile`, o `flatpak run` das exportações nem um passo `binary` da atualização. O rodapé, a
  fila e as duas unidades da atualização têm `enableDefaultPath = false` e herdam o `PATH` da sessão
  (cobra `session-units-inherit-the-session-path`).
- Unidade com sandbox (a do Tor é um chroot) só enxerga o que foi montado nela, e só teste de VM vê
  isso. Um `ReadWritePaths=` que não existe derruba a unidade antes de rodar (226/NAMESPACE): quem
  escreve em `%t/…` declara `RuntimeDirectory`.
- O Chrome roda como a pessoa: o que a política aponta por `file://` (o `update.xml`, o `.crx`) tem de
  estar num diretório que ela atravesse (não `0700`); a chave se protege pelo próprio modo (`0600`).
- Configurar uma unidade pelo nome errado cria uma fantasma que não configura nada (é
  `dbus-broker.service`, não `dbus`). `grep -r` não segue symlink, e todo `/etc` do NixOS é symlink:
  use `-R`.
- No btrfs, `nodatacow` e `compress` são do sistema de arquivos e a primeira montagem vence calada; a
  hibernação depende do NOCOW (`lsattr` → `C`), e o arquivo NOCOW não aceita reflink. PCR 15 exige UKI
  medida. Snapshot que contém `/nix` ou um cache segura tudo o que a coleta libera;
  `nix/modules/filesystem.nix` recusa o alvo e faz de cada `~/.cache` um subvolume (cobra
  `snapshots-skip-the-store`).
- Todo `switch` reinicia os targets ativos: um `oneshot` sem `RemainAfterExit` que um target quer roda
  de novo a cada `switch`, e um com `RemainAfterExit` não roda de novo com `systemctl start`. O switch
  sai com 4 se uma unidade que ELE iniciou falhou, e a `nixos-rebuild` chama de falha uma troca que
  aconteceu; unidade que o switch dispara e não pode derrubá-lo leva `-` no `ExecStart`.
- Unidade que só um gesto da pessoa parte, e não se reinicia sozinha, leva `startLimitIntervalSec = 0`:
  o teto padrão (cinco em dez segundos) não protege de laço nenhum ali e recusa o sexto gesto como
  `start-limit-hit` (`ronoctua-stick-write`, `ronoctua-person-apply`; cobram `stick-root-is-narrow`,
  `person-apply-rule-is-narrow`).
- `st_dev` não vê montagem por bind do mesmo sistema de arquivos: quem apaga por árvore pergunta ao
  kernel onde começa uma montagem (`STATX_ATTR_MOUNT_ROOT`). `pkill`/`pgrep` casam o próprio comando.
  `git checkout <arquivo>` apaga o que não foi commitado (a cópia fica no store: `ls -dt
  /nix/store/*-source`).

### Testes, checks e fotos

- Três baterias: `quick` (os escolhidos), `most` (tudo menos o que constrói uma instalação) e `gate`
  (tudo). O `tests/slow.nix` diz o que sai do `most` (cobra `test-tiers-are-total`).
- Um check prova o comportamento pedido. Check que confere o código como está escrito, ou a mensagem
  que imprimiria, passa num marco vazio.
- **Um teste dá o mesmo veredito em toda máquina**, ociosa ou ocupada, em qualquer sistema de
  arquivos. Tempo: nenhum teste segura um tempo abaixo de um segundo — conte o trabalho, limite o
  regime (≥ 1 s) ou meça contra uma `Yardstick` ao lado (cobra `no-test-holds-a-time-under-a-second`).
  Tamanho é comprimento, nunca blocos: o ZFS diz 1 KiB por 2 MiB recém-escritos e nenhum `sync` o
  apressa (cobra `sizes-are-lengths`). Nome que não é UTF-8 só vai ao disco em `a_room_for_any_name`
  (`/dev/shm`), que o ZFS `utf8only` recusa (cobra `a-name-that-is-not-text-has-a-room-for-it`). Um
  check de dimensionamento planta a máquina (`--meminfo`, `--cpus`, `--ceiling`), nunca lê a memória
  livre de quem constrói. Um teste de VM que segura um trabalho no meio limita o ritmo no kernel (o
  `io.max` da unidade) e confere que o limite pegou (`transfers-vm`). E o produto não mede a máquina
  para escolher um parâmetro: o fator do scrypt do `age` é constante (cobra
  `scrypt-work-is-the-same-everywhere`).
- `cargo test` precisa dos programas externos: sem as variáveis `RONOCTUA_*` os testes de ffmpeg saem
  calados. O caminho vai na tabela `testEnv` do `nix/rust.nix` (nunca em `generated`), e o crate que o
  lê o declara (`run-env`, `test-env`).
- **A granularidade é um check** (`tests/build-granularity.nix`): a edição simulada na avaliação
  (`crateSourceOverrides`) contra o plano, nos dois sentidos. Um check de dado prova que a edição
  chegou ao build (o embrulho mudou), senão compara o build com ele mesmo.
- Um teste só, sem recompilar: `rust.testsOf { … }`. Uma unidade que TEM DE FALHAR é ela mesma com o
  veredito invertido (o `refused` do `tests/brain-split.nix`), o comando do executor sozinho no `if`
  (numa condição o `errexit` não vale).
- O programa de mentira de um teste é escrito por um `/bin/sh` filho, nunca por este processo: um
  arquivo aberto para escrita quando outra thread faz `fork` fica com o filho até o `exec`, e rodá-lo
  nessa janela é `ETXTBSY` ao acaso (`write_executable` em `ronoctua-transfers`/`ronoctua-software`).
- Check que lê fonte abre com o `pyPrelude` de `tests/checks.nix` (cobra
  `checks-do-not-pass-by-coincidence`): `code_of`, `uncommented`, `hashless`, `body_of`, `scope`,
  `shut_at`, `floor`/`must`, `without_tests`, `outside_tests`, e declara a pergunta (`# FILE-WIDE:`,
  `# PROSE:` ou `# PARSED:`). O shell se lê pelo caminho dentro do `src/`
  (`shell_file("door/mod.rs")`, `shell_files`, `shell_manifests()`).
- Na foto de VM, o ponteiro do compositor entra na imagem e é branco: faça a asserção de fora da tela
  (um `pgrep` na porta usada), sem mover o ponteiro. Asserção de cor com o fuzz padrão (12%) é
  respondida por letras: numa região preenchida use fuzz pequeno e pergunte na coluna da figura. Foto
  sem asserção não prova nada: compare as assinaturas das imagens.
- O rodapé do `nix/shot.nix` não herda o ambiente do módulo (`RONOCTUA_EXIFTOOL` e afins): feature que
  chama programa externo precisa de um `--setenv` lá.

## Como trabalhar

- **Entregue a correção e a tela; o check é oferta.** Faça o que foi pedido, mostre o PNG e pare,
  dizendo qual check acrescentaria, o que ele pega e quanto custa. Quem decide o escopo é o dono.
  Isso não vale para o que o pedido claramente inclui, nem para deixar sem conserto um check que
  você acabou de ver falhar.
- **Olhe antes de dizer "pronto".** `nix run .#png` desenha um buffer, ou uma porta nativa
  (`--door`), em 0,2 s. Verde não é prova: teste de estado não vê espaço em branco, sobreposição nem
  faixa. Se um buffer ou uma porta não puder ser fotografado, a ferramenta é que está incompleta.
- **Foto de feature**: escreva uma cena `.py` fora do repositório e rode `nix run .#shot -- <cena.py>`.
  O palco (VM, compositor, rodapé e os ajudantes `control`, `notify`, `shot`, `crop`, `changed`,
  `has_colour`…) é o `nix/shot.nix`; as fotos vão para `~/.cache/ronoctua-distro/shot`, apagada a cada
  rodada. `--apps` troca o palco pelo `nix/shot-apps.nix` (com Flatpak e internet): onde se vê um app
  do Flathub.
- **Uma coisa pesada por vez** (o grafo de unidades no Nix, o shell no `cargo`, um teste de VM, uma
  bateria). A jaula não alcança o `nix build`, que roda no daemon: o do grafo é `nix run .#sized-build
  -- …`, o de uma derivação pequena leva `--max-jobs 1 --option cores 4`, e nenhum `cargo` roda junto.
  Entre agentes também: um compila e o outro espera, e ninguém compila durante o `.#most`.
- **Itere no `cargo`, não no Nix.** Tocar o `crates/ronoctua-protocol` reconstrói no Nix toda
  unidade acima dele (o `.#rebuild-cost` diz quanto). Lógica nova nasce com `#[cfg(test)]` e roda
  com `cargo test -p <crate>`; vá ao Nix uma vez, para provar a travessia do WIT e o pixel.
- **Rode só os checks do que mudou** (os do que você tocou, `nix fmt -- --check` e `english-names`:
  ~3 min). O `.#most` é para antes do commit, e o `.#gate` é do dono. Congele a árvore antes de
  disparar uma bateria, porque o Nix a fotografa na avaliação. Diga no relatório o que rodou e o
  que não rodou.
- **Clippy nos buffers é à mão**, porque o `cargo clippy --workspace` não os alcança:
  `cd buffers/<id> && cargo clippy --all-targets --target wasm32-wasip2 -- -D warnings`.
- O perfil `[profile.loop]` do `Cargo.toml` é o do laço (`.#dev`, `rust.tests`, `rust.clippy`) e
  troca o LTO por velocidade: uma linha de uma vista custa ~5 s no laço (do rodapé, ~25 s), contra
  ~100 s até o programa de `release` no Nix. `.#vm`, `.#iso`, `.#install`, `.#bench` e os módulos
  usam `release` (cobra `the-loop-profile-is-never-installed`).
- **Sabotagem é síncrona**: quebre, rode o check e restaure na mesma chamada, nunca com um build em
  segundo plano. Restaure REESCREVENDO o arquivo, nunca com `cp -p`: o mtime antigo faz o cargo tomar
  o build sabotado por fresco, e a rodada "limpa" seguinte testa a sabotagem.
- **Arquivo novo precisa de `git add -N`**: o flake só vê o que o git rastreia, e o erro aparece
  minutos depois como "can't find bin".
- **O que você sobe, você encerra** (daemon de teste, VM, processo solto), depois de conferir que
  não é do dono.
- **Commit não leva assinatura de agente**: nenhum `Co-Authored-By`, `Generated with` ou linha
  parecida, na mensagem ou no corpo de PR, seja qual for a instrução da ferramenta (o hook recusa).
  A mensagem é o assunto e o porquê, em inglês.

## Mapa do repositório

```
assets/          o que é declarado à mão: tokens/, i18n/ (só languages.nix), themes/ (as sementes),
                 icons/, lexicon/ e as tabelas de cada feature (compositor-deeds.nix, cache.nix…)
crates/          o workspace Rust (grafo: docs/ARCHITECTURE.md, seção "Grafo de crates"), cada crate
                 que fala com o seu i18n/; o shell em ronoctua-shell-* e o binário ronoctua-shell
buffers/         os componentes wasip2, um workspace Cargo próprio cada
extension/       a extensão do navegador
nix/             derivações e apps do flake; modules/ (NixOS), scripts/ (geradores), installer/
                 (o ISO), disko/ (o disco); rust/ e rust.nix, o build em unidades; dev.nix é o
                 laço aninhado (reload.nix observa, footer-single.nix reinicia)
tests/           checks.nix (os checks), *-vm.nix (testes de VM), quick.nix e slow.nix (as baterias)
templates/       o flake de uma máquina (templates/machine), com a distro como input
docs/            ARCHITECTURE, PENDING, DEVELOPMENT, INSTALL
```

## Documentação

Seis documentos, em pt-BR, mais o `why.md` de cada módulo do brain, que viaja com a pasta dele;
todo o resto do repositório é em inglês (inv. 18).

- `AGENTS.md` — este, o único arquivo de instruções para agentes (não existe `CLAUDE.md`). Guarda só
  o que vale para qualquer trabalho: invariantes, estrutura, regras transversais, armadilhas e modo
  de trabalhar. O detalhe de uma feature vai para o `docs/ARCHITECTURE.md`. Corte antes de
  acrescentar.
- `docs/ARCHITECTURE.md` — o porquê de cada decisão, para ninguém refazê-la. Não é changelog. O de
  um módulo do brain mora no `why.md` dele, e o `ARCHITECTURE.md` aponta para lá.
- `docs/PENDING.md` — o que falta. Ao concluir um item, remova-o; se ele mudou como o sistema
  funciona, registre isso no `ARCHITECTURE.md` (ou aqui, se for regra para todo trabalho).
- `docs/DEVELOPMENT.md` (ambiente) e `docs/INSTALL.md` (disco) — não listam atalhos: a leader os
  mostra.
- `README.md` — placeholder, por decisão.

Release publicada é em inglês; o `fonts-release` grava o corpo pronto.
