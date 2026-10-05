//! What one can do with a row of the sidebar.
//!
//! # Why a table of its own, and not more rows in `menu.rs`
//!
//! Because the subject is a different kind of thing. `menu::Action` is about an
//! `Entry` — a name in a directory, with an owner, a mode and a type — and
//! every one of its `Applies` variants asks a question only an entry can
//! answer. A sidebar row is a PLACE, a device or the trash; "is it a symlink"
//! and "is it pasteable" are not questions about it.
//!
//! Sharing one table would have meant a second `Applies` inside the first, and
//! twenty rows answering "not for this kind" — which is exactly the shape
//! `Applies` exists to avoid.
//!
//! # The facts come first, and that is the same surface
//!
//! Pressing an unmounted device does not mount it: it opens THIS, with what the
//! device is at the top and what can be done with it below, each row showing
//! its key. That was asked for from the screen — *"mostrar informações sobre o
//! item, e mostrar a opção de montá-lo"* — and it needs no second mechanism,
//! because a menu that names its subject is what the listing's menu already is.

use crate::sidebar::{Row, Target};

/// The prefix every action's text key carries.
///
/// The same discipline as `menu::KEY_PREFIX_OF` and `cmd-`: the id and the key
/// cannot diverge, because one is derived from the other and a check compares
/// them.
pub const KEY_PREFIX_OF: &str = "sidebar-";

/// When a row is offered.
///
/// A row that is offered and then refuses is worse than a row that is not
/// offered: the person spent a gesture to be told no, and nothing says which
/// part of the state made the difference. That sentence is `menu::Applies`'s
/// and it is repeated here because it is the whole reason this enum exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// Always.
    Always,
    /// A folder, a file or an application — anything that is not a device.
    Place,
    /// Any device, mounted or not.
    Device,
    /// A device with a filesystem mounted right now.
    Mounted,
    /// A device with nothing mounted.
    Unmounted,
    /// A mounted device the running system does not need.
    MountedSpare,
    /// A mounted device on a drive that can be PULLED OUT, and that the
    /// running system does not need.
    ///
    /// Both halves. Ejecting cuts the power to a drive: on something soldered
    /// in it means nothing, and on the disk the system booted from it means
    /// the machine stops.
    ///
    /// AN ENCRYPTED ONE MOUNTED OR NOT: a locked stick has nothing mounted
    /// and is still a stick one pulls out, and ejecting is how its power is
    /// cut; an open one that is not mounted is locked on the way.
    Removable,
    /// A device the running system does NOT need.
    ///
    /// It is what stands between a menu row and somebody formatting the disk
    /// they are running from. The fact is resolved by the disk border and
    /// carried on the row: this crate does not know what a volume is.
    Spare,
    /// An encrypted device that is open, and that the running system does
    /// not need: what locking it again is offered on.
    Unlocked,
}

/// One row of the sidebar's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// The id, in kebab-case. Stable: a check names it.
    pub id: &'static str,
    /// The text key, always `sidebar-<id>`.
    pub label: &'static str,
    /// What it does, for somebody who already knows what they want.
    ///
    /// # The same row answering two people
    ///
    /// [`Self::label`] is written for whoever is looking for a verb: *"apagar
    /// de vez"*. This is written for whoever wants to know exactly what will
    /// happen: which call, which flags, what is recoverable. It reaches the
    /// message line when the row is under the CURSOR — not only under the
    /// pointer, because the owner's rule is that whatever the mouse can do the
    /// keyboard can do, and until this existed the explanations were a thing
    /// only a mouse could ask for.
    ///
    /// Always `{prefix}-does-<id>`, so the check can find both halves.
    pub does: &'static str,
    /// The key that runs it, in this menu's own keymap.
    pub key: &'static str,
    /// When it is offered.
    pub applies: Applies,
}

