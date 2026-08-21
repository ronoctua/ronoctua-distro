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

group-buffer = buffer
group-quit = quit

## Modeline states

mode-normal = NORMAL
buffer-no-name = *no name*
mode-unknown = unknown

## Notifications

notif-empty = no notifications
notif-history = Notifications
notif-critical = CRITICAL

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
echo-no-on = { $sequencia } is not bound
# Same sequence, other case. Shown when the case IS the reason nothing
# matched — Caps Lock or Shift. Without it the screen names the consequence
# and hides the cause.
echo-no-on-box = { $sequencia } is not bound — { $outra } is
echo-cancelled = Quit
echo-no-window = No focused window
echo-no-action = { $comando } does nothing yet
echo-no-buffer = The glyph browser was not built
echo-no-opened = Could not open { $programa }
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

cmd-notif-history = Notification history
cmd-notif-dismiss = Dismiss notification
cmd-keyboard-restore = Restore the keyboard
group-notification = notification
echo-keyboard-restored = Keyboard restored
echo-keyboard-failed = Could not restore the keyboard: { $motivo }

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
expansion-page = RCtrl ←→

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

## Network — Wi-Fi and Ethernet (LEADER r)

cmd-network-open = Networks
cmd-network-scan = Scan networks
cmd-network-connect = Connect to network
cmd-network-disconnect = Disconnect
cmd-network-forget = Forget network
cmd-network-wifi-toggle = Toggle Wi-Fi radio
cmd-network-link = Show the link
group-network = network

# THE VIEW. States the list shows in place of rows when there is no row to
# show — each says something different, and folding them into one generic
# "no networks" would hide the difference between not finding and not looking.
network-scanning = scanning…
network-empty = no networks in range
network-no-adapter = this machine has no Wi-Fi radio
network-radio-off = radio off
network-connecting = connecting…
network-wired = wired
network-saved = saved
network-open = open
network-password = Password for { $rede }
network-password-help = Kept by NetworkManager, outside the repository.

# ECHOES — what the minibuffer says after the action.
echo-network-connected = Connected to { $rede }
echo-network-failed = Could not connect to { $rede }: { $motivo }
echo-network-disconnected = Disconnected
echo-network-forgotten = { $rede } forgotten
echo-network-radio-on = Wi-Fi radio on
echo-network-radio-off = Wi-Fi radio off
echo-network-link = { $rede } · { $banda } · { $taxa } · signal { $sinal }
echo-network-link-wired = { $rede } · { $taxa }
echo-network-no-link = Not connected
echo-network-unavailable = NetworkManager is not responding

## Syncthing (LEADER r s)

cmd-syncthing-open = Open Syncthing
cmd-syncthing-state = Syncthing status
cmd-syncthing-pause = Pause syncing
cmd-syncthing-resume = Resume syncing
group-syncthing = syncthing
syncthing-line = Syncthing
syncthing-idle = up to date
syncthing-syncing = syncing { $porcento }%
syncthing-stopped = stopped
echo-syncthing-state = { $conectados } connected · { $porcento }% · ↑{ $envio } ↓{ $recepcao }
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
network-remember = Remember { $rede }?  s = yes · n = no
# The list marker: tells "saved in NetworkManager" apart from "the password exists".
network-no-tpm = in TPM
echo-network-stored = Password for { $rede } sealed in the TPM
echo-network-no-stored = Connected. The password was not stored.
echo-network-store-failed = Connected, but I could not store the password: { $motivo }
## Out of memory
notif-oom-app = system
notif-oom-summary = Closed to free memory
notif-oom-pressure = { $app } was stalling the machine for want of memory.
notif-oom-used = { $app } filled memory and swap.
notif-oom-orphan = Something outside your buffers was closed: { $cgroup }

## The focused buffer's keymap
which-buffer = Focused buffer
cmd-exemplo-reiniciar = Restart the example

## The memory panel — LEADER s m

cmd-memory-open = System memory
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
rescue-generated-by = Generated by { $origem }. Editing here disappears on the next rebuild.
rescue-no-windows = The compositor did not answer — or there is no window at all.
rescue-empty = nothing to show
rescue-back = Leaving the terminal gives this screen back.
rescue-generation = generation { $numero }
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

# The view's own keys — navigation and marking. They live in a table
# (`OF_THE_VIEW`) rather than inside a `match`, because the list is what the
# manual and the check read: a key that only exists in a `match` arm is a key
# nothing can enumerate.
files-down = down
files-up = up
files-enter = open, or enter the directory
files-parent = up one directory
files-hint = type to filter · { $key } opens the action menu
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
files-hidden = show or hide the hidden ones
files-sort = change the sort
files-sort-reverse = reverse the order
files-mark = mark, and move on
files-mark-all = mark everything in sight
files-unmark-all = unmark everything
files-first = go to the first
files-last = go to the last
files-menu = action menu
files-measure = measure the directory, or examine the file

files-type-is = { $named }
files-type-disagrees = the name says { $named }, the content says { $found }

cmd-files-open = File manager
cmd-files-window = File manager, in a window
group-file = file

files-nothing-here = this directory is empty
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
cmd-transfers-add-media = download whatever is on the clipboard, as media
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
