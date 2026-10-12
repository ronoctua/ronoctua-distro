//! Pwned Passwords: how many times a password is in known breaches — by
//! k-anonymity, keyless.
//!
//! # What leaves the machine
//!
//! Five hexadecimal digits. The password's SHA-1 is computed HERE, the
//! password is dropped (its [`Secret`] zeroes itself) and so is the digest
//! once split; the question is `GET
//! https://api.pwnedpasswords.com/range/{first five}`, with `Add-Padding:
//! true` so the answer's SIZE does not tell which range was asked either.
//! The answer is every suffix in that range with its count; the suffix is
//! compared here, and the padding's lines (count 0) are nothing.

use crate::ask::{Net, Request, answered};
use crate::keys::Secret;
use crate::{Host, Refusal, Service, Why};

/// The service's origin.
pub const API: &str = "https://api.pwnedpasswords.com";

/// A range's answer is about 800 lines of 40 bytes; padded, up to 1 000.
pub const MOST_BYTES: u64 = 256 << 10;

/// A password's hash, split: what is sent, and what is kept.
#[derive(Clone, PartialEq, Eq)]
pub struct Split {
    /// The first five hex digits, upper case — all that is sent.
    pub prefix: String,
    /// The other thirty-five, upper case — compared here, never sent.
    suffix: String,
}

impl std::fmt::Debug for Split {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Split")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl Drop for Split {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.suffix.zeroize();
    }
}

/// The password's SHA-1, split — and the password dropped here, zeroed.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for an empty password.
pub fn split(password: Secret) -> Result<Split, Refusal> {
    use zeroize::Zeroize as _;
    if password.is_empty() {
        return Err(Refusal::new(Service::PwnedPasswords, Why::NothingToAsk));
    }
    let digest = aws_lc_rs::digest::digest(
        &aws_lc_rs::digest::SHA1_FOR_LEGACY_USE_ONLY,
        password.expose().as_bytes(),
    );
    drop(password);
    let mut hex = crate::virustotal::hex(digest.as_ref()).to_ascii_uppercase();
    let split = Split {
        prefix: hex[..5].to_owned(),
        suffix: hex[5..].to_owned(),
    };
    hex.zeroize();
    Ok(split)
}

/// The question: the prefix, with padding. Pure.
#[must_use]
pub fn request(split: &Split) -> Request {
    Request::get(
        Host::PwnedPasswords,
        format!("{API}/range/{}", split.prefix),
    )
    .header("Add-Padding", "true")
    .accepting("text/plain")
    .most(MOST_BYTES)
}

/// How many times the suffix is in the range's answer; `0` when it is not,
/// or only as padding. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for an answer that is not lines of `SUFFIX:COUNT`.
pub fn count_in(split: &Split, body: &[u8]) -> Result<u64, Refusal> {
    let malformed = || Refusal::new(Service::PwnedPasswords, Why::Malformed);
    let text = std::str::from_utf8(body).map_err(|_| malformed())?;
    let mut found = 0;
    let mut lines = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        lines += 1;
        let (suffix, count) = line.split_once(':').ok_or_else(malformed)?;
        let count: u64 = count.trim().parse().map_err(|_| malformed())?;
        if suffix.trim().eq_ignore_ascii_case(&split.suffix) {
            found = count;
        }
    }
    if lines == 0 {
        return Err(malformed());
    }
    Ok(found)
}

/// How many times this password is in known breaches. The password is
/// dropped before anything is sent. Blocks: a thread.
///
/// # Errors
///
/// [`Why::NothingToAsk`], [`Why::NotLet`], and the service's refusals.
pub fn check(net: &dyn Net, password: Secret) -> Result<u64, Refusal> {
    let split = split(password)?;
    let body = answered(net, &request(&split))?;
    count_in(&split, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::{Canned, header};

    /// `password`'s SHA-1: 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8.
    const PREFIX: &str = "5BAA6";
    const SUFFIX: &str = "1E4C9B93F3F0682250B6CF8331B7EE68FD8";

    /// FAILS IF: more than five digits leave, the padding is not asked for,
    /// or the answer's count stops being the suffix's.
    #[test]
    fn five_digits_leave_and_the_rest_is_compared_here() {
        let url = format!("{API}/range/{PREFIX}");
        let body = format!(
            "0018A45C4D1DEF81644B54AB7F969B88D65:10\r\n{SUFFIX}:9545824\r\n00D4F6E8FA6EECAD2A3AA415EEC418D38EC:0\r\n"
        );
        let net = Canned::new(&[(url.as_str(), 200, body.as_bytes())]);
        assert_eq!(check(&net, Secret::new("password")).unwrap(), 9_545_824);
        let asked = net.only();
        assert_eq!(asked.url, url);
        assert!(!asked.url.contains(SUFFIX));
        assert_eq!(
            asked.url.rsplit('/').next(),
            Some(PREFIX),
            "five digits, no more"
        );
        assert_eq!(header(&asked, "Add-Padding"), Some("true"));
        assert!(!format!("{:?}", split(Secret::new("password")).unwrap()).contains(SUFFIX));
    }

    /// FAILS IF: a password that is only padding, or absent, reads as found,
    /// or an answer that is not the range's reads as "not found".
    #[test]
    fn padding_and_absence_are_zero_and_nonsense_is_refused() {
        let s = split(Secret::new("password")).unwrap();
        assert_eq!(count_in(&s, format!("{SUFFIX}:0\n").as_bytes()).unwrap(), 0);
        assert_eq!(
            count_in(&s, b"0018A45C4D1DEF81644B54AB7F969B88D65:10\n").unwrap(),
            0
        );
        assert_eq!(count_in(&s, b"<html>").unwrap_err().why, Why::Malformed);
        assert_eq!(count_in(&s, b"").unwrap_err().why, Why::Malformed);
        let net = Canned::new(&[]);
        assert_eq!(
            check(&net, Secret::new("")).unwrap_err().why,
            Why::NothingToAsk
        );
        assert!(net.asked().is_empty());
    }
}
