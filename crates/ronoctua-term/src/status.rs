//! What programs in a terminal say about themselves (OSC 7501), kept by the
//! specification's rules.
//!
//! The library validates each report and hands it over
//! ([`crate::ghostty::Effect::ProgramStatus`]); it keeps none of them. The
//! holder does, here, and writes them into the terminal's state, where the
//! brain reads an agent's state and the view draws it.
//!
//! # The rules
//!
//! - A report REPLACES its record whole: a field it leaves out is gone, not
//!   kept from the last one.
//! - A clear removes the record and every record beneath it (`a` takes
//!   `a/b`); the program's own record (`""`) is above all of them.
//! - A prompt (OSC 133 A), the end of a command (OSC 133 D) and the end of
//!   the program drop every record that says `working` or `blocked`: nothing
//!   can be working at a prompt, and a program that crashed mid-task never
//!   said it stopped.
//! - A full reset (RIS) clears them all.
//! - At most [`MAX_RECORDS`]; past it the least recently reported goes.
//! - What a program wrote is kept plain ([`ronoctua_terminals::state::plain`])
//!   and bounded per field.

use ronoctua_terminals::state::{self, BlockedOn, ProgramState, StatusRecord};

use crate::ghostty::{self, ProgramStatus};

/// The most records a terminal keeps.
pub const MAX_RECORDS: usize = 256;

/// The most characters a record's id keeps.
const ID_MAX_CHARS: usize = 128;
/// The most characters of the name a machine matches on.
const APP_MAX_CHARS: usize = 64;
/// The most characters of a record's title.
const TITLE_MAX_CHARS: usize = 128;
/// The most characters of a record's message.
const MESSAGE_MAX_CHARS: usize = 512;

/// A terminal's records, least recently reported first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statuses {
    records: Vec<StatusRecord>,
}

impl Statuses {
    /// Takes a report, at `at_ms` (milliseconds since the epoch). `true` if
    /// the records changed.
    pub fn report(&mut self, report: &ProgramStatus, at_ms: u64) -> bool {
        let id = state::plain(&report.id, ID_MAX_CHARS);
        let state = match report.state {
            ghostty::ProgramState::Clear => return self.clear(&id),
            ghostty::ProgramState::Idle => ProgramState::Idle,
            ghostty::ProgramState::Working => ProgramState::Working,
            ghostty::ProgramState::Done => ProgramState::Done,
            ghostty::ProgramState::Blocked => ProgramState::Blocked,
            ghostty::ProgramState::Error => ProgramState::Error,
        };
        let kind = match (state, report.blocked_on) {
            (ProgramState::Blocked, ghostty::BlockedOn::Permission) => Some(BlockedOn::Permission),
            (ProgramState::Blocked, ghostty::BlockedOn::Question) => Some(BlockedOn::Question),
            (ProgramState::Blocked, ghostty::BlockedOn::Auth) => Some(BlockedOn::Auth),
            _ => None,
        };
        let record = StatusRecord {
            app: state::plain(&report.app, APP_MAX_CHARS),
            state,
            kind,
            title: state::plain(&report.title, TITLE_MAX_CHARS),
            msg: state::plain(&report.message, MESSAGE_MAX_CHARS),
            percent: report.percent.map(|p| p.min(100)),
            at_ms,
            id,
        };
        self.records.retain(|r| r.id != record.id);
        self.records.push(record);
        if self.records.len() > MAX_RECORDS {
            let extra = self.records.len() - MAX_RECORDS;
            self.records.drain(..extra);
        }
        true
    }

    /// Removes `id` and everything beneath it.
    fn clear(&mut self, id: &str) -> bool {
        let before = self.records.len();
        let below = format!("{id}/");
        self.records
            .retain(|r| !(id.is_empty() || r.id == id || r.id.starts_with(&below)));
        self.records.len() != before
    }

    /// A prompt, or the end of a command: nothing is working any more.
    pub fn prompt(&mut self) -> bool {
        self.drop_the_running()
    }

    /// The terminal's program ended: nothing in it is working any more.
    pub fn ended(&mut self) -> bool {
        self.drop_the_running()
    }

    /// A full reset (RIS).
    pub fn reset(&mut self) -> bool {
        let had = !self.records.is_empty();
        self.records.clear();
        had
    }