/// Every row, in the order the menu draws them.
///
/// By FREQUENCY and not alphabetically, like the listing's: opening is first
/// because it is what a menu is opened for, and formatting is last because a
/// gesture that lands one row off must not land on it.
#[must_use]
pub fn all_of() -> &'static [Action] {
    const ROWS: &[Action] = &[
        Action {
            id: "open",
            label: key::SIDEBAR_OPEN,
            does: key::SIDEBAR_DOES_OPEN,
            key: "RET",
            applies: Applies::Always,
        },
        Action {
            id: "mount",
            label: key::SIDEBAR_MOUNT,
            does: key::SIDEBAR_DOES_MOUNT,
            key: "m",
            applies: Applies::Unmounted,
        },
        Action {
            id: "unmount",
            label: key::SIDEBAR_UNMOUNT,
            does: key::SIDEBAR_DOES_UNMOUNT,
            key: "u",
            // MOUNTED AND SPARE, and the second half is the guard. A row that
            // offered to unmount the running system and then refused would be
            // the offer teaching that the refusal is arbitrary.
            //
            // Not `Removable`: a spare INTERNAL partition — somebody's data
            // disk — is perfectly unmountable, and only ejecting is about the
            // drive being pullable.
            applies: Applies::MountedSpare,
        },
        Action {
            id: "eject",
            label: key::SIDEBAR_EJECT,
            does: key::SIDEBAR_DOES_EJECT,
            key: "e",
            applies: Applies::Removable,
        },
        // LOCKING AGAIN, WITHOUT CUTTING THE POWER: an internal disk, or a
        // stick one will open again in a minute. Ejecting locks too, on its
        // way to the power.
        Action {
            id: "lock",
            label: key::SIDEBAR_LOCK,
            does: key::SIDEBAR_DOES_LOCK,
            key: "l",
            applies: Applies::Unlocked,
        },
        Action {
            id: "information",
            label: key::SIDEBAR_INFORMATION,
            does: key::SIDEBAR_DOES_INFORMATION,
            key: "i",
            applies: Applies::Device,
        },
        Action {
            id: "unpin",
            label: key::SIDEBAR_UNPIN,
            does: key::SIDEBAR_DOES_UNPIN,
            key: "d",
            applies: Applies::Always,
        },
        // LAST, AND ON PURPOSE. It is the one row here that cannot be undone,
        // and the listing's menu puts `delete` late for the same reason: a
        // gesture that lands one row off should not land on it.
        Action {
            id: "format",
            label: key::SIDEBAR_FORMAT,
            does: key::SIDEBAR_DOES_FORMAT,
            key: "F",
            applies: Applies::Spare,
        },
    ];
    ROWS
}

/// The rows offered for this sidebar row.
#[must_use]
pub fn for_row(row: &Row) -> Vec<&'static Action> {
    all_of()
        .iter()
        .filter(|a| offered(a.applies, row))
        .collect()
}

/// Whether one row is offered for this sidebar row.
#[must_use]
pub fn offered(applies: Applies, row: &Row) -> bool {
    let device = matches!(row.target, Target::Device(_));
    // WHAT THE SYSTEM NEEDS IS NOT SPARE, and that is one test standing between
    // a menu row and somebody formatting the disk they booted from.
    let spare = device && row.needed.is_none();
    match applies {
        Applies::Always => true,
        Applies::Place => !device,
        Applies::Device => device,
        Applies::Mounted => device && row.mounted == Some(true),
        Applies::MountedSpare => spare && row.mounted == Some(true),
        Applies::Unmounted => device && row.mounted == Some(false),
        Applies::Removable => {
            spare && row.removable && (row.mounted == Some(true) || row.seal.is_some())
        }
        Applies::Spare => spare,
        Applies::Unlocked => spare && row.seal == Some(crate::sidebar::Seal::Unlocked),
    }
}

/// The action a key runs, if this menu has one for it.
#[must_use]
pub fn by_key(key: &str) -> Option<&'static Action> {
    all_of().iter().find(|a| a.key == key)
}

/// The action with this id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Action> {
    all_of().iter().find(|a| a.id == id)
}

/// The text keys this table names.
///
/// Written out and not built with `format!`, for the reason the whole project
/// gives: a key assembled at runtime is invisible to grep and to the check that
/// compares this list against both `.ftl` files.
pub mod key {
    pub use ronoctua_i18n::key::{
        SIDEBAR_DOES_EJECT, SIDEBAR_DOES_FORMAT, SIDEBAR_DOES_INFORMATION, SIDEBAR_DOES_LOCK,
        SIDEBAR_DOES_MOUNT, SIDEBAR_DOES_OPEN, SIDEBAR_DOES_UNMOUNT, SIDEBAR_DOES_UNPIN,
        SIDEBAR_EJECT, SIDEBAR_FORMAT, SIDEBAR_INFORMATION, SIDEBAR_LOCK, SIDEBAR_MOUNT,
        SIDEBAR_OPEN, SIDEBAR_UNMOUNT, SIDEBAR_UNPIN,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::Section;
    use std::path::PathBuf;

    fn a_device(mounted: bool, removable: bool, needed: Option<&str>) -> Row {
        Row {
            id: "device:x".to_owned(),
            target: Target::Device("/block/x".to_owned()),
            label: Some("x".to_owned()),
            section: Section::Devices,
            mounted: Some(mounted),
            at: mounted.then(|| PathBuf::from("/run/media/x")),
            quiet: false,
            system: needed.is_some(),
            needed: needed.map(ToOwned::to_owned),
            removable,
            built_in: true,
            seal: None,
        }
    }

    fn a_place() -> Row {
        Row {
            id: "place:/x".to_owned(),
            target: Target::Place(PathBuf::from("/x")),
            label: Some("x".to_owned()),
            section: Section::Pinned,
            mounted: None,
            at: None,
            quiet: false,
            system: false,
            needed: None,
            removable: false,
            built_in: false,
            seal: None,
        }
    }

    fn ids(row: &Row) -> Vec<&'static str> {
        for_row(row).iter().map(|a| a.id).collect()
    }

