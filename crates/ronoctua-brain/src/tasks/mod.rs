//! Every feature of the brain, in one list.
//!
//! # THE DOOR IS THIS FILE
//!
//! Adding a task is a module beside this one and **one line** in [`tasks`].
//! Everything else — the queue, the gate, the announcement, the journal, the
//! socket, the screen — is already written and already watching. The five
//! places to touch are listed in this crate's own header, and they stay five
//! precisely because this list is a `vec![]` and not a registry somebody has to
//! wire up.
//!
//! [`crate::registry::Registry::of`] refuses a malformed or repeated id here,
//! at construction, so a mistake is a daemon that does not start rather than a
//! bracketed key on somebody's screen.

pub mod sweep;
pub mod update;

use std::sync::Arc;

use crate::moment::Offset;
use crate::task::Task;

/// Every task the brain knows.
///
/// # Why the zone is a parameter
///
/// Because [`Task::due`] is PURE — it takes no clock and opens no file — and a
/// schedule in a person's own day needs to know what o'clock it is where they
/// live. Handing the zone in at construction is what keeps the whole of the
/// scheduling testable in milliseconds, with a zone that jumps on a day the
/// test chooses.
#[must_use]
pub fn tasks(zone: Arc<dyn Offset>) -> Vec<Box<dyn Task>> {
    vec![
        // ONE LINE PER FEATURE. This is the door.
        Box::new(update::SystemUpdate::with(Arc::clone(&zone))),
        Box::new(sweep::CacheSweep::with(zone)),
    ]
}
