//! What a search finds, whatever the source.
//!
//! Every source answers with the same row, so the screen draws one list and the
//! person compares a Nix package and a Flatpak side by side. What differs is
//! said by [`Badge`]s, each a glyph and a sentence, never a colour alone.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{AppKey, Refusal, Source};

/// One program a search found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Found {
    /// Its identity.
    pub key: AppKey,
    /// Its name as its source gives it (data, not translated).
    pub name: String,
    /// One line on what it does (data).
    #[serde(default)]
    pub summary: String,
    /// The version its source would install, when known.
    #[serde(default)]
    pub version: String,
    /// What distinguishes it.
    #[serde(default)]
    pub badges: Vec<Badge>,
    /// How much would be downloaded, when the source says.
    #[serde(default)]
    pub download_bytes: Option<u64>,
    /// Where its picture is, when it has one.
    #[serde(default)]
    pub icon: Option<Picture>,
    /// Its site.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Its licence, as SPDX when known.
    #[serde(default)]
    pub license: Option<String>,
    /// Who made it, when the source says.
    #[serde(default)]
    pub publisher: Option<String>,
    /// The command it runs (Nix `mainProgram`): typing `rg` finds ripgrep.
    #[serde(default)]
    pub program: Option<String>,
    /// How well the query matched, lower is better (0 = name prefix). The
    /// screen ranks with `minibuffer::rank`; this is the source's own tier for
    /// rows it matched by what they DO rather than by name.
    #[serde(default)]
    pub by_description: bool,
}

/// Where a picture can be read from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "at"
)]
pub enum Picture {
    /// On the network (a catalogue's icon URL).
    Url {
        /// The address.
        url: String,
    },
    /// On disk.
    File {
        /// The path.
        path: PathBuf,
    },
    /// An icon-theme name, looked up through the launcher's chain.
    Named {
        /// The name.
        name: String,
    },
}

/// One distinguishing fact about a found program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Badge {
    /// Flathub verified the publisher.
    Verified,
    /// The licence is not free.
    Unfree,
    /// It runs in a sandbox (every Flatpak).
    Sandboxed,
    /// A command-line program: it opens in a terminal.
    CommandLine,
    /// Built only for ARM.
    ArmOnly,
    /// nixpkgs marks it broken.
    Broken,
    /// nixpkgs marks it insecure.
    Insecure,
    /// Its runtime reached end of life.
    EndOfLife,
    /// F-Droid lists anti-features (ads, tracking, non-free network services).
    AntiFeatures,
    /// A library or a language package rather than a program.
    Library,
    /// Already installed.
    Installed,
}

impl Badge {
    /// Every one, for `software-words-are-total`.
    pub const ALL: [Self; 11] = [
        Self::Verified,
        Self::Unfree,
        Self::Sandboxed,
        Self::CommandLine,
        Self::ArmOnly,
        Self::Broken,
        Self::Insecure,
        Self::EndOfLife,
        Self::AntiFeatures,
        Self::Library,
        Self::Installed,
    ];

    /// The stem of its text key (`software-badge-<word>`) and glyph token.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unfree => "unfree",
            Self::Sandboxed => "sandboxed",
            Self::CommandLine => "command-line",
            Self::ArmOnly => "arm-only",
            Self::Broken => "broken",
            Self::Insecure => "insecure",
            Self::EndOfLife => "end-of-life",
            Self::AntiFeatures => "anti-features",
            Self::Library => "library",
            Self::Installed => "installed",
        }
    }

    /// A badge that makes installing a REFUSAL rather than a choice.
    #[must_use]
    pub const fn refuses(self) -> Option<Refusal> {
        match self {
            Self::Broken => Some(Refusal::Broken),
            Self::Insecure => Some(Refusal::Insecure),
            _ => None,
        }
    }
}

/// One source's answer to one query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Batch {
    /// Whose answer.
    pub source: Source,
    /// Which query it answers: the screen drops a batch whose generation is
    /// not the latest, so a slow answer never overwrites a newer one.
    pub generation: u64,
    /// What was found, in the source's own order.
    pub found: Vec<Found>,
    /// How the source is.
    pub status: Status,
}

/// How a source stands for this search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "status"
)]
pub enum Status {
    /// It answered. `from` says what the answer is based on when that is worth
    /// saying (the Nix index's revision, the date of a cached catalogue).
    Answered {
        /// The basis, as data.
        #[serde(default)]
        from: Option<String>,
    },
    /// It is still asking.
    Asking,
    /// It did not answer, and why.
    Refused {
        /// The reason.
        refusal: Refusal,
    },
    /// The person turned it off.
    Off,
}

