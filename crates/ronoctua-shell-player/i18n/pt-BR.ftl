# The texts `ronoctua-shell-player` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Installer — the interface

echo-argument-refused = { $command } não aceita “{ $argument }”
globe-address-near = perto de { $place }, a cerca de { $km } km
globe-behind = este lugar está do outro lado do globo
# =====================================================================
# O PLAYER — `LEADER p`, a coluna à direita, a marca na modeline
# =====================================================================
group-player = player
player-title = Player
cmd-player-open = O player, no rodapé
cmd-player-window = O player, numa janela
cmd-player-column = A coluna do player, neste espaço de trabalho (ou fechá-la)
cmd-player-swap = Trocar janela e coluna
cmd-player-quit = Fechar o player em todo lugar: o que toca para, e toda janela, coluna e vista dele se fecha
cmd-player-play-file = Tocar um arquivo ou um endereço agora; a fila fica
cmd-player-enqueue = Pôr um arquivo ou um endereço na fila
cmd-player-together = Tocar outra coisa junto com o que toca: uma imagem, um som ou legendas
cmd-player-toggle = Tocar ou pausar
cmd-player-next = O próximo da fila
cmd-player-previous = O anterior da fila
cmd-player-stop = Parar
cmd-player-shuffle = Embaralhar a fila, ou parar de embaralhar
cmd-player-repeat = Repetir nada, a fila, ou este
cmd-player-up = Linha anterior
cmd-player-down = Próxima linha
cmd-player-less = O valor um passo atrás
cmd-player-more = O valor um passo à frente
cmd-player-enter = O que um clique na linha faz
cmd-player-remove = Tirar da fila, ou do que toca junto
cmd-player-browse = Escolher arquivos para a fila: música, vídeos, tudo o que o player toca
cmd-player-add = Pôr na fila o que a linha toca — uma rádio, um canal, um filme
cmd-player-play-now = Tocar agora o que a linha toca — logo depois do que toca; a fila fica
cmd-player-clear = Tirar tudo da fila
cmd-player-quieter = O volume um passo abaixo
cmd-player-louder = O volume um passo acima
cmd-player-only = Só o que toca — o vídeo ou a capa — na região, na janela ou na coluna
cmd-player-maximize = O que toca, na tela inteira — e de volta
cmd-player-pip = O que toca, numa janela pequena no canto, por cima das outras — e de volta
cmd-player-mute = Silenciar o player, ou devolver o som dele
cmd-player-tracks = Escolher entre as mídias do arquivo: os sons e as legendas dele
cmd-player-fold = Dobrar ou desdobrar a playlist ou o conjunto sob o cursor
cmd-player-earlier = Leva a entrada sob o cursor — um arquivo, uma playlist, um conjunto — um lugar para antes na lista dela
cmd-player-later = Leva a entrada sob o cursor — um arquivo, uma playlist, um conjunto — um lugar para depois na lista dela
cmd-player-also = Tocar também esta mídia do arquivo, ao lado da que está em uso: uma faixa só dela
cmd-player-rename = Outro nome para o item sob o cursor, ou para o que toca
cmd-player-save-set = Salvar o que toca como conjunto (.rset): os arquivos e como tocam juntos
cmd-player-save-queue = Salvar a fila num arquivo (.rqueue)
cmd-player-subtitles = As legendas sobre a imagem
cmd-player-lane-shown = Liga ou desliga a faixa sob o cursor: uma legenda aparece ou some, uma imagem ou um som toca ou para
cmd-player-ambience = Os sons ambientes
cmd-player-ambience-silence = Silenciar todos os sons ambientes
cmd-player-timeline = Todas as faixas do que toca, num relógio só
cmd-player-layout = Como as imagens do que toca dividem a tela
cmd-player-metadata = Os metadados do que toca: etiquetas, capa e fluxos
cmd-player-suggestions = Etiquetas para o arquivo que toca, sugeridas pela internet
cmd-player-find-subtitles = Legendas para o filme que toca, da internet
cmd-player-find-lyrics = Letras para a música que toca, da internet
cmd-player-internet = O que o player pergunta à internet, e as suas chaves
cmd-player-every-screen = O que toca na tela inteira de cada tela para onde vai; de novo, e elas fecham
cmd-player-radio = Rádios do mundo: as estações de todos os países
cmd-player-tv = TVs do mundo: os canais de todos os países
cmd-player-channels = Canais por tema: TV ao vivo, filmes e lives
cmd-player-search = Buscar pelo nome na lista desta página
cmd-player-favourite = Guardar o que está sob o cursor nos favoritos, ou tirar
cmd-player-nearer = Trazer o globo para mais perto
cmd-player-further = Levar o globo para mais longe
cmd-player-world = O globo de volta como a página o enquadra: o mundo, ou o país
player-up = sobe
player-down = desce
player-less = recua
player-more = avança
player-enter = escolhe
player-stop = parar
player-previous = anterior
player-remove = remove
player-browse = arquivos
player-add = pôr na fila
player-play-now = tocar já
player-clear = limpar a fila
player-quieter = mais baixo
player-louder = mais alto
player-only = só a mídia
player-maximize = tela inteira
player-pip = janela pequena
player-mute = mudo
player-swap = janela/coluna
# The swap's own sentence in the footer, where it opens the window in the
# view's place — the command's says window and column.
player-swap-footer = O player numa janela, no lugar do rodapé
player-tracks = mídias
player-fold = dobrar
player-earlier = antes
player-later = depois
player-also = tocar também
player-rename = renomear
player-subtitles = legendas
player-lane-shown = liga ou desliga
player-timeline = linha do tempo
player-search = buscar
player-favourite = favorito
player-nearer = mais perto
player-further = mais longe
player-world = inteiro
player-layout = disposição
player-every-screen = cada tela
ask-player-together = Para tocar junto (um caminho ou um endereço)
ask-player-rename = Um nome novo (vazio: o próprio)
ask-player-save-set = O nome do conjunto
ask-player-save-queue = O nome da fila
ask-player-lane-start = Começa em (ou +1,5)
ask-player-lane-until = Toca até (ou +1,5)
ask-player-lane-title = Nome dela (vazio: o do arquivo)
ask-player-tag-title = Título
ask-player-tag-artist = Artista
ask-player-tag-album = Álbum
ask-player-tag-album-artist = Artista do álbum
ask-player-tag-track = Número da faixa
ask-player-tag-disc = Número do disco
ask-player-tag-date = Data
ask-player-tag-genre = Gênero
ask-player-tag-composer = Compositor
ask-player-tag-lyricist = Letrista
ask-player-tag-publisher = Gravadora
ask-player-tag-isrc = ISRC
ask-player-tag-show = Série
ask-player-tag-season = Temporada
ask-player-tag-episode = Episódio
ask-player-tag-director = Direção
ask-player-tag-name = Nome da etiqueta nova
ask-player-tag-value = O valor dela
ask-player-tag-value-all = O valor dela (substitui tudo o que ela tem)
player-heading-now = Agora
player-heading-media = Mídias do arquivo
player-heading-media-set = Mídias dos arquivos
player-heading-listen = Ouvir e assistir
player-heading-features = Recursos variados
# One key a page (`player_page::PageId::key`): the row that opens it says it,
# and the page is titled by it.
player-page-files = Arquivos
player-page-files-together = Algo para tocar junto
player-page-files-open = Abrir conjunto ou fila
player-page-together = Tocar junto
player-page-internet = Internet
player-page-machines = Outras máquinas
player-page-sound = Som
player-page-picture = Imagem
player-page-subtitles = Legendas
player-page-gone = O que esta página mostrava não existe mais.
# The channels by theme, one name a category: `assets/channels.nix` declares
# the ids, `nix/broadcast.nix` lists the keys, and `channels-have-text` holds
# both languages to that list.
player-channels-action = Ação
player-channels-anime = Anime
player-channels-cartoons = Desenhos
player-channels-classic = Clássicos
player-channels-comedy = Comédia
player-channels-documentary = Documentários
player-channels-films = Filmes
player-channels-kids = Infantil
player-channels-music = Música
player-channels-news = Notícias
player-channels-relax = Relaxar
player-channels-science = Ciência
player-channels-scifi-horror = Ficção científica e terror
player-channels-series = Séries
player-channels-sports = Esportes
player-channels-travel = Viagem
player-channels-general = Geral
player-channels-entertainment = Entretenimento
player-channels-culture = Cultura
player-channels-lifestyle = Estilo de vida
player-channels-cooking = Culinária
player-channels-outdoor = Ao ar livre
player-channels-auto = Carros
player-channels-business = Negócios
player-channels-weather = Previsão do tempo
player-channels-public = Televisão pública
player-channels-legislative = Parlamentos
player-channels-religious = Religião
player-channels-shop = Compras
player-channels-interactive = Interativos
# Os sons ambientes, um nome por id de `assets/ambience.nix`; um som
# sem nome nesta porta é dito como tal, nunca só pelo id.
player-ambience-small-waterfall = Pequena cachoeira
player-ambience-rain-and-thunder = Chuva e trovão
player-ambience-thunderstorm = Trovoada com chuva
player-ambience-unnamed = Um som sem nome neste player ({ $id })
player-ambience-group-water = Água
player-ambience-group-storm = Tempestades
player-ambience-group-unnamed = Sons de um tipo sem nome neste player ({ $group })
# THE AMBIENT SOUNDS' PAGE (`player_ambience`) and its credits.
player-page-ambience = Sons ambientes
player-page-ambience-credits = Créditos
player-about-ambience = Gravações de água e de tempestades que tocam junto da fila, sem parar, até serem desligadas. As teclas da fila nunca as param.
player-ambience-held = em pausa
player-ambience-hold = Pausar todos
player-ambience-silence = Silenciar todos
player-ambience-silenced = Todos os sons ambientes foram desligados.
player-ambience-count = { $count ->
        [one] { $count } som
       *[other] { $count } sons
    }
