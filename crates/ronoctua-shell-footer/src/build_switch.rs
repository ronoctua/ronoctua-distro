//! Which system build is running — this machine's own, or the local distro
//! repository's reference build — and the switch between the two
//! (`LEADER / b`, and the modeline glyph it draws).
//!
//! # Why this exists
//!
//! `nixos-rebuild switch --flake .#noctua` run from a distro checkout builds
//! the REFERENCE machine (`flake.nix`'s `referenceMachine`), which reads
//! `tests/fixtures/machine` and NOT `/etc/nixos/host.nix` — so it comes up on
//! the distro's default layout, profile and hostname rather than this
//! machine's. A developer runs that by hand to try the tree in a clean room,
//! and nothing on screen says the machine is no longer its own: the first sign
//! is a key typing the wrong character, or a profile missing its programs.
//! This draws the difference, and makes the way back one gesture.
//!
//! # Telling the two apart
//!
//! `nix/modules/machine.nix` writes `/etc/ronoctua/machine.lock` — the machine
//! flake's own `flake.lock` — only where the build HAS a machine flake of its
//! own (`config.ronoctua.machine.source != null`), which the reference build
//! leaves null. The file's PRESENCE is "this is the machine's own build"; its
//! absence, "this is a reference build from a checkout". The decision is a
//! pure function of that one bit, so the surface tests it with a path that is
//! there or not — no compositor, no root.
//!
//! # Where the checkout is
//!
//! The owner chose [parse the `/etc/nixos` flake] over a new option: the
//! machine's flake already names the distro it builds from, and a developer's
//! names a LOCAL one. `inputs.ronoctua.url = "git+file:///…"` (or a bare
//! `path:/…` that is not in the store) is a checkout on this disk; a
//! `github:` or a `path:/nix/store/…` is not, and then there is no reference
//! build to offer and the glyph stays away. `/etc/nixos` is on the persistent
//! disk, the same file whichever generation booted, so the checkout is found
//! and the way back to `/etc/nixos` is known from either build.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// When the modeline draws the build glyph — the dev page's one setting
/// (`crate::build_page`), kept in `modeline.json` (`crate::adapt::Prefs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Show {
    /// Only on a reference build — THE DISTRO DEFAULT. The machine's own build
    /// is where a developer lives, and a mark on it every day would be the
    /// chrome invariant 14 keeps off; the reference build is the exception
    /// worth a glyph, so the glyph IS the exception.
    #[default]
    Dev,
    /// On both builds, wherever there is a checkout to switch to.
    Always,
    /// Never — the glyph is off, and the page is reached by its key alone.
    Never,
}

impl Show {
    /// The next value `RET` on the page's row walks to, round the three.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Dev => Self::Always,
            Self::Always => Self::Never,
            Self::Never => Self::Dev,
        }
    }

    /// Whether it is the default — so `modeline.json` keeps nothing for a
    /// setting nobody changed, the house rule for every switch in that file.
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Dev)
    }
}

/// Which build the glyph draws, under this setting — or `None` for no glyph.
/// A machine with no checkout has nothing to switch to and never draws one;
/// otherwise `Never` draws nothing, `Dev` draws only a reference build, and
/// `Always` draws whichever is running. Pure: the view tests it with no
/// compositor.
#[must_use]
pub fn glyph(show: Show, build: Build, has_checkout: bool) -> Option<Build> {
    if !has_checkout {
        return None;
    }
    match show {
        Show::Never => None,
        Show::Dev => (build == Build::Reference).then_some(build),
        Show::Always => Some(build),
    }
}

/// Present iff the running build has a machine flake of its own — see the
/// module's note. Absent on the reference build.
pub const MACHINE_LOCK: &str = "/etc/ronoctua/machine.lock";
/// The machine's flake, read for the distro checkout it names.
pub const MACHINE_FLAKE: &str = "/etc/nixos/flake.nix";
/// The link whose target names the running build (`nixos-system-<host>-<ver>`).
pub const CURRENT_SYSTEM: &str = "/run/current-system";

/// Which of the two builds is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Build {
    /// The machine's own build — `/etc/nixos`, with its `host.nix`.
    Own,
    /// A reference build from a distro checkout — no `host.nix`.
    Reference,
}

