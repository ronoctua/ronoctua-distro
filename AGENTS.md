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
6. **Nada imperativo.** Se não está no flake, não existe.
   Exceção aprovada pelo dono (2026-09-26): **os programas da pessoa**. A central de programas
   (`ronoctua-software`) instala, experimenta, atualiza e remove, só para o usuário, pacotes do
   perfil Nix dele, Flatpaks da instalação `--user`, AppImages e web apps sob `$HOME` e apps dentro
   do Android. Nada disso é padrão da distro nem entra no repositório ou em `/etc`. As únicas
   escritas fora da home são a imagem do Android que a pessoa pede (uma unidade root estreita, em
   `/var/lib/waydroid`) e o store, escrito pelo daemon do Nix. O suporte (Flatpak, nix-flatpak,
   AppImage, Waydroid, o índice de busca) é declarado no flake, e a sequência de atualização não
   instala nada além do que está declarado (a emenda de 2026-10-05, abaixo).
   Exceção aprovada no mesmo molde (2026-09-30): **as regras e a janela de segurança da pessoa, e os
   containers dela**. O firewall base continua declarado; as regras que a pessoa cria ficam em
   `/var/lib/ronoctua-security` e uma unidade declarada as aplica numa tabela nossa (`inet
   ronoctua`); a janela de sudo sem senha é uma linha numa pasta persistida do root
   (`/var/lib/ronoctua-sudo`), que sobrevive ao reboot e acaba no prazo dela (o `NOTAFTER` do
   próprio sudo) ou quando a pessoa a fecha — "sem prazo" não tem `NOTAFTER` —, e uma linha vencida
   sai no boot e no prazo (emenda aprovada pelo dono, 2026-10-11); a lista das chaves de segurança
   em que o `pam_u2f` confia é do root (`/var/lib/ronoctua-security/u2f_keys`), escrita só pela
   porta do sudo; imagens, containers e volumes do Podman ficam só na home.
   Exceção no mesmo molde (2026-10-05, aprovada no plano da leva): **o `/private` da pessoa**. Só
   pelo gesto dela, pela porta do sudo, a unidade declarada `ronoctua-private-root` cria, abre,
   tranca e muda uma imagem LUKS esparsa em `/var/lib/ronoctua-private` (o subvolume `@private`,
   declarado) e a monta em `/private`; antes de dormir, a `ronoctua-private-lock`, declarada, a
   tranca. Nada disso entra no flake, em `/etc`, no `fstab` ou no `crypttab`.

   <!-- anchor: L03 -->
   Emenda aprovada pelo dono (2026-10-05): **a máquina tem o próprio flake, e a pessoa declara**.
   `/etc/nixos` é o flake da máquina, com a distro como input que segue a branch `release`
   (`lib.machine`, `templates/machine`), e ele usa exatamente os pins que a distro testou
   (`machine-uses-the-distro-pins`); o nome da pasta está escrito só em `nix/machine.nix`
   (`machine-dir-has-one-name`). O que a pessoa declara (`person/software/*.json`) é dado, nunca
   código Nix: em `nix/modules/person.nix` um pacote, AppImage ou app Android declarado é uma PARTE
   dela (o apk, uma parte de dados que o reconciliador lê pelo caminho estável), que o sistema nunca
   avalia e que o realizador avalia sozinha (`personParts`; emenda de 2026-10-25, aprovada no plano
   das partes), e os lançadores web e os Flatpaks (nix-flatpak `--user`) vão para ela — nunca para
   o `systemPackages`. A entrada que não se aplica é pulada e dita em
   `/etc/ronoctua/person-applied.json`, e a que não avalia, não constrói ou não baixa falha só a
   própria parte.
   Um módulo do brain da pessoa com metade NixOS só é importado aprovado pelo hash atual
   (`nix/person-hash.nix`).

   <!-- anchor: L06 -->
   Emenda aprovada pelo dono (2026-10-05): **as declarações da pessoa chegam à máquina como dados**.
   Uma unidade root estreita (`ronoctua-person-apply.service`, que a pessoa inicia por uma regra
   polkit de um verbo, uma unidade e um usuário) valida o JSON que a pessoa preparou em
   `/run/ronoctua-person/staged` e o troca inteiro para `person/` no diretório da máquina: cada
   documento reescrito na forma canônica, nunca código Nix; link, arquivo especial, formato mais
   novo e tamanho acima do teto são recusados, e os dados velhos ficam. O único código que viaja
   junto são os módulos do brain da pessoa, e o `module.nix` de um deles só entra num build
   aprovado pela porta do sudo, no hash que o `nix/person-hash.nix` calcula da cópia da máquina
   (`ronoctua-person-approve.service`, sem regra polkit). Não existe mais entrada retida: a
   verificação que a segurava em `local/held.json` saiu em 2026-10-25, e todo `apply` apaga esse arquivo
   e o `local/verified.json`.

   <!-- anchor: L13 -->
   Exceção aprovada no mesmo molde (2026-10-05): **o Incus da pessoa**. O daemon, a ponte
   `incusbr0`, o pool e o perfil `default` são declarados (`nix/modules/incus.nix`, só no perfil
   `full`); as instâncias, imagens, snapshots e volumes que a pessoa cria ficam em `/var/lib/incus`
   (persistido, num subvolume próprio), nunca no repositório nem em `/etc`. O daemon não busca nada
   sozinho (`images.auto_update_interval = 0`), e toda chamada que o faz alcançar um remoto leva o
   `Pass` de cada host que o cliente do remoto pergunta (`hosts_of`: o Docker Hub são três; cobra
   `incus-asks-with-a-pass`). Estar no grupo `incus-admin` equivale a root, e a pessoa está nele por
   decisão do dono.

   <!-- anchor: L19 -->
   Emenda aprovada pelo dono (2026-10-05): **a atualização instala o que está declarado, e nada além
   disso**. O que a distro declara (`ronoctua.apps`) e o que a pessoa declara (`person/software/`,
   os apps do Android inclusive) entram pelo rebuild e pelo reconciliador (`ronoctua-software
   reconcile`, que só mexe no próprio registro); nenhum argumento da sequência nomeia o que
   instalar, e o gerador continua recusando todo argumento que instala. A ordem é lei do gerador
   (`steps.nix`, `system-update-laws-refuse`): a pessoa reúne antes da metade root, a metade root
   migra, move só o input da distro (nunca com a máquina retida) e aplica
   (`ronoctua-machine apply`, sob o lock de root dos dados da pessoa), e a pessoa reconcilia e
   publica por último, em toda saída. O input da distro só anda quando o rebuild pode ser tentado
   (o mesmo piso de bateria e a mesma folga no disco), para a pasta da máquina nunca nomear uma
   distro que não roda. Depois de aplicar, um passo do root planeja as partes do sistema novo
   (`parts-plan`, `realize-parts`: enraíza as derivações delas antes da coleta e não espera download
   nenhum, nunca `stop`, e não nomeia parte). Uma execução agendada espera por rede tarifada ou bateria abaixo do piso e
   nunca pergunta; uma à mão pergunta com o "não" primeiro — abaixo do piso antes de começar, numa
   rede tarifada quando o cérebro a segura — e o "sim" vale só para aquela execução.
   Exceção no mesmo molde (2026-10-11, decisão do dono): **o cache binário da pessoa**. A escolha
   (nome, chave, quem lê) é dado no `options.json` dela e o token de envio só na parte cifrada; o
   único estado fora da home é o netrc do root para um cache privado
   (`/var/lib/ronoctua-binary-cache/netrc`, `0600`), escrito só pelo gesto dela pela porta do sudo
   (`binary-cache-netrc`) e pelo instalador, e removido quando o token ou o cache privado saem; o
   netrc e o token nunca entram no flake, no store nem em `/etc`.
   Emenda aprovada pelo dono (2026-10-20): **os apps da distro podem seguir as próprias releases, no
   perfil da pessoa**. Só com a opção dela ligada (`distro-apps-follow-releases` no `options.json`,
   desligada por padrão; ligar é o gesto que permite `api.github.com` e `github.com`), o
   `person-pins` leva cada AppImage da tabela com `follows.github` à release mais nova, em
   `person/software/distro-pins.json`: dado da pessoa, só assinado, como o `options.json`, nunca de
   repositório fora da tabela, e nenhum argumento da sequência nomeia o que instalar. Os bytes são o
   download da própria pessoa, então a versão seguida é o pin com que a PARTE do próprio app é
   construída (todo AppImage da distro é parte, e um que segue tem de ser): nunca no `systemPackages`
   nem num cache público, e um download que falha deixa a parte na geração anterior — a última que
   chegou, ou a da distro. O que o juiz recusa (`nix/distro-pins.nix`, a tabela de casos do Rust) é
   dito, nunca uma parada (cobra `distro-apps-follow-when-on-and-newer`).
   Exceção no mesmo molde (2026-10-20, aprovada no plano da leva): **as extensões do VS Code e o
   Quicklisp da pessoa**. Só pelo gesto dela — a oferta dos pertences ou `ronoctua-vscode install`,
   com o `Pass` de cada host que `hosts_of` nomeia; `quicklisp-install`, digitado — o que a distro
   instala fica no home dela (`~/.vscode/extensions`, `~/quicklisp`), a partir de uma lista que é
   dado dela ou de arquivos pinados; nada disso entra no flake, em `/etc` nem no repositório.

   <!-- anchor: L25 -->
   Emenda aprovada pelo dono (2026-10-23, no plano da instalação sem download): **o que precisa de
   download chega como parte, fora do sistema**. Uma parte é declarada no flake
   (`ronoctua.parts.<id>`, no módulo dono e fora de qualquer `mkIf`) e realizada só a partir do
   plano do sistema em execução, só pelo `ronoctua-parts` (root), num perfil Nix do root
   (`/nix/var/nix/profiles/ronoctua/parts/<id>/v<interface>`). O fecho do sistema nunca contém uma
   parte: o plano (`/etc/ronoctua/parts-plan.json`) a nomeia como texto, e o sistema a alcança só
   por esse caminho estável, que pode ainda não existir — quem a lê pergunta
   `ronoctua_parts::here`, e a tela diz que ela está chegando. Cada parte é um processo
   (`ronoctua-part@<id>`): uma que não chega nunca impede o sistema nem as outras, é tentada de novo
   cada vez mais tarde, e a que falha fica na geração anterior. A sessão a vê por uma vista só
   (`/var/lib/ronoctua-parts/view`: `bin/` e um conjunto fechado de `share/`, nunca `lib/`). O que a
   pessoa entrega à mão, importa de um pendrive ou traz de um espelho entra no store só quando o
   Nix confere o hash que o flake já declara; uma saída construída chega só pelos substituters
   declarados. **Não livre é aprovado** (decisão do dono, 2026-10-24): o `pkgs` do sistema o admite,
   e nenhum não livre derruba uma avaliação (cobra `unfree-never-refuses`); os programas não livres
   da distro continuam partes, porque são downloads. O que ninguém pode redistribuir nunca vai no ISO
   publicado (cobra `iso-carries-nothing-undistributable`): numa máquina instalada sem rede, os ramos
   de hardware que o pedem esperam o marcador `hardware-deferred.json` sair, na primeira
   `ronoctua-machine apply` que os constrói.
   **A instalação nunca usa rede** (no mesmo plano): o ISO leva o fecho livre das duas bases, a cola
   que um host diferente constrói (calculada dos grafos de `.drv`) e todo nó do lock
   (`nix/installer/offline.nix`); todo `nix` do instalador roda `--offline`, sem substituter nem
   registro, e o sistema é construído antes do disco. Os pertences terminam no primeiro boot
   (`ronoctua-first-boot`, com a senha selada no TPM), e a primeira atualização desmira a máquina da
   árvore do ISO (`ronoctua-machine update-distro`). O ISO completo (`.#iso-complete`), com toda parte
   a bordo, não livres inclusive, é do desenvolvedor e **nunca é publicado** (cobra
   `the-complete-iso-is-never-released`).

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
    mostra a coluna do espaço que está nela, e um inquilino existe uma vez só: abri-lo em outro
    espaço o muda de lugar. A coluna vai do topo até o rodapé e não o estreita: modeline, minibuffer
    e faixa de notificação ficam com a largura toda. Em qualquer tamanho, barra inclusive, ela é um
    inquilino desenhado numa faixa, nunca dock nem taskbar: não lista nem lança programas. Não
    existe barra de cima, taskbar, menu iniciar, dock, ícone de desktop nem popup flutuante; tray,
    notificação e expansão são a mesma superfície crescendo, e a barra de alerta máximo (bateria
    fraca) toma o lugar da faixa de notificação enquanto existe. A barra do sudo (emenda do dono,
    2026-10-11) é a mesma superfície crescendo acima da faixa e da modeline, como a barra de alerta,
    e só existe enquanto um gesto de root é perguntado (`sudo_bar.rs`). Três exceções
    aprovadas: a sobreposição da captura (layer `overlay` que só existe enquanto se escolhe o que
    capturar e some antes de o arquivo ser escrito — `capture.rs`, 2026-09-26); as cortinas do
    redimensionador de tela, ferramenta de desenvolvimento (`LEADER / r`): duas superfícies na
    layer `overlay`, à direita e embaixo, que só existem enquanto ele está ligado e cobrem só o que
    ele cortou (`resizer.rs`, 2026-09-27); e a linha de vida, que ocupa o lugar do rodapé só
    enquanto ele está morto.
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