player-about-credits = Cada som aqui é uma gravação que alguém deu a todos: quem a fez, onde a publicou e sob que licença.
player-credit-by = Gravado por { $author }
player-credit-page = Publicado em { $page }
player-licence-cc0 = CC0: doado ao domínio público
player-licence-public-domain = Domínio público
player-not-running = O player não está rodando.
# Um player MAIS ANTIGO que o resto do sistema, que pode descartar o que
# não lê (`player::older`): dito, com a linha que o reinicia.
player-old-daemon = Este player é mais antigo que o resto do sistema e pode não entender o que se pede a ele. Reiniciar traz o novo; a fila fica guardada.
player-restart = Reiniciar o player
player-ask-restart = Reiniciar o player? O que toca para; a fila fica guardada.
player-restarting = Reiniciando o player…
player-pair-no-answer = A outra máquina não respondeu a tempo. Confira o nome, o código e se ela está ligada.
player-nothing = Nada carregado.
player-playing = Tocando
player-state = Estado
player-state-playing = tocando
player-state-paused = pausado
player-position = Posição
player-volume = Volume
player-muted = mudo
player-speed = Velocidade
player-repeat = Repetir
player-repeat-nothing = nada
player-repeat-queue = a fila
player-repeat-one = este
player-shuffle = Embaralhar
player-track-in-use = em uso
player-subtitles-off = Sem legendas
# THE SUBTITLES' PAGE (`player_subtitles`): the lanes over the picture, and
# where more come from; and a lane's own page (`player_lane`).
player-page-files-subtitle = Adicionar arquivo de legenda
player-page-lane = Faixa
player-about-subtitles = Até seis legendas ficam sobre a imagem ao mesmo tempo, cada uma na sua cor.
player-hidden = escondida
player-subtitle-bottom = embaixo
player-subtitle-top = em cima
player-lane-merged-plain = Com outra legenda sobre a imagem, ela veste a sua cor e perde os próprios estilos.
player-heading-beside = Ao lado do arquivo
player-heading-inside = Dentro do arquivo
player-subtitles-beside-none = Nenhum arquivo de legenda ao lado tem o nome dele.
player-subtitles-inside-none = O arquivo não traz legendas próprias.
player-subtitle-added = { $name } vai sobre a imagem.
player-subtitles-shown = { $count ->
        [one] { $count } visível
       *[other] { $count } visíveis
    }
player-lane-source = Arquivo
player-lane-on = Toca
player-lane-stack = Onde fica
player-lane-remove = Tirar
player-page-timeline = Linha do tempo
player-about-timeline = Todas as faixas do que toca, num relógio só. Arraste uma banda para mudar quando a faixa começa; um clique nela abre a página da faixa.
player-page-layout = Disposição
player-about-layout = Como as imagens do que toca dividem a tela: a forma, e o lugar de cada imagem — mostrada ou escondida, movida pelas setas no fim da linha dela. As telas de cada imagem se escolhem na página dela.
player-layout-form = Forma
player-form-auto = automática
player-form-single = uma imagem
player-form-side = lado a lado
player-form-stacked = empilhadas
player-form-grid = grade
player-form-strip = grande e faixa
player-form-corner = no canto
player-heading-layout-pictures = Imagens
player-layout-shown = mostrada
player-layout-cover = capa
player-layout-nothing = Nada do que toca tem imagem para dispor.
player-layout-no-room = sem espaço
player-layout-left-out = Uma imagem que ficaria pequena demais aqui nesta forma fica de fora; uma porta maior, ou outra forma, a mostra.
player-feature-every-screen = Mostrar em cada tela
player-every-screen-opened = O que toca ocupa { $count ->
        [one] uma tela inteira
       *[other] { $count } telas inteiras
    }; o mesmo de novo as fecha.
player-every-screen-closed = As janelas de tela inteira foram fechadas.
player-every-screen-one = Há uma tela só, e o que toca já está nela.
player-every-screen-nothing = Nada do que toca tem imagem para as telas.
player-every-screen-failed = Não foi possível abrir as janelas de tela inteira: { $why }
player-heading-lane-screens = Telas
player-lane-every-screen = Em todas as telas
player-screen-built-in = Tela embutida ({ $connector })
player-screen-named = { $description } ({ $connector })
player-screen-absent = não conectada
player-screen-last = Uma imagem vai para pelo menos uma tela; para não mostrá-la em nenhuma, esconda-a.
player-screens-unknown = Esta porta ainda não sabe o nome das telas.
player-heading-lane-sound = Saída de som
player-sink-default = Saída padrão
player-sink-headphones = fone
player-sink-speakers = alto-falantes
player-sink-bluetooth = Bluetooth
player-sink-other = outra saída
player-sink-gone = não está lá
player-sink-twin = { $name } ({ $n })
player-sinks-none = Nenhuma outra saída é conhecida ainda.
player-lane-start = Começa em
player-lane-start-refused = “{ $said }” não é um tempo: m:ss, h:mm:ss, segundos ou +/- segundos.
player-lane-until = Toca até
player-lane-until-end = o fim
player-lane-until-too-soon = Ela tem de parar depois de onde começa no arquivo ({ $from }).
player-lane-until-back = Ela volta a tocar até o fim.
player-lane-title = Nome
player-lane-over = Sobre a imagem
player-lane-over-none = nenhuma imagem toca
player-lane-volume = Volume próprio
player-lane-mute = Sem som
player-lane-picture-shown = Imagem na tela
player-lane-cover-shown = Capa na tela
about-player-lane-toggle = Liga ou desliga esta faixa
about-player-lane-name = A página desta faixa
about-player-lane-band = Arraste muda o início; um clique abre a página dela
about-player-deck-seek = Clique ou arraste para percorrer o que toca
about-player-playhead = Onde o que toca está agora: arraste para percorrer
about-player-lane-mute = Silencia esta faixa, ou devolve o som dela
about-player-lane-volume = O volume desta faixa: clique ou arraste, ou gire a roda sobre a linha dela
about-player-deck-volume = O volume do player: clique ou arraste, ou gire a roda
player-lane-about = { $name }: { $about }
player-lane-twin = { $name } · { $kind ->
        [video] imagem
        [audio] som
       *[subtitle] legenda
    }
