//! The local transcripts, counted — and Codex's limits, read where it
//! writes them.
//!
//! # What a token is here
//!
//! Everything the model processed for the account: Claude Code's
//! `input_tokens + output_tokens + cache_creation_input_tokens +
//! cache_read_input_tokens` of each answer, and Codex's `total_tokens` of each
//! turn (its input already counts the cached part). Cache reads dominate a
//! Claude count, and they are what the plan's windows weigh too.
//!
//! # Where
//!
//! - Claude Code: `<dir>/projects/<project>/<session>.jsonl` (and the
//!   sub-agents' files below a session). An answer is written as several
//!   lines with the same `message.id` and `requestId`, and a resumed session
//!   copies earlier answers into its new file — so each answer is counted
//!   ONCE, by those two ids (hashed to eight bytes, remembered at most
//!   [`SEEN_MOST`]). `<synthetic>` answers are Claude Code's own, not the
//!   model's.
//! - Codex: `<dir>/sessions/YYYY/MM/DD/rollout-….jsonl`, `.jsonl.zst` when
//!   compressed, and `archived_sessions/`. A `token_count` event carries the
//!   session's running total, so a turn is the total's step from the one
//!   before; the model is the last `turn_context`'s. A rollout moved to the
//!   archive is the same file: it is known by its name, not its path.
//!
//! # Incremental and bounded
//!
//! A [`Cache`] remembers where each file was read up to (only whole lines),
//! the counts by hour and model, and the answers seen. A count reads only
//! what was appended since. Everything older than [`DAYS`] goes — the counts,
//! the answers remembered, the files not written since — so the cache is
//! bounded by the window (`assets/cache.nix`).
//!
//! # Days are a zone's
//!
//! Counts are kept by UTC hour, which no zone changes. [`Cache::tally_in`]
//! makes days in the zone it is given (the door has the machine's,
//! `ronoctua_files::when::Zone`); [`by_day_and_model`] answers UTC days.

use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Account, Kind, Refusal, Why, Window};
use crate::files::{Line, next_line};
use crate::statusline::Limits;
use crate::time::{DAY_S, hour_of, parse_rfc3339};

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
mod policy {
    #![allow(dead_code, missing_docs)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// How many days back a count reaches (`llmTokenDays`).
pub const DAYS: u32 = policy::LLM_TOKEN_DAYS;

/// The most answers a count remembers, to count each once (`llmSeenMost`).
pub const SEEN_MOST: usize = policy::LLM_SEEN_MOST;

const _: () = assert!(
    DAYS > 0 && SEEN_MOST > 0,
    "a bound of nothing counts nothing"
);

/// The longest line read, in bytes: an answer carries what it wrote (a
/// whole file, sometimes). A longer one is skipped, not half-read.
const MOST_LINE: usize = 32 * 1024 * 1024;

/// The most a compressed log may decode to.
const MOST_DECODED: u64 = 256 * 1024 * 1024;

/// How much of the end of a log is read for its last limits.
const TAIL_BYTES: u64 = 4 * 1024 * 1024;

/// How deep the folders of logs are walked (and watched, [`crate::Watched`]).
pub const DEPTH: usize = 5;

/// Where Claude Code writes its transcripts, in an account's folder.
const CLAUDE_LOGS: &str = "projects";

/// Where Codex writes its session logs, and where it moves them when they
/// are archived, in an account's folder.
const CODEX_LOGS: [&str; 2] = ["sessions", "archived_sessions"];

/// How many of the most recently WRITTEN Codex logs are read for its limits.
const LIMIT_LOGS: usize = 3;

/// What Claude Code calls its own answers, which no model wrote.
const SYNTHETIC: &str = "<synthetic>";

/// Tokens by day (`YYYY-MM-DD`) and by model.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tally {
    /// By day.
    pub by_day: BTreeMap<String, u64>,
    /// By model (`""` when a Codex turn named none).
    pub by_model: BTreeMap<String, u64>,
}

/// Where a Codex log was read up to, besides its offset.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CodexFile {
    /// The session's running total at the last `token_count` read.
    pub last_total: u64,
    /// The model of the last `turn_context` read.
    pub model: String,
}

/// Where the count stopped in each file, so the next one reads only what
/// was appended — and what it counted, by hour and model (bounded by
/// [`DAYS`] and [`SEEN_MOST`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Cache {
    /// By file: the offset read up to (decoded bytes for a `.zst`).
    pub offsets: BTreeMap<String, u64>,
    /// By model, by UTC hour (seconds since the epoch): tokens.
    #[serde(default)]
    pub counted: BTreeMap<String, BTreeMap<i64, u64>>,
    /// The answers counted (a hash of `message.id` and `requestId`), with
    /// the hour each was in.
    #[serde(default)]
    pub seen: BTreeMap<u64, i64>,
    /// By Codex log: its running total and model.
    #[serde(default)]
    pub codex: BTreeMap<String, CodexFile>,
    /// By compressed log: its size on disk when it was last decoded — an
    /// archive does not change, and decoding it again to find nothing new
    /// would be the whole cost of a count.
    #[serde(default)]
    pub sizes: BTreeMap<String, u64>,
}

