# The texts `ronoctua-shell-video-metrics` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# THE VIDEO CATALOGUE (LEADER v, video_metrics.rs and its window).
group-video-metrics = vídeos
video-metrics-title = Métricas de vídeo
cmd-video-metrics-open = Métricas de vídeo: seus canais do YouTube, os vídeos pontuados
cmd-video-metrics-window = Métricas de vídeo, numa janela
cmd-video-metrics-scan = Varrer agora todos os canais atrás de vídeos novos
cmd-video-metrics-up = Linha anterior
cmd-video-metrics-down = Próxima linha
cmd-video-metrics-less = O valor sob o cursor um passo abaixo, ou o vídeo anterior na grade
cmd-video-metrics-more = O valor sob o cursor um passo acima, ou o vídeo seguinte na grade
cmd-video-metrics-enter = Abrir ou mudar o que está sob o cursor
cmd-video-metrics-page-back = A página de vídeos anterior
cmd-video-metrics-page-on = A página de vídeos seguinte
cmd-video-metrics-layout = Os vídeos em lista, ou numa grade de miniaturas
cmd-video-metrics-search = Buscar nas palavras dos vídeos — títulos, descrições, etiquetas e as suas
cmd-video-metrics-player = Abrir no player
cmd-video-metrics-kiosk = Abrir no Chrome, como quiosque
cmd-video-metrics-download = Baixar, pela fila de transferências
cmd-video-metrics-hide = Ocultar das listas, ou mostrar de novo
cmd-video-metrics-delete = Apagar de vez: ele nunca volta
cmd-video-metrics-add-channel = Acrescentar um canal do YouTube, pelo endereço ou pelo @handle
cmd-video-metrics-channel-score = Digitar a pontuação de um canal
cmd-video-metrics-points = Digitar os pontos de uma regra
cmd-video-metrics-days = Digitar em quantos dias a novidade se apaga
cmd-video-metrics-band = Digitar uma faixa de duração
cmd-video-metrics-words = Digitar as palavras que uma regra procura
cmd-video-metrics-second-tags = Digitar as suas etiquetas de um vídeo
cmd-video-metrics-weight = Digitar o peso de uma nota
video-metrics-up = acima
video-metrics-down = abaixo
video-metrics-less = menos
video-metrics-more = mais
video-metrics-enter = abrir
video-metrics-page-back = página anterior
video-metrics-page-on = próxima página
video-metrics-layout = lista ou grade
video-metrics-search = buscar
video-metrics-player = player
video-metrics-kiosk = Chrome
video-metrics-download = baixar
video-metrics-hide = ocultar
video-metrics-delete = apagar
ask-video-metrics-add-channel = Endereço ou @handle do canal
ask-video-metrics-channel-score = Pontuação, de 0 a 10 000
ask-video-metrics-points = Pontos
ask-video-metrics-days = Apaga-se em quantos dias
ask-video-metrics-band = De quanto a quanto (4:00-20:00)
ask-video-metrics-words = Palavras a procurar
ask-video-metrics-search = Buscar nos vídeos
ask-video-metrics-second-tags = Suas etiquetas, separadas por vírgulas
ask-video-metrics-weight = Pontos para esta nota
video-metrics-heading-scan = Varredura
video-metrics-last-scan = Última varredura
video-metrics-never = nunca
video-metrics-scan-running = varrendo desde { $when }
video-metrics-scan-cut = parou antes de terminar ({ $when })
video-metrics-scan-added = Vídeos novos
video-metrics-scan-failed = Canais que falharam
video-metrics-failed-of = { $failed } de { $channels }
video-metrics-pending = { $pending ->
        [1] Um vídeo espera os detalhes: a próxima varredura pergunta por ele.
       *[other] { $pending } vídeos esperam os detalhes: as próximas varreduras perguntam por eles, poucos de cada vez.
    }
