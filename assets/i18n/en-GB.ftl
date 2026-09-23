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
cmd-glyphs-nerd-fonts = Browse glyphs
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

mode-example = example
buffer-glyphs = glyphs
mode-glyphs = glyphs
buffer-lucide = lucide
mode-lucide = lucide
buffer-memory = memory
mode-memory = memory
buffer-transfers = transfers
mode-transfers = transfers
buffer-format = format
mode-format = format
buffer-unicode = unicode
mode-unicode = unicode
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
notif-action-copy-path = Copy the path
notif-action-memory-panel = Open the memory panel

# THE FILTERS THE DISTRO SHIPS WITH. The manager lists them by these names.
notif-rule-important-gets-room = Something important gets room
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
notif-type-in-the-footer = Typing happens in the footer — { $keys } opens it there.

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
notif-act-in-the-footer = This one runs from the footer: { $keys }, then open the notification.
notif-act-no-footer = The footer is not listening; the action went nowhere.

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
echo-no-buffer = The glyph browser was not built
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
group-window = window/buffer
group-terminal = terminal
group-workspace = workspace
cmd-workspace-overview = Workspace overview
cmd-window-float = Toggle floating
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
cmd-notif-filter-typed = Answer the filter sheet's question
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

cmd-help-shortcuts = Shortcut manual
group-help = help
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
cmd-wm-trailmark-jump = Jump to a window on the trail by letter
cmd-wm-trail-new = Start a new trail
cmd-wm-trail-next = Use the next trail
cmd-wm-trail-previous = Use the previous trail
cmd-wm-trail-clear = Empty this trail
cmd-wm-trail-delete = Delete this trail

cmd-wm-jump = Jump to a window by typing the letter on it
cmd-wm-jump-tiling = Jump to a tiled window by letter
cmd-wm-jump-everywhere = Jump to any window at all by letter
cmd-wm-jump-container = Jump to a container by letter
cmd-wm-jump-workspaces = Jump to a workspace by letter
cmd-wm-jump-floating = Jump to a floating window by letter

cmd-wm-overview = See the whole workspace at once
cmd-wm-overview-all = See every workspace at once
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
cmd-wm-scratchpad-jump = Jump to a window in the drawer by letter

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
glyphs-search = Type to search for a Nerd Fonts glyph
# The word the browser WRITES into the search field — see `unicode-prefix-*`.
glyphs-prefix-block = block
glyphs-prefix-tag = tag
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

# --- The glyph card (Nerd Fonts browser) -----------------------------------
glyphs-sheet-codepoint = Codepoint
glyphs-sheet-decimal = Decimal
glyphs-sheet-block = Block
glyphs-sheet-set = Set
glyphs-sheet-token = Icon token
glyphs-sheet-tray = Assigned to an app
glyphs-sheet-tags-pt = Tags (pt)
glyphs-sheet-tags-en = Tags (en)
glyphs-sheet-help = ↑↓ format · Enter copies · ←→ other glyph · Esc returns

# The NINE copy formats. They have been commands since milestone 7 and never
# had a key — nobody noticed because nothing could reach them.
glyphs-sheet-copied = ✓ copied
glyphs-sheet-copy-failed = ✗ could not copy — see the log
cmd-glyphs-copy-name = Name
cmd-glyphs-copy-glyph = Glyph
cmd-glyphs-copy-css = CSS class
cmd-glyphs-copy-hex = Hexadecimal
cmd-glyphs-copy-utf16 = UTF-16
cmd-glyphs-copy-escape = Unicode escape
cmd-glyphs-copy-html = HTML entity
cmd-glyphs-copy-rust = Rust literal
cmd-glyphs-copy-token = Design system token
cmd-glyphs-filter-set = Filter by set
cmd-glyphs-filter-block = Filter by block
cmd-glyphs-filter-tag = Filter by tag
cmd-glyphs-filter-token = Only design system tokens
cmd-glyphs-filter-app = Only those assigned to an app
cmd-glyphs-filter-clear = Clear the filter
cmd-glyphs-filter-panel = Choose the set
cmd-glyphs-assign = Assign to an app id

