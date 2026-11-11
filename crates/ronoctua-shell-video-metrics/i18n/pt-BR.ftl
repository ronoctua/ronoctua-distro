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
cmd-video-metrics-page-first = A primeira página de vídeos
cmd-video-metrics-page-last = A última página de vídeos
cmd-video-metrics-layout = Dispor os vídeos do próximo jeito: em lista, em lista com miniaturas, em miniaturas, em miniaturas com o canal, em miniaturas largas
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
video-metrics-page-first = primeira página
video-metrics-page-last = última página
video-metrics-layout = próxima disposição
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
video-metrics-skip-expired = Passaram do prazo no catálogo
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
# THE SAME WHY, ON ITS OWN: said after what registering a list did, which
# already says how many channels went in.
video-metrics-unkept-no-home = Não há uma pasta pessoal onde guardar o consentimento para perguntar ao YouTube: uma varredura não vai perguntar.
video-metrics-unkept-unparseable = O arquivo de consentimento não é legível, então ficou como está: uma varredura não vai perguntar ao YouTube.
video-metrics-unkept-unwritable = O consentimento para perguntar ao YouTube não pôde ser gravado: uma varredura não vai perguntar.
video-metrics-address = Endereço
video-metrics-handle = Handle
video-metrics-added = Acrescentado
video-metrics-reference = Varrido até
video-metrics-reference-none = ainda não
video-metrics-last-error = Último erro
video-metrics-its-videos = Os vídeos dele
video-metrics-score = Pontuação
about-video-metrics-score = A pontuação de um canal, de 0 a 10 000, entra em todo vídeo dele: cada passo a move de cem em cem, ou ela é digitada inteira.
video-metrics-its-shorts = Os Shorts dele
video-metrics-as-everywhere = como em todo lugar
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
# A SCORE AS A BARE NUMBER — a list's, before the arrows of the rating — a
# NUMBER to Fluent all the same, as `points` is.
video-metrics-score-number = { $score }
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
about-video-metrics-points = Os pontos que ela soma — negativos subtraem: cada passo os move de cem em cem, ou eles são digitados inteiros.
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
video-metrics-layout-list-thumbnails = lista com miniaturas
video-metrics-layout-grid-channel = miniaturas com canal
video-metrics-layout-wide = miniaturas largas
video-metrics-page = Página
video-metrics-per-page = Por página
video-metrics-page-of = { $page } de { $pages } · { $videos ->
        [1] um vídeo
       *[other] { $videos } vídeos
    }