impl Build {
    /// The machine's own build HAS the lock; the reference build does not.
    #[must_use]
    pub fn of(lock_present: bool) -> Self {
        if lock_present {
            Self::Own
        } else {
            Self::Reference
        }
    }
}

/// Everything the glyph and the switch read, gathered once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Which build is running.
    pub build: Build,
    /// The running build's hostname, from `/run/current-system`'s target —
    /// `noctua-z16` on the machine's own, `noctua-reference` on the reference.
    pub hostname: Option<String>,
    /// The distro checkout `/etc/nixos` builds from, when it names a local one.
    /// `None` leaves the glyph away: an installed machine has nothing to switch
    /// to.
    pub checkout: Option<PathBuf>,
}

impl State {
    /// The build a switch lands on — what names the service it starts
    /// (`ronoctua-build-switch-{own,reference}`) and the line it says.
    #[must_use]
    pub fn other_build(&self) -> Build {
        match self.build {
            Build::Own => Build::Reference,
            Build::Reference => Build::Own,
        }
    }
}

/// The local distro checkout a machine flake names, or `None` when it names a
/// published distro (a `github:`/`https:` input) or a pinned store copy.
///
/// It reads the ONE line the installer's template and a developer's edit both
/// write — `inputs.ronoctua.url = "<url>";` — and keeps the path only of the
/// two schemes that are a working tree on this disk.
#[must_use]
pub fn checkout_of(flake_nix: &str) -> Option<PathBuf> {
    let url = ronoctua_url(flake_nix)?;
    local_path(&url).map(PathBuf::from)
}

/// The `ronoctua` input's URL from a machine `flake.nix`, however the line is
/// spaced: the quoted value after a `ronoctua` … `url` on one line.
fn ronoctua_url(flake_nix: &str) -> Option<String> {
    flake_nix.lines().find_map(|line| {
        let line = line.trim_start();
        if line.starts_with('#') {
            return None;
        }
        let after = line.split_once("ronoctua")?.1;
        let after = after.split_once("url")?.1;
        quoted(after)
    })
}

/// The contents of the first `"…"` in `s`.
fn quoted(s: &str) -> Option<String> {
    let rest = s.split_once('"')?.1;
    let (inner, _) = rest.split_once('"')?;
    Some(inner.to_owned())
}

/// The local working-tree path a flake URL points at, or `None` when it points
/// elsewhere. `git+file:///p` and a bare `path:/p` are local; the Nix store is
/// not a working tree, and a remote scheme is not on this disk.
fn local_path(url: &str) -> Option<String> {
    let path = url
        .strip_prefix("git+file://")
        .or_else(|| url.strip_prefix("path:"))?;
    // Drop a `?ref=…`/`?rev=…` query the input may carry.
    let path = path.split('?').next().unwrap_or(path);
    (path.starts_with('/') && !path.starts_with("/nix/store/")).then(|| path.to_owned())
}

/// The hostname in a `/run/current-system` target, which NixOS names
/// `…-nixos-system-<host>-<version>`. `<version>` begins with a digit, so the
/// host is what stands between `nixos-system-` and the first `-<digit>`.
#[must_use]
pub fn hostname_of(current_system_target: &str) -> Option<String> {
    let name = current_system_target
        .rsplit('/')
        .next()
        .unwrap_or(current_system_target);
    let after = name.split_once("nixos-system-")?.1;
    // The version is the tail from the first `-` that a digit follows.
    let mut cut = after.len();
    let bytes = after.as_bytes();
    for (i, w) in bytes.windows(2).enumerate() {
        if w[0] == b'-' && w[1].is_ascii_digit() {
            cut = i;
            break;
        }
    }
    let host = &after[..cut];
    (!host.is_empty()).then(|| host.to_owned())
}