player-tracks-none = Sem mídias a escolher aqui.
# One of the file's media that another lane plays — added to play too.
player-track-too = também toca
player-track-plays-already = Essa já está tocando.
player-also-added = { $name } também toca.
# A window opened for the picture alone (`player_stage`) has no rows to
# bring back: a key that needs them says where they are.
player-stage-has-no-rows = Aqui só há a imagem; as linhas do player estão no rodapé, numa janela ou na coluna.
player-panel-no-room = Sem espaço para a imagem aqui; a tecla { $keys } a mostra sozinha.
player-heading-lyrics = Letra
player-lyrics-source = Origem
player-lyrics-from-file = arquivo .lrc
player-lyrics-from-tag = tags do arquivo
player-lyrics-from-host = { $host }
player-heading-queue = Fila
player-queue-empty = A fila está vazia.
# Um item que o player ainda lê da rede: aparece, e ainda não toca.
player-resolving = { $name } — lendo…
player-about-together = O que toca, faixa por faixa num relógio só: as imagens, os sons e as legendas, cada um do seu arquivo.
# What a lane of what plays is doing, beside its glyph and its name.
player-lane-apart = desvio de { $ms } ms
player-lane-pending = aguardando
player-lane-ended = terminou
player-lane-preparing = preparando
player-lane-refused = não toca
player-lane-switched-off = desligada
# Uma faixa do que toca: de que tipo ela é — duas faixas de um arquivo têm
# um nome só — e a que distância do relógio, ou que está desligada.
player-together-lanes = { $count ->
        [one] uma faixa
       *[other] { $count } faixas
    }
player-together-added = { $name } toca junto com o que toca.
player-lyrics-online = Letras do { $host }
player-sponsorblock = Pular trechos patrocinados ({ $host })
player-skip-sponsor = Patrocínios
player-skip-selfpromo = Autopromoção
player-skip-interaction = Pedidos de like ou inscrição
player-skip-intro = Aberturas
player-skip-outro = Créditos e telas finais
player-skip-preview = Prévias e recapitulações
player-skip-hook = Ganchos no começo
player-skip-music-offtopic = Conversa dentro de um clipe musical
player-skip-filler = Enrolação e desvios
player-trouble-write = Não foi possível gravar os ajustes do player: { $why }
cmd-player-pair = Parear com outra máquina da tailnet
cmd-player-machines = As outras máquinas da tailnet: tocar junto com uma
ask-player-pair = O nome da outra máquina, como a tela dela o mostra
ask-player-pair-code = O código que a outra máquina mostra
player-no-tailnet = Esta máquina não está numa tailnet, então nenhuma outra pode tocar junto com ela.
player-about-machines = Toque junto com outra máquina da tailnet: como a tela dela, como o alto-falante dela, ou os dois. Pareie uma vez com o código que a outra mostra.
player-join = juntar
player-joined-as = { $role }, { $ms } ms de diferença
player-guest = pode se juntar a esta
player-invite = Mostrar um código para parear
player-pair = Parear com uma máquina…
player-role-both = imagem e som
player-role-screen = só a imagem
player-role-sound = só o som
player-trouble-remote = A outra máquina: { $why }
player-quit-everywhere = Fechar o player em todo lugar
# The player's own files, and another name for what plays: rows of "More
# features", each asking on the door's line.
player-feature-save-set = Salvar conjunto…
player-feature-save-queue = Salvar fila…
player-feature-rename = Renomear…
player-renamed = Agora se chama { $name }.
player-renamed-back = { $was } voltou ao próprio nome.
player-saving = Gravando { $name } em { $folder }.
player-about-open = Um conjunto toca agora, logo depois do que toca. Uma fila toma o lugar da fila.
player-ask-replace-queue = Pôr { $name } no lugar da fila? A fila como está agora sai.
player-queue-opened = { $name } agora é a fila.
player-open-not-ours = { $name } não é um conjunto (.rset) nem uma fila (.rqueue).
player-heading-question = Uma pergunta
player-answer-no = Não
player-answer-yes = Sim
player-added = { $name } foi posto na fila.
player-playing-now = Tocando { $name } agora; a fila fica.
player-ask-clear = Tirar tudo da fila?
player-ask-quit = Fechar o player em todo lugar? O que toca para, e toda janela, coluna e vista dele se fecha. A fila fica guardada para a próxima vez.
player-queue-cleared = A fila está vazia agora.
player-not-installed = O player não está instalado nesta máquina: o perfil dela o deixa de fora (ronoctua.player.enable).
player-column-refused = A coluna não pôde ser levantada: { $why }
player-browse-nothing-to-play = Nada nesta pasta é algo que o player toque.
player-trouble-unplayable = Não foi possível tocar isto: { $why }
player-trouble-no-decoder = Este formato precisa de { $program }, que não está instalado.
player-trouble-unsupported = Nenhum decodificador aqui lê { $format }.
player-trouble-no-engine = O player não conseguiu subir o motor: { $why }
# Uma frase para cada problema que o player diz (`player::trouble_sentence`).
player-trouble-not-understood = O player não entendeu o pedido — ele e o resto do sistema são de versões diferentes: { $why }
player-trouble-too-many-leaves = A fila comporta no máximo { $most } itens.
player-trouble-too-many-depth = Playlists vão no máximo { $most } níveis.
player-trouble-too-many-lanes = No máximo { $most } faixas tocam juntas.
player-trouble-too-many-subtitles = No máximo { $most } legendas vão sobre uma imagem.
player-trouble-too-many-subtitle-pictures = Legendas desenhadas como imagem: no máximo { $most } sobre uma imagem.
player-trouble-too-many-voices = No máximo { $most } arquivos tocam ao mesmo tempo num conjunto.
player-trouble-too-many-pictures = No máximo { $most } imagens aparecem ao mesmo tempo.
player-trouble-too-many-ambience = No máximo { $most } sons ambientes tocam juntos.
player-trouble-no-such-entry = Esse item não está mais na fila.
player-trouble-not-a-playlist = Só uma playlist guarda itens.
player-trouble-into-itself = Uma playlist não entra nela mesma.
player-trouble-nameless = Uma playlist precisa de um nome.
player-trouble-unreadable = { $path } não pôde ser lido.
player-trouble-unwritable = { $path } não pôde ser gravado: { $why }
player-trouble-nowhere-to-save = { $path }: nenhuma pasta aceitou o que foi buscado para ele — nem ao lado dele, nem a do próprio player.
player-trouble-no-runtime-dir = Esta sessão não tem pasta de execução (XDG_RUNTIME_DIR), onde o player guarda os rascunhos e o que buscou.
player-trouble-too-large = { $path } é maior que qualquer arquivo que o player grava.
player-trouble-not-ours = { $path } não é um arquivo do player, diga o nome o que disser.
player-trouble-newer = { $path } foi salvo por um player mais novo (versão { $found }).
player-trouble-damaged = { $path } está danificado: algo nele não é o que o formato diz.
player-trouble-empty = { $path } não aponta nada que toque.
player-trouble-missing = { $program } não está nesta máquina, e isto precisa dele.
player-trouble-sink-gone = { $device } não está lá; a faixa toca na saída padrão.
player-trouble-unreachable = { $what } não respondeu: { $why }
player-trouble-stream-down = { $tried ->
        [one] { $title }: o endereço dele falhou.
       *[other] { $title }: os { $tried } endereços dele falharam.
    }
