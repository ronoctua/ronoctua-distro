# The texts `ronoctua-shell-catalogue` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# CANAIS POR TEMA, no catálogo local (`player_catalog`): uma fonte por vez,
# seus temas e categorias, e a listagem que uma linha deles abre.
player-about-catalog = Os canais, filmes e vídeos de cada fonte, lidos para esta máquina e filtrados na hora. Uma fonte é lida de novo quando aparece depois do prazo dela.
player-catalog-source = Canais de
player-catalog-ingesting = Lendo esta fonte…
player-catalog-stale = Lida { $date }; é hora de ler de novo.
player-catalog-never = Ainda não lida.
player-catalog-read-again = Ler esta fonte de novo
player-catalog-reading-now = Lendo { $name }…
player-catalog-search = Buscar pelo título
player-catalog-searched = “{ $words }”
player-catalog-searching = Buscando “{ $words }” no arquivo…
player-catalog-search-archive = Buscar “{ $words }” no arquivo inteiro
player-catalog-themes = Temas
player-catalog-categories = Categorias
player-catalog-everything = Tudo
player-catalog-ungrouped = Sem categoria
player-catalog-empty = Ainda não há nada aqui.
player-catalog-nothing-found = Nada corresponde.
player-catalog-adult = Só para adultos
player-catalog-letter = Inicial
player-catalog-letter-all = Todas
player-catalog-page-not-a-number = “{ $said }” não é um número de página.
player-catalog-per-page = Por página
player-catalog-sort = Ordem
player-catalog-sort-title = Pelo título
player-catalog-sort-popular = Mais vistos primeiro
player-catalog-sort-newest = Mais novos primeiro
player-catalog-sort-oldest = Mais antigos primeiro
player-catalog-country = País
player-catalog-country-all = Todos os países
player-catalog-language = Língua
player-catalog-language-all = Todas as línguas
player-catalog-quality = Altura da imagem
player-catalog-quality-any = Qualquer altura
player-catalog-quality-lines = { $lines } linhas ou mais
player-catalog-layout-lines = Duas linhas
player-catalog-layout-columns = Colunas
player-catalog-layout-posters = Capas
player-catalog-layout-big = Capas grandes
player-catalog-scope-in = Buscar em
player-catalog-scope-category = Esta lista
player-catalog-scope-every-category = Toda a fonte
player-catalog-scope-from = De
player-catalog-scope-source = Esta fonte
player-catalog-scope-every-source = Todas as fontes
player-catalog-kind-live = ao vivo
player-catalog-kind-audio = áudio
player-catalog-item-gone = Não está mais no catálogo.
ask-player-catalog-letter = Inicial
ask-player-catalog-page = Número da página
ask-player-catalog-search = Títulos com
# O CATÁLOGO ADULTO, atrás da senha do sistema da própria pessoa.
player-adult-locked = Trancado: a sua senha do sistema abre
player-adult-checking = conferindo…
ask-player-adult-password = A sua senha do sistema
player-adult-lock = Trancar agora
player-adult-locked-again = O catálogo adulto está trancado.
player-adult-shut = Trancado
player-adult-honest = Uma tranca nestas portas, não uma cifra: a lista é um arquivo neste disco como qualquer outro.
# AS FONTES DO CATÁLOGO (`player_sources`).
player-about-sources = Cole o link de uma lista IPTV, de um feed de podcast ou de vídeos, de um canal ou playlist do YouTube, ou de uma coleção do archive.org. O endereço fica privado nesta máquina e aparece só pelo host.
player-source-add = Adicionar uma fonte…
ask-player-source-url = Link de uma lista, um feed, um canal ou uma coleção
player-source-add-adult = Adicionar uma fonte ao catálogo adulto…
ask-player-adult-source-url = Link de uma lista para o catálogo adulto
player-source-adding = Lendo { $name }…
player-source-address = Endereço
player-source-plain = Sem criptografia: este endereço é http, e o que ele responde — a lista, a conta no endereço dela e as capas que ela guarda nesse mesmo site — atravessa a rede às claras.
player-source-kind = Tipo
player-source-kind-iptv-org = Índice de TV do iptv-org
player-source-kind-archive = Coleções do Internet Archive
player-source-kind-youtube-lives = Buscas de lives no YouTube
player-source-kind-nasa = Biblioteca da NASA
player-source-kind-commons = Categorias do Wikimedia Commons
player-source-kind-peertube = Vídeos do PeerTube
player-source-kind-playlist = Lista IPTV (M3U)
player-source-kind-feed = Feed (RSS ou Atom)
player-source-kind-youtube = Canal ou playlist do YouTube
player-source-kind-collection = Coleção do archive.org
player-source-posters = Buscar as capas
player-source-hosts = de { $hosts }
player-source-hosts-any = de onde a lista disser
player-source-posters-on = As capas dela serão buscadas.
player-source-posters-off = As capas dela não serão buscadas.
player-source-adult = No catálogo adulto
player-source-adult-on = Ela vai para o catálogo adulto.
player-source-adult-off = Ela sai do catálogo adulto.
player-source-adult-hidden = Fontes do catálogo adulto
player-source-remove = Remover…
ask-player-source-remove = Remover { $name } e tudo o que ela trouxe?
player-source-removed = { $name } foi removida.
