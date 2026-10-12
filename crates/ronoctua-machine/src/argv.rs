//! The words handed to every program this crate runs — pure, so what runs as
//! root is readable (and tested) without running it.
//!
//! The machine directory is always given as a `path:` flake. Without the
//! scheme Nix would take a folder that happens to hold a `.git` as a git
//! flake, and build only what git tracks; the machine directory is root's,
//! has no history, and is what it is on the disk.

use std::path::Path;

/// The experimental features the flake commands need, named on every call:
/// the machine's `nix.conf` turns them on, and a `nix` run with another
/// configuration (a test, the installer) must not depend on it.
pub const FEATURES: &str = "nix-command flakes";

/// The attribute path of the system a machine flake builds.
#[must_use]
pub fn toplevel_of(attr: &str) -> String {
    format!("nixosConfigurations.{attr}.config.system.build.toplevel")
}

/// Whether `attr` can name a `nixosConfigurations` attribute without
/// quoting: a user name (`[a-z_][a-z0-9_-]*`, at most 32 bytes).
#[must_use]
pub fn is_attr(attr: &str) -> bool {
    let mut bytes = attr.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'_'))
        && attr.len() <= 32
        && bytes.all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

/// The machine flake at `dir`, as a reference.
#[must_use]
pub fn flake(dir: &Path) -> String {
    format!("path:{}", dir.display())
}

/// `nix build` of the machine's system into `out_link`, never moving an
/// input: a lock that would have to change is a failed build, not a new
/// distro revision nobody asked for.
#[must_use]
pub fn build(dir: &Path, attr: &str, out_link: &Path) -> Vec<String> {
    vec![
        "build".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--no-update-lock-file".to_owned(),
        "--out-link".to_owned(),
        out_link.display().to_string(),
        format!("{}#{}", flake(dir), toplevel_of(attr)),
    ]
}

/// The same build, only measured: what would be downloaded and built.
#[must_use]
pub fn dry_run(dir: &Path, attr: &str) -> Vec<String> {
    vec![
        "build".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--no-update-lock-file".to_owned(),
        "--dry-run".to_owned(),
        format!("{}#{}", flake(dir), toplevel_of(attr)),
    ]
}

/// `nix flake lock` of a machine flake that has no lock yet.
#[must_use]
pub fn lock(dir: &Path) -> Vec<String> {
    vec![
        "flake".to_owned(),
        "lock".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        flake(dir),
    ]
}

/// `nix flake update` of the distro input alone — what the update's
/// `flake-update` step does, done by a rehearsal on its copy.
#[must_use]
pub fn update_distro(dir: &Path) -> Vec<String> {
    vec![
        "flake".to_owned(),
        "update".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        crate::DISTRO_INPUT.to_owned(),
        "--flake".to_owned(),
        flake(dir),
    ]
}

/// `nix-env` making `toplevel` the profile's newest generation (none is
/// made when it already is).
#[must_use]
pub fn set_profile(profile: &Path, toplevel: &str) -> Vec<String> {
    vec![
        "--profile".to_owned(),
        profile.display().to_string(),
        "--set".to_owned(),
        toplevel.to_owned(),
    ]
}

/// `bootctl list`, as JSON.
#[must_use]
pub fn list_entries() -> Vec<String> {
    vec![
        "--no-pager".to_owned(),
        "list".to_owned(),
        "--json=short".to_owned(),
    ]
}

/// `bootctl set-default <entry>`: the EFI variable `LoaderEntryDefault`,
/// which wins over `loader.conf`'s `default`. An empty entry unsets it, and
/// `loader.conf` — the profile's newest generation — is the default again.
#[must_use]
pub fn set_default(entry: &str) -> Vec<String> {
    vec!["set-default".to_owned(), entry.to_owned()]
}

/// `bootctl set-oneshot <entry>`: `LoaderEntryOneShot`, which the boot
/// loader reads and REMOVES at the next boot — the trial is tried once.
#[must_use]
pub fn set_oneshot(entry: &str) -> Vec<String> {
    vec!["set-oneshot".to_owned(), entry.to_owned()]
}

/// How a toplevel is switched to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    /// Activate now and install the boot loader.
    Switch,
    /// Only install the boot loader: the next boot is it.
    Boot,
}

/// The words of `<toplevel>/bin/switch-to-configuration`.
#[must_use]
pub fn switch(how: Switch) -> Vec<String> {
    vec![
        match how {
            Switch::Switch => "switch",
            Switch::Boot => "boot",
        }
        .to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_build_never_moves_an_input_and_reads_the_folder_as_it_is() {
        let words = build(
            Path::new("/etc/nixos"),
            "noctua",
            Path::new("/run/ronoctua-update/next"),
        );
        assert!(words.contains(&"--no-update-lock-file".to_owned()));
        assert_eq!(
            words.last().unwrap(),
            "path:/etc/nixos#nixosConfigurations.noctua.config.system.build.toplevel"
        );
        let at = words.iter().position(|w| w == "--out-link").unwrap();
        assert_eq!(words[at + 1], "/run/ronoctua-update/next");
        let dry = dry_run(Path::new("/run/x"), "noctua");
        assert!(
            dry.contains(&"--dry-run".to_owned())
                && dry.contains(&"--no-update-lock-file".to_owned())
        );
        assert!(
            !dry.contains(&"--out-link".to_owned()),
            "a rehearsal leaves no root"
        );
    }

    #[test]
    fn the_update_moves_the_distro_alone() {
        let words = update_distro(&PathBuf::from("/run/ronoctua-update/rehearsal"));
        let at = words.iter().position(|w| w == "update").unwrap();
        assert_eq!(words[at + 3], crate::DISTRO_INPUT);
        assert_eq!(words.last().unwrap(), "path:/run/ronoctua-update/rehearsal");
    }

    #[test]
    fn an_attribute_is_a_user_name() {
        for good in ["noctua", "_x", "a-b_9"] {
            assert!(is_attr(good), "{good}");
        }
        for bad in ["", "Noctua", "9x", "a.b", "a\"b", "a b", &"x".repeat(33)] {
            assert!(!is_attr(bad), "{bad}");
        }
    }

    #[test]
    fn bootctl_and_the_switch_take_fixed_words() {
        assert_eq!(set_default(""), ["set-default", ""]);
        assert_eq!(
            set_oneshot("nixos-generation-7.conf"),
            ["set-oneshot", "nixos-generation-7.conf"]
        );
        assert_eq!(list_entries(), ["--no-pager", "list", "--json=short"]);
        assert_eq!(switch(Switch::Boot), ["boot"]);
        assert_eq!(
            set_profile(Path::new("/nix/var/nix/profiles/system"), "/nix/store/a-b"),
            [
                "--profile",
                "/nix/var/nix/profiles/system",
                "--set",
                "/nix/store/a-b"
            ]
        );
    }
}
