//! Notifications and tray — section 4.2.
//!
//! Everything lives in the FOOTER'S OWN SURFACE, and nowhere else: there is no
//! bar, no corner popup and no card (invariants 14 and 15). What that imposes
//! on the design:
//!
//! - **One notification at a time** ([`queue`]). Stacking would need a
//!   container, and a container is a card.
//! - **The band grows ABOVE the modeline**, one line or three, exactly as the
//!   expansion grows below the minibuffer. It is the same surface, and that is
//!   what keeps it out of invariant 14's list.
//! - **Nothing holds the screen for ever** ([`presentation::Life`]). The band
//!   pushes the content up while it is there.
//! - **The tray is filterable** ([`tray`]), because in the modeline it contends
//!   for columns with the buffer's name.
//!
//! The icon chain ([`icon`]) has three steps and starts with a glyph, which is
//! invariant 10 in the form of code.
//!
//! The catalogue ([`catalogue`]) is the distro's generated table and the brain
//! modules' notifications, read at run time out of the brain's catalogue
//! ([`brain`]): a module of the person's never recompiles this crate.
//!
//! # The shape of the whole thing
//!
//! ```text
//!   dbus / oom  ->  Notification      what ARRIVED. Data, and it is final.
//!                        |
//!   catalogue   ->  Book::decide      which of ours it is, and which filter
//!                        |            caught it. Policy, and it is editable.
//!                        v
//!                   Presentation      show / life / loud / level / glyph
//!                        |
//!                     Queue           one on the band, the rest waiting
//!                        |
//!                     store           fifteen days of it, on disk
//! ```
//!
//! What arrived and what was decided never merge. They answer different
//! questions and they change for different reasons — see [`presentation`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod action;
pub mod brain;
pub mod catalogue;
pub mod dbus;
pub mod icon;
pub mod oom;
pub mod presentation;
pub mod queue;
pub mod rule;
pub mod store;
pub mod tray;

pub use action::{Action, Doing, Label, Program};
pub use catalogue::{Book, Entry};
pub use dbus::Event;
pub use icon::{Chain, Icon};
pub use oom::Reason;
pub use presentation::{INERT, Life, Presentation, Show, TOP_LEVEL, Verdict};
pub use queue::{Notification, Queue, Record, RecordKey, Shown, Urgency, eighths};
pub use rule::{Field, Rule, Term, Test, as_table, is_shareable_name};
pub use store::Preferences;
pub use tray::{FilterOf, Item};
