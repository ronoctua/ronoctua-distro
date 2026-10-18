# The texts `ronoctua-shell-calc` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# THE CALCULATOR'S ENGINE (`ronoctua-calc`): why an expression has no value,
# the converter's categories, the shapes of a quick answer, and every unit's
# name — chosen by the number (`$count`, a number), as `assets/units.nix` declares.
calc-why-empty = There is nothing to calculate.
calc-why-unexpected = Something here does not belong where it is.
calc-why-unbalanced = A parenthesis is opened and never closed, or closed and never opened.
calc-why-unknown-word = A word that is not a unit, a currency, a function or a time zone.
calc-why-incompatible = These are different kinds of quantity and cannot be combined.
calc-why-divide-by-zero = A division by zero.
calc-why-too-large = The result is too large to hold.
calc-why-domain = The function has no value there.
calc-why-no-rates = Converting between currencies needs the exchange rates, and there are none yet.
calc-why-unknown-currency = The exchange rates do not quote this currency.
calc-why-unknown-zone = No time zone has that name.
calc-why-bad-date = That date does not exist.
calc-why-bad-roman = That is not a Roman numeral (1 to 3999, in the usual form).
calc-why-bad-digit = A digit that the number's base does not have.
calc-rates-why-wrong-pass = The permission given is for another site than the European Central Bank's.
calc-rates-why-failed = The European Central Bank answered with a failure.
calc-rates-why-garbled = What came back is not the European Central Bank's rates file.
calc-category-length = Length
calc-category-mass = Mass
calc-category-volume = Volume
calc-category-area = Area
calc-category-temperature = Temperature
calc-category-time = Time
calc-category-speed = Speed
calc-category-data = Data
calc-category-energy = Energy
calc-category-power = Power
calc-category-pressure = Pressure
calc-category-angle = Angle
calc-category-frequency = Frequency
calc-category-fuel-economy = Fuel economy
calc-category-cooking = Cooking
unit-metre = { $count ->
        [one] metre
       *[other] metres
    }
unit-kilometre = { $count ->
        [one] kilometre
       *[other] kilometres
    }
unit-centimetre = { $count ->
        [one] centimetre
       *[other] centimetres
    }
unit-millimetre = { $count ->
        [one] millimetre
       *[other] millimetres
    }
unit-micrometre = { $count ->
        [one] micrometre
       *[other] micrometres
    }
unit-nanometre = { $count ->
        [one] nanometre
       *[other] nanometres
    }
unit-mile = { $count ->
        [one] mile
       *[other] miles
    }
unit-yard = { $count ->
        [one] yard
       *[other] yards
    }
unit-foot = { $count ->
        [one] foot
       *[other] feet
    }
unit-inch = { $count ->
        [one] inch
       *[other] inches
    }
unit-nautical-mile = { $count ->
        [one] nautical mile
       *[other] nautical miles
    }
unit-astronomical-unit = { $count ->
        [one] astronomical unit
       *[other] astronomical units
    }
unit-light-year = { $count ->
        [one] light-year
       *[other] light-years
    }
unit-kilogram = { $count ->
        [one] kilogram
       *[other] kilograms
    }
unit-gram = { $count ->
        [one] gram
       *[other] grams
    }
unit-milligram = { $count ->
        [one] milligram
       *[other] milligrams
    }
unit-microgram = { $count ->
        [one] microgram
       *[other] micrograms
    }
unit-tonne = { $count ->
        [one] tonne
       *[other] tonnes
    }
unit-pound = { $count ->
        [one] pound
       *[other] pounds
    }
unit-ounce = { $count ->
        [one] ounce
       *[other] ounces
    }
unit-stone = { $count ->
        [one] stone
       *[other] stones
    }
unit-carat = { $count ->
        [one] carat
       *[other] carats
    }
unit-short-ton = { $count ->
        [one] short ton
       *[other] short tons
    }
unit-long-ton = { $count ->
        [one] long ton
       *[other] long tons
    }
unit-cubic-metre = { $count ->
        [one] cubic metre
       *[other] cubic metres
    }
unit-litre = { $count ->
        [one] litre
       *[other] litres
    }
