//! Nix's `--log-format internal-json` lines, read into progress.
//!
//! # What Nix prints
//!
//! With `--log-format internal-json`, every line Nix writes on stderr is
//! `@nix ` and one JSON object (`JSONLogger`, `src/libutil/logging.cc`):
//!
//! ```text
//! @nix {"action":"start","id":…,"level":0,"parent":0,"text":"","type":104}
//! @nix {"action":"result","id":…,"type":105,"fields":[done,expected,running,failed]}
//! @nix {"action":"result","id":…,"type":106,"fields":[activityType,expected]}
//! @nix {"action":"stop","id":…}
//! @nix {"action":"msg","level":0,"msg":"…","raw_msg":"…"}
//! ```
//!
//! The numbers are Nix's own enums (`src/libutil/include/nix/util/logging.hh`),
//! read from the headers of the Nix 2.34.8 the system pins (its `-dev` output):
//! `actCopyPath = 100`, `actFileTransfer = 101`, `actRealise = 102`,
//! `actCopyPaths = 103`, `actBuilds = 104`, `actBuild = 105`,
//! `actSubstitute = 108`; `resProgress = 105`, `resSetExpected = 106`; and
//! `lvlError = 0`, `lvlWarn = 1`.
//!
//! # How it becomes one bar
//!
//! Exactly as Nix's own progress bar sums it (`src/libmain/progress-bar.cc`,
//! `getStatus`): per activity TYPE, the finished activities' `done`, plus
//! every live activity's `done` and `expected`, and the larger of that and
//! what `resSetExpected` announced for the type. A realisation announces its
//! totals up front on the `actRealise` activity (`[100, narBytes]`,
//! `[101, downloadBytes]`), which is what makes the bar honest from its first
//! byte instead of growing its own total as it goes.
//!
//! Of all that, ONE line is shown: a build when one runs (items), otherwise
//! the bytes copied into the store (a substitution streams the download into
//! the copy, so these move together and the copy's total is the unpacked
//! size Nix itself announces), otherwise bytes downloaded (a flake or a
//! tarball fetched with no copy around it). Before any of that, there is no
//! bar: evaluation has no total, and inventing one would be a lie.
//!
//! # Why no `-v`
//!
//! Measured on the pinned Nix: the activities arrive at the default
//! verbosity. `-v` adds only `evaluating file '…'` messages — 600 lines for
//! `hello`, before a single byte moves — so the argument vectors leave it out.
//!
//! # The errors
//!
//! A `msg` at level 0 is an error, and it carries `raw_msg`: the message
//! without the red `error:` prefix, the colours and the evaluation trace.
//! That is the trouble a failed job shows, and it is what the refusals are
//! read from ([`classify`]): the texts are nixpkgs' own (`check-meta.nix`) and
//! Nix's (`profile.cc`), captured from the pinned versions into
//! `tests/fixtures/nix/`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::progress::{Phase, Report};
use crate::{Failure, Refusal};

/// What starts every line of Nix's JSON log.
pub const PREFIX: &str = "@nix ";

/// `actCopyPath`: one store path being copied (a substitution's unpacking).
pub const ACT_COPY_PATH: u64 = 100;
/// `actFileTransfer`: one download.
pub const ACT_FILE_TRANSFER: u64 = 101;
/// `actRealise`: the whole realisation; it announces the byte totals.
pub const ACT_REALISE: u64 = 102;
/// `actCopyPaths`: the count of paths to substitute.
pub const ACT_COPY_PATHS: u64 = 103;
/// `actBuilds`: the count of derivations to build.
pub const ACT_BUILDS: u64 = 104;
/// `actBuild`: one derivation building.
pub const ACT_BUILD: u64 = 105;
/// `actSubstitute`: one path being fetched from a cache.
pub const ACT_SUBSTITUTE: u64 = 108;
/// `actQueryPathInfo`: asking a cache about a path (its `.narinfo`).
pub const ACT_QUERY_PATH_INFO: u64 = 109;
/// `resProgress`: `[done, expected, running, failed]`.
pub const RES_PROGRESS: u64 = 105;
/// `resSetExpected`: `[activityType, expected]`.
pub const RES_SET_EXPECTED: u64 = 106;
/// `lvlError`.
pub const LEVEL_ERROR: u64 = 0;
/// `lvlWarn`.
pub const LEVEL_WARN: u64 = 1;

