# The texts `ronoctua-iconset` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# ICONS — the library the icons app and fast-access share (`ronoctua-iconset`).
# The words a person TYPES before a colon to narrow a search: the search
# language is interface, so every language's word is accepted.
icons-prefix-block = bloco
icons-prefix-tag = etiqueta
icons-prefix-token = token
icons-prefix-app = app
icons-prefix-symbols = simbolos
icons-prefix-family = familia
icons-prefix-category = categoria
icons-prefix-cat = cat
icons-prefix-name = nome
icons-prefix-group = grupo
icons-prefix-subgroup = subgrupo
icons-prefix-tone = tom
icons-set-cod = Codicons
icons-set-custom = Personalizados
icons-set-dev = Devicons
icons-set-extra = Extra
icons-set-fa = Font Awesome
icons-set-fae = Font Awesome Extension
icons-set-iec = Símbolos de energia IEC
icons-set-indent = Indentação
icons-set-indentation = Indentação
icons-set-linux = Logos de distribuições
icons-set-md = Material Design
icons-set-oct = Octicons
icons-set-pl = Powerline
icons-set-ple = Powerline Extra
icons-set-pom = Pomicons
icons-set-seti = Seti-UI
icons-set-weather = Clima
icons-category-accessibility = Acessibilidade
icons-category-account = Conta
icons-category-animals = Animais
icons-category-arrows = Setas
icons-category-brands = Marcas
icons-category-buildings = Edifícios
icons-category-charts = Gráficos
icons-category-communication = Comunicação
icons-category-connectivity = Conectividade
icons-category-cursors = Cursores
icons-category-design = Design
icons-category-development = Desenvolvimento
icons-category-devices = Dispositivos
icons-category-emoji = Emoji
icons-category-files = Arquivos
icons-category-finance = Finanças
icons-category-food-beverage = Comida
icons-category-gaming = Jogos
icons-category-home = Casa
icons-category-layout = Layout
icons-category-mail = Correio
icons-category-math = Matemática
icons-category-medical = Médico
icons-category-multimedia = Multimídia
icons-category-nature = Natureza
icons-category-navigation = Navegação
icons-category-notifications = Notificações
icons-category-people = Pessoas
icons-category-photography = Fotografia
icons-category-science = Ciência
icons-category-seasons = Estações
icons-category-security = Segurança
icons-category-shapes = Formas
icons-category-shopping = Compras
icons-category-social = Social
icons-category-sports = Esportes
icons-category-sustainability = Sustentabilidade
icons-category-text = Texto
icons-category-time = Tempo
icons-category-tools = Ferramentas
icons-category-transportation = Transporte
icons-category-travel = Viagem
icons-category-weather = Clima
icons-family-symbol = Símbolo
icons-family-letter = Letra
icons-family-number = Número
icons-family-mark = Marca
icons-family-punctuation = Pontuação
icons-family-other = Outro
icons-family-emoji = Emoji
icons-group-faces = Carinhas e emoções
icons-group-people = Pessoas e corpo
icons-group-nature = Animais e natureza
icons-group-food = Comida e bebida
icons-group-travel = Viagem e lugares
icons-group-activities = Atividades
icons-group-objects = Objetos
icons-group-symbols = Símbolos
icons-group-flags = Bandeiras
icons-tone-light = Pele clara
icons-tone-medium-light = Pele clara média
icons-tone-medium = Pele média
icons-tone-medium-dark = Pele escura média
icons-tone-dark = Pele escura
# Why there are no icons to show.
icons-why-no-variable = As tabelas de ícones não foram entregues a este programa (RONOCTUA_ICONSET não está definida).
icons-why-unreadable = Uma tabela de ícones falta ou não pode ser lida: { $file }.
icons-why-garbled = Uma tabela de ícones não está no formato que o gerador escreve: { $file }.
