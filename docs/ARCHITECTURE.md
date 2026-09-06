# Arquitetura — o porquê

Este documento existe para impedir que uma decisão já tomada seja refeita. Cada seção diz o que foi
decidido e **por quê**, e o que foi descartado no caminho.

Junto de `CLAUDE.md` (o contrato do agente), `INSTALL.md` (instalar no disco) e `DEVELOPMENT.md`
(rodar o ambiente de trabalho), ele deve bastar para alguém continuar o projeto sem nenhum histórico
de conversa. O que ainda **falta** está em `PENDING.md`, não aqui.

Não há relato de depuração aqui, e não há changelog. Quando um bug ensinou algo que se repete, o
ensinamento virou uma armadilha no `CLAUDE.md` ou uma frase sobre o desenho atual; a história de como
o sistema chegou até aqui não sobreviveu de propósito.

A interface toma emprestadas algumas ideias e alguns nomes de editores de teclado — modeline,
minibuffer, a lista que aparece quando a tecla líder é apertada. É empréstimo de vocabulário, e não
um compromisso: nada aqui precisa se parecer com nenhum outro programa.

---

## Sumário das decisões

| Área | Decisão | Alternativa descartada |
|---|---|---|
| Sistema | NixOS mínimo, tudo por flakes | qualquer coisa imperativa |
| Compositor | Niri upstream, sem fork nem patch | fork com integração direta |
| Chrome | rodapé de duas linhas, só | barra, dock, popup de canto |
| Sistema de arquivos | btrfs | ZFS (out-of-tree), bcachefs (fora do mainline na 6.18) |
| Boot | systemd-boot + lanzaboote | GRUB |
| Renderer | wgpu + cosmic-text + glyphon | vello (não há carga vetorial) |
| Buffers nativos | WASM Component Model, display list retida | pixels, ou plugins nativos |
| i18n | Fluent | gettext |
| Fontes | assets de um Release, quatro inputs fixados por hash | dentro do repositório |

---

## 1. Fontes, glifos e ícones

### Fontes

#### A célula é 0,600 em, exata

Monoespaçamento exato, **inclusive nos 10.473 glifos Nerd Fonts** (variante Mono = ícone de uma
célula). Na largura normal o avanço é 600 de 1000 `unitsPerEm`.

**Isto é a fundação do layout do rodapé.** Com a célula exata, posicionar segmentos é aritmética de
inteiros — não é preciso medir texto, e é o que compra o orçamento de "um frame" para which-key e
completude. Se a fonte for trocada por uma build com outras métricas, o check `fonts-metrics`
falha e obriga a revisar isto antes de o rodapé desalinhar em silêncio.

#### O defeito das 27 variantes, e por que o conserto óbvio quebra

Uma *face* é um arquivo de fonte: uma combinação de peso, largura e inclinação — Regular, Bold,
SemiExtended Italic. São 135 delas, e daqui em diante o texto as chama de **variantes**.

O patcher do Nerd Fonts engoliu o "Semi" de "SemiExtended" para dentro do nome de família, jogando
27 das 135 variantes patcheadas em **nove famílias inventadas**. Efeito: pedir
`NoctuaIosevka Nerd Font Mono` nunca devolvia SemiExtended.

A armadilha está em consertar do jeito óbvio. As 27 SemiExtended têm `nameID 17` ∈ {`Extended`,
`Extended Italic`, `Extended Oblique`}; as 27 **Extended verdadeiras** têm exatamente os mesmos três.
Hoje não colidem só porque estão em famílias diferentes — ao unificar a família, colidem.

Por isso `nix/scripts/fix-font-names.py` **não remenda os 27 arquivos**: recomputa a nomenclatura
das 135 como função pura das métricas (`usWeightClass`, `usWidthClass`, `fsSelection`), e termina
afirmando 1 família e 135 pares (família, subfamília) distintos.

#### Cinco derivações, não uma

Um pacote único de 4,1 GB entraria no closure de toda máquina **e do ISO**, e um ISO instalável não
fecha com 2,1 GB de fonte.

| derivação | conteúdo | tamanho | onde |
|---|---|---:|---|
| `noctua-iosevka-core` | 4 arquivos RIBBI, largura normal | ~62 MB | **ISO, initrd, TTY, instalador** |
| `noctua-iosevka-ttf-nerdfonts` | 135 TTF patcheados | 2,1 GB | sistema instalado |
| `noctua-iosevka-ttf` | 135 TTF sem patch | 1,4 GB | sistema instalado |
| `noctua-iosevka-woff2{,-nerdfonts}` | 270 WOFF2 | 626 MB | sistema instalado |
| `noctua-iosevka` | meta, junta os quatro | 4,1 GB | `fonts.packages` |

"Nada é descartado" vale para o **sistema instalado**. A mídia leva o subconjunto porque é o que a
instalação precisa ter à mão — e **não porque exista um teto**. Havia um, de 4 GiB, e o número nunca
foi arbitrário: é o tamanho máximo de ARQUIVO do FAT32, e ele só mordia quando o `.iso` era
**copiado** para o pendrive (Ventoy, ou uma ESP de onde a UEFI dá boot no arquivo). Gravado com
`dd`, que escreve setores crus e é o que o `INSTALL.md` manda fazer, não há limite nenhum. O dono
tirou o teto e escolheu o `dd`; o `iso-is-measured` continua medindo, e o que ele recusa agora é
uma imagem **pequena demais** — `storeContents` que avalia para nada produz um ISO perfeitamente
válido e perfeitamente inútil, e essa falha se parece com sucesso.

#### O TTY não consome TTF

O console do Linux quer PSF/PSFU, bitmap, máximo de 512 glifos. Instalar uma fonte de console alheia
faria o TTY parecer com outra coisa — o oposto do que a regra queria. `make-console-psf.py`
rasteriza um PSF2 **do arquivo real** com FreeType: 498 codepoints, com folga sob o limite.

#### Duas cadeias de fallback, uma fonte

O renderer **não usa fontconfig** — se usasse, quebraria no instalador e em ambiente mínimo. Ele
resolve por caminho absoluto via `fontdb`, com `noctua-iosevka-core` embutida no binário. Mas as
duas cadeias precisam **concordar**, senão o mesmo texto renderiza diferente dentro e fora dos
nossos buffers; por isso `nix/fontconfig.nix` gera as duas de uma lista só, e um check compara as
escolhas em codepoints-sonda.

---

### Onde as fontes vivem, e por quê

**As fontes não estão no git.** São asset de um Release, consumido como input pinado por hash. Isso
contraria "monorepo único" e a decisão foi tomada com número na mão.

As duas licenças são declaradas aqui, e não num repositório separado: `LICENSE` (MIT) para o código,
`LICENSE-OFL.txt` para os arquivos de fonte. Um repositório só para ser dono de um Release custaria
dois remotos, dois clones e um input a mais, e deixaria as licenças implícitas em vez de escritas.

O Nix copia a árvore inteira do flake para o store a cada avaliação, e o path é endereçado por
conteúdo: **qualquer edição a qualquer arquivo grava uma cópia nova**. Com 4,1 GB dentro, seis
edições enchiam dezenas de GB.

| | fontes na árvore | fontes como input |
|---|---:|---:|
| tamanho do repo | 8,1 GB | **1,4 MB** |
| `nix flake check` após uma edição | minutos | **~10 s** |

**O que NÃO era o problema:** não é o custo de hashear (4,1 GB custam 4 s). O que justifica a
separação é a **gravação** por edição.

#### A mesma classe voltou, com o `target/`

Tirar as fontes resolveu *aquele* caso, não a *classe*. Depois o disco chegou a 44 cópias da árvore
somando **793 GB** — 42% do disco — cada uma carregando um `target/` de 18 GB.

O detalhe cruel: `nix/rust.nix` **já filtrava** via `lib.fileset`, e filtrava bem. Mas aquele filtro
roda sobre `src`, e `src = ./.` **já é** o store path com os GB dentro. Filtrar depois economiza
recompilação, não disco.

A correção é manter build fora da árvore (`CARGO_TARGET_DIR`). A guarda é o check `lean-tree`,
que mede `self` e falha acima de 20 MB — e tem fixture negativo. Sem o check, a correção se perderia
no primeiro `cargo build` fora do devshell, e o custo é invisível até o disco encher.

#### E a terceira vez foi pelo AMBIENTE, não pela árvore

A camada de dependências reconstruía sozinha, e a explicação óbvia estava errada: **o `crane`
funciona como projetado**. O `dummySrc` dele não carrega o hash da árvore — é montado de um
`Cargo.lock` limpo, de cada `Cargo.toml` **reparseado** e de um `dummy.rs` fixo, com
`unsafeDiscardStringContext`. Conferido na derivação real: o caminho do `rustFont` não aparece nela.
Editar `crates/**/*.rs` nunca a invalidou.

O que a invalidava eram as **31 variáveis que ela recebia e não usa**. `cargoArtifacts` era
`buildDepsOnly (commonArgs // …)`, e ali dentro iam os cinco `.wasm`, as três fontes, os tokens e o
i18n. Nenhuma é lida por dependência de terceiros — o `crane` substitui até os nossos `build.rs` por
stubs, então eles nunca rodam ali. Mas o Nix endereça por ENTRADA: com `RONOCTUA_BUFFER_MEMORY` no
ambiente, editar um buffer mudava o hash e pagava **4m46** de compilação por um valor que ninguém lê
— e os cinco `.wasm` eram dependências diretas, então a camada esperava cinco builds que não
consome.

A correção é a separação `buildBase`/`generated`: a camada recebe o que LINKA (wayland, xkbcommon,
vulkan) e nada mais. A guarda é `deps-layer-is-lean`, que lê os atributos da derivação gerada — em
tempo de avaliação, sem build — e recusa qualquer `RONOCTUA_*` ali. **Medido depois:** um `.ftl`
editado deixa o `.drv` da camada idêntico, e o do shell muda, como tem de mudar.

**Os números do laço, medidos:** dependências 4m46 (483 crates), workspace 2m15–5m46, clippy 13–16 s,
cada roteiro de VM 17–48 s (as vinte). O que sobra é caro por CONSTRUIR UMA INSTALAÇÃO — quatro
sistemas NixOS e o ISO —, e é essa a linha que separa as três baterias: `#quick` (dez escolhidos,
~1 min), `#most` (todo o resto) e `#gate` (tudo). Só o gate fecha o invariante 7; ver
"Três baterias" abaixo.

#### E a quarta foi pela REFERÊNCIA — que quem compila não devia pagar

A separação acima consertou a camada de dependências e deixou o resto de pé. Editar um **comentário**
em `buffers/memory/src/lib.rs` continuava reconstruindo o workspace, o clippy e os testes — três
derivações que não leem uma linha de buffer. Medido com `nix eval` nos `.drv`, antes e depois. Duas
causas, invisíveis pelo mesmo motivo: nenhuma das duas é lida em tempo de compilação.

1. **`option_env!` com caminho de store é referência**, e ela estava no `env` de quem COMPILA. O
   `.wasm` não entra em nada que o `rustc` leia — vai por argv ao processo filho —, mas está na
   ENTRADA da derivação, e é por entrada que o Nix endereça.
2. **`rustFont` levava `buffers/**/*.rs` no `src`**, com o comentário "são membros do workspace".
   Não são: o `Cargo.toml` da raiz tem `exclude = [ "buffers" ]` e lista cada membro à mão, então o
   cargo nunca olha lá dentro. Eram cinco pastas de fonte na compilação, para nada.

A correção da primeira **não é apagar a referência**. Ela precisa existir: sem ela o ISO deixa de
levar os componentes e a máquina alvo COMPILA cinco wasm — foi exatamente o que aconteceu com o
superpacote de ícones, e quem disse foi o `iso-carries-the-seed`. Ela **sobe uma derivação**:
`rust.shell` é o workspace embrulhado por `makeBinaryWrapper` com os cinco caminhos, e é ele que o
`ExecStart` do módulo, o laço aninhado e os roteiros de VM executam. Quem compila não conhece buffer
nenhum; quem executa conhece os cinco. Medido no fecho: o workspace passou a citar **zero** buffers e
o embrulho cita os cinco — 1,41 GiB contra 1,42.

Um embrulho e não uma variável em cada lançador porque são **nove lançadores** — a unidade de
usuário, os binds do niri, o laço aninhado, seis VMs —, e variável que um deles esquece não dá erro:
dá uma tecla que para de abrir janela. `--inherit-argv0` não é enfeite: o `nix run .#diagnostics` acha
o rodapé com `pgrep -f "bin/ronoctua-shell$"`, e sem ele todo processo apareceria como
`.ronoctua-shell-wrapped` — o diagnóstico cegaria justamente quando é ele que se está rodando.

**Medido, editando um comentário num buffer:**

| derivação | antes | depois |
|---|---|---|
| `ronoctua-0.1.0` (workspace) | reconstruía, 2m15–5m46 | **idêntica** |
| `ronoctua-clippy`, `ronoctua-tests` | reconstruíam o workspace de novo | **idênticas** |
| `ronoctua-shell` (o embrulho) | — | reconstrói, e é o que tem de reconstruir |
| o buffer | reconstruía | reconstrói |

E o que se perdeu foi a garantia que o `option_env!` dava de graça: se o build tinha a variável, o
binário tinha o caminho. É por isso que existe `shell-finds-its-buffers`, e ele compara os **dois
lados vindos de artefato real** — os nomes que os `.rs` do shell leem, e as strings que o embrulho
compilado carrega. Nome de um lado só reprova, nas duas direções: o buffer que ninguém ligou, e a
variável que parou de ser posta. Provado REPROVANDO: tirar `_LUCIDE` da tabela faz o check nomear a
que falta. `profile-minimal-closes` passou a nomear os cinco pela mesma razão que já nomeava o
superpacote.

De brinde, um defeito que estava lá desde sempre: **o binário do hot reload nunca teve os buffers**.
`nix run .#dev` compila com `cargo`, e as cinco variáveis não estavam exportadas ali — `i n` abria
janela antes da primeira edição e respondia "não consegui abrir" depois dela. Ele se escondia atrás
de quem roda `nix run .#dev` de DENTRO do `nix develop`, porque ali quatro delas existiam por acaso;
é o mesmo defeito que o comentário dos ícones descreve seis linhas acima, no mesmo arquivo. Hoje a
tabela é uma só (`rust.bufferComponents`), lida pelo embrulho, pelo devshell e pelo laço — e o
devshell escrevia quatro dos cinco à mão, faltando justamente o do editor.

#### E o hot reload nunca disparou — desde o primeiro commit

Tirar os buffers da compilação deixou o laço aninhado num estado esquisito: construir um buffer ficou
barato, e o observador do `nix run .#dev` observa `buffers/` — mas só sabia rodar `cargo build -p
ronoctua-shell`, que não reconstrói componente nenhum. Ao abrir isso, o defeito era outro e maior.

```
RONOCTUA_RELOAD_CMD='reconstruir'   # o que o dev.nix mandava
rebuild() { … }                     # o nome da função que ele define e exporta
```

O observador faz `eval` na string, recebe `reconstruir: command not found`, e engole com `|| true` —
**e o `|| true` está certo**: erro de compilação é o caso normal de um laço, e derrubar o observador
no primeiro `;` faltando obrigaria a reiniciar o desktop inteiro. Só que, com o nome errado, ele
engolia o único caminho que existia. **O hot reload não disparou uma vez desde o primeiro commit**, e
o que escondeu isso é que o rodapé que fica na tela é o do store, e funciona perfeitamente.

O `reload-reloads` não viu porque prova o **mecanismo** com um comando de fabricação própria. Faltava
prova da **fiação**, e é `reload-command-is-defined`: extrai o nome do script construído e exige que
ele seja função definida E exportada ali. Provado reprovando — com `reconstruir` de volta, acusa.

**E a segunda metade tinha de cair junto.** Consertar só a fiação seria uma regressão: o `dev.nix`
exportava seis variáveis e o build assa **trinta e uma**. O binário do `cargo` teria ficado sem
`btop`, sem área de transferência, sem `ghostty-vt`, sem terminfo — e ele SUBSTITUI o do store na
primeira edição. Os dois defeitos se escondiam: um deixava o outro sem consequência.

Hoje a tabela é uma só (`rust.generated // rust.bufferComponents`, interpolada), e
`dev-exports-what-the-build-bakes` lê os nomes dela — não uma lista escrita no check. A classe morre:
foi ela que produziu o superpacote de ícones (a lista abria com o glifo da categoria e nunca com o
ícone do app) e os cinco componentes.

Com as duas de pé, o `rebuild` passou a ter dois ramos, e quem decide é o caminho que mudou:
`buffers/` vai para `nix build .#buffer-<nome>` — não `cargo`, porque cada buffer é um workspace
próprio para `wasm32-wasip2`, com lock vendorizado e, em três deles, uma base gerada; um segundo jeito
de construir o mesmo artefato divergiria. O caminho chega por `RONOCTUA_RELOAD_FILE`, **exportada** e
não herdada por acidente de escopo — o `shellcheck` reclamou disso, e tinha razão.

Três detalhes que só aparecem em uso: o rodapé reinicia com `RONOCTUA_FOOTER_BIN` (o do store até a
primeira compilação, o do `cargo` depois), porque editar buffer reinicia sem compilar Rust nenhum; o
`--set-default` do embrulho deixa de ser teórico, já que é ele que permite ao caminho novo vencer o
assado; e se o `.wasm` sair idêntico, o laço diz que **arquivo novo precisa de `git add -N`**, em vez
de anunciar sucesso e não mudar nada na tela.

#### O formatador era a armadilha que ele mesmo documentava

`formatter` era o `nixfmt` cru. `nix fmt` sem argumento virava um `nixfmt` lendo stdin e **travava** —
sete horas de processo, uma vez. A armadilha ficou escrita no `CLAUDE.md`, e mesmo assim o hook de
pre-commit e o `check-fmt` mandavam rodar exatamente o comando que trava.

Documentar uma armadilha não a remove. Hoje o `formatter` é um invólucro: sem argumento formata a
árvore — `.nix`, o workspace e os **workspaces dos buffers**, que `cargo fmt --all` nunca
alcançou porque `buffers/` está em `exclude`. Com `--check` confere, que é o que o hook usa. A
armadilha deixou de existir por construção, e a formatação dos buffers passou a ser conferida
(`buffers-fmt`).

#### Três baterias, e por que se nomeia o LENTO

Havia dois caminhos e nada entre eles: `nix run .#quick`, ~1 min, e `nix run .#gate`, horas. Quem
está no meio de uma edição não paga o gate, então o que se verificava de fato era o `quick` — e o
`quick` **é uma lista de dez nomes escrita à mão**, que prometia "os checks que não sobem VM" sendo
que há mais de cem checks e dezenas deles não sobem VM. Nove criados num único dia ficaram de fora
sem que nada acusasse.

O custo disso não é hipotético. Nesta árvore o gate foi interrompido duas vezes, e da segunda por um
**diagnóstico errado meu**: li `cannot download ... from any mirror` como "o tarball sumiu", quando o
log dizia `Connection reset by peer` aos 100 MB e uma retomada estrangulada. Medido depois, as três
URLs respondem `200` e o arquivo baixa a 17,6 MB/s. Um degrau intermediário não teria evitado o erro
de leitura, mas teria evitado a pressa que o produziu.

Hoje são três, e a do meio é **derivada, não escrita**:

| | quanto | quando |
|---|---|---|
| `#quick` | ~1 min | no meio de uma edição |
| `#most` | minutos | antes de commitar |
| `#gate` | horas | antes de publicar — **só ele fecha o invariante 7** |

**A escolha que importa é qual lado se nomeia.** Nomear o rápido é o que já existia, e falha em
silêncio: um check barato esquecido não faz ruído nenhum. Nomear o lento (`tests/slow.nix`) faz o
esquecimento cair para o lado seguro — quem inventar um check caro e não o registrar torna o `most`
mais lento, e isso se percebe no relógio. Um check novo entra na bateria de todo dia sozinho, que é o
padrão certo.

**O critério é estrutural, e a exceção é medida.** O que sai do `most` constrói uma instalação — um
`system.build.toplevel` ou o `isoImage` — ou é um `mN`, que é a pergunta "o marco fechou?". Não é
"pareceu demorado": sete candidatos que SOAM caros foram cronometrados com `--rebuild` e ficaram
(`disko-vm` 77 s particionando de verdade, `oom-vm` 49 s com duas máquinas, `tpm-vm` 22 s emulando um
TPM), porque as vinte VMs da bateria levam de 17 a 48 s e nenhum sai da faixa. Até o `secureboot-vm`,
que o `CLAUDE.md` registrava como "~2 h calado", mede **98 s de roteiro**: as ~2 h eram construir a
imagem de disco na primeira vez, e não o check. Ele fica de fora pelo critério 1, não pelo relógio.

**Uma divisão derivada precisa que tudo tenha nome.** Quatro checks — `tokens-wit-name`,
`network-tuned`, `syncthing-tuned`, `wifi-tpm-vm` — só existiam DENTRO de um `mN`, e uma bateria
derivada de `attrNames checks` os perderia em silêncio. Ganharam atributo próprio, o que também é
defeito consertado por si: check que não se roda sozinho é check que não se depura. Com isso, tirar
os oito `mN` do `most` não tira **nada** dele.

**Duas guardas, porque lista à mão envelhece.** `test-tiers-are-total` exige que todo nome de
`slow.nix` exista em `checks`, e `quick-names-exist` faz o mesmo pelo laço curto: um check renomeado
falha DIZENDO QUAL, em vez de virar linha morta que não exclui nada. É a forma de
`rescue-polkit-is-narrow`.

**E o dimensionamento é um só.** `nix/gate.nix` passou a receber quais alvos e que texto final, então
`most` e `gate` são dois usos da mesma aritmética — a que lê `/proc/meminfo` e limita jobs × cores.
Sem isso o `most` mataria o `rustc` pelo `earlyoom` exatamente como o gate matava antes dela existir.
Ao terminar, cada bateria **diz o que não verificou, com os nomes**.

#### E o gate ainda não fechava — por 1,37 GB que o `fetchurl` não sabe retomar

Com as três baterias de pé, o gate morreu de novo, e não por nada deste repositório. A mensagem era
`cannot download android-studio-...tar.gz from any mirror`, que se lê como "o arquivo saiu do ar" —
e foi assim que eu a li da primeira vez, a ponto de escrever no `PENDING.md` um item falso e matar o
gate que estava rodando. **Ela não quer dizer isso.**

O que o log dizia, quatro vezes:

```
 13  1.37G  13 186.5M   ...  15.43M
curl: (56) Recv failure: Connection reset by peer
```

Começa a 13–15 MB/s e o caminho derruba a conexão por volta dos 100–190 MB. Reproduzido fora do Nix,
com `curl` puro e sem sandbox: 104 MB e o mesmo reset. **O que transforma uma transferência instável
numa parede é que o `fetchurl` repete SEM retomar** — quatro tentativas, cada uma do zero, cada uma
morrendo antes dos 200 MB de 1370. O arquivo nunca termina, e a mensagem culpa o Google.

E nenhum cache salva: **o Android Studio é unfree, então o Hydra não o constrói** e o
`cache.nixos.org` não tem cópia (medido: `path-info` responde `is not valid`); o espelho por hash
responde 404 para o mesmo hash. Toda máquina que instala o perfil `full` baixa esse arquivo do
Google, uma vez, pelo caminho de rede que tiver.

`nix/prefetch.nix` é `curl --continue-at -` num laço, o hash que a derivação **já declara**, e
`nix-store --add-fixed`. Depois disso a derivação de saída fixa já é válida e o Nix nunca busca.
Três coisas o mantêm honesto:

- **Nada é escrito à mão.** Nome, URL e hash saem de `pkgs.<attr>.unwrapped.src`, então não podem
  divergir do que o build quer. O que uma pessoa mantém é um nome de atributo — e uma asserção
  recusa a entrada se a fonte ganhar um segundo espelho, porque aí o `fetchurl` se vira sozinho e a
  gambiarra deve sair.
- **O caminho entra sem contexto.** `src.outPath` carrega dependência da própria derivação que isto
  existe para buscar: com o contexto, construir a ferramenta BAIXA o arquivo primeiro, e a correção
  não pode ser construída enquanto o defeito existir. `unsafeDiscardStringContext` deixa a string,
  que é tudo o que o script quer.
- **A consulta de validade é testada antes de ser confiada.** O ramo "já está no store" é um comando
  só, e se ele deixar de significar o que significa a falha é MUDA: tudo é baixado de novo e o único
  sintoma é um gate mais lento. Aconteceu na primeira escrita — `nix-store --query --valid` não é
  flag, e o ramo nunca rodou. Hoje o script pergunta primeiro pelo **próprio caminho**, que é válido
  por construção, e recusa se a resposta for não.

O `gate-fetches-what-no-cache-has` fecha a outra metade: a ferramenta funciona, o que pode deixar de
ser verdade em silêncio é o gate CHAMAR ela — e chamá-la **antes** das três pesadas, senão paga-se a
compilação inteira para então descobrir que o problema é a rede.

---

### Atlas de glifos

A premissa "dezenas de milhares de glifos não cabem no atlas" estava errada por uma ordem de
grandeza: **são 10.473**. A 32 px de célula, um atlas R8 de 4096² comporta 16.384 — o repertório
inteiro caberia. Não estamos contra um limite duro; estamos escolhendo custo.

Decisão: **dois `TextAtlas`**, com políticas diferentes.

1. **Fixo** — repertório previsível (ASCII, Latin-1, box-drawing, ícones em uso como token, glifos
   de app id). Populado no startup, **nunca sofre `trim()`**: a garantia de desempenho fica intacta
   porque o navegador não encosta nele.
2. **Transitório** — só o navegador de glifos. `trim()` por frame, LRU.

Com a lista virtualizada o navegador rasteriza só as linhas visíveis (~50 glifos), e cada uma custa
dezenas de microssegundos no `swash`.

---

### Camada de glifos gerada

Um só gerador produz a tabela de tokens de ícone **e** a base do navegador, do mesmo arquivo de
fonte. Uma fonte da verdade.

**Três guardas contra a deriva da PUA:** a versão do Nerd Fonts no `nameID 5` tem de casar com a
esperada; o `METADATA.version` do `glyphnames.json` também; e todo token de ícone tem de resolver num
codepoint que **existe no cmap**. O fixture negativo `icons-unknown-token-fails` prova a
terceira — sem ele, "o build falha se o codepoint não existir" seria afirmação não testada.

Medido: 10.764 nomes no `glyphnames.json`, 10.473 codepoints na fonte, **0 que não resolvem**.

#### As tags são bilíngues, e isso é uma garantia — não um esforço

Um terço dos glifos (3.307 de 10.347 nomeados) não tinha **uma palavra em português sequer**. Não era
falta de mecanismo: token sem entrada no léxico passava sem traduzir, virando uma tag inglesa fingindo
de tag pt. Quem procurava em português simplesmente não achava — e nada acusava, porque `tags_pt`
nunca ficava vazia.

Três coisas mudaram, e cada uma resolve um buraco distinto:

**Sinônimo em inglês passou a caber no léxico.** Ele era `token = ["pt", …]` — só traduzia. Não havia
como dizer que `location` também se acha por `locale` e `map`; isso só existia no `overlay.toml`, que
é **por glifo** e tem 31 entradas para 10.473 glifos. Agora a entrada aceita
`{ en = […], pt = […] }`, e a forma de lista continua valendo — reescrever as 390 entradas antigas
seriam 390 chances de errar sem ganho nenhum.

**O acento se resolve no gerador.** As tags pt são escritas **acentuadas**, que é como se escreve e é
o que vai para a tela; o gerador emite, num campo `search`, a forma sem acento de cada uma que difere.
Assim `localização` e `localizacao` acham o mesmo glifo sem o componente normalizar nada — normalizar
em runtime custaria percorrer 10.473 glifos × ~8 tags a cada tecla digitada.

**Nome próprio ganha CATEGORIA, nunca tradução.** `pytorch` continua `pytorch` — já é tag por si
mesmo, e é igual nos dois idiomas. O que entra é o que ele **é**. A categoria vem do conjunto de
origem, porque é o que o conjunto significa: as 100 logos de `linux-` são "software livre", as 324 de
`dev-` são "desenvolvimento". Sem isso, esses glifos só eram alcançáveis por quem já sabia o nome
exato. Traduzir marca (`elementary` → "elementar") produziria busca errada com cara de certa.

**E duas guardas no gerador, não num check posterior.** Nenhum glifo nomeado pode ficar sem tag pt, e
a cobertura ponderada não pode cair abaixo de 90%. Elas moram ali porque quem sabe qual glifo ficou
sem tradução é quem montou a tabela — a saída **não** diz, justamente porque o token sem tradução
entra em `tags_pt` sem traduzir. Barulhento de propósito: subir a versão do Nerd Fonts passa a falhar
até os glifos novos serem traduzidos.

Resultado medido: léxico de 390 → **2.565** entradas, **0 glifos sem tag pt**, cobertura de 63,3% →
**92,0%**. O custo temido não veio: a base foi de 2,66 para 2,70 MB e o `.wasm` de 2,51 para 2,74 MB,
com instanciação em **0,011 ms** de p99 contra um teto de 5 ms.

#### A ficha do glifo, e uma área de transferência que não existia

`Enter` num glifo trocava a tela por nada: ele "copiava". Agora ele abre a **ficha** — a mesma
superfície, o conteúdo inteiro substituído (invariante 14: não há janela nova nem popup). O estado da
listagem não é tocado, então `Esc` devolve a tela como ela estava; voltar é uma volta, e não um
recomeço.

Dentro dela: o glifo em `Scale::Huge` (token `xxlarge`, 116 px = exatamente quatro alturas de
célula, para o que vem embaixo não cair por cima dele), nome e apelidos, identidade técnica, uso no
sistema, tags nos dois idiomas, e as nove opções de cópia — **com o valor de verdade ao lado do
rótulo**, porque ver `nf nf-cod-folder` antes de copiar é metade da utilidade. `↑↓` andam nas opções,
`←→` trocam de glifo sem sair da ficha, `Enter` copia a selecionada.

**Copiar não existia.** `Effect::Copy` chegava ao host e virava `tracing::info!("copiar pedido pelo
buffer")` — os nove formatos nunca puseram nada em lugar nenhum. Não estava no `PENDING.md`, e este
próprio documento falava de "copiar" como se funcionasse: é o fallback silencioso na forma mais cara,
em que a funcionalidade **parece** pronta e a documentação confirma.

Agora é `wl-copy`, e a razão não é preguiça: quem é **dono** da seleção precisa continuar vivo para
servi-la quando alguém colar, e a janela do buffer morre quando se fecha o navegador — copiar um
glifo **para** fechar e colar é o uso. Dono que morre junto entregaria uma área de transferência
vazia, que é pior do que não copiar. É a mesma escolha do `niri msg action`: o cliente que já existe,
em vez de reimplementar o outro lado do protocolo.

**O feedback é do HOST, e volta ao buffer.** Copiar sem dizer nada é
indistinguível de não copiar — foi assim que a cópia passou meses quebrada. Mas quem sabe se deu
certo é o host, não o componente: um "copiado!" que o buffer mostrasse por conta própria seria
otimista, e mentiria exatamente quando importa. Por isso existe `input::copied(bool)` — o veredito
volta, a ficha o mostra na linha da ajuda, e a tecla seguinte o apaga (aviso que fica deixa de ser
resposta e vira decoração).

`Ctrl+V` fecha o par: o host lê a área de transferência e entrega `input::paste(string)`. O buffer
não lê clipboard como não lê arquivo (invariante 11). Antes, `Ctrl+V` caía na guarda de `Ctrl` do
host e não fazia nada — sem log, sem tela, indistinguível de "não havia nada copiado".

**Colar o próprio glifo acha o glifo.** É o gesto mais natural do navegador — pegar um ícone que se
vê num CSS, num commit, numa mensagem, e perguntar *qual é este* — e era o que a tela não respondia:
a busca olhava nome, conjunto e tag, e o caractere não é nenhum dos três. Agora ele é o **primeiro**
critério: identidade vence tudo, e não há o que ordenar depois dela.

Isso fechou o círculo com a cópia: copiar o glifo na ficha e colar de volta na busca devolve
exatamente ele. E a mesma passagem tirou uma duplicação — o filtro por prefixo (`fa:folder`) e a
pontuação da lista faziam a mesma pergunta com código próprio, e nada impedia um de ganhar um critério
que o outro não tivesse. Hoje é uma função só (`Glyph::match_score`), e a nota que ela devolve **é** a
ordem da lista.

**Rótulo traduzido não cabe por acaso.** A ficha alinha os valores numa coluna fixa, e o rótulo é uma
**chave**: o buffer nunca vê o texto traduzido nem o comprimento dele. "Token do design system" (22
células) saía como `Design system …` em qualquer tamanho de janela, porque o corte vinha do orçamento
declarado e não do espaço na tela. A coluna passou a caber o mais longo — e quem garante que continue
cabendo é o teste, que tem os dois lados: o orçamento que o buffer declarou na display list, e o
`.ftl` de verdade, nos dois idiomas.

**Três armadilhas que só o teste de VM encontrou**, e as três são de spawn:

1. `wl-copy` faz `exec cat` para servir a seleção. Sob uma unidade do systemd o `PATH` é mínimo e o
   `cat` não está lá — `exec cat: No such file or directory`. É a armadilha do `CLAUDE.md` um nível
   mais fundo: quem spawna **por nome** é o programa que nós spawnamos por caminho. Daí o
   `RONOCTUA_WL_COPY_PATH`.
2. `wait_with_output()` lê os canos até EOF, e o filho que o `wl-copy` deixa servindo herda esses
   canos — o EOF só viria quando ele morresse. O laço de eventos do buffer travava ali, e na tela
   isso apareceria como "o buffer parou de responder depois de copiar". `wait()` e `stderr`
   herdado: o erro do `wl-copy` cai no nosso journal, que é onde ele serve.
3. O `wl-paste` do `Ctrl+V` precisa do **mesmo** `PATH`, e sem ele voltava vazio — que o código
   tratava como "não havia nada copiado", porque área vazia não é erro. Duas causas com o mesmo
   sintoma: a correção foi dar o `PATH` **e** pôr o motivo no log, para as duas deixarem de ser a
   mesma linha.

O critério do check é a **área de transferência do sistema** (`wl-paste` dentro da VM), e não "o
`wl-copy` foi chamado" — essa segunda forma repetiria o defeito de hoje um nível acima.

#### As tags do glifo em foco, no que sobra do rodapé

Elas são as palavras que **acham** aquele glifo, e até agora eram invisíveis: dava para procurar por
"mapa", cair no `cod-location` e nunca descobrir por quê.

Ordem **do leitor primeiro** — é o que faz a elisão cortar o idioma dos OUTROS numa tela estreita.
A palavra que já apareceu não repete: ver a mesma duas vezes na mesma linha parece defeito.

Era ordem FIXA, pt primeiro, e isso foi um defeito desde o dia em que houve dois leitores: quem lia
a interface em inglês tinha o próprio idioma cortado primeiro, justamente na linha que existe para
dizer como o glifo se chama. O arranjo "que funciona sem saber" resolve ORDENAR duas coisas
mostradas; não resolve escolher entre elas, e foi por isso que aqui entrou uma porta.

A porta é a chave reservada **`-language`**, que o hospedeiro semeia junto das frases. Ela não afrouxa
o invariante 11: o valor é uma ETIQUETA, nunca uma frase, e a única coisa que o buffer pode fazer com
ela é INDEXAR dado que já tinha — escolher qual das próprias listas de tags mostra primeiro. Um buffer
que a usasse para escolher entre duas strings escritas por ele estaria escrevendo UI num idioma, que
é exatamente o que o invariante proíbe. O `-` na frente não é enfeite: em Fluent um nome com `-` é um
TERMO, e o `KEY` do gerador exclui termos de propósito — então `-language` não pode colidir com chave
de texto nenhuma **por construção**.

Discretas por construção, e não por ajuste: `TextDim` é a cor de menor prioridade da paleta, e
`Scale::Small` é o token `font.size.small` (15 px) contra os 18 px do corpo do buffer. **O nome
nunca cede espaço** — quem procura pelo nome está lendo o nome; abaixo de doze células livres as tags
não são emitidas, porque ali não se mostra uma palavra, mostra-se um toco elidido que ocupa a linha
para não dizer nada.

#### Três papéis, três corpos — e uma célula de distância entre tudo

O navegador tinha um corpo só para quase tudo (`Scale::Normal`, 18 px — 20 px na época): o convite da busca, o
contador, a lupa. O resultado é uma tela sem hierarquia, onde o que se lê de relance tem o mesmo peso
do que se lê com atenção, e onde um ícone — que é um **desenho** — sai com o corpo de uma letra e
fica fino ao lado dela.

A regra passou a ser por **papel**, e não por trecho:

| papel | escala | px | onde |
|---|---|---|---|
| glifo mostrado como ícone | `Large` | 40 | a lupa, o filtro, a grade e o glifo em foco no rodapé |
| conteúdo do buffer | `Normal` | 20 | o nome do glifo, as opções do painel, os valores da ficha |
| interface | `Base` | 18 | o campo de busca |
| acessório | `Small` | 15 | o contador, as tags, os rótulos da ficha |

`Scale::Base` é nova, e o que ela expõe já existia: `font.size.base`, o corpo do rodapé e do
minibuffer. **Um campo de busca é um minibuffer** — mesma função, mesma pergunta —, e ver os dois em
corpos diferentes na mesma tela é o tipo de detalhe que faz o sistema parecer montado de peças de
origens diferentes. O convite e o que se digita compartilham a escala de propósito: se só o convite
mudasse, o campo pularia de tamanho na primeira tecla.

##### O espaço entre blocos JÁ VEM na caixa da linha

Esta é a medida que muda tudo, e que eu não tinha feito: a caixa da fonte é **1,25 em** (ascendente
0,965 + descendente 0,285, em de 1000) e a entrelinha é **1,45** — então todo trecho já carrega
`0,10 em` de ar em cima e `0,10 em` embaixo, e a tinta de um ícone Nerd ocupa só `0,60 em` da caixa.
Somar uma célula vazia entre dois blocos é folga sobre folga: rendeu linha em branco em cima e
embaixo do cabeçalho, linha em branco entre as fileiras de ícones, e **um terço dos glifos fora da
tela**.

A regra é a inversa: **os blocos são adjacentes**, e o único branco que sobra é o que não completa
uma fileira. Pela mesma razão a margem do buffer é assimétrica — `marginX = 2`, `marginY = 0`. Na
horizontal a tinta ocupa o avanço inteiro (os glifos Nerd vão de 0 a 600 unidades num avanço de 600,
sem folga lateral) e sem margem o primeiro caractere encosta na janela; na vertical a folga já está
na caixa.

O mesmo vale para o vão da grade, que o dono pediu igual em todas as direções. Ele **não** é ajustável
nas duas: na vertical é o que sobra da caixa (`1,45 − 0,60 = 0,85 em`, a 40 px dá **34 px**) e não há
como encurtar sem sobrepor; na horizontal ele anda de 10,8 em 10,8 pixels (`0,600 × 18`). Três
colunas dão **36 px** — dois de diferença, o mais perto que a grade de células permite. Duas colunas
dariam 24 px, dez a menos, mas caberiam mais ícones por linha: é uma troca entre densidade e
regularidade, e o passo é uma constante só (`HORIZONTAL_GAP`).

**O rodapé é ancorado no fundo da janela.** Grudado na grade, a sobra da divisão ia toda para *baixo*
dele: uma faixa vazia no fim da tela. Ancorado, ela vira o vão acima do rodapé, e o check exige que
seja **menor que uma fileira** — se coubesse mais uma fileira de ícones ali, ela tinha de ter sido
usada. E o glifo em foco saiu para trecho próprio: junto do nome, ele herdava o corpo do texto.

##### Uma linha é o que compartilha um MEIO

Misturar corpos numa linha só funciona se eles se alinharem por algum lugar, e o lugar é o **meio** —
é o que o olho lê como "estão na mesma linha". Enquanto tudo tinha um tamanho isso não aparecia:
topo, meio e base andavam juntos, e o renderer punha cada trecho em `linha × entrelinha`. Bastou o
ícone de 40 px ao lado do texto de 18 px para o texto subir para o topo do ícone.

O renderer centraliza a **caixa**, e isso acerta a tinta por construção: a entrelinha é simétrica, o
meio da caixa da linha é o meio da caixa da fonte, e o meio da caixa da fonte fica `0,34 em` acima da
base — exatamente o meio da tinta de um ícone Nerd, que vai de `0,04` a `0,64 em`. Centralizar a
caixa, medir a tinta e centralizar a tinta dão o mesmo número, e a caixa é a única das três que o
renderer sabe sem abrir a fonte.

Numa linha de um corpo só a conta devolve `linha × entrelinha`, que é o que havia antes — é o que faz
o rodapé do sistema e a modeline não mudarem um pixel, já que eles nunca misturam tamanhos. Isso tem
uma consequência na ficha: o nome **não** pode dividir a linha do glifo `huge`, senão desceria para
o meio dele e os apelidos, que vêm depois, apareceriam acima. Ele tem linha própria.

##### Negrito de ícone não existe, e o build prova

O pedido de ícones "em bold" não tem como ser atendido, e vale registrar por que para não ser tentado
de novo. As 27 variantes são patcheadas com os **mesmos** contornos de ícone: em `cod-search`,
`cod-filter`, `md-wifi`, `fa-folder` e `cod-flame`, a caixa e a contagem de contornos são idênticas
byte a byte entre a Regular e a Bold — só as letras latinas engordam (`A` vai de `(88…512)` para
`(81…519)`). Embutir a face Bold para pedir peso nos ícones custaria uma segunda cópia da fonte no
binário e não mudaria um pixel: seria o fallback silencioso com outra roupa. Ícone fica mais pesado
**crescendo**, ou trocando por uma variante preenchida que o conjunto de origem tenha desenhado
(`cod-filter_filled`) — as duas são decisões de token, não de renderer.

---

#### O painel de conjuntos: um ícone, e o filtro que se acha sem saber o nome

Os cinco filtros sempre existiram como **prefixo na busca** (`md:`, `bloco:f0`, `tag:seta`).
Isso é rápido e é invisível: para digitar `md:` é preciso já saber que o conjunto se chama `md`, e
nada na tela contava. Quem não sabia tinha 10.473 glifos e nenhuma forma de recortá-los por origem.

O painel é a descoberta que faltava, e **não** um segundo mecanismo de filtro: escolher "Material
Design" escreve `md:` na busca, que continua editável. Ele desenha no **conteúdo do próprio buffer**,
entre a busca e a régua — nenhuma superfície nova, nenhum popup (invariante 14) —, e o que ele ocupa
sai da grade. O termo livre sobrevive à troca: quem digitou `folder` e escolheu Material Design fica
com `md:folder`.

**A lista sai da BASE, não de uma lista escrita no código.** A próxima versão das Nerd Fonts pode
trazer um conjunto novo, e uma lista à mão ficaria para trás em silêncio — o painel simplesmente não
o ofereceria. A ordem é por tamanho, e não alfabética, porque alfabética seria a ordem do nome
**traduzido**, que o buffer não vê (invariante 11): ordenar pelo prefixo daria uma ordem que não é a
que está na tela.

**Os nomes dos conjuntos existem em dois lugares, e um check os amarra.** No `.ftl` porque são texto
de interface e alguns se traduzem ("Clima", "Personalizados", "Logos de distribuições"); no `SETS` do
`gen-glyphdb.py` porque de lá viram **tag de busca**. Nada obriga os dois a concordarem, e "Clima" no
painel com "weather" na busca seria uma tela que ensina uma palavra que não funciona. Por isso o
`browser-filters` exige que o rótulo que se lê ache, digitado na busca, o conjunto inteiro — e por
isso o `SETS` passou a ser acentuado, já que a forma sem acento sai de graça do `no_accent`.

A chave é **montada em tempo de execução** (`glifos-conjunto-` mais o prefixo), então o compilador
não protege nada aqui: um conjunto novo sem tradução sairia como `[glifos-conjunto-xyz]`, que é o
fallback silencioso de sempre. O mesmo check confere existência nos dois idiomas e o orçamento de 24
células, como já fazia com os rótulos da ficha.

O ícone fica à direita do contador e **acende** quando há filtro em uso. Sem isso, uma lista restrita
e a base inteira teriam a mesma cara, e "sumiu metade dos glifos" viraria um mistério.

---

### Superpacote de ícones

Prioridade `Zafiro` → `Adwaita` → `MoreWaita` → `Papirus`, mais `kdePackages.breeze-icons` (única
cobertura decente de app KDE/Qt) e `hicolor-icon-theme` (herança final obrigatória pelo spec —
requisito, não escolha). Atenção: `breeze-icons` **não** é top-level.

**Nada mais por palpite.** O propósito é fallback de pixmap por `IconName` para apps estrangeiros, e
o que importa é cobertura de nomes de aplicativo. O `coverage.md` gerado decide se sobra buraco.

**Licenças:** as origens misturam GPL-3.0, CC-BY-SA-4.0 e MIT. A derivação declara `meta.license`
como a **lista** (não colapsa para uma), grava cada texto, e o manifesto carrega a licença **por nome
vencedor** — que é o único jeito de responder "de onde veio este ícone e sob que termos".

---

## 2. O compositor, e a fronteira com o niri

### A fronteira com o niri

`ronoctua-niri` é o **único** crate que fala com o compositor.

**Tipos próprios, não a crate `niri-ipc`.** O contrato tem de passar contra a tag pinada **e** contra
`niri-next`; depender da crate publicada tornaria isso vazio, porque o Cargo fixaria uma versão dos
tipos e o teste compararia tipos velhos com binário novo. Com tipos nossos — estreitos, só o que a
modeline usa — o mesmo código é exercitado contra os dois binários de verdade.

**Evento desconhecido não é erro.** Eventos são lidos como `Value` e só então mapeados; o que não
reconhecemos vira `Event::Ignored`. O niri versiona por calendário e acrescenta variantes com
frequência — um enum estrito quebraria a distro num upgrade por causa de um evento que não usamos.
O que o contrato prova é o contrário: que os eventos que **usamos** continuam existindo.

---

### Xwayland

**Sim**, via `xwayland-satellite` (o caminho upstream do Niri). Recusar custaria Steam, Java legado
e uma cauda de apps que não vão migrar — a regra proíbe *chrome* de desktop tradicional, não
aplicativos.

**A restrição que isso impõe não é a óbvia.** Buffer estrangeiro é correlacionado por **PID →
cgroup**, e o satellite cria as superfícies **em nome dele mesmo**: o `pid` reportado seria o dele,
igual para todas as janelas X11, e a correlação quebraria **em silêncio**.

Decisão: **um satellite por scope systemd de buffer**, com `$DISPLAY` próprio. Custo: um processo de
poucos MB por buffer X11 — e já se paga um scope por buffer de qualquer forma.

---

### Multi-monitor

**Duas linhas em todo output; minibuffer ativo só no de foco.** A modeline é por janela e diz o que
está em foco ali; o minibuffer é por tela e é onde se digita. Dois minibuffers simultâneos criariam a
pergunta "qual deles tem o prompt?", que não tem resposta boa.

Todo output mantém exatamente duas linhas, então o exclusive zone fica uniforme e **o conteúdo não
reflowa quando o foco troca de tela**.

**Consequência a respeitar:** o estado do minibuffer vive **no daemon, não na superfície**. Se o
output em foco desaparecer no meio de um prompt, o estado migra sem perder o que foi digitado.

---

### GPU nos testes

Dividido em dois, porque uma resposta só seria desonesta.

- **Correção, layout e screenshot → `lavapipe`** dentro do `runNixOSTest`. Hermético, roda no
  sandbox e em CI sem GPU; screenshots e OCR estáveis entre execuções.
- **Desempenho → nunca sob lavapipe.** Cravar 16 ms sobre rasterização por software mede o software.
  O CI gateia **métricas independentes de GPU** (`layout-fast`, `shaping-fast`,
  `instance-fast`); os números de frame saem de `nix run .#bench`, em hardware real.

`virtio-gpu` acelerado fica fora: exige passthrough e quebra a pureza do sandbox — caro exatamente
onde o valor do teste está.

**O niri não roda no TTY de uma VM sem GPU.** O backend DRM rejeita renderizadores EGL por software,
e numa VM o único EGL é o llvmpipe. Ele então sobe, cria o socket e fica com **zero output** — não
falha, não avisa — e uma superfície layer-shell precisa de output. Por isso os testes usam o backend
**winit** sobre `Xvfb`.

---

### Nada vaza da janela, e isso é garantia do HOST

Uma foto da tela mostrou o convite do navegador de glifos passando **por cima**
do contador. Texto sobre texto é o pior resultado possível: nenhum dos dois fica
legível, e não há erro em lugar nenhum.

São **dois** limites, e cada um sozinho deixa passar um caso:

- **O orçamento** (`comando-texto.largura-max`) é o que o buffer reservou para
  aquele trecho. Sem ele, dois textos da mesma linha colidem.
- **A borda** é a garantia do host: nada sai da janela, tenha o buffer pedido o
  que tiver. Sem ela, um buffer distraído escreve fora da tela — e o compositor
  não recorta.

**O buffer declara o orçamento; quem corta é o host.** Não é divisão arbitrária:
um buffer não pode elidir uma `key`, porque quem a traduz é o host e o
comprimento da frase muda com o idioma — "Type to search for a Nerd Fonts glyph"
e "Digite para buscar um glifo das Nerd Fonts" não têm o mesmo tamanho. O que o
buffer sabe é quanto espaço há.

E quando nem o orçamento resolve, o buffer **esconde o que importa menos**: numa
janela estreita o contador some, porque sem ele ainda se busca e sem o campo de
busca não.

---

### O buffer diz "grande", o host diz quantos pixels

O navegador de glifos precisava de ícones maiores, e o tamanho é decidido pelo host — o invariante 11
não deixa valor de token atravessar a fronteira. A saída é a mesma que a cor já usava: o buffer
**nomeia**, o host resolve. `large` vale `font.size.xlarge`.

**Por TRECHO, e não por buffer.** Começou no descritor, e o resultado foi o cabeçalho e o rodapé do
navegador crescendo junto com os ícones: texto de interface em corpo 32, desconfortável de ler e
estourando a linha. O que precisa ser grande ali é o **glifo**; o texto em volta é texto. A grade de
células continua sendo a do tamanho normal — um trecho grande ocupa mais de uma célula, e quem o
espaça é quem desenha, porque o host não sabe o que é grade e o que é texto corrido.

No renderizador isso custou uma linha: cada trecho já tinha o próprio `Buffer` do glyphon, então a
métrica passou a sair dele. A alternativa — um segundo renderizador — daria um segundo atlas e uma
segunda cópia da fonte para desenhar dois tamanhos.

Pelo mesmo caminho entrou `text::icon`: um buffer que quisesse uma lupa teria de escrever o
codepoint da PUA à mão — um número que muda entre versões das Nerd Fonts, embutido num componente que
não tem como saber disso. Nomeando o token, quem resolve é quem tem a tabela (invariantes 9, 10 e
11). Um nome que não resolva vira `?` **visível**: chegar até ali significa que a tabela e o WIT
divergiram, e isso tem de aparecer.

---

### A escala não é hierarquia, e uma run ocupa mais de uma célula

O parágrafo acima diz que o texto em volta do glifo é texto, e que corpo 32 é desconfortável de ler.
Isso foi escrito depois de acontecer no navegador — e voltou a acontecer, em **três buffers de uma
vez**, na forma de título: `formatar`, `transferências` e `memória` pediam `Scale::Large` para a
frase do topo. Quarenta pixels sobre uma janela cujas outras linhas todas têm vinte. O dono relatou
da tela, com uma foto.

Junto vinha o segundo defeito, e ele é o interessante: **a régua encostava no título**. Uma run de
40 px tem caixa de linha de 58, e a célula da grade é 29 — ela ocupa DUAS. A régua estava em
`line + 1`, que é exatamente a segunda, e caía na linha de base do próprio título: na tela, um
sublinhado atravessando as descidas. Nenhum teste falhou, e nenhum teria: a display list dizia linha
0 e linha 1, que são linhas diferentes, e **nada em lugar nenhum multiplicava uma linha pela altura
que a escala dela vale**. O `heights` do contexto existe para isso desde o glifo cortado da ficha —
os três títulos simplesmente não o consultavam.

**O que marca um título não é o tamanho.** O `colors.nix` já dizia, na abertura: ênfase é por peso e
cor, separação é por espaço em branco e uma régua de uma linha. O título passou a ser do tamanho do
corpo, com o glifo de acento na frente, a régua embaixo e uma célula vazia depois dela — no painel
de memória não há régua, e lá o que separa é só a célula vazia. Uma célula em vez de duas, o que uma
janela que precisa caber uma lista de discos agradece.

**Duas perguntas, no crate da display list.** `DisplayList::nothing_overlaps` (de lado) e
`nothing_is_covered` (para baixo) moram junto da lista que leem, e não dentro de um driver, para que
nenhum deles discorde sobre o que um comando ocupa. Elas não rodam no caminho de desenho: recusar
um quadro por um encostamento apagaria o buffer inteiro por um defeito cosmético. Quem as cobra é o
`states-driven`, que é o único que passa **todo buffer por todo estado** — e o `format`, `memory` e
`transfers` driven, que são os que enchem a tela de dados, porque lista vazia não encosta em nada.

Três detalhes que a primeira versão da pergunta errou:

- **Uma chave cobra o orçamento inteiro; um dado cobra o comprimento.** `Text::Key` é frase que o
  host vai traduzir — o buffer não sabe quanto ela mede, então o que ele reservou é tudo com que se
  pode contar, e orçamento que invade o campo seguinte é campo que SERÁ escrito por cima assim que o
  idioma mudar. `Text::Data` o buffer tem na mão, e quase sempre posiciona o campo seguinte a partir
  desse comprimento: cobrar dele um orçamento que comprovadamente não usa condenaria toda lista cuja
  última coluna vai até a borda.
- **A escala alarga tanto quanto sobe.** A célula é `0,600 em`, então um glifo `Large` cobre as duas
  colunas que o `heights` já lhe cobra na outra direção.
- **Só as linhas DEPOIS da primeira.** Dois comandos na mesma linha é o caso comum — uma linha de
  colunas, uma faixa de seleção sob o próprio texto, um ícone ao lado de um nome. Quem decide se
  aqueles se chocam é a largura, que é a outra pergunta.

E há uma terceira cobrança, que é sobre o tamanho e não sobre a geometria: **uma chave traduzida
nunca é desenhada em escala de glifo**. `Large` e `Huge` são legítimos e frequentes — é o que os três
navegadores usam para o codepoint, onde o conteúdo É o desenho. Frase, não. O comentário do
`font.size.xlarge` já dizia isso por escrito e não impediu nada, porque nada lia o comentário.

E a primeira coisa que ela encontrou de verdade não foi o título: foi a ficha do Lucide. São três
marcas antes do valor — ajustar, digitar, restaurar — e o espaço reservado para elas era de DUAS
células. A terceira, `History`, caía na primeira célula do valor: no `24 px` da linha de tamanho, o
glifo ficava sobre o `2`, em toda linha de ajuste. A reserva e a quantidade de coisas que reservam
têm de ser o mesmo número.

**E depois disso veio `nix run .#png`, que é a parte que muda o dia a dia.** Nada disso teria durado
meses se olhar custasse pouco. Custava uma VM NixOS: subir, niri sob Xvfb, capturar a raiz do X,
recortar pela geometria que o niri informou, passar OCR. Três minutos para ver se uma régua encostava
numa descida — e regra que custa três minutos é regra que se pula, que é exatamente o que aconteceu.

O comando desenha um buffer num PNG em **0,2 s**, sem compositor e sem VM. O que ele NÃO faz é ter
um segundo rasterizador: o componente é carregado como o host carrega, dirigido pelos mesmos
`Input`, e a display list dele atravessa o `paint` — o mesmo módulo que o `buffer.rs` usa para pôr um
buffer num `xdg_toplevel`. Foi por isso que o `paint` virou módulo: a conversão de lista em trechos,
a cor por token, a elisão pelo orçamento, a margem, a célula e a faixa de seleção estavam ao lado do
Wayland que as desenha, e o terminal já as pegava por cima do muro. Um PNG que discordasse da tela
significaria que essa ponte se duplicou — não que a tela é um mistério. Do `Gpu` só sai a superfície:
o `offscreen` entrega dispositivo e textura com as MESMAS limites, e o `prepare`/`draw` é o do
rodapé, linha por linha.

Duas coisas ele não desenha, e **diz** em vez de fingir: `Command::Svg`, que precisa do cache de
pixmap que a janela guarda, e qualquer tela que dependa do host responder ao buffer — uma leitura de
discos, uma fila, uma amostra de memória. Esses buffers saem com a tela vazia, que é o retrato
honesto do que eles têm.

**E o comando tem check** (`png-tool-draws`), porque ferramenta que ninguém confere é ferramenta que
para de funcionar calada, e a descoberta acontece no dia em que se precisa dela. Ele não se contenta
com "o arquivo apareceu": um PNG do tamanho certo, todo da cor do fundo, é EXATAMENTE o que um
desenho quebrado produz. Então o CRC de cada pedaço é recalculado e o `IDAT` passa por um `zlib` de
verdade — é o que segura as quarenta linhas escritas à mão, e um `65521` trocado por `65520` no
Adler-32 o derruba; o tamanho no cabeçalho tem de ser o que a ferramenta IMPRIMIU; tem de haver
tinta, isto é, pixels que não são a cor mais comum; e **pt-BR e en-GB têm de sair diferentes**, o que
fecha a corrente inteira numa comparação só — componente, display list, i18n, tokens, wgpu, pixel —
sem OCR e sem nomear uma cor ali, que seria uma segunda cópia de um token.

O ambiente gráfico é o lavapipe, o mesmo dos checks, e não é escolha de desempenho: é o que faz o
comando responder igual nesta máquina, numa sem GPU e dentro de um check. Seção 9.3 continua valendo
— aqui **nunca** se mede tempo de quadro. E o PNG sai sem compressão, escrito à mão em quarenta
linhas: fazer do `png` uma dependência direta custaria uma recompilação de toda a camada de
dependências do crane, minutos em toda máquina, para poupar essas quarenta.

---

### O design system existe, e é um crate — `ronoctua-design`

A seção acima termina dizendo que olhar passou a custar 0,2 s. O dono olhou, e voltou com quatro
coisas na mesma frase: *o título não tem ar proporcional em cima e em baixo; a régua encosta na
direita e sobra na esquerda; cada buffer tem uma margem, quase sempre excessiva; e a mesma vista sai
colada na borda no rodapé e com outro espaçamento em janela própria.* E a conclusão dele, que é a
única parte que importa: **"não existe padrão nenhum; não há um design system"** — e conserte de um
jeito que esses erros **não tenham como ocorrer de novo**.

Ele estava certo, e a causa é sempre a mesma: **não havia um lugar sobre o qual estar certo.**

**Quatro cabeçalhos, quatro respostas.** `example` punha título na linha 0, régua na 1, conteúdo na 2
na coluna 0. `format` punha o ícone na coluna 2, o texto na 5, régua na 1, uma linha em branco na 2 e
o conteúdo na 3. `memory` não tinha régua nenhuma e devolvia `line + 2`. `transfers` tinha régua e
não tinha branco. Ar acima do título: zero, nos quatro. **Três navegadores, três cópias** de
`grid_columns`, `rule_row`, `footer_row` e `row_step`, com números DIFERENTES — `ICON_COLUMNS` 2/4/2,
`HORIZONTAL_GAP` 3/3/4, `LABEL_WIDTH` 24/20/24 — e um literal `2` de ar à direita em cada uma.

**O invariante 12 já nomeava o crate, e o `PENDING` dizia que ele não existia.** Existe agora, e é o
que fecha aquele item: `crates/ronoctua-design` é **um crate compilado duas vezes**, não um crate do
hospedeiro com um gêmeo no convidado. Isso só é possível porque o `ronoctua-protocol` foi escrito
para atravessar — zero dependências, `no_std` mais `alloc` — e diz isso por escrito no próprio
manifesto desde sempre; o que faltava era alguém do lado do convidado consumi-lo.

**Ele é o dono das bindings do WIT, e é isso que torna "um cabeçalho" obrigatório em vez de
combinado.** Os sete buffers não chamam mais `wit_bindgen::generate!`: o crate chama uma vez, com
`pub_export_macro`, e cada componente exporta com
`export!(Component with_types_in ronoctua_design::bindings)`. Medido antes de qualquer migração: a
saída começa com `00 61 73 6d 0d 00 01 00`, a assinatura do component model, igual à de um buffer
construído pelo Nix. Junto foi embora o `#![allow(unsafe_code, missing_docs)]` que cada buffer
carregava — a isenção é de quem gera binding, e agora só um gera.

**A única coisa que difere entre os dois lados é a cor e o ícone**, e isso é um `trait Palette` com
duas implementações de vinte linhas. A aritmética é a mesma função, não duas.

#### O cabeçalho são três células, e não há régua nenhuma

O dono mudou de ideia no meio deste trabalho, e a mudança melhorou o argumento em vez de furá-lo:
**"não quero que exista linha alguma; zero linha."** O `colors.nix` sempre abriu dizendo que
separação é por espaço em branco *e* uma régua de uma linha; o painel de memória sempre desenhou o
cabeçalho **sem régua**, e esta seção já registrava que lá o que separa é a célula vazia. Dos quatro
formatos, o certo era o que não tinha régua. Agora é o de todos.

```
linha + 0   AR
linha + 1   glifo (coluna 0, largura ICON) + título (coluna ICON+GAP, até a borda)
linha + 2   AR
linha + 3   primeira linha de conteúdo   <- o que heading() devolve
```

A célula é 18 × 1,45 = **26,1 px**, e a caixa da fonte é 1,25 em = 22,5 px — então toda run já carrega
**1,8 px** de folga acima e abaixo da tinta. Ar acima do título: 26,1 + 1,8 = **27,9 px**. Abaixo:
1,8 + 26,1 = **27,9 px**. Dois vãos iguais, cada um exatamente *uma altura de linha mais a folga da
própria caixa* — que é, ao pé da letra, o "ar proporcional em cima e em baixo até a altura da linha"
que foi pedido. E três células em vez das quatro que a versão com régua custaria: uma janela que
precisa caber uma lista de discos agradece.

`Screen::rule` continua existindo, para separar BLOCOS — é o que o painel de memória usa entre as
suas quatro partes e os navegadores acima da grade. O que mudou é que `heading()` nunca emite uma.

#### E os separadores saem de um lugar só — `chrome::rule` e `chrome::divider`

Meses depois o dono voltou com a mesma frase, sobre outra tela: *"há algumas linhas horizontais que
tocam o lado direito da tela e não tocam o esquerdo"*, e as verticais *"não estão tocando a parte
baixa da tela"*. A régua dos BUFFERS já estava sangrada — o `paint.rs` foi consertado quando o crate
nasceu. O que não estava era tudo que o **hospedeiro** desenha por conta: a janela do update, o
gerenciador nas duas portas, a área de transferência.

Medido antes de mexer, e é o que prova que não era o caso e sim a causa: na janela do update a régua
começava em **x=12** e ia até a borda; no gerenciador os três divisores paravam em **y=798** de 800.
Eram **oito** retângulos escritos à mão em três arquivos, e cada um errava de um dos dois jeitos:

- `x: margin * cell` com `width: self.width` — recuo à esquerda e a largura INTEIRA, o que passa da
  borda direita e é cortado lá. Na tela: buraco de um lado, colado do outro.
- `height: linhas * line_height` — e a altura de linha é fracionária (26,5 px aqui), então o produto
  cai antes do fim. Uma contagem de linhas é um PALPITE de onde a superfície termina.

Agora há duas funções e mais nenhuma. `chrome::rule(largura, y)` recebe a largura da superfície e
**não tem parâmetro de recuo** — não dá para escrever o erro. `chrome::divider(x, de, fundo)` recebe
o **fundo** e não uma altura: "onde termina" passa a ser respondido com a borda, e quem sabe a borda
é quem desenha. O check `separators-come-from-chrome` recusa qualquer `Quad` de separador fora do
`chrome.rs`, e recusa também o `chrome.rs` perder as duas funções — um check que só proibisse
passaria no dia em que alguém as apagasse e voltasse à mão.

#### Dois trilhos, e só dois

Uma display list sai do `paint` por um de dois caminhos, e qual deles um comando toma **é** o padrão
de layout:

| trilho | por onde | quem anda nele |
|---|---|---|
| recuado | `spans_of` → `with_margin` | todo `Text`, toda `Grid` |
| **sangrado** | `quads_of` | `Rule`, `Selection` |

Texto é recuado porque glifo encostado na borda se lê como corte. Fronteira é o contrário: uma que
parasse na margem do texto teria começo e fim, e coisa com começo e fim é caixa — que o invariante 15
proíbe. Então essas duas começam em `x = 0`, medem a janela inteira, e **não passam pelo
`with_margin`**.

Foi isso que o dono fotografou. A régua saía como `"─".repeat(80)` na coluna 0 — um **glifo** —, e o
`with_margin` a empurrava duas células adentro. Três defeitos numa linha: folga na esquerda,
vazamento na direita em qualquer janela com menos de oitenta e duas colunas, e oitenta células de
largura que não tinham relação nenhuma com o tamanho da janela. Nenhuma das duas perguntas de
geometria enxergava: o `nothing_overlaps` filtra para `Text`, e o `columns_of` já modelava a régua
como largura inteira — **o modelo estava certo e quem mentia era o renderizador**.

Ela é um quad agora, com a espessura vindo de `space.rule.thickness` (o segundo bloco daquele arquivo
medido em PIXELS, e o comentário diz por quê), desenhado no **fim** da linha que ela nomeia. Isso
muda o que `Command::Rule { line }` significa: de *"um glifo na linha N"* para **"a fronteira no FIM
da linha N"** — que é o que uma régua sempre quis dizer na tela e nunca disse no tipo. Nada mudou no
WIT: nem campo, nem ordem.

#### Um recuo, e as duas portas param de discordar

`space.buffer.marginX` virou **`space.content.inset`**, e o nome era o defeito. Chamado de "margem do
buffer", ele chegava aos buffers e a mais nada: a região do rodapé pegava `modeline.edgeLeft` e a
listagem do gerenciador pegava `files.gutter` — que é a célula da MARCA e não margem nenhuma. Medido:
a mesma linha de arquivo começava a **10,8 px** no rodapé e a **24,0 px** na janela. Fator de 2,2, e
é exatamente o que foi relatado.

O `crates/ronoctua-shell/src/chrome.rs` é a casa do número, e a regra cabe numa frase: **conteúdo é
recuado por `chrome::INSET` em toda superfície; a modeline e o minibuffer não são conteúdo e ficam com
`modeline.edgeLeft`.** A exceção não é um furo, é a outra metade da regra — aquelas duas linhas são
uma barra de status, e o comentário do token já argumentava que duas células ali "começariam a parecer
margem".

Uma linha que já traz a própria calha paga a DIFERENÇA, e isso é `chrome::FIELDS_LEAD`. As duas
janelas tinham descoberto essa subtração cada uma por si e escrito um comentário explicando; o
rodapé era o terceiro lugar e simplesmente não tinha nenhum. Sobra: nenhuma. Duas células de
`size.base` são 21,6 px, e eram 24,0 px de `size.large` na janela — um quinto de célula, que era da
GRADE e não do layout. Desde que o sistema passou a ter uma medida só, o teste cobra o MESMO pixel,
sem folga.

**E o hit-test desceu junto.** `cell_at` devolvia a célula da tela; com o desenho recuado, ela
descreveria o campo à esquerda do que está sob o dedo, a tela inteira. É a forma de defeito que este
arquivo já pagou duas vezes — o desenho e o ponteiro calculando o mesmo número cada um do seu jeito.

#### O que o `recount` do terminal fazia, e ninguém via

`paint::spans_of` soma o recuo a toda run, e o `recount()` do terminal contava colunas na janela
inteira. **As duas últimas colunas de todo terminal eram escritas fora dela**, e o compositor não
recorta. O `context_of` do `buffer.rs` já subtraía; o `render-png` já somava de volta para achar o
tamanho da janela. Três lugares, uma aritmética, e este era o que discordava. Virou função livre com
teste — um método num struct que precisa de conexão Wayland não tem como ser testado, e era esse o
motivo de ninguém ter olhado.

---

### Um rodapé por sessão — garantido, e não afirmado

O socket de controle tem o display no nome, e por muito tempo um comentário afirmava que outro rodapé
"estaria errado em existir" — sem nada tornando isso verdade. Um segundo rodapé remove o arquivo,
abre o dele por cima, e o anterior continua vivo, com a layer surface na tela e o `wl_keyboard` no
seat.

O estrago não é cosmético. Substituído **com a leader aberta**, o anterior fica com
`keyboard-interactivity: exclusive` para sempre: nenhuma ordem chega mais nele para encerrar. Daí em
diante o rodapé novo **desenha** a lista de atalhos e o velho **recebe** as teclas. Na tela isso
aparece como "a lista abre e nenhuma tecla funciona, nem para abrir submenu" — sem erro em lugar
nenhum, e sem jeito de reproduzir, porque depende de o anterior estar no meio de uma sequência.

Agora o novo manda `quit` pelo socket e **espera o anterior largá-lo** (`leader::yield_the_place`).
Meio segundo: é uma saída limpa de quem já está no laço de eventos, não um desligamento.

#### "Nenhum atalho funciona" tem três causas com a mesma tela

O sintoma é sempre o mesmo — "os atalhos pararam" — e não distingue as três causas possíveis, que se
consertam em lugares opostos:

1. **a tecla não chega ao rodapé**, porque o compositor parou de despachar;
2. **a tecla chega com um modificador preso** e não casa com ligação nenhuma;
3. **o rodapé não tem o foco de teclado**, porque a exclusividade não foi concedida.

Duas medições valem mais do que a intuição aqui, e as duas contrariam o palpite óbvio.

**Um niri aninhado que não responde ao `wl_display.sync` não está travado.** Ele é **cliente do
compositor do host**: com a janela numa workspace oculta, o host para de alimentá-lo e ele dorme — e
dormindo não serve os próprios clientes nem o IPC. Com a janela visível, o mesmo `sync` responde em
0,00 s.

**Modificador preso é o que se parece com tudo o mais.** Com ninguém tocando no teclado, o compositor
chega a anunciar `alt=true logo=true`; o `logo` é a impressão digital do atalho do host, que captura
o `Super+…`, e o niri aninhado vê a descida e nunca a subida. Um fato explica três sintomas:

- **A leader para de disparar** — o niri casa bind por máscara EXATA, e o que chega tem um bit a
  mais do que o bind pede.
- **O clique não chega aos buffers, mas arrasta a janela** — no backend aninhado o `Mod` do niri
  **é o Alt**, e `Mod+arrastar` é mover janela. Com o Alt preso, todo clique vira arrasto. Foi esta
  observação do dono que fechou o caso.
- **O foco ainda muda no clique** — normal, e é o que fazia parecer que "o mouse funciona".

Nem `switch-layout` nem `load-config-file` limpam o latch; ele está no estado xkb do compositor, e o
invariante 3 proíbe consertá-lo lá dentro. O que sobra é reconhecê-lo depressa.

Disso saíram três coisas, e a mais importante não é o conserto:

**O log do rodapé vai para arquivo.** Ele ia para o terminal e mais nada, e o do niri tinha as quatro
linhas da partida — por isso as três primeiras ocorrências não deixaram rastro. Agora `nix run .#dev`
escreve `RONOCTUA_LOG`, o `tail -f` continua pondo na tela, e os logs **não** são apagados no `trap`
de saída: quem encerra depois de algo dar errado é quem mais precisa deles. O check
`footer-logs-to-file` prova as duas metades, inclusive que o pidfile continua apontando para o
rodapé — resolver o log com um `tee` no meio faria o `$!` ser o do tee, que é o defeito que o
`footer-is-single` existe para não ter.

**`nix run .#diagnostics`** roda o mesmo roteiro num comando: o `sync`, as filas, o CPU numa janela de
tempo, a interatividade de teclado da layer surface, **os últimos modificadores que o compositor
anunciou** e o fim dos dois logs. Só leitura — um diagnóstico que manda comando ou muda foco destrói a
evidência que veio buscar. Ele avisa nas duas armadilhas por onde eu passei: um `sync` sem resposta
pode ser só a janela oculta, e `ctrl`+`alt` juntos são normais (a leader **é** `Ctrl+Alt+Espaço`) — o que
denuncia é o `logo`.

E o conserto **do lado de cá**: a herança de modificadores passou a ser recapturada **quando a
sequência começa**, e não só ao ganhar o foco. Ela cobre o caso em que a leader ainda abre e as teclas
seguintes chegam com um modificador preso — que foi a segunda e a terceira ocorrências. Não cobre esta
quarta: aqui a leader nem abre, porque quem recusa a tecla é o compositor, antes de qualquer coisa
nossa. **Esse pedaço não tem conserto do nosso lado** (invariante 3), e o que se pode fazer é
reconhecê-lo em um comando em vez de em uma hora.

#### Quem reinicia o rodapé é outro processo, e por isso o estado vive num arquivo

Quem inicia o rodapé e quem o reinicia depois de recompilar são processos diferentes por construção —
o segundo é o watcher do hot reload. Uma variável de shell com o pid não atravessa essa fronteira, e
sob `set -u` a leitura não vira um `if` falso: vira `unbound variable`, que derruba o watcher inteiro
e nem o `|| true` do laço salva. O sintoma é "compilando…" na tela e mais nada, para sempre.

Por isso o supervisor é pacote (`nix/footer-single.nix`), como o `reload.nix`: separado, o mecanismo
é uma função de (pidfile, binário) para "há exatamente um vivo, e é o novo", que um check prova em
segundos.

E vale a distinção que custou o defeito: o `reload-reloads` prova que o comando **dispara**.
Disparar e reiniciar são coisas diferentes, e a segunda só aparece quando dois rodapés disputam o
teclado.

---

## 3. O rodapé

### O rodapé: onde cada linha fica, e o que aparece nelas

**Modeline e minibuffer ficam coladas; a expansão cresce abaixo e a notificação acima.** De cima
para baixo: a faixa de notificação (0 a 3 linhas), o nome do buffer, a linha do que se digita, e
então a lista — which-key, candidatos do localizador, gerenciador, manual.

A superfície é ancorada **embaixo**, e é isso que torna as duas direções livres: crescer para cima
não move a modeline na tela, come conteúdo. O primeiro da expansão deixou de ser constante por causa
disso — ele é `faixa + 2`, e o `row_at` do ponteiro recebe o número em vez de lê-lo. Com uma
constante ali, toda linha da lista descreveria a de cima enquanto houvesse notificação na tela, e
nada pareceria quebrado: o cursor estaria só uma linha fora.

A posição não é arbitrária, e as duas alternativas caem por defeitos opostos: com a lista **acima**
da modeline, o nome do buffer desce junto e a única linha que diz *onde você está* salta de lugar a
cada tecla; **entre as duas**, as duas linhas que se leem juntas ficam separadas por vinte de lista.
Abaixo do par, o que se lê como uma coisa só aparece como uma coisa só, e o que cresce cresce longe
dele.

A superfície continua ancorada embaixo, então o par sobe conforme a lista cresce — e é por isso que
a lista tem **teto**. A modeline mantém fundo um tom acima: coladas como estão, a diferença de cor é
a única coisa que ainda as distingue, e ela basta (invariante 15).

**A expansão para de crescer.** O teto sai de dois tokens (`space.expansion`): um absoluto em linhas
e um percentual da tela, valendo o menor. Nenhum dos dois serve sozinho — doze linhas são um terço
de um monitor de 1080 px e mais da metade de um de 600, e um percentual sem teto daria trinta e três
linhas numa tela 4K. O que passa do teto vira **página**.

**A página vira com o `CtrlRight`.** Ele é a tecla que **não se soma à leader**: com ele segurado, a
tecla vai para a navegação da região em vez de entrar na sequência. Sem uma tecla assim, paginar
custaria duas letras do keymap que nunca mais poderiam ser comando. O `Modifiers` do Wayland não
distingue Ctrl esquerdo de direito — o xkb já juntou os dois num bit —, então o rodapé rastreia o
keysym `Control_R` na descida e na subida, com ressincronização quando nenhum Ctrl está pressionado:
sem ela, perder o foco com a tecla segurada deixaria toda tecla indo para a navegação.

**A lista mostra o COMANDO, e a descrição vai para o cursor.** Ela mostrava o rótulo traduzido, e
isso respondia à pergunta errada: quem olha a lista quer saber *o que vai rodar*, e o nome do comando
é o que se digita depois no localizador. A frase inteira não sumiu — ela aparece na linha de mensagem
**quando o ponteiro passa por cima do item**. A divisão é essa: a lista é para varrer com o olho, a
frase só quando se pergunta por ela, e apontar com o mouse é a pergunta.

**A barra de rolagem se arrasta, e arrastar não executa.** Apertar o botão sobre ela entra em modo
de arraste em vez de clicar no item que está atrás — a barra fica justamente por cima da ponta
direita das linhas, e quem quer arrastar não quer rodar nada. É o pior tipo de surpresa: a pessoa
queria olhar e o sistema fez.

**Foco e ponteiro são DUAS faixas, e dois tokens.** O foco — o que o `Enter` vai rodar — leva o
`surface.selection`; a linha sob o ponteiro leva o `surface.hover`, mais fraco. Antes era uma cor só,
gasta no ponteiro, e o foco se distinguia pela COR DO TEXTO. Isso não vale no gerenciador: a cor
alcança UM campo de sete (o nome, por `files::colour_of`), uma pasta já é dessa cor, e as linhas do
menu não têm acento nenhum — ali o cursor era invisível. O dono reportou da tela, nas duas portas.
A objeção que a regra antiga fazia continua respondida: com dois tons, duas linhas acesas ainda dizem
qual é qual; e quando são a mesma linha só o foco é desenhado, porque pintar o tom fraco por cima do
forte apagaria justamente a linha em que se está. Medido na tela: fundo 18, hover 34, foco 56.

**A faixa pergunta a LINHA, o acento pergunta o CAMPO.** São duas profundidades de uma resolução só
(`View::band_row` e `View::hit`, com `at_point` embaixo das duas). A faixa diz em que linha de uma
lista o ponteiro está, e uma linha está na tela quer o ponteiro esteja sobre uma palavra dela ou não
— a coluna do nome é `Width::Fill`, então numa janela larga a maior parte de uma linha pertence a um
campo que cobre só as letras do arquivo. A janela perguntava a profunda para a faixa e por isso não
acendia nada nesse espaço; o rodapé nunca teve o defeito porque sempre perguntou a linha.

**A linha de atalhos é UMA decisão, e a fronteira entre a tecla e a prosa é do LOCALIZADOR.** O dono
fotografou as duas portas e a mesma frase saía em `text.primary` no rodapé e em `text.secondary` na
janela — e nenhuma das duas era escolha: a janela escrevia um token no próprio sítio de empurrar o
span, e o rodapé herdava o do braço que existe para "a pessoa está digitando no `M-x`", em que a dica
tinha sido pendurada depois. Agora ela é `chrome::hint_spans`, o único lugar que diz de que cor é uma
dica, e as duas portas chamam ele.

Dentro da linha são DOIS registros — a tecla, que o olho procura, e a frase, que explica — e por isso
ela não pode viajar como `String`. Partir a frase em Rust seria uma segunda cópia da tecla, e erraria
no dia em que um idioma pusesse `C-m` noutro lugar (medido: en-GB e pt-BR já discordam do
deslocamento). Quem sabe onde o argumento ficou é o Fluent, que envolve todo interpolado numa marca
de isolamento — o `ronoctua-i18n` tirava essa marca em DOIS lugares, um deles morto; agora tira num
só e devolve as faixas junto (`Texts::t_marked`). Ser argumento não basta para virar tecla: o
`files-panel-no-room` interpola um NÚMERO, então o trecho também tem de ser uma das notações que
aquele chamador passou.

Os dois tons são `text.subtle.strong` (a tecla) e `text.subtle.default` (a prosa), nomeados por
REGISTRO e não por lugar, a pedido do dono, para que outra linha discreta possa usá-los. E o preço
está dito onde eles moram: o vão entre `text.secondary` e `text.dim` vale 2,46× em luminância, os
degraus da própria rampa valem 2,2–2,5×, então dois degraus novos lá dentro são necessariamente mais
finos que qualquer um — 1,34× entre si. É aritmética, não gosto.

**A linha do minibuffer ABRE, e a altura aberta é uma CONSTANTE.** Ela mostra uma linha e corta o
resto; o dono pediu para ver o resto, e a ideia foi dele: um controle na borda direita quando o
conteúdo não cabe, e ao acioná-lo a linha vira várias, com barra quando ainda sobra. O controle é um
chevron — `icon.expand` fechado, `icon.collapse` aberto —, sem caixa e sem borda (invariante 15), e
ele recua o texto antes de a medida ser tirada, como o glifo de ejetar da barra lateral já faz.

**A altura aberta segue o CONTEÚDO, com teto, e isso foi uma escolha contra uma regra.** A altura do
rodapé é a mesma em todo output (`Footer::apply_geometry`), então uma quebra que dê duas linhas no
laptop e uma no monitor faz todo buffer refluir ao mover o foco de tela, sem nada ter mudado neles —
é o argumento que o `band::rows` faz sobre a notificação, e é por isso que os renques extras da faixa
são o CORPO e as AÇÕES, estrutura e não quebra.

A primeira versão obedeceu a regra: altura constante, texto rolando dentro. O dono viu na tela — uma
frase curta abrindo numa gaveta com quatro renques vazios — e escolheu a outra troca. Aberta, a
gaveta tem o tamanho do que há nela, até `modeline.foldLines`; passando disso, **rola**, e é esse o
emprego da barra. O preço está dito: é pago só enquanto uma gaveta está ABERTA, que é um estado em
que alguém entrou de propósito e de que sai com uma tecla, e nunca passa do teto.

**E a altura é perguntada ANTES do quadro.** Quem diz ao compositor quanto a superfície mede roda
antes do pintor, e a quebra só é conhecida dentro dele. Então o desenho anota quantos renques deu
(`fold_wrapped`) e a geometria lê essa anotação — atrasada de um quadro exatamente quando o texto ou
a largura acabou de mudar. O `draw_and_paint` compara e dá mais uma volta quando o número mexeu; a
guarda é o próprio número, então nada entra em laço.

**E os renques abertos entram no termo FIXO do teto.** Medido na tela: com o teto de 40% já no
limite, abrir a linha não empurrou o desktop — a região expandida é que cedeu as linhas. Deixá-los de
fora seria o rodapé passar do orçamento sem que nada fosse culpado.

**A gaveta é a linha CONTINUANDO, não uma segunda cópia dela.** O primeiro renque da quebra ocupa a
própria linha do minibuffer, no lugar da versão elidida; os outros seguem abaixo. Desenhada como
faixa sob um resumo intacto, as primeiras oito palavras apareciam duas vezes.

**E ela quebra TRECHOS, não uma string.** A linha é dois registros — a notação de uma tecla e a prosa
em volta — e quem sabe as cores é o `chrome::hint_spans`. Quebrando o texto juntado, a gaveta voltaria
como um único cinza, e a mesma frase seria desenhada de um jeito na primeira linha e de outro três
linhas abaixo. O espaço de junção fica no trecho ANTERIOR: com ele à frente do seguinte, `C-m` seria
` C-m` e a corrida pintada como tecla começaria uma célula cedo.

**O `Tab` alcança o controle, e é por isso que ele é um `Pane`.** O anel de foco é um só no sistema —
`View::cycle_pane` sobre `panes_on` — e o dono pediu o controle nele. `Pane::Fold` não é seção e não
tem linha: o que é do gerenciador é o FOCO que chega lá, e a linha continua sendo da porta, que
mostra um eco ou o prompt do `M-x` com a mesma facilidade. Quem diz se o controle existe é a porta
(`View::fold_is_shown`), pela mesma via por que ela já diz a largura desenhada — a regra escrita do
`panes_on` é que um painel que não é desenhado não pode ser alcançável. `Enter` alterna, as setas
andam na gaveta, e o foco se vê no próprio glifo: um glifo não tem linha para receber faixa.

**A linha de dicas do gerenciador é GERADA da tabela de teclas, e isso conserta um defeito de
espécie.** Ela ensinava onze das quarenta ações, escolhidas à mão em quatro frases; `C-w` — a tecla
que dá a região inteira à prévia — não estava entre elas, e o dono reportou como atalho faltando.
Marcar, ordenar, a playlist e o transporte do filme também não estavam. Nenhuma das quatro frases
estava errada: eram uma SEGUNDA lista, mantida à mão ao lado da primeira, e uma segunda lista é uma
lista que fica para trás.

**E as quarenta frases já existiam.** Todo id de `OF_THE_VIEW` tem valor nos dois `.ftl` desde que a
tabela existe, porque o `files-actions-have-text` exige um para todo literal `files-…` do arquivo —
e `files::OF_THE_VIEW` não entra no `leader::reachable`, então nenhuma delas estava no `M-x` nem no
manual. Quarenta frases traduzidas que nenhuma tela lia. `View::hint` as lê, na ordem da tabela, uma
notação por ação — a PRIMEIRA, o que faz da ordem da tabela uma decisão de ensino.

**O separador é do código, e por isso a regra é estrutural.** *"Cada atalho separado pelo ponto, e o
atalho sempre antes do texto"* é uma propriedade da LINHA, não da frase: um `·` dentro de um valor
`.ftl` não pode ser contado nem receber cor. Ele é `chrome::Piece::separator`, sai de
`mark.separator` (invariante 9 alcança o codepoint) e "um atalho entre dois pontos" passa a ser
consequência do laço em vez de convenção que dois idiomas têm de lembrar.

**E uma tecla é desenhada em DUAS corridas, para que nenhuma ligadura atravesse o modificador.**
`C->` saiu na tela como `C-→`. A face liga `->` — é para isso que a feature existe, e numa linha de
código está certa —, mas aqui ela transforma uma notação em OUTRA tecla, e ainda por cima numa que
este sistema proíbe: seta com `Ctrl` é o acorde de paginar do rodapé, tomado antes de a vista ver.
A linha existe para ensinar quarenta teclas e estava ensinando uma que não existe. Era invisível até
a linha passar a mostrar a tabela inteira — `C->` está ligado desde que a tabela existe e não era
desenhado em lugar nenhum. O `cosmic-text` molda uma corrida por vez e não expõe como desligar uma
feature, então o conserto é as duas letras não estarem na mesma corrida; como cada glifo da dica tem
uma célula e a posição é aritmética deste módulo, duas corridas caem exatamente onde uma caía. Vale
para TODA tecla e não só para o `>`: quais pares uma face liga é assunto da face.

**São QUATRO registros de cor, e o quarto é uma tecla que muda de sentido.** `S-<setas>` eram o giro
da peça 3D e o dono as quis para abrir e subir. Em vez de mudar o giro para uma tecla arbitrária —
`M-` é do compositor, letra nua é letra da busca, e seta com o `Ctrl` DIREITO é paginar numa porta e
nada na outra —, ele escolheu o contrário: *"faça com que o Shift+Setas mude a funcionalidade de
acordo com a seção que está em foco"*. `View::strong_meaning` resolve cada uma para o id de uma ação
REAL antes do `match` da tabela; a seção responde por uma direção quando tem o que dizer sobre ela,
e senão a LISTAGEM responde — nenhuma das quatro é morta em seção nenhuma, que é o que o
`every_view_key_in_the_table_actually_does_something` cobra. Na linha elas levam a frase da ação
resolvida e `text.subtle.alternate`, porque uma tecla cujo sentido anda debaixo do leitor tem de
dizer isso.

**O `C-o` saiu, e a lição dele é o motivo de tudo acima.** Era um segundo caminho para uma seção que
o anel do `Tab` já alcança, e o dono pediu um só. Era também a tecla que este arquivo guardava como
lição: implementada, funcionando, e nomeada em lugar nenhum — *"o que é o mesmo que não ter"*. A
lição sobreviveu à tecla.

**As setas ←/→ passaram a andar entre as seções, e o `TAB` continua ligado.** O dono pediu nessas
palavras, e `TAB`/`S-TAB` ficam ligados fora da lista: quem chega de qualquer outro programa aperta
`TAB` primeiro, e o gerenciador de notificações diz `TAB pane` a uma sala de distância — dois
gerenciadores do mesmo shell discordando do `TAB` é a divergência que as duas portas existem para
impedir, um nível acima.

**A prévia e os fatos ganharam CURSOR, e a marca de foco é a RÉGUA acesa.** Com o `Tab` num
painel nada mudava na tela, e o renque que os painéis DESCREVIAM perdia a marca ao mesmo tempo —
dois mecanismos certos (`focused_here` responde pela seção com o teclado; `colour_of` só acentua com
`Pane::Listing`) que juntos deixavam os painéis falando de um arquivo que nada apontava. Agora: a
playlist e os fatos têm cursor próprio, com a vista rolando atrás dele; e o renque da listagem fica
com uma faixa mais quieta (`surface.hover`, que já quer dizer "esta linha, sem o teclado"). Uma
acesa e uma apagada nunca são lidas como dois cursores.

**Duas respostas para o foco foram recusadas da tela antes desta.** A primeira foi um TÍTULO por
painel: custava uma linha do painel e era texto onde um tom bastava. A segunda foi a BARRA da seção,
acesa — e essa passou por dois desenhos, porque o primeiro pintava o TRILHO de azul com o polícia
cinza por cima e se lia como barra de progresso, com a parte acesa justamente onde não se está.
Acender o punho consertou a leitura e o dono recusou a marca de todo modo: uma célula inteira de cor
é muito para dizer só "aqui está o teclado".

**A marca é a RÉGUA em que a seção começa** (`chrome::divider_lit`, `files::focus_rule`). É o único
elemento já desenhado ao lado de uma seção, tem um pixel lógico de largura — o que a torna discreta
onde uma célula acesa não era —, não cobre conteúdo nenhum e não custa linha. Renque não serve às
duas seções, porque foto, filme e peça não têm renque; régua serve a qualquer conteúdo.

**E a régua que a faixa não tem é DESENHADA.** `Panes::rules` pula a régua antes do primeiro painel
quando eles descem para baixo da listagem — com razão, porque não há nada à esquerda dela para
separar, e uma linha na célula zero com nada atrás já foi um defeito fotografado. Isso deixava um
buraco exatamente onde a prévia seria marcada. Uma fronteira que existe PORQUE a seção está em foco
não é a mesma afirmação que uma que está sempre lá, então ela é desenhada, e some com o foco.

**E a faixa de um painel sai do NOME da seção, nunca de uma célula.** `band_of` resolve célula →
seção, e com os painéis EMBAIXO da listagem as mesmas células pertencem às duas: a faixa do foco
saía com a largura da lista acima do fato em que o cursor estava. Fotografado na primeira tentativa.
`files::band_for` responde onde uma seção NOMEADA está; uma célula não pode responder, porque a
pergunta não é "o que há nesta célula".

**A ALTURA DA GAVETA É MEDIDA UMA VEZ, e o dono achou o laço que a alternativa causa.** Aberta, a
gaveta seguia o conteúdo; e o conteúdo dessa linha é o que o PONTEIRO está tocando. Ele descreveu
cada volta: com a gaveta aberta, pousar sobre algo que se explica punha uma mensagem curta na linha,
a faixa encolhia para caber nela, tudo abaixo subia a diferença, o renque saía de baixo do ponteiro,
a mensagem sumia, a faixa voltava a crescer, o conteúdo descia, o renque voltava para baixo do
ponteiro — e de novo. Nada na cadeia está errado sozinho; o que está errado é a ALTURA de uma faixa
ser consequência do que o ponteiro toca. Então ela é medida ao ABRIR (`fold::settle`, que responde
uma vez e nunca mais) e guarda esse tamanho até fechar: mensagem curta é desenhada dentro dela com
renques em branco embaixo, e mensagem longa rola, que é para o que a barra serve. Foi o que o dono
pediu — *"o espaço de múltiplas linhas deve continuar como está, e dentro desse espaço será mostrada
a mensagem"*.

**E a barra da gaveta não arrastava na janela.** Essa porta resolve um clique andando pelas SEÇÕES do
gerenciador, e a linha 0 não é de seção nenhuma — é da porta, acima da região. Então nada que a
janela fizesse podia enxergar aquela barra, e a pressão caía no renque de baixo; o rodapé tinha o
gesto desde que a gaveta existe. O gêmeo dele agora existe aqui, com uma diferença que só o clique
revelou: **o chevron e a barra dividem a última célula da linha 0**. O controle vence, porque é o que
se mira e é o único jeito de fechar a gaveta com o mouse; a barra perde uma linha de área de pega e
nada mais — o arrasto continua mapeando contra o trilho INTEIRO, então quem pega abaixo pode subir
até o topo.

**A pasta `..` é o primeiro renque, e é o único a que nada pode ser feito.** `read_dir` nunca a
devolve — é propriedade da syscall —, então ela é posta DEPOIS da ordenação: não é um dos nomes
sendo ordenados, é a saída, e uma saída que fosse para o meio da lista ao ordenar por tamanho seria
uma saída que se procura. Na raiz não há renque, porque não há para onde ir. `Entry::hidden` a
excepciona (começa por ponto e não é oculta: esconder a saída com `C-h` a tiraria de quem apertou),
o cursor abre PASSANDO por ela, e ela carrega o modo, o dono e a data reais do pai — por isso
**toda célula dela responde a saída**: um bit de permissão clicável ali mudaria a pasta em que se
está, a partir de um renque que diz "voltar". Marcar, medir e o menu a recusam do mesmo jeito, e
`focused_path` resolve o pai em vez de devolver um caminho com `..` dentro.

**Clicar numa linha é apertar a tecla dela.** Num grupo, desce um nível; num comando, executa. As
duas coisas saem de graça da mesma definição, e o que se ganha é não haver um segundo caminho: nada
fica alcançável só pelo mouse, e o que o mouse faz é sempre algo que o teclado também faz. No
localizador, clicar é escolher e confirmar — e passa pelo mesmo `Enter`, senão uma ação que exige
argumento seria disparada sem ele pelo mouse enquanto o teclado pergunta.

**Duas navegações, e a diferença não é de gosto.** A região expandida serve a dois tipos de
conteúdo, e misturá-los dá o pior dos dois — barra que salta e contador que mente:

| | rola | pagina |
|---|---|---|
| quem | manual, localizador, histórico, **gerenciador — menu e ficha inclusive** | lista de atalhos |
| barra de rolagem | sim | não |
| contador `1/2` | não | sim, com a tecla que vira |
| arranjo | uma coluna, lista | colunas lado a lado |

**O menu do gerenciador é uma SEÇÃO, e por isso tem barra.** Ele rola — só a lista de atalhos pagina
—, mas passava ao lado das seções: cada porta carregava as linhas do modal por fora
(`expansion_window` no rodapé, uma seção falsa de `total == lines` na janela), e o que uma lista fora
das seções não ganha é barra, entalhe para arrastar, roda que para onde a barra para, faixa de hover
e faixa de foco. Eram cinco coisas que já existiam e nenhuma alcançava um menu de dezessete linhas
numa região de doze: cortado, sem nada dizendo, e sem como chegar no resto. Agora
`View::sections_of` o constrói como a seção da LISTAGEM — mesma coluna, mesma largura, `above` zero —
e ele tem offset próprio (`modal_offset`), porque a posição da listagem tem de sobreviver a um menu
abrir e fechar por cima dela.

Um manual é texto mais longo que a tela: ele rola, e o que diz onde você está é a barra — "página 2
de 5" não responde isso. Uma lista de atalhos é o contrário: mostra **o que cabe** e o resto é outra
página; barra ali seria um controle contínuo para conteúdo que anda em saltos, e o que falta dizer é
quantas páginas há e **qual tecla vira**. O contador vem com a tecla ao lado esquerdo justamente
porque um `1/2` sozinho diz que existe outra página e não diz como chegar nela.

**A lista de atalhos usa a largura antes da altura.** Dez atalhos numa região de doze linhas
caberiam em coluna única — uma tira estreita com a tela inteira vazia ao lado, e ainda
paginando ao passar de doze. Preenchendo as colunas que cabem, os mesmos dez viram poucas linhas e o
olho varre um bloco. O preenchimento é **por coluna** (desce a primeira, depois passa à seguinte),
que é o que mantém a ordem alfabética legível de cima para baixo.

Isso trouxe uma consequência no ponteiro: com colunas lado a lado, a linha sozinha não identifica o
item — a linha 2 existe em todas. Ignorar o `x` faria o cursor descrever, e o clique **executar**, o
item da primeira coluna sempre.

**O localizador rola atrás do cursor, e não pagina.** A seta desce o item em foco e a lista o
acompanha, começando a rolar **duas linhas antes** da borda — o `scrolloff` do vi. Paginar ali
contava a história errada: numa busca o que importa é o que casou, não em que fatia da lista se está.
O indicador `3/15` some quando o localizador está aberto; a barra de rolagem diz a posição para quem
quiser.

**As letras que casaram acendem.** Com busca por subsequência, `jme` filtrando
`window-move-left` parece arbitrário até as três letras aparecerem em outra cor. O casamento é
**guloso à esquerda**: preferir início de palavra leria melhor em kebab-case, mas pode escolher uma
posição que impede o resto de casar — e aí um candidato válido sumiria da busca. Um realce numa letra
vizinha é barato; um item que some, não.

**Um defeito que essa lista expôs: pedir tamanho não é pintar.** `aplicar_geometria` só comita o
tamanho novo; quem pinta é o `configure` que volta, e o compositor **só o manda quando a altura
muda**. Com a lista no teto, filtrar de 172 para 40 candidatos não muda altura nenhuma, e mover o
cursor também não — então digitar não filtrava na tela e a seta não descia. Tudo aparecia de uma vez
ao mexer o mouse, que era a única outra coisa que pintava. Toda mudança de estado agora pinta
explicitamente.

**Os dois-pontos alinham a lista.** `RET : abrir terminal` e `  a : ajuda` — a tecla encostada à
direita da própria coluna, os dois-pontos de todas as linhas na mesma coluna, uma célula de respiro
de cada lado. Um vão fixo — largo o bastante para um `C-M-S-s-X` — faria o olho atravessar o vazio
para achar o que uma tecla curta faz. No localizador o
alinhamento se inverte — cento e setenta nomes de comando encostados à direita dariam uma margem
esquerda serrilhada.

**O `+` do submenu é posto no desenho, e não na tradução.** Dentro do `.ftl` (`group-window = +janela`)
nada obriga que os seis grupos o tenham, e o esquecimento não aparece em lugar nenhum. Ele sai do
`is_group` que a árvore já sabe, e um check recusa `+` no `.ftl`. O rótulo de grupo tem tom próprio
(`text.group`), porque um submenu não *faz* nada: ele leva a outro nível.

**A expansão empurra os buffers, não os cobre.** O `exclusive zone` cresce junto com a superfície,
então abrir a lista redimensiona o que está atrás. Cobrir o conteúdo esconde justamente a linha que
se estava lendo — e num sistema sem popup (invariante 14), a superfície do rodapé é a única coisa
capaz de cobrir algo. O custo é o reflow: os buffers recebem `configure` a cada expansão.

**A modeline só mostra o que existe.** Sem janela em foco e sem nada acontecendo ela fica VAZIA —
não há contador de notificações, e o sino só existe com não lida (o dono pediu assim); assim que algo
abre, o nome do buffer aparece. Um rótulo de ausência inventado ocuparia coluna afirmando algo falso,
e quem o lê procura um buffer que não existe.

A sequência pendente aparece como `LEADER g-`. O `-` no fim custa duas colunas a menos que ` …`
dizendo a mesma coisa, e num rodapé onde tudo elide, espaço é o recurso.

**O rótulo da leader vem do ambiente**, posto por quem define o bind, e são **dois**:
`RONOCTUA_LEADER` (a sequência) e `RONOCTUA_LEADER_CHORD` (o que se segura). Em produção os dois são
`SUPER` — e essa coincidência é o que o keyd compra. No aninhado eles divergem: a sequência é
`CTRL ALT SPC` e o acorde é o `ALT` sozinho. Enquanto a tela chamava os dois de "a leader", segurar "a
leader" parecia incluir o Espaço — e incluir o Espaço **dispara a sequência**, que é o oposto do
acorde. Deduzir um do outro seria a tela ensinando o gesto errado.

---

### Preto puro, e o foco visível

O fundo do sistema é **`#000000`**. Numa tela OLED o preto é pixel desligado: contraste máximo e
menos bateria, que é o que importa num laptop. Superfícies — rodapé e buffers — são cinza escuro, e a separação entre conteúdo e chrome
passa a ser a diferença entre preto e cinza, sem régua nem caixa.

**A borda de foco de 2 px é do compositor, não nossa.** Com `border` e `focus-ring` desligados, duas
janelas abertas eram indistinguíveis: nada na tela dizia qual recebia tecla. Uma linha não é card,
sombra nem elevação (invariante 15) — é a mesma classe da régua de uma linha que o desenho já admite
como única separação além do espaço em branco. Quem a desenha é o niri (invariante 4).

**A borda inativa é vermelho escuro, e isso é o pedido possível.** O pedido foi: ao apertar a
leader, o contorno azul deveria **mudar de cor** em vez de sumir. A parte impossível é manter o
azul só na janela que estava em foco — nenhuma janela está em foco enquanto o rodapé tem o teclado, e
não há como pedir o contrário ao compositor: uma janela sem teclado não está ativa, e é isso que a
borda conta. O que dá para escolher é a cor que fica no lugar. Com `border.inactive` em vermelho
escuro, apertar a leader muda a cor do contorno — que é o efeito pedido. O preço, dito claramente:
janela sem foco também fica vermelho escuro em vez de cinza.

**E o acento muda de lugar quando o rodapé pega o teclado.** Enquanto a leader, o localizador ou o
manual estão abertos, *nenhuma* janela está em foco para o compositor — e não há como pedir o
contrário: uma janela sem teclado não está ativa, e é isso que a borda conta. A cor inativa era
`surface.buffer`, a mesma do fundo da janela, então as duas ficavam sem contorno nenhum e a tela
perdia toda indicação de limite. Agora ela é `rule.default` (visível), e o **azul sai da janela e
entra no nome do buffer na modeline**. Continua havendo exatamente uma coisa azul na tela, e ela
continua dizendo qual buffer recebe o que vier depois.

As três cores do config do niri — acento, borda inativa e fundo — vinham escritas à mão, iguais por
coincidência aos tokens de mesmo nome. Coincidência mantida à mão é o que o invariante 9 proíbe:
mudar `state.accent` mudava o rodapé e deixava a borda na cor velha. Agora elas são derivadas de
`assets/tokens/colors.nix`.

O corpo do texto subiu de 14 px para 18 px, e a entrelinha de 1,25 para 1,45. E o conteúdo dos
buffers ganhou margem — sem ela o primeiro caractere encosta no limite da janela e parece corte.

#### Havia DUAS medidas, e agora há uma

Por muito tempo o rodapé desenhava a 18 px e toda janela nossa — gerenciador, notificações,
atualização, terminal — mais o corpo dos buffers, a 20 px. A razão estava escrita: *"o buffer é maior
que o rodapé de propósito: o rodapé é referência periférica, o buffer é o que se lê"*. Existia até um
`chrome::Door` para nomear essa diferença, de modo que um teste pudesse dizer que as portas diferiam
**nisso e em nada mais**.

O dono fotografou as duas portas do gerenciador lado a lado e pediu a medida do rodapé em toda parte,
buffers inclusive — é a que ele lê o dia inteiro, e um sistema cujas metades estão 10% distantes
parece montado com peças de origens diferentes. Medido na foto: passo de linha de 26,1 px contra
29,0 px, que é 18 × 1,45 contra 20 × 1,45.

O que a mudança **derruba** é mais interessante que o número: sem uma segunda medida, o `Door` não
tinha o que nomear e saiu, e o `both_doors_put_content_in_the_same_place` deixou de tolerar uma
diferença de até uma célula — as duas portas começam o conteúdo no MESMO pixel, e o teste diz isso
sem folga nenhuma. Dois efeitos de lambuja: o `font.size.icon = 22` foi MEDIDO contra texto de 18, e
as janelas desenhavam ícone de 22 ao lado de texto de 20 — agora batem com a medida que o
`chrome-icons-match-the-text` reafirma; e o `xxlarge` foi reancorado de 116 para 104,4, porque o
comentário dele promete "exatamente quatro linhas de célula" e a célula mudou.

O que **fica**: `Scale::Base` e `Scale::Normal` passam a dar o mesmo pixel. Os dois nomes ficam, e
respondem a perguntas diferentes — `base` é "o que é INTERFACE dentro de um buffer" e `normal` é o
conteúdo dele. Duas perguntas que chegaram a uma resposta não são uma pergunta feita duas vezes.
Juntá-las é mexer no WIT, que reconstrói todos os buffers, e ninguém pediu isso.

---

### Os três segmentos da esquerda: o buffer, o modo e o estado

À esquerda da modeline ficam três coisas, e cada uma responde a uma pergunta diferente:

| segmento | pergunta | de onde vem |
|---|---|---|
| `*lucide*` | **qual** buffer é este? | o TÍTULO da janela, pelo niri |
| `lucide` | que **espécie** de buffer é? | o último trecho do `app_id` |
| `filtro` | o que ele está **fazendo**? | o que o próprio buffer relata |

Havia um quarto, e ele saiu: o **ramo do git**. Ele lia o `cwd` do processo da janela em foco e dos
filhos dele, em busca em largura pelo `/proc`, a cada troca de foco. Custava isso mais até vinte
`stat` subindo diretórios, para uma informação que em produção nem aparecia nos buffers — a unidade
do rodapé não tem `WorkingDirectory`, então o `cwd` é `$HOME` e não há repositório nenhum. O dono
pediu a coluna de volta, e ela foi para o estado.

#### O identificador é inglês, e a tela vem do i18n

Os dois primeiros segmentos foram desenhados CRUS por muito tempo, e é assim que `glifos` chegava a
uma tela em inglês. O descritor do componente dizia `name: "glifos"`, o host escrevia `*glifos*` no
título e `br.ronoctua.glifos` no `app_id`, e a modeline lia os dois de volta. **Um literal de UI
(invariante 8) e um nome em português (invariante 18) na mesma linha**, pela única porta que nenhum
check olhava — porque nenhuma regra de declaração enxerga uma string à direita de dois-pontos.

Hoje o id é inglês e o que a tela mostra vem de `buffer-<id>` e `mode-<id>`. Quem traduz o TÍTULO é o
host do buffer, no `set_title`; quem traduz o MODO é o rodapé, e só quando o `app_id` começa com
`br.ronoctua.` — o `app_id` de um app estrangeiro é dado dele, como o título, e inventar um nome para
o software dos outros é pior que mostrar o que eles escolheram.

**O `app_id` NÃO é traduzido**, e isso é deliberado: as *window rules* do niri casam por ele, e uma
regra que só dispara num idioma é uma regra que falha na máquina de quem mudou o `LANG`.

#### Onde cabe um idioma só, ele é o inglês

O invariante 18 tem duas metades, e a segunda foi escrita depois — porque a primeira sozinha deixava
uma pergunta sem resposta, e a resposta errada estava espalhada por 2 071 literais.

A primeira metade é a que já estava: **nome, comentário e commit são inglês**. A segunda responde
*onde o português pode aparecer*, e a regra é uma só: **pt-BR é um VALOR que tem par en-GB**. Vale
para os `.ftl`, para a tag de busca, para o léxico (chave inglesa, valor português) e para os seis
docs, que existem em português por decisão do dono. Fora disso não há par, e **sem par o idioma é o
inglês**.

Cinco lugares só têm um idioma, e por isso são ingleses: descrição de unidade do systemd, entrada
`.desktop`, `description` do manifesto da extensão, alias do shell e `Job::trouble` — o texto livre
que a fila põe na linha quando uma transferência quebra. O `trouble` é o caso que mostra por quê: ele
atravessa para a tela sem passar por chave, então uma frase em português ali é lida por quem escolheu
en-GB, sem tradução e sem volta. É o mesmo defeito do `[chave]` do invariante 8 com o sinal trocado:
não é o nome da chave aparecendo, é a chave que nunca existiu.

O mesmo raciocínio cobre o que **ninguém traduz**: mensagem de `assert`, de `bail!`, de `.context`,
saída de check e ajuda de linha de comando. Não há um `.ftl` para elas e nunca vai haver — logo, um
idioma, logo, inglês.

**O que cobra é parcial, e é honesto dizer.** O `english-names` lê nome e comentário — declaração,
campo, variante, `let` (padrão inteiro, tupla incluída), `fn` e parâmetro. Ele **não** lê literal de
string: um `bail!("a fila nao subiu")` passa. Quem segura essa metade hoje é a revisão, não o build.

---

### O estado do buffer, e por que o `Esc` é resolvido no cliente

O `mode` nunca foi um modo: é um campo constante do descritor, fixado no `init()` e nunca mais
tocado. Sub-estados sempre existiram de fato — no lucide, no glyphs e no unicode, letra é busca e há
uma ficha; no files, letra é query; no transfers há lista e detalhe — e nenhum tinha nome, então a
modeline dizia a mesma palavra com qualquer um deles na tela.

Agora o descritor declara `states`, o primeiro é o base, e o id dele **tem de ser `normal`** —
recusado no load, ao lado da guarda do prefixo `m` (§7.9). Não é cerimônia: é por esse id que a cor
decide entre o tom discreto e o alternativo, e é para ele que o `Esc` volta. Um buffer cujo base se
chamasse outra coisa se desenharia como permanentemente fora do normal.

#### `Esc` tem dois significados, e o keyd não podia dar o segundo

Toda tecla deste sistema com dois significados os ganha do **keyd**, que separa toque de retenção
antes de o compositor ver qualquer coisa. Para o `Esc` isso não serve: uma camada do keyd sobre ele
mudaria o `Esc` do terminal, do `vim` e de toda página do navegador — para dar um atalho a seis
janelas.

Então ele é resolvido **no cliente que já tem o teclado**. Enquanto um buffer nosso está em foco,
quem recebe as teclas é o processo dele; enquanto qualquer outra coisa está, nada disto existe. É a
razão inteira de a funcionalidade poder usar o `Esc`.

| | |
|---|---|
| `Esc` desce | marca, e **não entrega nada** — ainda pode virar retenção |
| `Esc` + `←`/`→` | gira o estado, e marca que não foi toque |
| `Esc` + outra tecla | **engolida** |
| `Esc` sobe sem uso | entrega `Cancel`, e só |

O preço é que o toque age no `release`. Para um toque isso é o tempo de soltar a tecla, e é o mesmo
contrato que a leader já ensina no `Super`.

**E o toque entrega `Cancel`, e só isso.** A primeira versão mandava o buffer para o base e *depois*
entregava o `Cancel`, com o argumento de que "o `Esc` volta ao normal" deveria ser garantia do host e
não confiança em seis componentes. Custou uma regressão de verdade: os seis já andavam **um nível por
`Esc`** — a ficha para a grade, e então a grade limpando a busca —, e o salto fazia os dois de uma
vez, então sair de uma ficha jogava fora o filtro que a pessoa tinha digitado para achá-la. Um nível
por toque é o que o Doom faz e o que os seis já faziam; a modeline continua verdadeira porque a volta
se relata sozinha, pelo mesmo `state-changed` de qualquer outra mudança.

Três detalhes que custaram teste próprio, e cada um é um jeito de a coisa falhar em silêncio:

- **repetição não é segundo press.** Segurar faz o compositor mandar mais, e tratar um deles como
  press novo limparia o "foi usado" — `Esc`+seta segurado um instante a mais cancelaria a busca;
- **teclas que não são seta são engolidas.** Sem isso, soltar depois de `Esc`+letra cancelaria E a
  letra teria entrado na busca no caminho: um gesto fazendo duas coisas que ninguém pediu;
- **release sem press não cancela nada.** Alt-Tab para dentro de um buffer com o `Esc` em baixo dá a
  esta janela o release e nunca o press. É a mesma armadilha que o `CLAUDE.md` registra para a
  leader: modificador segurado tem de ser descontado ao ganhar foco.

#### Os dois sentidos, e por que os dois

O host manda `input::enter-state`; o componente responde `effect::state-changed`. **Um só não
bastaria**: digitar uma letra no navegador de ícones entra na busca, e ninguém apertou uma tecla que
significa "entrar na busca" — sem o efeito de volta, a modeline seria verdadeira apenas sobre as
mudanças que o próprio host causou.

**E o host não anota nada por conta própria.** A primeira versão guardava o índice e avisava o
rodapé *antes* de entregar — que é a ordem que parece natural e está errada: **um componente pode
recusar**. `sheet` sem nada sob o cursor é uma tela de campos vazios, e os navegadores dizem não; o
host teria anunciado um estado em que a tela nunca esteve, e a modeline seguiria nomeando ele até a
mudança seguinte. Quem escreve o estado é o `state-changed` que volta — uma verdade só, e é a que
desenha.

Do lado do componente o relato sai de **um lugar só** — comparar o estado antes e depois de tratar a
entrada — e não de cada sítio que atribui `screen`. São de quatro a doze desses por buffer, e relato
por sítio é relato que alguém esquece.

#### Como o estado chega ao rodapé

O buffer é um processo à parte, e o rodapé só sabe o que o compositor conta: título, `app_id`, pid.
Então há **um empurrão e uma pergunta**, e um cobre o outro:

- o host do buffer **empurra** `state <pid> <id>` no socket de controle do rodapé, a cada mudança.
  Cobre tudo que acontece com a janela em foco, que é quase tudo;
- o rodapé **pergunta** (`states`, no socket que já existia do buffer) a cada troca de foco. Cobre um
  rodapé que reiniciou com buffers abertos — trocar de rodapé é uma tecla — e um buffer que mudou de
  estado sem foco.

Perguntar por quadro seria uma ida e volta de socket dentro do orçamento do quadro, para algo que
muda quando alguém aperta uma tecla. O mapa `pid → estado` é **podado contra a lista de janelas** do
compositor, e não por relógio: um pid é reusado pelo kernel mais cedo do que se espera.

**Janela estrangeira não tem estado**, e é a mesma regra que apagou o rótulo `fundamental`: o niri
não nos diz em que estado o `vim` está, e escrever `normal` ali gastaria uma coluna afirmando algo
que ninguém conferiu.

#### Duas cores, e não uma por estado

A rampa, de cima para baixo: o NOME do buffer é `text.primary`, o modo é `text.secondary`, e o
estado base é o mais apagado dos três — `normal` é o que diz "nada em particular", então recua. O
modo já esteve em `text.dim`, e quem disse que era escuro demais foi o teste de VM: o OCR parou de
achar `foot` na tela. **Teste que lê mal é pessoa que lê mal.**

`normal` sai discreto e **todo o resto sai no mesmo verde**, num token só
(`modeline.state.alternate`). O que um leitor tem de aprender é "discreto é o de sempre, verde é
não", e isso vale em qualquer buffer; trocar de tema é mexer numa linha. Um buffer PODE sobrepor por
estado, nomeando um papel (`tone`) no descritor — nome, nunca valor (invariante 11) —, e nenhum o faz
hoje.

**O acento não entra aqui.** Azul marca foco e mais nada: vive na borda da janela e se muda para o
nome do buffer quando o rodapé toma o teclado. Um estado em azul seria a segunda coisa azul na tela,
e aí nenhuma das duas diz "é aqui que a tecla vai".

#### O ponteiro alcança as duas linhas

O rodapé já tinha ponteiro — barra de rolagem, linhas do gerenciador, menu do botão direito — mas só
dentro da região expandida. Agora a modeline responde também: o segmento sob o cursor se explica **no
minibuffer** (nunca em tooltip, invariante 14), o botão esquerdo gira para o próximo estado e o
direito para o anterior. São exatamente as duas ordens que `Esc`+seta manda: **o mouse é um segundo
caminho para a mesma porta, e nunca uma porta própria** — e há check dos dois lados disso.

O que ele usa para saber onde está é o que o ÚLTIMO QUADRO desenhou, guardado como `(id, coluna,
largura)`. Não é economia: um `Motion` chega por pixel de movimento, e um segundo cálculo do layout
poderia discordar do que está na tela.

---

### O teto de textura não é um detalhe do wgpu, é o tamanho do monitor suportado

`Limits::downlevel_defaults()` foi escolhido para "ampliar onde a distro roda": ela desenha glifo e
quad, e não precisa de GPU moderna. Só que ele fixa `max_texture_dimension_2d` em **2048**, e a
superfície de apresentação **é uma textura**. O limite de compatibilidade virou, sem ninguém decidir
isso, o maior monitor que a distro atende.

Duas caras do mesmo defeito, e nenhuma delas apontava para os limites:

- **A janela fechava sozinha.** `Alt+=` cresce a coluna em 10% da tela; num monitor de 1920 o
  primeiro passo já pede 2112 px. `Surface::configure` **não devolve erro** — ele devolve `()`, e a
  dimensão inválida é erro de validação fatal. O processo morre, a janela some, e não há uma linha
  de log em lugar nenhum, porque o niri manda a saída do que ele spawna para `/dev/null`.
- **Monitor grande nunca teve rodapé.** Ele ocupa a largura da tela: num 2560 ou num 4K ele morreria
  na primeira configuração, antes do primeiro pixel.

A correção é `using_resolution(adapter.limits())`: troca só as três dimensões de textura pelas do
adaptador e mantém todo o resto downlevel — compatibilidade ampla sem um teto de resolução
inventado. E `gpu::fits` prende a dimensão ao que a GPU aceita, porque a alternativa a cortar o
conteúdo é matar o processo.

O check (`large-monitor-surface`) pede um dispositivo com os limites **de produção** e tenta a
textura de um 4K. Comparar números de `Limits` seria conferir a nossa aritmética contra ela mesma;
quem responde ali é o driver.

---

### Config gerado que ninguém valida é config quebrado

Dois defeitos passaram pelo `nix flake check` inteiro e só apareceram na tela:

1. **`duplicate keybind`** — `Alt+Ctrl+L` dos padrões do niri contra o `Ctrl+Alt+L` de emergência: a
   mesma tecla escrita em outra ordem. A deduplicação comparava **strings**.
2. **Dois blocos `layout {}`** no config de produção, desde sempre. O niri recusa nó duplicado.

Um config recusado não é um atalho quebrado: é o compositor caindo no **config padrão dele** — sem
nenhum atalho nosso e sem a leader. E o único sinal era uma caixa vermelha na tela.

O que faltava era perguntar a quem tem a resposta. O `niri-binds.nix` grepava por uma palavra e o
módulo grepava por outra; nenhum dos dois perguntava ao compositor se o arquivo **inteiro** fecha.
Agora o `niri-config-valid` roda `niri validate` nos dois configs.

Para isso o config do aninhado teve de virar **derivação**. Ele era montado por `sed` dentro do
script, em tempo de execução — nada tinha como alcançá-lo. É o mesmo movimento que o config de
produção já tinha feito, e pelo mesmo motivo.

E a comparação de teclas passou a ser **canônica**: modificadores ordenados, tudo minúsculo. Os binds
de emergência entram como reservados, porque eles saem sem o prefixo da leader — um acorde que caia
na mesma tecla não "perde para o nosso", ele quebra o arquivo.

---

### A mensagem de erro tem de ser legível — e não era

O niri avisou que o config estava quebrado, e o aviso saiu em **tofu**: uma caixinha por caractere. É
o modo de falha mais cruel possível, porque apaga a informação de que se precisa exatamente quando se
precisa dela.

A causa: `FONTCONFIG_FILE` substitui a configuração **inteira** do fontconfig, e o `nix/dev.nix`
apontava para o **fragmento** — só as regras de alias, sem um `<dir>` sequer. Sem diretório de fonte,
o fontconfig não acha fonte nenhuma. Medido: `fc-list` devolvia zero.

O nosso renderizador não passa por ali — ele resolve por caminho absoluto —, e é por isso que o
rodapé aparecia normal enquanto o aviso do compositor era ilegível. Duas cadeias de fonte, e só uma
quebrada — e nenhum check olhava a segunda.

O arquivo completo é **autossuficiente**, e não um `<include>` da base do sistema: incluir o
`fonts.conf` do nixpkgs traz junto o `/etc/fonts/conf.d` do host, e as regras de lá vencem as nossas
— no teste, `fc-match sans-serif` devolvia "Liberation Sans". O aninhado existe para mostrar o que a
distro vai mostrar.

---

### Notificações

Não há popup de canto nem card. Isso não é restrição estética; determina o desenho inteiro.

**A notificação tem uma faixa própria, ACIMA da modeline.** É a superfície do rodapé crescendo para
cima, exatamente como a expansão já cresce para baixo — uma superfície só, sem popup e sem card
(invariantes 14 e 15). Ela ocupa **uma linha**, ou **até três** quando há corpo e ações.

O pedido original era uma *toast* flutuante, e ela é literalmente o que o invariante 14 nomeia. A
segunda leitura — pôr a notificação **na** modeline — cai por outro motivo: a modeline tem uma linha
só, e é a linha que diz *onde você está*. Uma notificação pousando ali tira da tela a única coisa que
nunca deveria se mexer. A faixa acima resolve as duas: como a superfície é ancorada embaixo, crescer
para cima **não move a modeline** — quem cede é o conteúdo.

**Uma de cada vez.** A faixa tem uma região de mensagem. Empilhar exigiria contêiner, e contêiner é
card. Ou a notificação está na faixa, ou está na fila — e o contador `2/5` aparece **só com mais de
uma**, porque `1/1` não diz nada e custa três colunas de uma linha que elide.

**Nada fica para sempre — nem a crítica.** Essa regra foi invertida por decisão do dono, e a razão é
a faixa: enquanto ela está lá, o conteúdo está uma a três linhas mais curto. Uma notificação que
nunca sai é um imposto permanente sobre toda janela. A crítica ganha `long` (20 s) e **continua não
lida**: sair da tela e ter sido vista são eventos diferentes, e o sino na modeline é o que lembra
depois — ele só existe enquanto há algo não lido. O que sobrou de "a especificação permite ao
cliente cravar timeout" é uma aproximação: o `expire_timeout` vira a mais próxima das três classes,
e só quando nenhum filtro opinou.

**O que CHEGOU e o que foi DECIDIDO são dois tipos.** `Notification` é dado — um aplicativo disse
uma frase num instante, e nada revisa isso. `Presentation` é política — os filtros da pessoa
decidem, ela muda de ideia depois, e a mesma notificação mostrada duas vezes pode aparecer diferente.
Num struct só, "o app pediu 5 s" e "nós damos 6" ficavam indistinguíveis, e é o segundo que o
gerenciador precisa editar.

**Tudo o que a distro levanta está em `assets/notifications.nix`**, e sai dali gerado: a tabela Rust,
**a lista de chaves de texto que os dois idiomas têm de ter** e a recusa de um campo inválido no
build. Escrito em Rust, "toda notificação da distro" seria um `match` num arquivo e um `format!`
noutro, e a resposta honesta a "quais existem?" seria um grep. O gerenciador desenha ESSA lista.

**Uma entrada é um reconhecimento, não um construtor.** Nem todas nascem aqui: a da captura de tela é
mandada pelo compositor, em inglês, com o caminho do arquivo no campo do ícone. Então a entrada diz
como RECONHECÊ-LA e o que fazer com ela, e a mesma máquina serve uma que levantamos e uma que chega
de fora. As nossas carregam uma `tag` igual ao próprio id, que é por que quase todo `when` é de uma
linha. O check `screenshot-notification-contract` lê a fonte do niri fixado: se o texto lá mudar, o
filtro pararia de casar **em silêncio**.

**O sino abre o gerenciador, e só existe com algo não lido.** Era um sino com um número, em toda
tela, para sempre — um `0` respondendo a uma pergunta que ninguém fez —, e o dono pediu o contrário:
nada em repouso, e o sino pontilhado sozinho quando há algo a ler. O NÚMERO foi junto: quantas é a
pergunta do gerenciador, e o ponteiro a responde no minibuffer sem abrir nada ("2 não lidas, nenhuma
filtrada"). Clicar abre, e os dois botões fazem o mesmo: há uma coisa só a fazer ali, e um segundo
sentido para o botão direito seria um menu que ninguém pediu.

**O filtro é `campo + teste`, e não regex.** Duas razões, nenhuma ascética: é dependência num crate
que o rodapé linka, e é armadilha num arquivo que o dono edita à mão — um regex que casa demais vira
"uma notificação que eu nunca vi", que é o defeito mais difícil de notar. Três testes de texto
(`is`, `contains`, `starts-with`) e dois numéricos cobrem o que uma pessoa escreve, e cada um está
obviamente certo ou obviamente errado à primeira vista.

**A PRIMEIRA regra que casa decide**, e o gerenciador mostra qual foi. Com toda regra que casa
sobrepondo a anterior, a resposta a "por que isto está silencioso?" é uma lista, e a pessoa teria de
simular o livro inteiro na cabeça. Uma regra, uma resposta, uma coisa para editar.

**A ordem é: a distro fala primeiro, do geral ao específico; depois você, na mesma direção.** Padrão
por urgência → entrada do catálogo → primeira regra (as suas antes das da distro) → seu ajuste à
REGRA → seu ajuste à ENTRADA. Os dois últimos passos já estiveram colados nos dois do meio, e estava
errado de um jeito que um teste pegou: ajustar a captura para ser chamativa não fazia nada, porque
uma regra da distro sobre nível baixo rodava depois e a devolvia ao discreto.

**Suprimida não aparece e não conta.** Um contador que soma o que a pessoa pediu para nunca ver é um
contador que não dá para zerar. Mas ela é REGISTRADA, e a coluna da direita do gerenciador é onde
ela aparece — um filtro que ninguém consegue auditar é um filtro em que ninguém confia.

**Duas portas, uma `notifications::View`.** `n n` abre no rodapé, `n w` numa janela; duas colunas
quando cabe, uma quando não. O corte sai de um token, e vale para as duas portas — que é o que faz
"uma implementação, duas portas" ser verdade também do layout.

**A janela não RODA ação, e diz isso.** Marcar lida, ajustar filtro e navegar são dela; executar uma
ação é do rodapé, que é o processo que sabe abrir num escopo próprio (seção 9.1) e que tem a área de
transferência. Uma segunda implementação ali é exatamente a segunda porta discordando da primeira —
o defeito que este arranjo existe para evitar. Quando se aperta a letra na janela, ela responde com a
frase que diz onde a ação roda, em vez de não fazer nada.

**Quinze dias em disco.** O histórico é estado — foi o que aconteceu, é escrito por máquina e é
grande — e mora em `$XDG_STATE_HOME`. As preferências são configuração e moram em
`$XDG_CONFIG_HOME`, ao lado da tabela de ícone por app id que já estava lá. O arquivo é reescrito
inteiro porque um registro MUDA (ser lido é mutação do que já foi escrito), e um log append-only
precisaria de um segundo fluxo de "e esta foi lida" mais um replay para reconstruir o estado.

##### E o arquivo é o BROADCAST, porque a janela é um segundo escritor

Reescrever o arquivo inteiro é certo com UM escritor, e há dois: a janela é um processo próprio e
marca lida e esquece escrevendo. O rodapé lia esse arquivo UMA vez, na partida, e escrevia o vetor
dele por cima a cada mudança — então um `d` dado na janela se desfazia no instante em que qualquer
notificação chegasse, e o `reread()` de 2 s da janela mostrava o registro de volta. Um ajuste feito
lá não alcançava a faixa até o rodapé reiniciar.

O conserto é o mesmo do `theme.json`, e é por isso que ele é o precedente citado: **as duas portas
vigiam os dois arquivos por inotify**. O que o rodapé faz ao ser acordado não é reler — é
RECONCILIAR por id (`Queue::took_from_disk`): o que o arquivo decidiu sobre um registro que este
processo tem é tomado, e o que este processo tem e o arquivo não tem é dado por esquecido. O que
está vivo aqui — a faixa, a fila, a ordem de chegada — não está no arquivo e não pode vir dele.

Duas regras seguram o laço. **O que chegou DEPOIS do arquivo sobrevive**, comparado pelo carimbo de
tempo: sem isso, a janela apagaria a notificação que acabou de chegar no rodapé. E **nada é escrito
de volta** ao reconciliar: escrever mexeria no inotify do outro processo, que leria, que escreveria —
dois processos respondendo ao evento um do outro para sempre.

**`vibrate` e `audio` existem e não fazem nada — e a tela diz isso, na linha de mensagem.** Eles
existem porque a API externa que o dono vai pôr no ar vai setá-los, e derrubá-los na porta faria a
mesma notificação significar coisas diferentes conforme por onde entrou. O que eles não podem ser é o
"interruptor que não interrompe" que este repositório já pagou duas vezes: cada um está em `INERT`
com uma chave de texto dizendo por quê.

A razão era uma **terceira coluna na linha do interruptor**, e o dono pediu para tirar: duas linhas
de sete carregando uma frase faziam a folha parecer uma lista de problemas. Ela foi para a linha de
mensagem, quando a linha está em foco — que é onde este rodapé já explica coisa, e é o mesmo lugar
em que os campos do gerenciador de arquivos se explicam. O que fica na linha é o valor ser `não`,
que é verdade. O check passou a cobrar as duas metades: a razão existir, e **haver quem a desenhe**
— senão "saiu da linha" e "saiu do sistema" seriam a mesma coisa para quem confere.

**O que não tem glifo próprio ganha um genérico**, e nunca nada. Uma lista em que algumas linhas
começam com uma marca e outras não se lê como uma lista furada, e a marca é a resposta mais rápida a
"quanto isso importa" — antes de o olho chegar à primeira palavra. É a SEVERIDADE, e não um quadrado
neutro, porque a severidade é a única coisa que o protocolo dá que diz alguma coisa: um estranho que
gritou ganha o desenho de quem grita.

**O controle de marcar como lida fica na BORDA DIREITA da faixa**, e é um envelope aberto. Ele
começou como um círculo vazio na margem esquerda, e as duas coisas estavam erradas: a margem
esquerda é onde a frase começa, então o controle empurrava as palavras e se lia como parte delas; e
um círculo é uma FORMA, não um verbo — diz "há um controle aqui" e deixa o que ele faz por adivinhar.
Um envelope aberto é o desenho que todo mundo já lê como "lida", e a borda direita é para onde a mão
vai quando quer dispensar algo.

**A coluna da esquerda para ANTES da direita.** Os dois alvos ficam no fim da linha, e com o painel
terminando exatamente na divisão eles encostavam na primeira célula da outra coluna — lido da tela
como lixeiras que pertenciam às notificações de lá. O vão é o `space.modeline.segmentGap`, que é o
número que este sistema já usa para "duas coisas numa linha são duas coisas".

E as duas extensões saem de **uma função só** (`extent_of`). O desenho calculava a sua e o ponteiro
calculava a dele, e os dois concordavam por sorte: uma lixeira desenhada numa célula e testada em
outra é um controle que não faz nada onde aparece e faz onde não aparece.

**Na lista, cada linha tem dois alvos à direita**: o mesmo envelope e uma lixeira. Aqui o mouse AGE,
e não só mira — ao contrário do painel de transferências, cuja regra é "as teclas agem, o mouse
mira". A diferença é que lá o clique é na LINHA, onde nada na tela diz qual de cinco coisas
destrutivas ele rodaria; um clique numa lixeira não é palpite. Cada um tem tecla (`Espaço` e `d`), e
o gerador **recusa** uma ação que peça essas letras — uma ação encoberta por tecla de vista é uma
ação que ninguém alcança, e o jeito de descobrir seria apertando e vendo outra coisa acontecer.

**Esquecer não é dispensar.** Dispensar tira da FILA e nunca do registro; esquecer tira do registro
para sempre. Quinze dias é muito tempo para conviver com uma linha que não se quer, e a alternativa
era esperar ela envelhecer. É o registro da própria pessoa.

**O tray é lista de PERMISSÃO**, não de bloqueio. Um tray que mostra tudo come a linha no dia em que
se instala mais um aplicativo, e ninguém ligaria os pontos. O único furo é o estado de atenção,
porque a alternativa — um app pedindo atenção que ninguém vê — é pior.

### Cores: o gerador e os temas — `LEADER s t`

**Nenhuma cor é escrita à mão.** `assets/tokens/colors.nix` é uma tabela de RECEITAS — cada token
nomeia um degrau de uma família (`text.primary = "neutral.text-strong"`, `surface.selection =
"accent.element"`), e uma receita pode levar `alpha` (a faixa do transporte) ou escalar o croma do
degrau (`surface.mesh`). Os valores saem de `nix/scripts/gen-theme.py`, que roda UMA vez em
`nix/themes.nix` para todos os temas de `assets/themes/themes.nix` — uma LISTA, como a dos idiomas: o
primeiro é o padrão, e nenhum outro sítio nomeia um tema. Um tema é um arquivo de SEMENTES
(`assets/themes/<id>.nix`): a luminosidade do fundo e do rodapé, o matiz e o tinte dos cinzas, o
matiz e o croma do texto, um multiplicador global, e por família um matiz, um croma relativo, um
`shift` de contraste, uma `torsion` de matiz nos degraus de pintura e um `paint-chroma`. Nada de hex:
o check `colours-are-computed` recusa um literal nas receitas e nas sementes, e recusa qualquer
`import` da referência congelada fora de `nix/themes.nix`.

**A escada é a paleta do owl feita regra.** Cada família tem catorze degraus com NOME (nunca número:
um identificador do WIT começa por letra): quatro de pintura que sobem do rodapé por separação de
luminosidade (`ground`, `element`, `border`, `border-strong` — o fundo do brain, a faixa de seleção,
a borda inativa), três sólidos resolvidos a um contraste APCA sobre o rodapé (`solid` Lc 52,
`solid-hover` 60, `vivid` 44 a croma pleno), seis de texto (Lc 11, 23, 48, 79, 87, 100 — o
separador, o dim, o secondary, o brain.countdown, o primary, o brain.text) e o `on-solid` (o fundo,
se lê a Lc 45 sobre o sólido; senão o texto mais forte). A neutra tem as duas âncoras fixas
(`ground`, `chrome`) e os cinzas do owl byte a byte. Cada linha da tabela em `gen-theme.py` diz de
que valor de mão foi lida, e qual nasceu de nenhuma tela. O croma é RELATIVO — uma fração do máximo
que o sRGB mostra naquela luminosidade e naquele matiz, resolvido pela fronteira analítica de
`colorspace.py` (a bisseção erra perto do primário azul, onde um raio sai, entra e sai de novo) —, e
a luminosidade de um degrau APCA é bissecada rodando o pipeline INTEIRO, quantização a 8 bits
inclusive, porque o contraste que importa é o dos bytes que chegam à tela. A matemática é a do
`noctua-colors` do dono, portada em Python de biblioteca-padrão: Oklab com as matrizes publicadas
CONDICIONADAS (branco é L = 1 exato, cinza tem croma zero exato), APCA-W3 0.1.9 assinado, ΔE_ok,
Brettel para o relatório. Prova-se em `colorspace-tests`, e o owl rederivado fica a ≤ 5/255 por canal
do que estava na tela.

**As relações que a prosa prometia viraram GATES.** Contraste de cada texto sobre cada fundo em que
ele é desenhado (o `text.dim` sobre a faixa é gate porque o gerenciador de notificações e a janela
de atualização o desenham lá sem erguer; só o gerenciador de arquivos ergue), distância ΔE_ok entre
os cinzas da escada e entre a faixa de seleção e cada cinza que a ladeia (o par que o dono mediu),
degraus adjacentes de toda escada (≥ 0,012, L monótona), matiz entre as cinco famílias do rodapé
(≥ 14°), acento fora da faixa 158–172° (colapsa sobre o vermelho na protanopia), pares categóricos e
ordinais, e as quatro cores-filme do `shots.nix`. Deficiência de visão de cor é Warn e nunca Fail —
verde e amarelo do próprio owl estão a 0,009 sob deuteranopia, e o glifo carrega o significado. A
regressão compara cada chave gerada com `assets/themes/owl.reference.nix`, os 52 literais congelados
do dia das sementes: acima de ΔE_ok 0,03 é falha de build, salvo chave listada em `deviations` com
teto e motivo próprios (`text.subtle.faint`: o `text-shift` do registro foi medido no `strong` e vale
para os três degraus). `report.md` de `nix build .#themes` traz cada margem; `theme-references-hold`
e `themes-negative` (uma semente com o acento a 165° TEM de falhar) são os checks.

**Uma cor é um PAPEL, e o tema é do processo.** `ronoctua_tokens::Color` é um `u16` — o índice de um
papel — e `to_rgba8()` lê `themes::TABLE[tema][papel]`, uma tabela gerada com TODOS os temas. Os 280
sítios que nomeiam `color::TEXT_DIM` não mudaram: nomeiam o papel, e o papel resolve ao desenhar.
Igualdade e hash são por papel (`lift_onto_a_band` pergunta "este campo é o dim?", e é isso que
sempre quis perguntar). O tema é um `AtomicUsize` (`theme::set`), como o idioma é uma propriedade do
processo: enfiar um valor por 280 assinaturas carregaria algo que só o desenho lê. Buffers wasip2 não
souberam de nada — cruzam NOMES (inv. 11), e o host resolve; o enum do WIT cresceu para 147 nomes e
recompilaram uma vez. `to_rgba8` deixou de ser `const`; os três `const fn colour()` das TUIs viraram
`fn`. Cada processo escolhe no arranque (`theme::resolve_at_start`, em `main.rs`, antes de qualquer
porta): `$XDG_CONFIG_HOME/ronoctua/theme.json` > `RONOCTUA_THEME` (a opção `ronoctua.theme`) > o
primeiro. Fonte que nomeia tema inexistente ou não parseia cai para a seguinte e diz no log.
`render-png --theme` é determinístico (o primeiro, nunca arquivo nem ambiente); o instalador e o
resgate leem só o ambiente.

**O arquivo é o broadcast.** O socket de controle só entra no rodapé, e as portas — janela de
arquivos, notificações, atualização, cada host de buffer — são processos. Então o rodapé não avisa
ninguém: grava `theme.json` atomicamente (`ronoctua-xdg`, o quarto chamador que o comentário do
brain pedia, e que agora serve os três anteriores) e toda porta segura um `Notice::on_file` sobre
ele — o mesmo inotify do gerenciador, filtrado por UM nome, porque o `files-sidebar.json` mora ao
lado. Ao tocar, `theme::reload()` e repintura. O que uma CRIANÇA pintou com um valor — a forma de
onda do ffmpeg, o `currentColor` dos SVGs — se refaz porque o tema entrou na CHAVE da prévia e do
SVG rasterizado: chave nova, cache do GPU sem ela, upload de novo; sem isso a tinta velha ficava
na textura. Custo: o diretório `~/.config/ronoctua` passa a existir no primeiro arranque de qualquer
porta (o inotify quer um diretório); e uma troca com um filme na prévia respawna o ffmpeg, com uma
peça 3D relê o arquivo uma vez — chave nova é pedido novo, e o painel fica vazio até a resposta, como
sempre ficou.

**`LEADER s t` é a lista da rede com uma diferença: mover o cursor PRÉ-VISUALIZA.** O tema sob o
cursor vira o do rodapé na hora — o rodapé é a amostra, porque amostra "na cor do tema dele" não
existe: uma `Color` resolve pelo tema corrente e um valor preso a outro tema é justamente o que o
crate recusa guardar. A prévia é só do rodapé; as outras portas seguem o ARQUIVO, que só `RET`
(`theme-apply`) grava, senão toda janela piscaria pelos temas enquanto a pessoa olha. `Esc` volta ao
gravado; um `theme.json` escrito por fora durante a prévia vence a prévia. Linhas: SÓ o id, na coluna
da tecla (identificador, como a lista do `M-x` mostra ids de comando; o nome traduzido era uma segunda
grafia da mesma coisa e saiu a pedido do dono), visto no gravado, foco no acento; sem faixa (é do
gerenciador). Sem `theme-next`: o `check-keymap` recusa comando sem tecla.

**E uma segunda coluna, das escolhas SOBRE o tema** — hoje uma: o fundo que o compositor pinta onde não
há janela, preto ou a cor do tema. A cor é `surface.backdrop`, o único degrau neutro ABAIXO do chrome
(um passo de janela abaixo, espelhado: janela um passo acima, fundo um passo abaixo), no matiz e croma
relativo que a semente `backdrop` nomeia — a única superfície FEITA para ler como a cor do tema, com
gate de distância contra preto, chrome e janela (owl `#040C24`, forest `#031206`). Duas listas lado a
lado na mesma região: `<left>`/`<right>` cruzam, `<up>`/`<down>` andam na que tem o foco, o cabeçalho
da opção não recebe cursor, e a linha da direita fica na coluna do texto mesmo quando a da esquerda
acabou (`ExpansionRow::aside`). A prévia do fundo não pode ser do rodapé, que nunca pinta o fundo: é
do niri — SÓ o fragmento do niri é escrito ao mover o cursor (`theme::show_to_compositor`, com o tema
sob o outro cursor; o do Ghostty não, senão um terminal aberto durante a prévia vestiria o que ninguém
aplicou), e no `Esc` ele volta ao que o arquivo diz — ou SOME, se não há arquivo: máquina que nunca
escolheu não tem fragmento, e `sync_with_file` mantém isso no arranque e quando o arquivo some.
`RET` grava o PAR (`{ "theme", "backdrop" }`; arquivo sem o campo é preto, que é o que toda máquina
tinha). Preto NÃO é token: é o nome da opção, e o fragmento o escreve como `#000000`.

**Fora do shell, três consumidores seguem, e cada um até onde pode.** O niri: a borda estática vem do
tema padrão (substituída no build do config, sem IFD), e no FIM do config há `include
"/home/<usuário>/.config/ronoctua/niri-theme.kdl" optional=true` — o gerador escreve por tema DOIS
fragmentos, um por fundo (`niri-black.kdl`, `niri-backdrop.kdl`), cada um com a borda num `window-rule {
border { … } }` (nó repetível, o último vence) e o fundo num `layout { background-color }`: o niri
recusa dois `layout` DENTRO de um arquivo e FUNDE o de um arquivo incluído, campo a campo, o valor
posterior vencendo (`niri-config`, `MergeWith`; `themes-have-fragments` valida cada fragmento sob um
config que já tem `layout`). O rodapé COPIA o do par escolhido ao gravar — copiar É escolher — e o niri
v26.04 vigia arquivos incluídos e recarrega. O `background-color` estático é o `surface.base` do tema
padrão, o que vale antes de qualquer escolha. O Ghostty: `config-file` com a paleta gerada do tema
padrão (ANSI 16, fundo, cursor, seleção — `terminal.*`, mapeados das famílias) e depois
`config-file = ?~/.config/ronoctua/ghostty-theme.conf`, opcional, que o rodapé também copia; processo
novo do Ghostty veste, processo aberto não — e toda janela É um processo, por qualquer porta: a
config diz `gtk-single-instance = false`, mas a entrada `.desktop` do upstream passava
`--gtk-single-instance=true` no `Exec` (flag vence arquivo) e oferecia ativação por D-Bus, que é
de instância única por construção; o módulo instala a entrada reescrita sem os dois e sem o arquivo
de serviço (`ghosttyPerWindow`, um `symlinkJoin`), e `ghostty-windows-are-processes` mede o instalado.
Pelo lançador, uma janela aberta depois da troca entrava no processo antigo e não vestia nada.
**E o Ghostty do nixpkgs 26.05 só lê
`$XDG_CONFIG_HOME/ghostty/config`** — nunca `/etc/xdg` —, então o módulo do terminal passou a pôr o
arquivo lá pelo home-manager, que já cuida dos dotfiles desse usuário: até isso, nenhuma linha dele
chegava ao programa, decoração inclusive (a revisão mediu com `+show-config`, e o check
`ghostty-config-includes-theme` agora pergunta ao binário a configuração EFETIVA). Os fragmentos têm UM renderizador (o gerador); o shell só copia, do
`RONOCTUA_THEMES`, e os refaz ao arrancar e quando o arquivo muda por fora. GTK e Qt vestem o MODO
(escuro), não a cor (`nix/modules/foreign.nix`); a extensão do navegador veste o tema padrão do build.
O que NÃO segue ao vivo, e por quê: a extensão (não enxerga nossos arquivos) e terminais já abertos
(cada janela é um processo que leu o config). O fundo do compositor SEGUE: o `layout` do fragmento é
fundido no reload.

**O que está lá para contextos que ainda não existem.** Além dos 52 tokens de sempre, cada tema traz
`context.<família>.{solid,hover,ground,border,on,text}` para as oito famílias do rodapé e
`context.neutral.{surface,surface-raised,fg,fg-muted,border,border-strong}` (as tabelas de forma da
referência, sem o que o inv. 15 proíbe), `terminal.*`, `chart.one..eight` (matizes a arcos de ΔE
iguais a partir do acento, luminosidade subindo com o índice — é o que sobrevive à dicromacia) e
`level.zero..ten` (sequencial no matiz do acento, croma mudo nas pontas). Uma tela nova pede o papel
que existe; a família que ela quiser criar é uma linha na semente e passa pelos mesmos gates.

---

## 4. Teclado, keymap e a leader

### Um keymap, e um bind só no compositor

Fechar janela, trocar de foco, mudar de espaço, capturar tela — tudo isso era bind do niri. Agora é
comando do shell, e chega ao compositor por ação de IPC. O niri tem **um** bind: o que acorda o
daemon.

**Um keymap, e não dois.** Com dois havia duas notações, dois lugares onde uma tecla podia colidir, e
a lista de atalhos conhecia metade do sistema — quem apertasse a leader via uma lista que não era a
verdade.

O risco novo é de outra natureza: o nome da ação viaja como **string**. `MoveColumnLeft` errado por
uma letra compila, roda, e o niri responde `"error parsing request"` — que vira um `warn` no log e
nada na tela. É a mesma classe do fallback silencioso, e o `match` do Rust não protege: ele confere o
id do comando, não o nome do outro lado.

Por isso a tabela vive em **um** lugar (`crates/ronoctua-shell/src/actions.rs`), e o binário
`ronoctua-niri-actions` a percorre inteira contra um niri de verdade dentro do `niri-contract`. Ele já
pagou por si: `Screenshot` e `MoveWindowToWorkspaceDown` exigem argumento, e `{}` dá o **mesmo erro**
de um nome inexistente. Numa sonda à mão a resposta se perdeu num timeout de socket e eu concluí que
passava; o check não deixou.

#### A roda: um evento, dois significados, e três jeitos de o Wayland dizê-lo

A região expandida tem dois modos (ver "Duas navegações"), e a roda respeita o modo em vez de
inventar um terceiro: onde há **barra de rolagem** o entalhe anda três linhas — o que todo
visualizador de texto faz —, e onde há **contador de página** ele vira a página. Quem decide é
`navigation()`, e não a roda; a roda só diz quanto e para que lado.

O Wayland tem três jeitos de dizer a mesma coisa, e um cliente que lê só um funciona em metade dos
aparelhos: `value120` (o de hoje, em 120-avos de entalhe), `discrete` (o antigo, um inteiro por
entalhe) e `absolute` (o contínuo, em pixels de dedo — o único que o touchpad manda).

O contínuo se converte pela **altura da linha**, e não por um número de pixels escolhido a dedo:
assim o dedo anda o conteúdo na mesma proporção em que anda na tela, e a conta se recalibra sozinha
se a fonte mudar. Um "15 px por entalhe" seria mais um valor visual fora dos tokens (invariante 9).

E o acumulador não é zelo: cada evento de touchpad vale uma fração de entalhe, e truncar evento a
evento daria zero para sempre — o touchpad simplesmente não rolaria. Guardando o resto, os pedaços
somam. O `stop` de um gesto zera o que sobrou, senão o resto de um gesto entraria no começo do
próximo.

#### O clique no buffer chega como CÉLULA, e o buffer resolve o resto

O invariante 11 não é sobre unidade, é sobre fronteira: pixel é coisa de Wayland e de escala de tela,
e o buffer não conhece nenhuma das duas. O host converte pixel em célula (descontando a margem da
janela) e manda `input::click(posicao)` — exatamente como ele já converte tecla em `character`.

Quem resolve **qual item** está naquela célula é o buffer, porque só ele conhece o próprio layout: a
grade de glifos começa na linha 2, anda de duas em duas linhas e tem uma linha de nome embaixo. O
host mandar "o item número N" seria o host reimplementando o layout do buffer, e as duas contas
divergiriam no dia em que o cabeçalho ganhasse uma linha. Por isso o teste também não repete a
geometria: ele clica **onde o `render` disse que pôs o glifo**.

Uma diferença deliberada em relação ao rodapé: lá o clique numa linha **executa**, porque cada linha
é uma ação e clicar nela é apertar a tecla dela. No navegador de glifos o clique **seleciona**, e o
`Enter` continua sendo quem copia — ali a lista é dado que se navega, e copiar no clique trocaria a
área de transferência a cada glifo olhado.

#### O modificador que abriu a leader não conta na sequência

No aninhado a leader **é** `Ctrl+Alt+Espaço`. Quando a sequência começa, os dois modificadores estão
pressionados por construção — a mão acabou de apertá-los. Cada tecla sairia com eles junto, nada
casaria, e a tela diria `CTRL ALT SPC M-n is not bound`. Exigir que se solte o Alt antes de digitar
`i` é uma armadilha, e não há nada na tela que a ensine.

O mesmo estado acontece **sem** a mão, e aí é pior porque não há gesto que o desfaça: sair para um
atalho do host (`Super+Espaço`, no Omarchy) e voltar faz o niri aninhado perder a soltura da tecla.
A partir daí ele anuncia o modificador para sempre, toda ligação passa a não casar, e o sintoma é
"os atalhos pararam de funcionar" sem nada mudar no keymap.

A regra é: **desconta-se o que já estava segurado quando o rodapé ganhou o teclado**
(`Footer::discount`, sobre o campo `inheritance`). A herança é tirada no primeiro evento de modificador
depois do `enter` — que é quando o compositor conta o que está valendo. Um modificador apertado
**depois**, no meio da sequência, continua contando: é o que preserva a possibilidade de uma ligação
`C-…` existir um dia sem nascer quebrada.

E `leave()` zera os modificadores junto com o `CtrlRight`, pelo mesmo motivo e com mais razão: um
estado que só se atualiza com foco não pode continuar valendo sem foco.

O teste de VM segura o Alt com `xdotool keydown` durante a sequência inteira — o `xdotool key` solta
a tecla entre uma e outra, e por isso o caminho passava verde com o defeito presente.

#### A caixa é Shift, e nunca Caps Lock

O keymap distingue caixa de propósito: `w h` foca a janela da esquerda, `w H` a **move** para lá.
Essa distinção é **Shift** — um gesto a mais, deliberado, na hora.

O Caps Lock não é isso. Ele é um estado que fica ligado, que se liga sem querer, e que a pessoa
esqueceu que ligou. Com ele ligado o xkb entrega `I` onde se digitou `i`, e **toda** ligação
minúscula do keymap morre de uma vez. A tela mostra `CTRL ALT SPC I is not bound`: verdade, e inútil —
ela nomeia a consequência e esconde a causa. Pior nas ligações que existem nas duas caixas, onde
`w h` viraria `w H` e a janela se **moveria** quando se pediu para focar. Um atalho que faz outra
coisa é pior do que um que não faz nada.

Por isso o Lock é desfeito antes de a tecla virar `KeyPress` (`Footer::without_caps_lock`), nos dois
sentidos — com Shift junto o xkb já devolveu a minúscula, e desfazer só num sentido apagaria o
`w H`. O Shift continua decidindo a caixa, que é o desenho.

E quando a caixa **é** o motivo de nada casar, a mensagem passa a dizer isso: `leader::other_case`
procura a mesma sequência com a caixa trocada e, se for ela que existe, a área de eco mostra
`… não está ligado — … está`. Isso **não** é fallback: nada é executado pela outra caixa, justamente
porque `w H` e `w h` fazem coisas diferentes. É só a diferença entre a pessoa descobrir sozinha e ter
de mandar um screenshot.

#### A leader é a tecla Super sozinha, e isso custou um daemon

Em produção, **tocar** Super abre a sequência; **segurar** Super é o modificador dos acordes. A mesma
tecla física faz as duas coisas, e nenhuma delas é possível só com o compositor: o niri recusa bind em
modificador sozinho (`invalid key: Mod`) e não tem disparo na soltura — as propriedades de bind são
`allow-when-locked`, `allow-inhibiting`, `cooldown-ms`, `repeat` e `hotkey-overlay-title`.

Isso não é limitação a contornar. Um modificador **é** um modificador; distinguir toque de pressão
acontece abaixo do compositor, no fluxo de eventos do kernel. Quem faz isso é o **keyd**
(`nix/modules/keyboard.nix`): `leftmeta = overload(meta, f13)` — segurada, ela é Meta e os acordes
chegam ao niri como `Mod+…`; solta sem outra tecla, emite `F13`, que o niri liga à leader.

**É a peça mais perigosa do sistema por linha de código**, e vale dizer por quê: o keyd fica entre o
teclado e tudo o mais, e um erro nele é um teclado que não digita. O que limita o estrago é remapear
**uma** tecla — o teste de VM afirma que a seção `[main]` tem exatamente uma linha — e o serviço não
bloquear o boot: se ele não subir, perde-se a leader, não o teclado.

Uma armadilha já paga: o keyd avisa `failed to set effective group to keyd`, e o conserto óbvio
(`users.groups.keyd = { }`) **derruba o serviço** — a unidade do NixOS roda sem `CAP_SETGID`, então
com o grupo existindo o `setgid` falha e o processo sai com 255. O aviso é o caminho previsto.

No modo aninhado nada disso existe: a leader é `Ctrl+Alt+Espaço`. O `Mod` pertence ao compositor do
host, e pôr um remapeador global no caminho para desenvolver seria trocar o teclado da máquina inteira
por causa de um loop de teste.

**E por isso o aninhado não consegue ser inerte ao segurar.** O niri dispara bind na *descida* da
tecla: apertar Espaço com Alt segurado **é** o gatilho. A inércia em produção não vem do compositor,
vem do keyd. O que dá para consertar — e está consertado — é o `repeat=false`: sem ele, segurar
a leader re-dispara o `spawn` na taxa de repetição do teclado e a lista reabre sozinha várias
vezes. No aninhado, o acorde é o **`Alt` sozinho**, e a tela passou a dizer isso
(`RONOCTUA_LEADER_CHORD`). Para exercitar o comportamento de produção, o caminho é `nix run .#vm`,
onde o keyd existe.

**A rede de segurança, e por que ela não pode depender da leader.** Se o keyd cair, tocar Super não
emite mais `F13` e some o caminho para *todo comando por nome*. Dois binds vivem **fora** da leader
por isso: `Ctrl+Alt+L` abre a sequência sem passar pelo `F13`, e `Ctrl+Shift+L` roda
`keyboard-restore`, que reinicia o serviço e **diz na tela** se conseguiu — restaurar em silêncio é
indistinguível de não restaurar. Reiniciar unidade do sistema de dentro da sessão precisa de polkit;
a regra libera **um** verbo, **uma** unidade e **um** usuário, porque um `manage-units` aberto ao
`wheel` deixaria qualquer processo da sessão parar qualquer serviço. Os dois binds são gerados da
mesma tabela dos acordes, e o gerador **recusa** que eles saiam com o prefixo da leader: seria a
chave trancada dentro do cofre.

**O rodapé não subia em produção.** O binário não estava em `systemPackages` e o bind da leader
spawnava o *nome* `ronoctua-shell`, que o `PATH` de uma unidade não resolve. O sistema instalado
ficava sem modeline, sem minibuffer e sem leader, e nenhum check acusava — nenhum deles subia a
sessão de verdade. Agora o rodapé é unidade de usuário do `graphical-session.target`, todo `spawn` do
config carrega caminho do store, e o `keyboard-vm` afirma as duas coisas.

**Segurar a leader é inerte.** O gatilho da sequência é `LEADER+Space`, um acorde como os outros —
não a leader sozinha. Segurá-la não abre a lista e não tira o foco da janela, e há uma guarda no
build que recusa um bind na leader sozinha: seria o único jeito de esse comportamento voltar.

`LEADER w h` é **sequência**: a leader é apertada e solta, o compositor avisa o daemon, e o rodapé
passa a receber teclado. `LEADER+←` é **acorde**: a leader fica segurada, e o rodapé nunca vê essa
tecla — quando ele ganha o teclado, o acorde já terminou. Quem resolve acorde é o compositor, sempre.

Isso poderia trazer de volta as duas tabelas. Não traz, porque **o bind do niri é gerado**: a tabela
de acordes vive em `actions.rs`, ao lado do que cada comando faz, e o `export-niri-binds` emite o KDL
a partir dela. Some um id da tabela, some o bind. É o mesmo padrão dos tokens, do i18n e do WIT —
gerar, em vez de manter duas cópias sincronizadas à mão.

O nome da ação existe em duas grafias — `FocusColumnLeft` no IPC, `focus-column-left` no config — e a
segunda é **derivada** da primeira, para não haver uma terceira tabela. Um erro aí não é silencioso: o
niri recusa o config na partida.

**Os acordes saíram da lista do toque.** Eles apareciam ali em tom apagado, e não eram escolhas do
que se está fazendo: a leader já está solta, e apertar `←` naquele momento não faz nada. Vinte e
cinco linhas para dizer o que existe em *outro* modo de uso — a lista encolheu pela metade quando
saíram. O que saiu foi para o **manual** (`LEADER h s`), que ocupa a mesma região expandida e é
gerado da tabela: mudar um atalho muda o manual, sem passo intermediário. Um manual escrito à mão
seria a segunda cópia da tabela de atalhos, e o dia em que ela mente é o dia em que alguém confia
nela.

O `+` que os prefixava virou a marca de **submenu**, e só dela.

O config do niri passou a ser uma **derivação**, e não uma string. Interpolar a saída de uma derivação
numa string de avaliação seria import-from-derivation: a avaliação do flake pararia para esperar um
build.

**`Esc` não mata buffer.** Ele cancela o que está em curso — a busca — e para aí. Destruir trabalho é
comando nomeado (`LEADER w d`), porque não pode depender de uma tecla que se aperta por reflexo ao sair
de qualquer coisa.

#### As 141 ações do compositor viram comandos

**Toda funcionalidade é um comando nomeado**, e do sistema faz parte o que o compositor sabe fazer.
As 141 ações do niri entram no registro como `niri-<nome>`, alcançáveis pelo localizador
(`LEADER :`).

**A lista é gerada do binário pinado.** `niri msg action --help` dá nome, descrição e — por
subcomando — se a ação exige argumento. Escrita à mão, ela envelheceria em silêncio: uma ação
removida vira comando que responde `"error parsing request"`, que é um `warn` no log e nada na tela.
O gerador **para o build** se extrair menos de 100 — um parser que degrada em silêncio devolveria
tabela vazia, e as ações simplesmente sumiriam da lista.

**Elas executam pelo cliente do próprio niri**, e não pelo nosso JSON. Não é preguiça: `Screenshot`
exige `show_pointer` e `MoveWindowToWorkspaceDown` exige `focus`, com padrões que existem no clap do
niri e em lugar nenhum que a gente possa ler — mandar `{}` dá o **mesmo erro** de um nome
inexistente. Com `niri msg action <nome>`, os padrões são os dele, o erro é o dele (e vai para a área
de eco, não para o log), e não há conversão nossa para errar. Os ~25 comandos do keymap continuam no
caminho direto por IPC, que é o quente e é provado ação por ação pelo `niri-contract`.

**Elas não ganham tecla**, e é a única exceção à regra de que todo comando tem atalho: 141 atalhos
seriam 141 teclas a menos para o que se usa. A exceção é reconhecível pelo prefixo `niri-`, então ela
não cresce sem alguém escrevê-lo.

**Ação com argumento pergunta.** Escolhida no localizador, `focus-workspace` vira um prompt
(`focus-workspace: `) e o que se digita vai como argumento. Uma sequência de teclas não tem como
carregar um número — ela é um caminho até um id, não uma chamada com parâmetro.

**As 141 descrições estão nos dois idiomas**, e um check recusa ação sem chave. Subir a tag do niri
**para o build** até o que apareceu ser traduzido. É barulhento de propósito: a alternativa é o
inglês do upstream aparecendo no meio de uma lista em português, que é o fallback silencioso com
outra roupa.

---

### A primeira tecla de um keymap de buffer é um ACORDE

O descritor de um buffer declara ligações — o editor declara `m s` para salvar. O host do buffer as
resolve **no processo dele**, porque é ele que segura o teclado enquanto tem o foco (invariante 4:
um buffer é janela própria).

A primeira versão deixava qualquer letra abrir uma sequência, e o resultado foi pior do que a
funcionalidade faltando: **o editor engolia todo `m` digitado no arquivo.** `advance_sequence`
segurava a letra esperando a próxima; se a próxima não completasse `m s`, as duas eram descartadas —
de propósito, porque reinjetá-las digitaria um `m` no arquivo um toque depois de a pessoa ter
seguido em frente. O efeito prático: nenhuma palavra com `m` podia ser escrita.

A regra é uma linha, e está isolada em `opens_sequence` para poder ser testada sem compositor:

```rust
const fn opens_sequence(ctrl: bool, logo: bool, idle: bool) -> bool {
    ctrl && !logo && idle
}
```

**Um acorde não é texto; uma letra é.** A primeira tecla vem com `Ctrl` — `Ctrl+m`, depois `s` — e
da segunda em diante são letras simples, como em `SPC m s`. `logo` fica de fora e não é ignorado:
`Super` é do compositor, e um buffer que respondesse a ele roubaria uma ligação do niri.

O que sobra é o which-key não listar essas ligações sob `SPC m`. O obstáculo de tipo saiu —
`ronoctua_keymap::Entry` passou a guardar `String` justamente por isso —, mas o que ficou é maior e
só apareceu depois: **o rodapé não tem como saber o descritor de um buffer em execução.** Ele conhece
as janelas por IPC do niri (id, título, `app_id`) e nada mais.

---

### Workspaces: `TAB`, nove fixos, e o número na tecla

O grupo saiu de `e` para **`TAB`** — a tecla do alternador de janelas de todo sistema —, e `TAB TAB`
mostra o panorama. Cada ação tem várias teclas de propósito: `j`/`k` do vi, `[`/`]` de quem vem do
navegador, setas para quem não decorou nenhum dos dois, e `.`/`,`. Redundância aqui custa uma linha
de tabela e poupa uma decisão por uso.

**São nove espaços declarados no config**, e não os dinâmicos do niri. Ele cria e destrói espaço
conforme o uso: `LEADER TAB 7` num sistema com três espaços não teria para onde ir, e a mesma tecla
levaria a lugares diferentes conforme quantas janelas estivessem abertas. Declarados, o número na
tecla **é** o número do espaço. O `0` vai para o último, que é a convenção do teclado — o zero fecha
a fileira depois do nove.

`TAB m` repete o submenu inteiro para **levar a janela**: quem aprendeu `TAB 3` ganha `TAB m 3` de
graça.

**E a ligação passou a carregar argumento.** `TAB 3` e `TAB 7` são o mesmo comando com números
diferentes; sem isso, a alternativa seria `workspace-ir-1`…`-9` — nove nomes no localizador dizendo a
mesma coisa e nove chaves de texto para traduzir, só para carregar um dígito. O acorde já fazia isso
(`LEADER+3` leva o `3` no KDL); agora a sequência também. Os dois vão pelo cliente do compositor,
porque a forma do argumento no JSON é dele.

---

### O teclado: layout num lugar, e o Caps Lock como Compose

O layout vive em `nix/keyboard-xkb.nix` e é lido pelo config de produção, pelo aninhado e pelo console
do TTY. Escrito nos três, um dia divergiriam — e o sintoma seria a mesma tecla dando caractere
diferente conforme onde se digita.

Hoje ele é `gb`, porque o teclado em uso é britânico. É decisão temporária e o instalador vai
perguntar isso um dia; até lá, muda em um lugar.

**O Caps Lock é a tecla Compose.** O teclado britânico não tem tecla de acento, e o `gb` puro só
alcança `^` e `` ` `` mortos — falta o agudo, o til e o cedilha, que é metade do português. Com
`compose:caps`, `CAPS ' a` dá `á`, `CAPS ~ a` dá `ã`, `CAPS , c` dá `ç`: uma regra só para toda
acentuação, e **nada muda na digitação normal**.

Era o AltGr (`compose:ralt`), e a troca é decisão do dono. Também é a melhor tecla pelo mérito: o Caps
Lock é a maior tecla da fileira de descanso, sob o mindinho esquerdo, e o que ele faz por padrão —
gritar — ninguém pediu. O AltGr volta a ser AltGr, e com ele os símbolos de nível 3 (`€`, `½`, `æ`)
voltam a ser um toque em vez de uma sequência de Compose.

**Compose não é modificador**, e a diferença aparece mais aqui do que aparecia no AltGr: não se
segura. Aperta e solta o Caps, depois `'`, depois `a`. É também por isso que ele não colide com a
leader nem com acorde nenhum — tecla que não levanta modificador não fica presa embaixo de um.

**E o Caps Lock não deixou de existir**: `shift:both_capslock` põe os dois Shifts juntos como o
interruptor dele. Não custa nada — ninguém aperta os dois sem querer — e mantém a função ao alcance de
quem escreve uma sigla.

As alternativas foram descartadas com motivo: `gb(intl)` e `gb(extd)` põem acento **morto no nível
1**, e a tecla `` ` `` deixa de escrever crase até você apertar espaço — num teclado que digita código
o dia inteiro, isso é pior do que o problema que resolve. E `lv3:caps_switch` faria do Caps um
seletor de nível 3 em vez de Compose: dá `ê` e `è`, e não dá `á`, `ã` nem `ç`.

**A resposta do instalador é `compose`, e não `options`.** A sétima pergunta escolhe a tecla de
acento; o que o xkb recebe é ela mais o que o sistema acrescenta de qualquer jeito. Eram um campo só,
e então responder sobre acento levava junto, calado, o interruptor do Caps Lock — resposta a uma
pergunta tirando algo que ninguém perguntou.

Isso funciona **dentro dos nossos buffers também**: o SCTK monta uma `xkb::compose::State` e o texto
que chega ao rodapé e ao terminal já vem composto. Não é suposição — está no `init_compose` da versão
que o `Cargo.lock` fixa.

E há um check para o nome: **o xkb não recusa um layout que não conhece**, ele cai no `us` e segue. O
sintoma seria o teclado inteiro digitando outra coisa sem uma linha de erro — a mesma classe do
fallback silencioso, e a mais cara delas, porque o jeito de investigar é digitando. O
`keyboard-layout-exists` confere layout, variante e **cada** opção contra o registro do
`xkeyboard-config` que a sessão vai usar, e o keymap do console contra o banco do `kbd`, que é outro.

E existir não basta: **opção que o xkb não conhece ele IGNORA, com status zero** — medido, com
`compose:naoexiste` compilando limpo e só uma linha em `stderr`. Por isso o
`keyboard-options-do-what-they-say` compila o keymap de verdade e lê os símbolos de volta: exige
`Multi_key` em alguma tecla e `Caps_Lock` no segundo nível dos dois Shifts. Sem o primeiro não há
acento; sem o segundo o Caps Lock deixou de existir e ninguém foi avisado. Provado reprovando nos
dois. E como o instalador carrega os padrões em constantes de Rust — ele roda num TTY do ISO, sem
flake para avaliar —, o `installer-defaults-match-the-keyboard` compara as duas cópias: divergindo,
a máquina instalada apertando Enter e a construída por `nix build` digitariam diferente.

---

### O id do comando é kebab puro, e a chave sai dele

O id era `dominio.acao` e a chave de texto era `cmd-dominio-acao`: a mesma informação em duas
grafias, com nada ligando uma à outra. Uma linha copiada com o id trocado e a chave esquecida
compilava — as duas constantes existem — e a lista mostrava o rótulo do comando vizinho.

Com o id em kebab (`window-left`), a chave é o id com prefixo, e o registro **confere**. O
`Registro` recusa id com ponto, recusa chave que não seja a do próprio comando, e a ligação do keymap
não carrega mais rótulo: ele sai do registro, pelo id. Uma informação, um lugar.

---

### Invariante 8: por que ele não depende de disciplina

"Zero string literal em UI" é uma regra que ninguém cumpre por vontade. O que a faz valer são duas
guardas, nenhuma delas um teste:

1. **Build.** `gen-i18n.py` exige o mesmo conjunto de chaves nos dois `.ftl`, **nos dois sentidos**
   (tradução faltando e tradução órfã são os dois erro) e recusa valor vazio.
2. **Compilação.** O mesmo gerador emite uma constante por chave. `key::CMD_ARQUIVO_ABRIR` não tem
   como estar errada; uma chave inventada não compila.

**O vão entre as duas, que já custou caro.** A guarda 1 vê o que *está* nos arquivos, nunca o que o
código *pede*. A guarda 2 protege quem escreve `key::X`; quem escreve `"x"` compila igual. E
`Texts::t` devolve `[x]` para chave desconhecida, de propósito, para não derrubar o shell.
Junte os três e há um caminho sem nenhum alarme — foi assim que o instalador imprimiu
`[inst-step-partition]` com o CI verde.

A lição generalizada: **um fallback que não derruba nada precisa de alguém procurando por ele.** Onde
houver, tem de haver um check que o busque.

**Fluent, e não gettext:** plural, gênero e ordem de argumento são do IDIOMA. Em gettext viram regra
numérica dentro do Rust; em Fluent ficam no `.ftl`, onde quem traduz mexe.

---

## 5. Buffers nativos e o protocolo

### Buffers nativos

Um buffer é um componente `wasm32-wasip2` que **não desenha**: produz uma `DisplayList` que o host
rasteriza. É o que o torna testável sem GPU, sem Wayland e sem compositor.

O invariante 11 virou propriedade dos **tipos**, não regra que alguém lembra:

- `Color` carrega o **nome** do token. Não existe construtor que aceite `0xRRGGBB`.
- `Text` separa `Data` de `Key`. O dado sai literal; o rótulo sai como chave e o host traduz. Um
  buffer não consegue escrever uma frase de UI em português.
- O `world` do WIT não oferece filesystem, relógio de parede nem socket além do que o WASI dá.

A lista de comandos é curta de propósito: texto, régua de uma linha, seleção. **Não há retângulo
preenchido, borda, sombra nem raio** — um comando de retângulo seria a porta de entrada para o card.

**`xdg_toplevel`, e não uma região nossa:** quem posiciona a janela é o niri. O shell nunca
reimplementa layout de janelas, e é o que mantém o invariante 3 possível.

---

### Terminal: dois motores, e o trait que os torna intercambiáveis

`alacritty_terminal` e a `libghostty-vt`, atrás do mesmo trait `Engine`. O diferencial compara os
dois e exige diff de grade **zero** nos quinze casos do corpus. Com um motor só ele degradaria para
golden-file, que pega **regressão** (mudei algo e a saída mudou) e não **divergência** (dois motores
discordam da mesma sequência) — e o binário imprime quantos motores tem, porque as duas coisas se
chamam "o diferencial passou" e significam coisas diferentes.

A `Grid` tem forma **canônica** porque sem ela dois motores nunca seriam comparáveis: espaço no fim
da linha é removido e TAB vira espaço. Tab é movimento, nunca conteúdo — um motor guarda `\t` na
célula de origem, outro preenche com espaço, e os dois estão no mesmo estado visual.

#### `#[repr(C)]` promete arranjar, não conferir

O binding do Ghostty foi escrito uma vez a partir dos headers, compilou, linkou e devolveu grade
vazia. **Três dos seus layouts estavam errados, e nenhum deles produz diagnóstico** — numa ABI de 64
bits, largura errada e tamanho errado são só registradores com conteúdo errado:

- `GhosttyPoint` tem uma **união de 16 bytes**; declarado como `{ tag, x, y }` dá 12 onde a
  biblioteca espera 24, e todo campo depois da tag chega deslocado. Sintoma: grade em branco.
- `GhosttyGridRef` é uma **sized struct** — o chamador preenche `size` com o `sizeof`, e é como o
  upstream versiona o layout. Zerada, ela diz ter zero bytes e a biblioteca acredita: `SIGSEGV` mudo.
- `GhosttyCell` é `uint64_t`, não ponteiro. Mesma largura, mesmo registrador, nenhum aviso.

**Quem achou os três foi um programa de vinte linhas em C.** O compilador C confere essas formas de
graça; o `#[repr(C)]` só promete arranjar o que lhe mandaram arranjar. Seis iterações de tentativa e
erro no Rust não responderam o que um `sizeof` respondeu de primeira. Os quatro tamanhos estão
trancados num teste, para a próxima mudança de upstream falhar com número em vez de com tela vazia.

E o cursor **não sai da grade**: pergunta-se ao terminal (`ghostty_terminal_get`), porque ele não é
conteúdo de célula. Deixá-lo na origem fazia os dois motores divergirem em quinze de quinze casos —
o que se lê como "eles não concordam em nada", e não como "um deles nunca foi perguntado".

#### O terminal não dizia ao programa que terminal ele é

Sem `TERM`, o ncurses **não** cai em nada: imprime `cannot initialize terminal type ($TERM=
"unknown"); exiting` e o processo morre na mesma hora. Medido com o `htop` pinado. O rodapé roda
como unidade de usuário, onde `TERM` não existe, e o filho do pty herdava esse vazio — de modo que a
frase que o próprio `open_application` escreve ("o `htop` aberto direto abre e morre") valia também
pelo nosso caminho.

O `Pty::open` agora informa `xterm-256color` quando o ambiente não informa nada, e traz o
`TERMINFO_DIRS` por caminho de store como piso (o NixOS define o dele; o laço aninhado num host que
não é NixOS, não). Nenhuma das duas linhas sobrescreve o que já existe: quem define `TERM` está
descrevendo o próprio terminal.

O `btop` tem exigência diferente e **barulhenta**: sem locale UTF-8 ele escreve `No UTF-8 locale
detected!` em vermelho e sai. Na máquina o `LANG` chega do `i18n.defaultLocale` pelo `pam_env`, então
não há o que consertar — mas um `systemd-run` de teste não tem sessão, e por isso o `terminal-vm`
declara o locale em vez de fingir que o ambiente é vazio.

---

### O monitor de processos abre FLUTUANTE — e a regra é gerada

`Ctrl+Alt+Esc` e `LEADER+Esc` abrem o `btop`; `LEADER+S-Esc` abre o `bottom`. Três teclas, duas
formas: as duas últimas são acordes (leader SEGURADA), a primeira não passa pela leader nenhuma.

**Por que a terceira existe fora da leader.** É o mesmo argumento dos outros dois binds de
emergência, e não conveniência: o que se procura quando a máquina para de responder é o monitor de
processos, e uma máquina em thrashing é justamente onde a leader pode não chegar — ela depende do
`keyd` distinguir toque de segurar, do daemon ser escalonado e do rodapé estar vivo para expandir.
Este bind pede ao compositor que faça `spawn`, e mais nada. É o companheiro de
`nix/modules/memory.nix`: aquele mantém o desktop vivo sob pressão, este é como a pessoa vê **o que**
está comendo a máquina.

**A tecla é tomada do niri.** O padrão de upstream põe `Mod+Escape` em
`toggle-keyboard-shortcuts-inhibit`; o nosso acorde entra primeiro e a deduplicação descarta o dele.
A ação não se perde — como as outras 140, alcança-se por nome (`LEADER :`), que é para isso que a
exceção existe.

**Quem faz flutuar é o niri** (inv. 4). O que o shell decide é a IDENTIDADE da janela: o mesmo
terminal sobe com `app_id` `br.ronoctua.monitor` em vez de `br.ronoctua.terminal`, e uma
`window-rule` casa esse id. Com um `app_id` só, a regra flutuaria todo terminal — inclusive o
`LEADER w o`.

**E a regra é GERADA do mesmo constante** (`export-niri-window-rules`, como os acordes). Escrita à
mão, a string existiria em três lugares — o Rust, `nix/modules/niri.nix` e `nix/dev.nix` —, e o dia
em que um deles divergisse o sintoma seria o monitor abrindo lado a lado: **exatamente com a cara de
um atalho funcionando**. O `terminal-vm` fecha o outro lado, com um niri de verdade: a janela do
monitor volta `is_floating` verdadeiro no IPC, a do terminal comum volta falso, e oito segundos
depois a janela ainda existe — o que só é verdade se o programa lá dentro não morreu.

Os dois pacotes custam ~7 MiB somados: os 48 MiB de cada closure são glibc e gcc-lib, que todo
closure do sistema já tem.

---

### O navegador de glifos é uma GRADE

Uma lista mostrava ~20 ícones por tela em 10.473, e cada linha gastava a largura inteira com um
**nome** que ninguém lê quando está procurando uma **forma**. Em grade cabem centenas, e o olho varre
por desenho — que é como se escolhe ícone.

O nome continua existindo: aparece embaixo, só o do glifo em foco. É a mesma economia do vertico —
detalhe do item atual, não de todos.

A grade obrigou duas coisas: `input` ganhou `left`/`right` no WIT (com dois sentidos, andar
para o lado só acontece por transbordo, e o cursor pula de uma ponta à outra da tela), e o componente
passou a guardar o último `context` que o `render` recebeu — o `event` não recebe contexto, e sem
ele `Down` não sabe quantas colunas pular. Havia ali um `80x24` chutado.

**E o host de buffers passou a mandar tecla.** Ele chamava `render` uma vez e parava: nenhuma tecla
chegava ao componente, e o navegador era uma imagem — não dava para buscar, rolar nem escolher. Agora
ele guarda a `Instance` viva, traduz tecla em `Input` e executa os `Effect` que voltam.

---

### O navegador de símbolos Unicode — `LEADER i u`

O grupo `i` nasceu dizendo que o navegador de Nerd Fonts "é a primeira coleção, e vão entrar outras".
Esta é a segunda, e é a outra metade da mesma pergunta: aquele acha o **ícone** certo entre 10.473,
este acha o **caractere** certo — a seta, o operador, o bloco de desenho, a letra grega.

#### A base é a interseção da fonte com o UCD, e o número é o argumento

O renderizador embute os arquivos de fonte que usa e não tem cadeia de fallback descoberta em tempo
de execução — é deliberado. Um caractere que a
Iosevka não desenha apareceria como tofu — o fallback silencioso com outra roupa. Então a base não é
"todo o Unicode": é `cmap ∩ UnicodeData.txt`, medida em **7.529 caracteres**. Dois geradores, o mesmo
princípio das duas autoridades: lá `cmap − base_cmap` dá os glifos Nerd; aqui o `base_cmap` cruzado
com o UCD dá os caracteres com nome.

O `unicode-character-database` do nixpkgs entra pinado, e a versão é escrita **à mão** no
`unicodegen.nix`. Tirá-la de `lib.getVersion` seria uma guarda que não guarda: ela seguiria o pacote,
e um `nix flake update` trocaria a base inteira sem nada acusar. O gerador confere a constante contra
o cabeçalho do próprio `Blocks.txt` (`# Blocks-17.0.0.txt`), e um `assert` de eval pega o caso comum
antes do build.

#### O léxico serve os dois navegadores

Os nomes Unicode são em inglês, e o mesmo `tags.en-pt.toml` que traduz `arrow` e `folder` traduz
`LEFTWARDS` e `MATHEMATICAL`. Ele já cobria **45,9%** das palavras; ~420 entradas depois cobre
**95,3%**, e **nenhum** dos 7.529 fica sem uma palavra em português. Dois léxicos significariam
traduzir `arrow` duas vezes e vê-los divergir — o vocabulário é do português, não do buffer. O trato
do arquivo virou `nix/scripts/lexicon.py`, importado pelos dois geradores, pela mesma razão.

#### Três armadilhas que o molde do navegador de glifos não cobre

**`}` está dentro desta base.** O componente não tem parser de JSON e divide os registros por `"},"`;
o navegador de glifos pode se dar a esse luxo porque a base dele começa em `U+E000`. Aqui `U+007D`
existe, e `"`, `,`, `[`, `]` e `:` também. O que salva **não é o parser**: é o gerador não emitir
nenhum campo de string com caractere arbitrário — o caractere viaja como número em `cp` e em
`shows`. Provado nos dois lados: um check conta os campos de string com pontuação de JSON, e o teste
do componente exige o total exato de 7.529. Com o campo `char` de volta, o primeiro acusa **7 campos**.

**302 caracteres têm avanço zero.** Desenhados sozinhos numa grade eles somem ou pintam sobre a
célula vizinha. A convenção é mostrá-los sobre `U+25CC`, e quem compõe é o **gerador** — o componente
não pode escrever codepoint (invariantes 9 e 11). Daí o campo `shows`, uma lista de codepoints. E a
regra que separa as duas coisas: **o desenho nunca é o que se copia**. Colar `◌́` no documento de quem
usa é o pior defeito que esta funcionalidade pode ter, e o único que sobrevive a qualquer inspeção
visual — por isso o teste afirma que a cópia devolve **um** caractere e que ele não é o círculo.

**23 caracteres têm avanço dobrado** (as setas longas). O orçamento do trecho acompanha a tinta,
senão o host elide um caractere só em `…`. O passo da grade (5) absorve as 4 colunas de tinta, e o
teste afirma a **relação** — quem apertar o vão horizontal amanhã faz os 23 pintarem sobre o vizinho,
e nada mais na tela diria isso.

#### Abre filtrado, e o `Esc` tem duas camadas

O buffer nasce com `simbolos:` **escrito no campo**, e não com um sinalizador escondido: quem abre vê
3.899 de 7.529 e vê por quê. Um filtro invisível na abertura seria a pior variante de estado oculto.

Por isso o `Esc` ganhou uma camada: `simbolos:seta` → `simbolos:` → `` (os 7.529) → nada. Uma camada
só derrubaria o filtro no primeiro toque, de surpresa; nenhuma prenderia quem quer chegar às letras.
Cada toque muda o campo e o contador, então não existe aqui a tecla que parece não fazer nada.

O painel oferece **sete famílias**, e não os 73 blocos: o nome de bloco mais longo tem 39 células
contra as 24 que o painel reserva, e 73 blocos × 2 idiomas seriam 146 chaves de nome próprio. Os
blocos continuam alcançáveis pelo prefixo `bloco:`, **por nome** e não por faixa hexadecimal — as
fronteiras dos blocos do Unicode não caem em múltiplos redondos, e o atalho do outro navegador
responderia errado em todas elas.

#### Os emoji, e a segunda face que eles obrigaram

A Iosevka desenha **75 dos 1.184** emoji de codepoint único (6%) e **nenhuma** das 2.760 sequências.
Emoji de verdade exigiram embutir uma segunda face — e é a primeira vez que o renderizador tem mais
de uma.

**Isso não contradiz a regra da fonte única.** O que ela proíbe é o renderizador *consultar* o
fontconfig; a cadeia
aqui é **declarada** em duas linhas de código, não descoberta em tempo de execução. "Qual face
desenhou isto" continua tendo resposta fixa, e o check `renderer-loads-three-faces` a fixa. A guarda
conta o número **exato** de faces, e não `> 0`: com `> 0`, a face de emoji poderia sumir do build e o
único sintoma seria o tofu voltando na tela de quem procura um emoji.

A escolhida é a **colorida**. Medi as duas moldando os 3.944 com harfbuzz: a monocromática acerta
2.612 (66%), decompõe 1.258 — 👩💻 em vez de 👩‍💻 — e dá tofu em 74; a colorida acerta **3.944
(100%)**. Emoji são conteúdo, como a prévia de imagem do clipboard, não chrome — a paleta própria
deles não briga com o invariante 9. Custou 10,2 MB no binário e **nada** na moldagem: o alvo continua
em 0,1 ms.

**A base virou a UNIÃO das duas faces.** Enquanto havia uma face, `cmap ∩ UCD` era a verdade; com
duas, a interseção antiga passaria a esconder caracteres que aparecem perfeitamente. São 11.614
registros, dos quais 3.944 emoji.

**As tags de emoji vêm do CLDR, não do léxico.** `cldr-annotations` dá nome e palavras-chave oficiais
em pt e en para **100%** dos 3.944, incluindo sequências (👩‍💻 → *tecnóloga*) e bandeiras
(🇧🇷 → *bandeira: Brasil*). Passar `grinning face` pelo léxico daria "rosto" e "sorridente" e mais
nada; o CLDR dá *engraçado*, *feliz*, *lol*, *rindo*. O léxico continua servindo o resto da base.

**A guarda que o `cmap` não dá.** Uma sequência ZWJ tem todos os codepoints no cmap e mesmo assim sai
decomposta se a fonte não tiver a ligadura. Por isso o gerador **molda cada sequência com harfbuzz** e
derruba o build se alguma não virar um glifo só — é o que impede a base de prometer 3.944 e a tela
entregar pedaços. Trocando pela fonte monocromática, ele acusa **1.332**.

##### Os nove grupos de emoji são uma SEÇÃO do painel, e ficam sempre visíveis

O painel mostra os nove grupos do `emoji-test.txt` o tempo todo, separados por uma régua e sob um
rótulo pequeno e apagado (*só emoji*).

A objeção óbvia é boa: em `familia:letra`, `grupo:comida` recorta zero, e oferecer na tela um recorte
que não recorta nada é pior do que não oferecer. Mostrá-los só com a família `emoji` escolhida atende
a objeção e cobra caro por isso — um painel que **muda de tamanho embaixo da mão**, com as opções
andando de lugar entre um `Tab` e o seguinte, e sem nenhum jeito de descobrir que os subfiltros
existem. A resposta não é esconder, é **separar**.

Isso obriga o painel a não ser um retângulo. Com seções, a conta "índice n vai na linha n / colunas"
para de valer — ela não sabe das linhas que não têm opção nenhuma. `Panel` monta uma **lista
explícita de linhas** (régua, rótulo, opções), e `cell`/`at` são duas leituras da mesma lista em vez
de duas contas que precisam concordar. E ele **rola** em vez de paginar: paginar exigiria dividir por
"quantas opções cabem numa página", e com seções de tamanhos diferentes isso deixa de ser uma
divisão. A seleção é clampada, porque a lista cresce ao escolher o emoji e encolhe ao sair dele.

Os 98 subgrupos ficam fora do painel e alcançáveis por `subgrupo:`, pela mesma razão dos 73 blocos.

##### Abrir num filtro obrigou a gramática a ceder

Com `familia:emoji` na abertura, digitar era impossível: `familia:` consumia todo o resto, e a
primeira letra virava `familia:emojir` — o valor deixava de existir e a tela esvaziava sem dizer por
quê. Os filtros de **enumeração** (`familia:`, `cat:`, `grupo:`) passaram a tomar uma palavra e deixar
o resto como termo livre; os de **texto livre** (`bloco:`, `subgrupo:`, `nome:`) continuam tomando o
resto inteiro, porque `bloco:box drawing` tem espaço dentro do valor.

#### A busca casava substring, e isso dava duas respostas erradas

As capturas de tela mostraram os dois sintomas ao mesmo tempo: `seta para cima` devolvia **zero**, e
`seta` punha **PESETA SIGN** em primeiro.

Três causas, e uma regra que resolve as três. `has_tag` casava o termo **inteiro** contra cada tag, e
as tags são palavras soltas — nenhuma contém uma frase. `match_score` dava nota 2 a "o nome contém" e
4 a "tem tag", então `pe·seta·` vencia a tag `seta`. E `filter_by` **descartava a pontuação sempre que
havia prefixo ativo** — como o buffer abre com um, a relevância estava desligada justamente na postura
de abertura, e a lista saía em ordem alfabética do nome em inglês.

A regra agora é **prefixo de palavra**, e a consulta é partida em palavras que precisam casar
**todas**, com a nota sendo a pior delas. Prefixo e não palavra inteira porque a busca é incremental:
quem digitou `arr` ainda não terminou de escrever `arrow`. A consulta e o texto são partidos pela
**mesma** fronteira — sem isso `seti-ui` seria um token só e não acharia a tag que o texto parte em
duas. E a fronteira é a pontuação ASCII, não "tudo que não é alfanumérico": um caractere colado não é
alfanumérico, e parti-lo por aí deixaria a consulta vazia — o gesto de colar um caractere para saber
qual é mostraria a base inteira.

O mesmo conserto foi para o navegador de Nerd Fonts, que tinha os dois defeitos idênticos.

---

#### Um defeito que só esta base tem

A nota de identidade da busca (`is_this`) era "o termo **contém** este caractere". No navegador de
glifos isso é seguro: os glifos são todos da PUA e ninguém os digita por acaso. Aqui a base tem o
alfabeto — digitar `LONG LEFTWARDS ARROW` dava nota de identidade ao `a`, e a **letra a** aparecia em
primeiro na busca por uma seta. Aconteceu, e o teste da largura dupla foi quem pegou. Agora a
identidade exige que o termo, aparado, seja **um** caractere; o gesto que ela atende é colar um
caractere para saber qual é, e esse gesto derruba até o filtro de abertura.

---

### O navegador de ícones Lucide — `LEADER i l`

O terceiro da família, e o primeiro em que a fonte não **delimita** a base: ela é **reescrita**.

#### 634 codepoints já eram de outra pessoa

A Lucide publica os 1.668 ícones em `U+E038..U+E6BB`. A Iosevka daqui é patcheada com Nerd Fonts, e
o Nerd Fonts ocupa **634 daqueles mesmos codepoints** — Powerline, Font Awesome Extension, Weather,
Seti-UI. A face primária é a pedida por nome, e por isso ela **vence a cadeia de fallback**.

Sem tratar isso, 38% dos ícones apareceriam desenhados como outro símbolo. **Não como tofu** — tofu
se vê. Como um ícone bonito e errado, que nenhuma inspeção visual pega, porque não há nada de
estranho na tela: há a coisa errada, com toda a confiança da coisa certa.

O gerador reescreve o `cmap` para `U+F2000..U+F2683`, no plano 15, e **confere que a faixa inteira
está livre nas outras duas faces** antes de gravar. O fixture negativo é a faixa de origem: ele
derruba o build dizendo os 634. Sai o `GSUB` junto — a fonte original casa o nome do ícone por
ligadura, e uma ligadura viva faria a palavra `home` escrita em qualquer lugar virar um desenho.

**A sonda que decidiu o desenho.** Antes de escrever qualquer coisa, perguntei ao `cosmic-text` qual
face desenha um codepoint que só a terceira tem. Ele acha por **codepoint**, sem precisar do nome da
família — e é por isso que o WIT **não mudou uma linha**: os ícones viajam como dado, como os emoji,
e nenhum buffer futuro ganhou superfície nova. Se a resposta tivesse sido outra, teria sido preciso
pôr um seletor de face no protocolo.

#### Dois codepoints por ícone, e eles não são cópias um do outro

Cada registro traz `cp` e `lcp`. O `cp` é o que a **tela desenha**, na face remapeada; o `lcp` é o
que a **Lucide publica**, e é o que serve a quem cola numa página com a fonte deles.

Isso parece a armadilha de sempre — duas representações do mesmo fato — e não é: são fatos de
**fontes diferentes**, sobre **fontes diferentes**. A ficha mostra o `lcp`, a cópia do caractere
devolve o `lcp`, e o critério que amarra isso está escrito no driver de testes: `house` é `U+E0F5`.

Um caso obrigou uma regra: a ferramenta que gera a fonte **funde desenhos idênticos num glifo só**.
No 0.563.0, `clock` e `clock-4` têm o mesmo traço e a fonte guarda só `clock-4`. A regra sai do
desenho, não de uma lista de exceções: quem ficou sem glifo procura o ícone de nós idênticos aos
seus. Uma versão que funda outro par continua funcionando; uma que perca um ícone de verdade para o
build.

#### A personalização é do artefato — e a prévia é o artefato

O site da Lucide oferece cor, tamanho, espessura e traço absoluto. O que ela muda é o **texto
copiado**, e aí ela é exata, inclusive a conta do traço absoluto (`espessura × 24 ÷ tamanho`, a mesma
da lucide.dev). São **19 formas de cópia**: as oito abas do site (SVG, JSX, HTML, React, Vue, Svelte,
Preact, Solid, Angular), mais o que ele não tem e este sistema usa — o caractere da fonte, o
codepoint, o traçado cru, o data URI, o base64, a regra de CSS e o sprite. O desenho sai do **SVG de
origem**, não do contorno do glifo: um contorno seria um preenchido que não responde a
`stroke-width` nenhum.

O `Tab` restaura os cinco ajustes de uma vez. Desfazer cinco linhas uma a uma é o tipo de trabalho
que faz ninguém experimentar.

**Um glifo continua não podendo honrar a espessura** — ele tem o traço assado dentro dele, a Lucide
publica uma espessura só, e nenhuma fonte faria isso variar. Durante dois marcos a conclusão foi que
a tela então não mostra, e que *fingir que varia seria mentir*. A premissa estava certa; a conclusão,
não — ela só valia enquanto a única coisa que a tela sabia desenhar fosse **glifo**.

O que a ficha mostra hoje, no canto superior direito, é o **SVG rasterizado** — literalmente o
documento que a cópia entrega, pela mesma chamada a `svg()`. Não há um segundo construtor, e é isso
que faz a prévia ser o artefato em vez de uma aproximação dele: um critério compara os dois **byte a
byte**, antes e depois de mexer no traço. O glifo `Huge` da esquerda fica: um é o que a fonte
desenha (o que a cópia do *caractere* entrega), o outro é o que o SVG entrega, e vê-los lado a lado é
informação.

#### A display list ganhou a primeira primitiva de pixel — e a trava que faltava

`command::svg` é a quinta primitiva, ao lado de texto, régua, grade e seleção, e a primeira que
produz pixel arbitrário. O cabeçalho do WIT diz por que a lista é curta: *"cada capability a mais é
superfície que todo buffer futuro passa a poder usar"*. Ela entra com três limites:

- **Capacidade.** Quem a emitir sem declarar `vector` no descritor é recusado — e o primeiro `render`
  acontece antes de a janela existir, então a recusa é na partida. Até aqui a linha do WIT que diz
  *"o host recusa quem usar sem ter pedido"* era **promessa**: `validate` conferia o prefixo do keymap
  e mais nada, e nenhuma capacidade era exigida de ninguém. A trava vale em **todo** quadro, não só
  no primeiro: a prévia só existe na ficha, e conferir uma vez seria conferir o quadro sem ela.
- **A superfície é menor do que "um SVG".** O `resvg` está na árvore sem motor de texto — `<text>`
  não desenha — e sem `resources-dir`, então referência a arquivo externo não carrega.
- **A cor não atravessa como valor** (invariante 11): o buffer manda o token, e o host o substitui
  onde o documento diz `currentColor`. Sem isso a prévia sairia **preta sobre fundo preto**, que é
  indistinguível de "esta primitiva ainda não está implementada". Quando a pessoa digita a cor dela,
  não há `currentColor` para substituir e o desenho sai como o artefato sairia — ali é conteúdo, não
  chrome, a mesma fronteira da prévia de imagem do clipboard.

O custo é por **tecla**, não por quadro: `render` só roda quando o componente pede, e o cache é
indexado pela string do SVG — que já codifica o ícone e os cinco ajustes, então a chave é exata. Um
documento que não rasterize deixa a área vazia, e é o que se vê ao digitar uma cor pela metade
(`#8ab`): a prévia some e volta. Inventar um desenho no lugar seria pior.

#### Preencher a caixa apagava o ajuste de tamanho

O retângulo que o buffer pede é o **teto**, não o alvo. Rasterizando para *preencher*, um
`width="24"` e um `width="512"` produzem a mesma imagem — o SVG da Lucide tem `viewBox` fixo de 24, e
o que muda entre os dois é só a escala, que o preenchimento desfaz. A prévia mostrava com confiança
um tamanho que não era o escolhido, e o ajuste que a pessoa acabara de mexer não tinha efeito nenhum.

`Fit::Natural` é a resposta: o desenho sai no tamanho que o **documento declara**, e só encolhe
quando não cabe — como qualquer visualizador. Acima do que a caixa comporta ele para de crescer, e o
número da linha continua dizendo o tamanho real.

A escolha oposta continua certa para **ícone de aplicativo** (`Fit::Fill`): ali o tamanho
intrínseco do arquivo não quer dizer nada — o mesmo tema publica `.svg` de 16 e de 512 — e o que
importa é ocupar a célula que a linha reservou. É por isso que ela é um parâmetro com nome, e não um
padrão: as duas respostas certas são opostas, e confundi-las foi exatamente o que aconteceu.

O mesmo cuidado vale um passo adiante, na `Area`: `encaixar` cresce até a borda e desfaria a
rasterização natural. Quem desenha a prévia usa `centrar`, que **não amplia**.

#### A marca diz o que a tecla faz, e ela sai da mesma tabela que a tecla lê

As 24 linhas da ficha tinham exatamente a mesma cara, e a seta lateral fazia **três** coisas
conforme a linha: mudava o valor, trocava de ícone, ou — na da cor — **nada**, em silêncio. Era tecla
morta, e o `typing` (que decide se o próximo dígito substitui ou acrescenta) não tinha
representação nenhuma na tela.

O defeito de fundo era que **quem desenha não sabia o que a tecla faz**: a afordância estava
espalhada por `step_by`, `type_in`, `delete` e o despacho de `Paste`, e o desenho não consultava
nenhum deles. Desenhar a marca a partir de uma segunda lista só adiaria o problema — ela divergiria
no primeiro ajuste novo, e uma marca que promete uma tecla que não faz nada é pior do que marca
nenhuma. Agora há uma `affordance(n)`, e **quem age pergunta a ela antes de agir**.

Duas células reservadas antes do valor, em toda linha — inclusive nas sem marca. É a regra que o
navegador de símbolos já escreveu para o marcador dele: reservar só onde há marca faria a coluna do
valor andar de lugar de linha para linha. E a linha da cor passou a trocar de ícone como as de
cópia, para *"sem marca de seta"* querer dizer uma coisa só.

#### 24 linhas acionáveis não cabem, e por isso a ficha rola

A ficha do navegador de símbolos tem 9 linhas de ação; esta tem **24** (5 ajustes + 19 cópias). Numa
janela baixa elas não cabem, e sem rolagem as últimas seriam inalcançáveis: a tecla anda, o realce
some da tela, e nada denuncia. O critério correspondente percorre as 24 numa janela de 20 linhas e
exige que a seleção esteja visível em **todas**, e que a última copie de verdade — chegar lá e ver não
é a mesma coisa que agir.

#### O rótulo do painel tem de achar a própria categoria

São 43 categorias, e o rótulo é lido na tela **e** digitado na busca: o painel diz "Setas", a base
diz `arrows`. Duas representações do mesmo fato, e sem amarrá-las elas divergem em silêncio — o
painel continua bonito e o que ele promete deixa de ser achável.

O critério exige que cada rótulo, nos **dois idiomas**, ache a categoria inteira. Ele já pegou uma:
`medical` estava traduzido no léxico como `medico`, sem acento, e o rótulo `Médico` achava 4 de 42.

#### Um check citado num comentário não é um check

O `text.rs` prometeu por dois marcos um check com nome e tudo, e ele **não existia** — ninguém
confere se o nome citado num comentário corresponde a algo. `renderer-loads-three-faces` faz mais do
que contar faces: afirma qual delas desenha texto, qual desenha emoji e qual desenha ícone, e
**testemunha a colisão**, exigindo que a Iosevka ainda responda por `U+E0A0`. No dia em que ela
deixar de responder, o check falha e alguém relê a decisão do remapeamento em vez de redescobri-la
pela tela.


#### Cortar por codepoint inventa desenho

`layout::width` contava **codepoints**, sob a premissa de que a fonte é monoespaçada de verdade e
cada codepoint ocupa uma célula. Isso vale enquanto há uma face só. Com a de emoji, `👩‍❤️‍👨` são
**seis** codepoints e **um** glifo; uma bandeira são dois, um tom de pele, dois, uma marca sobre o
círculo pontilhado, dois.

O sintoma não é "texto cortado curto demais" — é **outro desenho**. Com orçamento de 4 células, o
casal vira `👩‍❤` + `…`, que remolda como uma mulher, um coração e reticências: centenas de células de
uma vez, e nenhuma delas parecendo erro de corte. Parecem emoji errados.

O corte opera sobre **clusters** (`unicode-segmentation`, que já estava na árvore via
`cosmic-text`). Um cluster é o que o renderizador desenha como uma coisa, e é a única unidade em que
cortar não inventa conteúdo. Três testes fixam os quatro casos que esta base tem: ZWJ, bandeira, tom
de pele e marca combinante.

**A lição não é sobre emoji.** É que a premissa estava escrita — em maiúsculas, no comentário certo —
e mesmo assim sobreviveu à mudança que a invalidou, porque nada a *verificava*. Um comentário que
explica uma premissa não a defende.

#### Onde está a seleção, quando a cor não obedece

Num buffer monocromático o token de cor resolve: o item em foco vira `state-accent`. Emoji **não
obedecem** — a cor deles está dentro do glifo (bitmap CBDT), e `state-accent` não muda um pixel. Em
3.944 das 11.614 células a seleção era invisível, e justamente nas que o buffer abre mostrando.

O marcador é um **ícone de token** (`selection` → `cod-triangle_up`) na linha **abaixo** da célula em
foco, apontando para ela. Embaixo, e não em volta: a fileira tem três células de altura para uma
caixa de duas e pouco, e essa sobra estava sem uso. Um contorno exigiria pintar fundo, que o
protocolo não tem e o invariante 15 não quer.

#### Tom de pele: cinco filtros que saem da sequência, não do nome

`tom:claro` … `tom:escuro`, 406 emoji cada. O tom vem do **codepoint modificador** (`U+1F3FB..FF`) na
sequência, e não do nome: em inglês o CLDR escreve *"light skin tone"* no fim, em português *"pele
clara"*, e há emoji cujo nome cita pele sem ter modificador nenhum. O codepoint está lá, ou não está.

Vale um filtro por consulta, como todos os outros — `tom:escuro grupo:pessoas` não cruza os dois. O
que convive com ele é o **termo livre**, e é ele que atende o gesto real: `tom:escuro mao` → 79.


#### A face primária vencia o fallback, e 75 emoji saíam em monocromático

`UP!` aparecia como uma caixinha branca de letras pretas ao lado de `ATM` e
`SOON` coloridos. A causa é a mesma que obrigou o remapeamento da Lucide, do
outro lado da fronteira: **quem tem o codepoint primeiro, ganha**. A face
primária é a pedida por nome (`Family::Name`), e a Iosevka traz **75** dos emoji
de presença padrão desenhados como texto.

Não era tofu — tofu se vê. Era o emoji **errado**, e a tela não tinha como
denunciar: os dois existem e os dois aparecem. Foi o dono quem viu, numa
captura.

**O seletor de variação não resolve.** Medido: `U+1F199 U+FE0F` continua na
Iosevka e vira **dois** glifos, porque a primária tem os dois codepoints e o
`cosmic-text` não implementa a regra de "prefira a fonte colorida quando houver
`FE0F`". Sair um glifo a mais é pior do que sair o glifo errado.

O corte é na cópia que o renderizador embute, e a regra é do Unicode, não nossa:
**`Emoji_Presentation=Yes`** quer dizer "a forma padrão deste caractere é o
emoji". Medido no 17.0.0: 154 codepoints nas duas faces, dos quais **75** de
presença de emoji (saem) e **79** de presença de texto (ficam) — `#`, os
dígitos, `©`, `®`, `™`, as setas. Esses são bases de sequência, e desenhá-los
coloridos por padrão quebraria texto comum.

Duas autoridades independentes dão o mesmo conjunto, e conferi que concordam: a
propriedade `Emoji_Presentation` do `emoji-data.txt`, e as linhas
`fully-qualified` de um codepoint sem `FE0F` do `emoji-test.txt`.

Só a cópia do renderizador é cortada. As outras duas leituras da fonte são
autoridades sobre outra coisa — o `glyphgen` pergunta "que glifos o Nerd Fonts
acrescentou", o `unicodegen` pergunta "que caracteres existem" —, e a base
continua cobrindo os 75, agora pela face certa.

**Esta é a terceira vez que a mesma pergunta aparece com outra roupa:** qual
face desenha o quê. Por isso o check `renderer-loads-three-faces` deixou de
contar faces e passou a nomear casos: os 75 na face de emoji, `#` e `©` na
primária, e `U+E0A0` ainda na Iosevka (que é a premissa do remapeamento da
Lucide).

---

### O localizador de aplicativos — `LEADER a`

É a **sexta** vista da região expandida, ao lado do which-key, do `M-x`, do histórico, do clipboard e
do manual. Não é menu iniciar (invariante 14): mesma superfície, mesmo gesto de digitar-e-estreitar,
mesma tecla de sair. O que ele acrescenta é a fonte da lista.

#### A lista não espera por nada

O requisito do dono foi literal: *"todos os nomes e descrições devem aparecer em tela muito rápido e
antes mesmo dos ícones serem carregados"*. Ele decide a estrutura inteira.

**A varredura é da partida, não da tecla.** Medido: **128 entradas em 68 ms** — quatro orçamentos de
frame. Varrer ao apertar `LEADER a` daria um congelamento visível toda vez, justo no gesto que tem de
ser instantâneo. Ela roda numa thread enquanto o rodapé já está de pé, e o catálogo chega por
`calloop::channel`. Ao abrir, um `stat` por diretório diz se o catálogo envelheceu; se sim, a
revarredura vai para a thread e a lista **troca embaixo** sem perder o que foi digitado.

**O ícone tem duas velocidades.** O glifo sai por tabela, em memória, no mesmo quadro. O pixmap do
app vem por outra thread — `icon::procurar` + `resvg` no tamanho final — e substitui o glifo quando
chega. Só as linhas **visíveis** são pedidas (doze), e o cache guarda inclusive o `None`: sem
distinguir "não perguntei" de "perguntei e não existe", um nome sem arquivo seria procurado num
superpacote de 335 031 arquivos a cada quadro.

#### O que decide a ordem da busca

Duas chaves por item, nesta ordem: o **nome dobrado** e o **id do desktop**. O id costuma ser o
binário — digitar `nvim` para achar "Neovim", ou `nautilus` para achar "Arquivos", é o gesto de quem
conhece o sistema. A chave secundária leva um degrau de `+3` na pontuação, que é uma a mais que a pior
possível: **qualquer** casamento no nome vence **qualquer** casamento no id, senão um item que casa
por um texto que não está na tela apareceria na frente do que a pessoa está lendo.

A **descrição fica fora da busca**. `Comment` é prosa, subsequência sobre prosa casa metade da lista,
e o realce não teria onde acender — a linha entraria no resultado sem nada dizendo por quê.

#### A cadeia de ícone, e o invariante 10

`tray-glyphs.nix` pelo id **e** pelo `Icon=` → `category-glyphs.nix` pela primeira categoria que casar
→ pixmap do superpacote (assíncrono) → `icon.generic`. A tabela de categorias é uma **lista**, não um
attrset: um navegador declara `Network;WebBrowser` e as duas casam, e um attrset é ordenado pela
chave — `Network` venceria e todo navegador viraria o globo de rede.

A categoria vem antes do genérico porque o genérico é **indistinguível de "não resolvi"**: uma lista
inteira do mesmo quadradinho parece proposital, e é exatamente o sintoma de a tabela não ter chegado
ao Rust — que foi o estado real até aqui (a `Chain` existia com o campo `glyphs` e ninguém o
construía).

Quem consulta o superpacote é o **host**, para app **estrangeiro**, que é o caso para o qual a
`Chain` foi escrita. Nenhum buffer WASM o alcança, e nem poderia: o `world` do WIT não tem sistema de
arquivos.

#### A linha do minibuffer não tem rótulo, e o ponteiro tem fundo

O `M-x` leva `M-x ` na frente porque isso é a **notação da própria tecla** — é o que distingue o
localizador de comandos da lista de atalhos, que dividem a mesma região. O de aplicativos **não leva
nada**: um rótulo que não muda com a seta nem com o que se digita ocupa a linha sem responder a
pergunta nenhuma, e a lista logo abaixo já diz o que é. Sem nada digitado e sem ponteiro na lista, a
linha fica vazia.

O que ela mostra, quando o ponteiro está sobre uma linha, é o **binário que vai rodar** — e ele é o
caminho resolvido, sempre. Antes metade dos aplicativos mostrava `/nix/store/…/bin/x` e a outra
metade só `x`, conforme o que o fornecedor escreveu no `Exec=`. A resolução acontece **na varredura**,
onde ainda se tem o `PATH` da sessão; dentro do `systemd-run --scope` ele é mínimo, e um nome nu ali
falha longe da causa — a armadilha que o `CLAUDE.md` registra.

O **fundo** (`surface.selection`) marca a linha sob o ponteiro. O token existia desde o começo,
descrito como "fundo da linha em foco dentro de uma lista expandida", e nunca havia chegado ao
rodapé: só a TUI do instalador o usava, e a lista não dava sinal nenhum de onde o mouse estava —
clicar era um ato de fé. Ele é dado ao **ponteiro** e não ao foco de propósito: o foco já se
distingue por cor do texto (invariante 15), e marcar os dois deixaria "qual está selecionado?" sem
resposta no instante em que o ponteiro repousa sobre outra linha. E só onde há clique: no manual, que
só se lê, uma linha acesa prometeria uma ação que não existe.

#### A precedência das vistas passou a existir uma vez

Ela estava escrita em quatro lugares — altura, navegação, desenho e captura de teclado —, e o que
mantinha o rodapé de pé era as quatro concordarem. Divergiram uma vez, e o sintoma foi a superfície
pedir vinte e duas linhas para desenhar seis. Agora há `Footer::view()`, **derivada** das flags e
não guardada: guardá-la criaria a segunda verdade que ela existe para não ter.

##### E ter UM lugar não bastou; era preciso que todo mundo perguntasse a ele

`Footer::view()` existiu por meses enquanto o ramo de teclas, o `expansion()` e o `region_is_free`
continuavam lendo o PRÓPRIO campo de cada vista. Ou seja: a ordem passou a estar escrita num lugar e
mantida à mão em três, o que é pior do que estar escrita em quatro — porque agora havia uma função
com a autoridade de decidir, e ninguém a consultava.

As três ordens tinham divergido em silêncio. O `view()` punha o manual em terceiro e o gerenciador de
notificações em ÚLTIMO; o ramo de teclas punha o gerenciador em terceiro e o manual em décimo; o
`expansion()` começava pelo seletor de temas e punha o clipboard em quarto. Três revisores
independentes acharam o mesmo defeito em três vistas diferentes, o que é o que um invariante mantido
à mão faz: não falha num lugar, falha em todos ao mesmo tempo e cada pessoa vê um.

O conserto não é reordenar — reordenar seria manter à mão outra vez. É **toda guarda perguntar ao
`view()`**: então no máximo um ramo casa, e a ordem em que eles aparecem no arquivo deixa de ser uma
resposta. O que restou de ordem no arquivo é acidente, e o `the-footer-draws-what-the-keyboard-drives`
recusa qualquer ramo que volte a ler um campo.

A pergunta do minibuffer é a única exceção, e é deliberada: entrar numa rede pede a senha na LINHA
enquanto a região continua desenhando a lista em que se está entrando. Uma pergunta é uma linha, não
uma região — e ela é interceptada ANTES de tudo, porque quatro vistas carregavam
`&& mx.asking_for().is_none()` e as outras sete não, então com o clipboard ou o manual abertos a
pergunta era desenhada e não podia ser respondida.

#### Nada na expansão elidia

Descoberto ao pôr descrição na terceira coluna: uma linha de duzentas células passava por cima da
barra de rolagem e saía pela borda — e o compositor **não recorta**, ela simplesmente não aparecia.
As duas colunas agora elidem, e o teto de metade da tela que morava no desenho passou para
`grid_for`, junto com a coluna do ícone: altura, paginação, clique e pintura leem **o mesmo número**.

A elisão é pelo **fim**, nunca pelo meio: as posições do realce são do começo do texto, e cortar o
meio faria o grifo acender letra que não está mais ali.

#### `systemd-run --scope` é síncrono

Achado ao ligar o lançamento: do manual, *"execution in this case is synchronous, and will return only
when the command finishes"*. O comentário no código afirmava o contrário — verdade no modo serviço,
falso com `--scope`. Com o `calloop` numa thread só, abrir **qualquer** app estrangeiro congelava
teclado, pintura e eventos do niri pela vida inteira do aplicativo. Escondia-se atrás de quatro
chamadores fixos; o localizador faria dele o caminho principal. A espera foi para uma thread nomeada,
e o veredito volta pelo canal — o `apps-vm` abre um app que dorme dois minutos e exige o rodapé
respondendo enquanto ele vive.

---

### O driver gráfico vem do Nix, e não do host

O `nix run .#dev` parou de subir com uma mensagem que não nomeia nada:

```
niri: symbol lookup error: /usr/lib64/libc.so.6:
      undefined symbol: __pointer_chk_guard, version GLIBC_PRIVATE
```

**`LD_LIBRARY_PATH` vence o `DT_RUNPATH`.** O contorno para host não-NixOS punha `/usr/lib64` inteiro
lá para alcançar **uma** biblioteca — o `libEGL.so.1` do host — e junto ia o `libc.so.6`. O processo
passou a rodar a libc do host contra o `ld.so` do Nix, e os símbolos `GLIBC_PRIVATE` da glibc só
combinam entre um `libc.so.6` e o `ld.so` **da mesma build**. Medido: host 2.44, Nix 2.42, e o
símbolo ausente do host.

Não existe versão a acertar. O que remove a classe inteira é não pôr as bibliotecas do host no
caminho — e para AMD e Intel isso não custa nada, porque **quem fala com a placa é o driver do
kernel**, e o mesa é biblioteca de espaço de usuário que o Nix tem. Medido: o niri aninhado sobe em
`wayland-2` com nada do host no caminho.

**A NVIDIA é a exceção real**: o blob dela tem de casar com o módulo do kernel, então só pode vir do
host. Esse ramo ficou, e ele **avisa** o que está fazendo — numa máquina só-NVIDIA a mistura acima é
um risco que alguém tem de saber que está correndo.

De quebra, treze ICDs Vulkan viraram um. A seleção por vendor existia e **nunca rodou**: estava
escrita como função de shell dentro de uma string Nix, saía escapada pelo `escapeShellArg` e era
*chamada* — o shell procurava um programa cujo nome era o corpo do `case`. Falhava em silêncio
(`2>/dev/null || true`) e todos os ICDs eram carregados, que são justamente os 650 ms que a tabela
existe para evitar. O `shellcheck` do próprio build recusou a forma quebrada (SC2289) quando ela foi
reescrita.

---

### O gerenciador de arquivos — `LEADER .`, e duas portas para a mesma vista

É a **oitava** vista da região expandida, e a primeira que também abre em janela.
`ronoctua-files` é a única fronteira com o sistema de arquivos — mesmo critério do
`ronoctua-niri`, do `ronoctua-network` e do `ronoctua-memory`.

#### O pedido tinha um conflito, e ele decidiu a arquitetura

*"Menu de contexto no botão direito"* contra o invariante 14, que proíbe popup flutuante — e o
`surface.rs` **recusava o botão direito por escrito**, com esse argumento. O que estava datado era o
argumento, não o invariante: ele foi escrito quando a única implementação imaginável era uma
superfície nova. O menu que existe desenha na **mesma região** do which-key, não flutua, não tem
superfície própria, e cada linha mostra a tecla que a roda. O mouse não alcança nada que o teclado
não alcance — e isso não é promessa, é o `files-everything-has-a-key` comparando as duas listas.

O mesmo vale para o "tooltip": ele vai para a **linha de mensagem**, que é onde o which-key já põe a
descrição sob o ponteiro. Apontar o `w` do grupo em `rwxrw-r--` diz o que aquele bit faz **naquele
arquivo** — e numa pasta diz outra coisa, porque ali `x` é "entrar" e não "executar". A escolha entre
as duas frases é do IDIOMA, então mora no `.ftl`: foi por isso que este repositório ganhou o primeiro
**seletor Fluent**.

#### Uma implementação, duas portas

O rodapé tem teto de doze linhas (`space.expansion`) e é certo que tenha — ele é um rodapé. Doze
linhas bastam para consultar e agir, e não bastam para o trabalho que se abre um gerenciador para
fazer. Então há `ronoctua-shell files`, janela própria, `app_id` `br.ronoctua.files`.

**A janela não é um componente wasm, e não podia ser.** O `world` do WIT não importa sistema de
arquivos (invariante 11, e é estrutural: não existe no `State`), e o `input` só tem `click` — sem
hover, sem botão direito, sem roda. A versão wasm custaria uma capacidade nova, três casos de `input`
quebrando o `match` exaustivo dos seis buffers, um diretório inteiro atravessando por valor a cada
mudança, **e a vista escrita duas vezes** — porque o rodapé não pode ser wasm de jeito nenhum. O
`ronoctua-shell terminal` já é janela nativa: é esse o molde.

O que as duas portas dividem é `files::View` — estado, teclas, cliques, menu, seletor e as `Row`s.
O que difere é uma coisa só: **como se inicia um processo**. Por isso o seletor mora DENTRO da vista
em vez de usar o localizador do rodapé; a janela não tem localizador, e dois seletores seriam dois
comportamentos para o mesmo gesto.

#### A base de MIME custa 125 KB, e não 75 MiB

O closure do `shared-mime-info` é 75 MiB porque arrasta glib e libxml2 — que pertencem ao
`update-mime-database`, o programa que **compilou** a base, e ele já rodou no upstream. O que sobra em
`share/mime` são 125 KB de dados: `globs2` (1 231 regras), `magic`, `subclasses`, `aliases`,
`generic-icons` e `types`. O `nix/mimegen.nix` os transforma em tabela Rust em tempo de build, e o
`files-closure-has-no-mime-db` prova que nada do pacote sobrevive para a execução.

**O ícone por tipo não precisou de tabela escrita à mão.** O `generic-icons` mapeia os 1 231 tipos
para **dezessete** nomes genéricos, então `assets/icons/mime-glyphs.nix` tem vinte e cinco linhas e
cobre tudo por construção. Uma tabela extensão→ícone escrita à mão cobriria as extensões de que
alguém lembrou, pareceria completa, e responderia o quadradinho neutro para o resto — que é
indistinguível de "não resolvi".

**Duas velocidades, e a fronteira entre elas.** O nome responde sem I/O nenhum e é o que uma listagem
de duzentas linhas usa; o conteúdo só é lido quando alguém pergunta. Fazer magic por linha seriam
duzentos `open` dentro do quadro que desenha — a forma de defeito que o localizador já pagou uma vez
com ícones.

**E o gerador recusa antes de gerar.** Cinco linhas da primeira tabela nomeavam tipos que não
existem: `application/x-iso9660-image`, `application/x-sqlite3` e `text/x-rust` são nomes que
qualquer um juraria certos, e a base os escreve `application/vnd.efi.iso`, `application/vnd.sqlite3`
e `text/rust`. Linha que não casa com nada parece proposital e ninguém audita.

#### Um cursor que não se vê é uma tecla que não funciona

O gerenciador foi entregue com as setas funcionando e **a tela sem mostrar nada**. O relatório que
voltou foi "a interação por teclado não está funcionando"; o teclado estava perfeito. As outras
vistas pintam o foco **dentro do `expansion()`**, cada uma por si — não há realce genérico no
`paint` —, e as linhas do gerenciador não pintavam nenhum. Vinte e cinco testes de unidade
concordavam com o cursor, porque todos olhavam o ESTADO.

Por isso o teste que ficou não afirma que o estado mudou: ele compara **o que seria desenhado** antes
e depois da seta. Provado removendo o realce, e a mensagem é a da falha real: *"o cursor mexeu e o
desenho ficou igual"*.

E a divisão dos canais é a que o rodapé já usava: **foco é cor do texto, marca é glifo na calha**.
Duas informações no mesmo canal deixam "em qual está o cursor?" sem resposta no instante em que uma
segunda linha é marcada. A calha de uma célula faz dois trabalhos — ela é o ar entre o ícone e a
borda, sem o qual o glifo encosta e se lê como corte.

#### A letra virou QUERY, e os comandos foram para o modificador

O gerenciador nasceu com um teclado de vi: `j`/`k` moviam, `s` ordenava, `m` marcava, e o filtro
era um MODO em que se entrava com `/`. O argumento estava escrito no código e era bom até onde ia —
*uma tecla que às vezes é texto e às vezes é comando é uma tecla que ninguém consegue usar para
nenhum dos dois.* O que ele errava era **qual dos dois** a letra deveria ser.

Numa lista de NOMES DE ARQUIVO a letra é texto. É o que o localizador (`LEADER a`) e o `M-x` já
faziam — nos dois, o que se digita é a busca e nunca uma tecla —, e é o que o dono pediu ao usar o
gerenciador: digitar `setup` para achar `setup.sh`, sem cerimônia antes. Então a ambiguidade foi
resolvida para o outro lado: **a letra é query**, e os comandos da vista ganharam `C-` e `M-`
(`OF_THE_VIEW`). O `M-` está livre enquanto a vista tem o teclado — a leader não está escutando, e
nenhum buffer está.

Três coisas quebraram com isso, e nenhuma delas fazia barulho:

1. **`notation_of` DESCARTAVA os modificadores.** Era um `match` próprio, ao lado do
   `Key::notation` do `ronoctua-keymap`, e enquanto toda tecla da tabela era uma letra nua o defeito
   não aparecia: `C-n` e `n` saíam ambos como `n`. Com a tabela sob `C-`/`M-`, esse silêncio seria a
   funcionalidade inteira falhando. Hoje quem escreve a notação é o keymap, que é quem a lê de volta.
2. **As duas portas discordavam sobre o Shift.** `Footer::translate` desconta o Shift com um motivo
   escrito — o xkb já o aplicou ao produzir o caractere, então contar os dois transforma `M-S` em
   `M-S-S` e nunca casa — e a janela nativa o mandava adiante. Invisível enquanto a notação jogava
   os modificadores fora.
3. **O botão direito passou a digitar uma vírgula.** As duas portas abriam o menu SINTETIZANDO
   `Char(',')`, escrito à mão. Virou `files::key_that_runs("files-menu")`, que lê a tecla da mesma
   tabela: uma reamarração leva o mouse junto, que é o único arranjo em que os dois não divergem.

E o `Esc` ficou com duas etapas: com query na tela ele **limpa a query**; vazia, fecha a vista. Uma
tecla que jogasse fora as duas coisas de uma vez deixaria quem errou uma letra sem nada na tela.

#### O quadro custava o diretório inteiro, e isso não era o compositor

O relatório foi: *"ao passar o mouse rapidamente por cima dos itens, dá para ver um atraso entre a
seleção do hover e a posição do mouse — mas suspeito que seja da virtualização"*. A suspeita era
razoável: o laço aninhado põe um compositor a mais entre a mão e a tela. **Não era.**

Medido, em release, nesta máquina:

| entradas | lista inteira | janela de 12 |
|---------:|--------------:|-------------:|
|      200 |       0,61 ms |      0,03 ms |
|    2 000 |       8,06 ms |      0,03 ms |
|   10 000 |      56,87 ms |      0,03 ms |

Cada quadro montava uma linha para CADA entrada — as nove células do modo, o tamanho, a data, o dono
e o grupo, tudo string pronta — e desenhava doze. E fazia isso **várias vezes por quadro**: a altura,
a contagem de páginas e o deslocamento pediam a lista de novo, cada um por sua conta. Como o hover
redesenha a cada linha que o ponteiro cruza, o atraso era aritmética, e teria seguido o sistema para
o hardware de verdade.

A correção tem duas metades. `View::rows_from` monta só a fatia que vai ser desenhada — mantendo o
índice ABSOLUTO, senão o foco cairia sempre na primeira linha visível —, e `expansion_len` responde
a contagem **sem montar nada**. O teste que ficou não afirma "é rápido", que é frágil: ele afirma que
o custo **não cresce**, comparando um diretório de 20 com um de 3 000. Provado ao contrário — com a
lista inteira, 38 ms contra 6,7 s.

Para o que o desenho custa de fato, a medida do `bench` continua valendo: preparo 0,73 ms e desenho
1,44 ms por quadro. O que sobra de atraso, esse sim, é a cadeia de composição — e o laço aninhado
tem um compositor a mais nela.

#### A query foi para o fim do caminho, e a linha de mensagem voltou a ser dela

Digitar filtrava e o que se digitava aparecia na linha de mensagem — a mesma linha onde o ponteiro
escreve o que está embaixo dele e onde a última ação diz o que fez. As duas coisas se apagavam por
turnos, dependendo da ordem.

Hoje a query é o ÚLTIMO campo do caminho, na cor de acento. Lidos juntos, caminho e query são uma
frase só: onde você está, e o que está estreitando ali. A cor é toda a formatação — sem caixa, sem
separador, sem prefixo (invariantes 14 e 15) —, e o campo não leva a lugar nenhum, porque ainda não é
um diretório.

Isso obrigou o rodapé a mostrar o caminho, que ele **nunca mostrou**: a porta de janela o desenha
desde o primeiro dia, e a do rodapé desenhava as linhas e mais nada. "Onde eu estou" é a primeira
pergunta que uma listagem levanta. Custou duas das doze linhas — caminho e cabeçalho —, e as duas
saem de `files_chrome`, uma função só, porque `expansion` e `expansion_window` precisam das mesmas e
duas cópias divergiriam.

E o `Backspace` com nada digitado **sobe um nível**. É o mesmo gesto continuado: apaga o que digitou,
continua apagando, e o que está atrás da query é a pasta em que você está. Não fecha a vista —
segurar a tecla levaria embora justamente o que ninguém quis.

#### O botão direito ENTRAVA na pasta

Relatado da tela, em duas formas que parecem defeitos diferentes: clicar com o direito numa pasta
VAZIA entrava nela e não mostrava menu nenhum; numa pasta cheia, mostrava o menu — **sobre o conteúdo
da pasta em que acabara de entrar**.

Uma causa só. O botão direito "focava" a linha com o clique que ATIVA (`Hit::Name`), que põe o cursor
e abre. Na pasta vazia, `amount()` virava zero e a guarda `amount() > row` recusava o menu; na cheia,
a guarda passava e o menu abria no lugar errado. O código dizia isso em voz alta, num comentário
explicando como detectar o caso que ele mesmo criava: *"o clique acima pode ter ENTRADO num
diretório"*. Um gesto que precisa ser desfeito depois era o gesto errado — hoje existe `focus_row`,
que só move o cursor.

#### Antes de trocar permissão, ele pergunta

O menu já arma as linhas irreversíveis — a tecla duas vezes — e ali isso funciona porque a linha está
na tela dizendo o que é. Trocar permissão não é isso: quando aplica, a resposta veio de uma LISTA DE
PRESETS, e o que a lista dizia era "leitura e escrita para o dono", não `0644` em seis arquivos.

Então virou uma pergunta, no mesmo `Picker` de todas as outras: mesmas teclas, mesmo estreitamento,
mesmo `Esc`. `Asking::Confirm` carrega o [`Doing`] pronto, que é o que a torna reutilizável — nada
nela é sobre modos. E o **"não" é a primeira linha**, porque o cursor começa na primeira: numa
pergunta que existe porque a ação não é obviamente reversível, um `Enter` distraído tem de escolher
o que não faz nada.

De quebra, as perguntas ganharam TÍTULO. Faltava, e o relato tinha a forma exata da falta: uma lista
de nomes subia e nada dizia que era sobre o DONO. As escolhas não podem dizer — uma lista de pessoas
é igual seja para quem é dono, seja para quem enviar —, e a linha de menu que abriu a pergunta já
saiu da tela. O título vive em `Asking::title()`, no próprio enum, porque um segundo `match` ao lado
do que monta as escolhas é um segundo lugar para esquecer uma variante — e esse o compilador não
pegaria: ele desenharia a frase errada.

#### `LEADER f f` abria outra coisa

`f f` era o seletor de arquivo do EDITOR, e foi relatado como "um gerenciador de arquivos diferente e
inferior". A leitura estava certa: quem digita `f f` num grupo chamado "file" está atrás de arquivos,
e o que subia era uma lista chata de nomes que só sabia fazer uma coisa com o que você escolhesse.
Hoje `f f` abre o mesmo que `.`.

O grupo tem DUAS teclas e nada mais: `f f` abre o gerenciador no rodapé e **`f w` o abre em janela**,
`w` de *window* — a mesma letra que toda vista com segunda porta usa (`t w`, `n w`, `s n w`, `s p w`,
`s b w`, `s w`). O teto de doze linhas do rodapé está certo para um rodapé e não para o trabalho que
se abre um gerenciador para fazer.

Este parágrafo ensinou por meses duas teclas que não existem — `f e` para o seletor do editor e `f m`
para a janela — e nenhuma das duas jamais foi ligada; a janela nasceu em `f w`, e o seletor do editor
não existe em tecla nenhuma (o caminho é abrir o arquivo pelo gerenciador, que manda texto ao
`emacsclient`). Documentação que ensina tecla errada é documentação mentindo, e é por isso que a
varredura de menções virou parte do trabalho de mexer no keymap.

#### Duas margens, e uma que era paga duas vezes

O espaço à esquerda dos ícones é a **calha**: a célula onde a marca mora, e o ar de que o glifo
precisa para não encostar na borda. A porta de janela somava a ela a `buffer.marginX` inteira, então
a mesma tabela ficava recuada três células ali e uma no rodapé. Hoje a janela deve a DIFERENÇA, e o
número está escrito uma vez só (`CONTENT_MARGIN`) porque ele é usado três vezes — desenhar, medir
quantas colunas cabem, e decidir sobre qual célula o ponteiro está. Duas das três discordando é um
tooltip sobre o campo vizinho.

E do outro lado faltava: a data terminava colada na borda, o que se lê como coluna cortada e não como
coluna que acabou. `space.files.edgeRight` é o ar depois da última coluna — e não é o `edgeRight` da
modeline, que é UM e não zero, e pelo mesmo motivo: glifo encostado na borda se lê como glifo
CORTADO. O zero era o defeito, e o comentário do próprio token o conta. Uma tabela não é uma linha de
status, e as duas medidas continuam sendo duas.

#### A linha na TELA não é a linha da LISTAGEM

O botão direito continuou errado depois de `focus_row`, e por outra causa inteiramente. `row_at`
responde qual **linha visível** o ponteiro está; o botão esquerdo nunca se importou, porque ele passa
pela `ExpansionRow`, que carrega o índice ABSOLUTO. O direito passava o número visível adiante como
se fosse uma entrada — com o cabeçalho na linha zero ele agia uma linha abaixo, e com a linha de
caminho acima dele, duas. Clicar com o direito no primeiro arquivo abria o terceiro.

Hoje os dois botões perguntam a MESMA coisa à linha: o que você é? E a resposta do cabeçalho e da
linha de caminho é "não sou um arquivo", que é a razão de nenhum dos dois ter menu.

**E uma pasta vazia também tem menu.** "Não há nada aqui" não é "não há nada a fazer": numa pasta
vazia as ações úteis são justamente as sobre a PASTA — colar o que foi copiado, abrir um terminal
nela — e o botão direito não respondia nada. O sujeito do menu passou a ser `menu_subject()`: a
entrada em foco, ou, sem nenhuma, um `Entry` do próprio diretório, lido do disco como qualquer outro.
Sintetizar aqui, em vez de ensinar o menu sobre um segundo tipo de sujeito, é o que mantém
`menu::for_entry` uma função com uma regra só.

**E o menu diz sobre QUEM ele é**, na primeira linha. Ele cobre a listagem, então a linha em que foi
aberto não está mais na tela: vinte verbos sem nada dizendo no que agiriam. Com marcas é pior, porque
a resposta não é a linha sob o ponteiro — são todas.

#### O compositor vê a tecla primeiro, e o `M-` nunca chegou

As teclas da vista foram para `M-` quando a letra virou query, e `M-RET` **abria um terminal**. Não
era defeito da vista: no laço aninhado a família dos acordes da leader É o `Alt` — a config diz isso
com todas as letras —, então o niri consumia a tecla antes de o rodapé existir para ela.

`Ctrl` é a família que ninguém acima de nós reivindica: a leader é uma sequência, os acordes são
`Mod+…`, e os binds de emergência são `Ctrl+Alt+…`. A regra ficou simples o bastante para virar
teste (`no_key_of_the_view_is_the_compositors`): nenhuma tecla da vista usa Alt. É o mesmo raciocínio
de `keyboard-layout-exists` — o que decide não é o nosso código, e por isso tem de ser conferido.

#### Uma confirmação que cobre um dos dois caminhos não é uma confirmação

A pergunta antes de trocar permissão foi posta no caminho do PRESET, e o relato voltou dizendo que
clicar nas letras `rwx` continuava alterando na hora. Estava certo: o clique no bit é o caminho que
as pessoas usam de fato, e ele produzia o `Doing` direto. Hoje os dois passam pela mesma
`Asking::Confirm` — e o bit especial (setuid) também, que é o que menos se quer virar apontando.

#### Uma recusa que o ponteiro cobria

"Trocar o dono não fez nada" era sobre uma ação que respondeu perfeitamente bem — numa linha que
outra coisa estava cobrindo. A precedência da linha de mensagem punha a descrição do que está sob o
PONTEIRO na frente do eco, e o eco EXPIRA: com a mão no mouse e o ponteiro parado sobre a linha que
acabou de ser clicada, a mensagem vinha e ia embora por baixo do tooltip.

Hoje o eco vem primeiro. Uma descrição do que está sob o ponteiro pode esperar três segundos; uma
recusa não, porque nesse tempo ela já não existe. A porta de janela faz o mesmo, e lá a mensagem é
limpa também pelo clique — ela não tem relógio, e sem isso um erro seria dono da linha até alguém
apertar uma tecla.

#### O atraso do hover, terceira medição

Duas causas reais saíram nas rodadas anteriores (a lista inteira por quadro; e o mesmo por evento de
movimento, dentro de `row_at`). O relato continuou, e a suspeita do dono é a virtualização. As
medidas que temos:

| o quê | quanto |
|---|---|
| montar a janela de 12 linhas | 0,03 ms |
| preparo + desenho de um quadro (bench) | 0,73 + 1,44 ms |
| `branch_of_pid`, por quadro (medida da época; o segmento saiu) | 0,013 ms |

Nada disso explica um atraso perceptível. O que sobra dentro do nosso alcance é a LATÊNCIA de
apresentação, e ali havia um quadro a mais: `desired_maximum_frame_latency` era 2. A fila mais funda
existe para uma animação manter a GPU alimentada — ela compra VAZÃO com LATÊNCIA. Aqui nada anima:
todo quadro é a resposta a algo que acabou de acontecer, e a única coisa que importa é quão cedo ele
é visto. Com dois, o quadro que o movimento produziu podia esperar atrás de um já enfileirado — um
refresh inteiro, ~16 ms a 60 Hz.

O que continua fora do nosso alcance é a cadeia: no laço aninhado há **um compositor a mais** entre a
mão e a tela, nos dois sentidos. Medir isso exige a distro instalada, que é o item 3 do `PENDING.md`
— e é a razão de ele existir.

#### O editor da distro saiu, e o editor da distro é o Emacs

`buffers/editor` existia por uma razão só, e ela estava escrita na primeira linha dele: *"isto existe
para que `LEADER f f` pare de prometer algo que não acontece"*. A mesma docstring dizia *"não é o
Emacs e não tenta ser"* — e o editor deste sistema É o Doom Emacs, com módulo próprio, daemon e
`doom sync`. Ninguém pediu um segundo.

Com `f f` abrindo o gerenciador, a razão acabou. Saíram: o componente wasm, `RONOCTUA_BUFFER_EDITOR`,
o comando `file-open` e o seu bind, o seletor de arquivo do localizador, e as chaves de texto de
ambos. **O que ele fazia continua acontecendo**: abrir um arquivo de texto no gerenciador chama
`emacsclient -c`, por CAMINHO de store como tudo o mais que este sistema spawna. `emacsclient` e não
`emacs`, porque o módulo o roda como daemon — um segundo processo levaria segundos e não veria os
buffers da sessão. E ele pode NÃO estar instalado (o perfil `minimal` o deixa de fora), então a
ausência vira uma frase na tela e não um silêncio.

**E o caminho vem do AMBIENTE, não de um `option_env!` — a segunda vez que esta armadilha custou.**
Assado no binário, o caminho do Emacs virava referência que o workspace carregava para dentro de
TODA closure: `ronoctua.editor.emacs = false` tirava o pacote do `systemPackages` e deixava o Emacs
inteiro no sistema assim mesmo. O comentário que estava ali dizia, em tantas palavras, *"ele pode
estar ausente"* — que é exatamente a forma de um interruptor que não interrompe. É a mesma armadilha
do superpacote de ícones, com a mesma saída: quem põe a variável é o `nix/modules/emacs.nix`, dentro
do próprio `mkIf`, e o `nix/dev.nix` para o laço aninhado. Quem perguntou foi o
`profile-minimal-closes`, no gate — e a resposta veio do `nix why-depends`, não de leitura de código.

#### Uma pergunta vaga e duas respostas em chave crua

"Tem certeza?" não diz de quê, e quando ela aparece a linha que foi clicada já saiu da tela. Hoje a
`Asking::Confirm` carrega a FRASE pronta, construída onde o `Doing` era conhecido: *"Trocar as
permissões de um.txt para 0644?"* — e com marcas, "3 itens", porque agir num conjunto e nomear um
deles é como uma operação em lote passa por individual.

As respostas saíam `files-no` e `files-yes`. A causa é mais funda do que parece e vinha de antes:
`Choice.label` guardava uma CHAVE, com o comentário "resolvida por quem desenha" — e ninguém
desenhava. O `picker_rows` escrevia o rótulo verbatim, então a chave crua ia para a tela, que é
exatamente o que o invariante 8 recusa. E não era só o desenho: `Picker::shown` FILTRA pelo rótulo,
então digitar estreitava uma lista de chaves. Os presets de permissão tinham o mesmo defeito desde o
primeiro dia.

A correção foi dar o bundle à `View` — um `Rc`, porque `Texts` é um bundle Fluent e não clona, e
porque as duas portas já têm um. Os rótulos passam a ser TEXTO no momento em que a pergunta é
montada, e aí desenhar e filtrar concordam por construção.

#### "Trocar o dono não fez nada" — e tinha feito

O pedido nem chegava a ser escrito: `/run/ronoctua-files/` é `RuntimeDirectory` da unidade que
`nix/modules/files.nix` instala, e o laço aninhado não tem NixOS. O que saía da thread era
`criar /run/ronoctua-files/staged: Permission denied`, três segundos na linha de mensagem — e a
precedência daquela linha punha a descrição do PONTEIRO na frente do eco, então com a mão no mouse a
mensagem ia e voltava por baixo do tooltip.

Duas correções, e as duas valem por si: o eco passou a vir primeiro (uma descrição pode esperar, uma
recusa não, porque ela expira), e a troca de dono **pergunta antes de começar** se o lugar onde ela
escreve existe. Assim a resposta é síncrona e diz o porquê, em vez de ser um erro de thread que
alguém precisa pegar no ar.

#### O modificador da leader era descontado para sempre

`Inheritance` existe para que o `Ctrl+Alt` que ABRE a leader não entre na tecla seguinte, e a sua
docstring prometia que *"um modificador apertado depois, no meio da sequência, conta"*. Não contava:
`on_announcement` só gravava quando o campo estava vazio, e nada jamais o limpava. O desconto durava
a sessão inteira.

Ficou invisível enquanto nenhuma vista tinha binding com modificador. O gerenciador tem, e ali a
falha é barulhenta do jeito errado: `M-s` não ordenaria, ele digitaria um `s` na query. Hoje o
anúncio do compositor — que é a única coisa que sabe o que está segurado AGORA — apaga da herança o
que foi solto.

A prova de que a tecla chega é de ponta a ponta, no `leader-vm`: abre o gerenciador pela leader,
digita, e confere no PIXEL que a lista estreitou; depois `Esc`, e depois `alt+h`, que só faz algo se
o Alt sobreviver ao desconto. Um teste de unidade não podia responder isso — ele entrega a `Key`
direto na `View::key`, e a pergunta era se a tecla CHEGA, atravessando Xvfb, niri e a layer surface.

E o mesmo roteiro cobrou duas lições do próprio harness: sem `HOME`, o `systemd-run` abria o
gerenciador em `/`, cuja listagem bate no teto de doze linhas — e uma lista no teto mede igual antes
e depois, então os pixels param de responder à pergunta. E "a região encolheu" também passaria com
um filtro que não casa NADA: por isso a asserção é comparativa — `zzz` tem de deixar MAIS na tela
que `qqq`.

#### Marca é lote, e há linhas que não têm plural

Com marcas, uma ação age sobre **todas** — é a regra que todo gerenciador segue e a que surpreende
quando não é seguida: tendo marcado seis arquivos, agir no sétimo não é o que ninguém quis dizer.

Mas nem toda linha tem plural. Renomear seis arquivos para um nome não significa nada, e "abrir com"
abre uma coisa. Essas tomam o item **em foco**, com marcas ou sem — pegar o primeiro MARCADO em vez
disso renomearia um arquivo que a pessoa não está olhando.

**Isso estava errado e um teste o achou.** Mudar permissão lia o primeiro caminho marcado e o modo
atual do item em FOCO — dois arquivos diferentes — e não dizia nada sobre os outros cinco. Ação
parcial silenciosa é pior que recusa, porque nada conta qual metade aconteceu. E a **primeira recusa
interrompe o resto**, pelo mesmo motivo: um conjunto meio mudado, com uma mensagem sobre um arquivo,
é um estado em que ninguém consegue agir.

**Clique é sobre a linha em que caiu**, marcas ou não: apontar um bit de uma linha e mudar seis
arquivos não é o que aquele gesto quer dizer.

#### O que precisa de root, e o que decididamente não

Quase nada. Listar, renomear, copiar, mandar para a lixeira, apagar, comprimir e mudar a permissão
dos **próprios** arquivos não pede privilégio nenhum. Quatro coisas pedem: trocar o dono, trocar para
um grupo do qual não se participa, mudar a permissão de arquivo alheio, e dizer ao SSD quais blocos
estão livres.

Elas passam por **uma** unidade de nome FIXO — `%i` é string livre, e uma regra polkit que casa
prefixo é porta de largura desconhecida. O pedido é escrito num diretório da pessoa, **um arquivo por
campo**: um caminho pode conter quebra de linha, dois-pontos e espaço, e toda codificação de uma
linha só é um parser esperando para discordar de quem escreveu.

**Apagar e mover NÃO estão entre os verbos privilegiados**, e a ausência é o desenho: são os dois que
não se desfazem, e um apagar como root a uma tecla de uma listagem é como um gerenciador de arquivos
apaga o sistema de alguém. Quem precisa disso abre o terminal root que o menu oferece, onde o que
está acontecendo é visível.

E o ajudante **recusa antes de agir**: caminho relativo, `/nix/store`, `/proc`, `/sys`, `/dev`, e
link simbólico no último componente. Esse último é o afiado — sem `NOFOLLOW`, mostra-se um link à
pessoa e uma troca entre o clique e a unidade faz o root agir em outro arquivo.

#### O cabeçalho, a ficha e o caminho de volta

O **cabeçalho** custa uma das doze linhas do rodapé, e paga: sem ele as colunas não têm nome —
`rwxr-xr-x  ana  users` é legível para quem já sabe, e uma tabela legível só para quem já sabe é uma
tabela para ninguém mais. Ele também carrega o único estado que a listagem não conseguiria mostrar de
outro jeito: **qual** coluna ordena, e em que direção. E é onde a ordenação é clicada, que é o gesto
que toda tabela do mundo tem; o `s` percorre o mesmo ciclo.

**Ordenar não move o cursor.** O `reread` o mantém pelo NOME, e é certo: ordena-se para ver onde um
arquivo está, não para perdê-lo de vista. Um teste chegou a afirmar o contrário, e o código estava
certo.

A **ficha de detalhe** (`i`) substitui a listagem em vez de ficar ao lado dela — doze linhas divididas
em dois painéis dariam seis a cada um. Ela mostra o que a listagem não cabe, e uma coisa que a
listagem deliberadamente não faz: **o que o CONTEÚDO diz que o arquivo é**, ao lado do que o nome
afirma. Um `.txt` com um PNG dentro é coisa real, e o caminho rápido da listagem — que nunca abre
arquivo — continuaria chamando aquilo de texto para sempre. Qualquer tecla a fecha: ela é aberta para
se olhar, então nenhum gesto se perde esperando o certo.

E a lixeira tem **volta**. Um "mover para a lixeira" de onde não se restaura é metade da ação, e a
metade que falta é a que torna a outra segura: navegar até a lixeira oferece `devolver`, que lê o
`.trashinfo` e põe o arquivo de volta onde estava. A raiz da lixeira é **parâmetro**, como o roster e
o fuso — dois testes lendo a mesma variável de ambiente correm, e quem perde põe um arquivo na
lixeira do outro.

#### Apagar sobrescrevendo: o que ele faz, e o que não pode fazer

Ele escreve por cima, sincroniza a cada passagem, trunca, renomeia para um nome sem significado e
desliga. Num sistema de arquivos que escreve no lugar e num disco que expõe seus blocos reais, isso é
destruição.

**Esta máquina não é nenhum dos dois.** O btrfs é copy-on-write: sobrescrever aloca extents NOVOS e
deixa os velhos intactos até serem recuperados, e um snapshot pode segurá-los para sempre. O SSD
nivela desgaste: o controlador remapeia a escrita e a cópia anterior fica num bloco que o sistema
operacional não consegue nomear.

Então a confirmação diz isso, nessas palavras, em vez de prometer um apagamento que não pode cumprir.
O que garante de verdade é a criptografia de disco que o sistema já tem — bloco que ninguém decifra é
bloco que ninguém recupera —, e o `fstrim` é o único passo que fala com o controlador.

#### Três defeitos que os testes acharam

- **Todo PNG virava APNG.** Os dois abrem com os mesmos oito bytes; o que os separa é uma regra
  aninhada do `magic`, e ela é **obrigatória**, não opcional. Tratada como opcional, a regra externa
  bastava — e toda foto de uma listagem voltava como PNG animado, com o ícone certo e o nome errado.
- **A coluna do nome vazava da tela.** Com um cell de largura, o ícone sozinho empurrava o nome para
  fora, e o compositor não recorta: o nome simplesmente não estava lá. O ícone passou a ceder também
  — por último, e o que nunca cede é o nome.
- **O menu mostrava `files-open`.** Chave crua na tela, que é a forma exata que o invariante 8 existe
  para recusar. Ela apareceu porque `rows()` não recebia o pacote de textos; passar a recebê-lo é o
  que faz `Field::text` ser sempre uma string pronta.

---

### A fila de transferências — `LEADER t t`, e por que ela é uma UNIDADE

Sete operações do gerenciador subiam uma thread e trabalhavam em silêncio: colar, apagar,
sobrescrever, lixeira, comprimir, extrair e medir pasta. A única coisa que a tela dizia sobre
qualquer uma delas era a listagem se atualizando quando acabava — inclusive no `shred`, que escreve
três vezes o arquivo e é a mais lenta de todas. Colar 40 GiB não dizia **nada**.

#### A fila não pode viver no rodapé, e não é preferência

Duas razões, e cada uma se sustenta sozinha.

**A mesma lista tem de ser vista de TRÊS processos.** O rodapé desenha a porcentagem na modeline e o
painel na região expandida; a janela do gerenciador é um `exec` do próprio binário, e portanto outro
processo inteiro; e o buffer wasip2 será um terceiro. Uma fila dentro do rodapé seria invisível para
os outros dois, e cada um acabaria criando a sua.

**E uma cópia de 40 GiB não pode morrer com o rodapé.** No `nix run .#dev` o rodapé reinicia **a cada
edição de arquivo**; na máquina instalada ele tem `Restart=on-failure` com um segundo — o que está
certo para uma tela e errado para uma hora de cópia.

Por isso `ronoctua-transfers` é um serviço de usuário, com o binário **extraído** do workspace
(`rust.transfers`). O check `transfers-unit-is-narrow` mede a closure: dezesseis caminhos, e nenhum
deles desenha. Com `rust.shell` no lugar seriam mesa e o carregador Vulkan dentro de uma unidade que
nunca abre janela — e nada falharia, que é exatamente por que existe um check.

#### O socket fala JSON, e é o único do projeto que fala

O socket de controle do rodapé fala palavras separadas por espaço, e o `rsplit_once(' ')` que existe
lá é um remendo: a sequência (`m f s`) tem espaços e o id não, e dividir no primeiro devolvia um par
plausível que listava uma tecla e não rodava nada.

Aqui o que atravessa são **caminhos**, e depois serão URLs. Um caminho carrega espaço, dois-pontos,
quebra de linha — e bytes que não são texto nenhum. Uma gramática de espaços perderia metade sem
dizer nada.

E um caminho **não é uma string**. `serde_json` serializa `PathBuf` por `to_str()` e **falha** num
nome que não é UTF-8: um gerenciador que levasse caminhos como texto recusaria copiar um arquivo
vindo de um disco Windows, e recusaria no socket, três processos longe de quem pediu. Por isso o
caminho vai como texto quando É texto e como bytes quando não é — o caso normal continua legível num
`socat`, e o esquisito continua correto. O teste que fecha isso manda um `nome-\xFF-torto` pelo
socket de verdade e confere os bytes na volta.

**O invariante 11 continua inteiro:** o JSON morre no host. Um buffer wasm recebe registros tipados
pelo WIT e não tem parser.

#### A cópia continua sendo do kernel — o que mudou foi o tamanho da mordida

`std::fs::copy` é `copy_file_range` por baixo, e no btrfs isso é cópia **do lado do servidor**: o
sistema de arquivos compartilha as extensões em vez de mover bytes por este processo. Trocá-la por um
laço de `read`/`write` para ganhar um número de progresso teria deixado toda cópia dentro do disco
dramaticamente mais lenta — pagando segundos por uma porcentagem.

Então a chamada fica e o que muda é o TAMANHO: oito mebibytes por vez, com o interruptor e o contador
lidos no meio. **Medido em 2026-08-21, um gibibyte no btrfs desta máquina: 153 ms numa chamada só
contra 178 ms em mordidas** — dezesseis por cento, e o que sobra são 128 syscalls onde havia uma. Um
laço de `read`/`write` estaria dez a cem vezes pior, que é a ordem de grandeza que o número existe
para separar. E a queda para `read`/`write` não é enfeite — `copy_file_range` recusa entre sistemas
de arquivos em kernel antigo (`EXDEV`), recusa o que não é arquivo comum (`EINVAL`) e em alguns não
existe (`ENOSYS`) —, e o caso interessante, copiar para um pendrive, é justamente o que atravessa.

#### Desistir APAGA o pedaço, e essa é a razão de o botão existir

`free_name` garante que não havia nada no destino antes, então tirar o que foi escrito não pode levar
o arquivo de ninguém. Sem isso, "colei 40 GiB na pasta errada" viraria "colei 19 GiB na pasta
errada", que é o mesmo problema com um número menor.

**Pausar não é parar.** O interruptor tem três estados e dois deles encerram a operação se
`allowed` for escrito com um `if` só — o teste `pausing_holds_and_resuming_finishes` é o que separa
os dois, e ele solta a pausa de OUTRA thread, que é o arranjo que a fila tem de verdade.

#### O cursor do painel é um ID, nunca uma linha

A lista **reordena por baixo**: o que ainda acontece fica acima do que acabou, então no instante em
que uma cópia termina toda linha abaixo dela se move. Um cursor que estava na linha três apontaria
para outro trabalho sem a mão ter saído do lugar — e o `C-c` seguinte desistiria da transferência
errada.

#### A modeline some em repouso, e o sino também

O sino só existe com algo não lido — em repouso a linha da direita é vazia, por decisão do dono. O
que ele conserva é a PRIORIDADE: numa linha estreita demais para tudo, ele é a última das marcas da
direita a sair (2 contra 3 da transferência e 5 da atualização), porque as outras têm um painel e um
diário e uma notificação que ninguém viu não está em lugar nenhum. E reservar as duas bordas
(`modeline.edgeLeft`/`edgeRight`) no orçamento do `resolve` veio junto: uma linha cujo único
sobrevivente é o nome do buffer, à esquerda, era encolhida à largura inteira e desenhada uma célula
adentro — uma célula fora da tela —, e essa linha só passou a existir quando o sino deixou de estar
sempre lá. O segmento das transferências aparece porque algo está acontecendo; em repouso seria uma
coluna gasta com `0%`, para sempre, em toda tela.

Mas ele **fica quando algo falhou**. Uma cópia que morreu há dois minutos não deixa nada correndo, e
um segmento que só mostrasse trabalho em curso tiraria a falha da tela antes de alguém ver.

E a média é média, não soma: um contador conta bytes e outro conta entradas. Somar dá um número em
coisa nenhuma — um arquivo de um gibibyte mais três entradas apagadas leria `1073741827` de algo.

#### O que a fila não faz

Não desenha, não traduz e não toca o disco: toda operação local é do `ronoctua_files::watch`,
vigiada. O rótulo de uma linha sai como CHAVE de texto mais dado (`Work::word`, `Stage::word`), e
quem tem o pacote monta a frase — invariante 8. O check `transfers-words-are-total` cobra as duas
línguas para cada palavra, porque a chave é **montada em runtime** e é portanto uma das que a seção
das sete derivadas descreve: uma variante nova não falha a compilação, ela desenha a chave crua na
tela.

**Uma unidade morta por `SIGTERM` deixa o pedaço no destino.** Desistir pelo painel não — aquele
caminho desenrola e remove o que escreveu —, mas matar é matar, e o processo não tem tratador. Está
escrito no módulo, e o `TimeoutStopSec` é curto de propósito: uma fila que demorasse noventa segundos
para morrer atrasaria todo logout.

#### Quatro motores, e o motor sai do trabalho

`Engine` tem quatro valores e `Ask::Add` não carrega nenhum: quem decide é `Work::engine()`. A
alternativa — um campo de motor no pedido — deixaria um chamador pedir que um magnet fosse *colado*,
e o `serve` teria de escolher em qual metade do pedido acreditar. Aqui não há do que discordar, e o
despacho é o único `match` do arquivo.

- **`Local`** — as sete operações do gerenciador, pelo `ronoctua_files::watch`.
- **`Fetch`** — aria2, por JSON-RPC no *loopback*. **HTTP escrito à mão**, e a razão é o
  `Cargo.lock`: a conversa inteira é um POST de um objeto pequeno para `127.0.0.1`, sem TLS, sem
  redirecionamento e sem reuso de conexão; um cliente HTTP traria `tokio`, `hyper` e uma pilha de TLS
  para o `Cargo.lock` que todo crate do workspace compartilha, para poupar sessenta linhas que nunca
  vão mudar. **O segredo nunca vai em `argv`** — ali seria legível por qualquer processo em
  `/proc/*/cmdline` — e sim num arquivo `0600` dentro do `XDG_RUNTIME_DIR`, que é `0700` e da pessoa.
  Ele **nasce na máquina** a cada sessão (invariante 13), e o diretório ser esvaziado no logout é o
  que faz a rotação acontecer sozinha.
- **`Media`** — yt-dlp, como processo, **em grupo próprio**. Pausar é `SIGSTOP` no GRUPO e não no
  processo: o yt-dlp sobe um ffmpeg para juntar as faixas, e parar só o pai deixaria o filho
  trabalhando. É uma pausa mais honesta que matar e recomeçar, que jogaria fora a extração e bateria
  no site de novo. O contêiner é **MKV**, e não é preferência: as faixas boas do YouTube são VP9 ou
  AV1 com Opus, que não cabem em MP4 — forçar MP4 faz o yt-dlp *descer* para um par que caiba. O
  caminho do binário vem do **ambiente**, posto pela unidade: baked por `option_env!` ele seria uma
  referência, e arrastaria o yt-dlp para o perfil reduzido, onde `ronoctua.media.enable` é `false` —
  a mesma armadilha em que o `RONOCTUA_EMACSCLIENT` caiu. (O `ffmpeg-full` já não entra nessa conta:
  ele é incondicional desde que o dono pediu suporte completo a formatos.)
- **`Torrent`** — uma sessão do libtorrent para TODAS as linhas, e por isso o único motor que não é
  uma thread por trabalho: uma sessão é um objeto com porta, cache de disco e threads próprias, e
  duas seriam duas portas e duas cópias da DHT. A thread por trabalho que o `serve` sobe apenas
  entrega o pedido e volta.

### BitTorrent v2 por FFI, e por que não foi escrito do zero

O dono pediu **BitTorrent v2**. Medido no nixpkgs pinado, não suposto: o `librqbit` do `rqbit` 8.1.1
não tem `meta version`, nem `file tree`, nem `piece layers` — o único vestígio de v2 é um alias de
tipo com um comentário. Escrever v2 é metainfo v2, verificação Merkle com SHA-256, torrent híbrido e
a metade v2 do protocolo de peers: trabalho de especialista, com prazo incerto.

E o fecho contradiz a intuição:

| | fecho | v2 hoje | `unsafe`/C++ | tokio |
|---|---:|:---:|:---:|:---:|
| librqbit (biblioteca) | 69,8 MiB | não | não | **sim** |
| **libtorrent-rasterbar** | **62,7 MiB** | **sim** | sim | não |
| qbittorrent-nox (fora do processo) | 886,7 MiB | sim | não | — |

O que pesa nos daemons é Qt e Python, não o motor. O libtorrent é **mais leve que o rqbit**, fala v2
hoje e não traz um segundo modelo de concorrência. O que se perde é a autoria do motor — o app
continua nosso: a fila, o painel, o teclado, os textos e o `.desktop`. A troca foi feita de propósito
e está escrita aqui porque contradiz o "criado do zero" da conversa que a originou.

#### A fronteira é de seis verbos, e não da biblioteca

O libtorrent são 272 cabeçalhos `.hpp` e **nenhuma ABI C**: bindá-lo seria bindar C++, que não tem
ABI estável, tem *name mangling*, exceções e `std::shared_ptr`. O que a fila precisa são seis verbos
— adicionar, pausar, retomar, remover, "onde está tudo" e salvar o retomar —, e isso cabe em C
puro. `nix/torrent-shim/shim.h` é a fronteira inteira.

**Nada que atravessa possui memória.** Toda string que volta é um arranjo de tamanho fixo dentro de
um `struct` que o chamador alocou: não há `free` para esquecer nem tempo de vida para errar. Custa
uma cópia por leitura e elimina uma classe inteira de defeito.

**E `sizeof` É o contrato.** O `CLAUDE.md` já registra que `#[repr(C)]` promete ARRANJAR e não
conferir — um tamanho errado é um registrador com o conteúdo errado, sem diagnóstico nenhum, e no
`libghostty-vt` isso custou três layouts errados achados à mão com um programa em C. Aqui a medição
virou check: `nix/torrent-shim.nix` compila o cabeçalho REAL com o compilador C++ real e escreve
cada `sizeof` e cada `offsetof`; `torrent-layout-matches-cxx` compara essa tabela com as constantes
do Rust, e nenhum dos dois lados pode andar sozinho.

Uma armadilha nova, que o Ghostty não teve: **`-fvisibility=hidden` esconde o `extern "C"` também**.
Sem ele o `.so` exporta as dezenas de milhares de símbolos que os cabeçalhos do libtorrent
instanciam; com ele, e sem `__attribute__((visibility("default")))` nas doze funções, o link do Rust
falha com "undefined symbol" longe da bandeira que causou.

#### v2 não é alegação: há uma prova, e ela é offline

"libtorrent 2.0 suporta BEP 52" é o que um changelog diz. O `ronoctua-test-torrent` faz um torrent
**puramente v2** a partir de um diretório que ele mesmo escreveu, entrega a uma sessão de verdade, e
exige um info hash v2 e **nenhum v1** — depois faz o mesmo com um torrent só-v1, para que uma
bandeira que fosse simplesmente sempre verdadeira também reprovasse. Confere ainda os bytes do
próprio `.torrent` (`meta version`, `file tree`, `piece layers`, e nenhum `pieces` do v1) e que os
pedaços **verificam**: chegar a `seeding` com `done == total` é ter percorrido a árvore Merkle de
SHA-256 sobre bytes em disco. Nada disso toca tracker, peer ou DHT.

O hash que nomeia um torrent aqui é o v2 inteiro quando existe, e nunca o `get_best()` do libtorrent
— que TRUNCA um SHA-256 a vinte bytes para que caminhos v1 possam carregá-lo. Truncados, dois
torrents v2 diferentes podem colidir, e o que esse nome identifica é o que um cancelamento apaga.

#### Semear é porta aberta, e um torrent pronto continua semeando

A sessão escuta em 6881-6889, uma faixa fixa que `nix/modules/transfers.nix` abre no firewall e que
a thread da sessão declara com os mesmos números. **UPnP e NAT-PMP ficam desligados**: um programa
que pede ao roteador para abrir uma porta sozinho mudou a exposição da máquina sem ninguém decidir.

A linha vai para `done` quando os bytes chegaram, e a sessão continua devolvendo-os — que é para o
que serve um cliente de torrent. **Desistir (`C-c`) é o que para**, e leva os dados junto; tirar a
linha da lista não para a semeadura. Isso está dito aqui e no módulo em vez de ser descoberto.

### Baixar como você, e o que isso custa

Quatro coisas precisam de conta: vídeo com restrição de idade, vídeo privado,
clube de membros, e 1080p Premium. O modo normal é anônimo e continua sendo o
padrão; o que segue é a outra metade, e ela tem um preço que está dito no nome
da tecla.

#### OAuth2 não é uma das saídas, e isso foi medido

O dono pediu OAuth2, e o yt-dlp pinado responde no próprio código-fonte —
`extractor/youtube/_base.py:655`:

```python
if username.startswith('oauth'):
    raise ExtractorError(
        f'Login with OAuth is no longer supported. {self._youtube_login_hint}', ...)
```

A linha seguinte recusa senha também. O `_youtube_login_hint` que ele aponta diz
**cookies**. Então cookies é o que existe, e não por preferência: é o que sobrou.

O papel que o OAuth fazia — provar que quem pede é um dispositivo de verdade —
passou para o **PO token**. O yt-dlp carrega o arcabouço inteiro
(`extractor/youtube/pot/`) e **nenhum gerador**: cunhar um token é executar o
desafio BotGuard do Google, que é JavaScript ofuscado.

#### O gerador roda quando é preciso e sai — e essa é a forma, não um detalhe

`ronoctua.transfers.poToken = "on-demand"` liga o gerador; o padrão é `none`.

O custo é **53,6 MiB**, e ele é menor do que parece — o que foi medido, e não
suposto. Cunhar exige um runtime de JavaScript, e a primeira conta que fiz
somava os 182,6 MiB do Deno; o `nix why-depends` disse outra coisa:

```
nixos-system → system-path → yt-dlp-2026.07.04 → deno-2.8.3
```

O yt-dlp do nixpkgs **já depende do Deno** (ele o usa para decifrar assinatura),
então o runtime está pago de qualquer jeito e esta opção acrescenta só o
gerador. Continua desligada por padrão porque é um subprocesso por token numa
máquina que pode nunca baixar vídeo — não porque pese.

Upstream oferece duas formas, e a escolha entre elas é a razão de
`nix/pot-provider.nix` existir:

- um **servidor HTTP** que fica de pé e faz cache — rápido na repetição, e um
  processo residente pela sessão inteira;
- um **script**, invocado uma vez por token, que imprime o token e sai.

A segunda é a empacotada, e a primeira é **deliberadamente deixada de fora**: o
`getpot_bgutil_http.py` não é instalado, então não existe caminho de código que
procure um servidor, não há porta, e não há nada para ninguém lembrar de
encerrar. O custo são alguns segundos na primeira vez que um vídeo precisa de
token; eles ficam num cache sob `XDG_CACHE_HOME` e valem horas.

`pot-provider-is-script-only` confere isso como forma: reprova se a metade
servidora aparecer, e — dentro do sandbox, **sem rede** — roda o gerador e
compara a versão dele com a do plugin. Rodar sem rede é o que prova que Deno e
todas as dependências estão no fecho em vez de serem buscadas no primeiro uso.

#### Deno, e não Node

Não é preferência. O caminho do Node quer `build/generate_once.js`, o que
significa compilar o TypeScript com swc — um passo de build, uma toolchain, e
uma segunda coisa para manter em dia. O Deno lê `src/generate_once.ts` como
está. O próprio plugin concorda: pontua Deno 20 e Node 10, e pega o maior.

E o Node **não entra no fecho**, embora o `npm` seja usado para montar o
`node_modules`. Medido: 302,8 MiB contra 53,6 depois de tirá-lo, e a referência
chegava por shebangs de utilitários de linha de comando dentro do
`node_modules` que nada aqui executa. `disallowedReferences` o cobra.

#### O token do gerador NÃO é a mesma coisa que o token colado à mão

Um PO token não diz quem você é: diz que quem pede é um dispositivo de verdade.
Sem conta, o gerador o vincula a um *visitor id* criado na hora — então o
download anônimo continua anônimo e ganha a atestação que o YouTube passou a
exigir dos clientes padrão. Por isso o **gerador vale para todo download de
mídia**, e não só para os identificados: o anônimo é o caso comum e é
justamente o que quebra sem ele.

O arquivo `$XDG_STATE_HOME/ronoctua/po-token` é o oposto e continua atrás do
`t i`: alguém o colou, então ele está vinculado à sessão em que foi cunhado, e
mandá-lo num pedido anônimo prenderia essa sessão a ele. Uma linha por token, na
grafia `CLIENT.CONTEXT+TOKEN` do próprio yt-dlp — copiada sem tradução no meio,
porque um passo de tradução é um passo que pode estar errado.

As duas chaves de `--extractor-args` são diferentes de propósito
(`youtubepot-bgutilscript:` e `youtube:`): o yt-dlp indexa essa opção por
extrator, e duas para a MESMA chave se substituem.

#### O prefixo é a escolha, e não um interruptor

`LEADER t v` baixa o vídeo; `LEADER t i v` baixa o mesmo vídeo **como você**.
São teclas diferentes de propósito. Um interruptor guardado em outro lugar faria
pressionar uma tecla e não saber qual das duas aconteceu — e das duas, a que não
se desfaz é a identificada: um download anônimo sempre pode ser repetido como
você, e o contrário não, porque o pedido já levou quem estava pedindo.

Pela mesma razão a linha carrega isso (`job.identified` no WIT): a tela diz o
que aconteceu em vez de pedir ao leitor que lembre. Na lista é uma **cor** — a
coluna tem uma célula —, no detalhe é uma palavra.

Há exatamente **um** lugar em todo o sistema onde a identidade é um interruptor:
a extensão. A interceptação de um download que o navegador começou não tem
momento em que perguntar, então lá é um botão — mostrado no popup, logo acima
dos botões que ele governa, e as entradas do menu de contexto **dizem no título**
qual das duas farão.

#### De onde saem os cookies é decisão da MÁQUINA

`ronoctua.transfers.cookies` vale `browser`, `file` ou `none`, e o padrão segue
`ronoctua.browser.enable` — no perfil reduzido, que não tem navegador, é `none`.
Nada dos cookies entra no repositório: um pote de cookies é uma sessão, e
`nix/modules/secrets.nix` põe sessão na mesma frase que senha (invariante 13).

`browser` lê a base do próprio Chrome, e **só funciona por causa de uma decisão
que já estava tomada por outro motivo**: `nix/modules/browser.nix` força
`--password-store=basic`, e é exatamente esse o chaveiro que o yt-dlp chama de
`basictext`. Sem ele, decifrar a base no Linux pediria um chaveiro que esta
distro deliberadamente não tem.

`file` é a opção mais segura das duas, e a razão não é óbvia: o YouTube **gira**
os cookies que entrega, e um download que usa os do navegador em uso pode
invalidá-los — o que aparece como o navegador se deslogando sozinho. Quem
exportou um arquivo quis isso. Por isso, quando os dois estão configurados, o
arquivo ganha.

#### O aria2 não abre a base de um navegador

Um link identificado pelo motor de links precisa de um **arquivo**. E ele não
recebe `--load-cookies`: essa opção do aria2 vale para a sessão inteira e não
por download, então um link identificado faria todos os outros da fila levarem
os cookies da pessoa junto — identidade vazando para pedidos que ninguém pediu
para identificar. O que o aria2 aceita por download é um **cabeçalho**, então o
pote é lido aqui e vai exatamente o que pertence àquele endereço: domínio como
sufixo (`youtube.com` não casa `notyoutube.com`), caminho como prefixo, `secure`
só sob TLS, e expiração zero é sessão e não vencido.

#### Pedir para ser você sem ter de onde tirar não vira anônimo

Vira uma linha que falhou, com a frase. O contrário — cair calado para anônimo —
baixaria o trailer em vez do vídeo, ou nada, e o erro pareceria ser do site.

### O navegador manda para cá — e a extensão não deixa rastro

A pergunta do dono foi exata: uma extensão que só ele tem torna o navegador único se todo site
puder listá-la. As vias de detecção são conhecidas, e **todas elas são chaves de manifesto**:

1. **`web_accessible_resources`** — qualquer página faz `fetch('chrome-extension://<id>/x.png')`;
2. **`content_scripts`** — o que ele deixa no DOM é um marcador que a página lê;
3. **`externally_connectable`** — entrega `chrome.runtime` a uma origem;
4. **`host_permissions`** — o acesso permanente é o que torna o item 2 possível em toda parte;
5. **`declarativeNetRequest`** — a página infere pela própria requisição alterada.

O manifesto é **gerado** sem nenhuma das cinco, e `extension-has-no-fingerprint` reprova o build no
dia em que uma aparecer. O que sobra roda no *service worker*, onde JavaScript de página não
enxerga. A interceptação é `chrome.downloads.onCreated` mais `cancel`; o menu de mídia entrega ao
yt-dlp o endereço **da aba**, que é o que ele resolve, então o caso comum não injeta nada. A única
injeção é o "procurar mídia nesta página", sob `activeTab`, quando se aperta o botão — e ela lê e
volta, sem tocar em elemento, atributo ou estilo.

**O que NÃO é prometido:** isso remove a detecção passiva por página; não remove a inferência
comportamental — um download que a página provocou e que nunca aparece é um sinal. Isso não é
consertável de dentro de uma extensão.

#### Cancelar depois de perguntar, e não antes

`onCreated` pergunta à fila ANTES de cancelar. Se a fila não estiver no ar, o navegador fica com o
download: cancelar primeiro e perguntar depois perderia o arquivo toda vez que a unidade estivesse
fora, que é o único desfecho pior do que não interceptar.

#### E a porta do navegador só sabe ADICIONAR

O host de native messaging aceita quatro mensagens e **nenhuma delas nomeia um id de trabalho**: não
há como pausar, cancelar, listar ou limpar por ali. Uma página que se apoderasse da extensão não
teria mensagem para cancelar uma cópia de quarenta gibibytes, porque ela não existe.

#### Instalar fora da Web Store, sem chave no repositório

O Chrome não carrega uma extensão de um diretório sem alguém ligar o modo de desenvolvedor e clicar,
o que é imperativo e o invariante 6 recusa. O que ele faz sem ninguém clicar é honrar uma política em
`/etc/opt/chrome/policies/managed/` — mas uma extensão *force-installed* precisa de um `.crx`
assinado e de um id estável, e o id vem da chave de assinatura.

E a chave não pode estar aqui: `nix/modules/secrets.nix` é explícito ao listar "chave SSH privada" na
mesma frase que senha e token. Então a unidade declarada em `nix/modules/browser-extension.nix` gera
a chave **na máquina** no primeiro boot, empacota o `.crx` que o flake construiu, deriva o id dessa
chave e escreve os dois arquivos que o Chrome lê — a política e o manifesto do host de native
messaging, cujo `allowed_origins` precisa do MESMO id, e é por isso que uma unidade só escreve os
dois. É o mesmo arranjo do segredo do Wi-Fi: a unidade é declarada, o que ela produz é estado da
máquina.

O formato CRX3 está documentado em lugar nenhum conveniente, e o `nix/scripts/pack-crx.py` o escreve
à mão: `"Cr24"`, versão, um cabeçalho protobuf com a chave pública e a assinatura, e o zip. A
assinatura cobre `"CRX3 SignedData\0"` mais o tamanho, o `signed_header_data` e o zip — e o id são
os dezesseis primeiros bytes do SHA-256 da chave pública DER, escritos no alfabeto `a`–`p` do
próprio Chrome. O zip é reprodutível (data fixa, caminhada ordenada): sem isso o mesmo diretório
empacota em dois arquivos diferentes, a assinatura muda, e cada rebuild vira uma extensão diferente.

#### E o app não depende dela

Tudo o que a extensão faz tem tecla: `t a` para um link (ou magnet, ou
`.torrent`), `t v`, `t u` e `t l` para vídeo, áudio e legenda de uma página, e
`t i` na frente de qualquer um dos quatro para ir como você. O que a extensão
tem de verdade que o teclado não tem é **interceptar** um download que o
navegador começou — que por natureza acontece dentro do navegador — e **olhar a
página** atrás de mídia, que serve só para os sites que o yt-dlp não conhece.

Isso não é uma promessa escrita: `downloads-do-not-need-the-extension` lê os
dois lados — o `want:` de cada botão do popup e a tabela de teclas do leader — e
exige que cada coisa que a extensão oferece tenha tecla, duas vezes: a anônima e
a identificada. Uma entrada nova no popup sem a tecla correspondente derruba o
build.

**O que não é afirmado aqui** é que uma build específica do Chrome honra um `update_url` `file://`.
O `tests/extension-vm.nix` confere o que é conferível — a chave existe, o `.crx` está assinado, o id
da política é o id do `.crx`, e a lista do host nomeia exatamente ele — e o `docs/PENDING.md` registra
que o último passo é do dono, num navegador de verdade.


### A barra lateral — de onde se parte, e o que ela custou descobrir

O gerenciador tinha uma vista só. Para chegar a `~/main/temp` digitava-se o caminho; um pendrive
montado não aparecia em lugar nenhum, e um desmontado não existia para o sistema inteiro — não
havia `udisks2`, nem regra udev, nem `boot.supportedFilesystems` em parte alguma do repositório.

#### O gerenciador são QUATRO seções, e cada uma responde por si

A barra lateral, a listagem, a prévia e os fatos são **seções** (`files::Section`), e cada uma diz
três coisas sobre si: **onde está** (`from`..`to` em células, `top` por `lines`), **o que tem**
(`content`, já no deslocamento dela) e **quanto tem** (`total`). Quem desenha percorre as seções e
põe cada uma no seu trilho; quem resolve um ponto (`View::hit`) acha a seção que o contém, depois a
linha DAQUELA seção, e só então o campo — entre os campos daquela seção. As barras de rolagem, a
roda e o arrasto perguntam à seção. Nada mais decide.

**O chrome é da LISTAGEM.** O caminho e o cabeçalho são duas linhas de uma coluna só: ficam no topo
dela, não rolam, e as outras três seções começam na primeira linha da região. `View::chrome_rows` é
o único lugar que os constrói, e as duas portas o pedem de lá.

**A faixa de baixo não é da região.** Quando os painéis não cabem ao lado da listagem eles descem, e
lá têm linhas PRÓPRIAS: `View::row_count` não as conta e as duas portas as desenham por
`View::strip_rows`. Somá-las à altura da região fazia a contagem e o desenho discordarem — que é a
divergência que este arranjo existe para impedir.

**E `take` é TETO, nunca contagem.** `usize::MAX` quer dizer "todas"; uma função que MATERIALIZA
linhas tem de limitar ao que existe, senão a sentinela vira pedido de 2⁶⁴ linhas vazias. Custou seis
sessões mortas pelo `systemd-oomd` num só dia — ele mata o cgroup inteiro da janela, e o agente vai
junto sem deixar rastro.

**O que muda em `Layout` é um campo só.** `layout_beside(width, from)` resolve as colunas contra
`width - from` — a largura da LISTAGEM — e só então desloca; resolver contra a largura inteira e
deslocar depois deixaria a última coluna cair fora da borda, que é a única coisa que nunca cede.
E `Layout.from` é onde a marca vai. **Deixá-la na célula zero foi o defeito que o teste pegou**: ela
fica embaixo da última letra de um lugar da barra, e clicar nela marca outra coisa.

**Um segundo, do mesmo tipo, e pior**: o `header` continuava chamando o layout sem a barra. Os nomes
das colunas apareceriam sobre a barra, nomeando colunas que estavam em outro lugar, e clicar num
ordenaria pela coluna que por acaso estivesse embaixo da palavra. Nenhum dos dois seria visto por um
teste que só contasse linhas: os dois são sobre QUAL célula, e é por isso que as perguntas de
geometria olham a célula e não a linha.

#### Uma fonte para as dez pastas, e a que faltava

Os quatro padrões pedidos eram `~/main`, `~/main/temp`, `~/main/prj` e uma pasta de capturas. Os três
primeiros **já eram da distro**: `xdg.userDirs` os define, e `projects` é `main/prj`, no singular.
Escrevê-los de novo numa lista da barra seria um segundo lugar dizendo onde ficam os projetos, e no
dia em que um mudasse nada avisaria que o outro não mudou.

Então `nix/user-dirs.nix` passou a ser a tabela — nomes RELATIVOS à home, pelo mesmo motivo que
`nix/files-start.nix` já dava: um caminho absoluto nomearia a máquina de alguém, e o repositório é
público. `nix/modules/home.nix` monta o `xdg.userDirs` dela, e `nix/files-sidebar.nix` nomeia quais
entram na barra **por chave**, nunca por caminho.

**A quarta pasta não existia, e as capturas estavam caindo fora da distro.** Nada escrevia
`screenshot-path`, então o niri usava o padrão dele —
`~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png` —, um caminho que contradiz o próprio
`XDG_PICTURES_DIR` desta distro (`main/media/image`) e que nada criava. Uma barra apontando para uma
pasta de capturas apontaria para o vazio para sempre. Hoje `screenshot` é a décima entrada da tabela,
o niri escreve nela, o `xdg.userDirs` a CRIA (o `createDirectories` cobre o `extraConfig`), e
`screenshots-land-in-a-declared-directory` compara as duas pontas — porque o defeito não era nenhuma
das duas estar errada, era as duas poderem divergir.

#### Ocultar não é remover, e as duas frases são diferentes

Uma linha que a distro manda vem do Nix e voltaria no próximo start. "Removê-la" seria uma promessa
que o reinício quebra — a classe de falha calada que este projeto recusa em todo o resto. Então ela é
**ocultada**, fica no arquivo, e o seletor de ocultas a traz de volta; o que a pessoa acrescentou é
**removido** de verdade. `Gone` diz qual dos dois aconteceu, porque a frase na tela difere.

**E o `add` tem de olhar as ocultas PRIMEIRO.** Uma linha oculta não está em `rows` (o `assemble` a
tira) nem em `added` (um padrão nunca esteve), então uma verificação nesses dois diz "é nova" e
empurra uma segunda cópia — que o `assemble` volta a ocultar sob o mesmo ajuste. A linha seria
acrescentada, em duplicata, e continuaria fora da tela.

#### `cargo test` escreveu na configuração de quem o rodou

Este é o defeito mais caro da barra, e ele chegou à tela do dono: o gerenciador
abriu **sem barra nenhuma**, numa build que a tinha compilada.

A persistência achava o `$XDG_CONFIG_HOME` sozinha. Os testes da vista apertam a
tecla que esconde a barra e a que fixa um arquivo — porque é assim que se prova
que uma tecla faz algo —, e cada uma delas chamava `save_sidebar`. O resultado
foi o `~/.config/ronoctua/files-sidebar.json` REAL com um caminho de `/tmp`
fixado e `showing: false`.

**A regra que isso quebrou já estava escrita neste crate, e com o motivo certo.**
`trash::Bin` recebe a raiz como parâmetro porque *"dois testes ajustando a mesma
variável de ambiente disputam, e o que perde põe um arquivo na lixeira do
outro"*; `who::Roster` recebe o caminho do `passwd`; `when::Zone`, o do
`localtime`. A barra foi a primeira coisa do crate a ler o ambiente por conta
própria.

O conserto não foi documentar: foi **apagar as duas funções que achavam a home**.
`store::read()` e `store::write()` não existem mais — só `read_from(dir)` e
`write_to(dir)`, e o `Sidebar` carrega o diretório que recebeu, ou `None`. Uma
barra sem destino escreve nada e não tem `stamp`; é isso que todo teste constrói,
e `a_sidebar_with_no_store_writes_absolutely_nothing` prova que a pasta continua
vazia depois de esconder e fixar. Quem chama o `config_dir()` é o APLICATIVO,
uma vez, e entrega a resposta.

O sintoma merece registro à parte, porque é o que torna a classe cara: **nada
falhou**. Os testes passaram, os checks passaram, a build tinha a
funcionalidade, e a única coisa errada estava num arquivo de configuração que
ninguém pensa em olhar.

#### O `HintIgnore` do udisks colidiu com o pedido, e a saída já existia

O pedido nomeava `/boot` e as duas partições do pendrive. A medição diz que o udisks marca as DUAS
partições EFI — a `/boot` pelo GUID do GPT, e a de 4 MiB do pendrive pelo tipo `0xef` do MBR — com
`HintIgnore`. Honrar o hint contraria o pedido; ignorá-lo põe uma ESP de 4 MiB na barra de alguém.

A saída não foi um mecanismo novo: `HintIgnore` virou `Volume.quiet`, e `quiet` anda sob o gesto que
já governa os dotfiles. *"Escondido pela única regra que o Unix tem: o nome começa com ponto"* tem
aqui o gêmeo exato — **discreto pela única regra que o udisks tem** —, e é o `C-h` que já existia.

#### Nenhuma regra polkit nossa, e isso inverte o padrão do repositório

`files.nix`, `rescue.nix` e `memory.nix` escrevem uma regra estreita **porque a unidade é NOSSA** e
mais ninguém a autoriza. O udisks não é nosso: ele traz ações que já separam removível de sistema
pelo `HintSystem` — `filesystem-mount` é `allow_active: yes`, `filesystem-mount-system` é
`auth_admin_keep`. Uma regra nossa só poderia ALARGAR isso, e seria um segundo lugar para manter em
dia com ids de ação de terceiros, que o polkit interpreta em runtime — id defasado é silêncio.
Por isso o check é o INVERSO do `files-polkit-is-narrow`, e ele confere as duas pontas: que o udisks
está ligado, que não escrevemos regra nenhuma, e que **as quatro regras que devem existir continuam
existindo** — sem essa última ele passaria numa árvore onde todas foram apagadas.

**E não há agente polkit na sessão gráfica.** O único da distro é o `pkttyagent`, no console de
resgate; o `pkexec` traz o dele para dentro do terminal que abre. Então uma ação `auth_admin_keep`
não tem a quem perguntar, e sem `auth.no_user_interaction` o polkit ESPERA um agente que nunca vai se
registrar: a linha é apertada e não acontece nada, para sempre. Com a opção, a recusa volta na hora e
tem NOME, que é o que a transforma numa frase. Volume de sistema, por isso, não oferece montar —
oferece o terminal root que o gerenciador já tem, que é como este repositório já entrega trabalho
privilegiado.

#### 212 MiB de Python para um sistema de arquivos que esta distro não usa

O fecho do `udisks2` é **538,3 MiB**, e `nix why-depends --all` dá um caminho só até o maior pedaço:
`udisks → xfsprogs-bin (305,4 MiB) → python3-env (258,0 MiB)`, que é o `xfs_scrub`. Aritmética de
conjuntos sobre o fecho diz que 18 caminhos e 212 MiB são alcançáveis por ele e por mais ninguém —
contra 40 MiB do `libblockdev` e menos de 1 MiB de cada um dos outros. **Tirar só o `xfsprogs` é
quase todo o ganho**, e mexer no resto custaria uma decisão a defender por quase nada.

Medido depois do `override`: **343,2 MiB**, zero `xfsprogs`, zero Python, e `exfat`, `ntfs3g`,
`dosfstools`, `e2fsprogs`, `btrfs-progs` e `util-linux` todos no `PATH` forçado do `udisksd`. É de
graça porque **montar XFS é operação do kernel**: o que sai com ele é verificar, reparar,
redimensionar e escrever XFS, verbos que nenhuma tela tem — e `disks-tools-are-pinned` recusa,
**contra o código**, o dia em que um deles for acrescentado ao proxy.

Um deles saiu dessa lista depois, e a saída é decisão e não afrouxamento: **`format` existe**, porque
escrever um sistema de arquivos passou a ser coisa que esta distro faz. O que continua proibido é
`check`, `repair`, `resize`, `set_label` e `take_ownership` — os verbos cujo ajudante o override
levou embora, e oferecer um que silenciosamente não funciona para XFS é exatamente a falha calada que
o check existe para pegar. O mesmo check passou a cobrar o INVERSO também: que `Recipe::filesystem`
nomeie os cinco que este build escreve e nenhum outro, porque a tela DIZ que o XFS não está aqui — e
uma receita chamada `xfs` faria daquela linha uma mentira.

O `boot.supportedFilesystems` que o módulo liga quase não faz nada, e o módulo DIZ isso: o
`force-path.patch` do nixpkgs faz o `udisksd` sobrescrever o próprio `PATH`, então o
`system.fsPackages` nunca chega nele. A linha fica pela outra porta, que é real — um `mount` digitado
por uma pessoa no terminal usa o `PATH` do sistema —, e custa zero de fecho.

#### Sinal, e não relógio

Plugar um pendrive é um EVENTO, com a mão de alguém atrás. O `stale()` do gerenciador faz um `stat`
por segundo porque mtime é *estado*; isto não é. E duas razões decidem sozinhas: o relógio do rodapé
**só dispara com o gerenciador aberto** — plugar antes de abrir é o caso comum, não a borda — e a
janela **não tem relógio nenhum**, só o Wayland e o canal de `Report`.

A thread manda o `Rack` INTEIRO, e não um cutucão: plugar um pendrive emite quatro
`InterfacesAdded` em milissegundos, e um cutucão faria o laço que desenha dar quatro
`GetManagedObjects`. Assim o barramento só é tocado nessa thread, e a última mensagem vence. E chega
às DUAS portas sem fonte de evento nova nenhuma, porque as duas já carregavam `files::Report`.

#### E a pasta também dá notícia — `ronoctua_files::notice`

A frase acima continua verdadeira e ficou pela metade: mtime é estado, **e não é o estado inteiro**.
Um arquivo REESCRITO no lugar não mexe no mtime da PASTA, e o `vfat` de um pendrive carimba em
baldes de dois segundos — nos dois casos o `stat` responde "nada mudou" para sempre, e as colunas de
tamanho e data mentem enquanto a vista estiver aberta. Por isso a resposta ao aviso é um `reread()`
seco: **perguntar `stale()` depois do kernel ter falado é deixar o oráculo pior derrubar o melhor**,
e derrubaria exatamente a notícia que o aviso carrega.

O nome é `notice` porque `watch` já é do harness de pausa/cancelamento das cópias longas. Uma
instância de inotify por porta, **um watch na pasta ABERTA e nunca recursivo** (é assim que o
`max_user_watches` não é o limite), mais um na pasta em que a barra lateral persiste. `MODIFY` fica
DE FORA de propósito — dispara a cada `write` e um log crescendo viraria mangueira; `CLOSE_WRITE` é
o momento que uma listagem quer, e `ATTRIB` entra porque as colunas de permissão e dono mudam sem a
pasta mudar.

Todas as chamadas de inotify moram na thread do observador: a porta manda o caminho por `mpsc` e
toca um `pipe`, que é um dos dois descritores do `poll`. Assim não há corrida sobre a qual watch um
evento enfileirado pertencia — as ordens são tomadas ANTES de drenar, e evento sem dono é
descartado. Largar o `Notice` fecha a ponta de escrita, o `poll` volta com `HUP` e a thread sai:
encerrar não custa nenhuma syscall que possa falhar, que é o que um `Drop` quer.

A rajada é absorvida **na thread**, onde dormir é de graça: a primeira mudança sai NA HORA (segurá-la
faria um gerenciador que só debouncing parecer um gerenciador pensando) e o resto vem numa segunda
mensagem, no máximo uma vez por segundo. Sem isso, copiar N arquivos para dentro da pasta aberta
custaria um `reread()` por arquivo — e `reread()` é um `readdir` mais um `lstat` e uma consulta de
MIME POR ENTRADA, ou seja, O(N²) no gesto em que a listagem é maior.

O rodapé **fica com o `stat` de um segundo**, e não é redundância: o inotify vê o que ESTA máquina
faz, e um arquivo posto por outra máquina em NFS, sshfs ou SMB não emite nada — só o mtime se mexe.
O relógio já estava sendo pago pelo piscar, pela pizza e pela contagem do cérebro, então o piso custa
um `stat`. A janela **não ganhou relógio nenhum**: em repouso ela não acorda uma vez sequer, que é
o que faz várias janelas abertas não custarem nada. O único caso em que ela arma um Timer é o
observador não ter subido — aí o silêncio seria o defeito relatado voltando sem nada dizer.

Fechar o gerenciador **descansa** o watch em vez de matar a thread (`Notice::rest`), e os DOIS
lugares em que a vista some chamam por isso — o segundo é fácil de esquecer: perder o foco do
teclado derruba o gerenciador sem ninguém ter fechado nada.

#### O `ronoctua-files` não conhece o `ronoctua-disks`, e é de propósito

A barra modela o que alguém CONFIGUROU — pasta, arquivo, aplicativo, lixeira. Um dispositivo é
descoberto, não configurado. Modelá-lo ali faria o `ronoctua-files` depender do `ronoctua-disks`, e
esse crate constrói o `ronoctua-files-root`: seria um cliente D-Bus dentro de um binário que roda
como root, e uma segunda fronteira num crate cujo motivo de existir é ter UMA. O hospedeiro tem os
dois e emenda as linhas por `devices_are`.

### A prévia e os fatos — dois painéis, e por que eles custaram tão pouco

Nome, tamanho, modo e data: nenhuma das quatro colunas responde *é esta a foto?*, que é a pergunta
que se leva a um gerenciador de arquivos quando se abre um. `C-v` acende a prévia e `C-i` acende os
fatos — um desenha o arquivo, o outro lista o que ele carrega dentro.

#### Isto não custou uma primitiva nova, e a razão foi escrita antes

O `files_window.rs` abre dizendo por que a janela **não é um componente wasm**: o `world` do WIT não
importa sistema de arquivos (invariante 11, e é estrutural), o `input` não tem hover, nem botão
direito, nem roda, e um diretório inteiro atravessaria por valor a cada mudança. O argumento foi
escrito para recusar um gerenciador wasm, e é ele que faz esta seção ser barata: prévia e fatos são
pixels e linhas de texto dentro de código nativo. **Zero mudança no WIT, zero primitiva nova na
display list, zero buffer recompilado.** Do outro lado da fronteira, uma miniatura teria custado uma
capacidade, um comando novo e todos os seis buffers recompilados por causa do `match` exaustivo.

#### O crate é `ronoctua-inspect`, e ele NÃO é um módulo do `ronoctua-files`

Parece um módulo — lê arquivo, e o `ronoctua-files` é a fronteira com o disco. Mas esse crate
constrói o `ronoctua-files-root`, o binário que roda como **root**, e o `nix/rust.nix` o EXTRAI
(`rust.filesRoot`) exatamente para que o que roda como root não carregue o desktop junto: o
`files-vm` recusa `wgpu`, `wasmtime`, `mesa` e a fonte na closure dele. Pôr `image` e `resvg` ali
seria pôr decodificador de formato estrangeiro dentro de um binário privilegiado, e a armadilha do
`CLAUDE.md` diz por quê — **uma derivação referencia o que os BYTES dela citam**. O crate separado
não é organização: é o que mantém aquela extração possível.

Por isso a seta aponta ao contrário do que se imaginaria: o `ronoctua-inspect` depende do
`ronoctua-files` — é de lá que vem a base de MIME —, e o `ronoctua-files` não sabe que ele existe.
Quem tem os dois é o hospedeiro, exatamente como já tinha o `ronoctua-disks`.

#### A geometria é a da barra lateral de novo, e a aritmética está escrita no token

`space.files.panelsFrom` é **96**, e o comentário ao lado do número mostra a conta. O piso de um
painel é `Dimensions  3024 x 4032` — o fato comum mais largo, vinte e três células — mais o ar à
direita, vinte e quatro; dois painéis, com a régua que separa cada um do que está à sua esquerda,
são `24 + 1 + 24 + 1 = 50`. Em 96 sobram 46, que é exatamente o que a listagem quer para manter
ícone, nome, tamanho e data: `1 + (2+1) + (12+1) + (10+1) + (16+1) + 1`. Uma célula mais estreita, e
a data sai.

E as duas fases são as de sempre: 96 é **preferência**; o que não cede é "nada vaza da tela"; e
quando não cabe, `right_columns` retira o par **inteiro**, nunca metade de um painel. Meio painel é
a coluna cortada que a régua da direita já tinha ensinado a não desenhar.

**Um lugar decide as quatro regiões.** O `View::split` já era "o único lugar onde o corte é
calculado", e o motivo estava escrito nele: os dois pintores e os dois hit-testers perguntavam a
`sidebar_columns` cada um por sua conta, e num segundo monitor as linhas eram dispostas com a largura
da PRIMEIRA tela e a régua com a da tela que estava sendo pintada — numa mais estreita, a linha e os
campos caíam em células diferentes. Com quatro regiões, `split` virou uma chamada fina para
`View::panes(width)` em vez de ganhar irmãs ao lado, que seriam três chances a mais de discordar.

**E os painéis são seções, como a barra lateral e a listagem.** Cada um diz onde começa e onde
termina em células, quantas linhas ocupa e a partir de qual deslocamento; quem desenha percorre as
seções e quem resolve um ponto acha a seção primeiro. A fronteira não é aritmética espalhada: é o
`from`..`to` da seção, num lugar só.

**Os dois viram juntos: colunas à direita, ou faixa embaixo, nunca um de cada.** A frase do dono é
binária, e há um segundo motivo que decide sozinho — um modo misto deixaria a listagem mais CURTA e
mais ESTREITA ao mesmo tempo, que é pagar as duas contas para ver metade do que se pediu.

#### Cinco programas externos, e só um faz pixel — a escolha foi medida

`Tool::ALL` tem cinco, e cada um faz o que nenhum outro faz: o `ffprobe` responde o que o arquivo É,
o `exiftool` lê o Exif, o `pdftoppm` e o `pdfinfo` são do PDF, e o `ffmpeg` é o único que faz PIXEL
sair de um arquivo que o crate `image` não decodifica — quadro de vídeo, imagem de dentro de
contêiner, capa de faixa de áudio.

A alternativa óbvia foi tentada nesta máquina, e o resultado é o argumento inteiro: o `magick`
**DESPEJOU O CORE** num DNG de 17 MB, enquanto o `ffmpeg` deu um PNG de 400 px do MESMO arquivo em
**0,21 s**; um HEIC em **0,09 s**; e um quadro de um mp4 de 355 MB em **0,17 s**. Um segundo
conversor não seria redundância: seria um segundo modo de falhar para formatos que o primeiro já lê,
e a pergunta "por que esta prévia não saiu?" passaria a ter duas respostas possíveis.

**Toda porta externa devolve a figura no stdout**, e quem a decodifica é o crate `image` — o `ffmpeg`
recebe `-f image2 -c:v png -`, o `pdftoppm` recebe `-png` e `-`, e o caminho do JPEG embutido
devolve o JPEG que a câmera gravou. Nenhuma ferramenta externa decide arranjo de pixel, e o `Rgba`
nasce num lugar só — é isso que mantém os tetos num lugar só também: a paciência de 5 s e o
`STDOUT_CEILING` de 32 MiB valem para as cinco portas, e não para cada chamada por sua conta.

#### Três defeitos que apareceram OLHANDO as fotografias do dono

Nenhum teste teria pego os três, e não por descuido: os três são sobre o que a tela mostra, e todos
os três passam por código que devolve os pixels sem reclamar de nada.

1. **O `.dng` era recusado na porta.** O `mime::examined` fareja um DNG como `image/tiff` — e está
   certo, um DNG É um TIFF, é assim que o formato é feito. Só que o nome diz `image/x-adobe-dng`, e
   mandar o arquivo ao decodificador de TIFF o faz ler os planos RAW e falhar. Quem resolve é o
   `settled()`: quando o nome DESCENDE do farejado, os dois concordam e o nome é apenas a resposta
   mais fina. O farejado só vence quando os dois discordam de verdade — um PNG chamado `.txt`.
2. **A prévia do DNG saía quase preta.** O `ffmpeg` decodifica os planos lineares do RAW sem curva de
   tom nenhuma. O que a tela mostra hoje é o JPEG que a própria câmera gravou dentro do arquivo: é
   por isso que `How::Embedded` existe, e é por isso que ele é do `exiftool` e não do `ffmpeg`.
3. **Retrato saía deitado.** O crate `image` **não** aplica a orientação Exif. O `upright` lê
   `decoder.orientation()` ANTES de o decodificador ser consumido e gira a imagem; e o caminho do
   JPEG embutido precisa de uma segunda leitura, porque o JPEG que a câmera guardou não leva a
   etiqueta que o DNG em volta dele leva.

E um quarto, nos fatos: **o `ffprobe` responde a um JPEG com `25 fps` e `0,04 s`**, mais uma taxa de
bits calculada em cima desses dois. É o demuxer `image2` inventando um filme de um quadro — todos
aqueles números são fatos sobre COMO o ffmpeg abriu o arquivo, e nenhum é fato sobre o arquivo. Uma
ficha dizendo "25 fps" embaixo de uma fotografia estaria mentindo com cara de quem sabe. O
`pretend_film()` conhece o `image2` e tudo que termina em `_pipe`; um GIF animado é aberto pelo
demuxer `gif` de verdade e mantém os seus 3,96 s e os seus 11 fps, que são dele.

#### Uma recusa é um VALOR, e não uma ausência

`Made` carrega `Option<Rgba>` **e** `Option<Refusal>`. Um painel em branco e um painel que recusou são
idênticos na tela, e só um dos dois tem frase para dizer — sem o segundo campo, "este arquivo não
tem figura", "falta o programa", "é grande demais" e "demorou" sairiam todos como o mesmo retângulo
vazio. São oito (`Nothing`, `NotText`, `Empty`, `Unreadable`, `Missing`, `TooBig`, `Failed`, `TimedOut`),
cada uma com a sua chave, e a `Missing` carrega o NOME do programa que falta — que é a diferença
entre uma tela inerte e uma linha que diz o que fazer.

A primeira delas se chamava `NoPicture` e dizia *"este tipo de arquivo não tem imagem dentro"*. O
dono a reportou da tela, contra um markdown, e a frase estava errada por dentro: **este painel não é
um painel de imagem** — ele responde o que tem DENTRO de um arquivo, e um arquivo de letras está
cheio de conteúdo. Hoje ela se chama `Nothing`, vale para o que não tem nem figura nem texto, e sua
chave é a MESMA que uma pasta usa: de onde alguém olha, uma pasta e um `.zip` são o mesmo fato.

#### A prévia mostra TEXTO, e é por isso que a frase sobre imagem morreu

O dono selecionou um markdown e o painel disse *"there is no picture inside this kind of file"*.
A frase era verdadeira e a pergunta era outra: **este painel não é um painel de imagem**. Ele
responde o que tem dentro de um arquivo, e a maioria dos arquivos de uma máquina de desenvolvedor
tem letras dentro. Consertar só a frase teria sido trocar uma mentira por um silêncio.

##### "É texto?" é UMA pergunta, e quem responde é a base do freedesktop

`is_a(mime, "text/plain")`, e nada mais. Sem cláusula `starts_with("text/")`: todo `text/*` já
descende de `text/plain`, e uma segunda regra escrita à mão é uma segunda resposta a uma pergunta que
a base resolve.

Ela alcança mais do que parece — **277 tipos** chegam a `text/plain` pelo DAG, incluindo
`application/json` (na profundidade três), `application/xml`, `application/x-shellscript`,
`application/toml`, `application/yaml`, `application/x-desktop` e `application/sql`. Nenhum precisa
ser nomeado.

E os quatro descendentes que NÃO são texto — `image/svg+xml`, `image/x-eps`, `audio/x-mpegurl`,
`video/vnd.mpegurl` — também não precisam de lista de exceção, porque **a figura é pedida primeiro**:
as duas imagens ganham uma, e um `.m3u` genuinamente é texto.

##### Três tetos, e cada um tem o argumento certo

- **1 MiB de bytes.** É um trinta e dois avos do `STDOUT_CEILING`: um caminho que não gera processo
  nenhum não pode ter mais folga do que um processo filho tem. E teto declarado é limite; leitura sem
  teto é surpresa.
- **2 000 linhas, e o argumento é a BARRA.** O `Scroll` põe piso de uma linha na alça: a 2 000 linhas
  num painel de doze a alça é 8% da trilha, ainda agarrável; a 100 000 é um ponto, e um clique de
  roda anda três linhas em cem mil. Barra que não dá para usar é barra que mente sobre onde se está.
- **512 bytes por linha, e este é o que teria doído.** Um `.js` minificado é UMA linha de dois
  megabytes, e o `elide_end` percorre o argumento inteiro por cluster antes de cortar — aquela
  `String` seria percorrida **uma vez por frame**, dentro do frame que desenha.

##### Tudo o que é caro acontece UMA vez

Ler, quebrar, expandir tabulação e calcular um estado de realce por linha acontecem na thread do
inspetor e ficam no cache junto com os pixels. Não é otimização: o estado do realce ATRAVESSA linhas,
então colorir "só as visíveis" significaria rodar desde a linha zero de qualquer forma. O que sobra
para o frame é indexar uma linha e andar por meia dúzia de faixas.

A tabulação vira espaços **na leitura, antes do realce** — as faixas são deslocamentos em bytes no
texto final, e expandir depois moveria todas. A largura é token (`space.files.tabWidth`) e chega por
parâmetro, porque o `ronoctua-inspect` não recebe valor de token.

#### O que se mexe, se mexe na prévia

O dono pediu: *"quando o arquivo for animado, o preview deve mostrar a animação"* — e, ao ser
perguntado até onde ia, respondeu GIF **e vídeo**, com variedade. As duas metades são o mesmo
problema visto de dois lados, e a medição decidiu o desenho.

##### Os números que decidiram

No GIF do próprio dono: 44 quadros, 400×400, 90 ms cada, **707 ms** para compor todos e **26,9 MiB**
de RGBA. No vídeo dele (1080p60, 355 MB): trinta quadros escalados a 256 saíram em **139 ms** — o
ffmpeg decodifica ~215 quadros por segundo, muito acima do que a reprodução pede, e entrega RGBA cru
de tamanho exato.

Três conclusões caem daí, e nenhuma é opinião:

1. **Guardar o filme decodificado está fora.** 26,9 MiB por GIF, e o cache de textura inteiro são
   39 fatias de 1 MiB. Um quadro por vez é o que cabe.
2. **Uma textura por quadro está fora.** 44 quadros encheriam as 39 fatias com o filme de um
   arquivo e despejariam todos os ícones. É UMA textura, reenviada sob a mesma chave.
3. **PNG por quadro está fora.** Seria um encode no ffmpeg e um decode aqui, por quadro, para uma
   imagem que vive 90 ms. Cru não custa nenhum dos dois.

##### O quadro chegando É o relógio

Nenhuma das duas portas tinha timer rápido o bastante, e nenhuma ganhou um. O player espera a hora
DEVIDA do próprio quadro e o manda pelo canal `calloop` que as portas já escutam — então **o quadro
chegando é o que acorda o laço**, e quando o player para, as acordadas param com ele.

Um timer na porta teria de ser avisado de quando rodar e de quando não. Este não pode errar isso,
porque não há o que avisar.

##### A `era`, e por que um `Stop` no canal não bastava

O player é uma thread com `mpsc` na entrada, como o inspetor. Mas um `Stop` mandado enquanto o
ffmpeg está no meio de um quadro fica na fila até o quadro terminar — e "terminar" pode ser o fim de
um filme. Então o laço lê um **número**, e "parar" é incrementá-lo. A espera também é em passos de
20 ms pelo mesmo motivo: um GIF pode declarar segundos.

Pausar e retomar são um **sinalizador** e não uma mensagem, pelo mesmo argumento visto do outro
lado: a thread está DENTRO de uma leitura quando a ordem chega, então uma mensagem seria obedecida
no fim do filme. E o número passou a ser lido dentro da leitura também, não só entre quadros — é
disso que trata "o travamento vivo", mais abaixo.

É isso que garante que o filho morre quando o cursor sai, e é a razão de a thread possuir o processo
em vez de a porta.

##### O que o `ronoctua-inspect` responde, e o que ele não faz

Ele responde **se** um arquivo se mexe (`motion::moving`) e nada mais. A pergunta é da mesma
família que "o que tem dentro", e é barata: contar dois quadros de um GIF custa **41 ms** contra os
707 de compor os 44, porque o iterador do `image` é preguiçoso e dois quadros já respondem "mais de
um".

Rodar a animação não é dessa família: precisa de thread e de um processo vivo, e a promessa escrita
do crate é que ele não sobe thread nenhuma — a concorrência é do hospedeiro. O player mora no
`ronoctua-shell`.

Um GIF de um quadro só é uma FOTOGRAFIA, e é por isso que a contagem existe: subir thread e relógio
para uma imagem parada é custo sem nada do outro lado.

##### O teto de quadros saiu, e a frase que o justificava também (2026-08-30)

Havia um teto de seiscentos quadros, e o argumento escrito aqui era: *"uma prévia que toca um filme
no canto de um gerenciador de arquivos é um reprodutor de vídeo que ninguém pediu"*.

O dono pediu. As palavras dele: *"Eu gostaria que fosse uma pequena reprodução do vídeo, como um
miniplayer, com controles que aparecem no hover do mouse"*, *"quero que seja possível rodar o vídeo
inteiro"*, *"e quero que no controle tenha 'linha do tempo' para eu escolher a parte do vídeo"*.

O invariante 14 não mudou, e nada aqui é chrome nova: os controles são a última LINHA da própria
prévia, tirada do conteúdo, do mesmo jeito que a barra horizontal já tirava uma. O que estava datado
era o argumento — a mesma forma da reversão do menu de botão direito, que o `surface.rs` recusava
por escrito até o dia em que foi pedido.

O teto de cadência do vídeo (30) FICA, e o GIF continua andando no ritmo declarado dele: num painel
de 256 píxeis, sessenta quadros por segundo e trinta são a mesma imagem, e sessenta são o dobro das
acordadas num laptop. O GIF não tem teto porque um GIF em outra cadência **parece quebrado**, e a
cadência é o que o arquivo diz.

##### A imagem era enquadrada DUAS vezes, e é isso que deixava tanto ar em volta

O relato foi *"o vídeo pode ser maior e os espaços em branco muito menores"*, com foto.

O `pad` do ffmpeg letterboxa o filme dentro de um quadro cujo tamanho nós escolhemos, e os dois
lados desse quadro eram quantizados em passos de 128 INDEPENDENTEMENTE. Uma caixa de 380 por 300
virava um quadro de **384 por 128** — três para um —, então um filme 16:9 ganhava setenta e oito
colunas transparentes de cada lado, e a porta enquadrava esse retângulo de novo dentro da caixa,
acrescentando faixas em cima e embaixo. Duas molduras para uma imagem.

Agora a largura é quantizada e a **altura segue a forma da caixa**, então o `pad` É o ar que o painel
deixaria de qualquer jeito: uma moldura só, a que a forma do filme exige. Como o quadro já vem
enquadrado, a porta o desenha PREENCHENDO a caixa — `View::preview_fills` é quem diz isso, e a foto
parada, que não vem enquadrada, continua passando pelo `fit_into`.

O GIF é compostado no mesmo tamanho, com as barras transparentes, pelo mesmo motivo: as duas fontes
dão UMA forma de quadro, e as portas têm uma regra só.

Contas na mesma caixa, com um filme 16:9: antes a parte visível saía 163 por 92; agora sai 237 por
128.

##### O tamanho do quadro é NOSSO, e é o que consertou o defeito que ele fotografou

O relato foi *"parece que os frames foram girados na maioria dos vídeos; e ainda a amostra parece
ser muito pequena"*. Eram dois defeitos que se compunham.

O primeiro: `play_video` deduzia a ALTURA do quadro pelo número de bytes que por acaso chegaram num
`read()` de 64 KiB. Com largura 512 uma linha tem 2 048 bytes, e 65 536 são exatamente 32 delas — o
código declarava o quadro como 512×32 e passava a ler fatias horizontais do filme.

O segundo sobrevivia ao conserto do primeiro: as duas portas mediam a caixa da imagem pela FOTO
(`preview_pixels`) e desenhavam dentro dela o quadro que o player tivesse mandado. Duas formas
diferentes, então todo filme saía esticado nas proporções do próprio pôster.

O caminho óbvio — perguntar `width`/`height` ao ffprobe em vez de adivinhar — teria trocado um
defeito por outro, e isso foi MEDIDO: um vídeo de celular reporta 640×360 codificado com uma matriz
dizendo "gire 90°", e o ffmpeg renderiza **256×456**. Escalar para a forma reportada achataria a
imagem *e* dessincronizaria a contagem de bytes.

A resposta remove a classe inteira:

```
-vf scale=W:H:force_original_aspect_ratio=decrease,pad=W:H:-1:-1:color=#00000000
```

O que sai do pipe é **exatamente** `W×H`, qualquer que seja o arquivo — verificado contra o binário
real nas três formas (paisagem, retrato-por-rotação, retrato-por-codificação). `frame_bytes` virou
aritmética, e um quadro curto é DESCARTADO em vez de enviado. E `W`/`H` vêm da caixa que o painel
mediu, quantizada em passos de 128 px: arrastar a borda da janela custa no máximo sete respawns em
vez de sessenta por segundo.

A cor do letterbox é **transparente**, e não um token: a moldura seria uma de DUAS — o rodapé é
`surface.chrome` e a janela é `surface.buffer` — e a textura é uma só, desenhada nas duas portas.
Medido: as barras chegam com alfa 0 e a imagem com alfa 255, e o pipeline de imagem mistura com
`ALPHA_BLENDING`. Cada porta mostra o próprio fundo através delas, e nenhum valor de token atravessa
para o `ronoctua-inspect`.

##### O relógio é o de parede, e quem atrasa perde um quadro

O sono era fixo (`1000/fps`) e vinha DEPOIS da leitura, do envio e da pintura — então o período real
era isso mais o trabalho, o filme corria uns 17% devagar, e o erro ACUMULAVA: um minuto depois, dez
segundos atrás.

Agora o quadro `n` vence em `origin + n/fps`. Atrasado, o seguinte é **lido e descartado** — a
leitura é obrigatória, um pipe não se pula, mas o envio, o upload e a pintura, que são a metade
cara, não acontecem. Passando de trinta descartes seguidos o `origin` é reancorado: uma máquina que
não dá conta deve rodar o filme em câmera lenta, que é honesto, e não disparar por ele.

##### O transporte não são quatro mecanismos

| pedido | o que acontece | o que custa |
|---|---|---|
| pausar | o laço PARA DE LER; o ffmpeg trava num pipe cheio | nada, e 0% de CPU |
| retomar | volta a ler | nada |
| buscar | o filho morre e outro nasce em `-ss` | 108–242 ms, medido |
| som | o mesmo, com uma segunda saída | o mesmo |
| redimensionar | o mesmo, no `Fit` novo | o mesmo |

`SIGSTOP` era a alternativa óbvia e está errada: congela o dispositivo de áudio no meio do buffer, o
que estala. Travar num pipe é o mecanismo que o kernel já dá, e para o som no MESMO instante porque
o som sai do mesmo filho — `-map 0:a:0? -f pulse -buffer_duration 80 default`, uma segunda saída do
mesmo ffmpeg. Um segundo processo (`ffplay`) demuxaria e leria o arquivo duas vezes, e começaria com
o desencontro da própria latência de seek, sem nada capaz de corrigi-lo.

**Pedir som num filme MUDO mata a imagem**, e isso é medido: o ffmpeg recusa o comando INTEIRO
(*"output file does not contain any stream"*, saída 234, zero quadros). Por isso `Shape` carrega
`sound`, a tecla recusa e o glifo não é desenhado — e o respawn-em-silêncio depois de 400 ms existe
como guarda, não como caminho.

Um filme que ACABA precisa DIZER isso (`Beat::Ended`): antes os quadros simplesmente paravam de
chegar, o último ficava para sempre e o glifo de play nunca voltava. Ausência de acordada não é um
evento e não se desenha.

##### O travamento vivo que ninguém teria visto

`read_exact` é uma leitura **bloqueante sem prazo**, e a `era` só era checada *entre* quadros. Um
ffmpeg que abre o pipe e nunca escreve — um mount que travou, um dispositivo que sumiu — segurava a
thread do player pelo resto da sessão: a porta continuava pintando, `orders.recv()` nunca mais
rodava, e nada na tela dizia. Agora a leitura é `poll` com tique de 50 ms, e o prazo e a `era` são
perguntados a cada volta.

E o filho morre por uma **guarda de `Drop`**. Estava escrito à mão em quatro saídas, e a quinta que
alguém acrescentasse amanhã vazaria um ffmpeg lendo um filme que ninguém está olhando; um `Drop`
cobre `return`, `?`, `break` e pânico, e não sobra onde esquecer. Mais `process_group(0)`, o idioma
do `transfers/media.rs`, para o sinal alcançar o grupo.

##### Os controles ficam POR CIMA do filme (2026-08-30)

Eles ocupavam uma linha tirada da imagem, e o dono pediu o contrário: *"quero que todos os controles
do player apareçam por cima do vídeo/gif, de forma a deixar o vídeo/gif ocupando toda a altura
possível do preview, e com os controles com um background com semi-transparência, para que seja
possível visualizá-los em todas as condições de contraste"*.

Isso custou uma coisa no renderer, e vale dizer qual: **não há teste de profundidade**, e a ordem é
retângulos → imagens → texto. Uma faixa empurrada junto com os outros retângulos fica ATRÁS da
imagem e não se vê; desenhar a lista inteira duas vezes poria todos os fundos por cima dela. Então
quem vai em cima entra no FIM da lista e o chamador diz onde esses começam — `Quads::draw_range`,
um buffer, um pipeline, duas chamadas.

A faixa é `surface.transport`, e é a única cor do sistema com alfa. Não é elevação e não faz card
(invariante 15): não tem borda, raio nem sombra — é uma banda da largura do painel, de uma linha,
que escurece o que cobre, com o filme aparecendo através dela. `CC` é quatro quintos: abaixo de uns
dois terços os glifos brigam com um quadro claro, e opaca ela deixa de ler como parte da imagem e
vira uma tira de chrome atravessando o filme, que é do que trata o invariante 14.

A linha continua carregando uma `Row`, porque duas células são clicáveis; então `Section::line_of` e
`View::hit` as resolvem com a aritmética que já existe.

##### A linha dos controles tem campo de fundo SEMPRE, e sem isso ela foge da mão

O dono pediu que os controles se mantivessem *"com o hover nos próprios controles"*. Com eles
escondidos a `Row` ficava sem campo nenhum, `Row::field_at` respondia `None`, e portanto `View::hit`
respondia `None` para todo ponto daquela linha. O ponteiro indo buscar o play saía da prévia no
caminho, `pointing` deixava de ser `Preview`, e os controles sumiam de baixo da mão que ia pegá-los.

O campo de fundo é o mesmo que a imagem já usa para ser clicável. E há um teste que caminha TODAS as
linhas do painel, a dos controles inclusive, exigindo que cada uma resolva para a prévia.

##### O relógio é MEDIDO, e é isso que dá ar igual dos dois lados

*"Com espaços em branco idênticos na esquerda e na direita."* Antes, a folga da esquerda era uma
célula fixa e a da direita era o que sobrasse depois de uma reserva de catorze: `1:23 / 5:40` tem
onze células e `3:44 / 10:34` tem doze, então a folga mudava de filme para filme. Agora `TRANSPORT_AIR`
é uma constante só, aplicada dos dois lados, e a largura do relógio viaja na `Section` — porque três
lugares dependem dela: o retângulo do cursor e o arrasto de cada porta.

A trilha também subiu para o MEIO da linha, no nível dos glifos. Ela estava no pé, que é onde uma
barra de rolagem mora e uma linha do tempo não.

```
▶  🔇     ▁▁▁▁▁▁▁●▁▁▁▁▁▁▁▁▁▁▁▁▁   1:23 / 5:40
```

**As quatro células da esquerda são reservadas sempre, e só a tinta depende do ponteiro.** Se a
trilha crescesse quando os botões somem, o cursor de reprodução escorregaria de baixo do ponteiro no
instante do clique — a mesma classe de defeito que o comentário do `Scroll::offset_at` já registra.

A linha do tempo é um `Scroll` em decissegundos (`total` = duração, `offset` = posição): o
`bar_across` desenha o cursor e o `offset_at` é o inverso exato, então **onde é desenhado é onde é
agarrado**, a lei que as outras duas barras já mantêm. O que se agarra é a TRILHA e não o cursor,
porque `offset_at` mapeia qualquer ponto dela para uma posição — uma linha do tempo que só se pegasse
pelo cursor de uma célula seria um controle em que é preciso mirar.

**GIF não ganha linha do tempo**, e é a duração que traça essa linha, sem `if` a mais: os quadros de
um GIF são interdependentes, então buscar significa decodificar desde o zero — 707 ms no arquivo do
dono — e não há duração, só laço.

##### `C-w` dá a região inteira, e é DERIVADO

```rust
fn whole(&self) -> bool { self.theatre && self.preview_on && self.film_here() }
```

Não é um modo guardado. Assim ele se desfaz sozinho quando o cursor sai, quando `C-v` esconde a
prévia e quando o filme não produziu nada — e não há caso de que alguém tenha de lembrar de desfazer,
que é o caso esquecido que deixaria o gerenciador sem listagem. Um filme que ACABOU mantém a região,
de propósito: o último quadro e o glifo de play ainda estão lá, e apertar play é como se assiste de
novo.


##### Uma barra não desenha por cima do que ela serve para alcançar

O dono fotografou uma linha da prévia correndo por baixo do polegar, com as
últimas letras atrás dele. A causa é aritmética e valia para **toda** seção que
rola: a barra fica na ÚLTIMA célula do vão da seção, e os campos recebiam essa
célula também.

A listagem e a barra lateral nunca tiveram o defeito, e não por sorte — as duas
recuam as colunas por `EDGE_RIGHT`, então a barra delas cai no ar. Os painéis não
tinham reserva nenhuma. Agora eles tiram uma célula, **e só quando há barra**:
`cut` não muda enquanto se rola, então um arquivo curto não paga uma coluna por
uma barra que nunca vai crescer.

A primeira tentativa encolheu o `to` da seção, e isso moveu a BARRA em vez do
texto — ela é desenhada em `to - 1`, então o buraco apenas reabriu uma célula à
esquerda. O que encolhe é a sala que os CAMPOS recebem; a seção guarda o vão
inteiro, porque é dele que a barra sai.

##### E a linha longa tem barra também, porque metade de uma vista não é uma vista

A prévia rola para os lados por tecla desde o primeiro dia da feature, e o dono
reportou a falta assim que as teclas funcionaram: **não havia o que arrastar**.
Este sistema atende o teclado E o mouse, e uma vista que oferece um dos dois não
está pronta.

A barra de baixo é o MESMO `Scroll` com os eixos trocados — `total` é a linha
mais larga, `lines` é o que cabe, e `thumb` já era agnóstico ao eixo. O que
entrou no crate foi só onde pôr o retângulo (`Scroll::bar_across`), e o que
entrou na seção foi um `across: Option<Scroll>` que só a prévia carrega.

E ela também não desenha por cima: a seção que tem barra de baixo entrega uma
LINHA, do mesmo jeito que a que tem barra de lado entrega uma célula.

O arrasto passou a ter **eixo**. Antes era só o painel, o que bastava enquanto
cada seção tinha uma barra — com duas, um ponteiro segurando uma não está
segurando a outra.

##### A rolagem saiu de graça, e o que a liberou foram três remoções

A seção da prévia nascia com `offset: 0` e `total: 1`, e três lugares recusavam o gesto por escrito.
Trocado o `total` por `preview_total()`, **a barra e a roda apareceram sozinhas** — o
`Section::bar_box` devolve o retângulo assim que há mais linhas do que cabem, e o `section_at` já
sabia que célula é de que seção. É a primeira vez que o rearranjo em seções se paga.

E **a figura continua não rolando sem um `if` em lugar nenhum**: `preview_total()` devolve 1, o
`slide` calcula `last = 1 - room` como zero, e o gesto é engolido por aritmética em vez de por um
caso especial.

##### Um defeito que já estava lá, e que este trabalho abriria duas vezes

`self.cursor` é escrito em **dezesseis** sítios do `files.rs`, e só um deles — o `move_by` — zerava o
`info_offset`. Então **clicar** noutro arquivo mantinha a ficha rolada no fato oito do arquivo
anterior. Com um `preview_offset` o mesmo buraco abriria de novo, e pior: na linha trezentos do
arquivo anterior.

O conserto é causal e fica num lugar só: no `View::looked`, quando o CAMINHO muda. Cobre os dezesseis
sítios, toda troca de diretório e os dois painéis, em três linhas — e re-decodificar o MESMO arquivo
(arrastar a borda da janela pede de novo num `target` novo) mantém o lugar, porque jogar alguém de
volta à linha um enquanto lê é pior do que o arrasto.

##### E duas contas que, sem conserto, deixariam a feature sem servir

O `panel_lines` devolvia o teto entre a caixa da figura e as linhas da ficha. Para um arquivo de
texto a ficha é curta, então o rodapé daria a um markdown **quatro linhas** — e o pedido era poder
rolar "para ver boa parte do conteúdo". Agora a prévia pede o que tem a dizer, com teto de 64: um
número que sai daquela função é um número que alguém multiplica, e o `usize::MAX` como sentinela já
custou 14,6 GiB a este repositório uma vez.

O `drawn_lines` estava escrito dentro do `if` dos fatos, e as duas portas discordavam sobre em que
altura um gesto de roda rola. As duas eram defeitos antes desta feature — a segunda já era errada
para os fatos na faixa, doze linhas onde o painel tem quatro — e as duas apareceram porque a prévia
passou a fazer as mesmas perguntas.

#### O GPS fica guardado, e a linha diz que ele está lá

`Place::Withheld` é o padrão, e a linha do lugar continua aparecendo: ela diz que a coordenada está
ESCRITA dentro do arquivo e vai junto com ele para onde ele for. É esse o fato que alguém quer antes
de mandar uma fotografia para um estranho, e é ele que uma ficha que simplesmente omitisse o GPS
esconderia. Mostrar os números por padrão seria o inverso: pôr a casa de alguém na tela porque o
cursor passou por cima de um arquivo.

#### A chave do cache é uma versão, e não um tamanho

O `Stamp` é `(caminho, mtime, bytes, alvo)`, com o alvo arredondado para cima até 128, 256 ou 512.
Sem a quantização, arrastar a borda de uma janela redecodificaria a foto de celular a cada
`configure` — uma vez por largura intermediária, enquanto a mão estivesse mexendo. E as dimensões da
imagem não entram na chave de propósito: quem responde "mudou?" é `mtime` mais `bytes`, e uma imagem
reescrita com o mesmo tamanho em pixels teria a mesma chave se elas entrassem.

#### A porta de janela não tinha pipeline de imagem nenhum

Ela desenhava glifo e retângulo, e mais nada; era o rodapé que já desenhava as capturas do clipboard
e os pixmaps do localizador. Uma prévia que funcionasse numa das duas portas e não na outra é
exatamente a divergência que o arranjo de duas portas existe para impedir, então a janela ganhou o
`Images` do `ronoctua-render` e o mesmo quadro texturizado, na mesma superfície. Não conseguir
criá-lo custa a figura, e não a janela.

### A prévia 3D — onze formatos, e ela se deixa girar

A pergunta que se leva a um arquivo 3D não é *como é isso*, é *como é isso do outro lado*. Uma
imagem parada de um modelo responde metade; por isso a prévia de uma forma é a única do sistema que
o teclado e o mouse MOVEM.

#### São DOIS tipos de arquivo, e só um deles guarda triângulos

Nove formatos são MALHAS — STL, 3MF, OBJ, PLY, OFF, glTF, GLB, COLLADA e AMF —, e ler um é ler os
triângulos que estão escritos nele. Dois — STEP e IGES — são REPRESENTAÇÕES DE FRONTEIRA: guardam
superfícies e as curvas que as recortam, e nenhum triângulo. O arquivo STEP do dono tem 106 faces
sobre 42 planos, 32 cilindros, 18 toros e 14 cones.

O STEP é MALHADO (`mesh/brep.rs`, `mesh/surface.rs`, `mesh/patch.rs`). O IGES não — o recorte dele se
expressa por outras entidades e não há um `.iges` para provar contra, então ele continua sendo
desenhado pelas arestas.

**A primeira rodada desenhou os dois como arame, e o dono recusou na tela**: *"não está legal, o que
eu quero é ver o objeto por completo; com preenchimento"*. O que o arame tinha de bom fica: quando
uma face não sai, é ela sozinha que vira contorno.

#### Malhar uma face é levar o contorno dela ao espaço da superfície

Para cada `ADVANCED_FACE`: andar os `FACE_OUTER_BOUND`/`FACE_BOUND` até os `EDGE_LOOP` e os
`ORIENTED_EDGE`, amostrar cada `EDGE_CURVE`, inverter esses pontos para o `(u,v)` da superfície,
triangular o polígono plano que resulta (com furos), e mapear de volta. Todo vértice sai de
`Surface::at`, então **todo vértice fica exatamente sobre a superfície de verdade**.

**Cada aresta é amostrada UMA VEZ e guardada pela id da entidade.** Duas faces que se encontram
compartilham a aresta entre elas, e se cada uma amostrasse por si as duas discordariam nos últimos
bits e o sólido se abriria em toda emenda. A segunda face recebe os pontos da primeira, invertidos.

**A emenda desdobra ANDANDO.** Uma face cilíndrica que dá a volta inteira tem uma aresta em `u = 0`
que é também `u = 2π`, e a inversa não tem como saber qual das duas está olhando — `atan2` responde
numa volta e mais nada. Então o laço é percorrido acumulando, e um passo que pularia mais de meia
volta é tomado como tendo dado a volta.

#### Uma superfície sabe ir; só cinco sabem voltar

`Surface::at` — a ida — é exata e barata para as oito. A VOLTA é que é difícil: plano, cilindro,
cone, esfera e toro têm inversa em fórmula fechada, quinze linhas cada. Extrusão, revolução e NURBS
não têm nenhuma, e usam UMA inversa numérica genérica (Gauss-Newton sobre `|S(u,v) − P|²`) dirigida
pela ida — que é a parte fácil e testável.

**A semente vem do ponto ANTERIOR do contorno**, não de uma busca. Dois vizinhos num contorno são
vizinhos na superfície, então o passo é minúsculo e não há outro ramo perto o bastante para cair
nele — que é exatamente onde uma busca fresca responderia diferente para dois pontos vizinhos e
rasgaria o polígono. Só o primeiro ponto de cada laço paga uma grade.

E é isso que torna a metade cega confiável: **a inversa genérica é medida contra a fechada**, num
cilindro, num cone, numa esfera e num toro. Código escrito contra um formato que ninguém aqui pode
conferir fica medido contra código que pode.

**Toda resposta é reposta pela ida.** As fechadas também respondem sempre — a de uma esfera é um
`atan2` e um `asin`, e diante de um ponto longe dela devolve os parâmetros do ponto mais próximo sem
reclamar. Um contorno feito disso é um polígono que não é a face. Então toda resposta passa por
`Surface::at` de volta e tem de cair onde começou.

#### O refino é UNIFORME, e a versão que não era não funcionava

Ear clipping responde "quais triângulos", não "quantos", e numa face que curva ele deita um triângulo
por cima de um filete inteiro. Então o resultado é subdividido: cada triângulo em quatro, cada aresta
no meio, um número fixo de vezes.

Dividir só as arestas INTERNAS mantém o contorno onde as duas faces vizinhas concordam que ele está —
o que é estanque, e é o que se quer. Mas **o ear clipping produz LEQUES**, e a aresta mais longa de
uma lasca de leque é a do contorno. Recusar dividi-la e dividir a curta faz uma lasca mais fina e um
raio novo comprido, para sempre: medido, a pior aresta interna ficou entre 1,7 e 9,3 vezes o alvo com
qualquer número de passadas, enquanto a contagem de triângulos dobrava. Refino que não converge não é
refino.

**O preço é que duas faces deixam de se encontrar exatamente** — cada uma divide a corda que
compartilham no meio EM SEUS PRÓPRIOS parâmetros. O contorno chega amostrado a menos de seis graus por
arco, onde isso é cerca de um milésimo do raio: abaixo de um pixel no tamanho em que isso é desenhado.
O que não é é exatamente zero, e por isso **um B-Rep não relata volume, área nem "superfície
fechada"** — seriam números sobre uma malha um milésimo de milímetro aberta. E não relata triângulos
tampouco, que seriam sobre o malhador; o que o ARQUIVO diz que tem — sólidos, cascas, faces, arestas,
vértices — já está no painel.

#### A melhor orelha, e não a primeira que serve

Custa uma varredura inteira em vez de uma saída antecipada, e é o que torna o resultado usável. O
domínio de uma face cilíndrica é um retângulo comprido com os dois lados longos amostrados denso —
pegar a primeira orelha anda por um dos lados e produz triângulos que atravessam de ponta a ponta.
São esses que cortam uma corda pelo meio do cilindro, e o refino que depois tem de resgatá-los
multiplica a face INTEIRA: medido, com a primeira orelha a peça do dono saía com 359 mil triângulos.
A nota é a área sobre o maior lado ao quadrado.

#### Quanto refinar sai dos TRIÂNGULOS, não da face

A primeira versão perguntava quanto parâmetro a face inteira cobria, e errava pela largura do
contorno: uma face cilíndrica que dá a volta cobre uma volta, mas o aro dela já chega amostrado a
seis graus, então o ear clipping produz triângulos de seis graus e não de trezentos e sessenta. Isso
punha todo cilindro no teto de subdivisão.

E a tolerância é uma FRAÇÃO da peça, não um comprimento: o que "fino o bastante" quer dizer é um
PIXEL, e um pixel é a peça dividida pelo painel. Vinte e cinco microns absolutos numa peça de 220 mm
é dezessete vezes mais fino do que a tela mostra — custou 174 mil triângulos e 39 ms por quadro para
detalhe que ninguém vê.

**Medido, para 512×384**, depois das três correções: a peça do dono sai com 27 mil triângulos e
**18 ms**; o moon buggy (77 faces, só NURBS) com 69 mil e 15 ms; um eixo traseiro de 2 536 faces com
150 mil e 32 ms. O teto de 300 mil triângulos por PEÇA é dividido entre as faces, e é um limite de
QUADRO e não de memória.

#### Face que não sai vira contorno — ela sozinha

Um tipo de superfície que este leitor não conhece, uma inversa que não converge, um laço que sai
cruzando a si mesmo, uma triangulação que falha: todos têm a mesma resposta, e ela é um desenho que
já funcionava antes de qualquer disto existir. Você vê a peça cheia com um pedaço em contorno —
visivelmente outro tipo de desenho, feito só de curvas que ESTÃO no arquivo — e um fato mais uma
frase na prévia dizem quantas faces são.

É também o que torna este módulo seguro de ter sido escrito: se a malhagem tiver um defeito, ela
degrada para o arame em vez de virar lixo. No arquivo do dono é 1 face de 106; no eixo traseiro, 69
de 2 536.

#### O que foi inventado no banco de tipos, e o que não

Seis dos onze já existem no `shared-mime-info` 2.4 que o flake pina: `model/stl`, `model/3mf`,
`model/obj`, `model/gltf+json`, `model/gltf-binary` e `model/iges` — e o gerador RECUSA pôr em `ours`
um tipo que a base já conhece. A resolução também já funcionava para os dois primeiros: um STL
binário não tem magic nenhum (a única regra do freedesktop é a palavra `solid`, que é a forma ASCII),
então o sniff devolve `None` e `settled` fica com o nome; um 3MF sniffa `application/zip` e a base diz
que ele DESCENDE disso, então os dois concordam e o nome, mais fino, vence. É a mesma figura que o
`.dng` já tinha escrita.

Os outros cinco a base não tem — conferido: `*.ply`, `*.off`, `*.dae`, `*.amf`, `*.step` e `*.stp` não
têm UM reivindicante upstream. Nascem no `ours` do `mime-glyphs.nix`, **com `under`**, e o COLLADA
usa `model/vnd.collada+xml`, que é o nome registrado dele: o que falta upstream é a linha, não o
nome. O AMF usa `model/x-amf` e não `application/x-amf`, que é do Action Message Format da Adobe há
vinte anos.

E todos ganharam glifo: para o freedesktop são `image-x-generic` — correto e inútil numa coluna de
trinta linhas, onde uma peça que se imprime fica indistinguível de uma fotografia.

#### Quando DOIS tipos reivindicam o mesmo nome: `prefer`

O `globs2` tem duas linhas para `*.obj` no mesmo peso — `application/x-tgif` e `model/obj` — e o
`mime::by_name` fica com a PRIMEIRA de um par empatado. Resultado: um `.obj` nesta máquina resolvia
para o documento de um programa de desenho do X11 dos anos 1990, e ficava sem prévia, sem fatos e com
o glifo neutro. Nem `exact` nem `ours` consertam: o primeiro decide só o ícone, e uma linha de `ours`
entra no FIM da lista com o mesmo peso 50 e perde o mesmo desempate.

O bloco `prefer` nomeia o vencedor, o perdedor e o padrão, e o gerador **remove a linha do
perdedor** — não sobe o peso do vencedor, que o faria ganhar também de um reivindicante futuro de
peso MAIOR, e isso é mais do que a linha diz. E ele recusa de três maneiras: tipo que a base não tem,
tipo que não reivindica aquele padrão (upstream mudou e a linha caducou), e pesos que já diferem
(upstream já decidiu e a linha não faz nada). As três foram provadas quebrando a linha de propósito.

#### Um arquivo por leitor, e o defeito que cada um pega no topo dele

`mesh.rs` virou `mesh/` no dia em que passou de dois formatos a onze, porque o que é difícil em cada
um é diferente — e está escrito no topo do arquivo que o tem:

- **OBJ**: índice NEGATIVO é relativo ao fim da lista COMO ELA ESTÁ NAQUELA LINHA. Está na
  especificação original, exportadores usam, e um leitor que trate `-1` como fora de faixa não acha
  face nenhuma e relata um arquivo vazio.
- **PLY**: uma propriedade que este leitor não quer ainda tem de ser ENTENDIDA, porque o registro não
  tem separador. Um vértice de scanner é `x y z nx ny nz r g b a confiança intensidade`, e pular isso
  é saber que `uchar` é um byte e `double` são oito. E o tipo do CONTADOR de uma lista é declarado
  separado do tipo dos itens porque são coisas separadas.
- **glTF**: a cena é uma ÁRVORE e a geometria está nas folhas. Sem compor as transformações dos nós,
  uma cena de várias peças desenha todas empilhadas na origem — uma pilha reconhecível, e errada. A
  unidade é o METRO pela especificação, então entra ×1000 e o painel diz de onde veio o número. Um
  acessor `sparse` é APLICADO e não ignorado: ignorá-lo devolve as posições de base, que são as
  posições erradas sem erro nenhum. E um `.gltf` é o único que recebe o CAMINHO do arquivo, porque é
  o único cuja geometria pode estar num SEGUNDO arquivo — e por isso é o único que precisa recusar os
  caminhos que não vai abrir: nada absoluto, nada com `..`, nada com esquema.
- **COLLADA**: `<asset>` manda em duas coisas. `unit meter=` diz quanto vale um número, e `up_axis`
  diz que lado é para cima — e `Y_UP` é o padrão e o que toda ferramenta de arte escreve, então um
  leitor que o ignore deita a maioria dos documentos de lado. As transformações de um `<node>`
  compõem NA ORDEM EM QUE ESTÃO ESCRITAS. E `<p>` é ENTRELAÇADO: com uma entrada de normal no
  deslocamento 1, só metade dos números é vértice.
- **AMF**: pode ser um zip OU o XML cru, sob a mesma extensão, então os BYTES decidem — um leitor que
  suportasse um só falharia em metade dos arquivos dizendo que estão quebrados.
- **STEP**: um `CIRCLE` é um círculo INTEIRO; o que faz dele um arco é o `EDGE_CURVE` que o
  referencia, cujos dois vértices dizem onde começa e para, e cujo `same_sense` diz por qual dos dois
  lados. Um leitor que desenhasse o círculo poria um anel completo em cada arredondamento da peça. E
  o cabeçalho **não é UTF-8**: o arquivo do dono carrega bytes Big5 num campo que a norma diz ser
  ASCII, e `from_utf8` recusaria o arquivo inteiro por causa disso. Lê-se as escapadas `\X2\…\X0\`
  e o resto como Latin-1 — **e o campo assim lido fica MARCADO**, porque o painel não mostra um valor
  de que não tem certeza: mostrar `Windows ¨Ï¥Î ªÌ` no lugar do autor lê-se como defeito do
  gerenciador, não como o que o arquivo diz.
- **IGES**: é o único leitor sem um arquivo real do dono por trás. Fica provado contra fixtures
  escritas da especificação, o que testa o entendimento deste leitor e não o que um CAD escreve de
  verdade. O que isso compra é a FORMA da falha: tipo de entidade que ele não conhece é PULADO e
  arquivo sem curva nenhuma é recusado — ele falha com arestas de menos, nunca com peça errada.

#### O leitor de STL: o que decide a forma é ARITMÉTICA, nunca a primeira palavra

Os oitenta bytes de cabeçalho de um STL binário são livres, e escritores põem qualquer coisa neles,
`solid` inclusive. Quem decide é `84 + 50·n == len`. A normal gravada no arquivo é **ignorada e
recalculada**: metade dos STL do mundo traz `0 0 0`, e uma normal que às vezes está certa é pior que
nenhuma — a luz falha em ALGUMAS faces e parece defeito do renderizador.

Um 3MF quase nunca tem a geometria em `3D/3dmodel.model`. A extensão *production* deixa a raiz
carregar só `<components>` apontando outras partes por `p:path`, e os fatiadores usam isso: o arquivo
do dono tem raiz de 1,5 KB e seus 21 386 triângulos numa parte embaixo. E tanto o componente quanto o
`<build><item>` levam transformação — aquele arquivo escala Z por 1,808 —, então um leitor que as
ignorasse desenharia a peça achatada sem nada dizendo por quê. As unidades viram MILÍMETROS na
entrada: o desenho não se importa com escala, e o painel de infos é a razão inteira de isso importar.

O ZIP é lido aqui, com `flate2`, e o teto é sobre o que SAI da descompressão — o único número que uma
bomba de zip não pode mentir. Um leitor de ZIP de biblioteca seria uma entrada nova no lock para
duzentas linhas.

#### Uma câmera, um corpo com duas metades

O `solid::draw` recebe um `Body` — que carrega uma malha E arestas — e monta um `Lens` antes de olhar
para o que tem dentro: mesma projeção, mesmo z-buffer, mesmo fundo transparente, mesmo supersampling.
Separar os dois caminhos seriam duas projeções que têm de concordar, e no dia em que discordassem os
dois tipos de arquivo enquadrariam diferente sem nada dizendo por quê.

O `Body` era um `enum` de um OU outro, e o B-Rep quebrou isso: uma peça sai cheia com as faces que
este leitor não malhou desenhadas em contorno, então as duas chegam juntas e são desenhadas juntas.
Preenche primeiro, traça depois, no mesmo buffer de profundidade — é o que põe um contorno na frente
do material a que pertence em vez de enterrado dentro dele.

#### A gaiola: sem linha oculta, e a profundidade vira ALFA

Não há remoção de linha oculta, porque sem as faces não há o que ocultar — e inventá-las seria
inventar a peça. Vê-se a gaiola inteira, que é o que um desenho de CAD é.

O que faz isso ler como volume em vez de como emaranhado é a aresta do fundo DESBOTAR, e o desbote é
por AMOSTRA e não por segmento: uma aresta comprida que foge do olho muda de profundidade ao longo do
próprio comprimento, e esse gradiente é a maior parte da leitura. O z-buffer ainda é testado e
escrito, porque onde duas arestas se cruzam a de perto tem de ganhar — é a única oclusão que existe
ali, e sem ela decide quem o arquivo escreveu por último.

Espessura e desbote são TOKENS (`space.files.meshWireWidth`, `meshWireFade`), e a espessura é em
DÉCIMOS de pixel porque o rasterizador tira duas amostras por lado: uma linha de uma amostra sai com
meio pixel na tela e vira fantasma cinza depois da média.

**Medido**: 2,3 ms para as 251 arestas do STEP do dono, que amostram 2 621 segmentos. Uma ordem de
grandeza abaixo de uma malha, e por isso o caminho do arame nunca virou a coisa que valia medir duas
vezes.

#### O rasterizador é em processo, e o fundo é TRANSPARENTE

`pixels::from_svg` é o precedente: o resvg rasteriza NO tamanho, em processo, e monta um `Rgba` à
mão. Um programa externo aqui seria a primeira biblioteca C que esta prévia precisou, e o
`files-closure-has-no-mime-db` é quem diria isso.

O fundo é transparente pela razão que a tarja do filme já registra: há UMA textura e DUAS portas, e o
fundo de cada uma é um token diferente. E a luz é fixa em relação à CÂMERA — uma luz fixa no mundo
põe a peça na própria sombra assim que alguém a vira, e o gesto que era para mostrar o outro lado
mostra uma silhueta.

**Medido**, para 512×384: 44 ms nos 225 154 triângulos do 3DBenchy, 17 ms num cortador de 121 724, 13
ms num porta-velas de 21 386 e 9 ms num OBJ de 49 999, dobrando com a peça ocupando o painel. É aritmética no preparo de cada triângulo, não memória —
empacotar o framebuffer não valeu nada e passar as funções de aresta a incremento valeu 4×. **Faces
de trás NÃO são descartadas**, e é o único lugar onde a velocidade foi recusada: medido em 1,5× (41
ms para 33), e correto só numa malha cujas faces estão todas no mesmo sentido. Malha com faces
invertidas existe, o `Sanity::flipped` as conta, e o que o descarte faz com uma dessas é abrir buracos
nela.

#### A thread é o `motion.rs` de novo, e a malha mora NELA

Mesma forma e pelas mesmas razões escritas lá: `mpsc` na entrada, `calloop::channel` na saída, era
atômica para abandonar um desenho no meio, drenagem até o mais novo, e **o quadro chegando é o
relógio** — nenhum timer novo. Canal próprio, não o de `Report`, porque um arrasto emite um quadro a
cada dezenas de milissegundos e nada disso é notícia sobre um diretório.

A malha fica na thread, indexada pela chave do arquivo, e é isso que separa "arrastar é fluido" de
"arrastar reabre 11 MB". `Looked` é `Box`ado, clonado e comparado pela porta: oito megabytes de
posições passando por ali seriam oito megabytes comparados campo a campo.

#### Os gestos, e por que as setas puras não giram

Arrastar gira, a roda dá zoom — a figura já engolia a roda —, `S-<setas>` giram, `C-k`/`C-j` aproximam
e afastam, `C-r` reenquadra. As setas PURAS continuam andando nos ARQUIVOS: o dono pediu isso
explicitamente para a prévia larga, que é justamente onde uma peça fica boa de olhar.

O arrasto **não** é um quarto `Axis`. `drag_to` inverte um trilho — uma fração, um eixo — e girar são
dois deltas sem trilho nenhum; ele é um gesto da porta, ao lado do `dragging_section`.

A direção do arrasto mora na `View::spin`, que fala em ONDE A MÃO FOI — `right` e `down` — e não em
ângulos. O `solid::axes` põe a CÂMERA no `yaw`, então yaw maior anda com a câmera no anti-horário e
tudo escorrega para a ESQUERDA na tela; um arrasto é o gesto contrário, a mão leva a peça JUNTO, e
puxar para a esquerda tem de virá-la para a esquerda. As portas tinham o sinal ao contrário e o dono
viu na tela. Uma convenção escrita em dois sítios de chamada e em nenhum lugar testável é uma
convenção que sai invertida num deles — agora as teclas passam pela mesma função, e um teste compara
o arrasto CONTRA a tecla.

#### O teclado não repetia em porta nenhuma

Segurar `S-<seta>` girava a peça uma vez e parava. A causa não era do 3D: as duas portas criavam o
teclado com `get_keyboard`, e o SCTK só gera repetição para um criado com `get_keyboard_with_repeat`
— então o `repeat_key`, escrito nas duas e documentado como *"segurar `j` anda na lista"*, era código
morto, e **nenhuma tecla repetia em lugar nenhum deste shell**. O ritmo e o atraso são do
COMPOSITOR, chegam como `RepeatInfo`, e nada aqui inventa nenhum dos dois.

O SCTK entrega a repetição pelo CALLBACK e não pelo método, então as duas portas ganharam um `typed`
por onde passam as duas entradas: uma repetição que tomasse outro caminho é uma tecla que faz algo
ligeiramente diferente na segunda vez. E o rodapé teve de criar o `calloop` ANTES do roundtrip — é
durante ele que o assento anuncia o teclado, e o `LoopHandle` precisa existir nessa hora.

Isso custou dois defeitos que estavam ali desde antes. **O `shift` não chegava** em tecla que não é
caractere: a regra escrita era "shift vem do keysym, porque o xkb já o aplicou", o que é verdade de um
CARACTERE e de nada mais — `Shift+Left` é `Left`, e o modificador é o único lugar onde aquele shift
existe. No rodapé o `discount` ainda o zerava por completo. Pego na tela: `S-<left>` subia um
diretório. E **a textura de um quadro só era enviada para filme**: a porta perguntava `moving_frame`
pelo nome, então a peça era desenhada, o log dizia que sim, e o painel ficava vazio. Agora as duas
perguntam `live_frame` — "a figura que se redesenha" —, e a próxima coisa que se redesenhar não
precisa de um terceiro sítio.

#### Os fatos saem da MESMA leitura

Um arquivo de cem megabytes lido duas vezes para manter as formas arrumadas são cem megabytes lidos
duas vezes, então o hospedeiro lê a malha uma vez e usa a resposta para as duas coisas: os fatos, e
saber que há algo para girar. Malha que não analisa é `Refusal::Failed` com frase, nunca uma espera
sem fim por um quadro que não vem.

Uma FORMA não é texto, e três dos onze tipos SÃO texto pela conta do banco — `model/obj`,
`model/x-off` e `model/step` descendem de `text/plain`, porque descendem mesmo. Sem esse braço, o
`.obj` de 4,3 MB do dono era lido por um mebibyte, realçado e jogado fora: `preview_fields` devolve
brancos para tudo que gira, então nenhuma linha dele podia chegar à tela.

Um corpo de arestas responde MENOS fatos, e honestamente: não tem volume, não tem área e não tem
"superfície fechada", porque as três são propriedades de uma SUPERFÍCIE e ali não há nenhuma. Em
troca tem o que só um B-Rep sabe — esquema, sólidos, cascas, faces, arestas — e a caixa, que é a
caixa DAS ARESTAS: para peça mecânica é a caixa da peça, e o caso em que difere é uma face que estufa
para fora do próprio contorno. Está escrito ao lado da conta.

A sanidade sai de ORDENAR as arestas e não de uma tabela de hash: a contagem é conhecida de antemão e
grande, e a mesma lista ordenada responde "quantos vértices distintos" de graça. E as medidas saem em
centímetros cúbicos porque 15 550 mm³ é um número de que ninguém tem noção e 15,6 cm³ é contra o que
se compra um rolo de filamento.

O `fluent-bundle` daqui **não localiza número nenhum** — medido, nos dois idiomas —, então uma medida
entregue como float teria levado o ponto do Rust para o pt-BR. `Arg::Fraction` a formata onde o
idioma é conhecido. `Arg::Number` não recebe esse tratamento de propósito: o Fluent escolhe ramo de
plural nele, e um texto não tem ramo.

### Formatar — `LEADER s f`, e a mesma porta pelo menu da barra

#### Por que é um buffer, e não linhas de menu

Porque são três perguntas e uma delas apaga um disco. Uma linha de menu tem espaço para um verbo;
isto precisa de espaço para dizer o que cada escolha significa, de um lugar para digitar um nome, e
de uma tela inteira dedicada a *"isto apaga tudo o que está aí"*. Cabe num `xdg_toplevel` porque é
o que um buffer é (invariante 4), e a barra lateral só ABRE — a decisão fica na superfície que tem
espaço para explicá-la.

#### Linguagem simples é conteúdo, e mora no `.ftl`

A linha do vfat diz *"abre em qualquer computador — pendrive, câmera, televisão"*. Não diz FAT32.
Quem já sabe que é FAT32 não perde nada; quem não sabe recebe uma resposta em vez de uma palavra para
procurar. E o **XFS aparece na lista, desabilitado, dizendo que esta distro não traz o programa que o
cria** — omitir sem dizer seria alguém procurando numa tela uma coisa que não está lá, que é a falha
calada que este repositório recusa em todo canto.

Não há campo para digitar em pergunta nenhuma: são listas, e o cursor anda com as setas. O único
lugar onde uma tecla é uma letra é o NOME, e ele cabe em **onze**, que é o limite do vfat e o mais
apertado dos cinco. Um número para todos em vez de cinco, e a razão é que o número é MOSTRADO: *"até
11 letras"* é regra que se segue, e um limite que mudasse conforme o cursor anda na lista de cima
seria regra que se mexe.

#### Duas pessoas, a mesma linha — o segundo registro

O rótulo é para quem não sabe o que é FAT32 e não deveria precisar saber. Mas quem sabe também usa
esta distro, e para essa pessoa *"abre em qualquer computador"* é uma frase que **esconde** a
resposta. Escolher um dos dois públicos seria escolher errado nos dois casos.

A saída é uma linha só, sob o cursor, na cor mais discreta que existe: `vfat (FAT32) · até 4 GiB
por arquivo · sem dono nem permissão`. Quem está começando nunca precisa ler; quem já sabe nunca
precisa adivinhar qual das cinco linhas é a que quer. E ela **acompanha o cursor**, que é o que a
separa de uma legenda: numa tela de escolha, a informação é sobre a linha que está prestes a ser
escolhida, ou não é sobre nada.

Nas três telas ela responde a mesma pergunta com o vocabulário de cada uma: na lista, o tamanho
EXATO em bytes — `32.0 GiB` é o que se lê, `34359738368 B` é o que se COMPARA com o que o fabricante
imprimiu no pendrive; na escolha, o sistema de arquivos de verdade e seus limites; e na confirmação,
o gesto inteiro — qual sistema, em qual nó, com qual nome, que é o mais perto que se chega do comando
que a pessoa teria digitado.

**O nome técnico é CHAVE e não literal no componente.** `vfat` é um fato sobre discos, e um
componente com opinião sobre discos deixou de ser um desenho (invariante 11). É também CONTEÚDO —
quanto do limite vale a pena nomear, como chamar o FAT32 na frente de alguém — e conteúdo mora no
`.ftl`, nos dois idiomas, onde o check enxerga.

#### E o mesmo em TODO menu, pelo TECLADO

O mecanismo já existia e estava pela metade: `files::Field` carrega uma `Explanation`, e o rodapé a
põe na linha de mensagem quando o **ponteiro** repousa sobre o campo. Só o ponteiro — o que
contradizia a regra do dono de que *o que o mouse faz, o teclado faz*, e deixava toda explicação do
gerenciador fora do alcance de quem não usa mouse.

Agora cada ação das duas tabelas (`menu.rs` e `sidebar_menu.rs`) carrega uma segunda chave,
`{prefixo}-does-<id>`, e `View::cursor_explains()` a entrega para a linha de mensagem **das duas
portas**. `apagar de vez` passa a dizer *"unlink(2) ou rmdir(2) direto: sem lixeira e sem volta"*;
`picotar` diz *"sobrescreve antes de apagar; num btrfs com cópia-na-escrita isso não garante nada"*,
que é uma ressalva verdadeira que nenhum rótulo caberia; `ejetar com segurança` diz *"desmonta e
corta a energia do disco (Drive.PowerOff)"*.

**Só com menu aberto, e é decisão.** Na listagem uma linha é um registro com colunas, e escolher uma
das colunas para explicar porque o cursor está em algum lugar da linha seria escolher ao acaso — é
para isso que o caminho por campo do ponteiro existe, e ele continua. Num menu a distinção não
aparece: todo campo da linha carrega a mesma resposta, porque a linha É uma escolha.

O prefixo é `-does-` e não `-detail-` porque `files-detail-` já era da ficha da entrada, e colidia em
`mode`, `owner` e `size` — a colisão apareceu no `i18n-complete`, que recusa chave repetida.

#### As duas recusas, e por que elas têm driver próprio

De fora do componente, uma recusa e um componente que morreu são a mesma coisa: nenhum plano chegou.
Por isso `format-driven` carrega as duas:

1. **Disco de que o sistema precisa é DESENHADO**, cinza, com o motivo escrito no fim da própria
   linha — e o `Enter` nele não anda. Nem cinza-e-depois-aceito, nem aceito-e-recusado-três-telas-
   depois. Ele está na lista porque quem procura o próprio disco tem de ACHÁ-LO e então ler por quê;
   uma lista que o omitisse mandaria a pessoa procurar uma linha que não existe, e a próxima coisa
   que ela pega é um terminal, que não pergunta nada.
2. **O primeiro `Enter` da confirmação VOLTA**, porque `não` é a primeira linha e onde o cursor
   começa. Quem chegou ali apertando `Enter` e aperta de novo no ritmo vai para trás. Apagar um disco
   custa uma tecla deliberada a mais, que é o preço certo.

#### Não existe estado `working`

Havia, no primeiro rascunho, e o `states-driven` o derrubou: todo estado declarado é oferecido no
`SPC w s`, então um `working` escolhível poria *"escrevendo no disco"* na modeline com nada sendo
escrito. A espera acontece **na lista**, com a linha de mensagem dizendo o que está em curso — que
também é onde ela deve acontecer, porque a linha que está sendo formatada está ali e vai mudar
sozinha.

#### O alvo é um id que o buffer RECEBEU, e o hospedeiro pergunta três vezes

O `Plan` nomeia o id que veio no último `Rack`. Do lado do hospedeiro, `buffer.rs` pergunta: mandei
este id; mandei-o como ALVO (um volume que o sistema precisa vai na lista, com `writable = false`);
e ele **ainda é um alvo** — `needed()` é reavaliado contra o volume, porque entre a leitura e a
tecla alguém pode ter montado o disco em `/mnt` ou o administrador pode ter editado o `fstab`. Três
perguntas e não uma, porque o buffer é um binário separado: guarda que mora dentro da coisa guardada
vale até alguém escrever um segundo buffer.

O que está montado desce ANTES, e a topologia é resolvida no hospedeiro, que a tem: para uma
partição é ela mesma; para um disco inteiro é toda partição DELE, achada pelo `Drive` e nunca
tirando um dígito do nó — `/dev/nvme0n1` termina em um e é um disco. Sem isso, o gesto mais comum de
todos — preparar um pendrive em que alguém acabou de clicar — falharia com `DeviceBusy`.

E `tear-down` **nunca** é passado ao udisks, embora ele desmonte sozinho: ele também **remove as
entradas de `fstab` do dispositivo**. Formatar um pendrive não pode editar a configuração de boot da
máquina como efeito colateral.

#### `dos` até 2 TiB, e o byte ao lado do sistema de arquivos

Disco inteiro ganha tabela nova mais UMA partição, em uma chamada só
(`CreatePartitionAndFormat`) — duas deixariam uma janela em que a partição existe sem sistema de
arquivos, e uma releitura dentro dessa janela mostra uma linha que ninguém abre. A tabela é `dos`
até 2 TiB e `gpt` acima, porque *"abre em qualquer computador"* inclui a câmera e o som do carro, e
esses leem MBR.

O `update-partition-type` não é enfeite: sem ele uma partição MBR com vfat sai tipada `0x83`, Linux.
O sistema de arquivos está certo e o byte ao lado está errado — o Windows não oferece abrir o
pendrive, e nada em lugar nenhum diz por quê. `disks-vm` mede o tipo da partição depois de formatar,
que é a única maneira de saber.

#### O que só a VM prova

`Manager::format` são três chamadas D-Bus com uma **corrida** entre elas: `Block.Format` escreve a
tabela, e a interface `PartitionTable` só aparece no objeto por um uevent que ainda não aconteceu.
Um teste com barramento simulado provaria a ordem dos argumentos, que nunca esteve em dúvida. O que
precisa de udisks de verdade é que a interface CHEGUE — daí o laço limitado de vinte tentativas de
100 ms, e daí o `ronoctua-format-disk`, um `[[bin]]` extraído por conta própria (`nix/rust.nix`)
para a VM não carregar wgpu, o runtime wasm e três fontes embutidas só para fazer três chamadas.

E a política do upstream ganhou duas asserções junto das de montar, porque é ela que torna a tela
possível: `modify-device` é `yes` para sessão ativa — formatar um pendrive não pede senha — e
`modify-device-system` é `auth_admin_keep`, que nesta sessão não tem quem responda. **As recusas do
buffer estão NA FRENTE dessas duas**: são o desenho, não a execução.

---

## 6. Rede, área de transferência e serviços

### Rede e Syncthing — `LEADER s n` e `LEADER s s`

#### A rede saiu da lista do "não se pergunta", e por quê

A rede esteve na lista do que **não** se pergunta, junto de teclado, fuso e placa de vídeo, sob uma
justificativa boa: instalador que pergunta demais é instalador que não decidiu.

Ela não cobre este caso, e por duas razões que nenhum dos outros itens da lista tem:

1. **Não há o que detectar.** O `nixos-facter` acha a placa de rede, mas a senha do Wi-Fi não é
   propriedade do hardware nem decisão do projeto: é um segredo por pessoa.
2. **Sem ela a instalação não roda.** O passo `inst-step-install` baixa o sistema inteiro da rede.
   Numa máquina sem porta Ethernet — e o laptop deste projeto é uma delas — não perguntar significa
   não instalar.

O critério continua valendo com a mesma força: **pergunta-se o que só a pessoa sabe, e nada além.**
A rede passou a caber nisso. São oito entradas no enum: seis telas sempre, uma calculada
(`resume_offset`) e uma **condicional** — a senha só aparece quando há rede escolhida, e quem vai de
cabo responde as mesmas seis de antes.

O SSID **não** entra no `host.nix`, e a senha não passa por `argv`: o perfil é escrito pelo
instalador (o mesmo mecanismo `ESCREVER` da passphrase, porque `/proc/<pid>/cmdline` é legível por
qualquer processo) em `/persist/etc/NetworkManager/system-connections`, que o `persist.nix` já
listava.

#### O nome de um token de ícone quebrou um buffer WASM

`wifi-0` parecia um nome. O enum `icon` do WIT é gerado dos tokens, e em WIT **cada segmento de um
identificador começa por letra** — o `wit_bindgen` recusou com `invalid character in identifier '0'`,
apontando para a linha do enum gerado, dentro da compilação de `ronoctua-buffer-unicode`. Um arquivo
que não tem nada a ver com ícones, três derivações adiante da causa.

Os nomes viraram palavras (`wifi-fraco`, `wifi-otimo`), mas o que resolve de verdade é o
`gen-tokens.py` **recusar na origem**, citando o token. É a quinta vez que este projeto paga para
aprender a mesma coisa: fronteira que aceita o que o outro lado recusa transforma um erro de
digitação num mistério.

#### A vista lê o cache do NetworkManager, e não espera o rádio

O localizador de aplicativos varre o XDG numa thread porque ler centenas de `.desktop` custa dezenas
de milissegundos. Aqui não há disco: o NetworkManager mantém em memória os pontos de acesso que o
rádio já viu, e devolvê-los é uma ida ao barramento.

O que demora é a **varredura**, e essa não é esperada: `RequestScan` volta na hora e o resultado
aparece na leitura seguinte. Bloquear até o rádio responder congelaria o rodapé por segundos — a
mesma armadilha do `systemd-run --scope`.

Rádio desligado lista **zero** redes, e não as da última varredura: oferecer uma conexão que não
pode acontecer faria a tecla falhar parecendo defeito.

#### Três vazios diferentes, e não um "sem redes"

Sem barramento, sem rádio e rádio desligado têm conserto diferente — o terceiro é uma tecla, os
outros dois não. Um texto genérico para os três mandaria a pessoa procurar no lugar errado.

#### Por que `overrideFolders = false` é a linha mais perigosa do módulo do Syncthing

Ela vale `true` no nixpkgs, e `true` significa *"o que não está declarado no Nix é apagado na
ativação"*. Este módulo não declara pasta nenhuma — caminho de pasta é dado pessoal (invariante 13) —
então o padrão **apagaria todas as pastas a cada `nixos-rebuild`**, inclusive as recém-adicionadas
pela interface, sem aviso e sem desfazer. Não há sintoma antes do estrago; por isso há check.

#### Os valores de rede saíram de uma medição, não de um blog

Uma transferência real de 592 GB entre duas máquinas deste projeto foi medida do começo ao fim. A
banda de 2,4 GHz rendia **2,9 MB/s**; a mesma transferência em 5 GHz rendeu **42 MB/s** — 16×, sem
mudar uma linha de configuração. Do que sobrou depois disso:

- `wifi.powersave = false` — o rádio dormia entre quadros;
- `tcp_slow_start_after_idle = 0` — o Syncthing pausa entre arquivos, e cada arquivo recomeçava em
  partida lenta; numa pasta de milhares de arquivos isso é a maior parte do tempo;
- BBR **com** `fq` — sob carga a latência até o roteador saía de 1,3 ms e chegava a 105 ms, que é
  enfileiramento puro; o cubic responde a isso enchendo a fila. BBR sem `fq` funciona pior que o
  cubic, e é por isso que os dois são conferidos juntos no check;
- `numConnections = 3` — o padrão é uma conexão TCP por dispositivo, e um fluxo só não mantém janela
  em voo suficiente num enlace cuja latência sob carga vai a 150 ms.

#### A senha do Wi-Fi saiu do arquivo e foi para o TPM

O jeito de todo mundo é `psk=<senha>` dentro do keyfile do NetworkManager, modo `0600`. Aqui isso já
vivia num volume LUKS, então não era bem "senha em texto plano no disco" — o que sobrava de exposição
era estreito e concreto: **cópia que sai do volume cifrado**. `/etc/NetworkManager/system-connections`
está na lista do `persist.nix`, e qualquer backup de `/persist` levaria as senhas junto.

O NetworkManager tem o mecanismo para isso e quase ninguém o usa fora do GNOME Keyring: **`psk-flags`**
mais um **agente de segredo** no D-Bus. O keyfile deixa de conter a senha; quem a entrega, quando o NM
pede, é o `ronoctua-network-agent`, abrindo um blob selado no TPM2.

**A simplificação que fez isto caber:** o agente não recebe a senha de ninguém. O
`AddAndActivateConnection` aceita a senha na própria chamada, e as `psk-flags` decidem apenas o que é
**persistido**. Então o rodapé continua entregando a senha ao NM na hora de conectar, e o daemon só
entra no boot seguinte — quando não há mais ninguém digitando. Sem isso, a senha teria de atravessar
mais um canal, e cada canal é uma superfície a mais.

**Escolha por rede, e não política global.** Depois de conectar, uma tecla: lembrar grava
`psk-flags=1` e sela; qualquer outra resposta deixa `psk-flags=2`, e nada persiste em lugar nenhum. A
conexão acontece **antes** da pergunta, com `psk-flags=2` — guardar primeiro e apagar se a pessoa
recusar deixaria a senha em disco no intervalo.

**Sem política de PCR, deliberadamente.** Prender o blob ao PCR 11 — que mede a UKI — faria **toda
atualização de kernel quebrar o Wi-Fi em silêncio**, e não compraria quase nada: o blob já vive dentro
de um volume que os mesmos PCRs protegem. Sem política, ele continua inútil em qualquer outra máquina,
que é o alvo, e sobrevive a atualização. É também o que permite ao instalador selar **de dentro do
ISO**: o TPM é o mesmo, e nenhum PCR do sistema final precisa existir ainda. O check de VM reinicia a
máquina exatamente para trancar isso — se alguém acrescentar `--tpm2-pcrs=` sem pensar, é ali que
aparece.

`--with-key=tpm2` e não `auto`: o `auto` mistura a chave de host, que no alvo ainda não existe quando
o instalador sela.

**O modo de falha é o fluxo normal.** Sem blob, sem TPM ou com o TPM recusando, o agente responde
`NoSecrets` — e para o NetworkManager isso significa "peça à pessoa", que é exatamente o que acontece
com quem nunca conectou naquela rede. Se o daemon estiver fora do ar, o NM não acha agente nenhum e
faz a mesma coisa. A rede não fica presa ao daemon estar vivo; fica presa a **alguém** poder responder.

**Quem autoriza é o barramento, não o daemon.** A política em `nix/modules/network.nix` decide que só o
root levanta o agente e só o usuário da distro pede selagem. Um `if usuario == "noctua"` dentro do
Rust seria uma segunda verdade — e a que erra em silêncio, porque um daemon que se autoriza sozinho
parece funcionar até não parecer.

**O que isto não protege:** root numa máquina ligada. A senha chega ao `wpa_supplicant` em RAM de
qualquer jeito, e não há desenho que evite isso.

#### Renomear identificador: a prova óbvia não serve

A passada dos 18 078 comentários provava-se sozinha: extrair só as faixas de comentário, traduzir,
recolocar, e exigir que as 44 747 linhas de código ficassem **byte a byte idênticas**. Sem git
(invariante 2), era a única rede.

Para os identificadores essa prova não existe — o código *é* o que muda. A ida-e-volta pelo mapa
inverso parece o análogo e **está errada**, medido: como os comentários já estavam em inglês,
`name`, `line` e `key` já ocorriam na árvore, e a volta traduzia de novo os que já estavam certos —
1 581 trocas na ida contra 3 742 na volta.

O que substituiu:

1. **`conflicts` antes de aplicar.** Para cada `velho → novo`, onde `novo` **já existe** como
   identificador? É o perigo real: um rename que cai sobre um nome ocupado o sombreia, e sombreamento
   dentro de uma função **compila**. Foi assim que `ponta → cap` ia colidir com uma variável `cap` na
   mesma função, e `casa → matches` com a macro `matches!`.
2. **O diário.** Cada `apply` grava `(arquivo, offset, velho, novo)`, e o `undo` reverte exatamente
   aqueles trechos. Voltar byte a byte prova que a aplicação foi posicional e completa.
3. **O compilador**, que cobre o significado — e a `cargo fmt` logo atrás, porque renomear muda a
   ordem alfabética dos `use` e o `rust-fmt` reprova.

**O reescritor tem de saber onde cada byte está.** `nome` é ao mesmo tempo identificador, campo do
JSON gerado (`"nome"`), palavra dentro de um valor `.ftl` em pt-BR e palavra num comentário. Um `sed`
troca os quatro, e os dois que **não** podem mudar são justamente os que nenhum compilador defende.
O que custou descobrir: comentário de bloco **aninhado**, raw string `r#"…"#`, char literal contra
lifetime, e a **captura de format** — `{nome}` dentro de string É referência a código, e são 667
delas.

#### A SUPERFÍCIE do check importa mais que a lista dele

O `english-names` nasceu lendo nome de arquivo e chave `.ftl`. Cada vez que a superfície dele foi
ampliada, ele achou uma safra inteira que a anterior não via — e não porque a lista de negação
tivesse melhorado, mas porque passou a **olhar outro lugar**:

| superfície acrescentada | o que apareceu |
|---|---|
| declaração Rust (`fn`, `struct`, `enum`, …) | 1 921 identificadores |
| definição Nix e Python | 274 nomes na camada de build |
| **variante de enum** | **63**, com enums meio traduzidos (`Gesture::Inserir` ao lado de `Confirm`) |
| **campo de struct** | **3**, entre eles `Minibuffer.segredo` |
| **subcomando da CLI** | **4** (`comando`, `satelite`, `verificar-keymap`, `exportar-binds-niri`) |

O padrão é sempre o mesmo: **a linha que nomeia essas coisas não tem palavra-chave.** Uma variante é
uma palavra maiúscula indentada; um campo é uma palavra minúscula e dois-pontos; um subcomando é uma
string num braço de `match`. Toda regra escrita em cima de "procure a palavra-chave X" é cega para
elas, e ficam invisíveis exatamente porque parecem prosa para o verificador.

Ao suspeitar de uma lacuna, a pergunta útil não é "que radical falta na lista?" — é **"que forma
sintática eu ainda não estou lendo?"**

#### Lista de negação acha o que você lembrou; ausência acha o que você esqueceu

O `english-names` recusa uma lista de radicais portugueses, e essa escolha está
justificada no próprio check: detectar "isto é português" em geral é impossível, recusar exatamente
as palavras que já estiveram aqui é possível.

O limite disso ficou medido. Com a lista passando limpa, uma varredura por **ausência** — segmento
que não aparece num vocabulário inglês de ~900 palavras — encontrou **231 nomes** que a lista não
cobria: `BUFFER_GLIFOS`, `MARGEM_DE_ROLAGEM`, `envolver`, `realcar`, `TETO_DO_HISTORICO`, `vigiar`,
`ceder_o_lugar`, `topos_alinhados`. Nenhum deles estava na lista porque ninguém tinha lembrado dele.

As duas convivem, com papéis diferentes: **a lista é o check** — roda em segundos, dá mensagem com
arquivo e linha, e impede a regressão; **a ausência é como se alimenta a lista**, e é o que se roda
depois de uma passada grande. Uma terceira, por morfologia (`-cao`, `-mento`, `-ivel`, `-dade`),
serve de conferência cruzada: as três concordarem é o que permite dizer "zero" sem apostar.

#### O que atravessa a fronteira não tem compilador

Três camadas viajam como **string** e por isso não quebram o build quando divergem — cada uma tem de
mudar nos dois lados na mesma passada:

- **O WIT.** O host carrega o componente por `Val` dinâmico (`ronoctua-runtime`), então os nomes dos
  casos são literais: 61 strings, mais o nome qualificado `ronoctua:buffer/view@0.1.0`. Renomear o
  `.wit` sem elas dá "o componente nao exporta", longe da causa.
- **Os campos do JSON gerado.** Mudam em três lugares juntos: o gerador Python, o parser escrito à
  mão do buffer (que não tem parser de JSON, invariante 11) e o `jq` dos checks. E dentro do próprio
  gerador o campo é escrito com aspas duplas e **lido** com simples — `stats['sem_pt']` sobreviveu à
  primeira passada e explodiu no build.
- **Os campos de `tracing`.** Os testes de VM fazem `grep` neles (`command=`, `engine=alacritty`,
  `text=foot`). O log e o `grep` mudam juntos, senão o teste passa a falhar por outro motivo.
- **O nome do subcomando e a flag de CLI.** `Some("comando")` é um braço de `match`, e
  `--teto-sem-pt` é uma string que o Nix passa ao Python. Nenhuma regra de declaração os alcança:
  os quatro subcomandos atravessaram a refatoração inteira intactos, e renomear `teto`→`cap` só do
  lado do chamador deu `unrecognized arguments` no build. O `english-names` passou a ler os braços
  de `match` do `main.rs`; a paridade das flags se confere comparando o `add_argument` de cada
  gerador com o que os `.nix` passam.

#### O `''` do Nix é uma linguagem dentro da outra, e o classificador tem de saber disso

Reescrever `.nix` com consciência de região custou três defeitos, todos da mesma família — **dentro
da fence há um segundo idioma, e as regras dos dois valem ao mesmo tempo**:

- **A fence não termina no primeiro `''`.** `'''`, `''$` e `''\` são escapes. Lendo `''${1:-6}` como
  fim de string, o `nix/bench-frames.nix` virou dois fragmentos e 90% do script saiu da análise —
  em silêncio, que é o pior jeito de uma varredura falhar.
- **O `#` da fence é do script, não do Nix.** Numa linha comentada do shell, `${prefixo}` continua
  sendo antiquotação VIVA. Pular essa linha renomeia a ligação e não o uso, e o arquivo para de
  avaliar com `undefined variable` — a única das três que falhou alto.
- **O comentário do KDL é `//`, não `#`.** Classificar a prosa de `nix/dev.nix` como código fez o
  mapa renomear palavras dentro dela: "um `tree_size` que mostre o `footer_proc` de duas `lines`".

E mais duas, que não são do Nix:

- **Uma aspa desemparelhada apaga o arquivo inteiro da reescrita.** Uma fence de um caractere que
  procura o par sem limite atravessa o fim da linha e marca o resto do arquivo como string; o
  `nix/scripts/scan-secrets.py` atravessou uma passada inteira sem uma troca e sem um erro. Em
  Python, shell e Nix, `"` e `'` **nunca cruzam a quebra de linha** — é esse limite que fecha a
  classe. O primo disso é a **aspa aninhada do shell**: em `[ "$(grep -c "footer-mark" …)" ]` a
  segunda aspa FECHA a primeira, então o token ficou fora de string em uma das cinco ocorrências —
  e só nela foi renomeado.
- **O hífen não é fronteira de palavra.** `binds-padrao.rs` virou `binds-sleepPattern.rs` do lado do
  Nix enquanto o `include!` do Rust seguia no nome velho. Nome de arquivo, id kebab e unidade do
  systemd casam no meio; depois de qualquer passada, varra alvo colado a `-` ou a `.`.

#### 4,1 GB não cabem no git, nem na cota do LFS — e Release é a resposta certa

Os 540 arquivos de fonte do `noctua-iosevka` são 4,1 GB. Em git puro o GitHub recusa; em LFS são
**quatro vezes**
a cota gratuita de 1 GB, e a única saída oferecida é um data pack pago.

O reflexo é achar que falta espaço. Não falta: **falta enquadrar o que essas fontes são.** Elas não
são fonte — são SAÍDA de build, geradas do Iosevka upstream com um plano e um patch. Versionar saída
de build em git é o que o LFS existe para adiar, e aqui nem ele alcança.

Um asset de Release não tem cota, aceita 2 GB por arquivo, é ilimitado para repositório público e é
de graça. E `fetchurl` com hash é tão reprodutível quanto um rev — o hash prende o byte, que é
exatamente o que se quer de um artefato.

Duas medidas que decidiram o formato: **zstd -19 leva um TTF a 24%** do tamanho (15,5 MB → 3,8 MB),
e o maior asset fica em ~510 MB, folgado dentro do limite. O `tar` vai com `--sort=name` e mtime
fixo, senão o hash muda a cada empacotamento e o `fetchurl` teria de ser atualizado à toa.

O que não dá para automatizar é a ordem: **o Release existe antes de o input apontar para ele**,
senão o `nix flake check` quebra num 404.

E há um 404 anterior a esse, que custou uma tentativa inteira: **asset de Release herda a
visibilidade do repositório.** Enquanto o `ronoctua-distro` era privado, `releases/download/…`
respondia 404 para tudo — anônimo, netrc com o token no lugar da senha, `Authorization: Bearer`, e
`nix --access-tokens`. Só a URL de API (`repos/…/releases/assets/<id>`) serve o arquivo, e ela exige
`Accept: application/octet-stream` — um cabeçalho que **input de flake não tem como mandar**. Não há
contorno: ou o repositório é público, ou o input volta a ser caminho local. Autenticar seria pior que
o caminho local, porque alcança o mesmo e ainda finge que qualquer um constrói.

São **quatro inputs**, um por asset, porque o `fonts-release` empacota uma pasta por tarball. Dois
detalhes do fetcher que não se deve redescobrir:

- **`file+https://`, não `https://`.** O Nix infere `type = "tarball"` de um `.tar.zst` e
  descompactaria cada asset no store — 4,1 GB de arquivos soltos que a derivação seguinte copia de
  novo.
  O `file+` força o fetcher de arquivo: ficam os 1,4 GB comprimidos, e o build extrai uma vez só.
- **Não há derivação de fonte intermediária.** Os quatro arquivos vão direto ao `nix/fonts.nix`, cujo
  `unpackPhase` os extrai na árvore que ele mesmo conserta. Cada tarball já traz um diretório de topo
  com o nome da pasta, então extrair os quatro lado a lado remonta o `noctua-iosevka/` original.
  Com `unpackPhase` próprio não se declara `sourceRoot`: o `cd` para ele mora dentro do padrão.

Quem trava é o `narHash` do `flake.lock`, não a tag. `fonts-v1` é ponteiro móvel como qualquer tag, e
o GitHub aceita reenviar um asset com o mesmo nome.

#### Caminho que guarda estado não é identificador

`/btrfs_tmp/@antigos` guarda os `@root` salvos pelo wipe. A passada trocou o `mkdir -p` e não a
string com o caminho completo, e o `impermanence-vm` reprovou — mas o conserto não era completar a
troca: era **desfazê-la**. Renomear um caminho de estado abandona o dado da máquina já instalada, e
o custo aparece no pior momento, que é quando alguém foi procurar um arquivo perdido. O mesmo vale
para `/persist/etc/ronoctua/wifi` e para o socket em `$XDG_RUNTIME_DIR`.

#### Captura de format órfã: o defeito que só existe na linha que executa

`f"{faixa}"` com o `faixa = …` renomeado duas linhas acima não é erro de sintaxe, não é erro de
importação e não aparece em revisão: o Python só levanta `NameError` quando **aquela linha roda**.
Três estavam vivas ao mesmo tempo — uma no caminho "não medido" do `nix run .#bench`, uma na linha
de resumo que o bench imprime SEMPRE, e uma no `modeline-vm`, que o type check do driver do NixOS
pegou. As duas primeiras nenhum check pegava.

O `format-captures-resolve` fecha isso, e o que ele precisa saber é o mesmo de cima: isolar o script
embutido do Nix ao redor (senão um atributo `name = …` do Nix conta como definição Python e esconde
justamente o órfão) e respeitar os escapes da fence.

#### A DSL de busca é interface, não nome

`familia:letra`, `bloco:f0`, `categoria:arrows` são o que a **pessoa digita**. O navegador Unicode já
aceitava os dois idiomas por desenho (`assets/lexicon/tags.en-pt.toml` existe para isso); os de
glifos e Lucide, só português. A passada uniformizou pelo que já existia — os três aceitam **ambos**.
Traduzir só para o inglês teria apagado metade de um recurso sob o pretexto de renomear.

#### Uma régua por PERGUNTA, e são duas

Havia seis funções medindo célula, e elas discordavam. Não eram seis respostas para a mesma pergunta:
eram duas perguntas e quatro cópias mal-feitas delas.

**Contar coisas** — `ronoctua_protocol::cells`, por CLUSTER. É o que RESERVA no buffer
(`budget::wide` a chama), o que CONFERE no hospedeiro (`columns_of`) e o que CORTA
(`layout::elide_end` anda os mesmos clusters). Mora no protocolo porque é o crate que os dois lados
compilam: uma régua com duas definições são duas réguas, e foi assim que `driven::cells` nasceu — uma
quarta, escrita só porque a do buffer era a errada para um check.

**O avanço** — o campo `width` do banco gerado. Um emoji avança `1,245 em` contra os `0,600` da
célula, então o cluster conta **um** e cobre **duas** células. Nenhum código pode saber disso: a
resposta está na fonte, e quem leu a fonte foi o gerador. `Symbol::ink` é quem usa.

As duas não se fundem, e é isso que torna honesto parar em duas. O `ink-is-not-a-count` cobra a
fronteira: o caractere que o navegador DESENHA nunca chega ao contador, e a run que o desenha orça
pelo campo medido.

O preço da unificação foi medido antes de ser pago: `unicode-segmentation` entrou no
`ronoctua-protocol` (que até então tinha zero dependências) e daí no `.wasm` dos buffers. Quatro não
pagam nada — não chamam `wide`, e o LTO derruba a tabela. Os que pagam: **unicode +0,6%, glyphs
+1,0%, lucide +2,7%, format +37,6%** (102 → 140 KB, o único caso em que a proporção assusta e o
número absoluto não). No conjunto, **+129 KiB sobre 9,57 MiB**.

E quatro defeitos latentes caíram junto, todos da mesma família: `runs_of` e `wrap_text` mediam e
CORTAVAM por `chars()` sob um comentário que afirmava, por escrito, que `layout::width` conta
caracteres — conta clusters; o mapeamento byte→célula do realce fazia o mesmo; e um teste do buffer
`format` reimplementava o `columns_of` do hospedeiro à mão.

#### Um terceiro idioma, e o que ele custava

A lista de idiomas estava escrita à mão em **95 sítios**, e **41** deles um terceiro idioma obrigaria
a editar — sem que nada apontasse para os outros 54. Pior: um `.ftl` solto em `assets/i18n` era
copiado para o store, passava pelo `english-names` e era **ignorado por todo o resto** — nem paridade
de chave, nem entrada em `LANGUAGES`, nem bundle, nem menu. Presente, processado e inerte.

Agora há uma lista, `assets/i18n/languages.nix`, na mesma forma dos tokens e das notificações. Dela
saem o enum do módulo, os argumentos dos três geradores de banco, o do `gen-i18n.py`, o do
`gen-extension.py`, os 24 sítios do `checks.nix` e as tags de cada banco. Os drivers não a leem: eles
listam o DIRETÓRIO que já recebem em `argv[2]`, porque não linkam o `ronoctua-i18n` de propósito.

Três coisas precisaram de desenho, e não só de encanamento:

1. **O `face()` é GERADO.** `include_str!` recebe literal e não se põe num laço, então o `match`
   escrito à mão era o último lugar onde um idioma declarado podia ficar sem corpo: o nome estaria em
   `LANGUAGES`, o arquivo no disco, e `Texts::fresh` responderia `None` calada.
2. **Três asserções eram pareadas por CONSTRUÇÃO** — `pt != en` no pixel, `grep -c` de um contra o do
   outro, passos do instalador de um contra os do outro. Duas comparadas entre si é a forma que não
   cresce: com três, o par continuaria concordando enquanto o novato não dizia nada. Viraram "todos
   distintos" e "todos iguais".
3. **O par `(en, pt)` estava na FORMA do dado** — no retorno de `translate()`, nos campos
   `tags_en`/`tags_pt` do banco, nas structs de três buffers e em duas chaves de texto. Virou um campo
   por idioma (`tags_<subtag>`, plano — o buffer não tem parser de JSON e leria um objeto aninhado
   como uma lista de lixo) e uma lista de listas do lado do Rust. O léxico já aceitava a forma nomeada
   (`{ en = [...], pt = [...] }`), então crescer é acrescentar chave, não reescrever 4 085 linhas.

O que **não** ficou barato, e é honesto dizer: as 1 309 traduções, e as palavras do léxico. Isso é
trabalho humano e não há encanamento que o remova. O que se removeu foi tudo o que se somava a ele.

O `third-language-costs-nothing` prova o encanamento sem inventar tradução: constrói um `zz-ZZ`
sintético — o idioma base com cada valor esticado ~20%, que é o que um alemão custa contra o inglês —
e exige que o gerador o aceite, que `LANGUAGES` vire três, que o `face()` ganhe o braço, e que um
`zz-ZZ` INCOMPLETO continue sendo recusado.

#### Uma chave de texto tem SETE derivados, e só um deles é o `.ftl`

Renomear as 358 chaves para o inglês (invariante 18) custou **dez rodadas do `nix flake check`**, e cada uma
revelou um lugar onde a chave é montada — nunca escrita. Ela só existe inteira em runtime, e por isso
nenhuma busca no código a enxerga:

1. o **prefixo** constante no buffer (`unicode-family-`);
2. a **constante sentinela** (`FAMILIA_TODOS = "todos"`), que vira o sufixo da opção "todas";
3. o **valor de dado** vindo do banco gerado;
4. a **tabela do gerador** (`gen-unicodedb.py`), onde o id nasce;
5. o **`.ftl`**, o único lugar visível;
6. a **lista esperada no arnês de teste**, que confere 1–4 contra 5;
7. o **nome da variável de ambiente** do instalador, derivado do rótulo por
   `format!("RONOCTUA_{}", rotulo.replace('-', "_").to_uppercase())`.

Quem renomear uma chave mexe nos sete. O compilador cobre zero deles.

#### `id`, `search` e `rótulo` são três papéis do mesmo dado

As tabelas do `gen-unicodedb.py` guardam `("symbol", "simbolo", "Símbolo")`, e a tentação é ler os
três como sinônimos. Não são:

- o **id** monta a chave (`unicode-family-symbol`) — é NOME, e por isso é inglês;
- a **busca** é o que a pessoa digita — é INTERFACE, e o projeto tem léxico bilíngue por desenho;
- o **rótulo** é o que o painel mostra, acentuado, e o check exige que digitá-lo ache a família
  inteira.

Traduzir o id sem olhar os outros dois quebrou a busca em português três vezes seguidas. A saída foi
**normalizar a entrada, não afrouxar a comparação**: `familia:` e `family:` aceitam as duas línguas,
uma tabela só (`translate_value`) converte para o id, e a comparação segue exata.

Afrouxar custou caro numa das tentativas: casar contra `tags_pt` inteiro fez `familia:emoji` devolver
3953 em vez de 3944. **Filtro que devolve a mais é pior que filtro que devolve a menos** — ninguém
confere o que não pediu.

#### O `[[bin]] name` do Cargo sobrepõe o nome do arquivo

Renomear `contrato-niri.rs` para `niri-contract.rs` **não** renomeia o binário: quem manda é o
`[[bin]] name` do `Cargo.toml`. O teste continuou chamando `bin/contrato-niri` e falhou com exit 127
— erro de shell, não de Rust, e portanto invisível ao `cargo check`.

#### Verificar por AUSÊNCIA, não por execução

Em todas as etapas em que declarei conclusão cedo demais, o que eu tinha verificado era *"o script
rodou"*. As duas verificações que de fato fecharam assunto foram outras:

- varrer o **dicionário inteiro** contra todos os literais e justificar cada falso positivo — foi
  assim que apareceram cinco arquivos e um `.wgsl` que a lista de extensões não cobria;
- cruzar **todo `bin/<algo>` citado** contra o que existe — foi assim que apareceram treze binários.

Uma busca construída a partir do que se espera achar devolve o que se espera achar.

---

### Quem espera o rádio é uma thread, não o quadro

`RequestScan` volta na hora; o rádio responde alguns segundos depois. Esperar no `calloop` congelaria
teclado, pintura e eventos do niri por esses segundos — a mesma armadilha do `systemd-run --scope`.
E um timer gastaria quadro repintando uma lista parada.

`LastScan` da `org.freedesktop.NetworkManager.Device.Wireless` muda exatamente quando há coisa nova
para ler. `ronoctua_network::watch_scan` sobe uma thread bloqueada no fluxo de mudança dessa
propriedade e avisa por `calloop::channel`; `Footer::rescanned` relê — e só se a vista estiver
aberta, porque preencher um estado que ninguém olha custa uma ida ao barramento à toa.

A thread é **preguiçosa**: sobe no primeiro `LEADER s n`, não na partida. Numa máquina sem rádio ela
falharia toda vez, e tentar de novo a cada tecla daria uma ida ao barramento e um aviso por toque —
por isso a marca de "já tentei" é posta *antes* de saber se deu certo.

---

### O histórico de clipboard

`c` virou GRUPO: a captura de tela era `t c`, e `t` passou a ser a fila de transferências. O dono
escolheu onde ela cairia — `c` de CAPTURA, que é o que as duas coisas embaixo dele fazem: o clipboard
captura o que se copia, a tela captura o que se vê. Custou uma tecla ao histórico (`c` virou `c c`),
que é a convenção que este keymap já usa em todo prefixo com um segundo item — `s n n`, `f f`,
`TAB TAB`.

`LEADER c c` abre, na região abaixo do minibuffer, o histórico à esquerda e — separado por uma régua,
que é a única divisão que o projeto usa (invariante 15) — o conteúdo do item, o que ele é, o estado
do ClipCascade e as ações à direita. É o **quinto** modo da mesma superfície, ao lado do which-key,
do `M-x`, do manual e do histórico de notificações: nenhuma janela nova, nenhum popup
(invariante 14).

#### O protocolo é `zwlr_data_control`, e não `wl_data_device`

A primeira versão usou o `wl_data_device` do SCTK. Ela **compilou, não deu erro, e nunca viu uma
cópia** — porque `wl_data_device.selection` só chega ao cliente que tem **foco de teclado** naquele
seat, e o rodapé em repouso não captura teclado — se capturasse, nenhum buffer receberia tecla.
O dispositivo era criado, o log dizia que estava pronto, e o histórico ficava vazio para sempre.

`zwlr_data_control_unstable_v1` existe exatamente para isto — a descrição dele diz "take the role of a
clipboard manager" —, entrega a seleção sem exigir foco, e é o que o `wl-paste --watch` e os
gerenciadores de clipboard usam por baixo.

#### Conexão própria, numa thread

Duas razões, e cada uma bastaria. **Ler uma oferta bloqueia**: quem escreve do outro lado é o programa
que copiou, e ele escreve quando quiser — um `read` no laço congelaria o rodapé, a mesma classe do
`wait_with_output` que travou o buffer ao copiar. E **o `Dispatch` do `Footer` é genérico**: o
`delegate_dispatch2!` do SCTK implementa `Dispatch<I, U>` para todo proxy cujo `U` fale `Dispatch2`, e
enfiar um protocolo nosso ali brigaria com essa impl.

É a mesma forma de `ronoctua_notify::dbus`: uma thread que escuta e manda pelo canal do `calloop` —
a quinta fonte de evento do daemon.

#### O histórico não toca o disco, e a senha não entra

Em memória ele morre com a sessão. Em arquivo sobreviveria a um reboot, entraria em backup, e viraria
um alvo que ninguém lembra que existe — guardando token, chave privada e senha.

E o que vem marcado como senha **nem entra**: `x-kde-passwordManagerHint` é o tipo que 1Password,
Bitwarden e KeePassXC anunciam, e é o único sinal padronizado que existe para "isto é segredo". A
comparação é sem caixa, porque o tipo aparece escrito de mais de um jeito entre aplicativos — um
histórico que guarda a senha porque o `H` veio maiúsculo não protege nada.

As duas decisões se sustentam mutuamente, e por isso são testadas juntas: sem a promessa de não
persistir, pular a senha seria meia proteção.

#### O QR é desenhado em quads

`qrcode` devolve a matriz; quem desenha é o pipeline de quads que o rodapé **já tem** — um quad por
módulo escuro, mais um quad claro embaixo pela zona de silêncio, que faz parte do código: sem ela a
câmera não acha as bordas.

O tamanho do módulo sai da altura disponível, e não de um número fixo: a região tem teto
(invariante 14) e a tela muda. Ilegível é o mesmo que ausente — ninguém consegue apontar a câmera.

#### Quantas células uma escala ocupa é o HOST quem diz

O glifo da ficha saía **cortado por baixo, encostado na régua**. A causa: o buffer reservava quatro
células para a escala `huge`, e a caixa de linha dela pede seis — 116 px de fonte com entrelinha
1,45 contra 29 px de célula. A altura de duas células da grade sofria do mesmo mal e só funcionava por
**coincidência**: 40 × 1,45 dá exatamente duas células.

O buffer não tem como acertar isso sozinho: tamanho de fonte e entrelinha são token, e ele não os vê
(invariante 11). Mas "quantas células" **não é** valor de token — é contagem de célula, a mesma
unidade em que ele já conta linha e coluna. Por isso o `context` passou a trazer `heights`, com o
número de células de cada escala, calculado pelo host a partir dos mesmos tokens que decidem o
tamanho em píxeis. As duas contas divergirem era o defeito; agora elas têm uma origem só.

E o glifo ganhou **uma célula de respiro** antes do que vem embaixo: um traço colado na base de um
desenho parece parte do desenho.

A decisão de escala e de espaço é **uma função só** (`sheet_glyph`), usada pelo desenho e pela
conta do clique — as duas já divergiram uma vez neste mesmo arquivo, e o sintoma foi o clique acertar
um item e a tela mostrar outro.

#### A prévia de imagem, e a terceira coisa que o renderer desenha

Até aqui o renderer sabia **glifo** e **retângulo de cor chapada**. Uma captura de tela colada
aparecia como `copia de tipo nao suportado ignorada` no log e nada na tela. Agora há um pipeline de
**quad texturizado** (`ronoctua_render::image`) — mesma economia do quad: sem buffer de vértices, os
quatro cantos saem do `vertex_index`, e a diferença é um bind group com a textura e o amostrador.

Isto não viola o invariante 15: uma imagem numa área não tem borda, raio nem sombra, e não sugere que
algo está *acima* de outra coisa. É o mesmo papel do quad.

Quatro decisões que valem o registro:

- **Encaixa, nunca estica.** Uma captura deformada faz quem olha duvidar do que copiou.
- **`Rgba8UnormSrgb`, e não `Unorm`.** Os bytes do PNG são sRGB e é a GPU que desfaz a curva; `Unorm`
  clarearia tudo — a mesma armadilha do `to_linear` do `CLAUDE.md`, vista do outro lado.
- **A textura só sobe quando muda, e a mudança é medida por VERSÃO.** Comparar dimensões não bastaria:
  duas capturas do mesmo monitor têm o mesmo tamanho e são imagens diferentes.
- **Os bytes ficam comprimidos no histórico.** Uma captura 4K são ~2 MB de PNG contra 33 MB de RGBA;
  guardar decodificado custaria isso **por entrada**. Quem decodifica é quem vai desenhar, e só o
  item em foco.

E o filtro de tamanho é uma guarda, não zelo: o cabeçalho de um PNG *declara* as dimensões, e um
arquivo malformado pode declarar 60000×60000 — 14 GB de RGBA alocados antes de qualquer verificação.
As dimensões são lidas do cabeçalho **antes** dos pixels, exatamente para poder recusar.

#### O realce de sintaxe é nosso, é um CRATE, e não pertence a app nenhum

`syntect` traria um motor de regex e ~1 MB de definições para o processo que precisa desenhar em um
frame; `tree-sitter` pediria uma dependência Nix **por linguagem**. O argumento sobreviveu ao
crescimento: hoje são 37 linguagens e **158 tipos MIME**, e o custo total é 55 KB de fonte Rust
gerada e **zero dependências**.

Ele nasceu como módulo do `ronoctua-clipboard`, e a primeira coisa que se descobriu ao mexer nele é
que **o clipboard nunca o usava**: o `lib.rs` só o declarava, e o único consumidor eram quatro linhas
do `surface.rs`. Um realçador que mora dentro de um app é o realçador que o próximo app copia — e o
próximo app chegou, que é a prévia do gerenciador. Ele é `crates/ronoctua-syntax`: `no_std` + `alloc`,
zero dependências, e a mudança inteira foram cinco linhas de call site.

##### A tabela declara TIPOS MIME, e nunca extensões

Porque `crates/ronoctua-files/src/mime.rs` já responde "o que é este arquivo", com a base do
freedesktop inteira: 1 233 padrões de nome, 496 pares de subclasse, 323 aliases. Um `"*.rs"` escrito
em `assets/syntax.nix` seria o SEGUNDO lugar decidindo o que um arquivo é, e o segundo lugar é o que
erra.

E a herança paga. Uma reivindicação em `application/xml` cobre **49** descendentes; uma em
`application/json` cobre 8; uma em `text/x-python` traz `python3`, `sagemath` e `scons`; e a tabela
de aliases entrega `text/x-diff`, `text/x-markdown` e `text/x-yaml` de graça. **Trinta e sete linhas
resolvem cento e cinquenta e oito tipos.**

##### E quando a base não tem o tipo, ele nasce em UM lugar

Acontece, e o TypeScript é o exemplo trabalhado: `*.ts` é `video/mp2t` no freedesktop, e o
freedesktop está **certo** — `.ts` significou fluxo de transporte MPEG anos antes de a linguagem
existir. Zig, Swift, Vue e Dockerfile simplesmente não estão lá.

A resposta **não** é uma lista de extensões no `assets/syntax.nix`. É uma linha no bloco `ours` de
`assets/icons/mime-glyphs.nix`, que já existia, já carregava o `text/x-nix` e já trazia a régua
escrita: o critério é o upstream não ter tipo nenhum. O gerador recusa um tipo que não esteja em
lugar nenhum dos dois, **nomeando aquele arquivo** — de modo que a regra não pode ser esquecida, só
discutida.

Uma coisa esse bloco não tinha, e um teste a cobrou: **cada tipo inventado diz de que descende**
(`under`). Sem isso o `is_a` responde `false` a toda pergunta sobre ele, e a primeira que se fez foi
"isto é texto?" — que É um `is_a`. Um `.nix` seria o único arquivo numa máquina NixOS cujo conteúdo a
prévia se recusaria a mostrar. Agora um tipo nasce INTEIRO ali: nome, glob, ícone e lugar na
hierarquia.

##### Quatorze papéis, doze cores, e dois que ficaram de fora

Ele reconhece comentário, texto, escape, número, constante, palavra reservada, tipo, função,
atributo, propriedade, etiqueta, título, e as duas linhas do diff. O critério de cada um: **existe
arquivo em que, sem este papel, duas coisas diferentes saem desenhadas iguais?** Papel que só deixa
mais bonito não entrou.

Dois não entraram, e os motivos valem o registro. **`Normal` não é variante**: as faixas carregam só
o que NÃO é comum, e o texto comum é comum por omissão — ele existia antes e nunca era construído,
que é uma mentira que o compilador ajuda a contar. E **`Operator` ficou de fora por medida, não por
gosto**: quem desenha aloca uma `String` por faixa dentro do frame que desenha, então uma faixa por
chave e por ponto-e-vírgula é uma alocação por chave e por ponto-e-vírgula — e derrota o próprio
propósito, porque o paredão cinza vira um paredão de confete.

Quatorze papéis chegam a doze tokens: `Constant` divide com `number` e `Heading` divide com
`attribute`. Continuam dois papéis cada, pelo mesmo motivo que o `Palette` do design escreve sobre
`Quiet` e `Dim`: no dia em que o tema os separar, a mudança é uma linha numa tabela e não uma caçada
por call site.

##### O estado cabe em oito bytes, e é isso que faz a rolagem custar o visível

Quem consome **rola**. Tem de ser possível começar a desenhar na linha quatro mil sem tokenizar as
quatro mil de cima, então `line` recebe um `State` e devolve outro, e `states` calcula um por linha
numa passada. Oito bytes por linha é o preço; a alternativa é uma passada pelo arquivo inteiro a cada
frame — 2 000 × 12 × 60 por segundo, que não é um problema de afinação, é forma errada.

Uma `const` assertion tranca o tamanho, e é o teste mais barato do crate: qualquer coisa que pusesse
uma `String` ali tiraria o `Copy` e poria uma alocação no orçamento do frame. É também por isso que
**heredoc ficou de fora** — seu terminador é texto arbitrário descoberto em tempo de execução, e
texto não cabe em oito bytes. Título setext ficou de fora pelo mesmo tipo de razão: precisaria do
conteúdo da linha ANTERIOR.

##### Duas ordens que são armadilha, e uma que já era

**O comentário é testado primeiro e RETORNA na hora** — é isso que faz "o comentário vence as aspas"
ser uma propriedade e não uma ordem que alguém tem de lembrar; sem ela, `# "isto"` abriria um texto
que nunca fecha. **A palavra é comparada inteira** — por prefixo, `iffy` viraria `if` colorido pela
metade. As duas são de sempre e continuam trancadas.

As duas novas: **no diff, os prefixos longos vêm antes dos curtos**, porque `--- a/x` e `+++ b/x`
começam com os mesmos caracteres de uma linha removida e de uma acrescentada — testar `+`/`-`
primeiro pinta o cabeçalho de cada trecho como se fosse conteúdo, num arquivo cuja função inteira é
distinguir os dois. E **um delimitador com teto (`max`) RECUSA em vez de fechar cedo**: a primeira
tentativa fechava, e o teste pegou — o `&'a str` do Rust saía como um texto que ia até a próxima
aspa, engolindo o tipo depois dele.

##### O que os testes cobram, e os três defeitos que eles acharam

Trinta e três testes, e o que carrega a feature é um só: **realçar a linha 40 a partir de
`states()[39]` tem de dar exatamente o que realçar desde a linha 1 dá**. Sem ele, rolar até uma linha
mostraria cores diferentes das que ela tem quando se chega nela de cima.

Três defeitos saíram deles na primeira execução: o `max` que fechava em vez de recusar; o raw string
do Rust, cujos `#` ficam **entre** o marcador e a aspa (`r#"a"b"#` fechava na aspa de dentro); e a
conta de deslocamento absoluto do texto inteiro. E dois testes nasceram sem morder — o de fronteira
de caractere só passou a pegar quando ganhou uma semente com **barra invertida imediatamente antes de
um caractere multibyte**, que é a única forma que faz o escape terminar dentro do `é`.

O realçador devolve **papel**, e não cor. Quem traduz é `chrome::syntax_color`, num lugar só, porque
os dois consumidores estão no mesmo crate e duas cópias seriam as duas prévias colorindo o mesmo Rust
de formas diferentes. Do lado do buffer, `Palette::syntax` faz o mesmo com os casos do enum WIT — e a
aritmética byte→célula mora no `Screen::code` do design, uma vez, em vez de em cada buffer que
tentasse.

##### Nos buffers é feature desligada, e o número é medido

Os buffers têm de 79 a 100 KB (`ronoctua_buffer_format.wasm` são 100 627 bytes) e a tabela é 55 KB de
fonte. Um buffer que não colore código cresceria metade do próprio tamanho para carregar uma tabela
que nunca lê. Com a feature desligada e o crate na árvore, o `format` mediu **+366 bytes**.

#### O ClipCascade diz a verdade

O painel mostra `ClipCascade: não integrado`, e o comando de configurar existe dizendo o mesmo. A
integração está no `PENDING.md`. Um indicador que fingisse "conectado" seria a fachada que este
projeto passou meses desfazendo — e a legenda das ações sai da **mesma tabela** que o check de órfão
lê (`leader::OF_CLIPBOARD_VIEW`), porque uma legenda escrita à mão é a segunda cópia que mente no
dia em que uma tecla mudar.

---

## 7. Memória, disco, boot e instalação

### A máquina não trava por falta de memória — e o desktop não é a vítima

O sintoma que se quis eliminar não é o processo morto: é a **máquina sumindo por minutos** enquanto o
kernel recicla, com o OOM killer chegando depois do ponto em que ainda dava para fazer algo. Quem
remove isso é um matador em espaço de usuário agindo por **pressão** — por travamento — e não por
quantos bytes sobraram.

Três coisas eram verdade antes, e cada uma é pior do que nada:

1. **O `systemd-oomd` já rodava vigiando coisa nenhuma.** `systemd.oomd.enable` é `true` por padrão
   neste nixpkgs e os três interruptores de slice são `false`: a lista de cgroups monitorados nasce
   vazia. Escrever `enable = true` "conserta" e não muda nada — o que arma é
   `ManagedOOMMemoryPressure=kill` numa slice, e nada mais.
2. **O `user@.service` traz `OOMScoreAdjust=100`**, herdado por tudo na sessão: o kernel preferia
   matar o compositor e o rodapé a matar um daemon.
3. **Compositor, rodapé e aplicativos dividiam uma slice só**, sem reserva para os dois primeiros nem
   teto para os últimos.

#### O desktop sobrevive por CONSTRUÇÃO

A `session.slice` é **irmã** da `app.slice`, não descendente, e o `systemd-oomd` só considera
descendentes do que vigia. Com a `app.slice` monitorada e mais nada dentro da sessão, o compositor e
o rodapé **não estão no conjunto de candidatos**. Não é xattr, não é preferência, não é heurística de
ranking — é topologia.

Por isso `enableRootSlice` e `enableUserSlices` ficam `false` **por decisão**. O primeiro vigia a
`-.slice`, cujos candidatos incluem os dois; e ali o `ManagedOOMPreference=avoid` deles seria
**ignorado**, porque o systemd só honra esse atributo quando o dono do cgroup e o dono do ancestral
vigiado coincidem — a `-.slice` é do root e a sessão é do usuário. O segundo instala um drop-in por
TIPO, que arma a `session.slice` junto.

#### A reserva precisa dos quatro níveis, ou vale zero

`memory.min` é limitado por **todos** os ancestrais: o valor efetivo é o mínimo do caminho. Posto só
na `session.slice`, ele vale zero — e o arquivo de unidade parece certo. São quatro drop-ins
(`user.slice`, `user-.slice`, `user@.service` e a `session.slice` do usuário) ou não é nada. O check
`memory-policy-shape` quebra um nível e falha; o teste de VM lê o `memory.min` **efetivo** na folha,
que é o único número que responde.

#### Tudo em porcentagem, e é isso que adapta à máquina

O systemd resolve `3%` contra a memória física **ao carregar a unidade, na máquina**. O flake guarda a
string; a mesma closure dá 245 MiB numa de 8 GiB e 491 MiB numa de 16. Nada é calculado em Nix.

Provado com **dois nós de RAM diferente**: um só nó mostra que o número existe, e um número que
existe é indistinguível de um que não adapta. O dobro de RAM tem de dar o dobro de reserva.

**`ManagedOOMMemoryPressureLimit` não é dessa família.** É fração de TEMPO — quanto de uma janela de
dez segundos a slice inteira passou esperando memória. `MemoryHigh=70%` e um limite de `50%` são
grandezas sem relação que por acaso compartilham o sinal.

#### A ordem de morrer se faz subindo os aplicativos

`OOMScoreAdjust` numa unidade de **usuário** é ignorado em silêncio: o kernel devolve `EACCES` abaixo
do valor herdado e o systemd engole em nível *debug*. A configuração compilaria, subiria e não faria
nada. A ordem se cria pelo outro lado, com `choom` elevando cada aplicativo estrangeiro — apps 500,
niri e rodapé 100, daemons 0.

#### E a morte não é silenciosa

`org.freedesktop.oom1.Manager.Killed` é sinal do barramento de **sistema**, e a política do systemd já
deixa qualquer usuário local recebê-lo. O rodapé abre uma segunda conexão e o transforma numa
notificação crítica — que já nunca expira e já abre o histórico. Zero pacote novo, zero unidade nova,
zero ponte de root para a sessão. Reporta morte por oomd; morte pelo kernel, não.

### O painel de memória — `LEADER s m`, e a primeira capacidade de sistema

A política acima é real e medida, e **nada na tela dizia isso**. O `btop` mostra processos; não mostra
o zram comprimindo, nem a reserva efetiva da `session.slice`, nem o que o `oomd` decidiu — e não
afrouxa nada. `LEADER s m` abre o buffer onde essas quatro coisas ficam.

#### O obstáculo: nada estruturado entrava num buffer

O `input` do WIT levava escalares — `char`, `bool`, uma string, o clique. E `effect::command(id)`
chegava ao host do buffer e era **só logado**. Um painel de memória precisa dos dois sentidos.

A extensão entrou pela porta que o próprio arquivo abriu para a Lucide: **uma `capability`**. Com a
trava, a superfície existe só para quem a declara — `input::survey` não é entregue a quem não pediu, e
`effect::ask` é recusado com o nome da capacidade que falta. `Effect::requires()` nasceu ao lado do
`Command::requires()` que já existia; até aqui o portão guardava só o que o buffer **desenha**, e o
primeiro pedido que **age** na máquina não é coisa para deixar sem portão.

**O custo apareceu no compilador, e foi mantido de propósito.** Um caso novo em `input` quebra o
`match` exaustivo dos quatro buffers existentes. A alternativa — um `world` separado que só este
buffer alveja — foi considerada e recusada: quatro braços explícitos dizendo "esta amostra não é
minha" custam quatro linhas, e a **exaustividade é o que pegou isso** no minuto em que nasceu. Um
curinga engoliria o próximo caso em silêncio.

#### Quem lê a máquina é um crate, e a raiz do `/proc` é parâmetro

`ronoctua-memory` é a única fronteira com `/proc`, os cgroups e a política do systemd — o mesmo
critério do `ronoctua-niri` e do `ronoctua-network`. Os parsers são puros: entra `&str`, sai número.
A raiz entra por parâmetro, como no `foreign::origin_at`, e é o que permite provar a gramática inteira
do `/proc/meminfo` **sem** um `/proc` — inclusive o estouro que um teste achou: a mesma linha que traz
`avg10=1.50` traz `total=2999599462`, e multiplicar isso por dez não cabe em `u32`.

**O total de um grupo não é a soma dos RSS.** Somar RSS mente a favor justamente do que se quer achar:
um navegador com seis processos conta o binário mapeado seis vezes. O kernel já mantém o número certo
por cgroup (`memory.current`), e um escopo é um cgroup por aplicativo por construção. RSS só vale
**dentro** do grupo, para dizer qual dos seis.

#### Duas velocidades, e a fronteira entre elas

O relógio de um segundo lê pseudo-arquivo — microssegundos, dentro do laço que desenha. O que custa
segundos (varrer disco, perguntar ao systemd, contar no journal) vai para thread com cancelamento, ou
para o gesto que o pediu. O `systemctl show` **não** está no relógio: são dois `fork`+`exec` por
leitura, e a armadilha do `systemd-run --scope` síncrono já mostrou o preço de bloquear esse laço.

E o store não é varrido: `nix path-info` responde pelo banco do próprio Nix o que uma caminhada de
minutos re-derivaria.

#### O interruptor tem dois alcances, e `--runtime` é a diferença

Os números dos dois perfis nascem no Nix e chegam ao Rust como **tabela em `/etc`** — o painel os
aplica verbatim, nunca os recalcula (invariante 5).

- **Agora**: `systemctl --user --runtime set-property`. Sem `--runtime` o systemd **grava em disco
  para os próximos boots** — uma segunda verdade permanente brigando com o flake, e ninguém veria.
- **Permanente**: uma unidade oneshot escreve UMA linha no `host.nix`, liberada por uma regra polkit
  que nomeia as cinco instâncias permitidas, verbo `start`, um usuário. O script **recusa** um arquivo
  cuja última linha não seja `}` em vez de editar às cegas.

O manual do systemd avisa que nem toda propriedade aceita mudança em execução, e o teste de VM é o
que decide — ele aceita. **E a escala não era nenhuma das duas óbvias.** `50%` no arquivo de unidade
volta do `systemctl show` como **2147483648**: fração de `UINT32_MAX`, nem porcentagem nem
permiríade. A primeira versão dividia por dez, por acreditar na palavra "permyriad" do código do
systemd; o teste respondeu com o número. E a duração vem **formatada** — `1min`, não os microssegundos
que o nome `…DurationUSec` promete —, então lê-la como inteiro dá zero para todo valor: a tela diria
que a proteção age "depois de 0s".

#### O modo tolerante afrouxa o lado dos aplicativos, e só ele

Teto 70% → 90%, pressão 50% → 70%, duração 15 s → 60 s. A reserva do desktop **não muda**: é ela que
mantém a máquina respondendo, e afrouxá-la seria desligar a proteção em vez de afrouxá-la. O preço vai
escrito na tela: no modo tolerante a máquina pode ficar lenta por até um minuto antes de alguém ser
fechado.

#### Um defeito que apareceu no caminho: o editor nunca salvou

O `effects_from` do runtime não tinha braço para `"save"`. O editor emitia `Effect::Save`, o host do
buffer sabia tratá-lo, e entre os dois o efeito caía em "efeito desconhecido" — um `warn` num log que
ninguém lê. `SPC m s` parecia uma tecla que não faz nada. Uma linha, e um teste que a segura.

### Um arquivo de swap, e o offset dele

O `disko` cria e ativa `/swap/swapfile`. O instalador criava um **segundo**, media o `resume_offset`
dele e o escrevia no `host.nix`: 16 GiB de disco desperdiçados e um offset apontando para o arquivo
que o kernel não usa — uma hibernação que falha em uso, não no boot. O passo saiu; o offset vem do
arquivo do disko.

E faltava `boot.resumeDevice`. Com `boot.initrd.systemd.enable`, o nixpkgs só emite `resume=` quando
essa opção existe: a linha de comando levava `resume_offset=` sozinho, que é meio caminho.



### O console de resgate — `Ctrl+Alt+Del`, e duas portas para a mesma tela

O rodapé é a **única** chrome (inv. 14): morto, não há modeline, minibuffer nem leader. Os três binds
fora da leader cobriam três casos e todos pedem `spawn` ao **compositor** — ou seja, os três somem
exatamente quando o compositor é o que quebrou. Faltava a porta que funciona quando nada funciona.

**O pedido tinha um conflito interno, e ele decidiu a arquitetura.** "Independente do niri e das
ferramentas gráficas" e "matar janela com um clique do mouse" não cabem na mesma tela: o console do
kernel não tem ponteiro. A saída é **um binário, duas portas**:

| | com o niri vivo | sem ele |
|---|---|---|
| quem abre | bind do niri → janela flutuante | kernel → `ctrl-alt-del.target` → VT 8 |
| o que existe | mouse e teclado | teclado |
| do que depende | Wayland, wgpu, atlas de fontes | nada disso |

É a mesma tela, o mesmo binário. Por isso o resgate é uma **aplicação de terminal** e não um cliente
Wayland: uma janela de resgate que precisa do compositor vivo é a chave trancada dentro do cofre.

**Não é chrome** (inv. 14). A porta de cima é um `xdg_toplevel` que o niri flutua por `window-rule`
gerada — exatamente o que o `Ctrl+Alt+Esc` já faz com o btop, com um terceiro `app_id`
(`br.ronoctua.rescue`) porque a regra dele pede janela maior. A porta de baixo não está no desktop.

#### Tomar o `Ctrl+Alt+Del` é assumir o que ele fazia

O systemd entrega `ctrl-alt-del.target` como apelido de `reboot.target`. Uma unidade de mesmo nome em
`/etc/systemd/system` — que é o que `systemd.targets` escreve — substitui o apelido. Um toque deixa de
reiniciar.

Isso é perda real, e é paga duas vezes: **reiniciar é uma linha dentro do console** (que pergunta
antes), e a saída dura do systemd continua intacta — **sete toques em dois segundos** ainda reiniciam
à força, sem passar por nada nosso. O `ctrl-alt-del-is-ours` afirma as duas metades, e a segunda é a
que ninguém pensaria em conferir: se o *burst* também tivesse saído, uma máquina totalmente travada
ficaria sem saída nenhuma, e nada diria isso até o dia.

#### Uma ação é DADO

Cada conserto de um toque é uma linha de tabela: id, chave de texto, o que roda, e se pede senha.
Nada sobre *o que* rodar mora no código que desenha — e é isso que torna a lista auditável do Nix:
`rescue-actions-have-text` lê os ids do Rust e exige a chave nos dois idiomas;
`rescue-polkit-is-narrow` exige que as unidades das linhas privilegiadas sejam **exatamente** as que a
regra de polkit permite, nos dois sentidos. Foi esse segundo sentido que achou, sozinho, que a
unidade do editor morava noutro arquivo.

**Duas faixas, e a linha é a do dono:** ver é livre, agir prova quem é. Reiniciar rodapé, niri ou a
sessão inteira não passa por polkit nenhum — não há display manager aqui, e essas são unidades **de
usuário**. O que toca o sistema, dá shell ou não se desfaz vai para `auth_self_keep`: a senha da
própria pessoa, uma vez, lembrada pela sessão. Quem pergunta é o polkit, com `pkttyagent` no VT — este
crate nunca vê senha, e um console de resgate é o pior lugar possível para errar autenticação.

#### O que ele não faz, e por quê

**Não edita o config do niri.** `/etc/niri/config.kdl` é link para o store e é gerado
(`nix/modules/niri.nix`): editar seria imperativo (inv. 6) e sumiria no rebuild. Ele **abre em
leitura**, dizendo de qual arquivo do flake nasceu — o que a pessoa procura é o arquivo cujo nome ela
sabe, e encontrá-lo junto com a origem é a diferença entre resposta e beco sem saída. O único
arquivo que a máquina de fato possui é o `host.nix`, e esse abre para editar.

**Não tem painéis de terminal lado a lado.** Um terminal é a tela inteira: o app sai da tela
alternada, entrega o terminal ao shell e o retoma quando ele fecha. Isso evita PTY nosso, parser de
VT e um segundo jeito de desenhar terminal — que é justamente a bagagem que um console de resgate não
deveria carregar. O `rescue-has-no-toolchain` cobra: linkar `ronoctua-term` traria `libghostty-vt`, e
com ele Zig, clang e LLVM. Medido: **89 caminhos no closure, nenhum compilador**.

**E não roda como root.** `Ctrl+Alt+Del` não pede login: um console de root a uma tecla de qualquer
máquina bloqueada seria buraco maior do que o que este arquivo fecha.

---

### Impermanência: renomear, não apagar

A tentação é `btrfs subvolume delete @root`. Não fazemos isso.

Se faltar um caminho em `directories`, apagar perde o estado sem recurso e a pessoa só descobre no
boot seguinte. Renomear para `@antigos/@root-<data>-<epoch>` deixa uma janela de recuperação, e os
antigos são coletados depois de `manterDias`.

O teste prova as três coisas juntas, porque cada uma sozinha passaria com o wipe quebrado: o que
estava em `/` sumiu, o que estava em `/persist` ficou, e o `@root` velho continua montável.

Duas armadilhas: **`boot.initrd.postDeviceCommands` é ignorado com initrd do systemd** (que o
lanzaboote exige), e **`mv origem destino` com destino existindo move a origem para dentro dele** —
dois wipes no mesmo segundo davam `@root` aninhado, sem erro.

---

### Instalador: catorze perguntas, e o "só" continua sendo o critério

A TUI coleta idioma, disco, instalação, o trio do teclado, locale, fuso, hostname, senha,
frase-secreta, a rede sem fio e o `resume_offset`. O "só" é a parte difícil, e por isso a lista é um
enum fechado com um teste que conta os casos — acrescentar uma pergunta quebra o teste, que é como
uma décima quinta para de entrar por desatenção.

Eram seis, e cada uma que entrou desde então entrou por não ter alternativa: **o teclado**, porque o
xkb não recusa layout inexistente e a frase-secreta é digitada às cegas; **o locale e o fuso**, porque
são do lugar e não do hardware; **a rede**, porque sem ela a instalação não roda numa máquina sem
porta Ethernet; e **a instalação**, abaixo.

O que **não** se pergunta — vídeo, sistema de arquivos, tamanho de partição, gerenciador de boot,
hibernação — ou o `nixos-facter` detecta, ou a distro já decidiu.

- **O disco é a primeira pergunta.** Ela apaga dados; perguntar por último faria a pessoa investir
  cinco respostas antes de descobrir que escolheu errado.
- **A passphrase conta CARACTERES, não bytes.** Doze com acento passam de doze bytes, e recusar
  puniria quem escreve no próprio idioma. O mínimo é 12 porque o disco **não tem limite de
  tentativas** — a única defesa é o custo do argon2 vezes o comprimento.
- **`--execute` não é o padrão.** Um comando que apaga um disco por omissão é um comando perigoso.
- **A confirmação pede o caminho do disco**, não "sim". Copiar o caminho obriga a lê-lo.

**Nenhum segredo passa por `argv`** — `/proc/<pid>/cmdline` é legível por qualquer processo. A
passphrase vai para um arquivo 0600 (**sem `\n` final**: o `cryptsetup` usa o arquivo inteiro como
chave, e a quebra entraria na frase), e a senha entra no `mkpasswd` por `stdin`.

O `host.nix` gerado **não leva segredo**: descreve a máquina e vai para backup. E o hash da senha é
escrito **antes** do `nixos-install` — com `mutableUsers = false` o ativador o lê durante a
instalação, e depois seria tarde.

#### Duas instalações, e por que é uma OPÇÃO e não um segundo sistema

`ronoctua.profile` vale `full` ou `minimal`, e a diferença é medida: **22,8 GiB contra 6,9**. Sai o
Android (6,5), saem os compiladores (4,0), saem as variantes da fonte (2,0), saem o navegador, o
editor e a mídia (2,0).

**É uma opção, e não um `nixosConfigurations` a mais.** O instalador continua mirando `#noctua` nos
dois casos; o perfil é uma linha no `host.nix`. Isso decide três coisas de uma vez: quem se arrepende
faz `nixos-rebuild` em vez de reinstalar, o alvo do instalador continua sendo um só, e a tabela que
liga perfil a chaves fica num arquivo (`nix/modules/profile.nix`) em vez de virar duas listas de
módulos que divergem.

Todas as chaves entram por `lib.mkDefault`, porque a tabela é a voz mais fraca da sala: `host.nix` e
`vm.nix` continuam vencendo.

**A reduzida não é outra distro.** Niri, rodapé, fonte primária, teclado, rede, impermanência e
terminal ficam. O que sai é aplicativo — o que se instala para USAR, não o que é preciso para entrar.

Duas coisas apareceram ao medir isto, e as duas eram interruptores que não interruptavam:

- **`ronoctua.fonts.notoExtra` não fazia nada.** O `noto-fonts` estava na lista incondicional *e* na
  opcional; o `vm.nix` desligava acreditando poupar, e o pacote entrava pelo outro caminho.
- **`ronoctua.icons.superpack` não tira os 0,61 GiB do closure.** O `ronoctua-shell` carrega o
  caminho do superpacote GRAVADO por `option_env!` (`RONOCTUA_ICONS`), e é instalado nos dois
  perfis. Por isso ele está **fora** da tabela: listá-lo seria anunciar uma economia que não
  acontece. Fechar isso é mudança no shell, não na tabela — está no `PENDING`.

#### O ISO carrega o que a máquina alvo compilaria

Instalar custava uma tarde, e o motivo não era o download. `nixos-install` **constrói** o sistema: o
que vem do nixpkgs chega pronto do cache público, e o que este repositório produz não existe em cache
nenhum — o shell, o niri do input pinado, o `libghostty-vt` com o Zig, os buffers wasm, a
família de fontes que vem de um Release do GitHub. São exatamente as compilações que o `earlyoom`
deste host mata por peso.

A semente vai no `isoImage.storeContents`, e o `iso-image.nix` roda `nix-store --load-db` no boot —
os caminhos chegam **válidos**, e não como arquivos que por acaso estão na mídia. Junto vão as
FONTES dos inputs do flake: sem elas o alvo tem as peças prontas e ainda assim precisa da rede para
avaliar o flake que as nomeia.

**Por que a semente não é o sistema reduzido inteiro.** Foi tentado e medido: o ISO deu 4380 MiB
contra o teto de 4096. Desses, ~3,5 GiB são nixpkgs de prateleira — a parte que um cache serve em
minutos. Levar só o que ninguém tem em cache deu **3702 MiB**, e o alvo continua não compilando nada.

O que sobrou de comparação honesta: a máquina alvo agora *baixa* onde antes *compilava*.

#### O ISO carregava 2,94 GiB que não eram dele

Ele instalava `rust.workspace`, a derivação que contém TODOS os binários do workspace. Uma derivação
referencia o que os bytes dela citam, e o `ronoctua-shell` cita o `libghostty-vt` — que arrastava
Zig, clang e LLVM (1,70 GiB) — mais o superpacote de ícones e o mesa, pelos caminhos de
`option_env!`. Um instalador de console não roda nada disso.

A correção é extrair só `bin/ronoctua-install` (`nix/rust.nix`): o Nix varre a SAÍDA, e o que os
bytes copiados não citam deixa de ser referência. O closure do instalador caiu de 5,81 GiB para 2,87
e o ISO de 2,1 GiB para 1,46 — que foi o orçamento que a semente gastou.

O mesmo defeito estava no desktop, por outra porta: `libghostty-vt.a`, a biblioteca **estática**, era
o único arquivo daquela saída que citava um caminho do Zig, e nós linkamos a dinâmica. Apagá-la tirou
1,70 GiB de **toda máquina instalada**. O check `iso-has-no-toolchain` é o que impede a volta calada.

#### O sistema que o instalador instala nunca era construído

O gate construía `nixosConfigurations.vm` e o ISO. Nenhum dos dois é o `noctua` — os `drvPath` são
diferentes, e o alvo real ficava de fora. "A closure do sistema fecha" era suposição, e o momento em
que ela seria desmentida é durante o `nixos-install`, **com o disco já apagado e nada para onde
voltar**.

Não é duplicata do VM: aquele existe para um teste bootar e deixa de fora o que uma máquina de
verdade carrega — o Android Studio, o SDK, o Flutter. É justamente essa parte que falta no outro.

O custo é honesto: o gate passa a construir a closure de desenvolvimento inteira. É caro uma vez por
mudança no sistema, contra um `nixos-install` que falha no meio pela primeira vez na vida de alguém.

### Por que existe um diretório de trabalho

O `disko` precisa saber **qual disco apagar**, e esse valor mora no `host.nix`, dentro do flake. No
momento em que ele roda, os dois lugares óbvios para o flake são impossíveis: `/mnt/etc/nixos` ainda
não existe — é o `disko` que vai montá-lo — e o `/etc/nixos` do ISO é link do store, somente-leitura.

Daí a sequência: o flake é copiado para `/tmp/ronoctua-flake`, o `host.nix` é escrito ali, o `disko`
avalia **esse** diretório, e só depois de montar o alvo é que tudo vai para `/mnt/etc/nixos` — agora
com o `resume_offset`, que só existe depois do swapfile.

O disco poderia chegar ao `disko` por linha de comando, e chegava. Mas o `nixos-install` seguinte
precisa do **mesmo** valor e não aceita um: ele lê o flake. Ter as duas fontes é ter duas verdades.

Isto não é refinamento: sem ele o caminho `--execute` nunca poderia ter rodado. O `nixosConfigurations.noctua`
que os dois comandos citam **não existia**, e o modo ENSAIO não acusava nada — ele *imprime* os
comandos em vez de executá-los. É a mesma classe do check escrito contra o código, e não contra o
critério.

---

### Secure Boot e TPM2

```
chaves → UKI assinada → firmware mede (PCR 7 e 11) → measured-uki
       → tpm2-measure-pcr passa a valer → PCR 15 estendido com a chave do volume
```

Nenhum elo pode ser pulado, e cada um custou uma descoberta:

- **`pkiBundle` não pode estar no store** — chave privada no store é chave pública. Vai para um
  `/etc/secureboot` real, criado na ativação.
- **O `sbctl` escreve nesse mesmo diretório**; com links para o store ele para em "permission
  denied", numa mensagem que fala de root e não de permissão de arquivo.
- **O `GUID` não pode ter quebra de linha:** `invalid UUID length: 37` são os 36 mais o `\n`.
- **O OVMF padrão não mede.** `bootctl` diz "Secure Boot: enabled" e todos os PCRs ficam zerados —
  precisa de `OVMF.override { tpmSupport = true; }`.

**O que o PCR 15 protege, de fato.** Copiar o header LUKS é *clonar*, não forjar: o clone tem a mesma
chave de volume, o PCR 15 mede idêntico e destrava corretamente. O PCR 15 não impede clonagem — ele
amarra o estado **pós-destrave** ao volume que foi realmente aberto.

---

### O cérebro — `LEADER b b`, e a linha que avisa antes

A máquina faz coisas sozinha, e até esta tela nada dizia o quê, quando, nem no que deu. `nix flake
update` era um ALIAS de shell; o rebuild morava no `rescue.nix`, a coleta de lixo no `memory.nix`, o
cache do tldr no `cli.nix`, e o `fc-cache` dentro de um crate Rust. Cinco arquivos, cinco
convenções, e nada que rodasse os cinco em ordem.

#### O que ele é: a mesma classe do `ronoctua-transfers`, um andar acima

`ronoctua-brain` é um daemon de usuário com relógio, pulso, fila e um registro de tarefas atrás de um
`Mutex` com `Condvar` e um contador de `version`, publicado INTEIRO num socket de JSON por linha
(`$XDG_RUNTIME_DIR/ronoctua-brain.sock`, um por USUÁRIO). O argumento é o da fila, um nível acima:
uma cópia de 40 GiB não pode morrer com o rodapé, e uma decisão sobre quando a máquina se reconstrói
também não — o rodapé reinicia a cada edição de arquivo no laço aninhado e a cada falha na máquina
instalada. E são três processos querendo a mesma resposta: a modeline, a tela do cérebro na região
expandida, e a janela em que essa tela abre.

**Ele não desenha, não segura teclado, não abre `/proc` e não reimplementa Nix.** O que está na tela
é um componente `wasm32-wasip2` em `buffers/brain/`, desenhado pelo rodapé como qualquer outro. Ele
não sabe o que uma atualização É: nomeia uma unidade e lê o que a unidade relatou (invariante 5).

#### A porta é o trait `Task`, e ela promete cinco lugares

Este é o ponto do crate. O dono pediu um lugar onde *"qualquer coisa nesse espaço"* vire
funcionalidade em vez de um daemon novo, e um lugar só é um lugar se os passos para usá-lo estiverem
escritos. Acrescentar uma tarefa toca **cinco**: um tipo em `src/tasks/`, uma linha no `vec![]` que
os lista, as duas chaves de texto nos dois idiomas, uma linha na página inicial do buffer (se ela
quiser uma) e um padrão em `prefs` (se ela tiver preferências). O que ela NÃO escreve, porque o
runner já é dono: a fila — no máximo uma tarefa por vez —, o portão de métrica, o aviso e a contagem
dele, o diário, o socket e toda tela que olha.

O trait tem seis métodos e **só o `run` toca a máquina**. Os outros — como se chama, quando vence,
do que precisa, por quanto tempo avisa, quais são os passos — não recebem relógio, não abrem arquivo
e não sobem processo. É isso que permite provar um ANO de agendamento em milissegundos, com um
`Clock` de teste, sem esperar um segundo sequer.

#### Não há uma segunda fronteira com o `/proc`

O `ronoctua-memory` continua sendo a única — o mesmo critério do `ronoctua-niri` e do
`ronoctua-network`. Quem cresceu foi ele: ganhou `cpu_pressure()` e `on_battery()`, e o cérebro recebe
o pulso já lido, por um trait (`pulse::Meter`). A alternativa seria o cérebro abrindo `pressure/cpu`
e
`class/power_supply` por conta própria — a segunda fronteira que este repositório recusa em todo
canto, e que aqui seria uma fronteira nova dentro do processo que decide quando reconstruir o
sistema.

#### Uma capacidade nova, `brain`, e por que não a `system`

Já havia a `system`, e ela é a do painel de memória. Mas `system` quer dizer "quanta memória há e
quem a está comendo"; esta termina com a máquina **reconstruindo o próprio sistema operacional**.
Dobrar uma na outra daria esse alcance ao painel de memória, que não pediu por ele. O WIT já faz
esse mesmo argumento duas vezes, para `transfers` e para `disks`, e a `brain` diz no próprio
comentário que é o mesmo argumento um passo adiante — são sete capacidades, e nenhuma delas é "e
mais o que couber".

#### `tune` é efeito, e não demanda

Uma `demand` é verbo mais ÍNDICE e não carrega valor nenhum: *aja na terceira linha*. "A cada sete
dias" é um valor. Por isso a preferência atravessa como `effect::tune(setting)` — um `{ task, knob
}`, com o `knob` sendo intervalo, parte do dia, ligado, passo ou "volte aos padrões" — e não como
mais um verbo na lista das demandas. É a mesma fronteira que separa "escolha esta" de "passe a valer
isto".

#### Dois defeitos do protocolo, achados na entrada

Nenhum dos dois é do cérebro. Os dois apareceram porque ele foi o primeiro a acrescentar uma
capacidade depois que a família delas já existia.

- **O `Request::requires()` prometia, no próprio comentário, que um pedido sem braço não
  compilaria** — e logo abaixo tinha um `_ => Capability::System`. As duas frases moravam no MESMO
  comentário, contradizendo-se. Um pedido novo não quebrava nada: ganhava calado a capacidade mais
  larga que existe.
- **O `Effect::requires()` tinha a mesma forma, um tom mais baixo, e pior**: o curinga era
  `_ => None`, isto é, um efeito não listado exigia capacidade NENHUMA. Falha silenciosa que falha
  ABERTA, no portão que guarda o que um buffer FAZ e não o que ele desenha.

Os dois são exaustivos hoje — vinte e um braços num, nove no outro.

#### O cérebro declarava a atualização DUAS VEZES

Este é o defeito mais caro da entrada, e ele já estava desenhado. A tarefa do cérebro carregava três
passos escritos à mão — `update-inputs`, `build`, `collect-garbage` — enquanto o
`assets/updates.nix` declara doze, nomeados `flake-update`, `system-rebuild`,
`store-collect-garbage` e mais nove. Não era um subconjunto: os dois conjuntos são **disjuntos**.
Todo nome de passo na tela era um nome que a sequência não usa — e a linha do "parou em" tira o id
do relatório que a UNIDADE escreve, então uma falha em `flake-update` procuraria uma chave que não
existe e desenharia a chave crua na frente de quem foi olhar por que a atualização não terminou.

Hoje a tarefa lê a tabela gerada: o `build.rs` traz para o `OUT_DIR` o `updates.rs` que o Nix
produziu (por `RONOCTUA_UPDATES_RS`), e um `include!` o traz para dentro do crate. Uma fonte, e a
lista na tela é a lista que roda.

#### A sequência: uma fonte, quatro produtos, e o invariante 6 resolvido por construção

De `assets/updates.nix` saem a tabela Rust, as chaves de texto que os dois idiomas têm de carregar,
os dois roteiros de shell — o do root e o da pessoa — e uma recusa em tempo de build com o id do
passo na mensagem. Escrita como script, "quais passos existem?" seria um grep por cinco arquivos, e
a resposta sairia errada.

O dono pediu os gerenciadores de linguagem junto — rust, go, js, npm —, e o `nix/modules/dev.nix`
abre nomeando esses mesmos gerenciadores como a coisa que esta distro recusa. As duas metades ficam,
e a costura é estrutural em vez de ser convenção:

1. **Todo passo declara uma PRECONDIÇÃO**, e um passo cuja precondição é falsa é pulado E DITO —
   nunca pulado em silêncio, que é a única forma de um passo declarado ser honesto sobre uma máquina
   em que ele não cabe.
2. **Nenhum passo pode INSTALAR.** Atualizar o que já está lá não é gerência imperativa de pacote; pôr
   o que não está, é. O gerador recusa o substrato `install`, cada verbo de instalação que ele conhece,
   todo metacaractere de shell e toda forma de escrever "passe isto por um shell" — não sobra
   argumento por onde um se esgueire.
3. **Passo que nomeia um BINÁRIO NU** — um que a flake não põe na máquina — **não pode nascer
   ligado**, não pode rodar como root, e tem de declarar o próprio pré-requisito em `needs`, para que
   a máquina que não tem a ferramenta ganhe um passo pulado e não uma falha. Ligá-lo é uma linha no
   `configuration.nix` da máquina onde o dono pôs a ferramenta à mão — a mesma forma da saída que o
   `dev.nix` já sanciona para o `~/.shellrc.local`.

E há uma quarta lei, que é de ordem: exatamente um `move-inputs` e exatamente um
`apply-configuration`, o primeiro antes do segundo, todo `reclaim-space` depois — e root primeiro,
pessoa depois, nunca intercalados. Intercalar custaria uma ida ao polkit a cada troca, e uma
sequência cuja ordem muda quando alguém vira um `root =` é uma sequência que ninguém prevê.

São **quarenta e uma** recusas no gerador ao todo, e **duas delas são provadas** por uma fixture
negativa: um passo que nasce ligado citando binário de fora, e um passo que instala. O check não se
contenta com "o build caiu" — ele cobra as duas frases, *invariante 6* e *instala*. Sem isso, "o
gerador recusa" seria uma afirmação não testada sobre o único código deste repositório que decide o
que roda na máquina de alguém.

#### Nada que ele faça pode PERGUNTAR

Não há agente polkit na sessão gráfica. A regra de que ele depende nomeia duas unidades, um verbo
(`start`) e um usuário, e devolve `polkit.Result.YES` — nunca um `AUTH_*`. Nenhum dos dois roteiros
invoca `sudo`, `pkexec`, `pkttyagent` ou `systemd-run`, e o `systemctl` é achado por CAMINHO DO
STORE, porque o `PATH` de uma unidade é mínimo e um nome falha longe da causa.

**E a razão é do DONO, não do desenho.** Às vezes o leitor de digital pede que ele aprove alguma
coisa, e ele não vê o pedido, ou está longe do laptop. Não é que ele recuse — é que não teve como
aprovar. Isso decide a funcionalidade inteira: o cérebro existe para rodar a atualização num BOM
MOMENTO, e um bom momento é quase sempre um em que ninguém está olhando para a tela. Ali um pedido
não é uma pergunta, é uma falha silenciosa com o formato exato de uma recusa — e o relatório diria
"você não quis" sobre uma coisa que ninguém viu. Sem agente, um `AUTH_*` já falharia; mas falharia
por ACIDENTE, e o `updates-never-ask` transforma a ausência em requisito.

#### O laço esperava passado a versão de ANTES da própria passada

O dono relatou a tela: o buffer do cérebro com **muito** lag, a janela deixando o minibuffer por
cima em vez de encolher, `Enter` levando segundos para trocar de página, e `LEADER w d` levando
segundos para a janela sumir. E a frase que fecha o diagnóstico antes de ele começar: *"a janela
deveria responder imediatamente, independente de o buffer estar com lag ou não."*

**O buffer não estava lento.** Medido, com o `nix run .#png` aquecido: 203 ms, o mesmo que a
memória, o formatador e a fila. A primeira medição disse 10 s e estava errada — ela pagava uma
avaliação de flake que as outras já tinham pago.

O que estava errado era o `Runner::serve`. Ele fazia a passada e então esperava a versão passar de
`seen` — só que `seen` era lido **antes** da passada, e uma passada ESCREVE. O `wait_past` recebia
um número que a própria passada já tinha ultrapassado, voltava na hora, e pumpava de novo: um giro,
na velocidade que o processador desse. Medido com o defeito replantado: **248 088 passadas em
300 ms**.

Nada quebrou e nada foi para o log. O que apareceu foi a JANELA, porque cada volta do giro acordava
todo assinante de `Ask::Watch` com uma `Mind` inteira — e do outro lado daquele socket está o laço
que desenha. A janela não estava lenta para redimensionar: ela não tinha tempo sobrando para o
Wayland.

**Dois consertos, porque um não bastava.** O laço passou a esperar passado as próprias escritas; e o
`Shared::change` parou de tocar o sino quando nada mudou — o `put_off` roda em toda passada e tira
um `postponed` que quase sempre é `None`, e só isso já era um `+1` de versão, duas vezes por
passada, para um não-evento.

**E o hospedeiro passou a se defender**, que é a metade que responde ao dono. Uma `mind` igual à
anterior agora morre numa comparação e não chega à conversão do WIT, nem ao componente, nem a um
quadro. É a regra que o `buffers/memory` já enuncia sobre a amostra dele — chega no relógio, e a
tela só se mexe quando MUDOU. O que isso não faz, e não vale prometer: o hospedeiro do buffer é um
laço `calloop` só, de propósito, então um `render` genuinamente lento ainda segura a janela. O que
ficou verdadeiro é que nada naquele laço tem permissão de ser lento.

#### E a unidade que o cérebro pedia era de USUÁRIO, sem `--user`

Achado puxando o mesmo fio. `nix/modules/update.nix` declara `ronoctua-update.service` em
`systemd.user.services`, e o `systemd::argv` do cérebro montava `systemctl --no-block start …` sem
`--user` — no barramento de SISTEMA, onde ela não existe.

E não falharia só: no barramento de sistema o `systemctl start` é autorizado pelo polkit **antes** de
o systemd procurar a unidade, e não há agente polkit na sessão. A chamada ficaria pendurada até
estourar, levantando um pedido de autenticação que ninguém pediu — a mesma forma de defeito que um
teste de unidade deste repositório já tinha levantado, e que o dono relatou por ter sido perguntado
cinco vezes. Este dispararia a cada atualização vencida, numa máquina em que ele pode não estar
sentado.

#### A faixa é a superfície do rodapé crescendo PARA BAIXO

Espelho da faixa de notificação, que cresce para cima. Uma superfície só, sem popup e sem card
(invariantes 14 e 15), com o glifo do cérebro na margem dizendo quem está falando — ela chega sem
ninguém ter pedido.

Ela é **colada embaixo do minibuffer**, e não a última linha da superfície. O terceiro motivo é o
que decide: uma linha verdadeiramente última moraria em `first_of_expansion() + expansion_rows()`, e
teria de ser recalculada no pintor, no hit-test e no quadrilátero de fundo, para sempre — que é
exatamente o defeito que o `first_of_expansion()` foi escrito para remover. Colada, ela custa **um
termo** (`band_rows() + FOOTER_LINES + brain_line()`), e todo leitor rio abaixo acompanha de graça.
Os outros dois motivos são que foi assim que se pediu, e que ela mantém o rodapé simétrico — faixa ·
modeline · minibuffer · cérebro · expansão —, de modo que esta linha se mexe na tela exatamente
quando a modeline se mexe.

#### A contagem pega carona no relógio de um segundo que o rodapé já bate

O cérebro manda o aviso UMA VEZ, e o que atravessa é o PRAZO; quem desenha calcula `deadline - now`.
Um segundo cronômetro para o mesmo segundo acordaria o laço duas vezes por um efeito só. E a pizza é
a
`band::countdown_glyph`, a MESMA da faixa de notificação: são duas contagens na mesma tela, e quem
aprendeu uma aprendeu a outra.

#### `Ask::Abort` não para uma contagem, e por isso são dois pedidos sob um comando

Enquanto o aviso corre, a tarefa está apenas na FILA, e o runner matricula tudo que está vencido a
cada batida. Tirar da fila não é o que a para: meio minuto depois ela volta, *"e a pessoa
responderia a mesma pergunta a cada meio minuto"*. O que move o próximo momento é o ADIAMENTO, que o
runner anota como evento. Então `brain-abort` é um comando só e dois pedidos — `Postpone` enquanto
nada está rodando, `Abort { task }` depois que alguma coisa está. Quem escolhe é uma função de três
linhas, e não quem aperta a tecla.

#### A cor é a única de fundo com matiz, e é um passo PARA O LADO

`surface.base` (`0x000000`) → `surface.chrome` (`0x121212`) → `modeline.background` (`0x1E1E1E`) →
`notification.background` (`0x242424`) é uma escada que o olho sobe, e cada degrau quer dizer "mais
uma camada de chrome". O `brain.background` é `0x161C28` — 22, 28 e 40 —, na mesma vizinhança de
luminância da modeline, com um viés de azul: não é mais uma coisa por cima, é um TIPO de linha
diferente. É o único fundo do arquivo que carrega matiz, e é esse o ponto — a máquina está prestes a
fazer alguma coisa consigo mesma, e a linha que diz isso não pode se ler como uma modeline que mudou
de ideia. O branco dela também é o único quase-branco neutro do arquivo, e não sai desta linha: o
corpo do texto é quente de propósito, e sobre um fundo azulado um branco quente se lê como sujo.

### Energia — `LEADER s p`, `LEADER q`, e um glifo que só aparece quando diz alguma coisa

Quatro daemons contam a história da energia — UPower (bateria, carregador, periféricos), o
power-profiles-daemon (perfil), o logind (dormir, tampa, inibidores) e o sysfs (limites de carga) —,
e **um crate os fala todos**: `ronoctua-power`, molde do `ronoctua-disks`. Cada serviço é lido por
conta própria e sua ausência é a SUA parte vazia no `Snapshot`; um desktop sem bateria e uma VM sem
PPD desenham a vista com o que existe e dizem o que não existe (`power-no-battery`,
`power-no-daemon`), nunca um erro. Três threads, cada uma com a própria conexão: `upower` (que cobre
o PPD pelo `path_namespace` de `/org/freedesktop/UPower`), `login` (`PropertiesChanged` e
`PrepareForSleep`) e `orders`. **Os fds dos inibidores vivem na thread das ordens** — fechar o fd é
soltar o inibidor, então quem o segura tem de ser o processo que sobrevive: a janela nativa manda
qualquer outra ordem direto ao seu `Handle` (PPD, logind e sysfs não têm estado), mas *manter
acordado* ela encaminha ao rodapé por `ronoctua-shell command`. E os dois interruptores são lidos de
`ListInhibitors`, nunca da memória: reiniciar o rodapé solta o fd, e a linha tem de dizer.

**Nada pergunta**, pela mesma razão do udisks: não há agente polkit na sessão. Toda chamada ao logind
vai com `false` no `interactive`, e o check `power-never-asks` lê o crate e recusa `true`, `pkexec`,
`sudo` e `systemd-run`. Nenhuma regra polkit nossa (`power-adds-no-polkit-rule-of-its-own`): dormir,
hibernar, reiniciar, desligar e inibir são `allow_active: yes` no logind 260, e `switch-profile` no
PPD 0.30. A única coisa que o upstream não entrega é o sysfs: `charge_control_start_threshold` e
`charge_control_end_threshold` são `root:root 0644`, e `GROUP=`/`MODE=` de udev só alcançam nós de
dispositivo. A regra usa `RUN+=` com o `chgrp`/`chmod` do store sobre `/sys$devpath/<atributo>`,
para o grupo `users`, e `power-thresholds-rule-lands` confere a FORMA dela — no
`services.udev.extraRules` AVALIADO, não no arquivo: o alvo (`/sys$devpath/<atributo>`), o grupo, o
`g+w` e o `TEST==` que a limita à bateria que tem o arquivo. Regra com o verbo certo e o caminho
errado não entrega nada, e passaria sem isso. Quando a regra não chegou (outro sistema, firmware sem o atributo) a vista escreve
`readonly` na seção e desenha os presets desabilitados — alcançáveis e recusando, o molde do XFS —,
e o `RET` nunca falha em silêncio. A escrita segue a ordem que o kernel aceita (`end` antes ao subir,
`start` antes ao descer) e a verdade RELIDA substitui o que se pediu.

**O glifo da modeline só aparece quando diz alguma coisa.** Bateria a 99% ou 100%, ou em
`FullyCharged`, não desenha nada: com limite de carga a 80% o UPower diz `FullyCharged` a 80 para
sempre, e um glifo que nunca sai não informa. O número (`NN%`) só entra a ≤30, a cor de perigo a
≤15 — pela regra LITERAL, carregando ou não, a mesma que o Bluetooth usa —, e a escada tem cinco
degraus por PALAVRA (`battery-empty`…`battery-full`, com a variante `-charging-`), o precedente do
`wifi-*`. O perfil segue a mesma lógica: a marca só aparece quando o ativo NÃO é o topo do que o
daemon oferece — num desktop cujo PPD só lista `balanced` e `power-saver`, o `balanced` em repouso
fica mudo, senão o glifo seria permanente. As três constantes (`HIDE_FROM`, `PERCENT_UNTIL`,
`DANGER_UNTIL`) moram no crate e a regra é uma função pura (`mark`), testada em 98/99/100, 30/31 e
15/16.

**O rodapé só aplica o que ALGUÉM NOMEOU.** O sistema instalado nomeia sempre — o módulo põe
`RONOCTUA_POWER_PROFILE` na unidade do rodapé e na sessão —, então a única máquina onde o desejo cai
no chão é a que não tem esta distro embaixo: o laço aninhado do `nix run .#dev`, que roda no
computador do dono e fala com o power-profiles-daemon DELE. Sem essa trava, abrir o laço de
desenvolvimento trocava o perfil que o dono estava usando (`prefs::Source::Default` não é palavra de
ninguém). O mesmo vale quando o arquivo é apagado e o desejo volta ao chão.

**"Desempenho" é o padrão da distro, e o rodapé é quem o aplica.** `ronoctua.power.profile` sai como
`RONOCTUA_POWER_PROFILE` na unidade do rodapé e no `sessionVariables`; o arquivo
`$XDG_CONFIG_HOME/ronoctua/power.json` vence a variável, e a variável vence o chão. No primeiro
snapshot com PPD o rodapé pede o perfil querido se ele é oferecido, senão o topo do oferecido — e diz
UMA vez `echo-power-profile-not-offered` (é o que a VM prova: `performance not offered, applying
balanced`). Com `holds` de outro programa ele não briga. O arquivo é o broadcast, como `theme.json`:
as duas portas o vigiam por inotify — o rodapé APLICA a mudança feita por fora, a janela só move a
marca "(sua escolha)". **Escolher é ordem E arquivo** (`Errand::Choose`, nas duas portas — `RET` na
linha, ou as setas girando os oferecidos): uma ordem só valeria até o próximo início do rodapé, que
aplica o arquivo por cima. E o rodapé anota o desejo ANTES de gravar, porque o inotify vê a própria
escrita como a de qualquer um — sem isso uma escolha eram duas ordens, duas releituras e dois ecos.

**A vista é UMA lista com cursor, e os cabeçalhos são linhas.** Oito seções (bateria, carregador,
perfil, limites, manter acordado, tampa e ocioso, ações, periféricos), `RET` faz o que a linha diz,
`←/→` giram o perfil ativo pelos oferecidos e movem os limites — DENTRO do admissível, nos dois
sentidos (oito toques de 40/80 davam 40/40, e o `RET` mandava um par que o kernel recusa). **Linha
desabilitada RECUSA**: perfil não oferecido, limite sem a regra udev, ação que o logind nega — cada
uma desenhada apagada dizendo o porquê, alcançável, e o `RET` nela não manda nada (mandar só traria
a recusa do daemon de volta como eco). Ação destrutiva **ARMA**: o primeiro `RET` troca a linha por
`power-action-confirm` — *"suspender? RET confirma, Esc cancela"* — e a mesma frase é PREFIXADA à
dica gerada, que segue dizendo o resto (a linha pode estar fora da página; a dica nunca está); só o
segundo `RET` manda. É um
mecanismo de confirmação sem nada flutuar (inv. 14), e `LEADER q s/h/r/p` não agem às cegas — abrem
a vista com o cursor na linha e já armada — e **só arma o que o logind faria**: numa máquina sem
hibernação o `q h` abre a linha apagada, sem pergunta, porque pergunta que o `RET` não responde é
pior que nenhuma. Ação que o logind nega (`Can* != yes`) é desenhada desabilitada dizendo `power-cannot`
("não disponível"); o porquê, quando existe, é a frase da seção (`power-no-daemon`, `power-thresholds-readonly`). `Esc` volta UM nível: editando → larga; armado → desarma;
senão fecha. **Valor curto fica na ponta direita da linha**: com a coluna do valor a um terço, "keep
awake: no sleep, no idle action" virava "keep awake: no s…" ao lado de um `off` de três letras —
foi a janela a 57 colunas que mostrou. **A palavra do daemon é FALADA** (`power_words::daemon_word`):
o logind diz `hybrid-sleep` e o UPower `HybridSleep`, e a tela dizia os dois crus no meio da frase
traduzida ("a máquina vai Hibernate sozinha"); o vocabulário é FECHADO, cada chave é constante (texto
que falta é erro de compilação), e palavra de fora aparece como veio. Periféricos com bateria (o
`!PowerSupply` do UPower) são da última seção e nunca da modeline: o teclado é assunto do Bluetooth.

**O tamanho do glifo de chrome é MEDIDO, e `font.size.icon` virou TETO.** O 22 já era uma medição
— o sino tem 600 unidades de tinta contra 751 do dígito, e 22 px é o que os iguala —, mas a bateria
vertical do Material tem 908: mais alta que o dígito no tamanho do TEXTO, e a 22 px ficava 48% acima
do número ao lado (o check `chrome-icons-match-the-text` a pegou; nenhuma bateria plana do arquivo
cabe na faixa, e a horizontal do FontAwesome tem metade da altura). A regra foi escrita uma vez, em
`gen-glyphdb.py`, que já abre a fonte: o tamanho do ícone, a menos que ele ponha a tinta do glifo
acima da faixa do check (`chromeTolerance`, os 15% para cada lado, agora num lugar só) — aí o
tamanho que leva a tinta à do dígito. 22 para o sino, 15 para a bateria, e todo desenho que já
passava fica exatamente onde estava — e a regra alcança só o que é desenhado CENTRADO na célula:
o glifo à frente de um segmento da modeline, da faixa e da linha do brain, e os dois chevrons; linha
de listagem é texto no tamanho base e não muda. Sai no `icons.json`, vira `tokens::icon::chrome_size`
(só as exceções escritas, agrupadas por tamanho) e é o número que o check remede. Nenhuma lista à
mão: a fonte é a autoridade, e um glifo novo de outra proporção cai no tamanho certo sozinho.

**Bateria baixa é notificação por TRANSIÇÃO**, levantada só pelo rodapé (`alarms(antes, depois)`):
`battery-low` ao cruzar 15 descarregando, `battery-critical` ao cruzar 8, `battery-charged` ao
chegar a `FullyCharged` — uma por cruzamento, e nunca de novo enquanto fica. O UPower é alinhado ao
mesmo número (`percentageLow = 15`) e `criticalPowerAction = "Hibernate"` porque o swapfile e o
`resumeDevice` já existem; `/var/lib/upower` persiste para o histórico que dá o `TimeToEmpty`.

### A saída — três marcas, e a que só existe porque o daemon foi ensinado a olhar

O `ronoctua-network` ganhou um `Snapshot` INTEIRO (a saída, o avião, os links, as conexões ativas) e
um `watch` que roda desde a partida, porque a modeline desenha com vista nenhuma aberta. São três
marcas, e cada uma diz algo que as barras do Wi-Fi não dizem: as barras dizem quão alto o roteador
fala; estas dizem se existe alguém do outro lado.

**`net-off` é um link que não leva a lugar nenhum** — o portal do hotel, o roteador sem uplink. NÃO é
"não há internet": um laptop com o cabo fora e o rádio desligado não tem saída e não há nada de
errado com ele, então a regra pede um link que se acha conectado (`Snapshot::adrift`). E ela só pode
existir porque o módulo LIGA o teste de conectividade do NetworkManager: sem `connectivity.uri` o
daemon responde `FULL` para qualquer rota, e portal cativo e uplink morto ficam idênticos a
"funciona". O custo — um GET a cada cinco minutos para um servidor que a pessoa não escolheu — está
escrito na opção, que pode ser desligada; o que se perde é a marca.

**`airplane` é decisão de alguém**, e é a única das três que é INTERRUPTOR: clicar sai do modo avião,
que é a única coisa que alguém quer fazer com ele. **`vpn` é um túnel de pé**, e ler só o `Vpn` do
NetworkManager seria o defeito: o WireGuard responde `false` ali (para o NM ele é um DISPOSITIVO) e é
justamente o que a maioria das pessoas chama de VPN hoje — `Active::tunnel` lê os dois.

As três abrem a vista da rede, e a vista DIZ o que a marca queria dizer: uma linha para a saída, uma
para o avião (com `RET` que volta), uma por túnel (com `RET` que derruba). Marca que abre uma tela
que não fala dela seria uma porta para uma parede.

### A vista da rede — uma `View`, duas portas, e oito seções que são linhas

O molde é o da energia, e a razão é a mesma: o rodapé tem teto de doze linhas (inv. 14) e esta vista
tem nove seções. `s n n` desenha na região; `s n w` abre a mesma `View` numa janela
(`br.ronoctua.network`). O que a `View` guarda é um cursor, um arquivo lido esperando resposta e a
senha do hotspot enquanto ele está de pé — nenhuma LEITURA: o `Snapshot`, os pontos de acesso, o
`/proc` e os dois túneis chegam num `Facts` na hora de pedir as linhas. **Um parâmetro e não
quatro**, porque uma assinatura que cresce com as seções é uma assinatura editada nas duas portas, no
clique e em cada teste toda vez que uma seção nasce.

**O que se digita se digita no RODAPÉ.** Senha de Wi-Fi e caminho de arquivo passam pelo minibuffer,
que a janela não tem — então lá a linha DIZ onde responder, que é o arranjo que as perguntas de
pareamento do Bluetooth já têm. Uma porta que não pode perguntar não finge que pode.

**A pergunta "lembrar?" continua sendo do rodapé**, desenhada acima da lista e não pela `View`: a
senha que ela guarda vive por uma tecla, e uma `View` que duas portas desenham não pode carregá-la.

**As cinco marcas somem em repouso.** `airplane`, `net-off`, `vpn`, `tailscale` e `tor` — as três
primeiras já existiam; as duas novas são RUNNING e não INSTALADO: um tailnet em que ninguém entrou e
um Tor parado não desenham nada. Clicar numa abre a vista NA LINHA dela (`View::fresh_at`), porque
uma marca é uma porta para a coisa de que ela fala, não para o prédio.

### O que a revisão do entregável encontrou, e o que virou regra

Um agente novo leu o entregável inteiro contra os requisitos e os invariantes, e o que ele achou vale
mais escrito do que corrigido em silêncio — cada item aqui é um defeito que passou por toda a bateria
verde.

**Dois que teriam ido para o commit.** `hotspot_password` lia `/dev/urandom` com `fs::read`, que
NUNCA retorna — um dispositivo de caractere não tem fim — então a função crescia um `Vec` até o
kernel matar o processo, no laço de eventos da própria porta; o `oomd` levava o cgroup inteiro junto.
E um comentário de `group_by_ssid` guardava três SSIDs REAIS lidos da tela do dono, dois deles de
vizinhos: num repositório público isso é endereço de rua (inv. 13). A lição do segundo é que o
invariante 13 vale para COMENTÁRIO, não só para fixture.

**O prazo de uma medição começa quando a medição começa.** A varredura ICMP marcava o prazo antes dos
envios; mandar para 254 endereços espera o ARP de cada vizinho desconhecido — três segundos medidos —
e a janela de escuta acabava antes do primeiro pacote sair: zero máquinas numa rede com cinco. E o
tempo medido desde o envio incluía essa espera, então uma máquina na sala ao lado aparecia com
3 000 ms. Por isso a varredura responde QUEM e o `ping` responde QUANTO.

**Um protocolo de linhas precisa de ESTADO.** O socket de controle do Tor termina a resposta com
`NNN ` — três dígitos e um espaço — e uma linha DENTRO de um bloco `NNN+` tem a mesma forma quando o
id do circuito passa de 99. Sem guardar "estou dentro de um bloco", a leitura parava no meio e as
sobras eram consumidas como resposta das perguntas seguintes: versão e tráfego chegando como linhas
de circuito, a cada dez segundos, só depois de uma sessão longa.

**Enquadramento é BYTE, texto é o fim.** O corpo HTTP chunked era lido como `String` antes de tirar o
enquadramento; um caractere partido entre dois chunks virava um caractere de substituição de tamanho
diferente e o `&str[..n]` seguinte dava pânico — matando a thread que também executa as ordens, em
silêncio, pelo resto da sessão. Uma máquina no tailnet com nome acentuado bastava.

**`null` não é ausência para o serde.** `#[serde(default)]` cobre campo FALTANDO, não campo presente
e nulo — e um `tailscaled` que nunca entrou num tailnet responde `"Peer": null`. A leitura inteira
falhava e um daemon rodando era relatado como ausente. Todo campo passa por `or_default`.

**Um interruptor lê o que ele MUDA.** O do tailnet lia `running` (está de pé) em vez de `wanted`
(pediram que ficasse de pé), então numa máquina que quer rodar e não consegue ele oferecia LIGAR o
que já estava pedido, sem jeito de desligar. O do Tor lia o circuito em vez do daemon: a seção só
existe porque o socket respondeu, isto é, porque a unidade está de pé.

**Um check que não checa fecha marco vazio.** O `tor-polkit-is-narrow` original passava por quatro
alargamentos da regra — `YES` virando `AUTH_ADMIN`, uma segunda unidade, `try-restart`, um segundo
usuário — porque procurava presença de palavras e nunca o RESULTADO. Agora lê a regra MERGED do
sistema, exige que a condição não teste mais nada além dos quatro comparadores, e foi provado contra
as cinco mutações. O `english-names` tinha o mesmo tipo de buraco: não conhecia o pretérito (`-ou`,
`-ei`), e `escapou` estava num `assert!` havia meses — corrigido, e ele achou mais dez mensagens em
português espalhadas pela árvore.

### Os dois túneis que não são do NetworkManager

`ronoctua-tunnel` é a fronteira, e são dois protocolos numa crate porque na tela são uma ideia só:
"estou saindo por outra coisa, e ela está funcionando?".

**Tailscale fala HTTP num socket Unix** (`--operator=<user>` no `tailscaled` é o que dispensa root —
não é grupo nem polkit, é o mecanismo do próprio daemon). Toda preferência vai MASCARADA
(`{"WantRunning":false,"WantRunningSet":true}`): sem a máscara o daemon toma a struct inteira como
dada e escreve zero em tudo que não foi mandado, o que desloga a máquina ao mexer num interruptor.
Todo campo é lido com `#[serde(default)]` — a local API não é estável, e uma release que renomeia um
campo tem de custar um FATO na tela, não a seção inteira.

**Tor fala um protocolo de linhas no socket de controle**, autenticado por cookie. `CookieAuthFile`
NÃO é opcional: sem ele o Tor escreve o cookie dentro do `DataDirectory`, que é `/var/lib/tor` a
0700 — então a flag de grupo abre um arquivo dentro de um diretório em que ninguém do grupo entra, e
a vista encontra "permission denied" numa máquina que parece configurada. Foi o
`tests/network-vm.nix` que falhou exatamente nessa linha.

**Nenhum dos dois sinaliza** — não há barramento para assinar —, então uma thread pergunta a cada dez
segundos e manda a cena inteira. Numa máquina sem os dois, a pergunta é um `connect` que falha.

**A única regra polkit nova do plano é o `tor.service`**, com os verbos `start` e `stop`, para um
usuário. `tor-polkit-is-narrow` lê a regra instalada e a constante do crate e compara nos DOIS
sentidos; e o Tor não sobe no boot, porque quem não pediu Tor não está usando Tor.

### `/proc`, ICMP e o que se pode saber sem privilégio

**`ronoctua-memory::net` é a fronteira do `/proc/net`** — a mesma regra que faz o `ronoctua-niri` o
único a falar com o compositor. Duas armadilhas viraram teste: o endereço em `/proc/net/tcp` é
LITTLE-ENDIAN (lido ao contrário, `0100007F` vira `1.0.0.127`, um endereço plausível que não é o que
está aberto), e UDP não tem `LISTEN` — o kernel escreve `07` para um socket sem par, e ler `0A` ali
não listaria o resolvedor na 53. **Porta sem dono diz que não é nossa**: `/proc/<pid>/fd` de outro
usuário não se lê, e uma segunda coluna vazia leria como "não há ninguém ali".

**`ronoctua-lan` varre a rede SEM PRIVILÉGIO NENHUM**: o Linux tem um terceiro tipo de socket ICMP
(`SOCK_DGRAM` + `IPPROTO_ICMP`) aberto a qualquer processo dentro de `net.ipv4.ping_group_range`, que
no NixOS é todo mundo — é o que o próprio `ping` usa em vez de um binário setuid. O kernel preenche o
identificador e o checksum, então o pacote sai com zeros nos dois e o que casa resposta com pergunta
é a SEQUÊNCIA.

**O prazo começa depois dos envios, e o número de milissegundos não vem da varredura.** As duas são a
mesma medição: mandar para 254 endereços não é instantâneo, porque cada um cujo endereço de hardware
o kernel não conhece ESPERA o ARP — três segundos na rede do dono. Com o prazo começando antes, a
janela de escuta acabava antes do primeiro pacote sair e a varredura achava zero máquinas numa rede
com cinco. E com o tempo medido desde o envio, uma máquina na sala ao lado aparecia com 3 000 ms.
Então a varredura responde QUEM, e `ping` — um endereço de cada vez, já resolvido — responde QUANTO.

### Bluetooth — `LEADER s b`, e o glifo é o que a coisa É

Um crate fala com o BlueZ (`ronoctua-bluetooth`, molde do `ronoctua-disks` e do `ronoctua-power`), e
o que atravessa o canal é a CENA INTEIRA (`Scene`), nunca um delta: durante uma varredura o daemon
emite centenas de mudanças por segundo, e delta é coisa que tem de ser reproduzida na ordem em que
foi produzida. **Tudo é lido por um `GetManagedObjects`** — adaptadores, dispositivos, baterias e
transportes numa resposta só — e NADA por `Properties.Get`: o BlueZ declara as propriedades
opcionais com um predicado `exists`, e `RSSI`, `TxPower`, `Modalias`, `Icon`, `Class` e `Appearance`
simplesmente não estão no mapa quando não há o que dizer; perguntar por uma delas responde
`No such property`, que é erro, não valor vazio.

**A regra de assinatura é por INTERFACE, não por caminho.** `InterfacesAdded` e `InterfacesRemoved`
saem de `/`, não de `/org/bluez` — uma regra com `path_namespace=/org/bluez` (o idioma que o
`ronoctua-power` usa para o UPower) veria toda mudança de propriedade e PERDERIA toda chegada e
partida: um dispositivo só apareceria na tela quando mudasse alguma outra coisa.

**São QUATRO threads, e duas delas existem por causa da varredura.** Uma segura o fluxo de sinais do
`org.bluez` e não faz mais nada; ela levanta uma bandeira. Outra segura o fluxo do BARRAMENTO
(`NameOwnerChanged` com `arg0=org.bluez`), porque um daemon que MORRE não remove os objetos um a um —
some o nome inteiro, e sem essa regra a modeline continuaria desenhando um dispositivo que não existe
mais. A terceira lê a cena quando a bandeira sobe, no MÁXIMO quatro vezes por segundo — ler dentro da
primeira transformaria uma varredura em centenas de idas ao barramento. A quarta executa as ordens,
porque `Device1.Connect` bloqueia por segundos, e é ela que SERVE o agente de pareamento: o objeto
que responde à pergunta tem de viver mais que a pergunta.

**E a RESPOSTA não passa pela fila de ordens.** `Pair` está bloqueando aquela thread enquanto a
pergunta existe; uma resposta enfileirada atrás dela esperaria a ordem que espera a resposta. O
`Watch` devolve, ao lado da mão, o punho das perguntas (`agent::Questions`), e responder é mandar
num canal. Cada pergunta leva um SERIAL: dois tempos limite que se cruzam não podem limpar a
pergunta um do outro.

**Ninguém pede permissão a ninguém, e desta vez nem grupo há.** O BlueZ não tem ação polkit nenhuma:
a política de barramento dele deixa `context="default"` falar com `org.bluez`, e essa é a porta
inteira. O módulo não escreve regra (não há o que alargar) e o check `bluetooth-bus-policy-is-open`
lê o arquivo INSTALADO — no dia em que o upstream mudar de ideia, um check diz, em vez de a tela
ficar muda. **O agente não reivindica nome no barramento**: `RegisterAgent` recebe um CAMINHO e o
bluetoothd guarda o nome único de quem chamou; pedir um nome bem-conhecido falharia contra o
`<deny own="*"/>` do dbus, com um erro que se lê exatamente como "daemon ausente".

**O glifo da modeline é o que a coisa É** — fone como fone, teclado como teclado, o pedido do dono —
e isso sai da CLASSE (BR/EDR) ou da APARÊNCIA (LE), números que o próprio dispositivo anuncia, nunca
do nome. O `Icon` do BlueZ é derivado desses dois e joga informação fora (um relógio é `None` nas
duas tabelas dele), então entra como terceira palavra e não como resposta. Até quatro marcas
(`bt-1`..`bt-4`), uma por dispositivo CONECTADO, alarmado primeiro; `NN%` só a ≤30 e cor de perigo a
≤15, os mesmos dois números da bateria da máquina e pela mesma regra literal. Clicar numa marca abre
AQUELE dispositivo inteiro, com o mesmo comando que o `RET` na linha dele roda.

#### O perfil de áudio não está no barramento que este crate fala

O `MediaTransport1` diz o que ESTÁ tocando: um UUID, um estado, um codec, um volume. Ele não diz o
que o aparelho PODE fazer — e é essa a pergunta atrás de "todas as opções e configurações": um
headset faz música em estéreo (A2DP) **ou** uma chamada com o microfone (HFP), nunca as duas ao mesmo
tempo. Quem é dono da placa é o PipeWire, então a resposta vem de lá.

**A medição decidiu qual programa**, e ela ficou meses por fazer no `PENDING`. As opções eram o
`pactl`, que arrasta o cliente do PulseAudio para dentro do fecho, e a dupla `wpctl`/`pw-dump`. Um
`nix path-info -r` sobre o `toplevel` do próprio sistema responde: os dois já estão em `sw/bin`,
postos lá pelo `services.pipewire`; o `pactl` não está. Custo zero, e é isso que tirou o item do
"tem preço" para o "está feito".

**A armadilha é a identidade.** O PipeWire nunca ouviu falar de `/org/bluez/hci0/dev_…`; o que ele
conhece é `bluez_card.AA_BB_CC_DD_EE_FF`, montado a partir do ENDEREÇO com `:` virando `_`. Procurar
a placa pelo caminho do objeto não acha nada, em silêncio — que é como esse tipo de erro sempre se
apresenta.

`pw-dump` LÊ e `wpctl` ESCREVE, e essa também é a linha de segurança: nada muda até uma tecla ser
apertada. A leitura acontece quando o nível do dispositivo ABRE e em mais lugar nenhum: o `pw-dump`
imprime o grafo inteiro, e perguntar por quadro seria um `fork` dentro do laço do rodapé.

**Sem placa, sem linha** — e sem a linha, a dica também não a ensina. Um teclado não tem placa de
áudio, e a primeira foto depois de escrito mostrava `→ o próximo perfil de áudio` na página de um
teclado: a coluna `Where::OnACard` existe porque `OnADevice` responde QUAL TELA e esta pergunta é
sobre o que a coisa naquela tela sabe fazer.

**O desenho do teclado é a única exceção medida do `chrome-icons-match-the-text`.** Um teclado é um
desenho LARGO e baixo: a tinta dele já ocupa a célula inteira, então nenhum tamanho o leva à altura
do dígito sem encostar no caractere ao lado, e não existe teclado quadrado na fonte (os nove que
passam na faixa são caps-lock, um `x`, uma engrenagem ou a tecla ⌘ — a coisa errada dita com
clareza). Então ele é medido pela outra régua, com o motivo escrito no check: preenche a célula e
não é uma lasca. Os outros cinco desenhos que a faixa recusou trocaram de glifo, que é o que a
mensagem do check manda fazer.

**O perfil de áudio não é trocável aqui, e isso está escrito** (`docs/PENDING.md`): o BlueZ diz o que
está tocando (`MediaTransport1`), não o que se pode escolher — quem sabe isso é o PipeWire, e é uma
fronteira que este entregável não abriu.

**A vista tem TRÊS níveis e o `Esc` volta um**: a lista (o rádio, os pareados, os por perto), o
DETALHE de um dispositivo (cada fato numa linha, e uma linha por serviço que ele oferece) e o rádio.
As teclas são LETRAS NUAS — nada se digita ali —, lidas da própria `OF_THE_VIEW` pelas duas portas,
de modo que uma letra acrescentada à tabela é uma tecla que funciona e uma tecla fora dela é uma
tecla que ninguém ensina. **Uma pergunta de pareamento toma a tela** (nada pareia até ela ser respondida) e o `Esc` ali
responde NÃO, em vez de deixar o outro lado esperando os sessenta segundos do bluetoothd. Quando a
vista NÃO PODE abrir — o gerenciador de arquivos está no rodapé, segurando uma pasta que alguém
navegou —, a pergunta vira NOTIFICAÇÃO, com os dois botões que rodam os mesmos comandos do `s b y` e
`s b n`; e ela é RETIRADA quando a pergunta acaba, porque um cartão crítico com dois botões que não
respondem mais nada é pior que cartão nenhum. Pergunta que pede DÍGITOS (um PIN) não aceita um "sim"
seco de lugar nenhum: abre a vista e pede no minibuffer, senão o que sairia daqui é um PIN vazio.

---

## 8. Desempenho, grafo e riscos

### Grafo de crates

`tokens`, `i18n`, `protocol` e `cmd` não dependem de nada e são dependência de todo o resto.
`runtime` **não conhece** `render` — produz display list, não rasteriza, e é o que permite testar
buffers headless. `niri` é folha: só o `shell` a toca. `term` isola todo o `unsafe` e todo o FFI.

Duas adições ao grafo original, cada uma por um problema real:

- **`ronoctua-glyphgen`** (hoje `nix/glyphgen.nix`). O consumidor da base compila para wasip2 e
  **não tem filesystem**: não pode ler o TTF em tempo de build. Sem gerador separado, ou ele deixa
  de ser wasip2, ou a tabela de ícones volta a ser hard-coded.
- **`ronoctua-cmd`.** Toda funcionalidade tem de ser um comando nomeado. `keymap` resolve a árvore de
  teclas; o **registro de comandos** é outra coisa, e sem crate próprio os ids nascem espalhados
  dentro do `shell`.

---

### Desempenho medido

Medido em ThinkPad Z16 Gen 2, Radeon 760M (RDNA3, RADV), 1920x1200@60 Hz — um frame são 16,7 ms.
`nix run .#bench` remede tudo isto e **falha se algum alvo estourar**; se estes números divergirem
do comando, o comando está certo e a tabela está velha.

| medida | valor | alvo |
|---|---:|---:|
| primeiro frame | 29,0 ms (p99 52,9) | 200 ms |
| which-key | 4,4 ms (p99 6,2) | 16,7 ms |
| resolver do layout | 0,0003 ms (p99) | — |
| primeira moldagem de texto | 0,1 ms | — |
| instanciação de componente | 0,011 ms (p99) | 5 ms |

Os marcos foram cronometrados até `Queue::present`, que no Wayland faz `attach` + `commit`: é o
instante em que o frame sai das **nossas** mãos, não em que vira luz. Medir a fronteira do nosso
código é a escolha honesta; um número "até o pixel" mediria o compositor junto.

Duas correções que esses números carregam, ambas de ordem de grandeza:

- **A família genérica custava 26×.** Pedir `Family::Monospace` ao cosmic-text resolve para
  `Noto Sans Mono`, que não está no nosso banco; a busca falhava para dentro do fallback e a primeira
  moldagem custava 4 447 ms. E não era só desempenho: **qual face desenhava o rodapé dependia do
  fallback**, o que contraria "o renderer não usa fontconfig" pelo mesmo motivo.
- **Um driver de hardware ausente custava 650 ms.** O loader Vulkan carrega com sucesso um ICD cuja
  biblioteca existe, procura dispositivo, não acha, e demora. Filtrar por existência não distingue
  nada — o critério é o vendor PCI dos dispositivos DRM presentes.

---

### Riscos conhecidos

| # | risco | mitigação |
|---|---|---|
| R1 | **Build Zig/Nix do `libghostty-vt`.** API pre-1.0; a FOD quebra a cada bump. | O trait torna isso não-bloqueante. Commit pinado. Sem ele, o diferencial degrada para golden-file. |
| R2 | **Deriva do IPC do Niri** contra "sem fork". | `ronoctua-niri` é folha com superfície estreita; o contrato roda contra a tag pinada **e** contra `niri-next`, dando aviso antes de a release virar obrigatória. Se a capability sumir: **parar e reportar**. |
| R3 | **Instanciação de 5 ms** contra os dados do `glyphdb`. | `memory_init_cow` + AOT `.cwasm`, com gate de CI que mede e falha acima de 5 ms. |
| R4 | **Xwayland quebrando PID → cgroup.** | Um satellite por scope. |
| R5 | **Closure de fonte de ~10 GB.** | Cinco derivações; o ISO leva 62 MB; `noto-fonts-extra` isolado e removível por opção de host. |
