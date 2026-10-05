//! The systemd journal: one follower, entries, the matches that filter them,
//! and where the follower was left.
//!
//! # One follower for every module
//!
//! The brain's watch thread runs ONE `journalctl --follow --output=json`
//! ([`Follow`]) — by its store path ([`VARIABLE`]), at the loosest priority
//! any module registered ([`loosest`]), from the cursor it saved ([`Keeper`])
//! — and only when some module asked for a [`Match`]. Without `--user` it
//! reads the merged system and user journals (the person is in `wheel`, which
//! the journal's ACL lets read the system's). Each line becomes an [`Entry`];
//! each module's [`Match::accepts`] decides, in process, whether it is heard.
//!
//! # The past is a PASS, and the present is a follower
//!
//! On a first start (no saved cursor) a module may ask to hear some of what
//! already happened ([`Backfill`]). That is a separate `journalctl` that ends
//! ([`pass_arguments`]): read to its end, it is the whole of the past at once,
//! and each module keeps the last few that it accepts ([`Shares`]). A follower
//! started with `--lines=N` would mix the past into the present with nothing
//! marking where one stops, and a module that asked for the last fifty would
//! get the first fifty of the window.
//!
//! **The pass is never cut with `--lines`.** journalctl counts those lines
//! BEFORE any module's filter, which it cannot apply (a field, a piece of the
//! message): the last twenty lines of a boot hold seven `Started`, out of five
//! thousand, and a rare match — an OOM kill, a refused packet — usually none.
//! The pass reads its whole window, and each match keeps a ring of its last
//! `most` as the entries go by, so memory is what the matches asked for and
//! never the window.
//!
//! **The window is every window asked for, joined** ([`widest`]): this boot,
//! the longest `since`, or — when both are asked — from whichever began first,
//! which needs to know when this boot began ([`probe_arguments`]). Each match
//! then takes only its own window out of it ([`Backfill::covers`]): a `since`
//! reaches across a reboot, and this boot is told apart by `_BOOT_ID`.
//!
//! # A journal it cannot read is said
//!
//! A follower that cannot start, cannot read, or ends is a [`Refusal`] — a
//! closed set of words the module's page draws as a sentence — never silence.
//! What journalctl itself said about it is kept too ([`Follow::said`]), for the
//! log.
//!
//! # Pure where it can be
//!
//! [`Entry::parse`], [`Match::accepts`], [`follow_arguments`],
//! [`pass_arguments`], [`probe_arguments`], [`loosest`], [`widest`],
//! [`Shares`] and [`read_entry`] read nothing: a test hands them lines and
//! values.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The variable the brain's unit sets to journalctl's store path.
pub const VARIABLE: &str = "RONOCTUA_JOURNALCTL";

/// A syslog priority: 0 (emergency) to 7 (debug). The smaller, the worse.
pub type Priority = u8;

/// `err`: the priority a watch on the machine's errors reads from.
pub const ERR: Priority = 3;
/// `warning`.
pub const WARNING: Priority = 4;
/// `info`: what journald gives a message that names none.
pub const INFO: Priority = 6;
/// `debug`: the loosest there is.
pub const DEBUG: Priority = 7;

/// The longest line read from journalctl, in bytes. An entry with a larger
/// field (a core dump's binary blob) is skipped, not truncated into a lie.
pub const LINE_MOST_BYTES: usize = 256 * 1024;

/// The field journald stamps on every entry with the boot it was written in.
pub const BOOT_ID: &str = "_BOOT_ID";

/// Where the journal was left: journald's opaque cursor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Cursor(pub String);

impl Cursor {
    /// The cursor kept in a file, or `None` when there is none — no file, an
    /// empty one, or one that is not a single line of text.
    ///
    /// A cursor is one line journald wrote; anything else in the file is not
    /// one, and handing it to `--after-cursor` would be a follower that refuses
    /// to start. `None` is the first-start answer, which is always safe.
    #[must_use]
    pub fn read_from(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        let line = raw.trim();
        (!line.is_empty() && !line.contains('\n')).then(|| Self(line.to_owned()))
    }

    /// Writes it into `dir/file`, atomically — a cursor cut in half by a kill
    /// would be a follower that cannot start the next time.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn write_to(&self, dir: &Path, file: &str) -> std::io::Result<()> {
        let mut line = self.0.clone();
        line.push('\n');
        ronoctua_xdg::write_atomically(dir, file, line.as_bytes())
    }
}

/// One journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// Every field, by name. A field that repeats keeps its first value; a
    /// binary field is read as lossy UTF-8.
    pub fields: BTreeMap<String, String>,
    /// `MESSAGE` (empty when absent).
    pub message: String,
    /// `PRIORITY`, when present and a number.
    pub priority: Option<Priority>,
    /// The unit it came from: `_SYSTEMD_USER_UNIT` for a user unit, else
    /// `_SYSTEMD_UNIT`.
    pub unit: Option<String>,
    /// `__CURSOR`: where this entry is.
    pub cursor: Cursor,
    /// `__REALTIME_TIMESTAMP`: when it was written, in microseconds since the
    /// epoch.
    pub realtime_us: u64,
}

