//! What a release IS, read out of GitHub's answer and out of `SHA256SUMS` —
//! pure, so the fixtures are the tests and the network is somebody else's.
//!
//! # The shape this reads (the release-cache package writes it)
//!
//! - the ISO whole, `ronoctua-distro-<label>-x86_64.iso`, or in parts,
//!   `ronoctua-distro-<label>-x86_64.iso.part-NN` (GitHub refuses an asset of
//!   2 GiB or more, so a bigger ISO travels cut), numbered from `00` or `01`
//!   with no gap;
//! - `SHA256SUMS`, one line per part AND one for the whole: a part is
//!   checked as it lands and the assembled ISO again, so a part that is
//!   right on its own but joined in the wrong order is still caught.
//!
//! Anything else is refused as [`Why::NoRelease`] — a release this does not
//! understand is not one it guesses at.

use std::path::Path;

use crate::{Asset, DOWNLOAD_HOST, Image, REPOSITORY, Refusal, Release, Sums, Why};

/// The file name of the whole ISO of `label`.
#[must_use]
pub fn iso_name_of(label: &str) -> String {
    format!("ronoctua-distro-{label}-x86_64.iso")
}

/// The name of the marker that says the image beside it was verified: the
/// ISO's name and `.sha256`, holding the whole's digest.
#[must_use]
pub fn marker_name_of(iso: &str) -> String {
    format!("{iso}.sha256")
}

/// The name of the folder an image is downloaded and assembled in, beside
/// where it lands: removing it removes every half of a download.
#[must_use]
pub fn scratch_name_of(iso: &str) -> String {
    format!("{iso}.download")
}

/// The most bytes of GitHub's answer this reads: a release's notes and its
/// assets' list, with room to spare.
pub const ANSWER_MOST_BYTES: u64 = 1 << 20;

/// The most bytes of `SHA256SUMS`.
pub const SUMS_MOST_BYTES: u64 = 64 << 10;

/// A label as the ISO's name carries it: letters, digits, `.`, `_`, `+` and
/// `-`, never a separator.
fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 64
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
}

/// What an asset's name says it is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Named {
    Whole(String),
    Part(String, u32),
    Sums,
    Other,
}

fn named(name: &str) -> Named {
    if name == "SHA256SUMS" {
        return Named::Sums;
    }
    let Some(rest) = name.strip_prefix("ronoctua-distro-") else {
        return Named::Other;
    };
    if let Some(label) = rest.strip_suffix("-x86_64.iso") {
        return if is_label(label) {
            Named::Whole(label.to_owned())
        } else {
            Named::Other
        };
    }
    if let Some((head, number)) = rest.rsplit_once(".iso.part-")
        && let Some(label) = head.strip_suffix("-x86_64")
        && is_label(label)
        && (2..=3).contains(&number.len())
        && number.bytes().all(|b| b.is_ascii_digit())
        && let Ok(n) = number.parse()
    {
        return Named::Part(label.to_owned(), n);
    }
    Named::Other
}

/// Whether an asset's address is one of the repository's own downloads:
/// `https://github.com/<REPOSITORY>/releases/download/…`. The queue asks it
/// on a pass for [`DOWNLOAD_HOST`], so an address anywhere else is refused
/// here, before anyone holds it.
#[must_use]
pub fn is_download(url: &str) -> bool {
    url.strip_prefix("https://")
        .and_then(|rest| rest.strip_prefix(DOWNLOAD_HOST))
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.strip_prefix(REPOSITORY))
        .is_some_and(|rest| {
            rest.starts_with("/releases/download/")
                && !rest.contains("/../")
                && !rest.contains(['?', '#', '\\', ' '])
        })
}

/// The host of an address (`https://host/…`), as a consent pass is asked
/// for it.
#[must_use]
pub fn host_of(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty() && !host.contains(['@', ':'])).then_some(host)
}

#[derive(serde::Deserialize)]
struct Answer {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<AnswerAsset>,
}

#[derive(serde::Deserialize)]
struct AnswerAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    #[serde(default)]
    state: Option<String>,
}

