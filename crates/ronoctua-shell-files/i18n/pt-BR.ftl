# The texts `ronoctua-shell-files` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

files-enter = abrir
files-parent = subir uma pasta
files-hint = digite para filtrar
# A OUTRA FORMA DA LISTAGEM. O que o glifo no fim da linha do caminho diz sob o
# ponteiro — para onde ele VAI, nunca onde ele está.
files-grid-on = os arquivos estão lado a lado, em miniaturas
files-grid-off = os arquivos voltaram a ser um por linha
# O QUE O ANEL DE TAMANHO DIZ QUE ELE PEGOU. Três chaves e não um seletor: um
# seletor escolhe o ramo pelo TIPO do argumento, e a linha a que isto chega é o
# `Step::Say`, cujos argumentos são strings por construção — então um número
# chegaria como texto e toda tela leria o ramo padrão.
files-grid-small = miniaturas pequenas
files-grid-medium = miniaturas médias
files-grid-large = miniaturas grandes
files-grid-moves = este se mexe — está parado porque outros estão se mexendo
files-grid-bigger = mostrar maiores
files-grid-none = os itens estão um por linha, então não há miniatura para dimensionar
files-no-root-helper = trocar o dono precisa do sistema instalado — a unidade ronoctua-files-root nao esta aqui
files-menu-on = ações em { $name }
files-menu-on-many = ações em { $n } marcados
files-ask-application = Abrir com qual aplicativo?
files-ask-owner = Trocar o DONO para quem?
files-ask-group = Trocar o GRUPO para qual?
files-ask-mode = Trocar as PERMISSÕES para qual conjunto?
files-ask-name = Renomear para:
files-sort = ordenação
files-sort-reverse = inverter a ordem
files-mark = marcar
files-mark-all = marcar tudo
files-unmark-all = desmarcar tudo
files-menu = menu de ações
files-measure = medir
files-pane-next = próxima seção
files-pane-previous = seção anterior
# E AS MESMAS DUAS SETAS, onde os itens estão lado a lado. Coisa diferente,
# palavra diferente: elas atravessam as SEÇÕES numa linha e andam pelos ITENS
# numa prateleira.
files-grid-next = próximo arquivo
files-grid-previous = arquivo anterior
files-type-is = { $named }
files-type-disagrees = o nome diz { $named }, o conteúdo diz { $found }
files-nothing-here = esta pasta está vazia
files-header = nomes das colunas
files-sidebar-focus = ir para a barra lateral, e voltar
files-preview-left = prévia para a esquerda
files-preview-right = prévia para a direita
files-grid = miniaturas
files-grid-size = tamanho da miniatura
files-eject = desmontar
files-unpin = tirar da barra
files-sidebar-hidden-ask = Trazer qual de volta?
files-sidebar-trash = lixeira
files-sidebar-open = abrir { $name }
files-sidebar-mount = montar { $name } e entrar
files-sidebar-unmount = desmontar { $name }
files-sidebar-launch = rodar { $name }
files-sidebar-hidden = { $name } saiu da barra — { $key } traz de volta
files-sidebar-gone = { $name } não está mais na barra lateral
files-sidebar-nothing-hidden = não há nada oculto para trazer de volta
files-mount-failed = não consegui montar { $name }: { $why }
files-mount-no-driver = este sistema não lê um sistema de arquivos { $type }
files-mount-needs-admin = { $name } é do sistema; { $key } abre um terminal root
files-unmount-busy = { $name } está em uso — feche o que está aberto ali
files-unmount-needed = { $name } é onde o sistema mora ({ $why }); desmontar não é oferecido
sidebar-menu-on = o que fazer com { $name }
sidebar-fact-where = montado em
sidebar-fact-needed = é onde o sistema mora
sidebar-not-mounted = não está montado
files-does-open-map-nowhere = este arquivo não tem coordenada gravada
files-disks-not-watching = os discos não vão se atualizar sozinhos: { $why }
files-nothing-to-choose = não há nada para escolher
files-no-pkexec = o `pkexec` não foi definido em nix/rust.nix
files-no-systemctl = o `systemctl` não foi definido em nix/rust.nix
# O que cada campo diz ao ponteiro. Campo que acende e não explica promete uma
# ação e não descreve nenhuma.
# THE WHOLE NAME, and only the name.
#
# The line stands in for a column that had to cut, so it says what the column
# would have said and nothing else: a sentence around it would push the end of
# a long name back off the edge, which is the thing this exists to undo.
files-explain-name = { $name }
# THE EXECUTE BIT, which is the one thing the type's glyph says besides the
# type. The owner asked for the colour to move off the name and onto the
# glyph, and a colour with no words is a legend nobody reads — so the pointer
# resting on a green glyph says what the green MEANS.
#
# It names the bit and not what running it would do: this manager does not run
# a file by pressing RET on it, and a sentence promising that would be a
# sentence about another program.
files-explain-executable = este arquivo tem o bit de execução — o sistema pode rodá-lo
files-explain-size = { $bytes } bytes exatos
files-no-trash = não sei onde fica a lixeira; nada foi movido
# ===================================================================
# WHAT IS INSIDE A FILE — the preview and the facts panel
#
# `ronoctua-inspect` is the border with a file's CONTENT, and it does not
# translate: it answers with a text KEY and its arguments, and whoever holds
# the bundle turns the pair into a sentence (invariant 8). Every number below
# arrives as DATA — what is written here is only the words around it.
# ===================================================================