impl Entry {
    /// The entry in one line of `journalctl --output=json`; `None` for a line
    /// that is not one (no object, or no `__CURSOR`).
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let object: serde_json::Map<String, serde_json::Value> = serde_json::from_str(line).ok()?;
        let mut fields = BTreeMap::new();
        for (name, value) in object {
            if let Some(text) = text_of(&value) {
                fields.insert(name, text);
            }
        }
        let cursor = Cursor(fields.get("__CURSOR")?.clone());
        let realtime_us = fields
            .get("__REALTIME_TIMESTAMP")
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        let priority = fields.get("PRIORITY").and_then(|p| p.parse().ok());
        let unit = fields
            .get("_SYSTEMD_USER_UNIT")
            .or_else(|| fields.get("_SYSTEMD_UNIT"))
            .cloned();
        let message = fields.get("MESSAGE").cloned().unwrap_or_default();
        Some(Self {
            fields,
            message,
            priority,
            unit,
            cursor,
            realtime_us,
        })
    }
}

/// A field's value as text: a string as it is, a byte array (journald's form
/// for non-UTF-8 or binary) as lossy UTF-8, a repeated field by its first.
fn text_of(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(items) if items.iter().all(serde_json::Value::is_u64) => {
            let bytes: Vec<u8> = items
                .iter()
                .filter_map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                .collect();
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
        serde_json::Value::Array(items) => items.first().and_then(text_of),
        _ => None,
    }
}

/// One field a match requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Field {
    /// The field's name (`_TRANSPORT`, `_SYSTEMD_UNIT`, `SYSLOG_IDENTIFIER`).
    pub name: String,
    /// The values it may have — any one of them (journalctl's rule for a
    /// field named twice). Empty: the field only has to be present.
    pub any_of: Vec<String>,
}

/// What a module hears of the journal before its match starts, on the first
/// start (on later starts, the saved cursor decides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Backfill {
    /// Nothing old: only what arrives from now on.
    #[default]
    Nothing,
    /// This boot's entries, the last `most` of them.
    ThisBoot {
        /// At most this many.
        most: usize,
    },
    /// The entries of the last `seconds`, the last `most` of them.
    Since {
        /// How far back, in seconds.
        seconds: u64,
        /// At most this many.
        most: usize,
    },
}

impl Backfill {
    /// How many old entries it keeps, at most. `0` for `Nothing`.
    #[must_use]
    pub const fn most(self) -> usize {
        match self {
            Self::Nothing => 0,
            Self::ThisBoot { most } | Self::Since { most, .. } => most,
        }
    }

    /// Whether an entry of the PASS falls inside this backfill's own window.
    ///
    /// `began_us` is when the pass began. `boot` is this boot's `_BOOT_ID`
    /// when the pass reached further back than this boot (both kinds were
    /// asked for — [`Window::needs_the_boot`]); `None` when the pass read this
    /// boot alone, and then `ThisBoot` takes all of it.
    #[must_use]
    pub fn covers(self, entry: &Entry, began_us: u64, boot: Option<&str>) -> bool {
        match self {
            Self::Nothing => false,
            Self::ThisBoot { .. } => {
                boot.is_none_or(|b| entry.fields.get(BOOT_ID).is_some_and(|e| e == b))
            }
            Self::Since { seconds, .. } => {
                entry
                    .realtime_us
                    .saturating_add(seconds.saturating_mul(1_000_000))
                    >= began_us
            }
        }
    }
}

/// What a module wants to hear: every condition must hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Match {
    /// Only entries at this priority or worse (a smaller number). An entry
    /// without `PRIORITY` counts as [`INFO`].
    pub max_priority: Option<Priority>,
    /// Fields that must be present with one of their values (AND across
    /// fields, OR within one).
    pub fields: Vec<Field>,
    /// Text the message must contain (as written, case included).
    pub message_contains: Option<String>,
    /// What it hears of the past on the first start.
    pub backfill: Backfill,
}

impl Match {
    /// Whether this entry is one the module wants. Pure.
    #[must_use]
    pub fn accepts(&self, entry: &Entry) -> bool {
        let priority_fits = self
            .max_priority
            .is_none_or(|most| entry.priority.unwrap_or(INFO) <= most);
        let fields_fit = self.fields.iter().all(|field| {
            entry.fields.get(&field.name).is_some_and(|value| {
                field.any_of.is_empty() || field.any_of.iter().any(|v| v == value)
            })
        });
        let message_fits = self
            .message_contains
            .as_deref()
            .is_none_or(|needle| entry.message.contains(needle));
        priority_fits && fields_fit && message_fits
    }
}

/// The priority the ONE follower reads at: the loosest any match asked for.
///
/// `None` is every priority — what a match with no `max_priority` needs, and
/// so what the follower must read the moment one such match exists. The
/// filter in process ([`Match::accepts`]) is what narrows it back down per
/// module; journalctl's `--priority` only keeps the pipe from carrying lines
/// nobody wants. An empty set of matches is also `None`, but nobody starts a
/// follower for no match.
#[must_use]
pub fn loosest<'a>(matches: impl IntoIterator<Item = &'a Match>) -> Option<Priority> {
    let mut out: Option<Priority> = Some(0);
    for m in matches {
        match (out, m.max_priority) {
            (Some(now), Some(p)) => out = Some(now.max(p)),
            _ => return None,
        }
    }
    out.filter(|p| *p < DEBUG)
}

/// What the one pass over the past reads: every window asked for, joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Window {
    /// Some match wants the whole of this boot.
    pub boot: bool,
    /// The longest `Since` asked for, in seconds; `0` when none was.
    pub seconds: u64,
}

