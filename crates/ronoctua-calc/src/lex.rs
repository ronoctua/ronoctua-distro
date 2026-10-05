//! Text into tokens.
//!
//! # Numbers are written in the person's language
//!
//! The decimal mark comes from the caller (`Texts::decimal_mark`), and the
//! OTHER of `.` and `,` groups thousands: with `,` as the mark, `1.234,5` is
//! one thousand two hundred and thirty-four and a half, and `1.5` is refused —
//! a group has exactly three digits, and guessing would be wrong half the time.
//!
//! # Words are the table's
//!
//! A unit, a currency, a function, `in`, `of`, `hex`, `now`: every word comes
//! from `assets/units.nix`, in every language at once (a person reading English
//! still types `metros`). The longest match wins, so `colher de sopa` is one
//! word and `min` is a minute, not a mile and an `n`. A symbol is matched as
//! written (`MB`, `°C`); a word without case or accents.

use std::collections::HashMap;
use std::sync::OnceLock;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Num as _, Zero as _};

use crate::rates::Currency;
use crate::units::{self, Means, Radix, UnitId, fold_char};
use crate::{Refusal, Why};

/// A token, and where it starts in the text (a byte offset).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub at: usize,
    pub end: usize,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A number, and the base it was written in.
    Number(BigRational, Radix),
    /// `+ - * / ^ ! % ( ) ² ³`, the Unicode signs folded into these.
    Op(char),
    Unit(UnitId),
    Currency(Currency),
    Means(Means),
    /// `2026-10-01`.
    Date(i64, u32, u32),
    /// `14:30`, `14:30:15`.
    Time(u32, u32, u32),
    /// A Roman numeral, already read.
    Roman(u32),
    /// A word the table does not know — a time zone's name, if it follows
    /// `in`; refused anywhere else.
    Word(String),
}

/// What a lexicon entry stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Meaning {
    Unit(UnitId),
    Currency(Currency),
    Means(Means),
}

/// One word or symbol of the table, as characters.
struct Entry {
    chars: Vec<char>,
    /// Matched as written (a symbol) rather than folded (a word).
    exact: bool,
    meaning: Meaning,
}

/// Every entry, by its first character (as written for a symbol, folded for
/// a word), the longest first.
struct Lexicon {
    by_first: HashMap<char, Vec<Entry>>,
}

fn lexicon() -> &'static Lexicon {
    static LEXICON: OnceLock<Lexicon> = OnceLock::new();
    LEXICON.get_or_init(|| {
        let mut by_first: HashMap<char, Vec<Entry>> = HashMap::new();
        let mut add = |word: &str, exact: bool, meaning: Meaning| {
            let chars: Vec<char> = word.chars().collect();
            if let Some(first) = chars.first() {
                by_first.entry(*first).or_default().push(Entry {
                    chars,
                    exact,
                    meaning,
                });
            }
        };
        for a in units::table().aliases {
            add(a.word, a.subtag == "*", Meaning::Unit(a.unit));
        }
        for (subtag, word, code) in &units::CURRENCY_WORDS {
            if let Some(c) = Currency::new(code) {
                add(word, *subtag == "*", Meaning::Currency(c));
            }
        }
        // THE CODES, as folded words: `USD`, `usd` and `Usd` alike. Only
        // these are currencies — any three capitals are not (`10 FPS`).
        for code in &units::CURRENCY_CODES {
            if let Some(c) = Currency::new(code) {
                add(&code.to_lowercase(), false, Meaning::Currency(c));
            }
        }
        for (_, word, means) in &units::KEYWORDS {
            add(word, false, Meaning::Means(*means));
        }
        for list in by_first.values_mut() {
            list.sort_by_key(|e| std::cmp::Reverse(e.chars.len()));
        }
        Lexicon { by_first }
    })
}

/// Whether a symbol containing `/` may be taken here: `m/s²` is metres per
/// second squared, not `(m/s)²`, so a following power leaves it in pieces.
fn slash_may_end(next: Option<char>) -> bool {
    !matches!(next, Some('^' | '²' | '³'))
}