video-metrics-scan-now = Varrer agora
video-metrics-ask-youtube = Perguntar ao YouTube
about-video-metrics-ask-youtube = Se uma varredura pode perguntar ao YouTube: desligado, nada é perguntado até você ligar de novo.
video-metrics-ask-youtube-note = Acrescentar um canal deixa { $hosts } serem perguntados — feeds e miniaturas, nada mais. A permissão é desta máquina, endereço por endereço: desligada aqui, nada nela pergunta a eles, nem as fontes do YouTube do player.
video-metrics-on = ligado
video-metrics-off = desligado
video-metrics-heading-catalogue = O catálogo
video-metrics-videos = Vídeos
video-metrics-channels = Canais
video-metrics-rules = Regras
video-metrics-weights = Pesos das notas
video-metrics-heading-left-out = Deixados de fora
video-metrics-skip-before-reference = Mais antigos que a última varredura do canal
video-metrics-skip-first-scan = Além da primeira varredura de um canal
video-metrics-skip-blocked-short = Shorts, enquanto estão bloqueados
video-metrics-skip-unavailable = Privados, removidos ou sumidos
video-metrics-skip-deleted = Apagados por você
video-metrics-reading = Lendo o catálogo…
video-metrics-why-not-yet = Isto ainda não tem resposta.
video-metrics-why-no-database = Esta compilação não tem catálogo de vídeos.
video-metrics-why-not-there = Nada foi catalogado ainda: acrescente um canal, e a próxima varredura preenche.
video-metrics-why-unreadable = O catálogo de vídeos está lá e não pode ser lido.
video-metrics-why-other-schema = O catálogo de vídeos foi escrito por uma versão mais nova; ele fica como está.
video-metrics-why-busy = Uma varredura está segurando o catálogo agora; tente de novo daqui a pouco.
video-metrics-why-not-found = Isto não está mais no catálogo.
video-metrics-why-failed = O catálogo não pôde ser escrito.
video-metrics-why-invalid = O catálogo não pode guardar isso: uma regra precisa de palavras ou de uma faixa que termine depois de começar, e a novidade precisa de pelo menos um dia e de pontos não negativos.
video-metrics-stop-bot-check = O YouTube perguntou se isto é um robô, e a varredura parou; a próxima tenta de novo.
video-metrics-stop-too-many-requests = O YouTube disse que foram pedidos demais, e a varredura parou; a próxima tenta de novo.
video-metrics-stop-offline = Não havia rede, e a varredura parou.
video-metrics-stop-not-let = Perguntar ao YouTube está desligado, e a varredura não perguntou nada.
video-metrics-add-a-channel = Acrescentar um canal…
video-metrics-no-channels = Nenhum canal ainda: acrescente um pelo endereço ou pelo @handle.
video-metrics-channel-row = { $score } · { $videos ->
        [1] um vídeo
       *[other] { $videos } vídeos
    } · varrido { $when }
video-metrics-channel-added = { $name } foi acrescentado: os vídeos mais novos chegam na próxima varredura.
video-metrics-channel-removed = O canal e os vídeos dele foram removidos.
video-metrics-add-absent = Esta instalação não tem yt-dlp, que diz de quem é o canal de um endereço.
video-metrics-add-unstartable = O yt-dlp não pôde ser iniciado.
video-metrics-add-slow = O yt-dlp demorou demais para dizer de quem é o canal.
video-metrics-add-failed = O yt-dlp não conseguiu dizer de quem é o canal.
video-metrics-add-garbled = O yt-dlp respondeu algo que não nomeia canal nenhum.
video-metrics-add-not-an-address = Isso não é um endereço do YouTube, um @handle nem uma id de canal.
video-metrics-add-gone = O YouTube não tem canal ali.
video-metrics-not-let-no-home = O canal entrou, e não há uma pasta pessoal onde guardar o consentimento para perguntar ao YouTube: uma varredura não vai perguntar.
video-metrics-not-let-unparseable = O canal entrou, e o arquivo de consentimento não é legível, então ficou como está: uma varredura não vai perguntar ao YouTube.
video-metrics-not-let-unwritable = O canal entrou, e o consentimento para perguntar ao YouTube não pôde ser gravado: uma varredura não vai perguntar.
video-metrics-address = Endereço
video-metrics-handle = Handle
video-metrics-added = Acrescentado
video-metrics-reference = Varrido até
video-metrics-reference-none = ainda não
video-metrics-last-error = Último erro
video-metrics-its-videos = Os vídeos dele
video-metrics-score = Pontuação
about-video-metrics-score = A pontuação de um canal, de 0 a 10 000, entra em todo vídeo dele: as setas movem de cem em cem, e o RET digita.
video-metrics-its-shorts = Os Shorts dele
video-metrics-shorts-follow = como em todo lugar
video-metrics-shorts-blocked = bloqueados
video-metrics-shorts-allowed = permitidos
video-metrics-its-rules = As regras dele
video-metrics-heading-remove = Remover
video-metrics-remove-channel = Remover este canal
ask-video-metrics-remove-channel = Remover { $name }? Os vídeos dele vão junto, e nenhum volta.
video-metrics-keep-channel = Não, manter o canal
video-metrics-remove-channel-it = Sim, remover o canal e os vídeos
video-metrics-block-shorts = Bloquear os Shorts do YouTube
video-metrics-block-shorts-note = Shorts bloqueados nunca são catalogados, e os que já estão ficam ocultos; um canal pode escolher outra coisa na página dele.
video-metrics-heading-score = Como um vídeo é pontuado
video-metrics-rule-term = Palavras a procurar
video-metrics-rule-band = Faixa de duração
video-metrics-rule-preferred = Duração preferida
video-metrics-rule-short = Penalidade de Short
video-metrics-rule-recency = Novidade
video-metrics-rule-live = Ao vivo ou estreia
video-metrics-none = não há
video-metrics-points-value = { $points ->
        [1] { $points } ponto
       *[other] { $points } pontos
    }