# --- The source sets, in the filter panel ----------------------------------
#
# A brand name is not translated; what describes one is — which is why these
# are keys and not data. They must be the SAME names the `SETS` table in
# gen-glyphdb.py turns into search tags: `browser-filters` requires that the
# label you read here finds, typed into the search, the whole set.
glyphs-set-all = All
glyphs-set-cod = Codicons
glyphs-set-custom = Custom
glyphs-set-dev = Devicons
glyphs-set-extra = Extra
glyphs-set-fa = Font Awesome
glyphs-set-fae = Font Awesome Extension
glyphs-set-iec = IEC Power
glyphs-set-indent = Indentation
glyphs-set-indentation = Indentation
glyphs-set-linux = Font Logos
glyphs-set-md = Material Design
glyphs-set-oct = Octicons
glyphs-set-pl = Powerline
glyphs-set-ple = Powerline Extra
glyphs-set-pom = Pomicons
glyphs-set-seti = Seti-UI
glyphs-set-weather = Weather

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
echo-copied-value = Copied: { $value }
echo-copy-failed = Could not copy — see the log
echo-qr-large = The content is too large for a QR code
echo-copy-only-text = For now only text can be copied again
clipboard-image = Image
clipboard-unreadable = Copied, and unreadable here: { $kind }

# --- The application launcher (LEADER a) -----------------------------------
cmd-apps-open = Open application
apps-empty = No applications found

# --- The Unicode symbol browser (LEADER i u) -------------------------------
cmd-unicode-symbols = Browse Unicode symbols
unicode-search = Type to search for a Unicode symbol
# The words the browser WRITES into the search field. Typed, so lower case and
# unaccented; the parser goes on taking every language's, whichever is set.
unicode-prefix-symbols = symbols
unicode-prefix-family = family
unicode-prefix-block = block
unicode-prefix-name = name
unicode-prefix-group = group
unicode-prefix-subgroup = subgroup
unicode-prefix-tone = tone
cmd-unicode-copy-character = Character
cmd-unicode-copy-name = Official name
cmd-unicode-copy-hex = Hexadecimal
cmd-unicode-copy-decimal = Decimal
cmd-unicode-copy-escape = Unicode escape
cmd-unicode-copy-html = HTML entity
cmd-unicode-copy-rust = Rust literal
cmd-unicode-copy-utf16 = UTF-16
cmd-unicode-copy-utf8 = UTF-8 bytes
cmd-unicode-filter-family = Filter by family
cmd-unicode-filter-block = Filter by block
cmd-unicode-filter-category = Filter by category
cmd-unicode-filter-symbols = Symbols only
cmd-unicode-filter-clear = Clear the filter
cmd-unicode-filter-panel = Choose the family

# The seven families in the panel. The names are the SAME ones the generator
# turns into search tags — `symbols-filters` requires that the label you read
# finds, typed into the search, the whole family.
unicode-family-all = All
unicode-family-symbol = Symbol
unicode-family-letter = Letter
unicode-family-number = Number
unicode-family-mark = Mark
unicode-family-punctuation = Punctuation
unicode-family-other = Other

unicode-sheet-codepoint = Codepoint
unicode-sheet-decimal = Decimal
unicode-sheet-family = Family
unicode-sheet-category = Category
unicode-sheet-block = Block
unicode-sheet-decomposition = Decomposition
unicode-sheet-aliases = Also called
unicode-sheet-width = Width
unicode-width-zero = Combining mark — the dotted support is not part of it
unicode-width-double = Takes two cells
unicode-sheet-help = ↑↓ format · Enter copies · ←→ other · Esc returns
unicode-sheet-copied = ✓ copied
unicode-sheet-copy-failed = ✗ could not copy — see the log

