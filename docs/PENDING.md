# O que falta

### Syncthing — o que a vista deliberadamente NÃO faz

Tudo abaixo é decisão, não esquecimento. Está aqui para que ninguém o refaça achando que faltou.

- **A metade de uma quantidade.** `minDiskFree` e `minHomeDiskFree` são um objeto com duas
  metades — um número e uma unidade — e a linha move o NÚMERO. Duas metades querem duas linhas, que
  é uma forma que nenhuma outra das 121 opções tem, e o que quase sempre se quer dizer é um número
  maior na unidade que já está lá. A unidade continua sendo a que o Syncthing tem.
- **A ignorância por pasta (`.stignore`).** É um editor de texto dentro de uma vista de rótulo e
  valor, e merece decisão própria — não meia.
- **A chave da API e a senha da interface.** São lidas para assinar um pedido e não entram no
  snapshot, quanto mais numa linha. O `one-syncthing-boundary` cobra.
- **Gráfico e histórico.** A página do próprio Syncthing continua a uma tecla (`s s a`) porque tem
  os dois, e uma lista de rótulo e valor não os substitui. Fingir o contrário seria pior.
- **Renomear uma pasta pelo id.** O Syncthing não muda o id de uma pasta; a linha do id é um FATO,
  e `RET` nela copia.

### A prévia de documento — o que ela deliberadamente NÃO faz

Tudo abaixo é decisão, não esquecimento.

- **Layout nenhum.** Um `.docx` é a descrição de uma página e a prévia não desenha nada dela: sem
  fontes, sem colunas, sem imagens, sem tabela dentro de tabela. Ela responde *o que este documento
  DIZ*, que é a pergunta que se leva a um painel de prévia; o arquivo abre num editor de verdade a
  uma tecla.
- **`.xls` continua vindo de um programa.** O `xls2csv` é o único dos três binários do `catdoc`
  que funciona contra o que o flake pina (364 057 bytes, remedidos), e uma planilha é BIFF — nem
  árvore de registros nem tabela de pedaços, e nada nesta árvore lê uma. O `.doc` e o `.ppt`
  deixaram de precisar de programa: eram esta linha, e viraram `src/ole.rs` + `src/doc.rs` +
  `src/ppt.rs` depois de o dono reportar a mesma frase de recusa duas vezes.
- **Nada de LibreOffice nem de pandoc.** 2-3 GB o primeiro, +297 MiB o segundo (e ele não lê
  `.doc/.xls/.ppt`). A linha que o `nix/modules/media.nix` já traça é que isto é suporte a FORMATO e
  não a aplicativo.
- **`.odg` e os outros zips que ninguém nomeou viram LISTA DE ENTRADAS.** É o que um `.zip` mostra,
  e é honesto: o painel diz o que está lá dentro em vez de dizer que não sabe. Dar texto a um
  desenho é decisão própria, e ninguém a pediu.

### 0.0000001-A. Os pacotes de rua — o que ficou de fora depois de entregues · pequeno

As ruas por região EXISTEM: `LEADER s g` (duas portas), `Work::Atlas` na fila, o leitor de
`.shp`/`.dbf` no `ronoctua-atlas-packs` e o `ronoctua-zip` descido de crate. O que sobrou é menor
que a entrega e fica escrito para não voltar como pergunta:

- **O tamanho do download não aparece ANTES de começar.** Não dá offline: o `index-v1.json` da
  Geofabrik traz id, nome e URL e nenhum tamanho, e a única resposta offline seria inventada. A
  tela diz onde o número aparece — a fila responde o total assim que o servidor o manda, e desistir
  lá custa uma tecla. Medidos: Andorra 6,3 MB, Portugal 803 MB, Sudeste 2 131 MB.
- **A janela da fila diz "baixar" e não "instalar uma região".** O `ronoctua_protocol::Errand`
  atravessa o WIT para um buffer wasip2, então uma variante própria é mudança de WIT, binding
  regerado e rebuild de todos os buffers — por uma palavra. A porta do RODAPÉ, que não é buffer, já
  diz a frase inteira (`transfers-work-region`).
- **Assar um pacote grande come memória proporcional à região.** O baker lê a camada de vias
  inteira (teto de 2 GiB por membro do zip) e monta os segmentos em RAM antes de escrever. Medido
  só em Andorra; Portugal e o Sudeste não foram assados aqui. Se doer, o conserto é escrever os
  segmentos em disco por célula e só então montar o índice.
- **Nada consulta a rede sozinho, nem para saber se há pacote novo** — e isso é decisão, não
  pendência: é uma tecla (`RET` na região de novo), nunca um relógio.

### 0.0000001-B. O globo — o que ele deliberadamente NÃO faz · registro

Tudo abaixo é decisão, não esquecimento.

- **A base para em 611 m**, que é o passo da quantização de 4 bytes por vértice. Sem pacote de ruas,
  aproximar além de `Pose::LOCAL` só mostraria a escada da grade, e a linha do endereço diz até onde
  ela consegue ir em vez de desenhar uma rua que não existe.
- **Não há busca por endereço.** O globo responde *onde é isto*, nunca *onde fica a Rua Augusta* —
  uma busca quer índice sobre nomes, tokenizador e ranqueamento, que é um segundo produto.
- **Uma projeção só**, ortográfica, sem alternativa plana: o dono pediu um globo.
- **Sem relevo, sem nome escrito no globo.** O `resvg` desta árvore está sem motor de texto
  (`default-features = false`), então texto sobre a figura teria de vir numa segunda passada pelo
  `ronoctua-render`.
- **E o `ronoctua-atlas` NÃO está no invariante 12.** Ele é `no_std`, sem dependência e capaz de
  `wasm32-wasip2` — o `atlas-crate-is-pure` cobra as três coisas —, mas acrescentar um quarto crate
  àquela frase é **editar um invariante**, e isso é do dono. Um buffer que quisesse o globo emitiria
  sete `command::svg` (um por `Ink`, um token cada, sem mudar o WIT) com a malha de 110 m, sem ruas
  e sem ponteiro, porque buffer não tem disco nem recebe valor de token.

### 0.0000001-C. As miniaturas — o que elas deliberadamente NÃO fazem · registro

Entregues no modo grade (`C-g`). O que ficou de fora, com o preço de cada um:

**Miniatura de pasta como mosaico do conteúdo.** Seriam N decodificações por pasta, e uma pasta de
pastas seria N². A pasta fica com o glifo dela.

**Documento de escritório** (`.odt`, `.docx`, `.ppt`, `.odp`) — não há rasterizador para eles na
árvore; só o PDF tem (`pdftoppm`). Ficam com o glifo do tipo.

**Cobertura da onda e da capa.** O que o shell PINTA fica no nosso cache e não no do freedesktop —
a onda de um áudio e uma peça 3D mudam com o tema, e o padrão não tem onde dizer isso. A conta é
feita pelo TIPO, antes de qualquer leitura, então a CAPA de um áudio (que é do arquivo e não muda com
o tema) fica no nosso também. Custa não reaproveitar a capa que outro gerenciador já extraiu; o que
custaria consertar é saber capa-de-onda antes de decodificar, que hoje só se sabe depois.
**Como saber que acabou:** um `.mp3` com capa aparecendo em `thumbnails/normal/`.

**Arrastar-e-soltar e seleção por retângulo FORAM FEITOS** (a pedido do dono, 2026-09-21), nas duas
formas da listagem e entre janelas e programas estrangeiros. O que ficou de fora é decisão, não
esquecimento:

- **Renomear pelo clique** — o segundo clique num nome já selecionado. Ele colide com o gesto que
  ABRE: os dois são "clicar numa coisa já escolhida", separados só por um relógio, e um relógio é a
  diferença entre abrir um arquivo e começar a editar o nome dele. `C-r` renomeia, e pergunta.
- **A FIGURA que acompanha o ponteiro** (`DragSource::start_drag` recebe `None` como ícone). Ela
  seria uma segunda superfície Wayland, desenhada pelo nosso renderizador, viva só durante o gesto —
  e o que ela diria já está na tela: a pasta ACESA diz onde o drop vai, que é a pergunta que o
  arrasto levanta. O compositor desenha o cursor de "copiar" ou "mover" por conta dele.
- **Rolar ao encostar na borda** durante o arrasto. Precisa de um relógio armado — a coisa que esta
  árvore evita em toda porta — e o que ele compraria é alcançar uma pasta fora da tela sem soltar;
  soltar, rolar e pegar de novo faz o mesmo em dois gestos.
- **O retângulo que ALTERNA em vez de tomar.** Segurar Ctrl e varrer inverteria cada item coberto;
  hoje `Ctrl` acumula sobre o que já estava marcado, que é o que a varredura precisa para somar duas
  passadas. Alternar por varredura é um gesto que se lê errado quando a mão passa duas vezes pelo
  mesmo item.

### O brain refeito — o que ficou de fora, e por quê · registro

Tudo abaixo é decisão, não esquecimento.

**O passo root não obedece ao interruptor da página.** Os seis passos da metade ROOT do update
continuam decididos por `ronoctua.update.steps`, que precisa de rebuild, e a página os desenha
RECUSANDO com a frase que diz de onde vem o valor. A camada da pessoa vive em
`$XDG_STATE_HOME/ronoctua/update-steps.conf`, e a metade root roda como root: `/root` não é a casa
de ninguém e não há como ela descobrir de quem foi a sessão que a iniciou. As formas de dar-lhe uma
acabam todas num diretório de root que um membro de `users` pode escrever — numa máquina
compartilhada, qualquer conta decidindo o que um update de root roda.
**Como saber que acabou:** `m t` num passo root mudando o que a unidade executa, sem que nenhum
diretório de root tenha virado gravável por grupo.

## Regra de manutenção — leia antes de mexer

**Ao concluir um item daqui, remova-o.** Se ele mudar *como o sistema funciona*, registre o porquê
no `docs/ARCHITECTURE.md`. O `AGENTS.md` só recebe o que vale para qualquer trabalho — um
invariante, uma regra transversal, uma armadilha —, nunca o detalhe de uma feature. Se não mudar
nada de fundo — é só trabalho feito —, **apenas apague**.

Esta lista não guarda histórico. Item riscado, marcado como pronto ou mantido "para registro" a
transforma em changelog — e aí ninguém mais confia que o que está escrito ainda falta. Há um check
que recusa esses marcadores.

Todo item traz **como saber que acabou** e o **tamanho** aproximado. Um enum de tokens e uma
reescrita do protocolo são coisas muito diferentes, e misturá-las esconde isso.

---

## Funcionalidade especificada e ausente

### 0.0000028. A folga do disco — o que nem os testes nem as VMs escritas alcançam · pequeno cada

A folga (`docs/ARCHITECTURE.md`, "Quando o disco aperta", e o `why.md` do módulo `disk-space`) foi
provada por teste com árvores plantadas, por sabotagem e por um ensaio lido num disco real; as VMs
escritas (`disk-space-vm`, `store-vm`, `cache-subvolume-vm`) rodam na bateria final. Fica de fora
delas:

- **O cérebro disparando a folga sozinho.** A VM a pede pela unidade; a costura inteira — cérebro →
  unidade → relatório → o evento `disk-space-relieved` no anel `Mind.events` → a frase no
  minibuffer, e `disk-space-still-short` quando nada mais sai — tem teste por pedaço, não junta.
- **A marca de "quase cheio" na modeline**, que só uma cena do `.#shot` fotografa (um disco pequeno
  enchido até abaixo da linha).
- **A montagem por bind dentro do cache.** `STATX_ATTR_MOUNT_ROOT` foi provado em `/` e num
  diretório comum; um bind dentro do cache precisa de root e de um subteste que o plante.
- acabou quando: a foto da marca e a da linha "espaço aberto no disco" estão numa cena do `.#shot`,
  e um subteste planta um bind no cache e roda as duas varreduras

### 0.0000029. O rodapé transparente — o que só a GPU do dono prova · minúsculo

Nos modos ocultos (um toque de `Ctrl` na raiz da leader gira normal → empurra → cobre;
`docs/ARCHITECTURE.md`, "Os modos ocultos"), o que empurra as janelas fica sobre a cor da mesa, opaca, e não depende de nada. O que
depende de a superfície do rodapé aceitar o modo `PreMultiplied`
(`ronoctua_render::gpu::alpha_mode_for`) é o que fica POR CIMA das janelas: o véu (o minibuffer
sozinho e a lista da leader nos dois modos, e tudo no `Cover`) e o pixel transparente da borda em
repouso. Os testes provam a escolha do modo e a limpeza multiplicada; se o driver da máquina do dono
oferece o modo, só ela responde. Sem ele o rodapé fica opaco, registra
`the surface cannot be see-through here` e diz `echo-footer-opaque` uma vez: as barras somem e fica
uma faixa de chrome.

- **como saber que acabou**: na máquina do dono, no modo que empurra o minibuffer sozinho (um eco)
  e no que cobre uma vista aberta mostram a janela através do véu, a linha de baixo da
  tela em repouso mostra a janela e não uma faixa de chrome, e nem a frase nem o aviso no journal
  aparecem. Se o driver recusar, a decisão do dono sobre isso fica escrita no `ARCHITECTURE.md` e o
  item sai.

### 0.000003. Os programas da pessoa — o que só uma máquina de verdade prova · pequeno cada

A central (`LEADER i`, `docs/ARCHITECTURE.md`, "Programas") foi provada por teste, por palco e por
avaliação; estas coisas não:

- **O Waydroid rodando**: preparar a imagem GAPPS, o `android_id` lido pelo `sqlite3` de dentro
  dela, a sessão, instalar um APK do F-Droid. Tudo foi escrito contra a fonte do waydroid 1.6.3 e
  testado com um `waydroid` falso; a VM não roda um Android útil.
- **O inibidor de atalhos num web app de verdade.** `for_window [app_id="^chrome-"]
  shortcuts_inhibitor disable` é gerado, mas o `--validate` não interpreta comando dentro de
  `for_window` (uma linha com comando inventado também passa). Só uma janela `--app` do Chrome 151
  prova que a leader continua chegando.
- **A ativação do nix-flatpak** instalando a extensão escura e escrevendo o override global dos
  `QT_*`: o que o experimento mediu foi o arquivo que o `flatpak override` escreve; a declaração
  foi conferida por avaliação.
- **Um Flatpak Electron na sessão real.** No palco o FreeTube procurava um servidor X sem
  `--ozone-platform=wayland`; na sessão há Xwayland, e falta ver qual dos dois ele usa e se o
  escuro chega igual.
- **O F-Droid guarda no catálogo compacto só nome e resumo**: a ficha de um app Android não tem
  descrição longa nem permissões. Guardar as duas por idioma é o que custa (o catálogo tem 5,47 MB).
- **O caminho de atributos assinados da verificação do F-Droid não tem teste**: a fixture era uma
  chave privada RSA de teste, e ela não entra num repositório público. O `entry.jar` de hoje não usa
  esse caminho.

### 0.0000031. Flatpak e app Android da distro não têm nome no registro · pequeno

O `name` do `distro-apps.json` vem da tabela só para AppImage e web app, e da entrada para um
pacote; a linha de um Flatpak ou de um app Android da distro não leva nome, e um deles recusado
antes de ser instalado apareceria na central pelo id. A tabela não tem nenhum hoje. Acabou quando:
as duas formas aceitam `name` e o `distro-app-sources-are-checked` o planta.

### 0.0000035. O que a câmera ainda não faz · pequeno a médio cada

- **Câmera MIPI atrás do libcamera** (muitos laptops novos com sensor Intel IPU): ela não é um nó
  V4L2 de captura comum e não aparece. Acaba quando uma dessas é fonte, provada numa máquina que a
  tem; uma ponte pelo nó libcamera do PipeWire passaria pelo `ronoctua.camera.pipewireCameras`,
  desligado por causa da luz (`docs/ARCHITECTURE.md`, "A câmera e o estúdio"). Médio.
- **Câmera só multiplanar não grava.** A varredura lê os formatos dela (`VIDEO_CAPTURE_MPLANE`), e
  tamanhos e taxas aparecem na vista ao lado da frase `camera-about-cannot-stream`; quem não a abre
  é a entrada V4L2 do ffmpeg, que só fala plano único. Acaba com a mesma ponte do item de cima (o
  libcamera, ou um `v4l2-relayd` declarado). Médio.
- **O `v4l_id` do udev abre cada câmera a cada login.** No instante em que o logind abre a sessão, o
  udev processa um `change` em cada nó de vídeo, e a regra `60-persistent-v4l.rules` roda o
  `v4l_id`, que abre o nó para um `QUERYCAP` (o `camera-vm` mede uma abertura por nó, e a aceita só
  contada contra os eventos que o udev processou depois de o vigia estar no lugar). Se esse
  `QUERYCAP` acende a luz de uma webcam UVC não foi medido: a VM não tem uma, e só o dono mede, na
  webcam do laptop, com os olhos no LED dela. Um login: reiniciar e olhar o LED no instante em que a
  sessão entra sozinha (o autologin do tty1) — um piscar ali é o `v4l_id`, porque nada mais da
  sessão abre câmera ao subir. E o mesmo evento sem reiniciar, quantas vezes for preciso:
  `sudo udevadm trigger --action=change --subsystem-match=video4linux --settle`, que faz o udev
  rodar a regra em cada nó como no login. Acaba com a medida e, se acender, com a decisão do dono
  sobre a regra, que também dá as propriedades `ID_V4L_*` que outros leem. Pequeno.
- **Um Flatpak que só lê câmera pelo portal não vê nenhuma, e nada na tela diz por quê.** Com
  `ronoctua.camera.pipewireCameras` desligado (o padrão, por causa da luz), o portal de câmera
  responde que não há câmera, e o programa diz isso ou fica calado. A central de programas, na ficha
  de um Flatpak, ou os ajustes da câmera poderiam dizer o motivo. Pequeno.
- **O recuo para `wlr-screencopy` nunca rodou contra um compositor de verdade.** Ele existe
  (`ronoctua-studio`, `screen.rs`) e a conversão de pixels é testada, mas o pinado anuncia o
  `ext-image-copy-capture-v1`, então só outro compositor o exercita. Pequeno.

### 0.0000036. O Chrome quiosque — o que só o Chrome de verdade, numa máquina, prova · pequeno cada

O quiosque (`LEADER m b`, `docs/ARCHITECTURE.md`, "O Chrome quiosque") foi medido numa VM com o
Chrome 151 real, offline, e o `kiosk-vm` repete o que importa. Isto a VM não mostra:

- **Vídeo HTML5 em tela cheia dentro de um quiosque** (E2): a tela cheia de verdade acabando tem de
  devolver as barras escondidas, e a guarda "aguardando fullscreen" (`kiosk.rs`) não pode tomar o
  relato do vídeo por um F11.