/// The release GitHub's `releases/latest` answered.
///
/// # Errors
///
/// [`Why::Failed`] for an answer that is not one; [`Why::NoRelease`] for a
/// draft, a pre-release, or a release without exactly one ISO (whole, or
/// parts with no gap) and its `SHA256SUMS`.
pub fn parse_latest(answer: &[u8]) -> Result<Release, Refusal> {
    let answer: Answer = serde_json::from_slice(answer).map_err(|_| Refusal::of(Why::Failed))?;
    if answer.draft || answer.prerelease {
        return Err(Refusal::of(Why::NoRelease));
    }
    let mut wholes: Vec<(String, Asset)> = Vec::new();
    let mut parts: Vec<(String, u32, Asset)> = Vec::new();
    let mut sums: Option<Asset> = None;
    for a in answer.assets {
        // AN ASSET STILL UPLOADING is not one yet: its bytes are not all
        // there, and a download of it would be checked against a line that
        // describes the whole.
        if a.state.as_deref().is_some_and(|s| s != "uploaded") {
            continue;
        }
        if !is_download(&a.browser_download_url) {
            continue;
        }
        let asset = Asset {
            name: a.name.clone(),
            url: a.browser_download_url,
            size_bytes: a.size,
        };
        match named(&a.name) {
            Named::Whole(label) if a.size > 0 => wholes.push((label, asset)),
            Named::Part(label, n) if a.size > 0 => parts.push((label, n, asset)),
            Named::Sums => sums = Some(asset),
            _ => {}
        }
    }
    let sums = sums.ok_or(Refusal::of(Why::NoRelease))?;
    let (label, image) = match wholes.as_slice() {
        [(label, asset)] => (
            label.clone(),
            Image::Whole {
                asset: asset.clone(),
            },
        ),
        [] => parts_of(parts)?,
        _ => return Err(Refusal::of(Why::NoRelease)),
    };
    Ok(Release {
        tag: answer.tag_name,
        label,
        published_at: answer.published_at.unwrap_or_default(),
        image,
        sums,
    })
}

/// The parts, as one image: one label, numbered from 0 or 1 with no gap.
fn parts_of(mut parts: Vec<(String, u32, Asset)>) -> Result<(String, Image), Refusal> {
    let label = parts
        .first()
        .map(|(l, _, _)| l.clone())
        .ok_or(Refusal::of(Why::NoRelease))?;
    if parts.iter().any(|(l, _, _)| *l != label) {
        return Err(Refusal::of(Why::NoRelease));
    }
    parts.sort_by_key(|(_, n, _)| *n);
    let first = parts.first().map_or(0, |(_, n, _)| *n);
    let numbered = parts
        .iter()
        .zip(first..)
        .all(|((_, n, _), wanted)| *n == wanted);
    if first > 1 || !numbered || parts.len() < 2 {
        return Err(Refusal::of(Why::NoRelease));
    }
    Ok((
        label,
        Image::Parts {
            parts: parts.into_iter().map(|(_, _, a)| a).collect(),
        },
    ))
}

impl Sums {
    /// `SHA256SUMS` as `sha256sum` writes it: `<64 hex> <space or *><name>`
    /// per line. A blank line is nothing; anything else that is not such a
    /// line refuses the whole file — a checksum file half understood is a
    /// file that might be checking something else.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`]: a line that is not one, a name with a `/` in it, or
    /// a name twice.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        let mut lines: Vec<(String, String)> = Vec::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let (sha, rest) = line.split_at_checked(64).ok_or(Refusal::of(Why::Failed))?;
            let name = rest
                .strip_prefix("  ")
                .or_else(|| rest.strip_prefix(" *"))
                .ok_or(Refusal::of(Why::Failed))?;
            if !crate::digest::is_sha256(sha)
                || name.is_empty()
                || name.contains(['/', '\0'])
                || lines.iter().any(|(_, n)| n == name)
            {
                return Err(Refusal::of(Why::Failed));
            }
            lines.push((sha.to_ascii_lowercase(), name.to_owned()));
        }
        Ok(Self { lines })
    }

    /// The digest of a file, by its name.
    #[must_use]
    pub fn of(&self, name: &str) -> Option<&str> {
        self.lines
            .iter()
            .find(|(_, n)| n == name)
            .map(|(sha, _)| sha.as_str())
    }

    /// Whether every asset of `release` and its whole ISO have a line.
    #[must_use]
    pub fn covers(&self, release: &Release) -> bool {
        self.of(&release.iso_name()).is_some()
            && release
                .image
                .assets()
                .iter()
                .all(|a| self.of(&a.name).is_some())
    }
}

