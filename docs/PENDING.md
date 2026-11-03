# O que falta

Cada item diz o que falta, por que importa e como saber que acabou. **Ao concluir um item, apague-o**;
se ele mudou como o sistema funciona, o porquê vai para o `docs/ARCHITECTURE.md`. Uma decisão tomada
— o que a distro deliberadamente não faz — só é apagada daqui, sem registro em lugar nenhum. Esta
lista não guarda histórico: item riscado ou marcado como pronto é recusado por um check (`docs-exist`).

Escreva para quem não acompanhou o trabalho: o nome da coisa como a pessoa a vê, o que falta e o
critério de pronto, em poucas linhas.

## Funcionalidade que falta

- **Quatro arquivos de preferência lidos pela metade não avisam na própria vista** (os outros 26
  avisam): `keyboards.json` (desenhado por um buffer WASM: falta um campo no protocolo),
  `brain.json` (o conserto teria de viajar no `Mind`), `index.json` (a página de um módulo do brain
  não leva argumentos) e `files-sidebar.json` (a barra lateral não tem linha para frase).

- **Câmeras MIPI (Intel IPU) e só multiplanar**: não aparecem ou não gravam; a ponte seria o
  libcamera pelo PipeWire, hoje desligado por causa da luz. Médio.
- **Um Flatpak que lê câmera pelo portal não vê nenhuma**, e nada diz por quê.
- **F-Droid**: a ficha de um app não tem descrição longa nem permissões (o catálogo compacto só guarda
  nome e resumo).
- **O crate `age` com o `mlkem768x25519`**: o X-Wing e a estrofe são nossos (`ronoctua-pq`) até uma
  versão do `age` trazê-los; então o nosso sai e o `pq-age-interop` passa com o do crate.
- **`/private`**: o cofre suspenso não volta pela chave FIDO2 (falta o plugin de token no
  `cryptsetup`); sem marca na modeline enquanto aberto.
- **O instalador com `--answers`**: um teclado sem mapa de console é recusado uma vez só, e a tela da
  BIOS ou do TPM pode apagá-lo; o console fica no mapa padrão naquela sessão (a máquina instalada
  continua protegida: o build do mapa falha antes de tocar o disco).
- **A tela de bloqueio**: a cor lisa das outras telas não segue um tema trocado com a sessão
  trancada.
- **Programas declarados**: mudar só o ícone de um web app aparece pela metade até a atualização; a
  trava da declaração não é a mesma dos pertences.
- **O vigia do journal** toma por folha a palavra do systemd sobre uma unidade de gerente delegado
  que não seja `user@<uid>.service` (um contêiner, um serviço com `Delegate=`): uma morte dentro dele
  pode ser atribuída à unidade de fora.
- **Git e arquivos compactados**: pôr um membro numa pasta de dentro do arquivo é recusado; o
  `render-png` não desenha uma fila.
- **As partes: o que ficou.** As partes ficam (decisão do dono, 2026-10-31, `docs/ARCHITECTURE.md`),
  e nenhum programa a mais vira parte. Do plano de 2026-10-23 sobra:
  - os checks `every-download-has-an-owner`, `parts-hold-no-plugin`, `parts-do-not-collide` e o lado
    Rust do `part-paths-are-read-as-arriving`;
  - a página das partes não nomeia o hardware que espera a primeira rede, não mostra a restauração do
    primeiro boot e não distingue as partes da pessoa das da distro; um hardware sem sonda falha no
    `build-first`; o `wide` do `nix/modules/person.nix` sobrou.

- **Escala fracionária (125%, 150%)**: a distro inteira desenha em escala inteira; o terminal
  seguiria o resto (um protocolo novo, `wp_fractional_scale_v1` com o viewporter, seria exceção ao
  inv. 3).
- **O terminal, o que ficou de fora da paridade com o Ghostty**: a forma do ponteiro que um programa
  pede (OSC 22), a área de transferência do kitty com tipos (OSC 5522 além do texto), o contraste
  mínimo, e o texto escrito sobre um sixel não apagar a imagem como no xterm. Um guardião de outra
  versão (depois de um `switch`) ainda não é anexado: a janela só diz que não pode. Abrir pela vista
  um terminal cujo programa já terminou mostra a janela e a fecha na hora, em vez de guardar a
  última tela até uma tecla. A marca dos terminais sem janela pisca por um instante quando um
  terminal novo abre, antes de a janela dele se ligar.
