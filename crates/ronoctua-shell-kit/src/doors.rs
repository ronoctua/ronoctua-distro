//! THE WORDS THAT OPEN A WINDOW of this binary — what a door hands
//! `Ui::launch` to open another door — and the [`Door`] each crate hands
//! the binary for every window it has.
//!
//! # Why a module of its own
//!
//! A door opens another by its command line (`ronoctua-shell <door> …`),
//! and the words are the opened door's to decide. A window that names the
//! other window's module to build them recompiles with all of it; the words
//! are a line, and every door can reach them here.
//!
//! # The table the binary dispatches by
//!
//! Each crate with windows says them in one table (`pub const DOORS`), and
//! the binary runs the one its first word names: its own subcommands first,
//! then every crate's doors, then the footer. So a crate's window is written,
//! with its words and its refusals, in the crate that has it, and the binary
//! names the crate once, in its list of tables.

/// A WINDOW OF THIS BINARY, by the word that names it on the command line
/// (`ronoctua-shell <name> …`).
#[derive(Clone, Copy)]
pub struct Door {
    /// Its word, after the binary's name.
    pub name: &'static str,
    /// Runs it, with the words after its own, until it closes.
    pub run: fn(words: &Words) -> anyhow::Result<()>,
}

/// THE WORDS AFTER A DOOR'S OWN, as the command line holds them — the
/// system's strings, not text — and taken as text only as far as the door
/// reads them.
///
/// # Why not a list of `String`
///
/// A word on a command line need not be UTF-8: it can be a file name. Each
/// door reads what it takes — no word, its first ([`Words::first`]) or all of
/// them ([`Words::all`]) — and only what it reads becomes text, so a word no
/// door reads is never converted. A door that takes no word, or only the
/// first, does not care what follows it; a list converted in full before the
/// door was found would make a trailing word that is not UTF-8 stop every one
/// of them, and the binary's own subcommands with them (`opener`, which reads
/// its file names as the system's strings, precisely to take such a name).
///
/// A word a door DOES read as text, and is not UTF-8, panics, as
/// `std::env::args` panics on the word it is asked for: that is what reading a
/// door's words always did.
#[derive(Debug, Clone, Default)]
pub struct Words(Vec<std::ffi::OsString>);

impl Words {
    /// The words after the door's own on this process's command line
    /// (`ronoctua-shell <door> <words>…`), none of them read yet.
    #[must_use]
    pub fn of_this_process() -> Self {
        std::env::args_os().skip(2).collect()
    }

    /// The first word, as text — what a door that takes one word reads.
    ///
    /// # Panics
    ///
    /// When that word is not UTF-8 (see [`Words`]).
    #[must_use]
    pub fn first(&self) -> Option<String> {
        self.0.first().map(text_of)
    }

    /// Every word, as text — what a door that takes all of them reads.
    ///
    /// # Panics
    ///
    /// When one of them is not UTF-8 (see [`Words`]).
    #[must_use]
    pub fn all(&self) -> Vec<String> {
        self.0.iter().map(text_of).collect()
    }
}

impl<W: Into<std::ffi::OsString>> FromIterator<W> for Words {
    fn from_iter<I: IntoIterator<Item = W>>(words: I) -> Self {
        Self(words.into_iter().map(Into::into).collect())
    }
}

/// A word as text, refused as `std::env::args` refuses it.
fn text_of(word: &std::ffi::OsString) -> String {
    word.clone()
        .into_string()
        .unwrap_or_else(|word| panic!("a word of the command line is not UTF-8: {word:?}"))
}

impl std::fmt::Debug for Door {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Door")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The door `name` names in `tables`, the first that does.
#[must_use]
pub fn door_named(tables: &[&[Door]], name: &str) -> Option<Door> {
    tables
        .iter()
        .flat_map(|t| t.iter())
        .find(|d| d.name == name)
        .copied()
}

/// The words that open the installer stick's window on a drive
/// (`ronoctua-shell stick --drive <id>`).
#[must_use]
pub fn stick_window_on(drive: &str) -> Vec<String> {
    vec!["stick".to_owned(), "--drive".to_owned(), drive.to_owned()]
}

/// The words that open the security centre's window checking `path` with
/// VirusTotal (`ronoctua-shell security online --check <path>`) — what the
/// file manager's "Check with VirusTotal" starts from either of its doors.
/// `online` is the word of the centre's section (`security::Section::Online`),
/// and the centre's window reads these back (its own test).
#[must_use]
pub fn security_checking(path: &std::path::Path) -> Vec<String> {
    vec![
        "security".to_owned(),
        "online".to_owned(),
        "--check".to_owned(),
        path.to_string_lossy().into_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::Words;
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;

    /// A file name that is not UTF-8.
    fn not_text() -> OsString {
        OsString::from_vec(b"/x/\xff.txt".to_vec())
    }

    /// FAILS IF: a word no door reads is made text — a trailing word that is
    /// not UTF-8 would then stop a door that takes only its first word, as
    /// it never did.
    #[test]
    fn a_word_no_door_reads_is_never_made_text() {
        let words: Words = [OsString::from("lucide"), not_text()].into_iter().collect();
        assert_eq!(words.first().as_deref(), Some("lucide"));
        let none: Words = std::iter::empty::<OsString>().collect();
        assert_eq!(none.first(), None);
        assert!(none.all().is_empty());
    }

    /// FAILS IF: a door that takes every word is handed them out of order,
    /// or short of one.
    #[test]
    fn every_word_is_read_in_its_order() {
        let words: Words = ["--drive", "usb-1"].into_iter().collect();
        assert_eq!(words.all(), ["--drive", "usb-1"]);
    }

    /// FAILS IF: a word a door reads as text, and is not UTF-8, reaches the
    /// door as some other text — a file name quietly turned into another one
    /// — instead of being refused.
    #[test]
    #[should_panic(expected = "is not UTF-8")]
    fn a_word_a_door_reads_that_is_not_text_is_refused() {
        let words: Words = [OsString::from("lucide"), not_text()].into_iter().collect();
        let _ = words.all();
    }
}
