//! The installer on a USB stick — the one crate that knows the distro's
//! releases and writes a stick whole.
//!
//! # The release (the release-cache package's contract)
//!
//! GitHub Releases of `ronoctua/ronoctua-distro`, the latest one: the ISO as
//! one asset (`ronoctua-distro-<label>-x86_64.iso`) or as parts
//! (`….iso.part-NN`), and `SHA256SUMS` (always with the whole ISO's line).
//! The `release` branch is the commit the ISO was built from. Reading it is
//! [`release`]'s, pure; asking GitHub for it is `net`'s, behind the feature
//! of the same name and a consent [`Pass`] — the shell's, never root's.
//!
//! # Which drives
//!
//! [`drives::judge`] reads the disk border's rack and says, for every drive a
//! person would call a stick (USB or a memory card, or one udisks calls
//! removable), whether it may be written and, when not, WHY — a refusal is a
//! sentence on screen, never a drive that is missing. [`targets`] is the ones
//! that may.
//!
//! # The write
//!
//! The download is the queue's (`Work::Stick`: aria2 with consent passes,
//! each part and the whole verified, assembled). Writing needs root: the
//! person's password, asked once up front, opens the sudo door for
//! `RootVerb::StickWrite`, which starts [`WRITE_UNIT`] at once; it waits for
//! the staged `ready` ([`staging`]), re-validates the drive (by-id, serial,
//! size, removable, unmounted, not a disk the system runs from —
//! [`device`]), writes with `O_DIRECT` hashing what it writes, then reads
//! back and compares ([`write`]). Where it is, it says in [`PROGRESS_FILE`].

#![forbid(unsafe_code)]

use std::path::PathBuf;

#[cfg(feature = "net")]
use ronoctua_consent::Pass;
use serde::{Deserialize, Serialize};

pub mod device;
pub mod digest;
pub mod drives;
#[cfg(feature = "net")]
mod net;
pub mod release;
pub mod staging;
pub mod write;

pub use drives::{Judged, Unfit, judge, targets};
pub use staging::stage;

/// The repository whose releases carry the ISO.
pub const REPOSITORY: &str = "ronoctua/ronoctua-distro";

/// The root unit `RootVerb::StickWrite` starts.
pub const WRITE_UNIT: &str = "ronoctua-stick-write.service";

/// The folder the person stages the write in (`0700`, theirs).
pub const STAGING: &str = "/run/ronoctua-stick/staged";

/// Where the root unit says how far it is (`0644`, root's).
pub const PROGRESS_FILE: &str = "/run/ronoctua-stick/progress.json";

/// The one format of [`Request`] and [`Progress`].
pub const FORMAT: u32 = 1;

/// The host the latest release is asked of.
pub const API_HOST: &str = "api.github.com";

/// The host every asset's address names (`browser_download_url`).
pub const DOWNLOAD_HOST: &str = "github.com";

/// The host GitHub sends an asset's download on to. A redirect is the host's
/// answer and not the person's consent (`ronoctua_http::web`), so this one is
/// asked for by name beside the other two: the person lets all three in one
/// row, and the queue refuses to start without a pass for it.
pub const ASSET_HOST: &str = "release-assets.githubusercontent.com";

/// The three hosts, as the consent row names them.
pub const HOSTS: [&str; 3] = [API_HOST, DOWNLOAD_HOST, ASSET_HOST];

/// Why it did not happen, as a closed set (`stick-refused-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The host was not let in `consent.json`.
    NotLet,
    /// The network did not answer.
    Offline,
    /// The queue's download did not finish: GitHub stopped answering, or the
    /// cache ran out of room (the queue's row says which).
    Download,
    /// There is no release with an ISO.
    NoRelease,
    /// A part or the whole does not match `SHA256SUMS`.
    Mismatch,
    /// The drive is not one that may be written (internal, the system's,
    /// mounted, too small, not whole).
    NotWritable,
    /// The drive changed between the question and the write.
    Changed,
    /// What was read back is not what was written.
    ReadBackDiffers,
    /// A program or the disk failed.
    Failed,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 9] = [
        Self::NotLet,
        Self::Offline,
        Self::Download,
        Self::NoRelease,
        Self::Mismatch,
        Self::NotWritable,
        Self::Changed,
        Self::ReadBackDiffers,
        Self::Failed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotLet => "not-let",
            Self::Offline => "offline",
            Self::Download => "download",
            Self::NoRelease => "no-release",
            Self::Mismatch => "mismatch",
            Self::NotWritable => "not-writable",
            Self::Changed => "changed",
            Self::ReadBackDiffers => "read-back-differs",
            Self::Failed => "failed",
        }
    }

    /// The one of this name; `None` for any other word.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// A refusal for this reason.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())
    }
}

