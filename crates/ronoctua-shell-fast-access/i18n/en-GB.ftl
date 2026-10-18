# The texts `ronoctua-shell-fast-access` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

calc-rates-why-unreachable = The European Central Bank could not be reached.

## file info editing — everything the info panel can change

ask-icons-colour = The stroke's colour (a CSS colour, or currentColor)
# THE WORDS OF `ronoctua-words`, for every door that translates or looks a
# word up: why it answered nothing, the parts of speech, who translated it
# and where a meaning came from.
words-why-empty = There is nothing to translate or look up.
words-why-too-long = The text is longer than the translator takes at once.
words-why-unknown-tongue = The translator does not know that language.
words-why-wrong-pass = The permission given is for another site than the one this would ask.
words-why-no-key = The translator needs a key, and none is kept.
words-why-key-refused = The translator refused the key.
words-why-quota = The translator's quota for today is spent.
words-why-paced = Asked too often; it will answer in a moment.
words-why-not-found = The word is not in the dictionary.
words-why-no-dictionary = There is no dictionary of this language on this machine, and asking Wiktionary was not allowed.
words-why-unreachable = The site could not be reached.
words-why-failed = The site answered with a failure.
words-why-garbled = What came back is not what was asked for.
words-part-noun = noun
words-part-proper-noun = proper noun
words-part-verb = verb
words-part-adjective = adjective
words-part-adverb = adverb
words-part-pronoun = pronoun
words-part-preposition = preposition
words-part-conjunction = conjunction
words-part-interjection = interjection
words-part-numeral = numeral
words-part-determiner = determiner
words-part-phrase = phrase
words-part-affix = affix
words-part-other = other
words-provider-mymemory = MyMemory
words-provider-deepl = DeepL
words-provider-libretranslate = LibreTranslate
words-source-wordnet = Open English WordNet, on this machine
words-source-wiktionary = Wiktionary
words-translated-by = Translated by { $provider }
# FAST-ACCESS (`LEADER d`, `crate::fast_access`): the search of everything.
fast-access-title = Search
group-fast-access = search
cmd-fast-access-open = Search everything, in the footer
cmd-fast-access-window = Search everything, in a window
cmd-fast-access-column = Search everything, in the column on the right
cmd-fast-access-float = Search everything, over the whole screen
cmd-fast-access-launch = Open an application by its id
cmd-fast-access-open-path = Open a file with what opens it
cmd-fast-access-reveal = Show a file in the file manager
cmd-fast-access-man = Read a manual page in a terminal
fast-access-placeholder = type to search · { $slash } lists what a search can be narrowed to
fast-access-empty = Your applications, commands, files and what they say, manual pages, programs to install, icons and sums — type a word. A slash and a letter narrow the search.
fast-access-nothing = Nothing found for “{ $words }”.
fast-access-searching = Searching…
fast-access-reading-icons = Reading the icon tables…
fast-access-icons-refused = The icon tables could not be read.
# The scopes a slash and a letter narrow to (`fast_access::Scope`): a noun,
# and what it searches.
fast-access-scope-all = everything
fast-access-scope-audio = audio
fast-access-scope-audio-does = audio files, by name and by what is written in them
fast-access-scope-video = videos
fast-access-scope-video-does = video files on this machine
fast-access-scope-youtube = YouTube
fast-access-scope-youtube-does = your video catalogue, and YouTube itself after a pause
fast-access-scope-images = images
fast-access-scope-images-does = pictures on this machine
fast-access-scope-documents = documents
fast-access-scope-documents-does = documents, books and texts
fast-access-scope-names = names
fast-access-scope-names-does = files and folders by their names only
fast-access-scope-contents = contents
fast-access-scope-contents-does = what is written inside files
fast-access-scope-manuals = manuals
fast-access-scope-manuals-does = the manual pages of the system and of your programs
fast-access-scope-programs = programs
fast-access-scope-programs-does = programs you can install
fast-access-scope-commands = commands
fast-access-scope-commands-does = the commands of this desktop, with their keys
fast-access-scope-web = web
fast-access-scope-web-does = Wikipedia and DuckDuckGo, after a pause
fast-access-scope-dictionary = dictionary
fast-access-scope-dictionary-does = what a word means
fast-access-scope-synonyms = synonyms
fast-access-scope-synonyms-does = other words for a word
fast-access-scope-translate = translate
fast-access-scope-translate-does = a translation — a language code first sends it there ({ $prefix } en …)
fast-access-scope-icons = icons
fast-access-scope-icons-does = Nerd Fonts glyphs, Lucide figures and Unicode characters
fast-access-scope-math = sums
fast-access-scope-math-does = sums, units, bases, dates and zones — “2+2” works without it
fast-access-scope-currency = currencies
fast-access-scope-currency-does = money from one currency into another, at today's rates
fast-access-scope-zones = time zones
fast-access-scope-zones-does = what time it is somewhere else
fast-access-scope-recent = recent
fast-access-scope-recent-does = what you opened from here, the most used first
fast-access-scope-prefixes = narrowing
fast-access-scope-prefixes-does = every way to narrow a search
# The sections of the results (`fast_access::SectionId`).
fast-access-section-prefixes = Narrow the search
fast-access-section-math = Sum
fast-access-section-currency = Currency
fast-access-section-zones = Time zones
fast-access-section-words = Words
fast-access-section-recent = Recent
fast-access-section-apps = Applications
fast-access-section-commands = Commands
fast-access-section-icons = Icons
fast-access-section-files = Files
fast-access-section-folders = Folders
fast-access-section-contents = What files say
fast-access-section-videos = Videos
fast-access-section-youtube = YouTube
fast-access-section-web = The web
fast-access-section-manuals = Manual pages
fast-access-section-packages = Programs to install
fast-access-section-settings = This search
fast-access-strip-nerd-fonts = Nerd Fonts
fast-access-strip-lucide = Lucide
fast-access-strip-unicode = Unicode
fast-access-strip-none = { $collection }: nothing matches
fast-access-strip-about = { $collection } · { $name }
fast-access-strip-earlier = { $collection }: the page before
fast-access-strip-later = { $collection }: the next page
# What the index answered nothing for (`ronoctua_index::Why`).
fast-access-index-why-no-engine = This build has no index.
fast-access-index-why-not-built = The index is not built yet; files show up as it walks your folders.
fast-access-index-why-unreadable = The index could not be read.
fast-access-index-why-other-format = The index is of another version; it is being built again.
fast-access-index-why-busy = The index is busy.
fast-access-index-why-full = The index is full.
fast-access-index-why-failed = The index could not answer that.
# The web (`ronoctua_websearch`).
fast-access-web-wikipedia = Wikipedia
fast-access-web-duckduckgo = DuckDuckGo
fast-access-web-searxng = SearXNG
fast-access-web-brave = Brave Search
fast-access-web-kiosk = Search the web for “{ $words }” in Chrome
fast-access-web-for = A search of the web for what you typed.
fast-access-web-why-empty = There are no words to search for.
fast-access-web-why-wrong-pass = The permission given is for another site.
fast-access-web-why-no-key = Brave Search needs a key, and none is kept.
fast-access-web-why-key-refused = Brave Search refused the key.
fast-access-web-why-paced = Asked too often; it will answer in a moment.
fast-access-web-why-unreachable = The site could not be reached.
fast-access-web-why-failed = The site answered with a failure.
fast-access-web-why-garbled = What came back is not a list of results.
# YouTube (`/y`).
fast-access-youtube-for = A search of YouTube for what you typed.
fast-access-thumbs-for = The pictures of the videos found.
fast-access-youtube-local = in your catalogue
fast-access-youtube-online = on YouTube
fast-access-shorts-shown = Shorts: shown
fast-access-shorts-hidden = Shorts: left out
fast-access-youtube-why-absent = This installation has no yt-dlp.
fast-access-youtube-why-unstartable = yt-dlp could not be started.
fast-access-youtube-why-slow = YouTube took too long to answer.
fast-access-youtube-why-failed = YouTube did not answer the search.
fast-access-youtube-why-garbled = What came back is not a list of videos.
fast-access-youtube-why-too-large = The answer was too large.
fast-access-youtube-why-not-an-address = That is not an address.
fast-access-youtube-why-not-a-list = That is one video, not a list.
fast-access-youtube-why-nothing-to-ask = There are no words to search for.
fast-access-views = { $n ->
        [one] { $n } view
       *[other] { $n } views
    }
