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

### 3. Input `fonts` é caminho local, e as fontes precisam de um Release · médio

Aponta para um caminho absoluto desta máquina; funciona aqui e em nenhuma outra.

**Publicar não é só trocar a URL.** As 540 faces são 4,1 GB: em git puro o GitHub recusa o
repositório, e em LFS são quatro vezes a cota gratuita de 1 GB — a única saída oferecida é um data
pack pago, que está descartado.

A saída é **GitHub Releases**: sem cota, 2 GB por asset, ilimitado para repositório público, de
graça. E `fetchurl` com hash é tão reprodutível quanto um rev de git — mais honesto, aliás, porque
estes arquivos são SAÍDA de build e não fonte. Medido: zstd -19 leva os 4,1 GB a ~1,4 GB, e o maior
asset (TTF-nerdfonts) fica em ~510 MB.

`nix run .#fonts-release -- <caminho>` empacota os quatro diretórios de forma determinística
(`--sort=name`, mtime fixo) e imprime os hashes prontos.

A ordem importa, e é a única parte que não dá para automatizar: o Release tem de **existir antes**
de o input apontar para ele, senão o `nix flake check` quebra num 404.

- acabou quando: o input `fonts` é um `fetchurl` dos assets do Release e o check
  `fonts-input-publishable` para de avisar

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