impl Cache {
    /// The counts as days in a zone: `offset_at` says how many seconds east
    /// of UTC the zone is at an instant (`Zone::offset_at`).
    #[must_use]
    pub fn tally_in(&self, offset_at: &dyn Fn(i64) -> i32) -> Tally {
        let mut tally = Tally::default();
        for (model, hours) in &self.counted {
            for (hour, n) in hours {
                *tally
                    .by_day
                    .entry(crate::time::day_of(*hour, offset_at(*hour)))
                    .or_default() += n;
                *tally.by_model.entry(model.clone()).or_default() += n;
            }
        }
        tally
    }

    /// Everything older than the window forgotten, and the answers seen kept
    /// under their bound — the oldest hours going first.
    fn prune(&mut self, start_s: i64) {
        for hours in self.counted.values_mut() {
            hours.retain(|hour, _| *hour >= start_s);
        }
        self.counted.retain(|_, hours| !hours.is_empty());
        self.seen.retain(|_, hour| *hour >= start_s);
        if self.seen.len() > SEEN_MOST {
            let mut hours: Vec<i64> = self.seen.values().copied().collect();
            hours.sort_unstable();
            let cut = hours[self.seen.len() - SEEN_MOST];
            self.seen.retain(|_, hour| *hour > cut);
        }
    }
}

/// Counts the account in `dir` (what was appended since `cache` last read),
/// and answers the tally in UTC days — [`Cache::tally_in`] for a zone's.
///
/// Blocks on the files: call it on a thread.
///
/// # Errors
///
/// [`Why::Unreadable`] when the account's folder does not read. A folder
/// with no logs yet is an empty tally.
pub fn by_day_and_model(dir: &Path, kind: Kind, cache: &mut Cache) -> Result<Tally, Refusal> {
    count(dir, kind, cache, crate::time::now_s())?;
    Ok(cache.tally_in(&|_| 0))
}

/// The count, at an instant.
pub(crate) fn count(dir: &Path, kind: Kind, cache: &mut Cache, now_s: i64) -> Result<(), Refusal> {
    if !dir.is_dir() {
        return Err(Refusal::of(Why::Unreadable));
    }
    let start = hour_of(now_s - i64::from(DAYS) * DAY_S);
    let logs = logs_of(dir, kind);
    let mut live = std::collections::BTreeSet::new();
    for log in &logs {
        let Ok(meta) = std::fs::metadata(&log.path) else {
            continue;
        };
        let written = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(now_s, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        if written < start {
            // Nothing in it is inside the window any more.
            continue;
        }
        live.insert(log.key.clone());
        let from = cache.offsets.get(&log.key).copied();
        if log.compressed && from.is_some() && cache.sizes.get(&log.key) == Some(&meta.len()) {
            continue;
        }
        let from = from.unwrap_or(0);
        let read = if log.compressed {
            read_compressed(log, from, kind, cache, start)
        } else {
            read_plain(log, from, meta.len(), kind, cache, start)
        };
        match read {
            Ok(to) => {
                cache.offsets.insert(log.key.clone(), to);
                if log.compressed {
                    cache.sizes.insert(log.key.clone(), meta.len());
                } else {
                    cache.sizes.remove(&log.key);
                }
            }
            Err(e) => eprintln!("ronoctua-llm: {} did not read: {e}", log.path.display()),
        }
    }
    cache.offsets.retain(|key, _| live.contains(key));
    cache.codex.retain(|key, _| live.contains(key));
    cache.sizes.retain(|key, _| live.contains(key));
    cache.prune(start);
    Ok(())
}

/// One log file, and the name it is known by.
struct Log {
    path: PathBuf,
    key: String,
    compressed: bool,
}

/// The folders an account's logs are in — what [`logs_of`] walks.
pub(crate) fn trees_of(dir: &Path, kind: Kind) -> Vec<PathBuf> {
    match kind {
        Kind::Claude => vec![dir.join(CLAUDE_LOGS)],
        Kind::Codex => CODEX_LOGS.iter().map(|d| dir.join(d)).collect(),
    }
}

/// Every log of the account, oldest name first.
fn logs_of(dir: &Path, kind: Kind) -> Vec<Log> {
    let transcript = |name: &str| name.ends_with(".jsonl");
    let rollout = |name: &str| {
        name.starts_with("rollout-") && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
    };
    let keep: &dyn Fn(&str) -> bool = match kind {
        Kind::Claude => &transcript,
        Kind::Codex => &rollout,
    };
    let mut found = Vec::new();
    for tree in trees_of(dir, kind) {
        walk(&tree, DEPTH, keep, &mut found);
    }
    match kind {
        Kind::Claude => found
            .into_iter()
            .map(|path| Log {
                key: path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                path,
                compressed: false,
            })
            .collect(),
        Kind::Codex => {
            let mut logs: Vec<Log> = found
                .into_iter()
                .filter_map(|path| {
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    let compressed = name.ends_with(".zst");
                    Some(Log {
                        key: name.trim_end_matches(".zst").to_owned(),
                        path,
                        compressed,
                    })
                })
                .collect();
            logs.sort_by(|a, b| a.key.cmp(&b.key).then(a.compressed.cmp(&b.compressed)));
            // The same rollout twice (archived and not, plain and
            // compressed): read once, the plain one first.
            logs.dedup_by(|later, earlier| later.key == earlier.key);
            logs
        }
    }
}

/// Files under `root` whose name `keep` takes, `depth` folders down at most;
/// links are not followed.
fn walk(root: &Path, depth: usize, keep: &dyn Fn(&str) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() && depth > 0 {
            walk(&path, depth - 1, keep, out);
        } else if kind.is_file() && entry.file_name().to_str().is_some_and(keep) {
            out.push(path);
        }
    }
}