# WHY THERE IS NOTHING TO SHOW. Drawn where the content would be, centred, so
# each one has to say whether there is anything to be done about it: "it
# failed" is not an answer, it is the absence of one.
#
# THE PANEL IS NOT AN IMAGE PANEL, and the sentence that said so is gone. It
# used to answer a markdown file with "there is no picture inside this kind of
# file", which is a true statement about a question nobody asked — the panel
# shows what is IN a file, and a file of letters is full of content. What is
# left here is a file that has neither picture nor text, and it now says what a
# folder says: `files-no-preview`, one sentence for one fact.
files-preview-not-text = este arquivo não é texto, apesar do que o nome diz
files-preview-empty = não há nada dentro deste arquivo
files-preview-unreadable = não deu para abrir este arquivo para leitura
files-preview-missing = { $program } não está instalado, e é ele que lê este formato
files-preview-too-big = grande demais para uma prévia ({ $amount })
# NOT "a picture". This is every reader's failure, not the raster path's: a
# Word document whose piece table does not add up, a presentation with no
# records, a program that ran and answered nothing. The owner read it off the
# screen against a `.doc` — a panel telling him a perfectly good document was
# "not a picture" is a sentence about the wrong thing twice over.
files-preview-failed = não deu para ler este arquivo — ele pode estar quebrado
files-preview-slow = passou de { $seconds } s e foi parado — não é o tamanho, é algo travado
# NÃO "quebrado". O arquivo está inteiro e é este leitor que não dá conta —
# dizer "quebrado" manda a pessoa consertar algo que não tem defeito.
files-preview-draco = as formas neste arquivo estão comprimidas, e este leitor não abre isso
files-preview-no-synth = este arquivo guarda uma partitura, não uma gravação, e não há aqui com que tocá-la
files-preview-playlist-unread = isto é uma lista do que tocar, numa grafia que este leitor não abre
files-preview-own-newer = isto foi salvo por um player mais novo que este
# THE PANEL'S LEFT COLUMN. Short nouns, and short on purpose: the column is as
# wide as the widest label here, so a needless word costs every other row.
files-fact-dimensions = dimensões
files-fact-duration = duração
files-fact-container = formato
files-fact-motion = foto com movimento
files-fact-video = vídeo
files-fact-audio = áudio
files-fact-rate = quadros
files-fact-sampling = amostragem
files-fact-channels = canais
files-fact-bitrate = taxa de bits
files-fact-title = título
files-fact-artist = artista
files-fact-album = álbum
files-fact-album-artist = artista do álbum
files-fact-track = faixa
files-fact-disc = disco
files-fact-year = ano
files-fact-genre = gênero
files-fact-composer = compositor
files-fact-publisher = editora
files-fact-isrc = ISRC
files-fact-encoder = codificador
files-fact-depth = bits por amostra
files-fact-layout = arranjo dos canais
files-fact-compression = compressão
files-fact-cover = capa
files-fact-camera = câmera
files-fact-lens = lente
files-fact-taken = feita em
files-fact-iso = ISO
files-fact-shutter = obturador
files-fact-aperture = abertura
files-fact-focal = focal
files-fact-place = lugar
files-fact-pages = páginas
# AND THE RIGHT ONE, when the number needs a word around it. A unit is language
# business — where it goes, and whether it is a word at all.
files-value-pixels = { $width } × { $height }
files-value-fps = { $fps } fps
files-value-hertz = { $khz } kHz
# ONE, TWO, OR A COUNT. Which numbers earn a word of their own is a decision of
# the LANGUAGE, and this is the shape Fluent has for it — `$n` has to arrive as
# a NUMBER (`Arg::Number`), or every branch but the last is unreachable.
files-value-channels = { $n ->
        [1] mono
        [2] estéreo
       *[other] { $n } canais
    }
