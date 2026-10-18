//! `ronoctua-shell-software` — the person's own programs (`LEADER i`): the
//! view, what it reads and the one handler of what it asks, its window, its
//! region in the footer, its camera and its keys.
//!
//! One view, two doors: what the screen DECIDES is `software`, what it
//! reads is `software_feed` (and its pictures `software_pictures`), what
//! both doors do for it is `software_reading`, and the doors are the window
//! (`software_window`) and the footer's region (`software_region`), which
//! the binary registers by its id. The footer ranks it by that id and names
//! nothing of it: what lives with the view closed — the modeline's marks, the
//! ends of the queue's jobs, the launcher's words into the centre — is said in
//! the work's words under both (`programs_words`). A line of this crate
//! recompiles this crate and the binary, and no other feature.

#![deny(unsafe_code)]

// THE FOUNDATION, by the names these modules already use (`crate::chrome::…`,
// `crate::door::…`): a `use` at the root is seen by every module below it and
// is no part of this crate's API.
use ronoctua_shell_door::{carry, door, file_picker, launch, pasting, photo, queue, region};
use ronoctua_shell_keep::keep;
use ronoctua_shell_kit::{
    bindings, chrome, consent_rows, echo, fields, line, minibuffer, sizes, wake, when,
};
// AND THE PERSON'S PROGRAMS AS THE CENTRE AND THE FOOTER SAY THEM — how a
// job ended, the modeline's two marks, the launcher's words into the centre:
// the work's words, under both.
use ronoctua_shell_work::{programs_words, software_words};

pub mod key;
pub mod software;
pub mod software_feed;
pub mod software_pictures;
pub mod software_reading;
pub mod software_region;
pub mod software_window;

/// THE CENTRE'S WINDOW the binary runs by name: `ronoctua-shell software
/// [level | search <words> | web <address>]`, and what the
/// `br.ronoctua.software.desktop` entry runs for a double click on an
/// `.AppImage`, an `.apk` or a `.flatpakref` (`software-file <path>`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "software",
        run: centre_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "software-file",
        run: file_door,
    },
];

/// THE CENTRE'S WINDOW'S CAMERA (`photograph --door software`), over a
/// machine laid out in a room.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "software",
    take: software_window::camera::taken,
}];

/// `ronoctua-shell software [level | search <words> | web <address>]` — the
/// person's own programs in a window of their own, the SAME `software::View`
/// the footer draws from: on the first page, on the level named, on what a
/// search for the words finds, or on a new web app's editor for the address.
fn centre_door(said: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let said = said.all();
    let opening = if said.is_empty() {
        software::Opening::Home
    } else {
        software_window::opening_of(&said.join(" ")).with_context(|| {
            format!(
                "usage: ronoctua-shell software [{} | {} <words> | {} <address>]",
                software_window::LEVEL_WORDS.join(" | "),
                software_window::SEARCH_WORD,
                software_window::WEB_WORD
            )
        })?
    };
    software_window::host_it(opening).context("host the software view")
}

/// `ronoctua-shell software-file <path>` — the window, with the question to
/// install that file up.
fn file_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let path = words
        .first()
        .context("usage: ronoctua-shell software-file <path>")?;
    software_window::host_it(software_window::file_opening(std::path::Path::new(&path)))
        .context("host the software view")
}
