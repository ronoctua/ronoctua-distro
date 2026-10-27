//! The programs and the system's own links a verb works with, as the
//! environment names them — every program by its store path.
//!
//! The units (`nix/modules/machine.nix`, the rescue's rebuild) and the
//! wrapped command on the `PATH` set the same variables from one table. A
//! variable a verb needs and does not find is a refusal that names it, never
//! a guess on a `PATH`.
//!
//! # The system root, for tests only
//!
//! `RONOCTUA_MACHINE_SYSROOT` puts the system's own paths — the system
//! profile, `/run/booted-system`, `/run/current-system`, this boot's identity,
//! root's lock of the person's data and the store a toplevel lives in — under
//! another folder, the way a chroot would. A toplevel is recorded (in
//! `state.json`, in what `bootctl` lists) by its path INSIDE that root, and
//! found on the disk under it ([`System::real`], [`System::logical`]). With
//! it set, nothing of the real system is reachable through these paths, so
//! the verbs stop asking for root; unset, they run against `/` and refuse
//! anybody else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The machine directory.
pub const MACHINE_DIR: &str = "RONOCTUA_MACHINE_DIR";
/// Where `state.json` lives.
pub const MACHINE_STATE: &str = "RONOCTUA_MACHINE_STATE";
/// The update's folder under `/run`.
pub const MACHINE_RUN: &str = "RONOCTUA_MACHINE_RUN";
/// The `nixosConfigurations` attribute the machine flake builds.
pub const MACHINE_ATTR: &str = "RONOCTUA_MACHINE_ATTR";
/// A distro checkout to OVERRIDE the machine flake's `ronoctua` input with, as
/// a `path:` of it — what the build switch's own direction sets, so a root
/// apply builds `/etc/nixos`'s own system WITHOUT fetching its input when that
/// input is a user-owned `git+file` repo root may not read. Unset on every
/// other apply.
pub const MACHINE_OVERRIDE_DIR: &str = "RONOCTUA_MACHINE_OVERRIDE_DIR";
/// How many generations the boot menu lists.
pub const MENU_LIMIT: &str = "RONOCTUA_MACHINE_MENU_LIMIT";
/// How long the bless waits for root's lock of the person's data, in
/// seconds ([`crate::BLESS_LOCK_WAIT_S`] when unset).
pub const BLESS_WAIT: &str = "RONOCTUA_MACHINE_BLESS_WAIT_S";
/// The template of a machine flake (a folder holding its `flake.nix`).
pub const TEMPLATE: &str = "RONOCTUA_MACHINE_TEMPLATE";
/// Another address for the distro input, written into a migrated
/// machine's `flake.nix` in place of the template's.
pub const DISTRO: &str = "RONOCTUA_MACHINE_DISTRO";
/// Where, inside a toplevel, the lock its machine flake was built from is
/// (`etc/ronoctua/machine.lock`, put there by `nix/modules/machine.nix`).
pub const SYSTEM_LOCK: &str = "RONOCTUA_MACHINE_SYSTEM_LOCK";
/// The system root (tests only).
pub const SYSROOT: &str = "RONOCTUA_MACHINE_SYSROOT";
/// The machine's own `nix`; `nix-env` is the one beside it.
pub const NIX: &str = "RONOCTUA_NIX";
/// systemd's `bootctl` — a UEFI machine's.
pub const BOOTCTL: &str = "RONOCTUA_BOOTCTL";
/// GRUB's `grub-editenv` — a BIOS machine's (`crate::loader`).
pub const GRUB_EDITENV: &str = "RONOCTUA_GRUB_EDITENV";
/// GRUB's folder, its menu and its environment block, inside the system root.
pub const GRUB_DIR: &str = "/boot/grub";
/// How many compilations fit at once on this machine
/// (`ronoctua-build-sizing`, `nix/build-sizing.nix`).
pub const BUILD_SIZING: &str = "RONOCTUA_BUILD_SIZING";
/// What builds a system's units, each by its class (`ronoctua-build-units`,
/// `nix/build-sizing.nix`).
pub const BUILD_UNITS: &str = "RONOCTUA_BUILD_UNITS";
/// What roots the compiled units of a build (`ronoctua-keep-units`,
/// `nix/keep-units.nix`).
pub const KEEP_UNITS: &str = "RONOCTUA_KEEP_UNITS";

