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
cmd-video-metrics-page-first = The first page of videos
cmd-video-metrics-page-last = The last page of videos
cmd-video-metrics-layout = Lay the videos out the next way: a list, a list with thumbnails, thumbnails, thumbnails with their channel
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
video-metrics-page-first = first page
video-metrics-page-last = last page
video-metrics-layout = next layout
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
video-metrics-skip-expired = Past their time in the catalogue
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
# THE SAME WHY, ON ITS OWN: said after what registering a list did, which
# already says how many channels went in.
video-metrics-unkept-no-home = There is no home to keep the consent to ask YouTube in: a scan will not ask.
video-metrics-unkept-unparseable = The consent file does not parse, so it was left as it is: a scan will not ask YouTube.
video-metrics-unkept-unwritable = The consent to ask YouTube could not be written: a scan will not ask.
video-metrics-address = Address
video-metrics-handle = Handle
video-metrics-added = Added
video-metrics-reference = Scanned up to
video-metrics-reference-none = not yet
video-metrics-last-error = Last error
video-metrics-its-videos = Its videos
video-metrics-score = Score
about-video-metrics-score = A channel's score, 0 to 10 000, goes into every video of it: each step moves it by a hundred, or it is typed whole.
video-metrics-its-shorts = Its Shorts
video-metrics-as-everywhere = as everywhere
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
# A SCORE AS A BARE NUMBER — a list's, before the arrows of the rating — a
# NUMBER to Fluent all the same, as `points` is.
video-metrics-score-number = { $score }
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
about-video-metrics-points = The points it adds — negative ones take away: each step moves them by a hundred, or they are typed whole.
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
video-metrics-layout-list-thumbnails = list with thumbnails
video-metrics-layout-grid-channel = thumbnails with channel
video-metrics-page = Page
video-metrics-per-page = Per page
video-metrics-page-of = { $page } of { $pages } · { $videos ->
        [one] one video
       *[other] { $videos } videos
    }
