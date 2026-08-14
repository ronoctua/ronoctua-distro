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

O caminho destrutivo **existe de verdade**: `nixosConfigurations.noctua` está definido, o ISO carrega
o flake em `/etc/nixos`, e a sequência copia o flake para um diretório gravável, escreve o `host.nix`
ali, roda o `disko` contra ele e só então copia tudo para `/mnt`. O modo ENSAIO não prova nada disso:
ele **imprime** os comandos.

A flag em português já saiu — era violação do invariante 18, e o `english-names` passou a ler flag de
CLI para ela não voltar. O que falta é o teste.

**O obstáculo tem duas metades, e só uma estava anotada.** A conhecida: o `nixos-install` não tem
rede dentro da VM, então a closure do sistema alvo precisa estar no store dela
(`system.extraDependencies`). A outra apareceu ao procurar como fazer: **bootar o resultado é outro
harness.** O `tests/disko-vm.nix` já boota o que instalou, mas pelo `makeDiskoTest` do próprio disko,
que roda o script DELE — não o nosso instalador. Um `runNixOSTest` comum instala e não boota.

Duas saídas, e a escolha é de desenho:

1. **Dois testes.** Um `runNixOSTest` roda `--execute` e afirma o que só o nosso orquestrador faz —
   o diretório de trabalho, o `host.nix` gerado no lugar certo, a ordem dos passos, o `resume_offset`
   depois do swapfile. O boot continua provado pelo `disko-vm`, que já existe. Mais barato, e cada
   teste falha por um motivo só.
2. **Um teste só**, ensinando o harness do disko a chamar o nosso instalador no lugar do script dele.
   Prova a coisa inteira de uma vez e custa entender um harness de terceiro.

- acabou quando: `--execute` roda contra um disco virtual num teste, e o sistema que ele instala boota

### 4. O which-key não mostra o keymap do buffer em foco · médio

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

### 5. Clipboard: o ClipCascade não está empacotado · decidir antes

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