/// Reads a plain log from `from`; answers where it stopped (after the last
/// whole line).
fn read_plain(
    log: &Log,
    from: u64,
    len: u64,
    kind: Kind,
    cache: &mut Cache,
    start: i64,
) -> std::io::Result<u64> {
    let from = if len < from {
        // Shorter than what was read: rewritten. Read again from the top.
        cache.codex.remove(&log.key);
        0
    } else {
        from
    };
    let mut file = std::fs::File::open(&log.path)?;
    file.seek(std::io::SeekFrom::Start(from))?;
    let mut reader = std::io::BufReader::with_capacity(256 * 1024, file);
    read_lines(&mut reader, from, &log.key, kind, cache, start)
}

/// Reads a compressed log from `from` decoded bytes.
fn read_compressed(
    log: &Log,
    from: u64,
    kind: Kind,
    cache: &mut Cache,
    start: i64,
) -> std::io::Result<u64> {
    let decoded = decode(&log.path)?;
    let from = if (decoded.len() as u64) < from {
        cache.codex.remove(&log.key);
        0
    } else {
        from
    };
    let at = usize::try_from(from)
        .unwrap_or(usize::MAX)
        .min(decoded.len());
    let mut reader = std::io::BufReader::new(&decoded[at..]);
    read_lines(&mut reader, from, &log.key, kind, cache, start)
}

/// A zstd file decoded whole — every frame, a skippable one skipped — up to
/// [`MOST_DECODED`] bytes.
fn decode(path: &Path) -> std::io::Result<Vec<u8>> {
    use ruzstd::decoding::StreamingDecoder;
    use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
    let mut source = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut out = Vec::new();
    loop {
        let mut decoder = match StreamingDecoder::new(&mut source) {
            Ok(d) => d,
            // The end of the file is where a frame header would begin.
            Err(FrameDecoderError::ReadFrameHeaderError(
                ReadFrameHeaderError::MagicNumberReadError(e),
            )) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(out),
            Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                length,
                ..
            })) => {
                std::io::copy(
                    &mut (&mut source).take(u64::from(length)),
                    &mut std::io::sink(),
                )?;
                continue;
            }
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ));
            }
        };
        let room = MOST_DECODED.saturating_sub(out.len() as u64);
        (&mut decoder).take(room + 1).read_to_end(&mut out)?;
        if out.len() as u64 > MOST_DECODED {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{} decodes past {MOST_DECODED} bytes", path.display()),
            ));
        }
    }
}

/// Reads whole lines, counting each; answers the offset after the last one.
fn read_lines(
    reader: &mut impl std::io::BufRead,
    mut at: u64,
    key: &str,
    kind: Kind,
    cache: &mut Cache,
    start: i64,
) -> std::io::Result<u64> {
    let mut buf = Vec::new();
    loop {
        match next_line(reader, &mut buf, MOST_LINE)? {
            Line::Whole(n) => {
                at += n as u64;
                match kind {
                    Kind::Claude => claude_line(&buf, cache, start),
                    Kind::Codex => codex_line(&buf, key, cache, start),
                }
            }
            Line::TooLong(n) => at += n as u64,
            Line::Partial | Line::End => return Ok(at),
        }
    }
}

/// FNV-1a, 64 bits: a stable hash of an answer's ids (stable across
/// toolchains, unlike `DefaultHasher`, so a cache outlives an upgrade).
fn fnv(parts: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (n, part) in parts.iter().enumerate() {
        if n > 0 {
            hash ^= u64::from(b':');
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        for byte in part.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

/// Adds `n` tokens of `model` at the hour of `at_s`.
fn add(cache: &mut Cache, model: &str, at_s: i64, n: u64) {
    if n == 0 {
        return;
    }
    let hours = cache.counted.entry(model.to_owned()).or_default();
    *hours.entry(hour_of(at_s)).or_default() += n;
}

/// One line of a Claude Code transcript.
fn claude_line(line: &[u8], cache: &mut Cache, start: i64) {
    use std::borrow::Cow;
    #[derive(Deserialize)]
    struct Entry<'a> {
        #[serde(borrow, default)]
        timestamp: Option<Cow<'a, str>>,
        #[serde(rename = "requestId", borrow, default)]
        request_id: Option<Cow<'a, str>>,
        #[serde(borrow, default)]
        message: Option<Message<'a>>,
    }
    #[derive(Deserialize)]
    struct Message<'a> {
        #[serde(borrow, default)]
        id: Option<Cow<'a, str>>,
        #[serde(borrow, default)]
        model: Option<Cow<'a, str>>,
        #[serde(default)]
        usage: Option<Usage>,
    }
    #[derive(Deserialize)]
    struct Usage {
        #[serde(default)]
        input_tokens: Option<u64>,
        #[serde(default)]
        output_tokens: Option<u64>,
        #[serde(default)]
        cache_creation_input_tokens: Option<u64>,
        #[serde(default)]
        cache_read_input_tokens: Option<u64>,
    }
    if memchr(line, b"\"usage\"").is_none() {
        return;
    }
    let Ok(entry) = serde_json::from_slice::<Entry<'_>>(line) else {
        return;
    };
    let Some(message) = entry.message else {
        return;
    };
    let Some(usage) = message.usage else {
        return;
    };
    let model = message.model.as_deref().unwrap_or_default();
    if model == SYNTHETIC {
        return;
    }
    let Some(at) = entry.timestamp.as_deref().and_then(parse_rfc3339) else {
        return;
    };
    if at < start {
        return;
    }
    if let (Some(id), Some(request)) = (message.id.as_deref(), entry.request_id.as_deref()) {
        let key = fnv(&[id, request]);
        if cache.seen.contains_key(&key) {
            return;
        }
        cache.seen.insert(key, hour_of(at));
    }
    let n = [
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_creation_input_tokens,
        usage.cache_read_input_tokens,
    ]
    .into_iter()
    .flatten()
    .fold(0_u64, u64::saturating_add);
    add(cache, model, at, n);
}

