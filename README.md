# ronoctua-distro

Uma distribuição Linux cujo desktop funciona como o Doom Emacs: sem barra, sem dock, sem menu
iniciar. Só o conteúdo ocupando a tela e um rodapé de duas linhas.

Este guia tem duas partes, e nada além delas. Se você quer entender **por que** o sistema é assim,
leia [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## Parte 1 — rodar a versão de desenvolvimento

Para experimentar sem mexer no seu computador.

### O que você precisa antes

**Nix com flakes habilitado.** Confira com `nix --version`; se der "command not found", instale por
<https://nixos.org/download>. Depois:

```
mkdir -p ~/.config/nix
echo 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```

**Nada além disso.** As fontes ocupam 4,1 GB e **não estão no git** — nem como LFS, cuja cota
gratuita é quatro vezes menor que elas. Elas são saída de build, e vivem como assets de um Release
**deste mesmo repositório**, que o flake busca por hash. Você não precisa instalar nada para isso.

### Passos

**1. Clonar.**

```
git clone https://github.com/ronoctua/ronoctua-distro.git
cd ronoctua-distro
```

As fontes vêm sozinhas: são um input do flake, buscadas do Release por hash na primeira construção.
São ~1,4 GB comprimidos, demoram, e é uma vez só.

**2. Entrar no ambiente.**

```
nix develop
```

Basta isto — não exige instalar mais nada. A primeira vez compila bastante; depois é instantâneo.

> **Sobre o `direnv`.** Ele entra no ambiente sozinho ao entrar na pasta, poupando digitar
> `nix develop`. Mas é um programa do **seu sistema**, não do flake — e o projeto não tem como
> fornecê-lo, porque você precisaria dele *antes* de poder entrar no ambiente. Se quiser:
> `sudo pacman -S direnv` (ou o gerenciador da sua distro), `eval "$(direnv hook zsh)"` no seu
> `.zshrc`, e `direnv allow`. O `.envrc` busca o `nix-direnv` sozinho.

**3. Verificar que está tudo de pé.**

```
nix flake check
```

Sai sem reclamar? A fundação está correta.

**4. Subir o desktop.**

```
nix run .#dev
```

Abre uma janela com o niri rodando dentro dela e o rodapé embaixo. É o loop de trabalho de verdade,
e não precisa de VM.

| tecla | o que faz |
|---|---|
| `Alt` + `Espaço` | abre a lista de atalhos, abaixo das duas linhas do rodapé |
| `Alt` + seta (segurando) | move o foco — é *acorde*, e não passa pela lista |
| `Alt` + `Espaço`, `:` | o localizador de comandos, com nome e descrição |
| `Alt` + `Espaço`, `h` `s` | o manual de atalhos; as setas rolam, `Esc` fecha |
| `Alt` + `Espaço`, `r` `r` | as redes ao alcance; `Enter` conecta, `f` esquece, `w` liga o rádio |
| `Alt` + `Espaço`, `r` `i` | banda, taxa e sinal do enlace — a resposta a "por que está lento?" |
| `Alt` + `Espaço`, `r` `s` `s` | o estado do Syncthing, que já vem instalado |
| `CtrlDir` + `←` `→` | vira a página do que estiver na região expandida |
| `Ctrl` + `Alt` + `L` | a leader alternativa, se a de cima falhar |
| `Ctrl` + `Shift` + `L` | restaura o teclado (reinicia o `keyd`) |
| mouse sobre um item | a descrição dele aparece na linha de baixo |
| clique num item | roda o comando, ou desce no submenu |
| `AltGr` `'` `a` | `á` — o AltGr é a tecla Compose (`~ a` → `ã`, `, c` → `ç`) |
| `Esc` | fecha o que estiver aberto |
| `Alt` + `Espaço`, `q` `q` | sai |

> **Por que `Alt`, e não `Super`?** Aqui o niri roda como *janela dentro* do seu compositor, e quem
> recebe o `Super` primeiro é ele — a tecla nunca chega ao niri lá dentro. Na distro instalada, onde
> o niri **é** o compositor da sessão, **tocar** `Super` abre a lista e **segurar** `Super` é o
> modificador dos acordes; quem separa as duas coisas é o `keyd`, que não existe aqui. Por isso, no
> aninhado, o acorde é o `Alt` **sozinho** — apertar o Espaço junto é o que abre a lista. O comando
> imprime as teclas certas ao subir; elas valem mais do que este texto.

Editar um `.rs` em `crates/` recompila e reinicia o rodapé sozinho, com o compositor de pé. Ele
imprime `hot reload ligado` na partida — se não imprimir, você não está na raiz do projeto.

### Os outros comandos

```
nix run .#vm       a distro inteira numa VM: initrd, impermanência, boot de verdade
nix run .#bench    mede os alvos de desempenho; sai != 0 se algum estourar
nix build .#iso    gera o ISO instalável
```

O `nix run .#vm` é mais lento que o `.#dev`, e o que se ganha é o boot completo. Usuário `noctua`,
senha `noctua`.

### Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| `hash mismatch` no input `fonts` | o Release foi refeito com outro conteúdo | atualize o hash no `flake.nix` com o que `nix run .#fonts-release` imprime |
| `path ... does not exist` no input `fonts` | o input ainda aponta para um caminho local | veja o item 5 do `docs/PENDING.md` |
| `nix run .#dev` diz que não há sessão gráfica | você está num TTY | use `nix run .#vm` |
| o rodapé não aparece, mas o niri sobe | o compositor recusou a layer surface | `RUST_LOG=ronoctua_shell=debug nix run .#dev` mostra o motivo |

---

## Parte 2 — instalar a versão de produção

> ⚠️ **Isto apaga o disco escolhido, por inteiro.**

### O que você precisa antes

Um computador **x86_64 com UEFI e TPM 2.0**, um **pendrive de 4 GB ou mais** (todo o conteúdo dele
será apagado), e saber **qual disco** você quer usar.

### Passo zero — se a distro vai substituir a máquina onde ela foi escrita

Este é o caso normal, e é o que dá errado com mais frequência: o disco que vai ser apagado é o
mesmo que guarda o repositório.

**Antes de gravar o pendrive**, garanta que estas três coisas existem em outro lugar:

1. **O repositório, empurrado.** E o Release das fontes publicado: elas são input do flake, e sem
   ele o flake não avalia numa máquina que não tenha os 4,1 GB no disco.
2. **`~/.shellrc.local` e `~/.config/doom/local.el`.** Eles nunca são versionados de propósito
   (invariante 13, guardam token e identidade) — e é exatamente por isso que ninguém lembra deles.
3. **`host.nix` e `facter.json`**, se já existirem. São desta máquina e também ficam fora do git.
   O instalador gera os dois sozinho; o backup só importa se você já os tinha ajustado à mão.

O resto do sistema é reconstruído pelo flake. O que não estiver nessa lista, e não estiver num
backup, deixa de existir.

### Passos

**1. Gerar o ISO** com `nix build .#iso`. O arquivo sai em `result/iso/`.

**2. Gravar no pendrive.** Descubra o dispositivo com `lsblk` e confira duas vezes — o comando
abaixo apaga o que estiver ali:

```
sudo dd if=result/iso/ronoctua-distro-*.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

É `/dev/sdX`, o disco inteiro — não `/dev/sdX1`.

**3. Bootar pelo pendrive.** Entre no setup da UEFI e **desative o Secure Boot** — só para esta
etapa. O sistema instalado o religa com chaves próprias.

**4. Responder às perguntas.**

| pergunta | observação |
|---|---|
| disco de destino | será apagado por inteiro |
| nome da máquina | começa com `noctua-`. Ex.: `noctua-z16` |
| sua senha | do usuário `noctua` |
| frase-secreta do disco | criptografa o disco. **Guarde**, veja abaixo |
| idioma | português ou inglês |
| layout do teclado | o nome do xkb: `br`, `us`, `gb`. É onde você vai digitar a frase-secreta |
| hibernação | se sim, reserva swap do tamanho da RAM |

Todo o resto — placa de vídeo, touchpad, bateria, TPM — é detectado automaticamente. O teclado não:
o `nixos-facter` lê hardware, e layout é o que os seus dedos esperam.

> **Ver o que ele faria, sem fazer.** `ronoctua-install` sem argumento **ensaia**: imprime o
> `host.nix` que geraria e cada comando que rodaria, e sai sem tocar em disco nenhum. É o padrão —
> apagar exige `--executar`.

**5. Esperar e reiniciar.** Ao terminar, tire o pendrive.

**6. Primeiro boot.** O disco destrava sozinho pelo TPM; você digita só a senha do usuário.

### Guardar a frase-secreta

O disco é criptografado e normalmente destrava sozinho, porque o TPM guarda a chave e a libera
quando reconhece que o computador não foi adulterado.

**Se algo mudar** — atualizar a UEFI, trocar de placa-mãe, desligar o Secure Boot — o TPM deixa de
liberar a chave, de propósito. Aí a frase-secreta é o único jeito de entrar.

Anote e guarde fora do computador. Sem ela, e nessa situação, os dados são irrecuperáveis.

### Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| o pendrive não aparece no boot | Secure Boot ainda ligado | desative no setup da UEFI |
| `TPM device not found` | TPM desativado no firmware | ligue-o; costuma aparecer como "PTT", "fTPM" ou "Security Device" |
| pede a frase-secreta todo boot | o TPM não libera a chave | algo no boot mudou; entre com a frase e rode `sudo ronoctua tpm-reenroll` |
