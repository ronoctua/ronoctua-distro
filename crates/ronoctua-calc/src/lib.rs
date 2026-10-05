//! The calculator's engine — for the calculator app and fast-access's `/=`.
//!
//! # Exact until it cannot be
//!
//! A number is a [`Number::Exact`] `BigRational` through `+ − × ÷` and integer
//! powers, so `0.1 + 0.2` is `0.3` and `1/3 × 3` is `1`. Only what has no
//! exact answer — a root, a logarithm, a trigonometric function — becomes a
//! [`Number::Approx`], which the doors draw with `≈`.
//!
//! # Pure
//!
//! [`parse`] and [`eval`] read nothing and ask nothing: the decimal mark comes
//! from the person's language (`Texts::decimal_mark`), and the time, the zone
//! and the exchange rates come in an [`Env`] the caller built. The one request
//! this crate makes is [`rates::fetch`] (feature `net`), the ECB's daily
//! reference rates, and it takes a `Pass` for `www.ecb.europa.eu` — asked only
//! when a currency is typed and the cached rates are stale.
//!
//! # Units
//!
//! [`units::table`] is generated from `assets/units.nix` (length, mass,
//! volume, area, temperature, time, speed, data, energy, power, pressure,
//! angle, frequency, fuel economy, cooking): each unit a dimension vector and
//! an exact factor. Its display names are the door's (`unit-<id>`, with a
//! numeric selector); its aliases per language are data.
//!
//! # Refusals
//!
//! Every failure is a [`Refusal`]: where in the text ([`Refusal::at`], a
//! byte offset) and why ([`Why`], a closed set with a sentence each in the
//! door, `calc-why-<id>`).

#![forbid(unsafe_code)]

use std::path::PathBuf;

pub use num_bigint::BigInt;
pub use num_rational::BigRational;
use num_traits::{Signed as _, ToPrimitive as _, Zero as _};

mod eval;
mod lex;
mod num;
mod parse;
pub mod rates;
pub mod units;

pub use units::{Radix, UnitId};

/// The most answers the calculator's history keeps (`calcHistoryEntries` in
/// `assets/cache.nix`): the newest, and the oldest goes as a new one comes.
pub const HISTORY_ENTRIES: usize = rates::policy::CALC_HISTORY_ENTRIES;

/// A parsed expression, ready for [`eval`].
///
/// Opaque: its tree is the engine's to shape. It remembers the text it was
/// parsed from, so a refusal can point into it.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    source: String,
    root: parse::Node,
}

impl Expr {
    /// The text it was parsed from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether evaluating it needs the exchange rates: it names two different
    /// currencies (`10 USD in BRL`). One currency alone (`10 USD + 5 USD`)
    /// needs none — and a door asks for consent only when this says so.
    #[must_use]
    pub fn wants_rates(&self) -> bool {
        let mut seen: Vec<rates::Currency> = Vec::new();
        walk(&self.root, &mut |n| {
            if let parse::NodeKind::Currency(c) = n.kind
                && !seen.contains(&c)
            {
                seen.push(c);
            }
        });
        seen.len() > 1
    }
}

/// Every node of a tree, the root first.
fn walk(node: &parse::Node, visit: &mut dyn FnMut(&parse::Node)) {
    use parse::NodeKind as K;
    visit(node);
    match &node.kind {
        K::Negate(a) | K::Percent(a) | K::Factorial(a) | K::Call(_, a) => walk(a, visit),
        K::Add(a, b) | K::Subtract(a, b) | K::Multiply(a, b) | K::Divide(a, b) | K::Power(a, b) => {
            walk(a, visit);
            walk(b, visit);
        }
        K::Convert(a, target) => {
            walk(a, visit);
            if let parse::Target::Units(t) = target {
                walk(t, visit);
            }
        }
        _ => {}
    }
}

/// A number: exact, or approximate once a transcendental touched it.
#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    /// An exact rational.
    Exact(BigRational),
    /// An approximation (drawn with `≈`).
    Approx(f64),
}

impl Number {
    /// Whether it is exact.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    /// Its nearest `f64` — for a drawing, never for more arithmetic.
    #[must_use]
    pub fn to_f64(&self) -> f64 {
        match self {
            Self::Exact(r) => r.to_f64().unwrap_or(f64::NAN),
            Self::Approx(f) => *f,
        }
    }
}

/// What a quantity is counted in: a unit of the table, or a currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Base {
    /// A unit of [`units::table`].
    Unit(units::UnitId),
    /// A currency the ECB quotes (or the euro).
    Currency(rates::Currency),
}

/// A value's unit: a product of bases with integer powers (`km`, `km/h`,
/// `m²`, `EUR`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quantity {
    /// The bases and their powers, in the order they were written.
    pub terms: Vec<(Base, i8)>,
}

