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

### 0.0000001. A opção "usar IA" da linha de resgate · médio

A linha que aparece quando o rodapé cai (`ronoctua-shell lifeline`) oferece quatro coisas, e três
funcionam: ver os logs, reiniciar e fechar. A quarta é **desenhada e recusa**, com frase própria — a
pedido do dono, que pediu a opção existindo e a implementação para depois: *"deixar a implementação
dessa funcionalidade de no pending, pois será implementada mais tarde"*.

O que falta é o que ela faria: pegar o que já está na mão naquele instante — a unidade, o
`Result=`, as últimas linhas do journal, o coredump se houver — e levar a alguém que responda. Isso
é uma decisão de rede e de privacidade antes de ser uma de código: o que sai desta máquina, para
onde, e com o quê. Nada disso está decidido.

- **como saber que acabou**: a quarta opção deixa de recusar, e o que ela manda está escrito na tela
  antes de sair daqui.
- **o que NÃO fazer sem decidir**: mandar log a lugar nenhum por padrão. O invariante 13 vale aqui
  como em qualquer outro lugar, e um backtrace carrega caminhos de arquivo.

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

### 0.0000001-D. O país da coordenada acende pela CAIXA · pequeno

O `Ink::Here` acende todo país cuja caixa envolvente contém o ponto (`ronoctua-atlas/src/globe.rs`):
uma coordenada em Londres acende também a Holanda e a Rússia. O atlas já sabe dizer em que país um
ponto está pelos anéis (`lookup::country_at`, o que o endereço usa); o acendimento pergunta só à
caixa. Anterior à reforma do player, visto nas fotos do globo.

- acabou quando: uma coordenada acende só o país cujos anéis a contêm, com um teste que planta um
  ponto em Londres e confere que só o GB acende

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

**Uma tarefa de execução única ainda não existe.** `Task::kind` existe, a WIT a carrega, a listagem
desenha o glifo e a cor, e as quatro tarefas de hoje são rotineiras. A distinção nasceu antes da
primeira de propósito: acrescentá-la no dia em que uma aparece mudaria o sentido de toda linha da
página de uma vez.
**Como saber que acabou:** uma tarefa que roda, escreve o que fez, e sai da lista.

### 0.0000003. Tirar a coordenada de um arquivo antes de enviá-lo · pequeno

A ficha mostra onde a fotografia foi tirada, porque esconder um fato de quem já tem o arquivo não
protegia ninguém (o `docs/ARCHITECTURE.md` guarda o argumento inteiro). O que MUDOU com isso é a
escala: cinco decimais numa linha passam despercebidos numa tela compartilhada, um globo apontando
para a casa de alguém não passa.

O que resolve isso não é esconder de novo — é um COMANDO que remove o GPS do arquivo antes de ele
sair daqui, no menu de ações do gerenciador, ao lado de copiar e renomear. O `exiftool` que o flake
já pina faz (`-gps:all=`), então não há programa novo; o que falta é decidir se ele escreve no
arquivo ou numa cópia, e dizer na tela qual dos dois.

- **como saber que acabou**: uma ação no `C-m` que apaga a coordenada, e a linha do lugar
  desaparecendo da ficha do mesmo arquivo em seguida.
- **o que NÃO fazer sem decidir**: mexer no arquivo original sem perguntar. O gesto que se desfaz
  com `Esc` é o padrão da casa, e este não se desfaz.

### 0.0000002. Um rodapé que TRAVA não é um rodapé que morre · pequeno

`OnFailure=` vê processo que morre. Um que para de responder — laço apertado, GPU travada, espera
sem prazo — continua `active` e a linha nunca nasce. O que resolveria é um `WatchdogSec=` na unidade
e um `sd_notify` no laço de eventos, e as duas coisas são um relógio novo numa porta cujo desenho
inteiro é não ter relógio em repouso.

- **como saber que acabou**: matar o rodapé com `SIGSTOP` levanta a linha.
- **o preço**: um `sd_notify` por intervalo, para sempre, em troca de um modo de falha que ninguém
  relatou ainda.

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

### 0.0000028. A folga do disco — o que só uma máquina da distro prova · pequeno cada

A folga (`docs/ARCHITECTURE.md`, "Quando o disco aperta") foi provada por teste com árvores
plantadas, por sabotagem e por um ensaio lido num disco real; estas coisas não:

- **A unidade no sandbox dela.** Que `nix-store --gc --print-roots` alcança o daemon sob
  `ProtectSystem=strict`, `PrivateNetwork` e `SystemCallFilter=@system-service`, como a pessoa, está
  medido (uma unidade transiente com as mesmas propriedades: 369 raízes, saída 0); falta o
  `--gc --max-freed`, que apaga, e a unidade de verdade numa máquina da distro. Um `space-vm` (~90 s):
  um btrfs pequeno cheio até abaixo do piso, uma árvore do cargo e lixo no store plantados; o
  cérebro dispara a folga, o disco volta à meta, a raiz plantada e a geração ficam.
- **O caminho inteiro até a linha.** Cérebro → unidade → relatório → `Relieved` no `Mind` →
  `disk-relieved` no minibuffer, e `disk-still-short` quando nada mais sai. Cada pedaço tem teste;
  a costura, não.
- **A página da tarefa mostra cadência e janela** ("roda a cada 7 dias", "entre qualquer hora"),
  que uma tarefa sem calendário não usa — como as duas vigias já mostram. A linha da lista diz o
  certo ("quando falta disco"); a página precisaria esconder as duas linhas quando `why_not` diz que
  não há calendário, e isso é o buffer do cérebro, com foto.

- **A aparagem numa troca de verdade.** Que o trecho de ativação grava
  `/run/ronoctua-generations/previous-system` antes de `/run/current-system` mudar, que a
  `ronoctua-system-generations-at-activation` roda a cada `switch` sem derrubá-lo, e que o menu de
  uma ESP real (systemd-boot) é lido. Provado em fixture com o `nix-env` de verdade, não numa troca.
  Um `store-vm` (~75 s): gerações plantadas, uma especialização, um rollback e uma troca.
- **A montagem por bind.** `STATX_ATTR_MOUNT_ROOT` foi provado em `/` e num diretório comum; um
  bind dentro do cache precisa de root. Um subteste de VM planta um e roda as duas varreduras.
- **A conversão do `~/.cache` num boot de verdade**: que ela vê `starting`, que o login espera por
  ela, e que o limite de 10 minutos corta e o bilhete segura a próxima. Provado com um
  `systemctl` substituto; o `cache-subvolume-vm` oferecido prova o resto.
- acabou quando: o `space-vm` passa e a foto da linha "espaço aberto no disco" está numa cena

### 0.0000029. O rodapé transparente — o que só a GPU do dono prova · minúsculo

