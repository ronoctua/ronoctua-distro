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
nix run .#quick       os checks que não sobem VM (~1 min); NÃO fecha nada, só adianta
nix run .#gate        verifica TUDO — dimensiona pela RAM e acusa o earlyoom; USE ESTE
nix run .#diagnostics instantâneo de quando os atalhos travam (só leitura)
nix run .#vm          VM completa;  .#iso gera o ISO;  .#bench falha se um alvo estourar
nix run .#install     instala (ensaia sem --execute);  ronoctua.profile: full ou minimal
```

Flake apps, nunca justfile nem Makefile. Passo que precise de instrução escrita devia ser automático.

## Como funciona

- **O rodapé é o desktop.** Superfície `wlr-layer-shell` ancorada embaixo: modeline e minibuffer
  COLADAS, expansão (which-key, localizador, manual) abaixo, com teto e página. Sem chrome (inv. 14).
- **A fronteira com o niri é só IPC.** `ronoctua-niri` é o único crate que fala `niri-ipc`.
- **A leader vem de FORA.** Em repouso o rodapé não captura teclado — senão nenhum buffer receberia
  tecla. Quem a vê é o niri, que roda `ronoctua-shell leader`; ele escreve no socket de controle
  (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`) e só então a superfície expande e pede
  `keyboard-interactivity: exclusive`. `ronoctua-shell command <id> [arg]` roda qualquer id — é por
  onde entram os QUATRO binds SEM leader (`Ctrl+Alt+L`, `Ctrl+Shift+L`, `Ctrl+Alt+Esc` o monitor).
- **`Ctrl+Alt+Del` é o RESGATE, não reinicia mais** (7 toques em 2 s ainda forçam): binário de
  TERMINAL, duas portas — janela flutuante, ou VT 8 sem niri. Ver é livre, agir é polkit.
- **Todo comando é `kebab-case`, e a chave de texto é `cmd-<id>`**; o registro recusa as duas
  divergirem. As 141 ações do niri entram como `niri-<nome>`, GERADAS do binário pinado, e são a
  exceção a "todo comando tem tecla" — alcançam-se por nome (`LEADER :`).
- **O teclado tem UM lugar** (`nix/keyboard-xkb.nix`): compositor, aninhado e console. O xkb **não
  recusa** layout inexistente — cai no `us` mudo; quem impede é `keyboard-layout-exists`.
- **Três navegadores, um léxico** (`i n`, `i u`, `i l`). Só o da Lucide personaliza; a prévia é o SVG
  COPIADO, e a marca sai da MESMA `affordance` que a tecla lê.
- **O localizador (`a`) é a SEXTA vista e a rede (`r r`) a SÉTIMA**, não menus (inv. 14). Aquele
  varre o XDG na PARTIDA em thread (pixmap por OUTRA); esta lê o CACHE do NetworkManager, e quem
  espera o rádio é uma THREAD em `LastScan`. A precedência das sete vive só em `Footer::view()`.
- **`LEADER r` é rede e Syncthing.** Só o `ronoctua-network` fala NetworkManager; `overrideFolders`
  FICA `false` (`true` apaga as pastas a cada rebuild). 592 GB medidos: 2,4 GHz 2,9 MB/s, 5 GHz 42.
- **A senha do Wi-Fi não fica em arquivo.** `psk-flags` mais o `ronoctua-network-agent` a entregar do
  TPM2 (**sem PCR** — com PCR, atualizar o kernel quebraria o Wi-Fi); autoriza a política do barramento.
- **Tokens e textos são GERADOS do Nix** (`assets/tokens/*.nix` → `ronoctua-tokens`; `*.ftl` →
  `ronoctua_i18n::chave::*`): chave ou cor inexistente é erro de **compilação** — inv. 8 e 9.
- **Layout é aritmética de inteiros.** `layout::resolve` decide em duas fases: preferência
  (`min_cols`) e depois "nada vaza da tela".
- **Buffers nativos são componentes wasip2.** Devolvem display list (`ronoctua-protocol`); rasteriza
  o `ronoctua-render`, posiciona o niri (inv. 4); não conhecem Wayland, cor nem idioma (inv. 11).
  Estado vive na `Instance` — `Runtime::render` avulso zera o buffer. Cinco primitivas: texto, régua,
  grade, seleção e **`svg`** — TRAVA de capacidade (`vector`); cor por token, `currentColor` é preto.
