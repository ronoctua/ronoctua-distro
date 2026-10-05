# Interface texts, en-GB. One of the TWO files kept by hand.
#
# INVARIANT 8: zero string literals in UI. Every sentence that appears on screen comes
# from here, and `nix/scripts/gen-i18n.py` generates one Rust constant per key —
# so referencing a key that does not exist is a COMPILE error, not
# missing text on screen.
#
# `pt-BR.ftl` has to have EXACTLY the same keys. The `i18n-complete` check
# fails the build if they diverge, in either direction.
#
# Format: Fluent (https://projectfluent.org). Key names in kebab-case
# with dots separating domain from item, the same as the command ids.

## Commands — the `M-x` list and the which-key labels

cmd-buffer-next = Next buffer
cmd-buffer-switch = Switch buffer
cmd-file-save = Save file
cmd-buffer-close = Close buffer
cmd-mx-open = Run command…
cmd-buffer-terminal = Open terminal
cmd-buffer-terminal-external = Open external terminal
cmd-monitor-btop = Process monitor (btop)
cmd-monitor-bottom = Process monitor (bottom)
cmd-session-quit = Quit session

## which-key prefix groups

group-quit = quit

## The modeline's two left segments

# THE BUFFER'S NAME (the window title) and its MAJOR MODE (the kind of buffer).
# Both are addressed by an id that is ENGLISH and kebab-case (invariant 18); what
# reaches the screen is the value here. That separation is why `glifos` used to
# leak onto an English-language screen: the id WAS the label.
buffer-example = example
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
buffer-memory = memory
mode-memory = memory
buffer-transfers = transfers
mode-transfers = transfers
buffer-format = format
mode-format = format
buffer-brain = brain
mode-brain = brain
mode-update = update
buffer-no-name = *no name*
mode-unknown = unknown

## The buffer's state — the modeline's third left segment

# WHAT THE BUFFER IS DOING, next to what kind of buffer it is. The id is
# ENGLISH and declared in the component's descriptor; these are what reach the
# screen. `Esc` always returns to `normal`, and every state that is not it is
# drawn in the one colour the modeline reserves for an alternative.
state-normal = normal
state-filter = filter
state-sheet = sheet
state-detail = detail
state-full = full
state-guard = guard
state-choose = choice
state-confirm = confirmation
state-task = task
state-log = log

## Notifications

notif-title = Notifications

# The band, above the modeline. The counter only appears with more than one.
notif-queue-position = { $shown }/{ $total }

# THE CATALOGUE — the distro's own notifications, one summary and one body
# each. For an entry that is REWRITTEN (the compositor's screenshot), these are
# what reaches the screen. For one we raise ourselves, they are the default
# text and the entry's name in the manager.
notif-screenshot-summary = Screen captured
notif-screenshot-body = The picture was saved to disk.
notif-screenshot-app = screen
notif-studio-photo-summary = Photo taken
notif-studio-photo-body = The photo was saved to disk.
notif-studio-recording-summary = Recording saved
notif-studio-recording-body = The recording was saved to disk.
notif-studio-app = camera
notif-action-open-video = Open the recording
notif-memory-killed-summary = Closed to free memory
notif-memory-killed-body = An application was closed to keep the machine responsive.

# What each notification lets you do.
notif-action-edit = Edit the image
notif-action-open = Open the image
notif-action-folder = Open the folder
notif-action-copy-image = Copy the image
notif-action-copy-path = Copy the path
notif-action-memory-panel = Open the memory panel

# THE FILTERS THE DISTRO SHIPS WITH. The manager lists them by these names.
notif-rule-important-gets-room = Something important gets room
notif-rule-bluetooth-connected-is-quiet = A Bluetooth device connecting is said by its mark
notif-rule-bluetooth-disconnected-is-quiet = A Bluetooth device leaving is said by its mark
notif-rule-small-things-are-quick = Small things go by quickly

# The manager: three lists, and a heading each.
notif-column-happened = Received
notif-column-suppressed = Filtered out
notif-column-catalogue = Notifications and filters
# THE HINT LINE reads each id of the manager's key tables as a short text —
# the file manager's convention; `cmd-<id>` is the long label the manual reads.
notif-read = read or unread
notif-forget = forget
notif-read-all = all read
notif-filters = filters
notif-any-device = A device

# THE FILTER SHEET — writing one of your own. The rule language has been
# complete since the day it was generated; until now the only way to reach it
# was editing the JSON by hand, which is not a way.
notif-draft-name = what to call it
notif-draft-field = look at
notif-draft-test = compare
notif-draft-value = against
notif-draft-unnamed = not named yet
notif-draft-nothing = nothing yet
notif-draft-unready = it needs a name and something to compare against
notif-draft-type = type this one
notif-draft-add = one more condition
notif-draft-drop = this condition goes
notif-draft-save = save the filter
notif-new = write a filter
notif-edit = edit this filter of yours
notif-drop = delete this filter of yours

# What a condition looks at, and how it compares. The words are the person's,
# not the table's: `summary` is what the protocol calls it and `the title` is
# what it is.
notif-field-app = the sender
notif-field-summary = the title
notif-field-body = the body
notif-field-tag = a tag
notif-field-catalogue = the kind
notif-field-urgency = how urgent
notif-field-level = the level
notif-test-is = is exactly
notif-test-contains = contains
notif-test-starts-with = starts with
notif-test-at-least = is at least
notif-test-at-most = is at most
notif-act = run its action
notif-adjust = adjust this kind
notif-where = { $at } of { $total }
notif-up = up
notif-down = down
notif-pane-next = next column
notif-pane-previous = previous column
notif-knob-next = next value
notif-knob-previous = previous value
notif-restore = the distro's default
notif-default = distro's default
notif-mine = your filter
notif-shared = on its way to everybody

# BEFORE A FILTER GOES TO EVERYBODY. The repository is public, and what lands in
# it is published: git keeps it and a mirror copies it.
notif-share = share it with everybody
notif-share-answer = answer
notif-share-does = and it does
notif-share-public = This goes into the repository, which is PUBLIC. Everything above is published with it, and there is no practical way to take it back. Nothing else of yours goes — not your history, not your adjustments, not this machine's name. Your own commit is what publishes it.
notif-share-not-a-name = A name in the repository is English, lower case, with hyphens for spaces (invariant 18) — it is what an adjustment is stored under. Rename the filter and come back.
notif-share-no = no, keep it to myself
notif-share-yes = yes, share it
notif-shared-written = It is in the repository. Your commit is what publishes it.
notif-share-failed = It could not be written into the repository.
notif-share-nowhere = This machine has no repository to share into.
notif-yours = your adjustment
notif-nothing-here = nothing here
notif-yes = yes
notif-no = no
notif-knob-show = shows
notif-knob-life = stays

# How the presentation's fields are named on screen.
notif-show-hidden = do not show
notif-show-brief = one line
notif-show-full = up to three lines
notif-life-quick = quick
notif-life-normal = normal
notif-life-long = long
notif-loud = attention-grabbing
notif-level = level
notif-blink = blink
notif-vibrate = vibrate
notif-audio = play a sound
notif-auto-read = arrives already read

# WHAT THE MANAGER KNOWS ABOUT ONE RECORD, on the column beside it. They are
# what the row has no space to say: where it came from, when, how long it stayed
# and — the one the owner asked for by name — WHICH FILTER DECIDED, which used to
# be glued to the end of the sentence as a raw id.
notif-fact-origin = who sent it
notif-fact-arrived = when it arrived
notif-about-record = { $when } — { $line }
notif-fact-life = how long it stayed
notif-fact-show = how it was shown
notif-fact-read = has it been read
notif-fact-filter = filter that decided
notif-fact-no-filter = none of them matched
notif-see = open it

# THE TWO SWITCHES THAT DO NOTHING, and why. A switch that switches nothing off
# and says nothing is the trap this project has already paid for once.
notif-inert-vibrate = no haptic device on this machine
notif-inert-audio = not implemented yet — see docs/PENDING.md

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
notif-glyph-read = Mark it read
notif-glyph-unread = Mark it unread again
notif-glyph-delete = Forget it for good
hint-notif-countdown = how long it has left
hint-notif-paused = the clock is stopped while the pointer is on it
hint-notif-action = { $action } — click it, or open the notification

## Installer — the six questions (section 7.1)

inst-title = Install ronoctua
inst-disk = Target disk
inst-disk-help = Will be erased entirely, with no recovery.
inst-hostname = Machine name
inst-hostname-help = Starts with `noctua-`. Lowercase, digits and hyphen.
inst-password = User password
inst-password-help = To log in once the system is installed.
inst-passphrase = Disk passphrase
inst-passphrase-help = Unlocks the disk when the TPM refuses. Keep it off the computer.
inst-language = Language
inst-language-help = For the interface and the keyboard.
inst-keyboard = Keyboard layout
inst-keyboard-help = Pick the one your keyboard is. It is what you will type the passphrase on.
inst-keyboard-variant = Layout variant
inst-keyboard-variant-help = Tells apart keyboards of the same country. No variant is a valid answer.
inst-compose-key = Accent key
inst-compose-key-help = The Compose key: press and release it, then two keys, and out comes the accent. Without it, a keyboard with no accent keys cannot reach them. The default is Caps Lock, which the two Shifts together take over.
inst-system-locale = System formats
inst-system-locale-help = Date, number and units. Not the interface language, which is already chosen.
inst-timezone = Time zone
inst-timezone-help = Where the machine is. It decides the clock and the log's timestamps.
inst-profile = Installation
inst-profile-help = `full` brings everything and is the recommended one. `minimal` leaves out Android, the toolchains, the browser, the editor, media and the font variants — it installs in minutes, and each group comes back with one line in host.nix.
inst-resume-offset = Swap offset
inst-resume-offset-help = Computed once the disk exists; not typed.

## Installer — the steps

inst-step-key = Hand the passphrase to disko
inst-step-partition = Partition and encrypt the disk
inst-step-flake = Copy the flake to the target system
inst-step-host-nix = Write this machine's description
inst-step-facter = Detect the hardware
inst-step-password = Write the password hash to /persist
inst-step-install = Build and install the system
inst-step-zone = Set this machine's time zone
inst-step-target = Make the target system's configuration folder

## Installer — the interface

inst-review = Review before running
inst-destructive = The marked steps erase the disk
inst-confirm = To confirm, type the disk path
inst-confirm-error = Does not match the chosen disk
inst-to-apply = For real, with the disk erased: --execute
inst-dryrun = Dry run: nothing was touched
inst-running = Installing
inst-done = Install finished. Remove the drive and reboot.
inst-failed = The install stopped at this step
inst-no-disk = No disk found on this machine
inst-keys-edit = Enter continues · Esc goes back · Ctrl+C quits
inst-keys-choose = ↑↓ choose · Enter continues · Esc goes back
inst-keys-review = Enter runs · Esc goes back · Ctrl+C quits
inst-keys-quit = Ctrl+C quits
echo-no-on = { $sequence } is not bound
# Same sequence, other case. Shown when the case IS the reason nothing
# matched — Caps Lock or Shift. Without it the screen names the consequence
# and hides the cause.
echo-no-on-box = { $sequence } is not bound — { $other } is
echo-cancelled = Quit
echo-no-window = No focused window
echo-no-action = { $command } does nothing yet
echo-argument-refused = { $command } does not take “{ $argument }”
echo-no-buffer = That screen was not built into this system
echo-no-opened = Could not open { $program }
cmd-window-left = Focus left
cmd-window-right = Focus right
cmd-window-up = Focus up
cmd-window-down = Focus down
cmd-window-move-left = Move left
cmd-window-move-right = Move right
cmd-window-move-up = Move up
cmd-window-move-down = Move down
cmd-window-maximize = Maximise column
cmd-window-screen-full = Fullscreen
cmd-window-center = Centre column
cmd-window-consume = Consume right
cmd-window-expel = Expel left
cmd-workspace-next = Next workspace
cmd-workspace-previous = Previous workspace
cmd-workspace-move-next = Move window to next workspace
cmd-workspace-move-previous = Move window to previous workspace
cmd-screen-capture = Screenshot
capture-drag = drag for a region
capture-click = click a window
capture-whole = the whole screen
about-capture-whole = Keeps the whole of the screen with the focus, as it was when the capture began.
capture-loupe = the magnifier
about-capture-loupe = Shows or hides the magnifier beside the pointer: the pixels under it, enlarged, with the one it is on outlined.
capture-hide-hint = hide this line
about-capture-hide-hint = Puts this line of shortcuts away, wherever the pointer is; the same key brings it back.
capture-cancel = cancel
about-capture-cancel = Gives the capture up and keeps nothing.
capture-size = { $width } × { $height }
capture-at = { $x }, { $y }
echo-capture-failed = The capture could not be taken — the reason is in the log
group-window = window/buffer
group-terminal = terminal
group-workspace = workspace
cmd-workspace-overview = Workspace overview
cmd-window-float = Toggle floating
cmd-workspace-all = See every workspace at once
cmd-workspace-go = Go to workspace
cmd-workspace-move = Move window to workspace

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-notif-open = Notifications
cmd-notif-window = Notifications in a window
cmd-notif-read-all = Mark every notification read
cmd-notif-read = Mark this notification read, or unread again
cmd-notif-forget = Forget this notification
cmd-notif-act = Run the notification's action
cmd-notif-see = Open the notification and read all of it
cmd-notif-filters = Notification filters
cmd-notif-new = Write a notification filter of your own
cmd-notif-drop = Delete this filter of yours
cmd-notif-share = Share this filter of yours with everybody
cmd-notif-share-answer = Answer the sharing question
cmd-notif-draft-type = Type what this line of the filter holds
cmd-notif-draft-add = Add one more condition to the filter
cmd-notif-draft-drop = Take this condition out of the filter
cmd-notif-draft-save = Save the filter
ask-notif-filter-name = The name of the filter
ask-notif-filter-value = The text the condition looks for
cmd-notif-adjust = Adjust this kind of notification
cmd-notif-up = Up one notification
cmd-notif-down = Down one notification
cmd-notif-pane-next = Next column of the manager
cmd-notif-pane-previous = Previous column of the manager
cmd-notif-knob-next = Next value of the adjustment
cmd-notif-knob-previous = Previous value of the adjustment
cmd-notif-restore = Back to the distro's default
cmd-notif-dismiss = Dismiss notification
cmd-keyboard-restore = Restore the keyboard
group-notification = notification
echo-keyboard-restored = Keyboard restored
echo-keyboard-failed = Could not restore the keyboard: { $reason }

## Help — the shortcut manual (LEADER h s)

cmd-help = Manuals
manual-list = MANUALS
manual-window-lead = MANUAL:
manual-next = next
manual-previous = previous
manual-open = open
manual-window = in a window
manual-down = down
manual-up = up
about-manual-next = The next manual of the list.
about-manual-previous = The previous manual of the list.
about-manual-open = Opens the manual under the cursor, here, in place of the list.
about-manual-window = Opens the manual in a window of its own, and takes it out of the footer.
about-manual-down = Moves the manual one line down.
about-manual-up = Moves the manual one line up.
manual-none = No manual matches what was typed.
manual-filter-empty = Type to filter the manuals.
manual-shortcuts = Shortcuts
manual-gestures = Hand gestures
about-manual-shortcuts = Every key the system answers: holding the leader, the compositor's modes, the emergency and media keys.
about-manual-gestures = Every hand gesture the camera reads, drawn, with what it does.
gesture-point-title = Move the pointer
gesture-click-title = Left click, and drag
gesture-right-click-title = Right click
gesture-scroll-title = Scroll
gesture-arrows-title = Arrow keys
gesture-escape-title = Esc
gesture-focus-title = Move the focus
gesture-workspace-title = Change workspace
gesture-overview-title = Every workspace
gesture-keyboard-title = On-screen keyboard
gesture-player-title = The player's column
gesture-close-title = Close the window
gesture-off-title = Turn gestures off
help-gestures = HAND GESTURES, SEEN BY THE CAMERA
gesture-point = Index and middle
gesture-point-does = Index and middle raised together, the thumb folded: the pointer follows the index.
gesture-click = Pointer + open hand
gesture-click-does = Pointing, the OTHER hand open with its fingers apart: left button, held down for as long as it stays open.
gesture-right-click = Pointer + hang loose
gesture-right-click-does = Pointing, the other hand with only the thumb and little finger out: one right click.
gesture-scroll = Three up, moving
gesture-scroll-does = One hand, thumb, index and middle raised: moving it up or down scrolls.
gesture-arrows = Both index fingers
gesture-arrows-does = Both index fingers side by side, pointing one way: that arrow key.
gesture-escape = Index fingers crossed
gesture-escape-does = Both index fingers crossed in an X: Esc.
gesture-focus = Open hand, swept
gesture-focus-does = The five fingers raised and apart: sweeping the hand moves the focus that way. Coming back moves nothing.
gesture-workspace = Both hands, three up
gesture-workspace-does = Both hands with thumb, index and middle pointing up: the workspace above; pointing down: the one below.
gesture-overview = Ten fingers
gesture-overview-does = Both hands open, side by side: every workspace at once.
gesture-keyboard = Hands stacked
gesture-keyboard-does = Both hands open, one above the other: the on-screen keyboard.
gesture-player = Rock sign
gesture-player-does = Index and little finger raised: opens the player's column, and closes it again.
gesture-close = Both middle fingers
gesture-close-does = Both middle fingers raised: closes the window in focus.
gesture-off = Arms crossed
gesture-off-does = The arms crossed in an X: turns gestures off and lets the camera go. The only gesture that does, on purpose.
help-chords = HOLDING THE LEADER
help-emergency = WHEN THE LEADER DOES NOT ANSWER
help-leader-alternate = Open the command list
## Compositor commands — §13, "every OS action has a command"

# These are the compositor's commands, each reachable by name in the command
# locator. The list is DECLARED by us in `assets/compositor-deeds.nix` — not
# mirrored from somebody else's help text — and the generator refuses a verb
# the pinned compositor does not have. A row added there without a sentence
# here stops the build, which is the opposite of appearing untranslated in the
# middle of the list.

cmd-wm-focus-left = Focus the window to the left
cmd-wm-focus-right = Focus the window to the right
cmd-wm-focus-up = Focus the window above
cmd-wm-focus-down = Focus the window below
cmd-wm-focus-beginning = Focus the first window on the scroll
cmd-wm-focus-end = Focus the last window on the scroll
cmd-wm-focus-parent = Focus the container this window is in
cmd-wm-focus-child = Focus inside the container
cmd-wm-focus-tiling = Focus the tiled windows
cmd-wm-focus-floating = Focus the floating windows
cmd-wm-focus-either = Switch focus between tiled and floating
cmd-wm-focus-screen-left = Focus the screen to the left
cmd-wm-focus-screen-right = Focus the screen to the right
cmd-wm-focus-screen-up = Focus the screen above
cmd-wm-focus-screen-down = Focus the screen below

cmd-wm-move-left = Move the window left
cmd-wm-move-right = Move the window right
cmd-wm-move-up = Move the window up
cmd-wm-move-down = Move the window down
cmd-wm-move-beginning = Move the window to the start of the scroll
cmd-wm-move-end = Move the window to the end of the scroll
cmd-wm-move-out-left = Take the window out of its column, to the left
cmd-wm-move-out-right = Take the window out of its column, to the right
cmd-wm-move-out-up = Move the window up within its column
cmd-wm-move-out-down = Move the window down within its column
cmd-wm-move-screen-left = Move the window to the screen on the left
cmd-wm-move-screen-right = Move the window to the screen on the right

cmd-wm-close = Close the focused window
cmd-wm-close-others = Close every window but this one
cmd-wm-full-screen = Fill the screen with this window
cmd-wm-full-layout = Fill the layout with this window, keeping it tiled
cmd-wm-full-application = Let the application decide what fullscreen means
cmd-wm-full-global = Fill every screen with this window
cmd-wm-float = Float this window, or put it back in the layout
cmd-wm-stick = Keep this window on every workspace
cmd-wm-calm = Stop this window asking for attention
cmd-wm-workspace-send = Send this window to a workspace, and stay where you are
cmd-wm-opacity-more = Make this window more opaque
cmd-wm-opacity-less = Make this window more transparent
cmd-wm-opacity-full = Make this window fully opaque again
cmd-wm-awake-visible = Keep the screen awake while this window is visible
cmd-wm-awake-focus = Keep the screen awake while this window has the focus
cmd-wm-awake-fullscreen = Keep the screen awake while this window is fullscreen
cmd-wm-awake-open = Keep the screen awake while this window is open
cmd-wm-awake-none = Let the screen sleep whatever this window does
cmd-wm-tearing-on = Let this window tear, for the lowest latency
cmd-wm-tearing-off = Stop this window tearing
cmd-wm-border-none = Draw no border around this window
cmd-wm-border-pixel = Draw a thin border around this window
cmd-wm-border-normal = Draw a border and a title bar around this window
cmd-wm-border-csd = Let this window draw its own decorations
cmd-wm-shadow-on = Draw a shadow under this window
cmd-wm-shadow-off = Draw no shadow under this window
cmd-wm-dim-on = Dim this window while it does not have the focus
cmd-wm-dim-off = Never dim this window

cmd-wm-columns = New windows open in a column of their own
cmd-wm-rows = New windows open under this one
cmd-wm-transpose = Turn the layout on its side
cmd-wm-width-next = Widen the column to the next size
cmd-wm-width-previous = Narrow the column to the previous size
cmd-wm-height-next = Make the window taller
cmd-wm-height-previous = Make the window shorter
cmd-wm-width-set = Set the column's width, as a fraction of the screen
cmd-wm-height-set = Set the window's height, as a fraction of the screen
cmd-wm-fit-columns = Share the screen out between the columns
cmd-wm-fit-rows = Share the column out between its windows
cmd-wm-fit-visible = Fit the columns that are on screen
cmd-wm-maximise = Give this column the whole screen

cmd-wm-align-left = Hold this column against the left edge
cmd-wm-align-right = Hold this column against the right edge
cmd-wm-align-centre = Hold this column in the middle
cmd-wm-align-top = Hold this window against the top
cmd-wm-align-bottom = Hold this window against the bottom
cmd-wm-align-middle = Hold this window in the middle of its column
cmd-wm-align-auto = Let the layout place this column again
cmd-wm-pin-beginning = Pin this column to the left of the screen
cmd-wm-pin-end = Pin this column to the right of the screen

cmd-wm-select = Mark this window, or unmark it
cmd-wm-select-none = Clear every mark
cmd-wm-select-workspace = Mark every window on this workspace
cmd-wm-select-move = Bring the marked windows here
cmd-wm-select-to-trail = Turn the marked windows into a trail

cmd-wm-trailmark = Put this window on the trail, or take it off
cmd-wm-trailmark-next = Go to the next window on the trail
cmd-wm-trailmark-previous = Go to the previous window on the trail
cmd-wm-trailmark-jump = Jump to a window on the trail by the number drawn on it
cmd-wm-trail-new = Start a new trail
cmd-wm-trail-next = Use the next trail
cmd-wm-trail-previous = Use the previous trail
cmd-wm-trail-clear = Empty this trail
cmd-wm-trail-delete = Delete this trail

cmd-wm-jump = Jump to a window by typing the number drawn on it
cmd-wm-jump-tiling = Jump to a tiled window by the number drawn on it
cmd-wm-jump-everywhere = Jump to any window at all by the number drawn on it
cmd-wm-jump-container = Jump to a container by the number drawn on it
cmd-wm-jump-workspaces = Jump to a workspace by the number drawn on it
cmd-wm-jump-floating = Jump to a floating window by the number drawn on it

cmd-wm-overview = See the whole workspace at once
cmd-wm-overview-all = See every workspace at once
cmd-wm-overview-all-on = Show every workspace at once
cmd-wm-overview-all-off = Back from every workspace to one
cmd-wm-zoom-out = Zoom the workspace out a little
cmd-wm-zoom-in = Zoom the workspace in a little
cmd-wm-zoom-reset = Put the workspace back to its own size
cmd-wm-content-smaller = Shrink what is inside this window
cmd-wm-content-bigger = Enlarge what is inside this window
cmd-wm-content-reset = Put this window's contents back to their own size

cmd-wm-ring-previous = Go back through the windows you were just in
cmd-wm-ring-next = Go forward through the windows you were just in
cmd-wm-ring-first = Go to the oldest window in the ring
cmd-wm-ring-last = Go to the newest window in the ring
cmd-wm-ring-set = Put this window at the end of the ring
cmd-wm-ring-remove = Take this window out of the ring

cmd-wm-workspace-next = Go to the next workspace on this screen
cmd-wm-workspace-previous = Go to the previous workspace on this screen
cmd-wm-workspace-next-screen = Go to the next workspace, on any screen
cmd-wm-workspace-previous-screen = Go to the previous workspace, on any screen
cmd-wm-workspace-back = Go back to the workspace you came from
cmd-wm-workspace-go = Go to a workspace by number
cmd-wm-workspace-move = Take this window to a workspace and follow it
cmd-wm-workspace-move-next = Take this window to the next workspace
cmd-wm-workspace-move-previous = Take this window to the previous workspace
cmd-wm-workspace-swap = Swap this workspace with another by number
cmd-wm-workspace-split = Show two workspaces side by side
cmd-wm-workspace-split-reset = Show one workspace again
cmd-wm-workspace-to-screen-left = Move this workspace to the screen on the left
cmd-wm-workspace-to-screen-right = Move this workspace to the screen on the right

cmd-wm-scratchpad-put = Put this window away in the drawer
cmd-wm-scratchpad-show = Take a window out of the drawer
cmd-wm-scratchpad-jump = Jump to a window in the drawer by the number drawn on it

cmd-wm-screens-off = Turn every screen off
cmd-wm-screens-on = Turn every screen back on
cmd-wm-keep-shortcuts = Let this window take the shortcuts, or take them back
cmd-wm-close-all = Close every window on this workspace
cmd-wm-reload = Read the compositor's configuration again
cmd-wm-move-out-beginning = Take the window out of its column, to the start of the scroll
cmd-wm-move-out-end = Take the window out of its column, to the end of the scroll
cmd-wm-workspace-swap-name = Swap this workspace's number with another's, keeping the windows
cmd-wm-workspace-split-v = Split the workspace top and bottom, at a fraction
cmd-wm-workspace-split-h = Split the workspace side by side, at a fraction
cmd-wm-jump-tiling-all = Jump to a tiled window on any screen
cmd-wm-jump-floating-all = Jump to a floating window on any screen
cmd-wm-jump-everywhere-all = Jump to any window on any screen
cmd-wm-jump-trailmark = Jump to a marked window
cmd-wm-jump-trailmark-all = Jump to a marked window on any screen
cmd-wm-filter-tiling = Show only the tiled windows, on this screen
cmd-wm-filter-tiling-all = Show only the tiled windows, on every screen
cmd-wm-filter-tiling-only = Show only the tiled windows, on this workspace alone
cmd-wm-filter-container = Show only this column, on this screen
cmd-wm-filter-container-all = Show only this column, on every screen
cmd-wm-filter-container-only = Show only this column, on this workspace alone
cmd-wm-filter-floating = Show only the floating windows, on this screen
cmd-wm-filter-floating-all = Show only the floating windows, on every screen
cmd-wm-filter-floating-only = Show only the floating windows, on this workspace alone
cmd-wm-filter-trailmark = Show only the marked windows, on this screen
cmd-wm-filter-trailmark-all = Show only the marked windows, on every screen
cmd-wm-filter-trailmark-only = Show only the marked windows, on this workspace alone
cmd-wm-filter-visible = Show only the windows now in view, on this screen
cmd-wm-filter-visible-all = Show only the windows now in view, on every screen
cmd-wm-filter-visible-only = Show only the windows now in view, on this workspace alone
cmd-wm-filter-reset = Show every window again
cmd-wm-placement-after = New windows open after the one in focus
cmd-wm-placement-before = New windows open before the one in focus
cmd-wm-placement-beginning = New windows open at the start of the scroll
cmd-wm-placement-end = New windows open at the end of the scroll
cmd-wm-placement-focus = New windows take the focus
cmd-wm-placement-no-focus = New windows open without taking the focus
cmd-wm-centre-horizontal = Keep the window in focus centred across
cmd-wm-no-centre-horizontal = Stop centring the window in focus across
cmd-wm-centre-vertical = Keep the window in focus centred down
cmd-wm-no-centre-vertical = Stop centring the window in focus down
cmd-wm-reorder-auto = Reorder the windows as the focus moves
cmd-wm-no-reorder-auto = Stop reordering the windows as the focus moves
cmd-wm-fit-fraction = New windows take a fraction of the screen
cmd-wm-fit-split = New windows share the column they open in
cmd-wm-no-fit = New windows keep the size they ask for
cmd-wm-resize-narrower = Make the window narrower
cmd-wm-resize-wider = Make the window wider
cmd-wm-resize-shorter = Make the window shorter
cmd-wm-resize-taller = Make the window taller
cmd-wm-resize-shrink-right = Pull the window's right edge in
cmd-wm-resize-grow-right = Push the window's right edge out
cmd-wm-resize-shrink-down = Pull the window's bottom edge in
cmd-wm-resize-grow-down = Push the window's bottom edge out
cmd-wm-resize-shrink-left = Pull the window's left edge in
cmd-wm-resize-grow-left = Push the window's left edge out
cmd-wm-resize-shrink-up = Pull the window's top edge in
cmd-wm-resize-grow-up = Push the window's top edge out
cmd-wm-nudge-left = Move the floating window left
cmd-wm-nudge-right = Move the floating window right
cmd-wm-nudge-up = Move the floating window up
cmd-wm-nudge-down = Move the floating window down
cmd-wm-toggle-size-this = Fill the screen with this window, or give it its size back
cmd-wm-toggle-size-active = Fill the screen with the window in focus, or give it its size back
cmd-wm-toggle-size-width = Give every window a width, or give them theirs back
cmd-wm-toggle-size-height = Give every window a height, or give them theirs back
cmd-wm-toggle-size-reset = Forget the sizes toggled
cmd-wm-fit-rows-visible = Share the height among the windows in view, keeping their proportions
cmd-wm-fit-columns-visible-equal = Share the width equally among the windows in view
cmd-wm-fit-rows-visible-equal = Share the height equally among the windows in view
cmd-wm-fit-columns-to-end = Fit the width from this window to the end, keeping proportions
cmd-wm-fit-rows-to-end = Fit the height from this window to the end, keeping proportions
cmd-wm-fit-columns-to-end-equal = Fit the width from this window to the end, equally
cmd-wm-fit-rows-to-end-equal = Fit the height from this window to the end, equally
cmd-wm-fit-columns-to-start = Fit the width from the start to this window, keeping proportions
cmd-wm-fit-rows-to-start = Fit the height from the start to this window, keeping proportions
cmd-wm-fit-columns-to-start-equal = Fit the width from the start to this window, equally
cmd-wm-fit-rows-to-start-equal = Fit the height from the start to this window, equally
cmd-wm-fit-rows-active = Fit the height of the window in focus
cmd-wm-fit-columns-all = Share the width among every window, keeping proportions
cmd-wm-fit-rows-all = Share the height among every window, keeping proportions
cmd-wm-trail-number = Go to a numbered trail
cmd-wm-trail-to-selection = Select the windows of this trail
cmd-wm-space-load = Load a saved layout
cmd-wm-space-save = Save this layout under a number
cmd-wm-space-restore-hide = Restore a saved layout, hiding the other windows
cmd-wm-space-restore = Restore a saved layout
cmd-wm-mode-split = Split the workspace…
cmd-wm-mode-jump = Jump to a window…
cmd-wm-mode-filter = Show only some windows…
cmd-wm-mode-placement = Where new windows go…
cmd-wm-mode-width = Set the window's width…
cmd-wm-mode-height = Set the window's height…
cmd-wm-mode-resize = Resize the window with the arrows…
cmd-wm-mode-floating = Move and resize a floating window with the arrows…
cmd-wm-mode-toggle-width = Toggle every window to a width…
cmd-wm-mode-toggle-height = Toggle every window to a height…
cmd-wm-mode-align = Align the window…
cmd-wm-mode-fit = Fit the windows…
cmd-wm-mode-trailmark = Mark windows…
cmd-wm-mode-trail = Trails of marked windows…
cmd-wm-mode-spaces = Saved layouts…
cmd-wm-mode-default = Leave the compositor's mode
group-workspace-move = move window
group-icons = icons
expansion-page = RCtrl ←→
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