video-metrics-by-score = By score
video-metrics-no-videos = No video matches.
video-metrics-video-row = { $channel } · { $age } · { $score }
video-metrics-video-row-short = { $age } · { $score }
video-metrics-video-row-undated = { $channel } · { $score }
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
about-video-metrics-copy = A fact the catalogue keeps; copied, it is on the clipboard, to be pasted anywhere.
video-metrics-yours = Yours
video-metrics-your-tags = Your tags
video-metrics-your-description = Your description
about-video-metrics-second-description = A description of your own, edited here and kept when you say so; a rule can look for its words.
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
# WHAT EVERY ROW IS, under the pointer and in the window's foot (the owner,
# 2026-11-09): a sentence for each thing a click takes (`about_of`, which a
# new row cannot compile without), and what the hand does to it — the door
# says the key or the click, these say what follows (`does_of`). No sentence
# spells a key: the gesture beside the name does.
video-metrics-act-opens = opens it
video-metrics-act-turns = turns it to the next choice
video-metrics-act-switches = switches it
video-metrics-act-types = asks for it to be typed
video-metrics-act-copies = copies it
video-metrics-act-does = does it
video-metrics-act-edits = edits it here
video-metrics-act-asks = asks first, with “no” first
video-metrics-act-answers = gives this answer
about-video-metrics-last-scan = When the last scan ended — or that one runs now, or stopped before its end.
about-video-metrics-scan-added = How many new videos the last scan brought in.
about-video-metrics-scan-failed = How many channels the last scan could not read; each channel's page says why.
about-video-metrics-videos = Every catalogued video, by score, with filters, as a list — with thumbnails or without — or as thumbnails, with their channel or without.
about-video-metrics-channels = The channels you follow, and adding one.
about-video-metrics-rules = How every video is scored, how long videos stay, and whether Shorts are blocked.
about-video-metrics-weights = What each of your ratings adds to a video's score.
about-video-metrics-left-out = Videos the scans found and did not keep, or took out when their time was up, by why; none of them is listed.
about-video-metrics-channel-row = A channel's page: its facts, its score, its Shorts, how long its videos stay, its own rules and its videos.
about-video-metrics-its-videos = The list of videos, only this channel's.
about-video-metrics-channel-shorts = Whether this channel's Shorts are catalogued: as everywhere, blocked, or allowed.
about-video-metrics-remove-channel = Removes the channel and every video of it, after asking.
about-video-metrics-reference = Where the scans stand on this channel: a video published more than a day before it is not taken in.
about-video-metrics-rule-recency = Points a new video gets, fading to nothing over some days.
about-video-metrics-rule-short = Points a Short gets — usually negative.
about-video-metrics-rule-preferred = Points a video gets when its length is in the band you prefer.
about-video-metrics-rule-live = Points a live stream or a premiere gets.
about-video-metrics-rule-band = Points a video gets when its length is in this band.
about-video-metrics-rule-term = Points a video gets when these words are in it.
about-video-metrics-add-band = A new length band; a video in it gets its points.
about-video-metrics-add-term = New words to look for; a video that has them gets their points.
about-video-metrics-block-shorts = Whether Shorts are catalogued at all; a channel may choose otherwise.
about-video-metrics-recency-days = Over how many days a new video's points fade.
about-video-metrics-rule-length = The lengths this rule is about.
about-video-metrics-term-words = The words this rule looks for.
about-video-metrics-term-match = How the words are found: whole, in order, or as beginnings.
about-video-metrics-term-field = Whether the words are looked for in this part of a video.
about-video-metrics-rule-on = Whether this rule counts; off, it adds nothing.
about-video-metrics-remove-rule = Removes this rule, after asking.
about-video-metrics-filter-channel = Every channel's videos, or one channel's.
about-video-metrics-filter-shorts = Shorts and the rest, only Shorts, or no Shorts.
about-video-metrics-filter-rating = Whatever your rating, or only one rating.
about-video-metrics-filter-hidden = Hidden videos left out, shown too, or only them.
about-video-metrics-filter-since = How far back, by when a video was published.
about-video-metrics-layouts = How the videos are laid out: a list, a list with thumbnails, thumbnails, or thumbnails with their channel.
# THE KEYS FIRST, and few words: the footer says it on one line, cut at its end.
about-video-metrics-page = { $earlier } { $prior } go back, { $later } { $next } on, { $first } { $last } to the first and the last page.
about-video-metrics-per-page = How many videos a page shows: each turn gives the next size, and round again. It holds while the view is open.
about-video-metrics-video-entry = The video's page: every field, your rating, tags and description, and what can be done with it.
about-video-metrics-published = When YouTube says it was published.
about-video-metrics-its-points = Its points without its channel's score and recency.
about-video-metrics-rating = Your rating of this video: its weight goes into the score.
about-video-metrics-second-tags = Tags of your own; a rule can look for them.
about-video-metrics-player-row = Puts it in the player's queue; one already there is asked about first.
about-video-metrics-kiosk-row = Its page on YouTube, in a window of Chrome with nothing around it.
about-video-metrics-download-row = Hands it to the transfers' queue, into your downloads.
about-video-metrics-hide-row = Hides it from the lists after asking, or shows it again at once.
about-video-metrics-delete-row = Deletes it for good after asking: no scan brings it back.
about-video-metrics-answer-no = Leaves everything as it is.
about-video-metrics-answer-yes = Does what the question asks.
about-video-metrics-text-line = Your description as it is written; a click puts the caret there.
# WHAT "AS EVERYWHERE" IS NOW, quietly beside the value, and said where the
# row is explained.
video-metrics-default-now = ({ $value })
video-metrics-follows-now = Everywhere, it is { $value } now.
video-metrics-follows-rule-everywhere = This is the rule everywhere: its page changes it on every channel.
# HOW LONG A VIDEO STAYS, WHAT IS SET ASIDE, AND THE GLYPHS of a video in
# the lists (the owner, 2026-11-09): a video goes when its time is up — the
# catalogue's, or its channel's own — unless it is set aside, or marked never
# to be deleted; the four arrows of its rating after its score, and its marks
# at the line's end, each acting on the first click. The forms and the hint
# words of the keys `s` and `f` are the view's (`OF_THE_VIEW`).
cmd-video-metrics-set-aside = Set it aside for later, or take it out of the set-aside videos
cmd-video-metrics-keep = Never delete it, or let it go when its time is up
cmd-video-metrics-keep-days = Type after how many days videos are deleted
video-metrics-set-aside = set aside
video-metrics-keep = keep
ask-video-metrics-keep-days = Deleted after how many days
video-metrics-set-aside-videos = Set-aside videos
video-metrics-filter-set-aside = Set aside
video-metrics-set-aside-any = set aside or not
video-metrics-set-aside-only = only those, newest first
video-metrics-set-aside-without = not set aside
video-metrics-by-set-aside = Newest set aside first
video-metrics-heading-keep = How long videos stay
video-metrics-keep-after = Deleted after
video-metrics-keep-never = never deleted
video-metrics-channel-keep = Its videos are deleted after
video-metrics-set-aside-row = Set aside for later
video-metrics-never-delete = Never delete it
video-metrics-goes-on = goes on { $date }
video-metrics-goes-unknown = goes when its time is up
video-metrics-kept-by-channel = its channel keeps every video
video-metrics-kept-set-aside = kept while set aside
video-metrics-kept-everywhere = kept for ever, as everywhere
video-metrics-kept-for-ever = kept for ever
video-metrics-without-the-mark = Without the mark: { $value }.
video-metrics-once-out = Out of the set-aside videos: { $value }.
# WHAT THE NEXT SCAN TAKES OUT, under a keep — quietly, so a small number
# typed by mistake surprises nobody. Said only when it is one or more.
video-metrics-expiring = { $count ->
        [one] One video leaves the catalogue at the next scan: its time is up.
       *[other] { $count } videos leave the catalogue at the next scan: their time is up.
    }