video-metrics-by-score = Por pontuação
video-metrics-no-videos = Nenhum vídeo corresponde.
video-metrics-video-row = { $channel } · { $age } · { $score }
video-metrics-video-row-short = { $age } · { $score }
video-metrics-video-row-undated = { $channel } · { $score }
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
about-video-metrics-copy = Um fato que o catálogo guarda; copiado, fica na área de transferência, para colar onde quiser.
video-metrics-yours = O que é seu
video-metrics-your-tags = Suas etiquetas
video-metrics-your-description = Sua descrição
about-video-metrics-second-description = Uma descrição sua, editada aqui e guardada quando você manda; uma regra pode procurar as palavras dela.
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
# WHAT EVERY ROW IS, under the pointer and in the window's foot (the owner,
# 2026-11-09): a sentence for each thing a click takes (`about_of`, which a
# new row cannot compile without), and what the hand does to it — the door
# says the key or the click, these say what follows (`does_of`). No sentence
# spells a key: the gesture beside the name does.
video-metrics-act-opens = abre
video-metrics-act-turns = passa à próxima opção
video-metrics-act-switches = alterna
video-metrics-act-types = pede para digitar
video-metrics-act-copies = copia
video-metrics-act-does = faz
video-metrics-act-edits = edita aqui
video-metrics-act-asks = pergunta antes, com o “não” primeiro
video-metrics-act-answers = dá esta resposta
about-video-metrics-last-scan = Quando a última varredura terminou — ou que uma roda agora, ou parou antes do fim.
about-video-metrics-scan-added = Quantos vídeos novos a última varredura trouxe.
about-video-metrics-scan-failed = Quantos canais a última varredura não leu; a página de cada um diz por quê.
about-video-metrics-videos = Todos os vídeos catalogados, por pontuação, com filtros, em lista — com miniaturas ou sem — ou em miniaturas, com o canal ou sem, ou largas.
about-video-metrics-channels = Os canais que você segue, e acrescentar um.
about-video-metrics-rules = Como cada vídeo é pontuado, quanto tempo os vídeos ficam, e se os Shorts são bloqueados.
about-video-metrics-weights = O que cada nota sua soma à pontuação de um vídeo.
about-video-metrics-left-out = Vídeos que as varreduras acharam e não guardaram, ou tiraram quando o prazo acabou, por motivo; nenhum deles aparece.
about-video-metrics-channel-row = A página de um canal: os dados dele, a pontuação, os Shorts, quanto tempo os vídeos dele ficam, as regras próprias e os vídeos.
about-video-metrics-its-videos = A lista de vídeos, só os deste canal.
about-video-metrics-channel-shorts = Se os Shorts deste canal entram no catálogo: como em todo lugar, bloqueados ou permitidos.
about-video-metrics-remove-channel = Remove o canal e todos os vídeos dele, depois de perguntar.
about-video-metrics-reference = Onde as varreduras estão neste canal: um vídeo publicado mais de um dia antes disso não entra.
about-video-metrics-rule-recency = Pontos que um vídeo novo ganha, sumindo em alguns dias.
about-video-metrics-rule-short = Pontos que um Short ganha — em geral, negativos.
about-video-metrics-rule-preferred = Pontos que um vídeo ganha quando a duração está na faixa que você prefere.
about-video-metrics-rule-live = Pontos que uma transmissão ao vivo ou estreia ganha.
about-video-metrics-rule-band = Pontos que um vídeo ganha com a duração nesta faixa.
about-video-metrics-rule-term = Pontos que um vídeo ganha quando tem estas palavras.
about-video-metrics-add-band = Uma faixa de duração nova; um vídeo nela ganha os pontos dela.
about-video-metrics-add-term = Palavras novas a procurar; um vídeo que as tem ganha os pontos delas.
about-video-metrics-block-shorts = Se os Shorts entram no catálogo; um canal pode escolher outra coisa.
about-video-metrics-recency-days = Em quantos dias os pontos de um vídeo novo somem.
about-video-metrics-rule-length = As durações desta regra.
about-video-metrics-term-words = As palavras que esta regra procura.
about-video-metrics-term-match = Como as palavras são achadas: inteiras, em ordem, ou pelo começo.
about-video-metrics-term-field = Se as palavras são procuradas nesta parte do vídeo.
about-video-metrics-rule-on = Se esta regra conta; desligada, não soma nada.
about-video-metrics-remove-rule = Remove esta regra, depois de perguntar.
about-video-metrics-filter-channel = Os vídeos de todos os canais, ou de um.
about-video-metrics-filter-shorts = Shorts e o resto, só Shorts, ou sem Shorts.
about-video-metrics-filter-rating = Qualquer nota sua, ou só uma.
about-video-metrics-filter-hidden = Os ocultos fora, junto, ou só eles.
about-video-metrics-filter-since = Até quando, pela data de publicação.
about-video-metrics-layouts = Como os vídeos são dispostos: em lista, em lista com miniaturas, em miniaturas, em miniaturas com o canal ou em miniaturas largas, com o título quase inteiro embaixo.
# THE KEYS FIRST, and few words: the footer says it on one line, cut at its end.
about-video-metrics-page = { $earlier } { $prior } voltam, { $later } { $next } avançam, { $first } { $last } vão à primeira e à última página.
about-video-metrics-per-page = Quantos vídeos uma página mostra: cada volta dá o próximo tamanho, e recomeça. Vale enquanto a vista está aberta.
about-video-metrics-video-entry = A página do vídeo: todos os campos, sua nota, etiquetas e descrição, e o que dá para fazer.
about-video-metrics-published = Quando o YouTube diz que foi publicado.
about-video-metrics-its-points = Os pontos dele, sem contar a pontuação do canal e a novidade.
about-video-metrics-rating = Sua nota deste vídeo: o peso dela entra na pontuação.
about-video-metrics-second-tags = Etiquetas suas; uma regra pode procurá-las.
about-video-metrics-player-row = Põe na fila do player; se já estiver lá, pergunta antes.
about-video-metrics-kiosk-row = A página dele no YouTube, numa janela do Chrome sem nada em volta.
about-video-metrics-download-row = Entrega à fila de transferências, para seus downloads.
about-video-metrics-hide-row = Oculta das listas depois de perguntar, ou mostra de novo na hora.
about-video-metrics-delete-row = Apaga de vez depois de perguntar: nenhuma varredura o traz de volta.
about-video-metrics-answer-no = Deixa tudo como está.
about-video-metrics-answer-yes = Faz o que a pergunta diz.
about-video-metrics-text-line = Sua descrição enquanto é escrita; um clique põe o cursor ali.
# WHAT "AS EVERYWHERE" IS NOW, quietly beside the value, and said where the
# row is explained.
video-metrics-default-now = ({ $value })
video-metrics-follows-now = Em todo lugar, agora é: { $value }.
video-metrics-follows-rule-everywhere = Esta é a regra de todo lugar: a página dela a muda em todos os canais.
# HOW LONG A VIDEO STAYS, WHAT IS SET ASIDE, AND THE GLYPHS of a video in
# the lists (the owner, 2026-11-09): a video goes when its time is up — the
# catalogue's, or its channel's own — unless it is set aside, or marked never
# to be deleted; the four arrows of its rating after its score, and its marks
# at the line's end, each acting on the first click. The forms and the hint
# words of the keys `s` and `f` are the view's (`OF_THE_VIEW`).
cmd-video-metrics-set-aside = Separar para depois, ou tirar dos vídeos separados
cmd-video-metrics-keep = Nunca apagar, ou deixar sair quando o prazo acabar
cmd-video-metrics-keep-days = Digitar depois de quantos dias os vídeos são apagados
video-metrics-set-aside = separar
video-metrics-keep = manter
ask-video-metrics-keep-days = Apagados depois de quantos dias
video-metrics-set-aside-videos = Vídeos separados
video-metrics-filter-set-aside = Separados
video-metrics-set-aside-any = separados ou não
video-metrics-set-aside-only = só eles, os mais recentes primeiro
video-metrics-set-aside-without = sem os separados
video-metrics-by-set-aside = Separados mais recentes primeiro
video-metrics-heading-keep = Quanto tempo os vídeos ficam
video-metrics-keep-after = Apagados depois de
video-metrics-keep-never = nunca apagados
video-metrics-channel-keep = Os vídeos dele são apagados depois de
video-metrics-set-aside-row = Separado para depois
video-metrics-never-delete = Nunca apagar
video-metrics-goes-on = sai em { $date }
video-metrics-goes-unknown = sai quando o prazo acabar
video-metrics-kept-by-channel = o canal mantém todos os vídeos
video-metrics-kept-set-aside = fica enquanto estiver separado
video-metrics-kept-everywhere = fica para sempre, como em todo lugar
video-metrics-kept-for-ever = mantido para sempre
video-metrics-without-the-mark = Sem a marca: { $value }.
video-metrics-once-out = Fora dos separados: { $value }.
# WHAT THE NEXT SCAN TAKES OUT, under a keep — quietly, so a small number
# typed by mistake surprises nobody. Said only when it is one or more.
video-metrics-expiring = { $count ->
        [one] Um vídeo sai do catálogo na próxima varredura: o prazo dele acabou.
       *[other] { $count } vídeos saem do catálogo na próxima varredura: o prazo deles acabou.
    }
