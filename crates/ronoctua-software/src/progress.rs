//! The lines the CLI prints so the queue can draw a bar.
//!
//! The queue runs `ronoctua-software run --progress` as a child and reads its
//! standard output, exactly as it reads yt-dlp's: every line that starts with
//! [`MARK`] and a tab is ours, and everything else is the programs' own chatter,
//! which the queue keeps as the job's trouble when it fails.
//!
//! ```text
//! RONOCTUA<TAB>step<TAB>downloading<TAB>1048576<TAB>52428800<TAB>org.gnome.Calculator
//! RONOCTUA<TAB>done<TAB>{"key":…,"desktop-ids":[…]}
//! RONOCTUA<TAB>failed<TAB>{"refusal":"offline","trouble":"…"}
//! ```
//!
//! Tab-separated, because a name has spaces and a tab never appears in one;
//! the last field of a step takes the rest of the line. The two ends carry JSON
//! because they carry a structure, and a second grammar for it would be the
//! one the two sides disagree about.

use serde::{Deserialize, Serialize};

use crate::{Failure, Outcome};

/// What every line of ours starts with — the queue's own mark, one word for
/// both children it reads.
pub const MARK: &str = "RONOCTUA";

/// What is happening right now, for the job's sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Working out what the order means (a name to a package, a page to an icon).
    Resolving,
    /// Another order of the same source is running; this one waits its turn.
    Waiting,
    /// Bytes are arriving.
    Downloading,
    /// Checking a signature or a hash.
    Verifying,
    /// Opening an archive or an image.
    Unpacking,
    /// Nix is building (a trial, or a package with no binary cache).
    Building,
    /// Putting it in place.
    Installing,
    /// Writing the `.desktop` and the icon.
    Integrating,
    /// Taking it out.
    Removing,
    /// Starting it (a trial, the Android session).
    Starting,
    /// Reading a catalogue.
    Indexing,
    /// Preparing a source (the Flathub remote, the Android image).
    Preparing,
}

impl Phase {
    /// Every one, for `software-words-are-total`.
    pub const ALL: [Self; 12] = [
        Self::Resolving,
        Self::Waiting,
        Self::Downloading,
        Self::Verifying,
        Self::Unpacking,
        Self::Building,
        Self::Installing,
        Self::Integrating,
        Self::Removing,
        Self::Starting,
        Self::Indexing,
        Self::Preparing,
    ];

    /// The stem of its text key (`software-phase-<word>`), and its wire word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Resolving => "resolving",
            Self::Waiting => "waiting",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Unpacking => "unpacking",
            Self::Building => "building",
            Self::Installing => "installing",
            Self::Integrating => "integrating",
            Self::Removing => "removing",
            Self::Starting => "starting",
            Self::Indexing => "indexing",
            Self::Preparing => "preparing",
        }
    }

    /// The phase a word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.word() == word)
    }
}

/// One line of ours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Beat {
    /// Progress. `total` zero means not known — the queue then draws the
    /// phase and no bar, which is the truth.
    Step {
        /// What is happening.
        phase: Phase,
        /// How much is behind.
        done: u64,
        /// How much there is, or zero.
        total: u64,
        /// The name it is on (data, not translated).
        now: String,
    },
    /// The order finished.
    Done(Outcome),
    /// The order failed.
    Failed(Failure),
}

impl Beat {
    /// The line, without its newline.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::Step {
                phase,
                done,
                total,
                now,
            } => {
                // A tab or a newline inside the name would split the line; the
                // name is data and loses nothing by being flattened.
                let now: String = now
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect();
                format!("{MARK}\tstep\t{}\t{done}\t{total}\t{now}", phase.word())
            }
            Self::Done(o) => format!(
                "{MARK}\tdone\t{}",
                serde_json::to_string(o).unwrap_or_default()
            ),
            Self::Failed(f) => format!(
                "{MARK}\tfailed\t{}",
                serde_json::to_string(f).unwrap_or_default()
            ),
        }
    }

    /// A line back into a beat; `None` for a line that is not ours.
    #[must_use]
    pub fn of_line(line: &str) -> Option<Self> {
        let rest = line
            .trim_end_matches(['\r', '\n'])
            .strip_prefix(MARK)?
            .strip_prefix('\t')?;
        let (kind, rest) = rest.split_once('\t')?;
        match kind {
            "step" => {
                let mut f = rest.splitn(4, '\t');
                let phase = Phase::of_word(f.next()?)?;
                let done = f.next()?.parse().ok()?;
                let total = f.next()?.parse().ok()?;
                let now = f.next().unwrap_or("").to_owned();
                Some(Self::Step {
                    phase,
                    done,
                    total,
                    now,
                })
            }
            "done" => serde_json::from_str(rest).ok().map(Self::Done),
            "failed" => serde_json::from_str(rest).ok().map(Self::Failed),
            _ => None,
        }
    }
}

/// Where the beats go while an order runs. The CLI prints them; a test keeps
/// them in a `Vec`.
pub trait Report {
    /// One beat.
    fn beat(&mut self, beat: Beat);

    /// Progress, shortened.
    fn step(&mut self, phase: Phase, done: u64, total: u64, now: &str) {
        self.beat(Beat::Step {
            phase,
            done,
            total,
            now: now.to_owned(),
        });
    }
}

impl Report for Vec<Beat> {
    fn beat(&mut self, beat: Beat) {
        self.push(beat);
    }
}

/// Prints every beat on standard output, flushed at once (the queue reads the
/// pipe line by line and a buffered beat is a frozen bar).
#[derive(Debug, Default)]
pub struct Printed;

impl Report for Printed {
    fn beat(&mut self, beat: Beat) {
        use std::io::Write as _;
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{}", beat.line());
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppKey, Refusal};

    #[test]
    fn every_beat_comes_back_from_its_line() {
        let beats = [
            Beat::Step {
                phase: Phase::Downloading,
                done: 10,
                total: 100,
                now: "a b\tc".into(),
            },
            Beat::Done(Outcome {
                key: Some(AppKey::Flatpak {
                    id: "org.x.Y".into(),
                }),
                desktop_ids: vec!["org.x.Y.desktop".into()],
                amount: None,
                rehearsed: false,
            }),
            Beat::Failed(Failure {
                refusal: Some(Refusal::Offline),
                trouble: "no route".into(),
            }),
        ];
        for b in beats {
            let back = Beat::of_line(&b.line()).expect("our own line");
            match (&b, &back) {
                // The tab inside the name is flattened on the way out.
                (Beat::Step { now, .. }, Beat::Step { now: n2, .. }) => {
                    assert_eq!(n2, &now.replace('\t', " "))
                }
                _ => assert_eq!(back, b),
            }
        }
    }

    #[test]
    fn a_line_that_is_not_ours_is_nobodys() {
        assert_eq!(Beat::of_line("Installing 1/3… 45%"), None);
        assert_eq!(Beat::of_line("RONOCTUAstep"), None);
        assert_eq!(Beat::of_line("RONOCTUA\tstep\tflying\t1\t2\tx"), None);
    }

    #[test]
    fn every_phase_is_found_by_its_word() {
        for p in Phase::ALL {
            assert_eq!(Phase::of_word(p.word()), Some(p));
        }
    }
}
