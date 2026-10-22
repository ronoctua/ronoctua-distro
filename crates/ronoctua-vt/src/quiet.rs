//! The kernel's console held quiet while a terminal UI paints it.
//!
//! # Why
//!
//! The kernel writes its messages on the console the person is looking at,
//! over whatever is there, at the cursor — and a line that reaches the last
//! row scrolls the whole screen up. A terminal UI repaints only the cells it
//! believes changed, so what the kernel wrote stays, and the screen goes on
//! wrong: the installer's next question read `B longings repository` after a
//! radio associated, and the help under it carried the rest of the kernel's
//! line. The ISO boots at `loglevel=4`, so a driver's warning does the same on
//! a real machine; the test VMs boot at 7, where every association does.
//!
//! So while a terminal UI holds the console, the console's level is 1 (only
//! `KERN_EMERG`), and the level it had comes back when the UI lets go. The
//! messages still reach the kernel's ring and the journal.
//!
//! # Why here and not in `ronoctua-memory`
//!
//! The level is the CONSOLE's — the first of the four numbers of
//! `/proc/sys/kernel/printk` is how loud the kernel is on it —, and this crate
//! is the one that speaks to the kernel's console.

use std::path::{Path, PathBuf};

/// Where the kernel keeps the console's level: the first of four numbers.
pub const PRINTK: &str = "/proc/sys/kernel/printk";

/// The level a terminal UI holds the console at: below it nothing is
/// printed but an emergency (`KERN_EMERG` is 0).
pub const QUIET_LEVEL: u8 = 1;

/// The console's level lowered to [`QUIET_LEVEL`], and given back when this
/// is dropped.
#[derive(Debug)]
pub struct Quiet {
    /// The file the level was written to.
    at: PathBuf,
    /// The level it had.
    was: u8,
}

impl Quiet {
    /// On the kernel's console ([`crate::here`]), its level lowered; `None`
    /// on any other terminal, and where the level cannot be read or written
    /// — a rehearsal by somebody who is not root, which then paints as before.
    #[must_use]
    pub fn of_term() -> Option<Self> {
        crate::here().then(|| Self::at(Path::new(PRINTK))).flatten()
    }

    /// The level kept in `path` lowered. One number written: the kernel
    /// takes it as the first of the four and leaves the others.
    #[must_use]
    pub fn at(path: &Path) -> Option<Self> {
        let was = level_of(&std::fs::read_to_string(path).ok()?)?;
        std::fs::write(path, format!("{QUIET_LEVEL}\n")).ok()?;
        Some(Self {
            at: path.to_owned(),
            was,
        })
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.at, format!("{}\n", self.was));
    }
}

/// The console's level, out of `/proc/sys/kernel/printk`'s text: its first
/// number. Pure.
#[must_use]
pub fn level_of(printk: &str) -> Option<u8> {
    printk.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the console's level is read from another field than the
    /// first, is not lowered while the guard lives, or does not come back
    /// when it goes — or a file that does not say a level is lowered anyway.
    #[test]
    fn the_console_is_quiet_while_held_and_given_back() {
        assert_eq!(level_of("4\t4\t1\t7\n"), Some(4));
        assert_eq!(level_of("7 4 1 7"), Some(7));
        assert_eq!(level_of(""), None);
        assert_eq!(level_of("loud"), None);

        let dir = std::env::temp_dir().join(format!("ronoctua-vt-quiet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let printk = dir.join("printk");
        std::fs::write(&printk, "7\t4\t1\t7\n").unwrap();
        let level = || level_of(&std::fs::read_to_string(&printk).unwrap());
        {
            let held = Quiet::at(&printk);
            assert!(held.is_some());
            assert_eq!(level(), Some(QUIET_LEVEL), "not quiet while held");
        }
        assert_eq!(level(), Some(7), "the level was not given back");

        std::fs::write(&printk, "loud\n").unwrap();
        assert!(Quiet::at(&printk).is_none());
        assert_eq!(std::fs::read_to_string(&printk).unwrap(), "loud\n");
        assert!(Quiet::at(&dir.join("missing")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