video-metrics-typed-not-keep-days = Isso não é um número de dias de um a { $most }; além disso, o anel tem “{ $never }”.
video-metrics-act-picks = dá esta nota
video-metrics-act-unpicks = tira a nota
about-video-metrics-set-aside-videos = Os vídeos que você separou, os mais recentes primeiro; separado, um vídeo nunca é apagado.
about-video-metrics-filter-set-aside = Os separados e o resto, só eles (os mais recentes primeiro), ou nenhum deles.
about-video-metrics-keep-everywhere = Quanto tempo um vídeo fica, contado da publicação, antes de ser apagado — ou para sempre. Os separados, e os marcados para nunca serem apagados, ficam.
about-video-metrics-channel-keep = Quanto tempo os vídeos deste canal ficam antes de serem apagados: como em todo lugar, dias próprios, ou para sempre.
about-video-metrics-set-aside-row = Se está entre os vídeos separados; enquanto estiver, nunca é apagado.
about-video-metrics-never-delete = Se fica para sempre, digam o que disserem o canal e o catálogo.
about-video-metrics-glyph-rate = Dá esta nota ao vídeo: o peso dela entra na pontuação.
about-video-metrics-glyph-rated = A nota do vídeo agora; tirada, o peso dela sai da pontuação.
about-video-metrics-glyph-set-aside = Separa: entra nos vídeos separados, e nunca é apagado enquanto estiver lá.
about-video-metrics-glyph-take-out = Separado: nunca é apagado enquanto estiver; desligado, sai dos vídeos separados.
about-video-metrics-glyph-forever-no = Sai quando o prazo acabar; marcado, fica para sempre.
about-video-metrics-glyph-forever-video = Fica para sempre pela sua marca, digam o que disserem o canal e o catálogo.
about-video-metrics-glyph-forever-channel = Fica para sempre, como todo vídeo do canal; sua marca o mantém se o canal mudar.
about-video-metrics-glyph-forever-everywhere = Fica para sempre, como os vídeos em todo lugar; sua marca o mantém se isso mudar.
about-video-metrics-glyph-forever-aside = Fica enquanto estiver separado; marcado, fica para sempre, separado ou não.
# THE SAME CATALOGUE IN OTHER APPS (the owner, 2026-11-09): the brain's page
# on its scan, the player's two lists of it, and a video — or a channel's
# videos — in Nyxt. The hint word and the form of the key `n` are the view's
# (`OF_THE_VIEW`).
cmd-video-metrics-nyxt = Abrir no Nyxt
video-metrics-nyxt = Nyxt
video-metrics-heading-elsewhere = Em outros apps
video-metrics-in-the-brain = A agenda da varredura, no brain
video-metrics-in-the-player = Vídeos por pontuação, no player
video-metrics-set-aside-in-the-player = Vídeos separados, no player
video-metrics-channel-in-chrome = Os vídeos dele no Chrome
video-metrics-channel-in-nyxt = Os vídeos dele no Nyxt
about-video-metrics-in-the-brain = A página da varredura deste catálogo no brain: quando roda de novo, o que espera, o que fez.
about-video-metrics-in-the-player = Uma janela nova do player, na lista destes vídeos por pontuação.
about-video-metrics-set-aside-in-the-player = Uma janela nova do player, nos vídeos que você separou.
about-video-metrics-nyxt-row = A página dele no YouTube, no Nyxt.
about-video-metrics-channel-in-chrome = A página de vídeos dele no YouTube, os mais recentes primeiro, numa janela do Chrome sem nada em volta.
about-video-metrics-channel-in-nyxt = A página de vídeos dele no YouTube, os mais recentes primeiro, no Nyxt.
# A LIST OF SUBSCRIPTIONS, IMPORTED AND EXPORTED (the owner, 2026-11-09):
# Google Takeout's list of the channels an account subscribes to, each
# channel staged for the person to keep or discard before the list is
# registered — and the channels here saved as a list of the same shape. No
# sentence spells a key: the gesture beside the name does.
video-metrics-import-export = Importar ou exportar uma lista
about-video-metrics-import-export = Traz os canais em que uma conta do YouTube é inscrita, da lista do Google Takeout, e salva os canais daqui numa lista do mesmo formato.
video-metrics-unregistered = { $count ->
        [one] um ainda não cadastrado
       *[other] { $count } ainda não cadastrados
    }
