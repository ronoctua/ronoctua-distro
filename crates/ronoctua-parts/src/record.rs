//! What became of a part: its record, root's, one file per part.
//!
//! The record is the broadcast: the shell's page, the brain's mark and the
//! software centre read it (`0644`), and only root's realizer writes it,
//! atomically. It says the generation in use, the one before it, what the
//! plan wants, and — when the two differ — why: arriving, waiting for
//! something, or failed and tried again.

use ronoctua_person::reports::SkipWhy;
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
    /// For a part of the person's, the fingerprint of the declaration the
    /// plan wants ([`crate::plan::Entry`]): [`Record::wanted`] is what its
    /// realizer evaluated of THAT declaration, or `None` until it did.
    /// Absent for the distro's parts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<String>,
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
    /// The error itself, for the page: Nix's `error:` lines and what they
    /// indent ([`error_of`]), never the list of what it would build before
    /// them nor the outputs it would have made. The whole of what it said is
    /// the realizer's journal.
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
    /// The person's declared entry did not evaluate: an alias that throws,
    /// an `abort` deep in a package.
    EvalFailed,
    /// The person's declared entry is one the system leaves out, for the
    /// reason it gives such an entry in `person-applied.json`
    /// ([`SkipWhy::AttrMissing`]): nixpkgs has no such name. Held with the
    /// consumer's own reason, never as an evaluation that failed.
    AttrMissing,
    /// The same, for [`SkipWhy::NotAPackage`]: the name is not a package.
    NotAPackage,
    /// The same, for [`SkipWhy::RefusedByMeta`]: its `meta` refuses it here
    /// — not free and not consented to, broken or insecure.
    RefusedByMeta,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 8] = [
        Self::BuildFailed,
        Self::OutputDiffers,
        Self::ProfileFailed,
        Self::HandInRefused,
        Self::EvalFailed,
        Self::AttrMissing,
        Self::NotAPackage,
        Self::RefusedByMeta,
    ];

    /// Its kebab-case name — for the three a part is held with for the
    /// system's own reason, that reason's ([`SkipWhy::id`]).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BuildFailed => "build-failed",
            Self::OutputDiffers => "output-differs",
            Self::ProfileFailed => "profile-failed",
            Self::HandInRefused => "hand-in-refused",
            Self::EvalFailed => "eval-failed",
            Self::AttrMissing => "attr-missing",
            Self::NotAPackage => "not-a-package",
            Self::RefusedByMeta => "refused-by-meta",
        }
    }

    /// The reason the system gives an entry it leaves out, when the part was
    /// held with one — what the software centre says of the entry.
    #[must_use]
    pub const fn skipped(self) -> Option<SkipWhy> {
        match self {
            Self::AttrMissing => Some(SkipWhy::AttrMissing),
            Self::NotAPackage => Some(SkipWhy::NotAPackage),
            Self::RefusedByMeta => Some(SkipWhy::RefusedByMeta),
            Self::BuildFailed
            | Self::OutputDiffers
            | Self::ProfileFailed
            | Self::HandInRefused
            | Self::EvalFailed => None,
        }
    }

    /// The one a part is held with when its entry's own build refuses it
    /// with the system's reason `word` (`personParts`'s `refused`), or
    /// `None` for a word that is not one of those.
    #[must_use]
    pub fn of_skip(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|w| w.skipped().is_some_and(|s| s.id() == word))
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

/// How long a failure's detail may be in the record: the error, never a
/// whole log.
pub const DETAIL_MOST: usize = 2048;

