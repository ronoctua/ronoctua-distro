//! A network made of answers, which records what it was asked: what every
//! test of this crate hands where the daemon would reach a host.

use std::sync::{Mutex, PoisonError};

use ronoctua_consent::Pass;
use ronoctua_http::Reply;

use crate::Why;
use crate::ask::{Fetch, Net, Request, Web, covered};
use crate::sources::{Address, Refused};

/// Canned answers, each given once, to the request whose address is exactly
/// the one named.
#[derive(Default)]
pub(crate) struct Canned {
    answers: Mutex<Vec<(String, Reply)>>,
    asked: Mutex<Vec<Request>>,
}

impl Canned {
    /// A network that answers `url` with `status` and `body`, once each.
    pub(crate) fn new(answers: &[(&str, u16, &[u8])]) -> Self {
        Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .map(|(url, status, body)| {
                        (
                            (*url).to_owned(),
                            Reply {
                                status: *status,
                                headers: Vec::new(),
                                body: body.to_vec(),
                            },
                        )
                    })
                    .collect(),
            ),
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Every request asked, cloned.
    pub(crate) fn asked(&self) -> Vec<Request> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Every address asked, in order.
    pub(crate) fn urls(&self) -> Vec<String> {
        self.asked().into_iter().map(|r| r.url).collect()
    }
}

impl Net for Canned {
    fn send(&self, request: &Request) -> Result<Reply, Why> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        let mut answers = self.answers.lock().unwrap_or_else(PoisonError::into_inner);
        let at = answers
            .iter()
            .position(|(u, _)| *u == request.url)
            .ok_or(Why::Unreachable)?;
        Ok(answers.remove(at).1)
    }
}

impl Web for Canned {
    fn get(&self, pass: &Pass, fetch: &Fetch) -> Result<Reply, Refused> {
        covered(pass, &fetch.url)?;
        self.send(&Request {
            source: crate::Source::Archive,
            url: fetch.url.clone(),
            gzip: fetch.gzip,
            most_bytes: fetch.most_bytes,
        })
        .map_err(Refused::from)
    }

    fn get_own(&self, address: &Address, most_bytes: u64) -> Result<Reply, Refused> {
        self.send(&Request {
            source: crate::Source::Archive,
            url: address.expose().to_owned(),
            gzip: true,
            most_bytes,
        })
        .map_err(Refused::from)
    }
}

/// A pass for `host`, from a consent file of the test's own.
#[cfg(feature = "net")]
pub(crate) fn pass_for(host: &str, tag: &str) -> Pass {
    let dir = std::env::temp_dir().join(format!(
        "ronoctua-broadcast-pass-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let store = ronoctua_consent::store_at(Some(dir.clone()));
    ronoctua_consent::let_hosts_in(&store, &[host], true).expect("a consent file");
    let pass = ronoctua_consent::pass_in(&store, host).expect("let");
    let _ = std::fs::remove_dir_all(dir);
    pass
}
