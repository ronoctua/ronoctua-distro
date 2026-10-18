# The texts `ronoctua-shell-inspect` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# WHAT THE PREVIEW SAYS WHEN THERE IS NO PICTURE. Not a refusal — those come
# from `ronoctua-inspect` and win when there is one — but the answer for a
# directory, a broken link, and a file nothing has been asked about. It used to
# be a large icon of the file's type, which said what the listing's own icon
# column already said and made an empty panel look full.
files-no-preview = sem prévia
# AND WHAT THE FACTS KEY SAYS WHEN THERE IS NOTHING TO SCROLL. From the
# outside, a key that refuses and a key that is dead look the same.
files-facts-all-shown = todos os fatos já estão à vista
files-preview-cut = só as primeiras { $lines } linhas estão aqui — o arquivo continua
# O TRANSPORTE DA PRÉVIA. O que o ponteiro parado sobre um controle diz, e o que
# o gerenciador responde quando uma tecla faz algo que não se vê.
files-play-hold = segurar onde está
files-play-resume = deixar seguir
files-sound-on = ouvir
files-sound-off = em silêncio
files-sound-none = este arquivo não tem som nenhum
files-entry-not-here = esta entrada está noutro lugar, e nada aqui a busca: { $path }
files-entry-at = { $path }
files-entry-titled = { $name } · { $path }
files-entry-refused = este nome não pode ser um caminho: { $path }
files-list-nothing = não há nesta lista nada que se possa tocar daqui
files-list-at-start = esta é a primeira da lista
files-list-at-end = esta é a última da lista
files-no-sound = esta máquina não tem som para tocá-lo — a imagem segue sem ele
files-film-stuck = nada saiu dele por { $seconds } s — foi parado
files-film-seek = ir para { $at } de { $total }
files-theatre-on = a prévia está com a região inteira
files-theatre-off = a listagem está com o espaço de volta
files-theatre-nothing = nada está selecionado, então não há o que expandir
files-wide-hidden = esse painel está escondido, então não há o que expandir
files-facts-wide-on = a ficha está com a região inteira, em quantas colunas ela couber
files-facts-wide-off = a ficha está de volta na coluna dela
files-yes = sim, aplicar
files-no = não, deixar como está
files-confirm-mode = Trocar as permissões de { $what } para { $mode }?
files-facts-scroll = página das informações
# AS TECLAS DA PRÓPRIA PRÉVIA, e a razão de estarem aqui em vez de saírem do
# `menu.rs` é a do `leader::OF_CLIPBOARD_VIEW`: chave que só existe dentro de um
# braço de `match` é chave que nada consegue enumerar. São estas que o
# `LEADER :` lista, então cada uma diz o que FAZ, não em que tecla está.
files-preview-scroll = página da prévia
# O transporte do filme. `files-play` é uma tecla para os dois porque é isso
# que ela é: apertar com o filme rodando é o que pausa.
files-play = tocar ou pausar
files-rewind = voltar
files-forward = avançar
files-sound = som
files-facts-wide = expandir a ficha
# As da peça. `files-turn-left` é a direção da MÃO e não um eixo — veja o
# `View::spin`, que é onde essa convenção está escrita.
files-turn-left = girar à esquerda
files-turn-right = girar à direita
files-turn-up = girar para cima
files-turn-down = girar para baixo
files-zoom-in = aproximar
files-zoom-out = afastar
files-reframe = reenquadrar
files-refused = { $why }
files-explain-group = o grupo é { $who }; clicar troca
files-explain-measure = clique para medir esta pasta — é uma caminhada
# THE NUMBER SAYS WHAT IT IS. Each of the ten letters explains itself, and the
# form most people actually type explained nothing at all — read off the screen:
# the pointer rested on it and the line stayed empty.
#
# One sentence for three digits and for four: naming the three audiences stays
# true when a leading digit arrives, and the special bit that brought it already
# has a sentence of its own on the letter beside it.
files-explain-octal = { $octal } é a mesma permissão escrita como número — um dígito para o dono, um para o grupo, um para todos os outros
files-octal-can = { $kind ->
        [dir] { $bits ->
                [0] não pode fazer nada com ele
                [1] pode entrar nele, mas não listá-lo
                [2] pode criar e apagar coisas nele, sem poder entrar nem listar
                [3] pode entrar nele e criar e apagar coisas nele
                [4] pode listar os nomes que há nele
                [5] pode listar os nomes que há nele e entrar
                [6] pode listar os nomes que há nele e criar e apagar coisas
               *[7] pode listar, entrar, criar e apagar
            }
       *[file] { $bits ->
                [0] não pode fazer nada com ele
                [1] pode executá-lo
                [2] pode alterá-lo
                [3] pode alterá-lo e executá-lo
                [4] pode lê-lo
                [5] pode lê-lo e executá-lo
                [6] pode lê-lo e alterá-lo
               *[7] pode lê-lo, alterá-lo e executá-lo
            }
    }
