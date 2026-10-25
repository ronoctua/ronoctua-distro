//! What became of a part: its record, root's, one file per part.
//!
//! The record is the broadcast: the shell's page, the brain's mark and the
//! software centre read it (`0644`), and only root's realizer writes it,
//! atomically. It says the generation in use, the one before it, what the
//! plan wants, and — when the two differ — why: arriving, waiting for
//! something, or failed and tried again.

use ronoctua_person::{Document, FORMAT, Refusal, Stamp, Why as FileWhy};
use serde::{Deserialize, Serialize};

/// One part's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Record {
    /// [`FORMAT`].
    pub format: u32,
    /// The part.
    pub id: String,
    /// What the running system's plan wants, or `None` when it wants nothing
    /// of this part any more.
    pub wanted: Option<Target>,
    /// The generation in use: what the profile points at.
    pub current: Option<Generation>,
    /// The generation before it, kept so a part can go back.
    pub previous: Option<Generation>,
    /// Where it stands.
    pub phase: Phase,
    /// What it waits for, while [`Phase::Waiting`].
    pub waiting_for: Option<Waiting>,
    /// The tries of the wanted output since it was last here.
    pub attempts: u32,
    /// Why the last try did not bring it, when it did not.
    pub last_failure: Option<Failure>,
    /// The downloads the part still needs, from their origins: what the
    /// last failed try would have fetched ([`ronoctua_person::frontier`]). Each is a
    /// way in for the person — a file handed in, a stick, a mirror.
    pub missing: Vec<ronoctua_person::frontier::Download>,
}

/// What the plan wants of a part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Target {
    /// The derivation.
    pub drv: String,
    /// The output.
    pub out: String,
    /// The interface, part of the profile's path.
    pub interface: u32,
}

/// A generation of a part's profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Generation {
    /// The store path the profile points at.
    pub out: String,
    /// The interface it was realized at.
    pub interface: u32,
    /// When it became the profile's.
    pub since: Stamp,
}

/// Where a part stands — the closed set the screen draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// The wanted output is the profile's: nothing to do.
    Here,
    /// Being realized now, or queued to be.
    Arriving,
    /// Not tried, because of [`Record::waiting_for`].
    Waiting,
    /// Tried [`NEEDS_YOU_AFTER`] times with a network and failed every time:
    /// still tried, ever more rarely, and the person is asked to step in.
    NeedsYou,
    /// The plan does not want it (any more); a profile left behind stays
    /// until it is trimmed.
    Unwanted,
}

impl Phase {
    /// Every one, in the order the keys would be written.
    pub const ALL: [Self; 5] = [
        Self::Here,
        Self::Arriving,
        Self::Waiting,
        Self::NeedsYou,
        Self::Unwanted,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Here => "here",
            Self::Arriving => "arriving",
            Self::Waiting => "waiting",
            Self::NeedsYou => "needs-you",
            Self::Unwanted => "unwanted",
        }
    }
}

/// What a part waits for before it is tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Waiting {
    /// No link at all: a network coming up tries it again.
    Network,
    /// A metered link: nothing is downloaded on it by itself. Another
    /// network tries it again, and so does the person's "try now" — the yes
    /// for that one try ([`crate::names::YES`]).
    Metered,
}

impl Waiting {
    /// Every one.
    pub const ALL: [Self; 2] = [Self::Network, Self::Metered];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Metered => "metered",
        }
    }
}

/// Whether a try waits for another network: on a metered link, unless the
/// person said yes to this one try. A link that does not say counts as not
/// metered — NetworkManager's "unknown". Pure.
#[must_use]
pub fn waits_for_the_meter(metered: Option<bool>, yes: bool) -> bool {
    metered == Some(true) && !yes
}

/// Why a try did not bring the part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Failure {
    /// The reason, as a closed set.
    pub why: Why,
    /// When.
    pub at: Stamp,
    /// What `nix` said last, for the journal and the page's detail.
    pub detail: String,
}

/// Why a try failed — the closed set the screen says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// `nix build` of the part failed: a download, or a build under it.
    BuildFailed,
    /// The build gave another output than the plan's: the plan is stale, and
    /// the next switch brings the right one.
    OutputDiffers,
    /// The profile could not be switched to the built output.
    ProfileFailed,
    /// A file handed in is not the download its name says: other bytes.
    HandInRefused,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 4] = [
        Self::BuildFailed,
        Self::OutputDiffers,
        Self::ProfileFailed,
        Self::HandInRefused,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BuildFailed => "build-failed",
            Self::OutputDiffers => "output-differs",
            Self::ProfileFailed => "profile-failed",
            Self::HandInRefused => "hand-in-refused",
        }
    }
}