unit-decilitre = { $count ->
        [one] decilitre
       *[other] decilitres
    }
unit-centilitre = { $count ->
        [one] centilitre
       *[other] centilitres
    }
unit-millilitre = { $count ->
        [one] millilitre
       *[other] millilitres
    }
unit-cubic-centimetre = { $count ->
        [one] cubic centimetre
       *[other] cubic centimetres
    }
unit-gallon = { $count ->
        [one] gallon
       *[other] gallons
    }
unit-imperial-gallon = { $count ->
        [one] imperial gallon
       *[other] imperial gallons
    }
unit-quart = { $count ->
        [one] quart
       *[other] quarts
    }
unit-pint = { $count ->
        [one] pint
       *[other] pints
    }
unit-fluid-ounce = { $count ->
        [one] fluid ounce
       *[other] fluid ounces
    }
unit-cubic-foot = { $count ->
        [one] cubic foot
       *[other] cubic feet
    }
unit-cubic-inch = { $count ->
        [one] cubic inch
       *[other] cubic inches
    }
unit-square-metre = { $count ->
        [one] square metre
       *[other] square metres
    }
unit-square-kilometre = { $count ->
        [one] square kilometre
       *[other] square kilometres
    }
unit-square-centimetre = { $count ->
        [one] square centimetre
       *[other] square centimetres
    }
unit-square-millimetre = { $count ->
        [one] square millimetre
       *[other] square millimetres
    }
unit-hectare = { $count ->
        [one] hectare
       *[other] hectares
    }
unit-acre = { $count ->
        [one] acre
       *[other] acres
    }
unit-square-foot = { $count ->
        [one] square foot
       *[other] square feet
    }
unit-square-inch = { $count ->
        [one] square inch
       *[other] square inches
    }
unit-square-yard = { $count ->
        [one] square yard
       *[other] square yards
    }
unit-square-mile = { $count ->
        [one] square mile
       *[other] square miles
    }
unit-kelvin = { $count ->
        [one] kelvin
       *[other] kelvins
    }
unit-celsius = { $count ->
        [one] degree Celsius
       *[other] degrees Celsius
    }
unit-fahrenheit = { $count ->
        [one] degree Fahrenheit
       *[other] degrees Fahrenheit
    }
unit-rankine = { $count ->
        [one] degree Rankine
       *[other] degrees Rankine
    }
unit-second = { $count ->
        [one] second
       *[other] seconds
    }
unit-millisecond = { $count ->
        [one] millisecond
       *[other] milliseconds
    }
unit-microsecond = { $count ->
        [one] microsecond
       *[other] microseconds
    }
unit-nanosecond = { $count ->
        [one] nanosecond
       *[other] nanoseconds
    }
unit-minute = { $count ->
        [one] minute
       *[other] minutes
    }
unit-hour = { $count ->
        [one] hour
       *[other] hours
    }
unit-day = { $count ->
        [one] day
       *[other] days
    }
unit-week = { $count ->
        [one] week
       *[other] weeks
    }
unit-month = { $count ->
        [one] month
       *[other] months
    }
unit-year = { $count ->
        [one] year
       *[other] years
    }
unit-metre-per-second = { $count ->
        [one] metre per second
       *[other] metres per second
    }
unit-kilometre-per-hour = { $count ->
        [one] kilometre per hour
       *[other] kilometres per hour
    }
unit-mile-per-hour = { $count ->
        [one] mile per hour
       *[other] miles per hour
    }
unit-knot = { $count ->
        [one] knot
       *[other] knots
    }
unit-foot-per-second = { $count ->
        [one] foot per second
       *[other] feet per second
    }
unit-bit = { $count ->
        [one] bit
       *[other] bits
    }
unit-byte = { $count ->
        [one] byte
       *[other] bytes
    }
unit-kilobit = { $count ->
        [one] kilobit
       *[other] kilobits
    }
unit-megabit = { $count ->
        [one] megabit
       *[other] megabits
    }
unit-gigabit = { $count ->
        [one] gigabit
       *[other] gigabits
    }
unit-kibibit = { $count ->
        [one] kibibit
       *[other] kibibits
    }
unit-mebibit = { $count ->
        [one] mebibit
       *[other] mebibits
    }