video-metrics-typed-not-keep-days = That is not a number of days from one to { $most }; past that, the ring has “{ $never }”.
video-metrics-act-picks = gives it this rating
video-metrics-act-unpicks = takes its rating away
about-video-metrics-set-aside-videos = The videos you set aside, newest first; while set aside, a video is never deleted.
about-video-metrics-filter-set-aside = Set-aside videos and the rest, only them (newest first), or none of them.
about-video-metrics-keep-everywhere = How long a video stays, counted from when it was published, before it is deleted — or for ever. Set-aside videos, and the ones marked never to be deleted, stay.
about-video-metrics-channel-keep = How long this channel's videos stay before they are deleted: as everywhere, days of its own, or for ever.
about-video-metrics-set-aside-row = Whether it is among the set-aside videos; while it is, it is never deleted.
about-video-metrics-never-delete = Whether it is kept for ever, whatever its channel and the catalogue keep.
about-video-metrics-glyph-rate = Rates the video so: the rating's weight goes into its score.
about-video-metrics-glyph-rated = The video's rating now; taken away, its weight leaves the score.
about-video-metrics-glyph-set-aside = Sets it aside: it joins the set-aside videos, and is never deleted while there.
about-video-metrics-glyph-take-out = Set aside: never deleted while it is; switched off, it leaves the set-aside videos.
about-video-metrics-glyph-forever-no = Goes when its time is up; marked, it is kept for ever.
about-video-metrics-glyph-forever-video = Kept for ever by your mark, whatever its channel and the catalogue keep.
about-video-metrics-glyph-forever-channel = Kept for ever, as every video of its channel; your mark keeps it if the channel changes.
about-video-metrics-glyph-forever-everywhere = Kept for ever, as videos are everywhere; your mark keeps it if that changes.
about-video-metrics-glyph-forever-aside = Kept while set aside; marked, it is kept for ever, set aside or not.
# THE SAME CATALOGUE IN OTHER APPS (the owner, 2026-11-09): the brain's page
# on its scan, the player's two lists of it, and a video — or a channel's
# videos — in Nyxt. The hint word and the form of the key `n` are the view's
# (`OF_THE_VIEW`).
cmd-video-metrics-nyxt = Open it in Nyxt
video-metrics-nyxt = Nyxt
video-metrics-heading-elsewhere = In other apps
video-metrics-in-the-brain = The scan's schedule, in the brain
video-metrics-in-the-player = Videos by score, in the player
video-metrics-set-aside-in-the-player = Set-aside videos, in the player
video-metrics-channel-in-chrome = Its videos in Chrome
video-metrics-channel-in-nyxt = Its videos in Nyxt
about-video-metrics-in-the-brain = The brain's page for this catalogue's scan: when it runs next, what it waits for, what it did.
about-video-metrics-in-the-player = A new window of the player, on its list of these videos by score.
about-video-metrics-set-aside-in-the-player = A new window of the player, on the videos you set aside.
about-video-metrics-nyxt-row = Its page on YouTube, in Nyxt.
about-video-metrics-channel-in-chrome = Its videos' page on YouTube, newest first, in a window of Chrome with nothing around it.
about-video-metrics-channel-in-nyxt = Its videos' page on YouTube, newest first, in Nyxt.
# A LIST OF SUBSCRIPTIONS, IMPORTED AND EXPORTED (the owner, 2026-11-09):
# Google Takeout's list of the channels an account subscribes to, each
# channel staged for the person to keep or discard before the list is
# registered — and the channels here saved as a list of the same shape. No
# sentence spells a key: the gesture beside the name does.
video-metrics-import-export = Import or export a list
about-video-metrics-import-export = Brings in the channels a YouTube account subscribes to, from Google Takeout's list, and saves the channels here as a list of the same shape.
video-metrics-unregistered = { $count ->
        [one] one not registered yet
       *[other] { $count } not registered yet
    }
