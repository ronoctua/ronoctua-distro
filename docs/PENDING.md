# O que falta

## Regra de manutenção — leia antes de mexer

**Ao concluir um item daqui, remova-o.** Se ele mudar *como o sistema funciona*, escreva uma linha
no `CLAUDE.md` (se todo agente precisa saber) ou no `docs/ARCHITECTURE.md` (se é o porquê de uma
decisão). Se não mudar nada de fundo — é só trabalho feito —, **apenas apague**.

Esta lista não guarda histórico. Item riscado, marcado como pronto ou mantido "para registro" a
transforma em changelog — e aí ninguém mais confia que o que está escrito ainda falta. Há um check
que recusa esses marcadores.

Todo item traz **como saber que acabou** e o **tamanho** aproximado. Um enum de tokens e uma
reescrita do protocolo são coisas muito diferentes, e misturá-las esconde isso.

---



## Funcionalidade especificada e ausente

### 0.0001. O realce de sintaxe deixa quatro coisas de fora, de propósito · pequeno cada

Não são esquecimentos; cada uma tem um motivo estrutural escrito no `ARCHITECTURE`, e estão aqui para
que ninguém as descubra achando que achou um defeito.

- **Heredoc** (`<<EOF`, `<<'PY'`). O terminador é texto arbitrário descoberto em tempo de execução, e
  o `State` que atravessa linhas cabe em oito bytes de propósito — texto não cabe lá. Custaria o
  `Copy` e poria uma alocação no orçamento do frame.
- **Título setext** do markdown (o sublinhado de `===`). Precisaria do conteúdo da linha ANTERIOR, e
  o desenho inteiro é "uma linha mais oito bytes".
- **Linguagem dentro de `<script>` e `<style>`** no HTML. O `State::inner` já tem a vaga; o que falta
  é o markup delegar, e isso não custa mudança de tipo nenhuma.
- **A segunda leva de linguagens** — `erlang`, `elixir`, `lisp`, `ocaml`, `crystal`, `julia`,
  `fortran`, `verilog`, `vhdl`, `tcl`, `groovy`, `d`, `vala`, `nim`, `qml`, `gherkin`, `rpm-spec`,
  `gettext` e o resto. **É só dado**: um attrset cada em `assets/syntax.nix`, zero Rust, zero papel
  novo, zero cor nova. É esse o ponto da forma.

- acabou quando: o item que se resolveu sai desta lista; nenhum deles muda como o sistema funciona,
  então nenhum precisa de linha no `CLAUDE.md`

### 0.0002. Nenhum buffer wasm usa o realce ainda · pequeno

O caminho está inteiro e provado por teste: `ronoctua-design` tem a feature `syntax`, o
`Palette::syntax` está implementado dos DOIS lados, o `Screen::code` faz a aritmética byte→célula uma
vez, e o `nix/buffers.nix` já copia o crate e passa a tabela. O que não existe é um buffer que peça a
feature — nenhum dos oito mostra conteúdo de arquivo.

Medido, para quando alguém for: com o crate na árvore e a feature DESLIGADA, o `format.wasm` cresceu
366 bytes; a tabela são 55 KB de fonte, e é isso que um buffer paga ao ligá-la.

- acabou quando: um buffer liga `features = [ "syntax" ]` e desenha uma linha colorida numa foto do
  `nix run .#png`



### 0.0. As nove vistas do rodapé não têm estado · pequeno, e é uma decisão

O estado é do BUFFER, e um buffer é uma janela do compositor (invariante 4): o `Esc` que volta ao
base é resolvido no processo que tem o teclado, e é por isso que ele pode existir sem roubar o `Esc`
do `vim`. As vistas do rodapé — transferências em `t t`, arquivos, rede, clipboard — não são janelas:
são painéis que o próprio rodapé desenha e que somem quando se escolhe algo.

Elas têm estados de fato (o gerenciador tem query e pergunta de permissão; a fila tem lista e
pergunta), e hoje a modeline não os mostra. **Isto não foi esquecido, foi deixado de fora**: o
mecanismo que o levaria até lá é outro — o rodapé teria de reportar a si mesmo — e a inconsistência
que isso cria (`t t` sem estado, `t w` com) é menor que a de inventar um segundo caminho.