- **`LEADER s m` é o painel de memória**, e a capacidade `system` é a trava dele: o host amostra
  (`ronoctua-memory`, única fronteira com `/proc`) e manda `input::survey`; o buffer só desenha e pede
  por `effect::ask`. Relógio de 1 s lê pseudo-arquivo; varredura e `systemctl` vão para thread ou
  gesto. Perfil tolerante: números do Nix numa tabela em `/etc`, e `--runtime` ou o systemd GRAVA.
- **Duas instalações, uma opção só.** `ronoctua.profile` = `full` (22,8 GiB) ou `minimal` (6,9);
  `nix/modules/profile.nix` é a tabela, tudo `mkDefault`. Instalar mira `#noctua` nos dois — mudar de
  ideia é `nixos-rebuild`. **O ISO leva no `storeContents` o que nenhum cache tem** (shell, niri,
  ghostty-vt, buffers, fontes) MAIS as fontes dos inputs: o alvo baixa, não compila.

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
  **Uma derivação referencia o que os BYTES dela citam**: `rust.workspace` tem todo binário, e punha
  Zig+clang+LLVM (1,70 GiB) e os ícones no ISO — extraia o binário (`rust.installer`, `rust.rescue`).
  **Interruptor que não interrompe**: `notoExtra` e `icons.superpack` desligavam e o pacote entrava
  por outra porta — meça com `nix why-depends`, não confie no `mkIf`. **`option_env!` com caminho de
  store é referência, e quem COMPILA a paga**: ícones e os 5 buffers vêm do AMBIENTE, senão editar
  buffer reconstrói o workspace; ela sobe uma derivação (`rust.shell`) e nunca some.
- **Nunca peça `Family::Monospace`**: fallback, **4 447 ms** contra 0,76 ms. **Buffer não tem parser
  de JSON**: campo nenhum leva caractere cru. **Token vira WIT**: segmento começa por LETRA.