# The emoji family and its NINE groups. The groups only show up in the panel
# when emoji is selected — they are sub-filters of it. The names are the SAME
# ones the generator turns into search tags.
unicode-family-emoji = Emoji
unicode-group-faces = Smileys & emotion
unicode-group-people = People & body
unicode-group-nature = Animals & nature
unicode-group-food = Food & drink
unicode-group-travel = Travel & places
unicode-group-activities = Activities
unicode-group-objects = Objects
unicode-group-symbols = Symbols
unicode-group-flags = Flags
unicode-sheet-group = Group
unicode-sheet-subgroup = Subgroup
cmd-unicode-filter-group = Filter by group

# === Lucide icon browser — `LEADER i l` ====================================
cmd-lucide-icons = Browse Lucide icons
lucide-search = Type to search for a Lucide icon
# The words the browser WRITES into the search field — see `unicode-prefix-*`.
lucide-prefix-category = category
lucide-prefix-name = name

cmd-lucide-copy-svg = SVG
cmd-lucide-copy-jsx = JSX
cmd-lucide-copy-vanilla = HTML (lucide.js)
cmd-lucide-copy-react = React
cmd-lucide-copy-vue = Vue
cmd-lucide-copy-svelte = Svelte
cmd-lucide-copy-preact = Preact
cmd-lucide-copy-solid = Solid
cmd-lucide-copy-angular = Angular
cmd-lucide-copy-font = Icon font
cmd-lucide-copy-name = Name
cmd-lucide-copy-component = Component
cmd-lucide-copy-character = Character
cmd-lucide-copy-hex = Codepoint
cmd-lucide-copy-path = Path data
cmd-lucide-copy-uri = Data URI
cmd-lucide-copy-base64 = Base64
cmd-lucide-copy-css = CSS rule
cmd-lucide-copy-sprite = Sprite
cmd-lucide-filter-category = Filter by category
cmd-lucide-filter-clear = Clear the filter
cmd-lucide-filter-panel = Pick a category

lucide-adjust-size = Size
lucide-adjust-stroke = Stroke width
lucide-adjust-absolute = Absolute stroke width
lucide-adjust-color = Color
lucide-adjust-cap = Line caps
lucide-yes = yes
lucide-no = no
lucide-cap-round = round
lucide-cap-straight = butt
lucide-cap-square = square

lucide-sheet-name = Name
lucide-sheet-component = Component
lucide-sheet-codepoint = Codepoint
lucide-sheet-categories = Categories
lucide-sheet-aliases = Also called
lucide-sheet-tags = Tags
lucide-sheet-help = ↑↓ row · ←→ adjusts where marked, otherwise switches icon · Enter copies · Tab resets · Esc goes back
lucide-sheet-copied = ✓ copied
lucide-sheet-copy-failed = ✗ could not copy — check the log

lucide-category-all = All
lucide-category-accessibility = Accessibility
lucide-category-account = Account
lucide-category-animals = Animals
lucide-category-arrows = Arrows
lucide-category-brands = Brands
lucide-category-buildings = Buildings
lucide-category-charts = Charts
lucide-category-communication = Communication
lucide-category-connectivity = Connectivity
lucide-category-cursors = Cursors
lucide-category-design = Design
lucide-category-development = Development
lucide-category-devices = Devices
lucide-category-emoji = Emoji
lucide-category-files = Files
lucide-category-finance = Finance
lucide-category-food-beverage = Food
lucide-category-gaming = Gaming
lucide-category-home = Home
lucide-category-layout = Layout
lucide-category-mail = Mail
lucide-category-math = Math
lucide-category-medical = Medical
lucide-category-multimedia = Multimedia
lucide-category-nature = Nature
lucide-category-navigation = Navigation
lucide-category-notifications = Notifications
lucide-category-people = People
lucide-category-photography = Photography
lucide-category-science = Science
lucide-category-seasons = Seasons
lucide-category-security = Security
lucide-category-shapes = Shapes
lucide-category-shopping = Shopping
lucide-category-social = Social
lucide-category-sports = Sports
lucide-category-sustainability = Sustainability
lucide-category-text = Text
lucide-category-time = Time
lucide-category-tools = Tools
lucide-category-transportation = Transportation
lucide-category-travel = Travel
lucide-category-weather = Weather