video-metrics-heading-import = From YouTube's subscriptions
video-metrics-import-note = Google Takeout exports the channels an account subscribes to: choose YouTube, then its subscriptions, and download the list. Choose it here, and each channel waits for you to keep or discard it.
video-metrics-takeout = Google Takeout
about-video-metrics-takeout = Opens Google Takeout in Chrome, where the list of subscriptions is exported.
video-metrics-choose-list = Choose the list…
about-video-metrics-choose-list = Opens the file picker in your downloads, showing the lists of channels.
video-metrics-pick-import = The list of subscriptions
video-metrics-filter-csv = Lists of channels (.csv)
about-video-metrics-pick = The file picker, over the page: the folder a list is read from or written to, and the file.
video-metrics-act-takes = takes it
video-metrics-import-left-out = Lines left out
about-video-metrics-import-left-out = Lines of the file whose first column is not a YouTube channel's id: nothing of them was taken.
video-metrics-heading-already = Already in the catalogue
video-metrics-heading-to-decide = To decide
video-metrics-decisions = So far
video-metrics-decisions-value = { $keep } kept · { $discard } discarded · { $undecided } undecided
about-video-metrics-decisions = How many channels of the list are kept, discarded and still undecided.
video-metrics-decision = Decision
about-video-metrics-decision = Whether this channel joins the catalogue with the others you keep, or is dropped, when the list is registered.
video-metrics-decision-undecided = undecided
video-metrics-decision-keep = kept
video-metrics-decision-discard = discarded
video-metrics-decision-mixed = mixed
video-metrics-decide-all = All of them
about-video-metrics-decide-all = Gives every channel still to register the same decision: undecided, kept or discarded. While their decisions differ, asked first.
# EVERY CHANNEL GIVEN ONE DECISION while theirs differ: what each was given
# goes, so it is asked — one sentence per decision, the count being the
# channels whose decision changes (never zero: theirs differ).
ask-video-metrics-decide-all-undecided = Make every channel of the list undecided? { $changed ->
        [one] One channel loses its decision.
       *[other] { $changed } channels lose their decision.
    }
ask-video-metrics-decide-all-keep = Mark every channel of the list kept? { $changed ->
        [one] The decision of one channel changes.
       *[other] The decision of { $changed } channels changes.
    }
ask-video-metrics-decide-all-discard = Mark every channel of the list discarded? { $changed ->
        [one] The decision of one channel changes.
       *[other] The decision of { $changed } channels changes.
    }
