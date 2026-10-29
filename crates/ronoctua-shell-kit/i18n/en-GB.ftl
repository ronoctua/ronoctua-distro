# The texts `ronoctua-shell-kit` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Commands — the `M-x` list and the which-key labels

cmd-buffer-next = Next buffer
cmd-buffer-switch = Switch buffer
cmd-file-save = Save file
cmd-buffer-close = Close buffer
cmd-buffer-terminal = Open terminal
cmd-session-quit = Quit session

## The modeline's two left segments

# THE NATIVE WINDOWS, which are not buffers and reach the same place: the
# modeline turns any `br.ronoctua.<id>` into `mode-<id>`, and it does not care
# whether the window is a wasm component or a process of ours.
mode-files = files
mode-monitor = monitor
mode-notifications = notifications
mode-power = power
mode-bluetooth = bluetooth
mode-display = screens
mode-network = network
mode-history = shell history
mode-rescue = rescue
mode-terminal = terminal
mode-capture = screenshot
mode-example = example
mode-memory = memory
mode-transfers = transfers
mode-format = format
mode-brain = brain
mode-update = update
buffer-no-name = *no name*
mode-unknown = unknown

## Notifications

# The band, above the modeline. The counter only appears with more than one.
notif-queue-position = { $shown }/{ $total }
# THE HINT LINE reads each id of the manager's key tables as a short text —
# the file manager's convention; `cmd-<id>` is the long label the manual reads.
notif-read = read or unread
notif-act = run its action
# What the pointer explains on the BELL, on the modeline — not on the band.
# THE EXACT ZERO COMES FIRST. Fluent matches an exact number before a plural
# category, and Portuguese's `one` category covers 0 as well — without the
# `[0]` branch "0 filtradas" came out as "uma filtrada". Both languages keep
# the same shape so the next person sees the trap here too.
hint-notif-bell = { $unread ->
        [0] none unread
        [one] one unread
       *[other] { $unread } unread
    }, { $filtered ->
        [0] none filtered out
        [one] one filtered out
       *[other] { $filtered } filtered out
    } — click opens the manager
hint-notif-read = mark as read and take it off the screen
hint-notif-open = take it off the band and open it whole in the manager
notif-see-whole = see it whole
hint-notif-countdown = how long it has left
hint-notif-paused = the clock is stopped while the pointer is on it
hint-notif-action = { $action } — a click picks it, and a click on the pick runs it

## Installer — the interface

cmd-window-left = Focus left
cmd-window-move-left = Move left
cmd-workspace-go = Go to workspace

## Compositor commands — §13, "every OS action has a command"

# THE KEYS THAT WALK THE LINE ONCE IT IS OPEN, and the sentence that teaches
# them. They are a chord of the RIGHT Ctrl, which is the one family no view
# sees — so they are in no registry and in no manual, and this sentence is the
# only thing on screen that says they exist. It is the lesson the file
# manager's `C-o` cost this project — implemented, working, and named nowhere,
# which is the same as not having it; the key is gone now and the lesson is
# why that view's hint line is generated out of its key table. And it is the
# reason this sentence is APPENDED to the hint rather than standing on its own:
# a key taught on a line nobody has opened teaches nothing.
#
# The notation is a value and not a `key_that_runs`, exactly as
# `expansion-page` is: a right-Ctrl chord is not a command of the registry, so
# there is no table to read it back out of, and the two words in it are
# translated.
minibuffer-fold-keys = RCtrl ↑↓
minibuffer-fold-hint = · { $keys } move through the rest of this line
about-fold-walk = Move through the rest of this opened line, a row at a time; going back past its first row closes it.
echo-copied = Copied
echo-image-copied = The image is on the clipboard
echo-copied-value = Copied: { $value }
echo-copy-failed = Could not copy — see the log

## The leader's three sets: the key tapped right after the leader.

