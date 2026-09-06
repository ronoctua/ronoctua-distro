# Rodar o ambiente de desenvolvimento

Para trabalhar no sistema sem instalá-lo — o compositor sobe numa janela do seu computador, e o
rodapé embaixo dela. Não precisa de VM.

## O que você precisa antes

**Nix com flakes habilitado.** Confira com `nix --version`; se der "command not found", instale por
<https://nixos.org/download>. Depois:

```
mkdir -p ~/.config/nix
echo 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```

**Nada além disso.** Os arquivos de fonte ocupam 4,1 GB e não estão no git — nem como LFS, cuja cota
gratuita é quatro vezes menor que eles. São saída de build, e vivem como assets de um Release deste
mesmo repositório, que o flake busca por hash.

## Passos

**1. Clonar.**

```
git clone https://github.com/ronoctua/ronoctua-distro.git
cd ronoctua-distro
```

Os arquivos de fonte vêm sozinhos: são input do flake, buscados do Release por hash na primeira
construção. São ~1,4 GB comprimidos, demoram, e é uma vez só.

**2. Entrar no ambiente.**

```
nix develop
```

Basta isto. A primeira vez compila bastante; depois é instantâneo. Ao entrar, ele imprime os comandos
disponíveis — essa lista é mais confiável do que qualquer coisa escrita aqui, porque sai do próprio
flake.

> **Sobre o `direnv`.** Ele entra no ambiente sozinho ao entrar na pasta, poupando digitar
> `nix develop`. Mas é um programa do **seu sistema**, não do flake — e o projeto não tem como
> fornecê-lo, porque você precisaria dele *antes* de poder entrar no ambiente. Se quiser:
> `sudo pacman -S direnv` (ou o gerenciador da sua distro), `eval "$(direnv hook zsh)"` no seu
> `.zshrc`, e `direnv allow`. O `.envrc` busca o `nix-direnv` sozinho.

**3. Verificar que está tudo de pé.**

```
nix run .#gate
```

Sai sem reclamar? A fundação está correta. Ele demora bastante na primeira vez.

Ele roda o `nix flake check`, e faz duas coisas antes: **mede a memória livre** e limita quantas
compilações podem correr ao mesmo tempo, e constrói as três pesadas uma de cada vez. Sem isso, numa
máquina com `earlyoom`, o `rustc` é morto com `signal: 15` e **sem diagnóstico nenhum** — parece
defeito de compilação e é falta de RAM. Se acontecer mesmo assim, ele diz que foi o `earlyoom`, em
vez de deixar você adivinhando.

**4. Subir o desktop.**

```
nix run .#dev
```

Abre uma janela com o compositor rodando dentro dela e o rodapé embaixo.

Editar um `.rs` em `crates/` recompila e reinicia o rodapé sozinho, com o compositor de pé. Editar um
`.rs` em `buffers/` reconstrói **aquele componente** e reinicia o rodapé apontando para ele — são
caminhos diferentes porque um buffer é um componente wasm, e quem constrói componente é o flake. Ele
imprime `hot reload ligado` na partida — se não imprimir, você não está na raiz do projeto.

> **Arquivo novo precisa de `git add -N`.** O flake enxerga a árvore do **git**, e um arquivo que
> nunca foi adicionado é invisível para ele: a construção passa, sai idêntica, e a sua edição
> simplesmente não está lá. O laço percebe e diz isso em vez de anunciar sucesso.

## Mexer em arquivos sem sair do teclado

`LEADER .` abre o gerenciador na região do rodapé; `LEADER f w` abre a mesma coisa numa janela, sem o
teto de doze linhas. As duas desenham a mesma vista — o que muda é só o tamanho.

Dentro dele, a tecla que resolve tudo é a **vírgula**: ela abre o menu de ações, e cada linha mostra
a tecla que a roda. Não há uma segunda tabela para decorar, e isso é de propósito: o menu sai da
mesma tabela de onde as teclas são lidas, então não tem como divergir.

> **O botão direito abre o mesmo menu**, e passar o ponteiro sobre uma permissão explica aquele bit
> na linha de baixo — numa pasta o `x` é "entrar" e não "executar", e a frase muda junto. Nada disso
> flutua: é a mesma região, e o invariante 14 continua de pé.

Trocar dono, grupo ou permissão é **escolher numa lista**, nunca digitar uma sintaxe. Só o dono pede
senha, porque só ele precisa de root — e quem pergunta é o polkit, uma vez por sessão.

## A única tecla que você precisa saber

Um toque em **`Ctrl+Alt+Espaço`** abre a lista de atalhos, logo abaixo das duas linhas do rodapé. É
por ela que se descobre todo o resto: cada tecla vem com o que faz, e passar o ponteiro sobre um item
mostra a descrição na linha de baixo.

Não há uma segunda tabela para consultar, e isso é de propósito — a lista sai do mesmo registro de
onde as teclas são lidas, então ela não tem como divergir. Um markdown com dezesseis atalhos teria.

