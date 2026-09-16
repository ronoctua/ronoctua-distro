//! Enough of the zip container for the formats that are one.
//!
//! # Why it is a crate and was a module of `ronoctua-inspect`
//!
//! Because a zip is not a file being inspected. It was written for 3MF and
//! AMF, which are zips of XML, and it moved once already — out of `mesh/`,
//! when the office formats turned out to be zips too. It moves again for the
//! same reason and one crate further: a REGION PACK is a zip
//! (`portugal-latest-free.shp.zip`), and the crate that bakes one
//! (`ronoctua-atlas-packs`) must not depend on the file manager's inspector to
//! walk a central directory. Copying the two hundred lines would be the second
//! walk through one, which is the arrangement this repository refuses
//! everywhere else.
//!
//! Written here rather than taken from a crate because the whole of what is
//! needed is "walk the directory, inflate one member" — see [`members`].
//!
//! # What it does NOT decide
//!
//! What a failure MEANS to whoever asked. It answers [`Trouble`], which has
//! two shapes and no sentence; `ronoctua_inspect::zip` turns those into a
//! `Refusal` the panel can draw, and the pack baker turns them into a
//! transfer's own trouble. A reader that named one caller's error type would
//! have made the second caller depend on the first.

use std::io::Read as _;

/// What a zip can be that this reader will not walk.
///
/// TWO SHAPES AND NO SENTENCE. Which words a person sees is the CALLER's, and
/// the two callers are not even in the same kind of program: the inspector
/// draws a refusal in a panel, the queue writes a line into a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// It is not a zip this reader can walk: no end-of-central-directory, a
    /// directory entry without its signature, an offset past the end, or a
    /// compression method that is neither stored nor deflated.
    Malformed,
    /// Past [`INFLATED_CEILING`], or ZIP64, whose sizes live in the extra
    /// field. The number is what was measured or, for a stored member, what
    /// the directory CLAIMS.
    TooBig(u64),
}

/// How many bytes a member may inflate to.
///
/// A zip is a thing somebody can build to inflate for ever out of a few
/// kilobytes. The compressed size cannot bound it — that is the whole trick —
/// so the bound is on what comes OUT, applied while inflating and not after.
/// Two hundred and fifty-six mebibytes is roughly this crate's mesh ceiling
/// expressed as XML, which is about ten times bulkier than the binary form,
/// and it is generous for a document: `sample.docx` is a megabyte.
pub const INFLATED_CEILING: u64 = 256 * 1024 * 1024;

/// One member of the zip, as the central directory describes it.
pub struct Member {
    name: String,
    method: u16,
    compressed: u64,
    plain: u64,
    at: u64,
}

impl Member {
    /// Its name inside the archive.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How many bytes it is once inflated, as the directory CLAIMS.
    ///
    /// A claim and not a measurement, which is why nothing bounds an inflate
    /// by it — see [`member_bytes`]. It is honest enough to list beside a name.
    #[must_use]
    pub const fn plain(&self) -> u64 {
        self.plain
    }
}

/// The members of a zip, found through its central directory.
///
/// Written here rather than taken from a crate because the whole of what is
/// needed is "walk the directory, inflate one member" — and a zip crate would
/// be a new entry in the lock for that, where `flate2` is already in the tree
/// under `png`. The workspace's rule is what decides: a dependency is welcome
/// when it is Rust and costs no store path, and it is not welcome merely
/// because it exists.
pub fn members(bytes: &[u8]) -> Result<Vec<Member>, Trouble> {
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
        return Err(Trouble::Malformed);
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
            return Err(Trouble::Malformed);
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
            return Err(Trouble::TooBig(u64::from(u32::MAX)));
        }
        let name_at = at + 46;
        if name_at + name_len > bytes.len() {
            return Err(Trouble::Malformed);
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
pub fn member_bytes(bytes: &[u8], m: &Member) -> Result<Vec<u8>, Trouble> {
    member_up_to(bytes, m, INFLATED_CEILING)
}

/// The same, with a ceiling the CALLER knows.
///
/// [`INFLATED_CEILING`] is right for a document and far too small for a
/// region's roads, which is a layer bigger than every document this system has
/// ever opened put together. The ceiling does not go away — a zip is a thing
/// somebody can build to inflate for ever out of a few kilobytes, and the
/// bound is still on what comes OUT — it is raised where what is being opened
/// is known. See `ronoctua_atlas_packs::bake::LAYER_CEILING`.
///
/// # Errors
///
/// What [`member_bytes`] answers.
pub fn member_up_to(bytes: &[u8], m: &Member, ceiling: u64) -> Result<Vec<u8>, Trouble> {
    let at = m.at as usize;
    if at + 30 > bytes.len() || !bytes[at..].starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
        return Err(Trouble::Malformed);
    }
    let name_len = u16::from_le_bytes([bytes[at + 26], bytes[at + 27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]) as usize;
    let from = at + 30 + name_len + extra_len;
    let to = from + m.compressed as usize;
    if to > bytes.len() {
        return Err(Trouble::Malformed);
    }
    let raw = &bytes[from..to];
    match m.method {
        0 => {
            if m.plain > ceiling {
                return Err(Trouble::TooBig(m.plain));
            }
            Ok(raw.to_vec())
        }
        8 => {
            let mut out = Vec::new();
            let took = flate2::read::DeflateDecoder::new(raw)
                .take(ceiling + 1)
                .read_to_end(&mut out)
                .map_err(|_| Trouble::Malformed)?;
            if took as u64 > ceiling {
                return Err(Trouble::TooBig(took as u64));
            }
            Ok(out)
        }
        _ => Err(Trouble::Malformed),
    }
}

/// A member by name, tolerating the leading slash the relationships use.
pub fn part<'a>(list: &'a [Member], name: &str) -> Option<&'a Member> {
    let want = name.trim_start_matches('/');
    list.iter().find(|m| m.name.trim_start_matches('/') == want)
}

#[cfg(test)]
mod tests {
    use super::{INFLATED_CEILING, Member, Trouble, member_bytes};

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
            Err(Trouble::TooBig(INFLATED_CEILING + 1))
        );
    }
}