# --- Clipboard ---
cmd-clipboard-open = Clipboard
cmd-clipboard-copy-of-new = Copy again
cmd-clipboard-remove = Remove from history
cmd-clipboard-pin = Pin / unpin
cmd-clipboard-qrcode = Generate QR code
cmd-clipboard-clear = Clear the history
clipboard-empty = Nothing copied yet
clipboard-history = History
clipboard-item = Item
clipboard-heading-sync = Sync
clipboard-content-text = text
clipboard-sync-state = syncing
clipboard-sync-server = server
clipboard-sync-account = account
clipboard-sync-forget = forget this account
clipboard-sync-images = images too, not only text
clipboard-sync-notify = say when something arrives
clipboard-sync-cap = largest copy that is sent
clipboard-sync-cap-server = { $size }, which is the server's limit
clipboard-sync-cap-unset = whatever the server carries
clipboard-sync-salt = salt (the same on every device)
clipboard-sync-salt-none = none
clipboard-sync-rounds = rounds the key is derived over
clipboard-setting-on = on
clipboard-setting-off = off
clipboard-mark = mark on the modeline
clipboard-mark-never = never
clipboard-mark-when-off = when it is not syncing
clipboard-mark-when-on = while it is syncing
clipboard-sync-on = yes
clipboard-sync-off = not right now
clipboard-sync-down = cannot reach the server
clipboard-sync-unset = not set
clipboard-down = next
clipboard-up = previous
clipboard-copy-of-new = copy again
clipboard-remove = remove
clipboard-pin = pin
clipboard-qrcode = QR code
clipboard-clear = clear
clipboard-sync = sync
clipboard-more = change
clipboard-less = back
cmd-clipboard-more = Turn this setting forwards
cmd-clipboard-less = Turn this setting back
cmd-clipboard-down = Next entry
cmd-clipboard-up = Previous entry
cmd-clipboard-sync = Sync the clipboard between machines
ask-clipboard-server = The address of the ClipCascade server
ask-clipboard-cap = The largest copy that is sent, in bytes (0 for the server's own limit)
ask-clipboard-rounds = How many rounds the key is derived over
ask-clipboard-salt = The salt, the same on every device on this server
ask-clipboard-account = The account on that server
ask-clipboard-password = The password for that account
echo-clipboard-synced = The clipboard is syncing
echo-clipboard-sync-off = The clipboard stays on this machine
echo-clipboard-sync-failed = Could not sync: { $reason }
clipboard-pinned = pinned
clipboard-content = Content
clipboard-types = Types
clipboard-size = Size
echo-copied = Copied
echo-image-copied = The image is on the clipboard
echo-copied-value = Copied: { $value }
echo-copy-failed = Could not copy — see the log
echo-qr-large = The content is too large for a QR code
echo-copy-only-text = For now only text can be copied again
clipboard-image = Image
clipboard-unreadable = Copied, and unreadable here: { $kind }

# --- The application launcher (LEADER a) -----------------------------------
cmd-apps-open = Open application
apps-empty = No applications found
# The launcher's own commands, and its last row, which looks for what was typed
# among the programs one can install.
cmd-apps-details = See what this program is and where it came from
apps-pin = pin to the sidebar
apps-details = details
cmd-apps-remove = Remove this program, after a question
apps-remove = remove
cmd-apps-search-to-install = Search the programs to install for what was typed
apps-search-to-install = Search “{ $query }” to install

## Network — Wi-Fi and Ethernet (LEADER s n)

cmd-network-open = Networks
cmd-network-scan = Scan networks
cmd-network-connect = Connect to network
ask-network-connect = Password for this network
cmd-network-disconnect = Disconnect
cmd-network-forget = Forget network
cmd-network-wifi-toggle = Toggle Wi-Fi radio
cmd-network-link = Show the link
cmd-theme-open = Themes
cmd-theme-apply = Apply this theme and background
group-network = network

# THE VIEW. States the list shows in place of rows when there is no row to
# show — each says something different, and folding them into one generic
# "no networks" would hide the difference between not finding and not looking.
network-scanning = scanning…
network-empty = no networks in range
network-no-adapter = this machine has no Wi-Fi radio
network-radio-off = radio off
network-wired = wired
network-saved = saved
network-open = open
network-password = Password for { $network }
network-password-help = Kept by NetworkManager, outside the repository.

# ECHOES — what the minibuffer says after the action.
echo-network-connected = Connected to { $network }
echo-network-failed = Could not connect to { $network }: { $reason }
echo-network-disconnected = Disconnected
echo-network-forgotten = { $network } forgotten
echo-network-radio-on = Wi-Fi radio on
echo-network-radio-off = Wi-Fi radio off
echo-network-link = { $network } · { $band } · { $rate } · signal { $signal }
echo-network-link-wired = { $network } · { $rate }
echo-network-no-link = Not connected
echo-network-unavailable = NetworkManager is not responding

## Syncthing (LEADER s s)

cmd-syncthing-open = Syncthing
cmd-syncthing-web = Syncthing's own page
cmd-syncthing-state = Syncthing status
cmd-syncthing-pause-all = Pause syncing
cmd-syncthing-resume-all = Resume syncing
group-syncthing = syncthing
echo-syncthing-state = { $connected } connected · { $percent }% · ↑{ $up } ↓{ $down }
echo-syncthing-stopped = Syncthing is not responding
echo-syncthing-paused = Syncing paused
echo-syncthing-resumed = Syncing resumed

# Syncthing (LEADER s s) — one view, two doors, and the mark the
# modeline may draw. The option labels below are owed by
# `assets/syncthing-options.nix` through the generator; `syncthing-options-have-text`
# is what says so.
echo-audio-will-not-parse = audio.json will not parse, and writing would replace it. Fix or remove the file.
syncthing-title = Syncthing
hint-syncthing-on = Everything is being synchronised.
hint-syncthing-off = Nothing is being synchronised.
syncthing-no-service = Syncthing is not answering. Nothing is being synchronised.
syncthing-gone = It is not there any more.
syncthing-state = state
syncthing-state-down = not running
syncthing-state-paused = everything is paused
syncthing-state-syncing = syncing
syncthing-state-idle = up to date
syncthing-connected = machines connected
syncthing-connected-of = { $connected } of { $total }
syncthing-traffic = transferred
syncthing-traffic-both = ↑ { $up }  ↓ { $down }
syncthing-invitations = asking to be let in
syncthing-wants-in = a machine at { $address }
syncthing-offered-by = a folder offered by { $who }
syncthing-folders = folders
syncthing-no-folders = No folder is shared yet.
syncthing-devices = machines
syncthing-no-devices = No other machine is paired yet.
syncthing-service = the service
syncthing-settings = more settings
syncthing-global = the whole service
syncthing-interface = the web interface
syncthing-interface-about = The page `w` opens. It stays on this machine: on the network it would be an administrative port with no password.
syncthing-options = settings
syncthing-advanced = rarely worth touching
syncthing-advanced-about = Nothing in the flake puts these back: they are yours, and they survive a rebuild.
syncthing-open = open
syncthing-do = do it
syncthing-on = on
syncthing-off = off
syncthing-shared = shared
syncthing-not-shared = not shared
syncthing-mark = mark on the modeline
syncthing-mark-never = never
syncthing-mark-when-off = when nothing is syncing
syncthing-mark-when-on = while it is syncing
syncthing-unit = the system service
syncthing-unit-running = running
syncthing-unit-stopped = stopped
syncthing-unit-moving = changing
syncthing-unit-unknown = cannot tell
syncthing-unit-absent = not installed on this machine
syncthing-start-unit = start the service
syncthing-stop-unit = stop the service
syncthing-restart = restart it
syncthing-pause-everything = pause everything
syncthing-resume-everything = start everything again
syncthing-version = version
syncthing-this-machine = this machine
syncthing-uptime = up for
syncthing-listening = listening on
syncthing-web = web interface
syncthing-for-days = { $n } days
syncthing-for-hours = { $n } hours
syncthing-for-minutes = { $n } minutes
syncthing-for-seconds = { $n } seconds
syncthing-folder-id = folder id
syncthing-device-id = machine id
syncthing-size = size
syncthing-errors = things it could not do
syncthing-shared-with = shared with
syncthing-actions = what can be done
syncthing-rescan = look at the disk now
# THE REST OF THE HINT LINE'S WORDS. Eight of this view's ten keys had only
# a `cmd-` label, and the line reads the SHORT form: the screen said
# `[syncthing-down] · [syncthing-up] · …`, the missing-key marker, in both
# doors of `LEADER s s`. Found by `every-hint-key-explains-itself`, which
# walks every id of every table.
syncthing-up = up
syncthing-down = down
syncthing-less = previous
syncthing-more = next
syncthing-enter = open
syncthing-pause = pause
syncthing-add = add
syncthing-forget = remove
syncthing-override = send this copy out
syncthing-revert = undo local changes
syncthing-forget-folder = stop sharing it
syncthing-forget-device = forget this machine
syncthing-address = reached at
syncthing-running = it is running
syncthing-device-connected = connected
syncthing-device-away = not connected
syncthing-device-paused = paused
syncthing-state-scanning = reading the disk
syncthing-state-preparing = working out what to move
syncthing-state-cleaning = tidying old versions
syncthing-state-failing = something is wrong
syncthing-state-unknown = doing something new
ask-syncthing-enter = New value
cmd-syncthing-add-device = Add a machine
ask-syncthing-add-device = The other machine's id
cmd-syncthing-add-folder = Share a folder
ask-syncthing-add-folder = Where the folder is on this disk
echo-syncthing-flake-owns = The flake writes this one. Changing it here would last until the next rebuild.
echo-syncthing-refused = Syncthing refused: { $reason }
echo-syncthing-not-running = Syncthing is not running.
echo-syncthing-not-allowed = The system would not let that happen.
echo-syncthing-unknown = That did not work, and the log says why.
echo-syncthing-will-not-parse = syncthing.json will not parse, and writing would replace it. Fix or remove the file.
cmd-syncthing-window = Syncthing, in a window
cmd-syncthing-up = up one row
cmd-syncthing-down = down one row
cmd-syncthing-less = the previous value
cmd-syncthing-more = the next value
cmd-syncthing-enter = do what this row does
cmd-syncthing-pause = pause or resume
cmd-syncthing-rescan = look at the disk now
cmd-syncthing-add = add one
cmd-syncthing-forget = remove, or say no

# What every option Syncthing exposes is called, in our words.
syncthing-opt-folder-label = name
syncthing-opt-folder-path = where it is
syncthing-opt-folder-type = what it does
syncthing-opt-folder-paused = paused
syncthing-opt-rescan-interval = look again every
syncthing-opt-watch-for-changes = notice changes at once
syncthing-opt-watch-delay = wait before noticing
syncthing-opt-watch-timeout = give up watching after
syncthing-opt-ignore-permissions = ignore permissions
syncthing-opt-keep-conflicts = copies of a conflict
syncthing-opt-pull-order = what to fetch first
syncthing-opt-block-pull-order = what part of a file first
syncthing-opt-copy-range-method = how to copy a block
syncthing-opt-filesystem-type = filesystem
syncthing-opt-marker-name = marker folder
syncthing-opt-auto-normalize = fix odd file names
syncthing-opt-case-sensitive = upper and lower case differ
syncthing-opt-junctions-as-dirs = junctions are folders
syncthing-opt-ignore-delete = never delete anything
syncthing-opt-disable-sparse = write holes as zeroes
syncthing-opt-disable-fsync = do not wait for the disk
syncthing-opt-block-indexing = do not index
syncthing-opt-mod-time-window = clocks may differ by
syncthing-opt-max-concurrent-writes = writes at once
syncthing-opt-copy-ownership = take owner from the folder
syncthing-opt-sync-ownership = sync owner
syncthing-opt-send-ownership = send owner
syncthing-opt-sync-xattrs = sync extended attributes
syncthing-opt-send-xattrs = send extended attributes
syncthing-opt-scan-progress-interval = report scan progress every
syncthing-opt-puller-max-pending = keep in flight
syncthing-opt-puller-delay = wait between pulls
syncthing-opt-puller-pause = pause between pulls
syncthing-opt-copiers = copying threads
syncthing-opt-hashers = hashing threads
syncthing-opt-folder-group = group
syncthing-opt-least-free-space = keep free on this disk
syncthing-opt-versioning = keep old versions
syncthing-opt-versioning-cleanup = tidy old versions every
syncthing-opt-versioning-path = where old versions go
syncthing-opt-xattr-permitted = extended attributes to keep
syncthing-opt-device-name = name
syncthing-opt-device-paused = paused
syncthing-opt-compression = compress
syncthing-opt-auto-accept-folders = take folders it offers
syncthing-opt-introducer = let it introduce others
syncthing-opt-skip-introduction-removals = keep what it stops introducing
syncthing-opt-untrusted = cannot read the files
syncthing-opt-device-send-limit = send no faster than
syncthing-opt-device-receive-limit = receive no faster than
syncthing-opt-connections = connections to it
syncthing-opt-max-request = largest request
syncthing-opt-remote-interface-port = its web interface port
syncthing-opt-device-group = group
syncthing-opt-device-addresses = where to find it
syncthing-opt-allowed-networks = only on these networks
syncthing-opt-ignored-folders = folders to ignore from it
syncthing-opt-nat-traversal = open a port on the router
syncthing-opt-relays = go through a relay
syncthing-opt-announce-locally = announce on this network
syncthing-opt-announce-globally = announce on the internet
syncthing-opt-send-limit = send no faster than
syncthing-opt-receive-limit = receive no faster than
syncthing-opt-limit-inside-the-lan = limit inside this network too
syncthing-opt-low-priority = run at low priority
syncthing-opt-auto-upgrade = look for a new version every
syncthing-opt-usage-report = send usage figures
syncthing-opt-crash-reporting = send crash reports
syncthing-opt-open-a-browser = open a browser on start
syncthing-opt-folder-concurrency = folders working at once
syncthing-opt-keep-temporaries = keep half-copied files for
syncthing-opt-reconnect-interval = try again every
syncthing-opt-progress-interval = report progress every
syncthing-opt-announce-lan-addresses = announce local addresses too
syncthing-opt-overwrite-names = rename machines on connect
syncthing-opt-send-full-index = send the whole index after an upgrade
syncthing-opt-temp-index-min-blocks = share a half-copied file after
syncthing-opt-incoming-request-buffer = buffer for incoming requests
syncthing-opt-connection-limit-enough = connections that are enough
syncthing-opt-connection-limit-max = connections at the most
syncthing-opt-priority-tcp-lan = priority: TCP on this network
syncthing-opt-priority-tcp-wan = priority: TCP elsewhere
syncthing-opt-priority-quic-lan = priority: QUIC on this network
syncthing-opt-priority-quic-wan = priority: QUIC elsewhere
syncthing-opt-priority-relay = priority: through a relay
syncthing-opt-priority-upgrade-threshold = swap for a better route at
syncthing-opt-traffic-class = traffic class
syncthing-opt-announce-port = announcement port
syncthing-opt-announce-group = announcement group
syncthing-opt-nat-lease = router port lease
syncthing-opt-nat-renewal = renew the router port every
syncthing-opt-nat-timeout = give the router
syncthing-opt-stun-keepalive-start = first STUN keepalive after
syncthing-opt-stun-keepalive-least = STUN keepalive no sooner than
syncthing-opt-relay-reconnect = try the relay again every
syncthing-opt-pre-releases = offer test versions
syncthing-opt-audit = write an audit log
syncthing-opt-audit-file = audit log file
syncthing-opt-report-delay = wait before the first report
syncthing-opt-report-insecurely = report without checking the certificate
syncthing-opt-report-address = where reports go
syncthing-opt-crash-address = where crash reports go
syncthing-opt-releases-address = where new versions are announced
syncthing-opt-listen-addresses = listen on
syncthing-opt-discovery-servers = who to ask where a machine is
syncthing-opt-stun-servers = STUN servers
syncthing-opt-always-local-networks = treat these as local
syncthing-opt-least-free-home = keep free on the home disk
syncthing-opt-interface-address = where it listens
syncthing-opt-interface-enabled = the web interface exists
syncthing-opt-interface-tls = over HTTPS
syncthing-opt-interface-theme = its theme
syncthing-opt-interface-user = user name
syncthing-opt-interface-auth-mode = how it checks who you are
syncthing-opt-interface-basic-auth-prompt = ask the browser for a password
syncthing-opt-interface-metrics-without-auth = metrics without a password
syncthing-opt-interface-insecure-admin = administer without a password
syncthing-opt-interface-insecure-host-check = do not check the host header
syncthing-opt-interface-insecure-frames = let other pages embed it
syncthing-opt-interface-session-life = a session lasts
syncthing-opt-interface-socket-permissions = socket permissions

# What a number is counted in.
syncthing-unit-seconds = seconds
syncthing-unit-hours = hours
syncthing-unit-minutes = minutes
syncthing-unit-kbps = kbit/s
syncthing-unit-kib = KiB
syncthing-unit-connections = connections
syncthing-unit-writes = at a time
syncthing-unit-threads = threads
syncthing-unit-folders = at a time
syncthing-unit-copies = copies
syncthing-unit-blocks = blocks

# The words a ring turns through.
syncthing-word-none = none
syncthing-word-sendreceive = send and receive
syncthing-word-sendonly = send only
syncthing-word-receiveonly = receive only
syncthing-word-receiveencrypted = receive, encrypted
syncthing-word-random = in any order
syncthing-word-alphabetic = by name
syncthing-word-smallest-first = smallest first
syncthing-word-largest-first = largest first
syncthing-word-oldest-first = oldest first
syncthing-word-newest-first = newest first
syncthing-word-standard = the usual way
syncthing-word-in-order = in order
syncthing-word-copy-file-range = copy_file_range
syncthing-word-ioctl = by reference (reflink)
syncthing-word-sendfile = sendfile
syncthing-word-duplicate-extents = duplicate extents
syncthing-word-all = whatever works
syncthing-word-basic = the ordinary one
syncthing-word-fake = a pretend one, for testing
syncthing-word-trashcan = a wastebasket
syncthing-word-simple = a few numbered copies
syncthing-word-staggered = more of the recent ones
syncthing-word-external = hand them to a program
syncthing-word-metadata = only the descriptions
syncthing-word-always = always
syncthing-word-never = never
syncthing-word-static = a name and a password
syncthing-word-ldap = an LDAP directory
## Installer — the seventh question (network)

inst-wifi = Wi-Fi network
inst-wifi-help = The password stays on the installed machine, never in the repository.
inst-wifi-none = no Wi-Fi — I will use a cable
inst-wifi-scanning = Looking for networks…
inst-wifi-empty = No networks in range
inst-wifi-password = Network password
inst-wifi-password-short = a WPA network password is at least 8 characters
inst-step-seal-wifi = Seal the Wi-Fi password in the TPM
inst-step-wifi = Write the Wi-Fi network profile

## Network — the password sealed in the TPM (psk-flags)

# The one-key question, after the connection succeeds.
network-remember = Keep the password of { $network } on this machine?
network-remember-no = no, store nothing
network-remember-yes = yes, seal it in the TPM
# The list marker: tells "saved in NetworkManager" apart from "the password exists".
network-no-tpm = in TPM
echo-network-stored = Password for { $network } sealed in the TPM
echo-network-no-stored = Connected. The password was not stored.
echo-network-store-failed = Connected, but I could not store the password: { $reason }
## Out of memory
notif-oom-app = system
notif-oom-pressure = { $app } was stalling the machine for want of memory.
notif-oom-used = { $app } filled memory and swap.
notif-oom-orphan = Something outside your buffers was closed: { $cgroup }

## The focused buffer's keymap
which-buffer = Focused buffer
hint-state = State: { $state } — click cycles, Esc returns, Esc+arrows chooses
which-state = State of the focused buffer

## The leader's three sets: the key tapped right after the leader.
# Alone, a key that has a footer and a window opens the footer; after a tap
# of Alt it opens the window; after a tap of Ctrl the whole tree is there. The
# short word is what the hint line draws beside the key; `cmd-` is what the
# pointer reads, and what `LEADER :` lists.
leader-set-window = windows
cmd-leader-set-window = The same keys, each opening its app in a window instead of the footer
leader-set-full = everything
cmd-leader-set-full = The whole tree of keys, with every submenu as it was

## The memory panel — LEADER s m

cmd-memory-open = System memory
cmd-example-open = Several keyboards in one app, the reference
example-title = Several keyboards
example-note = Each keyboard moves its own selection and types into its own field, in its cursor's colour.
example-list = A list
example-form = A form
example-empty = empty
example-item-one = Morning
example-item-two = Noon
example-item-three = Afternoon
example-item-four = Evening
example-item-five = Night
example-item-six = Dawn
example-field-name = Name
example-field-note = Note
cmd-example-clear = Empty the form
cmd-format-open = Format a disk
cmd-memory-full = Full overview
cmd-memory-guard = Freeze protection
cmd-memory-tab = Switch RAM and disk
cmd-memory-refresh = Refresh now
cmd-memory-scan = Scan
cmd-memory-scan-cancel = Stop the scan
cmd-memory-stop = Close what is focused
cmd-memory-sweep-store = Collect the store's garbage
cmd-memory-sweep-generations = Trim the system's old generations
cmd-memory-sweep-journal = Trim the journal
cmd-memory-tolerant = Loosen for this session
cmd-memory-default = Back to the default
cmd-memory-persist = Save as permanent

memory-title = Memory
memory-ram = RAM
memory-zram = zram
memory-swap = Swap
memory-pressure = Stall
memory-disk = Disk
memory-guard = Protection
memory-kernel = Kernel
memory-cached = File cache
memory-anonymous = Programs
memory-shared = Shared
memory-dirty = To be written
memory-desktop = Desktop
memory-apps = Applications
memory-system = System
memory-consumers = What is using it
memory-scanning = Scanning…
memory-nothing-yet = Nothing scanned yet
memory-asked = Request sent; the next reading shows the result.

memory-help-overview = m f full · m o protection · m s scan · m r refresh
memory-help-full = m v RAM/disk · m s scan · m k close · m g m n m j housekeeping
memory-help-guard = m t loosen · m d default · m p save · Esc goes back

memory-guard-description = When memory runs out, the machine can stop answering for minutes. This protection closes ONE application before that happens — never the desktop, which is outside the candidate set by construction.
memory-guard-detail = It measures how long things spend waiting on memory, not how many bytes are left: it acts only after a sustained stall, never after a spike.
memory-guard-armed = armed
memory-guard-off = not armed
memory-guard-unknown = cannot tell
memory-guard-strict = default
memory-guard-tolerant = tolerant
memory-guard-limit = Acts above
memory-guard-for = After
memory-guard-ceiling = Applications' ceiling
memory-guard-acted = Acted (this boot)
memory-guard-persisted = After a reboot
memory-guard-session-only = This change lasts until reboot. m p writes it into host.nix, and then a nixos-rebuild switch is still missing.
memory-guard-cost = In tolerant mode the machine may be slow for up to a minute before anything is closed.

memory-confirm-stop = Close what is focused? The same key again confirms; Esc cancels.
memory-confirm-sweep = Delete what is not in use? The same key again confirms; Esc cancels.
memory-confirm-persist = Write this profile into host.nix? The same key again confirms; Esc cancels.

group-system = system
memory-vram = Graphics

## Rescue console — Ctrl+Alt+Del

rescue-title = Rescue
rescue-section-state = State
rescue-section-actions = Actions
rescue-section-windows = Windows
rescue-section-terminals = Terminal
rescue-section-config = Configuration
rescue-section-log = Log
rescue-section-system = System

## Rescue console — the actions

rescue-restart-footer = Restart the footer
rescue-restart-compositor = Restart the compositor
rescue-restart-session = Restart the graphical session
rescue-restart-audio = Restart audio
rescue-restore-keyboard = Restore the keyboard
rescue-close-window = Close the focused window
rescue-refresh-fonts = Rebuild the font cache
rescue-open-terminal = Open a terminal
rescue-kill-window = Kill the chosen window
rescue-kill-hog = Kill what is eating the memory
rescue-edit-host = Edit this machine's configuration
rescue-rebuild-switch = Apply the configuration
rescue-rollback-now = Go back to the previous generation
rescue-rollback-boot = Previous generation at next boot
rescue-restart-network = Restart the network
rescue-reboot = Restart the machine
rescue-poweroff = Shut the machine down

## Rescue console — what it says

rescue-keys = ↑↓ choose · Enter runs · Tab changes section · Esc leaves
rescue-running = running…
rescue-ran = done
rescue-failed = failed
rescue-confirm = The same key again confirms; Esc cancels.
rescue-needs-password = asks for your password
rescue-read-only = read-only
rescue-generated-by = Generated by { $source }. Editing here disappears on the next rebuild.
rescue-no-windows = The compositor did not answer — or there is no window at all.
rescue-empty = nothing to show
rescue-back = Leaving the terminal gives this screen back.
rescue-generation = generation { $number }
rescue-door-console = console
rescue-door-window = window

cmd-rescue-open = Rescue console

# ===================================================================
# The file manager — LEADER . (footer) and the window
#
# The permission explanations are why this file grew its first SELECTOR. On a
# directory, `x` is not "execute", it is "may enter", and `r` without `x` lists
# names one cannot reach — saying "execute" there says something false. Which
# of the two sentences applies is a decision of the LANGUAGE, so it lives here
# and not in the Rust.
# ===================================================================

# What the first letter of the column says.
files-kind-directory = A directory: a place, not a thing.
files-kind-file = An ordinary file.
files-kind-symlink = A shortcut: it points somewhere else, and deleting it does not delete the target.
files-kind-fifo = A named pipe: one program writes, another reads.
files-kind-socket = A socket: two programs talking through it.
files-kind-block = A block device — a disk, or part of one.
files-kind-char = A character device.

files-perm-owner-read = { $kind ->
        [dir] { $set ->
                [yes] { $who } can see the names inside this directory
               *[no] { $who } can NOT see the names inside this directory
            }
       *[file] { $set ->
                [yes] { $who } can read this file
               *[no] { $who } can NOT read this file
            }
    }
files-perm-owner-write = { $kind ->
        [dir] { $set ->
                [yes] { $who } can create and delete things inside this directory
               *[no] { $who } can NOT create or delete anything in this directory
            }
       *[file] { $set ->
                [yes] { $who } can change this file
               *[no] { $who } can NOT change this file
            }
    }
files-perm-owner-execute = { $kind ->
        [dir] { $set ->
                [yes] { $who } can ENTER this directory
               *[no] { $who } can NOT enter this directory, nor anything inside it
            }
       *[file] { $set ->
                [yes] { $who } can run this file as a program
               *[no] { $who } cannot run this file
            }
    }
files-perm-group-read = { $kind ->
        [dir] { $set ->
                [yes] the group { $who } can see the names inside this directory
               *[no] the group { $who } can NOT see the names inside this directory
            }
       *[file] { $set ->
                [yes] the group { $who } can read this file
               *[no] the group { $who } can NOT read this file
            }
    }
files-perm-group-write = { $kind ->
        [dir] { $set ->
                [yes] the group { $who } can create and delete things inside this directory
               *[no] the group { $who } can NOT create or delete anything in this directory
            }
       *[file] { $set ->
                [yes] the group { $who } can change this file
               *[no] the group { $who } can NOT change this file
            }
    }
files-perm-group-execute = { $kind ->
        [dir] { $set ->
                [yes] the group { $who } can ENTER this directory
               *[no] the group { $who } can NOT enter this directory
            }
       *[file] { $set ->
                [yes] the group { $who } can run this file as a program
               *[no] the group { $who } cannot run this file
            }
    }
files-perm-other-read = { $kind ->
        [dir] { $set ->
                [yes] anybody on this machine can see the names in this directory
               *[no] nobody else can see the names in this directory
            }
       *[file] { $set ->
                [yes] anybody on this machine can read this file
               *[no] nobody else can read this file
            }
    }
files-perm-other-write = { $kind ->
        [dir] { $set ->
                [yes] anybody on this machine can create and delete things here
               *[no] nobody else can create or delete anything here
            }
       *[file] { $set ->
                [yes] anybody on this machine can change this file
               *[no] nobody else can change this file
            }
    }
files-perm-other-execute = { $kind ->
        [dir] { $set ->
                [yes] anybody on this machine can enter this directory
               *[no] nobody else can enter this directory
            }
       *[file] { $set ->
                [yes] anybody on this machine can run this file
               *[no] nobody else can run this file
            }
    }

# The three bits that are not about a person. The sentence leads with what
# surprises: whoever points at an `s` is asking about the `s`.
files-perm-setuid = { $set ->
        [yes] Runs as { $who }, not as whoever started it.
       *[no] The bit is set on a position that cannot execute — almost always a mistake.
    }
files-perm-setgid = { $kind ->
        [dir] { $set ->
                [yes] Whatever is created in here inherits the group { $group }.
               *[no] The bit is set on a position that cannot execute.
            }
       *[file] { $set ->
                [yes] Runs with the group { $group }, not with the starter's.
               *[no] The bit is set on a position that cannot execute.
            }
    }
files-perm-sticky = { $set ->
        [yes] Only the owner of a file may delete it from here — it is what makes /tmp work.
       *[no] The bit is set on a position that cannot execute.
    }

# The four modes that answer "what should this be".
files-preset-file = ordinary file — you write, everyone else reads
files-preset-private = you, and nobody else
files-preset-program = program — everyone runs it, only you change it
files-preset-private-dir = a directory of your own
files-preset-readonly = nobody changes it, not even you

# The size. The full stop is HERE because the decimal separator belongs to the
# language.
files-size-bytes = { $whole } B
files-size-kib = { $whole }.{ $tenth } KiB
files-size-mib = { $whole }.{ $tenth } MiB
files-size-gib = { $whole }.{ $tenth } GiB
files-size-tib = { $whole }.{ $tenth } TiB

# ISO order in both languages: it sorts on its own and never confuses a day
# with a month, which is the only way a column of dates does not lie.
files-when = { $date } { $time }
files-when-utc = { $date } { $time } UTC
# THE SAME INSTANT, WRITTEN OUT. The column's form is `files-when`; this is
# what the pointer asks for, where there is a line and the question was which
# day and which second.
files-when-long = { $date } at { $time }

files-order-name = name
files-order-size = size
files-order-modified = modified
files-order-kind = kind

# The rows of the action menu. The id and the key cannot diverge — a test
# compares the two, because a row copied with the id changed COMPILES and shows
# the neighbouring row's words.
files-open = open
files-open-map = open the place in a map
files-open-with = open with
files-terminal = terminal here
files-root-terminal = open as root
files-copy-path = copy the path
files-copy = copy
files-cut = cut
files-paste = paste here
files-rename = rename
files-info = information
files-mode = permissions
files-owner = owner
files-group = group
files-compress = compress…
files-extract = extract
files-trash = move to the trash
files-delete = delete for good
files-shred = delete by overwriting

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
files-enter = open
files-parent = up one directory
files-hint = type to filter
# The panels. `no-room` is what the toggle says when it turns one on at a width
# that cannot hold it: the state flips anyway and the screen says why, which is
# the shape `files-sidebar-only` already has. `says` is one fact in full on the
# message line — a panel is twenty-four cells and a path is two hundred
# characters, so the row that had to be cut is the row somebody wants to read.
files-panel-no-room = there is no room for the panel at this width — it needs { $cells } columns
files-fact-says = { $label }: { $value }
# WHAT THE PREVIEW SAYS WHEN THERE IS NO PICTURE. Not a refusal — those come
# from `ronoctua-inspect` and win when there is one — but the answer for a
# directory, a broken link, and a file nothing has been asked about. It used to
# be a large icon of the file's type, which said what the listing's own icon
# column already said and made an empty panel look full.
files-no-preview = no preview
# AND WHAT THE FACTS KEY SAYS WHEN THERE IS NOTHING TO SCROLL. From the
# outside, a key that refuses and a key that is dead look the same.
files-facts-all-shown = every fact is already on screen
files-preview-all-shown = the whole preview is already on screen
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
files-playlist-previous = previous track
files-playlist-next = next track
files-no-sound = this machine has no sound to play it through — the picture goes on without it
files-film-stuck = nothing has come out of it for { $seconds } s — it was stopped
files-film-seek = go to { $at } of { $total }
files-theatre-on = the preview has the whole region
files-theatre-off = the listing has its room back
files-theatre-nothing = nothing is selected, so there is nothing to expand
files-wide-hidden = that panel is hidden, so there is nothing to expand
files-facts-wide-on = the facts have the whole region, in as many columns as it holds
files-facts-wide-off = the facts are back in their column
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
files-yes = yes, apply it
files-no = no, leave it as it is
files-items = { $n } items
files-confirm-mode = Change the permissions of { $what } to { $mode }?
files-no-editor = Emacs is not installed in this profile
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
files-first = the first
files-last = the last
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

cmd-apps-pin = Put the application on the file manager's sidebar
cmd-files-open = File manager
cmd-files-window = File manager, in a window
group-file = file

files-nothing-here = this directory is empty

# THE SIDEBAR — where one starts from.
#
# The first five are ids of `OF_THE_VIEW` and of `menu.rs`;
# `files-actions-have-text` derives them from the id on its own.
files-preview = preview
files-facts = information
files-facts-scroll = page the information
files-columns = permissions and date
files-header = column names
files-sidebar = sidebar
files-sidebar-focus = go to the sidebar, and back
# THE PREVIEW'S OWN KEYS, and the reason they are here rather than derived from
# `menu.rs` is `leader::OF_CLIPBOARD_VIEW`'s: a key that exists only inside a
# `match` arm is a key nothing can enumerate. These are what `LEADER :` lists,
# so each one is written as the thing it DOES and not as the key it is on.
files-preview-scroll = page the preview
files-preview-left = preview left
files-preview-right = preview right
# The film's transport. `files-play` is one key for both, because that is what
# it is: pressing it while it plays is what pauses it.
files-play = play or pause
files-rewind = rewind
files-forward = forward
files-sound = sound
files-theatre = expand the preview
files-facts-wide = expand the facts
files-grid = thumbnails
files-grid-size = thumbnail size
# The shape's. `files-turn-left` is the hand's direction and not an axis — see
# `View::spin`, which is where that convention is written down.
files-turn-left = turn left
files-turn-right = turn right
files-turn-up = turn up
files-turn-down = turn down
files-zoom-in = zoom in
files-zoom-out = zoom out
files-reframe = reframe
files-eject = unmount
files-unpin = unpin
files-pin = put on the sidebar
files-unhide = bring back a hidden sidebar row
files-sidebar-hidden-ask = Bring which one back?
files-sidebar-trash = trash
files-sidebar-open = open { $name }
files-sidebar-mount = mount { $name } and go in
files-sidebar-unmount = unmount { $name }
files-sidebar-launch = run { $name }
files-sidebar-pinned = { $name } is on the sidebar now
files-sidebar-hidden = { $name } is hidden — { $key } brings it back
files-sidebar-gone = { $name } is no longer on the sidebar
files-sidebar-only = this key acts on a sidebar row — { $key } goes there
files-sidebar-nothing-hidden = there is nothing hidden to bring back
files-sidebar-unwritable = could not save the sidebar: { $why }
files-mount-failed = could not mount { $name }: { $why }
files-mount-no-driver = this system cannot read a { $type } filesystem
files-mount-needs-admin = { $name } belongs to the system; { $key } opens a root terminal
files-unmount-busy = { $name } is in use — close what is open on it
files-unmount-needed = { $name } is where the system lives ({ $why }); unmounting is not offered

# THE SIDEBAR'S MENU. Each id becomes `sidebar-<id>`, and a check compares both.
sidebar-open = open
sidebar-mount = mount
sidebar-unmount = unmount
sidebar-eject = eject safely
sidebar-information = information
sidebar-unpin = take off the sidebar
sidebar-format = format
sidebar-menu-on = what to do with { $name }
sidebar-fact-where = mounted at
sidebar-fact-node = device
sidebar-fact-kind = filesystem
sidebar-fact-size = size
sidebar-fact-drive = drive
sidebar-fact-needed = this is where the system lives
sidebar-not-mounted = not mounted
# WHY A DISK IS NOT ON OFFER TO FORMAT. The sentence is built on the host and
# crosses ready-made: the buffer has no language bundle (inv. 11).
format-needed-at = the system lives at { $at }
format-needed-fstab = /etc/fstab mounts this one at boot
format-needed-swap = it is the system's swap
format-refused-busy = { $name } is in use — close what is open on it and try again
format-refused-admin = { $name } belongs to the system, and permission cannot be asked for here
format-refused-other = could not format { $name }: { $why }
# THE FORMAT BUFFER — `LEADER s f`. Three questions, each answered with the
# arrows and Enter. The language is deliberately plain: the vfat row says what
# it is for, not what it is called. Somebody who knows it is FAT32 loses
# nothing; somebody who does not gets an answer instead of a word to look up.
format-title = Format a disk
format-empty = no disk here to format
format-hint-list = ↑↓ choose · Enter continue · Esc close
format-hint-choose = ↑↓ choose · type a name · Enter continue · Esc back
format-hint-confirm = ↑↓ choose · Enter answer · Esc back
format-whole = the whole disk
format-part = one part of the disk
format-blank = empty
format-choose-title = What will this disk be used for?
format-recipe-universal = opens on any computer — pendrive, camera, television
format-recipe-large = the same, but takes large files (over 4 GB)
format-recipe-windows = for use on Windows
format-recipe-linux = for use on Linux
format-recipe-modern = for Linux, keeping snapshots of the disk
format-recipe-missing = XFS is not here: this distro does not carry the program that writes it
format-name = name
format-name-limit = up to 11 letters or numbers
format-confirm-title = This erases everything on it
format-confirm-whole = the whole disk will be erased, with every part of it
format-confirm-part = this part of the disk will be erased
format-confirm-no = no, go back
format-confirm-yes = yes, erase and format
format-working = writing to the disk — do not switch off or pull it out
format-done = done
# THE DETAIL LINE — the same buffer answering a second person.
#
# The rows above are for somebody who does not know what FAT32 is and should not
# have to. These are for somebody who does: the real name, the limits, and what
# the gesture is going to do to the disk. One line, under the cursor, in the
# dimmest colour there is — a beginner never has to read it, and an expert never
# has to guess.
format-tech-universal = vfat (FAT32) · up to 4 GiB per file · no owner or permissions
format-tech-large = exFAT · no practical file size limit · no owner or permissions
format-tech-windows = NTFS · Windows owner and permissions · written by ntfs-3g
format-tech-linux = ext4 · Unix owner and permissions · journalled
format-tech-modern = Btrfs · subvolumes, snapshots, checksums and compression
format-tech-missing = XFS · mkfs.xfs is not installed in this distro
format-tech-part = only this partition is rewritten; the disk's table does not change
format-tech-whole = a fresh partition table (dos up to 2 TiB, gpt above) and ONE partition
format-tech-needed = mounted by the running system · udisks would refuse, and this screen does not even offer
format-tech-nothing = no disk under the cursor
# EACH ACTION'S SECOND REGISTER — what it DOES, for somebody who already knows.
#
# The label is for whoever is looking for a verb ("delete for good"); this line
# is for whoever wants to know exactly what will happen: which call, which
# options, what can be undone. It reaches the message line when the row is under
# the CURSOR — not only under the pointer, because whatever the mouse can do the
# keyboard can do.
files-does-open = opens in the MIME type's default application; a folder, or an archive, is walked into
# THE ONE ACTION IN THIS MENU THAT LEAVES THE MACHINE, and the sentence says
# so before the key is pressed — the same courtesy the glance's two hosts get.
files-does-open-map = opens the coordinate written in this file in the browser's map — the coordinate leaves this machine
files-does-open-map-nowhere = this file has no coordinate written in it
files-does-open-with = choose among the `.desktop` files that declare this MIME type
files-does-terminal = opens the terminal in this folder, as you
files-does-root-terminal = opens the terminal as root, through pkexec
files-does-copy-path = puts the absolute path on the clipboard; opens nothing
files-does-copy = marks for copying; pasting runs it through the queue, which outlives the footer
files-does-cut = marks for moving; on the same filesystem it is rename(2), instant
files-does-paste = runs what was marked; across filesystems it is a copy and then a removal
files-does-rename = renames in place with rename(2); does not cross filesystems
files-does-pin = adds a row to the sidebar, written to $XDG_CONFIG_HOME
files-does-unhide = also lists what begins with a dot
files-does-info = size, owner, mode, times and type; does not touch the contents
files-does-mode = changes the permission bits with chmod(2)
files-does-owner = changes the owner with chown(2); asks for the root password
files-does-group = changes the group with chown(2)
files-does-compress = builds an archive through the queue, without blocking the listing
files-does-extract = extracts into a folder named after the archive
files-does-restore = puts it back at the origin path the trash recorded
files-does-trash = moves it to the XDG trash; does not cross filesystems
files-does-delete = unlink(2) or rmdir(2) straight away: no trash, no way back
files-does-shred = overwrites before deleting; on copy-on-write btrfs that guarantees nothing

# And the sidebar menu's, which talk to udisks.
sidebar-does-open = enters the folder; for an unmounted device it opens its sheet
sidebar-does-mount = udisks2's Filesystem.Mount — it picks the point and the options
sidebar-does-unmount = Filesystem.Unmount, never forced: it refuses while something is open
sidebar-does-eject = unmounts, locks an encrypted disk again, and cuts the drive's power (Drive.PowerOff): what makes it safe to pull
sidebar-does-information = label, node, filesystem, size and drive
sidebar-does-unpin = removes the row; a distro default comes back hidden, yours is removed
sidebar-does-format = opens the formatting buffer already pointed at this target




files-disks-not-watching = the disks will not refresh on their own: { $why }
files-nothing-to-choose = there is nothing to choose from
files-copied = copied
files-not-copied = could not copy
files-nothing-opens = no application declares that it opens { $type }
files-no-pkexec = `pkexec` was not set in nix/rust.nix
files-no-systemctl = `systemctl` was not set in nix/rust.nix
files-refused = { $why }

# The table's header. Clicking one sorts by it; clicking again reverses. The
# `s` key goes round the same cycle.
files-column-icon = kind
files-column-name = name
files-column-perms = permissions
files-column-owner = owner
files-column-group = group
files-column-size = size
files-column-modified = modified

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
files-explain-owner = { $who } owns it; clicking changes that
files-explain-group = the group is { $who }; clicking changes that
files-explain-size = exactly { $bytes } bytes
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
files-restore = put back where it came from
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
# OVER the picture, on its last line, and only when some face did not fill. A
# piece that is part outline is a piece somebody should be told about — and the
# panel is where they are looking.
files-preview-outlined = { $n ->
        [one] one face drawn as outline
       *[other] { $n } faces drawn as outline
    }

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
globe-address-near = near { $place }, about { $km } km away
globe-address-nowhere = no named place near this coordinate
globe-behind = this place is on the far side of the globe
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
cmd-atlas-open = world data
cmd-atlas-window = world data in a window
group-geography = geography
files-explain-globe = drag to turn it, the wheel to come closer
files-explain-globe-dot = where this file says it was made — Enter frames it
files-explain-globe-address = { $value } — press to open this coordinate in the browser's map.
globe-north = move the globe north
globe-south = move the globe south
globe-east = move the globe east
globe-west = move the globe west
files-fact-where = where
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

# === The transfer queue — `LEADER t` =======================================
#
# Seven operations used to go off to a thread saying nothing on screen: paste,
# delete, shred, trash, compress, extract and measure. They all join one queue
# now, and it is that queue the modeline summarises and the panel details.
group-transfer = transfers
# `c` stopped being a command and became a group: the clipboard captures what
# you copy, and the screen captures what you see.
group-miscellaneous = miscellaneous

cmd-transfers-open = the transfer queue
# The VIEW's keys. The id IS the key, as in the file manager: `cmd-` belongs
# to the command registry, and these are not in it.
transfers-down = down
transfers-up = up
transfers-pause = hold or release this transfer
transfers-cancel = give this transfer up
transfers-forget = take this row off the list
transfers-sweep = take everything finished off the list
transfers-launch = open what was installed
transfers-detail = the transfer, field by field
transfers-answer = answer
transfers-give-up-no = No, let it carry on
transfers-give-up-yes = Yes, give it up

transfers-empty = nothing under way
transfers-offline = the transfer queue is not up

# What the operation IS.
transfers-work-copy = copy
transfers-work-move = move
transfers-work-delete = delete
transfers-work-shred = overwrite
transfers-work-trash = move to the trash
transfers-work-restore = put back from the trash
transfers-work-compress = compress
transfers-work-extract = extract
transfers-work-measure = measure

# Where it is.
transfers-stage-weighing = weighing
transfers-stage-running = under way
transfers-stage-paused = held
transfers-stage-done = done
transfers-stage-failed = failed
transfers-stage-cancelled = given up

# Who moves it. For now only this machine's disk.
transfers-engine-local = disk

# The subject, when there is more than one.
transfers-and-more = { $name } and { $rest } more
# Where it goes. The arrow is the only drawing here, and it is not chrome: it is
# the preposition.
transfers-into = → { $where }
# How many items, for the operations that count things rather than bytes.
transfers-items = { $done } of { $total }

# ---------------------------------------------------------------------------
# THE SAME QUEUE, IN A WINDOW OF ITS OWN — `LEADER t w`, the wasip2 buffer.
#
# Not a second implementation: the same socket, the same rows, with room for
# all of them at once and a detail screen the footer's region cannot hold. Its
# own keys live under `SPC m`, like every buffer's.
# ---------------------------------------------------------------------------
cmd-transfers-window = the transfer queue, in a window
cmd-transfers-add = download whatever is on the clipboard
cmd-transfers-video = download the video of whatever is copied
cmd-transfers-audio = download only the audio of whatever is copied
cmd-transfers-subtitles = download only the subtitles of whatever is copied
cmd-transfers-add-as-me = download whatever is copied, as me
cmd-transfers-video-as-me = download the video, as me
cmd-transfers-audio-as-me = download only the audio, as me
cmd-transfers-subtitles-as-me = download only the subtitles, as me
# The group that costs anonymity — which is why it is a prefix and not a
# switch kept somewhere else.
group-identified = as me (costs anonymity)
transfers-queued-as-me = in the queue, AS YOU — this one is not anonymous
transfers-window-field-identity = asked by
transfers-identity-mine = you
transfers-identity-anonymous = nobody
transfers-nothing-copied = nothing has been copied
transfers-not-an-address = what is copied is not an address
transfers-queued = in the queue
# THE WINDOW'S commands: the panel's table, bound under the local leader.
cmd-transfers-down = the next transfer
cmd-transfers-up = the previous transfer
cmd-transfers-pause = hold or let go
cmd-transfers-cancel = give up
cmd-transfers-forget = drop from the list
cmd-transfers-sweep = clear what has finished
cmd-transfers-launch = open what it installed
cmd-transfers-detail = this row, in detail

transfers-window-title = transfers
transfers-window-detail = the transfer, field by field
transfers-window-empty = nothing in the queue
transfers-window-unknown = not known
transfers-window-confirm-cancel = Give this transfer up? What was already written at the destination is deleted.

transfers-window-field-engine = moved by
transfers-window-field-stage = state
transfers-window-field-errand = operation
transfers-window-field-subject = right now
transfers-window-field-done = done
transfers-window-field-total = total
transfers-window-field-trouble = trouble
transfers-window-field-answer = answer

# The engines still to come, and the network operations. They are here already
# because the buffer names each one as a constant — and a word with no
# translation fails the check instead of showing up on screen.
transfers-engine-fetch = link
transfers-engine-media = media
transfers-engine-torrent = torrent
transfers-engine-atlas = region

transfers-work-fetch = download
transfers-work-media = download video
transfers-work-audio = download audio
transfers-work-subtitles = download subtitles
transfers-work-torrent = torrent
transfers-work-region = install a region's streets
# The person's own programs: an order of the software center, run by its own
# program. The row's first half is the program's NAME, so the work is a verb
# with no object of its own.
transfers-engine-software = programs
transfers-work-software = change a program


# ---------------------------------------------------------------------------
# THE BROWSER EXTENSION.
#
# FIFTEEN keys, and `nix/scripts/gen-extension.py` demands every one of them in
# both languages: a missing one does not fall back to English, it puts the
# key’s own name on a button. The popup writes no words of its own — it builds
# the screen out of these.
# ---------------------------------------------------------------------------
ext-title = ronoctua
ext-queue-up = the queue is up
ext-queue-down = the queue is not up
ext-video = video, at the best quality
ext-audio = the audio alone
ext-subtitles = the subtitles alone
ext-scan = look for media on this page
ext-found-none = nothing found on this page
ext-page-not-media = this tab has no address to download
ext-sent = sent to the queue
ext-failed = the queue refused
ext-intercept-on = downloads go to the queue
ext-intercept-off = downloads stay in the browser
ext-send-link = download this link through ronoctua
ext-send-media = download this page’s media
ext-as-me = as you — not anonymous
ext-anonymous = anonymous


# ===================================================================
# THE BRAIN'S SCREEN — LEADER b, and a window of its own
#
# What the machine does by itself, when it does it, and what came of it.
#
# No sentence here spells a date. The buffer draws those ISO, because a
# component cannot translate (invariant 11) and *19 Aug* against *19 ago* is
# exactly the kind of thing it must not invent. What is written below is the
# words AROUND the numbers, and never the numbers.
#
# A NOTE ON HOW THESE ARE WRITTEN, because the last set was rewritten from
# scratch and the owner said why: a title names the thing it is over, a label
# names the field beside it, and a sentence says what something does. None of
# them is a remark. `What it does` over a list of tasks, `The machine` over two
# percentages and `How it last went` over one line were all the same mistake —
# a heading talking about the screen instead of naming what is on it.
# ===================================================================

# THE COMMANDS, and they are SHORT on purpose: they are the phrases in the
# header's ladder, where each one has about a dozen cells and everything after
# a long one falls off the end of the line.
cmd-brain-open = Brain
cmd-brain-run = Run it now
cmd-brain-rehearse = Rehearse it
cmd-brain-abort = Stop it
cmd-brain-proceed = Start now
cmd-brain-reset-steps = Reset steps
cmd-brain-interval = How often
cmd-brain-window = Change time
cmd-brain-switch = Switch it off
cmd-brain-keys = Every key
cmd-brain-log = Live log
cmd-brain-row-also = The row's other action

# THE PAGE ITSELF.
brain-title = Brain
brain-nothing = Nothing is set up yet
brain-tasks = Tasks
brain-activity = Activity
brain-no-activity = Nothing has happened yet
brain-log = Live log
brain-no-log = Nothing logged yet
brain-schedule = Schedule
brain-steps = Steps
brain-history = History

# WHAT THE HEADER TEACHES about the page's own keys. `cmd-*` covers the rest.
brain-do-open = Open it
brain-do-back = Back
brain-do-pane = Other half
brain-do-change = Change this
brain-do-keys = Every key
about-brain-do-open = Open the task under the cursor: its schedule, its steps and what it did the last times.
about-brain-do-pane = Take the keyboard to the other half of the page.
about-brain-do-change = Change the setting or the step under the cursor.
about-brain-do-back = Go back to the list of every task.
about-brain-keys = Open this line to see every key the page answers to, or close it again.
about-brain-run = Run this task now, whatever its schedule says.
about-brain-rehearse = Rehearse this task: every step is walked and nothing on the machine changes.
about-brain-switch = Switch this task off, or on again; switched off, it never runs by itself.
about-brain-interval = Step to the next cadence this task runs at.
about-brain-window = Step to the next part of the day this task may run in.
about-brain-reset-steps = Put every step of this task back as the distro ships it.
about-brain-proceed = Start the task that is counting down now, without waiting for the count.
about-brain-abort = Stop the task that is running or counting down.
about-brain-log = Show the brain's own log as it is written — every tick, what it decided, every warning and error — or go back.
about-brain-row-also = Do the other thing the row under the cursor offers: on an error not yet seen, ignore its type from now on.

# THE SCHEDULE'S ROWS. Each is a field's NAME, so each is a noun phrase — and
# the value beside it is the answer, never half of a sentence that started in
# the label.
brain-enabled = Runs on its own
brain-on = Yes
brain-off = No, only when you ask
brain-every = Runs every
brain-days = days
brain-part = Runs between
brain-next = Next run
brain-soon = Starts in
brain-run-now = Run it now
brain-rehearse-it = Rehearse it
brain-rehearsal = Rehearsing now
brain-never = Never
brain-needs-root = Runs as root
brain-flake-owned = In the flake
brain-changed = Changed by you

# THE FOOTER'S FIELDS.
brain-last-run = Last run

# THE TASKS. `brain-task-<id>` names one and `<id>-does` says what it does —
# the second is a paragraph in the footer, so it has room to be a whole
# explanation rather than a line.

# WHERE A TASK IS RIGHT NOW. A short phrase each: they sit in a column twelve
# cells wide on the home page.
brain-standing-idle = Waiting
brain-standing-queued = In the queue
brain-standing-running = Running

# AND HOW A RUN ENDED. `failed` is the honest word for a run that did not
# finish; whether that was a fault or a decision is the reason's job.
brain-outcome-skipped = Did not run
brain-outcome-done = Worked
brain-outcome-failed = Failed
brain-outcome-held = Waited

# WHAT IS IN THE WAY, in the column beside a task's name.
brain-hold-ram = Memory busy
brain-hold-cpu = Processor busy
brain-hold-battery = On battery
brain-hold-busy = One at a time
brain-hold-network = No network
brain-hold-asleep = Asleep

# AND THE SAME THING AS A WARNING, on the line under the header — the one
# place on this screen that is allowed to alarm, and only while something is
# actually in the way.
#
# A WHOLE SENTENCE PER REASON and not one sentence with the reason put into it:
# a `Run` carries a key and nothing else (invariant 11). It is also the better
# of the two — "the processor is too busy" says the thing, "held: cpu" makes
# somebody guess.
brain-held-ram = Nothing will start while this much of the memory is in use. It goes on by itself when the machine is quieter.
brain-held-cpu = Nothing will start while the processor is this busy. It goes on by itself when the machine is quieter.
brain-held-battery = Nothing will start on battery. Plug the machine in, or change that in brain.json.
brain-held-other = Something is in the way. The task it is holding says what.

# AND WHY A RUN DID NOT HAPPEN AT ALL. `aborted` is NOT an error — it is the
# person having decided, and the screen agreeing with them.
brain-why-off = Switched off
brain-why-no-systemctl = No systemd here
brain-why-runaway = It stopped reporting and the waiting was given up. It may well still be going.
brain-why-aborted = You stopped it. Nothing went wrong.
brain-why-postponed = You said not now, and the next moment moved.
brain-why-ram = The memory was busy
brain-why-cpu = The processor was busy
brain-why-battery = It was on battery
brain-why-busy = Something else was running
brain-why-network = There was no network
brain-why-asleep = The machine was asleep

# WHICH PART OF THE DAY a task may start in. The hours are the same digits in
# every language; the words around them are not.
brain-part-any = any hour
brain-part-night = 02:00 and 05:00
brain-part-morning = 08:00 and 12:00
brain-part-afternoon = 13:00 and 18:00
brain-part-evening = 19:00 and 23:00

# THE BAND UNDER THE MINIBUFFER — one line, and only while a countdown runs.
#
# It is not the brain's screen: it is the warning that arrives without anybody
# having asked for it, on a line of its own below the pair (invariant 14 — it is
# the same surface growing, not a popup). Three sentences, because that is all
# that fits: what is about to happen, and the two answers to it.
#
# The task's own name carries the subject — `brain-task-<id>` — so a task added
# tomorrow inherits this sentence instead of needing one of its own.
brain-band-soon = { $task } — starts in { $seconds } s
brain-band-abort = not now
brain-band-proceed = now
hint-brain-countdown = how long is left before it starts

# AND WHAT THE TWO ANSWERS COST. `not now` is not "never": the run leaves the
# queue and the next moment moves, which is the only thing that actually stops
# a countdown — taking it out of the queue alone would put it straight back in
# on the next beat, and the person would answer the same question every half
# minute.
echo-brain-postponed = Not now — it comes back at its next moment
echo-brain-proceeding = Starting now
echo-brain-idle = There is nothing about to happen
echo-brain-unreachable = The brain is not answering

# THE UPDATE, WATCHED — the modeline's mark, the leader's group for the view
# that watches a run, and the countdown's two answers that view teaches too.
#
# The mark shows only while something runs and goes on its own. The update's
# commands, its steps and the view's own words are its brain module's texts,
# in that module's folder.
group-update = update
brain-abort = stop
brain-proceed = start now
hint-update-mark = click to watch the update happen

## The theme chooser — LEADER s t. The themes are listed by id and have no
## key; these are the options beside them — the desk under the windows, the
## ground of the footer, the ground of the windows and the borders — their
## answers, and the hint. The absolute desk is `theme-backdrop-black` under a
## dark theme and `theme-backdrop-white` under a light one; the near-white,
## `theme-backdrop-near`, is offered only under a light theme.
theme-option-theme = Theme
theme-option-backdrop = Variations
theme-option-footer = Footer
theme-option-windows = Windows
theme-option-borders = Borders
theme-backdrop-black = Black
theme-backdrop-white = White
theme-backdrop-near = Near white
theme-backdrop-theme = The theme's colour
theme-ground-theme = The theme's
theme-ground-desk = The desk's
theme-borders-standard = Standard
theme-borders-discreet = Discreet
theme-hint = { $apply } applies · { $left }/{ $right } changes column
echo-theme-unsaved = the theme could not be saved: { $error }
## What the person's theme file says that was refused, said where the file is
## read. `$file` is the file's name, `$field` its own key (theme, backdrop,
## footer, windows, borders) and `$value` what it said — the file's words.
echo-theme-refused = { $file } says { $field } is “{ $value }”, which is not an option here — refused
echo-theme-unreadable = { $file } could not be read, so every choice in it was refused: { $error }
## A write the chooser REFUSED: the file is there and does not read whole,
## and writing the whole choice over it would lose what it says. `$why` is
## the sentence that says what did not read (`echo-theme-refused` or
## `echo-theme-unreadable`).
echo-theme-kept = { $file } was not written, so nothing in it is lost; fix it or remove it by hand: { $why }

## Power — the battery, the charger, the profile and sleep (LEADER s p)

cmd-power-open = Power
cmd-power-window = Power in a window
cmd-power-profile-performance = Power profile: performance
cmd-power-profile-balanced = Power profile: balanced
cmd-power-profile-saver = Power profile: power saver
cmd-power-keep-awake = Keep the machine awake (toggle)
cmd-power-keep-lid = Ignore the lid (toggle)
cmd-power-thresholds = Charge thresholds
cmd-power-suspend = Suspend
cmd-power-hibernate = Hibernate
cmd-power-reboot = Reboot
cmd-power-off = Power off
cmd-power-enter = Apply, arm or confirm the row
cmd-power-less = Lower the threshold under the cursor
cmd-power-more = Raise the threshold under the cursor
cmd-power-up = Up one row
cmd-power-down = Down one row
group-power = power

# THE HINT LINE reads each id of the view's key table as a short text.
power-enter = apply
power-less = less
power-more = more
power-up = up
power-down = down

power-title = Power

# The modeline's two marks, on the line under the pointer.
hint-battery = { $percent }% { $state }, { $time }, { $rate } W — click opens power
hint-profile = profile: { $profile } — click opens power

# The view's sections, one heading each.
power-heading-battery = Battery
power-heading-adapter = Charger
power-heading-profile = Profile
power-heading-thresholds = Charge thresholds
power-heading-awake = Keep awake
power-heading-lid = Lid and idling
power-heading-actions = Actions
power-heading-peripherals = Peripherals

power-no-battery = no battery in this machine
power-no-adapter = no charger known
power-plugged = plugged in
power-unplugged = unplugged
power-no-daemon = power-profiles-daemon is not running

power-fact-charge = charge
power-fact-time = time
power-fact-rate = rate
power-fact-energy = energy
power-fact-health = health
power-fact-health-daemon = health, by the firmware
power-percent = { $percent }%
power-fact-cycles = cycles
power-fact-voltage = voltage
power-fact-temperature = temperature
power-fact-technology = chemistry
power-fact-make = make
power-fact-supply = supply
power-fact-behaviour = charge behaviour
power-fact-charge-type = charge type
power-fact-idle = idle

power-time-empty = { $time } left
power-time-full = { $time } to full
power-time-unknown = time unknown
power-duration = { $h } h { $m } min
power-duration-minutes = { $m } min
power-rate = { $rate } W
power-energy = { $now } / { $full } Wh
power-health = { $health }% ({ $full } of { $design } Wh)
power-voltage = { $v } V
power-temperature = { $t } °C

power-state-unknown = unknown
power-state-charging = charging
power-state-discharging = discharging
power-state-empty = empty
power-state-full = full
power-state-pending-charge = waiting to charge
power-state-pending-discharge = waiting to discharge

power-technology-unknown = unknown
power-technology-lithium-ion = lithium-ion
power-technology-lithium-polymer = lithium-polymer
power-technology-lithium-iron-phosphate = lithium iron phosphate
power-technology-lead-acid = lead-acid
power-technology-nickel-cadmium = nickel-cadmium
power-technology-nickel-metal-hydride = nickel-metal hydride

power-profile-performance = performance
power-profile-balanced = balanced
power-profile-power-saver = power saver
power-profile-wanted = (your choice)
power-profile-not-offered = not offered by this machine
power-degraded = performance degraded: { $why }
power-inhibited = performance held off: { $why }
power-hold = { $who } holds { $profile }: { $why }
power-battery-aware = lower the profile when the battery runs low
power-on = on
power-off-label = off

power-threshold-start = start charging below ({ $supply })
power-threshold-end = stop charging at ({ $supply })
power-threshold-pending = { $value }% — RET writes
power-threshold-none = not adjustable
power-preset-longevity = 40–80: a laptop that lives on the desk
power-preset-desk = 60–80: mostly plugged in
power-preset-full = full charge: a trip
power-thresholds-unsupported = this battery has no charge thresholds
power-thresholds-readonly = the thresholds are read-only for you — the udev rule has not reached this battery

power-keep-awake = keep awake: no sleep, no idle action
power-keep-lid = ignore the lid: closing it does nothing
power-inhibitor = { $who } holds { $what }: { $why }

power-lid-closed = the lid is closed
power-lid-open = the lid is open
power-lid-action = closing the lid
power-lid-action-external = closing the lid, on the charger
power-lid-action-docked = closing the lid, docked
power-lid-action-value = { $action }
power-action-unset = the default
power-power-key = the power key
power-idle-action = { $action } after { $time }
power-idle-never = nothing
power-critical-action = at the critical level, UPower will
power-blocked = blocked right now

power-action-suspend = suspend
power-action-suspend-then-hibernate = suspend, then hibernate
power-action-hybrid-sleep = suspend and hibernate at once
power-action-hibernate = hibernate
power-action-reboot = reboot
power-action-off = power off
power-action-confirm = { $action }? RET confirms, Esc cancels
power-cannot = not available
# THE DAEMONS' WORDS — logind's kebab-case and UPower's CamelCase land on
# the same key (`power_words::daemon_word`); a word outside this list is shown raw.
power-duration-hours = { $h } h
hint-battery-no-rate = { $percent }% { $state }, { $time } — click opens power
notif-battery-critical-body-unset = { $percent }% left. The machine will act on its own soon.
power-word-ignore = do nothing
power-word-poweroff = power off
power-word-reboot = reboot
power-word-halt = halt
power-word-kexec = reboot through kexec
power-word-suspend = suspend
power-word-hibernate = hibernate
power-word-hybrid-sleep = go into hybrid sleep
power-word-suspend-then-hibernate = suspend, then hibernate
power-word-lock = lock the screen
power-word-factory-reset = factory reset
power-word-soft-reboot = soft reboot
# THE KERNEL'S TWO CHARGE VOCABULARIES — `charge_behaviour` and `charge_type`,
# from the `power_supply` class; the raw word shows only for one we do not know.
power-behaviour-auto = charge normally
power-behaviour-inhibit-charge = hold the charge where it is
power-behaviour-inhibit-charge-awake = hold the charge while awake
power-behaviour-force-discharge = run off the battery
power-charge-type-unknown = unknown
power-charge-type-na = not applicable
power-charge-type-trickle = trickle
power-charge-type-fast = fast
power-charge-type-standard = standard
power-charge-type-adaptive = adaptive
power-charge-type-custom = custom
power-charge-type-long-life = long life
power-charge-type-bypass = straight from the charger
# WHAT AN INHIBITOR HOLDS — nouns, because "holds sleep" is a thing held and the
# lid's `sleep` above is an action; in Portuguese they are two words.
notif-battery-critical-body-ignore = { $percent }% left, and nothing will stop it. Plug in now.
power-what-sleep = sleep
power-what-idle = idling
power-what-shutdown = shutdown
power-what-handle-power-key = the power key
power-what-handle-suspend-key = the suspend key
power-what-handle-hibernate-key = the hibernate key
power-what-handle-lid-switch = the lid
power-what-handle-reboot-key = the reboot key
power-word-sleep = sleep

power-peripheral = { $kind }: { $model }
power-kind-unknown = device
power-kind-line-power = charger
power-kind-battery = battery
power-kind-ups = UPS
power-kind-monitor = monitor
power-kind-mouse = mouse
power-kind-keyboard = keyboard
power-kind-pda = PDA
power-kind-phone = phone
power-kind-media-player = media player
power-kind-tablet = tablet
power-kind-computer = computer
power-kind-gaming-input = gamepad
power-kind-pen = pen
power-kind-touchpad = touchpad
power-kind-modem = modem
power-kind-network = network device
power-kind-headset = headset
power-kind-speakers = speakers
power-kind-headphones = headphones
power-kind-video = video device
power-kind-other-audio = audio device
power-kind-remote-control = remote control
power-kind-printer = printer
power-kind-scanner = scanner
power-kind-camera = camera
power-kind-wearable = wearable
power-kind-toy = toy
power-kind-bluetooth = Bluetooth device

# What the doors say when a daemon refused, and what the window cannot do.
door-no-footer = The footer could not be reached to run this
power-what-profile = the profile
power-what-lock = the lock
power-what-thresholds = the thresholds
power-what-action = the action
power-refused-authority = not allowed for this session
power-refused-absent = the daemon is not there, or the hardware cannot
power-refused-invalid = the value was refused
power-refused-other = refused
echo-power-refused = { $what } { $why }
echo-power-profile-not-offered = { $wanted } is not offered by this machine; { $applied } applied

# THE THREE ALARMS, raised by the footer when UPower's level changes.
notif-power-app = power
notif-bluetooth-app = bluetooth
notif-bluetooth-pairing-summary = Something wants to pair
notif-bluetooth-pairing-keys = { $yes } says yes, { $no } says no — after this card goes.
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

# WHAT THE MACHINE DID, said on the minibuffer's line (`events.rs`). The
# summary is the line; the body is only read when the line was busy and the
# event came as a notification instead — and then the last sentence says so.
notif-privacy-app = privacy
notif-network-app = network
notif-minibuffer-was-busy = This usually shows in the minibuffer, but it was busy, so it came as a notification.
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
notif-network-lost-summary = { $network } dropped
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

# THE WAY OUT — the three marks, and what the daemon believes.
network-way = the internet
network-airplane = aeroplane mode
network-tunnel-drop = RET takes it down
cmd-network-airplane = Aeroplane mode (toggle)
echo-network-refused = The network refused: { $why }
echo-view-busy = another view has the footer: Esc closes it first
hint-net-off = connected, and nothing out there — click opens the network
hint-net-portal = something is asking you to log in — click opens the network
hint-airplane = aeroplane mode: every radio and cable is off — click turns it back on
hint-vpn = { $name } is up — click opens the network
hint-tailscale = on the { $tailnet } tailnet — click opens it
hint-tor = going out through Tor — click opens it
hint-clip-on = the clipboard is syncing — click opens it
hint-clip-off = the clipboard is not syncing — click opens it
hint-history-on = the shell history is syncing — click opens it
hint-history-off = the shell history is not syncing — click opens it
network-way-unknown = not decided yet
network-way-none = no way out
network-way-portal = a login page is in the way
network-way-limited = connected, but it is not answering
network-way-full = it answers

# THE VIEW — one list, four headings, and the same rows in both doors.
network-heading-overview = at a glance
network-heading-links = cable and Wi-Fi
network-heading-wifi = Wi-Fi
network-heading-remembered = remembered networks
network-no-remembered = this machine remembers no network it cannot see
network-out-of-range = not in range
network-heading-tunnels = tunnels
network-radio = Wi-Fi radio
network-on = on
network-off = off
network-no-link = this machine has no cable and no radio
network-no-traffic = nothing has gone in or out yet
network-no-tunnel = no tunnel here
network-tunnel-raise = RET brings it up
network-state-connected = connected
network-state-connecting = connecting…
network-state-disconnected = not connected
network-fact-network = Wi-Fi network
network-fact-band = Wi-Fi band
network-fact-signal = signal strength
network-fact-rate = link speed
network-fact-address = this machine's address
network-import-address = your address on it
network-fact-address-tailnet = IPv4 on the tailnet
network-fact-address-tailnet6 = IPv6 on the tailnet
network-fact-gateway = router
network-fact-dns = DNS server
network-fact-received = received
network-fact-received-boot = received since boot
network-fact-sent = sent
network-fact-sent-boot = sent since boot
network-heading-ports = open ports
network-heading-monitor = traffic
network-no-ports = nothing is listening
network-port-not-ours = not ours to see
network-ports-more = { $count ->
    [one] and one more of ours
   *[other] and { $count } more of ours
  }
network-per-second = { $bytes }/s
number-decimal-mark = .
# THE UNITS ARE THE SAME IN EVERY LANGUAGE; the DECIMAL SEPARATOR is not, and
# that is why these are keys with a number in them rather than a string built
# in the crate.
network-rate-kilobits = { $rate } kb/s
network-rate-megabits = { $rate } Mb/s
network-rate-gigabits = { $rate } Gb/s
network-no-rate = not up
network-band = { $ghz } GHz
network-band-unknown = an unusual frequency
network-heading-lan = machines on this network
network-lan-nobody = nobody has been spoken to yet
network-lan-gateway = this network's router
network-lan-this-machine = this machine
network-lan-looking = looking for the others…
network-lan-answered = answered
network-lan-refused = this machine may not send a ping: the system does not let it open the socket
network-lan-swept = { $answered ->
    [one] one machine answered out of { $asked } asked, { $range }
   *[other] { $answered } machines answered out of { $asked } asked, { $range }
  }
cmd-network-lan = Look for the other machines here
network-lan = look
network-import = bring in
network-section-next = section
network-section-previous = section back
network-hotspot-start = share this connection
network-hotspot-stop = stop sharing
network-hotspot-cost = the radio leaves the network it is on: what it shares is the cable
network-hotspot-on = this machine is the network now
network-hotspot-password = password for this hotspot
echo-network-hotspot = sharing as { $network }, password { $password }
echo-network-hotspot-failed = could not share: { $reason }
cmd-network-import = Bring a VPN in from a file
cmd-network-section-next = Next section of the network view
cmd-network-section-previous = Previous section of the network view
cmd-network-tor-open = Tor
cmd-network-tor-more = Turn this Tor setting forwards
cmd-network-tor-less = Turn this Tor setting back
cmd-network-tor-back = Leave Tor's settings
ask-network-import = Path of the .conf or .ovpn file
network-import-file = file to bring in
network-import-kind = kind of VPN
network-import-wireguard = WireGuard
network-import-openvpn = OpenVPN
network-import-endpoint = goes to
network-import-dns = it would move your DNS to
network-import-everything = everything this machine sends would go through it
network-import-asks = it asks for a user and a password as soon as it is brought in, and keeps them
network-import-bring = bring it in
network-import-leave = leave it where it is
network-import-where = Path of the .conf or .ovpn file
network-import-help = Read before importing: the file decides where your traffic goes.
echo-network-imported = { $network } brought in
echo-network-not-a-tunnel = { $file } is neither a WireGuard nor an OpenVPN file
echo-network-import-failed = could not bring { $file } in: { $reason }

# THE TWO TUNNELS. Neither is on at rest: Tor does not start at boot and
# Tailscale needs a login, so a machine that uses neither draws neither
# heading.
network-heading-tailscale = Tailscale
network-heading-tor = Tor
network-tailscale-state = the tailnet connection
network-tailscale-running = up
network-tailscale-stopped = down
network-tailscale-needslogin = waiting for a login
network-tailscale-nostate = starting
network-tailscale-starting = starting
network-tailscale-needsmachineauth = waiting to be approved on the tailnet
network-tailscale-inuseotheruser = another person on this machine is using it
network-tailscale-login = log in
network-tailscale-logout = log out
network-tailscale-routes = accept tailnet routes
network-tailscale-shields = refuse what comes in
network-tailscale-online = online
network-tailscale-offline = offline
network-tailscale-exit = everything goes through here
network-tor-state = the Tor service
network-tor-ready = a circuit is up
network-tor-not-ready = no circuit
network-tor-bootstrap = starting, { $percent }%
network-tor-circuits = { $count ->
    [one] one circuit open
   *[other] { $count } circuits open
  }
network-tor-identity = ask for a new identity

# TOR, WHOLE. The section is drawn whether Tor is running or not, and that is
# the fix rather than a feature: it used to live behind "the control socket
# answered", which is only true once Tor is already up — so a stopped Tor had
# no heading, no row and no key, and the one switch that existed was written to
# STOP. There was no way in the whole shell to turn Tor on.
network-tor-running = running
network-tor-stopped = stopped
network-tor-moving = changing
network-tor-absent = not installed on this machine
network-tor-unknown = cannot be asked
network-tor-start = start it
network-tor-stop = stop it
network-tor-autostart = start it with the session
network-tor-settings = settings
network-tor-sheet = Tor
network-tor-browser = open a browser through Tor
# WHAT THE FLAKE OWNS. The row is drawn and reachable and it refuses, saying
# where the value comes from — the shape the format buffer's disabled XFS row
# already has. Editing it would be a key whose effect the generated torrc
# undoes before it is pressed.
network-tor-from-flake = set by the system's configuration
network-tor-heading-service = The service
network-tor-heading-path = Where it comes out
network-tor-heading-bridges = Bridges
network-tor-heading-onion = Onion services
network-tor-heading-advanced = Rarely touched
network-tor-advanced-about = These change how Tor builds circuits. The defaults are what most people want.
ask-tor-setting = The new value — empty clears it
ask-tor-lines = One line to add — empty clears them all
network-tor-open = settings
network-tor-back = back
network-tor-edit = change
network-tor-more = more
network-tor-less = less
network-tor-drawer = the drawer
echo-tor-written = Tor's settings are written
echo-tor-refused = Tor would not take that: { $reason }
echo-tor-not-running = written; Tor will read it when it starts

# THE TABLE, from `assets/tor-options.nix` through the generator. A key nobody
# wrote draws as `[tor-opt-…]` on screen, which is why
# `tor-options-have-text` counts them.
tor-opt-socks-port = SOCKS address
tor-opt-cookie-authentication = cookie authentication
tor-opt-cookie-file = where the cookie is kept
tor-opt-cookie-group-readable = the tor group may read the cookie
tor-opt-exit-nodes = come out through
tor-opt-entry-nodes = go in through
tor-opt-exclude-nodes = never use
tor-opt-exclude-exit-nodes = never come out through
tor-opt-strict-nodes = treat those lists as a rule, not a preference
tor-opt-circuit-lifetime = keep using a circuit for
tor-opt-use-bridges = use bridges
tor-opt-bridge-lines = bridges
tor-opt-bridge-transport = transport
tor-opt-firewall-ports = ports this network lets out
tor-opt-fascist-firewall = only go out on 80 and 443
tor-opt-onion-directory = where the onion key is kept
tor-opt-onion-ports = ports this onion service answers
tor-opt-onion-single-hop = one hop only — fast, and not anonymous
tor-opt-safe-socks = refuse SOCKS requests that would leak the name
tor-opt-new-circuit-period = consider a new circuit every
tor-opt-circuit-build-timeout = give up building a circuit after
tor-opt-entry-guards = how many entry guards
tor-opt-use-entry-guards = keep going in through the same relays
tor-opt-distinct-subnets = keep a circuit's relays on different networks
tor-opt-ipv6 = use IPv6
tor-opt-prefer-ipv6 = prefer IPv6
tor-opt-connection-padding = cover traffic
tor-opt-reduced-padding = less cover traffic
tor-opt-avoid-disk-writes = write to disk as little as possible
tor-opt-dormant-timeout = go idle after
tor-opt-dormant-on-start = start idle
tor-opt-socks-proxy = reach the network through a SOCKS proxy
tor-opt-https-proxy = reach the network through an HTTPS proxy
tor-opt-log-level = what Tor writes to the log
tor-unit-seconds = seconds
tor-unit-guards = guards
tor-word-none = none
tor-word-obfs4 = obfs4
tor-word-snowflake = snowflake
tor-word-meek-lite = meek
tor-word-auto = as Tor decides
tor-word-0 = off
tor-word-1 = on
tor-word-notice-syslog = the usual
tor-word-info-syslog = more
tor-word-debug-syslog = everything
network-fact-name = name on the tailnet
network-fact-tailnet = your tailnet's name
network-fact-magic-dns = MagicDNS
network-fact-version = Tor's version
network-fact-socks = SOCKS address
echo-network-login = open { $url } to finish logging in
# THE HINT LINE'S WORDS — the id read as a text key, short because the line
# is one line. The long form is `cmd-<id>`, which the manual reads.
network-up = up
network-down = down
network-connect = connect
network-scan = scan
network-wifi-toggle = radio
network-disconnect = disconnect
network-forget = forget
network-link = link
network-title = network
cmd-network-window = Network in a window
cmd-network-up = Up one row
cmd-network-down = Down one row
echo-network-tunnel-up = { $network } is coming up
echo-network-tunnel-down = { $network } is down
echo-network-deleted = { $network } deleted

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

cmd-bluetooth-open = Bluetooth
cmd-bluetooth-window = Bluetooth in a window
cmd-bluetooth-scan = Look for devices nearby
cmd-bluetooth-power = Turn the radio on or off
cmd-bluetooth-discoverable = Let other devices see this machine
cmd-bluetooth-reconnect = Reconnect the trusted devices
cmd-bluetooth-disconnect-all = Disconnect every device
cmd-bluetooth-accept = Say yes to the pairing
cmd-bluetooth-refuse = Say no to the pairing
cmd-bluetooth-device = Open the device under the cursor
cmd-bluetooth-connect = Connect or disconnect the device
cmd-bluetooth-pair = Pair with the device
cmd-bluetooth-forget = Forget the device
cmd-bluetooth-trust = Trust the device, or stop trusting it
cmd-bluetooth-block = Block the device, or unblock it
cmd-bluetooth-rename = Give the device another name
ask-bluetooth-rename = A name for it
ask-bluetooth-accept = The code it asks for — a PIN, or six digits
cmd-bluetooth-unnamed = Show the devices that give no name
cmd-bluetooth-up = Up one row
cmd-bluetooth-down = Down one row
group-bluetooth = bluetooth

# THE HINT LINE reads each id of the view's key table as a short text.
bluetooth-down = down
bluetooth-up = up
bluetooth-device = open
bluetooth-connect = connect
bluetooth-pair = pair
bluetooth-forget = forget
bluetooth-trust = trust
bluetooth-block = block
bluetooth-rename = rename
bluetooth-scan = scan
bluetooth-power = radio
bluetooth-discoverable = visible
bluetooth-unnamed = unnamed
bluetooth-accept = yes
bluetooth-refuse = no

bluetooth-title = Bluetooth
bluetooth-row-kind = { $name } — { $kind }
bluetooth-audio-state = { $what } — { $state }

# The modeline's device marks, on the line under the pointer.
hint-bluetooth-device = { $kind }: { $name } — click opens bluetooth
hint-bluetooth-device-battery = { $kind }: { $name }, { $percent }% left — click opens bluetooth

# THE THREE SECTIONS, and what an empty one says.
bluetooth-heading-adapter = Radio
bluetooth-heading-paired = Paired
bluetooth-heading-nearby = Nearby
bluetooth-nothing-here = nothing here
bluetooth-no-adapter = there is no Bluetooth radio on this machine
bluetooth-no-daemon = the Bluetooth daemon is not answering
bluetooth-scanning = looking for devices
bluetooth-unnamed-hidden = { $count } nearby with no name — press h to show them

# THE RADIO'S OWN ROWS.
bluetooth-adapter-power = the radio
bluetooth-adapter-discoverable = other devices can see this machine
bluetooth-adapter-pairable = other devices can pair with it
bluetooth-adapter-timeout = it stops being visible after
bluetooth-adapter-forever = it stays visible
bluetooth-adapter-pairable-forever = it stays open
bluetooth-adapter-alias = this machine is called
bluetooth-power-state-on = on
bluetooth-power-state-off = off
bluetooth-power-state-off-enabling = turning on
bluetooth-power-state-on-disabling = turning off
bluetooth-power-state-off-blocked = blocked by the hardware switch

# A DEVICE'S FACTS, one per line, in the order the detail draws them.
bluetooth-fact-address = address
bluetooth-fact-address-type = address type
bluetooth-fact-alias = called
bluetooth-fact-name = its own name
bluetooth-fact-connected = connected
bluetooth-fact-paired = paired
bluetooth-fact-bonded = the keys were kept
bluetooth-fact-trusted = trusted
bluetooth-fact-blocked = blocked
bluetooth-fact-legacy = old-style pairing
bluetooth-fact-wake = it may wake this machine
bluetooth-fact-services = services read
bluetooth-fact-battery = battery
bluetooth-fact-battery-source = the battery is reported by
bluetooth-fact-rssi = signal
bluetooth-fact-tx-power = it transmits at
bluetooth-fact-class = class
bluetooth-fact-appearance = appearance
bluetooth-fact-modalias = firmware id
bluetooth-fact-adapter = through the radio
bluetooth-fact-reconnect = who reconnects
bluetooth-fact-volume = volume it reports
bluetooth-fact-codec = codec
bluetooth-fact-audio = audio profile
bluetooth-yes = yes
bluetooth-no = no
bluetooth-rssi-value = { $dbm } dBm
bluetooth-tx-power-value = { $dbm } dBm
bluetooth-percent = { $percent }%
bluetooth-seconds = { $seconds }s

# THE KINDS, from the device's own class or appearance.
bluetooth-kind-headphones = headphones
bluetooth-kind-headset = headset
bluetooth-kind-speaker = speaker
bluetooth-kind-keyboard = keyboard
bluetooth-kind-mouse = mouse
bluetooth-kind-gamepad = gamepad
bluetooth-kind-tablet = tablet
bluetooth-kind-phone = phone
bluetooth-kind-computer = computer
bluetooth-kind-watch = watch
bluetooth-kind-camera = camera
bluetooth-kind-printer = printer
bluetooth-kind-network = network
bluetooth-kind-display = display
bluetooth-kind-unknown = device

# BLUEZ'S OWN ENUMERATIONS, in the person's language. Closed lists, not
# free-form data — a device's NAME stays the device's. `word_or` falls back to
# the daemon's word if it grows a value, so a new one is untranslated and never
# `[a key in brackets]`.
bluetooth-adapter-pairable-timeout = open to pairing for
bluetooth-fact-manufacturer = maker
bluetooth-fact-version = bluetooth version
bluetooth-fact-roles = roles
bluetooth-fact-service = offers
bluetooth-audio-profile = what it is doing
bluetooth-heading-settings = Other settings
bluetooth-setting-volume = volume
bluetooth-setting-mute = silenced
bluetooth-nothing-to-adjust = nothing to adjust here
bluetooth-settings = the other half
bluetooth-more = more
bluetooth-less = less
cmd-bluetooth-settings = Bluetooth: cross to the other half of the page
cmd-bluetooth-more = Bluetooth: more of whatever the cursor is on
cmd-bluetooth-less = Bluetooth: less of whatever the cursor is on
bluetooth-audio-off = off
bluetooth-audio-a2dp-sink = music, in stereo
bluetooth-audio-a2dp-source = music, from it
bluetooth-audio-headset-head-unit = a call, with its microphone
bluetooth-audio-headset-audio-gateway = a call, through this machine
bluetooth-codec-0 = SBC
bluetooth-codec-1 = MPEG-1/2 audio
bluetooth-codec-2 = AAC
bluetooth-codec-4 = aptX
bluetooth-codec-255 = the maker's own
bluetooth-address-type-public = fixed
bluetooth-address-type-random = random
bluetooth-reconnect-none = never
bluetooth-reconnect-host = when this machine asks
bluetooth-reconnect-device = when the device asks
bluetooth-reconnect-any = either side
bluetooth-transport-idle = idle
bluetooth-transport-pending = starting
bluetooth-transport-active = playing
bluetooth-transport-suspending = stopping
bluetooth-transport-broadcasting = broadcasting

# WHAT A DEVICE CAN DO — the profiles behind its service ids.
bluetooth-profile-headset = calls, with its microphone
bluetooth-profile-handsfree = hands-free calls
bluetooth-profile-audio-sink = plays this machine's sound
bluetooth-profile-audio-source = sends sound to this machine
bluetooth-profile-remote-control = its buttons control playback
bluetooth-profile-human-interface = sends keystrokes and pointer moves
bluetooth-profile-file-transfer = browsing its files
bluetooth-profile-object-push = receiving files sent to it
bluetooth-profile-phone-book = reading a phone's contacts
bluetooth-profile-message-access = reading a phone's messages
bluetooth-profile-network = sharing its internet connection
bluetooth-profile-battery = reporting how much charge it has
bluetooth-profile-device-information = reporting its maker and model
bluetooth-profile-serial = a serial port over the air
bluetooth-profile-sim-access = reading a phone's SIM card

# THE KEYBOARDS — what a key can be made into, and what went wrong.
keyboard-trouble-file = the file of keyboard rules could not be read or written
keyboard-trouble-no-place = this machine has no place for keyboard rules — they are written on the installed system
keyboard-trouble-refused = keyd would not take these rules, so nothing changed
keyboard-trouble-restart = the rules were written, but keyd did not restart
keyboard-trouble-mode = the keyboards could not switch, so they stay as they were
keyboard-mode = Keyboards
keyboard-mode-together = together, as one
keyboard-mode-independent = each on its own
keyboard-independent-note = Each keyboard types through its own cursor, with its own layout, keys and leader.
keyboard-unit = One keyboard
keyboard-unplugged = not plugged in
keyboard-cursor = cursor
keyboard-types-through = Types through cursor
keyboard-layout = Layout
keyboard-variant = Variant
keyboard-leader-key = Leader key
keyboard-repeat-rate = Repeat rate
keyboard-repeat-delay = Repeat delay
keyboard-machines = the machine's
keyboard-compositors = the compositor's
keys-region-taken = another keyboard has the footer open; this one waits until it closes

bluetooth-fact-kind = what it is
bluetooth-heading-profiles = The services it announces
bluetooth-about-profile = { $id } — { $said }
bluetooth-about-profile-unknown = { $id } — an id of the maker's own, with no published meaning. It is drawn as it is, which is what a search engine takes.
bluetooth-about-audio-profile = Which way the sound goes: stereo music, or a call with the microphone. PipeWire decides this, not the device.
bluetooth-about-sink-volume = How loud this machine plays through it. RET turns it up a step; the arrows move it either way.
bluetooth-about-mute = Silences it without losing the number, so turning it back gives the same volume.
bluetooth-door-this-keyboard = settings for this keyboard
bluetooth-door-all-keyboards = settings for all keyboards
bluetooth-about-this-keyboard = Opens the keyboard settings with this one selected, where a key can be made to mean another.
bluetooth-about-all-keyboards = Opens the keyboard settings, which list every keyboard this machine has.
# WHAT A ROW MEANS, for the message line the pointer asks on.
bluetooth-about-adapter = Which radio of this machine it is paired through. A machine with one Bluetooth chip has one.
bluetooth-about-address = Its hardware address — the six bytes that identify this unit, and what settings are remembered against.
bluetooth-about-address-type = A public address belongs to the maker for good; a random one is generated, and some devices change theirs to avoid being followed.
bluetooth-about-alias = The name YOU gave it. It is what every screen here shows; the device's own name is the row below.
bluetooth-about-appearance = A number the device sends to say what shape it is — 961 is a keyboard, 962 a mouse. Low-energy devices send this instead of a class.
bluetooth-about-audio = What the sound link is doing right now, as the daemon reports it.
bluetooth-about-battery = How much charge it says it has. Not every device reports one, and none of them is precise.
bluetooth-about-battery-source = Who is reporting that charge: the standard battery service, or the maker's own.
bluetooth-about-blocked = Blocked means this machine refuses it, even if it asks. It is how you keep a device from reconnecting without forgetting it.
bluetooth-about-bonded = The keys agreed when you paired are still stored, so the two can trust each other without pairing again.
bluetooth-about-class = A number a classic device sends to say what it is — headphones, a phone, a computer. Low-energy devices send an appearance instead.
bluetooth-about-codec = How the sound is compressed on its way over. SBC works everywhere; the others need both sides to agree.
bluetooth-about-connected = Whether there is a link up right now. A paired device that is switched off is paired and not connected.
bluetooth-about-kind = What this device is, worked out from what it says about itself — its class, its appearance, and the services it offers.
bluetooth-about-legacy = It pairs the old way, without the numeric comparison. Older accessories do this, and it is less secure.
bluetooth-about-modalias = The maker and model numbers it reports. This is what keyd matches a keyboard on, and what a search engine finds.
bluetooth-about-name = The name the device calls itself. What you see everywhere else is the alias above.
bluetooth-about-paired = Whether this machine and the device have exchanged keys. Pairing is what makes a connection possible later.
bluetooth-about-reconnect = Which side is allowed to start the connection — this machine, the device, or either of them.
bluetooth-about-rssi = How loudly this machine hears it, in dBm. Closer to zero is stronger; below about -90 it is on the edge.
bluetooth-about-services = Whether the daemon has finished asking the device what it can do. Until it has, the list below may be short.
bluetooth-about-trusted = A trusted device may connect without being asked each time. It is the difference between a keyboard that just works and one that asks.
bluetooth-about-tx-power = How loudly the device says it is transmitting. With the signal strength above, it is a rough idea of distance.
bluetooth-about-volume = The volume the DEVICE reports over Bluetooth. What this machine plays at is in the other column.
bluetooth-about-wake = Whether pressing a key on it may bring this machine back from sleep.

# PAIRING — the question the other side asks, and the two answers.
bluetooth-pairing-confirm = { $name } wants to pair. Does it show { $code }?
bluetooth-pairing-passkey = type { $code } on { $name }
bluetooth-pairing-pin = { $name } wants a PIN
bluetooth-pairing-passkey-here = { $name } wants six digits typed here
bluetooth-pairing-authorize = { $name } wants to pair
bluetooth-pairing-service = { $name } wants to use { $service }
bluetooth-pairing-answer = y accepts · n refuses

# WHAT WENT WRONG, said as the name of the door that shut.
echo-bluetooth-refused = { $what }: { $why }
bluetooth-what-connect = the connection
bluetooth-what-pair = the pairing
bluetooth-what-forget = forgetting the device
bluetooth-what-radio = the radio
bluetooth-what-device = the device
bluetooth-refused-authority = this session is not allowed
bluetooth-refused-absent = there is no such thing on this machine
bluetooth-refused-away = nobody answered
bluetooth-refused-busy = something else is doing it
bluetooth-refused-rejected = the other side said no
bluetooth-refused-other = it did not work

## Screens — the monitors, their pictures and their light (LEADER s d)

cmd-display-open = Screens
cmd-display-window = Screens in a window
cmd-display-down = Down one row
display-back = back
cmd-display-up = Up one row
cmd-display-less = The value under the cursor, back one
cmd-display-more = The value under the cursor, on one
cmd-display-enter = Open, copy or answer
cmd-display-automatic = Let the compositor choose this setting
cmd-display-forget = Forget what was chosen for this screen
cmd-display-arrange = Where the screens sit
group-display = Screens

display-down = down
display-up = up
display-less = less
display-more = more
display-leftwards = leftwards
display-rightwards = rightwards
display-enter = open
display-enter-answers = answer
display-enter-copies = copy
display-automatic = let it choose
display-forget = forget
display-arrange = arrange


display-title = Screens
display-heading-screens = Screens
display-heading-every = All of them
display-heading-picture = The picture
display-heading-light = The light
display-heading-session = The session
display-heading-arrangement = Where they sit
display-heading-forgetting = Forget this screen's settings?
display-heading-keeping = Keeping this?

display-nothing-here = No screen is attached.
display-reading = Asking the compositor…
display-no-mode = the compositor reports none
display-no-thread-short = the screens cannot be read here
display-no-watch = this door cannot see the file change
display-no-thread = The screens cannot be read on this machine; nothing here will change.
display-no-compositor = There is no compositor to ask, so there is nothing to show.
display-went-away = That screen is gone.

display-settings-failed = `display.json` will not parse, so nothing chosen is shown here. The file is untouched — fix or remove it.
display-one-screen-only = There is one screen, so there is nothing to arrange.

display-this-one = { $name } — this one
display-arrangement = Arrangement
display-text-size = Size of the text

# Under the cursor on the size row: what a change reaches at once, and what
# waits. It names what was MEASURED — see docs/PENDING.md — and never promises
# an application nobody watched.
display-text-reaches = the shell and its windows follow at once; GTK and Qt applications follow as they redraw
display-text-reopens = { $names } keep their size until reopened

display-fact-connector = Socket
display-fact-filed = Its settings follow
display-fact-size = Screen
display-filed-by-itself = the monitor, wherever it is plugged in
display-filed-silent = the socket — the monitor says nothing about itself
display-filed-no-serial = the socket — the monitor gives no serial, so an identical one would answer for it

display-on = Driven
display-resolution = Resolution
display-rate = Refresh rate
display-scale = Scale
display-turn = Turn
display-place = Place
display-vary = Varying refresh
display-brightness = Brightness
display-first = Opens focused here

display-vary-cannot = the hardware cannot
display-vary-always = always
display-vary-on-demand = only when a window asks
display-light-is-roots = the file belongs to root — the rule that opens it has not run here
display-light-on-its-buttons = This monitor's brightness is on its own buttons.

display-turn-and-picture = { $set } (the picture is { $picture })
display-turn-normal = normal
display-turn-rotated-90 = a quarter turn
display-turn-rotated-180 = upside down
display-turn-rotated-270 = three quarters
display-turn-flipped = mirrored
display-turn-flipped-90 = mirrored, a quarter turn
display-turn-flipped-180 = flipped vertically
display-turn-flipped-270 = mirrored, three quarters

display-value-yes = yes
display-value-no = no
display-value-off = not driven
display-value-automatic = the compositor chooses
display-value-percent = { $percent }%
display-value-rate = { $hz } Hz
display-value-inches = { $inches } inches
display-value-place = x={ $x } y={ $y }
display-value-screens = { $n ->
        [0] no screens
        [one] one screen
       *[other] { $n } screens
    }

display-left-of = to the left of { $other }
display-right-of = to the right of { $other }
display-above = above { $other }
display-below = below { $other }
display-arrange-how = The arrows move the screen under the cursor to the next edge of a neighbour — beside it, lined up with it — and Shift with ↑ or ↓ puts it above or below.

display-ask = { $what } on { $screen }.
display-ask-seconds = It goes back on its own in { $seconds }s.
display-ask-forget = Everything chosen for { $screen } goes back to what the compositor decides.
display-ask-nothing-yet = Nothing has changed yet.
display-leave-it = Leave it as it is
display-forget-it = Forget it all
display-place-and-picture = { $set } (it is at { $picture })
display-scale-and-picture = { $set } (the picture is { $picture })
display-screen-gone = { $screen } is not there any more; the change went with it.
display-screen-gone-alone = { $screen } is not there any more; nothing had been changed.
display-only-one-lit = yes — and the only screen lit
display-dark-not-ours = no — and nothing here turned it off
display-only-one-offered = the only one this monitor offers
display-put-it-back = Put it back
display-keep-it = Keep it
display-what-power-off = The screen was turned off
display-what-resolution = The resolution changed
display-what-rate = The refresh rate changed
display-what-scale = The scale changed
display-what-turn = The picture was turned

display-refused-no-compositor = there is no compositor to ask
display-refused-no-screen = that screen is not there any more
display-refused-no-light = this screen's brightness is on its own buttons
display-no-light-here = No screen's brightness can be moved from here
display-refused-not-writable = the file belongs to root, and the rule that opens it has not run on this panel
display-refused-nowhere = there is nowhere to write it down
display-refused-other = it did not work
echo-display-refused-plain = The screens refused: { $why }

# THE KEYBOARDS (`LEADER s k`) — which ones this machine has, and what each key
# on each one means. Remapping is per keyboard because keyd matches devices;
# the layout and the repeat rate are the compositor's, one for all of them.
buffer-keyboard = Keyboards
mode-keyboard = keyboards
state-keyboard-editing = one keyboard
cmd-keyboard-open = Keyboards: what each key does
cmd-keyboard-next = Keyboards: the next row
cmd-keyboard-previous = Keyboards: the row before
cmd-keyboard-list = Keyboards: back to the list
keyboard-title = Keyboards
keyboard-one = What each key does
keyboard-none = the kernel is reporting no keyboard at all
keyboard-built-in = part of this machine
keyboard-attached = plugged or paired in
keyboard-changed = changed
keyboard-untouched = nothing changed
keyboard-becomes = does
keyboard-the-key = the key
keyboard-unchanged = itself
keyboard-holds = held one way, tapped the other
keyboard-global = The layout, the compose key and the repeat rate are the compositor's — one of each, for every keyboard.
keyboard-same-model = Two keyboards of this model answer to one rule: keyd tells them apart by make and model, and these share both.
keyboard-is-the-leader = This is the leader. Moving it takes the key that opens everything, on this keyboard.
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

# ---------------------------------------------------------------------------
# LEADER s l — language, clock, date and what the system decides.
# ---------------------------------------------------------------------------
language-en-gb = English
language-pt-br = Português
locale-heading-language = Interface
locale-speaking = in use
locale-from-file = You chose this language; it is kept in language.json.
locale-from-system = This is the machine's language, from the system configuration.
locale-from-default = Nobody has chosen; this is the first declared language.
locale-heading-time = Clock and date
locale-clock-24h = 24-hour clock
locale-clock-12h = 12-hour clock
locale-clock-sample-24 = { $hh }:{ $mm }
locale-clock-sample-12 = { $hh }:{ $mm } { $half }
locale-clock-exact-24 = { $hh }:{ $mm }:{ $ss }
locale-clock-exact-12 = { $hh }:{ $mm }:{ $ss } { $half }
locale-clock-am = am
locale-clock-pm = pm
locale-order-ymd = Year first
locale-order-dmy = Day first
locale-order-mdy = Month first
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
locale-heading-machine = Decided by the system
# THE COMMAND'S OWN WORDS, drawn on the hint line: `z` asks for the zone
# THIS INTERFACE reads a timestamp in, which is not the machine's.
locale-zone = This interface's zone
locale-zone-unknown = not known
locale-zone-is-declared = The zone is declared in the system configuration; changing it is a rebuild.
locale-decimal = Decimal separator
locale-decimal-is-the-languages = The separator belongs to the language, and follows it.
locale-measurement = Measurement
locale-metric = Metric
locale-not-written = The choice could not be written to disk.
locale-measurement-is-declared = Metric whatever the language, and that is a decision rather than a default.
cmd-locale-open = Language and region, in the footer
cmd-locale-window = Language and region, in a window
cmd-locale-up = Previous row
cmd-locale-down = Next row
cmd-locale-enter = Choose
group-locale = language/region

# ---------------------------------------------------------------------------
# LEADER s g — the regions whose streets this machine has.
#
# The names are the world's regions as Geofabrik publishes them, which is data
# and never a key: `Île-de-France` is not translated, and looking for
# `[geo-region-…]` would be looking for a translation of a name somebody wrote
# down. What IS a key is the sentence around it.
# ---------------------------------------------------------------------------
geo-title = Regions
mode-geo = regions
geo-up = up
geo-down = down
geo-enter = install
geo-find = filter
geo-remove = remove
geo-installed = Installed
geo-available = Regions of the world
geo-none-installed = No region's streets are on this machine. An address stops at the nearest town.
geo-no-world = This build carries no world data, so there is no catalogue to choose from.
geo-filter = Filter
geo-filter-none = every region
geo-pack = { $size } · { $ways } ways · { $version }
geo-have = installed
geo-get = install
geo-in-parts = in parts
geo-only-in-parts = This region is published in parts. Its parts are in this list, and the filter finds them by its name.
geo-sure = Remove the streets of { $region }?
geo-no = No, keep them
geo-yes = Yes, remove them
geo-not-installed = That row is not an installed region.
geo-not-removed = The region could not be removed.
geo-not-in-catalogue = This region is not in this build's catalogue, so it cannot be fetched again.
ask-geo-find = A region's name, or part of it — empty shows them all
geo-queued = In the queue. Its size is what the server answers, and it can be given up there.
geo-no-home = There is no data directory to install into.
cmd-geo-open = Regions and their streets, in the footer
cmd-geo-window = Regions and their streets, in a window
cmd-geo-up = Previous row
cmd-geo-down = Next row
cmd-geo-enter = Install the region under the cursor, or install it again
cmd-geo-find = Narrow the list of regions
cmd-geo-remove = Remove the installed region under the cursor
group-geo = geography

# ---------------------------------------------------------------------------
# LEADER g — the glance: what time it is somewhere, and the weather and money.
# ---------------------------------------------------------------------------
world-place-europe-london = London
world-place-america-sao-paulo = Brasília
world-temperature = { $degrees } °C
world-rate = 1 { $from } = { $much } { $to }
cmd-world-glance = A glance: the time, the weather and the money
# WHAT EACH SECTION IS FOR. Every one of them opens with a sentence saying
# what it changes and WHERE that shows, because a screen of settings that only
# names them leaves the person to guess which half of the machine they reach.
locale-interface-does = What this interface speaks. It changes at once, in the footer and in every window; the machine's own language is set in the system configuration.
locale-time-does = How this interface writes every date and every time it draws — the footer's clock, the file manager's dates, the glance. It does not move the machine's clock; that is the section below.
locale-numbers-does = How numbers are written and what units the temperature comes in. They reach every screen here, including the glance line.
locale-heading-machine-clock = This machine's clock
locale-machine-does = The computer's own clock, zone and date — what every program on this machine reads, not just this interface.
locale-automatic = Kept right automatically
locale-automatic-on = on
locale-automatic-off = off
locale-automatic-does = The system asks a time server over the internet and keeps the clock in step with it. This is how the distro runs; turning it off hands the clock to you.
locale-automatic-server = Asking { $server }.
locale-automatic-synchronised = The clock is in step.
locale-automatic-adrift = It has not been able to reach a time server yet.
locale-automatic-cannot = There is no time client installed on this machine.
locale-watched = Watched by the brain
locale-watched-does = The brain's clock watch looks a few times a day and turns the automatic keeping back on if it finds it off. It is the same switch as on that task's own page.
locale-watched-unknown = The brain is not running, so this cannot be read or changed from here.
locale-machine-kept = While the clock is kept automatically these cannot be set by hand: the next check would put them back.
locale-machine-unreachable = This machine's clock could not be asked about; nothing here can be changed.
# THE MACHINE'S TWO SAY WHAT PRESSING THEM DOES. `Date` and `Time` are the
# INTERFACE's rows one section above — they choose a SHAPE and show a live
# sample of it — and two rows of one list carrying one word are two facts a
# person cannot tell apart.
locale-machine-date = Set the date
locale-machine-time = Set the time
locale-machine-zone = Time zone
locale-shows-times-in = Show times in
# THE SHAPE IS SPELT OUT and is not the person's chosen order: this is a
# value being TYPED, and a prompt that followed the display preference would
# ask for `09/21/2026` on one machine and `21/09/2026` on the next.
ask-locale-system-date = The machine's date, as 2026-09-21
ask-locale-system-time = The machine's time, as 14:32 or 14:32:05
ask-locale-system-zone = The machine's time zone, such as Europe/Lisbon
cmd-locale-system-date = Set this machine's date
cmd-locale-system-time = Set this machine's time
cmd-locale-system-zone = Set this machine's time zone
locale-not-a-date = That is not a date in that shape.
locale-not-a-time = That is not a time in that shape.
locale-clock-refused = The machine refused the change; see: journalctl -u systemd-timedated
locale-heading-glance = The glance line
locale-glance-does = The line { $keys } opens above the modeline. Each place below draws the time there, and the date, temperature and exchange rate you switch on for it.
locale-add-place-where = A zone, such as Europe/Lisbon. It starts out showing its clock and nothing else.
locale-no-places = No place is watched; { $keys } would show nothing.
locale-shows-clock = Time
locale-shows-date = Date
locale-shows-weather = Weather
locale-shows-rate = Exchange rate
locale-on = shown
locale-off = hidden
locale-heading-sources = Who is asked
locale-asked = asked
locale-not-asked = not asked
locale-sources-are-asked = Each request tells that host this machine's address and which places it watches. Nothing here needs an account or a key.
locale-source-weather = The weather
locale-source-rate = Exchange rates
# The hosts a source asks, in the order it asks them: the second is only asked
# when the first does not answer.
locale-source-hosts = Asks { $hosts }, in that order.
locale-source-answered = Last answer { $when }, from { $host }.
locale-source-never = Nothing has answered yet.
locale-source-trouble = The last attempt failed — { $said }
locale-title = Language and region
locale-up = up
locale-down = down
locale-enter = choose
locale-language = Language
locale-clock = Clock
locale-date = Date
locale-zone-is-yours = You chose this zone; the machine's own is set in the system configuration.
locale-zone-is-the-machines = The machine's own zone. Choosing one here changes what this interface shows, not what the system runs on.
locale-heading-numbers = Numbers and units
locale-decimal-language = { $mark } — the language's own
locale-decimal-point = { $mark } — a full stop, whatever the language
locale-decimal-comma = { $mark } — a comma, whatever the language
locale-units-metric = Metric — °C
locale-units-imperial = Imperial — °F
locale-drop-place = Remove this place
locale-add-place = Add a place
locale-add-place-how = a zone, such as Europe/Lisbon
ask-locale-zone = Zone for this interface's clock, such as Europe/Lisbon
ask-locale-place = Zone of the place to add, such as Europe/Lisbon
ask-locale-where = Latitude and longitude, as two numbers
locale-no-such-zone = There is no zone by that name.
locale-not-a-coordinate = That is not a latitude and a longitude.
locale-no-place-here = Put the cursor on one of the glance's places first.
locale-place-gone = That place is no longer on the glance.
cmd-locale-less = Previous value
cmd-locale-more = Next value
cmd-locale-zone = Choose the zone this interface shows times in
cmd-locale-place = Add a place to the glance
cmd-locale-where = Say where a place is
locale-less = previous
locale-more = next
# THE TWO LETTERS, and they had no short text at all: the hint line read
# `[locale-place]` and `[locale-where]` on screen, which is the missing-key
# marker doing duty as a word. `cmd-locale-place` has been there all along —
# it is the LONG form, and the line reads the short one.
locale-place = add a place
locale-where = where it is
world-temperature-f = { $degrees } °F

# =====================================================================
# SOUND — `LEADER s a`, two doors, and the marks the modeline draws
# =====================================================================
group-audio = sound
audio-title = Sound

cmd-audio-open = Sound, in the footer
cmd-audio-window = Sound, in a window
cmd-audio-up = Previous row
cmd-audio-down = Next row
cmd-audio-enter = Use this one
cmd-audio-less = Quieter, or the previous value
cmd-audio-more = Louder, or the next value
cmd-audio-mute = Silence this, or give it back
cmd-audio-default = Play through this one from now on

audio-up = up
audio-down = down
audio-enter = use
audio-less = quieter
audio-more = louder
audio-mute = silence
audio-default = make it the one

# The headings, and the order they are read in.
audio-heading-output = Playing through
audio-heading-input = Listening with
audio-heading-apps = Programs
audio-heading-card = The card
audio-heading-settings = Settings

audio-chosen-elsewhere = Chosen: { $device }, which is not here
audio-nothing-plays = There is nothing to play through.
audio-nothing-listens = There is no microphone.
audio-nothing-runs = No program is playing or recording.
audio-no-pipewire = The sound server is not answering.
audio-muted = silent
audio-in-use = in use

audio-profile = What the card is doing
audio-socket-out = Playing out of
audio-socket-in = Recording from
audio-port-none = No socket is in use.
audio-unplugged = nothing plugged in
audio-card-off = This card is switched off.

audio-listening-now = listening now

audio-setting-step = One press moves
audio-setting-over = Allow more than full
audio-setting-flash = Show the line above the modeline
audio-on = yes
audio-off = no
audio-step-points = { $points }%

audio-media-none = This build has no media player to talk to.

# The ports we have our own word for. What is not here draws in PipeWire's.
audio-port-speaker = Speakers
audio-port-headphones = Headphones
audio-port-headset = Headset
audio-port-headset-mic = Headset microphone
audio-port-internal-mic = Built-in microphone
audio-port-microphone = Microphone
audio-port-line-in = Line in
audio-port-line-out = Line out

cmd-volume-up = Louder
cmd-volume-down = Quieter
cmd-volume-mute = Silence the machine
cmd-mic-mute = Silence the microphone
cmd-media-stop = Stop playing
cmd-brightness-up = Brighter screen
cmd-brightness-down = Dimmer screen
cmd-media-play = Play, or hold
cmd-media-next = Next track
cmd-media-previous = Previous track

hint-volume = { $device } at { $percent }% — sound
hint-volume-muted = { $device }, silent — sound
hint-microphone = Something is listening — sound
sound-muted = silent

echo-audio-refused = The sound server { $why } ({ $words })
audio-refused-gone = no longer has that device
audio-refused-absent = is not answering
audio-refused-invalid = would not take that value
audio-refused-other = refused
help-media = The keyboard's own keys

# The two windows that are not wasm buffers and reach the same modeline.
# `daemon::mode_label` turns `br.ronoctua.<id>` into `mode-<id>` whatever the
# window is made of; without these the modeline drew `[mode-audio]`.
mode-audio = sound
mode-syncthing = syncthing
mode-pointers = pointers
mode-locale = language/region

# ATUIN — THE SHELL'S HISTORY, AND WHETHER IT LEAVES THIS MACHINE.
#
# `nix/modules/home.nix` refused sync in writing, and the owner asked for the
# other answer. What did not change is the command with a token in its argv,
# which is why `secrets_filter` and `history_filter` are drawn rows of this
# view rather than defaults nobody reads.
atuin-title = Shell history
atuin-heading-glance = At a glance
atuin-heading-account = The account
atuin-heading-leaving = What leaves this machine
atuin-heading-search = The search
atuin-heading-keys = The keys
atuin-heading-advanced = Rarely touched
atuin-advanced-about = These change how the search behaves. The defaults are what most people want.
atuin-absent = atuin is not installed on this machine
atuin-not-logged-in = not logged in
atuin-state = the history
atuin-on = on
atuin-off = off
atuin-syncing = syncing
atuin-not-syncing = staying on this machine
atuin-server = server
atuin-account = account
atuin-entries = entries
atuin-last-sync = last sync
atuin-mark = mark on the modeline
atuin-mark-never = never
atuin-mark-when-off = when it is not syncing
atuin-mark-when-on = while it is syncing
atuin-never = not yet
atuin-not-set = not set
atuin-sync-now = sync now
atuin-sync = sync now
atuin-log-in = log in to a server
atuin-log-out = log out and forget the key
atuin-show-key = show the encryption key
atuin-key-about = This key is what decrypts your history. The server never sees it, and a second machine cannot sync without it.
atuin-key-ask = Show the key on screen?
atuin-from-flake = set by the system's configuration
atuin-down = next
atuin-up = previous
atuin-enter = change
atuin-more = more
atuin-less = less
atuin-back = back
atuin-drawer = the drawer
cmd-atuin-open = Shell history
cmd-atuin-window = Shell history in a window
cmd-atuin-sync = Sync the shell history now
cmd-atuin-down = Next row
cmd-atuin-up = Previous row
cmd-atuin-enter = Change this setting
cmd-atuin-more = Turn this setting forwards
cmd-atuin-less = Turn this setting back
cmd-atuin-back = Leave the shell history
ask-atuin-server = The address of the sync server
ask-atuin-account = The account on that server
ask-atuin-password = The password for that account
ask-atuin-key = The key from your first machine — empty if this is the first
ask-atuin-setting = The new value
echo-atuin-synced = The history is synced
echo-atuin-refused = atuin said: { $reason }
echo-atuin-logged-out = Logged out, and the key is forgotten
group-atuin = history

# The table, from `assets/atuin-options.nix` through the generator.
atuin-opt-accept-past-line-end = let the cursor go past the end of the line
atuin-opt-auto-sync = sync by itself
atuin-opt-command-chaining = read `a && b` as two commands
atuin-opt-common-prefix = prefixes to look past
atuin-opt-common-subcommands = subcommands to keep
atuin-opt-ctrl-n-shortcuts = Ctrl+1..9 pick a result
atuin-opt-cwd-filter = directories never recorded
atuin-opt-daemon-enabled = run the background daemon
atuin-opt-daemon-sync-frequency = the daemon syncs every
atuin-opt-enter-accept = Enter runs, instead of editing
atuin-opt-exit-mode = leaving the search gives back
atuin-opt-filter-mode = search across
atuin-opt-history-filter = commands never recorded
atuin-opt-ignored-commands = commands to drop
atuin-opt-inline-height = how tall the search is
atuin-opt-invert = draw the search upside down
atuin-opt-keymap-mode = the search's key bindings
atuin-opt-local-timeout = give up on the local database after
atuin-opt-max-preview-height = the preview's height
atuin-opt-network-connect-timeout = give up connecting after
atuin-opt-network-timeout = give up on the server after
atuin-opt-prefers-reduced-motion = less movement
atuin-opt-prefix-key = the search's prefix key
atuin-opt-scroll-context-lines = lines kept around the cursor
atuin-opt-scroll-exits = scrolling past the end leaves
atuin-opt-search-mode = how a query matches
atuin-opt-search-style = how the search is drawn
atuin-opt-secrets-filter = never record a command that looks like a secret
atuin-opt-show-help = show the key hints
atuin-opt-show-preview = show the whole command
atuin-opt-show-tabs = show the tabs
atuin-opt-store-failed = record commands that failed
atuin-opt-strip-trailing-whitespace = drop trailing spaces
atuin-opt-sync-frequency = sync every
atuin-opt-sync-records = use the record store
atuin-opt-up-key-filter = the Up arrow searches across
atuin-opt-update-check = check for a new version
atuin-opt-word-chars = what counts as a word
atuin-opt-word-jump-mode = how a word jump works
atuin-opt-workspaces = treat a git repository as a workspace
atuin-unit-lines = lines
atuin-unit-seconds = seconds
atuin-word-auto = as atuin decides
atuin-word-compact = compact
atuin-word-directory = this directory
atuin-word-emacs = emacs
atuin-word-full = full
atuin-word-fulltext = anywhere in the line
atuin-word-fuzzy = fuzzy
atuin-word-global = everything
atuin-word-host = this machine
atuin-word-prefix = the start of the line
atuin-word-return-original = what you had typed
atuin-word-return-query = what you searched for
atuin-word-session = this session
atuin-word-skim = skim
atuin-word-subl = sublime
atuin-word-vim-insert = vim, insert
atuin-word-vim-normal = vim, normal
atuin-word-workspace = this project

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
about-files-parent = Leave this directory for the one that contains it.
about-files-enter = Open what is under the cursor: go into a directory, or open a file in the program that reads it.
about-files-menu = Open the list of everything that can be done to what is under the cursor, each row showing the key that runs it.
about-files-mark = Mark the row under the cursor, or take the mark off it — what is marked is what the next action acts on.
about-files-mark-all = Mark every entry of this directory at once.
about-files-unmark-all = Take the mark off every entry of this directory.
about-files-columns = Show or hide the permission bits, the owner, the group and the modification date on every row of the listing.
about-files-header = Show or hide the row that names the listing's columns and carries the arrow saying which one it is ordered by. It starts hidden.
about-files-sort = Change what the listing is ordered by: the name, the size, the date or the kind.
about-files-sort-reverse = Turn the order round, from the last to the first.
about-files-measure = Add up what a directory holds on disk, by walking it.
about-files-pane-next = Give the keyboard to the next section — the listing, the sidebar, the preview and the facts, round.
about-files-pane-previous = Give the keyboard to the section before this one.
# THE TWO COLUMNS ON THE RIGHT, and the sidebar on the left.
about-files-preview = Show or hide the preview column — the picture, the film, the text or the shape the file under the cursor holds.
about-files-facts = Show or hide the column of facts about the file under the cursor: its size, its dates, its type and what the file itself carries.
about-files-facts-scroll = Turn to the next page of the facts column, and round to the top again at the end.
about-files-sidebar = Show or hide the column of places — your ten directories, and the disks that are plugged in.
about-files-eject = Unmount the removable disk this row stands for, so it can be taken out safely.
about-files-unpin = Take this place off the sidebar. One the distribution ships comes back hidden; one of your own is gone.
# THE PREVIEW: the text, the film and the shape.
about-files-preview-scroll = Turn to the next page of the preview, and round to the top again at the end.
about-files-preview-left = Slide the previewed text sideways, towards the beginning of its long lines.
about-files-preview-right = Slide the previewed text sideways, towards the end of its long lines.
about-files-theatre = Give the preview the whole region, and give the region back. With nothing selected there is no preview to expand.
about-files-facts-wide = Give the facts the whole region, and give the region back. Expanded they are laid out in as many columns as the width holds, so each value is shown more of.
about-files-grid-next = Move to the next file along the shelf. With the entries side by side the sideways arrows walk them; TAB still crosses to the next section.
about-files-grid-previous = Move to the file before this one along the shelf. With the entries side by side the sideways arrows walk them; Shift TAB still crosses back a section.
about-files-grid = Show the files side by side as pictures instead of one to a line. The glyph at the end of the path's line does the same, and draws which shape it would go to. It starts off.
about-files-grid-size = Ring through the three sizes a thumbnail is drawn at. On a screen that would hold more pictures than the manager draws at once, the smallest step is the one that is skipped.
about-files-play = Start what the preview is showing, or hold it where it is.
about-files-rewind = Jump backwards in what is playing.
about-files-forward = Jump forwards in what is playing.
about-files-sound = Give the film its sound, or take it away. A film with no soundtrack has none to give.
about-files-playlist-previous = Show the entry before this one in the playlist.
about-files-playlist-next = Show the next entry of this playlist.
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
# THE TRANSFER QUEUE.
about-transfers-up = Move the cursor one row up the queue.
about-transfers-down = Move the cursor one row down the queue.
about-transfers-pause = Hold this transfer where it is, or let it carry on.
about-transfers-cancel = Ask whether to give this transfer up: what has been written so far would be deleted.
about-transfers-detail = Open this transfer one field a line: what moves it, who asked, the whole failure. Pressed there, back to the list.
about-transfers-answer = Take the answer the cursor is on.
about-transfers-forget = Take this row off the list. The file it wrote stays where it is.
about-transfers-sweep = Take every finished row off the list at once.
about-transfers-launch = Open the program this row installed, from the entry it put in the launcher.
# THE NOTIFICATION MANAGER, where one row means something else.
about-notif-edit = Open this filter of your own to be rewritten: its name, its conditions and what it does.

# =====================================================================
# THE LINE THAT OUTLIVES THE FOOTER — when the interface itself falls
# =====================================================================
#
# The owner moved the pointer across the hint line and everything closed. He
# named two defects and only one of them was the crash: *"o outro é o erro de
# que a ronoctua-distro não sabe lidar com isso, ou seja, acaba deixando o
# usuário sem interface"*. These are the words of the answer to the second one.
#
# It is a second layer-shell row raised by systemd's `OnFailure=`, and it is not
# a second chrome (invariant 14): it is the footer's own row drawn by another
# process, alive exactly while there is no chrome at all.
lifeline-fell = The interface closed on its own.
lifeline-fell-and-came-back = The interface closed on its own, and came back.
# THE UNIT'S OWN WORDS, and they are DATA — what the machine said, the way a
# filename is. `{ $why }` is systemd's `Result=`: `core-dump`, `signal`,
# `oom-kill`, `exit-code`.
lifeline-why = { $unit } · { $why }
# AND THE UNIT ALONE, when systemd has nothing to say about it — a
# transient unit is gone by the time the line asks. A dot between one
# thing and nothing separates nothing.
lifeline-unit = { $unit }
lifeline-logs = see the logs
lifeline-restart = restart the interface
lifeline-ai = ask an AI
lifeline-close = close
lifeline-logs-opened = the journal is open in a terminal
lifeline-restarting = systemd was asked to start it again
lifeline-would-not-start = systemd would not start it; the logs say why
lifeline-no-journalctl = the journal would not open; journalctl is not on this machine
# THE NESTED LOOP HAS NO UNIT. `nix/footer-single.nix` starts the footer as an
# ordinary background child, so there is nothing for systemd to restart — and
# saying so beats answering about a unit it has never heard of.
lifeline-not-a-unit = there is no systemd unit here; the nested loop starts it by hand

# =====================================================================
# POINTERS — `LEADER s i`: mice, touchpads and every cursor
# =====================================================================
group-pointers = pointers
pointers-title = Mice and touchpads

cmd-pointers-open = Mice and touchpads, in the footer
cmd-pointers-window = Mice and touchpads, in a window
cmd-pointers-up = Previous row
cmd-pointers-down = Next row
cmd-pointers-enter = Change this, or use it
cmd-pointers-less = The previous value
cmd-pointers-more = The next value
cmd-pointers-forget = Forget this device

pointers-up = up
pointers-down = down
pointers-enter = change
pointers-less = previous
pointers-more = next
pointers-forget = forget

pointers-heading-cursors = Cursors
pointers-heading-mice = Mice
pointers-heading-touchpads = Touchpads
pointers-heading-general = Every cursor
pointers-heading-away = Not connected

pointers-about-cursors = Every mouse and every touchpad can move a cursor of its own, each in its own colour, each focusing its own window. Put two devices on one cursor to have them move it together.
pointers-about-mice = What is set under “Every mouse” applies to each mouse that does not set it for itself.
pointers-about-touchpads = What is set under “Every touchpad” applies to each touchpad that does not set it for itself.
pointers-about-general = Focus follows the pointer for every cursor at once: the compositor has one setting for all of them.
pointers-about-away = Devices seen before, kept with their cursor and their settings for when they come back.
pointers-about-twins = Two devices of the same model have the same name to the compositor, so they always drive one cursor together.

pointers-detect = A new device gets its own cursor
pointers-together = Every mouse and touchpad moves one cursor
pointers-own-cursor = a cursor of its own
pointers-emulated = The virtual machine's own pointers move
pointers-about-emulated = The virtual machine's tablet and mice carry the host's pointer in. With the host's own mice handed to this machine they are off, so no cursor stands still; turn them on to take the pointer out of the window again.
pointers-not-drawn = not drawn: nothing on it moves
pointers-cursor = Cursor { $number }
pointers-cursor-on = This cursor is on
pointers-colour = Colour
pointers-colour-value = colour { $number }
pointers-on-cursor = Drives
pointers-on-cursor-value = cursor { $number }
pointers-new-cursor = a cursor of its own
pointers-every-mouse = Every mouse
pointers-every-touchpad = Every touchpad
pointers-device-on = This device is on
pointers-no-devices = No mouse or touchpad is connected.
pointers-no-compositor = The compositor is not answering, so no device can be read or set.
pointers-settings-failed = pointers.json could not be read, and is left as it is: the choices shown are the defaults until it is fixed or removed.

pointers-setting-events = Off while a mouse is connected
pointers-setting-speed = Speed
pointers-setting-profile = Acceleration
pointers-setting-natural = Natural scrolling
pointers-setting-left = Left-handed
pointers-setting-tap = Tap to click
pointers-setting-tap-map = Two and three fingers tap
pointers-setting-tap-drag = Tap and drag
pointers-setting-drag-lock = Drag survives lifting
pointers-setting-finger-drag = Drag with fingers
pointers-setting-click = Pressing clicks with
pointers-setting-click-map = Two and three fingers press
pointers-setting-middle = Left and right together are the middle button
pointers-setting-scroll = Scrolls with
pointers-setting-scroll-button = Scroll button
pointers-setting-scroll-lock = Scroll button stays pressed
pointers-setting-dwt = Off while typing
pointers-setting-dwtp = Off while the trackpoint is used
pointers-setting-factor = Scroll distance

pointers-size = Size
pointers-size-value = { $pixels } px
pointers-hide-after = Hide when still for
pointers-never = never
pointers-seconds = { $seconds } s
pointers-hide-typing = Hide while typing
pointers-focus = Focus follows the pointer
pointers-focus-no = no, a click focuses
pointers-focus-yes = yes
pointers-focus-always = always
pointers-about-typing = The coloured border marks the window the keyboards type into, and only cursor 1's click moves it. Choose “the last cursor to pick a window” and every cursor's click moves it — and the typing with it: the compositor draws the border for the keyboards' focus alone.
pointers-typing = Typing goes to
pointers-typing-first = cursor 1's window
pointers-typing-cursor = cursor { $number }'s window
pointers-typing-latest = the last cursor to pick a window
pointers-heading-keyboards = Keyboards
pointers-about-keyboards = In Independent mode every keyboard types through a cursor of its own, with its own layout and leader key. That is set on the keyboards' screen.
pointers-open-keyboards = Open the keyboard settings

pointers-on = on
pointers-off = off
pointers-flat = steady
pointers-adaptive = faster with a faster hand
pointers-lrm = right, then middle
pointers-lmr = middle, then right
pointers-lock-off = no
pointers-lock-on = briefly
pointers-lock-sticky = until the next tap
pointers-fingers-off = off
pointers-fingers-three = three fingers
pointers-fingers-four = four fingers
pointers-click-none = no buttons
pointers-click-areas = where it is pressed
pointers-click-fingers = how many fingers press
pointers-scroll-none = nothing
pointers-scroll-two = two fingers
pointers-scroll-edge = the edge
pointers-scroll-button = a held button
pointers-button-none = none
pointers-button-middle = the middle button
pointers-button-side = the side button
pointers-button-extra = the extra button
pointers-factor-value = × { $factor }
pointers-speed-value = { $speed }
pointers-is-first = the cursor the keyboards are on

# =====================================================================
# LEADER d — development: tools for whoever builds this distro.
# =====================================================================
# The screen resizer (`LEADER / r`, `resizer.rs`): two curtains cut the
# screen, and everything is laid out in the room they leave. Its line on the
# bottom curtain says the room in pixels and in the grid, and each key's word
# is short because the line is one line; `about-` is what the pointer reads.
group-dev = development
cmd-dev-resizer = Resize the whole screen
hint-resizer = The screen is cut to { $width } × { $height } px — { $columns } columns by { $lines } lines. Click to have it whole again.
echo-resizer-on = The screen can be resized: drag its right edge, its bottom edge or the corner, or click a curtain and use the keys its line teaches.
echo-resizer-off = The screen is whole again.
echo-resizer-failed = The screen could not be resized: { $why }
resizer-size = { $width } × { $height } px
resizer-grid = { $columns } columns × { $lines } lines
resizer-off = whole screen
about-resizer-off = Ends the resizing: the curtains go and the screen is whole again.
resizer-narrower = narrower
about-resizer-narrower = One column narrower, or a stride of them with Shift held.
resizer-wider = wider
about-resizer-wider = One column wider, or a stride of them with Shift held.
resizer-shorter = shorter
about-resizer-shorter = One line shorter, or a stride of them with Shift held.
resizer-taller = taller
about-resizer-taller = One line taller, or a stride of them with Shift held.
resizer-largest = largest
about-resizer-largest = As large as the curtains allow — everything but the two strips they keep to be grabbed.

# =====================================================================
# THE CAMERA STUDIO — `LEADER m c` (the app) and `LEADER s c` (settings)
# =====================================================================
group-camera = camera
mode-camera = camera
camera-title = Camera
camera-settings-title = Camera settings

cmd-camera-open = The camera, in a window
cmd-camera-settings-open = Camera settings, in the footer
cmd-camera-settings-window = Camera settings, in a window
cmd-camera-settings-up = Previous row
cmd-camera-settings-down = Next row
cmd-camera-settings-less = The previous value
cmd-camera-settings-more = The next value
cmd-camera-settings-enter = Change this, or use it
cmd-camera-settings-remove = Remove this scene or script
cmd-camera-photo = Take a photo of what the camera shows
cmd-camera-record = Start or stop recording
cmd-camera-effect-next = The next effect on the camera
cmd-camera-effect-previous = The previous effect on the camera
cmd-camera-background-next = The next background: blurred, replaced, removed or left alone
cmd-camera-param-less = Less of the effect
cmd-camera-param-more = More of the effect
cmd-camera-camera-next = Show the next camera
cmd-camera-mirror = Show the camera as a mirror, or not
cmd-camera-scene-next = Put the next scene on air
cmd-camera-scene-previous = Put the previous scene on air
cmd-camera-script = Run the first script, or stop the one running
cmd-camera-next-cue = The running script's next scene
cmd-camera-virtual = Offer the picture to other programs as a camera, or stop
cmd-camera-stream = Start or stop the live stream
cmd-camera-scene-add = A new scene, a copy of the one on air
cmd-camera-scene-rename = Rename the scene on air
cmd-camera-script-add = A new script of scenes
cmd-camera-stream-url = The address a stream is sent to
cmd-camera-stream-key = The stream's key
ask-camera-scene-add = Name of the new scene
ask-camera-scene-rename = New name of the scene on air
ask-camera-script-add = Script — name: scene seconds, scene seconds, … (add “loop” to repeat)
ask-camera-stream-url = Stream address (rtmp://… or srt://…)
ask-camera-stream-key = Stream key

camera-settings-up = up
camera-settings-down = down
camera-settings-less = previous
camera-settings-more = next
camera-settings-enter = change
camera-settings-remove = remove
camera-photo = photo
camera-record = record
camera-effect-next = effect
camera-effect-previous = previous effect
camera-background-next = background
camera-param-less = less
camera-param-more = more
camera-camera-next = camera
camera-mirror = mirror
camera-scene-next = scene
camera-scene-previous = previous scene
camera-script = script
camera-next-cue = next cue
camera-virtual = virtual camera
camera-stream = stream
camera-menu-effect = pick effect
camera-menu-background = pick background
camera-menu-camera = pick camera
camera-menu-scene = pick scene
camera-mixer = sound
camera-fit = fit or fill
camera-pin = pin
about-camera-menu-effect = Open the list of effects, to pick one directly.
about-camera-menu-background = Open the list of backgrounds, to pick one directly.
about-camera-menu-camera = Open the list of cameras, to pick one directly.
about-camera-menu-scene = Open the list of scenes, to pick one directly.
camera-button-menu = click: pick from the list · right click: the next one
about-camera-mixer = Open or close the sound: the microphone's and the machine's levels and volumes.
about-camera-fit = Show the whole picture with bars, or fill the window and crop the picture.
about-camera-pin = Keep the controls over the picture, or let them go when nobody uses them.

camera-effect-none = none
camera-effect-grey = Grey
camera-effect-sepia = Sepia
camera-effect-negative = Negative
camera-effect-posterize = Posterize
camera-effect-thermal = Thermal
camera-effect-night = Night vision
camera-effect-tone = Tone
camera-effect-pixelate = Pixelate
camera-effect-blur = Blur
camera-effect-sketch = Sketch
camera-effect-vignette = Vignette
camera-effect-symmetry = Symmetry
camera-effect-kaleidoscope = Kaleidoscope
camera-effect-chroma-key = Green screen
camera-effect-background-blur = Blurred background
camera-effect-background-replace = Replaced background
camera-effect-background-remove = No background
camera-param-amount = Amount
camera-param-levels = Levels
camera-param-gain = Gain
camera-param-brightness = Brightness
camera-param-contrast = Contrast
camera-param-saturation = Saturation
camera-param-temperature = Warmth
camera-param-size = Block size
camera-param-radius = Radius
camera-param-strength = Strength
camera-param-segments = Segments
camera-param-hue = Colour of the screen
camera-param-tolerance = Tolerance
camera-param-steady = Steadiness of the edge

studio-scene-camera = Camera
studio-scene-screen = Screen
studio-scene-screen-and-camera = Screen and camera

studio-trouble-no-camera = No camera is connected.
studio-trouble-camera-busy = The camera is being used by { $program }.
studio-trouble-camera-silent = The camera stopped sending pictures; it will be tried again.
studio-trouble-no-model = The model that finds the person is not installed, so the background cannot be blurred, replaced or removed.
studio-trouble-no-hand-model = The models that find hands are not installed, so gestures cannot be read.
studio-trouble-no-backdrop = No picture is chosen for the replaced background (Camera settings, Effects).
studio-trouble-no-ffmpeg = ffmpeg could not be run, so no camera can be opened.
studio-trouble-no-screen = The screen cannot be captured here.
studio-trouble-no-window = No open window matches this scene's window.
studio-trouble-no-virtual = The virtual camera device does not exist: the kernel module is not loaded.
studio-trouble-virtual-busy = Another program is writing to the virtual camera.
studio-trouble-encoder = The recording could not go on: { $why }
studio-trouble-stream = The stream stopped: { $why }
studio-trouble-no-stream-target = Set the stream's address and key first (Camera settings, Streaming).
studio-trouble-name = That name is already used, or it is the last scene.
studio-trouble-write = A file could not be written: { $why }
studio-not-running = The camera studio is not answering.
studio-photo-saved = Photo saved: { $file }
studio-recording-saved = Recording saved: { $file }
studio-countdown = { $seconds }
studio-recording-for = Recording { $time }
studio-streaming-for = Live { $time }
studio-level = { $percent }%
studio-muted = muted
studio-sound-microphone = Microphone
studio-sound-desktop = The machine's sound

camera-heading-cameras = Cameras
camera-about-cameras = Every camera this machine sees, with the modes and the controls its driver declares. What is changed here is kept for that camera and put back each time it is plugged in.
camera-default = The camera the app opens
camera-default-first = the first colour camera
camera-infrared = infrared
camera-cannot-stream = cannot be opened
camera-about-cannot-stream = This camera's driver only offers multi-planar capture, which needs libcamera. Cameras on some laptops arrive this way; a relay such as v4l2-relayd turns them into a camera every program opens.
camera-mode = Mode
camera-mode-chosen = chosen for it
camera-mode-chosen-is = chosen for it: { $mode }
camera-mirrored = Shown as a mirror
camera-shuttered = The lens is covered by the camera's own shutter.
camera-factory = Put every control back as it came

camera-heading-effects = Effects
camera-about-effects = What is done to the camera's picture, live — in the app, and in photos and recordings. The virtual camera takes this effect, none, or one of its own: see its section.
camera-effect = Effect
camera-background = Background
camera-backdrop = Picture behind you

camera-heading-virtual = Virtual camera
camera-about-virtual = Other programs — a browser call, a meeting — can choose the “ronoctua” camera and get this picture, through the effect chosen for it below. It only sends while it is on.
camera-virtual-now = On now
camera-virtual-at-start = On at every start
camera-virtual-sends = It sends
camera-sends-program = the scene on air
camera-sends-camera = the camera alone

camera-heading-capture = Photos and recordings
camera-about-capture = A recording is written as it goes and survives a crash; with MP4 chosen it is turned into MP4 when it ends.
camera-size = Size
camera-rate = Frames per second
camera-fps = { $fps } per second
camera-container = Recordings are saved as
camera-container-mp4 = MP4
camera-container-mkv = Matroska
camera-encoder = Encoded by
camera-encoder-auto = the graphics card, or the processor
camera-encoder-hardware = the graphics card
camera-encoder-software = the processor
camera-quality = Quality (lower is better)
camera-countdown = Countdown before a photo
camera-at-once = none
camera-seconds = { $seconds } s
camera-cursor = The pointer in screen captures
camera-pictures = Photos go to
camera-videos = Recordings go to

camera-heading-sound = Sound
camera-about-sound = The microphone is opened only while something records or streams, and the modeline's microphone mark says so.
camera-microphone = Record the microphone
camera-microphone-gain = Microphone volume
camera-desktop = Record the machine's sound
camera-desktop-gain = Machine's volume

camera-heading-scenes = Scenes
camera-about-scenes = A scene is what is on air: the camera, the screen, a window, a picture, arranged in layers. Drag a small layer in the app to move it and turn the wheel over it to resize it — during a recording too.
camera-no-studio = The camera studio is not answering; scenes and scripts appear when it does.
camera-on-air = on air
camera-scene-add = New scene
camera-scene-rename = Rename the scene on air

camera-heading-scripts = Scripts
camera-about-scripts = A script puts scenes on air in order, each for so many seconds or until “next cue”.
camera-script-bad = A script is written as “name: scene seconds, scene seconds, …”, with scenes that exist.
camera-running = running
camera-script-add = New script

camera-heading-stream = Streaming
camera-about-stream = A live stream to any service that takes RTMP or SRT. The key is kept in a file only you can read, and never appears on screen or in a list of processes.
camera-stream-url = Address
camera-stream-key = Key
camera-stream-now = Live now
camera-not-set = not set
camera-set = set

camera-heading-privacy = Privacy
camera-about-privacy = The modeline shows a camera whenever any program has a camera open — ours included.
camera-mark = The camera mark
camera-mark-when-in-use = whenever a camera is open
camera-mark-never = never

camera-heading-defaults = Defaults
camera-about-defaults = Puts every setting on this screen back as it came, and turns the effect and the background off. Your scenes, the stream's address and its key stay.
camera-defaults = Put everything back
camera-defaults-confirm = Once more to confirm
camera-defaults-done = Every setting is back as it came.
camera-heading-app = The camera app
camera-open-app = Open the camera
hint-camera-in-use = A camera is open by { $programs }. Click for the camera settings.
hint-gestures = Hand gestures are on, and the camera reads your hands. Click to turn them off.
compositor-mode-split = split
compositor-mode-jump = jump
compositor-mode-filter = filter
compositor-mode-placement = placement
compositor-mode-width = width
compositor-mode-height = height
compositor-mode-resize = resize
compositor-mode-floating = floating
compositor-mode-toggle-width = toggle width
compositor-mode-toggle-height = toggle height
compositor-mode-align = align
compositor-mode-fit = fit
compositor-mode-trailmark = mark
compositor-mode-trail = trail
compositor-mode-spaces = layouts
hint-compositor-mode = The compositor is in a key mode: the keys mean what the line below says. Click to leave it.
hint-recording = The studio is recording or streaming. Click for the camera settings.

# =====================================================================
# THE PLAYER — `LEADER p`, the column on the right, the modeline's mark
# =====================================================================
group-player = player
mode-player = player
mode-software = software
mode-camera-settings = camera settings
mode-manual = manual
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

player-up = up
player-down = down
player-less = less
player-more = more
player-enter = choose
player-toggle = play/pause
player-stop = stop
player-next = next
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

ask-player-play-file = Play now (a path or an address)
ask-player-enqueue = Add to the queue (a path or an address)
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
# What joins the sources of a set nobody named, in the queue and in a
# hint: in quotes, so the spaces around it stay.
player-set-joiner = {" + "}
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
tongue-pt-pt = Portuguese (Portugal)
tongue-pt = Portuguese
tongue-es = Spanish
tongue-fr = French
tongue-de = German
tongue-it = Italian
tongue-nl = Dutch
tongue-sv = Swedish
tongue-no = Norwegian
tongue-da = Danish
tongue-fi = Finnish
tongue-is = Icelandic
tongue-pl = Polish
tongue-cs = Czech
tongue-sk = Slovak
tongue-sl = Slovenian
tongue-hr = Croatian
tongue-sr = Serbian
tongue-bs = Bosnian
tongue-mk = Macedonian
tongue-bg = Bulgarian
tongue-ro = Romanian
tongue-hu = Hungarian
tongue-el = Greek
tongue-tr = Turkish
tongue-ru = Russian
tongue-uk = Ukrainian
tongue-be = Belarusian
tongue-lt = Lithuanian
tongue-lv = Latvian
tongue-et = Estonian
tongue-ka = Georgian
tongue-hy = Armenian
tongue-az = Azerbaijani
tongue-sq = Albanian
tongue-ca = Catalan
tongue-eu = Basque
tongue-gl = Galician
tongue-eo = Esperanto
tongue-ar = Arabic
tongue-he = Hebrew
tongue-fa = Persian
tongue-ku = Kurdish
tongue-ur = Urdu
tongue-hi = Hindi
tongue-bn = Bengali
tongue-ta = Tamil
tongue-te = Telugu
tongue-ml = Malayalam
tongue-si = Sinhala
tongue-th = Thai
tongue-vi = Vietnamese
tongue-id = Indonesian
tongue-ms = Malay
tongue-tl = Tagalog
tongue-my = Burmese
tongue-km = Khmer
tongue-zh-cn = Chinese (simplified)
tongue-zh-tw = Chinese (traditional)
tongue-zh = Chinese
tongue-ja = Japanese
tongue-ko = Korean
tongue-sw = Swahili
tongue-af = Afrikaans
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
hint-player = { $what }. A click plays or pauses; the right button opens the column, or closes it.
hint-player-next = Next: { $what }. The right button opens the player here.
hint-player-next-shuffled = Next, in the shuffled order: { $what }. The right button opens the player here.
hint-player-next-end = This is the last item in the queue: next stops the player. The right button opens the player here.
# The modeline's pair while only the ambient sounds are on (`player_mark`).
hint-player-ambience = Hold the ambient sounds, or let them go on
hint-player-next-ambience = Only the ambient sounds are on: the queue has nothing next
hint-workspace = Workspace { $number } — click goes there, the right button shows every workspace at once
hint-workspace-current = Workspace { $number }, the one in front — click fits all of it on the screen, or puts it back; the right button shows every workspace at once
overview-back = back to the normal view
overview-mouse = a click on a window goes into it
cmd-onscreen-keyboard = On-screen keyboard
cmd-gestures = Hand gestures
cmd-gestures-calibrate = Calibrate hand gestures
osk-key-esc = Esc
osk-key-ctrl = Ctrl
osk-key-alt = Alt
osk-key-altgr = AltGr
osk-key-super = Super
osk-key-fn = Fn
osk-key-home = Home
osk-key-end = End
osk-key-page-up = PgUp
osk-key-page-down = PgDn
osk-key-insert = Ins
osk-key-delete = Del
osk-key-function = F{ $number }
osk-key-leader = Leader
osk-setting-languages = Languages
osk-setting-height = Key height
osk-setting-width = Width
osk-width-small = Small
osk-width-medium = Medium
osk-width-whole = Whole screen
about-osk-language = Switch to the next language: { $languages }
about-osk-settings = The keyboard's own settings; press again to go back to the keys
about-osk-hide = Put the keyboard away
about-osk-leader = The leader, as its key would open it
about-osk-fn = Function keys, Insert and Delete, and Home, End and the pages on the arrows
about-osk-modifier = Tap: held for the next key. Tap twice: kept on until tapped again
about-osk-choice = { $name } — tap to offer this language on the keyboard, or to stop offering it
about-osk-height = How tall each key is
about-osk-width = How wide the keyboard is
echo-osk-last-language = One language always stays on.
echo-osk-types-nothing = The compositor offers no virtual keyboard, so these keys reach no window.
echo-gestures-on = Gestures are on: the camera reads your hands.
echo-gestures-off = Gestures are off, and the camera is let go.
echo-gestures-move-nothing = Gestures are on, but the compositor offers no virtual pointer, so the pointer will not move.
echo-gestures-calibrate-top-left = Point at the screen's top left corner, and open the other hand.
echo-gestures-calibrate-top-right = Now the top right corner, and open the other hand.
echo-gestures-calibrate-bottom-right = Now the bottom right corner, and open the other hand.
echo-gestures-calibrate-bottom-left = Last, the bottom left corner, and open the other hand.
echo-gestures-calibrated = Calibrated: the pointer now follows the corners you showed.
echo-gestures-calibration-failed = The corners were too close together; the pointer keeps the area it had.
echo-gesture-escape = Gesture: Esc.
echo-gesture-arrow = Gesture: an arrow key.
echo-gesture-focus = Gesture: focus moved.
echo-gesture-workspace = Gesture: workspace changed.
echo-gesture-overview = Gesture: every workspace.
echo-gesture-keyboard = Gesture: the on-screen keyboard.
echo-gesture-player = Gesture: the player's column.
echo-gesture-close = Gesture: the window was closed.
osk-height-low = Low
osk-height-medium = Medium
osk-height-tall = Tall

## The way back, first on every view's line: Esc, and a click on it
view-back = back
about-view-back = Goes back one level, and from the first one closes this view — the same as pressing Esc.

## A question on a window's header line — what is typed there
line-cancel = cancel
about-line-cancel = Lets the question go without answering it — the same as pressing Esc.
line-answer = answer
about-line-answer = Answers with what is typed on the line — the same as pressing Enter.
door-paste-failed = What was copied could not be read.

## The window menu — the modeline's app name, clicked (LEADER w o)
cmd-window-menu = Everything that can be done to the focused window, as a grid of controls
window-menu-none = No window has the focus.
window-menu-fullscreen = Fullscreen
window-menu-unfullscreen = Leave fullscreen
window-menu-float = Float
window-menu-tile = Back into the layout
window-menu-stick = Stick to every workspace
window-menu-unstick = Unstick
window-menu-maximise = Maximise
window-menu-centre = Centre
window-menu-put-away = Put in the drawer
window-menu-resize = Resize with the keys
window-menu-send-to = send to workspace
window-menu-move = move
window-menu-size = size
window-menu-fullscreen-kinds = kinds of fullscreen
window-menu-close = Close
window-menu-close-others = Close the others
window-menu-kill = Force quit
about-window-menu-kill = Kills the window's program at once. Whatever it had not saved is lost.
window-menu-workspace = Workspace { $n }
window-menu-workspace-empty = Workspace { $n }, empty
window-menu-next = The next one
window-menu-previous = The previous one
window-menu-left = Left
window-menu-right = Right
window-menu-up = Up
window-menu-down = Down
window-menu-out-left = Out of the column, left
window-menu-out-right = Out of the column, right
window-menu-out-up = Out of the column, up
window-menu-out-down = Out of the column, down
window-menu-beginning = To the beginning
window-menu-end = To the end
window-menu-pin-beginning = Pin at the beginning
window-menu-pin-end = Pin at the end
window-menu-screen-left = To the screen on the left
window-menu-screen-right = To the screen on the right
window-menu-width-next = Next width
window-menu-width-previous = Previous width
window-menu-height-next = Next height
window-menu-height-previous = Previous height
window-menu-toggle-size = Full size, or back
window-menu-reset-size = Reset the size
window-menu-full-layout = Fill the area above the footer, or leave it
window-menu-full-application = Fullscreen for the program only, or leave it
window-menu-full-global = Cover every screen, or leave them
window-menu-align = align
window-menu-align-auto = Let the layout place it
window-menu-align-bottom = Bottom
window-menu-align-left = Left edge
window-menu-align-middle = Middle of the column
window-menu-align-right = Right edge
window-menu-align-top = Top
window-menu-awake-focus = Awake while focused
window-menu-awake-fullscreen = Awake while fullscreen
window-menu-awake-none = Let the screen sleep
window-menu-awake-open = Awake while open
window-menu-awake-visible = Awake while visible
window-menu-behaviour = behaviour
window-menu-border-csd = Its own decorations
window-menu-border-none = No border
window-menu-border-normal = Border and title
window-menu-border-pixel = Thin border
window-menu-calm = Stop asking for attention
window-menu-content = zoom the content
window-menu-content-bigger = Bigger
window-menu-content-reset = Its own size
window-menu-content-smaller = Smaller
window-menu-dim-off = Never dim
window-menu-dim-on = Dim when unfocused
window-menu-float-keys = Move with the keys
window-menu-grow-down = Push the bottom out
window-menu-grow-left = Push the left out
window-menu-grow-right = Push the right out
window-menu-grow-up = Push the top out
window-menu-height = height
window-menu-keep-shortcuts = Let it take the shortcuts
window-menu-look = appearance
window-menu-narrower = Narrower
window-menu-nudge-down = Nudge down
window-menu-nudge-left = Nudge left
window-menu-nudge-right = Nudge right
window-menu-nudge-up = Nudge up
window-menu-opacity-full = Fully opaque
window-menu-opacity-less = More transparent
window-menu-opacity-more = More opaque
window-menu-out-beginning = Out, to the beginning
window-menu-out-end = Out, to the end
window-menu-select = Select
window-menu-shadow-off = No shadow
window-menu-shadow-on = Shadow
window-menu-share = { $percent }%
window-menu-shorter = Shorter
window-menu-shrink-down = Pull the bottom in
window-menu-shrink-left = Pull the left in
window-menu-shrink-right = Pull the right in
window-menu-shrink-up = Pull the top in
window-menu-take-to = take to workspace
window-menu-taller = Taller
window-menu-tearing-off = No tearing
window-menu-tearing-on = Allow tearing
window-menu-wider = Wider
window-menu-width = width
cmd-wm-float-off = Put this window back in the layout — and out of the drawer, if it came from there
cmd-drawer-open = What the drawer holds, to take a window out of it
window-menu-drawer = drawer
window-menu-put-back = Put back in the drawer
window-menu-take-out = Out of the drawer for good
window-menu-drawer-one = { $app } — { $title }
window-menu-drawer-next = The next one out
window-menu-drawer-jump = Pick one by its number
about-window-menu-take-out-this = Takes this window out of the drawer, over the workspace in front.
hint-drawer = { $count ->
        [one] One window in the drawer. Click to see it; right click takes it out.
       *[other] { $count } windows in the drawer. Click to see them; right click takes the next one out.
    }
window-menu-drawer-empty = The drawer is empty.
cmd-trail-open = The trails: this window on one, walking them, making and deleting them
cmd-selection-open = The selected windows, and what can be done to them together
window-menu-selection = selection
window-menu-trail = trail
window-menu-trail-on = Put on the trail
window-menu-trail-off = Take off the trail
window-menu-trail-next-window = Next window on it
window-menu-trail-previous-window = Previous window on it
window-menu-trail-to-selection = Select its windows
window-menu-trail-new = New trail
window-menu-trail-number = Trail { $n }
window-menu-trail-in-use = Trail { $n }, in use
window-menu-trail-next = Next trail
window-menu-trail-previous = Previous trail
window-menu-trail-clear = Empty it
window-menu-trail-delete = Delete it
window-menu-yes-delete-trail = Yes, delete this trail
window-menu-unselect = Unselect
window-menu-select-workspace = Select the whole workspace
window-menu-select-move = Bring them here
window-menu-select-to-trail = Turn them into a trail
window-menu-select-none = Clear the selection
hint-selection-uncounted = Windows selected, more than this footer could count. Click for what to do with them.
hint-trail-current = Trail { $number }, the one in use.
hint-trail-number = Trail { $number }. Click to put it in use.
hint-selection = { $count ->
        [one] One window selected. Click for what to do with it.
       *[other] { $count } windows selected. Click for what to do with them.
    }
hint-trail = { $trails ->
        [one] One trail
       *[other] { $trails } trails
    }, { $windows ->
        [0] no window on the one in use
        [one] one window on the one in use
       *[other] { $windows } windows on the one in use
    }. Click for the trails.
cmd-wm-float-centre = Move this floating window to the middle of the room windows have
window-menu-drawer-out = { $name } — out
about-window-menu-put-back-this = Puts this window, which the drawer let out, back in it.
notif-trail-drawer-summary = { $window } left the trail
notif-trail-drawer-body = Window { $id } ({ $window }) went into the drawer while it was on the trail in use, so it was taken off the trail. Walking a trail onto a window in the drawer crashes the compositor — a bug of Scroll, not of this system — and this keeps that from happening.
notif-action-trail-panel = Open the trails
window-menu-no = No
window-menu-yes-kill = Yes, force it to quit
window-menu-yes-close-others = Yes, close every other window
hint-window-menu = Click to open the options for everything that can be done to this window

# THE PERSON'S OWN PROGRAMS (LEADER i) — `software.rs`, its words
# (`software_words.rs`) and the queue's phases. A key is never text here:
# every sequence arrives as `{ $keys }`.
software-title = Software
group-software = programs
cmd-software-open = Your programs: search, install, try and remove
cmd-software-window = Your programs, in a window
cmd-software-trials = The programs being tried
cmd-software-maintenance = Updates, space and going back, for your programs
cmd-software-android = Android and its apps
cmd-software-launch-latest = Open what was installed last
software-down = down
software-up = up
software-enter = open
software-next-source = next source
software-previous-source = previous source
software-less = previous
software-more = next
software-install = install
software-try-it = try
software-launch = start
software-update = update
software-edit = edit
software-remove = remove
software-stop = stop
software-paste = paste
cmd-software-down = The next row
cmd-software-up = The row above
cmd-software-enter = Do what this row does; on a fact, copy it
cmd-software-next-source = Show the next source, or the next shelf of icons
cmd-software-previous-source = Show the previous source, or the previous shelf of icons
cmd-software-less = The previous value of this row
cmd-software-more = The next value of this row
cmd-software-install = Install it for you, from the source shown
cmd-software-try-it = Run it without installing
cmd-software-launch = Start the program
cmd-software-update = Update this one now
cmd-software-edit = Change this web app
cmd-software-remove = Remove it, after a question
cmd-software-stop = Stop it
cmd-software-paste = Paste what was copied into the search
cmd-software-web-new = A web app from an address
ask-software-web-new = Address of the site
cmd-software-web-address = Change the web app's address
ask-software-web-address = Address it opens
cmd-software-web-name = Change the web app's name
ask-software-web-name = Name in the launcher
cmd-software-icon-path = An image of yours as the web app's icon
ask-software-icon-path = Path of the image
cmd-software-file-path = Install an .AppImage, .apk or .flatpakref file
ask-software-file-path = Path of the file to install
software-type-on-a-row = To write here, press { $keys } on the row.
software-type-to-search = Type a name, or what it does, and the search starts by itself.
software-reading = Reading what is on this computer…
software-asking = asking…
software-on = on
software-off = off
software-joined = { $first } · { $second }
software-echo-queued = { $app } went to the queue — { $keys } shows it.
software-source-all = every source
software-source-nix = Nix
software-source-flatpak = Flatpak
software-source-appimage = AppImage
software-source-android = Android
software-source-web = web app
software-source-system = the system
software-origin-mine = installed by you
software-origin-declared = declared in the flake
software-search = Search
software-search-empty = type a name, or what it does
software-filter = Showing
software-heading-installed = Installed for you
software-note-yours = What is installed here is yours alone: the system and the repository stay as the flake says.
software-note-none = Nothing installed by you yet. The system's own programs are in the launcher — { $keys }.
software-note-none-here = Nothing from this source.
software-heading-trying = Being tried
software-heading-looking-after = Looking after
software-updates = Updates, space and going back
software-updates-count = { $count ->
        [one] one available
       *[other] { $count } available
    }
software-up-to-date = up to date
software-heading-more = More
software-web-new = New web app
software-web-new-value = from an address
software-trials = Trials
software-trials-count = { $count ->
        [0] none running
        [one] one running
       *[other] { $count } running
    }
software-android-state = Android system
software-android-absent = not on this machine
software-android-not-prepared = not prepared
software-android-preparing = preparing
software-android-running = running
software-android-stopped = stopped
software-advanced = Advanced
software-advanced-value = sources, permissions, history
software-heading-search = Looking for a program
software-make-web-app = Make a web app of it
software-open-once = Open it once, without making one
software-install-file = Install this file
software-not-a-package-file = That is not an .AppImage, .apk or .flatpakref file.
software-note-type = Type a name, or what it does. An address makes a web app; a path to an .AppImage, .apk or .flatpakref installs that file.
software-heading-results = Found
software-heading-by-description = Found by what they do
software-note-nothing-found = Nothing by that name. Another spelling, or what it does, may find it.
software-heading-asked = Where it looked
software-note-offline = No network: showing what this computer already knows.
software-status-from = based on { $from }
software-status-off = turned off in Advanced
software-row-working = Now
software-queued = waiting in the queue
software-working-now = working
software-working = { $percent } % · { $phase }
software-row-launch = Open the program
software-no-window = no window: it runs in a terminal
software-row-install = Install for you
software-install-with = with { $source }
software-recommended = recommended
software-size-download = { $size } to download
software-row-try = Try without installing
software-try-terminal = opens a terminal where { $program } can be typed
software-try-window = opens it in a window and keeps nothing
software-row-traces = What it may leave
software-row-network = Network while trying
software-row-keep = Keep it for good
software-keep-value = installs what was tried, with no new download
software-row-stop = Stop trying it
software-row-source = Source
software-also-in = also in { $sources }
software-row-version = Version
software-row-size = Size
software-row-update = Update
software-update-to = { $version } is ready
software-row-permissions = Permissions
software-permissions-value = what it may reach
software-row-edit = Change this web app
software-row-declare = Make it permanent
software-declare-copies = copies the line for host.nix
software-nothing-to-declare = This source has no line in the flake; it stays yours as it is.
software-note-declare-unfree = It is not free software: the line alone is not enough, the machine also has to allow this one package.
software-row-remove = Remove from this computer
software-heading-about = About it
software-heading-details = Details
software-row-reference = Its name in the source
software-row-publisher = Made by
software-row-licence = Licence
software-row-site = Site
software-row-needs = Needs
software-row-installed-size = Installed, it takes
software-row-permission = May reach
software-row-anti-features = F-Droid warns about
software-note-trial = Trying runs it now and installs nothing; it leaves the disk at the next clean-up.
software-note-why-nix = It comes from the same nixpkgs as the system, updates with it and follows its theme.
software-note-why-flatpak = Its developer publishes it and it runs isolated; the first one also downloads a shared base.
software-note-why-appimage = One file from its developer; it runs with all your permissions.
software-note-why-android = It runs inside Android, which is prepared once.
software-note-why-web = It is the site itself, in a window of its own; it is always the site's own version.
software-heading-trials = What you are trying
software-try-new = Try something new
software-try-new-value = type its name
software-note-no-trials = Nothing is being tried.
software-trial-running = open for { $minutes } min
software-trial-closed = closed · on disk until the next clean-up
software-heading-web = A new web app
software-web-address = Address
software-web-no-address = none yet
software-web-name = Name
software-web-icon = Icon on the launcher
software-web-no-icon-yet = none chosen yet
software-web-profile = Chrome profile
software-web-category = Kind
software-web-fetching = Reading the site's name and icons…
software-web-host-name = the name stays the site's address until one is typed
software-web-create = Create it
software-web-save = Save the changes
software-web-once = Open it once, as it will look
software-web-remove = Remove this web app
software-web-no-icon = Choose an icon first: the launcher needs one.
software-note-web = A web app is the site in a window with nothing of Chrome's; its logins are your Chrome profile's.
software-heading-icon = Its icon
software-shelf = From
software-shelf-site = the site itself
software-shelf-dashboard = Dashboard Icons
software-shelf-collection = the distro's collection
software-shelf-file = a file of yours
software-icon-looking-for = Looking for
software-icon-typed = Choose an image from the files
software-icon-clipboard = Use the copied image
software-icon-files = Pick one in the file manager
software-note-drop = Or drop an image here.
software-icon-now = Chosen now
software-icon-chosen = chosen
software-note-no-icons = No icon by that name here.
software-heading-android = Android
software-note-android = Android apps run inside an Android system beside this one; it costs memory while it runs.
software-android-image = Image
software-android-gapps = with Google Play (about 1.2 GB)
software-android-vanilla = without Google (about 0.8 GB)
software-note-gapps = Google Play asks for this machine to be registered once as an uncertified device; this page shows how once Android is prepared.
software-android-prepare = Prepare Android
software-note-preparing = Downloading and preparing Android's image — about a gigabyte, once. The queue shows it; no percentage is invented here.
software-heading-register = Registering for Google Play
software-note-uncertified = The Play Store only works after this device is registered on Google's page for uncertified devices: copy the id below and paste it there.
software-android-id = Device id
software-android-id-unknown = known once Android has run
software-android-register = Google's page
software-android-windows = Windows
software-android-one-per-app = one per app
software-android-one-for-all = one for all of Android
software-note-windows-next-session = A change of windows takes effect the next time Android starts.
software-heading-android-apps = Inside Android
software-note-no-android-apps = No app installed inside Android yet.
software-android-apk = Install an APK file
software-android-aurora = Google Play without an account
software-android-aurora-value = Aurora Store, from F-Droid
software-android-erase = Erase Android
software-cannot-undo = this cannot be undone
software-heading-updates = Updates
software-updates-all = Everything
software-web-always-latest = always the site's own version
software-follows-system = follows the system (nixpkgs { $rev })
software-look-again = Look for updates now
software-updates-auto = Automatically
software-auto-with = with the system's update — { $keys }
software-heading-space = Space
software-clean = Free space now
software-clean-generations = Old versions of your Nix list
software-clean-flatpak = Unused Flatpak runtimes
software-clean-appimage = AppImage leftovers
software-clean-trials = Tried programs
software-clean-trials-value = leave by themselves at the next clean-up
software-heading-generations = Going back (Nix)
software-note-no-generations = Your Nix list has no earlier version yet.
software-generation = Version { $n }
software-in-use = in use
software-older-versions = Older versions
software-heading-sources = Sources
software-online = Search online
software-online-off = off, with the search online
software-switch-flathub = Flathub
software-switch-fdroid = F-Droid
software-switch-appimagehub = AppImageHub and GitHub
software-switch-dashboard = Dashboard Icons, for web apps
software-nix-index = Nix index
software-prefer = Recommend first
software-prefer-nix-first = Nix, then a verified Flatpak
software-prefer-flatpak-first = Flatpak, then Nix
software-ask-above = Ask before downloading
software-ask-above-size = { $size } or more
software-ask-never = never
software-heading-trial-defaults = When trying
software-default-traces = What a trial may leave
software-default-network = Network in the sealed trial
software-heading-web-defaults = New web apps
software-web-profile-default = Chrome profile they use
software-profile-last-used = the one Chrome used last
software-heading-kept = What is kept
software-consents = Non-free packages allowed
software-consents-count = { $count ->
        [0] none
        [one] one
       *[other] { $count }
    }
software-history = History
software-history-value = the last few hundred things done
software-heading-permissions-list = Flatpak permissions
software-heading-dark = Dark Flatpaks
software-dark-gtk4 = GTK 4 and libadwaita
software-dark-follows = follow the system's choice
software-dark-gtk3 = GTK 3
software-dark-gtk3-value = dark, through the system's Adwaita-dark
software-dark-gtk3-missing = light: the Adwaita-dark extension is not installed
software-dark-qt = Qt and KDE
software-dark-qt-value = the session's Qt settings kept out
software-dark-qt-leaks = the session's Qt settings reach them
software-dark-forced = Forcing a theme
software-dark-none-forced = no app forces GTK_THEME
software-dark-forces = forces GTK_THEME — see its permissions
software-note-gtk-theme = GTK_THEME makes a Flatpak lose its own style; nothing here sets it, and an app that carries it is shown in red.
software-heading-declarative = Declaring it
software-note-declare = To keep a program across reinstalls, Make it permanent copies its line for host.nix; a Flatpak's goes in ronoctua.software.declaredFlatpaks, which nix-flatpak installs.
software-heading-permissions = What { $app } may reach
software-note-no-permissions = It asks for nothing beyond its own window.
software-note-permissions-restart = A change reaches the app the next time it starts.
software-heading-asks = What it asks for
software-gtk-theme-forced = forced into it: it loses its own style
software-sure-reset = Put { $app } back to what it asks for? Every change made here goes.
software-yes-reset = Yes, put it back
software-sure-unset-gtk-theme = Take GTK_THEME away from { $app }? It gets its own style back, and nothing here can put GTK_THEME back.
software-yes-unset-gtk-theme = Yes, take it away
software-heading-consents = Non-free packages you allowed
software-note-consents = Nix installs these although their licence is not free, and updates them with that same yes.
software-note-no-consents = None yet.
software-allowed = allowed
software-heading-history = What was done
software-note-no-history = Nothing done yet.
software-failed = did not work
software-no = No
software-sure-remove = Remove { $app }?
software-yes-remove = Yes, remove it and keep what it saved
software-yes-remove-data = Yes, and erase what it saved too
software-sure-web-remove = Remove the web app { $app }? The site and your login stay in Chrome.
software-yes-remove-web = Yes, remove the web app
software-sure-unfree = { $app } is not free software. Allow Nix to install this one?
software-yes-unfree = Yes, allow it
software-sure-unverified = Nothing vouches for what { $app } downloads. Install it anyway?
software-yes-trust = Yes, I trust whoever made it
software-sure-download = { $app } downloads { $size }. Go ahead?
software-yes-download = Yes, download it
software-sure-arm = { $app } is made for phone chips only and may not run here. Install it anyway?
software-yes-arm = Yes, install it anyway
software-sure-rollback = Go back to version { $n } of your Nix list?
software-yes-rollback = Yes, go back
software-sure-clean = Free what nothing uses? Old versions go, and going back to them stops being possible.
software-yes-clean = Yes, free it
software-sure-android = Prepare Android { $image }? It downloads once and stays until you erase it.
software-yes-android = Yes, prepare it
software-sure-erase-android = Erase Android, its apps and everything they saved?
software-yes-erase = Yes, erase it
software-sure-foreign = { $file } was checked by no catalogue and runs with your permissions. Install it?
software-sure-leave = Leave without saving?
software-yes-leave = Yes, throw the changes away
software-traces-scratch = none — a scratch folder, gone at logout
software-traces-home = your own folder and settings
software-traces-sandbox = sealed — none of your files
software-badge-verified = verified by its developer
software-badge-unfree = not free software
software-badge-sandboxed = runs isolated from your files
software-badge-command-line = a command, with no window
software-badge-arm-only = made for phone chips only
software-badge-broken = broken in this version of the system
software-badge-insecure = marked insecure
software-badge-end-of-life = its base gets no more updates
software-badge-anti-features = F-Droid warns about what it does
software-badge-library = a library, not a program
software-badge-installed = already installed
software-phase-resolving = working out what it means
software-phase-waiting = waiting for another order of the same source
software-phase-downloading = downloading
software-phase-verifying = checking it
software-phase-unpacking = unpacking
software-phase-building = building
software-phase-installing = installing
software-phase-integrating = putting it in the launcher
software-phase-removing = removing
software-phase-starting = starting
software-phase-indexing = reading the catalogue
software-phase-preparing = preparing
software-order-install = installed
software-order-install-file = installed from a file
software-order-remove = removed
software-order-upgrade = updated
software-order-tidy = freed space
software-order-try = tried
software-order-stop-trial = stopped trying
software-order-keep = kept after trying
software-order-prepare = prepared
software-order-refresh = catalogue fetched again
software-order-rollback = went back
software-order-web-create = web app created
software-order-web-edit = web app changed
software-order-android-session = Android started or stopped
software-order-android-erase = Android erased
software-order-permission = a permission changed
software-order-android-windows = Android's windows changed
software-grant-files = Your files
software-grant-network = Network
software-grant-sound = Sound
software-grant-devices = Cameras and other devices
software-grant-display = Draws through
software-grant-unset-gtk-theme = Take GTK_THEME away
software-grant-reset = Back to what it asks for
software-reach-read-write = read and write
software-reach-read-only = read only
software-reach-nothing = none — only what you hand it
software-display-wayland = Wayland
software-display-x11 = X11, through Xwayland
software-category-audiovideo = sound and video
software-category-audio = sound
software-category-video = video
software-category-development = programming
software-category-education = learning
software-category-game = games
software-category-graphics = pictures and drawing
software-category-network = the internet
software-category-office = office
software-category-science = science
software-category-settings = settings
software-category-system = system tools
software-category-utility = tools
software-refusal-unfree-needs-consent = It is not free software, and you have not allowed it yet.
software-refusal-unverified-needs-consent = Nothing vouches for this download, and you have not said you trust it.
software-refusal-big-needs-consent = It is bigger than you asked to be warned about, and you have not said yes.
software-refusal-insecure = Marked insecure in nixpkgs; not installed from here.
software-refusal-broken = Broken in this version of nixpkgs; it will not build.
software-refusal-unavailable = nixpkgs has it, but not for this kind of computer.
software-refusal-source-absent = This computer does not have what that source needs.
software-refusal-source-unprepared = That source has not been prepared yet.
software-refusal-declared-in-the-flake = It comes with the system (the flake); only the flake changes it.
software-refusal-arm-only = Made only for phone chips; it cannot run on this computer.
software-refusal-offline = No network to ask.
software-refusal-rate-limited = The service asked to wait before asking again.
software-refusal-integrity = What arrived does not match its checksum or signature, so it was thrown away.
software-refusal-too-large = The answer was larger than anything sane for that request.
software-refusal-not-an-appimage = That file is not an AppImage.
software-refusal-legacy-appimage = An old type-1 AppImage, which this system does not run.
software-refusal-dwarfs-appimage = An AppImage packed with DwarFS, which this system cannot open.
software-refusal-not-an-apk = That file is not an Android package.
software-refusal-url-scheme = Only http:// and https:// addresses make web apps.
software-refusal-url-credentials = An address with a name or a password in it is not kept.
software-refusal-url-invalid = That is not an address.
software-refusal-duplicate-window = Another web app already opens that same window in that profile.
software-refusal-no-browser = Web apps need Chrome, which is off on this system.
software-refusal-profile-unknown = Chrome has no profile by that name.
software-refusal-icon-unusable = That image cannot be an icon: unreadable, or too small.
software-refusal-profile-is-nix-env = Your Nix profile was made by the old nix-env, which nix profile refuses to change.
software-refusal-conflict = Two packages want the same file in your Nix profile.
software-refusal-not-cancellable-now = This part of the work cannot be interrupted.
software-refusal-unknown-item = Nothing by that name.
software-refusal-tool-failed = The program doing it failed; its own words are in the queue.
software-refusal-no-update-info = It says nothing about updates; look on its site.
software-refusal-cannot-try = Only a Nix package runs without being installed, and only one that has something to run.
software-refusal-session-down = Android is not running.
software-refusal-will-not-parse = software.json will not parse, and writing would replace it.
software-refusal-wrong-user = That runs as a different user from this one.
software-not-in-launcher = It is not in the launcher yet; the launcher is reading its folders again.
software-nothing-to-open = Nothing was installed since the footer started.
software-no-such-level = “{ $words }” names no page of the programs.
software-no-copied-image = No image was copied.
software-image-not-kept = The copied image could not be written to a file.
notif-software-app = programs
hint-software-trial = { $count ->
        [one] A program is being tried, and nothing of it is installed. Click to see it.
       *[other] { $count } programs are being tried, and nothing of them is installed. Click to see them.
    }
hint-software-android = Android is running, and it costs memory while it runs. Click for Android.
notif-software-installed-summary = { $app } is installed
notif-software-installed-body = It is in the launcher now, for you alone.
notif-software-web-created-summary = { $app } is ready
notif-software-web-created-body = The web app is in the launcher now.
notif-software-removed-summary = { $app } was removed
notif-software-removed-body = Its entry left the launcher.
notif-software-updated-summary = { $app } was updated
notif-software-updated-body = The new version starts the next time it opens.
notif-software-failed-summary = { $app } did not work
notif-software-failed-body = { $why }
notif-software-trial-ended-summary = { $app } closed
notif-software-trial-ended-body = Nothing of it was installed; it leaves the disk at the next clean-up.
notif-software-android-ready-summary = Android is ready
notif-software-android-ready-body = Apps inside it are installed from its page.
notif-action-software-launch-latest = Open it
notif-action-transfers-panel = Open the queue
notif-action-software-maintenance = See the updates
notif-action-software-trials = See the trials
notif-action-software-android = Open Android

# =====================================================================
# THE CHROME KIOSK — `LEADER m b`: the person's own Chrome without its
# bars, the modeline that adapts to it (`adapt.rs`, `kiosk.rs`), its view
# (`kiosk_view.rs`) and the `,` group. A key is never text here.
# =====================================================================
cmd-kiosk-open = Open your Chrome as a kiosk, without its bars
cmd-kiosk-back = Back, in the kiosk's tab
cmd-kiosk-forward = Forward, in the kiosk's tab
cmd-kiosk-reload = Reload the kiosk's tab
cmd-kiosk-tab = Bring this tab of the kiosk to the front
cmd-kiosk-tab-next = Next tab of the kiosk
cmd-kiosk-tab-previous = Previous tab of the kiosk
cmd-kiosk-tab-close = Close the kiosk's tab
cmd-kiosk-tab-left = Move the kiosk's tab one place left
cmd-kiosk-tab-right = Move the kiosk's tab one place right
cmd-kiosk-tab-move = Put a kiosk tab in another place
cmd-kiosk-new-tab = Open an address in a new tab of the kiosk
cmd-kiosk-address = Edit the address of the kiosk's tab
cmd-kiosk-zoom = The kiosk's zoom, in steps
cmd-kiosk-zoom-in = Zoom the kiosk's page in
cmd-kiosk-zoom-out = Zoom the kiosk's page out
cmd-kiosk-zoom-reset = Put the kiosk's zoom back to 100%
cmd-kiosk-chrome-ui = Show or hide Chrome's own bars on the kiosk
cmd-kiosk-address-line = Show or hide the address line above the modeline
cmd-kiosk-more = Everything the kiosk can do
cmd-kiosk-tabs = Every tab of the kiosk
cmd-kiosk-extensions = The kiosk's extensions
cmd-kiosk-extension = Open an extension's page in a new tab
cmd-kiosk-extension-options = Open an extension's options
cmd-kiosk-extension-details = An extension's details in Chrome
cmd-kiosk-release = Make the kiosk an ordinary Chrome window again
cmd-modeline-adapt-kiosk = Adapt the modeline to the kiosk, or not
cmd-footer-hide-push = Hide the modeline and the minibuffer, pushing the windows away when something opens; again, show them
cmd-footer-hide-cover = Hide the modeline and the minibuffer, laying what opens over the windows; again, show them
echo-footer-opaque = This screen cannot draw the footer see-through, so the hidden bars leave an opaque strip.
cmd-kiosk-list-down = Next row of the kiosk's list
cmd-kiosk-list-up = Previous row of the kiosk's list
cmd-kiosk-list-enter = The row of the kiosk's list: the tab to the front, or the extension's page
cmd-kiosk-tab-earlier = Move the tab under the cursor one place earlier
cmd-kiosk-tab-later = Move the tab under the cursor one place later
cmd-wm-full-application-on = Tell the window it is fullscreen, in its own tile
cmd-wm-full-application-off = Tell the window it is no longer fullscreen
kiosk-list-down = down
kiosk-list-up = up
kiosk-list-enter = open
kiosk-tab-earlier = earlier
kiosk-tab-later = later
kiosk-tab-close = close
kiosk-new-tab = new tab
kiosk-extension-options = options
kiosk-extension-details = details
ask-kiosk-address = The address, or words to search
ask-kiosk-new-tab = Address or words for the new tab (empty opens a blank one)
kiosk-tabs-more = +{ $count }
kiosk-zoom-percent = { $percent }%
kiosk-tab-front = { $host }, in front
kiosk-menu-back = Back
kiosk-menu-forward = Forward
kiosk-menu-reload = Reload
kiosk-menu-new-tab = New tab
kiosk-menu-address = Edit the address
kiosk-menu-tabs = Tabs
kiosk-menu-tab-previous = Previous tab
kiosk-menu-tab-next = Next tab
kiosk-menu-tab-left = Tab to the left
kiosk-menu-tab-right = Tab to the right
kiosk-menu-tab-close = Close the tab
kiosk-menu-extensions = Extensions
kiosk-menu-zoom = Zoom
kiosk-menu-zoom-in = Bigger
kiosk-menu-zoom-out = Smaller
kiosk-menu-zoom-reset = Actual size
kiosk-menu-show-chrome = Show Chrome's bars
kiosk-menu-hide-chrome = Hide Chrome's bars
kiosk-menu-address-show = Show the address line
kiosk-menu-address-hide = Hide the address line
kiosk-menu-release = Ordinary window
kiosk-heading-pinned = Pinned
kiosk-heading-others = The others
kiosk-heading-disabled = Turned off
kiosk-no-extensions = This Chrome has no extension besides ours.
window-menu-kiosk = Kiosk
hint-kiosk-back = One page back in this tab
hint-kiosk-forward = One page forward in this tab
hint-kiosk-reload = Load this page again
hint-kiosk-tabs-before = { $count ->
        [one] One more tab to the left
       *[other] { $count } more tabs to the left
    } — click lists them all
hint-kiosk-tabs-after = { $count ->
        [one] One more tab to the right
       *[other] { $count } more tabs to the right
    } — click lists them all
hint-kiosk-tab = { $title } — click brings it to the front, drag it to move it, the middle button closes it, the right button lists every tab
hint-kiosk-tab-current = { $title } — the tab in front; click lists every tab, the middle button closes it
hint-kiosk-new-tab = A new tab, at an address or a search
hint-kiosk-ext = { $name } — click opens its page in a new tab, the right button its options
hint-kiosk-extensions = { $names } — click lists every extension
hint-kiosk-extensions-none = No extension besides ours — click lists them
hint-kiosk-zoom = The page at { $percent }% — the wheel changes it, click opens the steps, the right button goes back to 100%
hint-kiosk-address-line-on = The address line is up — click hides it
hint-kiosk-address-line-off = The address line is hidden — click shows it above the modeline
hint-kiosk-chrome-hidden = Chrome's bars are hidden — click shows them (normal mode)
hint-kiosk-chrome-shown = Chrome's bars are showing — click hides them again
hint-kiosk-more = Everything the kiosk can do
hint-kiosk-offline-bridge = The browser bridge is not installed on this machine: the kiosk can only show and hide Chrome's bars
hint-kiosk-offline-chrome = Chrome has not connected — the ronoctua extension may be off
hint-kiosk-offline-version = The extension is another version than the footer: restart Chrome
hint-kiosk-address = { $url } — click edits the address
echo-kiosk-offline = Chrome is not connected, so the kiosk cannot do that yet.
echo-kiosk-unmatched = The kiosk window did not answer; it opened as an ordinary Chrome window.
echo-kiosk-no-history = Nothing behind in this tab.
echo-kiosk-no-future = Nothing ahead in this tab.
echo-kiosk-not-a-kiosk = The window in focus is not a kiosk.
echo-kiosk-address-refused = That address cannot open here: only http and https.
echo-kiosk-refused = Chrome refused: { $why }
echo-kiosk-no-chrome = Chrome is off on this system.
echo-kiosk-no-bridge = The browser bridge is not installed, so a kiosk cannot be paired.
echo-kiosk-no-page = That extension has no page of its own; its details opened instead.
echo-kiosk-no-extension = No extension by that id is in this Chrome.
echo-kiosk-no-nonce = No random number could be read to pair a kiosk.
echo-kiosk-no-tab = The kiosk has no such tab.
echo-modeline-will-not-parse = modeline.json will not parse, and writing would replace what is in it.

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

# THE BRAIN'S MODULES — what a notice answers to, on the strip under the
# minibuffer and on the brain's page; the page's notices, a watch, and the
# activity log folded.
cmd-brain-act = Do what it offers
cmd-brain-dismiss = Put it away
cmd-brain-snooze = Put it off
about-brain-act = Do what the notice in front offers: the first action of the module that raised it.
about-brain-dismiss = Put the notice in front away. It comes back only if its module raises it again.
about-brain-snooze = Put the notice in front off, for as long as its first snooze says.
brain-band-dismiss = put away
brain-band-snooze = { $minutes } min
brain-band-more = +{ $count }
hint-brain-more = every notice, on the brain's page
echo-brain-no-notice = The brain has nothing to say right now
echo-brain-not-offered = The notice does not offer that
echo-brain-no-action = No module of the brain offers that
echo-brain-dismissed = Put away
echo-brain-snoozed = Put off
echo-brain-taken = The brain took it
echo-brain-refused = The brain would not: { $why }
brain-outcome-noted = Noted
brain-notices = Notices
brain-actions = What it offers
brain-watching = Watching
brain-activity-last = Last:
brain-activity-first = First
brain-activity-times = Times
brain-since = Since

# ICONS — the library the icons app and fast-access share (`ronoctua-iconset`).
# The words a person TYPES before a colon to narrow a search: the search
# language is interface, so every language's word is accepted.
icons-prefix-block = block
icons-prefix-tag = tag
icons-prefix-token = token
icons-prefix-app = app
icons-prefix-symbols = symbols
icons-prefix-family = family
icons-prefix-category = category
icons-prefix-cat = cat
icons-prefix-name = name
icons-prefix-group = group
icons-prefix-subgroup = subgroup
icons-prefix-tone = tone
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
icons-set-cod = Codicons
icons-set-custom = Custom
icons-set-dev = Devicons
icons-set-extra = Extra
icons-set-fa = Font Awesome
icons-set-fae = Font Awesome Extension
icons-set-iec = IEC Power
icons-set-indent = Indentation
icons-set-indentation = Indentation
icons-set-linux = Font Logos
icons-set-md = Material Design
icons-set-oct = Octicons
icons-set-pl = Powerline
icons-set-ple = Powerline Extra
icons-set-pom = Pomicons
icons-set-seti = Seti-UI
icons-set-weather = Weather
icons-category-all = All categories
icons-category-accessibility = Accessibility
icons-category-account = Account
icons-category-animals = Animals
icons-category-arrows = Arrows
icons-category-brands = Brands
icons-category-buildings = Buildings
icons-category-charts = Charts
icons-category-communication = Communication
icons-category-connectivity = Connectivity
icons-category-cursors = Cursors
icons-category-design = Design
icons-category-development = Development
icons-category-devices = Devices
icons-category-emoji = Emoji
icons-category-files = Files
icons-category-finance = Finance
icons-category-food-beverage = Food
icons-category-gaming = Gaming
icons-category-home = Home
icons-category-layout = Layout
icons-category-mail = Mail
icons-category-math = Math
icons-category-medical = Medical
icons-category-multimedia = Multimedia
icons-category-nature = Nature
icons-category-navigation = Navigation
icons-category-notifications = Notifications
icons-category-people = People
icons-category-photography = Photography
icons-category-science = Science
icons-category-seasons = Seasons
icons-category-security = Security
icons-category-shapes = Shapes
icons-category-shopping = Shopping
icons-category-social = Social
icons-category-sports = Sports
icons-category-sustainability = Sustainability
icons-category-text = Text
icons-category-time = Time
icons-category-tools = Tools
icons-category-transportation = Transportation
icons-category-travel = Travel
icons-category-weather = Weather
icons-family-all = All families
icons-family-symbol = Symbol
icons-family-letter = Letter
icons-family-number = Number
icons-family-mark = Mark
icons-family-punctuation = Punctuation
icons-family-other = Other
icons-family-emoji = Emoji
icons-group-faces = Smileys & emotion
icons-group-people = People & body
icons-group-nature = Animals & nature
icons-group-food = Food & drink
icons-group-travel = Travel & places
icons-group-activities = Activities
icons-group-objects = Objects
icons-group-symbols = Symbols
icons-group-flags = Flags
icons-tone-light = Light skin
icons-tone-medium-light = Medium-light skin
icons-tone-medium = Medium skin
icons-tone-medium-dark = Medium-dark skin
icons-tone-dark = Dark skin
# Why there are no icons to show.
icons-why-no-variable = The icon tables were not given to this program (RONOCTUA_ICONSET is not set).
icons-why-unreadable = An icon table is missing or cannot be read: { $file }.
icons-why-garbled = An icon table is not in the shape its generator writes: { $file }.
# THE CALCULATOR'S ENGINE (`ronoctua-calc`): why an expression has no value,
# the converter's categories, the shapes of a quick answer, and every unit's
# name — chosen by the number (`$count`, a number), as `assets/units.nix` declares.
calc-why-empty = There is nothing to calculate.
calc-why-unexpected = Something here does not belong where it is.
calc-why-unbalanced = A parenthesis is opened and never closed, or closed and never opened.
calc-why-unknown-word = A word that is not a unit, a currency, a function or a time zone.
calc-why-incompatible = These are different kinds of quantity and cannot be combined.
calc-why-divide-by-zero = A division by zero.
calc-why-too-large = The result is too large to hold.
calc-why-domain = The function has no value there.
calc-why-no-rates = Converting between currencies needs the exchange rates, and there are none yet.
calc-why-unknown-currency = The exchange rates do not quote this currency.
calc-why-unknown-zone = No time zone has that name.
calc-why-bad-date = That date does not exist.
calc-why-bad-roman = That is not a Roman numeral (1 to 3999, in the usual form).
calc-why-bad-digit = A digit that the number's base does not have.
calc-rates-why-wrong-pass = The permission given is for another site than the European Central Bank's.
calc-rates-why-unreachable = The European Central Bank could not be reached.
calc-rates-why-failed = The European Central Bank answered with a failure.
calc-rates-why-garbled = What came back is not the European Central Bank's rates file.
calc-category-length = Length
calc-category-mass = Mass
calc-category-volume = Volume
calc-category-area = Area
calc-category-temperature = Temperature
calc-category-time = Time
calc-category-speed = Speed
calc-category-data = Data
calc-category-energy = Energy
calc-category-power = Power
calc-category-pressure = Pressure
calc-category-angle = Angle
calc-category-frequency = Frequency
calc-category-fuel-economy = Fuel economy
calc-category-cooking = Cooking
calc-shape-arithmetic = Arithmetic
calc-shape-conversion = Conversion
calc-shape-percentage = Percentage
calc-shape-base = Number base
calc-shape-roman = Roman numerals
calc-shape-date = Date
calc-shape-zone = Time zone
calc-shape-currency = Currency
unit-metre = { $count ->
        [one] metre
       *[other] metres
    }
unit-kilometre = { $count ->
        [one] kilometre
       *[other] kilometres
    }
unit-centimetre = { $count ->
        [one] centimetre
       *[other] centimetres
    }
unit-millimetre = { $count ->
        [one] millimetre
       *[other] millimetres
    }
unit-micrometre = { $count ->
        [one] micrometre
       *[other] micrometres
    }
unit-nanometre = { $count ->
        [one] nanometre
       *[other] nanometres
    }
unit-mile = { $count ->
        [one] mile
       *[other] miles
    }
unit-yard = { $count ->
        [one] yard
       *[other] yards
    }
unit-foot = { $count ->
        [one] foot
       *[other] feet
    }
unit-inch = { $count ->
        [one] inch
       *[other] inches
    }
unit-nautical-mile = { $count ->
        [one] nautical mile
       *[other] nautical miles
    }
unit-astronomical-unit = { $count ->
        [one] astronomical unit
       *[other] astronomical units
    }
unit-light-year = { $count ->
        [one] light-year
       *[other] light-years
    }
unit-kilogram = { $count ->
        [one] kilogram
       *[other] kilograms
    }
unit-gram = { $count ->
        [one] gram
       *[other] grams
    }
unit-milligram = { $count ->
        [one] milligram
       *[other] milligrams
    }
unit-microgram = { $count ->
        [one] microgram
       *[other] micrograms
    }
unit-tonne = { $count ->
        [one] tonne
       *[other] tonnes
    }
unit-pound = { $count ->
        [one] pound
       *[other] pounds
    }
unit-ounce = { $count ->
        [one] ounce
       *[other] ounces
    }
unit-stone = { $count ->
        [one] stone
       *[other] stones
    }
unit-carat = { $count ->
        [one] carat
       *[other] carats
    }
unit-short-ton = { $count ->
        [one] short ton
       *[other] short tons
    }
unit-long-ton = { $count ->
        [one] long ton
       *[other] long tons
    }
unit-cubic-metre = { $count ->
        [one] cubic metre
       *[other] cubic metres
    }
unit-litre = { $count ->
        [one] litre
       *[other] litres
    }
unit-decilitre = { $count ->
        [one] decilitre
       *[other] decilitres
    }
unit-centilitre = { $count ->
        [one] centilitre
       *[other] centilitres
    }
unit-millilitre = { $count ->
        [one] millilitre
       *[other] millilitres
    }
unit-cubic-centimetre = { $count ->
        [one] cubic centimetre
       *[other] cubic centimetres
    }
unit-gallon = { $count ->
        [one] gallon
       *[other] gallons
    }
unit-imperial-gallon = { $count ->
        [one] imperial gallon
       *[other] imperial gallons
    }
unit-quart = { $count ->
        [one] quart
       *[other] quarts
    }
unit-pint = { $count ->
        [one] pint
       *[other] pints
    }
unit-fluid-ounce = { $count ->
        [one] fluid ounce
       *[other] fluid ounces
    }
unit-cubic-foot = { $count ->
        [one] cubic foot
       *[other] cubic feet
    }
unit-cubic-inch = { $count ->
        [one] cubic inch
       *[other] cubic inches
    }
unit-square-metre = { $count ->
        [one] square metre
       *[other] square metres
    }
unit-square-kilometre = { $count ->
        [one] square kilometre
       *[other] square kilometres
    }
unit-square-centimetre = { $count ->
        [one] square centimetre
       *[other] square centimetres
    }
unit-square-millimetre = { $count ->
        [one] square millimetre
       *[other] square millimetres
    }
unit-hectare = { $count ->
        [one] hectare
       *[other] hectares
    }
unit-acre = { $count ->
        [one] acre
       *[other] acres
    }
unit-square-foot = { $count ->
        [one] square foot
       *[other] square feet
    }
unit-square-inch = { $count ->
        [one] square inch
       *[other] square inches
    }
unit-square-yard = { $count ->
        [one] square yard
       *[other] square yards
    }
unit-square-mile = { $count ->
        [one] square mile
       *[other] square miles
    }
unit-kelvin = { $count ->
        [one] kelvin
       *[other] kelvins
    }
unit-celsius = { $count ->
        [one] degree Celsius
       *[other] degrees Celsius
    }
unit-fahrenheit = { $count ->
        [one] degree Fahrenheit
       *[other] degrees Fahrenheit
    }
unit-rankine = { $count ->
        [one] degree Rankine
       *[other] degrees Rankine
    }
unit-second = { $count ->
        [one] second
       *[other] seconds
    }
unit-millisecond = { $count ->
        [one] millisecond
       *[other] milliseconds
    }
unit-microsecond = { $count ->
        [one] microsecond
       *[other] microseconds
    }
unit-nanosecond = { $count ->
        [one] nanosecond
       *[other] nanoseconds
    }
unit-minute = { $count ->
        [one] minute
       *[other] minutes
    }
unit-hour = { $count ->
        [one] hour
       *[other] hours
    }
unit-day = { $count ->
        [one] day
       *[other] days
    }
unit-week = { $count ->
        [one] week
       *[other] weeks
    }
unit-month = { $count ->
        [one] month
       *[other] months
    }
unit-year = { $count ->
        [one] year
       *[other] years
    }
unit-metre-per-second = { $count ->
        [one] metre per second
       *[other] metres per second
    }
unit-kilometre-per-hour = { $count ->
        [one] kilometre per hour
       *[other] kilometres per hour
    }
unit-mile-per-hour = { $count ->
        [one] mile per hour
       *[other] miles per hour
    }
unit-knot = { $count ->
        [one] knot
       *[other] knots
    }
unit-foot-per-second = { $count ->
        [one] foot per second
       *[other] feet per second
    }
unit-bit = { $count ->
        [one] bit
       *[other] bits
    }
unit-byte = { $count ->
        [one] byte
       *[other] bytes
    }
unit-kilobit = { $count ->
        [one] kilobit
       *[other] kilobits
    }
unit-megabit = { $count ->
        [one] megabit
       *[other] megabits
    }
unit-gigabit = { $count ->
        [one] gigabit
       *[other] gigabits
    }
unit-kibibit = { $count ->
        [one] kibibit
       *[other] kibibits
    }
unit-mebibit = { $count ->
        [one] mebibit
       *[other] mebibits
    }
unit-kilobyte = { $count ->
        [one] kilobyte
       *[other] kilobytes
    }
unit-megabyte = { $count ->
        [one] megabyte
       *[other] megabytes
    }
unit-gigabyte = { $count ->
        [one] gigabyte
       *[other] gigabytes
    }
unit-terabyte = { $count ->
        [one] terabyte
       *[other] terabytes
    }
unit-petabyte = { $count ->
        [one] petabyte
       *[other] petabytes
    }
unit-kibibyte = { $count ->
        [one] kibibyte
       *[other] kibibytes
    }
unit-mebibyte = { $count ->
        [one] mebibyte
       *[other] mebibytes
    }
unit-gibibyte = { $count ->
        [one] gibibyte
       *[other] gibibytes
    }
unit-tebibyte = { $count ->
        [one] tebibyte
       *[other] tebibytes
    }
unit-pebibyte = { $count ->
        [one] pebibyte
       *[other] pebibytes
    }
unit-joule = { $count ->
        [one] joule
       *[other] joules
    }
unit-kilojoule = { $count ->
        [one] kilojoule
       *[other] kilojoules
    }
unit-megajoule = { $count ->
        [one] megajoule
       *[other] megajoules
    }
unit-calorie = { $count ->
        [one] calorie
       *[other] calories
    }
unit-kilocalorie = { $count ->
        [one] kilocalorie
       *[other] kilocalories
    }
unit-watt-hour = { $count ->
        [one] watt-hour
       *[other] watt-hours
    }
unit-kilowatt-hour = { $count ->
        [one] kilowatt-hour
       *[other] kilowatt-hours
    }
unit-electronvolt = { $count ->
        [one] electronvolt
       *[other] electronvolts
    }
unit-british-thermal-unit = { $count ->
        [one] British thermal unit
       *[other] British thermal units
    }
unit-watt = { $count ->
        [one] watt
       *[other] watts
    }
unit-kilowatt = { $count ->
        [one] kilowatt
       *[other] kilowatts
    }
unit-megawatt = { $count ->
        [one] megawatt
       *[other] megawatts
    }
unit-horsepower = horsepower
unit-metric-horsepower = metric horsepower
unit-pascal = { $count ->
        [one] pascal
       *[other] pascals
    }
unit-hectopascal = { $count ->
        [one] hectopascal
       *[other] hectopascals
    }
unit-kilopascal = { $count ->
        [one] kilopascal
       *[other] kilopascals
    }
unit-megapascal = { $count ->
        [one] megapascal
       *[other] megapascals
    }
unit-bar = { $count ->
        [one] bar
       *[other] bars
    }
unit-millibar = { $count ->
        [one] millibar
       *[other] millibars
    }
unit-atmosphere = { $count ->
        [one] atmosphere
       *[other] atmospheres
    }
unit-pound-per-square-inch = { $count ->
        [one] pound per square inch
       *[other] pounds per square inch
    }
unit-millimetre-of-mercury = { $count ->
        [one] millimetre of mercury
       *[other] millimetres of mercury
    }
unit-torr = { $count ->
        [one] torr
       *[other] torrs
    }
unit-radian = { $count ->
        [one] radian
       *[other] radians
    }
unit-degree = { $count ->
        [one] degree
       *[other] degrees
    }
unit-gradian = { $count ->
        [one] gradian
       *[other] gradians
    }
unit-arcminute = { $count ->
        [one] arcminute
       *[other] arcminutes
    }
unit-arcsecond = { $count ->
        [one] arcsecond
       *[other] arcseconds
    }
unit-turn = { $count ->
        [one] turn
       *[other] turns
    }
unit-hertz = hertz
unit-kilohertz = kilohertz
unit-megahertz = megahertz
unit-gigahertz = gigahertz
unit-revolution-per-minute = { $count ->
        [one] revolution per minute
       *[other] revolutions per minute
    }
unit-litre-per-100-km = { $count ->
        [one] litre per 100 kilometres
       *[other] litres per 100 kilometres
    }
unit-kilometre-per-litre = { $count ->
        [one] kilometre per litre
       *[other] kilometres per litre
    }
unit-mile-per-gallon = { $count ->
        [one] mile per gallon
       *[other] miles per gallon
    }
unit-mile-per-imperial-gallon = { $count ->
        [one] mile per imperial gallon
       *[other] miles per imperial gallon
    }
unit-cup = { $count ->
        [one] cup
       *[other] cups
    }
unit-customary-cup = { $count ->
        [one] customary cup
       *[other] customary cups
    }
unit-metric-cup = { $count ->
        [one] metric cup
       *[other] metric cups
    }
unit-tablespoon = { $count ->
        [one] tablespoon
       *[other] tablespoons
    }
unit-customary-tablespoon = { $count ->
        [one] customary tablespoon
       *[other] customary tablespoons
    }
unit-teaspoon = { $count ->
        [one] teaspoon
       *[other] teaspoons
    }
unit-customary-teaspoon = { $count ->
        [one] customary teaspoon
       *[other] customary teaspoons
    }

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

# THE LEADER'S SETS AND THE BARS' MODES (2026-09-30): the leader pressed again
# walks the sets, and a tap of Ctrl at the root walks the bars' three modes.
leader-set-default = default
cmd-leader-set-default = The keys as they are, each opening its app in the footer
cmd-footer-bars-cycle = The next of the bars' three modes: shown, hidden and pushing the windows, hidden and over them
cmd-leader-close = Close the leader's list, as its Esc does
echo-bars-shown = Modeline and minibuffer shown
echo-bars-push = Modeline and minibuffer hidden; what opens pushes the windows away
echo-bars-cover = Modeline and minibuffer hidden; what opens lies over the windows
# THE MINIBUFFER'S DRAWER by name, and the compositor's transient keys for it.
cmd-minibuffer-fold-open = Open the minibuffer's line to read the rest of it, or walk down it
cmd-minibuffer-fold-close = Walk up the opened minibuffer's line, or close it
cmd-wm-fold-keys-hold = Let Ctrl with the arrows walk the minibuffer's line while a hint too long for it is on it
cmd-wm-fold-keys-free = Give Ctrl with the arrows back to the windows
# THE MAXIMUM ALERT BAR (alert.rs). The title is drawn in capitals.
alert-battery-low = Low battery!!!
alert-battery-low-body = { $percent }% left, on battery, and the charger is not in.
hint-alert-close = Closes this alert; it comes back if the battery falls another step
cmd-alert-dismiss = Close the alert bar; it comes back if what caused it gets worse
# A PREFERENCE THAT COULD NOT BE SAVED is said, never only logged.
echo-preference-not-written = { $file } could not be saved ({ $why }): what you changed lasts only until the footer restarts.
# In a window the change is not in force anywhere: the file is the only road.
echo-setting-not-written = { $file } could not be saved ({ $why }), so nothing changed.
# THE FOOTER'S SETTINGS (LEADER s r, footer_settings.rs).
group-footer-settings = footer
# The modeline's name for its window (`br.ronoctua.footer-settings`).
mode-footer-settings = footer settings
footer-settings-title = Footer
cmd-footer-settings-open = The footer's settings: the bars, the free minibuffer and the kiosk
cmd-footer-settings-window = The footer's settings, in a window
cmd-footer-settings-up = Previous row
cmd-footer-settings-down = Next row
cmd-footer-settings-enter = Change the setting under the cursor
footer-settings-up = up
footer-settings-down = down
footer-settings-enter = change
cmd-footer-hours = Show the time on the free minibuffer, or stop showing it
footer-settings-bars = The bars
footer-settings-bars-mode = Modeline and minibuffer
footer-settings-bars-shown = shown
footer-settings-bars-push = hidden, pushing
footer-settings-bars-cover = hidden, covering
footer-settings-bars-note = Hidden, the bars come back at the screen's bottom edge; pushing, what opens moves the windows away, and covering, it lies over them. A tap of { $keys } at the leader's root goes to the next of the three.
footer-settings-free = The free minibuffer
footer-settings-hours = Hours
footer-settings-hours-note = The time, at the right of the minibuffer, while nothing else is said on it.
footer-settings-kiosk = Kiosk
footer-settings-kiosk-adapt = Adapt the modeline to a kiosk
footer-settings-on = on
footer-settings-off = off

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

# A MODULE THE SYSTEM SWITCHED OFF (`ronoctua.brain.modules.<id>.enable =
# false`): its row's reason, and what every ask about it is refused with — a
# row's reason sits in a column sixteen cells wide.
brain-why-system-off = Off by system
# And the date-and-time page's line under the clock watch's switch, when the
# system switched that watch off.
locale-watched-system-off = The system's configuration switched this watch off, so it cannot be switched on from here; the configuration switches it back on.

## file picker — the one picker of a file, a folder or a name to save under
file-picker-filter-any = Every file
file-picker-filter-images = Pictures
file-picker-filter-media = What the player plays
file-picker-filter-lanes = What plays together
file-picker-filter-saved = Sets and queues
file-picker-filter-subtitles = Subtitles
file-picker-filter-packages = Programs to install
file-picker-filter-tunnels = Tunnels (.conf, .ovpn)
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
player-page-pick-save-set = Save what plays as a set
player-page-pick-save-queue = Save the queue
player-added-several = { $count } files added to the queue.
player-playing-several = Playing { $count } files now.
syncthing-pick-folder = A folder on this machine
network-pick-tunnel = A tunnel to bring in
software-pick-package = A file to install — an AppImage, an APK or a Flatpak reference
camera-pick-backdrops = Pictures for behind you: the arrows turn among the ones taken

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
files-ask-modified = Modified on (year-month-day hour:minute:second):
files-modified-bad = That is not a date this can read. Write it as 2026-09-30 14:05, seconds optional.
files-ask-tag = { $field }:
files-ask-meta = { $detail }:
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
files-remove-location = remove the place it was made
files-does-remove-location = asks first; then rewrites the file — or writes a copy beside it — without its GPS coordinate. Taken out of the file itself, it cannot be undone
files-trash-on-volume = Going to { $volume }'s own trash, not the home's: its space stays taken on { $volume } until that trash lets it go — which it does on its own, like the home's, while the drive is plugged in.
files-trash-left-on-volume = { $volume } left with files in its own trash ({ $n }): their space stays taken on it until that trash lets them go, which happens only while it is plugged in.

# THE GIT LINE under the file manager's path, and the git browser `C-h`
# opens (`git_line.rs`, `git_browser.rs`). What is quoted is data: a branch,
# a subject, a path.
files-git = history
# The browser's window in the modeline (`br.ronoctua.git`, `daemon::mode_label`).
mode-git = git history
about-files-git = Open the repository's history: its changes, commits, branches, stashes and remotes, beside what each one changed.
files-git-title = History of { $name }
files-git-detached = detached at { $id }
files-git-unborn = { $branch }, no commits yet
files-git-changed = { $n ->
        [one] 1 changed
       *[other] { $n } changed
    }
files-git-staged = { $n ->
        [one] 1 staged
       *[other] { $n } staged
    }
files-git-untracked = { $n ->
        [one] 1 untracked
       *[other] { $n } untracked
    }
files-git-conflicted = { $n ->
        [one] 1 in conflict
       *[other] { $n } in conflict
    }
files-git-clean = nothing changed
files-git-list-item = { $item },
files-git-subject = “{ $subject }”
files-git-line-explain = On { $branch }: { $changed } changed, { $staged } staged, { $untracked } untracked. { $keys } or a click opens the history.
files-git-line-refused = This folder is in a repository git would not read. { $keys } opens the history, which says why.
files-git-age-now = just now
files-git-age-minutes = { $n ->
        [one] a minute ago
       *[other] { $n } minutes ago
    }
files-git-age-hours = { $n ->
        [one] an hour ago
       *[other] { $n } hours ago
    }
files-git-age-days = { $n ->
        [one] yesterday
       *[other] { $n } days ago
    }
files-git-age-months = { $n ->
        [one] a month ago
       *[other] { $n } months ago
    }
files-git-age-years = { $n ->
        [one] a year ago
       *[other] { $n } years ago
    }
# Why git answered nothing (`ronoctua_git::Why`), one per reason.
files-git-why-no-git = This session was not given git, so the repository cannot be read.
files-git-why-no-ssh = This remote is reached over SSH and this session was not given ssh.
files-git-why-no-askpass = This remote wants a token and the helper that hands it over is missing.
files-git-why-not-a-repo = This folder is not in a repository.
files-git-why-unstartable = git could not be started.
files-git-why-failed = git could not read this.
files-git-why-garbled = git answered something this browser does not understand.
files-git-why-needs-credentials = The remote wants a name and a password or a key, and fetching never asks for one.
files-git-why-refused = The remote refused the credentials it was given.
files-git-why-unreachable = The remote could not be reached.
files-git-why-too-large = It is larger than a view shows.
# The browser's columns.
files-git-kind-changes = Changes
files-git-kind-history = History
files-git-kind-refs = Branches and tags
files-git-kind-stashes = Stashes
files-git-kind-remotes = Remotes
files-git-kind-count = { $kind } { $n }
files-git-file-history-title = History of { $path }
files-git-reading = Reading…
files-git-empty-changes = Nothing changed in the work tree.
files-git-empty-history = No commits yet.
files-git-empty-refs = No branches and no tags.
files-git-empty-stashes = Nothing is stashed.
files-git-empty-remotes = No remotes.
files-git-empty-file = No commit touched this file.
files-git-pick-item = Pick something on the left to see it here.
files-git-pick-file = Pick a file to see what changed in it.
files-git-diff-no-files = A remote changes no files.
files-git-diff-nothing = No change in the content.
files-git-diff-staged = Staged
files-git-diff-unstaged = Not staged
files-git-diff-untracked = Not in the repository yet
files-git-diff-binary = Not text: nothing to show line by line.
files-git-diff-no-content = Only the file's details changed, not what is in it.
files-git-diff-link = A symbolic link, and where it points:
files-git-diff-special = Not a file — a pipe, a socket or a device — so nothing in it is read.
files-git-blame-title = Who wrote each line of { $path }
files-git-files = { $n ->
        [one] 1 file changed
       *[other] { $n } files changed
    }
files-git-remote-branches = Its branches, as last fetched
files-git-no-remote-branches = None fetched yet.
files-git-fact-path = file
files-git-fact-state = state
files-git-fact-from = renamed from
files-git-fact-name = name
files-git-fact-kind = kind
files-git-fact-upstream = follows
files-git-fact-fetch = fetches from
files-git-fact-push = pushes to
files-git-fact-stash = stash
files-git-fact-commit = commit
files-git-fact-author = author
files-git-fact-date = date
files-git-fact-parents = parents
files-git-ref-branch = a branch
files-git-ref-remote = a remote's branch
files-git-ref-tag = a tag
files-git-mark-branch = branch
files-git-mark-remote = remote
files-git-mark-tag = tag
files-git-change-added = added
files-git-change-modified = changed
files-git-change-deleted = deleted
files-git-change-renamed = renamed
files-git-change-copied = copied
files-git-change-type = changed its type
files-git-change-unmerged = unmerged
files-git-state-untracked = not in the repository yet
files-git-state-conflicted = in conflict
files-git-state-staged = staged: { $what }
files-git-state-unstaged = not staged: { $what }
# What the browser says after an action.
files-git-fetched = Fetched: the remotes' branches are up to date.
files-git-fetch-refused = Nothing was fetched. { $why }
files-git-fetching = A fetch is already under way.
files-git-no-file = There is no file under the cursor.
files-git-blame-gone = The file is not in the work tree any more, so nobody wrote its lines there.
files-git-nothing-to-copy = There is no commit under the cursor.
files-git-no-version = There is no older version of this file to open.
files-git-kept = Opened { $path } as { $id } has it — a copy that cannot be changed.
files-git-keep-failed = The copy could not be made: { $why }
files-git-keep-why-disk = The disk would not take it: it is full, or the cache cannot be written there.
# The browser's keys: the word the hint line shows, and what the pointer
# resting on it is told.
files-git-down = down
files-git-up = up
files-git-enter = open
files-git-next = next column
files-git-previous = previous column
files-git-page-down = page down
files-git-page-up = page up
files-git-changes = changes
files-git-history = history
files-git-refs = branches
files-git-stashes = stashes
files-git-remotes = remotes
files-git-file-history = file's history
files-git-blame = who wrote it
files-git-copy = copy id
files-git-open-version = open this version
files-git-fetch = fetch
files-git-terminal = terminal
files-git-left = left
files-git-right = right
about-files-git-down = Move the cursor one row down; in the diff, move the text one line.
about-files-git-up = Move the cursor one row up; in the diff, move the text one line.
about-files-git-enter = Go one column deeper: from an item to what it is, from a file to what changed in it.
about-files-git-next = Give the keyboard to the column on the right.
about-files-git-previous = Give the keyboard to the column on the left.
about-files-git-page-down = Move a whole screen down in the column that has the keyboard.
about-files-git-page-up = Move a whole screen up in the column that has the keyboard.
about-files-git-changes = List what the work tree has that the last commit does not.
about-files-git-history = List the commits, newest first.
about-files-git-refs = List the branches, the remotes' branches and the tags.
about-files-git-stashes = List what was stashed away.
about-files-git-remotes = List the remotes and where each one is.
about-files-git-file-history = List only the commits that changed the file under the cursor, across its renames.
about-files-git-blame = Show who last wrote each line of the file as the work tree has it; the same key goes back to the diff.
about-files-git-copy = Copy the id of the commit under the cursor.
about-files-git-open-version = Open the file as the commit had it, as a copy that cannot be changed.
about-files-git-fetch = Bring the remotes' new commits here, without changing any of yours. It never asks for a password.
about-files-git-terminal = Open a terminal at the repository's top folder.
about-files-git-left = Move the diff's text to the left.
about-files-git-right = Move the diff's text to the right, to read a long line.

# The archive rows of the file manager's menu (`ronoctua_files::menu`).
files-archive-test = test the archive
files-archive-convert = convert to another format…
files-archive-add = add what was copied to this archive
files-archive-extract = extract here, beside the archive
files-archive-remove = take out of the archive
files-does-archive-test = reads every member and checks it against its sums (7zz t, unrar t, bsdtar -t), through the queue
files-does-archive-convert = extracts into a hidden folder beside it and packs again in the format chosen on the sheet
files-does-archive-add = 7zz a: the copied files go in at the archive's top, encrypted with its password when it has one
files-does-archive-extract = takes the chosen members out into the archive's own folder; nothing there is replaced
files-does-archive-remove = 7zz d: rewrites the archive without them — it cannot be undone
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
# Why an archive's work did not happen (`ronoctua_archive::Why`).
files-archive-why-no-tool = { $name }: this session was not given the program this archive needs
files-archive-why-unsupported = { $name }: this format cannot do that — a tar has no password and changes only by conversion
files-archive-why-needs-password = { $name } is encrypted, and needs its password
files-archive-why-wrong-password = That password does not open { $name }
files-archive-why-corrupt = { $name } is damaged, or is not an archive
files-archive-why-exists = { $name } already exists; nothing was replaced
files-archive-why-unstartable = the program for { $name } could not be started
files-archive-why-failed = the work on { $name } did not finish
files-archive-why-stopped = the work on { $name } was stopped, and what it had written was removed
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
transfers-work-archive-add = add to the archive
transfers-work-archive-remove = take out of the archive
transfers-work-archive-test = test the archive
transfers-work-archive-convert = convert the archive
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
sidebar-lock = lock
sidebar-does-lock = unmounts what came out of it and locks the container (Encrypted.Lock); the power stays on
# Formatting with encryption (the format buffer, and its host).
# The format buffer's third question, as the modeline names its state.
state-encrypt = encryption
format-encrypt-title = Encrypt it?
format-encrypt-no = no — whoever holds the disk can read it
format-encrypt-yes = yes — it opens only with a passphrase
format-tech-encrypt-no = the filesystem is written in the clear
format-tech-encrypt-yes = LUKS2 · the passphrase is asked twice, on this window's own line
format-encrypt-differ = the two passphrases differ; nothing was written
format-encrypt-let-go = no passphrase was given; nothing was written
format-encrypted = encrypted
ask-format-encrypt-passphrase = A passphrase for the disk
ask-format-encrypt-again = The same passphrase again

# THE ICONS APP (`LEADER m i`) — `crate::icons`, `crate::icons_page`,
# `crate::icons_window`. One app for the three collections the three buffers
# used to be: Nerd Fonts, Lucide and Unicode.
cmd-icons-open = Icons
cmd-icons-window = Icons, in a window
cmd-icons-nerd-fonts = Browse the Nerd Fonts glyphs
cmd-icons-lucide = Browse the Lucide icons
cmd-icons-unicode = Browse the Unicode characters
cmd-icons-colour = The Lucide icon's colour
ask-icons-colour = The stroke's colour (a CSS colour, or currentColor)
# What the modeline names the app's window (`br.ronoctua.icons`).
mode-icons = icons
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

# THE CALCULATOR (`LEADER =`, `crate::calc`): the leader's group and its two
# doors, the window's name and the modeline's name for it.
group-calc = calculator
cmd-calc-open = The calculator, in the footer
cmd-calc-window = The calculator, in a window of its own
calc-title = Calculator
# The modeline's name for its window (`br.ronoctua.calc`).
mode-calc = calculator
calc-mode-basic = Basic
calc-mode-scientific = Scientific
calc-mode-convert = Convert
calc-sum = Sum
calc-amount = Amount
calc-empty = type a sum, or press the keys
calc-equals-exact = = { $value }
calc-equals-approx = ≈ { $value }
calc-history-title = History
calc-history-empty = No answer is kept yet: { $keys } keeps one.
calc-from = From
calc-to = To
calc-find = Find a unit — { $side }:
calc-find-none = No unit of this category has that name.
calc-category-currency = Currency
calc-rates-for = Converting currencies needs today's exchange rates from the European Central Bank.
calc-rates-asking = Asking { $host } for today's exchange rates…
calc-rates-of = Exchange rates of { $date }, from the European Central Bank.
calc-unkept-unparsed = { $file } will not parse, so this answer was not kept; the file was left as it is.
calc-unkept = This answer could not be kept in { $file }: { $why }
calc-float-unreached = The compositor could not be asked to move this window.
# The header's words, and what each one does under the pointer.
calc-equals = keep
about-calc-equals = Keeps the answer in the history and goes on from it; on an answer picked in the history, puts its sum back.
calc-mode = mode
about-calc-mode = Goes to the next mode, round the three: basic, scientific, convert.
calc-clear = clear
about-calc-clear = Empties the sum.
calc-copy = copy
about-calc-copy = Copies the answer — the one picked in the history, or the one on the line.
calc-history = history
about-calc-history = Picks an answer of the history, one older each time.
calc-take = take
about-calc-take = Takes the unit under the cursor for the side being chosen.
calc-ring-next = next category
about-calc-ring-next = Goes to the next category of units.
calc-ring-previous = previous category
about-calc-ring-previous = Goes to the previous category of units.
about-calc-from = Chooses the unit the amount is in: what is typed next finds it.
about-calc-to = Chooses the unit the amount is converted into: what is typed next finds it.
calc-swap = swap
about-calc-swap = Trades the two units: what was converted from is converted into.
calc-float = compact or normal
about-calc-float = Floats the window, compact, over the others — or puts it back in the layout among them.
calc-keys-narrow = The functions' keys need more room than this. Type them by name instead: sin, cos, √, ln…
# The keypad's legends: what is not a digit or the decimal mark.
calc-key-add = +
calc-key-subtract = −
calc-key-multiply = ×
calc-key-divide = ÷
calc-key-open = (
calc-key-close = )
calc-key-percent = %
calc-key-power = xʸ
calc-key-square = x²
calc-key-reciprocal = x⁻¹
calc-key-factorial = n!
calc-key-degree = °
calc-key-sin = sin
calc-key-cos = cos
calc-key-tan = tan
calc-key-asin = asin
calc-key-acos = acos
calc-key-atan = atan
calc-key-ln = ln
calc-key-log = log
calc-key-sqrt = √
calc-key-cbrt = ∛
calc-key-exp = exp
calc-key-pi = π
calc-key-e = e
calc-key-clear = C
calc-key-equals = =

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

# CHANNELS BY THEME, on the local catalogue (`player_catalog`): one source at
# a time, its themes and categories, and the listing a row of them opens.
player-about-catalog = Every source's channels, films and videos, read into this machine and filtered in an instant. A source is read again when it is shown past its freshness.
player-page-catalog = Listing
player-page-sources = Sources
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
player-catalog-page = Page
player-catalog-page-of = { $page } of { $pages }
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
player-catalog-layout = Shown as
player-catalog-layout-list = List
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
player-catalog-shown = { $from }–{ $to } of { $total }
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
player-adult-open = The adult catalogue is open until 15 minutes pass with no key and no click.
player-adult-lock = Lock it now
player-adult-locked-again = The adult catalogue is locked.
player-adult-wrong = That is not your password.
player-adult-no-service = This system has no ronoctua-unlock service, so the adult catalogue stays shut.
player-adult-shut = Locked
player-adult-unavailable = The password could not be checked.
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

# THE VIDEO CATALOGUE (LEADER v, video_metrics.rs and its window).
group-video-metrics = videos
mode-video-metrics = video metrics
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
brain-why-saver = The power saver was on

mode-security = security centre
# The file manager's menu row that checks a file with VirusTotal.
files-virustotal = check with VirusTotal
files-does-virustotal = sends its fingerprint (SHA-256) to VirusTotal, in the security centre's window; the file itself only if you say yes
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
# The view's keys, as the hint line names them (`security::OF_THE_VIEW`).
security-down = down
security-up = up
security-enter = do it
security-left = finger back
security-right = finger on
security-relax = no password
security-shields = shields
security-rule-add = add rule
security-rule-delete = remove
security-enrol = enrol
# THE SECURITY CENTRE (`LEADER s e`, `crate::security`): its leader group,
# its commands, its view's keys and questions, and its page.
group-security = security
cmd-security-open = Open the security centre
cmd-security-window = Open the security centre in a window
cmd-security-relax = Let the administrator act without a password for a while, or end that now
cmd-security-shields = Refuse every incoming connection, or stop refusing them
cmd-security-firewall = Switch your firewall on or off
cmd-security-check-password = Check whether a password is in known breaches
cmd-security-check-email = Check whether an e-mail address is in known breaches
cmd-security-check-file = Check a file with VirusTotal
cmd-security-down = Next row of the security centre
cmd-security-up = Previous row of the security centre
cmd-security-enter = Do what the row under the cursor offers
cmd-security-left = The previous finger to enrol
cmd-security-right = The next finger to enrol
cmd-security-rule-add = Add a firewall rule
cmd-security-rule-delete = Remove the rule or the finger under the cursor
cmd-security-enrol = Enrol the chosen finger
cmd-security-password = Your password, for the administrator
cmd-security-rule = A firewall rule, in words
cmd-security-vt-key = Keep your VirusTotal key
cmd-security-vt-forget = Forget your VirusTotal key
cmd-security-home = Treat the network that just came up as home
ask-security-password = Your password
ask-security-rule = Rule (allow tcp 8080 from 192.168.1.0/24 via wlan0 home)
ask-security-vt-key = Your VirusTotal API key
# The file manager's changes only the administrator may make (an owner, a
# group, permissions), through the same sudo door.
ask-files-root-password = Your password, for what only the administrator may change
ask-security-check-password = Password to check (only five characters of its fingerprint leave this machine)
ask-security-check-email = E-mail address to check (it is sent to XposedOrNot)
ask-security-check-file = File to check (its fingerprint is sent to VirusTotal)
hint-security-until = The administrator asks for no password until { $time }; a click opens where it ends
transfers-engine-scan = VirusTotal
transfers-work-scan = send a file to VirusTotal
echo-security-checking = Checking…
echo-security-file-checked = { $file }: { $verdict }
echo-security-finger-enrolled = Finger enrolled.
echo-security-finger-removed = Finger removed.
echo-security-finger-sudo-changed = Fingers for the administrator changed.
echo-security-firewall-applied = The firewall is as you asked.
echo-security-key-forgotten = The VirusTotal key is forgotten.
echo-security-key-kept = The VirusTotal key is kept.
echo-security-key-not-a-key = That is not a key VirusTotal takes.
echo-security-key-not-kept = The key could not be kept in your folder.
echo-security-nothing-to-delete = There is no rule or finger under the cursor.
echo-security-no-new-network = No network that came up is waiting to be placed.
# The action beside the watch's "a new network is treated as public".
notif-action-treat-as-home = Treat it as home
echo-security-relaxed = The administrator asks for no password until the time on the modeline.
echo-security-relax-ended = The administrator asks for the password again.
echo-security-rule-not-understood = That is not a rule: allow or deny, then tcp or udp, the ports, and optionally from, via and home, public or everywhere.
echo-security-shields-up = Every incoming connection is refused now.
echo-security-upload-queued = { $file } is on its way to VirusTotal; the transfers list shows how far.
security-title = Security
security-on = on
security-off = off
security-unknown = unknown
security-answer-no = No
security-answer-yes = Yes
security-busy = The administrator is being asked…
security-busy-enrol = Touch the reader when it asks.
security-heading-now = Now
security-heading-firewall = Firewall
security-heading-admin = Administrator
security-heading-finger = Fingerprint
security-heading-device = This machine
security-heading-privacy = Network privacy
security-heading-online = Online checks
security-heading-suggest = Suggestions
security-now-nothing = Nothing is off.
security-now-reading = Reading…
security-now-firewall-failed = The firewall failed
security-now-settings-unreadable = The firewall's settings cannot be read
security-now-firewall-off = Your firewall is off
security-now-sudo-open = No administrator password until
security-now-shields = Every incoming connection is refused
security-now-refused = { $count ->
    [one] One connection refused lately
   *[other] { $count } connections refused lately
  }
security-fw-switch = Your firewall
security-fw-network = Network on { $iface }
security-fw-profile-home = home: trusted
security-fw-profile-public = public: strangers around
security-fw-served = this machine serves it
security-fw-treat-as-home = Treat this network as home
security-fw-shields = Refuse every incoming connection
security-fw-shields-note = Raising them is at once; lowering them asks for your password, as every loosening does.
security-fw-log = Refused connections in the log
security-fw-log-off = not logged
security-fw-log-quiet = a few a minute
security-fw-log-chatty = many a minute
security-fw-rule-add = Add a rule
security-fw-blocked = Refused lately:
security-fw-blocked-row = { $proto } { $port } from { $source } on { $iface } ({ $count ->
    [one] once
   *[other] { $count } times
  })
security-fw-watch-silent = The brain is not answering, so the connections refused lately cannot be shown.
security-fw-ports = What this machine answers on:
security-fw-port-row = { $who }: { $proto } { $port }
security-fw-port-unknown = a program of another user
security-fw-port-container = container { $name }
security-fw-verdict-open = open: the firewall is off
security-fw-verdict-allowed = allowed by your rule
security-fw-verdict-partly = allowed only on some networks, or from some addresses
security-fw-verdict-refused = refused, unless the system opens it
security-fw-verdict-shielded = refused: every connection is
security-sudo-password = The administrator's password
security-sudo-asked = asked every time
security-sudo-not-asked-until = not asked until { $time }
security-sudo-end = Ask for it again now
security-sudo-open-for = Do not ask for { $minutes } minutes
security-sudo-note = It asks for your password first, and ends by itself at the time chosen, when the machine sleeps, and at the next start.
security-sudo-recent = This week:
security-finger-reader = Reader
security-finger-enrolled = enrolled
security-finger-enrol = Enrol
security-finger-ring-note = The arrows to the sides choose the finger; it asks for your password first, then for a few touches.
security-finger-sudo = Fingers for the administrator in a terminal
security-finger-sudo-note = Only in a terminal: here, and everywhere else, it is your password.
security-finger-ask-remove = Remove your { $finger }? It cannot be used again until it is enrolled again.
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
security-device-secure-boot = Secure Boot
security-device-secure-boot-enabled = on
security-device-secure-boot-disabled = off
security-device-secure-boot-setup-mode = in setup mode: keys can be changed
security-device-secure-boot-unsupported = not a UEFI start
security-device-tpm = Security chip (TPM)
security-device-tpm-absent = none
security-device-tpm-present = version { $version }
security-device-encryption = The system's disk
security-device-encrypted = encrypted
security-device-not-encrypted = not encrypted
security-device-hsi = Firmware security level
security-device-firmware-updates = Firmware updates
security-device-firmware-updates-count = { $count ->
    [0] none known
    [one] one available
   *[other] { $count } available
  }
security-device-firmware-note = Read only: firmware is not installed from here.
security-device-updates = Automatic updates
security-device-lock = Screen lock
security-device-lock-none = none: this system has no lock screen
security-device-autologin = Starting the machine
security-device-autologin-gate = the disk's passphrase is the only gate
security-privacy-mac = Hardware address on { $name }
security-privacy-mac-permanent = the device's own: networks can follow you
security-privacy-mac-preserve = as the device has it
security-privacy-mac-random = new every time
security-privacy-mac-stable = its own for this network
security-privacy-mac-explicit = one you chose
security-privacy-mac-default = the system's choice
security-privacy-dns = Encrypted DNS on { $name }
security-privacy-dns-yes = always
security-privacy-dns-opportunistic = when the server offers it
security-privacy-dns-no = never
security-privacy-dns-default = the system's choice
security-privacy-none = No network is up.
security-privacy-ssh = SSH server
security-privacy-ssh-on = something answers on port 22
security-privacy-note = These are read, not changed here: each connection's settings are the network's.
security-online-vt-key = VirusTotal key
security-online-key-kept = kept
security-online-key-none = none: paste one from your VirusTotal account
security-online-vt-forget = Forget the VirusTotal key
security-online-check-file = Check a file
security-online-file-note = Only its fingerprint (SHA-256) is sent to VirusTotal. A file it has never seen is sent only if you say yes.
security-online-check-email = Check an e-mail address
security-online-email-note = The address itself is sent to XposedOrNot.
security-online-email-clean = { $address } is in no known breach.
security-online-email-found = { $address } is in { $count ->
    [one] one breach
   *[other] { $count } breaches
  }: { $names }
security-online-check-password = Check a password
security-online-password-note = Only five characters of its fingerprint (SHA-1) leave this machine; the rest is compared here.
security-online-password-clean = That password is in no known breach.
security-online-password-found = That password was found { $count ->
    [one] once
   *[other] { $count } times
  } in breaches: do not use it.
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
security-vt-ask-upload = VirusTotal has never seen { $file }. Send the file itself?
security-vt-upload-warning = A file sent to VirusTotal is shared with its partners and can be downloaded by its paying subscribers. Do not send anything private.
security-vt-state-asking = asking…
security-vt-state-unknown = never seen, not sent
security-vt-state-sending = sending, { $percent }%
security-vt-state-pending = sent; waiting for the analysis
security-vt-verdict-clean = clean: none of { $engines } engines found anything
security-vt-verdict-suspicious = suspicious: { $found } of { $engines } engines
security-vt-verdict-malicious = malicious: { $found } of { $engines } engines
security-rule-word-allow = allow
security-rule-word-deny = deny
security-rule-word-from = from
security-rule-word-on = via
security-rule-word-home = home
security-rule-word-public = public
security-rule-word-everywhere = everywhere
security-rule-word-any = any
security-about-show = Goes to where this is
security-about-profile = Home or public: a public network lets nothing in that your rules for home would
security-about-log = How many refused connections go in the log
security-about-rule = Change this rule, in words; the delete key removes it
security-about-allow-home = Allows this port on home networks, with your password
security-about-allow-public = Allows this port on public networks too, with your password
security-about-finger = Removes this finger, after a question
security-about-finger-sudo = Lets a finger answer the administrator in a terminal, with your password
security-about-report = Opens the file's report on VirusTotal's page
security-about-url = Opens the page in your browser
security-about-answer = Answers the question
security-suggest-vpn = Use a VPN: import its file in the network view
security-suggest-tor = Go through Tor
security-suggest-tailnet = Reach your own machines through your tailnet
security-suggest-hibp = Have I Been Pwned, in your browser (no address goes with it)

# THE PERSON'S CONTAINERS (LEADER s o) — `containers.rs`, its words
# (`containers_words.rs`), its threads and its window. A status line, a name
# and a log line are podman's own words and are drawn as they came; a key is
# never text here: every sequence arrives as `{ $keys }`.
containers-title = Containers
group-containers = containers
mode-containers = containers
cmd-containers-open = Your containers: pods, compose, images, volumes and networks
cmd-containers-window = Your containers, in a window
containers-down = down
containers-up = up
containers-enter = open
containers-left = left
containers-right = right
containers-next-kind = next kind
containers-previous-kind = previous kind
containers-next-tab = next tab
containers-previous-tab = previous tab
containers-start-stop = start/stop
containers-restart = restart
containers-remove = remove
containers-logs = log
containers-shell = shell
containers-inspect = inspect
containers-pull = pull
containers-build = build
containers-login = log in
containers-prune = prune
containers-reveal = show values
cmd-containers-down = The next row
cmd-containers-up = The row above
cmd-containers-enter = Open the row; on a fact, copy it
cmd-containers-left = The column to the left, or back from the detail
cmd-containers-right = The column to the right, or the detail of the row
cmd-containers-next-kind = The next kind: containers, pods, compose, images, volumes, networks
cmd-containers-previous-kind = The previous kind
cmd-containers-next-tab = The detail's next tab
cmd-containers-previous-tab = The detail's previous tab
cmd-containers-start-stop = Start it, or stop it; a compose project goes up, or down after a question
cmd-containers-restart = Restart it
cmd-containers-remove = Remove it, after a question
cmd-containers-logs = Its log, as it grows
cmd-containers-shell = A shell inside it, in the terminal
cmd-containers-inspect = Everything podman knows of it, highlighted
cmd-containers-pull = Pull an image, on the queue
ask-containers-pull = Image to pull
cmd-containers-build = Build an image from a folder, on the queue
ask-containers-build = Folder to build
cmd-containers-login = Log in to a registry: its name and your user, then the password on a hidden line
ask-containers-login = Registry and user (quay.io alice)
cmd-containers-login-password = The password of the registry login being typed, on a hidden line
ask-containers-login-password = The registry's password
cmd-containers-prune = Remove what nothing uses, after a question
cmd-containers-reveal = Show the environment's values, or hide them again
containers-kind-containers = Containers
containers-kind-pods = Pods
containers-kind-compose = Compose
containers-kind-images = Images
containers-kind-volumes = Volumes
containers-kind-networks = Networks
containers-empty-containers = No containers. A pull puts an image here, and `podman run` or compose makes one.
containers-empty-pods = No pods.
containers-empty-compose = No compose projects. One appears when compose makes its containers.
containers-empty-images = No images.
containers-empty-volumes = No volumes.
containers-empty-networks = No networks.
containers-tab-overview = Overview
containers-tab-logs = Log
containers-tab-stats = Use
containers-tab-inspect = Inspect
containers-tab-ports = Ports
containers-tab-mounts = Mounts
containers-tab-env = Environment
containers-state-created = created
containers-state-running = running
containers-state-paused = paused
containers-state-exited = stopped
containers-state-stopping = stopping
containers-state-unknown = in a state podman did not name
containers-why-no-podman = Containers are not part of this installation: podman is not here.
containers-why-unstartable = podman could not be started.
containers-why-failed = podman could not do it; the system log says why.
containers-why-garbled = podman answered something that is not what was asked.
containers-why-not-found = It is not there any more.
containers-why-refused = The registry refused the login.
containers-why-too-large = podman's answer was too large to show.
containers-reading = Asking podman…
containers-working = { $what } { $name }: { $stage }
containers-pod-value = { $status } · { $count }
containers-project-value = { $count ->
        [one] { $up } of 1 running
       *[other] { $up } of { $count } running
    }
containers-image-unnamed = unnamed ({ $id })
containers-heading-members = Its containers
containers-row-name = Name
containers-row-image = Image
containers-row-state = State
containers-row-status = podman says
containers-row-created = Made
containers-row-started = Started
containers-row-finished = Ended
containers-row-exit = Ended with
containers-row-command = Runs
containers-row-restart = Restarts
containers-row-restarts = Restarted
containers-row-network = Network
containers-row-pod = Pod
containers-row-project = Compose project
containers-row-service = Service
containers-row-id = Id
containers-row-folder = Folder
containers-row-file = Compose file
containers-row-size = Size
containers-row-used-by = Used by
containers-row-platform = Made for
containers-row-driver = Driver
containers-row-mountpoint = Kept in
containers-row-subnet = Addresses
containers-row-dns = Names inside it
containers-used-by = { $count ->
        [0] no container
        [one] one container
       *[other] { $count } containers
    }
containers-network-value = { $network }: { $address }
containers-on = on
containers-off = off
containers-open = open
containers-logs-waiting = Waiting for the log…
containers-logs-empty = It has written nothing.
containers-logs-ended = The log ends here: podman stopped following it.
containers-stats-waiting = Waiting for podman's first count…
containers-stats-not-running = Only a running container uses the machine.
containers-stats-every = Counted again every { $seconds } seconds, while this is shown.
containers-stat-cpu = Processor
containers-stat-memory = Memory
containers-stat-memory-share = Of its memory limit
containers-stat-network = Network in / out
containers-stat-disk = Disk in / out
containers-stat-pids = Processes
containers-ports-none = It publishes no port: nothing outside it reaches it.
containers-port-label = { $published } to { $inside }
containers-port-local = this machine only
containers-port-open = open: the firewall is off
containers-port-allowed = let in
containers-port-home-only = let in at home only
containers-port-shields = refused: shields are up
containers-port-refused = refused by the firewall
containers-port-unknown = rules unreadable
containers-port-firewall = The firewall's rules
containers-ports-blocked = Someone on another machine cannot reach a port the firewall refuses; only this machine can. A rule of yours lets it in.
containers-ports-unknown = The firewall's rules could not be read, so what reaches these ports is not known.
containers-port-narrowed = let in from some addresses only
containers-ports-shields = Shields are up: the firewall drops every new connection from another machine, whatever the rules say, until they are lowered.
containers-ports-narrowed = A rule of yours lets in only the addresses or the connection it names; anyone else cannot reach these ports.
containers-ports-open = The firewall is off: anyone on a network this machine is on reaches these ports.
containers-inspect-env = Environment values
containers-row-nth = { $label } { $n }
containers-mounts-none = It mounts nothing of this machine's.
containers-mount-value = { $source } · { $how } · { $kind }
containers-mount-writable = it can write
containers-mount-read-only = read only
containers-env-none = Its environment is empty.
containers-env-values = Values
containers-env-hidden = hidden
containers-env-shown = shown
containers-env-mask = ••••••
containers-ask-remove = Remove { $name }?
containers-ask-remove-running = It is running: it is stopped first.
containers-ask-remove-pod = Its containers are removed with it.
containers-ask-remove-used = Containers made from it are removed with it.
containers-ask-prune-system = Remove every stopped container and pod, every network no container uses, every image nothing names and the build cache?
containers-ask-prune-images = Remove every image nothing names?
containers-ask-prune-volumes = Remove every volume no container uses — and what is in them?
containers-ask-down = Take { $name } down? Its containers are stopped and removed; its volumes stay.
containers-answer-no = No
containers-answer-yes = Yes
containers-did-start = Starting
containers-did-stop = Stopping
containers-did-restart = Restarting
containers-did-pause = Pausing
containers-did-unpause = Resuming
containers-did-remove = Removing
containers-did-prune = Pruning
containers-echo-asked = { $what } { $name }…
containers-echo-done = { $what } { $name }: done.
containers-echo-refused = { $what } { $name }: { $why }
containers-echo-queued = { $what } { $name } went to the queue — { $keys } shows it.
containers-no-queue = The transfer queue is not answering, so nothing was queued.
containers-refused-not-here = That is not something one does to { $kind }.
containers-refused-not-running = Only a running container has a shell to open.
containers-refused-no-file = compose did not say where this project's file is.
containers-refused-no-folder = { $path } is not a folder.
containers-refused-no-containerfile = { $path } has no Containerfile or Dockerfile to build.
containers-refused-login-words = Type the registry and your user on it, a space between them: quay.io alice.
containers-refused-no-login = No login is waiting for a password: start with the registry and the user.
containers-echo-logging-in = Logging in to { $registry } as { $user }…
containers-echo-logged-in = Logged in to { $registry } as { $user }.
containers-echo-login-refused = The login to { $registry } did not go through: { $why }
# The queue's words for the containers' jobs (`ronoctua-transfers`).
transfers-engine-container = containers
transfers-work-container-pull = pull the image
transfers-work-container-build = build the image
transfers-work-container-compose-up = start the project
transfers-work-container-compose-down = take down the project
transfers-work-container-compose-pull = pull the project's images

# THE BELONGINGS (`LEADER s v`, `crate::belongings`): the person's
# configuration, kept encrypted in a private repository they name.
group-belongings = belongings
# The modeline's name for its window (`br.ronoctua.belongings`).
mode-belongings = belongings
belongings-title = Belongings
cmd-belongings-open = Belongings: the configuration kept in your own repository
cmd-belongings-window = Belongings, in a window
cmd-belongings-sync = Sync the belongings now: asks the passphrase
cmd-belongings-up = Previous row
cmd-belongings-down = Next row
cmd-belongings-less = The previous value of the row
cmd-belongings-more = The next value of the row
cmd-belongings-enter = What the row under the cursor does
cmd-belongings-keep-local = Keep this machine's side of the conflict: it goes over the repository's
cmd-belongings-take-remote = Take the repository's side of the conflict: this machine's is set aside
cmd-belongings-keep-both = Keep both sides: the repository's lands beside this one, with the other machine's name
cmd-belongings-apply = Apply the decisions and finish the sync
cmd-belongings-abandon = Drop the prepared sync: nothing is written, and the passphrase is forgotten
cmd-belongings-all-local = Every conflict: keep this machine's side
cmd-belongings-all-remote = Every conflict: take the repository's side
cmd-belongings-repository = Set the repository's address
cmd-belongings-add = Keep another file or folder of your home
cmd-belongings-auth = Reach the repository by an SSH deploy key, or by a token
cmd-belongings-keygen = Make this machine's SSH deploy key
cmd-belongings-copy-key = Copy the deploy key's public half, to give it to the repository
cmd-belongings-token = Keep a token for the repository, readable by you alone
cmd-belongings-user = The user name the repository's host asks for
cmd-belongings-passphrase = Change the passphrase: asks the one in use, then the new one twice
cmd-belongings-period = How often you are reminded to sync
belongings-up = up
belongings-down = down
belongings-less = less
belongings-more = more
belongings-enter = do
belongings-keep-local = keep mine
belongings-take-remote = take theirs
belongings-keep-both = keep both
belongings-sync = sync
ask-belongings-sync = Passphrase of the belongings
ask-belongings-repository = The repository's address: git@host:path, ssh://… or https://…
ask-belongings-add = A file or folder of your home to keep
ask-belongings-token = The token, as the repository's host gave it
ask-belongings-user = The user name the host asks for
ask-belongings-passphrase = The passphrase in use
ask-belongings-passphrase-new = The new passphrase
ask-belongings-passphrase-again = The new passphrase again
ask-belongings-sync-again = The passphrase again — a new repository keeps it for good
about-belongings-conflict = Shows what differs between the two sides, when both are text
about-belongings-rule = Takes it out of what is kept on every machine you sync
about-belongings-kept = Stops keeping it: the file stays where it is
about-belongings-excluded = Keeps it again
about-belongings-never = Keeps it anyway: it goes to every machine you sync
belongings-status = Status
belongings-repository = Repository
belongings-none-yet = none yet
belongings-last-sync = Last sync
belongings-never-here = never on this machine
belongings-next = Next reminder
belongings-due-now = now
belongings-no-reminder = no reminders
belongings-kept-now = Kept now
belongings-files-weigh = { $n ->
    [one] one file, { $size }
   *[other] { $n } files, { $size }
    }
belongings-last-run = Last run
belongings-first-run = This machine has not synced with this repository yet. The first sync restores what the repository keeps; where a file here differs, the repository's side starts chosen.
belongings-prefs-refused = belongings.json does not read as settings, so nothing is written over it: correct it by hand, or delete it.
belongings-no-places = This session has no home or no runtime folder, so the belongings have nowhere to live.
belongings-outcome-synced = In step: { $pulled } written here, { $pushed } sent, { $deleted } deleted.
belongings-outcome-waiting = { $n ->
    [one] Waiting for your decision on one conflict.
   *[other] Waiting for your decisions on { $n } conflicts.
    }
belongings-outcome-abandoned = The prepared sync was dropped.
belongings-prepared-gone = A sync was prepared here and is gone — the session ended since — so nothing waits for you: sync again.
belongings-outcome-keyed = A deploy key was made.
belongings-outcome-rekeyed = The passphrase was changed.
belongings-large = { $name } weighs { $size }: every sync carries it.
belongings-unreadable = { $n ->
    [one] One file here could not be read, and was left as it is on both sides.
   *[other] { $n } files here could not be read, and were left as they are on both sides.
    }
belongings-unwritten = { $n ->
    [one] One file from the repository could not be written here; the next sync tries again.
   *[other] { $n } files from the repository could not be written here; the next sync tries again.
    }
belongings-sync-heading = Sync
belongings-sync-now = Sync now
belongings-apply = Apply the decisions
belongings-decided-of = { $n } of { $of } decided
belongings-abandon = Drop this sync
belongings-waiting-note = The sync is prepared and waits for you: nothing here was written yet.
belongings-plan-waits = A sync is prepared and waits for your decisions: apply it or drop it first.
belongings-conflicts = { $n ->
    [one] One conflict
   *[other] { $n } conflicts
    }
belongings-conflicts-note = Changed here and in the repository since the last sync. Decide each: keep this machine's, take the repository's, or — for what you added — keep both.
belongings-all-keep-local = All: keep this machine's
belongings-all-take-remote = All: take the repository's
belongings-undecided-left = { $n ->
    [one] One conflict has no decision yet.
   *[other] { $n } conflicts have no decision yet.
    }
belongings-undecided = undecided
belongings-chosen = chosen
belongings-sides = here: { $here } · repository: { $there }, changed on { $machine }
belongings-side = { $size }, { $when }
belongings-side-deleted = deleted
belongings-decided-keep-local = keep this machine's
belongings-decided-take-remote = take the repository's
belongings-decided-keep-both = keep both
belongings-decide-keep-local = Keep this machine's
belongings-decide-take-remote = Take the repository's
belongings-decide-keep-both = Keep both
belongings-diff-title = Difference in { $name }
belongings-diff-note = The lines that begin with - are this machine's; the ones that begin with + are the repository's.
belongings-kept = What is kept
belongings-what-leaks = Everything is encrypted with your passphrase before it leaves this machine. The repository shows how many files there are, their sizes and when each sync happened; never their names, their places or what is in them.
belongings-added-by-you = added by you
belongings-added-elsewhere = added on another machine
belongings-taken-out = taken out by you
belongings-add = Keep something else
belongings-large-note = What you add is kept whole, folders and links included. Past { $size }, the page says so: every sync carries it.
belongings-never = Never kept
belongings-auth-ssh = an SSH deploy key
belongings-auth-token = an HTTPS token
belongings-access = Access
belongings-auth = Reached by
belongings-deploy-key = Deploy key
belongings-copy-it = copy the public key
belongings-make-it = make one
belongings-deploy-note = Give this public key write access to the repository, as a deploy key. The private half stays on this machine, readable by you alone, and is never kept in the belongings.
belongings-keygen-note = This machine has no deploy key yet. Making one takes a moment on the queue; then its public half appears here to copy.
belongings-token = Token
belongings-token-is-kept = kept, readable by you alone
belongings-user = User name
belongings-token-note = The token is kept in a file only you can read, and reaches git through its path alone — never on a command line.
belongings-passphrase = Passphrase
belongings-change-passphrase = Change the passphrase
belongings-passphrase-note = The passphrase opens one small file, and that file opens the rest: changing it encrypts that file again, on every machine at once. Nothing recovers a passphrase that is lost.
belongings-period = Reminder
belongings-remind-every = Remind me to sync
belongings-period-days = { $n ->
    [one] every day
   *[other] every { $n } days
    }
belongings-period-never = never
belongings-needs-key = There is no deploy key yet: make one under Access, and give it write access to the repository.
belongings-needs-token = There is no token yet: keep one under Access.
belongings-nothing-prepared = There is no prepared sync.
belongings-no-conflict-here = The cursor is not on a conflict.
belongings-both-only-added = Both sides can be kept only for what you added yourself.
belongings-no-difference = The difference of this file is not here any more; sync again to see it.
belongings-not-text = This file is not text, so there is no difference to show: decide by its size and time.
belongings-key-already = This machine has a deploy key already.
belongings-no-key-yet = There is no deploy key yet.
belongings-passphrase-first = The first sync sets the passphrase: there is none to change yet.
belongings-empty-secret = Nothing was typed.
belongings-token-kept = The token is kept.
belongings-passphrases-differ = The two new passphrases differ, so nothing was changed.
belongings-first-passphrases-differ = The two passphrases differ, so nothing was sent.
belongings-not-written = A file of the belongings could not be written; the system log says why.
belongings-never-kept = That is never kept: it is how this machine reaches the repository.
belongings-outside-home = Only what is in your home can be kept.
belongings-now-kept = { $name } is kept in the belongings now.
belongings-stopped-keeping = { $name } is no longer kept in the belongings; it stays where it is.
belongings-queued = Sent to the queue: { $what }.
belongings-op-prepare = the sync
belongings-op-apply = the sync, with your decisions
belongings-op-abandon = dropping the sync
belongings-op-keygen = making the deploy key
belongings-op-rekey = the new passphrase
belongings-invalid-empty = Nothing was typed.
belongings-invalid-credentials-in-address = The address carries a user and a password: give the address alone, and the key or the token under Access.
belongings-invalid-plain-http = An http:// address sends the access in the clear: use https:// or SSH.
belongings-invalid-not-remote = A local path or file:// is not a remote repository.
belongings-invalid-unknown = The address is git@host:path, ssh://… or https://….
belongings-refused-no-repository = No repository is set: give its address under Status.
belongings-refused-no-passphrase = The passphrase was not there, or somebody else could read its file; it was not used.
belongings-refused-wrong-passphrase = That passphrase does not open these belongings.
belongings-refused-needs-access = The repository wants access this machine was not given: a deploy key or a token, under Access.
belongings-refused-refused = The repository refused this machine's key or token.
belongings-refused-unreachable = The repository could not be reached.
belongings-refused-raced = Another machine kept syncing while this one did; try again.
belongings-refused-not-ours = That repository is not a belongings repository, or is from a newer version.
belongings-refused-empty = The repository is empty: the first sync sets the passphrase.
belongings-refused-corrupt = Something in the repository does not decrypt or does not read.
belongings-refused-undecided = A conflict has no decision yet.
belongings-refused-no-session = There is no prepared sync to apply.
belongings-refused-no-tools = This installation does not have git or ssh-keygen.
belongings-refused-failed = A file here could not be read or written; the system log says which.
belongings-refused-busy = Another run of the belongings is going on: wait for it to end.
belongings-reason-preference = your settings
belongings-reason-programs = the programs you installed
belongings-reason-draft = a draft, or a running program's number
belongings-reason-generated = made again from your settings on each machine
belongings-reason-access = how this machine reaches the repository
belongings-reason-own-state = what the belongings keep about themselves
belongings-reason-identity = the machine's own identity: two alike would collide
belongings-reason-machine = this machine's own state
belongings-reason-history = a history of this machine
belongings-reason-cache = a cache, made again when needed
# The file manager's menu row (`C-m`, `k`).
files-belongings = keep in the belongings, or stop
files-does-belongings = adds it to what the belongings keep in your repository and put back on a new machine; when it is kept already, takes it out — the file stays where it is
# The transfer queue's words for a sync.
transfers-engine-belongings = belongings
transfers-work-belongings = belongings
# The installer's fifteenth question.
inst-belongings = Belongings repository
inst-belongings-help = Optional. The private repository your configuration is kept in, encrypted: git@host:path, ssh://… or https://…, never with a password in it. Its first sync after the installation puts your settings back.
inst-step-belongings = Write the belongings' repository into your home

# A lookup or a directory whose host consent.json does not let: nothing
# was sent. The engine says it as the consent row (`Trouble::Off`); this is
# the sentence of the refusal itself, where it is said apart from that row.
player-trouble-lookup-not-let = { $service } was not asked: its host is not let.
player-trouble-directory-not-let = { $source } was not asked: its host is not let.
# A web app's page whose host consent.json would not let (`Refusal::NotLet`).
software-refusal-not-let = Its host is not let, so nothing was sent: a catalogue is let by its row under Sources, a web app's address by typing it.
software-consent-flathub = Flathub's search, its programs' pages and their pictures.
software-consent-fdroid = F-Droid's index, its icons and the packages it serves.
software-consent-appimagehub = The AppImage catalogue, and the GitHub releases it points at.
software-consent-dashboard = The icon index a web app's picture is chosen from.

# THE ROW THAT ASKS FOR A HOST (`consent_rows.rs`), the same in every app:
# what pressing it does, what it is for, and why a press was not kept
# (`ronoctua_consent::Unkept`).
consent-ask = Ask { $hosts }
consent-hosts-and = { $hosts } and { $last }
consent-note = { $what } Pressing allows it.
consent-allow = allow
about-consent-allow = Lets the sites the row names be asked — what pressing the row does. It is kept in consent.json, for every app.
consent-unkept-no-home = There is no home folder to keep the permission to ask { $hosts } in.
consent-unkept-unparseable = The permission file (consent.json) does not parse; it was left as it is, and { $hosts } was not let.
consent-unkept-unwritable = The permission to ask { $hosts } could not be written.
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
# What the modeline names the search's windows (`br.ronoctua.fast-access`,
# and the one over everything).
mode-fast-access = search
mode-fast-access-float = search
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
fast-access-no-shell = This shell does not know its own program, so it cannot open a window.
fast-access-unopened = Nothing opens { $what }.
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

# THE RANKING OF THE VIDEO CATALOGUE, a page of the player (LEADER p k,
# player_ranking.rs). Titles and channel names are data.
player-page-ranking = Videos by score
player-page-ranked-video = This video
cmd-player-ranking = Videos by score: the video catalogue's best first, a click from the queue
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
player-ranking-play-now = Play now
player-ranking-add = Add to the queue
player-ranking-kiosk = Watch in Chrome
player-ranking-download = Download
player-ranking-hide = Hide from the ranking
player-ranking-delete = Delete from the catalogue
ask-player-ranking-delete = Delete “{ $title }” from the video catalogue for good? It never comes back, not even in a scan.
player-ranking-hidden = Hidden: { $title }. The video catalogue's hidden videos bring it back.
player-ranking-deleted = Deleted for good: { $title }
player-ranking-not-changed = { $title } was not changed: { $why }
player-ranking-downloading = Downloading { $title }; { $keys } shows the transfers.
player-ranking-downloading-plain = Downloading { $title }.
player-ranking-not-downloaded = The transfer queue did not take { $title }: it is not running.
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

mode-clipboard = clipboard

# The resizer's ready sizes and its memory (`resizer.rs`): `p` walks the
# rooms of the commonest screens, and a resizer turned on again starts at
# the size it was left at.
resizer-preset = ready size
about-resizer-preset = The next ready size of a common screen that fits this one.
resizer-preset-back = smaller
about-resizer-preset-back = The ready size before this one.
resizer-no-preset = no ready size fits this screen
echo-capture-outside-the-room = Nothing was captured: the rectangle lay wholly under the resizer's curtains, outside the room it leaves.

# The screens (`display.rs`): the vertical arrangement, the render depth, a
# dark screen's filed values, and every refusal named.
display-rise = put above
display-sink = put below
cmd-display-rise = The screen under the cursor up, above its neighbour
cmd-display-sink = The screen under the cursor down, below its neighbour
display-depth = Colour depth
display-depth-bits = { $bits } bits per colour
display-depth-asked = Colour depth asked for
display-dark-shows-filed = The compositor is not drawing on this screen: what each row shows is what is filed for it, and comes back when it lights.
display-refused-compositor = the compositor answered with an error
display-refused-unreadable = the screens' file will not parse, and writing would replace it — fix or remove it
display-refused-panel = the panel would not take the brightness
display-refused-monitor = the monitor did not answer

# The power and Bluetooth refusals that had no name, named: the daemon's
# own words go to the journal, never to the line (`power_words.rs`,
# `bluetooth_words.rs`).
power-refused-timed-out = got no answer in time
power-refused-failed = could not be done
power-refused-no-bus = could not reach the system bus
bluetooth-refused-timed-out = it got no answer in time
bluetooth-refused-no-bus = the system bus could not be reached

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
tunnel-refused-no-daemon = its daemon is not running
tunnel-refused-authority = its socket is not this session's to use
tunnel-refused-unit = its service would not start or stop
tunnel-refused-not-written = the file could not be written
tunnel-refused-config = Tor would not read its configuration
tunnel-refused-daemon = the daemon said no
tunnel-refused-other = it did not work
echo-tunnel-refused-line = Tor would not read this line: { $line }

# The file manager's state on the modeline (`files::View::state`): a question
# about who may touch a file, drawn in warning.
state-permission = permission

# The clipboard's window (`clipboard_window.rs`): the second screen of the
# footer's history, followed over the control socket.
group-clipboard = clipboard
cmd-clipboard-window = The clipboard's history in a window
clipboard-window-waiting = The history is the footer's, and it is on its way.
clipboard-window-no-footer = The footer is not running, and the history is kept there: nothing to show.
clipboard-no-room = There is no room left on screen to draw it: scroll back up, or give the list more lines.
clipboard-act-stale = The history changed before that key arrived, so nothing was done: look again and press it again.

# The lifeline's "ask an AI", through Claude Code (`lifeline.rs`): what would
# leave is gathered, shown in a terminal, and asked about with "no" first.
lifeline-ai-gathering = gathering what would go: the unit, how it ended, its journal and its core dump
lifeline-ai-no-report = the report could not be written, and nothing left this machine
lifeline-ai-shown = what would go to Claude Code is open in a terminal; nothing has left yet — send it?
lifeline-ai-no = no, keep it here
lifeline-ai-yes = yes, ask Claude Code
lifeline-ai-kept = nothing left this machine, and the report is gone
lifeline-ai-opened = Claude Code is open in a terminal, with the report
lifeline-ai-no-claude = Claude Code is not on this machine; it comes with the development profile's agents
# THE PROMPT Claude Code starts with, in the person's language. `{ $path }`
# is the report this line wrote, readable by this person alone.
lifeline-ai-prompt = The interface of my computer (the systemd user unit { $unit }) failed. The file { $path } has the unit, how it ended, the last lines of its journal and its core dump. Read it, tell me what most likely happened, and what I can do about it.
lifeline-report-title = What happened to the interface
lifeline-report-unit = Unit
lifeline-report-result = How it ended
lifeline-report-journal = The last lines of its journal
lifeline-report-core = Its core dump
lifeline-report-nothing = (nothing)

example-code = A piece of code
# The network view: the gateway's round trip, measured on demand.
network-latency = round trip to the router
network-latency-measure = measure
network-latency-measuring = measuring…
network-latency-ms = { $fastest } to { $slowest } ms ({ $answered } of { $asked })
network-latency-silent = no answer
network-latency-refused = could not measure: { $error }
# The names of the machines on the network, from systemd-resolved.
network-lan-names-refused = the names are unknown: { $why }
lan-resolve-why-no-bus = the system bus could not be reached
lan-resolve-why-no-resolved = systemd-resolved is not running
lan-resolve-why-failed = systemd-resolved could not answer
# A VPN whose file asks for a user and a password.
ask-network-vpn-user = User for this VPN
ask-network-vpn-password = Password for this VPN
echo-network-vpn-stored = { $network } will come up without asking
echo-network-vpn-store-failed = the VPN's password was not stored: { $reason }
# A phone's internet, over Bluetooth.
bluetooth-fact-internet = uses its internet
bluetooth-about-internet = The phone shares its connection over Bluetooth. The link is NetworkManager's, and the network view lists it too, under the same interface.
bluetooth-internet-through = yes, through { $interface }
echo-bluetooth-internet-on = going out through { $device } now
echo-bluetooth-internet-off = no longer going out through { $device }
echo-bluetooth-internet-failed = could not use the internet of { $device }: { $reason }
# A place's currencies, chosen on screen.
locale-currency = Currency
locale-currency-pair = { $from } in { $into }
locale-currency-none = none chosen
locale-not-a-currency-pair = That is not two currencies the rates know, three letters each, like EUR BRL.
ask-locale-currency = Its currency and the one it is worth in, like EUR BRL
# A question that offers answers for what is typed.
line-offers = { $keys }: { $offers }
about-line-offer = Takes the first answer offered for what is typed.
studio-trouble-no-face = A text in the scene has no typeface to be drawn with.
studio-trouble-rehearsal = This is the development loop's studio: it rehearses, and writes no photo, recording or stream.
camera-virtual-effect = It goes through
camera-virtual-effect-preview = the preview's effect
# A phone's link in the network view, and the network crate's own refusals.
network-fact-phone = phone
network-refusal-no-phone = NetworkManager has no Bluetooth device for this phone: it is not paired, or it does not share its connection
network-refusal-no-vpn = there is no VPN by that name any more
network-refusal-not-vpn = that profile is not a VPN

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
player-trouble-tag-chapters-only-matroska = A chapter's title is written only in Matroska, and this file is { $container }.
player-trouble-tag-remux-would-drop = This file is too big to be written in place, and the copy that writes it would lose items it keeps beyond these fields. Its fields stay as they are.
player-origin-matroska-track-of = Matroska tags of stream { $index } · { $kind }
player-origin-matroska-track-lang = Matroska tags of stream { $index } · { $kind } · { $lang }
player-origin-matroska-track-uid = Matroska tags of the track whose UID is { $uid }
