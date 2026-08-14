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
nix flake check
```

Sai sem reclamar? A fundação está correta. Ele demora bastante na primeira vez, e **não mostra o log
das derivações sem `-L`** — silêncio ali não é sintoma de travamento.

**4. Subir o desktop.**

```
nix run .#dev
```

Abre uma janela com o compositor rodando dentro dela e o rodapé embaixo.

Editar um `.rs` em `crates/` recompila e reinicia o rodapé sozinho, com o compositor de pé. Ele
imprime `hot reload ligado` na partida — se não imprimir, você não está na raiz do projeto.

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

Dois atalhos ficam fora da lista de propósito, para quando o teclado para de responder:
`Ctrl+Alt+L` abre a lista quando a leader falha, e `Ctrl+Shift+L` restaura o teclado.

O comando imprime as teclas certas ao subir. Elas valem mais do que este texto.

## Os outros comandos

```
nix run .#vm           a distro inteira numa VM: initrd, impermanência, boot de verdade
nix run .#bench        mede os alvos de desempenho; sai != 0 se algum estourar
nix run .#diagnostics  instantâneo de quando os atalhos travam (só leitura)
nix build .#iso        gera o ISO instalável
```

O `nix run .#vm` é mais lento que o `.#dev`, e o que se ganha é o boot completo. Usuário `noctua`,
senha `noctua`.

## Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| `hash mismatch` num input de fonte | o asset foi reenviado com outro conteúdo | `nix flake update fonts-ttf` (e os outros três), e confira por que ele mudou |
| `404` num input de fonte | o Release não está publicado, ou o repositório está privado | asset de Release herda a visibilidade do repositório; não há token que contorne |
| `nix run .#dev` diz que não há sessão gráfica | você está num TTY | use `nix run .#vm` |
| o rodapé não aparece, mas o compositor sobe | ele recusou a layer surface | `RUST_LOG=ronoctua_shell=debug nix run .#dev` mostra o motivo |
| nenhum atalho responde | tecla que não chega, modificador preso, ou foco não concedido | `nix run .#diagnostics` separa as três |
