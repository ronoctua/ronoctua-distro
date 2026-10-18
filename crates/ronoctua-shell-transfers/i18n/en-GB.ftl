# The texts `ronoctua-shell-transfers` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# === The transfer queue — `LEADER t` =======================================
#
# Seven operations used to go off to a thread saying nothing on screen: paste,
# delete, shred, trash, compress, extract and measure. They all join one queue
# now, and it is that queue the modeline summarises and the panel details.
group-transfer = transfers
cmd-transfers-open = the transfer queue
transfers-give-up-no = No, let it carry on
transfers-give-up-yes = Yes, give it up
transfers-empty = nothing under way
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
transfers-window-field-identity = asked by
transfers-identity-mine = you
transfers-identity-anonymous = nobody
transfers-window-detail = the transfer, field by field
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
