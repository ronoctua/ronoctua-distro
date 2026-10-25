# Instalar no disco

> ⚠️ **Isto apaga o disco escolhido, por inteiro.**

## O que você precisa antes

Um computador **x86_64 com TPM 2.0**, um **pendrive de 8 GB ou mais** (todo o conteúdo dele será
apagado), e saber **qual disco** você quer usar.

- **O TPM é obrigatório.** É nele que a máquina guarda os segredos que abre sozinha, a senha do Wi-Fi
  primeiro: com ele, uma cópia do disco não leva esses segredos. Sem um TPM 2.0 o instalador para
  depois do idioma e explica como ligá-lo no setup do firmware (costuma aparecer como TPM, "Security
  Device", PTT ou fTPM); o modo ensaio ainda mostra o resto das telas.
- **O UEFI é o recomendado, e a BIOS também instala.** Ligado pelo UEFI, o boot é o systemd-boot, o
  padrão da distro. Ligado pela BIOS (quase sempre o modo Legacy/CSM ligado no setup de um computador
  que tem UEFI), o instalador explica como passar ao UEFI e só segue se você escolher "Continuar com a
  pior alternativa": a máquina instala com o GRUB e o mesmo ensaio de boot. Essa tela vem antes da do
  TPM, porque no modo BIOS muitos computadores escondem o TPM. Sem terminal, a BIOS só segue com
  `RONOCTUA_INST_FIRMWARE=bios`.

## Passo zero — se a distro vai substituir a máquina onde ela foi escrita

Este é o caso normal, e é o que dá errado com mais frequência: o disco que vai ser apagado é o mesmo
que guarda o repositório.

**Antes de gravar o pendrive**, garanta que estas três coisas existem em outro lugar:

1. **O repositório, empurrado.** E o Release das fontes publicado: os arquivos de fonte são input do
   flake, e sem eles o flake não avalia numa máquina que não tenha os 4,1 GB no disco.
2. **Os pertences, sincronizados** (`LEADER s v`). O repositório privado deles é de onde a máquina
   nova restaura a sua configuração e, por padrão, também `~/.ssh` (menos a chave de acesso ao próprio
   repositório), a configuração do git, `~/.gnupg`, `~/.shellrc.local`, a sua configuração do VS Code
   em `~/.config/Code/User` (com a lista das suas extensões, que a máquina nova oferece reinstalar) e
   a sua configuração do Emacs em `~/.config/emacs` (sem os pacotes e caches, que se baixam de
   novo). Se o seu Emacs lê outro
   lugar (`~/.emacs.d`, `~/.emacs`, ou um `$XDG_CONFIG_HOME` fora de `~/.config`), ele não viaja:
   mova a configuração para `~/.config/emacs` (e desfaça o `$XDG_CONFIG_HOME` que o desvia) — a
   página dos pertences diz qual lugar o Emacs lê. O que você marcou como "não guardar" não vai:
   guarde por fora.
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

**Num pendrive com Ventoy**, o ISO é um arquivo na partição dele. Baixe-o direto para lá: o
`fetch-iso` só dá nome ao ISO depois de lê-lo de volta do próprio pendrive, igual ao da versão.

```
nix run github:ronoctua/ronoctua-distro/release#fetch-iso -- --into /run/media/$USER/Ventoy
```

As partes esperam no cache deste computador (`~/.cache/ronoctua/fetch-iso`) e só o ISO juntado vai
para o pendrive: ele precisa do tamanho do ISO livre, e o computador, do mesmo tanto enquanto junta.
Uma cópia feita de outro jeito (pelo gerenciador de arquivos, por exemplo) se confere onde está:

```
nix run github:ronoctua/ronoctua-distro/release#fetch-iso -- --check /run/media/$USER/Ventoy/ronoctua-distro-….iso
```

O `--check` também lê um pendrive gravado com `dd` (`--check /dev/sdX`, como root). Seja qual for o
jeito, **ejete o pendrive antes de tirá-lo**: a cópia termina na memória antes de chegar ao
pendrive, e um pendrive tirado cedo demais guarda o nome e o tamanho do ISO com bytes velhos dentro.
O ISO confere o próprio pendrive no boot, antes de usá-lo, e para dizendo isso (cerca de 40 s num
pendrive USB 3).

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
| programas opcionais | só na `full`: os programas pesados, cada um marcado ou não. O desmarcado fica de fora desta máquina — veja a seção abaixo |
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
| tamanho | ~37 GiB com tudo; ~35,5 com as escolhas padrão da instalação (sem o LibreOffice) | ~12,5 GiB |
| o que traz | tudo | o sistema e o trabalho nele (o toolchain do devshell, o rust, o Common Lisp e os agentes de código), sem os aplicativos |
| fica de fora | — | Android Studio e Flutter, os outros compiladores (go, python, node, gcc), o Blender e o OrcaSlicer, o ONLYOFFICE e o LibreOffice, os Crafting Apps, o Chrome, o Emacs, o VS Code, os players de mídia e as variantes extras da fonte |

**A recomendada é a `full`**: ela é a que não pede uma segunda decisão depois.

**Na `full`, a tela seguinte lista os programas pesados** — Android Studio, Flutter, o Android SDK, o
VS Code, o Blender, o OrcaSlicer, o VLC, o ONLYOFFICE, o LibreOffice e as Crafting Apps —, cada um
marcado ou não; o LibreOffice vem desmarcado. `Enter` (ou espaço) numa linha marca ou desmarca, e
`Enter` em "Seguir com estes" continua. O desmarcado não é baixado agora, e nenhuma atualização o
traz, **só nesta máquina**: ele vira uma recusa sua, válida só para o nome desta máquina e gravada
no seu repositório — assinada e enviada antes de o disco ser tocado quando o repositório já tem algo;
num repositório vazio, assinada pelo `init`, depois da instalação. Um programa marcado que esta
máquina (pelo mesmo nome) deixou de fora antes volta; só uma recusa que os seus pertences têm para
todas as máquinas continua valendo, e a instalação a diz. Para trazer um programa de volta depois, é
a central de programas ("Trazer de volta"), nunca o `host.nix`.

A `minimal` não é outra distro — compositor, rodapé, fonte, teclado, rede, impermanência e terminal
continuam lá. O que muda é que ela **baixa uma fração** do que a `full` baixa, e é por isso que ela
existe: quem trabalha NA distro reinstala com frequência, e quem só quer experimentar um aplicativo o
instala nela em vez de baixar 35,5 GiB para chegar nele.

**Ligar um grupo depois é uma linha** em `/etc/nixos/host.nix`, seguida do rebuild de
[A pasta da máquina](#a-pasta-da-máquina) (`sudo systemctl start ronoctua-rescue-rebuild.service`):

```
ronoctua.browser.enable = true;    # o Chrome
ronoctua.editor.emacs   = true;    # o Emacs, com o daemon (a configuração é a sua)
ronoctua.editor.vscode  = true;    # o VS Code, sem telemetria (~0,91 GiB; a configuração é a sua)
ronoctua.media.enable   = true;    # VLC, mpv, gstreamer (o ffmpeg vem sempre)
ronoctua.dev.toolchains = true;    # go, python, node, gcc (o rust e o Common Lisp vêm sempre)
ronoctua.dev.android    = true;    # Android Studio, SDK, Flutter
ronoctua.dev.agents     = true;    # Claude Code e Codex
ronoctua.workshop.enable = true;   # Blender e OrcaSlicer (que abre .stl, .3mf e .step)
ronoctua.office.enable  = true;    # ONLYOFFICE e LibreOffice (~3 GiB)
ronoctua.crafts.enable  = true;    # os Crafting Apps: PhotoCraft, VectorCraft, FilmCraft… (~2,2 GiB)
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

### Os Crafting Apps

Nenhum Crafting App abre um tipo de arquivo sozinho: eles ficam no "abrir com" do gerenciador de
arquivos, onde você escolhe um deles para um arquivo. Para um tipo abrir sempre num deles, o padrão é
uma linha sua no `~/.config/mimeapps.list`, com o nome do lançador da distro,
`ronoctua-appimage-<app>.desktop`:

```
[Default Applications]
application/vnd.oasis.opendocument.text=ronoctua-appimage-wordcraft.desktop
```

Um tipo `text/*` (Markdown, RTF, HTML, texto puro) é exceção no gerenciador de arquivos: ele vai
sempre para o editor, antes de qualquer `mimeapps.list` ser lido, e ali um Crafting App só abre o
arquivo pelo "abrir com", um de cada vez. A linha do `mimeapps.list` para um tipo `text/*` vale para
os outros programas que abrem arquivos (o `xdg-open`).

Não use o botão "tornar padrão" do próprio app. O do WordCraft (o único observado; os outros não
foram conferidos) reescreve a linha do tipo no `mimeapps.list` com o nome que o app espera,
`ai.storyteller.wordcraft.desktop`, que a distro não usa: ele desfaz a linha que você escreveu, e o
que fica no lugar nenhum programa lê.

### O Emacs

A distro não escreve configuração nenhuma do Emacs: o que ele lê é seu, em `~/.config/emacs`, que
viaja nos pertences. O daemon sobe com a máquina, e o `e`, o `$EDITOR` e o gerenciador de arquivos
abrem nele. Ele é o Emacs nativo do Wayland (pgtk), e um compositor que cai leva o daemon junto: os
buffers e o que foi editado desde o último auto-save se perdem, e o daemon volta vazio no mesmo
segundo. Encerrar a sessão, desligar e reiniciar não perdem o que foi editado: o daemon termina antes
do compositor, grava o auto-save de cada buffer modificado (`#arquivo#`, que o `M-x recover-file`
devolve) e roda o `kill-emacs-hook`; os buffers abertos fecham. No `Encerrar sessão`, o rodapé
reinicia o daemon e espera por ele até cinco segundos, dizendo isso no minibuffer (encerrar de novo
sai na hora), e o daemon novo fica para a próxima sessão.

Na sua configuração, duas linhas diminuem o que uma queda leva:

```elisp
(auto-save-visited-mode 1)  ; grava os arquivos abertos sozinho, com o teclado parado
(desktop-save-mode 1)       ; guarda de tempos em tempos quais buffers estão abertos, e os reabre
```

### O VS Code

É a build da Microsoft, no ambiente FHS do nixpkgs (onde o servidor de uma extensão feito para outras
distros — o do rust-analyzer, por exemplo — roda), com a telemetria desligada por política
(`/etc/vscode/policy.json`): a configuração aparece como gerenciada, e o seu `settings.json` não a
religa. A sua configuração (`~/.config/Code/User`) viaja nos pertences; as extensões não, e sim a
lista das do Marketplace, do perfil padrão (`ronoctua-extensions.json`, ao lado), que a máquina
atualiza sozinha: uma extensão que você instala entra, uma que você desinstala sai — e sai das outras
máquinas também, sem voltar pela cópia delas. Na máquina nova, a página dos pertences oferece
reinstalar as que faltam, com o "não" primeiro, dizendo de quais hosts do Marketplace elas vêm; o
"sim" libera esses hosts e as baixa, uma de cada vez, na fila de transferências. Enquanto faltar
alguma, a página mantém uma linha que as instala (para tentar de novo depois de uma falha), e
`ronoctua-vscode install`, num terminal, faz o mesmo quando os hosts já foram liberados.

**O rust-analyzer** funciona sem ajuste: o VS Code da distro roda num ambiente que não se diz NixOS
para as extensões, e a extensão usa o servidor que traz, mesmo sem rede. Se você prefere o servidor da
toolchain da distro (o do mesmo Rust que compila), ponha no `settings.json` do perfil padrão e no de
cada perfil seu:

```json
"rust-analyzer.server.path": "rust-analyzer"
```

e deixa de ter o servidor do lançamento da extensão, que pode trazer recursos mais novos.

**A rede que ele abre sozinho.** O VS Code é um programa de terceiro que fala com o fabricante, como o
Chrome: mesmo com a telemetria desligada, ao subir ele pede à CDN dele as listas de controle e o alerta
de emergência e pergunta ao Marketplace se há versão nova das extensões — a do Copilot Chat embutido
inclusive, que ele baixa e instala sozinho. O dicionário do corretor que ele baixaria do Google, a
distro bloqueia; por isso o `code` num terminal avisa, a cada chamada, que não conhece a opção
`host-resolver-rules` e a passa adiante. Para calar o Marketplace, dois ajustes no seu
`settings.json`:

```json
"extensions.autoCheckUpdates": false,
"extensions.ignoreRecommendations": true
```

O primeiro tira a busca da partida (as extensões deixam de se atualizar sozinhas; a busca à mão
continua); o segundo, a pergunta que abrir um arquivo faz (um `.py` pergunta pela extensão de Python).
O `"chat.disableAIFeatures": true` esconde o chat e a IA, mas não cala nada disso.

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
mudou, e os fontes de que o sistema foi avaliado (`kept-sources`: o flake da máquina, a distro e
cada input dela, também a cada boot), para a atualização seguinte não baixar de novo o que não
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
`/var/lib/ronoctua-machine/legacy-<data>`. O flake novo segue a branch `release`; para seguir outra
coisa, diga de onde vem a distro: `nix run .#adopt-machine -- --distro <flake>`.

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
conferências rodam antes do primeiro passo. Os programas que a `full` deixa de fora vêm em
`RONOCTUA_INST_LEFT_OUT`: os ids separados por vírgula (`blender,libreoffice`) ou `none`; sem ela,
fica de fora o que a tela traz desmarcado, e o ensaio imprime, depois do plano, a escolha que seria
assinada (os desmarcados e os marcados), sob o caminho em que é escrita. Para ver as telas novas sem instalar nada: `ronoctua-install --show access` (ou
`network`, `token`, `passphrase`, `passphrase-new`, `hop`, `review`, `left-out`, `compose`) — num terminal, no
tamanho e nas cores dele.

A instalação responde ao mouse num terminal que o mande (por SSH, por exemplo): um clique escolhe a
linha de uma lista, um segundo a confirma, e as teclas da última linha (`Enter`, `Esc`, `Ctrl+C` e,
na lista de programas, `Space`) são clicáveis.


## Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| o pendrive não aparece no boot | Secure Boot ainda ligado | desative no setup da UEFI |
| o boot para dizendo `The stick does not hold this ISO whole` (ou, num ISO anterior a essa conferência, num shell de emergência depois de `Failed to mount /sysroot/nix/.ro-store`) | o pendrive não guarda o ISO inteiro: a cópia não chegou toda a ele (tirado antes de ejetar) ou ele perdeu escritas | grave de novo com `fetch-iso -- --into` a pasta do pendrive, ou confira a cópia com `fetch-iso -- --check`, e ejete antes de tirar |
| o instalador para depois do idioma, falando do TPM | TPM desativado no firmware | ligue-o (costuma aparecer como "PTT", "fTPM" ou "Security Device"); a tela diz os passos |
| o instalador diz que o computador ligou no modo BIOS | o modo Legacy/CSM está ligado | escolha UEFI no setup (a tela diz os passos), ou "Continuar com a pior alternativa" |
| a tela da rede não segue | sem cabo, a rede escolhida não conectou ou pede uma página de login | conecte o cabo ou escolha outra rede; a tela diz qual dos três |
| a tela da chave do repositório não segue | a chave foi adicionada sem escrita | adicione-a de novo com permissão de escrita |
