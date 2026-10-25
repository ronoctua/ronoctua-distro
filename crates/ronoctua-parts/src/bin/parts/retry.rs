//! The next try, on a clock armed only while a part waits for it.
//!
//! A failed try is no failed unit (`ronoctua_parts::exit`), so systemd does
//! not try again by itself: the record says when the next try is due
//! (`Record::due_at`), and this arms ONE transient timer for the earliest of
//! them, which starts the planner. With nothing due, there is no timer at
//! all — evented, not a clock. A part waiting for a network has no due time:
//! the network coming up starts the planner (the dispatcher).

use ronoctua_parts::names;

use crate::disk;
use crate::{program, say};

/// The transient timer's unit, `<RETRY>.timer`.
const RETRY: &str = "ronoctua-parts-retry";

/// Arms the timer for the earliest due try, or disarms it when none is due.
pub fn arm() {
    let now = disk::now().unix_s();
    let earliest = disk::read_records()
        .iter()
        .filter_map(ronoctua_parts::record::Record::due_at)
        .min();
    let systemctl = program("RONOCTUA_SYSTEMCTL");
    // ONE TIMER: the one before it is gone first, whether it fired or not.
    let _ = disk::run_quiet(&[
        systemctl.clone(),
        "stop".to_owned(),
        format!("{RETRY}.timer"),
    ]);
    let Some(earliest) = earliest else {
        return;
    };
    let after = (earliest - now).max(1);
    let armed = disk::run_quiet(&[
        program("RONOCTUA_SYSTEMD_RUN"),
        format!("--unit={RETRY}"),
        format!("--on-active={after}s"),
        "--timer-property=AccuracySec=10s".to_owned(),
        "--description=The next try of a part of the machine".to_owned(),
        "--collect".to_owned(),
        systemctl,
        "start".to_owned(),
        "--no-block".to_owned(),
        names::PLANNER_UNIT.to_owned(),
    ]);
    if !armed {
        say(4, &format!("the next try, in {after} s, was not armed"));
    }
}
