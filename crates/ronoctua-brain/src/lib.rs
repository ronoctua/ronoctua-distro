//! The brain: what the machine does by itself, when it may do it, and what
//! happened.
//!
//! # Why a crate, and a daemon, of its own
//!
//! Because the questions it answers outlive every process that asks them. "Has
//! this machine been updated in the last week" is not a question the footer can
//! answer, because the footer is restarted on every file edit in the nested
//! loop and on every crash on the installed one. It is the same argument
//! `ronoctua-transfers` makes for a copy of forty gibibytes, one level up: a
//! copy must survive the screen, and so must a decision about when the machine
//! rebuilds itself.
//!
//! And because three processes want the same answer — the footer's modeline,
//! the brain's own screen in the expanded region, and the window that screen
//! opens in. A schedule living inside the footer would be invisible to the
//! other two, and each would grow its own.
//!
//! # What it does NOT do
//!
//! - **It does not read `/proc`.** [`ronoctua_memory`] is the only crate that
//!   does, exactly as `ronoctua-niri` is the only one that speaks to the
//!   compositor. The pulse arrives through [`pulse::Meter`], already read.
//! - **It reimplements no Nix logic** (invariant 5). It does not know what an
//!   update IS; it names a unit and reads what the unit reported.
//! - **It runs no privileged command.** It asks systemd to start a unit, and
//!   the unit — and the narrow polkit rule that lets it be started — belong to
//!   the flake. `systemctl` is found by STORE PATH and never by name: under a
//!   unit the `PATH` is minimal and a name fails far from its cause.
//! - **It holds no keyboard and creates no surface** (invariant 14). What is on
//!   screen is a `wasm32-wasip2` component in `buffers/brain/`, drawn by the
//!   footer like every other one.
//! - **It does not translate.** What crosses is a WORD — a task's id, a hold's
//!   name — and whoever has the bundle makes a sentence out of it (invariant 8).
//!
//! # THE DOOR — how a future feature is added
//!
//! This is the point of the crate. The owner asked for a place where "anything
//! in that space" becomes a feature rather than a new daemon, and a place is
//! only a place if the steps to use it are written down.
//!
//! Adding a task touches **five** places, and nothing else:
//!
//! 1. **`src/tasks/<name>.rs`** — one type implementing [`task::Task`]. Five
//!    methods: what it is called, when it is due, what it needs, how long it
//!    warns for, and what it does.
//! 2. **One line in [`tasks::tasks`]** — the single `vec![]` that lists them.
//!    [`registry::Registry::of`] refuses a malformed or repeated id there, at
//!    construction, the way `ronoctua-cmd` refuses a command.
//! 3. **`brain-task-<id>` and `brain-task-<id>-does` in both `.ftl`** — the
//!    name the screen draws and the line under the cursor that says what it
//!    really does. Two languages, or the screen shows the key in brackets.
//! 4. **One home-page row in `buffers/brain/`**, if it wants one. A task with
//!    no row still runs; it simply has no screen of its own.
//! 5. **A default in [`prefs`]**, if it has preferences. A task without them
//!    inherits the general gate and nothing else.
//!
//! What a new task does **not** have to write, because the runner already owns
//! it: the queue (at most one task runs at a time, and what comes due meanwhile
//! waits), the metric gate (busy machine, battery), the announcement and its
//! countdown, the journal, the socket, and every screen that watches.
//!
//! # The shape of it
//!
//! ```text
//!   footer      ──┐
//!   brain buffer ─┼─→  socket  →  Shared (Mutex + Condvar + version)
//!   `ronoctua-brain look` ─┘        │
//!                                   ├─→ ONE runner thread → systemctl start
//!                                   │        ↑ pulse (ronoctua-memory)
//!                                   └─→ thread per watcher → the whole mind
//! ```
//!
//! # Where the database would go
//!
//! Not here, and not yet. What is written today is two small JSON files, for
//! the reason [`journal`] gives: a few dozen records, rewritten when something
//! happens. The day the brain keeps a MEASUREMENT per minute — which is the
//! first feature that needs one — the seam is [`journal::Journal`], whose whole
//! surface is "give me the last run of this task" and "write this one down".
//! A store under `$XDG_STATE_HOME/ronoctua/brain/` replaces the file behind
//! those two calls, and nothing above them changes.

#![forbid(unsafe_code)]

pub mod client;
pub mod journal;
pub mod mind;
pub mod moment;
pub mod prefs;
pub mod pulse;
pub mod registry;
pub mod runner;
pub mod schedule;
pub mod serve;
pub mod systemd;
pub mod task;
pub mod tasks;
pub mod wire;

pub use journal::Journal;
pub use mind::{Announcement, Mind, Shared, Standing, StepState, TaskState};
pub use moment::{Clock, Moment, Offset};
pub use prefs::{Gate, PartOfDay, Prefs, TaskPrefs, Where};
pub use pulse::{Meter, Pulse};
pub use registry::Registry;
pub use runner::Runner;
pub use task::{
    Due, Hold, Outcome, OutcomeKind, Report, RunRecord, Sheet, Step, StepOutcome, Task, Verdict,
    Watched,
};
pub use wire::{Ask, Say};
