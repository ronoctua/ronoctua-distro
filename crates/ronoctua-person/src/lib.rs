//! The person's declared data, as the files say it.
//!
//! # Where it lives
//!
//! ```text
//! repository plain/person/          ⇄  $XDG_STATE_HOME/ronoctua/person/   (the person's processes)
//!   software/declared.json               the programs declared for this person
//!   software/declined.json               the distro's programs this person declined
//!   software/icons/<sha256>.png          content-addressed icons the entries name
//!   software/distro-pins.json            the distro's AppImages followed past its pins
//!   machines/<hostname>.json             each machine's record (its `state.json`)
//!   options.json                         opt-ins (signing commits by SSH, their binary cache)
//!
//! <machine dir>/person/              the root copy the root applier swaps in (data, never Nix)
//! <machine dir>/local/approved.json  person brain modules approved through the sudo door
//! <machine dir>/local/signer.json    the person's signing key root trusts, and the last verdict on it
//! $XDG_STATE_HOME/ronoctua/person-signed/  the manifest the person signed of the mirror, and its signature
//! /etc/ronoctua/person-applied.json  what Nix applied and skipped, and why
//! /etc/ronoctua/distro-apps.json     the distro's program registry on this machine
//! /etc/ronoctua/declared-android.json the Android apps to reconcile, each by its part and file
//! /var/lib/ronoctua-machine/state.json the boot trial, the hold, the last apply
//! ```
//!
//! The machine directory is never written here as a literal: it reaches a
//! program by `RONOCTUA_MACHINE_DIR` (one name, `machine-dir-has-one-name`),
//! so this crate names only what is BELOW it ([`names`]).
//!
//! # The contract every file keeps
//!
//! JSON, kebab-case, `"format": 1`, and an unknown field refused — a newer
//! machine's file is refused as [`Why::Newer`] before its fields are read, so
//! an older one never rewrites what it does not understand. Every list
//! carries an `id`, and two copies of a list meet through [`merge::merge_by_id`]
//! — the declines machine by machine ([`merge::merge_declined`]).
//!
//! # Pure
//!
//! Serde and nothing else. Reading, validating and merging are functions of
//! bytes; who opens the file (with `O_NOFOLLOW`, as root, under size caps) is
//! the caller's business.
//!
//! # And root's half, beside it
//!
//! The `ronoctua-person` binary (feature `apply`) is the one program that
//! writes the machine's copy: `apply` validates a staged tree and swaps it
//! into the machine directory ([`tree`]), and
//! `approve` records a brain module's approval at the hash root computed
//! ([`approve`]). What each decides is here, pure and tested; the binary
//! only opens, runs and writes. Its units and folders are named in [`root`].

#![forbid(unsafe_code)]

pub mod approve;
pub mod choice;
pub mod declared;
pub mod declined;
pub mod distro_pins;
pub mod frontier;
pub mod local;
pub mod merge;
pub mod options;
pub mod reports;
pub mod root;
pub mod state;
pub mod tree;

mod check;
#[cfg(test)]
mod golden;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The one format every file here is written in.
pub const FORMAT: u32 = 1;

