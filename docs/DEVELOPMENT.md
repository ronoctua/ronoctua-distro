# Desenvolver

Para trabalhar no sistema sem instalá-lo: o compositor sobe numa janela do seu computador, com o
rodapé embaixo dela. Para o boot inteiro, há uma VM.

## Antes de começar

**Nix com flakes.** Confira com `nix --version`; se der "command not found", instale por
<https://nixos.org/download>. Depois:

```
mkdir -p ~/.config/nix
echo 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```

**Nada além disso.** As fontes (4,1 GB) não estão no git — nem como LFS, cuja cota gratuita é quatro
vezes menor que elas. São saída de build e moram como assets de um Release deste repositório, que o
flake busca por hash na primeira construção (~1,4 GB comprimidos, uma vez só).

```
git clone https://github.com/ronoctua/ronoctua-distro.git
cd ronoctua-distro
```

Todo comando abaixo roda **na raiz do clone**. `nix develop` entra no ambiente e imprime os comandos
disponíveis — essa lista sai do próprio flake e vale mais do que qualquer texto aqui.

> **Sobre o `direnv`.** Ele entra no ambiente sozinho ao entrar na pasta. É um programa do **seu
> sistema**, não do flake — você precisaria dele *antes* de poder entrar no ambiente. Se quiser:
> instale pelo gerenciador da sua distro, ponha `eval "$(direnv hook zsh)"` no `.zshrc` e rode
> `direnv allow`. O `.envrc` busca o `nix-direnv` sozinho.

## Os dois comandos do dia a dia

Prontos para copiar:

```
nix run .#dev
```

```
nix run .#vm -- --pointers --keyboards
```

