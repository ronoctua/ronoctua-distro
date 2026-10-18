# The texts `ronoctua-shell-pointers` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# =====================================================================
# POINTERS — `LEADER s i`: mice, touchpads and every cursor
# =====================================================================
group-pointers = pointers
pointers-title = Mice and touchpads
cmd-pointers-open = Mice and touchpads, in the footer
cmd-pointers-window = Mice and touchpads, in a window
cmd-pointers-up = Previous row
cmd-pointers-down = Next row
cmd-pointers-enter = Change this, or use it
cmd-pointers-less = The previous value
cmd-pointers-more = The next value
cmd-pointers-forget = Forget this device
pointers-up = up
pointers-down = down
pointers-enter = change
pointers-less = previous
pointers-more = next
pointers-forget = forget
pointers-heading-cursors = Cursors
pointers-heading-mice = Mice
pointers-heading-touchpads = Touchpads
pointers-heading-general = Every cursor
pointers-heading-away = Not connected
pointers-about-cursors = Every mouse and every touchpad can move a cursor of its own, each in its own colour, each focusing its own window. Put two devices on one cursor to have them move it together.
pointers-about-mice = What is set under “Every mouse” applies to each mouse that does not set it for itself.
pointers-about-touchpads = What is set under “Every touchpad” applies to each touchpad that does not set it for itself.
pointers-about-general = Focus follows the pointer for every cursor at once: the compositor has one setting for all of them.
pointers-about-away = Devices seen before, kept with their cursor and their settings for when they come back.
pointers-about-twins = Two devices of the same model have the same name to the compositor, so they always drive one cursor together.
pointers-detect = A new device gets its own cursor
pointers-together = Every mouse and touchpad moves one cursor
pointers-own-cursor = a cursor of its own
pointers-emulated = The virtual machine's own pointers move
pointers-about-emulated = The virtual machine's tablet and mice carry the host's pointer in. With the host's own mice handed to this machine they are off, so no cursor stands still; turn them on to take the pointer out of the window again.
pointers-not-drawn = not drawn: nothing on it moves
pointers-cursor = Cursor { $number }
pointers-cursor-on = This cursor is on
pointers-colour = Colour
pointers-colour-value = colour { $number }
pointers-on-cursor = Drives
pointers-on-cursor-value = cursor { $number }
pointers-new-cursor = a cursor of its own
pointers-every-mouse = Every mouse
pointers-every-touchpad = Every touchpad
pointers-device-on = This device is on
pointers-no-devices = No mouse or touchpad is connected.
pointers-no-compositor = The compositor is not answering, so no device can be read or set.
pointers-settings-failed = pointers.json could not be read, and is left as it is: the choices shown are the defaults until it is fixed or removed.
pointers-setting-events = Off while a mouse is connected
pointers-setting-speed = Speed
pointers-setting-profile = Acceleration
pointers-setting-natural = Natural scrolling
pointers-setting-left = Left-handed
pointers-setting-tap = Tap to click
pointers-setting-tap-map = Two and three fingers tap
pointers-setting-tap-drag = Tap and drag
pointers-setting-drag-lock = Drag survives lifting
pointers-setting-finger-drag = Drag with fingers
pointers-setting-click = Pressing clicks with
pointers-setting-click-map = Two and three fingers press
pointers-setting-middle = Left and right together are the middle button
pointers-setting-scroll = Scrolls with
pointers-setting-scroll-button = Scroll button
pointers-setting-scroll-lock = Scroll button stays pressed
pointers-setting-dwt = Off while typing
pointers-setting-dwtp = Off while the trackpoint is used
pointers-setting-factor = Scroll distance
pointers-size = Size
pointers-size-value = { $pixels } px
pointers-hide-after = Hide when still for
pointers-never = never
pointers-seconds = { $seconds } s
pointers-hide-typing = Hide while typing
pointers-focus = Focus follows the pointer
pointers-focus-no = no, a click focuses
pointers-focus-yes = yes
pointers-focus-always = always
pointers-about-typing = The coloured border marks the window the keyboards type into, and only cursor 1's click moves it. Choose “the last cursor to pick a window” and every cursor's click moves it — and the typing with it: the compositor draws the border for the keyboards' focus alone.
pointers-typing = Typing goes to
pointers-typing-first = cursor 1's window
pointers-typing-cursor = cursor { $number }'s window
pointers-typing-latest = the last cursor to pick a window
pointers-heading-keyboards = Keyboards
pointers-about-keyboards = In Independent mode every keyboard types through a cursor of its own, with its own layout and leader key. That is set on the keyboards' screen.
pointers-open-keyboards = Open the keyboard settings
pointers-flat = steady
pointers-adaptive = faster with a faster hand
pointers-lrm = right, then middle
pointers-lmr = middle, then right
pointers-lock-off = no
pointers-lock-on = briefly
pointers-lock-sticky = until the next tap
pointers-fingers-off = off
pointers-fingers-three = three fingers
pointers-fingers-four = four fingers
pointers-click-none = no buttons
pointers-click-areas = where it is pressed
pointers-click-fingers = how many fingers press
pointers-scroll-none = nothing
pointers-scroll-two = two fingers
pointers-scroll-edge = the edge
pointers-scroll-button = a held button
pointers-button-none = none
pointers-button-middle = the middle button
pointers-button-side = the side button
pointers-button-extra = the extra button
pointers-factor-value = × { $factor }
pointers-speed-value = { $speed }
pointers-is-first = the cursor the keyboards are on
