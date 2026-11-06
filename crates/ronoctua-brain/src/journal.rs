//! What happened, as it crosses: how far back records are kept and shown,
//! and how the activity log folds them.
//!
//! The journal ITSELF — the file the daemon rewrites whole, and why a failed
//! run moves the schedule while a hold is never written down — is the
//! daemon's (`ronoctua_brain_daemon::journal`). What is here is what both
//! sides of the socket agree on: the reaches a screen draws
//! ([`ACTIVITY_DAYS`], [`RECENT_DAYS`]) and the ceiling under them
//! ([`HISTORY`]), which records the schedule counts from ([`schedules`]), and
//! the fold the footer and the daemon both make of one list ([`grouped`]).

use crate::moment::{DAY, Moment};
use crate::task::RunRecord;

/// How far back the home page's activity log reaches, in days.
///
/// FIFTEEN, because the owner asked for fifteen days in place of the last
/// hundred records: *"em vez de mostrar as últimas 100 atividades, passe a
/// mostrar os últimos 15 dias"*. A count was a reach that changed with how
/// busy the machine was — a hundred records is three days of an agent at
/// work and a month of a quiet machine — and a reach in days is the same
/// promise on both.
pub const ACTIVITY_DAYS: u16 = 15;

/// How far back a task's OWN history reaches, in days.
///
/// TWENTY, the owner's number, in place of a task's last ten runs: *"passem a
/// armazenar os últimos 20 dias"*. A task that ran last a month ago has an
/// empty history and still its last run, which its row shows — the journal
/// never drops a task's last record ([`Journal::put`] in the daemon).
pub const RECENT_DAYS: u16 = 20;

/// How far back the journal keeps anything: the longer of the two reaches,
/// so a screen is never promised a day the file already let go of.
pub const KEPT_DAYS: u16 = if ACTIVITY_DAYS > RECENT_DAYS {
    ACTIVITY_DAYS
} else {
    RECENT_DAYS
};

/// How many records are kept at most, however young they are.
///
/// A CEILING UNDER THE REACH, and not the reach: the days are what a screen
/// promises, and this is what bounds the file and the mind that crosses the
/// socket on every change when something writes far more than anybody
/// planned for. Twenty days of an agent counted every ten minutes of a
/// working day is around a thousand records; two thousand and forty-eight is
/// that twice, and still a few hundred kilobytes. Past it the oldest go
/// first, whatever their age — never a task's last record.
pub const HISTORY: usize = 2048;

/// The first moment a reach of `days` counted back from `now` covers.
#[must_use]
pub const fn since(now: Moment, days: u16) -> Moment {
    now.saturating_sub(days as Moment * DAY)
}

/// Whether the schedule counts from this record: a run that worked, failed or
/// was put off — never a hold (the machine was busy) and never a note (a
/// module wrote down what it noticed).
#[must_use]
pub fn schedules(r: &RunRecord) -> bool {
    !matches!(
        r.kind,
        crate::task::OutcomeKind::Held | crate::task::OutcomeKind::Noted
    )
}

/// Consecutive records that say the same thing, folded into one row of the
/// activity log.
///
/// # Why the log folds at all
///
/// Because a watch writes the same line as often as the thing it watches
/// happens, and a hold is written once per episode but an episode can come back
/// every night. Forty rows of "RAM is busy" bury the one row that says the
/// update failed. Folded, they are one row — "Last: <when>", how many times,
/// and the first of them — that opens into all of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The newest of them: what the row draws.
    pub newest: RunRecord,
    /// How many records the row folds; `1` is one that stands alone.
    pub repeats: u32,
    /// When the OLDEST of them began.
    pub first: Moment,
    /// All of them, NEWEST FIRST, `newest` included — what the row opens into.
    pub members: Vec<RunRecord>,
}

/// The records, NEWEST FIRST, with every run of consecutive ones that say the
/// same thing folded into one [`Group`].
///
/// `runs` is OLDEST FIRST, as the journal keeps it. "The same thing" is the
/// same task, kind, step, word and sentence — not the same trouble text, which
/// carries a machine's numbers and would split every fold in two. PURE: the
/// daemon and the footer fold the same list the same way, and the cut is the
/// caller's.
#[must_use]
pub fn grouped(runs: &[RunRecord]) -> Vec<Group> {
    let same = |a: &RunRecord, b: &RunRecord| {
        a.task == b.task && a.kind == b.kind && a.at == b.at && a.why == b.why && a.says == b.says
    };
    let mut out: Vec<Group> = Vec::new();
    for r in runs.iter().rev() {
        match out.last_mut() {
            Some(g) if same(&g.newest, r) => {
                g.repeats = g.repeats.saturating_add(1);
                g.first = r.began;
                g.members.push(r.clone());
            }
            _ => out.push(Group {
                newest: r.clone(),
                repeats: 1,
                first: r.began,
                members: vec![r.clone()],
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::OutcomeKind;

    #[test]
    fn what_says_the_same_thing_in_a_row_is_one_row() {
        let held = |at: Moment| {
            let mut r = ran("fixture", at, OutcomeKind::Held);
            r.why = "ram".to_owned();
            r
        };
        let runs = vec![
            ran("fixture", 10, OutcomeKind::Done),
            held(20),
            held(30),
            held(40),
            ran("other", 50, OutcomeKind::Done),
            held(60),
        ];
        let rows = grouped(&runs);
        // NEWEST FIRST, and a fold never crosses another task's record: the
        // hold after `other` is its own row, not a fourth repeat.
        assert_eq!(rows.len(), 4);
        assert_eq!((rows[0].newest.began, rows[0].repeats), (60, 1));
        assert_eq!(rows[1].newest.task, "other");
        assert_eq!((rows[2].newest.began, rows[2].repeats), (40, 3));
        assert_eq!(rows[2].first, 20, "the fold forgot where it began");
        assert_eq!(
            rows[2].members.iter().map(|r| r.began).collect::<Vec<_>>(),
            vec![40, 30, 20]
        );
        assert_eq!((rows[3].newest.began, rows[3].repeats), (10, 1));
        // A DIFFERENT WORD IS A DIFFERENT ROW, however alike the rest is.
        let mut other_word = held(70);
        other_word.why = "cpu".to_owned();
        let rows = grouped(&[held(60), other_word]);
        assert_eq!(rows.len(), 2);
        assert!(grouped(&[]).is_empty());
    }

    fn ran(task: &str, at: Moment, kind: OutcomeKind) -> RunRecord {
        RunRecord {
            task: task.to_owned(),
            began: at,
            ended: at + 1,
            kind,
            ..RunRecord::default()
        }
    }
}
