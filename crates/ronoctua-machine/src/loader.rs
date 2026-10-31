//! The boot loader the trial is armed in: systemd-boot's EFI variables on a
//! UEFI machine, GRUB's environment block on a BIOS one
//! (`ronoctua.firmware`, `nix/modules/boot-loader.nix`).
//!
//! # One trial, two loaders
//!
//! The trial is two things the loader keeps across a reboot: a DEFAULT, the
//! last good entry, and a ONE-SHOT, the new entry, tried at the next boot and
//! forgotten by the loader itself. systemd-boot keeps them in
//! `LoaderEntryDefault` and `LoaderEntryOneShot` (`bootctl set-default`,
//! `set-oneshot`). GRUB keeps `next_entry` in its environment block, reads it
//! once and clears it — the one-shot, as NixOS's `grub.cfg` already handles
//! it — and the default is a variable of ours, `ronoctua_default`, read after
//! NixOS's own choice and never on a boot of once (`nix/modules/
//! boot-loader.nix`'s `extraConfig`, which also marks that boot on the
//! kernel's command line for the trial's deadline). An empty entry unsets
//! either, and the menu's own first entry — the profile's newest generation —
//! is the default again, on both.
//!
//! # A trial the loader cannot start
//!
//! Neither the deadline nor the bless sees a kernel that never ran. GRUB
//! falls back to the last good entry in the same boot (its `fallback`,
//! `nix/modules/boot-loader.nix`). systemd-boot restarts when the entry it
//! starts fails — but only with `reboot-on-error yes` in its `loader.conf`,
//! and its default, `auto`, says no to entries with no boot counting, which
//! NixOS's never have. So the line is the trial's, as boot counting's would
//! be: written while the default is posted — while a trial is armed, and
//! the default is the last good system, which this loader has started —
//! and gone when it is unset, which every verdict does. Written for good,
//! it would restart forever a machine whose newest generation cannot start
//! with no trial at all — the first boot after the installation —, where
//! the firmware would have tried the stick. The restart is the way back:
//! systemd-boot unsets `LoaderEntryOneShot` the moment it reads it.

use std::path::{Path, PathBuf};

use std::io::Write as _;

use crate::boot::{self, Entry};
use crate::{System, argv, run, say, system};

/// GRUB's variable for the trial's default.
pub const GRUB_DEFAULT: &str = "ronoctua_default";

/// GRUB's variable for the boot of once, which its `grub.cfg` clears.
pub const GRUB_ONESHOT: &str = "next_entry";

/// systemd-boot's configuration, on the ESP — `/boot` (`nix/disko`).
pub const LOADER_CONF: &str = "/boot/loader/loader.conf";

/// The line of [`LOADER_CONF`] that restarts the machine when the entry
/// systemd-boot starts fails — a trial's only, as a trial's default is.
pub const REBOOT_ON_ERROR: &str = "reboot-on-error yes";

/// systemd-boot's `LoaderEntryOneShot`, as the kernel shows the EFI variable
/// (`efivarfs`): four bytes of attributes, then the entry's id in UTF-16LE,
/// ended by a NUL. The boot loader removes it the moment a boot reads it.
pub const ONESHOT_VARIABLE: &str =
    "/sys/firmware/efi/efivars/LoaderEntryOneShot-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";

/// The loader, and the tool that writes what it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Loader {
    /// systemd-boot, through `bootctl`.
    Systemd {
        /// `bootctl`.
        bootctl: PathBuf,
        /// Its `loader.conf`, on the disk.
        conf: PathBuf,
        /// Its one-shot's EFI variable ([`ONESHOT_VARIABLE`]), read only.
        oneshot: PathBuf,
    },
    /// GRUB, through `grub-editenv` on the block beside its menu.
    Grub {
        /// `grub-editenv`.
        editenv: PathBuf,
        /// The folder of `grub.cfg` and `grubenv`, on the disk.
        dir: PathBuf,
    },
}