/// How a value is written: its digits in a base, a percentage, or a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shown {
    /// Decimal digits, with the language's mark.
    #[default]
    Decimal,
    /// A percentage: `0.15` is written `15%`.
    Percent,
    /// In a base (`255 in hex` is `0xFF`), or as a Roman numeral.
    Radix(Radix),
    /// The number is seconds since the epoch, and this is its date at the
    /// offset (seconds east of UTC) of the person's zone or of the zone `in`
    /// named.
    Date {
        /// Seconds east of UTC.
        offset_s: i32,
    },
    /// The same, with the time of day.
    DateTime {
        /// Seconds east of UTC.
        offset_s: i32,
    },
}

/// A result: a number, and the unit it is in (`None` for a bare number).
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    /// The number.
    pub number: Number,
    /// The unit, when there is one.
    pub unit: Option<Quantity>,
    /// How [`format`] writes the number.
    pub shown: Shown,
}

/// What an evaluation may read, built by the caller.
#[derive(Debug, Clone)]
pub struct Env {
    /// Now, in seconds since the epoch — for dates ("today + 30 days").
    pub now_unix_s: i64,
    /// The person's zone.
    pub zone: ronoctua_files::when::Zone,
    /// The zoneinfo root (`RONOCTUA_ZONEINFO`), where another zone is read
    /// from by name; `None` answers only the person's zone and UTC.
    pub zoneinfo: Option<PathBuf>,
    /// The exchange rates, when the caller has them (cached or fetched).
    pub rates: Option<rates::Rates>,
}

impl Env {
    /// An environment with a time and a zone, and nothing else.
    #[must_use]
    pub const fn at(now_unix_s: i64, zone: ronoctua_files::when::Zone) -> Self {
        Self {
            now_unix_s,
            zone,
            zoneinfo: None,
            rates: None,
        }
    }
}

/// Why an expression has no value, as a closed set (`calc-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Why {
    /// There is nothing to calculate.
    Empty,
    /// A character or a word that does not belong where it is.
    Unexpected,
    /// A parenthesis opened and never closed (or closed and never opened).
    Unbalanced,
    /// A word that is neither a unit, a currency, a zone nor a function.
    UnknownWord,
    /// Two quantities of different kinds (`km + kg`, `m in s`).
    Incompatible,
    /// A division by zero.
    DivideByZero,
    /// A result too large to hold (a power of a power).
    TooLarge,
    /// Outside a function's domain (the root of a negative number).
    Domain,
    /// A currency was asked for and there are no rates.
    NoRates,
    /// A currency the rates do not quote.
    UnknownCurrency,
    /// A zone the zoneinfo does not have.
    UnknownZone,
    /// A date that does not exist.
    BadDate,
    /// A Roman numeral that is not one.
    BadRoman,
    /// A digit that its base does not have (`0b102`).
    BadDigit,
}

impl Why {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 14] = [
        Self::Empty,
        Self::Unexpected,
        Self::Unbalanced,
        Self::UnknownWord,
        Self::Incompatible,
        Self::DivideByZero,
        Self::TooLarge,
        Self::Domain,
        Self::NoRates,
        Self::UnknownCurrency,
        Self::UnknownZone,
        Self::BadDate,
        Self::BadRoman,
        Self::BadDigit,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Unexpected => "unexpected",
            Self::Unbalanced => "unbalanced",
            Self::UnknownWord => "unknown-word",
            Self::Incompatible => "incompatible",
            Self::DivideByZero => "divide-by-zero",
            Self::TooLarge => "too-large",
            Self::Domain => "domain",
            Self::NoRates => "no-rates",
            Self::UnknownCurrency => "unknown-currency",
            Self::UnknownZone => "unknown-zone",
            Self::BadDate => "bad-date",
            Self::BadRoman => "bad-roman",
            Self::BadDigit => "bad-digit",
        }
    }
}

/// Why, and where: the byte offset into the text of what was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Refusal {
    /// The byte offset into the expression's text.
    pub at: usize,
    /// Why.
    pub why: Why,
}

/// What kind of question [`quick`] recognised — what the fast-access card
/// is titled with (`calc-shape-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shape {
    /// Plain arithmetic: `2+2`.
    Arithmetic,
    /// A unit conversion: `10 km in mi`.
    Conversion,
    /// A percentage: `20% of 150`.
    Percentage,
    /// A change of base: `0xff in dec`.
    Base,
    /// A Roman numeral, either way.
    Roman,
    /// Date arithmetic: `2026-10-01 + 30 days`.
    Date,
    /// A time in another zone.
    Zone,
    /// A currency conversion.
    Currency,
}

impl Shape {
    /// Every shape, for a test that walks the words.
    pub const ALL: [Self; 8] = [
        Self::Arithmetic,
        Self::Conversion,
        Self::Percentage,
        Self::Base,
        Self::Roman,
        Self::Date,
        Self::Zone,
        Self::Currency,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Arithmetic => "arithmetic",
            Self::Conversion => "conversion",
            Self::Percentage => "percentage",
            Self::Base => "base",
            Self::Roman => "roman",
            Self::Date => "date",
            Self::Zone => "zone",
            Self::Currency => "currency",
        }
    }
}

