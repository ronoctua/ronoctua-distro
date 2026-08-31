//! Enough of the zip container for the two formats that are one.
//!
//! 3MF is a zip of XML, and an AMF may be. Written here rather than taken from
//! a crate because the whole of what is needed is "walk the directory, inflate
//! one member" — see [`members`].

use std::io::Read as _;

use super::INFLATED_CEILING;
use crate::Refusal;

/// One member of the zip, as the central directory describes it.
pub struct Member {
    name: String,
    method: u16,
    compressed: u64,
    plain: u64,
    at: u64,
}

/// The members of a zip, found through its central directory.
///
/// Written here rather than taken from a crate because the whole of what is
/// needed is "walk the directory, inflate one member" — and a zip crate would
/// be a new entry in the lock for that, where `flate2` is already in the tree
/// under `png`. The workspace's rule is what decides: a dependency is welcome
/// when it is Rust and costs no store path, and it is not welcome merely
/// because it exists.
pub fn members(bytes: &[u8]) -> Result<Vec<Member>, Refusal> {
    // The end-of-central-directory record is last, after a comment of up to
    // 64 KiB, so it is found by scanning backwards for its signature.
    let tail = bytes.len().saturating_sub(66_000);
    let mut eocd = None;
    let mut i = bytes.len().saturating_sub(22);
    while i >= tail {
        if bytes[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]) {
            eocd = Some(i);
            break;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    let Some(eocd) = eocd else {
        return Err(Refusal::Failed);
    };
    let count = u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]) as usize;
    let mut at = u32::from_le_bytes([
        bytes[eocd + 16],
        bytes[eocd + 17],
        bytes[eocd + 18],
        bytes[eocd + 19],
    ]) as usize;

    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if at + 46 > bytes.len() || !bytes[at..].starts_with(&[0x50, 0x4b, 0x01, 0x02]) {
            return Err(Refusal::Failed);
        }
        let u16_at = |o: usize| u16::from_le_bytes([bytes[at + o], bytes[at + o + 1]]);
        let u32_at = |o: usize| {
            u32::from_le_bytes([
                bytes[at + o],
                bytes[at + o + 1],
                bytes[at + o + 2],
                bytes[at + o + 3],
            ])
        };
        let method = u16_at(10);
        let compressed = u32_at(20);
        let plain = u32_at(24);
        let name_len = u16_at(28) as usize;
        let extra_len = u16_at(30) as usize;
        let comment_len = u16_at(32) as usize;
        let local = u32_at(42);
        // ZIP64 puts these in the extra field. A 3MF that needs it is far past
        // this crate's ceilings anyway, so it is refused rather than
        // half-understood.
        if compressed == u32::MAX || plain == u32::MAX || local == u32::MAX {
            return Err(Refusal::TooBig(u64::from(u32::MAX)));
        }
        let name_at = at + 46;
        if name_at + name_len > bytes.len() {
            return Err(Refusal::Failed);
        }
        let name = String::from_utf8_lossy(&bytes[name_at..name_at + name_len]).into_owned();
        out.push(Member {
            name,
            method,
            compressed: u64::from(compressed),
            plain: u64::from(plain),
            at: u64::from(local),
        });
        at = name_at + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// One member's bytes, inflated with a ceiling ON THE WAY OUT.
///
/// The bound is on what comes out and is applied while inflating, because that
/// is the only bound a zip bomb cannot lie about: the compressed size is a few
/// kilobytes by construction.
pub fn member_bytes(bytes: &[u8], m: &Member) -> Result<Vec<u8>, Refusal> {
    let at = m.at as usize;
    if at + 30 > bytes.len() || !bytes[at..].starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
        return Err(Refusal::Failed);
    }
    let name_len = u16::from_le_bytes([bytes[at + 26], bytes[at + 27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]) as usize;
    let from = at + 30 + name_len + extra_len;
    let to = from + m.compressed as usize;
    if to > bytes.len() {
        return Err(Refusal::Failed);
    }
    let raw = &bytes[from..to];
    match m.method {
        0 => {
            if m.plain > INFLATED_CEILING {
                return Err(Refusal::TooBig(m.plain));
            }
            Ok(raw.to_vec())
        }
        8 => {
            let mut out = Vec::new();
            let took = flate2::read::DeflateDecoder::new(raw)
                .take(INFLATED_CEILING + 1)
                .read_to_end(&mut out)
                .map_err(|_| Refusal::Failed)?;
            if took as u64 > INFLATED_CEILING {
                return Err(Refusal::TooBig(took as u64));
            }
            Ok(out)
        }
        _ => Err(Refusal::Failed),
    }
}

/// A member by name, tolerating the leading slash the relationships use.
pub fn part<'a>(list: &'a [Member], name: &str) -> Option<&'a Member> {
    let want = name.trim_start_matches('/');
    list.iter().find(|m| m.name.trim_start_matches('/') == want)
}

#[cfg(test)]
mod tests {
    use super::{Member, member_bytes};
    use crate::Refusal;
    use crate::mesh::INFLATED_CEILING;

    #[test]
    // THE DEFECT THIS CATCHES: bounding the inflation by the COMPRESSED size,
    // which is the one number a zip bomb is built to make small.
    fn a_member_that_claims_to_inflate_past_the_ceiling_is_refused() {
        let m = Member {
            name: "big".to_owned(),
            method: 0,
            compressed: 4,
            plain: INFLATED_CEILING + 1,
            at: 0,
        };
        let mut fake = vec![0x50, 0x4b, 0x03, 0x04];
        fake.extend_from_slice(&[0_u8; 26]);
        fake.extend_from_slice(b"data");
        assert_eq!(
            member_bytes(&fake, &m),
            Err(Refusal::TooBig(INFLATED_CEILING + 1))
        );
    }
}