# Alone, a key that has a footer and a window opens the footer; after a tap
# of Alt it opens the window; after a tap of Ctrl the whole tree is there. The
# short word is what the hint line draws beside the key; `cmd-` is what the
# pointer reads, and what `LEADER :` lists.
leader-set-window = windows
leader-set-full = everything
# ISO order in both languages: it sorts on its own and never confuses a day
# with a month, which is the only way a column of dates does not lie.
files-when = { $date } { $time }
files-when-utc = { $date } { $time } UTC
# THE SAME INSTANT, WRITTEN OUT. The column's form is `files-when`; this is
# what the pointer asks for, where there is a line and the question was which
# day and which second.
files-when-long = { $date } at { $time }
# THE VIEW'S OWN KEYS, and every one of these sentences is now ON SCREEN.
#
# They live in a table (`OF_THE_VIEW`) rather than inside a `match`, because
# the list is what the check reads: a key that only exists in a `match` arm is
# a key nothing can enumerate. For a long time that was the only reader —
# `files::OF_THE_VIEW` is in no registry, so none of these was in `M-x` or in
# the manual, and forty translated sentences were carried by both languages and
# drawn by nothing. `View::hint` reads them, in the table's order.
#
# SO THEY ARE SHORT AND THEY CARRY NO COMMA. The owner's rule for that line —
# one shortcut between any two separators, the key first, and the words after
# it brief. A comma inside one of these reads on screen as the end of the
# chunk, which is what the separator is for.
files-down = down
files-up = up
# The panels. `no-room` is what the toggle says when it turns one on at a width
# that cannot hold it: the state flips anyway and the screen says why, which is
# the shape `files-sidebar-only` already has. `says` is one fact in full on the
# message line — a panel is twenty-four cells and a path is two hundred
# characters, so the row that had to be cut is the row somebody wants to read.
files-panel-no-room = there is no room for the panel at this width — it needs { $cells } columns
files-fact-says = { $label }: { $value }
files-preview-all-shown = the whole preview is already on screen
files-playlist-previous = previous track
files-playlist-next = next track
files-first = the first
files-last = the last
cmd-files-open = File manager
cmd-files-window = File manager, in a window
group-file = file
# THE SIDEBAR — where one starts from.
#
# The first five are ids of `OF_THE_VIEW` and of `menu.rs`;
# `files-actions-have-text` derives them from the id on its own.
files-preview = preview
files-facts = information
files-columns = permissions and date
files-sidebar = sidebar
files-theatre = expand the preview
files-sidebar-only = this key acts on a sidebar row — { $key } goes there
# The table's header. Clicking one sorts by it; clicking again reverses. The
# `s` key goes round the same cycle.
files-column-icon = kind
files-column-name = name
files-column-perms = permissions
files-column-owner = owner
files-column-group = group
files-column-size = size
files-column-modified = modified
files-explain-owner = { $who } owns it; clicking changes that
cmd-atlas-open = world data
cmd-atlas-window = world data in a window
transfers-pause = hold or release this transfer
cmd-transfers-pause = hold or let go
# THE PAGE ITSELF.
brain-title = Brain
brain-why-no-systemctl = No systemd here
brain-why-runaway = It stopped reporting and the waiting was given up. It may well still be going.
brain-why-aborted = You stopped it. Nothing went wrong.
brain-why-postponed = You said not now, and the next moment moved.
brain-why-cpu = The processor was busy
brain-why-battery = It was on battery
brain-why-busy = Something else was running
brain-why-network = There was no network
brain-why-asleep = The machine was asleep
echo-brain-unreachable = The brain is not answering

## `theme-backdrop-near`, is offered only under a light theme.

theme-option-footer = Footer
theme-option-windows = Windows
theme-option-borders = Borders
theme-option-gaps = Spacing
theme-backdrop-black = Black
theme-backdrop-white = White
theme-backdrop-near = Near white
theme-backdrop-theme = The theme's colour
theme-ground-theme = The theme's
theme-ground-desk = The desk's
theme-borders-standard = Standard
theme-borders-discreet = Discreet
theme-gaps-with = With gaps
theme-gaps-none = No gaps
echo-theme-unsaved = the theme could not be saved: { $error }