/// An answer for a search box.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// What it recognised.
    pub shape: Shape,
    /// The value.
    pub value: Value,
    /// The same value in other units of its kind (the detail pane's
    /// alternatives), most useful first.
    pub also: Vec<Value>,
}

/// The expression in `text`, with `mark` as the decimal separator (the other
/// of `.`/`,` then groups thousands). Words of every language are understood.
///
/// # Errors
///
/// A [`Refusal`] pointing into `text`.
pub fn parse(text: &str, mark: char) -> Result<Expr, Refusal> {
    if text.trim().is_empty() {
        return Err(Refusal {
            at: 0,
            why: Why::Empty,
        });
    }
    let tokens = lex::lex(text, mark)?;
    let root = parse::parse(&tokens, text)?;
    Ok(Expr {
        source: text.to_owned(),
        root,
    })
}

/// The value of `expr` in `env`.
///
/// # Errors
///
/// A [`Refusal`] pointing into the expression's text.
pub fn eval(expr: &Expr, env: &Env) -> Result<Value, Refusal> {
    eval::value(&expr.root, env)
}

/// `value` in the units `to` — the converter's live value.
///
/// # Errors
///
/// [`Why::Incompatible`] across kinds, [`Why::NoRates`] and
/// [`Why::UnknownCurrency`] for currencies, and the arithmetic's own.
pub fn convert(value: &Value, to: &Quantity, env: &Env) -> Result<Value, Why> {
    eval::convert_value(value, &to.terms, env)
}

/// An answer when `text` is a question this engine answers ("2+2", "10 km in
/// mi", "20% of 150", "0xff in dec"), and `None` when it is not one — a bare
/// number or a bare word is not a question. A search box asks this of every
/// keystroke, so it is cheap and never refuses.
#[must_use]
pub fn quick(text: &str, mark: char, env: &Env) -> Option<Answer> {
    let expr = parse(text, mark).ok()?;
    // A SHORT LONE ROMAN NUMERAL is a word as often as a number (`MIX`,
    // `DIV`, `CLI`, `CD`, `XL`): it is a question when asked (`XIV in dec`),
    // computed with (`XIV + 1`) or long enough that no word of capitals is one
    // (`MMXXVI`, `MCMXCIV`).
    if matches!(expr.root.kind, parse::NodeKind::Roman(_)) && text.trim().chars().count() < 4 {
        return None;
    }
    let shape = shape_of(&expr.root)?;
    let value = eval(&expr, env).ok()?;
    let also = eval::alternatives(&value, env);
    Some(Answer { shape, value, also })
}

fn shape_of(root: &parse::Node) -> Option<Shape> {
    use parse::NodeKind as K;
    match &root.kind {
        // NOT A QUESTION: what a search box holds while a person types a
        // word or a number — `42`, `km`, `USD`, `e`, a date on its own.
        K::Number(_, Radix::Decimal)
        | K::Unit(_)
        | K::Currency(_)
        | K::Constant(_)
        | K::Date(..)
        | K::Time(..) => return None,
        K::Number(..) => return Some(Shape::Base),
        K::Roman(_) => return Some(Shape::Roman),
        K::Convert(_, parse::Target::Radix(Radix::Roman)) => return Some(Shape::Roman),
        K::Convert(inner, parse::Target::Radix(_)) if matches!(inner.kind, K::Roman(_)) => {
            return Some(Shape::Roman);
        }
        K::Convert(_, parse::Target::Radix(_)) => return Some(Shape::Base),
        K::Convert(_, parse::Target::Zone(_)) => return Some(Shape::Zone),
        _ => {}
    }
    let mut found = (false, false, false, false, false);
    walk(root, &mut |n| match n.kind {
        K::Date(..) | K::Time(..) | K::Now | K::Today => found.0 = true,
        K::Currency(_) => found.1 = true,
        K::Percent(_) => found.2 = true,
        K::Unit(_) => found.3 = true,
        K::Roman(_) => found.4 = true,
        _ => {}
    });
    Some(match found {
        (true, ..) => Shape::Date,
        (_, true, ..) => Shape::Currency,
        (_, _, true, ..) => Shape::Percentage,
        (_, _, _, true, _) => Shape::Conversion,
        (_, _, _, _, true) => Shape::Roman,
        _ => Shape::Arithmetic,
    })
}

