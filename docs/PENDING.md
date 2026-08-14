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

Os itens abaixo saem da comparação com o **prompt de bootstrap**, não do que parece faltar olhando
o código. A referência de seção (`§7.6`) aponta para lá.

---



## Funcionalidade especificada e ausente

### 1. Terminal: o motor principal não existe, e a causa é a API · grande · BLOQUEADO no upstream

**Os papéis estão invertidos.** A §7.10 define `libghostty-vt` como o motor, com `alacritty_terminal`
como segunda implementação mantida para o diferencial. Existe só o alacritty.

**A causa foi medida, e não é a que estava anotada aqui.** Não falta a FOD com o cache Zig — o
upstream tem o alvo `zig build lib-vt` e instala onze headers. Falta a **API**: os grupos que o
`vt.h` do v1.3.1 declara são `allocator`, `key`, `osc`, `paste`, `sgr` e `wasm` — parsers e
codificadores de entrada. Não há `screen.h`, `terminal.h` nem `grid.h`.

O trait `Engine` exige `fn grid(&self) -> Grid`. O estado da tela existe em Zig, dentro da
biblioteca, e **não atravessa a fronteira C**. A prosa do header promete "screen, scrollback" e logo
acima avisa: *"This is an incomplete, work-in-progress API."*

O pacote `ghostty` do nixpkgs também não serve: os outputs são `out`, `man`, `shell_integration`,
`terminfo` e `vim` — nenhum `dev`, nenhuma lib.

Construir a FOD e escrever o FFI para descobrir isso no fim seria trabalho perdido, e deixaria um
`Engine` que compila e devolve grade vazia.

O que se perde enquanto durar: o diferencial degrada a golden-file — pega regressão nossa, não
divergência entre implementações. É o risco R1, e ele segue permanente.

- onde: `crates/ronoctua-term/`; o trait `Engine` em `src/lib.rs:185` é o que torna isso reversível
- acabou quando: o upstream expõe a tela em C **e** o check `term-differential` compara dois motores
  com diff de grade zero

### 2. `ronoctua-design` não existe · decidir antes

O invariante 12 nomeia `ronoctua-design` e `ronoctua-glyphdb` como componentes `wasm32-wasip2`. O
`glyphdb` virou dados gerados por `nix/glyphgen.nix` e consumidos por `buffers/glyphs` — efeito
equivalente. Já o `ronoctua-design` (§7.8: "componentes → display list", sem threads, filesystem nem
wgpu) não existe em forma nenhuma: cada buffer monta a display list direto do `ronoctua-protocol`.

**Decidir primeiro:** o invariante descreve algo que ainda queremos, ou a realidade o superou? Se
superou, **o invariante muda** — e isso é decisão do dono, não do agente.

- acabou quando: ou existe `ronoctua-design` compilando para wasip2, ou o invariante 12 foi reescrito

### 3. Input `fonts` ainda é caminho local — o Release já existe · médio

O Release existe: <https://github.com/ronoctua/ronoctua-distro/releases/tag/fonts-v1>,
quatro assets, 1,4 GB. O input ainda é `path:/home/noctua/main/prjs/ronoctua-fonts`,
e é a última coisa entre "compila neste disco" e "compila para quem clonar".

## Os hashes, já medidos

```
noctua-iosevka-TTF.tar.zst              sha256-osypDML0dwl8yg+oJu8jfu7JwUNg7Liphtg15K0VFfE=
noctua-iosevka-TTF-nerdfonts.tar.zst    sha256-YMc7DXeL1N5t4CA1+yeAfO1bO3XqAo7OAH+f64DKGbI=
noctua-iosevka-WOFF2.tar.zst            sha256-A5S4F4e5NdkfHV4Pd7cFtfYnWShRXFZGeCrfheKD02A=
noctua-iosevka-WOFF2-nerdfonts.tar.zst  sha256-VeWizQwXYlqBrzK3ZlUXk0CYwGIErGBLxhGveYKaH+s=
```

## O `reports/` NÃO é problema — conferido

Parecia ser: `nix/fonts.nix` faz `cp -r reports $out/reports` e os tarballs não
trazem `reports/`. Mas ele é **gerado no próprio build** (`mkdir -p reports`, e
`--report "reports/<dir>.json"` na correção das faces), não lido do `fontSrc`.
Nada a decidir e nada a mudar.

## Duas formas de buscar, e qual escolher

**`pkgs.fetchurl` dentro de `nix/fonts.nix`** é mais simples — sem input, sem
`flake.lock`, e o `.tar.zst` é descompactado por você no `unpackPhase`. MAS
quebra o `fonts-input-publishable`, que lê `flake.lock` e deixaria de encontrar
o nó `fonts`. Se for por aqui, o check tem de ser reescrito para olhar o
`fonts.nix`, e a mensagem dele muda de sentido.

**Quatro inputs de flake** mantém o check como está e o `flake.lock` como fonte
da verdade. Cuidado: `type = "tarball"` do Nix **não descompacta `.zst`** — use
`flake = false` e trate como arquivo, descompactando na derivação.

A segunda é a que preserva o que já existe. Comece por ela.

## O passo a passo

