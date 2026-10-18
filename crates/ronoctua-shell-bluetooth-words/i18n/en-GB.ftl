# The texts `ronoctua-shell-bluetooth-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

ask-bluetooth-rename = A name for it
bluetooth-device = open
bluetooth-rename = rename
bluetooth-power = radio
bluetooth-discoverable = visible
bluetooth-title = Bluetooth
bluetooth-no-daemon = the Bluetooth daemon is not answering
# THE KINDS, from the device's own class or appearance.
bluetooth-kind-headphones = headphones
bluetooth-kind-headset = headset
bluetooth-kind-speaker = speaker
bluetooth-kind-keyboard = keyboard
bluetooth-kind-mouse = mouse
bluetooth-kind-gamepad = gamepad
bluetooth-kind-tablet = tablet
bluetooth-kind-phone = phone
bluetooth-kind-computer = computer
bluetooth-kind-watch = watch
bluetooth-kind-camera = camera
bluetooth-kind-printer = printer
bluetooth-kind-network = network
bluetooth-kind-display = display
bluetooth-kind-unknown = device
# PAIRING — the question the other side asks, and the two answers.
bluetooth-pairing-confirm = { $name } wants to pair. Does it show { $code }?
bluetooth-pairing-passkey = type { $code } on { $name }
bluetooth-pairing-pin = { $name } wants a PIN
bluetooth-pairing-passkey-here = { $name } wants six digits typed here
bluetooth-pairing-authorize = { $name } wants to pair
bluetooth-pairing-service = { $name } wants to use { $service }
# WHAT WENT WRONG, said as the name of the door that shut.
echo-bluetooth-refused = { $what }: { $why }
bluetooth-what-connect = the connection
bluetooth-what-pair = the pairing
bluetooth-what-forget = forgetting the device
bluetooth-what-radio = the radio
bluetooth-what-device = the device
bluetooth-refused-authority = this session is not allowed
bluetooth-refused-absent = there is no such thing on this machine
bluetooth-refused-away = nobody answered
bluetooth-refused-busy = something else is doing it
bluetooth-refused-rejected = the other side said no
bluetooth-refused-other = it did not work

## file info editing — everything the info panel can change

bluetooth-refused-timed-out = it got no answer in time
bluetooth-refused-no-bus = the system bus could not be reached
