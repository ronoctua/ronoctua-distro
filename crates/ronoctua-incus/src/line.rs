//! The lines `ronoctua-incus-job` prints, one per line on its stdout, and
//! what the queue reads out of them.
//!
//! # Text, not JSON
//!
//! Four words and a number: the queue reads them while the child runs, and a
//! line it cannot read is chatter, never a parse error. The daemon's own words
//! (`said`) are DATA, the row's "now" — a person sees the stage Incus named,
//! as a row of the containers sees podman's.
//!
//! ```text
//! percent <0-100>
//! bytes <done>/<total|->
//! said <the daemon's words>
//! done
//! refused <why-id>
//! ```

use crate::{Step, Why};

/// One line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// Where the job is.
    Step(Step),
    /// The daemon's own words for where it is (`rootfs: 23% (12.30MB/s)`).
    Said(String),
    /// It ended well.
    Done,
    /// It ended refused.
    Refused(Why),
}

/// The longest `said` kept: a row shows one line.
const SAID_MOST_BYTES: usize = 256;

impl Line {
    /// The line as it is printed.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Step(Step::Percent(n)) => format!("percent {n}"),
            Self::Step(Step::Bytes { done, total }) => match total {
                Some(total) => format!("bytes {done}/{total}"),
                None => format!("bytes {done}/-"),
            },
            Self::Said(text) => format!("said {}", one_line(text)),
            Self::Done => "done".to_owned(),
            Self::Refused(why) => format!("refused {}", why.id()),
        }
    }

    /// The line a printed one is; `None` for anything else.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\n', '\r']);
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        match word {
            "percent" => rest
                .parse::<u8>()
                .ok()
                .filter(|n| *n <= 100)
                .map(|n| Self::Step(Step::Percent(n))),
            "bytes" => {
                let (done, total) = rest.split_once('/')?;
                let done = done.parse().ok()?;
                let total = if total == "-" {
                    None
                } else {
                    Some(total.parse().ok()?)
                };
                Some(Self::Step(Step::Bytes { done, total }))
            }
            "said" => Some(Self::Said(one_line(rest))),
            "done" if rest.is_empty() => Some(Self::Done),
            "refused" => Why::of_id(rest).map(Self::Refused),
            _ => None,
        }
    }

    /// Whether it is the last line a job prints.
    #[must_use]
    pub const fn is_verdict(&self) -> bool {
        matches!(self, Self::Done | Self::Refused(_))
    }
}

/// What a line of `ronoctua-incus-job` said of where it is; `None` for
/// anything else.
#[must_use]
pub fn step_of(line: &str) -> Option<Step> {
    match Line::parse(line)? {
        Line::Step(step) => Some(step),
        Line::Said(_) | Line::Done | Line::Refused(_) => None,
    }
}

/// Text as one line, cut to what a row keeps, on a character boundary.
fn one_line(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.len() <= SAID_MOST_BYTES {
        return flat.to_owned();
    }
    let mut end = SAID_MOST_BYTES;
    while !flat.is_char_boundary(end) {
        end -= 1;
    }
    flat[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_reads_back_as_itself() {
        let lines = [
            Line::Step(Step::Percent(0)),
            Line::Step(Step::Percent(100)),
            Line::Step(Step::Bytes {
                done: 5,
                total: Some(9),
            }),
            Line::Step(Step::Bytes {
                done: 5,
                total: None,
            }),
            Line::Said("rootfs: 23% (12.30MB/s)".to_owned()),
            Line::Done,
        ]
        .into_iter()
        .chain(Why::ALL.map(Line::Refused));
        for line in lines {
            assert_eq!(Line::parse(&line.text()), Some(line.clone()), "{line:?}");
        }
    }

    #[test]
    fn only_a_step_is_a_step_and_garbage_is_nothing() {
        assert_eq!(step_of("percent 42"), Some(Step::Percent(42)));
        assert_eq!(
            step_of("bytes 10/-\n"),
            Some(Step::Bytes {
                done: 10,
                total: None
            })
        );
        for nothing in [
            "percent 101",
            "percent -1",
            "bytes 1",
            "bytes x/2",
            "done",
            "said hi",
            "refused nonsense",
            "",
            "Error: not found",
        ] {
            assert_eq!(step_of(nothing), None, "{nothing:?}");
        }
        assert_eq!(Line::parse("refused nonsense"), None);
        assert_eq!(Line::parse("done now"), None);
        assert!(Line::Done.is_verdict());
        assert!(Line::Refused(Why::Gone).is_verdict());
        assert!(!Line::Said(String::new()).is_verdict());
    }

    #[test]
    fn the_daemon_s_words_stay_one_short_line() {
        let said = Line::Said(format!("a\nb\r{}", "é".repeat(400))).text();
        assert!(!said.contains('\n') && !said.contains('\r'));
        let Some(Line::Said(back)) = Line::parse(&said) else {
            panic!("{said}");
        };
        assert!(back.len() <= SAID_MOST_BYTES);
        assert!(back.starts_with("a b "));
    }
}