- acabou quando: uma vista do rodapé aparece na modeline com o mesmo vocabulário de um buffer, sem
  um segundo mecanismo de relato

### 0.001. O minibuffer não mostra os submenus montados em runtime · pequeno

`SPC m` e `SPC b s` são listados a partir do que a janela em foco publicou, e por isso não estão na
`ronoctua_keymap::Tree` — as ligações dela são `&'static str`. A consequência é que a sequência
pendente que o minibuffer desenha vem só do `Leader`, e ela **para no nível anterior**: com a lista
de estados na tela, a linha diz `LEADER b-` e não `LEADER b s-`.

Isto **não é novo**: o `SPC m` faz o mesmo desde que existe, e o `buffer_pending` dele nunca foi
alimentado no prefixo. O estado só acrescentou um segundo caso, e por isso vira item — dois é
padrão, um era acidente.

O conserto é um só e serve para ambos: o prefixo do minibuffer passa a somar o que os submenus de
runtime têm pendente, em vez de ler só a sequência do `Leader`.

- acabou quando: com a lista de estados aberta, o minibuffer diz `LEADER b s-`, e o `SPC m` diz
  `LEADER m-`

### 0.01. Nenhum buffer usa o `tone` explícito · pequeno

O descritor deixa um estado escolher o papel da cor (`accent`, `success`, `warning`, `error`) em vez
de aceitar a regra padrão, e os seis buffers usam `standard`. O caminho está provado no runtime (um
tom desconhecido cai em `standard` em vez de sumir) e no teste de forma, mas **não** ponta a ponta
numa tela.

O primeiro candidato real é a pergunta de permissão do gerenciador de arquivos, que é `warning` de
verdade — e ela depende do item acima.

- acabou quando: um estado sai numa cor que não é o verde de todos os outros, e há check disso


### 0.015. Os passos que o brain desenha e os que a unidade roda não têm quem os compare · pequeno

A tabela é uma só (`assets/updates.nix` gera, e o brain a LÊ por `build.rs`), então os ids não têm
mais como divergir — foi assim que o defeito anterior morreu. O que continua sem prova é a outra
ponta: a **unidade** escreve o relatório com esses ids, e nada confere que ela escreve os mesmos.
Hoje é o mesmo gerador que produz os dois, o que torna a divergência improvável e não impossível.

- acabou quando: um check lê os ids da tabela e os ids que o roteiro gerado escreve no relatório, e
  falha se um lado tiver um que o outro não tem

### 0.016. A faixa de notificação não é descontada do teto da expansão · pequeno

`expansion_ceiling` recebe quanto é fixo, e o rodapé passa `FOOTER_LINES + brain_line()`. A faixa de
notificação — até três linhas — continua de fora, então com ela na tela a região passa do percentual
que o token promete. É anterior à linha do cérebro e não foi criado por ela; o que a linha fez foi
tornar o número um parâmetro, que é o que permite consertar isto numa linha.

O que falta decidir é se ela DEVE entrar: a faixa é transitória de um jeito que as outras linhas não
são, e descontá-la faria a lista encolher e crescer sozinha enquanto uma notificação passa.

- decidir: se a faixa entra no termo fixo; se entrar, é um argumento a mais na chamada

### 0.02. Notificação não toca som · pequeno

O campo `audio` existe ponta a ponta — protocolo, catálogo, filtro, gerenciador, disco — e **não
faz nada**. Ele não está mudo: `ronoctua_notify::INERT` diz por quê, o gerenciador desenha a razão
ao lado do interruptor e o check `switches-that-do-nothing-say-so` cobra os três. Mas ele continua
sendo um interruptor que não interrompe, e este item é a dívida.

O caminho já está medido: o pipewire que a distro liga traz `pw-play`, e o
`sound-theme-freedesktop` são uns 400 KB com `bell.oga`, `message.oga` e `dialog-warning.oga`. O que
falta decidir é qual som para qual nível — e isso é escolha do dono, não de quem implementa.

- acabou quando: uma notificação com `audio = true` toca, o `INERT` perde a linha do `audio`, e o
  check acusa se ela voltar

### 0.03. Glifo por nome só vale para o que é DECLARADO · pequeno, e é uma decisão

