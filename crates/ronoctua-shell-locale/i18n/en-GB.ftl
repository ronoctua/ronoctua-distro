# The texts `ronoctua-shell-locale` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

locale-heading-language = Interface
locale-speaking = in use
locale-from-file = You chose this language; it is kept in language.json.
locale-from-system = This is the machine's language, from the system configuration.
locale-from-default = Nobody has chosen; this is the first declared language.
locale-heading-time = Clock and date
locale-clock-24h = 24-hour clock
locale-clock-12h = 12-hour clock
locale-order-ymd = Year first
locale-order-dmy = Day first
locale-order-mdy = Month first
locale-heading-machine = Decided by the system
locale-zone-unknown = not known
locale-zone-is-declared = The zone is declared in the system configuration; changing it is a rebuild.
locale-decimal = Decimal separator
locale-decimal-is-the-languages = The separator belongs to the language, and follows it.
locale-measurement = Measurement
locale-metric = Metric
locale-not-written = The choice could not be written to disk.
locale-measurement-is-declared = Metric whatever the language, and that is a decision rather than a default.
cmd-locale-open = Language and region, in the footer
cmd-locale-window = Language and region, in a window
cmd-locale-up = Previous row
cmd-locale-down = Next row
cmd-locale-enter = Choose
group-locale = language/region
# ---------------------------------------------------------------------------
# LEADER s g — the regions whose streets this machine has.
#
# The names are the world's regions as Geofabrik publishes them, which is data
# and never a key: `Île-de-France` is not translated, and looking for
# `[geo-region-…]` would be looking for a translation of a name somebody wrote
# down. What IS a key is the sentence around it.
# ---------------------------------------------------------------------------
geo-title = Regions
geo-up = up
geo-down = down
geo-enter = install
geo-find = filter
geo-remove = remove
geo-installed = Installed
geo-available = Regions of the world
geo-none-installed = No region's streets are on this machine. An address stops at the nearest town.
geo-no-world = This build carries no world data, so there is no catalogue to choose from.
geo-filter = Filter
geo-filter-none = every region
geo-pack = { $size } · { $ways } ways · { $version }
geo-have = installed
geo-get = install
geo-in-parts = in parts
geo-only-in-parts = This region is published in parts. Its parts are in this list, and the filter finds them by its name.
geo-sure = Remove the streets of { $region }?
geo-no = No, keep them
geo-yes = Yes, remove them
geo-not-installed = That row is not an installed region.
geo-not-removed = The region could not be removed.
geo-not-in-catalogue = This region is not in this build's catalogue, so it cannot be fetched again.
ask-geo-find = A region's name, or part of it — empty shows them all
geo-queued = In the queue. Its size is what the server answers, and it can be given up there.
geo-no-home = There is no data directory to install into.
cmd-geo-open = Regions and their streets, in the footer
cmd-geo-window = Regions and their streets, in a window
cmd-geo-up = Previous row
cmd-geo-down = Next row
cmd-geo-enter = Install the region under the cursor, or install it again
cmd-geo-find = Narrow the list of regions
cmd-geo-remove = Remove the installed region under the cursor
group-geo = geography
# WHAT EACH SECTION IS FOR. Every one of them opens with a sentence saying
# what it changes and WHERE that shows, because a screen of settings that only
# names them leaves the person to guess which half of the machine they reach.
locale-interface-does = What this interface speaks. It changes at once, in the footer and in every window; the machine's own language is set in the system configuration.
locale-time-does = How this interface writes every date and every time it draws — the footer's clock, the file manager's dates, the glance. It does not move the machine's clock; that is the section below.
locale-numbers-does = How numbers are written and what units the temperature comes in. They reach every screen here, including the glance line.
locale-heading-machine-clock = This machine's clock
locale-machine-does = The computer's own clock, zone and date — what every program on this machine reads, not just this interface.
locale-automatic = Kept right automatically
locale-automatic-on = on
locale-automatic-off = off
locale-automatic-does = The system asks a time server over the internet and keeps the clock in step with it. This is how the distro runs; turning it off hands the clock to you.
locale-automatic-server = Asking { $server }.
locale-automatic-synchronised = The clock is in step.
locale-automatic-adrift = It has not been able to reach a time server yet.
locale-automatic-cannot = There is no time client installed on this machine.
locale-watched = Watched by the brain
locale-watched-does = The brain's clock watch looks a few times a day and turns the automatic keeping back on if it finds it off. It is the same switch as on that task's own page.
locale-watched-unknown = The brain is not running, so this cannot be read or changed from here.
locale-machine-kept = While the clock is kept automatically these cannot be set by hand: the next check would put them back.
locale-machine-unreachable = This machine's clock could not be asked about; nothing here can be changed.
# THE MACHINE'S TWO SAY WHAT PRESSING THEM DOES. `Date` and `Time` are the
# INTERFACE's rows one section above — they choose a SHAPE and show a live
# sample of it — and two rows of one list carrying one word are two facts a
# person cannot tell apart.
locale-machine-date = Set the date
locale-machine-time = Set the time
locale-machine-zone = Time zone
locale-shows-times-in = Show times in
# THE SHAPE IS SPELT OUT and is not the person's chosen order: this is a
# value being TYPED, and a prompt that followed the display preference would
# ask for `09/21/2026` on one machine and `21/09/2026` on the next.
ask-locale-system-date = The machine's date, as 2026-09-21
ask-locale-system-time = The machine's time, as 14:32 or 14:32:05
cmd-locale-system-date = Set this machine's date
cmd-locale-system-time = Set this machine's time
cmd-locale-system-zone = Set this machine's time zone
locale-not-a-date = That is not a date in that shape.
locale-not-a-time = That is not a time in that shape.
locale-clock-refused = The machine refused the change; see: journalctl -u systemd-timedated
locale-heading-glance = The glance line
locale-glance-does = The line { $keys } opens above the modeline. Each place below draws the time there, and the date, temperature and exchange rate you switch on for it.
locale-add-place-where = A zone, such as Europe/Lisbon. It starts out showing its clock and nothing else.
locale-no-places = No place is watched; { $keys } would show nothing.
locale-shows-clock = Time
locale-shows-date = Date
locale-shows-weather = Weather
locale-shows-rate = Exchange rate
locale-on = shown
locale-off = hidden
locale-heading-sources = Who is asked
locale-asked = asked
locale-not-asked = not asked
locale-sources-are-asked = Each request tells that host this machine's address and which places it watches. Nothing here needs an account or a key.
locale-source-weather = The weather
locale-source-rate = Exchange rates
# The hosts a source asks, in the order it asks them: the second is only asked
# when the first does not answer.
locale-source-hosts = Asks { $hosts }, in that order.
locale-source-answered = Last answer { $when }, from { $host }.
locale-source-never = Nothing has answered yet.
locale-source-trouble = The last attempt failed — { $said }
locale-title = Language and region
locale-up = up
locale-down = down
locale-enter = choose
locale-language = Language
locale-clock = Clock
locale-date = Date
locale-zone-is-yours = You chose this zone; the machine's own is set in the system configuration.
locale-zone-is-the-machines = The machine's own zone. Choosing one here changes what this interface shows, not what the system runs on.
locale-heading-numbers = Numbers and units
locale-decimal-language = { $mark } — the language's own
locale-decimal-point = { $mark } — a full stop, whatever the language
locale-decimal-comma = { $mark } — a comma, whatever the language
locale-units-metric = Metric — °C
locale-units-imperial = Imperial — °F
locale-drop-place = Remove this place
locale-add-place = Add a place
locale-add-place-how = a zone, such as Europe/Lisbon
ask-locale-where = Latitude and longitude, as two numbers
locale-no-such-zone = There is no zone by that name.
locale-not-a-coordinate = That is not a latitude and a longitude.
locale-no-place-here = Put the cursor on one of the glance's places first.
locale-place-gone = That place is no longer on the glance.
cmd-locale-less = Previous value
cmd-locale-more = Next value
cmd-locale-zone = Choose the zone this interface shows times in
cmd-locale-place = Add a place to the glance
cmd-locale-where = Say where a place is
locale-less = previous
locale-more = next
locale-where = where it is
# A MODULE THE SYSTEM SWITCHED OFF (`ronoctua.brain.modules.<id>.enable =
# false`): its row's reason, and what every ask about it is refused with — a
# row's reason sits in a column sixteen cells wide.
brain-why-system-off = Off by system
# And the date-and-time page's line under the clock watch's switch, when the
# system switched that watch off.
locale-watched-system-off = The system's configuration switched this watch off, so it cannot be switched on from here; the configuration switches it back on.

## file info editing — everything the info panel can change

# A place's currencies, chosen on screen.
locale-currency = Currency
locale-currency-pair = { $from } in { $into }
locale-currency-none = none chosen
locale-not-a-currency-pair = That is not two currencies the rates know, three letters each, like EUR BRL.
ask-locale-currency = Its currency and the one it is worth in, like EUR BRL
