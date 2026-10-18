# The texts `ronoctua-shell-inspect` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# WHAT THE PREVIEW SAYS WHEN THERE IS NO PICTURE. Not a refusal — those come
# from `ronoctua-inspect` and win when there is one — but the answer for a
# directory, a broken link, and a file nothing has been asked about. It used to
# be a large icon of the file's type, which said what the listing's own icon
# column already said and made an empty panel look full.
files-no-preview = no preview
# AND WHAT THE FACTS KEY SAYS WHEN THERE IS NOTHING TO SCROLL. From the
# outside, a key that refuses and a key that is dead look the same.
files-facts-all-shown = every fact is already on screen
files-preview-cut = only the first { $lines } lines are here — the file goes on
# THE PREVIEW'S TRANSPORT. What the pointer resting on a control says, and what
# the manager says back when a key does something one cannot see.
files-play-hold = hold it where it is
files-play-resume = let it go on
files-sound-on = let it be heard
files-sound-off = in silence
files-sound-none = this file carries no sound
files-entry-not-here = this entry is somewhere else, and nothing here fetches it: { $path }
files-entry-at = { $path }
files-entry-titled = { $name } · { $path }
files-entry-refused = this name cannot be a path: { $path }
files-list-nothing = there is nothing on this list that can be played from here
files-list-at-start = this is the first of the list
files-list-at-end = this is the last of the list
files-no-sound = this machine has no sound to play it through — the picture goes on without it
files-film-stuck = nothing has come out of it for { $seconds } s — it was stopped
files-film-seek = go to { $at } of { $total }
files-theatre-on = the preview has the whole region
files-theatre-off = the listing has its room back
files-theatre-nothing = nothing is selected, so there is nothing to expand
files-wide-hidden = that panel is hidden, so there is nothing to expand
files-facts-wide-on = the facts have the whole region, in as many columns as it holds
files-facts-wide-off = the facts are back in their column
files-yes = yes, apply it
files-no = no, leave it as it is
files-confirm-mode = Change the permissions of { $what } to { $mode }?
files-facts-scroll = page the information
# THE PREVIEW'S OWN KEYS, and the reason they are here rather than derived from
# `menu.rs` is `leader::OF_CLIPBOARD_VIEW`'s: a key that exists only inside a
# `match` arm is a key nothing can enumerate. These are what `LEADER :` lists,
# so each one is written as the thing it DOES and not as the key it is on.
files-preview-scroll = page the preview
# The film's transport. `files-play` is one key for both, because that is what
# it is: pressing it while it plays is what pauses it.
files-play = play or pause
files-rewind = rewind
files-forward = forward
files-sound = sound
files-facts-wide = expand the facts
# The shape's. `files-turn-left` is the hand's direction and not an axis — see
# `View::spin`, which is where that convention is written down.
files-turn-left = turn left
files-turn-right = turn right
files-turn-up = turn up
files-turn-down = turn down
files-zoom-in = zoom in
files-zoom-out = zoom out
files-reframe = reframe
files-refused = { $why }
files-explain-group = the group is { $who }; clicking changes that
files-explain-measure = click to measure this directory — it is a walk
# THE NUMBER SAYS WHAT IT IS. Each of the ten letters explains itself, and the
# form most people actually type explained nothing at all — read off the screen:
# the pointer rested on it and the line stayed empty.
#
# One sentence for three digits and for four: naming the three audiences stays
# true when a leading digit arrives, and the special bit that brought it already
# has a sentence of its own on the letter beside it.
files-explain-octal = { $octal } is the same permission written as a number — one digit for the owner, one for the group, one for everybody else
files-octal-can = { $kind ->
        [dir] { $bits ->
                [0] cannot do anything with it
                [1] can enter it, but not list it
                [2] can create and delete things in it, without being able to enter or list it
                [3] can enter it and create and delete things in it
                [4] can list the names in it
                [5] can list the names in it and enter it
                [6] can list the names in it and create and delete things in it
               *[7] can list, enter, create and delete
            }
       *[file] { $bits ->
                [0] cannot do anything with it
                [1] can run it
                [2] can change it
                [3] can change and run it
                [4] can read it
                [5] can read and run it
                [6] can read and change it
               *[7] can read, change and run it
            }
    }
