# The texts `ronoctua-shell-footer` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Commands — the `M-x` list and the which-key labels

cmd-mx-open = Run command…
cmd-buffer-terminal-external = Open external terminal
cmd-monitor-btop = Process monitor (btop)
cmd-monitor-bottom = Process monitor (bottom)

## which-key prefix groups

group-quit = quit

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
notif-memory-killed-summary = Closed to free memory

## Installer — the interface

echo-no-on = { $sequence } is not bound
# Same sequence, other case. Shown when the case IS the reason nothing
# matched — Caps Lock or Shift. Without it the screen names the consequence
# and hides the cause.
echo-no-on-box = { $sequence } is not bound — { $other } is
echo-cancelled = Quit
echo-no-window = No focused window
echo-no-action = { $command } does nothing yet
cmd-window-right = Focus right
cmd-window-up = Focus up
cmd-window-down = Focus down
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
echo-capture-failed = The capture could not be taken — the reason is in the log
group-window = window/buffer
group-terminal = terminal
group-workspace = workspace
cmd-workspace-overview = Workspace overview
cmd-window-float = Toggle floating
cmd-workspace-all = See every workspace at once
cmd-workspace-move = Move window to workspace

## Notifications, keyboard and help — commands that existed only as an order or a key

cmd-keyboard-restore = Restore the keyboard
echo-keyboard-restored = Keyboard restored
echo-keyboard-failed = Could not restore the keyboard: { $reason }

## The session's end

echo-session-ending = Letting your programs save before the session ends — quit again to leave now

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
expansion-page = RCtrl ←→
echo-clipboard-synced = The clipboard is syncing
echo-clipboard-sync-off = The clipboard stays on this machine
echo-clipboard-sync-failed = Could not sync: { $reason }
echo-copy-only-text = For now only text can be copied again
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

cmd-theme-open = Themes
cmd-theme-apply = Apply this theme and background

## Syncthing (LEADER s s)

hint-syncthing-on = Everything is being synchronised.
hint-syncthing-off = Nothing is being synchronised.

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

cmd-leader-set-window = The same keys, each opening its app in a window instead of the footer
cmd-leader-set-full = The whole tree of keys, with every submenu as it was

## The memory panel — LEADER s m

cmd-memory-open = System memory
cmd-example-open = Several keyboards in one app, the reference
cmd-format-open = Format a disk
group-system = system

## Rescue console — what it says

cmd-rescue-open = Rescue console
cmd-apps-pin = Put the application on the file manager's sidebar
files-sidebar-unwritable = could not save the sidebar: { $why }
group-geography = geography
# `c` stopped being a command and became a group: the clipboard captures what
# you copy, and the screen captures what you see.
group-miscellaneous = miscellaneous
transfers-queued-as-me = in the queue, AS YOU — this one is not anonymous
transfers-nothing-copied = nothing has been copied
transfers-not-an-address = what is copied is not an address
transfers-queued = in the queue
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
cmd-brain-abort = Stop it
cmd-brain-proceed = Start now
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
hint-update-mark = click to watch the update happen

## `theme-backdrop-near`, is offered only under a light theme.

theme-option-theme = Theme
theme-option-backdrop = Variations
theme-hint = { $apply } applies · { $left }/{ $right } changes column

## Power — the battery, the charger, the profile and sleep (LEADER s p)

# The modeline's two marks, on the line under the pointer.
hint-battery = { $percent }% { $state }, { $time }, { $rate } W — click opens power
hint-profile = profile: { $profile } — click opens power
hint-battery-no-rate = { $percent }% { $state }, { $time } — click opens power
notif-battery-critical-body-unset = { $percent }% left. The machine will act on its own soon.
# WHAT AN INHIBITOR HOLDS — nouns, because "holds sleep" is a thing held and the
# lid's `sleep` above is an action; in Portuguese they are two words.
notif-battery-critical-body-ignore = { $percent }% left, and nothing will stop it. Plug in now.
echo-power-profile-not-offered = { $wanted } is not offered by this machine; { $applied } applied
# THE THREE ALARMS, raised by the footer when UPower's level changes.
notif-power-app = power
notif-bluetooth-app = bluetooth
notif-bluetooth-pairing-summary = Something wants to pair
notif-bluetooth-pairing-keys = { $no } says no, { $yes } says yes — after this card goes.
# WHAT THE MACHINE DID, said on the minibuffer's line (`events.rs`). The
# summary is the line; the body is only read when the line was busy and the
# event came as a notification instead — and then the last sentence says so.
notif-privacy-app = privacy
notif-network-app = network
notif-minibuffer-was-busy = This usually shows in the minibuffer, but it was busy, so it came as a notification.
notif-network-lost-summary = { $network } dropped
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

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

# The modeline's device marks, on the line under the pointer.
hint-bluetooth-device = { $kind }: { $name } — click opens bluetooth
hint-bluetooth-device-battery = { $kind }: { $name }, { $percent }% left — click opens bluetooth
keys-region-taken = another keyboard has the footer open; this one waits until it closes

## Screens — the monitors, their pictures and their light (LEADER s d)