impl Loader {
    /// The loader the system names a tool for; `None` when it names none.
    pub(crate) fn of(system: &System) -> Option<Self> {
        if let Some(bootctl) = &system.bootctl {
            return Some(Self::Systemd {
                bootctl: bootctl.clone(),
                conf: system.real(LOADER_CONF),
                oneshot: system.real(ONESHOT_VARIABLE),
            });
        }
        system.grub_editenv.as_ref().map(|editenv| Self::Grub {
            editenv: editenv.clone(),
            dir: system.real(system::GRUB_DIR),
        })
    }

    /// What the loader keeps the trial in, for the sentences that say it.
    pub(crate) const fn variables(&self) -> &'static str {
        match self {
            Self::Systemd { .. } => "the EFI variables",
            Self::Grub { .. } => "GRUB's environment block",
        }
    }

    /// The menu's entries; `None`, said, when they cannot be read.
    pub(crate) fn entries(&self) -> Option<Vec<Entry>> {
        match self {
            Self::Systemd { bootctl, .. } => {
                let ran = run::capture(bootctl, &argv::list_entries()).ok()?;
                if !ran.ok() {
                    say(
                        4,
                        &format!("bootctl lists no entries: {}", ran.stderr.trim()),
                    );
                    return None;
                }
                let listed = boot::entries(&ran.stdout);
                if listed.is_none() {
                    say(4, "bootctl's list is not the JSON it should be");
                }
                listed
            }
            Self::Grub { dir, .. } => {
                let cfg = dir.join("grub.cfg");
                let Ok(text) = std::fs::read_to_string(&cfg) else {
                    say(4, &format!("GRUB's menu cannot be read: {}", cfg.display()));
                    return None;
                };
                let listed = boot::grub_entries(&text);
                if listed.is_none() {
                    say(4, &format!("GRUB's menu lists no entry: {}", cfg.display()));
                }
                listed
            }
        }
    }

    /// The default the next boots fall back to; an empty entry unsets it.
    /// On systemd-boot, the restart on a start that fails goes with it
    /// ([`REBOOT_ON_ERROR`]): posted only when the default is, gone whenever
    /// it is unset, written or not.
    pub(crate) fn set_default(&self, entry: &str) -> bool {
        match self {
            Self::Systemd { bootctl, conf, .. } => {
                let set = bootctl_word(bootctl, &argv::set_default(entry));
                if entry.is_empty() || set {
                    restart_on_error(conf, !entry.is_empty());
                }
                set
            }
            Self::Grub { editenv, dir } => grub_set(editenv, dir, GRUB_DEFAULT, entry),
        }
    }

    /// The entry the next boot tries once; an empty entry unsets it.
    pub(crate) fn set_oneshot(&self, entry: &str) -> bool {
        match self {
            Self::Systemd { bootctl, .. } => bootctl_word(bootctl, &argv::set_oneshot(entry)),
            Self::Grub { editenv, dir } => grub_set(editenv, dir, GRUB_ONESHOT, entry),
        }
    }

    /// Unsets both, so the menu's own first entry is the loader's choice.
    pub(crate) fn clear(&self) {
        self.set_oneshot("");
        self.set_default("");
    }

    /// The entry the next boot tries once, as the loader keeps it now: an
    /// empty one when none is set, `None` when it cannot be read. Both
    /// loaders forget it the moment a boot reads it, so a one-shot still set
    /// is a boot nobody has tried yet.
    pub(crate) fn oneshot(&self) -> Option<String> {
        match self {
            Self::Systemd { oneshot, .. } => match std::fs::read(oneshot) {
                Ok(bytes) => efi_text(&bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // NO VARIABLE in a folder of them is none set; no folder
                    // at all is a machine whose variables nobody can read.
                    oneshot.parent()?.is_dir().then(String::new)
                }
                Err(_) => None,
            },
            Self::Grub { editenv, dir } => {
                let block = dir.join("grubenv");
                if !block.exists() {
                    return Some(String::new());
                }
                let ran = run::capture(editenv, &argv::grub_list(&block)).ok()?;
                if !ran.ok() {
                    return None;
                }
                Some(
                    ran.stdout
                        .lines()
                        .find_map(|l| l.strip_prefix(GRUB_ONESHOT)?.strip_prefix('='))
                        .unwrap_or_default()
                        .to_owned(),
                )
            }
        }
    }
}

