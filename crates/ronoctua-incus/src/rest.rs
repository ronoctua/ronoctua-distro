//! The daemon's REST API, over its unix socket.
//!
//! # One way in
//!
//! Every call — a listing, an action, a backup streamed to a file — is
//! [`exchange`]: connect, frame with `ronoctua-http`, read the answer. The
//! answer is Incus's envelope (`shared/api/response.go`): `sync` with the
//! value in `metadata`, `async` with an operation to follow, or `error`
//! with a code and the daemon's words. The words go to the log, the caller
//! gets a [`Why`].
//!
//! # Operations
//!
//! Anything that takes time is an operation. It is followed by long-polling
//! `/1.0/operations/<id>/wait` a second at a time ([`wait`]): between two
//! polls the caller hears where it is ([`Operation::step`], the daemon's
//! `progress`) and may stop it, which cancels it on the daemon.

use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use ronoctua_http::streamed::{self, Body, Moved};
use ronoctua_http::{Call, Reply};
use serde::Deserialize;
use serde_json::Value;

use crate::{Refusal, Step, Why};

/// The `Host` a call names. The socket has no name; the daemon reads none.
pub(crate) const HOST: &str = "incus";

/// How long a listing or a small call may take: a socket on this machine,
/// but `recursion=2` asks every instance for its state.
pub(crate) const READ: Duration = Duration::from_secs(15);

/// The most a JSON answer may be.
pub(crate) const JSON_MOST_BYTES: u64 = 64 << 20;

/// How long one poll of an operation waits on the daemon, in seconds.
const WAIT_SLICE_S: u64 = 1;

/// The longest the daemon's words are logged.
const SAID_MOST_BYTES: usize = 512;

/// Why a followed call ended without its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Halt {
    /// Refused — by the daemon, or before asking it.
    Refused(Refusal),
    /// Stopped by whoever was hearing it; the operation was cancelled.
    Stopped,
}

impl From<Refusal> for Halt {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// The socket, open, with a deadline on every read and write.
///
/// # Errors
///
/// [`Why::NotInGroup`] when the socket refuses this person (its group is
/// `incus-admin`), [`Why::NoIncus`] when nothing is there.
pub(crate) fn connect(socket: &Path, patience: Duration) -> Result<UnixStream, Refusal> {
    let stream = UnixStream::connect(socket).map_err(|e| {
        Refusal::of(if e.kind() == std::io::ErrorKind::PermissionDenied {
            Why::NotInGroup
        } else {
            Why::NoIncus
        })
    })?;
    let deadline = Some(patience.max(Duration::from_millis(1)));
    stream
        .set_read_timeout(deadline)
        .and_then(|()| stream.set_write_timeout(deadline))
        .map_err(|_| Refusal::of(Why::NoIncus))?;
    Ok(stream)
}

/// One call: the request, a streamed body when given, and the answer — a 2xx
/// body to `to`, any other kept in the reply.
///
/// # Errors
///
/// As [`connect`]; [`Why::Failed`] when the exchange broke; [`Halt::Stopped`]
/// when `heard` said so.
pub(crate) fn exchange(
    socket: &Path,
    call: &Call<'_>,
    body: Option<Body<'_>>,
    to: &mut dyn std::io::Write,
    most: u64,
    heard: &mut dyn FnMut(Moved) -> bool,
) -> Result<Reply, Halt> {
    let stream = connect(socket, call.patience)?;
    let mut stopped = false;
    let mut hearing = |m: Moved| {
        let on = heard(m);
        stopped |= !on;
        on
    };
    streamed::exchange(stream, call, body, to, most, &mut hearing).map_err(|e| {
        if stopped {
            Halt::Stopped
        } else {
            tracing::warn!(path = call.path, error = %e, "the exchange with incus broke");
            Halt::Refused(Refusal::of(Why::Failed))
        }
    })
}

/// One call whose answer is JSON, kept whole.
///
/// # Errors
///
/// As [`exchange`].
pub(crate) fn call(
    socket: &Path,
    method: &str,
    path: &str,
    json: Option<&str>,
    patience: Duration,
) -> Result<Reply, Refusal> {
    call_with(socket, method, path, json, &[], patience)
}

/// [`call`], with headers.
///
/// # Errors
///
/// As [`exchange`].
pub(crate) fn call_with(
    socket: &Path,
    method: &str,
    path: &str,
    json: Option<&str>,
    headers: &[(&str, &str)],
    patience: Duration,
) -> Result<Reply, Refusal> {
    let mut call = Call::new(method, path, HOST)
        .with_headers(headers)
        .waiting(patience);
    if let Some(json) = json {
        call = call.with_body(json);
    }
    let mut out = Vec::new();
    let mut reply = exchange(socket, &call, None, &mut out, JSON_MOST_BYTES, &mut |_| {
        true
    })
    .map_err(|h| match h {
        Halt::Refused(r) => r,
        Halt::Stopped => Refusal::of(Why::Failed),
    })?;
    if reply.went_through() {
        reply.body = out;
    }
    Ok(reply)
}

/// What the daemon answered, out of its envelope.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Answered {
    /// The value, now.
    Sync(Value),
    /// An operation to follow: its id, and the operation as it started.
    Async(String, Value),
}