/// The number of `value` as [`Value::shown`] says, with `mark` as the decimal
/// separator and at most `digits` decimals (rounded half away from zero,
/// trailing zeros dropped).
///
/// The unit is not written: its name is the door's (`unit-<id>`, chosen by
/// the number). A base, a percentage and a date are notation, the same in
/// every language: `0xFF`, `15%`, `2026-10-31 14:30`.
#[must_use]
pub fn format(value: &Value, mark: char, digits: usize) -> String {
    let integer = || match &value.number {
        Number::Exact(r) if r.is_integer() => Some(r.to_integer()),
        _ => None,
    };
    match value.shown {
        Shown::Decimal => decimal(&value.number, mark, digits),
        Shown::Percent => {
            let hundred = Number::Exact(BigRational::from_integer(100.into()));
            let n = value
                .number
                .mul(&hundred)
                .unwrap_or(Number::Approx(f64::NAN));
            format!("{}%", decimal(&n, mark, digits))
        }
        Shown::Radix(radix) => match (integer(), radix) {
            (Some(n), Radix::Hexadecimal) => prefixed(&n, "0x", 16),
            (Some(n), Radix::Binary) => prefixed(&n, "0b", 2),
            (Some(n), Radix::Octal) => prefixed(&n, "0o", 8),
            (Some(n), Radix::Roman) => n
                .to_u32()
                .and_then(lex::roman_text)
                .unwrap_or_else(|| n.to_string()),
            _ => decimal(&value.number, mark, digits),
        },
        Shown::Date { offset_s } | Shown::DateTime { offset_s } => {
            let secs = integer().and_then(|n| n.to_i64()).unwrap_or(0);
            let (y, m, d, h, min) = eval::civil_at(secs, offset_s);
            if matches!(value.shown, Shown::Date { .. }) {
                format!("{y:04}-{m:02}-{d:02}")
            } else {
                format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}")
            }
        }
    }
}

fn prefixed(n: &BigInt, prefix: &str, radix: u32) -> String {
    let sign = if n.is_negative() { "-" } else { "" };
    format!(
        "{sign}{prefix}{}",
        n.abs().to_str_radix(radix).to_uppercase()
    )
}

fn decimal(n: &Number, mark: char, digits: usize) -> String {
    match n {
        Number::Exact(r) => exact_text(r, mark, digits),
        Number::Approx(f) => {
            let magnitude = f.abs();
            // SCIENTIFIC past what fixed digits can say: a light-year in
            // millimetres, a femtosecond in years.
            let text = if magnitude >= 1e15
                || (magnitude > 0.0 && magnitude < 10f64.powi(-i32::try_from(digits).unwrap_or(9)))
            {
                let s = format!("{f:.digits$e}");
                let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
                format!("{}e{exponent}", trimmed(mantissa))
            } else {
                trimmed(&format!("{f:.digits$}")).to_owned()
            };
            if text == "-0" {
                "0".to_owned()
            } else {
                text.replace('.', &mark.to_string())
            }
        }
    }
}

/// A rational as decimals: `1/3` with 4 digits is `0.3333`.
fn exact_text(r: &BigRational, mark: char, digits: usize) -> String {
    let scale = num_traits::pow(BigInt::from(10u8), digits);
    let magnitude = r.abs() * BigRational::from_integer(scale.clone());
    // Half away from zero: floor(x + 1/2) on the magnitude.
    let half = BigRational::new(BigInt::from(1u8), BigInt::from(2u8));
    let scaled = (magnitude + half).floor().to_integer();
    let whole = &scaled / &scale;
    let part = &scaled % &scale;
    let sign = if r.is_negative() && !scaled.is_zero() {
        "-"
    } else {
        ""
    };
    if digits == 0 || part.is_zero() {
        return format!("{sign}{whole}");
    }
    let decimals = format!("{part:0>digits$}");
    format!("{sign}{whole}{mark}{}", decimals.trim_end_matches('0'))
}

/// `1.2500` → `1.25`, `2.000` → `2`.
fn trimmed(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(n: i64, d: i64) -> Value {
        Value {
            number: Number::Exact(BigRational::new(BigInt::from(n), BigInt::from(d))),
            unit: None,
            shown: Shown::Decimal,
        }
    }

    #[test]
    fn numbers_are_written_with_the_mark_of_the_language() {
        assert_eq!(format(&exact(1, 3), ',', 4), "0,3333");
        assert_eq!(format(&exact(2, 3), '.', 2), "0.67");
        assert_eq!(format(&exact(-5, 2), '.', 6), "-2.5");
        assert_eq!(format(&exact(4, 2), ',', 6), "2");
        assert_eq!(format(&exact(-1, 1000), '.', 2), "0", "no negative zero");
        let approx = Value {
            number: Number::Approx(1.25),
            unit: None,
            shown: Shown::Decimal,
        };
        assert_eq!(format(&approx, ',', 4), "1,25");
        assert_eq!(parse("  ", '.').unwrap_err().why, Why::Empty);
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());
    }
}

#[cfg(test)]
mod engine {
    use super::*;
    use crate::num::ratio;
    use ronoctua_files::when::Zone;

    /// 2026-09-30 12:00 UTC.
    const NOON: i64 = 1_790_769_600;

    fn env() -> Env {
        let mut env = Env::at(NOON, Zone::utc());
        env.zoneinfo = std::env::var_os("RONOCTUA_ZONEINFO").map(PathBuf::from);
        env
    }

