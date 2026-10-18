# The texts `ronoctua-shell-video-metrics` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# THE VIDEO CATALOGUE (LEADER v, video_metrics.rs and its window).
group-video-metrics = videos
video-metrics-title = Video metrics
cmd-video-metrics-open = Video metrics: your YouTube channels, their videos scored
cmd-video-metrics-window = Video metrics, in a window
cmd-video-metrics-scan = Scan every channel for new videos now
cmd-video-metrics-up = Previous row
cmd-video-metrics-down = Next row
cmd-video-metrics-less = The value under the cursor down a step, or the video before on the grid
cmd-video-metrics-more = The value under the cursor up a step, or the video after on the grid
cmd-video-metrics-enter = Open or change what is under the cursor
cmd-video-metrics-page-back = The page of videos before
cmd-video-metrics-page-on = The page of videos after
cmd-video-metrics-layout = The videos as a list, or as a grid of thumbnails
cmd-video-metrics-search = Search the videos' words — titles, descriptions, tags and yours
cmd-video-metrics-player = Open it in the player
cmd-video-metrics-kiosk = Open it in Chrome, as a kiosk
cmd-video-metrics-download = Download it, through the transfers' queue
cmd-video-metrics-hide = Hide it from the lists, or show it again
cmd-video-metrics-delete = Delete it for good: it never comes back
cmd-video-metrics-add-channel = Add a YouTube channel, by its address or its @handle
cmd-video-metrics-channel-score = Type a channel's score
cmd-video-metrics-points = Type a rule's points
cmd-video-metrics-days = Type over how many days recency fades
cmd-video-metrics-band = Type a length band
cmd-video-metrics-words = Type the words a rule looks for
cmd-video-metrics-second-tags = Type your tags of a video
cmd-video-metrics-weight = Type a rating's weight
video-metrics-up = up
video-metrics-down = down
video-metrics-less = less
video-metrics-more = more
video-metrics-enter = open
video-metrics-page-back = page back
video-metrics-page-on = page on
video-metrics-layout = list or grid
video-metrics-search = search
video-metrics-player = player
video-metrics-kiosk = Chrome
video-metrics-download = download
video-metrics-hide = hide
video-metrics-delete = delete
ask-video-metrics-add-channel = Channel's address or @handle
ask-video-metrics-channel-score = Score, 0 to 10 000
ask-video-metrics-points = Points
ask-video-metrics-days = Fades over how many days
ask-video-metrics-band = From how long to how long (4:00-20:00)
ask-video-metrics-words = Words to look for
ask-video-metrics-search = Search the videos for
ask-video-metrics-second-tags = Your tags, separated by commas
ask-video-metrics-weight = Points for this rating
video-metrics-heading-scan = Scanning
video-metrics-last-scan = Last scan
video-metrics-never = never
video-metrics-scan-running = scanning since { $when }
video-metrics-scan-cut = stopped before it ended ({ $when })
video-metrics-scan-added = New videos
video-metrics-scan-failed = Channels that failed
video-metrics-failed-of = { $failed } of { $channels }
video-metrics-pending = { $pending ->
        [one] One video waits for its details: the next scan asks for it.
       *[other] { $pending } videos wait for their details: the next scans ask for them, a few at a time.
    }
