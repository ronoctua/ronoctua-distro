# Arquitetura — o porquê

Este documento existe para impedir que uma decisão já tomada seja refeita. Cada seção diz o que foi
decidido e **por quê**, e o que foi descartado no caminho.

Junto de `AGENTS.md` (o contrato do agente), `INSTALL.md` (instalar no disco) e `DEVELOPMENT.md`
(rodar o ambiente de trabalho), ele deve bastar para alguém continuar o projeto sem nenhum histórico
de conversa. O que ainda **falta** está em `PENDING.md`, não aqui.

Não há relato de depuração aqui, e não há changelog. Quando um bug ensinou algo que se repete, o
ensinamento virou uma armadilha no `AGENTS.md` ou uma frase sobre o desenho atual; a história de como
o sistema chegou até aqui não sobreviveu de propósito.

A interface toma emprestadas algumas ideias e alguns nomes de editores de teclado — modeline,
minibuffer, a lista que aparece quando a tecla líder é apertada. É empréstimo de vocabulário, e não
um compromisso: nada aqui precisa se parecer com nenhum outro programa.

---

## Sumário das decisões

| Área | Decisão | Alternativa descartada |
|---|---|---|
| Sistema | NixOS mínimo, tudo por flakes | qualquer coisa imperativa |
| Compositor | Scroll upstream, sem fork nem patch, atrás de um contrato | fork com integração direta |
| Chrome | rodapé de duas linhas, só | barra, dock, popup de canto |
| Sistema de arquivos | btrfs | ZFS (out-of-tree), bcachefs (fora do mainline na 6.18) |
| Boot | systemd-boot, UKI não assinada — ver *Secure Boot e TPM2* | GRUB; lanzaboote |
| Renderer | wgpu + cosmic-text + glyphon | vello (não há carga vetorial) |
| Buffers nativos | WASM Component Model, display list retida | pixels, ou plugins nativos |
| i18n | Fluent | gettext |
| Fontes | assets de um Release, quatro inputs fixados por hash | dentro do repositório |
| Janelas | uma `Door<C>` genérica com padrões sobrescrevíveis | uma macro por janela; um trait de janela com toda a encanação |

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

#### E por um ano a segunda cadeia não existiu

A frase acima descrevia uma intenção. O `chainJson` era gerado, instalado em
`/etc/ronoctua/fallback-chain.json`, e **lido por nada** — o comentário no
`nix/fontconfig.nix` dizia *"the ronoctua-render crate reads this at build time"* e nenhum `.rs`
mencionava o arquivo. O banco do renderer eram QUATRO faces embutidas por `include_bytes!`, e a
tabela interna do cosmic-text pede famílias por NOME (`Script::Arabic => ["Noto Sans Arabic"]`):
nenhum desses nomes existia no banco, então o iterador varria as quatro e devolvia `glyph_id == 0`.

**Todo japonês, árabe, hebraico, devanágari, tailandês e coreano da tela era tofu**, e nem no log
aparecia — o aviso do cosmic-text cai em `log::debug`, e nada aqui liga o `log` ao `tracing`.

O motor nunca foi o problema. O `cosmic-text` 0.19 traz **harfrust** (o porte do HarfBuzz),
`unicode-bidi` e `unicode-script`, e o `Text::prepare` já pedia `Shaping::Advanced` — ligadura
árabe, reordenação devanágari e bidi RTL rodariam desde sempre. Faltavam FACES.

O conserto tem três peças, e a primeira é a que faz a frase de cima virar verdade:

- **`nix/fontconfig.nix` ganhou a metade que faltava** — `scripts` (37 escritas → família),
  `han` (a unificação Han por locale), `hanScripts` (quem segue o locale e quem não segue) e
  `common`. Dali saem TRÊS consumidores: o `.conf` do fontconfig, o `nix/coverage-fonts.nix`
  (que resolve família → arquivo e **recusa a build** se uma não tiver face) e o
  `nix/chain.nix`, que gera o `impl cosmic_text::Fallback` do renderer. Uma declaração, e agora
  o renderer realmente a lê.
- **A cadeia continua DECLARADA** (§6.4): `new_with_locale_and_db_and_fallback` recebe a nossa,
  e é por isso que ela pode pedir a CJK **monoespaçada** — a do upstream pede a proporcional, e
  numa grade de células isso não tem como ser colocado.
- **As faces viajam no EMBRULHO** (`RONOCTUA_COVERAGE_FONTS`), nunca em `generated`: são 44
  arquivos e 54 MB, e um caminho de store no ambiente de quem compila reconstrói os 33 crates —
  o quarto incidente desta seção, chegando pelo mesmo lado.

Ausência não é erro: o instalador e o resgate seguem com as quatro embutidas e `Text::coverage`
diz quantas subiram, para uma tela poder dizer que a interface está reduzida em vez de mostrar
caixinhas.

**O renderizador nasce na língua da interface.** O locale é o que escolhe, entre as quatro faces
Han, se 直 sai japonês ou chinês. Ele começava num padrão e só uma TROCA de língua o movia
(`Text::set_locale`): toda porta, o rodapé e a linha de vida desenhavam o Han da primeira língua do
jeito do padrão até alguém escolher uma língua de novo — só a foto (`photograph`) acertava, porque
chamava `set_locale` ao nascer. Agora `Text::new` recebe o locale como recebe a escala: quem cria
um renderizador diz a língua dele, e esquecer é erro de compilação, não uma tela que difere da foto.

**Duas medições que viraram desenho.** A primeira: pôr `Noto Sans Symbols 2` na cauda comum fez
ele responder por `U+1F550` ANTES da face de emoji — o relógio voltou a sair monocromático, que é
exatamente o defeito que o `strip-emoji.py` existe para impedir, chegando por outra porta. A face
de emoji passou a ser a PRIMEIRA da cauda. A segunda: a sonda do armênio era `U+0531`, e a Iosevka
desenha 91 dos 96 codepoints armênios — uma sonda que a face primária responde não prova nada,
porque a primária é pedida por NOME e ganha de toda cadeia. O gerador agora **recusa** uma sonda
assim, e a do armênio virou `U+FB13`, uma ligadura que a Iosevka não tem.

#### A primeira TLS deste workspace, e por que ela não contradiz o `aria2`

O `aria2.rs` recusa um cliente HTTP por escrito: *"traria tokio, hyper e uma pilha TLS para um
`Cargo.lock` que todo crate do workspace compartilha, para poupar umas sessenta linhas que nunca
vão mudar."* Isso continua **dois terços certo** — não há tokio aqui e não há hyper. O que mudou é
o terço restante: o aria2 fala com `127.0.0.1`, e o relance fala com a máquina de outra pessoa,
onde não existe versão aceitável de "sem TLS".

Então o corte é onde este projeto sempre o põe — a dependência entra quando o problema dela
aparece, e entra do tamanho do problema:

- **rustls sobre `std::net::TcpStream`**, bloqueante, numa thread. Sem runtime assíncrono.
- **Provedor `ring` e não `aws-lc-rs`**, que é o padrão da 0.23: o padrão quer cmake e uma
  toolchain Go no closure de build, para buscar dois números.
- **Raízes do SISTEMA** (`/etc/ssl/certs/ca-certificates.crt`, que o `security.pki` monta), nunca
  `webpki-roots`. Raiz embutida envelhece DENTRO do binário, e o dia em que uma expira o sintoma é
  toda busca falhando numa máquina cujo navegador está bem. Sem o arquivo, o crate diz uma vez e
  não busca nada — feature que não funciona, e não certificado ignorado.
- **Sem proxy, keep-alive, cookie ou autenticação.** Dois hosts respondiam um `GET` com JSON e
  `Connection: close`. Redirecionamento e gzip vieram depois, quando a central de programas e as
  buscas do player os pediram, e moram num módulo só (`ronoctua_http::web`): no máximo cinco
  saltos, só `https` (um redirecionamento para `http` é recusado; o primeiro endereço `http` de
  `commons.wikimedia.org` e `coverartarchive.org` é pedido direto em `https`), e o gzip inflado
  com teto na SAÍDA. Qualquer outra codificação num 2xx é recusada; numa recusa (4xx, 5xx) o
  corpo cai e o status continua sendo a resposta.
- **O decodificador de `chunked` não é novo**: é o do `ronoctua-tunnel`, e é por isso que a
  dependência aponta para lá. Ela lê estranho pelo nome e certo pelo conteúdo — a alternativa era
  um segundo decodificador, e o primeiro existe porque uma fronteira de chunk no meio de um
  caractere multi-byte estourou uma thread e matou toda tecla de túnel de uma sessão.

**Medido ao vivo, contra os hosts reais:** Londres 15,3 °C, 1 GBP = 6,9233 BRL. E as zonas de
verdade: Londres 23:05 contra Brasília 19:05.

**Um teste que passava pela razão errada, e foi reescrito.** O primeiro `a_place_keeps_what_it_had`
chamava `once()` e afirmava que os valores antigos sobreviviam, na premissa de que a máquina de
teste não tem rede — e esta tem: a busca funcionou, Londres voltou a 15,3 °C, e a asserção falhou
pelo único motivo que era boa notícia. Num sandbox ela teria passado sem provar nada, e continuaria
passando depois de alguém trocar o `Err` por limpar o campo. A regra da mesclagem virou função com
nome (`fetch::keep`) e o teste passou a testar ELA — sem rede, em 0,00 s.

#### A célula deixou de ser uma contagem

`ronoctua_protocol::cells` contava grafemas, e estava certo enquanto a única face era a Iosevka:
monoespaçada em `0,600 em`, um grafema uma célula. Com a cadeia alcançando o CJK isso virou
mentira — `日本語` são três grafemas e **seis** células, e cada coluna depois dele era colocada na
metade da distância que precisava.

A régua passa a somar `East_Asian_Width` (W e F) mais `Emoji_Presentation`, de uma tabela
**gerada dos arquivos do próprio Unicode** (`nix/width.nix`, 123 faixas). `A` (ambíguo) fica
ESTREITO de propósito: ele cobre cirílico, grego e o box-drawing de que esta interface é feita, e
alargá-los quebraria toda tela existente para ajudar um caso que este sistema não tem.

`cells_of` continua devolvendo grafemas — é por onde se CORTA, e cortar por outra coisa inventa
desenho. As duas respostas passaram a ser diferentes, então `elide_end`, `elide_middle`, `window`
e o `wrap` ganharam `prefix_of`/`suffix_of`: **orçam em CÉLULAS e cortam em GRAFEMAS**. Antes eles
faziam `take(max)` de grafemas contra um orçamento de células, o que transborda — e transborda na
única direção que custa alguma coisa, desenhando por cima da coluna vizinha.
E o `ronoctua_render::layout::width`, que era uma SEGUNDA cópia do mesmo `graphemes().count()`,
passou a ser a função do protocolo: duas réguas que concordavam por acidente.

**Medido, com a face monoespaçada da Noto CJK**: o ideograma anda 18,00 px numa célula de
10,80 px, ou **1,667 célula**, contra as 2 que a régua reserva; o hangul, 1,533. A tinta é mais
ESTREITA que o reservado, então nada se sobrepõe — sobra vão. Fechá-lo seria reescalar a face para
`upem 833` (o em da CJK é 1,0 e a nossa célula 0,6; 1,0/0,833 = 1,2 em = 2 células), e isso é
cirurgia de tabela em cinco faces com `CFF2` e `FontMatrix` — cosmético contra um risco real, e
por isso ficou em `docs/PENDING.md` com o número. O check refuta só a direção perigosa: acima de
2,02 células ele para a build.

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

A correção é manter build fora da árvore: onde o cargo escreve é o `nix/cargo-dirs.nix` (veja "Os
builds do cargo" na seção do disco). A guarda é o check `lean-tree`, que mede `self` e falha acima
do teto — e tem fixture negativo. Ele só vê o que o git rastreia: um `target/` ignorado nunca chega a
ele, e é a sessão, que manda os intermediários de todo projeto para o cache, que cobre o `cargo`
rodado fora do devshell. O custo é invisível até o disco encher.

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

#### E a quinta não foi por entrada nenhuma: era o PERFIL, e o laço estava morto

As quatro acima são sobre **o que invalida** um build. A quinta é sobre **quanto custa** um, e sobre
uma linha que fazia o laço não existir.

O `[profile.release]` da raiz é afinado para o binário que roda por meses: `lto = "thin"` e
`codegen-units = 1`. Uma unidade de codegen é o que dá ao LLVM o crate inteiro para otimizar de uma
vez — e `crates/ronoctua-shell` tem **178 mil linhas num crate só**, então a geração de código dele
roda numa THREAD. Medido nesta máquina de 12 núcleos:

| | `release` | `[profile.loop]` |
|---|---:|---:|
| uma linha editada no shell, incremental | **148 s** | **19 s** |
| build FRIO, com os 483 crates de terceiros | — | **139 s** |

O build frio no perfil do laço é mais rápido que um incremental no de produção. Nada disso serve para
olhar a interface num compositor aninhado; serve para o ISO — e o mesmo perfil era pago pelo `cargo
test` e pelo `clippy` do hook, que se roda dezenas de vezes por dia contra uma instalação que se faz
raramente.

**Nomear o lado RÁPIDO é a decisão.** Um `release` barato com um `dist` para a entrega seria mais
econômico e faria o esquecimento cair para o lado errado: um binário sem LTO no ISO não tem sintoma
nenhum além de ser mais lento, na máquina de outra pessoa, meses depois. Assim, quem não pedir o
perfil do laço recebe o de produção. Cobra o `the-loop-profile-is-never-installed`, que lê o FECHO
dos sete programas que uma máquina instala. O preço é uma camada de dependências por perfil, paga
uma vez por `Cargo.lock`.

**E o laço estava morto, o que é a metade que ninguém tinha medido.** A linha do hot reload em
`nix/dev.nix` era `--bin ronoctua-rescue` sem `-p ronoctua-rescue`. `--bin` nomeia um alvo DENTRO dos
pacotes que o `-p` escolheu, então o cargo não construía quatro binários menos um: recusava o comando
inteiro com *"no bin target named `ronoctua-rescue` in `ronoctua-shell`"*. O laço caía no `else`,
dizia "a compilação falhou" e seguia com o rodapé antigo — **toda edição de Rust, de toda sessão**. O
que isso parece, de fora, é um laço que precisa ser reiniciado a cada mudança; e é por isso que o
custo do build do Nix era o que se sentia.

O `dev-rebuilds-what-sits-beside` existia e passava: ele conferia que o NOME aparecia na linha. O
nome aparecia, escrito certo, num comando que não roda. Agora ele deriva dos manifestos qual pacote
declara cada binário e recusa um `--bin` fora dos `-p` — que é a forma de errar que aconteceu.

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
usuário, os binds do compositor, o laço aninhado, seis VMs —, e variável que um deles esquece não dá erro:
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
`nix run .#dev` compila com `cargo`, e as cinco variáveis não estavam exportadas ali — `m i n` abria
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
sete horas de processo, uma vez. A armadilha ficou escrita no `AGENTS.md`, e mesmo assim o hook de
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
que o `AGENTS.md` registrava como "~2 h calado", mede **98 s de roteiro**: as ~2 h eram construir a
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

**Uma VM não é um compilador** (2026-09-27). A aritmética dividia a memória por um pico de `rustc`
(1792 MiB) e dava a todo derivado o mesmo `--max-jobs`. Os testes de VM não cabem nessa conta: cada
um segura o que os nós declaram (`virtualisation.memorySize`, de 1 a 4 GiB por teste) enquanto o
roteiro roda, e quatro lado a lado são de 8 a 16 GiB numa máquina de 14. Uma rodada passava quando
os pequenos coincidiam e outra morria no meio das VMs. Por isso as VMs viraram uma fase própria, em
quatro passos: as três pesadas; **o que as VMs bootam** (o `driver` de cada teste, que é o sistema
dele), dimensionado como compilação; **as VMs**, empacotadas pela memória declarada mais 256 MiB de
QEMU por nó e 128 MiB de driver por teste, e pelos núcleos que declaram; e o resto. Cada VM roda como
`nix build <o .drv dela>` com tudo já no store, então nada compila ao lado de uma VM.

**O declarado é o teto, não o típico.** Medido pelo `VmRSS` do QEMU no pico: `footer-vm` declara
2048 MiB e tocou 582; `camera-vm`, 3072 e 965; `oom-vm`, dois nós e 3072, tocou 910;
`player-remote-vm`, dois nós e 4096, tocou 1064. Mesmo assim se planeja pelo declarado, porque é o
que o convidado tem direito de alcançar, e um número medido envelhece a cada teste que muda. Baixar
o `memorySize` dos testes para ganhar paralelismo não compensa: mudaria o que `oom-vm` e
`memory-vm` medem, e a fase já cabe três de 2 GiB por vez numa máquina com 10 GiB livres.

**E o `rustc` do shell não se divide.** As pesadas recebiam `--cores` = memória ÷ 1792 MiB, como se
todo compilador fosse igual. Sob `release` o `ronoctua-shell` é uma unidade de código só, com LTO
fino: um processo que mediu 5120 MiB sozinho, enquanto o workspace inteiro, com `--cores 4`, fez
pico de 7658 MiB. O shell se paga uma vez, e cada núcleo a mais paga o que se mediu ao lado dele
(846 MiB); o gate usa 5632 e 1024 MiB, com margem. E eram quatro pesadas, não três: o workspace
`loop`, que três checks alcançam pelo `.#dev`, compilava na última fase, quatro de cada vez.

**E avaliar também não é de graça.** Um teste de VM é um sistema NixOS, e avaliá-lo segura ~260 MiB
até o avaliador sair. O `nix build` da bateria avaliava as 31 VMs num processo só — 8,2 GiB de RSS,
medido — e esse processo vivia o build inteiro, ao lado de cada compilador e de cada VM, sem entrar
em conta nenhuma. Agora nada se avalia enquanto algo constrói: a fase 0 pede o `.drv` de cada alvo
em lotes cujos avaliadores cabem no orçamento, um depois do outro, e as fases seguintes constroem
esses caminhos, o que não precisa de avaliador. A exceção é o último passo do gate, o
`nix flake check`, que avalia tudo de uma vez por definição — e a essa altura não tem mais nada para
construir. A medição achou também um acoplamento: o check que lê o script do gate avaliava o app, e
um app que avaliasse as VMs no próprio texto custaria 8 GiB a esse check. Por isso os fatos das VMs
são perguntados em tempo de execução, e não escritos no script.

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

#### A quarta face é a única que este repositório DESENHA

O dono pediu o logotipo da Tailscale na modeline, e ele não existe em nenhuma das cinco fontes.
Dois invariantes se encontram aí e sobra uma porta só: o 10 diz que ícone de UI é **glifo de
fonte** e que componente nenhum do design system consulta pixmap; o 9 diz que codepoint é **token**,
resolvido no build contra um cmap de verdade. Então a marca é desenhada NUMA FACE NOSSA.

O molde já existia — o `nix/lucidegen.nix` constrói uma face, remapeia codepoint e é carregada pelo
renderer. A diferença é que esta não tem fonte upstream atrás dela:

* **`assets/icons/marks.nix` é a verdade**, e declara CÍRCULOS numa grade, não um caminho. Um
  caminho é uma coisa que ninguém lê nem ajusta; a marca da Tailscale são três linhas que se
  **veem** (`"ooo" "***" "o*o"`, `*` cheio e `o` anel). O dia em que uma marca precisar de uma curva
  que não é círculo, vai precisar de um caminho — e de uma razão escrita ao lado.
* **`U+F3000`**: passa do Nerd Fonts (que para em `U+F1AF0`) e da Lucide (remapeada para `U+F2000`).
  A face pedida por NOME vence a cadeia de fallback, então uma marca dividindo codepoint com um
  glifo do Nerd Fonts sairia desenhada como OUTRO símbolo — não tofu, que se vê, mas um ícone
  bonito e errado. O gerador confere a faixa nas três outras faces antes de escrever, e
  `marks-range-taken-fails` prova a guarda.
* **O AVANÇO É COPIADO, e a caixa é um TETO.** Medido sobre os 6896 glifos `md-` da face
  construída: o AVANÇO é 600 em todos, e é ele que decide quantas células um glifo ocupa. A caixa
  de tinta não é constante — 747 caixas distintas, a mais comum `(0, 40, 600, 640)` em 1992 delas,
  e o `md-battery` em `(28, -114, 572, 794)`. Então o gerador exige avanço IGUAL e tinta DENTRO da
  caixa do glifo de referência. A primeira versão exigia caixa igual, apoiada na afirmação de que
  todo ícone compartilha uma — e recusava desenhos legítimos: uma marca cujos pontos claros são
  menores não alcança a borda de cima. Se o tamanho na tela está certo é outra pergunta, e ela tem
  check próprio (`chrome-icons-match-the-text`), que mede a tinta contra o dígito.
* **Buraco tem de ser buraco.** TrueType preenche pela regra não-zero: um anel é um contorno externo
  e um interno enrolado ao CONTRÁRIO, e enrolar os dois igual dá um disco cheio sem diagnóstico
  nenhum. A área com sinal dos contornos é comparada — o fato em si, e não uma rasterização dele.

**Quatro pontos, e não nove.** O desenho de que ela saiu tem nove numa grade três por três: cinco
claros e quatro cheios. Carregar os cinco claros falhou duas vezes na medição — como anéis no 2,5 do
próprio desenho eles se quebram a 13 px, e no 1,6 pesam 75% de um ponto cheio contra os 31% do
original. Mas os cinco claros nunca eram o ponto: o que distingue esta marca de uma grade de pontos
qualquer — que foi o que o dono recusou — é a FORMA que os cheios fazem, e ela sobrevive sozinha. Com
os outros cinco fora, sobra uma marca legível a 13 px, que não é um menu e não é o logotipo de
ninguém: parecida, e não igual. E o desenho é nosso, então nada aqui se apoia em uso nominativo.

**O que é DESENHADO é centrado, não a grade.** Uma marca pode usar só parte dela — quatro pontos de
nove — e centrar a grade deixa o desenho baixo ou de lado dentro da caixa, o que numa modeline lê
como um glifo que escorregou.

**Os dois tons, decididos OLHANDO** (o registro da tentativa que não vingou): A 22 px — o único tamanho em que esta marca é desenhada, nos
dois sítios que a usam — a marca tem 13 px de tinta e cada ponto 3,3 px. O anel de 1,6 rasteriza
como um QUADRADO vazado de três pixels e pesa 75% do disco ao lado, contra os 31% do desenho
original; no 2,5 do próprio desenho ele se quebra em marcas soltas. O ponto MENOR sai com dois
pixels contra os três do cheio: mais claro, e os nove centros — que é o que o olho reconhece —
continuam todos lá. A escolha é do tamanho, não do gosto: o anel é fiel ao DESENHO em qualquer
tamanho e o ponto menor é fiel ao TOM no tamanho em que isto é desenhado.

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
vazia, que é pior do que não copiar. É a mesma escolha do `scrollmsg`: o cliente que já existe,
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
   `cat` não está lá — `exec cat: No such file or directory`. É a armadilha do `AGENTS.md` um nível
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

## 2. O compositor, e a fronteira com ele

### O compositor é o Scroll, e a razão é o ponteiro

O Scroll é um fork do sway — logo, do wlroots — que segue os lançamentos do sway e troca o layout em
grade por **rolagem horizontal infinita**. Duas razões o escolheram, e a primeira é a que decidiu:

1. **Vários ponteiros ao mesmo tempo.** O compositor anterior dirige um assento com um ponteiro só, e
   isso é o modelo de assento dele, não um ajuste — não há como contornar de fora. Os aplicativos que
   o dono vai escrever são construídos em cima de vários cursores. O wlroots tem multi-seat.
2. **O layout é o que ele já queria**: rolagem infinita em vez de nove espaços de trabalho.

Tudo o mais é consequência — comandos em texto livre, blocos `output` mesclados, `reload` que libera
tudo o que foi aplicado em execução — e cada consequência está escrita ao lado do código que ela
alcança.

### A fronteira é um CONTRATO, e o adaptador é um crate só

O pedido do dono foi explícito: *"tudo que desenvolvermos deve funcionar como um contrato
independente, ou seja, não deve depender de externos como WMs"*. Então não são um crate e sim dois,
e a divisão é esse pedido feito tipo:

- **`ronoctua-compositor`** é o contrato. Vocabulário neutro (`Window`, `Output`, `Naming`, `Mode`,
  `Logical`, `Transform`, `Ask`, `Event`, `State`, `Deed`, `Arity`), um `trait Compositor` e um
  `trait Events`. **Zero protocolo**: nenhum socket, nenhum JSON, nenhuma string de comando — e nem
  `serde_json` entre as dependências, para que a fronteira não possa ser contornada por descuido. É
  o molde do `ronoctua-syntax` e do `ronoctua-atlas`, que já são crates puros devolvendo papel e
  nunca cor.
- **`ronoctua-scroll`** é o adaptador: o enquadramento do IPC do i3 (`"i3-ipc"` + tamanho + tipo), os
  comandos em texto, a tradução dos eventos. **É o único lugar da árvore que sabe qual compositor
  é.**

`ronoctua-shell`, `ronoctua-display` e `ronoctua-rescue` dependem só do contrato.

**E a abstração se paga hoje, em vez de ser cerimônia.** Um `trait` com uma implementação seria
enfeite se o que ele desse fosse trocar de WM um dia. Ele dá três coisas agora: o teste de contrato
passa a exercitar o *trait*, então qualquer adaptador futuro passa no mesmo binário; um
`FakeCompositor` em `#[cfg(test)]` torna testável o que só uma VM alcançava; e "não há compositor"
vira um estado nomeado em vez de um `warn` repetido em seis lugares.

### A tabela de feitos é NOSSA, e o gerador a confere contra a fonte pinada

O compositor anterior enumerava as ações dele e nós as extraíamos do binário. Este não enumera nada:
os comandos são texto livre, espalhados por quatro arrays em C. Então a tabela é declarada em
**`assets/compositor-deeds.nix`** — 112 feitos, no hábito do projeto (`assets/notifications.nix`,
`assets/syntax.nix`) — e de lá saem a tabela Rust e as chaves que os dois idiomas devem.

O que a mantém honesta é o gerador: ele **recusa um verbo que o `sway/commands.c` da fonte pinada não
tem**, varrendo as duas tabelas de handlers com um piso de contagem para que uma varredura vazia não
passe por verde. Um verbo que suma vira falha de build com o id na mensagem — mais cedo e mais barato
que uma VM.

E `wire::spoken` VALIDA o argumento antes de falar: número tem de parsear como `f64`, palavra tem de
casar `[A-Za-z0-9._:+-]+`. Os separadores de comando deste compositor são `;` e `,`, então um
argumento sem validação seria uma superfície de injeção que ações enumeradas não tinham — e há um
teste chamado `an_argument_cannot_smuggle_a_second_command`.

### O que este compositor NÃO responde, e o que ficou no lugar

Três buracos, e nenhum tem substituto igual:

- **Não há `GET_LAYERS`.** Nada no IPC diz quais layer surfaces existem nem quem tem o teclado. O
  `.#diagnostics` perguntava isso em uma linha; agora pergunta se o IPC responde (que é o mesmo fato
  por outra porta) e manda ler o log do rodapé para a metade do foco.
- **O `reload` responde `success` haja o que houver.** Medido contra um compositor vivo: fragmento
  quebrado e config principal quebrado voltaram os dois `{"success": true}`, porque `load_config`
  devolve `config->active || !config->validating || config_load_success` e `config->active` é verdade
  em toda sessão de pé. E `--validate` só julga o arquivo PRINCIPAL: `load_include_configs` é `void` e
  joga fora o resultado de cada include. **Não há evento sobre isso.** Por isso o
  `Snapshot::config_failed` saiu — um campo que nunca pode ser verdade é pior que campo nenhum — e o
  que responde no lugar é o portão de BUILD (`compositor-config-valid`, `display-fragment-validates`),
  mais a regra que este projeto já tinha: `Ok` não é "aconteceu", e quem confirma é a leitura seguinte.
- **Não há captura de tela.** Quem tira a foto é o `grim`, pelo `wlr-screencopy`, e a notificação
  passou a ser NOSSA — reconhecida por uma tag que nós escrevemos, em vez do par (app, resumo) de
  outro programa, que custava um `rewrite` e um check lendo a fonte alheia.

E uma coisa que ele NÃO expõe e a distro recusa por cima: **`scrollnag`**. Ele levanta essa janela
sozinho num erro de config e numa escala que ele considera ruim; é chrome flutuante com botões
próprios sobre uma mesa cuja única chrome é o rodapé (invariante 14), então o config diz
`scrollnag_command -`. O preço está escrito onde a linha está.

### O IPC não tem permissão por cliente

`ipc-server.c` carrega um `// TODO: Check if they're permitted` onde a autorização estaria, e entre os
comandos há `lua_eval` — execução de Lua arbitrário dentro do compositor. **A permissão do arquivo de
socket é a única fronteira que existe.** Isso não muda nada do que fazemos (o socket é do usuário,
sob `$XDG_RUNTIME_DIR`), e está escrito aqui porque é o tipo de fato que não se quer descobrir depois.

---

### Xwayland

**Sim, e embutido.** Este compositor traz XWayland dentro dele — confirmado no binário construído —,
então o `xwayland-satellite` saiu inteiro.

**O que o satellite existia para resolver continua resolvido, por outro caminho.** Buffer estrangeiro
é correlacionado por **PID → cgroup**, e um satellite compartilhado criaria as superfícies em nome
DELE: o `pid` seria o mesmo para toda janela X11 e a correlação quebraria em silêncio. Era por isso
que havia um satellite por scope de buffer. Aqui o compositor reporta o PID do cliente X REAL, pela
extensão XRes, então a correlação vale sem processo nenhum no meio.

---

### Multi-monitor

**Duas linhas em todo output; minibuffer ativo só no de foco.** A modeline é por janela e diz o que
está em foco ali; o minibuffer é por tela e é onde se digita. Dois minibuffers simultâneos criariam a
pergunta "qual deles tem o prompt?", que não tem resposta boa.

Todo output mantém exatamente duas linhas, então o exclusive zone fica uniforme e **o conteúdo não
reflowa quando o foco troca de tela**. A exceção são as barras ocultas: a borda de baixo as revela só
na tela em que o ponteiro está, e as outras ficam num pixel (ver "Os modos ocultos").

**Consequência a respeitar:** o estado do minibuffer vive **no daemon, não na superfície**. Se o
output em foco desaparecer no meio de um prompt, o estado migra sem perder o que foi digitado.

### O redimensionador de tela — `LEADER d r`, e por que são cortinas

É uma ferramenta de desenvolvimento, e o grupo `LEADER d` existe para ela: ver o sistema inteiro
numa tela menor, mudando a largura e a altura com o ponteiro ou com as teclas, com rodapé, coluna
e janelas se rearranjando ao vivo. Ligado, ele põe uma marca na modeline (a cor é a da marca dos
gestos, `MODELINE_WATCHING`), e um clique nela o desliga.

**Cortinas, e não modo de vídeo.** O que se testa é o shell numa tela menor, e trocar o modo testa
o monitor: ele apaga por um instante, só oferece os tamanhos do EDID e mexe nas outras saídas. Uma
cortina é uma superfície layer-shell com zona exclusiva, e o compositor já dispõe rodapé, coluna e
janelas no que as zonas deixam. O que sobra, a sala, fica ancorado no canto de cima à esquerda, e
cada porta faz a mesma conta que faria numa tela daquele tamanho. Quem posiciona continua sendo o
compositor (inv. 4).

**A layer `overlay`.** O Scroll pinado aplica as zonas camada por camada: overlay, depois top,
bottom e background (`arrange_layers`, em `sway/desktop/layer_shell.c`). Rodapé e coluna vivem em
`top`, então uma zona em `overlay` estreita os dois; em `top`, o resultado dependeria de qual
superfície nasceu primeiro. As cortinas são a terceira exceção do inv. 14, aprovada pelo dono: só
existem enquanto o redimensionador está ligado e só cobrem o que ele cortou.

**Duas superfícies.** Uma layer tem uma borda exclusiva só, e o corte tem duas. A cortina da
direita é ancorada em cima, à direita e embaixo, e a zona é a largura dela; a de baixo, à esquerda,
à direita e embaixo, e a zona é a altura dela. A de baixo tem pelo menos uma linha de texto, e é
nela que a sala se diz (`W × H px · C colunas × L linhas`) e as teclas se ensinam, clicáveis, sobre
`surface.header`; o resto das duas é `surface.base`, sem texto. A borda de cada uma é
`rule.strong`, e `capture.guide` quando está acesa ou sendo arrastada. Nenhum papel novo. Sob o
ponteiro, a frase de uma tecla vem logo depois dela, no lugar do que a seguia (`line_of`): no fim da
linha inteira ela era a parte cortada numa sala estreita, e nada até a tecla acesa se move. E a
linha para onde a sala para (`line_width`): quando a cortina de baixo cobre o canto, o quadrado do
canto é a alça dos dois eixos, e uma tecla desenhada ali tomava o clique — medido na VM, um clique
dois pixels dentro do canto estreitava a sala uma coluna em vez de pegá-la.

**O dono do canto.** A cortina que o compositor arranja primeiro cobre o quadrado do canto, e
ninguém decide qual é: cada uma lê isso do próprio configure (a da direita com a altura toda, ou a
de baixo com a largura toda). O desenho não depende da resposta. O quadrado do canto é a alça dos
dois eixos e desenha os dois lados internos, então as duas bordas atravessam a tela nas duas
ordens. Cada alça é pega no retângulo cujos lados ela desenha (`resizer::grips` e
`resizer::edges`).

**O arrasto é medido no quadro da pegada, sem origem.** Enquanto um botão está preso numa layer, o
Scroll pinado informa o ponteiro como o ponto local do press mais o quanto o cursor andou na tela
(`handle_pointer_motion` do `seatop_down`, e o toque faz a mesma conta), e não refaz essa base
quando a superfície se move. Por isso o arrasto é só a posição informada menos a posição do press
(`Drag::delta`). A primeira versão somava a origem nova da cortina, tirada de cada configure, e
contava de novo cada passo já dado: com a mão parada, a sala corria até o piso.

**O settle.** O commit que mapeia a cortina não leva estado de layer, e o compositor não manda o
arranjo às janelas nele (o mesmo defeito da coluna do player). O primeiro quadro pede o callback, e
o `frame_done` chama `Compositor::settle`. Depois disso, mudar tamanho e zona manda o arranjo
sozinho, e destruir as cortinas devolve a tela sozinho.

**Três processos.** Todos são `ronoctua-shell`:

- `resizer` é o guardião e não tem superfície. Mede a tela em foco pelo IPC, escreve o estado, sobe
  as duas cortinas e espera. Quando uma sai, ele encerra a outra e apaga os arquivos: as duas
  cortinas, ou nenhuma. Sobrevive ao hot reload do `.#dev` e morre com o cgroup do rodapé em
  produção; o rodapé que sobe varre o que um guardião morto por sinal deixou (`sweep_stale`).
- `resizer-curtain right` e `resizer-curtain bottom` são uma porta cada. Rodam com `pdeathsig TERM`
  e conferem o pai pelo `/proc/<ppid>/cmdline`.

Uma porta por processo, porque `theme::reload()` e `toolkits::reload()` guardam o "mudou?" do
processo: a segunda porta do mesmo processo nunca repintaria com tema ou tamanho de texto novo.

**O arquivo é o broadcast.** `runtime_file("resizer.json")`, por instância, em px lógicos, escrito
por `write_atomically`, existe exatamente enquanto o redimensionador está ligado; o guardião fica em
`resizer.pid`, conferido pela linha de comando e nunca pelo nome. Quem escreve mescla por campo e
nunca escreve sobre um arquivo que não conseguiu ler (ausente quer dizer desligado). A cortina que
arrasta ignora o próprio eixo ao ler, e é assim que ela reconhece a própria escrita. Ela escreve
ANTES do commit, então o configure que a outra recebe já está no arquivo; só um eixo longo que nem
o arquivo explica é uma tela que mudou, e só esse caso custa uma pergunta ao compositor.

**As teclas andam na grade.** As setas levam ao menor tamanho que contém uma coluna ou uma linha a
mais ou a menos (`stepped`), então cada tecla muda o layout do rodapé em exatamente uma unidade.
`Shift` anda `space.resizer.bigStep`; `f` volta ao maior tamanho, que é a tela menos as duas
faixas finas que as cortinas guardam para continuarem pegáveis. O piso é `minColumns` ×
`minLines`, e numa tela menor que o piso as cortinas ganham (`fitted`): mais finas que isso, elas
não poderiam ser pegas de volta.

**Rodapé e coluna medem a sala, não o monitor.** `expansion_ceiling` media o monitor, e uma lista
longa passava do topo de uma tela cortada. Agora ele mede a sala mais baixa entre as telas
(`resizer::smallest_room`, sobre `room_height`). A coluna tinha o mesmo defeito na largura: as
cortinas não mudam saída nenhuma, então nenhum `ScreensChanged` chega, e o teto de `maxPct` saía do
monitor. Uma coluna média ocupava 77% de uma sala de 700 px e, abaixo de uns 540 px, saía pela borda
esquerda da sala. Agora o teto sai da sala (`column::room_of`, sobre `resizer::room_width`), e a
coluna vigia o `resizer.json` como o rodapé e diz a largura de novo quando a sala muda.

**`d` era o relance, que foi para `g`.** O dono deu o `LEADER d` ao grupo de desenvolvimento. O
relance (`world-glance`) é `g`, ligado pela constante `GLANCE_KEYS`, que é a mesma que as frases
ensinam.

**O que ele não simula.** Tela cheia ignora a área útil (`sway/tree/arrange.c`), e as cortinas
cobrem as bordas dela. Janelas flutuantes, popups de apps, a captura, a visão geral e o ponteiro
dos gestos usam a saída inteira. Só a tela em foco é cortada. Ligar com o rodapé no `overlay` (uma
janela em tela cheia o cobrindo) arranja estranho até a tela cheia acabar.

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

**O compositor não roda no TTY de uma VM sem GPU.** O backend DRM rejeita renderizadores EGL por
software, e numa VM o único EGL é o llvmpipe. Ele então sobe, cria o socket e fica com **zero
output** — não falha, não avisa — e uma superfície layer-shell precisa de output. Por isso os testes
escolhem o backend E o renderizador: `WLR_BACKENDS=x11` sobre `Xvfb` onde se precisa de pixels,
`headless` com `WLR_RENDERER=pixman` onde só se precisa do parser. O segundo par é o que tira EGL,
GBM e o nó DRM do caminho por inteiro, e é o que faz `--validate` caber num sandbox de dez
milissegundos.

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
meses se olhar custasse pouco. Custava uma VM NixOS: subir, o compositor sob Xvfb, capturar a raiz
do X, recortar pela geometria que ele informou, passar OCR. Três minutos para ver se uma régua encostava
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

**E porta nativa não é buffer.** O `render-png` é um binário à parte que empresta dois módulos
puros, e a janela e a coluna do player puxam o shell inteiro. Por isso a câmera das portas mora
DENTRO do binário — `ronoctua-shell photograph`, que o `nix run .#png -- --door …` chama —, e quatro
decisões a seguram:

- **A porta é a de produção, desligada** (`door::Detached`). O `Ui` está numa conexão que ninguém
  serve, a coluna é a de produção em volta do player, e a tecla e o clique descem pelos caminhos da
  porta (`typed_into`, `pointer_into`, `act_into`). O quadro sai de `door::compose`, a função que o
  `Door::paint` chama; só muda onde os pixels caem — uma textura lida de volta pelo `offscreen`,
  como no `render-png`. Um fotógrafo com pintor próprio seria a segunda aritmética que o
  `render-png` existe para recusar.
- **O player não tem por onde agir.** Tudo o que sai da porta — perguntar ao daemon, gravar as
  preferências, parear, abrir a janela do palco, perguntar ao compositor, a thread de cada imagem — é
  método de um campo só (`player_window::Reach`), e o braço `Kept` guarda em vez de agir; a foto
  imprime o que teria feito. Antes eram seis `if` sobre um `Option`, e uma sétima chamada passaria
  por fora dos seis: um clique numa foto pausaria o filme da pessoa. Sob `cargo test`,
  `daemon::about_to_act` derruba o teste antes do ato — sem ele, um palco aberto num teste subia o
  binário de teste de novo como `player maximized`, e o libtest lê isso como um filtro que casa o
  próprio teste.
- **O daemon responde pela caminhada.** O `--state` é uma resposta do socket a `{"ask":"look"}`, na
  grafia do daemon, e não um segundo formato inventado para a ferramenta. Ele vale na ordem em que
  aparece: antes de qualquer passo é o estado inicial, depois de um clique é a resposta a ele. O
  quadro de vídeo, que chega por memfd, vira uma imagem parada (`--frame`).
- **A mesma foto em toda máquina.** Tema, página, tamanho do texto, forma da data e preferências do
  player são os pedidos ou os padrões, nunca os arquivos da pessoa; o navegador de arquivos lista
  `--places`, e uma caminhada que o abre sem isso é recusada. O relógio só anda entre passos: dois
  cliques são um duplo porque a caminhada diz `double`, e não porque a máquina foi rápida. O que não
  caberia na foto — uma imagem parada que nenhuma imagem usou, uma célula fora da superfície, um
  tamanho dito duas vezes — é recusado com uma frase.

O `png-tool-draws` abaixo ainda não passa pelo `--door`.

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

**Um compositor aninhado que não responde ao `wl_display.sync` não está travado.** Ele é **cliente do
compositor do host**: com a janela numa workspace oculta, o host para de alimentá-lo e ele dorme — e
dormindo não serve os próprios clientes nem o IPC. Com a janela visível, o mesmo `sync` responde em
0,00 s.

**Modificador preso é o que se parece com tudo o mais.** Com ninguém tocando no teclado, o compositor
chega a anunciar `alt=true logo=true`; o `logo` é a impressão digital do atalho do host, que captura
o `Super+…`, e o compositor aninhado via a descida e nunca a subida. Um fato explica três sintomas:

- **A leader para de disparar** — um bind é casado por máscara EXATA, e o que chega tem um bit a
  mais do que o bind pede.
- **O clique não chega aos buffers, mas arrasta a janela** — no laço aninhado o `$mod`
  **é o Alt**, e `$mod+arrastar` é mover janela. Com o Alt preso, todo clique vira arrasto. Foi esta
  observação do dono que fechou o caso.
- **O foco ainda muda no clique** — normal, e é o que fazia parecer que "o mouse funciona".

Recarregar o keymap não limpava o latch; ele está no estado xkb do compositor, e o invariante 3
proíbe consertá-lo lá dentro. O que sobrava era reconhecê-lo depressa.

**O mecanismo era do compositor anterior, e não deste.** Lá o estado de modificador era CONTADO —
pressões contra soltas —, então uma tecla solta fora da janela nunca voltava. O backend Wayland do
wlroots implementa `.modifiers` e passa a máscara do HOST direto, com `update_state = false` no
handler de tecla: o estado aqui é espelho do de lá, nunca derivado. Isso ainda **não foi medido**
neste compositor, e por isso a defesa (`nix/keyboard-nested.nix`) continua de pé; a medição está no
`docs/PENDING.md`.

Disso saíram três coisas, e a mais importante não é o conserto:

**O log do rodapé vai para arquivo.** Ele ia para o terminal e mais nada, e o do compositor tinha as quatro
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
para baixo: a faixa de notificação (0 a 3 linhas), a linha do relance ou do endereço do quiosque,
a modeline, o minibuffer (com a gaveta dele quando aberta, e a linha que explica a dica sob o
ponteiro), a linha do cérebro, e então a lista — which-key, candidatos do localizador,
gerenciador, manual —, com o teclado na tela embaixo de tudo. Com as barras ocultas, a modeline e o
minibuffer podem faltar, um ou os dois, e o minibuffer sozinho sobe para a linha da modeline: quantas
linhas as barras ocupam é o `hidden::Shape` que diz, nunca uma constante (ver "Os modos ocultos").

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
linhas numa tela 4K. O que passa do teto vira **página**. O percentual é da altura que o rodapé
tem: a do monitor, ou a que o redimensionador de tela deixa (`resizer::room_height`).

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

Com as barras ocultas isso deixa de valer, por pedido do dono: com `LEADER Alt LEADER` tudo cobre, e
com `LEADER LEADER` cobrem o minibuffer e a lista da leader enquanto nada empurra (ver "Os modos
ocultos"). O argumento acima continua verdadeiro lá, e é o preço aceito.

**O teclado sai na hora, num commit só dele; a altura espera o quadro** (`commit_now` e
`configure_repeats_itself`, em `surface.rs`). A altura nova vai junto com o quadro desenhado nela,
para o compositor nunca mostrar o quadro velho na caixa nova — o salto. O teclado não tem imagem a
esperar, e ia no mesmo commit só porque era posto ao lado da altura: ficava preso enquanto o quadro
era desenhado, uns 90 ms para a primeira lista da leader no renderizador por software da VM, e uma
tecla digitada nesse intervalo ia para a janela atrás do rodapé, ou para lugar nenhum (o `apps-vm`
digitava `a` logo depois de `leader active`, e o localizador nunca abria). Agora ele sai sozinho,
num commit sem tamanho e sem buffer. O `configure` que responde a esse commit repete a altura antiga
e chega DEPOIS do quadro já desenhado na nova; tomado como ordem, ele redimensionava de volta e a
modeline pulava por um quadro — por isso `configure_repeats_itself` o reconhece e não muda nada.
Juntar os dois commits de novo traz de volta a tecla perdida.

**A modeline só mostra o que existe.** Sem janela em foco e sem nada acontecendo ela fica VAZIA —
não há contador de notificações, e o sino só existe com não lida (o dono pediu assim); assim que algo
abre, o nome do buffer aparece. Um rótulo de ausência inventado ocuparia coluna afirmando algo falso,
e quem o lê procura um buffer que não existe.

A sequência pendente aparece como `LEADER g-` — e `LEADER ALT g-` ou `LEADER CTRL s n-` fora do
conjunto padrão (ver "Três conjuntos da leader"). O `-` no fim custa duas colunas a menos que ` …`
dizendo a mesma coisa, e num rodapé onde tudo elide, espaço é o recurso.

**O rótulo da leader vem do ambiente**, posto por quem define o bind, e são **dois**:
`RONOCTUA_LEADER` (a sequência) e `RONOCTUA_LEADER_CHORD` (o que se segura). Em produção os dois são
`SUPER` — e essa coincidência é o que o keyd compra. No aninhado eles divergem: a sequência é
`CTRL ALT SPC` e o acorde é o `ALT` sozinho. Enquanto a tela chamava os dois de "a leader", segurar "a
leader" parecia incluir o Espaço — e incluir o Espaço **dispara a sequência**, que é o oposto do
acorde. Deduzir um do outro seria a tela ensinando o gesto errado.

---

### Os modos ocultos — `LEADER LEADER` empurra, `LEADER Alt LEADER` cobre

O dono pediu um rodapé que some: modeline e minibuffer fora da tela em repouso, de volta quando se
pede. São dois modos, e o que os separa é o que o rodapé faz com as janelas enquanto mostra alguma
coisa. A tabela é dele, e `hidden::Shape::of` a responde inteira (o teste percorre todas as
combinações):

| | `LEADER LEADER` — empurra | `LEADER Alt LEADER` — cobre |
|---|---|---|
| o minibuffer sozinho (eco, pergunta, a sequência) e a lista da leader | por cima, com véu | por cima, com véu |
| uma vista-app do rodapé aberta | empurra, sobre a mesa | por cima, com véu |
| a faixa de notificação, a linha do cérebro | empurram, com o fundo delas | por cima, com o fundo delas |
| o teclado na tela | empurra, com o fundo dele | empurra SOZINHO (só as linhas dele), com o fundo dele |
| o ponteiro na borda de baixo | as barras empurram, sobre a mesa | as barras por cima, com véu |
| um quiosque em foco | as barras empurram, sobre a mesa | nada: só a borda revela |

"Sobre a mesa" era "sem fundo" — a limpeza de alfa 0, com a mesa aparecendo por trás — até o dono
pedir (2026-09-28) a cor da mesa, opaca: o mesmo aspecto, sem a franja branca que o texto desenhado
sobre nada ganha (ver "Transparente só onde a superfície deixa", abaixo). As linhas que empurram
VESTEM a página da mesa (`hidden::Shape::page`; ver "As faixas seguem o fundo", em "Cores"), então o
fundo delas é simplesmente o `surface.chrome` dessa página, que É a mesa, e texto, hover e seleção
sobre ele foram medidos ali.

**O gesto é a leader de novo, na raiz.** Com a leader aberta e nada digitado — nem tecla da árvore,
nem o `,` do buffer, nem o `w s` —, a leader chegando outra vez esconde as barras no conjunto padrão
(empurrar) e no das janelas (cobrir), e as devolve quando o modo já é aquele; de um modo para o outro,
vai ao pedido (`hidden::leader_again`, `hidden::toggled`). Só com a leader À VISTA: ativa sob uma
vista-app ou o localizador de comandos, ela não está na tela — a vista continua com a região e as
teclas —, e a leader de novo ali recomeça, em vez de esconder as barras a partir de uma lista que a
pessoa não vê. No conjunto completo, ou com algo digitado, ela recomeça a sequência como sempre fez: a árvore inteira é onde se vai para digitar, e
uma sequência recomeçada por engano custa menos que barras escondidas por engano. Os dois gestos são
também comandos (`footer-hide-push`, `footer-hide-cover`, pelo nome no `LEADER :`), e a raiz da lista
ganha uma linha cuja tecla é o rótulo da própria leader (`hidden::bars_row`): a tecla do gesto não
cabe em árvore nenhuma, e sem a linha o ponteiro não a alcançaria (invariante 19). Clicar nela é a
leader chegando (`Click::Leader`), e não uma tecla.

**O modo é preferência, em `modeline.json`** (`adapt::Prefs::hidden`, `"push"` ou `"cover"`).
Ausente é visível. Uma palavra que não é nenhuma das duas recusa o arquivo com a frase que toda
chave dele já diz, em vez de voltar calada ao visível — no gesto, na vigia do arquivo
(`adapt::reread`, que lê com `parsed` e mantém a cópia que o rodapé tem) e na partida, onde não há
cópia: o rodapé sobe com o padrão e diz a frase uma vez. O gesto lê o arquivo, mescla e grava, então o
modo sobrevive ao reinício, e outra mão que o mude — outro rodapé, o arquivo editado — move este
também.

**Por que tudo empurra quando qualquer coisa empurra.** O layer-shell reserva a zona contada da
borda ANCORADA — a de baixo — e a faixa cresce no TOPO da superfície. Uma zona que cobrisse a faixa
e não as linhas abaixo dela não se pede. Então, empurrando, a superfície inteira é a zona, e nada
nela leva véu: com a faixa ou o cérebro no ar, o minibuffer que estava por cima passa a empurrar
junto e perde o véu. É a geometria do protocolo, escrita e não contornada. Cobrindo, nada reserva.

**O teclado na tela empurra em todos os modos** (`hidden::Inputs::keyboard_out`, decisão do dono
de 2026-09-28). Ele digita na janela em foco, e por cima dela cobriria justamente a linha em que se
digita — o prompt de um terminal. No `Push` ele empurra junto com o resto; no `Cover` empurra
SOZINHO: é a linha de baixo da superfície, e a zona conta da borda de baixo, então ela reserva só a
altura dele (`hidden::geometry`, `keyboard_lines`) e o que está acima continua por cima das janelas,
com o véu. Cobra `the_keyboard_pushes_in_every_mode`.

**Por cima, a largura toda: a camada `overlay` e a zona `-1`** (`hidden::geometry`, decisão do dono
de 2026-09-28). Uma superfície de zona 0 é posta dentro do espaço que as EXCLUSIVAS deixam, e a
coluna da direita é uma (`top`, ancorada em cima, à direita e embaixo, reservando a largura dela): o
rodapé oculto que não reservava nada saía estreitado pela coluna — modeline, minibuffer, faixa e
lista sem a largura dela, as marcas contadas da borda direita deslocadas, e o pixel da borda parando
antes do canto. É o invariante 14. A zona `-1` é o "ignore as outras zonas" do protocolo, e a camada
`overlay` põe o rodapé ACIMA da coluna, que está em `top` — na mesma camada, a coluna criada depois
dele o cobriria.

**Oculto, uma camada só.** Empurrando, o rodapé oculto CONTINUA na `overlay` e só a zona muda (a
altura dele no lugar de `-1`). Ele voltava para `top` para empurrar, e isso era um laço: uma
superfície que muda de camada sob o ponteiro recebe um `leave` do compositor, o `leave` esconde o
que a borda revelou, o ponteiro está na borda de novo, e o rodapé virava a cada ~15 ms — 1044 voltas
em três segundos, medido no palco, com as janelas pulando a altura do rodapé a cada volta (o dono viu
o conteúdo "duplicado" e não conseguiu fotografar). Na `overlay` ele também é reservado PRIMEIRO (é
a primeira camada arranjada), então a coluna em `top` continua parando acima dele. Com as barras
visíveis ele fica em `top` com a zona do tamanho dele, como sempre; a camada só muda quando um modo
é trocado, por tecla. Uma camada que muda é mandada na hora, num commit sem quadro (o comentário do
`apply_geometry`). Cobra `the_edge_never_moves_a_hidden_footer_to_another_layer`.

O preço, escrito: oculto, o rodapé ignora as cortinas do redimensionador de tela (`LEADER d r`,
ferramenta de desenvolvimento) — por cima, a zona `-1` ignora toda outra zona; empurrando, ele é
reservado antes das cortinas, criadas depois na mesma camada. Nos dois casos ele não senta na tela
menor que elas fazem: fica na borda de baixo de verdade, e a cortina de baixo é desenhada por cima
dele.

**Quem empurra é uma vista-APP** (`ViewState::is_an_app`). A lista da leader, o localizador de
comandos e o menu da janela ficam de fora: são o teclado passando a caminho de outra coisa, e ficam
por cima como o minibuffer de onde pendem.

**Isto é a exceção a "a expansão empurra os buffers".** O argumento de lá — cobrir esconde a linha
que se estava lendo — continua verdadeiro, e é o preço que o dono aceitou: sob o véu, texto sobre
uma janela movimentada pode ficar difícil de ler.

**Em repouso, um pixel transparente, sem zona própria** (`hidden::geometry`, `pixels_of`), por
cima, com a largura toda. É a borda que o
ponteiro acha. Só a última linha de pixels revela (`hidden::at_the_edge`, contra a altura que o
compositor CONFIGUROU, `Screen::asked`: a que um quadro desenhado adiante já mudou não é a da
superfície a que as coordenadas do ponteiro pertencem, e o ponteiro voltando por cima do rodapé que
acabara de encolher estava "na borda" em qualquer `y`); reveladas, as barras
ficam enquanto o ponteiro estiver em qualquer ponto do rodapé, e sair dele as esconde. A superfície
inteira não serve de gatilho por causa da faixa: com ela no ar, o ponteiro atravessando-a para
apertar um botão traria as barras para baixo dela, e a superfície, ancorada embaixo, cresce para cima
— o botão fugiria da mão. O preço é o do `Edge` da tela cheia, que mantém a precedência: aquele pixel
rouba o clique da última linha de pixels das janelas.

**A borda é de uma tela — enquanto nada mais está no rodapé.** As linhas são contadas uma vez para
todas as telas, como sempre, e o `Shape` é um só. Com nada mais à mostra, só a tela em que o ponteiro
está cresce: a outra fica no pixel (o `revealed_on` guarda o conector da tela em que ele está) e não
desenha nele nada das linhas que a primeira mostra. Com outra coisa no rodapé — um eco, uma vista, a
faixa —, que aparece em todas as telas, como as duas linhas sempre apareceram, uma borda revelada
põe as barras em TODAS: em `Push`, a zona delas também, e as janelas da outra tela sobem enquanto o
ponteiro fica na borda; cobrindo, a outra tela desenha a modeline por cima. É o preço de as linhas
serem uma aritmética só para todas as telas (a decisão do dono foi mantê-lo); separar exigiria um
`Shape` por tela, na geometria e na pintura. Uma tela que chega com as barras ocultas nasce na
geometria que as outras têm (`create_screen`), e não nas duas linhas: nada que um monitor plugado muda
pede geometria, e ela ficava com uma faixa sob o véu reservando duas linhas. Uma tela que sai leva a borda junto: nenhum `leave` virá de uma superfície que não existe
mais. Clicar no minibuffer revelado e vazio abre a leader com a lista na hora, e é por aí que o
ponteiro chega aos dois gestos.

**Nos dois modos**, o minibuffer aparece quando tem o que dizer: uma vista com o teclado, um eco (o
de um evento inclusive), uma pergunta, o volume ou o brilho, a gaveta aberta
(`Footer::minibuffer_busy`). A modeline nunca vem sem o minibuffer embaixo; sozinho, ele sobe para a
linha dela. As marcas da modeline somem com ela. Num quiosque em foco, empurrando, as barras ficam
enquanto o foco for dele: o quiosque não tem barra própria, e a modeline é onde estão os controles
dele. Cobrindo, só a borda.

**Linha que não é desenhada não é acertada.** `modeline_placed`, `adapter_drawn`, as marcas da linha,
o chevron da gaveta e o contador de páginas são escritos na pintura, e a pintura de uma linha oculta
os escreve vazios; `segment_at`, `hint_under`, `pointer_on_fold` e `on_the_empty_minibuffer`
perguntam o `shape` antes de tudo.

**As linhas das barras deixaram de ser constante.** `FOOTER_LINES` era a soma, e agora ela é
`shape.lines()`, de zero a dois. O `apply_geometry` guarda o `Shape` num campo, e toda soma de linhas
lê o campo: essas somas estão sob o `metric`, e um `Shape` calculado dentro delas seria o rodapé
perguntando a si mesmo o que contém (`the-footer-does-not-ask-itself`). **A guarda é uma só**: o
`draw_and_paint` compara o `Shape` de agora com o do campo e pede geometria quando diferem. Um eco, o
foco indo para um quiosque, o volume, a faixa, a lista parando de esperar — nenhum deles pedia
geometria, porque com as barras visíveis as linhas que eles tocam sempre estavam lá; ocultas, cada um
tira ou põe uma linha, e perguntar no quadro é perguntar uma vez por todos. O teto da expansão
continua contando as duas barras: um teto que as seguisse encolheria uma vista aberta cada vez que a
mão tocasse a borda.

**Transparente só onde a superfície deixa.** O rodapé pede ao `Gpu` uma superfície que o compositor
componha com o alfa dela, e recebe o modo `PreMultiplied` quando a superfície o oferece
(`gpu::alpha_mode_for`); toda outra porta continua com o primeiro modo oferecido, opaco na prática,
como antes. O quadro limpa com a cor que quem chama passa (`Gpu::frame`), multiplicada pelo próprio
alfa numa superfície pré-multiplicada (`gpu::clear_of`): escrita como está, uma cor de limpeza com
alfa zero seria SOMADA à janela de trás — um brilho onde se queria um buraco. Oculto, o rodapé limpa
com `surface.clear`, não pinta o fundo da modeline e pinta, antes de tudo, o fundo das linhas da
modeline até o chão da região (`hidden::Shape::grounds`): `surface.veil` enquanto elas ficam por
cima das janelas, e o `surface.chrome` da página da mesa, OPACO, enquanto empurram — para que
cérebro, teclado, seleção e hover pintem os próprios fundos sobre ele. O teclado na tela ganha um
fundo próprio (o `surface.chrome` da página que a tela veste), porque o ar entre as teclas era a cor
de limpeza. Quads, imagens e o texto do glyphon misturam com `ALPHA_BLENDING`, que sobre um destino
pré-multiplicado é o `over` pré-multiplicado nos valores LINEARES. O alvo é sRGB, então o que se
grava é `encode(c × α)`, e um compositor que mistura valores codificados (o GLES2 e o pixman do
wlroots) espera `encode(c) × α`. Sobre o véu, a diferença é o véu um pouco mais claro que o número
dele, afinado a olho. Sobre NADA (alfa 0) o texto não sai pré-multiplicado válido: na borda de um
glifo o rgb passa do alfa, e sobre qualquer coisa que não seja preto sai uma franja branca em volta
da letra. Por isso nenhuma letra fica sobre nada: as linhas que empurram eram desenhadas assim, com
a mesa aparecendo por trás, e pagavam a franja até o dono pedir a cor da mesa pintada (2026-09-28) —
o mesmo aspecto, opaco. Alfa 0 sobra só onde não há letra: o pixel da borda em repouso e a limpeza
sob o véu. Posto por cima de uma janela em tela cheia pela borda, o rodapé é pintado como o visível
(`hidden::Shape::painted`), em qualquer modo: linhas no véu ficariam sobre o filme ou o slide,
ilegíveis, e linhas na mesa seriam uma faixa de mesa recortada numa janela que nunca saiu dali.

**Os dois tokens são o chrome com alfa**: `surface.clear` a zero, `surface.veil` a quatro quintos —
o chrome DA PÁGINA que a tela veste, então sobre um rodapé posto na mesa o véu é a mesa a quatro
quintos. O matiz do chrome, e não uma cor própria, porque a superfície que não consegue ser
transparente limpa com o chrome que sempre teve, e não com preto. O texto sobre eles fica com os
tons que o `Pen` escolhe para `surface.chrome`, e o `every-ground-is-measured` os dispensa por
escrito, numa lista própria (decisão do dono): o que está embaixo do véu é uma janela, e nenhum
portão sabe qual. A linha que empurra não está nessa lista: ela é a mesa, e as mesas são medidas.

**Sem o modo, o rodapé diz.** A superfície fica no primeiro modo oferecido, o journal registra
`the surface cannot be see-through here`, e o rodapé põe `echo-footer-opaque` no minibuffer uma vez,
na primeira vez que um modo precisa dele. Sem a frase, a pessoa veria as barras sumirem e uma faixa
de chrome ficar, e nada dizendo por quê.

**A lista espera a mão parar** (`leader::Patience`; `LIST_PATIENCE`, 2 s, ao lado de
`ECHO_SECONDS`). Só com as barras ocultas, e só quando uma TECLA abriu a leader: quem sabe a
sequência a digita de um fôlego, e uma lista abrindo por cima das janelas a cada leader passaria
piscando a caminho do comando. A sequência pendente aparece na hora; a lista, quando a mão para,
contando da ÚLTIMA tecla — quem lê o primeiro nível e depois digita é exatamente quem não deve
esperar de novo. Mostrada uma vez, fica mostrada até a sequência acabar: os níveis seguintes e a
troca de conjunto aparecem na hora. Aberta pelo ponteiro (o minibuffer vazio, a linha enxertada, um
`leader-set-*` pelo nome), ela aparece na hora: a mão no mouse não digita rápido, e dois segundos de
nada seriam um clique que errou — e isso vale também para um conjunto escolhido pelo ponteiro ou
pelo nome numa leader que uma tecla abriu e cuja lista ainda espera (`leader_set_by_name`); o toque
de `Alt` ou `Ctrl` continua esperando. O relógio é numerado como o do `wait_for_the_keyboard`: cada tecla
arma um novo, e o antigo que dispara acha o número velho e não faz nada. Enquanto espera, a região
não toma altura, e com ela saem a linha que explica a dica e o contador de páginas.

**O vidro fosco ficou de fora, por decisão do dono**: o compositor pinado não desfoca o que está
atrás de uma superfície de camada, e o véu é translúcido, não fosco.

---

### Preto puro, e o foco visível

O fundo do sistema era **`#000000`**. Numa tela OLED o preto é pixel desligado: contraste máximo e
menos bateria, que é o que importa num laptop. Superfícies — rodapé e buffers — são cinza escuro, e
a separação entre conteúdo e chrome passa a ser a diferença entre preto e cinza, sem régua nem
caixa. Hoje a mesa é uma resposta da pessoa (`LEADER s t`; ver "A mesa, o rodapé, as janelas e as
bordas", em "Cores"): o preto continua sendo a mesa absoluta de todo tema escuro
(`surface.absolute`), e o padrão é a cor do tema.

**A borda de foco de 2 px é do compositor, não nossa.** Com `border` e `focus-ring` desligados, duas
janelas abertas eram indistinguíveis: nada na tela dizia qual recebia tecla. Uma linha não é card,
sombra nem elevação (invariante 15) — é a mesma classe da régua de uma linha que o desenho já admite
como única separação além do espaço em branco. Quem a desenha é o compositor (invariante 4).

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

As três cores do config do compositor — acento, borda inativa e fundo — vinham escritas à mão, iguais por
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
| `*lucide*` | **qual** buffer é este? | o TÍTULO da janela, pelo compositor |
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

**O `app_id` NÃO é traduzido**, e isso é deliberado: as regras `for_window` do compositor casam por ele, e uma
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
  esta janela o release e nunca o press. É a mesma armadilha que o `AGENTS.md` registra para a
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

**Janela estrangeira não tem estado**, e é a mesma regra que apagou o rótulo `fundamental`: o compositor
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

### A modeline que se adapta — um adaptador por programa

Alguns programas têm uma barra de ferramentas inteira que este sistema esconde: o Chrome quiosque
esconde as abas e a barra do Chrome. Para esses, a modeline vira a barra escondida. Um adaptador
(`adapt::ModelineAdapter`) reivindica a janela em foco (`claims`), desenha segmentos sobre ela e
responde ao ponteiro neles. Hoje há um, o do quiosque (`kiosk::Adapter`), e a forma foi escolhida
para aguentar outros.

**O título sai, e o programa vem primeiro.** Quando um adaptador toma a janela, o `buffer` (o
título) sai, porque a aba da frente diz melhor o que ele dizia, e `adapt::graft` põe o `mode` (o
nome do programa) como primeiro item: prioridade 4, a do título, não removível, com piso de quatro
células (`goo…` ainda distingue o Chrome). Os segmentos do adaptador entram logo depois dele. Sem o
título, o acento que ia para ele quando o rodapé toma o teclado vai para o `mode`
(`adapt::mode_colour`).

**Um enum fechado, e não um objeto de trait.** O despacho é o `enum Adapted`. O rodapé pergunta ao
adaptador enquanto também se altera, e um `&dyn` emprestado do rodapé brigaria com cada um desses
empréstimos. Um adaptador novo é uma variante e um braço em cada método.

**O que foi desenhado responde ao ponteiro.** É a regra do resto da modeline: ao lado de
`modeline_placed`, o rodapé guarda `adapter_drawn`, o alvo de cada segmento desenhado, e clique,
botão do meio, roda e arrasto leem essa tabela, nunca um segundo layout. O arrasto arma no press e
age no release: soltar no mesmo segmento é clique, soltar em outro é mover. A modeline ganhou roda
(`wheel_lines`) sobre os segmentos adaptados.

**O adaptador não roda nada.** Toda resposta dele é um comando do registro (`adapt::Act`), e a vista
do adaptador e o grupo `,` alcançam os mesmos comandos, porque os três saem de uma tabela só
(`kiosk_view::ROOT`): a grade da vista gera o `,`, como o menu da janela gera `LEADER w`. O
`footer-mouse-reaches-nothing-new` exige que todo comando que um segmento roda esteja no registro e
tenha tecla. O `,` de um quiosque roda no rodapé (`local_runs_here`): o Chrome não publica keymap, e
a sequência vai por `run_with`.

**A faixa contada, em duas passadas.** As abas são uma faixa (`adapt::Strip`) com um contador `+N`
de cada lado para o que não coube. O contador diz quantas NÃO foram desenhadas, e quais foram é o
layout que decide: a conta não existe antes do layout, e a largura dela entra nele. `adapt::resolve`
faz duas passadas. A primeira reserva os contadores na largura máxima (a faixa inteira contada); a
segunda mantém só o que sobreviveu, escreve o número de verdade (ou tira o contador que dá zero) e
dispõe de novo. Todo sobrevivente fica no máximo tão largo quanto era, então nada que sobreviveu
some e nada que saiu volta: a mesma entrada dá o mesmo quadro. O número de prioridade de uma aba
vizinha cresce com a distância da aba da frente, então as mais distantes saem primeiro, os
sobreviventes são contíguos e os contadores contam o que ficou fora deles.

**Pools, como os números dos espaços.** O id de um segmento é `&'static str`, então os membros são
nomeados pelo lugar (`kiosk-tab-1` a `kiosk-tab-16`, `kiosk-ext-1` a `kiosk-ext-8`), e o adaptador
diz a que cada lugar se refere. O `footer-mouse-reaches-nothing-new` nomeia os dois pools.

**Liga e desliga por programa.** `modeline.json`, escrito pelo `ronoctua-store`, guarda uma chave
por adaptador (ausente é ligado) e se a linha de endereço do quiosque está de pé (ausente é
desligada). O arquivo é o broadcast: o rodapé o vigia e reconhece a própria escrita. O comando é
`modeline-adapt-kiosk`, que a vista, o `,` e o `LEADER :` alcançam.

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
  de log em lugar nenhum onde alguém estivesse olhando — o rodapé é uma unidade, e o que ele diz ao
  cair vai para o journal de um serviço que ninguém abre enquanto uma janela some.
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

1. **`duplicate keybind`** — um padrão do compositor contra o `Ctrl+Alt+l` de emergência: a
   mesma tecla escrita em outra ordem. A deduplicação comparava **strings**.
2. **Dois blocos de layout** no config de produção, desde sempre, e o compositor daquele dia recusava
   nó duplicado.

Um config recusado não é um atalho quebrado: é o compositor caindo no **config padrão dele** — sem
nenhum atalho nosso e sem a leader. E o único sinal era uma caixa vermelha na tela.

O que faltava era perguntar a quem tem a resposta. O `compositor-binds.nix` grepava por uma palavra e
o módulo grepava por outra; nenhum dos dois perguntava ao compositor se o arquivo **inteiro** fecha.
Agora o `compositor-config-valid` roda `scroll --validate` sobre o config de produção, e o
`display-fragment-validates` faz o mesmo com o fragmento que a vista de telas escreve.

**E o veredito é a SAÍDA, nunca o `$?`.** Medido: um erro dentro de um `include` imprime
`Error(s) loading config!` e **sai zero**, porque `load_include_configs` é `void` e joga fora o
resultado de cada include. Um portão no status teria passado sobre todo fragmento quebrado que existe.

**A migração para este compositor mostrou por que isso vale.** Dois defeitos novos passaram por tudo
e só este comando os viu: `bindsym Mod+Left`, onde `Mod` não é um modificador aqui (a tabela é
`Shift Lock Control Ctrl Mod1 Alt Mod2 Mod3 Mod4 Super Mod5`) e cada acorde era recusado
individualmente enquanto a leader seguia funcionando; e `WheelScrollDown`, o nome da roda no
compositor anterior, que aqui é `button5`. Nenhum dos dois é visível em fonte nenhuma: são strings que
só o parser dele julga. E o modo de falhar é o mais quieto que existe — **uma linha ruim não derruba o
arquivo**, ela é recusada sozinha, então o sintoma é uma família inteira de atalhos sem fazer nada.

Para isso o config do aninhado teve de virar **derivação**. Ele era montado por `sed` dentro do
script, em tempo de execução — nada tinha como alcançá-lo. É o mesmo movimento que o config de
produção já tinha feito, e pelo mesmo motivo.

E a comparação de teclas passou a ser **canônica**: modificadores ordenados, tudo minúsculo. Os binds
de emergência entram como reservados, porque eles saem sem o prefixo da leader — um acorde que caia
na mesma tecla não "perde para o nosso", ele quebra o arquivo.

---

### A mensagem de erro tem de ser legível — e não era

O compositor avisou que o config estava quebrado, e o aviso saiu em **tofu**: uma caixinha por caractere. É
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

**Uma entrada é um reconhecimento, não um construtor.** Nem todas nascem aqui — uma vinda de um
aplicativo estrangeiro chega pelo barramento com o texto dele. Então a entrada diz como
RECONHECÊ-LA e o que fazer com ela, e a mesma máquina serve uma que levantamos e uma que chega de
fora. As nossas carregam uma `tag` igual ao próprio id, que é por que quase todo `when` é de uma
linha — a da captura inclusive, agora que ela é nossa: havia um check lendo a FONTE de outro
programa para que o par (app, resumo) não se mexesse em silêncio, e ele morreu junto com a
dependência que o justificava.

**Algumas das nossas deixaram de chegar à faixa.** Um aparelho Bluetooth conectando ou saindo e um
programa instalado ou web app criado viraram EVENTOS, ditos no minibuffer, junto com os que já
nasceram assim: câmera, microfone, a gravação e a transmissão do estúdio, rede, modo avião, VPN,
tailnet, Tor e carregador (ver "Eventos no minibuffer"). A entrada de cada um continua no catálogo,
porque é ela que decide a notificação de quando a linha estava ocupada; as dos aparelhos guardam as
tags, e com elas os filtros que as escondem. O que pode querer resposta continua notificação: os
alarmes de bateria, o arquivo que a câmera gravou, os outros fins da central de programas.

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

**Duas portas, uma `notifications::View`.** `LEADER n` abre no rodapé, `LEADER Alt n` numa janela
(`n n` e `n w` na árvore completa); duas colunas
quando cabe, uma quando não. O corte sai de um token, e vale para as duas portas — que é o que faz
"uma implementação, duas portas" ser verdade também do layout.

**As duas portas fazem as mesmas coisas.** Cada nível, cada tecla e cada clique; o nome e o texto de
um filtro, digitados na linha da própria porta — o minibuffer no rodapé, o cabeçalho na janela
(`Ui::ask`, com colar) —, que parte do que o rascunho já tem e devolve a resposta à mesma
`View::typed` (a vista guarda o que perguntou; resposta vazia mantém o que havia); e toda forma de
ação. O que um botão FAZ é decidido uma vez, em `notifications::deed_of`, para as duas portas: copiar
um caminho, copiar uma imagem e abrir um programa num escopo próprio (`foreign::start_in_scope`) a
janela faz sozinha, porque são uma chamada de qualquer processo; o botão de um app de fora
(`ActionInvoked`, dito por quem tem o nome no barramento) e um comando do registro são do rodapé, e a
janela os entrega pelo soquete de controle (`notif-act`, com o nome inteiro do registro), dizendo no
cabeçalho quando não há rodapé para receber.

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

**O que a janela marcou como lida é lido À MÃO também no rodapé.** Copiar a bandeira não bastava: o
`r` do rodapé tira o cartão da faixa e manda `NotificationClosed` ao remetente, e o da janela só
reescrevia o arquivo — uma palavra, dois contratos, conforme a porta. Antes de reconciliar, o rodapé
pergunta quais registros ele tem como não lidos e o arquivo diz lidos (`Queue::read_there`), e faz com
eles o que o próprio `r` faz. Só os que mudaram, e uma vez: a escrita do próprio rodapé não nomeia
nada.

##### Um registro se chama `(id, quando chegou)`

A `id` do protocolo não é nome de registro, e nada nunca prometeu que fosse. O contador do
barramento nasce em 1 a cada partida do rodapé; o das notificações que nós mesmos levantamos nasce
em `u32::MAX` e desce; o histórico dura quinze dias em disco. Uma restart basta para a mesma `id`
nomear vários registros, e dentro de uma sessão o `replaces_id` faz o mesmo por desenho — uma barra
de progresso reusa a id e cada chegada é lembrada.

Marcar lida e esquecer varriam a história com `id ==` e agiam em TODOS os que casassem. Como a
lista é desenhada do mais novo para o mais velho, os extras estavam sempre ABAIXO da linha em que a
pessoa estava — que é exatamente como o dono descreveu o defeito. O nome é o PAR, e ele é de graça:
as duas metades já eram persistidas e as duas portas já liam as duas do mesmo arquivo.

##### Quatro níveis, e nenhuma porta sabe que existem

O molde é o do Bluetooth: um campo diz onde se está, um `match` em `View::rows` desenha, `Esc` volta
um passo. A lista, o registro ABERTO, a ficha de um tipo e a ficha de um filtro. As duas portas
pedem `rows()` e pintam `Row.fields`; nenhuma tem um `if` sobre nível nenhum.

O registro aberto existe porque uma linha é uma linha: uma notificação que não cabia não tinha o que
apertar. Ali o corpo é QUEBRADO em vez de cortado, os botões são desenhados em vez de só terem
letra, e o que o gerenciador sabe — de onde veio, quando, quanto ficou, como apareceu, qual filtro
decidiu — é uma coluna de fatos ao lado. **Qual filtro decidiu saiu do título**: estava colado na
frase, cru e sem tradução, e lia-se `bluetooth: Bluetooth Keyboard connected by
small-things-are-quick`, o que diz que algo chamado `small-things-are-quick` conectou o teclado. É
fato do registro, e foi para onde os fatos estão.

##### Uma ação não tem letra própria

Ela tinha, e o dono leu a tela: `[b] bluetooth` numa desconexão. Três coisas erradas numa linha.

A letra só funcionava no GERENCIADOR — a faixa não pode tomar teclado, porque o rodapé em repouso
não o captura, ou nenhum buffer receberia tecla —, então era uma tecla desenhada numa tela e
apertada noutra. E o gerenciador já ensina uma dúzia de teclas em quatro níveis e vai ensinar mais:
uma letra que a notificação possui colide com alguma delas mais cedo ou mais tarde, e colide em
SILÊNCIO, porque a pessoa aperta e outra coisa acontece.

O que alcança uma ação agora é o cursor do registro aberto, e é um cursor só: ele rola o corpo — a
razão de o nível existir — e pousa nas ações, que são linhas como as outras. `RET` roda a que
estiver sob ele.

E a linha da lista só MARCA que há ações. Uma notificação pode ter várias, com rótulos que são
frases, e uma linha é uma linha: duas delas já espremiam a frase para fora. A célula é reservada
para a coluna inteira, senão o fim de cada frase muda de lugar conforme a linha.

##### E a ação tinha de FUNCIONAR

Ela não funcionava, e as duas portas erravam de jeitos diferentes.

No rodapé, `bluetooth-open` pergunta ao `region_is_free` — e a vista que segurava o rodapé era o
GERENCIADOR, que é de onde o botão foi apertado. Recusava, dizia numa linha que ninguém estava
lendo, e o único efeito visível era a linha ficar quieta. Uma ação de COMANDO fecha o gerenciador
antes de rodar; copiar um caminho e abrir um programa não fecham, porque deixam a tela onde está.

Na janela, a resposta era "a ação roda pelo rodapé". É verdade e é inútil, e a regra do dono diz por
quê: uma porta não manda a pessoa à outra. A janela faz o que qualquer processo faz e entrega ao
rodapé, pelo socket de controle, só o que só ele pode fazer.

**Agir é ter lido, e lido À MÃO.** Nas duas portas, rodar uma ação marca o registro como `r` marca:
sai da faixa e o remetente recebe `NotificationClosed`, que é o que um daemon de notificação diz
depois de uma ação. Antes a ação só virava a bandeira, e o cartão ficava na faixa depois do clique.

##### O filtro da pessoa, e o filtro de todo mundo

A linguagem de regras está completa desde o dia em que foi gerada — sete campos, cinco comparações,
os filtros da pessoa tentados antes dos da distro — e o único caminho até ela era editar o JSON de
preferências à mão, na grafia de variante do serde (`{"Contains": "backup"}`). Isso não é interface.
Agora se escreve na tela: nome, condições e veredito, com as setas girando o que se gira.

E um filtro pode deixar de ser só seu. `p` abre uma pergunta que nasce no **não** (a regra do
formatar, pelo mesmo motivo), mostra o filtro inteiro e diz numa frase que o repositório é PÚBLICO e
que não há volta prática. Respondido sim, o shell escreve uma linha em `assets/shared/notifications.json`
— na forma que o `assets/notifications.nix` usa, porque alguém vai abrir esse arquivo — e o build
junta a regra à tabela da distro. **Quem publica de verdade é o commit do dono.**

O caminho do repositório vem da opção `ronoctua.repo`, nunca de procurar: um programa que sai
caçando um checkout do git acha o de outra pessoa, e este escreve no que achar. E **nada pergunta à
rede**. "Já chegou?" se responde contra a tabela que ESTE binário compilou: a única estrada por onde
conteúdo da distro alcança uma máquina é um build pelo flake, então estar na tabela é ter chegado.
É a maneira eficiente porque não é uma conferência.

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

### Eventos no minibuffer — o que a máquina fez, numa linha

O dono escolheu o que é EVENTO: um fato sobre a máquina que vale uma linha e nada mais.

| Grupo | Eventos | De onde vem a leitura |
|---|---|---|
| Privacidade | uma câmera aberta (dizendo por quais programas) e livre; um microfone escutando e livre; o estúdio começando e parando de gravar e de transmitir | a varredura do `/proc` de quem segura a câmera; o PipeWire; o estado do estúdio |
| Bluetooth e redes | um aparelho conectado e desconectado; uma rede conectada e caída; modo avião; VPN; tailnet; Tor | o BlueZ; o NetworkManager; o `ronoctua-tunnel` |
| Energia | carregador conectado e removido | o UPower |
| Programa instalado | um programa instalado e um web app criado, com a ação de abri-lo | o fim do trabalho na fila |

**No minibuffer, e não na faixa.** A faixa é para o que chega e pode querer resposta; um fato que
passa é o que a linha do minibuffer já faz com as respostas do próprio shell (o eco). Então o evento
é dito ali, no resumo da entrada dele no catálogo (`notif-<tag>-summary`), pelo tempo de um eco. E é
dito com a modeline à mostra ou não: com as barras ocultas, um evento é o que traz o minibuffer à
tela por um instante, sob o véu (ver "Os modos ocultos").

**A primeira leitura é só referência** (`events::Edge`). Toda fonte é comparada com a leitura
anterior, e a primeira não tem anterior: o rodapé subindo com um fone conectado não é o fone
conectando. Sem isso, cada partida do rodapé dizia "conectado" uma vez por aparelho, que é o que o
rádio fazia.

**O que é nosso não é notícia.** O medidor da vista de som e a captura do estúdio abrem o microfone
a pedido de alguém, e os dois se nomeiam `ronoctua-audio` no PipeWire (`pipewire::OWN_STREAM`); o
evento pergunta `Snapshot::someone_else_listens`. Sem nome, os dois respondiam `pw-cat`, que é o que
o `pw-cat` de qualquer outro programa também responde — separar os nossos era perder um estranho ou
anunciar o medidor a cada movimento do cursor. A marca da modeline continua perguntando
`is_listening`, porque um microfone que o medidor segura ESTÁ aberto. Pela mesma razão, o estúdio não
conta como alguém abrindo a câmera (`events::OUR_CAMERA_HOLDERS`): a prévia foi pedida, e a gravação
é um evento próprio. Quem segura o dispositivo nessa hora é o `ffmpeg` filho do estúdio
(`ronoctua_camera::Capture`), com o mesmo nome do `ffmpeg` de qualquer um; então o que é nosso se
decide pela LINHAGEM: o `ronoctua-memory`, única fronteira com o `/proc`, lê o pai de cada um no
`stat` (`camera::Holder::parents`, até `LINEAGE` níveis), e é nosso quem é ou roda sob um processo da
lista (`Holder::under`). A marca da modeline continua contando todos.

**Uma leitura sem daemon não é leitura.** Um NetworkManager reiniciando não responde por um segundo,
e isso não é toda rede caindo. O mesmo vale para o rádio (o `Manager::read` do BlueZ devolve a cena
vazia em qualquer erro de D-Bus, e comparada ela era todo aparelho saindo e voltando) e para os
túneis: a tailnet só conta quando o tailscaled respondeu, e o Tor quando o socket de controle
respondeu ou quando o systemd diz que a unidade está parada ou não existe — socket calado com a
unidade rodando não é leitura. A referência fica a da última resposta. O preço: um tailscaled que
morre de vez não diz "fora da tailnet". O modo avião é dito uma vez, e não mais uma por rede que ele derrubou;
mudar de rede é perder uma e achar a outra; um link que É a rede (o ponto de acesso) não é saída. Um
carregador que aparece ou some — uma doca que o UPower só agora notou — não é tomada. E a câmera é
lida pela varredura do `/proc` a cada 2 s, então o eco dela pode chegar até 2 s depois.

**Ocupada, a linha passa o evento à faixa, na hora** (`events::route`). A linha é de alguém enquanto
uma vista está aberta (a leader inclusive), uma pergunta está sendo digitada, a gaveta da linha está
aberta, ou a tela está sob uma janela em tela cheia com o rodapé reduzido à borda. O evento que chega
então não espera a linha: uma câmera que abriu enquanto alguém digitava no localizador é notícia
agora, não daqui a dois minutos. Ele vai para a faixa pela porta de toda notificação
(`Footer::arrived`), com a tag dele, e o corpo ABRE com a frase que diz por que está ali
(`notif-minibuffer-was-busy`): o mesmo fato chega em dois lugares conforme o que a pessoa estava
fazendo, e uma notificação que não dissesse isso seria lida como a linha tendo quebrado. Abre, e as
entradas são `full`, porque a faixa desenha UMA linha de corpo, cortada no espaço: uma entrada
`brief` nunca mostrava a frase, e depois do corpo do evento ela era a parte que o corte levava.

**A fila** (`events::Line`). Eventos vêm em rajada — voltar da suspensão traz a rede, o túnel e o
fone no mesmo segundo —, então a linha guarda os que esperam, em ordem, e mostra cada um pelo tempo
de um eco. O eco de uma pessoa (o shell respondendo o que acabou de ser pedido) toma a linha na hora,
e o evento interrompido volta para o INÍCIO da fila, para ser dito inteiro depois. Com
`EVENT_ECHOES` (quatro) esperando, o seguinte vai para a faixa: uma fila sem fim é uma linha que diz,
um minuto atrasada, o que já não é verdade. Pela mesma razão, **um evento que diz o contrário de um
que está na linha ou na fila o substitui** (`events::contradicts`): câmera, microfone, gravação,
transmissão, avião, carregador, tailnet e Tor pelo par; rede, VPN e aparelho Bluetooth pelo par e
pelo mesmo nome. Na linha, ele entra no lugar com o tempo inteiro; na fila, no lugar do outro. E
quando a linha passa a ser de alguém, o que ainda esperava vai para a faixa, na ordem em que seria
dito; o que já estava NA linha foi dito e sai sem ir a lugar nenhum, a menos que tenha ficado menos
de `READ_IN` (1 s) — antes disso ninguém o leu. Mandá-lo à faixa era o mesmo fato duas vezes, com a
frase de que a linha estava ocupada, a cada leader apertada logo depois de um evento. Isso é perguntado uma vez por quadro, no
topo do `draw_and_paint`, porque toda mudança do que segura a linha acaba num quadro, e uma lista dos
jeitos de tomar a linha é a lista que esquece um. Com só a gaveta aberta, o evento que está na linha
fica: a gaveta o está mostrando.

**O ponteiro segura o evento, por um tempo.** Com a mão sobre a linha ele não sai, e fica mais um
tempo de eco depois que ela sai: alcançar a ação leva mais que os três segundos que ele teria. Mas no
máximo `HELD_AT_MOST` ecos (três) desde que subiu: a linha é a última da tela, onde um ponteiro
costuma ficar PARADO, e com as barras ocultas ela é a borda que as revela — segurar sem fim deixava
"uma câmera está aberta" na tela muito depois de ela fechar, com a fila parada atrás.

**A gaveta fecha com a linha.** Aberta para ler um evento ou um eco inteiro, ela ficava aberta depois
que ele saía — em repouso nada a fecha senão o chevron, e o rodapé em repouso não tem teclado para o
`Esc` —, e uma gaveta aberta conta a linha como ocupada: todo evento seguinte ia para a faixa com a
linha vazia. Quando a linha esvazia em repouso, a gaveta fecha e a geometria é pedida de novo
(`close_the_drawer_of_an_empty_line`).

**O Bluetooth guarda as tags de sempre** (`bluetooth-connected`, `bluetooth-disconnected`), e os dois
filtros que calam um aparelho indo e vindo casam a TAG: calam também a notificação de quando a linha
estava ocupada. As entradas novas do catálogo são `full` (a segunda linha da faixa é a do porquê) e
`quick` — o passo do minibuffer — e nascem lidas, menos câmera e microfone abertos, que o sino deve
guardar.

**A ação vai no fim da linha, com a cara da ação da faixa.** Um evento cuja entrada oferece ações as
desenha depois da frase (`chrome::Piece::action`, `chrome::said_with_actions`), no tom dos botões da
faixa (`notification.acts`, e `notification.acts-lit` sob o ponteiro): o dono pediu que uma ação
parecesse UMA coisa onde quer que esteja. Só entram as ações que são comando do registro
(`events::actions_of`); um comando que o registro não conhece fica de fora, em vez de virar um botão
que não faz nada. As ações ficam coladas no fim do espaço, e quem cede é a frase
(`chrome::room_for_actions`): em repouso não há linha embaixo para dizer o que faz a ação sob o
ponteiro, então a frase `cmd-<id>` dela toma o lugar das palavras (`chrome::explained_in_place`), e
com as ações logo depois das palavras uma frase de outro tamanho as tiraria de baixo do ponteiro — o
laço que o `Footer::explain_rows` descreve. O espaço é o da tela MAIS ESTREITA
(`room_for_the_actions`): com o fim de cada tela, duas telas de larguras diferentes desenhavam a ação
em células diferentes, e a tabela do ponteiro (`hint_placed`) é uma só. Clicar numa ação encerra o
evento. Hoje só o programa instalado oferece uma (`software-launch-latest`), e o evento leva o id do
`.desktop` daquele trabalho (`Event::opens`): o "Abrir" da linha abre o programa que ela nomeia
(`events::launch_of`), e não o último que terminou — a fila e o ponteiro seguram uma instalação
antiga na linha enquanto outras terminam. O botão da faixa, quando o evento cai nela, continua
abrindo o último, como sempre abriu.

### Cores: o gerador e os temas — `LEADER s t`

**Nenhuma cor é escrita à mão.** `assets/tokens/colors.nix` é uma tabela de RECEITAS — cada token
nomeia um degrau de uma família (`text.primary = "neutral.text-strong"`,
`surface.selection = "accent.element"`), e uma receita pode levar `alpha` (a faixa do transporte) ou
escalar o croma do degrau (`surface.mesh`). Os valores saem de `nix/scripts/gen-theme.py`, que roda
UMA vez em `nix/themes.nix` para todos os temas de `assets/themes/themes.nix` — uma LISTA, como a
dos idiomas, com a ordem (`order`) e o padrão (`default`) em campos separados, e nenhum outro sítio
nomeia um tema. Um tema é um arquivo de SEMENTES (`assets/themes/<id>.nix`): o modo (`dark` ou
`light`), a luminosidade do fundo e do rodapé, o matiz e o tinte dos cinzas, a mesa (`backdrop`:
matiz, croma e passo), o matiz e o croma do texto, um multiplicador global, e por família um matiz,
um croma relativo, um `shift` de contraste, uma `torsion` de matiz nos degraus de pintura e um
`paint-chroma`. Nada de hex: o check `colours-are-computed` recusa um literal nas receitas e nas
sementes, e recusa qualquer `import` da referência congelada fora de `nix/themes.nix`.

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
ordinais, e as quatro cores-filme (as do `testsrc2`, que uma foto procura na tela). Deficiência de visão de cor é Warn e nunca Fail —
verde e amarelo do próprio owl estão a 0,009 sob deuteranopia, e o glifo carrega o significado. A
regressão compara cada chave gerada com `assets/themes/owl.reference.nix`, os 52 literais congelados
do dia das sementes: acima de ΔE_ok 0,03 é falha de build, salvo chave listada em `deviations` com
teto e motivo próprios (`text.subtle.faint`: o `text-shift` do registro foi medido no `strong` e vale
para os três degraus). `report.md` de `nix build .#themes` traz cada margem; `theme-references-hold`
e `themes-negative` (uma semente com o acento a 165° TEM de falhar) são os checks.

**Uma cor é um PAPEL, e o tema é do processo.** `ronoctua_tokens::Color` é um `u16` — o índice de um
papel — e `to_rgba8()` lê `themes::TABLE[tema][página][papel]`, uma tabela gerada com TODOS os
temas, cada um nas suas quatro páginas (ver "As faixas seguem o fundo", abaixo). Os 280 sítios que
nomeiam `color::TEXT_DIM` não mudaram: nomeiam o papel, e o papel resolve ao desenhar. Igualdade e
hash são por papel (`lift_onto_a_band` pergunta "este campo é o dim?", e é isso que sempre quis
perguntar). A paleta é um `AtomicUsize` (`tema × 4 + página`: `theme::set` muda o tema e guarda a
página, `theme::set_ground` muda a página e guarda o tema, `theme::palette()` lê os dois numa
palavra só, para um quadro nunca ler um tema de um lugar e uma página de outro), como o idioma é uma
propriedade do processo: enfiar um valor por 280 assinaturas carregaria algo que só o desenho lê.
Buffers wasip2 não souberam de nada — cruzam NOMES (inv. 11), e o host resolve; o enum do WIT
cresceu para 147 nomes e recompilaram uma vez. `to_rgba8` deixou de ser `const`; os três
`const fn colour()` das TUIs viraram `fn`. Cada processo escolhe no arranque
(`theme::resolve_at_start`, em `main.rs`, antes de qualquer porta):
`$XDG_CONFIG_HOME/ronoctua/theme.json` > `RONOCTUA_THEME` (a opção `ronoctua.theme`) > o padrão
declarado. Fonte que nomeia tema inexistente ou não parseia cai para a seguinte e diz no log; as
quatro respostas SOBRE o tema vêm só do arquivo. `render-png --theme` é determinístico (o primeiro,
nunca arquivo nem ambiente); o instalador e o resgate leem só o ambiente.

**O arquivo é o broadcast.** O socket de controle só entra no rodapé, e as portas — janela de
arquivos, notificações, atualização, cada host de buffer — são processos. Então o rodapé não avisa
ninguém: grava `theme.json` atomicamente (`ronoctua-xdg`, o quarto chamador que o comentário do
brain pedia, e que agora serve os três anteriores) e toda porta segura um `Notice::on_file` sobre
ele — o mesmo inotify do gerenciador, filtrado por UM nome, porque o `files-sidebar.json` mora ao
lado. Ao tocar, `theme::reload()` — verdadeiro quando muda o tema OU qualquer resposta sobre ele,
porque o fundo de uma porta pode ser a mesa — e repintura. O que uma CRIANÇA pintou com um valor — a
forma de onda do ffmpeg, o `currentColor` dos SVGs, o globo, a arte do manual — se refaz porque a
paleta (`theme::palette()`, tema e página) entrou na CHAVE da prévia e do SVG rasterizado: chave
nova, cache do GPU sem ela, upload de novo; sem isso a tinta velha ficava na textura — e, com as
páginas, uma figura pintada para o chrome apareceria sobre a mesa. Custo: o diretório
`~/.config/ronoctua` passa a existir no primeiro arranque de qualquer porta (o inotify quer um
diretório); e uma troca com um filme na prévia respawna o ffmpeg, com uma peça 3D relê o arquivo uma
vez — chave nova é pedido novo, e o painel fica vazio até a resposta, como sempre ficou.

**`LEADER s t` é a lista da rede com uma diferença: mover um cursor PRÉ-VISUALIZA** (a região,
`theme_chooser.rs`). A escolha sob os cursores vira a do processo na hora — o rodapé é a amostra,
porque amostra "na cor do tema dele" não existe: uma `Color` resolve pela paleta corrente e um valor
preso a outro tema é justamente o que o crate recusa guardar. **E a prévia grava o ARQUIVO**
(`theme::show_everywhere`), com os dois fragmentos: o arquivo é o canal que toda outra porta já
vigia, e uma prévia que ficava fora dele era a prévia de uma faixa da tela — o dono viu o minibuffer
mudar e as janelas, a mesa e os terminais não. Um segundo canal foi recusado pelo motivo de sempre
(uma segunda coisa a manter em passo, e toda leitora teria de aprender a precedência entre os dois).
O preço, escrito: o arquivo que quer dizer "escolhido" é gravado enquanto ninguém escolheu, e um
rodapé que morre no meio da prévia deixa a prévia gravada — uma preferência, reversível pelas mesmas
teclas. `RET` (`theme-apply`) aplica as cinco respostas sob os cinco cursores. `Esc` devolve tudo ao
que era quando o seletor abriu — ou APAGA arquivo e fragmentos, se não havia arquivo: máquina que
nunca escolheu não ganha um por ter olhado a lista (`chosen_before`, perguntado uma vez, na
abertura, porque depois da primeira prévia o arquivo sempre existe), e `sync_with_file` mantém "sem
arquivo, sem fragmento" no arranque e quando o arquivo some. Um `theme.json` escrito por fora
durante a prévia vence a prévia; a gravação da própria porta ela reconhece (`View::previewed`),
senão a primeira seta movia o "gravado" e o `Esc` não tinha para onde voltar. Temas: SÓ o id
(identificador, como a lista do `M-x` mostra ids de comando; o nome traduzido era uma segunda grafia
da mesma coisa e saiu a pedido do dono). Sem `theme-next`: o `check-keymap` recusa comando sem
tecla.

**Cinco colunas, cada uma uma lista isolada: Tema | Variações | Rodapé | Janelas | Bordas.** O tema
e as quatro respostas SOBRE ele (ver "A mesa, o rodapé, as janelas e as bordas", abaixo), lado a
lado, com uma régua entre duas (`chrome::divider`, com o ar da tabela do gerenciador: `files.gutter`
antes do texto e `files.edgeRight` depois, para nenhum rótulo encostar na régua). O dono pediu cada
coluna ISOLADA, e é o que cada uma tem: título próprio (que não recebe cursor), cursor próprio —
guardado por coluna, então cruzar e voltar o acha onde estava — e HOVER próprio: a faixa do ponteiro
cobre só as células da coluna sob ele, na linha em que ele está, de régua a régua
(`Layout::band_px`), e nunca a linha inteira, que é o que ela fazia quando o seletor eram duas
listas desenhadas como um par tecla/texto. `<left>`/`<right>` cruzam, `<up>`/`<down>` andam na
coluna que tem o teclado, e cada passo pré-visualiza a escolha inteira. O clique acerta pela mesma
`Layout::column_at` que a faixa usa: o primeiro escolhe na coluna sob o ponteiro, o segundo é o
`RET` (inv. 19). O cursor é cor: acento na coluna com o teclado, `text.primary` no cursor das outras
(o `RET` aplica as cinco, então as cinco têm de se ler), e a marca de visto fica na resposta que o
arquivo diz. **A largura de cada coluna é a do rótulo mais largo na tradução mais longa**, com os
rótulos da mesa nos DOIS modos: medida no idioma da tela, as réguas andariam ao trocar de idioma, e
a coluna da mesa andaria quando a prévia cruza de um tema escuro para um claro ("Preto" e "Branco"
não têm a mesma largura em todo idioma). Num rodapé estreito as mais largas cedem primeiro, até um
piso, e o rótulo é elidido no fim (`Layout::of`).

**A coluna da mesa segue o modo do tema sob o cursor.** Um tema escuro oferece Preto e Cor do tema;
um claro, Branco, Quase branco e Cor do tema (`Backdrop::offered`, `Backdrop::text_key`). A mesa é
guardada como RESPOSTA (`View::desk`) e não como número de linha — as listas têm tamanhos
diferentes, e um número mudaria de sentido sob o cursor dos temas —, então um quase-branco escolhido
num tema claro sobrevive ao cursor passar por um escuro (mostrado na linha da mesa absoluta, que é a
que ele veste ali) e volta com o próximo claro.

**Fora do shell, três consumidores seguem, e cada um até onde pode.** O compositor: a borda e a
mesa estáticas vêm do tema padrão (substituídas no build do config, sem IFD) e são as da RESPOSTA
padrão — a mesa `surface.backdrop` e as bordas `standard`, iguais às do `scroll-backdrop-standard.conf`
(o `themes-have-fragments` cobra): numa máquina que nunca escolheu, o que a tela mostra e o que o
rodapé acha que ela mostra são uma coisa só, e as linhas do `Push` oculto pousam na mesma cor. No FIM
do config há
`include "${XDG_CONFIG_HOME:-$HOME/.config}/ronoctua/scroll-theme.conf"` — o gerador escreve por
tema SEIS fragmentos, um por par de respostas (`scroll-<mesa>-<bordas>.conf`: mesa `backdrop`,
`near` ou `absolute`; bordas `standard` ou `discreet`), cada um com as bordas em `client.focused` e
companhia e a mesa num `output * bg`. Os de `near` de um tema escuro existem e nunca são copiados:
ali um "quase branco" é vestido como a mesa absoluta. **Aqui não há ordem a acertar**: ajuste de
saída é MESCLADO (`*`, depois o conector, depois o identificador; campo a campo, o último vence), e
`client.focused` simplesmente sobrescreve o valor anterior — então o fragmento ganha do que o config
já diz, sem nó duplicado e sem precedência para lembrar. O `themes-have-fragments` valida cada um
sob um config que JÁ pinta uma mesa, que é a forma dessa prova. O rodapé COPIA o da escolha ao
gravar (`theme::fragments`, pela mesa VESTIDA) — copiar É escolher — **e então PEDE a releitura**,
porque este compositor não vigia arquivo nenhum: escrever e parar aí deixaria a mesa na cor velha
pelo resto da sessão. O `background-color` estático é o `surface.base` do tema padrão, o que vale
antes de qualquer escolha. O Ghostty: `config-file` com a paleta gerada do tema padrão (ANSI 16,
fundo, cursor, seleção — `terminal.*`, mapeados das famílias) e depois
`config-file = ?~/.config/ronoctua/ghostty-theme.conf`, opcional, que o rodapé também copia — de um
dos QUATRO fragmentos do tema, um por página (`ghostty-<página>.conf`: `theme` com o fundo da janela
do tema, `backdrop`, `near` e `absolute` com a mesa por fundo e a paleta daquela página), pela
resposta `windows`; processo novo do Ghostty veste, processo aberto não — e toda janela É um
processo, por qualquer porta: a config diz `gtk-single-instance = false`, mas a entrada `.desktop`
do upstream passava `--gtk-single-instance=true` no `Exec` (flag vence arquivo) e oferecia ativação
por D-Bus, que é de instância única por construção; o módulo instala a entrada reescrita sem os dois
e sem o arquivo de serviço (`ghosttyPerWindow`, um `symlinkJoin`), e `ghostty-windows-are-processes`
mede o instalado. Pelo lançador, uma janela aberta depois da troca entrava no processo antigo e não
vestia nada. **E o Ghostty do nixpkgs 26.05 só lê `$XDG_CONFIG_HOME/ghostty/config`** — nunca
`/etc/xdg` —, então o módulo do terminal passou a pôr o arquivo lá pelo home-manager, que já cuida
dos dotfiles desse usuário: até isso, nenhuma linha dele chegava ao programa, decoração inclusive (a
revisão mediu com `+show-config`, e o check `ghostty-config-includes-theme` agora pergunta ao
binário a configuração EFETIVA). Os fragmentos têm UM renderizador (o gerador); o shell só copia, do
`RONOCTUA_THEMES`, e os refaz ao arrancar e quando o arquivo muda por fora. GTK e Qt vestem o MODO
(escuro ou claro, pelo fragmento `mode` do tema), não a cor (`nix/modules/foreign.nix`); a extensão
do navegador veste o tema padrão do build. O que NÃO segue ao vivo, e por quê: a extensão (não
enxerga nossos arquivos) e terminais já abertos (cada janela é um processo que leu o config). O
fundo do compositor SEGUE: o `layout` do fragmento é fundido no reload.

**O que está lá para contextos que ainda não existem.** Além dos 52 tokens de sempre, cada tema traz
`context.<família>.{solid,hover,ground,border,on,text}` para as oito famílias do rodapé e
`context.neutral.{surface,surface-raised,fg,fg-muted,border,border-strong}` (as tabelas de forma da
referência, sem o que o inv. 15 proíbe), `terminal.*`, `chart.one..eight` (matizes a arcos de ΔE
iguais a partir do acento, luminosidade subindo com o índice — é o que sobrevive à dicromacia) e
`level.zero..ten` (sequencial no matiz do acento, croma mudo nas pontas). Uma tela nova pede o papel
que existe; a família que ela quiser criar é uma linha na semente e passa pelos mesmos gates.

#### A mesa, o rodapé, as janelas e as bordas — as quatro respostas sobre o tema

Pedidas pelo dono em 2026-09-28, e nenhuma é um tema: são escolhas SOBRE o tema, uma coluna cada no
`LEADER s t`, gravadas ao lado dele: `theme.json` é `{ "theme", "backdrop", "footer", "windows",
"borders" }`. Só `theme` é obrigatório; resposta ausente é o padrão (`Choice::with_theme`: a cor do
tema, o tema, o tema, padrão). **Resposta desconhecida é RECUSADA e dita**, como um tema
desconhecido sempre foi — no log e na linha do rodapé, onde o arquivo é lido (`theme::Complaint`,
`echo-theme-refused`) —, e o campo fica no padrão: nunca vira um padrão calado, que é como "minha
escolha voltou sozinha" acontece sem nada dizer por quê.

- **A mesa** (`backdrop`), o que o compositor pinta onde não há janela. Três mesas por tema, cada
  uma um token, e o seletor oferece duas num tema escuro e três num claro (`Backdrop::offered`):
  - **`absolute`** (`surface.absolute`): o extremo de que o tema se afasta, sem croma — `#000000`
    num tema escuro, `#FFFFFF` num claro. Era `black`, um literal do gerador com o nome da opção, e
    o primeiro tema claro fez o nome mentir: o pixel desligado não é a ponta de uma escada que desce
    do papel. Uma palavra para as duas polaridades, e o modo diz qual; o rótulo é Preto ou Branco
    pelo modo do tema MOSTRADO (`Backdrop::text_key`). Um arquivo antigo que diz `"black"` lê-se
    como `absolute`, e a gravação seguinte já diz `"absolute"`.
  - **`theme`** (`surface.backdrop`): a cor do tema, o único degrau neutro com matiz próprio — a
    semente `backdrop` diz matiz, croma e passo —, um passo abaixo do chrome nos dois modos (numa
    tela, toda janela fica sobre algo mais escuro que ela). **É o padrão**, por decisão do dono:
    todo tema a resolve e todo portão a separa da absoluta, então mostrá-la numa máquina nova não
    custa nada, e o trabalho deixa de ser invisível por padrão.
  - **`near`** (`surface.near`): o quase-branco de um tema claro, a meio caminho entre o chrome e o
    papel (`NEAR_SHARE`), no matiz e no tinte dos fundos. Só um tema claro o oferece; `"near"` sob
    um tema escuro é VESTIDO como a mesa absoluta (`Backdrop::worn_in`) e continua `"near"` no
    arquivo, então voltar a um tema claro traz o quase-branco de volta. Pinta-se a mesa vestida
    (`Choice::worn_backdrop`); guarda-se a resposta.
- **O rodapé** (`footer`) **e as janelas** (`windows`): sobre o que ficam — `theme` (o chrome do
  tema) ou `desk` (a mesa escolhida, para lerem como parte dela e não como uma faixa atravessada
  nela). A do rodapé vale para modeline, minibuffer e região, em todo modo, e para a coluna da
  direita; nunca para a faixa de notificação nem para a linha do cérebro. A das janelas vale para
  toda porta aberta por `door::run`, os buffers que ela hospeda e o terminal.
- **As bordas** (`borders`): `standard` ou `discreet` (ver "As bordas discretas", abaixo).

As respostas são do PROCESSO como o tema é (`theme::wear`: um byte atômico ao lado do índice da
paleta), pelo mesmo motivo — todo pintor pergunta na hora de desenhar —, e prévia e gravação chegam
pelo mesmo caminho: o pintor não sabe distinguir uma da outra. O que ele pergunta é QUAL PÁGINA
(`theme::footer_page`, `windows_page`, `desk_page`), e não qual cor.

#### As faixas seguem o fundo — uma paleta por página

A primeira resposta a "o rodapé e as janelas sobre a mesa" pôs os tons do chrome sobre a mesa e
mediu todos lá também. Isso empurrou as mesas dos temas claros para uma cor: uma mesa cinza fica no
meio das faixas do chrome, e uma faixa sobre ela não se distinguia dela (medido no papyrus: o hover
a ΔE 0,004 da mesa, `text.primary` a Lc 74,9 contra o piso de 75). **O dono escolheu o outro
caminho: as faixas seguem o fundo.** Cada tema é resolvido em QUATRO PÁGINAS — `chrome` (a base),
`backdrop`, `near` e `absolute` (`PAGES`, `Theme.on_page`) —, e numa página de mesa a mesa toma o
lugar do chrome e todo texto, faixa e régua é resolvido de novo a partir dela. Nenhum tom da base é
desenhado sobre uma mesa; a mesa é medida como o `surface.chrome` da própria página.

- **Um degrau de pintura é o mesmo passo de LUZ** que a base dá a partir do chrome, com o croma da
  base. Não o mesmo ΔL: numa página preta os 0,018 da janela são o byte 0, e numa branca os 0,07 do
  hover são um passo de luz menor que a partir de um cinza médio; a razão de luminância (com o
  flare de 0,05 que a borda fraca também usa) é uma medida só nas duas pontas. E o croma absoluto,
  não o relativo: a mesma fração do máximo é uma faixa mais pálida numa página branca — a seleção
  chegou a 0,043 de uma régua na página branca do papyrus, contra o piso de 0,05.
- **Um texto é resolvido por Lc a partir da página, e nunca mais fraco que a tinta da base** ali:
  uma página com MAIS espaço que o chrome (o branco num tema claro, o preto num escuro) dá tintas
  mais pálidas, e a faixa e o cérebro, que não seguem a página, ficariam com uma tinta mais fraca
  que a medida — `text.primary` sobre a faixa, na página branca do papyrus, a 63,7 contra o piso de
  65. Fica a que lê mais. Numa página mais escura que o chrome de um tema claro o teto da escada
  desce até os dois degraus de cima continuarem dois (sem isso, eram o mesmo `#000000`).
- **O que não muda em página nenhuma** (`IDENTICAL_ON_EVERY_PAGE`, copiado da base): a faixa de
  notificação e a linha do cérebro (o dono: elas não mudam com essas opções), as duas listras do
  relance, as bordas (são do compositor, e os fragmentos levam a base) e as três mesas (são o que as
  páginas SÃO). E numa página de mesa `modeline.background` É a página: a modeline tem a cor da
  mesa, sem faixa de chrome atravessada.
- **Todo portão roda em toda página** — onze temas × quatro páginas, 44 paletas, `Fail 0` na linha 5
  do `report.md`. O que roda só na base diz por quê: `BASE_ONLY_PAIRS` (o chrome contra o fundo, o
  chrome e a janela contra as mesas, e o chrome contra a faixa da modeline — numa página de mesa o
  "chrome" É a mesa, e esses pares a mediriam contra si mesma), os degraus que partem do fundo, as
  mesas entre si (`NEAR_GATES`) e as bordas discretas (`DISCREET_DESKS`), esses dois porque só medem
  tokens iguais em toda página. O terminal é medido contra o fundo que o fragmento de cada página
  escreve (`TERMINAL_BACKGROUND`). O `every-ground-is-measured` dispensa as três mesas como papéis
  sem texto, com esse motivo: o texto sobre uma mesa é o da página dela, medido lá como
  `surface.chrome`.
- **O que sai**: `<id>.json` (a base) e `<id>.<página>.json`; o `report.json` por página; e o
  `gen-tokens.py` gera `themes::TABLE[tema][página][papel]`, `themes::PAGES`, `counts::PAGES` e o
  modo de cada tema (`Theme::is_light`, `theme::is_light_at`).

**Quem veste qual página.** Uma superfície sobre uma mesa não pinta outra cor: VESTE a página da
mesa (`tokens::theme::set_ground`) e pinta exatamente como pintaria no chrome — nenhum pintor
aprendeu o que é uma mesa. O rodapé veste, por tela, a resposta `footer` (`hidden::Shape::page`),
menos enquanto as linhas do modo oculto `Push` empurram as janelas: aí ele veste a página da mesa de
onde elas saíram. A coluna da direita veste a resposta `footer` (é vizinha do rodapé, chrome numa
faixa — inv. 14 —, e não janela). Toda porta veste `Content::page()`, que por padrão é a resposta
`windows`, perguntada a cada quadro e a cada troca do arquivo, e limpa com `surface.chrome`; os
buffers que ela hospeda vestem junto (cruzam nomes, inv. 11: nada muda do lado WASM). O terminal
recebe o `ghostty-<página>.conf` da resposta `windows`. A linha de vida não põe página: fica na
base. Faixa, cérebro, relance e bordas nunca mudam. E todo cache de figura pintada com valor de
token é chaveado por `theme::palette()`, tema e página numa palavra só.

**As exceções dos temas claros, escritas nos portões** (decisão do dono, 2026-09-28). Entre duas
mesas o piso é 0,10, o de "uma escolha que não faz nada". As três de um tema claro são FEITAS para
ser três papéis próximos — branco, quase-branco e o do tema —, e o piso de 0,10 empurrava a do tema
para uma cor (empurrou: um pêssego, depois um lilás). Então, num tema claro: a mesa do tema contra o
branco ≥ 0,07 (`LIGHT_DISTANCE_FLOORS`), contra o quase-branco ≥ 0,05, e o quase-branco contra o
branco ≥ 0,02, um JND (`NEAR_GATES`, que só rodam num tema claro: num escuro o quase-branco nunca é
oferecido). Medido: a mesa do tema a 0,094 do branco no papyrus e no flashbang, e a 0,068 e 0,073 do
quase-branco.

**As sementes que se moveram para caber em quatro páginas.** A mesa do papyrus ficou dessaturada — o
cinza quente da própria folha — e o flashbang foi refeito num prateado sóbrio — acento aço, estados
e matizes de conteúdo com cerca de metade do croma —, os dois a pedido do dono. A linha do cérebro
mudou de matiz em cinco temas (flashbang, frost, industrial, owl, papyrus): ela é igual em toda
página, e em alguma delas a mesa ou a faixa de seleção chegava perto dela. A torção do acento do
forest e do frost mudou para a seleção ficar a 0,05 de uma régua nas páginas mais escuras. Cada
semente diz o número e o que o prende.

#### As bordas discretas

`borders` é `standard` — `border.active` na janela em foco e `border.inactive` nas outras, como
sempre — ou `discreet`: a janela em foco num contorno quieto (`border.discreet`, o degrau do
acento com menos da metade da cor) e toda outra num contorno EXTREMAMENTE fraco, e nunca
transparente — o dono: a pessoa ainda precisa de uma referência para redimensionar com o mouse. O
`focused_inactive` é das OUTRAS: neste compositor cada coluna é um contêiner, e a janela que cada
coluna focou por último — quase toda janela da tela — veste essa classe; com a cor discreta nela,
foco e fora de foco pareciam iguais (o dono viu, 2026-09-28). O marcado continua em
`border.selected`: o que ele diz é "marcado", e a resposta não o cala. É uma cor, e não uma borda
mais fina, porque o compositor pinado não varia a largura da borda pelo foco. E é opaca: borda com
alfa seria elevação (inv. 15).

**A borda fraca é uma por mesa** (`border.faint-<mesa>`), porque é medida contra a mesa em que fica:
a mesa subida na escada da interface até a razão de luminância chegar a `FAINT_RATIO` (1,05) —
`#090909` sobre o preto, `#F9F9F9` sobre o branco, mais ou menos o menor degrau que a paleta já pede
ao olho (o chrome contra a janela é 1,035 no owl). Razão, e não ΔE, porque o ΔE_ok não tem trecho
linear perto do preto: o primeiro byte acima do preto já está a 0,067, e um passo que o olho mal
acha mede 0,14 ali e 0,018 perto do branco; a razão com flare é um número só nas duas pontas.
Portões, por mesa e na base: a fraca contra a mesa com ΔE ≥ 0,012 (visível, medido nos bytes, então
cair nos bytes da mesa é Fail) e razão ≤ 1,10 (fraca); e as três em ordem — a discreta pelo menos
0,03 mais longe da mesa que a fraca, e a padrão pelo menos 0,03 mais longe que a discreta
(`DISCREET_ORDER`).

**A linha de foco da coluna segue a resposta** (`theme::focus_line`): o acento com as bordas padrão,
`border.discreet` com as discretas — a coluna diz "em foco" do jeito que o contorno de uma janela
diz. Ela é desenhada na página do rodapé, então `border.discreet` contra `surface.chrome` tem piso
de 0,05 em toda página.

Os fragmentos: `write_fragments` escreve `scroll-<mesa>-<bordas>.conf` (seis), com as cores de
cada classe de cliente e o `output * bg` da mesa, e `ghostty-<página>.conf` (quatro);
`theme::fragments` escolhe quais copiar a partir da escolha, pela mesa vestida.

---

## 4. Teclado, keymap e a leader

### Um keymap, e um bind só no compositor

Fechar janela, trocar de foco, mudar de espaço, capturar tela — tudo isso era bind do compositor.
Agora é comando do shell, e chega a ele como um FEITO pelo IPC. O compositor tem **um** bind: o que
acorda o daemon.

**Um keymap, e não dois.** Com dois havia duas notações, dois lugares onde uma tecla podia colidir, e
a lista de atalhos conhecia metade do sistema — quem apertasse a leader via uma lista que não era a
verdade.

O risco novo é de outra natureza: o comando viaja como **texto**. Um verbo errado por uma letra
compila, roda, e volta uma recusa que é uma linha de log e nada na tela. É a mesma classe do fallback
silencioso, e o `match` do Rust não protege: ele confere o id do comando, não o que vai no fio.

**Três coisas respondem a isso, e duas são de BUILD.** A tabela é declarada uma vez
(`assets/compositor-deeds.nix`); o gerador **recusa um verbo que o `sway/commands.c` da fonte pinada
não tem**, então um verbo removido é falha de build com o id na mensagem; e `wire::spoken` valida o
argumento, porque os separadores de comando aqui são `;` e `,` e um argumento livre seria injeção. A
terceira é o `compositor-contract`, que percorre a tabela inteira contra um compositor de verdade —
só ele responde se o verbo existente também ACEITA o que mandamos.

**E "aceitou" não é "fez".** Medido: `fit_size v` parseia, responde `"success": true` e não muda
nada; o que faz é `fit_size v all equal`. Um comando pode ser silenciosamente inerte, e é por isso
que nada aqui trata a resposta do IPC como prova.

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
atalho do host (`Super+Espaço`, por exemplo) e voltar fazia o compositor aninhado perder a soltura da tecla.
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

Por isso o Lock é desfeito antes de a tecla virar `KeyPress` (`door::keys::without_caps_lock`, a
mesma função no rodapé e em toda janela), nos dois sentidos — com Shift junto o xkb já devolveu a
minúscula, e desfazer só num sentido apagaria o `w H`. O Shift continua decidindo a caixa, que é o
desenho.

E quando a caixa **é** o motivo de nada casar, a mensagem passa a dizer isso: `leader::other_case`
procura a mesma sequência com a caixa trocada e, se for ela que existe, a área de eco mostra
`… não está ligado — … está`. Isso **não** é fallback: nada é executado pela outra caixa, justamente
porque `w H` e `w h` fazem coisas diferentes. É só a diferença entre a pessoa descobrir sozinha e ter
de mandar um screenshot.

#### A leader é a tecla Super sozinha, e isso custou um daemon

Em produção, **tocar** Super abre a sequência; **segurar** Super é o modificador dos acordes. A mesma
tecla física faz as duas coisas, e nenhuma delas é possível só com o compositor: ele recusa bind em
modificador sozinho (`invalid key: Mod`) e não tem disparo na soltura — as propriedades de bind são
`allow-when-locked`, `allow-inhibiting`, `cooldown-ms`, `repeat` e `hotkey-overlay-title`.

Isso não é limitação a contornar. Um modificador **é** um modificador; distinguir toque de pressão
acontece abaixo do compositor, no fluxo de eventos do kernel. Quem faz isso é o **keyd**
(`nix/modules/keyboard.nix`): `leftmeta = overload(meta, f13)` — segurada, ela é Meta e os acordes
chegam ao compositor como `Mod4+…`; solta sem outra tecla, emite `F13`, que ele liga à leader.

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

**E por isso o aninhado não consegue ser inerte ao segurar.** O compositor dispara bind na *descida* da
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

**Segurar a leader é inerte.** O que abre a sequência é um bind do compositor — o `F13` que o keyd
emite no toque, o `Ctrl+Alt+Espaço` no aninhado —, nunca o modificador da leader sozinho. Segurá-lo
não abre a lista e não tira o foco da janela, e há uma guarda no build que recusa um bind no
modificador sozinho: seria o único jeito de esse comportamento voltar.

`LEADER w h` é **sequência**: a leader é apertada e solta, o compositor avisa o daemon, e o rodapé
passa a receber teclado. `LEADER+←` é **acorde**: a leader fica segurada, e o rodapé nunca vê essa
tecla — quando ele ganha o teclado, o acorde já terminou. Quem resolve acorde é o compositor, sempre.

**O que chega logo depois da leader ainda é da sequência, mesmo sem ser tecla da árvore.** Um toque
de `Alt` ou de `Ctrl` — apertado e solto sozinho — troca o conjunto em que ela anda (ver "Três
conjuntos da leader"), e a própria leader chegando de novo, com nada digitado, esconde ou devolve as
barras no conjunto padrão e no das janelas (ver "Os modos ocultos"); com algo digitado, ou no
conjunto completo, ela recomeça na raiz, como sempre fez. Nenhum dos dois é bind novo no compositor:
o toque é lido pelo rodapé, que já tem o teclado, e a leader de novo é o mesmo `ronoctua-shell
leader` chegando pelo socket.

Isso poderia trazer de volta as duas tabelas. Não traz, porque **o bind do compositor é gerado**: a
tabela de acordes vive em `actions.rs`, ao lado do que cada comando faz, e o `export-binds` emite as
linhas `bindsym` a partir dela. Some um id da tabela, some o bind. É o mesmo padrão dos tokens, do
i18n e do WIT — gerar, em vez de manter duas cópias sincronizadas à mão.

O texto que vai no bind é o MESMO que vai no fio quando o comando é chamado por nome, porque os dois
saem do mesmo feito declarado. Não há segunda grafia a derivar, e por isso não há terceira tabela.

**Um erro aí É silencioso, e esse é o perigo novo.** Uma linha `bindsym` que o compositor não entende
é recusada SOZINHA — o resto do arquivo carrega — então o sintoma é um atalho, ou uma família inteira
deles, sem fazer nada. Quem responde é o `compositor-config-valid`, que põe o arquivo inteiro na
frente do parser dele e lê a SAÍDA.

**Os acordes saíram da lista do toque.** Eles apareciam ali em tom apagado, e não eram escolhas do
que se está fazendo: a leader já está solta, e apertar `←` naquele momento não faz nada. Vinte e
cinco linhas para dizer o que existe em *outro* modo de uso — a lista encolheu pela metade quando
saíram. O que saiu foi para o **manual** dos atalhos (`LEADER h`, e ele na lista dos manuais), que
ocupa a mesma região expandida e é gerado da tabela: mudar um atalho muda o manual, sem passo
intermediário. Um manual escrito à mão seria a segunda cópia da tabela de atalhos, e o dia em que ela
mente é o dia em que alguém confia nela.

**Os manuais têm duas portas, e as duas fazem o mesmo.** A região do rodapé e a janela
(`ronoctua-shell manual`, `manual_window.rs`) dirigem a mesma `manual::View`: a lista, com o filtro
digitado ali mesmo — espaço incluído, porque os nomes têm espaço, `DEL` tirando um grafema e `C-v`
colando como uma linha só —, o manual aberto dela com `RET` ou com o segundo clique, e `Esc` subindo
um nível: do manual para a lista, da lista para fora. O manual aberto anda uma linha nas setas, uma
página em `SPC` e nas teclas de página, e vai às pontas em `<home>` e `<end>`; o leitor de teclas do
rodapé não tinha símbolo para as teclas de página, e o `ronoctua-keymap` ganhou `<prior>`, `<next>`,
`<home>` e `<end>`, os nomes do Emacs. A janela abre na lista, ou num manual pela palavra que o `C-w`
do rodapé lhe passa, e uma palavra que não é de manual nenhum é recusada. O cabeçalho dela ensina as
teclas do nível, cada uma um clique, menos o `C-w`: ele tira um manual do rodapé para uma janela, e
a janela já é ela.

O `+` que os prefixava virou a marca de **submenu**, e só dela.

O config do compositor passou a ser uma **derivação**, e não uma string. Interpolar a saída de uma derivação
numa string de avaliação seria import-from-derivation: a avaliação do flake pararia para esperar um
build.

**`Esc` não mata buffer.** Ele cancela o que está em curso — a busca — e para aí. Destruir trabalho é
comando nomeado (`LEADER w d`), porque não pode depender de uma tecla que se aperta por reflexo ao sair
de qualquer coisa.

#### Os feitos do compositor viram comandos

**Toda funcionalidade é um comando nomeado**, e do sistema faz parte o que o compositor sabe fazer.
Os 112 feitos entram no registro como `wm-<nome>`, alcançáveis pelo localizador (`LEADER :`).

**A lista é DECLARADA, e não extraída.** O compositor anterior enumerava as ações dele e nós as
líamos do binário; este não enumera nada — os comandos são texto livre em quatro arrays em C. Então a
tabela é nossa (`assets/compositor-deeds.nix`), o que aliás é o que o contrato pede: o vocabulário da
distro deixa de ser emprestado.

**O que impede uma tabela declarada de envelhecer é o gerador.** Ele varre o `sway/commands.c` da
fonte PINADA e **recusa um verbo que não esteja lá**, com piso de contagem para que uma varredura
vazia não passe por verde. Um verbo removido é falha de build com o id na mensagem — e não um comando
que responde uma recusa que ninguém lê. Ele também recusa um `{n}` sem argumento declarado e um
argumento sem `{n}`, que é a outra metade da mesma mentira.

**E o argumento é VALIDADO** (`wire::spoken`): número tem de parsear como `f64`, palavra tem de casar
`[A-Za-z0-9._:+-]+`. Os separadores de comando aqui são `;` e `,`, então isto não é higiene — é a
diferença entre um argumento e uma injeção. Há um teste chamado
`an_argument_cannot_smuggle_a_second_command`.

**Eles vão pelo caminho direto**, sempre. Não há cliente de linha de comando no meio: o texto que o
bind escreve é o mesmo que o socket recebe, e ambos saem do mesmo feito. O compositor anterior
guardava padrões de argumento dentro do clap dele, num lugar que não dava para ler, e por isso as
ações enumeradas eram executadas pelo cliente DELE; aqui não há nada escondido para herdar.

**Eles não ganham tecla**, e é a única exceção à regra de que todo comando tem atalho: 112 atalhos
seriam 112 teclas a menos para o que se usa. A exceção é reconhecível pelo prefixo `wm-`, então ela
não cresce sem alguém escrevê-lo.

**Feito com argumento pergunta.** Escolhido no localizador, ir para um espaço vira um prompt e o que
se digita vai como argumento — depois de passar pela validação acima. Uma sequência de teclas não tem
como carregar um número: ela é um caminho até um id, não uma chamada com parâmetro.

**As 112 descrições estão nos dois idiomas**, e um check recusa feito sem chave. Subir a tag do
compositor **para o build** até o que apareceu ser traduzido. É barulhento de propósito: a
alternativa é o inglês do upstream aparecendo no meio de uma lista em português, que é o fallback
silencioso com outra roupa.

---

### Três conjuntos da leader — o que vem logo depois dela

O dono pediu que a tecla que abria o SUBMENU de um app abrisse o app: `LEADER s n` é a rede no
rodapé, onde era `LEADER s n n`. Um toque de `Alt` logo depois da leader faz a mesma tecla abrir a
JANELA do app, e um toque de `Ctrl` devolve a árvore de antes, com cada submenu como era. São três
árvores (`leader::Set`: `Footer`, `Window`, `Full`), e toda sequência começa na primeira.

**A dobra é LIDA da convenção das duas portas, e não listada** (`leader::folds`). Um grupo G dobra
quando `G <última tecla de G>` e `G w` são comandos, e comandos diferentes: é a convenção que toda
vista com rodapé e janela já seguia só nos nomes. No conjunto padrão G vira o comando do rodapé; no
das janelas, o da janela; e tudo abaixo de G sai dos dois — o resto de um grupo dobrado (`s n a`, o
modo avião) fica a um toque de `Ctrl`. Só a dobra mais de fora conta. Uma lista mantida à mão ao lado
das tabelas seria mais uma coisa a esquecer no dia em que um app ganhasse janela; o
`the_collapsed_groups_are_these` fixa o que a regra responde hoje, e um grupo novo que dobre é uma
decisão tomada de propósito. Hoje são dezessete: `.`, `p`, `t`, `n`, `i`, `s h`, `s n`, `s s`, `s d`,
`s a`, `s c`, `s i`, `s p`, `s b`, `s l`, `s g` e `s u` (cuja janela é a `update-watch`).

**O `w` não dobra, e a regra diz por quê sem exceção**: a tecla dobrada dele É o `w` dele (`w w`), um
binding e não duas portas. **E nada embaixo do `w` dobra.** Ali estão os níveis do menu da janela,
gerados com as letras do menu, e neles `w` é só mais uma letra: `w v v` seleciona a janela e `w v w`
o espaço inteiro, dois feitos de um nível que a regra, lida sozinha, tomava por rodapé e janela — o
teste fixado pegou. Grupo com uma porta só (`RET`, `TAB`, `TAB m`, `s q`) ou com nenhuma (`m`,
`m i`, `s`, `d`, `t i`) é igual nos três. E um grupo dobra nos dois conjuntos ou em nenhum: uma tecla
que fosse comando num e lista no outro seria uma terceira regra que ninguém escreveu.

**Três árvores, e não uma árvore com um interruptor dentro.** Cada uma passa pelo `Tree::build_it`
(`leader::Trees::build`), então uma colisão que a dobra crie — um grupo dobrado caindo numa tecla que
é prefixo de outra coisa — é erro de build no conjunto em que acontece, com o nome dele, como é na
árvore completa. O `check-keymap` constrói as três; o `reachable` pergunta à completa, que tem toda
sequência que as outras dobram. O `,` do buffer, a linha condensada `1…9` e a linha do gesto das
barras entram na raiz de cada conjunto por uma função só (`Footer::leader_root`), e o `w s` é
enxertado sob o `w`, que é grupo nos três.

**Um toque, e não um acorde.** Segurar `Alt` com a letra é `M-s`, e `M-` é a família do compositor:
não chega ao rodapé. E no aninhado o `Ctrl` e o `Alt` da própria leader estão segurados por
construção quando a sequência começa, então um modificador segurado não pode querer dizer nada aqui.
Um toque — apertado e solto sem nada no meio — é um gesto próprio (`leader::Tap`): uma soltura sem
descida (a da leader do aninhado) não conta; uma tecla no meio, ou um segundo modificador, é acorde,
e por isso o `Control_R` segurado com as setas continua paginando; `AltGr` não é `Alt`, porque é como
metade do mundo digita as próprias letras. O toque é lido nos dois caminhos de tecla, o do assento e
o dos teclados independentes (`typed` e `key_released`). O ponteiro agindo no meio — um botão, a
roda, um dedo encostando — também é acorde (`Tap::pointer_acted`): sem isso, o `Ctrl` solto depois
de um Ctrl-clique em `ALT janelas` trocava para a árvore inteira.

**No teclado na tela, o toque também vale.** Lá um modificador tocado TRAVA, e a soltura dele só sai
depois da próxima tecla — que a essa altura já fez acorde: a tecla sai como `M-s`, que nada liga. Então, com a
leader na raiz, um `Alt` ou `Ctrl` travado sozinho, sem tecla digitada depois, é o toque
(`osk::Board::spend_a_lone_tap`, `leader::set_of_osk_tap`): a trava é desfeita na hora, a soltura
sai, e o conjunto troca ali mesmo. Fora da raiz da leader, a trava é o que sempre foi.

**Só na raiz, e o último vence.** Um conjunto escolhido no meio de `s n` daria às teclas já digitadas
um sentido com que elas não foram digitadas. Toda abertura volta ao padrão.

**A leader de novo não recomeça mais em dois dos três.** Na raiz do padrão e do das janelas, ela
esconde ou devolve as barras (ver "Os modos ocultos"). No completo, recomeça, como sempre: é onde se
vai para digitar, e uma sequência recomeçada por engano custa menos que barras escondidas por engano.

**A tela diz o conjunto.** A sequência pendente é `SUPER ALT-` ou `SUPER CTRL s n-`: o mesmo `s n`
roda uma coisa diferente em cada conjunto, e a sequência na tela é o único lugar que diz qual árvore
a próxima tecla anda. `ALT` e `CTRL` são constantes ao lado das tabelas (`WINDOW_SET_KEY`,
`FULL_SET_KEY`), nunca texto num `.ftl`. Na raiz, a linha de dicas da leader ensina os outros
conjuntos como cliques (`leader::set_hint`: `ALT janelas · CTRL tudo` no padrão, só o `CTRL` no das
janelas), porque o ponteiro não toca modificador (invariante 19). O clique roda `leader-set-window`
ou `leader-set-full`, comandos que existem para isso e para o `LEADER :`, e o ponteiro parado lê a
frase `cmd-` deles. Pelo nome, uma leader fechada abre já no conjunto, e uma no meio de uma sequência
recomeça na raiz dele. Trocar de conjunto, e abrir a leader, tiram o último eco da pessoa da linha
(como toda tecla da sequência): o eco passa na frente da sequência e das dicas, e o eco de um `Esc`
ou de uma tecla sem comando logo antes as escondia — e os cliques delas — pelos três segundos dele.

**O que se ensina parte do conjunto padrão** (`leader::taught_keys`): a sequência do rodapé quando
existe (`t`, a fila), senão a das janelas depois do `ALT` (`ALT .`, a janela do gerenciador), senão a
da árvore completa depois do `CTRL` (`CTRL s b y`, responder um pareamento). Ensinada de outro jeito,
a tecla faz outra coisa quando digitada onde toda sequência começa: `t t` no padrão é a fila e depois
uma tecla que não leva a lugar nenhum. A vista de programas, o cartão de pareamento e as células do
menu da janela leem dela; as constantes ao lado dos bindings (`TRANSFERS_KEYS`, `PAIRING_YES_KEYS`)
continuam sendo da árvore completa, que é quem as lê.

**As sequências citadas neste documento são as da árvore completa**, a que um toque de `Ctrl`
alcança. No conjunto padrão, um grupo de duas portas é a própria tecla do grupo: `LEADER t t` é
`LEADER t`, e `s n w` é `LEADER Alt s n`.

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
`Super` é do compositor, e um buffer que respondesse a ele roubaria uma ligação dele.

O que sobra é o which-key não listar essas ligações sob `SPC m`. O obstáculo de tipo saiu —
`ronoctua_keymap::Entry` passou a guardar `String` justamente por isso —, mas o que ficou é maior e
só apareceu depois: **o rodapé não tem como saber o descritor de um buffer em execução.** Ele conhece
as janelas pelo IPC do compositor (id, título, `app_id`) e nada mais.

---

### Workspaces: `TAB`, nove fixos, e o número na tecla

O grupo saiu de `e` para **`TAB`** — a tecla do alternador de janelas de todo sistema —, e `TAB TAB`
mostra o panorama. Cada ação tem várias teclas de propósito: `j`/`k` do vi, `[`/`]` de quem vem do
navegador, setas para quem não decorou nenhum dos dois, e `.`/`,`. Redundância aqui custa uma linha
de tabela e poupa uma decisão por uso.

**São nove espaços NUMERADOS**, e não os dinâmicos que aparecem e somem. O compositor cria e destrói
espaço conforme o uso: `LEADER TAB 7` sem nada declarado não teria para onde ir, e a mesma tecla
levaria a lugares diferentes conforme quantas janelas estivessem abertas. O que o comando manda é
`workspace number N`, que NOMEIA o número e cria o espaço se ele não existir — então o dígito na
tecla **é** o número do espaço, sempre.

**Nove, e o `0` é TODOS eles** (regra do dono). O `0` não é um décimo espaço: `LEADER 0` e `Super+0`
mostram todos os espaços de uma vez (`workspace-all`). O arquivo que o Scroll traz põe um décimo no
`0`, e o `every-compositor-shortcut-is-bound` registra a troca (`INSTEAD`). Onde o `0` leva a um
número — `TAB 0`, `TAB m 0` e as faixas que movem ou trocam de espaço, `Super+Shift+0` e
`Super+Ctrl+0` —, ele nomeia o nono, e nunca cria um espaço que nada mais aqui alcança. A modeline
tem nove números (`workspaces::POOL`), e o `footer-mouse-reaches-nothing-new` recusa um décimo.

**O que se perde é o espaço VAZIO, e isso fica declarado.** Este compositor não tem espaço
persistente nem pré-criação — conferido no `scroll.5`: há `workspace <nome> output/gaps/…` e nenhuma
forma de declarar um que exista vazio —, e um `exec` visitando 1..9 na partida não resolve, porque um
espaço vazio é destruído assim que se sai dele. A visão geral mostra o que tem janela. Recriá-los com
um daemon nosso seria desproporcional, e está escrito aqui como não feito.

`TAB m` repete o submenu inteiro para **levar a janela**: quem aprendeu `TAB 3` ganha `TAB m 3` de
graça.

**E a ligação passou a carregar argumento.** `TAB 3` e `TAB 7` são o mesmo comando com números
diferentes; sem isso, a alternativa seria `workspace-ir-1`…`-9` — nove nomes no localizador dizendo a
mesma coisa e nove chaves de texto para traduzir, só para carregar um dígito. O acorde já fazia isso
(`LEADER+3` leva o `3` na linha do bind); agora a sequência também. Os dois montam o MESMO texto,
pelo mesmo feito declarado, e o número passa pela validação de `wire::spoken` antes de sair.

---

### O menu da janela — o nome do app, e `LEADER w` gerado dele

**O nome do app na modeline é o menu da janela** (`window_menu.rs`, `LEADER w o`). Clicar no
segundo segmento (`mode`; o título, à esquerda, fica como está) abre na região a grade da leader com
tudo o que o compositor faz com a janela em foco: uma tecla e um rótulo por célula, o grupo com `+`
no tom de grupo, e o grupo troca a grade pela dele. O que difere é a volta: `Esc` sobe UM nível,
porque um submenu é um lugar aonde a pessoa escolheu ir. É uma grade que PAGINA, como a da leader, e
não uma lista com cursor (`Footer::navigation`); o `every-view-follows-its-cursor` remede essa
desculpa a cada build: lê os braços de `navigation` que respondem `Page` e recusa um cursor do menu,
no `window_menu.rs` e nos caminhos de tecla e de clique do rodapé.

**`LEADER w` é o menu inteiro, gerado dele** (`window_menu::leader_bindings`), a pedido do dono: um
grupo só para janela, e não dois que discordam. Cada célula é uma sequência e cada submenu um grupo;
o que o `w` já ligava ao MESMO comando não é ligado de novo, e uma tecla que os dois dessem a
comandos diferentes é colisão, que a árvore recusa. Por isso o tom de grupo (`text.group`) aparece
em dois braços do rodapé — o da leader e o do menu — e continua querendo dizer só "submenu da
leader" (`a-heading-is-not-a-submenu`). Sob o ponteiro, cada célula ensina a tecla de fora do menu
(`window_menu::taught`).

**Todo item age na janela do menu, por critério** (`Compositor::deed_on`, `[con_id=N] …`), e age
antes de o menu fechar: sem alvo o compositor age na última janela focada, e o foco anda entre abrir
o menu e clicar.

**A gaveta é alcançável pelo ponteiro** (o *scratchpad*; `LEADER w g`, e a marca `drawer` na
modeline enquanto a gaveta tiver alguma janela, guardada ou fora dela): uma janela guardada não tem
foco nem nome, e nada clicável a trazia de volta. O botão esquerdo abre o menu na gaveta, uma célula
por janela guardada; o direito tira a próxima, que é o acorde do próprio compositor
(`wm-scratchpad-show`).

**As trilhas são números na modeline** (`trails.rs`, `LEADER w y`): o glifo com quantas janelas a
trilha em uso tem e, a partir de DUAS trilhas, um número por trilha, a em uso mais clara — a regra
dos espaços. O clique num número põe aquela trilha em uso (`wm-trail-number`, o mesmo dígito do modo
de trilha); o compositor conta de ZERO, e a conversão mora só em `trails::index_of`. Andar pela
trilha até uma janela da gaveta derruba o compositor, e o inv. 3 proíbe o patch: toda janela
guardada sai da trilha em uso (`untrail_the_drawer`), e uma notificação diz isso.

**A seleção é uma CÓPIA** (`selection.rs`, `LEADER w v` e a marca `selection`). O compositor guarda
a seleção num campo que nenhuma resposta, nenhum evento e nem a API Lua escrevem — medido na fonte
pinada —, então o rodapé mantém a dele, escrita pelos feitos ao passarem por `run_with`; os acordes
de seleção (a família `Insert`) passam pelo nosso binário para tecla e clique escreverem a mesma
cópia. Ela não vê a seleção feita por script, nem a coluna inteira que o compositor marca em alguns
modos: a marca diz "há algo selecionado" pela cópia, e o menu age no que o compositor de fato tem.

**Toda marca nova tem porta de teclado.** `mode` roda `window-menu`; `drawer`, `trail` e `selection`
abrem o menu no nível deles, que sob a leader são os grupos `w g`, `w y` e `w v`; o número de uma
trilha é o dígito do modo de trilha. O `footer-mouse-reaches-nothing-new` confere cada par.

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

### Ponteiros: um assento por dispositivo, e toda porta ouve todos

O compositor desenha um cursor por ASSENTO, e cada assento tem o próprio foco. Isso é o suporte a
vários cursores, e ele é do compositor desde o fork do sway; o que faltava era a distro usá-lo.

**Um dispositivo apontador, um assento.** O `ronoctua-pointers` vê os dispositivos pelo contrato
(`Compositor::devices`, `seats`, e o evento `DevicesChanged`) e decide, num plano puro
(`plan.rs`), em que assento cada um fica. O primeiro ponteiro que a máquina viu fica no `seat0`,
com os teclados; os seguintes ganham `ronoctua-cursor-N`. O número fica ANOTADO em
`pointers.json`, e um número de um dispositivo lembrado não é dado a outro — o mouse que volta da
gaveta volta ao mesmo cursor, com a mesma cor. Dois mouses idênticos têm o mesmo identificador
(`vendor:product:nome`), e o `seat … attach` recebe identificador: são um dispositivo só, e a
tela diz isso em vez de oferecer uma escolha que o compositor não honraria.

**Tirar um dispositivo de um assento é um RELOAD, e é o único caminho.** O `attach` MOVE o
dispositivo e o compositor não tem *detach* nem remoção de assento. O que tira é reler a
configuração: o fragmento (`scroll-pointers.conf`) já não nomeia aquele assento, o reload destrói
o assento sem configuração, e o dispositivo cai no *fallback*, que é o `seat0`. Por isso o
fragmento nunca escreve `attach` para o primeiro cursor: voltar a ele é NÃO ser nomeado.

**Cada ordem lê o arquivo, muda uma coisa e escreve.** Duas portas rodam a thread — o rodapé e a
janela —, e a tela de monitores pagou duas vezes por guardar uma cópia das preferências entre
ordens (`wrote_the_screens`). Aqui cópia nenhuma sobrevive a uma ordem. E só o rodapé DETECTA: é
ele que está vivo a sessão inteira, e duas portas numerando o mesmo mouse brigariam.

**As cores são as mesmas em todo tema.** A cor do cursor é como a pessoa sabe qual mão é qual; um
tema que repintasse os cursores desfaria isso. Oito matizes a ΔE iguais a partir de uma matiz fixa,
numa luminosidade só (numa rampa, dois cursores difeririam sobretudo em quão claros são), com o
portão `cursors` do `gen-theme.py`. O hover de cada cursor é por tema, porque deita na superfície
do tema — e só aparece colorido com mais de um cursor: sozinho, é o `surface.hover` de sempre.

**As formas são do Bibata, recoloridas na build.** Recusado: desenhar trinta e cinco cursores à
mão, com o animado. Aceito: a fonte SVG que o nixpkgs já pina, com os três marcadores de cor
trocados pelos tokens e o `ctgen` fazendo o Xcursor. Oito temas, quatro tamanhos, 43 MB — com 16 e
96 px eram 92, e os bitmaps de 96 eram metade.

**Por que cada porta troca o estado em vez de reescrever a lógica.** As quinze portas guardavam
um ponteiro, e cada uma tem centenas de linhas lendo `self.row_under_pointer`, `self.dragging`.
Reescrever cada uma para um mapa por assento era reescrever a lógica de ponteiro da shell inteira.
O `Seats::switch_to` guarda o estado do ponteiro anterior e traz o do que falou antes de a porta
ler qualquer coisa — a lógica fica como está e passa a valer por cursor.

**O que não se cruza sem patch do compositor (inv. 3)**, e está escrito aqui para não virar
promessa: `focus_follows_mouse` é global; só o foco de um assento (o do último comando) ganha a
borda `client.focused`; `send_unfocus` desativa uma janela quando um assento a deixa, mesmo com
outro nela; uma superfície `exclusive` toma o teclado de todos os assentos; e app estrangeiro que
ouve um assento só responde só ao primeiro.

**Teclados independentes: por que o keyd sai, e não é consertado.** O keyd agarra todo teclado e
emite UM virtual — é o desenho dele, e é o certo para uma pessoa. Dois teclados em dois assentos
precisam de dois dispositivos no compositor, e nenhuma configuração do keyd dá isso. Então o modo
Independentes troca o daemon, e não o remenda: o `ronoctua-keys` fala o MESMO vocabulário
(`assets/keyboards.nix`, com o mesmo sentido do `overload` do keyd — toque é pressionar e soltar sem
outra tecla no meio), para que a mesma palavra faça a mesma coisa nos dois modos. O layout vai no
bloco `input` do gêmeo, INTEIRO (layout, variante E opções): o compositor só mescla o `type:keyboard`
num dispositivo que já existe quando o config é lido, e um gêmeo nascido depois viria sem nada.

**Por que a leader não passa pelo Wayland nesse modo.** Uma layer com teclado `exclusive` recebe o
foco de TODOS os assentos de uma vez, e não há IPC que foque uma layer para um assento só. Pedir o
teclado seria calar todos os outros enquanto alguém usa a leader. O daemon já tem todo teclado; ele
diz ao rodapé qual tocou a leader, e DESVIA só aquele pelo socket até a região fechar. O rodapé
traduz com o xkb daquele teclado e entrega o mesmo `KeyEvent` que o Wayland entregaria.

**Por que o par teclado–cursor é pelo `vendor:produto`.** O teclado Bluetooth e o touchpad dele são
dois dispositivos do mesmo aparelho, e é por esse par que o `plan::one_physical` já junta as
interfaces de um touchpad. Sem par, o teclado digita pelo cursor 1, que é onde todo teclado estava.

---

### A porta — uma base para toda janela

Toda janela do shell é um `door::Content` aberto por `door::run`, e a `Door<C>` é dona da
encanação: os handlers do SCTK são escritos uma vez, genéricos (`impl<C: Content> … for Door<C>`,
e `delegate_dispatch2!` aceita a forma genérica). A alternativa óbvia era uma MACRO que colasse os
handlers em cada janela — descartada porque uma macro gera dezenove cópias iguais que o compilador
não compara com nada, e o primeiro ajuste feito à mão numa delas é a deriva de volta. A segunda era
um trait com a encanação dentro, que cada janela implementasse: é o que existia, escrito à mão
dezenove vezes. Uma estrutura genérica tem UMA cópia e o compilador a instancia.

**Por que padrões e não um formato fixo.** O dono pediu as duas coisas: estabilidade e UX
consistente por construção, e liberdade para uma janela não seguir um padrão quando isso for
decidido. Cada comportamento de `Content` tem um padrão que É o padrão do sistema; sobrescrever
é explícito, um método por vez, e o `every-window-is-a-door` recusa a outra saída — um handler
próprio ao lado da porta. O `Ui` é separado da `Door` para um conteúdo poder recebê-lo por `&mut`
enquanto a porta segura a encanação: são dois empréstimos de coisas diferentes.

**O toque veio de graça, e esse era o teste da base.** Um dedo é tocado pelo mesmo código de
ponteiro de um mouse, com uma mão própria (`door::touch::Fingers`); nenhuma janela escreveu uma
linha sobre toque, e toda janela futura o terá.

#### Toda porta sabe a sua tela

A `Ui` de toda porta guarda as telas que a conexão dela descreve (`door::screens::Screens`): o
conector do `wl_output` versão 4 (`eDP-1`), a descrição e o output, mantidos por `new_output`,
`update_output` e `output_destroyed`, e em quais delas a superfície está, pelo `enter` e `leave` da
própria superfície. É protocolo do núcleo (inv. 3) e nenhuma pergunta ao compositor no laço. A
janela ouve `Content::screens_changed`; `Ui::screen_named` substitui o `Door::output_named`. Uma
porta que não sabe o nome da tela em que está conta como TODAS as telas, porque o que é roteado a uma
tela e some porque o compositor demorou a dizer onde a janela está é um defeito sem sintoma.

A coluna sabe a sua tela antes da superfície, pelo espaço de trabalho, e diz ao inquilino
(`Tenant::placed_on`) cada vez que ela muda: depois do nascimento (o construtor, ou o `detached` de
uma fotografia), `Column::now_on` é o único lugar em que o `shown_on` muda e o único que avisa. A
coluna que NASCE numa tela deve o aviso (`placement_owed`): o construtor não tem `Ui` e o primeiro
`place` a acha onde é querida (`Step::Stay`), então ela avisa na primeira volta do laço ou no
primeiro quadro, o que vier antes. A fotografia não avisa: a tela dela não tem conector, e não
saber conta como todas. O rodapé guarda o conector em cada `Screen`, lido quando a saída é descrita, e é dele que
saem a chave da imagem do player, a borda da tela coberta e a tela do ponteiro.

#### Toda janela digita o que o rodapé digita

A regra do dono — as duas portas fazem as mesmas coisas — tinha uma lacuna que era da base, e não
de uma vista: só o rodapé tinha onde perguntar. Toda janela que precisava de um nome, um endereço,
um caminho ou uma senha mandava a pessoa ao rodapé, e o rascunho que só a janela tinha não se
respondia em lugar nenhum. A metade do minibuffer que pergunta virou peça própria, `line::Line` (o
id que pergunta, se é segredo, o texto), e as duas portas seguram a mesma: o `Minibuffer` guarda
uma enquanto pergunta, e o `Ui` de toda porta também (`Ui::asking`, aberta por `Ui::ask`).

- **Enquanto a linha pergunta, toda tecla é dela**: `Door::typed` a desvia antes do
  `Content::typed`, o mesmo desvio do `minibuffer_key` do rodapé. `RET` responde em
  `Content::answered(id, texto)`, que a janela liga à mesma chamada da vista que a resposta do
  rodapé alcança; `Esc` e `C-g` largam (`Content::unanswered`); `DEL` tira um grafema inteiro, e
  caractere de controle não se digita.
- **As duas portas desenham a pergunta com as mesmas peças** (`door::list::question_pieces`): a
  pergunta (`ask-<id>`), o texto — cortado no começo, porque o fim é onde a próxima letra entra — e
  `ESC` e `RET` como cliques (inv. 19), explicados sob o ponteiro (`about-line-*`). Na janela é a
  linha 0, no lugar da mensagem e dos atalhos; no rodapé é a linha do minibuffer. O rodapé
  desenhava a pergunta como prosa, cortada no fim, e ela só se respondia pelo teclado.
- **Colar não espera ninguém, e cola o que está copiado agora.** `C-v` na linha, ou `Ui::paste`
  pedido pela janela, lê a seleção viva numa thread com prazo e teto (`pasting.rs`), e o texto volta
  pelo canal da porta — e só para a pergunta para a qual foi lido. O rodapé lê do mesmo jeito, pelo
  canal dele (a linha do minibuffer, a busca dos programas, o filtro dos manuais e a imagem do
  ícone). Ele colava a entrada mais nova do próprio histórico, que nunca guarda uma cópia marcada
  como segredo: uma senha copiada de um gerenciador de senhas colava a cópia anterior numa linha
  mascarada, onde ninguém via que era a errada.
- **Segredo não sai do processo.** O texto é mascarado na tela e no `Debug`, e a resposta vai para
  `Content::answered` e para nada mais: nem `argv`, nem o soquete do rodapé. No rodapé vale o mesmo
  para o log: a linha que registra toda resposta do minibuffer omite o texto de uma pergunta
  secreta (`Minibuffer::answered_a_secret`), e o registro de cada tecla lida perde a tecla enquanto
  a linha é secreta (`Minibuffer::typing_a_secret`) — antes, a senha ia para o log uma letra por
  linha. A exceção é o segredo cujo dono É o rodapé: o PIN de um pareamento vai ao agente, que é do
  processo do rodapé, pelo soquete de controle — que não é `argv`, e cujo registro só escreve o id
  do comando.
- **Uma pergunta que acabou leva a linha junto, nas duas portas.** A vista de rede esquece o que
  perguntou a qualquer tecla que ouve, e um clique chega a ela com a linha aberta: a porta fecha a
  linha (`network::Question::is_the_views`), e uma senha que chega sem pergunta não entra em rede
  nenhuma. O PIN de um pareamento que terminou, ou foi trocado por outro, também sai da linha do
  rodapé, como já saía da janela.
- **O que só o rodapé faz é pedido a ele fora do laço** (`Ui::tell_the_footer`: a linha de
  `ronoctua-shell command`, escrita no soquete de controle por uma thread), e um rodapé que não
  está lá vira frase na linha, nunca silêncio. As ordens do turno em que a janela fecha vão sem
  thread: o processo acaba com o turno, e a thread morreria antes de conectar.
- **Abrir um programa é da janela, e a falha é dita nela.** `Ui::launch` inicia um programa num
  escopo próprio, fora do laço (`foreign::start_in_scope`), e `Ui::launch_entry` inicia uma
  entrada do lançador pelo id: o catálogo é lido numa thread e a entrada começa como o rodapé a
  começa (`apps::argv_of`, com o terminal para quem pede um). O que deu errado — "não está no
  lançador ainda", "não abriu" — é frase na linha DESSA janela. O gerenciador de arquivos, a página
  do Syncthing, o login do Tailscale e o mapa soltavam um filho e só registravam a falha; a janela
  dos programas e a fila entregavam o id ao rodapé, que dizia a recusa na linha dele, na outra
  porta. Numa janela de buffer, que desenha todas as linhas, a mensagem da porta toma as linhas de
  baixo, e o componente é desenhado sem elas enquanto ela está lá.
- **Uma tecla chega a toda janela lida do mesmo jeito** (`door::keys::as_read` e `key_of`, um leitor
  só no lugar das cópias de cada janela): o Caps Lock desfeito como no rodapé — numa janela que
  digita texto, o terminal e o buffer, ele continua sendo maiúscula, menos num acorde — e o
  `Delete` sem o U+007F que as cópias digitavam. O botão Voltar do mouse é o `Esc`, pelo caminho da
  tecla — e um clique numa tecla, do botão ou do cabeçalho, é APERTAR E SOLTAR: o host de um buffer
  só decide que o `Esc` foi um toque quando ele sobe (segurado, ele escolhe um estado), e com o
  aperto sozinho o botão Voltar não fazia nada numa janela de buffer e deixava o `Esc` preso para a
  tecla seguinte.

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

**`xdg_toplevel`, e não uma região nossa:** quem posiciona a janela é o compositor. O shell nunca
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

**A tecla não compete com nada.** Passamos `-c`, então o config que o compositor traz não é lido e
`toggle-keyboard-shortcuts-inhibit`; o nosso acorde entra primeiro e a deduplicação descarta o dele.
A ação não se perde — como as outras 140, alcança-se por nome (`LEADER :`), que é para isso que a
exceção existe.

**Quem faz flutuar é o compositor** (inv. 4). O que o shell decide é a IDENTIDADE da janela: o mesmo
terminal sobe com `app_id` `br.ronoctua.monitor` em vez de `br.ronoctua.terminal`, e uma
`window-rule` casa esse id. Com um `app_id` só, a regra flutuaria todo terminal — inclusive o
`LEADER RET /`.

**E a regra é GERADA da mesma constante** (`export-window-rules`, como os acordes). Escrita à
mão, a string existiria em três lugares — o Rust, `nix/modules/scroll.nix` e `nix/dev.nix` —, e o dia
em que um deles divergisse o sintoma seria o monitor abrindo lado a lado: **exatamente com a cara de
um atalho funcionando**. O `terminal-vm` fecha o outro lado, com um compositor de verdade: a janela do
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
desenhou isto" continua tendo resposta fixa, e o check `renderer-loads-four-faces` a fixa. A guarda
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

#### As duas primitivas que faltavam, e a regra minha que estava errada

Escrevi no `docs/PENDING.md` que um buffer não podia ter um container que rola, porque nenhuma das
cinco primitivas era uma região rolável e inventar uma seria um sexto vocabulário. O dono disse que
a regra está errada. Está, e o raciocínio também: **um buffer já desenha a janela de lista que
quiser** — ele é dono do deslocamento, emite as linhas que quer, e o hospedeiro não recorta nada.
Não faltava nada no desenho.

Faltavam duas coisas, e nenhuma delas é uma região.

**`command::bar` — o polegar.** É uma fração de um trilho em PIXELS, e este lado conta CÉLULAS. Uma
barra feita de blocos seria uma sexta barra parecida com as cinco de verdade e igual a nenhuma delas.
Então a barra atravessa como NÚMEROS — total, mostrados, deslocamento — e o hospedeiro a desenha com
o `ronoctua_design::Scroll`, que compila dos dois lados da parede desde o dia em que foi escrito e é
a mesma aritmética das nove vistas. O resíduo de pixel continua sendo do hospedeiro, que é o único
que conhece a superfície. Nada é desenhado quando a lista cabe (`Scroll::cut`).

**`input::wheel` — a mão.** Em LINHAS e nunca em pixels, a mesma parede que o `click` respeita, pela
aritmética única do `surface::axis_lines` — a que guarda a fração entre eventos, que é o que faz um
gesto lento mover alguma coisa. Uma barra que ninguém pega é o retrato de um controle.

**E `command::paragraph`, pela mesma razão do outro lado.** Um buffer segura uma CHAVE. O `text`
desenha UMA linha e o hospedeiro a elide no orçamento — certo para um rótulo, inútil para uma
descrição, que cortada na largura de uma coluna é uma descrição que ninguém lê. E quebrar linha
precisa da string TRADUZIDA, que este lado nunca vê e não saberia medir. Então o RETÂNGULO atravessa
e o hospedeiro o preenche, com o mesmo `layout::wrap` que um nome de arquivo comprido já usa, e elide
a última linha quando sobra texto. É o que deixa um buffer ter um rodapé que explica o que está sob o
cursor — a forma que toda vista do rodapé tem e nenhum buffer podia ter.

E o portão do `nothing_is_covered` aprendeu a distinguir **CHÃO de TINTA**: uma seleção existe para
ficar embaixo, e um rodapé com fundo próprio e um parágrafo em cima dele se denunciava como defeito.
Uma RÉGUA não é chão — um título alto que alcança a régua abaixo dele é exatamente o defeito que
aquele portão foi escrito para pegar —, e uma BARRA também não: o que mantém uma barra honesta fora
disso é o teste de COLUNA, não uma isenção.

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
confere se o nome citado num comentário corresponde a algo. `renderer-loads-four-faces` faz mais do
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
face desenha o quê. Por isso o check `renderer-loads-four-faces` deixou de
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
falha longe da causa — a armadilha que o `AGENTS.md` registra.

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
teclado, pintura e eventos do compositor pela vida inteira do aplicativo. Escondia-se atrás de quatro
chamadores fixos; o localizador faria dele o caminho principal. A espera foi para uma thread nomeada,
e o veredito volta pelo canal — o `apps-vm` abre um app que dorme dois minutos e exige o rodapé
respondendo enquanto ele vive.

---

### O driver gráfico vem do Nix, e não do host

O `nix run .#dev` parou de subir com uma mensagem que não nomeia nada:

```
scroll: symbol lookup error: /usr/lib64/libc.so.6:
      undefined symbol: __pointer_chk_guard, version GLIBC_PRIVATE
```

**`LD_LIBRARY_PATH` vence o `DT_RUNPATH`.** O contorno para host não-NixOS punha `/usr/lib64` inteiro
lá para alcançar **uma** biblioteca — o `libEGL.so.1` do host — e junto ia o `libc.so.6`. O processo
passou a rodar a libc do host contra o `ld.so` do Nix, e os símbolos `GLIBC_PRIVATE` da glibc só
combinam entre um `libc.so.6` e o `ld.so` **da mesma build**. Medido: host 2.44, Nix 2.42, e o
símbolo ausente do host.

Não existe versão a acertar. O que remove a classe inteira é não pôr as bibliotecas do host no
caminho — e para AMD e Intel isso não custa nada, porque **quem fala com a placa é o driver do
kernel**, e o mesa é biblioteca de espaço de usuário que o Nix tem. Medido: o compositor aninhado sobe em
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
`ronoctua-scroll`, do `ronoctua-network` e do `ronoctua-memory`.

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

O que as duas portas dividem é `files::View` — estado, teclas, cliques, menu, seletor e as `Row`s —
e a resposta sobre o que abre um arquivo (`files::opener_of`: texto e tipo que ninguém nomeia vão
para o editor, o resto para o programa que declara o tipo). O que difere é uma coisa só: **como se
inicia um processo**. Por isso o seletor mora DENTRO da vista
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

**Mas há uma terceira resposta, e ignorá-la deu um defeito na tela: o EMPATE.** O `by_name` escolhia
pelo peso do glob e descartava empates em silêncio, ficando com o primeiro da tabela. O dono
fotografou uma listagem e perguntou por que um VÍDEO estava com o glifo de música: o arquivo é
`big_buck_bunny_480p_stereo.ogg`, `audio/ogg` pelo nome e `video/x-theora+ogg` pelo conteúdo. Nenhuma
das duas respostas estava errada — o que estava errado é que o nome nunca teve uma. **Quatro tipos
reivindicam `*.ogg` no mesmo peso**, porque a extensão nomeia um CONTÊINER e o contêiner não diz o
que tem dentro.

Então `mime::named` devolve `Named { mime, sure }`, e `sure = false` é o nome dizendo que não sabe.
Aí — e só aí — os bytes decidem, **entre os tipos que reivindicam aquele nome**: um arquivo cujo
magic nomeia algo sem glob para ele é um arquivo mentindo sobre a extensão, e a extensão é o que a
pessoa digitou. Medido na pasta do dono: **8 nomes de 64** empatam, todos contêiner de mídia ou
playlist; todo `.jpg`, `.rs` e `.pdf` continua respondido sem tocar no arquivo.

**E as seis grafias de playlist ganharam UM glifo** (`list`), o que não é arrumação: `audio/x-mpegurl`
e `application/vnd.apple.mpegurl` reivindicam `*.m3u` e `*.m3u8` no mesmo peso, então qual delas
vence é decidido pelos bytes — e isso não pode ser visível numa listagem, porque ninguém olhando
distingue os dois nomes. Cobra o `playlist-formats-are-total`.

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

#### A tecla do gerenciador abria outra coisa, e hoje o grupo é o `.`

`f f` era o seletor de arquivo do EDITOR, e foi relatado como "um gerenciador de arquivos diferente e
inferior". A leitura estava certa: quem digita uma tecla num grupo chamado "file" está atrás de
arquivos, e o que subia era uma lista chata de nomes que só sabia fazer uma coisa com o que você
escolhesse. A tecla passou a abrir o gerenciador.

Depois disso o arranjo ficou com uma coisa alcançável por TRÊS sequências — `.`, `f f` e, na janela,
`f w` — e a letra `f` não dizia nada que o `.` não diga. O dono pediu a forma única: *"não quero que
`LEADER f` exista; quero que `LEADER .` seja um submenu"*. Então **`.` virou o grupo**, `. .` é o
rodapé e `. w` é a janela.

E isso obrigou uma coisa que vale escrever: **o `.` deixou de ser comando para poder ser prefixo**.
O registro recusa as duas coisas na mesma sequência — um prefixo que também executa faz o primeiro
toque rodar algo enquanto a pessoa ainda está digitando —, e é a mesma regra que fez `s b b`, `s n n`
e `t t` dobrarem a letra. A segunda porta ganhou casa sem inventar um segundo grupo, e o preço era um
toque a mais — que deixou de ser pago: no conjunto padrão da leader o grupo de duas portas é a
própria tecla, `LEADER .` abre o gerenciador no rodapé e `LEADER Alt .` a janela, e a árvore com
`. .` e `. w` continua inteira a um toque de `Ctrl` (ver "Três conjuntos da leader"). A regra
continua valendo nessa árvore, e é ela que dá à convenção os dois nomes que a dobra lê.

O grupo tem DUAS teclas e nada mais: `. .` abre o gerenciador no rodapé e **`. w` o abre em janela**,
`w` de *window* — a mesma letra que toda vista com segunda porta usa (`t w`, `n w`, `s n w`, `s p w`,
`s b w`, `s u w`). O teto de doze linhas do rodapé está certo para um rodapé e não para o trabalho que
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
com todas as letras —, então o compositor consumia a tecla antes de o rodapé existir para ela.

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

**Nas duas portas.** A janela perguntava só ao catálogo: um texto ia para quem declarasse
`text/plain`, e um arquivo sem tipo conhecido dizia que nada o abria — onde o rodapé mandava os dois
ao Emacs. A escolha (`files::opener_of`) e o comando (`files::editor_command`, o cliente, `-c` e o
arquivo, na pasta dele) são uma resposta só, e a janela a inicia no escopo próprio dela.

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
direto na `View::key`, e a pergunta era se a tecla CHEGA, atravessando Xvfb, o compositor e a layer surface.

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

#### As duas portas da fila fazem o mesmo, por UMA tabela

O painel do rodapé (`t t`) e a janela (`t w`, um buffer wasm) discordavam: `RET` segurava o
trabalho num e abria a página dele no outro, `C-p` subia num e segurava no outro, a janela tinha uma
página por trabalho e perguntava antes de desistir, e o painel não tinha nem a página nem a pergunta
— desistia na hora. E só o painel abria o que uma instalação pôs no lançador.

Agora as teclas são uma tabela só, em `ronoctua-protocol` (`TRANSFERS_KEYS`), que compila dos dois
lados (invariante 12). O painel lê as notações; o buffer declara cada acorde sob o líder local
(`local_binding`: `C-c` é `, c`, que o host dele devolve como `C-c`) e recebe o resto — as setas,
`RET`, `TAB`, `SPC` — como as intenções que o host já manda. Quais teclas uma linha ENSINA também é
uma resposta só (`transfers_taught`): segurar só onde o motor segura, desistir só do que está em
curso, esquecer só do que acabou, abrir só o que uma instalação fez. As duas linhas de teclas são
cliques, e o que cada porta faz com o id é o mesmo:

- `RET` (e `TAB`, `C-d`) é a página do trabalho, um campo por linha — o que o segundo clique numa
  linha faz, nas duas; `Esc` volta uma página e só então fecha.
- `SPC` segura e solta; `C-c` PERGUNTA, na tela da vista, com o `não` primeiro e o cursor nele — as
  setas andam entre as duas respostas, `RET` fica com a do cursor, `Esc` é `não`, e o primeiro
  clique numa resposta escolhe e o segundo responde. Uma pergunta sobre um trabalho que acabou some
  com ele.
- `C-o` abre o que a instalação pôs no lançador. No buffer isso é um pedido (`launch-job`, com o id);
  o host acha a entrada — o buffer só sabe que ela existe (`job.opens`) — e a inicia da própria
  janela (`Ui::launch_entry`), como a janela dos programas faz; o que der errado é dito nas linhas
  de baixo da janela, que a mensagem da porta toma do componente enquanto está lá.

O registro `job` do WIT ganhou `pauses` e `opens` para isso: sem eles o buffer ensinaria segurar
numa instalação que a fila recusa segurar, e não saberia que uma linha abre algo.

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

**E `sizeof` É o contrato.** O `AGENTS.md` já registra que `#[repr(C)]` promete ARRANJAR e não
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

**Onde cada parte é provada.** O `tests/extension-vm.nix` confere o que não precisa de navegador:
a chave existe, o `.crx` está assinado, o id da política é o id do `.crx`, a lista dos dois hosts
nomeia exatamente ele, e a versão sobe quando a build muda. O `tests/kiosk-vm.nix` roda o Chrome
de verdade, offline, e prova o resto: que ele instala a extensão pelo `update_url` `file://` e sobe
os dois hosts (veja "O Chrome quiosque").

### O Chrome quiosque — `LEADER m b`: o seu Chrome, sem as barras dele

O dono pediu o Chrome dele sem a interface do Chrome, e uma modeline que faz o que a barra fazia:
as abas ao vivo (trocar e reordenar), as extensões fixadas e o acesso às outras, o zoom, uma linha
de endereço, voltar ao Chrome comum, editar o endereço, abrir aba nova, voltar, avançar e
recarregar.

**Uma janela comum do Chrome da pessoa.** O quiosque abre no perfil que o Chrome usou por último
(lido do `Local State` pelo `ronoctua-software`; `Default` quando não há), com os logins dela. O
compositor faz `fullscreen enable application` na janela: diz a ela que está em tela cheia
enquanto o tile mantém o tamanho, e o Chrome esconde abas e barra. A janela fica no lugar dela, e
o rodapé fica embaixo. Nunca se passa `--kiosk`, `--start-fullscreen`, `--user-data-dir` nem
`--enable-features`: os dois primeiros trancam o navegador inteiro, o terceiro perde os logins, o
quarto substitui o do embrulho. O `web-apps-are-never-kiosk` varre também o `ronoctua-browser` e
exige que o `launch::argv` tenha só as palavras do quiosque.

**Os feitos são ON e OFF, nunca o alternar.** `full-application-on` e `full-application-off`
(`fullscreen enable|disable application`, em `assets/compositor-deeds.nix`). Nenhuma resposta e
nenhum evento do compositor dizem esse estado, então o rodapé guarda uma cópia por par (`kiosk::Ui`)
e diz qual quer, pelo raciocínio do `overview-all-on/off`. O único lugar que diz ON ou OFF é
`Footer::kiosk_bars`. Duas coisas mexem na cópia além dos comandos do rodapé: uma tela cheia de
verdade (vídeo HTML5) que acaba leva o estado junto, e o rodapé o põe de volta; e o F11 dentro do
Chrome, que o rodapé SEGUE em vez de brigar com a pessoa. A guarda: depois de pedir as barras
escondidas, um relato `normal` que chega antes de o Chrome dizer `fullscreen` é a foto velha, e não
um F11. Pelo mesmo motivo, o acorde `M-f` (`wm-full-application`) passa pelo nosso binário, como a
seleção e o `S-TAB`: ligado direto no compositor, ele viraria as barras pelas costas da cópia.

**Um caminho só, com o Chrome aberto ou fechado.** `kiosk-open [endereço ou palavras]` roda

```
chrome --profile-directory=<p> --new-window chrome-extension://<id>/kiosk.html#n=<nonce>&p=<p>&u=<endereço>
```

(`ronoctua_browser::launch::argv`). Com o Chrome aberto, o processo dele entrega a linha ao
navegador que já roda; fechado, a mesma linha o sobe. O id da extensão nasce na máquina, então o
rodapé o lê do manifesto do host (`RONOCTUA_BROWSER_MANIFEST`, no ambiente da unidade do rodapé). A
outra porta é `br.ronoctua.kiosk.desktop`, sem `MimeType`: abre um endereço quando pedem, e nunca
toma os links do navegador padrão.

**O par nasce de duas metades, nunca de tempo.** A página `kiosk.html` põe o nonce no título, que o
compositor relata (só de janela `google-chrome`), e diz `kiosk-born` à extensão, cujo estado chega
ao rodapé pela ponte. As duas chegam em qualquer ordem. Com as duas, o rodapé esconde as barras,
manda `Go`, e a página faz `location.replace` para o endereço. O par fica em `kiosk.json`, no
diretório de runtime, para um rodapé que reinicia continuar sabendo, e o arquivo é conferido contra
cada lista inteira de janelas (pelo id, ou reapontado pelo nonce num título). Um lançamento sem as
duas metades em 20 s (`kiosk::PATIENCE`) vira janela comum, e a linha diz por quê; um lançamento que
falhou antes de haver Chrome só diz a falha dele, porque nenhuma janela abriu. Uma página que o
rodapé não vai parear (esse prazo, ou um rodapé que reiniciou no meio do lançamento) recebe `Go` e
`Release`: sem o `Release`, a extensão continuaria contando a janela como quiosque, com a porta, o
host e o worker vivos por ela. Sem rodapé (o host diz `footer up:false`, ou a porta falha), a página
vai ao endereço na hora; um temporizador de 30 s, armado só enquanto uma página espera, cobre a
página que ninguém lançou. Uma página recarregada ou restaurada também vai direto.

**Palavras esperam a aba em que o quiosque cai.** `kiosk-open duas palavras` sem quiosque aberto
lança sem endereço, e o `Go` sem endereço fecha a página de nascimento no lugar da página de aba
nova do Chrome. A busca não vai para a aba que está indo embora: as palavras ficam no par, e o
próximo estado que mostra uma aba que não é a de nascimento as recebe (`kiosk::born_on`).

**A ponte são três saltos com um vocabulário só** (`ronoctua_browser::wire`). A extensão fala com o
`ronoctua-browser` pelo enquadramento do Chrome (4 bytes de tamanho e o JSON); o host fala com o
rodapé uma linha JSON por mensagem, e o rodapé é o servidor (`ronoctua[-dev]-browser.sock`). Ele
liga o socket depois de tomar o display (`leader::listen`, onde um rodapé anterior é convidado a
sair), e um servidor que sai só apaga o arquivo se ainda for o dele (pelo inode): ligado antes, o
socket do rodapé antigo, vivo, recusava o novo, e o antigo levava o arquivo ao sair. O estado vai
inteiro a cada mudança, com versão; as ordens levam `serial` e voltam como `done`, com o motivo
quando falham. A extensão obedece uma ordem por vez, na ordem em que o rodapé mandou: lado a lado,
cada uma terminava quando o Chrome respondia, e `[activate X, create]` deixava X na frente. Campo
desconhecido é tolerado, e ordem desconhecida é recusada (`refused-order`). Endereço é só `http`,
`https` ou a página de aba nova, conferido no rodapé, no host e na extensão. Os nomes dos dois hosts
saem de uma tabela só (`nix/browser-hosts.nix`), que a extensão e os manifestos leem.

**A porta nativa só existe enquanto há um quiosque.** Ela mantém o service worker vivo, e os
ouvintes `tabs.*` e `windows.*` só são armados nessa hora: a navegação comum nunca acorda o worker.
Os ids dos quiosques ficam em `storage.local` (o `storage.session` é apagado a cada atualização da
extensão) e são limpos no `onStartup`. Uma janela que fechou enquanto não havia porta sai quando o
próximo estado é montado, e a última leva a porta junto. Um host que acaba com quiosques abertos é
subido de novo depois de uma pausa (1 s, dobrando até 16 s, que fica abaixo do meio minuto que um
worker parado ganha); uma porta que viveu mais que isso zera a pausa. As permissões novas são duas,
cada uma com o motivo no gerador: `management` (só `getAll`) e `search` (a linha de endereço busca
no motor da pessoa). O manifesto continua sem nenhuma das cinco chaves que deixam uma página
detectar a extensão: abrir `kiosk.html` pela linha de comando não precisa de
`web_accessible_resources`.

**O host acrescenta o que só um processo ao lado do perfil lê**: as extensões fixadas
(`extensions.pinned_extensions`, no `Preferences` e depois no `Secure Preferences`) e o popup de
cada uma, no manifesto dela (`ronoctua_browser::profile`). Quando o rodapé some, o host espera o
socket reaparecer por inotify e reenvia o último estado inteiro. Como ele ACRESCENTA a cada estado,
o teto de linha do rodapé é o dobro do quadro que o host aceita, e o host não manda uma linha que o
rodapé recusaria: ela derrubaria a conexão, e a seguinte receberia a mesma linha. Uma conexão que
acaba assim que é feita espera antes da próxima (100 ms, dobrando até 5 s). É um binário próprio,
extraído em `rust.browser` como o da fila: o Chrome o sobe como a pessoa a cada quiosque, e o fecho
dele é um leitor de JSON e um socket.

**Sem conexão, o par continua de pé.** A modeline desenha o segmento `kiosk-offline` com o motivo: a
ponte não está instalada, o Chrome não conectou, ou a versão é outra. Quiosque↔normal continua
respondendo, porque só depende do compositor. Todo caminho que depende da extensão pergunta a mesma
coisa que a marca (`Kiosks::live`): com a versão diferente, nenhum comando manda ordem, a vista diz
que o Chrome não está conectado (inclusive nas extensões e no zoom, em vez de "nenhuma" e "100%"), e
o `kiosk-open` diz isso em vez de lançar um segundo quiosque tão surdo quanto o primeiro.

**A modeline do quiosque**, da esquerda para a direita: o `mode` (`google-chrome`); voltar,
avançar e recarregar; `+N`; as abas vizinhas com o título curto (`space.kiosk.tabCells`) e a da
frente (até `activeCells`, não removível, em `text.primary`; as outras em `text.secondary`, nunca no
acento); `+N`; aba nova; as extensões fixadas uma a uma, pelo nome curto (`extensionCells`); a
lista de extensões; o zoom (com a porcentagem quando não é 100%); a linha de endereço;
quiosque↔normal (o glifo diz o que o clique fará); e o `…`, que abre a vista. Numa aba vizinha, o
clique a traz para a frente; em qualquer aba, arrastar muda o lugar, o botão direito abre a lista
nela, o do meio a fecha e a roda troca de aba. No zoom, a roda muda o zoom. O piso da linha é o
`mode` mais a aba da frente elidida.

**A linha de endereço é uma linha nossa**, acima da modeline: a terceira ocupante da linha do
relance e do volume. Os dois que vêm e vão tomam a linha e a devolvem, com a mesma altura, então
nada se move. Ela mostra o host inteiro e elide só o caminho, no meio, para um subdomínio enganoso
não se esconder, e o clique edita o endereço no minibuffer. Quando nem o host cabe (tela estreita ou
cortada), sai o COMEÇO dele: o fim é o domínio que diz de quem é a página, e nada passa da borda. Só
aparece com as barras escondidas: no modo normal, a omnibox do Chrome está lá. Ela nasce e some com
o foco, o estado do Chrome e as barras, e todo caminho que mexe nisso já mudou o estado quando o
rodapé olha; por isso a altura é conferida contra a que foi APLICADA (`address_row_applied`), nunca
contra uma leitura de "antes".

**A vista existe só no rodapé** (`kiosk_view.rs`): uma janela tiraria o foco do quiosque que ela
controla, e todo comando age na aba da frente. A grade tem a gramática do menu da janela, e a
célula `B` do menu da janela de um quiosque a abre (`LEADER w B`); em `Footer::view()`, ela fica
logo acima do menu da janela. As listas de abas e de extensões
guardam o id do item, nunca a linha, porque o Chrome reordena por baixo do cursor.

**`kiosk.rs` é puro.** Ele devolve `Errand`s, e o rodapé os executa (`Footer::kiosk_errands`).
Isso o deixa testável sem Chrome e impede os métodos do rodapé de se perguntarem em anel
(`the-footer-does-not-ask-itself`).

**Decidido aqui, e pode mudar:** a adaptação da modeline nasce ligada e a linha de endereço
desligada; o clique do meio fecha a aba; as abas vizinhas mostram o título curto.

#### Medido com o Chrome de verdade

O mecanismo foi medido antes de o rodapé ser escrito, numa VM com o Chrome 151 real
(151.0.7922.71), o Scroll pinado e a extensão instalada pela política, offline, num tile de
512×768:

- **`fullscreen enable application` esconde abas e barra.** O `innerHeight` da página foi 768,
  igual à janela, contra 681 sem ele: 87 px de barras. O `disable` devolve os 681, e o
  `windows.get().state` passa de `fullscreen` a `normal` e volta. A alternativa desenhada para o
  caso de falha (a extensão pedindo `windows.update({state:'fullscreen'})`) não foi preciso.
- **O Chrome mostra o aviso "para sair da tela cheia, segure Esc"** por alguns segundos depois de
  cada `enable`.
- **O F11 é invisível ao IPC.** F11, ou segurar Esc, dentro do quiosque faz o próprio Chrome sair:
  as barras voltam (681), o estado vira `normal` e a árvore do compositor não muda
  (`fullscreen_mode` continua 0). É por isso que o rodapé segue o estado que a extensão relata. F11
  numa janela comum pede tela cheia, e o compositor faz a de espaço de trabalho (`fullscreen_mode`
  1, a saída inteira).
- **Não é preciso inibidor de atalhos.** Um bind do compositor disparou com a janela comum, na tela
  cheia de aplicativo e depois do F11: o Chrome não pede o inibidor, e nenhuma regra nova de janela
  foi escrita.
- **A instalação forçada leva uns 3 s, offline, por `file://`** (3,3 s, 2,7 s, 3,3 s e 2,7 s em
  quatro partidas da VM).
- **A página abre pela linha de comando** (E4): `--new-window chrome-extension://<id>/kiosk.html#…`
  abre com o Chrome aberto (0,54 s até o título) e com ele fechado.
- **O Chrome chama o host com a origem** (E5): `["…/bin/ronoctua-browser",
  "chrome-extension://<id>/"]`, e o host da fila do mesmo jeito. O ambiente tem `XDG_RUNTIME_DIR` e
  `WAYLAND_DISPLAY`, e não tem `RONOCTUA_INSTANCE`.
- **O título chega antes da navegação** (E9): o IPC viu `<nonce> - Google Chrome` antes de a página
  sair.
- **O host não morre parado**: depois de 150 s sem nada, ele continua vivo e responde.
- **Com o host de verdade e um rodapé de mentira**: o par nasceu; o estado chegou com o popup de uma
  extensão; o host recusou `javascript:` (`refused-url`) sem repassar; um rodapé reiniciado foi
  achado e recebeu o estado em 0,02 s; fechar o quiosque encerrou o host; sem rodapé, a página foi
  ao endereço em 1,2 s.
- **O zoom do Chrome é por site**: duas abas do mesmo host foram juntas para 110%.

O `tests/kiosk-vm.nix` repete como asserção o que importa: a instalação pela política, um download
chegando à fila, o par, as barras (pelo menos 60 px entre um estado e o outro), abas e zoom, o
rodapé reiniciado e o host saindo com o quiosque. O Chrome entra nele pelo nome, o único não livre
de um teste, pelo precedente do `uade`; ele fica no `most`. Lá a altura da página se compara com o
CONTEÚDO da janela (`window_rect`), e não com o `rect`: na sessão de verdade o contêiner leva a
borda de 2 px (`default_border pixel 2`), que o Chrome não preenche. Com as barras escondidas o
`innerHeight` foi 695, igual ao conteúdo; com elas, 608 (os mesmos 87 px da medição).

#### Os quatro defeitos que o quiosque expôs

- **O host da fila morria no Chrome de verdade.** O Chrome passa a origem `chrome-extension://<id>/`
  como primeiro argumento, e o manifesto não pode acrescentar uma palavra nossa; o
  `ronoctua-transfers` só aceitava `browser`. O `extension-vm` escondia isso passando `browser` à
  mão. Agora os dois hosts aceitam a origem (`ronoctua_browser::native::caller`), e o
  `extension-vm` os chama como o Chrome chama.
- **A extensão nunca atualizava.** A versão estava congelada em `1.0.0`, e o Chrome só troca uma
  extensão forçada por uma versão MAIOR: o mesmo número com arquivos novos é ignorado, calado, para
  sempre. Agora a versão é `<base>.<n>`. O `base` fica em `nix/extension-version.nix`; o `n` é um
  contador da máquina, guardado ao lado da chave (o caminho no store empacotado por último e o
  contador), que sobe cada vez que a unidade empacota outra build. Toda mudança em `extension/`, nos
  textos ou nas cores dela chega como versão maior, sem passo humano. Um hash das fontes não tem
  ordem, e a hora do commit anda com commits que nem tocaram a extensão.
- **A instalação forçada nunca podia ter funcionado.** `/var/lib/ronoctua-browser` era `0700 root`,
  e o Chrome roda como a pessoa: ele não entrava no diretório do `update.xml` e do `.crx`, e
  esperava para sempre sem log (na primeira partida da VM de medição, nada foi instalado em 120 s).
  Agora o diretório é `0755`, e a chave se protege pelo próprio modo (`0600`).
- **O Chrome que o rodapé abria procurava um X.** O `--ozone-platform-hint=auto` escolhe o Wayland
  pelo `XDG_SESSION_TYPE`, e não pelo `WAYLAND_DISPLAY`. O gerenciador de usuário recebe o
  `XDG_CURRENT_DESKTOP` do `sessionVariables`, mas não o `XDG_SESSION_TYPE` ao lado dele, e o
  compositor não o define; a linha `import-environment` do config dizia "Environment variable
  $XDG_SESSION_TYPE not set, ignoring" e não importava nada. Todo programa aberto pelo rodapé (o
  quiosque, os web apps, o Chrome do lançador) herdava a sessão sem tipo: o Chrome escolhia o X11,
  não achava `DISPLAY` (que ninguém importa) e saía (`Missing X server or $DISPLAY`). A VM de
  medição passava o tipo à mão e não viu. Agora a unidade do compositor tem
  `XDG_SESSION_TYPE=wayland` no ambiente, e a linha do config o leva ao gerenciador; o
  `kiosk-vm` confere que ele chega ao gerenciador e ao rodapé.


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

**A quarta pasta não existia, e as capturas estavam caindo fora da distro.** Nada declarava onde uma
captura pousa, então ela ia para o padrão de quem a tirava — um caminho que contradizia o próprio
`XDG_PICTURES_DIR` desta distro (`main/media/image`) e que nada criava. Uma barra apontando para uma
pasta de capturas apontaria para o vazio para sempre. Hoje `screenshot` é a décima entrada da tabela,
o rodapé monta o nome a partir dela (`RONOCTUA_SCREENSHOTS`, porque o compositor não tira foto
nenhuma — quem tira é o `grim`), o `xdg.userDirs` a CRIA (o `createDirectories` cobre o
`extraConfig`), e
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

### A dica que se explica, e a linha que não pode crescer

A linha de dicas ensina quarenta teclas em três palavras cada, que é tudo o que uma linha guarda. O
dono pediu a outra metade: *"ao colocar o cursor por cima de qualquer atalho do minibuffer, de
qualquer coisa da ronoctua-distro, quero que apareça uma linha abaixo dos atalhos explicando com
mais detalhes o atalho que está sobre hover"*.

Duas coisas nisso são decisão e não detalhe.

**Onde a frase mora.** Ela é `cmd-<id>` para os ids que são comandos do registro — cento e dois de
cento e sessenta e dois já tinham uma — e `about-<id>` para os que são teclas de uma VISTA:
`files-preview`, `notif-read`, `transfers-pause`. Escrever `cmd-files-preview` diria que o `C-v` do
gerenciador é um comando registrado, que é exatamente o que `cmd-` significa. Nunca as duas, porque
duas frases para uma tecla são dois lugares para manter em dia — e o check que cobra isso achou,
ao ser escrito, **dez ids sem frase curta nenhuma**: oito das dez teclas do Syncthing e duas do
idioma desenhavam `[syncthing-down]` na tela, o marcador de chave faltante, nas duas portas.

**De onde vem a linha.** Do próprio quadro anterior (`hint_marks`), pela regra que o `band_placed` e
o `modeline_placed` já seguem: o que o ponteiro está em cima tem de ser o que foi DESENHADO. A linha
é construída dentro do quadro a partir de uma dúzia de empréstimos do rodapé, e um segundo
construtor para o ponteiro perguntar seria a mesma tabela mantida duas vezes — que discordariam na
primeira tecla que um contexto retirasse.

**E a linha SAI DA REGIÃO, nunca da altura do rodapé.** Este é o desenho inteiro. O rodapé é
ancorado embaixo: uma linha acrescentada sob o minibuffer deixa a superfície mais alta, e tudo o que
está acima da linha nova sobe uma linha na TELA. O minibuffer está acima dela — então a dica
escorregaria de baixo do ponteiro que está em cima dela, o hover acabaria, a linha sumiria, a dica
voltaria para baixo do ponteiro, e roda. É o laço de layout que o `Fold::held` descreve por inteiro,
com o ponteiro no mesmo lugar da corrente. Então a linha é paga com uma linha da região de baixo:
`grid()` desenha uma linha a menos enquanto a explicação está de pé, `wanted_height` soma a dela, e
os dois termos se cancelam. A listagem fica uma linha mais curta enquanto o ponteiro descansa numa
tecla, e nada acima dela se mexe um pixel. Uma região de UMA linha não oferece a frase: tirar a
única que ela tem faria a altura crescer de qualquer jeito.

Nas dez janelas o mesmo desenho custa menos, e por isso elas compartilham um `chrome::Hinted`: uma
janela não está ancorada no fundo da tela e simplesmente mostra uma linha a menos de lista.

**E o cabeçalho de uma janela ABRE, como o minibuffer.** A linha de dicas é CORTADA na largura que
recebe — `hint_spans` elide, não quebra —, e a tabela da vista de rede tem 174 células em inglês
contra uma janela que o compositor põe em metade de uma saída. Então mais da metade do que aquela linha
ensinava era inalcançável em cada uma das dez portas: uma reticência onde parava, e nada para
apertar. O rodapé abre a sua desde que o `crate::fold` foi escrito, e o gerenciador abre a dele; as
dez nunca ganharam nenhuma, e o dono achou isso na janela de idioma e região — *"a linha do header
com os atalhos não tem a feature com o ícone de chevron para expandir/recolher o header"*.

O desenho é o do rodapé, inteiro: o chevron na borda direita quando a linha não cabe (e também
enquanto está aberta, senão o gesto que fecha some no instante em que o conteúdo passa a caber),
o resto quebrado sob ela até `space.modeline.foldLines`, uma barra quando ainda sobra, e as teclas
que a percorrem ensinadas DENTRO dela. A tecla é o `Ctrl` DIREITO com `↑↓` — a única família de
modificador livre em toda vista por construção, rastreada pelo keysym porque o xkb funde os dois
`Ctrl` num bit só —, e `Esc` fecha a gaveta antes de fechar a janela.

**O que muda em relação ao que havia é que nada disso é de nenhuma porta.** Eram dez cópias de
`if let Some(text) = message … else … hint_spans(…)`, cada uma com a sua chance de responder
diferente, e a gaveta não estava em nenhuma delas. `chrome::Hinted::head` é o desenho, e o mesmo
`Hinted` responde o ponteiro (`pressed`, `dragged`, `turned`), o teclado (`took_a_key`) e a
medição de altura (`settled`) — um controle é pego onde é desenhado, e aqui isso vale para onze
portas de uma vez. O gerenciador segue com a `fold::Fold` própria porque o controle dele está no
anel do `Tab`; o `every-window-header-opens` PROVA essa exceção em vez de a isentar.

### O nome que não cabe — três caminhos, e um deles anda

Uma listagem é uma grade, e grade corta. O dono pediu o resto de um nome cortado por três caminhos,
e os três são deliberadamente diferentes:

- **o ponteiro** põe o nome inteiro no minibuffer, pelo canal do `explain` que a permissão, o dono e
  a data já usam — e só quando ele FOI cortado, porque explicar um nome que se lê inteiro gasta a
  única linha do rodapé em toda linha de toda listagem;
- **o teclado** o ECOA por três segundos ao chegar nele (`Step::Say`, o `ECHO_SECONDS` que as duas
  portas já dividem);
- **o desenho** o faz andar de lado, *"em uma animação de movimento, de tempo em tempo"*.

O andar é `slide.rs`, e é uma função PURA de uma duração: um segundo parado, uma célula a cada 200
ms até o fim, um segundo parado, e volta ao começo de uma vez — a forma que o dono escolheu quando
perguntei (*"passo a passo, com pausa nas pontas"*). Pura porque então o relógio é da PORTA e a
animação é aritmética: as duas portas desenham esta listagem e nenhuma delas pode ter ideia própria
de onde o nome chegou. Uma célula por vez porque é o que a mantém uma grade — movimento sub-célula
exigiria desenhar o nome num deslocamento em PIXELS, que é um segundo jeito de posicionar texto numa
superfície onde tudo o mais é posicionado em colunas.

O relógio é ARMADO, o único padrão que este repositório já abençoou como exceção: sobe quando um
nome começa a andar e cai sozinho quando nenhum anda. Cinco vezes por segundo é mais rápido do que o
segundo que o rodapé já acorda, e um temporizador permanente de 200 ms acordaria o laço a sessão
inteira para que um nome pudesse se mexer nos poucos quadros em que alguém descansa sobre ele.

**Um nome anda por vez** — o do ponteiro, ou o do cursor quando o ponteiro não está sobre nenhum.
Dois seriam duas coisas se mexendo numa tela onde nada se mexe, e o olho está num dos dois.

### O modo miniatura — uma segunda forma da listagem, e nenhuma segunda aritmética

Uma listagem responde *onde está*; uma grade de miniaturas responde *qual é*. São a mesma lista com
duas formas, e a segunda nasce desligada (`C-g`, ou o glifo clicável no fim da linha do caminho).

**O problema não é desenhar figuras: é que uma linha deixa de ser uma fileira.** Dezoito lugares
deste arquivo transformavam uma linha de tela num índice de entrada e de volta, e todos estavam
certos enquanto uma entrada ocupava uma linha. A tentação é espalhar um `if grade` por todos eles, e
é assim que dois deles passam a discordar.

O que foi feito em vez disso tem três peças e nada mais:

1. **`Section` ganhou UM campo** — `grid: Option<ronoctua_design::Grid>`. `None` é exatamente o
   comportamento de antes.
2. **`offset` e `total` continuam em ENTRADAS.** A roda, o arrasto, o `follow` e o
   `scroll_listing_to` já falam entradas; dar a eles uma segunda unidade é como um número passa a
   significar duas coisas. O deslocamento é travado num múltiplo da prateleira na LEITURA, que é o
   arranjo que o `facts_offset` já tem e pelo mesmo motivo: o que o desalinha não é uma escrita, é a
   GRADE mudando embaixo dele.
3. **As duas conversões viraram dois métodos, um o inverso do outro** — `Section::line_of` e
   `Section::row_at` —, e os dois delegam ao `Grid`. É a mesma soma nos dois sentidos, que é
   literalmente o que o `ronoctua-design` escreveu no topo do `grid.rs`: os três navegadores de
   ícones tinham cada um a sua, e o sintoma era *"o clique cair num item enquanto a tela mostrava
   outro"*. Numa listagem, esse sintoma apaga o arquivo errado.

**As linhas continuam sendo linhas.** Uma prateleira de `down` linhas produz `down` `Row`s, com os
nomes na última delas. É isso que mantém o `expansion_window` achatando uma linha por linha, os dois
pintores caminhando o `content` como sempre, e o `Field` sem uma linha própria.

**E a célula responde no vão.** `View::hit` terminava em `field_at`, que exige um CAMPO; uma
miniatura é quase toda célula vazia com uma figura pintada por cima pela porta. Sem um braço de
aritmética ali, o hover e o clique responderiam "lugar nenhum" na maior parte de cada célula —
exatamente o defeito que o dono reportou uma vez sobre o botão de expandir, multiplicado por sessenta.
A faixa, pela mesma razão, deixou de ser uma linha de largura total e passou a ser o RETÂNGULO da
célula: uma faixa da largura da coluna acende sete arquivos para dizer que o ponteiro está sobre um.

**A animação tem uma regra só, e ela é medida.** O que anima DENTRO do processo anima sozinho; o que
precisa de um processo anima sob o ponteiro. GIF, WebP animado e APNG têm um iterador preguiçoso
dentro do `image` — um descritor e um quadro cada, 2,35 ms por quadro medidos. Vídeo custa um ffmpeg.
A fronteira não foi escolhida: é a única que existe. E o número que ela evita foi medido também —
cinquenta GIFs com os quadros decodificados de antemão são **215 MiB**, que é o que o dono pediu para
isto não custar, com essas palavras.

**O custo de uma miniatura é o PROCESSO, não o arquivo.** Medido contra a pasta do dono, 64 arquivos
e 36 extensões: 109 ms para um JPEG de 22 KB e 167 ms para um MP4 de 355 MB. É por isso que o
trabalho é um pool e não a thread única da prévia — aquela esvazia a fila até o pedido mais novo, o
que é certo para um painel sobre um arquivo e jogaria fora 63 das 64 células — e é por isso que
existe um cache em disco: a segunda visita a uma pasta não pode repagar oito segundos de processo.

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
`AGENTS.md` diz por quê — **uma derivação referencia o que os BYTES dela citam**. O crate separado
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

##### Expandir tabulação é de UMA função, e o leitor de documentos não a chamava

`text::flatten` abre o tab até o próximo stop e tira o `\r`. Ela era privada deste módulo, e
`document::lines_of` montava as linhas dele sem ela — duas réguas para a mesma pergunta, e esta com
consequência visível. O painel orça em CÉLULAS (`ronoctua_protocol::cells` chama o tab de UMA) e o
shaper o leva ao próximo stop, então uma linha com um tab dentro era medida cabendo e desenhada
vários cells mais larga, **por cima da coluna ao lado**. O dono fotografou: o `sample.rtf` dele tem
uma lista indentada por `\tab`, e ela atravessava a prévia e cobria os fatos. Um `.rs` nunca teve o
defeito, porque este leitor achata desde o dia em que foi escrito.

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
rodava, e nada na tela dizia. Agora a leitura é `poll` com tique de 20 ms (`frames::TICK`), e o
prazo e a `era` são perguntados a cada volta.

E o filho morre por uma **guarda de `Drop`**. Estava escrito à mão em quatro saídas, e a quinta que
alguém acrescentasse amanhã vazaria um ffmpeg lendo um filme que ninguém está olhando; um `Drop`
cobre `return`, `?`, `break` e pânico, e não sobra onde esquecer. Mais `process_group(0)`, o idioma
do `transfers/media.rs`, para o sinal alcançar o grupo.

Desde o estúdio o leitor e a guarda não são mais do player: moram em `ronoctua_camera::frames`, um
só para todo leitor de quadros da árvore — a câmera, as fontes do estúdio e este miniplayer, que é
onde as duas armadilhas foram aprendidas. O `motion-never-leaks-a-child` pergunta dos dois lados: o
player entrega a leitura e o filho a eles, e eles ainda fazem `poll` e ainda matam.

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

#### O GPS é mostrado, e a escolha de esconder era uma escolha errada de LUGAR

`Place::Withheld` foi o padrão por um ano. O argumento está preservado aqui porque ele não era tolo:
a ficha é o que alguém abre pouco antes de mandar uma fotografia para um estranho, e uma tela
compartilhada, um projetor ou alguém em pé atrás transformam *"onde isto foi tirado"* numa revelação
que ninguém pediu — quase sempre a casa onde foi tirada.

O que o argumento errava é **de quem é a decisão**. A ficha não é um canal: ela mostra o que o
arquivo tem, para quem já tem o arquivo. Quem vê a tela pode abrir o mesmo arquivo com qualquer
outro programa e ler a mesma coordenada — esconder não protegia nada, só fazia desta a única
ferramenta da máquina que **mente por omissão** sobre o que está lendo. E a linha que dizia "está
escrito aqui dentro, mas não vou mostrar" era a pior das três opções possíveis: dava o susto sem dar
a informação, e obrigava a pessoa a sair do gerenciador para descobrir o que o próprio gerenciador
acabara de dizer que sabia.

O dono removeu a regra por instrução explícita: *"tudo que é tipo de informação que o arquivo prover
DEVE ser mostrado na coluna de info"*. Então a coordenada é um fato como os outros — cinco decimais
(cerca de um metro, que é tudo que um telefone sabe), o sinal como hemisfério, e
`files-value-coordinates` nos dois idiomas.

**O que MUDA de verdade é a escala, e isso fica escrito:** cinco decimais numa linha passam
despercebidos numa tela compartilhada; um globo apontando para a casa de alguém, não. O lugar de
resolver isso é um comando que TIRA a coordenada do arquivo antes de enviá-lo — que não existe hoje
e está no `docs/PENDING.md` com essa frase.

### O ponteiro toma vários, arrasta e solta — e o que o gesto NÃO inventou

O pedido do dono foi por *"total suporte ao mouse na versão thumbnail da listagem"*, e na conversa
que abriu o plano ele estendeu o escopo três vezes: arrastar-e-soltar entra, inclusive entre janelas
e com programas estrangeiros; o retângulo é desenhado POR CIMA das miniaturas, com alfa; ele vale
para as DUAS formas da listagem; e o segundo botão sobre um item NÃO marcado limpa as marcas e age
naquele item.

**Nada disso é um vocabulário novo.** Marcar é o `mark_row` que o `m` do teclado já usava, limpar é
o `C-A`, e o retângulo cruza a MESMA aritmética que desenha as células (`Grid::at`/`index_at`, uma
soma lida nos dois sentidos). O que precisou existir foram três coisas, e cada uma paga a própria
linha:

**Uma função diz o que o retângulo cobre, e duas a leem.** `View::sweep_box` responde em células e
linhas da região; o pintor a converte com `files::sweep_px` — o mesmo molde do `band_px` que a faixa
de foco já usa — e a SELEÇÃO sai de `View::swept`, que cruza esse retângulo com o de cada entrada.
Escrito duas vezes, o desenho e a tomada divergiriam na primeira edição de qualquer um dos dois, e o
sintoma é um `delete` levando o que a banda não estava por cima. É a lei que as barras deste
gerenciador já obedecem, chegando num gesto novo.

**E ele é desenhado no grupo de CIMA.** Não há teste de profundidade neste renderizador: a ordem é
retângulo → imagem → texto, e nas duas portas os retângulos já saem em DUAS faixas com as figuras
entre elas (`Quads::draw_range`, que é o que põe os controles do filme sobre o filme). O rodapé
desenha as dele; as de uma janela, desde a porta única, o `Door::paint` desenha uma vez para toda
janela, e o `present` do gerenciador corta o grupo de cima do fim da lista e o põe em
`Canvas::over`. Um retângulo empilhado com os outros fundos é um retângulo debaixo de toda
miniatura — invisível numa grade, que é figura de borda a borda, e uma grade é o único lugar onde
ele importa. Medido na bateria de fotos: sobre a mesma coluna, a cor própria das figuras cai de
83 073 para 52 916 pixels enquanto a banda está de pé, que é o número que um retângulo desenhado por
baixo não produz.

**Arrastar é o protocolo, não um gesto interno.** `carry.rs` é a fronteira com o `wl_data_device` do
SCTK e o formato é o `text/uri-list` do freedesktop (RFC 2483, `file://` percent-encoded) — é por
isso que o arrasto atravessa para outra janela nossa e para qualquer programa estrangeiro, nos dois
sentidos, sem que nenhum dos dois lados saiba do outro. Um gesto interno teria sido mais fácil e
valeria só dentro desta janela, que é a metade menos útil do que o dono pediu.

**E um cliente que é as DUAS pontas não pode ler o próprio cano.** `wl_data_offer.receive` devolve
uma ponta de leitura e pede ao COMPOSITOR que mande a fonte escrever; quando a fonte é este mesmo
processo, essa ordem chega como um EVENTO Wayland, e ler dentro do `drop_performed` bloqueia
exatamente o laço que a entregaria. Medido, nessa ordem: `dropped=0`, e um décimo de segundo depois
`the drag's list could not be written error=Broken pipe`. Não havia nada errado com a codificação
nem com o protocolo — um cliente de uma thread que é as duas pontas tem de pegar o caminho curto, e
ele já tem a resposta na mão (`Carrying::offering`). De fora, o cano é lido com prazo, que é o que
faz um escritor estrangeiro lento custar um quarto de segundo em vez da sessão.

**E um destino de drop é uma LINHA, nunca um campo.** Esta é a única coisa que a entrega descobriu
na TELA e não no teste: `View::hit` acaba em `field_at`, e uma PASTA não tem tamanho para desenhar,
então depois do nome dela a linha não responde nada. Na bateria de fotos a pasta nunca acendeu e o
arquivo caiu no vazio, enquanto o ARQUIVO uma linha abaixo respondia em toda a largura — porque um
arquivo tem tamanho. É o mesmo defeito que as miniaturas já consertaram um painel adiante
(`Section::row_at`), e aqui o que se mira por três caracteres de nome é para onde os arquivos de
alguém vão. `View::row_under` pergunta à SEÇÃO, que é aritmética e não tem o que deixar de
responder, e a linha que ela calcula sai de `row_of_line` — a MESMA função que o `hit` usa, para que as
duas nunca discordem sobre qual linha é a primeira entrada. Hover e clique não mudaram: eles são
sobre um campo, e o que o olho vê aceso continua sendo o que o clique pega.

**O que ficou de fora está no `docs/PENDING.md`**, com o motivo de cada um: a figura que acompanha o
ponteiro, rolar ao encostar na borda, renomear pelo segundo clique e o retângulo que alterna em vez
de tomar.

### O globo — três camadas, e nenhuma delas é do gerenciador de arquivos

O dono foi explícito ao recusar a primeira versão do plano: *"o globo, o suporte a coordenada etc.
serão utilizados por outros APPs que eu criarei, então tenha certeza de que isso não seja uma
feature criada específica e exclusivamente para o filemanager"*. O molde já existia no repositório,
escrito para outro assunto: *"o realce de sintaxe é um crate, e não é de app nenhum … devolve PAPEL,
nunca cor"*. O globo é a mesma divisão um assunto adiante.

| camada | o que sabe | o que não pode saber |
|---|---|---|
| `ronoctua-atlas` — a tabela assada, `address(...)` e a projeção | geometria e PAPÉIS (`Ink`) | cor, idioma, porta, token, Wayland. `no_std`, zero dependências |
| `ronoctua-atlas-packs` — onde um pacote mora, como se assa um e o que ele responde | disco e shapefile | desenho: não depende do crate acima, e o `one-zip-reader` cobra |
| `shell/globe.rs` + `chrome::ink_color` | como aquilo vira pixel NESTA máquina | qual vista pediu — é o `Owner` que diz |

O gerenciador é o PRIMEIRO chamador e nunca o dono. Três outros já seguram coordenada hoje e não
tinham o que fazer com ela: o relance (`assets/world.nix` guarda latitude e longitude por lugar), o
`LEADER s l` (que PERGUNTA uma coordenada a uma pessoa) e o instalador, cujo próprio comentário diz
que joga fora *"as coordenadas e os códigos de país de um mapa que este instalador não desenha"*.

#### O horizonte é a única geometria de verdade nova

Projeção ortográfica sobre uma esfera girada é aritmética de colégio: `z > 0` é o hemisfério
visível. O que não é óbvio é o recorte. **Descartar os vértices invisíveis liga o último visível ao
próximo em linha reta e desenha uma corda ATRAVÉS do planeta** — o sintoma é a África ganhar um lado
reto, e isso se lê como dado ruim, não como recorte errado.

São duas metades: cortar cada ARESTA no horizonte (bissecção pelo grande círculo, oito vezes, que é
menos de um pixel) e **andar pelo ARO** entre uma saída e a entrada seguinte, no sentido que mantém
o interior dentro — decidido uma vez por anel. É isso que fecha a África quando metade dela foi para
trás.

E o preenchimento é `EvenOdd`, nunca `Winding`: anel interno não vem marcado no dado, então com a
regra de rotação o Cáspio vira terra. Cobra o `atlas-fill-is-even-odd`.

#### Zoom é ESCALA e pan é CENTRO, e confundir os dois tira o globo da tela

O zoom multiplica o raio; o pan move o centro. Se o passo do pan não dividir pelo zoom, um globo
aproximado sai da tela num único gesto — e perto do polo ele também divide por `cos φ`, senão
arrastar parece preso em vez de engrenado.

#### A figura mora nas primeiras linhas do painel, e o campo para isso já existia

`Section::above` quer dizer "quantas das primeiras linhas são chrome e não rolam", e era usado só
pelo cabeçalho da listagem. O globo é exatamente isso virado para o outro lado: N linhas REAIS, com
campos reais (o alvo do ponto, o endereço, o alvo do Google Maps), então `hit`, a faixa, o clique e
a tecla continuam passando pela máquina que já existe.

**E elas ROLAM.** Eram `Section::above` — fixas —, e isso custou duas coisas que o dono achou com a
mão: banda fixa deixa o trilho do polegar (`Section::visible`) mais curto que o painel enquanto o
ARRASTO divide pelo painel (`Room::panel`), então pegar a barra num arquivo com globo a jogava para
longe do cursor; e a figura custava seis das doze linhas do rodapé enquanto o arquivo estivesse sob
o cursor. Rolando, ela devolve as linhas, e foi isso que aposentou o `C-g` — a tecla que existia
para comprar essas linhas de volta. **A figura é CORTADA no topo, nunca encolhida.** Nada neste
renderizador recorta uma imagem, e uma imagem meio fora do painel seria desenhada POR CIMA da
listagem; então a thread projeta o globo INTEIRO e pinta só o que sobra abaixo das linhas que
rolaram (`Sight::from`, `painted`), com o ponto e as marcas subindo junto. Encolher — reprojetar o
globo na caixa menor — mudaria o que ele MOSTRA a cada linha rolada. O corte está na chave da
textura: sem ele, um globo rolado uma linha receberia de volta a figura da linha de antes.

**E `globe_lines` pergunta ao SUJEITO, nunca ao desenho.** Por uma rodada ele perguntou se o
componente estava APONTADO, e isso é um impasse sem sintoma: quem aponta o componente é a porta,
quando `globe_px` devolve uma caixa, e `globe_px` recusa caixa de menos de duas linhas. Zero linhas
→ nenhuma caixa → ninguém aponta → zero linhas, para sempre, em toda máquina. A tela mostrava uma
foto com coordenada e um painel de fatos comuns, que se lê como arquivo sem coordenada.

#### A chave da textura tem CINCO partes

Dono, assunto, tamanho, o corte do topo e tema. Sem o dono,
duas vistas com a mesma coordenada no mesmo tamanho e tema resolvem para UMA chave, e como as duas
reescrevem a textura a cada quadro elas piscam entre duas poses sem nada em log nenhum. Por isso o
`Owner` nasce com a variante antes de ela ser alcançada (`Atlas`, e agora `Player`) — e a era do
desenho é POR DONO (`[AtomicU64; GLOBES_AT_ONCE]`), senão arrastar um globo cancelaria o quadro do
outro para sempre. A pose, o conjunto de marcas e a marca acesa são pintados DENTRO da figura e
ficam FORA da chave, de propósito: o globo é figura que se move, e as duas portas o sobem a cada
quadro sob a mesma chave, como um filme. Uma chave por grupo aceso seria uma textura nova do
tamanho do globo a cada grupo que o ponteiro cruza, e passado o teto do renderer o `dump` jogaria
fora toda textura que o quadro não segura — as miniaturas e os ícones fora da tela junto.

#### As marcas: grupos pela distância na tela, pegos onde foram pintados

O player põe marcas num globo (países e estações; a cidade de um canal de TV ainda fica na âncora do
país) e o atlas as agrupa pela distância na tela (`atlas.markMerge`), com o raio pela faixa do total
(`atlas.markTier1..4`). A vista entrega um `MarkSet` com uma `generation` que ela muda a cada
conjunto novo; o quadro que volta da thread traz os grupos já nos pixels da figura cortada — como o
ponto da coordenada — e a geração de que são. `Globe::mark_at` acha o centro mais próximo dentro do
disco e de `atlas.markSlack`, lendo os mesmos números da pintura, e recusa um quadro de outra
geração: os ids do conjunto novo nunca pegam um grupo do velho. O lugar do grupo aceso é procurado
na thread, uma vez por grupo.

**O player é o segundo chamador (`crate::player_globe`).** As páginas de rádios e de TVs do mundo e
as listas de estações e de canais desenham o globo com as marcas que o daemon já posicionou: a
vista monta o `MarkSet` da página (uma geração por lista, por resposta da mesa e por recorte), diz
qual grupo acende — o que está sob o ponteiro, senão o da linha do cursor — e ENQUADRA uma vez
quando as marcas chegam: a página do mundo inteiro, uma lista nas marcas dela. Uma porta sem thread
(a foto, o teste) desenha no lugar pela mesma função da thread (`Globe::drawn_here`). O globo do
player conta como uma figura a mais no teto do renderer (`image::PLAYER_GLOBE`): na janela ele fica
nas linhas ao lado da faixa dos filmes, nunca no lugar dela.

**A âncora de um país fica sobre a própria terra, e isso foi medido.** A âncora é o ponto de rótulo
do Natural Earth, e em onze países ele está fora do país na grade: é onde um mapa escreve o NOME. O
da Nova Zelândia fica no mar ao largo de Taranaki; os de sete países de ilhas pequenas ficam ao lado
das ilhas (AG, GQ, GS, IO, ST, TT, VC); e três países de poucos passos da grade têm o ponto onde a
grade o dá ao vizinho que o leitor encontra primeiro, ou à água (MC, MO, VA — o de Mônaco anda
0,3 km, o do Vaticano 1,8). Uma marca
representa o país, e o clique nela pergunta o país em que o ponto está; então o gerador
(`gen-atlas.py`, `on_land`) leva esse ponto ao ponto da grade mais próximo que o leitor acha dentro
dos anéis do próprio país, e o `every_anchor_is_inside_its_own_country` cobra na tabela assada.
A âncora mora na tabela assada, numa seção própria (`Anchors`: código, longitude e latitude, em
ordem de código), lida por `anchor_of` e `country_name`; a versão da tabela continua 2, porque um
leitor antigo pula a seção que não conhece. O gerador tira o ponto do `LABEL_X`/`LABEL_Y` do Natural
Earth já pinado e recusa a tabela com menos de cem âncoras ou com um ponto fora do planeta.

### As ruas por região — um pacote, uma fila, e um leitor que recusa

O embarcado responde país, estado e lugar mais próximo em qualquer lugar do mundo, em 10,70 MB. A
RUA exige o dado da região na máquina, e foi o que o dono pediu junto: *"suporte completo para que o
usuário possa facilmente ativar/baixar/manter o suporte a ruas por país"*.

**E o catálogo carrega as 555 regiões, não as 526 que dá para baixar.** Vinte e nove delas — Brasil,
Alemanha, França, Rússia, Estados Unidos e todos os continentes — a Geofabrik publica só em
`.osm.pbf`, e um pacote é assado de shapefile. Descartá-las na assadura foi o que fez o dono
procurar o Brasil e não achar nada; de fora, uma linha ausente e uma tecla que não faz nada são
indistinguíveis. Agora a linha existe, recusa, e diz que a região é publicada em PARTES — e as
partes estão na mesma lista, alcançáveis pelo nome do país porque o rótulo delas o carrega
(`Sudeste, Brazil`). É a linha do XFS do formatar, um assunto adiante.

**O que se guarda de um download de 101 arquivos são DUAS camadas e DUAS colunas.** Medido no menor
pacote que a Geofabrik publica (Andorra): 6,3 MB no fio, 13,8 MB abertos, `roads` e `places` são 34%
disso, e 27,8% das 8 799 vias têm nome. Uma via sem nome é uma linha sobre a qual o painel só
poderia dizer "uma estrada", e o lugar é uma resposta melhor que essa.

**Quem baixa e assa é a FILA, num trabalho só.** Duas razões: o `-latest-` é um REDIRECIONAMENTO e o
nosso HTTP não segue um (está escrito no cabeçalho dele), e o nome DATADO para onde ele aponta é a
versão do pacote — a única que existe. Um trabalho só é a forma do `Work::Media`, que baixa e depois
remuxa: dois trabalhos encadeados exigiriam um segundo escalonador, e o estado do meio (um zip que
não é de ninguém) é justamente o que uma fila existe para não deixar.

**O leitor recusa onde não pode conferir.** O `.dbf` casa com o `.shp` por POSIÇÃO e por nada mais —
não há chave em nenhum dos dois —, então um registro a mais desloca todos os atributos em silêncio e
cada via passa a ter o nome da vizinha. Isso **se lê como verdade** na tela, que é o pior tipo de
defeito, e a única detecção possível é a contagem: contagens diferentes são recusa.

**E o pacote só ACRESCENTA.** A rua e o bairro entram no `Address` que o embarcado montou, e o
bairro só substitui a cidade quando está mais PERTO. Um pacote instalado nunca pode piorar a
resposta que existia sem ele.

**A vista `LEADER s g` filtra nas duas portas.** `f`, ou o segundo clique na linha do filtro,
pergunta na linha da própria porta — o minibuffer no rodapé, o cabeçalho na janela — sob o mesmo
`geo::FIND`, e as duas respostas vão para `View::took_a_filter`; colar é o `C-v` da linha, e
enquanto ela pergunta `f` e `d` são letras do filtro. A janela dizia "digitar é no rodapé", e a
lista de quinhentas e trinta regiões que ela existe para ler não se estreitava nela. As duas portas
leem uma tecla pelo mesmo leitor (`geo::press_of_key`), pela notação inteira: `C-f` filtrava nas
duas, porque cada uma casava o símbolo e largava o modificador.

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

### O documento — porque quase todos eles são o MESMO arquivo

A pasta de testes do dono tem onze documentos e a prévia abria um: o PDF. O resto — `.docx`,
`.odt`, `.odp`, `.ods`, `.xlsx`, `.pptx` — caía em "sem prévia", e o `.rtf` era pior que isso: a
base do freedesktop diz que `text/rtf` desce de `text/plain`, o que é verdade, então o leitor de
texto o reivindicava e o painel desenhava `{\rtf1\ansi\deff0{\fonttbl…`. É a lição da playlist
outra vez, escrita nas palavras daquele módulo: **o que decide não é do que um arquivo é feito, é
para que ele SERVE**.

O que tornou isso barato é que os seis primeiros são um formato só: **zip de XML**. Este crate tem
leitor de zip desde o dia em que aprendeu a desenhar um 3MF, e um parser de XML desde o mesmo dia.
Faltavam as vinte linhas que dizem qual membro guarda as palavras — `word/document.xml`,
`content.xml`, `ppt/slides/slideN.xml`. O leitor subiu de `mesh/zip.rs` para `src/zip.rs` quando
ganhou o segundo consumidor: um zip não é uma forma, e um leitor de que duas metades sem relação
precisam não pertence a nenhuma das duas.

**A resposta é `text::Read`**, que é o tipo de um `.rs`. Essa é a integração inteira: o painel já
rola, já pagina, já move de lado, já pendura barra. Um tipo `Document` próprio seria uma segunda
coisa para o painel aprender e uma segunda coisa para as duas portas desenharem. Sem realce —
`language: None` —, porque documento é prosa e prosa não é colorida.

**Planilha vira TABELA**, escolha do dono em três palavras: *"planilha vira tabela"*. O que torna
uma planilha legível num painel monoespaçado é a mesma coluna começando na mesma célula em toda
linha, e um despejo separado por vírgulas é o arquivo e não a planilha. Por isso o `.xls` — que
chega como CSV do `xls2csv` — passa pelo mesmo `align` que o `.xlsx` e o `.ods`; a decisão é sobre
planilhas, não sobre qual biblioteca abriu uma. E a medida é em CÉLULAS (`ronoctua_protocol::cells`):
uma tabela acolchoada por BYTES entorta na primeira palavra acentuada.

Três armadilhas que custaram medição:

- **O container é o ÚLTIMO a ser perguntado.** Todos os seis descem de `application/zip`, então um
  `kind_of` que perguntasse o genérico primeiro responderia "um arquivo compactado" para um
  documento do Word — e o painel listaria `[Content_Types].xml` onde deviam estar as palavras. O
  check lê a ORDEM, e lê também que ele é perguntado UMA vez: um `is("application/zip")` no topo
  deixa o fall-through no fim, e uma verificação que só olhasse a última posição passaria verde.
- **A ordem das transparências não é a ordem dos nomes.** `slide10.xml` vem antes de `slide2.xml`
  como texto; a apresentação lida assim é uma apresentação embaralhada.
- **As strings do `.xlsx` moram noutra parte.** Uma célula de tipo `s` guarda um ÍNDICE para
  `xl/sharedStrings.xml`; sem seguir isso, uma planilha de palavras é uma planilha de inteiros
  pequenos. E uma coluna pulada (`<c r="C1">` logo depois de `<c r="A1">`) tem de continuar pulada,
  ou todo valor escorrega uma coluna para a esquerda.

**E os OLE são outra coisa.** `.doc`, `.xls` e `.ppt` são compound files — um sistema de arquivos
dentro de um arquivo — e nada neles é XML. O `catdoc` traz três binários em 36,6 MiB medidos.

**A medição que estava aqui era FALSA, e custou duas reclamações do dono.** Esta seção dizia que
`catdoc sample.doc` devolvia **8 377 bytes**; esse número foi medido contra um `catdoc` de OUTRO
nixpkgs, não o que o flake pina. Remedido contra o binário que a distro instala, nos arquivos do
dono:

| programa | resposta | saída |
|---|---|---|
| `catdoc sample.doc` | **zero bytes** | **69** |
| `catppt sample.ppt` | zero bytes | 0 |
| `xls2csv sample.xls` | 364 057 bytes | 0 |

Os dois primeiros falham do único jeito que nada mais adiante consegue ver — nada no stderr —, e o
painel dizia *"o que voltou não era uma imagem — o arquivo pode estar quebrado"* sobre um documento
perfeitamente bom. Por isso resposta vazia é **falha** e não documento vazio: painel em branco e
painel recusado são iguais na tela, e só um dos dois tem frase. **A lição é do processo e não do
formato**: um número escrito num comentário é uma medição, e medir contra um binário que não é o
que se instala é não ter medido. Nada de LibreOffice (2-3 GB) e nada de pandoc: a linha que o
`media.nix` já traça é que isto é suporte a FORMATO, não a aplicativo.

**O DOCUMENTO também é lido aqui** (`src/doc.rs`), e o que ele tem de difícil é onde o texto mora:
não num lugar só. O stream `WordDocument` guarda os caracteres, mas em PEDAÇOS, e a tabela que diz
onde cada um começa vive em OUTRO stream — `1Table` ou `0Table`, e qual dos dois é um BIT no
cabeçalho. Um leitor que pegasse de `fcMin` a `fcMac` acerta o Word 6 e erra o 97. Cada descritor
de pedaço carrega um deslocamento e uma marca dizendo se aquele pedaço é UTF-16 ou um byte por
caractere — e quando é um byte, os bytes são **cp1252 e não Latin-1**: uma aspa curva é `0x92` lá e
um caractere de controle aqui, então o caminho fácil transforma todo apóstrofo que o Word escreveu
num codepoint que não desenha nada. O que fica entre `0x13` e `0x14` é INSTRUÇÃO DE CAMPO
(`PAGEREF _Toc123 \h`), que é maquinária e sai — o mesmo juízo que o `rtf::plain` faz sobre o
`\listtext`.

**A apresentação deixou de precisar de programa, e o dono é quem pediu**: *"o preview também não foi
capaz de abrir o arquivo sample.ppt"*. Uma recusa honesta continua sendo um painel sem nada, então o
`.ppt` é lido aqui — `src/ole.rs` abre o compound file e `src/ppt.rs` anda os registros do stream
`PowerPoint Document`. É o argumento do `zip.rs` outra vez: o que uma prévia precisa de um OLE é UM
stream nomeado, e isso é um cabeçalho, uma tabela de setores e uma caminhada. Um crate faria isso
direito e traria escrita, transação e a API de diretório junto.

Três coisas que o formato cobra:

- **Todo registro é oito bytes de cabeçalho**, e o nibble de versão `0xF` é o que diz "aqui dentro
  há outros registros". Dois átomos carregam texto e a diferença entre eles é o ALFABETO:
  `TextCharsAtom` é UTF-16 e `TextBytesAtom` é um byte por caractere, cada byte a metade BAIXA da
  unidade UTF-16 que ele representa.
- **Só as TRANSPARÊNCIAS contam.** O mesmo stream guarda a prosa do slide-mestre — *Click to edit the
  title text format*, e um `*` por nível de marcador — e um leitor que pegasse todo átomo de texto
  poria a instrução do template na tela como se o autor a tivesse escrito. Medido no arquivo do dono:
  **catorze dos vinte e quatro** átomos são do mestre. Texto conta dentro de um container `Slide` e
  em lugar nenhum mais, que é também o que o leitor de `.pptx` ao lado faz — uma apresentação lida de
  dois jeitos tem de dizer a mesma coisa.
- **O stream pode estar na tabela MINI.** Abaixo de 4 KiB ele não mora nos setores comuns e sim
  dentro do stream do Root Entry, em setores de 64 bytes com uma FAT própria. O arquivo do dono usa a
  comum (57 093 bytes); qual das duas nada no `.ppt` diz, o TAMANHO decide, e por isso as duas são
  lidas e as duas têm fixture.

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

**E o enum chegou a 256 nomes, e o wasmtime media a etiqueta errado.** Até a versão 39 o wasmtime
dava DOIS bytes à etiqueta de um enum ou variante com exatamente 256 casos, onde a ABI canônica — e
o `wit-bindgen` que constrói todo buffer — dá um (wasmtime #12066). Quando os tokens de ícone
chegaram a 256, todo `render` de buffer voltou `discriminant N out of range`: o host lia um ícone e
o byte seguinte como uma etiqueta só. O workspace estava na 38 e está pinado na **40**, em que os
dois lados medem a etiqueta pela mesma regra; o relato está no comentário do `Cargo.toml`. Descer da
40 traz o defeito de volta em todo buffer.

#### A vista lê o cache do NetworkManager, e não espera o rádio

O localizador de aplicativos varre o XDG numa thread porque ler centenas de `.desktop` custa dezenas
de milissegundos. Aqui não há disco: o NetworkManager mantém em memória os pontos de acesso que o
rádio já viu, e devolvê-los é uma ida ao barramento.

O que demora é a **varredura**, e essa não é esperada: `RequestScan` volta na hora e o resultado
aparece na leitura seguinte. Bloquear até o rádio responder congelaria o rodapé por segundos — a
mesma armadilha do `systemd-run --scope`.

Rádio desligado lista **zero** redes, e não as da última varredura: oferecer uma conexão que não
pode acontecer faria a tecla falhar parecendo defeito.

#### A vista foi lida da tela, e onze coisas voltaram

Ela passava em tudo. O dono abriu, olhou, e trouxe onze — nove delas defeitos de LEITURA: a tela
estava certa e se lia mal. O que elas têm em comum vale mais do que cada uma:

**Uma aritmética escrita três vezes já tinha divergido.** Rede, energia e Bluetooth são a mesma
forma — lista de rótulo e valor — e cada uma tinha escrito a colocação. Entre as três havia três
respostas, duas delas no MESMO arquivo: valor curto encostado na borda direita da superfície, valor
longo caindo para o terço da linha, e fato no terço sempre. Numa janela larga a primeira põe `off` a
cento e cinquenta colunas de `modo avião`, com nada no meio. `chrome::field_pair` é uma só, e o
valor começa numa COLUNA (`space.view.labelCells`) — o que faz das cinco uma lista de definições:
o olho desce uma coluna em vez de ziguezaguear. É a mesma regra que o `chrome::rule` e o
`chrome::divider` já tinham estabelecido para separador.

**Uma seção escrita inteira tem uma coluna só, e rótulo nenhum cortado.** O `field_pair` coloca
cada linha sozinha: numa porta estreita, onde a coluna é a metade da linha, cada rótulo longo
empurrava o próprio valor, e os valores viravam um ziguezague cortado na borda (`desligad…`, `de…`).
Quem escreve a seção inteira de uma vez — as páginas do player, em duas passadas
(`player_rows::Builder::build`) — mede antes: os valores da seção começam depois do rótulo mais
largo e das duas células de ar (`chrome::section_column`), nunca antes da coluna de toda outra
vista nessa largura e nunca tão longe que o valor mais largo seja cortado; onde a linha não
comporta os dois, o rótulo QUEBRA nas linhas de baixo (`chrome::aligned_pair`), que continuam a
dele: o cursor não para nelas, e um clique nelas é na linha (`Row::continues`, `Row::owner`). Uma
primeira tentativa mudou o próprio `field_pair` (um nome nunca empurra, cortado na coluna) e
alcançou as dezessete vistas que o chamam: na de energia, `fechar a tampa, na tomada` e `fechar a
tampa, no dock` viravam o mesmo `fechar a tampa, n…`. O `field_pair` voltou à regra de sempre, e
levar a regra da seção a outras vistas é decisão do dono.

**Fato e controle eram desenhados idênticos.** A diferença ia ser um glifo na frente, e o dono
RECUSOU a marca: sobrou o TOM — `text.label-acts` no rótulo e `text.primary` no valor de quem
responde ao `RET`, contra os tons de fato de quem não responde. O acento já está gasto no cursor,
então a diferença é de peso e não de cor, e um leitor que não separa bem tons a lê pela posição da
linha na seção. UM comentário no código continuou nomeando um `icon.row-acts` que nunca chegou a existir em
token nenhum, e dois outros registravam a AUSÊNCIA da marca — o que é a razão de esta frase
estar escrita, e de ela dizer o número certo.

**Uma lista que se lê continuamente não pode ser livre para crescer.** O Wi-Fi é relido o tempo
todo e a resposta muda: o roteador do vizinho responde, o celular de alguém aparece, o sinal
reordena. Com a lista livre, tudo abaixo dela subia e descia sem ninguém tocar em nada. Uma
PRATELEIRA (`network::Shelf`) desenha SEMPRE o mesmo número de linhas, completando com vazio, e
rola dentro de si com barra própria. Três decisões dentro dessa:

* **preenche até a altura inteira já com um item.** Completar só até o que tem é o defeito de volta
  — de uma rede para seis move cinco linhas;
* **vazia não desenha nada.** Um bloco de linhas em branco sob um título não diz nada; a seção diz
  por quê numa frase;
* **o TOTAL viaja no `Row`.** A prateleira é quem corta, e tanto a barra quanto a seta na borda
  precisam do número inteiro; contá-lo de outro jeito seria montar a lista uma segunda vez.

**E a prateleira é uma CAIXA, não uma faixa da largura da tela** (`chrome::shelf_box`). A largura é
a de UMA linha dela — `view.markerCells` + `view.labelCells` + `view.valueCells` — mais o ar de cada
lado, a célula da própria barra e uma célula livre antes da barra da página. Três coisas saem disso,
e as três foram lidas da tela pelo dono:

* **as duas barras deixam de disputar coluna.** Moravam as duas na última célula da superfície;
* **o fundo não alcança o polegar da página.** Sendo da largura da superfície e empilhado DEPOIS da
  barra, ele a apagava onde houvesse prateleira na tela — *"ela acaba desaparecendo e mudando de
  tamanho"*. A ordem agora é fundo → faixa → barra → texto, e a geometria é o que faz a ordem não
  ser a única defesa;
* **a barra fica perto do que ela rola.** Numa janela de 118 colunas a caixa tem 73: o polegar
  encosta na lista em vez de ficar a cinquenta colunas dela.

Largura FIXA e não a da linha mais larga que a prateleira tenha: uma borda direita que segue o
conteúdo é, na horizontal, exatamente o que a prateleira existe para impedir na vertical. Abaixo da
medida ela é APERTADA, que é a única resposta honesta quando não há tela para uma.

**A roda tinha cinco cópias e as cinco erravam igual.** `(pixels / linha) as i64 * 3`: a divisão
inteira joga fora a fração de linha, então gesto lento de touchpad não movia NADA até cruzar uma
linha inteira e então movia TRÊS; e o detente de mouse chega como quinze pixels, que é zero linha,
então a roda só andava quando dois caíam no mesmo quadro. `surface::axis_lines` faz da LINHA a
primitiva e do detente a linha dividida; `wheel_arrived` guarda a sobra e `wheel_lines` gasta só o
inteiro. **O rodapé conta linhas junto**, e a diferença dos seus dois modos foi para onde ela
pertence: onde a região ROLA, um passo é uma linha; onde ela PAGINA, um passo é uma página e custa
um detente inteiro — `LINES_PER_DETENT` linhas de dedo, a mesma distância que uma volta da roda
pede. Antes o acumulador guardava detentes, e o lado que rola gastava três linhas de uma vez: era a
mesma gagueira que as cinco janelas acabavam de perder, viva na porta sobre a qual o relato foi
escrito.

**E uma chegada não segue o cursor** (`Footer::settle_region`). Três coisas chegam sozinhas na vista
de rede — o snapshot do NetworkManager, a leitura de 10 s dos túneis e uma varredura que terminou —
e as três chamavam `follow_cursor`, que ESCRAVIZA o deslocamento ao cursor. Quem estava lendo a
lista de Wi-Fi com o cursor parado na seção do tailnet via a página ser jogada de volta para lá
segundos depois, sem ter tocado em nada. É a mesma separação que as cinco janelas já fazem entre
`settle_offset` e `follow`: chegada ajusta, tecla de gente segue.

A primeira versão disso seguia o cursor quando o `settle` o tivesse MOVIDO — um cabo desligado leva
seis fatos embaixo dele. Um revisor desmontou o argumento: o `settle` move o cursor por dois motivos
e os DOIS são obra da própria chegada — a lista encurtou, ou a linha sob o cursor virou um título.
De umas oitenta linhas ali, dezenove são títulos e até vinte e cinco são enchimento de prateleira,
então mais ou menos uma mudança em cada três cai numa; o cursor anda um, e a página é jogada para
onde ele está. Que é o relato, exatamente. Nada que uma chegada faça é razão para mover a página: o
cursor é uma COR, e a próxima seta leva a região até ele.

**Rede lembrada fora de alcance existia em lugar nenhum.** `saved` era uma palavra ao lado de uma
rede que o rádio via, então "quais Wi-Fi esta máquina conhece?" respondia "os que você está perto",
e esquecer um que não se está perto era impossível.

**Três `tailnet` e três `address`.** Cada um era um fato diferente vestindo a mesma palavra, numa
lista onde a palavra é a única coisa que diz qual fato é. O check `labels-of-a-view-are-distinct`
prende isso por VISTA — duas vistas podem dizer `on`, e dizem; o que não pode é uma lista dizer a
mesma palavra duas vezes.

**Cinco janelas rolavam sem barra nenhuma.** Só a do gerenciador desenhava uma. A roda funcionava,
não havia o que agarrar, e nada na tela dizia onde se estava numa lista de dez seções — o dono
reportou como "a barra não funciona", e a resposta é que não havia barra para funcionar. O check
que fecha isso (`every-list-window-has-a-bar`) achou a quinta sozinho, que ninguém tinha citado.

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
**persistido**. Então a porta em que a senha foi digitada — o rodapé ou a janela — a entrega ao NM na
hora de conectar, e o daemon só entra no boot seguinte — quando não há mais ninguém digitando. Sem isso, a senha teria de atravessar
mais um canal, e cada canal é uma superfície a mais.

**Escolha por rede, e não política global.** Depois de conectar, uma pergunta da vista, nas duas
portas: `sim` grava `psk-flags=1` e sela; `não` — o primeiro, onde o cursor nasce, e o `Esc` — deixa
`psk-flags=2`, e nada persiste em lugar nenhum. A
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
| **subcomando da CLI** | **4** (`comando`, `satelite`, `verificar-keymap`, `exportar-binds`) |

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
- **Nem todo comentário começa por `#`.** Classificar a prosa de `nix/dev.nix` como código fez o
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

Renomear um `contrato-*.rs` para `*-contract.rs` **não** renomeia o binário: quem manda é o
`[[bin]] name` do `Cargo.toml`. O teste continuou chamando o nome antigo e falhou com exit 127
— erro de shell, não de Rust, e portanto invisível ao `cargo check`.

#### Verificar por AUSÊNCIA, não por execução

Em todas as etapas em que declarei conclusão cedo demais, o que eu tinha verificado era *"o script
rodou"*. As duas verificações que de fato fecharam assunto foram outras:

- varrer o **dicionário inteiro** contra todos os literais e justificar cada falso positivo — foi
  assim que apareceram cinco arquivos e um `.wgsl` que a lista de extensões não cobria;
- cruzar **todo `bin/<algo>` citado** contra o que existe — foi assim que apareceram treze binários.

Uma busca construída a partir do que se espera achar devolve o que se espera achar.

---


#### O Syncthing ganhou vista, e o argumento é o invariante 14

Por anos esta distro instalou e AFINOU o Syncthing — os valores do módulo saíram de uma
transferência medida de 592 GB — e não ofereceu tela nenhuma sobre ele. `LEADER s s` tinha quatro
teclas: uma abria a interface WEB num navegador, uma ecoava o estado numa linha, duas pausavam e
retomavam. Acrescentar uma pasta, deixar uma máquina entrar ou mudar qualquer uma das 121 opções
significava sair do sistema e ir para uma página. É exatamente a chrome de fora que o invariante 14
existe para negar, e foi o que este trabalho fechou.

A página continua a uma tecla (`s s a`), e isso é deliberado: ela tem gráfico e histórico que uma
lista de rótulo e valor não substitui, e fingir o contrário seria pior do que dizê-lo.

#### Evento, não relógio — e por que a mudança RELÊ em vez de fundir

`/rest/events?since=N&timeout=60` é um LONG POLL: segura a conexão e responde no instante em que
algo acontece. Medido contra a 2.1.3, que é a versão que o flake pina: um poll ocioso bloqueia o
minuto inteiro e devolve lista vazia, uma mudança real volta em **17 ms**, e o cliente bloqueado
gasta **zero** tique de CPU em 25 segundos. Nenhum temporizador novo entrou no sistema.

O que chega é um lote de eventos, e o crate **relê tudo** em vez de fundir cada um. A fusão é onde
moram os defeitos: o som aprendeu isso caro, com o `metadata` do PipeWire chegando parcial e sendo
substituído inteiro — e o único jeito de alguém notar foi comparar duas fotos da mesma tela. O
Syncthing traz o mesmo perigo em mais formas: vinte tipos de evento, alguns com o objeto inteiro e
outros com um campo. Reler são dez chamadas de poucos milissegundos na loopback, e só quando algo
mudou; um piso de meio segundo impede que uma transferência — que emite progresso a cada segundo —
vire laço.

#### 121 opções declaradas, e as três naturezas de uma linha

`assets/syncthing-options.nix` declara cada opção: a chave JSON, a espécie, a faixa, se o flake a
fixa, se é avançada. Dali saem a tabela Rust, as chaves que os dois idiomas devem, e o
`syncthing-options-are-declared`, que compara a declaração com o ESQUEMA REAL do Syncthing nos dois
sentidos — uma opção que ele acrescentou e uma que nós inventamos falham a build em vez de
simplesmente não estarem lá.

1. **O flake fixa 14.** Desenhadas, alcançáveis e RECUSANDO, com a frase dizendo de onde vem o
   valor. É o invariante 6 na tela: oferecê-las como editáveis seria oferecer uma tecla cujo efeito
   o próximo `nixos-rebuild` desfaz em silêncio. **Duas das catorze são padrão do nixpkgs e não do
   nosso módulo** — `localAnnouncePort` e `maxFolderConcurrency` —, e só o
   `syncthing-flake-owned-rows-refuse` as viu: as duas estavam declaradas editáveis.
2. **As suas.** Nada no flake as repõe, e sobrevivem ao rebuild porque o módulo declara
   `overrideFolders` e `overrideDevices` desligados.
3. **As avançadas**, numa GAVETA fechada. O chevron é dela e de mais nada na vista — por argumento
   do próprio dono, que o recusou numa linha que age dizendo que aquilo *"é utilizado em elementos
   de descolapsar/colapsar"*.

#### As duas portas digitam, e a pergunta é da vista

O id de 63 caracteres de uma máquina, o caminho de uma pasta, onde fica uma pasta oferecida e o
valor de uma opção de texto ou de lista são digitados na porta em que a tecla foi apertada: no
minibuffer do rodapé ou na linha do cabeçalho da janela (`Ui::ask`), onde `C-v` cola — e um id quase
sempre chega colado. A janela respondia com uma frase mandando digitar no rodapé, e por ela não se
acrescentava máquina nem pasta.

Quem sabe O QUE foi perguntado é a `View` (`View::asked`), porque a resposta sozinha não diz: um
caminho pode ser de uma pasta nova ou de uma oferecida, e são ordens diferentes. As duas portas
entregam o texto ao mesmo `View::answered(id, texto)`, e a pergunta só responde quando o id é o
dela; sem pergunta da vista, quem decide é o id (`syncthing::unasked`), que é o que faz `s s d` e
`s s f` funcionarem com a vista fechada. **Pergunta largada deixa de existir** — `Esc` na linha, ou
qualquer tecla que chegue à vista, já que enquanto a linha pergunta toda tecla é dela. O `Esc` do
minibuffer não a limpava, e um caminho digitado depois em `s s f` saía como `AddDevice`, com o
caminho no lugar do id da máquina. Resposta vazia é "deixa para lá" nas duas portas; no rodapé ela
reabria a pergunta, ou apertava `RET` na linha sob o cursor.

#### A marca, e a única regra polkit nova desde o Tor

A marca da modeline é a única do sistema cuja EXISTÊNCIA é uma escolha: `Never`, `WhenOff` (padrão)
e `WhenOn`. `WhenOff` acende quando nada está viajando — pausado OU serviço fora do ar, que do
outro lado da tela são o mesmo fato —, e a vista é que diz qual dos dois. Uma máquina sem nada
configurado não acende nada: marca que nasce acesa no primeiro boot não quer dizer nada no segundo
dia.

Parar a unidade é a ÚNICA coisa privilegiada de toda a vista. Pausar, retomar, varrer, acrescentar
pasta, escrever qualquer opção, até reiniciar o processo — tudo vai pela loopback e não pergunta a
ninguém. Parar o `syncthing.service` não pode: é unidade de SISTEMA rodando como o dono, e
`manage-units` sobre uma dessas é do root. Sem a regra, "desligar o Syncthing" seria a única coisa
da vista que responde mandando a pessoa abrir um terminal.

#### Um HTTP à mão, e ele deixou de ser dois

`ronoctua-tunnel` tinha um framer HTTP cuja primeira linha dizia ser *"a forma que o
`shell::syncthing` já tem"*. Eram dois, e o Syncthing faria um terceiro. Agora é um crate
(`ronoctua-http`), e escrever o check custou uma descoberta: havia um TERCEIRO já, no `aria2.rs`,
e era o único dos três cuja resposta era lida como `String` antes de ser cortada — com o pânico que
os outros dois já tinham pagado esperando dentro dele. O `ronoctua-world` deixou de depender do
crate dos túneis para ter um decodificador de chunk.

### Quem espera o rádio é uma thread, não o quadro

`RequestScan` volta na hora; o rádio responde alguns segundos depois. Esperar no `calloop` congelaria
teclado, pintura e eventos do compositor por esses segundos — a mesma armadilha do `systemd-run --scope`.
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

`c` virou GRUPO e depois deixou de ser: a captura de tela era `t c`, `t` passou a ser a fila de
transferências, e o dono escolheu `c` de CAPTURA para as duas — o clipboard captura o que se copia, a
tela captura o que se vê. Ele desfez isso depois, e a razão está na frase: uma palavra que cobre dois
trabalhos sem relação é um grupo que lê bem e responde mal, e custava uma tecla ao clipboard, que se
usa o dia inteiro, para hospedar um screenshot, que não. A captura foi para `LEADER m s`
(MISCELÂNEA, o gaveteiro do que não pertence a nenhum outro) e `c` voltou a ser COMANDO — o que só é
legal porque nada mais pende dele: prefixo que também é comando é a colisão `PrefixWithCommand`.

`LEADER c` abre, na região abaixo do minibuffer, o histórico à esquerda e — separado por uma régua,
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
  clarearia tudo — a mesma armadilha do `to_linear` do `AGENTS.md`, vista do outro lado.
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

#### O ClipCascade dizia a verdade, e agora a verdade é outra

Por meses o painel mostrou `ClipCascade: não integrado`, com o comando de configurar dizendo o
mesmo. Era honesto e era um lugar reservado: o `PENDING.md` guardava a pergunta com o único marcador
`· decidir antes` que aquele arquivo já teve — **o histórico de clipboard deve sair deste
computador?**

O dono respondeu que sim, e o `ronoctua-clipcascade` é a resposta construída. Três decisões dentro
dela valem mais que o código:

**O cliente é NOSSO, não o do upstream.** O cliente Python vigia a área de transferência ele mesmo,
com `wl-paste`. Rodá-lo ao lado significaria DOIS vigias e DUAS histórias, e o painel só poderia
dizer "rodando" — nunca "integrado". Falando o protocolo aqui, o que o rodapé já guarda **é** o que
sincroniza, nos dois sentidos. O protocolo é pequeno: login por formulário com CSRF, PBKDF2-SHA256
para AES-256-GCM, e quadros STOMP sobre WebSocket.

**O nonce tem dezesseis bytes, e foi isso que decidiu a dependência.** O plano deste trabalho dizia
que a cripto não custaria nada novo, porque o `ring` tem AES-256-GCM. Tem — com nonce de DOZE bytes,
fixo: `ring::aead::NONCE_LEN` é 12 e não há outro construtor. O pycryptodome usa 16 por padrão
(medido cinco vezes na 3.2.0 instalada), e GCM com nonce que não é de 96 bits deriva o J0 por GHASH
em vez de concatenação — o `ring` calcularia outro J0 e **nenhuma** mensagem autenticaria, em
silêncio. O `aes-gcm` é genérico no tamanho, e o vetor capturado do Python do upstream é o que prova
que os dois concordam.

**O que o clipboard marcou como senha não atravessa a rede**, e o filtro fica na fronteira do crate
(`worth_sending`) e não na vista: uma segunda porta sobre este crate não pode esquecê-lo.

**O `salt` e as `hash_rounds` são da PESSOA, e não há endpoint que os sirva.** Esta é a segunda
suposição sobre a cripto que a fonte do upstream desmentiu, e ela é pior que a do nonce porque
falha em SILÊNCIO. A primeira versão deste crate pedia os dois ao `/max-size` — com um comentário
afirmando que "o ClipCascade os entrega junto com o login, o sal é por instalação e as rodadas são
configuráveis". Medido na 3.2.0 instalada: `/max-size` responde `{"maxsize": n}` e mais nada; o
`core/config.py` inicia `salt` em `""` e `hash_rounds` em `664937`; o `cli/login.py` e o
`gui/login.py` PERGUNTAM os dois, com a frase *"use exatamente o mesmo sal em todos os
dispositivos"* ao lado. Ou seja: são parâmetros que a pessoa escolhe uma vez e repete em cada
máquina, e o servidor nunca soube deles.

A versão errada funcionava por acaso — o campo ausente lia `""` e o `unwrap_or` caía em 664937, que
são justamente os padrões do upstream — e teria quebrado na primeira instalação que mudasse
qualquer um dos dois, derivando uma chave que não decifra nada. **O sintoma seria um clipboard
vazio**, que é indistinguível de "ninguém copiou nada". Hoje os dois são preferências desta máquina
e LINHAS da vista, ao lado do endereço e da conta, pelo mesmo motivo que aqueles são preferências:
descrevem a instalação de alguém, e nada disso entra no repositório (invariante 13).

E a legenda das ações não existe mais: ela era escrita à mão dentro do painel, a partir dos rótulos
LONGOS `cmd-*`, e numa tela com poucos itens caía **em cima da linha do minibuffer** — o dono
fotografou. Hoje é uma dica como a de toda outra vista (`chrome::hint_of`), gerada da tabela que a
própria vista carrega.

### O histórico do shell — `LEADER s h`, e o login que as duas portas digitam

`s h h` desenha a vista na região e `s h w` a abre numa janela (`br.ronoctua.history`). Ela tem
janela e o clipboard não porque o `atuin` é um BINÁRIO e o `config.toml` é um ARQUIVO, e um segundo
processo lê os dois (`docs/PENDING.md`, item 6).

**O login são quatro respostas, e quem as guarda é a `View`.** Servidor, conta, senha e a chave da
primeira máquina — vazia na primeira — são quatro `history::Question` feitas uma depois da outra, e
um ajuste de texto, lista ou quantidade é uma quinta. A `View` pede cada uma (`Errand::Ask`) e guarda
as respostas até ali; a porta só abre a própria linha — o minibuffer no rodapé, o cabeçalho na
janela — e devolve o texto ao mesmo `View::answered`, que responde com a próxima pergunta ou com a
ordem. Antes, a cadeia morava em três campos do rodapé, e a janela mandava digitar lá. A senha
espera a chave dentro da `View` num tipo que mascara o `Debug`, vai para o `atuin` pelo STDIN, e sai
quando a pergunta é largada (`View::let_go`: o `Esc` da linha nas duas portas) ou quando uma tecla
chega à vista. **A chave também é segredo**: decifra o histórico inteiro, e numa linha às claras ia
para o journal com toda resposta que o rodapé registra. Valor vazio num ajuste não escreve nada —
`chave = ` não é TOML, e o Atuin então volta ao padrão em TUDO, calado.

**O cursor guarda a linha, não o índice.** A pergunta da chave cresce uma frase ACIMA da linha
dela; pelo índice o cursor ficava na frase, o `settle` o levava à primeira linha da página que age
— o anel da marca — e o segundo `RET`, o "sim, mostra", girava o anel. Hoje a linha da chave é a
mesma nos três estados (mostrar, perguntar, mostrada), e o `RET` na chave mostrada a guarda. O
primeiro clique seleciona e o segundo age (inv. 19) — o clique apertava `RET`, e o primeiro clique
em "sair" saía —, e título ou fato não pegam clique.

**O que uma ordem diz ao terminar é uma resposta só** (`history::said_when_done`): recusa sempre;
sincronizar e sair, pelo nome; ajuste, marca e login, nada, porque as linhas já mostram. O rodapé
dizia "o histórico está sincronizado" depois de TODA ordem, e a janela não dizia nada.

---

### Os atalhos do compositor

**A distro não lê o config que o compositor traz** (`-c`), e por um tempo isso significou que os
atalhos dele não existiam aqui: os feitos eram alcançáveis só pelo nome, e a razão escrita era que
cem atalhos custariam cem teclas. O dono pediu o contrário, e o custo real é menor do que a frase
dizia: as teclas do Scroll moram quase todas atrás do Super segurado, e a leader é o Super TOCADO.
Então todo bind e todo modo do arquivo pinado é gerado da nossa tabela, e o check lê o arquivo de
verdade, então um atalho novo do upstream não fica de fora em silêncio.

**Onde as duas tabelas se encontram a distro fica com a tecla** (nove: `S-setas`, `f`, `S-f`, `c`,
`t`, `g`, `,`, `.` e a roda), e o que do Scroll ficaria sem tecla nenhuma ganha outra — escolha do
dono, porque as duas alternativas eram tirar do dono uma tecla que ele já usa ou deixar uma ação sem
tecla. **No `.#dev` o prefixo é o Alt e o teclado aninhado não tem Super**, então os dez
`Super+Alt+…` virariam `Alt+Alt+…`, que é a tecla sem o Alt: ganham `C-S-` e uma letra ali, e um
teste recusa qualquer bind do aninhado com um segundo Alt. **Dois desvios, e são de propósito**: o
`button8` com a janela inteira tomaria o botão Voltar de todo navegador, então pede o prefixo; e o
`jump criteria [app_id="firefox"]` nomeia um programa, que a regra dos feitos não aceita.

**Um modo precisa ser dito**, e sem barra nada o diz — uma pessoa num modo sem saber está com o
teclado quebrado. O evento `mode` do compositor entra no contrato (`ModeChanged`) e o rodapé o
desenha duas vezes: uma marca na modeline cujo clique é o `Esc`, e as teclas do modo na linha de
dicas, lidas da mesma tabela que gera o bloco `mode { }` — a faixa de dígitos sobre o mesmo feito
contada uma vez só.

### A câmera e o estúdio

**Por que um daemon.** Um dispositivo V4L2 transmite para UM leitor. Se o app abrisse a câmera,
a câmera virtual morreria com a janela, e uma gravação também; uma cena com a tela e a webcam
precisa de um processo que tenha as duas. Então o `ronoctua-studio` é dono de todo vídeo, e app e
ajustes são clientes. Ele fica de pé com a sessão e NÃO é ativado por socket: adotar o descritor que
o systemd abriu pede `from_raw_fd`, que é `unsafe`, e um daemon parado num `Condvar` custa o mesmo
que um que não existe. Com nada assistindo, gravando ou transmitindo, toda fonte fecha.

**Por que memória e não a câmera virtual para a prévia.** O `v4l2loopback` é legível por qualquer
programa. A prévia de uma cena com a TELA ficaria aberta à máquina inteira enquanto a pessoa só
olhava. Um `memfd` passado por `SCM_RIGHTS` chega só a quem pediu. O anel tem três fatias com um
número de sequência escrito antes e depois dos pixels; uma fatia sobrescrita durante a cópia é
jogada fora, nunca mostrada rasgada. É `pread`/`pwrite`, sem `mmap` e sem `unsafe`, e a cópia que o
leitor faz é a que o upload já faria.

**Por que um interpretador nosso e não o `tract`.** Medido: o `tract-tflite` recusa o
`selfie_segmenter` (110 `DEQUANTIZE` de pesos fp16 e um `Convolution2DTransposeBias` CUSTOM). O
modelo usa onze operações; um leitor de flatbuffer mais o interpretador são poucos milhares de
linhas sem dependência, o molde do MD5 e do zip. A referência é o MediaPipe 1.0.1 sobre uma entrada
sintética de 16×16 (`reference.txt`): pior diferença 5,8e-5, e uma sabotagem de um operador a leva a
0,31. Em release, 25 ms por máscara numa thread própria; a máscara é suavizada no tempo e o quadro
nunca espera por ela.

**Por que a composição é na CPU.** As fontes chegam do ffmpeg JÁ no tamanho da camada, então compor
é copiar e misturar retângulos: ~9 ms por quadro a 720p com duas camadas. A GPU pediria ler de
volta cada quadro para o encoder, e isso só vale depois de uma medida que a peça.

**Por que o encoder não reinicia.** Trocar de cena, mover a webcam ou mudar um efeito durante a
gravação muda o que o compositor LÊ; o encoder recebe quadros crus num cano e não sabe de cena
nenhuma. O som chega por FIFOs não bloqueantes: um encoder que para de ler perde som em vez de
travar o laço — o laço travado, com a trava na mão, foi um deadlock medido ao parar uma gravação.

**Por que o fundo é desfocado em tamanho reduzido.** Medido nesta máquina: o desfoque de quadro
inteiro (seis passadas de caixa a 1280×720) custava 46 ms, e uma câmera a 30 fps dá 33. Cada quadro
ficava mais para trás, e como a fonte lia a câmera no mesmo laço que aplicava o efeito, o atraso se
acumulava na fila do ffmpeg — o dono leu o vídeo como inutilizável. Duas correções, e as duas são
necessárias: o fundo é reduzido por um quarto do raio (entre 1 e 8), desfocado lá e trazido de volta
bilinear (9,5 ms, e o que a redução perde é o detalhe que o desfoque existe para tirar; onde a máscara
é a pessoa, o pixel original fica byte a byte); e a câmera é lida numa thread própria que guarda só
o quadro mais novo, então nenhum efeito, por lento que seja, volta a acumular atraso. Medido depois,
com a câmera real e o fundo desfocado: 30 quadros por segundo.

**Os ajustes digitam nas duas portas.** `LEADER s c c` é a região do rodapé e `LEADER s c w` a
janela, e as duas perguntam o nome de uma cena nova, o novo nome da cena no ar, um roteiro, o
endereço da transmissão e a chave dela: a tecla da linha devolve uma pergunta (`Errand::Ask`), o
rodapé a faz no minibuffer e a janela na própria linha do cabeçalho, e as duas respostas entram em
`camera_settings::answered`. A vista fica na tela enquanto se digita, com o cursor na linha que
perguntou — o rodapé fechava a vista antes de perguntar, e a resposta voltava para uma tela vazia.
Resposta vazia é "deixa para lá" nas duas portas. O roteiro é lido contra o livro de cenas na hora de
cumprir, e o que não se lê vira frase. **A chave é segredo**: mascarada na linha, fora do log, e
escrita no arquivo `0600` pelo processo que a digitou — nunca desce o soquete do rodapé nem entra
num `argv`. As duas portas leem uma tecla pelo mesmo leitor (`camera_settings::press_of_key`), que lê
a letra pela notação inteira: o rodapé tomava `C-d` por `d` e tirava uma cena, e a janela não fazia
nada. O som (`audio::press_of_key`) e os ponteiros (`pointers::press_of_key`) tinham o mesmo defeito
com `C-m` e `M-d`, e têm o mesmo leitor.

**Genérico por construção.** Nada nomeia um fabricante. Câmera é o nó cujo `device_caps` tem
`VIDEO_CAPTURE`; infravermelho é o nó cujo único formato é cinza; a identidade é o `card`, o `bus` e
a INTERFACE USB, porque cor e IR de uma mesma webcam têm `card` e `bus` iguais. O modo sai do que o
dispositivo enumera (pelo menos 24 quadros por segundo se algum modo chega lá, a maior área até
1080p, comprimido antes de cru no mesmo tamanho), e o
`camera-vm` prova tudo contra uma "webcam" que é um loopback alimentado por `testsrc`.

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
compositor e rodapé 100, daemons 0.

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
critério do `ronoctua-scroll` e do `ronoctua-network`. Os parsers são puros: entra `&str`, sai número.
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

### O disco se cuida sozinho — o piso do Nix, as gerações por contagem e os snapshots

Pedido do dono (2026-09-28): o disco se resolve "sem precisar de intervenções minhas", em todo
computador que instalar a distro, e nada pode apagar o que o Nix da pessoa usa — com UMA exceção,
concedida por nome: um limite por contagem nas gerações do SISTEMA, que sempre guarda a atual, a em
execução e a do boot. Todo tamanho é porcentagem do disco de quem instala — o piso e a meta com um
teto em GiB, como o journald faz com os dele —, e os números moram em `assets/cache.nix`; o cérebro
lê os mesmos pelo `nix/cache.nix`, que recusa o build quando uma relação entre eles se quebra (piso
abaixo da meta, e os tetos na mesma ordem; teto do cargo dentro do que a meta deixa; N ≥ 2).

#### O que quebrava a garantia

Três coisas com cara de faxina: o `nix.gc` semanal com `--delete-older-than 30d` (em `base.nix`), o
`generations` do painel de memória (o mesmo comando) e o passo `nix-profile-history` da
atualização, ligado. Como root, `--delete-older-than` apaga as gerações antigas de TODO perfil em
`/nix/var/nix/profiles`, `per-user` inclusive — o histórico do Nix da própria pessoa. O timer saiu (a
coleta da atualização é a semanal, e o piso cobre os dias entre elas); o passo da pessoa vem
desligado (a limpeza continua na vista de programas, como ato dela); o painel e o `ngc` do shell
chamam a aparagem.

#### O piso é o do próprio Nix

`min-free` e `max-free` fazem cada build e cada download coletar lixo — só lixo, nunca geração nem
nada com raiz — quando o disco cai abaixo do piso, até a meta. O valor depende do disco, então não
cabe no `nix.conf` gerado: uma unidade de boot (`ronoctua-store-floor`, antes do `nix-daemon` e
querida por ele) mede o sistema de arquivos do `/nix/store` e escreve `/run/ronoctua-store/free.conf`,
que o `nix.conf` inclui por último (`extraOptions` vem depois de `nix.settings`, e o último vence).

O arquivo sai `0644` e por rename porque o Nix **pula sem aviso** um `!include` que não consegue ler
(`configuration.cc` engole o erro de leitura; conferido na 2.34.8): um modo errado não é erro em
lugar nenhum, é uma máquina sem piso. Ausente também não é erro — é assim antes da unidade rodar e no
sandbox em que o nixpkgs valida o `nix.conf`.

**Porcentagem com teto.** Sem ele, dez por cento de um disco de 8 TB são 800 GB: um disco cheio
sobretudo dos arquivos da pessoa estaria "sem espaço" com centenas de gigabytes livres, o Nix
coletaria a cada build e o cérebro abriria espaço de hora em hora para nada. O piso é
`min(diskFloorPercent, diskFloorCapGiB)` e a meta `min(diskGoalPercent, diskGoalCapGiB)`, no piso do
Nix, no do cérebro e na meta da folga — a mesma conta nos três.

#### As gerações do sistema, por contagem — e o menu de boot cabe nela

`ronoctua-system-generations` guarda as N mais novas (`systemGenerations`, cinco: com atualização
semanal, cerca do mês que os 30 dias guardavam), a atual do perfil, a em execução, a do boot, a de
onde a última troca SAIU e toda geração que o menu de boot lista, e apaga os links das outras
gerações do perfil do sistema — de nenhum outro. O perfil é argumento só para o fixture provar o
programa, e ele recusa qualquer um que não se chame `system` ou esteja sob `per-user` ou
`system-profiles`. N ≥ 2 porque o rollback do console de resgate vai para a anterior.

- **Especialização.** Dar boot ou trocar para uma especialização deixa `/run/booted-system`
  apontando para o toplevel dela, que nenhum `system-N-link` resolve; cada geração é comparada
  também com `system-N-link/specialisation/*`.
- **A de onde a troca saiu.** Quem voltou para a 3 porque da 4 à 8 estão quebradas, e depois teve a
  atualização semanal construindo a 9, perderia a 3 na mesma troca que saiu dela: nada a guardava
  (não era a atual, a em execução nem a do boot). Um trecho de ativação grava
  `/run/ronoctua-generations/previous-system` enquanto `/run/current-system` ainda aponta para o
  sistema que sai — a ativação só troca esse link no último passo —, e a aparagem guarda o que ele
  aponta. No boot `/run` é novo e não há nada a gravar.
- **O menu.** Todo loader habilitado lista no máximo N gerações (`configurationLimit` ou
  `maxGenerations` em `mkDefault N`, e uma asserção por loader — systemd-boot, grub, extlinux,
  limine, rEFInd e o lanzaboote, se importado); um loader que ninguém sabe limitar
  (`generationsDir`, `initScript`, o `external` sem o lanzaboote) é recusado na avaliação. O
  construtor do menu usa as L mais novas e remove o resto, e com L ≤ N toda entrada está entre as
  guardadas. Quando a instalação do loader falha e o perfil avança mesmo assim, o menu fica para
  trás; por isso a aparagem lê os pontos de montagem de boot (`/boot`, a ESP, o XBOOTLDR, os
  `mirroredBoots`) e guarda a geração cujo número está no nome de uma entrada
  (`nixos-generation-7.conf`, a UKI do lanzaboote) ou cujo toplevel está no texto de um menu (grub,
  extlinux, limine, rEFInd). Menu ilegível ou ausente não lista nada: vale a contagem.

Ele **nunca** roda `switch-to-configuration`: o switch pega `/run/nixos/switch-to-configuration.lock`
sem esperar, e uma unidade automática que o rodasse mataria o `nixos-rebuild` de alguém no meio.

**Duas unidades, um programa.** `ronoctua-system-generations.service` é a que o passo
`old-generations` da atualização (antes da coleta), o painel de memória e o `ngc` iniciam, e cuja
falha eles veem. `ronoctua-system-generations-at-activation.service` roda o mesmo no boot e a cada
`switch` — o switch reinicia os targets ativos, e um `oneshot` sem `RemainAfterExit` que o
`multi-user.target` quer roda de novo, depois do loader instalado —, mas com `-` no `ExecStart`: o
switch sai com 4 quando uma unidade que ele iniciou falhou, o `nixos-rebuild` chama a troca inteira
de falha, e o passo `system-rebuild` da atualização pararia a sequência por causa de uma aparagem,
semana após semana, com o sistema novo rodando. Falhas antigas não contam (o switch faz
`reset-failed` no começo). E sem dependências padrão: um target espera toda unidade que ele quer e
que as tenha, e o boot esperaria uma aparagem em prioridade ociosa. Nenhuma das duas tem
`RemainAfterExit`: com ele, o `systemctl start` da atualização não faria nada. N é contagem, não
fatia do disco: num disco de 128 GB cinco closures podem passar de 60 GB, e o piso não apaga geração.

Os logs de build envelhecem por `e /nix/var/log/nix/drvs - - - 30d` (`buildLogDays`), limpos pelo
`systemd-tmpfiles-clean.timer` que o nixpkgs já tem. Com `noatime`, a idade é a da escrita.

#### O btrfs: nenhum snapshot guarda o store, e o `~/.cache` é um subvolume

Snapshots de uma raiz que contém `/nix` seguram tudo o que a coleta libera, e a maior parte de um
disco pode sumir assim sem que nada diga. No layout da distro `@nix` é subvolume de topo, e um snapshot de
`@root` nunca o contém; `nix/modules/filesystem.nix` mantém isso para o que uma máquina acrescentar.
Alvo de snapper ou btrbk em `/nix` (ou abaixo), num `.cache`, ou em `/` onde `/nix` não é montagem
própria é recusado na avaliação; o btrbk nomeia subvolumes como o topo os vê (`@nix`), e o `subvol=`
do `fileSystems` diz qual montagem cada um é.

Cada usuário normal ganha `ronoctua-cache-subvolume-<nome>`, antes do
`systemd-user-sessions.service` — que é o que deixa entrar o getty, o gerenciador de login e todo
gerente de usuário —, que faz do `~/.cache` um subvolume próprio. Com as dependências padrão e não
antes do `sysinit.target`: uma home montada tarde (pela rede, por um dispositivo lento) seria um
ciclo de ordem no boot. Snapshot não desce em subvolume aninhado, então os de `/home` deixam
de carregar o que a varredura apaga, e a raiz da varredura é um dispositivo só. O `~/.cache` que falta
vira subvolume direto; o que existe é copiado com `cp -a --reflink=always` para um subvolume ao lado
e trocado por `mv --exchange`, que é atômico — o boot seguinte termina ou desfaz uma troca
interrompida olhando qual dos dois é o subvolume (inode 256). `always` e nunca `auto`: uma cópia que
não fosse reflink escreveria o cache inteiro de novo, no boot, num disco que pode ser a razão de isto
existir. Arquivo NOCOW não aceita reflink, e aí o cache fica diretório, dito no journal. A MUDANÇA
só acontece durante o boot (`systemctl is-system-running` diz `starting`): no `switch` que traz a
unidade, alguém está logado ou vai estar, e copiar um cache dentro do switch é um switch parado;
fazer subvolume de um `~/.cache` que falta não move nada e acontece a qualquer hora. Uma tentativa
deixa um bilhete ANTES de começar (`/var/lib/ronoctua-cache-subvolume/<nome>`), que só o sucesso
tira: a que falhou, ou que foi cortada pelo limite de 10 minutos ou por falta de energia, não é
tentada de novo por `keepDays` — sem isso, a causa (o arquivo NOCOW, o disco cheio demais para os
metadados da cópia) continuaria lá, e todo boot pagaria a cópia antes do login. Link, montagem e
subvolume dentro ficam como estão.

`dynamic_reclaim` e `periodic_reclaim` (kernel ≥ 6.11, só dados) devolvem ao espaço não alocado os
block groups quase vazios, o que um `btrfs balance` manual fazia — sem root, sem polkit, na escala do
chunk de cada máquina. Num kernel mais velho o glob da linha `w` não casa e ela não escreve nada; por
isso não há asserção de versão. O scrub mensal do nixpkgs, um por sistema de arquivos, é o único
relógio: bit rot não tem evento. Os três só existem onde `ronoctua.filesystem.btrfs` é verdade
(derivado de `fileSystems`); a regra dos snapshots vale em toda máquina.

#### Os builds do cargo: fora de toda árvore, enxutos, e só o que se lê de novo fica no store

**Onde o cargo escreve** mora num lugar só, `nix/cargo-dirs.nix`:
`<cache>/cargo-target/rustc-<versão>/target` para o que o cargo promove (os binários, por hard link)
e `<cache>/cargo-target/rustc-<versão>/build/{workspace-path-hash}` para o resto — `deps/`,
fingerprints, estado incremental, quase todos os bytes. O `build.build-dir` é estável desde o 1.91, e
o template vira dois níveis por workspace (medido: `build/ab/cdef…`, com o `CACHEDIR.TAG` na raiz
deles). A versão no caminho faz um compilador novo começar um diretório novo em vez de
recompilar por cima do velho, e o velho vira uma unidade que a varredura leva inteira; o nome é
`cargo-target` e não `cargo` porque há quem aponte o `CARGO_HOME` para `~/.cache/cargo`.

A **sessão** de toda máquina (`nix/modules/cache.nix`, sem depender da varredura) define só o
`CARGO_BUILD_BUILD_DIR`, duas vezes: `sessionVariables` para login, console e ssh, e
`environment.d/60-ronoctua-cargo.conf` para o gerente de usuário, que é quem abre terminal, editor e
rust-analyzer. O `CARGO_TARGET_DIR` fica de fora de propósito: ele não tem template, então seria um
diretório só para todo projeto da máquina — vence o `build.target-dir` de cada um, um `cargo clean`
em qualquer lugar esvazia os binários de todos, e dois projetos com um binário de mesmo nome se
sobrescrevem. Cada projeto guarda o próprio `./target`, que fica pequeno. **Este repositório**
exporta as duas (o devshell sempre; o `.#dev` só o que o chamador não exportou), porque o `./target`
dele estaria dentro da árvore do flake e o laço procura o rodapé em `$CARGO_TARGET_DIR/loop/`. O
`.#dev` testa com `if` e não com `${VAR:-…}`: a `}` do template fecha a expansão, e uma variável que
já existia saía com uma `}` a mais.

**O `[profile.dev]`** é o de todo `cargo` local, nunca do Nix (as derivações pedem `release` ou
`loop`): `line-tables-only` nos nossos crates, nada nas dependências. O binário de teste do shell
passou de 716 MiB (631 de `.debug_*`) para 134 MiB (54), e um `cargo test -p ronoctua-shell
--no-run` do zero ocupa 3,8 GB de diretório de build. Sem `split-debuginfo`:
com só as tabelas de linha, o que sobra é quase todo `.debug_line`, que o DWARF separado deixa no
binário de qualquer jeito. O `release` perdeu o `debug = 1`: o fixup do stdenv roda `strip -S` no
`bin/` (o log de todo build do workspace diz isso), então ele nunca chegou a máquina nenhuma e só
engordava a camada de dependências e atrasava o link com LTO; o `.symtab` sobrevive ao `-S`, e o
`perf` continua nomeando funções. Cobra `cargo-dev-profile-is-lean`, que também recusa um
`CARGO_PROFILE_*` no devshell e um `.cargo/config*` na árvore — os dois vencem o manifesto.

**Só a camada fica no store.** O crane arquiva o `target/` inteiro no `$out` de toda derivação que
não é `buildPackage`. Nas camadas de dependências (`buildDepsOnly`, que força isso) é o produto; nos
testes, no clippy e no formatador eram gigabytes por edição que ninguém lia, cada um com um link para
a camada do `loop`. Os três têm `doInstallCargoArtifacts = false` — o `$out` continua existindo,
porque a instalação padrão do crane é `mkdir -p $out` —, e `leaf-builds-keep-no-artifacts` recusa a
derivação nova que vier com o padrão (um `cargoTest` ou um `cargoNextest`; o `cargoDoc` já vem
desligado). O `deps-layer-is-lean` recusa um `CARGO_*_DIR` numa camada ou em quem a usa: a camada
chega como `./target`, e com o diretório mudado o cargo não olha lá e recompila tudo, calado.

**As camadas em uso têm raiz; as velhas, não.** Nada instalado as referencia, então a coleta as
leva, e agora a coleta acontece sozinha. O `ronoctua-keep-sources` (`nix/kept-sources.nix`) segura
a camada que ESTA avaliação construiria quando ela já está no store — nunca é motivo para
compilar. A raiz leva o nome do caminho que segura (`layer-release-<hash>…`, `sources-<hash>…`), não
só o da camada: o diretório é da pessoa, e todo checkout deste repositório roda o mesmo script
nele, então uma raiz por nome seria tomada pelo último checkout que rodou — um worktree com outro
lockfile soltaria as 483 crates compiladas do checkout principal, e a coleta do piso, durante o
build do próprio worktree, as levaria. Cada execução renova as suas (`touch -h`), e uma raiz que
nenhum checkout renovou em `oldCompilerDays` é solta: é assim que a camada de um lockfile antigo
morre. Roda no `.#dev`, no `.#quick` depois do build e nas baterias depois
da fase 1. As fases 1 e 2 da bateria constroem com `--out-link` no diretório de trabalho dela, que o
`trap` apaga: o que elas pagaram fica com raiz enquanto a bateria dura, e nem um minuto a mais.

#### Quando o disco aperta: o cérebro abre espaço, na hora

O piso do Nix só age enquanto algo compila, e só no store; um disco cheio por qualquer outra coisa —
o cargo, antes de tudo — fica cheio até alguém ver. Quem vê é o cérebro: o pulso de cada tique já lê
o `statvfs` do sistema de arquivos do `/nix/store` e do da home (`Pulse::disk_short`: algum deles
abaixo do próprio piso, porcentagem com teto; `Pulse::disk_free_per_mille`, o mais cheio, é o que a
tela diz). Nada avisa um processo que um disco está enchendo — o inotify fala de arquivos, não de
capacidade —, então o evento é o tique que já existe, sem relógio novo.

A tarefa `disk-space` (`tasks/space.rs`) nunca vence pelo calendário: a linha dela diz "quando falta
disco". Ela entra por `Task::wanted_now` — abaixo do piso, e se o último INÍCIO foi há mais de
`reliefGapMinutes`, ensaio incluído (o runner guarda quando cada tarefa começou; o diário, onde
ensaio não é execução, não sabe, e o cérebro do `.#dev`, que só ensaia, disparava a folga a cada
tique) — e é a única que `Task::gated` tira do portão: a máquina está mais ocupada justamente
enquanto um build enche o disco, e uma folga que esperasse a memória acalmar esperaria o disco
lotar. Pelo mesmo motivo ela entra na CABEÇA da fila (`Shared::queue_first`): o runner só olha a
cabeça, e atrás de uma varredura retida pelo portão ela ficaria retida também. Roda em prioridade
ociosa, então ceder é trabalho do escalonador, não de um bloqueio.

**Uma folga que deixou o disco curto não se repete.** Quando o que enche o disco é da pessoa —
arquivos, programas, cinco closures do sistema —, nada mais sai sem ela, e uma folga por hora
andaria o cache e coletaria o store à toa, dizendo "continua sem espaço" a cada vez. Depois de uma
assim, a seguinte espera, além do intervalo, o disco encher MAIS do que ela o deixou: só então há
algo novo para liberar.

O trabalho é `ronoctua-disk-relief` (crate da varredura, unidade de usuário em
`nix/modules/space.nix`, o sandbox dela copiado da varredura, com `-%C` para uma pessoa sem
`~/.cache` ainda), em dois passos, e os números chegam como argumentos da mesma tabela de onde o
piso do cérebro é compilado — por isso não há opção para eles, nem interruptor para a folga (quem
a desliga é a página do cérebro; um segundo interruptor deixaria a tarefa pedindo uma unidade que
não existe, de hora em hora):

- **`cargo-builds`**, só quando o sistema de arquivos do CACHE está abaixo da meta — onde `/nix` e
  a home são dois, um store cheio não fica mais folgado recompilando as crates de alguém: o
  diretório inteiro de um compilador que ninguém usa, depois de `oldCompilerDays` — nunca o mais
  recente nem o da sessão (`--keep-compiler`), e só um diretório com o nome de compilador
  (`--compiler-prefix`, de `cargo-dirs.nix`) — e, enquanto todas as árvores do cargo sob o cache
  pesarem mais que `cargoCeilingPercent` do disco, unidades. Uma unidade é o que um
  `.fingerprint/<nome>-<hash>` nomeia — as entradas de `deps/`, `build/` e `examples/` com o mesmo
  hash (medido no 1.97: 768 fingerprints, toda saída nomeada por uma) — ou um diretório de
  `incremental/`. O fingerprint sai PRIMEIRO: o cargo reconstrói a unidade sem fingerprint, e uma
  remoção interrompida nunca deixa um fingerprint abonando saída que sumiu.
- **`store-garbage`**: `nix-store --gc --max-freed <meta − livre>`, pelo daemon, como a pessoa — só
  lixo, até a meta e não além, para o próximo build não baixar de novo o que acabou de sair.

**O que sai primeiro.** Com `noatime`, a idade de um arquivo é a da escrita: uma dependência
compilada há um ano e ligada em todo build desde então parece ter um ano, e levá-la a recompila —
e o cargo recompila tudo o que depende de uma saída mais nova que ele, então uma unidade velha pode
custar o workspace inteiro. Saem primeiro, então, o que ninguém lê de novo: a unidade que um build
POSTERIOR da mesma crate e do mesmo tipo (`lib-serde`, `run-build-script-…`) substituiu — um dia ou
mais mais nova, que um build escreve suas unidades em minutos —, a saída que nenhum fingerprint
nomeia e o estado incremental; depois o resto, da compilada há mais tempo para a mais recente.

**Só sob a trava do próprio cargo, em qualquer layout.** Nada sai de uma árvore do cargo sem que a
folga segure, exclusiva e sem esperar (`File::try_lock`), TODA trava que o cargo deixa em qualquer
lugar dela (`.cargo-lock`, `.cargo-build-lock`, `.cargo-artifact-lock`) — entenda ela o layout ou
não: o `target/<triplo>` do layout dividido tem as suas e nenhum fingerprint, e é uma árvore em que
um build pode estar escrevendo. Medido no 1.97: segurar qualquer uma das três faz um `cargo build`
imprimir "Blocking waiting for file lock" e esperar. Trava tomada, ou que nem pôde ser tentada,
deixa a árvore inteira (e o compilador inteiro). Unidade por unidade só num perfil que ela entende
(`.fingerprint/`, `deps/` e `.cargo-build-lock`, a trava do build inteiro desde o 1.96); o resto só
sai INTEIRO, e só depois de `oldCompilerDays` sem ninguém tocar. O ensaio solta cada trava assim
que a tentou: não apaga nada, então não tem build nenhum a segurar, e um `cargo build` da pessoa
não espera uma pergunta. As árvores são achadas pelo `CACHEDIR.TAG` que o cargo escreve (a
assinatura sozinha é da convenção, de muitos programas; a linha "created by cargo" é dele), até
oito níveis abaixo do cache: a da distro fica no quinto.

**`--cargo-root` é do cache ou nada.** Absoluto, sem `..`, dentro de `--root` e no mesmo sistema de
arquivos, e dele só sai o que tem nome de compilador: uma execução à mão com `--cargo-root
~/projetos` ofereceria todo projeto parado há uma semana à regra do compilador.

**Raiz do Nix nunca sai com a pasta.** Um `nix develop --profile` deixa `…/devshell-profile-1-link`
dentro do `~/.cache`, apontando para o closure inteiro do ambiente. A varredura por entrada levaria
a pasta depois de 40 dias parada, e a coleta seguinte levaria o closure. As duas varreduras
perguntam ao Nix (`nix-store --gc --print-roots`, pelo caminho no store) e comparam o
`(st_dev, st_ino)` do próprio link, lido sem segui-lo — o mesmo por qualquer caminho que se chegue
a ele; o Nix 2.34 imprime o link entre aspas, como o C++ escreve um `std::filesystem::path`. Sem
resposta, toda pasta com um link fica. **E uma resposta que ela não sabe ler não é resposta**: uma
linha num formato que o parser não conhece, ou uma lista sem raiz nenhuma (toda máquina tem ao
menos o perfil do sistema), é `Roots::Unknown`, com a frase do porquê no journal — senão um Nix que
mudasse o formato desligaria a guarda calado, com "0 com raiz do Nix" dito como fato. O formato é
provado contra o `nix-store` fixado, num store próprio no diretório de build, pelo teste
`the_pinned_nix_prints_roots_this_reads` (a derivação `tests` passa `RONOCTUA_NIX_STORE`).

**Todo link tem idade.** As raízes são perguntadas uma vez, antes da caminhada; um
`nix build -o ~/.cache/p/result` feito durante ela não está em resposta nenhuma. Por isso link e
socket contam pela própria data (lida sem seguir): um link feito agora deixa a pasta jovem.

**Montagem por bind não é diretório.** "Nunca sai do sistema de arquivos" comparava `st_dev`, e um
bind do mesmo sistema de arquivos (`/persist/x` sobre `~/.cache/x`) mantém o número: levar a pasta
inteira apagaria o que o `/persist` guarda. O kernel diz onde começa uma montagem
(`STATX_ATTR_MOUNT_ROOT`, desde o 5.8), e uma dentro é recusada como outro dispositivo.

**Olhar não é usar.** Ler o `CACHEDIR.TAG` movia o atime dele (num sistema de arquivos com
`relatime`), e toda árvore passava a ter um segundo de idade: o teste de um compilador parado havia
dez dias o viu ficar. A tag é aberta com `O_NOATIME` (recusado num arquivo de outro dono, que então
é lido normalmente e só pode parecer mais novo).

**O diretório do relatório existe antes do sandbox.** As unidades que o cérebro dispara abrem
`%t/ronoctua-brain` com `ReadWritePaths`, e um caminho que não existe derruba a unidade com
226/NAMESPACE antes do programa rodar (medido com uma unidade transiente). Só o script da
atualização o criava, então numa sessão em que nenhuma atualização tinha rodado a varredura e o lixo
nem começavam, e o cérebro esperava a paciência inteira. As três unidades declaram
`RuntimeDirectory = "ronoctua-brain"` com `Preserve`, e o `the-brain-reads-what-the-units-write`
cobra uma por unidade que escreve ali.

**O que a pessoa ouve.** A tarefa mede antes e depois e põe um `Relieved` no `Mind` (o maior ganho
entre os sistemas de arquivos, o quanto sobra no mais cheio, e se algum continua abaixo do piso); o
rodapé o transforma no evento `disk-relieved` ("espaço aberto no disco: 12,3 GB liberados") ou
`disk-still-short` — este o único evento do catálogo sem `auto-read`, porque o que falta liberar é
decisão da pessoa, e dito UMA vez por aperto: outra folga que acha o mesmo não o repete, só uma que
abriu espaço antes. O primeiro `Mind` que o rodapé vê é a base, e um ensaio nunca chega ali. O btrfs
devolve o espaço quando a transação fecha, não quando o arquivo sai; a folga pede `syncfs` aos dois
sistemas de arquivos antes de dizer que terminou, senão "liberado" sairia menor e "continua sem
espaço" seria dito de um disco que não está.

**O que ela precisa.** Uma sessão gráfica: o cérebro roda numa. Uma máquina em que ninguém entra
graficamente fica com o piso do Nix durante os builds e a aparagem a cada ativação, e nada que ande
um cache. As máquinas dos testes de VM desligam a tarefa (`disk-space` no `brain.json`, ao lado do
`system-update`): um disco de teste pequeno cruza o piso fácil, e a coleta levaria os caminhos que um
teste registrou e não enraizou.

Ensaio num disco de 1,9 TB, sem apagar nada: o cache fica acima da meta e nada é levado; forçando a
meta a 99% e o teto a 1%, 13 173 unidades, 76,8 GiB, de 95,8 GiB até 19,0 GiB, em 5 s.

### Um arquivo de swap, e o offset dele

O `disko` cria e ativa `/swap/swapfile`. O instalador criava um **segundo**, media o `resume_offset`
dele e o escrevia no `host.nix`: 16 GiB de disco desperdiçados e um offset apontando para o arquivo
que o kernel não usa — uma hibernação que falha em uso, não no boot. O passo saiu; o offset vem do
arquivo do disko.

E faltava `boot.resumeDevice`. Com `boot.initrd.systemd.enable`, o nixpkgs só emite `resume=` quando
essa opção existe: a linha de comando levava `resume_offset=` sozinho, que é meio caminho.



### Quando o rodapé morre — a linha que sobrevive a ele

O dono passou o ponteiro pela linha de dicas e a interface inteira fechou. Ele apontou dois defeitos
naquilo, e só um deles era a queda: *"o outro é o erro de que a ronoctua-distro não sabe lidar com
isso, ou seja, acaba deixando o usuário sem interface"*.

Ele estava certo, e era pior do que parece. **Com o rodapé morto, os quatro binds de emergência estão
mortos junto.** `Ctrl+Alt+L`, `Ctrl+Shift+L`, `Ctrl+Alt+Esc` e `Ctrl+Alt+Del` são todos
`ronoctua-shell command <id>`, e todos escrevem no socket de controle **do rodapé**. Dentro de um
compositor vivo, com o rodapé caído, não havia caminho nenhum de volta pela tela — a porta do
`Ctrl+Alt+Del` que sobrevive é a do systemd, e ela só dispara num console que o compositor não está
segurando.

**A detecção é `OnFailure=`, e custa zero.** Não há thread, não há relógio, não há pergunta: o
systemd já sabe que a unidade falhou e levanta a outra. Isso responde ao que o dono pediu — *"caso
haja algo do brain procurando erros o tempo todo, faça isso da melhor maneira possível, de forma a
não consumir muita memória CPU/RAM"* — melhor do que qualquer coisa que procurasse.

E foi **medido**, porque a sabedoria recebida diz o contrário: `OnFailure=` está documentado como
disparando quando a unidade entra em `failed`, e uma unidade com `Restart=` e sem teto de reinício
supostamente nunca chega lá. Sondado aqui com duas unidades transientes — `Restart=on-failure`,
`RestartSec=1`, `StartLimitIntervalSec=0`, um filho saindo com 7 — a testemunha disparou em **toda**
falha, três de três. Um `ExecStopPost` teria sido o plano B e não foi preciso.

**O brain fica de fora, e isso é decisão.** O contrato dele está escrito neste documento: *"não
desenha, não segura teclado, não abre `/proc` e não reimplementa Nix"*. Ele é um decisor com fila e
registro de tarefas, e uma queda não é tarefa agendada. O dono sugeriu que a detecção fosse dele; o
que ele pediu de verdade — detecção que não custe nada — o systemd entrega sem o brain existir no
caminho.

**A linha é uma SEGUNDA superfície layer-shell, e o invariante 14 foi levado ao dono antes de ser
contornado.** Lido ao pé da letra, "nenhuma chrome além do rodapé" proíbe exatamente isso. A leitura
que ele aprovou: não é uma segunda chrome, é a linha do rodapé desenhada por outro processo — ela
existe precisamente quando não há chrome nenhuma, fica onde o rodapé fica, e sai numa tecla.

Duas linhas: o que aconteceu (e o que a unidade disse de si, `Result=`), e quatro opções com uma
letra cada — ver os logs, reiniciar, IA, fechar. A IA é **desenhada e recusa**, com frase própria
dizendo que ainda não existe: é o molde da linha do XFS no formatar, e é o oposto de uma opção que
falta. O teclado só é tomado `OnDemand` — em repouso ela não tira o teclado de ninguém, e um clique
lhe dá o foco como a qualquer janela; as opções são uma TABELA justamente para que
`lifeline-reaches-nothing-the-keyboard-does-not` possa ler que cada uma tem letra.

É um subcomando do binário que já existe, ao lado dos outros vinte e quatro. Um crate próprio seria
um segundo fechamento carregando o mesmo wgpu para um processo que vive segundos por ano.

No laço aninhado não há unidade nenhuma — `nix/footer-single.nix` sobe o rodapé como filho comum e o
`dev.nix` espera pelo **compositor** —, então o próprio `footer-single` vigia o PID e levanta a mesma
linha. Ele distingue queda de recarga por um arquivo `.onpurpose` escrito antes do `kill`: sem isso
a linha nasceria a cada save, que é o alarme que ninguém lê depois da segunda vez.

### O ciclo que matou a shell duas vezes

`Footer::metric` é o fundo da aritmética desta porta: ele transforma largura, altura e a **primeira
linha da região** num frame, e essa primeira linha é o `first_of_expansion`. Portanto nada que o
`metric` alcance pode perguntar o que a região **contém** — no instante em que pergunta, a geometria
do rodapé vira função do conteúdo do rodapé.

Aconteceu duas vezes, com dois pares diferentes de funções, e as duas vezes o sintoma foi o mesmo:
stack overflow, `process::abort`, modeline e minibuffer simplesmente sumidos, sem diagnóstico nenhum.
A primeira foi `expansion` ↔ `expansion_window`. A segunda:

```
grid_full → expansion_len → screen_columns → metric → first_of_expansion → explain_rows → grid_full
```

e bastava o ponteiro parar sobre uma tecla da linha de dicas. Dois coredumps de 2026-09-15 carregam
essa pilha e mais nada.

Os dois consertos são um só: **as duas arestas gratuitas saíram**. `screen_columns` perguntava
`metric` só para ler `columns()`, que é `width / cell` e nunca lê `first`; agora constrói o frame com
`0`. E `explain_rows` media a região; agora **lembra** — `grid` escreve `region_can_spare_a_row` onde
a medição acontece, e a soma lê de volta. Custa um quadro: no primeiríssimo hover a linha ainda não é
oferecida, que é a mesma batida que o `hint_placed` já corre, e pela mesma razão.

O check que existia para a primeira vez, `manager-builds-its-own-rows`, escreveu no próprio
comentário o buraco por onde a segunda entrou: *"um par diferente de funções chamando uma à outra
passaria intocado, e não há check neste arquivo que pegaria esse"*. O
`the-footer-does-not-ask-itself` fecha isso: monta o grafo de chamadas `self.…()` entre os métodos do
`Footer`, exige que nada abaixo de `metric` alcance o conteúdo, e recusa qualquer ciclo que não esteja
declarado — as cinco recursões de despacho que existem estão listadas com o motivo, e uma declarada
que desaparecer também falha, porque linha que não exclui nada não exclui nada.

### O console de resgate — `Ctrl+Alt+Del`, e duas portas para a mesma tela

O rodapé é a **única** chrome (inv. 14): morto, não há modeline, minibuffer nem leader. Os três binds
fora da leader cobriam três casos e todos pedem `spawn` ao **compositor** — ou seja, os três somem
exatamente quando o compositor é o que quebrou. Faltava a porta que funciona quando nada funciona.

**O pedido tinha um conflito interno, e ele decidiu a arquitetura.** "Independente do compositor e
das ferramentas gráficas" e "matar janela com um clique do mouse" não cabem na mesma tela: o console
do kernel não tem ponteiro. A saída é **um binário, duas portas**:

| | com o compositor vivo | sem ele |
|---|---|---|
| quem abre | bind do compositor → janela flutuante | kernel → `ctrl-alt-del.target` → VT 8 |
| o que existe | mouse e teclado | teclado |
| do que depende | Wayland, wgpu, atlas de fontes | nada disso |

É a mesma tela, o mesmo binário. Por isso o resgate é uma **aplicação de terminal** e não um cliente
Wayland: uma janela de resgate que precisa do compositor vivo é a chave trancada dentro do cofre.

**Não é chrome** (inv. 14). A porta de cima é um `xdg_toplevel` que o compositor flutua por uma regra
`for_window` gerada — exatamente o que o `Ctrl+Alt+Esc` já faz com o btop, com um terceiro `app_id`
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

**Duas faixas, e a linha é a do dono:** ver é livre, agir prova quem é. Reiniciar rodapé, compositor
ou a sessão inteira não passa por polkit nenhum — não há display manager aqui, e essas são unidades **de
usuário**. O que toca o sistema, dá shell ou não se desfaz vai para `auth_self_keep`: a senha da
própria pessoa, uma vez, lembrada pela sessão. Quem pergunta é o polkit, com `pkttyagent` no VT — este
crate nunca vê senha, e um console de resgate é o pior lugar possível para errar autenticação.

#### O que ele não faz, e por quê

**Não edita o config do compositor.** `/etc/scroll/config` é link para o store e é gerado
(`nix/modules/scroll.nix`): editar seria imperativo (inv. 6) e sumiria no rebuild. Ele **abre em
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

**A reduzida não é outra distro.** Compositor, rodapé, fonte primária, teclado, rede, impermanência e
terminal ficam. O que sai é aplicativo — o que se instala para USAR, não o que é preciso para entrar.

Duas coisas apareceram ao medir isto, e as duas eram interruptores que não interruptavam:

- **`ronoctua.fonts.notoExtra` não fazia nada, e depois passou a fazer a coisa errada.** Primeiro
  o `noto-fonts` estava na lista incondicional *e* na opcional: o `vm.nix` desligava acreditando
  poupar e o pacote entrava pelo outro caminho. Tirada a duplicata, o interruptor passou a
  interromper — e aí o número que o justificava é que estava errado. A opção dizia *"the single
  largest item in the font closure (over 1 GB on its own)"*; medido com `nix path-info -S` contra
  a nixpkgs pinada, **49,1 MB**. Errado por vinte vezes, porque o número envelheceu quando o
  `noto-fonts-extra` foi absorvido no `noto-fonts` na 26.05 — a mesma renomeação que o AGENTS.md
  já anota, chegando como medição velha em vez de erro de avaliação. **A opção não existe mais**
  (decisão do dono, 2026-09-08, com o número na mesa): um perfil ficava sem árabe, hebraico,
  devanágari e tailandês para poupar cinquenta megabytes que lhe disseram serem mil.
- **`ronoctua.icons.superpack` não tira os 0,61 GiB do closure.** O `ronoctua-shell` carrega o
  caminho do superpacote GRAVADO por `option_env!` (`RONOCTUA_ICONS`), e é instalado nos dois
  perfis. Por isso ele está **fora** da tabela: listá-lo seria anunciar uma economia que não
  acontece. Fechar isso é mudança no shell, não na tabela — está no `PENDING`.

#### O ISO carrega o que a máquina alvo compilaria

Instalar custava uma tarde, e o motivo não era o download. `nixos-install` **constrói** o sistema: o
que vem do nixpkgs chega pronto do cache público, e o que este repositório produz não existe em cache
nenhum — o shell, o compositor do input pinado, o `libghostty-vt` com o Zig, os buffers wasm, a
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

### Secure Boot e TPM2 — e por que a distro NÃO os instala

**A decisão, tomada com o dono e com os números na frente: o disco pergunta a frase-senha uma vez,
no boot, e nada é perguntado depois.** Secure Boot e destrave por TPM ficam desligados. O que segue
é o que se aprendeu construindo os dois, e por que o que estava montado era pior que qualquer das
duas pontas.

**O que existia, e não funcionava.** O layout do disko pedia `tpm2-device=auto` e o instalador
rodava `systemd-cryptenroll --tpm2-pcrs=7+11+12+15` incondicionalmente — mas o `nix/modules/boot.nix`
que ligava o lanzaboote **não era importado por sistema nenhum**: as opções `ronoctua.boot.*` não
existiam em `noctua`, `minimal` nem `vm` (`error: attribute 'boot' missing`), e nenhum teste o usava
— o `secureboot-vm` monta a própria configuração à mão. Sem a corrente abaixo, os PCRs 11, 12 e 15
nunca são estendidos; ficam em zero, batem sempre, e o selo vira o PCR 7 sozinho. A máquina abria
sozinha para qualquer volume, e com o autologin da sessão um laptop levado desligado abriria direto
na mesa. Conveniência sem a proteção que deveria pagar por ela.

**Por que não ligar o Secure Boot em vez de remover o TPM.** É defensável e quase foi a escolha: ele
não custa nada no dia a dia (o lanzaboote reassina a cada `nixos-rebuild`) e fecha o ataque que a
cifra não fecha — trocar o kernel de uma máquina desligada por um que capture a frase-senha. O que
pesou contra foram os modos de tranca, todos reais: uma geração anterior à mudança não está assinada
e o rollback do NixOS deixa de funcionar para ela; o ISO deste repositório **não é assinado**, então
o resgate exige entrar na firmware antes; e perder `/etc/secureboot` ou resetar a firmware tranca a
máquina até reinscrever. Nenhum é tijolo — desligar o Secure Boot na firmware devolve tudo — mas o
sintoma é "o laptop não liga", que é o pior do sistema. Contra isso, o modelo de ameaça de um laptop
que anda com o dono é roubo, e a cifra em repouso já responde.

**A chave do Wi-Fi continua selada no TPM**, e isso não é contradição: é `systemd-creds`,
deliberadamente **sem PCRs** (com PCR, atualizar o kernel a quebraria). Selar uma credencial e
segurar a chave do volume são perguntas diferentes.

**O `secureboot-vm` e o `tpm-vm` ficam.** Eles montam as próprias configurações e provam que a
corrente funciona — são o registro executável de como ligar isto, no dia em que se quiser.

A corrente, para esse dia:

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

### O cérebro — `LEADER b`, e a linha que avisa antes

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
quiser uma) e um padrão em `prefs` (se ela tiver preferências). A LISTA desenha qualquer tarefa sem
saber nada dela: `brain-task-<id>` a nomeia, `<id>-does` diz o que faz, e `Task::kind` decide o
glifo e a cor. O que ela NÃO escreve, porque o
runner já é dono: a fila — no máximo uma tarefa por vez —, o portão de métrica, o aviso e a contagem
dele, o diário, o socket e toda tela que olha.

#### E o interruptor não alcançava a execução

Atrás daquele defeito havia um maior, e o dono decidiu consertá-lo: `m t` gravava no `brain.json` e
**nada na máquina lia esse arquivo**. Quem decide quais passos rodam é `/etc/ronoctua/updates.conf`,
gerado da opção `ronoctua.update.steps`, que precisa de um rebuild. A página oferecia uma tecla, a
tecla gravava um arquivo, e a sequência seguia rodando exatamente o que rodava — sem nada, em lugar
nenhum, dizendo isso.

O `turned_on` do script gerado passou a ler **três camadas**, da menos específica para a mais: o
padrão da distro, a tabela da máquina, e por cima delas os GIROS da pessoa, que o brain escreve de
`brain.json` para `$XDG_STATE_HOME/ronoctua/update-steps.conf` toda vez que as preferências mudam.
Uma fonte de verdade (`brain.json`) e uma vista derivada dela, na forma de uma linha por passo, que é
a forma que o script já sabe ler — a alternativa era ensinar um `writeShellApplication` a parsear
JSON, que é ou uma dependência nova ou um `grep` que um dia responderá com confiança e errado sobre
a máquina de alguém. É a mesma relação que o fragmento do compositor tem com o `theme.json`.

**A camada da pessoa é só da metade dela**, e isso é uma fronteira e não um esquecimento: a metade
root roda como root, cuja casa é `/root`, e não há jeito honesto de ela descobrir de quem foi a
sessão que a iniciou. As formas de dar-lhe um acabam todas num diretório de root que um membro de
`users` pode escrever — numa máquina compartilhada, qualquer conta decidindo o que um update de root
roda. Os seis passos root ficam com a opção do Nix, e a página os desenha RECUSANDO com a frase que
diz de onde vem o valor — a mesma resposta que a vista do Syncthing dá para as catorze opções que o
flake fixa (invariante 6).

E o que prova isso é um check que **roda a função gerada** em vez de ler o gerador. A primeira forma
dele lia o código-fonte pelo nome do arquivo, e uma sabotagem passou direto: apontar a guarda para
`/nonexistent` deixando o redirecionamento embaixo manteve o nome no texto e quebrou a
funcionalidade. Um check que procura uma palavra mede como a fonte está escrita, que não é a
propriedade.

#### `você mudou` sobre seis passos que ninguém tocou

O dono abriu a página do update e viu seis linhas dizendo `you changed`, numa máquina onde não tinha
mudado nada. Perguntou o que aquilo queria dizer. Era um defeito, e a forma dele vale guardar.

`TaskPrefs::steps_off` era *"um conjunto de exceções e não uma lista do que roda"*, e o comentário
que dizia isso estava certo sobre o que o campo é e errado sobre o que ele supõe: um conjunto de
"desligados" só consegue expressar uma coisa — *a pessoa DESLIGOU um passo que vem ligado* —, e
portanto assume que **todo passo vem ligado**. O `assets/updates.nix` entrega **seis dos doze
desligados**.

A aritmética era `on = !contains(id)` e `changed = contains(id) == on_by_default`. Com o conjunto
vazio, para cada um daqueles seis: `on` dava LIGADO — o contrário do que a distro entrega, com o
tique aceso — e `changed` dava verdadeiro. E apertar a tecla piorava ao contrário: o passo entrava no
conjunto, `on` virava falso e `changed` virava falso, então a linha passava a dizer "não mexido"
**depois** de a pessoa ter escrito no arquivo dela.

O conserto tem duas metades. A exceção virou um **giro sobre o padrão declarado** —
`on = on_by_default ^ flipped`, `changed = flipped` —, com o nome do campo mudado para
`steps_flipped` e um `serde(alias)` no antigo: sob a leitura velha uma entrada só podia ser um passo
ligado que alguém desligou, e sob esta ela quer dizer exatamente a mesma coisa, então nada em disco
precisa migrar. E o que **atravessa** deixou de ser `changed` e passou a ser `on_by_default`: o padrão
é o fato DECLARADO e o único dos dois que não é derivado, "mudou" é uma subtração que qualquer um com
os dois faz, e a tela ganhou de graça a capacidade de dizer de que um passo foi mudado.

#### Uma tarefa pode ser uma VIGIA, e isso mudou duas coisas

A tarefa que olha o daemon do Emacs não se parece com as outras três em nada. As três fazem e
relatam; esta só OLHA, e o resultado inteiro dela é uma frase. Duas consequências, e as duas valem
para a próxima vigia que aparecer.

**A cadência dela é em minutos.** `interval_days` é a unidade certa para um rebuild e inútil aqui: o
daemon pode cair às dez da manhã, e uma checagem que voltasse amanhã deixaria alguém sem a sessão do
editor por um dia enquanto a tela dizia que estava tudo em ordem. `TaskPrefs::every_minutes` é
`Option` e vale para quem a declara — `schedule::next_moment` curto-circuita o calendário inteiro na
primeira linha quando ela está lá, porque não há janela para achar, dia para caminhar nem fuso para
consultar, e `local_day` arredondaria cinco minutos para "hoje" e devolveria sempre o mesmo instante.
Não é um knob: a escada de cadência que a tela percorre é em dias, e uma tarefa que pensa em minutos
não tem degrau nenhum ali para onde ir.

**E ela levanta um AVISO** (`task::Notice`, por `Report::say`), que a faixa do rodapé desenha. Uma
frase que só aparecesse numa tela que alguém tem de abrir seria uma vigia que ninguém ouve. Veio pelo
`Report` e não por um quarto `Outcome` nem por um método a mais no trait: `run` recebe `&self` e
devolve o que FEZ, um aviso não é isso, um quarto `Outcome` faria todo leitor de um outcome aprender
sobre avisos, e um método a mais não teria onde a tarefa lembrar o que acabou de achar. O `Report` já
é o canal de volta de uma execução.

O aviso carrega CHAVES e um id de COMANDO — nem o rodapé nem um componente inventa palavra — e **pode
não ter botão**. O do "este Emacs não é o cliente" não tem um de propósito: o que está aberto é o
trabalho de alguém, com buffers que nada aqui sabe se foram salvos.

O trait tem oito métodos e **só o `run` toca a máquina**. Os outros — como se chama, de que tipo é,
quando vence,
do que precisa, por quanto tempo avisa, quais são os passos — não recebem relógio, não abrem arquivo
e não sobem processo. É isso que permite provar um ANO de agendamento em milissegundos, com um
`Clock` de teste, sem esperar um segundo sequer.

#### Não há uma segunda fronteira com o `/proc`

O `ronoctua-memory` continua sendo a única — o mesmo critério do `ronoctua-scroll` e do
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

#### A atualização é uma vista, com as duas portas

`LEADER s u` virou grupo: `s u u` abre a vista no rodapé, `s u w` na janela — a porta que a marca
animada da modeline abre — e `s u n` começa a atualização agora. As duas portas mostram a mesma
`update_window::View`: os passos declarados com o veredito de cada um, o que o passo sob o cursor É
(o segundo registro, com o motivo de um pulo) e as duas saídas, a da metade root e a da pessoa, ao
vivo por dois `journalctl -f` que a porta mata ao fechar. As teclas são uma tabela só, ensinada nas
duas e clicável nas duas: as setas andam nos passos, `C-j` e `C-k` andam a saída uma página, `G` vai
ao fim dela, `u` começa a atualização pela mesma porta do `s u n` (`update_window::begin`, com a frase
que diz se foi pelo cérebro ou pelo systemd), e `a` e `p` respondem à contagem do cérebro pela mesma
decisão dos botões da faixa (`brain::answer`: adiar enquanto só está anunciado, abortar quando já
roda, e dizer que não há nada quando não há). A janela só olhava: não tinha cabeçalho, não começava
nada e não respondia à contagem.

Cada porta arruma a vista no seu espaço. A janela a divide em duas metades, a dos passos e a da
saída, cada uma com a sua rolagem. A região do rodapé tem teto de linhas, e nela a vista é UMA lista
— o segundo registro, os passos, uma linha de ar e a saída —, que a rolagem, a barra e a roda da
região já leem; ela conta as linhas sem construí-las e constrói só as que desenha, porque a saída
guarda até duas mil. Uma linha que chega de um diário não move a página, com a exceção que um log
tem: a página que a pessoa levou ao fim da saída continua no fim enquanto o fim anda.

#### A faixa é a superfície do rodapé crescendo PARA BAIXO

Espelho da faixa de notificação, que cresce para cima. Uma superfície só, sem popup e sem card
(invariantes 14 e 15), com o glifo do cérebro na margem dizendo quem está falando — ela chega sem
ninguém ter pedido.

Ela é **colada embaixo do minibuffer**, e não a última linha da superfície. O terceiro motivo é o
que decide: uma linha verdadeiramente última moraria em `first_of_expansion() + expansion_rows()`, e
teria de ser recalculada no pintor, no hit-test e no quadrilátero de fundo, para sempre — que é
exatamente o defeito que o `first_of_expansion()` foi escrito para remover. Colada, ela custa **um
termo** na soma das linhas (`brain_line_at` é `modeline_line() + shape.lines() + fold_rows() +
explain_rows()`), e todo leitor rio abaixo acompanha de graça. Os outros dois motivos são que foi
assim que se pediu, e que ela mantém o rodapé simétrico — faixa · modeline · minibuffer · cérebro ·
expansão —, de modo que esta linha se mexe na tela exatamente quando a modeline se mexe. Com as
barras ocultas o `shape.lines()` pode ser zero: a linha do cérebro sobe para onde elas estariam, com
o fundo dela, e no modo que empurra ela empurra as janelas enquanto está no ar (ver "Os modos
ocultos").

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

### Idioma e região — `LEADER s l`, duas portas que digitam e dois escritores de um arquivo

A vista é uma `View` (`locale.rs`) com duas portas: a região do rodapé (`s l l`) e a janela
(`locale_window.rs`, `s l w`). As duas leem uma tecla pelo mesmo leitor, `locale::press_of_key`,
pela notação inteira da `OF_THE_VIEW` — e foi isso que fez `z`, `a` e `c` existirem: a linha de
atalhos das duas portas as ensinava, e em nenhuma a tecla, ou o clique nela, fazia alguma coisa.

**As duas portas digitam.** O fuso desta interface, um lugar novo, onde fica um lugar e a data, a
hora e o fuso da máquina são cada um um `Asking`, perguntado na linha da própria porta — o
minibuffer no rodapé, o cabeçalho na janela (`Ui::ask`) — sob o comando de `Asking::command`; a
resposta das duas vai para `locale::took`, que confere antes de escrever, e colar é o `C-v` da
linha. A janela dizia "digitar é no rodapé", e as três perguntas da máquina nem no rodapé chegavam a
lugar nenhum: eram feitas sob ids que nenhum comando tinha, e a resposta rodava um comando que não
existia. **Um lugar é nomeado pelo FUSO**, lido quando a pergunta abre, e nunca pela linha, que a
outra porta pode ter mexido entretanto. `c` pergunta sobre o lugar sob o cursor — era "o lugar
zero" — e, longe de qualquer lugar, diz por quê; a coordenada que ele pede não liga o tempo, e a
pedida pelo interruptor do tempo (`Asking::Weather`) liga.

**O arquivo é o broadcast, e uma porta escreve uma MUDANÇA, nunca a cópia inteira.** `locale.json` e
`world.json` têm dois escritores, e cada porta escrevia a cópia que tinha lido ao abrir — desfazendo
o que a outra tivesse escrito depois: um fuso escolhido no rodapé voltava ao antigo no próximo anel
girado numa janela aberta antes, e um lugar acrescentado sumia no próximo interruptor. Agora a vista
entrega a mudança (`Setting`, `GlanceChange`, o lugar pelo fuso) e a porta a deita sobre o que o
arquivo diz na hora de escrever (`prefs::with`, `glance_change`). E as duas portas vigiam os dois
arquivos: o `locale.json` pela `Door` de toda janela, porque as datas de toda janela seguem a forma
escolhida, e pelo rodapé, que só o lia ao nascer; o `world.json` pelo rodapé e pela janela desta
vista. A vigia relê só os arquivos (`View::reread_files`) — o barramento e o soquete do brain
respondem o mesmo de um instante atrás.

O `world.json` se lê com `parsed` antes de escrever: quem regrava a lista sobre o padrão que um
arquivo quebrado devolve apaga os lugares dele. O `locale.json` não, de propósito: todo campo dele é
um anel da tela, então o que um arquivo quebrado perde é o que a tela já mostra como padrão — e
recusar deixaria os quatro anéis sem girar até alguém consertar o arquivo à mão.

### O relógio da máquina — `ronoctua-clock`, e por que o automático é o padrão

A tela de idioma e região mudava o relógio, a data e o fuso da INTERFACE e nada do computador. O
dono chamou isso de configurações incompletas, e tinha razão: o que alguém quer dizer com "o fuso
horário" é o da máquina.

**Um crate fala com o `timedate1`**, molde do `ronoctua-disks` e do `ronoctua-power`: `Standing` sai
de uma chamada de propriedades, três ordens saem por método, e as REGRAS — o que é editável dado o
que o daemon respondeu — ficam num `lib.rs` sem zbus, testáveis contra retratos copiados de uma
máquina de verdade. O `timesync1` é lido no mesmo crate e só para saber QUEM está sendo perguntado;
nada escreve nele, porque o interruptor que a pessoa quer dizer é o `SetNTP` do `timedate1`.

**O automático é o padrão porque já existe um cliente.** O `systemd-timesyncd` disciplina o relógio
continuamente, corrige deriva em vez de dar saltos e sabe até onde confiar numa resposta. A tarefa
`clock-sync` do brain é a VIGIA e o INTERRUPTOR: ela confere, religa o automático quando o encontra
desligado, e diz quando o relógio está à deriva. Uma tarefa que buscasse a hora por HTTPS a cada
seis horas seria um segundo acertando este relógio, menos exato que o primeiro e discordando dele —
e o sintoma de dois donos de um relógio é um carimbo que anda para trás.

**O que o automático mantém não se digita.** O daemon recusa `SetTime` enquanto o NTP está ligado, e
a razão é boa: a próxima checagem poria de volta. As duas linhas continuam DESENHADAS e recusam com
uma frase, em vez de sumirem — uma linha que não é desenhada e uma que não pode ser usada são a
mesma coisa vista de uma cadeira, e só uma delas se explica.

**O fuso passou a ser da pessoa, e isso é uma decisão sobre o invariante 6.** `time.timeZone = null`
não é uma brecha: é o NixOS declarando que o `/etc/localtime` é do `timedated`, e é a forma que
`ronoctua.power.profile` (um piso que um arquivo vence) já tem. Preenchendo `ronoctua.time.zone`, o
flake retoma o fuso, o NixOS põe `NIXOS_STATIC_TIMEZONE=1`, e a linha é desenhada RECUSANDO — que é
o que as catorze linhas fixas do Syncthing já fazem. O instalador continua PERGUNTANDO o fuso; o que
mudou é que ele semeia `/etc/localtime` no alvo em vez de escrever uma linha no `host.nix`.

**Custa uma regra polkit, e ela é a oitava.** Três ações (`set-ntp`, `set-time`, `set-timezone`), um
usuário, `YES`. A quarta que o upstream declara — `set-local-rtc` — fica de fora porque nada nesta
árvore a chama, e uma ação concedida que ninguém chama é uma porta aberta para ninguém. O
`timedate-polkit-is-narrow` lê a regra instalada e a constante `ronoctua_clock::timedate::ACTIONS`
nos DOIS sentidos.

---

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
acordado* ela entrega ao rodapé pelo soquete de controle (`Ui::tell_the_footer`), com o estado QUERIDO
como argumento (`power-keep-awake on`/`off`, `power_words::keep_word`) — nunca uma inversão que o
rodapé decidiria de novo por uma leitura um instante mais velha ou mais nova —, e um rodapé que não
está lá vira frase no cabeçalho. A leader roda o mesmo comando sem argumento, e aí ele inverte o que o
logind diz. E os dois interruptores são lidos de
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
passava fica exatamente onde estava — e a regra alcança só o que é desenhado CENTRADO na célula
(`Size::OnTheCell`, e `Size::Lowered` com tamanho, a legenda que o teclado na tela baixa até o meio
de uma linha):
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
(`br.ronoctua.network`). O que a `View` guarda é um cursor, um arquivo lido esperando resposta, a
senha do hotspot enquanto ele está de pé e sobre o que é a pergunta aberta — nenhuma LEITURA: o `Snapshot`, os pontos de acesso, o
`/proc` e os dois túneis chegam num `Facts` na hora de pedir as linhas. **Um parâmetro e não
quatro**, porque uma assinatura que cresce com as seções é uma assinatura editada nas duas portas, no
clique e em cada teste toda vez que uma seção nasce.

**O que se digita é decidido pela `View`, e as duas portas digitam.** Três coisas aqui pedem texto: a
senha de uma rede protegida sem perfil, o caminho de um `.conf` ou `.ovpn` e um ajuste do Tor que não
é anel nem interruptor. A `View` decide que uma linha é necessária (`Errand::Ask` com uma `Question`:
o id do comando e se é segredo) e guarda sobre o que ela é — a rede, o ajuste —, porque quando o
texto volta a única outra testemunha seria o cursor, e uma varredura que chega no meio o move. Cada
porta abre a própria linha — o minibuffer no rodapé, o cabeçalho na janela (`Ui::ask`, com colar) —
e entrega a resposta ao mesmo `View::answered`: a senha junta a rede perguntada; o caminho é lido
por `network::read_tunnel` (teto de 256 KiB, `~/`, recusa com o nome do arquivo) e o cursor cai em
"trazer", com a leitura do arquivo acima; o valor do Tor substitui, e numa lista acrescenta — vazio
limpa. A tecla também é lida de um jeito só, pela notação inteira (`network_words::press_of_key`):
lida sem o modificador no rodapé e pelo texto na janela, o `S-<right>`/`S-<left>` que a ficha do Tor
ensina não fazia nada em nenhuma das duas.

**A pergunta "guardar a senha?" é uma tela da `View`**, nas duas portas: a pergunta no topo, `não` e
`sim` embaixo dela como linhas que a tecla e o clique respondem (inv. 19), e a lista embaixo como
contexto — toda linha dela vira título enquanto a pergunta está de pé, então o cursor não anda até
ela e o clique não a pega. O `não` vem primeiro, o cursor nasce nele e o `Esc` também é `não`. A
`View` guarda só o nome da rede: a senha fica na porta que a digitou e sai de lá nas duas respostas,
ou quando a vista fecha. O `Errand` que a leva ao NetworkManager mascara o `Debug`
(`network::Password`), e a linha do rodapé que registra a resposta do minibuffer não escreve o texto
de uma pergunta secreta (`Minibuffer::answered_a_secret`) — antes, a senha do Wi-Fi ia para o
journal como "argumento" do `network-connect`.

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

**`ronoctua-memory::net` é a fronteira do `/proc/net`** — a mesma regra que faz o `ronoctua-scroll` o
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

### As telas — `LEADER s d`, uma `View`, duas portas, e uma pergunta que se responde sozinha

O molde é o da rede e o da energia, e a razão é a mesma: o rodapé tem teto de doze linhas (inv. 14) e
a ficha de UMA tela tem dezoito. `s d d` desenha na região; `s d w` abre a mesma `View` numa janela
(`br.ronoctua.display`). Um crate é a fronteira (`ronoctua-display`): o compositor por IPC, os backlights por
`/sys`, os monitores externos por DDC/CI escrito por nós sobre `/dev/i2c-*` — sem `ddcutil`.

Nada se digita nesta vista, e não há o que colar nem soltar em nenhuma das portas; o que as duas
dividem é o leitor de tecla (`display_words::press_of_key`): setas, `RET` e `Esc` com o que estiver
segurado, e as letras pela notação inteira da `OF_THE_VIEW`. O rodapé largava os modificadores e
tomava `C-a` por `a`; a janela lia o caractere que o xkb digitou e tomava `C-a` por nada.

**O que é diferente de toda outra vista é o RELÓGIO.** Uma mudança de escala, de resolução, de taxa ou
de giro pode deixar a tela ilegível, e quem não consegue ler a tela não consegue desfazer. Então a
mudança é aplicada PROVISORIAMENTE (`Order::Try`), uma pergunta toma a região, o cursor nasce em
"voltar atrás", e em quinze segundos sem resposta ela volta sozinha. O relógio é o MONOTÔNICO: com o
de parede, um passo de NTP para trás congelava a contagem para sempre, que é exatamente o resultado
que a constante existe para impedir.

**Há DUAS naturezas de pergunta e elas têm padrões opostos.** A de cima aplica primeiro e se responde
sozinha, porque o risco é não conseguir dizer nada. Esquecer (`d`) não aplica nada, não tem relógio e
espera — o risco ali é perder decisões, e um relógio que responde sozinho seria a forma errada. Uma
mudança numa tela APAGADA não é prevista de jeito nenhum: não há imagem para comparar, então é
gravada direto.

**"Manter" grava UM botão, o que foi perguntado, e o que foi PEDIDO.** Gravar os três que a leitura
sabe (escala, modo, posição) tirava a pessoa da escolha automática do compositor em todos eles na
primeira vez que ela mexesse em qualquer um; e ler o valor de volta trocaria "escolha você" pelo
número que ELE escolheu — a prévia e a coisa mantida deixariam de ser a mesma mudança.

**O arquivo é o que sobrevive ao reboot; o IPC é o que faz o compositor concordar com ele.** E
escrever sozinho não faz NADA, porque ele não vigia arquivo nenhum — não há inotify nele. Por isso
`Order::Keep` grava e então PEDE a releitura.

**Desfazer é UM comando, e isso é propriedade deste compositor.** `load_main_config` libera
`config->output_configs` e reconstrói do arquivo, então toda ordem `output` mandada em execução deixa
de existir de uma vez. O anterior fazia o contrário — PRESERVAVA o transitório quando o arquivo
relido era igual ao anterior —, e desfazer custava seis chamadas por tela, inclusive dos botões que
ninguém tinha tocado, porque "não escolhido" era uma instrução e não uma ausência.

**Uma frase não é uma linha, e uma ordem que não muda nada não é uma ordem.** As duas regras foram
escritas cada uma num lugar e valiam só ali. A prosa: a pergunta mantinha suas duas frases fora do
cursor e as outras seis da vista usavam a metade que não pulava — no primeiro quadro de uma máquina
cujo fragmento não parseia, a mensagem de erro da própria vista acendia na cor do foco como se
alguém a tivesse escolhido, e `RET` a copiava. Agora só existe um jeito de desenhar frase. A ordem:
`would_move` recusava a PERGUNTA que não mudaria nada, e a fila alcançava só os botões com relógio;
quando a posição deixou de perguntar, `a` sobre ela apagava uma posição já vazia e mandava um `Keep`
mesmo assim — uma leitura, uma reescrita do `display.json` e seis chamadas de IPC, uma por repetição
de tecla, redesenhando byte a byte. A regra mora em `keep_for` agora, que compara o `Wanted` que
montou com o que está no arquivo, e vale para todo botão que grava.

**Sair com uma pergunta de pé devolve a mudança primeiro.** A prévia vive na configuração EM
EXECUÇÃO do compositor, e o que a derruba é esta thread pedindo a releitura — `Order::Undo`. Uma
porta que marcava `quit` e voltava largava o remetente, `main` acabava, e o processo levava a thread
junto: a ordem enfileirada nunca era cumprida e a mudança ficava de pé até o compositor reiniciar. Pior:
`previewed` é o conjunto DAQUELA thread, então uma porta aberta depois manda um `Undo` que não envia
nada — não havia volta de dentro do shell. `Handle::finish` manda e ESPERA (o canal entrega o que
está na fila antes de dizer que o remetente sumiu), e as duas portas o chamam: a janela ao ser
fechada, o rodapé ao sair. **A espera tem PRAZO — três segundos — e não é um `join`**: o
pedido comum do adaptador não tem tempo de leitura, então um compositor que aceita a conexão e nunca
responde seguraria a thread, e um `join` seguraria a PORTA — uma janela que não fecha. O prazo é o
que a thread pode honestamente levar: uma leitura profunda (um quinto de segundo por porta, sobre
quantas portas a máquina tiver) mais a releitura do `Undo`.

**Cada porta escreve de volta só o que ELA mudou.** São duas `Eyes` — uma por porta —, e cada uma
guarda a leitura que a última ordem dela deixou. `write` levava as telas SEMPRE, no argumento de que
"as telas são desta thread": não são, há duas. Um `Order::Text` não tira leitura própria, então
mexer no tamanho do texto numa janela escrevia as telas velhas dela por cima do arquivo — apagando
do `display.json` E do fragmento uma escala que a outra porta acabara de gravar. Agora há uma
bandeira por metade (`wrote_the_text`, `wrote_the_screens`) e a mesclagem é uma função pura,
testável sem disco. **E o fragmento só é reescrito quando os BYTES dele mudam** — agora pelo que vem
DEPOIS da escrita, e não pela escrita: este compositor não vigia arquivo, então o rename não custa
nada; o que custa é a releitura que o `Keep` manda em seguida, que é uma carga completa da
configuração e uma leitura nova em cada porta. Pular a escrita é o que pula isso.

**E uma recusa derruba só a pergunta dela.** A resposta de uma ordem chega depois de uma ida e volta,
e nesse meio-tempo a pergunta pode ter sido respondida e outra ter subido — `→`, `Esc`, `→` de novo
são duas perguntas sobre a MESMA tela dentro de uma ida e volta, que é o caso que o comentário antigo
dizia ser impossível. O conector não basta; o `Ask` também viaja na ordem e é ele que as separa.

**A janela tem relógio ARMADO, e é a única assim** — quatro outras batem sempre, pela vida da
janela, e a do gerenciador arma uma só quando o inotify falha; o do rodapé não alcança esta: são dois processos
com duas `View`s. Ele é armado só enquanto há uma pergunta com relógio de pé e se desarma sozinho, de
modo que em repouso a janela não acorda nenhuma vez. Ela também ouve DOIS sinais de que uma tela
mexeu — o `wl_output` do Wayland, que ela já recebe por ser cliente, e o evento `output` do próprio
compositor, que é o mais largo dos dois (dispara também em modo, escala e releitura) — e o
`display.json` inteiro. **O terceiro sinal que existia morreu com o compositor anterior**: havia um
evento dizendo que a última carga de configuração falhou, e era por ele que a porta que ESCREVE o
fragmento sabia que ele fora recusado. Este não publica nada disso e o `reload` responde `success`
haja o que houver, então quem responde agora é o portão de BUILD.

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

#### A notificação não toca som, e isso é uma decisão

O campo `audio` existe ponta a ponta — protocolo, catálogo, filtro, gerenciador, disco — e não faz
nada. Ele não está mudo sobre isso: `ronoctua_notify::INERT` diz por quê, o gerenciador desenha a
razão ao lado do interruptor, e o `switches-that-do-nothing-say-so` cobra que continue dizendo.

Isto foi item do `PENDING` por meses, com o caminho medido (o `pw-play` que o pipewire já traz, mais
uns 400 KB de `sound-theme-freedesktop`) e uma pergunta aberta que era do dono: qual som para qual
nível. **Em 2026-09-10 ele respondeu que está fora do escopo**, e o item saiu de lá. Fica escrito
aqui para que não volte como dívida: o interruptor continua inerte de propósito, e a tela continua
dizendo isso.

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

**E os dois programas não são mais chamados daqui.** Este crate os rodava e analisava o JSON por
conta, o que fazia DOIS leitores de um daemon dentro de um repositório que escreve um cabeçalho
inteiro contra isso (`ronoctua-disks`, "uma fronteira, um crate"). No dia em que discordassem sobre
um volume, nada diria qual estava certo. A análise mudou para o `ronoctua-audio`; o que ficou aqui
é a pergunta do Bluetooth feita a ele, POR ENDEREÇO. Cobra o `one-pipewire-boundary`.

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
As teclas são LETRAS NUAS — o que se digita vai para a linha da porta, que toma toda tecla enquanto
pergunta —, lidas da própria `OF_THE_VIEW` pelas duas portas, de modo que uma letra acrescentada à
tabela é uma tecla que funciona e uma tecla fora dela é uma tecla que ninguém ensina. O leitor é um
só (`bluetooth_words::press_of_key`), pela notação INTEIRA: o rodapé montava a notação só com o
shift, e `C-f` virava `f` e esquecia o aparelho sob o cursor, enquanto a janela, lendo o caractere
de controle, não fazia nada. O Syncthing tinha a mesma divisão (`C-d` esquecia uma pasta) e ganhou
o mesmo leitor único (`syncthing::press_of_key`). **Uma pergunta de pareamento toma a tela** (nada pareia até ela ser respondida) e o `Esc` ali
responde NÃO, em vez de deixar o outro lado esperando os sessenta segundos do bluetoothd. Quando a
vista NÃO PODE abrir — o gerenciador de arquivos está no rodapé, segurando uma pasta que alguém
navegou —, a pergunta vira NOTIFICAÇÃO, com os dois botões que rodam os mesmos comandos do `s b y` e
`s b n`; e ela é RETIRADA quando a pergunta acaba, porque um cartão crítico com dois botões que não
respondem mais nada é pior que cartão nenhum. Pergunta que pede DÍGITOS (um PIN) não aceita um "sim"
seco de lugar nenhum: pede os dígitos na linha da porta em que se está — o minibuffer, ou o cabeçalho
da janela —, MASCARADOS nas duas, senão o que sairia daqui é um PIN vazio. Um nome novo também se
digita nas duas portas, partindo do nome que a coisa já tem (`View::name_being_changed`), e a vista
guarda O QUE está sendo renomeado — o aparelho, ou esta máquina — e diz o que a resposta vira
(`View::renamed`), a mesma chamada nas duas.

**A pergunta de pareamento se responde também na janela.** O agente é do rodapé — o bluetoothd
pergunta ao agente padrão, e só uma porta pode registrá-lo —, então a pergunta chega ao processo do
rodapé. Ele a escreve num arquivo do diretório de runtime enquanto ela existe (`bluetooth::standing`,
`0600`, por tela, com o pid de quem escreveu) e o apaga quando ela acaba; a janela vigia o arquivo por
inotify e desenha a pergunta como o rodapé desenha, com `y`, `n`, `Esc`, `RET` na linha e os dígitos
na própria linha. A resposta volta ao agente pelo soquete de controle (`bluetooth-accept`,
`bluetooth-refuse`) — nunca num argv, e o rodapé só registra o id do comando. A resposta da janela
NOMEIA a pergunta para a qual foi dada — o caminho do aparelho e o que ele perguntou, antes dos
dígitos (`bluetooth::Answer`) —, e o rodapé só a aceita se é essa a pergunta de pé; o agente, por sua
vez, só responde a pergunta que ainda espera (`Questions::answer`). Entre a janela ouvir o arquivo e
responder pelo soquete, a pergunta pode acabar e outra tomar o lugar, e o "sim" dado a um aparelho
confirmava o seguinte. Pelo mesmo motivo, a linha de dígitos do rodapé sai quando a pergunta acaba
ou é trocada, como a da janela já saía. Um arquivo cujo
escritor morreu não é pergunta: o agente foi junto, e a janela lê nada. E quando a pessoa ESTÁ na
janela — o foco é dela, que foi onde o `p` foi apertado —, o rodapé não abre a região nem toma o
teclado: é na janela que os dígitos vão ser digitados. Uma pergunta ainda de pé quando a janela fecha
VOLTA ao rodapé (`bluetooth-open`), e a vista do rodapé abre sempre sobre a pergunta que estiver de
pé: fechar a janela não responde por ninguém, e responder não por ela seria decidir pela pessoa.
Se a região está ocupada e a vista não pode abrir, a pergunta vira o cartão, como quando chega com
a região ocupada — senão ficaria em tela nenhuma até o agente desistir.

**A frase de uma linha aparece sob o ponteiro nas duas portas.** Um fato sobre o qual ninguém tropeça
duas vezes por ano carrega uma frase inteira (`Row::description`); o rodapé a põe na linha de
mensagem, e a janela na linha 0 do cabeçalho (`door::list::header_about`), abaixo da pergunta e da
mensagem e acima dos atalhos — a mesma ordem.

---

### Teclados — `LEADER s k`, e por que o ajuste por dispositivo mora abaixo do compositor

O compositor tem **um** `input type:keyboard { … }` para todos os teclados da máquina: um layout, uma taxa
de repetição, e nenhuma forma de nomear um dispositivo. Não é esquecimento — as sete seções de
entrada dele são por TIPO de dispositivo, e o `apply_libinput_settings` ramifica pela capacidade do
aparelho, nunca pelo nome. O IPC também não expõe dispositivo de entrada nenhum: as dezesseis
requisições vão de `Outputs` a `Casts` e não há `InputDevices`. O invariante 3 fecha a porta de
mudar isso aqui.

Então **layout, tecla de acentos e repetição são globais por construção**, e a tela diz isso em vez
de oferecer um ajuste por teclado que não existe. O que sobra por dispositivo é o remapeamento, e
quem o faz é o `keyd`, abaixo do compositor, casando por `vendor:product`.

**A regra do keyd que decide todo o desenho**: *um id de dispositivo só pode estar listado em um
arquivo de configuração*. O arquivo da distro reivindica `*`, então um teclado com regra própria tem
de SAIR do coringa — e um teclado fora do coringa não tem regra nenhuma, a não ser que o arquivo
dele as repita. Uma dessas regras é a leader.

Daí as três consequências, e nenhuma delas é escolha de estilo:

1. **O diretório é da máquina, não do store.** `/etc/keyd` é link para `/var/lib/ronoctua/keyd`,
   criado por `systemd-tmpfiles` com posse do usuário da sessão. A subtração abaixo do `[ids]` muda
   quando alguém configura um teclado, e caminho do store não muda.
2. **A regra da leader é declarada UMA vez** (`assets/keyboards.nix`) e repetida em TODO arquivo
   renderizado. O módulo a semeia — para a primeira inicialização, antes de o shell ter escrito
   qualquer coisa — e o `ronoctua-keyboards` a escreve em cada arquivo. Cobra o
   `keyd-rules-carry-the-leader`, e a falha que ele impede é precisa e silenciosa: configurar o
   teclado é como se perderia a tecla que abre tudo, NAQUELE teclado, com o arquivo válido e o keyd
   satisfeito.
3. **Nada é instalado sem o keyd ter lido.** `keyd check` recebe ARQUIVOS — medido contra o
   binário, porque as três grafias plausíveis fazem três coisas diferentes: com os arquivos ele
   recusa o que não entende (255), com `-c <dir>` ele recusa tudo (o `-c` vira nome de arquivo), e
   com o diretório ele aceita tudo (um diretório não parseia nada). A primeira versão disto usava a
   do meio, que teria feito da funcionalidade uma tela que nunca escreve.

O buffer não tem tabela nossa: os teclados, as teclas oferecidas e os significados CHEGAM
(`input::keyboards`), cada um com a palavra do keyd e a chave de texto que o host resolve, e ele
devolve três strings que o host confere contra o que mandou (invariante 11 intacto). E `kbd` no
handler não faz um teclado: o botão de energia, a tampa, o barramento de vídeo, o alto-falante e o
controle do headset o carregam também — medido em 2026-09-08, nove dispositivos desta máquina o
carregavam e DOIS eram teclados. O filtro é o alfabeto, como o próprio keyd faz.

### O player — `LEADER p`, um processo, e a coluna à direita

#### Um processo, o libmpv e os decodificadores

**Por que um daemon.** O dono pediu o mesmo vídeo no rodapé e numa janela com UM áudio, vários
vídeos presos a um áudio, e duas máquinas tocando juntas. As três coisas são a mesma: um relógio
mestre e vistas que o seguem. Um processo por vista seria N decodificações e N áudios que se
desalinham; um processo que decodifica UMA vez e publica quadros num anel de memória por vista
(`ronoctua-frames`, o mesmo do estúdio, que subiu para ser comum) está junto por construção.

**Por que libmpv, e por que FFI à mão.** O pedido era "todos os formatos do VLC e do DeaDBeeF". O
libmpv sobre o `ffmpeg-full` cobre o primeiro (libopenmpt, gme, modplug, bluray, dvdnav, dav1d,
libass) e a render API de software entrega o quadro na CPU, que é onde o anel precisa dele. Medido
na fase 0: 1,34 ms por quadro a 1080p, o processo inteiro a 40% de um núcleo, e o `hwdec` não
mudou nada (o quadro volta para a CPU de qualquer jeito). A FFI são quinze funções; um gerador de
bindings traria o cabeçalho inteiro e um `clang` na build para usar quinze. O preço é o layout sem
conferência, e é por isso que o `mpv-layout-matches-c` compila uma sonda contra os cabeçalhos
pinados — ele achou o `cancel_fn` que o `mpv_stream_cb_info` ganhou na API 1.106 e a struct não
tinha (40 bytes contra 48; funcionava porque o libmpv zera a struct antes de a entregar).

**Por que o motor é outro crate.** Uma feature `engine` no `ronoctua-player` não basta: o workspace
unifica features, e o shell passaria a linkar o libmpv no dia em que o daemon fosse compilado ao
lado. `ronoctua-player` é o cliente, o protocolo e as regras puras; `ronoctua-player-engine` é o
binário, o único que cita o libmpv, e é extraído (`rust.player`) como o estúdio. Dentro dele, cada
coisa de que o daemon é dono é um módulo de `engine/` — as vozes (`voice`), a sessão de faixas
(`session`), a janela da fila no mpv (`queue`), o que se faz do que chega (`resolve`), os
decodificadores (`decode`), as saídas de som (`sinks`), as legendas e os sons ambientes, as
etiquetas e os rascunhos (`tags`, `drafts`), as buscas (`lookups`, `desk`, `fetch`) e os diretórios
(`directory`) —, e o `engine.rs` ficou com o soquete, a trava e o despacho dos pedidos.

**O que só o DeaDBeeF abria.** Um decodificador de linha de comando por família, escrevendo WAV (ou
PCM cru) num cano que o mpv lê por um protocolo nosso (`ronoctua-decode://`,
`mpv_stream_cb_add_ro`). Duas medições: todo WAV escrito em cano diz tamanho ZERO — o escritor não
pode voltar para corrigir —, e o leitor do ffmpeg acredita e chama o primeiro pacote de corrompido;
por isso o cabeçalho é reescrito na fronteira (`decoders::endless`), mantendo o formato do
decodificador (o do sidplayfp é MONO). E o libsndfile não abre FIFO para escrever WAV, então o
`fluidsynth` sai em RAW no stdout. O AdPlug não existe no nixpkgs e é empacotado aqui
(`nix/adplug.nix`). O `uade`, a música do Amiga, é o único decodificador não livre, e entra PELO
NOME: um nixpkgs próprio cujo predicado admite só ele, então a tabela (`nix/player-tools.nix`) nunca
pede `allowUnfree` e nenhum outro pacote não livre chega por ela. Os rips de console (PSF, PSF2,
USF, GSF, 2SF, NCSF, SSF) vão pelo `zxtune`, e o Super Audio CD pelo `sacd`.

**O que o libgme lê precisa do demuxer nomeado.** O ffmpeg reconhece VGM, NSF, SPC e os irmãos com
uma nota de sondagem de 25, e o libmpv exige 26; sem nome, todos terminavam em "formato não
reconhecido". Baixar o limiar para todo arquivo deixaria um palpite fraco ganhar noutro lugar, então
o nome vai só para essas extensões, nas opções do `loadfile`.

**O perfil mínimo não leva o player, e a razão é um número.** libmpv sobre `ffmpeg-full` é um
fecho de 1 487 MiB e os decodificadores mais 670 — 1 682 MiB juntos, quase tudo alcance do próprio
`ffmpeg-full` (python, samba, opencv, flite, qtbase). Um sistema mínimo já não tem mpv nem VLC; o
player segue a mídia na tabela do perfil, e uma linha o liga. Sem ele, as teclas dizem que ele não
está instalado — "não responde, sobe sozinho" seria uma frase falsa ali.

#### A fila é uma árvore nossa, e o mpv toca uma janela dela

**A fila é do daemon, e é uma árvore.** A fila (`ronoctua_player::queue`) guarda arquivos,
conjuntos, playlists com nome e streams, e cada entrada tem um `EntryId` que o daemon nunca reusa:
o id da faixa no MPRIS, o cursor de uma porta e "tocar esta" sobrevivem a mover, e a playlist que
um XSPF, uma pasta ou uma lista do YouTube trazia continua com o nome dela — achatada na playlist do
mpv, ela o perdia. O que toca depois, com repetir e aleatório (uma permutação que começa pela
atual), é resposta da árvore, nunca do mpv. Os tetos — 10 000 folhas e quatro níveis, mais os de
cada conjunto — são recusas com o número, e uma recusa deixa a fila exatamente como estava.

**O mpv toca duas entradas dela.** Áudio sem lacuna entre dois arquivos só existe dentro da
playlist do mpv (`gapless-audio`, `prefetch-playlist=yes`), então ele recebe a folha que toca e a
seguinte, e nada mais. Cada entrada do mpv é reconhecida pelo id que o próprio mpv dá a ela, e todo
evento é conferido com a playlist dele como ela está AGORA: um evento pode ser mais velho que um
`loadfile` feito depois, e "a fila acabou" só vale com o `idle-active` relido. As opções de um
arquivo — início, `referrer`, `user-agent` — vão no `loadfile` dele, com o comprimento na frente
(`%len%`, porque um `user-agent` pode ter vírgula): o `start` global passava para o arquivo
seguinte. Stream não tem seguinte no mpv: um endereço que falha passa ao próximo relé, e só com
todos falhando a frase é dita. Se o mpv abre sozinho uma entrada como lista (uma lista dentro de
lista, um arquivo compactado), ela vira uma playlist do que ele achou, com o mesmo id.

**O que o daemon faz do que recebe.** Pasta vira playlist com o nome dela (subpastas primeiro,
depois as mídias pelo nome; ocultos, capas, cue sheets e listas ficam de fora, e pasta por link
simbólico não é seguida). Lista local — m3u, pls, XSPF, ASX — vira playlist com o nome que ela dá a
si mesma (`<title>`, `#PLAYLIST:`), senão o do arquivo; `.m3u8` com `#EXT-X-` é uma mídia só (HLS);
lista de rádio num servidor vira UM stream com os relés dela; lista do YouTube vira playlist dos
endereços dos vídeos (`ronoctua_ytdlp::flat`), nunca dos endereços resolvidos, que expiram. O
disco é lido antes da trava do motor; a rede, numa thread (`player-resolve`), com um marcador no
lugar que já tem o id e que tocar espera. Quem abre de fora — o gerenciador, o
`ronoctua-shell play`, o `OpenUri` do MPRIS — põe logo depois do que toca e toca, como o `P` da
página de arquivos. As listas são lidas pelo MESMO leitor da prévia do gerenciador
(`ronoctua-playlist`): o daemon não pode linkar o `ronoctua-inspect`, que traz todo decodificador
da prévia, e dois leitores seriam duas respostas sobre o que uma lista contém; e o XML delas passa
pela guarda de profundidade (`ronoctua_xml::parse`, veja "Grafo de crates").

**A fila sobrevive ao daemon.** Ela é gravada em `queue.rqueue` (o formato do player, abaixo) toda
vez que a forma dela muda, e o `player.json` guarda o caminho da folha atual NA ÁRVORE, a posição, o
volume, repetir e aleatório — por evento (troca de folha, pausa, parada, volume, fechar), nunca por
relógio. O id não é gravado: um caminho na árvore continua valendo quando o próximo daemon dá ids
novos. A versão 1 (a fila plana e um índice) é importada uma vez.

**O nome de uma entrada.** Um arquivo é o título que uma lista ou quem o pôs deu a ele, senão o
nome do arquivo; uma playlist, o nome dela; um conjunto, o nome salvo, senão o do `.rset` de onde
veio, senão todas as fontes distintas juntas (`queue::together`), cortadas por grafema na largura —
nunca o nome de um arquivo só quando tocam vários. O conector é da porta, por chave
(`player-set-joiner`); onde o próprio daemon diz o nome — o título no MPRIS, uma lista que o mpv
expandiu — ele junta com ` + `, um sinal e não uma palavra de língua nenhuma.

**Os arquivos são do player, e a pasta vai inteira ao daemon.** O player lista a própria biblioteca
(`LEADER p l`, uma página da vista) em vez de mandar a pessoa ao gerenciador: começa na pasta de
música, gira para vídeos e clipes musicais (a décima primeira pasta do `nix/user-dirs.nix`), e só
mostra o que toca — o critério é UM (`player_browse::glyph_if_played`). Uma pasta vai ao daemon
como UM endereço, e é ele que a caminha e a faz uma playlist com o nome dela (a fila é uma árvore:
uma pasta achatada em arquivos deste lado perdia o nome); a porta só confere, antes de mandar, que
há algo nela para tocar, e diz se não há. `P` toca AGORA, logo depois do que toca, e a fila fica —
como um arquivo aberto de fora; recomeçar a fila é só o "abrir fila" do próprio player.

**A fila é uma árvore na porta também** (`player_queue`). Uma playlist com nome guarda entradas, e
um conjunto guarda faixas; os dois são GRUPOS, a linha encabeçada por um triângulo (`md-menu_right`,
`md-menu_down` — não os chevrons da gaveta, que abrem uma LINHA para baixo) que um clique vira na
hora: é um controle, e não a primeira metade de um duplo clique. `TAB` vira o grupo sob o cursor, `→`
abre e `←` fecha. **`C-↑` e `C-↓` movem a entrada sob o cursor — um arquivo, uma playlist, um
conjunto — uma casa entre as irmãs** (`player-earlier`, `player-later`, `Where::OnAnEntry`; decisão
do dono, 2026-09-29): as setas puras andam nas linhas e, num grupo, o dobram, então mover um GRUPO
precisava de outra tecla, e a mesma serve a toda entrada — as setas de lado numa folha deixaram de
movê-la. A tecla é lida pela notação inteira antes da seta pura (`player::press_of_key`), nas duas
portas; o `RCtrl ↑↓` da gaveta é o Ctrl da DIREITA, e continua dela. A linha do cursor leva no fim
um `↑` e um `↓` que fazem o mesmo para o ponteiro (invariante 19), e nunca os triângulos, que a
linha de um grupo já leva na cabeça. O número de
uma entrada diz o grupo em que está (`3.2.`): numerada por nível, `4.` de uma playlist ficava logo
acima de `4.` da fila. O
que o grupo guarda recua `view.nestCells` por nível, e os nomes abaixo de um grupo começam depois do
nome dele, mesmo num nível sem grupos. Qual grupo está aberto é da VISTA, por id: como a pessoa
deixou, e — um que ninguém tocou — aberto enquanto o que toca está nele. O que toca ENTRANDO num
grupo o abre — só entrando: uma playlist fechada enquanto toca não abre de novo a cada faixa —, e o
cursor segue a linha em que estava, porque as linhas que abrem acima dele levavam o `RET` a outra
entrada; sair dele nunca o fecha, senão a fila andando tirava de baixo do cursor a linha em que
ele estava. Um grupo fechado com o que toca dentro veste o tom do que toca. Um conjunto que toca mostra as faixas dele sob a linha, cada uma pelo glifo do tipo; um
que não toca diz quantas tem — o estado só traz as faixas do que toca. O nome de um conjunto que
ninguém nomeou é TODAS as fontes dele juntas (`player-set-joiner`), cortado por grafema na largura
da linha; nunca o de um arquivo só. E toda linha é a entrada pelo id: o esboço chega inteiro a cada
mudança de forma e a porta põe o cursor de volta na entrada, onde quer que ela tenha ido
(`View::outline_heard`, o mesmo caminho nas duas portas).

#### Duas mensagens inteiras, e o resto a pedido

**O daemon manda duas mensagens, cada uma inteira** (decisão do dono, 2026-09-28): o `State`,
pequeno, a cada evento — a posição no máximo quatro vezes por segundo —, e o `Queue`, o contorno da
fila (id, nome, tipo, filhos e quantas faixas tem cada conjunto), só quando a forma dela muda e a
quem pede (`queue`). As faixas de um conjunto dentro da fila fariam cada movimento de um volume
reenviar a fila inteira; por isso as faixas e a disposição do que toca vão no `State`, e a fila leva
só a forma. Um observador recebe o contorno e depois o estado, e o estado diz de que revisão da fila
ele é (`revision`). O contorno sai sempre antes do estado que vai com ele, e a porta guarda o último
de cada um.

**O que é grande ou raro é pedido, nunca transmitido.** A folha de um arquivo (etiquetas, fluxos,
capítulos), a página de uma busca ou de um diretório e o caminho de um rascunho vão só para a
conexão que pediu (`Say::Sheet`, `Say::Desk`, `Say::Draft`); o `State` leva a geração de cada uma
(`sheet_generation`, `desks[].generation`), e a porta que mostra aquilo pede de novo quando ela
anda. Quase nenhuma porta mostra a folha, e uma página de trinta filmes a cada quarto de segundo
seria o estado inteiro de novo.

**Recusa, nunca silêncio.** Um pedido que o daemon não entende recebe `not-understood`; antes era
silêncio, e uma porta nova falando com um daemon velho não sabia que não tinha sido ouvida. Toda
recusa vai também para `State.troubles`, porque a maioria das portas manda sem esperar resposta.
Cada `Trouble` é uma frase da porta, uma por variante, e passar de um teto é `too-many{what, most}`,
com o número.

**Uma linha, uma escrita.** O daemon monta a linha inteira e a escreve de uma vez. Serializada
direto no socket, cada token era uma escrita; o socket Unix conta o custo de cada uma no buffer, e
uma porta que parou de ler por um instante recebia meia linha, cortada pelo prazo de escrita. A
linha de um broadcast é serializada uma vez para todos os observadores.

**Uma porta mais nova que o daemon o troca, perguntando.** O `State` diz a versão do protocolo
(`protocol`, hoje 2); um estado sem ela, ou mais velho, faz a home dizer que o player é antigo e
oferecer reiniciá-lo, com o "não" primeiro. Reiniciar é o `Quit` do MPRIS — o único jeito de fechar
um daemon que talvez não conheça o `quit` do soquete, e que todo daemon antigo responde —, esperar o
soquete parar de responder (no máximo 3 s) e subir o daemon de novo pela unidade
(`client::restart`), numa thread.

#### `.rset` e `.rqueue`

**Um conjunto e uma fila são arquivos do player** (decisão do dono, 2026-09-28): `.rset` guarda
mídias tocadas juntas e os ajustes delas (as faixas, a disposição), `.rqueue` guarda uma fila —
conjuntos, playlists, arquivos e endereços. São JSON com `format` e `version`: o `format` separa
"não é nosso" de "é nosso e está danificado", e a versão recusa, com frase, um arquivo escrito por
um player mais novo. Campo desconhecido é ignorado; valor fora da faixa é recusado, nunca corrigido
— o escritor arruma o conjunto antes (`Set::tidy`) para nunca escrever o que o leitor recusa.
Caminho dentro da pasta do arquivo é gravado relativo (a pasta pode mudar de disco); o resto é
absoluto, e um absoluto que sumiu é procurado pelo mesmo nome ao lado do arquivo. Um stream nunca
aponta para um arquivo local: uma fila mandada a outra máquina não pode nomear um arquivo privado.
Um conjunto que entra na fila é conferido contra os tetos da sessão, e um `.rqueue` com um conjunto
de vinte faixas é recusado inteiro. Aberto de fora, um `.rqueue` entra como UMA playlist; só o
"Abrir fila" do player o faz a fila (`put` com `replace`).

**Os dois tipos são nossos na base de MIME** (`assets/player-types.nix`, lista `own`:
`application/x-ronoctua-set+json` e `application/x-ronoctua-queue+json`, sob `application/json`):
o gerenciador os mostra com o glifo de lista, a prévia os lê pelo mesmo leitor
(`ronoctua_playlist::own`) e o player é quem os abre. O `playlist-formats-are-total` lê a lista
`own` como lê as outras.

**Salvar e abrir são perguntas na linha da porta, e só "abrir fila" substitui a fila.** Salvar o
conjunto pergunta o nome — com o nome do que toca já na linha, que mudar começa do que é — e depois a
pasta, com a de música; um arquivo que já está lá só é regravado depois de uma pergunta com o
"não" primeiro, e um nome que não pode ser de arquivo ou uma pasta que não existe são recusados numa
frase. Salvar a fila é igual, sem nome de partida. Abrir é uma página dos arquivos com outro
propósito (`Purpose::Open`), que lista só `.rset` e `.rqueue` (`player_browse::Wanted`): um CONJUNTO
toca agora, logo depois do que toca, a fila mantida; uma FILA toma o lugar da fila, e com a fila
cheia pergunta antes, o "não" primeiro — é o único lugar em que o player substitui a fila. Renomear
pergunta com o nome atual na linha, pela linha "Renomear…" (o que toca) ou por `r` numa entrada; a
resposta vazia ou igual ao nome não muda nada. As respostas voltam a `View::answered`, que sabe do
que a pergunta era (`Typing`) e entrega o resto a `player_question::answered_saying` — um caminho
nas duas portas. A página de tocar junto (`Purpose::Lane`) lista também as legendas, que tocam sobre
uma imagem e nunca sozinhas na fila, e deixa as listas de fora, que não são faixa.

#### Faixas, vozes e o relógio

**Faixas e vozes.** O que toca são faixas (`Lane`): uma imagem, um som, legendas, cada uma com a
fonte, a trilha dela, um início no relógio do conjunto, um trecho da fonte (`from`/`until`),
volume, mudo, telas e saída — as grandezas com a unidade no nome (`start_seconds`,
`volume_percent`). Uma voz é um mpv: a imagem e o som de UMA fonte a partir de UM início e de um
trecho só (`session::voices`). Imagem e som da mesma fonte, juntos no tempo, dividem uma voz; o
segundo idioma do mesmo arquivo é outra voz, porque um mpv manda uma trilha de áudio a um destino
só. Legenda não tem voz: é desenhada na voz da imagem. Toda voz é aberta do mesmo jeito — com som e
com contexto de render —, e a que não toca som recebe `aid=no`; assim qualquer voz pega qualquer
parte, e a que já tem a fonte aberta a mantém: um filme que vira conjunto continua tocando onde
estava, e mover uma faixa no tempo nunca recarrega nada. Cada voz tem a própria thread de render e
a de eventos, que param pela flag dela — nunca por um índice numa lista que muda —, e é encerrada
FORA da trava do motor (as duas threads pegam a trava; juntá-las sob ela esperaria para sempre),
com o contexto de render liberado antes do mpv, como a libmpv exige.

**Os tetos.** 16 faixas, 6 legendas por imagem, 8 vozes e 6 imagens (`PICTURES_MOST`: vídeos
ligados e sons cuja capa aparece). O cenário do dono — três vídeos, dois sons e seis legendas —
cabe; passar é recusa com o número. Várias imagens em alta resolução custam CPU, porque o render é
por software, e o teto de imagens é o que protege a máquina.

**Um arquivo sozinho já é faixas.** Sozinho, um arquivo tem faixas implícitas — 1 a imagem (se ele
tem uma que não é capa), 2 o som, 3 as legendas —, cada uma com a trilha que o mpv escolheu.
Escolher outra trilha, ou ligar e desligar uma faixa, o mantém sozinho: é escolha do próprio mpv.
Qualquer outra mudança o faz conjunto, com o MESMO `EntryId` e sem parar de tocar. Um stream da
rede toca sozinho: os relés e os cabeçalhos dele não são de uma faixa. (A porta: "As mídias do
arquivo são páginas", abaixo.)

**O relógio.** O primeiro som ligado que está tocando marca o tempo; sem som, um relógio do próprio
motor, que espera as vozes carregarem antes de andar. Um som só assume o tempo quando pousou onde o
relógio está (a menos de 120 ms): um som ainda a caminho puxaria todas as outras vozes para onde
ele estava. Uma voz é mandada para onde deve estar enquanto assenta (4 s depois de carregar ou de
um seek) e empurrada depois disso: a imagem em 5%, o som em 2% — o mpv mantém o tom esticando o som,
e 5% se ouvem —, e um som mais de 400 ms fora pula. Medido no mpv pinado (saída nula): um som 300 ms
atrás volta em 14 s; uma imagem, em 6 s; um som 600 ms fora pula e pousa a menos de ~60 ms. Uma
faixa antes do início fica parada no primeiro quadro, e depois do fim, no último (`keep-open`); uma
faixa no fim não marca mais o tempo — o fim é onde ela parou, um quadro antes da duração —, e o
conjunto termina com a última imagem ou som. Uma thread de relógio só existe enquanto é preciso:
duas vozes, uma faixa esperando o início, ou o relógio do motor andando; uma voz sozinha avança o
conjunto pelos próprios eventos de posição. A thread acorda no início da próxima faixa, não um tick
depois.

**Nada é aberto pelo nome.** `sub-auto=no` e `audio-file-auto=no`, na fila e nos sons ambientes:
uma legenda ou um som é uma faixa que a pessoa vê e escolheu, nunca um arquivo que o mpv achou ao
lado do filme. Medido: com `fuzzy`, `long1.flac` ao lado de `long.mp4` virou o som do filme, e uma
máquina que o seguia pelo endereço não tinha essa trilha e tocou silêncio.

**A página de uma faixa** (`player_lane`, `PageId::Lane`) abre com `RET` na linha da faixa, onde
quer que ela esteja — a linha do tempo, a página das legendas, tocar junto, as faixas de um conjunto
na fila — e com o SEGUNDO clique na banda ou no nome dela num deck. Ela tem o que a faixa é e está
fazendo, o arquivo, o NOME dela (`RET` pergunta digitado, a linha abrindo com o nome de agora;
esvaziado, ela volta ao do arquivo — a regra do nome de uma entrada da fila; `LaneChange::Title`),
se ela toca (um interruptor), QUANDO ELA COMEÇA no relógio do conjunto (`←`/`→` meio
segundo; `RET` pergunta digitado, a linha abrindo com o início de agora — `m:ss`, `h:mm:ss`,
segundos, ou `+1,5`/`-0.3` a partir de onde está, com qualquer das duas marcas decimais
(`player_timeline::parse_start`) —, e o que não é tempo é recusado numa frase que o cita), ONDE ELA
PARA no próprio arquivo ("o fim", ou o tempo em que foi cortada: `←`/`→` meio segundo a partir do fim
do arquivo, que o player diz em `LaneState::duration_seconds`, e passar do fim a devolve ao fim;
`RET` pergunta digitado, relativo a onde para agora; um fim antes de onde ela começa no arquivo é
recusado numa frase; `d` devolve o fim — `LaneChange::Trim`), o volume
próprio e o mudo de um SOM (uma imagem não soa: a voz dela recebe `aid=no`, e legenda não tem som),
se a imagem de uma faixa — a de um filme, ou a capa de um som — aparece entre as imagens (o arranjo
INTEIRO vai com a faixa entrando ou saindo de `hidden`, ou de `covers`, `Ask::Arrange`), onde ficam
as linhas de uma legenda e SOBRE QUAL IMAGEM ela fica (um anel dos filmes do que toca,
`LaneChange::Over`; com um filme só, a linha é um fato), e "Tirar", que fecha a página de volta na
linha que a abriu. As três linhas — nome, fim, imagem — eram mudanças que o protocolo já tinha e
porta nenhuma oferecia (decisão do dono, 2026-09-29). O início se
escreve com o décimo de segundo depois da marca decimal da língua (`0:12,5`), a forma que a pergunta
lê de volta. Depois vêm PARA ONDE ELA VAI:

- **As telas de uma imagem** (a de um filme, ou a capa de um som): um interruptor para todas as
  telas e um para cada tela que a porta conhece pelo nome (`View::screens`, que a porta diz — a
  vista não conhece saída nenhuma), a embutida dita assim (`ronoctua_display::connector::is_internal`)
  e as outras pela descrição e o conector, porque dois monitores do mesmo modelo são duas linhas.
  Desligar "todas" deixa a imagem na tela em que a porta está; desligar uma tela de "todas" deixa as
  outras que a porta conhece. A última tela não é tirada, e isso é dito (`player-screen-last`): uma
  imagem em nenhuma tela é uma imagem escondida, e o interruptor dela é outro. Uma tela para onde a
  imagem vai que não está conectada aparece como "não conectada", nunca some: ela volta com o cabo.
  Uma porta que não sabe o nome de nenhuma tela diz isso. Legenda não tem telas: ela vai sobre a
  imagem dela.
- **A saída de um som**: a padrão da máquina e uma linha por saída que o daemon lê (`State::sinks`),
  com o glifo e a palavra do tipo (fone, alto-falantes, Bluetooth, outra) e "em uso" na escolhida;
  `RET` manda a faixa para lá (`LaneChange::Sink`). Uma saída escolhida que sumiu fica, dita "não
  está lá" — o som toca na padrão enquanto isso —, e sem nenhuma lida a página diz que só a padrão
  é conhecida. O daemon só lê as saídas enquanto alguém segura a conexão `Ask::Sinks`
  (`client::hold_sinks`), e a porta segura só enquanto uma linha delas está NA TELA
  (`player_lane::sinks_in_sight`, pelas linhas desenhadas e a rolagem): `player::SinksHeld` é uma
  thread que conecta e lê a conexão até uma das pontas soltar — a porta, largando a garra, desliga a
  conexão; o player, saindo, a fecha —, e largá-la solta o `pw-dump` do daemon. Conectar é dela,
  nunca do laço. Uma garra cuja conexão acabou diz isso (`SinksHeld::ended`), e o próximo estado que
  chega a larga para a porta segurar de novo: um player reiniciado nunca soube da garra antiga. A
  barra da coluna e a coluna estacionada não mostram linha nenhuma de saída, e largam. Uma saída é
  dita pelo nome que o servidor de som dá para pessoas, o de agora ou o último que a porta viu
  (`player::sink_label`) — `bluez_output.…` é o nome dele, não o da pessoa —, e duas do mesmo nome
  ganham um número.

#### Saídas de som

**Cada faixa de som vai à saída dela.** A lista vem do `ronoctua-audio`, a única fronteira com o
PipeWire, e só enquanto alguém precisa dela: uma porta que a mostra segura uma conexão aberta
(`sinks`), e uma faixa com saída própria precisa saber quando ela some. Soltar a assinatura mata o
`pw-dump` na hora. A faixa guarda a saída pelo `node.name`, e o mpv recebe
`audio-device=pipewire/<node.name>` — sem `ao=`: o nome do dispositivo já escolhe o driver. Se a
saída some, a faixa toca na padrão e isso é dito uma vez; quando a saída volta, a faixa volta para
ela. O player não mede a latência de um fone Bluetooth: a diferença para uma caixa se acerta
movendo o início da faixa.

#### Outras máquinas

**Remoto: por que a tailnet e por que um código.** A tailnet já é cifrada e já autentica máquina a
máquina; o daemon escuta só nos endereços dela e recusa conexão fora das faixas dela mesmo assim.
O que se acrescenta é que uma máquina da MESMA tailnet ainda precisa ser pareada — um código de seis
dígitos lido em voz alta, trocado por uma chave de 32 bytes. A mídia viaja por HTTP com `Range` na
mesma porta, porque é assim que o mpv busca um trecho; um endereço (YouTube) viaja como endereço.
O relógio é medido uma vez (oito idas e voltas, a mediana) e a correção é a mesma regra dos vídeos
locais — com uma janela de quatro segundos depois de cada carga em que quem segue SALTA em vez de
empurrar, porque a carga pela tailnet atrasa ~0,7 s e um empurrão de 5% levaria catorze segundos
para pagar isso. Medido entre duas instâncias pela tailnet real: ±70 ms desde o primeiro segundo.

**Outras máquinas, faixa por faixa.** O `Moment` leva as faixas do que toca; cada arquivo é servido
em `/media/<folha>/<faixa>` só enquanto aquela folha toca, e duas faixas do mesmo arquivo dividem o
caminho da primeira: um download e uma voz do outro lado. O papel escolhe as faixas — tela: imagens
e legendas; alto-falante: sons —, e as saídas e telas de quem é seguido nunca viajam. As legendas
de arquivo vêm inteiras (poucos KB) e são mescladas deste lado, como as daqui; uma trilha de dentro
do filme da outra máquina só o mpv tem aberta, e aparece sozinha. Seguir é uma sessão de faixas no
relógio da outra máquina: a fila de quem segue não é tocada, e ao sair a folha dela volta, pausada
onde estava. Enquanto segue, pausar, buscar, pular e mudar faixas é recusado com frase
(`following`): quem decide é a outra máquina. As duas precisam falar a versão 2.

#### Fechar em todo lugar

**Fechar em todo lugar** (`player-quit`, `LEADER p q`, e uma linha da vista que pergunta antes, com
o "não" desenhado primeiro e o cursor nascendo nele). O daemon recebe `Ask::Quit`: lembra a fila (a
desta máquina, se ela seguia outra), para o som, diz `Say::Farewell` a todo observador e fecha a
conexão, fecha as de imagem, tira o soquete do disco, responde `done` e sai com SUCESSO 200 ms
depois — o `Restart=on-failure` da unidade não o traz de volta. Sai por `_exit` (`mpv::leave_now`),
e não pelo `exit` da biblioteca C, que roda os `atexit` e os destrutores C++ com as threads do
libmpv ainda vivas: um que caísse ali seria uma falha, e a unidade traria de volta o player que
acabara de fechar. A saída é armada ANTES da despedida e espera por ela no máximo 5 s, e toda
escrita a um observador tem 1 s de paciência (`WATCHER_PATIENCE`): uma porta parada que não lê mais
segurava a trava do motor — e todo pedido atrás dela — para sempre. Depois da despedida nada mais
grava a fila: o `stop` que cala o som move o lugar do mpv nela, e o arquivo acordaria sem ela. A
despedida também cala os sons ambientes, para no meio uma gravação de etiquetas (o rascunho sai e o
arquivo fica como estava), apaga os rascunhos do editor e encerra as buscas e os diretórios. O MPRIS
`Quit` faz o mesmo. Do lado das portas:

- **O relógio da porta distingue despedida de sumiço** (`client::Heard`: `State`, `Farewell`,
  `Gone`, lidos por `player::hearing`). Toda porta fecha na despedida — a vista do rodapé, a
  janela, a janela de palco e a coluna — e NENHUMA fecha no sumiço: um player que caiu é um player
  que a porta diz não estar rodando, e ninguém o fechou de propósito.
- **Fechar nunca liga o player para desligá-lo, nem espera por ele.** `player::tell_if_running`
  escreve só a quem escuta (`client::tell_if_running`) e não lê a resposta: fechar e reler as
  preferências são os dois pedidos que o player não recusa, e o laço do rodapé esperava até 3 s
  por um player ocupado. Gravar uma preferência pede `reload` do mesmo jeito — as linhas das
  preferências existem sem player, e mudar uma depois de fechar em todo lugar o trazia de volta. O
  `client::preview` parou de chamar o `ensure`: a thread de uma imagem pergunta de novo a cada
  segundo, e trazia de volta um player que alguém acabara de fechar. E o `client::ensure` pede a
  unidade DUAS vezes: o soquete sai do disco antes de o processo sair, e um pedido nesse meio
  encontrava a unidade ainda ativa, não fazia nada, e a porta esperava um player que ninguém subia.
- **Sem daemon rodando, fechar ainda fecha as portas.** O rodapé (`Footer::player_quit`) fecha a
  vista e encerra as portas que são processos — a coluna (`right_column::end`), a janela de palco
  (`player_stage::end`) e toda outra janela, `player::end_the_doors`. Janela não tem slot, e são
  quantas a pessoa abrir, então cada uma guarda `player-window-<pid>.pid` enquanto está aberta
  (`player_stage::WindowSlot`, em volta da porta; a foto nunca o escreve), e o fim confere a linha
  de comando antes de acreditar no pid. Uma janela ou a coluna que fecha o player avisa o daemon
  ela mesma (se ele roda), encerra ela mesma as outras portas — sem rodapé, elas vão também —,
  entrega a mesma ordem ao rodapé, para a vista dele, e sai.

#### A marca da modeline

**A marca da modeline é um PAR: tocar/pausar e próximo, num segmento só.** O layout tira um
segmento removível INTEIRO, então dois glifos num segmento nunca se separam numa linha estreita;
dois segmentos no mesmo degrau se separariam (o empate vai para o último declarado) e os dois do
`segmentGap` entre eles leriam como marcas sem relação. O primeiro glifo diz o que o clique FAZ,
como o botão do transporte: pausa enquanto toca, toca enquanto está pausado (decisão do dono,
2026-09-28; antes dizia o estado). Entre os glifos, uma célula de ar
(`space.modeline.bondGap`) que não é de nenhum. Uma função pura, `player_mark::zone_at`, diz que
glifo está em cada célula do segmento, e os três leitores a leem: a pintura (um run por glifo, cada
um no tamanho de cromo dele — `glyph_and_text` dimensiona só um glifo à FRENTE, e o segundo sairia
no tamanho do texto), o clique e o hover. Em tocar/pausar, o esquerdo alterna (`p t`) e o direito
LEVANTA a coluna (`player-column raise`, nunca o alternar do `p c`); em próximo, o esquerdo avança
(`p n`) e o direito abre o player no rodapé (`p p`). Sob o ponteiro, o primeiro diz o que toca e o
segundo o que vem — o que o próprio player diz que toca depois (`State::next`, que já sabe do
aleatório e do repetir; a frase diz "na ordem embaralhada" com o aleatório ligado), um conjunto sem
nome dito pelas fontes dele juntas nesta língua, e que próximo para o player no último item. Uma marca que chega ou sai ao lado move o par sob um ponteiro PARADO, e nenhum
movimento vem dizer isso: depois do quadro, o segmento sob o último ponto do ponteiro é lido de
novo (`Footer::modeline_moved_under_the_pointer`), e um segundo quadro sai só se mudou — uma
rodada, porque ela lê a tabela que o próprio quadro pôs.

**O par da modeline com só o ambiente.** Sem nada da fila carregado — nenhuma folha atual, nenhuma
máquina seguida — e com um som ligado (`player::only_ambience`), o par aparece e é do ambiente: o
tocar/pausar pausa e retoma os sons, e o glifo diz qual (`player::toggle_ask`: `ambient-toggle`, o
mesmo que `LEADER p t`, o `SPC` da vista e o botão do transporte pedem); o próximo não tem para onde
ir, então é desenhado no tom de uma tecla que nenhum clique aperta, não pega clique e diz por quê
sob o ponteiro (`player_mark::inert`, a resposta que a pintura, o clique e a frase leem). As mesmas
células, as mesmas zonas. Com uma folha atual, mesmo parada, o par é da fila: um clique nunca pausa
a chuva no lugar de tocar a música. As teclas de mídia falam com o player pelo MPRIS, que só vira a
fila, e nunca chegam ao ambiente.

#### As três portas e a pilha de páginas

**As três portas fazem as mesmas coisas.** O rodapé, a janela e a coluna leem a mesma `View`, e o
que uma porta tinha e a outra não foi fechado nos dois sentidos:

- **Digitar é da porta onde se está.** O caminho ou endereço, o vídeo a seguir e o pareamento são
  uma `player_question::Question` que a vista entrega à porta: o rodapé pergunta no minibuffer, a janela e a
  coluna na linha do próprio cabeçalho (`Ui::ask`), e a vista continua aberta embaixo — o rodapé
  fechava a vista para perguntar e perdia a pasta e o propósito dos arquivos. A resposta vai a UMA
  função (`player_question::answered`), qualquer que seja a porta, e colar na pergunta vale nas duas. O
  pareamento são duas perguntas: o nome da outra máquina, e depois o código, mascarado — o código
  não entra em log nem em `argv`, e o `Debug` do pedido não o escreve (`player_machines::Code`); o nome
  fica com a PORTA entre as duas perguntas (`player_machines::Pairing`), porque `player-pair` também
  é pedido pelo nome (`LEADER :`), sem vista aberta. O código vai numa thread
  (`player_machines::pair_then`), nas duas portas: o player só responde
  depois de a outra máquina responder, o que pode passar dos três segundos que toda outra ordem
  espera — o laço ficava parado por eles, e a espera vencida dizia "o player não está rodando"
  enquanto o pareamento seguia. A espera agora é a do pareamento, e vencê-la tem frase própria
  (`player-pair-no-answer`).
- **Todo pedido vai numa fila, na ordem em que foi feito** (`player::tell_then`). O player atende
  cada conexão numa thread, atrás de um lock que não é por ordem de chegada: uma thread por pedido
  deixava uma busca arrastada pela barra cair num lugar anterior ao último, e duas coisas postas na
  fila trocarem de lugar. Então cada processo tem UM contador (`tell_in_order`) que manda um pedido
  por vez e espera a palavra dele antes do próximo, acha o player uma vez e só o procura de novo
  quando a conexão falha. Um valor dito de novo antes da vez dele — a posição, um volume — é dito uma
  vez, o mais novo (`supersedes`): um arrasto são sessenta pedidos por segundo. O `Put` tem fila
  própria, porque o player anda na pasta antes de responder, fora do lock, e uma pausa apertada nesse
  meio não espera a caminhada; dois `Put` continuam na ordem. Um `Put` ou um salvamento esperam um
  minuto; sem resposta nesse tempo, o player ainda está trabalhando (`Told::Working`) e nada é dito
  — "o player não está rodando" seria falso —, e uma despedida como resposta não é pedido aceito. Um
  player mais antigo fecha a conexão sem palavra diante de um pedido que não entende
  (`Told::Unheard`), e a porta cuja home já avisa isso diz a mesma coisa, não que ele não roda.
- **A posição e o volume são barras nas linhas**, nas duas portas: apertar põe o valor ali e
  segurar arrasta (`player_rows::slider_at` e `player_rows::slid`, a aritmética da barra de tempo
  da janela).
  A barra é o começo do valor que `chrome::aligned_pair` posicionou, lida de volta do campo — é pega
  onde foi desenhada —, e numa porta estreita ela cede as células para o tempo ser lido inteiro: um
  `12:…` lê como uma hora errada.
- **Parar é uma tecla da vista** (`s`), a mesma do botão da janela.
- **O rodapé tem uma coluna de mídia** (`player_media`). A região é larga e baixa, então o painel
  vai à direita das linhas, na largura da coluna nos três tamanhos dela — 72, 48 ou 32 células, o
  mais largo que deixa às linhas pelo menos a largura dele —, com a última célula para a barra da
  região (`player_media::footer_panel`, função só da largura: as linhas são dispostas no que sobra,
  e perguntar a altura da região de volta seria o rodapé perguntando a si mesmo). Ele existe
  enquanto algo está CARREGADO, e não só enquanto um vídeo toca: a imagem — um filme, ou a capa que
  o mpv decodifica como uma —, e sem quadro um CARTÃO DE TÍTULO, o que toca em palavras
  (`text.secondary`) no meio do lugar da imagem, sobre o chão da própria região — nunca sobre
  `surface.backdrop`, que não leva texto. No pé do painel ficam uma linha do transporte e a barra de
  tempo, as duas linhas da janela nesta porta: os botões são postos por `player_media::buttons` e a
  barra por `player_media::Track`, o quadro GUARDA o que pôs (`Footer::player_drawn`, ao lado do
  `modeline_placed`), e a pintura, o clique, o arrasto e o realce leem a mesma resposta
  (`panel_glyphs`, `panel_button_at`, `panel_track_at`). O botão sob o ponteiro se explica na linha
  de mensagem, nunca numa linha da região, que sairia de baixo da mão que perguntou; e um quadro que
  MOVE os botões (o glifo das mídias vem e vai com o arquivo) relê o botão sob uma mão parada, como
  o par da modeline, senão ela acendia um e o clique levava o vizinho. Ao lado das linhas o painel
  guarda as linhas da imagem em TODA página — 16:9 na largura dele, mais o pé
  (`player_media::panel_lines`), a região completada por linhas vazias até isso, sob o teto: a
  região tinha a altura das linhas, e uma página de duas linhas deixava o filme sem nenhuma. Abaixo de 32
  células livres o painel não cabe, e a home diz isso sob "Agora", com a tecla que mostra a imagem
  sozinha (`player-panel-no-room`) — uma recusa é uma frase; antes ele sumia em silêncio abaixo de
  97 colunas. Cada tela desenha as imagens roteadas a ELA (`player_media::placed_on_screen`), e o
  rodapé guarda uma imagem por faixa para todas — as faixas roteadas a alguma das telas dele
  (`player_media::lanes_shown`) —, pedida uma vez e desenhada onde vai. As chaves das texturas são
  `player:picture:lane-{faixa}:{conector}` (`Picture::key`): a textura de uma tela nunca é a de
  outra. A thread de quadros é a mesma da janela (`player_picture`), e só existe enquanto o painel
  mostra uma imagem.
- **`o` é o palco do rodapé, e o filme passa dos dois tetos.** A região vira o painel — a imagem, ou
  o cartão, com o transporte e a barra no pé —, as linhas somem e continuam respondendo às teclas, e
  `Esc` as devolve. O palco pode levar o rodapé inteiro a `expansion.playerStagePct` (70%) da tela, e
  o `maxLines` não o limita: quinze linhas de filme são uma tira (decisão do dono, 2026-09-28). Quem
  lê é o `expansion_ceiling`, a partir do campo GUARDADO `player_only`, nunca do que a região
  contém. Um clique na imagem toca ou pausa e o duplo clique a põe na tela inteira. Tela inteira e
  canto continuam sendo da janela, aberta já assim: uma faixa da tela não enche a tela nem flutua,
  que é a regra da coluna.
- **Toda porta troca a si mesma pela outra** (`player-swap`, a tecla `w`): o último botão do
  transporte da janela (desenhado como a coluna) e da coluna (desenhado como a janela), o último da
  barra, o último do painel do rodapé — que diz a frase dele, `player-swap-footer`: dali ela abre a
  janela no lugar da vista — e, só enquanto o painel não está de pé (nada carregado), o glifo da
  janela no título "Agora", como o de esvaziar a fila está no título da fila: os dois ao mesmo tempo
  eram um botão desenhado duas vezes. A janela pede ao rodapé que LEVANTE a coluna
  (`player-column raise`, `player::RAISE`: nunca o alternar, que fecharia uma coluna já de pé) e
  fecha; uma janela de palco faz o mesmo, e fechar é o que fecha o palco; a coluna pede a janela e
  fecha; o rodapé fecha a vista e abre a janela. A porta que troca fecha SÓ QUANDO O RODAPÉ ACEITOU
  a ordem (`Ui::hand_over`): com a linha de vida no lugar dele, ela fica e diz `door-no-footer` —
  nunca uma porta que some sem nada no lugar. As linhas "Abrir a coluna" e "Abrir numa janela"
  saíram da vista: o glifo de trocar e as teclas da líder fazem isso, e o ponteiro alcança a coluna
  pelo botão direito no par da modeline. A porta que troca é quem sabe qual é, então a vista devolve
  `Errand::Swap` e cada porta o executa; o que um botão faz sai de uma função só,
  `player_transport::button_errand`, lida pelo transporte, pela barra e pelo título.

**A vista é uma pilha de páginas** (`player_page`). Embaixo fica a home, que nunca sai; uma linha
que abre uma página a empilha (`Target::Go`), e `Esc` desempilha uma e põe o cursor de volta na
linha que a abriu PELA IDENTIDADE (o `spot` da linha): um estado que chegou no meio e moveu as
linhas da home — uma seção que apareceu acima — não tira o cursor do lugar. Só da home o `Esc`
fecha a porta, e uma pergunta aberta responde "não" a ele antes de tudo. As três portas guiam a
mesma `View`, e a janela e a coluna ensinam o `Esc` primeiro, clicável, como o rodapé
(`chrome::with_the_way_back`): um ponteiro que entra numa página e não sai dela é um ponteiro
preso.

- **A home diz o que é verdade agora, e nada que explique.** Na ordem: Agora (problemas, o que toca,
  estado, posição, volume, velocidade, repetir, aleatório; sem a linha do álbum, que é metadado),
  Mídias do arquivo (só quando o arquivo oferece escolha), Letra (só quando há, e uma linha Origem
  depois de ar dizendo de onde veio em palavras — `file`, `tag` e o host viravam "Letra de file", e
  escrito apagado sob as linhas lia como o último verso), Fila, Ouvir e assistir (Arquivos, e Sons
  ambientes com o que soa agora — o nome de um, quantos, ou "em pausa") e Recursos variados (Tocar junto, Salvar conjunto…, Salvar fila…, Abrir conjunto ou fila, Renomear…,
  Internet, Outras máquinas, Fechar o player em todo lugar). O que
  uma página é e por que existe é dito NA página: Internet, Outras máquinas e Tocar junto guardam as
  frases que a home tinha (`home_has_no_about_text`). A única frase da home é a recusa do rodapé sem
  espaço para a imagem, porque é o que é verdade desta porta agora, e o cursor fica na linha dele
  quando ela vem ou vai.
- **Uma chave por página**, `player-page-<id>`: a linha que abre diz ela, e a página tem ela por
  título (`PageId::key`). Duas palavras para um lugar eram o que o censo do
  `labels-of-a-view-are-distinct` achava assim que as páginas eram desenhadas.
- **Uma página cujo assunto sumiu fica, e diz** (`player-page-gone`): Tocar junto precisa de algo
  tocando, cujas faixas ela mostra. Sem player respondendo, a página diz isso, como a home.
- **Arquivos planos, nunca um diretório `player/`.** Nove checks leem os fontes do shell a um nível
  de profundidade (`glob("*.rs")`), e um diretório passaria em todos sem ser lido. As páginas moram
  ao lado: `player_rows` (a linha e o construtor), `player_page`, `player_home`, `player_internet`,
  `player_machines`, `player_question`, `player_tracks`, `player_subtitles` (a página das legendas),
  `player_ambience` (os sons ambientes e os créditos), `player_lane` (a página de uma faixa),
  `player_media` e `player_transport`. Por isso o censo de uma vista, nos dois
  checks que tratam a vista como um arquivo (`labels-of-a-view-are-distinct` e
  `the-cursor-is-a-colour`), é o arquivo com `struct View` MAIS os irmãos `<vista>_*.rs` sem vista
  própria que escrevem as linhas dela (o `Builder` pelo nome, como quer que seja escrito, ou `impl
  View`); lido sozinho, `player.rs` não desenhava rótulo nenhum das páginas.
- **`LEADER p m` abre a página das máquinas** (`player-machines`) e o pareamento é só pelo nome;
  **`LEADER p j`** (`player-together`) é a pergunta do que tocar junto, que era `p f`: uma faixa do
  tipo que o nome do arquivo diz (`player_browse::lane_kind_of`). Uma página
  aberta PELO NOME (um comando, `f`) volta, no `Esc`, à linha dela na página de baixo, e não a onde o
  cursor calhava estar; e uma página que já está na pilha é revisitada, não empilhada de novo —
  `p m` e `k` alternados empilhavam as mesmas duas sem fim.
- **Digitar é escolher.** O que tocar junto, digitado na página que o escolhe, fecha a página
  e diz o que foi feito, como o clique num arquivo; e um caminho digitado para tocar ou pôr na fila
  diz a frase que a escolha diria (`player_question::answered_saying`, `View::answer_came`).
- **Onde as linhas não são desenhadas** — o palco da janela e da coluna, o `o` do rodapé, a barra —
  uma tecla que age na linha do cursor não age numa linha que ninguém vê: as linhas voltam e a tecla
  é gasta nisso; uma que ABRE algo (`k`, o glifo das mídias, `f`, `c`, os comandos de página) traz
  as linhas e abre ali; e as de todo lugar agem (`Press::unseen`, a resposta que as três leem).
  Antes, o glifo das mídias no palco empilhava uma página atrás da imagem, e as setas seguintes
  trocavam o som às cegas. Uma janela aberta PARA o palco não tem linhas a devolver (sair dele a
  fecha), e diz onde elas estão (`player-stage-has-no-rows`). Na barra, `Esc` fecha a coluna
  sobre qualquer página.
- **O consentimento nomeia o host no RÓTULO** (`Letras do lrclib.net`, `Pular trechos patrocinados
  (sponsor.ajay.app)`): um rótulo quebra e nunca é cortado nestas páginas, e o host no valor saía
  `desligado (lrc…` na coluna pequena.
- **As mídias do arquivo são páginas** (`player_tracks`, `player_subtitles`): Som, Imagem e
  Legendas, cada faixa uma linha pelo nome dela entre as outras do tipo — duas que se leriam iguais
  ganham o codec e depois o número (`player::track_name`): lado a lado numa página, três `eng` eram
  três escolhas que ninguém distingue —, e a que está em uso diz isso no valor. A home só oferece um
  tipo de que haja mais de um — dois sons, duas imagens, uma legenda (que se desliga) —, e a linha
  da home abre a página no `RET` e gira a faixa em uso nas setas, o anel que sempre foi. CAPA não
  conta como imagem (`albumart`, que o mpv lista como faixa de vídeo: a embutida, o `cover.jpg` ao
  lado, o anexo do Matroska): o daemon a diz como capa e a porta a deixa de fora
  (`player_tracks::of_kind`), senão todo álbum oferecia duas imagens. As LEGENDAS são oferecidas
  sempre que há uma imagem, com ou sem trilha própria: a página delas é também de onde vêm as de
  fora (abaixo). O que toca são FAIXAS, e um arquivo sozinho já é as dele: escolher pede à faixa do
  tipo a trilha escolhida (`Ask::Track`) — a imagem também, desde que o player toca faixas —, e o
  arquivo continua um arquivo só. Uma trilha que nada toca leva no fim da linha
  o glifo de TOCAR TAMBÉM (`md-layers_plus`, e `a` na linha): uma faixa só dela, do mesmo arquivo,
  ao lado da que está em uso (`Ask::AddLane` com a trilha) — a segunda língua num outro fone, um
  segundo ângulo, uma segunda legenda —, e o arquivo vira conjunto sem sair do lugar na fila; a que
  outra faixa já toca diz "também toca". `k` e o glifo das mídias no transporte
  (`md-layers_outline`) existem com qualquer escolha (`TrackKind::with_a_choice`). Eles abrem o
  primeiro tipo com escolha, e numa dessas páginas o próximo, NO LUGAR dela: o
  `Esc` volta a de onde o `k` foi apertado, e não por cada tipo que ele andou. O transporte tem
  ONZE botões, e na coluna pequena eles cabem sem folga: um décimo segundo é uma decisão sobre o
  que sai.

**A imagem sozinha é um palco, e os controles são dela só enquanto há atividade.** Fora do palco a
faixa da imagem é um tom de superfície, e o quadro é pedido na proporção do vídeo para que ela
apareça; no palco a imagem toma a superfície e o preto é a tarja do próprio player. O transporte
volta por cima a cada gesto e sai sozinho, mas nunca sob a mão — um controle que some sob o ponteiro
que ia apertá-lo é um controle que ninguém aperta — e escondido ele não é alvo de nada.

#### A coluna do player

**A coluna.** Uma superfície layer-shell ancorada em cima, à direita e embaixo, com zona exclusiva
da própria largura: as janelas encolhem, nada é coberto. A zona de uma superfície é recortada do
que as superfícies criadas ANTES dela deixaram — é assim que o compositor arranja a camada —, então
a coluna termina onde a área do rodapé começa e nunca estreita a modeline, o minibuffer nem a faixa.
A ordem é a armadilha: um rodapé que sobe DEPOIS da coluna seria arranjado depois dela e
estreitado; por isso ele, na partida, derruba e levanta de novo uma coluna que já esteja de pé,
no espaço e no tamanho que ela tinha. Uma por espaço de trabalho, e a base é de todas (veja "As
colunas").

**A coluna e a tecla alternam, e "está de pé?" é uma pergunta sobre a COLUNA, não sobre um
número.** O `column-player.pid` sobrevive a uma coluna morta sem `Drop` (morta a
sinal, ou levada com a sessão), e o número dele volta a outro processo; perguntar só "o PID vive?"
fazia o interruptor mandar `TERM` a quem o herdou, responder "fechei" e não abrir nada. Por isso a
linha de comando é lida: este programa, aberto como a coluna do player (`column`, `column player`,
com o que mais a abertura disser). `LEADER p c` fecha a coluna quando ela está NESTE espaço, e a
traz para cá quando está em outro.

**A coluna pede ao compositor que avise as janelas, porque o commit que a mapeia não avisa.** A
zona exclusiva só conta com a superfície mapeada, e o commit do primeiro quadro re-arranja as janelas
sem mandar o arranjo — só manda quem também muda estado de layer. Repetir a zona não conta: o
wlroots ignora um valor igual ao que já tem. Sem patch (invariante 3), o que resta é o IPC, onde todo
comando termina mandando o que está pendente: um `nop`, pedido quando o primeiro quadro é MOSTRADO
(o frame callback pedido nele) — só então a zona foi contada. Um `sync` depois de apresentar corria
com o driver, que pode fazer o commit numa thread própria. E a prova é a JANELA receber uma largura NOVA
depois da coluna, comparada com a de ANTES dela: comparada com a primeira de todas, o teste passava
com o defeito.

**A barra traz o volume.** Anterior, tocar/pausar e próximo; uma linha de ar; mais alto, mudo e mais
baixo (`md-volume_plus`, o alto-falante e `md-volume_minus`, um desenho em três estados); uma linha
de ar; e trocar de porta. O passo do volume é um só, `player_transport::stepped_volume`, o mesmo do
`-`/`=` da vista, dos botões da barra e das setas na linha do volume. Sem estado, não há passo
(`player_transport::volume_step`): um volume chutado de nada era mandado e o player o guardava — quem estava a
20% e apertou "mais baixo" antes do primeiro estado tocava a seguir a 95%. A porta diz que o player
não responde.

**Na barra, o player é um controle remoto:** anterior, tocar/pausar e próximo, um por linha, uma
linha de ar e o mudo. Cada um é pego na linha inteira e acende sob o ponteiro, a roda é o volume, e
nenhuma frase os explica (decisão do dono). O resto da vista — linhas, mensagem, palco — fica
guardado e volta quando a coluna alarga. Nos outros três tamanhos o player adapta: o transporte
desenha os botões que cabem inteiros, e a imagem toma a fatia do tamanho (`Size::picture_lines`).

#### Que arquivo o player abre, e o MPRIS

**Que arquivo o player abre, e por que a lista é LIDA e não escrita.** A entrada `.desktop` tem de
nomear tipo por tipo — ela não pode dizer `audio/*` —, e a primeira forma era uma lista à mão com 51
dos 95 tipos de áudio e vídeo que o `shared-mime-info` conhece: os que alguém lembrou. Agora a lista
sai da base na BUILD (listar o store ao avaliar seria import-from-derivation, pago por toda avaliação
do flake), mais os tipos de mídia que moram em `application/` e os NOSSOS. E os padrões vão no
`mimeapps.list` de `$XDG_DATA_DIRS/applications`, que é onde a especificação guarda os da
distribuição: abaixo do arquivo da pessoa e do `/etc/xdg`, então uma escolha de alguém vence.

**Os nossos tipos** (`assets/player-types.nix`) são um por FAMÍLIA de decodificador — AdLib, Amiga,
trilha de console, chip de 8 bits —, porque o que a base não nomeia resolve para
`application/octet-stream`, e tipo que ninguém reivindica é arquivo que programa nenhum oferece
abrir. Eles entram nos três lugares que perguntam: a tabela do gerenciador (`mime-glyphs.nix`
importa as linhas), a base do sistema (`share/mime/packages/ronoctua-player.xml`, para quem pergunta
ao `xdg-mime`) e a entrada. **A maior parte da decisão é o que fica DE FORA**: um glob é reivindicar
todo arquivo com aquele nome, escrito por qualquer programa, então extensão que a base já dá a outra
coisa (`.iso`, `.gsf`, `.stc`, `.asc`…) ou que é de outra coisa lá fora (`.cff` é fonte, `.bam` é
genoma, `.mib` é SNMP) e toda extensão de duas letras ficam só no decodificador: o player toca quando
recebe, e não toma dos outros. A lista do que saiu e por quê está no topo do arquivo.

Dois grupos entraram depois. Os formatos do próprio player (`own`, veja ".rset e .rqueue") e as
legendas: os oito tipos do freedesktop (`subtitles.known`) e os que a base não tem
(`subtitles.invented`: TTML/DFXP, SBV, JACOSub, PJS, AQTitle, SCC, srv3, json3, o `.sup` do PGS e
o `.idx` do VobSub; o `.sub` dele já é o `*.sub` dos tipos do freedesktop). Pela mesma regra,
`.stl`, `.usf`, `.ssf`, `.rt` e `.txt` não são reivindicados — lá fora eles nomeiam outras coisas
com a mesma frequência — e só são lidos como legenda quando acrescentados a um filme, onde o
conteúdo decide (veja "Legendas"). O `.lrc` também fica de fora: é a letra ao lado de uma música,
que o player já mostra quando ela toca, e reivindicá-lo tiraria arquivos de letra do editor de
texto para uma recusa.

**O MPRIS é a especificação inteira, menos `Playlists`.** A `TrackList` são as folhas da fila na
ordem da árvore, e o id de uma faixa é o `EntryId` dela (`/br/ronoctua/player/track/<id>`): ele
sobrevive a mover, e `GoTo`, `RemoveTrack` e `AddTrack` (depois de uma faixa, ou primeiro com
`NoTrack`) agem pelo id. Toda mudança de forma da fila sai como `TrackListReplaced`. O `OpenUri` é o
`put` que toca, como abrir de fora. O título de um conjunto nunca é o de um arquivo só: são as
fontes juntas por ` + `. A capa (`mpris:artUrl`) é a que o daemon tirou do arquivo para o cache
(veja "Metadados"); a imagem ao lado do arquivo só quando não há outra. Os sons ambientes não
aparecem — não são faixa, e as teclas de mídia são da fila. `Playlists` fica de fora: um `.rqueue` é
um arquivo da pessoa, onde ela o pôs, e o player não mantém uma biblioteca deles para listar. `Quit`
é o mesmo caminho do `quit` do soquete (`engine::quit`, em "Fechar em todo lugar"); quem traz o
player de volta é o próximo pedido, porque o `client::ensure` pede a unidade ao systemd quando
ninguém responde e o daemon não mora ao lado do shell.

#### Legendas

**O conteúdo decide o formato.** A extensão engana demais: `.sub` é MicroDVD, SubViewer, MPsub ou o
fluxo de imagens do VobSub; `.txt` é TMPlayer ou VPlayer; `.stl` é o binário da EBU, o texto da
Spruce ou um modelo 3D. O `sniff` lê o arquivo, cada formato vota com as linhas que reconhece, e a
extensão só desempata. O charset segue a ordem da certeza — marca de ordem de bytes, UTF-16 sem
marca (pelos bytes zero), UTF-8 válido e, por fim, o palpite do chardetng —, e a página pode dizer
que foi palpite. Um SAMI com várias línguas mostra a primeira que diz algo.

**Leitores próprios, e a cauda pelo ffmpeg.** Lemos SRT, WebVTT, ASS/SSA, SAMI, MicroDVD, MPL2,
SubViewer, TMPlayer, SBV, TTML/DFXP/IMSC, EBU STL, o timed text do YouTube (srv3, json3) e LRC;
TMPlayer, SBV, TTML e EBU STL o ffmpeg 8.1.2 não lê. JACOSub, PJS, AQTitle, RealText, VPlayer,
MPsub, SCC, Spruce STL e MCC, e toda trilha de texto de dentro de um filme (`-map 0:s:<n>`), passam
pelo ffmpeg, que devolve ASS — no máximo 8 MiB e 30 s; passar de um é recusa com o número. Ler a
trilha de dentro de um filme custa ler o filme inteiro, então o que foi lido fica guardado pelo
carimbo (arquivo, tamanho, data, trilha), dentro do teto de `assets/cache.nix`
(`playerSubtitlesMib`); e um ASS de dentro do próprio filme, sozinho, é mostrado pelo mpv sem ler
nada.

**A pilha é nossa, não da libass.** A libass separa falas que se sobrepõem, mas não na ordem das
faixas: o que já está na tela fica parado e só o que entra agora é empurrado (libass 0.17.4,
`fix_collisions`), camadas diferentes nunca colidem, e `\pos`/`\move` desligam a colisão. Por isso
a mescla corta a linha do tempo onde muda o conjunto de falas visíveis e dá a cada fala o seu
`MarginV`: embaixo, a faixa 1 junto da borda e cada faixa seguinte acima das de baixo; em cima, ao
contrário. A altura é a contagem de linhas, e a própria mescla quebra as linhas longas, por igual
(`WrapStyle: 2`), para que a conta dela seja a conta que a libass desenha. Medido com a libass
pinada: a faixa 2 que fala primeiro sobe quando a faixa 1 entra.

**Uma mescla por imagem, fora da trava.** Cada voz que mostra uma imagem recebe UM script ASS com
todas as faixas de palavras sobre ela, cada uma na sua cor, empilhadas na ordem das faixas. Ler um
arquivo, convertê-lo e mesclar seis faixas não acontece sob a trava do motor: a trava entrega um
plano à thread `player-subtitles` e aplica a resposta, e um plano que mudou no meio faz da resposta
velha uma que ninguém aplica. O script vai para
`$XDG_RUNTIME_DIR/ronoctua-player-subs/<voz>.ass` e entra com `sub-add … select` ANTES de o anterior
sair (`sub-remove`): trocar faixas nunca deixa a imagem sem legenda. Enquanto isso a faixa diz
`preparing`. O início e o trecho de cada faixa entram na mescla, no relógio da fonte da imagem.

**O que cada faixa guarda na pilha.** O diálogo veste a cor da faixa (a cor que o arquivo deu a um
trecho só vale para a faixa sozinha), com itálico, negrito e sublinhado. Os letreiros de um ASS
(`\pos`, `\move`, desenhos) ficam onde o arquivo pôs, escalados para a resolução da mescla, numa
camada acima do diálogo. Só o topo é pedido de uma fala: o pé é onde todo formato põe o que não diz
nada, então quem manda ali é a faixa.

**O que o mpv mostra sozinho.** Uma faixa ASS sozinha é mostrada inteira, com a tipografia dela.
Legenda de imagem (PGS, VobSub) vai para a trilha secundária, uma de cada vez e nas cores dela, e
enquanto ela existe as faixas de palavras sobem para a banda de cima. Legenda na rede (um endereço,
ou a trilha de um filme que toca pela rede) só o mpv tem aberta: aparece sozinha, e entre outras é
recusada com a frase. Texto nunca vai para a trilha secundária.

**As cores das faixas.** `subtitle.lane-one` a `lane-six` e `subtitle.edge` são resolvidas contra
uma imagem escura fixa (L 0,25), nunca contra o cromo: seis matizes em arcos de ΔE iguais a partir
do amarelo da faixa 1, todos com Lc 75 sobre a imagem, e a borda preta. São iguais em todo tema e em
toda página — o daemon não sabe o tema e desenha sem porta aberta (`ronoctua-tokens` no motor) —, e
os portões medem a distância entre as faixas e o contraste de cada uma sobre a borda. A POSIÇÃO de
uma faixa decide a cor, e o chip é o único lugar do cromo em que ela aparece (abaixo).

**Abrir uma legenda de fora.** Uma legenda aberta pelo gerenciador, pelo `ronoctua-shell play` ou
pelo MPRIS vai sobre o filme da fila que tem o nome dela (o mesmo nome, ou o nome e uma marca:
`filme.pt-BR.srt`), senão sobre o que toca, senão sobre o filme ao lado que tem o nome dela — que
então entra e toca; senão é recusada com a frase. Uma legenda nunca é um item da fila, e nunca
recomeça a fila.

**Os tetos.** 16 MiB e 200 000 falas por arquivo; seis faixas por imagem (a sétima é recusada) e uma
legenda de imagem; no máximo 16 falas ao mesmo tempo na tela; XML com mais de 48 níveis recusado
antes de ser lido (`ronoctua_xml`). Cada recusa diz o número.

**A página das legendas são as faixas sobre a imagem** (`player_subtitles`, `LEADER p u` e `u`).
Até seis legendas ficam sobre uma imagem ao mesmo tempo, e cada uma é uma linha: o chip da cor dela,
a língua como o arquivo a diz e o nome, e o que ela está fazendo — embaixo ou em cima da imagem,
escondida, esperando, se preparando, ou recusada com a frase do player embaixo (a sétima, uma
segunda legenda de imagem, uma legenda da rede entre outras). Um ASS mostrado junto de outra
legenda diz que perde os próprios estilos. Um clique no chip ou `v` mostra ou esconde a faixa,
`←`/`→` a trocam de banda (`LaneChange::Stack`, um anel de dois), `d` a tira, e `RET` — o segundo
clique na linha — abre a página da faixa. Abaixo: "Adicionar arquivo de legenda" (os arquivos,
abertos na pasta do filme, `Purpose::Subtitle`: toda extensão de legenda e também as genéricas, um
`.txt` do TMPlayer — escolher é o que o torna legenda), "Buscar na internet", que diz na linha e no
`RET` que ainda não existe no player, "Ao lado do arquivo" (os arquivos da pasta do filme com o
nome dele, `subtitles::named_after`, lidos uma vez por filme e de novo quando a página abre — as
linhas são construídas várias vezes por quadro —, e numa thread: a porta que tem laço entrega à
vista um `player_subtitles::Wake`, que a acorda quando a lista chega, e a pasta de um filme numa
montagem de rede não congela o rodapé; sem laço — a fotografia, um teste — ela é lida ali) e "Dentro do arquivo" (as trilhas do filme, como as
outras mídias do arquivo). A sétima sobre uma imagem é recusada ANTES de ser mandada, com a frase
que o player diria. A língua aparece como o código que o arquivo diz: ainda não há tabela de nomes
de línguas no player, e um nome adivinhado seria uma palavra que o arquivo não disse.

**A cor de uma faixa de legenda só aparece num chip.** As seis cores são medidas contra uma imagem
escura fixa e contra a borda preta que toda legenda veste (`subtitle.edge`), nunca contra uma página:
num tema claro o amarelo da primeira sumiria. Por isso a porta desenha a cor de uma faixa só dentro
de um chip cujo fundo é essa borda (`player_rows::Chip`, `chrome::chip`) — uma amostra de como a
faixa aparece sobre o filme. O chip ocupa as `space.player.chipCells` primeiras células da linha,
com o glifo na do meio; o fundo vai meia célula além do glifo de cada lado (o glifo Material ocupa
uma célula de tinta, `icon::ink_across`), então o ar é igual dos dois lados, e fica
`space.player.chipAir` longe das bordas da linha. Todo pixel do fundo está dentro do controle que o
clique pega. A cor vem da POSIÇÃO da faixa entre as legendas da mesma imagem
(`session::subtitle_slot`), a mesma com que o daemon pinta: esconder uma não repinta as outras.

#### A linha do tempo

**A linha do tempo é UMA geometria, lida pela pintura e pelo ponteiro** (`player_timeline`, pura).
`lay` recebe as faixas, o comprimento de cada fonte, o do conjunto, o relógio e o espaço, e devolve
um `Deck`: uma régua de tempo e uma linha por faixa, todas no MESMO trilho, então o playhead é um x
só em todas; `Deck::quads` desenha (bandas, fantasma, marcas da régua, medidores, playhead, barra) e
`Deck::hit` responde o clique pelos mesmos números. Uma faixa só É a barra de tempo de hoje, com o
mudo e o volume onde a janela sempre os teve; a coluna pequena guarda o glifo, a banda e o mudo; uma
faixa ao vivo corre até o fim do trilho; mais faixas que o espaço rolam sob UMA barra. As cores são
papéis próprios (`player.lane.*`, `player.playhead`, `player.tile*`) sobre famílias que toda semente
já tem — imagem azul, som verde, legenda roxa, como em todo editor —, medidas antes de virar portão
(a dupla mais próxima, azul e verde no `flashbang`, a 0,079). Só um SOM tem mudo e medidor na linha
dele: uma imagem não toca som (`aid=no`) e legenda não tem, e um mudo que não responde seria um
controle desenhado à toa.

**Onde o deck é desenhado.** Na janela fora do palco e na coluna grande, média e pequena, um BLOCO
sob o transporte, no lugar da barra de tempo (`Layout::deck` linhas a partir de `Layout::bar`): as
faixas de um CONJUNTO — o que o esboço da fila diz que a entrada atual é —, no máximo
`space.player.deckLanes` e nunca mais de um quarto das linhas da superfície, o resto rolando sob uma
barra que é controle (a trilha gravada com `canvas.bar`, a roda sobre ela rola). Um arquivo sozinho
é uma faixa: as faixas implícitas dele (imagem, som, legenda) são um arquivo num relógio, e o deck
dele é a barra de sempre (`player_timeline::set_lanes`). No palco — só a imagem, a tela inteira, o
canto — e na página da linha do tempo, a barra global e nada mais. No rodapé, que não tem transporte
sob o qual pôr um bloco, as faixas de um conjunto são as primeiras linhas do "Agora", no lugar da
posição (`View::deck_in_now`); e a página **Linha do tempo** (`t`, `LEADER p i`, a linha em Recursos
variados) mostra TODAS as faixas em todas as portas, a de um arquivo sozinho inclusive — mover o
início do som é como se acerta um som atrasado num fone sem fio —, cada uma uma linha. Como linhas,
o deck é montado UMA vez com as linhas (`player_rows::DeckRow`, um `Rc<Deck>` que as linhas dividem);
a porta pinta as formas sob as palavras, cada sequência de linhas de uma vez para o playhead ser um
traço só, e pega o clique pelo mesmo deck (`player_rows::deck_at`, a gêmea de `slider_at`). O
teclado anda nelas: a régua é a posição (as setas buscam), a linha de uma faixa é a faixa — `←`/`→`
movem o início meio segundo (em qualquer linha de faixa, a de tocar junto e a de um conjunto na fila
também), `RET` abre a página dela, `d` a tira, `v` a mostra ou esconde. Sem nome — a coluna
pequena —, o cursor é o glifo.

**Um gesto só, em todas as portas** (`Deck::press`, `Grip`). O clique diz o que pedir agora e o que
o ponteiro segura até soltar, com a geometria em que foi apertado, para o arrasto seguir o trilho em
que começou seja o que for que o próximo estado desenhe. O glifo liga e desliga a faixa, o mudo dela
a silencia, o nome a escolhe (o segundo clique abre a página dela); o trilho busca ali e o arrasto segue buscando; o playhead é
pego onde está e busca enquanto é arrastado; os medidores — o da faixa e o do player na régua — vão
aonde o clique cai. A banda é SEGURADA e nada é pedido: passado `DRAG_SLACK_CELLS` ela vira arrasto e
deixa um fantasma onde cairia (`Deck::holding`, o mesmo cálculo de `lay`), e só ao soltar pede o
início novo, nunca antes do primeiro segundo; solta onde foi apertada, é um clique — um tremor da mão
que empurrasse uma faixa um quadro seria um gesto que ninguém fez. **O clique na banda segue a
invariante 19** (`Act::Choose`; decisão do dono, 2026-09-29): o primeiro ESCOLHE a faixa e o segundo
abre a página dela, como em toda linha — e o nome no bloco também. Numa linha de deck, escolher é o
cursor vir à linha (`View::band_clicked`; a banda não leva o cursor no aperto, só ao soltar, senão o
primeiro clique já seria o segundo); no bloco sob o transporte, que não tem cursor, a faixa escolhida
veste a cor do cursor (`Player::deck_chosen`, o `focused` de `Deck::words`) e o segundo clique nela
abre a página. Antes, um clique só na banda abria a página enquanto o nome pedia dois. `Esc`
cancela o arrasto antes de tudo: a banda fica onde estava e soltar não pede nada. A folga que separa o
clique do arrasto é `space.player.dragSlack`, duas células — a do `sweepSlack`, pela mesma mão que
treme —, e o fantasma é desenhado nos segundos que o trilho mostrava quando a banda foi apertada
(`Held::span`): um deck sem duração conhecida alarga enquanto toca, e o fantasma fugia da mão. A
roda sobre o bloco é um volume — o da faixa sob ela, quando soa, e o do player na régua; sobre uma
faixa que não soa não há volume, e ela rola as faixas; sobre as LINHAS do deck ela rola a lista,
porque uma página maior que a porta precisa rolar e um deck que prendesse a roda seria a prateleira
que parou a página. Sob o ponteiro, o glifo acende no tom de uma tecla sob o ponteiro
(`text.subtle.lit.key`, o da linha de controle das colunas) — nunca no do cursor, que numa faixa sem
nome é o glifo — e a linha 0 da janela (a mensagem do rodapé) diz o que o controle faz, com o nome da
faixa na frente (`player_timeline::hover_sentence`): a coluna pequena desenha a faixa só pelo glifo. Uma linha
de deck é figura: nenhuma faixa de hover acende atrás dela. O playhead anda com cada estado que chega
(evento, não relógio).

#### Cada imagem na sua tela

**Cada porta compõe a SUA tela** (`player_layout::tiles_on`). Um conjunto pode mandar um filme a uma
tela e outro à outra, e a capa às duas (`Lane::screens`), então nenhuma porta mostra "as imagens":
cada uma pergunta quais faixas estão roteadas à tela em que está — a janela pela tela em que a
superfície entrou (`Ui::screens`, do `wl_output` e do `enter`), a coluna pela tela do espaço dela
(`Tenant::placed_on`, que vem antes do `enter`), cada superfície do rodapé pela própria — e só
essas. Uma faixa é imagem quando o daemon diz que ela tem uma agora (`LaneState::picture`: um filme
ligado, ou a capa de um som que a pessoa quer ver); a ordem e o que está escondido são do arranjo.
Uma porta que não sabe a tela mostra todas: uma imagem sumida porque o compositor demorou a dar
nome a uma saída seria um defeito sem sintoma além de uma faixa vazia. Todas são postas por
`arrange::place` na forma do arranjo, cada uma no próprio formato, e a porta desenha e pega o
clique nos lugares que essa função deu (`Player::placed`); a faixa da imagem tem a altura que a
forma pede (`arrange::natural_aspect`), sob o mesmo teto de antes. Cada imagem é pedida ao daemon
PELA FAIXA (`Ask::Preview { lane }`) e a thread dela fica enquanto a faixa aparece ali: mudar a
ordem não reabre nada.

**Mostrar em cada tela** (`M`, `player-every-screen`, a linha em Recursos variados). Uma janela por
tela para onde alguma imagem vai — `ronoctua-shell player maximized --screen <conector>` —, cada uma
posta em tela cheia NA SAÍDA DELA (`xdg_toplevel.set_fullscreen(Some(output))`, o argumento do
próprio xdg-shell: nenhum protocolo novo) e compondo o que é roteado ali; ela espera a conexão
descrever aquela saída pelo nome antes de pedir a tela cheia. Cada janela segura um slot próprio,
`player-stage@<conector>.pid`, e apertar de novo acha todas pelos slots e as fecha
(`player_stage::everywhere`); o conector só vira nome de arquivo se for um nome seguro. Quem abre
escreve o slot com o pid do filho na hora — a janela escreve o mesmo pid quando sobe, e um segundo
aperto antes disso abria outra rodada —, e cada filho é esperado numa thread, senão virava zumbi do
rodapé. Sem escopo, como a janela de palco e a coluna: o pid é o slot, e o `systemd-run --scope` só
vira o programa depois de uma ida e volta. Com uma tela só não há o que espalhar, e isso é dito;
sem nenhuma imagem roteada, também; uma porta que não sabe o nome de nenhuma tela diz isso, e não
que há uma só. A decisão é pura
(`everywhere_plan`), e as telas são as que a PORTA conhece — o rodapé as dele, a janela e a coluna
as do `wl_output` —, nunca uma pergunta ao compositor.

#### A disposição

**A disposição é uma conta só** (`arrange::place`, pura), e toda porta que põe imagens lê a mesma:
a faixa de imagem da janela e da coluna, o palco, o pip e o painel do rodapé. Sete formas —
Automática, Uma, Lado a lado, Empilhadas, Grade, Tira e Canto; a Automática é a melhor das três do
meio pelo que mostra —, e cada retângulo tem o formato da própria imagem, nunca espremido. Quando
falta espaço acima do piso, saem as ÚLTIMAS da ordem, e a primeira fica sempre. O Canto é a única
forma em que imagens se sobrepõem: as outras ficam sobre a primeira, no canto de baixo à direita,
sem sombra — um quadro dentro do quadro. Um teste de propriedade passa todas as formas por uma a
oito imagens, sete proporções e sete áreas: dentro da área, sem sobreposição (menos o Canto), o ar
mantido, nada abaixo do piso.

**Um teto de imagens, contado dos dois lados.** Um conjunto mostra no máximo `PICTURES_MOST` (6)
imagens, e cada uma é uma textura do quadro. O renderizador não conhece player nenhum, então conta
as dele por um número próprio (`image::PLAYER_PICTURES`, no `CEILING`); o shell, que liga os dois,
não compila se eles se separarem (`player_layout`), e nenhuma porta pede mais que isso. O que passa
do teto do renderizador é descartado sem erro em lugar nenhum — é o número deste recurso que
falharia calado (`files-panels-have-two-pictures`).

**A página Disposição** (`player_layout`, `PageId::Layout`, `y`, `LEADER p y` e a linha em Recursos
variados, que diz a forma). A FORMA é um anel — as sete de `Form`, cada uma com o glifo dela, o
Automático com `md-view_compact` — que as setas e o `RET` giram. Embaixo, um ESQUEMA desenha onde
cada imagem vai nessa forma, um ladrilho cada (`chrome::tile`), sem palavra nenhuma, numa caixa no
formato de uma tela; o ladrilho da imagem cuja linha tem o cursor acende (`Builder::light_schematic`,
porque as linhas das imagens vêm depois do esquema). O esquema é posto UMA vez com as linhas, pela
mesma `arrange::place` das portas, na caixa em que ESTA porta põe as imagens e com o piso dela
(`View::pictures_room`, dito pela porta): o desenho é o que a porta mostra. Uma imagem que a forma
deixaria menor que o piso (`space.player.tileFloor`) fica fora da porta — não tem ladrilho, a linha
dela diz "sem espaço" e uma frase embaixo diz por quê (`player_layout::left_out`); escolher o
canto numa coluna pequena não fazia nada visível. Sem caixa conhecida, o esquema mostra todas, sem
piso. Cada porta o pinta
sobre as linhas dele, cortado ao que está na tela (`Schematic::quads`, `player_rows::SchematicRow`);
é figura, sem faixa de hover. Os ladrilhos que se sobrepõem — o canto — ganham um anel do chão
antes, o ar que `place` guarda em volta deles. Depois, uma linha por imagem que pode aparecer — todo
filme ligado e todo som cuja capa pode — na ordem em que são postas: `RET` mostra ou esconde (a capa
de um som), `←`/`→` a movem uma casa, e o `◀` e o `▶` no fim da linha fazem o mesmo para o ponteiro
(invariante 19). As setas têm células próprias: o valor e o rótulo são dispostos no que sobra, e a
coluna dos valores da seção abre espaço para elas, com uma célula de ar — numa coluna estreita o
valor corria por baixo delas, e depois foi cortado para dar lugar (`pair_with_controls`). Toda
mudança manda o arranjo INTEIRO (`Ask::Arrange`). Seguindo outra máquina, o arranjo é o dela, e a
página diz isso numa frase.

#### Sons ambientes

**Só gravações, e só as ouvidas.** Os sons ambientes são gravações de verdade — chuva, trovoada,
água —, cada uma aprovada de ouvido pelo dono, sob CC0 ou domínio público; um som novo só entra
assim (a lista começou com três). `assets/ambience.nix` é a lista; `nix/ambience.nix` recusa na
avaliação uma licença que não seja essas duas, o nome de um gerador (`anoisesrc`, `sine`,
`aevalsrc`, `anullsrc`, `lavfi`) em qualquer campo e um original que não seja um nome de arquivo;
no build, um original cujo sha256 não é o aprovado. Os originais (695 MiB) são o asset de Release
`ambience-v1`, empacotado por `nix run .#ambience-release`, e o sistema carrega só os laços
(42 MiB), com o player.

**A emenda é medida, não escolhida de ouvido.** A janela de cada gravação é a que o `ebur128`
mostra com as duas pontas no mesmo fundo e nenhum evento perto delas. O fim da janela é cruzado no
começo em igual potência (ruído com ruído mantém o nível), o ganho leva ao alvo de loudness só como
ganho — o `loudnorm` que cairia em compressão é recusado, porque achataria o trovão —, e o laço sai
em FLAC 48 kHz: o Opus esquece o pre-skip a cada volta e toca ~6,5 ms de decodificador aquecendo. O
build recusa uma emenda que se ouve (um buraco abaixo do mais baixo do próprio laço, silêncio, um
degrau que a gravação não dá de um segundo para o outro, um trovão dentro do cruzamento) e grava
cada medida no `index.json`.

**O mpv dá a volta sozinho.** Cada laço é tocado três vezes pelo mpv pinado, pelas três rotas, e
comparado amostra por amostra com o laço emendado: `loop-file` e a janela de duas entradas devolvem
o laço idêntico; `amovie=…:loop=0` perde 4 608 amostras (96 ms, um quadro de FLAC) a cada volta.
Fica o `loop-file`, que é exato e não pede nada ao motor, e o build recusa um `route` que não seja
exato. O que o build não vê é o aparelho: `--ao=pcm` não tem relógio.

**Ao lado da fila, e fora dela.** Um mpv por som ligado (`audio-client-name=ronoctua-ambience`, sem
scripts, sem controle de mídia), até `atOnce` ao mesmo tempo. Parar, próximo, as teclas de mídia,
o volume do player e o MPRIS são da fila e nunca chegam a eles; um som para quando é desligado, com
"silenciar todos" ou quando o player fecha. Escolher toca na hora, para sempre. O volume de cada som
é lembrado; que um som estava ligado, nunca — o player que sobe não toca nada sozinho.

**Até o Release existir, a máquina diz que não tem.** A unidade só recebe `RONOCTUA_AMBIENCE` quando
o flake tem os laços (`ronoctua ? ambience`), e sem ele o player diz `no-ambience` em vez de uma
página vazia.

**Os sons ambientes são uma página, ao lado da fila** (`player_ambience`, `LEADER p e`, e a linha em
Ouvir e assistir). Uma linha por som: o glifo do grupo dele (`ambience-*`; um grupo sem glifo veste
a onda), o nome (`player-ambience-<id>`, e um som sem nome nesta porta é dito como um, nunca pelo id
cru) e o volume próprio como barra, com o volume enquanto soa, ou "desligado", ou "em pausa". A barra
é de `space.player.soundBar`, mais curta que a do volume do player: três lado a lado com a barra
inteira espremiam os nomes num toco na coluna. Os sons ficam sob o título do grupo deles
(`player-ambience-group-<grupo>`, `Builder::heading_within`), que não abre seção: os valores
continuam numa coluna só, e toda barra da página tem o mesmo tamanho, medido pela palavra mais longa
que uma linha pode dizer — "desligado" tirava uma célula da barra da linha dele. `RET` — e o segundo clique — liga ou desliga, e ligado
toca na hora; `←`/`→` e a barra mudam o volume, ligado ou não, que o player guarda. Um som além do
`at_once` da máquina é recusado antes de ser mandado. Enquanto algum soa, uma linha os pausa todos
(`ambient-toggle`) e outra silencia todos. Os créditos são uma página: quem gravou, a licença e onde
foi publicado, cada um uma frase que quebra e nunca é cortada — um autor ou um endereço cortado na
largura de uma coluna não é crédito. Sem sons na máquina, a página diz por quê, como o player diz.

#### Metadados

**A folha: ler tudo, e o que falta dito.** `ronoctua-tags` é a única fronteira com as etiquetas e as
capas de um arquivo de mídia. A folha junta quatro leitores, cada um opcional: o lofty (as etiquetas
dos formatos que ele conhece), o ffprobe (fluxos, capítulos, fatos técnicos), o exiftool (tudo, só
para ver) e o `mkvextract`/`mkvmerge` do Matroska — só para um Matroska: um FLAC nunca pergunta a
eles, nem diz que faltam. Um programa que falta vira uma linha que diz qual, nunca uma folha menor
calada, e os campos comuns do tipo aparecem sempre, em branco inclusive. A leitura é do daemon, numa
thread própria (`player-sheet`), nunca sob a trava; as últimas oito folhas ficam guardadas pelo
carimbo do arquivo, e um arquivo que começa a tocar é lido antes de alguém pedir. A folha vai só a
quem pede (veja "Duas mensagens inteiras").

**O palpite do lofty não é lei.** Sem assinatura conhecida, o lofty procura um sincronismo de
MPEG/AAC no primeiro kilobyte e acha um dentro de um Matroska (e um MP3 dentro de um Ogg Theora);
acreditado, ele poria uma etiqueta ID3 na frente do arquivo. Um palpite que não começa o arquivo só
vale confirmado pelo ffprobe; senão decide o formato do ffprobe, e depois a extensão.

**Nunca escrever no lugar, e uma escrita por vez.** O mpv pode estar tocando o arquivo. Toda
escrita vai para `.<nome>.ronoctua-<pid>.part` na mesma pasta — clonada por reflink onde o sistema
de arquivos clona, copiada onde não, com o espaço conferido antes — e termina num `rename`; o mpv
segue no inode antigo e não é recarregado (sem lacuna), e a próxima carga lê o novo. O carimbo
(`len`, `mtime`, `ino`) com que a folha foi lida é conferido no começo e de novo antes do `rename`:
um arquivo mudado desde então é recusado, nunca sobrescrito. Arquivo com outros nomes (hard link),
de outro dono, marcado só-leitura ou em pasta sem escrita é recusado com frase. As escritas vão
para UMA thread (`player-retag`), uma de cada vez — o rascunho leva o nome do arquivo e do processo,
e duas escritas do mesmo arquivo dividiriam um —, e o andamento e o fim estão em `State.retag`.

**O rascunho é relido.** O lofty 0.25.4 trunca um arquivo cuja etiqueta termina o arquivo quando
ela encolhe — um FLAC sem quadros depois dos metadados virou 24 bytes —, e o arquivo quebrado ainda
abre. Por isso todo rascunho é lido de volta (`native::verify`): cada campo, cada etiqueta nomeada,
cada imagem, as etiquetas que ninguém tocou e, no FLAC, os bytes depois dos metadados. Um rascunho
que não confere é jogado fora e o arquivo fica como estava. E o lofty genérico perde o que não tem
nome para ele (uma chave Vorbis desconhecida, o `COMM:iTunNORM` do iTunes): o crate edita as
etiquetas concretas, e os campos leem e escrevem só o item sem descrição.

**Quem escreve o quê.** O lofty para os formatos dele, até 512 MiB, porque ele reescreve com o resto
do arquivo na memória (o MP4, inteiro) — acima disso a folha diz que é grande demais; o
`mkvpropedit` para Matroska, lendo antes todas as etiquetas globais, porque `--tags global:`
substitui todas; o remux (`ffmpeg -map 0 -c copy`) só onde o muxer guarda tudo (hoje, Ogg com
vídeo). AVI, TS, FLV e ASF são recusados pelo nome: o remux deles perderia o índice, os PIDs ou as
chaves da Apple. As ferramentas do Matroska só entram na unidade com `ronoctua.media.enable`
(`nix/player-tools.nix`, `matroska`): o `mkvtoolnix-cli` traz o Qt, 560 MiB sozinho e +32,5 MiB
onde o perfil completo já tem o qtbase. Sem elas a folha de um Matroska é lida pelo ffprobe e diz
que não pode ser gravada, nomeando o programa que falta.

**Toda etiqueta, pelo próprio nome.** Uma linha de Todas as etiquetas que o formato grava é gravada
pelo nome dela (`Edit::SetTag`, `RemoveTag`), e um nome vale inteiro: todo valor que ele tinha
naquela etiqueta (um Vorbis que diz ARTIST duas vezes) dá lugar ao digitado. Cada escritor tem o seu
caminho: o lofty pelo item concreto — os nomes crus do Vorbis, do APE e do RIFF; no ID3v2 o quadro
pelo id, o `TXXX` pela descrição, e um `COMM` ou `USLT` com descrição à parte do campo
(`COMM:iTunNORM` é uma linha dela); no MP4 o átomo, ou o freeform `----:<mean>:<nome>` —; o Matroska
pela mesma leitura e troca dos campos (`--tags global:`), só as etiquetas do arquivo inteiro e cada
uma sozinha, porque a de uma faixa, a aninhada e a que tem filhas perderiam o resto na troca; e o
remux pelo `-metadata` do ffmpeg, no contêiner (nos dois lugares, como um campo) ou no fluxo que o
ffprobe disse. Um nome passa pela gramática da etiqueta (`ronoctua_tags::tag_name`, pura, que a
porta pergunta antes de mandar e o escritor de novo): uma palavra digitada para um MP3 vira
`TXXX:<palavra>`, para um MP4 o freeform do iTunes, e um nome listado volta como foi — senão a linha
dele gravaria outra etiqueta. O que não se grava diz por quê, cada um na frase dele: o que o
exiftool e o ffprobe leem para mostrar muda pela etiqueta que espelham; o número da faixa e do disco
(`TRCK`, `trkn`) muda pelo campo, porque um texto digitado não cabe num número e num total; dado que
não é texto; a etiqueta de uma faixa do Matroska; e, num contêiner que nada grava, o motivo do
contêiner. "Acrescentar uma etiqueta" vai para a etiqueta principal do arquivo (`Sheet::adding`: o
formato primário do lofty, as globais do Matroska, o contêiner do remux), ou diz por que o arquivo
não aceita nenhuma.

**O que segue um arquivo regravado.** Quando uma folha nova substitui a que se tinha — gravada
aqui, relida com `refresh`, ou achada mudada quando o arquivo toca de novo —, o que o player diz
dele segue: um item da fila que tinha o título ANTIGO como nome recebe o novo (um nome que a pessoa
ou uma lista deu é deles e fica), o título, o artista, o álbum e a letra do estado enquanto ele
toca, o MPRIS com eles, e `sheet_generation`.

**A capa, e o cache do player.** Uma imagem dentro de um FLAC não é endereço que um ouvinte abra;
então a capa de cada arquivo lido é tirada para o cache do player
(`$XDG_CACHE_HOME/ronoctua/player`), com o hash do arquivo, do carimbo e do lugar da imagem por
nome: um arquivo regravado tem nome novo, e o velho envelhece até sair. A varredura do `~/.cache`
leva entradas inteiras da raiz, e `~/.cache/ronoctua` é tocado todo dia, então nada dentro dele
seria varrido: a pasta `player` tem teto próprio (`playerMib` em `assets/cache.nix`) e é escrita só
pelo daemon — depois de cada escrita sai o arquivo lido há mais tempo, e a leitura grava o horário
de acesso de propósito (numa montagem `noatime` ele não mudaria sozinho). As capas e as listas dos
diretórios dividem essa pasta e esse teto.

**Um endereço tem folha só de leitura.** Nada nesta máquina tem os bytes de um rádio: a folha dele
é o que ele diz enquanto toca — as palavras ICY (`icy-title` é a música no ar, `icy-name` a
estação) ou as etiquetas de um arquivo atrás de um endereço, como o mpv as leu —, com as mesmas
linhas de uma folha de arquivo, todas dizendo que não podem mudar. Cada coisa nova que o stream diz
faz `sheet_generation` andar.

**Campos de várias linhas no editor, e a pergunta sobrevive à porta.** Letra, comentário e
descrição são editados num arquivo do daemon em `$XDG_RUNTIME_DIR/ronoctua-player-drafts` (pasta
0700, arquivo 0600 antes de qualquer byte), que ele vigia com inotify — evento, não relógio: a
thread só existe enquanto há rascunho aberto, e tirar o último remove a vigia, o que a acorda para
sair. Salvo com um valor diferente do arquivo, o rascunho aparece em `State.drafts`, e qualquer
porta pergunta "gravar isto?", com o "não" primeiro: a porta que abriu o editor pode já não
existir. A gravação é recusada se o campo mudou no arquivo desde que o rascunho nasceu (outro campo
mudado não importa).

**Os metadados são a última seção da home, e a folha é pedida pela porta** (`player_metadata`, plano
P7; `LEADER p d` volta à home com o cursor na seção). O estado leva só `sheet_generation`; a porta
que mostra a seção pede a folha do arquivo mostrado (`client::sheet`, a um player que RODA — nunca
um iniciado para isso) quando a geração ou o arquivo mudam, numa thread que deixa a resposta onde a
vista a pega e acorda a porta (`Meta::look`, o padrão do `Beside` das legendas). Uma porta sem laço
— a fotografia (`--sheet`), um teste — recebe a folha (`Meta::given`) e não pergunta a ninguém.
Enquanto a nova não chega, a folha velha do mesmo arquivo continua na tela; sem nenhuma, a seção diz
que lê. Uma que não veio a tempo (`client::SHEET_PATIENCE`, um disco lento) não troca a velha nem diz
"não roda": é contada e pedida de novo nos próximos assentos, até três vezes por geração, e sem folha
velha a seção diz que ainda lê (`Got::Slow`). Com o player ido (`Hearing::Shows(None)`) a porta
esquece o que tinha dele — folhas, mesas, pedidos (`View::player_gone`): um player que volta conta
gerações e candidatos do zero, e uma página guardada com o mesmo número seria a do outro. Com várias
fontes tocando, um anel ("Do arquivo") escolhe qual.

- **Todo campo comum, sempre, em branco inclusive** (`Field::common`): um campo vazio diz
  `player-tag-empty` no registro da resposta e age como os outros. `RET` (o segundo clique) pergunta
  um campo de uma linha na linha da porta, SEGURANDO o valor de agora (`Question.initial`, uma
  pergunta por campo — `ask-player-tag-<campo>` —, que o rodapé devolve à vista por
  `player_metadata::asks`); nada digitado, ou o mesmo valor, é deixa-pra-lá; número, data e ISRC são
  conferidos na porta (`Field::check`) e recusados numa frase antes de qualquer pedido. `d` apaga
  depois de uma pergunta, "não" primeiro; num campo em branco diz que não há o que apagar. O valor
  vem INTEIRO, quebrado na coluna dele (`Builder::pair_whole`, também nos Fluxos e em Todas as
  etiquetas): "ver todos os metadados" não cabe num título cortado em "The Lighthous…". Um campo de
  VÁRIAS linhas — letra, comentário, descrição — mostra a primeira linha depois de quantas tem ("3
  linhas · …") e abre a PÁGINA DELE (`PageId::Field`): o texto inteiro, verso a verso, e a linha
  "Editar no editor" — ou por que o arquivo não aceita. Ela abre no EDITOR: `Ask::Draft`, e o daemon
  responde onde escreveu o rascunho (`Told::Draft`, pelo mesmo contador de pedidos em ordem), que a
  porta abre (`files::editor_command`). Salvo com valor novo, o estado diz (`State::drafts`) e
  QUALQUER porta pergunta se grava, "não" primeiro, com o texto que gravaria sob as respostas — a
  porta que abriu o editor pode já não existir. O "não" (e `Esc`) é AINDA NÃO (`Ask::KeepDraft`): o
  rascunho fica aberto e a pergunta volta quando ele for salvo de novo — salvar no meio da escrita é
  como se usa um editor, e jogar fora o rascunho era perder o texto sem aviso. Jogar fora é `d` no
  campo enquanto o rascunho está aberto, depois de uma pergunta própria (`Ask::DropDraft`). A
  pergunta sai de toda porta quando o rascunho deixa de estar salvo ou aberto — outra porta
  respondeu —, em vez de ficar na tela esperando uma resposta sobre nada.
- **Um campo que o arquivo não aceita é um fato**, desenhado como fato, e `RET` diz por quê; quando
  um motivo vale para todos (um endereço, o Matroska sem as ferramentas), ele é dito uma vez sob o
  título. O andamento e o fim da última gravação do arquivo mostrado vêm de `State::retag` (a
  porcentagem, as que esperam, gravado, capa salva, ou a recusa na frase dela).
- **A capa é uma imagem das linhas.** O daemon guarda a capa de cada arquivo lido no cache dele e
  nomeia o arquivo na folha (`Sheet::cover_file`); a porta o decodifica na mesma thread que buscou a
  folha, até `space.player.coverPx` no lado maior, e o desenha sobre `space.player.coverLines`
  linhas próprias (`player_rows::CoverRow`), na proporção dele, a partir da primeira célula das
  linhas e com `space.files.pictureAir` de ar acima e abaixo — uma imagem não encosta na linha de
  cima nem na de baixo. Rolada pela metade, a parte que a rolagem tirou sai da imagem também, sob uma
  chave própria (`player_metadata::cover_place`, a conta que a janela e o rodapé pintam). A linha da
  capa abre a página dela: outra imagem dos arquivos (abertos na pasta do arquivo,
  `Purpose::Cover`, só imagens), salvar ao lado do arquivo, ou tirar depois de uma pergunta.
- **Todas as etiquetas, cada uma agindo.** Toda linha que o arquivo aceita age como um campo: `RET`
  pergunta o valor na linha da porta SEGURANDO o de agora quando ele cabe inteiro — uma linha, o
  único valor do nome —, e vazia, com a pergunta que diz que substitui tudo
  (`player-tag-value-all`), quando não cabe (várias linhas, um valor cortado, um nome com dois
  valores); `d` tira depois de uma pergunta, "não" primeiro, que diz quantos valores saem. Uma linha
  que o arquivo não aceita é um fato e `RET` diz por quê; um motivo que vale para um grupo inteiro
  (o exiftool, as etiquetas de uma faixa do Matroska) é dito uma vez sob o título dele, e um que
  vale para a página, uma vez — não de novo sob "Acrescentar uma etiqueta", que o disse. Essa linha
  vem primeiro, com o nome da etiqueta para onde vai: pergunta o nome e depois o valor, e um nome
  que a etiqueta não aceita é recusado numa frase que diz a regra dela (letras sem `=` no Vorbis,
  quatro caracteres no RIFF…); um nome que já existe pergunta o valor como quem substitui tudo.
- **Técnico, e as páginas.** Os fatos técnicos são só leitura, cada um na unidade dele (a duração
  como relógio, taxas com a marca decimal da língua); Fluxos, Capítulos e Todas as etiquetas abrem
  páginas, a última agrupada por quem leu cada etiqueta (lofty por formato, o Matroska por alvo, o
  ffprobe pelo contêiner ou pelo fluxo, o exiftool pelo grupo, e o que um stream diz de si). O que
  não pôde ser lido é dito no fim, e "Ler de novo" pede `Ask::Refresh`. A linha Sugestões abre a
  página delas e diz o que o daemon está fazendo por este arquivo ou quantas achou.

#### Buscas online

**Letra e patrocínio nascem desligados.** Os dois contam a um estranho o que está tocando. O
SponsorBlock é perguntado pelo PREFIXO de quatro dígitos do SHA-256 do id — a forma que o próprio
serviço recomenda —, e o SHA-256 foi escrito à mão (`ronoctua_lookup::sponsorblock`, com os vetores
da especificação no teste) em vez de uma dependência para um uso só.

**As buscas são um crate, e são do daemon.** `ronoctua-lookup` é a única fronteira com LRCLIB,
SponsorBlock, MusicBrainz, Cover Art Archive, Wikidata, TMDB, OpenSubtitles e SubDL. Cada serviço
devolve valores tipados ou uma recusa `{serviço, porquê}` de um conjunto fechado, e a frase é da
porta. Os módulos são puros sobre um `Net`: o do daemon espera o passo do host e pede pela web; o
dos testes é uma lista de respostas escritas à mão que registra cada pedido — é assim que se prova o
endereço e os cabeçalhos sem soquete. Quem pergunta é o daemon, nunca uma porta: o MusicBrainz
aceita um pedido por segundo por ENDEREÇO e responde 503 acima disso, e três portas seriam três
limitadores. O passo é um estático por host (MusicBrainz 1 s, os outros 250 ms), e quem pede
reserva o próximo horário livre: dois pedidos juntos nunca levam o mesmo.

**Uma mesa por tipo de busca, e a página sobrevive à porta.** Sugestões de etiquetas, legendas e
letras são mesas (`Desk`), cada uma com uma thread que nasce na primeira busca. A busca ganha um
número quando é pedida, e o que ela acha só vale enquanto o número ainda é o da mesa: uma busca nova
SUBSTITUI a que está andando, que para entre dois pedidos. Uma busca feita no rodapé é a página que
a janela encontra.

**Consentimento é um toque numa linha que nomeia o host.** Nada busca sozinho, a não ser as duas
chaves que já buscavam (letras automáticas e SponsorBlock), nascidas desligadas. Cada serviço tem o
seu consentimento em `player.json` (`consented`), nascido vazio; a linha que busca nomeia os hosts
do serviço, e apertá-la é o consentimento: a porta grava e só então pede. O daemon lê o arquivo na
hora de perguntar — nunca uma cópia da partida — e recusa um serviço não permitido com `off{host}`,
sem mandar nada; o teste `nothing_reaches_the_network_until_somebody_says_so` cobre todo serviço.

**As chaves são arquivos da pessoa, privados desde o primeiro byte.** `player-keys/` na raiz de
configuração da instância (o `.#dev` tem as suas), pasta `0700`, cada arquivo criado `0600` antes
do primeiro byte e renomeado por cima do anterior; a porta grava, o daemon lê na hora de usar. A
chave é um `Secret`: não imprime, não serializa, e vai num cabeçalho — `Bearer` para o TMDB,
`Api-Key` para o OpenSubtitles. O SubDL só aceita a chave na query string: ela entra só no
transporte, nunca no endereço que um log ou um `Debug` mostra, e a página diz que o SubDL a vê nos
registros dele. Colar uma chave é testá-la (`key-check`): o OpenSubtitles pela chave e, com uma
conta, pelo login e pela cota do dia; o TMDB pelo token de leitura (uma chave v3 é recusada na
hora, porque ela só funciona na query string); o SubDL por uma busca de um resultado. O login do
OpenSubtitles vive só na memória do daemon, é reaproveitado e refeito uma vez quando o serviço
recusa o token; o host que o login indica só recebe a chave se for do `opensubtitles.com`. A mesma
escrita privada passou a guardar as máquinas pareadas (`remote::keep`), que antes gravava e só
depois fazia `chmod` — o arquivo ficava legível entre as duas chamadas.

**Não há fonte de legenda sem chave.** Decisão do dono (2026-09-28): as legendas vêm do
OpenSubtitles (a chave, e se a pessoa quiser a conta, que dá mais downloads por dia; a página diz
quantos restam) e do SubDL (chave grátis). A fonte sem chave que existia, o Podnapisi, está fora do
ar — o domínio não resolve desde junho de 2026 —, e nenhuma outra sem chave pôde ser verificada: sem
chave, a API do OpenSubtitles responde 403. O leitor do Podnapisi fica pronto e desligado
(`podnapisi::AWAKE`): acordá-lo é virar a constante e conferir o formato, que nunca foi documentado
nem pôde ser medido. A página de busca diz que a busca pede a chave da pessoa, em vez de parecer
funcionar.

**Uma legenda baixada nunca apaga outra.** Ela é salva como `<filme>.<língua>.<ext>` ao lado do
filme, em UTF-8 (legenda é o último lugar onde vivem code pages antigas); os mesmos bytes são o
mesmo arquivo, e bytes diferentes ganham `.2`…`.9`. Uma pasta que não aceita escrita manda a
legenda para `$XDG_DATA_HOME/…/player/subtitles/<hash do filme>/` — dado, não cache, porque um
conjunto salvo a nomeia. Se o filme toca, ela vira faixa na hora, sobre a imagem que é aquele
filme.

**Uma letra ao lado da música só substitui outra com a pessoa dizendo.** Salvar ao lado com um
`.lrc` já lá é recusado com `exists`, na hora, para a porta perguntar ("não" primeiro) e mandar de
novo com `replace`. A letra também pode ir para a etiqueta.

**Não há tradução de letras.** Decisão do dono (2026-09-28): letras são as originais, buscadas e
baixadas em qualquer língua; nem tradução automática, nem por chave. A pesquisa mostrou o que havia:
o LRCLIB não tem traduções nem campo de língua; o endpoint de traduções do NetEase é não oficial e
traduz quase sempre para chinês; o Musixmatch só as dá em planos pagos; Genius e LyricsTranslate não
têm API para o texto. O que cobriria qualquer música seria tradução de máquina — com a chave da
pessoa num serviço, ou modelos offline pinados no build, que custam closure —, e é isso que ficou
de fora. A página de letras diz: uma fonte (LRCLIB), sem campo de língua e sem tradução.

**Filmes: o que vem sem chave, e o que pede o token.** O Wikidata dá, sem chave, título, data,
direção, gênero e língua original de um filme, e quase nunca um pôster (pôster não é livre, e por
isso raramente está no Commons); sinopse e pôster vêm do TMDB, com o token de leitura da pessoa, e a
página diz isso. As sugestões de música vêm do MusicBrainz — pelo id que o arquivo já tem, senão
por busca — e a capa do Cover Art Archive, três pedidos em três hosts. Nada busca sozinho: sugerir
é uma busca que a pessoa aperta, e gravar é escolher campo a campo, com o valor de agora discreto ao
lado.

**As línguas do conteúdo são uma tabela** (`assets/tongues.nix`). Não são as línguas da interface:
são aquelas em que se busca legenda e em que um fluxo é marcado. Cada linha tem o código deste
sistema (`pt-BR`), o ISO 639-1 e 639-2/B e o código de cada serviço — o SubDL escreve `BR_PT` e
`ZH_BG`, e o `PT` dele é o português europeu. O gerador recusa código duplicado, forma errada e
língua em que nenhum serviço busca. A busca padrão é a língua da interface, depois o inglês.

**As buscas na rede, nas portas (P8).** As páginas que perguntam à rede pelo que toca — sugestões
de etiquetas, legendas, letras — e a página Internet, que diz o que pergunta e guarda as chaves:

- **O daemon pergunta; a porta mostra a mesa.** Toda busca é do daemon (`find::Desk`: um limitador
  por host para a máquina inteira, e uma página que sobrevive à porta — o que se buscou no rodapé é
  o que a janela acha). A porta guarda a última página de cada mesa (`player_find::Found`) e, quando
  a geração dela anda no `State::desks` enquanto uma página a mostra, pede a nova FORA do laço e é
  acordada quando ela chega — a forma da folha (`Meta::look`). Uma porta sem laço (foto, teste)
  recebe a página (`Found::given`, `photograph --desk`) e não pergunta nada. Uma página é sobre o
  arquivo da busca: mostrada só enquanto é o arquivo que a página mostra (`Found::page_for`).
- **Pressionar a linha que nomeia os hosts é o consentimento.** A linha que busca diz a quem
  pergunta ("Perguntar a musicbrainz.org, coverartarchive.org"), e sob ela, enquanto não foram
  permitidos, que pressionar permite. O pressionar vira `Errand::Let`: os serviços gravados no
  `player.json` MESCLADOS com o que está no disco (`store().parsed()`: um arquivo que não é lido é
  recusado com frase, nunca sobrescrito) e só DEPOIS a busca, que o daemon checa lendo o arquivo na
  hora. As chaves da página Internet seguem a mesma regra: `Errand::Keep` leva SÓ o interruptor
  virado (`player::Switch`), deitado sobre o arquivo como está no disco — nunca a cópia inteira que a
  porta leu ao abrir, que devolvia um consentimento retirado noutra porta. A cópia da vista é o que
  ela MOSTRA: o `player.json` é vigiado (`player_internet::Watched`, a mesma vigia das chaves
  guardadas e dos favoritos) e relido quando muda, e relido também depois de toda escrita desta
  porta — uma recusada não fica na tela como feita (`View::errand_carried`). Uma busca recusada por um host não permitido diz isso e ganha a linha "Permitir {host} e
  perguntar de novo", que permite só aquele e repete a busca da mesa. A foto guarda o que teria
  permitido ("would have let the player ask: …").
- **Sugestões.** Um anel de quem sugere — MusicBrainz para uma música, Wikidata para um filme (o
  tipo da folha escolhe o primeiro), TMDB — e o Wikidata diz antes que o pôster só vem com o TMDB e
  o token. O TMDB sem token não pergunta: diz que precisa da chave e leva à página Internet.
  Resultados achados por outro que não o do anel dizem de onde vieram. Uma mesa que voltou sem página
  (sem player, sem resposta a tempo) é esquecida e pedida de novo no próximo assento, sem acordar a
  porta (um player ausente seria perguntado em laço). Cada sugestão é uma linha com
  o NOME (título e artista, dado) na linha inteira e, embaixo e recuado, o que se diz dela (álbum,
  data, ou "lendo os detalhes…"): `Builder::pair_under`, porque uma coluna de valores espremia o
  nome até um toco cortado no meio da palavra numa porta estreita.
- **A página de uma sugestão.** Uma linha por campo que ela traz: o liga/desliga na cabeça (um
  glifo que o clique vira na hora, como `RET` e as setas), o campo, o valor SUGERIDO inteiro — ele
  quebra na coluna dele e nunca é cortado — e, sob ele no registro apagado, "agora: …" com o que o
  arquivo diz. Um campo que o arquivo já diz igual é um fato que diz isso, sem liga/desliga. O
  liga/desliga nasce ligado para um campo em branco e desligado para um preenchido; a capa, ligada
  só num arquivo sem capa. "Gravar n campos" pede `Ask::Take` com exatamente o que está ligado e
  difere do arquivo, e a gravação se diz como toda gravação dos metadados (`State::retag`).
- **As duas capas, uma geometria.** A sugerida fica ao lado da do arquivo: a largura das linhas
  dividida em caixas iguais, `space.player.coverGap` células entre elas, cada imagem posta na caixa
  pela conta de toda capa (`cover_place`), e as caixas tão altas quanto a imagem mais alta nelas (no
  máximo `coverLines`) — numa coluna estreita um quadrado enche a largura antes da altura, e sobrava
  ar entre as capas e as legendas "Sugerida"/"Atual". Uma caixa cuja imagem não chegou guarda o
  lugar. A capa sugerida é buscada pelo daemon quando a página abre (`Ask::SuggestedCover`), no
  tamanho de uma porta (500 px no Cover Art Archive; tirar a sugestão busca a de 1200), e fica no
  diretório de runtime (`preview-<n>`) enquanto a página for a da mesa: uma busca nova a apaga. A
  recusa desse pedido é dita no lugar de "Buscando a capa…". A textura vai sob o arquivo E o carimbo
  dele (hora e tamanho): o `preview-<n>` recomeça do 1 num player reiniciado, e uma textura nunca é
  tirada pela chave. As duas capas são contadas no teto de imagens do quadro
  (`ronoctua_render::image::PLAYER_COVERS`), e `Builder::pictures` não põe mais que isso.
- **Buscar legendas** (Mais recursos, só com vídeo; e a linha da página das legendas). Diz o
  limite primeiro: sem a chave do OpenSubtitles ou do SubDL não há busca, e a página leva à
  Internet. As línguas são a da interface e o inglês, cada uma pelo NOME (`tongue-<código>`, nunca o
  código); a linha "Escolher as línguas" abre a página delas (`PageId::Languages`): toda língua da
  tabela, pelo nome e em ordem de nome, ligada ou desligada — o ponteiro alcança todas —, e uma linha
  para digitar uma pelo código ou pelo nome. Na busca, o × no fim da linha (ou `d`) tira — nunca a
  última, que não tem ×; `RET` no nome só diz como se tira. Uma língua que duas linhas da tabela
  dividem (`pt`, `por`, `zh`) é dita sem região (`tongues::name_key`, `SHARED`, gerado de
  `assets/tongues.nix`): um fluxo marcado `por` é português, e chamá-lo de brasileiro seria palpite.
  Os fluxos do arquivo dizem a língua assim também. Só os serviços com chave são perguntados. Os achados vêm por
  língua, os sincronizados para este arquivo (a impressão do OpenSubtitles) primeiro e marcados,
  com downloads, "para surdos", "tradução automática" e o serviço sob o nome; os downloads
  restantes do dia são uma linha. Escolher um o baixa ao lado do filme e o põe sobre a imagem, e a
  linha diz onde foi salvo.
- **Buscar letras.** Uma fonte (LRCLIB), sem campo de língua e sem tradução, dito no topo. Um anel
  diz para onde vão (um `.lrc` ao lado da música, ou a letra da própria música); ao lado, onde já há
  um `.lrc`, a página pergunta antes, com o "não" primeiro, e só então manda `replace` (o daemon
  recusa com `Exists` quem não perguntou).
- **A página Internet.** Três seções: o que pergunta sozinho (letras automáticas, SponsorBlock e as
  categorias), o que pergunta quando se pressiona — um liga/desliga por serviço (MusicBrainz com o
  Cover Art Archive, que as buscas dele nomeiam juntos) e por diretório, com os hosts NUMA LINHA SÓ
  DELES sob o nome: um host cortado no meio numa porta estreita não nomeava ninguém —, e as chaves
  da pessoa, cada uma dizendo SE está guardada, nunca o quê, e a que host vai. `RET` pergunta numa
  linha mascarada (a conta: o usuário, e depois a senha mascarada); o que se digita é guardado no
  arquivo privado dela (`ronoctua_lookup::keys`, `0600` desde o primeiro byte, sincronizado antes do
  `rename`), o serviço que a linha nomeia é permitido, e guardar e testar vão numa thread PRÓPRIA —
  nunca no laço, que um disco lento não pode segurar, nem na do contador de pedidos, que um pause não
  pode esperar pelos segundos de um serviço —, e a porta diz se guardou e se o serviço aceitou. As
  chaves guardadas são vigiadas: guardada numa porta, a outra a vê. `d` esquece, depois de uma
  pergunta com o "não" primeiro; sobre uma que não está guardada, diz que não há o que esquecer. A
  linha do LRCLIB diz "permitido, pela chave acima" enquanto as letras automáticas o perguntam
  sozinhas, e desligá-la desliga as duas. O nome de usuário da conta não vai ao log do rodapé
  (`player_question::unlogged`). A foto guarda QUAL chave teria guardado,
  nunca a chave.

#### Rádios, TVs e canais por tema

**Rádios, TVs e o Internet Archive são um crate** (`ronoctua-broadcast`). radio-browser, iptv-org e
archive.org são diretórios públicos, sem chave. Cada um vira valores tipados (estação, canal, item)
ou uma recusa `{diretório, porquê}` de um conjunto fechado; a frase é da porta. Os módulos são puros
sobre um `Net`, e nada ali cria thread nem lê relógio: quem chama é um trabalhador do daemon, e
"agora" é argumento.

**Os diretórios são mesas do daemon, e a página já vem com o globo.** Rádios do mundo, TVs do mundo,
a TV de um tema, os filmes do Internet Archive e o que está ao vivo no YouTube são mesas do daemon,
cada uma com a sua thread e a regra das buscas: a listagem nova substitui a que está andando. A
página traz as linhas E as marcas do globo: um país na âncora do atlas, uma estação que diz onde
está no lugar dela, as que não dizem na âncora do país, e as que não têm país que o mapa conheça
contadas como "sem lugar no mapa". A porta agrupa e desenha, e nunca posiciona. Cada linha que toca
traz o `Stream` pronto, com os relés e a origem, e tocar é pôr esse stream na fila.

**O servidor do radio-browser é achado na hora.** Os espelhos mudam (num dia medido, só um respondia
e outro tinha sumido do DNS). Só o nome de entrada, `all.api.radio-browser.info`, fica no código;
a lista vem de `/json/servers`, e só vale um nome sob `api.radio-browser.info`. Sem lista, a
entrada responde por todos.

**O clique do radio-browser espera o som.** O serviço pede que o `/json/url/<uuid>` seja chamado
toda vez que uma estação começa a tocar. O daemon chama depois do `FILE_LOADED` — o som chegou, não
a linha —, uma vez por reprodução: um relé tentado depois de outro que falhou, uma pausa, a mesma
entrada recarregada são a mesma reprodução; parar e tocar de novo é outra. Só com o rádio permitido.
O mesmo começo põe a estação nos tocados há pouco.

**A TV é um índice local.** Os arquivos do iptv-org vêm com gzip (1,6 MB em vez de 11,7 MB) e viram
um índice compacto no cache do player, renovado uma vez por dia sob uma trava do processo — para as
duas mesas de TV não baixarem juntas, e sem segurar o lock do cache, que seguraria as capas. Busca,
país e categoria são respondidos dele, sem rede. Canal adulto, fechado, bloqueado, da categoria
`xxx` ou sem stream que toque nunca entra; stream sem canal também não, porque nada prova que não é
adulto. `UK` vira `GB`, e um código que não é país é recusado, nunca adivinhado. Os streams vão do
mais alto ao mais baixo, e o geobloqueado vai por último; um canal só leva como relés os streams
pedidos com os mesmos cabeçalhos do primeiro.

**Uma lista aparece na hora e se renova por trás.** O que um diretório respondeu fica no cache do
player pelo tempo que `assets/channels.nix` diz (rádio 6 h, TV e archive um dia, lives 30 min). Uma
lista guardada aparece na hora; uma mais velha aparece marcada como velha enquanto a nova é pedida,
e fica, com a recusa, quando o diretório não responde. Busca digitada nunca é guardada.

**Um filme do archive é o arquivo que o mpv toca, aberto quando entra na fila.** A busca pede
`mediatype:(movies)`, senão a coleção responde primeiro as subcoleções. A linha leva o filme
inteiro (o endereço da página dele); ao ser posto, o daemon pergunta ao archive os arquivos, e o
filme vira um stream, ou uma playlist com o nome dele — a página nunca espera a lista de arquivos
de trinta filmes. De cada original fica uma cópia: h.264 até 1080 linhas, depois Ogg/WebM/Matroska,
depois h.264 maior; som só quando o item não tem imagem; miniatura, GIF, metadado e torrent nunca.
O endereço guardado é o `/download/`, que não muda quando o arquivo troca de servidor, e a licença
que o archive diz é mostrada.

**As lives são do YouTube, pelo yt-dlp.** A busca do que está ao vivo agora usa o filtro Live e só
guarda `is_live`; um stream ao vivo tem `network-timeout=12`, para um relé morto ceder em segundos e
não no minuto do mpv. As lives pedem o yt-dlp, que só o perfil de mídia instala: sem ele a recusa diz
isso.

**As categorias são declaradas em `assets/channels.nix`.** Cada tema junta TV ao vivo do iptv-org,
filmes do archive e lives, e toda categoria tem lives, com a fonte dita na tela. O gerador recusa
uma categoria do iptv-org que não existe, conferida contra uma captura da lista deles (pelo carimbo
exato da captura): um erro de digitação seria uma linha de TV sempre vazia.

**Favoritos são gravados pelo daemon.** A estrela é um pedido (`favourite`), e o daemon aplica ao
arquivo como ele está agora, sob a trava dele: duas portas apertando ao mesmo tempo são duas
mudanças, uma depois da outra. O que se guarda vem da página de um diretório, dos tocados há pouco
ou dos próprios favoritos; o que nenhum deles tem é recusado. As portas leem e vigiam os dois
arquivos.

**Os diretórios, nas portas (P9).** Ouvir e assistir ganhou Rádios do mundo, TVs do mundo e Canais
por tema (`LEADER p r`, `p v`, `p g`), e cada uma é uma pilha de páginas (`crate::player_directory`):

- **A página pede a lista dela ao aparecer.** As mesas de diretório são uma por máquina e de todas
  as portas: rádio, televisão, a TV de um tema, o archive e as lives. Por isso uma página que abre —
  ou a que se volta com `Esc` — pede a SUA lista (`View::owing`, junto com o que a tecla fez, num
  `Errand::Then`), e o cache do daemon responde de novo sem rede enquanto a lista é fresca. A porta
  guarda o que a mesa disse de cada lista da pilha dela (`Found::listing`): outra porta que pediu
  outra lista tomou a mesa, e a página desta continua mostrando a própria. A página diz "Perguntando
  a {host}…" desde o pressionar, e não só quando o estado do daemon diz que a mesa está ocupada;
  "Atualizando…" sobre uma lista velha; a recusa em frase com "Tentar de novo" (menos onde outra
  tentativa não adianta: sem yt-dlp, as lives não são listadas); e de quando é a lista. O daemon
  RECUSA NA HORA algumas perguntas antes de tocar na mesa (diretório não permitido, sem yt-dlp, tema
  sem aquilo, palavras vazias, um limite, o player ausente): essa resposta volta à porta com a lista
  a que se referia (`Told::Listing`), e a página a diz no lugar do "Perguntando…". Uma pergunta
  aceita que outra porta substituiu na mesa — a mesa andou, está parada e segura outra lista — deixa
  de estar perguntando, e a página diz que não há lista ainda, com a linha que pergunta de novo. A
  lives recusada pelo yt-dlp é dita pelo nome do tema na língua da pessoa, nunca pelo id.
- **Nenhum diretório nasce permitido.** Enquanto não for, a seção diz a quem perguntaria ("Perguntar
  a radio-browser.info") e que pressionar permite; o pressionar é o `Errand::Let` das buscas (um
  diretório, mesclado no `player.json`) e depois a pergunta. Nada vai para a rede antes, e a página
  que abre sem permissão não pergunta nada.
- **Rádios e TVs do mundo.** O globo, a busca pelo nome (`/`, na linha da porta), Favoritos e Tocados
  há pouco com a contagem, na rádio os Gêneros e as Línguas, e um título Países com uma linha por país
  — o NOME pelo atlas, na língua da pessoa, e quantos. O que o globo não põe no mapa vem sob um título
  próprio, "Sem lugar no mapa (n)": os países sem ponto no atlas, cada um uma linha que abre, e na TV
  "De país nenhum", a lista dos canais sem país (`TvList::Placeless`). Um país abre a lista dele numa
  página titulada por ele; um gênero, uma língua (pelo nome da tabela de línguas: a palavra do
  radio-browser quando ela diz a região — `brazilian portuguese` é pt-BR —, o código quando só uma
  linha o tem, a língua sem região quando duas o dividem) e uma busca abrem a mesma página de
  estações, cada uma com o próprio título. Os Gêneros e as Línguas trazem os que têm mais estações;
  qualquer outro se acha pelo nome (`/`). Uma lista de estações vem em PÁGINAS de
  `table::STATIONS_MOST`, as mais tocadas primeiro (o `offset` do radio-browser): a página diz "As
  500 mais tocadas de 2380 estações — página 1 de 5" (o total vem da linha que a abriu) e tem
  "Estações anteriores" e "Mais estações".
- **Uma linha que toca.** A estrela na cabeça (um controle: o clique guarda ou tira dos favoritos,
  como `b`), o nome na linha inteira e, embaixo no registro quieto, o que ela é — codec e kbps, o
  país numa lista de vários, "não respondeu" — e a linha apagada, o cursor ainda na cor dele — quando
  o stream não respondeu nesta sessão; de um canal
  a altura da imagem, "só em alguns países", "nem sempre no ar" e as cidades que os feeds dão. Numa
  coluna de valores o nome virava um toco em três linhas e o "não respondeu" era cortado. Uma linha
  de várias — esta, um resultado, um rótulo quebrado — acende INTEIRA sob o ponteiro, de qualquer das
  linhas dela, como o clique a toma inteira (`player_rows::whole_row`, nas duas portas). `RET` toca
  agora (depois do que toca, a fila mantida), `a` põe na fila, e o daemon é quem grava os favoritos
  (`Ask::Favourite`); as portas só leem e vigiam os dois arquivos.
- **Canais por tema.** Uma linha por tema da tabela (`assets/channels.nix`), e a página do tema com
  as três fontes em seções, cada uma só onde o tema a tem, com a própria permissão e o próprio
  estado: a TV ao vivo, os filmes do Internet Archive (a busca pelo título, "Página n dos N
  encontrados", os direitos sob cada filme pelo NOME da licença — domínio público, CC BY-SA 4.0 —,
  "Filmes anteriores" e "Mais filmes", que pedem só a página de filmes de novo) e o que está ao vivo
  no YouTube, com as palavras que acharam cada um. Toda categoria do iptv-org é um tema (a tabela
  ganhou as que faltavam), e o Anime não tem TV ao vivo: o iptv-org não tem categoria de anime, e a
  `animation` dele são todos os desenhos, que são do tema Desenhos.
- **O globo, um gesto em toda porta.** Na janela e na coluna, as primeiras `space.player.globeLines`
  linhas da página e uma linha que diz onde o ponteiro está; no rodapé, enquanto o painel de mídia
  está ao lado das linhas, no lugar do filme (a imagem é deixada de pedir enquanto isso), e a linha
  fica entre as linhas. O ponteiro sobre um grupo o acende e diz quantos e o lugar (a contagem
  primeiro: o nome é o que uma porta estreita corta); o grupo aceso vale só para a página em que foi
  achado (`player_globe::Seen::under`) — os ids das marcas recomeçam em toda página, e o velho
  acendia outra estação. O primeiro clique leva o cursor à primeira linha que o grupo representa SEM
  rolar a página (o globo está no topo, e um país lá embaixo o tirava da mão antes do segundo
  clique; `View::globe_keeps_the_scroll`), o segundo faz o `RET` dela — uma estação
  toca, um país abre a página — e um grupo de várias linhas enquadra e RECORTA a lista nelas, com a
  linha "Mostrando n de N: mostrar todos" e `Esc` devolvendo todas antes de sair da página. Arrastar
  (depois de `atlas.clickSlack`) gira, a roda aproxima, `C-k`/`C-j`/`C-r`. A caixa em que o globo é
  pintado e a que o ponteiro lê são uma conta só (`player_globe::box_of`, `picture_px`). No rodapé,
  cada tela guarda onde o desenhou (pelo conector), o ponteiro é lido pela caixa da tela em que está,
  e o globo é medido UMA vez por quadro, na maior — medido por tela, uma trocava o tamanho da outra.
  Um clique no painel fora do globo não pausa nada nem abre o palco: ali não há imagem.
- **A cidade de um canal ainda é dita, não desenhada** (a mesma limitação do daemon): o atlas não
  acha um lugar pelo nome, e o canal fica na âncora do país.

### As colunas — quatro tamanhos, uma linha de controle e um espaço de trabalho cada

**Por que uma base.** Uma coluna é mais que uma janela ancorada à direita: tem tamanho, uma linha
que o muda, uma linha que diz que ela tem o teclado, o espaço de trabalho a que pertence, o slot e
a zona que as janelas precisam ouvir. Escrito por inquilino, o próximo esqueceria um desses. Por
isso `column::Column<T>` é o `door::Content` de toda coluna, como a `Door` é de toda janela, e o
inquilino (`column::Tenant`) só desenha na `Area` que recebe. Ele tem os mesmos ganchos da porta,
cada um com padrão, mais `parked`/`shown` (parar e retomar o trabalho pesado) e
`acts_without_rows`. O cabeçalho dele desce para a linha 1 (`Content::header_line`,
`chrome::Hinted::at`).

**Quatro tamanhos, em células.** Em `space.column`, contadas no conteúdo, mais a margem de cada
lado:

| tamanho | células | por quê |
|---|---|---|
| grande | 72 | meia média a mais: a lista de arquivos do player abre uma segunda coluna, e uma imagem 16:9 chega a ~17 linhas |
| média | 48 | a de sempre; o player abre nela |
| pequena | 32 | a mais estreita em que o transporte de onze botões cabe inteiro, sem folga |
| barra | 3 | 5 com as margens: ímpar, para um glifo ficar no meio, e perto do dobro da espessura da modeline |

Nenhum passa de `maxPct` (45%) da largura lógica da tela (da sala, enquanto o redimensionador corta
aquela tela: `column::room_of`), e nenhum fica abaixo da barra (`column::width_px`). Um tamanho que
o teto deixa igual ao de baixo é desenhado inerte e diz `column-no-room`, e `C-<left>`/`C-<right>`
passam por cima dele. O tamanho não é preferência: toda coluna abre no padrão do inquilino.
`--size` existe para o rodapé levantar de novo uma coluna como ela estava
(`right_column::raise_again`).

**Adaptar, ou conteúdo próprio.** `Tenant::shape(size)` diz como o inquilino preenche um tamanho.
Grande, média e pequena adaptam por padrão (`Shape::Adapted`: as linhas refluem); a barra só existe
para quem tem conteúdo feito para ela (`Shape::Own`). Sem isso, o glifo da barra é inerte e diz
`column-bar-not-offered`.

**A linha de controle.** A linha 0 é da coluna em todo tamanho: `[fechar] │ [G][M][P][B]`, contados
da borda direita (`chrome::from_the_edge`, e o ponteiro os pega por `chrome::at_the_edge`), os
tamanhos do mais largo ao mais estreito, o atual em `text.primary` — nunca no acento, que na coluna
é da linha de foco. Sob o ponteiro, a frase do glifo, curta ("Tamanho grande", "Fechar"), encostada
pela direita no fechar com uma célula de ar (`space.column.saidAir`); em repouso a linha é muda.
Um tamanho recusado também se explica ali, em chave própria e curta ("Sem espaço aqui", "Sem barra
ainda"): a frase inteira do recusado é a da linha do inquilino, e cortada com reticência perdia o
motivo. Um teste confere, em toda língua, que nenhuma frase da linha é cortada na coluna pequena.
O fechar se explica "Fechar"; a tecla `C-q` na gaveta, "Fechar a coluna; o que ela mostra
continua". Na barra, a linha 0 tem um
glifo só, que volta ao tamanho de antes em vez de girar pelos quatro, o fechar fica na ÚLTIMA linha,
contada da borda de baixo (`Span::foot`; o inquilino para acima dela, `column::Area::end`), e
nenhum dos dois se explica: a barra é um controle remoto de glifos conhecidos (decisão do dono). As
teclas — `C-<left>`, `C-<right>` e `C-q` — são lidas ANTES do inquilino, porque o player lê tecla
nomeada sem modificador e tomaria `C-<left>` pelo `<left>` dele; cada id da tabela nomeia seu ato
(`column::Own`), porque com um `_` a terceira tecla estreitava a coluna. O desenho é uma função pura
(`decoration`), e o ponteiro lê o que ela colocou (`glyph_at`).

**A régua do fechar fica onde o ar dos dois lados é igual, medido na TINTA.** A régua é o
`chrome::divider` entre o fechar e o tamanho grande. Pelas células ela ficaria um pixel e pouco fora
do meio que o olho vê: o `md-close` tinge o avanço inteiro e o `md-size_xl` começa a um quinto do
dele, fora do centro. Então o gerador de glifos mede, contra a face real, onde a tinta de cada glifo
começa e acaba (`across` no `icons.json`, `tokens::icon::ink_across`), e a coluna põe a régua no
meio entre o fim da tinta de um e o começo da do outro, com a aritmética de borda do renderizador
— a mesma função, `ronoctua_render::from_the_border`, que o `Text::prepare` usa para os glifos, lida
por `chrome::edge_x` — e o empurrão do `Size::OnTheCell`. Medido na foto: 10 px de cada lado, nos três
tamanhos. A altura é a da linha menos `space.column.ruleInset` em cada ponta — a da tinta dos
glifos, para a régua ser da linha e não uma fronteira entre duas.

**Fechar.** O glifo e `C-q` fecham a COLUNA e nada mais: `Tenant::closing` avisa o inquilino (o
player larga as imagens), a porta sai, e o `Drop` devolve o slot. O que o player toca continua
tocando.

**A linha de foco.** Enquanto algum teclado está na coluna, uma linha de 2 px
(`space.column.focusLine`, o peso da borda de foco do compositor) de borda a borda, sobre tudo
(`chrome::focus_line`), na cor do contorno em foco: o acento com as bordas padrão e
`border.discreet` com as discretas (`theme::focus_line`; ver "As bordas discretas", em "Cores"). Ela
fica na entrelinha acima da linha 0, que nada tinge, então não move span nem marca — o teste compara
os dois quadros. Antes, nada ficava azul quando a coluna tinha o teclado.

**Um espaço de trabalho por coluna.** A coluna pertence ao espaço em que foi aberta, pelo NOME (um
espaço vazio some, e `TAB 3` o recria com o mesmo nome), e aparece na tela que mostra esse espaço:
cada tela mostra a coluna do seu. O contrato ganhou `Workspace { name, visible, output }` e
`Compositor::workspaces()`. A decisão é de duas funções puras, `wanted` (que tela o espaço quer) e
`step` (ficar, mostrar, mudar de tela ou estacionar), e um lugar só muda a superfície,
`Column::place`. Estacionar destrói o papel da superfície e põe no lugar uma superfície sem papel
(`Shell::Parked`): o processo, a fila e o estado ficam. Mostrar pede a tela pelo nome
(`Ui::screen_named`), e uma tela ainda não descrita espera o `outputs_changed`. Uma tela que some
fecha a superfície: a coluna estaciona (`Content::layer_closed` devolve `Closed::Park`), e a
próxima lista de espaços diz onde ela volta. A coluna não segue mais o foco de tela em tela (o
`follow_the_focus` do player saiu): quem decide a tela é o espaço.

**O custo, dito.** Trocar entre um espaço com coluna e outro sem re-arranja as janelas por um
quadro: a zona sai e volta com a superfície. E um espaço que só tem a coluna estacionada some dos
números da modeline; o player continua alcançável pela marca `player`.

**As colunas são um slot por espaço de trabalho, e o inquilino é uma variante**
(`right_column::Tenant`): quem toma um espaço encerra quem estava nele. O invariante 14 fala da
BORDA, não do player; escrita como o arquivo do player, a regra valeria só para ele, e a próxima
coisa que quisesse a borda traria um segundo arquivo — duas colunas lado a lado.

**O slot.** Um arquivo por inquilino, `column-<inquilino>.pid`, com `<pid> <tamanho> <espaço…>` —
o espaço por último, para um nome com espaços sobreviver —, escrito por rascunho e `rename`, cada
processo só o seu. `take` encerra o mesmo inquilino onde ele estiver e quem ocupa o espaço de
chegada (a decisão pura é `to_end`); `toggle` fecha a coluna se ela está AQUI e, se está em outro
lugar, a traz. A guarda contra pid reciclado lê a linha de comando pelo mesmo `Opening::parse` que
abre a coluna. O `column.pid` antigo ainda é lido como o do player.

**Uma pergunta na barra alarga a coluna.** Pergunta é digitada onde é desenhada, e a barra não
desenha cabeçalho: um `Ui::ask` com a coluna na barra a devolve ao tamanho de antes. O mesmo vale
para uma tecla que age sobre uma linha que ninguém vê (`Tenant::acts_without_rows`).

### Programas — `LEADER i`, os programas da PESSOA, e por que não há daemon novo

O dono pediu, numa feature só, instalar, experimentar, atualizar e remover programas **só para a
pessoa** — pacotes Nix, Flatpak, AppImage, apps Android e web apps —, com busca em todas as fontes.
Isso colide com o invariante 6 ("se não está no flake, não existe"), e a exceção foi APROVADA
pelo dono e escrita no próprio invariante: os programas da pessoa ficam fora do flake; o sistema,
o repositório e `/etc` nunca. As duas únicas escritas fora da home são a imagem do Android, que a
pessoa pede (uma unidade root estreita), e o store, que quem escreve é o daemon do Nix. O SUPORTE —
Flatpak, nix-flatpak, `appimage-run`, Waydroid, o índice de busca — continua declarado.

**Por que não há daemon novo.** O desenho do motor propôs um (`ronoctua-softwared`, com socket
próprio), e a fila já é exatamente o que ele seria: um processo de usuário que sobrevive ao rodapé,
com lista, progresso, cancelamento, a marca da modeline e a notificação de fim. Então o trabalho é
um crate com biblioteca e linha de comando (`ronoctua-software`), e a fila roda a CLI como filho —
o molde do yt-dlp (`Work::Software { order, label }`, `ronoctua-software run --progress` com a
ordem em JSON no stdin, linhas `RONOCTUA\tstep|done|failed` no stdout). O filho vai para um escopo
próprio (`systemd-run --user --scope --collect --slice=app.slice -p OOMPolicy=kill`): a unidade da
fila é `OOMPolicy=stop`, e um OOM dentro dela levaria todas as cópias em voo junto. Cancelar é
`SIGTERM` no grupo e `SIGKILL` 3 s depois; **pausar não existe** (`Engine::pauses()` falso), porque
um `nix profile add` parado no meio não é um estado que se retoma.

**Uma pista por fonte, e ela é um ARQUIVO.** A tela manda ordens pela fila e a atualização roda a
mesma CLI de uma unidade: as pistas têm de ser algo que os dois vejam. Um `flock` por fonte
(`lock.rs`) — o kernel solta quando o processo morre, e uma ordem que esperava diz que está
esperando em vez de parecer travada.

**O que a pessoa tem é lido da REALIDADE, toda vez** (`state::read`): o manifesto do perfil Nix
(`nix profile list --json`, v3), `flatpak list --user`, as AppImages com o `app.json` delas,
`waydroid app list`, e os `.desktop` dos web apps. Não há registro de "o que instalamos" para
divergir da máquina: o que se instalou pelo terminal aparece, o que se apagou à mão some.

**Ensaio.** No `.#dev` (`RONOCTUA_SOFTWARE_REHEARSE=1`) toda ordem que muda algo imprime o argv e
não roda: o laço vive na sessão do dono, e experimentar a tela não pode tocar o perfil Nix, os
Flatpaks nem o Android do host. Buscar e ler continuam reais.

#### Nix: o perfil da pessoa, e o não livre só com consentimento

`nix profile add|remove|upgrade|rollback|wipe-history` com `--log-format internal-json`; o
progresso agrega os registros como o `progress-bar.cc` do próprio Nix (lido na 2.34.8), e `-v` NÃO
entra — só acrescenta ~600 linhas de "evaluating file" num `hello`. `nixpkgs#` é a MESMA revisão do
sistema: `nix.registry.nixpkgs` aponta para o input travado por `narHash`. **Não livre só com
consentimento por pacote**, lembrado em `software.json`: o nixpkgs recusa unfree em avaliação pura
e a única porta é `NIXPKGS_ALLOW_UNFREE=1 --impure`, passada também nos upgrades daquele elemento
e só neles. Quebrado, inseguro e indisponível são recusados ANTES de o Nix rodar, pelo índice.
`remove` e `upgrade` procuram o elemento no manifesto antes, porque um nome que não casa só avisa
e sai 0.

**O índice de busca é assado na build** (`nix/software-index.nix`): a receita do próprio nixpkgs
(`packages-info`) sobre o input, com `tryEval` por registro e `lib.meta.availableOn` (o `available`
do meta mentia sobre o Chrome, que é unfree e não indisponível). Portão M1 medido antes de escrever
código: 4,2 GB de pico e 36 s, dentro do teto; 144 198 pacotes, 18,8 MB. Nada vai à memória: uma
busca é uma passada no arquivo (27–61 ms otimizada) e uma consulta por atributo é busca binária em
páginas de 4 KiB (0,08 ms).

**Experimentar sem instalar** (`trial.rs`): `nix build --out-link $XDG_RUNTIME_DIR/…/trials/<k>/out`
— a raiz de GC morre com a sessão, e a coleta do store leva o resto; é isso o "não permanente".
Roda como `ronoctua-try-<k>` (`systemd-run --user --collect --slice=app.slice`), que executa
`ronoctua-software trial-exec <registro>`: o registro guarda o que rodar, e o nosso terminal fica
FORA da casa descartável. Três rastros: **casa descartável** (padrão; `HOME` e `XDG_*_HOME` sob o
cache, apagados no fim), **a casa da pessoa**, e **caixa de areia** (`bwrap`: store e `/etc` só
leitura, casa em tmpfs, só `wayland-1` e `pipewire-0` no runtime, rede ligável). Programa sem
`.desktop` abre o shell da pessoa com o `bin` do pacote na frente do `PATH` — rodar `rg` direto
piscaria uma janela e fecharia. "Ficar com ele" é `nix profile add` do MESMO caminho: instantâneo.

#### Flatpak: sempre `--user`, e o nix-flatpak sem apagar nada

Toda chamada leva `--user` e `--` antes dos posicionais (o GLib só engole `--` quando nada não
lido vem antes). **`-y` e não `--noninteractive`**: na 1.16.6 pinada o segundo troca para uma
transação "quieta" que imprime uma linha por operação e nenhuma porcentagem; com `-y` e
`FLATPAK_FANCY_OUTPUT=0` sai a tabela e o ` NN%`. O progresso é em MILÉSIMOS da transação, nunca
bytes — a saída simples não tem byte nenhum.

**nix-flatpak** (v0.7.0, módulos NixOS e Home Manager) está pronto para o declarativo, com três
travas: `uninstallUnmanaged = false` (senão toda instalação da pessoa sumiria na próxima ativação),
nada de atualização na ativação nem por timer (quem atualiza é a sequência da distro), e o remoto
flathub vem de um `.flatpakrepo` VENDORIZADO (`assets/flatpak/`), para a ativação funcionar
offline. `ronoctua.software.declaredFlatpaks` vira `services.flatpak.packages` do HM — é a linha
que "Tornar permanente" copia. **Tirar um id dessa lista DESINSTALA o app** na próxima ativação
(lido no script gerado do nix-flatpak); o que a pessoa instalou à mão continua protegido. Um app
declarado aparece na lista como do flake e recusa remover (`DeclaredInTheFlake`), com frase.

**Busca**: a API do Flathub quer `pt-BR` — medido ao vivo, `pt_BR` responde em inglês e `pt-br`
em português de Portugal. Sem rede, responde o catálogo appstream que o próprio flatpak deixa
(3 352 apps em 0,14 s, 10,8 MB de pico, cache de 1,84 MB lido em 8 ms, busca em 0,4 ms). As
atualizações são guardadas por COMMIT do remoto e não por versão: o cache para de mentir no
instante em que alguém atualiza pelo terminal.

#### AppImage: `~/Applications`, e o arquivo é lido em Rust

ELF mais `AI\x02`; tipo 1 e DwarFS são recusados com frase. O offset do squashfs é
`e_shoff + e_shentsize × e_shnum` (conferido contra o `readelf` num AppImage real), e o
`unsquashfs` extrai SÓ o `.desktop`, o `.DirIcon` e o `hicolor`. Três medidas da 4.7.5:
`-follow-symlinks` desliga os curingas (os links são resolvidos em Rust, dentro do diretório de
trabalho), `-q` sozinho ainda desenha a barra (é `-n`), e um padrão que não casa sai 0. O
`.desktop` nosso tem `Exec=/run/current-system/sw/bin/appimage-run <arquivo>`. **Atualizar** é
pela última release do GitHub que o `.upd_info` nomeia — resposta guardada 6 h e `If-None-Match`,
porque o limite anônimo é 60 por hora e o ETag alterna entre dois valores (medido); o digest da
release é conferido quando existe, e é `null` em algumas. A extração velha do `appimage-run` sai
junto com a troca; uma órfã espera 24 h, porque pode ser um app aberto à mão.

#### Android: Waydroid, a imagem com Google Play, e o F-Droid verificado

Só no perfil `full`. **Nada é baixado até a pessoa pedir**: preparar inicia uma unidade root
modelo (`ronoctua-android-init@GAPPS` ou `@VANILLA`, `waydroid init -s <tipo>`, que confere o
sha256 do OTA sozinho) — é a única escrita fora da home que a pessoa dispara, e a regra polkit dá
SÓ o verbo `start`, SÓ essas unidades (mais o reset e a do id), SÓ ao usuário. O
`waydroid-container` ganhou `ConditionPathExists=/var/lib/waydroid/waydroid.cfg`: antes do preparo
ele PULA em vez de falhar, porque um sistema `degraded` faz a ativação HM do nix-flatpak pular calada.

**A imagem padrão é a com Google Play (GAPPS)**, escolha do dono; a sem Google continua oferecida.
Com GAPPS a Play Store só funciona depois de registrar o aparelho como não certificado, e por isso a
tela mostra o `android_id` (lido por uma unidade root para `/run/ronoctua-android/android-id`) com
"copiar" e o endereço do Google. O Waydroid sai 0 nos próprios erros: instalar e remover são
confirmados listando os apps de novo.

**O F-Droid é verificado de ponta a ponta**, e em Rust: o `entry.jar` assinado (RSA-2048/SHA-256,
sem atributos assinados) → o certificado fixado (`43238d51…c9ccab`) → o sha256 do `index-v2.json`
→ o sha256 do APK. O índice de 61 MB é compactado em streaming (4 458 apps, 264 só-ARM; 2,5 s em
debug, 25,8 MB de pico, 5,47 MB no disco) e apagado. A versão oferecida é a estável mais nova que
roda em x86 e pede Android 13 ou menos; só-ARM é desenhado recusando.

#### Web apps: `--app`, nunca `--kiosk`

A janela é `chrome --profile-directory=<p> --app=<url>`. **`--kiosk` foi lido na fonte do Chromium e recusado**: vale
para o PROCESSO inteiro do Chrome — ou tranca toda a navegação em tela cheia, ou é ignorado com o
Chrome já aberto — e pede outro perfil, perdendo os logins. Uma janela `--app` lado a lado no
Scroll pede decoração de servidor e sai sem omnibox, abas, barra de título nem botões: só o
conteúdo e a borda de 2 px do compositor, que o dono decidiu manter (é o sinal de foco). O
`--enable-features=WaylandWindowDecorations` saiu do `browser.nix`: ele só existe no Electron, e o
último `--enable-features` da linha vence os outros.

O quiosque do `LEADER m b` não contradiz isto: ele não passa `--kiosk` nem nenhuma das outras três
bandeiras recusadas. É uma janela comum do Chrome, cujas barras o compositor esconde por um feito
(veja "O Chrome quiosque").

O `.desktop` (`~/.local/share/applications/ronoctua-web-<slug>.desktop`) é a ÚNICA fonte da
verdade — URL, perfil e origem do ícone em `X-Ronoctua-Web-*` —, e o `Exec` é
`ronoctua-software web-open <slug>`: a URL nunca entra no `Exec` (é lá que um `%` ou uma aspa
viraria argumento). O `StartupWMClass` é o id que o Chromium dá à janela, e ele NÃO leva a porta
(lido no código do Chromium: `url.host()` e o caminho), então dois web apps que só diferem na porta
são recusados como `DuplicateWindow`. Só `http(s)`, sem credenciais na URL.
`for_window [app_id="^chrome-"] shortcuts_inhibitor disable` é gerado com as regras de janela: um
web app nunca engole a leader.

**Ícones**: do próprio site (manifest `any` → `apple-touch-icon` → o maior `rel=icon` →
`favicon.ico`), do **dashboardicons** (`metadata.json` pelo jsDelivr — e
a variante `light` é o desenho CLARO, para fundo escuro, ao contrário de como se lê), da coleção da
distro (os nomes do superpacote) ou de um arquivo da pessoa. Tudo passa por bytes mágicos, teto de
tamanho, quadrado com fundo transparente e PNG com hash no nome; SVG com `href` externo é recusado.
Frames de ICO são escolhidos por TAMANHO — o `image` prefere profundidade de cor e pegaria 16 px
em vez de 256.

#### O HTTP ficou um só, e aprendeu bytes

O `ronoctua-http` devolvia o corpo como `String` com perda, o que corrompe PNG, ICO e JAR. Agora
há `Reply` com bytes e cabeçalhos, o chamador escolhe o teto, redirecionamento só `https→https`, e
todos os endereços resolvidos são tentados. **E um fechamento sem `close_notify` é FIM, não erro**,
quando há bytes: o WhatsApp Web, o google.com e o httpbin fecham TLS assim, e o framer chamava isso
de `Offline` (com `Content-Length`, o corpo curto continua sendo erro).

#### O lançador passou a ver o que a pessoa instala

Quatro defeitos impediriam a feature de funcionar, e todos eram do lançador:
1. **O carimbo seguia o link até o store**, de mtime fixo: um `nix profile add` só aparecia
   quando o rodapé reiniciava. Agora é o caminho canônico (que troca a cada geração do perfil) mais
   os subdiretórios, e o rodapé reescaneia quando a fila termina um trabalho de programas.
2. **Ícone por nome só era procurado no superpacote.** A cadeia agora é superpacote → `hicolor`
   em `XDG_DATA_HOME` e em cada `XDG_DATA_DIRS` → `pixmaps`, com os tamanhos em ordem NUMÉRICA (era
   lexicográfica: `8x8` antes de `84x84`) e ICO decodificando. Medido neste host: de 140 entradas, o
   superpacote sozinho resolvia 105 e a cadeia resolve 137.
3. **`foreign::launch` relançava** diretamente um app que saía com código ≠ 0. Com escopos, o
   código de saída é do app; a sonda de escopo roda uma vez por processo.
4. **O `PATH` das unidades era o mínimo do systemd**: o `Exec=` de `~/.nix-profile` e o
   `flatpak run` das exportações não abriam, e todo passo `binary` da atualização era pulado. As
   unidades `ronoctua-footer`, `ronoctua-update`, `ronoctua-update-rehearsal` e
   `ronoctua-transfers` têm `enableDefaultPath = false` e herdam o `PATH` da sessão, que o NixOS
   gera de `environment.profiles`. Consequência aceita pelo dono: passos `binary` ligados, como o
   `doom-sync`, passam a rodar de verdade.

O nome na modeline de um web app vem do `Name` do `.desktop` que declara aquele `StartupWMClass`
(sem isso apareceria `com__-Default`).

#### Atualização: uma classe nova, e nenhum passo instala

`does = "upgrade-software"` (rótulo honesto na página do cérebro). Raiz: `flatpak-system`
(`flatpak update --system`, o ÚNICO `--system` da feature) e `android-images` (desligado). Pessoa:
`nix-profile`, `flatpak-user`, `appimages`, `android-apps` → `ronoctua-software upgrade <fonte>`, na
MESMA pista da tela; e `flatpak-unused`, `nix-profile-history`, `appimage-cache` →
`ronoctua-software tidy <fonte>` (o `nix-profile-history` vem desligado: ver *O disco se cuida
sozinho*). A lei de ordem nova do gerador: um passo `upgrade-software` não
roda antes de `apply-configuration`. "Atualizar tudo" pula a fonte que responde `SourceUnprepared`
— uma máquina que nunca preparou o Android não falha toda atualização.

#### A vista: uma `View`, duas portas, e as duas digitam

`LEADER i` é o grupo (`i i` rodapé, `i w` janela, `i t` experiências, `i b` web app novo, `i u`
manutenção, `i a` Android); `a` continua sendo o lançador sozinho. `software.rs` é a vista PURA —
níveis (início, resultados, ficha, experiências, editor de web app, ícones, Android, manutenção,
avançado, permissões, consentimentos, histórico), linhas, teclas e perguntas — e devolve à porta um
`Errand` (enfileirar uma ordem, perguntar na linha da porta, abrir, copiar…); nada nela faz I/O, e
ela não sabe em que porta está. As
threads são do `software_feed.rs` (retrato, busca por geração, ficha, ícones), e as figuras são um
pool próprio (`software_pictures.rs`) cuja chave leva o lado em pixels.
- **Digitar é buscar, nas duas portas**: letra, `DEL` e `C-v` passam pelas mesmas chamadas da
  vista; o Nix responde na hora e as fontes de rede depois de 250 ms parado. Uma URL colada vira
  "criar web app", um caminho de `.AppImage`/`.apk`/`.flatpakref` vira "instalar este arquivo". No
  rodapé a busca aparece também na linha do minibuffer.
- **As duas portas perguntam**: o endereço de um web app novo, o nome e o endereço do rascunho, o
  caminho de uma imagem e o de um arquivo a instalar são uma `line::Line` — no minibuffer do
  rodapé, no cabeçalho da janela — e a resposta chega ao mesmo `View::typed`. A imagem copiada
  vira ícone nas duas, lida da seleção viva fora do laço (`pasting::image`, `software_feed::Kept`).
  O `software-open` do rodapé aceita as palavras da janela (`advanced`, `search <palavras>`,
  `web <endereço>`).
- **Abrir**: um web app uma vez, uma página, o gerenciador de arquivos e a página do brain abrem
  num escopo próprio, pelo caminho único de lançamento (`foreign::start_in_scope`), em qualquer
  porta. Um programa instalado também: o rodapé o acha no catálogo que ele guarda, a janela lê o
  catálogo numa thread (`Ui::launch_entry`), e os dois o iniciam do mesmo jeito (`apps::argv_of`,
  com o terminal para quem precisa). "Não está no lançador ainda" e "não abriu" são ditos na porta
  em que a pessoa está. A janela entregava o id ao rodapé, que dizia a recusa na linha dele e ainda
  fechava a própria vista de programas.
- **Toda pergunta que não se desfaz abre no NÃO** (remover, apagar o Android, devolver as
  permissões, instalar um arquivo que catálogo nenhum conferiu), com o cursor nela.
- **O fim de um trabalho da fila é o evento**: `transfers::ended` diz que um trabalho de programas
  acabou; o rodapé relê o lançador, a vista, as marcas, e diz como ele acabou. Instalado e web app
  criado são ditos no minibuffer, com a ação de abrir o programa no fim da linha (ver "Eventos no
  minibuffer"); os outros (falhou, removido, atualizado, experiência encerrada, Android pronto)
  continuam notificação. Não há relógio.
- **Duas marcas, mudas em repouso**: uma experiência rodando (frasco) e o Android ligado (ele custa
  memória, que é a razão de existir a marca). O clique abre o nível delas.
- **O lançador acha o que não tem**: nada casando, a última linha é "procurar «x» para instalar";
  `C-i` abre a ficha do programa sob o cursor e `C-d` a abre com a pergunta de remover já de pé. Um
  programa que o flake instalou mostra a frase "declarado no flake" em vez de perguntar.
- **Permissões do Flatpak numa forma reduzida** (`Grant`: arquivos da pasta pessoal em três
  alcances, rede, som, dispositivos, Wayland ou X11, e voltar ao padrão), lidas das permissões
  EFETIVAS (`flatpak info --show-permissions`) e escritas por `flatpak override --user`. Não existe
  grant que ponha variável: `GTK_THEME` só pode ser tirado.

#### O modo escuro do Flatpak — medido, e duas linhas declaradas

O dono lembrava de ter ouvido que instalar o "Adwaita-dark" dava tema escuro e quebrava a interface
de vários apps. Nada foi instalado antes de medir. O palco `nix run .#shot -- --apps` (Flatpak
1.16.6, o portal gtk e as duas chaves do dconf que o `foreign.nix` escreve) rodou apps reais do
Flathub, e a medida é a luminância média da janela (0 é preto, 1 é branco), em 2026-09-26:

| App (toolkit) | sem a extensão | com a extensão | `GTK_THEME` forçado | tema claro |
|---|---|---|---|---|
| Eyedropper 2.2.1 (libadwaita, GNOME 50) | 0,151 | 0,151 | 0,151 | 0,966 |
| gedit 50.0 (GTK 3) | 0,146 | 0,146 | — | 0,984 |
| Geany 2.1.0 (GTK 3 simples) | **0,931** | **0,436** | 0,436 | 0,931 |
| KCalc 26.08.1 (Qt 6, KDE 6.11) | 0,155 | 0,153 | — | — |
| FreeTube 0.25.3 (Electron) | 0,170 | 0,170 | — | 0,955 |

- **`org.gtk.Gtk3theme.Adwaita-dark` é o que escurece um app GTK 3 que não sabe do modo escuro**,
  e não mexe em mais nada: o Geany vai de claro a escuro (o 0,436 é o editor dele, que tem esquema
  de cor próprio e continua branco; a janela é escura), o libadwaita anda 0,0002, e o gedit e o
  KCalc não andam, porque seguem o portal sozinhos. O Electron fica onde estava. **E o Flatpak a
  instala SOZINHO** como ref relacionada quando vê `gtk-theme=Adwaita-dark` — o wrapper da CLI põe o
  módulo GIO do dconf no ambiente para isso. Ela é declarada no nix-flatpak mesmo assim: fica certa
  antes do primeiro app GTK 3 e volta depois de um `flatpak uninstall --unused`.
- **O defeito lembrado não se reproduziu**: `GTK_THEME=Adwaita:dark` e `GTK_THEME=Adwaita-dark`
  não mudaram o libadwaita de hoje. Era o libadwaita antigo, ou um tema de terceiro no lugar da
  folha dele. `GTK_THEME` continua proibido (cobra `flatpak-dark-is-never-forced`): com qualquer
  outro nome, o app desenha o tema que a variável manda em vez do dele, e não há o que ganhar.
- **O que QUEBRAVA era nosso**: o bloco `qt` do `foreign.nix` exporta `QT_QPA_PLATFORMTHEME=gnome`
  e `QT_STYLE_OVERRIDE`, que chegam à caixa de areia e nomeiam um plugin e um estilo que o runtime
  do KDE não tem — o KCalc escuro saía CLARO (0,944). `unset-environment` global no nix-flatpak o
  devolveu a 0,153, porque o runtime do KDE lê o portal por conta própria.
- **Trocar claro/escuro com os apps abertos funciona** no libadwaita e no gedit (0,150 → 0,966 e
  0,146 → 0,984, sem reabrir).

O palco teve dois defeitos que só a primeira rodada achou: o gedit abortava no primeiro ícone
(o `glycin` decodifica imagem por `flatpak-spawn --sandbox`, que precisa do `flatpak-portal` na
sessão), e o Chromium recusa rodar como root sem `--no-sandbox` e procura um X sem
`--ozone-platform=wayland`. E o disco da VM foi para o cache: o driver o põe no
`XDG_RUNTIME_DIR`, que é tmpfs, e cada byte de runtime seria RAM.

## 8. Desempenho, grafo e riscos

### Grafo de crates

`tokens`, `i18n`, `protocol` e `cmd` não dependem de nada e são dependência de todo o resto.
`runtime` **não conhece** `render` — produz display list, não rasteriza, e é o que permite testar
buffers headless. `compositor` e `scroll` são folhas: só o `shell` as toca, e só a segunda sabe qual
compositor é. `term` isola todo o `unsafe` e todo o FFI.

As adições ao grafo original, cada uma por um problema real:

- **`ronoctua-glyphgen`** (hoje `nix/glyphgen.nix`). O consumidor da base compila para wasip2 e
  **não tem filesystem**: não pode ler o TTF em tempo de build. Sem gerador separado, ou ele deixa
  de ser wasip2, ou a tabela de ícones volta a ser hard-coded.
- **`ronoctua-cmd`.** Toda funcionalidade tem de ser um comando nomeado. `keymap` resolve a árvore de
  teclas; o **registro de comandos** é outra coisa, e sem crate próprio os ids nascem espalhados
  dentro do `shell`.
- **`ronoctua-playlist`.** Os leitores de lista de reprodução, com dois chamadores: a prévia do
  gerenciador (`ronoctua-inspect::playlist`, que guarda a parte que é sobre TIPO MIME) e o daemon do
  player, que não pode linkar o `ronoctua-inspect`.
- **`ronoctua-xml`.** O XML que vem de fora — uma lista que o servidor de uma rádio mandou, uma
  legenda baixada, as tags que o `mkvextract` tira de um arquivo de qualquer origem — só é lido
  depois de medido (`ronoctua_xml::parse`). O `roxmltree` recursa uma vez por nível e não tem teto:
  cem mil `<a>` cabem sob qualquer teto de bytes e estouram a pilha, e estouro de pilha não é pânico
  que uma thread segure — derruba o PROCESSO (o daemon do player, o rodapé na prévia). A contagem
  lê o texto antes de o leitor vê-lo (teto `DEPTH_MOST`, 48) e recusa uma entidade cujo valor é
  marcação, a única via por onde o aninhamento cresceria longe da vista. A guarda nasceu dentro das
  legendas do player; três crates leem XML de fora (as listas, as legendas e as tags do Matroska), e
  uma guarda copiada em cada um é a que o próximo leitor esquece. O `ronoctua-inspect` (3MF, AMF,
  DAE e os formatos de escritório) ainda lê pelo `roxmltree` direto; uma cena do DAE aninha fundo de
  verdade, e o teto dele é outra medida.
- **`ronoctua-player` e `ronoctua-player-engine`.** O cliente, o protocolo e as regras puras de um
  lado; o daemon, o único que cita o libmpv, do outro (veja "O player"). O `ronoctua-player` não
  tem rede nem processo: a fila, a sessão, a disposição e as legendas são funções que a porta e o
  daemon chamam do mesmo jeito.
- **`ronoctua-tags`.** A única fronteira com as etiquetas e as capas de um arquivo de mídia.
  `sheet` e `can` são o esquema, só serde; a feature `engine` traz o lofty, o `image` e um
  executor de programas próprio, pelo caminho. Próprio porque o `ronoctua_inspect::tools` recebe
  o enum `Tool`, e uma variante nova ali obriga a variável na unidade do rodapé
  (`document-formats-are-total`), que não grava arquivo nenhum.
- **`ronoctua-lookup` e `ronoctua-broadcast`.** As buscas online e os diretórios de rádio, TV e
  filmes. A feature `net` (a TLS do `ronoctua-http`) é padrão; sem ela ficam os tipos, os
  parsers, as chaves e a tabela gerada. Nenhum dos três depende do `ronoctua-player`: é o
  protocolo que depende deles, sem features, para levar os tipos deles no fio.
- **O esquema sem feature não promete que a porta não linka TLS.** O Nix compila o workspace
  numa rodada só, e o Cargo unifica as features: o shell já linka a TLS pelo `ronoctua-software`.
  A feature desligada diz o que a porta USA, e os manifestos dizem isso.
- **`ronoctua-ytdlp`.** Onde o yt-dlp está (`program`, `installed`), as flags do dispositivo e as
  duas listagens do player — a playlist sem baixar nada (`watch?v=…&list=` é pedida como
  `/playlist?list=`, porque a outra forma responde um vídeo só) e a busca do que está ao vivo —,
  cada execução com prazo de 25 s e morta quando largada. A fila e o player o chamam; ele não
  conhece o `ronoctua-playlist`: fazer de uma listagem um `Stream` é do daemon.
- **`ronoctua-audio`** ganhou uma assinatura que para quando é largada (o `pw-dump` morre com
  ela), e continua o único que roda um programa do PipeWire.
- **`ronoctua-atlas`** continua `no_std` e sem dependências. As marcas e as âncoras entraram nele,
  e não no shell: agrupar pontos é geometria, e o player não é o último a pôr pontos num globo.
- **`ronoctua-clock`.** Uma fronteira, um crate — a regra que o `ronoctua-disks` e o
  `ronoctua-power` já seguem. Dois chamadores querem o `timedate1`: a vista `LEADER s l` e a vigia
  do brain. Um segundo leitor de `timedatectl` seria uma segunda resposta sobre um relógio só.
- **`ronoctua-software`.** Biblioteca e CLI dos programas da pessoa, com três chamadores: o shell
  (lê, busca, detalha), a fila (roda a CLI como filho; depende da biblioteca sem features) e a
  atualização (roda a CLI de uma unidade). As features `net` (TLS pelo `ronoctua-http`) e `draw`
  (`image` com ICO e `resvg`) nascem desligadas; o binário exige as duas.
- **`ronoctua-browser`.** O Chrome em execução, pequeno de propósito: um leitor de JSON e um socket
  (`serde`, `rustix`, `ronoctua-xdg`), sem renderizador e sem TLS. Tem dois chamadores: o rodapé
  (o servidor do socket, a linha de comando do quiosque, as regras de endereço e o vocabulário da
  ponte) e a fila (o enquadramento do native messaging, que antes era uma cópia dela). O binário
  `ronoctua-browser` é o host que o Chrome sobe, extraído em `rust.browser`.

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
| R2 | **Deriva do IPC do compositor** contra "sem fork". | Metade desse IPC é o do i3, congelado por compatibilidade há uma década. A outra metade — os comandos do fork — é conferida em BUILD: o gerador recusa um verbo que a fonte pinada não tenha. O contrato roda contra a tag pinada, e não contra uma segunda, porque o risco deixou de ter o mesmo tamanho. Se um verbo sumir: **parar e reportar**. |
| R3 | **Instanciação de 5 ms** contra os dados do `glyphdb`. | `memory_init_cow` + AOT `.cwasm`, com gate de CI que mede e falha acima de 5 ms. |
| R4 | **Xwayland quebrando PID → cgroup.** | Resolvido pelo compositor: o XWayland é embutido e ele reporta o PID do cliente X real (extensão XRes), então não há processo intermediário para confundir a correlação. Era um satellite por scope. |
| R5 | **Closure de fonte de ~10 GB.** | Cinco derivações; o ISO leva 62 MB; `noto-fonts-extra` isolado e removível por opção de host. |
| R6 | **Serviços de fora mudam sem aviso** — os formatos do OpenSubtitles, SubDL, TMDB, MusicBrainz e Wikidata foram escritos da documentação; o filtro Live do YouTube é interno dele; o Podnapisi já sumiu. | Cada serviço devolve uma recusa tipada, nunca um pânico; um campo de tipo inesperado perde o campo e não a resposta; os testes rodam sobre respostas gravadas; nada busca sem consentimento, e a tela diz o que falhou. |
