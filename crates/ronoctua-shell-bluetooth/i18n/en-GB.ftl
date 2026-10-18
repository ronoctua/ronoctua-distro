# The texts `ronoctua-shell-bluetooth` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

cmd-bluetooth-open = Bluetooth
cmd-bluetooth-window = Bluetooth in a window
cmd-bluetooth-scan = Look for devices nearby
cmd-bluetooth-power = Turn the radio on or off
cmd-bluetooth-discoverable = Let other devices see this machine
cmd-bluetooth-reconnect = Reconnect the trusted devices
cmd-bluetooth-disconnect-all = Disconnect every device
cmd-bluetooth-accept = Say yes to the pairing
cmd-bluetooth-refuse = Say no to the pairing
cmd-bluetooth-device = Open the device under the cursor
cmd-bluetooth-connect = Connect or disconnect the device
cmd-bluetooth-pair = Pair with the device
cmd-bluetooth-forget = Forget the device
cmd-bluetooth-trust = Trust the device, or stop trusting it
cmd-bluetooth-block = Block the device, or unblock it
cmd-bluetooth-rename = Give the device another name
cmd-bluetooth-unnamed = Show the devices that give no name
cmd-bluetooth-up = Up one row
cmd-bluetooth-down = Down one row
group-bluetooth = bluetooth
# THE HINT LINE reads each id of the view's key table as a short text.
bluetooth-down = down
bluetooth-up = up
bluetooth-connect = connect
bluetooth-pair = pair
bluetooth-forget = forget
bluetooth-trust = trust
bluetooth-block = block
bluetooth-scan = scan
bluetooth-unnamed = unnamed
bluetooth-accept = yes
bluetooth-refuse = no
bluetooth-row-kind = { $name } — { $kind }
bluetooth-audio-state = { $what } — { $state }
# THE THREE SECTIONS, and what an empty one says.
bluetooth-heading-adapter = Radio
bluetooth-heading-paired = Paired
bluetooth-heading-nearby = Nearby
bluetooth-nothing-here = nothing here
bluetooth-no-adapter = there is no Bluetooth radio on this machine
bluetooth-scanning = looking for devices
bluetooth-unnamed-hidden = { $count } nearby with no name — press h to show them
# THE RADIO'S OWN ROWS.
bluetooth-adapter-power = the radio
bluetooth-adapter-discoverable = other devices can see this machine
bluetooth-adapter-pairable = other devices can pair with it
bluetooth-adapter-timeout = it stops being visible after
bluetooth-adapter-forever = it stays visible
bluetooth-adapter-pairable-forever = it stays open
bluetooth-adapter-alias = this machine is called
bluetooth-power-state-on = on
bluetooth-power-state-off = off
bluetooth-power-state-off-enabling = turning on
bluetooth-power-state-on-disabling = turning off
bluetooth-power-state-off-blocked = blocked by the hardware switch
# A DEVICE'S FACTS, one per line, in the order the detail draws them.
bluetooth-fact-address = address
bluetooth-fact-address-type = address type
bluetooth-fact-alias = called
bluetooth-fact-name = its own name
bluetooth-fact-connected = connected
bluetooth-fact-paired = paired
bluetooth-fact-bonded = the keys were kept
bluetooth-fact-trusted = trusted
bluetooth-fact-blocked = blocked
bluetooth-fact-legacy = old-style pairing
bluetooth-fact-wake = it may wake this machine
bluetooth-fact-services = services read
bluetooth-fact-battery = battery
bluetooth-fact-battery-source = the battery is reported by
bluetooth-fact-rssi = signal
bluetooth-fact-tx-power = it transmits at
bluetooth-fact-class = class
bluetooth-fact-appearance = appearance
bluetooth-fact-modalias = firmware id
bluetooth-fact-adapter = through the radio
bluetooth-fact-reconnect = who reconnects
bluetooth-fact-volume = volume it reports
bluetooth-fact-codec = codec
bluetooth-fact-audio = audio profile
bluetooth-yes = yes
bluetooth-no = no
bluetooth-rssi-value = { $dbm } dBm
bluetooth-tx-power-value = { $dbm } dBm
bluetooth-percent = { $percent }%
bluetooth-seconds = { $seconds }s
# BLUEZ'S OWN ENUMERATIONS, in the person's language. Closed lists, not
# free-form data — a device's NAME stays the device's. `word_or` falls back to
# the daemon's word if it grows a value, so a new one is untranslated and never
# `[a key in brackets]`.
bluetooth-adapter-pairable-timeout = open to pairing for
bluetooth-fact-manufacturer = maker
bluetooth-fact-version = bluetooth version
bluetooth-fact-roles = roles
bluetooth-fact-service = offers
bluetooth-audio-profile = what it is doing
bluetooth-heading-settings = Other settings
bluetooth-setting-volume = volume
bluetooth-setting-mute = silenced
bluetooth-nothing-to-adjust = nothing to adjust here
bluetooth-settings = the other half
bluetooth-more = more
bluetooth-less = less
cmd-bluetooth-settings = Bluetooth: cross to the other half of the page
cmd-bluetooth-more = Bluetooth: more of whatever the cursor is on
cmd-bluetooth-less = Bluetooth: less of whatever the cursor is on
bluetooth-audio-off = off
bluetooth-audio-a2dp-sink = music, in stereo
bluetooth-audio-a2dp-source = music, from it
bluetooth-audio-headset-head-unit = a call, with its microphone
bluetooth-audio-headset-audio-gateway = a call, through this machine
bluetooth-codec-0 = SBC
bluetooth-codec-1 = MPEG-1/2 audio
bluetooth-codec-2 = AAC
bluetooth-codec-4 = aptX
bluetooth-codec-255 = the maker's own
bluetooth-address-type-public = fixed
bluetooth-address-type-random = random
bluetooth-reconnect-none = never
bluetooth-reconnect-host = when this machine asks
bluetooth-reconnect-device = when the device asks
bluetooth-reconnect-any = either side
bluetooth-transport-idle = idle
bluetooth-transport-pending = starting
bluetooth-transport-active = playing
bluetooth-transport-suspending = stopping
bluetooth-transport-broadcasting = broadcasting
# WHAT A DEVICE CAN DO — the profiles behind its service ids.
bluetooth-profile-headset = calls, with its microphone
bluetooth-profile-handsfree = hands-free calls
bluetooth-profile-audio-sink = plays this machine's sound
bluetooth-profile-audio-source = sends sound to this machine
bluetooth-profile-remote-control = its buttons control playback
bluetooth-profile-human-interface = sends keystrokes and pointer moves
bluetooth-profile-file-transfer = browsing its files
bluetooth-profile-object-push = receiving files sent to it
bluetooth-profile-phone-book = reading a phone's contacts
bluetooth-profile-message-access = reading a phone's messages
bluetooth-profile-network = sharing its internet connection
bluetooth-profile-battery = reporting how much charge it has
bluetooth-profile-device-information = reporting its maker and model
bluetooth-profile-serial = a serial port over the air
bluetooth-profile-sim-access = reading a phone's SIM card
bluetooth-fact-kind = what it is
bluetooth-heading-profiles = The services it announces
bluetooth-about-profile = { $id } — { $said }
bluetooth-about-profile-unknown = { $id } — an id of the maker's own, with no published meaning. It is drawn as it is, which is what a search engine takes.
bluetooth-about-audio-profile = Which way the sound goes: stereo music, or a call with the microphone. PipeWire decides this, not the device.
bluetooth-about-sink-volume = How loud this machine plays through it. RET turns it up a step; the arrows move it either way.
bluetooth-about-mute = Silences it without losing the number, so turning it back gives the same volume.
bluetooth-door-this-keyboard = settings for this keyboard
bluetooth-door-all-keyboards = settings for all keyboards
bluetooth-about-this-keyboard = Opens the keyboard settings with this one selected, where a key can be made to mean another.
bluetooth-about-all-keyboards = Opens the keyboard settings, which list every keyboard this machine has.
# WHAT A ROW MEANS, for the message line the pointer asks on.
bluetooth-about-adapter = Which radio of this machine it is paired through. A machine with one Bluetooth chip has one.
bluetooth-about-address = Its hardware address — the six bytes that identify this unit, and what settings are remembered against.
bluetooth-about-address-type = A public address belongs to the maker for good; a random one is generated, and some devices change theirs to avoid being followed.
bluetooth-about-alias = The name YOU gave it. It is what every screen here shows; the device's own name is the row below.
bluetooth-about-appearance = A number the device sends to say what shape it is — 961 is a keyboard, 962 a mouse. Low-energy devices send this instead of a class.
bluetooth-about-audio = What the sound link is doing right now, as the daemon reports it.
bluetooth-about-battery = How much charge it says it has. Not every device reports one, and none of them is precise.
bluetooth-about-battery-source = Who is reporting that charge: the standard battery service, or the maker's own.
bluetooth-about-blocked = Blocked means this machine refuses it, even if it asks. It is how you keep a device from reconnecting without forgetting it.
bluetooth-about-bonded = The keys agreed when you paired are still stored, so the two can trust each other without pairing again.
bluetooth-about-class = A number a classic device sends to say what it is — headphones, a phone, a computer. Low-energy devices send an appearance instead.
bluetooth-about-codec = How the sound is compressed on its way over. SBC works everywhere; the others need both sides to agree.
bluetooth-about-connected = Whether there is a link up right now. A paired device that is switched off is paired and not connected.
bluetooth-about-kind = What this device is, worked out from what it says about itself — its class, its appearance, and the services it offers.
bluetooth-about-legacy = It pairs the old way, without the numeric comparison. Older accessories do this, and it is less secure.
bluetooth-about-modalias = The maker and model numbers it reports. This is what keyd matches a keyboard on, and what a search engine finds.
bluetooth-about-name = The name the device calls itself. What you see everywhere else is the alias above.
bluetooth-about-paired = Whether this machine and the device have exchanged keys. Pairing is what makes a connection possible later.
bluetooth-about-reconnect = Which side is allowed to start the connection — this machine, the device, or either of them.
bluetooth-about-rssi = How loudly this machine hears it, in dBm. Closer to zero is stronger; below about -90 it is on the edge.
bluetooth-about-services = Whether the daemon has finished asking the device what it can do. Until it has, the list below may be short.
bluetooth-about-trusted = A trusted device may connect without being asked each time. It is the difference between a keyboard that just works and one that asks.
bluetooth-about-tx-power = How loudly the device says it is transmitting. With the signal strength above, it is a rough idea of distance.
bluetooth-about-volume = The volume the DEVICE reports over Bluetooth. What this machine plays at is in the other column.
bluetooth-about-wake = Whether pressing a key on it may bring this machine back from sleep.
bluetooth-pairing-answer = y accepts · n refuses

## file info editing — everything the info panel can change

# A phone's internet, over Bluetooth.
bluetooth-fact-internet = uses its internet
bluetooth-about-internet = The phone shares its connection over Bluetooth. The link is NetworkManager's, and the network view lists it too, under the same interface.
bluetooth-internet-through = yes, through { $interface }
echo-bluetooth-internet-on = going out through { $device } now
echo-bluetooth-internet-off = no longer going out through { $device }
echo-bluetooth-internet-failed = could not use the internet of { $device }: { $reason }
