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

---



## Funcionalidade especificada e ausente

### 1. `ronoctua-design` não existe · decidir antes

O invariante 12 nomeia `ronoctua-design` e `ronoctua-glyphdb` como componentes `wasm32-wasip2`. O
`glyphdb` virou dados gerados por `nix/glyphgen.nix` e consumidos por `buffers/glyphs` — efeito
equivalente. Já o `ronoctua-design` — a camada de componentes que produziria a display list, sem
threads, filesystem nem wgpu — não existe em forma nenhuma: cada buffer a monta direto do
`ronoctua-protocol`.

**Decidir primeiro:** o invariante descreve algo que ainda queremos, ou a realidade o superou? Se
superou, **o invariante muda** — e isso é decisão do dono, não do agente.

- acabou quando: ou existe `ronoctua-design` compilando para wasip2, ou o invariante 12 foi reescrito

### 2. Desempenho medido só em host não-NixOS · pequeno

`nix run .#bench` mede num niri **aninhado** dentro de outro compositor, com o contorno de driver de
`nix/graphics-host.nix`. O caminho gráfico da distro instalada é mais curto.

- acabou quando: o mesmo comando roda num NixOS instalado e os números entram no `ARCHITECTURE.md`

### 3. O caminho `--execute` do instalador nunca rodou · médio

O caminho destrutivo **existe de verdade**, e o modo ENSAIO não prova nada dele: ele **imprime** os
comandos. O que falta é rodar.

## O teste de VM foi TENTADO e removido — o que se sabe

O harness certo **não** é o do disko nem um `runNixOSTest` comum: é o padrão do próprio nixpkgs em
`nixos/tests/installer.nix` — um segundo nó com `virtualisation.useBootLoader`, ligado DEPOIS, que
boota do disco que o primeiro escreveu.

O teste chegou a rodar. A closure do alvo entra por `system.extraDependencies` e isso funciona (a
falha inicial do `android-studio` era mirror, e passa na segunda tentativa). **Onde ele parou:** o
`ronoctua-install --execute` roda por 900 s e sai com 124 (timeout) **sem imprimir uma linha** — nem
o plano, que o modo sem terminal imprime ANTES de qualquer passo. Isso não é lentidão; é não chegar
ao começo.

**A explicação mais provável apareceu depois, e custa nada testar.** O `install.rs` decide o modo por
`stdin().is_terminal() && stdout().is_terminal()`, e o shell do driver de teste **é um TTY**. O
binário teria entrado na TUI, desenhado na tela alternada que ninguém lê e ficado esperando tecla até
o timeout — o que explica as duas coisas ao mesmo tempo: o travamento *e* o silêncio, já que o plano
só é impresso no modo sem terminal. Primeira tentativa: `< /dev/null`.

**Cada iteração custa ~15 min**, então vale instrumentar junto: rodar o binário redirecionando para
um arquivo dentro da VM e ler o arquivo, em vez de depender do que o driver mostra.

O teste ficou mais barato desde então: com `ronoctua.profile = "minimal"` a closure que precisa
entrar na VM é de 6,9 GiB e não de 22,8.

- acabou quando: `--execute` roda contra um disco virtual num teste, e o sistema que ele instala boota

### 4. O `ronoctua.icons.superpack` não tira o superpacote do closure · pequeno

A opção existe, e desligá-la só o tira do `environment.systemPackages`. Os 0,61 GiB continuam no
closure de **toda máquina**, porque o `ronoctua-shell` carrega o caminho do superpacote GRAVADO em
tempo de compilação por `option_env!` (`RONOCTUA_ICONS`, posto em `nix/rust.nix`) e é instalado nos
dois perfis. Medido com `nix why-depends`, não suposto.

Por isso ele está fora da tabela de `nix/modules/profile.nix`: listá-lo lá anunciaria uma economia
que não acontece.

O caminho é o mesmo que o instalador já usa para as três listas do xkb (`model.rs`): variável de
ambiente em tempo de EXECUÇÃO primeiro, `option_env!` como reserva do `nix run .#dev`. O
`nix/dev.nix` já exporta `RONOCTUA_ICONS`; falta o `nix/modules/icons.nix` fazer o mesmo e o
`nix/rust.nix` parar de gravar.

- acabou quando: com `superpack = false`, `noctua-icon-theme` não aparece em `nix path-info -r` do
  sistema, e o `apps-icon-from-superpack` continua passando com ele ligado

### 5. O which-key não mostra o keymap do buffer em foco · médio

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

### 6. Clipboard: o ClipCascade não está empacotado · decidir antes

O painel mostra `ClipCascade: não integrado` e o comando de configurar diz o mesmo — honesto, e
reservando o lugar.

**O obstáculo é anterior ao código: ele não existe no nixpkgs.** Integrar exigiria empacotá-lo
primeiro, e ele não é um binário solto — é um cliente que fala com um **servidor próprio**, com
credenciais e um endereço para guardar. Isso traz duas coisas que este sistema trata com cuidado:
mais um segredo (invariante 13) e a área de transferência saindo da máquina.

A pergunta que vem antes de qualquer linha: **o histórico de clipboard deve sair deste computador?**
O `atuin` já respondeu não para o histórico do shell, e pelo mesmo motivo — um comando com token no
argv não deve atravessar a rede. Um clipboard carrega senha colada com a mesma frequência.

- decidir: se sim, o item vira empacotar o ClipCascade e guardar a credencial como os outros
  segredos; se não, o lugar reservado no painel sai e o comando some do keymap