    #[test]
    fn the_running_system_is_never_offered_formatting_or_unmounting() {
        // THE ROW THIS TABLE EXISTS TO NOT DRAW. A file manager that puts
        // "format" one key away from the disk it booted from is a file manager
        // that erases somebody's system, and a refusal after the offer would
        // teach that the refusal is arbitrary.
        let root = a_device(true, false, Some("/"));
        let offered = ids(&root);
        assert!(!offered.contains(&"format"), "{offered:?}");
        assert!(!offered.contains(&"unmount"), "{offered:?}");
        assert!(!offered.contains(&"eject"), "{offered:?}");
        // And it is still a row one can go to and ask about.
        assert!(offered.contains(&"open") && offered.contains(&"information"));
    }

    #[test]
    fn an_internal_data_disk_unmounts_but_is_not_ejected() {
        // A spare partition on a soldered-in disk: unmounting it is ordinary,
        // and cutting the power to the drive it shares with the system is not.
        let data = ids(&a_device(true, false, None));
        assert!(data.contains(&"unmount"), "{data:?}");
        assert!(!data.contains(&"eject"), "{data:?}");
    }

    #[test]
    fn a_pendrive_gets_everything() {
        let stick = ids(&a_device(true, true, None));
        for wanted in ["open", "unmount", "eject", "information", "unpin", "format"] {
            assert!(stick.contains(&wanted), "{wanted} is missing: {stick:?}");
        }
        // Mounted, so mounting is not offered.
        assert!(!stick.contains(&"mount"));
    }

    #[test]
    fn an_unmounted_stick_offers_mounting_and_not_unmounting() {
        let cold = ids(&a_device(false, true, None));
        assert!(cold.contains(&"mount"));
        assert!(!cold.contains(&"unmount") && !cold.contains(&"eject"));
        // AND FORMATTING, which is the case somebody opens this for: a stick
        // that will not mount is usually a stick to be reformatted.
        assert!(cold.contains(&"format"));
    }

    #[test]
    fn a_locked_stick_mounts_by_its_passphrase_and_an_open_one_locks_again() {
        // LOCKED: unmounted, so `mount` is on the menu — and it is the row
        // that asks for the passphrase. Nothing to lock.
        let mut locked = a_device(false, true, None);
        locked.seal = Some(crate::sidebar::Seal::Locked);
        let offered = ids(&locked);
        assert!(
            offered.contains(&"mount") && offered.contains(&"format"),
            "{offered:?}"
        );
        assert!(!offered.contains(&"lock"), "{offered:?}");
        // AND IT EJECTS, mounted or not: a locked stick is still a stick one
        // pulls out, and ejecting is how its power is cut.
        assert!(offered.contains(&"eject"), "{offered:?}");
        // AN OPEN ONE THAT IS NOT MOUNTED ejects too, locked on the way.
        let mut open_idle = a_device(false, true, None);
        open_idle.seal = Some(crate::sidebar::Seal::Unlocked);
        assert!(ids(&open_idle).contains(&"eject"));
        // AND A PLAIN STICK THAT IS NOT MOUNTED is as it was.
        assert!(!ids(&a_device(false, true, None)).contains(&"eject"));
        // OPEN AND MOUNTED: it unmounts, ejects (locking on the way) and
        // locks without the power going.
        let mut open = a_device(true, true, None);
        open.seal = Some(crate::sidebar::Seal::Unlocked);
        let offered = ids(&open);
        for wanted in ["unmount", "eject", "lock"] {
            assert!(offered.contains(&wanted), "{wanted}: {offered:?}");
        }
        // AND AN OPEN ONE THE SYSTEM NEEDS is not offered locking: that is the
        // running system's own disk.
        let mut root = a_device(true, false, Some("/"));
        root.seal = Some(crate::sidebar::Seal::Unlocked);
        assert!(!ids(&root).contains(&"lock"));
    }

    #[test]
    fn a_folder_is_not_a_disk() {
        let place = ids(&a_place());
        assert_eq!(place, ["open", "unpin"]);
    }

    #[test]
    fn no_two_rows_share_a_key() {
        let mut keys: Vec<&str> = all_of().iter().map(|a| a.key).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "two menu rows with the same key");
    }

    #[test]
    fn every_label_is_its_own_id() {
        // The same guard `menu.rs` has: a label that is another row's key
        // compiles, and which-key draws the wrong sentence with no error.
        for a in all_of() {
            assert_eq!(
                a.label,
                format!("{KEY_PREFIX_OF}{}", a.id),
                "the row `{}` shows another one's text",
                a.id
            );
        }
    }
}
