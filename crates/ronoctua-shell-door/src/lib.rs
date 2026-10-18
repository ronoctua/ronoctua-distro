//! `ronoctua-shell-door` — every window's plumbing, once.
//!
//! # What belongs here
//!
//! What makes a window a window, and what every window hands a gesture to.
//! A window is what it draws and what its keys do (`door::Content`); the rest
//! is `door::Door`'s — the registry, the seats, the keyboards and their repeat,
//! the pointers and the finger, the scale, the GPU, the watches, the header,
//! the list and its bar — and a window that wants one of them otherwise says
//! so by overriding one method. Beside it: the right column every tenant
//! lives in (`column`, `right_column`), the clipboard and the drag (`carry`,
//! the one file that speaks `wl_data_device`), the file picker, starting a
//! program (`launch`), a root gesture handed to the footer (`sudo::Gate`), the
//! queue heard (`queue`), the brain's socket (`brainlink`), the table of the
//! windows that float (`floating`) and the harness every camera photographs a
//! door in (`photo`).
//!
//! # Why it is a crate
//!
//! Because a window's plumbing is the same for every window, and it stands on
//! the kit alone: it names no feature and not the footer, so a line of a view
//! never recompiles it. The modules keep the paths they had (`crate::door`,
//! `crate::column`), here and through the root of every crate above; the
//! kit's modules are named here at the root for the same reason.

// `deny`, not `forbid`: `door/mod.rs` needs ONE unsafe block to pass Wayland's
// pointers to wgpu, with its contract written beside it.
#![deny(unsafe_code)]

// THE KIT, by the names these modules already use (`crate::chrome::…`).
use ronoctua_shell_kit::{
    bars, bindings, chrome, daemon, desk, echo, fields, fold, foreign, glyph, hidpi, ipc, language,
    line, mind_wire, seats, security_words, sizes, text_select, theme, toolkits, wake, watchdog,
    wheel, when,
};
// The list's tests drive the minibuffer's own completion.
#[cfg(test)]
use ronoctua_shell_kit::minibuffer;

pub mod brainlink;
// THE SCREEN CAPTURE'S ANSWER, between the overlay that chose and the footer
// that started it: the words both ends speak.
pub mod capture_wire;
pub mod carry;
pub mod clipboard;
// The clipboard's history between the footer, which keeps it, and the window,
// which draws it: the wire both speak.
pub mod clipboard_wire;
pub mod column;
pub mod cursor_shape;
pub mod door;
// THE ONE FILE PICKER every door asks for a file, a folder or a name to save
// under.
pub mod file_picker;
// The table of the windows that open floating, and the `app_id` each has.
pub mod floating;
pub mod key;
pub mod launch;
pub mod pasting;
pub mod photo;
pub mod places;
pub mod queue;
// A FEATURE'S VIEW IN THE FOOTER'S REGION: the `Region` a feature writes,
// the `Doorway` both doors are, and the slot host the footer keeps them in.
pub mod region;
pub mod right_column;
pub mod room;
// WHAT A REGION LAID OUT IN SECTIONS ANSWERS THE FOOTER (`region::Shape::
// Sections`): the layout, the pointer, the bands and the pictures, by what
// each answer is.
pub mod sections;
pub mod sudo;
pub mod tags;