impl std::error::Error for Refusal {}

/// One downloadable asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Asset {
    /// Its file name.
    pub name: String,
    /// Where it is downloaded from.
    pub url: String,
    /// Its size, in bytes.
    pub size_bytes: u64,
}

/// The ISO, whole or in parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "as")]
pub enum Image {
    /// One asset.
    Whole {
        /// The asset.
        asset: Asset,
    },
    /// Parts, in order, joined into the ISO.
    Parts {
        /// The parts.
        parts: Vec<Asset>,
    },
}

impl Image {
    /// The assets to download, in order.
    #[must_use]
    pub fn assets(&self) -> Vec<&Asset> {
        match self {
            Self::Whole { asset } => vec![asset],
            Self::Parts { parts } => parts.iter().collect(),
        }
    }

    /// The whole image's size, in bytes: the asset's, or the parts' sum.
    #[must_use]
    pub fn size_bytes(&self) -> u64 {
        self.assets().iter().map(|a| a.size_bytes).sum()
    }
}

/// The latest release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Release {
    /// Its tag.
    pub tag: String,
    /// The ISO's label (`26.05.20261005`).
    pub label: String,
    /// When it was published (RFC 3339, GitHub's).
    pub published_at: String,
    /// The ISO.
    pub image: Image,
    /// `SHA256SUMS`.
    pub sums: Asset,
}

impl Release {
    /// The whole ISO's file name, as `SHA256SUMS` names it and as it lands in
    /// the cache.
    #[must_use]
    pub fn iso_name(&self) -> String {
        release::iso_name_of(&self.label)
    }
}

/// `SHA256SUMS`, read.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Sums {
    /// `(sha256 hex, file name)`, in the file's order.
    pub lines: Vec<(String, String)>,
}

/// A drive the installer may be written to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Target {
    /// The disk border's id for the drive.
    pub drive: String,
    /// The stable device path (`/dev/disk/by-id/usb-…`).
    pub by_id: PathBuf,
    /// Its serial, when it has one.
    pub serial: Option<String>,
    /// Its size, in bytes.
    pub size_bytes: u64,
    /// What to call it on screen (vendor and model), data.
    pub label: String,
}

impl Target {
    /// Whether an image of `bytes` fits on it. An image that fills the drive
    /// exactly fits; the root writer pads the last block with zeros, inside
    /// the drive's own last block.
    #[must_use]
    pub const fn fits(&self, bytes: u64) -> bool {
        bytes > 0 && write::padded(bytes) <= self.size_bytes
    }
}

/// What the person stages for the root unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Request {
    /// Always [`FORMAT`].
    pub format: u32,
    /// This write's own number, never `0`: the unit copies it into every
    /// [`Progress`] it writes about this request, and the queue's job that
    /// staged it reads no other. One `progress.json` serves every write the
    /// machine makes, and a job must never take another write's `Done` for
    /// its own.
    pub nonce: u64,
    /// The verified ISO.
    pub image: PathBuf,
    /// Its SHA-256 (hex), checked again by the root unit while it writes.
    pub sha256: String,
    /// The drive.
    pub target: Target,
}

/// Where the write is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Waiting for `ready`.
    Waiting,
    /// Writing.
    Writing,
    /// Reading back.
    Verifying,
    /// Written and verified.
    Done,
    /// It failed — or was given up, which says no [`Progress::why`].
    Failed,
}

impl Phase {
    /// Every one.
    pub const ALL: [Self; 5] = [
        Self::Waiting,
        Self::Writing,
        Self::Verifying,
        Self::Done,
        Self::Failed,
    ];

