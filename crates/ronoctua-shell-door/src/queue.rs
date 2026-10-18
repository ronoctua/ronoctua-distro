//! THE TRANSFER QUEUE, from a door's side — the list heard, what a finished
//! program's order opens, and the keys that open the queue.
//!
//! # Why a module of its own
//!
//! The queue's VIEW (the transfers' crate) is one screen; listening to the
//! queue is every door's that hands it work — the file manager, the buffers,
//! the software centre, the containers, Incus, the security centre — and
//! each named the view to do it. What a job's end comes to, in words and in
//! reports, is `ronoctua_shell_work::jobs`'s.

use ronoctua_tokens as tokens;
use ronoctua_transfers::{Job, Stage};

/// The modeline's segment text, or `None` when there is nothing to say —
/// here, beside the list heard, because the modeline is the footer's and the
/// queue's panel a feature's, and both read the one list.
///
/// # Why it disappears, when the notification's never does
///
/// Because they answer different questions. The bell is the only indicator that
/// something is PENDING, so a screen without it says "nothing is waiting" — and
/// it has to be able to say that. A transfer segment at rest would say `0%`
/// for ever, which is a number that means nothing and a column that costs
/// something on every screen this system runs on.
///
/// It stays for a FAILURE, though. A copy that died two minutes ago leaves
/// nothing running, and a segment that only showed work in flight would take
/// the failure off screen before anybody saw it.
#[must_use]
pub fn segment(jobs: &[Job]) -> Option<String> {
    use ronoctua_transfers::ledger;
    let afoot = ledger::afoot(jobs);
    if afoot == 0 {
        return ledger::troubled(jobs)
            .then(|| format!("{} {}", tokens::icon::ERROR, tokens::icon::TRANSFER));
    }
    // THE FIGURE WHEN THERE IS ONE, the COUNT when there is not. While
    // everything is still being weighed nothing knows its total, and `0%` would
    // be a number somebody could act on wrongly — `2` is the truth.
    Some(match ledger::along(jobs) {
        Some(per) => format!("{} {}%", tokens::icon::TRANSFER, per / 10),
        None => format!("{} {afoot}", tokens::icon::TRANSFER),
    })
}

/// The launcher entry a finished program's order made, when it made one — what
/// `C-o` on its row opens.
#[must_use]
pub fn opens(job: &Job) -> Option<&str> {
    if job.stage != Stage::Done || !matches!(job.work, ronoctua_transfers::Work::Software { .. }) {
        return None;
    }
    job.outcome
        .as_ref()
        .and_then(|o| o.desktop_ids.first())
        .map(String::as_str)
}

/// Brings up the thread that watches the queue and pushes every list into
/// `calloop`.
///
/// It gives back `false` when there is nowhere to put a socket — and **that is
/// not fatal**. Without it the footer loses the percentage and the panel and
/// goes on drawing everything else; taking the shell down over a missing
/// `XDG_RUNTIME_DIR` would trade one feature for all of them. It is the same
/// criterion as `daemon::listen_to_the_compositor` and the control socket.
///
/// # Why a thread and not the socket in `calloop`
///
/// The same two reasons the compositor's stream gives. A JSON line can arrive split
/// across two reads, which in the main loop is a state machine and on a thread
/// with a blocking `read_line` is one line of code. And the client RECONNECTS —
/// it starts the daemon when nobody is home — which is a sleep and a retry, and
/// a sleep in the loop that draws is a frozen footer.
pub fn listen(
    channel: smithay_client_toolkit::reexports::calloop::channel::Sender<Vec<Job>>,
) -> bool {
    if let Err(e) = ronoctua_transfers::wire::socket_path() {
        tracing::warn!(error = %e, "no transfer queue; the segment does not appear");
        return false;
    }
    let built = std::thread::Builder::new()
        .name("transfers".into())
        .spawn(move || {
            // The closure says "stop" only when the other end has gone, which
            // means the footer is already on its way out. There is nothing
            // useful to log in a shell that is quitting.
            let _ = ronoctua_transfers::client::watch(|jobs| channel.send(jobs).is_err());
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the transfers thread");
            false
        }
    }
}

/// The keys that open the queue, as the leader's trees teach them — taught
/// once by each door (`bindings::taught`), never written in a text.
#[must_use]
pub fn queue_keys() -> String {
    crate::bindings::taught("transfers-open", None).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_transfers::{Engine, Work};
    use std::path::PathBuf;

    fn a_job(id: u64, stage: Stage) -> Job {
        let mut j = Job::fresh_one(
            id,
            Engine::Local,
            Work::Paste {
                paths: vec![PathBuf::from("/home/film.mkv")],
                into: PathBuf::from("/backup"),
                moves: false,
            },
        );
        j.stage = stage;
        j.total = 1000;
        j.done = 620;
        j
    }

    #[test]
    fn the_segment_says_the_count_before_it_can_say_a_figure() {
        // THE DEFECT THIS CATCHES: `0%` while two folders are being weighed.
        // Nothing knows its total yet, and a zero is a number somebody would
        // read as "stuck" — the count is what is actually true.
        let weighing = vec![
            Job::fresh_one(
                1,
                Engine::Local,
                Work::Measure {
                    path: PathBuf::from("/a"),
                },
            ),
            Job::fresh_one(
                2,
                Engine::Local,
                Work::Measure {
                    path: PathBuf::from("/b"),
                },
            ),
        ];
        let said = segment(&weighing).expect("two running and there is no segment");
        assert!(said.ends_with('2'), "{said}");
        assert!(!said.contains('%'), "it invented a percentage: {said}");

        // With a total, the figure. Per mille in, per cent out.
        assert_eq!(
            segment(&[a_job(1, Stage::Running)]),
            Some(format!("{} 62%", tokens::icon::TRANSFER))
        );

        // AT REST THERE IS NO SEGMENT. A permanent `0%` is a column spent on a
        // number that means nothing.
        assert_eq!(segment(&[]), None);
        assert_eq!(segment(&[a_job(1, Stage::Done)]), None);

        // BUT A FAILURE STAYS. Nothing is running, and taking the segment away
        // would take the only sign off the screen before anybody saw it.
        let mut broke = a_job(1, Stage::Failed);
        broke.trouble = Some("no space".to_owned());
        assert!(
            segment(&[broke]).is_some(),
            "the failure went away from the modeline"
        );
    }
}