/// The system profile, inside the system root.
pub const PROFILE: &str = "/nix/var/nix/profiles/system";
/// The system that booted.
pub const BOOTED: &str = "/run/booted-system";
/// The system that runs.
pub const CURRENT: &str = "/run/current-system";
/// This boot's identity, new at every boot.
pub const BOOT_ID: &str = "/proc/sys/kernel/random/boot_id";

/// The variables, read once: the process's own, or a test's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env(BTreeMap<String, String>);

impl Env {
    /// The process's environment (only the variables this crate reads).
    #[must_use]
    pub fn of_process() -> Self {
        Self(
            [
                MACHINE_DIR,
                MACHINE_STATE,
                MACHINE_RUN,
                MACHINE_ATTR,
                MACHINE_OVERRIDE_DIR,
                MENU_LIMIT,
                BLESS_WAIT,
                TEMPLATE,
                DISTRO,
                SYSTEM_LOCK,
                SYSROOT,
                NIX,
                BOOTCTL,
                GRUB_EDITENV,
                BUILD_SIZING,
                BUILD_UNITS,
                KEEP_UNITS,
            ]
            .into_iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|v| !v.is_empty())
                    .map(|v| (name.to_owned(), v))
            })
            .collect(),
        )
    }

    /// These pairs.
    #[must_use]
    pub fn of(pairs: &[(&str, &str)]) -> Self {
        Self(
            pairs
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|&(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
        )
    }

    /// A variable's text.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    /// A variable as a path.
    #[must_use]
    pub fn path(&self, name: &str) -> Option<PathBuf> {
        self.get(name).map(PathBuf::from)
    }
}

/// The programs and the system's own links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    /// The system root (`/` but in tests).
    pub root: Option<PathBuf>,
    /// The attribute the machine flake builds.
    pub attr: Option<String>,
    /// A distro checkout to override the `ronoctua` input with (the build
    /// switch's own direction) — a `path:` of it, so the apply does not fetch
    /// a user-owned `git+file` input as root. `None` on every other apply.
    pub override_dir: Option<PathBuf>,
    /// The machine's `nix`.
    pub nix: Option<PathBuf>,
    /// `bootctl`.
    pub bootctl: Option<PathBuf>,
    /// `grub-editenv`.
    pub grub_editenv: Option<PathBuf>,
    /// How many generations the boot menu lists, as the variable says it.
    pub menu_limit: Option<String>,
    /// The template of a machine flake.
    pub template: Option<PathBuf>,
    /// The distro input's address for a migrated machine, when not the
    /// template's.
    pub distro: Option<String>,
    /// Where, inside a toplevel, the lock it was built from is.
    pub system_lock: Option<String>,
    /// How long the bless waits for root's lock of the person's data, in
    /// seconds, as the variable says it.
    pub bless_wait: Option<String>,
    /// The build sizing.
    pub sizing: Option<PathBuf>,
    /// What builds the units.
    pub build_units: Option<PathBuf>,
    /// What roots a build's units.
    pub keep_units: Option<PathBuf>,
}