video-metrics-heading-import = Das inscrições do YouTube
video-metrics-import-note = O Google Takeout exporta os canais em que uma conta é inscrita: escolha o YouTube, depois as inscrições, e baixe a lista. Escolha-a aqui, e cada canal espera você mantê-lo ou descartá-lo.
video-metrics-takeout = Google Takeout
about-video-metrics-takeout = Abre o Google Takeout no Chrome, onde a lista de inscrições é exportada.
video-metrics-choose-list = Escolher a lista…
about-video-metrics-choose-list = Abre o seletor de arquivos nos seus downloads, mostrando as listas de canais.
video-metrics-pick-import = A lista de inscrições
video-metrics-filter-csv = Listas de canais (.csv)
about-video-metrics-pick = O seletor de arquivos, sobre a página: a pasta de onde a lista é lida ou para onde é gravada, e o arquivo.
video-metrics-act-takes = escolhe
video-metrics-import-left-out = Linhas deixadas de fora
about-video-metrics-import-left-out = Linhas do arquivo cuja primeira coluna não é o id de um canal do YouTube: nada delas foi trazido.
video-metrics-heading-already = Já no catálogo
video-metrics-heading-to-decide = A decidir
video-metrics-decisions = Até agora
video-metrics-decisions-value = mantidos: { $keep } · descartados: { $discard } · sem decisão: { $undecided }
about-video-metrics-decisions = Quantos canais da lista estão mantidos, descartados e ainda sem decisão.
video-metrics-decision = Decisão
about-video-metrics-decision = Se este canal entra no catálogo com os outros que você mantém, ou é descartado, quando a lista for cadastrada.
video-metrics-decision-undecided = sem decisão
video-metrics-decision-keep = mantido
video-metrics-decision-discard = descartado
video-metrics-decision-mixed = decisões variadas
video-metrics-decide-all = Todos eles
about-video-metrics-decide-all = Dá a mesma decisão a todo canal ainda por cadastrar: sem decisão, mantido ou descartado. Enquanto as decisões variam, pergunta antes.
# EVERY CHANNEL GIVEN ONE DECISION while theirs differ: what each was given
# goes, so it is asked — one sentence per decision, the count being the
# channels whose decision changes (never zero: theirs differ).
ask-video-metrics-decide-all-undecided = Deixar todos os canais da lista sem decisão? { $changed ->
        [one] Um canal perde a decisão.
       *[other] { $changed } canais perdem a decisão.
    }
