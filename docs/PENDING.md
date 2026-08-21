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

### 0. A fila de transferências tem UM motor de quatro · grande, e em degraus

`LEADER t t` já existe: a fila é uma unidade própria, as sete operações longas do gerenciador entram
nela, a modeline mostra ícone e porcentagem, e dá para segurar e desistir. O que existe é o motor
**local** — o disco desta máquina. O dono pediu que esse mesmo painel seja o gerenciador de tudo o
que se transfere, e os outros três degraus não foram feitos:

- **O buffer wasip2 (`t w`).** A mesma fila numa janela própria, no molde do painel de memória:
  capacidade nova no WIT (`transfers`), `input::ledger` entrando e `effect::ask` saindo, lowering no
  `ronoctua-runtime`, `buffers/transfers` com workspace próprio, entrada na semente e no
  `iso-carries-the-seed`. **A tecla `t w` ainda não existe** — não há atalho pendurado no vazio.
  - acabou quando: `t w` abre janela mostrando a MESMA fila que `t t`, e pausa e desiste por lá

- **Download comum e vídeo.** `aria2` (1.37.0) e `yt-dlp` (2026.07.04) **já estão instalados**
  (`nix/modules/cli.nix`, `nix/modules/media.nix`): falta a unidade do aria2 com RPC em 127.0.0.1 e
  `--rpc-secret` **gerado na máquina** (invariante 13), o motor que espelha o `tellActive`, o motor
  do yt-dlp lendo `--progress-template`, e o `t a` que pergunta o endereço.
  - acabou quando: `t a` com um link direto baixa pelo aria2 e com um link de vídeo pelo yt-dlp, e os
    dois aparecem na mesma lista das operações locais

- **Torrent, com `librqbit` como biblioteca da unidade** — o app próprio que o dono pediu, e não o
  `rqbit` empacotado. Traz tokio ao workspace, confinado a este binário. Mais o `.desktop` que
  registra `x-scheme-handler/magnet` e `application/x-bittorrent`, que é o que faz um magnet chegar
  por `xdg-open` **sem depender de extensão nenhuma**.
  - acabou quando: um magnet clicado em qualquer lugar vira linha na fila, baixa, pausa e semeia

- **O navegador manda para cá.** Extensão **nossa**, construída no flake (não da Web Store —
  invariante 6), que cancela o download do Chrome e entrega a URL por **native messaging** a
  `ronoctua-transfers`. Native messaging é o caminho limpo: sem porta TCP e sem segredo
  compartilhado, e o manifesto em `/etc/opt/chrome/native-messaging-hosts/` é declarável em Nix.
  - **O risco está aqui, e é anterior ao código:** instalar extensão fora da Web Store no Chrome
    estável **não está estabelecido** — o `--load-extension` foi restringido nas versões recentes e a
    política de instalação local varia. Este degrau começa por PROVAR isso numa VM. Se o Chrome
    recusar, a saída honesta é ficar com o magnet e o `.torrent` por `xdg-open` mais o `t a` para
    colar link, e escrever aqui o que faltou. **Vigiar a pasta de downloads não conta**: aquilo não
    pausa, não cancela e chama de transferência qualquer arquivo salvo.
  - acabou quando: um download começado no Chrome aparece em `t t` com pausa e cancelamento reais

### 0.1. BitTorrent v2 não vem com o librqbit · grande, e decidir antes

Medido, não suposto: no `rqbit` 8.1.1 do nixpkgs pinado, `TorrentMetaV1Info`
(`crates/librqbit_core/src/torrent_metainfo.rs`) não tem `meta version`, `file tree` nem
`piece layers`, e o hash é o crate `sha1w`. O único vestígio de v2 é um alias de tipo `Id32`
comentado como *"used in Bittorrent V2"*, sem nada atrás. **V2 é trabalho DENTRO do librqbit** —
metainfo v2, verificação Merkle com SHA-256, torrent híbrido e as extensões v2 do protocolo de peers.

- **decidir antes:** upstream, fork nosso (o invariante 3 proíbe forkar o **niri**, não uma
  dependência), ou FFI para `libtorrent-rasterbar`, que já tem v2. O tamanho muda por um fator grande
  entre as três.
- acabou quando: um torrent puramente v2 baixa e verifica

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

`iso-fits-usb-stick` recusa um ISO maior que 4 GiB, e **ele está em 3712 MiB** — medido no gate de
2026-08-20, com o gerenciador de arquivos dentro. Sobram **384 MiB**, ou 9,4%. O dono levantou que o
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
