//! VirusTotal: what many engines said of a file — by its hash first, and by
//! the file itself only when the person answered yes to a question that says
//! what that shares.
//!
//! # The hash never leaves as more than a hash
//!
//! The SHA-256 is computed HERE ([`sha256_of`], `ring`, streamed), and the
//! first question is `GET /api/v3/files/{sha256}` with the person's key in
//! `x-apikey` ([`lookup`]). A file VirusTotal has seen answers its last
//! analysis; one it has not answers 404, which is [`Checked::Unknown`] — and
//! nothing more is sent on its own.
//!
//! # The upload is the person's, after a "no" first
//!
//! A file sent to VirusTotal is shared with its partners and can be
//! downloaded by its premium subscribers. So the door asks, with the "no"
//! drawn first and the cursor on it, and only a yes queues the upload as
//! `Work::Scan` on the transfer queue — which runs `ronoctua-scan`
//! (`src/bin/scan.rs`) as a child, because the queue carries no TLS. The file
//! goes as a streamed `multipart/form-data` ([`send_file`],
//! `ronoctua_http::web::upload_let`): straight to `/api/v3/files` up to
//! [`DIRECT_MOST`], through the address `/api/v3/files/upload_url` hands out
//! up to [`UPLOAD_MOST`], and refused above that ([`Why::TooLarge`]).
//!
//! # Polled only while it is pending
//!
//! An analysis is `queued`, then `in-progress`, then `completed`. It is asked
//! again every [`POLL`] — never faster than the free key's four requests a
//! minute — only while it is not completed, and never past [`WAIT_MOST`].
//!
//! # Where the answers are kept
//!
//! `$XDG_STATE_HOME/ronoctua/security/scans.json` ([`Scans`]): the newest
//! [`SCANS_MOST`] files, by hash, merged by hash so the door and the queue's
//! child — two processes — never erase each other's. The path is kept as the
//! person's data, for the row that names the file.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, object};
use crate::keys::Secret;
use crate::{Host, Refusal, Service, Why};

/// The API's origin and version.
pub const API: &str = "https://www.virustotal.com/api/v3";

/// The most a file may be to go straight to `/api/v3/files`.
pub const DIRECT_MOST: u64 = 32 << 20;

/// The most a file may be at all: the address `upload_url` hands out takes up
/// to this.
pub const UPLOAD_MOST: u64 = 650 << 20;

/// How often a pending analysis is asked again: the host's own pace.
pub const POLL: Duration = Host::VirusTotal.every();

/// How long the queue's child waits for an analysis before it says so.
pub const WAIT_MOST: Duration = Duration::from_secs(30 * 60);

/// The folder under the state root, and the file in it.
pub const SCANS_DIR: &str = "security";
/// See [`SCANS_DIR`].
pub const SCANS_FILE: &str = "scans.json";
/// The most files kept in [`SCANS_FILE`].
pub const SCANS_MOST: usize = 64;

/// The address a person opens to read the whole report in a browser: the
/// hash, and never the key.
#[must_use]
pub fn page_of(sha256: &str) -> String {
    format!("https://www.virustotal.com/gui/file/{sha256}")
}

/// Whether `s` is a SHA-256 as this writes one: 64 lowercase hex digits.
/// Anything else is refused before it becomes a path.
#[must_use]
pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The SHA-256 of the file at `path`, read in 64 KiB pieces, `heard` told the
/// bytes read so far and the file's length after each (answering `false`
/// stops).
///
/// # Errors
///
/// The file cannot be opened or read, or the caller stopped (`Interrupted`).
pub fn sha256_of(path: &Path, heard: &mut dyn FnMut(u64, u64) -> bool) -> std::io::Result<String> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = vec![0u8; 64 << 10];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        context.update(&buffer[..n]);
        read += n as u64;
        if !heard(read, length) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "stopped",
            ));
        }
    }
    Ok(hex(context.finish().as_ref()))
}

/// Bytes as lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// What the engines said, counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// Engines that called it malicious.
    pub malicious: u32,
    /// Engines that called it suspicious.
    pub suspicious: u32,
    /// Engines that found nothing.
    pub undetected: u32,
    /// Engines that called it harmless.
    pub harmless: u32,
}

