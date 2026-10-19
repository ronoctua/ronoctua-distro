//! The plain-link engine: aria2, over its JSON-RPC, on the loopback.
//!
//! # Why aria2 and not a downloader of our own
//!
//! Because what makes a download of four gibibytes finish on bad wifi is
//! segmenting, resuming and retrying, and aria2 has spent fifteen years on
//! exactly that. `nix/modules/cli.nix` already installs it for the same reason.
//! What this file adds is the queue: a row, a bar, a pause and a cancel that
//! mean the same thing they mean for a copy.
//!
//! # Why HTTP by hand, and not an HTTP crate
//!
//! The whole conversation is a POST of a small JSON object to `127.0.0.1`,
//! answered by a small JSON object. There is no TLS, no redirect, no
//! compression and no connection reuse to get right — `Connection: close` and
//! read to the end covers it. An HTTP client would bring `tokio`, `hyper` and a
//! TLS stack into a `Cargo.lock` every crate in the workspace shares, to save
//! about sixty lines that will never change.
//!
//! # The secret is never on a command line
//!
//! aria2's `--rpc-secret` on `argv` is world-readable through `/proc`, so the
//! unit writes it into a file under `XDG_RUNTIME_DIR` — which is `0700` and
//! the person's own — and passes `--conf-path`. This reads the same file.
//! Nothing about it is in the repository (invariant 13).

use crate::auth::{self, Cookies};
use crate::engine::{POLL, Progress};
use crate::job::{Identity, Stage, Work};
use crate::ledger::Shared;
use anyhow::{Context as _, Result, bail};
use ronoctua_files::watch::{Control, Flow};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

/// Where the RPC listens. The loopback, and never a route off this machine:
/// an RPC with the power to write anywhere the person can write is an
/// administrative port, and `nix/modules/syncthing.nix` already says why one of
/// those does not go on the LAN.
const ENDPOINT: &str = "127.0.0.1:6800";

/// How long to wait for the loopback before giving up on it.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(5);

/// The file the unit wrote the secret into.
fn conf_path() -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").context(
        "XDG_RUNTIME_DIR is not set; without it there is nowhere to read aria2's secret from",
    )?;
    Ok(PathBuf::from(dir).join("ronoctua-aria2.conf"))
}

/// The secret, read out of that file.
///
/// The file is aria2's own configuration, so the line is `rpc-secret=<token>`
/// and there is exactly one source of truth for it — the alternative was two
/// files that agree until the day they do not.
fn secret() -> Result<String> {
    let path = conf_path()?;
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read {}", path.display()))?;
    secret_in(&text)
        .map(ToOwned::to_owned)
        .with_context(|| format!("{} has no `rpc-secret=`", path.display()))
}

/// The token inside an aria2 configuration file.
///
/// Split out from the reading so it can be tested without a file: this crate's
/// `transfers-socket-is-private` check refuses `temp_dir` outright, and on this
/// machine `/tmp` is a RAM filesystem a test has no business filling.
fn secret_in(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("rpc-secret="))
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// One JSON-RPC call, and its `result`.
///
/// The token goes in as the first parameter, which is how aria2 wants it — not
/// as a header.
fn call(method: &str, mut params: Vec<Value>) -> Result<Value> {
    let token = format!("token:{}", secret()?);
    params.insert(0, Value::String(token));
    let body = serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": "ronoctua",
        "method": method,
        "params": params,
    }))?;

    // THE FRAMER IS `ronoctua-http`'S, AND IT IS NOT OURS ANY MORE.
    //
    // This wrote its own request line and split its own answer, which was the
    // third copy of that in the workspace — and the only one of the three
    // whose reply was read as a `String` before it was split, so a chunked
    // answer with a multi-byte character in it had the panic the other two had
    // already paid for waiting in it. `one-http-writer` is what says one.
    let at: std::net::SocketAddr = ENDPOINT
        .parse()
        .with_context(|| format!("`{ENDPOINT}` is not an address"))?;
    // AND THE DEADLINE STAYS THIS CRATE'S. The framer's default is two
    // seconds, which is right for a daemon that either answers at once or is
    // not there; aria2 is asked to start a download and chose five.
    let call = ronoctua_http::Call::new("POST", "/jsonrpc", "127.0.0.1")
        .with_body(&body)
        .waiting(PATIENCE);
    let payload = ronoctua_http::over_tcp(at, &call)
        .with_context(|| format!("aria2 is not answering at {ENDPOINT}"))?
        .body;
    let parsed: Value = serde_json::from_str(&payload)
        .with_context(|| format!("aria2's answer is not JSON: {payload}"))?;
    if let Some(e) = parsed.get("error") {
        let message = e
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no message");
        bail!("aria2 refused `{method}`: {message}");
    }
    parsed
        .get("result")
        .cloned()
        .with_context(|| format!("`{method}`'s answer has no `result`"))
}

/// Whether the RPC is answering at all.
///
/// Asked ONCE, before a job is taken, so a queue with no aria2 says so on the
/// row instead of showing a job that never moves.
pub fn afoot() -> bool {
    call("aria2.getVersion", vec![]).is_ok()
}

/// What one `aria2.tellStatus` says, in the terms this file uses.
struct Status {
    state: String,
    done: u64,
    total: u64,
    name: String,
    trouble: Option<String>,
}

