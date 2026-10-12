//! Long work, followed to its end: what `ronoctua-incus-job` does for the
//! queue.
//!
//! # Why a child of the queue
//!
//! A launch from a remote image is minutes of download and an export is a
//! file the size of a disk: either outlives the door that asked. The queue
//! owns the row, its cancel and its group; this owns what the work IS — the
//! calls, the operation followed a poll at a time, the bytes streamed to and
//! from a file — and prints where it is as [`Line`]s.
//!
//! # A cancel is the daemon's too
//!
//! The queue's cancel is `SIGTERM`. The job then cancels the daemon's
//! operation (`DELETE /1.0/operations/<id>`), removes the half-written file
//! it was making, and leaves: [`Ended::Stopped`].
//!
//! # Consent, minted here
//!
//! A [`ronoctua_consent::Pass`] cannot cross a process: the job mints its own
//! from the person's `consent.json` (`minter`), at the moment it asks — for
//! every host the remote of a launch or of an image copy asks
//! ([`Job::needs_hosts`]), and for nothing else.

use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ronoctua_consent::Pass;
use ronoctua_http::Call;
use ronoctua_http::streamed::{BYTES, Body, Moved};
use serde_json::json;

use crate::client::{self, instance_path};
use crate::rest::{self, Answered, Halt, Operation};
use crate::{Incus, Job, Line, Refusal, Step, Why};

/// How the job ended without its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// Refused — by the daemon, or before asking it.
    Refused(Refusal),
    /// Stopped by the queue; the daemon's operation was cancelled.
    Stopped,
}

impl From<Halt> for Ended {
    fn from(halt: Halt) -> Self {
        match halt {
            Halt::Refused(r) => Self::Refused(r),
            Halt::Stopped => Self::Stopped,
        }
    }
}

impl From<Refusal> for Ended {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// How long a backup the job made may outlive it: a job killed between
/// making it and deleting it leaves one, and the daemon deletes it then.
const BACKUP_LIFE_S: u64 = 24 * 60 * 60;

/// How many bytes pass between two lines of a stream.
const BYTES_BETWEEN_LINES: u64 = 1 << 20;

/// How long a streamed call waits on a quiet socket. Not the length of the
/// transfer: the deadline is per read and per write.
const STREAM_PATIENCE: std::time::Duration = std::time::Duration::from_secs(120);

/// The most a file pulled or an export may be: what any disk here holds.
const FILE_MOST_BYTES: u64 = 1 << 50;

/// What the job hears and says while it runs.
pub struct Hearing<'a> {
    /// Whether the queue asked it to stop.
    pub stop: &'a dyn Fn() -> bool,
    /// Where it says where it is.
    pub say: &'a mut dyn FnMut(&Line),
    /// The last step said, so a line is printed only when it changes.
    last: Option<Step>,
    /// The last words said.
    said: Option<String>,
}

impl std::fmt::Debug for Hearing<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hearing")
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl<'a> Hearing<'a> {
    /// A hearing that stops when `stop` says so and says through `say`.
    pub fn new(stop: &'a dyn Fn() -> bool, say: &'a mut dyn FnMut(&Line)) -> Self {
        Self {
            stop,
            say,
            last: None,
            said: None,
        }
    }

    /// An operation moved: its step and its words, when they changed.
    fn operation(&mut self, op: &Operation) -> bool {
        if let Some(step) = op.step
            && self.last != Some(step)
        {
            self.last = Some(step);
            (self.say)(&Line::Step(step));
        }
        if op.said.is_some() && op.said != self.said {
            self.said.clone_from(&op.said);
            if let Some(said) = &op.said {
                (self.say)(&Line::Said(said.clone()));
            }
        }
        !(self.stop)()
    }

    /// A stream moved: a line every [`BYTES_BETWEEN_LINES`], and the last.
    fn moved(&mut self, moved: Moved) -> bool {
        let (done, total) = match moved {
            Moved::Sent { done, total } => (done, Some(total)),
            Moved::Received { done, total } => (done, total),
        };
        let before = match self.last {
            Some(Step::Bytes { done, .. }) => done,
            _ => 0,
        };
        if done < before || done - before >= BYTES_BETWEEN_LINES || Some(done) == total {
            let step = Step::Bytes { done, total };
            self.last = Some(step);
            (self.say)(&Line::Step(step));
        }
        !(self.stop)()
    }
}