- **O IME além do terminal**: as perguntas das portas, o acesso rápido e a busca do terminal ainda
  não recebem texto do fcitx5; a janela de candidatos veste o tema do fcitx5, não os nossos tokens; o
  Chrome só usa o IME com `--enable-wayland-ime --wayland-text-input-version=3`. Os apps GTK (o
  Ghostty, o Emacs) não chegam ao fcitx5: ficam no `GTK_IM_MODULE=simple`, porque sob o fcitx5 o
  Compose não junta acentos neles; os dois juntos pedem que o fcitx5 componha o Compose.

## O que só uma máquina de verdade, uma foto ou um servidor provam

### Fotos que faltam

O `photograph --door footer` (com `--click` e `--wheel`) alcança parte do rodapé, e o `.#shot` o
resto. Faltam fotos, com asserção, de:

- **no rodapé**: a central de segurança, os containers, o acesso rápido, o ranking, os ícones, a
  calculadora (três modos), o catálogo de vídeos, os pertences nos estados novos, a faixa do cérebro
  com um lembrete tocando, a marca de disco quase cheio, a linha do sistema na página das partes, a
  pergunta secreta, a barra de alerta máximo, as horas no minibuffer, a gaveta que cresce sob o
  ponteiro, e o seletor de arquivos como região;
- **do player**: a coluna de mídia no rodapé (vários tamanhos, mp3 com e sem capa, fluxo sem
  duração), a linha do tempo do miniplayer arrastada, o deck no "Agora", o roteamento por tela com
  duas saídas, os metadados com a capa, a listagem e a grade do catálogo, o par da modeline (com o
  ambiente só), e o filme recolhido;
- **outras**: o localizador numa saída em escala 2 e um buffer com `Command::Svg`; oito janelas sem
  câmera (histórico do shell, notificações, som, energia, ponteiros, Syncthing, manuais, geo); a
  legenda do libass num quadro do daemon; duas telas numa foto; um vídeo e uma forma 3D no painel do
  gerenciador; "instalar só para você" (a sala não tem catálogo); as marcas das contas LLM vindas do
  brain; a página dos sons ambientes com o catálogo; o acesso rápido com resposta (web, YouTube,
  dicionário, tradução); os programas da distro de fora do Nix (Flatpak, AppImage, web, Android) na
  central; e a câmera das portas comparada com o `.#shot` no mesmo instante.

### Na máquina do dono

- **O rodapé transparente**: nos modos ocultos, o véu e a borda em repouso mostram a janela por baixo
  (o driver aceita `PreMultiplied`?), sem a frase `echo-footer-opaque`.
- **Os crafts do ArtCraft Launcher**: rodam fora de um ambiente FHS? que lançador escrevem, que tipos
  tomam? E o tamanho do launcher no `profile.nix`.
- **O custo semanal de rolar o nixpkgs**: uma atualização depois de o nixpkgs andar, sem cache
  binário, cabe nos `runawayMinutes = 120`?
- **O LED da câmera no login**: o `v4l_id` do udev abre cada câmera; acende a luz?
- **VS Code**: o fim da sessão com ele aberto deixa um core de ~3,6 GB? o Copilot Chat embutido
  sobrevive a uma troca de versão? algo no terminal dele lê o `ID=linux`?
- **Chrome quiosque**: vídeo em tela cheia, popup de outra extensão, onde cai a fixação de uma
  extensão, busca em tela cheia, sessão restaurada, "continuar de onde parou", a primeira instalação
  forçada num perfil usado, os 8 px a mais de largura, e o quiosque como primeira partida do Chrome.
- **Programas da pessoa**: o Waydroid rodando (imagem, sessão, um APK do F-Droid); a leader chegando
  dentro de um web app; a ativação do nix-flatpak (o tema escuro, o override dos `QT_*`); um Flatpak
  Electron na sessão.
- **Tamanho do texto**: GTK, Qt, Chromium e Electron seguem a mudança ao vivo?
- **O boot do ISO pisca e reimprime o prompt** num notebook AMD (amdgpu): falta o log
  (`journalctl -b -o short-monotonic -u systemd-vconsole-setup`, e as linhas do `fbcon`/`amdgpu`).
- **A barra do sudo com um sudo de verdade** (senha, chave, dedo, rodapé morto) e o cadastro de uma
  chave de verdade.