impl Window {
    /// Whether the pass must first learn when this boot began: both kinds
    /// were asked for, and the pass starts at whichever began first.
    #[must_use]
    pub const fn needs_the_boot(self) -> bool {
        self.boot && self.seconds > 0
    }
}

/// The one window that covers every backfill asked for; `None` when nobody
/// wants anything old (every backfill `Nothing`, or a `most` of zero).
///
/// THIS BOOT AND A `since` ARE JOINED, not ranked: a week on a machine booted
/// this morning reaches past the boot, and a boot ten days old reaches past
/// the week. Each match then takes its own share of the one pass
/// ([`Backfill::covers`], [`Shares`]).
#[must_use]
pub fn widest(backfills: impl IntoIterator<Item = Backfill>) -> Option<Window> {
    let mut window = Window::default();
    for b in backfills {
        match b {
            Backfill::ThisBoot { most } if most > 0 => window.boot = true,
            Backfill::Since { seconds, most } if most > 0 => {
                window.seconds = window.seconds.max(seconds);
            }
            _ => {}
        }
    }
    (window.boot || window.seconds > 0).then_some(window)
}

/// Why the follower is not following, as a closed set
/// (`brain-journal-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    /// [`VARIABLE`] is not set: this process was not given journalctl.
    NoProgram,
    /// journalctl could not be started.
    Unstartable,
    /// journalctl ran and gave up before a single entry: it could not open
    /// the journal, or it refused what it was asked (a cursor it no longer
    /// knows). What it said is [`Follow::said`].
    Unreadable,
    /// journalctl ended (or its output could not be read) after it had been
    /// following.
    Ended,
}

impl Refusal {
    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoProgram => "no-program",
            Self::Unstartable => "unstartable",
            Self::Unreadable => "unreadable",
            Self::Ended => "ended",
        }
    }
}

/// journalctl's store path, from [`VARIABLE`]; `None` when the unit did not
/// set it (never a bare name: a unit's `PATH` is minimal).
#[must_use]
pub fn program() -> Option<PathBuf> {
    std::env::var_os(VARIABLE)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The arguments every invocation shares: JSON, one object a line, no pager.
fn common(loosest: Option<Priority>) -> Vec<String> {
    let mut args = vec!["--output=json".to_owned(), "--no-pager".to_owned()];
    if let Some(p) = loosest {
        args.push(format!("--priority=0..{p}"));
    }
    args
}

/// The arguments of the one follower. Pure.
///
/// From `after` when there is a saved cursor — and then EVERY line after it,
/// because `--follow` otherwise implies the last ten and a brain that was off
/// for an hour would hear ten of that hour. Otherwise `backfill` decides what
/// old lines come first (`Nothing` is `--lines=0`). `loosest` is the loosest
/// priority any module asked for (`None`: every priority).
///
/// The watch thread hands the past in through a PASS ([`pass_arguments`]) and
/// then starts this from the pass's last cursor, so `backfill` here is only
/// for a caller that wants both in one stream — and its `most` is counted by
/// journalctl BEFORE any filter in process, so it is exact only for a match
/// that journalctl applies whole (a priority and nothing else). See this
/// crate's header.
#[must_use]
pub fn follow_arguments(
    after: Option<&Cursor>,
    loosest: Option<Priority>,
    backfill: Backfill,
) -> Vec<String> {
    let mut args = vec!["--follow".to_owned()];
    args.extend(common(loosest));
    match (after, backfill) {
        (Some(cursor), _) => {
            args.push(format!("--after-cursor={}", cursor.0));
            args.push("--lines=all".to_owned());
        }
        (None, Backfill::Nothing) => args.push("--lines=0".to_owned()),
        (None, Backfill::ThisBoot { most }) => {
            args.push("--boot".to_owned());
            args.push(format!("--lines={most}"));
        }
        (None, Backfill::Since { seconds, most }) => {
            args.push(format!("--since=-{seconds}s"));
            args.push(format!("--lines={most}"));
        }
    }
    args
}

/// The arguments of the PASS over the past — a journalctl that ENDS. Pure.
///
/// The WHOLE window and no `--lines`: see this crate's header. `loosest` is
/// the loosest priority of the matches that want a past; `boot_began_s` is
/// when this boot's first entry was written ([`probe_arguments`]), wanted
/// only when [`Window::needs_the_boot`] — without it the pass starts where
/// the `since` does, and a boot older than that is read from there. `now_s`
/// is now, in seconds since the epoch.
#[must_use]
pub fn pass_arguments(
    loosest: Option<Priority>,
    window: Window,
    boot_began_s: Option<u64>,
    now_s: u64,
) -> Vec<String> {
    let mut args = common(loosest);
    if window.needs_the_boot() {
        // FROM WHICHEVER BEGAN FIRST: `--boot` and `--since` together would
        // be what both allow, and each match wants what either allows.
        let since = now_s.saturating_sub(window.seconds);
        let from = boot_began_s.map_or(since, |b| b.min(since));
        args.push(format!("--since=@{from}"));
    } else if window.boot {
        args.push("--boot".to_owned());
    } else {
        args.push(format!("--since=-{}s", window.seconds));
    }
    args
}

/// The arguments that read when this boot began: its first entry, whatever
/// its priority. The caller reads ONE entry and drops the stream, which stops
/// journalctl. Pure.
#[must_use]
pub fn probe_arguments() -> Vec<String> {
    let mut args = common(None);
    args.push("--boot".to_owned());
    args
}

/// Each match's share of the one pass over the past, kept AS THE PASS IS
/// READ: the last `most` entries it accepts inside its own window, and never
/// more than that in memory — however long the window.
#[derive(Debug)]
pub struct Shares {
    wanted: Vec<Match>,
    kept: Vec<VecDeque<Entry>>,
    began_us: u64,
    boot: Option<String>,
    last: Option<Cursor>,
}

impl Shares {
    /// Shares for these matches, of a pass that began at `began_us`; `boot`
    /// as [`Backfill::covers`] takes it.
    #[must_use]
    pub fn new(wanted: Vec<Match>, began_us: u64, boot: Option<String>) -> Self {
        let kept = wanted.iter().map(|_| VecDeque::new()).collect();
        Self {
            wanted,
            kept,
            began_us,
            boot,
            last: None,
        }
    }

    /// One entry of the pass, offered to every match.
    pub fn offer(&mut self, entry: &Entry) {
        for (m, kept) in self.wanted.iter().zip(&mut self.kept) {
            let most = m.backfill.most();
            if most == 0
                || !m
                    .backfill
                    .covers(entry, self.began_us, self.boot.as_deref())
                || !m.accepts(entry)
            {
                continue;
            }
            if kept.len() == most {
                kept.pop_front();
            }
            kept.push_back(entry.clone());
        }
        self.last = Some(entry.cursor.clone());
    }

    /// Each match's share, in the order the matches were given, oldest
    /// first — and the cursor of the last entry READ, taken or not, which is
    /// where the follower goes on from.
    #[must_use]
    pub fn taken(self) -> (Vec<Vec<Entry>>, Option<Cursor>) {
        (self.kept.into_iter().map(Vec::from).collect(), self.last)
    }
}

/// Reads the next entry off a stream of journalctl's JSON lines.
///
/// `Ok(None)` at the end of the stream. A line that is not an entry, or that
/// is longer than [`LINE_MOST_BYTES`], is SKIPPED — never cut into a shorter
/// entry that says something the journal did not.
///
/// # Errors
///
/// The stream could not be read.
pub fn read_entry(from: &mut impl BufRead) -> std::io::Result<Option<Entry>> {
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = from
            .take(LINE_MOST_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            return Ok(None);
        }
        if line.last() != Some(&b'\n') && read > LINE_MOST_BYTES {
            // Too long: drop the rest of it, then go on.
            let mut rest = Vec::new();
            from.read_until(b'\n', &mut rest)?;
            continue;
        }
        if let Some(entry) = std::str::from_utf8(&line).ok().and_then(Entry::parse) {
            return Ok(Some(entry));
        }
    }
}

