# The texts `ronoctua-shell-door` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Notifications

notif-action-open-video = Open the recording
notif-memory-killed-body = An application was closed to keep the machine responsive.
# What each notification lets you do.
notif-action-edit = Edit the image
notif-action-open = Open the image
notif-action-folder = Open the folder
notif-action-copy-image = Copy the image
notif-action-copy-path = Copy the path
notif-action-memory-panel = Open the memory panel
notif-shared-written = It is in the repository. Your commit is what publishes it.
notif-share-failed = It could not be written into the repository.
notif-share-nowhere = This machine has no repository to share into.
# THE TWO SWITCHES THAT DO NOTHING, and why. A switch that switches nothing off
# and says nothing is the trap this project has already paid for once.
notif-inert-vibrate = no haptic device on this machine
notif-inert-audio = not implemented yet — see docs/PENDING.md

## Installer — the interface

echo-no-opened = Could not open { $program }

## Compositor commands — §13, "every OS action has a command"

clipboard-sync-down = cannot reach the server
ask-clipboard-cap = The largest copy that is sent, in bytes (0 for the server's own limit)
ask-clipboard-rounds = How many rounds the key is derived over
ask-clipboard-salt = The salt, the same on every device on this server

## Network — Wi-Fi and Ethernet (LEADER s n)

ask-network-connect = Password for this network
network-open = open

## The memory panel — LEADER s m

cmd-memory-refresh = Refresh now
cmd-memory-scan = Scan
# The VIEW's keys. The id IS the key, as in the file manager: `cmd-` belongs
# to the command registry, and these are not in it.
transfers-down = down
transfers-up = up
transfers-cancel = give this transfer up
transfers-forget = take this row off the list
transfers-sweep = take everything finished off the list
transfers-launch = open what was installed
transfers-detail = the transfer, field by field
transfers-offline = the transfer queue is not up
# THE WINDOW'S commands: the panel's table, bound under the local leader.
cmd-transfers-down = the next transfer
cmd-transfers-up = the previous transfer
cmd-transfers-cancel = give up
cmd-transfers-forget = drop from the list
cmd-transfers-sweep = clear what has finished
cmd-transfers-launch = open what it installed
cmd-transfers-detail = this row, in detail
transfers-work-region = install a region's streets
transfers-work-software = change a program
brain-log = Live log
# AND WHY A RUN DID NOT HAPPEN AT ALL. `aborted` is NOT an error — it is the
# person having decided, and the screen agreeing with them.
brain-why-off = Switched off
# AND WHAT THE TWO ANSWERS COST. `not now` is not "never": the run leaves the
# queue and the next moment moves, which is the only thing that actually stops
# a countdown — taking it out of the queue alone would put it straight back in
# on the next beat, and the person would answer the same question every half
# minute.
echo-brain-postponed = Not now — it comes back at its next moment
echo-brain-proceeding = Starting now
echo-brain-idle = There is nothing about to happen
brain-abort = stop
brain-proceed = start now

## Power — the battery, the charger, the profile and sleep (LEADER s p)

# What the doors say when a daemon refused, and what the window cannot do.
door-no-footer = The footer could not be reached to run this
notif-bluetooth-pairing-body = Open Bluetooth to answer.
notif-action-bluetooth-accept = Yes, pair it
notif-action-bluetooth-refuse = No, refuse it
notif-bluetooth-connected-summary = { $device } connected
notif-bluetooth-connected-body = It is ready to use.
notif-bluetooth-disconnected-summary = { $device } disconnected
notif-bluetooth-disconnected-body = It may have been switched off, run out of battery, or moved out of range.
notif-bluetooth-battery-low-summary = { $device } is running out
notif-bluetooth-battery-low-body = { $percent }% left. Charge it when you can.
notif-battery-low-summary = Battery low
notif-battery-low-body = { $percent }% left. Plug in when you can.
notif-battery-critical-summary = Battery critical
notif-battery-critical-body = { $percent }% left. The machine will { $action } on its own soon.
notif-battery-charged-summary = Battery charged
notif-battery-charged-body = You can unplug.
notif-action-power-panel = Open the power panel
notif-action-bluetooth-panel = Open Bluetooth
notif-camera-in-use-summary = A camera is open by { $programs }
notif-camera-in-use-body = It stays open until that program lets it go.
notif-camera-free-summary = No program has a camera open any more
notif-camera-free-body = Every program that had one open has let it go.
notif-microphone-in-use-summary = A microphone is listening
notif-microphone-in-use-body = A program opened it. The sound view says which.
notif-microphone-free-summary = No microphone is listening any more
notif-microphone-free-body = Every program that was listening has stopped.
notif-studio-recording-started-summary = The camera is recording
notif-studio-recording-started-body = It records until it is stopped.
notif-studio-recording-stopped-summary = The camera stopped recording
notif-studio-recording-stopped-body = The recording has ended.
notif-studio-streaming-started-summary = The camera is streaming
notif-studio-streaming-started-body = What it films goes out live until the stream is stopped.
notif-studio-streaming-stopped-summary = The stream has ended
notif-studio-streaming-stopped-body = Nothing goes out any more.
notif-network-connected-summary = Connected to { $network }
notif-network-connected-body = The machine has a way out through it.
notif-network-lost-body = The machine is no longer connected through it.
notif-airplane-on-summary = Aeroplane mode is on
notif-airplane-on-body = Every radio and cable is off.
notif-airplane-off-summary = Aeroplane mode is off
notif-airplane-off-body = The radios and the cable are back on.
notif-vpn-up-summary = { $vpn } is up
notif-vpn-up-body = What leaves the machine goes through the tunnel.
notif-vpn-down-summary = { $vpn } is down
notif-vpn-down-body = What leaves the machine no longer goes through the tunnel.
notif-tailnet-up-summary = On the tailnet
notif-tailnet-up-body = This machine reaches the others on it.
notif-tailnet-down-summary = Off the tailnet
notif-tailnet-down-body = The other machines on it are out of reach.
notif-tor-up-summary = Going out through Tor
notif-tor-up-body = There is a circuit through Tor.
notif-tor-down-summary = No longer going out through Tor
notif-tor-down-body = There is no circuit through Tor any more.
notif-charger-plugged-summary = Charger plugged in
notif-charger-plugged-body = The power comes from the charger.
notif-charger-unplugged-summary = Charger unplugged
notif-charger-unplugged-body = The machine runs on its battery.
echo-view-busy = another view has the footer: Esc closes it first
network-tor-open = settings

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

keyboard-mode = Keyboards
# THE TRANSFER QUEUE.
about-transfers-up = Move the cursor one row up the queue.
about-transfers-down = Move the cursor one row down the queue.
about-transfers-cancel = Ask whether to give this transfer up: what has been written so far would be deleted.
about-transfers-detail = Open this transfer one field a line: what moves it, who asked, the whole failure. Pressed there, back to the list.
about-transfers-forget = Take this row off the list. The file it wrote stays where it is.
about-transfers-sweep = Take every finished row off the list at once.
about-transfers-launch = Open the program this row installed, from the entry it put in the launcher.
pointers-on = on
pointers-off = off
# ---------------------------------------------------------------------
# THE RIGHT COLUMN'S OWN LINE (`crate::column`): its sizes, and the keys
# that change them, after its tenant's own
# ---------------------------------------------------------------------
column-wider = wider
column-narrower = narrower
column-close = close
about-column-wider = One size wider; from the bar, back to the size it had
about-column-narrower = One size narrower, down to the bar
about-column-large = Large size
about-column-medium = Medium size
about-column-small = Small size
about-column-bar = Bar
about-column-close = Close the column; what it shows goes on
# The close glyph's own sentence, under the pointer on the control line —
# as short as the sizes', because the line is a small column's.
about-column-close-glyph = Close
column-bar-not-offered = This column has nothing made for the bar yet
column-no-room = This screen has no room for a wider column
# The two refusals again, under the pointer on the control line: the room
# there is what a small column leaves beside its glyphs.
about-column-no-room = No room here
about-column-bar-not-made = No bar yet
ask-player-enqueue = Add to the queue (a path or an address)

## A question on a window's header line — what is typed there

door-paste-failed = What was copied could not be read.

## The window menu — the modeline's app name, clicked (LEADER w o)

notif-action-trail-panel = Open the trails
ask-software-icon-path = Path of the image
ask-software-file-path = Path of the file to install
software-not-in-launcher = It is not in the launcher yet; the launcher is reading its folders again.
notif-action-transfers-panel = Open the queue
notif-action-software-maintenance = See the updates
notif-action-software-trials = See the trials
notif-action-software-android = Open Android
echo-brain-no-action = No module of the brain offers that
echo-brain-snoozed = Put off
calc-shape-arithmetic = Arithmetic
calc-shape-conversion = Conversion
calc-shape-percentage = Percentage
calc-shape-base = Number base
calc-shape-roman = Roman numerals
calc-shape-date = Date
calc-shape-zone = Time zone
calc-shape-currency = Currency
# A PREFERENCE THAT COULD NOT BE SAVED is said, never only logged.
echo-preference-not-written = { $file } could not be saved ({ $why }): what you changed lasts only until the footer restarts.

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-any = Every file
file-picker-filter-images = Pictures
file-picker-row-play-folder = Play this folder
file-picker-row-enqueue-folder = Add this folder to the queue
file-picker-ask-overwrite = { $name } is already in { $folder }. Write over it?
file-picker-answer-no = No
file-picker-answer-yes = Yes, write over it
file-picker-type-note = Type to narrow the folder; start with / or ~ to type a path.
file-picker-places = Places
file-picker-refused = This folder could not be read: { $why }
file-picker-empty = Nothing here to choose, and no folders.
file-picker-no-match = Nothing here holds “{ $typed }”.
file-picker-name = Name
file-picker-typed = Typed
file-picker-shows = Shows
file-picker-here = Choose this folder
file-picker-take = { $count ->
    [one] Take the marked file
   *[other] Take the { $count } marked files
  }
file-picker-home = Home
file-picker-no-address = This page does not take a web address.
file-picker-nothing-there = There is nothing at { $typed }.
file-picker-no-name = Type a name first.
file-picker-bad-name = “{ $name }” cannot be a file's name.
file-picker-is-a-folder = { $name } is a folder.
file-picker-down = down
file-picker-up = up
file-picker-left = left
file-picker-right = right
file-picker-enter = open or choose
file-picker-play-now = play now
file-picker-mark = mark
file-picker-erase = erase
file-picker-parent = up a folder
file-picker-filter = what shows
file-picker-play-folder = play the folder
file-picker-enqueue-folder = queue the folder
file-picker-paste = paste
file-picker-hidden = hidden files
about-file-picker-down = The next row — in a grid, the cell under this one
about-file-picker-up = The row before — in a grid, the cell above this one
about-file-picker-left = The cell before, in a grid
about-file-picker-right = The next cell, in a grid
about-file-picker-enter = Open the folder, or choose the file, under the cursor
about-file-picker-play-now = Choose it to play now, right after what plays, the queue kept
about-file-picker-mark = Mark the file under the cursor, or unmark it, to take several at once
about-file-picker-erase = Take the last letter off what was typed
about-file-picker-parent = Go up to the folder above
about-file-picker-filter = Turn what the folder shows: the page's own files, or every file
about-file-picker-play-folder = Play the folder that is open, everything under it
about-file-picker-enqueue-folder = Put the folder that is open in the queue, everything under it
about-file-picker-paste = Type what was copied — a path, or a file:// address
about-file-picker-hidden = Show the hidden files, or hide them again

## file info editing — everything the info panel can change

transfers-work-scan = send a file to VirusTotal
# The action beside the watch's "a new network is treated as public".
notif-action-treat-as-home = Treat it as home
transfers-work-belongings = belongings
about-consent-allow = Lets the sites the row names be asked — what pressing the row does. It is kept in consent.json, for every app.
clipboard-act-stale = The history changed before that key arrived, so nothing was done: look again and press it again.
about-line-offer = Takes the first answer offered for what is typed.
belongings-repair-part-not-read = Part of { $file } cannot be read here: it stays in the file as written, and this machine goes on without it.
transfers-work-incus-launch = create an instance
transfers-work-incus-image-copy = copy an image
transfers-work-incus-export = export an instance
transfers-work-incus-import = import an instance
transfers-work-incus-publish = publish an instance as an image
transfers-work-incus-push = send a file into an instance
transfers-work-incus-pull = take a file out of an instance
transfers-work-stick-fetch = download the installer

## asked, above the modeline — who asks, what for, and what sudo waits for.

sudo-bar-asks = { $who } asks to { $deed }
sudo-bar-who-security = The security centre
sudo-bar-who-security-window = The security centre's window
sudo-bar-who-files = The file manager
sudo-bar-who-files-window = The file manager's window
sudo-bar-who-stick = The installer stick
sudo-bar-who-stick-window = The installer stick's window
sudo-bar-who-belongings = The belongings
sudo-bar-who-belongings-window = The belongings' window
sudo-bar-who-update = The update
sudo-bar-who-update-window = The update's window
sudo-bar-deed-relax-minutes = open the passwordless window for { $minutes ->
        [one] one minute
       *[other] { $minutes } minutes
    }
sudo-bar-deed-relax-hours = open the passwordless window for { $hours ->
        [one] an hour
       *[other] { $hours } hours
    }
sudo-bar-deed-relax-no-limit = open the passwordless window until you end it
sudo-bar-deed-relax-end = end the passwordless window
sudo-bar-deed-firewall = change the firewall
sudo-bar-deed-finger-enrol = enrol a fingerprint ({ $finger })
sudo-bar-deed-finger-delete = remove a fingerprint ({ $finger })
sudo-bar-deed-finger-delete-all = remove every fingerprint
sudo-bar-deed-finger-sudo-on = let your fingers answer the administrator
sudo-bar-deed-finger-sudo-off = stop your fingers answering the administrator
sudo-bar-deed-files-root = change what only the administrator may change
sudo-bar-deed-private = use the /private vault
sudo-bar-deed-private-create = create the /private vault ({ $size })
sudo-bar-deed-private-open = open the /private vault
sudo-bar-deed-private-lock = lock the /private vault
sudo-bar-deed-private-enrol = let a security key open the /private vault
sudo-bar-deed-private-forget = forget the key that opens the /private vault
sudo-bar-deed-private-passphrase = change the /private vault's passphrase
sudo-bar-deed-private-grow = make the /private vault larger ({ $size })
sudo-bar-deed-stick = write the installer to a stick
sudo-bar-deed-approve = approve the root part of your brain module { $id }
sudo-bar-deed-keys = let { $count ->
        [one] one security key
       *[other] { $count } security keys
    } answer the administrator
sudo-bar-deed-keys-off = stop your security keys answering the administrator
sudo-bar-deed-signer-pin = pin your signing key on this machine
sudo-bar-deed-cache-netrc = let this machine read your private binary cache { $name }
sudo-bar-deed-cache-netrc-remove = take your private binary cache's login off this machine