/// The error in what `nix` said: each `error:` with the lines it indents
/// under it — what failed, why, and the builder's last words. What is not
/// the error goes, so the footer's region shows a short one:
///
/// - before the first `error:`, the derivations Nix would build, the paths it
///   would fetch and the builds it began;
/// - a line of Nix's own between two errors (another build begun);
/// - an error that only follows another — each derivation that needed the
///   one that failed, `Reason: 1 dependency failed.`;
/// - and inside an error, the `Output paths:` it would have made, a builder's
///   empty line (`>`), and `For full logs, run: nix log …`.
///
/// All of it when no line is an error (a refusal of ours, a program that did
/// not start). The whole of what Nix said — the way to the builder's whole
/// log among it — is the realizer's journal. Pure.
#[must_use]
pub fn error_of(said: &str) -> String {
    // THE ERRORS, each its first line and the indented lines under it.
    let mut errors: Vec<Vec<&str>> = Vec::new();
    for line in said.lines() {
        if line.starts_with("error:") {
            errors.push(vec![line]);
        } else if line.starts_with(char::is_whitespace)
            && let Some(error) = errors.last_mut()
        {
            error.push(line);
        }
    }
    if errors.is_empty() {
        return said.to_owned();
    }
    // AN ERROR THAT ONLY FOLLOWS ANOTHER, as Nix 2.34 says it (`Reason: N
    // dependencies failed.`) and as Nix before it did (`error: N dependencies
    // of derivation '…' failed to build`) — never the only one.
    let follows = |error: &[&str]| {
        let first = error[0];
        (first.contains(" dependencies of derivation ") && first.ends_with(" failed to build"))
            || error.iter().any(|l| {
                let l = l.trim();
                l.starts_with("Reason: ")
                    && (l.ends_with(" dependency failed.") || l.ends_with(" dependencies failed."))
            })
    };
    errors.retain(|e| !follows(e));
    if errors.is_empty() {
        return error_of_whole(said);
    }
    let mut out = String::new();
    for error in errors {
        // What the line before began: the outputs' heading takes the store
        // paths under it, the log's the `nix log` command under it.
        let mut paths = false;
        let mut log = false;
        for line in error {
            let trimmed = line.trim();
            if (paths && trimmed.starts_with("/nix/store/"))
                || (log && trimmed.starts_with("nix log "))
            {
                continue;
            }
            paths = trimmed == "Output paths:";
            log = trimmed == "For full logs, run:";
            // THE OUTPUTS IT WOULD HAVE MADE, A BUILDER'S EMPTY LINE AND THE
            // WAY TO THE WHOLE LOG (both of Nix's forms): none says what
            // failed.
            if paths || log || trimmed == ">" || trimmed.starts_with("For full logs, run ") {
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
    }
    // AS IT ENDED: an error said with no line break after it keeps none.
    if !said.ends_with('\n') {
        out.pop();
    }
    out
}

/// What Nix said from its first `error:` line on, as it said it: what
/// [`error_of`] keeps when every error there only follows another.
fn error_of_whole(said: &str) -> String {
    let mut at = 0;
    for line in said.split_inclusive('\n') {
        if line.starts_with("error:") {
            return said[at..].to_owned();
        }
        at += line.len();
    }
    said.to_owned()
}

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
            declared: None,
        }
    }

    /// The record after the plan says what it wants (`None`: nothing).
    ///
    /// A new target restarts the count of tries; the same target keeps it, so
    /// a boot never forgets how often a part already failed.
    #[must_use]
    pub fn planned(mut self, wanted: Option<Target>) -> Self {
        self.declared = None;
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

    /// The record after the plan wants the person's declaration of
    /// `fingerprint`: the target its realizer evaluated stays while the
    /// declaration is the same one; another declaration is evaluated anew,
    /// its tries counted from none.
    #[must_use]
    pub fn planned_entry(mut self, fingerprint: &str) -> Self {
        if self.declared.as_deref() != Some(fingerprint) {
            self.wanted = None;
            self.attempts = 0;
            self.last_failure = None;
            self.missing.clear();
        }
        self.declared = Some(fingerprint.to_owned());
        self.waiting_for = None;
        self.phase = match &self.wanted {
            Some(want) if self.is_current(want) => Phase::Here,
            _ if self.attempts >= NEEDS_YOU_AFTER => Phase::NeedsYou,
            _ => Phase::Arriving,
        };
        self
    }

    /// The record after the realizer evaluated the person's declaration
    /// into `target`: what the part wants from now on.
    #[must_use]
    pub fn resolved(mut self, target: Target) -> Self {
        if self.is_current(&target) {
            self.phase = Phase::Here;
        }
        self.wanted = Some(target);
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
            detail: bounded(&error_of(detail), DETAIL_MOST),
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
        let detail = bounded(&error_of(detail), DETAIL_MOST);
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

/// `text` in at most `most` bytes. An error too long keeps its first line —
/// what failed — and its end — what the builder said last, and where the
/// whole log is —, and the middle goes; one line with no end of its own
/// keeps its end.
fn bounded(text: &str, most: usize) -> String {
    if text.len() <= most {
        return text.to_owned();
    }
    match text.split_once('\n') {
        Some((first, rest)) if first.len() < most / 4 => {
            let end = tail(rest, most - first.len() - 1);
            // From a line's beginning: half a line is no line Nix said.
            let cut = rest.len() - end.len();
            let end = match end.split_once('\n') {
                Some((_, whole)) if cut > 0 && rest.as_bytes()[cut - 1] != b'\n' => whole,
                _ => end.as_str(),
            };
            format!("{first}\n{end}")
        }
        _ => tail(text, most),
    }
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

    /// FAILS IF: a part of the person's loses the target its realizer
    /// evaluated while the declaration stays the same — every boot would
    /// evaluate and try it again —, keeps it when the declaration changed,
    /// or forgets how often a declaration already failed.
    #[test]
    fn a_persons_part_keeps_its_target_while_its_declaration_stays() {
        let fp_a = "a".repeat(64);
        let fp_b = "b".repeat(64);
        let r = Record::new("person-x").planned_entry(&fp_a);
        assert_eq!((r.phase, r.wanted.clone()), (Phase::Arriving, None));
        assert!(r.try_now(0), "never tried: tried now");
        let failed = r.failed(Why::EvalFailed, "error: attribute missing", at(10), true);
        let again = failed.planned_entry(&fp_a);
        assert_eq!(again.attempts, 1, "the same declaration keeps its count");
        let here = again
            .resolved(want(OUT_1))
            .arrived(at(20))
            .planned_entry(&fp_a);
        assert_eq!(here.phase, Phase::Here);
        assert!(!here.try_now(100), "here: not tried again at a boot");
        let changed = here.planned_entry(&fp_b);
        assert_eq!(
            (changed.phase, changed.wanted.clone(), changed.attempts),
            (Phase::Arriving, None, 0)
        );
        assert_eq!(
            changed.current.map(|c| c.out),
            Some(OUT_1.to_owned()),
            "the generation it had stays until the next arrives"
        );
        let distro = Record::new("x")
            .planned_entry(&fp_a)
            .planned(Some(want(OUT_2)));
        assert_eq!(distro.declared, None, "a distro's part declares nothing");
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

    /// What Nix 2.34 said of a part whose download failed under it, as
    /// `nix build` writes it with no terminal: what it would build first,
    /// then the error.
    const NIX_SAID: &str = "\
these 2 derivations will be built:
  /nix/store/dd7f0jfswx6phjl6zn8pncxkllib8i6f-dep-src.drv
  /nix/store/zd2yv8iyjw4zazyjrgcd8h5sqk8q0wl9-part.drv
building '/nix/store/dd7f0jfswx6phjl6zn8pncxkllib8i6f-dep-src.drv'...
error: Cannot build '/nix/store/dd7f0jfswx6phjl6zn8pncxkllib8i6f-dep-src.drv'.
       Reason: builder failed with exit code 1.
       Output paths:
         /nix/store/2p53prbj1nki6rnfzyp35b9msj89rhv7-dep-src
       Last 2 log lines:
       > curl: (22) The requested URL returned error: 404
       > error: cannot download x.tar.gz from any mirror
       For full logs, run:
         nix log /nix/store/dd7f0jfswx6phjl6zn8pncxkllib8i6f-dep-src.drv
error: Cannot build '/nix/store/zd2yv8iyjw4zazyjrgcd8h5sqk8q0wl9-part.drv'.
       Reason: 1 dependency failed.
       Output paths:
         /nix/store/9ychva4wjpip2f0bp9vbrgk4mw5wy2sj-part
";

    /// FAILS IF: a failed part's record keeps what Nix would have built
    /// instead of the error — the page opened on a list of derivations —,
    /// keeps the outputs the build would have made, the way to the whole log
    /// or the errors that only follow the first (the footer's region shows a
    /// short error), loses the builder's words, or a refusal of ours with no
    /// `error:` in it is lost.
    #[test]
    fn a_failure_keeps_the_error_itself() {
        let failed = Record::new("x").planned(Some(want(OUT_1))).trying().failed(
            Why::BuildFailed,
            NIX_SAID,
            at(1),
            true,
        );
        let detail = failed.last_failure.expect("a failure").detail;
        assert_eq!(
            detail,
            "\
error: Cannot build '/nix/store/dd7f0jfswx6phjl6zn8pncxkllib8i6f-dep-src.drv'.
       Reason: builder failed with exit code 1.
       Last 2 log lines:
       > curl: (22) The requested URL returned error: 404
       > error: cannot download x.tar.gz from any mirror
"
        );
        // AS NIX BEFORE 2.34 SAID IT: the error, and the derivation that
        // needed it, on a line of its own.
        let older = "\
these 2 derivations will be built:
  /nix/store/a.drv
building '/nix/store/a.drv'...
error: builder for '/nix/store/a.drv' failed with exit code 1;
       last 1 log lines:
       >
       > no such file
       For full logs, run 'nix log /nix/store/a.drv'.
building '/nix/store/c.drv'...
error: 1 dependencies of derivation '/nix/store/b.drv' failed to build
";
        assert_eq!(
            error_of(older),
            "\
error: builder for '/nix/store/a.drv' failed with exit code 1;
       last 1 log lines:
       > no such file
"
        );
        // AN EVALUATION'S ERROR is one error, its trace indented under it.
        let evaluated =
            "error:\n       … while evaluating 'x'\n       error: attribute 'y' missing\n";
        assert_eq!(error_of(evaluated), evaluated);
        let one = "error: attribute 'ripgrep' missing";
        assert_eq!(error_of(one), one, "as it ended, with no line break");
        let ours = "built /nix/store/a, the plan wants /nix/store/b";
        assert_eq!(error_of(ours), ours);
        let refused = Record::new("x").refused_hand_in(ours, at(2));
        assert_eq!(refused.last_failure.expect("said").detail, ours);
        // AN ERROR TOO LONG keeps what failed and what the builder said last.
        let long = format!(
            "error: Cannot build 'x.drv'.\n{}> the last thing it said\n",
            "> a line the builder printed\n".repeat(200)
        );
        let kept = bounded(&long, DETAIL_MOST);
        assert!(kept.len() <= DETAIL_MOST, "{}", kept.len());
        assert!(kept.starts_with("error: Cannot build 'x.drv'.\n"));
        assert!(kept.ends_with("> the last thing it said\n"));
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