unit-kilobyte = { $count ->
        [one] kilobyte
       *[other] kilobytes
    }
unit-megabyte = { $count ->
        [one] megabyte
       *[other] megabytes
    }
unit-gigabyte = { $count ->
        [one] gigabyte
       *[other] gigabytes
    }
unit-terabyte = { $count ->
        [one] terabyte
       *[other] terabytes
    }
unit-petabyte = { $count ->
        [one] petabyte
       *[other] petabytes
    }
unit-kibibyte = { $count ->
        [one] kibibyte
       *[other] kibibytes
    }
unit-mebibyte = { $count ->
        [one] mebibyte
       *[other] mebibytes
    }
unit-gibibyte = { $count ->
        [one] gibibyte
       *[other] gibibytes
    }
unit-tebibyte = { $count ->
        [one] tebibyte
       *[other] tebibytes
    }
unit-pebibyte = { $count ->
        [one] pebibyte
       *[other] pebibytes
    }
unit-joule = { $count ->
        [one] joule
       *[other] joules
    }
unit-kilojoule = { $count ->
        [one] kilojoule
       *[other] kilojoules
    }
unit-megajoule = { $count ->
        [one] megajoule
       *[other] megajoules
    }
unit-calorie = { $count ->
        [one] calorie
       *[other] calories
    }
unit-kilocalorie = { $count ->
        [one] kilocalorie
       *[other] kilocalories
    }
unit-watt-hour = { $count ->
        [one] watt-hour
       *[other] watt-hours
    }
unit-kilowatt-hour = { $count ->
        [one] kilowatt-hour
       *[other] kilowatt-hours
    }
unit-electronvolt = { $count ->
        [one] electronvolt
       *[other] electronvolts
    }
unit-british-thermal-unit = { $count ->
        [one] British thermal unit
       *[other] British thermal units
    }
unit-watt = { $count ->
        [one] watt
       *[other] watts
    }
unit-kilowatt = { $count ->
        [one] kilowatt
       *[other] kilowatts
    }
unit-megawatt = { $count ->
        [one] megawatt
       *[other] megawatts
    }
unit-horsepower = horsepower
unit-metric-horsepower = metric horsepower
unit-pascal = { $count ->
        [one] pascal
       *[other] pascals
    }
unit-hectopascal = { $count ->
        [one] hectopascal
       *[other] hectopascals
    }
unit-kilopascal = { $count ->
        [one] kilopascal
       *[other] kilopascals
    }
unit-megapascal = { $count ->
        [one] megapascal
       *[other] megapascals
    }
unit-bar = { $count ->
        [one] bar
       *[other] bars
    }
unit-millibar = { $count ->
        [one] millibar
       *[other] millibars
    }
unit-atmosphere = { $count ->
        [one] atmosphere
       *[other] atmospheres
    }
unit-pound-per-square-inch = { $count ->
        [one] pound per square inch
       *[other] pounds per square inch
    }
unit-millimetre-of-mercury = { $count ->
        [one] millimetre of mercury
       *[other] millimetres of mercury
    }
unit-torr = { $count ->
        [one] torr
       *[other] torrs
    }
unit-radian = { $count ->
        [one] radian
       *[other] radians
    }
unit-degree = { $count ->
        [one] degree
       *[other] degrees
    }
unit-gradian = { $count ->
        [one] gradian
       *[other] gradians
    }
unit-arcminute = { $count ->
        [one] arcminute
       *[other] arcminutes
    }
unit-arcsecond = { $count ->
        [one] arcsecond
       *[other] arcseconds
    }
unit-turn = { $count ->
        [one] turn
       *[other] turns
    }
unit-hertz = hertz
unit-kilohertz = kilohertz
unit-megahertz = megahertz
unit-gigahertz = gigahertz
unit-revolution-per-minute = { $count ->
        [one] revolution per minute
       *[other] revolutions per minute
    }
unit-litre-per-100-km = { $count ->
        [one] litre per 100 kilometres
       *[other] litres per 100 kilometres
    }
unit-kilometre-per-litre = { $count ->
        [one] kilometre per litre
       *[other] kilometres per litre
    }