files-explain-octal-digit = { $part ->
        [special] { $digit } is the first digit of { $octal }: the special bits — setuid, setgid and the sticky bit, each explained on the letter that carries it.
        [owner] { $digit } is the owner's digit of { $octal }: { $name } { $can }.
        [group] { $digit } is the group's digit of { $octal }: the group { $name } { $can }.
       *[other] { $digit } is the last digit of { $octal }: everybody else { $can }.
    }
files-explain-when = { $when }, in this machine's local time
files-explain-when-utc = { $when }, in UTC — /etc/localtime could not be read
# The detail sheet.
files-detail-path = path
files-detail-type = type
files-detail-mode = permission
files-detail-owner = owner:group
files-detail-size = size
files-detail-modified = modified
files-detail-links = names of this inode
files-detail-target = points at
files-detail-broken = points at (does not exist)
# OVER the picture, on its last line, and only when some face did not fill. A
# piece that is part outline is a piece somebody should be told about — and the
# panel is where they are looking.
files-preview-outlined = { $n ->
        [one] one face drawn as outline
       *[other] { $n } faces drawn as outline
    }
# THE GLOBE AND WHAT IT ANSWERS — shared, and the prefix says so.
#
# These are not the file manager's words even though it is the first caller:
# the glance, the locale view and the installer all hold coordinates and could
# ask the same question. A sentence with an app's prefix is an app owning a
# word that is not its.
#
# WHICH LINES THERE ARE IS THE SELECTOR and the ORDER is the language's. A
# separator written into a value could not be counted and could not be given a
# register — see `chrome::Piece::separator`.
globe-address =
    { $has ->
        [street] { $street }, { $place } — { $region }, { $country }
        [place] { $place } — { $region }, { $country }
        [region] { $region }, { $country }
       *[country] { $country }
    }