display-no-light-here = No screen's brightness can be moved from here
state-keyboard-editing = one keyboard
cmd-keyboard-open = Keyboards: what each key does
# ---------------------------------------------------------------------------
# LEADER g — the glance: what time it is somewhere, and the weather and money.
# ---------------------------------------------------------------------------
world-place-europe-london = London
world-place-america-sao-paulo = Brasília
world-temperature = { $degrees } °C
world-rate = 1 { $from } = { $much } { $to }
cmd-world-glance = A glance: the time, the weather and the money
world-temperature-f = { $degrees } °F
audio-media-none = This build has no media player to talk to.
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
# =====================================================================
# LEADER d — development: tools for whoever builds this distro.
# =====================================================================
# The screen resizer (`LEADER / r`, `resizer.rs`): two curtains cut the
# screen, and everything is laid out in the room they leave. Its line on the
# bottom curtain says the room in pixels and in the grid, and each key's word
# is short because the line is one line; `about-` is what the pointer reads.
group-dev = development
hint-resizer = The screen is cut to { $width } × { $height } px — { $columns } columns by { $lines } lines. Click to have it whole again.
echo-resizer-on = The screen can be resized: drag its right edge, its bottom edge or the corner, or click a curtain and use the keys its line teaches.
echo-resizer-off = The screen is whole again.
echo-resizer-failed = The screen could not be resized: { $why }
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
window-menu-no = No
window-menu-yes-kill = Yes, force it to quit
window-menu-yes-close-others = Yes, close every other window
hint-window-menu = Click to open the options for everything that can be done to this window
software-nothing-to-open = Nothing was installed since the footer started.
notif-software-app = programs
hint-software-trial = { $count ->
        [one] A program is being tried, and nothing of it is installed. Click to see it.
       *[other] { $count } programs are being tried, and nothing of them is installed. Click to see them.
    }
hint-software-android = Android is running, and it costs memory while it runs. Click for Android.
notif-software-trial-ended-summary = { $app } closed
notif-software-trial-ended-body = Nothing of it was installed; it leaves the disk at the next clean-up.
notif-action-software-launch-latest = Open it
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
# THE BRAIN'S MODULES — what a notice answers to, on the strip under the
# minibuffer and on the brain's page; the page's notices, a watch, and the
# activity log folded.
cmd-brain-act = Do what it offers
cmd-brain-dismiss = Put it away
cmd-brain-snooze = Put it off
brain-band-dismiss = put away
brain-band-snooze = { $minutes } min
brain-band-more = +{ $count }
hint-brain-more = every notice, on the brain's page
echo-brain-no-notice = The brain has nothing to say right now
cmd-leader-set-default = The keys as they are, each opening its app in the footer
cmd-footer-bars-cycle = The next of the bars' three modes: shown, hidden and pushing the windows, hidden and over them
cmd-leader-close = Close the leader's list, as its Esc does
echo-bars-shown = Modeline and minibuffer shown
echo-bars-push = Modeline and minibuffer hidden; what opens pushes the windows away
echo-bars-cover = Modeline and minibuffer hidden; what opens lies over the windows
cmd-wm-fold-keys-hold = Let Ctrl with the arrows walk the minibuffer's line while a hint too long for it is on it
cmd-wm-fold-keys-free = Give Ctrl with the arrows back to the windows
# THE MAXIMUM ALERT BAR (alert.rs). The title is drawn in capitals.
alert-battery-low = Low battery!!!
alert-battery-low-body = { $percent }% left, on battery, and the charger is not in.
hint-alert-close = Closes this alert; it comes back if the battery falls another step
cmd-alert-dismiss = Close the alert bar; it comes back if what caused it gets worse
# THE FOOTER'S SETTINGS (LEADER s r, footer_settings.rs).
group-footer-settings = footer
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

## file info editing — everything the info panel can change

# Formatting with encryption (the format buffer, and its host).
# The format buffer's third question, as the modeline names its state.
state-encrypt = encryption
hint-security-until = The administrator asks for no password until { $time }; a click opens where it ends
fast-access-unopened = Nothing opens { $what }.
echo-capture-outside-the-room = Nothing was captured: the rectangle lay wholly under the resizer's curtains, outside the room it leaves.
# The file manager's state on the modeline (`files::View::state`): a question
# about who may touch a file, drawn in warning.
state-permission = permission
hint-security-no-limit = The administrator asks for no password until you end it; a click opens where it ends

## asked, above the modeline — who asks, what for, and what sudo waits for.

sudo-bar-title = Administrator
sudo-bar-password = Your password
sudo-bar-password-instead = Your password, instead
sudo-bar-touch = Touch your security key, or type your password
sudo-bar-finger = Place your finger on the reader, or type your password
sudo-bar-wrong = That was not your password. { $asked }
sudo-bar-checking = Checking…
sudo-bar-cancelling = Cancelling…
sudo-bar-answer = answer
sudo-bar-use-password = use the password
sudo-bar-cancel = cancel
about-sudo-bar-answer = Hands what you typed to sudo, for this gesture alone
about-sudo-bar-use-password = Stops waiting for the key or the finger, and asks your password
about-sudo-bar-cancel = Does nothing, and the screen that asked says so

## a secret asked in the same bar (crate::sudo_bar's ask) — by kind.

askpass-bar-title-ssh = SSH key
askpass-bar-title-sudo = System password
askpass-bar-ask-ssh = Passphrase for your SSH key
askpass-bar-ask-sudo = Your system password
askpass-bar-title-pin = Security key PIN
askpass-bar-ask-pin = PIN for your security key
askpass-bar-title-password = Login password
askpass-bar-ask-password = Password to log in
askpass-bar-title-touch = Security key
askpass-bar-ask-touch = Touch your security key
askpass-bar-title-host = Unknown host
askpass-bar-ask-host = Type yes to trust this host
askpass-bar-title-ssh-other = SSH
askpass-bar-ask-ssh-other = SSH is asking