/// How many failed tries with a network before the person is asked.
pub const NEEDS_YOU_AFTER: u32 = 5;

/// The first wait after a failed try, in seconds; each try after it waits
/// twice as long, up to [`LONGEST_WAIT_S`].
pub const FIRST_WAIT_S: i64 = 60;

/// The longest wait between two tries: a part that keeps failing is still
/// tried twice a day, and the person can always try it now.
pub const LONGEST_WAIT_S: i64 = 12 * 3600;

/// How long to wait after the `attempts`-th failed try.
#[must_use]
pub fn wait_s(attempts: u32) -> i64 {
    let doublings = attempts.saturating_sub(1).min(20);
    FIRST_WAIT_S
        .saturating_mul(1_i64 << doublings)
        .min(LONGEST_WAIT_S)
}

/// How long a failure's detail may be in the record: the last lines of what
/// `nix` said, never a whole log.
pub const DETAIL_MOST: usize = 2048;

impl Record {
    /// A part never seen before.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self {
            format: FORMAT,
            id: id.to_owned(),
            wanted: None,
            current: None,
            previous: None,
            phase: Phase::Unwanted,
            waiting_for: None,
            attempts: 0,
            last_failure: None,
            missing: Vec::new(),
        }
    }

    /// The record after the plan says what it wants (`None`: nothing).
    ///
    /// A new target restarts the count of tries; the same target keeps it, so
    /// a boot never forgets how often a part already failed.
    #[must_use]
    pub fn planned(mut self, wanted: Option<Target>) -> Self {
        if self.wanted != wanted {
            self.attempts = 0;
            self.last_failure = None;
            self.missing.clear();
        }
        self.wanted = wanted;
        self.waiting_for = None;
        self.phase = match &self.wanted {
            None => Phase::Unwanted,
            Some(want) if self.is_current(want) => Phase::Here,
            Some(_) if self.attempts >= NEEDS_YOU_AFTER => Phase::NeedsYou,
            Some(_) => Phase::Arriving,
        };
        self
    }

    /// When the next try is due, in seconds since the epoch: after a failed
    /// try with a network, its wait ([`wait_s`]). `None` when nothing is due
    /// on a clock — the part is here, unwanted, never tried, or waits for a
    /// network, which an event brings.
    #[must_use]
    pub fn due_at(&self) -> Option<i64> {
        if !matches!(self.phase, Phase::Arriving | Phase::NeedsYou) || self.attempts == 0 {
            return None;
        }
        let failed = self.last_failure.as_ref()?;
        Some(failed.at.unix_s().saturating_add(wait_s(self.attempts)))
    }

    /// Whether the planner should try it now (`now`, seconds since the
    /// epoch): never tried, waiting for a network (the planner runs when one
    /// comes), or due.
    #[must_use]
    pub fn try_now(&self, now: i64) -> bool {
        match self.phase {
            Phase::Waiting => true,
            Phase::Arriving | Phase::NeedsYou => self.due_at().is_none_or(|due| due <= now),
            Phase::Here | Phase::Unwanted => false,
        }
    }

    /// The part waits for `what`, not tried: no attempt counted.
    #[must_use]
    pub fn waits(mut self, what: Waiting) -> Self {
        self.phase = Phase::Waiting;
        self.waiting_for = Some(what);
        self
    }

    /// Whether the profile already holds `want`.
    #[must_use]
    pub fn is_current(&self, want: &Target) -> bool {
        self.current
            .as_ref()
            .is_some_and(|c| c.out == want.out && c.interface == want.interface)
    }

    /// The record as a try begins.
    #[must_use]
    pub fn trying(mut self) -> Self {
        self.phase = Phase::Arriving;
        self.waiting_for = None;
        self
    }

    /// The record after the wanted output became the profile's, at `now`.
    #[must_use]
    pub fn arrived(mut self, now: Stamp) -> Self {
        if let Some(want) = self.wanted.clone()
            && !self.is_current(&want)
        {
            self.previous = self.current.take();
            self.current = Some(Generation {
                out: want.out,
                interface: want.interface,
                since: now,
            });
        }
        self.phase = Phase::Here;
        self.waiting_for = None;
        self.attempts = 0;
        self.last_failure = None;
        self.missing.clear();
        self
    }

    /// The record after a file handed in was refused: said, and not counted
    /// as a try of the part.
    #[must_use]
    pub fn refused_hand_in(mut self, detail: &str, now: Stamp) -> Self {
        self.last_failure = Some(Failure {
            why: Why::HandInRefused,
            at: now,
            detail: tail(detail, DETAIL_MOST),
        });
        self
    }

    /// The record with the downloads the part still needs: what the last
    /// try would have fetched, said after it failed.
    #[must_use]
    pub fn missing(mut self, downloads: Vec<ronoctua_person::frontier::Download>) -> Self {
        self.missing = downloads;
        self
    }

    /// The record after a try failed — counted only when it could have
    /// worked: a try with no network says it waits, and is not counted.
    #[must_use]
    pub fn failed(mut self, why: Why, detail: &str, now: Stamp, linked: bool) -> Self {
        let detail = tail(detail, DETAIL_MOST);
        if linked {
            self.attempts = self.attempts.saturating_add(1);
            self.waiting_for = None;
            self.phase = if self.attempts >= NEEDS_YOU_AFTER {
                Phase::NeedsYou
            } else {
                Phase::Arriving
            };
        } else {
            self.waiting_for = Some(Waiting::Network);
            self.phase = Phase::Waiting;
        }
        self.last_failure = Some(Failure {
            why,
            at: now,
            detail,
        });
        self
    }
}

