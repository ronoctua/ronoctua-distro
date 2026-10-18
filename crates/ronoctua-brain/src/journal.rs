//! What happened, as it crosses: how many records are kept and shown, and
//! how the activity log folds them.
//!
//! The journal ITSELF — the file the daemon rewrites whole, and why a failed
//! run moves the schedule while a hold is never written down — is the
//! daemon's (`ronoctua_brain_daemon::journal`). What is here is what both
//! sides of the socket agree on: the numbers a screen draws ([`RECENT`],
//! [`ACTIVITY`], [`HISTORY`]), which records the schedule counts from
//! ([`schedules`]), and the fold the footer and the daemon both make of one
//! list ([`grouped`]).

use crate::moment::Moment;
use crate::task::RunRecord;

/// How many runs are kept, across every task.
///
/// It was sixty-four, matched to the transfer queue's ceiling on finished
/// jobs. Two things moved it. The home page now shows the last [`ACTIVITY`]
/// things the brain did, so the list has to be at least that long to fill it;
/// and a five-minute watch was added, which at sixty-four would have wrapped
/// the whole list inside six hours and taken every other task's history with
/// it. Two hundred and fifty-six is a few days of the watch and still years of
/// everything else — and what protects the rare tasks is not this number, it
/// is the floor in the daemon's `Journal::put`.
///
/// The whole list crosses the socket on every change. At this size that is a
/// few tens of kilobytes, and the argument for sending the whole mind is in
/// `crate::wire`.
pub const HISTORY: usize = 256;

/// How many of a task's OWN runs are kept whatever else happens, and shown.
///
/// Ten, because the owner asked for ten. It is a floor on the pruning of the
/// daemon's `Journal::put` as well as the number the screen draws: a task's
/// row promises its last ten, and a ceiling that could take them is a promise
/// the screen cannot keep.
pub const RECENT: usize = 10;

/// How many records the home page's activity log shows.
///
/// A hundred, because the owner asked for a hundred. It is smaller than
/// [`HISTORY`] on purpose — what is kept and what is shown are two questions,
/// and the log scrolls, so showing fewer than are kept is a choice about the
/// list and not about the disk.
pub const ACTIVITY: usize = 100;

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
