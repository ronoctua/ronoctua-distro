//! The action menu — as DATA, and never as a `match`.
//!
//! # Why a table
//!
//! The same reason `ronoctua_cmd::Registry` and `ronoctua_rescue::all_of` are
//! tables: a table can be checked and a `match` arm cannot. Every row here is
//! read TWICE — once by whatever draws the menu, once by whatever runs the
//! choice — and a check compares the rows against the two `.ftl` files and
//! against the key list in both directions.
//!
//! It is also the lesson `buffers/lucide` paid for: an affordance scattered
//! across four functions, with the drawing consulting none of them, gave
//! twenty-four identical-looking rows where the same key did three different
//! things. Here, what decides whether a row is offered and what decides whether
//! it runs are the same field.
//!
//! # And why that makes it extensible by FORMAT
//!
//! [`Applies::Under`] asks the MIME database whether a type descends from
//! another, so a row written for `application/zip` reaches the fifty types that
//! are a zip underneath. Adding "verify this signature" for
//! `application/pgp-signature` later is one row, not a new branch — which is
//! what the owner asked for when they said specific features for specific
//! formats would come.

use ronoctua_i18n::key;

use crate::{Entry, Kind, mime};

/// The prefix every action's text key carries.
///
/// The same discipline as `cmd-` and `rescue-`: the id and the key cannot
/// diverge, because one is derived from the other and a check compares them.
pub const KEY_PREFIX_OF: &str = "files-";

/// Whether an action asks who you are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Anyone at this keyboard. Everything that acts on your own files.
    Free,
    /// polkit asks, once, and remembers for the session.
    Password,
}

/// When a row is offered.
///
/// A row that is offered and then refuses is worse than a row that is not
/// offered: the person spent a gesture to be told no, and nothing says which
/// part of the state made the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// Always.
    Always,
    /// Only on a directory — following a link into one, because entering is
    /// what a link into a directory is for.
    Dir,
    /// Only on something that is not a directory.
    File,
    /// Only on a symlink, as the link and not as its target.
    Symlink,
    /// Only when something is on the clipboard.
    Pasteable,
    /// Only for a type that IS the named one, or descends from it.
    Under(&'static str),
    /// Only for something that IS in the trash.
    ///
    /// A "move to the trash" one cannot come back from is half an action, and
    /// the half that is missing is the one that makes the other safe. Being in
    /// the trash is a property of WHERE the entry is, which is why this variant
    /// is answered by the caller rather than by the entry.
    InTrash,
}

/// One row of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// The id, in kebab-case. Stable: it is what a check and a keymap name.
    pub id: &'static str,
    /// The text key, always `files-<id>`.
    pub label: &'static str,
    /// The key that runs it, in the menu's own keymap.
    pub key: &'static str,
    /// When it is offered.
    pub applies: Applies,
    /// Whether it asks who you are.
    pub tier: Tier,
    /// Whether the same key has to be pressed twice.
    ///
    /// Only for what cannot be undone. A confirmation on a reversible action
    /// teaches people to press twice without reading, which is exactly what
    /// makes the confirmation on the irreversible one useless.
    pub confirms: bool,
}