# Words (`/d`, `/s`, `/t`).
fast-access-words-for = Looking the word up on Wiktionary.
fast-access-translate-for = A translation of what you typed.
fast-access-translate-into = Into { $tongue }
fast-access-synonyms-none = No other word for it.
# Sums, currencies and zones.
fast-access-math-also = The same, in other units
fast-access-zone-none = No time zone has that name.
# The detail pane (`fast_access_detail`).
fast-access-detail-name = Name
fast-access-detail-address = Address
fast-access-detail-channel = Channel
fast-access-detail-duration = Length
fast-access-detail-views = Seen
fast-access-detail-from = Found
fast-access-detail-keys = Keys
fast-access-detail-command = Command
fast-access-detail-program = Program
fast-access-detail-section = Section
fast-access-detail-description = About it
fast-access-detail-source = From
fast-access-detail-zone = Zone
fast-access-detail-time = Now there
fast-access-detail-scope = Searches
fast-access-detail-short = A Short
fast-access-detail-version = Version
fast-access-detail-licence = Licence
fast-access-detail-site = Site
fast-access-detail-runs = Runs
fast-access-detail-download = Download
fast-access-act-open = Open
fast-access-act-run = Run it
fast-access-act-copy = Copy it
fast-access-act-copy-address = Copy the address
fast-access-act-play-player = Play it in the player
fast-access-act-play-kiosk = Open it in the Chrome kiosk
fast-access-act-install = Install it, through the queue
fast-access-act-centre = Show it in the programs' centre
fast-access-act-terminal = Read it in a terminal
fast-access-act-narrow = Narrow the search to it
fast-access-act-kiosk = Open it in the Chrome kiosk
# The question a web video asks (`fast_access::Question::PlayIn`).
fast-access-play-in = Play “{ $title }” in:
fast-access-play-player = the player
fast-access-play-kiosk = the Chrome kiosk
fast-access-play-remember = and do the same from now on
# The settings of the search, at the foot of the empty page.
fast-access-setting-park = Keep the window ready after it closes
fast-access-setting-door = Web videos play in
fast-access-door-ask = asked each time
fast-access-setting-searxng = Your SearXNG, asked beside the others
fast-access-searxng-none = none
fast-access-searxng-bad = “{ $address }” is not an https address with a host.
fast-access-setting-brave = Your Brave Search key
fast-access-brave-none = none
fast-access-brave-kept-short = kept
fast-access-brave-kept = The Brave Search key is kept, readable by you alone.
fast-access-brave-forgotten = The Brave Search key is forgotten.
fast-access-web-key-no-home = There is no home folder to keep the key in.
fast-access-web-key-empty = Nothing was typed.
fast-access-web-key-malformed = The key has a line break or a control character in it.
fast-access-web-key-unwritable = The key could not be written.
fast-access-yes = yes
fast-access-no = no
fast-access-unkept = The search's settings (fast-access.json) could not be written: { $why }
ask-fast-access-tag = The tag's new value
ask-fast-access-meta = The detail's new value
ask-fast-access-searxng = Your SearXNG's address (https://…), empty for none
ask-fast-access-brave = Your Brave Search key, empty to forget it
fast-access-recent-none = Nothing opened from here yet.
# The keys the header teaches (`fast_access::OF_THE_VIEW`).
fast-access-enter = open
about-fast-access-enter = Does what the row under the cursor is for: opens it, runs it, plays it or copies it — and the search closes, except in the column.
fast-access-secondary = the other way
about-fast-access-secondary = Does the second thing the row offers: shows a file in the file manager, a video in the other player, a program's page.
fast-access-copy = copy
about-fast-access-copy = Copies what the row is: a path, an address, an answer, a character.
fast-access-detail = detail
about-fast-access-detail = Moves the keys into the detail beside the results, and back.
fast-access-whole = whole
about-fast-access-whole = Gives the detail the whole area, and gives it back.
fast-access-shorts = Shorts
about-fast-access-shorts = Shows YouTube's Shorts among the videos found, or leaves them out.
fast-access-clear = clear
about-fast-access-clear = Empties what is typed.
fast-access-page = page
about-fast-access-page = Shows the next icons of a strip — or the ones before.