/// Runs one job to its end.
///
/// `minter` answers a pass for a host the person let (`ronoctua_consent::pass`
/// in the product; a test's own consent file in the tests).
///
/// # Errors
///
/// [`Ended::Refused`], [`Ended::Stopped`].
pub fn run(
    incus: &Incus,
    job: &Job,
    minter: &dyn Fn(&str) -> Option<Pass>,
    hearing: &mut Hearing<'_>,
) -> Result<(), Ended> {
    // CONSENT BEFORE ANYTHING IS ASKED OF THE DAEMON: a pass for every host
    // the remote's client asks, or the job is refused.
    let passes = job
        .needs_hosts()
        .iter()
        .map(|host| minter(host).ok_or(Refusal::of(Why::NotLet)))
        .collect::<Result<Vec<Pass>, Refusal>>()?;
    match job {
        Job::Launch { create } => {
            let id = incus.start_create(&passes, create, true)?;
            follow(incus, &id, hearing)
        }
        Job::ImageCopy {
            server,
            protocol,
            alias,
            kind,
        } => {
            let body = client::image_copy_of(&passes, server, *protocol, alias, *kind)?;
            operate(incus, "POST", "/1.0/images", &body.to_string(), hearing)
        }
        Job::Publish { name, alias } => {
            let _ = instance_path(name)?;
            if !is_alias(alias) {
                return Err(Refusal::of(Why::Invalid).into());
            }
            let body = json!({
                "source": { "type": "instance", "name": name },
                "aliases": [{ "name": alias, "description": "" }],
                "public": false,
            });
            operate(incus, "POST", "/1.0/images", &body.to_string(), hearing)
        }
        Job::Export { name, to } => export(incus, name, to, hearing),
        Job::Import { from } => import(incus, from, hearing),
        Job::Push { name, from, to } => push(incus, name, from, to, hearing),
        Job::Pull { name, from, to } => pull(incus, name, from, to, hearing),
    }
}

/// Whether an image alias may be asked for: a line of printable text, no
/// longer than the daemon keeps.
fn is_alias(alias: &str) -> bool {
    !alias.trim().is_empty()
        && alias.len() <= 255
        && alias.chars().all(|c| !c.is_control() && !c.is_whitespace())
}

/// Follows an operation to its end, saying where it is.
fn follow(incus: &Incus, id: &str, hearing: &mut Hearing<'_>) -> Result<(), Ended> {
    rest::wait(&incus.socket, id, None, &mut |op| hearing.operation(op))?;
    Ok(())
}

/// A call that starts an operation, followed.
fn operate(
    incus: &Incus,
    method: &str,
    path: &str,
    body: &str,
    hearing: &mut Hearing<'_>,
) -> Result<(), Ended> {
    rest::operate(&incus.socket, method, path, Some(body), None, &mut |op| {
        hearing.operation(op)
    })?;
    Ok(())
}

/// A file made new, never over one: `create_new`, mode 0600 from its first
/// byte (a backup holds the whole instance — its secrets too).
fn made(to: &Path) -> Result<File, Ended> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(to)
        .map_err(|e| {
            tracing::warn!(to = %to.display(), error = %e, "the file could not be made");
            Ended::Refused(Refusal::of(
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Why::Invalid
                } else {
                    Why::Failed
                },
            ))
        })
}

/// A regular file opened to be sent, and its length.
fn sent(from: &Path) -> Result<(File, u64), Ended> {
    let file = File::open(from).map_err(|e| {
        tracing::warn!(from = %from.display(), error = %e, "the file could not be opened");
        Ended::Refused(Refusal::of(Why::Gone))
    })?;
    let meta = file.metadata().map_err(|_| Refusal::of(Why::Failed))?;
    if !meta.is_file() {
        return Err(Refusal::of(Why::Invalid).into());
    }
    Ok((file, meta.len()))
}

/// A path inside an instance, as the daemon takes it: absolute, one line.
fn inside(path: &str) -> Result<String, Ended> {
    if path.starts_with('/') && !path.chars().any(char::is_control) {
        Ok(rest::query_value(path))
    } else {
        Err(Refusal::of(Why::Invalid).into())
    }
}

/// What a half-made file becomes when the job did not end well: nothing.
fn unless_done<T>(result: Result<T, Ended>, to: &Path) -> Result<T, Ended> {
    if result.is_err() {
        let _ = std::fs::remove_file(to);
    }
    result
}