video-metrics-scan-now = Scan now
video-metrics-ask-youtube = Ask YouTube
about-video-metrics-ask-youtube = Whether a scan may ask YouTube: switched off, nothing is asked until you switch it on again.
video-metrics-ask-youtube-note = Adding a channel lets { $hosts } be asked — its feeds and thumbnails, nothing else. The permission is this machine's, address by address: off here, nothing on it asks them, the player's YouTube sources included.
video-metrics-on = on
video-metrics-off = off
video-metrics-heading-catalogue = The catalogue
video-metrics-videos = Videos
video-metrics-channels = Channels
video-metrics-rules = Rules
video-metrics-weights = Rating weights
video-metrics-heading-left-out = Left out
video-metrics-skip-before-reference = Older than the channel's last scan
video-metrics-skip-first-scan = Beyond a channel's first scan
video-metrics-skip-blocked-short = Shorts, while they are blocked
video-metrics-skip-unavailable = Private, removed or gone
video-metrics-skip-deleted = Deleted by you
video-metrics-reading = Reading the catalogue…
video-metrics-why-not-yet = This is not answered yet.
video-metrics-why-no-database = This build has no video catalogue.
video-metrics-why-not-there = Nothing is catalogued yet: add a channel, and the next scan fills it.
video-metrics-why-unreadable = The video catalogue is there and cannot be read.
video-metrics-why-other-schema = The video catalogue was written by a newer version; it is left as it is.
video-metrics-why-busy = A scan holds the catalogue right now; try again in a moment.
video-metrics-why-not-found = It is not in the catalogue any more.
video-metrics-why-failed = The catalogue could not be written.
video-metrics-why-invalid = The catalogue cannot keep that: a rule needs words or a band that ends after it starts, recency needs at least a day and no negative points.
video-metrics-stop-bot-check = YouTube asked whether this is a robot, so the scan stopped; the next one tries again.
video-metrics-stop-too-many-requests = YouTube said too many requests, so the scan stopped; the next one tries again.
video-metrics-stop-offline = There was no network, so the scan stopped.
video-metrics-stop-not-let = Asking YouTube is switched off, so the scan asked nothing.
video-metrics-add-a-channel = Add a channel…
video-metrics-no-channels = No channel yet: add one by its address or its @handle.
video-metrics-channel-row = { $score } · { $videos ->
        [one] one video
       *[other] { $videos } videos
    } · scanned { $when }
video-metrics-channel-added = { $name } was added: its newest videos arrive at the next scan.
video-metrics-channel-removed = The channel and its videos were removed.
video-metrics-add-absent = This installation has no yt-dlp, which says whose channel an address is.
video-metrics-add-unstartable = yt-dlp could not be started.
video-metrics-add-slow = yt-dlp took too long to say whose channel it is.
video-metrics-add-failed = yt-dlp could not say whose channel it is.
video-metrics-add-garbled = yt-dlp answered something that names no channel.
video-metrics-add-not-an-address = That is no YouTube address, @handle or channel id.
video-metrics-add-gone = YouTube has no channel there.
video-metrics-not-let-no-home = The channel is in, and there is no home to keep the consent to ask YouTube in: a scan will not ask.
video-metrics-not-let-unparseable = The channel is in, and the consent file does not parse, so it was left as it is: a scan will not ask YouTube.
video-metrics-not-let-unwritable = The channel is in, and the consent to ask YouTube could not be written: a scan will not ask.
video-metrics-address = Address
video-metrics-handle = Handle
video-metrics-added = Added
video-metrics-reference = Scanned up to
video-metrics-reference-none = not yet
video-metrics-last-error = Last error
video-metrics-its-videos = Its videos
video-metrics-score = Score
about-video-metrics-score = A channel's score, 0 to 10 000, goes into every video of it: the arrows move it by a hundred, and RET types it.
video-metrics-its-shorts = Its Shorts
video-metrics-shorts-follow = as everywhere
video-metrics-shorts-blocked = blocked
video-metrics-shorts-allowed = allowed
video-metrics-its-rules = Its rules
video-metrics-heading-remove = Removing
video-metrics-remove-channel = Remove this channel
ask-video-metrics-remove-channel = Remove { $name }? Its videos go with it, and none of them comes back.
video-metrics-keep-channel = No, keep the channel
video-metrics-remove-channel-it = Yes, remove it and its videos
video-metrics-block-shorts = Block YouTube Shorts
video-metrics-block-shorts-note = Blocked Shorts are never catalogued, and those already in are hidden; a channel may choose otherwise on its own page.
video-metrics-heading-score = How a video is scored
video-metrics-rule-term = Words to look for
video-metrics-rule-band = Length band
video-metrics-rule-preferred = Preferred length
video-metrics-rule-short = Short penalty
video-metrics-rule-recency = Recency
video-metrics-rule-live = Live or premiere
video-metrics-none = none
video-metrics-points-value = { $points ->
        [one] { $points } point
       *[other] { $points } points
    }