Todo programa que a distro instala para a pessoa escolher usar — com lançador ou de terminal — é uma
entrada `ronoctua.apps.<id>` (`nix/modules/distro-apps.nix`), escrita no módulo dono e **fora de
qualquer `mkIf`** (a presença é o `when`), e não uma linha solta em `systemPackages`, que nenhuma
recusa alcançaria. Linha solta só para o que não é escolha da pessoa (a lista e o porquê estão no
`docs/ARCHITECTURE.md`). O que segura um programa (variável, serviço, passo da atualização) segue
`ronoctua.distroApps.installed.<id>`, não o interruptor do grupo. Um id publicado entra no
livro-razão de `assets/distro-apps.nix` e só sai dele por `formerly` ou `retired`.

<!-- anchor: L16 -->

```
nix run .#release            publica: pins movidos (seu único commit), gate, release (só para frente), ISO
nix run .#fetch-iso          baixa o ISO da versão, confere cada parte e o inteiro, junta
nix run .#iso-sweep          apaga os ISOs que nada enraíza, menos o mais novo de cada nome
nix run .#adopt-machine      máquina do layout antigo → flake próprio (migrate + apply, com sudo)
nix run .#apply-here         a máquina com o checkout no lugar do input; o lock fica como estava
```

A branch `release` é o que toda máquina segue: só o `.#release` a move, e o `.#release` é do dono,
como o `.#gate`. A distro não tem cache binário: o ISO leva sempre a semente. O cache é da PESSOA (o
Cachix dela): nome, chave e visibilidade no `options.json` (parte em claro, assinada), o token de
envio só na parte cifrada, e só `nix/modules/binary-cache.nix` declara substituter, chave e netrc
(cobra `binary-cache-declared-once`). O token nunca vai em argumento, no ambiente de um processo
longo nem em log (cobra `binary-cache-token-never-prints`). Um cache público recebe só as saídas da
própria distro, nunca um caminho da pessoa, um download ou um não livre — e por isso um programa não
livre nunca é assado na compilação (`generated`): vai no `atRunTime`, senão toda unidade que o lê
fica fora de qualquer cache público (cobra `release-cache-holds-only-free-builds`).

Tudo é app do flake — nunca justfile nem Makefile. Um passo que precise de instrução escrita
deveria ser automático. A instalação tem dois perfis, `ronoctua.profile = "full"` (~37 GiB; ~35,5 com o
padrão do instalador, sem o LibreOffice) ou `"minimal"` (~12,5 GiB), com a tabela em
`nix/modules/profile.nix`. Os dois trazem o trabalho na distro: o toolchain do devshell no store
(`nix/toolchain.nix`, a fonte única), o rust, o Common Lisp e os agentes no PATH, o KVM e o
`nix-direnv`.

## Como o sistema é montado

### Processos

- **Compositor**: o Scroll (`dawsers/scroll`, que segue os lançamentos do sway), pinado numa tag —
  nunca `main`. A sessão passa `-c` com o config gerado por nós; o do upstream nunca é lido.
- **`ronoctua-shell`**: o binário fino que compõe os ~50 crates `ronoctua-shell-*` — a base, uma
  funcionalidade por crate e o rodapé (`ronoctua-shell-footer`, tipo `Footer`) — e a linha de
  comando: os subcomandos dele (`leader`, `command <id>`, `photograph`…) e a tabela `DOORS` de cada
  crate (`capture`, `lifeline`, cada janela).
- **Daemons de usuário**, para o que tem de sobreviver ao rodapé: `ronoctua-transfers` (a fila),
  `ronoctua-brain` (o que a máquina faz sozinha, em módulos — veja abaixo), `ronoctua-player` e
  `ronoctua-studio` (a câmera). Falam com o shell por socket, um JSON por linha, e nunca um delta:
  cada mensagem vai inteira (o player manda duas, o estado e o contorno da fila). O que é grande e
  quase ninguém mostra — a folha de um arquivo, a página de uma busca — nunca vai no estado: ele
  leva a geração, e a porta que mostra pede, fora do laço, quando ela anda.
- **`ronoctua-software`**: não é daemon. É a biblioteca e a CLI dos programas da pessoa
  (`LEADER i`); toda ordem que muda algo é um `Work::Software` que a fila roda como filho, e
  instalar só para a pessoa é DECLARAR (`Order::Declare`): instala na hora e grava a entrada no
  `declared.json` dela, que a atualização constrói em cada máquina.