/// Every row, in the order the menu draws them.
///
/// The order is by FREQUENCY and not alphabetical: opening is first because it
/// is what most menus are opened for, and destroying is late because a gesture
/// that lands one row off should not land on `delete`.
#[must_use]
pub fn all_of() -> &'static [Action] {
    const ROWS: &[Action] = &[
        // --- getting at the thing ------------------------------------
        Action {
            id: "open",
            label: key::FILES_OPEN,
            key: "RET",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "open-with",
            label: key::FILES_OPEN_WITH,
            key: "o",
            applies: Applies::File,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "terminal",
            label: key::FILES_TERMINAL,
            key: "t",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        // `pkexec` asks, and the terminal it opens is visibly a root terminal.
        Action {
            id: "root-terminal",
            label: key::FILES_ROOT_TERMINAL,
            key: "!",
            applies: Applies::Always,
            tier: Tier::Password,
            confirms: false,
        },
        // --- moving it about -----------------------------------------
        Action {
            id: "copy-path",
            label: key::FILES_COPY_PATH,
            key: "y",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "copy",
            label: key::FILES_COPY,
            key: "c",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "cut",
            label: key::FILES_CUT,
            key: "x",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "paste",
            label: key::FILES_PASTE,
            key: "p",
            applies: Applies::Pasteable,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "rename",
            label: key::FILES_RENAME,
            key: "r",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        // --- what it is ----------------------------------------------
        Action {
            id: "info",
            label: key::FILES_INFO,
            key: "i",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "mode",
            label: key::FILES_MODE,
            key: "m",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "owner",
            label: key::FILES_OWNER,
            key: "O",
            applies: Applies::Always,
            tier: Tier::Password,
            confirms: false,
        },
        Action {
            id: "group",
            label: key::FILES_GROUP,
            key: "G",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        // --- archives, and the first rows that exist because of a FORMAT ---
        //
        // `Applies::Under` asks the database whether a type DESCENDS from
        // another, so a row written for `application/zip` reaches the fifty
        // types that are a zip underneath. A future row for one specific
        // format is one line here, not a new branch.
        Action {
            id: "compress",
            label: key::FILES_COMPRESS,
            key: "z",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "extract",
            label: key::FILES_EXTRACT,
            key: "u",
            applies: Applies::Under("application/zip"),
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "extract-tar",
            label: key::FILES_EXTRACT_TAR,
            key: "U",
            applies: Applies::Under("application/x-tar"),
            tier: Tier::Free,
            confirms: false,
        },
        // --- getting rid of it ---------------------------------------
        //
        // Last, and in this order: the reversible one first, and the two
        // that are not come after it with a confirmation each. A gesture
        // that lands one row off should not land on `delete`.
        Action {
            id: "restore",
            label: key::FILES_RESTORE,
            key: "R",
            applies: Applies::InTrash,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "trash",
            label: key::FILES_TRASH,
            key: "d",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: false,
        },
        Action {
            id: "delete",
            label: key::FILES_DELETE,
            key: "D",
            applies: Applies::Always,
            tier: Tier::Free,
            confirms: true,
        },
        Action {
            id: "shred",
            label: key::FILES_SHRED,
            key: "Z",
            applies: Applies::File,
            tier: Tier::Free,
            confirms: true,
        },
    ];

    ROWS
}

/// The rows that apply to this entry, in menu order.
///
/// `pasteable` is whether the clipboard holds anything: a `paste` row on an
/// empty clipboard is a row that would refuse.
#[must_use]
pub fn for_entry(
    entry: &Entry,
    of_type: &str,
    pasteable: bool,
    in_trash: bool,
) -> Vec<&'static Action> {
    all_of()
        .iter()
        .filter(|a| offered(a.applies, entry, of_type, pasteable, in_trash))
        .collect()
}

/// Whether one row applies.
#[must_use]
pub fn offered(
    applies: Applies,
    entry: &Entry,
    of_type: &str,
    pasteable: bool,
    in_trash: bool,
) -> bool {
    match applies {
        Applies::Always => true,
        Applies::InTrash => in_trash,
        Applies::Dir => matches!(entry.opens_as(), Kind::Directory),
        Applies::File => !matches!(entry.opens_as(), Kind::Directory),
        Applies::Symlink => matches!(entry.kind, Kind::Symlink),
        Applies::Pasteable => pasteable,
        Applies::Under(parent) => mime::is_a(of_type, parent),
    }
}

/// The row a key runs, if any.
#[must_use]
pub fn by_key(key: &str) -> Option<&'static Action> {
    all_of().iter().find(|a| a.key == key)
}