/// A running journalctl — the follower, or a pass. Dropping it stops it.
#[derive(Debug)]
pub struct Follow {
    child: Child,
    lines: BufReader<ChildStdout>,
    /// The last line journalctl wrote on its standard error, kept by a thread
    /// that drains it: a pipe nobody reads fills up, and a journalctl blocked
    /// on it would stop writing entries too.
    said: Arc<Mutex<String>>,
    /// Whether an entry has come out of it yet — what tells "it could not
    /// read the journal" from "it read, and then it ended".
    heard: bool,
}

impl Follow {
    /// Starts the follower at `program` with [`follow_arguments`].
    ///
    /// # Errors
    ///
    /// [`Refusal::Unstartable`].
    pub fn start(
        program: &Path,
        after: Option<&Cursor>,
        loosest: Option<Priority>,
        backfill: Backfill,
    ) -> Result<Self, Refusal> {
        Self::spawn(program, &follow_arguments(after, loosest, backfill))
    }

    /// Starts journalctl at `program` with these arguments — a follower, or a
    /// pass ([`pass_arguments`]).
    ///
    /// # Errors
    ///
    /// [`Refusal::Unstartable`].
    pub fn spawn(program: &Path, args: &[String]) -> Result<Self, Refusal> {
        let mut child = Command::new(program)
            .args(args)
            .env("SYSTEMD_COLORS", "0")
            .env("SYSTEMD_PAGER", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| Refusal::Unstartable)?;
        let (Some(out), Some(err)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Refusal::Unstartable);
        };
        let said = Arc::new(Mutex::new(String::new()));
        let keeping = Arc::clone(&said);
        // A NAMED THREAD, and a short-lived one: it ends when journalctl
        // closes its standard error, which is when it exits.
        let drained = std::thread::Builder::new()
            .name("journal-said".to_owned())
            .spawn(move || {
                for line in BufReader::new(err).lines() {
                    let Ok(line) = line else { break };
                    let line = line.trim();
                    if !line.is_empty()
                        && let Ok(mut last) = keeping.lock()
                    {
                        line.clone_into(&mut last);
                    }
                }
            });
        if drained.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Refusal::Unstartable);
        }
        Ok(Self {
            child,
            lines: BufReader::new(out),
            said,
            heard: false,
        })
    }

    /// The next entry. Blocks until one arrives: call it on the watch thread.
    /// A line that is not an entry, or longer than [`LINE_MOST_BYTES`], is
    /// skipped.
    ///
    /// # Errors
    ///
    /// At the end of journalctl's output: [`Refusal::Unreadable`] when it
    /// gave no entry at all and exited with a failure, [`Refusal::Ended`]
    /// otherwise — which for a PASS is simply its end.
    pub fn next_entry(&mut self) -> Result<Entry, Refusal> {
        match read_entry(&mut self.lines) {
            Ok(Some(entry)) => {
                self.heard = true;
                Ok(entry)
            }
            Ok(None) | Err(_) => {
                let failed = self.child.wait().is_ok_and(|status| !status.success());
                Err(if failed && !self.heard {
                    Refusal::Unreadable
                } else {
                    Refusal::Ended
                })
            }
        }
    }

    /// The last thing journalctl said on its standard error, if anything —
    /// in its own words, for the log.
    #[must_use]
    pub fn said(&self) -> String {
        self.said.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Whether journalctl is still running `within` after it started — what
    /// tells a follower that TOOK from one that gives up at once (a journal
    /// it may not open, a cursor it will not seek to), before a single entry
    /// says so. A follower on a quiet journal writes nothing for hours, and
    /// "not followed" must not be what the page says about it meanwhile.
    ///
    /// It only looks at the process, never at the pipe: what journalctl
    /// wrote in the meantime waits there for [`Follow::next_entry`]. `false`
    /// the moment it has exited.
    pub fn settled(&mut self, within: Duration) -> bool {
        settles(&mut self.child, within)
    }
}

