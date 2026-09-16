//! `drwxr-xr-x`, taken apart — one cell at a time, each able to say what it
//! means for THIS file.
//!
//! # Why a cell knows its own explanation
//!
//! Ten characters carry ten different facts and look like one word. Almost
//! nobody reads them left to right; people recognise `755` and guess the rest,
//! which is why "why can't the other user open my folder" is a question with a
//! thirty-year-old answer nobody remembers: on a DIRECTORY, `x` is not
//! "execute", it is "may enter", and `r` without `x` lists names one cannot
//! reach.
//!
//! So the difference between a file and a directory is not a footnote here — it
//! is in the sentence, chosen by the translation with a Fluent selector. The
//! word order and the choice belong to the language (the reason this project
//! uses Fluent), and what crosses from here is a KEY plus arguments, never a
//! sentence (invariant 8).

use ronoctua_i18n::key;

use crate::Kind;

/// Whose permission a bit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// The owning user.
    Owner,
    /// The owning group.
    Group,
    /// Everyone else.
    Other,
}

impl Who {
    /// The three, in the order they appear on screen.
    pub const ALL: [Self; 3] = [Self::Owner, Self::Group, Self::Other];

    /// How far to shift a `rwx` triplet for this one.
    const fn shift(self) -> u32 {
        match self {
            Self::Owner => 6,
            Self::Group => 3,
            Self::Other => 0,
        }
    }
}

/// What a bit permits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    /// Read: the content of a file, the names inside a directory.
    Read,
    /// Write: the content of a file, or CREATING AND DELETING inside a
    /// directory — which is why a file one cannot write can still be deleted.
    Write,
    /// Execute for a file; ENTER for a directory.
    Execute,
}

impl What {
    /// The three, in the order they appear on screen.
    pub const ALL: [Self; 3] = [Self::Read, Self::Write, Self::Execute];

    const fn bit(self) -> u32 {
        match self {
            Self::Read => 0o4,
            Self::Write => 0o2,
            Self::Execute => 0o1,
        }
    }

    const fn letter(self) -> char {
        match self {
            Self::Read => 'r',
            Self::Write => 'w',
            Self::Execute => 'x',
        }
    }
}

/// The three bits that are not about a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    /// Runs as the owner.
    SetUid,
    /// Runs as the group; on a directory, new files INHERIT the group.
    SetGid,
    /// Only the owner of a file may delete it — what `/tmp` is for.
    Sticky,
}

impl Special {
    const fn bit(self) -> u32 {
        match self {
            Self::SetUid => 0o4000,
            Self::SetGid => 0o2000,
            Self::Sticky => 0o1000,
        }
    }

    /// Which `x` position this bit lands on when it is set.
    const fn rides_on(self) -> Who {
        match self {
            Self::SetUid => Who::Owner,
            Self::SetGid => Who::Group,
            Self::Sticky => Who::Other,
        }
    }
}

/// What one character of the permission column stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    /// The first character: what the thing IS.
    Kind,
    /// One of the nine.
    Bit(Who, What),
}

/// One drawn character, and everything needed to explain it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// What is drawn.
    pub letter: char,
    /// What it stands for.
    pub segment: Segment,
    /// Whether the permission is granted.
    pub set: bool,
    /// The special bit riding on this position, when one is set.
    ///
    /// A `s` in the owner's `x` cell is TWO facts in one character, and the
    /// explanation has to give the one that surprises: that the program runs as
    /// somebody else.
    pub special: Option<Special>,
}

/// The ten characters of `drwxr-xr-x`, in order.
///
/// The kind comes from the mode itself and not from [`crate::Entry::kind`], so
/// this function stays pure: a `u32` in, ten cells out, testable without a
/// filesystem.
#[must_use]
pub fn cells(mode: u32) -> [Cell; 10] {
    let kind = Kind::of_mode(mode);
    let mut out = [Cell {
        letter: '-',
        segment: Segment::Kind,
        set: false,
        special: None,
    }; 10];
    out[0] = Cell {
        letter: kind_letter(kind),
        segment: Segment::Kind,
        set: true,
        special: None,
    };
    let mut at = 1;
    for who in Who::ALL {
        for what in What::ALL {
            let set = mode & (what.bit() << who.shift()) != 0;
            let special = [Special::SetUid, Special::SetGid, Special::Sticky]
                .into_iter()
                .find(|s| {
                    matches!(what, What::Execute) && s.rides_on() == who && mode & s.bit() != 0
                });
            // The special bit REPLACES the letter, and the case says whether
            // the `x` underneath is there: `s` means both, `S` means the
            // special bit on a position that cannot execute — which is almost
            // always a mistake, and the only way to see it is the case.
            let letter = match (special, set) {
                (Some(Special::Sticky), true) => 't',
                (Some(Special::Sticky), false) => 'T',
                (Some(_), true) => 's',
                (Some(_), false) => 'S',
                (None, true) => what.letter(),
                (None, false) => '-',
            };
            out[at] = Cell {
                letter,
                segment: Segment::Bit(who, what),
                set,
                special,
            };
            at += 1;
        }
    }
    out
}

