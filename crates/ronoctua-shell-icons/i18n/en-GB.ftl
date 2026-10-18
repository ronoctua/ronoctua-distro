# The texts `ronoctua-shell-icons` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Compositor commands — §13, "every OS action has a command"

group-icons = icons
# The value words of the enumeration filters (`family:letter`), typed after
# the prefix. One word each, without accents: they are typed, not read.
icons-value-family-symbol = symbol
icons-value-family-letter = letter
icons-value-family-number = number
icons-value-family-mark = mark
icons-value-family-punctuation = punctuation
icons-value-family-other = other
icons-value-family-emoji = emoji
icons-value-group-faces = faces
icons-value-group-people = people
icons-value-group-nature = nature
icons-value-group-food = food
icons-value-group-travel = travel
icons-value-group-activities = activities
icons-value-group-objects = objects
icons-value-group-symbols = symbols
icons-value-group-flags = flags
icons-value-tone-light = light
icons-value-tone-medium-light = medium-light
icons-value-tone-medium = medium
icons-value-tone-medium-dark = medium-dark
icons-value-tone-dark = dark
# The panels' labels. Typed into the search, each finds its whole category —
# the generators put the same words into the tags.
icons-set-all = All sets
icons-category-all = All categories
icons-family-all = All families

## file info editing — everything the info panel can change

# THE ICONS APP (`LEADER m i`) — `crate::icons`, `crate::icons_page`,
# `crate::icons_window`. One app for the three collections the three buffers
# used to be: Nerd Fonts, Lucide and Unicode.
cmd-icons-open = Icons
cmd-icons-window = Icons, in a window
cmd-icons-nerd-fonts = Browse the Nerd Fonts glyphs
cmd-icons-lucide = Browse the Lucide icons
cmd-icons-unicode = Browse the Unicode characters
cmd-icons-colour = The Lucide icon's colour
icons-title = Icons
# The three screens, as their tabs read.
icons-screen-nerd-fonts = Nerd Fonts
icons-screen-lucide = Lucide
icons-screen-unicode = Unicode
about-icons-tab-nerd-fonts = Show the Nerd Fonts glyphs
about-icons-tab-lucide = Show the Lucide icons
about-icons-tab-unicode = Show the Unicode characters and emoji
# What the tab line says where nothing is typed yet, per screen.
icons-search-nerd-fonts = type to search the glyphs
icons-search-lucide = type to search the icons
icons-search-unicode = type to search the characters
icons-count = { $shown } of { $total }
icons-loading = Reading the icon tables…
icons-none = Nothing here matches “{ $query }”.
# The grid's keys, and the page's.
icons-page = open
about-icons-page = Open the page of the icon under the cursor: its facts, its size and every way to copy it
icons-copy = copy
about-icons-copy = Copy the icon under the cursor, as it is drawn
icons-less = less
about-icons-less = Turn the setting under the cursor down, or show the icon before this one
icons-more = more
about-icons-more = Turn the setting under the cursor up, or show the icon after this one
icons-enter = copy
about-icons-enter = Copy in the format under the cursor, or change the setting under it
icons-reset = reset
about-icons-reset = Put every adjustment back as it was
icons-filters = filters
about-icons-filters = Open or shut the panel of filters: the sets, the categories and the families, each a choice that writes itself into the search
icons-choose = choose
about-icons-choose = Filter by the choice under the cursor, keeping the words typed
icons-change = change
about-icons-change = Turn the setting under the cursor up — from its top, back to its bottom
# The heading of the panel's second section: filters only emoji have.
icons-panel-emoji = Only emoji
icons-colour-closed = The Lucide colour belongs to the icons app: open it with { $keys }, then a Lucide icon's page.
# The icon's page: its sections, its facts and its settings.
icons-heading-facts = About it
icons-heading-adjust = Adjust
icons-heading-copy = Copy as
icons-fact-name = Name
icons-fact-codepoint = Codepoint
icons-fact-set = Set
icons-fact-token = Icon token
icons-fact-tray = Assigned to an app
# One row of tags per language, the key completed with its subtag —
# `buffer-tag-rows-have-text` asks for one per declared language.
icons-sheet-tags-en = Tags (en)
icons-sheet-tags-pt = Tags (pt)
icons-fact-component = Component
icons-fact-categories = Categories
icons-fact-aliases = Also called
icons-fact-family = Family
icons-fact-category = Category
icons-fact-block = Block
icons-fact-decomposition = Decomposition
icons-fact-width = Width
icons-fact-group = Group
icons-fact-subgroup = Subgroup
icons-fact-tone = Skin tone
icons-width-zero = Combining mark — the dotted support is not part of it
icons-width-one = One cell
icons-width-double = Takes two cells
icons-yes = yes
icons-no = no
icons-adjust-size = Size
icons-adjust-stroke = Stroke width
icons-adjust-absolute = Absolute stroke width
icons-adjust-colour = Colour
icons-adjust-cap = Line caps
icons-size-px = { $px } px
icons-cap-round = round
icons-cap-butt = butt
icons-cap-square = square
# The copy formats, per collection (`ronoctua_iconset::copy`).
icons-copy-nerd-fonts-name = Name
icons-copy-nerd-fonts-glyph = Glyph
icons-copy-nerd-fonts-css = CSS class
icons-copy-nerd-fonts-hex = Hexadecimal
icons-copy-nerd-fonts-utf16 = UTF-16
icons-copy-nerd-fonts-escape = Unicode escape
icons-copy-nerd-fonts-html = HTML entity
icons-copy-nerd-fonts-rust = Rust literal
icons-copy-nerd-fonts-token = Design system token
icons-copy-unicode-character = Character
icons-copy-unicode-name = Official name
icons-copy-unicode-hex = Hexadecimal
icons-copy-unicode-decimal = Decimal
icons-copy-unicode-escape = Unicode escape
icons-copy-unicode-html = HTML entity
icons-copy-unicode-rust = Rust literal
icons-copy-unicode-utf16 = UTF-16
icons-copy-unicode-utf8 = UTF-8 bytes
icons-copy-lucide-svg = SVG
icons-copy-lucide-jsx = JSX
icons-copy-lucide-vanilla = HTML (lucide.js)
icons-copy-lucide-react = React
icons-copy-lucide-vue = Vue
icons-copy-lucide-svelte = Svelte
icons-copy-lucide-preact = Preact
icons-copy-lucide-solid = Solid
icons-copy-lucide-angular = Angular
icons-copy-lucide-font = Icon font
icons-copy-lucide-name = Name
icons-copy-lucide-component = Component
icons-copy-lucide-character = Character
icons-copy-lucide-hex = Codepoint
icons-copy-lucide-path = Path data
icons-copy-lucide-uri = Data URI
icons-copy-lucide-base64 = Base64
icons-copy-lucide-css = CSS rule
icons-copy-lucide-sprite = Sprite