- **Dentro de `''…''` valem DOIS idiomas**: `#`/`//` é do script embutido, mas `${...}` é do Nix e vive até em linha comentada; **`` ` `` é substituição de comando ATÉ no comentário**; e a fence FECHA num `''` qualquer — inclusive dentro de comentário (`'''`, `''$`, `''\` escapam).
- **Chave de texto tem SETE derivados**, só um é o `.ftl` (ARCHITECTURE); **pergunta do instalador
  tem 13** — enum, `ALL_KEYS`, rótulo, ajuda, padrão, `get`/`set`, validação, `is_list`, `options`,
  valor escolhido, dois testes e o `.ftl`. **`to_linear()` desfaz a curva sRGB**; **célula = 0,600 ×
  fonte**; **caixa 1,25 em** contra entrelinha 1,45. `tiny-skia` entrega RGBA PRÉ-MULTIPLICADO.
- **Fallback silencioso precisa de check**: a tabela de tray era validada no build e não chegava ao
  Rust; idem o que cruza a fronteira como STRING. **Cortar por CODEPOINT inventa desenho**: elide por
  CLUSTER. **Cor de token não alcança emoji**.
- **`Limits::downlevel_defaults()` trava a textura em 2048 px** e a superfície É textura: `Surface::configure` devolve `()` e **mata o processo** — use `using_resolution`. **`systemd-run --scope` é SÍNCRONO**: no `calloop` de uma thread, esperar congela o rodapé. **Check contra o CÓDIGO, não o CRITÉRIO,** fecha marco vazio.
- **`nix flake check` SEM `-L` não mostra log**: silêncio não é sintoma; o `secureboot-vm` leva ~2 h calado. **`nix fmt` sem argumento formata a árvore** — Nix, workspace e os cinco buffers; `--check` confere.
- **`rustc` morto por `signal: 15` SEM diagnóstico é o `earlyoom` do HOST** (`-m 10,5`, `rustc` no `--prefer`), e `--max-jobs` não salva: o que estoura é jobs × cores. Por isso o gate é `nix run .#gate`, que mede, limita o produto e acusa o culpado. **`pkill`/`pgrep` casa o PRÓPRIO comando.**
- **Modificador segurado mata o keymap.** No aninhado a leader É `Ctrl+Alt+Espaço`: DESCONTE o que já
  estava em baixo ao ganhar o foco E ao começar a sequência. Caps Lock idem — `not bound` mudo.
- **"Nenhum atalho funciona" tem três causas com a MESMA tela** — tecla que não chega, modificador preso, foco não concedido: `nix run .#diagnostics` ANTES de supor. No Xvfb ninguém tem foco (`xdotool windowfocus`), e **`xdotool key` SOLTA a tecla**: segurar exige `keydown`.
- **ICD Vulkan ausente custa 650 ms mudo**: filtre por vendor. **`memory_init_cow`** é obrigatório (p99 95× pior); **`Queue::present`** precisa do frame, que o `Drop` DESCARTA sem erro.
- **O backend DRM do niri rejeita EGL por software**: numa VM sem GPU sobe com **zero output**, e layer-shell sem output nunca é criada. Use **winit** com `Xvfb` + `LD_LIBRARY_PATH` das libs X. **`RUST_LOG` nosso filtra o niri**, que sobe MUDO — ache o display por `niri.<display>.<pid>.sock`.
- **Quem tem o codepoint primeiro GANHA** — a face pedida por nome vence o fallback. Custou 634 da PUA (Lucide → `U+F2000`) e 75 emoji da Iosevka (`Emoji_Presentation`); `FE0F` vira dois glifos.
- **As fontes são asset de Release, nunca git** — 4,1 GB que git guardaria para sempre; o input é **`file+https://`**, senão o Nix
  infere tarball e descompacta os 4,1 GB no store. **O console não consome TTF**; **`/etc/nixos` e `/etc/fonts/conf.d` são read-only**
  (`localConf`). Genérico é `sans-serif`; ISO é `image.baseName`. Glifo **não** leva `nf-`; 27 variantes SemiExtended: ARCHITECTURE.
- **`systemd.oomd.enable` JÁ é `true` e não vigia nada** — arma é `ManagedOOMMemoryPressure=kill` numa slice. `session.slice` é IRMÃ da `app.slice`: vigiar só a segunda tira o desktop do conjunto por construção, e `enableRootSlice`/`enableUserSlices` desfazem isso. **`memory.min` é limitado por TODOS os ancestrais** — quatro drop-ins ou vale zero. Porcentagem o systemd resolve na MÁQUINA. **`OOMScoreAdjust` em unidade de usuário é ignorado em silêncio**: suba o app com `choom`. **`grep -r` não segue symlink**, e todo `/etc` do NixOS é symlink: `-R`.
- **No btrfs, `nodatacow`/`compress` são do SISTEMA DE ARQUIVOS**: a 1ª montagem vence em silêncio; hibernação depende do NOCOW (`lsattr` → `C`). **PCR 15 exige UKI medida.**
- **`#[repr(C)]` promete ARRANJAR, não conferir** — layout errado de FFI não dá diagnóstico: confira com `sizeof` em C. Ghostty: `Point` tem união (24B), `GridRef` é *sized struct*, `Cell` é `uint64_t`.
- **Componente wasip2 não pode ser membro do workspace do host** (`rust-lld` recusa o version script mudo); **`char` é embutido do WIT**. **API gráfica mudou muito — leia a fonte.**
  **nixpkgs 26.05:** `noto-fonts-emoji` → `noto-fonts-color-emoji`; `nixfmt-rfc-style` → `nixfmt`; `xorg.libX11` → `libx11`; `pkgs.system` → `pkgs.stdenv.hostPlatform.system`. **Atributo dinâmico em duas sentenças** é erro de avaliação: um attrset só.
- **Spawn em unidade do systemd: use CAMINHO, nunca nome** — o `PATH` é mínimo e a falha aparece longe da causa. O caminho do store
  vem por `option_env!`, posto no `nix/rust.nix`. E **`min_cols` é piso de PREFERÊNCIA**; o que não cede é "nada vaza da tela".
  `Shrink`≠`removable`. **Upstream do Niri: `niri-wm/niri`**, por calendário — pinar tag, nunca `main`. Blur da 26.04 não.

## Documentação — seis arquivos, em pt-BR (o resto é inglês — inv. 18)

- `CLAUDE.md` — este. **Teto de 150 linhas** (`docs-exist`); corte antes de acrescentar. Sem log de
  progresso, changelog nem narrativa. `docs/ARCHITECTURE.md` — o porquê do que não se deve refazer.
- `docs/PENDING.md` — o que falta. **Ao concluir, remova de lá**: se mudar como o sistema funciona,
  escreva uma linha aqui ou no ARCHITECTURE; se não, só apague. `README.md` é placeholder, por decisão.
- `INSTALL.md` (disco) e `DEVELOPMENT.md` (ambiente): NÃO liste atalho — a leader os mostra. **Release
  publicada é em INGLÊS**; o `fonts-release` grava o corpo pronto.