/// How many of the program's own non-JSON lines are kept, for the trouble of
/// a failure that happened before Nix's logger was set up.
const PLAIN_KEPT: usize = crate::run::TAIL;

/// One record, as Nix writes it. Fields a record of another action does not
/// carry are absent and default.
#[derive(Debug, Deserialize)]
struct Record {
    action: String,
    #[serde(default)]
    id: u64,
    #[serde(default, rename = "type")]
    kind: u64,
    #[serde(default)]
    parent: u64,
    #[serde(default)]
    level: u64,
    #[serde(default)]
    fields: Vec<serde_json::Value>,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    raw_msg: Option<String>,
}

/// One live activity.
#[derive(Debug, Default)]
struct Act {
    kind: u64,
    parent: u64,
    done: u64,
    expected: u64,
    running: u64,
    expected_by_type: HashMap<u64, u64>,
    /// The store name it is about (`hello-2.12.3`), for the line's "now".
    name: Option<String>,
    /// When it started, in arrival order — the newest named one is "now".
    order: u64,
    /// A part of something larger, as Nix's own bar decides it at START
    /// (`progress-bar.cc`, `startActivity`): a download inside a copy or
    /// inside a cache query, a copy inside a substitution. Decided then and
    /// kept, because Nix may stop the parent before the child — the query
    /// about `hello` stops one line before its `.narinfo` download does.
    part: bool,
}

/// The finished activities of one type, and what was announced for it.
#[derive(Debug, Default, Clone, Copy)]
struct ByType {
    done: u64,
    expected: u64,
}

/// Summed numbers of one activity type.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Done so far.
    pub done: u64,
    /// Expected in all.
    pub expected: u64,
    /// Running now.
    pub running: u64,
}

/// What the line should say right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What is happening.
    pub phase: Phase,
    /// How much is behind.
    pub done: u64,
    /// How much there is.
    pub total: u64,
    /// The store name it is on, when it is on one.
    pub now: String,
}

/// Everything read so far from one Nix process.
#[derive(Debug, Default)]
pub struct Log {
    acts: HashMap<u64, Act>,
    by_type: HashMap<u64, ByType>,
    started: u64,
    errors: Vec<String>,
    warnings: Vec<String>,
    plain: Vec<String>,
}

impl Log {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Read one line of the process's output. `true` when it was one of Nix's
    /// JSON records; any other line is kept as plain text (Nix prints plain
    /// `error:` lines when it fails before its logger is set up).
    pub fn feed(&mut self, line: &str) -> bool {
        let Some(json) = line.strip_prefix(PREFIX) else {
            let text = strip_ansi(line.trim_end());
            if !text.trim().is_empty() {
                self.plain.push(text);
                if self.plain.len() > PLAIN_KEPT {
                    self.plain.remove(0);
                }
            }
            return false;
        };
        let Ok(r) = serde_json::from_str::<Record>(json) else {
            return false;
        };
        match r.action.as_str() {
            "start" => self.start(&r),
            "stop" => self.stop(r.id),
            "result" => self.result(&r),
            "msg" => self.message(&r),
            _ => {}
        }
        true
    }

    fn start(&mut self, r: &Record) {
        self.started += 1;
        let name = match r.kind {
            ACT_BUILD | ACT_SUBSTITUTE | ACT_COPY_PATH => r
                .fields
                .first()
                .and_then(serde_json::Value::as_str)
                .map(store_name),
            _ => None,
        };
        let part = match r.kind {
            ACT_FILE_TRANSFER => {
                self.has_ancestor(r.parent, ACT_COPY_PATH)
                    || self.has_ancestor(r.parent, ACT_QUERY_PATH_INFO)
            }
            ACT_COPY_PATH => self.has_ancestor(r.parent, ACT_SUBSTITUTE),
            _ => false,
        };
        self.acts.insert(
            r.id,
            Act {
                kind: r.kind,
                parent: r.parent,
                name,
                order: self.started,
                part,
                ..Act::default()
            },
        );
    }

    fn stop(&mut self, id: u64) {
        let Some(act) = self.acts.remove(&id) else {
            return;
        };
        let by = self.by_type.entry(act.kind).or_default();
        by.done = by.done.saturating_add(act.done);
        for (kind, expected) in act.expected_by_type {
            let by = self.by_type.entry(kind).or_default();
            by.expected = by.expected.saturating_sub(expected);
        }
    }