unicode-panel-emoji = emoji only
unicode-tone-light = Light skin
unicode-tone-medium-light = Medium-light skin
unicode-tone-medium = Medium skin
unicode-tone-medium-dark = Medium-dark skin
unicode-tone-dark = Dark skin

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
syncthing-typed-in-the-footer = It is typed in the footer's minibuffer.
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
network-remember = Remember { $network }?  s = yes · n = no
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
cmd-memory-sweep-generations = Remove old generations
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

files-format-zstd = .tar.zst — what you keep
files-format-gzip = .tar.gz — what you send anywhere
files-format-zip = .zip — what you send to somebody on Windows

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
files-compress = compress
files-extract = extract
files-extract-tar = extract the tar
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
files-ask-format = Compress in which format?
files-ask-name = Rename to:
files-hidden = hidden files
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
files-does-open = opens in the MIME type's default application; a folder is entered
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
files-does-extract-tar = extracts the tar right here, with no folder
files-does-restore = puts it back at the origin path the trash recorded
files-does-trash = moves it to the XDG trash; does not cross filesystems
files-does-delete = unlink(2) or rmdir(2) straight away: no trash, no way back
files-does-shred = overwrites before deleting; on copy-on-write btrfs that guarantees nothing

# And the sidebar menu's, which talk to udisks.
sidebar-does-open = enters the folder; for an unmounted device it opens its sheet
sidebar-does-mount = udisks2's Filesystem.Mount — it picks the point and the options
sidebar-does-unmount = Filesystem.Unmount, never forced: it refuses while something is open
sidebar-does-eject = unmounts and cuts the drive's power (Drive.PowerOff): what makes it safe to pull
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

transfers-empty = nothing under way
transfers-offline = the transfer queue is not up
# The hint line, with the key resolved at the time — never written here.
transfers-hint = { $pause } holds, { $cancel } gives up, { $sweep } clears what ended

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
cmd-transfers-hold = hold or let go
cmd-transfers-give-up = give up
cmd-transfers-drop = drop from the list
cmd-transfers-clear = clear what has finished
cmd-transfers-detail = this row, in detail

transfers-window-title = transfers
transfers-window-detail = the transfer, field by field
transfers-window-empty = nothing in the queue
transfers-window-unknown = not known
transfers-window-hint = m p holds · m c gives up · m d details · m l clears
transfers-window-hint-detail = m d back to the list · m p holds · m c gives up
transfers-window-confirm-cancel = giving up deletes the piece already written. Enter confirms, any other key drops the question.
transfers-window-asked = request sent

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
# ONE CLICK UPDATES EVERYTHING — the sequence in `assets/updates.nix`
#
# The sequence is declared once and generated from there: the table, the two
# runners, and this list of keys. `gen-updates.py` derives `update-<id>` and
# `update-does-<id>` from every step's id and emits them to `keys.txt`, so a
# step with no sentence in both languages cannot be written.
#
# The order below is the SEQUENCE'S order, which is the declaration's order —
# there is no `order` field, on purpose, and sorting these alphabetically would
# put back the second source the table exists to remove.
# ===================================================================
cmd-system-update = Update the system
cmd-system-update-rehearsal = Rehearse the update