/// The longest entry at `chars[i..]`.
fn longest(chars: &[char], folded: &[char], i: usize) -> Option<(usize, Meaning)> {
    let lex = lexicon();
    let mut best: Option<(usize, Meaning, bool)> = None;
    for key in [chars[i], folded[i]] {
        for e in lex.by_first.get(&key).into_iter().flatten() {
            let n = e.chars.len();
            if i + n > chars.len() || best.is_some_and(|(m, _, _)| m > n) {
                continue;
            }
            let source = if e.exact { chars } else { folded };
            if source[i..i + n] != e.chars[..] {
                continue;
            }
            let next = chars.get(i + n).copied();
            // A BOUNDARY: `m` does not match the start of `mile`.
            let last = e.chars[n - 1];
            if continues(last) && next.is_some_and(continues) {
                continue;
            }
            if e.chars.contains(&'/') && !slash_may_end(next) {
                continue;
            }
            // Longest wins; at equal length a symbol (as written) wins.
            if best.is_none_or(|(m, _, exact)| n > m || (n == m && e.exact && !exact)) {
                best = Some((n, e.meaning, e.exact));
            }
        }
    }
    best.map(|(n, m, _)| (n, m))
}

pub fn lex(text: &str, mark: char) -> Result<Vec<Token>, Refusal> {
    let chars: Vec<char> = text.chars().collect();
    let folded: Vec<char> = chars.iter().map(|c| fold_char(*c)).collect();
    // Byte offset of every character, and of the end.
    let mut offsets: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
    offsets.push(text.len());
    let refuse = |i: usize, why: Why| Refusal {
        at: offsets[i],
        why,
    };
    let group = if mark == ',' { '.' } else { ',' };

    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let starts_number =
            c.is_ascii_digit() || (c == mark && chars.get(i + 1).is_some_and(char::is_ascii_digit));
        let kind = if starts_number {
            let (kind, n) = number(&chars, i, mark, group).map_err(|(k, why)| refuse(k, why))?;
            i = n;
            kind
        } else if let Some((n, meaning)) = longest(&chars, &folded, i) {
            i += n;
            match meaning {
                Meaning::Unit(u) => Kind::Unit(u),
                Meaning::Currency(c) => Kind::Currency(c),
                Meaning::Means(m) => Kind::Means(m),
            }
        } else if let Some(op) = operator(c) {
            i += 1;
            // `**` is `^`.
            if op == '*' && chars.get(i) == Some(&'*') {
                i += 1;
                Kind::Op('^')
            } else {
                Kind::Op(op)
            }
        } else if c.is_alphabetic() {
            let mut n = i;
            while n < chars.len()
                && (continues(chars[n])
                    || chars[n] == '_'
                    || (chars[n] == '-' && chars.get(n + 1).is_some_and(|c| c.is_alphabetic())))
            {
                n += 1;
            }
            let word: String = chars[i..n].iter().collect();
            i = n;
            let roman = word.chars().all(|c| "IVXLCDM".contains(c));
            if roman && let Some(n) = roman_value(&word) {
                Kind::Roman(n)
            } else if roman {
                return Err(refuse(start, Why::BadRoman));
            } else {
                Kind::Word(word)
            }
        } else {
            return Err(refuse(i, Why::Unexpected));
        };
        out.push(Token {
            at: offsets[start],
            end: offsets[i],
            kind,
        });
    }
    Ok(out)
}

/// Whether a character continues a word: a letter or a digit — but not `²`,
/// which Unicode counts as a digit and a formula reads as a power.
fn continues(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_digit()
}

fn operator(c: char) -> Option<char> {
    Some(match c {
        '+' => '+',
        '-' | '−' | '–' => '-',
        '*' | '×' | '·' | '∙' | '⋅' => '*',
        '/' | '÷' | '∕' => '/',
        '^' => '^',
        '!' => '!',
        '%' => '%',
        '(' => '(',
        ')' => ')',
        '²' => '²',
        '³' => '³',
        _ => return None,
    })
}

type NumberError = (usize, Why);