impl Stats {
    /// How many engines answered at all.
    #[must_use]
    pub const fn engines(self) -> u32 {
        self.malicious + self.suspicious + self.undetected + self.harmless
    }

    /// The verdict these counts are.
    #[must_use]
    pub const fn verdict(self) -> Verdict {
        if self.malicious > 0 {
            Verdict::Malicious
        } else if self.suspicious > 0 {
            Verdict::Suspicious
        } else {
            Verdict::Clean
        }
    }
}

/// The verdict of an analysis, as a closed set (`security-vt-verdict-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No engine found anything.
    Clean,
    /// Some engine found it suspicious, and none malicious.
    Suspicious,
    /// Some engine found it malicious.
    Malicious,
}

impl Verdict {
    /// Every one, in the order the keys are written.
    pub const ALL: [Self; 3] = [Self::Clean, Self::Suspicious, Self::Malicious];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Suspicious => "suspicious",
            Self::Malicious => "malicious",
        }
    }
}

/// A file's last analysis.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Report {
    /// What the engines said.
    pub stats: Stats,
    /// When it was analysed, in seconds since the epoch.
    pub analysed_s: Option<i64>,
}

/// What a hash lookup found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// VirusTotal had analysed it.
    Known(Report),
    /// VirusTotal has never seen it: only an upload would tell.
    Unknown,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FileAnswer {
    data: FileData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FileData {
    attributes: FileAttributes,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FileAttributes {
    last_analysis_stats: Option<Stats>,
    last_analysis_date: Option<i64>,
}

/// The question for a file's report. Pure.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for a hash that is not one.
pub fn file_request(key: &Secret, sha256: &str) -> Result<Request, Refusal> {
    if !is_sha256(sha256) {
        return Err(Refusal::new(Service::VirusTotal, Why::NothingToAsk));
    }
    Ok(
        Request::get(Host::VirusTotal, format!("{API}/files/{sha256}"))
            .secret_header("x-apikey", key.clone()),
    )
}

/// A file's report, out of the answer's body. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for a body that is not a file object with its stats —
/// a file with no analysis yet reads as pending in the queue, never as clean.
pub fn read_file(body: &[u8]) -> Result<Report, Refusal> {
    let answer: FileAnswer = object(Service::VirusTotal, body)?;
    let stats = answer
        .data
        .attributes
        .last_analysis_stats
        .ok_or(Refusal::new(Service::VirusTotal, Why::Malformed))?;
    Ok(Report {
        stats,
        analysed_s: answer.data.attributes.last_analysis_date,
    })
}

/// What VirusTotal knows of the file with this hash. Blocks: a thread.
///
/// # Errors
///
/// A refusal: no key, the key refused, not let, unreachable, malformed.
pub fn lookup(net: &dyn Net, key: &Secret, sha256: &str) -> Result<Checked, Refusal> {
    match answered(net, &file_request(key, sha256)?) {
        Ok(body) => read_file(&body).map(Checked::Known),
        Err(r) if r.why == Why::NothingFound => Ok(Checked::Unknown),
        Err(r) => Err(r),
    }
}

/// The question for an upload address (files above [`DIRECT_MOST`]). Pure.
#[must_use]
pub fn upload_url_request(key: &Secret) -> Request {
    Request::get(Host::VirusTotal, format!("{API}/files/upload_url"))
        .secret_header("x-apikey", key.clone())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct UrlAnswer {
    data: String,
}

/// The upload address out of the answer — refused unless it is `https` on
/// VirusTotal's own host: the person let that host, and the file goes to no
/// other. Pure.
///
/// # Errors
///
/// [`Why::Malformed`].
pub fn read_upload_url(body: &[u8]) -> Result<String, Refusal> {
    let answer: UrlAnswer = object(Service::VirusTotal, body)?;
    let prefix = format!("https://{}/", Host::VirusTotal.name());
    let fine = answer.data.starts_with(&prefix)
        && !answer.data.chars().any(|c| c.is_control() || c == ' ');
    fine.then_some(answer.data)
        .ok_or(Refusal::new(Service::VirusTotal, Why::Malformed))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SentAnswer {
    data: SentData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SentData {
    id: String,
}

/// Whether `s` may be an analysis id in a path: 1–200 of letters, digits and
/// `=`, `_`, `-`, `:` (VirusTotal's are base64).
#[must_use]
pub fn is_analysis(s: &str) -> bool {
    (1..=200).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'=' | b'_' | b'-' | b':'))
}

/// The analysis id an upload answered. Pure.
///
/// # Errors
///
/// [`Why::Malformed`].
pub fn read_sent(body: &[u8]) -> Result<String, Refusal> {
    let answer: SentAnswer = object(Service::VirusTotal, body)?;
    is_analysis(&answer.data.id)
        .then_some(answer.data.id)
        .ok_or(Refusal::new(Service::VirusTotal, Why::Malformed))
}

/// The question for an analysis. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for an id that is not one.
pub fn analysis_request(key: &Secret, id: &str) -> Result<Request, Refusal> {
    if !is_analysis(id) {
        return Err(Refusal::new(Service::VirusTotal, Why::Malformed));
    }
    Ok(
        Request::get(Host::VirusTotal, format!("{API}/analyses/{id}"))
            .secret_header("x-apikey", key.clone()),
    )
}

/// Where an analysis is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Analysis {
    /// Not done yet: ask again after [`POLL`].
    Pending,
    /// Done, with what the engines said.
    Done(Stats),
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AnalysisAnswer {
    data: AnalysisData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AnalysisData {
    attributes: AnalysisAttributes,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AnalysisAttributes {
    status: String,
    stats: Stats,
}

/// Where an analysis is, out of its answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for a status this does not know.
pub fn read_analysis(body: &[u8]) -> Result<Analysis, Refusal> {
    let answer: AnalysisAnswer = object(Service::VirusTotal, body)?;
    let attributes = answer.data.attributes;
    match attributes.status.as_str() {
        "completed" => Ok(Analysis::Done(attributes.stats)),
        "queued" | "in-progress" => Ok(Analysis::Pending),
        _ => Err(Refusal::new(Service::VirusTotal, Why::Malformed)),
    }
}

/// Sends the file at `path` to VirusTotal, streamed, and answers the
/// analysis id. The person's pass for the host is needed and read now; the
/// key goes in `x-apikey` to that host alone. Blocks: a thread (the queue's
/// child).
///
/// # Errors
///
/// [`Why::NotLet`], [`Why::TooLarge`] above [`UPLOAD_MOST`],
/// [`Why::Unreadable`] for a file that cannot be read, and the host's
/// refusals.
#[cfg(feature = "net")]
pub fn send_file(
    net: &dyn Net,
    key: &Secret,
    path: &Path,
    sent: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<String, Refusal> {
    use ronoctua_http::web;
    let refused = |why| Refusal::new(Service::VirusTotal, why);
    let pass = ronoctua_consent::pass(Host::VirusTotal.name()).map_err(|_| refused(Why::NotLet))?;
    let mut file = std::fs::File::open(path).map_err(|_| refused(Why::Unreadable))?;
    let length = file.metadata().map_err(|_| refused(Why::Unreadable))?.len();
    if length > UPLOAD_MOST {
        return Err(refused(Why::TooLarge));
    }
    let url = if length <= DIRECT_MOST {
        format!("{API}/files")
    } else {
        read_upload_url(&answered(net, &upload_url_request(key))?)?
    };
    let _turn = crate::pace::of(Host::VirusTotal).wait();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let headers = [("x-apikey", key.expose())];
    let upload = web::Upload {
        url: &url,
        headers: &headers,
        user_agent: crate::USER_AGENT,
        most: crate::ask::MOST_BYTES,
        // A slow uplink on a big file: each write may wait this long.
        patience: Duration::from_secs(120),
        abort: None,
    };
    let sending = ronoctua_http::multipart::Sending {
        part: ronoctua_http::multipart::Part {
            field: "file",
            filename: &name,
            content_type: "application/octet-stream",
            length,
        },
        file: &mut file,
        sent,
    };
    let got = web::upload_let(&pass, &upload, sending).map_err(|e| {
        tracing::info!(kind = ?e.kind, "the file did not reach VirusTotal");
        refused(crate::internet::why_of_kind(e.kind))
    })?;
    match crate::ask::why_of_status(Service::VirusTotal, got.reply.status) {
        None => read_sent(&got.reply.body),
        Some(why) => Err(refused(why)),
    }
}

/// Where one file's check is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum State {
    /// Its hash is being asked.
    Asking,
    /// VirusTotal has never seen it, and nothing was sent.
    Unknown,
    /// The file is on its way.
    Sending {
        /// Bytes sent.
        sent: u64,
        /// Of.
        of: u64,
    },
    /// Sent, and the analysis is not done.
    Pending,
    /// Analysed.
    Checked {
        /// What the engines said.
        report: Report,
    },
    /// It did not come to an answer.
    Failed {
        /// Why.
        why: Why,
    },
}

/// One file's check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scan {
    /// The file's SHA-256.
    pub sha256: String,
    /// The file, as the person named it — data.
    pub path: String,
    /// When this state was reached, in seconds since the epoch.
    pub at_s: i64,
    /// Where it is.
    #[serde(flatten)]
    pub state: State,
}

/// `scans.json`: the newest first, one per hash.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scans {
    /// The checks.
    pub scans: Vec<Scan>,
}

impl Scans {
    /// The same, with `scan` put first — replacing the one of its hash — and
    /// at most [`SCANS_MOST`] kept. Pure.
    #[must_use]
    pub fn with(mut self, scan: Scan) -> Self {
        self.scans.retain(|s| s.sha256 != scan.sha256);
        self.scans.insert(0, scan);
        self.scans.truncate(SCANS_MOST);
        self
    }

    /// The check of this hash, if there is one.
    #[must_use]
    pub fn of(&self, sha256: &str) -> Option<&Scan> {
        self.scans.iter().find(|s| s.sha256 == sha256)
    }
}

/// The store of `scans.json`, under this instance's state root.
#[must_use]
pub fn scans() -> ronoctua_store::Store<Scans> {
    ronoctua_store::Store::at(
        ronoctua_xdg::state_dir().map(|d| d.join(SCANS_DIR)),
        SCANS_FILE,
    )
}

/// Keeps `scan` in `store`, merged by hash over what the file says NOW — read
/// with `parsed`, so a file another process wrote badly is not erased.
///
/// # Errors
///
/// The file does not parse (and is left as it is), or cannot be written.
pub fn keep(store: &ronoctua_store::Store<Scans>, scan: Scan) -> std::io::Result<()> {
    let now = match store.path() {
        Some(p) if p.exists() => store
            .parsed()
            .ok_or_else(|| std::io::Error::other("scans.json does not parse"))?,
        _ => Scans::default(),
    };
    if let Some(dir) = store.path().as_deref().and_then(Path::parent) {
        std::fs::create_dir_all(dir)?;
    }
    store.write(&now.with(scan))
}

/// Which `every`-percent step of a send `done` of `of` bytes is in — 0 until
/// the first is reached; what the child writes the sending state at. Pure.
#[must_use]
pub fn sending_step(done: u64, of: u64, every: u64) -> u64 {
    done.saturating_mul(100) / of.max(1) / every.max(1)
}

/// What a cancelled send leaves a file's check as: one still being asked
/// about or sent goes back to "never seen, not sent" — VirusTotal got no
/// whole file — and one already sent, analysed or failed is left as it is.
/// Pure.
#[must_use]
pub fn after_cancel(state: &State) -> Option<State> {
    match state {
        State::Asking | State::Sending { .. } => Some(State::Unknown),
        State::Unknown | State::Pending | State::Checked { .. } | State::Failed { .. } => None,
    }
}

/// The queue cancelled the send of `sha256`: its row in `store` stops saying
/// it is on its way ([`after_cancel`]). The child cannot say it — the queue
/// kills its group.
///
/// # Errors
///
/// The file does not parse (and is left as it is), or cannot be written.
pub fn cancelled(store: &ronoctua_store::Store<Scans>, sha256: &str) -> std::io::Result<()> {
    let Some(now) = store.parsed() else {
        return Ok(());
    };
    let Some(scan) = now.of(sha256) else {
        return Ok(());
    };
    match after_cancel(&scan.state) {
        Some(state) => keep(
            store,
            Scan {
                state,
                ..scan.clone()
            },
        ),
        None => Ok(()),
    }
}

/// One line the queue's child says: how far it got, or how it ended. The
/// grammar both sides read, so a change here changes both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Beat {
    /// Hashing the file.
    Hashing {
        /// Bytes read.
        done: u64,
        /// Of.
        of: u64,
    },
    /// Sending the file.
    Sending {
        /// Bytes sent.
        done: u64,
        /// Of.
        of: u64,
    },
    /// Waiting for the analysis.
    Waiting,
    /// Done, with the verdict.
    Done(Verdict),
    /// It did not come to an answer.
    Failed(Why),
}

/// What every beat's line begins with.
pub const BEAT: &str = "RONOCTUA-SCAN";

impl Beat {
    /// The line, with no line break. Pure.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::Hashing { done, of } => format!("{BEAT}\thashing\t{done}\t{of}"),
            Self::Sending { done, of } => format!("{BEAT}\tsending\t{done}\t{of}"),
            Self::Waiting => format!("{BEAT}\twaiting"),
            Self::Done(v) => format!("{BEAT}\tdone\t{}", v.id()),
            Self::Failed(why) => format!("{BEAT}\tfailed\t{}", why.id()),
        }
    }

    /// The beat a line is; `None` for any other line. Pure.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let mut words = line.trim_end().split('\t');
        if words.next()? != BEAT {
            return None;
        }
        let beat = match (words.next()?, words.next(), words.next()) {
            ("hashing", Some(d), Some(o)) => Self::Hashing {
                done: d.parse().ok()?,
                of: o.parse().ok()?,
            },
            ("sending", Some(d), Some(o)) => Self::Sending {
                done: d.parse().ok()?,
                of: o.parse().ok()?,
            },
            ("waiting", None, None) => Self::Waiting,
            ("done", Some(v), None) => Self::Done(Verdict::ALL.into_iter().find(|x| x.id() == v)?),
            ("failed", Some(w), None) => Self::Failed(Why::ALL.into_iter().find(|x| x.id() == w)?),
            _ => return None,
        };
        words.next().is_none().then_some(beat)
    }

    /// Whether it is the last line.
    #[must_use]
    pub const fn is_last(&self) -> bool {
        matches!(self, Self::Done(_) | Self::Failed(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::{Canned, header};

    const SHA: &str = "275a021bbfb6489e54d471899f7db9d1663fc695ec2fe2a2c4538aabf651fd0f";

    /// FAILS IF: a cancelled send leaves its row saying it is on its way, a
    /// file already sent is taken back, or the row's sending state is kept
    /// more often than every tenth.
    #[test]
    fn a_cancel_takes_the_send_back_and_sending_is_kept_coarsely() {
        let dir = std::env::temp_dir().join(format!("ronoctua-vt-cancel-{}", std::process::id()));
        let store = ronoctua_store::Store::at(Some(dir.clone()), SCANS_FILE);
        let scan = |sha: &str, state| Scan {
            sha256: sha.to_owned(),
            path: "/home/p/a.exe".to_owned(),
            at_s: 1,
            state,
        };
        keep(&store, scan("a", State::Sending { sent: 5, of: 10 })).unwrap();
        keep(&store, scan("b", State::Pending)).unwrap();
        cancelled(&store, "a").unwrap();
        cancelled(&store, "b").unwrap();
        cancelled(&store, "none").unwrap();
        let now = store.parsed().unwrap();
        assert_eq!(now.of("a").map(|s| &s.state), Some(&State::Unknown));
        assert_eq!(now.of("b").map(|s| &s.state), Some(&State::Pending));
        assert_eq!(after_cancel(&State::Asking), Some(State::Unknown));
        let steps: Vec<u64> = (0..=100).map(|d| sending_step(d, 100, 10)).collect();
        let mut changes = steps.windows(2).filter(|w| w[0] != w[1]).count();
        changes += 1;
        assert_eq!(changes, 11, "{steps:?}");
        assert_eq!(sending_step(0, 0, 10), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the hash stops being the file's SHA-256 (the EICAR test
    /// string's, which every engine knows), or the progress stops counting.
    #[test]
    fn the_hash_is_computed_here_and_streamed() {
        let dir = std::env::temp_dir().join(format!("ronoctua-vt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("eicar.com");
        std::fs::write(
            &file,
            br"X5O!P%@AP[4\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*",
        )
        .unwrap();
        let mut heard = Vec::new();
        let sha = sha256_of(&file, &mut |d, o| {
            heard.push((d, o));
            true
        })
        .unwrap();
        assert_eq!(sha, SHA);
        assert_eq!(heard, [(68, 68)]);
        assert!(
            sha256_of(&file, &mut |_, _| false).is_err(),
            "a stop is a stop"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the lookup stops being a GET of the hash with the key in its
    /// header (and never in the address), a 404 stops meaning "never seen",
    /// or the counts stop being read.
    #[test]
    fn a_hash_is_looked_up_with_the_key_in_a_header() {
        let url = format!("{API}/files/{SHA}");
        let body = br#"{"data":{"id":"x","type":"file","attributes":{"last_analysis_date":1690000000,
            "last_analysis_stats":{"harmless":0,"type-unsupported":4,"suspicious":1,"malicious":60,"undetected":7}}}}"#;
        let net = Canned::new(&[(url.as_str(), 200, body)]);
        let key = Secret::new("the-key");
        let checked = lookup(&net, &key, SHA).unwrap();
        let Checked::Known(report) = checked else {
            panic!("{checked:?}")
        };
        assert_eq!(report.stats.malicious, 60);
        assert_eq!(report.stats.suspicious, 1);
        assert_eq!(report.stats.undetected, 7);
        assert_eq!(report.stats.engines(), 68);
        assert_eq!(report.stats.verdict(), Verdict::Malicious);
        assert_eq!(report.analysed_s, Some(1_690_000_000));
        let asked = net.only();
        assert_eq!(header(&asked, "x-apikey"), Some("the-key"));
        assert!(!asked.url.contains("the-key"));
        assert!(!format!("{asked:?}").contains("the-key"));

        let net = Canned::new(&[(url.as_str(), 404, br#"{"error":{"code":"NotFoundError"}}"#)]);
        assert_eq!(lookup(&net, &key, SHA).unwrap(), Checked::Unknown);
        let net = Canned::new(&[(url.as_str(), 401, b"{}")]);
        assert_eq!(lookup(&net, &key, SHA).unwrap_err().why, Why::KeyRefused);
        // A HASH THAT IS NOT ONE never becomes a path.
        let net = Canned::new(&[]);
        assert_eq!(
            lookup(&net, &key, "../users").unwrap_err().why,
            Why::NothingToAsk
        );
        assert!(net.asked().is_empty());
    }

    /// FAILS IF: a file with no analysis reads as clean, or a verdict stops
    /// following the loudest engine.
    #[test]
    fn no_analysis_is_never_clean() {
        assert_eq!(
            read_file(br#"{"data":{"attributes":{}}}"#).unwrap_err().why,
            Why::Malformed
        );
        let s = |malicious, suspicious| Stats {
            malicious,
            suspicious,
            undetected: 50,
            harmless: 0,
        };
        assert_eq!(s(0, 0).verdict(), Verdict::Clean);
        assert_eq!(s(0, 2).verdict(), Verdict::Suspicious);
        assert_eq!(s(1, 2).verdict(), Verdict::Malicious);
    }

    /// FAILS IF: an upload address on another host is taken — the file would
    /// go where the person never let it.
    #[test]
    fn an_upload_address_is_virustotals_own() {
        let good = br#"{"data":"https://www.virustotal.com/_ah/upload/AMmfu6b"}"#;
        assert_eq!(
            read_upload_url(good).unwrap(),
            "https://www.virustotal.com/_ah/upload/AMmfu6b"
        );
        for bad in [
            &br#"{"data":"https://evil.example/_ah/upload/x"}"#[..],
            br#"{"data":"http://www.virustotal.com/_ah/upload/x"}"#,
            br#"{"data":"https://www.virustotal.com.evil.example/x"}"#,
            br#"{"data":"https://www.virustotal.com/a b"}"#,
        ] {
            assert_eq!(read_upload_url(bad).unwrap_err().why, Why::Malformed);
        }
    }

    /// FAILS IF: an analysis stops reading as pending until it is completed,
    /// or an id that could become a path of its own is taken.
    #[test]
    fn an_analysis_is_pending_until_completed() {
        let queued = br#"{"data":{"attributes":{"status":"queued","stats":{}}}}"#;
        assert_eq!(read_analysis(queued).unwrap(), Analysis::Pending);
        let going = br#"{"data":{"attributes":{"status":"in-progress"}}}"#;
        assert_eq!(read_analysis(going).unwrap(), Analysis::Pending);
        let done = br#"{"data":{"attributes":{"status":"completed","stats":{"malicious":0,"suspicious":0,"undetected":70,"harmless":0}}}}"#;
        let Analysis::Done(stats) = read_analysis(done).unwrap() else {
            panic!()
        };
        assert_eq!(stats.verdict(), Verdict::Clean);
        assert_eq!(
            read_analysis(br#"{"data":{"attributes":{"status":"lost"}}}"#)
                .unwrap_err()
                .why,
            Why::Malformed
        );
        assert_eq!(
            read_sent(br#"{"data":{"type":"analysis","id":"NjY0MjRlOTVj=="}}"#).unwrap(),
            "NjY0MjRlOTVj=="
        );
        assert!(read_sent(br#"{"data":{"id":"../../x"}}"#).is_err());
        assert!(analysis_request(&Secret::new("k"), "a/b").is_err());
        assert!(
            POLL >= Duration::from_secs(15),
            "the free key's four a minute"
        );
    }

    /// FAILS IF: a check stops replacing the one of its hash, stops going
    /// first, or the file grows without a bound.
    #[test]
    fn scans_are_merged_by_hash_and_bounded() {
        let scan = |sha: &str, state: State| Scan {
            sha256: sha.to_owned(),
            path: "/home/p/a.exe".to_owned(),
            at_s: 1,
            state,
        };
        let mut s = Scans::default()
            .with(scan("a", State::Unknown))
            .with(scan("b", State::Pending));
        s = s.with(scan("a", State::Failed { why: Why::TooLarge }));
        let order: Vec<&str> = s.scans.iter().map(|x| x.sha256.as_str()).collect();
        assert_eq!(order, ["a", "b"]);
        assert_eq!(
            s.of("a").map(|x| &x.state),
            Some(&State::Failed { why: Why::TooLarge })
        );
        for n in 0..(SCANS_MOST + 5) {
            s = s.with(scan(&n.to_string(), State::Unknown));
        }
        assert_eq!(s.scans.len(), SCANS_MOST);
        let text = serde_json::to_string(&s.scans[0]).unwrap();
        assert!(text.contains(r#""state":"unknown""#), "{text}");
        let back: Scan = serde_json::from_str(&text).unwrap();
        assert_eq!(back, s.scans[0]);
    }

    /// FAILS IF: a beat and its line stop being one grammar both ways, or a
    /// line that is not one is taken for one.
    #[test]
    fn a_beat_is_one_line_both_ways() {
        let beats = [
            Beat::Hashing { done: 1, of: 2 },
            Beat::Sending { done: 3, of: 4 },
            Beat::Waiting,
            Beat::Done(Verdict::Suspicious),
            Beat::Failed(Why::NotLet),
        ];
        for b in beats {
            assert_eq!(Beat::parse(&b.line()), Some(b.clone()), "{}", b.line());
        }
        for other in [
            "hashing\t1\t2",
            "RONOCTUA-SCAN\tdone\tunsure",
            "RONOCTUA-SCAN\twaiting\textra",
            "RONOCTUA-SCAN\tsending\tx\t1",
        ] {
            assert_eq!(Beat::parse(other), None, "{other}");
        }
        assert!(Beat::Done(Verdict::Clean).is_last() && !Beat::Waiting.is_last());
    }

    #[test]
    fn the_page_names_the_hash_and_never_a_key() {
        assert_eq!(
            page_of(SHA),
            format!("https://www.virustotal.com/gui/file/{SHA}")
        );
        assert!(is_sha256(SHA) && !is_sha256(&SHA.to_uppercase()) && !is_sha256("ab"));
    }
}
