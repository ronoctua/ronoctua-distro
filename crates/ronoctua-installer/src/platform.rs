//! What the machine is, read before the first question about it: how its
//! firmware started it, and whether it has the TPM 2.0 the installation
//! seals into.
//!
//! # The TPM is required, and why
//!
//! The machine keeps secrets it has to open on its own, with nobody there to
//! type them — the Wi-Fi password first (`ronoctua_network::secret`, sealed
//! by `systemd-creds`): sealed in the TPM, a copy of the disk is not a copy of
//! them. Without one, a secret like that is either left readable or not kept
//! at all. So the installation does not start on a machine with no TPM 2.0
//! — the TPM switched off in the firmware's setup, most often — and the
//! screen says what it is, why, and how to switch it on (`Stop::Tpm`, the
//! TUI's).
//!
//! # BIOS is installed, and UEFI is recommended
//!
//! A machine started from BIOS (a UEFI one whose setup keeps the legacy
//! mode, CSM, on, most often: a machine with BIOS alone rarely has a TPM
//! 2.0) installs with GRUB (`ronoctua.firmware`,
//! `nix/modules/boot-loader.nix`), the boot trial and all. But UEFI is the
//! better way — systemd-boot, the distro's default, and in BIOS mode many
//! machines hide the TPM — so the screen says so, and how to switch it, and
//! BIOS goes on only by the person's choice (`Stop::Bios`), shown before the
//! TPM's for that reason.

use std::path::{Path, PathBuf};

/// The system root the machine is read under — tests only, as
/// `RONOCTUA_MACHINE_SYSROOT` is `ronoctua-machine`'s: a build's sandbox has
/// no `/sys/firmware/efi` and no TPM, and what it reads there is the
/// builder's, not the machine a test means. Not a `RONOCTUA_INST_` name: that
/// prefix is the answers', which leave the environment.
pub const SYSROOT: &str = "RONOCTUA_INSTALLER_SYSROOT";

/// The terminal-less installation's exit with no TPM 2.0 (after the
/// network's 3 and the repository's 4).
pub const EXIT_TPM: u8 = 5;

/// Its exit on a machine started from BIOS, BIOS not chosen.
pub const EXIT_BIOS: u8 = 6;

/// The variable the terminal-less installation chooses BIOS by, on a machine
/// started from it — the TUI's "worse alternative", said as plainly.
pub const FIRMWARE: &str = "RONOCTUA_INST_FIRMWARE";

/// Its value that chooses BIOS.
pub const CHOOSES_BIOS: &str = "bios";

/// The machine, as the installer needs to know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// The firmware started the machine through UEFI.
    pub uefi: bool,
    /// A TPM 2.0 is there for the installation to seal into.
    pub tpm2: bool,
}

/// What the tests' machines are: UEFI, with a TPM 2.0 — the machine every
/// screen was written for, with no stop on the way.
impl Default for Platform {
    fn default() -> Self {
        Self {
            uefi: true,
            tpm2: true,
        }
    }
}

impl Platform {
    /// This machine (or the one under [`SYSROOT`]).
    #[must_use]
    pub fn here() -> Self {
        let root = std::env::var_os(SYSROOT)
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from("/"), PathBuf::from);
        Self::under(&root)
    }

    /// A system root under `root` that reads as this platform — for the
    /// tests that run the installer where the real one is not the machine
    /// they mean ([`SYSROOT`]).
    ///
    /// # Errors
    ///
    /// When the folders cannot be made.
    pub fn plant(self, root: &Path) -> std::io::Result<()> {
        if self.uefi {
            std::fs::create_dir_all(root.join("sys/firmware/efi"))?;
        }
        if self.tpm2 {
            let tpm = root.join("sys/class/tpm/tpm0");
            std::fs::create_dir_all(&tpm)?;
            std::fs::write(tpm.join("tpm_version_major"), "2\n")?;
        }
        Ok(())
    }

    /// The machine whose `/sys` is under `root`: UEFI when the firmware
    /// left its variables there (`sys/firmware/efi`), and a TPM 2.0 when the
    /// kernel has one whose major version is 2 (`sys/class/tpm/tpm*/
    /// tpm_version_major`) — a TPM the firmware switched off is not there.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        let tpm2 = std::fs::read_dir(root.join("sys/class/tpm"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|t| {
                std::fs::read_to_string(t.path().join("tpm_version_major"))
                    .is_ok_and(|v| v.trim() == "2")
            });
        Self {
            uefi: root.join("sys/firmware/efi").is_dir(),
            tpm2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a machine with no EFI folder reads as UEFI, or a TPM that
    /// is not version 2 — or no TPM at all — reads as the one required.
    #[test]
    fn the_machine_is_read_off_its_sysfs() {
        let root = std::env::temp_dir().join(format!("ronoctua-platform-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(
            Platform::under(&root),
            Platform {
                uefi: false,
                tpm2: false
            }
        );
        let tpm = root.join("sys/class/tpm/tpm0");
        std::fs::create_dir_all(&tpm).unwrap();
        std::fs::write(tpm.join("tpm_version_major"), "1\n").unwrap();
        assert!(!Platform::under(&root).tpm2, "a TPM 1.2 is not the one");
        std::fs::write(tpm.join("tpm_version_major"), "2\n").unwrap();
        std::fs::create_dir_all(root.join("sys/firmware/efi")).unwrap();
        assert_eq!(Platform::under(&root), Platform::default());
        // AND A PLANTED ROOT reads as what was planted.
        let _ = std::fs::remove_dir_all(&root);
        for p in [
            Platform::default(),
            Platform {
                uefi: false,
                tpm2: true,
            },
            Platform {
                uefi: true,
                tpm2: false,
            },
        ] {
            let _ = std::fs::remove_dir_all(&root);
            p.plant(&root).unwrap();
            assert_eq!(Platform::under(&root), p);
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
