//! `ronoctua-shell-kit` — the vocabulary every surface of the shell draws and
//! says with.
//!
//! # What belongs here
//!
//! What the footer, every door and every feature use alike, and that names
//! none of them: where content begins and how a row of fields is laid out
//! (`chrome`, `fields`), the bridge from a display list to the renderer
//! (`paint`), the theme and the language a process is in, every seat's
//! pointers, the scale of a screen, the wheel, the minibuffer's line, the
//! keymap a process installed (`bindings`), the sockets the shell writes to
//! (`ipc`), the clock as the person reads it (`when`), and the words more than
//! one feature says.
//!
//! # Why it is a crate
//!
//! Because a module of the shell's single crate was recompiled by every edit of
//! every view, and these are the modules that change least and are read most.
//! Nothing of the shell is below this crate — the compiler refuses a path
//! upwards — so an edit of a view never reaches it, and an edit of it reaches
//! every shell crate, in parallel. A helper two features use comes down to
//! here, or to the door; it is never reached up for.
//!
//! The modules keep the paths they had: each is `crate::<module>` here and,
//! through the root of every crate above, there too.

// `deny`, as in the binary: an exception would be an explicit `#[allow]`
// beside its contract, and this crate has none.
#![deny(unsafe_code)]

pub mod actions;
pub mod band;
// WHERE EVERY SCROLLBAR OF A FRAME WAS DRAWN: its column is the bar's, for
// the pointer and for every band, whatever drew it.
pub mod bars;
pub mod bindings;
pub mod chrome;
pub mod clipboard_copy;
pub mod consent_rows;
pub mod daemon;
// THE NOTIFICATION DESK'S WORDS: what a button comes to, and what a view asks
// of the queue and the book the footer keeps.
pub mod desk;
pub mod doors;
pub mod echo;
pub mod fields;
pub mod fold;
pub mod foreign;
pub mod glyph;
pub mod hidpi;
pub mod ipc;
pub mod key;
pub mod keyboard;
pub mod language;
pub mod line;
// The pure half of the brain bridge — the daemon's records turned into the
// WIT's — which `ronoctua-render-png` links without the footer's line.
pub mod mind_wire;
pub mod minibuffer;
pub mod network_refusal;
pub mod paint;
pub mod seats;
pub mod security_words;
// A size as a person reads it — one arithmetic for the footer and for the
// photograph of the brain's page.
pub mod sizes;
// A name too long for its column, walking sideways a cell at a time.
pub mod slide;
pub mod text_select;
// Which theme this process draws in, and how the doors hear about a change.
pub mod theme;
pub mod toolkits;
pub mod wake;
pub mod watchdog;
pub mod wheel;
pub mod when;