/// The permission column as one string — `drwxr-xr-x`.
#[must_use]
pub fn shown(mode: u32) -> String {
    cells(mode).iter().map(|c| c.letter).collect()
}

/// The four octal digits, as `chmod` takes them.
///
/// Four and not three whenever a special bit is set: printing `755` for a mode
/// that is really `4755` would be the screen describing a file that is not
/// there.
#[must_use]
pub fn octal(mode: u32) -> String {
    let bits = mode & 0o7777;
    if bits & 0o7000 == 0 {
        format!("{:03o}", bits & 0o777)
    } else {
        format!("{bits:04o}")
    }
}

/// The mode with one bit flipped — what clicking a cell does.
#[must_use]
pub const fn toggled(mode: u32, who: Who, what: What) -> u32 {
    mode ^ (what.bit() << who.shift())
}

/// The mode with a special bit flipped.
#[must_use]
pub const fn toggled_special(mode: u32, special: Special) -> u32 {
    mode ^ special.bit()
}

/// The permission presets the `p` key offers, as `(octal, text key)`.
///
/// A short list on purpose. It is not a shortcut for typing — it is the answer
/// to "what should this be", and four good answers help where forty do not.
/// Typing an octal by hand stays available for whoever knows exactly what they
/// want.
pub const PRESETS: [(u32, &str); 5] = [
    (0o644, key::FILES_PRESET_FILE),
    (0o600, key::FILES_PRESET_PRIVATE),
    (0o755, key::FILES_PRESET_PROGRAM),
    (0o700, key::FILES_PRESET_PRIVATE_DIR),
    (0o444, key::FILES_PRESET_READONLY),
];

/// A sentence waiting for a language.
///
/// The key and its arguments, never the text: what turns this into Portuguese
/// or English is whoever holds the bundle (invariant 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// The text key.
    pub key: &'static str,
    /// The arguments, by name.
    pub args: Vec<(&'static str, String)>,
}

impl Explanation {
    /// The arguments in the shape `Texts::t_with` takes.
    #[must_use]
    pub fn borrowed(&self) -> Vec<(&str, &str)> {
        self.args.iter().map(|(k, v)| (*k, v.as_str())).collect()
    }
}

/// Which third of the octal a digit is, by its POSITION and how many there are.
///
/// # Why the count decides and not the index
///
/// `644` is three digits — owner, group, everybody else — and `4755` is four:
/// the first one carries the special bits. Counting from the LEFT with a fixed
/// meaning per index would call the `4` of `4755` the owner's, which is the
/// one reading that is always wrong. Counting from the RIGHT is what the
/// format actually promises, and that is what this does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The leading digit, when there is one: setuid, setgid and the sticky bit.
    Special,
    /// The owning user's digit.
    Owner,
    /// The owning group's digit.
    Group,
    /// Everybody else's digit.
    Other,
}

impl Part {
    /// The word the text key selects on. A `&'static str` and not the enum's
    /// `Debug`: a selector's branch name is part of the `.ftl`'s contract, and
    /// a derived spelling would change the day somebody renames a variant.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Special => "special",
            Self::Owner => "owner",
            Self::Group => "group",
            Self::Other => "other",
        }
    }
}

/// Which part the digit at `at` is, in a number of `digits` digits.
#[must_use]
pub const fn part_of(digits: usize, at: usize) -> Part {
    // FROM THE RIGHT: the last is everybody else, then the group, then the
    // owner, and anything before that is the special bits.
    match digits.saturating_sub(at) {
        1 => Part::Other,
        2 => Part::Group,
        3 => Part::Owner,
        _ => Part::Special,
    }
}