    fn result(&mut self, r: &Record) {
        let int = |i: usize| {
            r.fields
                .get(i)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        };
        match r.kind {
            RES_PROGRESS => {
                if let Some(act) = self.acts.get_mut(&r.id) {
                    act.done = int(0);
                    act.expected = int(1);
                    act.running = int(2);
                }
            }
            RES_SET_EXPECTED => {
                let (kind, expected) = (int(0), int(1));
                let Some(act) = self.acts.get_mut(&r.id) else {
                    return;
                };
                let before = act.expected_by_type.insert(kind, expected).unwrap_or(0);
                let by = self.by_type.entry(kind).or_default();
                by.expected = by.expected.saturating_sub(before).saturating_add(expected);
            }
            _ => {}
        }
    }

    fn message(&mut self, r: &Record) {
        // `raw_msg` is the message without the prefix, the colours and the
        // trace; a plain `log()` line has only `msg`.
        let text = r.raw_msg.clone().unwrap_or_else(|| strip_ansi(&r.msg));
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        match r.level {
            LEVEL_ERROR => self.errors.push(text),
            LEVEL_WARN => self.warnings.push(text),
            _ => {}
        }
    }

    /// The numbers of one activity type, summed as Nix's progress bar sums them.
    #[must_use]
    pub fn stats(&self, kind: u64) -> Stats {
        let by = self.by_type.get(&kind).copied().unwrap_or_default();
        let mut s = Stats {
            done: by.done,
            expected: by.done,
            running: 0,
        };
        for act in self.acts.values().filter(|a| a.kind == kind) {
            s.done = s.done.saturating_add(act.done);
            s.expected = s.expected.saturating_add(act.expected);
            s.running = s.running.saturating_add(act.running);
        }
        s.expected = s.expected.max(by.expected);
        s
    }

    /// The store name of the newest live activity that names one.
    fn now(&self, kinds: &[u64]) -> String {
        self.acts
            .values()
            .filter(|a| kinds.contains(&a.kind))
            .filter_map(|a| a.name.as_ref().map(|n| (a.order, n)))
            .max_by_key(|(order, _)| *order)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    }

    /// What the line should say, or `None` while nothing measurable has
    /// started (evaluation: no total to show).
    #[must_use]
    pub fn step(&self) -> Option<Step> {
        if self.acts.values().any(|a| a.kind == ACT_BUILD) {
            let builds = self.stats(ACT_BUILDS);
            return Some(Step {
                phase: Phase::Building,
                done: builds.done,
                total: builds.expected,
                now: self.now(&[ACT_BUILD]),
            });
        }
        let named = [ACT_SUBSTITUTE, ACT_COPY_PATH];
        let copy = self.stats(ACT_COPY_PATH);
        if copy.expected > 0 {
            return Some(Step {
                phase: Phase::Downloading,
                done: copy.done.min(copy.expected),
                total: copy.expected,
                now: self.now(&named),
            });
        }
        // A download that stands on its own (a flake, a tarball): not a
        // cache's `.narinfo`, and not the inside of a copy counted above.
        let transfer = self.stats(ACT_FILE_TRANSFER);
        (transfer.expected > 0
            && self
                .acts
                .values()
                .any(|a| a.kind == ACT_FILE_TRANSFER && !a.part))
        .then(|| Step {
            phase: Phase::Downloading,
            done: transfer.done.min(transfer.expected),
            total: transfer.expected,
            now: self.now(&named),
        })
    }

    /// Whether an activity or one of its live ancestors is of `kind`.
    fn has_ancestor(&self, mut id: u64, kind: u64) -> bool {
        // Bounded: a malformed log with a parent cycle must not hang a job.
        for _ in 0..64 {
            match self.acts.get(&id) {
                Some(a) if a.kind == kind => return true,
                Some(a) => id = a.parent,
                None => return false,
            }
        }
        false
    }

    /// The errors Nix reported, in its own words.
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The warnings Nix reported (`'hello' is already added`).
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The reason in the closed set, when the errors name one.
    #[must_use]
    pub fn refusal(&self) -> Option<Refusal> {
        self.errors
            .iter()
            .chain(&self.plain)
            .find_map(|e| classify(e))
    }