globe-address-nowhere = no named place near this coordinate
atlas-absent = the world data is not on this machine
# THE ATTRIBUTION, ON SCREEN. GeoNames is CC BY 4.0 and the street packs are
# ODbL: both REQUIRE it, and this repository is MIT + OFL — so it is said in
# four places, of which this is the one a person actually reads. See
# `LICENSE-DATA.txt` for the other three.
atlas-credits = places from GeoNames (CC BY 4.0) · borders from Natural Earth (public domain) · streets from OpenStreetMap contributors (ODbL)
atlas-heading = World data
atlas-baseline = borders and places
atlas-present = on this machine
atlas-offered = streets, by region
atlas-not-yet = not on this machine
atlas-not-yet-said = downloading a region's streets is not built yet — the picker, the queue and the reader are what is left
atlas-open = open
atlas-previous = previous
atlas-next = next
atlas-back = back
files-explain-globe = drag to turn it, the wheel to come closer
files-explain-globe-dot = where this file says it was made — Enter frames it
files-explain-globe-address = { $value } — press to open this coordinate in the browser's map.
globe-north = move the globe north
globe-south = move the globe south
globe-east = move the globe east
globe-west = move the globe west
files-fact-where = where
# ---------------------------------------------------------------------------
# LEADER s l — language, clock, date and what the system decides.
# ---------------------------------------------------------------------------
language-en-gb = English
language-pt-br = Português
about-files-facts-scroll = Turn to the next page of the facts column, and round to the top again at the end.
# THE PREVIEW: the text, the film and the shape.
about-files-preview-scroll = Turn to the next page of the preview, and round to the top again at the end.
about-files-facts-wide = Give the facts the whole region, and give the region back. Expanded they are laid out in as many columns as the width holds, so each value is shown more of.
about-files-play = Start what the preview is showing, or hold it where it is.
about-files-rewind = Jump backwards in what is playing.
about-files-forward = Jump forwards in what is playing.
about-files-sound = Give the film its sound, or take it away. A film with no soundtrack has none to give.
about-globe-north = Turn the globe northwards, towards the pole above.
about-globe-south = Turn the globe southwards, towards the pole below.
about-globe-east = Turn the globe eastwards, so the land to the right comes round.
about-globe-west = Turn the globe westwards, so the land to the left comes round.
about-files-turn-left = Turn the shape to the left, about its upright axis.
about-files-turn-right = Turn the shape to the right, about its upright axis.
about-files-turn-up = Tip the shape away from you, to look at it from below.
about-files-turn-down = Tip the shape towards you, to look at it from above.
about-files-zoom-in = Bring the shape closer.
about-files-zoom-out = Take the shape further away.
about-files-reframe = Put the shape back in the middle, at the size it started at.
ask-player-play-file = Play now (a path or an address)
player-trouble-tag-not-a-file = This is not a file on this machine: its tags can only be read.
player-trouble-tag-gone = The file is not there any more.
player-trouble-tag-changed-since = The file changed since it was read; nothing was written. It is read again: repeat the change.
player-trouble-tag-hard-linked = The file has other names (hard links), which a new copy would lose.
player-trouble-tag-read-only-folder = Its folder cannot be written.
player-trouble-tag-read-only-file = The file is marked read only.
player-trouble-tag-not-yours = Another user owns this file.
player-trouble-tag-no-room = Not enough room for the copy: { $need } needed, { $free } free.
player-trouble-tag-unreadable = Its tags could not be read.
player-trouble-tag-unwritable = Writing failed; the file is as it was.
player-trouble-tag-no-writer = Nothing here writes the tags of this kind of file ({ $container }).
player-trouble-tag-missing = { $program ->
        [mkvextract] mkvextract is not installed: Matroska files are read whole and written only with the media profile.
        [mkvmerge] mkvmerge is not installed: Matroska files are read whole and written only with the media profile.
        [mkvpropedit] mkvpropedit is not installed: Matroska files are read whole and written only with the media profile.
       *[other] { $program } is not installed.
    }
player-trouble-tag-program-failed = { $program } failed; the file is as it was.
player-trouble-tag-timed-out = { $program } took too long and was stopped.
player-trouble-tag-too-big = Too big for this: { $bytes }, at most { $most }.
player-trouble-tag-not-an-image = That is not an image this player can put in a file.
player-trouble-tag-bad-number = Not a number (1, or 1/12).
player-trouble-tag-bad-date = Not a date (2024, 2024-05 or 2024-05-31).
player-trouble-tag-bad-isrc = Not an ISRC (two letters, three letters or digits, seven digits).
player-trouble-tag-bad-language = Not a language code (pt, por, pt-BR).
player-trouble-tag-read-only-view = Read to be shown: it changes through its field, or through the file's own tag above.
player-trouble-tag-bad-name = Not a name this tag takes.
player-trouble-tag-not-text = Its value is data, not text: it is not written here.
player-trouble-tag-not-global = It belongs to one track, chapter or attachment, or is nested with other tags: only the whole file's tags, each on its own, are written here.
player-trouble-tag-no-such-thing = That picture or stream is not in the file any more.
player-trouble-tag-taken = A cover is already beside the file under both names it could take.
player-trouble-tag-cancelled = Stopped; the file is as it was.
player-container-mpeg = MPEG audio
player-container-aac = AAC
player-container-flac = FLAC
player-container-ogg-vorbis = Ogg Vorbis
player-container-opus = Opus
player-container-speex = Speex
player-container-ogg-other = Ogg
player-container-mp4 = MP4
player-container-wav = WAV
player-container-aiff = AIFF
player-container-ape = Monkey's Audio
player-container-mpc = Musepack
player-container-wavpack = WavPack
player-container-matroska = Matroska
player-container-webm = WebM
player-container-avi = AVI
player-container-mpeg-ts = MPEG-TS
player-container-flv = Flash video
player-container-asf = ASF
player-container-other = another container
player-tag-title = Title
player-tag-artist = Artist
player-tag-album = Album
player-tag-album-artist = Album artist
player-tag-track = Track number
player-tag-disc = Disc number
player-tag-date = Date
player-tag-genre = Genre
player-tag-composer = Composer
player-tag-lyricist = Lyricist
player-tag-comment = Comment
player-tag-lyrics = Lyrics text
player-tag-publisher = Record label
player-tag-isrc = ISRC
player-tag-description = Description
player-tag-show = Series
player-tag-season = Season
player-tag-episode = Episode
player-tag-director = Director

