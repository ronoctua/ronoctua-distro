# The texts `ronoctua-shell-manual` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Help — the shortcut manual (LEADER h s)

cmd-help = Manuals
manual-list = MANUALS
manual-window-lead = MANUAL:
manual-next = next
manual-previous = previous
manual-open = open
manual-window = in a window
manual-down = down
manual-up = up
about-manual-next = The next manual of the list.
about-manual-previous = The previous manual of the list.
about-manual-open = Opens the manual under the cursor, here, in place of the list.
about-manual-window = Opens the manual in a window of its own, and takes it out of the footer.
about-manual-down = Moves the manual one line down.
about-manual-up = Moves the manual one line up.
manual-none = No manual matches what was typed.
manual-filter-empty = Type to filter the manuals.
manual-shortcuts = Shortcuts
manual-gestures = Hand gestures
about-manual-shortcuts = Every key the system answers: holding the leader, the compositor's modes, the emergency and media keys.
about-manual-gestures = Every hand gesture the camera reads, drawn, with what it does.
gesture-point-title = Move the pointer
gesture-click-title = Left click, and drag
gesture-right-click-title = Right click
gesture-scroll-title = Scroll
gesture-arrows-title = Arrow keys
gesture-escape-title = Esc
gesture-focus-title = Move the focus
gesture-workspace-title = Change workspace
gesture-overview-title = Every workspace
gesture-keyboard-title = On-screen keyboard
gesture-player-title = The player's column
gesture-close-title = Close the window
gesture-off-title = Turn gestures off
help-gestures = HAND GESTURES, SEEN BY THE CAMERA
gesture-point = Index and middle
gesture-point-does = Index and middle raised together, the thumb folded: the pointer follows the index.
gesture-click = Pointer + open hand
gesture-click-does = Pointing, the OTHER hand open with its fingers apart: left button, held down for as long as it stays open.
gesture-right-click = Pointer + hang loose
gesture-right-click-does = Pointing, the other hand with only the thumb and little finger out: one right click.
gesture-scroll = Three up, moving
gesture-scroll-does = One hand, thumb, index and middle raised: moving it up or down scrolls.
gesture-arrows = Both index fingers
gesture-arrows-does = Both index fingers side by side, pointing one way: that arrow key.
gesture-escape = Index fingers crossed
gesture-escape-does = Both index fingers crossed in an X: Esc.
gesture-focus = Open hand, swept
gesture-focus-does = The five fingers raised and apart: sweeping the hand moves the focus that way. Coming back moves nothing.
gesture-workspace = Both hands, three up
gesture-workspace-does = Both hands with thumb, index and middle pointing up: the workspace above; pointing down: the one below.
gesture-overview = Ten fingers
gesture-overview-does = Both hands open, side by side: every workspace at once.
gesture-keyboard = Hands stacked
gesture-keyboard-does = Both hands open, one above the other: the on-screen keyboard.
gesture-player = Rock sign
gesture-player-does = Index and little finger raised: opens the player's column, and closes it again.
gesture-close = Both middle fingers
gesture-close-does = Both middle fingers raised: closes the window in focus.
gesture-off = Arms crossed
gesture-off-does = The arms crossed in an X: turns gestures off and lets the camera go. The only gesture that does, on purpose.
help-chords = HOLDING THE LEADER
help-emergency = WHEN THE LEADER DOES NOT ANSWER
help-leader-alternate = Open the command list
help-media = The keyboard's own keys
