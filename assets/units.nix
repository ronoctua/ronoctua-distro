# THE UNITS THE CALCULATOR CONVERTS — the one place they are declared.
#
# `nix/unitsgen.nix` (with `nix/scripts/gen-units.py`) turns this into the Rust
# table `ronoctua-calc` includes (`RONOCTUA_UNITS_RS`), and REFUSES the build
# when a unit has no words in some language of `assets/i18n/languages.nix`, when
# two units answer to the same word or symbol, or when a factor is not exact.
#
# # What a unit is
#
# A dimension (powers of length, mass, time, temperature, data and angle) and
# an EXACT factor to that dimension's base unit — metre, kilogram, second,
# kelvin, bit, radian — written as a decimal or a fraction of decimals, never as
# a float: `mi` is `1609.344`, and `psi` is a pound-force over a square inch,
# written as that quotient. Three optional fields:
#
#   offset      what is added after the factor, for an AFFINE unit: °C is
#               K − 273.15, so its offset is `273.15`
#   pi          the power of π in the factor. An angle is the one dimension
#               whose units cannot all be rational against each other (a degree
#               is π/180 radian), so the factor carries π apart and a
#               conversion that does not cancel it is approximate
#   reciprocal  the unit counts the INVERSE of its dimension: mpg and km/L are
#               distance per volume, and convert against L/100 km by division
#
# # Words
#
# `symbols` are written as a formula writes them and matched exactly, case
# included (`mB` is not `MB`), in every language. `words` are per language
# SUBTAG and matched without case or accents; the FIRST two of each language are
# the singular and the plural a person reads, which is what the `unit-<id>` key
# of that language's `.ftl` says. A phrase is one word (`fluid ounce`, `colher
# de sopa`): the longest phrase wins, so `colher de sopa` is never `colher` and
# then `de`.
#
# # And the words that are not units
#
# `currencies`, `connectives` (`in`, `of`), `radices` (`hex`), `instants`
# (`now`) and `functions` share the same rule: every language has words for each
# (`*` is every language at once, for notation like `sqrt`), and no word means
# two things — in any two languages either, because the calculator understands
# every language's words at once.
{
  units = [
    # ---- length --------------------------------------------------------
    {
      id = "metre";
      category = "length";
      dimension.length = 1;
      factor = "1";
      symbols = [ "m" ];
      words.en = [
        "metre"
        "metres"
        "meter"
        "meters"
      ];
      words.pt = [
        "metro"
        "metros"
      ];
    }
    {
      id = "kilometre";
      category = "length";
      dimension.length = 1;
      factor = "1000";
      symbols = [ "km" ];
      words.en = [
        "kilometre"
        "kilometres"
        "kilometer"
        "kilometers"
      ];
      words.pt = [
        "quilômetro"
        "quilômetros"
        "kilometro"
        "kilometros"
      ];
    }
    {
      id = "centimetre";
      category = "length";
      dimension.length = 1;
      factor = "0.01";
      symbols = [ "cm" ];
      words.en = [
        "centimetre"
        "centimetres"
        "centimeter"
        "centimeters"
      ];
      words.pt = [
        "centímetro"
        "centímetros"
      ];
    }
    {
      id = "millimetre";
      category = "length";
      dimension.length = 1;
      factor = "0.001";
      symbols = [ "mm" ];
      words.en = [
        "millimetre"
        "millimetres"
        "millimeter"
        "millimeters"
      ];
      words.pt = [
        "milímetro"
        "milímetros"
      ];
    }
    {
      id = "micrometre";
      category = "length";
      dimension.length = 1;
      factor = "0.000001";
      symbols = [
        "µm"
        "μm"
        "um"
      ];
      words.en = [
        "micrometre"
        "micrometres"
        "micrometer"
        "micrometers"
        "micron"
        "microns"
      ];
      words.pt = [
        "micrômetro"
        "micrômetros"
        "mícron"
        "mícrons"
      ];
    }
    {
      id = "nanometre";
      category = "length";
      dimension.length = 1;
      factor = "0.000000001";
      symbols = [ "nm" ];
      words.en = [
        "nanometre"
        "nanometres"
        "nanometer"
        "nanometers"
      ];
      words.pt = [
        "nanômetro"
        "nanômetros"
      ];
    }
    {
      id = "mile";
      category = "length";
      dimension.length = 1;
      factor = "1609.344";
      symbols = [ "mi" ];
      words.en = [
        "mile"
        "miles"
      ];
      words.pt = [
        "milha"
        "milhas"
      ];
    }
    {
      id = "yard";
      category = "length";
      dimension.length = 1;
      factor = "0.9144";
      symbols = [ "yd" ];
      words.en = [
        "yard"
        "yards"
      ];
      words.pt = [
        "jarda"
        "jardas"
      ];
    }
    {
      id = "foot";
      category = "length";
      dimension.length = 1;
      factor = "0.3048";
      symbols = [ "ft" ];
      words.en = [
        "foot"
        "feet"
      ];
      words.pt = [
        "pé"
        "pés"
      ];
    }
    {
      id = "inch";
      category = "length";
      dimension.length = 1;
      factor = "0.0254";
      # NOT `in`: that is the word a conversion is asked with.
      symbols = [ "″" ];
      words.en = [
        "inch"
        "inches"
      ];
      words.pt = [
        "polegada"
        "polegadas"
      ];
    }
    {
      id = "nautical-mile";
      category = "length";
      dimension.length = 1;
      factor = "1852";
      symbols = [ "nmi" ];
      words.en = [
        "nautical mile"
        "nautical miles"
      ];
      words.pt = [
        "milha náutica"
        "milhas náuticas"
      ];
    }
    {
      id = "astronomical-unit";
      category = "length";
      dimension.length = 1;
      factor = "149597870700";
      symbols = [ "au" ];
      words.en = [
        "astronomical unit"
        "astronomical units"
      ];
      words.pt = [
        "unidade astronômica"
        "unidades astronômicas"
      ];
    }
    {
      id = "light-year";
      category = "length";
      dimension.length = 1;
      factor = "9460730472580800";
      symbols = [ "ly" ];
      words.en = [
        "light-year"
        "light-years"
        "light year"
        "light years"
      ];
      words.pt = [
        "ano-luz"
        "anos-luz"
        "ano luz"
        "anos luz"
      ];
    }

    # ---- mass ----------------------------------------------------------
    {
      id = "kilogram";
      category = "mass";
      dimension.mass = 1;
      factor = "1";
      symbols = [ "kg" ];
      words.en = [
        "kilogram"
        "kilograms"
        "kilogramme"
        "kilogrammes"
        "kilo"
        "kilos"
      ];
      words.pt = [
        "quilograma"
        "quilogramas"
        "quilo"
        "quilos"
      ];
    }
    {
      id = "gram";
      category = "mass";
      dimension.mass = 1;
      factor = "0.001";
      symbols = [ "g" ];
      words.en = [
        "gram"
        "grams"
        "gramme"
        "grammes"
      ];
      words.pt = [
        "grama"
        "gramas"
      ];
    }
    {
      id = "milligram";
      category = "mass";
      dimension.mass = 1;
      factor = "0.000001";
      symbols = [ "mg" ];
      words.en = [
        "milligram"
        "milligrams"
      ];
      words.pt = [
        "miligrama"
        "miligramas"
      ];
    }
    {
      id = "microgram";
      category = "mass";
      dimension.mass = 1;
      factor = "0.000000001";
      symbols = [
        "µg"
        "μg"
        "ug"
        "mcg"
      ];
      words.en = [
        "microgram"
        "micrograms"
      ];
      words.pt = [
        "micrograma"
        "microgramas"
      ];
    }
    {
      id = "tonne";
      category = "mass";
      dimension.mass = 1;
      factor = "1000";
      symbols = [ "t" ];
      words.en = [
        "tonne"
        "tonnes"
        "metric ton"
        "metric tons"
      ];
      words.pt = [
        "tonelada"
        "toneladas"
      ];
    }
    {
      id = "pound";
      category = "mass";
      dimension.mass = 1;
      factor = "0.45359237";
      symbols = [
        "lb"
        "lbs"
      ];
      words.en = [
        "pound"
        "pounds"
      ];
      words.pt = [
        "libra"
        "libras"
      ];
    }
    {
      id = "ounce";
      category = "mass";
      dimension.mass = 1;
      factor = "0.028349523125";
      symbols = [ "oz" ];
      words.en = [
        "ounce"
        "ounces"
      ];
      words.pt = [
        "onça"
        "onças"
      ];
    }
    {
      id = "stone";
      category = "mass";
      dimension.mass = 1;
      factor = "6.35029318";
      symbols = [ "st" ];
      words.en = [
        "stone"
        "stones"
      ];
      words.pt = [
        "stone"
        "stones"
      ];
    }
    {
      id = "carat";
      category = "mass";
      dimension.mass = 1;
      factor = "0.0002";
      symbols = [ "ct" ];
      words.en = [
        "carat"
        "carats"
      ];
      words.pt = [
        "quilate"
        "quilates"
      ];
    }
    {
      id = "short-ton";
      category = "mass";
      dimension.mass = 1;
      factor = "907.18474";
      symbols = [ ];
      words.en = [
        "short ton"
        "short tons"
        "us ton"
        "us tons"
      ];
      words.pt = [
        "tonelada curta"
        "toneladas curtas"
      ];
    }
    {
      id = "long-ton";
      category = "mass";
      dimension.mass = 1;
      factor = "1016.0469088";
      symbols = [ ];
      words.en = [
        "long ton"
        "long tons"
        "imperial ton"
        "imperial tons"
      ];
      words.pt = [
        "tonelada longa"
        "toneladas longas"
      ];
    }

    # ---- volume --------------------------------------------------------
    {
      id = "cubic-metre";
      category = "volume";
      dimension.length = 3;
      factor = "1";
      symbols = [
        "m³"
        "m3"
      ];
      words.en = [
        "cubic metre"
        "cubic metres"
        "cubic meter"
        "cubic meters"
      ];
      words.pt = [
        "metro cúbico"
        "metros cúbicos"
      ];
    }
    {
      id = "litre";
      category = "volume";
      dimension.length = 3;
      factor = "0.001";
      symbols = [
        "L"
        "l"
        "ℓ"
      ];
      words.en = [
        "litre"
        "litres"
        "liter"
        "liters"
      ];
      words.pt = [
        "litro"
        "litros"
      ];
    }
    {
      id = "decilitre";
      category = "volume";
      dimension.length = 3;
      factor = "0.0001";
      symbols = [
        "dL"
        "dl"
      ];
      words.en = [
        "decilitre"
        "decilitres"
        "deciliter"
        "deciliters"
      ];
      words.pt = [
        "decilitro"
        "decilitros"
      ];
    }
    {
      id = "centilitre";
      category = "volume";
      dimension.length = 3;
      factor = "0.00001";
      symbols = [
        "cL"
        "cl"
      ];
      words.en = [
        "centilitre"
        "centilitres"
        "centiliter"
        "centiliters"
      ];
      words.pt = [
        "centilitro"
        "centilitros"
      ];
    }
    {
      id = "millilitre";
      category = "volume";
      dimension.length = 3;
      factor = "0.000001";
      symbols = [
        "mL"
        "ml"
      ];
      words.en = [
        "millilitre"
        "millilitres"
        "milliliter"
        "milliliters"
      ];
      words.pt = [
        "mililitro"
        "mililitros"
      ];
    }
    {
      id = "cubic-centimetre";
      category = "volume";
      dimension.length = 3;
      factor = "0.000001";
      symbols = [
        "cm³"
        "cm3"
        "cc"
      ];
      words.en = [
        "cubic centimetre"
        "cubic centimetres"
        "cubic centimeter"
        "cubic centimeters"
      ];
      words.pt = [
        "centímetro cúbico"
        "centímetros cúbicos"
      ];
    }
    {
      id = "gallon";
      category = "volume";
      dimension.length = 3;
      factor = "0.003785411784";
      symbols = [ "gal" ];
      words.en = [
        "gallon"
        "gallons"
        "us gallon"
        "us gallons"
      ];
      words.pt = [
        "galão"
        "galões"
        "galão americano"
        "galões americanos"
      ];
    }
    {
      id = "imperial-gallon";
      category = "volume";
      dimension.length = 3;
      factor = "0.00454609";
      symbols = [ ];
      words.en = [
        "imperial gallon"
        "imperial gallons"
        "uk gallon"
        "uk gallons"
      ];
      words.pt = [
        "galão imperial"
        "galões imperiais"
      ];
    }
    {
      id = "quart";
      category = "volume";
      dimension.length = 3;
      factor = "0.000946352946";
      symbols = [ "qt" ];
      words.en = [
        "quart"
        "quarts"
      ];
      words.pt = [
        "quarto de galão"
        "quartos de galão"
      ];
    }
    {
      id = "pint";
      category = "volume";
      dimension.length = 3;
      factor = "0.000473176473";
      symbols = [ "pt" ];
      words.en = [
        "pint"
        "pints"
      ];
      words.pt = [
        "pinta"
        "pintas"
      ];
    }
    {
      id = "fluid-ounce";
      category = "volume";
      dimension.length = 3;
      factor = "0.0000295735295625";
      symbols = [
        "fl oz"
        "floz"
      ];
      words.en = [
        "fluid ounce"
        "fluid ounces"
      ];
      words.pt = [
        "onça líquida"
        "onças líquidas"
      ];
    }
    {
      id = "cubic-foot";
      category = "volume";
      dimension.length = 3;
      factor = "0.028316846592";
      symbols = [
        "ft³"
        "ft3"
      ];
      words.en = [
        "cubic foot"
        "cubic feet"
      ];
      words.pt = [
        "pé cúbico"
        "pés cúbicos"
      ];
    }
    {
      id = "cubic-inch";
      category = "volume";
      dimension.length = 3;
      factor = "0.000016387064";
      symbols = [
        "in³"
        "in3"
      ];
      words.en = [
        "cubic inch"
        "cubic inches"
      ];
      words.pt = [
        "polegada cúbica"
        "polegadas cúbicas"
      ];
    }

    # ---- area ----------------------------------------------------------
    {
      id = "square-metre";
      category = "area";
      dimension.length = 2;
      factor = "1";
      symbols = [
        "m²"
        "m2"
      ];
      words.en = [
        "square metre"
        "square metres"
        "square meter"
        "square meters"
      ];
      words.pt = [
        "metro quadrado"
        "metros quadrados"
      ];
    }
    {
      id = "square-kilometre";
      category = "area";
      dimension.length = 2;
      factor = "1000000";
      symbols = [
        "km²"
        "km2"
      ];
      words.en = [
        "square kilometre"
        "square kilometres"
        "square kilometer"
        "square kilometers"
      ];
      words.pt = [
        "quilômetro quadrado"
        "quilômetros quadrados"
      ];
    }
    {
      id = "square-centimetre";
      category = "area";
      dimension.length = 2;
      factor = "0.0001";
      symbols = [
        "cm²"
        "cm2"
      ];
      words.en = [
        "square centimetre"
        "square centimetres"
        "square centimeter"
        "square centimeters"
      ];
      words.pt = [
        "centímetro quadrado"
        "centímetros quadrados"
      ];
    }
    {
      id = "square-millimetre";
      category = "area";
      dimension.length = 2;
      factor = "0.000001";
      symbols = [
        "mm²"
        "mm2"
      ];
      words.en = [
        "square millimetre"
        "square millimetres"
        "square millimeter"
        "square millimeters"
      ];
      words.pt = [
        "milímetro quadrado"
        "milímetros quadrados"
      ];
    }
    {
      id = "hectare";
      category = "area";
      dimension.length = 2;
      factor = "10000";
      symbols = [ "ha" ];
      words.en = [
        "hectare"
        "hectares"
      ];
      words.pt = [
        "hectare"
        "hectares"
      ];
    }
    {
      id = "acre";
      category = "area";
      dimension.length = 2;
      factor = "4046.8564224";
      symbols = [ "ac" ];
      words.en = [
        "acre"
        "acres"
      ];
      words.pt = [
        "acre"
        "acres"
      ];
    }
    {
      id = "square-foot";
      category = "area";
      dimension.length = 2;
      factor = "0.09290304";
      symbols = [
        "ft²"
        "ft2"
      ];
      words.en = [
        "square foot"
        "square feet"
      ];
      words.pt = [
        "pé quadrado"
        "pés quadrados"
      ];
    }
    {
      id = "square-inch";
      category = "area";
      dimension.length = 2;
      factor = "0.00064516";
      symbols = [
        "in²"
        "in2"
      ];
      words.en = [
        "square inch"
        "square inches"
      ];
      words.pt = [
        "polegada quadrada"
        "polegadas quadradas"
      ];
    }
    {
      id = "square-yard";
      category = "area";
      dimension.length = 2;
      factor = "0.83612736";
      symbols = [
        "yd²"
        "yd2"
      ];
      words.en = [
        "square yard"
        "square yards"
      ];
      words.pt = [
        "jarda quadrada"
        "jardas quadradas"
      ];
    }
    {
      id = "square-mile";
      category = "area";
      dimension.length = 2;
      factor = "2589988.110336";
      symbols = [
        "mi²"
        "mi2"
      ];
      words.en = [
        "square mile"
        "square miles"
      ];
      words.pt = [
        "milha quadrada"
        "milhas quadradas"
      ];
    }

    # ---- temperature (affine) ------------------------------------------
    {
      id = "kelvin";
      category = "temperature";
      dimension.temperature = 1;
      factor = "1";
      symbols = [ "K" ];
      words.en = [
        "kelvin"
        "kelvins"
      ];
      words.pt = [
        "kelvin"
        "kelvins"
      ];
    }
    {
      id = "celsius";
      category = "temperature";
      dimension.temperature = 1;
      factor = "1";
      offset = "273.15";
      symbols = [
        "°C"
        "℃"
        "ºC"
      ];
      words.en = [
        "degree Celsius"
        "degrees Celsius"
        "celsius"
        "centigrade"
      ];
      words.pt = [
        "grau Celsius"
        "graus Celsius"
        "celsius"
        "centígrado"
        "centígrados"
      ];
    }
    {
      id = "fahrenheit";
      category = "temperature";
      dimension.temperature = 1;
      factor = "5/9";
      # K = (°F + 459.67) × 5/9, so the offset is 459.67 × 5/9.
      offset = "459.67*5/9";
      symbols = [
        "°F"
        "℉"
        "ºF"
      ];
      words.en = [
        "degree Fahrenheit"
        "degrees Fahrenheit"
        "fahrenheit"
      ];
      words.pt = [
        "grau Fahrenheit"
        "graus Fahrenheit"
        "fahrenheit"
      ];
    }
    {
      id = "rankine";
      category = "temperature";
      dimension.temperature = 1;
      factor = "5/9";
      symbols = [
        "°R"
        "°Ra"
      ];
      words.en = [
        "degree Rankine"
        "degrees Rankine"
        "rankine"
      ];
      words.pt = [
        "grau Rankine"
        "graus Rankine"
        "rankine"
      ];
    }

    # ---- time ----------------------------------------------------------
    {
      id = "second";
      category = "time";
      dimension.time = 1;
      factor = "1";
      symbols = [
        "s"
        "sec"
      ];
      words.en = [
        "second"
        "seconds"
        "secs"
      ];
      words.pt = [
        "segundo"
        "segundos"
        "seg"
      ];
    }
    {
      id = "millisecond";
      category = "time";
      dimension.time = 1;
      factor = "0.001";
      symbols = [ "ms" ];
      words.en = [
        "millisecond"
        "milliseconds"
      ];
      words.pt = [
        "milissegundo"
        "milissegundos"
      ];
    }
    {
      id = "microsecond";
      category = "time";
      dimension.time = 1;
      factor = "0.000001";
      symbols = [
        "µs"
        "μs"
      ];
      words.en = [
        "microsecond"
        "microseconds"
      ];
      words.pt = [
        "microssegundo"
        "microssegundos"
      ];
    }
    {
      id = "nanosecond";
      category = "time";
      dimension.time = 1;
      factor = "0.000000001";
      symbols = [ "ns" ];
      words.en = [
        "nanosecond"
        "nanoseconds"
      ];
      words.pt = [
        "nanossegundo"
        "nanossegundos"
      ];
    }
    {
      id = "minute";
      category = "time";
      dimension.time = 1;
      factor = "60";
      symbols = [ "min" ];
      words.en = [
        "minute"
        "minutes"
        "mins"
      ];
      words.pt = [
        "minuto"
        "minutos"
      ];
    }
    {
      id = "hour";
      category = "time";
      dimension.time = 1;
      factor = "3600";
      symbols = [
        "h"
        "hr"
      ];
      words.en = [
        "hour"
        "hours"
        "hrs"
      ];
      words.pt = [
        "hora"
        "horas"
      ];
    }
    {
      id = "day";
      category = "time";
      dimension.time = 1;
      factor = "86400";
      symbols = [ "d" ];
      words.en = [
        "day"
        "days"
      ];
      words.pt = [
        "dia"
        "dias"
      ];
    }
    {
      id = "week";
      category = "time";
      dimension.time = 1;
      factor = "604800";
      symbols = [ "wk" ];
      words.en = [
        "week"
        "weeks"
      ];
      words.pt = [
        "semana"
        "semanas"
      ];
    }
    {
      # THE MEAN GREGORIAN MONTH, a twelfth of the year below. Added to a DATE
      # it is a calendar month instead (`2026-01-31 + 1 month`), which is what
      # a person means there.
      id = "month";
      category = "time";
      dimension.time = 1;
      factor = "2629746";
      symbols = [ "mo" ];
      words.en = [
        "month"
        "months"
      ];
      words.pt = [
        "mês"
        "meses"
      ];
    }
    {
      # THE MEAN GREGORIAN YEAR, 365.2425 days.
      id = "year";
      category = "time";
      dimension.time = 1;
      factor = "31556952";
      symbols = [ "yr" ];
      words.en = [
        "year"
        "years"
      ];
      words.pt = [
        "ano"
        "anos"
      ];
    }

    # ---- speed ---------------------------------------------------------
    {
      id = "metre-per-second";
      category = "speed";
      dimension = {
        length = 1;
        time = -1;
      };
      factor = "1";
      symbols = [ "m/s" ];
      words.en = [
        "metre per second"
        "metres per second"
        "meter per second"
        "meters per second"
      ];
      words.pt = [
        "metro por segundo"
        "metros por segundo"
      ];
    }
    {
      id = "kilometre-per-hour";
      category = "speed";
      dimension = {
        length = 1;
        time = -1;
      };
      factor = "1000/3600";
      symbols = [
        "km/h"
        "kph"
        "kmh"
      ];
      words.en = [
        "kilometre per hour"
        "kilometres per hour"
        "kilometer per hour"
        "kilometers per hour"
      ];
      words.pt = [
        "quilômetro por hora"
        "quilômetros por hora"
      ];
    }
    {
      id = "mile-per-hour";
      category = "speed";
      dimension = {
        length = 1;
        time = -1;
      };
      factor = "1609.344/3600";
      symbols = [
        "mph"
        "mi/h"
      ];
      words.en = [
        "mile per hour"
        "miles per hour"
      ];
      words.pt = [
        "milha por hora"
        "milhas por hora"
      ];
    }
    {
      id = "knot";
      category = "speed";
      dimension = {
        length = 1;
        time = -1;
      };
      factor = "1852/3600";
      symbols = [
        "kn"
        "kt"
      ];
      words.en = [
        "knot"
        "knots"
      ];
      words.pt = [
        "nó"
        "nós"
      ];
    }
    {
      id = "foot-per-second";
      category = "speed";
      dimension = {
        length = 1;
        time = -1;
      };
      factor = "0.3048";
      symbols = [
        "ft/s"
        "fps"
      ];
      words.en = [
        "foot per second"
        "feet per second"
      ];
      words.pt = [
        "pé por segundo"
        "pés por segundo"
      ];
    }

    # ---- data ----------------------------------------------------------
    {
      id = "bit";
      category = "data";
      dimension.data = 1;
      factor = "1";
      symbols = [ "bit" ];
      words.en = [
        "bit"
        "bits"
      ];
      words.pt = [
        "bit"
        "bits"
      ];
    }
    {
      id = "byte";
      category = "data";
      dimension.data = 1;
      factor = "8";
      symbols = [ "B" ];
      words.en = [
        "byte"
        "bytes"
      ];
      words.pt = [
        "byte"
        "bytes"
      ];
    }
    {
      id = "kilobit";
      category = "data";
      dimension.data = 1;
      factor = "1000";
      symbols = [
        "kbit"
        "kb"
      ];
      words.en = [
        "kilobit"
        "kilobits"
      ];
      words.pt = [
        "quilobit"
        "quilobits"
      ];
    }
    {
      id = "megabit";
      category = "data";
      dimension.data = 1;
      factor = "1000000";
      symbols = [
        "Mbit"
        "Mb"
      ];
      words.en = [
        "megabit"
        "megabits"
      ];
      words.pt = [
        "megabit"
        "megabits"
      ];
    }
    {
      id = "gigabit";
      category = "data";
      dimension.data = 1;
      factor = "1000000000";
      symbols = [
        "Gbit"
        "Gb"
      ];
      words.en = [
        "gigabit"
        "gigabits"
      ];
      words.pt = [
        "gigabit"
        "gigabits"
      ];
    }
    {
      id = "kibibit";
      category = "data";
      dimension.data = 1;
      factor = "1024";
      symbols = [ "Kibit" ];
      words.en = [
        "kibibit"
        "kibibits"
      ];
      words.pt = [
        "kibibit"
        "kibibits"
      ];
    }
    {
      id = "mebibit";
      category = "data";
      dimension.data = 1;
      factor = "1048576";
      symbols = [ "Mibit" ];
      words.en = [
        "mebibit"
        "mebibits"
      ];
      words.pt = [
        "mebibit"
        "mebibits"
      ];
    }
    {
      id = "kilobyte";
      category = "data";
      dimension.data = 1;
      factor = "8000";
      symbols = [
        "kB"
        "KB"
      ];
      words.en = [
        "kilobyte"
        "kilobytes"
      ];
      words.pt = [
        "quilobyte"
        "quilobytes"
      ];
    }
    {
      id = "megabyte";
      category = "data";
      dimension.data = 1;
      factor = "8000000";
      symbols = [ "MB" ];
      words.en = [
        "megabyte"
        "megabytes"
      ];
      words.pt = [
        "megabyte"
        "megabytes"
      ];
    }
    {
      id = "gigabyte";
      category = "data";
      dimension.data = 1;
      factor = "8000000000";
      symbols = [ "GB" ];
      words.en = [
        "gigabyte"
        "gigabytes"
      ];
      words.pt = [
        "gigabyte"
        "gigabytes"
      ];
    }
    {
      id = "terabyte";
      category = "data";
      dimension.data = 1;
      factor = "8000000000000";
      symbols = [ "TB" ];
      words.en = [
        "terabyte"
        "terabytes"
      ];
      words.pt = [
        "terabyte"
        "terabytes"
      ];
    }
    {
      id = "petabyte";
      category = "data";
      dimension.data = 1;
      factor = "8000000000000000";
      symbols = [ "PB" ];
      words.en = [
        "petabyte"
        "petabytes"
      ];
      words.pt = [
        "petabyte"
        "petabytes"
      ];
    }
    {
      id = "kibibyte";
      category = "data";
      dimension.data = 1;
      factor = "8192";
      symbols = [ "KiB" ];
      words.en = [
        "kibibyte"
        "kibibytes"
      ];
      words.pt = [
        "kibibyte"
        "kibibytes"
      ];
    }
    {
      id = "mebibyte";
      category = "data";
      dimension.data = 1;
      factor = "8388608";
      symbols = [ "MiB" ];
      words.en = [
        "mebibyte"
        "mebibytes"
      ];
      words.pt = [
        "mebibyte"
        "mebibytes"
      ];
    }
    {
      id = "gibibyte";
      category = "data";
      dimension.data = 1;
      factor = "8589934592";
      symbols = [ "GiB" ];
      words.en = [
        "gibibyte"
        "gibibytes"
      ];
      words.pt = [
        "gibibyte"
        "gibibytes"
      ];
    }
    {
      id = "tebibyte";
      category = "data";
      dimension.data = 1;
      factor = "8796093022208";
      symbols = [ "TiB" ];
      words.en = [
        "tebibyte"
        "tebibytes"
      ];
      words.pt = [
        "tebibyte"
        "tebibytes"
      ];
    }
    {
      id = "pebibyte";
      category = "data";
      dimension.data = 1;
      factor = "9007199254740992";
      symbols = [ "PiB" ];
      words.en = [
        "pebibyte"
        "pebibytes"
      ];
      words.pt = [
        "pebibyte"
        "pebibytes"
      ];
    }

    # ---- energy --------------------------------------------------------
    {
      id = "joule";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "1";
      symbols = [ "J" ];
      words.en = [
        "joule"
        "joules"
      ];
      words.pt = [
        "joule"
        "joules"
      ];
    }
    {
      id = "kilojoule";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "1000";
      symbols = [ "kJ" ];
      words.en = [
        "kilojoule"
        "kilojoules"
      ];
      words.pt = [
        "quilojoule"
        "quilojoules"
      ];
    }
    {
      id = "megajoule";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "1000000";
      symbols = [ "MJ" ];
      words.en = [
        "megajoule"
        "megajoules"
      ];
      words.pt = [
        "megajoule"
        "megajoules"
      ];
    }
    {
      # The thermochemical calorie.
      id = "calorie";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "4.184";
      symbols = [ "cal" ];
      words.en = [
        "calorie"
        "calories"
      ];
      words.pt = [
        "caloria"
        "calorias"
      ];
    }
    {
      id = "kilocalorie";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "4184";
      symbols = [
        "kcal"
        "Cal"
      ];
      words.en = [
        "kilocalorie"
        "kilocalories"
      ];
      words.pt = [
        "quilocaloria"
        "quilocalorias"
      ];
    }
    {
      id = "watt-hour";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "3600";
      symbols = [ "Wh" ];
      words.en = [
        "watt-hour"
        "watt-hours"
        "watt hour"
        "watt hours"
      ];
      words.pt = [
        "watt-hora"
        "watts-hora"
        "watt hora"
      ];
    }
    {
      id = "kilowatt-hour";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "3600000";
      symbols = [ "kWh" ];
      words.en = [
        "kilowatt-hour"
        "kilowatt-hours"
        "kilowatt hour"
        "kilowatt hours"
      ];
      words.pt = [
        "quilowatt-hora"
        "quilowatts-hora"
        "quilowatt hora"
      ];
    }
    {
      # Exact since the 2019 redefinition of the SI.
      id = "electronvolt";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "0.0000000000000000001602176634";
      symbols = [ "eV" ];
      words.en = [
        "electronvolt"
        "electronvolts"
      ];
      words.pt = [
        "elétron-volt"
        "elétrons-volt"
        "eletronvolt"
      ];
    }
    {
      # The International Table British thermal unit.
      id = "british-thermal-unit";
      category = "energy";
      dimension = {
        length = 2;
        mass = 1;
        time = -2;
      };
      factor = "1055.05585262";
      symbols = [
        "BTU"
        "Btu"
      ];
      words.en = [
        "British thermal unit"
        "British thermal units"
      ];
      words.pt = [
        "unidade térmica britânica"
        "unidades térmicas britânicas"
      ];
    }

    # ---- power ---------------------------------------------------------
    {
      id = "watt";
      category = "power";
      dimension = {
        length = 2;
        mass = 1;
        time = -3;
      };
      factor = "1";
      symbols = [ "W" ];
      words.en = [
        "watt"
        "watts"
      ];
      words.pt = [
        "watt"
        "watts"
      ];
    }
    {
      id = "kilowatt";
      category = "power";
      dimension = {
        length = 2;
        mass = 1;
        time = -3;
      };
      factor = "1000";
      symbols = [ "kW" ];
      words.en = [
        "kilowatt"
        "kilowatts"
      ];
      words.pt = [
        "quilowatt"
        "quilowatts"
      ];
    }
    {
      id = "megawatt";
      category = "power";
      dimension = {
        length = 2;
        mass = 1;
        time = -3;
      };
      factor = "1000000";
      symbols = [ "MW" ];
      words.en = [
        "megawatt"
        "megawatts"
      ];
      words.pt = [
        "megawatt"
        "megawatts"
      ];
    }
    {
      # Mechanical horsepower: 550 foot-pounds-force per second.
      id = "horsepower";
      category = "power";
      dimension = {
        length = 2;
        mass = 1;
        time = -3;
      };
      factor = "550*0.3048*0.45359237*9.80665";
      symbols = [ "hp" ];
      words.en = [
        "horsepower"
        "horsepower"
      ];
      words.pt = [
        "horsepower"
        "horsepower"
      ];
    }
    {
      # Metric horsepower: 75 kilograms-force metre per second.
      id = "metric-horsepower";
      category = "power";
      dimension = {
        length = 2;
        mass = 1;
        time = -3;
      };
      factor = "735.49875";
      symbols = [ "PS" ];
      words.en = [
        "metric horsepower"
        "metric horsepower"
      ];
      words.pt = [
        "cavalo-vapor"
        "cavalos-vapor"
        "cv"
      ];
    }

    # ---- pressure ------------------------------------------------------
    {
      id = "pascal";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "1";
      symbols = [ "Pa" ];
      words.en = [
        "pascal"
        "pascals"
      ];
      words.pt = [
        "pascal"
        "pascals"
      ];
    }
    {
      id = "hectopascal";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "100";
      symbols = [ "hPa" ];
      words.en = [
        "hectopascal"
        "hectopascals"
      ];
      words.pt = [
        "hectopascal"
        "hectopascals"
      ];
    }
    {
      id = "kilopascal";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "1000";
      symbols = [ "kPa" ];
      words.en = [
        "kilopascal"
        "kilopascals"
      ];
      words.pt = [
        "quilopascal"
        "quilopascals"
      ];
    }
    {
      id = "megapascal";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "1000000";
      symbols = [ "MPa" ];
      words.en = [
        "megapascal"
        "megapascals"
      ];
      words.pt = [
        "megapascal"
        "megapascals"
      ];
    }
    {
      id = "bar";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "100000";
      symbols = [ "bar" ];
      words.en = [
        "bar"
        "bars"
      ];
      words.pt = [
        "bar"
        "bares"
      ];
    }
    {
      id = "millibar";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "100";
      symbols = [ "mbar" ];
      words.en = [
        "millibar"
        "millibars"
      ];
      words.pt = [
        "milibar"
        "milibares"
      ];
    }
    {
      id = "atmosphere";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "101325";
      symbols = [ "atm" ];
      words.en = [
        "atmosphere"
        "atmospheres"
      ];
      words.pt = [
        "atmosfera"
        "atmosferas"
      ];
    }
    {
      # A pound-force over a square inch.
      id = "pound-per-square-inch";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "0.45359237*9.80665/0.00064516";
      symbols = [ "psi" ];
      words.en = [
        "pound per square inch"
        "pounds per square inch"
      ];
      words.pt = [
        "libra por polegada quadrada"
        "libras por polegada quadrada"
      ];
    }
    {
      id = "millimetre-of-mercury";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "133.322387415";
      symbols = [ "mmHg" ];
      words.en = [
        "millimetre of mercury"
        "millimetres of mercury"
        "millimeter of mercury"
        "millimeters of mercury"
      ];
      words.pt = [
        "milímetro de mercúrio"
        "milímetros de mercúrio"
      ];
    }
    {
      id = "torr";
      category = "pressure";
      dimension = {
        length = -1;
        mass = 1;
        time = -2;
      };
      factor = "101325/760";
      symbols = [ "Torr" ];
      words.en = [
        "torr"
        "torrs"
      ];
      words.pt = [
        "torr"
        "torrs"
      ];
    }

    # ---- angle ---------------------------------------------------------
    {
      id = "radian";
      category = "angle";
      dimension.angle = 1;
      factor = "1";
      symbols = [ "rad" ];
      words.en = [
        "radian"
        "radians"
      ];
      words.pt = [
        "radiano"
        "radianos"
      ];
    }
    {
      id = "degree";
      category = "angle";
      dimension.angle = 1;
      factor = "1/180";
      pi = 1;
      symbols = [
        "°"
        "º"
        "deg"
      ];
      words.en = [
        "degree"
        "degrees"
      ];
      words.pt = [
        "grau"
        "graus"
      ];
    }
    {
      id = "gradian";
      category = "angle";
      dimension.angle = 1;
      factor = "1/200";
      pi = 1;
      symbols = [
        "grad"
        "gon"
      ];
      words.en = [
        "gradian"
        "gradians"
      ];
      words.pt = [
        "grado"
        "grados"
      ];
    }
    {
      id = "arcminute";
      category = "angle";
      dimension.angle = 1;
      factor = "1/10800";
      pi = 1;
      symbols = [
        "′"
        "arcmin"
      ];
      words.en = [
        "arcminute"
        "arcminutes"
        "minute of arc"
        "minutes of arc"
      ];
      words.pt = [
        "minuto de arco"
        "minutos de arco"
      ];
    }
    {
      id = "arcsecond";
      category = "angle";
      dimension.angle = 1;
      factor = "1/648000";
      pi = 1;
      symbols = [ "arcsec" ];
      words.en = [
        "arcsecond"
        "arcseconds"
        "second of arc"
        "seconds of arc"
      ];
      words.pt = [
        "segundo de arco"
        "segundos de arco"
      ];
    }
    {
      id = "turn";
      category = "angle";
      dimension.angle = 1;
      factor = "2";
      pi = 1;
      symbols = [ "rev" ];
      words.en = [
        "turn"
        "turns"
        "revolution"
        "revolutions"
      ];
      words.pt = [
        "volta"
        "voltas"
        "revolução"
        "revoluções"
      ];
    }

    # ---- frequency -----------------------------------------------------
    {
      id = "hertz";
      category = "frequency";
      dimension.time = -1;
      factor = "1";
      symbols = [ "Hz" ];
      words.en = [
        "hertz"
        "hertz"
      ];
      words.pt = [
        "hertz"
        "hertz"
      ];
    }
    {
      id = "kilohertz";
      category = "frequency";
      dimension.time = -1;
      factor = "1000";
      symbols = [ "kHz" ];
      words.en = [
        "kilohertz"
        "kilohertz"
      ];
      words.pt = [
        "quilohertz"
        "quilohertz"
      ];
    }
    {
      id = "megahertz";
      category = "frequency";
      dimension.time = -1;
      factor = "1000000";
      symbols = [ "MHz" ];
      words.en = [
        "megahertz"
        "megahertz"
      ];
      words.pt = [
        "megahertz"
        "megahertz"
      ];
    }
    {
      id = "gigahertz";
      category = "frequency";
      dimension.time = -1;
      factor = "1000000000";
      symbols = [ "GHz" ];
      words.en = [
        "gigahertz"
        "gigahertz"
      ];
      words.pt = [
        "gigahertz"
        "gigahertz"
      ];
    }
    {
      id = "revolution-per-minute";
      category = "frequency";
      dimension.time = -1;
      factor = "1/60";
      symbols = [ "rpm" ];
      words.en = [
        "revolution per minute"
        "revolutions per minute"
      ];
      words.pt = [
        "rotação por minuto"
        "rotações por minuto"
      ];
    }

    # ---- fuel economy (reciprocal) -------------------------------------
    #
    # Volume per distance is an AREA (m³/m), and these are kept apart from the
    # area category by category, not by dimension: `L/100km in m²` is refused.
    {
      id = "litre-per-100-km";
      category = "fuel-economy";
      dimension.length = 2;
      factor = "0.001/100000";
      symbols = [
        "L/100km"
        "l/100km"
        "L/100 km"
        "l/100 km"
      ];
      words.en = [
        "litre per 100 kilometres"
        "litres per 100 kilometres"
        "liters per 100 kilometers"
      ];
      words.pt = [
        "litro por 100 quilômetros"
        "litros por 100 quilômetros"
      ];
    }
    {
      id = "kilometre-per-litre";
      category = "fuel-economy";
      dimension.length = 2;
      factor = "0.001/1000";
      reciprocal = true;
      symbols = [
        "km/L"
        "km/l"
      ];
      words.en = [
        "kilometre per litre"
        "kilometres per litre"
        "kilometers per liter"
      ];
      words.pt = [
        "quilômetro por litro"
        "quilômetros por litro"
      ];
    }
    {
      id = "mile-per-gallon";
      category = "fuel-economy";
      dimension.length = 2;
      factor = "0.003785411784/1609.344";
      reciprocal = true;
      symbols = [ "mpg" ];
      words.en = [
        "mile per gallon"
        "miles per gallon"
      ];
      words.pt = [
        "milha por galão"
        "milhas por galão"
      ];
    }
    {
      id = "mile-per-imperial-gallon";
      category = "fuel-economy";
      dimension.length = 2;
      factor = "0.00454609/1609.344";
      reciprocal = true;
      symbols = [ ];
      words.en = [
        "mile per imperial gallon"
        "miles per imperial gallon"
      ];
      words.pt = [
        "milha por galão imperial"
        "milhas por galão imperial"
      ];
    }

    # ---- cooking -------------------------------------------------------
    #
    # Volumes, and convertible with the volume category: a cup is so many
    # millilitres. The customary US measures and the metric ones are different
    # units with different words — `colher de sopa` is the metric 15 mL.
    {
      # The US legal cup (nutrition labelling), and the Brazilian xícara.
      id = "cup";
      category = "cooking";
      dimension.length = 3;
      factor = "0.00024";
      symbols = [ ];
      words.en = [
        "cup"
        "cups"
      ];
      words.pt = [
        "xícara"
        "xícaras"
        "xícara de chá"
        "xícaras de chá"
        "copo"
        "copos"
      ];
    }
    {
      id = "customary-cup";
      category = "cooking";
      dimension.length = 3;
      factor = "0.0002365882365";
      symbols = [ ];
      words.en = [
        "customary cup"
        "customary cups"
        "us cup"
        "us cups"
      ];
      words.pt = [
        "xícara americana"
        "xícaras americanas"
      ];
    }
    {
      id = "metric-cup";
      category = "cooking";
      dimension.length = 3;
      factor = "0.00025";
      symbols = [ ];
      words.en = [
        "metric cup"
        "metric cups"
      ];
      words.pt = [
        "xícara métrica"
        "xícaras métricas"
      ];
    }
    {
      id = "tablespoon";
      category = "cooking";
      dimension.length = 3;
      factor = "0.000015";
      symbols = [ ];
      words.en = [
        "tablespoon"
        "tablespoons"
        "tbsp"
      ];
      words.pt = [
        "colher de sopa"
        "colheres de sopa"
      ];
    }
    {
      id = "customary-tablespoon";
      category = "cooking";
      dimension.length = 3;
      factor = "0.00001478676478125";
      symbols = [ ];
      words.en = [
        "customary tablespoon"
        "customary tablespoons"
        "us tablespoon"
        "us tablespoons"
      ];
      words.pt = [
        "colher de sopa americana"
        "colheres de sopa americanas"
      ];
    }
    {
      id = "teaspoon";
      category = "cooking";
      dimension.length = 3;
      factor = "0.000005";
      symbols = [ ];
      words.en = [
        "teaspoon"
        "teaspoons"
        "tsp"
      ];
      words.pt = [
        "colher de chá"
        "colheres de chá"
      ];
    }
    {
      id = "customary-teaspoon";
      category = "cooking";
      dimension.length = 3;
      factor = "0.00000492892159375";
      symbols = [ ];
      words.en = [
        "customary teaspoon"
        "customary teaspoons"
        "us teaspoon"
        "us teaspoons"
      ];
      words.pt = [
        "colher de chá americana"
        "colheres de chá americanas"
      ];
    }
  ];

  # THE CURRENCIES A PERSON NAMES BY A WORD OR A SIGN. Every code of `quoted`
  # below is understood as it is (`SEK`, `zar`); these are the ones with a
  # name or a symbol people actually type.
  currencies = [
    {
      code = "EUR";
      symbols = [ "€" ];
      words.en = [
        "euro"
        "euros"
      ];
      words.pt = [
        "euro"
        "euros"
      ];
    }
    {
      code = "USD";
      symbols = [
        "US$"
        "$"
      ];
      words.en = [
        "dollar"
        "dollars"
        "us dollar"
        "us dollars"
      ];
      words.pt = [
        "dólar"
        "dólares"
      ];
    }
    {
      code = "GBP";
      symbols = [ "£" ];
      words.en = [
        "pound sterling"
        "pounds sterling"
        "sterling"
      ];
      words.pt = [
        "libra esterlina"
        "libras esterlinas"
      ];
    }
    {
      code = "BRL";
      symbols = [ "R$" ];
      words.en = [
        "real"
        "reais"
        "brazilian real"
      ];
      words.pt = [
        "real"
        "reais"
      ];
    }
    {
      code = "JPY";
      symbols = [ "¥" ];
      words.en = [ "yen" ];
      words.pt = [
        "iene"
        "ienes"
      ];
    }
    {
      code = "CHF";
      symbols = [ ];
      words.en = [
        "swiss franc"
        "swiss francs"
      ];
      words.pt = [
        "franco suíço"
        "francos suíços"
      ];
    }
  ];

  # THE ISO 4217 CODES A PERSON MAY TYPE, in any case: the ones the ECB's daily
  # file quotes against the euro. Only these are currencies — `10 FPS` and
  # `4 CPU` are not, and do not make a door ask for the rates. A code the ECB
  # adds later is refused as an unknown word until it is written here; one it
  # drops is refused as an unknown currency by the rates.
  quoted = [
    "AUD"
    "BRL"
    "CAD"
    "CHF"
    "CNY"
    "CZK"
    "DKK"
    "EUR"
    "GBP"
    "HKD"
    "HUF"
    "IDR"
    "ILS"
    "INR"
    "ISK"
    "JPY"
    "KRW"
    "MXN"
    "MYR"
    "NOK"
    "NZD"
    "PHP"
    "PLN"
    "RON"
    "SEK"
    "SGD"
    "THB"
    "TRY"
    "USD"
    "ZAR"
  ];

  # THE WORD A CONVERSION IS ASKED WITH (`10 km in mi`), and the one a
  # percentage is taken with (`20% of 150`).
  connectives = {
    into = {
      en = [
        "in"
        "to"
        "into"
        "as"
      ];
      pt = [
        "em"
        "para"
        "como"
      ];
    };
    of = {
      en = [ "of" ];
      pt = [
        "de"
        "do"
        "da"
      ];
    };
  };

  # THE BASES A NUMBER CAN BE SHOWN IN (`255 in hex`).
  radices = {
    decimal = {
      en = [
        "dec"
        "decimal"
      ];
      pt = [
        "dec"
        "decimal"
      ];
    };
    hexadecimal = {
      en = [
        "hex"
        "hexadecimal"
      ];
      pt = [
        "hex"
        "hexadecimal"
      ];
    };
    binary = {
      en = [
        "bin"
        "binary"
      ];
      pt = [
        "bin"
        "binário"
      ];
    };
    octal = {
      en = [
        "oct"
        "octal"
      ];
      pt = [
        "oct"
        "octal"
      ];
    };
    roman = {
      en = [
        "roman"
        "roman numerals"
      ];
      pt = [
        "romano"
        "romanos"
        "algarismos romanos"
      ];
    };
  };

  # THE INSTANTS a date is counted from.
  instants = {
    now = {
      en = [ "now" ];
      pt = [ "agora" ];
    };
    today = {
      en = [ "today" ];
      pt = [ "hoje" ];
    };
  };

  # THE FUNCTIONS. Their names are notation, the same in every language (`*`),
  # with the Portuguese school names beside them.
  functions = {
    sqrt."*" = [
      "sqrt"
      "√"
    ];
    sqrt.pt = [ "raiz" ];
    cbrt."*" = [
      "cbrt"
      "∛"
    ];
    abs."*" = [ "abs" ];
    floor."*" = [ "floor" ];
    ceil."*" = [ "ceil" ];
    round."*" = [ "round" ];
    exp."*" = [ "exp" ];
    ln."*" = [ "ln" ];
    log."*" = [
      "log"
      "log10"
    ];
    log2."*" = [ "log2" ];
    sin."*" = [ "sin" ];
    sin.pt = [ "sen" ];
    cos."*" = [ "cos" ];
    tan."*" = [ "tan" ];
    tan.pt = [ "tg" ];
    asin."*" = [
      "asin"
      "arcsin"
    ];
    asin.pt = [ "arcsen" ];
    acos."*" = [
      "acos"
      "arccos"
    ];
    atan."*" = [
      "atan"
      "arctan"
    ];
    atan.pt = [ "arctg" ];
    sinh."*" = [ "sinh" ];
    cosh."*" = [ "cosh" ];
    tanh."*" = [ "tanh" ];
  };

  # THE CONSTANTS, notation too.
  constants = {
    pi."*" = [
      "pi"
      "π"
    ];
    e."*" = [ "e" ];
    tau."*" = [
      "tau"
      "τ"
    ];
  };

  # WHAT THE UNITS ARE BY DEFINITION, written apart from their factors: `value
  # from` is `is to` (times π^pi). The generator carries them to the engine's
  # test and the `units-are-total` check, which compute each one through the
  # factors — a factor typed wrong, or an engine that converts an affine, a
  # reciprocal or a π unit wrong, fails one of them. Every category has some,
  # and so does every kind of conversion.
  known = [
    {
      value = "1";
      from = "mile";
      to = "kilometre";
      is = "1.609344";
    }
    {
      value = "1";
      from = "inch";
      to = "centimetre";
      is = "2.54";
    }
    {
      value = "1";
      from = "foot";
      to = "inch";
      is = "12";
    }
    {
      value = "1";
      from = "yard";
      to = "foot";
      is = "3";
    }
    {
      value = "1";
      from = "nautical-mile";
      to = "metre";
      is = "1852";
    }
    {
      value = "1";
      from = "light-year";
      to = "metre";
      is = "299792458*31557600";
    }
    {
      value = "1";
      from = "pound";
      to = "gram";
      is = "453.59237";
    }
    {
      value = "1";
      from = "pound";
      to = "ounce";
      is = "16";
    }
    {
      value = "1";
      from = "stone";
      to = "pound";
      is = "14";
    }
    {
      value = "1";
      from = "short-ton";
      to = "pound";
      is = "2000";
    }
    {
      value = "1";
      from = "long-ton";
      to = "pound";
      is = "2240";
    }
    {
      value = "1";
      from = "carat";
      to = "milligram";
      is = "200";
    }
    {
      value = "1";
      from = "gallon";
      to = "litre";
      is = "3.785411784";
    }
    {
      value = "1";
      from = "gallon";
      to = "fluid-ounce";
      is = "128";
    }
    {
      value = "1";
      from = "quart";
      to = "pint";
      is = "2";
    }
    {
      value = "1";
      from = "imperial-gallon";
      to = "litre";
      is = "4.54609";
    }
    {
      value = "1";
      from = "cubic-foot";
      to = "cubic-inch";
      is = "1728";
    }
    {
      value = "1";
      from = "cubic-inch";
      to = "cubic-centimetre";
      is = "16.387064";
    }
    {
      value = "1";
      from = "acre";
      to = "square-foot";
      is = "43560";
    }
    {
      value = "1";
      from = "square-mile";
      to = "acre";
      is = "640";
    }
    {
      value = "1";
      from = "hectare";
      to = "square-metre";
      is = "10000";
    }
    {
      value = "1";
      from = "square-yard";
      to = "square-foot";
      is = "9";
    }
    {
      value = "0";
      from = "celsius";
      to = "fahrenheit";
      is = "32";
    }
    {
      value = "100";
      from = "celsius";
      to = "fahrenheit";
      is = "212";
    }
    {
      value = "-40";
      from = "celsius";
      to = "fahrenheit";
      is = "-40";
    }
    {
      value = "0";
      from = "kelvin";
      to = "celsius";
      is = "-273.15";
    }
    {
      value = "0";
      from = "fahrenheit";
      to = "rankine";
      is = "459.67";
    }
    {
      value = "1";
      from = "week";
      to = "day";
      is = "7";
    }
    {
      value = "1";
      from = "year";
      to = "day";
      is = "365.2425";
    }
    {
      value = "1";
      from = "year";
      to = "month";
      is = "12";
    }
    {
      value = "1";
      from = "knot";
      to = "kilometre-per-hour";
      is = "1.852";
    }
    {
      value = "1";
      from = "mile-per-hour";
      to = "kilometre-per-hour";
      is = "1.609344";
    }
    {
      value = "1";
      from = "metre-per-second";
      to = "kilometre-per-hour";
      is = "3.6";
    }
    {
      value = "1";
      from = "foot-per-second";
      to = "metre-per-second";
      is = "0.3048";
    }
    {
      value = "1";
      from = "byte";
      to = "bit";
      is = "8";
    }
    {
      value = "1";
      from = "kibibyte";
      to = "byte";
      is = "1024";
    }
    {
      value = "1";
      from = "gibibyte";
      to = "mebibyte";
      is = "1024";
    }
    {
      value = "1";
      from = "megabyte";
      to = "kilobyte";
      is = "1000";
    }
    {
      value = "1";
      from = "megabit";
      to = "kibibit";
      is = "1000000/1024";
    }
    {
      value = "1";
      from = "kilowatt-hour";
      to = "megajoule";
      is = "3.6";
    }
    {
      value = "1";
      from = "kilocalorie";
      to = "kilojoule";
      is = "4.184";
    }
    {
      value = "1";
      from = "british-thermal-unit";
      to = "joule";
      is = "1055.05585262";
    }
    {
      value = "1";
      from = "electronvolt";
      to = "joule";
      is = "1.602176634/10000000000000000000";
    }
    {
      value = "1";
      from = "horsepower";
      to = "watt";
      is = "0.45359237*9.80665*0.3048*550";
    }
    {
      value = "1";
      from = "metric-horsepower";
      to = "watt";
      is = "735.49875";
    }
    {
      value = "1";
      from = "atmosphere";
      to = "kilopascal";
      is = "101.325";
    }
    {
      value = "1";
      from = "atmosphere";
      to = "torr";
      is = "760";
    }
    {
      value = "1";
      from = "bar";
      to = "millibar";
      is = "1000";
    }
    {
      value = "1";
      from = "pound-per-square-inch";
      to = "pascal";
      is = "0.45359237*9.80665/0.00064516";
    }
    {
      value = "1";
      from = "millimetre-of-mercury";
      to = "pascal";
      is = "133.322387415";
    }
    {
      value = "1";
      from = "turn";
      to = "degree";
      is = "360";
    }
    {
      value = "1";
      from = "degree";
      to = "arcminute";
      is = "60";
    }
    {
      value = "1";
      from = "arcminute";
      to = "arcsecond";
      is = "60";
    }
    {
      value = "100";
      from = "gradian";
      to = "degree";
      is = "90";
    }
    {
      value = "180";
      from = "degree";
      to = "radian";
      is = "1";
      pi = 1;
    }
    {
      value = "1";
      from = "turn";
      to = "radian";
      is = "2";
      pi = 1;
    }
    {
      value = "1";
      from = "kilohertz";
      to = "hertz";
      is = "1000";
    }
    {
      value = "60";
      from = "revolution-per-minute";
      to = "hertz";
      is = "1";
    }
    {
      value = "1";
      from = "kilometre-per-litre";
      to = "litre-per-100-km";
      is = "100";
    }
    {
      value = "5";
      from = "litre-per-100-km";
      to = "kilometre-per-litre";
      is = "20";
    }
    {
      value = "1";
      from = "mile-per-gallon";
      to = "kilometre-per-litre";
      is = "1.609344/3.785411784";
    }
    {
      value = "1";
      from = "mile-per-imperial-gallon";
      to = "kilometre-per-litre";
      is = "1.609344/4.54609";
    }
    {
      value = "1";
      from = "cup";
      to = "millilitre";
      is = "240";
    }
    {
      value = "1";
      from = "metric-cup";
      to = "millilitre";
      is = "250";
    }
    {
      value = "1";
      from = "customary-cup";
      to = "fluid-ounce";
      is = "8";
    }
    {
      value = "1";
      from = "tablespoon";
      to = "millilitre";
      is = "15";
    }
    {
      value = "1";
      from = "customary-cup";
      to = "customary-tablespoon";
      is = "16";
    }
    {
      value = "1";
      from = "customary-tablespoon";
      to = "customary-teaspoon";
      is = "3";
    }
    {
      value = "1";
      from = "teaspoon";
      to = "millilitre";
      is = "5";
    }
  ];
}
