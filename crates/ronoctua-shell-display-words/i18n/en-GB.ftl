# The texts `ronoctua-shell-display-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Screens — the monitors, their pictures and their light (LEADER s d)

display-refused-no-compositor = there is no compositor to ask
display-refused-no-screen = that screen is not there any more
display-refused-no-light = this screen's brightness is on its own buttons
display-refused-not-writable = the file belongs to root, and the rule that opens it has not run on this panel
display-refused-nowhere = there is nowhere to write it down
display-refused-other = it did not work
echo-display-refused-plain = The screens refused: { $why }

## file info editing — everything the info panel can change

display-refused-compositor = the compositor answered with an error
display-refused-unreadable = the screens' file will not parse, and writing would replace it — fix or remove it
display-refused-panel = the panel would not take the brightness
display-refused-monitor = the monitor did not answer
