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
de pé. Um `.ftl` de um crate (`crates/<crate>/i18n/`) refaz os textos e as chaves deste checkout
(`ronoctua-keys`) e reinicia o rodapé: uma frase editada não compila nada (a primeira da sessão
compila o rodapé com o `cargo` uma vez, porque o do store é o embrulho que fixa os textos do build),
e uma chave nova recompila só os crates que a dizem. Um `.rs` em `buffers/` reconstrói **aquele
componente** e reinicia o rodapé apontando para ele — um buffer é um componente wasm, e quem
constrói componente é o flake. O laço imprime `hot reload ligado` na partida; se não imprimir, você
não está na raiz do projeto.

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

O **`quick`** são os nomes escolhidos em `tests/quick.nix` — formatação, clippy, testes, o plano do
cargo, as leituras declaradas, a granularidade de uma edição e tabelas que o compilador não confere
—, construídos pelo `ronoctua-sized-build`. É um **subconjunto**, não uma categoria, e ele diz isso
ao terminar.

O **`most`** é todo check menos os de `tests/slow.nix`, que **constroem uma instalação** (um sistema
NixOS, uma imagem de disco, o ISO) ou são marcos. As VMs comuns ficam: cada roteiro leva de 17 a 48 s.
Ao terminar, ele **lista o que não verificou**, pelo nome. `test-tiers-are-total` exige que a divisão
não envelheça, e um check novo entra no `most` sozinho.