**Isto está em uso**: a entrada `screenshot` nomeia `md-monitor_screenshot`, porque nada no design
system diz "captura de tela" — o token `camera` é uma câmera de vídeo. O que falta é a outra porta.


`assets/notifications.nix` aceita um token (`camera`) ou um nome Nerd Fonts qualquer
(`md-camera_iris`), e resolve os dois no BUILD. Isso cobre tudo o que existe hoje: a distro declara,
o gerador resolve, e o rodapé não carrega tabela nenhuma.

O que não existe é a outra porta: um serviço externo mandando `md-bell` pelo barramento. Para
resolver isso em runtime o rodapé precisaria dos 10.473 nomes — 2,8 MB de `glyphs.json`, ou uma
tabela nova e enxuta lida **sob demanda** de `RONOCTUA_GLYPHDB`, no molde do `RONOCTUA_ICONS`. Custa
zero enquanto ninguém pedir, e é por isso que fica para quando a API existir.

- acabou quando: `notify-send --hint=string:glyph:md-bell` desenha o sino, e não desenha nada quando
  o nome não existe

### 0. A extensão instalada fora da Web Store não foi provada num Chrome de verdade · médio

Tudo o que é conferível já é conferido, e por check: a chave nasce na máquina, o `.crx` é assinado
(a assinatura foi verificada com `openssl` contra a chave pública, do jeito que o Chrome verifica),
o id da política é o id do `.crx`, e o `allowed_origins` do host de native messaging nomeia
exatamente ele. O que **não** foi provado é o último passo: que uma build específica do Chrome
honre `ExtensionSettings` com `installation_mode: force_installed` e um `update_url` `file://`.

Não é código: é abrir o navegador depois de um rebuild e ver se a extensão está lá.

- **se o Chrome recusar**, a saída honesta é carregar desempacotada uma vez pelo
  `chrome://extensions` e escrever aqui que foi preciso. **Vigiar a pasta de downloads não conta**:
  aquilo não pausa, não cancela e chama de transferência qualquer arquivo salvo.
- acabou quando: um download começado no Chrome aparece em `t t` com pausa e cancelamento reais,
  sem ninguém ter clicado em nada para instalar

### 0.1. O gerador de PO token roda sem o `canvas`, e isso pode parar de bastar · pequeno

O `canvas` está nas dependências do gerador e **nenhum arquivo sob `server/src` o
importa** — o jsdom só o procura se uma página pedir um contexto 2D. Ele é
instalado com `--ignore-scripts`, então a ligação nativa não é construída, e a
saída traz uma linha dizendo isso. Medido: o token é cunhado e aceito assim
mesmo, e o plugin lê a ÚLTIMA linha da saída, então o aviso é inofensivo.

O que isso arrisca é o dia em que o BotGuard passar a exigir uma impressão de
canvas de verdade: aí os tokens seriam recusados, e o sintoma apareceria como
vídeo que não baixa, não como erro do gerador.

- **o conserto, se chegar esse dia:** construir o `canvas` — o que traz Cairo,
  Pango e librsvg para o fecho e roda node-gyp dentro do sandbox. Não vale
  pagar isso por uma sonda que hoje não é usada.
- acabou quando: ou o aviso some, ou está escrito aqui que ele parou de bastar

### 0.15. Apagar num pendrive não tem lixeira · pequeno, e é uma decisão adiada

`trash.rs` recusa atravessar sistema de arquivos com `EXDEV` em vez de copiar, e a recusa está
certa: mover para a lixeira promete ser instantâneo, e copiar quarenta gigas atrás desse gesto seria
a promessa quebrada. O que faltava era a pessoa saber POR QUE dois caminhos estavam em sistemas
diferentes — e a barra lateral resolve isso, porque agora o pendrive está na tela com linha própria.

**A barra melhorou a recusa em vez de criar a necessidade de substituí-la**, e é por isso que isto
fica aqui em vez de ter entrado junto. O que falta é o `$topdir/.Trash-$uid` da especificação, e ele
tem um modo de falhar que é próprio de removível: apagar escreve o arquivo de volta NO pendrive, e
quem desconecta em seguida acha que liberou espaço. Some a isso que num vfat não há dono, então
`.Trash-$uid` e o `$topdir/.Trash` da especificação respondem diferente a "quem pode ler isto".