/// The names of the files, relative to where each lives — never a machine's
/// absolute path.
pub mod names {
    /// Under the person's mirror and under the machine's `person/`.
    pub const DECLARED: &str = "software/declared.json";
    /// Under the person's mirror and under the machine's `person/`.
    pub const DECLINED: &str = "software/declined.json";
    /// The folder of content-addressed icons (`<sha256>.png`).
    pub const ICONS: &str = "software/icons";
    /// Under the person's mirror and under the machine's `person/`: the
    /// distro's AppImages followed past its pins ([`crate::distro_pins`]).
    pub const DISTRO_PINS: &str = "software/distro-pins.json";
    /// The folder of the machines' records (`<hostname>.json`).
    pub const MACHINES: &str = "machines";
    /// The opt-ins.
    pub const OPTIONS: &str = "options.json";
    /// The person's mirror, under `$XDG_STATE_HOME/ronoctua/`.
    pub const MIRROR: &str = "person";
    /// The root copy, under the machine directory.
    pub const MACHINE_PERSON: &str = "person";
    /// Under the machine directory, root only.
    pub const APPROVED: &str = "local/approved.json";
    /// Under the machine directory: what the update's verification of the
    /// person's entries wrote — the entries it held back, the followed
    /// AppImages it let in —, gone with it (an entry is a part, which fails
    /// alone). Every apply removes them.
    pub const RETIRED: [&str; 2] = ["local/held.json", "local/verified.json"];
    /// Under `/etc/ronoctua/`, written by Nix.
    pub const PERSON_APPLIED: &str = "person-applied.json";
    /// Under `/etc/ronoctua/`, written by Nix.
    pub const DISTRO_APPS: &str = "distro-apps.json";
    /// Under `/etc/ronoctua/`, written by Nix.
    pub const DECLARED_ANDROID: &str = "declared-android.json";
    /// Under `/var/lib/ronoctua-machine/`, written by `ronoctua-machine`.
    pub const STATE: &str = "state.json";
    /// Under the machine directory, root only: the person's signing key this
    /// machine trusts, and what the last apply made of the signature
    /// ([`crate::local::Signer`]).
    pub const SIGNER: &str = "local/signer.json";
    /// Under the machine directory: the installation's marker that what
    /// nobody may redistribute waits for the first network — written by the
    /// installer, taken away by `ronoctua-machine apply` once the machine
    /// builds without it.
    pub const DEFERRED_HARDWARE: &str = "hardware-deferred.json";
    /// The person's signed manifest of their mirror and its signature, under
    /// `$XDG_STATE_HOME/ronoctua/` — beside the mirror and never inside it,
    /// so it is not itself plain data: [`crate::tree::MANIFEST`] and
    /// [`crate::tree::SIGNATURE`], what a staging carries to root.
    pub const SIGNED: &str = "person-signed";
}

/// Why a file was refused, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The bytes are not JSON.
    NotJson,
    /// It says a format newer than this machine reads: update this machine.
    Newer,
    /// It is JSON of the wrong shape — a missing or unknown field, a value of
    /// the wrong type, or a format this machine never wrote.
    Shape,
    /// The shape is right and a value is not what it must be (an id that
    /// does not name its entry, a hostname with a space, a hash that is not
    /// SRI).
    Invalid,
    /// Two entries of one list carry the same id.
    Duplicate,
}

impl Why {
    /// Every one, in the order the keys would be written.
    pub const ALL: [Self; 5] = [
        Self::NotJson,
        Self::Newer,
        Self::Shape,
        Self::Invalid,
        Self::Duplicate,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotJson => "not-json",
            Self::Newer => "newer",
            Self::Shape => "shape",
            Self::Invalid => "invalid",
            Self::Duplicate => "duplicate",
        }
    }
}

/// A refusal: why, where (a JSON pointer, `""` for the whole file) and what
/// was found — English, for the journal; the screen says [`Why`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Why.
    pub why: Why,
    /// The JSON pointer of what was refused.
    pub at: String,
    /// What was found, for the journal.
    pub detail: String,
}

impl Refusal {
    /// A refusal for `why` at `at`.
    #[must_use]
    pub fn new(why: Why, at: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            why,
            at: at.into(),
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at '{}': {}", self.why.id(), self.at, self.detail)
    }
}

impl std::error::Error for Refusal {}

/// A moment, as the files write it: RFC 3339 in UTC to the second
/// (`2026-10-05T12:00:00Z`) and nothing else, so two stamps compare as text
/// and as time alike — which is what "the later edit wins" reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Stamp(String);

impl Stamp {
    /// The stamp `text` is, when it is exactly one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        check::stamp_seconds(text).map(|_| Self(text.to_owned()))
    }

    /// The stamp of `unix_s` seconds since the epoch.
    #[must_use]
    pub fn from_unix_s(unix_s: i64) -> Self {
        Self(check::stamp_of(unix_s))
    }

    /// Seconds since the epoch.
    #[must_use]
    pub fn unix_s(&self) -> i64 {
        check::stamp_seconds(&self.0).unwrap_or(0)
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Stamp {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| format!("not a UTC RFC 3339 stamp: {text:?}"))
    }
}

