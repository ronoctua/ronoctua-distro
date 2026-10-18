# The texts `ronoctua-iconset` says, in en-GB (invariant 8).
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
icons-prefix-block = block
icons-prefix-tag = tag
icons-prefix-token = token
icons-prefix-app = app
icons-prefix-symbols = symbols
icons-prefix-family = family
icons-prefix-category = category
icons-prefix-cat = cat
icons-prefix-name = name
icons-prefix-group = group
icons-prefix-subgroup = subgroup
icons-prefix-tone = tone
icons-set-cod = Codicons
icons-set-custom = Custom
icons-set-dev = Devicons
icons-set-extra = Extra
icons-set-fa = Font Awesome
icons-set-fae = Font Awesome Extension
icons-set-iec = IEC Power
icons-set-indent = Indentation
icons-set-indentation = Indentation
icons-set-linux = Font Logos
icons-set-md = Material Design
icons-set-oct = Octicons
icons-set-pl = Powerline
icons-set-ple = Powerline Extra
icons-set-pom = Pomicons
icons-set-seti = Seti-UI
icons-set-weather = Weather
icons-category-accessibility = Accessibility
icons-category-account = Account
icons-category-animals = Animals
icons-category-arrows = Arrows
icons-category-brands = Brands
icons-category-buildings = Buildings
icons-category-charts = Charts
icons-category-communication = Communication
icons-category-connectivity = Connectivity
icons-category-cursors = Cursors
icons-category-design = Design
icons-category-development = Development
icons-category-devices = Devices
icons-category-emoji = Emoji
icons-category-files = Files
icons-category-finance = Finance
icons-category-food-beverage = Food
icons-category-gaming = Gaming
icons-category-home = Home
icons-category-layout = Layout
icons-category-mail = Mail
icons-category-math = Math
icons-category-medical = Medical
icons-category-multimedia = Multimedia
icons-category-nature = Nature
icons-category-navigation = Navigation
icons-category-notifications = Notifications
icons-category-people = People
icons-category-photography = Photography
icons-category-science = Science
icons-category-seasons = Seasons
icons-category-security = Security
icons-category-shapes = Shapes
icons-category-shopping = Shopping
icons-category-social = Social
icons-category-sports = Sports
icons-category-sustainability = Sustainability
icons-category-text = Text
icons-category-time = Time
icons-category-tools = Tools
icons-category-transportation = Transportation
icons-category-travel = Travel
icons-category-weather = Weather
icons-family-symbol = Symbol
icons-family-letter = Letter
icons-family-number = Number
icons-family-mark = Mark
icons-family-punctuation = Punctuation
icons-family-other = Other
icons-family-emoji = Emoji
icons-group-faces = Smileys & emotion
icons-group-people = People & body
icons-group-nature = Animals & nature
icons-group-food = Food & drink
icons-group-travel = Travel & places
icons-group-activities = Activities
icons-group-objects = Objects
icons-group-symbols = Symbols
icons-group-flags = Flags
icons-tone-light = Light skin
icons-tone-medium-light = Medium-light skin
icons-tone-medium = Medium skin
icons-tone-medium-dark = Medium-dark skin
icons-tone-dark = Dark skin
# Why there are no icons to show.
icons-why-no-variable = The icon tables were not given to this program (RONOCTUA_ICONSET is not set).
icons-why-unreadable = An icon table is missing or cannot be read: { $file }.
icons-why-garbled = An icon table is not in the shape its generator writes: { $file }.