/// The envelope, as `shared/api/response.go` writes it.
#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    operation: String,
    #[serde(default)]
    error_code: i64,
    #[serde(default)]
    error: String,
    #[serde(default)]
    metadata: Value,
}

/// The answer a reply carries. Pure.
///
/// # Errors
///
/// The daemon's refusal ([`why_of`]); [`Why::Failed`] for an answer that is
/// not its envelope.
pub(crate) fn answered(reply: &Reply) -> Result<Answered, Refusal> {
    let Ok(envelope) = serde_json::from_slice::<Envelope>(&reply.body) else {
        tracing::warn!(
            status = reply.status,
            "incus answered something that is not its envelope"
        );
        return Err(Refusal::of(if reply.status == 404 {
            Why::Gone
        } else {
            Why::Failed
        }));
    };
    match envelope.kind.as_str() {
        "sync" if reply.went_through() => Ok(Answered::Sync(envelope.metadata)),
        "async" if reply.went_through() => {
            let id = operation_id(&envelope.operation)
                .or_else(|| {
                    envelope
                        .metadata
                        .get("id")
                        .and_then(Value::as_str)
                        .and_then(operation_id)
                })
                .ok_or_else(|| {
                    tracing::warn!("incus answered an operation with no id");
                    Refusal::of(Why::Failed)
                })?;
            Ok(Answered::Async(id, envelope.metadata))
        }
        _ => {
            let code = if envelope.error_code == 0 {
                i64::from(reply.status)
            } else {
                envelope.error_code
            };
            Err(refused(code, &envelope.error))
        }
    }
}

/// An operation's id out of its path (`/1.0/operations/<uuid>`) or itself;
/// `None` for anything that is not one — it goes into a path.
fn operation_id(said: &str) -> Option<String> {
    let id = said.rsplit('/').next().unwrap_or("");
    let fits = |c: char| c.is_ascii_hexdigit() || c == '-';
    (!id.is_empty() && id.len() <= 64 && id.chars().all(fits)).then(|| id.to_owned())
}

/// The value of a sync answer.
///
/// # Errors
///
/// As [`answered`]; [`Why::Failed`] for an operation where a value was due.
pub(crate) fn sync(reply: &Reply) -> Result<Value, Refusal> {
    match answered(reply)? {
        Answered::Sync(value) => Ok(value),
        Answered::Async(..) => {
            tracing::warn!("incus answered an operation where a value was due");
            Err(Refusal::of(Why::Failed))
        }
    }
}

/// A refusal of the daemon's, logged with its words.
pub(crate) fn refused(code: i64, said: &str) -> Refusal {
    let why = why_of(code, said);
    tracing::warn!(code, why = why.id(), said = cut(said), "incus refused");
    Refusal::of(why)
}

/// What a refusal of the daemon's is — MEASURED on the pinned daemon's own
/// words (`internal/server/response`, `operationlock`). Pure.
#[must_use]
pub(crate) fn why_of(code: i64, said: &str) -> Why {
    let said = said.to_ascii_lowercase();
    if said.contains("is busy running") || (said.contains("busy") && said.contains("operation")) {
        Why::Busy
    } else if code == 404 || said.contains("not found") {
        Why::Gone
    } else if code == 403 {
        Why::NotInGroup
    } else if code == 409 || said.contains("already exists") {
        // A NAME THAT IS TAKEN is a name Incus does not take.
        Why::Invalid
    } else {
        Why::Failed
    }
}

/// The daemon's words, cut for the log on a character boundary.
fn cut(said: &str) -> &str {
    let said = said.trim();
    if said.len() <= SAID_MOST_BYTES {
        return said;
    }
    let mut end = SAID_MOST_BYTES;
    while !said.is_char_boundary(end) {
        end -= 1;
    }
    &said[..end]
}

