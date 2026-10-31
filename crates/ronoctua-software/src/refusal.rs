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
    /// An Android app the record holds and Android no longer lists: taken
    /// out inside Android, so an update has nothing to move and does not put
    /// it back.
    NotInsideAndroid,
    /// The preferences file does not parse, and writing would replace it.
    WillNotParse,
    /// This order runs as root only (the Android image), or never as root.
    WrongUser,
    /// The host was not let in the person's `consent.json` (a refusal kept
    /// there, or the consent could not be written), and nothing was sent.
    NotLet,
    /// A program that came from a file with no address: there is no
    /// download a declaration could pin, so it cannot follow the person to
    /// another machine.
    NothingToPin,
    /// This machine's applier refused the person's declaration (a cap, a
    /// file it does not read): nothing of it changed here. The applier's own
    /// words are the trouble.
    DeclarationRefused,
    /// The person's declaration file does not read here (a newer format, or
    /// broken), and writing would replace it: it is left as it is.
    DeclarationUnreadable,
    /// The distro needs it: it cannot be kept uninstalled.
    Essential,
    /// The machine could not be rebuilt with the change; it runs as it was.
    ApplyFailed,
    /// This machine's name is not one a declaration can name (a hostname
    /// with capitals or spaces).
    MachineUnnamed,
    /// It left the person's declaration, and the system this machine runs
    /// still carries it: the next apply takes it out (the update held the
    /// machine when it was removed, or another computer removed it). Why it
    /// was not removed NOW, and an installed row's trouble until then.
    LeavesAtNextApply,
    /// The same, and the system that waits for the next restart already left
    /// it out: it leaves at that restart.
    LeavesAtNextRestart,
    /// Brought back HERE, and the decline names other computers only: it
    /// is theirs to bring back — or every computer's at once, asked first.
    DeclinedElsewhere,
    /// A part of the distro already offers this program: the person's own copy
    /// would only be shadowed (the first id in order wins the view), so it is
    /// refused rather than installed to sit there, redundant and invisible.
    ProvidedByAPart,
}

impl Refusal {
    /// Every one, for `software-words-are-total`.
    pub const ALL: [Self; 47] = [
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
        Self::NotInsideAndroid,
        Self::WillNotParse,
        Self::WrongUser,
        Self::NotLet,
        Self::NothingToPin,
        Self::DeclarationRefused,
        Self::DeclarationUnreadable,
        Self::Essential,
        Self::ApplyFailed,
        Self::MachineUnnamed,
        Self::LeavesAtNextApply,
        Self::LeavesAtNextRestart,
        Self::DeclinedElsewhere,
        Self::ProvidedByAPart,
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
            Self::NotInsideAndroid => "not-inside-android",
            Self::WillNotParse => "will-not-parse",
            Self::WrongUser => "wrong-user",
            Self::NotLet => "not-let",
            Self::NothingToPin => "nothing-to-pin",
            Self::DeclarationRefused => "declaration-refused",
            Self::DeclarationUnreadable => "declaration-unreadable",
            Self::Essential => "essential",
            Self::ApplyFailed => "apply-failed",
            Self::MachineUnnamed => "machine-unnamed",
            Self::LeavesAtNextApply => "leaves-at-next-apply",
            Self::LeavesAtNextRestart => "leaves-at-next-restart",
            Self::DeclinedElsewhere => "declined-elsewhere",
            Self::ProvidedByAPart => "provided-by-a-part",
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