    fn answer(text: &str, mark: char) -> String {
        match parse(text, mark).and_then(|e| eval(&e, &env())) {
            Ok(v) => {
                let unit = v.unit.as_ref().map(|q| {
                    q.terms
                        .iter()
                        .map(|(b, p)| {
                            let name = match b {
                                Base::Unit(u) => units::table().unit(*u).unwrap().id.to_owned(),
                                Base::Currency(c) => c.code().to_owned(),
                            };
                            if *p == 1 { name } else { format!("{name}^{p}") }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                });
                let approx = if v.number.is_exact() { "" } else { "≈" };
                match unit {
                    Some(u) => format!("{approx}{} {u}", format(&v, mark, 6)),
                    None => format!("{approx}{}", format(&v, mark, 6)),
                }
            }
            Err(r) => format!("{}@{}", r.why.id(), r.at),
        }
    }

    fn en(text: &str) -> String {
        answer(text, '.')
    }

    #[test]
    fn arithmetic_is_exact() {
        assert_eq!(en("2+2"), "4");
        assert_eq!(en("0.1 + 0.2"), "0.3");
        assert_eq!(en("1/3 * 3"), "1");
        assert_eq!(en("2^10"), "1024");
        assert_eq!(en("2^-2"), "0.25");
        assert_eq!(en("-3^2"), "-9");
        assert_eq!(en("(1+2)*3"), "9");
        assert_eq!(en("2(3+4)"), "14");
        assert_eq!(en("10 − 4 × 2 ÷ 4"), "8");
        assert_eq!(en("5!"), "120");
        assert_eq!(en("2**3"), "8");
        assert_eq!(en("0^0"), "1", "the empty product");
        assert_eq!(en("0^3"), "0");
        assert_eq!(en("20% of 150"), "30");
        assert_eq!(en("-20% of 150"), "-30");
        assert_eq!(en("5 of 10"), "unexpected@2", "`of` takes a percentage");
        // pt-BR: "cinco de dez" is five out of ten, never fifty.
        assert_eq!(en("5 de 10"), "unexpected@2");
        assert_eq!(
            en("10 FPS"),
            "unknown-word@3",
            "three capitals are not a code"
        );
        assert_eq!(en("10 usd + 1 USD"), "11 USD", "a code in any case");
        assert_eq!(en("7/0"), "divide-by-zero@0");
        assert_eq!(en("9^9^9"), "too-large@0");
        assert_eq!(en("(1+2"), "unbalanced@0");
        assert_eq!(en("1 +"), "unexpected@3");
        assert_eq!(en("2 frobs"), "unknown-word@2");
        assert_eq!(en("   "), "empty@0");
    }

    #[test]
    fn the_mark_is_the_languages() {
        assert_eq!(answer("1,5 + 1", ','), "2,5");
        assert_eq!(answer("1.234,5 * 2", ','), "2469");
        assert_eq!(answer("1,234.5 * 2", '.'), "2469");
        assert_eq!(answer("1/8", ','), "0,125");
    }

    #[test]
    fn transcendentals_are_approximate_and_say_so() {
        assert_eq!(en("sqrt(16)"), "4");
        assert_eq!(en("sqrt 9/4"), "0.75", "a function takes what follows it");
        assert_eq!(en("sqrt(2)"), "≈1.414214");
        assert_eq!(en("log(1000)"), "3");
        assert_eq!(en("ln(1)"), "0");
        assert_eq!(en("sin(30°)"), "0.5");
        assert_eq!(en("cos(pi)"), "-1");
        assert_eq!(en("2 pi"), "≈6.283185");
        assert_eq!(en("sqrt(-1)"), "domain@0");
        assert_eq!(en("ln(0)"), "domain@0");
        assert_eq!(en("abs(-2.5)"), "2.5");
        assert_eq!(en("round(2.5)"), "3");
        assert_eq!(en("sqrt(16 m^2)"), "4 metre");
        // pt-BR: the Portuguese school name of the sine.
        assert_eq!(en("sen(90°)"), "1");
        // THE MULTIPLES OF π ARE EXACT, in any angle unit and through `pi`:
        // never 6.1e-17 where the answer is 0.
        assert_eq!(en("cos(90°)"), "0");
        assert_eq!(en("sin(180°)"), "0");
        assert_eq!(en("sin(pi)"), "0");
        assert_eq!(en("cos(2 pi)"), "1");
        assert_eq!(en("sin(pi/6)"), "0.5");
        assert_eq!(en("cos(60°)"), "0.5");
        assert_eq!(en("sin(-30°)"), "-0.5");
        assert_eq!(en("sin(270°)"), "-1");
        assert_eq!(en("sin(100 grad)"), "1");
        assert_eq!(en("cos(0.5 turn)"), "-1");
        assert_eq!(en("tan(45°)"), "1");
        assert_eq!(en("tan(135°)"), "-1");
        assert_eq!(en("tan(180°)"), "0");
        assert_eq!(en("tan(90°)"), "domain@0", "no tangent at a right angle");
        assert_eq!(en("tan(pi/2)"), "domain@0");
        assert_eq!(en("tan(-270°)"), "domain@0");
        assert_eq!(en("sin(1e9 * 180°)"), "0", "reduced exactly, not in floats");
        assert_eq!(en("sin(45°)"), "≈0.707107", "irrational stays approximate");
        assert_eq!(en("sin(1°)"), "≈0.017452");
        assert_eq!(en("sin(1)"), "≈0.841471", "a bare number is radians");
        assert_eq!(en("sin(1 rad)"), "≈0.841471");
        assert_eq!(
            en("sin(0.000000000001)"),
            "≈1e-12",
            "a small angle is not snapped to 0"
        );
    }

    #[test]
    fn units_convert_and_carry() {
        assert_eq!(en("10 km in mi"), "6.213712 mile");
        assert_eq!(en("1 mi in km"), "1.609344 kilometre");
        assert_eq!(en("1 ft in cm"), "30.48 centimetre");
        assert_eq!(en("10 km / 2 h"), "5 kilometre hour^-1");
        assert_eq!(en("10 km / 2 h in km/h"), "5 kilometre-per-hour");
        assert_eq!(en("100 km/h in mph"), "62.137119 mile-per-hour");
        assert_eq!(en("10 km / 5 m"), "2000");
        assert_eq!(en("1 km + 1 m"), "1.001 kilometre");
        assert_eq!(en("1/2 cup in mL"), "120 millilitre");
        assert_eq!(en("3 m * 2 m"), "6 metre^2");
        assert_eq!(en("6 m² in cm²"), "60000 square-centimetre");
        assert_eq!(en("1 GiB in MB"), "1073.741824 megabyte");
        assert_eq!(en("1 kWh in J"), "3600000 joule");
        assert_eq!(en("1 atm in psi"), "14.695949 pound-per-square-inch");
        assert_eq!(en("1 km + 1 kg"), "incompatible@0");
        assert_eq!(en("10 m in s"), "incompatible@0");
        assert_eq!(
            en("1 L/100km in m²"),
            "incompatible@0",
            "a fuel economy is not an area"
        );
        // pt-BR: a unit's words in Portuguese, accents or not.
        assert_eq!(en("2 colheres de sopa em mL"), "30 millilitre");
        assert_eq!(en("3 quilometros em metros"), "3000 metre");
    }

    #[test]
    fn temperatures_are_affine_and_fuel_is_reciprocal() {
        assert_eq!(en("20 °C in °F"), "68 fahrenheit");
        assert_eq!(en("-40 °F in °C"), "-40 celsius");
        assert_eq!(en("0 °C in K"), "273.15 kelvin");
        assert_eq!(
            en("20 °C + 5 K"),
            "25 celsius",
            "an addition adds a difference"
        );
        assert_eq!(en("10 L/100km in km/L"), "10 kilometre-per-litre");
        assert_eq!(en("30 mpg in L/100km"), "7.840486 litre-per-100-km");
        assert_eq!(en("0 km/L in L/100km"), "divide-by-zero@0");
    }

    #[test]
    fn angles_go_through_pi_only_when_they_must() {
        assert_eq!(en("90° in grad"), "100 gradian", "exact: π cancels");
        assert_eq!(en("1 turn in °"), "360 degree");
        assert_eq!(en("180° in rad"), "≈3.141593 radian");
    }

    #[test]
    fn percentages_bases_and_romans() {
        assert_eq!(en("20% of 150"), "30");
        // pt-BR: the Portuguese word for `of`.
        assert_eq!(en("20% de 150"), "30");
        assert_eq!(en("150 + 10%"), "165");
        assert_eq!(en("150 - 10%"), "135");
        assert_eq!(en("10% + 5%"), "15%");
        assert_eq!(en("0xff in dec"), "255");
        assert_eq!(en("255 in hex"), "0xFF");
        assert_eq!(en("10 in bin"), "0b1010");
        assert_eq!(en("0o17 + 1"), "16");
        assert_eq!(en("-255 in hex"), "-0xFF");
        assert_eq!(en("2026 in roman"), "MMXXVI");
        assert_eq!(en("XIV + 1"), "15");
        assert_eq!(en("0 in roman"), "bad-roman@0");
        assert_eq!(en("1.5 in hex"), "domain@0");
        assert_eq!(en("0b102"), "bad-digit@4");
        assert_eq!(en("IIII"), "bad-roman@0");
    }

    #[test]
    fn dates_and_zones() {
        assert_eq!(en("2026-10-01 + 30 days"), "2026-10-31");
        assert_eq!(en("2026-01-31 + 1 month"), "2026-02-28", "a calendar month");
        assert_eq!(en("2026-12-25 - 2026-10-01"), "85 day");
        assert_eq!(en("today"), "2026-09-30");
        assert_eq!(en("now + 2 h"), "2026-09-30 14:00");
        assert_eq!(en("2026-02-30"), "bad-date@0");
        assert_eq!(en("now in UTC"), "2026-09-30 12:00");
        // THE REAL ZONE FILES, or this proves nothing: the dev shell and
        // `rust.tests` both set the variable.
        assert!(env().zoneinfo.is_some(), "RONOCTUA_ZONEINFO is not set");
        assert_eq!(en("now in Tokyo"), "2026-09-30 21:00");
        // pt-BR: a city typed with its accent.
        assert_eq!(en("now in São Paulo"), "2026-09-30 09:00");
        assert_eq!(en("now in America/New_York"), "2026-09-30 08:00");
        assert_eq!(en("14:30 in Tokyo"), "2026-09-30 23:30");
        assert_eq!(en("now in Atlantis"), "unknown-zone@0");
        assert_eq!(en("10 km in Tokyo"), "unknown-word@0");
        // TOO FAR IS REFUSED, never wrapped into a wrong date (or a panic).
        assert_eq!(en("today + 1e15 years"), "too-large@0");
        assert_eq!(en("2026-10-01 + 700000000000000000 years"), "too-large@0");
        assert_eq!(en("now - -9223372036854775808 s"), "too-large@0");
        assert_eq!(en("now + 9223372036854775807 s"), "too-large@0");
        assert_eq!(en("today + 2000000 years"), "too-large@0");
        assert_eq!(en("today + 1000 years"), "3026-09-30");
    }

    #[test]
    fn currencies_need_rates_only_to_change() {
        assert_eq!(en("10 USD + 5 USD"), "15 USD");
        assert_eq!(en("10 USD in BRL"), "no-rates@0");
        let expr = parse("10 USD in BRL", '.').unwrap();
        assert!(expr.wants_rates());
        assert!(!parse("10 USD + 5 USD", '.').unwrap().wants_rates());
        let mut env = env();
        env.rates = Some(rates::Rates {
            day: rates::Day {
                year: 2026,
                month: 9,
                day: 30,
            },
            per_euro: [
                (
                    rates::Currency::new("USD").unwrap(),
                    BigRational::new(12.into(), 10.into()),
                ),
                (
                    rates::Currency::new("BRL").unwrap(),
                    BigRational::new(6.into(), 1.into()),
                ),
            ]
            .into_iter()
            .collect(),
        });
        let v = eval(&expr, &env).unwrap();
        assert_eq!(format(&v, '.', 2), "50");
        let v = eval(&parse("$12 in €", '.').unwrap(), &env).unwrap();
        assert_eq!(format(&v, '.', 2), "10");
        assert_eq!(
            eval(&parse("1 JPY in EUR", '.').unwrap(), &env)
                .unwrap_err()
                .why,
            Why::UnknownCurrency
        );
    }

    #[test]
    fn quick_answers_questions_and_ignores_the_rest() {
        let e = env();
        let shape = |t: &str| quick(t, '.', &e).map(|a| a.shape);
        assert_eq!(shape("2+2"), Some(Shape::Arithmetic));
        assert_eq!(shape("10 km in mi"), Some(Shape::Conversion));
        assert_eq!(shape("20% of 150"), Some(Shape::Percentage));
        assert_eq!(shape("0xff in dec"), Some(Shape::Base));
        assert_eq!(shape("XIV in dec"), Some(Shape::Roman));
        assert_eq!(shape("XIV + 1"), Some(Shape::Roman));
        for word in ["XIV", "MIX", "DIV", "CLI", "CD", "DC", "XL", "CV"] {
            assert_eq!(shape(word), None, "`{word}` alone is a word being typed");
        }
        // `OF` IS A PERCENTAGE'S: `5 de 10` reads "five out of ten".
        assert_eq!(shape("5 de 10"), None);
        assert_eq!(shape("5 of 10"), None);
        assert_eq!(shape("20% de 150"), Some(Shape::Percentage));
        // ONLY THE CODES THE RATES QUOTE are currencies, in any case.
        assert_eq!(shape("10 FPS"), None);
        assert_eq!(en("4 CPU in USD"), "unknown-word@2", "and asks no rates");
        assert_eq!(shape("10 usd + 5 USD"), Some(Shape::Currency));
        assert_eq!(shape("2026-10-01 + 30 days"), Some(Shape::Date));
        assert_eq!(shape("now in UTC"), Some(Shape::Zone));
        assert_eq!(shape("10 USD + 5 USD"), Some(Shape::Currency));
        assert_eq!(shape("42"), None, "a number is not a question");
        assert_eq!(shape("km"), None, "nor is a unit");
        assert_eq!(shape("hello world"), None);
        assert_eq!(shape("e"), None, "a constant alone is a letter being typed");
        assert_eq!(shape("I"), None, "and so is a one-letter numeral");
        assert_eq!(shape("2026-10-01"), None);
        assert_eq!(shape("MMXXVI"), Some(Shape::Roman));
        assert_eq!(shape("now"), Some(Shape::Date));
        assert_eq!(shape("10 km in"), None, "half a question is not answered");
        let a = quick("10 km", '.', &e).unwrap();
        assert!(!a.also.is_empty() && a.also.len() <= 4);
        let mi = units::table().by_id("mile").unwrap();
        assert!(
            a.also.iter().any(|v| v.unit
                == Some(Quantity {
                    terms: vec![(Base::Unit(mi), 1)]
                })),
            "10 km offers miles"
        );
    }

    /// EVERY CLOSED SET THAT BECOMES WORDS HAS THEM, in every language: a
    /// refusal without a sentence would be drawn as its key.
    #[test]
    fn every_refusal_category_shape_and_unit_has_words() {
        let mut keys: Vec<String> = Why::ALL
            .iter()
            .map(|w| format!("calc-why-{}", w.id()))
            .collect();
        keys.extend(
            rates::Why::ALL
                .iter()
                .map(|w| format!("calc-rates-why-{}", w.id())),
        );
        keys.extend(Shape::ALL.iter().map(|s| format!("calc-shape-{}", s.id())));
        keys.extend(
            units::Category::ALL
                .iter()
                .map(|c| format!("calc-category-{}", c.id())),
        );
        keys.extend(
            units::table()
                .units
                .iter()
                .map(|u| format!("unit-{}", u.id)),
        );
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).unwrap();
            for k in &keys {
                assert!(texts.has(k), "{language} has no `{k}`");
            }
            let one = texts.t_args("unit-metre", &[("count", ronoctua_i18n::Arg::Number(1))]);
            let two = texts.t_args("unit-metre", &[("count", ronoctua_i18n::Arg::Number(2))]);
            assert_ne!(one, two, "{language}: the unit's name follows the number");
        }
    }