/// What this cell means for this file — the sentence the pointer asks for.
///
/// `owner` and `group` are the NAMES, already resolved: a sentence saying "the
/// group may write" is worth less than one saying "`users` may write", and the
/// person reading it wants to know whether they are in that group.
#[must_use]
pub fn explain(cell: Cell, mode: u32, owner: &str, group: &str) -> Explanation {
    let kind = Kind::of_mode(mode);
    let of_kind = kind_word(kind);
    // A special bit is the surprising fact in the cell, so it wins the
    // explanation: whoever points at an `s` is asking about the `s`.
    if let Some(special) = cell.special {
        return Explanation {
            key: match special {
                Special::SetUid => key::FILES_PERM_SETUID,
                Special::SetGid => key::FILES_PERM_SETGID,
                Special::Sticky => key::FILES_PERM_STICKY,
            },
            args: vec![
                ("kind", of_kind.to_owned()),
                ("set", yes_no(cell.set).to_owned()),
                ("who", owner.to_owned()),
                ("group", group.to_owned()),
            ],
        };
    }
    match cell.segment {
        Segment::Kind => Explanation {
            key: match kind {
                Kind::Directory => key::FILES_KIND_DIRECTORY,
                Kind::File => key::FILES_KIND_FILE,
                Kind::Symlink => key::FILES_KIND_SYMLINK,
                Kind::Fifo => key::FILES_KIND_FIFO,
                Kind::Socket => key::FILES_KIND_SOCKET,
                Kind::Block => key::FILES_KIND_BLOCK,
                Kind::Char => key::FILES_KIND_CHAR,
            },
            args: Vec::new(),
        },
        Segment::Bit(who, what) => Explanation {
            key: bit_key(who, what),
            args: vec![
                ("kind", of_kind.to_owned()),
                ("set", yes_no(cell.set).to_owned()),
                (
                    "who",
                    match who {
                        Who::Owner => owner.to_owned(),
                        Who::Group => group.to_owned(),
                        Who::Other => String::new(),
                    },
                ),
            ],
        },
    }
}

/// The nine keys, one per (whose, what).
///
/// Written out rather than assembled from two halves: a key built with
/// `format!` cannot be a `&'static str`, would not be checked by the generator
/// and would let a typo reach the screen as `[files-perm-onwer-read]`.
const fn bit_key(who: Who, what: What) -> &'static str {
    match (who, what) {
        (Who::Owner, What::Read) => key::FILES_PERM_OWNER_READ,
        (Who::Owner, What::Write) => key::FILES_PERM_OWNER_WRITE,
        (Who::Owner, What::Execute) => key::FILES_PERM_OWNER_EXECUTE,
        (Who::Group, What::Read) => key::FILES_PERM_GROUP_READ,
        (Who::Group, What::Write) => key::FILES_PERM_GROUP_WRITE,
        (Who::Group, What::Execute) => key::FILES_PERM_GROUP_EXECUTE,
        (Who::Other, What::Read) => key::FILES_PERM_OTHER_READ,
        (Who::Other, What::Write) => key::FILES_PERM_OTHER_WRITE,
        (Who::Other, What::Execute) => key::FILES_PERM_OTHER_EXECUTE,
    }
}

/// The value the `$set` selector branches on.
///
/// A word and not a boolean, because it crosses into Fluent as an argument and
/// a variant is named by a literal — `[yes]` and `*[no]`.
const fn yes_no(set: bool) -> &'static str {
    if set { "yes" } else { "no" }
}

/// The value the `$kind` selector branches on.
///
/// Only two branches matter to a permission sentence: a directory behaves
/// differently, everything else behaves like a file. A device node's `x` bit is
/// a question nobody has ever needed answered in a listing.
/// The word a text key selects on for this kind — `dir` or `file`.
///
/// PUBLIC because the octal's digits ask it too: what an `x` bit permits is
/// entering a directory and running a file, and the digit that carries it has
/// to say the same thing the letter beside it says. One word, one place.
pub const fn kind_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Directory => "dir",
        _ => "file",
    }
}