## file info editing — everything the info panel can change

files-detail-name = name
files-fact-edits = { $label }: { $value } — press to change it
files-fact-takes-place-out = { $label }: { $value } — press to take the place out of the file
files-meta-title = title
files-meta-author = author
files-meta-description = description
files-meta-keywords = keywords
files-meta-copyright = copyright
files-meta-taken = made on
files-fact-tags = tags
files-fact-details = details
files-tags-absent = the tag reader is not installed here
files-tags-stopped = the tag reader stopped before it answered
files-tag-cannot = { $label } cannot be changed here: { $why }
files-tag-several-lines = { $label } has several lines; it is edited in the player's editor, not on one line
files-modified-bad = That is not a date this can read. Write it as 2026-09-30 14:05, seconds optional.
files-ask-location = { $n ->
        [one] Take the place out of { $name }? It rewrites the file and cannot be undone.
       *[other] Take the place out of these { $n } files? It rewrites each one and cannot be undone.
    }
files-location-copy = write a copy without the place, beside it
files-location-overwrite = take it out of the file itself — cannot be undone
files-location-copied = { $n ->
        [one] A copy without the place: { $name }
       *[other] { $n } copies without the place, each beside its original
    }
files-location-removed = { $n ->
        [one] The place is gone from { $name }.
       *[other] The place is gone from { $n } files.
    }
files-amend-too-long = That is longer than a detail can be ({ $most } bytes); nothing was written.
files-amend-bad-date = That is not a date: write it as 2026-09-30 14:05. Nothing was written.
files-amend-not-written = exiftool wrote nothing: this file cannot take that here. It is as it was.
files-amend-taken = A file with the copy's name is already there; nothing was written.
# Why a file's tags cannot be written, said precisely (`ronoctua_tags::Why`).
player-tag-format-id3v2 = An ID3v2 tag
player-tag-format-id3v1 = An ID3v1 tag
player-tag-format-ape = An APEv2 tag
player-tag-format-vorbis = A Vorbis comment
player-tag-format-mp4 = An MP4 tag
player-tag-format-riff-info = A RIFF INFO list
player-tag-format-aiff-text = An AIFF text chunk
player-trouble-tag-no-field-in = { $format } has no place for { $field }.
player-trouble-tag-no-pictures-in = { $format } holds no pictures: this file's cover cannot be put in or taken out.
player-trouble-tag-streams-only-matroska = A stream's language, name and marks are written only in Matroska, and this file is { $container }.
player-trouble-tag-attachment = An attachment — a picture, a font — is kept in the file and not played: it has no stream's language or name to change.
player-trouble-tag-read-only = { $format }: only read here, never changed on its own.
player-trouble-tag-only-the-front-cover = Only the front cover is replaced or taken out here; the file's other pictures stay as they are.
player-trouble-tag-chapters-only-matroska = A chapter's title is written only in Matroska, and this file is { $container }.
player-trouble-tag-remux-would-drop = This file is too big to be written in place, and the copy that writes it would lose items it keeps beyond these fields. Its fields stay as they are.