impl System {
    /// The system `env` describes.
    #[must_use]
    pub fn from_env(env: &Env) -> Self {
        Self {
            root: env.path(SYSROOT).filter(|p| p != Path::new("/")),
            attr: env.get(MACHINE_ATTR).map(str::to_owned),
            override_dir: env.path(MACHINE_OVERRIDE_DIR),
            nix: env.path(NIX),
            bootctl: env.path(BOOTCTL),
            grub_editenv: env.path(GRUB_EDITENV),
            menu_limit: env.get(MENU_LIMIT).map(str::to_owned),
            template: env.path(TEMPLATE),
            distro: env.get(DISTRO).map(str::to_owned),
            system_lock: env
                .get(SYSTEM_LOCK)
                .filter(|p| is_inside(p))
                .map(str::to_owned),
            bless_wait: env.get(BLESS_WAIT).map(str::to_owned),
            sizing: env.path(BUILD_SIZING),
            build_units: env.path(BUILD_UNITS),
            keep_units: env.path(KEEP_UNITS),
        }
    }

    /// Whether the verbs act on the real system (and so need root).
    #[must_use]
    pub const fn is_the_real_one(&self) -> bool {
        self.root.is_none()
    }

    /// Where `logical` (a path as the system names it: `/nix/store/…`,
    /// `/run/booted-system`) is on the disk.
    #[must_use]
    pub fn real(&self, logical: &str) -> PathBuf {
        match &self.root {
            Some(root) => root.join(logical.trim_start_matches('/')),
            None => PathBuf::from(logical),
        }
    }

    /// The path the system names `real` by — the inverse of [`System::real`];
    /// `real` itself when it is outside the root.
    #[must_use]
    pub fn logical(&self, real: &Path) -> String {
        match &self.root {
            Some(root) => real.strip_prefix(root).map_or_else(
                |_| real.display().to_string(),
                |inside| format!("/{}", inside.display()),
            ),
            None => real.display().to_string(),
        }
    }

    /// Where the link `logical` ends, as the system names it; `None` when it
    /// leads nowhere.
    #[must_use]
    pub fn resolve(&self, logical: &str) -> Option<String> {
        std::fs::canonicalize(self.real(logical))
            .ok()
            .map(|p| self.logical(&p))
    }

    /// The system profile, on the disk.
    #[must_use]
    pub fn profile(&self) -> PathBuf {
        self.real(PROFILE)
    }

    /// The lock the machine flake of `toplevel` was built from, as the system
    /// carries it; `None` for a system that carries none (built before the
    /// machine flake, or by no machine flake at all).
    #[must_use]
    pub fn lock_of(&self, toplevel: &str) -> Option<Vec<u8>> {
        let inside = self.system_lock.as_deref()?;
        std::fs::read(self.real(toplevel).join(inside)).ok()
    }

    /// This boot's identity; `None` when it cannot be read.
    #[must_use]
    pub fn this_boot(&self) -> Option<String> {
        std::fs::read_to_string(self.real(BOOT_ID))
            .ok()
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
    }

    /// Root's lock of the person's data, on the disk.
    #[must_use]
    pub fn person_lock(&self) -> PathBuf {
        self.real(ronoctua_person::root::LOCK)
    }

    /// How many generations the boot menu lists: a count of at least one,
    /// or a refusal that says what the variable held instead.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Failed`], said.
    pub fn menu_limit(&self) -> Result<u64, crate::Refusal> {
        let text = needed(self.menu_limit.as_ref(), MENU_LIMIT)?;
        match text.parse::<u64>() {
            Ok(n) if n >= 1 => Ok(n),
            _ => {
                crate::say(
                    3,
                    &format!("{MENU_LIMIT} is {text:?}, not a count of generations"),
                );
                Err(crate::Refusal::of(crate::Why::Failed))
            }
        }
    }

    /// How long the bless waits for root's lock of the person's data:
    /// [`crate::BLESS_LOCK_WAIT_S`] when the variable is unset, a refusal
    /// that says what it held when it is not a count of seconds.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Failed`], said.
    pub fn bless_wait_s(&self) -> Result<u64, crate::Refusal> {
        let Some(text) = &self.bless_wait else {
            return Ok(crate::BLESS_LOCK_WAIT_S);
        };
        text.parse::<u64>().map_err(|_| {
            crate::say(
                3,
                &format!("{BLESS_WAIT} is {text:?}, not a count of seconds"),
            );
            crate::Refusal::of(crate::Why::Failed)
        })
    }