Custo real: um teste honesto precisa de um SEGUNDO sistema de arquivos, o que quer dizer VM — e as
quatro provas que `trash.rs` tem hoje rodam contra uma árvore temporária, sem barramento e sem
máquina.

- acabou quando: apagar um arquivo num pendrive o põe em `$topdir/.Trash-$uid`, o gerenciador o
  lista de volta, e desmontar não deixa a lixeira num volume que não está mais lá

### 0.2. Matar a unidade no meio de uma cópia deixa o pedaço · pequeno

Desistir pelo painel desenrola e remove o que foi escrito; `systemctl --user stop` não — o processo
não tem tratador de `SIGTERM`, e escrever um exigiria `unsafe` ou um crate novo num crate que é
`forbid(unsafe_code)`. `TimeoutStopSec=5` limita o atraso, não a sujeira.

- acabou quando: parar a unidade com uma cópia em andamento não deixa arquivo pela metade no destino


### 3. Desempenho medido só em host não-NixOS · pequeno

`nix run .#bench` mede num niri **aninhado** dentro de outro compositor, com o contorno de driver de
`nix/graphics-host.nix`. O caminho gráfico da distro instalada é mais curto.

- acabou quando: o mesmo comando roda num NixOS instalado e os números entram no `ARCHITECTURE.md`

### 4. O `--execute` chega ao passo 5 de 10 · pequeno

**Três defeitos do instalador saíram daqui, e nenhum deles o modo ensaio podia ver** — ele imprime os
comandos em vez de rodá-los. Os três estão corrigidos:

- **O diretório de trabalho ERA o store.** `cp -rT` não segue symlink dado na linha de comando, e
  `/etc/nixos` é link para o store: escrever o `host.nix` dava `Read-only file system`. É `-rTL`.
- **O `disko` não podia funcionar.** O `host.nix` gerado diz `facterReport = ./facter.json`, e o
  `disko --flake #noctua` AVALIA esse arquivo — que só era criado três passos depois, dentro do
  `/mnt` que o disko ainda não montou. Toda instalação morreria no passo quatro. O `nixos-facter`
  roda antes agora, no diretório de trabalho.
- **O nó instalador não bootava.** Raiz em disco de rascunho depende do `autoFormat`, que o initrd do
  systemd não implementa (`Can't find ext4 filesystem`, pânico aos 2,7 s; sem o initrd do systemd,
  `Attempted to kill init!` aos 1,2 s). Raiz em **tmpfs** não tem esse passo, e deixa o `/dev/vda`
  inteiro para o disko.

**O teste existe em `tests/installer-vm.nix` e NÃO está ligado ao `tests/checks.nix`** — check
vermelho que todo mundo aprende a ignorar é pior do que lacuna assumida.

**Onde ele para: custo, não defeito.** O passo 5 (`disko --mode destroy,format,mount --flake`)
avalia a configuração NixOS inteira DENTRO da VM, e os 900 s de timeout padrão do driver não bastam —
sai com 124 sem terminar. A próxima tentativa é: `timeout` maior no `execute`, mais memória no nó, e
o `diskoScript` no `system.extraDependencies` para a avaliação ter menos a fazer.

- acabou quando: `--execute` roda até o fim contra um disco virtual, e o sistema que ele instala boota

### 5. Clipboard: o ClipCascade não está empacotado · decidir antes

O painel mostra `ClipCascade: não integrado` e o comando de configurar diz o mesmo — honesto, e
reservando o lugar.

**O obstáculo é anterior ao código: ele não existe no nixpkgs.** Integrar exigiria empacotá-lo
primeiro, e ele não é um binário solto — é um cliente que fala com um **servidor próprio**, com
credenciais e um endereço para guardar. Isso traz duas coisas que este sistema trata com cuidado:
mais um segredo (invariante 13) e a área de transferência saindo da máquina.

A pergunta que vem antes de qualquer linha: **o histórico de clipboard deve sair deste computador?**
O `atuin` já respondeu não para o histórico do shell, e pelo mesmo motivo — um comando com token no
argv não deve atravessar a rede. Um clipboard carrega senha colada com a mesma frequência.

- decidir: se sim, o item vira empacotar o ClipCascade e guardar a credencial como os outros
  segredos; se não, o lugar reservado no painel sai e o comando some do keymap