**Por que não `Super`?** Aqui o compositor roda como *janela dentro* do seu, e quem recebe o `Super`
primeiro é o de fora — a tecla nunca chega. Na distro instalada, onde ele **é** o compositor da
sessão, a leader é o `Super` sozinho. O `Ctrl+Alt+Espaço` também não é intocável: se o **seu**
ambiente já usar essa combinação, ele a captura antes, e a tecla não chega aqui. O sintoma é
"nenhum atalho funciona", que tem três causas com a mesma tela — rode `nix run .#diagnostics` antes
de supor qual delas é.

Quatro atalhos ficam fora da lista de propósito, para quando a máquina para de responder:
`Ctrl+Alt+L` abre a lista quando a leader falha, `Ctrl+Shift+L` restaura o teclado,
`Ctrl+Alt+Esc` abre o monitor de processos e **`Ctrl+Alt+Del` abre o console de resgate**.

> **O `Ctrl+Alt+Del` deixou de reiniciar.** Ele abre o console de resgate, e reiniciar é uma linha lá
> dentro. Se a máquina estiver travada a ponto de nem isso responder, **sete toques em dois segundos**
> ainda reiniciam à força — essa saída é do systemd e não passa por nada nosso.
>
> Com o compositor vivo, o console abre numa janela flutuante e o mouse funciona. Sem ele — compositor
> travado, sessão que não subiu —, o systemd abre o mesmo programa num console virtual próprio, e aí é
> só teclado. `Alt+F1` volta para onde você estava; sair do console também.

O comando imprime as teclas certas ao subir. Elas valem mais do que este texto.

## Os outros comandos

```
nix run .#quick        dez escolhidos — o laço de edição, ~1 min
nix run .#most         os que não constroem instalação — antes de commitar
nix flake check        o mesmo que .#gate, cru — sem dimensionar pela memória
nix run .#vm           a distro inteira numa VM: initrd, impermanência, boot de verdade
nix run .#bench        mede os alvos de desempenho; sai != 0 se algum estourar
nix run .#diagnostics  instantâneo de quando os atalhos travam (só leitura)
nix build .#iso        gera o ISO instalável
```

### As três baterias, e por que são três

| | quanto | quando |
|---|---|---|
| `nix run .#quick` | ~1 min | no meio de uma edição |
| `nix run .#most` | minutos | antes de commitar |
| `nix run .#gate` | horas | antes de publicar — **só ele fecha o invariante 7** |

O **`quick`** são dez nomes em `tests/quick.nix`: formatação dos dois idiomas, clippy, testes e as
tabelas que o compilador não confere. É um **subconjunto escolhido**, e não uma categoria — há mais
de cem checks e a maioria também não sobe VM. Ele diz isso ao terminar.

O **`most`** é derivado: todos os checks menos os de `tests/slow.nix`. O que sai de lá **constrói uma
instalação** — um sistema NixOS, uma imagem de disco ou o ISO — ou é um marco (`mN`), que é pergunta
de hora de publicar. As VMs comuns ficam, e ficam por medição: cada roteiro leva de 17 a 48 s, e
mesmo os que particionam um disco ou emulam um TPM não saem da faixa (`disko-vm` 77 s, `oom-vm` 49 s,
`tpm-vm` 22 s). Ao terminar, ele **lista o que não verificou**, com os nomes.

> **A divisão não pode envelhecer.** `test-tiers-are-total` exige que todo nome de `tests/slow.nix`
> exista em `checks`, e `quick-names-exist` faz o mesmo pelo laço curto. Um check novo entra no
> `most` sozinho — que é o padrão certo, porque um check caro esquecido faz o relógio reclamar, e um
> check barato esquecido não faria ruído nenhum.

O `nix run .#vm` é mais lento que o `.#dev`, e o que se ganha é o boot completo. Usuário `noctua`,
senha `noctua`.

### Quando o ponteiro parece atrasado

```
RONOCTUA_MEASURE_HOVER=1 RUST_LOG=ronoctua_render=info nix run .#dev
```

Cada linha que o ponteiro cruza vira uma medida: quanto tempo do EVENTO até o `present`. É a nossa
metade da latência — sem compositor e sem varredura. Se ela vier em milissegundos de um dígito, o
atraso que se vê não está no que o rodapé calcula, e o que sobra é a cadeia: no laço aninhado há um
compositor a mais entre a mão e a tela, nos dois sentidos. Medir a outra metade exige a distro
instalada (`docs/PENDING.md`, item 3).

## Quando é preciso instalar de verdade

Boot, TPM, hibernação e desempenho gráfico não se provam em VM. E instalar costumava custar uma
tarde: o `nixos-install` **constrói** o sistema na máquina alvo, e o que este repositório produz não
existe em cache público nenhum — o shell, o niri do input pinado, o `libghostty-vt` e os buffers wasm
eram compilados do zero, toda vez.

Duas coisas mudaram isso, e elas se somam:

**O ISO carrega o que ninguém tem em cache.** A semente vai no `isoImage.storeContents` e o
`nix-store --load-db` a registra no boot. A máquina alvo não compila nada; ela baixa do
`cache.nixos.org` o que já vem pronto de lá.