    /// EVERY DEFINITION OF `assets/units.nix` (`known`) HOLDS THROUGH THE
    /// ENGINE: `convert` goes through the factors, the offsets of an affine
    /// unit, the reciprocal of a fuel economy and the power of π of an angle,
    /// and lands on the defined value — exactly, or within a double's rounding
    /// where π is involved. A round trip alone could not see a wrong factor
    /// (it cancels); a definition can.
    #[test]
    fn conversions_agree_with_definitions() {
        let t = units::table();
        let e = env();
        let one = |id: &str| Quantity {
            terms: vec![(Base::Unit(t.by_id(id).unwrap()), 1)],
        };
        let mut kinds = (0, 0, 0);
        for k in &units::KNOWN {
            let start = Value {
                number: Number::Exact(ratio(k.value)),
                unit: Some(one(k.from)),
                shown: Shown::Decimal,
            };
            let got = convert(&start, &one(k.to), &e).unwrap().number;
            let (a, b) = (
                t.unit(t.by_id(k.from).unwrap()).unwrap(),
                t.unit(t.by_id(k.to).unwrap()).unwrap(),
            );
            kinds.0 += usize::from(a.is_affine() || b.is_affine());
            kinds.1 += usize::from(a.reciprocal || b.reciprocal);
            kinds.2 += usize::from(k.pi != 0);
            if k.pi == 0 {
                assert_eq!(got, Number::Exact(ratio(k.is)), "{k:?}");
            } else {
                let want = Number::Exact(ratio(k.is)).to_f64()
                    * std::f64::consts::PI.powi(i32::from(k.pi));
                assert!(
                    (got.to_f64() - want).abs() <= want.abs() * 1e-12,
                    "{k:?}: {got:?}"
                );
            }
        }
        assert!(
            units::KNOWN.len() >= 60,
            "{} definitions",
            units::KNOWN.len()
        );
        assert!(
            kinds.0 >= 3 && kinds.1 >= 3 && kinds.2 >= 2,
            "every kind of conversion: {kinds:?}"
        );
        for c in units::Category::ALL {
            assert!(
                units::KNOWN
                    .iter()
                    .any(|k| t.unit(t.by_id(k.from).unwrap()).unwrap().category == c),
                "{} has a definition",
                c.id()
            );
        }
    }