- **O popup de outra extensão aberto numa aba** (E7), que é o que `RET` faz na lista de extensões:
  falta ver quais popups funcionam fora da bolha.
- **Onde o Chrome 151 guarda `extensions.pinned_extensions`** (E8). Na VM o `Preferences` ainda não
  existia quando o host leu; o host lê o `Preferences` e depois o `Secure Preferences`, e falta ver
  em qual deles uma fixação de verdade cai.
- **`search.query` numa janela em tela cheia** (E10): a busca da linha de endereço abrindo na aba
  certa.
- **Sessão restaurada com um quiosque aberto** (E11): o Chrome reabre a janela sem o rodapé ter
  pedido, e falta ver o que ela vira.
- **`--new-window` com "continuar de onde parou"** ligado no perfil (E12).
- **O tempo da primeira instalação forçada num perfil com uso** (E14). Na VM, com perfil novo,
  foram uns 3 s.
- **Os 8 px a mais de `innerWidth`** na tela cheia de aplicativo (520 num tile de 512): se é a
  borda do compositor contada pelo Chrome, e se corta alguma coisa da página.
- **O quiosque como PRIMEIRA partida do Chrome num perfil novo**: a extensão ainda não foi
  instalada quando a linha de comando chega, e a página `kiosk.html` falha.

- acabou quando: cada item foi visto no hardware do dono e virou frase no `ARCHITECTURE.md`, ou
  conserto

### 0.0000037. O VS Code — o que só uma sessão de verdade mede · pequeno cada

O VS Code (`docs/ARCHITECTURE.md`, "VS Code: a build da Microsoft…") foi medido no `vscode-vm`, contra
substitutos. Fica o que só uma sessão de verdade, com a rede de verdade, diz:

- **Um `SIGTRAP` quando a unidade dele é parada.** Na VM, `systemctl --user stop` da unidade que o
  roda termina o VS Code num `trap int3`, com um core de ~3,6 GB por partida — o bastante para encher
  o disco da VM, e o `systemd-coredump` chegou a 3 GiB de memória; o teste não guarda core nenhum
  (`Storage=none`). Medir numa sessão Wayland: o fim da sessão (`s q q`) com o VS Code aberto deixa
  um core? Se deixa, cada saída guarda gigabytes.
- **O Copilot Chat embutido, depois de uma troca do VS Code.** A cópia que o Marketplace atualizou
  fica em `~/.vscode/extensions` (fora da lista da pessoa); quando a distro move o VS Code e o
  embutido do pacote passa a ser mais novo, a 1.119 diz "Removing stale auto-update builtin
  extensions". Ver numa máquina que isso acontece sem nada da pessoa.
- **O terminal integrado vê `ID=linux`** (o ambiente FHS se diz assim para o rust-analyzer não se
  remendar). Nada da distro lê o `ID` ali; ver se algo que a pessoa usa no terminal do VS Code lê.

- acabou quando: cada item foi visto numa sessão de verdade e virou frase no `ARCHITECTURE.md`, ou
  conserto

### 0.0000039. As colunas: o que ficou para depois · pequeno

- **A coluna em duas telas de verdade**: que cada tela mostra a coluna do seu espaço, e que uma tela
  que some estaciona a coluna. Nenhum teste a cobre (o subteste de duas saídas do `player-vm` é da
  janela cheia, não da coluna).

- acabou quando: o teste de duas telas da coluna existe

### 0.00000395. Os checks oferecidos do redimensionador, das colunas e do quiosque · pequeno

- `native-hosts-take-chromes-origin` — ESCRITO e registrado, nunca construído: ele precisa dos
  binários `rust.transfers` e `rust.browser`, e a leva não construía os programas no Nix. Falta
  rodá-lo, quebrá-lo de propósito e ver a mensagem.

- acabou quando: ele foi quebrado de propósito, a mensagem exata foi vista e o arquivo restaurado

### 0.000005. O que a exibição de escritas ainda não alcança · pequeno cada

A cadeia de fallback existe, 37 escritas desenham e a régua da célula conhece a largura leste-
asiática (ver `docs/ARCHITECTURE.md`, "Duas cadeias de fallback"). Três coisas ficam de fora, e
nenhuma é um defeito escondido: são limites medidos.

- **A tinta do CJK é mais estreita que a célula reservada.** Medido: o ideograma anda 1,667 célula
  das 2 que a régua dá, e o hangul 1,533. Nada se sobrepõe — sobra vão, e uma linha de japonês fica
  com entreletra folgada. Fechar exige reescalar as cinco faces `NotoSansMonoCJK` para `upem 833`
  mexendo em `head` E no `FontMatrix` do `CFF2`, porque com `CFF2` os contornos têm escala própria.
  **Como saber que acabou:** o `nix run .#rust.bins.ronoctua-test-faces` imprimir 2,000 para o
  ideograma, e uma foto de um parágrafo em japonês ao lado do mesmo em latim mostrar a mesma cor de
  página.

- **Não dá para DIGITAR japonês, só exibir.** Não há `zwp_text_input_v3` nem
  `zwp_input_method_v2` em lugar nenhum da árvore, então não há pré-edição nem string de commit;
  e o `Input::Char` do WIT é um `char`, então `buffer.rs` toma `utf8?.chars().next()?` e joga
  fora o resto. Com IME isso perderia a composição inteira. O pedido do dono era exibição, e é o
  que existe. **Como saber que acabou:** um IME de verdade (fcitx5, ibus) compondo numa janela
  nossa, com a pré-edição desenhada.

- **RTL se reordena DENTRO do span, e o layout à volta é da esquerda para a direita.** O
  cosmic-text roda o algoritmo bidi em cada `Buffer`, que é o que faz o hebraico do clipboard
  sair na ordem certa. O que não existe é uma tela cuja direção seja RTL — rótulo à direita,
  valor à esquerda —, e nem precisa: os dois idiomas declarados são LTR. **Como saber que
  acabou:** só se um idioma RTL for declarado; até lá, isto é registro do limite, não tarefa.

### 0.00001. O tamanho do texto: o que ele ainda não alcança · pequeno cada

O texto do shell anda de 70% a 200%, **tudo** que o host desenha anda junto, as dez portas ouvem o
arquivo mudar, e desde 2026-09-07 o tamanho **sai do shell**: ele viaja na DESCRIÇÃO DA FONTE
(`font-name`, `monospace-font-name`, `document-font-name`), que é o que GTK e Qt leem, e o Ghostty
ganha arquivo próprio mais um `SIGUSR2`. Duas coisas ficam de fora.

- **GTK e Qt seguem a descrição da fonte, e isso foi lido nos binários, não numa tela.** O
  `qgnomeplatform` 0.8.4 assina `changed::font-name` e `changed::monospace-font-name` (e NÃO
  `text-scaling-factor` — foi por isso que o canal mudou); o `libgtk` lê as duas. Mas nenhum teste
  daqui abre um app GTK ou Qt: o que existe é a evidência das strings. **Como saber que acabou:**
  abrir um de cada numa sessão com portal, mudar o tamanho, e medir — e escrever na tela o que segue
  ao vivo e o que espera redesenhar, em vez de prometer.

- **O Chromium e o Electron não foram medidos.** Eles leem o portal; se leem a descrição da fonte ou
  só o multiplicador que agora é zerado, ninguém aqui verificou. **Como saber que acabou:** o mesmo
  método acima, e se seguirem só o multiplicador, eles entram na frase da tela como exceção
  nomeada.

O que ESTÁ medido de ponta a ponta é o Ghostty: o `ghostty-config-includes-theme` deita o
`ghostty-size.conf` e pergunta ao binário a configuração EFETIVA, que respondeu `11 -> 22` no teto
da faixa. É o único aplicativo estrangeiro que este repositório consegue interrogar sem sessão.

### 0.0000104. O que o som ainda não faz · pequeno cada

O `LEADER s a` cobre o que a máquina toca, o que ela ouve, o que cada programa faz com os dois, e a
placa por trás disso. Quatro coisas ficaram de fora, e cada uma tem uma razão:

- **Um programa não se manda para outro aparelho.** A linha de um fluxo mexe no volume e no mudo
  dele, e não o move do alto-falante para o fone. O PipeWire faz isso por metadata (`target.object`)
  e não por `wpctl`, então seria uma segunda forma de escrever no grafo — a fronteira hoje escreve
  por um programa só, e essa é a linha de segurança que ela tem.
- **Nada é gravado em arquivo.** O `pw-cat --record` já está lá, e apontá-lo para um arquivo em vez
  de para o medidor seria uma feature de captura: um lugar onde escrever, um nome, um formato, uma
  fila. É outra obra e ninguém a pediu.
- **Não há equalizador nem balanço.** O `channelMap` e os `channelVolumes` por canal estão lidos e
  guardados no `Node`; o que falta é a tela. O balanço é o mais próximo de existir — são dois
  números que já chegam.
- **O `pro-audio` aparece como o PipeWire o dá.** É um perfil na lista e gira como qualquer outro;
  o que ele significa (canais crus, sem conversão) não ganha frase própria.

- acabou quando: alguém pedir um deles, e não antes

### 0.000014. Os DESENHOS nascem em pixel físico — falta a foto em escala 2 · pequeno

O ícone do localizador, os painéis do gerenciador e o `Command::Svg` dos buffers nascem agora no
pixel FÍSICO, com a escala na chave do cache (`hidpi.rs` diz onde cada um); a prévia do clipboard é
a imagem copiada no tamanho dela, só reduzida. O que o item pedia como prova
ainda não existe: nenhuma foto foi tirada numa saída em `scale 2`, e o `render-png` não desenha
vetor (nenhum buffer usa `svg`, que está atrás da trava `vector`), então o `Command::Svg` só tem
teste de unidade.

**Como saber que acabou:** uma foto do localizador numa saída em `scale 2` com o ícone e a letra ao
lado, e o `render-png --scale 2` de um buffer com `Command::Svg`.

### 0.000015. A escala fracionária é arredondada pelo compositor · pequeno, e é uma decisão

As dez portas seguem a escala da tela (`hidpi.rs`, check `every-window-knows-its-scale`), e a
escala que elas seguem é a INTEIRA: `wl_surface::set_buffer_scale` só aceita inteiro. Numa saída que
o compositor dirige em 1,5, ele diz 2 ao cliente e reduz o resultado — o que é nítido, e é uma
reamostragem a mais.

O protocolo tem a resposta (`wp_fractional_scale_v1` + `wp_viewporter`): o cliente pede a escala
fracionária, desenha nela e diz por `wp_viewport` que tamanho lógico aquele buffer ocupa. São dois
globals novos em dez portas, e a aritmética de `hidpi::physical` deixa de ser inteira.

**Não foi feito porque não foi medido**: a diferença entre "2 e o compositor reduz" e "1,5 exato" é
sub-pixel numa fonte monoespaçada, e o dono não tem monitor em escala fracionária para fotografar a
diferença. **Como saber que acabou:** uma foto dos dois caminhos na mesma saída em 1,5, e o número
de energia de borda que a nota do `hidpi.rs` já usa; se a diferença não aparecer, a decisão vira
"fica inteiro" escrita no `hidpi.rs`, e este item sai.

### 0.000017. O keymap do aninhado espera UMA medição para poder sumir · minúsculo

`nix/keyboard-nested.nix` tira o Super do keymap do laço aninhado, e existe por um defeito medido:
um modificador que descia dentro da janela e subia fora dela ficava preso para sempre, e como um
bind é casado por igualdade, um bit sobrando matava todos os atalhos da sessão.

**O que tornava isso incurável era o estado ser CONTADO** — pressões contra soltas —, e o backend
Wayland do wlroots não conta: ele implementa `.modifiers` e passa a máscara do HOST direto
(`wlr_keyboard_notify_modifiers`, `backend/wayland/seat.c`), e o handler de tecla manda
`update_state = false`. O estado aqui é ESPELHO do de lá, nunca derivado, e o host reenvia a máscara
a cada troca de foco.

Isso é LEITURA, não medição, e é por isso que o arquivo continua de pé. **A medição é um gesto**: no
`nix run .#dev`, apertar um atalho do host construído sobre Super, deixar o host levar a janela,
voltar, e tentar a leader. Se ela abrir, somem três coisas — este arquivo, o check
`nested-survives-a-stray-modifier` e o segundo bind de leader com um Shift que ninguém apertou.

- acabou quando: o gesto foi feito e a leader abriu, ou não abriu e o arquivo ganhou a nova medição

### 0.000017c. Espaço de trabalho VAZIO não existe mais · minúsculo, e é uma perda declarada

`LEADER TAB 1..9` continua significando sempre a mesma coisa: o comando é `workspace number N`, que
nomeia o número e cria o espaço se ele não existir. O que se perdeu é o espaço vazio APARECER — este
compositor não tem espaço persistente nem pré-criação (conferido no `scroll.5`: há `workspace <nome>
output/gaps/layout_default_mode/exec` e nenhuma forma de declarar um que exista vazio), e um `exec`
visitando 1..9 na partida não resolve, porque um espaço vazio é destruído assim que se sai dele.

Na prática: a visão geral mostra os espaços com janela, e os outros aparecem quando alguém põe algo
neles. Recriá-los exigiria um daemon nosso segurando nove espaços abertos, o que é desproporcional ao
que se ganha.

- acabou quando: o dono disser que faz falta, ou o compositor ganhar espaço persistente

### 0.00002. A luz noturna e o espelhamento de telas · médio o primeiro, nenhum o segundo

A vista de telas mexe em modo, taxa, escala, rotação, posição, VRR e brilho. Duas coisas que as
pessoas esperam de uma tela ficaram de fora, e por motivos diferentes.

**Luz noturna** (temperatura de cor por horário). Dá para fazer: o compositor registra o
`wlr_gamma_control_manager_v1` e liga a cena a ele (`sway/server.c`). Só que quem aplica a rampa é o
backend, e fora do DRM não há rampa a aplicar — o loop aninhado e todo teste deste repositório rodam
em `x11` ou `headless`. Ou seja: dá para escrever, não dá para **provar**, e este repositório não
chama de pronto o que não fotografou. O dono decidiu deixar de fora sabendo disso, e a troca de
compositor não mudou nada disso — **nem foi remedido**, o que está escrito aqui para não virar
afirmação.

**Como saber que acabou:** um cliente `wlr-gamma-control` nosso, a rampa calculada da temperatura em
kelvin, hora de início e fim na ficha da tela — e a prova na máquina do dono, por olho, dita como
prova por olho no relatório.

**Espelhamento.** Não é adiamento, é ausência do compositor, e continua sendo depois da troca: não
há `mirror` nem `clone` entre os subcomandos de `output` (conferido no `scroll-output.5`). Uma linha
na tela oferecendo espelhar seria um interruptor que não interrompe — que é justamente o que o check
`switches-that-do-nothing-say-so` existe para recusar. **Como saber que acabou:** o dia em que o
compositor fixado tiver como espelhar, a linha entra na ficha de um monitor, ao lado da rotação.

### 0.00003. A profundidade de bits não volta lida do compositor · minúsculo

A linha está na ficha de um monitor (6, 8, 10 ou automática), escrita no fragmento como as outras
(`render_bit_depth`) e relida pelo `Compositor::reread`. O que o item pedia e não existe é a leitura
de volta: o IPC do Scroll fixado não informa a profundidade (o `ipc-json.c` não tem o campo), então
a linha diz o que está ARQUIVADO e diz que o compositor não confirma — um monitor que não aceita 10
é desenhado em outra profundidade (`scroll-output.5`) sem que a vista saiba.

**Como saber que acabou:** o dia em que o compositor fixado informar a profundidade de uma saída, a
linha mostra o valor lido ao lado do pedido.

### 0.00004. A rede: o que ainda falta · pequeno cada

A vista da rede está inteira: uma `View` com duas portas (`s n n` e `s n w`), dez seções com
prateleiras de altura fixa, anel de `TAB`, dica GERADA da `OF_THE_VIEW`, cinco marcas na modeline
(a da Tailscale desenhada por nós), VPN com prévia antes de importar, Tailscale, Tor, portas,
monitor, quem está na rede (com nome, IPv6 e a latência do roteador sob pedido), redes lembradas,
hotspot com a senha nas duas portas, e VPN que guarda a senha. O que ficou de fora tem motivo, e
cada um é pequeno:

- **Hotspot na TELA do dono.** Está implementado e provado por teste unitário (a linha oferece o
  outro estado, diz o que custa e recusa com o rádio desligado), mas nunca foi acionado: ligá-lo
  derruba o Wi-Fi da máquina de quem está lendo isto. O ar simulado já existe (`tests/wifi-air.nix`,
  com `mac80211_hwsim`); falta um check do hotspot sobre ele.
- **Um emoji num rótulo come uma célula do ar.** `layout::width` conta CLUSTERES e o avanço de um
  emoji é 1,245 em contra os 0,600 da célula — quase duas. A coluna do valor é calculada com essa
  régua, então um SSID com um emoji desenha uma célula mais larga do que foi medido. As duas células
  de ar de `chrome::field_pair` absorvem UM; com dois, o rótulo encosta no valor. Trocar a régua é
  trocá-la em todo o sistema — é a mesma função que corta em toda parte —, e o preço é maior do que
  o defeito.
- **Prateleira mais alta do que a região não tem barra.** Num rodapé cujo teto (`expansion.maxLines`
  contra `maxFooterPct`) fica abaixo de `space.shelf.lines`, nenhuma prateleira cabe inteira, então
  nenhuma ganha barra. Teclado e roda continuam alcançando as linhas: a seta na borda rola a
  prateleira e o texto sob o cursor muda, que é o sinal.
- **A prateleira só é arrastável quando cabe INTEIRA na tela.** A barra de uma prateleira meio fora
  da região precisaria de um trilho mais curto do que a faixa, e arrastar esse trilho moveria a
  lista por uma quantidade diferente do arrasto. Quando ela está meio fora, quem está se movendo é a
  REGIÃO, e a barra da região já diz isso. Uma prateleira que atravessa a borda continua rolando por
  teclado e por roda.

- acabou quando: cada item acima ou existe, ou está escrito aqui com o motivo de não existir

### 0.00006. O Bluetooth deixa duas coisas do BlueZ de fora, de propósito · registro

O detalhe do dispositivo mostra e deixa mudar tudo o que o `Device1` tem — inclusive o `WakeAllowed`
e o perfil de áudio (este por PipeWire, ver `ronoctua_bluetooth::audio`), e a internet de um
celular (PAN, `org.bluez.Network1`). Faltam duas interfaces, e cada uma é uma decisão e não um
esquecimento:

- **`org.bluez.MediaPlayer1`/`MediaControl1`** (tocar, pausar, faixa de um celular ou caixa). Fora
  **de propósito**: o PipeWire já publica isso como MPRIS, o `playerctl` fala MPRIS, e uma segunda
  implementação aqui seria uma segunda resposta para "o que está tocando" — a mesma razão pela qual
  o crate não reimplementa layout de janela.
- **`ManufacturerData`, `ServiceData`, `AdvertisingFlags`, `Sets`, `CablePairing`.** Bytes crus de
  anúncio; o `Sets` (LE Audio) e o `CablePairing` estão em toda máquina moderna. São FATOS sem frase
  possível hoje: mostrar `0x004C: 0215...` é encher a tela com o que ninguém lê.

- acabou quando: é registro; sai quando uma das duas deixar de ser decisão

### 0.0001. O realce de sintaxe deixa duas coisas de fora, de propósito · pequeno cada

Não são esquecimentos; cada uma tem um motivo estrutural escrito no `ARCHITECTURE`, e estão aqui para
que ninguém as descubra achando que achou um defeito.

- **Heredoc** (`<<EOF`, `<<'PY'`). O terminador é texto arbitrário descoberto em tempo de execução, e
  o `State` que atravessa linhas cabe em oito bytes de propósito — texto não cabe lá. Custaria o
  `Copy` e poria uma alocação no orçamento do frame.
- **Título setext** do markdown (o sublinhado de `===`). Precisaria do conteúdo da linha ANTERIOR, e
  o desenho inteiro é "uma linha mais oito bytes".

- acabou quando: o item que se resolveu sai desta lista; nenhum deles muda como o sistema funciona,
  então nenhum precisa de linha no `AGENTS.md`

### 0.0003. A linha do tempo do miniplayer não está numa foto · pequeno, e tem preço

As fotos provam o player inteiro — o filme anda (seis amostras nomeiam qual quadro está na tela), o
`C-;` segura (três amostras idênticas), a imagem sai 130x97 (1,34, o 4:3 do arquivo; sob o defeito
seria uma faixa de ~16:1) e os controles chegam com o ponteiro (0 pixel de glifo sem ele, 29 com).

O que elas NÃO provam é a linha do tempo, e por um motivo de projeto e não de descuido: elas usam um
**GIF**, que o crate `image` decodifica em processo — o palco das fotos não tem grupo de mídia nenhum. E um
GIF não tem duração, então não ganha linha do tempo (buscar num GIF é compor desde o primeiro
quadro: 707 ms no arquivo do dono).

Provar a linha do tempo pede um **vídeo**, que pede `ffmpeg-full` DENTRO da VM das fotos, mais as
duas variáveis que o `nix/modules/media.nix` põe na sessão. Isso já existe: o rodapé do palco leva
`RONOCTUA_FFMPEG` e `RONOCTUA_FFPROBE`, e `MEDIA` tem o `film.mp4`. Falta a cena.

O preço já foi pago, e é maior do que o ffmpeg: medido com `nix path-info -S` no driver do `.#shot`,
o palco do player somou **+198 MiB** (de 5,2 para 5,4 GiB, 33 caminhos). O ffmpeg é 1,2 MiB (só a
saída `bin`; a `lib` e a `data` já vinham pelo mpv), a mídia 9,9 MiB e o daemon 6,9 MiB; os outros
~180 MiB são a tabela de decodificadores (`nix/player-tools.nix`: o zxtune e o Qt5 que ele puxa,
~130 MiB; a soundfont, 31 MiB; vgmstream, uade…), mais uma segunda avaliação do nixpkgs para o único
decodificador não livre. As cenas de `film.mp4` e `subbed.mkv` não usam a tabela: o mpv decodifica os
dois sozinho. Por isso (decisão delegada C4, 2026-10-04) a tabela saiu do palco padrão: só o
`nix run .#shot -- --player-tools <cena>` a constrói, e só lá `start_player(tools=True)` a entrega ao
daemon.

- acabou quando: uma cena do `.#shot` arrasta o cursor da linha do tempo e mede que ele andou

### 0.00031. O que a câmera das portas e o palco do player ainda não provaram · pequeno cada

O `ronoctua-shell photograph` e o palco do player no `.#shot` entraram juntos, e três provas pediam
VM numa máquina com menos de 25 GB livres — então não foram feitas:

- **A foto contra a tela.** A janela e a coluna do player, fotografadas pelos dois caminhos — a
  câmera e o `.#shot` com um `film.mp4` pausado num segundo conhecido, o `look("nome")` e o
  `frame_png` do mesmo instante dados à câmera — e toda diferença explicada. Hoje o que se sabe é que
  a câmera chama a mesma `door::compose` que a porta chama.
- **A legenda do libass no quadro do daemon.** O único pacote de legenda do `subbed.mkv` ficava no
  byte 776 462 de 1 147 591, retido pelo `-shortest`, e nenhum quadro do daemon mostrou o "hello".
  Sem o `-shortest` (`tests/player-media.nix`), o mpv pinado com `--vo=image` desenha o "hello" em 1 s
  e em 3 s, e o arquivo antigo, nos mesmos instantes, não desenha — mas o arquivo testado foi feito à
  mão com o mesmo comando, a derivação não foi construída e nada foi refotografado na VM. O
  `player-vm` continua sem olhar um pixel da legenda.
- **Duas telas numa foto.** O `start_compositor(outputs=2)` sobe as duas, mas as duas janelas do X
  ficam empilhadas em `+0+0`, o `shot()` não recebe `output=`, e qual nome de saída cada janela do X
  carrega não foi medido.
- **A coluna de mídia do rodapé na tela** (`player_media`). A câmera não alcança o rodapé, e o que
  existe são testes da geometria (o degrau por largura, o pé do painel, o botão e a barra pegos onde
  o rodapé os pinta, o teto do palco). Falta o `.#shot`: um filme a 1024 px (degrau pequeno) e a
  1500 px (médio), o botão de tocar e a barra de tempo apertados onde a linha de depuração "the
  player's panel laid out" diz que estão, o `o` levando o rodapé a 70% da tela e o `Esc` de volta,
  um mp3 com capa, um flac sem capa (o cartão de título em `text.secondary`, nunca sobre
  `surface.backdrop`), um fluxo sem duração e uma tela de 700 px (a frase da recusa na home). E o
  que só a tela mostra: o filme do mesmo tamanho numa página de duas linhas e na home
  (`player_media::panel_lines`), o glifo de trocar só no painel, o `k` sob o `o` trazendo as
  linhas e abrindo a página, e o botão aceso sob uma mão parada quando o glifo das mídias chega.
- **O par da modeline numa foto.** Os quatro cliques do par (tocar/pausar e levantar a coluna;
  próximo e abrir a vista) e a modeline depois de fechar o player em todo lugar nunca foram vistos
  numa tela: a cena (`modeline-pair`) foi escrita e não rodou, porque pede a VM.

- acabou quando: cada item acima sai daqui com a foto que o prova

### 0.00034. Os sons ambientes e o par da modeline sem foto · pequeno cada

- **A página não foi fotografada com um catálogo.** O palco do `.#shot` passa o catálogo de teste
  (`tests/player-media.nix`: dois tons sob os ids reais), mas a página, a linha da home e o par da
  modeline só foram vistos com estados escritos para a foto. Sons novos só entram com aprovação de
  ouvido do dono.
- **O par da modeline com só o ambiente não foi visto na tela.** A câmera não alcança o rodapé; o
  que existe é o teste da geometria (as mesmas células, o próximo no tom inerte, as frases sob o
  ponteiro). Falta o `.#shot`: um som ligado com a fila vazia, o par, o próximo apagado, um clique
  nele que não faz nada e o `hint-player-next-ambience` sob o ponteiro, e o tocar/pausar trocando o
  glifo quando o daemon responde.
- **A emenda num aparelho de verdade.** O build prova, amostra por amostra, o que o mpv ENTREGA
  (`--ao=pcm`, sem relógio); uma falta de dados do aparelho enquanto o mpv volta ao zero não foi
  medida.

- acabou quando: o `ambience-v1` está publicado e no flake, o player de uma instalação lista os três
  sons e toca um, e as duas fotos existem

### 0.00035. A linha do tempo nas portas — o que ainda não foi visto ou feito · pequeno cada

- **O deck no "Agora" do rodapé não foi visto na tela.** A câmera não alcança o rodapé; o que
  existe são os testes da vista (as linhas do deck no lugar da posição, só para um conjunto) e a
  aritmética dividida com a janela (`player_rows::deck_at`, `Deck::press`), provada pelos testes da
  janela. Falta o `.#shot`: um conjunto tocando, o deck no "Agora", uma banda arrastada (o fantasma),
  a frase sob o ponteiro na linha da mensagem e o `Esc` que cancela.
- **O roteamento por tela no rodapé não foi visto na tela.** Cada superfície do rodapé compõe as
  imagens roteadas ao conector dela (`player_media::placed_on_screen`), provado por teste; falta o
  `.#shot` com duas saídas: um filme só numa, a capa só na outra, e o esquema da página Disposição
  pintado na região.
- **"Mostrar em cada tela" não foi visto em duas telas.** A decisão é testada e as fotos mostram a
  frase; falta a VM de duas saídas (`player-vm`, o plano) provar que o compositor pinado honra o
  `set_fullscreen(Some(output))` e que cada janela mostra só o que vai à tela dela.
- **Os glifos das linhas são pequenos** ao lado do texto: a forma na Disposição, o `◀`/`▶` e os tipos
  de saída de som são desenhados no tamanho do texto, como toda marca de linha da vista.
- **O que o rodapé ganhou na revisão não foi visto na tela**: um tamanho só por imagem entre as
  telas que a mostram, o arrasto do deck que acaba com a vista, a garra das saídas refeita depois de
  um player reiniciado, a pasta ao lado do filme lida numa thread e a Disposição medida na caixa do
  painel. Os testes da vista e da janela cobrem a lógica dividida.
- **Decisões do dono**, levantadas na revisão e não tomadas aqui:
  - um som em duas saídas: uma faixa tem uma saída, e a mesma trilha como segunda faixa é recusada;
  - uma frase dita no cabeçalho de uma coluna estreita é cortada na linha fechada (a gaveta a mostra
    inteira) — vale para toda janela, não só o player;
  - no canto, as imagens pequenas ficam SOBRE a primeira, com um anel do chão (invariante 15).
  (Tomadas em 2026-09-29: `C-↑`/`C-↓` movem uma entrada ou um grupo; a página da faixa ganhou nome,
  fim e imagem; uma nota nunca perde linha; o clique numa banda escolhe e o segundo abre.)

- acabou quando: a foto do rodapé existe, a VM de duas saídas mostra cada tela com as suas imagens,
  e cada decisão acima foi tomada

### 0.00036. Os metadados do player — o que ainda não foi visto ou feito · pequeno cada

- **A seção no rodapé não foi vista na tela.** A câmera não alcança o rodapé; a capa sobre as linhas
  da região sai da mesma conta que a janela pinta (`player_metadata::cover_place`, testada), e o
  resto são as linhas da vista, que as fotos da janela e da coluna mostram. Falta o `.#shot`: uma
  música com capa no rodapé, rolada até a capa ficar pela metade.
- **O rascunho no editor não foi visto de ponta a ponta.** O daemon (E7) foi medido num namespace
  privado, e a vista pergunta "gravar?" pelo estado (teste); falta ver o `Told::Draft` abrindo o
  `emacsclient` e o salvar voltando como pergunta, com um player de verdade (`player-vm` ou o `.#dev`).
- **As buscas na rede (P8) não foram vistas com um daemon de verdade.** As páginas (Sugestões, a de
  uma sugestão com as duas capas, Buscar legendas, Buscar letras, Internet) foram fotografadas na
  janela e na coluna com mesas dadas (`photograph --desk`), e o daemon foi medido contra falsos no
  lane E8; falta a volta inteira: a porta pedindo a mesa quando a geração anda, a capa sugerida
  buscada ao abrir a página, e o teste de uma chave dizendo "aceitou" — num `player-vm` com o palco
  do `lookups-vm`, ou no `.#dev` com uma chave de verdade. E no rodapé, que a câmera não alcança.
- **Etiquetas pelo próprio nome não foram gravadas por um daemon de verdade.** O escritor foi provado
  em arquivos feitos no teste (FLAC, MP3, M4A, e um Ogg com o ffmpeg de verdade) e com programas de
  mentira para o Matroska e o remux; a página, fotografada com folhas dadas (`--sheet`). Falta a volta
  inteira — gravar, a folha nova chegando pela geração — com um player de verdade.
- **Gravar Matroska com o `mkvtoolnix` de verdade.** O escritor tem testes com os três programas de
  verdade (a etiqueta de uma faixa pelo `TrackUID`, o título de um capítulo), mas eles só rodam onde
  `RONOCTUA_MKVEXTRACT`, `RONOCTUA_MKVMERGE` e `RONOCTUA_MKVPROPEDIT` os nomeiam — a derivação
  `tests` do Nix não os passa, e lá os testes se dizem pulados. Falta passá-los, e um subteste no
  `player-vm` com o perfil de mídia.
- **Os formatos de resposta** do MusicBrainz, Cover Art Archive, Wikidata, TMDB, OpenSubtitles e
  SubDL vêm da documentação, com fixtures escritas à mão; só o do LRCLIB foi conferido contra uma
  resposta de verdade. O moviehash contra o `dummy.bin` publicado (4 GiB) só roda à mão; o
  `breakdance.avi` roda no `moviehash-published-vectors`.
- **Os códigos de língua do OpenSubtitles e do SubDL** em `assets/tongues.nix` foram escritos das
  listas dos serviços e só conferidos na forma: as duas listas pedem a chave de uma conta.
- **O Podnapisi** volta quando o domínio responder (`podnapisi::AWAKE`).
- **O que o `lookups-vm` e o `directories-vm` não conferem** (os dois passam no `.#most`, com uma CA
  descartável e `networking.hosts` para um falso HTTPS de cada host). O `lookups-vm` não confere o
  `0600` de uma chave: quem a grava é a porta, com o que a pessoa digita, e o teste a grava como a
  porta gravaria (`lookup-keys-are-private` prende como o arquivo nasce). O `directories-vm` não abre
  um filme inteiro do archive deixado (só a recusa sem a permissão) nem a TV além da recusa.
- **`ronoctua_lookup::keys::write_private_file`** mora no crate das buscas e guarda também as máquinas
  pareadas: é do `ronoctua-store`.
- **Os diretórios (P9) não foram vistos com um daemon de verdade nem no rodapé.** As páginas (Rádios
  e TVs do mundo com o globo, a lista de um país, gêneros, línguas, uma busca, os temas com as três
  fontes, favoritos, tocados há pouco) foram fotografadas na janela e na coluna com mesas dadas e o
  globo desenhado no lugar; falta a volta inteira — a porta pedindo a lista ao abrir e a mesa
  chegando, a estrela gravada pelo daemon e relida pela vigia do arquivo — e o globo no painel do
  rodapé (arrastar, a roda, um clique), que só os testes da vista cobrem. Também não medido: o custo
  de reprojetar o globo a cada evento do ponteiro com milhares de marcas (`nix run .#bench`). Duas
  telas: o rodapé guarda o globo desenhado por tela e o mede uma vez na maior (sem foto — a câmera
  não alcança o rodapé).
- **As páginas de estações e a recusa na hora não foram vistas contra o radio-browser**: o `offset`
  de uma página seguinte e a recusa que volta à página (`Told::Listing`) são testados com falsos e
  fotografados com `--desk`/`--refused`.

- acabou quando: a foto do rodapé existe, o rascunho e uma etiqueta foram vistos ida e volta, os
  diretórios foram vistos contra um daemon, cada formato de serviço tem uma resposta de verdade como
  fixture, e cada linha só de leitura acima ou age ou diz por que não

### 0.00037. Legendas: o que só um arquivo de verdade ou a VM provam · pequeno cada

- **Legenda de imagem (PGS, VobSub) num teste de VM.** O ffmpeg não codifica PGS a partir de texto,
  então a trilha secundária só tem teste de unidade. Falta uma amostra pinada (`downloads-are-pinned`)
  e o subteste que a vê no quadro, com as faixas de texto subindo para a banda de cima.
- **A trilha de dentro de um filme que toca pela rede** (seguindo outra máquina) só aparece sozinha:
  a chave de acesso à mídia vai num cabeçalho, e o ffmpeg a levaria no `argv`.

- acabou quando: a legenda de imagem tem subteste com pixel, e as outras duas existem ou estão no
  `ARCHITECTURE.md` como decisão

### 0.0. As TREZE vistas do rodapé não têm estado · pequeno, e é uma decisão

O estado é do BUFFER, e um buffer é uma janela do compositor (invariante 4): o `Esc` que volta ao
base é resolvido no processo que tem o teclado, e é por isso que ele pode existir sem roubar o `Esc`
do `vim`. As vistas do rodapé — transferências em `t t`, arquivos, rede, clipboard — não são janelas:
são painéis que o próprio rodapé desenha e que somem quando se escolhe algo.

Elas têm estados de fato (o gerenciador tem query e pergunta de permissão; a fila tem lista e
pergunta), e hoje a modeline não os mostra. **Isto não foi esquecido, foi deixado de fora**: o
mecanismo que o levaria até lá é outro — o rodapé teria de reportar a si mesmo — e a inconsistência
que isso cria (`t t` sem estado, `t w` com) é menor que a de inventar um segundo caminho.

- acabou quando: uma vista do rodapé aparece na modeline com o mesmo vocabulário de um buffer, sem
  um segundo mecanismo de relato

### 0.015. Os passos que o brain desenha e os que a unidade roda não têm quem os compare · pequeno

A tabela é uma só (o `steps.nix` do módulo `system-update` gera, e o brain a LÊ pelo
`tables.nix`), então os ids não têm mais como divergir — foi assim que o defeito anterior morreu. O que continua sem prova é a outra
ponta: a **unidade** escreve o relatório com esses ids, e nada confere que ela escreve os mesmos.
Hoje é o mesmo gerador que produz os dois, o que torna a divergência improvável e não impossível.

- acabou quando: um check lê os ids da tabela e os ids que o roteiro gerado escreve no relatório, e
  falha se um lado tiver um que o outro não tem

### 0.03. Glifo por nome só vale para o que é DECLARADO · pequeno, e é uma decisão

**Isto está em uso**: a entrada `screenshot` nomeia `md-monitor_screenshot`, porque nada no design
system diz "captura de tela" — o token `camera` é uma câmera de vídeo. O que falta é a outra porta.