/// Every record in `dir` ([`crate::names::RECORDS`] on a machine), in id
/// order; one that does not read is left out. What the parts' page and the
/// brain's watch read — the realizer reads the same files.
#[must_use]
pub fn read_all(dir: &std::path::Path) -> Vec<Record> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".json")
                .map(str::to_owned)
        })
        .filter(|id| crate::id::is_id(id))
        .collect();
    ids.sort();
    ids.iter()
        .filter_map(|id| std::fs::read(dir.join(format!("{id}.json"))).ok())
        .filter_map(|bytes| ronoctua_person::parse::<Record>(&bytes).ok())
        .collect()
}

/// The last `most` bytes of `text`, cut at a character boundary.
fn tail(text: &str, most: usize) -> String {
    if text.len() <= most {
        return text.to_owned();
    }
    let mut from = text.len() - most;
    while !text.is_char_boundary(from) {
        from += 1;
    }
    text[from..].to_owned()
}

impl Document for Record {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if !crate::id::is_id(&self.id) {
            return Err(Refusal::new(FileWhy::Invalid, "/id", self.id.clone()));
        }
        let paths = self
            .wanted
            .iter()
            .flat_map(|w| [("/wanted/out", &w.out)])
            .chain(self.current.iter().map(|g| ("/current/out", &g.out)))
            .chain(self.previous.iter().map(|g| ("/previous/out", &g.out)));
        for (at, path) in paths {
            if !crate::plan::is_store_path(path) {
                return Err(Refusal::new(FileWhy::Invalid, at, path.clone()));
            }
        }
        if self.waiting_for.is_some() != (self.phase == Phase::Waiting) {
            return Err(Refusal::new(
                FileWhy::Invalid,
                "/waiting-for",
                "waiting-for is set exactly while the phase is waiting",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a part downloads by itself on a metered link, the person's
    /// yes does not let one try through, or a link that does not say holds
    /// every part back.
    #[test]
    fn a_metered_link_holds_back_all_but_the_persons_try() {
        assert!(waits_for_the_meter(Some(true), false));
        assert!(!waits_for_the_meter(Some(true), true));
        assert!(!waits_for_the_meter(Some(false), false));
        assert!(!waits_for_the_meter(None, false));
        let r = Record::new("x").waits(Waiting::Metered);
        assert_eq!(
            (r.phase, r.waiting_for, r.attempts),
            (Phase::Waiting, Some(Waiting::Metered), 0)
        );
        assert!(r.try_now(0), "another network tries it again");
    }

    const OUT_1: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-yt-dlp-1";
    const OUT_2: &str = "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-yt-dlp-2";
    const DRV: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-yt-dlp.drv";

    fn want(out: &str) -> Target {
        Target {
            drv: DRV.to_owned(),
            out: out.to_owned(),
            interface: 1,
        }
    }

    fn at(s: i64) -> Stamp {
        Stamp::from_unix_s(s)
    }

    /// FAILS IF: a part that failed loses the generation it had, or a part
    /// that arrives forgets the one before — the promise is that a part
    /// which cannot update keeps working, and one that did can go back.
    #[test]
    fn a_failed_part_keeps_what_it_had() {
        let here = Record::new("yt-dlp")
            .planned(Some(want(OUT_1)))
            .arrived(at(10));
        assert_eq!(here.phase, Phase::Here);
        assert_eq!(here.current.as_ref().map(|g| g.out.as_str()), Some(OUT_1));

        let failed =
            here.planned(Some(want(OUT_2)))
                .trying()
                .failed(Why::BuildFailed, "404", at(20), true);
        assert_eq!(failed.current.as_ref().map(|g| g.out.as_str()), Some(OUT_1));
        assert_eq!(failed.phase, Phase::Arriving);
        assert_eq!(failed.attempts, 1);

        let arrived = failed.arrived(at(30));
        assert_eq!(
            arrived.current.as_ref().map(|g| g.out.as_str()),
            Some(OUT_2)
        );
        assert_eq!(
            arrived.previous.as_ref().map(|g| g.out.as_str()),
            Some(OUT_1)
        );
        assert_eq!((arrived.attempts, arrived.last_failure), (0, None));
    }

    /// FAILS IF: a try with no network counts against the part — a laptop
    /// carried offline for a week would come back asking the person about
    /// every part — or the count does not reach the person when the network
    /// was there and the part still failed.
    #[test]
    fn only_tries_that_could_have_worked_count() {
        let mut record = Record::new("chrome").planned(Some(want(OUT_1)));
        for s in 0..10 {
            record = record.failed(Why::BuildFailed, "no link", at(s), false);
        }
        assert_eq!((record.attempts, record.phase), (0, Phase::Waiting));
        assert_eq!(record.waiting_for, Some(Waiting::Network));

        for s in 0..NEEDS_YOU_AFTER {
            record = record
                .trying()
                .failed(Why::BuildFailed, "403", at(i64::from(s)), true);
        }
        assert_eq!(record.phase, Phase::NeedsYou);

        // A boot replans the same target: the count survives it.
        let replanned = record.clone().planned(Some(want(OUT_1)));
        assert_eq!(
            (replanned.attempts, replanned.phase),
            (NEEDS_YOU_AFTER, Phase::NeedsYou)
        );
        // A new target starts over.
        let new = record.planned(Some(want(OUT_2)));
        assert_eq!((new.attempts, new.phase), (0, Phase::Arriving));
    }

    /// FAILS IF: the waits do not grow, grow past twelve hours, or a part
    /// that failed is tried again before its wait — a retry storm on every
    /// planner run — or one that waits for a network is not tried when the
    /// planner runs because a network came.
    #[test]
    fn a_failed_part_waits_longer_each_time() {
        assert_eq!((wait_s(1), wait_s(2), wait_s(3)), (60, 120, 240));
        assert_eq!(wait_s(99), LONGEST_WAIT_S);
        let failed = Record::new("x").planned(Some(want(OUT_1))).trying().failed(
            Why::BuildFailed,
            "404",
            at(1_000),
            true,
        );
        assert_eq!(failed.due_at(), Some(1_060));
        assert!(!failed.try_now(1_059));
        assert!(failed.try_now(1_060));
        let offline = Record::new("y").planned(Some(want(OUT_1))).failed(
            Why::BuildFailed,
            "no link",
            at(1_000),
            false,
        );
        assert_eq!(offline.due_at(), None);
        assert!(offline.try_now(1_000));
        let fresh = Record::new("z").planned(Some(want(OUT_1)));
        assert!(fresh.try_now(0));
    }

    /// FAILS IF: a record goes out in a shape its own reader refuses, or a
    /// detail grows without bound — a failing build's whole log would end up
    /// in a file the shell reads at every change.
    #[test]
    fn a_record_reads_back_and_its_detail_is_bounded() {
        let long = "x".repeat(DETAIL_MOST * 3) + "é";
        let record = Record::new("yt-dlp").planned(Some(want(OUT_1))).failed(
            Why::BuildFailed,
            &long,
            at(5),
            true,
        );
        let detail = &record.last_failure.as_ref().expect("a failure").detail;
        assert!(detail.len() <= DETAIL_MOST && detail.ends_with('é'));
        let back: Record = ronoctua_person::parse(ronoctua_person::to_json(&record).as_bytes())
            .expect("reads back");
        assert_eq!(back, record);
    }
}