ask-video-metrics-decide-all-keep = Marcar todos os canais da lista como mantidos? { $changed ->
        [one] A decisão de um canal muda.
       *[other] A decisão de { $changed } canais muda.
    }
ask-video-metrics-decide-all-discard = Marcar todos os canais da lista como descartados? { $changed ->
        [one] A decisão de um canal muda.
       *[other] A decisão de { $changed } canais muda.
    }
video-metrics-leave-decisions = Não, deixar como estão
video-metrics-decide-them-all = Sim, mudar todos
about-video-metrics-staged = Um canal da lista, ainda não cadastrado: mantido ou descartado quando a lista for cadastrada. A página dele define a pontuação, os Shorts, quanto tempo os vídeos ficam e as regras antes disso.
video-metrics-staged-note = Ainda não cadastrado: nada dele é varrido até que seja.
video-metrics-register = Cadastrar os mantidos
about-video-metrics-register = Cadastra os canais marcados como mantidos e descarta os descartados; os sem decisão ficam nesta página. Pergunta antes.
ask-video-metrics-register = Cadastrar { $kept ->
        [0] nenhum canal
        [one] um canal
       *[other] { $kept } canais
    } e descartar { $discarded ->
        [0] nenhum
        [one] um
       *[other] { $discarded }
    }? Os sem decisão continuam aqui.
