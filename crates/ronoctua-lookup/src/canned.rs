//! A network made of answers, which records what it was asked: what every
//! test of this crate hands where the daemon would reach a host.

use std::sync::{Mutex, PoisonError};

use ronoctua_http::Reply;

use crate::Why;
use crate::ask::{Net, Request};

/// Canned answers, each given once, to the request whose address (without
/// its secret query) is exactly the one named.
#[derive(Default)]
pub(crate) struct Canned {
    answers: Mutex<Vec<(String, Reply)>>,
    /// Every request, in order.
    pub asked: Mutex<Vec<Request>>,
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

    /// The one request asked; panics when there were more or none.
    pub(crate) fn only(&self) -> Request {
        let asked = self.asked();
        assert_eq!(asked.len(), 1, "{asked:?}");
        asked[0].clone()
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

/// The value of a header a request carries, exposed — for an assertion.
pub(crate) fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.expose())
}