impl From<Stamp> for String {
    fn from(stamp: Stamp) -> Self {
        stamp.0
    }
}

impl std::fmt::Display for Stamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One of the files: its format and its own validation.
pub trait Document: Serialize + DeserializeOwned {
    /// The format it says it is.
    fn format(&self) -> u32;

    /// Every value checked, past the shape serde already checked.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] or [`Why::Duplicate`], with the pointer.
    fn validate(&self) -> Result<(), Refusal>;
}

/// Reads one file from its bytes: JSON, then the format (a newer one is
/// refused before its fields are read), then the shape, then the values.
///
/// # Errors
///
/// A [`Refusal`] saying which of the four failed.
pub fn parse<T: Document>(bytes: &[u8]) -> Result<T, Refusal> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| Refusal::new(Why::NotJson, "", e.to_string()))?;
    let format = value
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Refusal::new(Why::Shape, "/format", "no numeric format"))?;
    if format > u64::from(FORMAT) {
        return Err(Refusal::new(
            Why::Newer,
            "/format",
            format!("format {format}, this machine reads {FORMAT}"),
        ));
    }
    if format != u64::from(FORMAT) {
        return Err(Refusal::new(
            Why::Shape,
            "/format",
            format!("format {format} was never written"),
        ));
    }
    let doc: T =
        serde_json::from_value(value).map_err(|e| Refusal::new(Why::Shape, "", e.to_string()))?;
    doc.validate()?;
    Ok(doc)
}

/// The file's bytes as every writer writes them: pretty, two spaces, one
/// line break at the end — so the golden fixtures are byte-exact and a
/// rewrite of an unchanged value changes nothing in git.
///
/// # Panics
///
/// Never for the types of this crate (their serialisation cannot fail).
#[must_use]
pub fn to_json<T: Serialize>(value: &T) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("plain data serialises");
    text.push('\n');
    text
}

/// The person's declined programs in their mirror, under `ours` — our
/// folder of the person's state root (`$XDG_STATE_HOME/ronoctua[-dev]`,
/// whose name is `ronoctua-xdg`'s): `<ours>/person/software/declined.json`.
/// The one spelling of that path, for whoever reads or seeds it.
#[must_use]
pub fn mirror_declined(ours: &std::path::Path) -> std::path::PathBuf {
    ours.join(names::MIRROR).join(names::DECLINED)
}

/// Whether `host` may name a machine (lower-case RFC 1123 label).
#[must_use]
pub fn is_hostname(host: &str) -> bool {
    check::hostname(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_exact_and_round_trip_through_seconds() {
        for (text, s) in [
            ("1970-01-01T00:00:00Z", 0),
            ("2026-10-05T12:00:00Z", 1_791_201_600),
            ("2000-02-29T23:59:59Z", 951_868_799),
        ] {
            let stamp = Stamp::parse(text).unwrap();
            assert_eq!(stamp.unix_s(), s, "{text}");
            assert_eq!(Stamp::from_unix_s(s).as_str(), text);
        }
        for bad in [
            "2026-10-05 12:00:00Z",
            "2026-10-05T12:00:00+01:00",
            "2026-10-05T12:00:00.5Z",
            "2026-02-30T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-10-05T24:00:00Z",
            "",
        ] {
            assert_eq!(Stamp::parse(bad), None, "{bad}");
        }
        assert!(Stamp::parse("2026-10-05T12:00:00Z") < Stamp::parse("2026-10-05T12:00:01Z"));
    }

    /// FAILS IF: the mirror's declined programs are spelled anywhere but
    /// under the mirror, in the file the staged tree names.
    #[test]
    fn the_mirrors_declines_are_under_the_mirror() {
        assert_eq!(
            mirror_declined(std::path::Path::new("/h/.local/state/ronoctua")),
            std::path::Path::new("/h/.local/state/ronoctua/person/software/declined.json")
        );
    }

    #[test]
    fn every_why_has_its_wire_name() {
        for why in Why::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
    }
}
