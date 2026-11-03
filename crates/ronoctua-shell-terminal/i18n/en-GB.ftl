# The texts `ronoctua-shell-terminal` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# THE TERMINAL'S WINDOW (`crate::terminal`): its three identities' titles,
# until the program names the window itself.
terminal-title = Terminal
terminal-title-monitor = Monitor
terminal-title-rescue = Rescue console

# WHERE THE TERMINAL LIVES, said on the window's line when it is not where
# a terminal lives: with no scope of its own it runs in the window's process,
# and dies with the window.
terminal-in-process = This terminal lives in its window: the session gave it no scope of its own, so closing the window ends its program
# A terminal left running by another build of the system (a switch since):
# its screen comes in a form this window cannot decode yet.
terminal-other-build = This terminal was started by another build of the system, and this window cannot read its screen

# A PASTE THAT COULD RUN A COMMAND (a line break the program would take for
# Enter): asked, the no first.
terminal-paste-unsafe = This paste could run a command as it lands
terminal-paste-refuse = Leave it out
terminal-paste-allow = Paste it

# A PASTE THE TERMINAL DID NOT WRITE, said on the window's line: the window
# judged it by its copy of the program's modes, the terminal by its own,
# which the program may have changed a moment before — or the terminal
# could not take it at all.
terminal-paste-refused-unsafe = The terminal did not take this paste: its program changed how it reads one, and the paste could run a command as it lands
terminal-paste-refused = The terminal could not take this paste

# A PROGRAM ASKING TO READ THE CLIPBOARD (OSC 52): asked, the no first.
terminal-clipboard-read = { $program } asks to read the clipboard
terminal-clipboard-read-unnamed = The program asks to read the clipboard
terminal-clipboard-refuse = Refuse
terminal-clipboard-allow = Let it read

# WHAT A COPY SAID (`C-S-c`).
terminal-copied = { $count ->
    [one] One character copied
   *[other] { $count } characters copied
  }
terminal-nothing-selected = Nothing is selected to copy

# NO TERMINAL COULD START, and the window has nothing else to show: said for
# as long as a refusal is, and then the window closes.
terminal-could-not-start = The terminal could not start; this window closes
# A NEW TERMINAL of the group (`C-S-t`) could not start: the others stay.
terminal-new-failed = A new terminal could not start
# `C-S-RET` ON A TERMINAL OF THE WINDOW'S OWN PROCESS: another window cannot
# show what lives and dies in this one.
terminal-detach-here = This terminal lives in this window and cannot have a window of its own

# A LINK UNDER `Ctrl` (OSC 8, or an address written as text), opened with a
# click — or not: a scheme this system does not open, or no opener at all.
terminal-link-refused = This link is not one this system opens
terminal-link-no-opener = Nothing here opens links

# THE SEARCH (`C-S-f`), over the last row: what it is, what was found, and
# its three clicks — the next match toward older output, toward newer, and
# the end. `$at` and `$total` are numbers.
terminal-search = Find
terminal-search-count = { $at } of { $total }
terminal-search-none = No match
terminal-search-older = Older
terminal-search-newer = Newer
terminal-search-close = Close

# THE GROUP'S COMMANDS (`LEADER :` and `LEADER Alt`), the window's own keys.
cmd-terminal-new = A new terminal, beside the one in front
cmd-terminal-next = The next terminal of this window
cmd-terminal-previous = The previous terminal of this window
cmd-terminal-detach = The terminal in front, in a window of its own