# EACH STEP'S NAME, and under it the line that reaches the message line when the
# cursor is on it — the same `{prefix}-does-<id>` shape the file manager and the
# format buffer use. The name is what it is called; the line under it is what it
# actually runs, and what it costs to be wrong about.
update-flake-update = fetch the new versions
update-does-flake-update = nix flake update on /etc/nixos: moves the pinned inputs forward and touches nothing but flake.lock. It needs the network, and on the ISO /etc/nixos is read-only — there the step is skipped and says so.
update-system-rebuild = rebuild the system
update-does-system-rebuild = nixos-rebuild switch, through the unit that already exists: it compiles, and it can bring a kernel that has never booted. The way back is the previous generation, in the boot menu. If it fails, the sequence stops here.
update-store-collect-garbage = collect the store's garbage
update-does-store-collect-garbage = nix-collect-garbage with no --delete-older-than: it deletes what nothing references, and the old generations stay — deleting them in the same breath would be deleting the way back.
update-store-optimise = deduplicate the store
update-does-store-optimise = nix store optimise: identical files become hard links. It runs after the collection, or it would optimise precisely what was about to be deleted.
update-old-generations = remove the old generations
update-does-old-generations = nix-collect-garbage --delete-older-than 30d: it deletes the way back. After it the boot menu has nothing to fall back to, which is why it ships off.
update-journal-vacuum = trim the journal
update-does-journal-vacuum = journalctl --vacuum-size=256M: it throws away the oldest record, this update's own included — which is the answer to "why did the rebuild fail?". It ships off.
update-font-cache = rebuild the font cache
update-does-font-cache = fc-cache -rf: from scratch, ignoring the timestamps, because a rebuild can have changed the font set and a stale cache with a fresh date is exactly what brings somebody here.
update-tldr-cache = update the tldr pages
update-does-tldr-cache = tldr --update, as you: the weekly unit downloads them as `nobody`, into a home nobody reads. It needs the network; without it the step is skipped, rather than waiting for a timeout and calling that a failure.
update-rust-toolchains = update the Rust toolchains
update-does-rust-toolchains = rustup update: it refreshes the channels already under ~/.rustup, and installs none. Here Rust comes from the flake, so that directory does not exist and the step is skipped saying so — it is for the machine where you put rustup there by hand.
update-node-global = update npm's global packages
update-does-node-global = npm update --global: it moves what is already in the global prefix and installs nothing. Node comes from the flake; the global prefix does not — without that directory the step is skipped and says so.
update-pnpm-global = update pnpm's global packages
update-does-pnpm-global = pnpm update --global: pnpm keeps a global store of its own, which is not npm's — which is why these are two steps and not one. Without its directory the step is skipped and says so.
update-python-tools = update the uv tools
update-doom-sync = Bring Doom Emacs's packages in line
update-does-python-tools = uv tool upgrade --all: it upgrades the tools under $XDG_DATA_HOME/uv/tools, one venv each, and adds none. uv comes from the flake; they do not — without that directory the step is skipped and says so.
update-does-doom-sync = Resolves Doom's package list against the configuration the rebuild just wrote, and byte-compiles what changed. Until it runs, the editor is still loading the previous one. It skips on a machine where Doom was never installed.

# WHY A STEP WAS SKIPPED. The path or the name it is about travels beside the
# sentence as data (`Needs::about()`), which is why none of these names one.
update-needs-always = nothing can make this step inapplicable
update-needs-binary = this program is not on your PATH
update-needs-directory = this directory does not exist
update-needs-writable-file = this file does not exist, or cannot be written to
update-needs-network = NetworkManager says this machine is not connected

# WHAT A FAILURE DOES TO THE REST OF THE SEQUENCE. Two answers and no third:
# this runs from a unit, and there is nobody to ask.
update-on-failure-stop = if it fails, the sequence stops here
update-on-failure-carry-on = if it fails, the sequence carries on to the next step

# THE FOUR CLASSES A STEP CAN BE. They decide the order laws, and the view
# groups by them.
update-kind-move-inputs = moves the pinned inputs
update-kind-apply-configuration = applies the configuration
update-kind-refresh-cache = refreshes a cache
update-kind-reclaim-space = reclaims disk space


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
cmd-brain-notice = Do what it offers

# THE PAGE ITSELF.
brain-title = Brain
brain-nothing = Nothing is set up yet
brain-tasks = Tasks
brain-activity = Activity
brain-no-activity = Nothing has happened yet
brain-schedule = Schedule
brain-steps = Steps
brain-history = History

