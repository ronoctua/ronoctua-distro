//! Where yt-dlp is, and whether this installation has it at all.

use std::path::{Path, PathBuf};

/// The variable the units set, inside the `mkIf` that decides whether the
/// package exists.
pub const VARIABLE: &str = "RONOCTUA_YT_DLP";

/// Where yt-dlp is, read from the ENVIRONMENT.
///
/// # Why not `option_env!`, which is what every other program path here uses
///
/// Because baked in, a store path is a REFERENCE, and this one would be a
/// reference to a package that is deliberately absent from half the
/// installations. yt-dlp lives under `ronoctua.media.enable`, which is
/// `mkDefault full` — so an `option_env!` would drag ffmpeg and yt-dlp into
/// the REDUCED profile through every binary that links this crate, and
/// `profile-minimal-closes` would fail naming a package nobody asked for. It is
/// the same trap `RONOCTUA_EMACSCLIENT` fell into, and the same fix: the value
/// is set by the unit, inside the `mkIf` that decides whether the package
/// exists at all.
///
/// The fallback is the bare name, which works in a devshell and fails inside a
/// unit with a minimal `PATH` — which is why the unit sets it.
#[must_use]
pub fn program() -> PathBuf {
    program_of(std::env::var(VARIABLE).ok().as_deref())
}

/// The same decision, over a value a test can hand it.
#[must_use]
pub fn program_of(set: Option<&str>) -> PathBuf {
    set.map(str::trim)
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from("yt-dlp"), PathBuf::from)
}

/// Whether this installation has yt-dlp at all.
///
/// The reduced profile does not, and a queue that accepted a media job there
/// would show a row that never moves; a page that offered lives there would
/// ask and never hear back. Both refuse at the door instead.
#[must_use]
pub fn installed() -> bool {
    installed_of(
        std::env::var(VARIABLE).ok().as_deref(),
        Path::new("/usr/bin/yt-dlp"),
    )
}

/// The same decision, over values a test can hand it.
#[must_use]
pub fn installed_of(set: Option<&str>, system: &Path) -> bool {
    set.is_some_and(|v| !v.trim().is_empty()) || system.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_says_where_it_is_and_a_devshell_finds_it_by_name() {
        assert_eq!(
            program_of(Some("/nix/store/x-yt-dlp/bin/yt-dlp")),
            PathBuf::from("/nix/store/x-yt-dlp/bin/yt-dlp")
        );
        assert_eq!(program_of(Some("  ")), PathBuf::from("yt-dlp"));
        assert_eq!(program_of(None), PathBuf::from("yt-dlp"));
    }

    #[test]
    fn a_blank_variable_is_not_an_installation() {
        let nowhere = Path::new("/nonexistent/ronoctua/yt-dlp");
        assert!(installed_of(Some("/x/bin/yt-dlp"), nowhere));
        assert!(!installed_of(Some(""), nowhere));
        assert!(!installed_of(None, nowhere));
    }
}
