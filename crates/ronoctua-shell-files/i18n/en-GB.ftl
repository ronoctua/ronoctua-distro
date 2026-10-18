# The texts `ronoctua-shell-files` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

files-enter = open
files-parent = up one directory
files-hint = type to filter
# THE LISTING'S OTHER SHAPE. What the glyph at the end of the path's line says
# under the pointer — where it GOES, never where it is.
files-grid-on = the files are side by side, in thumbnails
files-grid-off = the files are one to a line again
# WHAT THE SIZE RING SAYS IT LANDED ON. Three keys and not one selector: a
# selector picks its branch by the TYPE of its argument, and the line this
# reaches is `Step::Say`, whose arguments are strings by construction — so a
# number would arrive as text and every screen would read the default branch.
files-grid-small = small thumbnails
files-grid-medium = medium thumbnails
files-grid-large = large thumbnails
files-grid-moves = this one moves — it is holding still because others are
files-grid-bigger = show them bigger
files-grid-none = the entries are one to a line, so there is no thumbnail to size
files-no-root-helper = changing the owner needs the installed system — the ronoctua-files-root unit is not here
files-menu-on = actions on { $name }
files-menu-on-many = actions on { $n } marked
files-ask-application = Open with which application?
files-ask-owner = Change the OWNER to whom?
files-ask-group = Change the GROUP to which?
files-ask-mode = Change the PERMISSIONS to which set?
files-ask-name = Rename to:
files-sort = sort order
files-sort-reverse = reverse the order
files-mark = mark
files-mark-all = mark everything
files-unmark-all = unmark everything
files-menu = action menu
files-measure = measure
files-pane-next = next section
files-pane-previous = section before
# AND THE SAME TWO ARROWS, where the entries are side by side. A different
# thing, so a different word: they cross the SECTIONS on a line and walk the
# ENTRIES on a shelf.
files-grid-next = next file
files-grid-previous = file before
files-type-is = { $named }
files-type-disagrees = the name says { $named }, the content says { $found }
files-nothing-here = this directory is empty
files-header = column names
files-sidebar-focus = go to the sidebar, and back
files-preview-left = preview left
files-preview-right = preview right
files-grid = thumbnails
files-grid-size = thumbnail size
files-eject = unmount
files-unpin = unpin
files-sidebar-hidden-ask = Bring which one back?
files-sidebar-trash = trash
files-sidebar-open = open { $name }
files-sidebar-mount = mount { $name } and go in
files-sidebar-unmount = unmount { $name }
files-sidebar-launch = run { $name }
files-sidebar-hidden = { $name } is hidden — { $key } brings it back
files-sidebar-gone = { $name } is no longer on the sidebar
files-sidebar-nothing-hidden = there is nothing hidden to bring back
files-mount-failed = could not mount { $name }: { $why }
files-mount-no-driver = this system cannot read a { $type } filesystem
files-mount-needs-admin = { $name } belongs to the system; { $key } opens a root terminal
files-unmount-busy = { $name } is in use — close what is open on it
files-unmount-needed = { $name } is where the system lives ({ $why }); unmounting is not offered
sidebar-menu-on = what to do with { $name }
sidebar-fact-where = mounted at
sidebar-fact-needed = this is where the system lives
sidebar-not-mounted = not mounted
files-does-open-map-nowhere = this file has no coordinate written in it
files-disks-not-watching = the disks will not refresh on their own: { $why }
files-nothing-to-choose = there is nothing to choose from
files-no-pkexec = `pkexec` was not set in nix/rust.nix
files-no-systemctl = `systemctl` was not set in nix/rust.nix
# What each field says to the pointer. A field that lights up and explains
# nothing promises an action and describes none.
# THE WHOLE NAME, and only the name.
#
# The line stands in for a column that had to cut, so it says what the column
# would have said and nothing else: a sentence around it would push the end of
# a long name back off the edge, which is the thing this exists to undo.
files-explain-name = { $name }
# THE EXECUTE BIT, which is the one thing the type's glyph says besides the
# type. The owner asked for the colour to move off the name and onto the
# glyph, and a colour with no words is a legend nobody reads — so the pointer
# resting on a green glyph says what the green MEANS.
#
# It names the bit and not what running it would do: this manager does not run
# a file by pressing RET on it, and a sentence promising that would be a
# sentence about another program.
files-explain-executable = this file carries the execute bit — the system may run it
files-explain-size = exactly { $bytes } bytes
files-no-trash = I do not know where the trash is; nothing was moved
# ===================================================================
# WHAT IS INSIDE A FILE — the preview and the facts panel
#
# `ronoctua-inspect` is the border with a file's CONTENT, and it does not
# translate: it answers with a text KEY and its arguments, and whoever holds
# the bundle turns the pair into a sentence (invariant 8). Every number below
# arrives as DATA — what is written here is only the words around it.
# ===================================================================

