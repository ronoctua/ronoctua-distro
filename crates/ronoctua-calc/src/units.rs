//! The table of units, generated from `assets/units.nix`.
//!
//! `nix/unitsgen.nix` writes the table (`RONOCTUA_UNITS_RS`, in `generated`):
//! each unit a [`Dimension`] and an EXACT factor to its dimension's base unit,
//! an offset for the affine ones (temperature), a reciprocal flag for fuel
//! economy (L/100 km ↔ mpg), and search aliases per language. The generator
//! refuses a duplicate alias and a unit without words in some language.
//!
//! Beside the units the table carries the other words the parser knows —
//! the currencies' names and signs, and [`Means`]: `in`, `of`, `hex`, `now`,
//! `sqrt`, `pi` — under the same two rules: every language has words for each,
//! and no word means two things.

/// A unit: an index into [`Table::units`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(pub u16);

/// Powers of the base dimensions, in the order of the constants below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dimension(pub [i8; 6]);

impl Dimension {
    /// The index of length (metre).
    pub const LENGTH: usize = 0;
    /// The index of mass (kilogram).
    pub const MASS: usize = 1;
    /// The index of time (second).
    pub const TIME: usize = 2;
    /// The index of temperature (kelvin).
    pub const TEMPERATURE: usize = 3;
    /// The index of information (bit).
    pub const DATA: usize = 4;
    /// The index of plane angle (radian).
    pub const ANGLE: usize = 5;
}

/// What a unit measures — the converter's category ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Length.
    Length,
    /// Mass.
    Mass,
    /// Volume.
    Volume,
    /// Area.
    Area,
    /// Temperature (affine).
    Temperature,
    /// Time.
    Time,
    /// Speed.
    Speed,
    /// Data, SI and IEC, bits and bytes.
    Data,
    /// Energy.
    Energy,
    /// Power.
    Power,
    /// Pressure.
    Pressure,
    /// Plane angle.
    Angle,
    /// Frequency.
    Frequency,
    /// Fuel economy (reciprocal between L/100 km and mpg).
    FuelEconomy,
    /// Cooking measures.
    Cooking,
}

impl Category {
    /// Every category, in the ring's order.
    pub const ALL: [Self; 15] = [
        Self::Length,
        Self::Mass,
        Self::Volume,
        Self::Area,
        Self::Temperature,
        Self::Time,
        Self::Speed,
        Self::Data,
        Self::Energy,
        Self::Power,
        Self::Pressure,
        Self::Angle,
        Self::Frequency,
        Self::FuelEconomy,
        Self::Cooking,
    ];

    /// Its kebab-case name, the suffix of its key (`calc-category-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Mass => "mass",
            Self::Volume => "volume",
            Self::Area => "area",
            Self::Temperature => "temperature",
            Self::Time => "time",
            Self::Speed => "speed",
            Self::Data => "data",
            Self::Energy => "energy",
            Self::Power => "power",
            Self::Pressure => "pressure",
            Self::Angle => "angle",
            Self::Frequency => "frequency",
            Self::FuelEconomy => "fuel-economy",
            Self::Cooking => "cooking",
        }
    }
}

/// One unit of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    /// Its id: the tail of its display key (`unit-<id>`).
    pub id: &'static str,
    /// Its symbol as a formula writes it (`km`, `°C`, `MiB`) — the same in
    /// every language.
    pub symbol: &'static str,
    /// What it measures.
    pub category: Category,
    /// Its dimension.
    pub dimension: Dimension,
    /// The exact factor to the base unit of its dimension, as numerator and
    /// denominator (`mi` = 1609344/1000 m).
    pub factor: (i128, i128),
    /// The offset added after the factor, for an affine unit (`°C` → K adds
    /// 27315/100); `(0, 1)` for every other.
    pub offset: (i128, i128),
    /// The power of π in the factor: a degree is π/180 radian, and no
    /// rational says so. A conversion that does not cancel it is approximate.
    pub pi: i8,
    /// Whether it converts by the reciprocal (mpg against L/100 km).
    pub reciprocal: bool,
}

impl Unit {
    /// Whether it is affine (an offset is added): a temperature scale.
    #[must_use]
    pub const fn is_affine(&self) -> bool {
        self.offset.0 != 0
    }
}

/// A word a person types for a unit, in one language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alias {
    /// The language subtag (`en`, `pt`), or `*` for a SYMBOL — the same in
    /// every language, and matched as written (`MB` is not `mB`).
    pub subtag: &'static str,
    /// The word, folded (lowercase, no accents); a symbol as written.
    pub word: &'static str,
    /// The unit it names.
    pub unit: UnitId,
}

