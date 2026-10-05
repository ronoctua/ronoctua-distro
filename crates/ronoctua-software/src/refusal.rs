//! Why something was not done, as a closed set.
//!
//! A refusal is a VALUE with a sentence (`software-refusal-<word>` in every
//! language), never an absence: an empty row and a refused one look the same
//! on screen, and only one of them says why.

use serde::{Deserialize, Serialize};

/// One reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    /// The package's licence is not free, and the person has not said yes.
    UnfreeNeedsConsent,
    /// A download nothing vouches for, and the person has not said yes.
    UnverifiedNeedsConsent,
    /// Bigger than the person's threshold, and the person has not said yes.
    BigNeedsConsent,
    /// nixpkgs marks it insecure (known vulnerabilities).
    Insecure,
    /// nixpkgs marks it broken.
    Broken,
    /// nixpkgs has it, but not for this machine's platform.
    Unavailable,
    /// This machine has no support for that source (the minimal profile has
    /// no Waydroid and no `appimage-run`).
    SourceAbsent,
    /// The source is not ready yet (Android not prepared, no Flathub remote).
    SourceUnprepared,
    /// The flake installed it; only the flake can change it.
    DeclaredInTheFlake,
    /// An Android app built only for ARM.
    ArmOnly,
    /// No network.
    Offline,
    /// A service said to come back later (GitHub's 60 requests an hour).
    RateLimited,
    /// A hash or a signature did not match.
    Integrity,
    /// Larger than any sane answer to that request.
    TooLarge,
    /// The file is not an AppImage.
    #[serde(rename = "not-an-appimage")]
    NotAnAppImage,
    /// An old type-1 AppImage.
    #[serde(rename = "legacy-appimage")]
    LegacyAppImage,
    /// An AppImage whose payload is DwarFS, which `appimage-run` cannot open.
    #[serde(rename = "dwarfs-appimage")]
    DwarfsAppImage,
    /// The file is not an Android package.
    NotAnApk,
    /// Only `http` and `https` open as web apps.
    UrlScheme,
    /// An address with a user name or password in it.
    UrlCredentials,
    /// Not an address.
    UrlInvalid,
    /// Another web app already opens that exact window (same host, path and
    /// profile): Chrome would give both the same window id.
    DuplicateWindow,
    /// Chrome is not installed on this machine.
    NoBrowser,
    /// That Chrome profile does not exist.
    ProfileUnknown,
    /// The image is not usable as an icon (unreadable, or smaller than 32 px).
    IconUnusable,
    /// The profile belongs to the old `nix-env`, which `nix profile` refuses.
    ProfileIsNixEnv,
    /// Two packages want the same file in the profile.
    Conflict,
    /// This part of the work cannot be interrupted.
    NotCancellableNow,
    /// Nothing by that name.
    UnknownItem,
    /// The program failed; its words are the trouble.
    ToolFailed,
    /// The AppImage carries no update information and no known release page.
    NoUpdateInfo,
    /// Only a Nix package can be tried without installing.
    CannotTry,
    /// The Android session is not running.
    SessionDown,
    /// The preferences file does not parse, and writing would replace it.
    WillNotParse,
    /// This order runs as root only (the Android image), or never as root.
    WrongUser,
    /// The host was not let in the person's `consent.json` (a refusal kept
    /// there, or the consent could not be written), and nothing was sent.
    NotLet,
}

impl Refusal {
    /// Every one, for `software-words-are-total`.
    pub const ALL: [Self; 36] = [
        Self::UnfreeNeedsConsent,
        Self::UnverifiedNeedsConsent,
        Self::BigNeedsConsent,
        Self::Insecure,
        Self::Broken,
        Self::Unavailable,
        Self::SourceAbsent,
        Self::SourceUnprepared,
        Self::DeclaredInTheFlake,
        Self::ArmOnly,
        Self::Offline,
        Self::RateLimited,
        Self::Integrity,
        Self::TooLarge,
        Self::NotAnAppImage,
        Self::LegacyAppImage,
        Self::DwarfsAppImage,
        Self::NotAnApk,
        Self::UrlScheme,
        Self::UrlCredentials,
        Self::UrlInvalid,
        Self::DuplicateWindow,
        Self::NoBrowser,
        Self::ProfileUnknown,
        Self::IconUnusable,
        Self::ProfileIsNixEnv,
        Self::Conflict,
        Self::NotCancellableNow,
        Self::UnknownItem,
        Self::ToolFailed,
        Self::NoUpdateInfo,
        Self::CannotTry,
        Self::SessionDown,
        Self::WillNotParse,
        Self::WrongUser,
        Self::NotLet,
    ];

    /// The stem of its text key (`software-refusal-<word>`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::UnfreeNeedsConsent => "unfree-needs-consent",
            Self::UnverifiedNeedsConsent => "unverified-needs-consent",
            Self::BigNeedsConsent => "big-needs-consent",
            Self::Insecure => "insecure",
            Self::Broken => "broken",
            Self::Unavailable => "unavailable",
            Self::SourceAbsent => "source-absent",
            Self::SourceUnprepared => "source-unprepared",
            Self::DeclaredInTheFlake => "declared-in-the-flake",
            Self::ArmOnly => "arm-only",
            Self::Offline => "offline",
            Self::RateLimited => "rate-limited",
            Self::Integrity => "integrity",
            Self::TooLarge => "too-large",
            Self::NotAnAppImage => "not-an-appimage",
            Self::LegacyAppImage => "legacy-appimage",
            Self::DwarfsAppImage => "dwarfs-appimage",
            Self::NotAnApk => "not-an-apk",
            Self::UrlScheme => "url-scheme",
            Self::UrlCredentials => "url-credentials",
            Self::UrlInvalid => "url-invalid",
            Self::DuplicateWindow => "duplicate-window",
            Self::NoBrowser => "no-browser",
            Self::ProfileUnknown => "profile-unknown",
            Self::IconUnusable => "icon-unusable",
            Self::ProfileIsNixEnv => "profile-is-nix-env",
            Self::Conflict => "conflict",
            Self::NotCancellableNow => "not-cancellable-now",
            Self::UnknownItem => "unknown-item",
            Self::ToolFailed => "tool-failed",
            Self::NoUpdateInfo => "no-update-info",
            Self::CannotTry => "cannot-try",
            Self::SessionDown => "session-down",
            Self::WillNotParse => "will-not-parse",
            Self::WrongUser => "wrong-user",
            Self::NotLet => "not-let",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_is_listed_once_and_its_word_is_its_json() {
        let mut words: Vec<&str> = Refusal::ALL.iter().map(|r| r.word()).collect();
        let n = words.len();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), n, "a word is listed twice");
        for r in Refusal::ALL {
            assert_eq!(
                serde_json::to_string(&r).unwrap(),
                format!("\"{}\"", r.word())
            );
        }
    }
}
