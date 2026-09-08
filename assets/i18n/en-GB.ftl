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

group-brain = brain
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
state-update = update

## Notifications

notif-title = Notifications

# The band, above the modeline. The counter only appears with more than one.
notif-queue-position = { $shown }/{ $total }

# THE CATALOGUE — the distro's own notifications, one summary and one body
# each. For an entry that is REWRITTEN (the compositor's screenshot), these are
# what reaches the screen. For one we raise ourselves, they are the default
# text and the entry's name in the manager.
notif-screenshot-summary = Screen captured
notif-screenshot-body = The image is on the clipboard and saved to disk.
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
inst-step-tpm = Seal the disk key into the TPM

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
group-window = window
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

## Compositor actions — §13, "every OS action has a command"

# This section is LONG on purpose: these are the 141 actions niri exposes, each
# reachable by name in the command locator. The list of names is
# GENERATED from the pinned binary (nix/niri-actions.nix); the translations are written
# here, and the `niri-actions-translated` check rejects an action without a key in both
# languages. Bumping niri's tag STOPS the build until whatever appeared is
# translated — which is the opposite of showing up in English in the middle of the list.

cmd-niri-quit = Exit niri
cmd-niri-power-off-monitors = Power off all monitors via DPMS
cmd-niri-power-on-monitors = Power on all monitors via DPMS
cmd-niri-spawn = Spawn a command
cmd-niri-spawn-sh = Spawn a command through the shell
cmd-niri-do-screen-transition = Do a screen transition
cmd-niri-screenshot = Open the screenshot UI
cmd-niri-screenshot-screen = Screenshot the focused screen
cmd-niri-screenshot-window = Screenshot the focused window
cmd-niri-toggle-keyboard-shortcuts-inhibit = Enable or disable the keyboard shortcuts inhibitor (if any) for the focused surface
cmd-niri-close-window = Close the focused window
cmd-niri-fullscreen-window = Toggle fullscreen on the focused window
cmd-niri-toggle-windowed-fullscreen = Toggle windowed (fake) fullscreen on the focused window
cmd-niri-focus-window = Focus a window by id
cmd-niri-focus-window-in-column = Focus a window in the focused column by index
cmd-niri-focus-window-previous = Focus the previously focused window
cmd-niri-focus-column-left = Focus the column to the left
cmd-niri-focus-column-right = Focus the column to the right
cmd-niri-focus-column-first = Focus the first column
cmd-niri-focus-column-last = Focus the last column
cmd-niri-focus-column-right-or-first = Focus the next column to the right, looping if at end
cmd-niri-focus-column-left-or-last = Focus the next column to the left, looping if at start
cmd-niri-focus-column = Focus a column by index
cmd-niri-focus-window-or-monitor-up = Focus the window or the monitor above
cmd-niri-focus-window-or-monitor-down = Focus the window or the monitor below
cmd-niri-focus-column-or-monitor-left = Focus the column or the monitor to the left
cmd-niri-focus-column-or-monitor-right = Focus the column or the monitor to the right
cmd-niri-focus-window-down = Focus the window below
cmd-niri-focus-window-up = Focus the window above
cmd-niri-focus-window-down-or-column-left = Focus the window below or the column to the left
cmd-niri-focus-window-down-or-column-right = Focus the window below or the column to the right
cmd-niri-focus-window-up-or-column-left = Focus the window above or the column to the left
cmd-niri-focus-window-up-or-column-right = Focus the window above or the column to the right
cmd-niri-focus-window-or-workspace-down = Focus the window or the workspace below
cmd-niri-focus-window-or-workspace-up = Focus the window or the workspace above
cmd-niri-focus-window-top = Focus the topmost window
cmd-niri-focus-window-bottom = Focus the bottommost window
cmd-niri-focus-window-down-or-top = Focus the window below or the topmost window
cmd-niri-focus-window-up-or-bottom = Focus the window above or the bottommost window
cmd-niri-move-column-left = Move the focused column to the left
cmd-niri-move-column-right = Move the focused column to the right
cmd-niri-move-column-to-first = Move the focused column to the start of the workspace
cmd-niri-move-column-to-last = Move the focused column to the end of the workspace
cmd-niri-move-column-left-or-to-monitor-left = Move the focused column to the left or to the monitor to the left
cmd-niri-move-column-right-or-to-monitor-right = Move the focused column to the right or to the monitor to the right
cmd-niri-move-column-to-index = Move the focused column to a specific index on its workspace
cmd-niri-move-window-down = Move the focused window down in a column
cmd-niri-move-window-up = Move the focused window up in a column
cmd-niri-move-window-down-or-to-workspace-down = Move the focused window down in a column or to the workspace below
cmd-niri-move-window-up-or-to-workspace-up = Move the focused window up in a column or to the workspace above
cmd-niri-consume-or-expel-window-left = Consume or expel the focused window left
cmd-niri-consume-or-expel-window-right = Consume or expel the focused window right
cmd-niri-consume-window-into-column = Consume the window to the right into the focused column
cmd-niri-expel-window-from-column = Expel the bottom window from the focused column
cmd-niri-swap-window-right = Swap focused window with one to the right
cmd-niri-swap-window-left = Swap focused window with one to the left
cmd-niri-toggle-column-tabbed-display = Toggle the focused column between normal and tabbed display
cmd-niri-set-column-display = Set the display mode of the focused column
cmd-niri-center-column = Centre the focused column on the screen
cmd-niri-center-window = Centre the focused window on the screen
cmd-niri-center-visible-columns = Centre all fully visible columns on the screen
cmd-niri-focus-workspace-down = Focus the workspace below
cmd-niri-focus-workspace-up = Focus the workspace above
cmd-niri-focus-workspace = Focus a workspace by reference (index or name)
cmd-niri-focus-workspace-previous = Focus the previous workspace
cmd-niri-move-window-to-workspace-down = Move the focused window to the workspace below
cmd-niri-move-window-to-workspace-up = Move the focused window to the workspace above
cmd-niri-move-window-to-workspace = Move the focused window to a workspace by reference (index or name)
cmd-niri-move-column-to-workspace-down = Move the focused column to the workspace below
cmd-niri-move-column-to-workspace-up = Move the focused column to the workspace above
cmd-niri-move-column-to-workspace = Move the focused column to a workspace by reference (index or name)
cmd-niri-move-workspace-down = Move the focused workspace down
cmd-niri-move-workspace-up = Move the focused workspace up
cmd-niri-move-workspace-to-index = Move the focused workspace to a specific index on its monitor
cmd-niri-set-workspace-name = Set the name of the focused workspace
cmd-niri-unset-workspace-name = Unset the name of the focused workspace
cmd-niri-focus-monitor-left = Focus the monitor to the left
cmd-niri-focus-monitor-right = Focus the monitor to the right
cmd-niri-focus-monitor-down = Focus the monitor below
cmd-niri-focus-monitor-up = Focus the monitor above
cmd-niri-focus-monitor-previous = Focus the previous monitor
cmd-niri-focus-monitor-next = Focus the next monitor
cmd-niri-focus-monitor = Focus a monitor by name
cmd-niri-move-window-to-monitor-left = Move the focused window to the monitor to the left
cmd-niri-move-window-to-monitor-right = Move the focused window to the monitor to the right
cmd-niri-move-window-to-monitor-down = Move the focused window to the monitor below
cmd-niri-move-window-to-monitor-up = Move the focused window to the monitor above
cmd-niri-move-window-to-monitor-previous = Move the focused window to the previous monitor
cmd-niri-move-window-to-monitor-next = Move the focused window to the next monitor
cmd-niri-move-window-to-monitor = Move the focused window to a specific monitor
cmd-niri-move-column-to-monitor-left = Move the focused column to the monitor to the left
cmd-niri-move-column-to-monitor-right = Move the focused column to the monitor to the right
cmd-niri-move-column-to-monitor-down = Move the focused column to the monitor below
cmd-niri-move-column-to-monitor-up = Move the focused column to the monitor above
cmd-niri-move-column-to-monitor-previous = Move the focused column to the previous monitor
cmd-niri-move-column-to-monitor-next = Move the focused column to the next monitor
cmd-niri-move-column-to-monitor = Move the focused column to a specific monitor
cmd-niri-set-window-width = Change the width of the focused window
cmd-niri-set-window-height = Change the height of the focused window
cmd-niri-reset-window-height = Reset the height of the focused window back to automatic
cmd-niri-switch-preset-column-width = Switch between preset column widths
cmd-niri-switch-preset-column-width-back = Switch between preset column widths backwards
cmd-niri-switch-preset-window-width = Switch between preset window widths
cmd-niri-switch-preset-window-width-back = Switch between preset window widths backwards
cmd-niri-switch-preset-window-height = Switch between preset window heights
cmd-niri-switch-preset-window-height-back = Switch between preset window heights backwards
cmd-niri-maximize-column = Toggle the maximised state of the focused column
cmd-niri-maximize-window-to-edges = Toggle the maximised-to-edges state of the focused window
cmd-niri-set-column-width = Change the width of the focused column
cmd-niri-expand-column-to-available-width = Expand the focused column to space not taken up by other fully visible columns
cmd-niri-switch-layout = Switch between keyboard layouts
cmd-niri-show-hotkey-overlay = Show the hotkey overlay
cmd-niri-move-workspace-to-monitor-left = Move the focused workspace to the monitor to the left
cmd-niri-move-workspace-to-monitor-right = Move the focused workspace to the monitor to the right
cmd-niri-move-workspace-to-monitor-down = Move the focused workspace to the monitor below
cmd-niri-move-workspace-to-monitor-up = Move the focused workspace to the monitor above
cmd-niri-move-workspace-to-monitor-previous = Move the focused workspace to the previous monitor
cmd-niri-move-workspace-to-monitor-next = Move the focused workspace to the next monitor
cmd-niri-move-workspace-to-monitor = Move the focused workspace to a specific monitor
cmd-niri-toggle-debug-tint = Toggle a debug tint on windows
cmd-niri-debug-toggle-opaque-regions = Toggle visualisation of render element opaque regions
cmd-niri-debug-toggle-damage = Toggle visualisation of output damage
cmd-niri-toggle-window-floating = Move the focused window between the floating and the tiling layout
cmd-niri-move-window-to-floating = Move the focused window to the floating layout
cmd-niri-move-window-to-tiling = Move the focused window to the tiling layout
cmd-niri-focus-floating = Switches focus to the floating layout
cmd-niri-focus-tiling = Switches focus to the tiling layout
cmd-niri-switch-focus-between-floating-and-tiling = Toggles the focus between the floating and the tiling layout
cmd-niri-move-floating-window = Move the floating window on screen
cmd-niri-toggle-window-rule-opacity = Toggle the opacity of the focused window
cmd-niri-set-dynamic-cast-window = Set the dynamic cast target to the focused window
cmd-niri-set-dynamic-cast-monitor = Set the dynamic cast target to the focused monitor
cmd-niri-clear-dynamic-cast-target = Clear the dynamic cast target, making it show nothing
cmd-niri-stop-cast = Stop a PipeWire screencast
cmd-niri-toggle-overview = Toggle (open/close) the Overview
cmd-niri-open-overview = Open the Overview
cmd-niri-close-overview = Close the Overview
cmd-niri-toggle-window-urgent = Toggle urgent status of a window
cmd-niri-set-window-urgent = Set urgent status of a window
cmd-niri-unset-window-urgent = Unset urgent status of a window
cmd-niri-load-config-file = Reload the config file
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
cmd-clipboard-clipcascade-configure = Configure ClipCascade
clipboard-empty = Nothing copied yet
clipboard-pinned = pinned
clipboard-content = Content
clipboard-types = Types
clipboard-size = Size
clipboard-actions = Actions
clipboard-clipcascade = ClipCascade
clipboard-clipcascade-missing = not integrated
echo-copied = Copied
echo-copied-value = Copied: { $value }
echo-copy-failed = Could not copy — see the log
echo-qr-large = The content is too large for a QR code
echo-clipcascade-missing = ClipCascade is not integrated yet
echo-copy-only-text = For now only text can be copied again
clipboard-image = Image

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