/// Whether a child is still running `within` after now: its exit status is
/// looked at every few milliseconds, and nothing is read. `false` as soon as
/// it has exited — its status is then kept by the `Child`, so a `wait` after
/// this gets it back.
pub fn settles(child: &mut Child, within: Duration) -> bool {
    let until = Instant::now() + within;
    loop {
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => return false,
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        std::thread::sleep(left.min(Duration::from_millis(20)));
    }
}

impl Drop for Follow {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Where the follower was left, kept on disk — and written at most once per
/// `every`, however fast the journal moves.
///
/// # Why debounced
///
/// Because a cursor per entry is a write per journal line, and a machine that
/// logs a burst would have the brain rewriting a file hundreds of times a
/// second for a position nobody needs to that precision. What a restart loses
/// by the debounce is at most `every` of entries HEARD AGAIN, never missed:
/// the cursor on disk is always one the follower really passed.
///
/// # Why no clock of its own
///
/// [`Keeper::due`] says when the pending cursor should be written, and the
/// watch thread arms its one wait for that moment only while there is one
/// pending. A journal that says nothing costs nothing.
#[derive(Debug)]
pub struct Keeper {
    dir: Option<PathBuf>,
    file: String,
    every: Duration,
    pending: Option<Cursor>,
    written: Option<Instant>,
    /// When it was made: a cursor noted before anything was ever written is
    /// due from this moment, which is always already past.
    born: Instant,
}

impl Keeper {
    /// One keeping `dir/file`; `None` for `dir` keeps nothing anywhere, which
    /// is what a test hands in.
    #[must_use]
    pub fn at(dir: Option<PathBuf>, file: &str, every: Duration) -> Self {
        Self {
            dir,
            file: file.to_owned(),
            every,
            pending: None,
            written: None,
            born: Instant::now(),
        }
    }

    /// The cursor saved last time, if any.
    #[must_use]
    pub fn saved(&self) -> Option<Cursor> {
        self.dir
            .as_deref()
            .and_then(|d| Cursor::read_from(&d.join(&self.file)))
    }

    /// The follower passed this entry.
    pub fn note(&mut self, cursor: Cursor) {
        self.pending = Some(cursor);
    }

    /// When the pending cursor should be written; `None` when nothing is
    /// pending, which arms no clock at all.
    #[must_use]
    pub fn due(&self) -> Option<Instant> {
        self.pending.as_ref()?;
        Some(self.written.map_or(self.born, |at| at + self.every))
    }

    /// Writes the pending cursor now, if there is one.
    ///
    /// A write that FAILED counts as a write for the debounce: the cursor stays
    /// pending and is tried again `every` later — never at once, which on a
    /// disk that refuses would be a watch thread spinning on a clock that is
    /// always due.
    ///
    /// # Errors
    ///
    /// The file could not be written; the cursor stays pending.
    pub fn flush(&mut self) -> std::io::Result<()> {
        let Some(cursor) = self.pending.take() else {
            return Ok(());
        };
        self.written = Some(Instant::now());
        if let Some(dir) = self.dir.as_deref()
            && let Err(e) = cursor.write_to(dir, &self.file)
        {
            self.pending = Some(cursor);
            return Err(e);
        }
        Ok(())
    }

