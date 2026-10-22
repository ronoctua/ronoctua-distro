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

/// The loader, and the tool that writes what it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Loader {
    /// systemd-boot, through `bootctl`.
    Systemd {
        /// `bootctl`.
        bootctl: PathBuf,
        /// Its `loader.conf`, on the disk.
        conf: PathBuf,
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
            Self::Systemd { bootctl, conf } => {
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