player-trouble-listing-absent = { $what }: esta máquina não tem o yt-dlp para listá-la.
player-trouble-listing-unstartable = { $what }: o yt-dlp não pôde ser iniciado.
player-trouble-listing-slow = { $what }: o yt-dlp demorou demais e foi parado.
player-trouble-listing-failed = { $what }: o yt-dlp não conseguiu listá-la — sem rede, uma lista que foi removida ou um endereço que ele não conhece.
player-trouble-listing-garbled = { $what }: o yt-dlp respondeu algo que não é uma lista.
player-trouble-listing-too-large = { $what }: a lista é maior que o player lê.
player-trouble-listing-not-an-address = { $what } não é um endereço da web.
player-trouble-listing-not-a-list = { $what } é um vídeo só, não uma lista.
player-trouble-listing-nothing-to-ask = { $what }: não havia palavras para buscar.
player-trouble-subtitle = As legendas de { $path } não puderam ser lidas: { $why }.
player-subtitle-refusal-too-large = maior que { $most } MiB, o que nenhum arquivo de legendas é
player-subtitle-refusal-unknown = não é um formato de legenda que o player conheça
player-subtitle-refusal-damaged = o arquivo está danificado
player-subtitle-refusal-empty = o arquivo não tem legendas
player-subtitle-refusal-untimed = letra sem tempos: nada diz quando mostrar
player-subtitle-refusal-too-many-cues = mais de { $most } legendas num arquivo só
player-subtitle-refusal-too-slow = ler levou mais de { $most } s
player-subtitle-refusal-streamed = na rede, elas aparecem só sozinhas, nunca empilhadas com outras
player-subtitle-refusal-no-index = o .sub do VobSub precisa do .idx ao lado
player-trouble-no-picture = { $path } não tem sobre o que aparecer: nenhum filme da fila tem o nome dela, nada com imagem está tocando e nenhum filme ao lado tem o nome dela.
player-trouble-nothing-playing = Nada está tocando.
player-trouble-no-such-lane = Essa faixa não faz mais parte do que toca.
player-trouble-wrong-kind = Isso não vale para esta faixa.
player-trouble-bad-value = Esse valor não cabe nesta faixa.
player-trouble-last-lane = Um conjunto fica com pelo menos uma faixa.
player-trouble-following = Tocando junto com { $host }: quem decide é aquela máquina.
player-trouble-stream-plays-alone = Um stream da rede toca sozinho.
player-trouble-no-ambience = Esta máquina ainda não tem sons ambientes: eles vêm com o player quando as gravações forem publicadas.
player-trouble-no-such-sound = Não há som ambiente chamado { $id }.
player-trouble-sound-unplayable = { $sound } não tocou: { $why }
player-trouble-tags = { $name } — { $why }
player-trouble-no-such-draft = Esse rascunho já foi gravado ou descartado.
player-trouble-too-many-drafts = No máximo { $most } campos abertos no editor ao mesmo tempo.
player-trouble-too-many-writes = No máximo { $most } mudanças esperando gravação.
player-trouble-too-many-takes = No máximo { $most } downloads esperando.
player-service-lrclib = LRCLIB
player-service-sponsorblock = SponsorBlock
player-service-musicbrainz = MusicBrainz
player-service-coverartarchive = Cover Art Archive
player-service-wikidata = Wikidata
player-service-tmdb = TMDB
player-service-opensubtitles = OpenSubtitles
player-service-subdl = SubDL
player-service-podnapisi = Podnapisi
player-trouble-lookup-no-key = { $service } precisa da sua chave: guarde-a na página Internet.
player-trouble-lookup-key-refused = { $service } recusou a chave, o token ou a conta.
player-trouble-lookup-wrong-key = Essa é a chave v3 do TMDB; guarde o Token de Leitura da API no lugar dela.
player-trouble-lookup-quota = { $service }: os downloads de hoje acabaram.
player-trouble-lookup-too-many-asks = { $service } pediu para ir mais devagar; tente de novo daqui a pouco.
player-trouble-lookup-nothing-found = { $service } não achou nada.
player-trouble-lookup-nothing-to-ask = Nada com que perguntar a { $service }: sem título e sem palavras.
player-trouble-lookup-unreachable = Não foi possível falar com { $service }.
player-trouble-lookup-malformed = { $service } respondeu algo que este player não lê.
player-trouble-lookup-too-large = { $service } respondeu mais do que uma resposta deveria ter.
player-trouble-lookup-declined = { $service } recusou.
player-trouble-lookup-unreadable = Não foi possível ler o arquivo para tirar a impressão dele.
player-trouble-lookup-too-small = O arquivo é pequeno demais para a impressão; foi perguntado pelo nome.
player-trouble-lookup-dormant = { $service } está dormindo nesta versão: o domínio dele não responde desde junho de 2026.
player-trouble-off = Não perguntado: { $host } ainda não foi permitido.
player-trouble-exists = { $path } já existe.
player-trouble-no-such-candidate = Esse resultado é de uma busca anterior.
player-source-radio = radio-browser
player-source-iptv = iptv-org
player-source-archive = o Internet Archive
player-trouble-directory-unreachable = Não foi possível falar com { $source }.
player-trouble-directory-declined = { $source } recusou.
player-trouble-directory-too-many-asks = { $source } pediu para ir mais devagar; tente de novo daqui a pouco.
player-trouble-directory-nothing-found = { $source } não achou nada.
player-trouble-directory-malformed = { $source } respondeu algo que este player não lê.
player-trouble-directory-too-large = { $source } respondeu mais do que uma lista deveria ter.
player-trouble-directory-nothing-playable = Nada ali é algo que este player toque.
player-trouble-directory-nothing-to-ask = Nada com que perguntar a { $source }: digite algumas palavras.
player-trouble-directory-not-a-name = Isso não é um nome que { $source } dá; nada foi enviado.
player-trouble-no-such-category = Este tema não tem isso.
player-trouble-no-such-pick = Isso não está mais em nenhuma lista: abra de novo para guardar.
player-trouble-picks-no-home = Não há pasta pessoal onde guardar a lista.
player-trouble-picks-unreadable = A lista no disco não pôde ser lida, e ficou como estava.
player-trouble-picks-newer = A lista foi gravada por um player mais novo.
player-trouble-picks-unwritable = Não foi possível gravar a lista.
player-heading-metadata = Metadados
player-heading-technical = Técnico
player-meta-file = Do arquivo
player-meta-reading = Lendo o que o arquivo diz de si…
player-meta-still-reading = O player ainda está lendo este arquivo — ele pode estar num disco lento. Ler de novo pergunta outra vez.
player-field-blank = Nada escrito neste campo.
player-field-edit = Editar no editor
player-meta-fixed = Nada disto muda aqui. { $why }
player-meta-refresh = Ler de novo
player-meta-refreshing = Lendo o arquivo de novo.
player-meta-count = { $count }
player-meta-joiner = { " · " }
player-list-joiner = {", "}
player-meta-cover-size = { $width } × { $height }
player-meta-cover-none = nenhuma
player-meta-cover-some = no arquivo
player-meta-cover-nothing = Este arquivo não traz capa.
player-page-cover = Imagem da capa
player-page-files-cover = Outra imagem para a capa
player-page-streams = Fluxos
player-page-chapters = Capítulos
player-page-every-tag = Todas as etiquetas
player-cover-save = Salvar ao lado do arquivo
player-cover-remove = Tirar do arquivo
player-ask-remove-cover = Tirar a capa de { $name }? Nenhuma cópia dela fica guardada.
player-cover-putting = Pondo { $picture } em { $name }.
player-cover-saving = Salvando a capa ao lado de { $name }.
player-cover-removing = Tirando a capa de { $name }.
player-cover-saved = A capa foi salva como { $name }.
player-page-suggestions = Sugestões
player-page-candidate = Sugestão
player-page-find-subtitles = Buscar legendas
player-page-find-lyrics = Buscar letras
player-page-languages = Escolher as línguas
player-page-radio = Rádios do mundo
player-page-radio-genres = Gêneros
player-page-radio-languages = Línguas das rádios
player-page-stations = Estações
player-page-tv = TVs do mundo
player-page-tv-channels = Canais de TV
player-page-channels = Canais por tema
player-page-favourites = Favoritos
player-page-recent = Tocados há pouco
player-about-radio = Estações de rádio do mundo inteiro, do radio-browser: por país, por gênero, por língua ou pelo nome.
player-about-tv = Canais de TV do mundo inteiro, da lista pública do iptv-org: por país ou pelo nome.
player-about-favourites = O que você guardou com a estrela: estações, canais, filmes e lives.
player-about-recent = As transmissões que tocaram há pouco, como o player se lembra delas.
player-favourites-none = Nada guardado ainda.
player-recent-none = Nada tocado ainda.
player-radio-search = Buscar estações pelo nome
player-radio-genre-search = Achar um gênero pelo nome
player-radio-language-search = Achar uma língua pelo nome
player-about-radio-genres = Os gêneros com mais estações. Qualquer outro se acha pelo nome, como o radio-browser o escreve.
player-about-radio-languages = As línguas com mais estações. Qualquer outra se acha pelo nome, como o radio-browser a escreve — em inglês.
player-tv-search = Buscar canais pelo nome
player-heading-countries = Países
player-picks-radio = Estações de rádio
player-picks-tv = Televisão
player-picks-archive = Filmes do acervo
player-picks-live = Transmissões ao vivo
player-stations-count = { $count ->
        [one] uma estação
       *[other] { $count } estações
    }
