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

Três atalhos ficam fora da lista de propósito, para quando a máquina para de responder:
`Ctrl+Alt+L` abre a lista quando a leader falha, `Ctrl+Shift+L` restaura o teclado e
`Ctrl+Alt+Esc` abre o monitor de processos.

O comando imprime as teclas certas ao subir. Elas valem mais do que este texto.

## Os outros comandos

```
nix run .#quick        os checks que não sobem VM — o laço curto, ~1 min
nix flake check        o mesmo que .#gate, cru — sem dimensionar pela memória
nix run .#vm           a distro inteira numa VM: initrd, impermanência, boot de verdade
nix run .#bench        mede os alvos de desempenho; sai != 0 se algum estourar
nix run .#diagnostics  instantâneo de quando os atalhos travam (só leitura)
nix build .#iso        gera o ISO instalável
```

O `nix run .#quick` é o que se roda no meio do trabalho: formatação dos dois idiomas, clippy, testes,
keymap e as duas traduções — tudo o que não precisa de VM. Ele **não fecha nada**, e diz isso ao
terminar: o que fecha é o `.#gate`, que sobe as VMs, o ISO e os quatro sistemas.

O `nix run .#vm` é mais lento que o `.#dev`, e o que se ganha é o boot completo. Usuário `noctua`,
senha `noctua`.

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