    /// The failure of a process that exited badly: its refusal when the words
    /// name one (with just the sentence that named it — the rest of nixpkgs'
    /// message tells the person to edit `configuration.nix`, which is not how
    /// consent works here), otherwise [`Refusal::ToolFailed`] with the errors.
    /// `tail` is what [`crate::run::Ran`] kept, for when Nix said nothing
    /// structured at all.
    #[must_use]
    pub fn failure(&self, tail: &[String]) -> Failure {
        if let Some(refusal) = self.refusal() {
            let said = self
                .errors
                .iter()
                .chain(&self.plain)
                .find(|e| classify(e) == Some(refusal))
                .map(|e| first_sentence(e))
                .unwrap_or_default();
            return Failure {
                refusal: Some(refusal),
                trouble: said,
            };
        }
        let trouble = if !self.errors.is_empty() {
            bounded(&self.errors.join("\n\n"))
        } else if !self.plain.is_empty() {
            self.plain.join("\n")
        } else {
            tail.iter()
                .filter(|l| !l.starts_with(PREFIX))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        };
        Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble,
        }
    }
}

/// The reason in the closed set that a text of Nix's or nixpkgs' names.
///
/// The texts, from the pinned versions (see `tests/fixtures/nix/`):
/// `… because it has an unfree license`, `… because it has problems:\n-
/// broken: …`, `… because it is marked as insecure`, `… because it is not
/// available on the requested hostPlatform`; `An existing package already
/// provides the following file` and `This is the conflicting file from the
/// new package` (`profile.cc`), `collision between …` (`buildenv.cc`); and
/// `profile '…' is incompatible with 'nix-env'`.
#[must_use]
pub fn classify(text: &str) -> Option<Refusal> {
    let t = text.to_ascii_lowercase();
    if t.contains("has an unfree license") {
        Some(Refusal::UnfreeNeedsConsent)
    } else if t.contains("- broken:") || t.contains("is marked as broken") {
        Some(Refusal::Broken)
    } else if t.contains("is marked as insecure") {
        Some(Refusal::Insecure)
    } else if t.contains("not available on the requested hostplatform") {
        Some(Refusal::Unavailable)
    } else if t.contains("incompatible with 'nix-env'") {
        Some(Refusal::ProfileIsNixEnv)
    } else if t.contains("already provides the following file")
        || t.contains("conflicting file")
        || t.contains("conflict for the following files")
        || t.contains("collision between")
    {
        Some(Refusal::Conflict)
    } else if t.contains("resolve host") {
        Some(Refusal::Offline)
    } else {
        None
    }
}

/// The first line of a message, trimmed — the sentence that says what
/// happened, before the advice.
fn first_sentence(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// A long message cut to the last [`crate::run::TAIL`] lines.
fn bounded(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.len().saturating_sub(crate::run::TAIL);
    lines[from..].join("\n")
}

/// The store name of a store path: `/nix/store/<hash>-hello-2.12.3` →
/// `hello-2.12.3`, and a derivation's `.drv` dropped.
#[must_use]
pub fn store_name(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    let name = base.split_once('-').map_or(base, |(_, rest)| rest);
    name.strip_suffix(".drv").unwrap_or(name).to_owned()
}

/// A text without its terminal colour codes (`ESC [ … final`).
#[must_use]
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            // Parameters and intermediates, then one final byte in `@`..`~`.
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        }
    }
    out
}

/// When to print a beat: Nix sends a `resProgress` for every chunk (1 716 of
/// them for a 1.5 MiB download, measured), and a beat per record would be a
/// pipe full of lines saying the same thing. A new phase, a new name or a new
/// total goes out at once; a moving count at most every [`Pace::EVERY`].
#[derive(Debug, Default)]
pub struct Pace {
    last: Option<(Step, Instant)>,
}

impl Pace {
    /// The shortest time between two beats that only move the count.
    pub const EVERY: Duration = Duration::from_millis(250);

    /// A fresh pace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `step`, arriving at `at`, is worth a beat.
    pub fn wants(&mut self, step: &Step, at: Instant) -> bool {
        let go = match &self.last {
            None => true,
            Some((last, when)) => {
                last.phase != step.phase
                    || last.now != step.now
                    || last.total != step.total
                    || (last.done != step.done
                        && (step.done >= step.total || at.duration_since(*when) >= Self::EVERY))
            }
        };
        if go {
            self.last = Some((step.clone(), at));
        }
        go
    }
}