cmd-syncthing-open = Open Syncthing
cmd-syncthing-state = Syncthing status
cmd-syncthing-pause = Pause syncing
cmd-syncthing-resume = Resume syncing
group-syncthing = syncthing
syncthing-line = Syncthing
syncthing-idle = up to date
syncthing-syncing = syncing { $percent }%
syncthing-stopped = stopped
echo-syncthing-state = { $connected } connected · { $percent }% · ↑{ $up } ↓{ $down }
echo-syncthing-stopped = Syncthing is not responding
echo-syncthing-paused = Syncing paused
echo-syncthing-resumed = Syncing resumed

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
cmd-example-restart = Restart the example

## The memory panel — LEADER s m

cmd-memory-open = System memory
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
rescue-restart-niri = Restart the compositor
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
files-when = { $y }-{ $m }-{ $d } { $hh }:{ $mm }
files-when-utc = { $y }-{ $m }-{ $d } { $hh }:{ $mm } UTC

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
files-theatre-on = give the preview the whole region
files-theatre-off = give the listing its room back
files-theatre-nothing = nothing is playing, so there is no region to give
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
files-columns = permissions and owner
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
files-theatre = the whole region
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
files-explain-owner = { $who } owns it; clicking changes that
files-explain-group = the group is { $who }; clicking changes that
files-explain-size = exactly { $bytes } bytes
files-explain-measure = click to measure this directory — it is a walk
files-explain-when = and { $seconds } seconds, in this machine's local time
files-explain-when-utc = and { $seconds } seconds, in UTC — /etc/localtime could not be read

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
files-preview-failed = what came back was not a picture — the file may be damaged
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
# THE PHOTOGRAPH CARRIES A PLACE AND THIS SCREEN IS NOT PRINTING IT. What
# somebody needs before sending a picture to a stranger is not "hidden": it is
# that the file still knows, and that sending the file sends the place too.
files-value-place-withheld = written inside the file, not shown here — it goes wherever the photograph goes
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
group-capture = capture

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