video-metrics-days-value = { $days ->
        [one] one day
       *[other] { $days } days
    }
video-metrics-recency-value = { $points }, gone in { $days }
video-metrics-rule-in = { $value } · { $scope }
video-metrics-scope-own = this channel's
video-metrics-scope-everywhere = everywhere
video-metrics-band-value = { $from } to { $to }
video-metrics-band-joiner = to
video-metrics-band-row = Length { $band }
video-metrics-preferred-row = Preferred length: { $band }
video-metrics-term-row = “{ $words }”
video-metrics-add-band = Add a length band…
video-metrics-add-band-here = Add a length band for this channel…
video-metrics-bands-note = These bands are the ones everywhere; a band added here starts this channel's own list, and the ones everywhere stop counting for it.
video-metrics-add-term = Add words…
video-metrics-add-term-here = Add words for this channel…
video-metrics-rule-inherited-note = This is the rule everywhere; changing it here makes one for { $name } alone.
video-metrics-rule-own-note = This is { $name }'s own rule: it counts instead of the one everywhere.
video-metrics-rule-own-off-note = This is { $name }'s own rule, switched off: nothing of this kind counts for { $name }, not even the rule everywhere. Removing it brings the one everywhere back.
video-metrics-recency-points = Points today
video-metrics-recency-days = Fades over
video-metrics-rule-points = Points
about-video-metrics-points = The points it adds — negative ones take away: the arrows move them by a hundred, and RET types them.
video-metrics-rule-length = Length
video-metrics-term-words = Words
video-metrics-term-match = Match
video-metrics-match-word = whole words
video-metrics-match-phrase = the words in order
video-metrics-match-prefix = words that start so
video-metrics-field-title = In the title
video-metrics-field-description = In the description
video-metrics-field-tags = In the tags
video-metrics-field-second-description = In your description
video-metrics-field-second-tags = In your tags
video-metrics-rule-on = In force
video-metrics-remove-rule = Remove this rule
ask-video-metrics-remove-rule = Remove this rule? It cannot be brought back.
video-metrics-keep-rule = No, keep the rule
video-metrics-remove-rule-it = Yes, remove the rule
video-metrics-weights-note = What a rating of yours adds to a video's score, on every channel.
video-metrics-rating-very-negative = Very negative
video-metrics-rating-negative = Negative
video-metrics-rating-positive = Positive
video-metrics-rating-very-positive = Very positive
video-metrics-channel = Channel
video-metrics-every-channel = every channel
video-metrics-filter-shorts = Shorts
video-metrics-shorts-any = with and without
video-metrics-shorts-only = only Shorts
video-metrics-shorts-without = without Shorts
video-metrics-rating = Your rating
video-metrics-rating-any = any
video-metrics-rating-none = not rated
video-metrics-filter-hidden = Hidden
video-metrics-hidden-without = not shown
video-metrics-hidden-only = only hidden
video-metrics-hidden-any = shown too
video-metrics-filter-text = Search
video-metrics-search-none = nothing
video-metrics-published = Published
video-metrics-since-any = any time
video-metrics-since-day = the last day
video-metrics-since-week = the last week
video-metrics-since-month = the last 30 days
video-metrics-since-year = the last year
video-metrics-filter-layout = Layout
video-metrics-layout-list = list
video-metrics-layout-grid = thumbnails
video-metrics-page = Page
video-metrics-page-of = { $page } of { $pages } · { $videos ->
        [one] one video
       *[other] { $videos } videos
    }
video-metrics-by-score = By score
video-metrics-no-videos = No video matches.
video-metrics-video-row = { $score } · { $channel } · { $age }
video-metrics-age = { $days ->
        [0] today
        [1] yesterday
       *[other] { $days } days ago
    }