/// Export: a backup made by the daemon, streamed into a new file, deleted.
fn export(incus: &Incus, name: &str, to: &Path, hearing: &mut Hearing<'_>) -> Result<(), Ended> {
    let path = instance_path(name)?;
    let mut file = made(to)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let backup = format!("ronoctua-export-{now}");
    let body = json!({
        "name": backup,
        "expires_at": rfc3339(now + BACKUP_LIFE_S),
        "instance_only": false,
        "optimized_storage": false,
    });
    let made_backup = rest::operate(
        &incus.socket,
        "POST",
        &format!("{path}/backups"),
        Some(&body.to_string()),
        None,
        &mut |op| hearing.operation(op),
    );
    let result = made_backup.map_err(Ended::from).and_then(|_| {
        let at = format!("{path}/backups/{backup}/export");
        let call = Call::new("GET", &at, rest::HOST).waiting(STREAM_PATIENCE);
        let reply = rest::exchange(
            &incus.socket,
            &call,
            None,
            &mut file,
            FILE_MOST_BYTES,
            &mut |m| hearing.moved(m),
        )?;
        if reply.went_through() {
            file.flush().map_err(|_| Refusal::of(Why::Failed))?;
            Ok(())
        } else {
            Err(Ended::from(Halt::from(refused_reply(&reply))))
        }
    });
    // THE BACKUP GOES whatever happened: it is a copy of the instance the
    // daemon would otherwise keep until it expires.
    let _ = rest::operate(
        &incus.socket,
        "DELETE",
        &format!("{path}/backups/{backup}"),
        None,
        None,
        &mut |_| true,
    );
    unless_done(result, to)
}

/// The refusal a reply carries.
fn refused_reply(reply: &ronoctua_http::Reply) -> Refusal {
    rest::answered(reply)
        .err()
        .unwrap_or(Refusal::of(Why::Failed))
}

/// Import: a backup file streamed to the daemon, which makes the instance.
fn import(incus: &Incus, from: &Path, hearing: &mut Hearing<'_>) -> Result<(), Ended> {
    let (mut file, length) = sent(from)?;
    let call = Call {
        content_type: BYTES,
        ..Call::new("POST", "/1.0/instances", rest::HOST).waiting(STREAM_PATIENCE)
    };
    let mut answer = Vec::new();
    let mut reply = rest::exchange(
        &incus.socket,
        &call,
        Some(Body {
            from: &mut file,
            length,
        }),
        &mut answer,
        rest::JSON_MOST_BYTES,
        &mut |m| hearing.moved(m),
    )?;
    if reply.went_through() {
        reply.body = answer;
    }
    match rest::answered(&reply)? {
        Answered::Async(id, _) => follow(incus, &id, hearing),
        Answered::Sync(_) => Ok(()),
    }
}

/// Push: a file streamed into an instance, over what is there.
fn push(
    incus: &Incus,
    name: &str,
    from: &Path,
    to: &str,
    hearing: &mut Hearing<'_>,
) -> Result<(), Ended> {
    let path = format!("{}/files?path={}", instance_path(name)?, inside(to)?);
    let (mut file, length) = sent(from)?;
    let mode = format!(
        "{:04o}",
        file.metadata().map_or(0o644, |m| m.mode() & 0o777)
    );
    let headers = [
        ("X-Incus-type", "file"),
        ("X-Incus-write", "overwrite"),
        ("X-Incus-mode", mode.as_str()),
    ];
    let call = Call {
        content_type: BYTES,
        ..Call::new("POST", &path, rest::HOST)
            .with_headers(&headers)
            .waiting(STREAM_PATIENCE)
    };
    let mut answer = Vec::new();
    let mut reply = rest::exchange(
        &incus.socket,
        &call,
        Some(Body {
            from: &mut file,
            length,
        }),
        &mut answer,
        rest::JSON_MOST_BYTES,
        &mut |m| hearing.moved(m),
    )?;
    if reply.went_through() {
        reply.body = answer;
    }
    rest::sync(&reply)?;
    Ok(())
}

