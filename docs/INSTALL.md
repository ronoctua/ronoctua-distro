# Instalar no disco

> ⚠️ **Isto apaga o disco escolhido, por inteiro.**

## O que você precisa antes

Um computador **x86_64 com UEFI e TPM 2.0**, um **pendrive de 4 GB ou mais** (todo o conteúdo dele
será apagado), e saber **qual disco** você quer usar.

## Passo zero — se a distro vai substituir a máquina onde ela foi escrita

Este é o caso normal, e é o que dá errado com mais frequência: o disco que vai ser apagado é o mesmo
que guarda o repositório.

**Antes de gravar o pendrive**, garanta que estas três coisas existem em outro lugar:

1. **O repositório, empurrado.** E o Release das fontes publicado: os arquivos de fonte são input do
   flake, e sem eles o flake não avalia numa máquina que não tenha os 4,1 GB no disco.
2. **Os pertences, sincronizados** (`LEADER s v`). O repositório privado deles é de onde a máquina
   nova restaura a sua configuração e, por padrão, também `~/.ssh` (menos a chave de acesso ao próprio
   repositório), a configuração do git, `~/.gnupg`, `~/.shellrc.local` e o `local.el` do Doom. O que
   você marcou como "não guardar" não vai: guarde por fora.
3. **`host.nix` e `facter.json`**, se já existirem. São desta máquina e também ficam fora do git. O
   instalador gera os dois sozinho; o backup só importa se você já os tinha ajustado à mão.

O resto do sistema é reconstruído pelo flake. O que não estiver nessa lista, e não estiver num
backup, deixa de existir.

## Passos