player-channels-count = { $count ->
        [one] um canal
       *[other] { $count } canais
    }
player-globe-placeless = Sem lugar no mapa ({ $count })
player-globe-no-country = De país nenhum
player-globe-no-place = Sem lugar no mapa
player-globe-place = { $count } · { $place }
player-globe-one = { $name } · { $country }
player-radio-bitrate = { $kbps } kbps
player-radio-hls = transmissão adaptativa
player-stream-silent = não respondeu
player-tv-lines = { $lines } linhas
player-tv-geo-blocked = só em alguns países
player-tv-sometimes-off = nem sempre no ar
player-tv-shown = { $shown } mostrados dos { $total } encontrados.
player-stations-page-of = Página { $page } de { $pages } — { $total } estações ao todo, as mais tocadas primeiro.
player-stations-page = Página { $page } desta lista — { $most } por página, as mais tocadas primeiro.
player-stations-more = Mais estações
player-stations-earlier = Estações anteriores
player-listed-at = Lista de { $date }
player-listed-updating = Atualizando…
player-listing-nothing = Nada listado ainda.
player-listing-again = Tentar de novo
player-listing-every = Mostrando { $shown } de { $total }: mostrar todos
player-searched-title = Com o nome “{ $words }”
player-favourite-added = Guardado nos favoritos.
player-favourite-removed = Tirado dos favoritos.
player-about-internet = Nada aqui chega à internet até você permitir, e cada linha diz a quem pergunta. As do primeiro título perguntam sozinhas; as outras só quando você pressiona uma linha que as nomeia, e pressionar permite.
player-heading-on-their-own = Perguntados sozinhos
player-heading-when-asked = Perguntados ao pressionar
player-heading-keys = Suas chaves
player-heading-results = Resultados
player-consent-allowed = permitido
player-consent-not-allowed = não permitido
player-consent-by-the-switch = e a chave acima pergunta sozinha
player-consent-ask = Perguntar a { $hosts }
player-consent-first = pressionar permite
player-consent-let = Permitir { $host } e perguntar de novo
player-consent-hosts = pergunta a { $hosts }
player-key-sent-to = enviada só a { $host }
player-directory-archive = Filmes do Internet Archive
player-directory-lives = Lives do YouTube
player-key-opensubtitles = Chave do OpenSubtitles
player-key-opensubtitles-account = Conta do OpenSubtitles
player-key-tmdb = Chave de acesso do TMDB (token)
player-key-subdl = Chave do SubDL
player-key-words-opensubtitles = a chave do OpenSubtitles
player-key-words-opensubtitles-account = a conta do OpenSubtitles
player-key-words-tmdb = a chave de acesso do TMDB
player-key-words-subdl = a chave do SubDL
player-key-kept = guardada
player-key-none = não guardada
player-key-subdl-address = O SubDL só aceita a chave no endereço, então ela aparece nos registros do próprio SubDL.
player-ask-forget-key = Esquecer { $what }? O player não guarda outra cópia.
player-key-checking = Guardada. Perguntando a { $service } se aceita…
player-key-accepted = { $service } aceitou a chave.
player-key-forgotten = Chave esquecida.
player-key-nothing-to-forget = Não há o que esquecer: este computador não guarda { $what }.
player-key-trouble-no-home = Não há pasta pessoal onde guardá-la.
player-key-trouble-empty = Nada foi digitado.
player-key-trouble-malformed = Isso tem uma quebra de linha ou outro caractere de controle.
player-key-trouble-too-long = Longa demais para uma chave.
player-key-trouble-unwritable = Não foi possível gravar a chave.
player-key-trouble-unreadable = Não foi possível ler a chave guardada.
player-prefs-will-not-parse = O player.json não é lido, e gravar substituiria o que há nele.
player-desk-asking = Perguntando a { $hosts }…
player-desk-found = { $count ->
        [one] Um resultado
       *[other] { $count } resultados
    }
player-desk-searched-for = Buscado: { $words }
player-desk-by-fingerprint = Encontrado pela impressão digital do arquivo.
player-desk-by-id = Consultado pelo id que o arquivo traz.
player-desk-nothing-found = Nada encontrado.
player-take-taking = Buscando…
player-take-handed = Gravando no arquivo…
player-take-saved = Salvo como { $name }.
player-about-suggestions = Etiquetas para este arquivo, vindas da internet. A linha que pergunta diz a quem, e pressioná-la permite. Nada é gravado até você escolher o que levar.
player-suggest-source = Sugerido por
player-suggest-came-from = Estes vieram do serviço { $service }.
player-suggest-poster-needs-token = O pôster só vem com o TMDB e o seu token; o Wikidata quase nunca tem.
player-suggest-other-words = Buscar com outras palavras
player-suggest-reading = lendo os detalhes…
player-suggest-busy = perguntando…
player-about-candidate = Escolha o que gravar: um campo em branco no arquivo já começa escolhido. O que o arquivo diz agora está sob cada valor.
player-suggest-now = agora: { $value }
player-suggest-same = o arquivo diz o mesmo
player-suggest-cover = Capa
player-suggest-taken = será gravada
player-suggest-left = fica de fora
player-suggest-picture-suggested = Sugerida
player-suggest-picture-current = Atual
player-suggest-picture-none = Nenhuma capa veio
player-suggest-fetching = Buscando a capa…
player-suggest-nothing-chosen = Nada escolhido para gravar
player-suggest-write = { $count ->
        [one] Gravar um campo
       *[other] Gravar { $count } campos
    }
