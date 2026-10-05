//! Exchange rates: the European Central Bank's daily reference rates.
//!
//! # The one boundary with the ECB
//!
//! `https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml`, about
//! thirty currencies against the euro, published once a working day. Read
//! through `ronoctua-xml`'s depth guard, from the answer or from the cache
//! (`$XDG_CACHE_HOME/ronoctua/rates.xml`, stale after `ratesStaleHours` of
//! `assets/cache.nix`). Fetched only when a currency is typed and the cache
//! is stale, and only with a [`ronoctua_consent::Pass`] for [`HOST`] — the
//! first time, the door draws the consent row instead.

use std::collections::BTreeMap;

use crate::BigRational;

/// The host the rates come from.
pub const HOST: &str = "www.ecb.europa.eu";
/// The address of the daily file.
pub const ADDRESS: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// A currency, by its ISO 4217 code (`EUR`, `BRL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Currency([u8; 3]);

impl Currency {
    /// The euro, which every rate is against.
    pub const EUR: Self = Self(*b"EUR");

    /// The currency of a code: three ASCII letters, any case.
    #[must_use]
    pub fn new(code: &str) -> Option<Self> {
        let bytes: [u8; 3] = code.trim().as_bytes().try_into().ok()?;
        bytes
            .iter()
            .all(u8::is_ascii_alphabetic)
            .then(|| Self(bytes.map(|b| b.to_ascii_uppercase())))
    }

    /// Its code.
    #[must_use]
    pub fn code(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or_default()
    }
}

/// A day, as the ECB dates its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Day {
    /// The year.
    pub year: i32,
    /// The month, 1–12.
    pub month: u8,
    /// The day, 1–31.
    pub day: u8,
}

/// One day's rates: how many units of each currency one euro buys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rates {
    /// The day they were published for.
    pub day: Day,
    /// Units of each currency per euro, exact (the ECB writes decimals).
    /// The euro itself is not listed.
    pub per_euro: BTreeMap<Currency, BigRational>,
}

impl Rates {
    /// `amount` of `from` in `to`, exactly; `None` when either is not quoted.
    #[must_use]
    pub fn convert(
        &self,
        amount: &BigRational,
        from: Currency,
        to: Currency,
    ) -> Option<BigRational> {
        let one = || BigRational::from_integer(1.into());
        let rate = |c: Currency| {
            if c == Currency::EUR {
                Some(one())
            } else {
                self.per_euro.get(&c).cloned()
            }
        };
        let (from, to) = (rate(from)?, rate(to)?);
        if num_traits::Zero::is_zero(&from) {
            return None;
        }
        Some(amount / from * to)
    }
}

/// Why there are no rates, as a closed set (`calc-rates-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Why {
    /// The pass is for another host than [`HOST`].
    WrongPass,
    /// The host could not be reached.
    Unreachable,
    /// The host answered with a failure.
    Failed,
    /// The answer (or the cache) is not the ECB's file.
    Garbled,
}

impl Why {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 4] = [
        Self::WrongPass,
        Self::Unreachable,
        Self::Failed,
        Self::Garbled,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::WrongPass => "wrong-pass",
            Self::Unreachable => "unreachable",
            Self::Failed => "failed",
            Self::Garbled => "garbled",
        }
    }
}

/// The cache policy (`assets/cache.nix`), for `ratesStaleHours` — and the
/// calculator's history bound, which [`crate::HISTORY_ENTRIES`] reads.
pub(crate) mod policy {
    #![allow(dead_code, missing_docs)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// How old the cached rates may be before a currency typed asks for fresh
/// ones, in hours (`ratesStaleHours` in `assets/cache.nix`).
pub const STALE_HOURS: u32 = policy::RATES_STALE_HOURS;

/// The most an answer may weigh: the daily file is about two kilobytes.
#[cfg(feature = "net")]
const MOST_BYTES: u64 = 256 * 1024;

/// The rates in the ECB's XML (an answer, or the cached file).
///
/// # Errors
///
/// [`Why::Garbled`]: not XML (or deeper than the guard allows), no day, or
/// no rate, or a rate that is not a positive decimal.
pub fn parse(xml: &str) -> Result<Rates, Why> {
    let document = ronoctua_xml::parse(xml, ronoctua_xml::roxmltree::ParsingOptions::default())
        .map_err(|_| Why::Garbled)?;
    let cubes = || {
        document
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "Cube")
    };
    let day = cubes()
        .find_map(|n| n.attribute("time"))
        .and_then(day_of)
        .ok_or(Why::Garbled)?;
    let mut per_euro = BTreeMap::new();
    for n in cubes() {
        let (Some(code), Some(rate)) = (n.attribute("currency"), n.attribute("rate")) else {
            continue;
        };
        let currency = Currency::new(code).ok_or(Why::Garbled)?;
        let rate = decimal(rate).ok_or(Why::Garbled)?;
        per_euro.insert(currency, rate);
    }
    if per_euro.is_empty() {
        return Err(Why::Garbled);
    }
    Ok(Rates { day, per_euro })
}