## footer, windows, borders) and `$value` what it said — the file's words.

echo-theme-refused = { $file } says { $field } is “{ $value }”, which is not an option here — refused
echo-theme-unreadable = { $file } could not be read, so every choice in it was refused: { $error }

## `echo-theme-unreadable`).

echo-theme-kept = { $file } was not written, so nothing in it is lost; fix it or remove it by hand: { $why }

## Power — the battery, the charger, the profile and sleep (LEADER s p)

network-import = bring in
ask-network-import = Path of the .conf or .ovpn file
network-connect = connect

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

ask-bluetooth-accept = The code it asks for — a PIN, or six digits

## Screens — the monitors, their pictures and their light (LEADER s d)

mode-keyboard = keyboards
keyboard-key-capslock = Caps Lock
keyboard-key-leftalt = the left Alt
keyboard-key-leftmeta = the left Super
keyboard-key-rightalt = AltGr
keyboard-key-rightmeta = the right Super
keyboard-key-menu = the Menu key
keyboard-key-insert = Insert
keyboard-choice-default = itself
keyboard-choice-esc = Escape
keyboard-choice-control = Control
keyboard-choice-control-esc = Control held, Escape tapped
keyboard-choice-meta = Super
keyboard-choice-alt = Alt
keyboard-choice-backspace = Backspace
keyboard-choice-capslock = Caps Lock
keyboard-choice-noop = nothing at all
locale-clock-sample-24 = { $hh }:{ $mm }
locale-clock-sample-12 = { $hh }:{ $mm } { $half }
locale-clock-exact-24 = { $hh }:{ $mm }:{ $ss }
locale-clock-exact-12 = { $hh }:{ $mm }:{ $ss } { $half }
locale-clock-am = am
locale-clock-pm = pm
locale-date-ymd = { $y }-{ $m }-{ $d }
locale-date-dmy = { $d }/{ $m }/{ $y }
locale-date-mdy = { $m }/{ $d }/{ $y }
# THE TWELVE MONTHS, AS ONE KEY. A Fluent selector, because which numbers get a
# word of their own is the LANGUAGE's business — the same argument the channel
# counts already make. The month arrives as a NUMBER: `"01"` does not match `[1]`.
locale-month =
    { $m ->
        [1] January
        [2] February
        [3] March
        [4] April
        [5] May
        [6] June
        [7] July
        [8] August
        [9] September
        [10] October
        [11] November
       *[12] December
    }
