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

use std::path::{Path, PathBuf};

use crate::boot::{self, Entry};
use crate::{System, argv, run, say, system};

/// GRUB's variable for the trial's default.
pub const GRUB_DEFAULT: &str = "ronoctua_default";

/// GRUB's variable for the boot of once, which its `grub.cfg` clears.
pub const GRUB_ONESHOT: &str = "next_entry";

/// The loader, and the tool that writes what it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Loader {
    /// systemd-boot, through `bootctl`.
    Systemd(PathBuf),
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
            return Some(Self::Systemd(bootctl.clone()));
        }
        system.grub_editenv.as_ref().map(|editenv| Self::Grub {
            editenv: editenv.clone(),
            dir: system.real(system::GRUB_DIR),
        })
    }

    /// What the loader keeps the trial in, for the sentences that say it.
    pub(crate) const fn variables(&self) -> &'static str {
        match self {
            Self::Systemd(_) => "the EFI variables",
            Self::Grub { .. } => "GRUB's environment block",
        }
    }

    /// The menu's entries; `None`, said, when they cannot be read.
    pub(crate) fn entries(&self) -> Option<Vec<Entry>> {
        match self {
            Self::Systemd(bootctl) => {
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
    pub(crate) fn set_default(&self, entry: &str) -> bool {
        match self {
            Self::Systemd(bootctl) => bootctl_word(bootctl, &argv::set_default(entry)),
            Self::Grub { editenv, dir } => grub_set(editenv, dir, GRUB_DEFAULT, entry),
        }
    }

    /// The entry the next boot tries once; an empty entry unsets it.
    pub(crate) fn set_oneshot(&self, entry: &str) -> bool {
        match self {
            Self::Systemd(bootctl) => bootctl_word(bootctl, &argv::set_oneshot(entry)),
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