/// Pull: a file of an instance streamed into a new file here. A folder or a
/// link there is refused, and what was written of it removed.
fn pull(
    incus: &Incus,
    name: &str,
    from: &str,
    to: &Path,
    hearing: &mut Hearing<'_>,
) -> Result<(), Ended> {
    let path = format!("{}/files?path={}", instance_path(name)?, inside(from)?);
    let mut file = made(to)?;
    let call = Call::new("GET", &path, rest::HOST).waiting(STREAM_PATIENCE);
    let result = rest::exchange(
        &incus.socket,
        &call,
        None,
        &mut file,
        FILE_MOST_BYTES,
        &mut |m| hearing.moved(m),
    )
    .map_err(Ended::from)
    .and_then(|reply| {
        if !reply.went_through() {
            return Err(Ended::from(Halt::from(refused_reply(&reply))));
        }
        // THE DAEMON SAYS WHAT IT SENT: a folder's answer is its listing.
        match reply.header("x-incus-type") {
            Some("file") | None => file.flush().map_err(|_| Refusal::of(Why::Failed).into()),
            Some(_) => Err(Refusal::of(Why::Invalid).into()),
        }
    });
    unless_done(result, to)
}

/// A moment as the daemon reads one: `YYYY-MM-DDTHH:MM:SSZ`. Pure.
#[must_use]
pub(crate) fn rfc3339(unix_s: u64) -> String {
    let days = unix_s / 86_400;
    let rest = unix_s % 86_400;
    // CIVIL FROM DAYS (Howard Hinnant's), for 1970 onwards.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Reads one job from `input` — one JSON line, at most `most` bytes.
///
/// # Errors
///
/// [`Why::Invalid`] for anything that is not one job.
pub fn job_of(input: &mut dyn std::io::Read, most: u64) -> Result<Job, Refusal> {
    let mut text = String::new();
    input
        .take(most + 1)
        .read_to_string(&mut text)
        .map_err(|_| Refusal::of(Why::Invalid))?;
    if text.len() as u64 > most {
        return Err(Refusal::of(Why::Invalid));
    }
    serde_json::from_str(text.trim()).map_err(|e| {
        tracing::warn!(error = %e, "the job is not one this build reads");
        Refusal::of(Why::Invalid)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Create, Kind, Limits, Protocol, Source};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-incus-job-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn nowhere(dir: &Path) -> Incus {
        Incus {
            socket: dir.join("absent.socket"),
            cli: None,
        }
    }

    fn launch() -> Job {
        Job::Launch {
            create: Create {
                name: "web".to_owned(),
                kind: Kind::Container,
                source: Source::Remote {
                    server: "https://images.linuxcontainers.org".to_owned(),
                    protocol: Protocol::SimpleStreams,
                    alias: "debian/13".to_owned(),
                },
                profiles: Vec::new(),
                limits: Limits::default(),
            },
        }
    }

    /// FAILS IF: a job that makes the daemon reach a remote runs without the
    /// person's consent for that host — it must be refused before the
    /// socket (absent here: reaching it answers `no-incus`).
    #[test]
    fn a_job_that_reaches_a_remote_asks_the_consent_file_first() {
        let dir = scratch("consent");
        let incus = nowhere(&dir);
        let store = ronoctua_consent::store_at(Some(dir.join("consent")));
        let asked = std::cell::RefCell::new(Vec::new());
        let minter = |host: &str| {
            asked.borrow_mut().push(host.to_owned());
            ronoctua_consent::pass_in(&store, host).ok()
        };
        let stop = || false;
        let mut say = |_: &Line| {};
        let copy = Job::ImageCopy {
            server: "https://images.linuxcontainers.org".to_owned(),
            protocol: Protocol::SimpleStreams,
            alias: "alpine/3.22".to_owned(),
            kind: None,
        };
        for job in [launch(), copy.clone()] {
            assert_eq!(
                run(&incus, &job, &minter, &mut Hearing::new(&stop, &mut say)),
                Err(Ended::Refused(Refusal::of(Why::NotLet))),
                "{job:?}"
            );
        }
        assert_eq!(
            asked.borrow().as_slice(),
            ["images.linuxcontainers.org", "images.linuxcontainers.org"]
        );
        ronoctua_consent::let_hosts_in(&store, &["images.linuxcontainers.org"], true).unwrap();
        for job in [launch(), copy] {
            assert_eq!(
                run(&incus, &job, &minter, &mut Hearing::new(&stop, &mut say)),
                Err(Ended::Refused(Refusal::of(Why::NoIncus))),
                "with consent, the daemon is asked: {job:?}"
            );
        }
        // DOCKER HUB'S CLIENT ASKS TWO HOSTS MORE than its address names,
        // and each needs its own consent.
        let hub = Job::ImageCopy {
            server: "https://docker.io".to_owned(),
            protocol: Protocol::Oci,
            alias: "nginx".to_owned(),
            kind: None,
        };
        ronoctua_consent::let_hosts_in(&store, &["docker.io", "registry-1.docker.io"], true)
            .unwrap();
        assert_eq!(
            run(&incus, &hub, &minter, &mut Hearing::new(&stop, &mut say)),
            Err(Ended::Refused(Refusal::of(Why::NotLet))),
            "auth.docker.io was not let"
        );
        ronoctua_consent::let_hosts_in(&store, &["auth.docker.io"], true).unwrap();
        assert_eq!(
            run(&incus, &hub, &minter, &mut Hearing::new(&stop, &mut say)),
            Err(Ended::Refused(Refusal::of(Why::NoIncus))),
            "with the three, the daemon is asked"
        );
        // AND WORK THAT STAYS HERE ASKS NOBODY.
        asked.borrow_mut().clear();
        let publish = Job::Publish {
            name: "web".to_owned(),
            alias: "web-image".to_owned(),
        };
        assert_eq!(
            run(
                &incus,
                &publish,
                &minter,
                &mut Hearing::new(&stop, &mut say)
            ),
            Err(Ended::Refused(Refusal::of(Why::NoIncus)))
        );
        assert!(asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_is_never_made_over_another_nor_left_half_made() {
        let dir = scratch("files");
        let incus = nowhere(&dir);
        let stop = || false;
        let mut say = |_: &Line| {};
        let there = dir.join("there.tar.gz");
        std::fs::write(&there, b"keep me").unwrap();
        let export = Job::Export {
            name: "web".to_owned(),
            to: there.clone(),
        };
        assert_eq!(
            run(
                &incus,
                &export,
                &|_| None,
                &mut Hearing::new(&stop, &mut say)
            ),
            Err(Ended::Refused(Refusal::of(Why::Invalid)))
        );
        assert_eq!(std::fs::read(&there).unwrap(), b"keep me");
        let fresh = dir.join("fresh.tar.gz");
        let export = Job::Export {
            name: "web".to_owned(),
            to: fresh.clone(),
        };
        assert_eq!(
            run(
                &incus,
                &export,
                &|_| None,
                &mut Hearing::new(&stop, &mut say)
            ),
            Err(Ended::Refused(Refusal::of(Why::NoIncus)))
        );
        assert!(!fresh.exists(), "a failed export left its file behind");
        let pull = Job::Pull {
            name: "web".to_owned(),
            from: "relative/path".to_owned(),
            to: dir.join("x"),
        };
        assert_eq!(
            run(&incus, &pull, &|_| None, &mut Hearing::new(&stop, &mut say)),
            Err(Ended::Refused(Refusal::of(Why::Invalid))),
            "a path inside is absolute"
        );
        let push = Job::Push {
            name: "web".to_owned(),
            from: dir.clone(),
            to: "/root/x".to_owned(),
        };
        assert_eq!(
            run(&incus, &push, &|_| None, &mut Hearing::new(&stop, &mut say)),
            Err(Ended::Refused(Refusal::of(Why::Invalid))),
            "a folder is not a file"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A daemon on a socket of the test's own, answering every connection
    /// with the next of `answers` and keeping what each request said.
    fn daemon(
        dir: &Path,
        answers: Vec<Vec<u8>>,
    ) -> (PathBuf, std::thread::JoinHandle<Vec<String>>) {
        let path = dir.join("unix.socket");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            use std::io::{BufRead as _, Read as _, Write as _};
            let mut heard = Vec::new();
            for answer in answers {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut length = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 {
                        break;
                    }
                    if let Some(n) = line.strip_prefix("Content-Length: ") {
                        length = n.trim().parse().unwrap();
                    }
                    head.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                head.push_str(&String::from_utf8_lossy(&body));
                heard.push(head);
                let mut out = stream;
                out.write_all(&answer).unwrap();
            }
            heard
        });
        (path, server)
    }

    fn http(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        out.extend_from_slice(body);
        out
    }

    fn sync_op(status: &str, code: u16, resources: &str, metadata: &str) -> Vec<u8> {
        http(
            "200 OK",
            "",
            format!(
                r#"{{"type":"sync","status":"Success","status_code":200,"operation":"","error_code":0,"error":"","metadata":{{"id":"0c3a5e7f-1b2d-4c6e-8f0a-2b4d6f8a0c1e","class":"task","description":"","status":"{status}","status_code":{code},"resources":{resources},"metadata":{metadata},"may_cancel":true,"err":""}}}}"#
            )
            .as_bytes(),
        )
    }

    fn started() -> Vec<u8> {
        http(
            "202 Accepted",
            "",
            br#"{"type":"async","status":"Operation created","status_code":100,"operation":"/1.0/operations/0c3a5e7f-1b2d-4c6e-8f0a-2b4d6f8a0c1e","error_code":0,"error":"","metadata":{"id":"0c3a5e7f-1b2d-4c6e-8f0a-2b4d6f8a0c1e","status":"Running","status_code":103}}"#,
        )
    }

    /// THE WHOLE EXPORT against a daemon of the test's: the backup is asked
    /// for, its operation followed (with the daemon's words passed on), the
    /// tarball streamed into a NEW 0600 file, and the backup deleted.
    #[test]
    fn an_export_makes_follows_streams_and_cleans_up() {
        let dir = scratch("export");
        let tarball: Vec<u8> = (0..3_000_000u32).map(|n| (n % 251) as u8).collect();
        let (socket, server) = daemon(
            &dir,
            vec![
                started(),
                sync_op(
                    "Running",
                    103,
                    "null",
                    r#"{"create_backup_progress":"12.00MB (6.00MB/s)"}"#,
                ),
                sync_op(
                    "Success",
                    200,
                    r#"{"backups":["/1.0/instances/web/backups/b"]}"#,
                    "null",
                ),
                http(
                    "200 OK",
                    "Content-Type: application/octet-stream\r\n",
                    &tarball,
                ),
                started(),
                sync_op("Success", 200, "null", "null"),
            ],
        );
        let incus = Incus { socket, cli: None };
        let to = dir.join("web.tar.gz");
        let stop = || false;
        let mut lines = Vec::new();
        let mut say = |l: &Line| lines.push(l.clone());
        run(
            &incus,
            &Job::Export {
                name: "web".to_owned(),
                to: to.clone(),
            },
            &|_| None,
            &mut Hearing::new(&stop, &mut say),
        )
        .unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), tarball);
        assert_eq!(
            std::fs::metadata(&to).unwrap().mode() & 0o777,
            0o600,
            "a backup holds the instance's secrets"
        );
        assert!(
            lines.contains(&Line::Said("12.00MB (6.00MB/s)".to_owned())),
            "{lines:?}"
        );
        assert_eq!(
            lines.last(),
            Some(&Line::Step(Step::Bytes {
                done: 3_000_000,
                total: Some(3_000_000)
            }))
        );
        let heard = server.join().unwrap();
        assert!(
            heard[0].starts_with("POST /1.0/instances/web/backups HTTP/1.1"),
            "{}",
            heard[0]
        );
        assert!(
            heard[0].contains("\"expires_at\""),
            "a stray backup expires"
        );
        assert!(heard[1].starts_with(
            "GET /1.0/operations/0c3a5e7f-1b2d-4c6e-8f0a-2b4d6f8a0c1e/wait?timeout=1"
        ));
        assert!(
            heard[3].starts_with("GET /1.0/instances/web/backups/ronoctua-export-"),
            "{}",
            heard[3]
        );
        assert!(heard[3].contains("/export HTTP/1.1"));
        assert!(heard[4].starts_with("DELETE /1.0/instances/web/backups/ronoctua-export-"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A STOP CANCELS THE DAEMON'S OPERATION and leaves no file.
    #[test]
    fn a_stop_cancels_the_operation_on_the_daemon() {
        let dir = scratch("stop");
        let (socket, server) = daemon(
            &dir,
            vec![
                started(),
                sync_op("Running", 103, "null", r#"{"download_progress":"rootfs: 5% (1.00MB/s)"}"#),
                http("200 OK", "", br#"{"type":"sync","status":"Success","status_code":200,"operation":"","error_code":0,"error":"","metadata":null}"#),
            ],
        );
        let incus = Incus { socket, cli: None };
        let store = ronoctua_consent::store_at(Some(dir.join("consent")));
        ronoctua_consent::let_hosts_in(&store, &["images.linuxcontainers.org"], true).unwrap();
        let polls = std::cell::Cell::new(0);
        let stop = || {
            polls.set(polls.get() + 1);
            true
        };
        let mut lines = Vec::new();
        let mut say = |l: &Line| lines.push(l.clone());
        assert_eq!(
            run(
                &incus,
                &launch(),
                &|h| ronoctua_consent::pass_in(&store, h).ok(),
                &mut Hearing::new(&stop, &mut say)
            ),
            Err(Ended::Stopped)
        );
        assert_eq!(lines.first(), Some(&Line::Step(Step::Percent(5))));
        let heard = server.join().unwrap();
        assert!(heard[0].starts_with("POST /1.0/instances HTTP/1.1"));
        assert!(
            heard[0].contains("\"server\":\"https://images.linuxcontainers.org\""),
            "{}",
            heard[0]
        );
        assert!(heard[0].contains("\"start\":true"), "a launch starts it");
        assert!(
            heard[2].starts_with(
                "DELETE /1.0/operations/0c3a5e7f-1b2d-4c6e-8f0a-2b4d6f8a0c1e HTTP/1.1"
            ),
            "{}",
            heard[2]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pulled_folder_is_refused_and_its_listing_removed() {
        let dir = scratch("pull");
        let (socket, server) = daemon(
            &dir,
            vec![http(
                "200 OK",
                "X-Incus-Type: directory\r\n",
                br#"{"type":"sync","metadata":["a","b"]}"#,
            )],
        );
        let incus = Incus { socket, cli: None };
        let to = dir.join("etc");
        let stop = || false;
        let mut say = |_: &Line| {};
        assert_eq!(
            run(
                &incus,
                &Job::Pull {
                    name: "web".to_owned(),
                    from: "/etc".to_owned(),
                    to: to.clone(),
                },
                &|_| None,
                &mut Hearing::new(&stop, &mut say)
            ),
            Err(Ended::Refused(Refusal::of(Why::Invalid)))
        );
        assert!(!to.exists());
        let heard = server.join().unwrap();
        assert!(
            heard[0].starts_with("GET /1.0/instances/web/files?path=/etc HTTP/1.1"),
            "{}",
            heard[0]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_push_says_the_file_s_mode_and_streams_it() {
        let dir = scratch("push");
        let (socket, server) = daemon(
            &dir,
            vec![http(
                "200 OK",
                "",
                br#"{"type":"sync","status":"Success","status_code":200,"operation":"","error_code":0,"error":"","metadata":{}}"#,
            )],
        );
        let incus = Incus { socket, cli: None };
        let from = dir.join("script.sh");
        std::fs::write(&from, b"#!/bin/sh\necho hi\n").unwrap();
        std::fs::set_permissions(&from, std::os::unix::fs::PermissionsExt::from_mode(0o750))
            .unwrap();
        let stop = || false;
        let mut say = |_: &Line| {};
        run(
            &incus,
            &Job::Push {
                name: "web".to_owned(),
                from,
                to: "/root/my script.sh".to_owned(),
            },
            &|_| None,
            &mut Hearing::new(&stop, &mut say),
        )
        .unwrap();
        let heard = server.join().unwrap();
        assert!(
            heard[0]
                .starts_with("POST /1.0/instances/web/files?path=/root/my%20script.sh HTTP/1.1"),
            "{}",
            heard[0]
        );
        assert!(heard[0].contains("X-Incus-mode: 0750\r\n"));
        assert!(heard[0].contains("Content-Type: application/octet-stream\r\n"));
        assert!(heard[0].ends_with("#!/bin/sh\necho hi\n"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_moment_is_written_as_the_daemon_reads_one() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_791_201_600), "2026-10-05T12:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn a_job_is_one_json_line_and_nothing_else() {
        let wire = serde_json::to_string(&launch()).unwrap();
        assert_eq!(job_of(&mut wire.as_bytes(), 65_536).unwrap(), launch());
        assert_eq!(
            job_of(&mut "{}".as_bytes(), 65_536),
            Err(Refusal::of(Why::Invalid))
        );
        assert_eq!(
            job_of(&mut wire.as_bytes(), 10),
            Err(Refusal::of(Why::Invalid))
        );
        assert!(!is_alias("two words"));
        assert!(is_alias("web/2026-10"));
    }
}