`assets/notifications.nix` aceita um token (`camera`) ou um nome Nerd Fonts qualquer
(`md-camera_iris`), e resolve os dois no BUILD. Isso cobre tudo o que existe hoje: a distro declara,
o gerador resolve, e o rodapé não carrega tabela nenhuma.

O que não existe é a outra porta: um serviço externo mandando `md-bell` pelo barramento. Para
resolver isso em runtime o rodapé precisaria dos 10.473 nomes — 2,8 MB de `glyphs.json`, ou uma
tabela nova e enxuta lida **sob demanda** de `RONOCTUA_GLYPHDB`, no molde do `RONOCTUA_ICONS`. Custa
zero enquanto ninguém pedir, e é por isso que fica para quando a API existir.

- acabou quando: `notify-send --hint=string:glyph:md-bell` desenha o sino, e não desenha nada quando
  o nome não existe

### 0.1. O gerador de PO token roda sem o `canvas`, e isso pode parar de bastar · pequeno

O `canvas` está nas dependências do gerador e **nenhum arquivo sob `server/src` o
importa** — o jsdom só o procura se uma página pedir um contexto 2D. Ele é
instalado com `--ignore-scripts`, então a ligação nativa não é construída, e a
saída traz uma linha dizendo isso. Medido: o token é cunhado e aceito assim
mesmo, e o plugin lê a ÚLTIMA linha da saída, então o aviso é inofensivo.

O que isso arrisca é o dia em que o BotGuard passar a exigir uma impressão de
canvas de verdade: aí os tokens seriam recusados, e o sintoma apareceria como
vídeo que não baixa, não como erro do gerador.

- **o conserto, se chegar esse dia:** construir o `canvas` — o que traz Cairo,
  Pango e librsvg para o fecho e roda node-gyp dentro do sandbox. Não vale
  pagar isso por uma sonda que hoje não é usada.
- acabou quando: ou o aviso some, ou está escrito aqui que ele parou de bastar

### 3. Desempenho medido só em host não-NixOS · pequeno

`nix run .#bench` mede num compositor headless, sem janela, na GPU do host, com o contorno de driver
de `nix/graphics-host.nix`. O caminho gráfico da distro instalada é mais curto.

- acabou quando: o mesmo comando roda num NixOS instalado e os números entram no `ARCHITECTURE.md`

### 0.0000029b. O alívio do disco confia na conta do Nix · pequeno

O módulo `disk-space` faz uma coleta só, `nix-store --gc --max-freed`, e não mede o disco de novo
depois. Quando o Nix conta como liberado o que o disco não ganhou de volta (um caminho morto numa
camada só de leitura, medido no `disk-space-vm`: 263,8 MiB "liberados" e nada no `df`), a coleta para
antes de chegar ao lixo de verdade.

- acabou quando: o alívio mede o espaço livre depois de cada coleta e repete até a meta ou até a
  coleta não liberar mais nada, com um teste que planta um caminho cuja conta engana

### A lista de programas do instalador — o que a pista não provou · pequeno cada

- **A recusa chegou a um disco, mas não a um repositório.** O `installer-offline-vm` instala o
  full com programas deixados de fora, e o plano das partes da máquina não os tem. O
  `person-decline-chain` refaz o resto com os programas de verdade: o plano do instalador, o do
  primeiro boot (`plain --decline`, a chave fixada, o `apply`, a restauração) e as atualizações da
  pessoa. Nenhum teste leva o primeiro boot até um repositório de verdade, porque o teste de VM não
  tem rede. Acabou quando: uma instalação com rede no primeiro boot assina a escolha no repositório
  e a máquina continua sem o programa.

### Os painéis fora do gerenciador — o que a B1 deixou para depois · pequeno cada

A B1 tirou a prévia e os fatos do `files::View` sem mudar nada na tela (`inspection.rs`,
`inspection_sheet.rs`); o detalhe do fast-access é a porta sem listagem (`Subject::of_path` e
`Panel::focus`). Duas coisas ficaram de fora, de propósito:
- **O rodapé não se fotografa.** A câmera do gerenciador (`photograph --door files`) é a da janela;
  a vista do rodapé desenha as mesmas seções pela aritmética dele, e só a VM a vê.
  - pronto quando: a câmera tirar a vista do rodapé, ou a VM comparar as duas portas
- **Um filme e uma forma não se fotografam.** A câmera pede a resposta do inspetor na hora, mas não
  sobe o player nem o palco da forma: um vídeo aparece sem quadro e um `.stl` sem desenho.
  - pronto quando: `--frame` (ou um palco na hora) valer para a janela do gerenciador

### Atalhos, rodapé e modeline (leva de 2026-09-30) — o que só a tela e o compositor provam · pequeno cada

- **Fotografar no `.#shot` o que só o rodapé mostra**: a barra de alerta máximo nas duas fases, as
  horas no minibuffer livre, a dica que fica e a gaveta que cresce sob o ponteiro parado (medir que
  não há laço, amostrando muitos quadros), a linha `CTRL footer-bars-cycle` na raiz e a dica do
  próximo conjunto. O `photograph` não alcança o rodapé.
- **Medir o bind transitório `Ctrl+↑/↓`** (`wm-fold-keys-hold`) no compositor de verdade: que o
  `$ronoctua_shell` do config resolve, que o bind sai quando a dica sai, e se ele rouba de um
  programa em foco uma tecla que a pessoa esperava enquanto a mão está no rodapé.
- **Medir o `which-key` com a leader fechada entre as aberturas** (`.#bench`): a contagem agora é
  conferida, e a primeira rodada diz se o `leader-close` pelo soquete chega a tempo.
- acabou quando: as cenas existem com asserção, e as duas medidas estão escritas no
  `ARCHITECTURE.md`

### Lembretes e vigias do journal: o que falta ver e decidir · pequeno cada

- **A faixa do cérebro com o alarme nunca foi fotografada.** O `photograph` não alcança o rodapé;
  uma cena do `.#shot` que peça um lembrete para daqui a um minuto (pelo socket do cérebro) e
  fotografe a faixa quando ele tocar fecha isso, e também a linha da pergunta do `LEADER :`
  `reminders-new` no rodapé e na janela.
- **Da faixa só se adia pelo primeiro adiamento** (5 min, `, z`): a faixa e a lista de avisos do
  cérebro oferecem só o primeiro da lista; os de 10 e 60 minutos estão na página do módulo, uma
  linha cada. Se o dono quiser os três na faixa, é o `brain.rs` e o buffer que precisam de um
  seletor, não o módulo.
- **Um texto longo de lembrete é cortado na coluna do rótulo** da página (o que o buffer faz com todo
  rótulo). O aviso da faixa e a lista de avisos mostram a frase inteira.
- **A palavra do systemd sobre uma unidade de gerente delegado que não seja `user@<uid>.service`**
  (um contêiner, um serviço com `Delegate=`) é tomada por folha: numa morte dentro dele, a linha do
  kernel pode casar com ela em vez da unidade de dentro, se esta chegar mais longe no tempo.
- acabou quando: a foto da faixa tocando existe nas duas línguas, e o dono decidiu os adiamentos na
  faixa

### O seletor de arquivos — o que ficou de fora · pequeno cada

- **O pendrive espetado com ele aberto** entra nos lugares (e sai quando é tirado) pela vigia do
  `ronoctua-disks` enquanto o seletor está na tela (`watch_while`); isso tem teste de estado, e ainda
  não foi visto com um pendrive de verdade.
- **O `photograph` não alcança o seletor fora do player**: a região do rodapé (o seletor do player, a
  célula acesa, a faixa do cursor) e as portas de programas, Syncthing, rede e câmera, onde ele é
  lista, não são portas dele. Até lá, esses quadros só têm teste de estado.
- acabou quando: um pendrive espetado aparece no seletor aberto, e cada quadro acima tem foto

### 0.00000042. O navegador do git e os arquivos compactados: o que falta provar · pequeno cada

- **O lançamento pelo `C-h` da janela do gerenciador**: ele lança `ronoctua-shell git-browser
  <raiz>` por `systemd-run --user --scope`, que só existe com o gerenciador de usuário. Fora da VM
  viu-se o comando e a janela que ele abre (`br.ronoctua.git`, sob o Scroll do flake), e a linha do
  rodapé mudando sozinha com um ramo novo, um commit e um push; o lançamento em si pede um subteste
  de VM, que não existe.
- **A janela da fila com um trabalho de arquivo compactado** só foi vista por uma sonda fora do
  repositório: o `render-png` não recebe uma fila (o `--mind` é só do cérebro), e a conversão de
  produção (`transfers::queue_of`) mora no binário do shell. Dar ao `render-png` um `--queue` pede
  tirar essa conversão para um arquivo puro, como o `mind_wire.rs`.
- **A pergunta secreta no rodapé** (a senha de um arquivo, a frase-senha de um disco, no minibuffer)
  só se fotografa no `.#shot`; a da janela está nas fotos do `photograph --door files`.
- **Pôr membros numa pasta de DENTRO do arquivo**: hoje eles entram no topo (soltar ou `archive-add`),
  e soltar numa subpasta é recusado com uma frase — o `7zz a` guarda o caminho relativo à pasta de
  onde parte, e montar essa árvore pediria copiar os arquivos antes.
- acabou quando: o subteste do `C-h` existe, o `render-png` desenha uma fila, a pergunta secreta do
  rodapé tem foto, e soltar numa subpasta põe o membro lá

### O app de ícones e a calculadora — o que ainda não foi visto · pequeno cada

- **A porta do rodapé só foi fotografada à mão.** O `photograph` alcança a janela (`--door icons`);
  a região do rodapé (abas na primeira linha, grade, painel, página, o ponteiro que ela toma) foi
  vista num Xvfb com o compositor pinado e o rodapé da pista, nas duas línguas, mas nenhuma cena do
  `.#shot` nem check a refaz.
  - como saber que acabou: uma cena do `.#shot` abre `LEADER m i`, passa o ponteiro numa aba e numa
    célula, abre o painel, a página e a fotografa nas duas línguas.