/// An operation's state, as its `status_code` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpStatus {
    /// Waiting to begin.
    Pending,
    /// Under way.
    Running,
    /// Being cancelled.
    Cancelling,
    /// Done.
    Success,
    /// Failed (its `err` says why, in the daemon's words).
    Failure,
    /// Cancelled.
    Cancelled,
    /// Anything else.
    Other,
}

impl OpStatus {
    /// The status a `status_code` says (`shared/api/status_code.go`).
    #[must_use]
    pub const fn of_code(code: i64) -> Self {
        match code {
            100 | 105 => Self::Pending,
            101 | 103 => Self::Running,
            104 => Self::Cancelling,
            200 => Self::Success,
            400 => Self::Failure,
            401 => Self::Cancelled,
            _ => Self::Other,
        }
    }

    /// Whether it will not change again.
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Success | Self::Failure | Self::Cancelled)
    }
}

/// One of the daemon's operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Its id.
    pub id: String,
    /// What it is, in the daemon's words — data.
    pub description: String,
    /// Where it is.
    pub status: OpStatus,
    /// How far, when the daemon says.
    pub step: Option<Step>,
    /// The daemon's own words for how far (`rootfs: 23% (12.30MB/s)`) — data.
    pub said: Option<String>,
    /// Why it failed, in the daemon's words — data, for the log.
    pub err: String,
    /// Whether it may be cancelled.
    pub may_cancel: bool,
    /// What it touches, by kind (`instances`, `backups`…): API paths.
    pub resources: BTreeMap<String, Vec<String>>,
}

/// The operation, as `shared/api/operation.go` writes it.
#[derive(Debug, Deserialize)]
struct Raw {
    #[serde(default)]
    id: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    status_code: i64,
    #[serde(default)]
    resources: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    metadata: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    may_cancel: bool,
    #[serde(default)]
    err: String,
}

/// An operation out of its JSON. Pure.
#[must_use]
pub(crate) fn operation_of(value: &Value) -> Option<Operation> {
    let raw = Raw::deserialize(value).ok()?;
    let (step, said) = raw.metadata.as_ref().map_or((None, None), progress_of);
    Some(Operation {
        id: operation_id(&raw.id)?,
        description: raw.description,
        status: OpStatus::of_code(raw.status_code),
        step,
        said,
        err: raw.err,
        may_cancel: raw.may_cancel,
        resources: raw.resources.unwrap_or_default(),
    })
}

/// Where an operation is, out of its metadata. Pure.
///
/// The daemon says it twice (`internal/server/operations/util.go`): a
/// `progress` object (`stage`, `percent`, `processed`, `speed`, all strings)
/// and a `<stage>_progress` sentence; an image download says only the
/// sentence (`download_progress: "rootfs: 23% (12.30MB/s)"`). The object
/// wins; the sentence's percentage is read when it is all there is.
pub(crate) fn progress_of(
    metadata: &serde_json::Map<String, Value>,
) -> (Option<Step>, Option<String>) {
    let progress = metadata.get("progress").and_then(Value::as_object);
    let number = |key: &str| {
        progress
            .and_then(|p| p.get(key))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
            })
            .and_then(|s| s.trim().parse::<u64>().ok())
    };
    // THE STAGE THE OBJECT NAMES picks the sentence; else the first, by name.
    let stage = progress
        .and_then(|p| p.get("stage"))
        .and_then(Value::as_str)
        .map(|s| format!("{s}_progress"));
    let said = stage
        .as_deref()
        .and_then(|k| metadata.get(k))
        .or_else(|| {
            metadata
                .iter()
                .filter(|(k, v)| k.ends_with("_progress") && v.is_string())
                .map(|(_, v)| v)
                .next()
        })
        .and_then(Value::as_str)
        .map(str::to_owned);
    let step = if let Some(percent) = number("percent") {
        Some(Step::Percent(u8::try_from(percent.min(100)).unwrap_or(100)))
    } else if let Some(done) = number("processed") {
        Some(Step::Bytes { done, total: None })
    } else {
        said.as_deref().and_then(percent_in).map(Step::Percent)
    };
    (step, said)
}

