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
8. **Zero string literal em UI.** Tudo por `text-key`; `pt-BR` e `en-GB` completos.
9. **Zero valor visual hard-coded** — cor, fonte, tamanho, espaçamento, codepoint: tudo por token.
10. **Ícones da UI são glifos da fonte.** O superpacote é só fallback de pixmap para apps
    estrangeiros; nenhum componente do design system o consulta.
11. **Buffers WASM não tocam Wayland, não recebem valores de token, não traduzem.**
12. **`ronoctua-design` e `ronoctua-glyphdb` compilam para `wasm32-wasip2`.**
13. **Nenhum dado pessoal** — nem `host.nix`, `facter.json`, `~/.shellrc.local`: nascem NA máquina.
14. **Nenhuma chrome além do rodapé de duas linhas.** Sem barra, relógio, taskbar, menu iniciar,
    dock, ícone de desktop ou popup flutuante. Tray e notificações vivem na modeline.
15. **Sem cards, sem sombra, sem elevação.**
16. `noctua` e `ronoctua-distro` definidos em **um único lugar** cada — `nix/modules/base.nix`.
17. **Sem Docker.**
18. **Todo NOME, COMENTÁRIO e COMMIT são inglês** — arquivo, comando, check, chave, `[[bin]]`,
    identificador, `//`, `#`, docstring. pt-BR só nos 6 docs, no VALOR de chave e na **tag de busca**.

Ao bater num invariante: **pare e reporte.** Não contorne, não peça exceção implícita.

## Comandos

```
nix develop           entra no ambiente (direnv é atalho opcional, e do HOST)
nix run .#dev         niri aninhado + rodapé; leader = Ctrl+Alt+Espaço, não Super
nix run .#quick       DEZ escolhidos (tests/quick.nix), ~1 min; subconjunto, não categoria
nix run .#most        os que não constroem instalação — a bateria de todo dia
nix run .#gate        TUDO; dimensiona pela RAM e acusa o earlyoom. Só ele fecha o inv. 7
nix run .#vm  VM completa; .#iso o ISO; .#bench falha se estourar; .#diagnostics só lê
nix run .#install     instala (ensaia sem --execute);  ronoctua.profile: full ou minimal
```

Flake apps, nunca justfile nem Makefile. Passo que precise de instrução escrita devia ser automático.

## Como funciona

- **O rodapé é o desktop.** `wlr-layer-shell` embaixo: modeline e minibuffer COLADAS, expansão com teto e página, e nenhuma chrome (inv. 14). O ponteiro alcança as DUAS linhas, e clique só faz o que tecla faz.
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
  `Backspace` vazio SOBE; o direito só FOCA, e a linha na TELA não é a da listagem. Permissão
  PERGUNTA nos DOIS caminhos. **Rótulo de `Choice` é TEXTO, não chave**; texto abre no `emacsclient`.
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
- **`LEADER r` é rede e Syncthing.** Só o `ronoctua-network` fala NetworkManager; `overrideFolders`
  FICA `false` (`true` apaga as pastas a cada rebuild). A senha do Wi-Fi não fica em arquivo:
  `psk-flags` mais o agente entregando do TPM2 (**sem PCR** — com PCR, atualizar o kernel a quebra).
- **Tokens e textos são GERADOS do Nix** (`assets/tokens/*.nix`, `*.ftl`): chave ou cor inexistente é
  erro de **compilação** — inv. 8 e 9. **Layout é aritmética de inteiros**, em duas fases:
  preferência (`min_cols`), e "nada vaza da tela".
- **Buffers nativos são componentes wasip2.** Devolvem display list; rasteriza o `ronoctua-render`,
  posiciona o niri (inv. 4); não conhecem Wayland, cor nem idioma (inv. 11). Estado vive na
  `Instance`. Cinco primitivas: texto, régua, grade, seleção e **`svg`** (TRAVA `vector`), e **campo
  de record cruza por POSIÇÃO** e só depois pelo nome.
- **`LEADER s m` é o painel de memória**, e a capacidade `system` é a trava: o host amostra
  (`ronoctua-memory`, única fronteira com `/proc`) e manda `input::survey`; o buffer desenha e pede
  por `effect::ask`. Relógio de 1 s lê pseudo-arquivo; varredura e `systemctl` vão para thread.
- **Duas instalações, uma opção só.** `ronoctua.profile` = `full` (22,8 GiB) ou `minimal` (6,9);
  `nix/modules/profile.nix` é a tabela, tudo `mkDefault`. **O ISO leva no `storeContents` o que
  nenhum cache tem** (shell, niri, ghostty-vt, buffers, fontes): o alvo baixa, não compila.

## Mapa do repositório

```
assets/tokens/  tokens em Nix (verdade);  assets/icons/  app id e categoria → glifo;  assets/lexicon/  à mão
assets/i18n/  pt-BR e en-GB, todo texto de UI (inv. 8);  nix/fonts.nix  5 derivações + as 27 variantes + PSF
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
- **Chave de texto tem SETE derivados**, só um é o `.ftl`; **pergunta do instalador tem 13** — os dois no ARCHITECTURE. **`to_linear()` desfaz a curva sRGB**; **célula = 0,600 × fonte**; **caixa 1,25 em** contra entrelinha 1,45. `tiny-skia` entrega RGBA PRÉ-MULTIPLICADO.
- **Fallback silencioso precisa de check**: a tabela de tray era validada no build e não chegava ao
  Rust. **Cortar por CODEPOINT inventa desenho**: elide por CLUSTER. **Cor de token não alcança emoji**.
- **`Limits::downlevel_defaults()` trava a textura em 2048 px** e a superfície É textura: `Surface::configure` devolve `()` e **mata o processo** — use `using_resolution`. **`systemd-run --scope` é SÍNCRONO**: no `calloop` de uma thread, esperar congela o rodapé. **Check contra o CÓDIGO, não o CRITÉRIO,** fecha marco vazio.
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
- **Spawn em unidade do systemd: use CAMINHO, nunca nome** — o `PATH` é mínimo e a falha aparece longe da causa. O caminho do store vem por `option_env!`, posto no `nix/rust.nix`. E **`min_cols` é piso de PREFERÊNCIA**; o que não cede é "nada vaza da tela". `Shrink`≠`removable`. **Upstream do Niri: `niri-wm/niri`**, por calendário — pinar tag, nunca `main`. Blur da 26.04 não.

## Documentação — seis arquivos, em pt-BR (o resto é inglês — inv. 18)

- `CLAUDE.md` — este. **Teto de 150 linhas** (`docs-exist`); corte antes de acrescentar. Sem log de
  progresso, changelog nem narrativa. `docs/ARCHITECTURE.md` — o porquê do que não se deve refazer.
- `docs/PENDING.md` — o que falta. **Ao concluir, remova de lá**: se mudar como o sistema funciona,
  escreva uma linha aqui ou no ARCHITECTURE; se não, só apague. `README.md` é placeholder, por decisão.
- `INSTALL.md` (disco) e `DEVELOPMENT.md` (ambiente): NÃO liste atalho — a leader os mostra. **Release
  publicada é em INGLÊS**; o `fonts-release` grava o corpo pronto.