/// `2026-09-30`.
fn day_of(text: &str) -> Option<Day> {
    let mut parts = text.trim().splitn(3, '-');
    let year = parts.next()?.parse().ok()?;
    let month = parts
        .next()?
        .parse()
        .ok()
        .filter(|m| (1..=12).contains(m))?;
    let day = parts
        .next()?
        .parse()
        .ok()
        .filter(|d| (1..=31).contains(d))?;
    Some(Day { year, month, day })
}

/// A positive decimal written with a full stop (`1.1234`), exactly.
fn decimal(text: &str) -> Option<BigRational> {
    let (whole, part) = text.trim().split_once('.').unwrap_or((text.trim(), ""));
    if whole.is_empty() && part.is_empty()
        || !whole
            .bytes()
            .chain(part.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digits = format!("{whole}{part}");
    let numerator = crate::BigInt::parse_bytes(digits.as_bytes(), 10)?;
    let denominator = num_traits::pow(crate::BigInt::from(10u8), part.len());
    let r = BigRational::new(numerator, denominator);
    num_traits::Signed::is_positive(&r).then_some(r)
}

/// Where the rates are kept: `$XDG_CACHE_HOME/ronoctua[-dev]/rates.xml`.
#[must_use]
pub fn cache_file() -> Option<std::path::PathBuf> {
    ronoctua_xdg::cache_dir().map(|d| d.join(CACHE_FILE))
}

/// The cache file's name.
pub const CACHE_FILE: &str = "rates.xml";

/// The rates on disk, and when they were written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cached {
    /// The rates.
    pub rates: Rates,
    /// When the file was written, in seconds since the epoch.
    pub written_unix_s: i64,
}

impl Cached {
    /// Whether a currency typed now should ask for fresh rates: the file is
    /// older than [`STALE_HOURS`] (or from the future, which a clock set back
    /// makes and nothing should trust).
    #[must_use]
    pub fn stale(&self, now_unix_s: i64) -> bool {
        let age = now_unix_s.saturating_sub(self.written_unix_s);
        age < 0 || age > i64::from(STALE_HOURS) * 3600
    }
}

/// The cached rates in `path`, when there are any that parse.
#[must_use]
pub fn cached_in(path: &std::path::Path) -> Option<Cached> {
    let text = std::fs::read_to_string(path).ok()?;
    let written = std::fs::metadata(path).ok()?.modified().ok()?;
    let written_unix_s = written
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())?;
    Some(Cached {
        rates: parse(&text).ok()?,
        written_unix_s,
    })
}

/// The cached rates, from [`cache_file`].
#[must_use]
pub fn cached() -> Option<Cached> {
    cached_in(&cache_file()?)
}

/// Today's rates, from [`ADDRESS`], written to [`cache_file`] when they parse.
/// Blocks: call it on a thread.
///
/// The pass is checked BEFORE anything leaves the machine, and it is the only
/// way here: there is no request in this crate that does not take one.
///
/// # Errors
///
/// [`Why::WrongPass`], [`Why::Unreachable`], [`Why::Failed`],
/// [`Why::Garbled`].
#[cfg(feature = "net")]
pub fn fetch(pass: &ronoctua_consent::Pass) -> Result<Rates, Why> {
    fetch_stoppable(pass, None)
}

/// [`fetch`], its socket shut when `abort` is — what a search that the next
/// key replaces asks with (fast-access's `/$`). A fetch stopped answers
/// [`Why::Failed`], which its caller drops.
///
/// # Errors
///
/// As [`fetch`].
#[cfg(feature = "net")]
pub fn fetch_stoppable(
    pass: &ronoctua_consent::Pass,
    abort: Option<&ronoctua_http::web::Abort>,
) -> Result<Rates, Why> {
    fetch_by(pass, cache_file().as_deref(), &mut |url| {
        // THROUGH THE PASS, as every request a person's consent governs: the
        // host is checked again where the bytes leave, and a redirect to
        // another host carries none of this one's consent.
        let mut ask = ronoctua_http::web::Ask::get(url, USER_AGENT).most(MOST_BYTES);
        if let Some(a) = abort {
            ask = ask.abortable(a);
        }
        match ronoctua_http::web::request_let(pass, &ask) {
            Ok(got) if got.reply.status == 200 => Ok(got.reply.body),
            Ok(_) => Err(Why::Failed),
            Err(e) if e.kind == ronoctua_http::web::Kind::Offline => Err(Why::Unreachable),
            Err(_) => Err(Why::Failed),
        }
    })
}

/// The user agent the request says.
#[cfg(feature = "net")]
const USER_AGENT: &str = "ronoctua-calc";