# WHY THERE IS NOTHING TO SHOW. Drawn where the content would be, centred, so
# each one has to say whether there is anything to be done about it: "it
# failed" is not an answer, it is the absence of one.
#
# THE PANEL IS NOT AN IMAGE PANEL, and the sentence that said so is gone. It
# used to answer a markdown file with "there is no picture inside this kind of
# file", which is a true statement about a question nobody asked — the panel
# shows what is IN a file, and a file of letters is full of content. What is
# left here is a file that has neither picture nor text, and it now says what a
# folder says: `files-no-preview`, one sentence for one fact.
files-preview-not-text = this file is not text, whatever its name says
files-preview-empty = there is nothing inside this file
files-preview-unreadable = this file could not be opened for reading
files-preview-missing = { $program } is not installed, and it is what reads this format
files-preview-too-big = too big for a preview ({ $amount })
# NOT "a picture". This is every reader's failure, not the raster path's: a
# Word document whose piece table does not add up, a presentation with no
# records, a program that ran and answered nothing. The owner read it off the
# screen against a `.doc` — a panel telling him a perfectly good document was
# "not a picture" is a sentence about the wrong thing twice over.
files-preview-failed = this file could not be read — it may be damaged
files-preview-slow = it went past { $seconds } s and was stopped — not the size, something is stuck
# NOT "damaged". The file is fine and this reader is not equipped for it, which
# is a different thing to tell somebody: one sends them to repair a file that
# needs no repair.
files-preview-draco = the shapes in this file are compressed, and this reader does not open that
files-preview-no-synth = this file holds a score rather than a recording, and there is nothing here to play it with
files-preview-playlist-unread = this is a list of things to play, in a spelling this reader does not open
files-preview-own-newer = this was saved by a newer player than this one
# THE PANEL'S LEFT COLUMN. Short nouns, and short on purpose: the column is as
# wide as the widest label here, so a needless word costs every other row.
files-fact-dimensions = dimensions
files-fact-duration = duration
files-fact-container = format
files-fact-motion = moving photo
files-fact-video = video
files-fact-audio = audio
files-fact-rate = frame rate
files-fact-sampling = sampling
files-fact-channels = channels
files-fact-bitrate = bit rate
files-fact-title = title
files-fact-artist = artist
files-fact-album = album
files-fact-album-artist = album artist
files-fact-track = track
files-fact-disc = disc
files-fact-year = year
files-fact-genre = genre
files-fact-composer = composer
files-fact-publisher = publisher
files-fact-isrc = ISRC
files-fact-encoder = encoder
files-fact-depth = bit depth
files-fact-layout = channels laid out
files-fact-compression = compression
files-fact-cover = cover
files-fact-camera = camera
files-fact-lens = lens
files-fact-taken = taken
files-fact-iso = ISO
files-fact-shutter = shutter
files-fact-aperture = aperture
files-fact-focal = focal length
files-fact-place = place
files-fact-pages = pages
# AND THE RIGHT ONE, when the number needs a word around it. A unit is language
# business — where it goes, and whether it is a word at all.
files-value-pixels = { $width } × { $height }
files-value-fps = { $fps } fps
files-value-hertz = { $khz } kHz
# ONE, TWO, OR A COUNT. Which numbers earn a word of their own is a decision of
# the LANGUAGE, and this is the shape Fluent has for it — `$n` has to arrive as
# a NUMBER (`Arg::Number`), or every branch but the last is unreachable.
files-value-channels = { $n ->
        [1] mono
        [2] stereo
       *[other] { $n } channels
    }