files-explain-octal-digit = { $part ->
        [special] { $digit } é o primeiro dígito de { $octal }: os bits especiais — setuid, setgid e o sticky —, cada um explicado na letra que o carrega.
        [owner] { $digit } é o dígito do dono em { $octal }: { $name } { $can }.
        [group] { $digit } é o dígito do grupo em { $octal }: o grupo { $name } { $can }.
       *[other] { $digit } é o último dígito de { $octal }: todos os outros { $can }.
    }
files-explain-when = { $when }, na hora local desta máquina
files-explain-when-utc = { $when }, em UTC — não consegui ler o /etc/localtime
# A ficha de detalhe.
files-detail-path = caminho
files-detail-type = tipo
files-detail-mode = permissão
files-detail-owner = dono:grupo
files-detail-size = tamanho
files-detail-modified = modificado
files-detail-links = nomes deste inode
files-detail-target = aponta para
files-detail-broken = aponta para (não existe)
# POR CIMA da figura, na última linha dela, e só quando alguma face não encheu.
# Peça meio contornada é peça sobre a qual alguém precisa ser avisado — e o
# painel é onde a pessoa está olhando.
files-preview-outlined = { $n ->
        [one] uma face em contorno
       *[other] { $n } faces em contorno
    }
# THE GLOBE AND WHAT IT ANSWERS — shared, and the prefix says so.
#
# These are not the file manager's words even though it is the first caller:
# the glance, the locale view and the installer all hold coordinates and could
# ask the same question. A sentence with an app's prefix is an app owning a
# word that is not its.
#
# WHICH LINES THERE ARE IS THE SELECTOR and the ORDER is the language's. A
# separator written into a value could not be counted and could not be given a
# register — see `chrome::Piece::separator`.
globe-address =
    { $has ->
        [street] { $street }, { $place } — { $region }, { $country }
        [place] { $place } — { $region }, { $country }
        [region] { $region }, { $country }
       *[country] { $country }
    }