O **`gate`** roda o `nix flake check` depois de **medir a memória livre**: ele limita quantas
compilações correm juntas (`nix/build-sizing.nix`, a mesma conta do `ronoctua-machine apply`):
cada unidade Rust começa quando a fatia da classe dela (medida, `nix/rust/unit-classes.json`) cabe
ao lado do que roda, e as pesadas vão uma de cada vez. Sem
isso, numa máquina com `earlyoom`, o `rustc` morre com `signal: 15` **sem diagnóstico nenhum** —
parece defeito de compilação e é falta de RAM. Se acontecer mesmo assim, ele diz quem foi morto e
por quem — o `earlyoom`, o `systemd-oomd` ou o OOM killer do kernel. Numa máquina da distro ele
também não passa do teto do `nix-daemon` (ver [O que a máquina precisa
ter](#o-que-a-máquina-precisa-ter)).

As duas baterias andam em fases: **0** avalia o `.drv` de cada alvo em lotes, antes de construir
qualquer coisa (um teste de VM avaliado custa ~260 MiB, e todos juntos passam de 8 GiB); **1** o
build Rust: o que as unidades precisam e não é unidade, primeiro; cada unidade que o dry run lista
é um `nix build` próprio, por classe (as linhas `->` e `<-` dizem quando cada uma começou e acabou,
e quanto do orçamento ia junto), as execuções de teste depois de toda compilação, os agregados por
último — uma unidade que falha é construída uma vez, e o que precisa dela fica de fora até o fim,
pelo nome; **2** o que as VMs bootam;
**3** as VMs, sozinhas, tantas ao mesmo tempo quanto cabe na **memória que declaram**
(`virtualisation.memorySize`, somada por nó) — cada uma aparece com `->` ao começar e `<-` ao
acabar; **4** o resto. O gate termina no `nix flake check`.

> **`nix flake check` sem `-L` não mostra log** — silêncio não é sintoma.

### Olhar antes de dizer "pronto"

```
nix run .#png -- "$RONOCTUA_BUFFER_KEYBOARD" --out teclados.png
```

Um buffer vira PNG em 0,2 s, sem VM. Dentro do `nix develop`, cada componente tem a sua variável
(`RONOCTUA_BUFFER_BRAIN`, `…_EXAMPLE`, `…_FORMAT` e os outros); `--help` lista as opções — tamanho,
tema, idioma, e teclas e cliques para andar até a tela que se quer. O painel de memória desenha a
lista que a varredura do host (`ronoctua_memory::scan::ram`) tira de um `/proc` plantado numa pasta
(`--proc <pasta>`: `<pid>/statm`, `comm`, o link `exe` e `cmdline`), nunca a desta máquina.

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
(`--door belongings --places <pasta>`, sobre uma cópia da máquina plantada nela), a do pendrive
(`--door stick --scene <arquivo>`), a central de programas (`--door software --room <pasta>`), a do
Incus (`--door incus --scene <arquivo>`), a das contas LLM (`--door agents --places <pasta>`), a da
atualização sobre uma cena inteira (`--door update-scene --scene <arquivo>`), e quatro sobre uma
cena dada inteira (cada câmera na tabela `CAMERAS` do crate da porta): a das telas
(`--door display --screens one|two|stacked|dark`), a do clipboard
(`--door clipboard --entry <texto> --image <png>`), a cortina de baixo do redimensionador
(`--door curtain --screen <l>x<a> --room <l>x<a>`) e a linha de vida
(`--door lifeline --result <palavra> [--asking]`) —, e o próprio rodapé (`--door footer`,
`footer_photograph.rs`: modeline, minibuffer e região sobre um compositor substituto com um monitor;
`--app`, `--state`, `--tone`, `--states` e `--local` descrevem a janela em foco, `--screens` e
`--entry` dão as telas e o histórico, `--notification` põe um aviso na faixa, e os passos são
`--leader`, `--key` e `--command`), tirada por
`ronoctua-shell photograph`; toda porta que lê o resto da linha diz as opções dela em
`photograph --door <porta> --help`. No rodapé, as vistas do Incus, da segurança e do pendrive leem a
cena que uma variável nomeia, no formato da câmera da janela delas, nunca a desta máquina
(`RONOCTUA_PHOTOGRAPH_INCUS_SCENE`, `RONOCTUA_PHOTOGRAPH_SECURITY_SCENE` e
`RONOCTUA_PHOTOGRAPH_STICK_SCENE`). A porta é a de produção, desligada, e o quadro sai da mesma
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

O `.#png` roda os programas de LOOP das unidades (`rust.loop.bins.ronoctua-shell`,
`…ronoctua-render-png`): a primeira foto depois de uma edição numa vista custa a lib dela e o link
de loop do programa, não o link LTO de release. Iterando, a câmera é o mesmo subcomando do binário
do laço: `cargo build --profile loop -p ronoctua-shell` dentro do `nix develop`, e
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
nix run .#fetch-iso    baixa o ISO da versão publicada, confere e junta as partes
nix run .#install      instala (ensaia sem --execute)
nix run .#apply-here   a máquina instalada com este checkout no lugar do input
```

### Quando o ponteiro parece atrasado

```
RONOCTUA_MEASURE_HOVER=1 RUST_LOG=ronoctua_render=info nix run .#dev
```

Cada linha que o ponteiro cruza vira uma medida: do EVENTO até o `present`. É a nossa metade da
latência. Se ela vier em milissegundos de um dígito, o atraso que se vê está na cadeia — no laço há
um compositor a mais entre a mão e a tela. Medir a outra metade exige a distro instalada.

## O build em unidades

O `Cargo.toml` é a verdade: o `cargo` local (devshell, `.#dev`, `cargo test -p`) continua igual, e o
Nix só o lê. O que o Nix constrói é o plano que o cargo faz da árvore (`nix/rust/plan.json`), uma
derivation por unidade — o porquê está no `docs/ARCHITECTURE.md`, seção 10.

```
nix run .#rust-plan      regenera nix/rust/plan.json (dependência, feature, perfil ou alvo mudou)
nix run .#rebuild-cost   o que a árvore reconstrói contra o HEAD, e quanto, sem construir
nix run .#sized-build -- <instalável>…
                         nix build dimensionado: cada unidade pela sua classe, as pesadas sozinhas,
                         as execuções de teste depois de toda compilação
nix run .#rust-peaks     cada unidade contra a sua classe; falha se uma passou da fatia dela
nix run .#rust-peaks -- --write
                         regrava nix/rust/unit-classes.json com as classes que o store mediu
nix run .#profile-bench  mede cada variante do perfil de release no .#bench (horas; sessão gráfica)
```

- **Um manifesto mudado sem o plano** deixa toda unidade sem derivation: a avaliação recusa e diz
  `nix run .#rust-plan`. O `git diff nix/rust/plan.json` diz quais unidades mudaram.
- **O que uma edição custa, antes de construir**: `nix run .#rebuild-cost` (a árvore contra o
  `HEAD`; `-- --against <rev>`, ou `-- crates/x/src/a.rs` para uma edição de um byte simulada sem
  tocar o arquivo; `--json` para script). Uma entrada nunca medida é dita, nunca contada como
  grátis. O índice inteiro: `nix eval --json .#legacyPackages.x86_64-linux.rustUnits`.
- **Uma unidade**: `nix run .#sized-build -- .#legacyPackages.x86_64-linux.rust.bins.<programa>
  --no-link` (o programa de release; `rust.<nome>` é o que o módulo roda, com o embrulho de quem
  pinta; `rust.loop.bins.<programa>` o de loop; `rust.shell`/`rust.loopShell` o bundle do rodapé), e
  `rust.test.testRuns.<crate>`, `rust.test.clippy.<crate>`, `rust.release.deps."<chave>"` (um
  terceiro). O `metrics` diz quanto custou: `seconds`, `peak-kib` (o pico do build inteiro, da memória
  que o orçamento paga: anônima, shmem, swap e tabelas de página) e `cores` (os núcleos com que foi
  construída); o comando de um terceiro
  está no `nix-support/rustc.json` dele. Os agregados são `rust.tests`, `rust.clippy` e `rust.fmt`
  (os checks `rust-tests`, `rust-clippy`, `rust-fmt`).
- **Um programa que saiu do PATH** (os `driver` de teste, as ferramentas de check) roda pelo flake:
  `nix run .#rust.bins.ronoctua-test-faces`, `nix run .#rust.index -- query …`.
- **Um teste só, sem recompilar**, num check: `rust.testsOf { name; crate; exact; expect; … }`,
  sobre o harness que as unidades já construíram.
- **Quanto cabe nesta máquina agora**: `ronoctua-build-sizing --explain` (está no fecho do
  `ronoctua-machine`).
- **Uma unidade que mudou de tamanho** (o `.#rust-peaks` ou o check `unit-classes-fit-the-measures`
  a nomeia): depois de um build dela pelo `.#sized-build` (a um núcleo; a medida de um `nix build`
  cru tem os núcleos do `nix.conf` e não classifica: a unidade fica na classe que tinha até o build
  dimensionado seguinte dela, e o check a diz), `nix run .#rust-peaks -- --write` regrava a classe de
  toda unidade medida no store e mantém a das outras; commite o `nix/rust/unit-classes.json` com o
  resto.
  Depois de um `Cargo.lock` novo o mesmo comando, sem build, tira os ids que o lock renomeou e dá a
  cada renomeada a classe da que ela substituiu (o check `unit-classes-name-units-of-the-index`
  cobra).
- **As unidades guardadas**: num checkout,
  `~/.local/state/ronoctua/gcroots/units-<release|loop>-<hash do checkout>@<carimbo>/`; numa
  máquina, `/var/lib/ronoctua-machine/kept-units/release@<carimbo>/`. Nenhuma raiz é motivo para
  compilar: só o que o store tem é enraizado.
- **Um crate novo** declara o que lê em `[package.metadata.ronoctua]` (`env`, `test-env`,
  `env-absent`, `run-env`, `run-env-absent`, `beside`, `data`, `bins`): a unidade que lê um
  `RONOCTUA_*` não declarado falha dizendo o nome e a chave. Um programa que é ferramenta de teste
  declara `bins = { "<nome>" = "driver" }`.
- **Um check de granularidade que falha** diz a entrada e o porquê: "is built from it and was NOT
  rebuilt" é um fileset estreito demais; "was rebuilt, and nothing of it is built from that file" é
  um fileset, um `data` ou uma variável largos demais.

O shell são ~50 crates: `cargo test -p ronoctua-shell-<funcionalidade>` roda os testes de uma
funcionalidade sem compilar o rodapé (segundos); `cargo test -p ronoctua-shell-footer`, os do
rodapé; `cargo test -p ronoctua-shell`, os que perguntam a árvore inteira de teclas, as regiões
registradas e as tabelas de portas e câmeras. `ronoctua-shell check-keymap --dump` imprime o keymap
inteiro (os três conjuntos, os acordes, o registro, as ações do brain), uma linha por fato, para
comparar duas montagens. O clippy local de um crate sozinho (`cargo clippy -p <crate>
--all-targets`) vê o lado do produto dele; junto de quem o liga com `test-support` (`--workspace`),
não vê — o `rust.test.clippy.<crate>` do Nix vê os dois.

## Onde moram as frases, as cores e o que o brain desenha

Os NOMES compilam; os VALORES são dado lido em tempo de execução, e um programa recusa numa linha em
inglês que nomeia a variável o que não lê (os textos e o catálogo na partida, com código 78; a
paleta na primeira cor pedida, num pânico):

- **Uma frase** mora no crate que a diz: `crates/<crate>/i18n/<idioma>.ftl`, nos dois idiomas.
  Editar um valor não compila nada; no `.#dev` o laço refaz os textos e reinicia o rodapé, no
  devshell `ronoctua-keys` refaz os textos para onde o `RONOCTUA_TEXTS` dele aponta, e fora dos dois
  é `nix build .#texts` e `RONOCTUA_TEXTS=<saída>`.
- **Uma chave nova** vai ao fragmento do crate que a diz (dita por vários, ao crate mais próximo
  sobre o qual todos ficam) e então `ronoctua-keys`: o `cargo` recompila esse crate e o que o liga,
  sem reentrar no shell. `nix run .#texts -- missing pt-BR` diz o que falta traduzir, fragmento por
  fragmento; `nix run .#texts -- where <chave>` acha o fragmento de uma chave.
- **Um crate novo que fala**: `env = ["RONOCTUA_KEYS_DIR"]` no `[package.metadata.ronoctua]`, um
  `src/key.rs` (copie de outro), `pub mod key;` no `lib.rs` — fora de qualquer `#[cfg]` — e o
  `i18n/<idioma>.ftl`.
- **Uma cor**: mexa na semente (`assets/themes/<id>.nix`), `nix build .#themes --no-link
  --print-out-paths` (o `report.md` está lá) e `RONOCTUA_THEMES=<saída>` com o binário do `cargo`,
  ou `nix run .#png` — nenhum crate recompila.
- **O que o rodapé desenha de um módulo do brain** (marcas e glifos, avisos, ações, eventos,
  notificações) é o catálogo: `nix build .#brain-catalogue`; depois de mexer num `manifest.nix`,
  reentre o ambiente (`direnv reload`). O catálogo é dado e não recompila nada, mas o manifesto de
  um módulo DA DISTRO também é compilado na biblioteca `ronoctua-brain`, que fica sob o shell
  inteiro: uma marca nova num módulo da distro recompila a biblioteca, a base do shell, o rodapé,
  todo crate de funcionalidade e quatro programas (quanto, diz
  `nix run .#rebuild-cost -- crates/ronoctua-brain/modules/<id>/manifest.nix`; ver
  `docs/PENDING.md`). O de um módulo da pessoa recompila só o daemon.
- `ronoctua-shell check-texts` (toda chave de todo crate que o binário liga e de todo módulo do
  brain da distro, em todo idioma) e `ronoctua-shell check-catalogue` dizem se um diretório de
  textos ou um catálogo serve para o binário; `ronoctua-brain modules --catalogue` imprime o que o
  daemon compilou, na forma do catálogo.
- Um `cargo test` fora do devshell precisa de `RONOCTUA_TEXTS`, `RONOCTUA_THEMES` e
  `RONOCTUA_BRAIN_CATALOGUE`: o primeiro teste que fala, pinta ou desenha uma marca para nomeando a
  variável que falta.

## Instalar de verdade

Boot, TPM, hibernação e desempenho gráfico não se provam em VM.

**O ISO carrega o que ninguém tem em cache** — o shell, o compositor pinado, o `libghostty-vt`, os
buffers wasm e as fontes vão no `isoImage.storeContents`, sempre: a distro não tem cache binário. A
máquina alvo não compila nada; o resto ela baixa do `cache.nixos.org` — e do cache binário da pessoa,
quando os pertences dela nomeiam um.

**A instalação `minimal` tem ~12,5 GiB contra ~37 da `full` (~35,5 com o padrão do instalador).** Ela
deixa de fora os aplicativos, não a distro nem o trabalho nela — o toolchain do devshell, o Common Lisp
e os agentes estão nas duas — e um deles é uma linha depois:

```
ronoctua.browser.enable = true;    # no host.nix da máquina, depois sudo systemctl start ronoctua-rescue-rebuild.service
```

Depois da primeira instalação, o laço deixa de passar pelo ISO. O `/etc/nixos` é o flake da
máquina, e o código da distro não está lá: ele vem do input `ronoctua`, então editar ali não muda
nada. No checkout, `nix run .#apply-here` constrói a máquina com o checkout no lugar do input e troca
para ela, sem gravar o lock (ver [Publicar uma versão](#publicar-uma-versão)). E o `.#dev` continua
servindo ali dentro, com o nome próprio que ele tem (ver [`.#dev`](#dev--o-compositor-numa-janela)).

<!-- anchor: L01 -->

**O age pós-quântico se confere sem VM.** `cargo test -p ronoctua-pq` responde aos vetores dos três
textos que definem o `mlkem768x25519` (o X-Wing, o HPKE e o spec do age); `cargo run -p ronoctua-pq
--bin ronoctua-test-pq -- vectors` os lista um a um. O cruzamento com o `age` do Go, nos dois
sentidos, é o `pq-age-interop`, que roda os programas de `release`.

<!-- anchor: L15 -->

## O que a máquina precisa ter

Numa máquina instalada com a distro, nos dois perfis, tudo abaixo já vem pronto: o toolchain do
devshell está no store, saída por saída e com o `stdenv` (o `nix develop` não baixa toolchain
nenhum), o rust, o `cc` com que ele linka e os agentes de código estão no PATH, a pessoa está no grupo
`kvm`, o daemon oferece as quatro `system-features` e o direnv do sistema já
carrega o `nix-direnv`. **`LEADER / c`** clona este repositório numa pasta e a abre — a pergunta nomeia
o host para onde as suas configurações do git mandam o endereço (e, se não é o dele, o dele ao lado), e
respondê-la é deixá-lo ser consultado; um espelho privado que pede credenciais é recusado (o clone não
roda o seu auxiliar de credenciais), e o SSH é o seu — um `Host` com outro `HostName` no
`~/.ssh/config` (o `ssh.github.com` que o GitHub sugere) é perguntado pelo nome que o git usa. O que o
checkout gera (os ganchos, o WIT, os
buffers, as tabelas `-rs`) e os programas que o build assa seguem o commit: o primeiro `nix develop`
os constrói, e o que essas construções pedem de fora (as crates de um buffer) ainda vem da rede. Em
outro sistema, confira um a um:

| | o quê | por quê |
|---|---|---|
| memória | 14 GiB na máquina de referência | o link de release do programa do shell chega a ~3,9 GiB a doze núcleos (mais ~0,3 GiB por núcleo); o dimensionamento mede o que está livre e dá às pesadas os núcleos que cabem, um só abaixo de ~7,2 GiB livres numa máquina de 14 GiB (`ronoctua-build-sizing --explain` diz esta) |
| disco | dezenas de GB livres | o diretório de build do cargo (`nix/cargo-dirs.nix`, fora da árvore) passa de 70 GB com os perfis `loop` e `release`; o store cresce a cada bateria |
| KVM | `/dev/kvm` legível e gravável por você | os testes de VM e o `.#vm` sem aceleração emulam cada instrução |
| `system-features` | `kvm nixos-test big-parallel benchmark` no `nix.conf` do daemon | um teste de VM pede `kvm` e `nixos-test`; sem eles o Nix recusa construí-lo |
| `trusted-users` | o seu usuário, como na distro (`nix/modules/base.nix`) | de quem não é confiável o daemon ignora, com um aviso, os ajustes restritos de um `--option` (os substituters, o sandbox), e recusa um caminho sem assinatura trazido de outra máquina (`nix copy`) |
| rede | na primeira avaliação | os inputs do flake e as fontes (~1,4 GB comprimidos, assets do Release `fonts-v1`); depois, só o que muda |

**Uma máquina que só constrói** pode dar mais memória ao daemon: na distro, o `nix-daemon` tem
`MemoryHigh` em `ronoctua.memory.buildCeiling` (metade da RAM, por padrão), e o gate nunca planeja
além dele. Num servidor de build, `ronoctua.memory.buildCeiling = "85%";` no `host.nix` sobe o teto;
o `systemd-oomd` continua vigiando a `system.slice` como antes.

**O superpacote de ícones não é mais entrada do devshell.** Na `full` ele já está no store; na
`minimal` o devshell não o exporta e imprime a linha que o traz (`nix build .#noctua-icon-theme
--out-link ~/.cache/ronoctua-distro/icons`, 0,61 GiB). O `.#dev`, que é onde os ícones aparecem,
continua com ele como entrada: rodar o laço o traz, em qualquer máquina.


<!-- anchor: L16 -->

## Publicar uma versão

As máquinas seguem a branch `release`, e só o `nix run .#release` a move — para frente, depois do
gate. Ele roda na `main` limpa e igual à `origin/main`, e recusa antes de qualquer custo o que não
estiver assim (o login do `gh` também).

```
nix run .#release -- --dry-run          confere tudo que é de graça e diz o que faria
nix run .#release                       gate, sistemas, ISO (com a semente), rascunho, release, publicação
nix run .#release -- --sign             e as checksums numa tag assinada (pede a chave)
nix run .#release -- --rollback-to <rev>    a release volta a uma versão publicada antes, que volta a ser a mais recente
```

A distro não tem cache binário: nada é enviado a cache nenhum. O trabalho fica em
`~/.cache/ronoctua-distro/release/<tag>`; rodar de novo depois de uma falha retoma: o rascunho do
GitHub recebe os assets de novo. Ele nunca faz commit.

Para ver o checkout na própria máquina sem publicar, `nix run .#apply-here` (`-- --dry-run` só mede):
constrói o flake da máquina com o checkout no lugar do input e troca para ele, sem tocar o lock; a
próxima atualização volta à `release`. A troca recusa enquanto a atualização roda, enquanto um ensaio
de boot espera ser julgado e quando a geração nova tiraria do menu de boot a última que deu certo.

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
puramente v2 para experimentar sai de `nix run .#rust.bins.ronoctua-test-torrent -- <diretório>`,
que cria, verifica e não toca a rede (é um programa `driver`: nenhuma máquina o leva no PATH).