# A DATE WRITTEN OUT, one key per order.
#
# The order is still the person's: somebody who chose "month first" and got
# "3 September" would read a preference being ignored. Each language writes its
# three the way that language writes them, and the day and the year come
# UNPADDED — `03 September` is a column's spelling, and this is not a column.
locale-date-long-ymd = { $y }, { $month } { $d }
locale-date-long-dmy = { $d } { $month } { $y }
locale-date-long-mdy = { $month } { $d }, { $y }
# THE COMMAND'S OWN WORDS, drawn on the hint line: `z` asks for the zone
# THIS INTERFACE reads a timestamp in, which is not the machine's.
locale-zone = This interface's zone
mode-geo = regions
ask-locale-system-zone = The machine's time zone, such as Europe/Lisbon
ask-locale-zone = Zone for this interface's clock, such as Europe/Lisbon
ask-locale-place = Zone of the place to add, such as Europe/Lisbon
# THE TWO LETTERS, and they had no short text at all: the hint line read
# `[locale-place]` and `[locale-where]` on screen, which is the missing-key
# marker doing duty as a word. `cmd-locale-place` has been there all along —
# it is the LONG form, and the line reads the short one.
locale-place = add a place
# The two windows that are not wasm buffers and reach the same modeline.
# `daemon::mode_label` turns `br.ronoctua.<id>` into `mode-<id>` whatever the
# window is made of; without these the modeline drew `[mode-audio]`.
mode-audio = sound
mode-syncthing = syncthing
mode-pointers = pointers
mode-locale = language/region
# ---------------------------------------------------------------------------
# WHAT A SHORTCUT DOES, IN FULL — the line under the hint.
#
# The hint line teaches forty keys in three words each, which is all a line
# holds. Resting the pointer on one of them opens a second line underneath with
# the whole sentence, and this is where those sentences live.
#
# TWO NAMESPACES, AND THE OTHER ONE IS `cmd-`. A key that is a command of the
# registry already has its long form there — that is what which-key and the
# manual read, and a hundred of these ids have one. What is left are the keys
# that belong to a VIEW and to no registry: the file manager's, the transfer
# panel's, and one of the notification manager's. Writing `cmd-files-preview`
# would say that id is a registered command, which is exactly what `cmd-` means.
#
# `every-hint-key-explains-itself` refuses an id with neither and an id with
# both — two sentences for one key is two places to keep in step.
#
# THE FILE MANAGER'S LISTING.
about-files-up = Move the cursor one row up, in whichever section has the keyboard.
about-files-down = Move the cursor one row down, in whichever section has the keyboard.
about-files-first = Take the cursor to the first row of the listing.
about-files-last = Take the cursor to the last row of the listing.
about-files-columns = Show or hide the permission bits, the owner, the group and the modification date on every row of the listing.
# THE TWO COLUMNS ON THE RIGHT, and the sidebar on the left.
about-files-preview = Show or hide the preview column — the picture, the film, the text or the shape the file under the cursor holds.
about-files-facts = Show or hide the column of facts about the file under the cursor: its size, its dates, its type and what the file itself carries.
about-files-sidebar = Show or hide the column of places — your ten directories, and the disks that are plugged in.
about-files-theatre = Give the preview the whole region, and give the region back. With nothing selected there is no preview to expand.
about-files-playlist-previous = Show the entry before this one in the playlist.
about-files-playlist-next = Show the next entry of this playlist.
about-transfers-pause = Hold this transfer where it is, or let it carry on.
mode-camera = camera
mode-player = player
mode-software = software
mode-camera-settings = camera settings
mode-manual = manual

## The way back, first on every view's line: Esc, and a click on it

view-back = back
about-view-back = Goes back one level, and from the first one closes this view — the same as pressing Esc.

## A question on a window's header line — what is typed there

line-cancel = cancel
about-line-cancel = Lets the question go without answering it — the same as pressing Esc.
line-answer = answer
about-line-answer = Answers with what is typed on the line — the same as pressing Enter.

## The window menu — the modeline's app name, clicked (LEADER w o)

ask-software-web-new = Address of the site
cmd-software-web-address = Change the web app's address
ask-software-web-address = Address it opens
cmd-software-web-name = Change the web app's name
ask-software-web-name = Name in the launcher
# The core's own words: why a row has no moment, why an action was refused,
# and why a module's journal is not being followed.
brain-why-quiet = Only when asked
brain-why-unknown = It does not know how to do that
brain-why-refused = The machine refused it
brain-why-rehearsing = This brain only rehearses: it changes nothing on the machine
brain-journal-why-no-program = This brain was not given the journal to read
brain-journal-why-unstartable = The journal's reader could not be started
brain-journal-why-unreadable = The journal could not be read
brain-journal-why-ended = The journal stopped being followed; it will be tried again
echo-brain-not-offered = The notice does not offer that
echo-brain-dismissed = Put away
echo-brain-taken = The brain took it
echo-brain-refused = The brain would not: { $why }
# THE LEADER'S SETS AND THE BARS' MODES (2026-09-30): the leader pressed again
# walks the sets, and a tap of Ctrl at the root walks the bars' three modes.
leader-set-default = default
# THE MINIBUFFER'S DRAWER by name, and the compositor's transient keys for it.
cmd-minibuffer-fold-open = Open the minibuffer's line to read the rest of it, or walk down it
cmd-minibuffer-fold-close = Walk up the opened minibuffer's line, or close it
# The modeline's name for its window (`br.ronoctua.footer-settings`).
mode-footer-settings = footer settings