# THE TERMINALS NO WINDOW SHOWS (`crate::terminals`, `LEADER RET b`): the
# group of the leader, the view's title, its commands and the words its hint
# teaches them by.
group-terminals = background
terminals-title = Terminals with no window
cmd-terminals-open = The terminals with no window, in the footer
cmd-terminals-window = The terminals with no window, in a window
cmd-terminals-up = The previous terminal
cmd-terminals-down = The next terminal
cmd-terminals-enter = Show this terminal, in a window with the rest of its group
cmd-terminals-end = End this terminal — asked first
terminals-up = up
terminals-down = down
terminals-enter = show
terminals-end = end
# Its headings: the terminals that were alone in their windows, and each
# window's group — numbered, so two windows of the same size read apart.
# `$number` and `$count` are numbers.
terminals-alone = Alone in their windows
terminals-group = Window { $number }, with { $count } terminals
# Nothing in the background.
terminals-none = Every terminal has a window: none is in the background.
# How long a terminal has been without a window. Numbers.
terminals-since-moments = just now
terminals-since-minutes = { $minutes } min
terminals-since-hours = { $hours } h
terminals-since-days = { $days } d
# What the program in it says it is doing (OSC 7501), after its glyph.
terminals-working = working
terminals-blocked = waiting for you
terminals-done = done
terminals-idle = ready
terminals-failed = failed
# What it asked for while nobody was looking, when its program says nothing.
terminals-rang = rang
terminals-notified = sent a notification
# Its program ended with no window to see it: kept until it is shown once.
# `$code` is a number.
terminals-ended = ended
terminals-ended-code = ended with { $code }
# ENDING ONE (`d`), asked first, the no first: one whose program runs, and
# one whose program ended already. `$name` is what its row calls it.
terminals-end-question = End { $name }? Its program is hung up, and what it wrote goes with it.
terminals-close-question = Close { $name }? What it wrote goes with it.
terminals-answer-no = No, keep it
terminals-answer-yes = Yes, end it
# What ending one is said as, and why it could not be.
terminals-ending = Ending { $name }
terminals-not-ended = { $name } could not be ended: no thread could be started to tell it
terminals-no-registry = This session keeps no held terminals: it has no runtime folder

# THE TERMINAL'S SETTINGS (`crate::terminal_settings`, `LEADER RET s`), in
# `terminal.json`: the group of the leader, the view's title and commands,
# the words its hint teaches them by, its two sections, its rows and every
# value of each ring.
group-terminal-settings = settings
terminal-settings-title = Terminal settings
cmd-terminal-settings-open = The terminal's settings, in the footer
cmd-terminal-settings-window = The terminal's settings, in a window
cmd-terminal-settings-up = The previous setting
cmd-terminal-settings-down = The next setting
cmd-terminal-settings-less = This setting's previous value
cmd-terminal-settings-more = This setting's next value
cmd-terminal-settings-enter = Turn this setting to its next value
terminal-settings-up = up
terminal-settings-down = down
terminal-settings-less = less
terminal-settings-more = more
terminal-settings-enter = change
terminal-settings-new = For every new terminal
terminal-settings-windows = In every terminal window
terminal-settings-new-note = A new terminal takes the first three as it starts; one that runs keeps what it started with, and a program in it may ask for another cursor.
terminal-settings-windows-note = Every terminal window takes the last two at once, and a window's own zoom is counted from this size.
terminal-settings-scrollback = History each one keeps
terminal-settings-cursor = Its cursor
terminal-settings-blinks = The cursor blinks
terminal-settings-copy = What is selected is copied
terminal-settings-size = Size of the text
# `$megabytes` and `$percent` are numbers.
terminal-settings-megabytes = { $megabytes } MB
terminal-settings-cursor-block = a block
terminal-settings-cursor-bar = a bar
terminal-settings-cursor-underline = an underline
terminal-settings-yes = yes
terminal-settings-no = no
terminal-settings-copy-primary = to what a middle click pastes
terminal-settings-copy-clipboard = to the clipboard
terminal-settings-copy-both = to both
terminal-settings-copy-off = nowhere
terminal-settings-size-system = the system's ({ $percent } %)
terminal-settings-size-own = { $percent } %
# Nothing of the file read: said under the values, which are the defaults.
terminal-settings-unread = { $file } does not read as written: what is shown is the defaults, and nothing is written over it until it reads.
# A change that did not land, said where it was made.
terminal-settings-unparsed = { $file } does not read as written; nothing was changed over it
terminal-settings-not-written = { $file } could not be written: { $why }