**Nenhum dos dois precisa de `pkill` antes.** Cada um encerra sozinho o que uma execução anterior
deixou de pé — e só isso. Ver [O que fica rodando](#o-que-fica-rodando).

### `.#dev` — o compositor numa janela

Abre uma janela com o compositor dentro e o rodapé embaixo. A leader aqui é **`Ctrl+Alt+Espaço`**
(não `Super`: o `Super` é do compositor de fora, e nunca chega). Um toque abre a lista de atalhos, e
tudo o mais se descobre por ela; as teclas de emergência são impressas no terminal ao subir.

**Editar recarrega sozinho.** Um `.rs` em `crates/` recompila e reinicia o rodapé, com o compositor
de pé. Um `.rs` em `buffers/` reconstrói **aquele componente** e reinicia o rodapé apontando para ele —
um buffer é um componente wasm, e quem constrói componente é o flake. O laço imprime
`hot reload ligado` na partida; se não imprimir, você não está na raiz do projeto.

> **Arquivo novo precisa de `git add -N`.** O flake enxerga a árvore do **git**: um arquivo nunca
> adicionado é invisível para ele, a construção passa e a sua edição simplesmente não está lá. O laço
> percebe e diz isso em vez de anunciar sucesso.

**O laço é uma SEGUNDA cópia do sistema, com nome próprio.** Ele sobe com
`RONOCTUA_INSTANCE=dev`, e tudo o que é nosso ganha esse nome:

| | o sistema instalado | o laço |
|---|---|---|
| ajustes | `~/.config/ronoctua` | `~/.config/ronoctua-dev` |
| estado | `~/.local/state/ronoctua` | `~/.local/state/ronoctua-dev` |
| dados e cache | `…/share/ronoctua`, `~/.cache/ronoctua` | `…-dev` |
| fila e cérebro | `$XDG_RUNTIME_DIR/ronoctua-transfers.sock`, `…-brain.sock` | `ronoctua-dev-transfers.sock`, `ronoctua-dev-brain.sock` |

É isso que deixa rodar o laço **dentro da própria distro instalada** sem um mexer no outro: o
compositor de verdade continua lendo os seus arquivos, e a sessão de verdade continua com a sua fila
e o seu cérebro. **O cérebro do laço só ENSAIA** — senão seriam dois agendadores reconstruindo o
sistema, cada um no seu relógio. Na primeira vez, o laço copia os seus ajustes para
`~/.config/ronoctua-dev`, para começar parecido com a sua mesa; depois disso as duas cópias seguem
separadas. Para começar do zero:

```
rm -rf ~/.config/ronoctua-dev ~/.local/state/ronoctua-dev
```

O que **não** é nosso continua compartilhado de propósito: a Lixeira, o cache de miniaturas do
freedesktop, os apps instalados, o histórico do shell. São os seus arquivos, e uma cópia do sistema
mostra os arquivos da mesma pessoa. O mesmo vale para a máquina: rede, Bluetooth, som, brilho e
teclados são os de verdade, no laço como na sessão.

**O laço não dirige o quiosque do Chrome.** O Chrome é o da sessão de verdade, e o host que ele
sobe (`ronoctua-browser`) conecta no rodapé de verdade: o `RONOCTUA_INSTANCE` do laço não chega ao
Chrome. A prova do quiosque é o `kiosk-vm`, com o Chrome real, offline; para olhar a modeline dele,
veja [Olhar antes de dizer "pronto"](#olhar-antes-de-dizer-pronto).

**Testar em outra tela.** O grupo de desenvolvimento da leader tem um redimensionador: duas
cortinas cortam a tela pela direita e por baixo, e o sistema inteiro passa a caber no que sobra.
Arraste a borda ou o canto, ou clique numa cortina e use as teclas que a linha dela ensina; ela diz
o tamanho em pixels lógicos e em colunas × linhas. Ele continua de pé entre um hot reload e outro,
e funciona igual no `.#vm` e na máquina instalada. Janela em tela cheia ignora as cortinas. Para
medir uma coluna num tamanho e num espaço, ela abre pela linha de comando:
`ronoctua-shell column [player] [--on <espaço>] [--size large|medium|small|bar]`; outra do mesmo
inquilino encerra a anterior.

### `.#vm` — a distro inteira, com boot de verdade

Initrd, impermanência, unidades do systemd, o instalado como ele é. Usuário `noctua`, senha
`noctua`. É mais lenta que o `.#dev`: o vídeo é por software.

- **`--pointers`** entrega cada mouse e touchpad do seu computador à VM como ele mesmo, cada um com
  o seu cursor — sem isso o QEMU junta todos num aparelho só. O **touchpad embutido fica com você**:
  é ele que leva o ponteiro para fora da janela da VM. Lá dentro, ele move o ponteiro do próprio
  QEMU, que nasce desligado quando há uma mão de verdade (há uma linha em `LEADER s i` para ligá-lo).
- **`--keyboards`** faz o mesmo com os teclados **externos**. O embutido fica com você, e toda chave
  de segurança (YubiKey e afins) também — a VM nunca a toma.
- Os dois pedem `sudo` uma vez por aparelho, para ler o nó dele. Enquanto a VM roda, o que ela tomou
  não responde aqui; volta quando ela fecha.

**A VM lembra.** O disco é `ronoctua-vm.qcow2`, na pasta de onde você a chamou, e ele sobrevive a
desligar e reiniciar — dispositivos lembrados, cursores numerados, ajustes. Para uma VM do zero:

```
rm -f ronoctua-vm.qcow2 && nix run .#vm -- --pointers --keyboards
```

## O que fica rodando

Um `Ctrl+C` no terminal do `.#dev` encerra tudo o que ele subiu: compositor, rodapé, fila, cérebro.
O que escapa é o que foi morto **sem** poder limpar — terminal fechado, `kill -9`. Por isso:

- **O `.#dev` limpa na partida.** Todo processo que o laço sobe carrega no ambiente
  `RONOCTUA_INSTANCE=dev` e o PID da sessão que o subiu. Ao começar, ele encerra os que têm a marca
  e cuja sessão já não existe — nunca pelo **nome**. Um `pkill 'ronoctua-'` pegaria o rodapé, o
  cérebro e a fila da distro instalada, que têm os mesmos nomes; a marca não. Um segundo `.#dev`
  aberto em outro terminal está vivo, e é deixado em paz.
- **O `.#vm` encerra a VM que já estava no mesmo disco.** Duas não dividem um disco, e uma esquecida
  segura os seus mouses e teclados. A escolha é pelo caminho do disco na linha de comando do QEMU,
  então nenhuma outra VM é tocada.

Para ver o que sobrou de algum laço, sem matar nada:

```
grep -l 'RONOCTUA_INSTANCE=dev' /proc/[0-9]*/environ 2>/dev/null | cut -d/ -f3 | xargs -r ps -o pid,etime,args -p
```

## Verificar

| | quanto | quando |
|---|---|---|
| `nix run .#quick` | ~1 min | no meio de uma edição |
| `nix run .#most` | ~16 min | antes de commitar |
| `nix run .#gate` | horas | antes de publicar — **só ele fecha o invariante 7** |

O **`quick`** são dez nomes escolhidos em `tests/quick.nix` — formatação, clippy, testes e tabelas que
o compilador não confere. É um **subconjunto**, não uma categoria, e ele diz isso ao terminar.

O **`most`** é todo check menos os de `tests/slow.nix`, que **constroem uma instalação** (um sistema
NixOS, uma imagem de disco, o ISO) ou são marcos. As VMs comuns ficam: cada roteiro leva de 17 a 48 s.
Ao terminar, ele **lista o que não verificou**, pelo nome. `test-tiers-are-total` exige que a divisão
não envelheça, e um check novo entra no `most` sozinho.

O **`gate`** roda o `nix flake check` depois de **medir a memória livre**: ele limita quantas
compilações correm juntas e constrói as três pesadas uma de cada vez. Sem isso, numa máquina com
`earlyoom`, o `rustc` morre com `signal: 15` **sem diagnóstico nenhum** — parece defeito de compilação
e é falta de RAM. Se acontecer mesmo assim, ele diz quem o `earlyoom` matou.

As duas baterias andam em fases: **0** avalia o `.drv` de cada alvo em lotes, antes de construir
qualquer coisa (um teste de VM avaliado custa ~260 MiB, e todos juntos passam de 8 GiB); **1** as
quatro pesadas, uma de cada vez; **2** o que as VMs bootam; **3** as VMs, sozinhas, tantas ao mesmo
tempo quanto cabe na **memória que declaram** (`virtualisation.memorySize`, somada por nó) — cada
uma aparece com `->` ao começar e `<-` ao acabar; **4** o resto. O gate termina no
`nix flake check`.

> **`nix flake check` sem `-L` não mostra log** — silêncio não é sintoma.

### Olhar antes de dizer "pronto"

```
nix run .#png -- "$RONOCTUA_BUFFER_KEYBOARD" --out teclados.png
```

Um buffer vira PNG em 0,2 s, sem VM. Dentro do `nix develop`, cada componente tem a sua variável
(`RONOCTUA_BUFFER_BRAIN`, `…_EXAMPLE`, `…_FORMAT` e os outros); `--help` lista as opções — tamanho,
tema, idioma, e teclas e cliques para andar até a tela que se quer.

```
nix run .#png -- --door column --size small --state look.json --frame 1=quadro.png \
  --output eDP-1 --output 'DP-2=Um monitor' --language pt-BR --key '<down>' --out coluna.png
```

Com `--door`, a foto é de uma porta nativa — hoje a janela e a coluna do player, em todo tamanho,
barra inclusive, a janela das preferências do rodapé (`--door footer-settings`), a da atualização
(`--door update`, sobre os relatórios das duas metades e o que os diários disseram, `--report` e
`--journal`, nunca os da máquina; o `u` dela não começa nada, diz `would have …`), a da central de
segurança (`--door security`, sobre uma cena inteira em `--state`), a do gerenciador de arquivos
(`--door files`, que lê o resto da linha com as opções dela: `photograph --door files --help`), a
do app de ícones (`--door icons`, idem: `--screen`, `--type`, `--key`), a do navegador git
(`--door git --dir <pasta>`, sobre um repositório de verdade), a da calculadora (`--door calc`,
sobre um histórico vazio dela, que teclas e cliques enchem), a do video-metrics
(`--door video-metrics --catalog <arquivo>`, sobre uma cópia do catálogo dado), a dos containers
(`--door containers`, com um podman de mentira que responde o que foi gravado), a dos pertences
(`--door belongings --places <pasta>`, sobre uma cópia da máquina plantada nela), e quatro sobre uma
cena dada inteira (`scene_photograph.rs`): a das telas (`--door display --screens
one|two|stacked|dark`), a do clipboard (`--door clipboard --entry <texto> --image <png>`), a cortina
de baixo do redimensionador (`--door curtain --screen <l>x<a> --room <l>x<a>`) e a linha de vida
(`--door lifeline --result <palavra> [--asking]`) —, e o próprio rodapé (`--door footer`,
`footer_photograph.rs`: modeline, minibuffer e região sobre um compositor substituto com um monitor;
`--app`, `--state`, `--tone`, `--states` e `--local` descrevem a janela em foco, `--screens` e
`--entry` dão as telas e o histórico, e os passos são `--leader`, `--key` e `--command`), tirada por
`ronoctua-shell photograph`; toda porta que lê o resto da linha diz as opções dela em
`photograph --door <porta> --help`. A porta é a de produção, desligada, e o quadro sai da mesma
função com que ela se desenha na tela (`door::compose`); a comparação pixel a
pixel com a porta numa tela ainda está no `docs/PENDING.md`. O player dela não age fora da porta: o
que um clique faria sai impresso (`asked the player: …`, `would have …`), e a resposta do daemon é o
`--state` seguinte, que vale na ordem em que aparece, como as teclas e os cliques. O contorno da
fila é outra resposta e outro passo, o `--queue` (a resposta a `{"ask":"queue"}`): sem ele a fila da
foto é vazia. O `--frame <faixa>=<png>` é uma imagem parada no lugar dos quadros da imagem daquela
faixa (`every=` para toda imagem sem a sua), o `--output` descreve uma tela que a porta conhece pelo
nome — a primeira é a em que ela está, e o que é roteado a ela é o que ela mostra —, e o `--places`
é a pasta pessoal do seletor de arquivos, com os lugares dele nas pastas que há nela, no lugar das
da máquina. O `--sheet` é a folha de
um arquivo como o daemon a responde (`{"ask":"sheet"}`: as etiquetas, as capas, os fluxos, o que
pode ser gravado e por que não), o `--desk` é uma página de uma mesa do daemon (as buscas, os
diretórios de rádio e TV), o `--refused` é uma lista de diretório que o daemon recusou na hora
(`{"query":…,"trouble":…}`, como a resposta do `find` volta à porta que perguntou), e `--favourites`
e `--recent` são os favoritos e os tocados há pouco como o daemon os grava — nunca os da máquina.
Uma marca do globo é menor que uma célula: o `--list` diz onde cada grupo foi desenhado (`globe mark
[ids] at coluna,linha px x,y`), e `--hover-at`/`--click-at x,y` apontam para esse ponto, em pixels
lógicos. O que não caberia na foto é recusado com uma frase, nunca ignorado: uma imagem parada de
uma faixa que a porta não pôs ali também.

Um `look` se tira de um player que toca de verdade: numa cena do `.#shot`, `look("nome")` salva
`nome.json` ao lado das fotos; fora da VM, `python3 tests/player-client.py '{"ask":"look"}' >
look.json` pergunta ao player que estiver rodando, e `look` só lê. O contorno se tira do mesmo
jeito: `python3 tests/player-client.py '{"ask":"queue"}' > queue.json`, ou `player({"ask": "queue"})`
numa cena.

O `.#png -- --door` usa o workspace `release` do Nix, e editar o shell o reconstrói (minutos, e uns
8 GB de memória). Iterando, a câmera é o mesmo subcomando do binário do laço:
`cargo build --profile loop -p ronoctua-shell --bin ronoctua-shell` dentro do `nix develop`, e
`ronoctua-shell photograph …` com os mesmos argumentos. Sem GPU, aponte `VK_DRIVER_FILES` para o
lavapipe do Mesa do flake (`lvp_icd.x86_64.json`), como o app `.#png` faz. O `target/` do cargo é um
só para todo checkout: com duas árvores abertas, copie de lado o binário com que vai fotografar.

```
nix run .#shot -- <cena.py>        fotografa a feature do momento numa VM, pela cena escrita para ela
                                   (fora do repo); as fotos vão para ~/.cache/ronoctua-distro/shot
```

Teste verde não vê espaço em branco, sobreposição nem uma faixa no lugar errado. A foto vê.

O palco também tem o player: `start_player()` sobe o daemon (sem a tabela de decodificadores, que
custa 198 MiB; quem abre um formato dela roda `nix run .#shot -- --player-tools <cena.py>` e pede
`start_player(tools=True)`), `player({...})` pergunta ao socket, `frame_png(None, "nome")` salva a
primeira imagem que o daemon mostra, e `frame_png(<faixa>, "nome")` a de uma faixa, e `MEDIA` é a
pasta de arquivos que o `player-vm` prova que tocam (`tests/player-media.nix`). Ali estão também
`words.mkv` e `words.en.srt` (uma legenda de dentro e uma de ao lado do mesmo filme) e `list.xspf`
(uma playlist que diz o próprio nome). Suba o player antes de abrir a vista, senão o rodapé sobe um
por conta própria. Que a legenda do libass sai no quadro ainda não foi visto nesta VM
(`docs/PENDING.md`).

**A modeline do quiosque, sem Chrome.** A cena monta as duas metades do par à mão (`<N>` é o nonce,
`<W>` o id da janela no Chrome, e os arquivos de runtime levam o nome da instância,
`ronoctua[-dev]-…`):

1. `RONOCTUA_BROWSER_MANIFEST` aponta para um JSON com
   `{"allowed_origins":["chrome-extension://<32 letras de a a p>/"]}`;
2. uma janela com `app_id` `google-chrome`, em foco, com `<N>` no título, aberta ANTES do rodapé;
3. `$XDG_RUNTIME_DIR/ronoctua-kiosk.json` com
   `[{"nonce":"<N>","compositor":0,"chrome":<W>,"ui":"hidden"}]`;
4. um host falso que conecta em `$XDG_RUNTIME_DIR/ronoctua-browser.sock`, escreve uma linha só e
   mantém a conexão aberta (cada aba tem `id`, `index`, `active`, `title`, `url` e
   `zoom_percent`):

   ```
   {"said":"state","v":1,"kiosks":[{"window":<W>,"nonce":"<N>","state":"fullscreen","tabs":[…]}],"extensions":[…],"focused_window":<W>}
   ```

Com `"state":"normal"` o rodapé entende um F11 e mostra as barras do Chrome. `modeline.json`, no
diretório de ajustes, com `{"address_line":true}` mostra a linha de endereço; fechar o host mostra
a marca de sem conexão.

### O daemon do player sem VM, e sem tocar a sessão

O daemon de depuração (`cargo build -p ronoctua-player-engine`) roda num namespace próprio:
`unshare --map-root-user --net --mount`, com `XDG_RUNTIME_DIR`, `XDG_STATE_HOME`, `XDG_CONFIG_HOME` e
`HOME` em pastas temporárias, um `dbus-daemon --session` privado para o MPRIS, e `/dev/snd` e
`/run/tailscale` escondidos por montagens vazias (o som vai para a saída nula, e nenhuma tailnet de
verdade é alcançada). O cliente é o `tests/player-client.py`, apontado para o `XDG_RUNTIME_DIR` da
sala. Para as buscas e os diretórios, um servidor HTTPS falso responde por cada host: uma CA
descartável montada por cima do pacote de certificados do sistema e um `/etc/hosts` que manda os
nomes para 127.0.0.1, dentro da mesma sala. Nada disso fica no repositório — o roteiro é da tarefa,
como uma cena do `.#shot` —, e o `dbus-daemon` e o servidor que você subir, você encerra: o
namespace morre com o shell, eles não.

## Os outros comandos

```
nix run .#diagnostics  instantâneo de quando os atalhos travam (só leitura)
nix run .#bench        mede os alvos de desempenho; sai != 0 se algum estourar
nix build .#iso        o ISO instalável
nix run .#install      instala (ensaia sem --execute)
```

### Quando o ponteiro parece atrasado

```
RONOCTUA_MEASURE_HOVER=1 RUST_LOG=ronoctua_render=info nix run .#dev
```

Cada linha que o ponteiro cruza vira uma medida: do EVENTO até o `present`. É a nossa metade da
latência. Se ela vier em milissegundos de um dígito, o atraso que se vê está na cadeia — no laço há
um compositor a mais entre a mão e a tela. Medir a outra metade exige a distro instalada.

## Instalar de verdade

Boot, TPM, hibernação e desempenho gráfico não se provam em VM.

**O ISO carrega o que ninguém tem em cache** — o shell, o compositor pinado, o `libghostty-vt`, os
buffers wasm e as fontes vão no `isoImage.storeContents`. A máquina alvo não compila nada; o resto ela
baixa do `cache.nixos.org`.

**A instalação `minimal` tem 6,9 GiB contra 22,8 da `full`.** Ela deixa de fora os aplicativos, não a
distro — e um deles é uma linha depois:

```
ronoctua.browser.enable = true;    # em /etc/nixos/host.nix, depois nixos-rebuild switch
```

Depois da primeira instalação, o laço deixa de passar pelo ISO: edite, e rode
`nixos-rebuild switch --flake /etc/nixos#noctua` na própria máquina. E o `.#dev` continua servindo
ali dentro, com o nome próprio que ele tem (ver [`.#dev`](#dev--o-compositor-numa-janela)).

## Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| `hash mismatch` num input de fonte | o asset foi reenviado com outro conteúdo | `nix flake update fonts-ttf` (e os outros três), e confira por que ele mudou |
| `404` num input de fonte | o Release não está publicado, ou o repositório está privado | asset de Release herda a visibilidade do repositório; não há token que contorne |
| `nix run .#dev` diz que não há sessão gráfica | você está num TTY | use `nix run .#vm` |
| o rodapé não aparece, mas o compositor sobe | ele recusou a layer surface | `RUST_LOG=ronoctua_shell=debug nix run .#dev` mostra o motivo |
| nenhum atalho responde | tecla que não chega, modificador preso, ou foco não concedido — três causas com a mesma tela | `nix run .#diagnostics` separa as três; e confira se o **seu** ambiente não captura `Ctrl+Alt+Espaço` antes |
| a VM não abre dizendo que o disco está em uso | uma VM de outro disco com o mesmo nome, ou de outro usuário | a do mesmo disco o `.#vm` já encerra; confira com `ps -ef \| grep qemu` |
| a modeline não mostra porcentagem ao colar | a fila de transferências não subiu | fora do NixOS ela é iniciada pelo próprio rodapé; `RUST_LOG=ronoctua_transfers=info` diz o porquê |
| um link some da fila com `o aria2 nao esta atendendo` | o aria2 é uma **unidade**, e o laço não sobe unidade nenhuma | esperado fora de uma sessão de verdade; para exercitar o motor, use `nix run .#vm` |
| um vídeo falha com `esta instalacao nao tem o yt-dlp` | o caminho vem da unidade, não do binário | no laço, `export RONOCTUA_YT_DLP=$(which yt-dlp)` antes de subir |
| um vídeo com restrição de idade falha, mesmo com `t i v` | os cookies são decisão da MÁQUINA | `export RONOCTUA_COOKIES_BROWSER=chrome+basictext` antes de subir, ou `RONOCTUA_COOKIES_FILE`; no sistema instalado é `ronoctua.transfers.cookies` |
| `Login with OAuth is no longer supported` | o yt-dlp recusa OAuth | cookies é o caminho, e o PO token (`$XDG_STATE_HOME/ronoctua/po-token`) faz o papel de "dispositivo real" |
| o yt-dlp diz que precisa de PO token | o gerador está desligado nesta instalação (sem `ronoctua.media`), ou falta uma das três variáveis | `ronoctua.transfers.poToken = "on-demand"`; o `.#dev` já exporta `RONOCTUA_POT_HOME`, `RONOCTUA_POT_PLUGINS` e `RONOCTUA_DENO` — as três, ou o plugin não é carregado |
| o navegador se desloga depois de um download identificado | o YouTube gira os cookies | `ronoctua.transfers.cookies = "file"` com um jar exportado de uma janela anônima que você fechou SEM sair da conta |
| um magnet não abre nada | o `.desktop` é do sistema instalado | à mão, `ronoctua-transfers open '<magnet>'` |
| a extensão não aparece no Chrome | a política é escrita por unidade de SISTEMA, no boot | `systemctl status ronoctua-browser-extension`; feche todas as janelas do Chrome antes de conferir |

**A sessão de torrent vive dentro da fila.** Ela sobe no primeiro magnet e escuta em 6881-6889; o
estado fica em `$XDG_DATA_HOME/ronoctua/torrent` (no laço, `ronoctua-dev/torrent`). Um `.torrent`
puramente v2 para experimentar sai de `ronoctua-test-torrent <diretório>`, que cria, verifica e não
toca a rede.