player-suggest-write-cover = { $count ->
        [0] Gravar a capa
        [one] Gravar um campo e a capa
       *[other] Gravar { $count } campos e a capa
    }
player-suggest-writing = Gravando o que foi escolhido em { $name }…
player-about-find-subtitles = Legendas para este filme vindas da internet, nas línguas abaixo. A sincronizada para este mesmo arquivo vem marcada. Escolher uma a baixa ao lado do filme e a põe sobre a imagem.
player-find-needs-a-key = Buscar legendas exige a sua chave do OpenSubtitles ou do SubDL: hoje não há fonte sem chave.
player-find-languages = Línguas
player-find-unknown-language = { $code } não é uma língua que este player conheça.
player-find-language-there = { $language } já está na busca.
player-find-language-added = { $language } também entra na busca.
player-find-language-removed = { $language } saiu da busca.
player-find-language-last = A busca de legendas precisa de pelo menos uma língua.
player-find-language-searched = { $language } está na busca; d, ou o glifo no fim da linha, a tira.
player-find-type-language = Digitar uma língua
player-about-languages = As legendas são buscadas em toda língua ligada. A lista tem todas as línguas em que os serviços buscam.
player-find-hash-match = sincronizada para este arquivo
player-find-hearing = para surdos
player-find-machine = tradução automática
player-find-downloads = { $count ->
        [one] um download
       *[other] { $count } downloads
    }
player-find-downloads-left = Downloads restantes hoje
player-find-taking = Baixando { $name }…
player-lyrics-one-source = Uma fonte, o LRCLIB: sem campo de língua e sem tradução.
player-lyrics-to = Salvar em
player-lyrics-to-beside = um .lrc ao lado da música
player-lyrics-to-tag = a letra da própria música
player-find-synced = sincronizada
player-find-plain = só o texto
player-find-instrumental = instrumental
player-find-same-length = a duração da música
player-find-off-by = { $seconds } s a mais ou a menos
player-ask-replace-lrc = Substituir { $name }? O que ele tem não é guardado.
player-lyrics-taking = Salvando a letra ao lado de { $name }…
player-lyrics-into-tag = Gravando a letra em { $name }…
ask-player-find-words = Buscar
ask-player-search-radio = Estações com o nome
ask-player-search-tv = Canais com o nome
ask-player-search-genre = Gênero
ask-player-search-language = Língua (em inglês, como o radio-browser a escreve)
ask-player-find-language = Língua, pelo nome ou pelo código (japonês, pt-BR)
ask-player-key-opensubtitles = Chave da API do OpenSubtitles
ask-player-key-tmdb = Token de Leitura da API do TMDB
ask-player-key-subdl = Chave da API do SubDL
ask-player-account-user = Usuário do OpenSubtitles
ask-player-account-password = Senha do OpenSubtitles
tongue-en = Inglês
tongue-pt-br = Português (Brasil)
tongue-pt = Português
tongue-es = Espanhol
tongue-ja = Japonês
player-streams-none = Nenhum fluxo deste arquivo pôde ser lido.
player-chapters-none = Este arquivo não tem capítulos.
player-tags-none = Nenhuma ferramenta leu etiqueta deste arquivo.
player-stream-label = { $index } · { $kind }
player-stream-kind-video = imagem em movimento
player-stream-kind-cover = imagem de capa
player-stream-kind-audio = trilha de som
player-stream-kind-subtitle = trilha de legenda
player-stream-kind-data = dados no tempo
player-stream-kind-attachment = arquivo anexo
player-stream-default = padrão
player-stream-forced = forçada
player-stream-hearing = para quem ouve pouco
player-chapter-untitled = sem título
player-origin-id3v2 = Etiquetas ID3v2
player-origin-id3v1 = Etiquetas ID3v1
player-origin-ape = Etiquetas APEv2
player-origin-vorbis = Comentários Vorbis
player-origin-mp4 = Etiquetas MP4
player-origin-riff-info = Etiquetas RIFF INFO
player-origin-aiff-text = Trechos de texto AIFF
player-origin-matroska-global = Etiquetas Matroska do arquivo inteiro
player-origin-matroska-track = Etiquetas Matroska de uma trilha
player-origin-matroska-other = Etiquetas Matroska de outras partes
player-origin-container = O contêiner, como o ffprobe lê
player-origin-stream-n = Fluxo { $index }, como o ffprobe lê
player-origin-exiftool = exiftool, { $group }
player-origin-stream = O que o stream diz de si
player-tag-value-cut = { $size } ao todo · { $head }…
player-tag-value-binary = { $size } de dados
player-tag-add = Acrescentar uma etiqueta
player-tag-bad-name = “{ $typed }” não é um nome para { $where }: { $rule }.
player-tag-name-rule-vorbis = letras, dígitos e pontuação, sem = nem acentos
player-tag-name-rule-ape = de 2 a 255 letras, dígitos ou pontuação, sem acentos
player-tag-name-rule-riff = quatro letras ou dígitos, como ISFT
player-tag-name-rule-aiff = NAME, AUTH, ANNO ou (c)
player-tag-name-rule-mp4 = uma palavra, ou ----:mean:name
player-tag-name-rule-ffmpeg = qualquer coisa sem =
player-tag-name-rule-plain = uma linha, de até um kibibyte
player-sheet-unread = Não foi possível ler { $source }. { $why }
player-sheet-source-lofty = as etiquetas
player-sheet-source-ffprobe = os fluxos e os capítulos
player-sheet-source-exiftool = a lista de todas as etiquetas
player-sheet-source-matroska = as etiquetas próprias do Matroska
player-tag-empty = vazio
player-tag-already-empty = { $field }: nada a apagar, o campo já está vazio.
player-tag-more-lines = { $lines } linhas · { $first }
player-draft-open = aberto no editor
player-draft-opened = Aberto no editor: salve lá, e aqui se pergunta antes de gravar.
player-draft-says = O que o editor salvou
player-draft-kept = { $field }: segue aberto no editor; volta a perguntar ao salvar.
player-ask-drop-draft = { $field }: jogar fora o rascunho aberto no editor? O arquivo fica como está.
player-draft-dropped = { $field }: o rascunho foi jogado fora.
player-ask-write-draft = { $field } de { $name }: o editor salvou algo novo. Gravar no arquivo?
player-ask-clear-tag = { $field } de { $name }: apagar?
player-tag-writing = { $field } de { $name }: gravando no arquivo.
player-tag-clearing = { $field } de { $name }: apagando.
player-tag-named-writing = { $tag } de { $name }: gravando no arquivo.
player-tag-named-removing = { $tag } de { $name }: tirando do arquivo.
player-ask-remove-tag = { $values ->
        [one] { $tag } de { $name }: tirar do arquivo?
       *[other] { $tag } de { $name }: tirar os { $values } valores dela?
    }
player-retag-writing = Gravando no arquivo… { $percent }%
player-retag-waiting = { $count ->
        [one] Mais uma mudança esperando.
       *[other] Mais { $count } mudanças esperando.
    }