Nos modos ocultos (`LEADER LEADER`, `LEADER Alt LEADER`; `docs/ARCHITECTURE.md`, "Os modos
ocultos"), o que empurra as janelas fica sobre a cor da mesa, opaca, e não depende de nada. O que
depende de a superfície do rodapé aceitar o modo `PreMultiplied`
(`ronoctua_render::gpu::alpha_mode_for`) é o que fica POR CIMA das janelas: o véu (o minibuffer
sozinho e a lista da leader nos dois modos, e tudo no `Cover`) e o pixel transparente da borda em
repouso. Os testes provam a escolha do modo e a limpeza multiplicada; se o driver da máquina do dono
oferece o modo, só ela responde. Sem ele o rodapé fica opaco, registra
`the surface cannot be see-through here` e diz `echo-footer-opaque` uma vez: as barras somem e fica
uma faixa de chrome.

- **como saber que acabou**: na máquina do dono, com `LEADER LEADER` o minibuffer sozinho (um eco)
  e com `LEADER Alt LEADER` uma vista aberta mostram a janela através do véu, a linha de baixo da
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

### 0.0000035. O que a câmera ainda não faz · pequeno a médio cada

- **Câmera MIPI atrás do libcamera** (muitos laptops novos com sensor Intel IPU): ela não é um nó
  V4L2 de captura comum e não aparece. Acaba quando uma dessas é fonte, provada numa máquina que a
  tem. Médio.
- **Formatos multiplanares** (`VIDEO_CAPTURE_MPLANE`) são ignorados pela varredura. Pequeno.
- **Duas cadeias de efeito na mesma câmera** (uma para a prévia, outra para a câmera virtual): hoje o
  efeito é da FONTE. Acaba quando o `Sends::Camera` pode levar uma cadeia própria. Pequeno.
- **Texto como fonte de cena**: declarado no plano, não entrou. Pequeno.
- **Recuo para `wlr-screencopy`** quando o compositor não anuncia `ext-image-copy-capture-v1`. O
  pinado anuncia; outro compositor pode não anunciar. Pequeno.
- **A instância `.#dev` grava de verdade**: o plano dizia que ela só ENSAIARIA, como o cérebro, e
  ela não ensaia — um laço aninhado abre a câmera e escreve em `Videos`. Acaba com a decisão do dono
  sobre isso. Minúsculo.

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

### 0.0000037. `/var/lib/ronoctua-browser` não sobrevive à impermanência · pequeno

`nix/modules/persist.nix` não guarda o diretório. Com impermanência, a chave e o contador da versão
renascem a cada boot, e a extensão ganha outro id a cada boot: é outra extensão para o Chrome (o
comentário da chave em `nix/modules/browser-extension.nix` diz o que se perde com isso). O defeito é
anterior ao quiosque.

- acabou quando: `/var/lib/ronoctua-browser` está em `persist.nix`, e dois boots da `.#vm` dão o
  mesmo id

### 0.0000038. O redimensionador de tela: o que ficou de fora · pequeno cada

- **O cursor de redimensionar sobre as bordas.** `cursor_shape.rs` pede a seta a cada `enter`, e a
  porta precisa de um gancho para pedir outra forma sobre uma alça.
- **Tamanhos prontos**, além das setas e do arrasto.
- **A captura limitada à sala.** Hoje ela usa a saída inteira, cortinas inclusive.
- **Lembrar o último tamanho.** Ligar de novo começa do maior.

- acabou quando: cada um existe, ou está escrito no `ARCHITECTURE.md` por que não

### 0.0000039. As colunas: o que ficou para depois · pequeno cada

- **O shim do `column.pid` antigo** (`right_column::LEGACY`): sai quando nenhuma coluna aberta
  por um binário anterior ao slot por espaço puder estar de pé.
- **Duas telas de verdade**: que cada uma mostra a coluna do seu espaço, e que uma tela que some
  estaciona a coluna. O `player-vm` tem uma saída só.
- **`about-column-widen` existe e ninguém a lê**: a barra não tem frase (decisão do dono). Decidir
  se a chave fica para um manual ou sai dos dois `.ftl`.

- acabou quando: o shim saiu, o teste de duas telas existe, e a chave tem destino

### 0.00000395. Os checks oferecidos do redimensionador, das colunas e do quiosque · pequeno cada

Foram oferecidos com a entrega e não foram escritos. O que cada um pega:

- `layer-surfaces-are-the-ones-we-know` — censo de `create_layer_surface`: uma superfície
  layer-shell fora das conhecidas (rodapé, linha de vida, coluna, captura, cortinas) derruba o
  build. É o inv. 14 conferido por máquina.
- `a-column-is-a-column` — só `column.rs` cria superfície no namespace `ronoctua-column`: uma coluna
  que não passa pela base perde tamanho, linha de foco e espaço de trabalho.
- `focus_line` único em `separators-come-from-chrome` — a linha de foco só sai de
  `chrome::focus_line`.
- `one-browser-boundary` — só o `ronoctua-browser` fala o enquadramento do native messaging, lê o
  `Preferences` de um perfil e monta a linha de comando do quiosque.
- `native-hosts-take-chromes-origin` — comportamental e sem VM: cada host de native messaging,
  chamado com `chrome-extension://<id>/` como primeiro argumento, responde ao enquadramento.
- `extension-version-moves` — sem VM: duas builds diferentes da extensão saem com versões
  crescentes, e a mesma build mantém o número. Hoje só o `extension-vm` confere.
- `browser-socket-is-private` — o socket da ponte nasce no diretório de runtime da pessoa, nunca num
  diretório compartilhado.
- `extension-reads-extensions-only` — a permissão `management` só lê a lista (`getAll`): nenhuma
  chamada liga, desliga ou remove extensão.
- `resizer-vm`, no `slow` — liga o redimensionador, arrasta, confere a sala pelo retângulo que o
  compositor dá ao espaço, e desliga.

- acabou quando: cada um existe, foi quebrado de propósito, a mensagem exata foi vista e o arquivo
  restaurado

### 0.000004. O que falta de `LEADER s l`, e o relance que não existe · médio cada

A vista de idioma e região existe nas duas portas (`s l l` e `s l w`), troca de idioma ao vivo nas
dez portas, e o relógio e a ordem da data alcançam toda data do sistema. Duas coisas do pedido do
dono ficaram de fora, e nenhuma está começada — não há meio código no caminho de ninguém.

- **A MOEDA de um lugar ainda não se escolhe na tela.** Acrescentar e remover lugares, escolher o
  fuso desta interface e o que cada lugar mostra já são teclas (`LEADER s l`); o par de câmbio
  (`GBP` → `BRL`) continua vindo do `assets/world.nix` e um lugar acrescentado à mão nasce sem
  nenhum, então a linha do câmbio dele fica desligada. São três letras digitadas de cada lado, e
  o caminho já existe — é o mesmo `Asking` do fuso e da coordenada. **Como saber que acabou:**
  acrescentar Lisboa, ligar o câmbio dela e ver `1 EUR = …` no relance.

- **Não há completação ao digitar um fuso.** A linha da porta pergunta — o minibuffer no rodapé, o
  cabeçalho na janela — e o nome é conferido contra o ARQUIVO (`a_real_zone`), então errar diz "não
  existe fuso com esse nome" — o que é honesto e não é ajuda. São umas quatrocentas zonas e o
  instalador já lê a lista (`available_timezones`, do `zone1970.tab`). **Como saber que acabou:**
  digitar `Lis` e ver `Europe/Lisbon` oferecido, nas duas portas.

### 0.000005. O que a exibição de escritas ainda não alcança · pequeno cada

A cadeia de fallback existe, 37 escritas desenham e a régua da célula conhece a largura leste-
asiática (ver `docs/ARCHITECTURE.md`, "Duas cadeias de fallback"). Três coisas ficam de fora, e
nenhuma é um defeito escondido: são limites medidos.

- **A tinta do CJK é mais estreita que a célula reservada.** Medido: o ideograma anda 1,667
  célula das 2 que a régua dá, e o hangul 1,533. Nada se sobrepõe — sobra vão, e uma linha de
  japonês fica com entreletra folgada. Fechar exige reescalar as cinco faces `NotoSansMonoCJK`
  para `upem 833` mexendo em `head` E no `FontMatrix` do `CFF2`, porque com `CFF2` os contornos
  têm escala própria. **Como saber que acabou:** o `ronoctua-test-faces` imprimir 2,000 para o
  ideograma, e uma foto de um parágrafo em japonês ao lado do mesmo em latim mostrar a mesma
  cor de página.

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

- **Dois valores em pixel não escalam.** `space.rule.thickness` é 1 px, e é defensável — uma régua é
  uma linha, e uma linha é um pixel lógico em qualquer tamanho. Já `space.files.pictureAir` são 6 px
  e a justificativa dele está escrita **em células** ("uma célula inteira — 10,8 px no rodapé — é
  mais do que o vão precisa"): a 200% a célula tem 21,6 px e 6 deixa de ser a fração que o
  argumento supunha. **Como saber que acabou:** ou o ar da figura vira fração de célula, ou o
  comentário passa a dizer por que 6 px vale em toda a faixa.

### 0.0000103. Um teclado ligado com a janela aberta só aparece depois · pequeno

O `LEADER s k` lê `/proc/bus/input/devices` quando a janela abre e de novo depois de cada mudança —
que é quando os números da tela realmente mudam. Um teclado ligado ENQUANTO a janela está aberta não
aparece até que se mexa em alguma coisa ou se feche e abra.

O conserto é um socket `udev` numa thread, no molde da thread de sinais do `ronoctua-bluetooth`: o
kernel avisa `add`/`remove` de `input`, e a chegada manda a lista inteira. Não foi feito porque três
quilobytes lidos na abertura resolvem o caso comum, e uma thread e um socket para uma lista que
ninguém olha enquanto muda é preço antes de necessidade.

**Como saber que acabou:** com a janela aberta, parear o teclado Bluetooth e ver a linha aparecer
sem tocar em nada. **Tamanho:** pequeno.

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

### 0.000013. As telas não se empilham na vertical · pequeno

A disposição (`s d d` → `m`) move a tela sob o cursor com `←`/`→` e grava só o `x`
(`display.rs`, `Target::Place`). O `beside_said` sabe DIZER que uma tela está acima ou abaixo de
outra (`display-above` / `display-below`, as duas usadas), então a vista descreve uma relação que
não sabe criar. A frase da tela diz que as setas movem de um lado para o outro, o que é honesto;
o gesto do outro eixo é que falta.

`↑`/`↓` são o cursor, então precisa de um terceiro — `S-<setas>` é a família "mesma tecla, outro
sentido" que o sistema já usa no gerenciador e está livre nesse nível. **Como saber que acabou:**
duas telas empilhadas na foto, e o `y=` no fragmento.

### 0.000014. As LETRAS ficaram nítidas; os DESENHOS não · médio

As dez portas seguem a escala da tela e o texto é rasterizado nela (`hidpi.rs`), mas toda TEXTURA
continua nascendo em pixel lógico e é esticada pelo mesmo fator: o ícone do localizador
(`apps::pixmap_side`, tamanho lógico), a prévia do clipboard, os painéis do gerenciador (`Fit` medido
em lógico) e o `Command::Svg` dos buffers (`buffer.rs`, lado tirado de `paint::cell()`).

O `hidpi.rs` declara essa fronteira ("só duas coisas são físicas"), então é um limite escrito e não
uma contradição — mas o entregável se chama "o shell desenha nítido", e agora que a letra ao lado
está nítida a diferença aparece MAIS do que antes, não menos.

**O SVG é o que custa nada**: ele é rasterizado por nós, com o lado que a gente escolhe. Bastaria
multiplicar o lado pela escala e desenhar na caixa lógica. **O ícone custa uma CHAVE**: o cache do
localizador é indexado pelo nome nu do `Icon=` (`surface.rs`, `im.missing(key)`), e
`request_visible_icons` filtra o que já está em `self.icons` — então uma vez decodificado a 18 px ele
nunca é pedido de novo, nem numa troca de escala nem numa troca de tamanho do texto. É a mesma classe
que o AGENTS.md já registra para a paleta ("leva a paleta na chave do cache"), com `scale` e `size`
faltando; o `files::Stamp::key()` já carrega o passo de decodificação e está certo por isso.

**E as MINIATURAS saíram desta lista**, que é o que este item queria de cada textura: o alvo delas é
tirado do pixel FÍSICO (`files::thumb_side` recebe a célula e a linha já multiplicadas pela escala),
e a chave é o `Stamp`, que carrega esse passo. Sobram o ícone do localizador, a prévia do clipboard,
os painéis do gerenciador e o `Command::Svg`.

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

### 0.000016. Nove tamanhos de janela escritos como número · invariante 9

Nove dos dez `configure` caem num padrão quando o compositor não dá tamanho:
`cfg.new_size.0.map_or(900, NonZeroU32::get)` nas sete janelas de vista, `640`/`400` no hospedeiro de
buffer e no terminal. São medidas em pixels escritas à mão, e o invariante 9 diz que valor visual
nenhum é literal — cor, fonte, tamanho, espaçamento e codepoint saem de token.

**Oito dos nove são anteriores ao `hidpi.rs`; o nono é deste entregável** — a janela das telas
(`display_window.rs`) nasceu aqui e escreveu `900`/`600` como os outros, porque não há
`space.window` para ela ler. A frase que estava aqui dizia que TODOS eram herdados, e não era
verdade: uma nota que absolve o entregável de um número que ele acabou de escrever é pior que
número nenhum, porque o próximo a ler não confere. Está aqui em vez de consertado porque
a correção não é mecânica, é uma decisão de design sobre QUAL token. Um `space.window`
com uma largura e uma altura para todas responde bem para as sete vistas e mal para o terminal, que
quer um tamanho em CÉLULAS — 80×24 é a medida dele, não 640×400.

- acabou quando: os nove números saem de `assets/tokens/`, e o terminal declara o dele na unidade em
  que ele pensa

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

### 0.000017b. A palavra de uma recusa sem nome sai em inglês nas quatro vistas · pequeno

`echo-display-refused = As telas recusaram: { $why } ({ $words })` — o `$why` é traduzido, o
`$words` não. Para as cinco recusas COM NOME isso foi consertado: a frase traduzida já diz o fato
inteiro, e a palavra inglesa ao lado dela dizia a mesma coisa duas vezes, uma no idioma errado. Mas
o `Refusal::Other` fica, e tem de ficar: é a recusa sem nome, e a palavra é a única coisa que diz o
que deu errado. Ela é inglesa porque o invariante 18 exige que todo `bail!` e todo `.context` sejam.

**A mesma forma existe na energia, na rede e no rádio** (`echo-power-refused`,
`echo-network-refused`, `echo-bluetooth-refused`), então não é desta feature — mas é o invariante 8
e está aqui em vez de escondido. Numa máquina pt-BR, um DDC/CI que responde errado lê
"As telas recusaram: não funcionou (the monitor did not answer)".

A saída provável: cada erro que pode chegar à linha de mensagem vira uma variante com nome e uma
chave, e o `Other` deixa de ter `$words` — quatro vistas de encanamento, e uma delas não é esta.

- acabou quando: nenhuma frase na linha de mensagem tem uma metade num idioma que a pessoa não
  escolheu, nas quatro vistas

### 0.000019b. O tamanho do texto dos apps vive num banco, e nada o remove · invariante 6

`toolkits.rs` escreve `/org/gnome/desktop/interface/text-scaling-factor` no dconf com um `Command`.
É estado persistente da máquina, fora do flake e fora do `ronoctua-store` — e a regra desta casa é
que **o arquivo É o broadcast**, sem banco. Desligar `ronoctua.display.enable` não desfaz a chave.

O cabeçalho do módulo defende a escolha e a defesa é honesta: os dois `settings.ini` que o
`foreign.nix` escreve são caminhos do store, read-only, então o dconf é a única camada VIVA que
GTK e Qt leem. Não há conserto óbvio que não seja reimplementar a camada. Fica como decisão.

- acabou quando: o dono decide que é aceitável e isso vira uma linha no ARCHITECTURE, ou aparece
  um caminho declarativo que os toolkits leiam a quente

### 0.000018. A disposição anda 10 px por tecla, e cada tecla recarrega o compositor · médio

`NUDGE` são 10 px lógicos, e a razão está escrita: um passo que a pessoa consegue segurar. O custo
não estava. Passar um monitor de 1920 para o outro lado do vizinho são 192 teclas, e cada uma emite
um `Order::Keep`: uma leitura (`/sys/class/drm` mais duas conexões ao compositor), uma reescrita do
fragmento, **a releitura que o `Keep` pede em seguida — que é uma carga inteira da configuração** —,
e a leitura que o `watch` tira depois. Três leituras e uma recarga por repetição de tecla.

O rename em si deixou de custar (este compositor não vigia arquivo nenhum); quem custa é o pedido de
releitura, que é obrigatório justamente por isso.

**Duas saídas, e a escolha é do dono.** Ou as setas ENCOSTAM a tela nas bordas dos vizinhos (o
`beside_said` já fala nessa linguagem — "à direita de eDP-1"), o que faz do gesto dois ou três
toques em vez de 192; ou a escrita é adiada até a tecla parar, o que precisa de um relógio que a
vista não tem. A primeira é uma decisão de desenho e por isso está aqui em vez de feita.

- acabou quando: pôr um monitor ao lado do outro custa menos de dez teclas, e segurar a seta não
  recarrega a configuração do compositor a cada repetição

### 0.000019. Uma tela apagada esconde o que está gravado nela · pequeno

Numa tela que o compositor não está desenhando, as quatro linhas de "A imagem" trocam o VALOR pela
frase que diz que ela está apagada — então uma escala, um modo, um giro e um lugar que estão no
arquivo ficam ilegíveis, e o `a` (que devolveria um deles) é recusado na ficha inteira. Sobra o `d`,
que joga tudo fora.

A linha de sessão já foi consertada (ela lia `dark` e agora lê `wanted.off`, que é o que o
`kdl_of` realmente curto-circuita). As outras quatro precisam de uma decisão: mostrar o VALOR
gravado com a frase de apagada ao lado custa uma coluna, e a ficha já tem a linha "Acionada"
dizendo que a tela está apagada.

- acabou quando: numa tela de tampa fechada dá para ler o que está gravado para ela sem abrir o
  `display.json`

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

### 0.00003. A profundidade de bits existe no compositor e não na vista · minúsculo

A vista de telas mexe em modo, taxa, escala, rotação, posição e VRR. **Não mexe na profundidade de
bits**, e a razão MUDOU com a troca de compositor: antes era ausência dele — o fixado não tinha o
botão e respondia com um erro de parsing —, e agora o botão existe. `output <nome> render_bit_depth
6|8|10` está no `scroll-output.5`, e a partir de 10 o compositor implicitamente o eleva quando o HDR
está ligado.

Então isto deixou de ser espera e virou trabalho: um `Ask` novo, um feito declarado em
`assets/compositor-deeds.nix`, um campo em `Wanted` e uma linha no fragmento, mais a leitura de
volta para confirmar (porque `Ok` não é "aconteceu"). **Como saber que acabou:** a linha na ficha de
um monitor, ao lado do VRR, com o valor lido de volta batendo com o pedido.


### 0.00004. A rede não sabe nomes, e três coisas do plano ficaram medidas · pequeno cada

A vista da rede está inteira: uma `View` com duas portas (`s n n` e `s n w`), dez seções com
prateleiras de altura fixa, anel de `TAB`, dica GERADA da `OF_THE_VIEW`, cinco marcas na modeline
(a da Tailscale desenhada por nós), VPN com prévia antes de importar, Tailscale, Tor, portas,
monitor, quem está na rede, redes lembradas e hotspot. O que ficou de fora tem motivo, e cada um é
pequeno:

- **Nome de máquina na LAN.** As linhas de "esta rede" mostram endereço, fabricante (OUI do nmap) e
  interface — não o NOME. Quem sabe o nome de um vizinho é o `resolve1.ResolveAddress`, uma chamada
  de barramento por host em thread; a fronteira com o `systemd-resolved` ainda não existe neste
  projeto (o `ronoctua-network` só fala NetworkManager). É uma coluna a mais, não uma seção.
- **Latência na varredura.** O `ronoctua_lan::ping` mede uma máquina de cada vez e mede certo; a
  VARREDURA não traz número, e isso é uma decisão medida: o primeiro pacote para uma máquina cujo
  endereço de hardware o kernel não conhece espera o ARP — três segundos, medidos na rede do dono —
  e o número apareceria como um enlace péssimo para uma máquina na sala ao lado. A linha do gateway
  é onde um número honesto caberia, sob demanda.
- **IPv6 na vizinhança.** `/proc/net/arp` é IPv4 e o kernel não tem arquivo equivalente para IPv6 —
  os vizinhos v6 vivem no netlink, que é outra conversa. O título diz qual é.
- **Hotspot na TELA do dono.** Está implementado e provado por teste unitário (a linha oferece o
  outro estado, diz o que custa e recusa com o rádio desligado), mas nunca foi acionado: ligá-lo
  derruba o Wi-Fi da máquina de quem está lendo isto. Falta uma VM com `mac80211_hwsim`.
- **A senha do hotspot só aparece na porta que o ligou.** A linha lê o `View::sharing` da própria
  vista, que só o `Hotspot(true)` daquela porta preenche: ligado no rodapé, a janela diz "hotspot
  ligado" sem a senha, e o contrário também; reabrir a vista do rodapé também a perde. O conserto
  que a revisão propôs — ler a senha do perfil ativo no NetworkManager (`GetSecrets`) — pede uma
  decisão: para um perfil de sistema essa chamada pode exigir autorização, e nada que o sistema faz
  sozinho pode perguntar. A outra saída é um arquivo `0600` no diretório de runtime, escrito por
  quem liga e apagado quando o hotspot cai (o `bluetooth::standing` é o precedente) — uma senha a
  mais em disco, ainda que em tmpfs. Pequeno, depois da escolha.
- **Perfil de VPN com senha.** Importar um `.ovpn` que pede senha traz o perfil; a senha em si ainda
  é digitada no `nmcli` ou no primeiro `RET` do NetworkManager. O `store_vpn_secret` do plano
  (`GetSecrets` + `Update` com `password-flags=0`) não foi escrito.
- **Um check oferecido e não escrito.** Dos três que a revisão do entregável propôs, dois foram
  escritos depois — `the-footer-draws-what-the-keyboard-drives` (existe, e a ordem que ele prende
  deixou de ser mantida à mão: toda porta pergunta ao `view()`) e o das famílias do rádio, que virou
  o `every_service_this_table_names_has_a_sentence` mais as três enumerações do BlueZ que passaram a
  ter chave. Falta o terceiro: um check que prenda o `Errand` de cada vista sendo tratado nas DUAS
  portas sem `_ =>`. Hoje o compilador já pega o que falta numa porta (`match` sem `_`), mas não pega
  os dois lados tratando o MESMO `Errand` de jeitos diferentes.
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

### 0.00006. O Bluetooth não faz três coisas que o BlueZ oferece · média cada

O detalhe do dispositivo mostra e deixa mudar tudo o que o `Device1` tem — inclusive o `WakeAllowed`
e o perfil de áudio (este por PipeWire, ver `ronoctua_bluetooth::audio`). Faltam três interfaces
inteiras, e cada uma é uma decisão e não um esquecimento:

- **`org.bluez.MediaPlayer1`/`MediaControl1`** (tocar, pausar, faixa de um celular ou caixa). Fora
  **de propósito**: o PipeWire já publica isso como MPRIS, o `playerctl` fala MPRIS, e uma segunda
  implementação aqui seria uma segunda resposta para "o que está tocando" — a mesma razão pela qual
  o crate não reimplementa layout de janela.
- **`org.bluez.Network1`** (PAN: usar a internet do celular). É a única que vale a pena e não foi
  feita: precisa de uma linha na vista, de um `Ask` e de uma decisão sobre o que o NetworkManager faz
  com a interface `bnep0` que aparece — ou seja, atravessa a fronteira do `ronoctua-network`, e as
  duas vistas teriam de concordar sobre quem manda nela.
- **`ManufacturerData`, `ServiceData`, `AdvertisingFlags`, `Sets`, `CablePairing`.** Bytes crus de
  anúncio; o `Sets` (LE Audio) e o `CablePairing` estão em toda máquina moderna. São FATOS sem frase
  possível hoje: mostrar `0x004C: 0215...` é encher a tela com o que ninguém lê.

- acabou quando: a linha do PAN existe e as duas vistas concordam sobre quem manda no `bnep0`, ou
  este parágrafo diz por que não

### 0.0001. O realce de sintaxe deixa quatro coisas de fora, de propósito · pequeno cada

Não são esquecimentos; cada uma tem um motivo estrutural escrito no `ARCHITECTURE`, e estão aqui para
que ninguém as descubra achando que achou um defeito.

- **Heredoc** (`<<EOF`, `<<'PY'`). O terminador é texto arbitrário descoberto em tempo de execução, e
  o `State` que atravessa linhas cabe em oito bytes de propósito — texto não cabe lá. Custaria o
  `Copy` e poria uma alocação no orçamento do frame.
- **Título setext** do markdown (o sublinhado de `===`). Precisaria do conteúdo da linha ANTERIOR, e
  o desenho inteiro é "uma linha mais oito bytes".
- **Linguagem dentro de `<script>` e `<style>`** no HTML. O `State::inner` já tem a vaga; o que falta
  é o markup delegar, e isso não custa mudança de tipo nenhuma.
- **A segunda leva de linguagens** — `erlang`, `elixir`, `lisp`, `ocaml`, `crystal`, `julia`,
  `fortran`, `verilog`, `vhdl`, `tcl`, `groovy`, `d`, `vala`, `nim`, `qml`, `gherkin`, `rpm-spec`,
  `gettext` e o resto. **É só dado**: um attrset cada em `assets/syntax.nix`, zero Rust, zero papel
  novo, zero cor nova. É esse o ponto da forma.

- acabou quando: o item que se resolveu sai desta lista; nenhum deles muda como o sistema funciona,
  então nenhum precisa de linha no `AGENTS.md`

### 0.0002. Nenhum buffer wasm usa o realce ainda · pequeno

O caminho está inteiro e provado por teste: `ronoctua-design` tem a feature `syntax`, o
`Palette::syntax` está implementado dos DOIS lados, o `Screen::code` faz a aritmética byte→célula uma
vez, e o `nix/buffers.nix` já copia o crate e passa a tabela. O que não existe é um buffer que peça a
feature — nenhum dos oito mostra conteúdo de arquivo.

Medido, para quando alguém for: com o crate na árvore e a feature DESLIGADA, o `format.wasm` cresceu
366 bytes; a tabela são 55 KB de fonte, e é isso que um buffer paga ao ligá-la.

- acabou quando: um buffer liga `features = [ "syntax" ]` e desenha uma linha colorida numa foto do
  `nix run .#png`



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
dois sozinho. Ela está lá para que uma cena abra qualquer arquivo do `MEDIA`.

- acabou quando: uma cena do `.#shot` arrasta o cursor da linha do tempo e mede que ele andou; e o
  dono decide se a tabela de decodificadores fica no palco ou sai dele

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

### 0.00032. A home do player com uma fila grande, e o nome de uma língua · pequeno cada

- **A fila não tem teto na home.** Ela vem antes de Ouvir e assistir e de Recursos variados, então
  com duzentos itens esses ficam duzentas linhas abaixo, e o rodapé mostra umas quinze. A fila já é
  árvore (`player_queue`) e uma playlist de duzentos dobra numa linha, mas duzentos arquivos soltos
  no nível de cima continuam duzentas linhas: falta a home mostrar a atual e algumas em volta, com
  uma linha "Fila · N" que abre a página inteira.
- **Uma faixa diz o código da língua** (`eng`, `por`), cru, como o mpv o dá; duas iguais já ganham
  o codec e o número (`player::track_name`). Na página das legendas, uma faixa diz também o código
  que o arquivo declara (`pt-BR · Portuguese`, `player_subtitles::name`). O nome da língua sai da
  tabela gerada de línguas que as buscas trazem.

- acabou quando: a home mostra uma janela da fila e a faixa diz o nome da língua, nos dois idiomas

### 0.00033. O XML da prévia ainda não passa pela guarda de profundidade · pequeno

As listas, as legendas e as tags do Matroska leem XML pelo `ronoctua_xml::parse`, que mede o
aninhamento antes de o `roxmltree` recursar. O `ronoctua-inspect` ainda chama o `roxmltree` direto
em oito lugares (`document.rs`: o texto corrido dos documentos, o livro, as planilhas ODF e
OOXML; `mesh/`: AMF e DAE), e um
arquivo feito para isso derruba o rodapé na prévia. Não entrou no mesmo passo porque o teto não é
o mesmo: uma cena DAE aninha nós de verdade (um esqueleto passa de vinte níveis), e 48 é o teto
medido para listas e legendas.

- acabou quando: todo `Document::parse` do workspace passa pela guarda, com o teto de cada formato
  medido, e um check (`one-xml-reader`, no molde do `one-zip-reader`) recusa um novo leitor direto

### 0.00034. Os sons ambientes sem gravações, e o par da modeline sem foto · pequeno cada

- **As gravações não foram publicadas.** Os laços saem de três originais aprovados de ouvido, que
  vão num asset de Release `ambience-v1`: `nix run .#ambience-release -- <pasta dos originais>`
  confere cada sha256 contra `assets/ambience.nix`, empacota o arquivo sempre com os mesmos bytes e
  imprime as notas e o hash; o dono cria o Release e sobe o arquivo como o único asset. Depois: o
  input `ambience-v1` (`file+https://`, pelo motivo das fontes) com `nix flake lock --update-input
  ambience-v1`, o pacote `ambience` passado ao módulo e ao `.#dev`, e o `RONOCTUA_AMBIENCE` do
  `player.nix` sem a condição `ronoctua ? ambience`. O palco do `.#shot` passa o catálogo de teste
  (`--setenv=RONOCTUA_AMBIENCE=${MEDIA}/ambience`), para a página ser fotografada. Até lá o player diz na página que a máquina não tem sons ambientes, e a página, a
  linha da home e o par da modeline só foram vistos com estados escritos para a foto. Sons novos só entram com
  aprovação de ouvido do dono.
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
- **A régua fica rala num trilho curto.** Na coluna média, com os nomes, o trilho tem ~17 células e
  só o `0:00` cabe inteiro antes do fim (`ticks` só escreve o rótulo que termina dentro do trilho).
  Deslocar o último rótulo para terminar no fim do trilho daria uma segunda marca; é decisão de
  desenho, não feita.
- **O roteamento por tela no rodapé não foi visto na tela.** Cada superfície do rodapé compõe as
  imagens roteadas ao conector dela (`player_media::placed_on_screen`), provado por teste; falta o
  `.#shot` com duas saídas: um filme só numa, a capa só na outra, e o esquema da página Disposição
  pintado na região.
- **"Mostrar em cada tela" não foi visto em duas telas.** A decisão é testada e as fotos mostram a
  frase; falta a VM de duas saídas (`player-vm`, o plano) provar que o compositor pinado honra o
  `set_fullscreen(Some(output))` e que cada janela mostra só o que vai à tela dela.
- **Os glifos das linhas são pequenos** ao lado do texto: a forma na Disposição, o `◀`/`▶` e os tipos
  de saída de som são desenhados no tamanho do texto, como toda marca de linha da vista.
- **Uma faixa desligada não tem comprimento dito** (o daemon só manda `lane_states` das ligadas), e
  a banda dela, cinza, corre até o fim do trilho como uma ao vivo.
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
- **Um clique abaixo da última linha da lista do player escolhe a última linha** (`row_at` devolve
  um índice além das linhas, e `Row::owner` o prende à última): visto fotografando a página que
  escolhe uma capa. Antigo, não desta mudança.

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
- **O que ainda é só leitura.** O dono escolheu o genérico: toda linha de Todas as etiquetas que o
  formato grava age, e "Acrescentar uma etiqueta" pergunta nome e valor. Ficaram sem linha que os
  mude a língua, o título e as marcas de um fluxo (`Edit::Stream`, que o escritor do Matroska já
  faz) e o título de um capítulo; e as etiquetas de uma faixa do Matroska dizem que não são gravadas
  (`NotGlobal`: o `--tags track:<n>:` do `mkvpropedit` substitui as de uma faixa, e a faixa teria
  de ser achada pelo `TrackUID` no `mkvmerge -J`).
- **Tirar o último campo de um FLAC é recusado.** O lofty 0.25.4 não grava um comentário Vorbis
  VAZIO num FLAC: o rascunho relido não confere (`native::verify`) e é jogado fora, o arquivo como
  estava, e a tela diz que a gravação falhou. Vale para `d` no único campo ou na única etiqueta.
- **Etiquetas pelo próprio nome não foram gravadas por um daemon de verdade.** O escritor foi provado
  em arquivos feitos no teste (FLAC, MP3, M4A, e um Ogg com o ffmpeg de verdade) e com programas de
  mentira para o Matroska e o remux; a página, fotografada com folhas dadas (`--sheet`). Falta a volta
  inteira — gravar, a folha nova chegando pela geração — com um player de verdade.
- **Gravar Matroska com o `mkvtoolnix` de verdade.** O escritor foi testado com programas de
  mentira (`/bin/sh`), nunca com os três programas. Um subteste no `player-vm` com o perfil de
  mídia.
- **Etiquetas de um MP4 acima de 512 MiB.** O lofty reescreve com o arquivo inteiro na memória, e a
  folha diz que é grande demais — quase todo filme em MP4. Um escritor que não segure o arquivo
  (reescrever só o `moov`, ou o remux do ffmpeg onde ele guarda tudo).
- **Os formatos de resposta** do MusicBrainz, Cover Art Archive, Wikidata, TMDB, OpenSubtitles e
  SubDL vêm da documentação, com fixtures escritas à mão; só o do LRCLIB foi conferido contra uma
  resposta de verdade. E o moviehash contra os vetores publicados (`breakdance.avi`, 12,9 MB) tem o
  teste, `#[ignore]`, sem o download pinado que o alimenta.
- **Os códigos de língua do OpenSubtitles e do SubDL** em `assets/tongues.nix` foram escritos das
  listas dos serviços e só conferidos na forma: as duas listas pedem a chave de uma conta.
- **O Podnapisi** volta quando o domínio responder (`podnapisi::AWAKE`).
- **`lookups-vm` e `directories-vm`**: os palcos dos lanes E8 e E9 (uma CA descartável,
  `networking.hosts` para um falso HTTPS de cada host) como testes de VM.
- **`ronoctua_lookup::keys::write_private_file`** mora no crate das buscas e guarda também as máquinas
  pareadas: é do `ronoctua-store`.
- **A cidade de um canal de TV** é dita e não desenhada: o atlas não acha um lugar pelo nome dentro
  de um país (`ronoctua_atlas::place_named`, e usá-lo em `AtlasMap::place`).
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
- **Uma página com o diretório não permitido mostra a lista que a mesa ainda guarda**, de antes de a
  permissão ser tirada: a linha que pergunta vem em cima, mas a lista velha fica.

- acabou quando: a foto do rodapé existe, o rascunho e uma etiqueta foram vistos ida e volta, os
  diretórios foram vistos contra um daemon, cada formato de serviço tem uma resposta de verdade como
  fixture, e cada linha só de leitura acima ou age ou diz por que não

### 0.00037. Legendas: o que só um arquivo de verdade ou a VM provam · pequeno cada

- **Legenda de imagem (PGS, VobSub) num teste de VM.** O ffmpeg não codifica PGS a partir de texto,
  então a trilha secundária só tem teste de unidade. Falta uma amostra pinada (`downloads-are-pinned`)
  e o subteste que a vê no quadro, com as faixas de texto subindo para a banda de cima.
- **A trilha de dentro de um filme que toca pela rede** (seguindo outra máquina) só aparece sozinha:
  a chave de acesso à mídia vai num cabeçalho, e o ffmpeg a levaria no `argv`.
- **Um SAMI com várias línguas** mostra a primeira classe que diz algo; escolher outra pede um
  parâmetro que a faixa não tem.

- acabou quando: a legenda de imagem tem subteste com pixel, e as outras duas existem ou estão no
  `ARCHITECTURE.md` como decisão

### 0.00038. Faixas: o que ficou sem fazer · pequeno cada

- **Um conjunto com todas as faixas desligadas** não toca nada e nunca acaba: a thread do relógio
  anda enquanto ele "toca".
- **A porta não pede o contorno.** O `watch` manda o contorno da fila antes do estado que vai com
  ele, e a porta guarda o último; uma porta cujo contorno ficasse mais velho que o `revision` do
  estado (uma linha perdida) não pede o outro (`Ask::Queue`).

- acabou quando: um conjunto sem faixa ligada termina (com teste), e a porta pede o contorno quando
  a revisão do estado passa a dela

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

### 0.001. O minibuffer não mostra os submenus montados em runtime · pequeno

`SPC ,` e `SPC w s` são listados a partir do que a janela em foco publicou, e por isso não estão na
`ronoctua_keymap::Tree` — as ligações dela são `&'static str`. A consequência é que a sequência
pendente que o minibuffer desenha vem só do `Leader`, e ela **para no nível anterior**: com a lista
de estados na tela, a linha diz `LEADER w-` e não `LEADER w s-`.

Isto **não é novo**: o `SPC ,` (antes `SPC m`) faz o mesmo desde que existe, e o `buffer_pending` dele nunca foi
alimentado no prefixo. O estado só acrescentou um segundo caso, e por isso vira item — dois é
padrão, um era acidente.

O conserto é um só e serve para ambos: o prefixo do minibuffer passa a somar o que os submenus de
runtime têm pendente, em vez de ler só a sequência do `Leader`.

- acabou quando: com a lista de estados aberta, o minibuffer diz `LEADER w s-`, e o `SPC ,` diz
  `LEADER ,-`

### 0.01. Nenhum buffer usa o `tone` explícito · pequeno

O descritor deixa um estado escolher o papel da cor (`accent`, `success`, `warning`, `error`) em vez
de aceitar a regra padrão, e os seis buffers usam `standard`. O caminho está provado no runtime (um
tom desconhecido cai em `standard` em vez de sumir) e no teste de forma, mas **não** ponta a ponta
numa tela.

O primeiro candidato real é a pergunta de permissão do gerenciador de arquivos, que é `warning` de
verdade — e ela depende do item acima.

- acabou quando: um estado sai numa cor que não é o verde de todos os outros, e há check disso


### 0.015. Os passos que o brain desenha e os que a unidade roda não têm quem os compare · pequeno

A tabela é uma só (`assets/updates.nix` gera, e o brain a LÊ por `build.rs`), então os ids não têm
mais como divergir — foi assim que o defeito anterior morreu. O que continua sem prova é a outra
ponta: a **unidade** escreve o relatório com esses ids, e nada confere que ela escreve os mesmos.
Hoje é o mesmo gerador que produz os dois, o que torna a divergência improvável e não impossível.

- acabou quando: um check lê os ids da tabela e os ids que o roteiro gerado escreve no relatório, e
  falha se um lado tiver um que o outro não tem

### 0.016. A faixa de notificação não é descontada do teto da expansão · pequeno

`expansion_ceiling` recebe quanto é fixo, e o rodapé passa `FOOTER_LINES + brain_line()`. A faixa de
notificação — até três linhas — continua de fora, então com ela na tela a região passa do percentual
que o token promete. É anterior à linha do cérebro e não foi criado por ela; o que a linha fez foi
tornar o número um parâmetro, que é o que permite consertar isto numa linha.

O que falta decidir é se ela DEVE entrar: a faixa é transitória de um jeito que as outras linhas não
são, e descontá-la faria a lista encolher e crescer sozinha enquanto uma notificação passa.

- decidir: se a faixa entra no termo fixo; se entrar, é um argumento a mais na chamada

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

### 0.15. Apagar num pendrive não tem lixeira · pequeno, e é uma decisão adiada

`trash.rs` recusa atravessar sistema de arquivos com `EXDEV` em vez de copiar, e a recusa está
certa: mover para a lixeira promete ser instantâneo, e copiar quarenta gigas atrás desse gesto seria
a promessa quebrada. O que faltava era a pessoa saber POR QUE dois caminhos estavam em sistemas
diferentes — e a barra lateral resolve isso, porque agora o pendrive está na tela com linha própria.

**A barra melhorou a recusa em vez de criar a necessidade de substituí-la**, e é por isso que isto
fica aqui em vez de ter entrado junto. O que falta é o `$topdir/.Trash-$uid` da especificação, e ele
tem um modo de falhar que é próprio de removível: apagar escreve o arquivo de volta NO pendrive, e
quem desconecta em seguida acha que liberou espaço. Some a isso que num vfat não há dono, então
`.Trash-$uid` e o `$topdir/.Trash` da especificação respondem diferente a "quem pode ler isto".

Custo real: um teste honesto precisa de um SEGUNDO sistema de arquivos, o que quer dizer VM — e as
quatro provas que `trash.rs` tem hoje rodam contra uma árvore temporária, sem barramento e sem
máquina.

- acabou quando: apagar um arquivo num pendrive o põe em `$topdir/.Trash-$uid`, o gerenciador o
  lista de volta, e desmontar não deixa a lixeira num volume que não está mais lá

### 0.2. Matar a unidade no meio de uma cópia deixa o pedaço · pequeno

Desistir pelo painel desenrola e remove o que foi escrito; `systemctl --user stop` não — o processo
não tem tratador de `SIGTERM`, e escrever um exigiria `unsafe` ou um crate novo num crate que é
`forbid(unsafe_code)`. `TimeoutStopSec=5` limita o atraso, não a sujeira.

- acabou quando: parar a unidade com uma cópia em andamento não deixa arquivo pela metade no destino


### 3. Desempenho medido só em host não-NixOS · pequeno

`nix run .#bench` mede num compositor **aninhado** dentro de outro, com o contorno de driver de
`nix/graphics-host.nix`. O caminho gráfico da distro instalada é mais curto.

- acabou quando: o mesmo comando roda num NixOS instalado e os números entram no `ARCHITECTURE.md`

### 4. O `--execute` chega ao passo 5 de 10 · pequeno

**Três defeitos do instalador saíram daqui, e nenhum deles o modo ensaio podia ver** — ele imprime os
comandos em vez de rodá-los. Os três estão corrigidos:

- **O diretório de trabalho ERA o store.** `cp -rT` não segue symlink dado na linha de comando, e
  `/etc/nixos` é link para o store: escrever o `host.nix` dava `Read-only file system`. É `-rTL`.
- **O `disko` não podia funcionar.** O `host.nix` gerado diz `facterReport = ./facter.json`, e o
  `disko --flake #noctua` AVALIA esse arquivo — que só era criado três passos depois, dentro do
  `/mnt` que o disko ainda não montou. Toda instalação morreria no passo quatro. O `nixos-facter`
  roda antes agora, no diretório de trabalho.
- **O nó instalador não bootava.** Raiz em disco de rascunho depende do `autoFormat`, que o initrd do
  systemd não implementa (`Can't find ext4 filesystem`, pânico aos 2,7 s; sem o initrd do systemd,
  `Attempted to kill init!` aos 1,2 s). Raiz em **tmpfs** não tem esse passo, e deixa o `/dev/vda`
  inteiro para o disko.

**O teste existe em `tests/installer-vm.nix` e NÃO está ligado ao `tests/checks.nix`** — check
vermelho que todo mundo aprende a ignorar é pior do que lacuna assumida.

**Onde ele para: custo, não defeito.** O passo 5 (`disko --mode destroy,format,mount --flake`)
avalia a configuração NixOS inteira DENTRO da VM, e os 900 s de timeout padrão do driver não bastam —
sai com 124 sem terminar. A próxima tentativa é: `timeout` maior no `execute`, mais memória no nó, e
o `diskoScript` no `system.extraDependencies` para a avaliação ter menos a fazer.

- acabou quando: `--execute` roda até o fim contra um disco virtual, e o sistema que ele instala boota

### 6. A porta-janela do clipboard · decidir antes, e a decisão MUDOU

O clipboard virou uma vista (`clipboard_view.rs`) com tabela, dica gerada, seções e os ajustes do
ClipCascade — mas continua tendo UMA porta, o rodapé. Todas as outras vistas do sistema têm duas.

**E o obstáculo não é a tecla, é o DESENHO.** O histórico do clipboard não é um arquivo nem um
programa: é um `VecDeque` na memória do rodapé, alimentado pelo `zwlr_data_control` daquele processo.
Uma segunda porta teria de abrir um segundo vigia e passaria a guardar uma SEGUNDA história — que é
exatamente o defeito que o `ronoctua-clipcascade` existe para evitar um crate ao lado, e a razão de
o cliente ser nativo em vez de ser o programa do upstream rodando ao lado do nosso.

O histórico do Atuin não tem esse obstáculo (é um binário e um arquivo, e um segundo processo lê os
dois), e é por isso que a janela DELE existe e a deste não.

- decidir: ou a janela fala com o rodapé pelo socket de controle e não vigia nada (uma história,
  duas telas — é trabalho de protocolo, não de vista), ou o clipboard fica com uma porta só e a
  linha do `AGENTS.md` sobre "duas portas" ganha a exceção escrita
- e a TECLA continua em aberto: o `AGENTS.md` registra, com motivo, que `m` é a miscelânea *"e é o
  que libera `c` para ser o clipboard SOZINHO — comando, não prefixo"*. `LEADER m c` respeita essa
  linha; `c c`/`c w` a desmente e obriga a reescrevê-la

### 9. Os checks das quatro superfícies novas · médio

O plano previa dez. **Um está escrito** — o `every-modal-view-has-a-hint`, que é a classe do defeito
fotografado: ele lê o `enum ViewState` e exige um braço no `match` de dicas para toda vista que pega
o teclado, com quatro exceções nomeadas e justificadas. Foi sabotado (o braço do clipboard removido),
a mensagem exata observada, e o arquivo restaurado por md5. Achou de quebra que o localizador de apps
também não tinha braço — e essa é uma exceção legítima, agora escrita.

Os outros nove faltam, e cada um fecha um buraco que hoje só um teste de unidade cobre:

- `clipboard-draws-inside-its-region` — qualquer linha do clipboard calculada contra `end`
- `text-keys-are-constants` — uma `.t("literal")` fora de teste
- `one-http-writer` estendido — um quarto HTTP, ou um segundo TLS à mão
- `one-clipcascade-boundary` — um segundo crate falando `/clipsocket`
- `clipcascade-cipher-matches-upstream` — o vetor divergindo
- `tor-socks-is-declared-once` — o literal do Rust contra o `SOCKSPort` do torrc
- `tor-flake-owned-rows-refuse` — uma linha `fixed` que o módulo não fixa
- `atuin-options-have-text` e `tor-options-have-text` — chave de tabela sem texto num idioma
- `atuin-config-is-seeded-once` — o `config.toml` deixando de ser semeado e voltando a ser link

- pronto quando: cada um existir, tiver sido quebrado de propósito, a mensagem exata observada, e o
  arquivo restaurado por md5

### 5. Migrar o `options.rs` do Syncthing para o `ronoctua-options` · pequeno

O crate `ronoctua-options` nasceu com as tabelas do Tor, do ClipCascade e do Atuin: o que cada uma
precisa — para que palavra um anel gira, que passo uma seta dá, que chave de texto uma palavra tem —
é idêntico, e sem ele seriam três cópias.

O Syncthing tem a sua própria, mais velha e moldada em `serde_json::Value`, e ela **não** foi
migrada: cento e vinte e uma opções funcionando não valem um refactor no meio de outra entrega.

- pronto quando: `crates/ronoctua-syncthing/src/options.rs` usar o `Kind`, o `turn`, o `nudge` e o
  `word_key` compartilhados, e a bateria passar sem mudar uma linha de `assets/syncthing-options.nix`
