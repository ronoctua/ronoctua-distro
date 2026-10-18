# The texts `ronoctua-shell-display` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Screens — the monitors, their pictures and their light (LEADER s d)

cmd-display-open = Screens
cmd-display-window = Screens in a window
cmd-display-down = Down one row
display-back = back
cmd-display-up = Up one row
cmd-display-less = The value under the cursor, back one
cmd-display-more = The value under the cursor, on one
cmd-display-enter = Open, copy or answer
cmd-display-automatic = Let the compositor choose this setting
cmd-display-forget = Forget what was chosen for this screen
cmd-display-arrange = Where the screens sit
group-display = Screens
display-down = down
display-up = up
display-less = less
display-more = more
display-leftwards = leftwards
display-rightwards = rightwards
display-enter = open
display-enter-answers = answer
display-enter-copies = copy
display-automatic = let it choose
display-forget = forget
display-arrange = arrange
display-title = Screens
display-heading-screens = Screens
display-heading-every = All of them
display-heading-picture = The picture
display-heading-light = The light
display-heading-session = The session
display-heading-arrangement = Where they sit
display-heading-forgetting = Forget this screen's settings?
display-heading-keeping = Keeping this?
display-nothing-here = No screen is attached.
display-reading = Asking the compositor…
display-no-mode = the compositor reports none
display-no-thread-short = the screens cannot be read here
display-no-watch = this door cannot see the file change
display-no-thread = The screens cannot be read on this machine; nothing here will change.
display-no-compositor = There is no compositor to ask, so there is nothing to show.
display-went-away = That screen is gone.
display-settings-failed = `display.json` will not parse, so nothing chosen is shown here. The file is untouched — fix or remove it.
display-one-screen-only = There is one screen, so there is nothing to arrange.
display-this-one = { $name } — this one
display-arrangement = Arrangement
display-text-size = Size of the text
# Under the cursor on the size row: what a change reaches at once, and what
# waits. It names what was MEASURED — see docs/PENDING.md — and never promises
# an application nobody watched.
display-text-reaches = the shell and its windows follow at once; GTK and Qt applications follow as they redraw
display-text-reopens = { $names } keep their size until reopened
display-fact-connector = Socket
display-fact-filed = Its settings follow
display-fact-size = Screen
display-filed-by-itself = the monitor, wherever it is plugged in
display-filed-silent = the socket — the monitor says nothing about itself
display-filed-no-serial = the socket — the monitor gives no serial, so an identical one would answer for it
display-on = Driven
display-resolution = Resolution
display-rate = Refresh rate
display-scale = Scale
display-turn = Turn
display-place = Place
display-vary = Varying refresh
display-brightness = Brightness
display-first = Opens focused here
display-vary-cannot = the hardware cannot
display-vary-always = always
display-vary-on-demand = only when a window asks
display-light-is-roots = the file belongs to root — the rule that opens it has not run here
display-light-on-its-buttons = This monitor's brightness is on its own buttons.
display-turn-and-picture = { $set } (the picture is { $picture })
display-turn-normal = normal
display-turn-rotated-90 = a quarter turn
display-turn-rotated-180 = upside down
display-turn-rotated-270 = three quarters
display-turn-flipped = mirrored
display-turn-flipped-90 = mirrored, a quarter turn
display-turn-flipped-180 = flipped vertically
display-turn-flipped-270 = mirrored, three quarters
display-value-yes = yes
display-value-no = no
display-value-off = not driven
display-value-automatic = the compositor chooses
display-value-percent = { $percent }%
display-value-rate = { $hz } Hz
display-value-inches = { $inches } inches
display-value-place = x={ $x } y={ $y }
display-value-screens = { $n ->
        [0] no screens
        [one] one screen
       *[other] { $n } screens
    }
display-left-of = to the left of { $other }
display-right-of = to the right of { $other }
display-above = above { $other }
display-below = below { $other }
display-arrange-how = The arrows move the screen under the cursor to the next edge of a neighbour — beside it, lined up with it — and Shift with ↑ or ↓ puts it above or below.
display-ask = { $what } on { $screen }.
display-ask-seconds = It goes back on its own in { $seconds }s.
display-ask-forget = Everything chosen for { $screen } goes back to what the compositor decides.
display-ask-nothing-yet = Nothing has changed yet.
display-leave-it = Leave it as it is
display-forget-it = Forget it all
display-place-and-picture = { $set } (it is at { $picture })
display-scale-and-picture = { $set } (the picture is { $picture })
display-screen-gone = { $screen } is not there any more; the change went with it.
display-screen-gone-alone = { $screen } is not there any more; nothing had been changed.
display-only-one-lit = yes — and the only screen lit
display-dark-not-ours = no — and nothing here turned it off
display-only-one-offered = the only one this monitor offers
display-put-it-back = Put it back
display-keep-it = Keep it
display-what-power-off = The screen was turned off
display-what-resolution = The resolution changed
display-what-rate = The refresh rate changed
display-what-scale = The scale changed
display-what-turn = The picture was turned

## file info editing — everything the info panel can change

# The screens (`display.rs`): the vertical arrangement, the render depth, a
# dark screen's filed values, and every refusal named.
display-rise = put above
display-sink = put below
cmd-display-rise = The screen under the cursor up, above its neighbour
cmd-display-sink = The screen under the cursor down, below its neighbour
display-depth = Colour depth
display-depth-bits = { $bits } bits per colour
display-depth-asked = Colour depth asked for
display-dark-shows-filed = The compositor is not drawing on this screen: what each row shows is what is filed for it, and comes back when it lights.