unit-mile-per-gallon = { $count ->
        [one] mile per gallon
       *[other] miles per gallon
    }
unit-mile-per-imperial-gallon = { $count ->
        [one] mile per imperial gallon
       *[other] miles per imperial gallon
    }
unit-cup = { $count ->
        [one] cup
       *[other] cups
    }
unit-customary-cup = { $count ->
        [one] customary cup
       *[other] customary cups
    }
unit-metric-cup = { $count ->
        [one] metric cup
       *[other] metric cups
    }
unit-tablespoon = { $count ->
        [one] tablespoon
       *[other] tablespoons
    }
unit-customary-tablespoon = { $count ->
        [one] customary tablespoon
       *[other] customary tablespoons
    }
unit-teaspoon = { $count ->
        [one] teaspoon
       *[other] teaspoons
    }
unit-customary-teaspoon = { $count ->
        [one] customary teaspoon
       *[other] customary teaspoons
    }

## file info editing — everything the info panel can change

# THE CALCULATOR (`LEADER =`, `crate::calc`): the leader's group and its two
# doors, the window's name and the modeline's name for it.
group-calc = calculator
cmd-calc-open = The calculator, in the footer
cmd-calc-window = The calculator, in a window of its own
calc-title = Calculator
calc-mode-basic = Basic
calc-mode-scientific = Scientific
calc-mode-convert = Convert
calc-sum = Sum
calc-amount = Amount
calc-empty = type a sum, or press the keys
calc-equals-exact = = { $value }
calc-equals-approx = ≈ { $value }
calc-history-title = History
calc-history-empty = No answer is kept yet: { $keys } keeps one.
calc-from = From
calc-to = To
calc-find = Find a unit — { $side }:
calc-find-none = No unit of this category has that name.
calc-category-currency = Currency
calc-rates-asking = Asking { $host } for today's exchange rates…
calc-rates-of = Exchange rates of { $date }, from the European Central Bank.
calc-unkept-unparsed = { $file } will not parse, so this answer was not kept; the file was left as it is.
calc-unkept = This answer could not be kept in { $file }: { $why }
calc-float-unreached = The compositor could not be asked to move this window.
# The header's words, and what each one does under the pointer.
calc-equals = keep
about-calc-equals = Keeps the answer in the history and goes on from it; on an answer picked in the history, puts its sum back.
calc-mode = mode
about-calc-mode = Goes to the next mode, round the three: basic, scientific, convert.
calc-clear = clear
about-calc-clear = Empties the sum.
calc-copy = copy
about-calc-copy = Copies the answer — the one picked in the history, or the one on the line.
calc-history = history
about-calc-history = Picks an answer of the history, one older each time.
calc-take = take
about-calc-take = Takes the unit under the cursor for the side being chosen.
calc-ring-next = next category
about-calc-ring-next = Goes to the next category of units.
calc-ring-previous = previous category
about-calc-ring-previous = Goes to the previous category of units.
about-calc-from = Chooses the unit the amount is in: what is typed next finds it.
about-calc-to = Chooses the unit the amount is converted into: what is typed next finds it.
calc-swap = swap
about-calc-swap = Trades the two units: what was converted from is converted into.
calc-float = compact or normal
about-calc-float = Floats the window, compact, over the others — or puts it back in the layout among them.
calc-keys-narrow = The functions' keys need more room than this. Type them by name instead: sin, cos, √, ln…
# The keypad's legends: what is not a digit or the decimal mark.
calc-key-add = +
calc-key-subtract = −
calc-key-multiply = ×
calc-key-divide = ÷
calc-key-open = (
calc-key-close = )
calc-key-percent = %
calc-key-power = xʸ
calc-key-square = x²
calc-key-reciprocal = x⁻¹
calc-key-factorial = n!
calc-key-degree = °
calc-key-sin = sin
calc-key-cos = cos
calc-key-tan = tan
calc-key-asin = asin
calc-key-acos = acos
calc-key-atan = atan
calc-key-ln = ln
calc-key-log = log
calc-key-sqrt = √
calc-key-cbrt = ∛
calc-key-exp = exp
calc-key-pi = π
calc-key-e = e
calc-key-clear = C
calc-key-equals = =
