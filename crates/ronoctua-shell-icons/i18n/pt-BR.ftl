# The texts `ronoctua-shell-icons` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Comandos do compositor — §13, "toda ação do SO tem um comando"

group-icons = ícones
# The value words of the enumeration filters (`family:letter`), typed after
# the prefix. One word each, without accents: they are typed, not read.
icons-value-family-symbol = simbolo
icons-value-family-letter = letra
icons-value-family-number = numero
icons-value-family-mark = marca
icons-value-family-punctuation = pontuacao
icons-value-family-other = outro
icons-value-family-emoji = emoji
icons-value-group-faces = carinhas
icons-value-group-people = pessoas
icons-value-group-nature = natureza
icons-value-group-food = comida
icons-value-group-travel = viagem
icons-value-group-activities = atividades
icons-value-group-objects = objetos
icons-value-group-symbols = simbolos
icons-value-group-flags = bandeiras
icons-value-tone-light = claro
icons-value-tone-medium-light = medio-claro
icons-value-tone-medium = medio
icons-value-tone-medium-dark = medio-escuro
icons-value-tone-dark = escuro
# The panels' labels. Typed into the search, each finds its whole category —
# the generators put the same words into the tags.
icons-set-all = Todos os conjuntos
icons-category-all = Todas as categorias
icons-family-all = Todas as famílias

## file info editing — everything the info panel can change

# THE ICONS APP (`LEADER m i`) — `crate::icons`, `crate::icons_page`,
# `crate::icons_window`. One app for the three collections the three buffers
# used to be: Nerd Fonts, Lucide and Unicode.
cmd-icons-open = Ícones
cmd-icons-window = Ícones, numa janela
cmd-icons-nerd-fonts = Navegar pelos glifos das Nerd Fonts
cmd-icons-lucide = Navegar pelos ícones Lucide
cmd-icons-unicode = Navegar pelos caracteres Unicode
cmd-icons-colour = A cor do ícone Lucide
icons-title = Ícones
# The three screens, as their tabs read.
icons-screen-nerd-fonts = Nerd Fonts
icons-screen-lucide = Lucide
icons-screen-unicode = Unicode
about-icons-tab-nerd-fonts = Mostra os glifos das Nerd Fonts
about-icons-tab-lucide = Mostra os ícones Lucide
about-icons-tab-unicode = Mostra os caracteres Unicode e os emoji
# What the tab line says where nothing is typed yet, per screen.
icons-search-nerd-fonts = digite para buscar um glifo
icons-search-lucide = digite para buscar um ícone
icons-search-unicode = digite para buscar um caractere
icons-count = { $shown } de { $total }
icons-loading = Lendo as tabelas de ícones…
icons-none = Nada aqui corresponde a “{ $query }”.
# The grid's keys, and the page's.
icons-page = abrir
about-icons-page = Abre a página do ícone sob o cursor: o que ele é, o tamanho e todas as formas de copiá-lo
icons-copy = copiar
about-icons-copy = Copia o ícone sob o cursor, como ele é desenhado
icons-less = menos
about-icons-less = Diminui o ajuste sob o cursor, ou mostra o ícone anterior
icons-more = mais
about-icons-more = Aumenta o ajuste sob o cursor, ou mostra o próximo ícone
icons-enter = copiar
about-icons-enter = Copia no formato sob o cursor, ou muda o ajuste sob ele
icons-reset = restaurar
about-icons-reset = Devolve todos os ajustes ao que eram
icons-filters = filtros
about-icons-filters = Abre ou fecha o painel de filtros: os conjuntos, as categorias e as famílias, cada escolha escrita na busca
icons-choose = escolher
about-icons-choose = Filtra pela escolha sob o cursor, mantendo as palavras digitadas
icons-change = mudar
about-icons-change = Aumenta o ajuste sob o cursor — do máximo, volta ao mínimo
# O título da segunda seção do painel: filtros que só os emoji têm.
icons-panel-emoji = Só emoji
icons-colour-closed = A cor do Lucide é do app de ícones: abra-o com { $keys } e depois a página de um ícone Lucide.
# The icon's page: its sections, its facts and its settings.
icons-heading-facts = O que é
icons-heading-adjust = Ajustes
icons-heading-copy = Copiar como
icons-fact-name = Nome
icons-fact-codepoint = Codepoint
icons-fact-set = Conjunto
icons-fact-token = Token de ícone
icons-fact-tray = Atribuído a um app
# One row of tags per language, the key completed with its subtag —
# `buffer-tag-rows-have-text` asks for one per declared language.
icons-sheet-tags-en = Tags (en)
icons-sheet-tags-pt = Tags (pt)
icons-fact-component = Componente
icons-fact-categories = Categorias
icons-fact-aliases = Também chamado
icons-fact-family = Família
icons-fact-category = Categoria
icons-fact-block = Bloco
icons-fact-decomposition = Decomposição
icons-fact-width = Largura
icons-fact-group = Grupo
icons-fact-subgroup = Subgrupo
icons-fact-tone = Tom de pele
icons-width-zero = Marca combinante — o apoio pontilhado não é dela
icons-width-one = Uma célula
icons-width-double = Ocupa duas células
icons-yes = sim
icons-no = não
icons-adjust-size = Tamanho
icons-adjust-stroke = Espessura
icons-adjust-absolute = Traço absoluto
icons-adjust-colour = Cor
icons-adjust-cap = Pontas
icons-size-px = { $px } px
icons-cap-round = redondas
icons-cap-butt = retas
icons-cap-square = quadradas
# The copy formats, per collection (`ronoctua_iconset::copy`).
icons-copy-nerd-fonts-name = Nome
icons-copy-nerd-fonts-glyph = Glifo
icons-copy-nerd-fonts-css = Classe CSS
icons-copy-nerd-fonts-hex = Hexadecimal
icons-copy-nerd-fonts-utf16 = UTF-16
icons-copy-nerd-fonts-escape = Escape Unicode
icons-copy-nerd-fonts-html = Entidade HTML
icons-copy-nerd-fonts-rust = Literal Rust
icons-copy-nerd-fonts-token = Token do design system
icons-copy-unicode-character = Caractere
icons-copy-unicode-name = Nome oficial
icons-copy-unicode-hex = Hexadecimal
icons-copy-unicode-decimal = Decimal
icons-copy-unicode-escape = Escape Unicode
icons-copy-unicode-html = Entidade HTML
icons-copy-unicode-rust = Literal Rust
icons-copy-unicode-utf16 = UTF-16
icons-copy-unicode-utf8 = Bytes UTF-8
icons-copy-lucide-svg = SVG
icons-copy-lucide-jsx = JSX
icons-copy-lucide-vanilla = HTML (lucide.js)
icons-copy-lucide-react = React
icons-copy-lucide-vue = Vue
icons-copy-lucide-svelte = Svelte
icons-copy-lucide-preact = Preact
icons-copy-lucide-solid = Solid
icons-copy-lucide-angular = Angular
icons-copy-lucide-font = Fonte de ícones
icons-copy-lucide-name = Nome
icons-copy-lucide-component = Componente
icons-copy-lucide-character = Caractere
icons-copy-lucide-hex = Codepoint
icons-copy-lucide-path = Traçado
icons-copy-lucide-uri = Data URI
icons-copy-lucide-base64 = Base64
icons-copy-lucide-css = Regra CSS
icons-copy-lucide-sprite = Sprite