    /// EVERY UNIT CONVERTS TO EVERY OTHER OF ITS KIND AND BACK EXACTLY — the
    /// table's factors are rationals, so a round trip that is not the identity
    /// is a factor the generator lost.
    #[test]
    fn conversions_round_trip_exactly() {
        let t = units::table();
        let e = env();
        let x = Value {
            number: Number::Exact(BigRational::new(7.into(), 3.into())),
            unit: None,
            shown: Shown::Decimal,
        };
        let mut checked = 0;
        for a in 0..t.units.len() {
            for b in 0..t.units.len() {
                let (ua, ub) = (&t.units[a], &t.units[b]);
                if ua.dimension != ub.dimension
                    || (ua.category == units::Category::FuelEconomy)
                        != (ub.category == units::Category::FuelEconomy)
                {
                    continue;
                }
                let one = |n: usize| Quantity {
                    terms: vec![(Base::Unit(UnitId(u16::try_from(n).unwrap())), 1)],
                };
                let start = Value {
                    unit: Some(one(a)),
                    ..x.clone()
                };
                let there = convert(&start, &one(b), &e).unwrap();
                let back = convert(&there, &one(a), &e).unwrap();
                if ua.pi == ub.pi {
                    assert_eq!(
                        back.number, start.number,
                        "{} → {} → {}",
                        ua.id, ub.id, ua.id
                    );
                } else {
                    assert!(
                        (back.number.to_f64() - 7.0 / 3.0).abs() < 1e-9,
                        "{} ↔ {}",
                        ua.id,
                        ub.id
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 1000, "{checked} pairs");
        for c in units::Category::ALL {
            assert!(t.of(c).len() >= 2, "{} has something to convert", c.id());
        }
    }
}
