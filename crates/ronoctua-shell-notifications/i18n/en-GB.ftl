# The texts `ronoctua-shell-notifications` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Notifications

notif-title = Notifications
# THE FILTERS THE DISTRO SHIPS WITH. The manager lists them by these names.
notif-rule-important-gets-room = Something important gets room
notif-rule-bluetooth-connected-is-quiet = A Bluetooth device connecting is said by its mark
notif-rule-bluetooth-disconnected-is-quiet = A Bluetooth device leaving is said by its mark
notif-rule-small-things-are-quick = Small things go by quickly
# The manager: three lists, and a heading each.
notif-column-happened = Received
notif-column-suppressed = Filtered out
notif-column-catalogue = Notifications and filters
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
notif-adjust = adjust this kind
notif-where = { $at } of { $total }
notif-up = up
notif-down = down
notif-pane-next = next column
notif-pane-previous = previous column
notif-knob-next = next value
notif-knob-previous = previous value
notif-knob-turn = turn it
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
notif-glyph-read = Mark it read
notif-glyph-unread = Mark it unread again
notif-glyph-delete = Forget it for good

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
cmd-notif-knob-turn = Turn it to the next value; after the last comes the first
cmd-notif-restore = Back to the distro's default
cmd-notif-dismiss = Dismiss notification
group-notification = notification
# THE NOTIFICATION MANAGER, where one row means something else.
about-notif-edit = Open this filter of your own to be rewritten: its name, its conditions and what it does.