transfers-work-fetch = download
transfers-work-media = download video
transfers-work-audio = download audio
transfers-work-subtitles = download subtitles
transfers-work-torrent = torrent


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
update-does-python-tools = uv tool upgrade --all: it upgrades the tools under $XDG_DATA_HOME/uv/tools, one venv each, and adds none. uv comes from the flake; they do not — without that directory the step is skipped and says so.

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
# THE BRAIN'S SCREEN — LEADER b b, and a window of its own
#
# What the machine does BY ITSELF, when, and what came of it last time. No
# sentence here spells a date: the buffer draws those ISO, because a component
# cannot translate (invariant 11) and *19 Aug* against *19 ago* is exactly the
# kind of thing it must not invent. What is written below is the words AROUND
# the numbers, and never the numbers.
#
# TWO REGISTERS, and the second one is why the screen exists. `brain-hold-*`
# says what a run is waiting FOR — a machine that is busy, not a run that broke
# — and `brain-why-*` says why one did not happen at all. Neither of them ever
# says that somebody was asked and did not answer: the update path never asks,
# and `updates-never-ask` is the check that proves it. A reason on this screen
# is the machine's, or a decision the person took on purpose.
# ===================================================================
cmd-brain-open = What the machine does by itself
cmd-brain-run = Run it now
cmd-brain-rehearse = Rehearse it, changing nothing
cmd-brain-abort = Stop it
cmd-brain-proceed = Start now, without the countdown
cmd-brain-toggle-step = Switch this step on or off
cmd-brain-reset-steps = Back to the steps the distro ships
cmd-brain-interval = How often it runs
cmd-brain-window = Which part of the day
cmd-brain-switch = Switch this task off and on

