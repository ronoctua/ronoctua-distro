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
2. **`~/.shellrc.local` e `~/.config/doom/local.el`.** Eles nunca são versionados de propósito —
   guardam token e identidade — e é exatamente por isso que ninguém lembra deles.
3. **`host.nix` e `facter.json`**, se já existirem. São desta máquina e também ficam fora do git. O
   instalador gera os dois sozinho; o backup só importa se você já os tinha ajustado à mão.

O resto do sistema é reconstruído pelo flake. O que não estiver nessa lista, e não estiver num
backup, deixa de existir.

## Passos

**1. Gerar o ISO** com `nix build .#iso`. O arquivo sai em `result/iso/`.

**2. Gravar no pendrive.** Descubra o dispositivo com `lsblk` e confira duas vezes — o comando abaixo
apaga o que estiver ali:

```
sudo dd if=result/iso/ronoctua-distro-*.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

É `/dev/sdX`, o disco inteiro — não `/dev/sdX1`.

**3. Bootar pelo pendrive.** Entre no setup da UEFI e **desative o Secure Boot** — só para esta
etapa. O sistema instalado o religa com chaves próprias.

**4. Responder às perguntas.**

A primeira é o **idioma**, e todas as telas seguintes aparecem nele. Cada pergunta já vem com uma
opção escolhida; passar por todas apertando Enter dá uma máquina que funciona.

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
| rede sem fio | opcional; deixe em branco para instalar por cabo |

Todo o resto — placa de vídeo, touchpad, bateria, TPM, hibernação — é detectado automaticamente. O
teclado não: o `nixos-facter` lê hardware, e layout é o que os seus dedos esperam.

> **Ver o que ele faria, sem fazer.** `ronoctua-install` sem argumento **ensaia**: imprime o
> `host.nix` que geraria e cada comando que rodaria, e sai sem tocar em disco nenhum. É o padrão —
> apagar exige `--execute`.

### As duas instalações

| | `full` | `minimal` |
|---|---|---|
| tamanho | ~26,7 GiB | 6,9 GiB |
| o que traz | tudo | o sistema, sem os aplicativos |
| fica de fora | — | Android Studio e Flutter, os compiladores (rust, go, python, node, gcc), os agentes de código (Claude Code e Codex), o Blender e o OrcaSlicer, o Chrome, o Emacs, os players de mídia e as variantes extras da fonte |

**A recomendada é a `full`**: ela é a que não pede uma segunda decisão depois.

A `minimal` não é outra distro — compositor, rodapé, fonte, teclado, rede, impermanência e terminal
continuam lá. O que muda é que ela **instala em minutos**, e é por isso que ela existe: quem trabalha
NA distro reinstala com frequência, e quem só quer experimentar um aplicativo o instala nela em vez
de baixar 27 GiB para chegar nele.

**Ligar um grupo depois é uma linha** em `/etc/nixos/host.nix`, seguida de `nixos-rebuild switch`:

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

**6. Primeiro boot.** O disco destrava sozinho pelo TPM; você digita só a senha do usuário.

## Guardar a frase-secreta

O disco é criptografado e normalmente destrava sozinho, porque o TPM guarda a chave e a libera quando
reconhece que o computador não foi adulterado.

**Se algo mudar** — atualizar a UEFI, trocar de placa-mãe, desligar o Secure Boot — o TPM deixa de
liberar a chave, de propósito. Aí a frase-secreta é o único jeito de entrar.

Anote e guarde fora do computador. Sem ela, e nessa situação, os dados são irrecuperáveis.

## Depois de entrar

Um toque em **`Super`** abre a lista de atalhos, no rodapé. É por ela que se descobre todo o resto —
não há outra tabela para decorar, e a lista é sempre a verdade, porque sai do mesmo lugar de onde as
teclas são lidas.

Três atalhos ficam fora dela de propósito, para o caso de a máquina parar de responder:
`Ctrl+Alt+L` abre a lista quando o `Super` falha, `Ctrl+Shift+L` reinicia o `keyd` e
`Ctrl+Alt+Esc` abre o monitor de processos numa janela flutuante.

## Se algo der errado

| sintoma | causa | correção |
|---|---|---|
| o pendrive não aparece no boot | Secure Boot ainda ligado | desative no setup da UEFI |
| `TPM device not found` | TPM desativado no firmware | ligue-o; costuma aparecer como "PTT", "fTPM" ou "Security Device" |
| pede a frase-secreta todo boot | o TPM não libera a chave | algo no boot mudou; entre com a frase e rode `sudo ronoctua tpm-reenroll` |
