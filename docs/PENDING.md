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


### 1. `ronoctua-design` não existe · decidir antes

O invariante 12 nomeia `ronoctua-design` e `ronoctua-glyphdb` como componentes `wasm32-wasip2`. O
`glyphdb` virou dados gerados por `nix/glyphgen.nix` e consumidos por `buffers/glyphs` — efeito
equivalente. Já o `ronoctua-design` — a camada de componentes que produziria a display list, sem
threads, filesystem nem wgpu — não existe em forma nenhuma: cada buffer a monta direto do
`ronoctua-protocol`.

**Decidir primeiro:** o invariante descreve algo que ainda queremos, ou a realidade o superou? Se
superou, **o invariante muda** — e isso é decisão do dono, não do agente.

- acabou quando: ou existe `ronoctua-design` compilando para wasip2, ou o invariante 12 foi reescrito

### 2. O teto de 4 GiB do ISO · decidir antes

`iso-fits-usb-stick` recusa um ISO maior que 4 GiB, e **ele está em 3720 MiB** — medido no gate de
2026-08-21, com a fila de quatro motores e o libtorrent dentro. Sobram **376 MiB**, ou 9,2%. Os
quatro motores custaram **8 MiB**, e não os 62,7 do fecho do libtorrent: quase tudo que ele traz
(openssl, a `libstdc++`) já estava lá — o que é uma medição, não uma expectativa. O dono levantou que o
teto vai apertar conforme mais aplicativos e funcionalidades entrarem, e o número diz que ele tem
razão e que a margem é de uma feature grande, não de várias — mas
**o número não é arbitrário e a decisão precisa do fato**: 4 GiB é o limite de
tamanho de arquivo do **FAT32**, e ele só morde quando o `.iso` é **copiado**
para o pendrive (Ventoy, ou uma ESP de onde a UEFI dá boot no arquivo). Gravado
com `dd`, que escreve setores crus, não existe limite nenhum.

Então a escolha é entre duas coisas, e nenhuma delas é "aumentar o número":

- **Levantar o teto e assumir `dd`.** O ISO cresce à vontade e deixa de caber em
  Ventoy. Quem instala grava com `dd`, que já é o que o `INSTALL.md` manda.
- **Manter o teto e tirar a semente do ISO.** O `storeContents` é o que pesa —
  ele carrega o que nenhum cache público tem. Sem ele o ISO fica pequeno e a
  máquina alvo volta a **compilar** o shell, o niri pinado, o `libghostty-vt` e
  os buffers, que é exatamente o que a semente resolveu.

Uma terceira, mais trabalhosa: a semente numa **segunda partição** do mesmo
pendrive, fora do arquivo `.iso`. Cabe em FAT32 porque são vários arquivos, e
custa um passo a mais na gravação.

E uma **quarta, que é a que resolve em vez de adiar** — porque o dono já
levantou que muitos aplicativos ainda vão entrar, e as três acima escolhem entre
perder Ventoy, perder a semente ou ganhar um passo de particionamento:

- **Dois arquivos, e nenhum deles cresce sem limite.** O `.iso` sai pequeno e
  **sem `storeContents`** — volta a caber em qualquer lugar, Ventoy inclusive —,
  e a semente vira um **segundo arquivo ao lado**, um dump de store
  (`nix-store --export`, comprimido), que o instalador procura no volume de onde
  bootou ou num caminho dado por `--seed`. Gravar continua sendo copiar: dois
  arquivos em vez de um. Se a semente um dia passar de 4 GiB, ela se parte em
  pedaços numerados, que é coisa que um arquivo de dados aceita e uma imagem de
  boot não.
- **O teto deixa de ser sobre o sistema e passa a ser sobre o MEIO.** Hoje
  `iso-fits-usb-stick` mede o `.iso` inteiro e por isso cresce junto com a
  distro. Medindo só a imagem de boot, o número para de subir a cada aplicativo
  novo — e o check passa a dizer o porquê de 4 GiB (o tamanho máximo de arquivo
  do FAT32), que hoje ele não diz.
- **Custo:** mexe em `nix/installer/`, na derivação do ISO e no crate do
  instalador (achar, verificar e importar a semente antes do `nixos-install`),
  mais um check de que o ISO sozinho ainda instala — só que compilando. Não é
  pequeno: é do tamanho de uma funcionalidade, não de um ajuste.

- decidir: qual das quatro; e o `iso-fits-usb-stick` passa a dizer o porquê do
  número que ficar

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