## file info editing — everything the info panel can change

# The browser's window in the modeline (`br.ronoctua.git`, `daemon::mode_label`).
mode-git = git history
# What the modeline names the app's window (`br.ronoctua.icons`).
mode-icons = icons
# The modeline's name for its window (`br.ronoctua.calc`).
mode-calc = calculator
calc-rates-for = Converting currencies needs today's exchange rates from the European Central Bank.
mode-video-metrics = video metrics
brain-why-saver = The power saver was on
mode-security = security centre
# The security centre's refusals (`ronoctua_security::Why`), one per reason.
security-refusal-no-helper = This session was not given the security helper, so nothing here can act as administrator.
security-refusal-wrong-password = That is not your password.
security-refusal-needs-password = This needs your password.
security-refusal-not-allowed = Your account may not act as administrator.
security-refusal-unstartable = A program it needs could not be started.
security-refusal-failed = It did not work; the system log says why.
security-refusal-invalid = What was asked is not something the security centre accepts.
security-refusal-no-reader = There is no fingerprint reader.
security-refusal-unreadable = It could not be read.
security-refusal-busy = Another program is using the fingerprint reader.
security-refusal-timed-out = Nothing touched the reader in time.
ask-security-vt-key = Your VirusTotal API key
security-fw-profile-home = home: trusted
security-fw-profile-public = public: strangers around
security-fw-log-off = not logged
security-fw-log-quiet = a few a minute
security-fw-log-chatty = many a minute
security-finger-left-thumb = left thumb
security-finger-left-index-finger = left index finger
security-finger-left-middle-finger = left middle finger
security-finger-left-ring-finger = left ring finger
security-finger-left-little-finger = left little finger
security-finger-right-thumb = right thumb
security-finger-right-index-finger = right index finger
security-finger-right-middle-finger = right middle finger
security-finger-right-ring-finger = right ring finger
security-finger-right-little-finger = right little finger
security-finger-step-completed = Enrolled.
security-finger-step-stage-passed = Good. Touch the reader again.
security-finger-step-retry-scan = Touch the reader again.
security-finger-step-swipe-too-short = Too short; touch again, for longer.
security-finger-step-not-centred = Not in the middle; touch again, centred.
security-finger-step-remove-and-retry = Lift the finger and touch again.
security-finger-step-too-fast = Too fast; touch again, slower.
security-finger-step-duplicate = This finger is enrolled already.
security-finger-step-data-full = The reader holds no more fingers.
security-finger-step-failed = The reader could not enrol it.
security-finger-step-disconnected = The reader went away.
security-finger-step-unknown = The reader said something unexpected.
security-device-secure-boot-enabled = on
security-device-secure-boot-disabled = off
security-device-secure-boot-setup-mode = in setup mode: keys can be changed
security-device-secure-boot-unsupported = not a UEFI start
security-device-tpm-absent = none
security-device-tpm-present = version { $version }
security-privacy-mac-permanent = the device's own: networks can follow you
security-privacy-mac-preserve = as the device has it
security-privacy-mac-random = new every time
security-privacy-mac-stable = its own for this network
security-privacy-mac-explicit = one you chose
security-privacy-mac-default = the system's choice
security-privacy-dns-yes = always
security-privacy-dns-opportunistic = when the server offers it
security-privacy-dns-no = never
security-privacy-dns-default = the system's choice
security-online-refusal-no-key = { $service } needs your key: paste it on its row.
security-online-refusal-key-refused = { $service } refused the key.
security-online-refusal-too-many-asks = { $service } says to ask more slowly; try again in a minute.
security-online-refusal-nothing-found = { $service } found nothing.
security-online-refusal-nothing-to-ask = That is not something { $service } can be asked.
security-online-refusal-unreachable = { $service } could not be reached.
security-online-refusal-malformed = { $service } answered something this does not read.
security-online-refusal-too-large = Too large for { $service }.
security-online-refusal-unreadable = The file could not be read.
security-online-refusal-not-let = { $service } may not be asked: its host is not let.
security-service-virustotal = VirusTotal
security-service-xposedornot = XposedOrNot
security-service-pwnedpasswords = Pwned Passwords
security-vt-state-asking = asking…
security-vt-state-unknown = never seen, not sent
security-vt-state-sending = sending, { $percent }%
security-vt-state-pending = sent; waiting for the analysis
security-vt-verdict-clean = clean: none of { $engines } engines found anything
security-vt-verdict-suspicious = suspicious: { $found } of { $engines } engines
security-vt-verdict-malicious = malicious: { $found } of { $engines } engines
security-rule-word-allow = allow
security-rule-word-deny = deny
mode-containers = containers
# The modeline's name for its window (`br.ronoctua.belongings`).
mode-belongings = belongings
# THE ROW THAT ASKS FOR A HOST (`consent_rows.rs`), the same in every app:
# what pressing it does, what it is for, and why a press was not kept
# (`ronoctua_consent::Unkept`).
consent-ask = Ask { $hosts }
consent-hosts-and = { $hosts } and { $last }
consent-note = { $what } Pressing allows it.
consent-allow = allow
consent-unkept-no-home = There is no home folder to keep the permission to ask { $hosts } in.
consent-unkept-unparseable = The permission file (consent.json) does not parse; it was left as it is, and { $hosts } was not let.
consent-unkept-unwritable = The permission to ask { $hosts } could not be written.
# What the modeline names the search's windows (`br.ronoctua.fast-access`,
# and the one over everything).
mode-fast-access = search
mode-fast-access-float = search
mode-clipboard = clipboard
# The network's and the tunnels' refusals, each by its name
# (`network_words::refused`, `network_words::tunnel_refused`): the daemons'
# own words go to the journal, never to the line.
network-refused-authority = this session is not allowed
network-refused-absent = NetworkManager is not there, or the connection is gone
network-refused-invalid = a setting was not accepted
network-refused-secrets = no password was given, or it was refused
network-refused-failed = the connection could not be made
network-refused-timed-out = no answer in time
network-refused-no-bus = the system bus could not be reached
network-refused-no-radio = this machine has no Wi-Fi radio
network-refused-out-of-range = that network is no longer in range
network-refused-no-device = there is no device of that kind
network-refused-no-profile = there is no saved profile by that name
network-refused-import = the file is not a tunnel NetworkManager can import
network-refused-vault = the TPM would not keep or give back the password
network-refused-other = it did not work
# A question that offers answers for what is typed.
line-offers = { $keys }: { $offers }
network-refusal-no-phone = NetworkManager has no Bluetooth device for this phone: it is not paired, or it does not share its connection
network-refusal-no-vpn = there is no VPN by that name any more
network-refusal-not-vpn = that profile is not a VPN
# THE 2026-10-11 WAVE, the sudo door's root side. Why a root gesture
# did not happen (`ronoctua_security::Why`), beside the others.
security-refusal-cancelled = Cancelled: nothing was done.
security-refusal-no-key = No security key is plugged in.
security-sudo-no-limit = no limit
mode-stick = installer stick

## asked, above the modeline — who asks, what for, and what sudo waits for.

security-refusal-no-footer = The footer, where the administrator is asked, is not running: nothing was done.
mode-incus = Incus
# The modeline's name for its window (`br.ronoctua.agents`).
mode-agents = LLM accounts