brain-title = What the machine does by itself
brain-nothing = there is nothing the machine does by itself
brain-hint = Enter opens · m r runs it now · m e rehearses · m o on and off
brain-hint-update = m t a step on or off · m i how often · m w when · m a stops it · Esc goes back

# THE THREE BLOCKS of the page behind, and the labels down their left edge.
# That column is sixteen cells wide and the unit beside it is ten: these are
# short because the columns are, and a longer word here is a word somebody
# reads with its end cut off.
brain-steps = what it runs
brain-when = when it runs
brain-doing = right now
brain-outcome = outcome
brain-last = the last run
brain-every = every
brain-days = days
brain-part = part of the day
brain-next = next
brain-waiting = reason
brain-soon = starts in
brain-failed-at = it stopped at
brain-rehearsal = a rehearsal — it changes nothing
brain-never = never
brain-changed = you changed
brain-needs-root = it runs as root, with the whole machine's permissions

# THE ONE TASK THERE IS, and under the cursor the quiet line saying what that
# actually means — the rule the disk formatter's menus already follow, where
# every row carries a second register.
brain-task-system-update = Update the system
brain-task-system-update-does = the machine brings its own software forward and builds the new system as a boot entry, without switching to it: nothing changes underneath a session that is open, and what starts it is the next reboot — a moment you choose.