/// The row with this id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Action> {
    all_of().iter().find(|a| a.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: Kind) -> Entry {
        Entry {
            name: name.to_owned(),
            kind,
            mode: 0o100_644,
            links: 1,
            uid: 1000,
            gid: 100,
            bytes: 1,
            modified: 0,
            target: None,
            aims_at: None,
        }
    }

    #[test]
    fn the_key_of_every_row_is_its_own_id() {
        // THE DEFECT THIS CATCHES: a row copied with the id changed and the
        // text key forgotten. Both constants exist, so it COMPILES, and the
        // menu shows the neighbouring row's words. It is the exact defect
        // `ronoctua_cmd::Registry` was built to refuse, and this is the same
        // rule in the same shape.
        for a in all_of() {
            assert_eq!(
                a.label,
                format!("{KEY_PREFIX_OF}{}", a.id),
                "a linha `{}` mostra o texto de outra",
                a.id
            );
        }
    }

    #[test]
    fn no_two_rows_share_a_key_or_an_id() {
        for (i, a) in all_of().iter().enumerate() {
            for b in all_of().iter().skip(i + 1) {
                assert_ne!(a.key, b.key, "`{}` e `{}` na mesma tecla", a.id, b.id);
                assert_ne!(a.id, b.id, "id repetido: {}", a.id);
            }
        }
    }

    #[test]
    fn every_row_has_a_key_so_nothing_is_mouse_only() {
        // The owner's rule, mechanised: whatever the mouse can do the keyboard
        // can do. A row with no key would be reachable only by pointing at it.
        for a in all_of() {
            assert!(!a.key.is_empty(), "a linha `{}` nao tem tecla", a.id);
            assert!(!a.id.is_empty());
            assert!(
                a.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "o id `{}` nao e kebab-case",
                a.id
            );
        }
    }

    #[test]
    fn only_the_irreversible_rows_ask_twice() {
        // A confirmation on something undoable teaches people to press twice
        // without reading, which is what makes the one that matters useless.
        for a in all_of() {
            let irreversible = matches!(a.id, "delete" | "shred");
            assert_eq!(
                a.confirms, irreversible,
                "`{}` confirma={} e irreversivel={}",
                a.id, a.confirms, irreversible
            );
        }
    }

    #[test]
    fn the_trash_does_not_ask_and_the_two_that_destroy_do() {
        // Trashing is the reversible one, and making it ask would push people
        // towards `delete` — the opposite of what a trash is for.
        assert!(!by_id("trash").unwrap().confirms);
        assert!(by_id("delete").unwrap().confirms);
        assert!(by_id("shred").unwrap().confirms);
    }

    #[test]
    fn a_directory_is_not_offered_what_cannot_be_done_to_it() {
        let dir = entry("pasta", Kind::Directory);
        let offered: Vec<_> = for_entry(&dir, mime::DIRECTORY, false, false)
            .iter()
            .map(|a| a.id)
            .collect();
        // Overwriting a tree is a different operation with a different
        // confirmation; offering it here would be a promise this refuses.
        assert!(
            !offered.contains(&"shred"),
            "ofereceu sobrescrever uma pasta"
        );
        assert!(!offered.contains(&"open-with"));
        assert!(offered.contains(&"open"));
        assert!(offered.contains(&"trash"));
    }

    #[test]
    fn extract_appears_for_what_the_database_says_is_an_archive() {
        // The row is written for `application/zip` and reaches everything that
        // descends from it — which is what makes a future row for a specific
        // format one line instead of a new branch.
        let file = entry("documento.odt", Kind::File);
        let odt = "application/vnd.oasis.opendocument.text";
        assert!(
            for_entry(&file, odt, false, false)
                .iter()
                .any(|a| a.id == "extract"),
            "um .odt E um zip, e nada ofereceu"
        );

        let png = entry("foto.png", Kind::File);
        let offered: Vec<_> = for_entry(&png, "image/png", false, false)
            .iter()
            .map(|a| a.id)
            .collect();
        assert!(!offered.contains(&"extract"));
        assert!(!offered.contains(&"extract-tar"));
    }

    #[test]
    fn paste_only_shows_when_there_is_something_to_paste() {
        let file = entry("x", Kind::File);
        assert!(
            !for_entry(&file, "text/plain", false, false)
                .iter()
                .any(|a| a.id == "paste")
        );
        assert!(
            for_entry(&file, "text/plain", true, false)
                .iter()
                .any(|a| a.id == "paste")
        );
    }

    #[test]
    fn a_link_into_a_directory_behaves_like_the_directory() {
        // Opening follows the link — that is what a link is for. So the rows
        // that depend on "is this a place" have to follow it too, or the menu
        // and the Enter key disagree about the same row.
        let mut link = entry("atalho", Kind::Symlink);
        link.aims_at = Some(Kind::Directory);
        assert!(
            !for_entry(&link, mime::SYMLINK, false, false)
                .iter()
                .any(|a| a.id == "shred")
        );
    }

    #[test]
    fn only_what_genuinely_needs_root_asks_for_a_password() {
        // Seeing is free, and so is everything one can do to one's own files.
        // A password on `copy` would train the person to type it, which is the
        // thing that makes the password on `chown` worth anything.
        for a in all_of() {
            let expected = matches!(a.id, "root-terminal" | "owner");
            assert_eq!(
                a.tier == Tier::Password,
                expected,
                "`{}` pede senha e nao devia (ou o contrario)",
                a.id
            );
        }
    }

    #[test]
    fn a_key_and_an_id_find_the_same_row() {
        for a in all_of() {
            assert_eq!(by_key(a.key).map(|r| r.id), Some(a.id));
            assert_eq!(by_id(a.id).map(|r| r.key), Some(a.key));
        }
        assert!(by_key("nao-existe").is_none());
        assert!(by_id("nao-existe").is_none());
    }
}