player-tag-written = A última mudança foi gravada no arquivo.
player-tech-container = Contêiner
player-tech-length = Duração
player-tech-bit-rate = Taxa de bits
player-tech-video-codec = Codec de vídeo
player-tech-width = Largura
player-tech-height = Altura
player-tech-fps = Quadros por segundo
player-tech-audio-codec = Codec de áudio
player-tech-sample-rate = Taxa de amostragem
player-tech-channels = Canais
player-tech-channel-layout = Disposição dos canais
player-tech-bits-per-sample = Bits por amostra
player-tech-programs = Programas
player-tech-value-kbps = { $kbps } kb/s
player-tech-value-px = { $px } px
player-tech-value-hz = { $hz } Hz
# As recusas do player que o protocolo 3 trouxe.
player-trouble-already-queued = Isto já está na fila: { $name }.
player-trouble-changed-on-disk = O arquivo da fila mudou no disco desde que foi aberto ou salvo: { $path }
player-trouble-no-origin = Esta fila nunca foi aberta de um arquivo nem salva como um, então não há onde salvá-la de novo.
player-trouble-no-sound = Este arquivo não tem som para tocar.
player-trouble-not-yet = O player ainda não faz isto.
player-trouble-said = { $program } disse: { $text }
# Um texto de várias linhas, editado no lugar (as duas portas).
text-area-keep = guardar
about-text-area-keep = Guardar o que está escrito aqui.
text-area-discard = sair
about-text-area-discard = Sair do texto; se ele mudou e não foi guardado, você é perguntado antes.
ask-text-area-discard = Jogar fora o que foi escrito e não foi guardado?
text-area-no = Não, continuar escrevendo
text-area-yes = Sim, jogar fora
about-text-area = Um texto de várias linhas, digitado e mudado aqui.
text-area-not-kept = Nada guarda este texto aqui ainda; ele continua aberto como foi escrito.
player-channels-education = Educação
player-channels-space = Espaço
player-channels-history = História
player-channels-nature = Natureza e animais
player-channels-art = Arte e dança
player-channels-gaming = Jogos
player-channels-audiobooks = Audiolivros
player-channels-source-iptv = Canais de TV do iptv-org
player-channels-source-archive = Internet Archive
player-channels-source-lives = Ao vivo no YouTube
player-channels-source-nasa = Biblioteca de imagens e vídeos da NASA
player-channels-source-commons = Wikimedia Commons
player-channels-source-peertube = PeerTube, pelo SepiaSearch

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-media = O que o player toca
file-picker-filter-lanes = O que toca junto
file-picker-filter-saved = Conjuntos e filas
file-picker-filter-subtitles = Legendas
player-page-pick-save-set = Salvar o que toca como conjunto
player-page-pick-save-queue = Salvar a fila
player-added-several = { $count } arquivos postos na fila.
player-playing-several = Tocando { $count } arquivos agora.

## file info editing — everything the info panel can change