video-metrics-leave-decisions = No, leave them as they are
video-metrics-decide-them-all = Yes, change them all
about-video-metrics-staged = A channel of the list, not registered yet: kept or discarded when the list is registered. Its page sets its score, its Shorts, how long its videos stay and its rules before that.
video-metrics-staged-note = Not registered yet: nothing of it is scanned until it is.
video-metrics-register = Register the kept
about-video-metrics-register = Registers the channels marked kept and drops the discarded ones; the undecided stay on this page. Asked first.
ask-video-metrics-register = Register { $kept ->
        [0] no channel
        [one] one channel
       *[other] { $kept } channels
    } and drop { $discarded ->
        [0] none
        [one] one
       *[other] { $discarded }
    }? The undecided stay here.
video-metrics-not-yet = No, not yet
video-metrics-register-them = Yes, register them
video-metrics-registered = { $kept ->
        [0] No channel registered
        [one] One channel registered
       *[other] { $kept } channels registered
    }; { $discarded ->
        [0] none dropped
        [one] one dropped
       *[other] { $discarded } dropped
    }.
video-metrics-imported = { $staged ->
        [0] No channel
        [one] One channel
       *[other] { $staged } channels
    } to decide; { $already } already in the catalogue.
video-metrics-import-unreadable = The file could not be read.
video-metrics-import-too-large = The file is too large to be a list of subscriptions.
video-metrics-import-not-a-list = No line of the file names a YouTube channel: it is not a list of subscriptions.
video-metrics-import-not-text = The file is not text.
video-metrics-import-unclosed = A quote opened on line { $line } is never closed.
video-metrics-heading-export = Export
video-metrics-save-list = Save the channels as a list…
about-video-metrics-save-list = Saves the channels registered here as a list in Google's shape, which this page and Takeout's readers take.
video-metrics-pick-export = Where the list of channels goes
video-metrics-exported = { $count ->
        [0] No channel saved
        [one] One channel saved
       *[other] { $count } channels saved
    } in { $file }.
video-metrics-export-failed = The list could not be written there.
player-ranking-reading = Reading the video catalogue…
# How many videos a page of the ranking holds — not the catalogue listing's
# "Per page", which is another page of the same view.
player-ranking-page-size = Page size
player-ranking-none = Nothing is ranked yet: the video catalogue ({ $keys }) adds your channels, and its scans fill this list.
player-ranking-empty = No video ranks yet: the scans found none, or every one is hidden.
# A LIST WITH NOTHING ON IT, said for what it is: its Shorts turned away
# — the filter its page was read under — or nothing set aside yet.
player-ranking-empty-filtered = No video ranks with Shorts as “{ $shorts }”: the scans found none, or the Shorts row leaves them out.
player-ranking-aside-empty = Nothing is set aside yet: the bookmark beside a video sets it aside.
player-ranking-aside-empty-filtered = Nothing set aside with Shorts as “{ $shorts }”: nothing is set aside yet, or the Shorts row leaves out what is.
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
player-ranking-not-changed = { $title } was not changed. { $why }
player-ranking-downloading = Downloading { $title }; { $keys } shows the transfers.
player-ranking-downloading-plain = Downloading { $title }.
player-ranking-not-downloaded = The transfer queue did not take { $title }: it is not running.
# A MARK put on a video from the ranking, or taken off. Its rating's name is
# the video catalogue's own word for it.
player-ranking-rated = Your rating of { $title }: { $rating }
player-ranking-unrated = { $title } is no longer rated
player-ranking-now-set-aside = Set aside: { $title }
player-ranking-no-longer-set-aside = Out of the set-aside list: { $title }
player-ranking-now-forever = Never deleted: { $title }, by its own mark — which holds whatever its channel keeps, and if the channel changes
player-ranking-no-longer-forever = { $title }: its own never-delete mark is off — its channel's time, or the catalogue's, applies again
# A MARK PRESSED while the last one on the same video is still being written:
# two writes on their own threads land in either order.
player-ranking-still-marking = { $title }: its last mark is still being written; try again in a moment.
player-ranking-shown-again = Back in the ranking: { $title }