/// A line handler for [`crate::run::Call::run`]: it feeds `log` and prints a
/// beat through `report` when the pace allows.
pub fn follow<'a>(
    log: &'a mut Log,
    pace: &'a mut Pace,
    report: &'a mut dyn Report,
) -> impl FnMut(&str) + 'a {
    move |line: &str| {
        if log.feed(line)
            && let Some(step) = log.step()
            && pace.wants(&step, Instant::now())
        {
            report.step(step.phase, step.done, step.total, &step.now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::Beat;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/nix")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn read(name: &str) -> Log {
        let mut log = Log::new();
        for line in fixture(name).lines() {
            assert!(log.feed(line), "not a record: {line}");
        }
        log
    }

    /// The real download of `hello` from the pinned nixpkgs, through the
    /// pinned Nix: the bar's total is the unpacked size Nix announced
    /// (279 624 bytes, "273.1 KiB unpacked") from the first byte, and it ends
    /// full.
    #[test]
    fn a_real_download_is_one_bar_whose_total_never_moves() {
        let text = fixture("build-hello.jsonl");
        let mut log = Log::new();
        let mut totals = std::collections::BTreeSet::new();
        let mut last = None;
        for line in text.lines() {
            log.feed(line);
            if let Some(step) = log.step() {
                assert_eq!(step.phase, Phase::Downloading);
                assert!(step.done <= step.total, "{step:?}");
                totals.insert(step.total);
                last = Some(step);
            }
        }
        assert_eq!(totals.into_iter().collect::<Vec<_>>(), [279_624]);
        let last = last.expect("some progress");
        assert_eq!(last.done, 279_624);
        assert!(log.errors().is_empty() && log.refusal().is_none());
    }

    #[test]
    fn the_name_on_the_line_is_the_path_being_fetched() {
        let text = fixture("build-hello.jsonl");
        let mut log = Log::new();
        let mut names = std::collections::BTreeSet::new();
        for line in text.lines() {
            log.feed(line);
            if let Some(s) = log.step() {
                names.insert(s.now);
            }
        }
        assert!(names.contains("hello-2.12.3"), "{names:?}");
    }

    #[test]
    fn nixpkgs_refusals_are_read_from_its_own_words() {
        for (file, refusal) in [
            ("refused-unfree.jsonl", Refusal::UnfreeNeedsConsent),
            ("refused-broken.jsonl", Refusal::Broken),
            ("refused-insecure.jsonl", Refusal::Insecure),
            ("refused-unavailable.jsonl", Refusal::Unavailable),
        ] {
            let log = read(file);
            assert_eq!(log.refusal(), Some(refusal), "{file}");
            let failure = log.failure(&[]);
            assert_eq!(failure.refusal, Some(refusal));
            // Just the sentence: no advice to edit configuration.nix, no colour.
            assert!(
                failure.trouble.starts_with("Refusing to evaluate package"),
                "{file}: {}",
                failure.trouble
            );
            assert!(!failure.trouble.contains('\n') && !failure.trouble.contains('\u{1b}'));
            assert!(!failure.trouble.contains("configuration.nix"));
        }
    }

    #[test]
    fn a_conflict_in_the_profile_is_a_conflict() {
        // Built from the format string of `profile.cc` in Nix 2.34.8 (a real
        // one needs two packages in a profile, and nothing here installs).
        let log = read("refused-conflict.jsonl");
        assert_eq!(log.refusal(), Some(Refusal::Conflict));
        assert!(log.failure(&[]).trouble.contains("already provides"));
    }

    #[test]
    fn an_error_nothing_names_is_the_tool_failing_in_its_own_words() {
        let mut log = Log::new();
        log.feed(r#"@nix {"action":"msg","level":0,"msg":"\u001b[31;1merror:\u001b[0m flake 'nixpkgs' does not provide attribute 'nope'","raw_msg":"flake 'nixpkgs' does not provide attribute 'nope'"}"#);
        log.feed(r#"@nix {"action":"msg","level":1,"msg":"warning: something"}"#);
        let f = log.failure(&[]);
        assert_eq!(f.refusal, Some(Refusal::ToolFailed));
        assert_eq!(
            f.trouble,
            "flake 'nixpkgs' does not provide attribute 'nope'"
        );
        assert_eq!(log.warnings(), ["warning: something"]);
        // Nothing structured at all: the plain lines, then the tail.
        let mut plain = Log::new();
        assert!(!plain.feed("\u{1b}[31;1merror:\u{1b}[0m unrecognised flag '--nope'"));
        assert_eq!(
            plain.failure(&[]).trouble,
            "error: unrecognised flag '--nope'"
        );
        let empty = Log::new();
        assert_eq!(
            empty.failure(&["@nix {}".into(), "died".into()]).trouble,
            "died"
        );
    }

    #[test]
    fn a_build_counts_items_and_names_the_derivation() {
        let mut log = Log::new();
        for line in [
            r#"@nix {"action":"start","id":1,"level":0,"parent":0,"text":"","type":104}"#,
            r#"@nix {"action":"result","id":1,"type":105,"fields":[0,2,1,0]}"#,
            r#"@nix {"action":"start","id":2,"level":3,"parent":0,"text":"building","type":105,"fields":["/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-figlet-2.2.5.drv","",1,1]}"#,
        ] {
            log.feed(line);
        }
        assert_eq!(
            log.step(),
            Some(Step {
                phase: Phase::Building,
                done: 0,
                total: 2,
                now: "figlet-2.2.5".into()
            })
        );
        log.feed(r#"@nix {"action":"stop","id":2}"#);
        log.feed(r#"@nix {"action":"result","id":1,"type":105,"fields":[1,2,0,0]}"#);
        // No build running: nothing to show, not a stale "building".
        assert_eq!(log.step(), None);
        assert_eq!(
            log.stats(ACT_BUILDS),
            Stats {
                done: 1,
                expected: 2,
                running: 0
            }
        );
    }

    #[test]
    fn an_announced_total_is_taken_back_when_its_activity_stops() {
        let mut log = Log::new();
        log.feed(r#"@nix {"action":"start","id":7,"level":0,"parent":0,"text":"","type":102}"#);
        log.feed(r#"@nix {"action":"result","id":7,"type":106,"fields":[100,1000]}"#);
        log.feed(r#"@nix {"action":"result","id":7,"type":106,"fields":[100,1500]}"#);
        assert_eq!(log.stats(ACT_COPY_PATH).expected, 1500, "set, not added");
        log.feed(r#"@nix {"action":"stop","id":7}"#);
        assert_eq!(log.stats(ACT_COPY_PATH).expected, 0);
    }

    #[test]
    fn colours_are_stripped_and_names_are_store_names() {
        assert_eq!(
            strip_ansi("\u{1b}[31;1merror:\u{1b}[0m x \u{1b}[35;1my\u{1b}[0m"),
            "error: x y"
        );
        assert_eq!(
            store_name("/nix/store/6i6xl6bmcpxqd51m8nlva40d5c1bhndx-hello-2.12.3"),
            "hello-2.12.3"
        );
        assert_eq!(
            store_name("/nix/store/6i6xl6bmcpxqd51m8nlva40d5c1bhndx-hello-2.12.3.drv"),
            "hello-2.12.3"
        );
    }

    #[test]
    fn the_pace_lets_news_through_and_holds_a_moving_count() {
        let t0 = Instant::now();
        let s = |done, total, now: &str| Step {
            phase: Phase::Downloading,
            done,
            total,
            now: now.into(),
        };
        let mut pace = Pace::new();
        assert!(pace.wants(&s(0, 100, "a"), t0));
        assert!(!pace.wants(&s(10, 100, "a"), t0 + Duration::from_millis(10)));
        assert!(
            pace.wants(&s(20, 100, "b"), t0 + Duration::from_millis(20)),
            "a new name"
        );
        assert!(
            pace.wants(&s(30, 100, "b"), t0 + Duration::from_millis(400)),
            "time passed"
        );
        assert!(
            pace.wants(&s(100, 100, "b"), t0 + Duration::from_millis(410)),
            "the end"
        );
    }

    #[test]
    fn following_a_real_log_prints_few_beats_and_the_last_is_full() {
        let text = fixture("build-hello.jsonl");
        let mut log = Log::new();
        let mut pace = Pace::new();
        let mut beats: Vec<Beat> = Vec::new();
        {
            let mut each = follow(&mut log, &mut pace, &mut beats);
            for line in text.lines() {
                each(line);
            }
        }
        assert!(
            !beats.is_empty() && beats.len() < 20,
            "{} beats",
            beats.len()
        );
        match beats.last() {
            Some(Beat::Step { done, total, .. }) => assert_eq!(done, total),
            other => panic!("{other:?}"),
        }
    }
}