/// The percentage a sentence says (`rootfs: 23% (12.30MB/s)` → 23). Pure.
fn percent_in(said: &str) -> Option<u8> {
    let at = said.find('%')?;
    let digits: String = said[..at]
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse::<u8>().ok().filter(|n| *n <= 100)
}

/// Follows an operation to its end, a poll at a time.
///
/// `heard` is told the operation after every poll and may stop it — the
/// operation is then cancelled on the daemon. `until` bounds a call that is
/// not the queue's: an action is waited on for so long and then left to the
/// daemon, refused [`Why::Busy`].
///
/// # Errors
///
/// [`Halt::Refused`] for a refusal or a failed operation; [`Halt::Stopped`].
pub(crate) fn wait(
    socket: &Path,
    id: &str,
    until: Option<Instant>,
    heard: &mut dyn FnMut(&Operation) -> bool,
) -> Result<Operation, Halt> {
    let id = operation_id(id).ok_or(Halt::Refused(Refusal::of(Why::Failed)))?;
    let path = format!("/1.0/operations/{id}/wait?timeout={WAIT_SLICE_S}");
    loop {
        let reply = call(
            socket,
            "GET",
            &path,
            None,
            READ + Duration::from_secs(WAIT_SLICE_S),
        )?;
        let op = operation_of(&sync(&reply)?).ok_or_else(|| {
            tracing::warn!("incus answered a wait with something that is not an operation");
            Halt::Refused(Refusal::of(Why::Failed))
        })?;
        match op.status {
            OpStatus::Success => return Ok(op),
            OpStatus::Failure => return Err(Halt::Refused(refused(0, &op.err))),
            OpStatus::Cancelled => {
                tracing::warn!(id, "incus cancelled the operation");
                return Err(Halt::Refused(Refusal::of(Why::Failed)));
            }
            OpStatus::Pending | OpStatus::Running | OpStatus::Cancelling | OpStatus::Other => {}
        }
        if !heard(&op) {
            cancel(socket, &id);
            return Err(Halt::Stopped);
        }
        if until.is_some_and(|u| Instant::now() >= u) {
            tracing::warn!(
                id,
                "an operation outlived its caller's patience; it goes on"
            );
            return Err(Halt::Refused(Refusal::of(Why::Busy)));
        }
    }
}

/// Asks the daemon to cancel an operation. What it answers changes nothing
/// here: the caller is leaving either way.
pub(crate) fn cancel(socket: &Path, id: &str) {
    let Some(id) = operation_id(id) else { return };
    if let Err(r) = call(
        socket,
        "DELETE",
        &format!("/1.0/operations/{id}"),
        None,
        READ,
    ) {
        tracing::warn!(id, why = r.why.id(), "the operation could not be cancelled");
    }
}

/// A sync call's value.
///
/// # Errors
///
/// As [`call`] and [`sync`].
pub(crate) fn get(socket: &Path, path: &str) -> Result<Value, Refusal> {
    sync(&call(socket, "GET", path, None, READ)?)
}

/// A call that starts an operation (or answers at once), followed to its end.
///
/// # Errors
///
/// As [`call`], [`answered`] and [`wait`].
pub(crate) fn operate(
    socket: &Path,
    method: &str,
    path: &str,
    json: Option<&str>,
    until: Option<Instant>,
    heard: &mut dyn FnMut(&Operation) -> bool,
) -> Result<Option<Operation>, Halt> {
    let reply = call(socket, method, path, json, READ)?;
    match answered(&reply)? {
        Answered::Sync(_) => Ok(None),
        Answered::Async(id, _) => wait(socket, &id, until, heard).map(Some),
    }
}

/// A value as it goes into a query string: every byte but the unreserved
/// ones and `/` percent-encoded. Pure.
#[must_use]
pub(crate) fn query_value(value: &str) -> String {
    encoded(value, true)
}

/// A value as ONE segment of a path: every byte but the unreserved ones
/// percent-encoded, `/` included. Pure.
#[must_use]
pub(crate) fn path_segment(value: &str) -> String {
    encoded(value, false)
}