/// A base a number is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Radix {
    /// Base ten.
    Decimal,
    /// Base sixteen, `0xFF`.
    Hexadecimal,
    /// Base two, `0b1010`.
    Binary,
    /// Base eight, `0o17`.
    Octal,
    /// Roman numerals, 1 to 3999.
    Roman,
}

/// A function the parser knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Function {
    /// Square root.
    Sqrt,
    /// Cube root.
    Cbrt,
    /// Absolute value.
    Abs,
    /// Rounded down.
    Floor,
    /// Rounded up.
    Ceil,
    /// Rounded half away from zero.
    Round,
    /// `e` to the power.
    Exp,
    /// Natural logarithm.
    Ln,
    /// Base-ten logarithm.
    Log,
    /// Base-two logarithm.
    Log2,
    /// Sine.
    Sin,
    /// Cosine.
    Cos,
    /// Tangent.
    Tan,
    /// Arcsine.
    Asin,
    /// Arccosine.
    Acos,
    /// Arctangent.
    Atan,
    /// Hyperbolic sine.
    Sinh,
    /// Hyperbolic cosine.
    Cosh,
    /// Hyperbolic tangent.
    Tanh,
}

/// A constant the parser knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constant {
    /// π.
    Pi,
    /// Euler's number.
    E,
    /// τ = 2π.
    Tau,
}

/// What a word that is not a unit means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Means {
    /// The conversion: `10 km in mi`.
    Into,
    /// The percentage's object: `20% of 150`.
    Of,
    /// A base to show a number in.
    Radix(Radix),
    /// This instant.
    Now,
    /// Today, at midnight.
    Today,
    /// A function.
    Function(Function),
    /// A constant.
    Constant(Constant),
}

/// A definition the factors are tested against: `value from` is `is to`,
/// times π to the power `pi` (`assets/units.nix`, `known`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Known {
    /// The amount converted, as a fraction.
    pub value: (i128, i128),
    /// The unit it is in, by id.
    pub from: &'static str,
    /// The unit it is converted into, by id.
    pub to: &'static str,
    /// What it is by definition, as a fraction.
    pub is: (i128, i128),
    /// The power of π that goes with `is` (a degree into a radian).
    pub pi: i8,
}

// THE GENERATED TABLE: `UNITS`, `ALIASES`, `CURRENCY_WORDS`, `KNOWN`,
// `CURRENCY_CODES`, `KEYWORDS` and `SUBTAGS`, from `assets/units.nix` by `nix/unitsgen.nix`.
include!(concat!(env!("OUT_DIR"), "/units.rs"));

/// The whole table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Table {
    /// Every unit; a [`UnitId`] indexes this.
    pub units: &'static [Unit],
    /// Every alias, sorted by word.
    pub aliases: &'static [Alias],
}

/// The table.
static TABLE: Table = Table {
    units: &UNITS,
    aliases: &ALIASES,
};

/// The table.
#[must_use]
pub fn table() -> &'static Table {
    &TABLE
}

impl Table {
    /// The unit of an id.
    #[must_use]
    pub fn unit(&self, id: UnitId) -> Option<&'static Unit> {
        self.units.get(usize::from(id.0))
    }

    /// The unit whose id (the tail of `unit-<id>`) is `id`.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<UnitId> {
        self.units
            .iter()
            .position(|u| u.id == id)
            .and_then(|n| u16::try_from(n).ok())
            .map(UnitId)
    }

    /// The units of a category, in the declared order (the converter's list).
    #[must_use]
    pub fn of(&self, category: Category) -> Vec<UnitId> {
        (0..self.units.len())
            .filter(|n| self.units[*n].category == category)
            .filter_map(|n| u16::try_from(n).ok().map(UnitId))
            .collect()
    }
}

/// The units a word names in the language `subtag`: a symbol as written
/// (`MB`), or a word folded (`metros`, `Métros`) — the generator refuses a
/// word that names two units, so there is at most one; a `Vec` all the same,
/// so a caller never has to know that.
#[must_use]
pub fn find(word: &str, subtag: &str) -> Vec<UnitId> {
    let folded = fold(word);
    let mut out: Vec<UnitId> = table()
        .aliases
        .iter()
        .filter(|a| {
            if a.subtag == "*" {
                a.word == word
            } else {
                a.subtag == subtag && a.word == folded
            }
        })
        .map(|a| a.unit)
        .collect();
    out.dedup();
    out
}

/// Lowercase without accents, one character for one character — the table's
/// fold (`nix/scripts/gen-units.py` folds with the same table).
#[must_use]
pub fn fold(word: &str) -> String {
    word.chars().map(fold_char).collect()
}

/// One character, lowered and without its accent.
#[must_use]
pub fn fold_char(c: char) -> char {
    ronoctua_apps::fold::folded_char(c.to_lowercase().next().unwrap_or(c))
}
