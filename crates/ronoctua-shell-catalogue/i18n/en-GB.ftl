# The texts `ronoctua-shell-catalogue` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# CHANNELS BY THEME, on the local catalogue (`player_catalog`): one source at
# a time, its themes and categories, and the listing a row of them opens.
player-about-catalog = Every source's channels, films and videos, read into this machine and filtered in an instant. A source is read again when it is shown past its freshness.
player-catalog-source = Channels from
player-catalog-ingesting = Reading this source…
player-catalog-stale = Read { $date }; due to be read again.
player-catalog-never = Not read yet.
player-catalog-read-again = Read this source again
player-catalog-reading-now = Reading { $name }…
player-catalog-search = Search by title
player-catalog-searched = “{ $words }”
player-catalog-searching = Searching the archive for “{ $words }”…
player-catalog-search-archive = Search the whole archive for “{ $words }”
player-catalog-themes = Themes
player-catalog-categories = Categories
player-catalog-everything = Everything
player-catalog-ungrouped = No category
player-catalog-empty = Nothing here yet.
player-catalog-nothing-found = Nothing matches.
player-catalog-adult = Adults only
player-catalog-letter = Letter
player-catalog-letter-all = All
player-catalog-page-not-a-number = “{ $said }” is not a page number.
player-catalog-per-page = Per page
player-catalog-sort = Order
player-catalog-sort-title = By title
player-catalog-sort-popular = Most watched first
player-catalog-sort-newest = Newest first
player-catalog-sort-oldest = Oldest first
player-catalog-country = Country
player-catalog-country-all = Every country
player-catalog-language = Language
player-catalog-language-all = Every language
player-catalog-quality = Picture height
player-catalog-quality-any = Any height
player-catalog-quality-lines = { $lines } lines or more
player-catalog-layout-lines = Two lines
player-catalog-layout-columns = Columns
player-catalog-layout-posters = Posters
player-catalog-layout-big = Large posters
player-catalog-scope-in = Search in
player-catalog-scope-category = This list
player-catalog-scope-every-category = All of the source
player-catalog-scope-from = From
player-catalog-scope-source = This source
player-catalog-scope-every-source = Every source
player-catalog-kind-live = live
player-catalog-kind-audio = audio
player-catalog-item-gone = It is no longer in the catalogue.
ask-player-catalog-letter = Letter
ask-player-catalog-page = Page number
ask-player-catalog-search = Titles with
# THE ADULT CATALOGUE, behind the person's own system password.
player-adult-locked = Locked: your system password opens it
player-adult-checking = checking…
ask-player-adult-password = Your system password
player-adult-lock = Lock it now
player-adult-locked-again = The adult catalogue is locked.
player-adult-shut = Locked
player-adult-honest = A lock on these doors, not encryption: its list is a file on this disk like any other.
# THE CATALOGUE'S SOURCES (`player_sources`).
player-about-sources = Paste the link to an IPTV list, a podcast or video feed, a YouTube channel or playlist, or an archive.org collection. Its address stays private on this machine, and is shown by its host only.
player-source-add = Add a source…
ask-player-source-url = Link to a list, a feed, a channel or a collection
player-source-add-adult = Add a source to the adult catalogue…
ask-player-adult-source-url = Link to a list for the adult catalogue
player-source-adding = Reading { $name }…
player-source-address = Address
player-source-plain = Not encrypted: this address is http, and what it answers — the list, the account in its address, and the posters it keeps on that same site — crosses the network in the clear.
player-source-kind = Kind
player-source-kind-iptv-org = iptv-org's TV index
player-source-kind-archive = Internet Archive collections
player-source-kind-youtube-lives = YouTube searches for lives
player-source-kind-nasa = NASA's library
player-source-kind-commons = Wikimedia Commons categories
player-source-kind-peertube = PeerTube videos
player-source-kind-playlist = IPTV list (M3U)
player-source-kind-feed = Feed (RSS or Atom)
player-source-kind-youtube = YouTube channel or playlist
player-source-kind-collection = archive.org collection
player-source-posters = Fetch its posters
player-source-hosts = from { $hosts }
player-source-hosts-any = from wherever its list says
player-source-posters-on = Its posters will be fetched.
player-source-posters-off = Its posters will not be fetched.
player-source-adult = In the adult catalogue
player-source-adult-on = It moves to the adult catalogue.
player-source-adult-off = It leaves the adult catalogue.
player-source-adult-hidden = Sources of the adult catalogue
player-source-remove = Remove…
ask-player-source-remove = Remove { $name } and everything it brought?
player-source-removed = { $name } was removed.