fn number(v: &Value, field: &str) -> u64 {
    v.get(field)
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn status_of(gid: &str) -> Result<Status> {
    let v = call(
        "aria2.tellStatus",
        vec![
            Value::String(gid.to_owned()),
            json!([
                "status",
                "totalLength",
                "completedLength",
                "errorMessage",
                "files"
            ]),
        ],
    )?;
    let name = v
        .get("files")
        .and_then(Value::as_array)
        .and_then(|f| f.first())
        .and_then(|f| f.get("path"))
        .and_then(Value::as_str)
        .and_then(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    Ok(Status {
        state: v
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        done: number(&v, "completedLength"),
        total: number(&v, "totalLength"),
        name,
        trouble: v
            .get("errorMessage")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
            .map(ToOwned::to_owned),
    })
}

/// Runs one link to the end, or to whatever the switch says.
///
/// The shape is [`crate::local::run`]'s: it owns the thread, it writes the
/// ledger, and it never returns a value — the row IS the answer.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Fetch {
        url,
        into,
        identity,
    } = &work
    else {
        tracing::error!("the aria2 engine got a job that is not a link");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    // A BROWSER'S STORE IS NOT SOMETHING aria2 CAN OPEN, so an identified plain
    // download needs a file. Saying so is better than sending the request
    // without the cookies and letting the site answer 403.
    let cookies = Cookies::here();
    if *identity == Identity::Mine && cookies.as_file().is_none() {
        shared.touch(id, |j| {
            j.stage = Stage::Failed;
            j.trouble = Some(
                "an identified link needs a cookies FILE; \
             aria2 does not open the browser's database"
                    .to_owned(),
            );
        });
        return;
    }
    let header = auth::cookie_header(*identity, &cookies, url);
    match carry(shared, id, switch, url, into, header.as_deref()) {
        Ok(()) => shared.touch(id, |j| j.stage = Stage::Done),
        Err(e) => {
            // A cancellation is not a failure and must not read like one: the
            // person asked for it.
            let cancelled = switch.flow() == Flow::Stop;
            let text = format!("{e:#}");
            shared.touch(id, |j| {
                j.stage = if cancelled {
                    Stage::Cancelled
                } else {
                    Stage::Failed
                };
                if !cancelled {
                    j.trouble = Some(text);
                }
            });
        }
    }
}

/// The download itself, without saying that the JOB is over.
///
/// `pub(crate)` and it does not touch `Stage::Done`, because it has a second
/// caller with a second half: `crate::atlas` downloads a region and then bakes
/// it, and a row that said "done" between the two would be a row lying for the
/// minute that matters. Who is finished is decided by whoever called this.
pub(crate) fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    url: &str,
    into: &std::path::Path,
    header: Option<&str>,
) -> Result<()> {
    // `header` and NOT `load-cookies`: aria2 takes the latter for the whole
    // session and not per download, so one identified link would make every
    // other one in the queue carry the person's cookies too — identity leaking
    // into requests nobody asked to identify.
    let mut options = json!({ "dir": into.to_string_lossy(), "continue": "true" });
    if let Some(header) = header
        && let Some(map) = options.as_object_mut()
    {
        map.insert("header".to_owned(), json!([header]));
    }
    let gid = call("aria2.addUri", vec![json!([url]), options])?;
    let gid = gid
        .as_str()
        .context("aria2 did not give back a gid")?
        .to_owned();
    // UNDER WAY — or held, if a pause landed while aria2 was being asked: the
    // switch decides, and the loop below holds the download at once.
    shared.under_way(id, |_| {});

    let mut progress = Progress::new(shared, id);
    let mut held = false;

    loop {
        // THE SWITCH FIRST, so a cancel does not have to wait for one more
        // poll to be honoured.
        match switch.flow() {
            Flow::Stop => {
                // `forceRemove` and not `remove`: the second one waits for the
                // connection to close politely, and a person who pressed cancel
                // is not waiting for politeness. The piece already written goes
                // with it, which is the same promise the local copy makes.
                let _ = call("aria2.forceRemove", vec![Value::String(gid.clone())]);
                let _ = call("aria2.removeDownloadResult", vec![Value::String(gid)]);
                progress.flush();
                bail!("given up");
            }
            // THE ROW IS NOT WRITTEN HERE: the pause and the resume wrote it
            // with the switch (`crate::engine`'s header says why).
            Flow::Pause if !held => {
                call("aria2.pause", vec![Value::String(gid.clone())])?;
                held = true;
            }
            Flow::Run if held => {
                call("aria2.unpause", vec![Value::String(gid.clone())])?;
                held = false;
            }
            Flow::Pause | Flow::Run => {}
        }

        let status = status_of(&gid)?;
        progress.set(status.done, status.total, &status.name);

        match status.state.as_str() {
            "complete" => {
                progress.set(status.done, status.done.max(status.total), &status.name);
                progress.flush();
                let _ = call("aria2.removeDownloadResult", vec![Value::String(gid)]);
                return Ok(());
            }
            "error" => {
                progress.flush();
                bail!(
                    "{}",
                    status.trouble.unwrap_or_else(|| "aria2 failed".to_owned())
                );
            }
            "removed" => {
                progress.flush();
                bail!("given up");
            }
            _ => {}
        }

        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_secret_comes_out_of_the_conf_and_an_empty_one_is_not_a_secret() {
        let conf = "enable-rpc=true\nrpc-listen-all=false\nrpc-secret=abc123\n";
        assert_eq!(secret_in(conf), Some("abc123"));
        // A file the unit wrote before it had a token would otherwise send
        // `token:` and be refused far from here, with aria2's message.
        assert_eq!(secret_in("rpc-secret=\n"), None);
        assert_eq!(secret_in("enable-rpc=true\n"), None);
    }

    #[test]
    fn a_status_without_the_fields_reads_as_zero_and_not_as_a_panic() {
        // aria2 omits `totalLength` until it knows one, and a download with no
        // `Content-Length` never gets one. Zero is what the row means by "not
        // known", and it must not be a parse failure.
        let v = json!({ "status": "active", "completedLength": "1024" });
        assert_eq!(number(&v, "completedLength"), 1024);
        assert_eq!(number(&v, "totalLength"), 0);
    }
}
