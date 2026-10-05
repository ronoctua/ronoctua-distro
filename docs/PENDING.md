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

### 0.0000035. O que a câmera ainda não faz · pequeno a médio cada

- **Câmera MIPI atrás do libcamera** (muitos laptops novos com sensor Intel IPU): ela não é um nó
  V4L2 de captura comum e não aparece. Acaba quando uma dessas é fonte, provada numa máquina que a
  tem. Médio.
- **Câmera só multiplanar não grava.** A varredura lê os formatos dela (`VIDEO_CAPTURE_MPLANE`), e
  tamanhos e taxas aparecem na vista ao lado da frase `camera-about-cannot-stream`; quem não a abre
  é a entrada V4L2 do ffmpeg, que só fala plano único. Acaba com a mesma ponte do item de cima (o
  libcamera, ou um `v4l2-relayd` declarado). Médio.
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

### 0.0000039. As colunas: o que ficou para depois · pequeno

- **A coluna em duas telas de verdade**: que cada tela mostra a coluna do seu espaço, e que uma tela
  que some estaciona a coluna. Nenhum teste a cobre (o subteste de duas saídas do `player-vm` é da
  janela cheia, não da coluna).

- acabou quando: o teste de duas telas da coluna existe

### 0.00000395. Os checks oferecidos do redimensionador, das colunas e do quiosque · pequeno

- `native-hosts-take-chromes-origin` — ESCRITO e registrado, nunca construído: ele precisa dos
  binários `rust.transfers` e `rust.browser`, e a leva não construía o workspace no Nix. Falta
  rodá-lo, quebrá-lo de propósito e ver a mensagem.

- acabou quando: ele foi quebrado de propósito, a mensagem exata foi vista e o arquivo restaurado

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
  derruba o Wi-Fi da máquina de quem está lendo isto. Falta uma VM com `mac80211_hwsim`.
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
- **`lookups-vm` e `directories-vm` foram escritos e ainda não rodaram** (uma CA descartável,
  `networking.hosts` para um falso HTTPS de cada host). O `lookups-vm` não confere o `0600` de uma
  chave: quem a grava é a porta, com o que a pessoa digita, e o teste a grava como a porta gravaria
  (`lookup-keys-are-private` prende como o arquivo nasce). O `directories-vm` não abre um filme
  inteiro do archive deixado (só a recusa sem a permissão) nem a TV além da recusa.
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

`nix run .#bench` mede num compositor **aninhado** dentro de outro, com o contorno de driver de
`nix/graphics-host.nix`. O caminho gráfico da distro instalada é mais curto.

- acabou quando: o mesmo comando roda num NixOS instalado e os números entram no `ARCHITECTURE.md`

### 0.0000029b. O alívio do disco confia na conta do Nix · pequeno

O módulo `disk-space` faz uma coleta só, `nix-store --gc --max-freed`, e não mede o disco de novo
depois. Quando o Nix conta como liberado o que o disco não ganhou de volta (um caminho morto numa
camada só de leitura, medido no `disk-space-vm`: 263,8 MiB "liberados" e nada no `df`), a coleta para
antes de chegar ao lixo de verdade.

- acabou quando: o alívio mede o espaço livre depois de cada coleta e repete até a meta ou até a
  coleta não liberar mais nada, com um teste que planta um caminho cuja conta engana

### 4. O `--execute` chega ao passo 10 de 12 · médio

`tests/installer-vm.nix` instala de verdade num disco virtual e **não é check** (decisão do dono,
2026-10-05): check vermelho que todo mundo aprende a ignorar é pior do que lacuna assumida. A leva de
2026-09-30 o levou do passo 5 ao 10 e consertou três defeitos que o modo ensaio não via: o `disko`
pedia confirmação de novo num stdin que ninguém responde (agora `--yes-wipe-all-disks`), a cópia do
flake falhava porque o `disko` não cria `/mnt/etc` (um passo `inst-step-target` antes dela), e o
`executor::run` lia o stdout até o fim antes do stderr — o `nixos-install` enchia o cano do stderr e
a instalação ficava parada até o prazo global (agora um cano só, lido em bytes).

O que falta é do TESTE, não do instalador: sem rede, o `nixos-install` dentro da VM quer construir
o sistema-alvo do zero (4684 derivações, do `hex0-seed` ao workspace), porque as respostas do teste
(tema, língua, teclado, o `facter.json` gerado lá dentro) o fazem diferente do alvo que o nó traz no
store. O caminho é construir o `target` com as MESMAS respostas e um `facter.json` fixo, e pôr as
ferramentas de build no `extraDependencies`; custa ~1 h por tentativa e um fecho bem maior.

- acabou quando: `--execute` roda até o fim contra um disco virtual, o sistema que ele instala
  boota, e o teste volta para o `tests/checks.nix` (no `slow`)

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
  query` depois de uma varredura, e fora dela depois de apagado no player.
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