video-metrics-not-yet = Não, ainda não
video-metrics-register-them = Sim, cadastrá-los
video-metrics-registered = { $kept ->
        [0] Nenhum canal cadastrado
        [one] Um canal cadastrado
       *[other] { $kept } canais cadastrados
    }; { $discarded ->
        [0] nenhum descartado
        [one] um descartado
       *[other] { $discarded } descartados
    }.
video-metrics-imported = { $staged ->
        [0] Nenhum canal
        [one] Um canal
       *[other] { $staged } canais
    } a decidir; já no catálogo: { $already }.
video-metrics-import-unreadable = Não foi possível ler o arquivo.
video-metrics-import-too-large = O arquivo é grande demais para ser uma lista de inscrições.
video-metrics-import-not-a-list = Nenhuma linha do arquivo nomeia um canal do YouTube: não é uma lista de inscrições.
video-metrics-import-not-text = O arquivo não é texto.
video-metrics-import-unclosed = Uma aspa aberta na linha { $line } nunca se fecha.
video-metrics-heading-export = Exportar
video-metrics-save-list = Salvar os canais numa lista…
about-video-metrics-save-list = Salva os canais cadastrados aqui numa lista no formato do Google, que esta página e os leitores do Takeout aceitam.
video-metrics-pick-export = Onde a lista de canais vai
video-metrics-exported = { $count ->
        [0] Nenhum canal salvo
        [one] Um canal salvo
       *[other] { $count } canais salvos
    } em { $file }.
video-metrics-export-failed = Não foi possível gravar a lista ali.
player-ranking-reading = Lendo o catálogo de vídeos…
# How many videos a page of the ranking holds — not the catalogue listing's
# "Per page", which is another page of the same view.
player-ranking-page-size = Vídeos por página
player-ranking-none = Nada pontuado ainda: o catálogo de vídeos ({ $keys }) adiciona os seus canais, e as varreduras dele enchem esta lista.
player-ranking-empty = Nenhum vídeo pontuado ainda: as varreduras não acharam nenhum, ou todos estão ocultos.
# A LIST WITH NOTHING ON IT, said for what it is: its Shorts turned away
# — the filter its page was read under — or nothing set aside yet.
player-ranking-empty-filtered = Nenhum vídeo pontuado com Shorts em “{ $shorts }”: as varreduras não acharam nenhum, ou a linha dos Shorts os deixa de fora.
player-ranking-aside-empty = Nada separado ainda: o marcador ao lado de um vídeo o separa.
player-ranking-aside-empty-filtered = Nada separado com Shorts em “{ $shorts }”: nada foi separado ainda, ou a linha dos Shorts deixa de fora o que foi.
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
player-ranking-not-changed = { $title } não mudou. { $why }
player-ranking-downloading = Baixando { $title }; { $keys } mostra as transferências.
player-ranking-downloading-plain = Baixando { $title }.
player-ranking-not-downloaded = A fila de transferências não aceitou { $title }: ela não está rodando.
# A MARK put on a video from the ranking, or taken off. Its rating's name is
# the video catalogue's own word for it.
player-ranking-rated = Sua avaliação de { $title }: { $rating }
player-ranking-unrated = { $title } não tem mais avaliação
player-ranking-now-set-aside = Separado: { $title }
player-ranking-no-longer-set-aside = Fora dos separados: { $title }
player-ranking-now-forever = Nunca será apagado: { $title }, pela marca dele — que vale seja o que for que o canal mantenha, e se o canal mudar
player-ranking-no-longer-forever = { $title }: a marca dele de nunca apagar saiu — vale de novo o tempo do canal, ou o do catálogo
# A MARK PRESSED while the last one on the same video is still being written:
# two writes on their own threads land in either order.
player-ranking-still-marking = { $title }: a última marca dele ainda está sendo gravada; tente de novo num instante.
player-ranking-shown-again = De volta ao ranking: { $title }