/// Read the running build's state from the filesystem — the lock's presence,
/// the current system's hostname and the machine flake's checkout.
#[must_use]
pub fn read() -> State {
    let lock_present = std::path::Path::new(MACHINE_LOCK).exists();
    let hostname = std::fs::read_link(CURRENT_SYSTEM)
        .ok()
        .and_then(|p| hostname_of(&p.to_string_lossy()));
    let checkout = std::fs::read_to_string(MACHINE_FLAKE)
        .ok()
        .and_then(|f| checkout_of(&f))
        .filter(|p| p.exists());
    State {
        build: Build::of(lock_present),
        hostname,
        checkout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_is_the_machines_own_build() {
        assert_eq!(Build::of(true), Build::Own);
        assert_eq!(Build::of(false), Build::Reference);
    }

    #[test]
    fn a_git_file_url_is_a_checkout() {
        let flake = r#"{
          inputs.ronoctua.url = "git+file:///home/person/src/ronoctua-distro";
          outputs = { ... }: { };
        }"#;
        assert_eq!(
            checkout_of(flake),
            Some(PathBuf::from("/home/person/src/ronoctua-distro"))
        );
    }

    #[test]
    fn a_path_url_that_is_not_the_store_is_a_checkout() {
        let flake = r#"inputs.ronoctua.url = "path:/home/p/distro";"#;
        assert_eq!(checkout_of(flake), Some(PathBuf::from("/home/p/distro")));
    }

    #[test]
    fn a_query_is_dropped() {
        let flake = r#"inputs.ronoctua.url = "git+file:///home/p/distro?ref=main";"#;
        assert_eq!(checkout_of(flake), Some(PathBuf::from("/home/p/distro")));
    }

    #[test]
    fn a_store_copy_or_a_remote_is_no_checkout() {
        for url in [
            r#"inputs.ronoctua.url = "path:/nix/store/abc-source";"#,
            r#"inputs.ronoctua.url = "github:ronoctua/ronoctua-distro";"#,
            r#"inputs.ronoctua.url = "git+https://example/d";"#,
        ] {
            assert_eq!(checkout_of(url), None, "{url}");
        }
    }

    #[test]
    fn a_commented_line_is_not_read() {
        let flake = r#"
          # inputs.ronoctua.url = "git+file:///old/path";
          inputs.ronoctua.url = "github:ronoctua/ronoctua-distro";
        "#;
        assert_eq!(checkout_of(flake), None);
    }

    #[test]
    fn the_hostname_stands_before_the_version() {
        assert_eq!(
            hostname_of("/nix/store/xxxx-nixos-system-noctua-z16-26.05.20260731.5b4f72e"),
            Some("noctua-z16".to_owned())
        );
        assert_eq!(
            hostname_of("/nix/store/xxxx-nixos-system-noctua-reference-26.05.1"),
            Some("noctua-reference".to_owned())
        );
    }

    #[test]
    fn the_default_is_the_glyph_only_on_a_reference_build() {
        assert_eq!(Show::default(), Show::Dev);
        assert!(Show::Dev.is_default() && !Show::Always.is_default() && !Show::Never.is_default());
        // No checkout: never a glyph, whatever the setting.
        for show in [Show::Dev, Show::Always, Show::Never] {
            assert_eq!(glyph(show, Build::Own, false), None);
            assert_eq!(glyph(show, Build::Reference, false), None);
        }
        // Dev (the default): the reference build only.
        assert_eq!(glyph(Show::Dev, Build::Own, true), None);
        assert_eq!(
            glyph(Show::Dev, Build::Reference, true),
            Some(Build::Reference)
        );
        // Always: whichever runs. Never: neither.
        assert_eq!(glyph(Show::Always, Build::Own, true), Some(Build::Own));
        assert_eq!(
            glyph(Show::Always, Build::Reference, true),
            Some(Build::Reference)
        );
        assert_eq!(glyph(Show::Never, Build::Own, true), None);
        assert_eq!(glyph(Show::Never, Build::Reference, true), None);
    }

    #[test]
    fn the_setting_walks_the_three_round() {
        assert_eq!(Show::Dev.next(), Show::Always);
        assert_eq!(Show::Always.next(), Show::Never);
        assert_eq!(Show::Never.next(), Show::Dev);
    }

    #[test]
    fn the_other_direction_is_the_build_not_running() {
        let own = State {
            build: Build::Own,
            hostname: None,
            checkout: Some(PathBuf::from("/home/p/distro")),
        };
        assert_eq!(own.other_build(), Build::Reference);
        let reference = State {
            build: Build::Reference,
            hostname: None,
            checkout: Some(PathBuf::from("/home/p/distro")),
        };
        assert_eq!(reference.other_build(), Build::Own);
    }
}