    /// Writes the pending cursor if its moment has come. `true` when it wrote.
    ///
    /// # Errors
    ///
    /// As [`Keeper::flush`].
    pub fn flush_if_due(&mut self, now: Instant) -> std::io::Result<bool> {
        if self.due().is_some_and(|at| at <= now) {
            self.flush()?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Forgets the saved cursor — journalctl refused it, and a follower that
    /// kept asking for it would never start again.
    pub fn forget(&mut self) {
        self.pending = None;
        if let Some(dir) = self.dir.as_deref() {
            let _ = std::fs::remove_file(dir.join(&self.file));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KERNEL: &str = r#"{"__CURSOR":"s=abc;i=1","__REALTIME_TIMESTAMP":"1790000000000000","PRIORITY":"4","_TRANSPORT":"kernel","MESSAGE":"ronoctua-refused: IN=wlan0 SRC=10.0.0.9 PROTO=TCP DPT=22","SYSLOG_IDENTIFIER":["kernel","dup"]}"#;
    const UNIT: &str = r#"{"__CURSOR":"s=abc;i=2","__REALTIME_TIMESTAMP":"1790000000000001","PRIORITY":"3","_SYSTEMD_UNIT":"user@1000.service","_SYSTEMD_USER_UNIT":"ronoctua-player.service","MESSAGE":[98,114,111,107,101,110]}"#;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-journal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn a_line_becomes_an_entry() {
        let kernel = Entry::parse(KERNEL).unwrap();
        assert_eq!(kernel.priority, Some(4));
        assert_eq!(kernel.realtime_us, 1_790_000_000_000_000);
        assert_eq!(kernel.cursor, Cursor("s=abc;i=1".to_owned()));
        assert_eq!(
            kernel.fields["SYSLOG_IDENTIFIER"], "kernel",
            "a repeated field keeps its first"
        );
        let unit = Entry::parse(UNIT).unwrap();
        assert_eq!(unit.message, "broken", "a byte array is text");
        assert_eq!(
            unit.unit.as_deref(),
            Some("ronoctua-player.service"),
            "the user unit wins"
        );
        assert!(Entry::parse(r#"{"MESSAGE":"no cursor"}"#).is_none());
        assert!(Entry::parse("not json").is_none());
    }

    #[test]
    fn a_match_accepts_only_what_it_names() {
        let kernel = Entry::parse(KERNEL).unwrap();
        let unit = Entry::parse(UNIT).unwrap();
        let refused = Match {
            fields: vec![Field {
                name: "_TRANSPORT".to_owned(),
                any_of: vec!["kernel".to_owned()],
            }],
            message_contains: Some("ronoctua-refused: ".to_owned()),
            ..Match::default()
        };
        assert!(refused.accepts(&kernel));
        assert!(!refused.accepts(&unit), "no _TRANSPORT field");

        let errors = Match {
            max_priority: Some(ERR),
            ..Match::default()
        };
        assert!(errors.accepts(&unit), "3 is err");
        assert!(!errors.accepts(&kernel), "4 is only a warning");

        let either = Match {
            fields: vec![Field {
                name: "_SYSTEMD_USER_UNIT".to_owned(),
                any_of: vec![
                    "nftables.service".to_owned(),
                    "ronoctua-player.service".to_owned(),
                ],
            }],
            ..Match::default()
        };
        assert!(either.accepts(&unit), "OR within a field");
        let present = Match {
            fields: vec![Field {
                name: "_SYSTEMD_USER_UNIT".to_owned(),
                any_of: Vec::new(),
            }],
            ..Match::default()
        };
        assert!(
            present.accepts(&unit) && !present.accepts(&kernel),
            "empty: present"
        );

        let mut quiet = kernel;
        quiet.priority = None;
        let warnings = Match {
            max_priority: Some(WARNING),
            ..Match::default()
        };
        assert!(!warnings.accepts(&quiet), "no PRIORITY counts as info");
        assert!(
            Match::default().accepts(&quiet),
            "an empty match takes everything"
        );
    }

    #[test]
    fn the_follower_starts_where_it_was_left() {
        let cursor = Cursor("s=abc;i=9".to_owned());
        let args = follow_arguments(Some(&cursor), Some(ERR), Backfill::ThisBoot { most: 50 });
        // EVERY LINE AFTER THE CURSOR: `--follow` alone would give the last
        // ten, and a brain that was off for an hour would hear ten of it.
        assert_eq!(
            args,
            [
                "--follow",
                "--output=json",
                "--no-pager",
                "--priority=0..3",
                "--after-cursor=s=abc;i=9",
                "--lines=all",
            ]
        );
        assert_eq!(
            follow_arguments(None, None, Backfill::Nothing)
                .last()
                .unwrap(),
            "--lines=0"
        );
        assert_eq!(
            follow_arguments(
                None,
                None,
                Backfill::Since {
                    seconds: 604_800,
                    most: 500
                }
            )[3..],
            ["--since=-604800s", "--lines=500"]
        );
    }

    #[test]
    fn the_past_is_a_pass_that_ends_and_is_never_cut_before_the_filter() {
        // NO `--follow` in a pass: it is read to its end, and its end is the
        // whole of the past. AND NO `--lines`: journalctl would count them
        // before the match, and a rare match would get nothing.
        let boot = Window {
            boot: true,
            seconds: 0,
        };
        let pass = pass_arguments(Some(WARNING), boot, None, 1_000_000);
        assert!(
            !pass
                .iter()
                .any(|a| a == "--follow" || a.starts_with("--lines")),
            "{pass:?}"
        );
        assert_eq!(pass[2..], ["--priority=0..4", "--boot"]);
        let week = Window {
            boot: false,
            seconds: 604_800,
        };
        assert_eq!(
            pass_arguments(None, week, None, 1_000_000)[2..],
            ["--since=-604800s"]
        );
        // BOTH: from whichever began first — the week, on a machine booted
        // this morning; the boot, on one up for ten days.
        let both = Window {
            boot: true,
            seconds: 604_800,
        };
        assert!(both.needs_the_boot() && !boot.needs_the_boot());
        let now = 1_790_000_000;
        assert_eq!(
            pass_arguments(None, both, Some(now - 3_600), now)[2..],
            [format!("--since=@{}", now - 604_800)]
        );
        assert_eq!(
            pass_arguments(None, both, Some(now - 864_000), now)[2..],
            [format!("--since=@{}", now - 864_000)]
        );
        // The probe reads this boot from its start, at every priority.
        assert_eq!(probe_arguments(), ["--output=json", "--no-pager", "--boot"]);
    }

    #[test]
    fn the_loosest_priority_and_the_widest_past_cover_everybody() {
        let at = |p: Option<Priority>| Match {
            max_priority: p,
            ..Match::default()
        };
        assert_eq!(loosest(&[at(Some(ERR)), at(Some(WARNING))]), Some(WARNING));
        // ONE MATCH THAT TAKES EVERYTHING is a follower that reads everything.
        assert_eq!(loosest(&[at(Some(ERR)), at(None)]), None);
        // And debug IS everything: `--priority=0..7` says nothing.
        assert_eq!(loosest(&[at(Some(DEBUG))]), None);

        assert_eq!(widest([Backfill::Nothing, Backfill::Nothing]), None);
        assert_eq!(widest([Backfill::ThisBoot { most: 0 }]), None);
        // THIS BOOT AND A `since` ARE JOINED, not ranked: neither is wider
        // than the other on every machine.
        assert_eq!(
            widest([
                Backfill::Since {
                    seconds: 60,
                    most: 5
                },
                Backfill::ThisBoot { most: 3 }
            ]),
            Some(Window {
                boot: true,
                seconds: 60
            })
        );
        assert_eq!(
            widest([
                Backfill::Since {
                    seconds: 60,
                    most: 5
                },
                Backfill::Since {
                    seconds: 600,
                    most: 1
                }
            ]),
            Some(Window {
                boot: false,
                seconds: 600
            })
        );
        // Each module then takes only its own window of the pass.
        let now: u64 = 1_790_000_000_000_000;
        let at_us = |us: u64, boot: &str| Entry {
            realtime_us: us,
            fields: BTreeMap::from([(BOOT_ID.to_owned(), boot.to_owned())]),
            ..Entry::default()
        };
        let an_hour = Backfill::Since {
            seconds: 3600,
            most: 9,
        };
        assert!(an_hour.covers(&at_us(now - 3_600_000_000, "old"), now, Some("new")));
        assert!(!an_hour.covers(&at_us(now - 3_600_000_001, "new"), now, Some("new")));
        assert!(!Backfill::Nothing.covers(&at_us(now, "new"), now, None));
        let this_boot = Backfill::ThisBoot { most: 1 };
        assert!(
            this_boot.covers(&at_us(0, "old"), now, None),
            "a pass of this boot alone"
        );
        assert!(!this_boot.covers(&at_us(now, "old"), now, Some("new")));
        assert!(this_boot.covers(&at_us(0, "new"), now, Some("new")));
    }

    #[test]
    fn each_match_keeps_the_last_it_accepts_not_the_last_lines() {
        // THE DEFECT THIS CATCHES: `--lines=2` on the pass, and then the
        // filter — a module asking for its last two kills got the one kill
        // among the last two lines of the boot.
        let line = |n: u64, message: &str, boot: &str| Entry {
            message: message.to_owned(),
            cursor: Cursor(format!("i={n}")),
            realtime_us: n * 1_000_000,
            fields: BTreeMap::from([(BOOT_ID.to_owned(), boot.to_owned())]),
            ..Entry::default()
        };
        let kills = Match {
            message_contains: Some("Killed".to_owned()),
            backfill: Backfill::ThisBoot { most: 2 },
            ..Match::default()
        };
        let lately = Match {
            backfill: Backfill::Since {
                seconds: 9,
                most: 10,
            },
            ..Match::default()
        };
        let mut shares = Shares::new(vec![kills, lately], 11_000_000, Some("now".to_owned()));
        for e in [
            line(1, "Killed zero", "before"),
            line(2, "Killed one", "before"),
            line(3, "Killed two", "now"),
            line(4, "noise", "now"),
            line(5, "Killed three", "now"),
            line(6, "noise", "now"),
            line(7, "noise", "now"),
            line(8, "noise", "now"),
        ] {
            shares.offer(&e);
        }
        let (taken, last) = shares.taken();
        let said = |es: &[Entry]| es.iter().map(|e| e.message.clone()).collect::<Vec<_>>();
        assert_eq!(said(&taken[0]), ["Killed two", "Killed three"]);
        // THE `since` reaches across the boot, and keeps only its seconds.
        assert_eq!(
            said(&taken[1]),
            [
                "Killed one",
                "Killed two",
                "noise",
                "Killed three",
                "noise",
                "noise",
                "noise"
            ]
        );
        assert_eq!(last, Some(Cursor("i=8".to_owned())), "the last READ");
    }

    #[test]
    fn a_stream_skips_what_is_not_an_entry_and_what_is_too_long() {
        let long = format!(
            r#"{{"__CURSOR":"s=x;i=3","MESSAGE":"{}"}}"#,
            "y".repeat(LINE_MOST_BYTES + 10)
        );
        let text = format!("{KERNEL}\nnot json at all\n{long}\n{UNIT}\n");
        let mut reading = std::io::Cursor::new(text.into_bytes());
        let first = read_entry(&mut reading).unwrap().unwrap();
        assert_eq!(first.cursor.0, "s=abc;i=1");
        // THE LONG ONE IS SKIPPED WHOLE, and the next line is still a line:
        // a cut in the middle would parse its tail as something it is not.
        let second = read_entry(&mut reading).unwrap().unwrap();
        assert_eq!(second.cursor.0, "s=abc;i=2");
        assert_eq!(read_entry(&mut reading).unwrap(), None);
    }

    /// A PROGRAM THAT WRITES THE JOURNAL'S SHAPE — a script, so the spawn,
    /// the pipe and the end are journalctl's own path and nothing is faked
    /// below `Follow`.
    fn a_journalctl(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[test]
    fn a_journal_that_cannot_be_read_says_so_and_an_ended_one_says_that() {
        let dir = scratch("follow");
        // IT SPEAKS, THEN IT ENDS: two entries, then `Ended` — never a hang.
        // `printf` IS THE SHELL'S OWN: the script needs nothing on the PATH.
        let speaks = a_journalctl(
            &dir,
            "speaks",
            &format!("printf '%s\\n' '{KERNEL}' '{UNIT}'"),
        );
        let mut f = Follow::start(&speaks, None, None, Backfill::Nothing).expect("start");
        assert_eq!(f.next_entry().expect("first").priority, Some(4));
        assert_eq!(f.next_entry().expect("second").priority, Some(3));
        assert_eq!(f.next_entry(), Err(Refusal::Ended));

        // IT CANNOT READ: nothing on its output, a failure, and a sentence on
        // its error — the refusal is `Unreadable` and the sentence is kept.
        let blind = a_journalctl(
            &dir,
            "blind",
            "echo 'No journal files were opened due to insufficient permissions.' >&2\nexit 1",
        );
        let mut f = Follow::start(&blind, None, Some(ERR), Backfill::Nothing).expect("start");
        assert_eq!(f.next_entry(), Err(Refusal::Unreadable));
        // The drain thread may still be finishing; the pipe is closed, so it
        // will — wait for it rather than guess.
        let until = Instant::now() + Duration::from_secs(5);
        while f.said().is_empty() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            f.said().contains("insufficient permissions"),
            "{:?}",
            f.said()
        );

        // AND A PROGRAM THAT IS NOT THERE cannot start.
        assert_eq!(
            Follow::start(&dir.join("absent"), None, None, Backfill::Nothing).err(),
            Some(Refusal::Unstartable)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_follower_that_took_is_told_from_one_that_gave_up() {
        let dir = scratch("settled");
        // GIVES UP AT ONCE: not settled, and its refusal is still read after.
        let blind = a_journalctl(&dir, "blind", "echo 'Failed to seek to cursor' >&2\nexit 1");
        let mut f = Follow::spawn(&blind, &[]).expect("start");
        assert!(!f.settled(Duration::from_secs(5)));
        assert_eq!(f.next_entry(), Err(Refusal::Unreadable));
        // A QUIET JOURNAL, followed: nothing on its output, and still running.
        // A loop of the shell's own, because its input is `/dev/null` and
        // nothing on the PATH is assumed; the drop below kills it.
        let quiet = a_journalctl(&dir, "quiet", "while :; do :; done");
        let mut f = Follow::spawn(&quiet, &[]).expect("start");
        let began = Instant::now();
        assert!(f.settled(Duration::from_millis(150)));
        assert!(began.elapsed() >= Duration::from_millis(150));
        drop(f);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cursor_is_kept_debounced_and_forgotten_when_refused() {
        let dir = scratch("keeper");
        let mut k = Keeper::at(Some(dir.clone()), "cursor", Duration::from_secs(3600));
        assert_eq!(k.saved(), None, "a first start has no cursor");
        assert_eq!(k.due(), None, "nothing pending arms nothing");
        k.note(Cursor("s=a;i=1".to_owned()));
        // THE FIRST ONE IS WRITTEN AT ONCE: nothing was written recently.
        assert!(k.flush_if_due(Instant::now()).expect("write"));
        assert_eq!(k.saved(), Some(Cursor("s=a;i=1".to_owned())));
        // THE NEXT ONES WAIT their turn, however many arrive.
        k.note(Cursor("s=a;i=2".to_owned()));
        k.note(Cursor("s=a;i=3".to_owned()));
        assert!(!k.flush_if_due(Instant::now()).expect("no write"));
        assert!(
            k.due()
                .is_some_and(|at| at > Instant::now() + Duration::from_secs(3000))
        );
        assert_eq!(k.saved(), Some(Cursor("s=a;i=1".to_owned())));
        // …and a flush on the way out writes the newest.
        k.flush().expect("flush");
        assert_eq!(k.saved(), Some(Cursor("s=a;i=3".to_owned())));
        // A REFUSED CURSOR IS FORGOTTEN, so the next start is a first start.
        k.forget();
        assert_eq!(k.saved(), None);
        // A file that is not one cursor is none.
        std::fs::write(dir.join("cursor"), "one\ntwo\n").expect("write");
        assert_eq!(k.saved(), None);
        // A DISK THAT REFUSES is tried again a debounce later, not at once:
        // a cursor always due would be a watch thread that spins.
        let blocked = dir.join("not-a-dir");
        std::fs::write(&blocked, "a file where a directory should be").expect("plant");
        let mut refused = Keeper::at(
            Some(blocked.join("inner")),
            "cursor",
            Duration::from_secs(3600),
        );
        refused.note(Cursor("s=b;i=1".to_owned()));
        assert!(refused.flush().is_err(), "a write under a file succeeded");
        assert!(
            refused
                .due()
                .is_some_and(|at| at > Instant::now() + Duration::from_secs(3000)),
            "a failed write is due again at once"
        );
        // AND A KEEPER WITH NOWHERE TO WRITE keeps nothing and fails nothing.
        let mut nowhere = Keeper::at(None, "cursor", Duration::ZERO);
        nowhere.note(Cursor("x".to_owned()));
        nowhere.flush().expect("nowhere is not a failure");
        assert_eq!(nowhere.saved(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
