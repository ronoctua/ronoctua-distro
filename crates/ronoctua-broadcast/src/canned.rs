//! A network made of answers, which records what it was asked: what every
//! test of this crate hands where the daemon would reach a host.

use std::sync::{Mutex, PoisonError};

use ronoctua_http::Reply;

use crate::Why;
use crate::ask::{Net, Request};

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
