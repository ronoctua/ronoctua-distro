//! What the last stick gesture did — an import or an export.
//!
//! Root writes it ([`crate::names::STICK_SAID`], `0644`) when an import or an
//! export ends, and the parts' page reads it to say what came of the person's
//! gesture: the units are started from the page and waited for, and their
//! words are a file, never the journal.

use ronoctua_person::{Document, Refusal as FileRefusal, Stamp};
use serde::{Deserialize, Serialize};

/// The last stick gesture, and how it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Said {
    /// The file contract's format (`ronoctua_person::FORMAT`).
    pub format: u32,
    /// Which gesture.
    pub verb: Verb,
    /// When it ended.
    pub at: Stamp,
    /// How.
    pub outcome: Outcome,
}

impl Document for Said {
    fn format(&self) -> u32 {
        self.format
    }

    /// Nothing past the shape: every field is a closed set or a stamp.
    fn validate(&self) -> Result<(), FileRefusal> {
        Ok(())
    }
}

/// A stick gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verb {
    /// Downloads brought in from a stick.
    Import,
    /// This machine's kept downloads written on a stick.
    Export,
}

/// How a stick gesture ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", deny_unknown_fields)]
pub enum Outcome {
    /// Done, with how many downloads it moved.
    Done {
        /// Downloads copied in, or written out.
        count: u32,
    },
    /// Nothing was moved, for this reason.
    Refused {
        /// The reason, as a closed set.
        why: Refusal,
    },
}

/// Why a stick gesture moved nothing — the closed set the page says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    /// The person named no folder.
    NoFolderNamed,
    /// The folder named is not there.
    FolderMissing,
    /// The way to the folder goes through a link.
    FolderThroughALink,
    /// The folder is not on a removable medium (`/run/media/<user>/…`).
    NotARemovableMedium,
    /// The stick holds no downloads of this distro's (no
    /// `ronoctua-parts/nix-cache-info`).
    NoDownloadsThere,
    /// The downloads could not be written there.
    Unwritable,
}

impl Refusal {
    /// Every one.
    pub const ALL: [Self; 6] = [
        Self::NoFolderNamed,
        Self::FolderMissing,
        Self::FolderThroughALink,
        Self::NotARemovableMedium,
        Self::NoDownloadsThere,
        Self::Unwritable,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoFolderNamed => "no-folder-named",
            Self::FolderMissing => "folder-missing",
            Self::FolderThroughALink => "folder-through-a-link",
            Self::NotARemovableMedium => "not-a-removable-medium",
            Self::NoDownloadsThere => "no-downloads-there",
            Self::Unwritable => "unwritable",
        }
    }
}

/// The last gesture, as `path` ([`crate::names::STICK_SAID`] on a machine)
/// says it — `None` when there is none, or it does not read.
#[must_use]
pub fn read(path: &std::path::Path) -> Option<Said> {
    ronoctua_person::parse(&std::fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a refusal's name on disk is not the one the page's keys are
    /// written from, or a said gesture does not read back as written.
    #[test]
    fn a_said_gesture_reads_back_and_names_its_refusal() {
        for why in Refusal::ALL {
            let said = Said {
                format: ronoctua_person::FORMAT,
                verb: Verb::Import,
                at: Stamp::from_unix_s(0),
                outcome: Outcome::Refused { why },
            };
            let json = ronoctua_person::to_json(&said);
            assert!(json.contains(&format!("\"{}\"", why.id())), "{json}");
            let back: Said = ronoctua_person::parse(json.as_bytes()).expect("reads back");
            assert_eq!(back, said);
        }
    }
}