/// A number literal at `i`: a date, a time, a base-prefixed integer, or a
/// decimal with groups, a mark and an exponent. Returns the token and where
/// it ended.
fn number(chars: &[char], i: usize, mark: char, group: char) -> Result<(Kind, usize), NumberError> {
    let digits_at =
        |k: usize, n: usize| (k..k + n).all(|j| chars.get(j).is_some_and(char::is_ascii_digit));
    let value_at = |k: usize, n: usize| -> u32 {
        chars[k..k + n]
            .iter()
            .fold(0, |v, c| v * 10 + c.to_digit(10).unwrap_or(0))
    };
    let not_digit = |k: usize| !chars.get(k).is_some_and(char::is_ascii_digit);

    // 2026-10-01
    if digits_at(i, 4)
        && chars.get(i + 4) == Some(&'-')
        && digits_at(i + 5, 2)
        && chars.get(i + 7) == Some(&'-')
        && digits_at(i + 8, 2)
        && not_digit(i + 10)
    {
        return Ok((
            Kind::Date(
                i64::from(value_at(i, 4)),
                value_at(i + 5, 2),
                value_at(i + 8, 2),
            ),
            i + 10,
        ));
    }
    // 14:30 and 14:30:15
    let hour_digits = if digits_at(i, 2) { 2 } else { 1 };
    if digits_at(i, hour_digits)
        && chars.get(i + hour_digits) == Some(&':')
        && digits_at(i + hour_digits + 1, 2)
    {
        let h = value_at(i, hour_digits);
        let m = value_at(i + hour_digits + 1, 2);
        let mut end = i + hour_digits + 3;
        let mut s = 0;
        if chars.get(end) == Some(&':') && digits_at(end + 1, 2) {
            s = value_at(end + 1, 2);
            end += 3;
        }
        if h > 23 || m > 59 || s > 59 || !not_digit(end) {
            return Err((i, Why::BadDate));
        }
        return Ok((Kind::Time(h, m, s), end));
    }
    // 0xFF, 0b1010, 0o17
    if chars[i] == '0'
        && let Some(radix) = chars.get(i + 1).and_then(|p| match p {
            'x' | 'X' => Some((16, Radix::Hexadecimal)),
            'b' | 'B' => Some((2, Radix::Binary)),
            'o' | 'O' => Some((8, Radix::Octal)),
            _ => None,
        })
    {
        let mut n = i + 2;
        let mut text = String::new();
        while n < chars.len() && (chars[n].is_ascii_alphanumeric() || chars[n] == '_') {
            if chars[n] != '_' {
                if !chars[n].is_digit(radix.0) {
                    return Err((n, Why::BadDigit));
                }
                text.push(chars[n]);
            }
            n += 1;
        }
        if text.is_empty() {
            return Err((i, Why::BadDigit));
        }
        let value = BigInt::from_str_radix(&text, radix.0).map_err(|_| (i, Why::BadDigit))?;
        return Ok((Kind::Number(BigRational::from_integer(value), radix.1), n));
    }

    // 1,234.5e3 (with `.` as the mark)
    let mut n = i;
    let mut digits = String::new();
    while n < chars.len() && chars[n].is_ascii_digit() {
        digits.push(chars[n]);
        n += 1;
    }
    while chars.get(n) == Some(&group)
        && digits_at(n + 1, 3)
        && not_digit(n + 4)
        && !digits.is_empty()
    {
        digits.extend(&chars[n + 1..n + 4]);
        n += 4;
    }
    if chars.get(n) == Some(&group) && chars.get(n + 1).is_some_and(char::is_ascii_digit) {
        // `1,5` with `.` as the mark: not a group, and not this language's mark.
        return Err((n, Why::Unexpected));
    }
    let mut decimals = String::new();
    if chars.get(n) == Some(&mark) && chars.get(n + 1).is_some_and(char::is_ascii_digit) {
        n += 1;
        while n < chars.len() && chars[n].is_ascii_digit() {
            decimals.push(chars[n]);
            n += 1;
        }
    }
    let mut exponent: i32 = 0;
    if matches!(chars.get(n), Some('e' | 'E')) {
        let (sign, skip) = match chars.get(n + 1) {
            Some('-') => (-1, 2),
            Some('+') => (1, 2),
            _ => (1, 1),
        };
        if chars.get(n + skip).is_some_and(char::is_ascii_digit) {
            let mut k = n + skip;
            let mut e: i32 = 0;
            while k < chars.len() && chars[k].is_ascii_digit() {
                e = e
                    .checked_mul(10)
                    .and_then(|e| e.checked_add(chars[k].to_digit(10).map_or(0, |d| d as i32)))
                    .ok_or((n, Why::TooLarge))?;
                k += 1;
            }
            if e > 10_000 {
                return Err((n, Why::TooLarge));
            }
            exponent = sign * e;
            n = k;
        }
    }
    let whole = format!("{digits}{decimals}");
    let numerator = if whole.is_empty() {
        BigInt::zero()
    } else {
        BigInt::parse_bytes(whole.as_bytes(), 10).ok_or((i, Why::Unexpected))?
    };
    let scale = i32::try_from(decimals.len()).map_err(|_| (i, Why::TooLarge))? - exponent;
    let ten = BigInt::from(10u8);
    let value = if scale >= 0 {
        BigRational::new(
            numerator,
            num_traits::pow(ten, scale.unsigned_abs() as usize),
        )
    } else {
        BigRational::from_integer(numerator * num_traits::pow(ten, scale.unsigned_abs() as usize))
    };
    Ok((Kind::Number(value, Radix::Decimal), n))
}