# WHAT THE HEADER TEACHES about the page's own keys. `cmd-*` covers the rest.
brain-do-open = Open it
brain-do-back = Back
brain-do-pane = Other half
brain-do-change = Change this
brain-do-keys = Every key
brain-do-notice = Do it
brain-task-emacs-daemon = Watch the Emacs daemon
brain-task-emacs-daemon-does = Looks, every few minutes, for the Emacs server that holds your session and your configuration — and for an Emacs that is running WITHOUT it, which looks the same on screen and is not: it loaded its own copy, its buffers are not the session's, and closing it loses them. It reads the process table and nothing else, and the only thing it ever starts is the distro's own service.
emacs-no-daemon = The Emacs daemon is not running. Every editor window will load the whole configuration again, and none of them will share your buffers.
emacs-not-the-client = This Emacs is not connected to the daemon: it loaded its own copy of the configuration, and its buffers belong to it alone. Close it and open it again with `e`, or from the file manager.
cmd-brain-start-emacs = Start it

# THE CLOCK WATCH. It does not fetch the time — `systemd-timesyncd` does, and
# doing it twice would be two things setting one clock. What this does is keep
# the machine's automatic time-keeping switched on and say when it is not.
brain-task-clock-sync = Watch the clock
brain-task-clock-sync-does = Checks, a few times a day, that something is keeping this machine's clock right over the internet — and turns it back on when nothing is. It does not ask anybody for the time itself: the system's own time client does that continuously, and a second one would be two clocks disagreeing. Switch this off to keep the clock by hand.
clock-not-automatic = Nothing is keeping this machine's clock right. It will drift, and every timestamp on this machine drifts with it.
clock-not-synchronised = The clock is being looked after, but it has not been able to reach a time server yet.
cmd-brain-clock-automatic = Keep it right

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
brain-task-system-update = Update the system
brain-task-system-update-does = Moves the pinned inputs forward, rebuilds the system as a boot entry and reclaims store space. Nothing changes under a session that is open: the new system starts at the next reboot, which is a moment you choose.
brain-task-cache-sweep = Sweep the caches
brain-task-cache-sweep-does = Deletes whole folders under your cache directory that nothing has opened for a while, counting from the last time each was USED. It never follows a link out of that directory, never leaves the filesystem it started on and keeps anything stamped in the future. A cache is what can always be made again, so what is lost is time and not work.
brain-task-trash-expire = Empty the trash
brain-task-trash-expire-does = Deletes for good what you threw away four days ago or longer. The clock starts when you put it in the bin and not when the file was written, so something made years ago and deleted this morning has four days. It is the one thing here that cannot be undone, which is why the number is small.

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
echo-emacs-starting = Starting the Emacs daemon
echo-emacs-unreachable = The Emacs daemon could not be started; see: journalctl --user -u emacs
echo-clock-kept = This machine's clock is being kept right again
echo-clock-refused = The clock could not be handed over; see: journalctl -u systemd-timedated

# THE UPDATE, WATCHED — the modeline's mark and the window it opens.
#
# The mark shows only while something runs and goes on its own. The window is a
# command of its own because a mark one can only click is a mark the keyboard
# cannot reach.
cmd-update-watch = Watch the update
hint-update-mark = click to watch the update happen
echo-update-queued = In the queue — it starts when the machine is free
echo-update-started = The update has started
echo-update-nowhere = There is nothing here that can start it
update-hint = ↑↓ a step · C-j C-k the output · G the end · Esc closes
update-no-output = nothing has been said yet
update-verdict-ran = it worked
update-verdict-failed = it failed
update-verdict-off = switched off
update-verdict-skipped = did not apply
update-verdict-rehearsed = only rehearsed
update-verdict-stopped = it stopped here
update-verdict-pending = still to come

## The theme chooser — LEADER s t. The themes are listed by id and have no
## key; these are the option beside them (the desk under the windows), its
## answers — one `theme-backdrop-<id>` per answer — and the hint.
theme-option-theme = Theme
theme-option-backdrop = Background
theme-backdrop-black = Black
theme-backdrop-theme = The theme's colour
theme-hint = { $apply } applies · { $left }/{ $right } changes column · { $close } goes back
echo-theme-unsaved = the theme could not be saved: { $error }

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
power-answer-in-the-footer = The lock is the footer's to hold: { $awake }, or { $lid }
power-what-profile = the profile
power-what-lock = the lock
power-what-thresholds = the thresholds
power-what-action = the action
power-refused-authority = not allowed for this session
power-refused-absent = the daemon is not there, or the hardware cannot
power-refused-invalid = the value was refused
power-refused-other = refused
echo-power-refused = { $what } { $why } ({ $words })
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

