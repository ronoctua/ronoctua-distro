# The texts `ronoctua-shell-fast-access` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

calc-rates-why-unreachable = Não foi possível alcançar o Banco Central Europeu.

## file info editing — everything the info panel can change

ask-icons-colour = A cor do traço (uma cor CSS, ou currentColor)
# THE WORDS OF `ronoctua-words`, for every door that translates or looks a
# word up: why it answered nothing, the parts of speech, who translated it
# and where a meaning came from.
words-why-empty = Não há nada para traduzir ou procurar.
words-why-too-long = O texto é mais longo do que o tradutor aceita de uma vez.
words-why-unknown-tongue = O tradutor não conhece essa língua.
words-why-wrong-pass = A permissão dada é para outro site, não o que isto perguntaria.
words-why-no-key = O tradutor precisa de uma chave, e nenhuma está guardada.
words-why-key-refused = O tradutor recusou a chave.
words-why-quota = A cota de hoje do tradutor acabou.
words-why-paced = Perguntado vezes demais; ele responde daqui a pouco.
words-why-not-found = A palavra não está no dicionário.
words-why-no-dictionary = Não há dicionário desta língua nesta máquina, e perguntar ao Wiktionary não foi permitido.
words-why-unreachable = O site não pôde ser alcançado.
words-why-failed = O site respondeu com uma falha.
words-why-garbled = O que voltou não é o que foi pedido.
words-part-noun = substantivo
words-part-proper-noun = nome próprio
words-part-verb = verbo
words-part-adjective = adjetivo
words-part-adverb = advérbio
words-part-pronoun = pronome
words-part-preposition = preposição
words-part-conjunction = conjunção
words-part-interjection = interjeição
words-part-numeral = numeral
words-part-determiner = determinante
words-part-phrase = expressão
words-part-affix = afixo
words-part-other = outro
words-provider-mymemory = MyMemory
words-provider-deepl = DeepL
words-provider-libretranslate = LibreTranslate
words-source-wordnet = Open English WordNet, nesta máquina
words-source-wiktionary = Wiktionary
words-translated-by = Traduzido por { $provider }
# FAST-ACCESS (`LEADER d`, `crate::fast_access`): the search of everything.
fast-access-title = Busca
group-fast-access = busca
cmd-fast-access-open = Buscar em tudo, no rodapé
cmd-fast-access-window = Buscar em tudo, numa janela
cmd-fast-access-column = Buscar em tudo, na coluna da direita
cmd-fast-access-float = Buscar em tudo, sobre a tela inteira
cmd-fast-access-launch = Abrir um aplicativo pelo id
cmd-fast-access-open-path = Abrir um arquivo com o que o abre
cmd-fast-access-reveal = Mostrar um arquivo no gerenciador de arquivos
cmd-fast-access-man = Ler uma página de manual num terminal
fast-access-placeholder = digite para buscar · { $slash } lista como estreitar a busca
fast-access-empty = Seus aplicativos, comandos, arquivos e o que eles dizem, páginas de manual, programas para instalar, ícones e contas — digite uma palavra. Uma barra e uma letra estreitam a busca.
fast-access-nothing = Nada encontrado para “{ $words }”.
fast-access-searching = Buscando…
fast-access-reading-icons = Lendo as tabelas de ícones…
fast-access-icons-refused = As tabelas de ícones não puderam ser lidas.
# The scopes a slash and a letter narrow to (`fast_access::Scope`): a noun,
# and what it searches.
fast-access-scope-all = tudo
fast-access-scope-audio = áudio
fast-access-scope-audio-does = arquivos de áudio, pelo nome e pelo que está escrito neles
fast-access-scope-video = vídeos
fast-access-scope-video-does = arquivos de vídeo nesta máquina
fast-access-scope-youtube = YouTube
fast-access-scope-youtube-does = seu catálogo de vídeos, e o próprio YouTube depois de uma pausa
fast-access-scope-images = imagens
fast-access-scope-images-does = imagens nesta máquina
fast-access-scope-documents = documentos
fast-access-scope-documents-does = documentos, livros e textos
fast-access-scope-names = nomes
fast-access-scope-names-does = arquivos e pastas só pelo nome
fast-access-scope-contents = conteúdo
fast-access-scope-contents-does = o que está escrito dentro dos arquivos
fast-access-scope-manuals = manuais
fast-access-scope-manuals-does = as páginas de manual do sistema e dos seus programas
fast-access-scope-programs = programas
fast-access-scope-programs-does = programas que dá para instalar
fast-access-scope-commands = comandos
fast-access-scope-commands-does = os comandos desta área de trabalho, com as teclas
fast-access-scope-web = web
fast-access-scope-web-does = Wikipedia e DuckDuckGo, depois de uma pausa
fast-access-scope-dictionary = dicionário
fast-access-scope-dictionary-does = o que uma palavra quer dizer
fast-access-scope-synonyms = sinônimos
fast-access-scope-synonyms-does = outras palavras para uma palavra
fast-access-scope-translate = traduzir
fast-access-scope-translate-does = uma tradução — um código de língua primeiro manda para ela ({ $prefix } en …)
fast-access-scope-icons = ícones
fast-access-scope-icons-does = glifos do Nerd Fonts, figuras do Lucide e caracteres Unicode
fast-access-scope-math = contas
fast-access-scope-math-does = contas, unidades, bases, datas e fusos — “2+2” funciona sem isso
fast-access-scope-currency = moedas
fast-access-scope-currency-does = dinheiro de uma moeda para outra, nas cotações de hoje
fast-access-scope-zones = fusos horários
fast-access-scope-zones-does = que horas são em outro lugar
fast-access-scope-recent = recentes
fast-access-scope-recent-does = o que você abriu daqui, o mais usado primeiro
fast-access-scope-prefixes = estreitar
fast-access-scope-prefixes-does = todos os jeitos de estreitar uma busca
# The sections of the results (`fast_access::SectionId`).
fast-access-section-prefixes = Estreitar a busca
fast-access-section-math = Conta
fast-access-section-currency = Moeda
fast-access-section-zones = Fusos horários
fast-access-section-words = Palavras
fast-access-section-recent = Recentes
fast-access-section-apps = Aplicativos
fast-access-section-commands = Comandos
fast-access-section-icons = Ícones
fast-access-section-files = Arquivos
fast-access-section-folders = Pastas
fast-access-section-contents = O que os arquivos dizem
fast-access-section-videos = Vídeos
fast-access-section-youtube = YouTube
fast-access-section-web = A web
fast-access-section-manuals = Páginas de manual
fast-access-section-packages = Programas para instalar
fast-access-section-settings = Esta busca
fast-access-strip-nerd-fonts = Nerd Fonts
fast-access-strip-lucide = Lucide
fast-access-strip-unicode = Unicode
fast-access-strip-none = { $collection }: nada corresponde
fast-access-strip-about = { $collection } · { $name }
fast-access-strip-earlier = { $collection }: a página anterior
fast-access-strip-later = { $collection }: a próxima página
# What the index answered nothing for (`ronoctua_index::Why`).
fast-access-index-why-no-engine = Esta compilação não tem índice.
fast-access-index-why-not-built = O índice ainda não foi montado; os arquivos aparecem à medida que ele percorre suas pastas.
fast-access-index-why-unreadable = O índice não pôde ser lido.
fast-access-index-why-other-format = O índice é de outra versão; está sendo montado de novo.
fast-access-index-why-busy = O índice está ocupado.
fast-access-index-why-full = O índice está cheio.
fast-access-index-why-failed = O índice não conseguiu responder isso.
# The web (`ronoctua_websearch`).
fast-access-web-wikipedia = Wikipedia
fast-access-web-duckduckgo = DuckDuckGo
fast-access-web-searxng = SearXNG
fast-access-web-brave = Brave Search
fast-access-web-kiosk = Buscar “{ $words }” na web, no Chrome
fast-access-web-for = Uma busca na web pelo que você digitou.
fast-access-web-why-empty = Não há palavras para buscar.
fast-access-web-why-wrong-pass = A permissão dada é para outro site.
fast-access-web-why-no-key = O Brave Search precisa de uma chave, e nenhuma está guardada.
fast-access-web-why-key-refused = O Brave Search recusou a chave.
fast-access-web-why-paced = Perguntado vezes demais; ele responde daqui a pouco.
fast-access-web-why-unreachable = O site não pôde ser alcançado.
fast-access-web-why-failed = O site respondeu com uma falha.
fast-access-web-why-garbled = O que voltou não é uma lista de resultados.
# YouTube (`/y`).
fast-access-youtube-for = Uma busca no YouTube pelo que você digitou.
fast-access-thumbs-for = As imagens dos vídeos encontrados.
fast-access-youtube-local = no seu catálogo
fast-access-youtube-online = no YouTube
fast-access-shorts-shown = Shorts: mostrados
fast-access-shorts-hidden = Shorts: deixados de fora
fast-access-youtube-why-absent = Esta instalação não tem yt-dlp.
fast-access-youtube-why-unstartable = O yt-dlp não pôde ser iniciado.
fast-access-youtube-why-slow = O YouTube demorou demais para responder.
fast-access-youtube-why-failed = O YouTube não respondeu à busca.
fast-access-youtube-why-garbled = O que voltou não é uma lista de vídeos.
fast-access-youtube-why-too-large = A resposta era grande demais.
fast-access-youtube-why-not-an-address = Isso não é um endereço.
fast-access-youtube-why-not-a-list = Isso é um vídeo, não uma lista.
fast-access-youtube-why-nothing-to-ask = Não há palavras para buscar.
fast-access-views = { $n ->
        [one] { $n } visualização
       *[other] { $n } visualizações
    }