files-value-kbps = { $kbps } kbit/s
files-value-of-total = { $at } de { $total }
files-value-bits = { $n ->
        [one] { $n } bit
       *[other] { $n } bits
    }
files-value-lossless = nada foi descartado
files-value-lossy = encolhido descartando som
files-value-millimetres = { $mm } mm
files-value-seconds = { $s } s
files-value-coordinates = { $lat }, { $lon }
# The other selector, and a plural one: in pt-BR `one` covers 0 as well.
files-value-pages = { $n ->
        [one] { $n } página
       *[other] { $n } páginas
    }
# === A forma dentro de um arquivo de malha — `.stl` e `.3mf` ===============
#
# O gerenciador lê os triângulos e os gira; estas são as linhas que o painel de
# infos põe ao lado da figura. São o que alguém prestes a IMPRIMIR a peça lê:
# que tamanho tem, quanto material leva e se a superfície é fechada — que é a
# primeira pergunta de um fatiador e aquela sobre a qual ele recusa.
files-fact-format = formato
files-fact-unit = unidade
files-fact-objects = objetos
files-fact-triangles = triângulos
files-fact-vertices = vértices
files-fact-bounds = tamanho
files-fact-volume = volume
files-fact-area = superfície
files-fact-watertight = superfície fechada
files-fact-flipped = faces invertidas
files-fact-degenerate = triângulos chatos
files-fact-application = feito com
files-fact-designer = designer
files-fact-licence = licença
files-fact-created = criado em
files-fact-schema = escrito conforme
files-fact-source-name = nome de origem
files-fact-solids = sólidos
files-fact-shells = cascas
files-fact-faces = faces
files-fact-edges = arestas
files-fact-outlined = faces sem preenchimento
files-value-stl-binary = STL, em bytes
files-value-stl-ascii = STL, em palavras
files-value-three-mf = 3MF
files-value-obj = OBJ
# O mesmo formato escrito de dois jeitos, e qual dos dois decide se o arquivo
# abre num editor de texto. O painel diz, pela mesma razão que diz do STL.
files-value-ply-ascii = PLY, em palavras
files-value-ply-binary = PLY, em bytes
files-value-off = OFF
files-value-gltf = glTF
files-value-glb = glTF, em um arquivo só
files-value-collada = COLLADA
files-value-amf = AMF
files-value-step = STEP
files-value-iges = IGES
# NÃO é medida. A unidade do glTF é o metro pela especificação dele, e o
# arquivo não diz nada — então o painel diz de onde veio o número.
files-value-unit-by-convention = metro, pelo formato
files-value-count = { $n }
# AS MEDIDAS SÃO SEMPRE EM MILÍMETROS. Um 3MF pode declarar polegadas ou
# metros e o leitor converte na entrada, então esta unidade não é um palpite.
files-value-box = { $w } × { $d } × { $h } mm
files-value-cubic-centimetres = { $n } cm³
files-value-square-centimetres = { $n } cm²
files-value-mesh-closed = sim
# NÃO "não". O que dá para agir é quantas arestas estão abertas, porque é o
# número que uma ferramenta de reparo relata e que muda enquanto ela trabalha.
files-value-mesh-open = não — { $n } arestas abertas
about-files-parent = Sai desta pasta para a que a contém.
about-files-enter = Abre o que está sob o cursor: entra na pasta, ou abre o arquivo no programa que o lê.
about-files-menu = Abre a lista de tudo o que se pode fazer com o que está sob o cursor, cada linha mostrando a tecla que a executa.
about-files-mark = Marca a linha sob o cursor, ou tira a marca dela — o que está marcado é sobre o que a próxima ação age.
about-files-mark-all = Marca de uma vez todas as entradas desta pasta.
about-files-unmark-all = Tira a marca de todas as entradas desta pasta.
about-files-header = Mostra ou esconde a linha que nomeia as colunas da listagem e carrega a seta que diz por qual delas ela está ordenada. Nasce escondida.
about-files-sort = Muda por qual campo a listagem é ordenada: o nome, o tamanho, a data ou o tipo.
about-files-sort-reverse = Inverte a ordem, do último para o primeiro.
about-files-measure = Soma o que uma pasta guarda em disco, percorrendo-a.
about-files-pane-next = Dá o teclado à próxima seção — a listagem, a barra lateral, a prévia e as informações, em anel.
about-files-pane-previous = Dá o teclado à seção anterior a esta.
about-files-eject = Desmonta o disco removível desta linha, para que ele possa ser retirado com segurança.
about-files-unpin = Tira este lugar da barra lateral. Um que a distro traz volta oculto; um seu, some.
about-files-preview-left = Desliza o texto da prévia para o lado, em direção ao começo das linhas longas.
about-files-preview-right = Desliza o texto da prévia para o lado, em direção ao fim das linhas longas.
about-files-grid-next = Vai para o próximo arquivo da prateleira. Com os itens lado a lado, as setas laterais andam por eles; o TAB continua atravessando para a próxima seção.
about-files-grid-previous = Vai para o arquivo anterior da prateleira. Com os itens lado a lado, as setas laterais andam por eles; o Shift TAB continua voltando uma seção.
about-files-grid = Mostra os arquivos lado a lado, como figuras, em vez de um por linha. O glifo no fim da linha do caminho faz o mesmo, e desenha para qual forma ele levaria. Nasce desligado.
about-files-grid-size = Gira entre os três tamanhos em que uma miniatura é desenhada. Numa tela que caberia mais figuras do que o gerenciador desenha de uma vez, o degrau menor é o que fica de fora.