/// Every byte but the unreserved ones (and `/`, when `slash`) percent-encoded.
fn encoded(value: &str, slash: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~')
            || (slash && b == b'/')
        {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn reply(status: u16, body: &str) -> Reply {
        Reply {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn the_three_envelopes_say_three_things() {
        let value = sync(&reply(200, fixtures::PROFILES)).unwrap();
        assert!(value.is_array());
        match answered(&reply(202, fixtures::CREATE_ASYNC)).unwrap() {
            Answered::Async(id, _) => assert_eq!(id, "6916c8a6-9b7d-4abd-90b3-aedfec7f7492"),
            other @ Answered::Sync(_) => panic!("{other:?}"),
        }
        assert_eq!(
            answered(&reply(404, fixtures::NOT_FOUND)),
            Err(Refusal::of(Why::Gone))
        );
        assert_eq!(
            answered(&reply(400, fixtures::BUSY)),
            Err(Refusal::of(Why::Busy))
        );
        assert_eq!(
            answered(&reply(502, "<html>bad gateway</html>")),
            Err(Refusal::of(Why::Failed)),
            "something that is not the envelope is a failure, not a panic"
        );
    }

    #[test]
    fn a_refusal_is_read_from_the_daemon_s_words() {
        assert_eq!(
            why_of(400, "Instance is busy running a \"stop\" operation"),
            Why::Busy
        );
        assert_eq!(why_of(404, "Instance not found"), Why::Gone);
        assert_eq!(
            why_of(0, "Failed getting image: Image not found"),
            Why::Gone
        );
        assert_eq!(why_of(403, "not authorized"), Why::NotInGroup);
        assert_eq!(why_of(409, "Instance \"web\" already exists"), Why::Invalid);
        assert_eq!(why_of(500, "Failed to mount"), Why::Failed);
    }

    #[test]
    fn an_operation_says_where_it_is_both_ways() {
        let running =
            operation_of(&serde_json::from_str::<Value>(fixtures::OPERATION_DOWNLOADING).unwrap())
                .unwrap();
        assert_eq!(running.status, OpStatus::Running);
        assert_eq!(
            running.step,
            Some(Step::Percent(23)),
            "the sentence when it is all there is"
        );
        assert_eq!(running.said.as_deref(), Some("rootfs: 23% (12.30MB/s)"));
        assert!(running.may_cancel);
        let unpacking =
            operation_of(&serde_json::from_str::<Value>(fixtures::OPERATION_UNPACKING).unwrap())
                .unwrap();
        assert_eq!(unpacking.step, Some(Step::Percent(61)), "the object wins");
        assert_eq!(
            unpacking.said.as_deref(),
            Some("Unpacking image: 61% (310.40MB/s)")
        );
        let backup =
            operation_of(&serde_json::from_str::<Value>(fixtures::OPERATION_BACKUP_DONE).unwrap())
                .unwrap();
        assert_eq!(backup.status, OpStatus::Success);
        assert_eq!(
            backup.resources.get("backups").map(Vec::as_slice),
            Some(&["/1.0/instances/web/backups/ronoctua-export".to_owned()][..])
        );
        let failed =
            operation_of(&serde_json::from_str::<Value>(fixtures::OPERATION_FAILED).unwrap())
                .unwrap();
        assert_eq!(failed.status, OpStatus::Failure);
        assert!(failed.err.contains("Failed creating instance"));
        assert_eq!(why_of(0, &failed.err), Why::Failed);
        assert!(OpStatus::Cancelled.is_final() && !OpStatus::Cancelling.is_final());
    }

    #[test]
    fn an_operation_id_never_carries_a_path() {
        assert_eq!(
            operation_id("/1.0/operations/6916c8a6-9b7d-4abd-90b3-aedfec7f7492").as_deref(),
            Some("6916c8a6-9b7d-4abd-90b3-aedfec7f7492")
        );
        assert_eq!(operation_id("../../1.0/instances"), None);
        assert_eq!(operation_id(""), None);
        assert_eq!(percent_in("metadata: 100% (5.51GB/s)"), Some(100));
        assert_eq!(percent_in("no percentage"), None);
    }

    #[test]
    fn a_query_value_cannot_add_a_parameter() {
        assert_eq!(query_value("/etc/hosts"), "/etc/hosts");
        assert_eq!(query_value("/a b&c=d#é"), "/a%20b%26c%3Dd%23%C3%A9");
        assert_eq!(path_segment("a/b"), "a%2Fb", "a segment is one");
        assert_eq!(path_segment("v1.0_x~y-z"), "v1.0_x~y-z");
    }

    #[test]
    fn a_missing_socket_is_no_incus_and_answers_at_once() {
        let at = std::env::temp_dir().join(format!("ronoctua-incus-none-{}", std::process::id()));
        let started = Instant::now();
        assert_eq!(get(&at, "/1.0").unwrap_err(), Refusal::of(Why::NoIncus));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