/// The image of `release` in `dir`, when it is there whole and verified:
/// the ISO at its size and the marker holding a digest. Answers the digest.
#[must_use]
pub fn cached(dir: &Path, release: &Release) -> Option<String> {
    let iso = release.iso_name();
    let size = std::fs::symlink_metadata(dir.join(&iso)).ok()?;
    if !size.is_file() || size.len() != release.image.size_bytes() {
        return None;
    }
    let marker = std::fs::read_to_string(dir.join(marker_name_of(&iso))).ok()?;
    let sha = marker.trim();
    crate::digest::is_sha256(sha).then(|| sha.to_ascii_lowercase())
}

/// Removes from `dir` every image, marker and download that is not `iso`'s:
/// one image is kept, the newest asked for.
pub fn keep_only(dir: &Path, iso: &str) {
    let keep = [iso.to_owned(), marker_name_of(iso), scratch_name_of(iso)];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if keep.contains(&name) || !name.starts_with("ronoctua-distro-") {
            continue;
        }
        let path = entry.path();
        let gone = if entry.file_type().is_ok_and(|t| t.is_dir()) {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(e) = gone {
            eprintln!("could not remove {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAG: &str = "v26.05.20261005";

    fn asset(name: &str, size: u64) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "size": size,
            "state": "uploaded",
            "browser_download_url": format!(
                "https://github.com/ronoctua/ronoctua-distro/releases/download/{TAG}/{name}"
            ),
        })
    }

    fn answer(assets: &[serde_json::Value]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "tag_name": TAG,
            "name": "26.05.20261005",
            "published_at": "2026-10-05T12:00:00Z",
            "draft": false,
            "prerelease": false,
            "body": "notes",
            "assets": assets,
        }))
        .unwrap()
    }

    #[test]
    fn a_whole_iso_and_its_sums_are_a_release() {
        let r = parse_latest(&answer(&[
            asset("ronoctua-distro-26.05.20261005-x86_64.iso", 3_000_000_000),
            asset("SHA256SUMS", 200),
            asset("notes.txt", 10),
        ]))
        .unwrap();
        assert_eq!(r.label, "26.05.20261005");
        assert_eq!(r.tag, TAG);
        assert_eq!(r.published_at, "2026-10-05T12:00:00Z");
        assert_eq!(r.image.size_bytes(), 3_000_000_000);
        assert_eq!(r.iso_name(), "ronoctua-distro-26.05.20261005-x86_64.iso");
        assert_eq!(r.sums.name, "SHA256SUMS");
    }

    #[test]
    fn parts_are_joined_in_their_order_and_a_gap_is_no_release() {
        let p = |n: &str, s| asset(&format!("ronoctua-distro-1.0-x86_64.iso.part-{n}"), s);
        let r = parse_latest(&answer(&[
            p("01", 100),
            p("00", 2_000),
            p("02", 50),
            asset("SHA256SUMS", 200),
        ]))
        .unwrap();
        let Image::Parts { parts } = &r.image else {
            panic!("{r:?}")
        };
        let names: Vec<&str> = parts.iter().map(|a| a.name.as_str()).collect();
        assert!(names[0].ends_with("part-00") && names[2].ends_with("part-02"));
        assert_eq!(r.image.size_bytes(), 2_150);

        let gap = parse_latest(&answer(&[p("00", 1), p("02", 1), asset("SHA256SUMS", 2)]));
        assert_eq!(gap.unwrap_err().why, Why::NoRelease);
        let one = parse_latest(&answer(&[p("00", 1), asset("SHA256SUMS", 2)]));
        assert_eq!(
            one.unwrap_err().why,
            Why::NoRelease,
            "one part is not parts"
        );
    }

    #[test]
    fn what_is_not_a_release_is_said() {
        let no_sums = parse_latest(&answer(&[asset("ronoctua-distro-1-x86_64.iso", 5)]));
        assert_eq!(no_sums.unwrap_err().why, Why::NoRelease);
        let no_iso = parse_latest(&answer(&[asset("SHA256SUMS", 5)]));
        assert_eq!(no_iso.unwrap_err().why, Why::NoRelease);
        let two = parse_latest(&answer(&[
            asset("ronoctua-distro-1-x86_64.iso", 5),
            asset("ronoctua-distro-2-x86_64.iso", 5),
            asset("SHA256SUMS", 5),
        ]));
        assert_eq!(two.unwrap_err().why, Why::NoRelease, "which one?");
        assert_eq!(parse_latest(b"<html>").unwrap_err().why, Why::Failed);
        // AN ADDRESS ANYWHERE ELSE is not the repository's download.
        let mut elsewhere = asset("ronoctua-distro-1-x86_64.iso", 5);
        elsewhere["browser_download_url"] =
            serde_json::json!("https://example.com/ronoctua-distro-1-x86_64.iso");
        let r = parse_latest(&answer(&[elsewhere, asset("SHA256SUMS", 5)]));
        assert_eq!(r.unwrap_err().why, Why::NoRelease);
        // A HALF-UPLOADED ASSET is not one.
        let mut uploading = asset("ronoctua-distro-1-x86_64.iso", 5);
        uploading["state"] = serde_json::json!("starter");
        let r = parse_latest(&answer(&[uploading, asset("SHA256SUMS", 5)]));
        assert_eq!(r.unwrap_err().why, Why::NoRelease);
        // A LABEL WITH A SEPARATOR is not a label.
        assert_eq!(named("ronoctua-distro-a/b-x86_64.iso"), Named::Other);
    }

    #[test]
    fn sums_are_read_as_sha256sum_writes_them_and_nothing_else() {
        let a = "a".repeat(64);
        let b = "B".repeat(64);
        let s = Sums::parse(&format!("{a}  one.iso\n\n{b} *two.iso.part-00\r\n")).unwrap();
        assert_eq!(s.of("one.iso"), Some(a.as_str()));
        assert_eq!(s.of("two.iso.part-00"), Some("b".repeat(64).as_str()));
        assert_eq!(s.of("three"), None);
        for bad in [
            format!("{a} one.iso"),
            format!("{a}  dir/one.iso"),
            format!("{a}  one.iso\n{a}  one.iso"),
            "short  one.iso".to_owned(),
            format!("{}  one.iso", "z".repeat(64)),
        ] {
            assert_eq!(Sums::parse(&bad).unwrap_err().why, Why::Failed, "{bad:?}");
        }
    }

    #[test]
    fn a_download_address_is_the_repositorys_own() {
        assert!(is_download(
            "https://github.com/ronoctua/ronoctua-distro/releases/download/v1/x.iso"
        ));
        assert!(!is_download(
            "http://github.com/ronoctua/ronoctua-distro/releases/download/v1/x"
        ));
        assert!(!is_download(
            "https://github.com.evil/ronoctua/ronoctua-distro/releases/download/v1/x"
        ));
        assert!(!is_download(
            "https://github.com/other/ronoctua-distro/releases/download/v1/x"
        ));
        assert!(!is_download(
            "https://github.com/ronoctua/ronoctua-distro/releases/download/v1/x?y=1"
        ));
        assert_eq!(host_of("https://github.com/a"), Some("github.com"));
        assert_eq!(host_of("https://user@github.com/a"), None);
        assert_eq!(host_of("http://github.com/a"), None);
    }

    #[test]
    fn one_image_is_kept_and_a_verified_one_is_found() {
        let dir = std::env::temp_dir().join(format!("ronoctua-stick-keep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("ronoctua-distro-old-x86_64.iso.download")).unwrap();
        std::fs::write(dir.join("ronoctua-distro-old-x86_64.iso"), b"old").unwrap();
        std::fs::write(dir.join("ronoctua-distro-new-x86_64.iso"), b"new!").unwrap();
        std::fs::write(dir.join("unrelated"), b"x").unwrap();
        keep_only(&dir, "ronoctua-distro-new-x86_64.iso");
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["ronoctua-distro-new-x86_64.iso", "unrelated"]);

        let release = Release {
            tag: "v".to_owned(),
            label: "new".to_owned(),
            published_at: String::new(),
            image: Image::Whole {
                asset: Asset {
                    name: "ronoctua-distro-new-x86_64.iso".to_owned(),
                    url: String::new(),
                    size_bytes: 4,
                },
            },
            sums: Asset {
                name: "SHA256SUMS".to_owned(),
                url: String::new(),
                size_bytes: 1,
            },
        };
        assert_eq!(cached(&dir, &release), None, "no marker, not verified");
        std::fs::write(
            dir.join("ronoctua-distro-new-x86_64.iso.sha256"),
            format!("{}\n", "c".repeat(64)),
        )
        .unwrap();
        assert_eq!(cached(&dir, &release), Some("c".repeat(64)));
        std::fs::write(dir.join("ronoctua-distro-new-x86_64.iso"), b"new").unwrap();
        assert_eq!(
            cached(&dir, &release),
            None,
            "the wrong size is not the image"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