    /// `nix-env`, beside `nix`.
    #[must_use]
    pub fn nix_env(&self) -> Option<PathBuf> {
        self.nix.as_ref().map(|n| n.with_file_name("nix-env"))
    }
}

/// Whether `path` is a relative path that stays inside the folder it is
/// joined to: no root, no `..`, nothing empty.
fn is_inside(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// A value a verb needs, or a refusal that names the variable.
pub(crate) fn needed<T: Clone>(value: Option<&T>, name: &str) -> Result<T, crate::Refusal> {
    value.cloned().ok_or_else(|| {
        crate::say(
            3,
            &format!("{name} is not set: the unit that runs this names it"),
        );
        crate::Refusal::of(crate::Why::Failed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_moves_the_system_and_only_it() {
        let s = System::from_env(&Env::of(&[(SYSROOT, "/tmp/r"), (MENU_LIMIT, "5")]));
        assert!(!s.is_the_real_one());
        assert_eq!(s.real("/nix/store/x"), PathBuf::from("/tmp/r/nix/store/x"));
        assert_eq!(s.logical(Path::new("/tmp/r/nix/store/x")), "/nix/store/x");
        assert_eq!(s.logical(Path::new("/elsewhere")), "/elsewhere");
        assert_eq!(s.menu_limit(), Ok(5));
        let real = System::from_env(&Env::of(&[(SYSROOT, "/")]));
        assert!(real.is_the_real_one(), "`/` is the real root");
        assert_eq!(real.real("/nix/store/x"), PathBuf::from("/nix/store/x"));
        assert_eq!(real.profile(), PathBuf::from(PROFILE));
    }

    #[test]
    fn nix_env_is_beside_nix() {
        let s = System::from_env(&Env::of(&[(NIX, "/nix/store/abc-nix/bin/nix")]));
        assert_eq!(
            s.nix_env(),
            Some(PathBuf::from("/nix/store/abc-nix/bin/nix-env"))
        );
        assert_eq!(System::from_env(&Env::default()).nix_env(), None);
    }

    #[test]
    fn an_unreadable_limit_is_refused_and_an_empty_value_is_unset() {
        for bad in ["five", "0", "-1"] {
            let s = System::from_env(&Env::of(&[(MENU_LIMIT, bad)]));
            assert!(s.menu_limit().is_err(), "{bad}");
        }
        assert!(System::from_env(&Env::default()).menu_limit().is_err());
        assert_eq!(
            System::from_env(&Env::default()).bless_wait_s(),
            Ok(crate::BLESS_LOCK_WAIT_S)
        );
        assert_eq!(
            System::from_env(&Env::of(&[(BLESS_WAIT, "90")])).bless_wait_s(),
            Ok(90)
        );
        assert!(
            System::from_env(&Env::of(&[(BLESS_WAIT, "soon")]))
                .bless_wait_s()
                .is_err()
        );
        let s = System::from_env(&Env::of(&[(MACHINE_ATTR, "")]));
        assert_eq!(s.attr, None);
    }

    #[test]
    fn the_lock_a_system_carries_is_read_only_from_inside_it() {
        for good in ["etc/ronoctua/machine.lock", "machine.lock"] {
            let s = System::from_env(&Env::of(&[(SYSTEM_LOCK, good)]));
            assert_eq!(s.system_lock.as_deref(), Some(good));
        }
        for bad in ["/etc/x", "../x", "etc/../../x", "etc//x", "./x"] {
            let s = System::from_env(&Env::of(&[(SYSTEM_LOCK, bad)]));
            assert_eq!(s.system_lock, None, "{bad}");
        }
        assert_eq!(
            System::from_env(&Env::default()).lock_of("/nix/store/a"),
            None
        );
    }
}