video-metrics-days-value = { $days ->
        [1] um dia
       *[other] { $days } dias
    }
video-metrics-recency-value = { $points }, somem em { $days }
video-metrics-rule-in = { $value } · { $scope }
video-metrics-scope-own = deste canal
video-metrics-scope-everywhere = em todo lugar
video-metrics-band-value = { $from } a { $to }
video-metrics-band-joiner = a
video-metrics-band-row = Duração de { $band }
video-metrics-preferred-row = Duração preferida: { $band }
video-metrics-term-row = “{ $words }”
video-metrics-add-band = Acrescentar uma faixa de duração…
video-metrics-add-band-here = Acrescentar uma faixa de duração para este canal…
video-metrics-bands-note = Estas faixas são as de todo lugar; uma faixa acrescentada aqui começa a lista própria deste canal, e as de todo lugar deixam de contar para ele.
video-metrics-add-term = Acrescentar palavras…
video-metrics-add-term-here = Acrescentar palavras para este canal…
video-metrics-rule-inherited-note = Esta é a regra de todo lugar; mudar aqui cria uma só para { $name }.
video-metrics-rule-own-note = Esta é a regra própria de { $name }: ela conta no lugar da de todo lugar.
video-metrics-rule-own-off-note = Esta é a regra própria de { $name }, desligada: nada deste tipo conta para { $name }, nem a regra de todo lugar. Remover a regra traz a de todo lugar de volta.
video-metrics-recency-points = Pontos hoje
video-metrics-recency-days = Apaga-se em
video-metrics-rule-points = Pontos
about-video-metrics-points = Os pontos que ela soma — negativos subtraem: as setas movem de cem em cem, e o RET digita.
video-metrics-rule-length = Duração
video-metrics-term-words = Palavras
video-metrics-term-match = Casamento
video-metrics-match-word = palavras inteiras
video-metrics-match-phrase = as palavras em ordem
video-metrics-match-prefix = palavras que começam assim
video-metrics-field-title = No título
video-metrics-field-description = Na descrição
video-metrics-field-tags = Nas etiquetas
video-metrics-field-second-description = Na sua descrição
video-metrics-field-second-tags = Nas suas etiquetas
video-metrics-rule-on = Em vigor
video-metrics-remove-rule = Remover esta regra
ask-video-metrics-remove-rule = Remover esta regra? Ela não pode ser trazida de volta.
video-metrics-keep-rule = Não, manter a regra
video-metrics-remove-rule-it = Sim, remover a regra
video-metrics-weights-note = O que uma nota sua soma à pontuação de um vídeo, em todos os canais.
video-metrics-rating-very-negative = Muito negativa
video-metrics-rating-negative = Negativa
video-metrics-rating-positive = Positiva
video-metrics-rating-very-positive = Muito positiva
video-metrics-channel = Canal
video-metrics-every-channel = todos os canais
video-metrics-filter-shorts = Shorts
video-metrics-shorts-any = com e sem
video-metrics-shorts-only = só Shorts
video-metrics-shorts-without = sem Shorts
video-metrics-rating = Sua nota
video-metrics-rating-any = qualquer
video-metrics-rating-none = sem nota
video-metrics-filter-hidden = Ocultos
video-metrics-hidden-without = não aparecem
video-metrics-hidden-only = só os ocultos
video-metrics-hidden-any = aparecem também
video-metrics-filter-text = Busca
video-metrics-search-none = nada
video-metrics-published = Publicado
video-metrics-since-any = a qualquer hora
video-metrics-since-day = no último dia
video-metrics-since-week = na última semana
video-metrics-since-month = nos últimos 30 dias
video-metrics-since-year = no último ano
video-metrics-filter-layout = Disposição
video-metrics-layout-list = lista
video-metrics-layout-grid = miniaturas
video-metrics-page = Página
video-metrics-page-of = { $page } de { $pages } · { $videos ->
        [1] um vídeo
       *[other] { $videos } vídeos
    }
video-metrics-by-score = Por pontuação
video-metrics-no-videos = Nenhum vídeo corresponde.
video-metrics-video-row = { $score } · { $channel } · { $age }
video-metrics-age = { $days ->
        [0] hoje
        [1] ontem
       *[other] há { $days } dias
    }