**1. Gerar o ISO** com `nix build .#iso`. O arquivo sai em `result/iso/`. Ou baixe o da versão
publicada, já conferido: veja [O ISO de uma versão](#o-iso-de-uma-versão).

**2. Gravar no pendrive.** Descubra o dispositivo com `lsblk` e confira duas vezes — o comando abaixo
apaga o que estiver ali:

```
sudo dd if=result/iso/ronoctua-distro-*.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

É `/dev/sdX`, o disco inteiro — não `/dev/sdX1`.

**3. Bootar pelo pendrive.** Entre no setup da UEFI e **desative o Secure Boot**: o pendrive e o
sistema instalado bootam sem ele.

**4. Responder às perguntas.**

A primeira é o **idioma**, e todas as telas seguintes aparecem nele. As listas já vêm com uma opção
escolhida; o que só você sabe — disco, nome, senhas, o repositório — é digitado. Antes de começar,
leia [O repositório dos pertences e a rede](#o-repositório-dos-pertences-e-a-rede): a instalação exige
os dois, e a chave do repositório é adicionada no meio dela.

| pergunta | observação |
|---|---|
| idioma | português ou inglês. É a primeira porque o resto é desenhado nela |
| disco de destino | será apagado por inteiro |
| instalação | `full` ou `minimal` — veja a seção abaixo |
| layout do teclado | o nome do xkb: `br`, `us`, `gb`. É onde você vai digitar a frase-secreta |
| variante do teclado | `abnt2`, `nodeadkeys`, ou nenhuma |
| tecla de acentos | qual tecla compõe acentos. O padrão é o `Caps Lock` — e os dois Shifts juntos passam a ser o Caps Lock |
| locale do sistema | data, número, papel e unidades. O padrão é `en_GB.UTF-8`, que é métrico |
| fuso horário | o padrão é `Europe/London` |
| nome da máquina | começa com `noctua-`. Ex.: `noctua-thinkpad` |
| sua senha | do usuário `noctua` |
| frase-secreta do disco | criptografa o disco. **Guarde**, veja abaixo |
| rede sem fio | escolha uma, ou "sem Wi-Fi" para instalar por cabo. A instalação exige rede |
| repositório dos pertences | o endereço do seu repositório privado (SSH ou `https://`), sem senha dentro. Obrigatório |
| acesso ao repositório | num endereço SSH, uma chave desta máquina que você adiciona ao repositório com escrita (a tela a mostra e espera); num `https://`, um token |
| senha dos pertences | conferida no repositório antes de o disco ser tocado; num repositório vazio, escolhida e digitada duas vezes |

Todo o resto — placa de vídeo, touchpad, bateria, TPM, hibernação — é detectado automaticamente. O
teclado não: o `nixos-facter` lê hardware, e layout é o que os seus dedos esperam.

> **Ver o que ele faria, sem fazer.** `ronoctua-install` sem argumento **ensaia**: imprime o
> `host.nix` que geraria e cada comando que rodaria, e sai sem tocar em disco nenhum. É o padrão —
> apagar exige `--execute`.

### As duas instalações

| | `full` | `minimal` |
|---|---|---|
| tamanho | ~29 GiB | ~11,8 GiB |
| o que traz | tudo | o sistema e o trabalho nele (o toolchain do devshell, o rust e os agentes de código), sem os aplicativos |
| fica de fora | — | Android Studio e Flutter, os outros compiladores (go, python, node, gcc), o Blender e o OrcaSlicer, o Chrome, o Emacs, os players de mídia e as variantes extras da fonte |

**A recomendada é a `full`**: ela é a que não pede uma segunda decisão depois.

A `minimal` não é outra distro — compositor, rodapé, fonte, teclado, rede, impermanência e terminal
continuam lá. O que muda é que ela **baixa uma fração** do que a `full` baixa, e é por isso que ela
existe: quem trabalha NA distro reinstala com frequência, e quem só quer experimentar um aplicativo o
instala nela em vez de baixar 29 GiB para chegar nele.

**Ligar um grupo depois é uma linha** em `/etc/nixos/host.nix`, seguida do rebuild de
[A pasta da máquina](#a-pasta-da-máquina) (`sudo systemctl start ronoctua-rescue-rebuild.service`):

```
ronoctua.browser.enable = true;    # o Chrome
ronoctua.editor.emacs   = true;    # o Emacs e o Doom
ronoctua.media.enable   = true;    # VLC, mpv, gstreamer (o ffmpeg vem sempre)
ronoctua.dev.toolchains = true;    # rust, go, python, node, gcc
ronoctua.dev.android    = true;    # Android Studio, SDK, Flutter
ronoctua.dev.agents     = true;    # Claude Code e Codex
ronoctua.workshop.enable = true;   # Blender e OrcaSlicer (que abre .stl, .3mf e .step)
ronoctua.fonts.full     = true;    # as 27 variantes da Iosevka
```

Trocar `ronoctua.profile = "minimal"` por `"full"` liga todos de uma vez. Não é reinstalação.

**5. Esperar e reiniciar.** Ao terminar, tire o pendrive.

**6. Primeiro boot.** O disco pede a frase-secreta; depois dela a sessão entra sozinha, sem outra
senha. Os pertences já estão na home: a instalação os restaurou antes do primeiro login.

## Guardar a frase-secreta

O disco é criptografado, e a frase-secreta é a única chave dele: é digitada a cada boot, e o TPM não
guarda nada do disco (ele guarda só a senha do Wi-Fi). O porquê está em `nix/disko/default.nix`.

Anote e guarde fora do computador. Sem ela, os dados são irrecuperáveis.

## Depois de entrar

Um toque em **`Super`** abre a lista de atalhos, no rodapé. É por ela que se descobre todo o resto —
não há outra tabela para decorar, e a lista é sempre a verdade, porque sai do mesmo lugar de onde as
teclas são lidas.

Três atalhos ficam fora dela de propósito, para o caso de a máquina parar de responder:
`Ctrl+Alt+L` abre a lista quando o `Super` falha, `Ctrl+Shift+L` reinicia o `keyd` e
`Ctrl+Alt+Esc` abre o monitor de processos numa janela flutuante.

<!-- anchor: L03 -->

## A pasta da máquina

`/etc/nixos` não é uma cópia da distro: é o flake **desta máquina**, e a distro é um input dele, que
segue a branch `release`. Ali ficam o `flake.nix` (saído de `templates/machine`), o `flake.lock` (que
fixa a versão da distro), o `host.nix` e o `facter.json` que o instalador escreveu, as declarações da
pessoa em `person/` e o que só o root escreve em `local/`. A pasta sobrevive à raiz apagada a cada
boot.

Atualizar é mover o input da distro e reconstruir — a atualização faz as duas coisas sozinha. Para
aplicar à mão uma linha nova do `host.nix`:

```
sudo systemctl start ronoctua-rescue-rebuild.service
```

É o mesmo rebuild do resgate (`ronoctua-machine apply`): constrói antes — dimensionado à memória
livre e nas fases que a distro descreve, só o que o Nix diz que falta —, troca depois e arma o
ensaio de boot; um sistema novo que não sobe volta sozinho ao último bom. Depois de um build que deu
certo, a máquina guarda as unidades compiladas da versão em uso
(`/var/lib/ronoctua-machine/kept-units`, ~2 GiB), para a atualização seguinte recompilar só o que
mudou. Com a atualização rodando, ele recusa por estar ocupado. O atributo do flake tem o nome da
pessoa (`noctua`), e não o da máquina.

A pasta não aceita outra versão do nixpkgs nem de outro input além da que a distro testou: um
`follows` ou um `--override-input` faz a avaliação parar dizendo qual input mudou.

<!-- anchor: L12 -->

O **swap** tem o tamanho da RAM da máquina, arredondado para cima e limitado a um quarto do disco: o
relatório de hardware é escrito antes de o disco ser feito, e o tamanho sai dele. Quando o disco não
comporta a RAM, a máquina é instalada sem hibernação — o `host.nix` diz o tamanho escolhido e traz
`ronoctua.hibernation = false`, e com a bateria no fim ela desliga em vez de hibernar. Numa máquina já
instalada nada muda.

O **`/private`** (a pasta cifrada da pessoa) não existe até ela criá-lo na central de segurança
(`LEADER s e o`). A instalação só prepara o lugar dele, o subvolume `@private`; uma máquina
instalada antes ganha esse subvolume sozinha, no primeiro boot ou `switch` depois da atualização.

<!-- anchor: L16 -->

### O ISO de uma versão

Cada versão publicada no GitHub (`ronoctua/ronoctua-distro`, a mais recente) traz o ISO e o
`SHA256SUMS`. Um ISO de 2 GB ou mais vai em partes (`….iso.part-00`, `-01`…). Para baixar, conferir
e juntar num passo só:

```
nix run github:ronoctua/ronoctua-distro/release#fetch-iso
```

Uma máquina instalada também grava o pendrive sozinha (`LEADER s z`), da mesma versão. O ISO escrito
no pendrive com `dd` é o mesmo nos dois casos. O ISO leva sempre a semente — os programas que só a
distro compila, nunca as unidades de que eles são feitos —, e a instalação baixa o resto do
`cache.nixos.org`. A primeira atualização que muda o código de um crate compila na máquina o que
fica abaixo dos programas que mudaram.

### O seu cache binário

Se os seus pertences nomeiam um cache binário seu (o seu Cachix, na página do cache da vista da
atualização: `LEADER s u c`), a instalação já baixa dele, antes do `cache.nixos.org`. Para um cache
privado, o instalador lê o token da parte cifrada com a senha dos pertences, antes de tocar no disco,
e deixa na máquina o netrc do root com que o Nix dela o lê. Depois, cada atualização que troca a
máquina envia para ele o que ela construiu: num cache público, só as saídas da própria distro; num
privado, o sistema.

### Uma máquina instalada antes do flake próprio

Uma máquina do layout antigo — uma cópia da distro inteira na pasta da máquina — tem uma
atualização que não sabe migrar. Ela é adotada uma vez, de um checkout da distro:

```
nix run .#adopt-machine
```

O flake novo é construído ao lado do velho e só o substitui se construir; o velho fica guardado em
`/var/lib/ronoctua-machine/legacy-<data>`. Enquanto a branch `release` não existir, diga de onde vem
a distro: `nix run .#adopt-machine -- --distro github:ronoctua/ronoctua-distro/main`.

<!-- anchor: L20 -->

## O repositório dos pertences e a rede

A instalação **exige rede e o seu repositório privado.** É com ele que a máquina nasce sua: as
declarações (os programas que você declarou) entram no sistema antes de ele ser construído, e os seus
pertences são restaurados na home antes do primeiro login.

Tenha à mão, antes de começar:

1. **O endereço do repositório** — `git@github.com:voce/pertences.git`, `ssh://…` ou `https://…`, sem
   senha dentro. Um repositório vazio também serve: a instalação o cria.
2. **Onde adicionar uma chave a ele.** Num endereço SSH o instalador faz uma chave só desta máquina e a
   mostra em texto e em QR (dá para ler com o celular), com o link da página "adicionar chave" no
   GitHub, no GitLab e no Codeberg. Adicione-a **com permissão de escrita**: a tela só segue quando a
   chave lê e escreve. Num endereço `https://`, tenha um token que leia e escreva.
3. **A senha dos pertences.** Ela é conferida no repositório antes de o disco ser tocado. Num
   repositório vazio, você a escolhe ali (pelo menos 12 caracteres) e a digita duas vezes.

A pergunta da rede diz quais servidores a instalação vai alcançar; escolher uma rede, ou o cabo, é
deixar. Sem cabo, sem a rede escolhida conectada ou atrás de uma página de login, a tela diz por quê e
não segue; o resto — se o GitHub, o cache e o seu repositório respondem — é conferido nas telas
seguintes, cada uma dizendo o que não respondeu.

**O instalador segue o release.** Se a branch `release` estiver numa versão **mais nova** que a do
pendrive, ele avisa e inicia o instalador dessa versão, levando as respostas já dadas. Se o release não
responder, se o pendrive estiver à frente dele ou for uma versão de desenvolvimento, a instalação segue
com a versão do pendrive — e a máquina **fica nela**: o `flake.nix` e o `flake.lock` de `/etc/nixos`
são os que construíram o sistema, e a atualização ainda não a passa ao release (veja
`docs/PENDING.md`).

Sem terminal (`ronoctua-install` lendo o ambiente), o acesso é o **arquivo** da chave ou do token
(`RONOCTUA_INST_BELONGINGS_ACCESS`) e a senha vem em `RONOCTUA_INST_BELONGINGS_PASSPHRASE`; as mesmas
conferências rodam antes do primeiro passo. Para ver as telas novas sem instalar nada:
`ronoctua-install --show access` (ou `network`, `token`, `passphrase`, `passphrase-new`, `hop`,
`review`).

A instalação responde ao mouse num terminal que o mande (por SSH, por exemplo): um clique escolhe a
linha de uma lista, um segundo a confirma, e as teclas da última linha (`Enter`, `Esc`, `Ctrl+C`) são
clicáveis.


## Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| o pendrive não aparece no boot | Secure Boot ainda ligado | desative no setup da UEFI |
| a instalação para ao guardar a senha do Wi-Fi | TPM desativado no firmware | ligue-o (costuma aparecer como "PTT", "fTPM" ou "Security Device"), ou instale por cabo |
| a tela da rede não segue | sem cabo, a rede escolhida não conectou ou pede uma página de login | conecte o cabo ou escolha outra rede; a tela diz qual dos três |
| a tela da chave do repositório não segue | a chave foi adicionada sem escrita | adicione-a de novo com permissão de escrita |