/// A Codex log line, as far as tokens, models and limits go.
#[derive(Deserialize)]
struct CodexEntry<'a> {
    #[serde(borrow, default)]
    timestamp: Option<std::borrow::Cow<'a, str>>,
    #[serde(rename = "type", borrow, default)]
    kind: Option<std::borrow::Cow<'a, str>>,
    #[serde(borrow, default)]
    payload: Option<CodexPayload<'a>>,
}

#[derive(Deserialize)]
struct CodexPayload<'a> {
    #[serde(rename = "type", borrow, default)]
    kind: Option<std::borrow::Cow<'a, str>>,
    #[serde(borrow, default)]
    model: Option<std::borrow::Cow<'a, str>>,
    #[serde(default)]
    info: Option<CodexInfo>,
    #[serde(default)]
    rate_limits: Option<CodexLimits>,
}

#[derive(Deserialize)]
struct CodexInfo {
    #[serde(default)]
    total_token_usage: Option<CodexUsage>,
    #[serde(default)]
    last_token_usage: Option<CodexUsage>,
}

#[derive(Deserialize)]
struct CodexUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    total_tokens: Option<u64>,
}

impl CodexUsage {
    /// The turn's or the session's tokens: its `total_tokens`, or input and
    /// output when an older Codex wrote no total.
    fn total(&self) -> u64 {
        self.total_tokens.unwrap_or_else(|| {
            self.input_tokens
                .unwrap_or(0)
                .saturating_add(self.output_tokens.unwrap_or(0))
        })
    }
}

#[derive(Deserialize)]
struct CodexLimits {
    #[serde(default)]
    primary: Option<CodexWindow>,
    #[serde(default)]
    secondary: Option<CodexWindow>,
}

#[derive(Deserialize)]
struct CodexWindow {
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    window_minutes: Option<u64>,
    #[serde(default)]
    resets_at: Option<i64>,
    #[serde(default)]
    resets_in_seconds: Option<i64>,
}

/// One line of a Codex log.
fn codex_line(line: &[u8], key: &str, cache: &mut Cache, start: i64) {
    if memchr(line, b"token_count").is_none() && memchr(line, b"turn_context").is_none() {
        return;
    }
    let Ok(entry) = serde_json::from_slice::<CodexEntry<'_>>(line) else {
        return;
    };
    let Some(payload) = entry.payload else {
        return;
    };
    let file = cache.codex.entry(key.to_owned()).or_default();
    if entry.kind.as_deref() == Some("turn_context") {
        if let Some(model) = payload.model {
            file.model = model.into_owned();
        }
        return;
    }
    if payload.kind.as_deref() != Some("token_count") {
        return;
    }
    let Some(info) = payload.info else {
        return;
    };
    let Some(total) = info.total_token_usage.as_ref().map(CodexUsage::total) else {
        return;
    };
    // The step from the total before; the same total again is the same
    // event written twice, and a smaller one is a session that started over.
    let n = match total.checked_sub(file.last_total) {
        Some(step) => step,
        None => info
            .last_token_usage
            .as_ref()
            .map_or(total, CodexUsage::total),
    };
    file.last_total = total;
    let model = file.model.clone();
    let Some(at) = entry.timestamp.as_deref().and_then(parse_rfc3339) else {
        return;
    };
    if at >= start {
        add(cache, &model, at, n);
    }
}

/// Where `needle` first is in `hay`.
fn memchr(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Codex's limits, out of its most recently written session logs: the
/// latest `token_count` that carries `rate_limits`, by its own timestamp,
/// across them. The window of at most a day is the five-hour one; the
/// longer, the weekly one.
///
/// By the time a file was WRITTEN, not by its name: a rollout is named when
/// its session starts, and a session resumed today appends to the file of
/// the day it began.
///
/// # Errors
///
/// None today: a folder with no logs (or logs with no limits) is no window.
pub(crate) fn codex_limits(dir: &Path) -> Result<Limits, Refusal> {
    let mut logs: Vec<(std::time::SystemTime, Log)> = logs_of(dir, Kind::Codex)
        .into_iter()
        .filter_map(|log| Some((std::fs::metadata(&log.path).ok()?.modified().ok()?, log)))
        .collect();
    logs.sort_by_key(|(written, _)| std::cmp::Reverse(*written));
    let mut newest: Option<(i64, Limits)> = None;
    for (_, log) in logs.iter().take(LIMIT_LOGS) {
        let tail = match tail_of(log) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("ronoctua-llm: {} did not read: {e}", log.path.display());
                continue;
            }
        };
        if let Some((at, limits)) = last_limits(&tail)
            && newest.as_ref().is_none_or(|(when, _)| at > *when)
        {
            newest = Some((at, limits));
        }
    }
    Ok(newest.map(|(_, limits)| limits).unwrap_or_default())
}