- **A tela de bloqueio com um leitor de digital de verdade e em duas telas**: o dedo destravando, a
  pergunta mudando de tela quando a sua é desligada; e os gestos de tentar de novo e reter a máquina
  com um sudo e um systemd de verdade.
- **Contas de LLM**: o total de um dia bate com o `ccusage`/`/usage`; a marca muda de cor depois de
  uma conversa; o login de uma conta nova termina no terminal que abriu.
- **Os agentes com um Claude Code de verdade**: o gancho gerenciado roda (o `sh -c`, o ambiente
  herdado, o dono do arquivo em `/etc` aceito pela checagem dele); o conjunto da conta acende com o
  estado certo, pelo OSC 7501 no nosso terminal e pelo gancho fora dele; o clique foca a janela, ou
  abre o grupo de um terminal no background; uma conta de API sem créditos fica em perigo.
- **A entrada da firmware numa instalação nova**: com o `canTouchEfiVariables` desligado, a
  instalação não registra o systemd-boot no menu da firmware e conta com o caminho de reserva
  `\EFI\BOOT\BOOTX64.EFI`; falta ver numa máquina de verdade que ela liga sozinha depois de instalar,
  inclusive numa que teve outro sistema no disco.
- **O IME numa sessão de verdade**: com o Mozc ligado uma vez nas configurações do fcitx5,
  `Ctrl+Espaço` e `nihon` mostram a composição sublinhada no cursor e os candidatos ao lado, e o
  texto confirmado chega ao programa; o Compose continua funcionando no nosso terminal com o fcitx5
  rodando.
- **O player com o YouTube de verdade no `.#dev`**: toca, ganha o título, uma recusa diz a linha do
  yt-dlp, um álbum ganha o nome do álbum, o mesmo endereço duas vezes pergunta "já está na fila".
- **O catálogo de vídeos contra o YouTube de verdade** (um canal por `@handle`, uma varredura).
- **O pendrive**: o caminho inteiro (a vista, a senha, a fila, gravar e bootar um ISO real), e um
  pendrive espetado com o seletor de arquivos aberto.
- **`.#bench` num NixOS instalado**, e os números no `ARCHITECTURE.md`; e o `which-key` com a leader
  fechada entre as aberturas.
- **O bind transitório `Ctrl+↑/↓`** (`wm-fold-keys-hold`) no compositor de verdade: resolve, sai
  quando a dica sai, e não rouba uma tecla do programa em foco.
- **O player de ponta a ponta**: o rascunho de metadados abrindo no `emacsclient` e voltando como
  pergunta; as buscas na rede e as etiquetas gravadas com um daemon de verdade; os diretórios (rádios
  e TVs) contra o daemon, as páginas seguintes do radio-browser, e o custo de reprojetar o globo com
  milhares de marcas; a emenda dos sons ambientes num aparelho de verdade.
- **Os programas declarados numa máquina**: uma AppImage declarada com o GitHub de verdade virando
  entrada de lançador com o ícone de dentro, o pin de um apk do F-Droid, o reconciliador com o
  Waydroid rodando, o rebuild depois de um "remover de vez" e o envio da declaração ao repositório.
- **A espera do link no primeiro boot** (o tempo para a rede escolhida conectar) foi escolhida, não
  medida.
- **`.#adopt-machine` e `.#apply-here`** numa máquina.

### Num teste de VM ou contra um servidor

- **A coluna em duas telas** (cada tela a do seu espaço; a que some a estaciona), e o roteamento do
  player e o "mostrar em cada tela" com duas saídas.
- **Pertences por SSH e por token** contra um servidor git, a fila de ponta a ponta, e a restauração
  da home no primeiro boot com rede.
- **A primeira atualização de uma máquina instalada pelo ISO** contra o release no ar, e a recusa de
  programas do instalador chegando a um repositório.
- **O cache binário da pessoa** num Cachix de verdade (público e privado; o tempo da consulta), e uma
  máquina sem rede baixando dele na primeira atualização.
- **O Incus**: o WebSocket dos eventos contra o daemon, a vista sobre um daemon com uma instância, as
  `fixtures.rs` trocadas pelo que o `incus-vm` captura, e se o disco de uma VM feita de imagem fica
  sem cópia na escrita.
- **A folga do disco de ponta a ponta** (o cérebro dispara, o relatório volta como frase) e uma
  montagem por bind dentro do cache.