1. Meça `reports/` e decida.
2. `flake.nix`: quatro inputs `flake = false` com as URLs do Release.
3. `nix/fonts.nix`: uma derivação que monta
   `noctua-iosevka/{TTF,TTF-nerdfonts,WOFF2,WOFF2-nerdfonts}` a partir dos
   quatro, e passa isso como `fontSrc`. O `oflText` já vem do git — não mexa.
4. `nix flake check --keep-going -L`. **Nada mais rodando em paralelo**: duas
   verificações concorrentes se derrubam com SIGTERM e o log culpa o compilador.
5. Commit assinado. O toque na YubiKey vem **depois** do `pre-commit: ok`, não
   no começo — ver a memória `commit-exige-o-dono`.

## Como saber que acabou

`fonts-input-publishable` imprime `OK: input 'fonts' e um asset remoto e fixado
por hash`. Ele lê o `flake.lock` de verdade desde a fusão — antes disso respondia
por um fallback e teria dito "caminho local" para sempre.

### 4. Desempenho medido só em host não-NixOS · pequeno

`nix run .#bench` mede num niri **aninhado** dentro de outro compositor, com o contorno de driver de
`nix/graphics-host.nix`. O caminho gráfico da distro instalada é mais curto.

- acabou quando: o mesmo comando roda num NixOS instalado e os números entram no `ARCHITECTURE.md`

### 5. `--executar` do instalador: falta o teste ponta a ponta · médio

O caminho destrutivo agora **existe de verdade**: `nixosConfigurations.noctua` foi definido, o ISO
carrega o flake em `/etc/nixos`, e a sequência passou a copiar o flake para um diretório de trabalho
gravável, escrever o `host.nix` ali, rodar o `disko` contra ele e só então copiar tudo para `/mnt`.
Antes disso nada disso existia — o `#noctua` era um atributo inexistente, o `disko` avaliava um
caminho que só passaria a existir depois dele, e ninguém punha o flake no alvo. O modo ENSAIO não
via nada: ele **imprime** os comandos.

O que falta é rodar. Um teste de VM precisa da closure do sistema alvo dentro da VM (o
`nixos-install` não tem rede lá), e é isso que ainda não foi montado.

- acabou quando: um teste de VM roda `--executar` num disco virtual e boota o sistema resultante

### 6. O which-key não mostra o keymap do buffer em foco · médio

O host do buffer resolve o keymap do PRÓPRIO descritor: `Ctrl+<primeira tecla>` abre a sequência, as
seguintes são letras simples, e o que completa vira `Input::Command`. É o que torna o editor
salvável.

**Quem resolve é o processo do buffer, e não o rodapé.** Um buffer é janela própria (invariante 4):
enquanto tem o foco, é ele que segura o teclado.

Falta a outra metade: **o rodapé não lista essas ligações sob `SPC m`.** O obstáculo de tipo saiu —
`ronoctua_keymap::Entry` agora guarda `String`, e o enxerto cabe. O que resta é maior e só ficou
visível depois: **o rodapé não tem como saber o descritor de um buffer em execução.** Ele conhece as
janelas por IPC do niri (id, título, `app_id`) e nada mais; o keymap vive no processo do buffer.
Fechar isto pede o buffer PUBLICAR o descritor onde o rodapé leia — e, para a tecla não mentir, um
caminho de volta que entregue o `Input::Command` ao processo certo.

- acabou quando: com um buffer em foco, `SPC m` LISTA as ligações dele **e** a tecla seguinte as roda

### 7. Clipboard: o ClipCascade não está empacotado · decidir antes

O painel mostra `ClipCascade: não integrado` e o comando de configurar diz o mesmo — honesto, e
reservando o lugar.

**O obstáculo é anterior ao código: ele não existe no nixpkgs.** Integrar exigiria empacotá-lo
primeiro, e ele não é um binário solto — é um cliente que fala com um **servidor próprio**, com
credenciais e um endereço para guardar. Isso traz duas coisas que este sistema trata com cuidado:
mais um segredo (seção 9, invariante 13) e a área de transferência saindo da máquina.

A pergunta que vem antes de qualquer linha: **o histórico de clipboard deve sair deste computador?**
O `atuin` já respondeu não para o histórico do shell, e pelo mesmo motivo — um comando com token no
argv não deve atravessar a rede. Um clipboard carrega senha colada com a mesma frequência.

- decidir: se sim, o item vira empacotar o ClipCascade e guardar a credencial como os outros
  segredos; se não, o lugar reservado no painel sai e o comando some do keymap

### 8. Boot sem sessão gráfica não pede a senha do Wi-Fi · pequeno

Se não houver blob selado — rede nova, ou TPM recusando —, o agente responde `NoSecrets` e o
NetworkManager pergunta a quem estiver olhando. Num boot sem sessão gráfica não há ninguém: a máquina
fica sem Wi-Fi até o login.

Quem sofre: um boot remoto, ou o primeiro boot depois de trocar a senha do roteador.

O que falta: decidir se vale um caminho de console (o instalador tem TUI e poderia ser reusada) ou se
o comportamento está certo como está — não guardar a senha tem esse preço, e ele é o mesmo do
`psk-flags=2`.

### 9. `git init` · do dono

Invariante 2. `.gitignore` e `.gitattributes` estão prontos nos dois repositórios, e o devshell
instala o hook de pre-commit sozinho assim que `.git` existir.