video-metrics-the-video = The video
video-metrics-duration = Duration
video-metrics-kind = Kind
video-metrics-kind-short = a Short
video-metrics-kind-long = a video
video-metrics-kind-live = live or a premiere
video-metrics-its-points = Its own points
video-metrics-size = Picture
video-metrics-size-value = { $width } × { $height }
video-metrics-languages = Languages
video-metrics-age-limit = Age limit
video-metrics-captions = Captions
video-metrics-categories = Categories
video-metrics-chapters = Chapters
video-metrics-views = Views
video-metrics-first-seen = First seen
video-metrics-tags = YouTube's tags
video-metrics-description = YouTube's description
about-video-metrics-copy = RET puts this value on the clipboard.
video-metrics-yours = Yours
video-metrics-your-tags = Your tags
video-metrics-your-description = Your description
about-video-metrics-second-description = A description of your own, edited here: C-s keeps it, and a rule can look for its words.
video-metrics-description-kept = Your description was kept.
video-metrics-with-it = With it
video-metrics-hide-row = In the lists
video-metrics-shown = shown
video-metrics-hidden = hidden
ask-video-metrics-hide = Hide { $title }? It stays in the catalogue, and "Hidden: shown too" finds it.
video-metrics-leave-shown = No, leave it shown
video-metrics-hide-it = Yes, hide it
ask-video-metrics-delete = Delete { $title } for good? It never comes back, not even at the next scan.
video-metrics-keep-video = No, keep the video
video-metrics-delete-it = Yes, delete it for good
video-metrics-deleted = The video was deleted, and never comes back.
ask-video-metrics-again = It is already in the queue as { $name }. Add it again?
video-metrics-not-again = No, once is enough
video-metrics-again = Yes, add it again
video-metrics-player-unreachable = The player did not answer; nothing was added.
video-metrics-downloading = Downloading { $title }: the queue shows it ({ $keys }).
video-metrics-scan-asked = A scan was asked for; the modeline marks it while it runs.
video-metrics-scan-started = The brain did not take it, so the scan was started directly.
video-metrics-scan-nowhere = Nothing here can start a scan: neither the brain nor systemd answered.
video-metrics-scan-saver = The power saver is on, and no scan starts while it is: nothing was asked.
video-metrics-scan-other-instance = This is a second copy of the system: its brain only rehearses, and the scan unit is the installed session's, so nothing was asked here.
video-metrics-typed-not-a-number = That is not a whole number.
video-metrics-typed-not-days = That is not a whole number of days, one or more.
video-metrics-typed-not-a-band = Write a band as two lengths, like 4-20 or 0:30-3:00.
video-metrics-typed-no-words = There are no words in that to look for.
video-metrics-typed-no-row = There is no row here for that: put the cursor on what it changes, and type it there.
player-ranking-reading = Reading the video catalogue…
player-ranking-none = Nothing is ranked yet: the video catalogue ({ $keys }) adds your channels, and its scans fill this list.
player-ranking-empty = No video ranks yet: the scans found none, or every one is hidden.
player-ranking-layout-grid = Thumbnails
player-ranking-added = Added: { $title }
player-ranking-title = Video title
player-ranking-channel = Channel
player-ranking-score = Score
player-ranking-length = Running time
player-ranking-published = Published
player-ranking-actions = With this video
ask-player-ranking-delete = Delete “{ $title }” from the video catalogue for good? It never comes back, not even in a scan.
player-ranking-hidden = Hidden: { $title }. The video catalogue's hidden videos bring it back.
player-ranking-deleted = Deleted for good: { $title }
player-ranking-not-changed = { $title } was not changed: { $why }
player-ranking-downloading = Downloading { $title }; { $keys } shows the transfers.
player-ranking-downloading-plain = Downloading { $title }.
player-ranking-not-downloaded = The transfer queue did not take { $title }: it is not running.