    fn drop_the_running(&mut self) -> bool {
        let before = self.records.len();
        self.records
            .retain(|r| !matches!(r.state, ProgramState::Working | ProgramState::Blocked));
        self.records.len() != before
    }

    /// Whether any program waits on the person.
    #[must_use]
    pub fn blocked(&self) -> bool {
        self.records
            .iter()
            .any(|r| r.state == ProgramState::Blocked)
    }

    /// The records, least recently reported first.
    #[must_use]
    pub fn records(&self) -> &[StatusRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(id: &str, state: ghostty::ProgramState, title: &str) -> ProgramStatus {
        ProgramStatus {
            state,
            blocked_on: ghostty::BlockedOn::Permission,
            percent: None,
            id: id.to_owned(),
            app: "claude-code".to_owned(),
            title: title.to_owned(),
            message: String::new(),
        }
    }

    fn ids(s: &Statuses) -> Vec<&str> {
        s.records().iter().map(|r| r.id.as_str()).collect()
    }

    #[test]
    fn status_records_follow_the_spec() {
        // FAILS IF a report merges into its record instead of replacing it, a
        // clear leaves a child behind, a prompt keeps a "working" record, a
        // reset keeps any, or a program's control characters reach the state.
        use ghostty::ProgramState::{Blocked, Clear, Done, Working};
        let mut s = Statuses::default();
        assert!(s.report(&said("agent", Working, "Thinking"), 1));
        let mut with_message = said("agent", Blocked, "Edit\u{1b}[31m\u{202E}");
        with_message.message = "Apply?".to_owned();
        s.report(&with_message, 2);
        assert_eq!(s.records().len(), 1, "one record per id");
        let r = &s.records()[0];
        assert_eq!(
            (r.state, r.kind, r.title.as_str(), r.at_ms),
            (
                ProgramState::Blocked,
                Some(BlockedOn::Permission),
                "Edit[31m",
                2
            )
        );
        // REPLACED whole: a report without a message has none.
        s.report(&said("agent", Working, "Again"), 3);
        assert_eq!(s.records()[0].msg, "", "the old message is not kept");
        assert_eq!(s.records()[0].kind, None, "only a blocked record needs");
        assert!(!s.blocked());

        s.report(&said("agent/sub", Blocked, "Bash"), 4);
        s.report(&said("agentive", Done, "Other"), 5);
        assert!(s.blocked());
        assert!(s.report(&said("agent", Clear, ""), 6));
        assert_eq!(
            ids(&s),
            ["agentive"],
            "a clear takes its children, not a prefix"
        );

        s.report(&said("a", Working, ""), 7);
        s.report(&said("b", Blocked, ""), 8);
        assert!(s.prompt());
        assert_eq!(ids(&s), ["agentive"], "a prompt drops working and blocked");
        s.report(&said("c", Working, ""), 9);
        assert!(s.ended());
        assert_eq!(ids(&s), ["agentive"]);
        s.report(&said("x", Done, ""), 10);
        assert!(s.report(&said("", Clear, ""), 11));
        assert!(ids(&s).is_empty(), "the program's own record is above all");
        s.report(&said("y", Done, ""), 12);
        assert!(s.reset());
        assert!(s.records().is_empty());
    }

    #[test]
    fn past_the_limit_the_least_recently_reported_goes() {
        // FAILS IF a program that reports forever grows the state file
        // forever, or the record evicted is one just reported.
        let mut s = Statuses::default();
        for i in 0..MAX_RECORDS {
            s.report(
                &said(&format!("r{i}"), ghostty::ProgramState::Working, ""),
                i as u64,
            );
        }
        // r0 is reported again, so r1 is now the least recent.
        s.report(&said("r0", ghostty::ProgramState::Done, ""), 998);
        s.report(&said("new", ghostty::ProgramState::Done, ""), 999);
        assert_eq!(s.records().len(), MAX_RECORDS);
        assert!(ids(&s).contains(&"r0"), "a record just reported stays");
        assert!(!ids(&s).contains(&"r1"), "{:?}", &ids(&s)[..3]);
        assert_eq!(ids(&s).last(), Some(&"new"));
    }
}