/// The last [`TAIL_BYTES`] of a log (decoded, for a `.zst`), from its first
/// whole line.
fn tail_of(log: &Log) -> std::io::Result<Vec<u8>> {
    let (mut bytes, cut) = if log.compressed {
        let mut decoded = decode(&log.path)?;
        let cut = decoded.len().saturating_sub(TAIL_BYTES as usize);
        decoded.drain(..cut);
        (decoded, cut > 0)
    } else {
        let mut file = std::fs::File::open(&log.path)?;
        let from = file.metadata()?.len().saturating_sub(TAIL_BYTES);
        file.seek(std::io::SeekFrom::Start(from))?;
        let mut bytes = Vec::new();
        file.take(TAIL_BYTES).read_to_end(&mut bytes)?;
        (bytes, from > 0)
    };
    if cut {
        // Begun in the middle of a line: that line is not whole.
        let first = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |n| n + 1);
        bytes.drain(..first);
    }
    Ok(bytes)
}

/// The limits of the latest `token_count` in `text` that carries them, and
/// when it was written.
fn last_limits(text: &[u8]) -> Option<(i64, Limits)> {
    let mut found: Option<(i64, Limits)> = None;
    for line in text.split(|b| *b == b'\n') {
        if memchr(line, b"rate_limits").is_none() {
            continue;
        }
        let Ok(entry) = serde_json::from_slice::<CodexEntry<'_>>(line) else {
            continue;
        };
        let Some(at) = entry.timestamp.as_deref().and_then(parse_rfc3339) else {
            continue;
        };
        let Some(raw) = entry.payload.and_then(|p| p.rate_limits) else {
            continue;
        };
        let mut limits = Limits::default();
        for (named_long, w) in [(false, raw.primary), (true, raw.secondary)] {
            let Some(w) = w else { continue };
            let Some(resets_at_s) = w.resets_at.or_else(|| w.resets_in_seconds.map(|s| at + s))
            else {
                continue;
            };
            // How much is used, or the window is not known: read as 0 %, a
            // missing number would keep the mark calm at the edge.
            let Some(used) = w.used_percent.filter(|p| p.is_finite() && *p >= 0.0) else {
                continue;
            };
            let window = Window {
                used_percent: used as f32,
                resets_at_s,
            };
            let long = w.window_minutes.map_or(named_long, |m| m > 24 * 60);
            if long {
                limits.seven_day = Some(window);
            } else {
                limits.five_hour = Some(window);
            }
        }
        if found.as_ref().is_none_or(|(when, _)| at >= *when) {
            found = Some((at, limits));
        }
    }
    found
}

/// The file an account's count is kept in, under a cache root.
pub(crate) fn kept_path(cache: &Path, account: &Account) -> Option<PathBuf> {
    crate::id_folder(cache, &account.id).map(|d| d.join("tokens.json"))
}

/// The count kept for `account` (`$XDG_CACHE_HOME/ronoctua/llm/<id>/tokens.json`);
/// an empty one when there is none, or it does not read — it is a cache, and
/// counting again is what an empty one does.
#[must_use]
pub fn kept(account: &Account) -> Cache {
    ronoctua_xdg::cache_dir()
        .map(|root| kept_in(&root, account))
        .unwrap_or_default()
}