globe-address-nowhere = nenhum lugar com nome perto desta coordenada
atlas-absent = os dados do mundo não estão nesta máquina
# THE ATTRIBUTION, ON SCREEN. GeoNames is CC BY 4.0 and the street packs are
# ODbL: both REQUIRE it, and this repository is MIT + OFL — so it is said in
# four places, of which this is the one a person actually reads. See
# `LICENSE-DATA.txt` for the other three.
atlas-credits = lugares do GeoNames (CC BY 4.0) · fronteiras do Natural Earth (domínio público) · ruas dos colaboradores do OpenStreetMap (ODbL)
atlas-heading = Dados do mundo
atlas-baseline = fronteiras e lugares
atlas-present = nesta máquina
atlas-offered = ruas, por região
atlas-not-yet = não está nesta máquina
atlas-not-yet-said = baixar as ruas de uma região ainda não está pronto — faltam o seletor, a fila e o leitor
atlas-open = abrir
atlas-previous = anterior
atlas-next = próximo
atlas-back = voltar
files-explain-globe = arraste para girar, a roda para aproximar
files-explain-globe-dot = onde este arquivo diz que foi feito — Enter enquadra
files-explain-globe-address = { $value } — aperte para abrir esta coordenada no mapa do navegador.
globe-north = mover o globo para o norte
globe-south = mover o globo para o sul
globe-east = mover o globo para o leste
globe-west = mover o globo para o oeste
files-fact-where = onde
# ---------------------------------------------------------------------------
# LEADER s l — language, clock, date and what the system decides.
# ---------------------------------------------------------------------------
language-en-gb = English
language-pt-br = Português
about-files-facts-scroll = Vira para a próxima página da coluna de informações, e volta ao topo ao chegar ao fim.
# THE PREVIEW: the text, the film and the shape.
about-files-preview-scroll = Vira para a próxima página da prévia, e volta ao topo ao chegar ao fim.
about-files-facts-wide = Dá à ficha a região inteira, e devolve a região. Expandida, ela é disposta em quantas colunas a largura couber, então cada valor aparece mais por inteiro.
about-files-play = Toca o que a prévia está mostrando, ou pausa onde está.
about-files-rewind = Volta um trecho no que está tocando.
about-files-forward = Avança um trecho no que está tocando.
about-files-sound = Dá som ao filme, ou tira. Um filme sem trilha não tem som para dar.
about-globe-north = Gira o globo para o norte, na direção do polo de cima.
about-globe-south = Gira o globo para o sul, na direção do polo de baixo.
about-globe-east = Gira o globo para leste, trazendo à vista a terra que está à direita.
about-globe-west = Gira o globo para oeste, trazendo à vista a terra que está à esquerda.
about-files-turn-left = Gira a forma para a esquerda, em torno do eixo vertical.
about-files-turn-right = Gira a forma para a direita, em torno do eixo vertical.
about-files-turn-up = Inclina a forma para longe de você, para vê-la por baixo.
about-files-turn-down = Inclina a forma em sua direção, para vê-la por cima.
about-files-zoom-in = Aproxima a forma.
about-files-zoom-out = Afasta a forma.
about-files-reframe = Devolve a forma ao centro, no tamanho em que começou.
ask-player-play-file = Tocar agora (um caminho ou um endereço)
player-trouble-tag-not-a-file = Isto não é um arquivo nesta máquina: as etiquetas dele só podem ser lidas.
player-trouble-tag-gone = O arquivo não está mais lá.
player-trouble-tag-changed-since = O arquivo mudou desde que foi lido; nada foi gravado. Ele é lido de novo: repita a mudança.
player-trouble-tag-hard-linked = O arquivo tem outros nomes (hard links), que uma cópia nova perderia.
player-trouble-tag-read-only-folder = A pasta dele não aceita escrita.
player-trouble-tag-read-only-file = O arquivo está marcado como somente leitura.
player-trouble-tag-not-yours = Outro usuário é dono deste arquivo.
player-trouble-tag-no-room = Não há espaço para a cópia: precisa de { $need }, há { $free }.
player-trouble-tag-unreadable = Não foi possível ler as etiquetas.
player-trouble-tag-unwritable = A gravação falhou; o arquivo ficou como estava.
player-trouble-tag-no-writer = Nada aqui grava as etiquetas deste tipo de arquivo ({ $container }).
player-trouble-tag-missing = { $program ->
        [mkvextract] O mkvextract não está instalado: arquivos Matroska só são lidos por inteiro e gravados com o perfil de mídia.
        [mkvmerge] O mkvmerge não está instalado: arquivos Matroska só são lidos por inteiro e gravados com o perfil de mídia.
        [mkvpropedit] O mkvpropedit não está instalado: arquivos Matroska só são lidos por inteiro e gravados com o perfil de mídia.
       *[other] O { $program } não está instalado.
    }
player-trouble-tag-program-failed = O { $program } falhou; o arquivo ficou como estava.
player-trouble-tag-timed-out = O { $program } demorou demais e foi parado.
player-trouble-tag-too-big = Grande demais para isto: { $bytes }, no máximo { $most }.
player-trouble-tag-not-an-image = Isso não é uma imagem que o player consiga pôr num arquivo.
player-trouble-tag-bad-number = Não é um número (1, ou 1/12).
player-trouble-tag-bad-date = Não é uma data (2024, 2024-05 ou 2024-05-31).
player-trouble-tag-bad-isrc = Não é um ISRC (duas letras, três letras ou dígitos, sete dígitos).
player-trouble-tag-bad-language = Não é um código de língua (pt, por, pt-BR).
player-trouble-tag-read-only-view = Lido para mostrar: muda pelo campo dele, ou pela etiqueta do próprio arquivo, mais acima.
player-trouble-tag-bad-name = Não é um nome que esta etiqueta aceite.
player-trouble-tag-not-text = O valor dela é dado, não texto: não é gravado aqui.
player-trouble-tag-not-global = Ela pertence a uma trilha, um capítulo ou um anexo, ou está aninhada com outras etiquetas: aqui só se gravam as etiquetas do arquivo inteiro, cada uma sozinha.
player-trouble-tag-no-such-thing = Essa imagem ou esse fluxo não está mais no arquivo.
player-trouble-tag-taken = Já existe uma capa ao lado do arquivo com os dois nomes que ela poderia ter.
player-trouble-tag-cancelled = Parado; o arquivo ficou como estava.
player-container-mpeg = áudio MPEG
player-container-aac = AAC
player-container-flac = FLAC
player-container-ogg-vorbis = Ogg Vorbis
player-container-opus = Opus
player-container-speex = Speex
player-container-ogg-other = Ogg
player-container-mp4 = MP4
player-container-wav = WAV
player-container-aiff = AIFF
player-container-ape = Monkey's Audio
player-container-mpc = Musepack
player-container-wavpack = WavPack
player-container-matroska = Matroska
player-container-webm = WebM
player-container-avi = AVI
player-container-mpeg-ts = MPEG-TS
player-container-flv = vídeo Flash
player-container-asf = ASF
player-container-other = outro contêiner
player-tag-title = Título
player-tag-artist = Artista
player-tag-album = Álbum
player-tag-album-artist = Artista do álbum
player-tag-track = Número da faixa
player-tag-disc = Número do disco
player-tag-date = Data
player-tag-genre = Gênero
player-tag-composer = Compositor
player-tag-lyricist = Letrista
player-tag-comment = Comentário
player-tag-lyrics = Texto da letra
player-tag-publisher = Gravadora
player-tag-isrc = ISRC
player-tag-description = Descrição
player-tag-show = Série
player-tag-season = Temporada
player-tag-episode = Episódio
player-tag-director = Direção