/// Which source to recommend first when a program exists in several.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Prefer {
    /// Nix first (the system's own revision, no runtime to download), then a
    /// verified Flatpak — the owner's choice.
    #[default]
    NixFirst,
    /// Flatpak first for graphical programs; command-line programs stay Nix.
    FlatpakFirst,
}

impl Prefer {
    /// Every one, in the ring's order.
    pub const ALL: [Self; 2] = [Self::NixFirst, Self::FlatpakFirst];

    /// The stem of its text key.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::NixFirst => "nix-first",
            Self::FlatpakFirst => "flatpak-first",
        }
    }
}

/// Of several rows that are the same program, which one to recommend.
///
/// The order is: the one already installed; a row a badge refuses never; then
/// by preference — Nix first means Nix, verified Flatpak, Flatpak, AppImage,
/// Android; Flatpak first puts verified Flatpak and Flatpak before Nix unless
/// the program is a command-line one.
#[must_use]
pub fn recommended(rows: &[&Found], prefer: Prefer) -> Option<usize> {
    let rank = |f: &Found| -> u8 {
        if f.badges.contains(&Badge::Installed) {
            return 0;
        }
        if f.badges.iter().any(|b| b.refuses().is_some()) {
            return u8::MAX;
        }
        let verified = f.badges.contains(&Badge::Verified);
        let cli = f.badges.contains(&Badge::CommandLine);
        match (f.key.source(), prefer) {
            (Some(Source::Nix), Prefer::NixFirst) => 1,
            (Some(Source::Nix), Prefer::FlatpakFirst) => {
                if cli {
                    1
                } else {
                    4
                }
            }
            (Some(Source::Flatpak), Prefer::NixFirst) => {
                if verified {
                    2
                } else {
                    3
                }
            }
            (Some(Source::Flatpak), Prefer::FlatpakFirst) => {
                if verified {
                    2
                } else {
                    3
                }
            }
            (Some(Source::AppImage), _) => 5,
            (Some(Source::Android), _) => 6,
            (Some(Source::Web), _) => 7,
            (None, _) => 8,
        }
    };
    rows.iter()
        .enumerate()
        .filter(|(_, f)| rank(f) != u8::MAX)
        .min_by_key(|(_, f)| rank(f))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: AppKey, badges: &[Badge]) -> Found {
        Found {
            key,
            name: "X".into(),
            summary: String::new(),
            version: String::new(),
            badges: badges.to_vec(),
            download_bytes: None,
            icon: None,
            homepage: None,
            license: None,
            publisher: None,
            program: None,
            by_description: false,
        }
    }

    #[test]
    fn nix_first_is_the_default_and_a_broken_package_is_never_recommended() {
        let nix = row(
            AppKey::Nix {
                attr: "gimp".into(),
            },
            &[],
        );
        let broken = row(
            AppKey::Nix {
                attr: "gimp".into(),
            },
            &[Badge::Broken],
        );
        let flat = row(
            AppKey::Flatpak {
                id: "org.gimp.GIMP".into(),
            },
            &[Badge::Verified],
        );
        assert_eq!(recommended(&[&flat, &nix], Prefer::default()), Some(1));
        assert_eq!(recommended(&[&broken, &flat], Prefer::NixFirst), Some(1));
        assert_eq!(recommended(&[&broken], Prefer::NixFirst), None);
    }

    #[test]
    fn flatpak_first_keeps_command_line_programs_on_nix() {
        let cli = row(
            AppKey::Nix {
                attr: "ripgrep".into(),
            },
            &[Badge::CommandLine],
        );
        let flat = row(
            AppKey::Flatpak {
                id: "io.x.Rg".into(),
            },
            &[],
        );
        assert_eq!(recommended(&[&flat, &cli], Prefer::FlatpakFirst), Some(1));
        let gui = row(
            AppKey::Nix {
                attr: "gimp".into(),
            },
            &[],
        );
        assert_eq!(recommended(&[&gui, &flat], Prefer::FlatpakFirst), Some(1));
    }

    #[test]
    fn what_is_installed_wins_over_any_preference() {
        let nix = row(
            AppKey::Nix {
                attr: "gimp".into(),
            },
            &[],
        );
        let flat = row(
            AppKey::Flatpak {
                id: "org.gimp.GIMP".into(),
            },
            &[Badge::Installed],
        );
        assert_eq!(recommended(&[&nix, &flat], Prefer::NixFirst), Some(1));
    }
}