video-metrics-the-video = O vídeo
video-metrics-duration = Duração do vídeo
video-metrics-kind = Tipo
video-metrics-kind-short = um Short
video-metrics-kind-long = um vídeo
video-metrics-kind-live = ao vivo ou uma estreia
video-metrics-its-points = Os pontos dele
video-metrics-size = Imagem
video-metrics-size-value = { $width } × { $height }
video-metrics-languages = Línguas
video-metrics-age-limit = Idade mínima
video-metrics-captions = Legendas
video-metrics-categories = Categorias
video-metrics-chapters = Capítulos
video-metrics-views = Visualizações
video-metrics-first-seen = Visto pela primeira vez
video-metrics-tags = Etiquetas do YouTube
video-metrics-description = Descrição do YouTube
about-video-metrics-copy = O RET põe este valor na área de transferência.
video-metrics-yours = O que é seu
video-metrics-your-tags = Suas etiquetas
video-metrics-your-description = Sua descrição
about-video-metrics-second-description = Uma descrição sua, editada aqui: C-s guarda, e uma regra pode procurar as palavras dela.
video-metrics-description-kept = Sua descrição foi guardada.
video-metrics-with-it = Com ele
video-metrics-hide-row = Nas listas
video-metrics-shown = aparece
video-metrics-hidden = oculto
ask-video-metrics-hide = Ocultar { $title }? Ele continua no catálogo, e "Ocultos: aparecem também" encontra.
video-metrics-leave-shown = Não, deixar aparecendo
video-metrics-hide-it = Sim, ocultar
ask-video-metrics-delete = Apagar { $title } de vez? Ele nunca volta, nem na próxima varredura.
video-metrics-keep-video = Não, manter o vídeo
video-metrics-delete-it = Sim, apagar de vez
video-metrics-deleted = O vídeo foi apagado, e nunca volta.
ask-video-metrics-again = Ele já está na fila como { $name }. Acrescentar de novo?
video-metrics-not-again = Não, uma vez basta
video-metrics-again = Sim, acrescentar de novo
video-metrics-player-unreachable = O player não respondeu; nada foi acrescentado.
video-metrics-downloading = Baixando { $title }: a fila mostra ({ $keys }).
video-metrics-scan-asked = Uma varredura foi pedida; a modeline marca enquanto ela roda.
video-metrics-scan-started = O cérebro não a assumiu, então a varredura começou direto.
video-metrics-scan-nowhere = Nada aqui pode começar uma varredura: nem o cérebro nem o systemd responderam.
video-metrics-scan-saver = A economia de energia está ligada, e nenhuma varredura começa enquanto ela estiver: nada foi pedido.
video-metrics-scan-other-instance = Esta é uma segunda cópia do sistema: o cérebro dela só ensaia, e a unidade da varredura é a da sessão instalada, então nada foi pedido aqui.
video-metrics-typed-not-a-number = Isso não é um número inteiro.
video-metrics-typed-not-days = Isso não é um número inteiro de dias, um ou mais.
video-metrics-typed-not-a-band = Escreva uma faixa como duas durações, como 4-20 ou 0:30-3:00.
video-metrics-typed-no-words = Não há palavras nisso para procurar.
video-metrics-typed-no-row = Não há linha aqui para isso: ponha o cursor no que isso muda e digite lá.
player-ranking-reading = Lendo o catálogo de vídeos…
player-ranking-none = Nada pontuado ainda: o catálogo de vídeos ({ $keys }) adiciona os seus canais, e as varreduras dele enchem esta lista.
player-ranking-empty = Nenhum vídeo pontuado ainda: as varreduras não acharam nenhum, ou todos estão ocultos.
player-ranking-layout-grid = Miniaturas
player-ranking-added = Adicionado: { $title }
player-ranking-title = Título do vídeo
player-ranking-channel = Canal
player-ranking-score = Pontuação
player-ranking-length = Tempo de vídeo
player-ranking-published = Publicado
player-ranking-actions = Com este vídeo
ask-player-ranking-delete = Apagar “{ $title }” do catálogo de vídeos para sempre? Ele não volta, nem numa varredura.
player-ranking-hidden = Oculto: { $title }. Os vídeos ocultos do catálogo de vídeos o trazem de volta.
player-ranking-deleted = Apagado para sempre: { $title }
player-ranking-not-changed = { $title } não mudou: { $why }
player-ranking-downloading = Baixando { $title }; { $keys } mostra as transferências.
player-ranking-downloading-plain = Baixando { $title }.
player-ranking-not-downloaded = A fila de transferências não aceitou { $title }: ela não está rodando.
