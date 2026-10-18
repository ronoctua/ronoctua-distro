# The texts `ronoctua-shell-clipboard` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Compositor commands — §13, "every OS action has a command"

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
ask-clipboard-account = The account on that server
ask-clipboard-password = The password for that account
clipboard-pinned = pinned
clipboard-content = Content
clipboard-types = Types
clipboard-size = Size
echo-qr-large = The content is too large for a QR code
clipboard-image = Image
clipboard-unreadable = Copied, and unreadable here: { $kind }

## file info editing — everything the info panel can change

# The clipboard's window (`clipboard_window.rs`): the second screen of the
# footer's history, followed over the control socket.
group-clipboard = clipboard
cmd-clipboard-window = The clipboard's history in a window
clipboard-window-waiting = The history is the footer's, and it is on its way.
clipboard-window-no-footer = The footer is not running, and the history is kept there: nothing to show.
clipboard-no-room = There is no room left on screen to draw it: scroll back up, or give the list more lines.