/// [`fetch`] over any transport: `get` is HTTP for the machine and a canned
/// answer for a test.
#[cfg(feature = "net")]
fn fetch_by(
    pass: &ronoctua_consent::Pass,
    cache: Option<&std::path::Path>,
    get: &mut dyn FnMut(&str) -> Result<Vec<u8>, Why>,
) -> Result<Rates, Why> {
    if !pass.covers(HOST) {
        return Err(Why::WrongPass);
    }
    let body = get(ADDRESS)?;
    let text = String::from_utf8(body).map_err(|_| Why::Garbled)?;
    let rates = parse(&text)?;
    if let Some(path) = cache
        && let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
        && let Err(e) = ronoctua_xdg::write_atomically(dir, name, text.as_bytes())
    {
        tracing::warn!(error = %e, "the exchange rates were fetched and could not be cached");
    }
    Ok(rates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BigInt;

    /// The ECB's daily file, trimmed to three currencies — its real shape.
    const DAILY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
	<gesmes:subject>Reference rates</gesmes:subject>
	<gesmes:Sender>
		<gesmes:name>European Central Bank</gesmes:name>
	</gesmes:Sender>
	<Cube>
		<Cube time='2026-09-30'>
			<Cube currency='USD' rate='1.1734'/>
			<Cube currency='JPY' rate='168.47'/>
			<Cube currency='BRL' rate='6.2458'/>
		</Cube>
	</Cube>
</gesmes:Envelope>"#;

    fn ratio(n: i64, d: i64) -> BigRational {
        BigRational::new(BigInt::from(n), BigInt::from(d))
    }

    #[test]
    fn a_conversion_goes_through_the_euro_exactly() {
        let brl = Currency::new("brl").unwrap();
        let usd = Currency::new("USD").unwrap();
        assert_eq!(brl.code(), "BRL");
        assert!(Currency::new("EURO").is_none());
        let rates = Rates {
            day: Day {
                year: 2026,
                month: 9,
                day: 30,
            },
            per_euro: [(brl, ratio(6, 1)), (usd, ratio(12, 10))]
                .into_iter()
                .collect(),
        };
        assert_eq!(
            rates.convert(&ratio(12, 1), usd, brl),
            Some(ratio(60, 1)),
            "12 USD is 10 EUR is 60 BRL"
        );
        assert_eq!(
            rates.convert(&ratio(1, 1), Currency::EUR, brl),
            Some(ratio(6, 1))
        );
        assert_eq!(
            rates.convert(&ratio(1, 1), Currency::new("XYZ").unwrap(), brl),
            None
        );
    }

    #[test]
    fn the_daily_file_is_read_exactly() {
        let rates = parse(DAILY).unwrap();
        assert_eq!(
            rates.day,
            Day {
                year: 2026,
                month: 9,
                day: 30
            }
        );
        assert_eq!(rates.per_euro.len(), 3);
        assert_eq!(
            rates.per_euro[&Currency::new("USD").unwrap()],
            ratio(11734, 10000)
        );
        assert_eq!(parse("<x/>"), Err(Why::Garbled));
        assert_eq!(parse("not xml"), Err(Why::Garbled));
        let deep = format!("{}{}", "<a>".repeat(200), "</a>".repeat(200));
        assert_eq!(parse(&deep), Err(Why::Garbled), "the depth guard");
        assert_eq!(decimal("1.5"), Some(ratio(3, 2)));
        assert_eq!(decimal("-1"), None);
        assert_eq!(decimal("0"), None);
    }

    #[test]
    fn cached_rates_go_stale() {
        let c = Cached {
            rates: parse(DAILY).unwrap(),
            written_unix_s: 1_000_000,
        };
        let hours = i64::from(STALE_HOURS);
        assert!(!c.stale(1_000_000 + hours * 3600));
        assert!(c.stale(1_000_000 + hours * 3600 + 1));
        assert!(c.stale(999_999), "a file from the future is not trusted");
    }

    #[cfg(feature = "net")]
    #[test]
    fn nothing_leaves_without_a_pass_for_the_ecb() {
        let dir = std::env::temp_dir().join(format!("ronoctua-calc-rates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = ronoctua_consent::store_at(Some(dir.join("config")));
        ronoctua_consent::let_hosts_in(&store, &["example.org", HOST], true).unwrap();
        let wrong = ronoctua_consent::pass_in(&store, "example.org").unwrap();
        let mut asked = Vec::new();
        let cache = dir.join("rates.xml");
        let answer = fetch_by(&wrong, Some(&cache), &mut |url| {
            asked.push(url.to_owned());
            Ok(DAILY.as_bytes().to_vec())
        });
        assert_eq!(answer, Err(Why::WrongPass));
        assert!(asked.is_empty(), "a pass for another host sent nothing");

        let right = ronoctua_consent::pass_in(&store, HOST).unwrap();
        let rates = fetch_by(&right, Some(&cache), &mut |url| {
            asked.push(url.to_owned());
            Ok(DAILY.as_bytes().to_vec())
        })
        .unwrap();
        assert_eq!(asked, [ADDRESS]);
        assert_eq!(rates.per_euro.len(), 3);
        assert_eq!(
            cached_in(&cache).map(|c| c.rates),
            Some(rates),
            "and it was cached"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
