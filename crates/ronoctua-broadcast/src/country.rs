//! One spelling of a country: ISO 3166-1 alpha-2, and only a code that exists.
//!
//! # Why a code is checked and not trusted
//!
//! The directories disagree. iptv-org writes the United Kingdom `UK` — 688
//! channels, and not one `GB` — where radio-browser, Natural Earth and ISO
//! write `GB`; a globe that took both would put British TV nowhere, because
//! the atlas has no `UK`. So every code that arrives passes through
//! [`normal`], which spells it one way and REFUSES a code that is not a
//! country: a guess (the nearest-looking code, the first letter) would put a
//! channel on the wrong country and nothing on screen could tell.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Every ISO 3166-1 alpha-2 code, and `XK` (Kosovo: user-assigned by ISO,
/// and what every one of the three directories and Natural Earth write).
/// SORTED, for the binary search.
const CODES: [&str; 250] = [
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX", "AZ",
    "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR", "BS",
    "BT", "BV", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM", "CN",
    "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM", "DO", "DZ", "EC", "EE",
    "EG", "EH", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR", "GA", "GB", "GD", "GE", "GF",
    "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS", "GT", "GU", "GW", "GY", "HK", "HM",
    "HN", "HR", "HT", "HU", "ID", "IE", "IL", "IM", "IN", "IO", "IQ", "IR", "IS", "IT", "JE", "JM",
    "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN", "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC",
    "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY", "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK",
    "ML", "MM", "MN", "MO", "MP", "MQ", "MR", "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA",
    "NC", "NE", "NF", "NG", "NI", "NL", "NO", "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG",
    "PH", "PK", "PL", "PM", "PN", "PR", "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW",
    "SA", "SB", "SC", "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS",
    "ST", "SV", "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO",
    "TR", "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI",
    "VN", "VU", "WF", "WS", "XK", "YE", "YT", "ZA", "ZM", "ZW",
];

/// A country code, spelled once: two capitals that name a country.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(pub [u8; 2]);

impl Code {
    /// The two letters.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Always two ASCII capitals: a `Code` is only ever made by [`normal`]
        // (or by hand, in a test, from a literal).
        std::str::from_utf8(&self.0).unwrap_or("??")
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A code that names no country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unknown(pub String);

/// `code`, spelled the one way: trimmed, in capitals, `UK` as `GB`. Pure.
///
/// # Errors
///
/// [`Unknown`] for anything that is not a country's code — never a guess.
pub fn normal(code: &str) -> Result<Code, Unknown> {
    let upper = code.trim().to_ascii_uppercase();
    let upper = if upper == "UK" {
        "GB".to_owned()
    } else {
        upper
    };
    match upper.as_bytes() {
        [a, b] if CODES.binary_search(&upper.as_str()).is_ok() => Ok(Code([*a, *b])),
        _ => Err(Unknown(code.to_owned())),
    }
}

impl Serialize for Code {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Code {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        normal(&text).map_err(|u| serde::de::Error::custom(format!("not a country code: {}", u.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_united_kingdom_is_gb_whatever_the_source_says() {
        assert_eq!(normal("UK"), Ok(Code(*b"GB")));
        assert_eq!(normal("uk"), Ok(Code(*b"GB")));
        assert_eq!(normal(" GB "), Ok(Code(*b"GB")));
        assert_eq!(normal("br").map(|c| c.to_string()).as_deref(), Ok("BR"));
        assert_eq!(normal("XK"), Ok(Code(*b"XK")));
    }

    #[test]
    fn an_unknown_code_is_refused_and_never_guessed() {
        for bad in ["", "ZZ", "U", "GBR", "EU", "??", "B1", "é"] {
            assert!(normal(bad).is_err(), "{bad:?} was taken for a country");
        }
    }

    #[test]
    fn the_list_is_sorted_and_whole() {
        assert!(
            CODES.windows(2).all(|w| w[0] < w[1]),
            "the binary search needs order"
        );
        assert!(
            CODES
                .iter()
                .all(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase()))
        );
        assert!(!CODES.contains(&"UK"));
    }

    #[test]
    fn a_code_crosses_json_as_its_two_letters() {
        assert_eq!(serde_json::to_string(&Code(*b"GB")).unwrap(), "\"GB\"");
        assert_eq!(
            serde_json::from_str::<Code>("\"UK\"").unwrap(),
            Code(*b"GB")
        );
        assert!(serde_json::from_str::<Code>("\"ZZ\"").is_err());
    }
}