files-value-kbps = { $kbps } kbit/s
files-value-of-total = { $at } of { $total }
files-value-bits = { $n ->
        [one] { $n } bit
       *[other] { $n } bits
    }
files-value-lossless = nothing thrown away
files-value-lossy = made smaller by throwing sound away
files-value-millimetres = { $mm } mm
files-value-seconds = { $s } s
files-value-coordinates = { $lat }, { $lon }
# The other selector, and a plural one: in en-GB `one` is 1 and nothing else.
files-value-pages = { $n ->
        [one] { $n } page
       *[other] { $n } pages
    }
# === The shape inside a mesh file — `.stl` and `.3mf` ======================
#
# The manager reads the triangles itself and turns them; these are the rows the
# info panel puts beside the picture. They are what somebody about to PRINT the
# thing reads: how big it is, how much material it takes, and whether the
# surface is closed — which is the first question a slicer asks and the one it
# refuses over.
files-fact-format = format
files-fact-unit = unit
files-fact-objects = objects
files-fact-triangles = triangles
files-fact-vertices = vertices
files-fact-bounds = size
files-fact-volume = volume
files-fact-area = surface
files-fact-watertight = surface closed
files-fact-flipped = faces reversed
files-fact-degenerate = flat triangles
files-fact-application = made with
files-fact-designer = designer
files-fact-licence = licence
files-fact-created = created
files-fact-schema = written against
files-fact-source-name = original name
files-fact-solids = solids
files-fact-shells = shells
files-fact-faces = faces
files-fact-edges = edges
files-fact-outlined = faces not filled
files-value-stl-binary = STL, as bytes
files-value-stl-ascii = STL, as words
files-value-three-mf = 3MF
files-value-obj = OBJ
# The same format written two ways, and which one a file is decides whether it
# opens in a text editor. The panel says so for the same reason it says it of
# an STL.
files-value-ply-ascii = PLY, as words
files-value-ply-binary = PLY, as bytes
files-value-off = OFF
files-value-gltf = glTF
files-value-glb = glTF, as one file
files-value-collada = COLLADA
files-value-amf = AMF
files-value-step = STEP
files-value-iges = IGES
# NOT a measurement. glTF's unit is the metre by its own specification, and the
# file says nothing about it — so the panel says where the number came from.
files-value-unit-by-convention = metre, by the format
files-value-count = { $n }
# THE MEASUREMENTS ARE ALWAYS IN MILLIMETRES. A 3MF may declare inches or
# metres and the reader converts on the way in, so this unit is not a guess.
files-value-box = { $w } × { $d } × { $h } mm
files-value-cubic-centimetres = { $n } cm³
files-value-square-centimetres = { $n } cm²
files-value-mesh-closed = yes
# NOT "no". What a person can act on is how many edges are open, because that
# is the number a repair tool reports and the number that changes as it works.
files-value-mesh-open = no — { $n } open edges
about-files-parent = Leave this directory for the one that contains it.
about-files-enter = Open what is under the cursor: go into a directory, or open a file in the program that reads it.
about-files-menu = Open the list of everything that can be done to what is under the cursor, each row showing the key that runs it.
about-files-mark = Mark the row under the cursor, or take the mark off it — what is marked is what the next action acts on.
about-files-mark-all = Mark every entry of this directory at once.
about-files-unmark-all = Take the mark off every entry of this directory.
about-files-header = Show or hide the row that names the listing's columns and carries the arrow saying which one it is ordered by. It starts hidden.
about-files-sort = Change what the listing is ordered by: the name, the size, the date or the kind.
about-files-sort-reverse = Turn the order round, from the last to the first.
about-files-measure = Add up what a directory holds on disk, by walking it.
about-files-pane-next = Give the keyboard to the next section — the listing, the sidebar, the preview and the facts, round.
about-files-pane-previous = Give the keyboard to the section before this one.
about-files-eject = Unmount the removable disk this row stands for, so it can be taken out safely.
about-files-unpin = Take this place off the sidebar. One the distribution ships comes back hidden; one of your own is gone.
about-files-preview-left = Slide the previewed text sideways, towards the beginning of its long lines.
about-files-preview-right = Slide the previewed text sideways, towards the end of its long lines.
about-files-grid-next = Move to the next file along the shelf. With the entries side by side the sideways arrows walk them; TAB still crosses to the next section.
about-files-grid-previous = Move to the file before this one along the shelf. With the entries side by side the sideways arrows walk them; Shift TAB still crosses back a section.
about-files-grid = Show the files side by side as pictures instead of one to a line. The glyph at the end of the path's line does the same, and draws which shape it would go to. It starts off.
about-files-grid-size = Ring through the three sizes a thumbnail is drawn at. On a screen that would hold more pictures than the manager draws at once, the smallest step is the one that is skipped.