- **O nome do ícone numa linha da vista** (a da busca), e não na linha de explicação do cabeçalho,
  como o desenho aprovado pedia — o porquê está no `ARCHITECTURE.md` ("A grade, e por que ela acende
  uma CÉLULA").
  - como saber que acabou: o dono confirma a linha, ou diz onde o nome mora com o cabeçalho fechado.
- **Calculadora no rodapé em foto.** A vista do rodapé só se fotografa no `.#shot`; falta uma cena
  com os três modos, estreito e largo, nas duas línguas (as da janela estão no `photograph`).
  - como saber que acabou: a cena existe, com asserção.

### O motor do player (leva de 2026-09-30): o que só se prova no fim · médio

- **Um vídeo do YouTube de verdade num `.#dev`** — o laço não rodou (abriria um compositor e som na
  tela do dono). Que TOCA já está provado fora dele (2026-10-04, `aqz-KE-bpKQ`, o yt-dlp 2026.08.19
  do overlay `nix/yt-dlp.nix`, o `pot-provider` 1.3.2, o deno, as opções do `engine::options`, mpv
  0.41 sem tela e sem som): token GVS cunhado, formatos do cliente `VISIONOS`, 77 s de AV1 4K e Opus
  com 183 MB lidos e nenhum 403, sem pedidos em faixas (o 403 depois de ~2 MiB era o yt-dlp
  2026.07.04 do nixpkgs; veja o player no `ARCHITECTURE.md`). Falta o que só o laço mostra:
  1. `nix run .#dev`; conferir no terminal do laço que `RONOCTUA_POT_HOME`, `RONOCTUA_POT_PLUGINS` e
     `RONOCTUA_DENO` estão exportados (`env | grep -E 'POT|DENO'`), e que
     `$RONOCTUA_POT_PLUGINS/plugin/yt_dlp_plugins/extractor/getpot_bgutil_script.py` existe (a
     variável é a RAIZ do gerador; `yt-dlp -v --plugin-dirs "$RONOCTUA_POT_PLUGINS"` diz
     "Plugin directories: …/plugin/yt_dlp_plugins");
  2. num terminal do laço (o ambiente dele, `RONOCTUA_INSTANCE=dev`), `ronoctua-shell play
     'https://www.youtube.com/watch?v=aqz-KE-bpKQ'` (Big Buck Bunny, licença livre) — ou a página
     "Vídeo de um site" do player (`LEADER p h`), quando a pista das portas chegar — e ver: a folha
     entra com o endereço, TOCA, e em segundos ganha o título do vídeo (o `yt-dlp --print title`, ou
     o `media-title` no `FILE_LOADED`);
  3. `journalctl --user -t ronoctua-player -f` (ou o log do laço) durante o play: nenhuma linha
     `ytdl_hook` de erro; com `RUST_LOG=ronoctua_player::engine=debug` (o alvo é o crate do binário),
     a linha "a voice's yt-dlp options" traz `plugin-dirs=<raiz do gerador>`, e `pgrep -af yt-dlp`
     durante o carregamento mostra o `--plugin-dirs`;
  4. um vídeo que não toca (um id que não existe, `https://youtu.be/aaaaaaaaaaa`): a recusa diz
     "yt-dlp disse: [youtube] …" com a linha `ERROR:` do yt-dlp, e não só "falha ao carregar" (e o
     `ytdl_hook` do mpv entrega essa linha sozinha, uma por evento — se vier colada às outras, o
     `said_of` ainda a acha, mas o comentário do `voice.rs` fica errado);
  5. o endereço de uma página que é lista, fora do YouTube (um álbum do Bandcamp): a folha ganha o
     nome do ÁLBUM, e não o da primeira faixa, em segundos (`--playlist-items 0 --print
     playlist:title`);
  6. o mesmo endereço posto duas vezes com `once` (a porta do ranking, ou um `put` à mão no socket
     do player, `{"ask":"put","what":["<endereço>"],"once":true}`): a segunda é a pergunta "já está
     na fila".
  - pronto quando: os seis passos se comportam assim, e o custo do gerador no fecho foi medido de
    novo (`nix path-info -S` do `pot-provider`: 53,6 MiB na última medida) numa instalação com mídia.

- **Player, portas: o rodapé sem foto.** O rodapé com a imagem recolhida, os saltos do relógio no
  painel, o globo de uma página do mundo com a imagem recolhida e a pergunta "já está na fila?" na
  região não foram fotografados: só o `.#shot` alcança o rodapé.
  - pronto quando: a cena existe, com asserção.

### 0.32. O catálogo nas portas: o que a onda não rodou · pequeno

O motor ingere e as portas listam; o que falta é prova de máquina.

- **Sem prova de máquina, só de unidade** (os subtestes do `player-vm` não os alcançam): a cerca do
  redirecionamento em claro (uma lista em `localhost` que redireciona para `127.0.0.1` tem de ser
  recusada), a fonte colada direto no catálogo adulto, e a janela que recebe uma listagem adulta e
  pede a senha.
- **O rodapé só foi fotografado pelos testes de estado**: o `photograph` não alcança a vista do
  player no rodapé. Falta uma cena `.#shot` com a listagem, a grade de pôsteres e a tranca no
  rodapé, e a entrega da listagem à janela em tela cheia (`F`).
- **Um vídeo real de cada fonte padrão** tocado a partir da listagem (a NASA pelo `collection.json`,
  o archive pela página do item): só os testes de unidade o cobrem.
- pronto quando: os três casos acima têm subteste, e a cena do rodapé mostra as cinco disposições
  nas duas línguas.

- **Video metrics no rodapé, visto.** A vista da região (`LEADER v v`) só foi fotografada pela
  janela, que desenha as mesmas linhas; a do rodapé, com a grade e as miniaturas como capas, o
  realce da célula (base + `FIELDS_LEAD` + célula) contra a célula do clique
  (`cell_under_pointer`), a fila abrindo depois de um download e a pergunta "já está na fila"
  seguida, precisa de uma cena do `.#shot`.
- **Uma varredura contra o YouTube de verdade**, pelo `.#dev`: acrescentar um canal por `@handle`,
  varrer, e ver miniaturas, detalhes e uma recusa (bot-check) dita na página.

### Central de segurança: o que falta provar · pequeno cada

- **A região da central no rodapé ainda não foi fotografada.** A janela foi (`photograph --door
  security`), e as duas portas desenham as mesmas linhas (`security::View::rows`), mas a região do
  rodapé só se fotografa no `.#shot`: falta uma cena com a central aberta (`LEADER s e e`), a pergunta
  do envio ao VirusTotal, o escudo do vigia na modeline, a rolagem até a seção pedida
  (`security-open privacy`) e o evento "rede nova tratada como pública" no minibuffer com a ação
  "tratar como de casa".
- **Nenhum teste de VM passa pela página.** O `security-vm` prova a fundação e o `files-vm` a troca de
  dono pela porta do sudo; o caminho inteiro de um clique na página (senha na pergunta secreta →
  helper → `firewall.json` → a página relida pela vigia da pasta) só está coberto por testes de
  unidade de cada lado.
- acabou quando: a cena da região existe, e um subteste de VM percorre o clique da página até a
  página relida

### 0.00000043. Containers: o que a vista não provou · pequeno

- **A região do rodapé não foi fotografada** (fora do `.#shot` nenhuma ferramenta a alcança); o que
  ela desenha é o layout empilhado da janela estreita, fotografado com `photograph --door containers
  --columns 80`. O realce por zona (um tipo, uma aba) e o log que segue o fim enquanto o fim está à
  vista foram testados só no nível da vista.
- acabou quando: uma cena do `.#shot` com o podman substituto fotografa a região, o realce e o log

### 0.0000004. Os pertences — o que só uma máquina de verdade, com um servidor git, prova · pequeno cada

Os testes do crate rodam contra um remoto `file://` bare com o git real (volta completa, senha
errada, conflitos, nada em claro no repositório, `0600` mantido, apagamentos, corrida de push,
senha nova). O que eles não alcançam:

- **SSH e HTTPS de verdade**: a chave de implantação num servidor (`known_hosts` próprio,
  `accept-new`) e o token pelo `GIT_ASKPASS` só se provam contra um servidor. Um teste de VM com um
  `sshd` e um `git-http-backend` no segundo nó fecharia isso;
- **a fila de ponta a ponta**: a ordem `Work::Belongings` sai da porta, a fila roda o CLI extraído
  e o `status.json` volta à vista — cada metade está testada, a travessia não;
- **as fotos que nenhuma câmera tira**: a porta do rodapé e a marca da modeline pedem o `.#shot`, e a
  linha da fila (a ordem dita na língua da pessoa) pede uma entrada de fila no `render-png`, que
  hoje não existe.

- acabou quando: um teste de VM sincroniza duas máquinas por SSH e por token, e as três telas acima
  foram fotografadas.

### 0.3. fast-access: o que ficou sem prova · pequeno cada

- **A região do rodapé (`LEADER d d`) não foi fotografada**: é a mesma vista e o mesmo desenho da
  janela (`fast_access_rows::draw`), mas só o `.#shot` a mostra. Falta uma cena.
- **Vídeo apagado do catálogo sai do índice** (`Videos::gone_since`, testado contra um catálogo de
  mentira); falta ver com o catálogo de verdade e a unidade rodando: um vídeo em `ronoctua-index
  query` (numa máquina, `nix run .#rust.index -- query …`: o programa não está no PATH) depois de
  uma varredura, e fora dela depois de apagado no player.
- **Web, YouTube, dicionário e tradução não foram fotografados com resposta**: o fotógrafo não pede
  host; o grid do YouTube foi fotografado sobre uma resposta de mentira (`--videos`).
- **Instalar um programa pela fila** faz as perguntas da central (licença não livre, download acima
  do limite da pessoa) a partir do que o motor dela lê (`ronoctua_software::details`); não pergunta
  o que a central só sabe depois de uma falha anterior do CLI (`last_failure`), que a fila então
  recusa e diz.
- pronto quando: a cena do rodapé existe, e o vídeo apagado sai do índice com a unidade de verdade.

- **O ranking no rodapé só foi visto em teste.** A janela e a coluna foram fotografadas
  (`photograph --ranking`); o rodapé só se fotografa no `.#shot`, e a cena ainda não existe: abrir
  `LEADER p k` sobre um catálogo semeado pela linha de comando (`ronoctua-video-metrics add` +
  `scan --details <n>` contra a origem de loopback do `video-metrics-vm` e um yt-dlp substituto que
  responde a duração), ver a faixa do ponteiro nas palavras e não na miniatura, o `⋯` só na linha
  dele, o "já está na fila" com o "não" primeiro, e o `kiosk-open` que o rodapé roda ele mesmo. A
  origem de loopback não baixa miniatura (por desenho): a cena as escreve no disco.
  - pronto quando: a cena existe, roda, e as fotos têm asserção.

### Instalar firmware — fora, por decisão do dono · registro

O fwupd entra só para LER: o nível HSI e as atualizações conhecidas, na central de segurança.
Instalar firmware seria uma categoria nova de ato imperativo (invariante 6), e o dono a deixou de
fora na leva de 2026-09-30; o porquê está no `ARCHITECTURE.md` ("Firmware só lido").

- acabou quando: o dono decidir que instalar firmware entra, e com que forma de exceção

### A tela de bloqueio — fora, porque pede uma exceção do invariante 3 · médio

A central de segurança registra a falta na própria página. Uma tela de bloqueio honesta pede o
`ext-session-lock-v1` (o compositor esconde tudo e só a nossa superfície recebe entrada); bloquear
desenhando por cima, com layer-shell, deixaria as janelas alcançáveis por baixo. É um protocolo
Wayland padrão, mas cada protocolo é uma exceção do invariante 3 aprovada uma a uma, e esta não foi
pedida.

- acabou quando: o dono aprovar o `ext-session-lock-v1` como exceção (uma linha na tabela do
  `AGENTS.md`, um arquivo só e um check `one-session-lock`), e a tela tranca e destranca pela senha
  da conta numa VM


## A leva de 2026-10-05

O que cada pacote deixar por fazer entra aqui, depois da sua âncora; o que ele concluir sai da
lista acima.

<!-- anchor: L01 -->

### O age do Rust ainda não fala `mlkem768x25519` · pequeno

- **O X-Wing e a estrofe são nossos** (`crates/ronoctua-pq/src/{xwing,crypt}.rs`), porque o crate
  `age` 0.12 do lock só traz o PQ marcado das chaves de hardware. Quando uma versão do `age` trouxer
  o `mlkem768x25519`, o recipiente dela entra atrás da mesma API (`Identity`, `Recipient`) e a
  composição local sai.
- **Os checks novos foram provados fora do Nix.** Os scripts de `pq-xwing-vectors`,
  `pq-age-interop` e `pq-identity-never-prints` (o texto avaliado de cada um) rodaram, limpos e
  sabotados, contra os binários do `cargo`, com o `age` 1.3.1 do nixpkgs e com o pinado do
  `pq-age-interop` (`.#checks.x86_64-linux.pq-age-interop.pinnedGoAge`, este sim construído no
  Nix); nenhum dos três foi construído, porque pedem os programas de `release`.
- acabou quando: os três checks passam no `.#most`, e o `pq-age-interop` passa com o recipiente do
  crate `age` no lugar do nosso

<!-- anchor: L02 -->

<!-- anchor: L03 -->

### O flake da máquina e as declarações — o que a pista não provou · pequeno cada

- **O `host-nix-golden-evaluates` não foi construído inteiro na pista**: a metade do build roda o
  `ronoctua-install` de release do Nix, e construí-lo não cabia numa pista. As duas metades
  foram provadas à parte (o ensaio do binário do `cargo` dá exatamente a fixture; o `toplevel` da
  referência instancia). Acabou quando: o `.#most` o constrói.
- **O AppImage declarado nunca foi construído com um AppImage de verdade**: a fixture tem pin falso
  (só avalia). O `appimage-desktop.py` foi rodado à mão contra uma imagem plantada hostil (`Exec=`
  de outro programa, `TryExec=`, chave estranha, `Terminal=` inválido, `.DirIcon` que é link para
  `/etc/passwd`) e escreveu só o que devia. Falta ver um AppImage real virar entrada de lançador com
  o ícone de dentro, e o embrulho abrir. Check barato oferecido: aquela imagem plantada, como check.
- **A verificação segura o que o consumidor só pularia.** O `personEntries` de uma entrada que o
  consumidor recusa com motivo (não livre sem consentimento num `minimal`, um atributo que o nixpkgs
  não tem) falha na avaliação isolada, e a verificação a seguraria como `eval-failed`; o relatório
  diria `held` em vez do motivo. Ela volta sozinha quando a entrada muda (impressão digital nova).
  Acabou quando: o `raw` de cada construtor devolve um marcador inofensivo onde o `build` pularia a
  entrada, ou a verificação (pacote do aplicador) só segura o que o consumidor não sabe pular.
- **O `still-present` não foi exercitado**: o registro da distro estava vazio na pista. Acabou
  quando: o registro tem entradas e um app recusado que outro caminho traz aparece na lista.

<!-- anchor: L04 -->

### Emacs: o que a página só diz · pequeno cada

O alvo `emacs` leva só `~/.config/emacs`, e o que o Emacs de uma máquina lê fora dali é dito numa
frase da página dos pertences, nunca resolvido por ela:

- **Um `~/.emacs.d` (ou `~/.emacs`) que esconde `~/.config/emacs`.** Numa máquina de antes da pasta
  XDG da unidade, ou com uma configuração antiga, o Emacs lê o outro lugar; a página diz qual, e que
  mover a configuração para `~/.config/emacs` a faz viajar. Um `~/.emacs.d` só com o que o Emacs fez
  é dito à parte. Nada é movido nem apagado.
- **Um `~/.emacs.el` sozinho deixa de ser lido** quando uma sincronização traz `~/.config/emacs` para
  aquela casa: o Emacs passa a ler a configuração que chegou, e a página não diz nada (o Emacs lê o
  lugar que viaja). A unidade nunca cria a pasta XDG numa casa com `~/.emacs.el`.
- **Um framework clonado em `~/.config/emacs` viaja como configuração.** Nenhum checkout é
  reconhecido: o Doom ou o Spacemacs de uma máquina chegam à pasta das outras, e uma configuração
  própria e um framework no mesmo lugar em duas máquinas se misturam (com pergunta onde o mesmo
  arquivo difere).
- **Apagar ou mover `~/.config/emacs` numa máquina apaga nas outras** (cada uma guarda a cópia em
  `replaced/`), como qualquer pasta guardada.
- **`emacsclient -a ''` sobe um daemon sem a unidade.** Com a unidade parada, o `e` do shell sobe o
  Emacs direto, sem a pasta XDG antes; numa casa sem nada, ele cria o `~/.emacs.d`.
- **Uma adição feita à mão em `~/.config/emacs` antes do alvo** (a sobreposição que a primeira
  revisão achou) é o que era na a88e849: uma raiz própria no índice. O que isso faz, medido nos
  testes `an_old_addition_at_the_emacs_folder_is_a_root_and_deletes_nothing` e
  `an_old_addition_at_a_target_is_a_root_and_takes_no_file_out`: os arquivos dela viajam marcados
  como acrescentados, e as outras máquinas a seguem (a página delas a lista como acrescentada em
  outra máquina, ao lado do alvo, para o mesmo lugar); "deixar de guardar" na linha dela põe
  `~/.config/emacs` no que foi tirado, e isso é o "não nesta máquina" do alvo inteiro (ele tem um
  caminho só) — a máquina para de guardar a
  configuração do Emacs, as cópias do repositório ficam para as outras, e "guardar aqui de novo" no
  alvo a traz de volta; e os pacotes que uma máquina sem o alvo mandou por ela (`elpa/` e o resto do
  que o alvo pula) ficam no repositório para sempre — nenhuma máquina nova os guarda, e tirar a
  adição não os tira, por causa da guarda —, até um "em nenhuma máquina" do alvo, que tira o alvo
  inteiro. E com uma máquina na a88e849 no mesmo repositório (nenhuma versão foi publicada, então só
  entre builds de desenvolvimento): se ELA tira uma adição que cobre um alvo enquanto outra máquina
  não sincronizava, essa outra apaga a própria cópia dos arquivos do alvo que tinha de antes da
  adição (posta de lado em `replaced/`); a guarda só vale para quem tira.
- acabou quando: o dono decidir se algum desses casos ganha mais que a frase (uma pasta raiz que
  some inteira esperar pela pessoa, por exemplo), se o `e` passa pela unidade, e se uma adição no
  caminho de um alvo passa a ser o alvo — com uma migração que não quebre o "guardar de novo" nem uma
  máquina na versão anterior.

### Seguir as releases dos Crafting Apps nunca rodou de ponta a ponta · médio

A opção (`distro-apps-follow-releases`) foi provada por partes: o `pins refresh` sobre respostas
gravadas do GitHub e sem Nix, o juiz nos dois lados pela tabela de casos, o registro, a lista de
permitidos e o perfil da pessoa em máquinas avaliadas (`distro-apps-follow-when-on-and-newer`), o
verify do root com um `nix` de mentira. Nenhuma execução juntou o GitHub de verdade, o
`prefetch-file` com o nome do builder, a assinatura, a aplicação do root dentro da própria atualização, o verify construindo um Craft seguido e o rebuild que o põe
no perfil da pessoa.

- acabou quando: um teste de VM (ou uma atualização numa máquina de teste) segue uma release real,
  o verify a anota e o perfil da pessoa roda a versão seguida; e outra, com o pin seguido estragado,
  retém, fica com a última verificada e diz por quê

### O apk do F-Droid é fixado pelo que chegou, não pelo índice assinado · pequeno

O `nix run .#distro-pins` fixa o hash do apk que baixou por HTTPS da versão que o F-Droid sugere;
não confere esse hash com o índice assinado do repositório (`entry.jar`). Hoje a tabela da distro
não tem app Android.

- acabou quando: o hash do apk é o que o índice assinado do F-Droid diz, e um apk diferente é
  recusado

### O web app da distro não diz a classe da janela · pequeno

O lançador de um web app da distro não leva `StartupWMClass`, porque a conta do Chromium mora em
`web::app_id` (`crates/ronoctua-software`) e uma segunda cópia em Nix derivaria. A janela abre, mas
a modeline a nomeia como o Chrome. Hoje a tabela da distro não tem web app.

- acabou quando: o lançador é escrito pela mesma função que escreve o da pessoa (um verbo da CLI
  rodado no build), com a classe


<!-- anchor: L05 -->

### A vida da máquina (L05) — o que ficou por fazer · médio

- **O teste de VM `boot-trial-falls-back` foi escrito e não rodou** (está em `tests/slow.nix`:
  reinicia, então só o `.#gate` o roda): systemd-boot, EFI com variáveis persistentes, o alvo padrão
  da distro sem sobrescrever; um ensaio que entra em pânico, a volta sozinha, o bless, a recusa
  `did-not-boot`, e um ensaio cujo estágio 2 cai no modo de emergência e é reiniciado pelo prazo. Ele
  decide o que o desenho deixou aberto: que o `LoaderEntryDefault` vence o `default` do `loader.conf`
  no systemd-boot pinado, que o `LoaderEntrySelected` chega ao estágio 2 e que o prazo sobrevive ao
  isolamento do `emergency.target`. (O `migration-keeps-the-machine` e o `rescue-vm` passaram no
  `.#most`.)
  - pronto quando: ele passa.
- **O rollback do resgate desarma, e nenhuma VM o roda**: o `disarm` está provado nos testes do
  crate e a fiação (o rollback chama o `disarm` entre o `--rollback` e o switch) no
  `update-apply-rule-is-narrow`; o `rescue-vm` não aperta a linha do rollback.
  - pronto quando: uma VM arma um ensaio, faz o rollback pelo resgate e sobe no sistema de volta.
- **Tentar de novo o sistema que falhou**: o `apply` recusa para sempre o toplevel cujo ensaio falhou
  (ou foi desfeito), até a distro andar. Uma falha de acaso (a luz caiu no boot do ensaio) não tem
  como ser tentada de novo sem uma revisão nova.
  - pronto quando: o dono decidir se existe um gesto para isso (e qual porta o oferece).
- **Secure Boot (lanzaboote)**: as entradas são UKIs (`type2`), e não foi medido se o `bootctl list`
  mostra o `init=` delas. Até lá uma máquina com lanzaboote cai em `no-trial: not-in-menu` e fica
  sem ensaio, dito no registro.
  - pronto quando: o `secureboot-vm` (ou um irmão) arma e consagra um ensaio.
- **Dar `hold` por clique**: a vista da atualização já desenha o ensaio, o `no-trial`, a retenção e a
  frase de cada recusa (L19); reter a máquina numa revisão ainda é só `sudo ronoctua-machine hold
  <rev>` de um terminal. Pelas telas, pede um `RootVerb` novo na porta do sudo (sem argumento, lendo
  um pedido preparado, como os outros) e a pergunta com o "não" primeiro.
  - pronto quando: o dono decidir o gesto, e as duas portas o oferecerem com foto.

<!-- anchor: L06 -->

### Os dados da pessoa na máquina (L06) — o que ficou por fazer · médio

- **A aprovação de um módulo não tem tela.** O verbo, a unidade e o hash existem e funcionam
  (`ronoctua-person approve`, provado com o Nix de verdade contra o `nix/person-hash.nix`), mas
  nenhuma vista mostra o `brain.awaiting-approval` do `person-applied.json` nem pergunta. Hoje só se
  alcança de um terminal: `ronoctua-person request-approval <id> <hash>` e depois a porta do sudo
  com `person-module-approve` (`sudo $RONOCTUA_SECURITY_ROOT person-module-approve`). A vista que
  mostrar os módulos à espera prepara o pedido (`approve::Request`, `root::APPROVE`), passa por
  `authority::run(RootVerb::PersonModuleApprove)`, desenha o "não" primeiro (o código alcança root)
  e diz o resultado lendo `root::APPROVE_VERDICT` (`approve::Verdict`: o módulo, os dois hashes e o
  `Denied`), porque a linha da porta só diz `refused failed`. Cada `Denied` precisa de uma frase nos
  dois idiomas.
  - pronto quando: as duas portas de uma vista mostram o módulo à espera com o hash e aprovam por
    clique, com foto das duas.
- **O `manifest.nix` de um módulo da pessoa é avaliado por root antes de qualquer aprovação**, se o
  carregador o lê para saber marcas e prefixos (L03/L11). A avaliação não executa nada, mas pode ler
  um arquivo que só root lê e pô-lo numa derivação. A aprovação cobre a pasta inteira; falta decidir
  se um módulo não aprovado é avaliado de todo.
  - pronto quando: o dono decidir, e um check prender a decisão.

<!-- anchor: L07 -->

### O store leniente — o que ficou

- **Uma frase em cada vista que escreve com `parsed`** (som, câmera, estúdio, Syncthing, Tor, Atuin,
  ClipCascade, teclado na tela, player…): a parte que não leu já aparece na lista dos pertences
  ("Consertado na leitura", com "deixar" e "gravar o conserto", L21), mas a própria vista não diz
  nada — como antes desta leva. Fica se o dono quiser a frase também ali.
- **A vista dos ponteiros não tem porta de fotografia**: a frase da parte não lida (e a de antes,
  `pointers-settings-failed`) é provada só pelo teste das linhas.
- **A chave de deploy de OUTRA máquina** que já chegou a esta por um sync antigo não é reconhecida
  aqui (esta compara com a própria chave); só sai do repositório quando a máquina dona a vê.
- **Um link para o `/nix/store` que outra máquina empurrou antes desta regra** ainda é puxado por
  uma máquina que não tem aquele caminho. A puxada (`engine/sealed.rs`) deveria recusar
  materializar um link que `sift::into_the_store` reconhece — uma linha, que espera a puxada do
  formato 2 (pedaços), que reescreve esse arquivo.
- **Os seis checks novos** (`a-store-keeps-the-rest-of-a-broken-file`,
  `a-newer-field-survives-an-older-write`, `an-untouched-repair-is-not-written`,
  `belongings-drafts-are-never-kept`, `belongings-store-links-are-not-kept`,
  `belongings-access-never-travels`) foram provados pelo `cargo test` e por sabotagem; no Nix eles
  só rodam na bateria (`rust-tests` estreitado a um teste).
- **O custo da leitura a mais por escrita** não foi medido num store grande (a fila do player, o
  histórico das notificações continuam fora do `Store`).

<!-- anchor: L08 -->

### Pertences, formato 2 — o que o pacote do formato selado deixou · pequeno cada

- **A compactação guarda sempre os dias da tabela** (`budget.keepDays`): a vista oferece compactar
  (L21), mas não escolhe outro número; o `requests.json` aceita `Compact{keep_days}` e o
  `Engine::compact` o lê, para quando houver uma porta que peça.
- **`belongings-recover-without-the-distro` nunca rodou com o X-Wing de verdade.** Quando o pacote
  o escreveu, o `ronoctua-pq` era o substituto X25519, que o `age` do Go não abre; o X-Wing entrou
  depois (L01). O trecho do check (as duas execuções, a inteira e a com um pedaço cortado) rodou com
  o `age` 1.3.1 do nixpkgs sobre um repositório da mesma forma com chaves clássicas, e o `script`
  entregando a senha ao `age` foi provado dentro do sandbox do Nix; a execução é uma derivação
  própria (`belongingsRecovered`).
- **Os checks do formato 2 foram avaliados, não construídos.** Construir um pede a camada de
  dependências do workspace inteira de novo (o `Cargo.lock` ganhou o `fastcdc`), sob a trava pesada
  que as outras pistas esperam. A lista de testes, o filtro `--exact` com treze nomes e cada `grep` dos
  checks foram conferidos com o mesmo `cargo test` na pista, e a parte que lê fonte do
  `belongings-ask-with-a-pass` rodou fora do Nix, com sabotagem.
- **O GitHub guarda o histórico compactado** até a coleta dele; tirar de lá as versões velhas pede um
  pedido de limpeza ao suporte, que é da pessoa.
- acabou quando: o `.#most` passa com os oito checks.

<!-- anchor: L09 -->

### Pertences: a parte em claro assinada — o que o pacote deixou · pequeno cada

- **Editar um módulo do brain não pede a senha no gesto.** O módulo editado aqui espera "Assinar
  agora" na página dos pertences (ou a próxima sincronização, ou a identidade da sessão); a central
  de programas já pergunta a senha no gesto, e a atualização assina com a identidade da sessão. Um
  editor não é uma porta nossa: o gancho seria a vigia das raízes em claro do `belongings-reminder`,
  que hoje só manda o que pode mandar.
- **Uma senha errada na pergunta da central de programas não é dita nela.** O `sign` recusa
  (`wrong-passphrase`) dentro do trabalho da fila, o journal diz, e a mudança espera assinatura — a
  página dos pertences mostra "esperando a sua senha para ser assinado"; a linha do trabalho na
  central não diz por quê.
- **Duas máquinas que assinam ao mesmo tempo podem dar o mesmo contador** a manifestos diferentes:
  o contador só impede voltar para um MENOR, então um igual de outra máquina é aceito. O merge da
  sincronização seguinte assina um acima dos dois; nada retrocede além do empate.
- **A porta do rodapé não foi fotografada** nos estados novos (retido, esperando assinatura, sem
  chave fixada, outra chave, a recusa de root, a fixação recusada, a pergunta da senha da central de
  programas): é a mesma `View` e as mesmas linhas da janela, que foi, nos dois idiomas.
- **Os checks novos não foram construídos no Nix** além do `person-apply-rule-is-narrow`: o
  `belongingsSigned` compila os testes de quatro crates, e as partes que leem fonte rodaram fora do
  Nix, com sabotagem (os testes nomeados rodaram no `cargo`).
- acabou quando: o `.#most` passa com `belongings-tampered-plain-is-held`,
  `belongings-plain-rollback-is-refused`, `person-apply-refuses-unsigned-tree`,
  `brain-unsigned-module-never-compiled` e o `pq-identity-never-prints` estendido.

### Pertences: parte em claro e merge — o que o pacote deixou · pequeno cada

- **Um clone antigo não vê um repositório refeito vazio.** O `Repo::fetch` do `ronoctua-git` não
  poda (`--prune`), então uma máquina que já tem o clone toma o `origin/main` velho pelo do
  repositório novo e só o enche no próximo push que tiver o que mandar. A parte em claro não perde
  nada nesse caso (a base continua a de antes e nada é apagado), e o caso do clone novo já não usa
  base nenhuma; podar é da fronteira do git e da parte selada.
- **A porta do rodapé não foi fotografada** com os conflitos mesclados: é a mesma `View` e as mesmas
  linhas da janela, que foi.
- acabou quando: o `.#most` passa com os oito checks (construídos uma vez na pista, verdes).

<!-- anchor: L10 -->

### Sudo, o lado root — o que ficou · pequeno

O lado root e a barra do sudo (2026-10-11) estão prontos.

- **O `security-vm` estendido não rodou**: os novos passos (as durações, a volta do timer no boot, o
  dedo no terminal com a sentinela, a lista de chaves) foram escritos e esperam o `.#most`.
- acabou quando: o `.#most` passa com o `security-vm`.

<!-- anchor: L11 -->

- **A máquina passa os módulos da pessoa ao brain.** O `withPersonModules` existe no flake (L11);
  falta o `lib.machine` (L03, `nix/machine.nix`) usá-lo no lugar do `distroModules` cru:
  `(withPersonModules { dir = dir + "/person/brain"; only = <ids incluídos>; }).machineModules
  <ids aprovados>`. Os incluídos são os que o `approvals` já calcula (sem metade NixOS, ou com ela
  aprovada pelo hash do `nix/person-hash.nix`, L06); os aprovados são os da metade NixOS. A metade
  NixOS entra por ali — pela portaria do carregador —, nunca pelo `m.folder + "/module.nix"` cru, que
  vale com o brain desligado e não recusa `imports` de fora nem `disabledModules`. O filtro de nome do
  `approvals` (`my-[a-z0-9][a-z0-9-]{0,63}`) aceita ids que o carregador recusa (`my-2fa`): ou ele
  usa a gramática do carregador, ou deixa a pasta passar e o carregador a recusa com frase. O
  relatório (`person-applied.json`, L06) ganha as pastas de `.refused`, com a frase de cada uma —
  hoje o `BrainReport` só tem `included` e `awaiting-approval`. E a verificação isolada da
  atualização (L06/L19) avalia `brainModules.personProblemsOf.<id>` uma pasta por vez e tira do
  `only` a que não avalia. Até lá, um módulo `my-…` viaja nos pertences e nenhuma máquina o compila.

<!-- anchor: L12 -->

### O `/private`, o que ficou

- **Um arquivo do cofre, fora da listagem.** A listagem, o `stat` do vigia e o vigia passam pelo
  guarda (`ronoctua_files::listing`); o que a porta faz com UM arquivo dele no laço — os fatos do
  painel, um gesto sobre o arquivo focado — ainda não. Só alcança o instante entre o status dizer
  "aberto" e o gancho do sono suspender: depois, o status ouvido tira a pasta da tela.
- **Retomar pela chave.** Um cofre suspenso em uso só volta pela senha: o `luksResume` pela
  chave FIDO2 precisa do plugin de token do systemd no caminho do `cryptsetup`, que o pacote não
  declara. Abrir pela chave (o cofre fechado) funciona.
- **Uma marca na modeline enquanto aberto** (o design a deixa opcional): o `security-watch`
  poderia vigiar o `status.json`. Não foi feita.
- **O teste de VM `private-vm`** foi escrito e avaliado, não rodado (as baterias são da fase
  final). É ele que prova o `/private` montado visível fora da unidade (o `findmnt` depois do
  `create`, com as unidades sem namespace de montagem) e o gancho do sono desmontando a montagem
  de verdade. A chave FIDO2 não tem como ser testada lá.
- **A exceção do inv. 6 para o `/private`** foi escrita no `AGENTS.md` a partir do plano aprovado
  da leva; o dono confirma o texto.

<!-- anchor: L13 -->
### O Incus: o que a plataforma e o crate deixaram

- **As respostas que o `incus-vm` grava em `fixtures/` ainda não substituíram as escritas à mão** em
  `crates/ronoctua-incus/src/fixtures.rs`, salvo os eventos (o daemon manda o `source` e nenhum
  `name`, e o crate lê o nome do caminho). Faltam as instâncias, o estado, os snapshots, as imagens,
  os perfis, as redes, os pools, os volumes, os projetos e o `remote list`, e o `progress` das
  operações, que o teste não captura.
  - acabou quando: o `fixtures.rs` é o que o `incus-vm` capturou.
- **O WebSocket dos eventos não foi visto contra o daemon**: a forma dos eventos é conferida no
  `incus-vm` pelo `incus monitor` (o mesmo JSON), e o transporte (`/1.0/events`, o aperto de mão, o
  fim pelo `Stopper`) contra um servidor de teste; o `Events` do crate não abre o daemon de verdade.
  - acabou quando: um `instance-started` é lido pelo `Events` do crate num daemon (a vista do L23
    fotografada numa VM, ou um teste que o faça).
- **O disco de uma VM feita de uma imagem pode ficar com cópia na escrita, e isso não foi visto**: o
  crate pede `initial.btrfs.compression=none` no disco raiz de toda VM, e um volume de bloco NOVO
  feito assim é `+C` (o `incus-vm` confere). Mas uma VM feita de uma imagem num pool btrfs costuma
  ser um snapshot do volume da imagem em cache (`images/<fingerprint>`), e um snapshot guarda as
  flags do `root.img` que divide (medido): o pedido só chega lá se o daemon escreveu o volume da
  imagem sem cópia na escrita. Ver isso exige criar uma VM, e a VM de teste não tem KVM aninhado.
  Uma `+C` na pasta `virtual-machines/` (o desenho) não serve: a raiz de um subvolume não herda flag
  nenhuma (medido). O caminho que não depende do daemon é um pool `dir` para as VMs numa pasta `+C`
  (todo volume é pasta, e todo disco nasce dentro dela; medido), ao preço de snapshot e cópia
  inteiros. E uma VM criada pelo CLI cru fica com cópia na escrita: o Incus não tem padrão por tipo
  de instância num perfil.
  - acabou quando: num `incus-vm` com KVM aninhado (ou numa máquina), uma VM lançada pelo crate a
    partir de uma imagem importada tem `lsattr` C no `virtual-machines/<vm>/root.img` — ou, se não
    tiver, as VMs foram para o pool `dir` `+C`, com essa prova.

<!-- anchor: L14 -->

### Contas de LLM: a contagem medida numa conta de verdade · pequeno

O `ronoctua-llm` conta os tokens (`tokens::by_day_and_model`) e lê os limites do Codex a partir de
linhas inventadas no formato que a documentação e as ferramentas públicas descrevem: o Claude Code
com `message.id`/`requestId` repetidos e `<synthetic>`, o Codex com o total corrente do
`token_count`, o `turn_context` e o `.jsonl.zst`. Nenhum transcript de pessoa foi lido para isso, e
nada prova ainda que o Codex instalado escreve mesmo rollouts comprimidos ou o `resets_at` (o
`resets_in_seconds` também é aceito).

- acabou quando: numa conta real de cada agente, o total de um dia que o `ronoctua-llm refresh`
  guardou bate com o que o `ccusage` (ou o `/usage` do agente) diz, ou a diferença está explicada no
  `ARCHITECTURE.md`; e os limites do Codex lidos dos logs batem com os que ele mostra

<!-- anchor: L15 -->

<!-- anchor: L16 -->

### A primeira versão de verdade, e o que só ela mede · médio

O `.#release` e os três apps em volta existem e foram provados com substitutos (um `gh`, um `nix` e
um gate que só anotam que foram chamados, num repositório com `origin` local:
`release-refuses-dirty-tree`; o empacotamento e a junção do ISO: `iso-parts-reassemble`; a adoção e o
`.#apply-here`, até o switch: `machine-apps-stop-before-the-switch`). A primeira versão saiu pelo
`.#release` em 2026-10-21 (`v2026.10.21-4d366d8`), e o `nix run
github:ronoctua/ronoctua-distro/release#fetch-iso` juntou o ISO dela, interrompido uma vez no meio. A
distro não tem cache binário (decisão do dono, 2026-10-11): o ISO leva sempre a semente.

- **`.#adopt-machine` e `.#apply-here` nunca rodaram numa máquina.** Não há VM para eles: o primeiro
  pede uma máquina do layout antigo (a do dono), o segundo uma máquina instalada e um checkout. O
  `migrate` que o primeiro chama é o que o `migration-keeps-the-machine` (L05) prova.
  - acabou quando: a máquina do dono foi adotada com `nix run .#adopt-machine` e um `.#apply-here`
    voltou à `release` na atualização seguinte.
- **A tag assinada (`--sign`) não tem leitor.** O pendrive confere o `SHA256SUMS` contra o TLS do
  GitHub, não contra a assinatura.
  - pronto quando: o dono decidir se o pendrive (ou o `.#fetch-iso`) deve conferir a tag, e com que
    chave pública guardada onde.
- **Não livre é o que a licença, o predicado e o nome sabem.** O `nix/release-unfree.nix` pergunta a
  licença do pacote de topo daquele nome, o predicado da distro e, com o Android, o prefixo
  `android` — o `uade`, que entra por um nixpkgs próprio, já era um caso, e é coberto pela licença.
  Fica de fora o não livre sem nome de topo, fora do predicado e sem `android` no nome: um AppImage
  ou app Android que a distro registrar (`assets/distro-apps.nix`; hoje, os treze Crafting Apps,
  todos Apache-2.0) não tem
  `meta.license` do nixpkgs, e se for proprietário tem de entrar no predicado de
  `nix/modules/nixpkgs-config.nix`, senão o `.#release` o empurra para um cache público. O
  `release-cache-holds-only-free-builds` lista as portas do repositório por onde um não livre entra.
  - acabou quando: um check recusa entrada não livre da tabela da distro fora do predicado.

### O cache binário da pessoa, provado só com substitutos · médio

O cache é da pessoa (o Cachix dela): a página na vista da atualização, o envio depois de cada
atualização que trocou a máquina, o netrc do root para um cache privado e a instalação que já o usa
existem e foram provados por testes (`ronoctua-cachix`, `ronoctua-installer`, `ronoctua-belongings`
`take`, os verbos do `ronoctua-security`) e pelos checks `binary-cache-declared-once`,
`binary-cache-token-never-prints` e `release-cache-holds-only-free-builds`. Nenhum deles tocou o
Cachix nem uma instalação.

- **Nenhum envio chegou ao Cachix.** Os testes rodam com substitutos de `nix`, `nix-store` e `cachix`.
  Falta um cache de verdade, público e privado, e uma atualização que troca a máquina: o relatório do
  `cachix push` tem de bater com a contagem que o envio diz, e a consulta ao `cache.nixos.org`
  (milhares de caminhos de um sistema, para um cache privado) tem de caber no tempo de um passo.
  - acabou quando: os dois casos estão medidos aqui, com o tamanho e o tempo do primeiro envio.
- **O cache da pessoa nunca chegou a uma máquina pelo primeiro boot.** A instalação não usa cache
  nenhum (L25): o cache e o netrc de um cache privado chegam com os pertences, depois do primeiro boot.
  - acabou quando: uma máquina instalada sem rede, cujos pertences nomeiam um cache privado, baixa
    dele na primeira atualização depois que o token entra pela porta do sudo.
- **O veredito do netrc no rodapé não foi fotografado**: ele chega de uma thread depois do quadro que a
  câmera tira. O caminho até o minibuffer é o de toda porta.
- **A chave pública é conferida pela forma**, não contra o cachix.org: a tela não pergunta à rede.

<!-- anchor: L17 -->

### O pendrive do instalador: o que só uma máquina e uma versão de verdade provam · pequeno cada

- **O host do redirecionamento do GitHub é escrito**: a primeira versão publicada redireciona para
  `release-assets.githubusercontent.com`; se mudar, o aria2 segue e a linha de consentimento continua
  dizendo o antigo.
- **O caminho inteiro numa máquina** — a vista, a senha, a fila, a gravação de uma ISO real e o boot
  dela — não foi feito: pede um pendrive e o dono.
- **A barra da fila recomeça a cada parte baixada**; a gravação e a releitura somam uma barra só.
- **Dois "sim" no mesmo instante, um em cada porta**: entre a porta do sudo iniciar a unidade e a
  fila aceitar o trabalho (milissegundos), o segundo vê uma unidade esperando sem trabalho armado e a
  manda parar; o primeiro então falha dizendo que o gravador não roda. Fechar isso pede uma trava
  entre as duas portas (um arquivo de preparo criado `create_new` antes da senha).

<!-- anchor: L18 -->

### Os programas declarados — o que a pista não provou · pequeno cada

- **Numa máquina de verdade, nada do caminho foi feito**: declarar uma AppImage com o GitHub de
  verdade (o pin pelos bytes daqui, o `nix store add` sob o nome do construtor), o pin de um apk do
  F-Droid, o reconciliador com um Waydroid rodando, o rebuild de `ronoctua-update-apply.service`
  depois de um "remover de vez" (a regra polkit do L05) e o `plain push` para um repositório.
  - pronto quando: cada um foi visto numa máquina, e o que divergiu virou teste.
- **"Manter em todos os seus computadores" é oferecido para o que veio de um arquivo**: o
  `Installed` não diz de onde uma AppImage ou um apk veio, então a linha aparece e a ordem é recusada
  na fila, com a frase (`NothingToPin`), sem escrever nada.
  - pronto quando: o `Installed` leva a procedência e a linha diz o motivo antes do clique.
- **Mudar só o ícone de um web app declarado aparece pela metade até a atualização**: o
  reconciliador espera o build comparando nome, endereço e perfil (`reconcile::waits_for_build`), e
  não o ícone. Um reconcile antes da atualização (o de outro "remover de vez" de web app, de uma
  recusa) tira a entrada da pessoa, e o ícone velho do sistema volta até a atualização construir o
  novo.
  - pronto quando: a comparação inclui o ícone (o SHA-256 do arquivo de cada lado).
- **A trava da declaração não é dos pertences**: a central relê o documento antes de gravar e refaz a
  mudança sobre uma mescla que chegou, e os pertences não escrevem sobre um arquivo que mudou desde
  a leitura (`land_plain`); sobra o instante entre a releitura e o `rename` de cada lado.
  - pronto quando: o `plain pull` dos pertences toma o mesmo `declaration.lock` (o nome num lugar
    só, que os dois crates leiam).
- **A notificação de um "remover de vez" que a atualização segurou diz "removido"**: o fim do
  trabalho não sabe que a mudança esperou (o `Outcome` não diz, e um `Ended` novo pede uma etiqueta
  de notificação no rodapé (`ronoctua-shell-footer`) e no `assets/notifications.nix`). A lista diz a
  verdade — o programa continua nela com "sai na próxima atualização deste computador"
  (`Refusal::LeavesAtNextApply`).
  - pronto quando: o fim do trabalho leva que a mudança espera, e a notificação diz isso.
- **Os programas da distro de fora do Nix (Flatpak, AppImage, web, Android do registro) não passaram
  pela vista**: a tabela do L04 só tem as AppImages dos Crafting Apps (2026-10-20), e nenhuma foi
  desenhada na central; o `Declarations::distro_of` acha um Flatpak e um app
  Android pelo `.desktop` e pela fonte (a instalação é uma só), e uma AppImage ou um web app da
  distro pela entrada do sistema, e nenhum foi desenhado.
  - pronto quando: o registro tiver um, e a foto das duas portas o mostrar recusável.

<!-- anchor: L19 -->

### A atualização (L19) — o que ficou

- **A câmera do rodapé não tem ponteiro**: o palco dela não anuncia `wl_seat`, então o realce da frase
  da atualização que quebra em várias linhas (todas as linhas acendem, `update_window::one_control`)
  é provado por teste e pela foto da janela, não por uma foto do rodapé.
  - pronto quando: a câmera do rodapé aceitar `--hover`/`--click`.
- **A razão do aplicador não tem frase na vista**: o `person-stage` termina uma recusa em `refused
  <palavra>` e o relatório guarda `failed-<palavra>` (`not-signed`, `rolled-back`, `too-large`…), mas
  a vista só tem frase para as palavras da máquina (`update_window::refusal_key`); só o `busy` é
  notícia declarada, com frase. As recusas de assinatura a página dos pertences já diz (o
  `signer.json`); as outras só o diário.
  - pronto quando: a vista disser a frase de cada razão do aplicador que um `person-stage` pode dar.

<!-- anchor: L20 -->

### O instalador com o repositório da pessoa (L20) — o que a pista não provou · médio

- **A restauração no primeiro boot nunca alcançou um repositório de verdade.** O
  `installer-offline-vm` prova a instalação sem rede e o primeiro boot esperando a rede; o
  `person-decline-chain` prova a sequência do primeiro boot com repositórios locais.
  - acabou quando: um teste com rede no primeiro boot (um nó `server` com um repositório git por
    SSH) termina com a home restaurada, o `person-applied.json` nomeando um programa declarado e o
    registro do primeiro boot apagado.
- **A primeira atualização de uma máquina instalada pelo ISO nunca rodou contra o release de
  verdade**: o `ronoctua-machine update-distro` desmira e move o input, com os testes da troca da
  string e do passo. Acabou quando: uma máquina instalada sem rede, numa atualização com o release no
  ar, termina com o `flake.nix` do template e uma trava do release, e o `apply` a constrói.
- **A espera do link numa máquina real**: o tempo para a rede escolhida conectar e para a medição foi
  escolhido, não medido.
- **As telas novas foram vistas como texto (`--show`) e como imagem** (o QR lido de volta pelo
  `zbarimg`). O ponteiro foi provado pelo estado da TUI (os testes clicam onde `places` diz que cada
  coisa está desenhada); ninguém clicou num terminal de verdade por SSH.


<!-- anchor: L21 -->

### Pertences: a lista do que vai para o repositório — o que o pacote deixou · pequeno cada

- **O `belongings-list-is-the-walk` foi avaliado, não construído.** A metade de Rust é o
  `rust.testsOf` de doze testes (`belongingsListing`), sobre o harness que as unidades constroem,
  sob a trava pesada; a mesma linha de `cargo test --exact` rodou na pista ("12 passed") e cada
  teste foi sabotado. A metade que lê fonte rodou fora do Nix, com o prelúdio extraído do
  `checks.nix`, e pegou as sabotagens (um leitor de `TREES` no `walk.rs`, um `NEVER` importado na
  shell, o `PLAIN` reexportado de novo no `plain.rs`, a lista caminhando sozinha, e os apelidos
  dentro de chaves: `kept::{self as k}`, `kept::{*}`, `kept::{Place, x::{TREES}}`, `crate::{kept as
  k}`).
- **A vigia das pastas da lista tem teto** (64 pastas e 64 árvores, cada árvore com o teto da vigia,
  `TREE_MOST`): uma casa acrescentada inteira com mais pastas do que isso tem a parte de baixo ouvida
  só na próxima contagem que outra coisa dispare.
- **Uma linha por arquivo, sem dobrar.** Uma pasta acrescentada com milhares de arquivos vira
  milhares de linhas na lista (e a barra rola). Dobrar um grupo grande é decisão do dono.
- **A câmera do rodapé ganhou `--wheel` e `--click`**, e as páginas de pergunta foram fotografadas
  pelo caminho do ponteiro (roda até o alvo, clique duplo, a pergunta; clique duplo no "não", a lista
  com o cursor à mostra). Nenhum teste da shell dirige o `Footer` para provar o seguir do cursor: a
  prova é a foto.
- acabou quando: o `.#most` passa com o `belongings-list-is-the-walk`.

<!-- anchor: L22 -->

### A barra do sudo (2026-10-11) — o que ficou · pequeno

- **A barra nunca conversou com um sudo de verdade.** As fotos são de estados plantados no rodapé
  (`photograph --door footer --sudo <estado>`), e a conversa — a volta da senha errada, a senha que
  o gesto guarda, a janela que vai embora — só rodou nos testes, contra roteiros; a conversa da
  porta (`authority::converse`) tem os testes dela, com dublês do sudo num terminal. Falta ver, numa
  máquina, uma janela pedir root e a barra responder com a senha, com a chave e com o dedo, e o
  rodapé morto dizer `NoFooter` na janela.
  - acabou quando: os três jeitos de responder e o rodapé morto foram vistos na VM ou na máquina, e
    um passo do `security-vm` (com o rodapé de pé) cobre a senha pela barra.
- **O cadastro de uma chave nunca tocou uma chave.** O `pamu2fcfg` roda como a pessoa
  (`u2f::Enrolment`), e a seção, as duas listas e o "parar" foram provados com credenciais de mentira.
  - acabou quando: uma chave cadastrada pela tela responde ao sudo num terminal e na barra.
- **Uma credencial com PIN (`+pin`) não sobe a barra.** O cadastro da tela é só de presença, mas uma
  linha feita à mão com `pamu2fcfg -N` e o `origin` nosso faz o `pam_u2f` pedir o PIN por um prompt
  dele, que não é sentinela da porta: a barra não aparece e o gesto espera até o prazo do próprio
  sudo. A conversa já aceita "usar a senha" e cancelar antes de qualquer sentinela; falta a barra
  subir no começo da conversa para oferecê-los.
  - acabou quando: com uma linha `+pin` na lista do root, a barra oferece "usar a senha" e `Esc`, e
    a senha passa.


### Cifrar qualquer coisa e a seção pós-quântica (L22) — o que ficou · pequeno

- **Os quatro checks novos foram provados fora do Nix.** `seal-never-overwrites`,
  `seal-secrets-are-files`, `transfers-never-link-age` e `every-channel-says-its-quantum-status`
  rodaram (o texto de cada um no `tests/checks.nix`, com os caminhos do store trocados pelos
  binários do `cargo` de perfil `loop`), limpos e sabotados uma vez cada; nenhum foi construído,
  porque pedem os programas de `release`. As cópias dos fatos dos dois perfis que o
  `every-channel-says-its-quantum-status` lê (a unidade do rodapé de `noctua` e de `minimal`) foram
  avaliadas e construídas à parte.
  - acabou quando: os quatro passam no `.#most`.
- **Abrir um arquivo cifrado para você pede a senha dos pertences mesmo com a sessão destravada**
  (L10 guarda a identidade no keyring do kernel). A identidade teria de chegar ao `ronoctua-seal`
  sem tocar o disco nem o `argv` — por exemplo, o filho lendo a chave da sessão ele mesmo, pela
  mesma descrição que o L10 usa.
  - acabou quando: com a sessão destravada, o `U` sobre um arquivo "para você" abre sem perguntar,
    e o check `belongings-identity-never-touches-disk` continua verde.

<!-- anchor: L23 -->
### A vista do Incus: o que a pista da vista deixou

- **A vista nunca falou com um daemon de verdade.** As fotos são de uma cena (`photograph --door
  incus --scene`, e o rodapé com `RONOCTUA_PHOTOGRAPH_INCUS_SCENE`) e os testes, de leituras montadas;
  a travessia REST → `Readings` é a do crate, que o `incus-vm` confere. O console, o shell e a tela
  nunca abriram um terminal de verdade: a foto diz o que a janela lançaria, e o rodapé de uma foto
  não lança nada.
  - acabou quando: uma cena `.#shot` (ou o `incus-vm`) abre `LEADER s x` sobre um daemon com uma
    instância, e a foto mostra a lista, o detalhe e um evento (`instance-started`) mudando o estado
    sem tecla nenhuma, o endereço chegando pelo relógio depois do DHCP, e um `systemctl restart
    incus` dizendo acima da lista que os eventos pararam e voltando sozinho; e `C-o` abre o console
    no nosso terminal. Os testes do feed só provam o handshake fora do laço contra um socket que não
    responde, e a reabertura contra um socket que não existe.


<!-- anchor: L24 -->

### Contas de LLM: a marca e a vista vistas numa sessão de verdade · pequeno

O módulo `agent-accounts` e a vista `m a` foram provados por testes (as duas vigias do módulo com
inotify de verdade numa pasta temporária, a unidade só depois de uso, o relógio só até o próximo
reinício) e fotografados nas duas portas sobre uma máquina posta numa pasta. Não foram vistos numa
sessão: o cérebro iniciando o `ronoctua-llm-usage.service` depois de uma conversa real com o Claude
Code, a marca mudando de cor quando a statusline grava os limites, o login de uma conta nova abrindo
no terminal e a sessão do Codex encontrando o `CODEX_HOME` criado pela vista.

- acabou quando: numa máquina com uma conta de cada agente, uma conversa no Claude Code muda a cor
  da marca dele sem relógio nenhum, o journal do brain mostra a unidade iniciada uma vez depois do
  uso (e não de novo antes de dez minutos), e "Acrescentar uma conta do Codex…" termina com o login
  feito no terminal que abriu.
- **As marcas na modeline em `.#shot`.** A foto do rodapé (`photograph --door footer --room <pasta>
  --accounts-marks`) mostra as três marcas com dígito e a cor que o módulo dá aos arquivos da sala;
  falta a cena de VM (F2) com elas vindas do brain de verdade, pelo socket.


<!-- anchor: L25 -->

### A instalação sem download e as partes — o que falta da refatoração · grande

O mecanismo das partes existe (`ronoctua-parts`, `nix/modules/parts.nix`; o porquê no
`docs/ARCHITECTURE.md`, "Partes: o que chega depois da instalação") e está provado numa VM, com o
vigia do brain, a página `s u d` e as quatro saídas de um download teimoso, e todo não livre da distro
é uma parte (a base não admite nenhum), com o passo `parts-plan` na atualização. O plano aprovado em 2026-10-23 segue
por marcos, cada um verde:

- **O que ficou do M4.** Uma recusa que diga "a caminho" em vez de "sem navegador"
  (`Refusal::NoBrowser` da central, quando o Chrome é querido e ainda não chegou).
- **O que ficou do M5.** Já são partes as Crafting Apps, o escritório, a oficina, o VLC, o Flutter,
  as toolchains do full, o pacote de ícones e as variantes de fonte (o full mediu 19,2 GiB antes das
  três últimas). Falta virar parte o que tem ligações mais fundas:
  - o Rust e o toolchain do devshell, que o `system.extraDependencies` guarda nos dois perfis para
    o `nix develop` offline (reescrevendo o `devshell-is-in-the-store`);
  - o Common Lisp, cuja ativação semeia arquivos pelo caminho do store;
  - o Emacs, com uma unidade própria e um `.path` no lugar do `services.emacs`, que põe o pacote no
    `systemPackages`;
  - o yt-dlp, cujo overlay refaz o mpv.

  Faltam também separar a `corrected` das fontes por variante (hoje a face do sistema é construída
  da árvore inteira) e os checks de cobertura: `every-download-has-an-owner`, `parts-hold-no-plugin`,
  `parts-do-not-collide`, `parts-build`, o lado Rust do `part-paths-are-read-as-arriving` e o
  `parts-absent-vm`. Por fim, a espera pela bateria abaixo do piso e os números do `profile.nix`
  medidos de novo.
- **O que ficou do M6.** A instalação sem rede está provada (`installer-offline-vm`), o ISO tem
  6,4 GiB e cabe num pendrive de 8 GB, e o ISO completo existe (`.#iso-complete`, nunca publicado).
  Ficou para depois:
  - **o hardware que espera a primeira rede só aparece pelo que lhe falta**: a página das partes diz
    os downloads que a câmera IPU6 ou o Parallels ainda pedem, mas não nomeia o hardware (o registro
    guarda os downloads, não o ramo que os quis);
  - **o primeiro boot na página das partes**: a restauração dos pertences aparece só no journal e na
    vista dos pertences, e não como uma etapa em `s u d`;
  - **um hardware que nenhuma sonda cobre** (um convidado VirtualBox ou Hyper-V, um módulo de kernel
    fora da árvore): falha no `build-first`, com o disco intacto e o que faltou dito; uma sonda a mais
    por ramo resolve, ao custo do tamanho do ISO;
  - **o ISO completo nunca foi construído nem instalado** (~25 GB, à mão), e a revisão do instalador
    não diz que o ISO é completo;
  - **cada edição da árvore gera um ISO novo**, porque ele leva a árvore da distro: numa rodada de
    teste convivem o velho e o novo (6,4 GiB cada) até a varredura do fim;
  - **o `iso-builds-only-glue`**, um ensaio contra um store só com o conteúdo do ISO, ficou coberto
    pelo teste de VM, que é lento.
- **O que ficou do M7.** A base se recupera como uma parte (o porquê no `docs/ARCHITECTURE.md`, "A
  base se recupera do mesmo jeito"). Ficou para depois:
  - **a página do rodapé com a linha do sistema não foi fotografada**: só a janela (as duas portas
    desenham as mesmas linhas, `Page::lines`), porque a câmera do rodapé pede o `machine-state/`
    plantado;
  - **os downloads enraizados incluem as fontes locais** (os patches do nixpkgs, as fontes filtradas
    da árvore): o `nix path-info` não distingue um `fetchurl` de um `builtins.path`, e o `export` os
    leva também — pequenos, e inúteis para outra máquina;
  - **a `apply` de uma distro sem fases** (anterior a elas) não sabe o `.drv` do sistema: ela não diz
    o que faltou nem enraíza os downloads.
- **M8 — as entradas da pessoa viram partes**, e o `verify`/`held.json` delas sai.
- **M9 — documentação, tamanho e release.**

- acabou quando: o Z16 instala do ISO novo sem cabo e sem Wi-Fi até a sessão, e depois, com rede,
  as partes chegam uma a uma em `s u d`, uma que falha não segura as outras, e entregar o arquivo
  de um AppImage baixado no navegador a traz.

### As fotos da leva (F2) — o que ficou · pequeno cada

Cada vista nova da leva foi fotografada nas duas portas e nos dois idiomas (lista no relatório da
fase). O que a foto achou e não foi consertado nela:

- **"Instalar só para você" não se fotografa.** A câmera da central de programas não tem catálogo
  na sala (`Tools::default()`), então a busca não acha nada e a página de instalar não se abre.
  Acaba quando a sala aceitar um índice plantado e a página aparecer nas duas portas.
- **O Incus em três painéis, perto de 110 colunas, corta valores** (`500 processos, de um p…`,
  `criada e ligada pela fi…`): o painel do detalhe fica com ~24 células de valor. Em 81 e em 150
  colunas tudo cabe. Acaba quando o detalhe quebrar o valor como `chrome::aligned_pair` ou ganhar a
  largura que a lista do meio não usa.
- **Um caminho sem espaço quebra no meio do nome** (`…/software/history.jso` + `nl`, na lista dos
  pertences): `layout::wrap` corta a palavra longa na célula. Acaba quando o corte preferir depois
  de uma `/`.
- **A lista de passos da janela da atualização rola sem barra** (a única barra é a da saída, por
  decisão escrita no código); com os registros no topo, sobram sete passos visíveis em 30 linhas.
- **A recusa embutida começa com maiúscula depois de dois-pontos** (`não foi lido: O login venceu:`,
  nas contas LLM).
- **No rodapé, a barra lateral do gerenciador não mostra o cabeçalho de pendrive**: ela lê o udisks
  da máquina, e só a câmera da janela planta drives (`--blank`, `--luks`). O mesmo vale para os
  registros da máquina na vista da atualização do rodapé, que pedem `machine-state/` plantado.


## A leva de 2026-10-12

A granularidade do build e o shell em crates (o porquê no `docs/ARCHITECTURE.md`, seção 10). O que
cada passo deixou por fazer, por assunto.

### O build em unidades — o que ficou · pequeno cada

- **A fase 1 das camadas ainda existe** (`nix/rust/layers.nix`, `nix/scripts/rust-layer.py`,
  `rust-index.py`, `test_rust_index.py`, `rust.layers`/`layersWith`, o `depsFrom = "layers"` e o
  caminho da camada do executor). Só o `.#profile-bench` a usa, para medir cada variante sobre a
  camada dela. Portá-lo para as unidades de terceiros (o frio = a soma das unidades, os codegen
  units conferidos no `rustc.json` de cada uma) e então apagar tudo — numa leva em que a
  reconstrução de toda unidade já vá acontecer, porque o executor é entrada de todas. A regra de
  fronteira de caminho da fase 1 (`under`, `paths_in`) é a antiga e sai junto (latente: nenhum
  pacote `links` do plano imprime um caminho colado numa flag).
  - acabou quando: nenhuma derivation nomeia uma camada, e o `.#profile-bench` mede sobre as
    unidades.
- **Os outros apps que constroem um sistema não são dimensionados**: `.#apply-here`, `.#vm`,
  `.#iso`, `.#install`, `.#shot` e `.#release` rodam `nix build` com o `max-jobs` do `nix.conf`.
  Depois de uma edição, compilam sem conta.
  - acabou quando: cada um constrói pelo `ronoctua-sized-build` (o `.#apply-here`, como o apply,
    pelas `ronoctua.machine.phases`).
- **O ISO não leva as unidades**: a semente tem só os programas, e as libs têm `allowSubstitutes =
  false`. A primeira atualização depois de instalar que muda o código de um crate compila tudo o que
  está abaixo dos programas que mudaram — se mudou o shell, toda lib abaixo dele. Levar o
  `rust.release.keep` (~1,95 GiB) na semente custaria isso no ISO: decisão do dono.
- **`shellMiB`/`besideMiB` são do shell de um crate só** (o harness e o programa de release a 1, 3 e
  6 núcleos, 2026-10-15, pelo RSS do maior processo). A pesada de agora (`nix/rust/unit-classes.json`)
  é só o programa de release do shell (4 121 MiB a doze núcleos, do que o orçamento paga no build
  inteiro, M1); medi-lo a 1, 3 e 6 núcleos — o `metrics` agora diz os núcleos — daria a ele mais
  núcleos.
  - acabou quando: os números do `nix/build-sizing.nix` são das pesadas de agora, e o `.#rust-peaks`
    passa.
- **O teto do cgroup do root só é provado nas funções** (`sizing_cgroup_ceiling` sobre pastas
  plantadas).
  - acabou quando: um teste de VM roda o apply como root numa unidade com `MemoryMax` e o diário diz
    o orçamento daquele teto.
- **Testes de tempo de parede ao lado de uma compilação**: as baterias, o `.#sized-build` e por
  ele o `.#quick` rodam as execuções de teste depois de toda compilação (as últimas linhas do plano),
  mas um `nix build` à mão ainda as põe ao lado de um `rustc` (o teste de 20 ms do
  `ronoctua-broadcast` falhou assim uma vez).
  - acabou quando: o teste mede tempo de CPU.
- **As fatias leve e média foram simuladas sobre o RSS** (F5: o maior processo, com as páginas de
  arquivo), e o pico de agora é do que o orçamento paga (M1): das 2 319 unidades a um núcleo, 2 055
  picam até 256 MiB e 2 253 até 512, onde a fatia leve é de 768. Com doze processadores e ~8,3 GiB de
  orçamento, a leve de 768 já põe dez de uma vez; uma menor só ganha onde o orçamento, e não os
  processadores, limita. Simular de novo (a mesma simulação do F5, sobre os picos novos) diria se
  outra fatia ganha.
  - acabou quando: as fatias são as que a simulação sobre os picos de agora escolhe.
- **A grande espera atrás das leves**: na edição do xdg, a biblioteca de release do rodapé fica pronta
  aos ~167 s e começa 70 a 72 s depois (M1, com a medida do que o orçamento paga; 71 a 94 com a do
  PSS), porque o `sized_units` começa a primeira pronta que cabe, na
  ordem da cadeia em unidades, e ela, com só o programa do shell acima, vem atrás das leves de cadeia
  mais alta; quando sobra a fatia dela, já não há mais leves. Uma simulação do agendador sobre o
  plano, as unidades e os tempos de uma rodada reproduz a rodada a ±2 s, e nenhuma regra simples
  ganhou em todo orçamento: começar primeiro a unidade de mais de um terço do orçamento e
  guardar o lugar dela deu −30 s a 8 526 MiB, −12 a 8 366 e +79 a 4 096; a ordem pelo caminho
  crítico em segundos (com os tempos de verdade) deu +71 s a 8 366.
  - acabou quando: uma regra medida em mais de um orçamento põe a grande no build quando fica pronta
    sem atrasar nenhum conjunto além do ruído.
- **O `.#rebuild-cost -- <arquivo>` não simula um dado**: só reescreve fonte de crate, então num
  `.ftl` diz 0 entradas e numa semente recusa, onde a edição de verdade move 67 e 65 execuções de
  teste (F5). Sem argumento ele compara com o `HEAD`.
  - acabou quando: um `.ftl` e uma semente passam pelo `texts.override { fileEdits }` e pelo
    `themed.override { seedEdits }`, como nos checks de granularidade.
- **Pipelining por `.rmeta` nos terceiros**: com o mesmo número de threads que o cargo, os terceiros
  sozinhos custam 1,48–1,72× a camada (o caminho crítico cranelift-codegen → wasmtime espera cada
  rlib inteiro). O frio inteiro já é 1,01–1,03×; só vale se cortar 20% de algo que se sente.
- **O `DEP_*` de um pacote `links` cujo build script roda mais de uma vez no mesmo grafo**, dizendo
  coisas diferentes, não é atribuído (o relatório do cargo não diz qual execução é qual). Nenhum
  build script nosso depende de um assim; a unidade que dependesse recusaria dizendo por quê.
- **O rustfmt das unidades não alcança o `module.rs` dos módulos do brain**: eles entram por
  `#[path]` em código gerado, e nenhum `fmt.<crate>` os nomeia — só o `nix fmt`.
  - acabou quando: o `fmt` do daemon passa cada `module.rs` como raiz.
- **`graphics-rpath-only-where-graphics-are` constrói todo programa de release** (está no `most`); o
  RUNPATH já está num atributo da derivation (`passthru.graphics`), e o check pode virar avaliação.
- **O `ronoctua-design` lê `../ronoctua-protocol/wit`** pela feature `guest` (os buffers): um
  caminho que sai do crate. O host não compila a feature e nenhuma unidade o vê; o `nix/buffers.nix`
  remonta a árvore para isso.
- **A ponte Tor não liga o programa do transporte**: o anel `bridge-transport` escreve
  `ClientTransportPlugin <palavra>` sem `exec <caminho>` (as variáveis `RONOCTUA_TOR_OBFS4` e
  `_SNOWFLAKE` não eram lidas e saíram). Ligar é feature: o `lyrebird` e o `snowflake` continuam
  instalados pelo `nix/modules/tor.nix`.
- **O estúdio ganhou a libwayland no RUNPATH** (a cópia do crane a tirava, e a captura de tela dele
  não achava a biblioteca numa máquina instalada). Nenhum teste de VM fotografa a captura do
  estúdio.
- **O palco do `.#shot` tem no PATH só os programas do bundle**, o cérebro, a fila e o player: a
  cena do ranking (`ronoctua-video-metrics add`, `ronoctua-index query`) precisa pôr
  `rust.videoMetrics` e `rust.index` no palco ao nascer.
- **`declare-installs-now-and-records` é um teste de VM com nome de check** (sem `-vm`): quem filtra
  as VMs pelo nome sobe uma sem querer.
  - acabou quando: o nome termina em `-vm`.

### O perfil de release e o tamanho do programa · horas de medida, e uma decisão do dono

- **A decisão do perfil (a variante E) foi medida com o shell num crate só e a dois núcleos.** O
  `.#profile-bench` agora constrói cada variante dimensionada (N4), e o override de dezesseis
  unidades de código nomeia o `ronoctua-shell`, que é hoje um binário fino de ~800 linhas: medir de
  novo, e decidir se o override nomeia os crates grandes que nasceram (o rodapé, o kit, a busca, o
  jogador) ou sai.
  - acabou quando: uma medida nova do `.#profile-bench` está no `docs/ARCHITECTURE.md` e o `chosen`
    é o que ela escolheu.
- **O programa do shell cresceu**: 133,8 MB em release na linha de base, 147,5 MB com a variante E e
  163,7 MB com o shell em ~50 crates (genéricos monomorfizados em cada crate que os usa; o LTO fino
  não junta). A regra do perfil não julga tamanho: se ele deve julgar (um teto no fecho do sistema),
  é decisão do dono, e a medida roda de novo com ela.
- **A resolução do `.#bench`**: o shaping de produção sai com 0,1 ms e a instanciação com 0,001 ms;
  nesses números a regra de 3% só vale até um dígito impresso. Imprimir mais casas nos quatro
  programas de bench deixaria a regra morder ali também.

### O protocolo da região: as formas duplas · médio cada, com a matriz para provar

As nove famílias do R5 estenderam o mesmo protocolo em paralelo, e a integração deixou duas formas
de algumas ideias, cada uma documentada onde mora (`ronoctua-shell-door/src/region.rs`). Juntar um
par muda o desenho das regiões que usam a forma que sai:

- o que o ponteiro acende: `Region::hover_span`, `Region::lit`, `Region::band` (a faixa posta em
  pixels) e `Told::lights` — juntar o `band` ao `lit` pede a `Area` no `lit`;
- o estado que sobrevive à região: o `Kept` que o rodapé empresta por tipo (as linhas da máquina) e
  o `Resident` de uma funcionalidade; e o empréstimo dos campos do rodapé (`Lent`) ao lado do
  `Kept`;
- o ponteiro sobre os controles da região: `pointer`/`rests_on`, `grab`/`drag` e `wheel`, ao lado de
  `hovered`/`dragged`/`pressed`/`released`/`left`/`wheeled` com o `Under` e a mão;
- os ganchos do quadro: `before_frame`/`after_paint`/`after_frame` e
  `before_measuring`/`laid_out`/`painted`/`after_the_frame`/`framed`;
- o cursor de texto (`Told::caret` e `Region::caret`) e as figuras (`held_picture`, `picture`,
  `pictures` e `cover_picture`);
- a página: `Doorway::follow` (a superfície primeiro) e `Paging::Follow` (só o cursor, só com a
  região na tela); `Region::take_anchor` e `Paging::Top`; `Region::language_changed` e
  `Region::speak`;
- a altura depois de uma notícia: as regiões que dizem `rows_changed` respondem
  `says_when_its_rows_change`, e para as outras o hospedeiro mede de novo depois do quadro; e
  `Doorway::resized` ao lado de `rows_changed`.
- acabou quando: cada ideia tem uma forma só, e a matriz da linha de base sai igual.

### O ponteiro acende o que o clique toma (H1) — o que ficou · pequeno cada

Cada vista diz agora, linha a linha, o que o clique toma, e as duas portas acendem por essa resposta
(`ARCHITECTURE.md`, "O ponteiro acende só o que o clique toma"). Ficou:

- **Empilhado, a metade de um registro aberto que tem o teclado não se vê.** O divisor que a acende
  só existe lado a lado; o clique que dá o teclado aos fatos acende, e a mudança só aparece na seta
  seguinte. Acabou quando os dois arranjos mostram onde o teclado está.
- **Oito janelas sem câmera**: histórico do shell, notificações, som, energia, ponteiros, Syncthing,
  manuais e geo — a luz delas foi provada por teste, não por foto. Acabou quando
  `photograph --door <cada uma>` desenha a janela.
- **Bluetooth**: um dispositivo com `wake: None` e sem som desenha a metade de ajustes que nem o
  ponteiro nem o `TAB` alcançam. Acabou quando a página só desenha o que se alcança.
- **Acesso rápido**: a frase de recusa de uma entrada de playlist remota é descartada (a linha acende
  e o clique não diz nada), e todo clique leva o foco ao detalhe, mesmo onde não toma nada. Acabou
  quando a recusa é dita e o clique que não toma nada não move o foco.
- **Gerenciador de arquivos**: o ar com marcas toma o clique (larga as marcas) e não acende — fica
  para o dono decidir —, e o globo e uma linha de dispositivo sem estado de montagem divergem entre
  `View::takes` e `View::click`. Acabou quando o dono decidir o ar e as duas respostas forem uma.
- **Linhas quebradas de um ajuste** na segurança, nos agentes, nos pertences e nas configurações do
  rodapé tomam o clique e acendem só na primeira linha: coerente, mas diferente do pendrive, dos
  programas, da calculadora e do Incus, que acendem a frase inteira nas duas portas
  (`Region::control_of` no rodapé, `Content::controls` ou a conta da própria janela). Acabou quando
  toda frase quebrada que é um controle acende inteira nas duas portas.
- **O custo por quadro do gerenciador de arquivos**: a luz agora dispõe a coluna sob o ponteiro uma
  vez por quadro (`View::lit_at`), além da disposição de todas as colunas que a pintura já faz:
  numa pasta de 10 000 entradas, ~83 µs contra ~100 µs da pintura (eram cinco disposições, ~410 µs;
  `files::tests::the_pointers_cost_a_frame`, perfil `loop`). Acabou quando a luz ler as seções que a
  pintura dispôs, ou quando `nix run .#bench` medir o quadro com o ponteiro sobre a listagem.

### O shell em crates — o que ficou · pequeno cada

- **`run-command-handles-every-key` e a lista à mão `WITH_ACTION`** (`keymap.rs`) podem sair quando
  o `every_command_has_an_owner` perguntar ao `Region::command` e ao `closed` de cada dona se ela
  toma cada comando seu.
- **O `CONCRETE` do rodapé está vazio** (toda vista de funcionalidade é região): ele e os passos dos
  checks que o liam (`a-region-leaves-nothing-behind`, `the-slot-host-names-no-feature`) podem sair.
- **`notif-dismiss` é do `notifications::KEYS` e o rodapé o responde** (a faixa): o teste do binário
  o lista como exceção escrita (`THE_FOOTERS`). Passá-lo ao `leader::BASE` muda o dono no
  `check-keymap --dump`: decisão.
- **Uma palavra da linha de comando que a porta LÊ e não é UTF-8 para o programa** (`files
  <caminho>`, `git-browser <pasta>`, `software-file`, `buffer`, `play`, os programas do
  terminal/monitor/resgate). As tabelas de portas guardaram o comportamento (só o que a porta lê
  vira texto, e o que não é texto para com uma frase); a janela do histórico do git, aberta por
  `Launching::Program` (`args: Vec<String>`), perde os bytes de uma pasta que não é texto. Ler como
  `OsString`/`PathBuf` até o fim, ou recusar com frase, é mudança de comportamento: decisão do dono.
- **O conjunto da base está escrito em quatro checks** (`the-shell-foundation-names-no-feature`,
  `a-view-stands-on-the-foundation`, `the-file-manager-stands-on-the-foundation`,
  `footer-names-no-feature`), iguais hoje; um lugar só (no `shellManifestPrelude`) evitaria que um
  divirja.
- **A frase de download do catálogo de vídeos ensina a fila sem o rótulo do líder**
  (`video-metrics-downloading` diria `t` onde as outras portas dizem `SUPER t`): o
  `video_metrics_window.rs`, a câmera dele e a região usam `bindings::keys`, e as outras portas
  `bindings::taught`. Defeito de antes da leva, guardado igual.
- **A chave do Atuin (`Errand::ShowKey`) roda `atuin key` no laço**, nas duas portas; o conserto é
  mandar a ordem à thread do Atuin.
- **A recusa do udisks com o gerenciador fechado** (trancar um pendrive ocupado e fechar antes da
  resposta) não diz "ocupado", nem na base; a região que fica ainda tem a vista, e a frase custaria
  uma linha no `reported_closed`. Muda o que a base fazia: decisão do dono.
- **Abrir por cima é uma regra genérica** (`Footer::region_free_for`): um comando que a região
  MOSTRADA pede e que abre uma região ranqueada acima dela abre sem a pergunta de região livre. Hoje
  só o `git-history` do gerenciador a usa; se o dono a quiser só para quem a pede, é um
  `Closed::Over`.
- **Comportamento novo que o dono pode querer de outro jeito**: os comandos de tecla do clipboard
  rodados pelo nome com a vista fechada (`clipboard-remove`…) abrem a vista e agem nela.
- **A câmera do rodapé não alcança o menu da janela** (pergunta ao `SCROLLSOCK` de verdade, não ao
  compositor substituto), e a região de ícones dela mostra "Lendo as tabelas de ícones…": ela não
  espera a thread que lê o `RONOCTUA_ICONSET`, como a câmera da janela espera.

### Os valores como dado e o brain — o que ficou · pequeno cada

- **As metades compartilhadas dos módulos fazem E/S na biblioteca do brain**
  (`system-update/shared.rs` lê a bateria e os registros da máquina; `agent-accounts/shared.rs` lê
  as contas e escreve as linhas da pessoa): o rodapé chama essas funções direto, e por isso a
  biblioteca depende de `ronoctua-memory`, `-machine`, `-person`, `-belongings` e `-llm`. Trocar a
  leitura direta por dado do fio muda o comportamento (a janela vazia com o brain parado): decisão
  do dono.
- **A biblioteca do brain compila o manifesto de TODO módulo da distro**, embora o rodapé precise só
  dos que um `namedBy` lê (`clock_sync`, `system_update`, `video_metrics`, `agent_accounts`): uma
  marca nova num módulo da distro recompila o rodapé. E as tabelas de um módulo (`tables.nix`) vão
  para a biblioteca pelo manifesto, as do disco (`cache.rs`) e dos lembretes (`words.rs`) inclusive,
  que só o `module.rs` lê.
- **A gramática dos lembretes** (`reminders-grammar-*`) é valor de texto compilado na biblioteca do
  brain, e a tabela dela lê o `gen-i18n.py`: editar o gerador dos textos recompila 44 bibliotecas de
  release. Levá-la ao catálogo, ou dar à tabela um leitor próprio, fecha a última rota do
  `texts-are-data`.
- **Um glifo de módulo da pessoa que a fonte não tem** para o build é recusado pelo `measure` do
  catálogo, no build da máquina inteira; recusar na avaliação pediria o `glyphnames.json` na
  avaliação (import-from-derivation) — a alternativa é medir por módulo e deixar de fora o que
  falha.
- **O `mimeDb` lê o `icons.json` inteiro do glyphdb**, e o `syntax.rs` o lê: um ícone do chrome
  editado recompila os seis buffers. Um `mimeDb` que lesse só os tokens que usa cortaria isso.
- **Um glifo desenhado por vários tokens mostra um só** no navegador de ícones (o último em ordem de
  nome, a regra do glyphdb).
- **240 chaves de texto sem quem as diga no código** (`hint-volume`, `group-geography`,
  `sidebar-fact-*`, `cmd-file-save`, `atlas-*`…): foram atribuídas por regra de nome. Tirá-las muda
  o conjunto de mensagens.

### Checks oferecidos na leva · pequeno cada

Cada um com o que pega; nenhum foi escrito:

- `bench-reads-the-renderers-field` — o campo que o `nix/bench-frames.nix` procura é o que o
  `ronoctua_render::gpu` registra (`milestone =`); ~1 s, só leitura de fonte.
- `keymap-dump-is-total` — o `check-keymap --dump` tem uma linha por binding de cada conjunto e todo
  comando do registro; ~10 s.
- `profile-variants-stay-off-the-evaluation-path` — nenhum `.nix` fora do app e do `nix/rust.nix`
  nomeia `profileVariants`/`profileBench` (o import-from-derivation delas); ~1 s.
- `wrappers-set-what-is-keyed` — cada nome de `rust.keyed` vai com `--set`, nunca `--set-default`,
  em todo embrulho; ~1 s, avaliação.
- `every-binary-carries-its-data` — a lista dos programas que falam ou pintam sai do grafo (quem
  liga o `ronoctua-i18n`, o `ronoctua-tokens`), e cada um é embrulhado; ~1 s.
- `the-keymap-is-installed-at-start` — o binário instala o keymap antes de qualquer porta (sem isso
  toda tecla ensinada sairia vazia, calada); ~1 s.
- `the-foundation-stands-on-no-daemon` — a lista dos crates de fronteira de que o kit e a porta
  podem depender, cada um com o porquê; segundos.
- `every-key-is-said` — toda chave de texto é dita (constante, literal, id ou prefixo) — mostraria
  as 240 órfãs; ~5 s.
- o vigia do `.#dev` vivo depois de cada ramo — o bloco de recarga construído, o `ronoctua-reload`
  de verdade e `cargo` e rodapé de mentira: um valor editado, uma chave somada e um `.rs`, e o vigia
  de pé depois dos três; ~25 s.
- `brain-daemon-halves-reexport` — cada módulo da biblioteca com metade no daemon é reexportado no
  módulo homônimo do daemon (um item que faltasse faria um `module.rs` da pessoa parar de compilar
  longe da causa); ~1 s.
- `chrome-icons-match-the-text` estendido aos glifos medidos do catálogo do brain; o custo do de
  hoje.
- `our-crates-are-built-unit-by-unit` reconhecendo a vendorização do nosso lock pelo conteúdo do
  `Cargo.lock`, e não pelo caminho (um `importCargoLock` do nixpkgs passaria); o mesmo custo.