## file info editing — everything the info panel can change

files-ask-modified = Modificado em (ano-mês-dia hora:minuto:segundo):
files-ask-tag = { $field }:
files-ask-meta = { $detail }:
files-trash-on-volume = Indo para a lixeira do próprio { $volume }, não para a da pasta pessoal: o espaço continua ocupado em { $volume } até essa lixeira o liberar — o que ela faz sozinha, como a da pasta pessoal, enquanto o disco está conectado.
files-trash-left-on-volume = { $volume } saiu com arquivos na própria lixeira ({ $n }): o espaço deles continua ocupado nele até essa lixeira os liberar, o que só acontece enquanto ele está conectado.
files-git-why-not-a-repo = Esta pasta não está num repositório.
# An archive's formats and levels, by `ronoctua_archive::Format::id` and `Level::id`.
files-archive-zip = zip
files-archive-7z = 7z
files-archive-tar = tar
files-archive-tar-gz = tar.gz
files-archive-tar-xz = tar.xz
files-archive-tar-zst = tar.zst
files-archive-tar-bz2 = tar.bz2
files-archive-tar-lz4 = tar.lz4
files-archive-rar = RAR
files-archive-iso = imagem ISO
files-archive-other = outro formato
files-archive-level-store = sem compressão
files-archive-level-fastest = o mais rápido
files-archive-level-normal = normal
files-archive-level-maximum = máximo
files-archive-level-ultra = ultra
# The sheet `compress` and `convert` open.
files-compress-title = Comprimir { $what }
files-convert-title = Converter { $name }
files-sheet-format = formato
files-sheet-level = nível
files-sheet-password = senha
files-sheet-password-none = nenhuma
files-sheet-password-set = definida, AES-256
files-sheet-hide = esconder também os nomes
files-sheet-split = dividir em volumes
files-sheet-split-none = não
files-sheet-yes = sim
files-sheet-no = não
files-sheet-go = comprimir em { $name }
files-sheet-convert-go = converter em { $name }
# Walking into an archive, and the passwords it asks.
files-archive-reading = lendo { $name }…
files-archive-opening = tirando { $name } para abrir…
files-archive-add-top = os membros entram no topo do arquivo — suba até ele para adicionar
files-archive-passwords-differ = as duas senhas são diferentes; nada foi definido
files-archive-nothing-chosen = nada escolhido para tirar
# The panels inside an archive: what its listing says of a member, and why
# nothing is previewed.
files-archive-fact-packed = compactado
files-archive-fact-encrypted = cifrado
files-archive-preview = um membro é mostrado quando é aberto, depois de sair do arquivo compactado
ask-files-archive-password = A senha do arquivo compactado
ask-files-archive-password-wrong = Não era essa — a senha do arquivo compactado
ask-files-archive-password-new = Uma senha para o novo arquivo compactado
ask-files-archive-password-again = A mesma senha de novo
# An encrypted disk (LUKS) in the sidebar.
files-encrypted-locked = { $name } é cifrado e está trancado — montar pede a frase-senha
files-encrypted-unlocked = { $name } está destrancado; ejetar tranca de novo antes de cortar a energia
files-encrypted-fact = cifra
files-encrypted-fact-locked = trancado — LUKS
files-encrypted-fact-unlocked = destrancado — LUKS
files-unlock-working = destrancando { $name }…
files-unlock-wrong = essa frase-senha não abre { $name }
files-unlock-failed = { $name } não pôde ser destrancado: { $why }
files-unlock-locked = { $name } foi trancado de novo
ask-files-unlock = A frase-senha que destranca este disco
files-private-locked = { $name } está trancado: abrir pede a senha, ou um toque na chave.
files-private-open = { $name } está aberto: trancar o fecha até a senha abri-lo de novo.
stick-sidebar-drive = { $name } — o drive inteiro: pressione para ver o que fazer com ele