## file info editing — everything the info panel can change

files-ask-modified = Modified on (year-month-day hour:minute:second):
files-ask-tag = { $field }:
files-ask-meta = { $detail }:
files-trash-on-volume = Going to { $volume }'s own trash, not the home's: its space stays taken on { $volume } until that trash lets it go — which it does on its own, like the home's, while the drive is plugged in.
files-trash-left-on-volume = { $volume } left with files in its own trash ({ $n }): their space stays taken on it until that trash lets them go, which happens only while it is plugged in.
files-git-why-not-a-repo = This folder is not in a repository.
# An archive's formats and levels, by `ronoctua_archive::Format::id` and `Level::id`.
files-archive-zip = zip
files-archive-7z = 7z
files-archive-tar = tar
files-archive-tar-gz = tar.gz
files-archive-tar-xz = tar.xz
files-archive-tar-zst = tar.zst
files-archive-tar-bz2 = tar.bz2
files-archive-tar-lz4 = tar.lz4
files-archive-rar = RAR
files-archive-iso = ISO image
files-archive-other = another format
files-archive-level-store = no compression
files-archive-level-fastest = fastest
files-archive-level-normal = normal
files-archive-level-maximum = maximum
files-archive-level-ultra = ultra
# The sheet `compress` and `convert` open.
files-compress-title = Compress { $what }
files-convert-title = Convert { $name }
files-sheet-format = format
files-sheet-level = level
files-sheet-password = password
files-sheet-password-none = none
files-sheet-password-set = set, AES-256
files-sheet-hide = hide the names too
files-sheet-split = split into volumes
files-sheet-split-none = no
files-sheet-yes = yes
files-sheet-no = no
files-sheet-go = compress into { $name }
files-sheet-convert-go = convert into { $name }
# Walking into an archive, and the passwords it asks.
files-archive-reading = reading { $name }…
files-archive-opening = taking { $name } out to open it…
files-archive-add-top = members go in at the archive's top — go up to it to add them
files-archive-passwords-differ = the two passwords differ; nothing was set
files-archive-nothing-chosen = nothing chosen to take out
# The panels inside an archive: what its listing says of a member, and why
# nothing is previewed.
files-archive-fact-packed = packed
files-archive-fact-encrypted = encrypted
files-archive-preview = a member is shown once it is opened, after it is taken out of the archive
ask-files-archive-password = The archive's password
ask-files-archive-password-wrong = That was not it — the archive's password
ask-files-archive-password-new = A password for the new archive
ask-files-archive-password-again = The same password again
# An encrypted disk (LUKS) in the sidebar.
files-encrypted-locked = { $name } is encrypted and locked — mounting it asks for its passphrase
files-encrypted-unlocked = { $name } is unlocked; ejecting it locks it again before the power goes
files-encrypted-fact = encryption
files-encrypted-fact-locked = locked — LUKS
files-encrypted-fact-unlocked = unlocked — LUKS
files-unlock-working = unlocking { $name }…
files-unlock-wrong = that passphrase does not open { $name }
files-unlock-failed = { $name } could not be unlocked: { $why }
files-unlock-locked = { $name } is locked again
ask-files-unlock = The passphrase that unlocks this disk
files-private-locked = { $name } is locked: opening it asks for its passphrase, or a touch of its key.
files-private-open = { $name } is open: locking it closes it until the passphrase opens it again.
stick-sidebar-drive = { $name } — the whole drive: press for what can be done with it