# Words (`/d`, `/s`, `/t`).
fast-access-words-for = Procurando a palavra no Wiktionary.
fast-access-translate-for = Uma tradução do que você digitou.
fast-access-translate-into = Para { $tongue }
fast-access-synonyms-none = Nenhuma outra palavra para ela.
# Sums, currencies and zones.
fast-access-math-also = O mesmo, em outras unidades
fast-access-zone-none = Nenhum fuso horário tem esse nome.
# The detail pane (`fast_access_detail`).
fast-access-detail-name = Nome
fast-access-detail-address = Endereço
fast-access-detail-channel = Canal
fast-access-detail-duration = Duração
fast-access-detail-views = Visto
fast-access-detail-from = Encontrado
fast-access-detail-keys = Teclas
fast-access-detail-command = Comando
fast-access-detail-program = Programa
fast-access-detail-section = Seção
fast-access-detail-description = Sobre
fast-access-detail-source = De
fast-access-detail-zone = Fuso
fast-access-detail-time = Agora lá
fast-access-detail-scope = Busca
fast-access-detail-short = Um Short
fast-access-detail-version = Versão
fast-access-detail-licence = Licença
fast-access-detail-site = Site
fast-access-detail-runs = Executa
fast-access-detail-download = Download
fast-access-act-open = Abrir
fast-access-act-run = Executar
fast-access-act-copy = Copiar
fast-access-act-copy-address = Copiar o endereço
fast-access-act-play-player = Tocar no player
fast-access-act-play-kiosk = Abrir no quiosque do Chrome
fast-access-act-install = Instalar, pela fila
fast-access-act-centre = Mostrar na central de programas
fast-access-act-terminal = Ler num terminal
fast-access-act-narrow = Estreitar a busca a isto
fast-access-act-kiosk = Abrir no quiosque do Chrome
# The question a web video asks (`fast_access::Question::PlayIn`).
fast-access-play-in = Tocar “{ $title }” no:
fast-access-play-player = player
fast-access-play-kiosk = quiosque do Chrome
fast-access-play-remember = e fazer o mesmo daqui em diante
# The settings of the search, at the foot of the empty page.
fast-access-setting-park = Deixar a janela pronta depois de fechar
fast-access-setting-door = Vídeos da web tocam no
fast-access-door-ask = perguntado toda vez
fast-access-setting-searxng = Seu SearXNG, consultado junto dos outros
fast-access-searxng-none = nenhum
fast-access-searxng-bad = “{ $address }” não é um endereço https com um host.
fast-access-setting-brave = Sua chave do Brave Search
fast-access-brave-none = nenhuma
fast-access-brave-kept-short = guardada
fast-access-brave-kept = A chave do Brave Search está guardada, legível só por você.
fast-access-brave-forgotten = A chave do Brave Search foi esquecida.
fast-access-web-key-no-home = Não há pasta pessoal onde guardar a chave.
fast-access-web-key-empty = Nada foi digitado.
fast-access-web-key-malformed = A chave tem uma quebra de linha ou um caractere de controle.
fast-access-web-key-unwritable = Não foi possível gravar a chave.
fast-access-yes = sim
fast-access-no = não
fast-access-unkept = As configurações da busca (fast-access.json) não puderam ser gravadas: { $why }
ask-fast-access-tag = O novo valor da etiqueta
ask-fast-access-meta = O novo valor do detalhe
ask-fast-access-searxng = O endereço do seu SearXNG (https://…), vazio para nenhum
ask-fast-access-brave = Sua chave do Brave Search, vazio para esquecê-la
fast-access-recent-none = Nada foi aberto daqui ainda.
# The keys the header teaches (`fast_access::OF_THE_VIEW`).
fast-access-enter = abrir
about-fast-access-enter = Faz aquilo para que serve a linha do cursor: abre, executa, toca ou copia — e a busca fecha, menos na coluna.
fast-access-secondary = o outro jeito
about-fast-access-secondary = Faz a segunda coisa que a linha oferece: mostra um arquivo no gerenciador, um vídeo no outro player, a página de um programa.
fast-access-copy = copiar
about-fast-access-copy = Copia o que a linha é: um caminho, um endereço, uma resposta, um caractere.
fast-access-detail = detalhe
about-fast-access-detail = Leva as teclas para o detalhe ao lado dos resultados, e de volta.
fast-access-whole = inteiro
about-fast-access-whole = Dá ao detalhe a área inteira, e devolve.
fast-access-shorts = Shorts
about-fast-access-shorts = Mostra os Shorts do YouTube entre os vídeos encontrados, ou os deixa de fora.
fast-access-clear = limpar
about-fast-access-clear = Esvazia o que foi digitado.
fast-access-page = página
about-fast-access-page = Mostra os próximos ícones de uma faixa — ou os anteriores.
