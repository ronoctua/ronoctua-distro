# The texts `ronoctua-shell-overlays` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Installer — the interface

cmd-screen-capture = Screenshot
capture-drag = drag for a region
capture-click = click a window
capture-whole = the whole screen
about-capture-whole = Keeps the whole of the screen with the focus, as it was when the capture began.
capture-loupe = the magnifier
about-capture-loupe = Shows or hides the magnifier beside the pointer: the pixels under it, enlarged, with the one it is on outlined.
capture-hide-hint = hide this line
about-capture-hide-hint = Puts this line of shortcuts away, wherever the pointer is; the same key brings it back.
capture-cancel = cancel
about-capture-cancel = Gives the capture up and keeps nothing.
capture-size = { $width } × { $height }
capture-at = { $x }, { $y }
cmd-dev-resizer = Resize the whole screen
resizer-size = { $width } × { $height } px
resizer-grid = { $columns } columns × { $lines } lines
resizer-off = whole screen
about-resizer-off = Ends the resizing: the curtains go and the screen is whole again.
resizer-narrower = narrower
about-resizer-narrower = One column narrower, or a stride of them with Shift held.
resizer-wider = wider
about-resizer-wider = One column wider, or a stride of them with Shift held.
resizer-shorter = shorter
about-resizer-shorter = One line shorter, or a stride of them with Shift held.
resizer-taller = taller
about-resizer-taller = One line taller, or a stride of them with Shift held.
resizer-largest = largest
about-resizer-largest = As large as the curtains allow — everything but the two strips they keep to be grabbed.

## file info editing — everything the info panel can change

# The resizer's ready sizes and its memory (`resizer.rs`): `p` walks the
# rooms of the commonest screens, and a resizer turned on again starts at
# the size it was left at.
resizer-preset = ready size
about-resizer-preset = The next ready size of a common screen that fits this one.
resizer-preset-back = smaller
about-resizer-preset-back = The ready size before this one.
resizer-no-preset = no ready size fits this screen