## file info editing — everything the info panel can change

files-detail-name = nome
files-fact-edits = { $label }: { $value } — pressione para mudar
files-fact-takes-place-out = { $label }: { $value } — pressione para tirar o lugar do arquivo
files-meta-title = título
files-meta-author = autoria
files-meta-description = descrição
files-meta-keywords = palavras-chave
files-meta-copyright = direitos autorais
files-meta-taken = feito em
files-fact-tags = etiquetas
files-fact-details = detalhes
files-tags-absent = o leitor de etiquetas não está instalado aqui
files-tags-stopped = o leitor de etiquetas parou antes de responder
files-tag-cannot = { $label } não pode ser mudado aqui: { $why }
files-tag-several-lines = { $label } tem várias linhas; ele é editado no editor do player, não numa linha só
files-modified-bad = Isso não é uma data que eu consiga ler. Escreva assim: 2026-09-30 14:05, os segundos são opcionais.
files-ask-location = { $n ->
        [one] Tirar o lugar de { $name }? Isso reescreve o arquivo e não se desfaz.
       *[other] Tirar o lugar destes { $n } arquivos? Isso reescreve cada um e não se desfaz.
    }
files-location-copy = escrever uma cópia sem o lugar, ao lado
files-location-overwrite = tirar do próprio arquivo — não se desfaz
files-location-copied = { $n ->
        [one] Uma cópia sem o lugar: { $name }
       *[other] { $n } cópias sem o lugar, cada uma ao lado do original
    }
files-location-removed = { $n ->
        [one] O lugar saiu de { $name }.
       *[other] O lugar saiu de { $n } arquivos.
    }
files-amend-too-long = Isso é mais longo do que um detalhe pode ser ({ $most } bytes); nada foi escrito.
files-amend-bad-date = Isso não é uma data: escreva assim: 2026-09-30 14:05. Nada foi escrito.
files-amend-not-written = O exiftool não escreveu nada: este arquivo não aceita isso aqui. Ele continua como estava.
files-amend-taken = Já existe um arquivo com o nome da cópia; nada foi escrito.
# Why a file's tags cannot be written, said precisely (`ronoctua_tags::Why`).
player-tag-format-id3v2 = Uma etiqueta ID3v2
player-tag-format-id3v1 = Uma etiqueta ID3v1
player-tag-format-ape = Uma etiqueta APEv2
player-tag-format-vorbis = Um comentário Vorbis
player-tag-format-mp4 = Uma etiqueta MP4
player-tag-format-riff-info = Uma lista RIFF INFO
player-tag-format-aiff-text = Um bloco de texto AIFF
player-trouble-tag-no-field-in = { $format } não tem lugar para { $field }.
player-trouble-tag-no-pictures-in = { $format } não guarda imagens: a capa deste arquivo não pode ser posta nem tirada.
player-trouble-tag-streams-only-matroska = O idioma, o nome e as marcas de uma faixa só são gravados em Matroska, e este arquivo é { $container }.
player-trouble-tag-attachment = Um anexo — uma imagem, uma fonte — fica guardado no arquivo e não toca: não tem idioma nem nome de faixa para mudar.
player-trouble-tag-read-only = { $format }: aqui só se lê, e nunca se muda à parte.
player-trouble-tag-only-the-front-cover = Aqui só a capa da frente é trocada ou tirada; as outras imagens do arquivo ficam como estão.
player-trouble-tag-chapters-only-matroska = O título de um capítulo só é gravado em Matroska, e este arquivo é { $container }.
player-trouble-tag-remux-would-drop = Este arquivo é grande demais para ser gravado no lugar, e a cópia que o grava perderia itens que ele guarda além destes campos. Os campos ficam como estão.