/// A Roman numeral's value, only in its canonical form (`IIII` is refused,
/// `IV` is four), 1 to 3999.
pub fn roman_value(text: &str) -> Option<u32> {
    let digit = |c: char| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let values: Vec<i64> = text.chars().map(digit).collect::<Option<_>>()?;
    let mut total = 0i64;
    for (k, v) in values.iter().enumerate() {
        if values.get(k + 1).is_some_and(|next| next > v) {
            total -= v;
        } else {
            total += v;
        }
    }
    let total = u32::try_from(total).ok()?;
    (roman_text(total).as_deref() == Some(text)).then_some(total)
}

/// A number as a Roman numeral, 1 to 3999.
pub fn roman_text(mut n: u32) -> Option<String> {
    if !(1..=3999).contains(&n) {
        return None;
    }
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (v, s) in TABLE {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str, mark: char) -> Vec<Kind> {
        lex(text, mark)
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    fn num(n: i64, d: i64) -> Kind {
        Kind::Number(
            BigRational::new(BigInt::from(n), BigInt::from(d)),
            Radix::Decimal,
        )
    }

    #[test]
    fn numbers_follow_the_mark_of_the_language() {
        assert_eq!(kinds("1,234.5", '.'), [num(12345, 10)]);
        assert_eq!(kinds("1.234,5", ','), [num(12345, 10)]);
        assert_eq!(kinds("0,5", ','), [num(1, 2)]);
        assert_eq!(kinds("2.5e3", '.'), [num(2500, 1)]);
        assert_eq!(lex("1,5", '.').unwrap_err().why, Why::Unexpected);
        assert_eq!(
            lex("0b102", '.').unwrap_err(),
            Refusal {
                at: 4,
                why: Why::BadDigit
            }
        );
        assert!(matches!(
            kinds("0xff", '.')[..],
            [Kind::Number(_, Radix::Hexadecimal)]
        ));
        assert_eq!(kinds("2026-10-01", '.'), [Kind::Date(2026, 10, 1)]);
        assert_eq!(kinds("9:05", '.'), [Kind::Time(9, 5, 0)]);
        assert_eq!(kinds("XIV", '.'), [Kind::Roman(14)]);
        assert_eq!(lex("IIII", '.').unwrap_err().why, Why::BadRoman);
    }

    #[test]
    fn the_longest_word_wins_and_symbols_keep_their_case() {
        let unit = |id: &str| Kind::Unit(units::table().by_id(id).unwrap());
        assert_eq!(kinds("10min", '.'), [num(10, 1), unit("minute")]);
        assert_eq!(kinds("MB mb", '.')[0], unit("megabyte"));
        assert_ne!(kinds("MB mbar", '.')[1], unit("megabyte"));
        // pt-BR: a unit's Portuguese words are part of the table.
        assert_eq!(kinds("2 colheres de sopa", '.')[1], unit("tablespoon"));
        assert_eq!(kinds("Quilômetros", '.'), [unit("kilometre")]);
        assert_eq!(kinds("m/s", '.'), [unit("metre-per-second")]);
        assert_eq!(
            kinds("m/s²", '.'),
            [unit("metre"), Kind::Op('/'), unit("second"), Kind::Op('²')]
        );
        assert_eq!(kinds("20 °C", '.')[1], unit("celsius"));
        assert_eq!(kinds("30°", '.')[1], unit("degree"));
        assert_eq!(kinds("mile", '.'), [unit("mile")]);
    }

    #[test]
    fn roman_numerals_go_both_ways() {
        for n in 1..=3999 {
            let text = roman_text(n).unwrap();
            assert_eq!(roman_value(&text), Some(n), "{text}");
        }
        assert_eq!(roman_text(0), None);
        assert_eq!(roman_value("MMXXVI"), Some(2026));
        assert_eq!(roman_value("VX"), None);
    }
}