/// The first character of the permission column.
const fn kind_letter(kind: Kind) -> char {
    match kind {
        Kind::Directory => 'd',
        Kind::File => '-',
        Kind::Symlink => 'l',
        Kind::Fifo => 'p',
        Kind::Socket => 's',
        Kind::Block => 'b',
        Kind::Char => 'c',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_reads_like_ls() {
        assert_eq!(shown(0o040_755), "drwxr-xr-x");
        assert_eq!(shown(0o100_644), "-rw-r--r--");
        assert_eq!(shown(0o120_777), "lrwxrwxrwx");
        assert_eq!(shown(0o100_600), "-rw-------");
    }

    #[test]
    fn a_special_bit_replaces_the_x_and_the_case_says_which() {
        // `4755` runs as the owner AND is executable: lower-case `s`.
        assert_eq!(shown(0o104_755), "-rwsr-xr-x");
        // `4644` sets the bit on a file nobody can execute: upper-case `S`,
        // which is how one SEES the mistake. Without the case the column would
        // be identical to `0644` and the bit would be invisible.
        assert_eq!(shown(0o104_644), "-rwSr--r--");
        // `/tmp`: sticky and executable by all.
        assert_eq!(shown(0o041_777), "drwxrwxrwt");
        assert_eq!(shown(0o041_666), "drw-rw-rwT");
    }

    #[test]
    fn the_octal_grows_a_digit_when_a_special_bit_is_on() {
        // THE DEFECT THIS CATCHES: printing `755` for a mode that is really
        // `4755` — the screen describing a file that is not there, and an
        // octal one could copy into `chmod` to silently REMOVE the bit.
        assert_eq!(octal(0o100_755), "755");
        assert_eq!(octal(0o104_755), "4755");
        assert_eq!(octal(0o041_777), "1777");
        assert_eq!(octal(0o100_644), "644");
    }

    #[test]
    fn clicking_a_cell_flips_exactly_one_bit() {
        let before = 0o100_644;
        let after = toggled(before, Who::Group, What::Write);
        assert_eq!(octal(after), "664");
        // And back, which is what makes a click undoable by clicking again.
        assert_eq!(toggled(after, Who::Group, What::Write), before);
        // The file type bits are untouched — flipping a permission must never
        // turn a file into a socket.
        assert_eq!(after & 0o170_000, 0o100_000);
    }

    #[test]
    fn every_cell_has_an_explanation_and_none_of_them_is_the_same() {
        // THE DEFECT THIS CATCHES: two cells sharing a key, so the pointer
        // explains the neighbouring bit. Ten characters that look alike is
        // exactly where that would never be noticed.
        let mode = 0o040_755;
        let mut seen = Vec::new();
        for c in cells(mode) {
            let e = explain(c, mode, "ana", "users");
            assert!(!e.key.is_empty());
            seen.push((e.key, c.segment));
        }
        for (i, a) in seen.iter().enumerate() {
            for b in seen.iter().skip(i + 1) {
                assert_ne!(a.0, b.0, "two cells with the same key: {a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn the_explanation_carries_the_name_and_not_the_word_group() {
        let mode = 0o100_640;
        let cells = cells(mode);
        let group_read = cells[4];
        assert_eq!(group_read.segment, Segment::Bit(Who::Group, What::Read));
        let e = explain(group_read, mode, "ana", "users");
        assert!(e.args.contains(&("who", "users".to_owned())));
        assert!(e.args.contains(&("set", "yes".to_owned())));
        // And the one that is OFF says so, rather than not being explained.
        let other_read = cells[7];
        let e = explain(other_read, mode, "ana", "users");
        assert!(e.args.contains(&("set", "no".to_owned())));
    }

    #[test]
    fn a_directory_and_a_file_do_not_get_the_same_sentence() {
        // The whole reason the tooltip exists: on a directory `x` is "may
        // enter", and telling somebody it is "execute" is telling them
        // something false.
        let file = 0o100_755;
        let dir = 0o040_755;
        let of_file = explain(cells(file)[3], file, "ana", "users");
        let of_dir = explain(cells(dir)[3], dir, "ana", "users");
        assert_eq!(of_file.key, of_dir.key, "it is the same position");
        assert!(of_file.args.contains(&("kind", "file".to_owned())));
        assert!(of_dir.args.contains(&("kind", "dir".to_owned())));
    }

    #[test]
    fn a_special_bit_wins_the_explanation_of_its_cell() {
        let mode = 0o104_755;
        let e = explain(cells(mode)[3], mode, "root", "wheel");
        assert_eq!(e.key, key::FILES_PERM_SETUID);
    }
}
