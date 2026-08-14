//! Notifications and tray — section 4.2.
//!
//! Both live in the modeline, and nowhere else: there is no bar, no corner popup
//! and no card (invariants 14 and 15). What that imposes on the design:
//!
//! - **One notification at a time** ([`fila`]). Stacking would require a container.
//! - **The history uses THE surface**, growing below the minibuffer —
//!   never a second surface.
//! - **The tray is filterable** ([`tray`]), because in the modeline it contends for
//!   columns with the buffer's name.
//!
//! The icon chain ([`icon`]) has three steps and starts with a glyph, which is
//! invariant 10 in the form of code.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod dbus;
pub mod icon;
pub mod queue;
pub mod tray;

pub use dbus::Event;
pub use icon::{Chain, Icon};
pub use queue::{Notification, Queue, Urgency};
pub use tray::{FilterOf, Item};