    /// Its kebab-case name (`stick-phase-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Writing => "writing",
            Self::Verifying => "verifying",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    /// Whether the unit is over.
    #[must_use]
    pub const fn is_over(self) -> bool {
        matches!(self, Self::Done | Self::Failed)
    }
}

/// What `progress.json` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Progress {
    /// Always [`FORMAT`].
    pub format: u32,
    /// The [`Request::nonce`] this is about; `None` while the unit waits for
    /// a request, or when it ended before reading one.
    #[serde(default)]
    pub nonce: Option<u64>,
    /// Where it is.
    pub phase: Phase,
    /// Bytes written (or read back) so far.
    pub done_bytes: u64,
    /// Of.
    pub total_bytes: u64,
    /// Why it failed, when it did. `None` with [`Phase::Failed`] is a write
    /// the person gave up (an empty `cancel`); the queue's own refusals reach
    /// it through `cancel` too ([`staging::cancel_for`]).
    pub why: Option<Why>,
}

/// The latest release (`api.github.com`, with its `Pass`).
///
/// # Errors
///
/// [`Why::NotLet`] when the pass is not for [`API_HOST`]; [`Why::Offline`]
/// when GitHub does not answer; [`Why::NoRelease`] when the latest release
/// carries no ISO or no `SHA256SUMS`; [`Why::Failed`] for an answer that is
/// not one.
#[cfg(feature = "net")]
pub fn latest(pass: &Pass) -> Result<Release, Refusal> {
    net::latest(pass)
}

/// A release's checksums, asked of [`DOWNLOAD_HOST`] with its `Pass`.
///
/// # Errors
///
/// [`Why::NotLet`] when the pass is not for the file's host;
/// [`Why::Offline`]; [`Why::Failed`] for a file that is not `SHA256SUMS`.
#[cfg(feature = "net")]
pub fn sums(pass: &Pass, release: &Release) -> Result<Sums, Refusal> {
    net::sums(pass, release)
}

/// The folder the images land in: `$XDG_CACHE_HOME/ronoctua[-dev]/stick`.
///
/// The cache and not the downloads: the image is a means, two to four
/// gigabytes that are only ever written to a stick; `~/.cache` is a
/// subvolume no snapshot holds, and the sweep (`assets/cache.nix`) takes it
/// when nobody has used it for its days. Only ONE image is kept: assembling a
/// newer one removes the older (`release::keep_only`).
#[must_use]
pub fn cache_dir_of(cache_root: &std::path::Path) -> PathBuf {
    cache_root.join("stick")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_crosses_whole() {
        let request = Request {
            format: FORMAT,
            nonce: 7,
            image: "/home/p/.cache/ronoctua/stick/ronoctua-distro.iso".into(),
            sha256: "0".repeat(64),
            target: Target {
                drive: "/org/freedesktop/UDisks2/drives/x".to_owned(),
                by_id: "/dev/disk/by-id/usb-x".into(),
                serial: Some("123".to_owned()),
                size_bytes: 32 << 30,
                label: "Stick".to_owned(),
            },
        };
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&text).unwrap(), request);
        // A field nobody declared is refused: the root unit reads this.
        let extra = text.replacen('{', "{\"shell\":\"sh\",", 1);
        assert!(serde_json::from_str::<Request>(&extra).is_err());
    }

    #[test]
    fn every_reason_and_phase_crosses_by_its_id() {
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
            let text = serde_json::to_string(&why).unwrap();
            assert_eq!(text, format!("\"{}\"", why.id()));
        }
        assert_eq!(Why::of_id("not-yet"), None);
        for phase in Phase::ALL {
            let text = serde_json::to_string(&phase).unwrap();
            assert_eq!(text, format!("\"{}\"", phase.id()));
        }
    }

    #[test]
    fn an_image_that_fits_is_one_that_fits_padded() {
        let t = Target {
            drive: String::new(),
            by_id: PathBuf::new(),
            serial: None,
            size_bytes: 8192,
            label: String::new(),
        };
        assert!(t.fits(8192));
        assert!(t.fits(4097));
        assert!(!t.fits(8193));
        assert!(!t.fits(0), "nothing to write is not a write");
    }
}
