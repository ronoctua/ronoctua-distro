# The texts `ronoctua-shell-player` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Installer — the interface

echo-argument-refused = { $command } does not take “{ $argument }”
globe-address-near = near { $place }, about { $km } km away
globe-behind = this place is on the far side of the globe
# =====================================================================
# THE PLAYER — `LEADER p`, the column on the right, the modeline's mark
# =====================================================================
group-player = player
player-title = Player
cmd-player-open = The player, in the footer
cmd-player-window = The player, in a window
cmd-player-column = The player's column, on this workspace (or close it)
cmd-player-swap = Swap window and column
cmd-player-quit = Close the player everywhere: what plays stops, and every window, column and view of it closes
cmd-player-play-file = Play a file or an address now; the queue stays
cmd-player-enqueue = Add a file or an address to the queue
cmd-player-together = Play something else together with what plays: a picture, a sound or subtitles
cmd-player-toggle = Play or pause
cmd-player-next = The next in the queue
cmd-player-previous = The previous in the queue
cmd-player-stop = Stop
cmd-player-shuffle = Shuffle the queue, or stop shuffling
cmd-player-repeat = Repeat nothing, the queue, or this one
cmd-player-up = Previous row
cmd-player-down = Next row
cmd-player-less = The value back a step
cmd-player-more = The value on a step
cmd-player-enter = What a click on the row does
cmd-player-remove = Take it off the queue, or off what plays together
cmd-player-browse = Choose files for the queue: music, videos, anything the player plays
cmd-player-add = Put what the row plays in the queue — a station, a channel, a film
cmd-player-play-now = Play what the row plays now — right after what plays; the queue stays
cmd-player-clear = Take everything out of the queue
cmd-player-quieter = The volume down a step
cmd-player-louder = The volume up a step
cmd-player-only = Only what plays — the video or the cover — in the region, the window or the column
cmd-player-maximize = What plays, on the whole screen — and back
cmd-player-pip = What plays, in a small window in the corner, over the others — and back
cmd-player-mute = Mute the player, or give its sound back
cmd-player-tracks = Choose among the media in the file: its sounds and subtitles
cmd-player-fold = Fold or unfold the playlist or the set under the cursor
cmd-player-earlier = Move the entry under the cursor — a file, a playlist, a set — one place earlier in its list
cmd-player-later = Move the entry under the cursor — a file, a playlist, a set — one place later in its list
cmd-player-also = Play this one of the file's media too, beside the one in use: a lane of its own
cmd-player-rename = Another name for the entry under the cursor, or for what plays
cmd-player-save-set = Save what plays as a set (.rset): its files and how they play together
cmd-player-save-queue = Save the queue as a file (.rqueue)
cmd-player-subtitles = The subtitles over the picture
cmd-player-lane-shown = Switch the lane under the cursor on or off: a subtitle shows or hides, a picture or a sound plays or stops
cmd-player-ambience = The ambient sounds
cmd-player-ambience-silence = Silence every ambient sound
cmd-player-timeline = Every lane of what plays, on one clock
cmd-player-layout = How the pictures of what plays share a screen
cmd-player-metadata = The metadata of what plays: its tags, its cover and its streams
cmd-player-suggestions = Tags for the file that plays, suggested from the internet
cmd-player-find-subtitles = Subtitles for the film that plays, from the internet
cmd-player-find-lyrics = Lyrics for the song that plays, from the internet
cmd-player-internet = What the player asks of the internet, and your keys
cmd-player-every-screen = What plays on the whole of every screen it goes to; again, and those close
cmd-player-radio = World radio: the stations of every country
cmd-player-tv = World TV: the channels of every country
cmd-player-channels = Channels by theme: live TV, films and lives
cmd-player-search = Search this page's list by name
cmd-player-favourite = Keep what is under the cursor among the favourites, or take it off
cmd-player-nearer = Bring the globe nearer
cmd-player-further = Take the globe further away
cmd-player-world = The globe back as the page frames it: the world, or the country
player-up = up
player-down = down
player-less = less
player-more = more
player-enter = choose
player-stop = stop
player-previous = previous
player-remove = remove
player-browse = files
player-add = add to the queue
player-play-now = play now
player-clear = clear the queue
player-quieter = quieter
player-louder = louder
player-only = media only
player-maximize = whole screen
player-pip = small window
player-mute = mute
player-swap = window/column
# The swap's own sentence in the footer, where it opens the window in the
# view's place — the command's says window and column.
player-swap-footer = The player in a window, in place of the footer
player-tracks = media
player-fold = fold
player-earlier = earlier
player-later = later
player-also = play too
player-rename = rename
player-subtitles = subtitles
player-lane-shown = on or off
player-timeline = timeline
player-search = search
player-favourite = favourite
player-nearer = nearer
player-further = further
player-world = whole
player-layout = layout
player-every-screen = every screen
ask-player-together = To play together (a path or an address)
ask-player-rename = A new name (empty: its own)
ask-player-save-set = The set's name
ask-player-save-queue = The queue's name
ask-player-lane-start = Begins at (or +1.5)
ask-player-lane-until = Plays until (or +1.5)
ask-player-lane-title = Its name (empty: its file's)
ask-player-tag-title = Title
ask-player-tag-artist = Artist
ask-player-tag-album = Album
ask-player-tag-album-artist = Album artist
ask-player-tag-track = Track number
ask-player-tag-disc = Disc number
ask-player-tag-date = Date
ask-player-tag-genre = Genre
ask-player-tag-composer = Composer
ask-player-tag-lyricist = Lyricist
ask-player-tag-publisher = Record label
ask-player-tag-isrc = ISRC
ask-player-tag-show = Series
ask-player-tag-season = Season
ask-player-tag-episode = Episode
ask-player-tag-director = Director
ask-player-tag-name = The new tag's name
ask-player-tag-value = Its value
ask-player-tag-value-all = Its value (replaces all it holds)
player-heading-now = Now
player-heading-media = Media in the file
player-heading-media-set = Media in the files
player-heading-listen = Listen and watch
player-heading-features = More features
# One key a page (`player_page::PageId::key`): the row that opens it says it,
# and the page is titled by it.
player-page-files = Files
player-page-files-together = Something to play together
player-page-files-open = Open a set or a queue
player-page-together = Play together
player-page-internet = Internet
player-page-machines = Other machines
player-page-sound = Sound
player-page-picture = Picture
player-page-subtitles = Subtitles
player-page-gone = What this page showed is no longer here.
# The channels by theme, one name a category: `assets/channels.nix` declares
# the ids, `nix/broadcast.nix` lists the keys, and `channels-have-text` holds
# both languages to that list.
player-channels-action = Action
player-channels-anime = Anime
player-channels-cartoons = Cartoons
player-channels-classic = Classics
player-channels-comedy = Comedy
player-channels-documentary = Documentaries
player-channels-films = Films
player-channels-kids = Children
player-channels-music = Music
player-channels-news = News
player-channels-relax = Relaxation
player-channels-science = Science
player-channels-scifi-horror = Sci-fi and horror
player-channels-series = Series
player-channels-sports = Sport
player-channels-travel = Travel
player-channels-general = General
player-channels-entertainment = Entertainment
player-channels-culture = Culture
player-channels-lifestyle = Lifestyle
player-channels-cooking = Cooking
player-channels-outdoor = Outdoors
player-channels-auto = Cars
player-channels-business = Business
player-channels-weather = Weather
player-channels-public = Public television
player-channels-legislative = Parliaments
player-channels-religious = Religion
player-channels-shop = Shopping
player-channels-interactive = Interactive
# The ambient sounds, one name an id of `assets/ambience.nix`; a sound
# this door has no name for is said as one, never by its id alone.
player-ambience-small-waterfall = Small waterfall
player-ambience-rain-and-thunder = Rain and thunder
player-ambience-thunderstorm = Thunderstorm
player-ambience-unnamed = A sound this player has no name for ({ $id })
player-ambience-group-water = Water
player-ambience-group-storm = Storms
player-ambience-group-unnamed = Sounds of a kind this player has no name for ({ $group })
# THE AMBIENT SOUNDS' PAGE (`player_ambience`) and its credits.
player-page-ambience = Ambient sounds
player-page-ambience-credits = Credits
player-about-ambience = Recordings of water and storms that play beside the queue, round and round, until switched off. The queue's keys never stop them.
player-ambience-held = held
player-ambience-hold = Hold them
player-ambience-silence = Silence them all
player-ambience-silenced = Every ambient sound is off.
player-ambience-count = { $count ->
        [one] { $count } sound
       *[other] { $count } sounds
    }
player-about-credits = Every sound here is a recording someone gave to everyone: who made it, where it was published, and under what licence.
player-credit-by = Recorded by { $author }
player-credit-page = Published at { $page }
player-licence-cc0 = CC0: given to the public domain
player-licence-public-domain = Public domain
player-not-running = The player is not running.
# A player OLDER than the rest of the system, which may drop what it cannot
# read (`player::older`): said, with the row that starts it again.
player-old-daemon = This player is older than the rest of the system and may not understand what is asked of it. Starting it again brings the new one; the queue is kept.
player-restart = Start the player again
player-ask-restart = Start the player again? What plays stops; the queue is kept.
player-restarting = Starting the player again…
player-pair-no-answer = The other machine did not answer in time. Check its name and its code, and that it is on.
player-nothing = Nothing is loaded.
player-playing = Playing
player-state = State
player-state-playing = playing
player-state-paused = paused
player-position = Position
player-volume = Volume
player-muted = muted
player-speed = Speed
player-repeat = Repeat
player-repeat-nothing = nothing
player-repeat-queue = the queue
player-repeat-one = this one
player-shuffle = Shuffle
# The media in the file, on their pages (`player_tracks`): the one in use
# says so as its value, and a subtitle can be none.
player-track-in-use = in use
player-subtitles-off = No subtitles
# THE SUBTITLES' PAGE (`player_subtitles`): the lanes over the picture, and
# where more come from; and a lane's own page (`player_lane`).
player-page-files-subtitle = Add a subtitle file
player-page-lane = Lane
player-about-subtitles = Up to six subtitles lie over the picture at once, each in its own colour.
player-hidden = hidden
player-subtitle-bottom = at the foot
player-subtitle-top = at the top
player-lane-merged-plain = With another subtitle over the picture, it wears its colour and loses its own styles.
player-heading-beside = Beside the file
player-heading-inside = Inside the file
player-subtitles-beside-none = No subtitle file beside it has its name.
player-subtitles-inside-none = The file carries no subtitles of its own.
player-subtitle-added = { $name } goes over the picture.
player-subtitles-shown = { $count ->
        [one] { $count } shown
       *[other] { $count } shown
    }
player-lane-source = File
player-lane-on = Plays
player-lane-stack = Where it sits
player-lane-remove = Take it out
player-page-timeline = Timeline
player-about-timeline = Every lane of what plays, on one clock. Drag a band to change when its lane begins; a click on it opens the lane's page.
player-page-layout = Layout
player-about-layout = How the pictures of what plays share a screen: the form, and each picture's place — shown or hidden, moved by the arrows at the end of its row. Its screens are chosen on its own page.
player-layout-form = Form
player-form-auto = automatic
player-form-single = one picture
player-form-side = side by side
player-form-stacked = stacked
player-form-grid = grid
player-form-strip = large and strip
player-form-corner = in the corner
player-heading-layout-pictures = Pictures
player-layout-shown = shown
player-layout-cover = cover
player-layout-nothing = Nothing that plays has a picture to arrange.
player-layout-no-room = no room
player-layout-left-out = A picture that would be too small here in this form is left out; a larger door, or another form, shows it.
player-feature-every-screen = Show on every screen
player-every-screen-opened = What plays fills { $count ->
        [one] one screen
       *[other] { $count } screens
    }; the same again closes them.
player-every-screen-closed = The whole-screen windows are closed.
player-every-screen-one = There is one screen, and what plays is on it already.
player-every-screen-nothing = Nothing that plays has a picture for the screens.
player-every-screen-failed = The whole-screen windows could not be opened: { $why }
player-heading-lane-screens = Screens
player-lane-every-screen = On every screen
player-screen-built-in = Built-in screen ({ $connector })
player-screen-named = { $description } ({ $connector })
player-screen-absent = not connected
player-screen-last = A picture goes to one screen at least; to show it nowhere, hide it.
player-screens-unknown = This door does not know the screens by their names yet.
player-heading-lane-sound = Sound output
player-sink-default = Default output
player-sink-headphones = headphones
player-sink-speakers = speakers
player-sink-bluetooth = Bluetooth
player-sink-other = another output
player-sink-gone = not there
player-sink-twin = { $name } ({ $n })
player-sinks-none = No other output is known yet.
player-lane-start = Begins at
player-lane-start-refused = “{ $said }” is not a time: m:ss, h:mm:ss, seconds, or +/- seconds.
player-lane-until = Plays until
player-lane-until-end = its end
player-lane-until-too-soon = It has to stop after it begins in its file ({ $from }).
player-lane-until-back = It plays to its end again.
player-lane-title = Name
player-lane-over = Over the picture
player-lane-over-none = no picture plays
player-lane-volume = Its own volume
player-lane-mute = Silent
player-lane-picture-shown = Picture shown
player-lane-cover-shown = Cover shown
about-player-lane-toggle = Switch this lane on or off
about-player-lane-name = This lane's page
about-player-lane-band = Drag to move when it begins; a click opens its page
about-player-deck-seek = Click or drag to move through what plays
about-player-playhead = Where what plays is now: drag it to move through
about-player-lane-mute = Mute this lane, or give its sound back
about-player-lane-volume = This lane's own volume: click or drag, or turn the wheel over its line
about-player-deck-volume = The player's volume: click or drag, or turn the wheel
player-lane-about = { $name }: { $about }
player-lane-twin = { $name } · { $kind ->
        [video] picture
        [audio] sound
       *[subtitle] subtitle
    }
player-tracks-none = No media to choose here.
# One of the file's media that another lane plays — added to play too.
player-track-too = plays too
player-track-plays-already = That one plays already.
player-also-added = { $name } plays too.
# A window opened for the picture alone (`player_stage`) has no rows to
# bring back: a key that needs them says where they are.
player-stage-has-no-rows = Only the picture is here; the player's rows are in the footer, a window or the column.
# The footer's picture, when the region is too narrow to hold it beside
# the rows (`player_media::footer_panel`).
player-panel-no-room = No room for the picture here; { $keys } shows it alone.
player-heading-lyrics = Lyrics
player-lyrics-source = Source
player-lyrics-from-file = .lrc file
player-lyrics-from-tag = the file's tags
player-lyrics-from-host = { $host }
player-heading-queue = Queue
player-queue-empty = The queue is empty.
# An entry the player is still reading from the network: shown, not yet
# playable.
player-resolving = { $name } — reading…
player-about-together = What plays, lane by lane on one clock: its pictures, sounds and subtitles, each from its own file.
# What a lane of what plays is doing, beside its glyph and its name.
player-lane-apart = { $ms } ms apart
player-lane-pending = waiting
player-lane-ended = ended
player-lane-preparing = getting ready
player-lane-refused = does not play
player-lane-switched-off = switched off
# A lane of what plays: what kind it is — two lanes of one file have one
# name — and how far from the clock, or that it is off.
player-together-lanes = { $count ->
        [one] one lane
       *[other] { $count } lanes
    }
player-together-added = { $name } plays together with what plays.
player-lyrics-online = Lyrics from { $host }
player-sponsorblock = Skip sponsored parts ({ $host })
player-skip-sponsor = Sponsors
player-skip-selfpromo = Self-promotion
player-skip-interaction = Asks to like or subscribe
player-skip-intro = Intros
player-skip-outro = Credits and end cards
player-skip-preview = Previews and recaps
player-skip-hook = Teasers at the start
player-skip-music-offtopic = Talk inside a music video
player-skip-filler = Filler and tangents
player-trouble-write = The player's settings could not be written: { $why }
cmd-player-pair = Pair with another machine on the tailnet
cmd-player-machines = The other machines on the tailnet: play in step with one
ask-player-pair = The other machine's name, as its screen shows it
ask-player-pair-code = The code the other machine shows
player-no-tailnet = This machine is not on a tailnet, so no other machine can play in step with it.
player-about-machines = Play in step with another machine on the tailnet: as its screen, as its speaker, or both. Pair once with the code the other one shows.
player-join = join
player-joined-as = { $role }, { $ms } ms apart
player-guest = can join this one
player-invite = Show a code to pair
player-pair = Pair with a machine…
player-role-both = picture and sound
player-role-screen = picture only
player-role-sound = sound only
player-trouble-remote = The other machine: { $why }
player-quit-everywhere = Close the player everywhere
# The player's own files, and another name for what plays: rows of "More
# features", each asking on the door's line.
player-feature-save-set = Save as a set…
player-feature-save-queue = Save the queue…
player-feature-rename = Rename…
player-renamed = Now called { $name }.
player-renamed-back = { $was } has its own name again.
player-saving = Saving { $name } in { $folder }.
player-about-open = A set plays now, right after what plays. A queue takes the place of the queue.
player-ask-replace-queue = Put { $name } in place of the queue? The queue as it is now goes.
player-queue-opened = { $name } is the queue now.
player-open-not-ours = { $name } is neither a set (.rset) nor a queue (.rqueue).
player-heading-question = A question
player-answer-no = No
player-answer-yes = Yes
player-added = { $name } was added to the queue.
player-playing-now = Playing { $name } now; the queue stays.
player-ask-clear = Take everything out of the queue?
player-ask-quit = Close the player everywhere? What plays stops, and every window, column and view of it closes. The queue is kept for next time.
player-queue-cleared = The queue is empty now.
player-not-installed = The player is not installed on this machine: its profile leaves it out (ronoctua.player.enable).
player-column-refused = The column could not be raised: { $why }
player-browse-nothing-to-play = Nothing in this folder is something the player plays.
player-trouble-unplayable = This could not be played: { $why }
player-trouble-no-decoder = This format needs { $program }, which is not installed.
player-trouble-unsupported = No decoder here reads { $format }.
player-trouble-no-engine = The player could not start its engine: { $why }
# One sentence for every trouble the player can say (`player::trouble_sentence`).
player-trouble-not-understood = The player did not understand what was asked — the player and the rest of the system are of different versions: { $why }
player-trouble-too-many-leaves = The queue holds at most { $most } things to play.
player-trouble-too-many-depth = Playlists go at most { $most } deep.
player-trouble-too-many-lanes = At most { $most } lanes play together.
player-trouble-too-many-subtitles = At most { $most } subtitles go over one picture.
player-trouble-too-many-subtitle-pictures = Subtitles drawn as pictures: at most { $most } over one picture.
player-trouble-too-many-voices = At most { $most } files play at once in one set.
player-trouble-too-many-pictures = At most { $most } pictures show at once.
player-trouble-too-many-ambience = At most { $most } ambient sounds play together.
player-trouble-no-such-entry = That entry is not in the queue any more.
player-trouble-not-a-playlist = Only a playlist holds entries.
player-trouble-into-itself = A playlist cannot go inside itself.
player-trouble-nameless = A playlist needs a name.
player-trouble-unreadable = { $path } could not be read.
player-trouble-unwritable = { $path } could not be written: { $why }
player-trouble-nowhere-to-save = { $path }: no folder took what was fetched for it — neither beside it nor in the player’s own.
player-trouble-no-runtime-dir = This session has no runtime folder (XDG_RUNTIME_DIR), where the player keeps its drafts and what it fetched.
player-trouble-too-large = { $path } is larger than any file the player writes.
player-trouble-not-ours = { $path } is not one of the player's files, whatever its name says.
player-trouble-newer = { $path } was saved by a newer player (version { $found }).
player-trouble-damaged = { $path } is damaged: something in it is not what its format says.
player-trouble-empty = { $path } names nothing that plays.
player-trouble-missing = { $program } is not on this machine, and this needs it.
player-trouble-sink-gone = { $device } is not there; the lane plays on the default output.
player-trouble-unreachable = { $what } did not answer: { $why }
player-trouble-stream-down = { $tried ->
        [one] { $title }: its address failed.
       *[other] { $title }: all { $tried } of its addresses failed.
    }
player-trouble-listing-absent = { $what }: this machine has no yt-dlp to list it.
player-trouble-listing-unstartable = { $what }: yt-dlp could not be started.
player-trouble-listing-slow = { $what }: yt-dlp took too long, and was stopped.
player-trouble-listing-failed = { $what }: yt-dlp could not list it — no network, a list that was removed, or an address it does not know.
player-trouble-listing-garbled = { $what }: yt-dlp answered something that is not a list.
player-trouble-listing-too-large = { $what }: the list is larger than the player reads.
player-trouble-listing-not-an-address = { $what } is not a web address.
player-trouble-listing-not-a-list = { $what } is one video, not a list.
player-trouble-listing-nothing-to-ask = { $what }: there were no words to search for.
player-trouble-subtitle = The subtitles in { $path } could not be read: { $why }.
player-subtitle-refusal-too-large = larger than { $most } MiB, which no subtitle file is
player-subtitle-refusal-unknown = not a subtitle format the player knows
player-subtitle-refusal-damaged = the file is damaged
player-subtitle-refusal-empty = the file has no subtitles in it
player-subtitle-refusal-untimed = lyrics without times: nothing says when to show them
player-subtitle-refusal-too-many-cues = more than { $most } subtitles in one file
player-subtitle-refusal-too-slow = reading it took longer than { $most } s
player-subtitle-refusal-streamed = on the network, they are shown only by themselves, never stacked with others
player-subtitle-refusal-no-index = a VobSub .sub needs its .idx beside it
player-trouble-no-picture = { $path } has nothing to go over: no film in the queue has its name, nothing with a picture plays, and no film beside it has its name.
player-trouble-nothing-playing = Nothing is playing.
player-trouble-no-such-lane = That lane is no longer part of what plays.
player-trouble-wrong-kind = That does not apply to this lane.
player-trouble-bad-value = That value does not fit this lane.
player-trouble-last-lane = A set keeps at least one lane.
player-trouble-following = Playing in step with { $host }: that machine decides.
player-trouble-stream-plays-alone = A stream from the network plays alone.
player-trouble-no-ambience = This machine has no ambient sounds yet: they come with the player once their recordings are published.
player-trouble-no-such-sound = There is no ambient sound called { $id }.
player-trouble-sound-unplayable = { $sound } would not play: { $why }
player-trouble-tags = { $name } — { $why }
player-trouble-no-such-draft = That draft was already written or let go.
player-trouble-too-many-drafts = At most { $most } fields open in the editor at once.
player-trouble-too-many-writes = At most { $most } changes waiting to be written.
player-trouble-too-many-takes = At most { $most } downloads waiting.
player-service-lrclib = LRCLIB
player-service-sponsorblock = SponsorBlock
player-service-musicbrainz = MusicBrainz
player-service-coverartarchive = Cover Art Archive
player-service-wikidata = Wikidata
player-service-tmdb = TMDB
player-service-opensubtitles = OpenSubtitles
player-service-subdl = SubDL
player-service-podnapisi = Podnapisi
player-trouble-lookup-no-key = { $service } needs your key: keep it on the Internet page.
player-trouble-lookup-key-refused = { $service } refused the key, the token or the account.
player-trouble-lookup-wrong-key = That is TMDB's v3 key; keep the API Read Access Token instead.
player-trouble-lookup-quota = { $service }: today's downloads are spent.
player-trouble-lookup-too-many-asks = { $service } asked to slow down; try again in a moment.
player-trouble-lookup-nothing-found = { $service } found nothing.
player-trouble-lookup-nothing-to-ask = Nothing to ask { $service } with: no title and no words.
player-trouble-lookup-unreachable = { $service } could not be reached.
player-trouble-lookup-malformed = { $service } answered something this player does not read.
player-trouble-lookup-too-large = { $service } answered more than any answer should be.
player-trouble-lookup-declined = { $service } said no.
player-trouble-lookup-unreadable = The file could not be read to take its fingerprint.
player-trouble-lookup-too-small = The file is too small for a fingerprint; it was asked by its name.
player-trouble-lookup-dormant = { $service } is asleep in this version: its domain has not answered since June 2026.
player-trouble-off = Not asked: { $host } is not allowed yet.
player-trouble-exists = { $path } is already there.
player-trouble-no-such-candidate = That result belongs to an older search.
player-source-radio = radio-browser
player-source-iptv = iptv-org
player-source-archive = the Internet Archive
player-trouble-directory-unreachable = { $source } could not be reached.
player-trouble-directory-declined = { $source } said no.
player-trouble-directory-too-many-asks = { $source } asked to slow down; try again in a moment.
player-trouble-directory-nothing-found = { $source } found nothing.
player-trouble-directory-malformed = { $source } answered something this player does not read.
player-trouble-directory-too-large = { $source } answered more than any list should be.
player-trouble-directory-nothing-playable = Nothing in it is something this player plays.
player-trouble-directory-nothing-to-ask = Nothing to ask { $source } with: type some words.
player-trouble-directory-not-a-name = That is not a name { $source } gives; nothing was sent.
player-trouble-no-such-category = This theme has none of that.
player-trouble-no-such-pick = That is on no list any more: open it again to keep it.
player-trouble-picks-no-home = There is no home folder to keep the list in.
player-trouble-picks-unreadable = The list on disk could not be read, and was left as it is.
player-trouble-picks-newer = The list was written by a newer player.
player-trouble-picks-unwritable = The list could not be written.
player-heading-metadata = Metadata
player-heading-technical = Technical
player-meta-file = Of the file
player-meta-reading = Reading what the file says of itself…
player-meta-still-reading = The player is still reading this file — it may be on a slow disk. Read it again to ask once more.
player-field-blank = Nothing is written in this field.
player-field-edit = Edit in the editor
player-meta-fixed = None of these can be changed here. { $why }
player-meta-refresh = Read it again
player-meta-refreshing = Reading the file again.
player-meta-count = { $count }
player-meta-joiner = { " · " }
player-list-joiner = {", "}
player-meta-cover-size = { $width } × { $height }
player-meta-cover-none = none
player-meta-cover-some = in the file
player-meta-cover-nothing = This file carries no cover.
player-page-cover = Cover picture
player-page-files-cover = Another picture for the cover
player-page-streams = Streams
player-page-chapters = Chapters
player-page-every-tag = Every tag
player-cover-save = Save it beside the file
player-cover-remove = Take it out of the file
player-ask-remove-cover = Take the cover out of { $name }? No copy of it is kept.
player-cover-putting = Putting { $picture } in { $name }.
player-cover-saving = Saving the cover beside { $name }.
player-cover-removing = Taking the cover out of { $name }.
player-cover-saved = The cover was saved as { $name }.
player-page-suggestions = Suggestions
player-page-candidate = Suggestion
player-page-find-subtitles = Find subtitles
player-page-find-lyrics = Find lyrics
player-page-languages = Choose the languages
player-page-radio = World radio
player-page-radio-genres = Genres
player-page-radio-languages = Languages of radio
player-page-stations = Stations
player-page-tv = World TV
player-page-tv-channels = TV channels
player-page-channels = Channels by theme
player-page-favourites = Favourites
player-page-recent = Played lately
player-about-radio = Radio stations of the whole world, from radio-browser: by country, by genre, by language or by name.
player-about-tv = TV channels of the whole world, from iptv-org's public list: by country or by name.
player-about-favourites = What you kept with the star: stations, channels, films and lives.
player-about-recent = The streams that played lately, as the player remembers them.
player-favourites-none = Nothing kept yet.
player-recent-none = Nothing played yet.
player-radio-search = Search stations by name
player-radio-genre-search = Find a genre by its name
player-radio-language-search = Find a language by its name
player-about-radio-genres = The genres with the most stations. Any other is found by its name, as radio-browser writes it.
player-about-radio-languages = The languages with the most stations. Any other is found by its name, as radio-browser writes it — in English.
player-tv-search = Search channels by name
player-heading-countries = Countries
player-picks-radio = Radio stations
player-picks-tv = Television
player-picks-archive = Archive films
player-picks-live = Lives
player-stations-count = { $count ->
        [one] one station
       *[other] { $count } stations
    }
player-channels-count = { $count ->
        [one] one channel
       *[other] { $count } channels
    }
player-globe-placeless = No place on the map ({ $count })
player-globe-no-country = Of no country
player-globe-no-place = No place on the map
player-globe-place = { $count } · { $place }
player-globe-one = { $name } · { $country }
player-radio-bitrate = { $kbps } kbps
player-radio-hls = adaptive stream
player-stream-silent = did not answer
player-tv-lines = { $lines } lines
player-tv-geo-blocked = only in some countries
player-tv-sometimes-off = not always on
player-tv-shown = { $shown } shown of the { $total } found.
player-stations-page-of = Page { $page } of { $pages } — { $total } stations in all, the most played first.
player-stations-page = Page { $page } of this list — { $most } a page, the most played first.
player-stations-more = More stations
player-stations-earlier = Earlier stations
player-listed-at = List from { $date }
player-listed-updating = Updating…
player-listing-nothing = Nothing listed yet.
player-listing-again = Try again
player-listing-every = Showing { $shown } of { $total }: show them all
player-searched-title = Named “{ $words }”
player-favourite-added = Kept among the favourites.
player-favourite-removed = Taken off the favourites.
player-about-internet = Nothing here reaches the internet until you allow it, and every row says whom it asks. Those under the first title ask on their own; the others ask only when you press a row that names them, and pressing it allows them.
player-heading-on-their-own = Asked on their own
player-heading-when-asked = Asked when you press
player-heading-keys = Your keys
player-heading-results = Results
player-consent-allowed = allowed
player-consent-not-allowed = not allowed
player-consent-by-the-switch = and the switch above asks it on its own
player-consent-ask = Ask { $hosts }
player-consent-first = pressing allows it
player-consent-let = Allow { $host } and ask again
player-consent-hosts = asks { $hosts }
player-key-sent-to = sent only to { $host }
player-directory-archive = Internet Archive films
player-directory-lives = YouTube lives
player-key-opensubtitles = OpenSubtitles key
player-key-opensubtitles-account = OpenSubtitles account
player-key-tmdb = TMDB token
player-key-subdl = SubDL key
player-key-words-opensubtitles = the OpenSubtitles key
player-key-words-opensubtitles-account = the OpenSubtitles account
player-key-words-tmdb = the TMDB token
player-key-words-subdl = the SubDL key
player-key-kept = kept
player-key-none = not kept
player-key-subdl-address = SubDL takes its key only in the address, so SubDL sees it in its own logs.
player-ask-forget-key = Forget { $what }? The player keeps no other copy.
player-key-checking = Kept. Asking { $service } whether it takes it…
player-key-accepted = { $service } took the key.
player-key-forgotten = Key forgotten.
player-key-nothing-to-forget = There is nothing to forget: this machine does not keep { $what }.
player-key-trouble-no-home = There is no home folder to keep it in.
player-key-trouble-empty = Nothing was typed.
player-key-trouble-malformed = That has a line break or another control character in it.
player-key-trouble-too-long = Too long for a key.
player-key-trouble-unwritable = The key could not be written.
player-key-trouble-unreadable = The kept key could not be read.
player-prefs-will-not-parse = player.json will not parse, and writing would replace what is in it.
player-desk-asking = Asking { $hosts }…
player-desk-found = { $count ->
        [one] One result
       *[other] { $count } results
    }
player-desk-searched-for = Searched for: { $words }
player-desk-by-fingerprint = Matched by the file's fingerprint.
player-desk-by-id = Looked up by the id the file carries.
player-desk-nothing-found = Nothing found.
player-take-taking = Fetching…
player-take-handed = Writing into the file…
player-take-saved = Saved as { $name }.
player-about-suggestions = Tags for this file, from the internet. The row that asks names whom it asks, and pressing it allows them. Nothing is written until you choose what to take.
player-suggest-source = Suggested by
player-suggest-came-from = These came from { $service }.
player-suggest-poster-needs-token = A poster comes only with TMDB and your token; Wikidata almost never has one.
player-suggest-other-words = Search with other words
player-suggest-reading = reading the details…
player-suggest-busy = asking…
player-about-candidate = Choose what to write: a field the file has blank starts chosen. What the file says now is under each value.
player-suggest-now = now: { $value }
player-suggest-same = the file says the same
player-suggest-cover = Cover
player-suggest-taken = will be written
player-suggest-left = left out
player-suggest-picture-suggested = Suggested
player-suggest-picture-current = Current
player-suggest-picture-none = No cover came
player-suggest-fetching = Fetching the cover…
player-suggest-nothing-chosen = Nothing chosen to write
player-suggest-write = { $count ->
        [one] Write one field
       *[other] Write { $count } fields
    }
player-suggest-write-cover = { $count ->
        [0] Write the cover
        [one] Write one field and the cover
       *[other] Write { $count } fields and the cover
    }
player-suggest-writing = Writing what was chosen into { $name }…
player-about-find-subtitles = Subtitles for this film from the internet, in the languages below. One timed for this very file is marked. Choosing one downloads it beside the film and lays it over the picture.
player-find-needs-a-key = Searching subtitles needs your OpenSubtitles or SubDL key: there is no source without one today.
player-find-languages = Languages
player-find-unknown-language = { $code } is not a language this player knows.
player-find-language-there = { $language } is searched in already.
player-find-language-added = { $language } will be searched in too.
player-find-language-removed = { $language } is no longer searched in.
player-find-language-last = Subtitles are searched in one language at least.
player-find-language-searched = { $language } is searched in; d, or the glyph at the end of its row, takes it out.
player-find-type-language = Type a language
player-about-languages = Subtitles are searched in every language that is on. The list is every language the services search in.
player-find-hash-match = timed for this file
player-find-hearing = for the hearing impaired
player-find-machine = machine translated
player-find-downloads = { $count ->
        [one] one download
       *[other] { $count } downloads
    }
player-find-downloads-left = Downloads left today
player-find-taking = Downloading { $name }…
player-lyrics-one-source = One source, LRCLIB: no language field, and no translation.
player-lyrics-to = Save to
player-lyrics-to-beside = a .lrc beside the song
player-lyrics-to-tag = the song's own lyrics
player-find-synced = synced
player-find-plain = words only
player-find-instrumental = instrumental
player-find-same-length = the song's length
player-find-off-by = { $seconds } s longer or shorter
player-ask-replace-lrc = Replace { $name }? What it holds is not kept.
player-lyrics-taking = Saving the lyrics beside { $name }…
player-lyrics-into-tag = Writing the lyrics into { $name }…
ask-player-find-words = Search for
ask-player-search-radio = Stations named
ask-player-search-tv = Channels named
ask-player-search-genre = Genre
ask-player-search-language = Language (in English, as radio-browser writes it)
ask-player-find-language = Language, by its name or its code (Japanese, pt-BR)
ask-player-key-opensubtitles = OpenSubtitles API key
ask-player-key-tmdb = TMDB API Read Access Token
ask-player-key-subdl = SubDL API key
ask-player-account-user = OpenSubtitles user name
ask-player-account-password = OpenSubtitles password
tongue-en = English
tongue-pt-br = Portuguese (Brazil)
tongue-pt = Portuguese
tongue-es = Spanish
tongue-ja = Japanese
player-streams-none = No stream of this file could be read.
player-chapters-none = This file has no chapters.
player-tags-none = No tool read a tag of this file.
player-stream-label = { $index } · { $kind }
player-stream-kind-video = moving picture
player-stream-kind-cover = cover picture
player-stream-kind-audio = sound track
player-stream-kind-subtitle = subtitle track
player-stream-kind-data = timed data
player-stream-kind-attachment = attached file
player-stream-default = the default
player-stream-forced = forced
player-stream-hearing = for the hard of hearing
player-chapter-untitled = no title
player-origin-id3v2 = ID3v2 tags
player-origin-id3v1 = ID3v1 tags
player-origin-ape = APEv2 tags
player-origin-vorbis = Vorbis comments
player-origin-mp4 = MP4 tags
player-origin-riff-info = RIFF INFO tags
player-origin-aiff-text = AIFF text chunks
player-origin-matroska-global = Matroska tags of the whole file
player-origin-matroska-track = Matroska tags of a track
player-origin-matroska-other = Matroska tags of other parts
player-origin-container = The container, as ffprobe reads it
player-origin-stream-n = Stream { $index }, as ffprobe reads it
player-origin-exiftool = exiftool, { $group }
player-origin-stream = What the stream says of itself
player-tag-value-cut = { $size } in all · { $head }…
player-tag-value-binary = { $size } of data
player-tag-add = Add a tag
player-tag-bad-name = “{ $typed }” is not a name for { $where }: { $rule }.
player-tag-name-rule-vorbis = letters, digits and punctuation, without = or accents
player-tag-name-rule-ape = 2 to 255 letters, digits or punctuation, without accents
player-tag-name-rule-riff = four letters or digits, like ISFT
player-tag-name-rule-aiff = NAME, AUTH, ANNO or (c)
player-tag-name-rule-mp4 = a word, or ----:mean:name
player-tag-name-rule-ffmpeg = anything without =
player-tag-name-rule-plain = one line, at most a kibibyte
player-sheet-unread = { $source } could not be read. { $why }
player-sheet-source-lofty = the tags
player-sheet-source-ffprobe = the streams and chapters
player-sheet-source-exiftool = the list of every tag
player-sheet-source-matroska = Matroska's own tags
player-tag-empty = empty
player-tag-already-empty = { $field }: nothing to clear, the field is empty already.
player-tag-more-lines = { $lines } lines · { $first }
player-draft-open = open in the editor
player-draft-opened = Open in the editor: save it there, and it is asked about here before it is written.
player-draft-says = What the editor saved
player-draft-kept = { $field }: still open in the editor; asked again once saved.
player-ask-drop-draft = { $field }: throw away the draft open in the editor? The file stays as it is.
player-draft-dropped = { $field }: the draft was thrown away.
player-ask-write-draft = { $field } of { $name }: the editor saved something new. Write it into the file?
player-ask-clear-tag = { $field } of { $name }: clear it?
player-tag-writing = { $field } of { $name }: writing it into the file.
player-tag-clearing = { $field } of { $name }: clearing it.
player-tag-named-writing = { $tag } of { $name }: writing it into the file.
player-tag-named-removing = { $tag } of { $name }: taking it out.
player-ask-remove-tag = { $values ->
        [one] { $tag } of { $name }: take it out?
       *[other] { $tag } of { $name }: take out its { $values } values?
    }
player-retag-writing = Writing into the file… { $percent }%
player-retag-waiting = { $count ->
        [one] One more change waiting.
       *[other] { $count } more changes waiting.
    }
player-tag-written = The last change was written into the file.
player-tech-container = Container
player-tech-length = Length
player-tech-bit-rate = Bit rate
player-tech-video-codec = Video codec
player-tech-width = Width
player-tech-height = Height
player-tech-fps = Frames per second
player-tech-audio-codec = Audio codec
player-tech-sample-rate = Sample rate
player-tech-channels = Channels
player-tech-channel-layout = Channel layout
player-tech-bits-per-sample = Bits per sample
player-tech-programs = Programs
player-tech-value-kbps = { $kbps } kb/s
player-tech-value-px = { $px } px
player-tech-value-hz = { $hz } Hz
# The player's refusals that protocol 3 brought.
player-trouble-already-queued = It is already in the queue: { $name }.
player-trouble-changed-on-disk = The queue's file changed on the disk since it was opened or saved: { $path }
player-trouble-no-origin = This queue was never opened from a file nor saved as one, so there is nowhere to save it again.
player-trouble-no-sound = This file has no sound to play.
player-trouble-not-yet = The player cannot do this yet.
player-trouble-said = { $program } said: { $text }
# A text of several lines, edited in place (both doors).
text-area-keep = keep
about-text-area-keep = Keep what is written here.
text-area-discard = leave
about-text-area-discard = Leave the text; if it changed and was not kept, you are asked first.
ask-text-area-discard = Throw away what was written and not kept?
text-area-no = No, go on writing
text-area-yes = Yes, throw it away
about-text-area = A text of several lines, typed and changed here.
text-area-not-kept = Nothing keeps this text here yet; it stays open as written.
player-channels-education = Education
player-channels-space = Space
player-channels-history = History
player-channels-nature = Nature and animals
player-channels-art = Art and dance
player-channels-gaming = Games
player-channels-audiobooks = Audiobooks
player-channels-source-iptv = iptv-org's TV channels
player-channels-source-archive = The Internet Archive
player-channels-source-lives = Live on YouTube
player-channels-source-nasa = NASA Image and Video Library
player-channels-source-commons = Wikimedia Commons
player-channels-source-peertube = PeerTube, through SepiaSearch

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-media = What the player plays
file-picker-filter-lanes = What plays together
file-picker-filter-saved = Sets and queues
file-picker-filter-subtitles = Subtitles
player-page-pick-save-set = Save what plays as a set
player-page-pick-save-queue = Save the queue
player-added-several = { $count } files added to the queue.
player-playing-several = Playing { $count } files now.

## file info editing — everything the info panel can change

# Why the lyrics were not translated (`ronoctua_words::Why`).
player-trouble-translation = The lyrics were not translated: { $why }
player-trouble-translation-empty = there are no words to translate.
player-trouble-translation-too-long = the song is longer than the translator takes at once.
player-trouble-translation-unknown-tongue = the translator does not know that language.
player-trouble-translation-wrong-pass = the consent given is for another host than the translator's.
player-trouble-translation-no-key = the translator needs a key, and none is kept.
player-trouble-translation-key-refused = the translator refused the key.
player-trouble-translation-quota = the translator's quota for today is spent.
player-trouble-translation-paced = asked too often; try again in a moment.
player-trouble-translation-not-found = the translator found nothing for it.
player-trouble-translation-no-dictionary = there is no dictionary for this language.
player-trouble-translation-unreachable = the translator could not be reached.
player-trouble-translation-failed = the translator answered with a failure.
player-trouble-translation-garbled = the translator answered something that is not a translation.
# Why a press was not kept in the consent file (`ronoctua_consent::Unkept`).
player-consent-unkept-no-home = There is no home folder to keep the consent in.
player-consent-unkept-unparseable = The consent file (consent.json) will not parse; it was left as it is, and nothing was let.
player-consent-unkept-unwritable = The consent file could not be written; nothing was let.
player-lyrics-unsynced = Translated as a whole, not verse by verse:
# THE PLAYER'S DOORS (2026-09-30). The Queue's file on its title.
player-queue-unsaved = { $name } · not saved
player-queue-new-unsaved = new, not saved
player-save-queue-again = save again
cmd-player-save-queue-again = Save the queue again into the file it came from
player-saved-again = The queue was saved again in { $name }.
ask-player-already-queued = { $name } is already in the queue as { $number }. Add it again?
ask-player-already-queued-somewhere = { $name } is already in the queue. Add it again?
ask-player-queue-changed-on-disk = { $path } changed on the disk since the queue was opened or saved. Write over it?
# The seeks beside the clock, and a double click on a side of the picture.
player-rewind = back
player-forward = ahead
cmd-player-rewind = Ten seconds back
cmd-player-forward = Ten seconds ahead
player-seeked-back = −{ $seconds } s
player-seeked-ahead = +{ $seconds } s
# The picture folded away, on the "Now" title.
player-picture-fold = fold the picture
cmd-player-picture-fold = Fold the picture away and give its lines to the rows, or show it again
player-no-picture-to-fold = There is no picture here to fold away.
# A video from a website: its own page.
player-page-website = Video from a website
player-about-website = A video or a song on YouTube, or on one of the many other sites yt-dlp knows, plays from its page's address, typed or pasted on the line.
player-website-play = Play now…
player-website-enqueue = Add to the queue…
ask-player-website-play = Play now (a page's address)
ask-player-website-enqueue = Add to the queue (a page's address)
cmd-player-website = A video from a website: play it now or add it to the queue, by its address
player-website-recent = Addresses played lately
player-website-none = No address played from here yet.
player-website-not-an-address = “{ $typed }” is not a page's address.
player-website-forgotten = Forgotten: { $address }
player-file-no-home = there is no home folder to keep it in
player-file-unparseable = it is there and does not parse, so it was left as it is
player-file-unwritable = it could not be written
# A several-line field changed here, in this door.
player-field-edit-here = Edit here
# The countries of a world page: the most first, or A to Z.
player-countries-order = order
cmd-player-countries-order = The countries A to Z by name, or the most first
player-countries-by-count = The countries with the most first.
player-countries-by-name = The countries A to Z, by name.
# The lyrics translated under their verses.
player-lyrics-translate = translate
cmd-player-lyrics-translate = Translate the lyrics under their verses, or stop
player-lyrics-translate-to = Translate into
player-lyrics-translating = Translating…
player-lyrics-translated-by = Translated by
player-lyrics-none-to-translate = There are no lyrics to translate.
player-lyrics-let = Let { $host } translate them
player-consent-translation = Translating the lyrics
player-page-catalog = Listing
player-page-sources = Sources
player-catalog-page = Page
player-catalog-page-of = { $page } of { $pages }
player-catalog-layout = Shown as
player-catalog-layout-list = List
player-catalog-shown = { $from }–{ $to } of { $total }
player-adult-open = The adult catalogue is open until 15 minutes pass with no key and no click.
player-adult-wrong = That is not your password.
player-adult-no-service = This system has no ronoctua-unlock service, so the adult catalogue stays shut.
player-adult-unavailable = The password could not be checked.
# WHY A SOURCE GAVE NOTHING (`ronoctua_broadcast::sources::Refused`).
player-trouble-source-not-an-address = That is not a web address.
player-trouble-source-not-a-list = That is one video or one stream, not a list of them.
player-trouble-source-unknown = That is none of the sources read here: an M3U list, an RSS or Atom feed, a YouTube channel or playlist, an archive.org collection.
player-trouble-source-not-let = Its host is not allowed yet.
player-trouble-source-plain-http = It was sent on from an encrypted address to one that is not, which is never followed.
player-trouble-source-unreachable = Its host could not be reached.
player-trouble-source-declined = Its host refused the question.
player-trouble-source-too-many-asks = Its host asked to be asked less often.
player-trouble-source-nothing-found = There is nothing at that address.
player-trouble-source-malformed = Its answer is not what this reads.
player-trouble-source-too-large = Its answer is larger than what is read.
player-trouble-source-nothing-playable = It was read, and nothing in it plays.
player-trouble-source-no-yt-dlp = This installation has no yt-dlp to list it.
player-trouble-source-listing-failed = yt-dlp could not list it.
# THE CATALOGUE'S FILES (`ronoctua_broadcast::catalog::Trouble`).
player-trouble-catalog-no-home = There is no cache folder to keep the catalogue in.
player-trouble-catalog-absent = Nothing has been read into the catalogue yet.
player-trouble-catalog-other-schema = The catalogue is from another version, and is being made again.
player-trouble-catalog-unreadable = The catalogue could not be read.
player-trouble-catalog-unwritable = The catalogue could not be written.
player-trouble-catalog-too-large = That would pass the catalogue's ceiling, so nothing changed.
player-trouble-sources-no-home = There is no settings folder to keep your sources in.
player-trouble-sources-unreadable = Your sources' file could not be read.
player-trouble-sources-unparseable = Your sources' file is not one this reads; it is kept as it is, and nothing is added to it.
player-trouble-sources-unwritable = Your sources' file could not be written.
player-trouble-no-such-source = There is no such source.
# A LISTING'S KEYS.
cmd-player-listing-earlier = The listing's page before
cmd-player-listing-later = The listing's page after
cmd-player-listing-first = The listing's first page
cmd-player-listing-last = The listing's last page
cmd-player-listing-full = The listing alone on the whole screen
cmd-player-listing-layout = The listing's next layout
cmd-player-listing-source = The catalogue's next source
cmd-player-listing-letter = A letter of the listing, typed
cmd-player-listing-order = The listing's next order
# A LISTING'S KEYS, AS THE HINT LINE SAYS THEM.
player-listing-earlier = page before
player-listing-later = page after
player-listing-first = first page
player-listing-last = last page
player-listing-full = whole screen
player-listing-layout = shown as
player-listing-source = next source
player-listing-letter = letter
player-listing-order = order
# A lookup or a directory whose host consent.json does not let: nothing
# was sent. The engine says it as the consent row (`Trouble::Off`); this is
# the sentence of the refusal itself, where it is said apart from that row.
player-trouble-lookup-not-let = { $service } was not asked: its host is not let.
player-trouble-directory-not-let = { $source } was not asked: its host is not let.
# THE RANKING OF THE VIDEO CATALOGUE, a page of the player (LEADER p k,
# player_ranking.rs). Titles and channel names are data.
player-page-ranking = Videos by score
player-page-ranked-video = This video
cmd-player-ranking = Videos by score: the video catalogue's best first, a click from the queue
player-ranking-play-now = Play now
player-ranking-add = Add to the queue
player-ranking-kiosk = Watch in Chrome
player-ranking-download = Download
player-ranking-hide = Hide from the ranking
player-ranking-delete = Delete from the catalogue
# THE RANKING'S KEYS.
cmd-player-ranking-more = This video's own page: play, add, Chrome, download, hide, delete
cmd-player-ranking-earlier = The ranking's page before
cmd-player-ranking-later = The ranking's page after
cmd-player-ranking-first = The ranking's first page
cmd-player-ranking-last = The ranking's last page
cmd-player-ranking-layout = The ranking as a list, or as thumbnails
# THE RANKING'S KEYS, AS THE HINT LINE SAYS THEM.
player-ranking-more = more
player-ranking-earlier = page before
player-ranking-later = page after
player-ranking-first = first page
player-ranking-last = last page
player-ranking-layout = list or thumbnails
player-page-queue = The whole queue
player-queue-entries = { $count ->
        [one] one entry
       *[other] { $count } entries
    }
player-lane-language = Language shown
player-lane-switched-off-length = switched off · { $length }
player-translator = Translator
player-translator-mymemory = MyMemory
player-translator-deepl = DeepL
player-translator-libretranslate = LibreTranslate
player-translator-address = Server address
player-translator-not-an-address = “{ $typed }” is not an https:// address with a host: nothing could be asked there.
player-key-deepl = DeepL key
player-key-libretranslate = Server key
player-key-words-deepl = the DeepL key
player-key-words-libretranslate = the LibreTranslate server's key
player-key-kept-said = Kept: { $what }.
ask-player-translator-address = LibreTranslate server address (https://…)
ask-player-key-deepl = DeepL authentication key
ask-player-key-libretranslate = LibreTranslate server key
player-host-let = { $host } is allowed now. Put again what was refused.
player-stream-language-row = Language of the stream
player-stream-title-row = Name of the stream
player-stream-default-row = Marked as the default
player-stream-forced-row = Marked as forced
player-stream-language-of = the language of stream { $index }
player-stream-title-of = the name of stream { $index }
player-chapter-title-of = the title of the chapter at { $at }
ask-player-stream-language = The stream's language, as a code (en, pt-BR, por)
ask-player-stream-title = The stream's name
ask-player-chapter-title = The chapter's title
player-origin-matroska-track-of = Matroska tags of stream { $index } · { $kind }
player-origin-matroska-track-lang = Matroska tags of stream { $index } · { $kind } · { $lang }
player-origin-matroska-track-uid = Matroska tags of the track whose UID is { $uid }