# THE WAY OUT — the three marks, and what the daemon believes.
network-way = the internet
network-airplane = aeroplane mode
network-tunnel-drop = RET takes it down
cmd-network-airplane = Aeroplane mode (toggle)
echo-network-refused = the network refused: { $error }
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
network-lan-refused = this machine may not send a ping: { $error }
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
network-import-asks = it will ask for a password the first time it comes up
network-import-bring = RET brings it in
network-import-leave = Esc leaves it where it is
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
network-answer-in-the-footer = A password is typed in the footer: open the network there.
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
bluetooth-answer-in-the-footer = the question is answered in the footer
bluetooth-type-in-the-footer = the name is typed in the footer

# WHAT WENT WRONG, said as the name of the door that shut.
echo-bluetooth-refused = { $what }: { $why } ({ $words })
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
display-arrange-how = The arrows move the screen under the cursor from side to side.

display-ask = { $what } on { $screen }.
display-ask-seconds = It goes back on its own in { $seconds }s.
display-ask-forget = Everything chosen for { $screen } goes back to what the compositor decides.
display-ask-nothing-yet = Nothing has changed yet.
display-leave-it = Leave it as it is
display-forget-it = Forget it all
display-screen-is-dark = the compositor is not drawing on it
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
display-refused-not-writable = the file belongs to root, and the rule that opens it has not run on this panel
display-refused-nowhere = there is nowhere to write it down
display-refused-other = it did not work
echo-display-refused = The screens refused: { $why } ({ $words })
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
geo-ask-filter = Region:
geo-queued = In the queue. Its size is what the server answers, and it can be given up there.
geo-no-home = There is no data directory to install into.
geo-in-the-footer = Typing happens in the footer — { $keys } opens this there.
cmd-geo-open = Regions and their streets, in the footer
cmd-geo-window = Regions and their streets, in a window
cmd-geo-up = Previous row
cmd-geo-down = Next row
cmd-geo-enter = Install the region under the cursor, or install it again
cmd-geo-find = Narrow the list of regions
cmd-geo-remove = Remove the installed region under the cursor
group-geo = geography

# ---------------------------------------------------------------------------
# LEADER d — the glance: what time it is somewhere, and the weather and money.
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
locale-ask-system-date = The machine's date, as 2026-09-21:
locale-ask-system-time = The machine's time, as 14:32 or 14:32:05:
locale-ask-system-zone = The machine's time zone:
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
locale-ask-zone = Zone for this interface's clock:
locale-ask-place = Zone of the place to add:
locale-ask-where = Latitude and longitude, as two numbers:
locale-no-such-zone = There is no zone by that name.
locale-not-a-coordinate = That is not a latitude and a longitude.
locale-in-the-footer = Typing happens in the footer — { $keys } opens this there.
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
atuin-typed-in-the-footer = It is typed in the footer's minibuffer.
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
about-files-hidden = Show or hide the entries whose name begins with a dot.
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
about-transfers-cancel = Give this transfer up. What has been written so far is deleted.
about-transfers-forget = Take this row off the list. The file it wrote stays where it is.
about-transfers-sweep = Take every finished row off the list at once.
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
# DRAWN, AND REFUSING. The owner asked for the option to exist and for its
# implementation to wait, so it says which of the two it is.
lifeline-ai-not-yet = that one is not built yet — it is here so it has somewhere to be
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
camera-typed-elsewhere = Type it in the footer: the question is open there.

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
camera-about-effects = What is done to the camera's picture, live — in the app, in photos and recordings, and in what other programs get from the virtual camera.
camera-effect = Effect
camera-background = Background
camera-backdrop = Picture behind you

camera-heading-virtual = Virtual camera
camera-about-virtual = Other programs — a browser call, a meeting — can choose the “ronoctua” camera and get this picture, effects and all. It only sends while it is on.
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