/// The text of an EFI variable that holds one, as `efivarfs` shows it: four
/// bytes of attributes, then UTF-16LE up to a NUL; `None` for bytes that are
/// not that.
fn efi_text(bytes: &[u8]) -> Option<String> {
    let units: Vec<u16> = bytes
        .get(4..)?
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}

/// One `bootctl` word; whether it worked, said when it did not.
fn bootctl_word(bootctl: &Path, words: &[String]) -> bool {
    match run::capture(bootctl, words) {
        Ok(ran) if ran.ok() => true,
        Ok(ran) => {
            say(
                4,
                &format!("bootctl {}: {}", words.join(" "), ran.stderr.trim()),
            );
            false
        }
        Err(_) => false,
    }
}

/// `conf` with [`REBOOT_ON_ERROR`] as its last line (`on`) or without it,
/// every other line as NixOS's builder wrote it. Not there or not
/// readable, it is said and left: without the line a start that fails goes
/// back to the firmware, as it always did, and a file written from nothing
/// would turn systemd-boot's own defaults on — its editor among them.
fn restart_on_error(conf: &Path, on: bool) {
    let Ok(text) = std::fs::read_to_string(conf) else {
        if on {
            say(
                4,
                &format!(
                    "{} cannot be read: a trial systemd-boot cannot start goes back to the firmware",
                    conf.display()
                ),
            );
        }
        return;
    };
    let mut kept: String = text
        .lines()
        .filter(|l| l.trim() != REBOOT_ON_ERROR)
        .flat_map(|l| [l, "\n"])
        .collect();
    if on {
        kept.push_str(REBOOT_ON_ERROR);
        kept.push('\n');
    }
    if kept == text {
        return;
    }
    if let Err(e) = write_on_fat(conf, kept.as_bytes()) {
        say(4, &format!("{}: {e}", conf.display()));
    }
}

/// `bytes` into `path` on the ESP: a sibling written whole and flushed,
/// renamed over it, and the folder flushed — the FAT keeps a rename in its
/// cache, and NixOS's builder syncs after every write for that. No mode:
/// the FAT's is its mount's, and setting one is refused.
fn write_on_fat(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let folder = path
        .parent()
        .ok_or_else(|| std::io::Error::other("not a file's path"))?;
    let draft = folder.join(format!(".ronoctua-{}.new", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&draft)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&draft, path)?;
        // The rename, flushed where the file system flushes a folder; the
        // file is whole either way.
        let _ = std::fs::File::open(folder).and_then(|f| f.sync_all());
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// `name` in GRUB's environment block — set to `value`, or unset when it is
/// empty —, the block made first when there is none (NixOS's `grub.cfg` reads
/// it only when it exists).
fn grub_set(editenv: &Path, dir: &Path, name: &str, value: &str) -> bool {
    let block = dir.join("grubenv");
    let word = |words: &[String]| match run::capture(editenv, words) {
        Ok(ran) if ran.ok() => true,
        Ok(ran) => {
            say(
                4,
                &format!("grub-editenv {}: {}", words.join(" "), ran.stderr.trim()),
            );
            false
        }
        Err(_) => false,
    };
    if !block.exists() && !word(&argv::grub_create(&block)) {
        return false;
    }
    word(&argv::grub_set(&block, name, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: systemd-boot's one-shot is not read as `efivarfs` shows it
    /// — attributes first, then UTF-16LE to the NUL —, or bytes that are no
    /// such text are read as an entry.
    #[test]
    fn the_one_shot_is_read_as_the_firmware_keeps_it() {
        let mut bytes = vec![7, 0, 0, 0];
        for u in "nixos-0a1b.conf".encode_utf16().chain([0]) {
            bytes.extend(u.to_le_bytes());
        }
        assert_eq!(efi_text(&bytes).as_deref(), Some("nixos-0a1b.conf"));
        assert_eq!(efi_text(&[7, 0, 0, 0]).as_deref(), Some(""));
        assert_eq!(efi_text(&[7, 0]), None);
        assert_eq!(
            efi_text(&[7, 0, 0, 0, 0x00, 0xd8]),
            None,
            "a lone surrogate"
        );
    }
}