/// [`kept`], under a given cache root.
pub(crate) fn kept_in(root: &Path, account: &Account) -> Cache {
    kept_path(root, account)
        .and_then(|p| crate::files::read_bounded(&p, 64 * 1024 * 1024).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Keeps the count for `account`, written whole and atomically.
///
/// # Errors
///
/// [`Why::Unknown`] for an id that is not a folder's name; [`Why::Failed`]
/// when it could not be written.
pub fn keep(account: &Account, cache: &Cache) -> Result<(), Refusal> {
    let root = ronoctua_xdg::cache_dir().ok_or(Refusal::of(Why::Failed))?;
    keep_in(&root, account, cache)
}

/// [`keep`], under a given cache root.
pub(crate) fn keep_in(root: &Path, account: &Account, cache: &Cache) -> Result<(), Refusal> {
    let path = kept_path(root, account).ok_or(Refusal::of(Why::Unknown))?;
    let bytes = serde_json::to_vec(cache).map_err(|_| Refusal::of(Why::Failed))?;
    crate::files::write_keeping(&path, &bytes, 0o600).map_err(|_| Refusal::of(Why::Failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{HOUR_S, days_from_civil};

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-tokens-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Noon on 2026-10-05, UTC.
    fn noon() -> i64 {
        days_from_civil(2026, 10, 5) * DAY_S + 12 * HOUR_S
    }

    fn stamp(at_s: i64) -> String {
        let day = crate::time::day_of(at_s, 0);
        let s = at_s.rem_euclid(DAY_S);
        format!(
            "{day}T{:02}:{:02}:{:02}.123Z",
            s / HOUR_S,
            (s % HOUR_S) / 60,
            s % 60
        )
    }

    fn answer(at_s: i64, id: &str, request: &str, model: &str, tokens: [u64; 4]) -> String {
        format!(
            r#"{{"parentUuid":"p","type":"assistant","timestamp":"{}","requestId":"{request}","message":{{"id":"{id}","type":"message","role":"assistant","model":"{model}","content":[{{"type":"text","text":"a \"usage\" inside the text"}}],"usage":{{"input_tokens":{},"output_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{},"service_tier":"standard","cache_creation":{{"ephemeral_5m_input_tokens":0}}}}}}}}"#,
            stamp(at_s),
            tokens[0],
            tokens[1],
            tokens[2],
            tokens[3]
        )
    }

    /// FAILS IF: an answer written as several lines, or copied into a
    /// resumed session's file, is counted more than once; a `<synthetic>`
    /// answer is counted; a half-written line is read; or an appended line
    /// is not counted on the next pass.
    #[test]
    fn each_claude_answer_is_counted_once() {
        let dir = scratch("claude");
        let project = dir.join("projects").join("-home-p-work");
        std::fs::create_dir_all(&project).unwrap();
        let t = noon();
        let a = answer(t, "msg_a", "req_a", "claude-opus-4-1", [10, 20, 30, 40]);
        let b = answer(
            t + 2 * HOUR_S,
            "msg_b",
            "req_b",
            "claude-sonnet-4-5",
            [1, 2, 0, 0],
        );
        let synthetic = answer(t, "msg_s", "req_s", "<synthetic>", [99, 99, 0, 0]);
        let user = format!(
            r#"{{"type":"user","timestamp":"{}","message":{{"role":"user","content":"hi"}}}}"#,
            stamp(t)
        );
        // The first answer twice (two content blocks), and a half line.
        std::fs::write(
            project.join("s1.jsonl"),
            format!("{user}\n{a}\n{a}\n{synthetic}\n{b}\n{{\"type\":\"assist"),
        )
        .unwrap();
        let mut cache = Cache::default();
        count(&dir, Kind::Claude, &mut cache, t + 3 * HOUR_S).unwrap();
        let tally = cache.tally_in(&|_| 0);
        assert_eq!(tally.by_model["claude-opus-4-1"], 100);
        assert_eq!(tally.by_model["claude-sonnet-4-5"], 3);
        assert!(!tally.by_model.contains_key("<synthetic>"));
        assert_eq!(tally.by_day["2026-10-05"], 103);

        // A resumed session copies the first answer and adds one.
        let c = answer(
            t + 2 * HOUR_S,
            "msg_c",
            "req_c",
            "claude-opus-4-1",
            [0, 5, 0, 0],
        );
        std::fs::write(project.join("s2.jsonl"), format!("{a}\n{c}\n")).unwrap();
        // And the half line of the first file is finished.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(project.join("s1.jsonl"))
            .unwrap();
        std::io::Write::write_all(&mut f, b"ant\"}\n").unwrap();
        count(&dir, Kind::Claude, &mut cache, t + 3 * HOUR_S).unwrap();
        let tally = cache.tally_in(&|_| 0);
        assert_eq!(
            tally.by_model["claude-opus-4-1"], 105,
            "the copy is not counted again"
        );
        assert_eq!(
            cache.offsets["projects/-home-p-work/s1.jsonl"],
            std::fs::metadata(project.join("s1.jsonl")).unwrap().len()
        );

        // Days are the zone's: 23:30 UTC is the next day three hours east.
        let late = answer(
            t + 11 * HOUR_S + 1800,
            "msg_d",
            "req_d",
            "claude-opus-4-1",
            [0, 7, 0, 0],
        );
        std::fs::write(project.join("s3.jsonl"), format!("{late}\n")).unwrap();
        count(&dir, Kind::Claude, &mut cache, t + 12 * HOUR_S).unwrap();
        assert_eq!(cache.tally_in(&|_| 0).by_day.get("2026-10-06"), None);
        assert_eq!(cache.tally_in(&|_| 3 * 3600).by_day["2026-10-06"], 7);

        // And what is older than the window goes.
        count(
            &dir,
            Kind::Claude,
            &mut cache,
            t + i64::from(DAYS) * DAY_S + 13 * HOUR_S,
        )
        .unwrap();
        assert_eq!(cache.tally_in(&|_| 0), Tally::default());
        assert!(cache.seen.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn token_count(at_s: i64, total: u64, last: u64, limits: &str) -> String {
        format!(
            r#"{{"timestamp":"{}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{total},"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":{total}}},"last_token_usage":{{"input_tokens":{last},"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":{last}}},"model_context_window":272000}},"rate_limits":{limits}}}}}"#,
            stamp(at_s)
        )
    }

    fn turn(at_s: i64, model: &str) -> String {
        format!(
            r#"{{"timestamp":"{}","type":"turn_context","payload":{{"cwd":"/home/p","approval_policy":"on-request","model":"{model}","summary":"auto"}}}}"#,
            stamp(at_s)
        )
    }

    /// FAILS IF: Codex's running totals are added instead of stepped, a
    /// repeated event is counted, the model is not the turn's, a compressed
    /// log is not read, or an archived copy of a rollout is counted again.
    #[test]
    fn codex_turns_are_the_steps_of_the_running_total() {
        let dir = scratch("codex");
        let day = dir.join("sessions").join("2026").join("10").join("05");
        std::fs::create_dir_all(&day).unwrap();
        let t = noon();
        let limits = r#"{"primary":{"used_percent":12.0,"window_minutes":300,"resets_in_seconds":600},"secondary":{"used_percent":40.5,"window_minutes":10080,"resets_at":4102444800}}"#;
        let rollout = [
            r#"{"timestamp":"2026-10-05T12:00:00Z","type":"session_meta","payload":{"id":"x"}}"#
                .to_owned(),
            turn(t, "gpt-5-codex"),
            token_count(t + 60, 1000, 1000, "null"),
            token_count(t + 61, 1000, 1000, "null"),
            token_count(t + 120, 1500, 500, limits),
            turn(t + 200, "gpt-5"),
            token_count(t + 300, 1600, 100, "null"),
        ]
        .join("\n")
            + "\n";
        let name = "rollout-2026-10-05T12-00-00-0199aaaa-bbbb-cccc-dddd-eeeeffff0000.jsonl";
        std::fs::write(day.join(name), &rollout).unwrap();
        // An older session, compressed.
        let older = format!(
            "{}\n{}\n",
            turn(t - HOUR_S, "gpt-5-codex"),
            token_count(t - HOUR_S, 250, 250, "null")
        );
        let compressed = ruzstd::encoding::compress_to_vec(
            older.as_bytes(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        std::fs::write(
            day.join("rollout-2026-10-05T11-00-00-0199aaaa-0000-cccc-dddd-eeeeffff0000.jsonl.zst"),
            compressed,
        )
        .unwrap();
        // And the newer one, also archived under its own name.
        let archive = dir.join("archived_sessions");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(archive.join(name), &rollout).unwrap();

        let mut cache = Cache::default();
        count(&dir, Kind::Codex, &mut cache, t + HOUR_S).unwrap();
        let tally = cache.tally_in(&|_| 0);
        assert_eq!(tally.by_model["gpt-5-codex"], 1500 + 250);
        assert_eq!(tally.by_model["gpt-5"], 100);

        // Appended: only the step after the last total.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(day.join(name))
            .unwrap();
        std::io::Write::write_all(
            &mut f,
            format!("{}\n", token_count(t + 400, 1700, 100, "null")).as_bytes(),
        )
        .unwrap();
        count(&dir, Kind::Codex, &mut cache, t + HOUR_S).unwrap();
        assert_eq!(cache.tally_in(&|_| 0).by_model["gpt-5"], 200);

        // The limits are the newest event's: primary is the five hours.
        let got = codex_limits(&dir).unwrap();
        assert_eq!(
            got.five_hour,
            Some(Window {
                used_percent: 12.0,
                resets_at_s: t + 120 + 600
            })
        );
        assert_eq!(
            got.seven_day,
            Some(Window {
                used_percent: 40.5,
                resets_at_s: 4_102_444_800
            })
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: an archive that grew by a frame is not read from where the
    /// count stopped, or the first line of a short archive is taken for half
    /// a line.
    #[test]
    fn an_archive_is_read_once_and_whole() {
        let dir = scratch("archive");
        let archive = dir.join("archived_sessions");
        std::fs::create_dir_all(&archive).unwrap();
        let t = noon();
        let limits = r#"{"primary":{"used_percent":7.0,"window_minutes":299,"resets_at":4102444800},"secondary":null}"#;
        let first = format!(
            "{}\n{}\n",
            token_count(t, 300, 300, limits),
            turn(t + 1, "gpt-5")
        );
        let frame = |text: &str| {
            ruzstd::encoding::compress_to_vec(
                text.as_bytes(),
                ruzstd::encoding::CompressionLevel::Fastest,
            )
        };
        let path = archive
            .join("rollout-2026-10-05T12-00-00-0199aaaa-0000-0000-0000-000000000001.jsonl.zst");
        std::fs::write(&path, frame(&first)).unwrap();
        assert_eq!(
            codex_limits(&dir).unwrap().five_hour,
            Some(Window {
                used_percent: 7.0,
                resets_at_s: 4_102_444_800
            }),
            "the first line of a short archive is whole"
        );
        let mut cache = Cache::default();
        count(&dir, Kind::Codex, &mut cache, t + HOUR_S).unwrap();
        assert_eq!(
            cache.tally_in(&|_| 0).by_model[""],
            300,
            "no turn named a model yet"
        );

        // A frame appended: only what it adds.
        let second = token_count(t + 60, 450, 150, "null") + "\n";
        let mut bytes = frame(&first);
        bytes.extend(frame(&second));
        std::fs::write(&path, bytes).unwrap();
        count(&dir, Kind::Codex, &mut cache, t + HOUR_S).unwrap();
        let tally = cache.tally_in(&|_| 0);
        assert_eq!((tally.by_model[""], tally.by_model["gpt-5"]), (300, 150));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: Codex's limits are taken from the log with the newest NAME
    /// rather than the newest event — a session resumed today writes into
    /// the file named for the day it began — or a window that does not say
    /// how much of it is used is read as 0 %.
    #[test]
    fn codex_limits_are_the_newest_events_whatever_the_file_is_named() {
        let dir = scratch("resumed");
        let t = noon();
        let sessions = dir.join("sessions");
        let old_day = sessions.join("2026").join("09").join("20");
        let new_day = sessions.join("2026").join("10").join("04");
        std::fs::create_dir_all(&old_day).unwrap();
        std::fs::create_dir_all(&new_day).unwrap();
        let limits = |five: f64, week: f64| {
            format!(
                r#"{{"primary":{{"used_percent":{five},"window_minutes":300,"resets_at":4102444800}},"secondary":{{"used_percent":{week},"window_minutes":10080,"resets_at":4102531200}}}}"#
            )
        };
        // Yesterday's session, named for yesterday.
        let yesterday =
            new_day.join("rollout-2026-10-04T09-00-00-0199aaaa-0000-0000-0000-00000000000b.jsonl");
        std::fs::write(
            &yesterday,
            token_count(t - 27 * HOUR_S, 100, 100, &limits(10.0, 20.0)) + "\n",
        )
        .unwrap();
        // A session from September, resumed this morning: its file is named
        // for the day it began, and it has today's numbers.
        let resumed =
            old_day.join("rollout-2026-09-20T09-00-00-0199aaaa-0000-0000-0000-00000000000a.jsonl");
        std::fs::write(
            &resumed,
            format!(
                "{}\n{}\n",
                token_count(t - 15 * DAY_S, 50, 50, &limits(1.0, 2.0)),
                token_count(t, 90, 40, &limits(96.0, 81.0))
            ),
        )
        .unwrap();
        let set = |path: &Path, at: i64| {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(at.unsigned_abs()),
                )
                .unwrap();
        };
        set(&yesterday, t - 27 * HOUR_S);
        set(&resumed, t);
        let got = codex_limits(&dir).unwrap();
        assert_eq!(got.five_hour.map(|w| w.used_percent), Some(96.0));
        assert_eq!(got.seven_day.map(|w| w.used_percent), Some(81.0));

        // Both among the newest written, the file times the other way round:
        // the event's own time decides.
        set(&yesterday, t + 60);
        set(&resumed, t - DAY_S);
        assert_eq!(
            codex_limits(&dir)
                .unwrap()
                .five_hour
                .map(|w| w.used_percent),
            Some(96.0)
        );

        // A newer event whose five-hour window has no percentage: that
        // window is not known, not calm.
        let today = sessions.join("2026").join("10").join("05");
        std::fs::create_dir_all(&today).unwrap();
        let fresh =
            today.join("rollout-2026-10-05T12-00-00-0199aaaa-0000-0000-0000-00000000000c.jsonl");
        let unnamed = r#"{"primary":{"window_minutes":300,"resets_at":4102444800},"secondary":{"used_percent":82.0,"window_minutes":10080,"resets_at":4102531200}}"#;
        std::fs::write(&fresh, token_count(t + 120, 10, 10, unnamed) + "\n").unwrap();
        set(&fresh, t + 120);
        let got = codex_limits(&dir).unwrap();
        assert_eq!(got.five_hour, None);
        assert_eq!(got.seven_day.map(|w| w.used_percent), Some(82.0));

        // More logs than are read: the ones read are the last WRITTEN. Two
        // sessions named later than all of these ended two days ago; the
        // resumed one goes on.
        for name in ["13", "14"] {
            let later = today.join(format!(
                "rollout-2026-10-05T{name}-00-00-0199aaaa-0000-0000-0000-0000000000{name}.jsonl"
            ));
            std::fs::write(
                &later,
                token_count(t - 2 * DAY_S, 5, 5, &limits(3.0, 4.0)) + "\n",
            )
            .unwrap();
            set(&later, t - 2 * DAY_S);
        }
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&resumed)
            .unwrap();
        std::io::Write::write_all(
            &mut f,
            format!("{}\n", token_count(t + 300, 95, 5, &limits(97.0, 83.0))).as_bytes(),
        )
        .unwrap();
        drop(f);
        set(&resumed, t + 300);
        let got = codex_limits(&dir).unwrap();
        assert_eq!(got.five_hour.map(|w| w.used_percent), Some(97.0));
        assert_eq!(got.seven_day.map(|w| w.used_percent), Some(83.0));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_cache_crosses_a_file_and_stays_bounded() {
        let dir = scratch("kept");
        let account = Account {
            id: "codex-1".to_owned(),
            kind: Kind::Codex,
            name: "Home".to_owned(),
            dir: dir.join("codex"),
            default: true,
        };
        let mut cache = Cache::default();
        cache.offsets.insert("a".to_owned(), 10);
        cache
            .counted
            .entry("m".to_owned())
            .or_default()
            .insert(noon(), 5);
        cache.seen.insert(u64::MAX, noon());
        keep_in(&dir, &account, &cache).unwrap();
        assert_eq!(kept_in(&dir, &account), cache);
        std::fs::write(
            dir.join("llm").join("codex-1").join("tokens.json"),
            "{ broken",
        )
        .unwrap();
        assert_eq!(
            kept_in(&dir, &account),
            Cache::default(),
            "a broken cache is an empty one"
        );

        let mut many = Cache::default();
        for n in 0..(SEEN_MOST as u64 + 10) {
            many.seen
                .insert(n, noon() + i64::try_from(n % 50).unwrap() * HOUR_S);
        }
        many.prune(0);
        assert!(many.seen.len() <= SEEN_MOST);
        assert!(
            many.seen.values().all(|h| *h > noon()),
            "the oldest hour went first"
        );
        assert_ne!(fnv(&["msg_a", "req_a"]), fnv(&["msg_ar", "eq_a"]));
        let _ = std::fs::remove_dir_all(dir);
    }
}
