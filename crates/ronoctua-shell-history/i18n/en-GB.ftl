# The texts `ronoctua-shell-history` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

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