# Why the lyrics were not translated (`ronoctua_words::Why`).
player-trouble-translation = A letra não foi traduzida: { $why }
player-trouble-translation-empty = não há palavras para traduzir.
player-trouble-translation-too-long = a música é maior do que o tradutor aceita de uma vez.
player-trouble-translation-unknown-tongue = o tradutor não conhece esse idioma.
player-trouble-translation-wrong-pass = o consentimento dado é para outro endereço que não o do tradutor.
player-trouble-translation-no-key = o tradutor precisa de uma chave, e nenhuma foi guardada.
player-trouble-translation-key-refused = o tradutor recusou a chave.
player-trouble-translation-quota = a cota de hoje do tradutor acabou.
player-trouble-translation-paced = pedido vezes demais; tente de novo daqui a pouco.
player-trouble-translation-not-found = o tradutor não achou nada para isso.
player-trouble-translation-no-dictionary = não há dicionário para este idioma.
player-trouble-translation-unreachable = não foi possível alcançar o tradutor.
player-trouble-translation-failed = o tradutor respondeu com uma falha.
player-trouble-translation-garbled = o tradutor respondeu algo que não é uma tradução.
# Why a press was not kept in the consent file (`ronoctua_consent::Unkept`).
player-consent-unkept-no-home = Não há pasta pessoal onde guardar o consentimento.
player-consent-unkept-unparseable = O arquivo de consentimento (consent.json) não pôde ser lido; ficou como estava, e nada foi permitido.
player-consent-unkept-unwritable = O arquivo de consentimento não pôde ser gravado; nada foi permitido.
player-lyrics-unsynced = Traduzida inteira, não verso a verso:
# AS PORTAS DO PLAYER (2026-09-30). O arquivo da fila no título dela.
player-queue-unsaved = { $name } · não salva
player-queue-new-unsaved = nova, não salva
player-save-queue-again = salvar de novo
cmd-player-save-queue-again = Salvar a fila de novo no arquivo de onde ela veio
player-saved-again = A fila foi salva de novo em { $name }.
ask-player-already-queued = { $name } já está na fila como { $number }. Pôr de novo?
ask-player-already-queued-somewhere = { $name } já está na fila. Pôr de novo?
ask-player-queue-changed-on-disk = { $path } mudou no disco desde que a fila foi aberta ou salva. Gravar por cima?
# Os saltos ao lado do relógio, e um clique duplo num lado da imagem.
player-rewind = voltar
player-forward = avançar
cmd-player-rewind = Voltar dez segundos
cmd-player-forward = Avançar dez segundos
player-seeked-back = −{ $seconds } s
player-seeked-ahead = +{ $seconds } s
# A imagem recolhida, no título "Agora".
player-picture-fold = recolher a imagem
cmd-player-picture-fold = Recolher a imagem e dar as linhas dela às linhas de baixo, ou mostrá-la de novo
player-no-picture-to-fold = Não há imagem aqui para recolher.
# Um vídeo de um site: página própria.
player-page-website = Vídeo de um site
player-about-website = Um vídeo ou uma música do YouTube, ou de um dos muitos outros sites que o yt-dlp conhece, toca pelo endereço da página, digitado ou colado na linha.
player-website-play = Tocar agora…
player-website-enqueue = Pôr na fila…
ask-player-website-play = Tocar agora (o endereço de uma página)
ask-player-website-enqueue = Pôr na fila (o endereço de uma página)
cmd-player-website = Um vídeo de um site: tocar agora ou pôr na fila, pelo endereço
player-website-recent = Endereços tocados há pouco
player-website-none = Nenhum endereço tocado daqui ainda.
player-website-not-an-address = “{ $typed }” não é o endereço de uma página.
player-website-forgotten = Esquecido: { $address }
player-file-no-home = não há pasta pessoal onde guardá-lo
player-file-unparseable = ele existe e não pôde ser lido, então ficou como estava
player-file-unwritable = não foi possível gravá-lo
# Um campo de várias linhas mudado aqui, nesta porta.
player-field-edit-here = Editar aqui
# Os países de uma página do mundo: os com mais primeiro, ou de A a Z.
player-countries-order = ordem
cmd-player-countries-order = Os países de A a Z pelo nome, ou os com mais primeiro
player-countries-by-count = Os países com mais primeiro.
player-countries-by-name = Os países de A a Z, pelo nome.
# A letra traduzida sob os versos.
player-lyrics-translate = traduzir
cmd-player-lyrics-translate = Traduzir a letra sob os versos, ou parar
player-lyrics-translate-to = Traduzir para
player-lyrics-translating = Traduzindo…
player-lyrics-translated-by = Traduzida por
player-lyrics-none-to-translate = Não há letra para traduzir.
player-lyrics-let = Deixar { $host } traduzi-la
player-consent-translation = Traduzir a letra
player-page-catalog = Listagem
player-page-sources = Fontes
player-catalog-page = Página
player-catalog-page-of = { $page } de { $pages }
player-catalog-layout = Exibição
player-catalog-layout-list = Lista
player-catalog-shown = { $from }–{ $to } de { $total }
player-adult-open = O catálogo adulto fica aberto até passarem 15 minutos sem tecla e sem clique.
player-adult-wrong = Essa não é a sua senha.
player-adult-no-service = Este sistema não tem o serviço ronoctua-unlock: o catálogo adulto fica fechado.
player-adult-unavailable = Não foi possível conferir a senha.
# POR QUE UMA FONTE NÃO DEU NADA (`ronoctua_broadcast::sources::Refused`).
player-trouble-source-not-an-address = Isso não é um endereço da web.
player-trouble-source-not-a-list = Isso é um vídeo ou uma transmissão, não uma lista deles.
player-trouble-source-unknown = Isso não é nenhuma das fontes lidas aqui: uma lista M3U, um feed RSS ou Atom, um canal ou playlist do YouTube, uma coleção do archive.org.
player-trouble-source-not-let = O host dela ainda não foi permitido.
player-trouble-source-plain-http = Ela foi mandada de um endereço criptografado para um que não é, o que nunca é seguido.
player-trouble-source-unreachable = Não foi possível alcançar o host dela.
player-trouble-source-declined = O host dela recusou a pergunta.
player-trouble-source-too-many-asks = O host dela pediu para ser perguntado menos.
player-trouble-source-nothing-found = Não há nada nesse endereço.
player-trouble-source-malformed = A resposta dela não é o que isto lê.
player-trouble-source-too-large = A resposta dela é maior do que o que é lido.
player-trouble-source-nothing-playable = Ela foi lida, e nada nela toca.
player-trouble-source-no-yt-dlp = Esta instalação não tem o yt-dlp para listá-la.
player-trouble-source-listing-failed = O yt-dlp não conseguiu listá-la.
# OS ARQUIVOS DO CATÁLOGO (`ronoctua_broadcast::catalog::Trouble`).
player-trouble-catalog-no-home = Não há pasta de cache para guardar o catálogo.
player-trouble-catalog-absent = Nada foi lido para o catálogo ainda.
player-trouble-catalog-other-schema = O catálogo é de outra versão e está sendo refeito.
player-trouble-catalog-unreadable = Não foi possível ler o catálogo.
player-trouble-catalog-unwritable = Não foi possível gravar o catálogo.
player-trouble-catalog-too-large = Isso passaria do teto do catálogo, então nada mudou.
player-trouble-sources-no-home = Não há pasta de configuração para guardar as suas fontes.
player-trouble-sources-unreadable = Não foi possível ler o arquivo das suas fontes.
player-trouble-sources-unparseable = O arquivo das suas fontes não é um que isto lê; ele fica como está, e nada é acrescentado a ele.
player-trouble-sources-unwritable = Não foi possível gravar o arquivo das suas fontes.
player-trouble-no-such-source = Essa fonte não existe.
# AS TECLAS DE UMA LISTAGEM.
cmd-player-listing-earlier = A página anterior da listagem
cmd-player-listing-later = A página seguinte da listagem
cmd-player-listing-first = A primeira página da listagem
cmd-player-listing-last = A última página da listagem
cmd-player-listing-full = A listagem sozinha na tela inteira
cmd-player-listing-layout = A próxima disposição da listagem
cmd-player-listing-source = A próxima fonte do catálogo
cmd-player-listing-letter = Uma inicial da listagem, digitada
cmd-player-listing-order = A próxima ordem da listagem
# AS TECLAS DE UMA LISTAGEM, COMO A LINHA DE DICAS AS DIZ.
player-listing-earlier = página anterior
player-listing-later = página seguinte
player-listing-first = primeira página
player-listing-last = última página
player-listing-full = tela inteira
player-listing-layout = exibição
player-listing-source = próxima fonte
player-listing-letter = inicial
player-listing-order = ordem
# A lookup or a directory whose host consent.json does not let: nothing
# was sent. The engine says it as the consent row (`Trouble::Off`); this is
# the sentence of the refusal itself, where it is said apart from that row.
player-trouble-lookup-not-let = Nada perguntado a { $service }: endereço não permitido.
player-trouble-directory-not-let = Nada perguntado a { $source }: endereço não permitido.
# THE RANKING OF THE VIDEO CATALOGUE, a page of the player (LEADER p k,
# player_ranking.rs). Titles and channel names are data.
player-page-ranking = Vídeos por pontuação
player-page-ranked-video = Este vídeo
cmd-player-ranking = Vídeos por pontuação: os melhores do catálogo de vídeos primeiro, a um clique da fila
player-ranking-play-now = Tocar agora
player-ranking-add = Pôr na fila
player-ranking-kiosk = Assistir no Chrome
player-ranking-download = Baixar
player-ranking-hide = Ocultar do ranking
player-ranking-delete = Apagar do catálogo
# THE RANKING'S KEYS.
cmd-player-ranking-more = A página deste vídeo: tocar, pôr na fila, Chrome, baixar, ocultar, apagar
cmd-player-ranking-earlier = A página anterior do ranking
cmd-player-ranking-later = A página seguinte do ranking
cmd-player-ranking-first = A primeira página do ranking
cmd-player-ranking-last = A última página do ranking
cmd-player-ranking-layout = O ranking em lista, ou em miniaturas
# THE RANKING'S KEYS, AS THE HINT LINE SAYS THEM.
player-ranking-more = mais
player-ranking-earlier = página anterior
player-ranking-later = página seguinte
player-ranking-first = primeira página
player-ranking-last = última página
player-ranking-layout = lista ou miniaturas
player-page-queue = A fila inteira
player-queue-entries = { $count ->
        [one] um item
       *[other] { $count } itens
    }
player-lane-language = Língua mostrada
player-lane-switched-off-length = desligada · { $length }
player-translator = Tradutor
player-translator-mymemory = MyMemory
player-translator-deepl = DeepL
player-translator-libretranslate = LibreTranslate
player-translator-address = Endereço do servidor
player-translator-not-an-address = “{ $typed }” não é um endereço https:// com um host: nada poderia ser perguntado ali.
player-key-deepl = Chave do DeepL
player-key-libretranslate = Chave do servidor
player-key-words-deepl = a chave do DeepL
player-key-words-libretranslate = a chave do servidor LibreTranslate
player-key-kept-said = Guardada: { $what }.
ask-player-translator-address = Endereço do servidor LibreTranslate (https://…)
ask-player-key-deepl = Chave de autenticação do DeepL
ask-player-key-libretranslate = Chave do servidor LibreTranslate
player-host-let = { $host } está permitido agora. Ponha de novo o que foi recusado.
player-stream-language-row = Língua do fluxo
player-stream-title-row = Nome do fluxo
player-stream-default-row = Marcado como padrão
player-stream-forced-row = Marcado como forçado
player-stream-language-of = a língua do fluxo { $index }
player-stream-title-of = o nome do fluxo { $index }
player-chapter-title-of = o título do capítulo em { $at }
ask-player-stream-language = A língua do fluxo, como código (en, pt-BR, por)
ask-player-stream-title = O nome do fluxo
ask-player-chapter-title = O título do capítulo
player-origin-matroska-track-of = Etiquetas Matroska do fluxo { $index } · { $kind }
player-origin-matroska-track-lang = Etiquetas Matroska do fluxo { $index } · { $kind } · { $lang }
player-origin-matroska-track-uid = Etiquetas Matroska da trilha de UID { $uid }