**A instalação `minimal` tem 6,9 GiB contra 22,8.** Escolha-a na terceira pergunta. Ela deixa de fora
os aplicativos e não deixa de fora a distro — e quando você precisar de um deles, é uma linha:

```
ronoctua.browser.enable = true;    # em /etc/nixos/host.nix, depois nixos-rebuild switch
```

Instalar o Chrome assim baixa 0,42 GiB. Instalar a `full` para chegar nele baixa 23.

Depois da primeira instalação, o laço deixa de passar pelo ISO: edite, e rode `nixos-rebuild switch
--flake /etc/nixos#noctua` na própria máquina.

## Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| `hash mismatch` num input de fonte | o asset foi reenviado com outro conteúdo | `nix flake update fonts-ttf` (e os outros três), e confira por que ele mudou |
| `404` num input de fonte | o Release não está publicado, ou o repositório está privado | asset de Release herda a visibilidade do repositório; não há token que contorne |
| `nix run .#dev` diz que não há sessão gráfica | você está num TTY | use `nix run .#vm` |
| o rodapé não aparece, mas o compositor sobe | ele recusou a layer surface | `RUST_LOG=ronoctua_shell=debug nix run .#dev` mostra o motivo |
| nenhum atalho responde | tecla que não chega, modificador preso, ou foco não concedido | `nix run .#diagnostics` separa as três |
| a modeline não mostra porcentagem ao colar | a fila de transferências não subiu | `journalctl --user -u ronoctua-transfers`; fora do NixOS ela é iniciada pelo próprio rodapé, e `RUST_LOG=ronoctua_transfers=info` diz o porquê |
| um link some da fila com `o aria2 nao esta atendendo` | o aria2 é uma **unidade**, e o laço aninhado não sobe unidade nenhuma | é esperado fora de uma sessão de verdade; para exercitar o motor, rode `aria2c --conf-path=$XDG_RUNTIME_DIR/ronoctua-aria2.conf` à mão depois de gerar o arquivo, ou use `nix run .#vm` |
| um vídeo falha com `esta instalacao nao tem o yt-dlp` | o caminho vem da unidade, não do binário | no laço aninhado, `export RONOCTUA_YT_DLP=$(which yt-dlp)` antes de subir — baked por `option_env!` ele entraria no perfil reduzido, e por isso não é |
| um vídeo com restrição de idade falha, mesmo com `t i v` | os cookies são decisão da MÁQUINA, e o laço aninhado não sobe unidade nenhuma | `export RONOCTUA_COOKIES_BROWSER=chrome+basictext` antes de subir, ou aponte um arquivo com `RONOCTUA_COOKIES_FILE`; no sistema instalado é `ronoctua.transfers.cookies` |
| `Login with OAuth is no longer supported` | o yt-dlp recusa OAuth desde antes desta versão | não é contornável: cookies é o caminho, e o PO token (`$XDG_STATE_HOME/ronoctua/po-token`) é o que faz o papel de "dispositivo real" |
| o yt-dlp diz que precisa de PO token | o gerador é opcional e vem desligado | `ronoctua.transfers.poToken = "on-demand"`; no laço aninhado, `export RONOCTUA_POT_HOME=$(nix build --no-link --print-out-paths .#pot-provider)/server RONOCTUA_DENO=$(command -v deno)` |
| `Not implemented: HTMLCanvasElement's getContext()` | o `canvas` não é construído de propósito — nada sob `server/src` o importa | é esperado: o plugin lê a última linha da saída, e o token vem nela |
| o navegador se desloga sozinho depois de um download identificado | o YouTube gira os cookies, e usar os do navegador em uso pode invalidá-los | `ronoctua.transfers.cookies = "file"` com um jar exportado de uma janela anônima onde você entrou e que fechou SEM sair da conta |
| um magnet não abre nada | o `.desktop` é do sistema instalado | `xdg-open` só encontra o manipulador depois de um rebuild; à mão, `ronoctua-transfers open '<magnet>'` |
| a extensão não aparece no Chrome | a política é escrita por unidade de SISTEMA, no boot | `systemctl status ronoctua-browser-extension`; o Chrome lê a política ao **iniciar**, então feche todas as janelas antes de conferir |

**Um processo fica de pé depois do `nix run .#dev`, e é de propósito.** `ronoctua-transfers` é a fila:
ela existe para **sobreviver** ao rodapé reiniciar — que no laço aninhado acontece a cada edição de
arquivo. O `trap` de saída não a mata porque o socket é **por usuário**, não por sessão: matá-la
poderia derrubar uma cópia de verdade da sessão real. Ela é ociosa e a próxima execução a reaproveita;
para encerrá-la à mão, `pkill -f bin/ronoctua-transfers`.

**A sessão de torrent vive dentro dessa unidade.** Ela sobe na primeira vez que um magnet chega e
escuta em 6881-6889; matar a fila para a semeadura, e o estado (o que retomar) fica em
`$XDG_DATA_HOME/ronoctua/torrent`. Um `.torrent` puramente v2 para experimentar sai de
`ronoctua-test-torrent <diretório>`, que cria um, verifica, e não toca a rede.
