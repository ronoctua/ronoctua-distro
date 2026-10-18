# The texts `ronoctua-shell-syncthing` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

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
syncthing-title = Syncthing
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

## file picker — the one picker of a file, a folder or a name to save under

syncthing-pick-folder = A folder on this machine