- **`ronoctua-browser`**: o host de native messaging que o Chrome sobe enquanto existe uma janela
  quiosque. Não é daemon nosso: vive e morre com a porta da extensão. Ele conecta no rodapé, que é
  o servidor (`ronoctua[-dev]-browser.sock`), e manda o estado inteiro, um JSON por linha.
- **Buffers**: componentes WASM hospedados pelo shell (veja abaixo).

### O rodapé

- É o desktop: uma `wlr-layer-shell` ancorada embaixo, com modeline e minibuffer colados, a região
  de expansão acima deles (teto em `space.expansion.maxLines` e `maxFooterPct`; o filme sozinho do
  player, `o`, vai a `playerStagePct`) e a faixa de notificação acima da modeline. Crescer para cima
  não move a modeline.
- Em repouso ele não pega o teclado, senão nenhum buffer receberia tecla. Quem vê a leader é o
  compositor, que roda `ronoctua-shell leader` no socket de controle
  (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`); só então a superfície expande e pede teclado
  exclusivo. `ronoctua-shell command <id>` roda qualquer comando do registro.
- A região mostra uma vista por vez, e nenhuma é menu flutuante: é a região crescendo (inv. 14). **A
  precedência entre vistas mora só em `PRECEDENCE`** (`surface/mod.rs`), que `Footer::view()` lê;
  todo despachante (teclas, linhas, cliques, dica, comandos, `region_is_free`) é um `match` sobre
  `view()`, UMA linha por vista (cobra `the-footer-draws-what-the-keyboard-drives`), e só a pergunta
  do minibuffer, que é uma linha, fica fora. A vista de uma funcionalidade é a região do crate dela,
  registrada pelo binário (`regions.rs`), e só toma a região livre: a recusa é dita na tela.
- **`Footer::metric` não pergunta o que a região contém.** Ele transforma largura, altura e a
  primeira linha da região (`first_of_expansion`) num frame, e tudo o que ele alcança e consulta o
  conteúdo vira recursão — a shell já morreu duas vezes assim, de stack overflow e sem diagnóstico.
  O que a região sabe de si é guardado onde foi medido (cobra `the-footer-does-not-ask-itself`, que
  também recusa ciclos novos entre métodos do `Footer`).
- **Dois modos ocultos** (`hidden.rs`, gravados em `modeline.json`): um toque de `Ctrl` na raiz da
  leader gira normal → empurra → cobre (`hidden::next_mode`, `footer-bars-cycle`). As linhas saem de
  `hidden::Shape`, que o `apply_geometry` guarda para as somas sob o `metric`. Texto sobre a cor da
  mesa é opaco (sobre alfa 0 sai com franja branca), e nos dois modos o rodapé fica na camada
  `overlay` (`hidden::geometry`): uma camada que muda sob o ponteiro ganha um `leave`, e a borda
  virava um laço.
- **A modeline se adapta a um programa** (`adapt.rs`; hoje o Chrome quiosque): o que ela desenha
  responde ao ponteiro, e tudo o que um segmento roda é um comando do registro, que o grupo `,`
  também alcança (cobra `footer-mouse-reaches-nothing-new`).
- Se o rodapé morre, o `OnFailure=` levanta a linha de vida (`ronoctua-shell lifeline <unidade>`).
  Os binds de emergência morrem com ele, porque escrevem no socket dele. `Ctrl+Alt+Del` é o resgate.

### Portas: toda janela é uma `Door`

- Toda janela é um `door::Content` aberto por `door::run` (`crates/ronoctua-shell-door/src/door/`).
  A `Door<C>` cuida do resto: registro, assentos, teclados e repetição, ponteiros, toque, escala,
  GPU, as vigias de tema, idioma, tamanho do texto e forma da data, cabeçalho, lista, barra e roda.
  A janela escreve só `app_id`, `title` e `typed`; o resto tem um padrão, que ela pode sobrescrever
  (cobra `every-window-is-a-door`). O rodapé e a linha de vida são layer-shell montados com as
  mesmas peças. Toda janela de um crate está na tabela `DOORS` dele, e toda câmera na `CAMERAS`
  (cobra `every-door-is-dispatched`).
- Uma vista costuma ser **uma `View` com duas portas**, a região do rodapé e uma janela, que
  desenham as mesmas linhas. Na leader, a sequência dobrada abre no rodapé e a terminada em `w` na
  janela (`s n n` / `s n w`); o grupo que tem as duas vira uma tecla só (`bindings::folds`), e o
  `the_collapsed_groups_are_these` fixa os grupos que dobram.
- **A vista com as duas portas é uma região** (`region::Region`, no crate dela): UM tratador de
  recados (`carry_out`) sobre `&mut dyn region::Doorway`, que a janela e o rodapé implementam (cobra
  `errands-are-handled-alike`); ela lê o que o rodapé já guarda pelo que ele empresta, sem abrir de
  novo a thread de um daemon (cobra `regions-order-the-footers-threads`).
- **As duas portas fazem as mesmas coisas** (regra do dono, 2026-09-27): tudo o que a janela suporta
  o rodapé suporta, e vice-versa — cada nível, cada tecla, colar, soltar arquivo e **digitar**
  (buscar, responder uma pergunta, senha, caminho, nome). Nenhuma porta manda a pessoa fazer algo na
  outra; uma lacuna numa porta é defeito — e o que uma porta DIZ (uma recusa, uma falha) é dito
  nela. Uma janela pergunta com `Ui::ask` (a resposta chega em `Content::answered`), cola com
  `Ui::paste` e abre um programa com `Ui::launch`, tudo fora do laço; as duas portas colam a seleção
  viva (`pasting.rs`), nunca o histórico. Uma mensagem sai com o prazo da classe dela (`Ui::say`,
  `Ui::say_for`; `Ui::hold`, no rodapé `echo_held`, só para um estado que continua verdadeiro),
  nunca escrita direto em `Ui::message` (cobra `door-messages-expire`).
- Toda porta ouve todos os assentos e todos os teclados (`PerSeat` e `Seats::switch_to` em
  `seats.rs`; cobram `every-door-hears-every-seat` e `every-door-hears-every-keyboard`), segue a
  escala da tela em que está (`hidpi.rs`; cobra `every-window-knows-its-scale`) e repete teclas
  (`get_keyboard_with_repeat`; cobra `every-window-repeats` — o rodapé é a exceção, porque repetir
  a leader é erro).
- As colunas à direita têm uma base só, `column::Column<T>` (`column.rs`), como toda janela tem a
  `Door`: tamanho, linha de controle, linha de foco e espaço de trabalho são dela, e o inquilino
  (`column::Tenant`) só desenha na `column::Area` que recebe. O slot é um por espaço
  (`right_column.rs`), e fora da tela a coluna fica estacionada (`Shell::Parked`).

### Módulos do brain

- Uma funcionalidade do brain é uma pasta, `crates/ronoctua-brain/modules/<id>/`: `manifest.nix`
  (o que é, agenda, unidades, prefixos de chave, glifos, marcas, avisos, ações, eventos, `uses`,
  `namedBy`), `module.rs` (`pub fn new(birth) -> impl Module`), `module.nix`, `i18n/<idioma>.ftl`,
  `checks.nix` e `why.md`, e opcionalmente `tables.nix` e `shared.rs` (a metade que o `namedBy` lê).
  O id é o nome da pasta, e o `nix/brain-modules.nix` recusa pasta incompleta e campo desconhecido.
- O rodapé linka só a biblioteca `ronoctua-brain` (o fio, a mente, os `shared.rs` e os manifestos da
  distro: mexer num deles recompila o shell); todo `module.rs` é do `ronoctua-brain-daemon`,
  `[[bin]]` `ronoctua-brain` (cobram `brain-daemon-is-a-leaf` e `brain-library-links-no-module`). O
  que o rodapé desenha de um módulo é o catálogo do brain (`RONOCTUA_BRAIN_CATALOGUE`), dado lido em
  tempo de execução, e glifo de módulo não entra no glyphdb nem no WIT (cobra
  `brain-catalogue-is-what-the-daemon-compiled`).
- Núcleo, shell e buffer desenham marcas, avisos, páginas, ações e eventos de qualquer módulo sem
  nomeá-lo; nenhum arquivo de fora cita o id, uma chave, um glifo, uma unidade ou uma ação de um
  módulo, salvo o que o `namedBy` dele lista (cobra `brain-modules-are-sealed`).
- Uma marca abre um comando de um jeito só: `MarkId::opening(<comando>)`.
- Preferência se resolve sobre o que o módulo declarou (`Prefs::of_module`), e toda mudança é um
  `Ask::Set` aplicado pelo núcleo.

### A fronteira com o compositor

- Só o `ronoctua-scroll` fala IPC; shell, telas e resgate dependem apenas do contrato, e cada
  programa abre uma conexão só — a do shell, no kit (`daemon::open_the_compositor`).
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
- Nenhuma cor é escrita à mão. `assets/tokens/colors.nix` é receita
  (`text.primary = "neutral.text-strong"`), e os valores saem de `nix/scripts/gen-theme.py` a partir
  das sementes `assets/themes/<id>.nix`. Toda relação entre cores é um portão (contraste APCA,
  distância ΔE_ok, matiz), e o `owl` gerado é comparado com `owl.reference.nix`. `order` e `default`
  são campos separados em `themes.nix`: o primeiro da lista não é o padrão.
- Para afinar uma cor, mexa na semente e rode `nix build .#themes --no-link --print-out-paths`
  direto do shell (~20 s; dentro do `nix develop` demora mais): a linha 5 do `report.md` traz o
  placar dos portões de todas as paletas (`Fail 0`). Semente, receita, gerador e tema padrão não
  reconstroem Rust nenhum.
- Papel novo exige família em todas as sementes (menos a `.reference`) e um nome em `colors.nix` ou
  `shaped.nix`; sem isso o gerador recusa com `no family '<nome>' in the seed`.
- As cores das faixas de legenda (`subtitle.*`) são a exceção de página: resolvidas contra uma
  imagem escura fixa, iguais em todo tema e página. No cromo, a cor de uma faixa só aparece dentro
  de um chip de `subtitle.edge` (`chrome::chip`): sobre uma página ela não foi medida.
- Hex só existe gerado ou medido (`palettes.bin`, `report.md`, a fixture congelada). Quem escolhe
  uma cor diz matiz, croma e contraste; o solver resolve.
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

<!-- anchor: L01 -->

**Cifra e assinatura novas da pessoa são pós-quânticas** (`ronoctua-pq`, o único que conhece o
X-Wing): recipiente `age1pq1…`, que nunca viaja com um clássico — selar e abrir recusam a mistura —,
num formato que os vetores e o `age` do Go decidem (cobram `pq-xwing-vectors` e `pq-age-interop`), e
ML-DSA-65 para assinar. A identidade e a semente só existem num arquivo `0600` ou na memória que se
apaga, e nada as imprime (cobra `pq-identity-never-prints`). A máquina confia na chave que root
fixou (`local/signer.json`) ou na que a senha da pessoa abriu, nunca na que o repositório traz. A
parte em claro dos pertences viaja assinada, com um contador que só cresce: o que chega validamente
assinado entra sozinho, e o resto fica retido com a frase do motivo (cobram
`belongings-tampered-plain-is-held`, `belongings-plain-rollback-is-refused`,
`person-apply-refuses-unsigned-tree` e `brain-unsigned-module-never-compiled`).

<!-- anchor: L02 -->

**Transporte pós-quântico primeiro.** O TLS é o rustls sobre o `aws-lc-rs` com
`prefer-post-quantum`, o provedor passado ao builder e nunca instalado como padrão do processo
(`ronoctua_http::tls::provider`), e um host que não conhece o híbrido continua alcançado (cobra
`tls-is-post-quantum-first`). O `aws-lc-rs` é o único provedor das primitivas que o nosso código
chama, salvo as exceções escritas (cobra `one-crypto-implementation`); o `ring` do `Cargo.lock` não
compila: pergunte ao `cargo tree`, não ao lock. O SSH negocia pós-quântico primeiro, e todo sshd
nosso tem só a chave de host ed25519 (cobram `ssh-kex-is-pq-first` e `no-rsa-host-key`). O
transporte dos pertences é SÓ pós-quântico (`pq_only`, no SSH e no HTTPS do git; cobram
`belongings-ssh-is-pq-only` e `belongings-https-is-pq-only`), e o OpenSSL ignora CALADO um
`OPENSSL_CONF` que não consegue usar: só a prova contra um servidor clássico mostra o estreitamento.
O sntrup761 fica nas listas enquanto o GitHub depender dele.

<!-- anchor: L05 -->

Quem muda o sistema é o `ronoctua-machine apply` — o rebuild de resgate, a atualização e a central
de programas —: constrói antes, troca depois e arma o ensaio de boot (no systemd-boot ou, numa
máquina BIOS, no GRUB). Nenhum outro crate roda `bootctl` ou `grub-editenv`, escreve `LoaderEntry*`,
o `loader.conf` ou o `grubenv`, mexe no perfil do sistema ou chama `switch-to-configuration` (cobra
`one-bootctl-driver`). O que não fez um sistema novo devolve o lock do sistema em execução
(cobra `apply-reverts-the-lock-on-build-failure`), o bless pende do alvo padrão, nunca do
`graphical.target` (cobra `update-apply-rule-is-narrow`), e um ensaio é julgado pela identidade do
boot, nunca pelo relógio; a aparagem guarda o último bom e o ensaio (cobra
`generations-keep-the-boot-trial`).

<!-- anchor: L06 -->

Quem escreve o `person/` da máquina e o `local/` dela (`approved.json`, `signer.json`) é o binário
`ronoctua-person`, como root, e só ele (`apply`, `approve`, `pin`); ler é de todos (cobra
`one-person-boundary`). Todo arquivo da pessoa se lê com `ronoctua_person::parse`
(formato mais novo é recusado antes dos campos) e se escreve com `ronoctua_person::to_json`; o que
root abre debaixo de uma pasta da pessoa é aberto relativo a ela, com `O_NOFOLLOW` (cobram
`person-apply-refuses-links` e `person-apply-rule-is-narrow`). Quem mexe nesses dados, ou constrói a
partir deles, segura `ronoctua_person::root::Lock`, um arquivo `0600` de root e nunca uma pasta que
qualquer um trava.

<!-- anchor: L10 -->

**Os pertences levam as chaves da pessoa, nunca o acesso a eles** (emenda aprovada pelo dono,
2026-10-05): os alvos pré-definidos (`targets` em `assets/belongings.nix`) viajam selados, os
privados voltam com os bits do dono só (`kept::kept_mode`), e a chave de implantação nunca viaja,
sob nome nenhum (`crate::sift`). Um alvo nunca fica sobre arquivo do Home Manager (cobra
`belongings-targets-avoid-home-manager`). A identidade que a senha abre fica, pela sessão, só no
keyring do kernel (as portas leem o marcador `runtime/unlocked`), e o fim da sessão e o sono a
esquecem.

**Uma chave de segurança responde ao sudo, e só a ele** (decisão do dono, 2026-10-11): o `pam_u2f`
está sempre declarado, sempre `sufficient`, só no serviço `sudo`, atrás de uma porta de terminal,
com um `origin` fixo (`u2f::ORIGIN`) e a lista do root (`u2f::AUTHFILE`) como `authfile`; nunca
`nouserok`. Ligar e desligar é a própria lista, sem rebuild, e só o verbo `u2f-keys` da porta do
sudo a escreve; o `u2f_keys` da pessoa nunca é o que o PAM lê, e o `options.json` não diz nada disso
(cobra `pam-u2f-is-only-sufficient`). O dedo segue o mesmo molde.

<!-- anchor: L12 -->

**`/private` não se toca num laço de eventos.** Suspenso ao dormir com algo em uso, quem o tocar
espera no kernel até a senha retomá-lo, e a porta que perguntaria a senha é a que travou. Listar,
fazer `stat` ou vigiar um caminho pergunta antes `ronoctua_private::may_touch`; dentro do cofre
aberto, a leitura roda numa thread que o laço espera por um instante só.

<!-- anchor: L13 -->

<!-- anchor: L14 -->

O `ronoctua-llm` só EMBRULHA a statusline de uma conta do Claude Code e devolve a da pessoa byte a
byte (cobra `statusline-chains`). O token OAuth é lido só no `usage.rs`, em memória zerada, e nunca
renovado, e o pedido que o leva é montado, como todo pedido TLS, em memória medida antes e zerada
(cobra `llm-tokens-never-linger`). Quem vigia uma conta pergunta os caminhos ao
`ronoctua_llm::watched`.

<!-- anchor: L17 -->

## Regras de interface

### Cabeçalho, dicas e teclas

- O cabeçalho de uma janela é glifo, rótulo e título (`chrome::Title`) seguidos dos atalhos, e todo
  atalho é um clique (`chrome::Act`: `Act::Key` aperta a tecla pelo mesmo caminho do teclado,
  `Act::Run` roda um comando); tecla sem ação é desenhada como inerte, e o atalho sob o ponteiro
  acende (cobra `every-hint-key-is-a-click-or-says-so`). A tecla que um cabeçalho ensina funciona
  onde é ensinada.
- O cabeçalho de toda janela abre numa gaveta: chevron na borda, `RCtrl ↑↓` no teclado, e `Esc`
  fecha a gaveta antes de fechar a janela (`chrome::Hinted::head`; cobra
  `every-window-header-opens`).
- Toda dica se explica sob o ponteiro, numa linha abaixo dos atalhos, com a linha aberta. A frase é
  `cmd-<id>` para comando do registro e `about-<id>` para tecla de vista (cobra
  `every-hint-key-explains-itself`). Essa linha sai da região, nunca da altura do rodapé: se o
  rodapé crescesse, sairia de baixo do ponteiro, o hover acabaria e a linha sumiria, em laço.
- A coluna muda de tamanho com `C-<left>` e `C-<right>`, lidas antes do inquilino
  (`column::OF_THE_COLUMN`). Na linha de controle, a frase do glifo sob o ponteiro vai na própria
  linha; no tamanho barra, o glifo acende e não se explica (decisão do dono, 2026-09-27).
- Toda vista da região ensina `Esc` primeiro, e ele é clicável (`chrome::with_the_way_back`). O
  botão Voltar do mouse sobre o rodapé ou sobre uma janela é o mesmo `Esc`.
- Tecla nunca é texto no `.ftl`: a sequência sai do código (`bindings::taught`, ou uma constante
  ao lado do binding) mais `bindings::label()`, num `{ $keys }` (cobra `keys-are-never-text`).
  "LEADER" não é tecla de ninguém: em produção é `SUPER`, no `.#dev` é `CTRL ALT SPC`.

### Linhas, cores e cursor

- Os tons de uma linha vêm do tipo, nunca de um argumento de cor: `chrome::Kind` (rótulo e valor, de
  fato e de ajuste); do lado WASM, `Role::LabelFact`, `LabelSetting`, `ValueFact` e `ValueSetting`
  (cobra `labels-say-what-they-are`).
- O cursor é uma cor (`text.here.*`): o rótulo leva o acento e o valor continua dizendo se a linha
  age; `Row::focused` não é lido por porta nenhuma (cobra `the-cursor-is-a-colour`). A exceção é o
  gerenciador de arquivos, cujo cursor é a faixa `surface.selection` mais a régua acesa, também numa
  grade, e a célula sob o ponteiro é uma conta só, lida pela pintura e pelo clique
  (`chrome::in_cell`, `chrome::Shelves`; cobra `a-grid-cell-is-lit-alone`).
- Rótulo e valor têm uma aritmética só (`chrome::field_pair`: o valor começa em
  `space.view.labelCells`, limitada à metade da linha); uma seção escrita inteira alinha os valores
  depois do rótulo mais largo e quebra o que não cabe (`chrome::aligned_pair`).
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

<!-- anchor: L11 -->
- Exceção aprovada pelo dono (2026-10-05): as marcas das contas LLM,
  `brain:agent-accounts/claude.<n>` e `brain:agent-accounts/codex.<n>`, ficam na modeline também em
  repouso, uma por conta, e o tom diz o uso. Elas declaram `resting = true`, e só as listadas em
  `assets/brain-resting-marks.nix` podem declarar, nunca a de um módulo da pessoa (cobra
  `resting-marks-are-the-approved-ones`, que mantém este item e aquele arquivo iguais). O núcleo
  cobra só o tom: descarta a marca publicada em `Tone::Quiet` (a que diz só que está tudo bem) cujo
  manifesto não declara `resting`. Uma marca em outro tom publicada sem ter o que dizer continua
  proibida pela regra acima, e nenhum check a pega. Uma marca pode ter instâncias (`instances = N`,
  de 1 a 9, porque é um dígito; cobra `mark-instances-are-bounded`), cada uma um segmento
  `brain:<módulo>/<marca>.<n>`, e o clique entrega o número ao comando de `MarkId::opening`.
- Módulos do brain da pessoa (`$XDG_CONFIG_HOME/ronoctua/brain-modules/<id>/`, na parte em claro dos
  pertences): o id é `my-` mais uma palavra kebab, e `my`/`my-…` são só dela, prefixos de chave
  inclusive (cobra `brain-person-module-ids-are-prefixed`). A pasta precisa só de `manifest.nix`,
  `module.rs` e um `.ftl` por idioma; a metade NixOS entra só aprovada pela porta do sudo
  (`personNixosModules`), e o `checks.nix` não roda. O carregador recusa a pasta da pessoa sozinha,
  com frase (`personRefused`), nunca a avaliação inteira (cobra `brain-person-modules-load`); a
  máquina compila com `withPersonModules`, que muda só o daemon e dados (cobra
  `person-module-rebuilds-only-the-daemon`).

### Barras, ponteiro e rolagem

- **Um controle é pego onde é desenhado.** Uma função diz onde ele ficou, e a pintura e o acerto do
  ponteiro leem a mesma resposta (`paint::bar_box`; cobram `a-bar-is-grabbed-where-it-is-drawn` e
  `the-pointer-lights-what-the-click-takes`). Numa linha de várias linhas, o controle é pego só na
  linha em que foi desenhado (`player_rows::controls_under`).
- **O ponteiro acende só o que o clique toma, linha a linha** (ou célula a célula): a vista dá uma
  resposta por linha (`Row::takes`, `View::takes`), lida pelo `Row::takes` da região, pelo
  `Content::takes` da janela e pelo próprio clique — nunca uma para a vista inteira. A lista da porta
  acende e entrega o clique só onde ela diz, e o rodapé acende campo só em linha que toma clique
  (cobram `the-pointer-lights-row-by-row` e `the-pointer-lights-what-the-click-takes`).
- A coluna da barra é dela enquanto ela é desenhada: toda pintura de barra anota a trilha
  (`canvas.bar` nas portas, `bars_drawn.record` no rodapé), o ponteiro sobre a trilha sai das linhas
  e as faixas param onde a trilha começa (`bars.rs`; cobra `a-bar-owns-its-column`).
- Barra nenhuma desenha por cima do conteúdo: a que rola na vertical toma uma célula, a horizontal
  toma uma linha, e só quando existem. A faixa de seleção para uma célula antes da barra
  (`Pen::keeping`).
- Toda janela que rola desenha a própria barra, e a barra é um controle (`chrome::list_bar`; cobra
  `every-list-window-has-a-bar`). A roda tem uma aritmética só: a linha é a unidade, o passo da roda
  é dividido nela e a sobra fica guardada (`wheel::axis_lines`, `wheel_arrived`, `wheel_lines`).
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
  nasce nele. A pergunta, o menu ou o consentimento que um clique abre nasce sob o ponteiro: o
  primeiro clique numa linha dele só a seleciona — a do cursor inclusive — e o segundo responde
  (`aim::Aim`, `files::Picker::aimed`), senão um clique duplo responde por quem não escolheu. O que
  aparece sozinho (o cartão da faixa, a faixa do cérebro) também: um clique escolhe a ação, e só o
  clique na escolhida age.
- Recusa é um valor com frase, nunca uma ausência: na tela, painel vazio e painel recusado são
  iguais.
- Opção que ainda não faz nada diz isso na tela, em vez de parecer funcionar.
- Nada que o sistema faça sozinho pode perguntar: não há agente polkit na sessão, toda chamada leva
  `auth.no_user_interaction`, e nenhuma ação automática usa `sudo` ou `pkexec`. Um gesto da pessoa
  que precisa de root, de qualquer tela, é perguntado na barra do sudo do rodapé (`sudo_bar.rs`) — a
  senha, um toque de chave ou um dedo —, nunca na linha da porta: a janela manda o gesto pelo
  soquete do rodapé, espera fora do laço e diz o veredito na própria porta (rodapé morto é frase,
  `Why::NoFooter`). Ele passa pela porta do sudo, e só por ela: `ronoctua_security::authority`,
  feature `authority`, ligada só pelo rodapé e pela central de segurança (que pergunta pelo
  `has_helper`), linkada num programa só, o shell, e chamada só pelo `sudo_bar.rs` (cobra
  `sudo-door-is-one`). A porta lê só as próprias sentinelas, nunca o texto de um terceiro, e escreve
  a senha só depois da sentinela da senha (cobra `sudo-door-reads-its-own-sentinels`).
- O que se digita e não vai ao log é dito pela PERGUNTA (os campos `secret` e `unlogged` da
  `line::Line`), nunca pela vista que perguntou, que pode fechar antes do `RET`.
- **Nada fala com a rede antes de a pessoa permitir o host.** Toda requisição leva um `Pass` que só
  o `ronoctua-consent` cunha, a partir do `consent.json`; sem ele, a porta desenha a linha de
  consentimento (`consent_rows`) e não pergunta. Uma fonte que a pessoa configurou é o próprio
  consentimento, e é gravada lá como qualquer outro.
- Um evento do sistema (privacidade, Bluetooth e redes, carregador, programa instalado) é dito no
  minibuffer (`events.rs`); nem a primeira leitura de cada fonte nem uma leitura que não respondeu é
  evento. Ele vira notificação só quando a linha está ocupada ou a fila dela já tem quatro, e o
  corpo abre dizendo por quê (`notif-minibuffer-was-busy`); um evento contrário substitui o que
  ainda espera.

### Comandos e teclas

- Todo comando tem id `kebab-case` e chave `cmd-<id>`; o registro recusa os dois divergirem. Os
  feitos do compositor são `wm-<nome>`. Uma sequência não pode ser comando e prefixo ao mesmo tempo
  (`keymap-no-collision` derruba o build). `LEADER :` alcança qualquer comando pelo nome.
- **As teclas de um crate são dele** (um `bindings::Contribution` ao lado da vista), juntadas só
  pelo binário (`keymap::CONTRIBUTIONS`; cobra `every-contribution-is-assembled`).
- Grupos da leader: `s` é o sistema (`s q` as saídas, `s e` a segurança — `s e o`/`s e l` o
  `/private`, `s e c` cifrar, `s e q` o pós-quântico —, `s o` os containers, `s x` o Incus, `s z` o
  pendrive do instalador, `s v` os pertences, `s r` o rodapé), `w` é janela e buffer (são a mesma
  coisa, inv. 4), `m` é miscelânea — o que não pertence a outro grupo (`m i`, os ícones; `m a`, as
  contas LLM) —, `c` é o clipboard (`c c`, `c w`), `d` é o acesso rápido (`LEADER SPC` o abre
  flutuando), `=` a calculadora, `v` o video-metrics, `/` é desenvolvimento (ferramentas para quem
  constrói a distro: `/ r`, o redimensionador de tela, e `/ c`, que clona este repositório) e `,` é
  o grupo do buffer em foco. O relance (`world-glance`) é `g`. Numa janela de buffer, a tecla local
  `, r` é o acorde `C-r`; letra sozinha é texto.
- **Três conjuntos**, e a LEADER repetida na raiz anda por eles: `LEADER` é o padrão,
  `LEADER LEADER` o das janelas e `LEADER LEADER LEADER` a árvore inteira, e a seguinte volta ao
  padrão (`bindings::Set::next`). Um toque de `Ctrl` (`leader::Tap`) na raiz gira os modos das
  barras; `Alt` não escolhe nada, e botão, roda ou dedo no meio fazem acorde. A frase que ensina uma
  sequência a tira de `bindings::taught`, que parte do padrão; a constante ao lado do binding é da
  árvore inteira, e colada depois de `bindings::label()` ensina, sob um grupo dobrado, uma tecla que
  faz outra coisa.
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

- **O cargo planeja, o Nix constrói**: `nix/rust/plan.json` é o grafo de unidades do cargo,
  commitado (`nix run .#rust-plan` depois de mexer num `Cargo.toml`; um plano velho é recusado), e
  cada unidade — lib, programa, teste, clippy, rustfmt, terceiro — é uma derivation
  (`rust.{release,loop,test}`; cobra `our-crates-are-built-unit-by-unit`).
- **A classe de uma unidade é medida** (leve, média, grande ou pesada, do `metrics/peak-kib`: o pico
  do que o build inteiro segura e o orçamento paga — memória anônima, shmem, swap e tabelas de
  página, nunca a página de arquivo —, a um núcleo) e mora só em `nix/rust/unit-classes.json`:
  `nix run .#rust-peaks -- --write` a regrava depois de um build (com a folga; e depois de um lock
  novo, sem build), e uma unidade que passa da fatia da sua classe é recusada; medida em outros
  núcleos, é só dita (cobram `unit-classes-fit-the-measures` e
  `unit-classes-name-units-of-the-index`). O plano leva a fatia em MiB em cada linha: quem o lê é o
  programa do sistema em uso.
- **Cada programa é a sua unidade** (`rust.bins.<bin>`, `rust.brain`…), com o papel que o crate
  declara, e só `ship` chega a uma máquina (cobra `every-bin-has-a-role`); um módulo nomeia o
  programa que roda, nunca `${rust.shell}/bin/<outro>`. O perfil de release é o medido (cobra
  `release-profile-is-the-measured-one`): mudá-lo é medir de novo com o `.#profile-bench`.
- **Um crate declara o que lê** em `[package.metadata.ronoctua]` (`env`, `run-env`, `data`,
  `beside`, `bins`…): ler um `RONOCTUA_*` não declarado falha, e uma declaração que ninguém lê é
  recusada (cobra `crate-reads-are-declared`); `data` nomeia o que o crate compila, nunca a pasta.
- **A base do shell (kit, porta, report, keep, work, inspect) e o rodapé não nomeiam
  funcionalidade**: o que duas usam desce para a base, e uma funcionalidade fica sobre outra só onde
  o desenho diz (cobram `the-shell-foundation-names-no-feature`, `footer-names-no-feature` e
  `a-feature-stands-on-the-foundation`). As palavras de um daemon que a vista e o rodapé dizem ficam
  numa folha `-words`, nunca no kit.
- Guarda de teste numa biblioteca do shell é `cfg(any(test, feature = "test-support"))` (`cfg(test)`
  vale só no próprio crate), e quem fica sobre ela liga `test-support` só nos `[dev-dependencies]`
  (cobra `shell-tests-keep-the-machine-guards`).
- Dado a que um binário é chaveado (`rust.keyed`: paleta, textos, catálogo) vai no embrulho com
  `--set`, nunca `--set-default`; quem roda um programa sem embrulho exporta `rust.keyed`.
- Caminho num argumento de comando vai por `region::path_argument`, nunca por `to_string_lossy`.
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
- Uma linha de JSON num soquete é montada inteira e escrita com UM `write_all`: serializada direto
  no soquete, cada token é uma escrita, e um leitor parado por um instante recebe meia linha, cortada
  pelo prazo de escrita.
- Nada bloqueia a thread do laço de eventos: `systemd-run --scope`, HTTP, varredura de rádio e ICMP
  vão para uma thread.
- Programa chamado pelo sistema (unidade systemd, regra udev, script gerado) é chamado pelo caminho
  no store, nunca pelo nome: o `PATH` de uma unidade é mínimo e a falha aparece longe da causa. O
  caminho chega por `option_env!` (posto em `nix/rust.nix`) ou pelo ambiente.
- Segredo nunca vai no `argv` nem na query string (a exceção é o SubDL, que só aceita assim, e a tela
  diz): vai num arquivo criado já `0600` (`create_new` + `mode`) — um `chmod` depois deixa o arquivo
  legível entre as duas chamadas (cobra `stream-key-is-never-an-argument`).
- Processo filho que segura um dispositivo (microfone, câmera) roda sob `setpriv --pdeathsig KILL`,
  além da guarda de `Drop`.
- Todo filho que o rodapé sobe (um programa, uma porta, uma coluna) passa por `watchdog::unshared`:
  nasce sem as variáveis do watchdog do rodapé, que o `NotifyAccess=main` recusaria.
- Grandeza que cruza uma fronteira leva a unidade no nome ou no tipo.
- Fallback silencioso precisa de check: valor desconhecido é recusado, não vira um padrão plausível.
- Toda regra polkit nossa é estreita — uma ação, os verbos, uma unidade, um usuário — e tem check
  próprio, além do censo `polkit-rules-are-the-ones-we-know`. Condição que mistura `&&` e `||`
  leva parênteses: sem eles, a última alternativa libera qualquer ação para qualquer sujeito
  (aconteceu com a regra do resgate).
- Todo cache tem limite (`assets/cache.nix`). A chave de um cache leva tudo o que muda o desenho —
  a paleta (tema e página, `theme::palette()`), tamanho, escala, versão do formato —, senão o
  código fica certo e a tela, errada.

## Armadilhas

### Nix e build

- **Nada de build dentro da árvore** (custou 793 GB): onde o cargo escreve é o
  `nix/cargo-dirs.nix`, sob o cache e com a versão do `rustc` no caminho — o devshell e o `.#dev`
  exportam as duas variáveis, a sessão só o `CARGO_BUILD_BUILD_DIR` (cobram `lean-tree` e
  `deps-layer-is-lean`). Nunca use `${../.}`.
- **O `build-dir` não é o `target/`.** O `{workspace-path-hash}` vira dois níveis (`ab/cdef…`), com
  o `CACHEDIR.TAG` na raiz deles, e o executável de teste mora lá; no `target/` só fica o que o
  cargo promove (binários, por hard link). Em `${VAR:-…}` a `}` do template fecha a expansão: use
  `if`. As unidades não exportam `CARGO_TARGET_DIR`: teste que o lê no Nix cai no fallback. E o
  `target/` é um só para todo checkout: copie de lado o binário com que vai fotografar.
- **Uma derivação referencia o que os bytes dela citam.** Um `option_env!` com caminho do store
  também é referência, e quem compila a paga: ícones e buffers vêm do ambiente. E bitcode esconde
  referências: o caminho que uma lib de release assa só aparece no programa ligado, que recebe os
  valores dos crates dentro dele (`cited`; cobra `programs-hold-what-they-cite`).
- Um programa nomeado (`rust.<programa>`) carrega `passthru.bin`, o nome dele: no `path` de uma
  unidade, o `makeBinPath` o toma pela saída `bin` e escreve `<programa>/bin`, relativo, e nada roda
  (`setpriv: failed to execute`). Passe `"${pkg}"`, o caminho, como o `nix/modules/first-boot.nix`.
- Um programa que acha outro ao lado de `current_exe()` só acha CÓPIAS no mesmo `bin/`, e uma cópia
  não cita a unidade de que saiu (cobra `no-loop-build-reaches-a-machine`). O `--shrink-rpath` do
  fixup tira o RUNPATH de um `dlopen`: ele vai no `postFixup`. O que um build vai construir é o dry
  run (`will be built:`), não o fecho de `.drv`, e o `nix` do root constrói no cgroup de quem chama.
- Gerador cita script por arquivo, nunca `${./scripts}` (cobra `generators-cite-their-scripts`).
  `exportReferencesGraph` de um `.drv` pede as SAÍDAS do grafo; `lib.hasInfix` e `builtins.match`
  com `.*` estouram a pilha num texto grande; `lib.trim` e `builtins.match` perdem o contexto.
- **Geração só sai por contagem, e só a do sistema.** `nix-collect-garbage -d` ou
  `--delete-older-than` como root apaga as gerações de TODO perfil, `per-user` inclusive — o que a
  garantia do dono proíbe. Quem apaga geração é o `ronoctua-system-generations` (as N mais novas, a
  atual, a em execução, a do boot — especialização inclusive —, a de onde a última troca saiu e toda
  que o menu de boot lista ficam; nenhum outro perfil), e o menu de boot lista no máximo N (cobra
  `nix-gc-keeps-generations`). O resto é lixo: o piso do Nix (`min-free`) e a coleta da atualização.
  Cada parte guarda a geração atual e a anterior, por contagem (`--delete-generations +2`), e só o
  `ronoctua-parts` apara o perfil dela.
- **ISO velho não fica no disco.** Cada build e cada iteração de teste de VM do instalador deixa um
  ISO de ~5 GiB que nada enraíza (em 2026-10-23 eram oito, 38 GiB, com o pool em 93%). Todo app que
  constrói ISO (`.#iso`, `.#release`, `.#gate`, `.#most`, `.#sized-build`) sai pelo
  `nix/iso-sweep.nix`, que apaga os ISOs sem raiz, com os drivers de teste que os seguram, e guarda o
  mais novo de cada nome; um ISO que se quer guardar leva raiz (`--out-link`). ISO se constrói por
  esses apps, nunca por um `nix build` solto; à mão, `nix run .#iso-sweep` (cobra
  `iso-sweep-keeps-what-it-must`). O `.#release` apaga, ao terminar, a pasta em que corta o ISO em
  assets, e o `.#fetch-iso` pergunta o espaço antes de escrever. Nunca um `nix-collect-garbage` geral
  para abrir espaço: ele leva junto o que as baterias reaproveitam.
- **O Nix pula sem aviso um `!include` que não consegue ler**: um piso em arquivo `0600` é uma
  máquina sem piso, e um arquivo ausente também não é erro (cobra `store-has-a-floor`).
- **Interruptor que não interrompe**: um `mkIf` desligado não garante que o pacote saiu, porque ele
  pode entrar por outra porta. Meça com `nix why-depends`. E meça de novo antes de confiar num
  tamanho escrito num comentário (o `notoExtra` prometia poupar 1 GB e poupava 49 MB).
- Dentro de `''…''` valem duas linguagens: `#` e `//` são do script, mas `${…}` é do Nix e vale até
  em linha comentada; `` ` `` é substituição de comando até em comentário; e qualquer `''` fecha a
  string, inclusive dentro de um comentário ou de um literal Python (escapes: `'''`, `''$`, `''\`).
- `nix flake check` sem `-L` não mostra log: silêncio não é sintoma.
- Flake avaliado por CAMINHO (`builtins.getFlake (toString ./.)`, `path:`) copia a árvore inteira
  para o store, ignorados inclusive — gigabytes por avaliação. Avalie `git+file://`.
- Download sem versão é um hash com prazo de validade: o endereço regenera o arquivo e a cópia no
  store some na próxima coleta de lixo. Endereço vivo vira captura (`snapshot` em `nix/atlas.nix`;
  cobra `downloads-are-pinned`), e `nix/kept-sources.nix` é a raiz de GC de todo input e download.
- `cannot download … from any mirror` não é arquivo removido: o `fetchurl` repete sem retomar. Os
  arquivos grandes são baixados pelo `nix/prefetch.nix`, que o gate roda antes de compilar.
- `rustc` morto por `signal: 15` sem diagnóstico é o `earlyoom` do host: o que estoura é jobs ×
  cores, e a conta é uma só (`nix/build-sizing.nix`). O Nix não pesa derivation: cada unidade é um
  `nix build` próprio, começado pela fatia da classe dela (`sized_units`). Teste
  de VM segura a memória que declara (`virtualisation.memorySize`) enquanto roda, e as baterias o
  rodam numa fase própria, somando o declarado; e avaliam antes, construindo pelos `.drv`
  (`nix/gate.nix`), porque cada teste de VM avaliado segura ~260 MiB.
- No bash, `$EPOCHREALTIME` sai com a marca decimal do locale (vírgula no pt_BR): só os dígitos
  entram numa conta (cobra `sized-units-keep-time-in-any-locale`). E uma trap de `EXIT` lê as
  variáveis no escopo de onde o `exit` foi chamado: um `local` de mesmo nome numa função a esconde.
  Uma trap de sinal espera o comando em primeiro plano terminar (um `$(…)` também): o que o sinal
  tem de parar roda com `& wait $!`.
- `teste && comando` como última instrução do corpo de um laço derruba o script inteiro sob
  `errexit` + `pipefail` (que o `writeShellApplication` liga); um `ls` sem correspondência faz o
  mesmo. Use um `if` sem `else`. O check de um laço que varre diretório tem de plantar arquivos,
  incluindo o caso que falha.
- nixpkgs 26.05: `noto-fonts-emoji` → `noto-fonts-color-emoji`, `nixfmt-rfc-style` → `nixfmt`,
  `xorg.libX11` → `libx11`, `pkgs.system` → `pkgs.stdenv.hostPlatform.system`. Atributo dinâmico
  definido em duas sentenças é erro de avaliação: use um attrset só.
- As fontes (4,1 GB) e as gravações dos sons ambientes são asset de Release, nunca git. A face do
  sistema e os sons são input `file+https://` — senão o Nix infere tarball e descompacta —, e as três
  variantes da família, que só o full leva (a parte `fonts-full`), são downloads fixados com o mesmo
  hash NAR (`fontAssets`, `recursiveHash`): um input é buscado por toda avaliação, e teria de estar no
  ISO. `/etc/nixos` e `/etc/fonts/conf.d` são somente leitura (use
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
- O `tracing` põe aspas em valor `Debug`: procure `engine="alacritty"`, não `engine=alacritty`. O
  alvo é o crate do código que loga, e um `[[bin]]` é um crate com o nome dele: o núcleo e os
  módulos do brain logam sob `ronoctua_brain_daemon::…`, a biblioteca e o `main` do daemon sob
  `ronoctua_brain`.
- `pub mod key;` inserido entre um `#[cfg(…)]` e o item dele leva o atributo. E o `cargo clippy`
  local de uma seleção que junta uma biblioteca e quem a liga com `test-support` não vê o lado do
  produto dela: quem vê é o `rust-clippy`.

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
  (waveform do ffmpeg, `currentColor` de SVG) leva a paleta (`theme::palette()`) na chave do
  cache, senão a textura velha fica na GPU — e o `image::Images` não remove textura por chave.
- Só um teste chama `theme::set` e `theme::set_ground` (no `ronoctua-tokens`): os dois mudam o
  mesmo índice do processo, e os testes rodam em paralelo.
- Consumidor Nix de cor lê o store (`${themed}/…`), nunca o `colors.nix`, que só tem receitas.
- O nome de um glifo Nerd Font não leva `nf-` (a classe CSS leva). O console não usa TTF, e sim PSF,
  e o kernel recusa calado uma PSF que não tenha 256 ou 512 glifos e apaga com a posição 32 (cobra
  `console-font-is-what-the-kernel-takes`). Uma cor de 24 bits chega ao VT com um bit por canal:
  uma TUI no console pinta pelos 16 slots (`ronoctua-vt`), e só uma VM a vê (`installer-console-vm`).
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
  `KeyboardHold` (`surface/mod.rs`) perdoa um `leave` por aquisição; tratar o primeiro como perda
  fecha a leader no instante em que ela abre.
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
- Uma porta por processo. `theme::reload()` e `toolkits::reload()` respondem "mudou?" uma vez por
  processo, então a segunda porta do mesmo processo nunca repinta com tema ou tamanho de texto
  novo (é por isso que o redimensionador são três processos).

### Sistema

- `systemd.oomd.enable` já é `true` e não vigia nada: quem arma é o `ManagedOOMMemoryPressure=kill`
  numa slice. A `session.slice` é irmã da `app.slice`. `memory.min` é limitado por todos os
  ancestrais (quatro drop-ins, senão vale zero). `OOMScoreAdjust` em unidade de usuário é ignorado:
  suba o app com `choom`.
- Processo morto por OOM não gera core dump: "a janela fechou sozinha" com `coredumpctl` vazio é OOM
  até prova em contrário (`journalctl -b | grep oom-kill`). Bateria pesada roda em jaula:
  `systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=512M`.
- `systemd-run --scope` é síncrono: esperá-lo no `calloop` congela o rodapé.
- Unidade de usuário nasce com o `PATH` mínimo do systemd, e nele o `Exec=` de um app do
  `~/.nix-profile`, o `flatpak run` das exportações e todo passo `binary` da atualização não
  existem. O rodapé, a fila e as duas unidades da atualização têm `enableDefaultPath = false` e
  herdam o `PATH` da sessão (cobra `session-units-inherit-the-session-path`).
- Unidade com sandbox (a do Tor é um chroot) só enxerga o que foi montado nela, e só teste de VM vê
  isso — teste unitário nenhum. Um `ReadWritePaths=` que não existe derruba a unidade antes de ela
  rodar (226/NAMESPACE): quem escreve em `%t/…` declara `RuntimeDirectory`.
- O Chrome roda como a pessoa: o que a política dele aponta por `file://` (o `update.xml`, o `.crx`)
  tem de estar num diretório que ela atravesse. Com `/var/lib/ronoctua-browser` em `0700`, a
  instalação forçada esperava para sempre, sem log; a chave se protege pelo próprio modo (`0600`).
- Configurar uma unidade pelo nome errado cria uma unidade fantasma que não configura nada (é
  `dbus-broker.service`, não `dbus`).
- `grep -r` não segue symlink, e todo `/etc` do NixOS é symlink: use `-R`.
- No btrfs, `nodatacow` e `compress` são do sistema de arquivos, e a primeira montagem vence sem
  avisar; a hibernação depende do NOCOW (`lsattr` → `C`). PCR 15 exige UKI medida.
- Snapshot que contém `/nix` ou um cache segura tudo o que a coleta e a varredura liberam;
  `nix/modules/filesystem.nix` recusa o alvo e faz de cada `~/.cache` um subvolume (cobra
  `snapshots-skip-the-store`). Arquivo NOCOW não aceita reflink.
- Todo `switch` reinicia os targets ativos: um `oneshot` sem `RemainAfterExit` que um target quer
  roda de novo a cada `switch`, e um com `RemainAfterExit` não roda de novo com `systemctl start`.
  E o switch sai com 4 se uma unidade que ELE iniciou falhou (as falhas antigas ele reseta antes): a
  `nixos-rebuild` chama de falha uma troca que aconteceu. Unidade que o switch dispara e que não pode
  derrubá-lo leva `-` no `ExecStart`.
- Unidade que só um gesto da pessoa parte, e que não se reinicia sozinha, leva
  `startLimitIntervalSec = 0`: o teto padrão (cinco partidas em dez segundos) não protege de laço
  nenhum ali e recusa o sexto gesto como `start-limit-hit` (`ronoctua-stick-write`,
  `ronoctua-person-apply`; cobram `stick-root-is-narrow` e `person-apply-rule-is-narrow`).
- `st_dev` não vê montagem por bind do mesmo sistema de arquivos: quem apaga por árvore pergunta ao
  kernel onde começa uma montagem (`STATX_ATTR_MOUNT_ROOT`).
- `pkill` e `pgrep` casam o próprio comando. `git checkout <arquivo>` apaga o que não foi commitado
  (a cópia fica no store: `ls -dt /nix/store/*-source`).

### Testes, checks e fotos

- Três baterias: `quick` (os escolhidos), `most` (tudo menos o que constrói uma instalação) e `gate`
  (tudo). O `tests/slow.nix` diz o que sai do `most` (cobra `test-tiers-are-total`).
- Um check prova o comportamento pedido. Check que confere o código como ele está escrito, ou a
  mensagem que ele imprimiria, passa num marco vazio.
- **Um teste dá o mesmo veredito em toda máquina**, ociosa ou ocupada pelo gate, em qualquer sistema
  de arquivos. Tempo: nenhum teste segura um tempo abaixo de um segundo — conte o trabalho (os passos
  do SQLite), limite o regime (≥ 1 s) ou meça contra uma `Yardstick` cronometrada ao lado (cobra
  `no-test-holds-a-time-under-a-second`). Tamanho é comprimento, nunca blocos: o ZFS diz 1 KiB por
  2 MiB recém-escritos, e nenhum `sync` o apressa (cobra `sizes-are-lengths`). Um nome que não é
  UTF-8 só vai ao disco em `a_room_for_any_name` (`/dev/shm`): o ZFS com `utf8only` o recusa (cobra
  `a-name-that-is-not-text-has-a-room-for-it`). Um check de dimensionamento planta a máquina
  (`--meminfo`, `--cpus`, `--ceiling`), nunca lê a memória livre de quem constrói. Um teste de VM
  que segura um trabalho no meio limita o ritmo dele no kernel (o `io.max` da unidade, a origem fora
  do cache) e confere que o limite pegou, em vez de correr contra ele (`transfers-vm`). E o produto
  também não mede a máquina para escolher um parâmetro: o `age` sozinho escolhe o fator do scrypt
  pelo relógio (1 GiB numa máquina rápida, e uma abertura que a lenta recusa), e ele é constante
  (cobra `scrypt-work-is-the-same-everywhere`).
- `cargo test` precisa dos programas externos: sem as variáveis `RONOCTUA_*`, os testes de ffmpeg
  saem calados e não testam nada. O caminho vai na tabela `testEnv` do `nix/rust.nix` (nunca em
  `generated`, que chega aos programas), e o crate que o lê o declara (`run-env`, `test-env`).
- **A granularidade é um check** (`tests/build-granularity.nix`): a edição simulada na avaliação
  (`crateSourceOverrides`) contra o plano, nos dois sentidos. Um check de dado prova que a edição
  chegou ao build (o embrulho mudou), senão compara o build com ele mesmo.
- Um teste só, sem recompilar: `rust.testsOf { … }`. Uma unidade que TEM DE FALHAR é ela mesma com o
  veredito invertido (o `refused` do `tests/brain-split.nix`), o comando do executor sozinho no
  `if`: numa condição o `errexit` não vale.
- O programa de mentira de um teste é escrito por um `/bin/sh` filho, nunca por este processo: um
  arquivo que ele tem aberto para escrita quando outra thread de teste faz `fork` fica com o filho
  até o `exec`, e rodar o arquivo nessa janela é `ETXTBSY` ao acaso (`write_executable` no
  `ronoctua-transfers` e no `ronoctua-software`).
- Check que lê fonte abre com o `pyPrelude` de `tests/checks.nix` (cobra
  `checks-do-not-pass-by-coincidence`): `code_of`, `uncommented`, `hashless`, `body_of`, `scope`,
  `shut_at`, `floor`/`must`, `without_tests` e `outside_tests`, e declara a pergunta que faz
  (`# FILE-WIDE:`, `# PROSE:` ou `# PARSED:`). O shell se lê pelo caminho dentro do `src/`, nunca
  pela pasta do crate (`shell_file("door/mod.rs")`, `shell_files`, `shell_manifests()`).
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
- **Olhe antes de dizer "pronto".** `nix run .#png` desenha um buffer, ou uma porta nativa
  (`--door`), em 0,2 s. Verde não é prova: teste de estado não vê espaço em branco, sobreposição nem
  faixa. Se um buffer ou uma porta não puder ser fotografado, a ferramenta é que está incompleta.
- **Foto de feature**: escreva uma cena `.py` para a feature, fora do repositório, e rode
  `nix run .#shot -- <cena.py>`. O palco (VM, compositor, rodapé e os ajudantes `control`, `notify`,
  `shot`, `crop`, `changed`, `has_colour`…) é o `nix/shot.nix`; as fotos vão para
  `~/.cache/ronoctua-distro/shot`, que é apagada a cada rodada. Nenhuma cena fica no repositório.
  `--apps` troca o palco pelo `nix/shot-apps.nix` (sem rodapé, com Flatpak, o portal e a internet):
  é onde se vê um app do Flathub.
- **Uma coisa pesada por vez.** Pesado é construir o grafo de unidades no Nix, compilar o shell no
  `cargo`, um teste de VM e uma bateria. A jaula não alcança o `nix build`, que roda no daemon: o do
  grafo é `nix run .#sized-build -- …` (o link de release do shell chega a ~3,9 GiB), o de uma
  derivação pequena leva `--max-jobs 1 --option cores 4`, e nenhum `cargo` roda junto. Entre agentes
  também: um compila e o outro espera, e ninguém compila durante o `.#most`.
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