- **O hotspot** sobre o ar simulado (`tests/wifi-air.nix`).
- **O recuo para `wlr-screencopy`** do estúdio contra um compositor que só tem ele.
- **Um app recusado que outro caminho traz** aparecendo como `still-present` (o registro da distro
  estava vazio quando isso foi escrito).
- **O clique na central de segurança** até a página relida, e o `C-h` do gerenciador lançando o
  navegador do git.
- **Legenda de imagem (PGS)** num quadro, e o `mkvtoolnix` de verdade nos testes do escritor de tags.
- **O catálogo nas portas**: a cerca do redirecionamento em claro, a fonte adulta e a senha, e um
  vídeo de cada fonte padrão; um vídeo apagado saindo do índice do acesso rápido.
- **O apply como root num cgroup com `MemoryMax`**, dizendo o orçamento daquele teto.
- **Os formatos de resposta** do MusicBrainz, Cover Art Archive, Wikidata, TMDB, OpenSubtitles e
  SubDL, e os códigos de língua dos dois últimos, conferidos contra respostas de verdade (o Podnapisi
  volta quando o domínio responder).

## Dívida interna

Nada disso muda o que a pessoa vê; faça quando mexer na área.

- **O protocolo da região tem duas formas de algumas ideias** (`ronoctua-shell-door/src/region.rs`): o
  que o ponteiro acende, o estado que sobrevive à região, o ponteiro sobre os controles, os ganchos do
  quadro, o cursor e as figuras, a página, a altura depois de uma notícia. Juntar cada par. Médio.
- **O build em unidades**: a fase das camadas (`nix/rust/layers.nix` e companhia) só sobrevive pelo
  `.#profile-bench`; os apps que constroem um sistema (`.#apply-here`, `.#vm`, `.#iso`, `.#install`,
  `.#shot`, `.#release`) não são dimensionados; os números do `nix/build-sizing.nix` são de antes do
  shell em crates; as fatias leve e média vêm de uma simulação antiga; a unidade grande espera atrás
  das leves; o `.#rebuild-cost` não simula um `.ftl` nem uma semente; o rustfmt das unidades não
  alcança os `module.rs` do brain; o `graphics-rpath-only-where-graphics-are` pode virar avaliação; o
  `ronoctua-design` lê o WIT de fora do crate; um teste de tempo de parede ao lado de um `rustc`.
- **O shell em crates**: a lista à mão `WITH_ACTION` e o `run-command-handles-every-key` podem sair;
  o `CONCRETE` do rodapé está vazio; o conjunto da base está escrito em quatro checks; a câmera do
  rodapé não alcança o menu da janela, não espera a tabela de ícones e não tem ponteiro (`--hover`).
- **Os valores como dado**: a biblioteca do brain compila o manifesto de todo módulo; a gramática dos
  lembretes ainda é texto compilado; o `mimeDb` lê o `icons.json` inteiro; um glifo desenhado por
  vários tokens mostra um só no navegador de ícones; um glifo de módulo da pessoa que a fonte não tem
  só é recusado no build.
- **O perfil de release** foi medido com o shell num crate só: medir de novo (`.#profile-bench`) e
  decidir se a regra do perfil julga também o tamanho do programa (163,7 MB).
- **O custo por quadro do gerenciador** com o ponteiro sobre uma pasta grande (~83 µs a mais), e o
  `.#bench` imprimindo casas suficientes para a regra de 3% morder.
- **`ronoctua_lookup::keys::write_private_file`** guarda as máquinas pareadas e mora no crate das
  buscas: é do `ronoctua-store`.
- **Checks oferecidos**, cada um com o que pega:
  - `bench-reads-the-renderers-field` (o campo que o `nix/bench-frames.nix` procura é o que o
    renderizador registra);
  - `keymap-dump-is-total`;
  - `profile-variants-stay-off-the-evaluation-path`;
  - `every-binary-carries-its-data`;
  - `the-foundation-stands-on-no-daemon`;
  - o vigia do `.#dev` de pé depois de cada ramo de recarga;
  - `brain-daemon-halves-reexport`;
  - o `chrome-icons-match-the-text` estendido aos glifos do catálogo do brain;
  - o `our-crates-are-built-unit-by-unit` reconhecendo a vendorização pelo conteúdo do `Cargo.lock`.