# ITS STEPS, in the order they run. The real command travels beside each one as
# data, under the cursor, so these say what the step is FOR and not what it
# types.

# WHERE A TASK IS RIGHT NOW. Fourteen cells on the home page's last column, so
# one short phrase each.
brain-standing-idle = waiting
brain-standing-queued = in the queue
brain-standing-running = running now

# AND HOW THE LAST RUN ENDED. `failed` is the honest word for a run that did
# not finish; whether that was a fault or a decision is the reason's job, on
# the line below it.
brain-outcome-skipped = did not run
brain-outcome-done = it worked
brain-outcome-failed = it failed

# WHAT A RUN IS WAITING FOR — the most important sentences on this screen.
# NOTHING IS BROKEN AND NOTHING WAS LOST: the moment came, the machine was in
# no state for it, and it waits for the next one. Each says what is in the way,
# which is the thing somebody could act on if they wanted to.
brain-hold-ram = RAM is busy
brain-hold-cpu = CPU is busy
brain-hold-battery = on battery
brain-hold-busy = one at a time
brain-hold-network = no network
brain-hold-asleep = sleeping

# AND WHY ONE DID NOT HAPPEN AT ALL. The first two go in a column seventeen
# cells wide, beside the row's name; the rest have the width of a sentence.
# `aborted` is NOT an error — it is the person having decided, and the screen
# agreeing with them.
brain-why-off = switched off
brain-why-no-systemctl = no systemd here
brain-why-runaway = it stopped reporting and the waiting was given up — it may well still be going
brain-why-aborted = you stopped it — nothing went wrong
brain-why-postponed = you said not now, and the next moment moved

# WHICH PART OF THE DAY it may start in. The hours are the same digits in every
# language; the words around them are not.
brain-part-any = whenever the machine is free
brain-part-night = night, 02:00 to 05:00
brain-part-morning = morning, 08:00 to 12:00
brain-part-afternoon = afternoon, 13:00 to 18:00
brain-part-evening = evening, 19:00 to 23:00

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
display-config-failed = The compositor could not read its configuration. What is on the screen now will not come back after a restart.

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
